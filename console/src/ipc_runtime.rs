//! OID runtime adapter for Talvik's local IPC service.

use crate::ipc;
use oid_runtime::{
    backends::{
        BackendDescriptor, BackendHealth, BackendSummary, EmbeddingsRequest, EmbeddingsResult,
    },
    generation::ExecutionPlacement,
    GenerationMessage, GenerationRequest, GenerationResult, GenerationStatistics, GenerationStream,
    GenerationTermination, HardwareService, HardwareSnapshot, ModelMetadata, ModelStatus,
    ResidentModelSnapshot, RuntimeError, RuntimeService, RuntimeSnapshot,
};
use oid_shared::{EventBus, LifecycleState, RuntimeEvent, RuntimeStatus};
use std::collections::HashSet;
use std::io::{self, BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MODEL_PULL_TIMEOUT: Duration = Duration::from_secs(600);
const GENERATION_READ_POLL: Duration = Duration::from_millis(100);

/// Runtime view that reads model state and administers models through Talvik IPC.
pub struct IpcRuntimeService {
    bus: EventBus,
    hardware: HardwareSnapshot,
    unavailable_reported: AtomicBool,
    active_generations: Arc<Mutex<HashSet<String>>>,
}

impl std::fmt::Debug for IpcRuntimeService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IpcRuntimeService")
            .finish_non_exhaustive()
    }
}

impl IpcRuntimeService {
    /// Verify that the daemon supports this local IPC protocol version.
    pub fn probe() -> io::Result<()> {
        let response = Self::request("hello", false, REQUEST_TIMEOUT)?;
        let expected = format!("OK protocol\t{}", ipc::PROTOCOL_VERSION);
        if response.first().map(String::as_str) == Some(expected.as_str()) {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                response
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "empty Talvik protocol handshake".to_owned()),
            ))
        }
    }

    /// Create an IPC-only client for the local Talvik daemon.
    pub fn new(bus: EventBus) -> Self {
        let hardware = HardwareService::detect(&bus).snapshot();
        Self {
            bus,
            hardware,
            unavailable_reported: AtomicBool::new(false),
            active_generations: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    fn request(request: &str, multiline: bool, timeout: Duration) -> io::Result<Vec<String>> {
        let mut stream = ipc::connect()?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        writeln!(stream, "{request}")?;
        stream.flush()?;

        let mut reader = BufReader::new(stream);
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            let line = line.trim_end_matches(['\r', '\n']).to_owned();
            if line == "END" {
                break;
            }
            lines.push(line);
            if !multiline {
                break;
            }
        }
        Ok(lines)
    }

    fn status_fields() -> io::Result<Vec<String>> {
        let lines = Self::request("status", false, REQUEST_TIMEOUT)?;
        let line = lines
            .first()
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "empty Talvik status"))?;
        parse_status_response(line)
    }

    fn remote_snapshot() -> io::Result<RuntimeSnapshot> {
        let fields = Self::status_fields()?;
        let models = Self::remote_models()?;
        parse_runtime_snapshot(&fields, &models)
    }

    fn report_unavailable(&self, error: &io::Error) {
        if !self.unavailable_reported.swap(true, Ordering::Relaxed) {
            eprintln!("OID could not reach Talvik over local IPC: {error}");
        }
    }

    fn remote_models() -> io::Result<Vec<ModelMetadata>> {
        let lines = Self::request("model_list", true, REQUEST_TIMEOUT)?;
        lines
            .iter()
            .map(|line| {
                let fields = line.split('\t').collect::<Vec<_>>();
                if fields.len() != 4 || fields[0] != "MODEL" {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid Talvik model-list response",
                    ));
                }
                Ok(ModelMetadata {
                    id: ipc::decode_field(fields[1]),
                    name: ipc::decode_field(fields[2]),
                    family: "unknown".to_owned(),
                    backend: "talvikd".to_owned(),
                    quantization: "unknown".to_owned(),
                    context_window: 0,
                    memory_requirement_mb: 0,
                    capabilities: Vec::new(),
                    status: parse_model_status(fields[3]),
                    checksum: None,
                    location: "talvikd".to_owned(),
                })
            })
            .collect()
    }

    fn mutation(request: &str, timeout: Duration) -> Result<(), RuntimeError> {
        let lines = Self::request(request, false, timeout)
            .map_err(|error| RuntimeError::ModelLifecycle(error.to_string()))?;
        let response = lines.first().map(String::as_str).unwrap_or_default();
        if response == "OK" || response.starts_with("OK\t") {
            Ok(())
        } else {
            Err(RuntimeError::ModelLifecycle(
                response.strip_prefix("ERROR\t").map_or_else(
                    || format!("unexpected Talvik response: {response}"),
                    ipc::decode_field,
                ),
            ))
        }
    }

    fn cancel_remote(request_id: &str) -> io::Result<()> {
        let response = Self::request(
            &format!("cancel\t{}", ipc::encode_field(request_id)),
            false,
            REQUEST_TIMEOUT,
        )?;
        if response.first().map(String::as_str) == Some("OK cancelled") {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                response
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "empty Talvik cancellation response".to_owned()),
            ))
        }
    }

    fn remove_active_generation(active: &Mutex<HashSet<String>>, request_id: &str) {
        match active.lock() {
            Ok(mut active) => {
                active.remove(request_id);
            }
            Err(error) => eprintln!("Talvik IPC generation registry is unavailable: {error}"),
        }
    }
}

