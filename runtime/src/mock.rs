//! Deterministic mock runtime for the interactive console.

use crate::{
    backends::{BackendHealth, BackendManager, BackendSummary, LoadedModel},
    hardware::{HardwareService, HardwareSnapshot},
    models::{ModelDiscovery, ModelMetadata, ModelRegistry, ModelStatus},
    GenerationMessage, GenerationRequest, GenerationResult, GenerationStatistics, GenerationStream,
    LlamaCppAdapter, RuntimeApi, RuntimeConfig, RuntimeError, RuntimeService, RuntimeSnapshot,
    RuntimeStatus,
};
use oid_shared::{EventBus, LifecycleState, RuntimeEvent};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Instant;

/// Fake runtime service used until a real model backend is integrated.
#[derive(Clone, Debug)]
pub struct MockRuntime {
    config: RuntimeConfig,
    bus: EventBus,
    backends: BackendManager,
    models: ModelRegistry,
    hardware: HardwareService,
    started_at: Instant,
    loaded: Arc<Mutex<Option<LoadedModel>>>,
    active_generation: Arc<Mutex<Option<Arc<AtomicBool>>>>,
}

impl MockRuntime {
    /// Start a mock runtime and publish its startup event.
    #[must_use]
    pub fn start(config: RuntimeConfig, bus: EventBus) -> Self {
        let backends = BackendManager::new(bus.clone());
        let llama = LlamaCppAdapter::new();
        let _ = backends.register(Box::new(llama));
        let _ = backends.enable("llama.cpp");
        let hardware = HardwareService::detect(&bus);
        let models = ModelRegistry::in_memory(bus.clone());
        let discovery = ModelDiscovery::new(ModelDiscovery::default_directories());
        let _ = discovery.discover(&models, "llama.cpp");
        bus.publish(&RuntimeEvent::RuntimeStarted);
        bus.publish(&RuntimeEvent::HealthUpdated("Healthy".to_owned()));
        Self {
            config,
            bus,
            backends,
            models,
            hardware,
            started_at: Instant::now(),
            loaded: Arc::new(Mutex::new(None)),
            active_generation: Arc::new(Mutex::new(None)),
        }
    }

    /// Return the configured runtime name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.config.name
    }
}

impl RuntimeApi for MockRuntime {
    fn status(&self) -> RuntimeStatus {
        RuntimeStatus {
            state: LifecycleState::Ready,
            version: "0.1",
        }
    }
}

impl RuntimeService for MockRuntime {
    fn status(&self) -> RuntimeStatus {
        <Self as RuntimeApi>::status(self)
    }

    fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            health: "Healthy",
            backend: self
                .backends
                .active_backend()
                .unwrap_or_else(|| "None".to_owned()),
            models: self.models.list().len(),
            memory: self
                .loaded
                .lock()
                .ok()
                .and_then(|loaded| {
                    loaded
                        .as_ref()
                        .map(|model| format!("{} MB", model.memory_bytes / 1_048_576))
                })
                .unwrap_or_else(|| "--".to_owned()),
            loaded_model: self
                .loaded
                .lock()
                .ok()
                .and_then(|loaded| loaded.as_ref().map(|model| model.model_id.clone())),
            uptime_seconds: self.started_at.elapsed().as_secs(),
            backend_version: self
                .backends
                .discover()
                .into_iter()
                .find(|backend| backend.active)
                .and_then(|backend| backend.descriptor.library_version),
            model_memory_bytes: self
                .loaded
                .lock()
                .ok()
                .and_then(|loaded| loaded.as_ref().map(|model| model.memory_bytes)),
            generating: false,
            current_tokens_per_second: None,
            loaded_context: None,
        }
    }

    fn backend_list(&self) -> Vec<BackendSummary> {
        self.backends.discover()
    }

    fn backend_health(&self, id: &str) -> Option<BackendHealth> {
        self.backends.health(id)
    }

    fn model_list(&self) -> Vec<ModelMetadata> {
        self.models.list()
    }

    fn model_inspect(&self, id: &str) -> Option<ModelMetadata> {
        self.models.inspect(id)
    }

    fn model_load(&self, id: &str) -> Result<(), RuntimeError> {
        if self
            .loaded
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?
            .is_some()
        {
            return Err(RuntimeError::ModelLifecycle(
                "only one model may be loaded".to_owned(),
            ));
        }
        let model = self
            .models
            .inspect(id)
            .ok_or_else(|| RuntimeError::ModelNotFound(id.to_owned()))?;
        self.bus.publish(&RuntimeEvent::ModelLoading(id.to_owned()));
        self.models.set_status(id, &ModelStatus::Loading)?;
        match self.backends.load_model(&model) {
            Ok(loaded) => {
                self.models.set_status(id, &ModelStatus::Loaded)?;
                *self
                    .loaded
                    .lock()
                    .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))? =
                    Some(loaded);
                Ok(())
            }
            Err(error) => {
                let _ = self.models.set_status(id, &ModelStatus::Failed);
                self.bus
                    .publish(&RuntimeEvent::ErrorRaised(error.to_string()));
                Err(error)
            }
        }
    }

    fn model_unload(&self, id: &str) -> Result<(), RuntimeError> {
        self.bus
            .publish(&RuntimeEvent::ModelUnloading(id.to_owned()));
        self.models.set_status(id, &ModelStatus::Unloading)?;
        self.backends.unload_model(id)?;
        self.models.set_status(id, &ModelStatus::Registered)?;
        *self
            .loaded
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))? = None;
        Ok(())
    }

    fn tokenize(&self, text: &str) -> Result<usize, RuntimeError> {
        let count = self.backends.tokenize(text)?;
        self.bus
            .publish(&RuntimeEvent::TokenizerReady("llama.cpp".to_owned()));
        Ok(count)
    }

    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::too_many_lines)]
    fn generate(&self, request: GenerationRequest) -> Result<GenerationStream, RuntimeError> {
        if self
            .loaded
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?
            .is_none()
        {
            return Err(RuntimeError::ModelLifecycle(
                "load a model before generating".to_owned(),
            ));
        }
        let (sender, receiver) = mpsc::channel();
        let cancellation = Arc::new(AtomicBool::new(false));
        *self
            .active_generation
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("generation unavailable".to_owned()))? =
            Some(cancellation.clone());
        let cancel_for_worker = cancellation.clone();
        let bus = self.bus.clone();
        let backends = self.backends.clone();
        let active_generation = self.active_generation.clone();
        thread::spawn(move || {
            let started = Instant::now();
            bus.publish(&RuntimeEvent::GenerationStarted(request.request_id.clone()));
            let backend_request = crate::backends::GenerationRequest {
                request_id: request.request_id.clone(),
                model_id: request.model_id.clone(),
                input: request.prompt.clone(),
                options: request.options.clone(),
            };
            let mut generated = String::new();
            let mut generated_tokens = 0_u64;
            let mut first = true;
            let result = backends.generate_streaming(
                &backend_request,
                &mut |token| {
                    if cancel_for_worker.load(Ordering::SeqCst) {
                        return false;
                    }
                    if first {
                        bus.publish(&RuntimeEvent::FirstToken(request.request_id.clone()));
                        first = false;
                    }
                    generated.push_str(token);
                    generated_tokens += 1;
                    let _ = sender.send(GenerationMessage::Token(token.to_owned()));
                    bus.publish(&RuntimeEvent::TokenGenerated(request.request_id.clone()));
                    true
                },
                &cancel_for_worker,
            );
            if cancel_for_worker.load(Ordering::SeqCst)
                || result
                    .as_ref()
                    .is_err_and(|error| error.to_string().contains("cancelled"))
            {
                let latency_ms = started.elapsed().as_millis();
                let statistics = GenerationStatistics {
                    generated_tokens,
                    tokens_per_second: if latency_ms == 0 {
                        generated_tokens as f64
                    } else {
                        generated_tokens as f64 / (latency_ms as f64 / 1000.0)
                    },
                    latency_ms,
                    inference_time_ms: latency_ms,
                    ..GenerationStatistics::default()
                };
                let _ = sender.send(GenerationMessage::Cancelled(GenerationResult {
                    request_id: request.request_id.clone(),
                    text: generated,
                    statistics,
                }));
                bus.publish(&RuntimeEvent::GenerationCancelled(
                    request.request_id.clone(),
                ));
                if let Ok(mut active) = active_generation.lock() {
                    if active
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &cancel_for_worker))
                    {
                        *active = None;
                    }
                }
                return;
            }
            let statistics = match result {
                Ok(statistics) => statistics,
                Err(error) => {
                    let message = error.to_string();
                    let _ = sender.send(GenerationMessage::Failed(error));
                    bus.publish(&RuntimeEvent::GenerationFailed(message));
                    if let Ok(mut active) = active_generation.lock() {
                        if active
                            .as_ref()
                            .is_some_and(|current| Arc::ptr_eq(current, &cancel_for_worker))
                        {
                            *active = None;
                        }
                    }
                    return;
                }
            };
            let latency_ms = started.elapsed().as_millis();
            let statistics = GenerationStatistics {
                prompt_tokens: statistics.prompt_tokens,
                generated_tokens: statistics.generated_tokens,
                tokens_per_second: if latency_ms == 0 {
                    statistics.generated_tokens as f64
                } else {
                    statistics.generated_tokens as f64 / (latency_ms as f64 / 1000.0)
                },
                latency_ms,
                inference_time_ms: latency_ms,
                context_tokens: statistics.context_tokens,
            };
            let result = GenerationResult {
                request_id: request.request_id.clone(),
                text: generated,
                statistics,
            };
            let _ = sender.send(GenerationMessage::Completed(result));
            bus.publish(&RuntimeEvent::GenerationCompleted(request.request_id));
            if let Ok(mut active) = active_generation.lock() {
                if active
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &cancel_for_worker))
                {
                    *active = None;
                }
            }
        });
        Ok(GenerationStream::new(receiver, cancellation))
    }

    fn cancel_generation(&self) {
        if let Ok(active) = self.active_generation.lock() {
            if let Some(cancellation) = active.as_ref() {
                cancellation.store(true, Ordering::SeqCst);
            }
        }
    }

    fn hardware(&self) -> HardwareSnapshot {
        self.hardware.snapshot()
    }

    fn event_bus(&self) -> EventBus {
        self.bus.clone()
    }

    fn execute_command(&self, command: &str) -> String {
        self.bus
            .publish(&RuntimeEvent::CommandStarted(command.to_owned()));
        self.bus.publish(&RuntimeEvent::ThinkingStarted);
        self.bus.publish(&RuntimeEvent::ThinkingFinished);
        self.bus.publish(&RuntimeEvent::StreamingStarted);
        self.bus.publish(&RuntimeEvent::StreamingStopped);
        self.bus
            .publish(&RuntimeEvent::CommandCompleted(command.to_owned()));
        format!("Mock runtime received: {command}")
    }

    fn input_started(&self) {
        self.bus.publish(&RuntimeEvent::InputStarted);
    }

    fn input_stopped(&self) {
        self.bus.publish(&RuntimeEvent::InputStopped);
    }

    fn stop(&self) {
        self.bus.publish(&RuntimeEvent::RuntimeStopped);
    }
}

/// Construct a mock runtime from configuration, preserving the startup error contract.
///
/// # Errors
///
/// Returns an invalid-configuration error when required configuration values
/// are empty.
pub fn start(config: RuntimeConfig, bus: EventBus) -> Result<MockRuntime, RuntimeError> {
    config
        .validate()
        .map_err(RuntimeError::InvalidConfiguration)?;
    Ok(MockRuntime::start(config, bus))
}

#[cfg(test)]
mod tests {
    use super::MockRuntime;
    use crate::RuntimeService;
    use oid_shared::{EventBus, RuntimeConfig, RuntimeEvent};

    #[test]
    fn mock_runtime_is_deterministic_and_publishes_events() {
        let bus = EventBus::new();
        let receiver = bus.subscribe();
        let runtime = MockRuntime::start(RuntimeConfig::default(), bus);
        assert_eq!(runtime.snapshot().backend, "None");
        assert_eq!(runtime.snapshot().models, 0);
        let _ = runtime.execute_command("status");
        assert!(receiver
            .try_iter()
            .any(|event| matches!(event, RuntimeEvent::RuntimeStarted)));
    }
}
