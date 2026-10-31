//! Backend-neutral facts and supported output formats for a loaded model.

/// Safe context reuse supported by the selected backend and architecture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheStrategy {
    /// Context reuse is unavailable.
    Disabled,
    /// Attention state supports verified sequence trimming.
    Attention,
    /// Recurrent state requires bounded snapshots and verified restoration.
    Recurrent,
}

/// Tool output syntax that the serving edge can validate and parse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolFormat {
    /// JSON function calls enclosed in XML-style `tool_call` markers.
    XmlJson,
    /// Function/parameter XML blocks inside `tool_call` markers.
    XmlFunction,
}

/// Reasoning output syntax that can be separated from answer text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReasoningFormat {
    /// A leading think section with an explicit closing marker.
    ThinkTags,
}

/// Immutable, observed model facts plus implemented serving capabilities.
///
/// Template declarations alone do not establish arbitrary parser support.
/// This is backend support information, not a claim of model answer quality.
// Capability facts are independent, not mutually exclusive lifecycle states.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelProfile {
    /// Architecture read from the artifact, independent of its filename.
    pub architecture: String,
    /// Training context limit reported by the engine (not available RAM).
    pub context_length: u32,
    /// Whether the loaded engine has a decoder for text generation.
    pub completion: bool,
    /// Whether its selected template can render a user conversation.
    pub chat: bool,
    /// Whether this adapter implements the artifact's embedding mode.
    pub embedding: bool,
    /// Whether the artifact declares embedding pooling, including unavailable modes.
    pub embedding_declared: bool,
    /// Whether an initialized matching vision projector supports images.
    pub vision: bool,
    /// Context strategy selected from the actual architecture/backend.
    pub cache: CacheStrategy,
    /// Whether the tokenizer declares automatic BOS insertion.
    pub tokenizer_adds_bos: bool,
    /// Whether the template declares tool handling (which may be unsupported).
    pub tools_declared: bool,
    /// Implemented compatible tool syntax, or no usable tool parser.
    pub tool_format: Option<ToolFormat>,
    /// Implemented compatible reasoning syntax, if recognized.
    pub reasoning_format: Option<ReasoningFormat>,
    /// Whether the current template adapter implements a thinking switch.
    pub thinking_control: bool,
    /// Whether the template already opens reasoning before generation starts.
    pub reasoning_prefilled: bool,
}
