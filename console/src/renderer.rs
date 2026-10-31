//! Console rendering boundary.

use crate::terminal_ui;
use std::io::{self, Write};

/// Terminal renderer for the interactive console.
#[derive(Clone, Debug, Default)]
pub struct Renderer;

impl Renderer {
    /// Render a restrained console heading and command hint.
    pub fn render_startup() {
        println!(
            "\n{} {}",
            terminal_ui::accent("1;36", "OID Console"),
            terminal_ui::accent("2", env!("CARGO_PKG_VERSION"))
        );
        println!(
            "{}\n",
            terminal_ui::accent("2", "Type :help to begin. :quit to exit.")
        );
    }

    /// Render command output without adding UI state of its own.
    pub fn render_output(output: &[String]) {
        for line in output {
            println!("{line}");
        }
    }

    /// Render the terminal clear operation.
    pub fn clear() -> io::Result<()> {
        if !terminal_ui::interactive() {
            return Ok(());
        }
        let mut stdout = io::stdout().lock();
        write!(stdout, "\x1b[2J\x1b[H")?;
        stdout.flush()
    }

    /// Render a shutdown message.
    pub fn render_shutdown() {
        println!("Console closed.");
    }
}
