//! llama.cpp implementation of OID's backend-neutral model runner contract.

#![warn(missing_docs)]

use oid_llama_cpp_sys as native;
use oid_model_runner::{
    CancellationToken, ContentPart, DeviceCapability, ExecutionBackend, GenerationEvent,
    GenerationRequest, GenerationStats, GenerationStream, GenerationTermination, LoadModelRequest,
    LoadedModel, ModelId, ModelPlacement, ModelRunner, ModelRunnerError, ModelRuntimeStats,
    ModelState, PlacementPolicy,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

mod structured_output;

const DEFAULT_MEDIA_MARKER: &str = "<__media__>";

fn generation_input(request: &GenerationRequest) -> (String, Vec<(String, &[u8])>) {
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
    placement: ModelPlacement,
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

    /// Enumerate devices that the linked llama.cpp/ggml build initialized.
    #[must_use]
    pub fn devices() -> Vec<DeviceCapability> {
        native::devices()
            .into_iter()
            .map(|device| DeviceCapability {
                backend: Self::execution_backend(device.backend),
                name: device.name,
                compiled: true,
                available: true,
                free_memory_bytes: device.free_memory_bytes,
                total_memory_bytes: device.total_memory_bytes,
                shared_memory: device.shared_memory,
            })
            .collect()
    }

    fn unavailable() -> ModelRunnerError {
        ModelRunnerError::BackendUnavailable(
            "configure LLAMA_CPP_LIB_DIR and rebuild to enable llama.cpp".to_owned(),
        )
    }

    fn native_backend(backend: ExecutionBackend) -> Option<native::NativeBackend> {
        match backend {
            ExecutionBackend::Cuda => Some(native::NativeBackend::Cuda),
            ExecutionBackend::Vulkan => Some(native::NativeBackend::Vulkan),
            ExecutionBackend::Metal => Some(native::NativeBackend::Metal),
            ExecutionBackend::Cpu => Some(native::NativeBackend::Cpu),
            ExecutionBackend::Auto => None,
        }
    }

    fn execution_backend(backend: native::NativeBackend) -> ExecutionBackend {
        match backend {
            native::NativeBackend::Cuda => ExecutionBackend::Cuda,
            native::NativeBackend::Vulkan => ExecutionBackend::Vulkan,
            native::NativeBackend::Metal => ExecutionBackend::Metal,
            native::NativeBackend::Cpu => ExecutionBackend::Cpu,
        }
    }

    fn qualified_backends() -> Vec<ExecutionBackend> {
        std::env::var("TALVIK_QUALIFIED_GPU_BACKENDS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|value| match value.trim().to_ascii_lowercase().as_str() {
                "cuda" => Some(ExecutionBackend::Cuda),
                "vulkan" => Some(ExecutionBackend::Vulkan),
                "metal" => Some(ExecutionBackend::Metal),
                _ => None,
            })
            .collect()
    }

    fn select_backend(request: &LoadModelRequest) -> Result<ExecutionBackend, ModelRunnerError> {
        if request.placement.policy == PlacementPolicy::GpuRequired
            && request.placement.gpu_layers == Some(0)
        {
            return Err(ModelRunnerError::InvalidRequest(
                "GPU-required placement cannot request zero GPU layers".to_owned(),
            ));
        }
        if request.placement.policy == PlacementPolicy::CpuOnly
            || request.placement.backend == ExecutionBackend::Cpu
        {
            return Ok(ExecutionBackend::Cpu);
        }
        let available: Vec<_> = native::devices()
            .into_iter()
            .filter(|device| device.backend != native::NativeBackend::Cpu)
            .map(|device| Self::execution_backend(device.backend))
            .collect();
        Self::choose_backend(
            request.placement.policy,
            request.placement.backend,
            &available,
            &Self::qualified_backends(),
        )
    }

    fn choose_backend(
        policy: PlacementPolicy,
        requested: ExecutionBackend,
        available: &[ExecutionBackend],
        qualified: &[ExecutionBackend],
    ) -> Result<ExecutionBackend, ModelRunnerError> {
        if policy == PlacementPolicy::CpuOnly || requested == ExecutionBackend::Cpu {
            return Ok(ExecutionBackend::Cpu);
        }
        if requested != ExecutionBackend::Auto {
            if available.contains(&requested) {
                return Ok(requested);
            }
            if policy == PlacementPolicy::GpuRequired {
                return Err(ModelRunnerError::BackendUnavailable(format!(
                    "requested {} backend is not compiled, initialized, and usable",
                    requested.as_str()
                )));
            }
            return Ok(ExecutionBackend::Cpu);
        }
        for candidate in [
            ExecutionBackend::Cuda,
            ExecutionBackend::Metal,
            ExecutionBackend::Vulkan,
        ] {
            if available.contains(&candidate)
                && (matches!(
                    policy,
                    PlacementPolicy::GpuPreferred | PlacementPolicy::GpuRequired
                ) || qualified.contains(&candidate))
            {
                return Ok(candidate);
            }
        }
        if policy == PlacementPolicy::GpuRequired {
            return Err(ModelRunnerError::BackendUnavailable(
                "GPU placement is required but no requested/qualified compiled backend is usable"
                    .to_owned(),
            ));
        }
        Ok(ExecutionBackend::Cpu)
    }
}

