//! Intent contracts and lifecycle boundaries for Open Intelligence Desktop.
//!
//! The runtime owns intent state transitions. Execution belongs to other crates.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::{IntentId, OidError};
use oid_shared::{EventBus, RuntimeEvent};

/// Explicit lifecycle states for a user intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentState {
    /// The intent has been accepted but not authorized.
    Proposed,
    /// Policy has authorized the intent.
    Authorized,
    /// The operation is waiting for user approval.
    AwaitingApproval,
    /// The operation is executing.
    Executing,
    /// The operation is being validated.
    Verifying,
    /// The operation completed and its postconditions passed.
    Completed,
    /// Policy or the user rejected the intent.
    Rejected,
    /// Execution failed.
    Failed,
    /// Verification requested a rollback.
    RollbackRequired,
    /// Rollback completed and was verified.
    RolledBack,
    /// Rollback failed and requires operator attention.
    RollbackFailed,
}

/// Risk class assigned to a proposed system intent before execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentRisk {
    /// Observation only; no system mutation is allowed.
    ReadOnly,
    /// A user-scoped change with a defined rollback or recovery path.
    ReversibleUser,
    /// A privileged change that still has a defined rollback or recovery path.
    PrivilegedReversible,
    /// A destructive, security-sensitive, or externally consequential change.
    Sensitive,
}

/// A validated, backend-neutral intent proposal.
///
/// Model output may populate this proposal, but it is not an execution plan.
/// Policy, the operation coordinator, and the selected skill must validate it
/// again before any operation is authorized or executed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntentProposal {
    /// Stable identity of the user request.
    pub id: IntentId,
    /// Authenticated requester identity.
    pub requester: String,
    /// Desired user-visible outcome.
    pub desired_outcome: String,
    /// Resources the proposal may inspect or change.
    pub targets: Vec<String>,
    /// Typed capabilities required by the proposal.
    pub required_capabilities: Vec<String>,
    /// Policy risk classification.
    pub risk: IntentRisk,
    /// Authority required by the proposed operation.
    pub required_authority: String,
    /// Human-readable proposed steps; these are explanatory until compiled to
    /// a typed operation by a trusted coordinator.
    pub proposed_steps: Vec<String>,
    /// Observable postconditions that must be checked after execution.
    pub verification_criteria: Vec<String>,
    /// Explicit execution limits such as time, scope, or resource budgets.
    pub execution_constraints: Vec<String>,
}

impl IntentProposal {
    /// Validate the minimum contract shared by all `MarinaOS` intent adapters.
    ///
    /// # Errors
    ///
    /// Returns [`OidError::InvalidInput`] when a required field is missing.
    pub fn validate(&self) -> Result<(), OidError> {
        let required = [
            ("requester", self.requester.as_str()),
            ("desired outcome", self.desired_outcome.as_str()),
            ("required authority", self.required_authority.as_str()),
        ];
        for (name, value) in required {
            if value.trim().is_empty() {
                return Err(OidError::InvalidInput(name.to_owned()));
            }
        }
        if self.targets.is_empty() {
            return Err(OidError::InvalidInput("targets".to_owned()));
        }
        if self.required_capabilities.is_empty() {
            return Err(OidError::InvalidInput("required capabilities".to_owned()));
        }
        if self.proposed_steps.is_empty() {
            return Err(OidError::InvalidInput("proposed steps".to_owned()));
        }
        if self.verification_criteria.is_empty() {
            return Err(OidError::InvalidInput("verification criteria".to_owned()));
        }
        if self.execution_constraints.is_empty() {
            return Err(OidError::InvalidInput("execution constraints".to_owned()));
        }
        Ok(())
    }
}

impl IntentState {
    /// Return whether this is a terminal state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::Rejected
                | Self::Failed
                | Self::RolledBack
                | Self::RollbackFailed
        )
    }

    /// Return a stable display label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Authorized => "authorized",
            Self::AwaitingApproval => "awaiting-approval",
            Self::Executing => "executing",
            Self::Verifying => "verifying",
            Self::Completed => "completed",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
            Self::RollbackRequired => "rollback-required",
            Self::RolledBack => "rolled-back",
            Self::RollbackFailed => "rollback-failed",
        }
    }
}

