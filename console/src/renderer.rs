//! Console rendering boundary.

use crate::status_bar::StatusBar;
use oid_runtime::RuntimeService;
use std::io::{self, Write};

/// Terminal renderer for the interactive console.
#[derive(Clone, Debug, Default)]
pub struct Renderer;

impl Renderer {
    /// Render the startup banner and lifecycle milestones.
    pub fn render_startup() {
        println!("=================================================");
        println!(" AI CONSOLE");
        println!(" Intelligent Runtime v0.1");
        println!("=================================================");
        println!();
        println!("Initializing...");
        println!();
        println!("✓ Configuration loaded");
        println!("✓ Event bus started");
        println!("✓ Runtime initialized");
        println!("✓ Console ready");
        println!();
        println!("No models installed.");
        println!();
        println!("Type");
        println!("help");
        println!("to begin.");
        println!();
    }

    /// Render the permanent status line.
    pub fn render_status(runtime: &dyn RuntimeService, _status_bar: &StatusBar) {
        StatusBar::render(runtime);
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
