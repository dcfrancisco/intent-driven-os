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
use std::sync::atomic::AtomicBool;
#[cfg(native_llama_cpp)]
use std::sync::atomic::Ordering;

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
    (*data.cast::<AtomicBool>()).load(Ordering::SeqCst)
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
        pub fn llama_batch_get_one(tokens: *mut i32, n_tokens: i32) -> Batch;
        pub fn llama_decode(ctx: *mut c_void, batch: Batch) -> i32;
        pub fn llama_sampler_chain_default_params() -> SamplerChainParams;
        pub fn llama_sampler_chain_init(params: SamplerChainParams) -> *mut c_void;
        pub fn llama_sampler_chain_add(chain: *mut c_void, sampler: *mut c_void);
        pub fn llama_sampler_init_top_k(k: i32) -> *mut c_void;
        pub fn llama_sampler_init_top_p(p: f32, min_keep: usize) -> *mut c_void;
        pub fn llama_sampler_init_temp(t: f32) -> *mut c_void;
        pub fn llama_sampler_init_dist(seed: u32) -> *mut c_void;
        pub fn llama_sampler_sample(sampler: *mut c_void, ctx: *mut c_void, idx: i32) -> i32;
        pub fn llama_sampler_accept(sampler: *mut c_void, token: i32);
        pub fn llama_sampler_free(sampler: *mut c_void);
        pub fn llama_vocab_is_eog(vocab: *const c_void, token: i32) -> bool;
        pub fn llama_detokenize(
            vocab: *const c_void,
            tokens: *const i32,
            n_tokens: i32,
            text: *mut c_char,
            text_len_max: i32,
            remove_special: bool,
            unparse_special: bool,
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
    /// Why successful generation terminated.
    pub termination: NativeGenerationTermination,
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
                    if role == "__talvik_tools" {
                        (
                            "system".to_owned(),
                            format!(
                                "You may call one of these tools. When the user asks for a tool, do not answer in prose and do not invent the result. Emit exactly one call as <tool_call>{{\"name\":\"tool_name\",\"arguments\":{{}}}}</tool_call>. The arguments must be valid JSON. Tools: {content}"
                            ),
                        )
                    } else {
                        (role.to_owned(), content.to_owned())
                    }
                })
                .collect::<Vec<_>>();
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
    unsafe fn detokenize(vocabulary: *const c_void, tokens: &[i32]) -> Result<Vec<u8>, String> {
        let token_count = i32::try_from(tokens.len()).map_err(|_| "too many tokens".to_owned())?;
        let mut capacity = tokens
            .len()
            .checked_mul(16)
            .and_then(|value| value.checked_add(64))
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| "detokenized text is too large".to_owned())?;
        loop {
            let mut text = vec![
                0_i8;
                usize::try_from(capacity)
                    .map_err(|_| "token text is too large".to_owned())?
            ];
            let length = ffi::llama_detokenize(
                vocabulary,
                tokens.as_ptr(),
                token_count,
                text.as_mut_ptr(),
                capacity,
                true,
                false,
            );
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
                (1..marker.len())
                    .rev()
                    .find(|length| piece.ends_with(&marker[..*length]))
                    .map(|length| (&piece[..piece.len() - length], true))
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
                callback,
                cancelled,
            );
            Err("native llama.cpp is unavailable".to_owned())
        }
        #[cfg(native_llama_cpp)]
        {
            let formatted_prompt = self.format_prompt(prompt)?;
            let mut prompt_tokens = self.tokenize(&formatted_prompt)?;
            let prompt_count = prompt_tokens.len() as u64;
            let max_context =
                usize::try_from(context_size).map_err(|_| "context too large".to_owned())?;
            if prompt_tokens.len() >= max_context {
                return Err("prompt exceeds context size".to_owned());
            }
            let mut context_params = ContextParams {
                n_ctx: context_size,
                // The prompt is submitted as one logical batch. Keep the
                // micro-batch bounded for graph evaluation, but ensure the
                // native batch capacity can hold every valid context-sized
                // prompt. llama.cpp aborts when n_tokens exceeds n_batch.
                n_batch: context_size,
                n_ubatch: context_size.min(512),
                ..unsafe { ffi::llama_context_default_params() }
            };
            context_params.abort_callback = Some(abort_callback);
            context_params.abort_callback_data = std::ptr::from_ref(cancelled).cast_mut().cast();
            let context = unsafe { ffi::llama_init_from_model(self.pointer, context_params) };
            if context.is_null() {
                return Err("llama.cpp could not create a context".to_owned());
            }
            let sampler_params = unsafe { ffi::llama_sampler_chain_default_params() };
            let sampler = unsafe { ffi::llama_sampler_chain_init(sampler_params) };
            if sampler.is_null() {
                unsafe {
                    ffi::llama_free(context);
                }
                return Err("llama.cpp could not create a sampler".to_owned());
            }
            let samplers = unsafe {
                [
                    ffi::llama_sampler_init_top_k(top_k),
                    ffi::llama_sampler_init_top_p(top_p, 1),
                    ffi::llama_sampler_init_temp(temperature),
                    ffi::llama_sampler_init_dist(seed),
                ]
            };
            if samplers.iter().any(|sampler| sampler.is_null()) {
                unsafe {
                    for sampler in samplers {
                        if !sampler.is_null() {
                            ffi::llama_sampler_free(sampler);
                        }
                    }
                    ffi::llama_sampler_free(sampler);
                    ffi::llama_free(context);
                }
                return Err("llama.cpp could not create a sampler".to_owned());
            }
            unsafe {
                for sampler_part in samplers {
                    ffi::llama_sampler_chain_add(sampler, sampler_part);
                }
            }
            let decode_result = unsafe {
                ffi::llama_decode(
                    context,
                    ffi::llama_batch_get_one(
                        prompt_tokens.as_mut_ptr(),
                        i32::try_from(prompt_tokens.len())
                            .map_err(|_| "prompt too long".to_owned())?,
                    ),
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
                        ffi::llama_free(context);
                    }
                    return Err("generation cancelled".to_owned());
                }
                DecodeOutcome::NativeFailure => {
                    unsafe {
                        ffi::llama_sampler_free(sampler);
                        ffi::llama_free(context);
                    }
                    return Err(decode_error("prompt", decode_result));
                }
            }
            let vocab = unsafe { ffi::llama_model_get_vocab(self.pointer.cast_const()) };
            let mut generated_tokens = Vec::new();
            let mut emitted_bytes = Vec::new();
            let mut pending_utf8 = Vec::new();
            let mut generated = 0_u64;
            let mut cancelled_result = false;
            let mut termination = NativeGenerationTermination::Stop;
            for _ in 0..max_tokens {
                if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
                    cancelled_result = true;
                    break;
                }
                let token = unsafe { ffi::llama_sampler_sample(sampler, context, -1) };
                if unsafe { ffi::llama_vocab_is_eog(vocab, token) } {
                    break;
                }
                generated_tokens.push(token);
                let text = unsafe { Self::detokenize(vocab, &generated_tokens) }?;
                let (text, terminal) = Self::terminal_piece_prefix(&text);
                let delta = if text.starts_with(&emitted_bytes) {
                    &text[emitted_bytes.len()..]
                } else {
                    text
                };
                pending_utf8.extend_from_slice(delta);
                let delta = take_complete_utf8(&mut pending_utf8);
                if !delta.is_empty() && !callback(&delta) {
                    cancelled_result = true;
                    break;
                }
                emitted_bytes = text.to_vec();
                generated += 1;
                if terminal {
                    break;
                }
                unsafe {
                    ffi::llama_sampler_accept(sampler, token);
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
                    ffi::llama_decode(context, ffi::llama_batch_get_one(next.as_mut_ptr(), 1))
                };
                match classify_decode_result(
                    result,
                    cancelled.load(std::sync::atomic::Ordering::SeqCst),
                ) {
                    DecodeOutcome::Success => {}
                    DecodeOutcome::Cancelled => {
                        unsafe {
                            ffi::llama_sampler_free(sampler);
                            ffi::llama_free(context);
                        }
                        return Err("generation cancelled".to_owned());
                    }
                    DecodeOutcome::NativeFailure => {
                        unsafe {
                            ffi::llama_sampler_free(sampler);
                            ffi::llama_free(context);
                        }
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
                ffi::llama_free(context);
            }
            if cancelled_result {
                return Err("generation cancelled".to_owned());
            }
            Ok(NativeGenerationStatistics {
                prompt_tokens: prompt_count,
                generated_tokens: generated,
                context_tokens: prompt_count + generated,
                termination,
            })
        }
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{classify_decode_result, decode_error, DecodeOutcome};

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
    }
}

impl Drop for NativeModel {
    fn drop(&mut self) {
        #[cfg(native_llama_cpp)]
        unsafe {
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