/// An intent and its current lifecycle state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Intent {
    /// Stable intent identity.
    pub id: IntentId,
    /// User-visible description.
    pub description: String,
    /// Current state.
    pub state: IntentState,
}

/// Intent lifecycle controller that mirrors transitions onto the shared event bus.
#[derive(Clone, Debug)]
pub struct IntentSession {
    intent: Intent,
    events: EventBus,
}

impl IntentSession {
    /// Create a session and publish its proposed state.
    ///
    /// # Errors
    ///
    /// Returns an error when the intent description is empty.
    pub fn new(
        id: IntentId,
        description: impl Into<String>,
        events: EventBus,
    ) -> Result<Self, OidError> {
        let session = Self {
            intent: Intent::new(id, description)?,
            events,
        };
        session.publish_state();
        Ok(session)
    }

    /// Return the current intent snapshot.
    #[must_use]
    pub const fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Transition the intent and publish the new state.
    ///
    /// # Errors
    ///
    /// Returns an error when the transition is outside the lifecycle graph.
    pub fn transition(&mut self, next: IntentState) -> Result<(), OidError> {
        self.intent.transition(next)?;
        self.publish_state();
        Ok(())
    }

    fn publish_state(&self) {
        self.events.publish(&RuntimeEvent::IntentStateChanged {
            intent_id: self.intent.id.to_string(),
            state: self.intent.state.as_str().to_owned(),
        });
    }
}

impl Intent {
    /// Create a proposed intent.
    ///
    /// # Errors
    ///
    /// Returns an error when the description is empty.
    pub fn new(id: IntentId, description: impl Into<String>) -> Result<Self, OidError> {
        let description = description.into();
        if description.trim().is_empty() {
            return Err(OidError::InvalidInput("intent description".to_owned()));
        }
        Ok(Self {
            id,
            description,
            state: IntentState::Proposed,
        })
    }

    /// Transition the intent through the explicit lifecycle.
    ///
    /// # Errors
    ///
    /// Returns [`OidError::InvalidTransition`] for a transition outside the lifecycle graph.
    pub fn transition(&mut self, next: IntentState) -> Result<(), OidError> {
        let valid = matches!(
            (self.state, next),
            (
                IntentState::Proposed,
                IntentState::Authorized | IntentState::Rejected
            ) | (
                IntentState::Authorized,
                IntentState::AwaitingApproval | IntentState::Executing | IntentState::Rejected
            ) | (
                IntentState::AwaitingApproval,
                IntentState::Executing | IntentState::Rejected
            ) | (
                IntentState::Executing,
                IntentState::Verifying | IntentState::Failed
            ) | (
                IntentState::Verifying,
                IntentState::Completed | IntentState::RollbackRequired | IntentState::Failed
            ) | (
                IntentState::RollbackRequired,
                IntentState::RolledBack | IntentState::RollbackFailed
            )
        );
        if !valid {
            return Err(OidError::InvalidTransition {
                from: self.state.as_str().to_owned(),
                to: next.as_str().to_owned(),
            });
        }
        self.state = next;
        Ok(())
    }
}

/// Name of the lifecycle boundary represented by this crate.
///
/// ```
/// assert_eq!(oid_intent_runtime::boundary_name(), "intent-runtime");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "intent-runtime"
}

#[cfg(test)]
mod tests {
    use super::{Intent, IntentState};
    use oid_common::IntentId;

    #[test]
    fn accepts_the_happy_path_through_verification() {
        let id = IntentId::new("intent-1").expect("valid id");
        let mut intent = Intent::new(id, "inspect system health").expect("valid intent");
        for state in [
            IntentState::Authorized,
            IntentState::Executing,
            IntentState::Verifying,
            IntentState::Completed,
        ] {
            intent
                .transition(state)
                .expect("valid lifecycle transition");
        }
        assert!(intent.state.is_terminal());
    }

    #[test]
    fn rejects_skipping_authorization() {
        let id = IntentId::new("intent-2").expect("valid id");
        let mut intent = Intent::new(id, "change configuration").expect("valid intent");
        assert!(intent.transition(IntentState::Executing).is_err());
        assert_eq!(intent.state, IntentState::Proposed);
    }
}
