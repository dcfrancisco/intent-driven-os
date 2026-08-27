//! Prompt rendering and input boundary.

use crate::{editor::LineBuffer, presence::PresenceIndicator};
use std::io::{self, Write};

/// Console prompt presentation.
#[derive(Clone, Debug)]
pub struct Prompt {
    /// Prompt marker shown to the user.
    pub marker: String,
}

impl Default for Prompt {
    fn default() -> Self {
        Self {
            marker: "int>".to_owned(),
        }
    }
}

impl Prompt {
    /// Update the prompt marker to reflect the current shell directory.
    pub fn set_directory(&mut self, directory: &std::path::Path) {
        self.marker = format!("{}$", compact_directory(directory));
    }

    /// Render a line and place the terminal cursor at the logical edit position.
    pub fn render_line<W: Write>(
        &self,
        output: &mut W,
        buffer: &LineBuffer,
        presence: &PresenceIndicator,
    ) -> io::Result<()> {
        let text = buffer.text();
        let after_cursor = text.chars().count().saturating_sub(buffer.cursor());
        write!(
            output,
            "\r\x1b[2K{} {}{}",
            self.marker,
            text,
            presence.token()
        )?;
        if after_cursor > 0 {
            write!(output, "\x1b[{after_cursor}D")?;
        }
        output.flush()
    }
}

fn compact_directory(directory: &std::path::Path) -> String {
    let display = std::env::var_os("HOME")
        .and_then(|home| directory.strip_prefix(home).ok())
        .map_or_else(
            || directory.display().to_string(),
            |relative| {
                if relative.as_os_str().is_empty() {
                    "~".to_owned()
                } else {
                    format!("~/{}", relative.display())
                }
            },
        );

    const MAX_LENGTH: usize = 42;
    if display.chars().count() <= MAX_LENGTH {
        return display;
    }

    let components: Vec<_> = std::path::Path::new(&display)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    if components.len() >= 2 {
        format!(
            "…/{}/{}",
            components[components.len() - 2],
            components[components.len() - 1]
        )
    } else {
        display
    }
}

#[cfg(test)]
mod tests {
    use super::compact_directory;
    use std::path::Path;

    #[test]
    fn compacts_long_paths_to_the_final_two_components() {
        assert_eq!(
            compact_directory(Path::new(
                "/a/very-long-parent-directory-name/poc-projects/intent-driven-os"
            )),
            "…/poc-projects/intent-driven-os"
        );
    }
}