impl ModelRunner for LlamaCppModelRunner {
    #[allow(clippy::too_many_lines)]
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
            .map(oid_model_runner::ModelPath::as_str);
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
        let selected = match Self::select_backend(&request) {
            Ok(selected) => selected,
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
                return Err(error);
            }
        };
        let requested_layers = if selected == ExecutionBackend::Cpu {
            0
        } else {
            request
                .placement
                .gpu_layers
                .unwrap_or(u32::try_from(i32::MAX).unwrap_or(u32::MAX))
        };
        let selected_native = Self::native_backend(selected).unwrap_or(native::NativeBackend::Cpu);
        let first_load = native::load_model(
            request.path.as_str(),
            projector_path,
            native::NativeLoadOptions {
                backend: selected_native,
                gpu_layers: requested_layers,
            },
        );
        let mut fallback_reason = (selected == ExecutionBackend::Cpu
            && (request.placement.policy == PlacementPolicy::GpuPreferred
                || !matches!(
                    request.placement.backend,
                    ExecutionBackend::Auto | ExecutionBackend::Cpu
                )))
        .then(|| "no requested GPU backend was compiled, initialized, and usable".to_owned());
        let model_result = match first_load {
            Err(error)
                if selected != ExecutionBackend::Cpu
                    && request.placement.policy != PlacementPolicy::GpuRequired =>
            {
                fallback_reason = Some(error);
                native::load_model(
                    request.path.as_str(),
                    projector_path,
                    native::NativeLoadOptions {
                        backend: native::NativeBackend::Cpu,
                        gpu_layers: 0,
                    },
                )
            }
            result => result,
        };
        let model = match model_result {
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
        let native_placement = model.placement().clone();
        let selected_backend = Self::execution_backend(native_placement.backend);
        let placement = ModelPlacement {
            requested_policy: request.placement.policy,
            requested_backend: request.placement.backend,
            selected_backend,
            selected_device: native_placement.device,
            gpu_layers_requested: if selected_backend == ExecutionBackend::Cpu {
                0
            } else {
                requested_layers
            },
            gpu_layers_offloaded: native_placement.offloaded_gpu_layers,
            fallback_applied: fallback_reason.is_some(),
            fallback_reason,
        };
        let loaded_model = Loaded {
            model,
            placement: placement.clone(),
        };
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
            host_memory_bytes: (selected_backend == ExecutionBackend::Cpu).then_some(memory_bytes),
            gpu_memory_bytes: None,
            placement,
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
            let Ok(resident_models) = models.lock() else {
                self.generating.store(false, Ordering::SeqCst);
                return Err(ModelRunnerError::BackendFailure(
                    "model lock poisoned".to_owned(),
                ));
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
                let decode_time = native_stats
                    .generation_time
                    .saturating_sub(native_stats.prompt_eval_time);
                let stats = GenerationStats {
                    prompt_tokens: Some(native_stats.prompt_tokens),
                    generated_tokens: native_stats.generated_tokens,
                    time_to_first_token: first_token
                        .lock()
                        .ok()
                        .and_then(|value| value.map(|time| time.duration_since(started))),
                    prompt_eval_time: Some(native_stats.prompt_eval_time),
                    generation_time: Some(native_stats.generation_time),
                    tokens_per_second: (decode_time.as_secs_f64() > 0.0).then_some(
                        native_stats.generated_tokens as f64 / decode_time.as_secs_f64(),
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
                    let error = if error == "prompt exceeds context size" {
                        ModelRunnerError::InvalidRequest(error)
                    } else {
                        ModelRunnerError::BackendFailure(error)
                    };
                    let _ = sender.send(GenerationEvent::Error(error));
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

    fn placement(&self, model_id: &ModelId) -> Result<ModelPlacement, ModelRunnerError> {
        self.models
            .lock()
            .map_err(|_| ModelRunnerError::BackendFailure("model lock poisoned".to_owned()))?
            .get(model_id)
            .map(|loaded| loaded.placement.clone())
            .ok_or(ModelRunnerError::ModelNotLoaded)
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
    use super::{generation_input, LlamaCppModelRunner};
    use oid_model_runner::{
        ContentPart, ExecutionBackend, GenerationRequest, ImageInput, ModelId, PlacementPolicy,
    };

    #[test]
    fn automatic_selection_uses_only_qualified_available_gpu_routes() {
        assert_eq!(
            LlamaCppModelRunner::choose_backend(
                PlacementPolicy::Automatic,
                ExecutionBackend::Auto,
                &[ExecutionBackend::Vulkan],
                &[],
            )
            .expect("CPU fallback"),
            ExecutionBackend::Cpu
        );
        assert_eq!(
            LlamaCppModelRunner::choose_backend(
                PlacementPolicy::Automatic,
                ExecutionBackend::Auto,
                &[ExecutionBackend::Vulkan],
                &[ExecutionBackend::Vulkan],
            )
            .expect("qualified Vulkan"),
            ExecutionBackend::Vulkan
        );
    }

    #[test]
    fn required_gpu_never_silently_falls_back_to_cpu() {
        assert!(LlamaCppModelRunner::choose_backend(
            PlacementPolicy::GpuRequired,
            ExecutionBackend::Cuda,
            &[ExecutionBackend::Vulkan],
            &[],
        )
        .is_err());
    }

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
