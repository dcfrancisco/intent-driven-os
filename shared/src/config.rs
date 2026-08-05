//! Shared configuration values.

/// Minimal Phase 1 runtime configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    /// Display name used by clients.
    pub name: String,
    /// Public runtime version.
    pub version: String,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            name: "Intelligent Runtime".to_owned(),
            version: "v0.1".to_owned(),
        }
    }
}

impl RuntimeConfig {
    /// Validate the configuration without reading the filesystem or environment.
    ///
    /// # Errors
    ///
    /// Returns a message when a required value is empty.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("name must not be empty".to_owned());
        }
        if self.version.trim().is_empty() {
            return Err("version must not be empty".to_owned());
        }
        Ok(())
    }
}
