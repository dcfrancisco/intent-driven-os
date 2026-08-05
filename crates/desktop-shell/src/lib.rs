//! Terminal and desktop integration contracts for Open Intelligence Desktop.
//!
//! Wayland, D-Bus, and systemd adapters will be added behind explicit interfaces.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Identifies the desktop boundary.
///
/// ```
/// assert_eq!(oid_desktop_shell::boundary_name(), "desktop-shell");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "desktop-shell"
}
