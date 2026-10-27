//! OID runtime adapter for Talvik's local IPC service.

use crate::ipc;
use oid_runtime::{
    backends::{BackendHealth, BackendSummary, EmbeddingsRequest, EmbeddingsResult},
    GenerationRequest, GenerationStream, HardwareSnapshot, ModelMetadata, ModelStatus,
    RuntimeError, RuntimeService, RuntimeSnapshot,
};
use oid_shared::{EventBus, LifecycleState, RuntimeEvent, RuntimeStatus};
use std::io::{self, BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MODEL_PULL_TIMEOUT: Duration = Duration::from_secs(600);

/// Runtime view that reads model state and administers models through Talvik IPC.
pub struct IpcRuntimeService {
    local: Box<dyn RuntimeService>,
    unavailable_reported: AtomicBool,
}

impl std::fmt::Debug for IpcRuntimeService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IpcRuntimeService")
            .finish_non_exhaustive()
    }
}

impl IpcRuntimeService {
    /// Verify that Talvik IPC is reachable.
    pub fn probe() -> io::Result<()> {
        Self::request("status", false, REQUEST_TIMEOUT)?;
        Ok(())
    }

    /// Wrap the in-process runtime with Talvik IPC access.
    pub fn new(local: Box<dyn RuntimeService>) -> Self {
        Self {
            local,
            unavailable_reported: AtomicBool::new(false),
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

    fn status_fields(&self) -> io::Result<Vec<String>> {
        let lines = Self::request("status", false, REQUEST_TIMEOUT)?;
        let line = lines
            .first()
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "empty Talvik status"))?;
        parse_status_response(line)
    }

    fn report_unavailable(&self, error: &io::Error) {
        if !self.unavailable_reported.swap(true, Ordering::Relaxed) {
            eprintln!("OID could not reach Talvik over local IPC: {error}");
        }
    }

    fn remote_models(&self) -> io::Result<Vec<ModelMetadata>> {
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

    fn mutation(&self, request: &str, timeout: Duration) -> Result<(), RuntimeError> {
        let lines = Self::request(request, false, timeout)
            .map_err(|error| RuntimeError::ModelLifecycle(error.to_string()))?;
        let response = lines.first().map(String::as_str).unwrap_or_default();
        if response == "OK" || response.starts_with("OK\t") {
            Ok(())
        } else {
            Err(RuntimeError::ModelLifecycle(
                response
                    .strip_prefix("ERROR\t")
                    .map(ipc::decode_field)
                    .unwrap_or_else(|| format!("unexpected Talvik response: {response}")),
            ))
        }
    }
}

impl RuntimeService for IpcRuntimeService {
    fn status(&self) -> RuntimeStatus {
        let mut status = self.local.status();
        status.state = if self.status_fields().is_ok() {
            LifecycleState::Ready
        } else {
            LifecycleState::Stopped
        };
        status
    }

    fn snapshot(&self) -> RuntimeSnapshot {
        let mut snapshot = self.local.snapshot();
        match self.status_fields() {
            Ok(fields) => {
                snapshot.health = health_label(&fields[1]);
                snapshot.backend = format!("talvikd ({})", fields[2]);
                snapshot.loaded_model = nonempty(fields[3].clone());
                snapshot.loaded_models = fields[4]
                    .split(',')
                    .filter(|model| !model.is_empty())
                    .map(str::to_owned)
                    .collect();
                match self.remote_models() {
                    Ok(models) => snapshot.models = models.len(),
                    Err(error) => {
                        self.report_unavailable(&error);
                        snapshot.models = 0;
                    }
                }
                snapshot.backend_version = None;
                snapshot.model_memory_bytes = None;
                snapshot.current_tokens_per_second = None;
                snapshot.loaded_context = None;
                snapshot.generating = false;
            }
            Err(error) => {
                self.report_unavailable(&error);
                snapshot.health = "Unavailable";
                snapshot.backend = "talvikd (unavailable)".to_owned();
                snapshot.models = 0;
                snapshot.loaded_model = None;
                snapshot.loaded_models.clear();
                snapshot.backend_version = None;
                snapshot.model_memory_bytes = None;
                snapshot.current_tokens_per_second = None;
                snapshot.loaded_context = None;
                snapshot.generating = false;
            }
        }
        snapshot
    }

    fn backend_list(&self) -> Vec<BackendSummary> {
        self.local.backend_list()
    }

    fn backend_health(&self, id: &str) -> Option<BackendHealth> {
        self.local.backend_health(id)
    }

    fn model_list(&self) -> Vec<ModelMetadata> {
        match self.remote_models() {
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
                    .map(|error| ipc::decode_field(error))
                    .unwrap_or_else(|| format!("unexpected Talvik response: {response}")),
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
        self.mutation(
            &format!("model_load\t{}", ipc::encode_field(id)),
            REQUEST_TIMEOUT,
        )
    }

    fn model_unload(&self, id: &str) -> Result<(), RuntimeError> {
        self.mutation(
            &format!("model_unload\t{}", ipc::encode_field(id)),
            REQUEST_TIMEOUT,
        )
    }

    fn tokenize(&self, text: &str) -> Result<usize, RuntimeError> {
        self.local.tokenize(text)
    }

    fn embeddings(&self, request: &EmbeddingsRequest) -> Result<EmbeddingsResult, RuntimeError> {
        self.local.embeddings(request)
    }

    fn tool_support(&self) -> oid_runtime::backends::ToolSupport {
        self.local.tool_support()
    }

    fn generate(&self, request: GenerationRequest) -> Result<GenerationStream, RuntimeError> {
        self.local.generate(request)
    }

    fn cancel_generation(&self) {
        self.local.cancel_generation();
    }

    fn cancel_generation_for(&self, request_id: &str) {
        self.local.cancel_generation_for(request_id);
    }

    fn active_generation_id(&self, request_id: &str) -> Option<String> {
        self.local.active_generation_id(request_id)
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

fn parse_status_response(line: &str) -> io::Result<Vec<String>> {
    let fields = line
        .trim_end_matches(['\r', '\n'])
        .split('\t')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if fields.first().map(String::as_str) != Some("OK status") || fields.len() < 5 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Talvik status response",
        ));
    }
    Ok(fields)
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
    use super::parse_status_response;

    #[test]
    fn status_parser_preserves_empty_loaded_model_fields() {
        let fields = parse_status_response("OK status\tHealthy\tllama.cpp\t\t\r\n")
            .expect("valid status response");

        assert_eq!(fields, ["OK status", "Healthy", "llama.cpp", "", ""]);
    }
}
