//! Shared contracts and infrastructure primitives for Open Intelligence Desktop.
//!
//! This crate is intentionally dependency-light. Cross-cutting models should live here
//! only when they are genuinely shared by multiple bounded contexts.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Returns the name of this crate's current foundation layer.
///
/// ```
/// assert_eq!(oid_common::crate_name(), "common");
/// ```
#[must_use]
pub const fn crate_name() -> &'static str {
    "common"
}
