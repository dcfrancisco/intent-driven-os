//! Talvik's production runtime service and composition root.

use crate::{
    backends::{
        Backend, BackendHealth, BackendManager, BackendSummary, EmbeddingsRequest,
        EmbeddingsResult, LoadedModel,
    },
    hardware::{HardwareService, HardwareSnapshot},
    models::{ModelAcquirer, ModelDiscovery, ModelMetadata, ModelRegistry, ModelStatus},
    GenerationMessage, GenerationRequest, GenerationResult, GenerationStatistics, GenerationStream,
    LlamaCppAdapter, RuntimeApi, RuntimeConfig, RuntimeError, RuntimeService, RuntimeSnapshot,
    RuntimeStatus,
};
use oid_shared::{EventBus, LifecycleState, RuntimeEvent};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Instant;

static GENERATION_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Production runtime service used by the console and future clients.
#[derive(Clone, Debug)]
pub struct MarinaRuntime {
    config: RuntimeConfig,
    bus: EventBus,
    backends: BackendManager,
    models: ModelRegistry,
    hardware: HardwareService,
    started_at: Instant,
    loaded: Arc<Mutex<BTreeMap<String, LoadedModel>>>,
    active_generation: Arc<Mutex<Option<(String, String, Arc<AtomicBool>)>>>,
}

impl MarinaRuntime {
    /// Start Talvik's production runtime and publish its startup event.
    #[must_use]
    pub fn start(config: RuntimeConfig, bus: EventBus) -> Self {
        Self::start_with_backend(config, bus, Box::new(LlamaCppAdapter::new()))
    }

    fn start_with_backend(config: RuntimeConfig, bus: EventBus, backend: Box<dyn Backend>) -> Self {
        let backends = BackendManager::new(bus.clone());
        let backend_id = backend.descriptor().id;
        let _ = backends.register(backend);
        let _ = backends.enable(&backend_id);
        let hardware = HardwareService::detect(&bus);
        let models = config
            .registry_path
            .as_ref()
            .and_then(|path| ModelRegistry::open(path, bus.clone()).ok())
            .unwrap_or_else(|| ModelRegistry::in_memory(bus.clone()));
        let discovery_directories = config
            .model_directory
            .clone()
            .map_or_else(ModelDiscovery::default_directories, |directory| {
                vec![directory]
            });
        let discovery = ModelDiscovery::new(discovery_directories);
        let _ = discovery.discover(&models, &backend_id);
        for model in models.list() {
            if !matches!(
                model.status,
                ModelStatus::Registered | ModelStatus::Unavailable
            ) {
                let _ = models.set_status(&model.id, &ModelStatus::Registered);
            }
        }
        bus.publish(&RuntimeEvent::RuntimeStarted);
        bus.publish(&RuntimeEvent::HealthUpdated("Healthy".to_owned()));
        Self {
            config,
            bus,
            backends,
            models,
            hardware,
            started_at: Instant::now(),
            loaded: Arc::new(Mutex::new(BTreeMap::new())),
            active_generation: Arc::new(Mutex::new(None)),
        }
    }

    /// Return the configured runtime name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.config.name
    }
}

impl RuntimeApi for MarinaRuntime {
    fn status(&self) -> RuntimeStatus {
        RuntimeStatus {
            state: LifecycleState::Ready,
            version: "0.1",
        }
    }
}

fn admit_generation(
    hardware: &HardwareService,
    loaded: &Arc<Mutex<BTreeMap<String, LoadedModel>>>,
    request: &GenerationRequest,
) -> Result<(), RuntimeError> {
    let model_bytes = loaded
        .lock()
        .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?
        .values()
        .map(|model| model.memory_bytes)
        .sum();
    admit_memory(
        hardware.snapshot().available_ram_bytes,
        model_bytes,
        request.options.context_size,
    )
}

fn admit_memory(
    available: Option<u64>,
    model_bytes: u64,
    context_size: u32,
) -> Result<(), RuntimeError> {
    const SAFETY_MARGIN_BYTES: u64 = 512 * 1_048_576;
    const KV_BYTES_PER_CONTEXT_TOKEN: u64 = 16 * 1024;
    let available = available.ok_or_else(|| {
        RuntimeError::ModelLifecycle(
            "memory admission unavailable: available system memory is unknown".to_owned(),
        )
    })?;
    let context_bytes = u64::from(context_size)
        .checked_mul(KV_BYTES_PER_CONTEXT_TOKEN)
        .ok_or_else(|| {
            RuntimeError::ModelLifecycle("context memory estimate overflow".to_owned())
        })?;
    let required = model_bytes
        .saturating_add(context_bytes)
        .saturating_add(SAFETY_MARGIN_BYTES);
    if available < required {
        return Err(RuntimeError::ModelLifecycle(format!(
            "memory admission rejected: available={} required={} model={} context={}",
            available, required, model_bytes, context_bytes
        )));
    }
    Ok(())
}

