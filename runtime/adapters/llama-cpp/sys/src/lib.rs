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
#[cfg(native_mtmd)]
use std::io::Cursor;
use std::sync::atomic::AtomicBool;
#[cfg(native_llama_cpp)]
use std::sync::atomic::AtomicPtr;
#[cfg(native_llama_cpp)]
use std::sync::atomic::Ordering;
#[cfg(native_llama_cpp)]
use std::sync::Mutex;
#[cfg(native_llama_cpp)]
mod tool_grammar;

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
    allocated_context_size: u32,
    prompt_tokens: Vec<i32>,
    checkpoints: Vec<PromptCheckpoint>,
    relay: Box<CancellationRelay>,
}

// Only partial/recurrent state is copied. Attention KV stays in its existing
// native context and is trimmed after restoring the matching recurrent state.
#[cfg(native_llama_cpp)]
struct PromptCheckpoint {
    tokens: Vec<i32>,
    state: Vec<u8>,
}

#[cfg(native_llama_cpp)]
impl std::fmt::Debug for PromptCheckpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PromptCheckpoint")
            .field("tokens", &self.tokens.len())
            .field("state_bytes", &self.state.len())
            .finish()
    }
}

#[cfg(native_llama_cpp)]
const PARTIAL_STATE_ONLY: u32 = 1;
#[cfg(native_llama_cpp)]
const MAX_CHECKPOINT_BYTES: usize = 64 * 1_048_576;
#[cfg(native_llama_cpp)]
const MAX_TOTAL_CHECKPOINT_BYTES: usize = 128 * 1_048_576;

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

#[cfg(native_mtmd)]
#[repr(C)]
#[derive(Clone, Copy)]
struct MtmdContextParams {
    use_gpu: bool,
    device: *mut c_void,
    print_timings: bool,
    n_threads: i32,
    image_marker: *const c_char,
    media_marker: *const c_char,
    flash_attn_type: i32,
    warmup: bool,
    image_min_tokens: i32,
    image_max_tokens: i32,
    eval_callback: Option<unsafe extern "C" fn(*mut c_void, bool, *mut c_void) -> bool>,
    eval_callback_data: *mut c_void,
    batch_max_tokens: i32,
    progress_callback: Option<unsafe extern "C" fn(f32, *mut c_void) -> bool>,
    progress_callback_data: *mut c_void,
}

#[cfg(native_mtmd)]
#[repr(C)]
struct MtmdInputText {
    text: *const c_char,
    text_len: usize,
    add_special: bool,
    parse_special: bool,
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

#[cfg(native_mtmd)]
unsafe extern "C" fn mtmd_eval_callback(
    _tensor: *mut c_void,
    _ask: bool,
    data: *mut c_void,
) -> bool {
    if data.is_null() {
        return true;
    }
    let relay = &*data.cast::<CancellationRelay>();
    let current = relay.current.load(Ordering::SeqCst);
    current.is_null() || !(*current).load(Ordering::SeqCst)
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
        pub fn llama_model_meta_val_str(
            model: *const c_void,
            key: *const c_char,
            buf: *mut c_char,
            buf_size: usize,
        ) -> i32;
        pub fn llama_model_n_ctx_train(model: *const c_void) -> i32;
        pub fn llama_model_has_decoder(model: *const c_void) -> bool;
        pub fn llama_vocab_get_add_bos(vocab: *const c_void) -> bool;
        pub fn llama_vocab_bos(vocab: *const c_void) -> i32;
        #[cfg(test)]
        pub fn llama_vocab_get_text(vocab: *const c_void, token: i32) -> *const c_char;
        pub fn llama_vocab_get_attr(vocab: *const c_void, token: i32) -> i32;

        pub fn llama_model_n_embd(model: *const c_void) -> i32;
        pub fn ggml_backend_dev_count() -> usize;
        pub fn ggml_backend_dev_get(index: usize) -> *mut c_void;
        pub fn ggml_backend_dev_name(device: *mut c_void) -> *const c_char;
        pub fn ggml_backend_dev_type(device: *mut c_void) -> i32;
        pub fn ggml_backend_dev_memory(device: *mut c_void, free: *mut usize, total: *mut usize);
        pub fn ggml_backend_dev_backend_reg(device: *mut c_void) -> *mut c_void;
        pub fn ggml_backend_reg_name(registry: *mut c_void) -> *const c_char;
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
        pub fn llama_model_is_recurrent(model: *const c_void) -> bool;
        pub fn llama_model_is_hybrid(model: *const c_void) -> bool;
        pub fn llama_state_seq_get_size_ext(ctx: *mut c_void, seq_id: i32, flags: u32) -> usize;
        pub fn llama_state_seq_get_data_ext(
            ctx: *mut c_void,
            dst: *mut u8,
            size: usize,
            seq_id: i32,
            flags: u32,
        ) -> usize;
        pub fn llama_state_seq_set_data_ext(
            ctx: *mut c_void,
            src: *const u8,
            size: usize,
            seq_id: i32,
            flags: u32,
        ) -> usize;
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
        pub fn llama_sampler_init_grammar_lazy_patterns(
            vocab: *const c_void,
            grammar_str: *const c_char,
            grammar_root: *const c_char,
            trigger_patterns: *const *const c_char,
            num_trigger_patterns: usize,
            trigger_tokens: *const i32,
            num_trigger_tokens: usize,
        ) -> *mut c_void;
        pub fn llama_vocab_n_tokens(vocab: *const c_void) -> i32;
        pub fn llama_sampler_init_top_k(k: i32) -> *mut c_void;
        pub fn llama_sampler_init_top_p(p: f32, min_keep: usize) -> *mut c_void;
        pub fn llama_sampler_init_temp(t: f32) -> *mut c_void;
        pub fn llama_sampler_init_penalties(
            n_vocab: i32,
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

#[cfg(native_mtmd)]
mod mtmd_ffi {
    use super::{c_char, c_void, MtmdContextParams, MtmdInputText};

    #[allow(improper_ctypes)]
    extern "C" {
        pub fn mtmd_context_params_default() -> MtmdContextParams;
        pub fn mtmd_init_from_file(
            projector_path: *const c_char,
            model: *const c_void,
            params: MtmdContextParams,
        ) -> *mut c_void;
        pub fn mtmd_free(context: *mut c_void);
        pub fn mtmd_support_vision(context: *const c_void) -> bool;
        pub fn mtmd_bitmap_init(width: u32, height: u32, rgb: *const u8) -> *mut c_void;
        pub fn mtmd_bitmap_free(bitmap: *mut c_void);
        pub fn mtmd_input_chunks_init() -> *mut c_void;
        pub fn mtmd_input_chunks_free(chunks: *mut c_void);
        pub fn mtmd_helper_get_n_tokens(chunks: *const c_void) -> usize;
        pub fn mtmd_tokenize(
            context: *mut c_void,
            output: *mut c_void,
            text: *const MtmdInputText,
            bitmaps: *const *const c_void,
            bitmap_count: usize,
        ) -> i32;
        pub fn mtmd_helper_eval_chunks(
            context: *mut c_void,
            llama_context: *mut c_void,
            chunks: *const c_void,
            n_past: i32,
            sequence_id: i32,
            batch_size: i32,
            logits_last: bool,
            new_n_past: *mut i32,
        ) -> i32;
    }
}

/// Whether this build has been configured to link the native library.
#[must_use]
pub const fn is_available() -> bool {
    cfg!(native_llama_cpp)
}

/// Whether this build links llama.cpp's multimodal MTMD library.
#[must_use]
pub const fn is_mtmd_available() -> bool {
    cfg!(native_mtmd)
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
    logits: Vec<i8>,
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
        logits,
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
        cached.checkpoints.clear();
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
                .map_or(0, |count| i32::try_from(count.get()).unwrap_or(i32::MAX))
        })
}

#[cfg(any(native_llama_cpp, test))]
fn text_context_capacity(limit: u32, prompt_tokens: usize, max_tokens: u32) -> u32 {
    let required = required_text_context(limit, prompt_tokens, max_tokens);
    // Reserve room for the next tool result instead of rebuilding immediately
    // when prompt + output happens to fall just below a power-of-two boundary.
    let needed = required
        .saturating_add((required / 8).max(128))
        .max(512)
        .min(limit);
    needed
        .checked_next_power_of_two()
        .unwrap_or(limit)
        .min(limit)
}

#[cfg(any(native_llama_cpp, test))]
fn required_text_context(limit: u32, prompt_tokens: usize, max_tokens: u32) -> u32 {
    u32::try_from(prompt_tokens)
        .unwrap_or(u32::MAX)
        .saturating_add(max_tokens)
        .min(limit)
}

#[cfg(native_llama_cpp)]
fn compatible_prompt_reuse_len(
    cached: &PersistentContext,
    prompt_tokens: &[i32],
    context_size: u32,
    strategy: oid_model_runner::CacheStrategy,
) -> Option<usize> {
    if strategy == oid_model_runner::CacheStrategy::Disabled
        || cached.context_size != context_size
        || prompt_tokens.len() < 2
    {
        return None;
    }
    if strategy == oid_model_runner::CacheStrategy::Recurrent {
        return cached
            .checkpoints
            .iter()
            .filter(|checkpoint| {
                checkpoint.tokens.len() < prompt_tokens.len()
                    && prompt_tokens.starts_with(&checkpoint.tokens)
            })
            .map(|checkpoint| checkpoint.tokens.len())
            .max();
    }
    // Attention KV can be trimmed to any exact token prefix. A changed or
    // shorter user turn does not invalidate the unchanged system/tool prefix.
    // Always evaluate at least the final input token to obtain fresh logits.
    let common = cached
        .prompt_tokens
        .iter()
        .zip(prompt_tokens)
        .take_while(|(cached, current)| cached == current)
        .count();
    let reuse = common.min(prompt_tokens.len() - 1);
    (reuse > 0).then_some(reuse)
}

#[cfg(native_llama_cpp)]
fn save_prompt_checkpoint(cached: &mut PersistentContext, tokens: &[i32]) {
    if tokens.is_empty() {
        return;
    }
    // Replacing a checkpoint releases its buffer before allocating a new one.
    cached
        .checkpoints
        .retain(|checkpoint| checkpoint.tokens.len() != tokens.len());
    let size = unsafe { ffi::llama_state_seq_get_size_ext(cached.pointer, 0, PARTIAL_STATE_ONLY) };
    let retained: usize = cached
        .checkpoints
        .iter()
        .map(|checkpoint| checkpoint.state.len())
        .sum();
    if size == 0
        || size > MAX_CHECKPOINT_BYTES
        || retained.saturating_add(size) > MAX_TOTAL_CHECKPOINT_BYTES
    {
        return;
    }
    let mut state = vec![0; size];
    let written = unsafe {
        ffi::llama_state_seq_get_data_ext(
            cached.pointer,
            state.as_mut_ptr(),
            state.len(),
            0,
            PARTIAL_STATE_ONLY,
        )
    };
    if written != size {
        return;
    }
    cached.checkpoints.push(PromptCheckpoint {
        tokens: tokens.to_vec(),
        state,
    });
    eprintln!(
        "llama.cpp prompt checkpoint tokens={} bytes={size}",
        tokens.len()
    );
}

#[cfg(any(native_llama_cpp, test))]
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
    profile: oid_model_runner::ModelProfile,
    #[cfg(native_llama_cpp)]
    context: Mutex<Option<PersistentContext>>,
    #[cfg(native_mtmd)]
    mtmd: Option<*mut c_void>,
    #[cfg(native_mtmd)]
    mtmd_relay: Option<Box<CancellationRelay>>,
    #[cfg_attr(not(native_llama_cpp), allow(dead_code))]
    pointer: *mut c_void,
    memory_bytes: u64,
    placement: NativePlacement,
}

