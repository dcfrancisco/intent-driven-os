//! Minimal safe wrapper around the official llama.cpp C API.
//!
//! Native linking is opt-in through `LLAMA_CPP_LIB_DIR`. Without that
//! variable the wrapper remains buildable and reports an unavailable engine,
//! which keeps documentation and CI builds independent of a local toolchain.

#![allow(unsafe_code)]
#![warn(missing_docs)]

#[cfg(not(native_llama_cpp))]
use std::ffi::c_void;
#[cfg(native_llama_cpp)]
use std::ffi::{c_char, c_void, CStr, CString};
#[cfg(native_llama_cpp)]
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicPtr};
use std::sync::Mutex;
use std::time::Duration;
#[cfg(native_llama_cpp)]
use std::time::Instant;

#[cfg(any(native_llama_cpp, test))]
const LLAMA_DECODE_ABORTED: i32 = 2;

#[cfg(any(native_llama_cpp, test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodeOutcome {
    Success,
    Cancelled,
    NativeFailure,
}

#[cfg(any(native_llama_cpp, test))]
fn classify_decode_result(result: i32, cancelled: bool) -> DecodeOutcome {
    if result == 0 {
        DecodeOutcome::Success
    } else if result == LLAMA_DECODE_ABORTED && cancelled {
        DecodeOutcome::Cancelled
    } else {
        DecodeOutcome::NativeFailure
    }
}

#[cfg(any(native_llama_cpp, test))]
fn decode_error(stage: &str, result: i32) -> String {
    if result == LLAMA_DECODE_ABORTED {
        format!("llama.cpp {stage} decode aborted without an active Marina cancellation: {result}")
    } else {
        format!("llama.cpp {stage} decode failed: {result}")
    }
}

#[cfg(native_llama_cpp)]
#[repr(C)]
#[derive(Clone, Copy)]
struct ModelParams {
    devices: *mut *mut c_void,
    tensor_buft_overrides: *const c_void,
    n_gpu_layers: i32,
    split_mode: i32,
    load_mode: i32,
    tensor_read_lazy: i32,
    main_gpu: i32,
    tensor_split: *const f32,
    progress_callback: Option<unsafe extern "C" fn(f32, *mut c_void) -> bool>,
    progress_callback_user_data: *mut c_void,
    kv_overrides: *const c_void,
    vocab_only: bool,
    check_tensors: bool,
    use_extra_bufts: bool,
    no_host: bool,
    no_alloc: bool,
    load_mtp: bool,
}

#[cfg(native_llama_cpp)]
#[repr(C)]
struct ChatMessage {
    role: *const c_char,
    content: *const c_char,
}

#[cfg(native_llama_cpp)]
#[repr(C)]
#[derive(Clone, Copy)]
struct ContextParams {
    n_ctx: u32,
    n_batch: u32,
    n_ubatch: u32,
    n_seq_max: u32,
    n_rs_seq: u32,
    n_outputs_max: u32,
    n_outputs_max_per_seq: u32,
    n_threads: i32,
    n_threads_batch: i32,
    ctx_type: i32,
    rope_scaling_type: i32,
    pooling_type: i32,
    attention_type: i32,
    flash_attn_type: i32,
    rope_freq_base: f32,
    rope_freq_scale: f32,
    yarn_ext_factor: f32,
    yarn_attn_factor: f32,
    yarn_beta_fast: f32,
    yarn_beta_slow: f32,
    yarn_orig_ctx: u32,
    defrag_thold: f32,
    cb_eval: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool>,
    cb_eval_user_data: *mut c_void,
    type_k: i32,
    type_v: i32,
    abort_callback: Option<unsafe extern "C" fn(*mut c_void) -> bool>,
    abort_callback_data: *mut c_void,
    embeddings: bool,
    offload_kqv: bool,
    no_perf: bool,
    op_offload: bool,
    swa_full: bool,
    kv_unified: bool,
    samplers: *mut c_void,
    n_samplers: usize,
    ctx_other: *mut c_void,
}

#[cfg(native_llama_cpp)]
#[repr(C)]
#[derive(Clone, Copy)]
struct Batch {
    n_tokens: i32,
    token: *mut i32,
    embd: *mut f32,
    pos: *mut i32,
    n_seq_id: *mut i32,
    seq_id: *mut *mut i32,
    logits: *mut i8,
}

#[cfg(native_llama_cpp)]
#[derive(Debug)]
struct PersistentContext {
    pointer: *mut c_void,
    context_size: u32,
    prompt_tokens: Vec<i32>,
    relay: Box<CancellationRelay>,
}

#[cfg(native_llama_cpp)]
unsafe impl Send for PersistentContext {}

#[cfg(native_llama_cpp)]
#[derive(Debug)]
struct CancellationRelay {
    current: AtomicPtr<AtomicBool>,
}

#[cfg(native_llama_cpp)]
struct CancellationGuard(*const CancellationRelay);

#[cfg(native_llama_cpp)]
impl Drop for CancellationGuard {
    fn drop(&mut self) {
        unsafe {
            (*self.0)
                .current
                .store(std::ptr::null_mut(), Ordering::SeqCst);
        }
    }
}

#[cfg(native_llama_cpp)]
impl Drop for PersistentContext {
    fn drop(&mut self) {
        unsafe {
            ffi::llama_free(self.pointer);
        }
    }
}

#[cfg(native_llama_cpp)]
#[repr(C)]
#[derive(Clone, Copy)]
struct SamplerChainParams {
    no_perf: bool,
}

#[cfg(native_llama_cpp)]
unsafe extern "C" fn abort_callback(data: *mut c_void) -> bool {
    if data.is_null() {
        return false;
    }
    let relay = &*data.cast::<CancellationRelay>();
    let current = relay.current.load(Ordering::SeqCst);
    !current.is_null() && (*current).load(Ordering::SeqCst)
}

#[cfg(native_llama_cpp)]
mod ffi {
    use super::{
        c_char, c_void, Batch, ChatMessage, ContextParams, ModelParams, SamplerChainParams,
    };

