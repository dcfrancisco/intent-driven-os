//! Intent contracts and lifecycle boundaries for Open Intelligence Desktop.
//!
//! The runtime will own intent state transitions. Execution belongs to other crates.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Name of the lifecycle boundary represented by this crate.
///
/// ```
/// assert_eq!(oid_intent_runtime::boundary_name(), "intent-runtime");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "intent-runtime"
}
