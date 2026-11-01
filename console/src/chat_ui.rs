//! Shared, terminal-aware presentation for streamed model responses.

#![allow(dead_code)] // Shared by clients with different presentation entry points.

use std::fmt::Write as _;
use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const RESET: &str = "\x1b[0m";
const CYAN: &str = "\x1b[36m";
const BLUE: &str = "\x1b[94m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";

#[derive(Clone, Copy, Debug)]
struct TerminalFeatures {
    interactive: bool,
    color: bool,
}

impl TerminalFeatures {
    fn detect(plain: bool) -> Self {
        Self::resolve(
            io::stdout().is_terminal() && io::stderr().is_terminal(),
            std::env::var("TERM").ok().as_deref(),
            std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()),
            plain
                || std::env::var("TALVIK_PLAIN").is_ok_and(|value| value == "1" || value == "true"),
        )
    }

    fn resolve(tty: bool, term: Option<&str>, no_color: bool, plain: bool) -> Self {
        let interactive = tty && term != Some("dumb") && !plain;
        Self {
            interactive,
            color: interactive && !no_color,
        }
    }

    fn paint(self, style: &str, text: &str) -> String {
        if self.color {
            format!("{style}{text}{RESET}")
        } else {
            text.to_owned()
        }
    }
}

/// Scoped Ctrl+C handling, including while no tokens have arrived.
#[derive(Debug)]
pub struct InterruptGuard {
    registration: signal_hook::SigId,
    interrupted: Arc<AtomicBool>,
}

impl InterruptGuard {
    /// Register an interrupt flag and remove this registration on drop.
    pub fn new() -> io::Result<Self> {
        let interrupted = Arc::new(AtomicBool::new(false));
        let registration =
            signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&interrupted))?;
        Ok(Self {
            registration,
            interrupted,
        })
    }

    /// Consume an interrupt without losing later interrupts.
    pub fn take(&self) -> bool {
        self.interrupted.swap(false, Ordering::SeqCst)
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        signal_hook::low_level::unregister(self.registration);
    }
}

/// Completion states remain distinguishable without color.
#[derive(Clone, Copy, Debug)]
pub enum Outcome {
    Complete,
    Cancelled,
    Truncated,
    ContextLimit,
}

/// A response display owns and cleans up its temporary progress line.
#[derive(Debug)]
pub struct ChatDisplay {
    features: TerminalFeatures,
    renderer: StreamRenderer,
    waiting: Option<WaitingIndicator>,
    started: Instant,
    output_started: bool,
    output_line_open: bool,
    finished: bool,
}

impl ChatDisplay {
    /// Start a response. Piped, plain and dumb terminals get only model text.
    pub fn new(model: &str, plain: bool) -> io::Result<Self> {
        let features = TerminalFeatures::detect(plain);
        if features.interactive {
            writeln!(
                io::stdout(),
                "\n{} {}",
                features.paint(BOLD, "Talvik"),
                features.paint(CYAN, &safe_label(model))
            )?;
        }
        Ok(Self {
            features,
            renderer: StreamRenderer::new(features),
            waiting: features
                .interactive
                .then(|| WaitingIndicator::start(features)),
            started: Instant::now(),
            output_started: false,
            output_line_open: false,
            finished: false,
        })
    }

    /// Whether the presentation includes interactive decoration.
    pub fn interactive(&self) -> bool {
        self.features.interactive
    }

    /// Whether any model tokens have arrived, including partial thinking tags.
    pub fn output_started(&self) -> bool {
        self.output_started
    }

    /// Stream a token immediately; formatting delimiters may cross token boundaries.
    pub fn token(&mut self, token: &str) -> io::Result<()> {
        if token.is_empty() {
            return Ok(());
        }
        self.stop_waiting();
        self.output_started = true;
        let text = self.renderer.push(token);
        self.output_line_open = output_line_open_after(self.output_line_open, &text);
        let mut output = io::stdout().lock();
        output.write_all(text.as_bytes())?;
        output.flush()
    }

    /// Acknowledge an actual cancellation request without claiming completion.
    pub fn cancelling(&mut self) {
        self.stop_waiting();
        if self.features.interactive {
            let line_break = if self.output_line_open { "\n" } else { "" };
            let _ = writeln!(
                io::stderr(),
                "{line_break}{}",
                self.features.paint(DIM, "Cancelling…")
            );
            self.output_line_open = false;
        }
    }