    #[allow(improper_ctypes)]
    extern "C" {
        pub fn llama_backend_init();
        pub fn llama_backend_free();
        pub fn llama_print_system_info() -> *const c_char;
        pub fn llama_model_default_params() -> ModelParams;
        pub fn llama_model_load_from_file(path: *const c_char, params: ModelParams) -> *mut c_void;
        pub fn llama_model_free(model: *mut c_void);
        pub fn llama_model_get_vocab(model: *const c_void) -> *const c_void;
        pub fn llama_model_chat_template(
            model: *const c_void,
            name: *const c_char,
        ) -> *const c_char;
        pub fn llama_model_size(model: *const c_void) -> u64;
        pub fn llama_model_n_embd(model: *const c_void) -> i32;
        pub fn llama_get_embeddings(ctx: *mut c_void) -> *const f32;
        pub fn llama_chat_apply_template(
            template: *const c_char,
            chat: *const ChatMessage,
            n_msg: usize,
            add_ass: bool,
            buffer: *mut c_char,
            buffer_size: i32,
        ) -> i32;
        pub fn llama_context_default_params() -> ContextParams;
        pub fn llama_init_from_model(model: *mut c_void, params: ContextParams) -> *mut c_void;
        pub fn llama_free(ctx: *mut c_void);
        pub fn llama_get_memory(ctx: *mut c_void) -> *mut c_void;
        pub fn llama_memory_clear(memory: *mut c_void, data: bool);
        pub fn llama_memory_seq_rm(memory: *mut c_void, seq_id: i32, p0: i32, p1: i32) -> bool;
        pub fn llama_memory_seq_pos_max(memory: *mut c_void, seq_id: i32) -> i32;
        pub fn llama_batch_get_one(tokens: *mut i32, n_tokens: i32) -> Batch;
        pub fn llama_decode(ctx: *mut c_void, batch: Batch) -> i32;
        pub fn llama_sampler_chain_default_params() -> SamplerChainParams;
        pub fn llama_sampler_chain_init(params: SamplerChainParams) -> *mut c_void;
        pub fn llama_sampler_chain_add(chain: *mut c_void, sampler: *mut c_void);
        pub fn llama_sampler_init_grammar(
            vocab: *const c_void,
            grammar_str: *const c_char,
            grammar_root: *const c_char,
        ) -> *mut c_void;
        pub fn llama_sampler_init_top_k(k: i32) -> *mut c_void;
        pub fn llama_sampler_init_top_p(p: f32, min_keep: usize) -> *mut c_void;
        pub fn llama_sampler_init_temp(t: f32) -> *mut c_void;
        pub fn llama_sampler_init_penalties(
            penalty_last_n: i32,
            penalty_repeat: f32,
            penalty_freq: f32,
            penalty_present: f32,
        ) -> *mut c_void;
        pub fn llama_sampler_init_dist(seed: u32) -> *mut c_void;
        pub fn llama_sampler_sample(sampler: *mut c_void, ctx: *mut c_void, idx: i32) -> i32;
        pub fn llama_sampler_free(sampler: *mut c_void);
        pub fn llama_vocab_is_eog(vocab: *const c_void, token: i32) -> bool;
        pub fn llama_token_to_piece(
            vocab: *const c_void,
            token: i32,
            text: *mut c_char,
            text_len_max: i32,
            lstrip: i32,
            special: bool,
        ) -> i32;
        pub fn llama_tokenize(
            vocab: *const c_void,
            text: *const c_char,
            text_len: i32,
            tokens: *mut i32,
            max_tokens: i32,
            add_special: bool,
            parse_special: bool,
        ) -> i32;
    }
}

/// Whether this build has been configured to link the native library.
#[must_use]
pub const fn is_available() -> bool {
    cfg!(native_llama_cpp)
}

/// Initialize the process-wide llama.cpp backend.
///
/// # Errors
///
/// Returns an explanatory error when native linking was not configured or the
/// native engine rejected initialization.
pub fn initialize() -> Result<(), String> {
    #[cfg(native_llama_cpp)]
    unsafe {
        ffi::llama_backend_init();
        Ok(())
    }
    #[cfg(not(native_llama_cpp))]
    {
        Err("set LLAMA_CPP_LIB_DIR to a directory containing libllama".to_owned())
    }
}

/// Release process-wide llama.cpp resources.
pub fn shutdown() {
    #[cfg(native_llama_cpp)]
    unsafe {
        ffi::llama_backend_free();
    }
}

#[cfg(native_llama_cpp)]
struct OwnedBatch {
    batch: Batch,
    _positions: Vec<i32>,
    _sequence_counts: Vec<i32>,
    _sequence_id: Box<i32>,
    _sequence_ids: Vec<*mut i32>,
    _logits: Vec<i8>,
}

#[cfg(native_llama_cpp)]
fn make_batch(tokens: &mut [i32], start_position: usize) -> Result<OwnedBatch, String> {
    let count = i32::try_from(tokens.len()).map_err(|_| "batch is too large".to_owned())?;
    let positions = (0..tokens.len())
        .map(|index| {
            i32::try_from(start_position + index).map_err(|_| "position is too large".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut sequence_counts = vec![1_i32; tokens.len()];
    let sequence_id = Box::new(0_i32);
    let sequence_ids = vec![std::ptr::from_ref(&*sequence_id).cast_mut(); tokens.len()];
    let mut logits = vec![0_i8; tokens.len()];
    if let Some(last) = logits.last_mut() {
        *last = 1;
    }
    // Keep the backing allocations alive for the duration of llama_decode by
    // storing them in a thread-local scratch object is unnecessary: llama.cpp
    // consumes this batch synchronously. The API only retains the pointers
    // during the call.
    let batch = Batch {
        n_tokens: count,
        token: tokens.as_mut_ptr(),
        embd: std::ptr::null_mut(),
        pos: positions.as_ptr().cast_mut(),
        n_seq_id: sequence_counts.as_mut_ptr(),
        seq_id: sequence_ids.as_ptr().cast_mut(),
        logits: logits.as_mut_ptr(),
    };
    Ok(OwnedBatch {
        batch,
        _positions: positions,
        _sequence_counts: sequence_counts,
        _sequence_id: sequence_id,
        _sequence_ids: sequence_ids,
        _logits: logits,
    })
}

#[cfg(native_llama_cpp)]
fn invalidate_context(context: &mut Option<PersistentContext>) {
    if let Some(cached) = context.as_mut() {
        let memory = unsafe { ffi::llama_get_memory(cached.pointer) };
        if !memory.is_null() {
            unsafe { ffi::llama_memory_clear(memory, true) };
        }
        cached.prompt_tokens.clear();
    }
}

#[cfg(native_llama_cpp)]
fn inference_threads() -> i32 {
    std::env::var("TALVIK_THREADS")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|count| i32::try_from(count.get()).unwrap_or(i32::MAX))
                .unwrap_or(0)
        })
}

#[cfg(native_llama_cpp)]
fn compatible_prompt_reuse_len(
    cached: &PersistentContext,
    prompt_tokens: &[i32],
    context_size: u32,
) -> Option<usize> {
    (cached.context_size == context_size
        && cached.prompt_tokens.len() <= prompt_tokens.len()
        && cached.prompt_tokens == prompt_tokens[..cached.prompt_tokens.len()])
        .then(|| cached.prompt_tokens.len().saturating_sub(1))
}

