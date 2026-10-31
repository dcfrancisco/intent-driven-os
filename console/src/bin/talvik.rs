//! Talvik local model service client.

#![forbid(unsafe_code)]

#[path = "../build_info.rs"]
mod build_info;
#[path = "../chat_ui.rs"]
mod chat_ui;
#[path = "../ipc.rs"]
mod ipc;
#[path = "../ipc_runtime.rs"]
mod ipc_runtime;
use oid_runtime::{GenerationMessage, GenerationOptions, GenerationRequest, RuntimeService};
use oid_shared::EventBus;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

const HELP: &str = "Talvik local model runtime

Usage:
  talvik [--json] [--plain] <command> [arguments]

Commands:
  status                         Show daemon health and resident models
  models                         List discovered models and lifecycle state
  import <source> [id] [sha256]  Acquire and verify a GGUF artifact
  vision <model-id> <projector>  Pair a GGUF vision projector with a model
  vision-clear <model-id>        Remove a model's vision projector pairing
  load <model-id>                Load a model into residency
  unload <model-id>              Unload one resident model
  run <model-id> <prompt>        Stream a prompt through a selected model
  run --default <prompt>         Stream a prompt through the loaded default model
  run --route <prompt>           Use explicitly enabled ordered fallback
  cancel <request-id>            Request cancellation of a generation
  auth token create [label]      Create a scoped local API token
  routing status                 Show routing configuration

Use `talvik <command> --help` for command-specific usage. JSON output is
available for status, models, import, load, and unload with --json.

Chat uses terminal colors and progress automatically. Use --plain or
TALVIK_PLAIN=1 for raw text; NO_COLOR=1 disables colors. Pipes stay plain.

Routing is off by default. Enable ordered fallback with
TALVIK_ROUTING_ENABLED=1 and TALVIK_ROUTE_MODELS=model-a,model-b.";

fn main() {
    if let Err(error) = run() {
        eprintln!("talvik: {error}");
        std::process::exit(1);
    }
}

fn global_options(arguments: &mut Vec<String>) -> (bool, bool) {
    let mut json = false;
    let mut plain = false;
    while let Some(flag) = arguments.first() {
        match flag.as_str() {
            "--json" => json = true,
            "--plain" => plain = true,
            _ => break,
        }
        arguments.remove(0);
    }
    (json, plain)
}

fn print_version(json_output: bool) -> Result<(), serde_json::Error> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&build_info::json("talvik", None))?
        );
    } else {
        println!("{}", build_info::display("talvik"));
    }
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let (json_output, plain) = global_options(&mut arguments);
    let mut args = arguments.into_iter().peekable();
    let Some(command) = args.next() else {
        println!("{HELP}");
        return Ok(());
    };
    match command.as_str() {
        "--help" | "-h" | "help" => println!("{HELP}"),
        "--version" | "-V" => {
            let _ = args.next();
            println!("{}", build_info::display("talvik"));
        }
        "version" if args.next().is_none() => print_version(json_output)?,
        "status" if args.next().is_none() => request_formatted(
            &format!("status\t{}", build_info::current().version),
            json_output,
            "status",
        )?,
        "models" if args.next().is_none() => {
            request_formatted("model_list", json_output, "models")?;
        }
        "import" => {
            if args.peek().map(String::as_str) == Some("--help") {
                println!("Usage: talvik import <source> [model-id] [sha256]");
            } else {
                import(args, json_output)?;
            }
        }
        "load" => {
            let first = required(&mut args, "model id")?;
            if first == "--help" {
                println!("Usage: talvik load <model-id>");
            } else {
                request_formatted(
                    &format!("model_load\t{}", ipc::encode_field(&first)),
                    json_output,
                    "load",
                )?;
            }
        }
        "unload" => {
            let first = required(&mut args, "model id")?;
            if first == "--help" {
                println!("Usage: talvik unload <model-id>");
            } else {
                request_formatted(
                    &format!("model_unload\t{}", ipc::encode_field(&first)),
                    json_output,
                    "unload",
                )?;
            }
        }
        "vision" => {
            let model_id = required(&mut args, "model id")?;
            let projector = required(&mut args, "projector path")?;
            if args.next().is_some() {
                return Err("vision accepts a model id and projector path".into());
            }
            request_formatted(
                &format!(
                    "model_vision\t{}\t{}",
                    ipc::encode_field(&model_id),
                    ipc::encode_field(&projector)
                ),
                json_output,
                "vision",
            )?;
        }
        "vision-clear" => {
            let model_id = required(&mut args, "model id")?;
            if args.next().is_some() {
                return Err("vision-clear accepts one model id".into());
            }
            request_formatted(
                &format!("model_vision\t{}\t", ipc::encode_field(&model_id)),
                json_output,
                "vision",
            )?;
        }
        "cancel" => request(&format!(
            "cancel\t{}",
            ipc::encode_field(&required(&mut args, "request id")?)
        ))?,
        "run" => run_prompt(args, plain || json_output)?,
        "routing" if args.next().as_deref() == Some("status") && args.next().is_none() => {
            routing_status();
        }
        "auth" => auth(args)?,
        "status" | "models" if args.next().as_deref() == Some("--help") => {
            println!("Usage: talvik {command} [--json]");
        }
        _ => return Err(format!("unknown command `{command}`\n\n{HELP}").into()),
    }
    Ok(())
}

