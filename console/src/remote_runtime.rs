//! OID session adapter that delegates model execution to Marina over HTTP.

use oid_common::EvidenceId;
use oid_evidence_engine::{EvidenceStore, FileEvidenceStore, InferenceEvidenceRecord};
use oid_model_client::{ChatRequest, MarinaHttpClient, ModelClient, ModelClientError};
use oid_runtime::{
    BackendHealth, BackendSummary, GenerationMessage, GenerationRequest, GenerationResult,
    GenerationStatistics, GenerationStream, HardwareSnapshot, MarinaRuntime, ModelMetadata,
    ModelStatus, RuntimeError, RuntimeService, RuntimeSnapshot, RuntimeStatus,
};
use oid_shared::{EventBus, RuntimeEvent};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// OID-facing runtime that keeps policy/console operations local while routing
/// model discovery and inference through Marina's provider-neutral HTTP client.
pub struct RemoteRuntimeService {
    local: Box<dyn RuntimeService>,
    client: MarinaHttpClient,
    active: Arc<Mutex<Option<(String, Arc<AtomicBool>)>>>,
    evidence: Arc<Mutex<FileEvidenceStore>>,
    session_id: String,
}

impl std::fmt::Debug for RemoteRuntimeService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RemoteRuntimeService")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl RemoteRuntimeService {
    /// Wrap the local OID runtime with a Marina model client.
    pub fn new(local: MarinaRuntime, client: MarinaHttpClient) -> Self {
        let root = std::env::var_os("OID_STATE_DIR").map_or_else(
            || std::env::temp_dir().join("oid-console"),
            std::path::PathBuf::from,
        );
        let session_id = std::env::var("OID_SESSION_ID")
            .unwrap_or_else(|_| format!("oid-session-{}-{}", std::process::id(), timestamp()));
        Self {
            local: Box::new(local),
            client,
            active: Arc::new(Mutex::new(None)),
            evidence: Arc::new(Mutex::new(FileEvidenceStore::new(
                root.join("evidence.log"),
            ))),
            session_id,
        }
    }

    fn unavailable(error: impl ToString) -> RuntimeError {
        RuntimeError::ModelLifecycle(error.to_string())
    }

    fn append_evidence(
        evidence: &Arc<Mutex<FileEvidenceStore>>,
        record: InferenceEvidenceRecord,
    ) -> Result<(), RuntimeError> {
        evidence
            .lock()
            .map_err(|_| {
                RuntimeError::ModelLifecycle("inference evidence lock unavailable".to_owned())
            })?
            .append_inference(record)
            .map_err(|error| RuntimeError::ModelLifecycle(error.to_string()))
    }

    fn remote_models(&self) -> Vec<ModelMetadata> {
        let models = match self.client.list_models() {
            Ok(models) => models,
            Err(error) => {
                eprintln!("OID Marina model discovery failed: {error}");
                return Vec::new();
            }
        };
        models
            .into_iter()
            .map(|model| ModelMetadata {
                name: model.id.clone(),
                id: model.id,
                family: "remote".to_owned(),
                backend: "marina".to_owned(),
                quantization: "unknown".to_owned(),
                context_window: 32_768,
                memory_requirement_mb: 0,
                capabilities: vec!["chat".to_owned()],
                status: ModelStatus::Registered,
                checksum: None,
                location: "marina-http".to_owned(),
            })
            .collect()
    }
}

impl RuntimeService for RemoteRuntimeService {
    fn status(&self) -> RuntimeStatus {
        self.local.status()
    }

    fn snapshot(&self) -> RuntimeSnapshot {
        let mut snapshot = self.local.snapshot();
        snapshot.backend = "marina-http".to_owned();
        let models = self.remote_models();
        snapshot.models = models.len();
        snapshot.loaded_model = std::env::var("OID_MARINA_MODEL")
            .ok()
            .or_else(|| models.first().map(|model| model.id.clone()));
        snapshot.generating = self.active.lock().is_ok_and(|active| active.is_some());
        snapshot
    }

    fn backend_list(&self) -> Vec<BackendSummary> {
        self.local.backend_list()
    }

    fn backend_health(&self, id: &str) -> Option<BackendHealth> {
        self.local.backend_health(id)
    }

    fn model_list(&self) -> Vec<ModelMetadata> {
        self.remote_models()
    }

    fn model_inspect(&self, id: &str) -> Option<ModelMetadata> {
        self.remote_models()
            .into_iter()
            .find(|model| model.id == id)
    }

    fn model_pull(
        &self,
        _source: &str,
        _model_id: Option<&str>,
        _checksum: Option<&str>,
    ) -> Result<ModelMetadata, RuntimeError> {
        Err(RuntimeError::NotImplemented("remote model acquisition"))
    }

    fn model_load(&self, _id: &str) -> Result<(), RuntimeError> {
        Err(RuntimeError::NotImplemented("remote model administration"))
    }

    fn model_unload(&self, _id: &str) -> Result<(), RuntimeError> {
        Err(RuntimeError::NotImplemented("remote model administration"))
    }

