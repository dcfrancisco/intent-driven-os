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
            if args.next().as_deref() == Some("--verbose") {
                println!("{}", server::build_info::display("talvikd"));
            } else {
                println!("talvikd {}", server::build_info::current().version);
            }
            Ok(())
        }
        Some("version") => {
            if args.next().as_deref() == Some("--json") {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&server::build_info::json("talvikd", None))?
                );
            } else {
                println!("{}", server::build_info::display("talvikd"));
            }
            Ok(())
        }
        Some(argument) => Err(format!("unexpected argument `{argument}`; usage: talvikd").into()),
    }
}
