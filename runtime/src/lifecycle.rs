//! Explicit runtime lifecycle transitions.

use oid_shared::{LifecycleState, RuntimeError};

/// Lifecycle state machine for the runtime service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lifecycle {
    state: LifecycleState,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self {
            state: LifecycleState::Initializing,
        }
    }
}

impl Lifecycle {
    /// Return the current state.
    #[must_use]
    pub const fn state(&self) -> LifecycleState {
        self.state
    }

    /// Move to a valid next state.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested transition is not part of the
    /// explicit Phase 1 lifecycle.
    pub fn transition(&mut self, next: LifecycleState) -> Result<(), RuntimeError> {
        let valid = matches!(
            (self.state, next),
            (LifecycleState::Initializing, LifecycleState::Ready)
                | (LifecycleState::Ready, LifecycleState::Stopping)
                | (LifecycleState::Stopping, LifecycleState::Stopped)
        );
        if !valid {
            return Err(RuntimeError::InvalidLifecycleTransition {
                from: self.state.to_string(),
                to: next.to_string(),
            });
        }
        self.state = next;
        Ok(())
    }
}
