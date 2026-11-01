//! A quiet dBase-style dot prompt and horizontally scrolling input viewport.

use crate::{editor::LineBuffer, terminal_ui};
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Console prompt presentation.
#[derive(Clone, Debug)]
pub struct Prompt {
    /// Prompt marker shown to the user.
    pub marker: String,
}

impl Default for Prompt {
    fn default() -> Self {
        Self {
            marker: ".".to_owned(),
        }
    }
}

impl Prompt {
    /// Render a single-row input viewport with the terminal's native cursor.
    pub fn render_line<W: Write>(
        &self,
        output: &mut W,
        buffer: &LineBuffer,
        columns: u16,
    ) -> io::Result<()> {
        let (text, cursor) = input_viewport(&buffer.text(), buffer.cursor(), usize::from(columns));
        write!(
            output,
            "\r\x1b[2K{} {text}\x1b[{}G",
            terminal_ui::accent("1;36", &self.marker),
            cursor + 1
        )?;
        output.flush()
    }
}

fn input_viewport(text: &str, cursor: usize, columns: usize) -> (String, usize) {
    let chars = text.chars().collect::<Vec<_>>();
    let cursor = cursor.min(chars.len());
    // Dot + space, two overflow markers, and one column for the cursor.
    let capacity = columns.saturating_sub(5);
    let mut start = cursor;
    let mut width = 0;
    while start > 0 {
        let next = chars[start - 1].width().unwrap_or(0);
        if width + next > capacity {
            break;
        }
        width += next;
        start -= 1;
    }
    let mut end = start;
    width = 0;
    while end < chars.len() {
        let next = chars[end].width().unwrap_or(0);
        if width + next > capacity {
            break;
        }
        width += next;
        end += 1;
    }
    let left = if start > 0 { "<" } else { "" };
    let right = if end < chars.len() { ">" } else { "" };
    let visible = chars[start..end].iter().collect::<String>();
    let before_cursor = chars[start..cursor].iter().collect::<String>();
    (
        format!("{left}{visible}{right}"),
        2 + left.len() + before_cursor.width(),
    )
}

#[cfg(test)]
mod tests {
    use super::input_viewport;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn dot_prompt_uses_the_real_cursor_without_presence_glyphs() {
        assert_eq!(super::Prompt::default().marker, ".");
        assert_eq!(input_viewport("hello", 2, 80), ("hello".to_owned(), 4));
    }

    #[test]
    fn long_input_stays_on_one_row_and_tracks_cursor_navigation() {
        let text = "012345678901234567890123456789";
        for cursor in 0..=text.len() {
            let (visible, column) = input_viewport(text, cursor, 20);
            assert!(visible.width() + 2 < 20);
            assert!(column < 20);
        }
        assert!(input_viewport(text, text.len(), 20).0.starts_with('<'));
        assert!(input_viewport(text, 0, 20).0.ends_with('>'));
    }

    #[test]
    fn cursor_position_respects_wide_and_combining_text() {
        assert_eq!(
            input_viewport("界e\u{301}", 3, 80),
            ("界e\u{301}".to_owned(), 5)
        );
        let (visible, column) = input_viewport("界界界界界界界界界", 9, 12);
        assert!(visible.width() + 2 < 12);
        assert!(column < 12);
    }
}
