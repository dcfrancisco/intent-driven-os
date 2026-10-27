//! Permanent runtime status bar.

use oid_runtime::{RuntimeService, RuntimeSnapshot};
use std::process::Command;

/// Runtime status display mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StatusDisplayMode {
    /// Show a one-line health summary.
    #[default]
    Compact,
    /// Show all runtime fields.
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

    /// Render the current status snapshot.
    pub fn render(&self, runtime: &dyn RuntimeService) {
        match self.mode {
            StatusDisplayMode::Compact => println!("{}", Self::compact_line(runtime)),
            StatusDisplayMode::Full => println!("{}", Self::line(runtime, &local_time())),
            StatusDisplayMode::Hidden => {}
        }
    }

    /// Format the default compact status line.
    #[must_use]
    pub fn compact_line(runtime: &dyn RuntimeService) -> String {
        let snapshot = runtime.snapshot();
        format!(
            "[OID {} | model {}]",
            snapshot.health,
            loaded_model_label(&snapshot, "none")
        )
    }

    /// Format a status bar line from a snapshot and supplied time.
    #[must_use]
    pub fn line(runtime: &dyn RuntimeService, time: &str) -> String {
        let snapshot = runtime.snapshot();
        format!(
            " Runtime: {} | Active backend: {} | Backend version: {} | Loaded model: {} | Model memory: {} | Context: {} | TPS: {} | Uptime: {}s | Time: {}",
            snapshot.health,
            snapshot.backend,
            snapshot.backend_version.as_deref().unwrap_or("--"),
            loaded_model_label(&snapshot, "None"),
            snapshot.model_memory_bytes.map_or_else(|| "--".to_owned(), |bytes| format!("{} MB", bytes / 1_048_576)),
            snapshot.loaded_context.map_or_else(|| "--".to_owned(), |context| context.to_string()),
            snapshot.current_tokens_per_second.map_or_else(|| "--".to_owned(), |tps| format!("{tps:.1}")),
            snapshot.uptime_seconds,
            time
        )
    }
}

fn loaded_model_label<'a>(snapshot: &'a RuntimeSnapshot, empty_label: &'static str) -> &'a str {
    snapshot.loaded_model.as_deref().unwrap_or_else(|| {
        if snapshot.health == "Unavailable"
            || snapshot.backend == "marina-http"
            || snapshot.backend.ends_with("(unavailable)")
        {
            "unknown"
        } else {
            empty_label
        }
    })
}

fn local_time() -> String {
    Command::new("date")
        .arg("+%H:%M:%S")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|time| !time.is_empty())
        .unwrap_or_else(|| "--:--:--".to_owned())
}

#[cfg(test)]
mod tests {
    use super::StatusBar;
    use oid_runtime::{MarinaRuntime, RuntimeService};
    use oid_shared::{EventBus, RuntimeConfig};

    #[test]
    fn renders_deterministic_placeholder_information() {
        let runtime = MarinaRuntime::start(RuntimeConfig::default(), EventBus::new());
        let line = StatusBar::line(&runtime, "12:34:56");
        assert!(line.contains("Runtime: Healthy"));
        let backend = if oid_runtime::LlamaCppAdapter::native_available() {
            "llama.cpp"
        } else {
            "None"
        };
        assert!(line.contains(&format!("Active backend: {backend}")));
        assert!(line.contains("Backend version: --"));
        assert!(line.contains("Model memory: --"));
        assert!(line.contains("Loaded model: None"));
        assert!(line.contains("Uptime: "));
        assert!(line.contains("Time: 12:34:56"));
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
