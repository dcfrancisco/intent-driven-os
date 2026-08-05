//! Typed Linux operation and skill contracts for Open Intelligence Desktop.
//!
//! Concrete operations are deliberately absent from this foundation milestone.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Identifies the Linux operations boundary.
///
/// ```
/// assert_eq!(oid_linux_skills::boundary_name(), "linux-skills");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "linux-skills"
}
