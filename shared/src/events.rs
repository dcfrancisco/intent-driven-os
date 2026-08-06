//! Shared lifecycle events and the lightweight event bus.

use crate::types::LifecycleState;
use std::sync::{mpsc, Arc, Mutex};

/// Events emitted by the runtime lifecycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeEvent {
    /// Configuration was accepted.
    ConfigurationLoaded,
    /// Logging was initialized.
    LoggingInitialized,
    /// The runtime entered a new lifecycle state.
    StateChanged(LifecycleState),
    /// The runtime service became available.
    RuntimeStarted,
    /// The runtime service stopped.
    RuntimeStopped,
    /// A console command began processing.
    CommandStarted(String),
    /// A console command finished processing.
    CommandCompleted(String),
    /// An intent entered a new state.
    IntentStateChanged {
        /// Intent whose lifecycle state changed.
        intent_id: String,
        /// New state label.
        state: String,
    },
    /// An operation completed successfully.
    OperationCompleted(String),
    /// An evidence record was persisted.
    EvidenceRecorded(String),
    /// Runtime planning or resource selection began.
    ThinkingStarted,
    /// Runtime planning or resource selection finished.
    ThinkingFinished,
    /// Streaming output began.
    StreamingStarted,
    /// Streaming output stopped.
    StreamingStopped,
    /// A user began editing input.
    InputStarted,
    /// A user stopped editing input.
    InputStopped,
    /// The runtime raised a warning.
    WarningRaised(String),
    /// The runtime raised an error.
    ErrorRaised(String),
    /// User approval is required before continuing.
    ApprovalRequested,
    /// User approval was granted.
    ApprovalGranted,
    /// A runtime or backend became unavailable.
    Offline,
    /// A backend was registered with the runtime.
    BackendRegistered(String),
    /// A backend was enabled.
    BackendEnabled(String),
    /// A backend was disabled.
    BackendDisabled(String),
    /// Model metadata was registered.
    ModelRegistered(String),
    /// A model was loaded by a backend.
    ModelLoaded(String),
    /// A model was unloaded by a backend.
    ModelUnloaded(String),
    /// Hardware discovery completed.
    HardwareDetected,
    /// Runtime health changed.
    HealthUpdated(String),
    /// A backend initialization completed.
    BackendInitialized(String),
    /// A model began loading.
    ModelLoading(String),
    /// A model began unloading.
    ModelUnloading(String),
    /// Hardware-backed tokenizer became available.
    TokenizerReady(String),
    /// A generation request began.
    GenerationStarted(String),
    /// The first generated token was produced.
    FirstToken(String),
    /// A generated token was produced.
    TokenGenerated(String),
    /// A generation completed with metrics.
    GenerationCompleted(String),
    /// A generation was cancelled.
    GenerationCancelled(String),
    /// A generation failed.
    GenerationFailed(String),
}

/// A subscription to events published by an [`EventBus`].
pub type EventReceiver = mpsc::Receiver<RuntimeEvent>;

/// Small, in-process publish/subscribe event bus.
///
/// The bus deliberately has no async or network dependency. It is suitable for
/// the Phase 2 reference client and can later be replaced behind the same
/// runtime-facing interface.
#[derive(Clone, Debug, Default)]
pub struct EventBus {
    subscribers: Arc<Mutex<Vec<mpsc::Sender<RuntimeEvent>>>>,
}

impl EventBus {
    /// Create an empty event bus.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe to future events.
    #[must_use]
    pub fn subscribe(&self) -> EventReceiver {
        let (sender, receiver) = mpsc::channel();
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers.push(sender);
        }
        receiver
    }

    /// Publish an event to all active subscribers.
    pub fn publish(&self, event: &RuntimeEvent) {
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers.retain(|subscriber| subscriber.send(event.clone()).is_ok());
        }
    }
}
