//! Backend-neutral contract for local model inference.
//!
//! This crate owns the model-runtime vocabulary used by OID clients and
//! managers. Concrete engines, including llama.cpp, must implement this
//! contract in an adapter crate and must not leak engine-native types here.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod structured_output;

pub use structured_output::StructuredOutput;

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
    /// Optional multimodal projector paired with the text model.
    pub vision_projector_path: Option<ModelPath>,
    /// Backend-neutral placement requested for this load.
    pub placement: PlacementRequest,
}

/// Accelerator backend requested or selected for a model.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExecutionBackend {
    /// Let Talvik select an eligible compiled and usable backend.
    #[default]
    Auto,
    /// NVIDIA CUDA.
    Cuda,
    /// Khronos Vulkan.
    Vulkan,
    /// Apple Metal.
    Metal,
    /// Portable CPU execution.
    Cpu,
}

impl ExecutionBackend {
    /// Stable configuration and diagnostics name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cuda => "cuda",
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
            Self::Cpu => "cpu",
        }
    }
}

/// Policy controlling how a model may be placed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlacementPolicy {
    /// Select conservatively from qualified routes and available memory.
    #[default]
    Automatic,
    /// Keep all model layers on the CPU.
    CpuOnly,
    /// Prefer a usable GPU but permit an observable CPU fallback.
    GpuPreferred,
    /// Require successful GPU placement; CPU fallback is forbidden.
    GpuRequired,
}

impl PlacementPolicy {
    /// Stable configuration and diagnostics name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::CpuOnly => "cpu_only",
            Self::GpuPreferred => "gpu_preferred",
            Self::GpuRequired => "gpu_required",
        }
    }
}

/// Placement controls passed through the backend-neutral load boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlacementRequest {
    /// Placement policy.
    pub policy: PlacementPolicy,
    /// Backend preference or requirement.
    pub backend: ExecutionBackend,
    /// Maximum model layers to request for GPU offload. `None` lets the
    /// selected backend choose the maximum supported count.
    pub gpu_layers: Option<u32>,
}

/// Actual and requested placement facts retained with a resident model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelPlacement {
    /// Requested policy.
    pub requested_policy: PlacementPolicy,
    /// Requested backend.
    pub requested_backend: ExecutionBackend,
    /// Backend successfully selected by the native runtime.
    pub selected_backend: ExecutionBackend,
    /// Selected device name, when the native backend reports it.
    pub selected_device: Option<String>,
    /// Requested number of offloaded layers.
    pub gpu_layers_requested: u32,
    /// Actual offloaded layers, only when the native API can verify the value.
    pub gpu_layers_offloaded: Option<u32>,
    /// Whether CPU fallback was applied.
    pub fallback_applied: bool,
    /// Original selection or load failure that caused fallback.
    pub fallback_reason: Option<String>,
}

/// Backend-neutral accelerator capability reported by a native runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceCapability {
    /// Backend family.
    pub backend: ExecutionBackend,
    /// Native device label.
    pub name: String,
    /// Whether this backend is part of the linked native build.
    pub compiled: bool,
    /// Whether initialization and enumeration succeeded.
    pub available: bool,
    /// Free dedicated device memory, when measurable.
    pub free_memory_bytes: Option<u64>,
    /// Total dedicated device memory, when measurable.
    pub total_memory_bytes: Option<u64>,
    /// Whether capacity is shared with host RAM, when the backend can
    /// distinguish the memory pool.
    pub shared_memory: Option<bool>,
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
    /// Host allocation attributable to model weights, when measurable.
    pub host_memory_bytes: Option<u64>,
    /// Device allocation attributable to model weights, when measurable.
    pub gpu_memory_bytes: Option<u64>,
    /// Requested and actual placement facts.
    pub placement: ModelPlacement,
}

/// Minimum generation controls for the first inference slice.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationRequest {
    /// Explicit resident model identity.
    pub model_id: ModelId,
    /// Prompt text.
    pub prompt: String,
    /// Ordered text and image input. When non-empty, this replaces `prompt`.
    pub content: Vec<ContentPart>,
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
    /// Explicit strings that terminate generation without being emitted.
    pub stop_sequences: Vec<String>,
    /// Optional backend-neutral JSON output constraint.
    pub structured_output: Option<StructuredOutput>,
}

/// One ordered input segment for a generation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentPart {
    /// Plain UTF-8 prompt content.
    Text(String),
    /// Encoded image content.
    Image(ImageInput),
}

/// Request-scoped encoded image data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageInput {
    /// Declared MIME type; the backend also verifies the encoded format.
    pub media_type: String,
    /// Encoded image bytes; never persisted by the model runner.
    pub data: Vec<u8>,
}

impl ImageInput {
    /// Maximum encoded image size accepted by the portable contract.
    pub const MAX_ENCODED_BYTES: usize = 10 * 1024 * 1024;
    /// Maximum number of images in one generation request.
    pub const MAX_IMAGES_PER_REQUEST: usize = 4;

    /// Validate the declared format and encoded byte bounds.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` when the MIME type or encoded size is unsupported.
    pub fn validate(&self) -> Result<(), ModelRunnerError> {
        if !matches!(
            self.media_type.as_str(),
            "image/png" | "image/jpeg" | "image/bmp" | "image/gif"
        ) {
            return Err(ModelRunnerError::InvalidRequest(format!(
                "unsupported image media type `{}`",
                self.media_type
            )));
        }
        if self.data.is_empty() || self.data.len() > Self::MAX_ENCODED_BYTES {
            return Err(ModelRunnerError::InvalidRequest(format!(
                "encoded image size must be between 1 and {} bytes",
                Self::MAX_ENCODED_BYTES
            )));
        }
        Ok(())
    }
}

