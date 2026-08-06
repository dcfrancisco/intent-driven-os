//! Placeholder interfaces for a future optimized model runner.
//!
//! No model, inference engine, or AI integration is implemented in this crate.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::{OidError, OperationId};

/// Input boundary reserved for future model assistance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelRequest {
    /// Operation for which assistance was requested.
    pub operation_id: OperationId,
    /// User-visible prompt or context.
    pub input: String,
}

/// Output boundary reserved for future model assistance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelResponse {
    /// Proposed text; it is never an executable operation by itself.
    pub output: String,
}

/// Future model-runner boundary. The foundation implementation is intentionally absent.
pub trait ModelRunner: Send + Sync {
    /// Request assistance without granting execution authority.
    ///
    /// # Errors
    ///
    /// Returns an error when the future runner cannot produce a response.
    fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, OidError>;
}

/// Identifies the model boundary.
///
/// ```
/// assert_eq!(oid_model_runner::boundary_name(), "model-runner");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "model-runner"
}
