//! Authorization and approval contracts for Open Intelligence Desktop.
//!
//! Policy decisions must remain explicit and independently testable.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Identifies the policy boundary.
///
/// ```
/// assert_eq!(oid_policy_engine::boundary_name(), "policy-engine");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "policy-engine"
}
