//! Post-operation verification contracts for Open Intelligence Desktop.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::{
    ExecutionResult, OidError, OperationId, RollbackPlan, RollbackResult, VerificationPlan,
    VerificationResult,
};
use std::collections::BTreeSet;

/// Backward-compatible name for the canonical verification result.
pub type VerificationReport = VerificationResult;

/// Post-operation verification boundary.
pub trait Verifier: Send + Sync {
    /// Validate postconditions without changing system state.
    ///
    /// # Errors
    ///
    /// Returns an error when verification cannot be performed.
    fn verify(
        &self,
        plan: &VerificationPlan,
        execution: &ExecutionResult,
    ) -> Result<VerificationResult, OidError>;

    /// Validate that a rollback restored the expected state.
    ///
    /// # Errors
    ///
    /// Returns an error when rollback verification cannot be performed.
    fn verify_rollback(
        &self,
        plan: &RollbackPlan,
        execution: &ExecutionResult,
    ) -> Result<RollbackResult, OidError>;
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
    fn verify(
        &self,
        plan: &VerificationPlan,
        execution: &ExecutionResult,
    ) -> Result<VerificationResult, OidError> {
        let passed = !plan.checks.is_empty() && self.successful.contains(&execution.operation_id);
        Ok(VerificationResult {
            operation_id: execution.operation_id.clone(),
            passed,
            summary: if passed {
                "postconditions passed"
            } else {
                "operation was not recorded"
            }
            .to_owned(),
        })
    }

    fn verify_rollback(
        &self,
        plan: &RollbackPlan,
        execution: &ExecutionResult,
    ) -> Result<RollbackResult, OidError> {
        Ok(RollbackResult {
            operation_id: execution.operation_id.clone(),
            changed: plan.supported,
            summary: if plan.supported {
                plan.description.clone()
            } else {
                "rollback is not supported".to_owned()
            },
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
