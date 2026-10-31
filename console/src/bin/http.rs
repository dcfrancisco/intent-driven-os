//! Native loopback HTTP API for Marina.

use base64::{engine::general_purpose::STANDARD, Engine};
use oid_runtime::{
    auth, CacheStrategy, ContentPart, GenerationMessage, GenerationOptions, GenerationRequest,
    GenerationResult, GenerationStream, GenerationTermination, ImageInput, MarinaRuntime,
    ModelProfile, ParsedText, ReasoningFormat, ReasoningStreamParser, RuntimeService,
    StructuredOutput, ToolFormat,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_REQUEST_BODY_BYTES: usize = 56 * 1_048_576;
const MAX_CONTEXT_TOKENS: u64 = 32_768;
const CANCELLATION_CLEANUP_TIMEOUT: Duration = Duration::from_secs(30);
const IMAGE_MARKER: &str = "<__media__>";

fn decode_data_image(value: &str) -> Result<ImageInput, String> {
    let value = value
        .strip_prefix("data:")
        .ok_or_else(|| "image URL must use the data: scheme".to_owned())?;
    let (metadata, encoded) = value
        .split_once(',')
        .ok_or_else(|| "image data URL is missing its payload".to_owned())?;
    let (media_type, encoding) = metadata
        .split_once(';')
        .ok_or_else(|| "image data URL must declare base64 encoding".to_owned())?;
    if encoding != "base64"
        || !matches!(
            media_type,
            "image/png" | "image/jpeg" | "image/bmp" | "image/gif"
        )
    {
        return Err("only base64 PNG, JPEG, BMP, and GIF images are supported".to_owned());
    }
    if encoded.len() > ImageInput::MAX_ENCODED_BYTES.saturating_mul(4) / 3 + 8 {
        return Err("encoded image exceeds the 10 MiB limit".to_owned());
    }
    let data = STANDARD
        .decode(encoded)
        .map_err(|error| format!("invalid base64 image data: {error}"))?;
    if data.is_empty() || data.len() > ImageInput::MAX_ENCODED_BYTES {
        return Err("decoded image must be between 1 byte and 10 MiB".to_owned());
    }
    Ok(ImageInput {
        media_type: media_type.to_owned(),
        data,
    })
}

fn decode_ollama_image(value: &str) -> Result<ImageInput, String> {
    if value.len() > ImageInput::MAX_ENCODED_BYTES.saturating_mul(4) / 3 + 8 {
        return Err("encoded image exceeds the 10 MiB limit".to_owned());
    }
    let data = STANDARD
        .decode(value)
        .map_err(|error| format!("invalid base64 image data: {error}"))?;
    if data.is_empty() || data.len() > ImageInput::MAX_ENCODED_BYTES {
        return Err("decoded image must be between 1 byte and 10 MiB".to_owned());
    }
    let media_type = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if data.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if data.starts_with(b"BM") {
        "image/bmp"
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "image/gif"
    } else {
        return Err("image data must be PNG, JPEG, BMP, or GIF".to_owned());
    };
    Ok(ImageInput {
        media_type: media_type.to_owned(),
        data,
    })
}

fn push_text_part(parts: &mut Vec<ContentPart>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(ContentPart::Text(previous)) = parts.last_mut() {
        previous.push_str(text);
    } else {
        parts.push(ContentPart::Text(text.to_owned()));
    }
}

fn push_image_part(parts: &mut Vec<ContentPart>, image: ImageInput) -> Result<(), String> {
    let image_count = parts
        .iter()
        .filter(|part| matches!(part, ContentPart::Image(_)))
        .count();
    if image_count >= ImageInput::MAX_IMAGES_PER_REQUEST {
        return Err(format!(
            "at most {} images are supported per request",
            ImageInput::MAX_IMAGES_PER_REQUEST
        ));
    }
    parts.push(ContentPart::Image(image));
    Ok(())
}

fn flatten_content(parts: &[ContentPart]) -> String {
    let mut prompt = String::new();
    for part in parts {
        match part {
            ContentPart::Text(text) => prompt.push_str(text),
            ContentPart::Image(_) => prompt.push_str(IMAGE_MARKER),
        }
    }
    prompt
}

fn prepend_text_part(parts: &mut Vec<ContentPart>, prefix: &str) {
    if prefix.is_empty() {
        return;
    }
    if let Some(ContentPart::Text(first)) = parts.first_mut() {
        first.insert_str(0, prefix);
    } else {
        parts.insert(0, ContentPart::Text(prefix.to_owned()));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenAiGenerationEndpoint {
    ChatCompletions,
    Completions,
    Responses,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum OpenAiToolChoice {
    Auto,
    None,
    Required,
    Function(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OllamaGenerationEndpoint {
    Chat,
    Generate,
}

/// Process-local bootstrap and inference readiness shared by health routes.
#[derive(Debug, Default)]
pub(crate) struct Readiness {
    ready: AtomicBool,
    state: Mutex<String>,
}

impl Readiness {
    pub(crate) fn new() -> Self {
        Self {
            ready: AtomicBool::new(false),
            state: Mutex::new("starting".to_owned()),
        }
    }

    pub(crate) fn set(&self, ready: bool, state: impl Into<String>) {
        if let Ok(mut current) = self.state.lock() {
            *current = state.into();
        }
        self.ready.store(ready, Ordering::Release);
    }

    fn snapshot(&self, runtime: &dyn RuntimeService) -> (bool, String) {
        // Bootstrap describes default-model provisioning. Inference readiness
        // follows current residency, including explicit loads and unloads.
        let ready = !runtime.snapshot().loaded_models.is_empty();
        let bootstrap = if self.ready.load(Ordering::Acquire) && !ready {
            "no_resident_model".to_owned()
        } else {
            self.state
                .lock()
                .map_or_else(|_| "unknown".to_owned(), |state| state.clone())
        };
        (ready, bootstrap)
    }
}

/// Serve the native HTTP API on an explicitly configured address.
pub fn serve(
    listener: &TcpListener,
    runtime: &Arc<MarinaRuntime>,
    authentication_enabled: bool,
    default_model_id: Option<&String>,
    readiness: &Arc<Readiness>,
) {
    let default_model_id = default_model_id.cloned();
    let owners = Arc::new(Mutex::new(HashMap::<String, String>::new()));
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let runtime = Arc::clone(runtime);
                let owners = Arc::clone(&owners);
                let default_model_id = default_model_id.clone();
                let readiness = Arc::clone(readiness);
                std::thread::spawn(move || {
                    if let Err(error) = handle(
                        stream,
                        runtime.as_ref(),
                        authentication_enabled,
                        default_model_id.as_deref(),
                        &owners,
                        &readiness,
                    ) {
                        eprintln!("Marina HTTP request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("Marina HTTP accept failed: {error}"),
        }
    }
}

#[allow(clippy::too_many_lines)]
fn handle(
    mut stream: TcpStream,
    runtime: &dyn RuntimeService,
    authentication_enabled: bool,
    default_model_id: Option<&str>,
    owners: &Arc<Mutex<HashMap<String, String>>>,
    readiness: &Readiness,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    reader
        .get_mut()
        .set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line == "\r\n" || line == "\n" || line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }
    let content_length = header(&headers, "content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    if !request_body_size_is_allowed(content_length) {
        return respond_error(&mut stream, 413, "request body too large".to_owned());
    }
    let mut body = vec![0_u8; content_length];
    reader.read_exact(&mut body)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    let identity = if authentication_enabled {
        header(&headers, "authorization")
            .and_then(|value| value.strip_prefix("Bearer "))
            .and_then(auth::identity)
    } else {
        Some(auth::TokenIdentity {
            principal: "anonymous-local".to_owned(),
            scopes: vec![
                auth::MODELS_READ.to_owned(),
                auth::INFERENCE.to_owned(),
                "inference:cancel:self".to_owned(),
                auth::INFERENCE_CANCEL_ANY.to_owned(),
                auth::MODELS_ADMIN.to_owned(),
            ],
        })
    };
    if path != "/healthz" && path != "/readyz" && identity.is_none() {
        return if path.starts_with("/api/") {
            respond_ollama_error(&mut stream, 401, "missing or invalid bearer token")
        } else {
            respond_json(
                &mut stream,
                401,
                json!({"error": {"type": "authentication_error", "message": "missing or invalid bearer token"}}),
            )
        };
    }
    let identity = identity.unwrap_or_else(|| auth::TokenIdentity {
        principal: "health-check".to_owned(),
        scopes: Vec::new(),
    });
    match (method, path) {
        ("GET", "/healthz") => {
            let (ready, bootstrap) = readiness.snapshot(runtime);
            respond_json(
                &mut stream,
                200,
                json!({"status": "ok", "service_available": true, "inference_ready": ready, "bootstrap": bootstrap}),
            )
        }
        ("GET", "/readyz") => {
            let (ready, bootstrap) = readiness.snapshot(runtime);
            if ready {
                respond_json(
                    &mut stream,
                    200,
                    json!({"status": "ready", "inference_ready": true}),
                )
            } else {
                respond_json(
                    &mut stream,
                    503,
                    json!({"status": "not_ready", "inference_ready": false, "bootstrap": bootstrap}),
                )
            }
        }
        ("GET", "/version" | "/v1/version") => respond_json(
            &mut stream,
            200,
            super::build_info::json("talvikd", runtime.snapshot().backend_version.as_deref()),
        ),
        ("GET", "/api/version") => respond_json(
            &mut stream,
            200,
            json!({"version": env!("CARGO_PKG_VERSION")}),
        ),
        ("GET", "/v1/models") => {
            if !scope_allowed(&identity, auth::MODELS_READ) {
                return respond_scope_error(&mut stream, auth::MODELS_READ);
            }
            let resident_ids = runtime.snapshot().loaded_models;
            let data = runtime
                .model_list()
                .into_iter()
                .map(|model| {
                    json!({
                        "id": model.id,
                        "object": "model",
                        "owned_by": "marina",
                        "default": default_model_id.is_some_and(|id| id == model.id),
                        "resident": resident_ids.iter().any(|id| id == &model.id),
                        "estimated_memory_mib": model.memory_requirement_mb,
                        "runtime_profile": runtime.model_profile(&model.id).as_ref().map(profile_json)
                    })
                })
                .collect::<Vec<_>>();
            respond_json(&mut stream, 200, json!({"object": "list", "data": data}))
        }
        ("GET", "/api/tags") => {
            if !scope_allowed(&identity, auth::MODELS_READ) {
                return respond_ollama_scope_error(&mut stream, auth::MODELS_READ);
            }
            respond_json(&mut stream, 200, compatibility_tags(runtime))
        }
        ("GET", "/api/ps") => {
            if !scope_allowed(&identity, auth::MODELS_READ) {
                return respond_ollama_scope_error(&mut stream, auth::MODELS_READ);
            }
            respond_json(&mut stream, 200, compatibility_ps(runtime))
        }
        ("POST", "/api/show") => {
            if !scope_allowed(&identity, auth::MODELS_READ) {
                return respond_ollama_scope_error(&mut stream, auth::MODELS_READ);
            }
            let Some(request) = parse_ollama_json_body(&mut stream, &body)? else {
                return Ok(());
            };
            ollama_show(&mut stream, runtime, &request)
        }
        ("POST", "/api/embed" | "/api/embeddings") => {
            if !scope_allowed(&identity, auth::INFERENCE) {
                return respond_ollama_scope_error(&mut stream, auth::INFERENCE);
            }
            let legacy = path == "/api/embeddings";
            let Some(request) = parse_ollama_json_body(&mut stream, &body)? else {
                return Ok(());
            };
            ollama_embeddings(&mut stream, runtime, &request, legacy)
        }
        ("POST", "/api/chat" | "/api/generate") => {
            if !scope_allowed(&identity, auth::INFERENCE) {
                return respond_ollama_scope_error(&mut stream, auth::INFERENCE);
            }
            let endpoint = if path == "/api/chat" {
                OllamaGenerationEndpoint::Chat
            } else {
                OllamaGenerationEndpoint::Generate
            };
            let Some(request) = parse_ollama_json_body(&mut stream, &body)? else {
                return Ok(());
            };
            generate_ollama(&mut stream, runtime, &request, &identity, owners, endpoint)
        }
        ("POST", "/v1/embeddings") => {
            if !scope_allowed(&identity, auth::INFERENCE) {
                return respond_scope_error(&mut stream, auth::INFERENCE);
            }
            let Some(request) = parse_json_body(&mut stream, &body)? else {
                return Ok(());
            };
            embeddings(&mut stream, runtime, &request)
        }
        ("POST", path @ ("/v1/chat/completions" | "/v1/completions" | "/v1/responses")) => {
            if !scope_allowed(&identity, auth::INFERENCE) {
                return respond_scope_error(&mut stream, auth::INFERENCE);
            }
            let Some(request) = parse_json_body(&mut stream, &body)? else {
                return Ok(());
            };
            let endpoint = if path == "/v1/completions" {
                OpenAiGenerationEndpoint::Completions
            } else if path == "/v1/responses" {
                OpenAiGenerationEndpoint::Responses
            } else {
                OpenAiGenerationEndpoint::ChatCompletions
            };
            generate(&mut stream, runtime, &request, &identity, owners, endpoint)
        }
        ("POST", path) if path.starts_with("/v1/models/") && path.ends_with("/load") => {
            if !scope_allowed(&identity, auth::MODELS_ADMIN) {
                return respond_scope_error(&mut stream, auth::MODELS_ADMIN);
            }
            let model = path
                .trim_start_matches("/v1/models/")
                .trim_end_matches("/load");
            match runtime.model_load(model) {
                Ok(()) => respond_json(&mut stream, 200, json!({"id": model, "status": "loaded"})),
                Err(error) => respond_error(&mut stream, 409, error.to_string()),
            }
        }
        ("POST", path) if path.starts_with("/v1/models/") && path.ends_with("/context/reset") => {
            if !scope_allowed(&identity, auth::MODELS_ADMIN) {
                return respond_scope_error(&mut stream, auth::MODELS_ADMIN);
            }
            let model = path
                .trim_start_matches("/v1/models/")
                .trim_end_matches("/context/reset");
            match runtime.reset_context(model) {
                Ok(()) => respond_json(&mut stream, 200, json!({"id": model, "context": "reset"})),
                Err(error) => respond_error(&mut stream, 409, error.to_string()),
            }
        }
        ("POST", path) if path.starts_with("/v1/generations/") && path.ends_with("/cancel") => {
            if !identity.allows("inference:cancel:self")
                && !identity.allows(auth::INFERENCE_CANCEL_ANY)
            {
                return respond_json(
                    &mut stream,
                    403,
                    json!({"error": {"type": "authorization_error", "message": "generation cancellation is not authorized"}}),
                );
            }
            let request_id = path
                .trim_start_matches("/v1/generations/")
                .trim_end_matches("/cancel");
            let generation_id = runtime.active_generation_id(request_id);
            if let Ok(owners) = owners.lock() {
                if let Some(owner) = owners.get(request_id) {
                    if owner != &identity.principal && !identity.allows(auth::INFERENCE_CANCEL_ANY)
                    {
                        return respond_error(
                            &mut stream,
                            403,
                            "generation is owned by another principal".to_owned(),
                        );
                    }
                }
            }
            runtime.cancel_generation_for(request_id);
            respond_json(
                &mut stream,
                202,
                json!({"request_id": request_id, "generation_id": generation_id, "status": "cancellation_requested"}),
            )
        }
        (_, path) if path.starts_with("/api/") => {
            respond_ollama_error(&mut stream, 404, "not found")
        }
        _ => respond_error(&mut stream, 404, "not found".to_owned()),
    }
}

fn embeddings(
    stream: &mut TcpStream,
    runtime: &dyn RuntimeService,
    body: &Value,
) -> std::io::Result<()> {
    let Some(model) = body.get("model").and_then(Value::as_str) else {
        return respond_error(stream, 400, "model is required".to_owned());
    };
    let inputs = match body.get("input") {
        Some(Value::String(value)) => vec![value.clone()],
        Some(Value::Array(values)) => {
            let parsed = values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "input array values must be strings".to_owned())
                })
                .collect::<Result<Vec<_>, _>>();
            match parsed {
                Ok(inputs) => inputs,
                Err(message) => return respond_error(stream, 400, message),
            }
        }
        _ => return respond_error(stream, 400, "input is required".to_owned()),
    };
    if inputs.is_empty() || inputs.iter().any(|value| value.trim().is_empty()) {
        return respond_error(stream, 400, "input must contain non-empty text".to_owned());
    }
    if let Err(error) = runtime.model_load(model) {
        return respond_error(stream, runtime_error_status(&error), error.to_string());
    }
    let mut data = Vec::with_capacity(inputs.len());
    for (index, input) in inputs.iter().enumerate() {
        let result = runtime.embeddings(&oid_runtime::backends::EmbeddingsRequest {
            model_id: model.to_owned(),
            input: input.clone(),
        });
        let values = match result {
            Ok(result) => result.values,
            Err(error) => return respond_error(stream, 501, error.to_string()),
        };
        data.push(json!({"object":"embedding","embedding":values,"index":index}));
    }
    respond_json(
        stream,
        200,
        json!({"object":"list","data":data,"model":model,"usage":{"prompt_tokens":0,"total_tokens":0}}),
    )
}

fn parse_json_body(stream: &mut TcpStream, body: &[u8]) -> std::io::Result<Option<Value>> {
    match serde_json::from_slice(body) {
        Ok(value) => Ok(Some(value)),
        Err(error) => {
            respond_error(stream, 400, format!("invalid JSON body: {error}"))?;
            Ok(None)
        }
    }
}

fn parse_ollama_json_body(stream: &mut TcpStream, body: &[u8]) -> std::io::Result<Option<Value>> {
    match serde_json::from_slice(body) {
        Ok(value) => Ok(Some(value)),
        Err(error) => {
            respond_ollama_error(stream, 400, format!("invalid JSON body: {error}"))?;
            Ok(None)
        }
    }
}

fn compatibility_tags(runtime: &dyn RuntimeService) -> Value {
    let models = runtime
        .model_list()
        .into_iter()
        .map(|model| {
            let (size, modified_at) = local_model_file_metadata(&model.location);
            json!({
                "name": model.id.clone(),
                "model": model.id,
                "modified_at": modified_at,
                "size": size,
                "memory_requirement_bytes": model.memory_requirement_mb.saturating_mul(1_048_576),
                "digest": model.checksum,
                "details": {
                    "format": "gguf",
                    "family": model.family,
                    "families": [model.family],
                    "quantization_level": model.quantization,
                }
            })
        })
        .collect::<Vec<_>>();
    json!({"models": models})
}

fn compatibility_ps(runtime: &dyn RuntimeService) -> Value {
    let snapshot = runtime.snapshot();
    let metadata = runtime
        .model_list()
        .into_iter()
        .map(|model| (model.id.clone(), model))
        .collect::<HashMap<_, _>>();
    let models = snapshot
        .resident_models
        .into_iter()
        .map(|resident| {
            let model = metadata.get(&resident.id);
            json!({
                "name": resident.id,
                "model": resident.id,
                "size": resident.memory_bytes,
                "size_vram": resident.gpu_memory_bytes,
                "host_memory_bytes": resident.host_memory_bytes,
                "residency": "loaded",
                "execution_device": resident.execution_device,
                "selected_backend": resident.selected_backend,
                "requested_placement": resident.requested_placement,
                "gpu_layers_requested": resident.gpu_layers_requested,
                "gpu_layers_offloaded": resident.gpu_layers_offloaded,
                "fallback_applied": resident.fallback_applied,
                "fallback_reason": resident.fallback_reason,
                "load_duration_ms": resident.load_duration_ms,
                "qualification": {
                    "cpu": "not_run",
                    "cuda": "not_run",
                    "vulkan": "not_run",
                    "metal": "not_run"
                },
                "expires_at": null,
                "digest": model.and_then(|model| model.checksum.as_deref()),
                "idle_seconds": resident.idle_seconds,
                "details": {
                    "parent_model": "",
                    "family": model.map_or("unknown", |model| model.family.as_str()),
                    "families": model.map(|model| vec![model.family.as_str()]),
                    "quantization_level": model.map_or("unknown", |model| model.quantization.as_str()),
                    "format": "gguf"
                },
                "context_length": model.map(|model| model.context_window)
            })
        })
        .collect::<Vec<_>>();
    json!({
        "models": models,
        "active_requests": snapshot.active_requests,
        "pending_requests": snapshot.pending_requests,
        "queue_capacity": snapshot.queue_capacity,
        "automatic_eviction": snapshot.automatic_eviction,
        "idle_retention_seconds": snapshot.idle_retention_seconds,
        "inference_threads": snapshot.inference_threads
    })
}

fn ollama_show(
    stream: &mut TcpStream,
    runtime: &dyn RuntimeService,
    body: &Value,
) -> std::io::Result<()> {
    let Some(model_id) = body.get("model").and_then(Value::as_str) else {
        return respond_ollama_error(stream, 400, "model is required");
    };
    if let Some(verbose) = body.get("verbose") {
        if !verbose.is_boolean() {
            return respond_ollama_error(stream, 400, "verbose must be a boolean");
        }
        if verbose.as_bool() == Some(true) {
            return respond_ollama_error(
                stream,
                501,
                "verbose model metadata is not available for registered GGUF artifacts",
            );
        }
    }
    let Some(model) = runtime.model_inspect(model_id) else {
        return respond_ollama_error(stream, 404, format!("model {model_id:?} not found"));
    };
    respond_json(
        stream,
        200,
        json!({
            "details": {
                "format": "gguf",
                "family": model.family,
                "families": [model.family],
                "quantization_level": model.quantization
            },
            "capabilities": runtime.model_profile(model_id).as_ref().map(profile_capabilities).unwrap_or_default(),
            "declared_capabilities": model.capabilities,
            "runtime_profile": runtime.model_profile(model_id).as_ref().map(profile_json),
            "model_info": {}
        }),
    )
}

fn ollama_embeddings(
    stream: &mut TcpStream,
    runtime: &dyn RuntimeService,
    body: &Value,
    legacy: bool,
) -> std::io::Result<()> {
    let Some(model) = body.get("model").and_then(Value::as_str) else {
        return respond_ollama_error(stream, 400, "model is required");
    };
    let inputs = if legacy {
        body.get("prompt")
            .and_then(Value::as_str)
            .map(|input| vec![input.to_owned()])
    } else {
        match body.get("input") {
            Some(Value::String(input)) => Some(vec![input.clone()]),
            Some(Value::Array(inputs)) => {
                let inputs = inputs
                    .iter()
                    .map(|input| input.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>();
                if inputs.is_none() {
                    return respond_ollama_error(stream, 400, "input values must be strings");
                }
                inputs
            }
            _ => None,
        }
    };
    let Some(inputs) = inputs else {
        return respond_ollama_error(
            stream,
            400,
            if legacy {
                "prompt is required"
            } else {
                "input is required"
            },
        );
    };
    if inputs.is_empty() || inputs.iter().any(|input| input.trim().is_empty()) {
        return respond_ollama_error(stream, 400, "input must contain non-empty text");
    }
    for unsupported in ["dimensions", "keep_alive", "truncate", "options"] {
        if body.get(unsupported).is_some_and(|value| !value.is_null()) {
            return respond_ollama_error(
                stream,
                501,
                format!("embedding option `{unsupported}` is not supported yet"),
            );
        }
    }

    let request_started = Instant::now();
    let load_started = Instant::now();
    let was_resident = runtime
        .model_inspect(model)
        .is_some_and(|metadata| matches!(metadata.status, oid_runtime::ModelStatus::Loaded));
    if let Err(error) = runtime.model_load(model) {
        let status = if runtime.model_inspect(model).is_some() {
            409
        } else {
            404
        };
        return respond_ollama_error(stream, status, error.to_string());
    }
    let load_duration_ns = (!was_resident).then(|| duration_nanos(load_started.elapsed()));
    let mut embeddings = Vec::with_capacity(inputs.len());
    for input in &inputs {
        match runtime.embeddings(&oid_runtime::backends::EmbeddingsRequest {
            model_id: model.to_owned(),
            input: input.clone(),
        }) {
            Ok(result) => embeddings.push(result.values),
            Err(error) => return respond_ollama_error(stream, 501, error.to_string()),
        }
    }
    let total_duration_ns = duration_nanos(request_started.elapsed());
    if legacy {
        let Some(embedding) = embeddings.into_iter().next() else {
            return respond_ollama_error(stream, 400, "prompt is required");
        };
        return respond_json(stream, 200, json!({"embedding": embedding}));
    }
    let mut response = json!({
        "model": model,
        "embeddings": embeddings,
        "total_duration": total_duration_ns
    });
    if let Some(load_duration_ns) = load_duration_ns {
        response["load_duration"] = json!(load_duration_ns);
    }
    respond_json(stream, 200, response)
}

fn duration_nanos(duration: Duration) -> u128 {
    duration.as_nanos()
}

#[allow(clippy::cast_precision_loss)]
fn prompt_tokens_per_second(tokens: u64, duration_ms: Option<u128>) -> Option<f64> {
    duration_ms
        .and_then(|duration| (duration > 0).then_some(tokens as f64 * 1000.0 / duration as f64))
}

#[allow(clippy::too_many_lines)]
fn generate_ollama(
    stream: &mut TcpStream,
    runtime: &dyn RuntimeService,
    body: &Value,
    identity: &auth::TokenIdentity,
    owners: &Arc<Mutex<HashMap<String, String>>>,
    endpoint: OllamaGenerationEndpoint,
) -> std::io::Result<()> {
    let Some(model) = body.get("model").and_then(Value::as_str) else {
        return respond_ollama_error(stream, 400, "model is required");
    };
    if let Err((status, message)) = validate_ollama_generation_request(body, endpoint) {
        return respond_ollama_error(stream, status, message);
    }
    let structured_output = match parse_ollama_format(body.get("format")) {
        Ok(output) => output,
        Err((status, message)) => return respond_ollama_error(stream, status, message),
    };
    let mut prompt = match ollama_prompt(body, endpoint) {
        Ok(prompt) => prompt,
        Err((status, message)) => return respond_ollama_error(stream, status, message),
    };
    let base_prompt = prompt.clone();
    let enabled_thinking = body.get("think").and_then(Value::as_bool).or_else(|| {
        std::env::var("TALVIK_ENABLE_THINKING")
            .ok()
            .and_then(|v| v.parse().ok())
    });
    if endpoint == OllamaGenerationEndpoint::Chat {
        if let Some(enabled) = enabled_thinking {
            prompt.insert_str(
                0,
                &format!("\u{001e}__talvik_template\nenable_thinking={enabled}"),
            );
        }
    }
    let options = match ollama_generation_options(body) {
        Ok(options) => options,
        Err((status, message)) => return respond_ollama_error(stream, status, message),
    };
    let tool_definitions = body
        .get("tools")
        .and_then(Value::as_array)
        .filter(|tools| !tools.is_empty());
    if structured_output.is_some() && tool_definitions.is_some() {
        return respond_ollama_error(
            stream,
            400,
            "structured output cannot be combined with tools",
        );
    }
    let (prompt, tools) = if let Some(tools) = tool_definitions {
        (
            format!(
                "\u{001e}__talvik_tools\n{}\n{prompt}",
                Value::Array(tools.clone())
            ),
            true,
        )
    } else {
        (prompt, false)
    };
    let request_content = match ollama_request_content(body, endpoint, &base_prompt, &prompt) {
        Ok(content) => content,
        Err(message) => return respond_ollama_error(stream, 400, message),
    };
    let request_id = unique_request_id();
    let request_started = Instant::now();
    let was_resident = runtime
        .model_inspect(model)
        .is_some_and(|metadata| matches!(metadata.status, oid_runtime::ModelStatus::Loaded));
    let load_started = Instant::now();
    if let Err(error) = runtime.model_load(model) {
        let status = if runtime.model_inspect(model).is_some() {
            409
        } else {
            404
        };
        return respond_ollama_error(stream, status, error.to_string());
    }
    let load_duration_ns = if was_resident {
        0
    } else {
        duration_nanos(load_started.elapsed())
    };
    let output_profile = runtime.model_profile(model);
    if let Some(profile) = output_profile.as_ref() {
        if body.get("think").is_some() && !profile.thinking_control {
            return respond_ollama_error(
                stream,
                501,
                "the loaded template has no supported thinking control",
            );
        }
        if !profile.completion || (endpoint == OllamaGenerationEndpoint::Chat && !profile.chat) {
            return respond_ollama_error(
                stream,
                501,
                "the loaded model does not support this text generation endpoint",
            );
        }
        if request_content
            .iter()
            .any(|p| matches!(p, ContentPart::Image(_)))
            && !profile.vision
        {
            return respond_ollama_error(
                stream,
                501,
                "the loaded model has no initialized vision projector",
            );
        }
    }
    if tool_definitions.is_some() && !runtime.tool_support(model).supported {
        return respond_ollama_error(
            stream,
            501,
            "tool calls are unavailable for the selected model",
        );
    }
    let created_at = match iso8601_now() {
        Ok(created_at) => created_at,
        Err(error) => return respond_ollama_error(stream, 500, error.to_string()),
    };
    if !reserve_owner(owners, &request_id, &identity.principal) {
        return respond_ollama_error(
            stream,
            409,
            "request_id is already in use or ownership registry unavailable".to_owned(),
        );
    }
    let generation = match runtime.generate(GenerationRequest {
        request_id: request_id.clone(),
        model_id: model.to_owned(),
        prompt,
        content: request_content,
        options,
        structured_output: structured_output.clone(),
    }) {
        Ok(generation) => generation,
        Err(error) => {
            finish_owner(owners, &request_id);
            return respond_ollama_error(stream, runtime_error_status(&error), error.to_string());
        }
    };
    let _lease = HttpGenerationLease {
        generation: &generation,
        owners,
        request_id: &request_id,
    };
    stream.set_read_timeout(Some(Duration::from_millis(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(true);
    if streaming {
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n"
        )?;
        stream.flush()?;
    }
    let stream_tool_calls =
        streaming && endpoint == OllamaGenerationEndpoint::Chat && tool_definitions.is_some();
    let mut tool_parser = stream_tool_calls.then(|| {
        selected_tool_parser(
            request_id.clone(),
            output_profile.as_ref(),
            tool_definitions,
        )
    });
    let mut streamed_tool_calls = Vec::new();
    let mut reason_parser =
        if endpoint == OllamaGenerationEndpoint::Chat && structured_output.is_none() {
            reasoning_parser(output_profile.as_ref(), enabled_thinking)
        } else {
            None
        };
    let mut reasoning = String::new();
    let mut pending_completion = None;
    let mut output = String::new();
    loop {
        if peer_disconnected(stream)? {
            generation.cancel();
            let _ = drain_cancelled_generation(&generation);
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "inference client disconnected",
            ));
        }
        let message = pending_completion.take().map_or_else(
            || generation.recv_timeout(Duration::from_millis(100)),
            |result| Ok(GenerationMessage::Completed(result)),
        );
        let (message, thought) =
            split_reasoning_message(message, &mut reason_parser, &mut pending_completion);
        reasoning.push_str(&thought);
        if streaming && !thought.is_empty() {
            if let Err(error) = write_ndjson_line(
                stream,
                &json!({"model":model,"created_at":created_at,"message":{"role":"assistant","content":"","thinking":thought},"done":false}),
            ) {
                generation.cancel();
                let _ = drain_cancelled_generation(&generation);
                return Err(error);
            }
        }
        match message {
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Ok(GenerationMessage::Token(token)) => {
                if token.is_empty() {
                    continue;
                }
                if let Some(parser) = tool_parser.as_mut() {
                    let parts = match parser.push(&token) {
                        Ok(parts) => parts,
                        Err(message) => {
                            generation.cancel();
                            if !drain_cancelled_generation(&generation) {
                                eprintln!(
                                    "Talvik HTTP generation cleanup timed out request_id={request_id}"
                                );
                            }
                            return write_ndjson_line(stream, &json!({"error": message}));
                        }
                    };
                    for part in parts {
                        if let ToolStreamPart::Call { call, .. } = &part {
                            if !tool_definitions.is_some_and(|definitions| {
                                generated_tool_call_is_declared(call, definitions)
                            }) {
                                generation.cancel();
                                if !drain_cancelled_generation(&generation) {
                                    eprintln!(
                                        "Talvik HTTP generation cleanup timed out request_id={request_id}"
                                    );
                                }
                                return write_ndjson_line(
                                    stream,
                                    &json!({"error": "model returned an undeclared tool name"}),
                                );
                            }
                            streamed_tool_calls.push(call.clone());
                        }
                        if let Err(error) =
                            write_ollama_tool_stream_part(stream, model, &created_at, &part)
                        {
                            generation.cancel();
                            if !drain_cancelled_generation(&generation) {
                                eprintln!(
                                    "Talvik HTTP generation cleanup timed out request_id={request_id}"
                                );
                            }
                            return Err(error);
                        }
                    }
                } else {
                    output.push_str(&token);
                }
                if streaming && !tools {
                    let chunk = match endpoint {
                        OllamaGenerationEndpoint::Chat => json!({
                            "model": model,
                            "created_at": created_at,
                            "message": {"role": "assistant", "content": token},
                            "done": false
                        }),
                        OllamaGenerationEndpoint::Generate => json!({
                            "model": model,
                            "created_at": created_at,
                            "response": token,
                            "done": false
                        }),
                    };
                    if let Err(error) = write_ndjson_line(stream, &chunk) {
                        generation.cancel();
                        let _ = drain_cancelled_generation(&generation);
                        return Err(error);
                    }
                }
            }
            Ok(GenerationMessage::Completed(result)) => {
                if let Some(structured_output) = structured_output.as_ref() {
                    if let Err(message) = structured_output.validate_output(&output) {
                        if streaming {
                            return write_ndjson_line(stream, &json!({"error": message}));
                        }
                        return respond_ollama_error(stream, 502, message);
                    }
                }
                let tool_calls = if let Some(parser) = tool_parser.as_mut() {
                    let parts = match parser.finish() {
                        Ok(parts) => parts,
                        Err(message) => {
                            return write_ndjson_line(stream, &json!({"error": message}));
                        }
                    };
                    for part in parts {
                        if let ToolStreamPart::Call { call, .. } = &part {
                            if !tool_definitions.is_some_and(|definitions| {
                                generated_tool_call_is_declared(call, definitions)
                            }) {
                                return write_ndjson_line(
                                    stream,
                                    &json!({"error": "model returned an undeclared tool name"}),
                                );
                            }
                            streamed_tool_calls.push(call.clone());
                        }
                        write_ollama_tool_stream_part(stream, model, &created_at, &part)?;
                    }
                    (!streamed_tool_calls.is_empty()).then(|| {
                        streamed_tool_calls
                            .iter()
                            .map(ollama_tool_call)
                            .collect::<Vec<_>>()
                    })
                } else {
                    match parse_tool_calls_selected(
                        &output,
                        &unique_request_id(),
                        output_profile.as_ref(),
                        tool_definitions,
                    ) {
                        Ok(calls) => calls
                            .map(|calls| calls.iter().map(ollama_tool_call).collect::<Vec<_>>()),
                        Err(message) if streaming => {
                            return write_ndjson_line(stream, &json!({"error": message}));
                        }
                        Err(message) => return respond_ollama_error(stream, 502, message),
                    }
                };
                if tool_calls.as_ref().is_some_and(|calls| {
                    !tool_definitions.is_some_and(|definitions| {
                        calls
                            .iter()
                            .all(|call| generated_tool_call_is_declared(call, definitions))
                    })
                }) {
                    if streaming {
                        return write_ndjson_line(
                            stream,
                            &json!({"error": "model returned an undeclared tool name"}),
                        );
                    }
                    return respond_ollama_error(
                        stream,
                        502,
                        "model returned an undeclared tool name",
                    );
                }
                let mut terminal = match endpoint {
                    OllamaGenerationEndpoint::Chat => json!({
                        "model": model,
                        "created_at": created_at,
                        "message": {
                            "role": "assistant",
                            "content": if tool_calls.is_some() { "" } else { output.as_str() }
                        },
                        "done": true,
                        "done_reason": completion_finish_reason(result.statistics.termination)
                    }),
                    OllamaGenerationEndpoint::Generate => json!({
                        "model": model,
                        "created_at": created_at,
                        "response": output,
                        "done": true,
                        "done_reason": completion_finish_reason(result.statistics.termination)
                    }),
                };
                if let Some(tool_calls) = tool_calls.filter(|_| !streaming) {
                    if endpoint == OllamaGenerationEndpoint::Chat {
                        terminal["message"]["tool_calls"] = json!(tool_calls);
                    }
                }
                if !streaming
                    && !reasoning.trim().is_empty()
                    && endpoint == OllamaGenerationEndpoint::Chat
                {
                    terminal["message"]["thinking"] = json!(reasoning);
                }
                add_ollama_statistics(
                    &mut terminal,
                    &result.statistics,
                    duration_nanos(request_started.elapsed()),
                    load_duration_ns,
                );
                if streaming {
                    return write_ndjson_line(stream, &terminal);
                }
                return respond_json(stream, 200, terminal);
            }
            Ok(GenerationMessage::Cancelled(result)) => {
                let mut terminal = match endpoint {
                    OllamaGenerationEndpoint::Chat => json!({
                        "model": model,
                        "created_at": created_at,
                        "message": {"role": "assistant", "content": output},
                        "done": true,
                        "done_reason": "cancelled"
                    }),
                    OllamaGenerationEndpoint::Generate => json!({
                        "model": model,
                        "created_at": created_at,
                        "response": output,
                        "done": true,
                        "done_reason": "cancelled"
                    }),
                };
                add_ollama_statistics(
                    &mut terminal,
                    &result.statistics,
                    duration_nanos(request_started.elapsed()),
                    load_duration_ns,
                );
                if streaming {
                    return write_ndjson_line(stream, &terminal);
                }
                return respond_json(stream, 200, terminal);
            }
            Ok(GenerationMessage::Failed(error)) => {
                if streaming {
                    return write_ndjson_line(stream, &json!({"error": error.to_string()}));
                }
                return respond_ollama_error(
                    stream,
                    runtime_error_status(&error),
                    error.to_string(),
                );
            }
            Err(error) => {
                if streaming {
                    return write_ndjson_line(
                        stream,
                        &json!({"error": format!("generation stream failed: {error}")}),
                    );
                }
                return respond_ollama_error(stream, 500, error.to_string());
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn validate_ollama_generation_request(
    body: &Value,
    endpoint: OllamaGenerationEndpoint,
) -> Result<(), (u16, String)> {
    for field in ["keep_alive", "template", "context"] {
        if body.get(field).is_some_and(|value| !value.is_null()) {
            return Err((501, format!("request field `{field}` is not supported yet")));
        }
    }
    if body.get("think").is_some_and(|v| !v.is_boolean()) {
        return Err((400, "think must be a boolean".to_owned()));
    }
    if endpoint != OllamaGenerationEndpoint::Chat && body.get("think").is_some() {
        return Err((501, "think is supported only by chat".to_owned()));
    }
    parse_ollama_format(body.get("format"))?;
    if body.get("raw").and_then(Value::as_bool) == Some(true) {
        return Err((501, "raw prompt mode is not supported yet".to_owned()));
    }
    if body.get("raw").is_some_and(|raw| !raw.is_boolean()) {
        return Err((400, "raw must be a boolean".to_owned()));
    }
    for field in ["system", "suffix"] {
        if body.get(field).is_some_and(|value| !value.is_string()) {
            return Err((400, format!("{field} must be a string")));
        }
    }
    if endpoint == OllamaGenerationEndpoint::Generate && body.get("suffix").is_some() {
        return Err((
            501,
            "suffix completion is not supported by the active generation contract".to_owned(),
        ));
    }
    match endpoint {
        OllamaGenerationEndpoint::Chat => {
            let Some(messages) = body.get("messages").and_then(Value::as_array) else {
                return Err((400, "messages must be an array".to_owned()));
            };
            if messages.is_empty() {
                return Err((400, "messages must contain at least one message".to_owned()));
            }
            if messages.iter().any(|message| {
                message
                    .get("content")
                    .is_some_and(|content| !content.is_string() && !content.is_null())
            }) {
                return Err((400, "message content must be a string".to_owned()));
            }
            for message in messages {
                if let Some(images) = message.get("images").filter(|images| !images.is_null()) {
                    let Some(images) = images.as_array() else {
                        return Err((400, "message images must be an array".to_owned()));
                    };
                    if images.iter().any(|image| !image.is_string()) {
                        return Err((400, "message images must contain base64 strings".to_owned()));
                    }
                }
            }
        }
        OllamaGenerationEndpoint::Generate => {
            if !body.get("prompt").is_some_and(Value::is_string) {
                return Err((400, "prompt must be a string".to_owned()));
            }
            if let Some(images) = body.get("images").filter(|images| !images.is_null()) {
                let Some(images) = images.as_array() else {
                    return Err((400, "images must be an array".to_owned()));
                };
                if images.iter().any(|image| !image.is_string()) {
                    return Err((400, "images must contain base64 strings".to_owned()));
                }
            }
        }
    }
    if body
        .get("stream")
        .is_some_and(|stream| !stream.is_boolean())
    {
        return Err((400, "stream must be a boolean".to_owned()));
    }
    if body.get("tools").is_some_and(|tools| !tools.is_array()) {
        return Err((400, "tools must be an array".to_owned()));
    }
    if endpoint == OllamaGenerationEndpoint::Generate && body.get("tools").is_some() {
        return Err((400, "tools are only valid for /api/chat".to_owned()));
    }
    if let Some(options) = body.get("options") {
        let Some(options) = options.as_object() else {
            return Err((400, "options must be an object".to_owned()));
        };
        let supported = [
            "num_ctx",
            "num_predict",
            "temperature",
            "top_p",
            "top_k",
            "seed",
            "stop",
        ];
        if let Some(key) = options
            .keys()
            .find(|key| !supported.contains(&key.as_str()))
        {
            return Err((
                501,
                format!("generation option `{key}` is not supported yet"),
            ));
        }
        for (field, minimum, maximum) in [
            ("num_ctx", 1_u64, MAX_CONTEXT_TOKENS),
            ("num_predict", 1_u64, u64::from(u32::MAX)),
            ("top_k", 1_u64, i32::MAX as u64),
            ("seed", 0_u64, u64::from(u32::MAX)),
        ] {
            if let Some(value) = options.get(field) {
                if !value
                    .as_u64()
                    .is_some_and(|number| (minimum..=maximum).contains(&number))
                {
                    return Err((
                        400,
                        format!("options.{field} must be between {minimum} and {maximum}"),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn ollama_prompt(
    body: &Value,
    endpoint: OllamaGenerationEndpoint,
) -> Result<String, (u16, String)> {
    match endpoint {
        OllamaGenerationEndpoint::Chat => {
            let messages = body.get("messages").expect("validated messages array");
            let mut prompt = build_chat_prompt(&json!({"messages": messages}))
                .map_err(|message| (400, message))?;
            if let Some(system) = body.get("system").and_then(Value::as_str) {
                prompt.insert_str(0, &format!("\u{001e}system\n{system}\n"));
            }
            Ok(prompt)
        }
        OllamaGenerationEndpoint::Generate => {
            let prompt = body
                .get("prompt")
                .and_then(Value::as_str)
                .expect("validated prompt string");
            let mut output = String::new();
            if let Some(system) = body.get("system").and_then(Value::as_str) {
                output.push_str("\u{001e}system\n");
                output.push_str(system);
                output.push('\n');
            }
            output.push_str(prompt);
            if output.trim().is_empty() {
                return Err((400, "prompt must not be empty".to_owned()));
            }
            Ok(output)
        }
    }
}

fn ollama_request_content(
    body: &Value,
    endpoint: OllamaGenerationEndpoint,
    base_prompt: &str,
    final_prompt: &str,
) -> Result<Vec<ContentPart>, String> {
    let mut parts = match endpoint {
        OllamaGenerationEndpoint::Chat => build_chat_content(&json!({
            "messages": body.get("messages").ok_or_else(|| "messages is required".to_owned())?
        }))?,
        OllamaGenerationEndpoint::Generate => {
            let mut parts = vec![ContentPart::Text(base_prompt.to_owned())];
            if let Some(images) = body.get("images").filter(|images| !images.is_null()) {
                let images = images
                    .as_array()
                    .ok_or_else(|| "images must be an array".to_owned())?;
                for encoded in images {
                    let encoded = encoded
                        .as_str()
                        .ok_or_else(|| "images must contain base64 strings".to_owned())?;
                    push_image_part(&mut parts, decode_ollama_image(encoded)?)?;
                }
            }
            parts
        }
    };
    if !parts
        .iter()
        .any(|part| matches!(part, ContentPart::Image(_)))
    {
        return Ok(Vec::new());
    }
    let content_prompt = flatten_content(&parts);
    let prefix = final_prompt
        .strip_suffix(&content_prompt)
        .unwrap_or_default()
        .to_owned();
    prepend_text_part(&mut parts, &prefix);
    Ok(parts)
}

fn ollama_generation_options(body: &Value) -> Result<GenerationOptions, (u16, String)> {
    let supplied = body
        .get("options")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut standard = serde_json::Map::new();
    for (ollama_name, talvik_name) in [
        ("num_ctx", "context_size"),
        ("num_predict", "max_completion_tokens"),
        ("temperature", "temperature"),
        ("top_p", "top_p"),
        ("top_k", "top_k"),
        ("seed", "seed"),
        ("stop", "stop"),
    ] {
        if let Some(value) = supplied.get(ollama_name) {
            standard.insert(talvik_name.to_owned(), value.clone());
        }
    }
    if let Some(value) = standard.get("max_completion_tokens") {
        if value.as_u64().is_none_or(|count| count == 0) {
            return Err((
                400,
                "options.num_predict must be a positive integer".to_owned(),
            ));
        }
    }
    let context_size = standard
        .get("context_size")
        .and_then(Value::as_u64)
        .unwrap_or(4096);
    if context_size == 0 || context_size > MAX_CONTEXT_TOKENS {
        return Err((
            400,
            format!("options.num_ctx must be between 1 and {MAX_CONTEXT_TOKENS}"),
        ));
    }
    validate_generation_request(
        &Value::Object(standard.clone()),
        OpenAiGenerationEndpoint::ChatCompletions,
    )?;
    Ok(generation_options(
        &Value::Object(standard),
        u32::try_from(context_size).unwrap_or(u32::MAX),
    ))
}

fn ollama_tool_call(call: &Value) -> Value {
    let arguments = call
        .pointer("/function/arguments")
        .and_then(Value::as_str)
        .and_then(|arguments| serde_json::from_str::<Value>(arguments).ok())
        .unwrap_or_else(|| json!({}));
    json!({
        "function": {
            "name": call.pointer("/function/name").and_then(Value::as_str).unwrap_or("unknown"),
            "arguments": arguments
        }
    })
}

fn write_ndjson_line(stream: &mut TcpStream, value: &Value) -> std::io::Result<()> {
    writeln!(stream, "{value}")?;
    stream.flush()
}

fn write_openai_tool_stream_part(
    stream: &mut TcpStream,
    request_id: &str,
    created: u64,
    model: &str,
    part: &ToolStreamPart,
    headers_sent: &mut bool,
) -> std::io::Result<()> {
    let Some(delta) = openai_tool_stream_delta(part, !*headers_sent) else {
        return Ok(());
    };
    if !*headers_sent {
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n"
        )?;
        *headers_sent = true;
    }
    let event = json!({
        "id": request_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{"index": 0, "delta": delta, "finish_reason": Value::Null}]
    });
    write!(stream, "data: {event}\n\n")?;
    stream.flush()
}

fn openai_tool_stream_delta(part: &ToolStreamPart, include_role: bool) -> Option<Value> {
    let mut delta = match part {
        ToolStreamPart::Content(content) if content.is_empty() => None,
        ToolStreamPart::Content(content) => Some(json!({"content": content})),
        ToolStreamPart::Call { index, call } => Some(json!({
            "tool_calls": [{
                "index": index,
                "id": call["id"],
                "type": "function",
                "function": {
                    "name": call["function"]["name"],
                    "arguments": call["function"]["arguments"]
                }
            }]
        })),
    }?;
    if include_role {
        delta["role"] = json!("assistant");
    }
    Some(delta)
}

fn write_openai_stream_error(
    stream: &mut TcpStream,
    headers_sent: bool,
    code: &str,
    message: &str,
) -> std::io::Result<()> {
    write_openai_stream_failure(
        stream,
        headers_sent,
        502,
        "model_output_error",
        code,
        message,
    )
}

fn write_openai_stream_failure(
    stream: &mut TcpStream,
    headers_sent: bool,
    status: u16,
    error_type: &str,
    code: &str,
    message: &str,
) -> std::io::Result<()> {
    let error = json!({"error": {
        "type": error_type,
        "message": message,
        "param": Value::Null,
        "code": code
    }});
    if !headers_sent {
        return respond_json(stream, status, error);
    }
    let frame = openai_stream_error_frame(error_type, code, message);
    stream.write_all(frame.as_bytes())?;
    stream.flush()
}

fn openai_stream_error_frame(error_type: &str, code: &str, message: &str) -> String {
    let error = openai_error(error_type, code, message);
    format!("data: {error}\n\n")
}

fn openai_model_output_error(code: &str, message: &str) -> Value {
    openai_error("model_output_error", code, message)
}

fn openai_error(error_type: &str, code: &str, message: &str) -> Value {
    json!({"error": {
        "type": error_type,
        "message": message,
        "param": Value::Null,
        "code": code
    }})
}

fn respond_model_output_error(
    stream: &mut TcpStream,
    code: &str,
    message: &str,
) -> std::io::Result<()> {
    respond_json(stream, 502, openai_model_output_error(code, message))
}

fn responses_sse_frame(event_type: &str, sequence_number: u64, payload: Value) -> String {
    let mut event = match payload {
        Value::Object(event) => event,
        _ => serde_json::Map::new(),
    };
    event.insert("type".to_owned(), json!(event_type));
    event.insert("sequence_number".to_owned(), json!(sequence_number));
    format!("event: {event_type}\ndata: {}\n\n", Value::Object(event))
}

fn write_responses_sse_event(
    stream: &mut TcpStream,
    sequence_number: &mut u64,
    event_type: &str,
    payload: Value,
) -> std::io::Result<()> {
    let frame = responses_sse_frame(event_type, *sequence_number, payload);
    *sequence_number = sequence_number.saturating_add(1);
    stream.write_all(frame.as_bytes())?;
    stream.flush()
}

fn responses_output_message(request_id: &str, status: &str, text: &str) -> Value {
    json!({
        "id": format!("msg_{request_id}"),
        "type": "message",
        "status": status,
        "role": "assistant",
        "content": [{
            "type": "output_text",
            "text": text,
            "annotations": []
        }]
    })
}

// Keep wire fields explicit at serialization boundaries; callers supply named locals.
#[allow(clippy::too_many_arguments)]
fn responses_object(
    request_id: &str,
    created: u64,
    model: &str,
    body: &Value,
    temperature: f32,
    top_p: f32,
    status: &str,
    output: Vec<Value>,
    usage: Value,
    error: Value,
    incomplete_details: Value,
) -> Value {
    let mut response = json!({
        "id": format!("resp_{request_id}"),
        "object": "response",
        "created_at": created,
        "status": status,
        "background": false,
        "instructions": body.get("instructions").cloned().unwrap_or(Value::Null),
        "max_output_tokens": body.get("max_output_tokens").cloned().unwrap_or(Value::Null),
        "model": model,
        "parallel_tool_calls": false,
        "previous_response_id": Value::Null,
        "temperature": temperature,
        "text": {"format": {"type": "text"}},
        "top_p": top_p,
        "truncation": "disabled",
        "metadata": body.get("metadata").cloned().unwrap_or_else(|| json!({})),
        "store": false
    });
    response["output"] = Value::Array(output);
    response["usage"] = usage;
    response["error"] = error;
    response["incomplete_details"] = incomplete_details;
    response
}

// Keep wire fields explicit at serialization boundaries; callers supply named locals.
#[allow(clippy::too_many_arguments)]
fn write_responses_stream_start(
    stream: &mut TcpStream,
    sequence_number: &mut u64,
    request_id: &str,
    created: u64,
    model: &str,
    body: &Value,
    temperature: f32,
    top_p: f32,
) -> std::io::Result<()> {
    let message_id = format!("msg_{request_id}");
    let response = responses_object(
        request_id,
        created,
        model,
        body,
        temperature,
        top_p,
        "in_progress",
        Vec::new(),
        Value::Null,
        Value::Null,
        Value::Null,
    );
    write_responses_sse_event(
        stream,
        sequence_number,
        "response.created",
        json!({"response": response}),
    )?;
    write_responses_sse_event(
        stream,
        sequence_number,
        "response.in_progress",
        json!({"response": response}),
    )?;
    write_responses_sse_event(
        stream,
        sequence_number,
        "response.output_item.added",
        json!({
            "output_index": 0,
            "item": {
                "id": message_id,
                "type": "message",
                "status": "in_progress",
                "role": "assistant",
                "content": []
            }
        }),
    )?;
    write_responses_sse_event(
        stream,
        sequence_number,
        "response.content_part.added",
        json!({
            "item_id": message_id,
            "output_index": 0,
            "content_index": 0,
            "part": {"type": "output_text", "text": "", "annotations": []}
        }),
    )
}

// Keep wire fields explicit at serialization boundaries; callers supply named locals.
#[allow(clippy::too_many_arguments)]
fn write_responses_stream_failure(
    stream: &mut TcpStream,
    sequence_number: &mut u64,
    request_id: &str,
    created: u64,
    model: &str,
    body: &Value,
    temperature: f32,
    top_p: f32,
    partial_output: &str,
    code: &str,
    message: &str,
) -> std::io::Result<()> {
    let output = if partial_output.is_empty() {
        Vec::new()
    } else {
        vec![responses_output_message(
            request_id,
            "incomplete",
            partial_output,
        )]
    };
    let response = responses_object(
        request_id,
        created,
        model,
        body,
        temperature,
        top_p,
        "failed",
        output,
        Value::Null,
        json!({"type": "server_error", "code": code, "message": message}),
        Value::Null,
    );
    write_responses_sse_event(
        stream,
        sequence_number,
        "response.failed",
        json!({"response": response}),
    )
}

fn write_ollama_tool_stream_part(
    stream: &mut TcpStream,
    model: &str,
    created_at: &str,
    part: &ToolStreamPart,
) -> std::io::Result<()> {
    let Some(chunk) = ollama_tool_stream_chunk(model, created_at, part) else {
        return Ok(());
    };
    write_ndjson_line(stream, &chunk)
}

fn ollama_tool_stream_chunk(model: &str, created_at: &str, part: &ToolStreamPart) -> Option<Value> {
    let message = match part {
        ToolStreamPart::Content(content) if content.is_empty() => return None,
        ToolStreamPart::Content(content) => json!({"role": "assistant", "content": content}),
        ToolStreamPart::Call { call, .. } => json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [ollama_tool_call(call)]
        }),
    };
    Some(json!({
        "model": model,
        "created_at": created_at,
        "message": message,
        "done": false
    }))
}

fn generated_tool_call_is_declared(call: &Value, definitions: &[Value]) -> bool {
    let Some(name) = call.pointer("/function/name").and_then(Value::as_str) else {
        return false;
    };
    definitions.iter().any(|definition| {
        definition.pointer("/function/name").and_then(Value::as_str) == Some(name)
    })
}

fn validate_streamed_tool_call(
    call: &Value,
    previous_calls: &[Value],
    choice: &OpenAiToolChoice,
    parallel_tool_calls: bool,
) -> Result<(), String> {
    let mut calls = previous_calls.to_vec();
    calls.push(call.clone());
    validate_generated_tool_calls(choice, Some(&calls), parallel_tool_calls)
}

fn add_ollama_statistics(
    response: &mut Value,
    statistics: &oid_runtime::GenerationStatistics,
    total_duration_ns: u128,
    load_duration_ns: u128,
) {
    response["total_duration"] = json!(total_duration_ns);
    response["load_duration"] = json!(load_duration_ns);
    if statistics.metrics_known {
        response["prompt_eval_count"] = json!(statistics.prompt_tokens);
        response["eval_count"] = json!(statistics.generated_tokens);
    }
    if let Some(duration) = statistics.prompt_evaluation_time_ms {
        response["prompt_eval_duration"] = json!(duration.saturating_mul(1_000_000));
    }
    if let Some(duration) = statistics.generation_time_ms {
        response["eval_duration"] = json!(duration.saturating_mul(1_000_000));
    }
}

fn iso8601_now() -> std::io::Result<String> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| invalid_data(format!("system clock is before Unix epoch: {error}")))?
        .as_secs();
    iso8601_timestamp(seconds)
}

fn local_model_file_metadata(location: &str) -> (Option<u64>, Option<String>) {
    let Ok(metadata) = std::fs::metadata(location) else {
        return (None, None);
    };
    let size = Some(metadata.len());
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| iso8601_timestamp(duration.as_secs()).ok());
    (size, modified_at)
}

fn iso8601_timestamp(seconds: u64) -> std::io::Result<String> {
    let days = i64::try_from(seconds / 86_400)
        .map_err(|error| invalid_data(format!("timestamp is out of range: {error}")))?;
    let day_seconds = seconds % 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        day_seconds / 3_600,
        (day_seconds % 3_600) / 60,
        day_seconds % 60
    ))
}

fn scope_allowed(identity: &auth::TokenIdentity, scope: &str) -> bool {
    identity.allows(scope)
}

fn respond_scope_error(stream: &mut TcpStream, scope: &str) -> std::io::Result<()> {
    respond_json(
        stream,
        403,
        json!({"error": {"type": "authorization_error", "message": format!("required scope: {scope}")}}),
    )
}

fn respond_ollama_scope_error(stream: &mut TcpStream, scope: &str) -> std::io::Result<()> {
    respond_ollama_error(stream, 403, format!("required scope: {scope}"))
}

fn profile_capabilities(profile: &ModelProfile) -> Vec<&'static str> {
    let mut result = Vec::new();
    for (enabled, name) in [
        (profile.completion, "completion"),
        (profile.chat, "chat"),
        (profile.embedding, "embedding"),
        (profile.vision, "vision"),
        (profile.tool_format.is_some(), "tools"),
        (profile.reasoning_format.is_some(), "thinking"),
    ] {
        if enabled {
            result.push(name);
        }
    }
    result
}

fn profile_json(profile: &ModelProfile) -> Value {
    json!({
        "architecture": profile.architecture, "training_context_length": profile.context_length,
        "capabilities": profile_capabilities(profile), "tokenizer_adds_bos": profile.tokenizer_adds_bos,
        "cache_strategy": match profile.cache { CacheStrategy::Disabled => "disabled", CacheStrategy::Attention => "attention", CacheStrategy::Recurrent => "recurrent_checkpoint" },
        "tools_declared": profile.tools_declared,
        "tool_parser": profile.tool_format.map(|format| match format { ToolFormat::XmlJson => "xml_json", ToolFormat::XmlFunction => "xml_function" }),
        "embedding_declared":profile.embedding_declared,
        "reasoning_parser": profile.reasoning_format.map(|ReasoningFormat::ThinkTags| "think_tags"),
        "thinking_control": profile.thinking_control, "reasoning_prefilled": profile.reasoning_prefilled,
        "evidence": "loaded_artifact_and_backend_support"
    })
}

fn reasoning_parser(
    profile: Option<&ModelProfile>,
    enabled: Option<bool>,
) -> Option<ReasoningStreamParser> {
    let profile = profile?;
    match profile.reasoning_format? {
        ReasoningFormat::ThinkTags => Some(ReasoningStreamParser::new(
            enabled
                .filter(|_| profile.thinking_control)
                .unwrap_or(profile.reasoning_prefilled),
        )),
    }
}

fn selected_tool_parser(
    id: String,
    profile: Option<&ModelProfile>,
    definitions: Option<&Vec<Value>>,
) -> ToolCallStreamParser {
    let mut parser = ToolCallStreamParser::new(id);
    parser.format = profile
        .and_then(|p| p.tool_format)
        .unwrap_or(ToolFormat::XmlJson);
    parser.definitions = definitions.cloned().unwrap_or_default();
    parser
}

fn split_reasoning_message(
    message: Result<GenerationMessage, std::sync::mpsc::RecvTimeoutError>,
    parser: &mut Option<ReasoningStreamParser>,
    pending: &mut Option<GenerationResult>,
) -> (
    Result<GenerationMessage, std::sync::mpsc::RecvTimeoutError>,
    String,
) {
    match message {
        Ok(GenerationMessage::Token(token)) => {
            let parsed = parser.as_mut().map_or_else(
                || ParsedText {
                    content: token.clone(),
                    reasoning: String::new(),
                },
                |p| p.push(&token),
            );
            (
                Ok(GenerationMessage::Token(parsed.content)),
                parsed.reasoning,
            )
        }
        Ok(GenerationMessage::Completed(result)) => {
            let tail = parser
                .take()
                .map_or_else(ParsedText::default, |mut p| p.finish());
            if tail.content.is_empty() {
                (Ok(GenerationMessage::Completed(result)), tail.reasoning)
            } else {
                *pending = Some(result);
                (Ok(GenerationMessage::Token(tail.content)), tail.reasoning)
            }
        }
        other => (other, String::new()),
    }
}

fn write_openai_reasoning(
    stream: &mut TcpStream,
    id: &str,
    created: u64,
    model: &str,
    text: &str,
    headers: &mut bool,
) -> std::io::Result<()> {
    if !*headers {
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n")?;
    }
    let mut delta = json!({"reasoning_content":text});
    if !*headers {
        delta["role"] = json!("assistant");
    }
    *headers = true;
    let event = json!({"id":id,"object":"chat.completion.chunk","created":created,"model":model,"choices":[{"index":0,"delta":delta,"finish_reason":Value::Null}]});
    write!(stream, "data: {event}\n\n")?;
    stream.flush()
}

const TOOL_CALL_START: &str = "<tool_call>";
const TOOL_CALL_END: &str = "</tool_call>";
const MAX_TOOL_CALL_BYTES: usize = 64 * 1024;

enum ToolStreamPart {
    Content(String),
    Call { index: usize, call: Value },
}

struct ToolCallStreamParser {
    format: ToolFormat,
    definitions: Vec<Value>,
    buffer: String,
    inside_call: bool,
    close_search_from: usize,
    id_prefix: String,
    next_index: usize,
}

impl ToolCallStreamParser {
    fn new(id_prefix: String) -> Self {
        Self {
            format: ToolFormat::XmlJson,
            definitions: Vec::new(),
            buffer: String::new(),
            inside_call: false,
            close_search_from: 0,
            id_prefix,
            next_index: 0,
        }
    }

    fn push(&mut self, chunk: &str) -> Result<Vec<ToolStreamPart>, String> {
        self.buffer.push_str(chunk);
        self.process_buffer()
    }

    fn finish(&mut self) -> Result<Vec<ToolStreamPart>, String> {
        let mut parts = self.process_buffer()?;
        if self.inside_call {
            return Err("model ended with an incomplete tool call".to_owned());
        }
        if self.buffer.is_empty() {
            return Ok(parts);
        }
        if TOOL_CALL_START
            .as_bytes()
            .starts_with(self.buffer.as_bytes())
        {
            // Some instruction-tuned models emit another opening marker after
            // a complete call, then stop before its payload. The completed call
            // is already valid and streamed to the client, so discard only
            // this trailing partial marker instead of invalidating the turn.
            if self.next_index > 0 {
                self.buffer.clear();
                return Ok(parts);
            }
            return Err("model ended with an incomplete tool-call marker".to_owned());
        }
        let remaining = std::mem::take(&mut self.buffer);
        let candidate = remaining.trim();
        let candidate = candidate
            .strip_prefix("```json")
            .and_then(|v| v.trim_end().strip_suffix("```"))
            .map_or(candidate, str::trim);
        if let Ok(value) = serde_json::from_str::<Value>(candidate) {
            if value.get("name").is_some() && value.get("arguments").is_some() {
                parts.push(self.parse_call(&value)?);
                return Ok(parts);
            }
        }
        parts.push(ToolStreamPart::Content(remaining));
        Ok(parts)
    }

    fn process_buffer(&mut self) -> Result<Vec<ToolStreamPart>, String> {
        let mut parts = Vec::new();
        loop {
            if self.inside_call {
                let bytes = self.buffer.as_bytes();
                if let Some(relative) = find_byte_sequence(
                    &bytes[self.close_search_from.min(bytes.len())..],
                    TOOL_CALL_END.as_bytes(),
                ) {
                    let end = self.close_search_from + relative;
                    if end > MAX_TOOL_CALL_BYTES {
                        return Err("model tool call exceeded the 64 KiB limit".to_owned());
                    }
                    let value = parse_tool_payload(
                        self.buffer[..end].trim(),
                        self.format,
                        &self.definitions,
                    )?;
                    parts.push(self.parse_call(&value)?);
                    self.buffer.drain(..end + TOOL_CALL_END.len());
                    self.inside_call = false;
                    self.close_search_from = 0;
                    continue;
                }
                if self.buffer.len() > MAX_TOOL_CALL_BYTES {
                    return Err("model tool call exceeded the 64 KiB limit".to_owned());
                }
                self.close_search_from = self
                    .buffer
                    .len()
                    .saturating_sub(TOOL_CALL_END.len().saturating_sub(1));
                break;
            }

            let leading = self.buffer.trim_start();
            if self.next_index == 0
                && (leading.starts_with('{')
                    || leading.starts_with("```json")
                    || (!leading.is_empty() && "```json".starts_with(leading)))
            {
                if self.buffer.len() > MAX_TOOL_CALL_BYTES {
                    return Err("model tool call exceeded the 64 KiB limit".to_owned());
                }
                // A standalone JSON call may be fenced by coding models. Hold
                // only this leading bounded candidate until completion; never
                // scan arbitrary answer prose or later code blocks for calls.
                break;
            }
            if let Some(start) = self.buffer.find(TOOL_CALL_START) {
                if start > 0 {
                    parts.push(ToolStreamPart::Content(self.buffer[..start].to_owned()));
                }
                self.buffer.drain(..start + TOOL_CALL_START.len());
                self.inside_call = true;
                self.close_search_from = 0;
                continue;
            }

            let possible_marker_bytes =
                suffix_prefix_overlap(self.buffer.as_bytes(), TOOL_CALL_START.as_bytes());
            let safe_length = self.buffer.len().saturating_sub(possible_marker_bytes);
            if safe_length > 0 {
                parts.push(ToolStreamPart::Content(
                    self.buffer.drain(..safe_length).collect(),
                ));
            }
            break;
        }
        Ok(parts)
    }

    fn parse_call(&mut self, value: &Value) -> Result<ToolStreamPart, String> {
        let Some(name) = value.get("name").and_then(Value::as_str) else {
            return Err("model tool call is missing a function name".to_owned());
        };
        if name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err("model tool call has an invalid function name".to_owned());
        }
        let Some(arguments) = value.get("arguments") else {
            return Err("model tool call is missing arguments".to_owned());
        };
        let arguments = match arguments {
            Value::String(arguments) => serde_json::from_str::<Value>(arguments)
                .map_err(|error| format!("model tool call arguments are invalid JSON: {error}"))?,
            arguments => arguments.clone(),
        };
        if !arguments.is_object() {
            return Err("model tool call arguments must be a JSON object".to_owned());
        }
        let normalized = normalize_tool_call(
            &json!({"name": name, "arguments": arguments}),
            &self.id_prefix,
            self.next_index,
        );
        let index = self.next_index;
        self.next_index += 1;
        Ok(ToolStreamPart::Call {
            index,
            call: normalized,
        })
    }
}

fn parse_tool_payload(
    payload: &str,
    format: ToolFormat,
    definitions: &[Value],
) -> Result<Value, String> {
    if format == ToolFormat::XmlJson || payload.starts_with('{') {
        return serde_json::from_str(payload)
            .map_err(|error| format!("malformed model tool call: {error}"));
    }
    // This is the model's bounded function-tag protocol, not general XML:
    // no DTDs, entities, attributes, I/O or recursive element expansion.
    let rest = payload
        .strip_prefix("<function=")
        .ok_or_else(|| "model tool call is missing a function block".to_owned())?;
    let (name, rest) = rest
        .split_once('>')
        .ok_or_else(|| "model tool call has an incomplete function name".to_owned())?;
    let mut body = rest
        .trim_end()
        .strip_suffix("</function>")
        .ok_or_else(|| "model tool call is missing its function terminator".to_owned())?
        .trim();
    let schema = definitions
        .iter()
        .find(|d| d.pointer("/function/name").and_then(Value::as_str) == Some(name))
        .and_then(|d| d.pointer("/function/parameters/properties"));
    let mut arguments = serde_json::Map::new();
    while !body.is_empty() {
        let parameter = body
            .strip_prefix("<parameter=")
            .ok_or_else(|| "unexpected content in model function call".to_owned())?;
        let (key, rest) = parameter
            .split_once('>')
            .ok_or_else(|| "incomplete model parameter name".to_owned())?;
        if key.is_empty() || arguments.contains_key(key) {
            return Err("empty or duplicate model parameter name".to_owned());
        }
        let (raw, remaining) = rest
            .split_once("</parameter>")
            .ok_or_else(|| "incomplete model parameter value".to_owned())?;
        let raw = raw.strip_prefix('\n').unwrap_or(raw);
        let raw = raw.strip_suffix('\n').unwrap_or(raw);
        let kind = schema
            .and_then(|p| p.get(key))
            .and_then(|p| p.get("type"))
            .and_then(Value::as_str);
        let value = if kind == Some("string") {
            Value::String(raw.to_owned())
        } else {
            let parsed = serde_json::from_str::<Value>(raw.trim());
            match (kind, parsed) {
                (None, _) => Value::String(raw.to_owned()),
                (Some("integer"), Ok(v)) if v.is_i64() || v.is_u64() => v,
                (Some("number"), Ok(v)) if v.is_number() => v,
                (Some("boolean"), Ok(v)) if v.is_boolean() => v,
                (Some("object"), Ok(v)) if v.is_object() => v,
                (Some("array"), Ok(v)) if v.is_array() => v,
                (Some("null"), Ok(v)) if v.is_null() => v,
                _ => {
                    return Err(format!(
                        "model parameter {key:?} does not match its declared type"
                    ))
                }
            }
        };
        arguments.insert(key.to_owned(), value);
        body = remaining.trim();
    }
    Ok(json!({"name":name,"arguments":arguments}))
}

fn find_byte_sequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn suffix_prefix_overlap(value: &[u8], marker: &[u8]) -> usize {
    let maximum = value.len().min(marker.len().saturating_sub(1));
    (1..=maximum)
        .rev()
        .find(|length| value[value.len() - length..] == marker[..*length])
        .unwrap_or(0)
}

#[cfg(test)]
fn parse_tool_calls(output: &str, id_prefix: &str) -> Result<Option<Vec<Value>>, String> {
    parse_tool_calls_selected(output, id_prefix, None, None)
}

fn parse_tool_calls_selected(
    output: &str,
    id_prefix: &str,
    profile: Option<&ModelProfile>,
    definitions: Option<&Vec<Value>>,
) -> Result<Option<Vec<Value>>, String> {
    let mut parser = selected_tool_parser(id_prefix.to_owned(), profile, definitions);
    let mut calls = Vec::new();
    for part in parser.push(output)?.into_iter().chain(parser.finish()?) {
        if let ToolStreamPart::Call { call, .. } = part {
            calls.push(call);
        }
    }
    Ok((!calls.is_empty()).then_some(calls))
}

fn normalize_tool_call(value: &Value, id_prefix: &str, index: usize) -> Value {
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let arguments = value.get("arguments").map_or_else(
        || "{}".to_owned(),
        |arguments| match arguments {
            Value::String(arguments) => arguments.clone(),
            arguments => arguments.to_string(),
        },
    );
    json!({
        "id": format!("call_{id_prefix}_{index}"),
        "type": "function",
        "function": {"name": name, "arguments": arguments}
    })
}

fn validate_openai_tool_history(body: &Value) -> Result<(), String> {
    let Some(messages) = body.get("messages").and_then(Value::as_array) else {
        return Ok(());
    };
    let mut pending = std::collections::HashMap::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| "each message requires a string role".to_owned())?;
        if role == "tool" {
            let id = message
                .get("tool_call_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or_else(|| "tool messages require a non-empty tool_call_id".to_owned())?;
            let expected_name = pending.remove(id).ok_or_else(|| {
                format!("tool_call_id {id:?} has no preceding assistant tool call")
            })?;
            if let Some(name) = message.get("name").and_then(Value::as_str) {
                if name != expected_name {
                    return Err(format!(
                        "tool result name {name:?} does not match tool call {expected_name:?}"
                    ));
                }
            }
            continue;
        }
        if !pending.is_empty() {
            return Err(
                "each assistant tool call must have a matching tool result before the next message"
                    .to_owned(),
            );
        }
        if role != "assistant" {
            continue;
        }
        let Some(calls) = message.get("tool_calls").filter(|calls| !calls.is_null()) else {
            continue;
        };
        let calls = calls
            .as_array()
            .ok_or_else(|| "assistant tool_calls must be an array".to_owned())?;
        for call in calls {
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or_else(|| "each assistant tool call requires a non-empty id".to_owned())?;
            let name = call
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| "each assistant tool call requires a function name".to_owned())?;
            if call.get("type").and_then(Value::as_str) != Some("function") {
                return Err("assistant tool calls must have type `function`".to_owned());
            }
            if pending.insert(id, name).is_some() {
                return Err(format!("duplicate tool_call_id {id:?}"));
            }
        }
    }
    if !pending.is_empty() {
        return Err("assistant tool calls are missing their tool results".to_owned());
    }
    Ok(())
}

fn build_chat_prompt(body: &Value) -> Result<String, String> {
    let parts = build_chat_content(body)?;
    Ok(flatten_content(&parts))
}

// Validate the complete protocol object together so incompatible fields cannot bypass checks.
#[allow(clippy::too_many_lines)]
fn build_chat_content(body: &Value) -> Result<Vec<ContentPart>, String> {
    let Some(messages) = body.get("messages") else {
        let prompt = body
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "messages is required".to_owned())?;
        return Ok(vec![ContentPart::Text(prompt)]);
    };
    let messages = messages
        .as_array()
        .ok_or_else(|| "messages must be an array".to_owned())?;
    if messages.is_empty() {
        return Err("messages must contain at least one message".to_owned());
    }

    let mut parts = Vec::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| "each message requires a string role".to_owned())?;
        if !matches!(role, "system" | "developer" | "user" | "assistant" | "tool") {
            return Err(format!("unsupported message role: {role}"));
        }
        if role == "assistant" {
            if let Some(calls) = message.get("tool_calls").filter(|calls| !calls.is_null()) {
                // Keep history structured until the adapter can select the
                // loaded model's tool format, just like tool instructions.
                let history = json!({"calls": calls, "content": message.get("content").and_then(Value::as_str).unwrap_or("")});
                push_text_part(
                    &mut parts,
                    &format!("\u{001e}__talvik_tool_calls\n{history}"),
                );
                continue;
            }
        }
        push_text_part(&mut parts, &format!("\u{001e}{role}\n"));
        if role == "assistant" {
            let content = message
                .get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| "assistant message requires content or tool_calls".to_owned())?;
            push_text_part(&mut parts, content);
        } else {
            let content = message
                .get("content")
                .ok_or_else(|| "message content is required".to_owned())?;
            if role == "tool" {
                let content = content
                    .as_str()
                    .ok_or_else(|| "tool message content must be a string".to_owned())?;
                let name = message
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool");
                let call_id = message
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .map_or_else(String::new, |id| format!(" (tool_call_id: {id})"));
                push_text_part(
                    &mut parts,
                    &format!("Tool result for {name}{call_id}: {content}"),
                );
            } else if let Some(content) = content.as_str() {
                push_text_part(&mut parts, content);
            } else if let Some(content) = content.as_array() {
                for part in content {
                    match part.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            let text =
                                part.get("text").and_then(Value::as_str).ok_or_else(|| {
                                    "text content parts require string text".to_owned()
                                })?;
                            push_text_part(&mut parts, text);
                        }
                        Some("image_url") if role == "user" => {
                            let image_url = part.get("image_url").ok_or_else(|| {
                                "image_url content parts require an image_url value".to_owned()
                            })?;
                            let url = image_url
                                .as_str()
                                .or_else(|| image_url.get("url").and_then(Value::as_str))
                                .ok_or_else(|| {
                                    "image_url must contain a data URL string".to_owned()
                                })?;
                            let image = decode_data_image(url)?;
                            push_image_part(&mut parts, image)?;
                        }
                        Some("image_url") => {
                            return Err(
                                "image inputs are only supported in user messages".to_owned()
                            )
                        }
                        Some(kind) => {
                            return Err(format!("unsupported chat content part type `{kind}`"))
                        }
                        None => return Err("chat content parts require a type".to_owned()),
                    }
                }
            } else {
                return Err("message content must be a string or content-part array".to_owned());
            }
        }
        if let Some(images) = message.get("images").filter(|images| !images.is_null()) {
            let images = images
                .as_array()
                .ok_or_else(|| "message images must be an array".to_owned())?;
            if role != "user" && !images.is_empty() {
                return Err("image inputs are only supported in user messages".to_owned());
            }
            for encoded in images {
                let encoded = encoded
                    .as_str()
                    .ok_or_else(|| "message images must contain base64 strings".to_owned())?;
                push_image_part(&mut parts, decode_ollama_image(encoded)?)?;
            }
        }
    }
    Ok(parts)
}

