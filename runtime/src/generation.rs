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
}

/// Completed generation result.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationResult {
    /// Request identifier.
    pub request_id: String,
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
    pub(crate) fn new(
        receiver: mpsc::Receiver<GenerationMessage>,
        cancellation: Arc<AtomicBool>,
    ) -> Self {
        Self {
            receiver,
            cancellation,
        }
    }

    /// Receive the next stream item, blocking only the caller that reads it.
    ///
    /// # Errors
    ///
    /// Returns an error when the producer has closed the stream.
    pub fn recv(&self) -> Result<GenerationMessage, mpsc::RecvError> {
        self.receiver.recv()
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
