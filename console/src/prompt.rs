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
