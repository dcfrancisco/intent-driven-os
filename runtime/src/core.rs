//! Runtime orchestration core.

use crate::{service::MarinaRuntime, RuntimeConfig, RuntimeError};
use oid_shared::EventBus;

/// The single production composition-root namespace.
#[derive(Debug, Default)]
pub struct Runtime;

impl Runtime {
    /// Start Marina through the authoritative production composition root.
    ///
    /// # Errors
    ///
    /// Returns an error when configuration validation fails or the lifecycle
    /// cannot enter the ready state.
    pub fn start(config: RuntimeConfig) -> Result<MarinaRuntime, RuntimeError> {
        Self::start_with_bus(config, EventBus::new())
    }

    /// Start Marina with a caller-owned event bus for an in-process client.
    ///
    /// # Errors
    ///
    /// Returns an error when configuration validation fails.
    pub fn start_with_bus(
        config: RuntimeConfig,
        bus: EventBus,
    ) -> Result<MarinaRuntime, RuntimeError> {
        crate::service::start(config, bus)
    }
}

#[cfg(test)]
mod tests {
    use super::Runtime;
    use crate::RuntimeService;
    use oid_shared::{LifecycleState, RuntimeConfig};

    #[test]
    fn composition_root_starts_the_production_service() {
        let runtime = Runtime::start(RuntimeConfig::default()).expect("foundation starts");
        assert_eq!(
            RuntimeService::status(&runtime).state,
            LifecycleState::Ready
        );
        assert!(runtime.event_bus().subscribe().try_iter().next().is_none());
    }
}
