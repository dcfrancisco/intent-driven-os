//! Plugin extension contracts for Open Intelligence Desktop.
//!
//! This crate defines future-facing boundaries without loading or executing plugins yet.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::{OidError, PluginId, SkillId};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

/// Describes one command exposed by a loaded capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityCommand {
    /// Command name typed at the beginning of a prompt.
    pub name: String,
    /// Human-readable command description.
    pub description: String,
    /// Additional names accepted by the router.
    pub aliases: Vec<String>,
    /// Skill that owns governed execution for this command.
    pub skill_id: SkillId,
    /// Whether the command may change system state.
    pub mutates_system: bool,
    /// Completion candidates for the command's arguments.
    pub completion: Vec<String>,
}

impl CapabilityCommand {
    /// Validate command metadata before registration.
    ///
    /// # Errors
    ///
    /// Returns an error when the command name, aliases, or description is invalid.
    pub fn validate(&self) -> Result<(), OidError> {
        if self.name.trim().is_empty() || self.name.chars().any(char::is_whitespace) {
            return Err(OidError::InvalidInput("capability command name".to_owned()));
        }
        if self.description.trim().is_empty() {
            return Err(OidError::InvalidInput(
                "capability command description".to_owned(),
            ));
        }
        if self
            .aliases
            .iter()
            .any(|alias| alias.trim().is_empty() || alias.chars().any(char::is_whitespace))
        {
            return Err(OidError::InvalidInput(
                "capability command alias".to_owned(),
            ));
        }
        Ok(())
    }

    /// Return whether this command or one of its aliases matches a name.
    #[must_use]
    pub fn matches(&self, name: &str) -> bool {
        self.name == name || self.aliases.iter().any(|alias| alias == name)
    }
}

/// A plugin capability and the commands it contributes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityDescriptor {
    /// Stable capability identity.
    pub id: SkillId,
    /// Plugin that provided the capability.
    pub plugin_id: PluginId,
    /// Commands made available while the capability is loaded.
    pub commands: Vec<CapabilityCommand>,
}

/// Snapshot returned by registry discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredCapability {
    /// Capability metadata.
    pub descriptor: CapabilityDescriptor,
}

/// Thread-safe runtime registry for allowlisted dynamic capabilities.
#[derive(Clone, Debug, Default)]
pub struct CapabilityRegistry {
    capabilities: Arc<RwLock<BTreeMap<String, RegisteredCapability>>>,
    native_commands: Arc<RwLock<BTreeSet<String>>>,
}

