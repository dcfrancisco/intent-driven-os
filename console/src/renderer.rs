//! Console rendering boundary.

use crate::status_bar::StatusBar;
use oid_runtime::RuntimeService;
use std::io::{self, Write};

/// Terminal renderer for the interactive console.
#[derive(Clone, Debug, Default)]
pub struct Renderer;

impl Renderer {
    /// Render the startup banner and current runtime/model state.
    pub fn render_startup(runtime: &dyn RuntimeService) {
        let snapshot = runtime.snapshot();
        let models = runtime.model_list();
        println!("=================================================");
        println!(" AI CONSOLE");
        println!(" Intelligent Runtime v0.1");
        println!("=================================================");
        println!();
        println!("Runtime: {} ({})", snapshot.health, snapshot.backend);
        if models.is_empty() {
            println!("No models found in the active runtime.");
        } else {
            println!("Available models:");
            for model in models {
                println!("  {} [{:?}]", model.id, model.status);
            }
        }
        println!();
        println!("Type");
        println!("help");
        println!("to begin.");
        println!();
    }

    /// Render the permanent status line.
    pub fn render_status(runtime: &dyn RuntimeService, status_bar: &StatusBar) {
        status_bar.render(runtime);
    }

    /// Render command output without adding UI state of its own.
    pub fn render_output(output: &[String]) {
        for line in output {
            println!("{line}");
        }
    }

    /// Render the terminal clear operation.
    pub fn clear() -> io::Result<()> {
        let mut stdout = io::stdout().lock();
        write!(stdout, "\x1b[2J\x1b[H")?;
        stdout.flush()
    }

    /// Render a shutdown message.
    pub fn render_shutdown() {
        println!("Runtime stopped.");
    }
}
