//! Persistent Talvik runtime service.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[path = "../build_info.rs"]
pub(crate) mod build_info;
#[path = "http.rs"]
mod http;
#[path = "../ipc.rs"]
pub(crate) mod ipc;

use oid_runtime::{config, logging, Runtime, RuntimeService};
use oid_runtime::{GenerationMessage, GenerationRequest, GenerationResult};
use oid_shared::RuntimeEvent;
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let _ = logging::initialize();
    eprintln!("{}", build_info::display("talvikd"));
    ipc::ensure_socket_parent()?;
    let configuration = config::load().map_err(std::io::Error::other)?;
    let runtime = Arc::new(Runtime::start(configuration)?);
    let default_model = config::default_model().map_err(std::io::Error::other)?;
    let default_model_id = default_model.as_ref().map(|model| model.id.clone());
    let readiness = Arc::new(http::Readiness::new());
    runtime.event_bus().publish(&RuntimeEvent::RuntimeStarted);
    let http_configuration = config::http_config().map_err(std::io::Error::other)?;
    if let Some(address) = http_configuration.address {
        let http_runtime = Arc::clone(&runtime);
        let authentication_enabled = http_configuration.authentication_enabled;
        let http_default_model_id = default_model_id.clone();
        let http_address = address.clone();
        let http_readiness = Arc::clone(&readiness);
        thread::spawn(move || {
            if let Err(error) = http::serve(
                &http_address,
                &http_runtime,
                authentication_enabled,
                http_default_model_id.as_ref(),
                &http_readiness,
            ) {
                eprintln!("Model runtime HTTP server failed: {error}");
            }
        });
        eprintln!("Model runtime HTTP listening on {address}");
    }
    let listener = ipc::bind_listener()?;
    eprintln!("Model runtime listening on {}", ipc::endpoint());
    let bootstrap_runtime = Arc::clone(&runtime);
    let bootstrap_readiness = Arc::clone(&readiness);
    thread::spawn(move || {
        let ready = load_default_model(
            bootstrap_runtime.as_ref(),
            default_model,
            &bootstrap_readiness,
        );
        if ready {
            bootstrap_readiness.set(true, "ready");
        }
    });
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let runtime = Arc::clone(&runtime);
                thread::spawn(move || {
                    if let Err(error) = handle(stream, runtime.as_ref()) {
                        eprintln!("Talvik request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("Model runtime accept failed: {error}"),
        }
    }
    Ok(())
}

fn load_default_model(
    runtime: &dyn RuntimeService,
    default_model: Option<config::DefaultModelConfig>,
    readiness: &http::Readiness,
) -> bool {
    readiness.set(false, "provisioning");
    let Some(default_model) = default_model else {
        eprintln!("No default model configured; inference is not ready");
        readiness.set(false, "no_default_model");
        return false;
    };

    if !default_model.auto_load {
        eprintln!("Default model auto-load disabled; inference is not ready");
        readiness.set(false, "auto_load_disabled");
        return false;
    }

    if runtime.model_inspect(&default_model.id).is_none() {
        if !default_model.auto_download {
            report_default_model_failure(
                runtime,
                &default_model.id,
                "model is not installed and automatic download is disabled",
            );
            readiness.set(false, "auto_download_disabled");
            return false;
        }
        let Some(source) = default_model.source.as_deref() else {
            report_default_model_failure(
                runtime,
                &default_model.id,
                "model is not installed and no acquisition source is configured",
            );
            readiness.set(false, "missing_download_source");
            return false;
        };
        eprintln!(
            "Default model {} is not installed; pulling it from {}",
            default_model.id, source
        );
        if let Err(error) = runtime.model_pull(
            source,
            Some(&default_model.id),
            default_model.checksum.as_deref(),
        ) {
            report_default_model_failure(runtime, &default_model.id, &error.to_string());
            readiness.set(false, "download_failed");
            return false;
        }
    }

    match runtime.model_load(&default_model.id) {
        Ok(()) => eprintln!("Loaded default model: {}", default_model.id),
        Err(error) => {
            report_default_model_failure(runtime, &default_model.id, &error.to_string());
            readiness.set(false, "load_failed");
            return false;
        }
    }
    true
}

fn report_default_model_failure(runtime: &dyn RuntimeService, model_id: &str, reason: &str) {
    let warning = format!("default model {model_id} was not loaded; continuing: {reason}");
    eprintln!("Warning: {warning}");
    runtime.publish(RuntimeEvent::WarningRaised(warning));
}

#[allow(clippy::too_many_lines)]
fn handle(mut stream: ipc::TalvikStream, runtime: &dyn RuntimeService) -> std::io::Result<()> {
    let mut request = String::new();
    BufReader::new(stream.try_clone()?).read_line(&mut request)?;
    let fields: Vec<_> = request.trim_end_matches(['\r', '\n']).split('\t').collect();
    match fields.first().copied().unwrap_or_default() {
        "status" => {
            let snapshot = runtime.snapshot();
            let client_version = fields.get(1).copied().unwrap_or_default();
            let identity = build_info::current();
            writeln!(
                stream,
                "OK status\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                snapshot.health,
                snapshot.backend,
                snapshot.loaded_model.clone().unwrap_or_default(),
                snapshot.loaded_models.join(","),
                identity.version,
                identity.git_sha,
                identity.dirty,
                identity.target,
                identity.profile,
                client_version != identity.version
            )?;
        }
        "model_list" => {
            for model in runtime.model_list() {
                writeln!(
                    stream,
                    "MODEL\t{}\t{}\t{:?}",
                    ipc::encode_field(&model.id),
                    ipc::encode_field(&model.name),
                    model.status
                )?;
            }
            writeln!(stream, "END")?;
        }
        "model_pull" if (2..=4).contains(&fields.len()) => {
            let source = ipc::decode_field(fields[1]);
            let model_id = fields
                .get(2)
                .filter(|value| !value.is_empty())
                .map(|value| ipc::decode_field(value));
            let checksum = fields
                .get(3)
                .filter(|value| !value.is_empty())
                .map(|value| ipc::decode_field(value));
            match runtime.model_pull(&source, model_id.as_deref(), checksum.as_deref()) {
                Ok(model) => {
                    writeln!(
                        stream,
                        "OK\tmodel_pull\t{}\t{}\t{:?}\t{}\t{}",
                        ipc::encode_field(&model.id),
                        ipc::encode_field(&model.name),
                        model.status,
                        ipc::encode_field(&model.location),
                        ipc::encode_field(model.checksum.as_deref().unwrap_or_default())
                    )?;
                }
                Err(error) => {
                    writeln!(
                        stream,
                        "ERROR\tmodel_pull\t{}",
                        ipc::encode_field(&error.to_string())
                    )?;
                }
            }
        }
        "model_load" if fields.len() == 2 => {
            respond_result(&mut stream, runtime.model_load(fields[1]))?;
        }
        "model_unload" if fields.len() == 2 => {
            respond_result(&mut stream, runtime.model_unload(fields[1]))?;
        }
        "cancel" if fields.len() == 2 => {
            runtime.cancel_generation_for(&ipc::decode_field(fields[1]));
            writeln!(stream, "OK cancelled")?;
        }
        "generate" | "generate_v2" => match parse_generation_request(&fields) {
            Ok(request) => run_generation(&mut stream, runtime, request)?,
            Err(error) => writeln!(stream, "ERROR\t{}", ipc::encode_field(&error))?,
        },
        _ => writeln!(stream, "ERROR\tunknown request")?,
    }
    Ok(())
}

fn parse_generation_request(fields: &[&str]) -> Result<GenerationRequest, String> {
    match fields.first().copied().unwrap_or_default() {
        "generate" if fields.len() >= 3 => {
            let max_tokens = fields[2]
                .parse()
                .map_err(|_| "invalid max_tokens in generation request".to_owned())?;
            Ok(GenerationRequest {
                request_id: fields.get(3).filter(|value| !value.is_empty()).map_or_else(
                    || {
                        format!(
                            "ipc-{}-{}",
                            std::process::id(),
                            REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
                        )
                    },
                    |value| ipc::decode_field(value),
                ),
                model_id: ipc::decode_field(fields[1]),
                prompt: ipc::decode_field(&fields.get(4..).unwrap_or_default().join("\t")),
                options: oid_runtime::GenerationOptions {
                    max_tokens,
                    ..Default::default()
                },
            })
        }
        "generate_v2" if fields.len() >= 11 => {
            let max_tokens = parse_generation_value(fields[3], "max_tokens")?;
            let temperature = parse_generation_value(fields[4], "temperature")?;
            let top_p = parse_generation_value(fields[5], "top_p")?;
            let top_k = parse_generation_value(fields[6], "top_k")?;
            let seed = parse_generation_value(fields[7], "seed")?;
            let context_size = parse_generation_value(fields[8], "context_size")?;
            let stop_sequences =
                serde_json::from_str(&ipc::decode_field(fields[9])).map_err(|error| {
                    format!("invalid stop_sequences in generation request: {error}")
                })?;
            let request_id = ipc::decode_field(fields[2]);
            if request_id.is_empty() {
                return Err("generation request_id must not be empty".to_owned());
            }
            Ok(GenerationRequest {
                request_id,
                model_id: ipc::decode_field(fields[1]),
                prompt: ipc::decode_field(&fields[10..].join("\t")),
                options: oid_runtime::GenerationOptions {
                    max_tokens,
                    temperature,
                    top_p,
                    top_k,
                    seed,
                    context_size,
                    stop_sequences,
                },
            })
        }
        _ => Err("invalid generation request".to_owned()),
    }
}

fn parse_generation_value<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid {name} in generation request"))
}

