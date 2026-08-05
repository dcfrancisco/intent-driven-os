//! Security policy boundary.

use oid_shared::RuntimeError;

/// Identity presented by a runtime client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Principal {
    /// Stable principal name.
    pub name: String,
}

/// Authorization interface for future policy enforcement.
pub trait Policy: Send + Sync {
    /// Authorize an operation for a principal.
    ///
    /// # Errors
    ///
    /// Returns a policy error when the principal is not authorized.
    fn authorize(&self, _principal: &Principal, _operation: &str) -> Result<(), RuntimeError>;
}

/// Explicitly permissive placeholder for the non-operational Phase 1 runtime.
#[derive(Clone, Debug, Default)]
pub struct FoundationPolicy;

impl Policy for FoundationPolicy {
    fn authorize(&self, _principal: &Principal, _operation: &str) -> Result<(), RuntimeError> {
        Ok(())
    }
}
