//! Plugin extension contracts for Open Intelligence Desktop.
//!
//! This crate defines future-facing boundaries without loading or executing plugins yet.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Identifies the plugin boundary.
///
/// ```
/// assert_eq!(oid_plugin_sdk::boundary_name(), "plugin-sdk");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "plugin-sdk"
}
