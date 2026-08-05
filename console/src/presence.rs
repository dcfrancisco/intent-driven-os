//! Runtime-event-driven adaptive prompt presence indicator.

use oid_shared::{EventBus, EventReceiver, RuntimeEvent};

/// Semantic presence state represented by the prompt cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceState {
    /// Ready for input.
    Idle,
    /// User is editing input.
    Typing,
    /// Runtime is processing a command.
    Busy,
    /// Runtime output is arriving.
    Streaming,
    /// Runtime is waiting for explicit approval.
    Waiting,
    /// Runtime reported a recoverable issue.
    Warning,
    /// Runtime reported an error.
    Error,
    /// Runtime or backend is unavailable.
    Offline,
}

/// Prompt presence controller subscribed to runtime events.
#[derive(Debug)]
pub struct PresenceIndicator {
    state: PresenceState,
    receiver: EventReceiver,
}

impl PresenceIndicator {
    /// Subscribe to the runtime event bus.
    #[must_use]
    pub fn new(bus: &EventBus) -> Self {
        Self {
            state: PresenceState::Idle,
            receiver: bus.subscribe(),
        }
    }

    /// Drain and apply all pending runtime events.
    pub fn refresh(&mut self) {
        while let Ok(event) = self.receiver.try_recv() {
            self.apply(&event);
        }
    }

    /// Return the current semantic state.
    #[allow(dead_code)]
    #[must_use]
    pub const fn state(&self) -> PresenceState {
        self.state
    }

    /// Return a subtle, theme-neutral cursor token.
    #[must_use]
    pub const fn token(&self) -> &'static str {
        match self.state {
            PresenceState::Idle => "▋",
            PresenceState::Typing | PresenceState::Waiting => "▌",
            PresenceState::Busy => "█",
            PresenceState::Streaming => "▍",
            PresenceState::Warning => "▐",
            PresenceState::Error => "■",
            PresenceState::Offline => "□",
        }
    }

    fn apply(&mut self, event: &RuntimeEvent) {
        self.state = match event {
            RuntimeEvent::RuntimeStarted
            | RuntimeEvent::ConfigurationLoaded
            | RuntimeEvent::StateChanged(oid_shared::LifecycleState::Ready)
            | RuntimeEvent::InputStopped
            | RuntimeEvent::ThinkingFinished
            | RuntimeEvent::StreamingStopped
            | RuntimeEvent::CommandCompleted(_) => PresenceState::Idle,
            RuntimeEvent::RuntimeStopped
            | RuntimeEvent::Offline
            | RuntimeEvent::StateChanged(oid_shared::LifecycleState::Stopped) => {
                PresenceState::Offline
            }
            RuntimeEvent::InputStarted => PresenceState::Typing,
            RuntimeEvent::CommandStarted(_)
            | RuntimeEvent::ThinkingStarted
            | RuntimeEvent::ApprovalGranted => PresenceState::Busy,
            RuntimeEvent::StreamingStarted => PresenceState::Streaming,
            RuntimeEvent::ApprovalRequested => PresenceState::Waiting,
            RuntimeEvent::WarningRaised(_) => PresenceState::Warning,
            RuntimeEvent::ErrorRaised(_) => PresenceState::Error,
            RuntimeEvent::LoggingInitialized
            | RuntimeEvent::StateChanged(_)
            | RuntimeEvent::BackendRegistered(_)
            | RuntimeEvent::BackendEnabled(_)
            | RuntimeEvent::BackendDisabled(_)
            | RuntimeEvent::ModelRegistered(_)
            | RuntimeEvent::ModelLoaded(_)
            | RuntimeEvent::ModelUnloaded(_)
            | RuntimeEvent::HardwareDetected
            | RuntimeEvent::HealthUpdated(_)
            | RuntimeEvent::BackendInitialized(_)
            | RuntimeEvent::ModelLoading(_)
            | RuntimeEvent::ModelUnloading(_)
            | RuntimeEvent::TokenizerReady(_) => self.state,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::{PresenceIndicator, PresenceState};
    use oid_shared::{EventBus, RuntimeEvent};

    #[test]
    fn cursor_state_changes_only_after_runtime_events() {
        let bus = EventBus::new();
        let mut indicator = PresenceIndicator::new(&bus);
        assert_eq!(indicator.state(), PresenceState::Idle);
        bus.publish(&RuntimeEvent::StreamingStarted);
        indicator.refresh();
        assert_eq!(indicator.state(), PresenceState::Streaming);
        assert_eq!(indicator.token(), "▍");
    }
}
