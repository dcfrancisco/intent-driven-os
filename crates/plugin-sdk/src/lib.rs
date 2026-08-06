//! Plugin extension contracts for Open Intelligence Desktop.
//!
//! This crate defines future-facing boundaries without loading or executing plugins yet.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::{OidError, PluginId, SkillId};

/// Metadata exposed by an installed plugin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginDescriptor {
    /// Stable plugin identity.
    pub id: PluginId,
    /// Plugin version supplied by its manifest.
    pub version: String,
    /// Capabilities declared by the plugin.
    pub skills: Vec<SkillId>,
}

/// Plugin lifecycle boundary.
pub trait Plugin: Send + Sync {
    /// Return plugin metadata without executing plugin code.
    fn descriptor(&self) -> &PluginDescriptor;

    /// Initialize the plugin within a host-provided capability boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when initialization fails.
    fn initialize(&mut self) -> Result<(), OidError>;
}

/// Discovery boundary for plugin manifests.
pub trait PluginDiscovery: Send + Sync {
    /// Discover installed plugin descriptors.
    ///
    /// # Errors
    ///
    /// Returns an error when discovery cannot inspect plugin metadata.
    fn discover(&self) -> Result<Vec<PluginDescriptor>, OidError>;
}

/// Identifies the plugin boundary.
///
/// ```
/// assert_eq!(oid_plugin_sdk::boundary_name(), "plugin-sdk");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "plugin-sdk"
}