fn build_completion_prompt(body: &Value) -> Result<String, String> {
    body.get("prompt")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "prompt must be a string".to_owned())
}

fn build_response_prompt(body: &Value) -> Result<String, String> {
    let input = body
        .get("input")
        .and_then(Value::as_str)
        .ok_or_else(|| "input must be a string".to_owned())?;
    let instructions = body
        .get("instructions")
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "instructions must be a string".to_owned())
        })
        .transpose()?;
    let mut prompt = String::new();
    if let Some(instructions) = instructions {
        prompt.push_str("\u{001e}system\n");
        prompt.push_str(&instructions);
    }
    prompt.push_str("\u{001e}user\n");
    prompt.push_str(input);
    Ok(prompt)
}

fn parse_tool_choice(body: &Value) -> Result<OpenAiToolChoice, (u16, String)> {
    let Some(choice) = body.get("tool_choice") else {
        return Ok(OpenAiToolChoice::Auto);
    };
    match choice.as_str() {
        Some("auto") => Ok(OpenAiToolChoice::Auto),
        Some("none") => Ok(OpenAiToolChoice::None),
        Some("required") => Ok(OpenAiToolChoice::Required),
        Some(_) => Err((
            400,
            "`tool_choice` must be `auto`, `none`, `required`, or a function choice object"
                .to_owned(),
        )),
        None if choice.is_object() => {
            if choice.get("type").and_then(Value::as_str) != Some("function") {
                return Err((
                    400,
                    "function tool_choice must have type `function`".to_owned(),
                ));
            }
            let name = choice
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    (
                        400,
                        "function tool_choice requires a non-empty function name".to_owned(),
                    )
                })?;
            Ok(OpenAiToolChoice::Function(name.to_owned()))
        }
        None => Err((
            400,
            "`tool_choice` must be a string or function choice object".to_owned(),
        )),
    }
}