impl GenerationRequest {
    /// Validate generation controls before passing them to a backend.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` when a control is outside the supported range.
    pub fn validate(&self) -> Result<(), ModelRunnerError> {
        if self.prompt.trim().is_empty() && self.content.is_empty() {
            return Err(ModelRunnerError::InvalidRequest(
                "generation input is empty".to_owned(),
            ));
        }
        let mut image_count = 0;
        for part in &self.content {
            match part {
                ContentPart::Text(text) if text.trim().is_empty() => {
                    return Err(ModelRunnerError::InvalidRequest(
                        "text content part is empty".to_owned(),
                    ));
                }
                ContentPart::Text(_) => {}
                ContentPart::Image(image) => {
                    image_count += 1;
                    image.validate()?;
                }
            }
        }
        if image_count > ImageInput::MAX_IMAGES_PER_REQUEST {
            return Err(ModelRunnerError::InvalidRequest(format!(
                "at most {} images are supported per request",
                ImageInput::MAX_IMAGES_PER_REQUEST
            )));
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
        if let Some(structured_output) = &self.structured_output {
            structured_output
                .validate()
                .map_err(ModelRunnerError::InvalidRequest)?;
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

/// Reason a generation reached its successful terminal boundary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GenerationTermination {
    /// The model emitted an end-of-generation token or configured stop marker.
    #[default]
    Stop,
    /// The requested completion token budget was exhausted.
    Length,
    /// The requested context window was exhausted.
    ContextLength,
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
    /// Time spent evaluating the prompt before token generation.
    pub prompt_eval_time: Option<Duration>,
    /// Total generation duration.
    pub generation_time: Option<Duration>,
    /// Generated tokens per second, when calculable.
    pub tokens_per_second: Option<f64>,
    /// Why successful generation terminated.
    pub termination: GenerationTermination,
    /// Whether a compatible prompt prefix was reused.
    pub cache_hit: bool,
    /// Number of prompt tokens reused from the persistent native context.
    pub reused_prompt_tokens: u64,
}

/// Runtime-level model statistics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelRuntimeStats {
    /// One loaded model retained for compatibility with older consumers.
    pub loaded_model: Option<ModelId>,
    /// All resident models, in stable identifier order.
    pub loaded_models: Vec<ModelId>,
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
    /// The backend does not expose the requested capability.
    NotImplemented(String),
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
            Self::NotImplemented(value) => write!(formatter, "capability not implemented: {value}"),
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
    /// Load one local model while preserving other resident models.
    ///
    /// # Errors
    ///
    /// Returns a typed model or backend lifecycle error.
    fn load(&self, request: LoadModelRequest) -> Result<LoadedModel, ModelRunnerError>;
    /// Start streaming generation from the explicitly selected resident model.
    ///
    /// # Errors
    ///
    /// Returns a typed model or backend lifecycle error.
    fn generate(&self, request: GenerationRequest) -> Result<GenerationStream, ModelRunnerError>;

    /// Compute an embedding vector for one resident model.
    ///
    /// Backends that do not expose embeddings return `NotImplemented` rather
    /// than fabricating a vector.
    ///
    /// # Errors
    ///
    /// Returns a typed backend or capability error when embeddings are not
    /// available.
    fn embeddings(
        &self,
        _model_id: &ModelId,
        _text: &str,
        _context_size: u32,
    ) -> Result<Vec<f32>, ModelRunnerError> {
        Err(ModelRunnerError::NotImplemented("embeddings".to_owned()))
    }
    /// Report whether the selected resident model exposes tool-capable chat.
    ///
    /// # Errors
    ///
    /// Returns a typed backend error when capability inspection fails.
    fn tool_support(&self, _model_id: &ModelId) -> Result<bool, ModelRunnerError> {
        Ok(false)
    }
    /// Drop any retained conversation-prefix state for one resident model.
    ///
    /// # Errors
    ///
    /// Returns `NotImplemented` when the backend has no reusable context, or a
    /// lifecycle error when the model cannot be reset safely.
    fn reset_context(&self, _model_id: &ModelId) -> Result<(), ModelRunnerError> {
        Err(ModelRunnerError::NotImplemented(
            "inference context reset".to_owned(),
        ))
    }
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
    /// Inspect the requested and actual placement of one resident model.
    ///
    /// # Errors
    ///
    /// Returns `ModelNotLoaded` when the model is not resident.
    fn placement(&self, _model_id: &ModelId) -> Result<ModelPlacement, ModelRunnerError> {
        Err(ModelRunnerError::NotImplemented(
            "model placement diagnostics".to_owned(),
        ))
    }
    /// Unload the active model safely.
    ///
    /// # Errors
    ///
    /// Returns a typed lifecycle or cleanup error.
    fn unload(&self) -> Result<(), ModelRunnerError>;
    /// Unload one resident model safely.
    ///
    /// # Errors
    ///
    /// Returns a typed lifecycle or cleanup error.
    fn unload_model(&self, model_id: &ModelId) -> Result<(), ModelRunnerError> {
        let _ = model_id;
        self.unload()
    }
}

/// Identifies the model boundary.
#[must_use]
pub const fn boundary_name() -> &'static str {
    "model-runner"
}
