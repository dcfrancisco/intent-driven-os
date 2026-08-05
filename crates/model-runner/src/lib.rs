//! Placeholder interfaces for a future optimized model runner.
//!
//! No model, inference engine, or AI integration is implemented in this crate.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Identifies the model boundary.
///
/// ```
/// assert_eq!(oid_model_runner::boundary_name(), "model-runner");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "model-runner"
}