fn parse_chat_template_kwargs(body: &Value) -> Result<Option<bool>, (u16, String)> {
    let Some(kwargs) = body.get("chat_template_kwargs") else {
        return Ok(None);
    };
    let Some(kwargs) = kwargs.as_object() else {
        return Err((400, "`chat_template_kwargs` must be an object".to_owned()));
    };
    if let Some(field) = kwargs.keys().find(|key| key.as_str() != "enable_thinking") {
        return Err((
            501,
            format!("`chat_template_kwargs.{field}` is not supported yet"),
        ));
    }
    let Some(enabled) = kwargs.get("enable_thinking") else {
        return Ok(None);
    };
    enabled.as_bool().map(Some).ok_or_else(|| {
        (
            400,
            "`chat_template_kwargs.enable_thinking` must be a boolean".to_owned(),
        )
    })
}

fn validate_tool_definitions(body: &Value) -> Result<Vec<String>, (u16, String)> {
    let Some(tools) = body.get("tools") else {
        return Ok(Vec::new());
    };
    let Some(tools) = tools.as_array() else {
        return Err((400, "tools must be an array".to_owned()));
    };
    let mut names = Vec::with_capacity(tools.len());
    for (index, tool) in tools.iter().enumerate() {
        if tool.get("type").and_then(Value::as_str) != Some("function") {
            return Err((400, format!("tools[{index}].type must be `function`")));
        }
        let Some(function) = tool.get("function").and_then(Value::as_object) else {
            return Err((400, format!("tools[{index}].function must be an object")));
        };
        let Some(name) = function.get("name").and_then(Value::as_str) else {
            return Err((
                400,
                format!("tools[{index}].function.name must be a string"),
            ));
        };
        if name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err((
                400,
                format!("tools[{index}].function.name has an invalid format"),
            ));
        }
        if names.iter().any(|existing| existing == name) {
            return Err((400, format!("function tool name {name:?} is duplicated")));
        }
        if function
            .get("parameters")
            .is_some_and(|parameters| !parameters.is_object())
        {
            return Err((
                400,
                format!("tools[{index}].function.parameters must be an object"),
            ));
        }
        names.push(name.to_owned());
    }
    Ok(names)
}

