//! Keyboard-first terminal line editor.

use crate::{history::History, presence::PresenceIndicator, prompt::Prompt};
use oid_runtime::RuntimeService;
use std::io::{self, Read, Write};
use std::process::Command;

/// Result of one interactive input cycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditResult {
    /// The user submitted a line.
    Submitted(String),
    /// The user cancelled the current line.
    Cancelled,
    /// Input reached end-of-file.
    Eof,
}

/// Pure editable line buffer used by the terminal adapter.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LineBuffer {
    text: Vec<char>,
    cursor: usize,
}

impl LineBuffer {
    /// Create an empty line.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            text: Vec::new(),
            cursor: 0,
        }
    }

    /// Return the line as a string.
    #[must_use]
    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    /// Return the cursor position in characters.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Replace the current line and place the cursor at its end.
    pub fn replace(&mut self, value: &str) {
        self.text = value.chars().collect();
        self.cursor = self.text.len();
    }

    /// Insert a character at the cursor.
    pub fn insert(&mut self, character: char) {
        self.text.insert(self.cursor, character);
        self.cursor += 1;
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.text.remove(self.cursor);
        }
    }

    /// Delete the character at the cursor.
    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    /// Move one character left.
    pub const fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Move one character right.
    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.text.len());
    }

    /// Move to the beginning.
    pub const fn home(&mut self) {
        self.cursor = 0;
    }

    /// Move to the end.
    pub fn end(&mut self) {
        self.cursor = self.text.len();
    }
}

/// Terminal input adapter with a non-TTY line-oriented fallback.
#[derive(Clone, Debug, Default)]
pub struct LineEditor;

impl LineEditor {
    /// Read and edit one command line.
    pub fn read_command(
        prompt: &Prompt,
        history: &mut History,
        runtime: &dyn RuntimeService,
        presence: &mut PresenceIndicator,
    ) -> io::Result<EditResult> {
        let mut stdin = io::stdin().lock();
        let mut stdout = io::stdout().lock();
        let terminal_mode = TerminalMode::enter();
        if terminal_mode.is_raw() {
            Self::read_raw(&mut stdin, &mut stdout, prompt, history, runtime, presence)
        } else {
            drop(terminal_mode);
            write!(stdout, "{} ", prompt.marker)?;
            stdout.flush()?;
            Self::read_line(&mut stdin, &mut stdout)
        }
    }

    fn read_line<R: Read, W: Write>(input: &mut R, output: &mut W) -> io::Result<EditResult> {
        let mut line = String::new();
        let mut byte = [0_u8; 1];
        while input.read(&mut byte)? == 1 {
            if byte[0] == b'\n' || byte[0] == b'\r' {
                writeln!(output)?;
                return Ok(EditResult::Submitted(line));
            }
            line.push(byte[0] as char);
        }
        Ok(if line.is_empty() {
            EditResult::Eof
        } else {
            EditResult::Submitted(line)
        })
    }

    fn read_raw<R: Read, W: Write>(
        input: &mut R,
        output: &mut W,
        prompt: &Prompt,
        history: &mut History,
        runtime: &dyn RuntimeService,
        presence: &mut PresenceIndicator,
    ) -> io::Result<EditResult> {
        let mut buffer = LineBuffer::new();
        let mut editing = false;
        prompt.render_line(output, &buffer, presence)?;
        loop {
            let mut byte = [0_u8; 1];
            if input.read(&mut byte)? == 0 {
                return Ok(EditResult::Eof);
            }
            match byte[0] {
                3 => {
                    runtime.input_stopped();
                    presence.refresh();
                    writeln!(output, "^C")?;
                    return Ok(EditResult::Cancelled);
                }
                4 => return Ok(EditResult::Eof),
                8 | 127 => buffer.backspace(),
                10 | 13 => {
                    runtime.input_stopped();
                    presence.refresh();
                    writeln!(output)?;
                    return Ok(EditResult::Submitted(buffer.text()));
                }
                12 => {
                    write!(output, "\x1b[2J\x1b[H")?;
                    buffer.end();
                }
                27 => Self::read_escape(input, &mut buffer, history)?,
                byte if byte.is_ascii_graphic() || byte == b' ' => {
                    if !editing {
                        editing = true;
                        runtime.input_started();
                    }
                    buffer.insert(byte as char);
                }
                _ => {}
            }
            presence.refresh();
            prompt.render_line(output, &buffer, presence)?;
        }
    }

    fn read_escape<R: Read>(
        input: &mut R,
        buffer: &mut LineBuffer,
        history: &mut History,
    ) -> io::Result<()> {
        let mut sequence = [0_u8; 2];
        if input.read(&mut sequence[..1])? != 1 || sequence[0] != b'[' {
            return Ok(());
        }
        if input.read(&mut sequence[1..])? != 1 {
            return Ok(());
        }
        match sequence[1] {
            b'A' => {
                if let Some(previous) = history.previous() {
                    buffer.replace(&previous);
                }
            }
            b'B' => {
                if let Some(next) = history.next() {
                    buffer.replace(&next);
                }
            }
            b'C' => buffer.right(),
            b'D' => buffer.left(),
            b'H' | b'1' => buffer.home(),
            b'F' | b'4' => buffer.end(),
            b'3' => {
                let mut terminator = [0_u8; 1];
                let _ = input.read(&mut terminator)?;
                buffer.delete();
            }
            _ => {}
        }
        Ok(())
    }
}

struct TerminalMode {
    previous: Option<String>,
}

impl TerminalMode {
    fn enter() -> Self {
        let previous = Command::new("stty")
            .arg("-g")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned());
        if previous.is_some() {
            let _ = Command::new("stty")
                .args(["-icanon", "-echo", "min", "1", "time", "0"])
                .status();
        }
        Self { previous }
    }

    const fn is_raw(&self) -> bool {
        self.previous.is_some()
    }
}

impl Drop for TerminalMode {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            let _ = Command::new("stty").arg(previous).status();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LineBuffer;

    #[test]
    fn supports_editing_operations() {
        let mut line = LineBuffer::new();
        line.insert('a');
        line.insert('c');
        line.left();
        line.insert('b');
        line.home();
        line.delete();
        line.end();
        line.backspace();
        assert_eq!(line.text(), "b");
    }
}
