//! Runtime-owned llama.cpp adapter implementation.

use crate::backends::{Backend, BackendDescriptor, BackendHealth, LoadedModel};
use crate::models::ModelMetadata;
use oid_llama_cpp_sys as native;
use oid_shared::RuntimeError;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

/// Native llama.cpp backend adapter.
#[derive(Debug, Default)]
pub struct LlamaCppAdapter {
    initialized: AtomicBool,
    model: Mutex<Option<(String, native::NativeModel)>>,
}

impl LlamaCppAdapter {
    /// Construct an adapter that has not yet initialized llama.cpp.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this build can link the native engine.
    #[must_use]
    pub const fn native_available() -> bool {
        native::is_available()
    }

    fn unavailable() -> RuntimeError {
        RuntimeError::NativeBackendUnavailable(
            "configure LLAMA_CPP_LIB_DIR and rebuild to enable llama.cpp".to_owned(),
        )
    }
}

impl Backend for LlamaCppAdapter {
    fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            id: "llama.cpp".to_owned(),
            name: "llama.cpp".to_owned(),
            version: "backend-contract-v1".to_owned(),
            library_version: native::system_info(),
        }
    }

    fn initialize(&self) -> Result<(), RuntimeError> {
        if !native::is_available() {
            return Err(Self::unavailable());
        }
        if !self.initialized.swap(true, Ordering::SeqCst) {
            if let Err(error) = native::initialize() {
                self.initialized.store(false, Ordering::SeqCst);
                return Err(RuntimeError::NativeBackend(error));
            }
        }
        Ok(())
    }

    fn shutdown(&self) -> Result<(), RuntimeError> {
        if self
            .model
            .lock()
            .map_err(|_| RuntimeError::BackendManagerUnavailable)?
            .is_some()
        {
            return Err(RuntimeError::ModelLifecycle(
                "unload the active model first".to_owned(),
            ));
        }
        if self.initialized.swap(false, Ordering::SeqCst) {
            native::shutdown();
        }
        Ok(())
    }

    fn health(&self) -> BackendHealth {
        if self.initialized.load(Ordering::SeqCst) {
            BackendHealth::Healthy
        } else {
            BackendHealth::Unavailable
        }
    }

    fn list_models(&self) -> Result<Vec<String>, RuntimeError> {
        Ok(Vec::new())
    }

    fn load_model(&self, model: &ModelMetadata) -> Result<LoadedModel, RuntimeError> {
        if !self.initialized.load(Ordering::SeqCst) {
            return Err(Self::unavailable());
        }
        if !model.location.to_ascii_lowercase().ends_with(".gguf") {
            return Err(RuntimeError::InvalidModel(model.location.clone()));
        }
        let mut loaded = self
            .model
            .lock()
            .map_err(|_| RuntimeError::BackendManagerUnavailable)?;
        if loaded.is_some() {
            return Err(RuntimeError::ModelLifecycle(
                "only one model may be loaded".to_owned(),
            ));
        }
        let native_model =
            native::load_model(&model.location).map_err(RuntimeError::NativeBackend)?;
        let memory_bytes = native_model.memory_bytes();
        *loaded = Some((model.id.clone(), native_model));
        Ok(LoadedModel {
            model_id: model.id.clone(),
            memory_bytes,
        })
    }

    fn unload_model(&self, model_id: &str) -> Result<(), RuntimeError> {
        let mut loaded = self
            .model
            .lock()
            .map_err(|_| RuntimeError::BackendManagerUnavailable)?;
        match loaded.as_ref().map(|entry| entry.0.as_str()) {
            Some(id) if id == model_id => {
                loaded.take();
                Ok(())
            }
            Some(_) => Err(RuntimeError::ModelLifecycle(
                "requested model is not loaded".to_owned(),
            )),
            None => Err(RuntimeError::ModelLifecycle(
                "no model is loaded".to_owned(),
            )),
        }
    }

    fn generate_stream(
        &self,
        _request: &crate::backends::GenerationRequest,
    ) -> Result<Vec<crate::backends::StreamChunk>, RuntimeError> {
        Err(RuntimeError::NotImplemented("text generation"))
    }

    fn generate_streaming(
        &self,
        request: &crate::backends::GenerationRequest,
        callback: &mut dyn FnMut(&str) -> bool,
        cancellation: &std::sync::atomic::AtomicBool,
    ) -> Result<crate::generation::GenerationStatistics, RuntimeError> {
        let loaded = self
            .model
            .lock()
            .map_err(|_| RuntimeError::BackendManagerUnavailable)?;
        let (_, model) = loaded.as_ref().ok_or_else(|| {
            RuntimeError::ModelLifecycle("load a model before generating".to_owned())
        })?;
        let options = &request.options;
        let stats = model
            .generate_stream(
                &request.input,
                options.context_size,
                options.max_tokens,
                options.temperature,
                options.top_p,
                options.top_k,
                options.seed,
                callback,
                cancellation,
            )
            .map_err(RuntimeError::NativeBackend)?;
        Ok(crate::generation::GenerationStatistics {
            prompt_tokens: stats.prompt_tokens,
            generated_tokens: stats.generated_tokens,
            context_tokens: stats.context_tokens,
            ..crate::generation::GenerationStatistics::default()
        })
    }

    fn cancel_generation(&self, _request_id: &str) -> Result<(), RuntimeError> {
        Err(RuntimeError::NotImplemented("generation cancellation"))
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
            reason: "not implemented in Phase 4".to_owned(),
        }
    }

    fn tokenize(&self, text: &str) -> Result<usize, RuntimeError> {
        let loaded = self
            .model
            .lock()
            .map_err(|_| RuntimeError::BackendManagerUnavailable)?;
        let (_, model) = loaded.as_ref().ok_or_else(|| {
            RuntimeError::ModelLifecycle("load a model before tokenizing".to_owned())
        })?;
        model
            .tokenize(text)
            .map(|tokens| tokens.len())
            .map_err(RuntimeError::Tokenization)
    }
}