fn validate_generated_tool_calls(
    choice: &OpenAiToolChoice,
    calls: Option<&[Value]>,
    parallel_tool_calls: bool,
) -> Result<(), String> {
    if !parallel_tool_calls && calls.is_some_and(|calls| calls.len() > 1) {
        return Err(
            "model returned multiple tool calls while parallel calls were disabled".to_owned(),
        );
    }
    match choice {
        OpenAiToolChoice::Auto | OpenAiToolChoice::None => Ok(()),
        OpenAiToolChoice::Required if calls.is_some_and(|calls| !calls.is_empty()) => Ok(()),
        OpenAiToolChoice::Required => {
            Err("model did not produce the required tool call".to_owned())
        }
        OpenAiToolChoice::Function(name) => {
            let Some(calls) = calls.filter(|calls| !calls.is_empty()) else {
                return Err(format!("model did not call the required function {name:?}"));
            };
            if calls.len() != 1 {
                return Err(format!(
                    "model returned multiple calls for the required function {name:?}"
                ));
            }
            if calls[0].pointer("/function/name").and_then(Value::as_str) == Some(name) {
                Ok(())
            } else {
                Err(format!(
                    "model returned a tool call other than the required function {name:?}"
                ))
            }
        }
    }
}

fn tool_call_output_error_code(message: &str) -> &'static str {
    if message.contains("incomplete tool call") || message.contains("incomplete tool-call marker") {
        "incomplete_tool_call"
    } else if message == "model returned an undeclared tool name" {
        "undeclared_tool_name"
    } else if message.contains("parallel calls were disabled") {
        "parallel_tool_calls_not_allowed"
    } else if message.contains("required tool call") || message.contains("required function") {
        "tool_choice_not_satisfied"
    } else {
        "invalid_tool_call"
    }
}

fn parse_openai_response_format(
    value: Option<&Value>,
) -> Result<Option<StructuredOutput>, (u16, String)> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(format) = value.as_object() else {
        return Err((400, "`response_format` must be an object".to_owned()));
    };
    let Some(kind) = format.get("type").and_then(Value::as_str) else {
        return Err((400, "`response_format.type` must be a string".to_owned()));
    };
    let output = match kind {
        "json_object" => {
            if format
                .keys()
                .any(|key| !matches!(key.as_str(), "type" | "description"))
            {
                return Err((
                    400,
                    "`json_object` contains an unsupported field".to_owned(),
                ));
            }
            if format
                .get("description")
                .is_some_and(|description| !description.is_string())
            {
                return Err((
                    400,
                    "`response_format.description` must be a string".to_owned(),
                ));
            }
            StructuredOutput::JsonObject
        }
        "json_schema" => {
            if format
                .keys()
                .any(|key| !matches!(key.as_str(), "type" | "json_schema"))
            {
                return Err((
                    400,
                    "`json_schema` response_format contains an unsupported field".to_owned(),
                ));
            }
            let Some(spec) = format.get("json_schema").and_then(Value::as_object) else {
                return Err((
                    400,
                    "`response_format.json_schema` must be an object".to_owned(),
                ));
            };
            if spec
                .keys()
                .any(|key| !matches!(key.as_str(), "name" | "description" | "strict" | "schema"))
            {
                return Err((
                    400,
                    "`response_format.json_schema` contains an unsupported field".to_owned(),
                ));
            }
            if spec
                .get("name")
                .and_then(Value::as_str)
                .is_none_or(|name| name.trim().is_empty())
            {
                return Err((
                    400,
                    "`response_format.json_schema.name` must be a non-empty string".to_owned(),
                ));
            }
            if spec
                .get("description")
                .is_some_and(|description| !description.is_string())
            {
                return Err((
                    400,
                    "`response_format.json_schema.description` must be a string".to_owned(),
                ));
            }
            if spec.get("strict").and_then(Value::as_bool) != Some(true) {
                return Err((
                    501,
                    "only strict JSON Schema response formats are supported".to_owned(),
                ));
            }
            let Some(schema) = spec.get("schema") else {
                return Err((
                    400,
                    "`response_format.json_schema.schema` is required".to_owned(),
                ));
            };
            StructuredOutput::JsonSchema(schema.clone())
        }
        other => {
            return Err((501, format!("response format `{other}` is not supported")));
        }
    };
    output.validate().map_err(|error| {
        (
            400,
            format!("unsupported structured output schema: {error}"),
        )
    })?;
    Ok(Some(output))
}

