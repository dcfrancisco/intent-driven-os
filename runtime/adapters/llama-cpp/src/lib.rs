//! llama.cpp implementation of OID's backend-neutral model runner contract.

#![warn(missing_docs)]

use oid_llama_cpp_sys as native;
use oid_model_runner::{
    CancellationToken, ContentPart, GenerationEvent, GenerationRequest, GenerationStats,
    GenerationStream, GenerationTermination, LoadModelRequest, LoadedModel, ModelId, ModelRunner,
    ModelRunnerError, ModelRuntimeStats, ModelState,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

mod structured_output;

const DEFAULT_MEDIA_MARKER: &str = "<__media__>";

fn generation_input<'a>(request: &'a GenerationRequest) -> (String, Vec<(String, &'a [u8])>) {
    if request.content.is_empty() {
        return (request.prompt.clone(), Vec::new());
    }

    let mut prompt = String::new();
    let mut images = Vec::new();
    for part in &request.content {
        match part {
            ContentPart::Text(text) => prompt.push_str(text),
            ContentPart::Image(image) => {
                prompt.push_str(DEFAULT_MEDIA_MARKER);
                images.push((image.media_type.clone(), image.data.as_slice()));
            }
        }
    }
    (prompt, images)
}

#[derive(Debug)]
struct Loaded {
    model: native::NativeModel,
}

/// Direct local llama.cpp model runner.
///
/// The native model handle is confined to this adapter. Callers only see OID
/// domain types and streamed generation events.
#[derive(Debug)]
pub struct LlamaCppModelRunner {
    initialized: AtomicBool,
    models: Arc<Mutex<BTreeMap<ModelId, Loaded>>>,
    state: Arc<Mutex<ModelState>>,
    generating: Arc<AtomicBool>,
}

impl Default for LlamaCppModelRunner {
    fn default() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            models: Arc::new(Mutex::new(BTreeMap::new())),
            state: Arc::new(Mutex::new(ModelState::Unloaded)),
            generating: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl LlamaCppModelRunner {
    /// Construct an uninitialized runner.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this build has a native llama.cpp library configured.
    #[must_use]
    pub const fn native_available() -> bool {
        native::is_available()
    }

    fn unavailable() -> ModelRunnerError {
        ModelRunnerError::BackendUnavailable(
            "configure LLAMA_CPP_LIB_DIR and rebuild to enable llama.cpp".to_owned(),
        )
    }
}

impl ModelRunner for LlamaCppModelRunner {
    fn load(&self, request: LoadModelRequest) -> Result<LoadedModel, ModelRunnerError> {
        if !native::is_available() {
            return Err(Self::unavailable());
        }
        if !std::path::Path::new(request.path.as_str()).is_file() {
            return Err(ModelRunnerError::ModelNotFound(
                request.path.as_str().to_owned(),
            ));
        }
        let projector_path = request
            .vision_projector_path
            .as_ref()
            .map(|path| path.as_str());
        if let Some(path) = projector_path {
            if !std::path::Path::new(path).is_file() {
                return Err(ModelRunnerError::ModelNotFound(path.to_owned()));
            }
        }

        let mut models = self
            .models
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("model lock poisoned".to_owned()))?;
        if models.contains_key(&request.model_id) {
            return Err(ModelRunnerError::ModelBusy(format!(
                "model {} is already loaded",
                request.model_id
            )));
        }
        *self
            .state
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("state lock poisoned".to_owned()))? =
            ModelState::Loading;
        if !self.initialized.swap(true, Ordering::SeqCst) {
            native::initialize().map_err(|error| {
                self.initialized.store(false, Ordering::SeqCst);
                if let Ok(mut state) = self.state.lock() {
                    *state = ModelState::Failed;
                }
                ModelRunnerError::BackendFailure(error)
            })?;
        }
        let started = Instant::now();
        let model = match native::load_model(request.path.as_str(), projector_path) {
            Ok(model) => model,
            Err(error) => {
                if models.is_empty() {
                    self.initialized.store(false, Ordering::SeqCst);
                    native::shutdown();
                }
                if let Ok(mut state) = self.state.lock() {
                    *state = if models.is_empty() {
                        ModelState::Failed
                    } else {
                        ModelState::Ready
                    };
                }
                return Err(ModelRunnerError::BackendFailure(error));
            }
        };
        let memory_bytes = model.memory_bytes();
        let loaded_model = Loaded { model };
        models.insert(request.model_id.clone(), loaded_model);
        if let Ok(mut state) = self.state.lock() {
            *state = ModelState::Ready;
        }
        Ok(LoadedModel {
            model_id: request.model_id,
            backend: "llama.cpp".to_owned(),
            format: "GGUF".to_owned(),
            load_time: Some(started.elapsed()),
            memory_bytes: Some(memory_bytes),
        })
    }

    #[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
    fn generate(&self, request: GenerationRequest) -> Result<GenerationStream, ModelRunnerError> {
        request.validate()?;
        if self.generating.swap(true, Ordering::SeqCst) {
            return Err(ModelRunnerError::ModelBusy(
                "a generation is already active".to_owned(),
            ));
        }
        let models = self.models.clone();
        let model_id = request.model_id.clone();
        if let Err(error) = models
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("model lock poisoned".to_owned()))?
            .get(&model_id)
            .ok_or(ModelRunnerError::ModelNotLoaded)
        {
            self.generating.store(false, Ordering::SeqCst);
            return Err(error);
        }
        {
            let resident_models = match models.lock() {
                Ok(models) => models,
                Err(_) => {
                    self.generating.store(false, Ordering::SeqCst);
                    return Err(ModelRunnerError::BackendFailure(
                        "model lock poisoned".to_owned(),
                    ));
                }
            };
            for (resident_id, loaded) in resident_models.iter() {
                if resident_id != &model_id {
                    if let Err(error) = loaded.model.reset_context() {
                        self.generating.store(false, Ordering::SeqCst);
                        return Err(ModelRunnerError::BackendFailure(error));
                    }
                }
            }
        }
        if let Ok(mut state) = self.state.lock() {
            *state = ModelState::Generating;
        }
        let cancellation = CancellationToken::default();
        let worker_cancellation = cancellation.clone();
        let lifecycle_state = self.state.clone();
        let generating = self.generating.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let started = Instant::now();
            let result = (|| -> Result<GenerationStats, String> {
                let guard = models
                    .lock()
                    .map_err(|_| "model lock poisoned".to_owned())?;
                let loaded = guard
                    .get(&model_id)
                    .ok_or_else(|| "model unloaded".to_owned())?;
                let first_token = Arc::new(Mutex::new(None));
                let first_token_for_callback = first_token.clone();
                let mut callback = |token: &str| {
                    if worker_cancellation.is_cancelled() {
                        return false;
                    }
                    if let Ok(mut first) = first_token_for_callback.lock() {
                        if first.is_none() {
                            *first = Some(Instant::now());
                        }
                    }
                    sender
                        .send(GenerationEvent::Token(token.to_owned()))
                        .is_ok()
                };
                let grammar = structured_output::compile(request.structured_output.as_ref())?;
                let (prompt, images) = generation_input(&request);
                let native_stats = loaded.model.generate_stream_with_grammar_and_images(
                    &prompt,
                    request.context_size,
                    request.max_tokens,
                    request.temperature,
                    request.top_p,
                    request.top_k,
                    request.seed.unwrap_or(0),
                    &request.stop_sequences,
                    grammar.as_deref(),
                    &images,
                    &mut callback,
                    worker_cancellation.as_atomic(),
                )?;
                let stats = GenerationStats {
                    prompt_tokens: Some(native_stats.prompt_tokens),
                    generated_tokens: native_stats.generated_tokens,
                    time_to_first_token: first_token
                        .lock()
                        .ok()
                        .and_then(|value| value.map(|time| time.duration_since(started))),
                    prompt_eval_time: Some(native_stats.prompt_eval_time),
                    generation_time: Some(native_stats.generation_time),
                    tokens_per_second: (native_stats.generation_time.as_secs_f64() > 0.0)
                        .then_some(
                            native_stats.generated_tokens as f64
                                / native_stats.generation_time.as_secs_f64(),
                        ),
                    termination: match native_stats.termination {
                        native::NativeGenerationTermination::Stop => GenerationTermination::Stop,
                        native::NativeGenerationTermination::Length => {
                            GenerationTermination::Length
                        }
                        native::NativeGenerationTermination::ContextLength => {
                            GenerationTermination::ContextLength
                        }
                    },
                    cache_hit: native_stats.cache_hit,
                    reused_prompt_tokens: native_stats.reused_prompt_tokens,
                };
                Ok(stats)
            })();
            // A terminal event is an admission boundary: clients may issue the
            // next request as soon as they observe it. Release the adapter's
            // generation slot before making that event observable.
            generating.store(false, Ordering::SeqCst);
            if let Ok(mut lifecycle) = lifecycle_state.lock() {
                *lifecycle = ModelState::Ready;
            }
            match result {
                Ok(stats) if worker_cancellation.is_cancelled() => {
                    let _ = sender.send(GenerationEvent::Cancelled(stats));
                }
                Ok(stats) => {
                    let _ = sender.send(GenerationEvent::Completed(stats));
                }
                Err(error)
                    if worker_cancellation.is_cancelled() || error == "generation cancelled" =>
                {
                    let _ = sender.send(GenerationEvent::Cancelled(GenerationStats {
                        generation_time: Some(started.elapsed()),
                        ..GenerationStats::default()
                    }));
                }
                Err(error) => {
                    let _ = sender.send(GenerationEvent::Error(ModelRunnerError::BackendFailure(
                        error,
                    )));
                }
            }
        });
        Ok(GenerationStream::new(receiver, cancellation))
    }

    fn stats(&self) -> Result<ModelRuntimeStats, ModelRunnerError> {
        let models = self
            .models
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("model lock poisoned".to_owned()))?;
        let state = *self
            .state
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("state lock poisoned".to_owned()))?;
        let loaded_models: Vec<_> = models.keys().cloned().collect();
        let memory_bytes = models
            .values()
            .map(|loaded| loaded.model.memory_bytes())
            .sum::<u64>();
        Ok(ModelRuntimeStats {
            loaded_model: loaded_models.first().cloned(),
            loaded_models,
            memory_bytes: (!models.is_empty()).then_some(memory_bytes),
            state,
        })
    }

    fn embeddings(
        &self,
        model_id: &ModelId,
        text: &str,
        context_size: u32,
    ) -> Result<Vec<f32>, ModelRunnerError> {
        let models = self
            .models
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("model lock poisoned".to_owned()))?;
        let loaded = models
            .get(model_id)
            .ok_or(ModelRunnerError::ModelNotLoaded)?;
        loaded
            .model
            .embedding(text, context_size)
            .map_err(ModelRunnerError::BackendFailure)
    }

    fn tokenize(&self, text: &str) -> Result<u64, ModelRunnerError> {
        let models = self
            .models
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("model lock poisoned".to_owned()))?;
        let loaded = models
            .values()
            .next()
            .ok_or(ModelRunnerError::ModelNotLoaded)?;
        loaded
            .model
            .tokenize(text)
            .map(|tokens| tokens.len() as u64)
            .map_err(ModelRunnerError::BackendFailure)
    }

    fn tool_support(&self, model_id: &ModelId) -> Result<bool, ModelRunnerError> {
        let models = self
            .models
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("model lock poisoned".to_owned()))?;
        Ok(models
            .get(model_id)
            .is_some_and(|loaded| loaded.model.supports_tools()))
    }

    fn reset_context(&self, model_id: &ModelId) -> Result<(), ModelRunnerError> {
        if self.generating.load(Ordering::SeqCst) {
            return Err(ModelRunnerError::ModelBusy(
                "cannot reset context while generation is active".to_owned(),
            ));
        }
        let models = self
            .models
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("model lock poisoned".to_owned()))?;
        let loaded = models
            .get(model_id)
            .ok_or(ModelRunnerError::ModelNotLoaded)?;
        loaded
            .model
            .reset_context()
            .map_err(ModelRunnerError::BackendFailure)
    }

    fn unload(&self) -> Result<(), ModelRunnerError> {
        if self.generating.load(Ordering::SeqCst) {
            return Err(ModelRunnerError::ModelBusy(
                "cannot unload while generation is active".to_owned(),
            ));
        }
        let mut models = self
            .models
            .lock()
            .map_err(|_| ModelRunnerError::UnloadFailure("model lock poisoned".to_owned()))?;
        if models.is_empty() {
            return Err(ModelRunnerError::ModelNotLoaded);
        }
        if let Ok(mut state) = self.state.lock() {
            *state = ModelState::Unloading;
        }
        models.clear();
        drop(models);
        self.initialized.store(false, Ordering::SeqCst);
        native::shutdown();
        if let Ok(mut state) = self.state.lock() {
            *state = ModelState::Unloaded;
        }
        Ok(())
    }

    fn unload_model(&self, model_id: &ModelId) -> Result<(), ModelRunnerError> {
        if self.generating.load(Ordering::SeqCst) {
            return Err(ModelRunnerError::ModelBusy(
                "cannot unload while generation is active".to_owned(),
            ));
        }
        let mut models = self
            .models
            .lock()
            .map_err(|_| ModelRunnerError::UnloadFailure("model lock poisoned".to_owned()))?;
        if models.remove(model_id).is_none() {
            return Err(ModelRunnerError::ModelNotLoaded);
        }
        if let Ok(mut state) = self.state.lock() {
            *state = if models.is_empty() {
                ModelState::Unloading
            } else {
                ModelState::Ready
            };
        }
        let empty = models.is_empty();
        drop(models);
        if empty {
            self.initialized.store(false, Ordering::SeqCst);
            native::shutdown();
            if let Ok(mut state) = self.state.lock() {
                *state = ModelState::Unloaded;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::generation_input;
    use oid_model_runner::{ContentPart, GenerationRequest, ImageInput, ModelId};

    #[test]
    fn multimodal_input_preserves_text_image_order_and_uses_the_mtmd_marker() {
        let request = GenerationRequest {
            model_id: ModelId::new("vision").expect("model id"),
            prompt: "ignored fallback".to_owned(),
            content: vec![
                ContentPart::Text("before ".to_owned()),
                ContentPart::Image(ImageInput {
                    media_type: "image/png".to_owned(),
                    data: vec![1, 2, 3],
                }),
                ContentPart::Text(" after".to_owned()),
            ],
            max_tokens: 8,
            temperature: 0.0,
            top_p: 1.0,
            top_k: 1,
            context_size: 128,
            seed: None,
            stop_sequences: Vec::new(),
            structured_output: None,
        };
        let (prompt, images) = generation_input(&request);

        assert_eq!(prompt, "before <__media__> after");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].0, "image/png");
        assert_eq!(images[0].1, [1, 2, 3]);
    }
}
