//! Post-operation verification contracts for Open Intelligence Desktop.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::{OidError, OperationId};
use std::collections::BTreeSet;

/// Result of checking an operation's postconditions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationReport {
    /// Operation that was checked.
    pub operation_id: OperationId,
    /// Whether all required checks passed.
    pub passed: bool,
    /// Human-readable check summary.
    pub summary: String,
    /// Whether rollback should be attempted.
    pub rollback_required: bool,
}

/// Post-operation verification boundary.
pub trait Verifier: Send + Sync {
    /// Validate postconditions without changing system state.
    ///
    /// # Errors
    ///
    /// Returns an error when verification cannot be performed.
    fn verify(&self, operation_id: &OperationId) -> Result<VerificationReport, OidError>;

    /// Validate that a rollback restored the expected state.
    ///
    /// # Errors
    ///
    /// Returns an error when rollback verification cannot be performed.
    fn verify_rollback(&self, operation_id: &OperationId) -> Result<VerificationReport, OidError>;
}

/// Deterministic verifier used by read-only operations and contract tests.
#[derive(Clone, Debug, Default)]
pub struct InMemoryVerifier {
    successful: BTreeSet<OperationId>,
}

impl InMemoryVerifier {
    /// Mark an operation as having produced a result that can be verified.
    pub fn mark_successful(&mut self, operation_id: OperationId) {
        self.successful.insert(operation_id);
    }
}

impl Verifier for InMemoryVerifier {
    fn verify(&self, operation_id: &OperationId) -> Result<VerificationReport, OidError> {
        let passed = self.successful.contains(operation_id);
        Ok(VerificationReport {
            operation_id: operation_id.clone(),
            passed,
            summary: if passed {
                "postconditions passed"
            } else {
                "operation was not recorded"
            }
            .to_owned(),
            rollback_required: false,
        })
    }

    fn verify_rollback(&self, operation_id: &OperationId) -> Result<VerificationReport, OidError> {
        Ok(VerificationReport {
            operation_id: operation_id.clone(),
            passed: false,
            summary: "rollback is not applicable to this read-only operation".to_owned(),
            rollback_required: false,
        })
    }
}

/// Identifies the verification boundary.
///
/// ```
/// assert_eq!(oid_verification_engine::boundary_name(), "verification-engine");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "verification-engine"
}
