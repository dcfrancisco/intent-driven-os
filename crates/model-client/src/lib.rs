//! Provider-neutral OID model-client contract and Marina HTTP implementation.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A model visible to a client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDescriptor {
    /// Stable Marina model identifier.
    pub id: String,
}

/// A provider-neutral chat request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatRequest {
    /// Model identifier.
    pub model: String,
    /// User prompt text.
    pub prompt: String,
    /// Stable request identifier.
    pub request_id: String,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: u32,
    /// Context window requested by the caller.
    pub context_size: u32,
}

/// A provider-neutral chat response.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatResponse {
    /// Generated assistant text.
    pub text: String,
    /// Model that served the request.
    pub model: String,
    /// Provider request identifier.
    pub request_id: String,
    /// Marina-owned generation identity, separate from the caller request ID.
    pub generation_id: String,
    /// Measurements returned by Marina. Missing fields remain unknown.
    pub metrics: RemoteGenerationMetrics,
}

/// Generation measurements returned by Marina's native API.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteGenerationMetrics {
    /// Prompt token count, if reported.
    pub prompt_tokens: Option<u64>,
    /// Generated token count, if reported.
    pub generated_tokens: Option<u64>,
    /// Context token count, if reported.
    pub context_tokens: Option<u64>,
    /// Tokens per second, if reported.
    pub tokens_per_second: Option<f64>,
    /// End-to-end latency in milliseconds, if reported.
    pub latency_ms: Option<u128>,
    /// Backend inference time in milliseconds, if reported.
    pub inference_time_ms: Option<u128>,
}

/// Structured failure from a model client.
#[derive(Debug)]
pub enum ModelClientError {
    /// Marina could not be reached.
    Unavailable(String),
    /// Credentials were rejected.
    Unauthorized,
    /// The requested model or endpoint does not exist.
    NotFound(String),
    /// The request was rejected by admission or validation.
    Rejected(String),
    /// The request exceeded its deadline.
    Timeout,
    /// The caller cancelled the request.
    Cancelled,
    /// Marina returned an invalid response.
    MalformedResponse(String),
    /// Local request construction or I/O failed.
    Transport(String),
}

impl std::fmt::Display for ModelClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(message) => write!(formatter, "Marina unavailable: {message}"),
            Self::Unauthorized => formatter.write_str("Marina authentication failed"),
            Self::NotFound(message) => write!(formatter, "Marina resource not found: {message}"),
            Self::Rejected(message) => write!(formatter, "Marina request rejected: {message}"),
            Self::Timeout => formatter.write_str("Marina request timed out"),
            Self::Cancelled => formatter.write_str("Marina request cancelled"),
            Self::MalformedResponse(message) => {
                write!(formatter, "Marina malformed response: {message}")
            }
            Self::Transport(message) => write!(formatter, "Marina transport error: {message}"),
        }
    }
}

impl std::error::Error for ModelClientError {}

/// Provider-neutral model client interface used by OID.
pub trait ModelClient {
    /// Discover models available to the caller.
    ///
    /// # Errors
    ///
    /// Returns a structured provider or transport error.
    fn list_models(&self) -> Result<Vec<ModelDescriptor>, ModelClientError>;
    /// Submit one non-streaming chat request.
    ///
    /// # Errors
    ///
    /// Returns a structured provider, timeout, cancellation, or transport error.
    fn chat(
        &self,
        request: &ChatRequest,
        deadline: Duration,
        cancelled: &AtomicBool,
    ) -> Result<ChatResponse, ModelClientError>;
}

/// Configuration for a local Marina HTTP client.
#[derive(Clone)]
pub struct MarinaHttpConfig {
    /// HTTP base URL, for example `http://127.0.0.1:11434`.
    pub base_url: String,
    /// Optional bearer credential. Never included in diagnostics.
    pub bearer_token: Option<String>,
    /// Connection/read timeout for discovery requests.
    pub timeout: Duration,
}

impl std::fmt::Debug for MarinaHttpConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MarinaHttpConfig")
            .field("base_url", &self.base_url)
            .field(
                "bearer_token",
                &self.bearer_token.as_ref().map(|_| "<redacted>"),
            )
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl Default for MarinaHttpConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:11434".to_owned(),
            bearer_token: None,
            timeout: Duration::from_secs(10),
        }
    }
}

/// Native Marina HTTP client.
#[derive(Clone, Debug)]
pub struct MarinaHttpClient {
    config: MarinaHttpConfig,
    host: String,
    port: u16,
    cancelled_generation_ids: Arc<Mutex<HashMap<String, String>>>,
}

