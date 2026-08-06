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
