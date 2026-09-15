//! Marina local runtime client.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[path = "../ipc.rs"]
mod ipc;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
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
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or("usage: marinactl status|model list|model load <id>|model unload <id>|generate <model> <prompt>")?;
    if command == "status" {
        return request("status");
    }
    if command == "cancel" {
        return request("cancel");
    }
    if command == "model" {
        let action = args
            .next()
            .ok_or("usage: marinactl model list|load|unload")?;
        let request_line = match action.as_str() {
            "list" => "model_list".to_owned(),
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
    let mut stream = UnixStream::connect(ipc::socket_path())?;
    writeln!(stream, "{request}")?;
    for line in BufReader::new(stream).lines() {
        let line = line?;
        if let Some(error) = line.strip_prefix("ERROR\t") {
            return Err(error.to_owned().into());
        }
        println!("{}", line.strip_prefix("OK\t").unwrap_or(&line));
        if line == "OK" || line == "OK cancelled" {
            break;
        }
    }
    Ok(())
}

fn generate(model: &str, prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(ipc::socket_path())?;
    writeln!(
        stream,
        "generate\t{}\t128\t{}",
        model,
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
            let _ = request("cancel");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