fn required(
    args: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    args.next().ok_or_else(|| format!("{name} required").into())
}

fn import(
    mut args: impl Iterator<Item = String>,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = required(&mut args, "model source")?;
    let mut line = format!("model_pull\t{}", ipc::encode_field(&source));
    if let Some(id) = args.next() {
        line.push('\t');
        line.push_str(&ipc::encode_field(&id));
        if let Some(checksum) = args.next() {
            line.push('\t');
            line.push_str(&ipc::encode_field(&checksum));
        }
    }
    if args.next().is_some() {
        return Err("import accepts source, optional model id, and optional sha256".into());
    }
    request_formatted(&line, json_output, "import")
}

fn run_prompt(
    mut args: impl Iterator<Item = String>,
    plain: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = required(&mut args, "model id or --route")?;
    if target == "--help" {
        println!(
            "Usage: talvik run <model-id> <prompt> | talvik run --default <prompt> | talvik run --route <prompt>"
        );
        return Ok(());
    }
    let prompt = args.collect::<Vec<_>>().join(" ");
    if prompt.is_empty() {
        return Err("prompt required".into());
    }
    match target.as_str() {
        "--route" => route_generate(&prompt, plain),
        "--default" => {
            let default_model =
                oid_runtime::config::default_model()?.ok_or("no default model is configured")?;
            generate(&default_model.id, &prompt, plain)
                .map(|_| ())
                .map_err(|failure| failure.message.into())
        }
        _ => generate(&target, &prompt, plain)
            .map(|_| ())
            .map_err(|failure| failure.message.into()),
    }
}

fn routing_enabled() -> bool {
    std::env::var("TALVIK_ROUTING_ENABLED").is_ok_and(|value| routing_enabled_value(&value))
}