fn cached_prefix_matches(reuse_len: usize, actual_max_position: i32) -> bool {
    reuse_len > 0
        && i32::try_from(reuse_len)
            .ok()
            .and_then(|position| position.checked_sub(1))
            == Some(actual_max_position)
}

/// Return official llama.cpp system/build information.
#[must_use]
pub fn system_info() -> Option<String> {
    #[cfg(native_llama_cpp)]
    unsafe {
        let pointer = ffi::llama_print_system_info();
        (!pointer.is_null()).then(|| CStr::from_ptr(pointer).to_string_lossy().into_owned())
    }
    #[cfg(not(native_llama_cpp))]
    {
        None
    }
}

/// A loaded native model handle.
#[derive(Debug)]
pub struct NativeModel {
    #[cfg(native_llama_cpp)]
    context: Mutex<Option<PersistentContext>>,
    #[cfg_attr(not(native_llama_cpp), allow(dead_code))]
    pointer: *mut c_void,
    memory_bytes: u64,
}

/// Native generation measurements.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeGenerationStatistics {
    /// Prompt token count.
    pub prompt_tokens: u64,
    /// Generated token count.
    pub generated_tokens: u64,
    /// Context tokens consumed.
    pub context_tokens: u64,
    /// Time spent evaluating prompt tokens.
    pub prompt_eval_time: Duration,
    /// Total native inference time.
    pub generation_time: Duration,
    /// Why successful generation terminated.
    pub termination: NativeGenerationTermination,
    /// Whether a compatible prompt prefix was reused.
    pub cache_hit: bool,
    /// Number of prompt tokens reused from the persistent context.
    pub reused_prompt_tokens: u64,
}

/// Native reason a generation reached its successful terminal boundary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NativeGenerationTermination {
    /// The model emitted an end-of-generation token or stop marker.
    #[default]
    Stop,
    /// The requested completion token budget was exhausted.
    Length,
    /// The requested context window was exhausted.
    ContextLength,
}

#[cfg(native_llama_cpp)]
fn take_complete_utf8(buffer: &mut Vec<u8>) -> String {
    match std::str::from_utf8(buffer) {
        Ok(text) => {
            let text = text.to_owned();
            buffer.clear();
            text
        }
        Err(error) if error.error_len().is_none() => {
            let valid_length = error.valid_up_to();
            let text = String::from_utf8_lossy(&buffer[..valid_length]).into_owned();
            buffer.drain(..valid_length);
            text
        }
        Err(_) => {
            let text = String::from_utf8_lossy(buffer).into_owned();
            buffer.clear();
            text
        }
    }
}

// The handle is only accessed while held by the adapter mutex.
unsafe impl Send for NativeModel {}

impl NativeModel {
    /// Report whether the embedded chat template advertises tool handling.
    #[must_use]
    pub fn supports_tools(&self) -> bool {
        #[cfg(native_llama_cpp)]
        unsafe {
            let template =
                ffi::llama_model_chat_template(self.pointer.cast_const(), std::ptr::null());
            if template.is_null() {
                return false;
            }
            let template = CStr::from_ptr(template).to_string_lossy();
            template.contains("custom_tools") || template.contains("tool_calls")
        }
        #[cfg(not(native_llama_cpp))]
        {
            false
        }
    }

    /// Model memory reported by llama.cpp.
    #[must_use]
    pub const fn memory_bytes(&self) -> u64 {
        self.memory_bytes
    }

    /// Drop the retained inference context and its KV cache for this model.
    ///
    /// # Errors
    ///
    /// Returns an error when the context state lock is poisoned.
    pub fn reset_context(&self) -> Result<(), String> {
        #[cfg(native_llama_cpp)]
        {
            let mut context = self
                .context
                .lock()
                .map_err(|_| "native context lock poisoned".to_owned())?;
            context.take();
            Ok(())
        }
        #[cfg(not(native_llama_cpp))]
        {
            Err("native llama.cpp is unavailable".to_owned())
        }
    }

    /// Tokenize UTF-8 text using the loaded model vocabulary.
    ///
    /// # Errors
    ///
    /// Returns an error when the text cannot be represented for the C API or
    /// llama.cpp reports a tokenization failure.
    pub fn tokenize(&self, text: &str) -> Result<Vec<i32>, String> {
        #[cfg(native_llama_cpp)]
        unsafe {
            let vocabulary = ffi::llama_model_get_vocab(self.pointer.cast_const());
            let input = CString::new(text).map_err(|_| "text contains NUL".to_owned())?;
            let length = i32::try_from(text.len()).map_err(|_| "text is too long".to_owned())?;
            let required = ffi::llama_tokenize(
                vocabulary,
                input.as_ptr(),
                length,
                std::ptr::null_mut(),
                0,
                false,
                false,
            );
            if required >= 0 {
                return Err("llama.cpp returned no token count".to_owned());
            }
            let capacity = required
                .checked_neg()
                .ok_or_else(|| "token count overflow".to_owned())?;
            let mut tokens =
                vec![0; usize::try_from(capacity).map_err(|_| "token count overflow".to_owned())?];
            let count = ffi::llama_tokenize(
                vocabulary,
                input.as_ptr(),
                length,
                tokens.as_mut_ptr(),
                capacity,
                false,
                false,
            );
            if count < 0 {
                return Err("llama.cpp tokenization failed".to_owned());
            }
            tokens.truncate(usize::try_from(count).map_err(|_| "token count overflow".to_owned())?);
            Ok(tokens)
        }
        #[cfg(not(native_llama_cpp))]
        {
            let _ = text;
            Err("native llama.cpp is unavailable".to_owned())
        }
    }

