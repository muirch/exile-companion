//! Turns the Messages API's server-sent events back into the assistant's message.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::config::Usage;
use crate::ChatError;

/// Collects the data lines of one server-sent event. The event's type is in its JSON too, so the
/// `event:` line is skipped.
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

/// What the player can see while the message streams.
#[derive(Debug, Clone, PartialEq)]
pub enum Delta {
    /// The start of a new text block, after an earlier one in the same answer.
    Paragraph,
    Text(String),
    /// A progress note the model writes between tool calls.
    Note(String),
    Tool(String),
}

#[derive(Debug, Default)]
pub struct Message {
    pub content: Vec<Value>,
    pub stop_reason: Option<String>,
    pub stop_details: Option<Value>,
    pub usage: Usage,
    /// Streamed input of tool_use blocks that didn't parse, by block index. Their input is {}.
    pub bad_inputs: HashMap<usize, String>,
    pub stopped: bool,
    partial: HashMap<usize, String>,
    had_text: bool,
}

fn merge_usage(usage: &mut Usage, v: &Value) {
    let field = |name: &str| v.get(name).and_then(Value::as_u64);
    if let Some(n) = field("input_tokens") {
        usage.input_tokens = n;
    }
    if let Some(n) = field("output_tokens") {
        usage.output_tokens = n;
    }
    if let Some(n) = field("cache_read_input_tokens") {
        usage.cache_read_tokens = n;
    }
    if let Some(n) = field("cache_creation_input_tokens") {
        usage.cache_write_tokens = n;
    }
}

impl Message {
    pub fn apply(&mut self, event: &Value) -> Result<Option<Delta>, ChatError> {
        let index = event
            .get("index")
            .and_then(Value::as_u64)
            .map(|i| i as usize);
        match event.get("type").and_then(Value::as_str).unwrap_or("") {
            "message_start" => {
                if let Some(u) = event.pointer("/message/usage") {
                    merge_usage(&mut self.usage, u);
                }
            }
            "content_block_start" => {
                let (Some(i), Some(block)) = (index, event.get("content_block")) else {
                    return Ok(None);
                };
                if self.content.len() <= i {
                    self.content.resize(i + 1, Value::Null);
                }
                self.content[i] = block.clone();
                match block.get("type").and_then(Value::as_str) {
                    Some("tool_use") => {
                        let name = block["name"].as_str().unwrap_or("").to_string();
                        return Ok(Some(Delta::Tool(name)));
                    }
                    Some("text") => {
                        let first = !self.had_text;
                        self.had_text = true;
                        if !first {
                            return Ok(Some(Delta::Paragraph));
                        }
                    }
                    _ => {}
                }
            }
            "content_block_delta" => {
                let (Some(i), Some(delta)) = (index, event.get("delta")) else {
                    return Ok(None);
                };
                let Some(block) = self.content.get_mut(i) else {
                    return Ok(None);
                };
                let append = |block: &mut Value, field: &str, piece: &str| {
                    let old = block.get(field).and_then(Value::as_str).unwrap_or("");
                    block[field] = Value::String(format!("{old}{piece}"));
                };
                match delta.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text_delta" => {
                        let piece = delta["text"].as_str().unwrap_or("");
                        append(block, "text", piece);
                        return Ok(Some(Delta::Text(piece.to_string())));
                    }
                    "thinking_delta" => {
                        let piece = delta["thinking"].as_str().unwrap_or("");
                        append(block, "thinking", piece);
                        if !piece.trim().is_empty() {
                            return Ok(Some(Delta::Note(piece.to_string())));
                        }
                    }
                    "signature_delta" => {
                        block["signature"] = delta["signature"].clone();
                    }
                    "input_json_delta" => {
                        let piece = delta["partial_json"].as_str().unwrap_or("");
                        self.partial.entry(i).or_default().push_str(piece);
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                let Some(i) = index else {
                    return Ok(None);
                };
                if let Some(raw) = self.partial.remove(&i) {
                    let input = if raw.trim().is_empty() {
                        json!({})
                    } else {
                        match serde_json::from_str::<Value>(&raw) {
                            Ok(v) if v.is_object() => v,
                            _ => {
                                self.bad_inputs.insert(i, raw);
                                json!({})
                            }
                        }
                    };
                    if let Some(block) = self.content.get_mut(i) {
                        block["input"] = input;
                    }
                }
            }
            "message_delta" => {
                if let Some(d) = event.get("delta") {
                    if let Some(r) = d.get("stop_reason").and_then(Value::as_str) {
                        self.stop_reason = Some(r.to_string());
                    }
                    if let Some(s) = d.get("stop_details").filter(|s| !s.is_null()) {
                        self.stop_details = Some(s.clone());
                    }
                }
                if let Some(u) = event.get("usage") {
                    merge_usage(&mut self.usage, u);
                }
            }
            "message_stop" => self.stopped = true,
            "error" => {
                let e = event.get("error").cloned().unwrap_or_default();
                return Err(ChatError::Api {
                    status: 0,
                    kind: e["type"].as_str().unwrap_or("error").to_string(),
                    message: e["message"].as_str().unwrap_or("").to_string(),
                });
            }
            _ => {}
        }
        Ok(None)
    }

