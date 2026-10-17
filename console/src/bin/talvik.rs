//! Primary Talvik local model service and client command.

#![forbid(unsafe_code)]

#[allow(dead_code)]
#[path = "marina.rs"]
mod server;

use server::ipc;
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const HELP: &str = "Talvik local model runtime

Usage:
  talvik serve
  talvik status
  talvik models
  talvik import <source> [model-id] [sha256]
  talvik load <model-id>
  talvik unload <model-id>
  talvik run <model-id> <prompt>
  talvik run --route <prompt>
  talvik cancel <request-id>
  talvik auth token create [label] [--scopes=a,b]
  talvik routing status

Routing is off by default. Enable ordered fallback with
TALVIK_ROUTING_ENABLED=1 and TALVIK_ROUTE_MODELS=model-a,model-b.";

fn main() {
    if let Err(error) = run() {
        eprintln!("talvik: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        println!("{HELP}");
        return Ok(());
    };
    match command.as_str() {
        "--help" | "-h" | "help" => println!("{HELP}"),
        "--version" | "-V" => println!("talvik {}", env!("CARGO_PKG_VERSION")),
        "serve" => server::run()?,
        "status" => request("status")?,
        "models" => request("model_list")?,
        "import" => import(args)?,
        "load" => request(&format!(
            "model_load\t{}",
            ipc::encode_field(&required(&mut args, "model id")?)
        ))?,
        "unload" => request(&format!(
            "model_unload\t{}",
            ipc::encode_field(&required(&mut args, "model id")?)
        ))?,
        "cancel" => request(&format!(
            "cancel\t{}",
            ipc::encode_field(&required(&mut args, "request id")?)
        ))?,
        "run" => run_prompt(args)?,
        "routing" if args.next().as_deref() == Some("status") => routing_status(),
        "auth" => auth(args)?,
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

fn import(mut args: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
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
    request(&line)
}

fn run_prompt(mut args: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    let target = required(&mut args, "model id or --route")?;
    let prompt = args.collect::<Vec<_>>().join(" ");
    if prompt.is_empty() {
        return Err("prompt required".into());
    }
    if target == "--route" {
        route_generate(&prompt)
    } else {
        generate(&target, &prompt)
            .map(|_| ())
            .map_err(|failure| failure.message.into())
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
