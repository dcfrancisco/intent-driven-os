//! Public runtime API boundary.

use crate::generation::{GenerationRequest, GenerationStream};
use crate::{
    backends::{BackendHealth, BackendSummary, EmbeddingsRequest, EmbeddingsResult, ToolSupport},
    hardware::HardwareSnapshot,
    models::ModelMetadata,
    RuntimeStatus,
};
use oid_shared::{EventBus, RuntimeEvent};

/// Deterministic service data used by clients such as the Phase 2 console.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeSnapshot {
    /// Runtime health label.
    pub health: &'static str,
    /// Active backend label.
    pub backend: String,
    /// Number of installed models.
    pub models: usize,
    /// Display value for available memory.
    pub memory: String,
    /// Currently loaded model, if any.
    pub loaded_model: Option<String>,
    /// All resident models in stable identifier order.
    pub loaded_models: Vec<String>,
    /// Resident model details reported by the runtime-owned scheduler.
    pub resident_models: Vec<ResidentModelSnapshot>,
    /// Number of requests waiting for the single active generation slot.
    pub pending_requests: usize,
    /// Number of active generations.
    pub active_requests: usize,
    /// Configured maximum FIFO queue capacity.
    pub queue_capacity: usize,
    /// Whether bounded automatic idle-model eviction is enabled.
    pub automatic_eviction: bool,
    /// Configured idle retention in seconds.
    pub idle_retention_seconds: u64,
    /// Seconds since the mock/runtime service started.
    pub uptime_seconds: u64,
    /// Backend-reported library information.
    pub backend_version: Option<String>,
    /// Memory used by the loaded model.
    pub model_memory_bytes: Option<u64>,
    /// Whether a generation is currently active.
    pub generating: bool,
    /// Current generation throughput.
    pub current_tokens_per_second: Option<f64>,
    /// Current context usage.
    pub loaded_context: Option<u64>,
}

/// Truthful scheduler-owned information about one resident model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentModelSnapshot {
    /// Registered model identifier.
    pub id: String,
    /// Backend-reported or registry-estimated resident bytes, when measurable.
    pub memory_bytes: Option<u64>,
    /// Seconds since the runtime last used this resident model.
    pub idle_seconds: u64,
}

/// Runtime-facing service contract used by clients.
pub trait RuntimeService: Send + Sync {
    /// Return the current runtime lifecycle status.
    fn status(&self) -> RuntimeStatus;
    /// Return deterministic service information.
    fn snapshot(&self) -> RuntimeSnapshot;
    /// List registered backend descriptors.
    fn backend_list(&self) -> Vec<BackendSummary>;
    /// Return backend health by identifier.
    fn backend_health(&self, id: &str) -> Option<BackendHealth>;
    /// List registered model metadata.
    fn model_list(&self) -> Vec<ModelMetadata>;
    /// Inspect model metadata by identifier.
    fn model_inspect(&self, id: &str) -> Option<ModelMetadata>;
    /// Pull or import a local/remote GGUF artifact and register it.
    ///
    /// # Errors
    ///
    /// Returns an error when acquisition, validation, checksum verification,
    /// or registry persistence fails.
    fn model_pull(
        &self,
        source: &str,
        model_id: Option<&str>,
        checksum: Option<&str>,
    ) -> Result<ModelMetadata, oid_shared::RuntimeError>;
    /// Load a registered model through the active backend.
    ///
    /// # Errors
    ///
    /// Returns an error when the model is unknown or the backend rejects it.
    fn model_load(&self, id: &str) -> Result<(), oid_shared::RuntimeError>;
    /// Pair a registered text model with a local GGUF vision projector.
    ///
    /// Pass `None` to remove an existing projector pairing. A resident model
    /// must be unloaded before its pairing can be changed.
    ///
    /// # Errors
    ///
    /// Returns an error when the model or projector is invalid, the model is
    /// resident, or registry persistence fails.
    fn model_set_vision_projector(
        &self,
        id: &str,
        projector_path: Option<&str>,
    ) -> Result<(), oid_shared::RuntimeError> {
        let _ = (id, projector_path);
        Err(oid_shared::RuntimeError::NotImplemented(
            "vision projector configuration",
        ))
    }
    /// Unload the active model.
    ///
    /// # Errors
    ///
    /// Returns an error when the model is not loaded or the backend rejects it.
    fn model_unload(&self, id: &str) -> Result<(), oid_shared::RuntimeError>;
    /// Drop any retained prompt/KV state for a resident model.
    ///
    /// # Errors
    ///
    /// Returns an error when the model is not resident or cannot be reset
    /// safely.
    fn reset_context(&self, _id: &str) -> Result<(), oid_shared::RuntimeError> {
        Err(oid_shared::RuntimeError::NotImplemented(
            "inference context reset",
        ))
    }
    /// Count tokens using the active backend tokenizer.
    ///
    /// # Errors
    ///
    /// Returns an error when no tokenizer is available or tokenization fails.
    fn tokenize(&self, text: &str) -> Result<usize, oid_shared::RuntimeError>;
    /// Compute an embedding vector through the active backend.
    ///
    /// # Errors
    ///
    /// Returns an authentication, admission, or backend capability error when
    /// the active runtime cannot compute embeddings.
    fn embeddings(
        &self,
        request: &EmbeddingsRequest,
    ) -> Result<EmbeddingsResult, oid_shared::RuntimeError> {
        let _ = request;
        Err(oid_shared::RuntimeError::NotImplemented("embeddings"))
    }
    /// Report whether the active backend can execute structured tool calls.
    fn tool_support(&self, _model_id: &str) -> ToolSupport {
        ToolSupport {
            supported: false,
            reason: "tool calls are not supported by the active backend".to_owned(),
        }
    }
    /// Start an independent non-blocking generation.
    ///
    /// # Errors
    ///
    /// Returns an error when no model is loaded or generation cannot be admitted.
    fn generate(
        &self,
        request: GenerationRequest,
    ) -> Result<GenerationStream, oid_shared::RuntimeError>;
    /// Cancel the active generation, if one exists.
    fn cancel_generation(&self);
    /// Cancel a specific generation by its request identifier.
    fn cancel_generation_for(&self, request_id: &str) {
        let _ = request_id;
        self.cancel_generation();
    }
    /// Return the independent Marina generation ID for an active request.
    fn active_generation_id(&self, _request_id: &str) -> Option<String> {
        None
    }
    /// Return normalized hardware information.
    fn hardware(&self) -> HardwareSnapshot;
    /// Return an event publisher handle.
    fn event_bus(&self) -> EventBus;
    /// Publish a command lifecycle sequence and return mock output.
    fn execute_command(&self, command: &str) -> String;
    /// Publish that the user is editing input.
    fn input_started(&self);
    /// Publish that the user stopped editing input.
    fn input_stopped(&self);
    /// Publish a shutdown event.
    fn stop(&self);
    /// Publish a single event for a client-side lifecycle action.
    fn publish(&self, event: RuntimeEvent) {
        self.event_bus().publish(&event);
    }
}

/// Read-only service API exposed by the runtime orchestrator.
pub trait RuntimeApi {
    /// Return the current service status.
    fn status(&self) -> RuntimeStatus;
}
