//! Permanent runtime status bar.

use crate::{presence::PresenceState, terminal_ui};
use crossterm::{
    cursor,
    style::{Attribute, SetAttribute},
    QueueableCommand,
};
use oid_runtime::{RuntimeService, RuntimeSnapshot};
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Runtime status display mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StatusDisplayMode {
    /// Show a one-line health summary.
    #[default]
    Compact,
    /// Legacy full mode; the footer stays compact. Use :status for diagnostics.
    Full,
    /// Do not render status automatically.
    Hidden,
}

/// Status bar renderer using runtime data and local wall-clock time.
#[derive(Clone, Debug, Default)]
pub struct StatusBar {
    /// Configured display mode.
    pub mode: StatusDisplayMode,
}

impl StatusBar {
    /// Parse a configured display mode.
    #[must_use]
    pub fn mode(value: &str) -> StatusDisplayMode {
        match value {
            "full" => StatusDisplayMode::Full,
            "hidden" => StatusDisplayMode::Hidden,
            _ => StatusDisplayMode::Compact,
        }
    }

    /// Reserve a single bottom row on interactive terminals.
    pub fn attach(&self, runtime: &dyn RuntimeService) -> io::Result<StatusFooter> {
        let mut footer = StatusFooter::new(self.mode)?;
        footer.update(runtime, PresenceState::Idle)?;
        footer.resume()?;
        Ok(footer)
    }
}

/// A single reserved terminal row. Shell commands temporarily regain the full screen.
#[derive(Debug)]
pub struct StatusFooter {
    mode: StatusDisplayMode,
    interactive: bool,
    active: bool,
    colors: bool,
    columns: u16,
    rows: u16,
    state: String,
    model: String,
}

impl StatusFooter {
    fn new(mode: StatusDisplayMode) -> io::Result<Self> {
        let interactive = terminal_ui::interactive();
        let (columns, rows) = if interactive {
            crossterm::terminal::size()?
        } else {
            (80, 24)
        };
        let footer = Self {
            mode,
            interactive,
            active: false,
            colors: terminal_ui::colors(),
            columns,
            rows,
            state: "Ready".to_owned(),
            model: "No model loaded".to_owned(),
        };
        Ok(footer)
    }

    /// Available input width, including when the status row is hidden.
    pub fn columns(&self) -> u16 {
        self.columns
    }

    /// Refresh only the state and model; detailed metrics remain opt-in.
    pub fn update(
        &mut self,
        runtime: &dyn RuntimeService,
        presence: PresenceState,
    ) -> io::Result<()> {
        let snapshot = runtime.snapshot();
        let state = if snapshot.health == "Unavailable" {
            "Offline"
        } else if snapshot.health != "Healthy" {
            snapshot.health
        } else {
            match presence {
                PresenceState::Idle | PresenceState::Typing => "Ready",
                PresenceState::Busy => "Working",
                PresenceState::Streaming => "Generating",
                PresenceState::Waiting => "Approval required",
                PresenceState::Warning => "Warning",
                PresenceState::Error => "Error",
                PresenceState::Offline => "Offline",
            }
        };
        state.clone_into(&mut self.state);
        loaded_model_label(&snapshot, "No model loaded").clone_into(&mut self.model);
        self.repaint()
    }

    /// Repaint without polling the runtime or moving the user's input cursor.
    pub fn repaint(&self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        let mut output = io::stdout().lock();
        output
            .queue(cursor::SavePosition)?
            .queue(cursor::MoveTo(0, self.rows - 1))?;
        write!(output, "\x1b[2K")?;
        if self.colors {
            output.queue(SetAttribute(Attribute::Reverse))?;
        }
        write!(
            output,
            "{}",
            footer_line(&self.state, &self.model, usize::from(self.columns))
        )?;
        if self.colors {
            output.queue(SetAttribute(Attribute::Reset))?;
        }
        output.queue(cursor::RestorePosition)?.flush()
    }

    /// Release the reserved row before handing the terminal to a native program.
    pub fn suspend(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let mut output = io::stdout().lock();
        output
            .queue(cursor::SavePosition)?
            .queue(cursor::MoveTo(0, self.rows - 1))?;
        write!(output, "\x1b[2K\x1b[r")?;
        output.queue(cursor::RestorePosition)?.flush()
    }