    fn tokenize(&self, _text: &str) -> Result<usize, RuntimeError> {
        Err(RuntimeError::NotImplemented("remote tokenization"))
    }

    fn generate(&self, request: GenerationRequest) -> Result<GenerationStream, RuntimeError> {
        let cancellation = Arc::new(AtomicBool::new(false));
        let mut active = self.active.lock().map_err(|_| {
            RuntimeError::ModelLifecycle("remote generation state unavailable".to_owned())
        })?;
        if active.is_some() {
            return Err(RuntimeError::ModelLifecycle(
                "remote generation capacity exhausted".to_owned(),
            ));
        }
        let started_at = timestamp();
        let started_evidence_id = format!("inference-{}-started", request.request_id);
        Self::append_evidence(
            &self.evidence,
            InferenceEvidenceRecord {
                id: EvidenceId::new(&started_evidence_id)
                    .map_err(|error| RuntimeError::ModelLifecycle(error.to_string()))?,
                session_id: self.session_id.clone(),
                request_id: request.request_id.clone(),
                generation_id: None,
                model: request.model_id.clone(),
                backend: "marina-http".to_owned(),
                parameters: serde_json::json!({
                    "max_tokens": request.options.max_tokens,
                    "context_size": request.options.context_size,
                    "temperature": request.options.temperature,
                    "top_p": request.options.top_p,
                    "top_k": request.options.top_k,
                    "seed": request.options.seed,
                })
                .to_string(),
                started_at: started_at.clone(),
                completed_at: None,
                outcome: "started".to_owned(),
                termination_reason: None,
                error_class: None,
                verification: "pending".to_owned(),
                links: Vec::new(),
                metrics: "prompt_tokens=unknown;generated_tokens=unknown;context_tokens=unknown;tokens_per_second=unknown;latency_ms=unknown;inference_time_ms=unknown".to_owned(),
            },
        )?;
        *active = Some((request.request_id.clone(), Arc::clone(&cancellation)));
        drop(active);

        let (sender, receiver) = mpsc::channel();
        let client = self.client.clone();
        let active_state = Arc::clone(&self.active);
        let evidence = Arc::clone(&self.evidence);
        let session_id = self.session_id.clone();
        let worker_cancellation = Arc::clone(&cancellation);
        let event_bus = self.local.event_bus();
        let request_id = request.request_id.clone();
        let timeout = std::env::var("OID_MARINA_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map_or(Duration::from_secs(120), Duration::from_millis);
        std::thread::spawn(move || {
            event_bus.publish(&RuntimeEvent::GenerationStarted(request_id.clone()));
            let result = client.chat(
                &ChatRequest {
                    model: request.model_id.clone(),
                    prompt: request.prompt,
                    request_id: request_id.clone(),
                    max_tokens: request.options.max_tokens,
                    context_size: request.options.context_size,
                },
                timeout,
                &worker_cancellation,
            );
            match result {
                Ok(response) => {
                    let metrics = response.metrics.clone();
                    let statistics = statistics_from_metrics(&metrics);
                    let terminal = InferenceEvidenceRecord {
                        id: EvidenceId::new(format!("inference-{request_id}-completed"))
                            .expect("request ids are non-empty"),
                        session_id,
                        request_id: request_id.clone(),
                        generation_id: Some(response.generation_id.clone()),
                        model: response.model.clone(),
                        backend: "marina-http".to_owned(),
                        parameters: "{}".to_owned(),
                        started_at: started_at.clone(),
                        completed_at: Some(timestamp()),
                        outcome: "completed".to_owned(),
                        termination_reason: None,
                        error_class: None,
                        verification: "response received".to_owned(),
                        links: vec![format!("inference-{request_id}-started")],
                        metrics: render_metrics(&metrics),
                    };
                    if let Err(error) = Self::append_evidence(&evidence, terminal) {
                        let message = error.to_string();
                        let _ = sender.send(GenerationMessage::Failed(error));
                        event_bus.publish(&RuntimeEvent::GenerationFailed(message));
                    } else {
                        event_bus.publish(&RuntimeEvent::FirstToken(request_id.clone()));
                        event_bus.publish(&RuntimeEvent::TokenGenerated(request_id.clone()));
                        let _ = sender.send(GenerationMessage::Token(response.text.clone()));
                        let _ = sender.send(GenerationMessage::Completed(GenerationResult {
                            request_id: response.request_id,
                            generation_id: response.generation_id,
                            text: response.text,
                            statistics,
                        }));
                        event_bus.publish(&RuntimeEvent::GenerationCompleted(request_id));
                    }
                }
                Err(error) => {
                    let cancelled = matches!(error, ModelClientError::Cancelled)
                        || worker_cancellation.load(Ordering::SeqCst);
                    let outcome = if cancelled { "cancelled" } else { "failed" };
                    let reason = error.to_string();
                    let terminal = InferenceEvidenceRecord {
                        id: EvidenceId::new(format!("inference-{request_id}-{outcome}"))
                            .expect("request ids are non-empty"),
                        session_id,
                        request_id: request_id.clone(),
                        generation_id: if cancelled {
                            client
                                .cancelled_generation_id(&request_id)
                                .or_else(|| Some(request_id.clone()))
                        } else {
                            None
                        },
                        model: request.model_id.clone(),
                        backend: "marina-http".to_owned(),
                        parameters: "{}".to_owned(),
                        started_at,
                        completed_at: Some(timestamp()),
                        outcome: outcome.to_owned(),
                        termination_reason: (cancelled
                            || matches!(&error, ModelClientError::Timeout))
                            .then_some(reason.clone()),
                        error_class: (!cancelled).then(|| error_class(&error)),
                        verification: "not verified".to_owned(),
                        links: vec![format!("inference-{request_id}-started")],
                        metrics: "prompt_tokens=unknown;generated_tokens=unknown;context_tokens=unknown;tokens_per_second=unknown;latency_ms=unknown;inference_time_ms=unknown".to_owned(),
                    };
                    let evidence_result = Self::append_evidence(&evidence, terminal);
                    if cancelled && evidence_result.is_ok() {
                        let _ = sender.send(GenerationMessage::Cancelled(GenerationResult {
                            request_id: request_id.clone(),
                            generation_id: client
                                .cancelled_generation_id(&request_id)
                                .unwrap_or_else(|| request_id.clone()),
                            text: String::new(),
                            statistics: GenerationStatistics::default(),
                        }));
                        event_bus.publish(&RuntimeEvent::GenerationCancelled(request_id));
                    } else {
                        let message = match evidence_result {
                            Ok(()) => reason,
                            Err(error) => error.to_string(),
                        };
                        let _ = sender.send(GenerationMessage::Failed(Self::unavailable(&message)));
                        event_bus.publish(&RuntimeEvent::GenerationFailed(message));
                    }
                }
            }
            if let Ok(mut active) = active_state.lock() {
                *active = None;
            }
        });
        Ok(GenerationStream::from_parts(receiver, cancellation))
    }

    fn cancel_generation(&self) {
        if let Ok(active) = self.active.lock() {
            if let Some((_, cancellation)) = active.as_ref() {
                cancellation.store(true, Ordering::SeqCst);
            }
        }
    }

    fn cancel_generation_for(&self, request_id: &str) {
        if let Ok(active) = self.active.lock() {
            if let Some((active_id, cancellation)) = active.as_ref() {
                if active_id == request_id {
                    cancellation.store(true, Ordering::SeqCst);
                }
            }
        }
    }

    fn hardware(&self) -> HardwareSnapshot {
        self.local.hardware()
    }

    fn event_bus(&self) -> EventBus {
        self.local.event_bus()
    }

    fn execute_command(&self, command: &str) -> String {
        self.local.execute_command(command)
    }

    fn input_started(&self) {
        self.local.input_started();
    }

    fn input_stopped(&self) {
        self.local.input_stopped();
    }

    fn stop(&self) {
        self.local.stop();
    }

    fn publish(&self, event: RuntimeEvent) {
        self.local.publish(event);
    }
}

fn timestamp() -> String {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
        |_| "unknown".to_owned(),
        |duration| duration.as_millis().to_string(),
    )
}

