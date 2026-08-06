//! Authorization and approval contracts for Open Intelligence Desktop.
//!
//! Policy decisions must remain explicit and independently testable.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::{OidError, OperationId, SkillId};

/// Result of evaluating whether an operation may proceed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorizationDecision {
    /// Proceed without interactive approval.
    Allowed,
    /// Pause until the user explicitly approves.
    ApprovalRequired {
        /// Explanation shown to the user.
        reason: String,
    },
    /// Refuse the operation.
    Denied {
        /// Explanation for the denial.
        reason: String,
    },
}

/// Context supplied to a policy evaluator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationRequest {
    /// Operation being evaluated.
    pub operation_id: OperationId,
    /// Skill requested by the operation.
    pub skill_id: SkillId,
    /// Whether the operation mutates system state.
    pub mutates_system: bool,
}

/// A user approval that can be attached to one operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Approval {
    /// Operation approved by the user.
    pub operation_id: OperationId,
    /// Human-readable approval note.
    pub note: String,
}

/// Policy evaluation boundary.
pub trait PolicyEvaluator: Send + Sync {
    /// Evaluate authorization without executing the operation.
    ///
    /// # Errors
    ///
    /// Returns an error when policy evaluation cannot be completed.
    fn evaluate(&self, request: &AuthorizationRequest) -> Result<AuthorizationDecision, OidError>;
}

/// Approval persistence/validation boundary.
pub trait ApprovalStore: Send + Sync {
    /// Record explicit user approval.
    ///
    /// # Errors
    ///
    /// Returns an error when approval cannot be recorded.
    fn approve(&mut self, approval: Approval) -> Result<(), OidError>;

    /// Check whether an operation has approval.
    ///
    /// # Errors
    ///
    /// Returns an error when approval state cannot be read.
    fn is_approved(&self, operation_id: &OperationId) -> Result<bool, OidError>;
}

/// Deterministic policy that allows read-only skills and requires approval for mutations.
#[derive(Clone, Debug, Default)]
pub struct ReadOnlyPolicy;

impl PolicyEvaluator for ReadOnlyPolicy {
    fn evaluate(&self, request: &AuthorizationRequest) -> Result<AuthorizationDecision, OidError> {
        Ok(if request.mutates_system {
            AuthorizationDecision::ApprovalRequired {
                reason: format!("skill {} can change system state", request.skill_id),
            }
        } else {
            AuthorizationDecision::Allowed
        })
    }
}

/// In-memory approval store for deterministic tests and the reference client.
#[derive(Clone, Debug, Default)]
pub struct InMemoryApprovalStore {
    approvals: std::collections::BTreeSet<OperationId>,
}

impl ApprovalStore for InMemoryApprovalStore {
    fn approve(&mut self, approval: Approval) -> Result<(), OidError> {
        self.approvals.insert(approval.operation_id);
        Ok(())
    }

    fn is_approved(&self, operation_id: &OperationId) -> Result<bool, OidError> {
        Ok(self.approvals.contains(operation_id))
    }
}

/// Identifies the policy boundary.
///
/// ```
/// assert_eq!(oid_policy_engine::boundary_name(), "policy-engine");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "policy-engine"
}

#[cfg(test)]
mod tests {
    use super::{AuthorizationDecision, AuthorizationRequest, PolicyEvaluator, ReadOnlyPolicy};
    use oid_common::{OperationId, SkillId};

    fn request(mutates_system: bool) -> AuthorizationRequest {
        AuthorizationRequest {
            operation_id: OperationId::new("operation-1").expect("valid id"),
            skill_id: SkillId::new("skill-1").expect("valid id"),
            mutates_system,
        }
    }

    #[test]
    fn read_only_operations_are_allowed() {
        assert_eq!(
            ReadOnlyPolicy
                .evaluate(&request(false))
                .expect("policy works"),
            AuthorizationDecision::Allowed
        );
    }

    #[test]
    fn mutating_operations_require_approval() {
        assert!(matches!(
            ReadOnlyPolicy
                .evaluate(&request(true))
                .expect("policy works"),
            AuthorizationDecision::ApprovalRequired { .. }
        ));
    }
}