fn routing_enabled_value(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn route_models() -> Vec<String> {
    std::env::var("TALVIK_ROUTE_MODELS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn routing_status() {
    let models = route_models();
    println!(
        "routing: {}",
        if routing_enabled() {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("fallback order: {}", models.join(", "));
}

fn route_generate(prompt: &str, plain: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !routing_enabled() {
        return Err("routing is disabled; set TALVIK_ROUTING_ENABLED=1 to opt in".into());
    }
    let models = route_models();
    if models.is_empty() {
        return Err("TALVIK_ROUTE_MODELS must contain at least one model id".into());
    }
    let mut failures = Vec::new();
    for model in models {
        eprintln!("route: selecting {model}");
        if let Err(error) = request(&format!("model_load\t{}", ipc::encode_field(&model))) {
            eprintln!("route: {model} unavailable: {error}");
            failures.push(format!("{model}: {error}"));
            continue;
        }
        match generate(&model, prompt, plain) {
            Ok(_) => return Ok(()),
            Err(failure) if !failure.output_started => {
                eprintln!("route: {model} failed before output: {}", failure.message);
                failures.push(format!("{model}: {}", failure.message));
                let _ = request(&format!("model_unload\t{}", ipc::encode_field(&model)));
            }
            Err(failure) => {
                return Err(format!(
                    "{model} failed after streaming began; fallback suppressed: {}",
                    failure.message
                )
                .into());
            }
        }
    }
    Err(format!("all routed models failed: {}", failures.join("; ")).into())
}

struct GenerationFailure {
    message: String,
    output_started: bool,
}

fn generate(model: &str, prompt: &str, plain: bool) -> Result<bool, GenerationFailure> {
    generate_inner(model, prompt, plain).map_err(|(message, output_started)| GenerationFailure {
        message,
        output_started,
    })
}

fn generate_inner(model: &str, prompt: &str, plain: bool) -> Result<bool, (String, bool)> {
    let mut display =
        chat_ui::ChatDisplay::new(model, plain).map_err(|error| (error.to_string(), false))?;
    let interrupt = chat_ui::InterruptGuard::new().map_err(|error| (error.to_string(), false))?;
    ipc_runtime::IpcRuntimeService::probe().map_err(|error| (error.to_string(), false))?;
    let client = ipc_runtime::IpcRuntimeService::new(EventBus::new());
    let stream = client
        .generate(GenerationRequest {
            request_id: format!("talvik-{}-{}", std::process::id(), unique_suffix()),
            model_id: model.to_owned(),
            prompt: prompt.to_owned(),
            content: Vec::new(),
            options: GenerationOptions::default(),
            structured_output: None,
        })
        .map_err(|error| (error.to_string(), false))?;
    loop {
        if interrupt.take() {
            stream.cancel();
            display.cancelling();
        }
        match stream.recv_timeout(Duration::from_millis(50)) {
            Ok(GenerationMessage::Token(token)) => {
                if let Err(error) = display.token(&token) {
                    stream.cancel();
                    return Err((error.to_string(), display.output_started()));
                }
            }
            Ok(message @ (GenerationMessage::Completed(_) | GenerationMessage::Cancelled(_))) => {
                let cancelled = matches!(message, GenerationMessage::Cancelled(_));
                let (GenerationMessage::Completed(result) | GenerationMessage::Cancelled(result)) =
                    message
                else {
                    unreachable!()
                };
                display
                    .finish_generation(&result, cancelled)
                    .map_err(|error| (error.to_string(), display.output_started()))?;
                return Ok(display.output_started());
            }
            Ok(GenerationMessage::Failed(error)) => {
                return Err((error.to_string(), display.output_started()))
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err((
                    "connection closed before completion".to_owned(),
                    display.output_started(),
                ));
            }
        }
    }
}

fn request(line: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = ipc::connect()?;
    writeln!(stream, "{line}")?;
    for line in BufReader::new(stream).lines() {
        let line = line?;
        if let Some(error) = line.strip_prefix("ERROR\t") {
            let message = error
                .split_once('\t')
                .map_or(error.to_owned(), |(_, message)| ipc::decode_field(message));
            return Err(message.into());
        }
        println!("{}", line.strip_prefix("OK\t").unwrap_or(&line));
        if line == "OK" || line == "OK cancelled" || line == "END" {
            break;
        }
    }
    Ok(())
}

fn request_formatted(
    line: &str,
    json_output: bool,
    operation: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !json_output
        || !matches!(
            operation,
            "status" | "models" | "import" | "load" | "unload"
        )
    {
        return request(line);
    }
    let mut stream = ipc::connect()?;
    writeln!(stream, "{line}")?;
    let mut values = Vec::new();
    for line in BufReader::new(stream).lines() {
        let line = line?;
        if let Some(error) = line.strip_prefix("ERROR\t") {
            let message = error
                .split_once('\t')
                .map_or(error.to_owned(), |(_, message)| ipc::decode_field(message));
            return Err(format!("TALVIK-E-REQUEST: {message}").into());
        }
        if line == "END"
            || line == "OK"
            || line == "OK cancelled"
            || line.starts_with("OK\t")
            || line.starts_with("OK ")
        {
            if line.starts_with("OK") {
                values.push(line.clone());
            }
            if line == "END" || line == "OK" || line.starts_with("OK\t") || line.starts_with("OK ")
            {
                break;
            }
        } else if line.starts_with("MODEL\t") {
            values.push(line);
        }
    }
    let value = match operation {
        "status" => {
            let fields = values
                .first()
                .map(|line| line.split('\t').collect::<Vec<_>>())
                .unwrap_or_default();
            let loaded_models = fields
                .get(4)
                .map(|value| {
                    value
                        .split(',')
                        .filter(|item| !item.is_empty())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let diagnostics = fields
                .get(24)
                .and_then(|value| {
                    serde_json::from_str::<serde_json::Value>(&ipc::decode_field(value)).ok()
                })
                .unwrap_or(serde_json::Value::Null);
            json!({"object":"status","health":fields.get(1).copied().unwrap_or("unknown"),"backend":fields.get(2).copied().unwrap_or("unknown"),"loaded_model":fields.get(3).copied().unwrap_or(""),"loaded_models":loaded_models,"daemon_version":fields.get(5).copied().unwrap_or("unknown"),"daemon_git_sha":fields.get(6).copied().unwrap_or("unknown"),"daemon_dirty":fields.get(7).copied().unwrap_or("unknown"),"daemon_target":fields.get(8).copied().unwrap_or("unknown"),"daemon_profile":fields.get(9).copied().unwrap_or("unknown"),"version_mismatch":fields.get(10).copied().unwrap_or("unknown"),"inference_threads":fields.get(23).copied().unwrap_or(""),"diagnostics":diagnostics})
        }
        "models" => {
            json!({"object":"list","data":values.iter().filter_map(|line| { let fields = line.split('\t').collect::<Vec<_>>(); (fields.len() >= 4).then(|| json!({"id":ipc::decode_field(fields[1]),"name":ipc::decode_field(fields[2]),"status":fields[3]})) }).collect::<Vec<_>>() })
        }
        _ => json!({"object":operation,"response":values.first().cloned().unwrap_or_default()}),
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn auth(mut args: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    if args.next().as_deref() != Some("token") || args.next().as_deref() != Some("create") {
        return Err("usage: talvik auth token create [label] [--scopes=a,b]".into());
    }
    let arguments = args.collect::<Vec<_>>();
    let scopes = arguments
        .iter()
        .find_map(|value| value.strip_prefix("--scopes="))
        .map(|value| value.split(',').collect::<Vec<_>>());
    let label = arguments
        .iter()
        .filter(|value| !value.starts_with("--scopes="))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let label = if label.is_empty() {
        "local-client"
    } else {
        &label
    };
    let token = scopes.as_deref().map_or_else(
        || oid_runtime::auth::create_token(label),
        |scopes| oid_runtime::auth::create_token_with_scopes(label, scopes),
    )?;
    println!("{token}");
    eprintln!(
        "Token created in {}.",
        oid_runtime::auth::token_file().display()
    );
    Ok(())
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

#[cfg(test)]
mod tests {
    use super::routing_enabled_value;

    #[test]
    fn routing_requires_an_explicit_true_value() {
        assert!(routing_enabled_value("TRUE"));
        assert!(!routing_enabled_value("disabled"));
    }
}
