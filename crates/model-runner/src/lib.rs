//! Backend-neutral contract for local model inference.
//!
//! This crate owns the model-runtime vocabulary used by OID clients and
//! managers. Concrete engines, including llama.cpp, must implement this
//! contract in an adapter crate and must not leak engine-native types here.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::fmt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::Duration;

/// Stable identity for a model known to the runtime.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModelId(String);

impl ModelId {
    /// Construct an identifier from a non-empty value.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` when the value is empty or whitespace.
    pub fn new(value: impl Into<String>) -> Result<Self, ModelRunnerError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ModelRunnerError::InvalidRequest(
                "model id is empty".to_owned(),
            ));
        }
        Ok(Self(value))
    }

    /// Return the identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A local model artifact path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelPath(String);

impl ModelPath {
    /// Construct a path and require the GGUF extension used by this slice.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` for an empty path or `UnsupportedFormat` when
    /// the path does not end in `.gguf`.
    pub fn new(value: impl Into<String>) -> Result<Self, ModelRunnerError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ModelRunnerError::InvalidRequest(
                "model path is empty".to_owned(),
            ));
        }
        if !value.to_ascii_lowercase().ends_with(".gguf") {
            return Err(ModelRunnerError::UnsupportedFormat(value));
        }
        Ok(Self(value))
    }

    /// Return the path as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Request to load one local model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadModelRequest {
    /// Runtime model identity.
    pub model_id: ModelId,
    /// Local GGUF artifact path.
    pub path: ModelPath,
}

/// A successfully loaded model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedModel {
    /// Runtime model identity.
    pub model_id: ModelId,
    /// Backend identity that loaded the model.
    pub backend: String,
    /// Model format.
    pub format: String,
    /// Load duration, when measured by the adapter.
    pub load_time: Option<Duration>,
    /// Backend-reported model memory, when available.
    pub memory_bytes: Option<u64>,
}

/// Minimum generation controls for the first inference slice.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationRequest {
    /// Prompt text.
    pub prompt: String,
    /// Maximum number of generated tokens.
    pub max_tokens: u32,
    /// Sampling temperature.
    pub temperature: f32,
    /// Nucleus sampling probability in the exclusive range `(0, 1]`.
    pub top_p: f32,
    /// Top-K sampling limit.
    pub top_k: i32,
    /// Requested context size in tokens.
    pub context_size: u32,
    /// Optional deterministic seed.
    pub seed: Option<u32>,
}

impl GenerationRequest {
    /// Validate generation controls before passing them to a backend.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` when a control is outside the supported range.
    pub fn validate(&self) -> Result<(), ModelRunnerError> {
        if self.prompt.trim().is_empty() {
            return Err(ModelRunnerError::InvalidRequest(
                "prompt is empty".to_owned(),
            ));
        }
        if self.max_tokens == 0 {
            return Err(ModelRunnerError::InvalidRequest(
                "max tokens must be greater than zero".to_owned(),
            ));
        }
        if !self.temperature.is_finite() || self.temperature < 0.0 {
            return Err(ModelRunnerError::InvalidRequest(
                "temperature must be finite and non-negative".to_owned(),
            ));
        }
        if !self.top_p.is_finite() || self.top_p <= 0.0 || self.top_p > 1.0 {
            return Err(ModelRunnerError::InvalidRequest(
                "top_p must be greater than zero and at most one".to_owned(),
            ));
        }
        if self.top_k <= 0 {
            return Err(ModelRunnerError::InvalidRequest(
                "top_k must be greater than zero".to_owned(),
            ));
        }
        if self.context_size == 0 {
            return Err(ModelRunnerError::InvalidRequest(
                "context size must be greater than zero".to_owned(),
            ));
        }
        Ok(())
    }
}

/// A token or terminal generation event.
#[derive(Clone, Debug, PartialEq)]
pub enum GenerationEvent {
    /// Incremental generated text.
    Token(String),
    /// Successful completion with measurements.
    Completed(GenerationStats),
    /// Clean cancellation with measurements captured so far.
    Cancelled(GenerationStats),
    /// Backend or runtime failure.
    Error(ModelRunnerError),
}

/// Measurements for one generation request.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GenerationStats {
    /// Prompt token count, when available.
    pub prompt_tokens: Option<u64>,
    /// Generated token count.
    pub generated_tokens: u64,
    /// Time until the first token, when available.
    pub time_to_first_token: Option<Duration>,
    /// Total generation duration.
    pub generation_time: Option<Duration>,
    /// Generated tokens per second, when calculable.
    pub tokens_per_second: Option<f64>,
}

/// Runtime-level model statistics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelRuntimeStats {
    /// Currently loaded model, if any.
    pub loaded_model: Option<ModelId>,
    /// Model memory reported by the backend, when available.
    pub memory_bytes: Option<u64>,
    /// Current lifecycle state.
    pub state: ModelState,
}

