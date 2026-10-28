//! Talvik's production runtime service and composition root.

use crate::{
    backends::{
        Backend, BackendHealth, BackendManager, BackendSummary, EmbeddingsRequest,
        EmbeddingsResult, LoadedModel,
    },
    hardware::{HardwareService, HardwareSnapshot},
    models::{ModelAcquirer, ModelDiscovery, ModelMetadata, ModelRegistry, ModelStatus},
    GenerationMessage, GenerationRequest, GenerationResult, GenerationStatistics, GenerationStream,
    LlamaCppAdapter, ResidentModelSnapshot, RuntimeApi, RuntimeConfig, RuntimeError,
    RuntimeService, RuntimeSnapshot, RuntimeStatus,
};
use oid_shared::{EventBus, LifecycleState, RuntimeEvent};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

static GENERATION_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

type ActiveGeneration = (String, String, Arc<AtomicBool>);

#[derive(Debug)]
struct PendingGeneration {
    request: GenerationRequest,
    sender: mpsc::Sender<GenerationMessage>,
    cancellation: Arc<AtomicBool>,
    enqueued_at: Instant,
}

/// Runtime-owned memory and residency policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourcePolicy {
    /// Maximum number of resident models. Zero means no count limit.
    pub max_resident_models: usize,
    /// Optional hard memory budget applied in addition to physical availability.
    pub memory_budget_bytes: Option<u64>,
    /// Safety reservation retained after estimated allocations.
    pub safety_margin_bytes: u64,
    /// Whether idle models may be automatically unloaded for a request.
    pub automatic_eviction: bool,
    /// Maximum number of idle-model evictions for one request.
    pub recovery_budget: usize,
    /// Maximum queued/admission wait interval.
    pub admission_timeout: Duration,
    /// Minimum idle interval before automatic eviction.
    pub idle_retention: Duration,
    /// Maximum FIFO generation requests waiting behind the active request.
    pub queue_capacity: usize,
    /// Model IDs protected from automatic eviction.
    pub pinned_models: BTreeSet<String>,
}

impl Default for ResourcePolicy {
    fn default() -> Self {
        Self {
            max_resident_models: 0,
            memory_budget_bytes: None,
            safety_margin_bytes: SAFETY_MARGIN_BYTES,
            automatic_eviction: true,
            recovery_budget: 8,
            admission_timeout: Duration::from_secs(30),
            idle_retention: Duration::ZERO,
            queue_capacity: 8,
            pinned_models: BTreeSet::new(),
        }
    }
}

impl ResourcePolicy {
    fn from_environment() -> Self {
        let mut policy = Self::default();
        if let Some(value) = std::env::var_os("TALVIK_MAX_RESIDENT_MODELS")
            .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
        {
            policy.max_resident_models = value;
        }
        if let Some(value) = std::env::var_os("TALVIK_MEMORY_BUDGET_BYTES")
            .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
        {
            policy.memory_budget_bytes = Some(value);
        }
        if let Some(value) = std::env::var_os("TALVIK_SAFETY_MARGIN_BYTES")
            .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
        {
            policy.safety_margin_bytes = value;
        }
        if let Some(value) = std::env::var_os("TALVIK_AUTOMATIC_EVICTION") {
            policy.automatic_eviction = !matches!(
                value.to_string_lossy().to_ascii_lowercase().as_str(),
                "0" | "false" | "no"
            );
        }
        if let Some(value) = std::env::var_os("TALVIK_RECOVERY_BUDGET")
            .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
        {
            policy.recovery_budget = value;
        }
        if let Some(value) = std::env::var_os("TALVIK_ADMISSION_TIMEOUT_SECONDS")
            .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
        {
            policy.admission_timeout = Duration::from_secs(value);
        }
        if let Some(value) = std::env::var_os("TALVIK_QUEUE_CAPACITY")
            .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
        {
            policy.queue_capacity = value;
        }
        if let Some(value) = std::env::var_os("TALVIK_IDLE_RETENTION_SECONDS")
            .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
        {
            policy.idle_retention = Duration::from_secs(value);
        }
        if let Some(value) = std::env::var_os("TALVIK_PINNED_MODELS") {
            policy.pinned_models = value
                .to_string_lossy()
                .split(',')
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .collect();
        }
        policy
    }
}

/// Production runtime service used by the console and future clients.
#[derive(Clone, Debug)]
pub struct MarinaRuntime {
    config: RuntimeConfig,
    bus: EventBus,
    backends: BackendManager,
    models: ModelRegistry,
    hardware: HardwareService,
    started_at: Instant,
    loaded: Arc<Mutex<BTreeMap<String, LoadedModel>>>,
    residency_lock: Arc<Mutex<()>>,
    last_used: Arc<Mutex<BTreeMap<String, Instant>>>,
    active_generation: Arc<Mutex<Option<ActiveGeneration>>>,
    pending_generations: Arc<Mutex<VecDeque<PendingGeneration>>>,
    resource_policy: ResourcePolicy,
}

impl MarinaRuntime {
    /// Start Talvik's production runtime and publish its startup event.
    #[must_use]
    pub fn start(config: RuntimeConfig, bus: EventBus) -> Self {
        Self::start_with_backend(config, bus, Box::new(LlamaCppAdapter::new()))
    }

    fn start_with_backend(config: RuntimeConfig, bus: EventBus, backend: Box<dyn Backend>) -> Self {
        let hardware = HardwareService::detect(&bus);
        Self::start_with_backend_and_hardware(config, bus, backend, hardware)
    }