/// Native execution backend names understood by Talvik.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeBackend {
    /// NVIDIA CUDA.
    Cuda,
    /// Khronos Vulkan.
    Vulkan,
    /// Apple Metal.
    Metal,
    /// Portable CPU backend.
    Cpu,
}

impl NativeBackend {
    /// Stable diagnostics name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cuda => "cuda",
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
            Self::Cpu => "cpu",
        }
    }
}

/// One backend device enumerated by the linked ggml runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeDevice {
    /// Backend family.
    pub backend: NativeBackend,
    /// Native device name.
    pub name: String,
    /// Free device memory, when reported.
    pub free_memory_bytes: Option<u64>,
    /// Total device memory, when reported.
    pub total_memory_bytes: Option<u64>,
    /// Whether the device shares host memory, when known.
    pub shared_memory: Option<bool>,
}

/// Native model-load placement controls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeLoadOptions {
    /// Selected backend.
    pub backend: NativeBackend,
    /// Requested GPU layers. Zero is always CPU-only.
    pub gpu_layers: u32,
}

#[cfg(test)]
const fn cpu_load_options() -> NativeLoadOptions {
    NativeLoadOptions {
        backend: NativeBackend::Cpu,
        gpu_layers: 0,
    }
}

/// Placement facts observed after native model loading.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativePlacement {
    /// Selected backend.
    pub backend: NativeBackend,
    /// Selected device name, when known.
    pub device: Option<String>,
    /// Requested GPU layers.
    pub requested_gpu_layers: u32,
    /// Actual offloaded layers, only when the pinned API verifies the count.
    pub offloaded_gpu_layers: Option<u32>,
    /// Whether llama.cpp reports any non-CPU device used by the model.
    pub accelerator_used: bool,
}

/// Enumerate devices registered by the linked ggml build.
#[must_use]
pub fn devices() -> Vec<NativeDevice> {
    indexed_devices()
        .into_iter()
        .map(|(_, device)| device)
        .collect()
}

// Preserve native indices when filtering auxiliary/unsupported devices. BLAS
// and AMX are ACCEL devices, not independently selectable CPU runners.
fn indexed_devices() -> Vec<(usize, NativeDevice)> {
    #[cfg(native_llama_cpp)]
    unsafe {
        let mut devices = Vec::new();
        for index in 0..ffi::ggml_backend_dev_count() {
            let device = ffi::ggml_backend_dev_get(index);
            if device.is_null() {
                continue;
            }
            let backend = ffi::ggml_backend_dev_backend_reg(device);
            let backend_name = if backend.is_null() {
                String::new()
            } else {
                CStr::from_ptr(ffi::ggml_backend_reg_name(backend))
                    .to_string_lossy()
                    .to_ascii_lowercase()
            };
            let Some(backend) =
                classify_native_device(ffi::ggml_backend_dev_type(device), &backend_name)
            else {
                continue;
            };
            let name = CStr::from_ptr(ffi::ggml_backend_dev_name(device))
                .to_string_lossy()
                .into_owned();
            let mut free = 0_usize;
            let mut total = 0_usize;
            ffi::ggml_backend_dev_memory(
                device,
                std::ptr::addr_of_mut!(free),
                std::ptr::addr_of_mut!(total),
            );
            // The pinned ggml API does not expose a portable dedicated/shared
            // pool flag. Metal is unified and CUDA reports dedicated device
            // memory; Vulkan remains unknown because it may be discrete or
            // integrated. CPU capacity is host memory.
            let shared_memory = match backend {
                NativeBackend::Metal | NativeBackend::Cpu => Some(true),
                NativeBackend::Cuda => Some(false),
                NativeBackend::Vulkan => None,
            };
            devices.push((
                index,
                NativeDevice {
                    backend,
                    name,
                    free_memory_bytes: u64::try_from(free).ok().filter(|value| *value > 0),
                    total_memory_bytes: u64::try_from(total).ok().filter(|value| *value > 0),
                    shared_memory,
                },
            ));
        }
        devices
    }
    #[cfg(not(native_llama_cpp))]
    {
        Vec::new()
    }
}

#[cfg(any(native_llama_cpp, test))]
fn classify_native_device(device_type: i32, backend_name: &str) -> Option<NativeBackend> {
    // Values from the pinned ggml_backend_dev_type enum. An accelerator must
    // never win CPU selection just because its registry name is unfamiliar.
    match device_type {
        0 => Some(NativeBackend::Cpu),
        1 | 2 if backend_name.contains("cuda") => Some(NativeBackend::Cuda),
        1 | 2 if backend_name.contains("vulkan") => Some(NativeBackend::Vulkan),
        1 | 2 if backend_name.contains("metal") => Some(NativeBackend::Metal),
        _ => None,
    }
}

#[cfg(any(native_llama_cpp, test))]
fn render_tool_history(
    content: &str,
    format: Option<oid_model_runner::ToolFormat>,
) -> Result<String, String> {
    use oid_model_runner::ToolFormat;
    use std::fmt::Write as _;
    let history: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("invalid tool history: {e}"))?;
    let calls = history["calls"]
        .as_array()
        .filter(|calls| !calls.is_empty())
        .ok_or("tool history requires calls")?;
    let mut text = history["content"].as_str().unwrap_or("").to_owned();
    let escape = |value: &str| {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    for call in calls {
        let function = &call["function"];
        let name = function["name"]
            .as_str()
            .ok_or("tool history requires a function name")?;
        let arguments = match &function["arguments"] {
            serde_json::Value::String(value) => serde_json::from_str(value)
                .map_err(|e| format!("invalid tool history arguments: {e}"))?,
            value => value.clone(),
        };
        let arguments = arguments
            .as_object()
            .ok_or("tool history arguments must be an object")?;
        if !text.is_empty() {
            text.push('\n');
        }
        if let Some(id) = call["id"].as_str() {
            writeln!(text, "Tool call ID: {}", serde_json::json!(id)).map_err(|e| e.to_string())?;
        }
        match format {
            Some(ToolFormat::XmlJson) => {
                let payload = serde_json::json!({"name": name, "arguments": arguments});
                write!(text, "<tool_call>{payload}</tool_call>").map_err(|e| e.to_string())?;
            }
            Some(ToolFormat::XmlFunction) => {
                let valid_name = |value: &str| {
                    !value.is_empty()
                        && value
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                };
                if !valid_name(name) || arguments.keys().any(|key| !valid_name(key)) {
                    return Err(
                        "tool history has an unsupported function or parameter name".to_owned()
                    );
                }
                write!(text, "<tool_call>\n<function={name}>\n").map_err(|e| e.to_string())?;
                for (key, value) in arguments {
                    let value = value
                        .as_str()
                        .map_or_else(|| value.to_string(), str::to_owned);
                    write!(
                        text,
                        "<parameter={key}>\n{}\n</parameter>\n",
                        escape(&value)
                    )
                    .map_err(|e| e.to_string())?;
                }
                text.push_str("</function>\n</tool_call>");
            }
            None => return Err("the loaded model has no supported tool history format".to_owned()),
        }
    }
    Ok(text)
}

#[cfg(native_mtmd)]
struct OwnedMtmdBitmaps(Vec<*mut c_void>);

#[cfg(native_mtmd)]
impl Drop for OwnedMtmdBitmaps {
    fn drop(&mut self) {
        unsafe {
            for bitmap in self.0.drain(..) {
                mtmd_ffi::mtmd_bitmap_free(bitmap);
            }
        }
    }
}

#[cfg(native_mtmd)]
struct OwnedMtmdChunks(*mut c_void);

#[cfg(native_mtmd)]
impl Drop for OwnedMtmdChunks {
    fn drop(&mut self) {
        unsafe {
            mtmd_ffi::mtmd_input_chunks_free(self.0);
        }
    }
}

#[cfg(native_mtmd)]
fn decode_image_inputs(images: &[(String, &[u8])]) -> Result<Vec<(u32, u32, Vec<u8>)>, String> {
    const MAX_DIMENSION: u32 = 8192;
    const MAX_TOTAL_PIXELS: u64 = 16 * 1024 * 1024;
    let mut total_pixels = 0_u64;
    images
        .iter()
        .map(|(media_type, data)| {
            let format = match media_type.as_str() {
                "image/png" => image::ImageFormat::Png,
                "image/jpeg" => image::ImageFormat::Jpeg,
                "image/bmp" => image::ImageFormat::Bmp,
                "image/gif" => image::ImageFormat::Gif,
                _ => return Err(format!("unsupported image media type `{media_type}`")),
            };
            let (width, height) = image::ImageReader::with_format(Cursor::new(data), format)
                .into_dimensions()
                .map_err(|error| format!("could not read image dimensions: {error}"))?;
            if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
                return Err(format!(
                    "image dimensions must be between 1 and {MAX_DIMENSION} pixels"
                ));
            }
            total_pixels = total_pixels
                .checked_add(u64::from(width) * u64::from(height))
                .ok_or_else(|| "image pixel count overflowed".to_owned())?;
            if total_pixels > MAX_TOTAL_PIXELS {
                return Err(format!(
                    "images exceed the {MAX_TOTAL_PIXELS}-pixel request limit"
                ));
            }
            let decoded = image::load_from_memory_with_format(data, format)
                .map_err(|error| format!("could not decode image: {error}"))?
                .to_rgb8();
            if decoded.width() != width || decoded.height() != height {
                return Err("decoded image dimensions changed during validation".to_owned());
            }
            Ok((width, height, decoded.into_raw()))
        })
        .collect()
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

