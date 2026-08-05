//! Documentation anchor for the Open Intelligence Desktop workspace.
//!
//! Project documentation lives at the repository root and in `docs/`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Identifies the documentation package.
///
/// ```
/// assert_eq!(oid_docs::crate_name(), "docs");
/// ```
#[must_use]
pub const fn crate_name() -> &'static str {
    "docs"
}