fn parse_ollama_format(value: Option<&Value>) -> Result<Option<StructuredOutput>, (u16, String)> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let output = match value {
        Value::String(format) if format == "json" => StructuredOutput::JsonObject,
        Value::String(format) => {
            return Err((400, format!("Ollama format `{format}` is not supported")));
        }
        Value::Object(_) => StructuredOutput::JsonSchema(value.clone()),
        _ => {
            return Err((
                400,
                "`format` must be `json` or a JSON Schema object".to_owned(),
            ));
        }
    };
    output.validate().map_err(|error| {
        (
            400,
            format!("unsupported structured output schema: {error}"),
        )
    })?;
    Ok(Some(output))
}

// Validate the complete protocol object together so incompatible fields cannot bypass checks.
#[allow(clippy::too_many_lines)]
fn validate_generation_request(
    body: &Value,
    endpoint: OpenAiGenerationEndpoint,
) -> Result<(), (u16, String)> {
    for field in [
        "logprobs",
        "top_logprobs",
        "presence_penalty",
        "frequency_penalty",
        "logit_bias",
    ] {
        if body.get(field).is_some() {
            return Err((
                501,
                format!("request parameter `{field}` is not supported yet"),
            ));
        }
        if body.get("response_format").is_some() {
            if endpoint != OpenAiGenerationEndpoint::ChatCompletions {
                return Err((
                    501,
                    "`response_format` is only supported for Chat Completions".to_owned(),
                ));
            }
            parse_openai_response_format(body.get("response_format"))?;
        }
    }
    if let Some(options) = body.get("stream_options").filter(|value| !value.is_null()) {
        let Some(options) = options.as_object() else {
            return Err((400, "`stream_options` must be an object".to_owned()));
        };
        if let Some(field) = options.keys().find(|key| key.as_str() != "include_usage") {
            return Err((
                501,
                format!("`stream_options.{field}` is not supported yet"),
            ));
        }
        if options
            .get("include_usage")
            .is_some_and(|value| !value.is_boolean())
        {
            return Err((
                400,
                "`stream_options.include_usage` must be a boolean".to_owned(),
            ));
        }
        if !body.get("stream").and_then(Value::as_bool).unwrap_or(false) {
            return Err((
                400,
                "`stream_options` is only valid when `stream` is true".to_owned(),
            ));
        }
    }
    if body.get("chat_template_kwargs").is_some() {
        if endpoint != OpenAiGenerationEndpoint::ChatCompletions {
            return Err((
                501,
                "`chat_template_kwargs` is only supported for Chat Completions".to_owned(),
            ));
        }
        parse_chat_template_kwargs(body)?;
    }
    if let Some(value) = body.get("n") {
        match value.as_u64() {
            Some(1) => {}
            Some(_) => {
                return Err((
                    501,
                    "only one completion per request is supported".to_owned(),
                ));
            }
            None => return Err((400, "n must be a positive integer".to_owned())),
        }
    }
    if body.get("tools").is_some() && !body.get("tools").is_some_and(Value::is_array) {
        return Err((400, "tools must be an array".to_owned()));
    }
    if body.get("tool_choice").is_some() && endpoint != OpenAiGenerationEndpoint::ChatCompletions {
        return Err((
            501,
            "`tool_choice` is only supported for Chat Completions".to_owned(),
        ));
    }
    if let Some(value) = body.get("parallel_tool_calls") {
        if endpoint != OpenAiGenerationEndpoint::ChatCompletions {
            return Err((
                501,
                "`parallel_tool_calls` is only supported for Chat Completions".to_owned(),
            ));
        }
        if !value.is_boolean() {
            return Err((400, "`parallel_tool_calls` must be a boolean".to_owned()));
        }
    }
    if endpoint == OpenAiGenerationEndpoint::ChatCompletions {
        let names = validate_tool_definitions(body)?;
        match parse_tool_choice(body)? {
            OpenAiToolChoice::Auto if body.get("tool_choice").is_some() && names.is_empty() => {
                return Err((
                    400,
                    "`tool_choice: auto` requires a non-empty tools array".to_owned(),
                ));
            }
            OpenAiToolChoice::Required | OpenAiToolChoice::Function(_) if names.is_empty() => {
                return Err((
                    400,
                    "`tool_choice: required` requires a non-empty tools array".to_owned(),
                ));
            }
            OpenAiToolChoice::Function(name)
                if !names.iter().any(|candidate| candidate == &name) =>
            {
                return Err((
                    400,
                    format!("function tool_choice names unknown tool {name:?}"),
                ));
            }
            _ => {}
        }
    }
    if endpoint == OpenAiGenerationEndpoint::Completions {
        if body.get("messages").is_some() {
            return Err((
                400,
                "messages is not valid for the Completions endpoint; use prompt".to_owned(),
            ));
        }
        if body.get("prompt").is_some_and(|prompt| !prompt.is_string()) {
            return Err((
                501,
                "only string prompts are supported by the Completions endpoint".to_owned(),
            ));
        }
        if body.get("tools").is_some() {
            return Err((
                501,
                "tools are not supported by the legacy Completions endpoint".to_owned(),
            ));
        }
    }
    if endpoint == OpenAiGenerationEndpoint::Responses {
        if body.get("input").is_some_and(|input| !input.is_string()) {
            return Err((
                501,
                "only string input is supported by Responses".to_owned(),
            ));
        }
        if body.get("messages").is_some() || body.get("prompt").is_some() {
            return Err((
                400,
                "Responses requires `input`; `messages` and `prompt` are not accepted".to_owned(),
            ));
        }
        if body.get("tools").is_some() {
            return Err((501, "tools are not supported by Responses yet".to_owned()));
        }
    }
    if endpoint == OpenAiGenerationEndpoint::ChatCompletions
        && body.get("prompt").is_some()
        && body.get("messages").is_some()
    {
        return Err((
            400,
            "provide messages or the Talvik-specific prompt extension, not both".to_owned(),
        ));
    }
    for field in ["max_tokens", "max_completion_tokens", "max_output_tokens"] {
        if let Some(value) = body.get(field) {
            if !value
                .as_u64()
                .is_some_and(|tokens| tokens > 0 && u32::try_from(tokens).is_ok())
            {
                return Err((400, format!("{field} must be a positive integer")));
            }
        }
    }
    for (field, minimum, maximum) in [("temperature", 0.0, 2.0), ("top_p", 0.0, 1.0)] {
        if let Some(value) = body.get(field) {
            let Some(number) = value.as_f64() else {
                return Err((400, format!("{field} must be a number")));
            };
            if !(minimum..=maximum).contains(&number) {
                return Err((
                    400,
                    format!("{field} must be between {minimum} and {maximum}"),
                ));
            }
        }
    }
    if let Some(value) = body.get("stop") {
        let valid = value.is_string()
            || value
                .as_array()
                .is_some_and(|values| values.iter().all(Value::is_string));
        if !valid {
            return Err((
                400,
                "stop must be a string or an array of strings".to_owned(),
            ));
        }
    }
    if let Some(value) = body.get("stream") {
        if !value.is_boolean() {
            return Err((400, "stream must be a boolean".to_owned()));
        }
    }
    if let Some(value) = body.get("seed") {
        if value
            .as_u64()
            .is_none_or(|seed| u32::try_from(seed).is_err())
        {
            return Err((400, "seed must be an unsigned 32-bit integer".to_owned()));
        }
    }
    if let Some(value) = body.get("top_k") {
        if !value
            .as_i64()
            .is_some_and(|top_k| top_k > 0 && top_k <= i64::from(i32::MAX))
        {
            return Err((400, "top_k must be a positive 32-bit integer".to_owned()));
        }
    }
    if let Some(value) = body.get("context_size") {
        if !value
            .as_u64()
            .is_some_and(|context| context > 0 && context <= MAX_CONTEXT_TOKENS)
        {
            return Err((
                400,
                format!("context_size must be between 1 and {MAX_CONTEXT_TOKENS}"),
            ));
        }
    }
    if let Some(value) = body.get("timeout_ms") {
        if !value
            .as_u64()
            .is_some_and(|timeout| timeout > 0 && timeout <= 300_000)
        {
            return Err((400, "timeout_ms must be between 1 and 300000".to_owned()));
        }
    }
    Ok(())
}

fn generation_defaults(
    body: &Value,
    context_default: Option<&str>,
    thinking_default: Option<&str>,
) -> Result<(u32, Option<bool>), (u16, String)> {
    let context = if let Some(value) = body.get("context_size") {
        value
            .as_u64()
            .ok_or_else(|| (400, "context_size must be an integer".to_owned()))?
    } else {
        context_default
            .unwrap_or("4096")
            .parse::<u64>()
            .map_err(|_| {
                (
                    500,
                    "TALVIK_HTTP_CONTEXT_SIZE must be an integer".to_owned(),
                )
            })?
    };
    if context == 0 || context > MAX_CONTEXT_TOKENS {
        return Err((
            400,
            format!("context_size must be between 1 and {MAX_CONTEXT_TOKENS}"),
        ));
    }
    let thinking = match parse_chat_template_kwargs(body)? {
        Some(value) => Some(value),
        None => thinking_default
            .map(str::parse::<bool>)
            .transpose()
            .map_err(|_| {
                (
                    500,
                    "TALVIK_ENABLE_THINKING must be true or false".to_owned(),
                )
            })?,
    };
    Ok((u32::try_from(context).unwrap_or(u32::MAX), thinking))
}