impl RuntimeService for IpcRuntimeService {
    fn status(&self) -> RuntimeStatus {
        let state = match Self::status_fields() {
            Ok(_) => LifecycleState::Ready,
            Err(error) => {
                self.report_unavailable(&error);
                LifecycleState::Stopped
            }
        };
        RuntimeStatus {
            state,
            version: "0.1",
        }
    }

    fn snapshot(&self) -> RuntimeSnapshot {
        match Self::remote_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.report_unavailable(&error);
                unavailable_snapshot()
            }
        }
    }

    fn backend_list(&self) -> Vec<BackendSummary> {
        match Self::status_fields() {
            Ok(fields) if fields[2] != "None" => vec![BackendSummary {
                descriptor: BackendDescriptor {
                    id: fields[2].clone(),
                    name: fields[2].clone(),
                    version: format!("Talvik IPC {}", ipc::PROTOCOL_VERSION),
                    library_version: nonempty(fields[18].clone()),
                },
                enabled: true,
                active: true,
            }],
            Ok(_) => Vec::new(),
            Err(error) => {
                self.report_unavailable(&error);
                Vec::new()
            }
        }
    }

    fn backend_health(&self, id: &str) -> Option<BackendHealth> {
        let fields = match Self::status_fields() {
            Ok(fields) if id == fields[2] || id == format!("talvikd ({})", fields[2]) => fields,
            Ok(_) => return None,
            Err(error) => {
                self.report_unavailable(&error);
                return Some(BackendHealth::Unavailable);
            }
        };
        Some(match health_label(&fields[1]) {
            "Healthy" => BackendHealth::Healthy,
            "Unavailable" => BackendHealth::Unavailable,
            _ => BackendHealth::Error(format!("Talvik health is {}", fields[1])),
        })
    }

    fn model_list(&self) -> Vec<ModelMetadata> {
        match Self::remote_models() {
            Ok(models) => models,
            Err(error) => {
                self.report_unavailable(&error);
                Vec::new()
            }
        }
    }

    fn model_inspect(&self, id: &str) -> Option<ModelMetadata> {
        self.model_list().into_iter().find(|model| model.id == id)
    }

    fn model_pull(
        &self,
        source: &str,
        model_id: Option<&str>,
        checksum: Option<&str>,
    ) -> Result<ModelMetadata, RuntimeError> {
        let mut request = format!("model_pull\t{}", ipc::encode_field(source));
        if model_id.is_some() || checksum.is_some() {
            request.push('\t');
            request.push_str(&model_id.map(ipc::encode_field).unwrap_or_default());
        }
        if let Some(checksum) = checksum {
            request.push('\t');
            request.push_str(&ipc::encode_field(checksum));
        }
        let lines = Self::request(&request, false, MODEL_PULL_TIMEOUT)
            .map_err(|error| RuntimeError::ModelLifecycle(error.to_string()))?;
        let response = lines.first().map(String::as_str).unwrap_or_default();
        let fields = response.split('\t').collect::<Vec<_>>();
        if fields.len() != 7 || fields[0] != "OK" || fields[1] != "model_pull" {
            return Err(RuntimeError::ModelLifecycle(
                fields
                    .get(1)
                    .filter(|_| fields.first() == Some(&"ERROR"))
                    .map_or_else(
                        || format!("unexpected Talvik response: {response}"),
                        |value| ipc::decode_field(value),
                    ),
            ));
        }
        Ok(ModelMetadata {
            id: ipc::decode_field(fields[2]),
            name: ipc::decode_field(fields[3]),
            family: "unknown".to_owned(),
            backend: "talvikd".to_owned(),
            quantization: "unknown".to_owned(),
            context_window: 0,
            memory_requirement_mb: 0,
            capabilities: Vec::new(),
            status: parse_model_status(fields[4]),
            checksum: nonempty(ipc::decode_field(fields[6])),
            location: ipc::decode_field(fields[5]),
        })
    }

    fn model_load(&self, id: &str) -> Result<(), RuntimeError> {
        Self::mutation(
            &format!("model_load\t{}", ipc::encode_field(id)),
            REQUEST_TIMEOUT,
        )
    }

    fn model_unload(&self, id: &str) -> Result<(), RuntimeError> {
        Self::mutation(
            &format!("model_unload\t{}", ipc::encode_field(id)),
            REQUEST_TIMEOUT,
        )
    }

    fn tokenize(&self, text: &str) -> Result<usize, RuntimeError> {
        let request = format!("tokenize\t{}", ipc::encode_field(text));
        let lines = Self::request(&request, false, REQUEST_TIMEOUT)
            .map_err(|error| RuntimeError::Tokenization(error.to_string()))?;
        let response = lines.first().map(String::as_str).unwrap_or_default();
        let fields = response.split('\t').collect::<Vec<_>>();
        if fields.len() == 2 && fields[0] == "OK tokenize" {
            fields[1]
                .parse()
                .map_err(|_| RuntimeError::Tokenization("invalid Talvik token count".to_owned()))
        } else {
            Err(RuntimeError::Tokenization(
                fields
                    .get(1)
                    .filter(|_| fields.first() == Some(&"ERROR"))
                    .map_or_else(
                        || format!("unexpected Talvik tokenize response: {response}"),
                        |value| ipc::decode_field(value),
                    ),
            ))
        }
    }

    fn embeddings(&self, _request: &EmbeddingsRequest) -> Result<EmbeddingsResult, RuntimeError> {
        Err(RuntimeError::NotImplemented("Talvik IPC embeddings"))
    }

    fn tool_support(&self, _model_id: &str) -> oid_runtime::backends::ToolSupport {
        oid_runtime::backends::ToolSupport {
            supported: false,
            reason: "tool support is not reported by the Talvik IPC protocol".to_owned(),
        }
    }

    fn generate(&self, request: GenerationRequest) -> Result<GenerationStream, RuntimeError> {
        let request_id = request.request_id.clone();
        {
            let mut active = self.active_generations.lock().map_err(|error| {
                RuntimeError::ModelLifecycle(format!(
                    "Talvik IPC generation registry is unavailable: {error}"
                ))
            })?;
            if !active.insert(request_id.clone()) {
                return Err(RuntimeError::ModelLifecycle(format!(
                    "duplicate active generation request id: {request_id}"
                )));
            }
        }

        let mut stream = match ipc::connect() {
            Ok(stream) => stream,
            Err(error) => {
                Self::remove_active_generation(&self.active_generations, &request_id);
                return Err(RuntimeError::ModelLifecycle(format!(
                    "could not connect to Talvik IPC: {error}"
                )));
            }
        };
        if let Err(error) = stream
            .set_read_timeout(Some(GENERATION_READ_POLL))
            .and_then(|()| stream.set_write_timeout(Some(REQUEST_TIMEOUT)))
            .and_then(|()| {
                writeln!(stream, "{}", generation_request_line(&request))?;
                stream.flush()
            })
        {
            Self::remove_active_generation(&self.active_generations, &request_id);
            return Err(RuntimeError::ModelLifecycle(format!(
                "could not submit generation over Talvik IPC: {error}"
            )));
        }

        let cancellation = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        let active_generations = Arc::clone(&self.active_generations);
        let worker_request = request.clone();
        let worker_cancellation = Arc::clone(&cancellation);
        thread::Builder::new()
            .name(format!("talvik-ipc-{request_id}"))
            .spawn(move || {
                receive_generation(
                    stream,
                    worker_request,
                    sender,
                    worker_cancellation,
                    active_generations,
                );
            })
            .map_err(|error| {
                Self::remove_active_generation(&self.active_generations, &request_id);
                RuntimeError::ModelLifecycle(format!(
                    "could not start Talvik IPC generation stream: {error}"
                ))
            })?;

        Ok(GenerationStream::from_parts(receiver, cancellation))
    }

    fn cancel_generation(&self) {
        let request_ids = match self.active_generations.lock() {
            Ok(active) => active.iter().cloned().collect::<Vec<_>>(),
            Err(error) => {
                eprintln!("Talvik IPC generation registry is unavailable: {error}");
                return;
            }
        };
        for request_id in request_ids {
            if let Err(error) = Self::cancel_remote(&request_id) {
                eprintln!("Could not cancel Talvik IPC request {request_id}: {error}");
            }
        }
    }

    fn cancel_generation_for(&self, request_id: &str) {
        if let Err(error) = Self::cancel_remote(request_id) {
            eprintln!("Could not cancel Talvik IPC request {request_id}: {error}");
        }
    }

    fn hardware(&self) -> HardwareSnapshot {
        self.hardware.clone()
    }

    fn event_bus(&self) -> EventBus {
        self.bus.clone()
    }

    fn execute_command(&self, command: &str) -> String {
        self.bus
            .publish(&RuntimeEvent::CommandStarted(command.to_owned()));
        self.bus
            .publish(&RuntimeEvent::CommandCompleted(command.to_owned()));
        format!("Runtime command execution is unavailable over Talvik IPC: {command}")
    }

    fn input_started(&self) {
        self.bus.publish(&RuntimeEvent::InputStarted);
    }

    fn input_stopped(&self) {
        self.bus.publish(&RuntimeEvent::InputStopped);
    }

    fn stop(&self) {
        self.cancel_generation();
        self.bus.publish(&RuntimeEvent::RuntimeStopped);
    }

    fn publish(&self, event: RuntimeEvent) {
        self.bus.publish(&event);
    }
}