    /// Flush partial markup, reset styles and render a compact completion summary.
    pub fn finish(
        &mut self,
        outcome: Outcome,
        tokens: Option<u64>,
        rate: Option<f64>,
    ) -> io::Result<()> {
        self.stop_waiting();
        self.finished = true;
        let tail = self.renderer.finish();
        let mut output = io::stdout().lock();
        output.write_all(tail.as_bytes())?;
        writeln!(output)?;
        output.flush()?;
        if self.features.interactive {
            let (label, color) = match outcome {
                Outcome::Complete => ("✓ Complete", "\x1b[32m"),
                Outcome::Cancelled => ("Cancelled", "\x1b[33m"),
                Outcome::Truncated => ("Output limit reached", "\x1b[33m"),
                Outcome::ContextLimit => ("Context limit reached", "\x1b[33m"),
            };
            let mut details = format!("{:.1}s", self.started.elapsed().as_secs_f64());
            if let Some(tokens) = tokens {
                let _ = write!(details, " · {tokens} tokens");
            }
            if let Some(rate) = rate.filter(|rate| rate.is_finite() && *rate > 0.0) {
                let _ = write!(details, " · {rate:.1} tok/s");
            }
            writeln!(
                io::stderr(),
                "{} {}\n",
                self.features.paint(color, label),
                self.features.paint(DIM, &details)
            )?;
        }
        Ok(())
    }

    /// Finish a runtime response, preserving truncation and cancellation states.
    pub fn finish_generation(
        &mut self,
        result: &oid_runtime::GenerationResult,
        cancelled: bool,
    ) -> io::Result<()> {
        if !self.output_started() && !result.text.is_empty() {
            self.token(&result.text)?;
        }
        let outcome = if cancelled {
            Outcome::Cancelled
        } else {
            match result.statistics.termination {
                oid_runtime::GenerationTermination::Stop => Outcome::Complete,
                oid_runtime::GenerationTermination::Length => Outcome::Truncated,
                oid_runtime::GenerationTermination::ContextLength => Outcome::ContextLimit,
            }
        };
        self.finish(
            outcome,
            Some(result.statistics.generated_tokens),
            result
                .statistics
                .metrics_known
                .then_some(result.statistics.tokens_per_second),
        )
    }

    fn stop_waiting(&mut self) {
        self.waiting.take();
    }
}

impl Drop for ChatDisplay {
    fn drop(&mut self) {
        self.stop_waiting();
        if !self.finished && self.features.interactive {
            let _ = writeln!(
                io::stdout(),
                "{}",
                if self.features.color { RESET } else { "" }
            );
            let _ = writeln!(
                io::stderr(),
                "{}",
                self.features.paint("\x1b[31m", "Failed")
            );
        }
    }
}

fn safe_label(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control())
        .take(72)
        .collect()
}

fn output_line_open_after(previous: bool, rendered: &str) -> bool {
    rendered
        .chars()
        .last()
        .map_or(previous, |last| last != '\n')
}

#[derive(Debug)]
struct WaitingIndicator {
    stop: mpsc::Sender<()>,
    worker: Option<JoinHandle<()>>,
}

