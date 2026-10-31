//! Incremental separation of a recognized leading reasoning section.

/// Text emitted by a reasoning-aware parser. Tool parsing uses only content.
#[derive(Default, Debug, Eq, PartialEq)]
pub struct ParsedText {
    /// User-facing answer text.
    pub content: String,
    /// Model reasoning, without transport markers.
    pub reasoning: String,
}

#[derive(Debug)]
enum Phase {
    Leading,
    Reasoning,
    Content,
}

/// Parses a leading think section, preserving later literal/code markers.
#[derive(Debug)]
pub struct ReasoningStreamParser {
    phase: Phase,
    buffer: String,
}

impl ReasoningStreamParser {
    /// Start in reasoning only when the actual template prefills its opener.
    #[must_use]
    pub fn new(prefilled: bool) -> Self {
        Self {
            phase: if prefilled {
                Phase::Reasoning
            } else {
                Phase::Leading
            },
            buffer: String::new(),
        }
    }

    /// Parse arbitrary UTF-8 chunk boundaries; partial markers remain buffered.
    #[must_use]
    pub fn push(&mut self, text: &str) -> ParsedText {
        self.buffer.push_str(text);
        let mut result = ParsedText::default();
        if matches!(self.phase, Phase::Leading) {
            let trimmed = self.buffer.trim_start();
            if trimmed.starts_with("<think>") {
                let skip = self.buffer.len() - trimmed.len() + "<think>".len();
                self.buffer.drain(..skip);
                self.phase = Phase::Reasoning;
            } else if self.buffer.len() <= 1024 && "<think>".starts_with(trimmed) {
                return result;
            } else {
                self.phase = Phase::Content;
            }
        }
        if matches!(self.phase, Phase::Reasoning) {
            if let Some(end) = self.buffer.find("</think>") {
                result.reasoning.push_str(&self.buffer[..end]);
                self.buffer.drain(..end + "</think>".len());
                self.phase = Phase::Content;
            } else {
                let keep = (1.."</think>".len())
                    .rev()
                    .find(|n| self.buffer.as_bytes().ends_with(&b"</think>"[..*n]))
                    .unwrap_or(0);
                let safe = self.buffer.len() - keep;
                result
                    .reasoning
                    .push_str(&self.buffer.drain(..safe).collect::<String>());
                return result;
            }
        }
        result.content.push_str(&std::mem::take(&mut self.buffer));
        result
    }

    /// Flush incomplete markers as text in their current section on termination.
    #[must_use]
    pub fn finish(&mut self) -> ParsedText {
        let text = std::mem::take(&mut self.buffer);
        if matches!(self.phase, Phase::Reasoning) {
            ParsedText {
                reasoning: text,
                content: String::new(),
            }
        } else {
            ParsedText {
                content: text,
                reasoning: String::new(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_markers_unicode_and_tool_text_stay_in_their_sections() {
        let text = "<think>考え<tool_call>{\"name\":\"unsafe\"}</tool_call></think>Answer";
        for split in (0..=text.len()).filter(|n| text.is_char_boundary(*n)) {
            let mut parser = ReasoningStreamParser::new(false);
            let a = parser.push(&text[..split]);
            let b = parser.push(&text[split..]);
            let c = parser.finish();
            assert_eq!(
                a.content + b.content.as_str() + c.content.as_str(),
                "Answer"
            );
            assert_eq!(
                a.reasoning + b.reasoning.as_str() + c.reasoning.as_str(),
                "考え<tool_call>{\"name\":\"unsafe\"}</tool_call>"
            );
        }
    }
    #[test]
    fn literals_empty_sections_prefill_and_incomplete_markers() {
        let mut p = ReasoningStreamParser::new(false);
        assert_eq!(
            p.push("Answer: `<think>literal</think>`").content,
            "Answer: `<think>literal</think>`"
        );
        let mut p = ReasoningStreamParser::new(false);
        assert_eq!(
            p.push("<think></think>OK"),
            ParsedText {
                content: "OK".to_owned(),
                reasoning: String::new()
            }
        );
        let mut p = ReasoningStreamParser::new(true);
        assert_eq!(
            p.push("Reason</think>OK"),
            ParsedText {
                content: "OK".to_owned(),
                reasoning: "Reason".to_owned()
            }
        );
        let mut p = ReasoningStreamParser::new(false);
        assert_eq!(p.push("<thi"), ParsedText::default());
        assert_eq!(p.finish().content, "<thi");
        let mut p = ReasoningStreamParser::new(true);
        assert_eq!(p.push("unfinished </thi").reasoning, "unfinished ");
        assert_eq!(p.finish().reasoning, "</thi");
    }
}