impl MarinaHttpClient {
    /// Construct a client and reject non-HTTP or non-loopback endpoints.
    ///
    /// # Errors
    ///
    /// Returns an error when the endpoint is malformed or not loopback-only.
    pub fn new(config: MarinaHttpConfig) -> Result<Self, ModelClientError> {
        let base_url = config.base_url.clone();
        let authority = base_url
            .strip_prefix("http://")
            .ok_or_else(|| {
                ModelClientError::Transport(
                    "only http:// loopback Marina endpoints are supported".to_owned(),
                )
            })?
            .trim_end_matches('/');
        let (host, port) = authority.rsplit_once(':').ok_or_else(|| {
            ModelClientError::Transport("Marina URL must include a port".to_owned())
        })?;
        let port = port.parse::<u16>().map_err(|_| {
            ModelClientError::Transport("Marina URL has an invalid port".to_owned())
        })?;
        if host != "127.0.0.1" && host != "localhost" && host != "[::1]" {
            return Err(ModelClientError::Transport("remote Marina endpoints require a trusted TLS proxy and are not enabled in this client".to_owned()));
        }
        Ok(Self {
            config,
            host: host.trim_matches(['[', ']']).to_owned(),
            port,
            cancelled_generation_ids: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Return the Marina generation ID observed for a cancelled request.
    #[must_use]
    pub fn cancelled_generation_id(&self, request_id: &str) -> Option<String> {
        self.cancelled_generation_ids
            .lock()
            .ok()
            .and_then(|ids| ids.get(request_id).cloned())
    }

    /// Discover a client from OID environment configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured endpoint is invalid.
    pub fn from_environment() -> Result<Self, ModelClientError> {
        let mut config = MarinaHttpConfig::default();
        if let Ok(url) = std::env::var("OID_MARINA_URL") {
            config.base_url = url;
        }
        config.bearer_token = std::env::var("OID_MARINA_TOKEN").ok();
        Self::new(config)
    }
}

impl ModelClient for MarinaHttpClient {
    fn list_models(&self) -> Result<Vec<ModelDescriptor>, ModelClientError> {
        let response = self.request("GET", "/v1/models", None, self.config.timeout, None)?;
        let data = response
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ModelClientError::MalformedResponse("models data is missing".to_owned())
            })?;
        data.iter()
            .map(|model| {
                model
                    .get("id")
                    .and_then(Value::as_str)
                    .map(|id| ModelDescriptor { id: id.to_owned() })
                    .ok_or_else(|| {
                        ModelClientError::MalformedResponse("model id is missing".to_owned())
                    })
            })
            .collect()
    }

    fn chat(
        &self,
        request: &ChatRequest,
        deadline: Duration,
        cancelled: &AtomicBool,
    ) -> Result<ChatResponse, ModelClientError> {
        let body = json!({
            "model": request.model,
            "messages": [{"role": "user", "content": request.prompt}],
            "request_id": request.request_id,
            "max_tokens": request.max_tokens,
            "context_size": request.context_size,
            "stream": false,
        });
        let started = Instant::now();
        let response = self.request(
            "POST",
            "/v1/chat/completions",
            Some(body),
            deadline,
            Some((cancelled, request.request_id.as_str())),
        )?;
        if cancelled.load(Ordering::SeqCst) {
            return Err(ModelClientError::Cancelled);
        }
        if started.elapsed() > deadline {
            return Err(ModelClientError::Timeout);
        }
        let choice = response
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .ok_or_else(|| ModelClientError::MalformedResponse("choices are missing".to_owned()))?;
        let text = choice
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ModelClientError::MalformedResponse("assistant content is missing".to_owned())
            })?;
        Ok(ChatResponse {
            text: text.to_owned(),
            model: response
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(&request.model)
                .to_owned(),
            request_id: response
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(&request.request_id)
                .to_owned(),
            generation_id: response
                .get("generation_id")
                .and_then(Value::as_str)
                .or_else(|| response.get("id").and_then(Value::as_str))
                .unwrap_or(&request.request_id)
                .to_owned(),
            metrics: parse_metrics(&response),
        })
    }
}

fn parse_metrics(response: &Value) -> RemoteGenerationMetrics {
    let metrics = response.get("metrics").unwrap_or(response);
    let usage = response.get("usage").unwrap_or(response);
    RemoteGenerationMetrics {
        prompt_tokens: usage.get("prompt_tokens").and_then(Value::as_u64),
        generated_tokens: usage
            .get("completion_tokens")
            .and_then(Value::as_u64)
            .or_else(|| metrics.get("generated_tokens").and_then(Value::as_u64)),
        context_tokens: metrics.get("context_tokens").and_then(Value::as_u64),
        tokens_per_second: metrics.get("tokens_per_second").and_then(Value::as_f64),
        latency_ms: metrics
            .get("latency_ms")
            .and_then(Value::as_u64)
            .map(u128::from),
        inference_time_ms: metrics
            .get("inference_time_ms")
            .and_then(Value::as_u64)
            .map(u128::from),
    }
}