#[allow(clippy::too_many_lines)]
fn generate(
    stream: &mut TcpStream,
    runtime: &dyn RuntimeService,
    body: &Value,
    identity: &auth::TokenIdentity,
    owners: &Arc<Mutex<HashMap<String, String>>>,
    endpoint: OpenAiGenerationEndpoint,
) -> std::io::Result<()> {
    let Some(model) = body.get("model").and_then(Value::as_str) else {
        return respond_error(stream, 400, "model is required".to_owned());
    };
    if let Err((status, message)) = validate_generation_request(body, endpoint) {
        return respond_error(stream, status, message);
    }
    let structured_output = match parse_openai_response_format(body.get("response_format")) {
        Ok(output) => output,
        Err((status, message)) => return respond_error(stream, status, message),
    };
    let (context_size, enable_thinking) = match generation_defaults(
        body,
        std::env::var("TALVIK_HTTP_CONTEXT_SIZE").ok().as_deref(),
        std::env::var("TALVIK_ENABLE_THINKING").ok().as_deref(),
    ) {
        Ok(value) => value,
        Err((status, message)) => return respond_error(stream, status, message),
    };
    if structured_output.is_some()
        && body
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| !tools.is_empty())
    {
        return respond_error(
            stream,
            400,
            "structured output cannot be combined with tools".to_owned(),
        );
    }
    if endpoint == OpenAiGenerationEndpoint::ChatCompletions {
        if let Err(message) = validate_openai_tool_history(body) {
            return respond_error(stream, 400, message);
        }
    }
    let tool_choice = if endpoint == OpenAiGenerationEndpoint::ChatCompletions {
        match parse_tool_choice(body) {
            Ok(choice) => choice,
            Err((status, message)) => return respond_error(stream, status, message),
        }
    } else {
        OpenAiToolChoice::Auto
    };
    let tool_definitions = body
        .get("tools")
        .and_then(Value::as_array)
        .filter(|tools| !tools.is_empty())
        .filter(|_| tool_choice != OpenAiToolChoice::None);
    let parallel_tool_calls = body
        .get("parallel_tool_calls")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let (prompt_result, mut request_content) = match endpoint {
        OpenAiGenerationEndpoint::ChatCompletions => match build_chat_content(body) {
            Ok(content) => (Ok(flatten_content(&content)), content),
            Err(error) => (Err(error), Vec::new()),
        },
        OpenAiGenerationEndpoint::Completions => (build_completion_prompt(body), Vec::new()),
        OpenAiGenerationEndpoint::Responses => (build_response_prompt(body), Vec::new()),
    };
    let mut prompt = match prompt_result {
        Ok(prompt) => prompt,
        Err(message) => return respond_error(stream, 400, message),
    };
    let base_prompt = prompt.clone();
    if let Some(enabled) =
        enable_thinking.filter(|_| endpoint == OpenAiGenerationEndpoint::ChatCompletions)
    {
        prompt.insert_str(
            0,
            &format!("\u{001e}__talvik_template\nenable_thinking={enabled}"),
        );
    }
    if endpoint == OpenAiGenerationEndpoint::ChatCompletions {
        if let Some(tools) = tool_definitions {
            prompt.insert_str(
                0,
                &format!("\u{001e}__talvik_tools\n{}", Value::Array(tools.clone())),
            );
            let choice_instruction = match &tool_choice {
                OpenAiToolChoice::Auto | OpenAiToolChoice::None => None,
                OpenAiToolChoice::Required => Some("required".to_owned()),
                OpenAiToolChoice::Function(name) => Some(format!("function:{name}")),
            };
            if let Some(instruction) = choice_instruction {
                prompt.insert_str(0, &format!("\u{001e}__talvik_tool_choice\n{instruction}"));
            }
            if !parallel_tool_calls {
                prompt.insert_str(0, "\u{001e}__talvik_tool_choice\nsingle");
            }
        }
    } else if tool_definitions.is_some() {
        return respond_error(
            stream,
            501,
            "tools are not supported by the legacy Completions endpoint".to_owned(),
        );
    }
    if request_content
        .iter()
        .any(|part| matches!(part, ContentPart::Image(_)))
    {
        let prefix = prompt
            .strip_suffix(&base_prompt)
            .unwrap_or_default()
            .to_owned();
        prepend_text_part(&mut request_content, &prefix);
    } else {
        request_content.clear();
    }
    let request_id = body
        .get("request_id")
        .and_then(Value::as_str)
        .map_or_else(unique_request_id, str::to_owned);
    let timeout = body
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .map(|milliseconds| Duration::from_millis(milliseconds.min(300_000)));
    let options = generation_options(body, context_size);
    let response_temperature = options.temperature;
    let response_top_p = options.top_p;
    let created = match openai_timestamp() {
        Ok(created) => created,
        Err(error) => return respond_error(stream, 500, error.to_string()),
    };
    let was_resident = runtime
        .model_inspect(model)
        .map(|metadata| matches!(metadata.status, oid_runtime::ModelStatus::Loaded));
    let load_started = Instant::now();
    if let Err(error) = runtime.model_load(model) {
        return respond_error(stream, runtime_error_status(&error), error.to_string());
    }
    let model_load_time_ms = was_resident
        .filter(|resident| !resident)
        .map(|_| load_started.elapsed().as_millis());
    eprintln!(
        "Talvik HTTP generation started request_id={} model={} stream={} tools={} max_tokens={} context_size={} temperature={} top_p={} top_k={} seed={}",
        request_id,
        model,
        body.get("stream").and_then(Value::as_bool).unwrap_or(false),
        tool_definitions.is_some(),
        options.max_tokens,
        options.context_size,
        options.temperature,
        options.top_p,
        options.top_k,
        options.seed
    );
    let output_profile = runtime.model_profile(model);
    if let Some(profile) = output_profile.as_ref() {
        if !profile.completion
            || (endpoint == OpenAiGenerationEndpoint::ChatCompletions && !profile.chat)
        {
            return respond_error(
                stream,
                501,
                "the loaded model does not support this text generation endpoint".to_owned(),
            );
        }
        if body
            .get("chat_template_kwargs")
            .is_some_and(|kwargs| kwargs.get("enable_thinking").is_some())
            && !profile.thinking_control
        {
            return respond_error(
                stream,
                501,
                "the loaded template has no supported thinking control".to_owned(),
            );
        }
        if request_content
            .iter()
            .any(|p| matches!(p, ContentPart::Image(_)))
            && !profile.vision
        {
            return respond_error(
                stream,
                501,
                "the loaded model has no initialized vision projector".to_owned(),
            );
        }
    }
    if tool_definitions.is_some() {
        let support = runtime.tool_support(model);
        if !support.supported {
            return respond_error(
                stream,
                501,
                format!("tool calls are unavailable: {}", support.reason),
            );
        }
    }
    if !reserve_owner(owners, &request_id, &identity.principal) {
        return respond_error(
            stream,
            409,
            "request_id is already in use or ownership registry unavailable".to_owned(),
        );
    }
    if std::env::var("TALVIK_TRACE_PREFILL")
        .is_ok_and(|value| matches!(value.as_str(), "1" | "true"))
    {
        let messages = body.get("messages").and_then(Value::as_array);
        let roles = messages.into_iter().flatten().map(|message| json!({"role": message.get("role"), "content_bytes": message.get("content").map_or(0, |content| content.to_string().len())})).collect::<Vec<_>>();
        let tool_bytes = tool_definitions.map_or(0, |tools| {
            serde_json::to_vec(tools).map_or(0, |value| value.len())
        });
        eprintln!("Talvik HTTP request_profile request_id={request_id} prompt_bytes={} tool_count={} tool_schema_bytes={tool_bytes} messages={}", prompt.len(), tool_definitions.map_or(0, Vec::len), json!(roles));
    }
    let generation = match runtime.generate(GenerationRequest {
        request_id: request_id.clone(),
        model_id: model.to_owned(),
        prompt,
        content: request_content,
        options,
        structured_output: structured_output.clone(),
    }) {
        Ok(stream_result) => stream_result,
        Err(error) => {
            finish_owner(owners, &request_id);
            return respond_error(stream, runtime_error_status(&error), error.to_string());
        }
    };
    let _lease = HttpGenerationLease {
        generation: &generation,
        owners,
        request_id: &request_id,
    };
    // Bound dead/slow clients as well as the backend wait. The request body is
    // fully consumed before these settings are applied.
    stream.set_read_timeout(Some(Duration::from_millis(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
    // Admit and validate before starting SSE, then acknowledge chat requests
    // immediately. Tool output still passes the bounded parser and validation;
    // delaying the HTTP headers until output made CPU prefill look hung to IDEs.
    let defer_stream_headers = streaming
        && tool_definitions.is_some()
        && endpoint != OpenAiGenerationEndpoint::ChatCompletions;
    let stream_tool_calls = streaming
        && endpoint == OpenAiGenerationEndpoint::ChatCompletions
        && tool_definitions.is_some();
    let mut tool_parser = stream_tool_calls.then(|| {
        selected_tool_parser(
            request_id.clone(),
            output_profile.as_ref(),
            tool_definitions,
        )
    });
    let mut streamed_tool_calls = Vec::new();
    let mut stream_headers_sent = false;
    let mut response_sequence_number = 0;
    if streaming && !defer_stream_headers {
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n"
        )?;
        stream.flush()?;
        stream_headers_sent = true;
        if endpoint == OpenAiGenerationEndpoint::ChatCompletions {
            write_chat_stream_role(stream, &request_id, created, model)?;
        }
        if endpoint == OpenAiGenerationEndpoint::Responses {
            if let Err(error) = write_responses_stream_start(
                stream,
                &mut response_sequence_number,
                &request_id,
                created,
                model,
                body,
                response_temperature,
                response_top_p,
            ) {
                generation.cancel();
                if !drain_cancelled_generation(&generation) {
                    eprintln!("Talvik HTTP generation cleanup timed out request_id={request_id}");
                }
                return Err(error);
            }
        }
    }
    let mut reason_parser =
        if endpoint == OpenAiGenerationEndpoint::ChatCompletions && structured_output.is_none() {
            reasoning_parser(output_profile.as_ref(), enable_thinking)
        } else {
            None
        };
    let mut reasoning = String::new();
    let mut pending_completion = None;
    let mut output = String::new();
    let deadline = timeout.map(|duration| Instant::now() + duration);
    let mut last_keepalive = Instant::now();
    loop {
        if peer_disconnected(stream)? {
            generation.cancel();
            if !drain_cancelled_generation(&generation) {
                eprintln!("Talvik HTTP disconnect cleanup timed out request_id={request_id}");
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "inference client disconnected",
            ));
        }
        let wait = deadline.map_or(Duration::from_millis(100), |end| {
            end.saturating_duration_since(Instant::now())
                .min(Duration::from_millis(100))
        });
        let message = pending_completion.take().map_or_else(
            || generation.recv_timeout(wait),
            |result| Ok(GenerationMessage::Completed(result)),
        );
        if matches!(message, Err(std::sync::mpsc::RecvTimeoutError::Timeout))
            && deadline.is_none_or(|end| Instant::now() < end)
        {
            if streaming
                && stream_headers_sent
                && last_keepalive.elapsed() >= Duration::from_secs(5)
            {
                // SSE comments are ignored by OpenAI clients. They keep a long
                // prefill connection live without inventing content or usage.
                stream.write_all(b": keep-alive\n\n")?;
                stream.flush()?;
                last_keepalive = Instant::now();
            }
            continue;
        }
        let (message, thought) =
            split_reasoning_message(message, &mut reason_parser, &mut pending_completion);
        reasoning.push_str(&thought);
        if streaming && !thought.is_empty() {
            if let Err(error) = write_openai_reasoning(
                stream,
                &request_id,
                created,
                model,
                &thought,
                &mut stream_headers_sent,
            ) {
                generation.cancel();
                let _ = drain_cancelled_generation(&generation);
                return Err(error);
            }
        }
        match message {
            Ok(GenerationMessage::Token(token)) => {
                if token.is_empty() {
                    continue;
                }
                if let Some(parser) = tool_parser.as_mut() {
                    let parts = match parser.push(&token) {
                        Ok(parts) => parts,
                        Err(message) => {
                            generation.cancel();
                            if !drain_cancelled_generation(&generation) {
                                eprintln!(
                                    "Talvik HTTP generation cleanup timed out request_id={request_id}"
                                );
                            }
                            return write_openai_stream_error(
                                stream,
                                stream_headers_sent,
                                "invalid_model_output",
                                &message,
                            );
                        }
                    };
                    for part in parts {
                        if let ToolStreamPart::Call { call, .. } = &part {
                            let validation = tool_definitions
                                .filter(|definitions| {
                                    generated_tool_call_is_declared(call, definitions)
                                })
                                .ok_or_else(|| "model returned an undeclared tool name".to_owned())
                                .and_then(|_| {
                                    validate_streamed_tool_call(
                                        call,
                                        &streamed_tool_calls,
                                        &tool_choice,
                                        parallel_tool_calls,
                                    )
                                });
                            if let Err(message) = validation {
                                generation.cancel();
                                if !drain_cancelled_generation(&generation) {
                                    eprintln!(
                                        "Talvik HTTP generation cleanup timed out request_id={request_id}"
                                    );
                                }
                                return write_openai_stream_error(
                                    stream,
                                    stream_headers_sent,
                                    tool_call_output_error_code(&message),
                                    &message,
                                );
                            }
                            streamed_tool_calls.push(call.clone());
                        }
                        if let Err(error) = write_openai_tool_stream_part(
                            stream,
                            &request_id,
                            created,
                            model,
                            &part,
                            &mut stream_headers_sent,
                        ) {
                            generation.cancel();
                            if !drain_cancelled_generation(&generation) {
                                eprintln!(
                                    "Talvik HTTP generation cleanup timed out request_id={request_id}"
                                );
                            }
                            return Err(error);
                        }
                    }
                } else {
                    output.push_str(&token);
                }
                if streaming && tool_definitions.is_none() {
                    let event = match endpoint {
                        OpenAiGenerationEndpoint::ChatCompletions => json!({
                            "id": request_id.clone(),
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": model,
                            "choices": [{"index": 0, "delta": {"content": token}, "finish_reason": Value::Null}]
                        }),
                        OpenAiGenerationEndpoint::Completions => json!({
                            "id": request_id.clone(),
                            "object": "text_completion",
                            "created": created,
                            "model": model,
                            "choices": [{"index": 0, "text": token, "logprobs": Value::Null, "finish_reason": Value::Null}]
                        }),
                        OpenAiGenerationEndpoint::Responses => {
                            if let Err(error) = write_responses_sse_event(
                                stream,
                                &mut response_sequence_number,
                                "response.output_text.delta",
                                json!({
                                    "item_id": format!("msg_{request_id}"),
                                    "output_index": 0,
                                    "content_index": 0,
                                    "delta": token
                                }),
                            ) {
                                generation.cancel();
                                if !drain_cancelled_generation(&generation) {
                                    eprintln!(
                                        "Talvik HTTP generation cleanup timed out request_id={request_id}"
                                    );
                                }
                                return Err(error);
                            }
                            continue;
                        }
                    };
                    write!(stream, "data: {event}\n\n")?;
                    stream.flush()?;
                }
            }
            Ok(GenerationMessage::Completed(result)) => {
                eprintln!(
                    "Talvik HTTP generation completed request_id={} model={} termination={:?} generated_tokens={}",
                    request_id,
                    model,
                    result.statistics.termination,
                    result.statistics.generated_tokens
                );
                if let Some(structured_output) = structured_output.as_ref() {
                    if let Err(message) = structured_output.validate_output(&output) {
                        if streaming {
                            return write_openai_stream_error(
                                stream,
                                stream_headers_sent,
                                tool_call_output_error_code(&message),
                                &message,
                            );
                        }
                        return respond_model_output_error(
                            stream,
                            "invalid_model_output",
                            &message,
                        );
                    }
                }
                let finish_reason = completion_finish_reason(result.statistics.termination);
                let termination_reason =
                    completion_termination_reason(result.statistics.termination);
                let tool_calls = if endpoint == OpenAiGenerationEndpoint::ChatCompletions
                    && tool_definitions.is_some()
                {
                    if let Some(parser) = tool_parser.as_mut() {
                        let parts = match parser.finish() {
                            Ok(parts) => parts,
                            Err(message) => {
                                return write_openai_stream_error(
                                    stream,
                                    stream_headers_sent,
                                    tool_call_output_error_code(&message),
                                    &message,
                                );
                            }
                        };
                        for part in parts {
                            if let ToolStreamPart::Call { call, .. } = &part {
                                let validation = tool_definitions
                                    .filter(|definitions| {
                                        generated_tool_call_is_declared(call, definitions)
                                    })
                                    .ok_or_else(|| {
                                        "model returned an undeclared tool name".to_owned()
                                    })
                                    .and_then(|_| {
                                        validate_streamed_tool_call(
                                            call,
                                            &streamed_tool_calls,
                                            &tool_choice,
                                            parallel_tool_calls,
                                        )
                                    });
                                if let Err(message) = validation {
                                    return write_openai_stream_error(
                                        stream,
                                        stream_headers_sent,
                                        tool_call_output_error_code(&message),
                                        &message,
                                    );
                                }
                                streamed_tool_calls.push(call.clone());
                            }
                            write_openai_tool_stream_part(
                                stream,
                                &request_id,
                                created,
                                model,
                                &part,
                                &mut stream_headers_sent,
                            )?;
                        }
                        (!streamed_tool_calls.is_empty()).then(|| streamed_tool_calls.clone())
                    } else {
                        match parse_tool_calls_selected(
                            &output,
                            &unique_request_id(),
                            output_profile.as_ref(),
                            tool_definitions,
                        ) {
                            Ok(calls) => calls,
                            Err(message) => {
                                return respond_model_output_error(
                                    stream,
                                    tool_call_output_error_code(&message),
                                    &message,
                                );
                            }
                        }
                    }
                } else {
                    None
                };
                if tool_calls.as_ref().is_some_and(|calls| {
                    !tool_definitions.is_some_and(|definitions| {
                        calls
                            .iter()
                            .all(|call| generated_tool_call_is_declared(call, definitions))
                    })
                }) {
                    let message = "model returned an undeclared tool name";
                    return if streaming {
                        write_openai_stream_error(
                            stream,
                            stream_headers_sent,
                            "undeclared_tool_name",
                            message,
                        )
                    } else {
                        respond_model_output_error(stream, "undeclared_tool_name", message)
                    };
                }
                if let Err(message) = validate_generated_tool_calls(
                    &tool_choice,
                    tool_calls.as_deref(),
                    parallel_tool_calls,
                ) {
                    return if streaming {
                        write_openai_stream_error(
                            stream,
                            stream_headers_sent,
                            tool_call_output_error_code(&message),
                            &message,
                        )
                    } else {
                        respond_model_output_error(
                            stream,
                            tool_call_output_error_code(&message),
                            &message,
                        )
                    };
                }
                if endpoint == OpenAiGenerationEndpoint::Responses {
                    let usage = result.statistics.metrics_known.then(|| {
                        json!({
                            "input_tokens": result.statistics.prompt_tokens,
                            "input_tokens_details": {
                                "cached_tokens": result.statistics.reused_prompt_tokens
                            },
                            "output_tokens": result.statistics.generated_tokens,
                            "output_tokens_details": {"reasoning_tokens": 0},
                            "total_tokens": result.statistics.prompt_tokens.saturating_add(result.statistics.generated_tokens)
                        })
                    });
                    let output_message =
                        responses_output_message(&result.request_id, "completed", &output);
                    let response = responses_object(
                        &result.request_id,
                        created,
                        model,
                        body,
                        response_temperature,
                        response_top_p,
                        "completed",
                        vec![output_message.clone()],
                        usage.unwrap_or(Value::Null),
                        Value::Null,
                        Value::Null,
                    );
                    if streaming {
                        let message_id = format!("msg_{}", result.request_id);
                        write_responses_sse_event(
                            stream,
                            &mut response_sequence_number,
                            "response.output_text.done",
                            json!({
                                "item_id": message_id,
                                "output_index": 0,
                                "content_index": 0,
                                "text": output
                            }),
                        )?;
                        write_responses_sse_event(
                            stream,
                            &mut response_sequence_number,
                            "response.content_part.done",
                            json!({
                                "item_id": message_id,
                                "output_index": 0,
                                "content_index": 0,
                                "part": {
                                    "type": "output_text",
                                    "text": output,
                                    "annotations": []
                                }
                            }),
                        )?;
                        write_responses_sse_event(
                            stream,
                            &mut response_sequence_number,
                            "response.output_item.done",
                            json!({"output_index": 0, "item": output_message}),
                        )?;
                        write_responses_sse_event(
                            stream,
                            &mut response_sequence_number,
                            "response.completed",
                            json!({"response": response}),
                        )?;
                        return Ok(());
                    }
                    return respond_json(stream, 200, response);
                }
                if streaming {
                    if defer_stream_headers && !stream_headers_sent {
                        write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n"
                        )?;
                        stream.flush()?;
                    }
                    if let Some(tool_calls) = tool_calls.as_ref().filter(|_| tool_parser.is_none())
                    {
                        let event = json!({
                            "id": result.request_id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": model,
                            "choices": [{"index": 0, "delta": {"tool_calls": tool_calls}, "finish_reason": "tool_calls"}]
                        });
                        write!(stream, "data: {event}\n\n")?;
                    } else if endpoint == OpenAiGenerationEndpoint::Completions {
                        let event = json!({
                            "id": result.request_id,
                            "object": "text_completion",
                            "created": created,
                            "model": model,
                            "choices": [{"index": 0, "text": "", "logprobs": Value::Null, "finish_reason": finish_reason}]
                        });
                        write!(stream, "data: {event}\n\n")?;
                    } else {
                        let event = json!({
                            "id": result.request_id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": model,
                            "choices": [{
                                "index": 0,
                                "delta": {},
                                "finish_reason": if tool_calls.is_some() {"tool_calls"} else {finish_reason}
                            }],
                            "termination_reason": termination_reason
                        });
                        let event = if tool_definitions.is_some() && !output.is_empty() {
                            json!({
                                "id": result.request_id,
                                "object": "chat.completion.chunk",
                                "created": created,
                                "model": model,
                                "choices": [{"index": 0, "delta": {"content": output}, "finish_reason": finish_reason}],
                                "termination_reason": termination_reason
                            })
                        } else {
                            event
                        };
                        write!(stream, "data: {event}\n\n")?;
                    }
                    let include_usage = body
                        .pointer("/stream_options/include_usage")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    if include_usage {
                        let object = if endpoint == OpenAiGenerationEndpoint::Completions {
                            "text_completion"
                        } else {
                            "chat.completion.chunk"
                        };
                        let event = json!({
                            "id": result.request_id,
                            "object": object,
                            "created": created,
                            "model": model,
                            "choices": [],
                            "usage": {
                                "prompt_tokens": result.statistics.prompt_tokens,
                                "completion_tokens": result.statistics.generated_tokens,
                                "total_tokens": result.statistics.context_tokens
                            }
                        });
                        write!(stream, "data: {event}\n\n")?;
                    }
                    write!(stream, "data: [DONE]\n\n")?;
                    return stream.flush();
                }
                let (choice, response_object) = match endpoint {
                    OpenAiGenerationEndpoint::ChatCompletions => {
                        let mut message = tool_calls.as_ref().map_or_else(
                            || json!({"role":"assistant","content":output}),
                            |calls| json!({"role":"assistant","content":Value::Null,"tool_calls":calls}),
                        );
                        if !reasoning.trim().is_empty() {
                            message["reasoning_content"] = json!(reasoning);
                        }
                        (
                            json!({
                                "index": 0,
                                "message": message,
                                "finish_reason": if tool_calls.is_some() {"tool_calls"} else {finish_reason}
                            }),
                            "chat.completion",
                        )
                    }
                    OpenAiGenerationEndpoint::Completions => (
                        json!({
                            "index": 0,
                            "text": output,
                            "logprobs": Value::Null,
                            "finish_reason": finish_reason
                        }),
                        "text_completion",
                    ),
                    OpenAiGenerationEndpoint::Responses => {
                        unreachable!("Responses response was built before this match")
                    }
                };
                return respond_json(
                    stream,
                    200,
                    json!({
                        "id": result.request_id,
                        "generation_id": result.generation_id,
                        "object": response_object,
                        "created": created,
                        "model": model,
                        "backend": runtime.snapshot().backend,
                        "choices": [choice],
                        "termination_reason": termination_reason,
                        "execution": result.statistics.execution.as_ref().map(|execution| json!({
                            "backend": execution.backend,
                            "device": execution.device,
                            "requested_offloaded_layers": execution.requested_offloaded_layers,
                            "offloaded_layers": execution.offloaded_layers,
                            "fallback_reason": execution.fallback_reason
                        })),
                        "usage": {
                            "prompt_tokens": result.statistics.metrics_known.then_some(result.statistics.prompt_tokens),
                            "completion_tokens": result.statistics.metrics_known.then_some(result.statistics.generated_tokens),
                            "total_tokens": result.statistics.metrics_known.then_some(result.statistics.context_tokens)
                        },
                        "metrics": {
                            "tokens_per_second": result.statistics.metrics_known.then_some(result.statistics.tokens_per_second),
                            "latency_ms": result.statistics.latency_ms,
                            "inference_time_ms": result.statistics.backend_inference_time_ms,
                            "time_to_first_token_ms": result.statistics.time_to_first_token_ms,
                            "prompt_evaluation_time_ms": result.statistics.prompt_evaluation_time_ms,
                            "prompt_eval_time_ms": result.statistics.prompt_evaluation_time_ms,
                            "prompt_tokens_per_second": prompt_tokens_per_second(result.statistics.prompt_tokens, result.statistics.prompt_evaluation_time_ms),
                            "generation_time_ms": result.statistics.generation_time_ms,
                            "model_load_time_ms": model_load_time_ms,
                            "context_tokens": result.statistics.metrics_known.then_some(result.statistics.context_tokens),
                            "cache_hit": result.statistics.cache_hit,
                            "reused_prompt_tokens": result.statistics.reused_prompt_tokens
                        }
                    }),
                );
            }
            Ok(GenerationMessage::Cancelled(result)) => {
                eprintln!(
                    "Talvik HTTP generation cancelled request_id={} model={} generated_tokens={}",
                    request_id, model, result.statistics.generated_tokens
                );
                if endpoint == OpenAiGenerationEndpoint::Responses {
                    if streaming {
                        return write_responses_stream_failure(
                            stream,
                            &mut response_sequence_number,
                            &request_id,
                            created,
                            model,
                            body,
                            response_temperature,
                            response_top_p,
                            &output,
                            "cancelled",
                            "Responses generation was cancelled",
                        );
                    }
                    return respond_error(
                        stream,
                        500,
                        "Responses generation was cancelled".to_owned(),
                    );
                }
                if streaming {
                    if !stream_headers_sent {
                        write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n"
                        )?;
                        stream.flush()?;
                    }
                    write!(stream, "data: [DONE]\n\n")?;
                    return stream.flush();
                }
                let (choice, response_object) = match endpoint {
                    OpenAiGenerationEndpoint::ChatCompletions => (
                        json!({
                            "index": 0,
                            "message": {"role":"assistant","content":output},
                            "finish_reason": "cancelled"
                        }),
                        "chat.completion",
                    ),
                    OpenAiGenerationEndpoint::Completions => (
                        json!({
                            "index": 0,
                            "text": output,
                            "logprobs": Value::Null,
                            "finish_reason": "cancelled"
                        }),
                        "text_completion",
                    ),
                    OpenAiGenerationEndpoint::Responses => {
                        unreachable!("Responses cancellation returned before this match")
                    }
                };
                return respond_json(
                    stream,
                    200,
                    json!({
                        "id": result.request_id,
                        "generation_id": result.generation_id,
                        "object": response_object,
                        "created": created,
                        "model": model,
                        "backend": runtime.snapshot().backend,
                        "execution": result.statistics.execution.as_ref().map(|execution| json!({
                            "backend": execution.backend,
                            "device": execution.device,
                            "requested_offloaded_layers": execution.requested_offloaded_layers,
                            "offloaded_layers": execution.offloaded_layers,
                            "fallback_reason": execution.fallback_reason
                        })),
                        "choices": [choice],
                        "usage": {"completion_tokens": result.statistics.metrics_known.then_some(result.statistics.generated_tokens)},
                        "metrics": {
                            "tokens_per_second": result.statistics.metrics_known.then_some(result.statistics.tokens_per_second),
                            "latency_ms": result.statistics.latency_ms,
                            "inference_time_ms": result.statistics.backend_inference_time_ms,
                            "time_to_first_token_ms": result.statistics.time_to_first_token_ms,
                            "prompt_evaluation_time_ms": result.statistics.prompt_evaluation_time_ms,
                            "prompt_eval_time_ms": result.statistics.prompt_evaluation_time_ms,
                            "prompt_tokens_per_second": prompt_tokens_per_second(result.statistics.prompt_tokens, result.statistics.prompt_evaluation_time_ms),
                            "generation_time_ms": result.statistics.generation_time_ms,
                            "model_load_time_ms": model_load_time_ms,
                            "context_tokens": result.statistics.metrics_known.then_some(result.statistics.context_tokens),
                            "cache_hit": result.statistics.cache_hit,
                            "reused_prompt_tokens": result.statistics.reused_prompt_tokens
                        }
                    }),
                );
            }
            Ok(GenerationMessage::Failed(error)) => {
                eprintln!(
                    "Talvik HTTP generation failed request_id={request_id} model={model} error={error}"
                );
                if streaming && endpoint == OpenAiGenerationEndpoint::Responses {
                    return write_responses_stream_failure(
                        stream,
                        &mut response_sequence_number,
                        &request_id,
                        created,
                        model,
                        body,
                        response_temperature,
                        response_top_p,
                        &output,
                        "generation_failed",
                        &error.to_string(),
                    );
                }
                if streaming {
                    return write_openai_stream_failure(
                        stream,
                        stream_headers_sent,
                        runtime_error_status(&error),
                        if runtime_error_status(&error) == 400 {
                            "invalid_request_error"
                        } else {
                            "server_error"
                        },
                        "generation_failed",
                        &error.to_string(),
                    );
                }
                return respond_error(stream, runtime_error_status(&error), error.to_string());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                generation.cancel();
                eprintln!("Talvik HTTP generation timed_out request_id={request_id} model={model}");
                if drain_cancelled_generation(&generation) {
                    if streaming && endpoint == OpenAiGenerationEndpoint::Responses {
                        return write_responses_stream_failure(
                            stream,
                            &mut response_sequence_number,
                            &request_id,
                            created,
                            model,
                            body,
                            response_temperature,
                            response_top_p,
                            &output,
                            "timeout",
                            "generation request timed out",
                        );
                    }
                    if streaming {
                        return write_openai_stream_failure(
                            stream,
                            stream_headers_sent,
                            504,
                            "timeout_error",
                            "generation_timeout",
                            "generation request timed out",
                        );
                    }
                    return respond_error(stream, 504, "generation request timed out".to_owned());
                }
                if streaming && endpoint == OpenAiGenerationEndpoint::Responses {
                    return write_responses_stream_failure(
                        stream,
                        &mut response_sequence_number,
                        &request_id,
                        created,
                        model,
                        body,
                        response_temperature,
                        response_top_p,
                        &output,
                        "cancellation_cleanup_timeout",
                        "generation cancellation did not complete within the cleanup deadline",
                    );
                }
                if streaming {
                    return write_openai_stream_failure(
                        stream,
                        stream_headers_sent,
                        500,
                        "server_error",
                        "cancellation_cleanup_timeout",
                        "generation cancellation did not complete within the cleanup deadline",
                    );
                }
                return respond_error(
                    stream,
                    500,
                    "generation cancellation did not complete within the cleanup deadline"
                        .to_owned(),
                );
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                if streaming && endpoint == OpenAiGenerationEndpoint::Responses {
                    return write_responses_stream_failure(
                        stream,
                        &mut response_sequence_number,
                        &request_id,
                        created,
                        model,
                        body,
                        response_temperature,
                        response_top_p,
                        &output,
                        "generation_stream_closed",
                        "generation stream closed before completion",
                    );
                }
                if streaming {
                    return write_openai_stream_failure(
                        stream,
                        stream_headers_sent,
                        500,
                        "server_error",
                        "generation_stream_closed",
                        "generation stream closed before completion",
                    );
                }
                return respond_error(stream, 500, "generation stream closed".to_owned());
            }
        }
    }
}

fn generation_options(body: &Value, context_size: u32) -> GenerationOptions {
    let defaults = GenerationOptions::default();
    let max_tokens = body
        .get("max_completion_tokens")
        .or_else(|| body.get("max_output_tokens"))
        .or_else(|| body.get("max_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            std::env::var("TALVIK_HTTP_MAX_TOKENS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(128)
        })
        .min(u64::from(u32::MAX));
    let max_tokens = u32::try_from(max_tokens).unwrap_or(u32::MAX);
    let temperature = body
        .get("temperature")
        .and_then(|value| serde_json::from_value::<f32>(value.clone()).ok())
        .unwrap_or(defaults.temperature);
    let top_p = body
        .get("top_p")
        .and_then(|value| serde_json::from_value::<f32>(value.clone()).ok())
        .unwrap_or(defaults.top_p);
    let top_k = body
        .get("top_k")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(defaults.top_k);
    let seed = body
        .get("seed")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(defaults.seed);
    let stop_sequences = body
        .get("stop")
        .map(|value| match value {
            Value::String(stop) => vec![stop.clone()],
            Value::Array(values) => values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default();
    GenerationOptions {
        temperature,
        top_p,
        top_k,
        max_tokens,
        seed,
        context_size,
        stop_sequences,
    }
}

fn completion_finish_reason(termination: GenerationTermination) -> &'static str {
    match termination {
        GenerationTermination::Stop => "stop",
        GenerationTermination::Length | GenerationTermination::ContextLength => "length",
    }
}

fn completion_termination_reason(termination: GenerationTermination) -> &'static str {
    match termination {
        GenerationTermination::Stop => "stop",
        GenerationTermination::Length => "max_tokens",
        GenerationTermination::ContextLength => "context_length",
    }
}