fn generation_request_line(request: &GenerationRequest) -> String {
    let stop_sequences = serde_json::to_string(&request.options.stop_sequences)
        .expect("serializing a list of strings cannot fail");
    format!(
        "generate_v2\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        ipc::encode_field(&request.model_id),
        ipc::encode_field(&request.request_id),
        request.options.max_tokens,
        request.options.temperature,
        request.options.top_p,
        request.options.top_k,
        request.options.seed,
        request.options.context_size,
        ipc::encode_field(&stop_sequences),
        ipc::encode_field(&request.prompt),
    )
}

#[allow(clippy::needless_pass_by_value)]
fn receive_generation(
    stream: ipc::TalvikStream,
    request: GenerationRequest,
    sender: mpsc::Sender<GenerationMessage>,
    cancellation: Arc<AtomicBool>,
    active_generations: Arc<Mutex<HashSet<String>>>,
) {
    let mut reader = BufReader::new(stream);
    let mut text = String::new();
    let mut line = String::new();
    let mut cancel_sent = false;
    loop {
        if cancellation.load(Ordering::SeqCst) && !cancel_sent {
            if let Err(error) = IpcRuntimeService::cancel_remote(&request.request_id) {
                let _ = sender.send(GenerationMessage::Failed(RuntimeError::ModelLifecycle(
                    format!("could not cancel Talvik IPC generation: {error}"),
                )));
                break;
            }
            cancel_sent = true;
        }

        match reader.read_line(&mut line) {
            Ok(0) => {
                if line.is_empty() {
                    let _ = sender.send(GenerationMessage::Failed(RuntimeError::ModelLifecycle(
                        "Talvik IPC generation stream ended before a terminal response".to_owned(),
                    )));
                } else {
                    let _ = sender.send(GenerationMessage::Failed(RuntimeError::ModelLifecycle(
                        "Talvik IPC closed with an incomplete generation response".to_owned(),
                    )));
                }
                break;
            }
            Ok(_) => {
                let response = std::mem::take(&mut line);
                let response = response.trim_end_matches(['\r', '\n']);
                if let Some(token) = response.strip_prefix("TOKEN\t") {
                    let token = ipc::decode_field(token);
                    text.push_str(&token);
                    if sender.send(GenerationMessage::Token(token)).is_err() {
                        if let Err(error) = IpcRuntimeService::cancel_remote(&request.request_id) {
                            eprintln!(
                                "Could not cancel abandoned Talvik IPC request {}: {error}",
                                request.request_id
                            );
                        }
                        break;
                    }
                } else if response.starts_with("DONE\t") {
                    match parse_generation_terminal(response, &request, text) {
                        Ok(message) => {
                            let _ = sender.send(message);
                        }
                        Err(error) => {
                            let _ = sender.send(GenerationMessage::Failed(
                                RuntimeError::ModelLifecycle(error.to_string()),
                            ));
                        }
                    }
                    break;
                } else if let Some(error) = response.strip_prefix("ERROR\t") {
                    let _ = sender.send(GenerationMessage::Failed(RuntimeError::ModelLifecycle(
                        ipc::decode_field(error),
                    )));
                    break;
                } else {
                    let _ = sender.send(GenerationMessage::Failed(RuntimeError::ModelLifecycle(
                        format!("invalid Talvik IPC generation response: {response}"),
                    )));
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) => {}
            Err(error) => {
                let _ = sender.send(GenerationMessage::Failed(RuntimeError::ModelLifecycle(
                    format!("failed reading Talvik IPC generation: {error}"),
                )));
                break;
            }
        }
    }
    IpcRuntimeService::remove_active_generation(&active_generations, &request.request_id);
}

fn parse_generation_terminal(
    line: &str,
    request: &GenerationRequest,
    text: String,
) -> io::Result<GenerationMessage> {
    let fields = line.split('\t').collect::<Vec<_>>();
    if fields.len() == 3 {
        let generated_tokens = parse_required(fields[2], "generated token count")?;
        let result = GenerationResult {
            request_id: request.request_id.clone(),
            generation_id: request.request_id.clone(),
            text,
            statistics: GenerationStatistics {
                generated_tokens,
                ..GenerationStatistics::default()
            },
        };
        return match fields[1] {
            "completed" => Ok(GenerationMessage::Completed(result)),
            "cancelled" => Ok(GenerationMessage::Cancelled(result)),
            _ => Err(invalid_response("unknown generation terminal state")),
        };
    }
    if fields.len() != 22 {
        return Err(invalid_response(
            "invalid Talvik IPC generation terminal response",
        ));
    }

    let statistics = GenerationStatistics {
        generated_tokens: parse_required(fields[3], "generated token count")?,
        prompt_tokens: parse_required(fields[4], "prompt token count")?,
        context_tokens: parse_required(fields[5], "context token count")?,
        latency_ms: parse_required(fields[6], "latency")?,
        inference_time_ms: parse_required(fields[7], "inference duration")?,
        backend_inference_time_ms: parse_optional(fields[8], "backend inference duration")?,
        time_to_first_token_ms: parse_optional(fields[9], "time to first token")?,
        prompt_evaluation_time_ms: parse_optional(fields[10], "prompt evaluation duration")?,
        generation_time_ms: parse_optional(fields[11], "generation duration")?,
        tokens_per_second: parse_required(fields[12], "generation throughput")?,
        metrics_known: parse_bool(fields[13])?,
        termination: parse_termination(fields[14])?,
        cache_hit: parse_bool(fields[15])?,
        reused_prompt_tokens: parse_required(fields[16], "reused prompt token count")?,
        execution: parse_execution(&fields[17..22])?,
    };
    let result = GenerationResult {
        request_id: request.request_id.clone(),
        generation_id: ipc::decode_field(fields[2]),
        text,
        statistics,
    };
    match fields[1] {
        "completed" => Ok(GenerationMessage::Completed(result)),
        "cancelled" => Ok(GenerationMessage::Cancelled(result)),
        _ => Err(invalid_response("unknown generation terminal state")),
    }
}

fn parse_execution(fields: &[&str]) -> io::Result<Option<ExecutionPlacement>> {
    let backend = nonempty(ipc::decode_field(fields[0]));
    let device = nonempty(ipc::decode_field(fields[1]));
    let requested_offloaded_layers = parse_optional(fields[2], "requested offloaded layers")?;
    let offloaded_layers = parse_optional(fields[3], "offloaded layers")?;
    let fallback_reason = nonempty(ipc::decode_field(fields[4]));
    if backend.is_none()
        && device.is_none()
        && requested_offloaded_layers.is_none()
        && offloaded_layers.is_none()
        && fallback_reason.is_none()
    {
        return Ok(None);
    }
    Ok(Some(ExecutionPlacement {
        backend,
        device,
        requested_offloaded_layers,
        offloaded_layers,
        fallback_reason,
    }))
}

fn parse_termination(value: &str) -> io::Result<GenerationTermination> {
    match value {
        "stop" => Ok(GenerationTermination::Stop),
        "length" => Ok(GenerationTermination::Length),
        "context_length" => Ok(GenerationTermination::ContextLength),
        _ => Err(invalid_response("invalid generation termination reason")),
    }
}

fn parse_bool(value: &str) -> io::Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(invalid_response("invalid boolean in generation response")),
    }
}

