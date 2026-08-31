//! Compatibility bridge from the legacy runtime backend API to `ModelRunner`.

use crate::backends::{Backend, BackendDescriptor, BackendHealth, GenerationRequest, LoadedModel};
use crate::generation::GenerationStatistics;
use crate::models::ModelMetadata;
use oid_llama_cpp_adapter::LlamaCppModelRunner;
use oid_model_runner::{
    CancellationToken, GenerationEvent, LoadModelRequest, ModelId, ModelPath, ModelRunner,
};
use oid_shared::RuntimeError;
use std::collections::{hash_map::Entry, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Runtime compatibility adapter backed by OID's real model runner contract.
#[derive(Debug, Clone)]
pub struct LlamaCppAdapter {
    runner: Arc<LlamaCppModelRunner>,
    cancellations: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl Default for LlamaCppAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl LlamaCppAdapter {
    /// Construct an adapter with one shared model runner.
    #[must_use]
    pub fn new() -> Self {
        Self {
            runner: Arc::new(LlamaCppModelRunner::new()),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Whether this build has a native llama.cpp library configured.
    #[must_use]
    pub const fn native_available() -> bool {
        LlamaCppModelRunner::native_available()
    }

    fn forget_cancellation(&self, request_id: &str) {
        if let Ok(mut values) = self.cancellations.lock() {
            values.remove(request_id);
        }
    }
}

impl Backend for LlamaCppAdapter {
    fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            id: "llama.cpp".to_owned(),
            name: "llama.cpp".to_owned(),
            version: "oid-model-runner-v1".to_owned(),
            library_version: None,
        }
    }

    fn initialize(&self) -> Result<(), RuntimeError> {
        if Self::native_available() {
            Ok(())
        } else {
            Err(RuntimeError::NativeBackendUnavailable(
                "configure LLAMA_CPP_LIB_DIR and rebuild to enable llama.cpp".to_owned(),
            ))
        }
    }

    fn shutdown(&self) -> Result<(), RuntimeError> {
        match self.runner.unload() {
            Ok(()) | Err(oid_model_runner::ModelRunnerError::ModelNotLoaded) => Ok(()),
            Err(error) => Err(RuntimeError::NativeBackend(error.to_string())),
        }
    }

    fn health(&self) -> BackendHealth {
        if Self::native_available() {
            BackendHealth::Healthy
        } else {
            BackendHealth::Unavailable
        }
    }
    fn list_models(&self) -> Result<Vec<String>, RuntimeError> {
        Ok(Vec::new())
    }

    fn load_model(&self, model: &ModelMetadata) -> Result<LoadedModel, RuntimeError> {
        let model_id = ModelId::new(model.id.clone())
            .map_err(|error| RuntimeError::InvalidModel(error.to_string()))?;
        let path = ModelPath::new(model.location.clone())
            .map_err(|error| RuntimeError::InvalidModel(error.to_string()))?;
        let loaded = self
            .runner
            .load(LoadModelRequest { model_id, path })
            .map_err(|error| RuntimeError::NativeBackend(error.to_string()))?;
        Ok(LoadedModel {
            model_id: loaded.model_id.to_string(),
            memory_bytes: loaded.memory_bytes.unwrap_or_default(),
        })
    }

    fn unload_model(&self, _model_id: &str) -> Result<(), RuntimeError> {
        self.runner
            .unload()
            .map_err(|error| RuntimeError::NativeBackend(error.to_string()))
    }

    fn generate_stream(
        &self,
        request: &GenerationRequest,
    ) -> Result<Vec<crate::backends::StreamChunk>, RuntimeError> {
        let mut chunks = Vec::new();
        let cancellation = AtomicBool::new(false);
        let statistics = self.generate_streaming(
            request,
            &mut |text| {
                chunks.push(crate::backends::StreamChunk {
                    request_id: request.request_id.clone(),
                    text: text.to_owned(),
                    done: false,
                });
                true
            },
            &cancellation,
        )?;
        chunks.push(crate::backends::StreamChunk {
            request_id: request.request_id.clone(),
            text: String::new(),
            done: true,
        });
        let _ = statistics;
        Ok(chunks)
    }

    fn generate_streaming(
        &self,
        request: &GenerationRequest,
        callback: &mut dyn FnMut(&str) -> bool,
        cancellation: &AtomicBool,
    ) -> Result<GenerationStatistics, RuntimeError> {
        let stream_request = oid_model_runner::GenerationRequest {
            prompt: request.input.clone(),
            max_tokens: request.options.max_tokens,
            temperature: request.options.temperature,
            top_p: request.options.top_p,
            top_k: request.options.top_k,
            context_size: request.options.context_size,
            seed: Some(request.options.seed),
        };
        let stream = self
            .runner
            .generate(stream_request)
            .map_err(|error| RuntimeError::NativeBackend(error.to_string()))?;
        let request_cancellation = stream.cancellation_token();
        let Ok(mut cancellations) = self.cancellations.lock() else {
            stream.cancel();
            return Err(RuntimeError::ModelLifecycle(
                "cancellation registry poisoned".to_owned(),
            ));
        };
        match cancellations.entry(request.request_id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(request_cancellation);
            }
            Entry::Occupied(_) => {
                stream.cancel();
                return Err(RuntimeError::ModelLifecycle(
                    "duplicate active generation request id".to_owned(),
                ));
            }
        }
        let started = Instant::now();
        loop {
            if cancellation.load(Ordering::SeqCst) {
                stream.cancel();
            }
            match stream.recv_timeout(Duration::from_millis(25)) {
                Ok(GenerationEvent::Token(token)) => {
                    if !callback(&token) {
                        stream.cancel();
                    }
                }
                Ok(GenerationEvent::Completed(stats)) => {
                    self.forget_cancellation(&request.request_id);
                    let prompt_tokens = stats.prompt_tokens.unwrap_or_default();
                    return Ok(GenerationStatistics {
                        prompt_tokens,
                        generated_tokens: stats.generated_tokens,
                        latency_ms: started.elapsed().as_millis(),
                        inference_time_ms: stats.generation_time.unwrap_or_default().as_millis(),
                        context_tokens: prompt_tokens + stats.generated_tokens,
                        ..GenerationStatistics::default()
                    });
                }
                Ok(GenerationEvent::Cancelled(_)) => {
                    self.forget_cancellation(&request.request_id);
                    return Err(RuntimeError::ModelLifecycle(
                        "generation cancelled".to_owned(),
                    ));
                }
                Ok(GenerationEvent::Error(error)) => {
                    self.forget_cancellation(&request.request_id);
                    return Err(RuntimeError::NativeBackend(error.to_string()));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    self.forget_cancellation(&request.request_id);
                    return Err(RuntimeError::ModelLifecycle(
                        "generation stream closed".to_owned(),
                    ));
                }
            }
        }
    }

    fn cancel_generation(&self, request_id: &str) -> Result<(), RuntimeError> {
        if let Some(token) = self
            .cancellations
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("cancellation registry poisoned".to_owned()))?
            .get(request_id)
        {
            token.cancel();
        }
        Ok(())
    }
    fn embeddings(
        &self,
        _request: &crate::backends::EmbeddingsRequest,
    ) -> Result<crate::backends::EmbeddingsResult, RuntimeError> {
        Err(RuntimeError::NotImplemented("embeddings"))
    }
    fn tool_support(&self) -> crate::backends::ToolSupport {
        crate::backends::ToolSupport {
            supported: false,
            reason: "not implemented".to_owned(),
        }
    }
    fn tokenize(&self, text: &str) -> Result<usize, RuntimeError> {
        self.runner
            .tokenize(text)
            .and_then(|count| {
                usize::try_from(count).map_err(|_| {
                    oid_model_runner::ModelRunnerError::ResourceExhausted(
                        "token count exceeds platform limit".to_owned(),
                    )
                })
            })
            .map_err(|error| RuntimeError::NativeBackend(error.to_string()))
    }
}
