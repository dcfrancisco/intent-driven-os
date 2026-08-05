//! Command history boundary.

/// In-memory command history.
#[derive(Clone, Debug, Default)]
pub struct History {
    entries: Vec<String>,
    cursor: Option<usize>,
}

impl History {
    /// Create an empty history.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            cursor: None,
        }
    }

    /// Add a non-empty command and reset navigation.
    pub fn push(&mut self, command: String) {
        if !command.trim().is_empty() {
            self.entries.push(command);
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
}