    /// The content to keep in the conversation. After a mid-answer fallback, the declined
    /// model's thinking and tool calls before the last fallback block are left out, as the API
    /// asks. Its text stays.
    pub fn for_history(&self) -> Vec<Value> {
        let content: Vec<&Value> = self.content.iter().filter(|b| !b.is_null()).collect();
        let boundary = content
            .iter()
            .rposition(|b| b["type"] == "fallback")
            .unwrap_or(0);
        content
            .iter()
            .enumerate()
            .filter(|(i, b)| *i >= boundary || matches!(b["type"].as_str(), Some("text")))
            .map(|(_, b)| (*b).clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(lines: &str) -> Vec<Value> {
        let mut sse = SseLines::default();
        lines
            .lines()
            .filter_map(|l| sse.line(l))
            .map(|d| serde_json::from_str(&d).unwrap())
            .collect()
    }

    const TOOL_TURN: &str = r#"event: message_start
data: {"type":"message_start","message":{"id":"m","usage":{"input_tokens":12,"cache_read_input_tokens":3000,"cache_creation_input_tokens":40,"output_tokens":1}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Checking where you are."}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: ping
data: {"type":"ping"}

event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"t1","name":"passive_value","input":{}}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"limit\":"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":" 5}"}}

event: content_block_stop
data: {"type":"content_block_stop","index":1}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":80}}

event: message_stop
data: {"type":"message_stop"}

"#;

    #[test]
    fn rebuilds_thinking_and_tool_calls() {
        let mut m = Message::default();
        let deltas: Vec<Delta> = events(TOOL_TURN)
            .iter()
            .filter_map(|e| m.apply(e).unwrap())
            .collect();
        assert_eq!(
            deltas,
            vec![
                Delta::Note("Checking where you are.".into()),
                Delta::Tool("passive_value".into())
            ]
        );
        assert_eq!(m.content[0]["thinking"], "Checking where you are.");
        assert_eq!(m.content[0]["signature"], "sig");
        assert_eq!(m.content[1]["input"], json!({"limit": 5}));
        assert_eq!(m.stop_reason.as_deref(), Some("tool_use"));
        assert!(m.stopped);
        assert_eq!(
            m.usage,
            Usage {
                input_tokens: 12,
                output_tokens: 80,
                cache_read_tokens: 3000,
                cache_write_tokens: 40
            }
        );
    }

    #[test]
    fn broken_tool_input_is_kept_aside() {
        let mut m = Message::default();
        for e in [
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t","name":"what_if","input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"changes\": [\"a"}}),
            json!({"type":"content_block_stop","index":0}),
        ] {
            m.apply(&e).unwrap();
        }
        assert_eq!(m.content[0]["input"], json!({}));
        assert_eq!(m.bad_inputs[&0], "{\"changes\": [\"a");
    }

    #[test]
    fn second_text_block_starts_a_paragraph() {
        let mut m = Message::default();
        let start = |i: u64| json!({"type":"content_block_start","index":i,"content_block":{"type":"text","text":""}});
        assert_eq!(m.apply(&start(0)).unwrap(), None);
        assert_eq!(m.apply(&start(2)).unwrap(), Some(Delta::Paragraph));
    }

    #[test]
    fn stream_error_is_an_api_error() {
        let mut m = Message::default();
        let e = json!({"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}});
        assert!(
            matches!(m.apply(&e), Err(ChatError::Api { kind, .. }) if kind == "overloaded_error")
        );
    }

    #[test]
    fn history_drops_the_declined_models_calls_before_a_fallback() {
        let m = Message {
            content: vec![
                json!({"type":"thinking","thinking":"","signature":"a"}),
                json!({"type":"text","text":"Partial"}),
                json!({"type":"tool_use","id":"t0","name":"x","input":{}}),
                json!({"type":"fallback","from":{"model":"a"},"to":{"model":"b"}}),
                json!({"type":"thinking","thinking":"","signature":"b"}),
                json!({"type":"tool_use","id":"t1","name":"y","input":{}}),
            ],
            ..Default::default()
        };
        let kept: Vec<String> = m
            .for_history()
            .iter()
            .map(|b| b["type"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(kept, ["text", "fallback", "thinking", "tool_use"]);
    }

    #[test]
    fn history_without_fallback_is_unchanged() {
        let m = Message {
            content: vec![
                json!({"type":"thinking","thinking":"","signature":"a"}),
                json!({"type":"text","text":"Answer"}),
            ],
            ..Default::default()
        };
        assert_eq!(m.for_history(), m.content);
    }
}