fn write_chat_stream_role(
    stream: &mut impl Write,
    request_id: &str,
    created: u64,
    model: &str,
) -> std::io::Result<()> {
    let event = json!({
        "id": request_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{"index": 0, "delta": {"role": "assistant", "content": ""}, "finish_reason": Value::Null}]
    });
    write!(stream, "data: {event}\n\n")?;
    stream.flush()
}

/// Wait for the backend worker to publish its terminal result after a timeout.
///
/// Returning a timeout response before this point can leave the single
/// generation admission occupied on slower CPUs. Draining the terminal event
/// preserves the safety boundary: a subsequent request is admitted only after
/// the native worker has released its runtime state.
fn drain_cancelled_generation(generation: &GenerationStream) -> bool {
    let deadline = Instant::now() + CANCELLATION_CLEANUP_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        match generation.recv_timeout(remaining) {
            Ok(GenerationMessage::Token(_)) => {}
            Ok(
                GenerationMessage::Cancelled(_)
                | GenerationMessage::Completed(_)
                | GenerationMessage::Failed(_),
            ) => return true,
            Err(
                std::sync::mpsc::RecvTimeoutError::Timeout
                | std::sync::mpsc::RecvTimeoutError::Disconnected,
            ) => return false,
        }
    }
}

// A transport error must never leave an unobserved request consuming inference.
struct HttpGenerationLease<'a> {
    generation: &'a GenerationStream,
    owners: &'a Arc<Mutex<HashMap<String, String>>>,
    request_id: &'a str,
}

impl Drop for HttpGenerationLease<'_> {
    fn drop(&mut self) {
        self.generation.cancel();
        finish_owner(self.owners, self.request_id);
    }
}

fn peer_disconnected(stream: &TcpStream) -> std::io::Result<bool> {
    match stream.peek(&mut [0_u8; 1]) {
        Ok(0) => Ok(true),
        Ok(_) => Ok(false),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::Interrupted
            ) =>
        {
            Ok(false)
        }
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::BrokenPipe
            ) =>
        {
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

fn runtime_error_status(error: &oid_shared::RuntimeError) -> u16 {
    use oid_shared::RuntimeError;
    match error {
        RuntimeError::InvalidRequest(_) => 400,
        RuntimeError::ModelNotFound(_) => 404,
        RuntimeError::ResourceAdmission {
            retryable: true, ..
        } => 429,
        RuntimeError::ResourceAdmission {
            retryable: false, ..
        }
        | RuntimeError::NativeBackendUnavailable(_) => 503,
        RuntimeError::NotImplemented(_) => 501,
        RuntimeError::ModelLifecycle(_) => 409,
        _ => 500,
    }
}

fn reserve_owner(
    owners: &Arc<Mutex<HashMap<String, String>>>,
    request_id: &str,
    principal: &str,
) -> bool {
    let Ok(mut owners) = owners.lock() else {
        return false;
    };
    if let std::collections::hash_map::Entry::Vacant(entry) = owners.entry(request_id.to_owned()) {
        entry.insert(principal.to_owned());
        true
    } else {
        false
    }
}

fn finish_owner(owners: &Arc<Mutex<HashMap<String, String>>>, request_id: &str) {
    if let Ok(mut owners) = owners.lock() {
        owners.remove(request_id);
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

const fn request_body_size_is_allowed(size: usize) -> bool {
    size <= MAX_REQUEST_BODY_BYTES
}

#[allow(clippy::needless_pass_by_value)]
fn respond_json(stream: &mut TcpStream, status: u16, body: Value) -> std::io::Result<()> {
    let payload = body.to_string();
    write!(
        stream,
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        reason(status),
        payload.len()
    )
}

#[allow(clippy::needless_pass_by_value)]
fn respond_error(stream: &mut TcpStream, status: u16, message: String) -> std::io::Result<()> {
    let error_type = match status {
        400 => "invalid_request_error",
        401 => "authentication_error",
        403 => "permission_error",
        404 => "not_found_error",
        409 => "conflict_error",
        413 => "request_too_large",
        429 => "rate_limit_error",
        501 => "unsupported_feature",
        504 => "timeout_error",
        _ => "server_error",
    };
    respond_json(
        stream,
        status,
        json!({"error": {
            "type": error_type,
            "message": message,
            "param": Value::Null,
            "code": Value::Null
        }}),
    )
}

fn respond_ollama_error(
    stream: &mut TcpStream,
    status: u16,
    message: impl Into<String>,
) -> std::io::Result<()> {
    let message = message.into();
    respond_json(stream, status, json!({"error": message}))
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        502 => "Bad Gateway",
        501 => "Not Implemented",
        504 => "Gateway Timeout",
        500 => "Internal Server Error",
        _ => "Error",
    }
}

fn invalid_data(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
}

fn openai_timestamp() -> std::io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| invalid_data(format!("system clock is before Unix epoch: {error}")))
}

