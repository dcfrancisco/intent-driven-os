//! Backend-neutral generation requests, streams, and metrics.

use oid_shared::RuntimeError;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

/// Sampling and context options for one independent generation.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationOptions {
    /// Sampling temperature.
    pub temperature: f32,
    /// Nucleus sampling probability.
    pub top_p: f32,
    /// Top-K sampling limit.
    pub top_k: i32,
    /// Maximum generated tokens.
    pub max_tokens: u32,
    /// Deterministic sampling seed.
    pub seed: u32,
    /// Requested context size.
    pub context_size: u32,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            temperature: 0.8,
            top_p: 0.95,
            top_k: 40,
            max_tokens: 128,
            seed: 0,
            context_size: 4096,
        }
    }

    impl GenerationOptions {
        /// Validate controls before admission or backend execution.
        ///
        /// # Errors
        ///
        /// Returns a runtime error when a caller supplies an unsafe or unsupported
        /// generation value.
        pub fn validate(&self) -> Result<(), RuntimeError> {
            if self.max_tokens == 0 {
                return Err(RuntimeError::InvalidConfiguration(
                    "max_tokens must be greater than zero".to_owned(),
                ));
            }
            if !self.temperature.is_finite() || self.temperature < 0.0 {
                return Err(RuntimeError::InvalidConfiguration(
                    "temperature must be finite and non-negative".to_owned(),
                ));
            }
            if !self.top_p.is_finite() || self.top_p <= 0.0 || self.top_p > 1.0 {
                return Err(RuntimeError::InvalidConfiguration(
                    "top_p must be greater than zero and at most one".to_owned(),
                ));
            }
            if self.top_k <= 0 {
                return Err(RuntimeError::InvalidConfiguration(
                    "top_k must be greater than zero".to_owned(),
                ));
            }
            if self.context_size == 0 {
                return Err(RuntimeError::InvalidConfiguration(
                    "context_size must be greater than zero".to_owned(),
                ));
            }
            Ok(())
        }
    }
}

/// One independent generation request.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationRequest {
    /// Correlation identifier.
    pub request_id: String,
    /// Loaded model identifier.
    pub model_id: String,
    /// Prompt text.
    pub prompt: String,
    /// Generation options.
    pub options: GenerationOptions,
}

impl GenerationRequest {
    /// Validate request identity, prompt, and generation controls.
    ///
    /// # Errors
    ///
    /// Returns a runtime error when required request data is missing.
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if self.request_id.trim().is_empty() {
            return Err(RuntimeError::InvalidConfiguration(
                "request_id must not be empty".to_owned(),
            ));
        }
        if self.model_id.trim().is_empty() {
            return Err(RuntimeError::InvalidModel(
                "model_id must not be empty".to_owned(),
            ));
        }
        if self.prompt.trim().is_empty() {
            return Err(RuntimeError::InvalidConfiguration(
                "prompt must not be empty".to_owned(),
            ));
        }
        self.options.validate()
    }
}

/// Generation performance measurements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GenerationStatistics {
    /// Number of prompt tokens.
    pub prompt_tokens: u64,
    /// Number of generated tokens.
    pub generated_tokens: u64,
    /// Generated tokens per second.
    pub tokens_per_second: f64,
    /// End-to-end latency in milliseconds.
    pub latency_ms: u128,
    /// Total backend inference time in milliseconds.
    pub inference_time_ms: u128,
    /// Context tokens used.
    pub context_tokens: u64,
    /// Whether the numeric measurements are available and trustworthy.
    /// When false, consumers must display the measurements as unknown.
    pub metrics_known: bool,
    /// Why successful generation terminated.
    pub termination: GenerationTermination,
}

/// Reason a generation reached its successful terminal boundary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GenerationTermination {
    /// The model emitted an end-of-generation token or stop marker.
    #[default]
    Stop,
    /// The requested completion token budget was exhausted.
    Length,
    /// The requested context window was exhausted.
    ContextLength,
}

/// Completed generation result.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationResult {
    /// Request identifier.
    pub request_id: String,
    /// Runtime-owned generation identifier, distinct from the caller request.
    pub generation_id: String,
    /// Complete generated text.
    pub text: String,
    /// Runtime measurements.
    pub statistics: GenerationStatistics,
}

/// One live output item.
#[derive(Clone, Debug, PartialEq)]
pub enum GenerationMessage {
    /// A generated text fragment.
    Token(String),
    /// Final result and measurements.
    Completed(GenerationResult),
    /// Generation was cancelled after returning any partial output.
    Cancelled(GenerationResult),
    /// Generation failed.
    Failed(RuntimeError),
}

/// Non-blocking stream returned to a console or service client.
#[derive(Debug)]
pub struct GenerationStream {
    receiver: mpsc::Receiver<GenerationMessage>,
    cancellation: Arc<AtomicBool>,
}

impl GenerationStream {
    /// Construct a stream for a transport adapter that owns the producer.
    pub fn from_parts(
        receiver: mpsc::Receiver<GenerationMessage>,
        cancellation: Arc<AtomicBool>,
    ) -> Self {
        Self {
            receiver,
            cancellation,
        }
    }

    pub(crate) fn new(
        receiver: mpsc::Receiver<GenerationMessage>,
        cancellation: Arc<AtomicBool>,
    ) -> Self {
        Self::from_parts(receiver, cancellation)
    }

    /// Receive the next stream item, blocking only the caller that reads it.
    ///
    /// # Errors
    ///
    /// Returns an error when the producer has closed the stream.
    pub fn recv(&self) -> Result<GenerationMessage, mpsc::RecvError> {
        self.receiver.recv()
    }

    /// Receive the next stream item with a bounded wait.
    ///
    /// # Errors
    ///
    /// Returns a timeout or channel-disconnect error.
    pub fn recv_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<GenerationMessage, mpsc::RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }

    /// Attempt to receive without blocking.
    ///
    /// # Errors
    ///
    /// Returns an error when no item is available or the producer has closed.
    pub fn try_recv(&self) -> Result<GenerationMessage, mpsc::TryRecvError> {
        self.receiver.try_recv()
    }

    /// Cancel generation without terminating the process.
    pub fn cancel(&self) {
        self.cancellation.store(true, Ordering::SeqCst);
    }
}