impl RuntimeService for MarinaRuntime {
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
            memory: self.loaded.lock().ok().map_or_else(
                || "--".to_owned(),
                |loaded| {
                    let bytes: u64 = loaded.values().map(|model| model.memory_bytes).sum();
                    if loaded.is_empty() {
                        "--".to_owned()
                    } else {
                        format!("{} MB", bytes / 1_048_576)
                    }
                },
            ),
            loaded_model: self
                .loaded
                .lock()
                .ok()
                .and_then(|loaded| loaded.keys().next().cloned()),
            loaded_models: self
                .loaded
                .lock()
                .map(|loaded| loaded.keys().cloned().collect())
                .unwrap_or_default(),
            uptime_seconds: self.started_at.elapsed().as_secs(),
            backend_version: self
                .backends
                .discover()
                .into_iter()
                .find(|backend| backend.active)
                .and_then(|backend| backend.descriptor.library_version),
            model_memory_bytes: self.loaded.lock().ok().and_then(|loaded| {
                (!loaded.is_empty()).then(|| loaded.values().map(|model| model.memory_bytes).sum())
            }),
            generating: self
                .active_generation
                .lock()
                .is_ok_and(|active| active.is_some()),
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

    fn model_pull(
        &self,
        source: &str,
        model_id: Option<&str>,
        checksum: Option<&str>,
    ) -> Result<ModelMetadata, RuntimeError> {
        let directory = self
            .config
            .model_directory
            .clone()
            .or_else(|| std::env::var_os("TALVIK_MODEL_DIR").map(std::path::PathBuf::from))
            .or_else(|| std::env::var_os("MARINA_MODEL_DIR").map(std::path::PathBuf::from))
            .unwrap_or_else(|| {
                crate::config::talvik_home_directory()
                    .map(|home| home.join("models"))
                    .unwrap_or_else(|| std::path::PathBuf::from(".marina/models"))
            });
        let model = ModelAcquirer::acquire(source, &directory, model_id, checksum)?;
        self.models.register(model.clone())?;
        Ok(model)
    }

