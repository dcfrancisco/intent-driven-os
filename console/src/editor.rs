//! Keyboard-first terminal line editor.

use crate::signals::SignalController;
use crate::{
    history::History, presence::PresenceIndicator, prompt::Prompt, status_bar::StatusFooter,
    terminal_ui,
};
use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind,
        KeyModifiers,
    },
    execute,
};
use oid_runtime::RuntimeService;
use std::io::{self, Read, Write};

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
    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Move one character right.
    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.text.len());
    }

    /// Move to the beginning.
    pub fn home(&mut self) {
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
    /// Read and edit one command line, using terminal events only on a real TTY.
    pub fn read_command(
        prompt: &Prompt,
        history: &mut History,
        runtime: &dyn RuntimeService,
        presence: &mut PresenceIndicator,
        signals: &SignalController,
        footer: &mut StatusFooter,
    ) -> io::Result<EditResult> {
        let terminal_mode = TerminalMode::enter()?;
        let mut stdout = io::stdout().lock();
        if terminal_mode.is_some() {
            let mut session = EditorSession {
                prompt,
                history,
                runtime,
                presence,
                signals,
                footer,
            };
            Self::read_raw(&mut stdout, &mut session)
        } else {
            write!(stdout, "{} ", prompt.marker)?;
            stdout.flush()?;
            Self::read_line_interruptible(signals, &mut stdout)
        }
    }

    fn read_line_interruptible<W: Write>(
        signals: &SignalController,
        output: &mut W,
    ) -> io::Result<EditResult> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("oid-line-reader".to_owned())
            .spawn(move || {
                let mut input = io::stdin().lock();
                let mut line = Vec::new();
                let mut byte = [0_u8; 1];
                let result = loop {
                    let read = match input.read(&mut byte) {
                        Ok(read) => read,
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => break Err(error),
                    };
                    if read != 1 {
                        break if line.is_empty() {
                            Ok(EditResult::Eof)
                        } else {
                            Self::submitted(line)
                        };
                    }
                    if byte[0] == b'\n' || byte[0] == b'\r' {
                        break Self::submitted(line);
                    }
                    line.push(byte[0]);
                };
                let _ = sender.send(result);
            })
            .map_err(|error| io::Error::other(format!("line reader: {error}")))?;

        loop {
            if signals.is_shutdown_requested() {
                return Ok(EditResult::Eof);
            }
            match receiver.recv_timeout(std::time::Duration::from_millis(25)) {
                Ok(result) => {
                    let result = result?;
                    if matches!(result, EditResult::Submitted(_)) {
                        writeln!(output)?;
                    }
                    return Ok(result);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::other("line reader disconnected"));
                }
            }
        }
    }

    fn submitted(line: Vec<u8>) -> io::Result<EditResult> {
        String::from_utf8(line)
            .map(EditResult::Submitted)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn read_raw<W: Write>(
        output: &mut W,
        session: &mut EditorSession<'_>,
    ) -> io::Result<EditResult> {
        let mut buffer = LineBuffer::new();
        let mut editing = false;
        session
            .prompt
            .render_line(output, &buffer, session.footer.columns())?;
        loop {
            if session.signals.is_shutdown_requested() {
                return Ok(EditResult::Eof);
            }
            if session.signals.take_interrupt() {
                return session.cancel(output);
            }
            if !event::poll(std::time::Duration::from_millis(100))? {
                continue;
            }
            match event::read()? {
                Event::Resize(columns, rows) => session.footer.resize(columns, rows)?,
                Event::Paste(text) => {
                    for ch in text.chars() {
                        if matches!(ch, '\r' | '\n' | '\t') {
                            buffer.insert(' ');
                        } else if !ch.is_control() {
                            buffer.insert(ch);
                        }
                    }
                    if !editing {
                        session.runtime.input_started();
                        editing = true;
                    }
                }
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char('c') => return session.cancel(output),
                            KeyCode::Char('d') => {
                                write!(output, "\r\n")?;
                                return Ok(EditResult::Eof);
                            }
                            KeyCode::Char('l') => {
                                write!(output, "\x1b[2J\x1b[H")?;
                                session.footer.repaint()?;
                            }
                            KeyCode::Char('a') => buffer.home(),
                            KeyCode::Char('e') => buffer.end(),
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Enter => {
                                session.runtime.input_stopped();
                                session.presence.refresh();
                                write!(output, "\r\n")?;
                                return Ok(EditResult::Submitted(buffer.text()));
                            }
                            KeyCode::Backspace => buffer.backspace(),
                            KeyCode::Delete => buffer.delete(),
                            KeyCode::Left => buffer.left(),
                            KeyCode::Right => buffer.right(),
                            KeyCode::Home => buffer.home(),
                            KeyCode::End => buffer.end(),
                            KeyCode::Up => {
                                if let Some(previous) = session.history.previous() {
                                    buffer.replace(&previous);
                                }
                            }
                            KeyCode::Down => {
                                if let Some(next) = session.history.next() {
                                    buffer.replace(&next);
                                }
                            }
                            KeyCode::Char(ch) => {
                                if !editing {
                                    session.runtime.input_started();
                                    editing = true;
                                }
                                buffer.insert(ch);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            session.presence.refresh();
            session
                .prompt
                .render_line(output, &buffer, session.footer.columns())?;
        }
    }
}

struct EditorSession<'a> {
    prompt: &'a Prompt,
    history: &'a mut History,
    runtime: &'a dyn RuntimeService,
    presence: &'a mut PresenceIndicator,
    signals: &'a SignalController,
    footer: &'a mut StatusFooter,
}

impl EditorSession<'_> {
    fn cancel<W: Write>(&mut self, output: &mut W) -> io::Result<EditResult> {
        self.runtime.cancel_generation();
        self.runtime.input_stopped();
        self.presence.refresh();
        write!(output, "^C\r\n")?;
        Ok(EditResult::Cancelled)
    }
}

struct TerminalMode {
    was_raw: bool,
}

impl TerminalMode {
    fn enter() -> io::Result<Option<Self>> {
        if !terminal_ui::interactive() {
            return Ok(None);
        }
        let was_raw = crossterm::terminal::is_raw_mode_enabled()?;
        crossterm::terminal::enable_raw_mode()?;
        let guard = Self { was_raw };
        execute!(io::stdout(), EnableBracketedPaste)?;
        Ok(Some(guard))
    }
}

impl Drop for TerminalMode {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste);
        if !self.was_raw {
            let _ = crossterm::terminal::disable_raw_mode();
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
