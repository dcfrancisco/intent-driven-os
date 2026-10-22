//! Talvik local model service daemon.

#![forbid(unsafe_code)]

#[path = "talvik_server.rs"]
mod server;

fn main() {
    if let Err(error) = run() {
        eprintln!("talvikd: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => server::run(),
        Some("--help" | "-h" | "help") => {
            println!("Talvik local model service daemon\n\nUsage:\n  talvikd");
            Ok(())
        }
        Some("--version" | "-V") => {
            println!("talvikd {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(argument) => Err(format!("unexpected argument `{argument}`; usage: talvikd").into()),
    }
}
