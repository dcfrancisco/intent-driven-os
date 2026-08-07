//! AI Console executable.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod commands;
mod editor;
mod foundation;
mod history;
mod input_router;
mod operations;
mod presence;
mod prompt;
mod renderer;
mod shell;
mod status_bar;
mod tui;

fn main() {
    if let Err(error) = tui::Application::start().run() {
        eprintln!("AI Console failed to start: {error}");
        std::process::exit(1);
    }
}
