//! Public runtime API boundary.

use crate::{
    backends::{BackendHealth, BackendSummary},
    hardware::HardwareSnapshot,
    models::ModelMetadata,
    RuntimeStatus,
};
use oid_shared::{EventBus, RuntimeEvent};

/// Deterministic service data used by clients such as the Phase 2 console.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    /// Seconds since the mock/runtime service started.
    pub uptime_seconds: u64,
    /// Backend-reported library information.
    pub backend_version: Option<String>,
    /// Memory used by the loaded model.
    pub model_memory_bytes: Option<u64>,
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
    /// Load a registered model through the active backend.
    ///
    /// # Errors
    ///
    /// Returns an error when the model is unknown or the backend rejects it.
    fn model_load(&self, id: &str) -> Result<(), oid_shared::RuntimeError>;
    /// Unload the active model.
    ///
    /// # Errors
    ///
    /// Returns an error when the model is not loaded or the backend rejects it.
    fn model_unload(&self, id: &str) -> Result<(), oid_shared::RuntimeError>;
    /// Count tokens using the active backend tokenizer.
    ///
    /// # Errors
    ///
    /// Returns an error when no tokenizer is available or tokenization fails.
    fn tokenize(&self, text: &str) -> Result<usize, oid_shared::RuntimeError>;
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