fn error_class(error: &ModelClientError) -> String {
    match error {
        ModelClientError::Unavailable(_) => "unavailable",
        ModelClientError::Unauthorized => "unauthorized",
        ModelClientError::NotFound(_) => "not_found",
        ModelClientError::Rejected(_) => "rejected",
        ModelClientError::Timeout => "timeout",
        ModelClientError::Cancelled => "cancelled",
        ModelClientError::MalformedResponse(_) => "malformed_response",
        ModelClientError::Transport(_) => "transport",
    }
    .to_owned()
}

fn statistics_from_metrics(
    metrics: &oid_model_client::RemoteGenerationMetrics,
) -> GenerationStatistics {
    GenerationStatistics {
        prompt_tokens: metrics.prompt_tokens.unwrap_or_default(),
        generated_tokens: metrics.generated_tokens.unwrap_or_default(),
        context_tokens: metrics.context_tokens.unwrap_or_default(),
        tokens_per_second: metrics.tokens_per_second.unwrap_or_default(),
        latency_ms: metrics.latency_ms.unwrap_or_default(),
        inference_time_ms: metrics.inference_time_ms.unwrap_or_default(),
        metrics_known: metrics.prompt_tokens.is_some()
            && metrics.generated_tokens.is_some()
            && metrics.context_tokens.is_some()
            && metrics.tokens_per_second.is_some()
            && metrics.latency_ms.is_some()
            && metrics.inference_time_ms.is_some(),
    }
}

fn render_metrics(metrics: &oid_model_client::RemoteGenerationMetrics) -> String {
    format!(
        "prompt_tokens={};generated_tokens={};context_tokens={};tokens_per_second={};latency_ms={};inference_time_ms={}",
        option_display(metrics.prompt_tokens),
        option_display(metrics.generated_tokens),
        option_display(metrics.context_tokens),
        metrics.tokens_per_second.map_or_else(|| "unknown".to_owned(), |value| value.to_string()),
        option_display(metrics.latency_ms),
        option_display(metrics.inference_time_ms),
    )
}

fn option_display<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(|| "unknown".to_owned(), |value| value.to_string())
}