fn parse_required<T: std::str::FromStr>(value: &str, name: &str) -> io::Result<T> {
    value
        .parse()
        .map_err(|_| invalid_response(&format!("invalid {name} in generation response")))
}

fn parse_optional<T: std::str::FromStr>(value: &str, name: &str) -> io::Result<Option<T>> {
    if value.is_empty() {
        Ok(None)
    } else {
        parse_required(value, name).map(Some)
    }
}

fn invalid_response(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn parse_status_response(line: &str) -> io::Result<Vec<String>> {
    let fields = line
        .trim_end_matches(['\r', '\n'])
        .split('\t')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if fields.first().map(String::as_str) != Some("OK status") || fields.len() < 23 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Talvik status response",
        ));
    }
    Ok(fields)
}

fn parse_runtime_snapshot(
    fields: &[String],
    models: &[ModelMetadata],
) -> io::Result<RuntimeSnapshot> {
    let loaded_models = fields[4]
        .split(',')
        .filter(|model| !model.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let resident_models = models
        .iter()
        .filter(|model| model.status == ModelStatus::Loaded)
        .map(|model| ResidentModelSnapshot {
            id: model.id.clone(),
            memory_bytes: None,
            idle_seconds: 0,
        })
        .collect();
    Ok(RuntimeSnapshot {
        health: health_label(&fields[1]),
        backend: format!("talvikd ({})", fields[2]),
        models: models.len(),
        memory: fields[11].clone(),
        loaded_model: nonempty(fields[3].clone()),
        loaded_models,
        resident_models,
        pending_requests: parse_status_value(&fields[12], "pending request count")?,
        active_requests: parse_status_value(&fields[13], "active request count")?,
        queue_capacity: parse_status_value(&fields[14], "queue capacity")?,
        automatic_eviction: parse_bool(&fields[15])?,
        idle_retention_seconds: parse_status_value(&fields[16], "idle retention")?,
        uptime_seconds: parse_status_value(&fields[17], "uptime")?,
        backend_version: nonempty(fields[18].clone()),
        model_memory_bytes: parse_optional(&fields[19], "model memory")?,
        generating: parse_bool(&fields[20])?,
        current_tokens_per_second: parse_optional(&fields[21], "current throughput")?,
        loaded_context: parse_optional(&fields[22], "loaded context")?,
    })
}

fn unavailable_snapshot() -> RuntimeSnapshot {
    RuntimeSnapshot {
        health: "Unavailable",
        backend: "talvikd (unavailable)".to_owned(),
        models: 0,
        memory: "unavailable".to_owned(),
        loaded_model: None,
        loaded_models: Vec::new(),
        resident_models: Vec::new(),
        pending_requests: 0,
        active_requests: 0,
        queue_capacity: 0,
        automatic_eviction: false,
        idle_retention_seconds: 0,
        uptime_seconds: 0,
        backend_version: None,
        model_memory_bytes: None,
        generating: false,
        current_tokens_per_second: None,
        loaded_context: None,
    }
}

fn parse_status_value<T: std::str::FromStr>(value: &str, name: &str) -> io::Result<T> {
    value.parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid {name} in Talvik status response"),
        )
    })
}

