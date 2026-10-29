//! Native loopback HTTP API for Marina.

use oid_runtime::{
    auth, GenerationMessage, GenerationOptions, GenerationRequest, GenerationStream,
    GenerationTermination, MarinaRuntime, RuntimeService,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_REQUEST_BODY_BYTES: usize = 1_048_576;
const MAX_CONTEXT_TOKENS: u64 = 32_768;
const CANCELLATION_CLEANUP_TIMEOUT: Duration = Duration::from_secs(30);

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

    fn snapshot(&self) -> (bool, String) {
        (
            self.ready.load(Ordering::Acquire),
            self.state
                .lock()
                .map_or_else(|_| "unknown".to_owned(), |state| state.clone()),
        )
    }
}

/// Serve the native HTTP API on an explicitly configured address.
pub fn serve(
    address: &str,
    runtime: &Arc<MarinaRuntime>,
    authentication_enabled: bool,
    default_model_id: Option<&String>,
    readiness: &Arc<Readiness>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(address)?;
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
    Ok(())
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
            let (ready, bootstrap) = readiness.snapshot();
            respond_json(
                &mut stream,
                200,
                json!({"status": "ok", "service_available": true, "inference_ready": ready, "bootstrap": bootstrap}),
            )
        }
        ("GET", "/readyz") => {
            let (ready, bootstrap) = readiness.snapshot();
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
            let data = runtime
                .model_list()
                .into_iter()
                .map(|model| {
                    json!({
                        "id": model.id,
                        "object": "model",
                        "owned_by": "marina",
                        "default": default_model_id.is_some_and(|id| id == model.id),
                        "resident": matches!(model.status, oid_runtime::ModelStatus::Loaded),
                        "estimated_memory_mib": model.memory_requirement_mb
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
            let request = match parse_ollama_json_body(&mut stream, &body)? {
                Some(request) => request,
                None => return Ok(()),
            };
            ollama_show(&mut stream, runtime, &request)
        }
        ("POST", "/api/embed" | "/api/embeddings") => {
            if !scope_allowed(&identity, auth::INFERENCE) {
                return respond_ollama_scope_error(&mut stream, auth::INFERENCE);
            }
            let legacy = path == "/api/embeddings";
            let request = match parse_ollama_json_body(&mut stream, &body)? {
                Some(request) => request,
                None => return Ok(()),
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
            let request = match parse_ollama_json_body(&mut stream, &body)? {
                Some(request) => request,
                None => return Ok(()),
            };
            generate_ollama(&mut stream, runtime, &request, &identity, owners, endpoint)
        }
        ("POST", "/v1/embeddings") => {
            if !scope_allowed(&identity, auth::INFERENCE) {
                return respond_scope_error(&mut stream, auth::INFERENCE);
            }
            let request = match parse_json_body(&mut stream, &body)? {
                Some(request) => request,
                None => return Ok(()),
            };
            embeddings(&mut stream, runtime, &request)
        }
        ("POST", path @ ("/v1/chat/completions" | "/v1/completions" | "/v1/responses")) => {
            if !scope_allowed(&identity, auth::INFERENCE) {
                return respond_scope_error(&mut stream, auth::INFERENCE);
            }
            let request = match parse_json_body(&mut stream, &body)? {
                Some(request) => request,
                None => return Ok(()),
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
        return respond_error(stream, 409, error.to_string());
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
                "size_vram": null,
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
        "idle_retention_seconds": snapshot.idle_retention_seconds
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
            "capabilities": model.capabilities,
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
    let prompt = match ollama_prompt(body, endpoint) {
        Ok(prompt) => prompt,
        Err((status, message)) => return respond_ollama_error(stream, status, message),
    };
    let options = match ollama_generation_options(body) {
        Ok(options) => options,
        Err((status, message)) => return respond_ollama_error(stream, status, message),
    };
    let tool_definitions = body
        .get("tools")
        .and_then(Value::as_array)
        .filter(|tools| !tools.is_empty());
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
    let generation = match runtime.generate(GenerationRequest {
        request_id: request_id.clone(),
        model_id: model.to_owned(),
        prompt,
        options,
    }) {
        Ok(generation) => generation,
        Err(error) => return respond_ollama_error(stream, 429, error.to_string()),
    };
    if let Ok(mut owners) = owners.lock() {
        owners.insert(request_id.clone(), identity.principal.clone());
    }
    let streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(true);
    if streaming {
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n"
        )?;
        stream.flush()?;
    }
    let mut output = String::new();
    loop {
        match generation.recv() {
            Ok(GenerationMessage::Token(token)) => {
                output.push_str(&token);
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
                        finish_owner(owners, &request_id);
                        return Err(error);
                    }
                }
            }
            Ok(GenerationMessage::Completed(result)) => {
                finish_owner(owners, &request_id);
                let tool_calls = parse_tool_calls(&output, &unique_request_id())
                    .map(|calls| calls.iter().map(ollama_tool_call).collect::<Vec<_>>());
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
                if let Some(tool_calls) = tool_calls {
                    if endpoint == OllamaGenerationEndpoint::Chat {
                        terminal["message"]["tool_calls"] = json!(tool_calls);
                    }
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
                finish_owner(owners, &request_id);
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
                finish_owner(owners, &request_id);
                if streaming {
                    return write_ndjson_line(stream, &json!({"error": error.to_string()}));
                }
                return respond_ollama_error(stream, 500, error.to_string());
            }
            Err(error) => {
                finish_owner(owners, &request_id);
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
    for field in ["keep_alive", "format", "think", "template", "context"] {
        if body.get(field).is_some_and(|value| !value.is_null()) {
            return Err((501, format!("request field `{field}` is not supported yet")));
        }
    }
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
                    .get("images")
                    .is_some_and(|images| !images.is_null())
                    || message
                        .get("content")
                        .is_some_and(|content| !content.is_string() && !content.is_null())
            }) {
                return Err((
                    501,
                    "only text message content is supported; image and multimodal inputs are unavailable".to_owned(),
                ));
            }
        }
        OllamaGenerationEndpoint::Generate => {
            if !body.get("prompt").is_some_and(Value::is_string) {
                return Err((400, "prompt must be a string".to_owned()));
            }
            if body.get("images").is_some_and(|images| !images.is_null()) {
                return Err((
                    501,
                    "image inputs are not supported by the selected runtime".to_owned(),
                ));
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

fn parse_tool_calls(output: &str, id_prefix: &str) -> Option<Vec<Value>> {
    let mut calls = Vec::new();
    let mut remainder = output;
    while let Some(start) = remainder.find("<tool_call>") {
        let after_start = &remainder[start + "<tool_call>".len()..];
        let end = after_start.find("</tool_call>")?;
        let value: Value = serde_json::from_str(after_start[..end].trim()).ok()?;
        calls.push(normalize_tool_call(&value, id_prefix, calls.len()));
        remainder = &after_start[end + "</tool_call>".len()..];
    }
    if !calls.is_empty() {
        return Some(calls);
    }
    let value: Value = serde_json::from_str(output.trim()).ok()?;
    if value.get("name").is_some() && value.get("arguments").is_some() {
        return Some(vec![normalize_tool_call(&value, id_prefix, 0)]);
    }
    None
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
    let Some(messages) = body.get("messages") else {
        return body
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "messages is required".to_owned());
    };
    let messages = messages
        .as_array()
        .ok_or_else(|| "messages must be an array".to_owned())?;
    if messages.is_empty() {
        return Err("messages must contain at least one message".to_owned());
    }

    let mut prompt = String::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| "each message requires a string role".to_owned())?;
        if !matches!(role, "system" | "developer" | "user" | "assistant" | "tool") {
            return Err(format!("unsupported message role: {role}"));
        }
        let content = if role == "assistant" {
            message
                .get("tool_calls")
                .filter(|calls| !calls.is_null())
                .map(|calls| format!("<tool_calls>{calls}</tool_calls>"))
                .or_else(|| {
                    message
                        .get("content")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .ok_or_else(|| "assistant message requires content or tool_calls".to_owned())?
        } else {
            let content = message
                .get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| "message content must be a string".to_owned())?;
            if role == "tool" {
                let name = message
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool");
                let call_id = message
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .map_or_else(String::new, |id| format!(" (tool_call_id: {id})"));
                format!("Tool result for {name}{call_id}: {content}")
            } else {
                content.to_owned()
            }
        };
        prompt.push('\u{001e}');
        prompt.push_str(role);
        prompt.push('\n');
        prompt.push_str(&content);
    }
    Ok(prompt)
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

fn validate_generation_request(
    body: &Value,
    endpoint: OpenAiGenerationEndpoint,
) -> Result<(), (u16, String)> {
    for field in [
        "response_format",
        "stream_options",
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
    if body.get("tool_choice").is_some() {
        if endpoint != OpenAiGenerationEndpoint::ChatCompletions {
            return Err((
                501,
                "`tool_choice` is only supported for Chat Completions".to_owned(),
            ));
        }
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
        if body.get("stream").and_then(Value::as_bool) == Some(true) {
            return Err((501, "streaming Responses are not supported yet".to_owned()));
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
                .is_some_and(|tokens| tokens > 0 && tokens <= u64::from(u32::MAX))
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
        if !value
            .as_u64()
            .is_some_and(|seed| seed <= u64::from(u32::MAX))
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
    let prompt_result = match endpoint {
        OpenAiGenerationEndpoint::ChatCompletions => build_chat_prompt(body),
        OpenAiGenerationEndpoint::Completions => build_completion_prompt(body),
        OpenAiGenerationEndpoint::Responses => build_response_prompt(body),
    };
    let mut prompt = match prompt_result {
        Ok(prompt) => prompt,
        Err(message) => return respond_error(stream, 400, message),
    };
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
    let request_id = body
        .get("request_id")
        .and_then(Value::as_str)
        .map_or_else(unique_request_id, str::to_owned);
    let timeout = body
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .map(|milliseconds| Duration::from_millis(milliseconds.min(300_000)));
    let context_size = body
        .get("context_size")
        .and_then(Value::as_u64)
        .unwrap_or(4096);
    if context_size == 0 || context_size > MAX_CONTEXT_TOKENS {
        return respond_error(
            stream,
            429,
            format!("context_size must be between 1 and {MAX_CONTEXT_TOKENS}"),
        );
    }
    let options = generation_options(body, u32::try_from(context_size).unwrap_or(u32::MAX));
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
        return respond_error(stream, 409, error.to_string());
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
    let generation = match runtime.generate(GenerationRequest {
        request_id: request_id.clone(),
        model_id: model.to_owned(),
        prompt,
        options,
    }) {
        Ok(stream_result) => stream_result,
        Err(error) => return respond_error(stream, 429, error.to_string()),
    };
    if let Ok(mut owners) = owners.lock() {
        owners.insert(request_id.clone(), identity.principal.clone());
    }
    let streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
    let defer_stream_headers = streaming && tool_definitions.is_some();
    let mut stream_headers_sent = false;
    if streaming && !defer_stream_headers {
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n"
        )?;
        stream.flush()?;
        stream_headers_sent = true;
    }
    let mut output = String::new();
    let deadline = timeout.map(|duration| Instant::now() + duration);
    loop {
        let message = deadline.map_or_else(
            || {
                generation
                    .recv()
                    .map_err(|_| std::sync::mpsc::RecvTimeoutError::Disconnected)
            },
            |deadline| {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                } else {
                    generation.recv_timeout(remaining)
                }
            },
        );
        match message {
            Ok(GenerationMessage::Token(token)) => {
                output.push_str(&token);
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
                            unreachable!("Responses streaming is rejected during validation")
                        }
                    };
                    write!(stream, "data: {event}\n\n")?;
                    stream.flush()?;
                }
            }
            Ok(GenerationMessage::Completed(result)) => {
                finish_owner(owners, &request_id);
                eprintln!(
                    "Talvik HTTP generation completed request_id={} model={} termination={:?} generated_tokens={}",
                    request_id,
                    model,
                    result.statistics.termination,
                    result.statistics.generated_tokens
                );
                let finish_reason = completion_finish_reason(result.statistics.termination);
                let termination_reason =
                    completion_termination_reason(result.statistics.termination);
                let tool_calls = if endpoint == OpenAiGenerationEndpoint::ChatCompletions
                    && tool_definitions.is_some()
                {
                    parse_tool_calls(&output, &unique_request_id())
                } else {
                    None
                };
                if let Err(message) = validate_generated_tool_calls(
                    &tool_choice,
                    tool_calls.as_deref(),
                    parallel_tool_calls,
                ) {
                    return respond_error(stream, 502, message);
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
                    return respond_json(
                        stream,
                        200,
                        json!({
                            "id": format!("resp_{}", result.request_id),
                            "object": "response",
                            "created_at": created,
                            "status": "completed",
                            "background": false,
                            "error": Value::Null,
                            "incomplete_details": Value::Null,
                            "instructions": body.get("instructions").cloned().unwrap_or(Value::Null),
                            "max_output_tokens": body.get("max_output_tokens").cloned().unwrap_or(Value::Null),
                            "model": model,
                            "output": [{
                                "id": format!("msg_{}", result.request_id),
                                "type": "message",
                                "status": "completed",
                                "role": "assistant",
                                "content": [{
                                    "type": "output_text",
                                    "text": output,
                                    "annotations": []
                                }]
                            }],
                            "parallel_tool_calls": false,
                            "previous_response_id": Value::Null,
                            "temperature": response_temperature,
                            "text": {"format": {"type": "text"}},
                            "top_p": response_top_p,
                            "truncation": "disabled",
                            "usage": usage,
                            "metadata": {},
                            "store": false
                        }),
                    );
                }
                if streaming {
                    if defer_stream_headers && !stream_headers_sent {
                        write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n"
                        )?;
                        stream.flush()?;
                    }
                    if let Some(tool_calls) = tool_calls.as_ref() {
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
                            "choices": [{"index": 0, "delta": {}, "finish_reason": finish_reason}],
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
                    write!(stream, "data: [DONE]\n\n")?;
                    return stream.flush();
                }
                let (choice, response_object) = match endpoint {
                    OpenAiGenerationEndpoint::ChatCompletions => {
                        let message = tool_calls.as_ref().map_or_else(
                            || json!({"role":"assistant","content":output}),
                            |calls| json!({"role":"assistant","content":Value::Null,"tool_calls":calls}),
                        );
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
                finish_owner(owners, &request_id);
                eprintln!(
                    "Talvik HTTP generation cancelled request_id={} model={} generated_tokens={}",
                    request_id, model, result.statistics.generated_tokens
                );
                if endpoint == OpenAiGenerationEndpoint::Responses {
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
                finish_owner(owners, &request_id);
                eprintln!(
                    "Talvik HTTP generation failed request_id={request_id} model={model} error={error}"
                );
                return respond_error(stream, 500, error.to_string());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                generation.cancel();
                finish_owner(owners, &request_id);
                eprintln!("Talvik HTTP generation timed_out request_id={request_id} model={model}");
                if drain_cancelled_generation(&generation) {
                    return respond_error(stream, 504, "generation request timed out".to_owned());
                }
                return respond_error(
                    stream,
                    500,
                    "generation cancellation did not complete within the cleanup deadline"
                        .to_owned(),
                );
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                finish_owner(owners, &request_id);
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
        .unwrap_or(128)
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
        add_ollama_statistics, build_chat_prompt, build_completion_prompt, build_response_prompt,
        completion_finish_reason, completion_termination_reason, generation_options, iso8601_now,
        ollama_generation_options, ollama_prompt, ollama_tool_call, parse_tool_calls,
        request_body_size_is_allowed, validate_generated_tool_calls, validate_generation_request,
        validate_ollama_generation_request, validate_openai_tool_history, GenerationTermination,
        OllamaGenerationEndpoint, OpenAiGenerationEndpoint, OpenAiToolChoice, MAX_CONTEXT_TOKENS,
        MAX_REQUEST_BODY_BYTES,
    };
    use serde_json::json;

    #[test]
    fn rejects_oversized_http_bodies() {
        assert!(request_body_size_is_allowed(MAX_REQUEST_BODY_BYTES));
        assert!(!request_body_size_is_allowed(MAX_REQUEST_BODY_BYTES + 1));
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
        .expect("tool calls");
        assert_eq!(calls[0]["id"], "call_request-2_0");
        assert_eq!(calls[1]["id"], "call_request-2_1");
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
    }

    #[test]
    fn maps_text_responses_input_and_rejects_unavailable_streaming() {
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
            Err(501)
        );
    }

    #[test]
    fn rejects_unsupported_options_and_invalid_generation_values() {
        assert_eq!(
            validate_generation_request(
                &json!({"response_format":{"type":"json_object"}}),
                OpenAiGenerationEndpoint::ChatCompletions
            ),
            Err((
                501,
                "request parameter `response_format` is not supported yet".to_owned()
            ))
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
        assert_eq!(options.temperature, 0.2);
        assert_eq!(options.top_p, 0.75);
        assert_eq!(options.top_k, 10);
        assert_eq!(options.seed, 7);
        assert_eq!(options.stop_sequences, ["END"]);
    }

    #[test]
    fn rejects_unavailable_ollama_features_and_options() {
        assert_eq!(
            validate_ollama_generation_request(
                &json!({"model":"m","prompt":"hello","images":["base64"]}),
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
