//! Shared runtime value types.

use std::fmt;

/// Coarse lifecycle state exposed to clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleState {
    /// The runtime has not started.
    Initializing,
    /// The runtime is accepting requests.
    Ready,
    /// The runtime is stopping.
    Stopping,
    /// The runtime has stopped.
    Stopped,
}

impl fmt::Display for LifecycleState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Initializing => "Initializing",
            Self::Ready => "Ready",
            Self::Stopping => "Stopping",
            Self::Stopped => "Stopped",
        };
        formatter.write_str(label)
    }
}

/// Small, transport-neutral snapshot of runtime state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStatus {
    /// Current lifecycle state.
    pub state: LifecycleState,
    /// Runtime protocol version.
    pub version: &'static str,
}
