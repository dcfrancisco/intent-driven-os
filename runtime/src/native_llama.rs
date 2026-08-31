//! Compatibility bridge from the legacy runtime backend API to `ModelRunner`.

use crate::backends::{Backend, BackendDescriptor, BackendHealth, GenerationRequest, LoadedModel};
use crate::generation::GenerationStatistics;
use crate::models::ModelMetadata;
use oid_llama_cpp_adapter::LlamaCppModelRunner;
use oid_model_runner::{GenerationEvent, LoadModelRequest, ModelId, ModelPath, ModelRunner};
use oid_shared::RuntimeError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Runtime compatibility adapter backed by OID's real model runner contract.
#[derive(Debug, Clone)]
pub struct LlamaCppAdapter {
    runner: Arc<LlamaCppModelRunner>,
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
        }
    }

    /// Whether this build has a native llama.cpp library configured.
    #[must_use]
    pub const fn native_available() -> bool {
        LlamaCppModelRunner::native_available()
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
        _request: &GenerationRequest,
    ) -> Result<Vec<crate::backends::StreamChunk>, RuntimeError> {
        Err(RuntimeError::NotImplemented("buffered generation"))
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
            seed: Some(request.options.seed),
        };
        let stream = self
            .runner
            .generate(stream_request)
            .map_err(|error| RuntimeError::NativeBackend(error.to_string()))?;
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
                    return Err(RuntimeError::ModelLifecycle(
                        "generation cancelled".to_owned(),
                    ))
                }
                Ok(GenerationEvent::Error(error)) => {
                    return Err(RuntimeError::NativeBackend(error.to_string()))
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(RuntimeError::ModelLifecycle(
                        "generation stream closed".to_owned(),
                    ))
                }
            }
        }
    }

    fn cancel_generation(&self, _request_id: &str) -> Result<(), RuntimeError> {
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
    fn tokenize(&self, _text: &str) -> Result<usize, RuntimeError> {
        Err(RuntimeError::NotImplemented(
            "tokenization until a model is loaded",
        ))
    }
}