fn parse_model_status(value: &str) -> ModelStatus {
    match value {
        "Loaded" => ModelStatus::Loaded,
        "Loading" => ModelStatus::Loading,
        "Unloading" => ModelStatus::Unloading,
        "Failed" => ModelStatus::Failed,
        "Unavailable" => ModelStatus::Unavailable,
        _ => ModelStatus::Registered,
    }
}

fn health_label(value: &str) -> &'static str {
    if value.eq_ignore_ascii_case("healthy") {
        "Healthy"
    } else if value.eq_ignore_ascii_case("unavailable") {
        "Unavailable"
    } else {
        "Degraded"
    }
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::{
        generation_request_line, parse_generation_terminal, parse_status_response,
        receive_generation,
    };
    use oid_runtime::{GenerationOptions, GenerationRequest, GenerationStatistics};
    use std::collections::HashSet;
    use std::io::Write;
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;

    #[test]
    fn status_parser_preserves_empty_loaded_model_fields() {
        let fields = parse_status_response(
            "OK status\tHealthy\tllama.cpp\t\t\t0.1\tsha\tfalse\ttarget\tdebug\tfalse\t--\t0\t0\t0\tfalse\t0\t0\t\t\tfalse\t\t\r\n",
        )
            .expect("valid status response");

        assert_eq!(&fields[..5], ["OK status", "Healthy", "llama.cpp", "", ""]);
    }

    #[test]
    fn snapshot_parser_uses_remote_runtime_metrics() {
        let fields = parse_status_response(
            "OK status\tHealthy\tllama.cpp\tqwen\tqwen\t0.1\tsha\tfalse\ttarget\trelease\tfalse\t512 MB\t1\t1\t8\ttrue\t300\t100\tllama.cpp 1.0\t4000000\ttrue\t12.5\t2048",
        )
        .expect("valid Talvik status response");
        let snapshot = super::parse_runtime_snapshot(
            &fields,
            &[oid_runtime::ModelMetadata {
                id: "qwen".to_owned(),
                name: "Qwen".to_owned(),
                family: "unknown".to_owned(),
                backend: "talvikd".to_owned(),
                quantization: "unknown".to_owned(),
                context_window: 0,
                memory_requirement_mb: 0,
                capabilities: Vec::new(),
                status: oid_runtime::ModelStatus::Loaded,
                checksum: None,
                location: "talvikd".to_owned(),
            }],
        )
        .expect("parse remote snapshot");

        assert_eq!(snapshot.backend, "talvikd (llama.cpp)");
        assert_eq!(snapshot.loaded_model.as_deref(), Some("qwen"));
        assert_eq!(snapshot.models, 1);
        assert_eq!(snapshot.pending_requests, 1);
        assert_eq!(snapshot.active_requests, 1);
        assert_eq!(snapshot.queue_capacity, 8);
        assert!(snapshot.automatic_eviction);
        assert_eq!(snapshot.uptime_seconds, 100);
        assert_eq!(snapshot.model_memory_bytes, Some(4_000_000));
        assert!(snapshot.generating);
        assert_eq!(snapshot.current_tokens_per_second, Some(12.5));
        assert_eq!(snapshot.loaded_context, Some(2048));
    }

    #[test]
    fn generation_request_preserves_options_and_prompt_text() {
        let request = GenerationRequest {
            request_id: "request-1".to_owned(),
            model_id: "qwen".to_owned(),
            prompt: "first line\nsecond\tline".to_owned(),
            options: GenerationOptions {
                max_tokens: 32,
                temperature: 0.4,
                top_p: 0.8,
                top_k: 12,
                seed: 7,
                context_size: 2048,
                stop_sequences: vec!["END".to_owned()],
            },
        };
        let line = generation_request_line(&request);
        let fields = line.split('\t').collect::<Vec<_>>();

        assert_eq!(fields[0], "generate_v2");
        assert_eq!(fields[3], "32");
        assert_eq!(fields[4], "0.4");
        assert_eq!(fields[5], "0.8");
        assert_eq!(fields[6], "12");
        assert_eq!(fields[7], "7");
        assert_eq!(fields[8], "2048");
        assert_eq!(crate::ipc::decode_field(fields[9]), r#"["END"]"#);
        assert_eq!(
            crate::ipc::decode_field(&fields[10..].join("\t")),
            request.prompt
        );
    }

    #[test]
    fn generation_terminal_parser_preserves_metrics_and_generation_identity() {
        let request = GenerationRequest {
            request_id: "request-1".to_owned(),
            model_id: "qwen".to_owned(),
            prompt: "hello".to_owned(),
            options: GenerationOptions::default(),
        };
        let line = "DONE\tcompleted\tgeneration-1\t5\t3\t8\t1200\t1100\t1000\t200\t300\t800\t4.5\ttrue\tlength\tfalse\t0\tcpu\tCPU\t0\t0\t";
        let message = parse_generation_terminal(line, &request, "world".to_owned())
            .expect("valid terminal response");

        let oid_runtime::GenerationMessage::Completed(result) = message else {
            panic!("expected completion");
        };
        assert_eq!(result.request_id, "request-1");
        assert_eq!(result.generation_id, "generation-1");
        assert_eq!(result.text, "world");
        assert_eq!(result.statistics.generated_tokens, 5);
        assert_eq!(result.statistics.prompt_tokens, 3);
        assert_eq!(result.statistics.context_tokens, 8);
        assert_eq!(result.statistics.latency_ms, 1200);
        assert_eq!(result.statistics.tokens_per_second, 4.5);
        assert!(result.statistics.metrics_known);
        assert_eq!(
            result
                .statistics
                .execution
                .as_ref()
                .unwrap()
                .backend
                .as_deref(),
            Some("cpu")
        );
    }

    #[test]
    fn legacy_generation_terminal_keeps_available_token_count_unknown_metrics() {
        let request = GenerationRequest {
            request_id: "request-1".to_owned(),
            model_id: "qwen".to_owned(),
            prompt: "hello".to_owned(),
            options: GenerationOptions::default(),
        };
        let message = parse_generation_terminal("DONE\tcompleted\t4", &request, "world".to_owned())
            .expect("legacy response remains supported");

        let oid_runtime::GenerationMessage::Completed(result) = message else {
            panic!("expected completion");
        };
        assert_eq!(result.statistics.generated_tokens, 4);
        assert!(!result.statistics.metrics_known);
        assert_eq!(
            result.statistics,
            GenerationStatistics {
                generated_tokens: 4,
                ..GenerationStatistics::default()
            }
        );
    }

    #[test]
    fn generation_reader_streams_tokens_and_cleans_up_request_state() {
        let (client_stream, mut server_stream) = test_stream_pair();
        server_stream
            .write_all(
                b"TOKEN\tMock\\nresponse\nDONE\tcompleted\tgeneration-1\t2\t1\t3\t20\t18\t17\t2\t3\t15\t100.0\ttrue\tstop\tfalse\t0\tcpu\tCPU\t0\t0\t\n",
            )
            .expect("write mock generation");
        server_stream.flush().expect("flush mock generation");
        let request = GenerationRequest {
            request_id: "request-1".to_owned(),
            model_id: "qwen".to_owned(),
            prompt: "hello".to_owned(),
            options: GenerationOptions::default(),
        };
        let active_generations = Arc::new(Mutex::new(HashSet::from([request.request_id.clone()])));
        let (sender, receiver) = mpsc::channel();
        let worker_active = Arc::clone(&active_generations);
        let worker = thread::spawn(move || {
            receive_generation(
                client_stream,
                request,
                sender,
                Arc::new(std::sync::atomic::AtomicBool::new(false)),
                worker_active,
            );
        });

        assert_eq!(
            receiver.recv().expect("token message"),
            oid_runtime::GenerationMessage::Token("Mock\nresponse".to_owned())
        );
        let message = receiver.recv().expect("completion message");
        let oid_runtime::GenerationMessage::Completed(result) = message else {
            panic!("expected completion");
        };
        assert_eq!(result.text, "Mock\nresponse");
        assert_eq!(result.generation_id, "generation-1");
        worker.join().expect("generation reader exits");
        assert!(active_generations.lock().expect("active lock").is_empty());
    }

    #[cfg(unix)]
    fn test_stream_pair() -> (crate::ipc::TalvikStream, crate::ipc::TalvikStream) {
        std::os::unix::net::UnixStream::pair().expect("create Unix stream pair")
    }

    #[cfg(windows)]
    fn test_stream_pair() -> (crate::ipc::TalvikStream, crate::ipc::TalvikStream) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind TCP listener");
        let address = listener.local_addr().expect("read listener address");
        let client = std::net::TcpStream::connect(address).expect("connect TCP client");
        let (server, _) = listener.accept().expect("accept TCP client");
        (client, server)
    }
}