const END_MARKERS: [&str; 6] = [
    "<|im_end|>",
    "</|im_end|}",
    "|im_end|",
    "<|endoftext|>",
    "|endoftext|",
    "</s>",
];

/// Removes end-of-turn markers that a model emits as several separate tokens.
#[cfg_attr(not(native_llama_cpp), allow(dead_code))]
#[derive(Default)]
struct EndMarkerFilter {
    held: String,
}

#[cfg_attr(not(native_llama_cpp), allow(dead_code))]
impl EndMarkerFilter {
    /// Returns text that is safe to emit and whether an end marker was seen.
    fn push(&mut self, text: &str) -> (String, bool) {
        self.held.push_str(text);
        let earliest = END_MARKERS
            .iter()
            .filter_map(|marker| self.held.find(marker))
            .min();
        if let Some(mut index) = earliest {
            // Bare markers also match inside a split angle-bracketed marker
            // before its final '>' arrives. Consume its leading '<' as well.
            if index > 0
                && self.held.as_bytes()[index] == b'|'
                && self.held.as_bytes()[index - 1] == b'<'
            {
                index -= 1;
            }
            // A model can repeat '<' before a split terminal marker. Keep
            // these control-prefix bytes out of the completed response.
            while index > 0
                && self.held.as_bytes()[index] == b'<'
                && self.held.as_bytes()[index - 1] == b'<'
            {
                index -= 1;
            }
            let emit = self.held[..index].to_owned();
            self.held.clear();
            return (emit, true);
        }
        let mut keep = (1..=self.held.len())
            .rev()
            .filter(|length| self.held.is_char_boundary(self.held.len() - length))
            .find(|length| {
                let suffix = &self.held[self.held.len() - length..];
                END_MARKERS.iter().any(|marker| marker.starts_with(suffix))
            })
            .unwrap_or(0);
        // Keep runs of marker-start bytes together. Otherwise a repeated `<`
        // can cause every copy except the last to be emitted before an EOG
        // token, leaking a dangling marker into the completed response.
        let repeated_marker_prefix = self
            .held
            .chars()
            .rev()
            .take_while(|character| {
                END_MARKERS
                    .iter()
                    .any(|marker| marker.starts_with(*character))
            })
            .map(char::len_utf8)
            .sum::<usize>();
        keep = keep.max(repeated_marker_prefix);
        let mut split = self.held.len() - keep;
        if keep > 0 && self.held.as_bytes()[split] == b'<' {
            while split > 0 && self.held.as_bytes()[split - 1] == b'<' {
                split -= 1;
            }
        }
        let emit = self.held[..split].to_owned();
        self.held.drain(..split);
        (emit, false)
    }

    /// Returns text held back as a possible marker prefix once generation ends.
    fn finish(&mut self) -> String {
        let held = std::mem::take(&mut self.held);
        if END_MARKERS
            .iter()
            .any(|marker| marker.starts_with(held.as_str()))
            || held.chars().all(|character| {
                END_MARKERS
                    .iter()
                    .any(|marker| marker.starts_with(character))
            })
        {
            String::new()
        } else {
            held
        }
    }
}

// Both strings are produced by the loaded model's template. A candidate
// prefix is optional: rendering a partial conversation is not prefix-stable
// for every template. Generation must still work when no safe prefix exists.
#[cfg(native_llama_cpp)]
struct FormattedPrompt {
    text: String,
    cache_prefix: Option<String>,
    constrain_json_tools: bool,
}

#[cfg(native_llama_cpp)]
fn render_template_messages(
    template: &CStr,
    entries: &[(String, String)],
    add_assistant: bool,
) -> Result<String, String> {
    let roles = entries
        .iter()
        .map(|(role, _)| CString::new(role.as_str()).map_err(|_| "invalid chat role".to_owned()))
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
    // The C strings and messages remain alive through both native calls.
    let required = unsafe {
        ffi::llama_chat_apply_template(
            template.as_ptr(),
            messages.as_ptr(),
            messages.len(),
            add_assistant,
            std::ptr::null_mut(),
            0,
        )
    };
    if required < 0 {
        return Err(format!(
            "llama.cpp could not apply the model chat template: {required}"
        ));
    }
    let capacity = required
        .checked_add(1)
        .ok_or_else(|| "formatted prompt is too large".to_owned())?;
    let mut buffer = vec![
        0_u8;
        usize::try_from(capacity)
            .map_err(|_| "formatted prompt is too large".to_owned())?
    ];
    let written = unsafe {
        ffi::llama_chat_apply_template(
            template.as_ptr(),
            messages.as_ptr(),
            messages.len(),
            add_assistant,
            buffer.as_mut_ptr().cast(),
            capacity,
        )
    };
    if written < 0 || written >= capacity {
        return Err(format!(
            "llama.cpp could not format the model chat template: {written}"
        ));
    }
    buffer.truncate(
        usize::try_from(written).map_err(|_| "formatted prompt is too large".to_owned())?,
    );
    String::from_utf8(buffer)
        .map_err(|_| "llama.cpp produced an invalid UTF-8 chat prompt".to_owned())
}

#[cfg(native_llama_cpp)]
fn render_chat_prompt(
    template: &CStr,
    entries: &[(String, String)],
    enable_thinking: Option<bool>,
) -> Result<FormattedPrompt, String> {
    let text = apply_thinking_prefill(
        render_template_messages(template, entries, true)?,
        &template.to_string_lossy(),
        enable_thinking,
    );
    // Use structured message roles, never strings found in user content or a
    // particular family's delimiter. Some templates merge system/user turns;
    // a partial render may be empty, fail, or differ from the full prompt.
    let cache_prefix = entries
        .iter()
        .rposition(|(role, _)| role == "user")
        .filter(|position| *position > 0)
        .and_then(|position| render_template_messages(template, &entries[..position], false).ok())
        .filter(|prefix| !prefix.is_empty() && text.starts_with(prefix));
    Ok(FormattedPrompt {
        text,
        cache_prefix,
        constrain_json_tools: false,
    })
}

// The C role/content formatter does not execute the Jinja tools branch.
// Select the declaration envelope from template markers, never a model name.
#[cfg(any(native_llama_cpp, test))]
fn tool_declarations(template: &str, content: &str) -> Result<String, String> {
    if template.contains("<tools>")
        && template.contains("</tools>")
        && template.contains("<tool_call>")
        && template.contains("tojson")
    {
        let tools: Vec<serde_json::Value> = serde_json::from_str(content)
            .map_err(|error| format!("invalid tool definitions: {error}"))?;
        let signatures = tools
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        Ok(format!("# Tools\n\nYou may call one or more functions to assist with the user query.\n\nYou are provided with function signatures within <tools></tools> XML tags:\n<tools>\n{signatures}\n</tools>"))
    } else {
        Ok(format!("Tools: {content}"))
    }
}

#[cfg(any(native_llama_cpp, test))]
fn verified_checkpoint_position(prefix: &[i32], prompt: &[i32]) -> Option<usize> {
    // BPE tokenization can merge at a text boundary. Text-prefix matching alone
    // is insufficient; only an exact, nontrivial token prefix is reusable.
    (prefix.len() >= 64 && prefix.len() < prompt.len() && prompt.starts_with(prefix))
        .then_some(prefix.len())
}

