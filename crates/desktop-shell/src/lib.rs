//! Terminal and desktop integration contracts for Open Intelligence Desktop.
//!
//! Wayland, D-Bus, and systemd adapters will be added behind explicit interfaces.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::FoundationEvent;
use oid_plugin_sdk::{CapabilityCommand, CapabilityRegistry};

/// A validated D-Bus method call description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DbusMethodCall {
    /// Bus destination name.
    pub destination: String,
    /// Object path.
    pub path: String,
    /// Interface name.
    pub interface: String,
    /// Method name.
    pub method: String,
}

impl DbusMethodCall {
    /// Construct a D-Bus call descriptor.
    #[must_use]
    pub fn new(
        destination: impl Into<String>,
        path: impl Into<String>,
        interface: impl Into<String>,
        method: impl Into<String>,
    ) -> Self {
        Self {
            destination: destination.into(),
            path: path.into(),
            interface: interface.into(),
            method: method.into(),
        }
    }
}

/// Boundary for isolated D-Bus adapters.
pub trait DbusTransport: Send + Sync {
    /// Execute a read-only D-Bus method call.
    ///
    /// # Errors
    ///
    /// Returns an error when the transport cannot complete the call.
    fn call_read_only(&self, call: &DbusMethodCall) -> Result<String, oid_common::OidError>;
}

/// The execution path selected for one prompt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputRoute {
    /// Execute an existing operating-system command unchanged.
    NativeCli {
        /// Executable name.
        command: String,
        /// Arguments after the executable.
        args: Vec<String>,
    },
    /// Execute a registered, governed capability.
    DynamicCapability {
        /// Registered capability command metadata.
        command: CapabilityCommand,
        /// Arguments after the capability command.
        args: Vec<String>,
    },
    /// Pass the complete request to intent planning.
    Intent {
        /// Original user input.
        request: String,
    },
}

/// Routes terminal input while preserving native CLI compatibility.
#[derive(Clone, Debug)]
pub struct InputRouter {
    registry: CapabilityRegistry,
}

impl InputRouter {
    /// Construct a router over a capability registry.
    #[must_use]
    pub fn new(registry: CapabilityRegistry) -> Self {
        Self { registry }
    }

    /// Classify input into native CLI, dynamic capability, or intent mode.
    #[must_use]
    pub fn route(&self, input: &str) -> InputRoute {
        let request = input.trim();
        let tokens = shell_words(request);
        let Some(command) = tokens.first() else {
            return InputRoute::Intent {
                request: request.to_owned(),
            };
        };
        if let Some(capability) = self.registry.find_command(command) {
            return InputRoute::DynamicCapability {
                command: capability,
                args: tokens.into_iter().skip(1).collect(),
            };
        }
        if is_native_command(command) {
            return InputRoute::NativeCli {
                command: command.clone(),
                args: tokens.into_iter().skip(1).collect(),
            };
        }
        InputRoute::Intent {
            request: request.to_owned(),
        }
    }

    /// Return the backing capability registry.
    #[must_use]
    pub const fn registry(&self) -> &CapabilityRegistry {
        &self.registry
    }

    /// Provide help lines for all currently loaded dynamic commands.
    #[must_use]
    pub fn help_lines(&self) -> Vec<String> {
        self.registry
            .list()
            .into_iter()
            .flat_map(|capability| capability.descriptor.commands)
            .map(|command| {
                let aliases = if command.aliases.is_empty() {
                    String::new()
                } else {
                    format!(" (aliases: {})", command.aliases.join(", "))
                };
                format!("  {}{}  {}", command.name, aliases, command.description)
            })
            .collect()
    }

    /// Provide command-name completion candidates.
    #[must_use]
    pub fn complete(&self, prefix: &str) -> Vec<String> {
        self.registry.complete(prefix)
    }
}

fn shell_words(input: &str) -> Vec<String> {
    input.split_whitespace().map(str::to_owned).collect()
}

fn is_native_command(command: &str) -> bool {
    const COMMON_NATIVE_COMMANDS: &[&str] = &[
        "awk",
        "bash",
        "cargo",
        "cat",
        "chmod",
        "cp",
        "curl",
        "docker",
        "find",
        "git",
        "grep",
        "kill",
        "ls",
        "make",
        "mkdir",
        "mv",
        "ps",
        "pwd",
        "rm",
        "sed",
        "ssh",
        "systemctl",
        "tar",
        "touch",
        "uname",
        "whoami",
    ];
    COMMON_NATIVE_COMMANDS.contains(&command) || command_on_path(command)
}

fn command_on_path(command: &str) -> bool {
    let candidate = std::path::Path::new(command);
    if candidate.components().count() > 1 {
        return candidate.is_file();
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(command))
        .any(|path| {
            if !path.is_file() {
                return false;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                path.metadata()
                    .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
                    .unwrap_or(false)
            }
            #[cfg(not(unix))]
            {
                true
            }
        })
}

/// User-visible operation plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserPlan {
    /// What OID proposes to do.
    pub action: String,
    /// Why the action is being proposed.
    pub rationale: String,
    /// How the action can be undone, when supported.
    pub undo: Option<String>,
}

/// Boundary for rendering plans and evidence to a terminal or desktop surface.
pub trait ShellSurface: Send + Sync {
    /// Render a plan before execution.
    fn show_plan(&self, plan: &UserPlan);

    /// Render a lifecycle event.
    fn show_event(&self, event: &FoundationEvent);
}

/// Boundary for receiving user decisions from a shell surface.
pub trait ApprovalInput: Send + Sync {
    /// Ask the user for approval of a visible plan.
    fn request_approval(&self, plan: &UserPlan) -> bool;
}

/// Identifies the desktop boundary.
///
/// ```
/// assert_eq!(oid_desktop_shell::boundary_name(), "desktop-shell");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "desktop-shell"
}

#[cfg(test)]
mod tests {
    use super::{InputRoute, InputRouter};
    use oid_common::{PluginId, SkillId};
    use oid_plugin_sdk::{CapabilityCommand, CapabilityDescriptor, CapabilityRegistry};

    fn router() -> InputRouter {
        let registry = CapabilityRegistry::new();
        registry
            .reserve_native(["git", "cargo"])
            .expect("native commands");
        registry
            .register(CapabilityDescriptor {
                id: SkillId::new("postgres").expect("skill id"),
                plugin_id: PluginId::new("test-plugin").expect("plugin id"),
                commands: vec![CapabilityCommand {
                    name: "postgres".to_owned(),
                    description: "Diagnose PostgreSQL".to_owned(),
                    aliases: vec!["pg".to_owned()],
                    skill_id: SkillId::new("postgres").expect("skill id"),
                    mutates_system: false,
                    completion: vec!["status".to_owned()],
                }],
            })
            .expect("capability");
        InputRouter::new(registry)
    }

    #[test]
    fn routes_native_dynamic_and_intent_inputs() {
        let router = router();
        assert!(matches!(
            router.route("git status"),
            InputRoute::NativeCli { .. }
        ));
        assert!(matches!(
            router.route("postgres status"),
            InputRoute::DynamicCapability { .. }
        ));
        assert!(matches!(
            router.route("fix postgres won't start"),
            InputRoute::Intent { .. }
        ));
        assert!(matches!(
            router.route("sh -c true"),
            InputRoute::NativeCli { .. }
        ));
    }

    #[test]
    fn exposes_help_and_completion_for_loaded_capabilities() {
        let router = router();
        assert_eq!(router.complete("po"), vec!["postgres"]);
        assert!(router
            .help_lines()
            .iter()
            .any(|line| line.contains("postgres")));
    }
}