    fn model_load(&self, id: &str) -> Result<(), RuntimeError> {
        if self
            .loaded
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?
            .contains_key(id)
        {
            return Ok(());
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
                self.loaded
                    .lock()
                    .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?
                    .insert(id.to_owned(), loaded);
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
        if self
            .active_generation
            .lock()
            .is_ok_and(|active| active.is_some())
        {
            return Err(RuntimeError::ModelLifecycle(
                "cannot unload a model while generation is active".to_owned(),
            ));
        }
        self.bus
            .publish(&RuntimeEvent::ModelUnloading(id.to_owned()));
        self.models.set_status(id, &ModelStatus::Unloading)?;
        self.backends.unload_model(id)?;
        self.models.set_status(id, &ModelStatus::Registered)?;
        self.loaded
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?
            .remove(id);
        Ok(())
    }

    fn tokenize(&self, text: &str) -> Result<usize, RuntimeError> {
        let count = self.backends.tokenize(text)?;
        self.bus
            .publish(&RuntimeEvent::TokenizerReady("llama.cpp".to_owned()));
        Ok(count)
    }

    fn embeddings(&self, request: &EmbeddingsRequest) -> Result<EmbeddingsResult, RuntimeError> {
        self.backends.embeddings(request)
    }

    fn tool_support(&self) -> crate::backends::ToolSupport {
        self.backends.tool_support()
    }

    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::too_many_lines)]
    fn generate(&self, request: GenerationRequest) -> Result<GenerationStream, RuntimeError> {
        if !self
            .loaded
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?
            .contains_key(&request.model_id)
        {
            return Err(RuntimeError::ModelLifecycle(
                "requested model is not loaded".to_owned(),
            ));
        }
        admit_generation(&self.hardware, &self.loaded, &request)?;
        let cancellation = Arc::new(AtomicBool::new(false));
        let generation_id = format!(
            "talvik-gen-{}-{}",
            std::process::id(),
            GENERATION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let mut active_generation = self
            .active_generation
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("generation unavailable".to_owned()))?;
        if active_generation.is_some() {
            return Err(RuntimeError::ModelLifecycle(
                "generation capacity exhausted: one active generation is supported".to_owned(),
            ));
        }
        *active_generation = Some((
            request.request_id.clone(),
            generation_id.clone(),
            cancellation.clone(),
        ));
        drop(active_generation);
        let (sender, receiver) = mpsc::channel();
        let cancel_for_worker = cancellation.clone();
        let bus = self.bus.clone();
        let backends = self.backends.clone();
        let active_generation = self.active_generation.clone();
        let generation_id_for_worker = generation_id.clone();
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
                    metrics_known: true,
                    ..GenerationStatistics::default()
                };
                if let Ok(mut active) = active_generation.lock() {
                    if active
                        .as_ref()
                        .is_some_and(|(_, _, current)| Arc::ptr_eq(current, &cancel_for_worker))
                    {
                        *active = None;
                    }
                }
                let _ = sender.send(GenerationMessage::Cancelled(GenerationResult {
                    request_id: request.request_id.clone(),
                    generation_id: generation_id_for_worker.clone(),
                    text: generated,
                    statistics,
                }));
                bus.publish(&RuntimeEvent::GenerationCancelled(
                    request.request_id.clone(),
                ));
                return;
            }
            let statistics = match result {
                Ok(statistics) => statistics,
                Err(error) => {
                    let message = error.to_string();
                    if let Ok(mut active) = active_generation.lock() {
                        if active
                            .as_ref()
                            .is_some_and(|(_, _, current)| Arc::ptr_eq(current, &cancel_for_worker))
                        {
                            *active = None;
                        }
                    }
                    let _ = sender.send(GenerationMessage::Failed(error));
                    bus.publish(&RuntimeEvent::GenerationFailed(message));
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
                metrics_known: true,
                termination: statistics.termination,
            };
            let result = GenerationResult {
                request_id: request.request_id.clone(),
                generation_id: generation_id_for_worker,
                text: generated,
                statistics,
            };
            if let Ok(mut active) = active_generation.lock() {
                if active
                    .as_ref()
                    .is_some_and(|(_, _, current)| Arc::ptr_eq(current, &cancel_for_worker))
                {
                    *active = None;
                }
            }
            let _ = sender.send(GenerationMessage::Completed(result));
            bus.publish(&RuntimeEvent::GenerationCompleted(request.request_id));
        });
        Ok(GenerationStream::new(receiver, cancellation))
    }

    fn cancel_generation(&self) {
        if let Ok(active) = self.active_generation.lock() {
            if let Some((_, _, cancellation)) = active.as_ref() {
                cancellation.store(true, Ordering::SeqCst);
            }
        }
    }

    fn cancel_generation_for(&self, request_id: &str) {
        if let Ok(active) = self.active_generation.lock() {
            if let Some((active_id, _, cancellation)) = active.as_ref() {
                if active_id == request_id {
                    cancellation.store(true, Ordering::SeqCst);
                }
            }
        }
    }

    fn active_generation_id(&self, request_id: &str) -> Option<String> {
        self.active_generation.lock().ok().and_then(|active| {
            active.as_ref().and_then(|(active_id, generation_id, _)| {
                (active_id == request_id).then(|| generation_id.clone())
            })
        })
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

/// Construct Talvik's production runtime from validated configuration.
///
/// # Errors
///
/// Returns an invalid-configuration error when required configuration values
/// are empty.
pub fn start(config: RuntimeConfig, bus: EventBus) -> Result<MarinaRuntime, RuntimeError> {
    config
        .validate()
        .map_err(RuntimeError::InvalidConfiguration)?;
    Ok(MarinaRuntime::start(config, bus))
}

#[cfg(test)]
mod tests {
    use super::{admit_memory, MarinaRuntime};
    use crate::{
        LlamaCppAdapter, MockBackend, ModelMetadata, ModelRegistry, ModelStatus, RuntimeService,
    };
    use oid_shared::{EventBus, RuntimeConfig, RuntimeEvent};

    #[test]
    fn marina_runtime_is_deterministic_and_publishes_events() {
        let bus = EventBus::new();
        let receiver = bus.subscribe();
        let isolated_models = std::env::temp_dir().join(format!(
            "oid-runtime-empty-models-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let config = RuntimeConfig {
            model_directory: Some(isolated_models),
            ..RuntimeConfig::default()
        };
        let runtime = MarinaRuntime::start(config, bus);
        let expected_backend = if LlamaCppAdapter::native_available() {
            "llama.cpp"
        } else {
            "None"
        };
        assert_eq!(runtime.snapshot().backend, expected_backend);
        assert_eq!(runtime.snapshot().models, 0);
        let _ = runtime.execute_command("status");
        assert!(receiver
            .try_iter()
            .any(|event| matches!(event, RuntimeEvent::RuntimeStarted)));
    }

    #[test]
    fn startup_clears_persisted_transient_model_statuses() {
        let root = std::env::temp_dir().join(format!(
            "oid-runtime-status-reset-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create root");
        let registry_path = root.join("registry.db");
        let registry = ModelRegistry::open(&registry_path, EventBus::new()).expect("open registry");
        registry
            .register(ModelMetadata {
                id: "stale-model".to_owned(),
                name: "stale-model.gguf".to_owned(),
                family: "unknown".to_owned(),
                backend: "llama.cpp".to_owned(),
                quantization: "unknown".to_owned(),
                context_window: 0,
                memory_requirement_mb: 1,
                capabilities: vec!["text".to_owned()],
                status: ModelStatus::Loaded,
                checksum: None,
                location: root.join("stale-model.gguf").display().to_string(),
            })
            .expect("register stale model");
        drop(registry);

        let runtime = MarinaRuntime::start(
            RuntimeConfig {
                model_directory: Some(root.join("models")),
                registry_path: Some(registry_path),
                ..RuntimeConfig::default()
            },
            EventBus::new(),
        );

        assert_eq!(runtime.model_list()[0].status, ModelStatus::Registered);
        std::fs::remove_dir_all(root).expect("remove test data");
    }

    #[test]
    fn model_pull_imports_local_artifact_into_configured_store() {
        let root = std::env::temp_dir().join(format!(
            "oid-model-pull-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let source = root.join("source.gguf");
        let destination = root.join("models");
        let registry = root.join("registry.db");
        std::fs::create_dir_all(&root).expect("create root");
        std::fs::write(&source, b"GGUF").expect("write source");
        let config = RuntimeConfig {
            model_directory: Some(destination.clone()),
            registry_path: Some(registry),
            ..RuntimeConfig::default()
        };
        let runtime = MarinaRuntime::start(config, EventBus::new());
        let model = runtime
            .model_pull(
                source.to_str().expect("source path"),
                Some("local-demo"),
                None,
            )
            .expect("pull local model");
        assert_eq!(model.id, "local-demo");
        assert!(destination.join("source.gguf").is_file());
        assert_eq!(runtime.model_list().len(), 1);
        std::fs::remove_dir_all(root).expect("remove test data");
    }

    #[test]
    fn memory_admission_rejects_unknown_and_insufficient_capacity() {
        assert!(admit_memory(None, 0, 1).is_err());
        assert!(admit_memory(Some(512 * 1_048_576), 256 * 1_048_576, 32_768).is_err());
        assert!(admit_memory(Some(2 * 1_073_741_824), 256 * 1_048_576, 4_096).is_ok());
    }

    #[test]
    fn multiple_resident_models_are_independently_loadable_and_unloadable() {
        let root = std::env::temp_dir().join(format!(
            "oid-runtime-multi-model-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create root");
        let bus = EventBus::new();
        let runtime = MarinaRuntime::start_with_backend(
            RuntimeConfig {
                model_directory: Some(root.join("models")),
                registry_path: Some(root.join("registry.db")),
                ..RuntimeConfig::default()
            },
            bus,
            Box::new(MockBackend),
        );
        for id in ["alpha", "beta"] {
            runtime
                .models
                .register(ModelMetadata {
                    id: id.to_owned(),
                    name: format!("{id}.gguf"),
                    family: "test".to_owned(),
                    backend: "mock".to_owned(),
                    quantization: "test".to_owned(),
                    context_window: 4096,
                    memory_requirement_mb: 1,
                    capabilities: vec!["chat".to_owned()],
                    status: ModelStatus::Registered,
                    checksum: None,
                    location: root.join(format!("{id}.gguf")).display().to_string(),
                })
                .expect("register model");
            runtime.model_load(id).expect("load model");
        }
        assert_eq!(
            runtime.snapshot().loaded_models,
            vec!["alpha".to_owned(), "beta".to_owned()]
        );
        runtime.model_unload("alpha").expect("unload alpha");
        assert_eq!(runtime.snapshot().loaded_models, vec!["beta".to_owned()]);
        runtime.model_unload("beta").expect("unload beta");
        assert!(runtime.snapshot().loaded_models.is_empty());
        std::fs::remove_dir_all(root).expect("remove test data");
    }
}
