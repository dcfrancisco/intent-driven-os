//! Marina local runtime client.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[path = "../ipc.rs"]
mod ipc;

use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn main() {
    if let Err(error) = run() {
        eprintln!("marinactl: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|argument| argument == "--version" || argument == "-V") {
        println!("marinactl {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or("usage: marinactl status|model list|model pull <source> [model_id] [sha256]|model load <id>|model unload <id>|generate <model> <prompt>")?;
    if command == "auth" {
        let action = args
            .next()
            .ok_or("usage: marinactl auth token create [label]")?;
        if action == "token" && args.next().as_deref() == Some("create") {
            let label = args.collect::<Vec<_>>().join(" ");
            let label = if label.is_empty() {
                "local-client"
            } else {
                &label
            };
            let token = oid_runtime::auth::create_token(label)?;
            println!("{token}");
            eprintln!(
                "Token created in {}. Store the printed token securely; it cannot be recovered.",
                oid_runtime::auth::token_file().display()
            );
            return Ok(());
        }
        return Err("usage: marinactl auth token create [label]".into());
    }
    if command == "status" {
        return request("status");
    }
    if command == "cancel" {
        let request_id = args.next().ok_or("request id required")?;
        return request(&format!("cancel\t{}", ipc::encode_field(&request_id)));
    }
    if command == "model" {
        let action = args
            .next()
            .ok_or("usage: marinactl model list|pull <source> [model_id] [sha256]|load|unload")?;
        let request_line = match action.as_str() {
            "list" => "model_list".to_owned(),
            "pull" => {
                let source = args.next().ok_or("model source required")?;
                let model_id = args.next();
                let checksum = args.next();
                if args.next().is_some() {
                    return Err(
                        "model pull accepts source, optional model id, and optional sha256".into(),
                    );
                }
                let mut request = format!("model_pull\t{}", ipc::encode_field(&source));
                if let Some(model_id) = model_id {
                    request.push('\t');
                    request.push_str(&ipc::encode_field(&model_id));
                    if let Some(checksum) = checksum {
                        request.push('\t');
                        request.push_str(&ipc::encode_field(&checksum));
                    }
                } else if checksum.is_some() {
                    return Err("sha256 requires a model id".into());
                }
                request
            }
            "load" => format!("model_load\t{}", args.next().ok_or("model id required")?),
            "unload" => format!("model_unload\t{}", args.next().ok_or("model id required")?),
            _ => return Err("unknown model command".into()),
        };
        return request(&request_line);
    }
    if command == "generate" {
        let model = args.next().ok_or("model id required")?;
        let prompt = args.collect::<Vec<_>>().join(" ");
        if prompt.is_empty() {
            return Err("prompt required".into());
        }
        return generate(&model, &prompt);
    }
    Err("unknown command".into())
}

fn request(request: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = ipc::connect()?;
    writeln!(stream, "{request}")?;
    for line in BufReader::new(stream).lines() {
        let line = line?;
        if let Some(error) = line.strip_prefix("ERROR\t") {
            let mut fields = error.splitn(2, '\t');
            let operation = fields.next().unwrap_or("request");
            let message = fields
                .next()
                .map(ipc::decode_field)
                .unwrap_or_else(|| operation.to_owned());
            return Err(format!("{operation}: {message}").into());
        }
        if let Some(model_pull) = line.strip_prefix("OK\tmodel_pull\t") {
            let fields: Vec<_> = model_pull.split('\t').collect();
            if fields.len() >= 5 {
                println!(
                    "model_pull: id={} name={} status={} location={} sha256={}",
                    ipc::decode_field(fields[0]),
                    ipc::decode_field(fields[1]),
                    fields[2],
                    ipc::decode_field(fields[3]),
                    ipc::decode_field(fields[4])
                );
            } else {
                println!("{line}");
            }
        } else {
            println!("{}", line.strip_prefix("OK\t").unwrap_or(&line));
        }
        if line == "OK" || line == "OK cancelled" {
            break;
        }
    }
    Ok(())
}

fn generate(model: &str, prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = ipc::connect()?;
    let request_id = format!("cli-{}-{}", std::process::id(), unique_suffix());
    writeln!(
        stream,
        "generate\t{}\t128\t{}\t{}",
        model,
        ipc::encode_field(&request_id),
        ipc::encode_field(prompt)
    )?;
    let cancelled = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&cancelled))?;
    let reader = BufReader::new(stream.try_clone()?);
    for line in reader.lines() {
        let line = line?;
        if let Some(token) = line.strip_prefix("TOKEN\t") {
            print!("{}", ipc::decode_field(token));
            std::io::stdout().flush()?;
        } else if line.starts_with("DONE\t") {
            println!();
            println!("{line}");
            break;
        } else if let Some(error) = line.strip_prefix("ERROR\t") {
            return Err(ipc::decode_field(error).into());
        }
        if cancelled.swap(false, Ordering::SeqCst) {
            let _ = request(&format!("cancel\t{}", ipc::encode_field(&request_id)));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}