// The C template API selects a built-in formatter; it does not evaluate the
// GGUF's Jinja kwargs. Apply Qwen's documented assistant prefill explicitly.
#[cfg_attr(not(native_llama_cpp), allow(dead_code))]
fn apply_thinking_prefill(mut prompt: String, template: &str, enabled: Option<bool>) -> String {
    if template.contains("enable_thinking") && template.contains("<think>") {
        match enabled {
            Some(false) if !prompt.ends_with("</think>\n\n") => {
                prompt.push_str("<think>\n\n</think>\n\n");
            }
            Some(true) if !prompt.ends_with("<think>\n") => prompt.push_str("<think>\n"),
            _ => {}
        }
    }
    prompt
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
    /// Immutable artifact and serving support facts observed during load.
    #[must_use]
    pub const fn profile(&self) -> &oid_model_runner::ModelProfile {
        &self.profile
    }

    /// Report usable tool handling, requiring an implemented output parser.
    #[must_use]
    pub fn supports_tools(&self) -> bool {
        self.profile.tool_format.is_some()
    }

    /// Model memory reported by llama.cpp.
    #[must_use]
    pub const fn memory_bytes(&self) -> u64 {
        self.memory_bytes
    }

    /// Return placement facts observed during native loading.
    #[must_use]
    pub fn placement(&self) -> &NativePlacement {
        &self.placement
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
        self.tokenize_with_special(text, false)
    }

    #[cfg(native_llama_cpp)]
    fn tokenize_generation(&self, text: &str) -> Result<Vec<i32>, String> {
        let mut tokens = self.tokenize_with_special(text, true)?;
        let bos =
            unsafe { ffi::llama_vocab_bos(ffi::llama_model_get_vocab(self.pointer.cast_const())) };
        if self.profile.tokenizer_adds_bos && bos >= 0 && tokens.first() != Some(&bos) {
            tokens.insert(0, bos);
        }
        Ok(tokens)
    }

    fn tokenize_with_special(&self, text: &str, parse_special: bool) -> Result<Vec<i32>, String> {
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
                parse_special,
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
                parse_special,
            );
            if count < 0 {
                return Err("llama.cpp tokenization failed".to_owned());
            }
            tokens.truncate(usize::try_from(count).map_err(|_| "token count overflow".to_owned())?);
            Ok(tokens)
        }
        #[cfg(not(native_llama_cpp))]
        {
            let _ = (text, parse_special);
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
        if !self.profile.embedding {
            return Err(
                "the native adapter does not implement this model's embedding mode".to_owned(),
            );
        }
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
    #[allow(clippy::too_many_lines)]
    fn format_prompt(&self, prompt: &str) -> Result<FormattedPrompt, String> {
        unsafe {
            let template =
                ffi::llama_model_chat_template(self.pointer.cast_const(), std::ptr::null());
            if template.is_null() {
                return Ok(FormattedPrompt {
                    text: prompt.to_owned(),
                    cache_prefix: None,
                    constrain_json_tools: false,
                });
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
            let mut enable_thinking = None;
            let mut constrain_json_tools = false;
            let entries = entries
                .into_iter()
                .map(|(role, content)| {
                    if role == "__talvik_template" {
                        let value = content
                            .strip_prefix("enable_thinking=")
                            .ok_or_else(|| "invalid chat template option".to_owned())?
                            .parse::<bool>()
                            .map_err(|_| "invalid chat template option".to_owned())?;
                        enable_thinking = Some(value);
                        return Ok(None);
                    }
                    let entry = if role == "__talvik_tools" {
                        constrain_json_tools = self.profile.tool_format == Some(oid_model_runner::ToolFormat::XmlJson);
                        let format = match self.profile.tool_format {
                            Some(oid_model_runner::ToolFormat::XmlJson) => "Emit each call as <tool_call>{\"name\":\"tool_name\",\"arguments\":{\"argument_name\":\"argument_value\"}}</tool_call>. The arguments must be valid JSON.",
                            Some(oid_model_runner::ToolFormat::XmlFunction) => "Emit each call as <tool_call>\n<function=tool_name>\n<parameter=argument_name>\nargument_value\n</parameter>\n</function>\n</tool_call>. Repeat parameter blocks for every argument. Emit literal text for string parameters and valid JSON for numbers, booleans, arrays or objects, matching the schema.",
                            None => return Err("the loaded model has no supported tool output format".to_owned()),
                        };
                        let declarations = tool_declarations(&CStr::from_ptr(template).to_string_lossy(), content)?;
                        ("system".to_owned(), format!("{declarations}\n\n{format} Match argument names and types to the selected tool schema and include every required property. Never omit required arguments. When the user's request requires an external action, invoke an available tool that performs it; narration does not execute an action. Do not end with a promise to perform an action instead of calling the tool. Respect the user's requested scope, including exclusions such as no tests or no edits. Use tool results already present in the conversation to answer; do not repeat completed calls unless new data is needed. Never invent tool results or claim execution without a corresponding result."))
                    } else if role == "__talvik_tool_calls" {
                        ("assistant".to_owned(), render_tool_history(content, self.profile.tool_format)?)
                    } else if role == "tool" && self.profile.tool_format.is_some()
                        && CStr::from_ptr(template).to_bytes().windows(b"<tool_response>".len()).any(|part| part == b"<tool_response>")
                        && CStr::from_ptr(template).to_bytes().windows(b"</tool_response>".len()).any(|part| part == b"</tool_response>") {
                        // The role/content C API does not implement the full
                        // Jinja tool branch. Honor its declared response wrapper.
                        ("user".to_owned(), format!("<tool_response>\n{content}\n</tool_response>"))
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
                    Ok(Some(entry))
                })
                .collect::<Result<Vec<_>, String>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            render_chat_prompt(CStr::from_ptr(template), &entries, enable_thinking).map(
                |mut prompt| {
                    // Never constrain hidden reasoning as if it were a tool call.
                    let reasoning_disabled = self.profile.reasoning_format.is_none()
                        || (self.profile.thinking_control && enable_thinking == Some(false));
                    prompt.constrain_json_tools = constrain_json_tools && reasoning_disabled;
                    prompt
                },
            )
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
    #[allow(clippy::too_many_arguments)]
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
        callback: F,
        cancelled: &AtomicBool,
    ) -> Result<NativeGenerationStatistics, String>
    where
        F: FnMut(&str) -> bool,
    {
        self.generate_stream_with_grammar_and_images(
            prompt,
            context_size,
            max_tokens,
            temperature,
            top_p,
            top_k,
            seed,
            stop_sequences,
            grammar,
            &[],
            callback,
            cancelled,
        )
    }

    /// Generate text from a prompt and optional encoded image inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when input decoding, multimodal preprocessing, context
    /// creation, sampling, or native decoding fails.
    #[allow(
        clippy::missing_panics_doc,
        clippy::too_many_arguments,
        clippy::too_many_lines
    )]
    pub fn generate_stream_with_grammar_and_images<F>(
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
        images: &[(String, &[u8])],
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
                images,
                callback,
                cancelled,
            );
            Err("native llama.cpp is unavailable".to_owned())
        }
        #[cfg(native_llama_cpp)]
        {
            let inference_started = Instant::now();
            #[cfg(native_mtmd)]
            let _mtmd_cancellation_guard = if images.is_empty() {
                None
            } else {
                let relay = self
                    .mtmd_relay
                    .as_ref()
                    .ok_or_else(|| {
                        "the loaded model has no configured vision projector".to_owned()
                    })?
                    .as_ref();
                relay
                    .current
                    .store(std::ptr::from_ref(cancelled).cast_mut(), Ordering::SeqCst);
                Some(CancellationGuard(std::ptr::from_ref(relay)))
            };
            if cancelled.load(Ordering::SeqCst) {
                return Err("generation cancelled".to_owned());
            }
            let formatted = self.format_prompt(prompt)?;
            let constrain_json_tools = grammar.is_none() && formatted.constrain_json_tools;
            let formatted_prompt = formatted.text;
            #[allow(unused_assignments)]
            let mut prompt_tokens = Vec::new();
            let prompt_count: usize;
            #[cfg(native_mtmd)]
            let mut multimodal_chunks = None;
            #[cfg(native_mtmd)]
            let mut multimodal_bitmaps = None;
            if images.is_empty() {
                prompt_tokens = self.tokenize_generation(&formatted_prompt)?;
                prompt_count = prompt_tokens.len();
            } else {
                #[cfg(not(native_mtmd))]
                return Err("this build does not include llama.cpp MTMD support".to_owned());
                #[cfg(native_mtmd)]
                {
                    let mtmd = self.mtmd.ok_or_else(|| {
                        "the loaded model has no configured vision projector".to_owned()
                    })?;
                    let decoded = decode_image_inputs(images)?;
                    if cancelled.load(Ordering::SeqCst) {
                        return Err("generation cancelled".to_owned());
                    }
                    let mut owned_bitmaps = OwnedMtmdBitmaps(Vec::with_capacity(decoded.len()));
                    for (width, height, rgb) in &decoded {
                        let bitmap =
                            unsafe { mtmd_ffi::mtmd_bitmap_init(*width, *height, rgb.as_ptr()) };
                        if bitmap.is_null() {
                            return Err("MTMD could not create an image bitmap".to_owned());
                        }
                        owned_bitmaps.0.push(bitmap);
                    }
                    let bitmap_pointers = owned_bitmaps
                        .0
                        .iter()
                        .map(|bitmap| bitmap.cast_const())
                        .collect::<Vec<_>>();
                    let formatted_prompt_cstr = CString::new(formatted_prompt.as_bytes())
                        .map_err(|_| "prompt contains NUL".to_owned())?;
                    let input_text = MtmdInputText {
                        text: formatted_prompt_cstr.as_ptr(),
                        text_len: formatted_prompt.len(),
                        add_special: true,
                        parse_special: true,
                    };
                    let chunks = unsafe { mtmd_ffi::mtmd_input_chunks_init() };
                    if chunks.is_null() {
                        return Err("MTMD could not allocate input chunks".to_owned());
                    }
                    let chunks = OwnedMtmdChunks(chunks);
                    let tokenize_result = unsafe {
                        mtmd_ffi::mtmd_tokenize(
                            mtmd,
                            chunks.0,
                            std::ptr::from_ref(&input_text),
                            bitmap_pointers.as_ptr(),
                            bitmap_pointers.len(),
                        )
                    };
                    if tokenize_result != 0 {
                        return Err(format!("MTMD could not tokenize images: {tokenize_result}"));
                    }
                    if cancelled.load(Ordering::SeqCst) {
                        return Err("generation cancelled".to_owned());
                    }
                    prompt_count = unsafe { mtmd_ffi::mtmd_helper_get_n_tokens(chunks.0) };
                    if prompt_count == 0 {
                        return Err("MTMD produced an empty prompt".to_owned());
                    }
                    multimodal_chunks = Some(chunks);
                    multimodal_bitmaps = Some(owned_bitmaps);
                }
            }
            eprintln!(
                "llama.cpp generation prompt_tokens={prompt_count} context_size={context_size} max_tokens={max_tokens}"
            );
            let prompt_count_u64 = prompt_count as u64;
            let max_context =
                usize::try_from(context_size).map_err(|_| "context too large".to_owned())?;
            if prompt_count >= max_context {
                return Err("prompt exceeds context size".to_owned());
            }
            let recurrent = images.is_empty()
                && self.profile.cache == oid_model_runner::CacheStrategy::Recurrent;
            let mut checkpoint_positions = Vec::new();
            if recurrent && prompt_tokens.len() > 1 {
                if let Some(prefix_text) = formatted.cache_prefix {
                    // A failed optimization must not reject an otherwise valid
                    // request. Exact token validation also handles BPE merges.
                    if let Ok(prefix) = self.tokenize_generation(&prefix_text) {
                        if let Some(position) =
                            verified_checkpoint_position(&prefix, &prompt_tokens)
                        {
                            checkpoint_positions.push(position);
                        }
                    }
                }
                checkpoint_positions.push(prompt_tokens.len() - 1);
                checkpoint_positions.sort_unstable();
                checkpoint_positions.dedup();
            }
            let trace_prefill = std::env::var("TALVIK_TRACE_PREFILL")
                .is_ok_and(|value| matches!(value.as_str(), "1" | "true"));
            // Preserve the advertised logical limit while avoiding unused KV
            // allocation. Power-of-two headroom keeps nearby turns reusable;
            // requests that outgrow the retained capacity rebuild safely.
            // Multimodal position accounting retains its qualified full limit.
            let required_context_size =
                required_text_context(context_size, prompt_count, max_tokens);
            let allocated_context_size = if images.is_empty() {
                text_context_capacity(context_size, prompt_count, max_tokens)
            } else {
                context_size
            };
            let physical_batch = allocated_context_size.min(512);
            let mut persistent = self
                .context
                .lock()
                .map_err(|_| "native context lock poisoned".to_owned())?;
            let mut cache_hit = false;
            let mut reused_prompt_tokens = 0_u64;
            let reuse_len = if images.is_empty() {
                persistent
                    .as_ref()
                    .filter(|cached| cached.allocated_context_size >= required_context_size)
                    .and_then(|cached| {
                        compatible_prompt_reuse_len(
                            cached,
                            &prompt_tokens,
                            context_size,
                            self.profile.cache,
                        )
                    })
            } else {
                None
            };
            let (context, mut prompt_position) = if let Some(reuse_len) = reuse_len {
                let cached = persistent
                    .as_mut()
                    .ok_or_else(|| "native context cache disappeared".to_owned())?;
                cached
                    .relay
                    .current
                    .store(std::ptr::from_ref(cancelled).cast_mut(), Ordering::SeqCst);
                let restored = cached
                    .checkpoints
                    .iter()
                    .find(|checkpoint| checkpoint.tokens.len() == reuse_len)
                    .is_none_or(|checkpoint| unsafe {
                        ffi::llama_state_seq_set_data_ext(
                            cached.pointer,
                            checkpoint.state.as_ptr(),
                            checkpoint.state.len(),
                            0,
                            PARTIAL_STATE_ONLY,
                        ) == checkpoint.state.len()
                    });
                let memory = unsafe { ffi::llama_get_memory(cached.pointer) };
                let removed = restored
                    && !memory.is_null()
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
                    cached.checkpoints.clear();
                    (cached.pointer, 0)
                }
            } else {
                if let Some(cached) = persistent.take() {
                    drop(cached);
                }
                let mut context_params = ContextParams {
                    n_ctx: allocated_context_size,
                    // Keep graph reservation bounded. Prompts larger than this
                    // physical batch are decoded incrementally below.
                    n_batch: physical_batch,
                    n_ubatch: physical_batch,
                    n_outputs_max: 1,
                    n_outputs_max_per_seq: 1,
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
                eprintln!("llama.cpp context allocation requested_tokens={context_size} allocated_tokens={allocated_context_size}");
                persistent.replace(PersistentContext {
                    pointer,
                    context_size,
                    allocated_context_size,
                    prompt_tokens: Vec::new(),
                    checkpoints: Vec::new(),
                    relay,
                });
                (pointer, 0)
            };
            if let Some(cached) = persistent.as_mut() {
                cached.checkpoints.retain(|checkpoint| {
                    prompt_tokens.starts_with(&checkpoint.tokens)
                        && checkpoint_positions.contains(&checkpoint.tokens.len())
                });
            }
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
            let vocabulary = unsafe { ffi::llama_model_get_vocab(self.pointer.cast_const()) };
            let mut samplers = Vec::with_capacity(6);
            if constrain_json_tools {
                samplers.push(unsafe { tool_grammar::init(vocabulary) });
            }
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
                    ffi::llama_sampler_init_penalties(
                        ffi::llama_vocab_n_tokens(vocabulary),
                        64,
                        1.1,
                        0.0,
                        0.0,
                    ),
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
            let generation_position = if images.is_empty() {
                while prompt_position < prompt_tokens.len() {
                    let mut end = (prompt_position
                        + usize::try_from(physical_batch)
                            .map_err(|_| "invalid batch size".to_owned())?)
                    .min(prompt_tokens.len());
                    if let Some(boundary) = checkpoint_positions
                        .iter()
                        .copied()
                        .find(|position| *position > prompt_position && *position < end)
                    {
                        end = boundary;
                    }
                    let final_batch = end == prompt_tokens.len();
                    let mut batch =
                        make_batch(&mut prompt_tokens[prompt_position..end], prompt_position)?;
                    if !final_batch {
                        batch.logits.fill(0);
                    }
                    let decode_result = unsafe { ffi::llama_decode(context, batch.batch) };
                    prompt_position = end;
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
                    if trace_prefill {
                        eprintln!("Talvik prefill progress processed={prompt_position} total={} elapsed_ms={} reused={reused_prompt_tokens}", prompt_tokens.len(), inference_started.elapsed().as_millis());
                    }
                    if checkpoint_positions.contains(&prompt_position) {
                        if let Some(cached) = persistent.as_mut() {
                            save_prompt_checkpoint(cached, &prompt_tokens[..prompt_position]);
                        }
                    }
                }
                prompt_position
            } else {
                #[cfg(native_mtmd)]
                {
                    let mtmd = self.mtmd.ok_or_else(|| {
                        "the loaded model has no configured vision projector".to_owned()
                    })?;
                    let chunks = multimodal_chunks
                        .as_ref()
                        .ok_or_else(|| "MTMD input chunks were not initialized".to_owned())?;
                    let mut new_n_past = 0_i32;
                    let decode_result = unsafe {
                        mtmd_ffi::mtmd_helper_eval_chunks(
                            mtmd,
                            context,
                            chunks.0,
                            0,
                            0,
                            i32::try_from(physical_batch)
                                .map_err(|_| "invalid batch size".to_owned())?,
                            true,
                            std::ptr::from_mut(&mut new_n_past),
                        )
                    };
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
                            return Err(decode_error("multimodal prompt", decode_result));
                        }
                    }
                    drop(multimodal_bitmaps);
                    usize::try_from(new_n_past)
                        .map_err(|_| "MTMD returned a negative context position".to_owned())?
                }
                #[cfg(not(native_mtmd))]
                {
                    unreachable!("images are rejected when MTMD is unavailable")
                }
            };
            let prompt_eval_time = inference_started.elapsed();
            let vocab = unsafe { ffi::llama_model_get_vocab(self.pointer.cast_const()) };
            let mut pending_utf8 = Vec::new();
            let mut marker_filter = EndMarkerFilter::default();
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
                let (text, mut terminal) = Self::terminal_piece_prefix(&text);
                pending_utf8.extend_from_slice(text);
                let (delta, marker_seen) =
                    marker_filter.push(&take_complete_utf8(&mut pending_utf8));
                terminal |= marker_seen;
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
                if prompt_count_u64 + generated >= u64::from(context_size) {
                    termination = NativeGenerationTermination::ContextLength;
                    break;
                }
                let mut next = [token];
                let result = unsafe {
                    ffi::llama_decode(
                        context,
                        make_batch(
                            &mut next,
                            generation_position
                                .checked_add(
                                    usize::try_from(generated.saturating_sub(1))
                                        .map_err(|_| "token position is too large".to_owned())?,
                                )
                                .ok_or_else(|| "token position is too large".to_owned())?,
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
                let (final_text, _) = marker_filter.push(&String::from_utf8_lossy(&pending_utf8));
                if !final_text.is_empty() && !callback(&final_text) {
                    cancelled_result = true;
                }
            }
            if !cancelled_result {
                let held = marker_filter.finish();
                if !held.is_empty() && !callback(&held) {
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
                cached.prompt_tokens = if images.is_empty() {
                    prompt_tokens
                } else {
                    Vec::new()
                };
            }
            Ok(NativeGenerationStatistics {
                prompt_tokens: prompt_count_u64,
                generated_tokens: generated,
                context_tokens: prompt_count_u64 + generated,
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
    #[cfg(native_mtmd)]
    use std::io::Cursor;

    #[test]
    fn tool_history_uses_the_selected_protocol_and_preserves_content_and_ids() {
        use oid_model_runner::ToolFormat;
        let history = serde_json::json!({"content":"Inspecting.","calls":[{"id":"call_123","function":{"name":"read_file","arguments":"{\"path\":\"a<世界>\",\"count\":2}"}}]}).to_string();
        let json = super::render_tool_history(&history, Some(ToolFormat::XmlJson)).unwrap();
        assert!(json.starts_with("Inspecting.\nTool call ID: \"call_123\""));
        assert!(json.contains("<tool_call>{"));
        assert!(!json.contains("<tool_calls>"));
        assert!(json.contains("a<世界>"));
        let function = super::render_tool_history(&history, Some(ToolFormat::XmlFunction)).unwrap();
        assert!(function.contains("<function=read_file>"));
        assert!(function.contains("<parameter=count>\n2\n</parameter>"));
        assert!(function.contains("a&lt;世界&gt;"));
        assert!(super::render_tool_history(&history, None).is_err());
        assert!(super::render_tool_history("{}", Some(ToolFormat::XmlJson)).is_err());
    }

    #[test]
    fn text_context_capacity_reserves_output_and_bounded_growth_headroom() {
        assert_eq!(super::text_context_capacity(32768, 9288, 2048), 16384);
        assert_eq!(super::text_context_capacity(32768, 6032, 2048), 16384);
        assert_eq!(super::required_text_context(32768, 6032, 2048), 8080);

        assert_eq!(super::text_context_capacity(32768, 12, 16), 512);
        assert_eq!(super::text_context_capacity(128, 20, 8), 128);
        assert_eq!(super::text_context_capacity(5000, 4900, 2048), 5000);
        assert_eq!(
            super::text_context_capacity(32768, usize::MAX, u32::MAX),
            32768
        );
        assert_eq!(super::text_context_capacity(0, 0, 0), 0);
    }

    #[test]
    fn auxiliary_accelerators_are_not_cpu_or_gpu_runners() {
        use super::{classify_native_device as classify, NativeBackend};
        assert_eq!(classify(3, "blas"), None);
        assert_eq!(classify(3, "cpu"), None);
        assert_eq!(classify(4, "rpc"), None);
        assert_eq!(classify(1, "unknown"), None);
        assert_eq!(classify(0, "cpu"), Some(NativeBackend::Cpu));
        assert_eq!(classify(1, "cuda"), Some(NativeBackend::Cuda));
        assert_eq!(classify(2, "metal"), Some(NativeBackend::Metal));
        assert_eq!(classify(2, "vulkan"), Some(NativeBackend::Vulkan));
    }

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
        use oid_model_runner::CacheStrategy::Attention;
        let cached = super::PersistentContext {
            pointer: std::ptr::null_mut(),
            context_size: 512,
            allocated_context_size: 512,
            prompt_tokens: vec![1, 2, 3],
            checkpoints: Vec::new(),
            relay: Box::new(super::CancellationRelay {
                current: std::sync::atomic::AtomicPtr::new(std::ptr::null_mut()),
            }),
        };
        assert_eq!(
            super::compatible_prompt_reuse_len(&cached, &[1, 2, 3], 512, Attention),
            Some(2)
        );
        assert_eq!(
            super::compatible_prompt_reuse_len(&cached, &[1, 2, 3, 4], 512, Attention),
            Some(3)
        );
        assert_eq!(
            super::compatible_prompt_reuse_len(&cached, &[1, 9, 3], 512, Attention),
            Some(1)
        );
        assert_eq!(
            super::compatible_prompt_reuse_len(&cached, &[1, 2], 512, Attention),
            Some(1)
        );
        assert_eq!(
            super::compatible_prompt_reuse_len(&cached, &[1, 2, 3], 1024, Attention),
            None
        );
        assert_eq!(
            super::compatible_prompt_reuse_len(&cached, &[9, 2], 512, Attention),
            None
        );
        assert_eq!(
            super::compatible_prompt_reuse_len(
                &cached,
                &[1, 2],
                512,
                oid_model_runner::CacheStrategy::Recurrent
            ),
            None
        );
        assert_eq!(
            super::compatible_prompt_reuse_len(
                &cached,
                &[1, 2],
                512,
                oid_model_runner::CacheStrategy::Disabled
            ),
            None
        );
        std::mem::forget(cached);
    }

    #[cfg(native_llama_cpp)]
    #[test]
    #[ignore = "requires TALVIK_TEST_PROFILE_GGUF for metadata/tokenizer qualification"]
    fn loaded_profile_applies_tokenizer_metadata_without_duplicate_bos() {
        let path = std::env::var("TALVIK_TEST_PROFILE_GGUF").expect("GGUF fixture");
        super::initialize().expect("initialize");
        let model = super::load_model(&path, None, super::cpu_load_options()).expect("load");
        assert_ne!(model.profile().architecture, "unknown");
        assert!(model.profile().completion);
        assert!(model.profile().chat);
        assert!(model.profile().context_length > 0);
        let rendered = model.format_prompt("Reply OK.").expect("format");
        let tokens = model
            .tokenize_generation(&rendered.text)
            .expect("generation tokens");
        let literal = model
            .tokenize_with_special(&rendered.text, true)
            .expect("literal tokens");
        let bos = unsafe {
            super::ffi::llama_vocab_bos(super::ffi::llama_model_get_vocab(
                model.pointer.cast_const(),
            ))
        };
        if model.profile().tokenizer_adds_bos {
            assert_eq!(tokens.first(), Some(&bos));
            assert_eq!(tokens.iter().filter(|t| **t == bos).count(), 1);
            let bos_text = unsafe {
                std::ffi::CStr::from_ptr(super::ffi::llama_vocab_get_text(
                    super::ffi::llama_model_get_vocab(model.pointer.cast_const()),
                    bos,
                ))
            }
            .to_string_lossy();
            let explicit = model
                .tokenize_generation(&format!("{bos_text}Reply OK."))
                .expect("explicit BOS");
            assert_eq!(explicit.first(), Some(&bos));
            assert_eq!(explicit.iter().filter(|t| **t == bos).count(), 1);
        } else {
            assert_eq!(tokens, literal);
        }
        drop(model);
        super::shutdown();
    }

    #[test]
    fn template_output_formats_require_a_supported_protocol() {
        use oid_model_runner::{ReasoningFormat, ToolFormat};
        assert_eq!(
            super::output_formats("tools tool_calls unknown", false),
            (true, None, None, false)
        );
        assert_eq!(
            super::output_formats("tools <tool_call>{name:arguments}</tool_call>", false).1,
            Some(ToolFormat::XmlJson)
        );
        assert_eq!(
            super::output_formats(
                "tools <tool_call><function=f><parameter=x></parameter></function></tool_call>",
                false
            )
            .1,
            Some(ToolFormat::XmlFunction)
        );
        assert_eq!(
            super::output_formats("plain chat", true).2,
            Some(ReasoningFormat::ThinkTags)
        );
        assert!(!super::output_formats("enable_thinking without marker", true).3);
        assert!(super::output_formats("enable_thinking <think></think>", false).3);
        assert_eq!(
            super::output_formats("plain chat", false),
            (false, None, None, false)
        );
    }

    #[test]
    fn tool_declarations_honor_template_envelopes_without_changing_schemas() {
        let tools = r#"[{"type":"function","function":{"name":"run_in_terminal","parameters":{"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}}}]"#;
        let native =
            super::tool_declarations("<tools></tools> <tool_call> tool | tojson", tools).unwrap();
        let signatures = native
            .split("<tools>\n")
            .nth(1)
            .unwrap()
            .split("\n</tools>")
            .next()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(signatures).unwrap(),
            serde_json::from_str::<serde_json::Value>(tools).unwrap()[0]
        );
        assert_eq!(
            super::tool_declarations("<tool_call> <function=", tools).unwrap(),
            format!("Tools: {tools}")
        );
        assert!(super::tool_declarations("<tools></tools> <tool_call> tojson", "bad").is_err());
    }

    #[test]
    fn checkpoint_boundary_requires_an_exact_nontrivial_token_prefix() {
        let prefix = vec![7; 64];
        let mut prompt = prefix.clone();
        prompt.extend([8, 9]);
        assert_eq!(
            super::verified_checkpoint_position(&prefix, &prompt),
            Some(64)
        );
        assert_eq!(super::verified_checkpoint_position(&[], &prompt), None);
        assert_eq!(
            super::verified_checkpoint_position(&prefix[..63], &prompt),
            None
        );
        assert_eq!(super::verified_checkpoint_position(&prompt, &prompt), None);
        // A byte prefix may tokenize differently at its final boundary.
        let mut merged = prefix.clone();
        merged[63] = 99;
        assert_eq!(super::verified_checkpoint_position(&merged, &prompt), None);
    }

    #[cfg(native_llama_cpp)]
    #[test]
    fn cache_boundary_comes_from_structured_roles_across_template_families() {
        let entries = vec![
            ("system".to_owned(), "Static instructions.".to_owned()),
            (
                "user".to_owned(),
                "Content with <|im_start|>user\n and USER: inside it.".to_owned(),
            ),
        ];
        for family in ["chatml", "llama3", "deepseek", "vicuna"] {
            let template = std::ffi::CString::new(family).expect("template");
            let prompt = super::render_chat_prompt(&template, &entries, None).expect("render");
            let prefix = prompt.cache_prefix.expect("prefix-stable template");
            assert_eq!(
                prefix,
                super::render_template_messages(&template, &entries[..1], false).expect("prefix")
            );
            assert!(prompt.text.starts_with(&prefix));
            assert!(!prefix.contains("Content with"));
            assert!(prompt.text.contains("Content with"));
            // A changed user body must not change its static boundary.
            let mut changed = entries.clone();
            changed[1].1 = "A different request.".to_owned();
            assert_eq!(
                super::render_chat_prompt(&template, &changed, None)
                    .expect("changed")
                    .cache_prefix
                    .as_deref(),
                Some(prefix.as_str())
            );
            let user_only =
                super::render_chat_prompt(&template, &entries[1..], None).expect("user only");
            assert!(user_only.cache_prefix.is_none());
        }
    }

    #[cfg(native_llama_cpp)]
    #[test]
    fn cache_boundary_preserves_history_before_the_last_user_turn() {
        let template = std::ffi::CString::new("llama3").expect("template");
        let entries = vec![
            ("system".to_owned(), "Instructions.".to_owned()),
            ("user".to_owned(), "Previous question.".to_owned()),
            ("assistant".to_owned(), "Previous answer.".to_owned()),
            ("user".to_owned(), "New question.".to_owned()),
        ];
        let prompt = super::render_chat_prompt(&template, &entries, None).expect("render");
        let prefix = prompt.cache_prefix.expect("history prefix");
        assert_eq!(
            prefix,
            super::render_template_messages(&template, &entries[..3], false).expect("prefix")
        );
        assert!(prefix.contains("Previous answer."));
        assert!(!prefix.contains("New question."));
        assert!(prompt.text.starts_with(&prefix));
    }

    #[cfg(native_llama_cpp)]
    #[test]
    fn merged_system_template_falls_back_without_rejecting_generation() {
        let template = std::ffi::CString::new("gemma").expect("template");
        let entries = vec![
            ("system".to_owned(), "Static instructions.".to_owned()),
            ("user".to_owned(), "Question.".to_owned()),
        ];
        let prompt = super::render_chat_prompt(&template, &entries, None).expect("render");
        assert!(prompt.cache_prefix.is_none());
        assert!(prompt.text.contains("Static instructions.\n\nQuestion."));
        assert!(prompt.text.ends_with("<start_of_turn>model\n"));
        let invalid = std::ffi::CString::new("unsupported-talvik-template").expect("template");
        assert!(super::render_chat_prompt(&invalid, &entries, None).is_err());
    }

    #[cfg(native_llama_cpp)]
    #[test]
    #[ignore = "requires a hybrid/recurrent GGUF supplied through TALVIK_TEST_RECURRENT_GGUF"]
    fn recurrent_checkpoint_reuse_matches_cold_output_and_clears_on_cancel() {
        use std::sync::atomic::AtomicBool;
        let path = std::env::var("TALVIK_TEST_RECURRENT_GGUF").expect("recurrent GGUF fixture");
        super::initialize().expect("initialize");
        let model = super::load_model(&path, None, super::cpu_load_options()).expect("load");
        assert!(unsafe {
            super::ffi::llama_model_is_hybrid(model.pointer.cast_const())
                || super::ffi::llama_model_is_recurrent(model.pointer.cast_const())
        });
        // A user-only template has no static prefix before its first marker.
        // Do not pass that empty prefix through the native token-count probe.
        model
            .generate_stream(
                "\u{001e}__talvik_template\nenable_thinking=false\u{001e}user\nReply OK.",
                2048,
                4,
                0.0,
                1.0,
                40,
                42,
                &[],
                |_| true,
                &AtomicBool::new(false),
            )
            .expect("user-only template has no static prefix");
        model.reset_context().expect("reset short prompt");
        let prefix = "A completed record.\n".repeat(16);
        let prompt = |question: &str| {
            format!("\u{001e}__talvik_template\nenable_thinking=false\u{001e}system\n{prefix}Answer the arithmetic question with a number only.\u{001e}user\n{question}")
        };
        let run = |question: &str| {
            let mut text = String::new();
            let stats = model
                .generate_stream(
                    &prompt(question),
                    2048,
                    16,
                    0.0,
                    1.0,
                    40,
                    42,
                    &[],
                    |part| {
                        text.push_str(part);
                        true
                    },
                    &AtomicBool::new(false),
                )
                .expect("generation");
            (text, stats)
        };
        let first = run("What is 2 + 2?");
        let repeat = run("What is 2 + 2?");
        assert_eq!(first.0.trim(), "4");
        assert_eq!(repeat.0, first.0);
        assert!(repeat.1.cache_hit);
        assert!(repeat.1.reused_prompt_tokens >= repeat.1.prompt_tokens - 2);
        let changed = run("What is 3 + 3?");
        assert!(changed.1.cache_hit);
        assert!(changed.1.reused_prompt_tokens > 32);
        model.reset_context().expect("reset");
        let cold_changed = run("What is 3 + 3?");
        assert!(!cold_changed.1.cache_hit);
        assert_eq!(changed.0, cold_changed.0);
        assert_eq!(changed.0.trim(), "6");
        let cancellation = AtomicBool::new(false);
        assert!(model
            .generate_stream(
                &prompt("What is 3 + 3?"),
                2048,
                32,
                0.0,
                1.0,
                40,
                42,
                &[],
                |_| {
                    cancellation.store(true, std::sync::atomic::Ordering::SeqCst);
                    false
                },
                &cancellation
            )
            .is_err());
        let recovered = run("What is 3 + 3?");
        assert!(!recovered.1.cache_hit);
        assert_eq!(recovered.0, cold_changed.0);
        let context = model.context.lock().expect("context");
        let cache = context.as_ref().expect("cache");
        assert!(cache.checkpoints.len() <= 2);
        assert!(
            cache
                .checkpoints
                .iter()
                .map(|checkpoint| checkpoint.state.len())
                .sum::<usize>()
                <= super::MAX_TOTAL_CHECKPOINT_BYTES
        );
        drop(context);
        drop(model);
        super::shutdown();
    }

    #[cfg(native_llama_cpp)]
    #[test]
    #[ignore = "requires a real GGUF supplied through MARINA_TEST_GGUF"]
    fn embedded_chat_template_formats_a_user_prompt() {
        let model_path = std::env::var("MARINA_TEST_GGUF")
            .expect("MARINA_TEST_GGUF must point to a real GGUF for this test");
        super::initialize().expect("native backend should initialize");
        let model = super::load_model(&model_path, None, super::cpu_load_options())
            .expect("test GGUF should load");
        let formatted = model
            .format_prompt("Reply with exactly: MARINA_OK")
            .expect("embedded chat template should format the prompt")
            .text;
        assert!(formatted.contains("<|im_start|>user"));
        assert!(formatted.contains("Reply with exactly: MARINA_OK"));
        assert!(formatted.contains("<|im_start|>assistant"));
        let marker = model
            .tokenize_with_special("<|im_start|>", true)
            .expect("special marker");
        assert_eq!(marker.len(), 1);
        let prompt_tokens = model
            .tokenize_with_special(&formatted, true)
            .expect("formatted tokens");
        assert!(prompt_tokens.contains(&marker[0]));
        assert!(
            model
                .tokenize("<|im_start|>")
                .expect("literal marker")
                .len()
                > 1
        );
        drop(model);
        super::shutdown();
    }

    #[cfg(native_mtmd)]
    #[test]
    #[ignore = "requires SmolVLM GGUF and projector supplied through TALVIK_TEST_VISION_GGUF and TALVIK_TEST_VISION_PROJECTOR"]
    fn vision_input_uses_image_content_and_recovers_after_cancellation() {
        let model_path = std::env::var("TALVIK_TEST_VISION_GGUF")
            .expect("TALVIK_TEST_VISION_GGUF must point to a vision GGUF");
        let projector_path = std::env::var("TALVIK_TEST_VISION_PROJECTOR")
            .expect("TALVIK_TEST_VISION_PROJECTOR must point to its matching projector GGUF");
        let mut fixture = image::RgbImage::from_pixel(64, 64, image::Rgb([255, 255, 255]));
        for y in 16..48 {
            for x in 16..48 {
                fixture.put_pixel(x, y, image::Rgb([255, 0, 0]));
            }
        }
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(fixture)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .expect("encode deterministic image fixture");
        let image_bytes = encoded.into_inner();
        let images = [("image/png".to_owned(), image_bytes.as_slice())];

        super::initialize().expect("native backend should initialize");
        let model = super::load_model(
            &model_path,
            Some(&projector_path),
            super::cpu_load_options(),
        )
        .expect("vision model and projector should load");
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let mut answer = String::new();
        model
            .generate_stream_with_grammar_and_images(
                "What color is the square? Answer with one color. <__media__>",
                512,
                32,
                0.0,
                0.9,
                40,
                42,
                &[],
                None,
                &images,
                |piece| {
                    answer.push_str(piece);
                    true
                },
                &cancelled,
            )
            .expect("image-dependent generation should complete");
        assert!(answer.to_ascii_lowercase().contains("red"), "{answer:?}");

        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let cancelled_generation = model.generate_stream_with_grammar_and_images(
            "Describe every object in the image in detail. <__media__>",
            512,
            256,
            0.0,
            0.9,
            40,
            43,
            &[],
            None,
            &images,
            |_| {
                cancellation.store(true, std::sync::atomic::Ordering::SeqCst);
                false
            },
            &cancellation,
        );
        assert!(cancelled_generation.is_err());

        let mut recovered = String::new();
        model
            .generate_stream_with_grammar_and_images(
                "What color is the square? Answer with one color. <__media__>",
                512,
                32,
                0.0,
                0.9,
                40,
                44,
                &[],
                None,
                &images,
                |piece| {
                    recovered.push_str(piece);
                    true
                },
                &cancelled,
            )
            .expect("image generation should recover after cancellation");
        assert!(
            recovered.to_ascii_lowercase().contains("red"),
            "{recovered:?}"
        );
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
        let model = super::load_model(&model_path, None, super::cpu_load_options())
            .expect("test GGUF should load");
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
        let model = super::load_model(&model_path, None, super::cpu_load_options())
            .expect("test GGUF should load");
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

        let run_text = |text: &str| {
            let mut output = String::new();
            let stats = model
                .generate_stream(
                    text,
                    128,
                    8,
                    0.0,
                    0.95,
                    40,
                    42,
                    &[],
                    |piece| {
                        output.push_str(piece);
                        true
                    },
                    &cancelled,
                )
                .expect("prefix probe");
            (output, stats)
        };
        model.reset_context().expect("start changed-turn test cold");
        run_text("Answer this question briefly: what is two plus two?");
        let changed = run_text("Answer this question briefly: what is three plus three?");
        assert!(changed.1.cache_hit);
        model.reset_context().expect("cold comparison");
        let cold_changed = run_text("Answer this question briefly: what is three plus three?");
        assert_eq!(changed.0, cold_changed.0);
        let shorter = run_text("Answer this question briefly: why?");
        assert!(shorter.1.cache_hit);
        model.reset_context().expect("shortened cold comparison");
        let cold_shorter = run_text("Answer this question briefly: why?");
        assert_eq!(shorter.0, cold_shorter.0);

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
        assert!(unrelated.reused_prompt_tokens < unrelated.prompt_tokens);
        let after_unrelated = model
            .generate_stream(prompt, 128, 1, 0.0, 0.95, 40, 42, &[], |_| true, &cancelled)
            .expect("original conversation should remain isolated");
        assert!(after_unrelated.reused_prompt_tokens < after_unrelated.prompt_tokens);

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
        let run_capacity = |text: &str| {
            let mut output = String::new();
            let stats = model
                .generate_stream(
                    text,
                    4096,
                    1,
                    0.0,
                    0.95,
                    40,
                    42,
                    &[],
                    |piece| {
                        output.push_str(piece);
                        true
                    },
                    &cancelled,
                )
                .expect("capacity probe");
            (output, stats)
        };
        model.reset_context().expect("capacity probe reset");
        let small = "\u{001e}system\nReply OK only.\u{001e}user\nShort record.";
        run_capacity(small);
        assert_eq!(
            model
                .context
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .allocated_context_size,
            512
        );
        // Fresh allocation would reserve 1K, but the existing 512-token
        // context can still satisfy this request's actual maximum.
        let fits = model
            .generate_stream(
                small,
                4096,
                400,
                0.0,
                0.95,
                40,
                42,
                &[],
                |_| true,
                &cancelled,
            )
            .expect("reuse within actual capacity");
        assert!(fits.cache_hit);
        assert_eq!(
            model
                .context
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .allocated_context_size,
            512
        );
        let large = format!(
            "\u{001e}system\nReply OK only.\u{001e}user\n{}",
            "A completed record.\n".repeat(192)
        );
        let grown = run_capacity(&large);
        assert!(!grown.1.cache_hit, "growth must rebuild before decoding");
        let grown_capacity = model
            .context
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .allocated_context_size;
        assert!(grown_capacity > 512 && grown_capacity <= 4096);
        model.reset_context().expect("capacity cold comparison");
        assert_eq!(grown.0, run_capacity(&large).0);
        assert!(run_capacity(small).1.cache_hit);
        assert_eq!(
            model
                .context
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .allocated_context_size,
            grown_capacity,
            "shorter turns retain the existing allocation"
        );
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

    #[test]
    fn end_markers_split_across_tokens_are_removed() {
        let mut filter = super::EndMarkerFilter::default();
        let mut output = String::new();
        let mut terminal = false;
        for piece in ["11,664", " ", "|", "im", "_end", "|"] {
            let (text, seen) = filter.push(piece);
            output.push_str(&text);
            terminal |= seen;
        }
        assert_eq!(output, "11,664 ");
        assert!(terminal);
        assert_eq!(filter.finish(), "");
    }

    #[test]
    fn partial_marker_prefix_is_discarded_when_generation_ends() {
        let mut filter = super::EndMarkerFilter::default();
        let (text, seen) = filter.push("a <|im");
        assert_eq!(text, "a ");
        assert!(!seen);
        assert_eq!(filter.finish(), "");
        let (text, seen) = filter.push("plain | text");
        assert_eq!(text, "plain | text");
        assert!(!seen);
    }

    #[test]
    fn repeated_marker_prefix_is_not_leaked_at_generation_end() {
        let mut filter = super::EndMarkerFilter::default();
        let (text, seen) = filter.push("answer<<");
        assert_eq!(text, "answer");
        assert!(!seen);
        assert_eq!(filter.finish(), "");
    }

    #[test]
    fn bare_marker_match_does_not_leak_its_angle_bracket_prefix() {
        let mut filter = super::EndMarkerFilter::default();
        let mut output = String::new();
        let mut stopped = false;
        for part in ["answer", "<", "|", "im", "_end", "|", ">"] {
            let (delta, terminal) = filter.push(part);
            output.push_str(&delta);
            if terminal {
                stopped = true;
                break;
            }
        }
        output.push_str(&filter.finish());
        assert_eq!(output, "answer");
        assert!(stopped);
    }

    #[test]
    fn repeated_prefix_before_split_terminal_marker_is_not_emitted() {
        let mut filter = super::EndMarkerFilter::default();
        let mut output = String::new();
        for part in ["answer<", "<|", "im_end", "|>"] {
            let (delta, _) = filter.push(part);
            output.push_str(&delta);
        }
        output.push_str(&filter.finish());
        assert_eq!(output, "answer");
        let (output, stopped) = filter.push("answer<<|im_end|>");
        assert_eq!(output, "answer");
        assert!(stopped);
    }

    #[test]
    fn thinking_option_prefills_only_models_that_support_it() {
        let template = "enable_thinking <think>";
        let prompt = "assistant\n".to_owned();
        let disabled = super::apply_thinking_prefill(prompt.clone(), template, Some(false));
        assert_eq!(disabled, "assistant\n<think>\n\n</think>\n\n");
        assert_eq!(
            super::apply_thinking_prefill(disabled.clone(), template, Some(false)),
            disabled
        );
        assert_eq!(
            super::apply_thinking_prefill(prompt.clone(), template, Some(true)),
            "assistant\n<think>\n"
        );
        assert_eq!(
            super::apply_thinking_prefill(prompt.clone(), template, None),
            prompt
        );
        assert_eq!(
            super::apply_thinking_prefill(prompt.clone(), "chatml", Some(false)),
            prompt
        );
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

#[cfg(native_llama_cpp)]
fn model_metadata(pointer: *const c_void, key: &CStr) -> Option<String> {
    let required =
        unsafe { ffi::llama_model_meta_val_str(pointer, key.as_ptr(), std::ptr::null_mut(), 0) };
    let size = usize::try_from(required).ok()?.checked_add(1)?;
    if size > 1_048_576 {
        return None;
    }
    let mut buffer = vec![0_u8; size];
    let written = unsafe {
        ffi::llama_model_meta_val_str(pointer, key.as_ptr(), buffer.as_mut_ptr().cast(), size)
    };
    if written != required {
        return None;
    }
    buffer.truncate(size - 1);
    String::from_utf8(buffer).ok()
}

#[cfg(any(native_llama_cpp, test))]
fn output_formats(
    template: &str,
    think_tokens: bool,
) -> (
    bool,
    Option<oid_model_runner::ToolFormat>,
    Option<oid_model_runner::ReasoningFormat>,
    bool,
) {
    let declared = template.contains("tools") || template.contains("tool_call");
    let xml = declared && template.contains("<tool_call>") && template.contains("</tool_call>");
    let reasoning = think_tokens || (template.contains("<think>") && template.contains("</think>"));
    (
        declared,
        xml.then_some(
            if template.contains("<function=") && template.contains("<parameter=") {
                oid_model_runner::ToolFormat::XmlFunction
            } else {
                oid_model_runner::ToolFormat::XmlJson
            },
        ),
        reasoning.then_some(oid_model_runner::ReasoningFormat::ThinkTags),
        reasoning && template.contains("enable_thinking") && template.contains("<think>"),
    )
}

#[cfg(native_llama_cpp)]
fn inspect_loaded_profile(pointer: *const c_void, vision: bool) -> oid_model_runner::ModelProfile {
    use oid_model_runner::{CacheStrategy, ModelProfile};
    let architecture =
        model_metadata(pointer, c"general.architecture").unwrap_or_else(|| "unknown".to_owned());
    let template_pointer = unsafe { ffi::llama_model_chat_template(pointer, std::ptr::null()) };
    let template = if template_pointer.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(template_pointer) }
            .to_string_lossy()
            .into_owned()
    };
    let rendered = if template_pointer.is_null() {
        None
    } else {
        render_template_messages(
            unsafe { CStr::from_ptr(template_pointer) },
            &[("user".to_owned(), "Hello".to_owned())],
            true,
        )
        .ok()
    };
    let vocab = unsafe { ffi::llama_model_get_vocab(pointer) };
    let special_token = |text: &CStr| {
        let mut token = [0_i32; 1];
        let count = unsafe {
            ffi::llama_tokenize(
                vocab,
                text.as_ptr(),
                i32::try_from(text.to_bytes().len()).unwrap_or(i32::MAX),
                token.as_mut_ptr(),
                1,
                false,
                true,
            )
        };
        count == 1
            && unsafe { ffi::llama_vocab_get_attr(vocab, token[0]) & ((1 << 3) | (1 << 4)) != 0 }
    };
    let (tools_declared, tool_format, reasoning_format, thinking_control) = output_formats(
        &template,
        special_token(c"<think>") && special_token(c"</think>"),
    );
    let completion = unsafe { ffi::llama_model_has_decoder(pointer) };
    let recurrent =
        unsafe { ffi::llama_model_is_recurrent(pointer) || ffi::llama_model_is_hybrid(pointer) };
    let pooling_key = CString::new(format!("{architecture}.pooling_type")).ok();
    ModelProfile {
        architecture,
        context_length: u32::try_from(unsafe { ffi::llama_model_n_ctx_train(pointer) })
            .unwrap_or(0),
        completion,
        chat: completion && rendered.is_some(),
        // Pooled/encoder embedding admission and extraction are not yet
        // implemented by this adapter; report declaration separately.
        embedding: false,
        embedding_declared: pooling_key
            .as_ref()
            .and_then(|key| model_metadata(pointer, key))
            .is_some(),
        vision,
        cache: if recurrent {
            CacheStrategy::Recurrent
        } else if completion {
            CacheStrategy::Attention
        } else {
            CacheStrategy::Disabled
        },
        tokenizer_adds_bos: unsafe { ffi::llama_vocab_get_add_bos(vocab) },
        tools_declared,
        tool_format: tool_format.filter(|_| completion && rendered.is_some()),
        reasoning_format,
        thinking_control,
        reasoning_prefilled: rendered.is_some_and(|text| text.trim_end().ends_with("<think>")),
    }
}

impl Drop for NativeModel {
    fn drop(&mut self) {
        #[cfg(native_mtmd)]
        if let Some(context) = self.mtmd.take() {
            unsafe {
                mtmd_ffi::mtmd_free(context);
            }
        }
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
pub fn load_model(
    path: &str,
    projector_path: Option<&str>,
    options: NativeLoadOptions,
) -> Result<NativeModel, String> {
    #[cfg(native_llama_cpp)]
    unsafe {
        let path = CString::new(path).map_err(|_| "model path contains NUL".to_owned())?;
        let available = indexed_devices();
        let selected = available
            .iter()
            .position(|(_, device)| device.backend == options.backend);
        if selected.is_none() {
            return Err(format!(
                "requested {} backend is not compiled, initialized, and usable",
                options.backend.as_str()
            ));
        }
        let mut params = ffi::llama_model_default_params();
        params.n_gpu_layers = i32::try_from(options.gpu_layers).unwrap_or(i32::MAX);
        let mut native_devices = selected.map(|index| {
            vec![
                ffi::ggml_backend_dev_get(available[index].0),
                std::ptr::null_mut::<c_void>(),
            ]
        });
        if let Some(native_devices) = native_devices.as_mut() {
            params.devices = native_devices.as_mut_ptr();
        }
        let pointer = ffi::llama_model_load_from_file(path.as_ptr(), params);
        if pointer.is_null() {
            return Err("llama.cpp rejected the model".to_owned());
        }
        #[cfg(native_mtmd)]
        let (mtmd, mtmd_relay) = if let Some(projector_path) = projector_path {
            let Ok(projector_path) = CString::new(projector_path) else {
                ffi::llama_model_free(pointer);
                return Err("projector path contains NUL".to_owned());
            };
            let relay = Box::new(CancellationRelay {
                current: AtomicPtr::new(std::ptr::null_mut()),
            });
            let mut params = mtmd_ffi::mtmd_context_params_default();
            params.eval_callback = Some(mtmd_eval_callback);
            params.eval_callback_data = std::ptr::from_ref(&*relay).cast_mut().cast();
            let context = mtmd_ffi::mtmd_init_from_file(projector_path.as_ptr(), pointer, params);
            if context.is_null() {
                ffi::llama_model_free(pointer);
                return Err("llama.cpp rejected the vision projector".to_owned());
            }
            if !mtmd_ffi::mtmd_support_vision(context) {
                mtmd_ffi::mtmd_free(context);
                ffi::llama_model_free(pointer);
                return Err("the configured projector does not support vision".to_owned());
            }
            (Some(context), Some(relay))
        } else {
            (None, None)
        };
        #[cfg(not(native_mtmd))]
        if projector_path.is_some() {
            ffi::llama_model_free(pointer);
            return Err("this build does not include llama.cpp MTMD support".to_owned());
        }
        let memory_bytes = ffi::llama_model_size(pointer.cast_const());
        let device = selected.map(|index| available[index].1.name.clone());
        #[cfg(native_mtmd)]
        let vision = mtmd.is_some();
        #[cfg(not(native_mtmd))]
        let vision = false;
        let profile = inspect_loaded_profile(pointer.cast_const(), vision);
        Ok(NativeModel {
            profile,
            context: Mutex::new(None),
            #[cfg(native_mtmd)]
            mtmd,
            #[cfg(native_mtmd)]
            mtmd_relay,
            pointer,
            memory_bytes,
            placement: NativePlacement {
                backend: if options.gpu_layers == 0 {
                    NativeBackend::Cpu
                } else {
                    options.backend
                },
                device: (options.gpu_layers > 0).then_some(device).flatten(),
                requested_gpu_layers: options.gpu_layers,
                // The pinned API exposes device count, but not a reliable
                // layer count. Keep this unknown instead of echoing the
                // requested value as an observed fact.
                offloaded_gpu_layers: (options.gpu_layers == 0).then_some(0),
                accelerator_used: options.backend != NativeBackend::Cpu && options.gpu_layers > 0,
            },
        })
    }
    #[cfg(not(native_llama_cpp))]
    {
        let _ = (path, projector_path, options);
        Err("native llama.cpp is unavailable".to_owned())
    }
}