impl WaitingIndicator {
    fn start(features: TerminalFeatures) -> Self {
        let (stop, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let started = Instant::now();
            let mut tick = 0;
            let width = std::env::var("COLUMNS")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(80);
            loop {
                let label = if width < 38 {
                    "Waiting"
                } else {
                    "Waiting for model"
                };
                let line = format!(
                    "{} {label} · {:.1}s",
                    frames[tick % frames.len()],
                    started.elapsed().as_secs_f64()
                );
                let line = line
                    .chars()
                    .take(width.saturating_sub(1).max(1))
                    .collect::<String>();
                let _ = write!(io::stderr(), "\r\x1b[2K{}", features.paint(DIM, &line));
                let _ = io::stderr().flush();
                if receiver.recv_timeout(Duration::from_millis(100))
                    != Err(mpsc::RecvTimeoutError::Timeout)
                {
                    break;
                }
                tick += 1;
            }
            let _ = write!(io::stderr(), "\r\x1b[2K");
            let _ = io::stderr().flush();
        });
        Self {
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for WaitingIndicator {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Waiting,
    ThinkingPending,
    Thinking,
    Generating,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MarkupContext {
    Prose,
    InlineCode,
    FencedCode,
}

/// A small streaming highlighter, not a full Markdown parser. It preserves
/// Markdown text and never interprets a thinking delimiter inside a code block.
#[derive(Debug)]
struct StreamRenderer {
    features: TerminalFeatures,
    pending: String,
    phase: Phase,
    context: MarkupContext,
    bold: bool,
    heading: bool,
    line_start: bool,
    current_style: &'static str,
}

impl StreamRenderer {
    fn new(features: TerminalFeatures) -> Self {
        Self {
            features,
            pending: String::new(),
            phase: Phase::Waiting,
            context: MarkupContext::Prose,
            bold: false,
            heading: false,
            line_start: true,
            current_style: "",
        }
    }

    fn section(&mut self, phase: Phase, output: &mut String) {
        self.style("", output);
        if matches!(self.phase, Phase::Thinking | Phase::Generating) {
            output.push_str("\n\n");
        }
        let (label, style) = if phase == Phase::Thinking {
            ("Thinking", BLUE)
        } else {
            ("Generating", CYAN)
        };
        output.push_str(&self.features.paint(style, label));
        output.push('\n');
        self.phase = phase;
    }

    fn style(&mut self, style: &'static str, output: &mut String) {
        if self.features.color && self.current_style != style {
            output.push_str(RESET);
            output.push_str(style);
            self.current_style = style;
        }
    }

    fn push(&mut self, text: &str) -> String {
        if !self.features.interactive {
            return text.to_owned();
        }
        self.pending.extend(
            text.chars()
                .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t')),
        );
        let mut output = String::new();
        while !self.pending.is_empty() {
            let thinking_tag = if self.phase == Phase::Waiting {
                Some("<think>")
            } else if matches!(self.phase, Phase::Thinking | Phase::ThinkingPending) {
                Some("</think>")
            } else {
                None
            };
            if self.context == MarkupContext::Prose {
                if let Some(tag) = thinking_tag {
                    if tag.starts_with(&self.pending) && self.pending.len() < tag.len() {
                        break;
                    }
                    if self.pending.starts_with(tag) {
                        self.pending.drain(..tag.len());
                        if self.phase == Phase::Waiting {
                            self.phase = Phase::ThinkingPending;
                        } else if self.phase == Phase::ThinkingPending {
                            self.phase = Phase::Waiting;
                        } else {
                            self.section(Phase::Generating, &mut output);
                        }
                        continue;
                    }
                }
            }
            if self.phase == Phase::Waiting && !self.pending.starts_with(char::is_whitespace) {
                self.section(Phase::Generating, &mut output);
            }
            if self.phase == Phase::ThinkingPending {
                let ch = self.pending.chars().next().expect("nonempty buffer");
                if ch.is_whitespace() {
                    self.pending.drain(..ch.len_utf8());
                    continue;
                }
                self.section(Phase::Thinking, &mut output);
            }
            if self.line_start && "```".starts_with(&self.pending) && self.pending.len() < 3 {
                break;
            }
            if self.line_start && self.pending.starts_with("```") {
                self.context = if self.context == MarkupContext::FencedCode {
                    MarkupContext::Prose
                } else {
                    MarkupContext::FencedCode
                };
                self.style(CYAN, &mut output);
                output.push_str("```");
                self.pending.drain(..3);
                self.line_start = false;
                continue;
            }
            if self.context != MarkupContext::FencedCode && self.pending == "*" {
                break;
            }
            if self.context == MarkupContext::Prose && self.pending.starts_with("**") {
                self.bold = !self.bold;
                self.style(BOLD, &mut output);
                output.push_str("**");
                self.pending.drain(..2);
                continue;
            }
            let ch = self.pending.chars().next().expect("nonempty buffer");
            self.pending.drain(..ch.len_utf8());
            if self.context != MarkupContext::FencedCode && ch == '`' {
                self.context = if self.context == MarkupContext::InlineCode {
                    MarkupContext::Prose
                } else {
                    MarkupContext::InlineCode
                };
            }
            if self.line_start && ch == '#' {
                self.heading = true;
            }
            let style = if self.context != MarkupContext::Prose || ch == '`' {
                CYAN
            } else if self.bold || self.heading {
                BOLD
            } else {
                ""
            };
            self.style(style, &mut output);
            output.push(ch);
            self.line_start = ch == '\n';
            if self.line_start {
                self.heading = false;
            }
        }
        output
    }

    fn finish(&mut self) -> String {
        let mut output = std::mem::take(&mut self.pending);
        self.style("", &mut output);
        output
    }
}

#[cfg(test)]
mod tests {
    use super::{output_line_open_after, StreamRenderer, TerminalFeatures};

    fn features(color: bool) -> TerminalFeatures {
        TerminalFeatures {
            interactive: true,
            color,
        }
    }

    #[test]
    fn redirects_dumb_terminals_and_plain_are_unstyled() {
        for (out, err, term, plain) in [
            (false, true, Some("xterm"), false),
            (true, false, None, false),
            (true, true, Some("dumb"), false),
            (true, true, None, true),
        ] {
            let f = TerminalFeatures::resolve(out && err, term, false, plain);
            assert!(!f.interactive && !f.color);
            let mut renderer = StreamRenderer::new(f);
            assert_eq!(
                renderer.push("<think>raw</think>\n```rust\n"),
                "<think>raw</think>\n```rust\n"
            );
        }
    }

    #[test]
    fn no_color_keeps_readable_phase_labels_without_ansi() {
        let f = TerminalFeatures::resolve(true, Some("xterm"), true, false);
        assert!(f.interactive && !f.color);
        let mut renderer = StreamRenderer::new(f);
        assert_eq!(
            renderer.push("<think>reason</think>answer"),
            "Thinking\nreason\n\nGenerating\nanswer"
        );
    }

    #[test]
    fn thinking_and_generation_use_distinct_colors() {
        let mut renderer = StreamRenderer::new(features(true));
        let output = renderer.push("<think>reason</think>answer");

        assert!(output.contains("\x1b[94mThinking\x1b[0m"));
        assert!(!output.contains("\x1b[94mreason"));
        assert!(output.contains("\x1b[36mGenerating\x1b[0m"));
        assert!(output.contains("answer"));
    }

    #[test]
    fn cancellation_notice_starts_a_new_line_after_streamed_text() {
        assert!(output_line_open_after(false, "Thinking\nreason"));
        assert!(output_line_open_after(true, ""));
        assert!(!output_line_open_after(true, "reason\n"));
        assert!(!output_line_open_after(false, ""));
    }

    #[test]
    fn arbitrary_token_boundaries_do_not_change_formatting() {
        let text = "<think>考え</think>\n# Result\n**bold** and `code`\n```rust\n<think>literal</think>\n```\nDone.";
        let mut whole = StreamRenderer::new(features(true));
        let expected = whole.push(text) + &whole.finish();
        let mut split = StreamRenderer::new(features(true));
        let mut actual = String::new();
        for ch in text.chars() {
            actual.push_str(&split.push(&ch.to_string()));
        }
        actual.push_str(&split.finish());
        assert_eq!(actual, expected);
        assert!(actual.contains("Thinking"));
        assert!(actual.contains("<think>literal</think>"));
        assert!(actual.contains("\x1b[36m"));
        assert!(actual.contains("\x1b[1m"));
        assert!(actual.ends_with("Done."));
    }

    #[test]
    fn partial_markup_is_flushed_and_terminal_controls_are_filtered() {
        let mut renderer = StreamRenderer::new(features(false));
        assert_eq!(renderer.push("Hello\x1b\x07\r"), "Generating\nHello");
        assert_eq!(renderer.push("*"), "");
        assert_eq!(renderer.finish(), "*");
    }

    #[test]
    fn empty_thinking_wrappers_do_not_create_empty_sections() {
        let mut renderer = StreamRenderer::new(features(false));
        assert_eq!(
            renderer.push("<think>\n\n</think>\nAnswer"),
            "\nGenerating\nAnswer"
        );
    }

    #[test]
    fn ordinary_answers_do_not_claim_thinking_and_stream_before_newlines() {
        let mut renderer = StreamRenderer::new(features(false));
        assert_eq!(renderer.push("Hello"), "Generating\nHello");
        assert_eq!(renderer.push(" world"), " world");
    }
}
