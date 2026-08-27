//! Command history boundary.

/// Command history with an optional persistent backing file.
#[derive(Clone, Debug, Default)]
pub struct History {
    entries: Vec<String>,
    cursor: Option<usize>,
    path: Option<std::path::PathBuf>,
}

impl History {
    const MAX_ENTRIES: usize = 1_000;

    /// Create an empty history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            cursor: None,
            path: None,
        }
    }

    /// Load history from `~/.oip/history`, falling back to an empty in-memory
    /// history when the home directory or history file is unavailable.
    #[must_use]
    pub fn load_default() -> Self {
        let Some(home) = std::env::var_os("HOME") else {
            return Self::new();
        };
        Self::load(std::path::PathBuf::from(home).join(".oip/history"))
            .unwrap_or_else(|_| Self::new())
    }

    fn load(path: std::path::PathBuf) -> std::io::Result<Self> {
        let entries = match std::fs::read_to_string(&path) {
            Ok(contents) => contents
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        let start = entries.len().saturating_sub(Self::MAX_ENTRIES);
        Ok(Self {
            entries: entries[start..].to_vec(),
            cursor: None,
            path: Some(path),
        })
    }

    /// Add a non-empty command and reset navigation.
    pub fn push(&mut self, command: String) {
        if !command.trim().is_empty() {
            self.entries.push(command);
            if self.entries.len() > Self::MAX_ENTRIES {
                self.entries.remove(0);
            }
            if let Some(path) = &self.path {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Some(command) = self.entries.last() {
                    let _ = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                        .and_then(|mut file| {
                            use std::io::Write;
                            writeln!(file, "{command}")
                        });
                }
            }
        }
        self.cursor = None;
    }

    /// Return recorded entries.
    #[must_use]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Move backward through history.
    #[must_use]
    pub fn previous(&mut self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        let next = self.cursor.unwrap_or(self.entries.len()).saturating_sub(1);
        self.cursor = Some(next);
        self.entries.get(next).cloned()
    }

    /// Move forward through history, returning an empty line at the newest edge.
    #[must_use]
    pub fn next(&mut self) -> Option<String> {
        let cursor = self.cursor?;
        if cursor + 1 >= self.entries.len() {
            self.cursor = None;
            return Some(String::new());
        }
        self.cursor = Some(cursor + 1);
        self.entries.get(cursor + 1).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::History;

    #[test]
    fn navigates_back_and_forward() {
        let mut history = History::new();
        history.push("help".to_owned());
        history.push("status".to_owned());
        assert_eq!(history.previous().as_deref(), Some("status"));
        assert_eq!(history.previous().as_deref(), Some("help"));
        assert_eq!(history.next().as_deref(), Some("status"));
        assert_eq!(history.next().as_deref(), Some(""));
    }

    #[test]
    fn persists_and_reloads_commands() {
        let path = std::env::temp_dir().join(format!("oid-history-test-{}", std::process::id()));
        let mut history = History::load(path.clone()).expect("load history");
        history.push("ls -la".to_owned());
        history.push(":status".to_owned());

        let reloaded = History::load(path.clone()).expect("reload history");
        assert_eq!(reloaded.entries(), &["ls -la", ":status"]);
        std::fs::remove_file(path).expect("remove history fixture");
    }
}