    fn start_with_backend_and_hardware(
        config: RuntimeConfig,
        bus: EventBus,
        backend: Box<dyn Backend>,
        hardware: HardwareService,
    ) -> Self {
        let backends = BackendManager::new(bus.clone());
        let backend_id = backend.descriptor().id;
        let _ = backends.register(backend);
        let _ = backends.enable(&backend_id);
        let models = config
            .registry_path
            .as_ref()
            .and_then(|path| ModelRegistry::open(path, bus.clone()).ok())
            .unwrap_or_else(|| ModelRegistry::in_memory(bus.clone()));
        let discovery_directories = config
            .model_directory
            .clone()
            .map_or_else(ModelDiscovery::default_directories, |directory| {
                vec![directory]
            });
        let discovery = ModelDiscovery::new(discovery_directories);
        let _ = discovery.discover(&models, &backend_id);
        for model in models.list() {
            if !matches!(
                model.status,
                ModelStatus::Registered | ModelStatus::Unavailable
            ) {
                let _ = models.set_status(&model.id, &ModelStatus::Registered);
            }
        }
        bus.publish(&RuntimeEvent::RuntimeStarted);
        bus.publish(&RuntimeEvent::HealthUpdated("Healthy".to_owned()));
        Self {
            config,
            bus,
            backends,
            models,
            hardware,
            started_at: Instant::now(),
            loaded: Arc::new(Mutex::new(BTreeMap::new())),
            residency_lock: Arc::new(Mutex::new(())),
            last_used: Arc::new(Mutex::new(BTreeMap::new())),
            active_generation: Arc::new(Mutex::new(None)),
            pending_generations: Arc::new(Mutex::new(VecDeque::new())),
            resource_policy: ResourcePolicy::from_environment(),
        }
    }

