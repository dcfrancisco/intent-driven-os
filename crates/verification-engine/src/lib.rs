//! Post-operation verification contracts for Open Intelligence Desktop.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Identifies the verification boundary.
///
/// ```
/// assert_eq!(oid_verification_engine::boundary_name(), "verification-engine");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "verification-engine"
}