fn run_generation(
    stream: &mut ipc::TalvikStream,
    runtime: &dyn RuntimeService,
    request: GenerationRequest,
) -> std::io::Result<()> {
    match runtime.generate(request) {
        Ok(stream_result) => loop {
            match stream_result.recv() {
                Ok(GenerationMessage::Token(token)) => {
                    writeln!(stream, "TOKEN\t{}", ipc::encode_field(&token))?;
                    stream.flush()?;
                }
                Ok(GenerationMessage::Completed(result)) => {
                    write_generation_terminal(stream, "completed", &result)?;
                    break;
                }
                Ok(GenerationMessage::Cancelled(result)) => {
                    write_generation_terminal(stream, "cancelled", &result)?;
                    break;
                }
                Ok(GenerationMessage::Failed(error)) => {
                    writeln!(stream, "ERROR\t{}", ipc::encode_field(&error.to_string()))?;
                    break;
                }
                Err(_) => {
                    writeln!(
                        stream,
                        "ERROR\t{}",
                        ipc::encode_field("generation stream closed before completion")
                    )?;
                    break;
                }
            }
            stream.flush()?;
        },
        Err(error) => respond_result(stream, Err(error))?,
    }
    Ok(())
}

fn write_generation_terminal(
    stream: &mut ipc::TalvikStream,
    state: &str,
    result: &GenerationResult,
) -> std::io::Result<()> {
    let statistics = &result.statistics;
    let execution = statistics.execution.as_ref();
    let fields = vec![
        "DONE".to_owned(),
        state.to_owned(),
        ipc::encode_field(&result.generation_id),
        statistics.generated_tokens.to_string(),
        statistics.prompt_tokens.to_string(),
        statistics.context_tokens.to_string(),
        statistics.latency_ms.to_string(),
        statistics.inference_time_ms.to_string(),
        optional_ipc_field(statistics.backend_inference_time_ms),
        optional_ipc_field(statistics.time_to_first_token_ms),
        optional_ipc_field(statistics.prompt_evaluation_time_ms),
        optional_ipc_field(statistics.generation_time_ms),
        statistics.tokens_per_second.to_string(),
        statistics.metrics_known.to_string(),
        match statistics.termination {
            oid_runtime::GenerationTermination::Stop => "stop",
            oid_runtime::GenerationTermination::Length => "length",
            oid_runtime::GenerationTermination::ContextLength => "context_length",
        }
        .to_owned(),
        statistics.cache_hit.to_string(),
        statistics.reused_prompt_tokens.to_string(),
        execution.map_or_else(String::new, |value| {
            ipc::encode_field(value.backend.as_deref().unwrap_or_default())
        }),
        execution.map_or_else(String::new, |value| {
            ipc::encode_field(value.device.as_deref().unwrap_or_default())
        }),
        execution.map_or_else(String::new, |value| {
            optional_ipc_field(value.requested_offloaded_layers)
        }),
        execution.map_or_else(String::new, |value| {
            optional_ipc_field(value.offloaded_layers)
        }),
        execution.map_or_else(String::new, |value| {
            ipc::encode_field(value.fallback_reason.as_deref().unwrap_or_default())
        }),
    ];
    let line = fields.join("\t");
    writeln!(stream, "{line}")?;
    stream.flush()
}

