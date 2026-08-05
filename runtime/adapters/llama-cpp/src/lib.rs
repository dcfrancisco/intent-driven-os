//! Contract-only llama.cpp adapter skeleton.
//!
//! This crate deliberately contains no llama.cpp dependency, FFI, GGUF parser,
//! tokenizer, tensor operation, or token generation implementation.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_runtime::backends::{
    Backend, BackendDescriptor, BackendHealth, EmbeddingsRequest, EmbeddingsResult,
    GenerationRequest, StreamChunk, ToolSupport,
};
use oid_shared::RuntimeError;

/// Placeholder adapter proving that llama.cpp can implement the common backend contract.
#[derive(Clone, Debug, Default)]
pub struct LlamaCppAdapter;

impl Backend for LlamaCppAdapter {
    fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            id: "llama.cpp".to_owned(),
            name: "llama.cpp".to_owned(),
            version: "0.1".to_owned(),
        }
    }

    fn initialize(&self) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn shutdown(&self) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn health(&self) -> BackendHealth {
        BackendHealth::Unavailable
    }

    fn list_models(&self) -> Result<Vec<String>, RuntimeError> {
        Ok(Vec::new())
    }

    fn load_model(&self, _model_id: &str) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn unload_model(&self, _model_id: &str) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn generate_stream(
        &self,
        request: &GenerationRequest,
    ) -> Result<Vec<StreamChunk>, RuntimeError> {
        Ok(vec![StreamChunk {
            request_id: request.request_id.clone(),
            text: "llama.cpp adapter skeleton: inference is not implemented.".to_owned(),
            done: true,
        }])
    }

    fn cancel_generation(&self, _request_id: &str) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn embeddings(&self, _request: &EmbeddingsRequest) -> Result<EmbeddingsResult, RuntimeError> {
        Ok(EmbeddingsResult { values: Vec::new() })
    }

    fn tool_support(&self) -> ToolSupport {
        ToolSupport {
            supported: false,
            reason: "llama.cpp adapter skeleton has no tool implementation".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LlamaCppAdapter;
    use oid_runtime::backends::Backend;

    #[test]
    fn exposes_the_common_backend_contract_without_inference() {
        let adapter = LlamaCppAdapter;
        assert_eq!(adapter.descriptor().id, "llama.cpp");
        assert!(adapter.list_models().expect("skeleton list").is_empty());
        assert!(!adapter.tool_support().supported);
    }
}
