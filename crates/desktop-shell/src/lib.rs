//! Terminal and desktop integration contracts for Open Intelligence Desktop.
//!
//! Wayland, D-Bus, and systemd adapters will be added behind explicit interfaces.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::FoundationEvent;

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