impl CapabilityRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserve native command names that dynamic capabilities may not shadow.
    ///
    /// # Errors
    ///
    /// Returns an error when a native command name is empty or contains whitespace.
    pub fn reserve_native<I, S>(&self, commands: I) -> Result<(), OidError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut native = self
            .native_commands
            .write()
            .map_err(|_| OidError::Execution("native command registry poisoned".to_owned()))?;
        for command in commands {
            let command = command.into();
            if command.trim().is_empty() || command.chars().any(char::is_whitespace) {
                return Err(OidError::InvalidInput("native command name".to_owned()));
            }
            native.insert(command);
        }
        Ok(())
    }

    /// Register a capability after validating its metadata and command allowlist.
    ///
    /// # Errors
    ///
    /// Returns an error when metadata is invalid, a command collides with a
    /// native or already registered command, or the capability is empty.
    pub fn register(&self, descriptor: CapabilityDescriptor) -> Result<(), OidError> {
        if descriptor.commands.is_empty() {
            return Err(OidError::InvalidInput(
                "capability must expose a command".to_owned(),
            ));
        }
        let native = self
            .native_commands
            .read()
            .map_err(|_| OidError::Execution("native command registry poisoned".to_owned()))?;
        let mut names = BTreeSet::new();
        for command in &descriptor.commands {
            command.validate()?;
            if native.contains(&command.name)
                || command.aliases.iter().any(|alias| native.contains(alias))
            {
                return Err(OidError::Unauthorized(format!(
                    "dynamic command shadows native command: {}",
                    command.name
                )));
            }
            if !names.insert(command.name.clone())
                || command
                    .aliases
                    .iter()
                    .any(|alias| !names.insert(alias.clone()))
            {
                return Err(OidError::InvalidInput(
                    "duplicate capability command name".to_owned(),
                ));
            }
        }
        drop(native);
        let mut capabilities = self
            .capabilities
            .write()
            .map_err(|_| OidError::Execution("capability registry poisoned".to_owned()))?;
        if capabilities
            .values()
            .flat_map(|registered| {
                registered.descriptor.commands.iter().flat_map(|command| {
                    std::iter::once(command.name.as_str())
                        .chain(command.aliases.iter().map(String::as_str))
                })
            })
            .any(|name| names.contains(name))
        {
            return Err(OidError::InvalidInput(
                "capability command already registered".to_owned(),
            ));
        }
        if capabilities.contains_key(descriptor.id.as_str()) {
            return Err(OidError::InvalidInput(format!(
                "capability already registered: {}",
                descriptor.id
            )));
        }
        capabilities.insert(
            descriptor.id.to_string(),
            RegisteredCapability { descriptor },
        );
        Ok(())
    }

    /// Unregister a capability and return its metadata.
    ///
    /// # Errors
    ///
    /// Returns [`OidError::NotFound`] when the capability is not registered.
    pub fn unregister(&self, id: &SkillId) -> Result<RegisteredCapability, OidError> {
        let mut capabilities = self
            .capabilities
            .write()
            .map_err(|_| OidError::Execution("capability registry poisoned".to_owned()))?;
        capabilities
            .remove(id.as_str())
            .ok_or_else(|| OidError::NotFound(format!("capability: {id}")))
    }

    /// Find a dynamic command by its command name or alias.
    #[must_use]
    pub fn find_command(&self, name: &str) -> Option<CapabilityCommand> {
        let capabilities = self.capabilities.read().ok()?;
        capabilities
            .values()
            .flat_map(|capability| capability.descriptor.commands.iter())
            .find(|command| command.matches(name))
            .cloned()
    }

    /// Return a stable, sorted snapshot of all loaded capabilities.
    #[must_use]
    pub fn list(&self) -> Vec<RegisteredCapability> {
        self.capabilities
            .read()
            .map(|capabilities| capabilities.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Return dynamic commands matching a partial command prefix.
    #[must_use]
    pub fn complete(&self, prefix: &str) -> Vec<String> {
        let Ok(capabilities) = self.capabilities.read() else {
            return Vec::new();
        };
        let mut names: Vec<_> = capabilities
            .values()
            .flat_map(|capability| capability.descriptor.commands.iter())
            .flat_map(|command| {
                std::iter::once(command.name.clone()).chain(command.aliases.clone())
            })
            .filter(|name| name.starts_with(prefix))
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

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

#[cfg(test)]
mod tests {
    use super::{CapabilityCommand, CapabilityDescriptor, CapabilityRegistry};
    use oid_common::{PluginId, SkillId};

    fn descriptor(id: &str, command: &str) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: SkillId::new(id).expect("skill id"),
            plugin_id: PluginId::new("test-plugin").expect("plugin id"),
            commands: vec![CapabilityCommand {
                name: command.to_owned(),
                description: "test command".to_owned(),
                aliases: vec![],
                skill_id: SkillId::new(id).expect("skill id"),
                mutates_system: false,
                completion: vec!["status".to_owned()],
            }],
        }
    }

    #[test]
    fn registry_rejects_native_shadowing_and_supports_lifecycle() {
        let registry = CapabilityRegistry::new();
        registry.reserve_native(["git"]).expect("reserve native");
        assert!(registry.register(descriptor("postgres", "git")).is_err());
        registry
            .register(descriptor("postgres", "postgres"))
            .expect("register capability");
        assert_eq!(registry.complete("post"), vec!["postgres"]);
        assert!(registry.find_command("postgres").is_some());
        registry
            .unregister(&SkillId::new("postgres").expect("skill id"))
            .expect("unregister capability");
        assert!(registry.find_command("postgres").is_none());
    }
}