    /// Compute the model embedding for one input string.
    ///
    /// # Errors
    ///
    /// Returns an error when the embedding context cannot be created or the
    /// native engine cannot produce an embedding vector.
    pub fn embedding(&self, text: &str, context_size: u32) -> Result<Vec<f32>, String> {
        #[cfg(native_llama_cpp)]
        unsafe {
            let mut tokens = self.tokenize(text)?;
            if tokens.is_empty() {
                return Err("embedding input produced no tokens".to_owned());
            }
            let mut params = ffi::llama_context_default_params();
            let token_count = u32::try_from(tokens.len())
                .map_err(|_| "embedding input is too large".to_owned())?;
            params.n_ctx = context_size.max(token_count);
            params.n_batch = params.n_ctx.min(512);
            params.n_ubatch = params.n_batch;
            params.embeddings = true;
            let context = ffi::llama_init_from_model(self.pointer, params);
            if context.is_null() {
                return Err("llama.cpp could not create an embedding context".to_owned());
            }
            let result = ffi::llama_decode(
                context,
                ffi::llama_batch_get_one(
                    tokens.as_mut_ptr(),
                    i32::try_from(tokens.len()).map_err(|_| "embedding input is too large")?,
                ),
            );
            if result != 0 {
                ffi::llama_free(context);
                return Err(format!("llama.cpp embedding decode failed: {result}"));
            }
            let dimension = ffi::llama_model_n_embd(self.pointer.cast_const());
            let values = ffi::llama_get_embeddings(context);
            if dimension <= 0 || values.is_null() {
                ffi::llama_free(context);
                return Err("llama.cpp did not return an embedding vector".to_owned());
            }
            let dimension = usize::try_from(dimension)
                .map_err(|_| "llama.cpp returned an invalid embedding dimension")?;
            let vector = std::slice::from_raw_parts(values, dimension).to_vec();
            ffi::llama_free(context);
            Ok(vector)
        }
        #[cfg(not(native_llama_cpp))]
        {
            let _ = (text, context_size);
            Err("native llama.cpp is unavailable".to_owned())
        }
    }

