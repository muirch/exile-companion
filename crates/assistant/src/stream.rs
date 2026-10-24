/// Collects the data lines of one server-sent event. Anthropic repeats the event's type in its
/// JSON, so `event:` lines are skipped, and so are OpenRouter's `:` keep-alive comments.
#[derive(Debug, Default)]
pub struct SseLines {
    data: String,
}

impl SseLines {
    /// One line without its newline. Returns the event's data when the line ends the event.
    pub fn line(&mut self, line: &str) -> Option<String> {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            return (!self.data.is_empty()).then(|| std::mem::take(&mut self.data));
        }
        if let Some(rest) = line.strip_prefix("data:") {
            if !self.data.is_empty() {
                self.data.push('\n');
            }
            self.data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        }
        None
    }

    /// The last event's data when the stream ended without a blank line after it.
    pub fn finish(&mut self) -> Option<String> {
        self.line("")
    }
}

/// What the player can see while a message streams.
#[derive(Debug, Clone, PartialEq)]
pub enum Delta {
    /// The start of a new text block, after an earlier one in the same answer.
    Paragraph,
    Text(String),
    /// A progress note the model writes between tool calls, whole so far.
    Note(String),
    Tool(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_event_names_and_comments() {
        let mut sse = SseLines::default();
        let mut out = Vec::new();
        for line in [
            ": OPENROUTER PROCESSING",
            "",
            "event: ping",
            "data: {\"type\":\"ping\"}",
            "",
            "data: [DONE]",
        ] {
            out.extend(sse.line(line));
        }
        out.extend(sse.finish());
        assert_eq!(out, ["{\"type\":\"ping\"}", "[DONE]"]);
    }
}
