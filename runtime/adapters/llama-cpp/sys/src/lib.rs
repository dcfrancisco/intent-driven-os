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
    use super::{c_char, c_void, Batch, ContextParams, ModelParams, SamplerChainParams};

    #[allow(improper_ctypes)]
    extern "C" {
        pub fn llama_backend_init();
        pub fn llama_backend_free();
        pub fn llama_print_system_info() -> *const c_char;
        pub fn llama_model_default_params() -> ModelParams;
        pub fn llama_model_load_from_file(path: *const c_char, params: ModelParams) -> *mut c_void;
        pub fn llama_model_free(model: *mut c_void);
        pub fn llama_model_get_vocab(model: *const c_void) -> *const c_void;
        pub fn llama_model_size(model: *const c_void) -> u64;
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
        return Ok(());
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
        return (!pointer.is_null())
            .then(|| CStr::from_ptr(pointer).to_string_lossy().into_owned());
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
            return Ok(tokens);
        }
        #[cfg(not(native_llama_cpp))]
        {
            let _ = text;
            Err("native llama.cpp is unavailable".to_owned())
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
            let mut text =
                vec![0_i8; usize::try_from(capacity).map_err(|_| "text too large".to_owned())?];
            let length = ffi::llama_detokenize(
                vocabulary,
                tokens.as_ptr(),
                token_count,
                text.as_mut_ptr(),
                capacity,
                false,
                false,
            );
            if length < 0 {
                capacity = length
                    .checked_neg()
                    .ok_or_else(|| "detokenized text length overflow".to_owned())?;
                continue;
            }
            let length =
                usize::try_from(length).map_err(|_| "invalid detokenized length".to_owned())?;
            let bytes = std::slice::from_raw_parts(text.as_ptr().cast::<u8>(), length);
            // GGUF byte-fallback tokens can form an incomplete UTF-8 sequence
            // at an individual streaming boundary. Preserve the decoded byte
            // stream and replace only invalid sequences instead of failing an
            // otherwise valid generation.
            return Ok(bytes.to_vec());
        }
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
            let mut prompt_tokens = self.tokenize(prompt)?;
            let prompt_count = prompt_tokens.len() as u64;
            let max_context =
                usize::try_from(context_size).map_err(|_| "context too large".to_owned())?;
            if prompt_tokens.len() >= max_context {
                return Err("prompt exceeds context size".to_owned());
            }
            let mut context_params = ContextParams {
                n_ctx: context_size,
                n_batch: context_size.min(512),
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
                let delta = if text.starts_with(&emitted_bytes) {
                    &text[emitted_bytes.len()..]
                } else {
                    text.as_slice()
                };
                pending_utf8.extend_from_slice(delta);
                let delta = take_complete_utf8(&mut pending_utf8);
                if !delta.is_empty() && !callback(&delta) {
                    cancelled_result = true;
                    break;
                }
                emitted_bytes = text;
                generated += 1;
                unsafe {
                    ffi::llama_sampler_accept(sampler, token);
                }
                if generated >= u64::from(max_tokens) {
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
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_decode_result, decode_error, DecodeOutcome};

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
        return Ok(NativeModel {
            pointer,
            memory_bytes,
        });
    }
    #[cfg(not(native_llama_cpp))]
    {
        let _ = path;
        Err("native llama.cpp is unavailable".to_owned())
    }
}