    #[cfg(native_llama_cpp)]
    fn format_prompt(&self, prompt: &str) -> Result<String, String> {
        unsafe {
            let template =
                ffi::llama_model_chat_template(self.pointer.cast_const(), std::ptr::null());
            if template.is_null() {
                return Ok(prompt.to_owned());
            }
            // HTTP multi-turn requests use a private record separator so the
            // backend-neutral public contract remains plain text while the
            // native adapter can still pass distinct role/content messages to
            // the embedded GGUF chat template.
            let parsed = prompt
                .strip_prefix('\u{001e}')
                .map(|conversation| {
                    conversation
                        .split('\u{001e}')
                        .filter(|record| !record.is_empty())
                        .map(|record| {
                            record
                                .split_once('\n')
                                .ok_or_else(|| "invalid structured chat prompt".to_owned())
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;
            let entries = parsed.unwrap_or_else(|| vec![("user", prompt)]);
            let entries = entries
                .into_iter()
                .map(|(role, content)| {
                    let entry = if role == "__talvik_tools" {
                        (
                            "system".to_owned(),
                            format!(
                                "You may call one or more of these tools. When the user asks for a tool, do not answer in prose and do not invent results. Emit each call as <tool_call>{{\"name\":\"tool_name\",\"arguments\":{{}}}}</tool_call>. The arguments must be valid JSON. Tools: {content}"
                            ),
                        )
                    } else if role == "__talvik_tool_choice" {
                        let instruction = match content {
                            "single" => {
                                "Do not make more than one tool call in this response."
                                    .to_owned()
                            }
                            "required" => {
                                "A tool call is required for this response. You must call one of the listed tools and must not answer in prose."
                                    .to_owned()
                            }
                            name if name.starts_with("function:") => format!(
                                "You must call the listed function {:?} exactly once and must not call another function or answer in prose.",
                                &name["function:".len()..]
                            ),
                            _ => return Err("invalid tool-choice prompt directive".to_owned()),
                        };
                        ("system".to_owned(), instruction)
                    } else {
                        (role.to_owned(), content.to_owned())
                    };
                    Ok(entry)
                })
                .collect::<Result<Vec<_>, String>>()?;
            let roles = entries
                .iter()
                .map(|(role, _)| {
                    CString::new(role.as_str()).map_err(|_| "invalid chat role".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let contents = entries
                .iter()
                .map(|(_, content)| {
                    CString::new(content.as_str()).map_err(|_| "prompt contains NUL".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let messages = roles
                .iter()
                .zip(contents.iter())
                .map(|(role, content)| ChatMessage {
                    role: role.as_ptr(),
                    content: content.as_ptr(),
                })
                .collect::<Vec<_>>();
            let required = ffi::llama_chat_apply_template(
                template,
                messages.as_ptr(),
                messages.len(),
                true,
                std::ptr::null_mut(),
                0,
            );
            if required < 0 {
                return Err(format!(
                    "llama.cpp could not apply the model chat template: {required}"
                ));
            }
            let capacity = required
                .checked_add(1)
                .ok_or_else(|| "formatted prompt is too large".to_owned())?;
            let mut buffer = vec![
                0_i8;
                usize::try_from(capacity)
                    .map_err(|_| "formatted prompt is too large".to_owned())?
            ];
            let written = ffi::llama_chat_apply_template(
                template,
                messages.as_ptr(),
                messages.len(),
                true,
                buffer.as_mut_ptr(),
                capacity,
            );
            if written < 0 {
                return Err(format!(
                    "llama.cpp could not format the model chat template: {written}"
                ));
            }
            let bytes = std::slice::from_raw_parts(
                buffer.as_ptr().cast::<u8>(),
                usize::try_from(written).map_err(|_| "formatted prompt is too large".to_owned())?,
            );
            String::from_utf8(bytes.to_vec())
                .map_err(|_| "llama.cpp produced an invalid UTF-8 chat prompt".to_owned())
        }
    }

    #[cfg(native_llama_cpp)]
    unsafe fn token_piece(vocabulary: *const c_void, token: i32) -> Result<Vec<u8>, String> {
        let mut capacity = 64_i32;
        loop {
            let mut text = vec![
                0_i8;
                usize::try_from(capacity)
                    .map_err(|_| "token text is too large".to_owned())?
            ];
            let length =
                ffi::llama_token_to_piece(vocabulary, token, text.as_mut_ptr(), capacity, 0, false);
            if length < 0 {
                capacity = length
                    .checked_neg()
                    .ok_or_else(|| "detokenized text length overflow".to_owned())?;
                continue;
            }
            return Ok(std::slice::from_raw_parts(
                text.as_ptr().cast::<u8>(),
                usize::try_from(length).map_err(|_| "invalid detokenized length".to_owned())?,
            )
            .to_vec());
        }
    }

    #[cfg(native_llama_cpp)]
    fn terminal_piece_prefix(piece: &[u8]) -> (&[u8], bool) {
        // Some llama.cpp versions detokenize a special end token without its
        // angle brackets (for example `|im_end|`). Treat both spellings as
        // terminal markers so control text never reaches the caller.
        const TERMINAL_MARKERS: [&[u8]; 6] = [
            b"<|im_end|>",
            b"</|im_end|}",
            b"|im_end|",
            b"<|endoftext|>",
            b"|endoftext|",
            b"</s>",
        ];
        TERMINAL_MARKERS
            .iter()
            .find_map(|marker| {
                if let Some(index) = piece
                    .windows(marker.len())
                    .position(|window| window == *marker)
                {
                    return Some((&piece[..index], true));
                }
                None
            })
            .unwrap_or((piece, false))
    }

    /// Generate tokens and invoke the callback for every token piece.
    ///
    /// # Errors
    ///
    /// Returns an error when context creation, decoding, sampling, or native
    /// linking fails.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines, unused_mut)]
    pub fn generate_stream<F>(
        &self,
        prompt: &str,
        context_size: u32,
        max_tokens: u32,
        temperature: f32,
        top_p: f32,
        top_k: i32,
        seed: u32,
        stop_sequences: &[String],
        callback: F,
        cancelled: &AtomicBool,
    ) -> Result<NativeGenerationStatistics, String>
    where
        F: FnMut(&str) -> bool,
    {
        self.generate_stream_with_grammar(
            prompt,
            context_size,
            max_tokens,
            temperature,
            top_p,
            top_k,
            seed,
            stop_sequences,
            None,
            callback,
            cancelled,
        )
    }

    /// Generate tokens under an optional native GBNF decoding constraint.
    ///
    /// # Errors
    ///
    /// Returns an error when context creation, decoding, sampling, or native
    /// linking fails.
    pub fn generate_stream_with_grammar<F>(
        &self,
        prompt: &str,
        context_size: u32,
        max_tokens: u32,
        temperature: f32,
        top_p: f32,
        top_k: i32,
        seed: u32,
        stop_sequences: &[String],
        grammar: Option<&str>,
        mut callback: F,
        cancelled: &AtomicBool,
    ) -> Result<NativeGenerationStatistics, String>
    where
        F: FnMut(&str) -> bool,
    {
        #[cfg(not(native_llama_cpp))]
        {
            let _ = (
                prompt,
                context_size,
                max_tokens,
                temperature,
                top_p,
                top_k,
                seed,
                stop_sequences,
                grammar,
                callback,
                cancelled,
            );
            Err("native llama.cpp is unavailable".to_owned())
        }
        #[cfg(native_llama_cpp)]
        {
            let inference_started = Instant::now();
            let formatted_prompt = self.format_prompt(prompt)?;
            let mut prompt_tokens = self.tokenize(&formatted_prompt)?;
            let prompt_count = prompt_tokens.len() as u64;
            let max_context =
                usize::try_from(context_size).map_err(|_| "context too large".to_owned())?;
            if prompt_tokens.len() >= max_context {
                return Err("prompt exceeds context size".to_owned());
            }
            let physical_batch = context_size.min(512);
            let mut persistent = self
                .context
                .lock()
                .map_err(|_| "native context lock poisoned".to_owned())?;
            let mut cache_hit = false;
            let mut reused_prompt_tokens = 0_u64;
            let reuse_len = persistent.as_ref().and_then(|cached| {
                compatible_prompt_reuse_len(cached, &prompt_tokens, context_size)
            });
            let (context, mut prompt_position) = if let Some(reuse_len) = reuse_len {
                let cached = persistent
                    .as_mut()
                    .ok_or_else(|| "native context cache disappeared".to_owned())?;
                cached
                    .relay
                    .current
                    .store(std::ptr::from_ref(cancelled).cast_mut(), Ordering::SeqCst);
                let memory = unsafe { ffi::llama_get_memory(cached.pointer) };
                let removed = !memory.is_null()
                    && unsafe {
                        ffi::llama_memory_seq_rm(
                            memory,
                            0,
                            i32::try_from(reuse_len)
                                .map_err(|_| "cached prompt is too long".to_owned())?,
                            -1,
                        )
                    };
                let actual_max_position = if memory.is_null() {
                    -1
                } else {
                    unsafe { ffi::llama_memory_seq_pos_max(memory, 0) }
                };
                if removed && cached_prefix_matches(reuse_len, actual_max_position) {
                    cache_hit = true;
                    reused_prompt_tokens = reuse_len as u64;
                    (cached.pointer, reuse_len)
                } else {
                    eprintln!(
                        "llama.cpp prompt cache trim was incomplete; expected max position {}, observed {}; clearing KV cache before decoding",
                        i32::try_from(reuse_len)
                            .ok()
                            .and_then(|position| position.checked_sub(1))
                            .map_or_else(|| "none".to_owned(), |position| position.to_string()),
                        actual_max_position
                    );
                    if !memory.is_null() {
                        unsafe { ffi::llama_memory_clear(memory, true) };
                    }
                    cached.prompt_tokens.clear();
                    (cached.pointer, 0)
                }
            } else {
                if let Some(cached) = persistent.take() {
                    drop(cached);
                }
                let mut context_params = ContextParams {
                    n_ctx: context_size,
                    // Keep graph reservation bounded. Prompts larger than this
                    // physical batch are decoded incrementally below.
                    n_batch: physical_batch,
                    n_ubatch: physical_batch,
                    n_threads: inference_threads(),
                    n_threads_batch: inference_threads(),
                    ..unsafe { ffi::llama_context_default_params() }
                };
                let relay = Box::new(CancellationRelay {
                    current: AtomicPtr::new(std::ptr::from_ref(cancelled).cast_mut()),
                });
                context_params.abort_callback = Some(abort_callback);
                context_params.abort_callback_data = std::ptr::from_ref(&*relay).cast_mut().cast();
                let pointer = unsafe { ffi::llama_init_from_model(self.pointer, context_params) };
                if pointer.is_null() {
                    return Err("llama.cpp could not create a context".to_owned());
                }
                persistent.replace(PersistentContext {
                    pointer,
                    context_size,
                    prompt_tokens: Vec::new(),
                    relay,
                });
                (pointer, 0)
            };
            let relay = persistent
                .as_ref()
                .ok_or_else(|| "native context cache disappeared".to_owned())?
                .relay
                .as_ref();
            let _cancellation_guard = CancellationGuard(std::ptr::from_ref(relay));
            let grammar_source = grammar
                .map(CString::new)
                .transpose()
                .map_err(|_| "structured-output grammar contains NUL".to_owned())?;
            let grammar_root = CString::new("root").expect("static grammar root has no NUL");
            let sampler_params = unsafe { ffi::llama_sampler_chain_default_params() };
            let sampler = unsafe { ffi::llama_sampler_chain_init(sampler_params) };
            if sampler.is_null() {
                invalidate_context(&mut persistent);
                return Err("llama.cpp could not create a sampler".to_owned());
            }
            let mut samplers = Vec::with_capacity(6);
            if let Some(grammar_source) = grammar_source.as_ref() {
                let vocabulary = unsafe { ffi::llama_model_get_vocab(self.pointer.cast_const()) };
                samplers.push(unsafe {
                    ffi::llama_sampler_init_grammar(
                        vocabulary,
                        grammar_source.as_ptr(),
                        grammar_root.as_ptr(),
                    )
                });
            }
            samplers.extend(unsafe {
                [
                    ffi::llama_sampler_init_top_k(top_k),
                    ffi::llama_sampler_init_top_p(top_p, 1),
                    // Match the practical local-runner default: discourage
                    // runaway repeated fragments while leaving frequency and
                    // presence penalties disabled unless they become part of
                    // the public request contract.
                    ffi::llama_sampler_init_penalties(64, 1.1, 0.0, 0.0),
                    ffi::llama_sampler_init_temp(temperature),
                    ffi::llama_sampler_init_dist(seed),
                ]
            });
            if samplers.iter().any(|sampler| sampler.is_null()) {
                unsafe {
                    for sampler in samplers {
                        if !sampler.is_null() {
                            ffi::llama_sampler_free(sampler);
                        }
                    }
                    ffi::llama_sampler_free(sampler);
                    invalidate_context(&mut persistent);
                }
                return Err("llama.cpp could not create a sampler".to_owned());
            }
            unsafe {
                for sampler_part in samplers {
                    ffi::llama_sampler_chain_add(sampler, sampler_part);
                }
            }
            let remaining_prompt = prompt_tokens
                .get_mut(prompt_position..)
                .ok_or_else(|| "cached prompt position exceeds request prompt".to_owned())?;
            for prompt_batch in remaining_prompt.chunks_mut(
                usize::try_from(physical_batch).map_err(|_| "invalid batch size".to_owned())?,
            ) {
                let decode_result = unsafe {
                    ffi::llama_decode(context, make_batch(prompt_batch, prompt_position)?.batch)
                };
                prompt_position += prompt_batch.len();
                match classify_decode_result(
                    decode_result,
                    cancelled.load(std::sync::atomic::Ordering::SeqCst),
                ) {
                    DecodeOutcome::Success => {}
                    DecodeOutcome::Cancelled => {
                        unsafe {
                            ffi::llama_sampler_free(sampler);
                        }
                        invalidate_context(&mut persistent);
                        return Err("generation cancelled".to_owned());
                    }
                    DecodeOutcome::NativeFailure => {
                        unsafe {
                            ffi::llama_sampler_free(sampler);
                        }
                        invalidate_context(&mut persistent);
                        return Err(decode_error("prompt", decode_result));
                    }
                }
            }
            let prompt_eval_time = inference_started.elapsed();
            let vocab = unsafe { ffi::llama_model_get_vocab(self.pointer.cast_const()) };
            let mut pending_utf8 = Vec::new();
            let mut generated = 0_u64;
            let mut cancelled_result = false;
            let mut termination = NativeGenerationTermination::Stop;
            let mut previous_piece = String::new();
            let mut repeated_piece_count = 0_u8;
            for _ in 0..max_tokens {
                if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
                    cancelled_result = true;
                    break;
                }
                let token = unsafe { ffi::llama_sampler_sample(sampler, context, -1) };
                if unsafe { ffi::llama_vocab_is_eog(vocab, token) } {
                    break;
                }
                let text = unsafe { Self::token_piece(vocab, token) }?;
                let (text, terminal) = Self::terminal_piece_prefix(&text);
                pending_utf8.extend_from_slice(text);
                let delta = take_complete_utf8(&mut pending_utf8);
                if !delta.is_empty() {
                    if delta == previous_piece {
                        repeated_piece_count = repeated_piece_count.saturating_add(1);
                    } else {
                        previous_piece.clear();
                        previous_piece.push_str(&delta);
                        repeated_piece_count = 1;
                    }
                    if repeated_piece_count >= 8 {
                        termination = NativeGenerationTermination::Stop;
                        break;
                    }
                }
                let explicit_stop = stop_sequences
                    .iter()
                    .filter(|stop| !stop.is_empty())
                    .find_map(|stop| delta.find(stop).map(|index| (index, stop.len())));
                if let Some((index, _)) = explicit_stop {
                    if index > 0 && !callback(&delta[..index]) {
                        cancelled_result = true;
                    }
                    termination = NativeGenerationTermination::Stop;
                    break;
                }
                if !delta.is_empty() && !callback(&delta) {
                    cancelled_result = true;
                    break;
                }
                generated += 1;
                if terminal {
                    break;
                }
                if generated >= u64::from(max_tokens) {
                    termination = NativeGenerationTermination::Length;
                    break;
                }
                if prompt_count + generated >= u64::from(context_size) {
                    termination = NativeGenerationTermination::ContextLength;
                    break;
                }
                let mut next = [token];
                let result = unsafe {
                    ffi::llama_decode(
                        context,
                        make_batch(
                            &mut next,
                            usize::try_from(prompt_count)
                                .and_then(|value| {
                                    usize::try_from(generated)
                                        .map(|generated| value + generated - 1)
                                })
                                .map_err(|_| "token position is too large".to_owned())?,
                        )?
                        .batch,
                    )
                };
                match classify_decode_result(
                    result,
                    cancelled.load(std::sync::atomic::Ordering::SeqCst),
                ) {
                    DecodeOutcome::Success => {}
                    DecodeOutcome::Cancelled => {
                        unsafe {
                            ffi::llama_sampler_free(sampler);
                        }
                        invalidate_context(&mut persistent);
                        return Err("generation cancelled".to_owned());
                    }
                    DecodeOutcome::NativeFailure => {
                        unsafe {
                            ffi::llama_sampler_free(sampler);
                        }
                        invalidate_context(&mut persistent);
                        return Err(decode_error("token", result));
                    }
                }
            }
            if !cancelled_result && !pending_utf8.is_empty() {
                let final_text = String::from_utf8_lossy(&pending_utf8).into_owned();
                if !final_text.is_empty() && !callback(&final_text) {
                    cancelled_result = true;
                }
            }
            unsafe {
                ffi::llama_sampler_free(sampler);
            }
            if cancelled_result {
                invalidate_context(&mut persistent);
                return Err("generation cancelled".to_owned());
            }
            if let Some(cached) = persistent.as_mut() {
                cached.prompt_tokens = prompt_tokens;
            }
            Ok(NativeGenerationStatistics {
                prompt_tokens: prompt_count,
                generated_tokens: generated,
                context_tokens: prompt_count + generated,
                prompt_eval_time,
                generation_time: inference_started.elapsed(),
                termination,
                cache_hit,
                reused_prompt_tokens,
            })
        }
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{cached_prefix_matches, classify_decode_result, decode_error, DecodeOutcome};

    #[test]
    fn prompt_cache_reuse_requires_the_native_sequence_to_match_the_prefix() {
        assert!(cached_prefix_matches(36, 35));
        assert!(!cached_prefix_matches(36, 70));
        assert!(!cached_prefix_matches(0, -1));
        assert!(!cached_prefix_matches(usize::MAX, 35));
    }

    #[cfg(native_llama_cpp)]
    #[test]
    fn prompt_reuse_requires_matching_context_and_prefix() {
        let cached = super::PersistentContext {
            pointer: std::ptr::null_mut(),
            context_size: 512,
            prompt_tokens: vec![1, 2, 3],
            relay: Box::new(super::CancellationRelay {
                current: std::sync::atomic::AtomicPtr::new(std::ptr::null_mut()),
            }),
        };
        assert_eq!(
            super::compatible_prompt_reuse_len(&cached, &[1, 2, 3], 512),
            Some(2)
        );
        assert_eq!(
            super::compatible_prompt_reuse_len(&cached, &[1, 2, 3, 4], 512),
            Some(2)
        );
        assert_eq!(
            super::compatible_prompt_reuse_len(&cached, &[1, 9, 3], 512),
            None
        );
        assert_eq!(
            super::compatible_prompt_reuse_len(&cached, &[1, 2], 512),
            None
        );
        assert_eq!(
            super::compatible_prompt_reuse_len(&cached, &[1, 2, 3], 1024),
            None
        );
        std::mem::forget(cached);
    }

    #[cfg(native_llama_cpp)]
    #[test]
    #[ignore = "requires a real GGUF supplied through MARINA_TEST_GGUF"]
    fn embedded_chat_template_formats_a_user_prompt() {
        let model_path = std::env::var("MARINA_TEST_GGUF")
            .expect("MARINA_TEST_GGUF must point to a real GGUF for this test");
        super::initialize().expect("native backend should initialize");
        let model = super::load_model(&model_path).expect("test GGUF should load");
        let formatted = model
            .format_prompt("Reply with exactly: MARINA_OK")
            .expect("embedded chat template should format the prompt");
        assert!(formatted.contains("<|im_start|>user"));
        assert!(formatted.contains("Reply with exactly: MARINA_OK"));
        assert!(formatted.contains("<|im_start|>assistant"));
        drop(model);
        super::shutdown();
    }

    #[cfg(native_llama_cpp)]
    #[test]
    #[ignore = "requires a real GGUF supplied through MARINA_TEST_GGUF"]
    fn prompts_larger_than_the_physical_batch_decode_without_aborting() {
        let model_path = std::env::var("MARINA_TEST_GGUF")
            .expect("MARINA_TEST_GGUF must point to a real GGUF for this test");
        super::initialize().expect("native backend should initialize");
        let model = super::load_model(&model_path).expect("test GGUF should load");
        let prompt = format!("Summarize briefly: {}", "token ".repeat(700));
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let result = model.generate_stream(
            &prompt,
            2048,
            1,
            0.0,
            0.95,
            40,
            42,
            &[],
            |_| true,
            &cancelled,
        );
        assert!(result.is_ok(), "long prompt generation failed: {result:?}");
        drop(model);
        super::shutdown();
    }

    #[cfg(native_llama_cpp)]
    #[test]
    #[ignore = "requires a real GGUF supplied through MARINA_TEST_GGUF"]
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::too_many_lines)]
    fn persistent_context_reuses_a_compatible_prompt_prefix() {
        let model_path = std::env::var("MARINA_TEST_GGUF")
            .expect("MARINA_TEST_GGUF must point to a real GGUF for this test");
        super::initialize().expect("native backend should initialize");
        let load_started = std::time::Instant::now();
        let model = super::load_model(&model_path).expect("test GGUF should load");
        let load_time = load_started.elapsed();
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let prompt = "Reply briefly to the user.";
        eprintln!(
            "context benchmark model={} context_size=128 max_tokens=1 threads={} load_ms={}",
            model_path,
            super::inference_threads(),
            load_time.as_millis()
        );
        for request_index in 0..3 {
            model
                .reset_context()
                .expect("baseline reset should succeed");
            let started = std::time::Instant::now();
            let first_token = std::sync::Arc::new(std::sync::Mutex::new(None));
            let first_token_for_callback = first_token.clone();
            let result = model
                .generate_stream(
                    prompt,
                    128,
                    1,
                    0.0,
                    0.95,
                    40,
                    42,
                    &[],
                    |_| {
                        if let Ok(mut first_token) = first_token_for_callback.lock() {
                            first_token.get_or_insert_with(std::time::Instant::now);
                        }
                        true
                    },
                    &cancelled,
                )
                .expect("baseline generation should succeed");
            let first_token_ms = first_token
                .lock()
                .ok()
                .and_then(|first_token| *first_token)
                .map(|first_token| first_token.duration_since(started).as_millis());
            let prompt_tokens_per_second = (!result.prompt_eval_time.is_zero())
                .then(|| result.prompt_tokens as f64 / result.prompt_eval_time.as_secs_f64());
            let generation_tokens_per_second = (!result.generation_time.is_zero())
                .then(|| result.generated_tokens as f64 / result.generation_time.as_secs_f64());
            eprintln!(
                "context benchmark baseline_request={} elapsed_ms={} prompt_tokens={} generated_tokens={} prompt_eval_ms={} first_token_ms={:?} generation_ms={} prompt_tokens_per_second={:?} generation_tokens_per_second={:?} cache_hit={}",
                request_index + 1,
                started.elapsed().as_millis(),
                result.prompt_tokens,
                result.generated_tokens,
                result.prompt_eval_time.as_millis(),
                first_token_ms,
                result.generation_time.as_millis(),
                prompt_tokens_per_second,
                generation_tokens_per_second,
                result.cache_hit
            );
            assert!(!result.cache_hit);
        }
        model
            .reset_context()
            .expect("warm benchmark should start cold");
        for request_index in 0..10 {
            let started = std::time::Instant::now();
            let first_token = std::sync::Arc::new(std::sync::Mutex::new(None));
            let first_token_for_callback = first_token.clone();
            let result = model
                .generate_stream(
                    prompt,
                    128,
                    1,
                    0.0,
                    0.95,
                    40,
                    42,
                    &[],
                    |_| {
                        if let Ok(mut first_token) = first_token_for_callback.lock() {
                            first_token.get_or_insert_with(std::time::Instant::now);
                        }
                        true
                    },
                    &cancelled,
                )
                .expect("repeated generation should succeed");
            let first_token_ms = first_token
                .lock()
                .ok()
                .and_then(|first_token| *first_token)
                .map(|first_token| first_token.duration_since(started).as_millis());
            let prompt_tokens_per_second = (!result.prompt_eval_time.is_zero())
                .then(|| result.prompt_tokens as f64 / result.prompt_eval_time.as_secs_f64());
            let generation_tokens_per_second = (!result.generation_time.is_zero())
                .then(|| result.generated_tokens as f64 / result.generation_time.as_secs_f64());
            eprintln!(
                "context benchmark request={} elapsed_ms={} prompt_tokens={} generated_tokens={} prompt_eval_ms={} first_token_ms={:?} generation_ms={} prompt_tokens_per_second={:?} generation_tokens_per_second={:?} cache_hit={} reused_prompt_tokens={}",
                request_index + 1,
                started.elapsed().as_millis(),
                result.prompt_tokens,
                result.generated_tokens,
                result.prompt_eval_time.as_millis(),
                first_token_ms,
                result.generation_time.as_millis(),
                prompt_tokens_per_second,
                generation_tokens_per_second,
                result.cache_hit,
                result.reused_prompt_tokens
            );
            assert_eq!(result.cache_hit, request_index > 0);
            assert_eq!(result.reused_prompt_tokens > 0, request_index > 0);
        }

        model
            .reset_context()
            .expect("IDE-style benchmark should start cold");
        let first_turn = "\u{001e}user\nReview this code briefly.";
        let first_turn_result = model
            .generate_stream(
                first_turn,
                128,
                1,
                0.0,
                0.95,
                40,
                42,
                &[],
                |_| true,
                &cancelled,
            )
            .expect("initial IDE-style request should succeed");
        assert!(!first_turn_result.cache_hit);
        let continued_turn =
            "\u{001e}user\nReview this code briefly.\u{001e}assistant\nI found one issue.\u{001e}user\nSuggest a minimal fix.";
        let continued_turn_result = model
            .generate_stream(
                continued_turn,
                128,
                1,
                0.0,
                0.95,
                40,
                42,
                &[],
                |_| true,
                &cancelled,
            )
            .expect("extended IDE-style request should succeed");
        eprintln!(
            "context benchmark ide_followup prompt_eval_ms={} cache_hit={} reused_prompt_tokens={}",
            continued_turn_result.prompt_eval_time.as_millis(),
            continued_turn_result.cache_hit,
            continued_turn_result.reused_prompt_tokens
        );
        assert!(continued_turn_result.cache_hit);
        assert!(continued_turn_result.reused_prompt_tokens > 0);

        let unrelated = model
            .generate_stream(
                "This is a separate conversation. Reply briefly.",
                128,
                1,
                0.0,
                0.95,
                40,
                42,
                &[],
                |_| true,
                &cancelled,
            )
            .expect("independent conversation should succeed");
        assert!(!unrelated.cache_hit);
        let after_unrelated = model
            .generate_stream(prompt, 128, 1, 0.0, 0.95, 40, 42, &[], |_| true, &cancelled)
            .expect("original conversation should remain isolated");
        assert!(!after_unrelated.cache_hit);

        model
            .reset_context()
            .expect("explicit reset should succeed");
        let after_reset = model
            .generate_stream(prompt, 128, 1, 0.0, 0.95, 40, 42, &[], |_| true, &cancelled)
            .expect("generation after explicit reset should succeed");
        assert!(!after_reset.cache_hit);

        let cancel_prompt = "Cancel this request safely.";
        let cancellation = model.generate_stream(
            cancel_prompt,
            128,
            8,
            0.0,
            0.95,
            40,
            42,
            &[],
            |_| false,
            &cancelled,
        );
        assert!(cancellation.is_err());
        let after_cancellation = model
            .generate_stream(prompt, 128, 1, 0.0, 0.95, 40, 42, &[], |_| true, &cancelled)
            .expect("inference should recover after cancellation");
        assert!(!after_cancellation.cache_hit);
        drop(model);
        super::shutdown();
    }

    #[test]
    fn native_abort_is_cancellation_only_when_requested() {
        assert_eq!(classify_decode_result(2, true), DecodeOutcome::Cancelled);
        assert_eq!(
            classify_decode_result(2, false),
            DecodeOutcome::NativeFailure
        );
        assert_eq!(
            classify_decode_result(-1, true),
            DecodeOutcome::NativeFailure
        );
        assert!(decode_error("token", 2).contains("without an active Marina cancellation"));
    }

    #[test]
    fn successful_decode_remains_success() {
        assert_eq!(classify_decode_result(0, false), DecodeOutcome::Success);
    }

    #[cfg(native_llama_cpp)]
    #[test]
    fn terminal_special_pieces_are_not_emitted() {
        let (prefix, terminal) = super::NativeModel::terminal_piece_prefix(b"answer</|im_end|}");
        assert_eq!(prefix, b"answer");
        assert!(terminal);
        let (plain, terminal) = super::NativeModel::terminal_piece_prefix(b"answer");
        assert_eq!(plain, b"answer");
        assert!(!terminal);
        let (partial, terminal) = super::NativeModel::terminal_piece_prefix(b"answer<|im_");
        assert_eq!(partial, b"answer<|im_");
        assert!(!terminal);
    }
}

impl Drop for NativeModel {
    fn drop(&mut self) {
        #[cfg(native_llama_cpp)]
        unsafe {
            if let Ok(mut context) = self.context.lock() {
                context.take();
            }
            ffi::llama_model_free(self.pointer);
        }
    }
}

/// Load a GGUF file using `llama_model_load_from_file`.
///
/// # Errors
///
/// Returns an error when native linking is unavailable, the path contains an
/// embedded NUL, or llama.cpp rejects the model.
pub fn load_model(path: &str) -> Result<NativeModel, String> {
    #[cfg(native_llama_cpp)]
    unsafe {
        let path = CString::new(path).map_err(|_| "model path contains NUL".to_owned())?;
        let pointer =
            ffi::llama_model_load_from_file(path.as_ptr(), ffi::llama_model_default_params());
        if pointer.is_null() {
            return Err("llama.cpp rejected the model".to_owned());
        }
        let memory_bytes = ffi::llama_model_size(pointer.cast_const());
        Ok(NativeModel {
            context: Mutex::new(None),
            pointer,
            memory_bytes,
        })
    }
    #[cfg(not(native_llama_cpp))]
    {
        let _ = path;
        Err("native llama.cpp is unavailable".to_owned())
    }
}
