//! Evidence and audit contracts for Open Intelligence Desktop.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Identifies the evidence boundary.
///
/// ```
/// assert_eq!(oid_evidence_engine::boundary_name(), "evidence-engine");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "evidence-engine"
}
