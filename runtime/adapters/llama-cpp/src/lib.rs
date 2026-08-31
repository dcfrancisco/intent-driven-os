//! llama.cpp implementation of OID's backend-neutral model runner contract.

#![warn(missing_docs)]

use oid_llama_cpp_sys as native;
use oid_model_runner::{
    CancellationToken, GenerationEvent, GenerationRequest, GenerationStats, GenerationStream,
    LoadModelRequest, LoadedModel, ModelId, ModelRunner, ModelRunnerError, ModelRuntimeStats,
    ModelState,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug)]
struct Loaded {
    id: ModelId,
    model: native::NativeModel,
}

/// Direct local llama.cpp model runner.
///
/// The native model handle is confined to this adapter. Callers only see OID
/// domain types and streamed generation events.
#[derive(Debug)]
pub struct LlamaCppModelRunner {
    initialized: AtomicBool,
    model: Arc<Mutex<Option<Loaded>>>,
}

impl Default for LlamaCppModelRunner {
    fn default() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            model: Arc::new(Mutex::new(None)),
        }
    }
}

impl LlamaCppModelRunner {
    /// Construct an uninitialized runner.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this build has a native llama.cpp library configured.
    #[must_use]
    pub const fn native_available() -> bool {
        native::is_available()
    }

    fn unavailable() -> ModelRunnerError {
        ModelRunnerError::BackendUnavailable(
            "configure LLAMA_CPP_LIB_DIR and rebuild to enable llama.cpp".to_owned(),
        )
    }
}

impl ModelRunner for LlamaCppModelRunner {
    fn load(&self, request: LoadModelRequest) -> Result<LoadedModel, ModelRunnerError> {
        if !native::is_available() {
            return Err(Self::unavailable());
        }
        if !std::path::Path::new(request.path.as_str()).is_file() {
            return Err(ModelRunnerError::ModelNotFound(
                request.path.as_str().to_owned(),
            ));
        }
        let mut slot = self
            .model
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("model lock poisoned".to_owned()))?;
        if slot.is_some() {
            return Err(ModelRunnerError::ModelBusy(
                "one active model is supported".to_owned(),
            ));
        }
        if !self.initialized.swap(true, Ordering::SeqCst) {
            native::initialize().map_err(|error| {
                self.initialized.store(false, Ordering::SeqCst);
                ModelRunnerError::BackendFailure(error)
            })?;
        }
        let started = Instant::now();
        let model =
            native::load_model(request.path.as_str()).map_err(ModelRunnerError::BackendFailure)?;
        let memory_bytes = model.memory_bytes();
        *slot = Some(Loaded {
            id: request.model_id.clone(),
            model,
        });
        Ok(LoadedModel {
            model_id: request.model_id,
            backend: "llama.cpp".to_owned(),
            format: "GGUF".to_owned(),
            load_time: Some(started.elapsed()),
            memory_bytes: Some(memory_bytes),
        })
    }

    #[allow(clippy::cast_precision_loss)]
    fn generate(&self, request: GenerationRequest) -> Result<GenerationStream, ModelRunnerError> {
        let model = self.model.clone();
        model
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("model lock poisoned".to_owned()))?
            .as_ref()
            .ok_or(ModelRunnerError::ModelNotLoaded)?;
        let cancellation = CancellationToken::default();
        let worker_cancellation = cancellation.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let started = Instant::now();
            let result = (|| -> Result<GenerationStats, String> {
                let guard = model.lock().map_err(|_| "model lock poisoned".to_owned())?;
                let loaded = guard.as_ref().ok_or_else(|| "model unloaded".to_owned())?;
                let first_token = Arc::new(Mutex::new(None));
                let first_token_for_callback = first_token.clone();
                let mut callback = |token: &str| {
                    if worker_cancellation.is_cancelled() {
                        return false;
                    }
                    if let Ok(mut first) = first_token_for_callback.lock() {
                        if first.is_none() {
                            *first = Some(Instant::now());
                        }
                    }
                    sender
                        .send(GenerationEvent::Token(token.to_owned()))
                        .is_ok()
                };
                let native_stats = loaded.model.generate_stream(
                    &request.prompt,
                    4096,
                    request.max_tokens,
                    request.temperature,
                    0.95,
                    40,
                    request.seed.unwrap_or(0),
                    &mut callback,
                    worker_cancellation.as_atomic(),
                )?;
                let elapsed = started.elapsed();
                let stats = GenerationStats {
                    prompt_tokens: Some(native_stats.prompt_tokens),
                    generated_tokens: native_stats.generated_tokens,
                    time_to_first_token: first_token
                        .lock()
                        .ok()
                        .and_then(|value| value.map(|time| time.duration_since(started))),
                    generation_time: Some(elapsed),
                    tokens_per_second: (elapsed.as_secs_f64() > 0.0)
                        .then_some(native_stats.generated_tokens as f64 / elapsed.as_secs_f64()),
                };
                Ok(stats)
            })();
            match result {
                Ok(stats) if worker_cancellation.is_cancelled() => {
                    let _ = sender.send(GenerationEvent::Cancelled(stats));
                }
                Ok(stats) => {
                    let _ = sender.send(GenerationEvent::Completed(stats));
                }
                Err(error)
                    if worker_cancellation.is_cancelled() || error == "generation cancelled" =>
                {
                    let _ = sender.send(GenerationEvent::Cancelled(GenerationStats {
                        generation_time: Some(started.elapsed()),
                        ..GenerationStats::default()
                    }));
                }
                Err(error) => {
                    let _ = sender.send(GenerationEvent::Error(ModelRunnerError::BackendFailure(
                        error,
                    )));
                }
            }
        });
        Ok(GenerationStream::new(receiver, cancellation))
    }

    fn stats(&self) -> Result<ModelRuntimeStats, ModelRunnerError> {
        let slot = self
            .model
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("model lock poisoned".to_owned()))?;
        Ok(slot.as_ref().map_or_else(
            || ModelRuntimeStats {
                state: ModelState::Unloaded,
                ..Default::default()
            },
            |loaded| ModelRuntimeStats {
                loaded_model: Some(loaded.id.clone()),
                memory_bytes: Some(loaded.model.memory_bytes()),
                state: ModelState::Ready,
            },
        ))
    }

    fn unload(&self) -> Result<(), ModelRunnerError> {
        let mut slot = self
            .model
            .lock()
            .map_err(|_| ModelRunnerError::UnloadFailure("model lock poisoned".to_owned()))?;
        if slot.take().is_none() {
            return Err(ModelRunnerError::ModelNotLoaded);
        }
        self.initialized.store(false, Ordering::SeqCst);
        native::shutdown();
        Ok(())
    }
}
