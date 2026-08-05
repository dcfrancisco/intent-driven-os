//! Runtime configuration boundary.

use oid_shared::RuntimeConfig;

/// Load the deterministic Phase 1 configuration.
///
/// # Errors
///
/// Returns a validation message if the foundation configuration is invalid.
pub fn load() -> Result<RuntimeConfig, String> {
    let config = RuntimeConfig::default();
    config.validate().map(|()| config)
}