fn unique_request_id() -> String {
    format!(
        "http-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    )
}

#[cfg(test)]
mod tests {
    use super::{
        add_ollama_statistics, build_chat_content, build_chat_prompt, build_completion_prompt,
        build_response_prompt, completion_finish_reason, completion_termination_reason,
        flatten_content, generation_options, iso8601_now, ollama_generation_options, ollama_prompt,
        ollama_request_content, ollama_tool_call, ollama_tool_stream_chunk,
        openai_model_output_error, openai_stream_error_frame, openai_tool_stream_delta,
        parse_chat_template_kwargs, parse_ollama_format, parse_openai_response_format,
        parse_tool_calls, reason, request_body_size_is_allowed, responses_sse_frame,
        tool_call_output_error_code, validate_generated_tool_calls, validate_generation_request,
        validate_ollama_generation_request, validate_openai_tool_history,
        validate_streamed_tool_call, GenerationTermination, OllamaGenerationEndpoint,
        OpenAiGenerationEndpoint, OpenAiToolChoice, ToolCallStreamParser, ToolStreamPart,
        MAX_CONTEXT_TOKENS, MAX_REQUEST_BODY_BYTES, MAX_TOOL_CALL_BYTES,
    };
    use base64::Engine as _;
    use serde_json::json;

    #[test]
    fn chat_stream_acknowledges_role_without_fabricating_output_or_completion() {
        let mut bytes = Vec::new();
        super::write_chat_stream_role(&mut bytes, "request-1", 123, "local-model").unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.ends_with("\n\n"));
        let event: serde_json::Value =
            serde_json::from_str(text.trim().strip_prefix("data: ").unwrap()).unwrap();
        assert_eq!(event["id"], "request-1");
        assert_eq!(
            event["choices"][0]["delta"],
            json!({"role": "assistant", "content": ""})
        );
        assert!(event["choices"][0]["finish_reason"].is_null());
        assert!(event.get("usage").is_none());
    }

    #[test]
    fn request_ownership_cannot_be_overwritten_by_another_client() {
        use std::sync::{Arc, Mutex};
        let owners = Arc::new(Mutex::new(std::collections::HashMap::new()));
        assert!(super::reserve_owner(&owners, "same-id", "alice"));
        assert!(!super::reserve_owner(&owners, "same-id", "bob"));
        assert_eq!(owners.lock().unwrap().get("same-id").unwrap(), "alice");
        super::finish_owner(&owners, "same-id");
        assert!(super::reserve_owner(&owners, "same-id", "bob"));
    }

    #[test]
    fn runtime_errors_have_actionable_http_statuses() {
        use oid_shared::RuntimeError;
        assert_eq!(
            super::runtime_error_status(&RuntimeError::InvalidRequest("context overflow".into())),
            400
        );
        assert_eq!(
            super::runtime_error_status(&RuntimeError::ModelNotFound("missing".into())),
            404
        );
        assert_eq!(
            super::runtime_error_status(&RuntimeError::ResourceAdmission {
                message: "queue full".into(),
                retryable: true
            }),
            429
        );
        assert_eq!(
            super::runtime_error_status(&RuntimeError::NativeBackend("failure".into())),
            500
        );
    }

    #[test]
    fn transport_probe_distinguishes_idle_peer_from_closed_peer() {
        use std::net::{TcpListener, TcpStream};
        use std::time::Duration;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(1)))
            .unwrap();
        assert!(!super::peer_disconnected(&server).unwrap());
        drop(client);
        assert!(super::peer_disconnected(&server).unwrap());
    }

    #[test]
    fn transport_lease_cancels_and_removes_owner_on_early_return() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc, Mutex,
        };
        let (_sender, receiver) = mpsc::channel();
        let cancellation = Arc::new(AtomicBool::new(false));
        let generation = super::GenerationStream::from_parts(receiver, cancellation.clone());
        let owners = Arc::new(Mutex::new(std::collections::HashMap::from([(
            "test".to_owned(),
            "owner".to_owned(),
        )])));
        {
            let _lease = super::HttpGenerationLease {
                generation: &generation,
                owners: &owners,
                request_id: "test",
            };
        }
        assert!(cancellation.load(Ordering::SeqCst));
        assert_eq!(owners.lock().unwrap().len(), 0);
    }

    #[test]
    fn rejects_oversized_http_bodies() {
        assert!(request_body_size_is_allowed(MAX_REQUEST_BODY_BYTES));
        assert!(!request_body_size_is_allowed(MAX_REQUEST_BODY_BYTES + 1));
    }

    #[test]
    fn daemon_defaults_support_ide_context_and_allow_request_overrides() {
        assert_eq!(
            super::generation_defaults(&json!({}), None, None),
            Ok((4096, None))
        );
        assert_eq!(
            super::generation_defaults(&json!({}), Some("16384"), Some("false")),
            Ok((16384, Some(false)))
        );
        assert_eq!(
            super::generation_defaults(
                &json!({"context_size":8192,"chat_template_kwargs":{"enable_thinking":true}}),
                Some("16384"),
                Some("false")
            ),
            Ok((8192, Some(true)))
        );
        for (context, thinking) in [
            ("0", "false"),
            ("32769", "false"),
            ("bad", "false"),
            ("16384", "bad"),
        ] {
            assert!(super::generation_defaults(&json!({}), Some(context), Some(thinking)).is_err());
        }
    }

    fn loaded_profile_fixture() -> oid_runtime::ModelProfile {
        oid_runtime::ModelProfile {
            architecture: "arbitrary-architecture".to_owned(),
            context_length: 4096,
            completion: true,
            chat: true,
            embedding: false,
            embedding_declared: true,
            vision: false,
            cache: oid_runtime::CacheStrategy::Attention,
            tokenizer_adds_bos: true,
            tools_declared: true,
            tool_format: Some(oid_runtime::ToolFormat::XmlFunction),
            reasoning_format: Some(oid_runtime::ReasoningFormat::ThinkTags),
            thinking_control: true,
            reasoning_prefilled: false,
        }
    }

    #[test]
    fn profile_discovery_does_not_promote_declared_unsupported_capabilities() {
        let mut profile = loaded_profile_fixture();
        profile.tool_format = None;
        let json = super::profile_json(&profile);
        assert_eq!(json["tools_declared"], true);
        assert_eq!(json["embedding_declared"], true);
        assert!(!super::profile_capabilities(&profile).contains(&"tools"));
        assert!(!super::profile_capabilities(&profile).contains(&"embedding"));
        assert!(super::reasoning_parser(Some(&profile), Some(true)).is_some());
        assert!(super::reasoning_parser(None, Some(true)).is_none());
        profile.reasoning_prefilled = true;
        let mut disabled = super::reasoning_parser(Some(&profile), Some(false)).expect("parser");
        assert_eq!(disabled.push("answer").content, "answer");
        let mut enabled = super::reasoning_parser(Some(&profile), Some(true)).expect("parser");
        assert_eq!(enabled.push("reason").reasoning, "reason");
    }

    #[test]
    fn selected_function_parser_preserves_schema_types_at_every_split() {
        let profile = loaded_profile_fixture();
        let definitions = vec![
            json!({"function":{"name":"inspect","parameters":{"properties":{"path":{"type":"string"},"count":{"type":"integer"},"flag":{"type":"boolean"},"items":{"type":"array"}}}}}),
        ];
        let text = "<tool_call><function=inspect>\n<parameter=path>\n123\n</parameter>\n<parameter=count>2</parameter><parameter=flag>true</parameter><parameter=items>[1,2]</parameter></function></tool_call>";
        for split in 0..=text.len() {
            let mut parser =
                super::selected_tool_parser("test".to_owned(), Some(&profile), Some(&definitions));
            let parts = parser
                .push(&text[..split])
                .expect("first")
                .into_iter()
                .chain(parser.push(&text[split..]).expect("second"))
                .chain(parser.finish().expect("finish"))
                .collect::<Vec<_>>();
            let calls = parts
                .into_iter()
                .filter_map(|p| {
                    if let super::ToolStreamPart::Call { call, .. } = p {
                        Some(call)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(calls.len(), 1);
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(
                    calls[0]["function"]["arguments"].as_str().expect("args")
                )
                .expect("JSON"),
                json!({"path":"123","count":2,"flag":true,"items":[1,2]})
            );
        }
        for invalid in [
            "<function=inspect><parameter=x>1</parameter><parameter=x>2</parameter></function>",
            "<function=inspect><parameter=count>bad</parameter></function>",
            "<function=inspect>unexpected</function>",
        ] {
            assert!(super::parse_tool_payload(
                invalid,
                oid_runtime::ToolFormat::XmlFunction,
                &definitions
            )
            .is_err());
        }
    }

    #[test]
    fn leading_bare_and_fenced_json_calls_are_bounded_and_prose_is_preserved() {
        for text in [
            "{\"name\":\"inspect\",\"arguments\":{\"path\":\"123\"}}",
            "```json\n{\"name\":\"inspect\",\"arguments\":{\"path\":\"123\"}}\n```",
        ] {
            for split in 0..=text.len() {
                let mut parser = super::ToolCallStreamParser::new("test".to_owned());
                let mut parts = parser.push(&text[..split]).expect("part one");
                parts.extend(parser.push(&text[split..]).expect("part two"));
                parts.extend(parser.finish().expect("finish"));
                let calls = parts
                    .iter()
                    .filter(|p| matches!(p, super::ToolStreamPart::Call { .. }))
                    .count();
                assert_eq!(calls, 1);
            }
        }
        let text = "Here is an example: ```json\n{\"name\":\"inspect\",\"arguments\":{}}\n```";
        assert!(super::parse_tool_calls(text, "test")
            .expect("prose")
            .is_none());
        let mut parser = super::ToolCallStreamParser::new("test".to_owned());
        assert!(parser
            .push(&format!(
                "```json\n{}",
                "x".repeat(super::MAX_TOOL_CALL_BYTES)
            ))
            .is_err());
    }

    #[test]
    fn reasoning_is_removed_before_tool_proposals_are_parsed() {
        let profile = loaded_profile_fixture();
        let mut reason = super::reasoning_parser(Some(&profile), Some(false));
        let mut pending = None;
        let text="<think><tool_call>{\"name\":\"do_not_run\",\"arguments\":{}}</tool_call></think><tool_call>{\"name\":\"inspect\",\"arguments\":{}}</tool_call>";
        let (message, thought) = super::split_reasoning_message(
            Ok(oid_runtime::GenerationMessage::Token(text.to_owned())),
            &mut reason,
            &mut pending,
        );
        assert!(thought.contains("do_not_run"));
        let oid_runtime::GenerationMessage::Token(content) = message.expect("token") else {
            panic!("expected token")
        };
        let calls = super::parse_tool_calls_selected(&content, "test", Some(&profile), None)
            .expect("parse")
            .expect("calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["function"]["name"], "inspect");
        let mut reason = super::reasoning_parser(Some(&profile), Some(false));
        let _ = super::split_reasoning_message(
            Ok(oid_runtime::GenerationMessage::Token("<thi".to_owned())),
            &mut reason,
            &mut pending,
        );
        let result = oid_runtime::GenerationResult {
            request_id: "r".to_owned(),
            generation_id: "g".to_owned(),
            text: String::new(),
            statistics: oid_runtime::GenerationStatistics::default(),
        };
        let (tail, _) = super::split_reasoning_message(
            Ok(oid_runtime::GenerationMessage::Completed(result)),
            &mut reason,
            &mut pending,
        );
        assert!(matches!(tail,Ok(oid_runtime::GenerationMessage::Token(ref t)) if t=="<thi"));
        assert!(pending.is_some());
    }

    #[test]
    fn parses_chat_template_thinking_option() {
        assert_eq!(
            parse_chat_template_kwargs(&json!({
                "chat_template_kwargs": {"enable_thinking": false}
            })),
            Ok(Some(false))
        );
        assert_eq!(
            parse_chat_template_kwargs(&json!({
                "chat_template_kwargs": {"enable_thinking": true}
            })),
            Ok(Some(true))
        );
        assert!(parse_chat_template_kwargs(&json!({
            "chat_template_kwargs": {"enable_thinking": "false"}
        }))
        .is_err());
        assert!(parse_chat_template_kwargs(&json!({
            "chat_template_kwargs": {"unknown": false}
        }))
        .is_err());
    }

    #[test]
    fn openai_chat_content_preserves_inline_image_order() {
        let body = json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type":"text","text":"What is in this picture? "},
                    {"type":"image_url","image_url":{"url":"data:image/png;base64,AQID"}},
                    {"type":"text","text":" Answer briefly."}
                ]
            }]
        });
        let content = build_chat_content(&body).expect("inline data image should be accepted");

        assert_eq!(content.len(), 3);
        assert!(matches!(&content[1], oid_runtime::ContentPart::Image(image)
            if image.media_type == "image/png" && image.data == [1, 2, 3]));
        assert_eq!(
            flatten_content(&content),
            "\u{001e}user\nWhat is in this picture? <__media__> Answer briefly."
        );
        assert_eq!(
            build_chat_prompt(&body).expect("chat prompt"),
            flatten_content(&content)
        );
    }

    #[test]
    fn rejects_remote_openai_image_urls() {
        let body = json!({
            "messages": [{
                "role":"user",
                "content":[{"type":"image_url","image_url":{"url":"https://example.invalid/image.png"}}]
            }]
        });
        assert!(build_chat_content(&body)
            .expect_err("remote image URLs must not be fetched")
            .contains("data: scheme"));
    }

    #[test]
    fn ollama_generate_accepts_bounded_inline_base64_images() {
        let png_bytes = b"\x89PNG\r\n\x1a\nfixture";
        let body = json!({
            "prompt":"Describe this image.",
            "images":[super::STANDARD.encode(png_bytes)]
        });
        let content = ollama_request_content(
            &body,
            OllamaGenerationEndpoint::Generate,
            "Describe this image.",
            "Describe this image.",
        )
        .expect("Ollama base64 image should be accepted");

        assert_eq!(content.len(), 2);
        assert!(matches!(&content[1], oid_runtime::ContentPart::Image(image)
            if image.media_type == "image/png" && image.data == png_bytes));
    }

    #[test]
    fn exposes_a_bounded_context_policy() {
        assert_eq!(MAX_CONTEXT_TOKENS, 32_768);
    }

    #[test]
    fn normalizes_template_tool_calls_to_openai_shape() {
        let calls = parse_tool_calls(
            r#"<tool_call>{"name":"get_weather","arguments":{"city":"Manila"}}</tool_call>"#,
            "request-1",
        )
        .expect("valid tool output")
        .expect("tool call");
        assert_eq!(calls[0]["id"], "call_request-1_0");
        assert_eq!(calls[0]["type"], "function");
        assert_eq!(calls[0]["function"]["name"], "get_weather");
        assert_eq!(calls[0]["function"]["arguments"], r#"{"city":"Manila"}"#);
    }

    #[test]
    fn assigns_distinct_ids_to_multiple_tool_calls() {
        let calls = parse_tool_calls(
            concat!(
                "<tool_call>{\"name\":\"one\",\"arguments\":{}}</tool_call>",
                "<tool_call>{\"name\":\"two\",\"arguments\":{}}</tool_call>"
            ),
            "request-2",
        )
        .expect("valid tool output")
        .expect("tool calls");
        assert_eq!(calls[0]["id"], "call_request-2_0");
        assert_eq!(calls[1]["id"], "call_request-2_1");
    }

    #[test]
    fn streams_split_tool_markers_without_leaking_them() {
        let mut parser = ToolCallStreamParser::new("stream-1".to_owned());
        let mut parts = parser.push("before λ<tool_").expect("first chunk");
        parts.extend(
            parser
                .push("call>{\"name\":\"weather\",\"arguments\":{\"city\":\"Manila\"}}</tool_")
                .expect("second chunk"),
        );
        parts.extend(parser.push("call>after").expect("third chunk"));
        parts.extend(parser.finish().expect("finished stream"));

        assert!(matches!(&parts[0], ToolStreamPart::Content(text) if text == "before λ"));
        let ToolStreamPart::Call { index, call } = &parts[1] else {
            panic!("expected parsed tool call");
        };
        assert_eq!(*index, 0);
        assert_eq!(call["id"], "call_stream-1_0");
        assert_eq!(call["function"]["name"], "weather");
        assert_eq!(call["function"]["arguments"], r#"{"city":"Manila"}"#);
        assert!(matches!(&parts[2], ToolStreamPart::Content(text) if text == "after"));
    }

    #[test]
    fn parses_fragmented_arguments_and_multiple_tool_calls() {
        let mut parser = ToolCallStreamParser::new("fragments".to_owned());
        let mut parts = Vec::new();
        for fragment in [
            "<tool_call>{\"name\":\"search\",",
            "\"arguments\":{\"query\":\"a\\\"b 世界\"}}</tool_call><tool_call>",
            "{\"name\":\"clock\",\"arguments\":",
            "{\"timezone\":\"UTC\"}}</tool_call>",
        ] {
            parts.extend(parser.push(fragment).expect("fragment"));
        }
        parts.extend(parser.finish().expect("finished fragments"));
        let calls = parts
            .into_iter()
            .filter_map(|part| match part {
                ToolStreamPart::Call { index, call } => Some((index, call)),
                ToolStreamPart::Content(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, 0);
        assert_eq!(calls[0].1["id"], "call_fragments_0");
        assert_eq!(
            calls[0].1["function"]["arguments"],
            r#"{"query":"a\"b 世界"}"#
        );
        assert_eq!(calls[1].0, 1);
        assert_eq!(calls[1].1["id"], "call_fragments_1");
        assert_eq!(calls[1].1["function"]["name"], "clock");
    }

    #[test]
    fn serializes_openai_tool_call_stream_deltas() {
        let call = json!({
            "id": "call_stream_0",
            "type": "function",
            "function": {
                "name": "weather",
                "arguments": "{\"city\":\"Manila\"}"
            }
        });
        assert_eq!(
            openai_tool_stream_delta(&ToolStreamPart::Call { index: 0, call }, true),
            Some(json!({"tool_calls":[{
                "index":0,
                "id":"call_stream_0",
                "type":"function",
                "function":{"name":"weather","arguments":"{\"city\":\"Manila\"}"}
            }],"role":"assistant"}))
        );
    }

    #[test]
    fn validates_streamed_calls_before_emitting_them() {
        let first = json!({"function":{"name":"first","arguments":"{}"}});
        let second = json!({"function":{"name":"second","arguments":"{}"}});
        assert_eq!(
            validate_streamed_tool_call(
                &first,
                &[],
                &OpenAiToolChoice::Function("first".to_owned()),
                true
            ),
            Ok(())
        );
        assert!(validate_streamed_tool_call(
            &second,
            &[],
            &OpenAiToolChoice::Function("first".to_owned()),
            true
        )
        .is_err());
        assert!(
            validate_streamed_tool_call(&second, &[first], &OpenAiToolChoice::Auto, false).is_err()
        );
    }

    #[test]
    fn serializes_ollama_tool_stream_chunks_natively() {
        let call = json!({
            "id": "call_stream_0",
            "type": "function",
            "function": {
                "name": "weather",
                "arguments": "{\"city\":\"Manila\"}"
            }
        });
        assert_eq!(
            ollama_tool_stream_chunk(
                "qwen",
                "2026-10-29T00:00:00Z",
                &ToolStreamPart::Call { index: 0, call }
            ),
            Some(json!({
                "model":"qwen",
                "created_at":"2026-10-29T00:00:00Z",
                "message":{
                    "role":"assistant",
                    "content":"",
                    "tool_calls":[{"function":{"name":"weather","arguments":{"city":"Manila"}}}]
                },
                "done":false
            }))
        );
    }

    #[test]
    fn rejects_malformed_and_incomplete_streamed_tool_calls() {
        let mut malformed = ToolCallStreamParser::new("malformed".to_owned());
        assert!(malformed.push("<tool_call>{not-json}</tool_call>").is_err());

        let mut incomplete_call = ToolCallStreamParser::new("incomplete".to_owned());
        assert!(incomplete_call
            .push("<tool_call>{\"name\":\"weather\"}")
            .is_ok());
        assert!(incomplete_call.finish().is_err());

        let mut incomplete_marker = ToolCallStreamParser::new("marker".to_owned());
        assert!(incomplete_marker.push("text<tool_").is_ok());
        assert!(incomplete_marker.finish().is_err());

        let mut trailing_marker = ToolCallStreamParser::new("trailing".to_owned());
        let parts = trailing_marker
            .push("<tool_call>{\"name\":\"weather\",\"arguments\":{}}</tool_call><tool_")
            .expect("complete call is retained");
        assert!(parts
            .iter()
            .any(|part| matches!(part, ToolStreamPart::Call { .. })));
        assert!(trailing_marker.finish().is_ok());
    }

    #[test]
    fn bounds_streamed_tool_call_size() {
        let mut parser = ToolCallStreamParser::new("oversized".to_owned());
        let oversized = format!("<tool_call>{}", "x".repeat(MAX_TOOL_CALL_BYTES + 1));
        assert!(parser.push(&oversized).is_err());
    }

    #[test]
    fn preserves_ordinary_unicode_output_when_no_tool_call_is_present() {
        let output = "Hello, 世界 🌍";
        assert_eq!(
            parse_tool_calls(output, "plain").expect("valid output"),
            None
        );

        let mut parser = ToolCallStreamParser::new("plain".to_owned());
        let mut parts = parser.push(output).expect("content");
        parts.extend(parser.finish().expect("finish"));
        assert!(matches!(&parts[..], [ToolStreamPart::Content(text)] if text == output));
    }

    #[test]
    fn validates_openai_tool_result_correlation() {
        let valid = json!({"messages":[
            {"role":"assistant","tool_calls":[
                {"id":"call_a","type":"function","function":{"name":"first","arguments":"{}"}},
                {"id":"call_b","type":"function","function":{"name":"second","arguments":"{}"}}
            ]},
            {"role":"tool","tool_call_id":"call_b","name":"second","content":"two"},
            {"role":"tool","tool_call_id":"call_a","name":"first","content":"one"},
            {"role":"user","content":"Summarize"}
        ]});
        assert_eq!(validate_openai_tool_history(&valid), Ok(()));

        let unknown_id = json!({"messages":[
            {"role":"tool","tool_call_id":"missing","content":"result"}
        ]});
        assert!(validate_openai_tool_history(&unknown_id).is_err());

        let mismatched_name = json!({"messages":[
            {"role":"assistant","tool_calls":[
                {"id":"call_a","type":"function","function":{"name":"first","arguments":"{}"}}
            ]},
            {"role":"tool","tool_call_id":"call_a","name":"other","content":"result"}
        ]});
        assert!(validate_openai_tool_history(&mismatched_name).is_err());
    }

    #[test]
    fn validates_auto_none_required_and_function_tool_choices() {
        let tools = json!({"type":"function","function":{"name":"get_weather"}});
        assert!(validate_generation_request(
            &json!({"tools":[tools.clone()],"tool_choice":"auto"}),
            OpenAiGenerationEndpoint::ChatCompletions
        )
        .is_ok());
        assert!(validate_generation_request(
            &json!({"tools":[tools.clone()],"tool_choice":"none"}),
            OpenAiGenerationEndpoint::ChatCompletions
        )
        .is_ok());
        assert!(validate_generation_request(
            &json!({"tools":[tools.clone()],"parallel_tool_calls":false}),
            OpenAiGenerationEndpoint::ChatCompletions
        )
        .is_ok());
        assert_eq!(
            validate_generation_request(
                &json!({"tools":[tools.clone()],"parallel_tool_calls":"false"}),
                OpenAiGenerationEndpoint::ChatCompletions
            )
            .map_err(|(status, _)| status),
            Err(400)
        );
        assert_eq!(
            validate_generation_request(
                &json!({"tools":[tools.clone()],"tool_choice":"required"}),
                OpenAiGenerationEndpoint::ChatCompletions
            )
            .map_err(|(status, _)| status),
            Ok(())
        );
        assert_eq!(
            validate_generation_request(
                &json!({"tools":[tools.clone()],"tool_choice":{
                    "type":"function","function":{"name":"get_weather"}
                }}),
                OpenAiGenerationEndpoint::ChatCompletions
            )
            .map_err(|(status, _)| status),
            Ok(())
        );
        assert_eq!(
            validate_generation_request(
                &json!({"tools":[tools.clone()],"tool_choice":{
                    "type":"function","function":{"name":"missing"}
                }}),
                OpenAiGenerationEndpoint::ChatCompletions
            )
            .map_err(|(status, _)| status),
            Err(400)
        );
        assert_eq!(
            validate_generation_request(
                &json!({"tools":[tools.clone()],"tool_choice":"required"}),
                OpenAiGenerationEndpoint::ChatCompletions
            )
            .map_err(|(status, _)| status),
            Ok(())
        );
        assert_eq!(
            validate_generation_request(
                &json!({"tool_choice":"required"}),
                OpenAiGenerationEndpoint::ChatCompletions
            )
            .map_err(|(status, _)| status),
            Err(400)
        );
        assert_eq!(
            validate_generation_request(
                &json!({"tool_choice":"auto"}),
                OpenAiGenerationEndpoint::ChatCompletions
            )
            .map_err(|(status, _)| status),
            Err(400)
        );
    }

    #[test]
    fn enforces_generated_required_and_function_tool_calls() {
        let calls = vec![
            json!({"function":{"name":"get_weather","arguments":"{}"}}),
            json!({"function":{"name":"get_weather","arguments":"{}"}}),
        ];
        assert!(
            validate_generated_tool_calls(&OpenAiToolChoice::Required, Some(&calls), true).is_ok()
        );
        let selected_call = vec![calls[0].clone()];
        assert!(validate_generated_tool_calls(
            &OpenAiToolChoice::Function("get_weather".to_owned()),
            Some(&selected_call),
            true
        )
        .is_ok());
        assert!(validate_generated_tool_calls(&OpenAiToolChoice::Required, None, true).is_err());
        assert!(validate_generated_tool_calls(
            &OpenAiToolChoice::Function("get_weather".to_owned()),
            Some(&calls),
            true
        )
        .is_err());
        assert!(validate_generated_tool_calls(
            &OpenAiToolChoice::Function("get_weather".to_owned()),
            Some(&[json!({"function":{"name":"other"}})]),
            true
        )
        .is_err());
        assert!(
            validate_generated_tool_calls(&OpenAiToolChoice::Auto, Some(&calls), false).is_err()
        );
    }

    #[test]
    fn reports_successful_termination_without_collapsing_length_into_stop() {
        assert_eq!(
            completion_finish_reason(GenerationTermination::Stop),
            "stop"
        );
        assert_eq!(
            completion_finish_reason(GenerationTermination::Length),
            "length"
        );
        assert_eq!(
            completion_termination_reason(GenerationTermination::Length),
            "max_tokens"
        );
        assert_eq!(
            completion_finish_reason(GenerationTermination::ContextLength),
            "length"
        );
        assert_eq!(
            completion_termination_reason(GenerationTermination::ContextLength),
            "context_length"
        );
    }

    #[test]
    fn maps_openai_generation_controls_without_silently_ignoring_them() {
        let options = generation_options(
            &json!({
                "max_tokens": 7,
                "max_completion_tokens": 9,
                "temperature": 0.1,
                "top_p": 0.8,
                "top_k": 20,
                "seed": 42
            }),
            2048,
        );
        assert_eq!(options.max_tokens, 9);
        assert!((options.temperature - 0.1).abs() < f32::EPSILON);
        assert!((options.top_p - 0.8).abs() < f32::EPSILON);
        assert_eq!(options.top_k, 20);
        assert_eq!(options.seed, 42);
        assert_eq!(options.context_size, 2048);
    }

    #[test]
    fn keeps_chat_and_legacy_completion_prompts_distinct() {
        let chat = json!({"messages":[{"role":"user","content":"hello"}]});
        let completion = json!({"prompt":"hello"});
        assert_eq!(
            build_chat_prompt(&chat).expect("chat prompt"),
            "\u{001e}user\nhello"
        );
        assert_eq!(
            build_completion_prompt(&completion).expect("completion prompt"),
            "hello"
        );
    }

    #[test]
    fn preserves_tool_call_id_in_tool_result_prompt() {
        let body = json!({"messages":[
            {"role":"assistant","tool_calls":[
                {"id":"call_123","type":"function","function":{"name":"get_weather","arguments":"{}"}}
            ]},
            {"role":"tool","tool_call_id":"call_123","name":"get_weather","content":"sunny"}
        ]});
        let prompt = build_chat_prompt(&body).expect("chat prompt");
        assert!(prompt.contains("tool_call_id: call_123"));
        assert!(prompt.contains("Tool result for get_weather"));
        assert!(prompt.contains("\u{001e}__talvik_tool_calls\n"));
        assert!(!prompt.contains("<tool_calls>"));
    }

    #[test]
    fn maps_text_responses_input_and_accepts_streaming() {
        let response = json!({"instructions":"Be concise.","input":"hello"});
        assert_eq!(
            build_response_prompt(&response).expect("Responses prompt"),
            "\u{001e}system\nBe concise.\u{001e}user\nhello"
        );
        assert_eq!(
            validate_generation_request(
                &json!({"input":"hello","stream":true}),
                OpenAiGenerationEndpoint::Responses
            )
            .map_err(|(status, _)| status),
            Ok(())
        );
    }

    #[test]
    fn formats_responses_sse_events_with_type_and_sequence_number() {
        let frame = responses_sse_frame(
            "response.output_text.delta",
            3,
            json!({"item_id":"msg_test","delta":"hello"}),
        );
        assert!(frame.starts_with("event: response.output_text.delta\n"));
        let data = frame
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .expect("SSE event data");
        let event: serde_json::Value = serde_json::from_str(data).expect("valid event JSON");
        assert_eq!(event["type"], "response.output_text.delta");
        assert_eq!(event["sequence_number"], 3);
        assert_eq!(event["delta"], "hello");
    }

    #[test]
    fn classifies_model_output_failures_and_gateway_status() {
        let error = openai_model_output_error("incomplete_tool_call", "model ended early");
        assert_eq!(error["error"]["type"], "model_output_error");
        assert_eq!(error["error"]["code"], "incomplete_tool_call");
        assert_eq!(
            tool_call_output_error_code("model ended with an incomplete tool call"),
            "incomplete_tool_call"
        );
        assert_eq!(
            tool_call_output_error_code("model did not produce the required tool call"),
            "tool_choice_not_satisfied"
        );
        assert_eq!(reason(502), "Bad Gateway");
    }

    #[test]
    fn streamed_model_output_errors_do_not_end_with_success_sentinel() {
        let frame = openai_stream_error_frame(
            "model_output_error",
            "incomplete_tool_call",
            "model ended early",
        );
        assert!(frame.starts_with("data: "));
        assert!(!frame.contains("event:"));
        let payload: serde_json::Value =
            serde_json::from_str(frame.trim().strip_prefix("data: ").unwrap()).unwrap();
        assert_eq!(payload["error"]["type"], "model_output_error");
        assert!(frame.contains("\"code\":\"incomplete_tool_call\""));
        assert!(!frame.contains("[DONE]"));
    }

    #[test]
    fn rejects_unsupported_options_and_invalid_generation_values() {
        assert_eq!(
            validate_generation_request(
                &json!({"response_format":{"type":"json_object"}}),
                OpenAiGenerationEndpoint::ChatCompletions
            )
            .map_err(|(status, _)| status),
            Ok(())
        );
        assert_eq!(
            validate_generation_request(
                &json!({"response_format":{"type":"json_object"}}),
                OpenAiGenerationEndpoint::Completions
            )
            .map_err(|(status, _)| status),
            Err(501)
        );
        assert_eq!(
            validate_generation_request(
                &json!({"temperature":3}),
                OpenAiGenerationEndpoint::ChatCompletions
            )
            .map_err(|(status, _)| status),
            Err(400)
        );
        assert_eq!(
            validate_generation_request(
                &json!({"prompt":["hello"]}),
                OpenAiGenerationEndpoint::Completions
            )
            .map_err(|(status, _)| status),
            Err(501)
        );
    }

    #[test]
    fn accepts_stream_options_include_usage_only_for_streams() {
        let chat = OpenAiGenerationEndpoint::ChatCompletions;
        assert!(validate_generation_request(
            &json!({"stream":true,"stream_options":{"include_usage":true}}),
            chat
        )
        .is_ok());
        for (body, status) in [
            (json!({"stream_options":{"include_usage":true}}), 400),
            (
                json!({"stream":true,"stream_options":{"include_usage":"yes"}}),
                400,
            ),
            (json!({"stream":true,"stream_options":"x"}), 400),
            (json!({"stream":true,"stream_options":{"other":1}}), 501),
        ] {
            assert_eq!(
                validate_generation_request(&body, chat).map_err(|(code, _)| code),
                Err(status)
            );
        }
    }

    #[test]
    fn validates_openai_json_object_and_strict_schema_formats() {
        assert_eq!(
            parse_openai_response_format(Some(&json!({"type":"json_object"}))),
            Ok(Some(oid_runtime::StructuredOutput::JsonObject))
        );
        let schema = json!({
            "type":"object",
            "properties":{"mode":{"type":"string","enum":["fast","safe"]}},
            "required":["mode"],
            "additionalProperties":false
        });
        assert_eq!(
            parse_openai_response_format(Some(&json!({
                "type":"json_schema",
                "json_schema":{
                    "name":"settings",
                    "strict":true,
                    "schema":schema
                }
            }))),
            Ok(Some(oid_runtime::StructuredOutput::JsonSchema(schema)))
        );
        assert_eq!(
            parse_openai_response_format(Some(&json!({
                "type":"json_schema",
                "json_schema":{"name":"bad","strict":true,"schema":{"type":"string","pattern":"x"}}
            })))
            .map_err(|(status, _)| status),
            Err(400)
        );
        assert_eq!(
            parse_openai_response_format(Some(&json!({
                "type":"json_schema",
                "json_schema":{"name":"not-strict","schema":{"type":"object"}}
            })))
            .map_err(|(status, _)| status),
            Err(501)
        );
    }

    #[test]
    fn accepts_ollama_json_and_schema_formats() {
        assert_eq!(
            parse_ollama_format(Some(&json!("json"))),
            Ok(Some(oid_runtime::StructuredOutput::JsonObject))
        );
        let schema = json!({
            "type":"array",
            "items":{"type":"integer"}
        });
        assert_eq!(
            parse_ollama_format(Some(&schema)),
            Ok(Some(oid_runtime::StructuredOutput::JsonSchema(schema)))
        );
        assert_eq!(
            parse_ollama_format(Some(&json!("jsonl"))).map_err(|(status, _)| status),
            Err(400)
        );
        assert_eq!(
            validate_ollama_generation_request(
                &json!({"model":"m","prompt":"hello","format":{"type":"object","pattern":"x"}}),
                OllamaGenerationEndpoint::Generate
            )
            .map_err(|(status, _)| status),
            Err(400)
        );
    }

    #[test]
    fn maps_ollama_generation_options_to_runtime_controls() {
        let options = ollama_generation_options(&json!({
            "options": {
                "num_ctx": 2048,
                "num_predict": 32,
                "temperature": 0.2,
                "top_p": 0.75,
                "top_k": 10,
                "seed": 7,
                "stop": ["END"]
            }
        }))
        .expect("supported options");
        assert_eq!(options.context_size, 2048);
        assert_eq!(options.max_tokens, 32);
        assert_eq!(options.temperature.to_bits(), 0.2_f32.to_bits());
        assert_eq!(options.top_p.to_bits(), 0.75_f32.to_bits());
        assert_eq!(options.top_k, 10);
        assert_eq!(options.seed, 7);
        assert_eq!(options.stop_sequences, ["END"]);
    }

    #[test]
    fn rejects_unavailable_ollama_features_and_options() {
        assert_eq!(
            validate_ollama_generation_request(
                &json!({"model":"m","prompt":"hello","keep_alive":"5m"}),
                OllamaGenerationEndpoint::Generate
            )
            .map_err(|(status, _)| status),
            Err(501)
        );
        assert_eq!(
            validate_ollama_generation_request(
                &json!({"model":"m","prompt":"hello","options":{"repeat_penalty":1.1}}),
                OllamaGenerationEndpoint::Generate
            )
            .map_err(|(status, _)| status),
            Err(501)
        );
        assert_eq!(
            ollama_generation_options(&json!({"options":{"num_ctx":0}}))
                .map_err(|(status, _)| status),
            Err(400)
        );
    }

    #[test]
    fn builds_ollama_chat_and_generate_prompts() {
        let chat = json!({
            "system":"Be brief",
            "messages":[{"role":"user","content":"hello"}]
        });
        assert_eq!(
            ollama_prompt(&chat, OllamaGenerationEndpoint::Chat).expect("chat prompt"),
            "\u{001e}system\nBe brief\n\u{001e}user\nhello"
        );
        let generate = json!({"system":"Be brief","prompt":"hello"});
        assert_eq!(
            ollama_prompt(&generate, OllamaGenerationEndpoint::Generate).expect("generate prompt"),
            "\u{001e}system\nBe brief\nhello"
        );
        assert_eq!(
            validate_ollama_generation_request(
                &json!({"model":"m","prompt":"hello","suffix":"completion"}),
                OllamaGenerationEndpoint::Generate
            )
            .map_err(|(status, _)| status),
            Err(501)
        );
    }

    #[test]
    fn maps_tool_calls_to_ollama_function_shape() {
        let call = json!({
            "id":"call_0",
            "type":"function",
            "function":{"name":"weather","arguments":"{\"city\":\"Manila\"}"}
        });
        assert_eq!(
            ollama_tool_call(&call),
            json!({"function":{"name":"weather","arguments":{"city":"Manila"}}})
        );
    }

    #[test]
    fn reports_only_available_ollama_statistics_and_rfc3339_timestamps() {
        let mut response = json!({});
        add_ollama_statistics(
            &mut response,
            &oid_runtime::GenerationStatistics::default(),
            10,
            0,
        );
        assert_eq!(response["total_duration"], 10);
        assert_eq!(response["load_duration"], 0);
        assert!(response.get("eval_count").is_none());
        let timestamp = iso8601_now().expect("timestamp");
        assert_eq!(timestamp.len(), 20);
        assert!(timestamp.ends_with('Z'));
    }
}