fn optional_ipc_field<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(String::new, |value| value.to_string())
}

fn respond_result(
    stream: &mut ipc::TalvikStream,
    result: Result<(), oid_shared::RuntimeError>,
) -> std::io::Result<()> {
    match result {
        Ok(()) => writeln!(stream, "OK"),
        Err(error) => writeln!(stream, "ERROR\t{}", ipc::encode_field(&error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_generation_request;

    #[test]
    fn parses_ipc_v2_generation_options_and_escaped_prompt() {
        let fields = [
            "generate_v2",
            "qwen",
            "request-1",
            "32",
            "0.4",
            "0.8",
            "12",
            "7",
            "2048",
            r#"["END"]"#,
            "first line\\nsecond",
            " line",
        ];
        let request = parse_generation_request(&fields).expect("valid v2 generation request");

        assert_eq!(request.request_id, "request-1");
        assert_eq!(request.model_id, "qwen");
        assert_eq!(request.prompt, "first line\nsecond\t line");
        assert_eq!(request.options.max_tokens, 32);
        assert_eq!(request.options.temperature, 0.4);
        assert_eq!(request.options.top_p, 0.8);
        assert_eq!(request.options.top_k, 12);
        assert_eq!(request.options.seed, 7);
        assert_eq!(request.options.context_size, 2048);
        assert_eq!(request.options.stop_sequences, ["END"]);
    }

    #[test]
    fn rejects_invalid_generation_option_values() {
        let fields = [
            "generate_v2",
            "qwen",
            "request-1",
            "not-a-number",
            "0.4",
            "0.8",
            "12",
            "7",
            "2048",
            "[]",
            "hello",
        ];

        assert_eq!(
            parse_generation_request(&fields),
            Err("invalid max_tokens in generation request".to_owned())
        );
    }

    #[test]
    fn accepts_legacy_generation_requests_with_empty_prompts() {
        let request = parse_generation_request(&["generate", "qwen", "16"])
            .expect("legacy generation request");

        assert_eq!(request.model_id, "qwen");
        assert!(request.prompt.is_empty());
        assert_eq!(request.options.max_tokens, 16);
    }
}