impl MarinaHttpClient {
    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        timeout: Duration,
        cancellation: Option<(&AtomicBool, &str)>,
    ) -> Result<Value, ModelClientError> {
        let address = format!("{}:{}", self.host, self.port);
        let mut stream = TcpStream::connect_timeout(
            &address
                .to_socket_addrs()
                .map_err(|error| ModelClientError::Unavailable(error.to_string()))?
                .next()
                .ok_or_else(|| {
                    ModelClientError::Unavailable("Marina address did not resolve".to_owned())
                })?,
            timeout,
        )
        .map_err(|error| ModelClientError::Unavailable(error.to_string()))?;
        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .map_err(|error| io_error(&error))?;
        let payload = body.map(|value| value.to_string()).unwrap_or_default();
        let authorization = self
            .config
            .bearer_token
            .as_ref()
            .map_or(String::new(), |token| {
                format!("Authorization: Bearer {token}\r\n")
            });
        write!(stream, "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n{authorization}Content-Length: {}\r\nContent-Type: application/json\r\n\r\n{payload}", self.host, payload.len()).map_err(|error| io_error(&error))?;
        stream.flush().map_err(|error| io_error(&error))?;
        let started = Instant::now();
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            if let Some((cancelled, request_id)) = cancellation {
                if cancelled.load(Ordering::SeqCst) {
                    let _ = self.cancel(request_id);
                    return Err(ModelClientError::Cancelled);
                }
            }
            if started.elapsed() >= timeout {
                return Err(ModelClientError::Timeout);
            }
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => bytes.extend_from_slice(&buffer[..count]),
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        || error.kind() == io::ErrorKind::TimedOut => {}
                Err(error) => return Err(ModelClientError::Transport(error.to_string())),
            }
        }
        parse_response(&bytes)
    }

    fn cancel(&self, request_id: &str) -> Result<(), ModelClientError> {
        let response = self.request(
            "POST",
            &format!("/v1/generations/{request_id}/cancel"),
            None,
            Duration::from_secs(1),
            None,
        )?;
        if let Some(generation_id) = response.get("generation_id").and_then(Value::as_str) {
            if let Ok(mut ids) = self.cancelled_generation_ids.lock() {
                ids.insert(request_id.to_owned(), generation_id.to_owned());
            }
        }
        Ok(())
    }
}

fn parse_response(bytes: &[u8]) -> Result<Value, ModelClientError> {
    let separator = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| {
            ModelClientError::MalformedResponse("HTTP headers are incomplete".to_owned())
        })?;
    let headers = String::from_utf8_lossy(&bytes[..separator]);
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| ModelClientError::MalformedResponse("HTTP status is missing".to_owned()))?;
    let body = &bytes[separator + 4..];
    let value: Value = serde_json::from_slice(body)
        .map_err(|error| ModelClientError::MalformedResponse(error.to_string()))?;
    if !(200..300).contains(&status) {
        let message = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("Marina rejected the request")
            .to_owned();
        return Err(match status {
            401 => ModelClientError::Unauthorized,
            404 => ModelClientError::NotFound(message),
            504 => ModelClientError::Timeout,
            _ => ModelClientError::Rejected(message),
        });
    }
    Ok(value)
}

fn io_error(error: &io::Error) -> ModelClientError {
    ModelClientError::Transport(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        parse_metrics, parse_response, MarinaHttpClient, MarinaHttpConfig, ModelClientError,
    };
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn rejects_remote_endpoints_and_redacts_tokens() {
        let error = MarinaHttpClient::new(MarinaHttpConfig {
            base_url: "http://example.test:11434".to_owned(),
            bearer_token: Some("secret".to_owned()),
            timeout: Duration::from_secs(1),
        })
        .expect_err("remote endpoint");
        assert!(!error.to_string().contains("secret"));
    }

    #[test]
    fn maps_auth_and_admission_errors() {
        let unauthorized =
            parse_response(b"HTTP/1.1 401 Unauthorized\r\n\r\n{\"error\":{\"message\":\"no\"}}")
                .expect_err("unauthorized");
        assert!(matches!(unauthorized, ModelClientError::Unauthorized));
        let rejected = parse_response(
            b"HTTP/1.1 429 Too Many Requests\r\n\r\n{\"error\":{\"message\":\"busy\"}}",
        )
        .expect_err("rejected");
        assert!(matches!(rejected, ModelClientError::Rejected(message) if message == "busy"));
    }

    #[test]
    fn preserves_optional_metrics_as_unknown_when_absent() {
        let metrics = parse_metrics(&json!({
            "usage": {"prompt_tokens": 3, "completion_tokens": 5},
            "metrics": {"latency_ms": 12}
        }));
        assert_eq!(metrics.prompt_tokens, Some(3));
        assert_eq!(metrics.generated_tokens, Some(5));
        assert_eq!(metrics.latency_ms, Some(12));
        assert_eq!(metrics.context_tokens, None);
        assert_eq!(metrics.tokens_per_second, None);
    }
}