    /// Return the configured runtime name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.config.name
    }

    fn touch_model(&self, id: &str) {
        if let Ok(mut last_used) = self.last_used.lock() {
            last_used.insert(id.to_owned(), Instant::now());
        }
    }

    fn eviction_candidate(&self, requested: &str) -> Option<String> {
        let active_id = self
            .active_generation
            .lock()
            .ok()
            .and_then(|active| active.as_ref().map(|(id, _, _)| id.clone()));
        let pinned = &self.resource_policy.pinned_models;
        let last_used = self.last_used.lock().ok()?;
        self.loaded
            .lock()
            .ok()?
            .keys()
            .filter(|id| id.as_str() != requested)
            .filter(|id| active_id.as_deref() != Some(id.as_str()))
            .filter(|id| !pinned.contains(*id))
            .filter(|id| {
                last_used
                    .get(*id)
                    .is_none_or(|used| used.elapsed() >= self.resource_policy.idle_retention)
            })
            .min_by_key(|id| last_used.get(*id).copied().unwrap_or(self.started_at))
            .cloned()
    }

    fn recover_for_generation(&self, request: &GenerationRequest) -> Result<(), RuntimeError> {
        let mut attempts = 0;
        loop {
            let loaded = self
                .loaded
                .lock()
                .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?;
            let resident = loaded.contains_key(&request.model_id);
            drop(loaded);
            if resident
                && admit_generation(&self.hardware, &self.loaded, request, &self.resource_policy)
                    .is_ok()
            {
                self.touch_model(&request.model_id);
                return Ok(());
            }
            if attempts >= self.resource_policy.recovery_budget
                || !self.resource_policy.automatic_eviction
            {
                if resident {
                    return admit_generation(
                        &self.hardware,
                        &self.loaded,
                        request,
                        &self.resource_policy,
                    );
                }
                return Err(RuntimeError::ResourceAdmission {
                    retryable: true,
                    message: format!(
                        "model {} is not resident and recovery could not load it",
                        request.model_id
                    ),
                });
            }
            if !resident {
                if self.models.inspect(&request.model_id).is_none() {
                    return Err(RuntimeError::ModelNotFound(request.model_id.clone()));
                }
                let over_count = self.resource_policy.max_resident_models > 0
                    && self.loaded.lock().is_ok_and(|loaded| {
                        loaded.len() >= self.resource_policy.max_resident_models
                    });
                if over_count {
                    let Some(candidate) = self.eviction_candidate(&request.model_id) else {
                        return Err(RuntimeError::ResourceAdmission {
                            retryable: true,
                            message:
                                "resident-model limit reached and no idle model is reclaimable"
                                    .to_owned(),
                        });
                    };
                    RuntimeService::model_unload(self, &candidate)?;
                    attempts += 1;
                    continue;
                }
                match RuntimeService::model_load(self, &request.model_id) {
                    Ok(()) => {
                        self.touch_model(&request.model_id);
                        continue;
                    }
                    Err(RuntimeError::ResourceAdmission { .. }) => {}
                    Err(error) => return Err(error),
                }
            }
            let Some(candidate) = self.eviction_candidate(&request.model_id) else {
                return Err(RuntimeError::ResourceAdmission {
                    retryable: true,
                    message:
                        "memory admission failed: no idle, unpinned resident model is reclaimable"
                            .to_owned(),
                });
            };
            self.bus.publish(&RuntimeEvent::WarningRaised(format!(
                "reclaiming idle model {candidate} for generation {} (attempt {})",
                request.model_id,
                attempts + 1
            )));
            self.bus
                .publish(&RuntimeEvent::ModelUnloading(candidate.clone()));
            RuntimeService::model_unload(self, &candidate)?;
            attempts += 1;
        }
    }

    fn start_next_queued(&self) {
        let pending = self
            .pending_generations
            .lock()
            .ok()
            .and_then(|mut queue| queue.pop_front());
        let Some(pending) = pending else { return };
        self.bus.publish(&RuntimeEvent::WarningRaised(format!(
            "starting queued generation {} after {} ms",
            pending.request.request_id,
            pending.enqueued_at.elapsed().as_millis()
        )));
        if pending.cancellation.load(Ordering::SeqCst) {
            let _ = pending
                .sender
                .send(GenerationMessage::Cancelled(GenerationResult {
                    request_id: pending.request.request_id,
                    generation_id: String::new(),
                    text: String::new(),
                    statistics: GenerationStatistics::default(),
                }));
            self.start_next_queued();
            return;
        }
        if pending.enqueued_at.elapsed() > self.resource_policy.admission_timeout {
            let _ =
                pending
                    .sender
                    .send(GenerationMessage::Failed(RuntimeError::ResourceAdmission {
                        message: "generation admission deadline exceeded while queued".to_owned(),
                        retryable: true,
                    }));
            self.start_next_queued();
            return;
        }
        let queued_sender = pending.sender;
        let queued_cancellation = pending.cancellation.clone();
        match self.generate(pending.request) {
            Ok(inner) => {
                thread::spawn(move || loop {
                    if queued_cancellation.load(Ordering::SeqCst) {
                        inner.cancel();
                    }
                    match inner.recv_timeout(Duration::from_millis(100)) {
                        Ok(message) => {
                            let terminal = matches!(
                                message,
                                GenerationMessage::Completed(_)
                                    | GenerationMessage::Cancelled(_)
                                    | GenerationMessage::Failed(_)
                            );
                            let _ = queued_sender.send(message);
                            if terminal {
                                break;
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                });
            }
            Err(error) => {
                let _ = queued_sender.send(GenerationMessage::Failed(error));
                self.start_next_queued();
            }
        }
    }
}

impl RuntimeApi for MarinaRuntime {
    fn status(&self) -> RuntimeStatus {
        RuntimeStatus {
            state: LifecycleState::Ready,
            version: "0.1",
        }
    }
}

fn admit_generation(
    hardware: &HardwareService,
    loaded: &Arc<Mutex<BTreeMap<String, LoadedModel>>>,
    request: &GenerationRequest,
    policy: &ResourcePolicy,
) -> Result<(), RuntimeError> {
    let resident = loaded
        .lock()
        .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?;
    let model_bytes = resident
        .get(&request.model_id)
        .map_or(0, |model| model.memory_bytes);
    let resident_bytes = resident.values().map(|model| model.memory_bytes).sum();
    admit_memory_with_resident(
        hardware.snapshot().available_ram_bytes,
        resident_bytes,
        model_bytes,
        request.options.context_size,
        policy,
    )
}

const SAFETY_MARGIN_BYTES: u64 = 512 * 1_048_576;
const KV_BYTES_PER_CONTEXT_TOKEN: u64 = 16 * 1024;
const DEFAULT_CONTEXT_SIZE: u32 = 4_096;

#[cfg(test)]
fn admit_memory(
    available: Option<u64>,
    model_bytes: u64,
    context_size: u32,
    policy: &ResourcePolicy,
) -> Result<(), RuntimeError> {
    admit_memory_with_resident(available, 0, model_bytes, context_size, policy)
}

fn admit_memory_with_resident(
    available: Option<u64>,
    resident_bytes: u64,
    incremental_model_bytes: u64,
    context_size: u32,
    policy: &ResourcePolicy,
) -> Result<(), RuntimeError> {
    let available = available.ok_or_else(|| {
        RuntimeError::ModelLifecycle(
            "memory admission unavailable: available system memory is unknown".to_owned(),
        )
    })?;
    let context_bytes = u64::from(context_size)
        .checked_mul(KV_BYTES_PER_CONTEXT_TOKEN)
        .ok_or_else(|| {
            RuntimeError::ModelLifecycle("context memory estimate overflow".to_owned())
        })?;
    let incremental_required = incremental_model_bytes
        .saturating_add(context_bytes)
        .saturating_add(policy.safety_margin_bytes);
    let budget_required = resident_bytes.saturating_add(incremental_required);
    if available < incremental_required {
        return Err(RuntimeError::ResourceAdmission {
            retryable: true,
            message: format!(
                "memory admission rejected: available={available} required={incremental_required} model={incremental_model_bytes} context={context_bytes} resident={resident_bytes}"
            ),
        });
    }
    if let Some(budget) = policy.memory_budget_bytes {
        if budget < budget_required {
            return Err(RuntimeError::ResourceAdmission {
                retryable: true,
                message: format!(
                    "memory budget rejected: budget={budget} required={budget_required} model={incremental_model_bytes} context={context_bytes} resident={resident_bytes}"
                ),
            });
        }
    }
    Ok(())
}

impl RuntimeService for MarinaRuntime {
    fn status(&self) -> RuntimeStatus {
        <Self as RuntimeApi>::status(self)
    }

    fn snapshot(&self) -> RuntimeSnapshot {
        let last_used = self
            .last_used
            .lock()
            .map(|last_used| last_used.clone())
            .unwrap_or_default();
        let resident_models = self
            .loaded
            .lock()
            .map(|loaded| {
                loaded
                    .values()
                    .map(|model| ResidentModelSnapshot {
                        id: model.model_id.clone(),
                        memory_bytes: Some(model.memory_bytes),
                        idle_seconds: last_used
                            .get(&model.model_id)
                            .copied()
                            .map_or(0, |used| used.elapsed().as_secs()),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let pending_requests = self
            .pending_generations
            .lock()
            .map_or(0, |pending| pending.len());
        let active_requests = self
            .active_generation
            .lock()
            .map_or(0, |active| usize::from(active.is_some()));
        RuntimeSnapshot {
            health: "Healthy",
            backend: self
                .backends
                .active_backend()
                .unwrap_or_else(|| "None".to_owned()),
            models: self.models.list().len(),
            memory: self.loaded.lock().ok().map_or_else(
                || "--".to_owned(),
                |loaded| {
                    let bytes: u64 = loaded.values().map(|model| model.memory_bytes).sum();
                    if loaded.is_empty() {
                        "--".to_owned()
                    } else {
                        format!("{} MB", bytes / 1_048_576)
                    }
                },
            ),
            loaded_model: self
                .loaded
                .lock()
                .ok()
                .and_then(|loaded| loaded.keys().next().cloned()),
            loaded_models: self
                .loaded
                .lock()
                .map(|loaded| loaded.keys().cloned().collect())
                .unwrap_or_default(),
            resident_models,
            pending_requests,
            active_requests,
            queue_capacity: self.resource_policy.queue_capacity,
            automatic_eviction: self.resource_policy.automatic_eviction,
            idle_retention_seconds: self.resource_policy.idle_retention.as_secs(),
            uptime_seconds: self.started_at.elapsed().as_secs(),
            backend_version: self
                .backends
                .discover()
                .into_iter()
                .find(|backend| backend.active)
                .and_then(|backend| backend.descriptor.library_version),
            model_memory_bytes: self.loaded.lock().ok().and_then(|loaded| {
                (!loaded.is_empty()).then(|| loaded.values().map(|model| model.memory_bytes).sum())
            }),
            generating: self
                .active_generation
                .lock()
                .is_ok_and(|active| active.is_some()),
            current_tokens_per_second: None,
            loaded_context: None,
        }
    }

    fn backend_list(&self) -> Vec<BackendSummary> {
        self.backends.discover()
    }

    fn backend_health(&self, id: &str) -> Option<BackendHealth> {
        self.backends.health(id)
    }

    fn model_list(&self) -> Vec<ModelMetadata> {
        self.models.list()
    }

    fn model_inspect(&self, id: &str) -> Option<ModelMetadata> {
        self.models.inspect(id)
    }

    fn model_pull(
        &self,
        source: &str,
        model_id: Option<&str>,
        checksum: Option<&str>,
    ) -> Result<ModelMetadata, RuntimeError> {
        let directory = self
            .config
            .model_directory
            .clone()
            .or_else(|| std::env::var_os("TALVIK_MODEL_DIR").map(std::path::PathBuf::from))
            .or_else(|| std::env::var_os("MARINA_MODEL_DIR").map(std::path::PathBuf::from))
            .unwrap_or_else(|| {
                crate::config::talvik_home_directory().map_or_else(
                    || std::path::PathBuf::from(".marina/models"),
                    |home| home.join("models"),
                )
            });
        let model = ModelAcquirer::acquire(source, &directory, model_id, checksum)?;
        self.models.register(model.clone())?;
        Ok(model)
    }

    fn model_load(&self, id: &str) -> Result<(), RuntimeError> {
        let _residency_guard = self
            .residency_lock
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("residency unavailable".to_owned()))?;
        if self
            .loaded
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?
            .contains_key(id)
        {
            return Ok(());
        }
        let model = self
            .models
            .inspect(id)
            .ok_or_else(|| RuntimeError::ModelNotFound(id.to_owned()))?;
        if self.resource_policy.max_resident_models > 0
            && self
                .loaded
                .lock()
                .is_ok_and(|loaded| loaded.len() >= self.resource_policy.max_resident_models)
        {
            return Err(RuntimeError::ResourceAdmission {
                message: format!(
                    "resident-model limit reached while loading {id}; unload an idle model or use inference recovery"
                ),
                retryable: true,
            });
        }
        let resident_bytes = self
            .loaded
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?
            .values()
            .map(|loaded| loaded.memory_bytes)
            .sum::<u64>();
        let estimated_model_bytes = model
            .memory_requirement_mb
            .checked_mul(1_048_576)
            .ok_or_else(|| {
                RuntimeError::ModelLifecycle("model memory estimate overflow".to_owned())
            })?;
        // Loading is admitted against the default context reservation so a
        // model cannot become visibly Loaded while the default generation
        // request is already known to be impossible. The backend-reported
        // size is checked again after native loading because a GGUF may need
        // additional repack/working memory beyond its registry estimate.
        admit_memory_with_resident(
            self.hardware.snapshot().available_ram_bytes,
            resident_bytes,
            estimated_model_bytes,
            DEFAULT_CONTEXT_SIZE,
            &self.resource_policy,
        )?;
        self.bus.publish(&RuntimeEvent::ModelLoading(id.to_owned()));
        self.models.set_status(id, &ModelStatus::Loading)?;
        match self.backends.load_model(&model) {
            Ok(loaded) => {
                if let Err(error) = admit_memory_with_resident(
                    self.hardware.snapshot().available_ram_bytes,
                    resident_bytes,
                    loaded.memory_bytes,
                    DEFAULT_CONTEXT_SIZE,
                    &self.resource_policy,
                ) {
                    let cleanup = self.backends.unload_model(id).err();
                    let _ = self.models.set_status(id, &ModelStatus::Registered);
                    if let Some(cleanup) = cleanup {
                        return Err(RuntimeError::ModelLifecycle(format!(
                            "memory admission rejected and model cleanup failed: {error}; {cleanup}"
                        )));
                    }
                    return Err(error);
                }
                self.models.set_status(id, &ModelStatus::Loaded)?;
                self.loaded
                    .lock()
                    .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?
                    .insert(id.to_owned(), loaded);
                self.touch_model(id);
                Ok(())
            }
            Err(error) => {
                let _ = self.models.set_status(id, &ModelStatus::Failed);
                self.bus
                    .publish(&RuntimeEvent::ErrorRaised(error.to_string()));
                Err(error)
            }
        }
    }

    fn model_unload(&self, id: &str) -> Result<(), RuntimeError> {
        let _residency_guard = self
            .residency_lock
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("residency unavailable".to_owned()))?;
        if self
            .active_generation
            .lock()
            .is_ok_and(|active| active.is_some())
        {
            return Err(RuntimeError::ModelLifecycle(
                "cannot unload a model while generation is active".to_owned(),
            ));
        }
        self.bus
            .publish(&RuntimeEvent::ModelUnloading(id.to_owned()));
        self.models.set_status(id, &ModelStatus::Unloading)?;
        if let Err(error) = self.backends.unload_model(id) {
            let _ = self.models.set_status(id, &ModelStatus::Loaded);
            return Err(error);
        }
        self.models.set_status(id, &ModelStatus::Registered)?;
        self.loaded
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("loader unavailable".to_owned()))?
            .remove(id);
        if let Ok(mut last_used) = self.last_used.lock() {
            last_used.remove(id);
        }
        Ok(())
    }

    fn tokenize(&self, text: &str) -> Result<usize, RuntimeError> {
        let count = self.backends.tokenize(text)?;
        self.bus
            .publish(&RuntimeEvent::TokenizerReady("llama.cpp".to_owned()));
        Ok(count)
    }

    fn embeddings(&self, request: &EmbeddingsRequest) -> Result<EmbeddingsResult, RuntimeError> {
        self.backends.embeddings(request)
    }

    fn tool_support(&self, model_id: &str) -> crate::backends::ToolSupport {
        self.backends.tool_support(model_id)
    }

    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::too_many_lines)]
    fn generate(&self, request: GenerationRequest) -> Result<GenerationStream, RuntimeError> {
        let cancellation = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        if self
            .active_generation
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("generation unavailable".to_owned()))?
            .is_some()
        {
            let mut queue = self.pending_generations.lock().map_err(|_| {
                RuntimeError::ModelLifecycle("generation queue unavailable".to_owned())
            })?;
            if queue.len() >= self.resource_policy.queue_capacity {
                return Err(RuntimeError::ResourceAdmission {
                    message: "generation queue is full while another request is active".to_owned(),
                    retryable: true,
                });
            }
            queue.push_back(PendingGeneration {
                request,
                sender,
                cancellation: cancellation.clone(),
                enqueued_at: Instant::now(),
            });
            return Ok(GenerationStream::new(receiver, cancellation));
        }
        self.recover_for_generation(&request)?;
        let generation_id = format!(
            "talvik-gen-{}-{}",
            std::process::id(),
            GENERATION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let mut active_generation = self
            .active_generation
            .lock()
            .map_err(|_| RuntimeError::ModelLifecycle("generation unavailable".to_owned()))?;
        if active_generation.is_some() {
            return Err(RuntimeError::ModelLifecycle(
                "generation capacity exhausted: one active generation is supported".to_owned(),
            ));
        }
        *active_generation = Some((
            request.request_id.clone(),
            generation_id.clone(),
            cancellation.clone(),
        ));
        drop(active_generation);
        let cancel_for_worker = cancellation.clone();
        let bus = self.bus.clone();
        let backends = self.backends.clone();
        let active_generation = self.active_generation.clone();
        let runtime = self.clone();
        let generation_id_for_worker = generation_id.clone();
        thread::spawn(move || {
            let started = Instant::now();
            bus.publish(&RuntimeEvent::GenerationStarted(request.request_id.clone()));
            let backend_request = crate::backends::GenerationRequest {
                request_id: request.request_id.clone(),
                model_id: request.model_id.clone(),
                input: request.prompt.clone(),
                options: request.options.clone(),
            };
            let mut generated = String::new();
            let mut generated_tokens = 0_u64;
            let mut first = true;
            let result = backends.generate_streaming(
                &backend_request,
                &mut |token| {
                    if cancel_for_worker.load(Ordering::SeqCst) {
                        return false;
                    }
                    if first {
                        bus.publish(&RuntimeEvent::FirstToken(request.request_id.clone()));
                        first = false;
                    }
                    generated.push_str(token);
                    generated_tokens += 1;
                    let _ = sender.send(GenerationMessage::Token(token.to_owned()));
                    bus.publish(&RuntimeEvent::TokenGenerated(request.request_id.clone()));
                    true
                },
                &cancel_for_worker,
            );
            if cancel_for_worker.load(Ordering::SeqCst)
                || result
                    .as_ref()
                    .is_err_and(|error| error.to_string().contains("cancelled"))
            {
                let latency_ms = started.elapsed().as_millis();
                let statistics = GenerationStatistics {
                    generated_tokens,
                    tokens_per_second: if latency_ms == 0 {
                        generated_tokens as f64
                    } else {
                        generated_tokens as f64 / (latency_ms as f64 / 1000.0)
                    },
                    latency_ms,
                    inference_time_ms: latency_ms,
                    metrics_known: true,
                    ..GenerationStatistics::default()
                };
                if let Ok(mut active) = active_generation.lock() {
                    if active
                        .as_ref()
                        .is_some_and(|(_, _, current)| Arc::ptr_eq(current, &cancel_for_worker))
                    {
                        *active = None;
                    }
                }
                let _ = sender.send(GenerationMessage::Cancelled(GenerationResult {
                    request_id: request.request_id.clone(),
                    generation_id: generation_id_for_worker.clone(),
                    text: generated,
                    statistics,
                }));
                bus.publish(&RuntimeEvent::GenerationCancelled(
                    request.request_id.clone(),
                ));
                runtime.start_next_queued();
                return;
            }
            let statistics = match result {
                Ok(statistics) => statistics,
                Err(error) => {
                    let message = error.to_string();
                    if let Ok(mut active) = active_generation.lock() {
                        if active
                            .as_ref()
                            .is_some_and(|(_, _, current)| Arc::ptr_eq(current, &cancel_for_worker))
                        {
                            *active = None;
                        }
                    }
                    let _ = sender.send(GenerationMessage::Failed(error));
                    bus.publish(&RuntimeEvent::GenerationFailed(message));
                    runtime.start_next_queued();
                    return;
                }
            };
            let latency_ms = started.elapsed().as_millis();
            let statistics = GenerationStatistics {
                prompt_tokens: statistics.prompt_tokens,
                generated_tokens: statistics.generated_tokens,
                tokens_per_second: statistics.tokens_per_second,
                latency_ms,
                inference_time_ms: statistics.inference_time_ms,
                backend_inference_time_ms: statistics.backend_inference_time_ms,
                time_to_first_token_ms: statistics.time_to_first_token_ms,
                prompt_evaluation_time_ms: statistics.prompt_evaluation_time_ms,
                generation_time_ms: statistics.generation_time_ms,
                context_tokens: statistics.context_tokens,
                metrics_known: statistics.metrics_known,
                termination: statistics.termination,
                cache_hit: statistics.cache_hit,
                reused_prompt_tokens: statistics.reused_prompt_tokens,
                execution: statistics.execution,
            };
            let result = GenerationResult {
                request_id: request.request_id.clone(),
                generation_id: generation_id_for_worker,
                text: generated,
                statistics,
            };
            if let Ok(mut active) = active_generation.lock() {
                if active
                    .as_ref()
                    .is_some_and(|(_, _, current)| Arc::ptr_eq(current, &cancel_for_worker))
                {
                    *active = None;
                }
            }
            let _ = sender.send(GenerationMessage::Completed(result));
            bus.publish(&RuntimeEvent::GenerationCompleted(request.request_id));
            runtime.start_next_queued();
        });
        Ok(GenerationStream::new(receiver, cancellation))
    }

    fn cancel_generation(&self) {
        if let Ok(active) = self.active_generation.lock() {
            if let Some((_, _, cancellation)) = active.as_ref() {
                cancellation.store(true, Ordering::SeqCst);
            }
        }
    }

    fn cancel_generation_for(&self, request_id: &str) {
        if let Ok(active) = self.active_generation.lock() {
            if let Some((active_id, _, cancellation)) = active.as_ref() {
                if active_id == request_id {
                    cancellation.store(true, Ordering::SeqCst);
                    return;
                }
            }
        }
        let pending = self.pending_generations.lock().ok().and_then(|mut queue| {
            queue
                .iter()
                .position(|pending| pending.request.request_id == request_id)
                .and_then(|position| queue.remove(position))
        });
        if let Some(pending) = pending {
            pending.cancellation.store(true, Ordering::SeqCst);
            let _ = pending
                .sender
                .send(GenerationMessage::Cancelled(GenerationResult {
                    request_id: pending.request.request_id,
                    generation_id: String::new(),
                    text: String::new(),
                    statistics: GenerationStatistics::default(),
                }));
        }
    }

    fn active_generation_id(&self, request_id: &str) -> Option<String> {
        self.active_generation.lock().ok().and_then(|active| {
            active.as_ref().and_then(|(active_id, generation_id, _)| {
                (active_id == request_id).then(|| generation_id.clone())
            })
        })
    }

    fn hardware(&self) -> HardwareSnapshot {
        self.hardware.snapshot()
    }

    fn event_bus(&self) -> EventBus {
        self.bus.clone()
    }

    fn execute_command(&self, command: &str) -> String {
        self.bus
            .publish(&RuntimeEvent::CommandStarted(command.to_owned()));
        self.bus.publish(&RuntimeEvent::ThinkingStarted);
        self.bus.publish(&RuntimeEvent::ThinkingFinished);
        self.bus.publish(&RuntimeEvent::StreamingStarted);
        self.bus.publish(&RuntimeEvent::StreamingStopped);
        self.bus
            .publish(&RuntimeEvent::CommandCompleted(command.to_owned()));
        format!("Mock runtime received: {command}")
    }

    fn input_started(&self) {
        self.bus.publish(&RuntimeEvent::InputStarted);
    }

    fn input_stopped(&self) {
        self.bus.publish(&RuntimeEvent::InputStopped);
    }

    fn stop(&self) {
        self.bus.publish(&RuntimeEvent::RuntimeStopped);
    }
}

/// Construct Talvik's production runtime from validated configuration.
///
/// # Errors
///
/// Returns an invalid-configuration error when required configuration values
/// are empty.
pub fn start(config: RuntimeConfig, bus: EventBus) -> Result<MarinaRuntime, RuntimeError> {
    config
        .validate()
        .map_err(RuntimeError::InvalidConfiguration)?;
    Ok(MarinaRuntime::start(config, bus))
}

#[cfg(test)]
mod tests {
    use super::{
        admit_memory, MarinaRuntime, ResourcePolicy, KV_BYTES_PER_CONTEXT_TOKEN,
        SAFETY_MARGIN_BYTES,
    };
    use crate::{
        GenerationMessage, GenerationOptions, GenerationRequest, HardwareService, HardwareSnapshot,
        LlamaCppAdapter, MockBackend, ModelMetadata, ModelRegistry, ModelStatus, RuntimeService,
    };
    use oid_shared::{EventBus, RuntimeConfig, RuntimeEvent};
    use std::sync::atomic::AtomicBool;
    use std::sync::{mpsc, Arc};
    use std::time::{Duration, Instant};

    #[test]
    fn marina_runtime_is_deterministic_and_publishes_events() {
        let bus = EventBus::new();
        let receiver = bus.subscribe();
        let isolated_models = std::env::temp_dir().join(format!(
            "oid-runtime-empty-models-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let config = RuntimeConfig {
            model_directory: Some(isolated_models),
            ..RuntimeConfig::default()
        };
        let runtime = MarinaRuntime::start(config, bus);
        let expected_backend = if LlamaCppAdapter::native_available() {
            "llama.cpp"
        } else {
            "None"
        };
        assert_eq!(runtime.snapshot().backend, expected_backend);
        assert_eq!(runtime.snapshot().models, 0);
        let _ = runtime.execute_command("status");
        assert!(receiver
            .try_iter()
            .any(|event| matches!(event, RuntimeEvent::RuntimeStarted)));
    }

    #[test]
    fn startup_clears_persisted_transient_model_statuses() {
        let root = std::env::temp_dir().join(format!(
            "oid-runtime-status-reset-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create root");
        let registry_path = root.join("registry.db");
        let registry = ModelRegistry::open(&registry_path, EventBus::new()).expect("open registry");
        registry
            .register(ModelMetadata {
                id: "stale-model".to_owned(),
                name: "stale-model.gguf".to_owned(),
                family: "unknown".to_owned(),
                backend: "llama.cpp".to_owned(),
                quantization: "unknown".to_owned(),
                context_window: 0,
                memory_requirement_mb: 1,
                capabilities: vec!["text".to_owned()],
                status: ModelStatus::Loaded,
                checksum: None,
                location: root.join("stale-model.gguf").display().to_string(),
            })
            .expect("register stale model");
        drop(registry);

        let runtime = MarinaRuntime::start(
            RuntimeConfig {
                model_directory: Some(root.join("models")),
                registry_path: Some(registry_path),
                ..RuntimeConfig::default()
            },
            EventBus::new(),
        );

        assert_eq!(runtime.model_list()[0].status, ModelStatus::Registered);
        std::fs::remove_dir_all(root).expect("remove test data");
    }

    #[test]
    fn model_pull_imports_local_artifact_into_configured_store() {
        let root = std::env::temp_dir().join(format!(
            "oid-model-pull-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let source = root.join("source.gguf");
        let destination = root.join("models");
        let registry = root.join("registry.db");
        std::fs::create_dir_all(&root).expect("create root");
        std::fs::write(&source, b"GGUF").expect("write source");
        let config = RuntimeConfig {
            model_directory: Some(destination.clone()),
            registry_path: Some(registry),
            ..RuntimeConfig::default()
        };
        let runtime = MarinaRuntime::start(config, EventBus::new());
        let model = runtime
            .model_pull(
                source.to_str().expect("source path"),
                Some("local-demo"),
                None,
            )
            .expect("pull local model");
        assert_eq!(model.id, "local-demo");
        assert!(destination.join("source.gguf").is_file());
        assert_eq!(runtime.model_list().len(), 1);
        std::fs::remove_dir_all(root).expect("remove test data");
    }

    #[test]
    fn memory_admission_rejects_unknown_and_insufficient_capacity() {
        let policy = ResourcePolicy::default();
        assert!(admit_memory(None, 0, 1, &policy).is_err());
        assert!(admit_memory(Some(512 * 1_048_576), 256 * 1_048_576, 32_768, &policy).is_err());
        assert!(admit_memory(Some(2 * 1_073_741_824), 256 * 1_048_576, 4_096, &policy).is_ok());
    }

    #[test]
    fn memory_admission_accepts_exact_boundary_and_rejects_one_byte_below() {
        let model_bytes: u64 = 1_596_822_528;
        let context_size = 4_096;
        let context_bytes = u64::from(context_size) * KV_BYTES_PER_CONTEXT_TOKEN;
        let required = model_bytes
            .saturating_add(context_bytes)
            .saturating_add(SAFETY_MARGIN_BYTES);

        let policy = ResourcePolicy::default();
        assert!(admit_memory(Some(required), model_bytes, context_size, &policy).is_ok());
        assert!(admit_memory(Some(required - 1), model_bytes, context_size, &policy).is_err());
    }

    #[test]
    fn resident_model_is_not_counted_as_a_new_physical_allocation() {
        let policy = ResourcePolicy::default();
        let resident = 1_596_822_528;
        let available = 67_108_864 + SAFETY_MARGIN_BYTES;
        assert!(
            super::admit_memory_with_resident(Some(available), resident, 0, 4_096, &policy,)
                .is_ok()
        );
    }

    #[test]
    fn aggregate_budget_includes_resident_models_once() {
        let policy = ResourcePolicy {
            memory_budget_bytes: Some(2_000),
            safety_margin_bytes: 100,
            ..ResourcePolicy::default()
        };
        assert!(super::admit_memory_with_resident(Some(10_000), 1_000, 0, 0, &policy).is_ok());
        assert!(super::admit_memory_with_resident(Some(10_000), 1_900, 1, 0, &policy).is_err());
    }

    #[test]
    fn retention_selects_expired_lru_and_respects_pinning_and_active_requests() {
        let mut runtime = MarinaRuntime::start_with_backend(
            RuntimeConfig::default(),
            EventBus::new(),
            Box::new(MockBackend),
        );
        runtime.resource_policy.idle_retention = Duration::from_secs(60);
        runtime.loaded.lock().expect("loaded lock").extend([
            (
                "alpha".to_owned(),
                crate::backends::LoadedModel {
                    model_id: "alpha".to_owned(),
                    memory_bytes: 1,
                },
            ),
            (
                "beta".to_owned(),
                crate::backends::LoadedModel {
                    model_id: "beta".to_owned(),
                    memory_bytes: 1,
                },
            ),
        ]);
        let expired = Instant::now()
            .checked_sub(Duration::from_secs(120))
            .expect("test duration is representable");
        runtime
            .last_used
            .lock()
            .expect("last-used lock")
            .extend([("alpha".to_owned(), expired), ("beta".to_owned(), expired)]);

        assert_eq!(runtime.eviction_candidate("none").as_deref(), Some("alpha"));
        runtime
            .resource_policy
            .pinned_models
            .insert("alpha".to_owned());
        assert_eq!(runtime.eviction_candidate("none").as_deref(), Some("beta"));
        *runtime.active_generation.lock().expect("active lock") = Some((
            "beta".to_owned(),
            "generation".to_owned(),
            Arc::new(AtomicBool::new(false)),
        ));
        assert_eq!(runtime.eviction_candidate("none"), None);
    }

    #[test]
    fn retention_keeps_recent_idle_models_until_the_expiry_boundary() {
        let mut runtime = MarinaRuntime::start_with_backend(
            RuntimeConfig::default(),
            EventBus::new(),
            Box::new(MockBackend),
        );
        runtime.resource_policy.idle_retention = Duration::from_secs(60);
        runtime.loaded.lock().expect("loaded lock").insert(
            "recent".to_owned(),
            crate::backends::LoadedModel {
                model_id: "recent".to_owned(),
                memory_bytes: 1,
            },
        );
        runtime
            .last_used
            .lock()
            .expect("last-used lock")
            .insert("recent".to_owned(), Instant::now());
        assert_eq!(runtime.eviction_candidate("none"), None);
    }

    #[test]
    fn queue_capacity_and_admission_deadline_are_bounded() {
        let mut runtime = MarinaRuntime::start_with_backend(
            RuntimeConfig::default(),
            EventBus::new(),
            Box::new(MockBackend),
        );
        runtime.resource_policy.queue_capacity = 1;
        *runtime.active_generation.lock().expect("active lock") = Some((
            "active".to_owned(),
            "generation".to_owned(),
            Arc::new(AtomicBool::new(false)),
        ));
        let request = |id: &str| GenerationRequest {
            request_id: id.to_owned(),
            model_id: "model".to_owned(),
            prompt: "prompt".to_owned(),
            options: GenerationOptions::default(),
        };
        let queued = runtime
            .generate(request("queued"))
            .expect("queue one request");
        assert!(matches!(
            runtime.generate(request("overflow")),
            Err(oid_shared::RuntimeError::ResourceAdmission { .. })
        ));
        runtime.cancel_generation_for("queued");
        assert_eq!(runtime.snapshot().pending_requests, 0);
        let (sender, receiver) = mpsc::channel();
        runtime
            .pending_generations
            .lock()
            .expect("queue lock")
            .push_back(super::PendingGeneration {
                request: request("expired"),
                sender,
                cancellation: Arc::new(AtomicBool::new(false)),
                enqueued_at: Instant::now()
                    .checked_sub(Duration::from_secs(31))
                    .expect("test duration is representable"),
            });
        runtime.start_next_queued();
        assert!(matches!(
            receiver.recv().expect("deadline response"),
            GenerationMessage::Failed(oid_shared::RuntimeError::ResourceAdmission { .. })
        ));
        queued.cancel();
    }

    #[test]
    fn rejected_model_load_preserves_existing_resident_model() {
        let root = std::env::temp_dir().join(format!(
            "oid-runtime-memory-admission-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create root");
        let bus = EventBus::new();
        let available = 600 * 1_048_576;
        let hardware = HardwareService::from_snapshot(HardwareSnapshot {
            available_ram_bytes: Some(available),
            ..HardwareSnapshot::default()
        });
        let runtime = MarinaRuntime::start_with_backend_and_hardware(
            RuntimeConfig {
                model_directory: Some(root.join("models")),
                registry_path: Some(root.join("registry.db")),
                ..RuntimeConfig::default()
            },
            bus,
            Box::new(MockBackend),
            hardware,
        );
        for (id, memory_requirement_mb) in [("alpha", 1), ("beta", 64)] {
            runtime
                .models
                .register(ModelMetadata {
                    id: id.to_owned(),
                    name: format!("{id}.gguf"),
                    family: "test".to_owned(),
                    backend: "mock".to_owned(),
                    quantization: "test".to_owned(),
                    context_window: 4096,
                    memory_requirement_mb,
                    capabilities: vec!["chat".to_owned()],
                    status: ModelStatus::Registered,
                    checksum: None,
                    location: root.join(format!("{id}.gguf")).display().to_string(),
                })
                .expect("register model");
        }

        runtime.model_load("alpha").expect("alpha load");
        assert!(runtime.model_load("beta").is_err());
        assert_eq!(runtime.snapshot().loaded_models, vec!["alpha".to_owned()]);
        assert_eq!(
            runtime.model_inspect("beta").expect("beta").status,
            ModelStatus::Registered
        );
        assert_eq!(runtime.snapshot().health, "Healthy");

        std::fs::remove_dir_all(root).expect("remove test data");
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn generation_recovers_by_eviction_and_reloads_registered_models() {
        let root = std::env::temp_dir().join(format!(
            "oid-runtime-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create root");
        let mut runtime = MarinaRuntime::start_with_backend_and_hardware(
            RuntimeConfig {
                model_directory: Some(root.join("models")),
                registry_path: Some(root.join("registry.db")),
                ..RuntimeConfig::default()
            },
            EventBus::new(),
            Box::new(MockBackend),
            HardwareService::from_snapshot(HardwareSnapshot {
                available_ram_bytes: Some(640 * 1_048_576),
                ..HardwareSnapshot::default()
            }),
        );
        runtime.resource_policy.max_resident_models = 1;
        for id in ["alpha", "beta"] {
            runtime
                .models
                .register(ModelMetadata {
                    id: id.to_owned(),
                    name: format!("{id}.gguf"),
                    family: "test".to_owned(),
                    backend: "mock".to_owned(),
                    quantization: "test".to_owned(),
                    context_window: 4096,
                    memory_requirement_mb: if id == "alpha" { 1 } else { 64 },
                    capabilities: vec!["chat".to_owned()],
                    status: ModelStatus::Registered,
                    checksum: None,
                    location: root.join(format!("{id}.gguf")).display().to_string(),
                })
                .expect("register model");
        }
        runtime.model_load("alpha").expect("alpha load");
        runtime
            .resource_policy
            .pinned_models
            .insert("alpha".to_owned());
        let pinned_error = runtime.generate(GenerationRequest {
            request_id: "pinned-rejection".to_owned(),
            model_id: "beta".to_owned(),
            prompt: "hello".to_owned(),
            options: GenerationOptions::default(),
        });
        assert!(matches!(
            pinned_error,
            Err(oid_shared::RuntimeError::ResourceAdmission { .. })
        ));
        assert!(runtime
            .snapshot()
            .loaded_models
            .contains(&"alpha".to_owned()));
        runtime.resource_policy.pinned_models.clear();
        let first = runtime
            .generate(GenerationRequest {
                request_id: "active".to_owned(),
                model_id: "alpha".to_owned(),
                prompt: "hello".to_owned(),
                options: GenerationOptions::default(),
            })
            .expect("active generation");
        let queued = runtime
            .generate(GenerationRequest {
                request_id: "queued".to_owned(),
                model_id: "alpha".to_owned(),
                prompt: "queued".to_owned(),
                options: GenerationOptions::default(),
            })
            .expect("queued generation");
        runtime.cancel_generation_for("queued");
        assert_eq!(runtime.snapshot().pending_requests, 0);
        first.cancel();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            assert!(!remaining.is_zero(), "active generation did not terminate");
            match first.recv_timeout(remaining) {
                Ok(
                    GenerationMessage::Completed(_)
                    | GenerationMessage::Cancelled(_)
                    | GenerationMessage::Failed(_),
                ) => break,
                Ok(GenerationMessage::Token(_)) => {}
                Err(error) => panic!("active generation did not terminate: {error}"),
            }
        }
        let queued_message = queued
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("queued generation did not terminate");
        assert!(matches!(
            queued_message,
            GenerationMessage::Completed(_)
                | GenerationMessage::Cancelled(_)
                | GenerationMessage::Failed(_)
        ));
        let request = |id: &str| GenerationRequest {
            request_id: format!("request-{id}"),
            model_id: id.to_owned(),
            prompt: "hello".to_owned(),
            options: GenerationOptions {
                max_tokens: 1,
                ..GenerationOptions::default()
            },
        };
        let stream = runtime.generate(request("beta")).expect("beta recovers");
        stream.cancel();
        let _ = stream.recv_timeout(std::time::Duration::from_secs(2));
        assert!(runtime
            .snapshot()
            .loaded_models
            .contains(&"beta".to_owned()));
        assert_eq!(
            runtime.model_inspect("alpha").expect("alpha").status,
            ModelStatus::Registered
        );

        let stream = runtime.generate(request("alpha")).expect("alpha reloads");
        stream.cancel();
        let _ = stream.recv_timeout(std::time::Duration::from_secs(2));
        assert!(runtime
            .snapshot()
            .loaded_models
            .contains(&"alpha".to_owned()));
        assert_eq!(
            runtime.model_inspect("beta").expect("beta").status,
            ModelStatus::Registered
        );

        std::fs::remove_dir_all(root).expect("remove test data");
    }

    #[test]
    fn multiple_resident_models_are_independently_loadable_and_unloadable() {
        let root = std::env::temp_dir().join(format!(
            "oid-runtime-multi-model-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create root");
        let bus = EventBus::new();
        let runtime = MarinaRuntime::start_with_backend(
            RuntimeConfig {
                model_directory: Some(root.join("models")),
                registry_path: Some(root.join("registry.db")),
                ..RuntimeConfig::default()
            },
            bus,
            Box::new(MockBackend),
        );
        for id in ["alpha", "beta"] {
            runtime
                .models
                .register(ModelMetadata {
                    id: id.to_owned(),
                    name: format!("{id}.gguf"),
                    family: "test".to_owned(),
                    backend: "mock".to_owned(),
                    quantization: "test".to_owned(),
                    context_window: 4096,
                    memory_requirement_mb: 1,
                    capabilities: vec!["chat".to_owned()],
                    status: ModelStatus::Registered,
                    checksum: None,
                    location: root.join(format!("{id}.gguf")).display().to_string(),
                })
                .expect("register model");
            runtime.model_load(id).expect("load model");
        }
        assert_eq!(
            runtime.snapshot().loaded_models,
            vec!["alpha".to_owned(), "beta".to_owned()]
        );
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.resident_models.len(), 2);
        assert_eq!(snapshot.pending_requests, 0);
        assert_eq!(snapshot.active_requests, 0);
        assert!(snapshot
            .resident_models
            .iter()
            .all(|model| model.memory_bytes == Some(1_048_576)));
        runtime.model_unload("alpha").expect("unload alpha");
        assert_eq!(runtime.snapshot().loaded_models, vec!["beta".to_owned()]);
        runtime.model_unload("beta").expect("unload beta");
        assert!(runtime.snapshot().loaded_models.is_empty());
        std::fs::remove_dir_all(root).expect("remove test data");
    }
}