/// Controlled model lifecycle state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ModelState {
    /// No model is loaded.
    #[default]
    Unloaded,
    /// A model is being loaded.
    Loading,
    /// A model is ready for generation.
    Ready,
    /// Generation is active.
    Generating,
    /// A model is being unloaded.
    Unloading,
    /// The previous lifecycle operation failed.
    Failed,
}

/// Errors owned by the model-runtime boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelRunnerError {
    /// A request is malformed.
    InvalidRequest(String),
    /// The requested artifact format is unsupported.
    UnsupportedFormat(String),
    /// The model artifact was not found.
    ModelNotFound(String),
    /// The model artifact could not be read.
    ModelUnreadable(String),
    /// A backend is unavailable.
    BackendUnavailable(String),
    /// The model is not currently loaded.
    ModelNotLoaded,
    /// The loaded model is already in use or another model is active.
    ModelBusy(String),
    /// A backend rejected loading or generation.
    BackendFailure(String),
    /// Resources are insufficient for the requested operation.
    ResourceExhausted(String),
    /// Generation was cancelled before completion.
    Cancelled,
    /// Model cleanup failed.
    UnloadFailure(String),
}

impl fmt::Display for ModelRunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(value) => write!(formatter, "invalid request: {value}"),
            Self::UnsupportedFormat(value) => {
                write!(formatter, "unsupported model format: {value}")
            }
            Self::ModelNotFound(value) => write!(formatter, "model not found: {value}"),
            Self::ModelUnreadable(value) => write!(formatter, "model unreadable: {value}"),
            Self::BackendUnavailable(value) => write!(formatter, "backend unavailable: {value}"),
            Self::ModelNotLoaded => formatter.write_str("model is not loaded"),
            Self::ModelBusy(value) => write!(formatter, "model is busy: {value}"),
            Self::BackendFailure(value) => write!(formatter, "backend failure: {value}"),
            Self::ResourceExhausted(value) => write!(formatter, "resources exhausted: {value}"),
            Self::Cancelled => formatter.write_str("generation cancelled"),
            Self::UnloadFailure(value) => write!(formatter, "unload failed: {value}"),
        }
    }
}

impl std::error::Error for ModelRunnerError {}

/// Cooperative cancellation signal propagated into a backend.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// Request cancellation.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Check whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// Expose the read-only cancellation flag to an adapter implementation.
    #[must_use]
    pub fn as_atomic(&self) -> &AtomicBool {
        &self.0
    }
}

/// Incremental output stream returned by a runner.
#[derive(Debug)]
pub struct GenerationStream {
    receiver: mpsc::Receiver<GenerationEvent>,
    cancellation: CancellationToken,
}

impl GenerationStream {
    /// Construct a stream for an adapter worker.
    #[must_use]
    pub fn new(receiver: mpsc::Receiver<GenerationEvent>, cancellation: CancellationToken) -> Self {
        Self {
            receiver,
            cancellation,
        }
    }

    /// Receive the next event, blocking only the caller consuming the stream.
    ///
    /// # Errors
    ///
    /// Returns the channel receive error if the adapter worker disconnects.
    pub fn recv(&self) -> Result<GenerationEvent, mpsc::RecvError> {
        self.receiver.recv()
    }

    /// Receive the next event with a bounded wait.
    ///
    /// # Errors
    ///
    /// Returns a timeout or channel-disconnect error.
    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<GenerationEvent, mpsc::RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }

    /// Request cooperative cancellation.
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// Return a clone of the cancellation token for request registries.
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }
}

/// Stable model-runtime contract implemented by inference adapters.
pub trait ModelRunner: Send + Sync {
    /// Load one local model.
    ///
    /// # Errors
    ///
    /// Returns a typed model or backend lifecycle error.
    fn load(&self, request: LoadModelRequest) -> Result<LoadedModel, ModelRunnerError>;
    /// Start streaming generation from the currently loaded model.
    ///
    /// # Errors
    ///
    /// Returns a typed model or backend lifecycle error.
    fn generate(&self, request: GenerationRequest) -> Result<GenerationStream, ModelRunnerError>;
    /// Tokenize text with the currently loaded model.
    ///
    /// # Errors
    ///
    /// Returns a typed model or backend error when tokenization cannot run.
    fn tokenize(&self, text: &str) -> Result<u64, ModelRunnerError>;
    /// Inspect lifecycle and resource information.
    ///
    /// # Errors
    ///
    /// Returns a typed backend or synchronization error.
    fn stats(&self) -> Result<ModelRuntimeStats, ModelRunnerError>;
    /// Unload the active model safely.
    ///
    /// # Errors
    ///
    /// Returns a typed lifecycle or cleanup error.
    fn unload(&self) -> Result<(), ModelRunnerError>;
}

/// Identifies the model boundary.
#[must_use]
pub const fn boundary_name() -> &'static str {
    "model-runner"
}
