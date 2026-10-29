//! Talvik local model service client.

#![forbid(unsafe_code)]

#[path = "../build_info.rs"]
mod build_info;
#[path = "../ipc.rs"]
mod ipc;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const HELP: &str = "Talvik local model runtime

Usage:
  talvik [--json] <command> [arguments]

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

Routing is off by default. Enable ordered fallback with
TALVIK_ROUTING_ENABLED=1 and TALVIK_ROUTE_MODELS=model-a,model-b.";

fn main() {
    if let Err(error) = run() {
        eprintln!("talvik: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let json_output = arguments.first().is_some_and(|value| value == "--json");
    if json_output {
        arguments.remove(0);
    }
    let mut args = arguments.into_iter();
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
        "version" if args.next().is_none() => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&build_info::json("talvik", None))?
                );
            } else {
                println!("{}", build_info::display("talvik"));
            }
        }
        "status" if args.next().is_none() => request_formatted(
            &format!("status\t{}", build_info::current().version),
            json_output,
            "status",
        )?,
        "models" if args.next().is_none() => {
            request_formatted("model_list", json_output, "models")?;
        }
        "import" => {
            if args.next().as_deref() == Some("--help") {
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
        "run" => run_prompt(args)?,
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

fn run_prompt(mut args: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
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
        "--route" => route_generate(&prompt),
        "--default" => {
            let default_model =
                oid_runtime::config::default_model()?.ok_or("no default model is configured")?;
            generate(&default_model.id, &prompt)
                .map(|_| ())
                .map_err(|failure| failure.message.into())
        }
        _ => generate(&target, &prompt)
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

fn route_generate(prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
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
        match generate(&model, prompt) {
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

fn generate(model: &str, prompt: &str) -> Result<bool, GenerationFailure> {
    generate_inner(model, prompt).map_err(|(message, output_started)| GenerationFailure {
        message,
        output_started,
    })
}

fn generate_inner(model: &str, prompt: &str) -> Result<bool, (String, bool)> {
    let mut stream = ipc::connect().map_err(|error| (error.to_string(), false))?;
    let request_id = format!("talvik-{}-{}", std::process::id(), unique_suffix());
    writeln!(
        stream,
        "generate\t{}\t128\t{}\t{}",
        ipc::encode_field(model),
        ipc::encode_field(&request_id),
        ipc::encode_field(prompt)
    )
    .map_err(|error| (error.to_string(), false))?;
    let cancelled = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&cancelled))
        .map_err(|error| (error.to_string(), false))?;
    let reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|error| (error.to_string(), false))?,
    );
    let mut output_started = false;
    for line in reader.lines() {
        let line = line.map_err(|error| (error.to_string(), output_started))?;
        if let Some(token) = line.strip_prefix("TOKEN\t") {
            output_started = true;
            print!("{}", ipc::decode_field(token));
            std::io::stdout()
                .flush()
                .map_err(|error| (error.to_string(), true))?;
        } else if let Some(state) = line.strip_prefix("DONE\t") {
            println!();
            println!("DONE {state}");
            return Ok(output_started);
        } else if let Some(error) = line.strip_prefix("ERROR\t") {
            return Err((ipc::decode_field(error), output_started));
        }
        if cancelled.swap(false, Ordering::SeqCst) {
            let _ = request(&format!("cancel\t{}", ipc::encode_field(&request_id)));
        }
    }
    Err(("connection closed before DONE".to_owned(), output_started))
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
            json!({"object":"status","health":fields.get(1).copied().unwrap_or("unknown"),"backend":fields.get(2).copied().unwrap_or("unknown"),"loaded_model":fields.get(3).copied().unwrap_or(""),"loaded_models":loaded_models,"daemon_version":fields.get(5).copied().unwrap_or("unknown"),"daemon_git_sha":fields.get(6).copied().unwrap_or("unknown"),"daemon_dirty":fields.get(7).copied().unwrap_or("unknown"),"daemon_target":fields.get(8).copied().unwrap_or("unknown"),"daemon_profile":fields.get(9).copied().unwrap_or("unknown"),"version_mismatch":fields.get(10).copied().unwrap_or("unknown"),"inference_threads":fields.get(23).copied().unwrap_or("")})
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