    /// Restore the footer after a child command, accounting for changed geometry.
    pub fn resume(&mut self) -> io::Result<()> {
        if !self.interactive || self.mode == StatusDisplayMode::Hidden {
            return Ok(());
        }
        (self.columns, self.rows) = crossterm::terminal::size()?;
        if self.columns < 8 || self.rows < 3 {
            return Ok(());
        }
        let mut output = io::stdout().lock();
        // Make room if the cursor was at the physical bottom of the terminal.
        write!(output, "\n\x1b[1A")?;
        output.queue(cursor::SavePosition)?;
        write!(output, "\x1b[1;{}r", self.rows - 1)?;
        output.queue(cursor::RestorePosition)?.flush()?;
        self.active = true;
        self.repaint()
    }

    /// Keep the footer and input inside the new terminal bounds.
    pub fn resize(&mut self, columns: u16, rows: u16) -> io::Result<()> {
        if (columns, rows) == (self.columns, self.rows) {
            return Ok(());
        }
        self.suspend()?;
        self.columns = columns;
        self.rows = rows;
        self.active =
            self.interactive && self.mode != StatusDisplayMode::Hidden && columns >= 8 && rows >= 3;
        let mut output = io::stdout().lock();
        if self.active {
            write!(output, "\x1b[1;{}r", rows - 1)?;
        }
        output
            .queue(cursor::MoveTo(
                0,
                rows.saturating_sub(if self.active { 2 } else { 1 }),
            ))?
            .flush()?;
        self.repaint()
    }
}

impl Drop for StatusFooter {
    fn drop(&mut self) {
        let _ = self.suspend();
    }
}

fn footer_line(state: &str, model: &str, columns: usize) -> String {
    let available = columns.saturating_sub(2);
    let clean = |text: &str| {
        text.chars()
            .filter(|ch| !ch.is_control())
            .collect::<String>()
    };
    let mut left = fit_text(&format!("{} · {}", clean(state), clean(model)), available);
    let hint = ":help";
    if columns >= 72 && left.width() + hint.width() + 3 <= available {
        left.push_str(&" ".repeat(available - left.width() - hint.width()));
        left.push_str(hint);
    }
    let padding = available.saturating_sub(left.width());
    format!(" {left}{}", " ".repeat(padding))
}

fn fit_text(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let next = ch.width().unwrap_or(0);
        if used + next >= width {
            break;
        }
        result.push(ch);
        used += next;
    }
    result.push('…');
    result
}

fn loaded_model_label<'a>(snapshot: &'a RuntimeSnapshot, empty_label: &'static str) -> &'a str {
    snapshot.loaded_model.as_deref().unwrap_or_else(|| {
        if snapshot.health == "Unavailable" || snapshot.backend.ends_with("(unavailable)") {
            "unknown"
        } else {
            empty_label
        }
    })
}

#[cfg(test)]
mod tests {
    use super::StatusBar;
    use oid_runtime::{MarinaRuntime, RuntimeService};
    use oid_shared::{EventBus, RuntimeConfig};

    #[test]
    fn default_footer_contains_only_state_model_and_help() {
        let line = super::footer_line("Ready", "qwen3.5-4b", 80);
        assert!(line.starts_with(" Ready · qwen3.5-4b"));
        assert!(line.ends_with(":help"));
        assert!(!line.contains("Runtime:") && !line.contains("Backend") && !line.contains("Time:"));
    }

    #[test]
    fn footer_stays_on_one_row_with_wide_model_names_and_small_terminals() {
        use unicode_width::UnicodeWidthStr;
        for columns in [8, 12, 20, 40, 80, 120] {
            let line = super::footer_line("Ready", "大型モデル-qwen3.5-4b-Q4_K_M", columns);
            assert!(line.width() < columns);
            assert!(!line.contains("Threads") && !line.contains("Uptime") && !line.contains("TPS"));
        }
        assert!(super::footer_line("Ready", "qwen", 80).contains(":help"));
        assert!(!super::footer_line("Ready", "qwen", 20).contains(":help"));
    }

    #[test]
    fn supports_configurable_display_modes() {
        assert_eq!(
            StatusBar::mode("compact"),
            super::StatusDisplayMode::Compact
        );
        assert_eq!(StatusBar::mode("full"), super::StatusDisplayMode::Full);
        assert_eq!(StatusBar::mode("hidden"), super::StatusDisplayMode::Hidden);
    }

    #[test]
    fn unavailable_runtime_does_not_claim_that_no_model_is_loaded() {
        let runtime = MarinaRuntime::start(RuntimeConfig::default(), EventBus::new());
        let mut snapshot = runtime.snapshot();
        snapshot.health = "Unavailable";
        snapshot.backend = "talvikd (unavailable)".to_owned();

        assert_eq!(super::loaded_model_label(&snapshot, "none"), "unknown");
    }
}
