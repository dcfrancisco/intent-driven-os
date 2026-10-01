//! Runtime configuration boundary.

use oid_shared::RuntimeConfig;

/// Load the deterministic Phase 1 configuration.
///
/// # Errors
///
/// Returns a validation message if the foundation configuration is invalid.
pub fn load() -> Result<RuntimeConfig, String> {
    let mut config = RuntimeConfig::default();
    let state = std::env::var_os("MARINA_STATE_DIR").map_or_else(
        || {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.join(".marina"))
        },
        |path| Some(std::path::PathBuf::from(path)),
    );
    config.model_directory = std::env::var_os("MARINA_MODEL_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| state.clone().map(|path| path.join("models")));
    config.registry_path = std::env::var_os("MARINA_REGISTRY")
        .map(std::path::PathBuf::from)
        .or_else(|| state.map(|path| path.join("models.registry")));
    config.validate().map(|()| config)
}
