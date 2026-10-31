//! AI Console executable.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod audio;
mod chat_ui;
mod commands;
mod editor;
mod foundation;
mod history;
mod input_router;
mod ipc;
mod ipc_runtime;
mod operations;
mod presence;
mod prompt;
mod renderer;
mod shell;
mod signals;
mod status_bar;
mod tui;

fn main() {
    let application = match tui::Application::start() {
        Ok(application) => application,
        Err(error) => {
            eprintln!("AI Console failed to start: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = application.run() {
        eprintln!("AI Console failed to start: {error}");
        std::process::exit(1);
    }
}
