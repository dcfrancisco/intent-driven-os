//! Native loopback HTTP API for Marina.

use oid_runtime::{
    auth, GenerationMessage, GenerationOptions, GenerationRequest, GenerationStream,
    GenerationTermination, MarinaRuntime, RuntimeService,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_REQUEST_BODY_BYTES: usize = 1_048_576;
const MAX_CONTEXT_TOKENS: u64 = 32_768;
const CANCELLATION_CLEANUP_TIMEOUT: Duration = Duration::from_secs(30);

/// Serve the native HTTP API on an explicitly configured address.
pub fn serve(
    address: &str,
    runtime: &Arc<MarinaRuntime>,
    authentication_enabled: bool,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(address)?;
    let owners = Arc::new(Mutex::new(HashMap::<String, String>::new()));
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let runtime = Arc::clone(runtime);
                let owners = Arc::clone(&owners);
                std::thread::spawn(move || {
                    if let Err(error) =
                        handle(stream, runtime.as_ref(), authentication_enabled, &owners)
                    {
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
    owners: &Arc<Mutex<HashMap<String, String>>>,
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
    if path != "/healthz" && identity.is_none() {
        return respond_json(
            &mut stream,
            401,
            json!({"error": {"type": "authentication_error", "message": "missing or invalid bearer token"}}),
        );
    }
    let identity = identity.unwrap_or_else(|| auth::TokenIdentity {
        principal: "health-check".to_owned(),
        scopes: Vec::new(),
    });
    match (method, path) {
        ("GET", "/healthz") => respond_json(&mut stream, 200, json!({"status": "ok"})),
        ("GET", "/version" | "/v1/version") => respond_json(
            &mut stream,
            200,
            super::build_info::json("talvikd", runtime.snapshot().backend_version.as_deref()),
        ),
        ("GET", "/v1/models") => {
            if !scope_allowed(&identity, auth::MODELS_READ) {
                return respond_scope_error(&mut stream, auth::MODELS_READ);
            }
            let data = runtime
                .model_list()
                .into_iter()
                .map(|model| json!({"id": model.id, "object": "model", "owned_by": "marina"}))
                .collect::<Vec<_>>();
            respond_json(&mut stream, 200, json!({"object": "list", "data": data}))
        }
        ("GET", "/api/tags") => {
            if !scope_allowed(&identity, auth::MODELS_READ) {
                return respond_scope_error(&mut stream, auth::MODELS_READ);
            }
            respond_json(&mut stream, 200, ollama_tags(runtime))
        }
        ("GET", "/api/ps") => {
            if !scope_allowed(&identity, auth::MODELS_READ) {
                return respond_scope_error(&mut stream, auth::MODELS_READ);
            }
            respond_json(&mut stream, 200, ollama_ps(runtime))
        }
        ("POST", "/v1/embeddings") => {
            if !scope_allowed(&identity, auth::INFERENCE) {
                return respond_scope_error(&mut stream, auth::INFERENCE);
            }
            let request: Value =
                serde_json::from_slice(&body).map_err(|error| invalid_data(error.to_string()))?;
            embeddings(&mut stream, runtime, &request)
        }
        ("POST", "/v1/chat/completions" | "/v1/completions") => {
            if !scope_allowed(&identity, auth::INFERENCE) {
                return respond_scope_error(&mut stream, auth::INFERENCE);
            }
            let request: Value =
                serde_json::from_slice(&body).map_err(|error| invalid_data(error.to_string()))?;
            generate(&mut stream, runtime, &request, &identity, owners)
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
        _ => respond_error(&mut stream, 404, "not found".to_owned()),
    }
}

fn embeddings(
    stream: &mut TcpStream,
    runtime: &dyn RuntimeService,
    body: &Value,
) -> std::io::Result<()> {
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_data("model is required"))?;
    let inputs = match body.get("input") {
        Some(Value::String(value)) => vec![value.clone()],
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid_data("input array values must be strings"))
            })
            .collect::<Result<Vec<_>, _>>()?,
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

fn ollama_tags(runtime: &dyn RuntimeService) -> Value {
    let models = runtime
        .model_list()
        .into_iter()
        .map(|model| {
            json!({
                "name": model.id.clone(),
                "model": model.id,
                "modified_at": null,
                "size": null,
                "memory_requirement_bytes": model.memory_requirement_mb.saturating_mul(1_048_576),
                "digest": model.checksum,
                "details": {
                    "family": model.family,
                    "quantization_level": model.quantization,
                    "parameter_size": null,
                    "format": "gguf"
                }
            })
        })
        .collect::<Vec<_>>();
    json!({"models": models})
}

fn ollama_ps(runtime: &dyn RuntimeService) -> Value {
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
                "idle_seconds": resident.idle_seconds,
                "details": {
                    "family": model.map_or("unknown", |model| model.family.as_str()),
                    "quantization_level": model.map_or("unknown", |model| model.quantization.as_str()),
                    "format": "gguf"
                }
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

fn parse_tool_calls(output: &str) -> Option<Vec<Value>> {
    let mut calls = Vec::new();
    let mut remainder = output;
    while let Some(start) = remainder.find("<tool_call>") {
        let after_start = &remainder[start + "<tool_call>".len()..];
        let end = after_start.find("</tool_call>")?;
        let value: Value = serde_json::from_str(after_start[..end].trim()).ok()?;
        calls.push(normalize_tool_call(&value, calls.len()));
        remainder = &after_start[end + "</tool_call>".len()..];
    }
    if !calls.is_empty() {
        return Some(calls);
    }
    let value: Value = serde_json::from_str(output.trim()).ok()?;
    if value.get("name").is_some() && value.get("arguments").is_some() {
        return Some(vec![normalize_tool_call(&value, 0)]);
    }
    None
}

fn normalize_tool_call(value: &Value, index: usize) -> Value {
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
        "id": format!("call_{index}"),
        "type": "function",
        "function": {"name": name, "arguments": arguments}
    })
}

#[allow(clippy::too_many_lines)]
fn generate(
    stream: &mut TcpStream,
    runtime: &dyn RuntimeService,
    body: &Value,
    identity: &auth::TokenIdentity,
    owners: &Arc<Mutex<HashMap<String, String>>>,
) -> std::io::Result<()> {
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_data("model is required"))?;
    let tool_definitions = body
        .get("tools")
        .and_then(Value::as_array)
        .filter(|tools| !tools.is_empty());
    let mut prompt = if let Some(messages) = body.get("messages").and_then(Value::as_array) {
        let mut prompt = String::new();
        for message in messages {
            let role = message
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("user");
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
                    .ok_or_else(|| {
                        invalid_data("assistant message requires content or tool_calls")
                    })?
            } else {
                let content = message
                    .get("content")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid_data("message content must be a string"))?;
                if role == "tool" {
                    let name = message
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("tool");
                    format!("Tool result for {name}: {content}")
                } else {
                    content.to_owned()
                }
            };
            prompt.push('\u{001e}');
            prompt.push_str(role);
            prompt.push('\n');
            prompt.push_str(&content);
        }
        prompt
    } else {
        body.get("prompt")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    if let Some(tools) = tool_definitions {
        prompt.insert_str(
            0,
            &format!("\u{001e}__talvik_tools\n{}", Value::Array(tools.clone())),
        );
    }
    if prompt.is_empty() {
        return respond_error(stream, 400, "prompt or messages is required".to_owned());
    }
    let request_id = body
        .get("request_id")
        .and_then(Value::as_str)
        .map_or_else(unique_request_id, str::to_owned);
    let max_tokens = body
        .get("max_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(128)
        .min(u64::from(u32::MAX));
    let max_tokens = u32::try_from(max_tokens).unwrap_or(u32::MAX);
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
    if let Err(error) = runtime.model_load(model) {
        return respond_error(stream, 409, error.to_string());
    }
    if tool_definitions.is_some() {
        let support = runtime.tool_support();
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
        options: GenerationOptions {
            max_tokens,
            context_size: u32::try_from(context_size).unwrap_or(u32::MAX),
            ..Default::default()
        },
    }) {
        Ok(stream_result) => stream_result,
        Err(error) => return respond_error(stream, 429, error.to_string()),
    };
    if let Ok(mut owners) = owners.lock() {
        owners.insert(request_id.clone(), identity.principal.clone());
    }
    let streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
    if streaming {
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n"
        )?;
        stream.flush()?;
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
                    let event = json!({
                        "id": request_id.clone(),
                        "object": "chat.completion.chunk",
                        "model": model,
                        "choices": [{"index": 0, "delta": {"content": token}, "finish_reason": Value::Null}]
                    });
                    write!(stream, "data: {event}\n\n")?;
                    stream.flush()?;
                }
            }
            Ok(GenerationMessage::Completed(result)) => {
                finish_owner(owners, &request_id);
                let finish_reason = completion_finish_reason(result.statistics.termination);
                let termination_reason =
                    completion_termination_reason(result.statistics.termination);
                if streaming {
                    if let Some(tool_calls) = parse_tool_calls(&output) {
                        let event = json!({
                            "id": result.request_id,
                            "object": "chat.completion.chunk",
                            "model": model,
                            "choices": [{"index": 0, "delta": {"tool_calls": tool_calls}, "finish_reason": "tool_calls"}]
                        });
                        write!(stream, "data: {event}\n\n")?;
                    } else {
                        let event = json!({
                            "id": result.request_id,
                            "object": "chat.completion.chunk",
                            "model": model,
                            "choices": [{"index": 0, "delta": {}, "finish_reason": finish_reason}],
                            "termination_reason": termination_reason
                        });
                        write!(stream, "data: {event}\n\n")?;
                    }
                    write!(stream, "data: [DONE]\n\n")?;
                    return stream.flush();
                }
                let tool_calls = parse_tool_calls(&output);
                let message = tool_calls.as_ref().map_or_else(
                    || json!({"role":"assistant","content":output}),
                    |calls| json!({"role":"assistant","content":Value::Null,"tool_calls":calls}),
                );
                return respond_json(
                    stream,
                    200,
                    json!({"id": result.request_id, "generation_id": result.generation_id, "object": "chat.completion", "model": model, "choices": [{"index": 0, "message": message, "finish_reason": if tool_calls.is_some() {"tool_calls"} else {finish_reason}}], "termination_reason": termination_reason, "usage": {"prompt_tokens": result.statistics.prompt_tokens, "completion_tokens": result.statistics.generated_tokens, "total_tokens": result.statistics.context_tokens}, "metrics": {"tokens_per_second": result.statistics.metrics_known.then_some(result.statistics.tokens_per_second), "latency_ms": result.statistics.metrics_known.then_some(result.statistics.latency_ms), "inference_time_ms": result.statistics.metrics_known.then_some(result.statistics.inference_time_ms), "context_tokens": result.statistics.metrics_known.then_some(result.statistics.context_tokens)}}),
                );
            }
            Ok(GenerationMessage::Cancelled(result)) => {
                finish_owner(owners, &request_id);
                if streaming {
                    write!(stream, "data: [DONE]\n\n")?;
                    return stream.flush();
                }
                return respond_json(
                    stream,
                    200,
                    json!({"id": result.request_id, "generation_id": result.generation_id, "object": "chat.completion", "model": model, "choices": [{"index": 0, "message": {"role": "assistant", "content": output}, "finish_reason": "cancelled"}], "usage": {"completion_tokens": result.statistics.generated_tokens}, "metrics": {"tokens_per_second": result.statistics.metrics_known.then_some(result.statistics.tokens_per_second), "latency_ms": result.statistics.metrics_known.then_some(result.statistics.latency_ms), "inference_time_ms": result.statistics.metrics_known.then_some(result.statistics.inference_time_ms), "context_tokens": result.statistics.metrics_known.then_some(result.statistics.context_tokens)}}),
                );
            }
            Ok(GenerationMessage::Failed(error)) => {
                finish_owner(owners, &request_id);
                return respond_error(stream, 500, error.to_string());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                generation.cancel();
                finish_owner(owners, &request_id);
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
    respond_json(
        stream,
        status,
        json!({"error": {"type": "marina_error", "message": message}}),
    )
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
        504 => "Gateway Timeout",
        500 => "Internal Server Error",
        _ => "Error",
    }
}

fn invalid_data(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
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
        completion_finish_reason, completion_termination_reason, parse_tool_calls,
        request_body_size_is_allowed, GenerationTermination, MAX_CONTEXT_TOKENS,
        MAX_REQUEST_BODY_BYTES,
    };

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
        )
        .expect("tool call");
        assert_eq!(calls[0]["type"], "function");
        assert_eq!(calls[0]["function"]["name"], "get_weather");
        assert_eq!(calls[0]["function"]["arguments"], r#"{"city":"Manila"}"#);
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
}
