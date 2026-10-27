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
use oid_runtime::{GenerationMessage, GenerationRequest};
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
    let fields: Vec<_> = request.trim_end().split('\t').collect();
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
        "generate" if fields.len() >= 3 => {
            let request = GenerationRequest {
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
                model_id: fields[1].to_owned(),
                prompt: ipc::decode_field(&fields[4..].join("\t")),
                options: oid_runtime::GenerationOptions {
                    max_tokens: fields[2].parse().unwrap_or(128),
                    ..Default::default()
                },
            };
            match runtime.generate(request) {
                Ok(stream_result) => loop {
                    match stream_result.recv() {
                        Ok(GenerationMessage::Token(token)) => {
                            writeln!(stream, "TOKEN\t{}", ipc::encode_field(&token))?;
                        }
                        Ok(GenerationMessage::Completed(result)) => {
                            writeln!(
                                stream,
                                "DONE\tcompleted\t{}",
                                result.statistics.generated_tokens
                            )?;
                            break;
                        }
                        Ok(GenerationMessage::Cancelled(result)) => {
                            writeln!(
                                stream,
                                "DONE\tcancelled\t{}",
                                result.statistics.generated_tokens
                            )?;
                            break;
                        }
                        Ok(GenerationMessage::Failed(error)) => {
                            writeln!(stream, "ERROR\t{}", ipc::encode_field(&error.to_string()))?;
                            break;
                        }
                        Err(_) => break,
                    }
                    stream.flush()?;
                },
                Err(error) => respond_result(&mut stream, Err(error))?,
            }
        }
        _ => writeln!(stream, "ERROR\tunknown request")?,
    }
    Ok(())
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
