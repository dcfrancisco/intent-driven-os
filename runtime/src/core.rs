//! Runtime orchestration core.

use crate::{
    api::RuntimeApi,
    backends::BackendManager,
    hardware::{HardwareProvider, HardwareSnapshot, UnavailableHardware},
    lifecycle::Lifecycle,
    models::{EmptyModelCatalog, ModelCatalog},
    RuntimeConfig, RuntimeError, RuntimeStatus,
};
use oid_shared::{LifecycleState, RuntimeEvent};
use std::fmt;

/// Phase 1 Intelligent Runtime orchestrator.
pub struct Runtime {
    config: RuntimeConfig,
    lifecycle: Lifecycle,
    hardware: Box<dyn HardwareProvider>,
    #[allow(dead_code)]
    backends: BackendManager,
    #[allow(dead_code)]
    models: Box<dyn ModelCatalog>,
    events: Vec<RuntimeEvent>,
}

impl fmt::Debug for Runtime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Runtime")
            .field("config", &self.config)
            .field("lifecycle", &self.lifecycle)
            .field("hardware", &self.hardware.snapshot())
            .field("backends", &self.backends)
            .field("models", &"ModelRegistry")
            .field("events", &self.events)
            .finish()
    }
}

impl Runtime {
    /// Start the runtime from validated configuration and foundation providers.
    ///
    /// # Errors
    ///
    /// Returns an error when configuration validation fails or the lifecycle
    /// cannot enter the ready state.
    pub fn start(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        config
            .validate()
            .map_err(RuntimeError::InvalidConfiguration)?;
        let mut lifecycle = Lifecycle::default();
        lifecycle.transition(LifecycleState::Ready)?;
        Ok(Self {
            config,
            lifecycle,
            hardware: Box::new(UnavailableHardware),
            backends: BackendManager::new(oid_shared::EventBus::new()),
            models: Box::new(EmptyModelCatalog),
            events: vec![
                RuntimeEvent::ConfigurationLoaded,
                RuntimeEvent::StateChanged(LifecycleState::Ready),
            ],
        })
    }

    /// Return the configured runtime name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Return the current hardware snapshot.
    #[must_use]
    pub fn hardware(&self) -> HardwareSnapshot {
        self.hardware.snapshot()
    }

    /// Return lifecycle events emitted so far.
    #[must_use]
    pub fn events(&self) -> &[RuntimeEvent] {
        &self.events
    }
}

impl RuntimeApi for Runtime {
    fn status(&self) -> RuntimeStatus {
        RuntimeStatus {
            state: self.lifecycle.state(),
            version: "0.1",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Runtime;
    use crate::api::RuntimeApi;
    use oid_shared::{LifecycleState, RuntimeConfig};

    #[test]
    fn starts_ready_without_backend_or_model_implementation() {
        let runtime = Runtime::start(RuntimeConfig::default()).expect("foundation starts");
        assert_eq!(runtime.status().state, LifecycleState::Ready);
        assert!(runtime.events().len() >= 2);
    }
}
