//! Claude over the Messages API with the player's own key.

use std::collections::HashMap;

use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::config::{Effort, Model, Usage};
use crate::stream::Delta;
use crate::turn::{Call, Dialect, Finish, Llm, Reply, Setup};
use crate::{http, ChatError};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
/// Progress notes between tool calls come back readable (thinking display "updates"), and a
/// policy decline is retried on the model Anthropic picks for its category.
const BETAS: &str = "thinking-display-updates-2026-08-18,server-side-fallback-2026-07-01";
// A player asks every ten or twenty minutes, so the 5-minute cache would be written again for
// almost every question. An hour costs twice the input price once instead.
const CACHE: &str = "1h";
// Room for thinking plus the answer. Streaming keeps a large value from timing out.
const MAX_TOKENS: u32 = 64_000;

pub struct Anthropic {
    agent: ureq::Agent,
    key: String,
    model: Model,
    effort: Effort,
}

impl Anthropic {
    pub fn new(key: String, model: Model, effort: Effort, user_agent: &str) -> Self {
        Self {
            agent: http::agent(user_agent),
            key,
            model,
            effort,
        }
    }
}

/// The system prompt and the tools come first and don't change within a conversation, so they
/// stay in the prompt cache. The top-level breakpoint moves along with the messages.
pub fn body(model: Model, effort: Effort, setup: &Setup<'_>, messages: &[Value]) -> Value {
    let tools: Vec<Value> = setup
        .tools
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.schema,
                "eager_input_streaming": true,
            })
        })
        .collect();
    let mut b = json!({
        "model": model.id(),
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "system": [{ "type": "text", "text": setup.system, "cache_control": { "type": "ephemeral", "ttl": CACHE } }],
        "tools": tools,
        "messages": messages,
        "thinking": { "type": "adaptive" },
        "output_config": { "effort": effort.as_str() },
        "cache_control": { "type": "ephemeral", "ttl": CACHE },
    });
    if model.has_updates_and_fallback() {
        b["thinking"]["display"] = json!("updates");
        b["fallbacks"] = json!("default");
    }
    b
}

impl Llm for Anthropic {
    fn dialect(&self) -> Dialect {
        Dialect::Anthropic
    }

    fn body(&self, setup: &Setup<'_>, messages: &[Value]) -> Value {
        body(self.model, self.effort, setup, messages)
    }

    fn send(&self, body: Value) -> mpsc::Receiver<Result<Value, ChatError>> {
        let mut headers = vec![
            ("x-api-key", self.key.clone()),
            ("anthropic-version", API_VERSION.into()),
        ];
        if self.model.has_updates_and_fallback() {
            headers.push(("anthropic-beta", BETAS.into()));
        }
        http::post_stream(self.agent.clone(), API_URL.into(), headers, body)
    }

    fn assembler(&self) -> Box<dyn crate::turn::Assembler> {
        Box::new(Assembler::new(self.model))
    }
}

pub struct Assembler {
    model: Model,
    content: Vec<Value>,
    stop_reason: Option<String>,
    stop_details: Option<Value>,
    usage: Usage,
    /// Streamed input of tool_use blocks that didn't parse, by block index. Their input is {}.
    bad_inputs: HashMap<usize, String>,
    stopped: bool,
    partial: HashMap<usize, String>,
    had_text: bool,
}

impl Assembler {
    pub fn new(model: Model) -> Self {
        Self {
            model,
            content: Vec::new(),
            stop_reason: None,
            stop_details: None,
            usage: Usage::default(),
            bad_inputs: HashMap::new(),
            stopped: false,
            partial: HashMap::new(),
            had_text: false,
        }
    }

    /// The content to keep in the conversation. After a mid-answer fallback, the declined
    /// model's thinking and tool calls before the last fallback block are left out, as the API
    /// asks. Its text stays.
    fn for_history(&self) -> Vec<(usize, Value)> {
        let content: Vec<(usize, &Value)> = self
            .content
            .iter()
            .enumerate()
            .filter(|(_, b)| !b.is_null())
            .collect();
        let boundary = content
            .iter()
            .rposition(|(_, b)| b["type"] == "fallback")
            .unwrap_or(0);
        content
            .iter()
            .enumerate()
            .filter(|(at, (_, b))| *at >= boundary || b["type"] == "text")
            .map(|(_, (i, b))| (*i, (*b).clone()))
            .collect()
    }
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

impl crate::turn::Assembler for Assembler {
    fn apply(&mut self, event: &Value) -> Result<Option<Delta>, ChatError> {
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
                let append = |block: &mut Value, field: &str, piece: &str| -> String {
                    let whole = format!(
                        "{}{piece}",
                        block.get(field).and_then(Value::as_str).unwrap_or("")
                    );
                    block[field] = Value::String(whole.clone());
                    whole
                };
                match delta.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text_delta" => {
                        let piece = delta["text"].as_str().unwrap_or("");
                        append(block, "text", piece);
                        return Ok(Some(Delta::Text(piece.to_string())));
                    }
                    "thinking_delta" => {
                        let whole =
                            append(block, "thinking", delta["thinking"].as_str().unwrap_or(""));
                        if !whole.trim().is_empty() {
                            return Ok(Some(Delta::Note(whole)));
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

    fn finish(self: Box<Self>) -> Result<Reply, ChatError> {
        if !self.stopped {
            return Err(ChatError::Truncated);
        }
        let kept = self.for_history();
        let calls = kept
            .iter()
            .filter(|(_, b)| b["type"] == "tool_use")
            .map(|(i, b)| Call {
                id: b["id"].as_str().unwrap_or("").to_string(),
                name: b["name"].as_str().unwrap_or("").to_string(),
                input: match self.bad_inputs.get(i) {
                    Some(raw) => Err(raw.clone()),
                    None => Ok(b["input"].clone()),
                },
            })
            .collect();
        let finish = match self.stop_reason.as_deref() {
            Some("refusal") => Finish::Refused(
                self.stop_details
                    .as_ref()
                    .and_then(|d| d["category"].as_str())
                    .map(str::to_string),
            ),
            Some("max_tokens") => Finish::Cut,
            Some("pause_turn") => Finish::Pause,
            Some("tool_use") => Finish::ToolUse,
            _ => Finish::End,
        };
        let content: Vec<Value> = kept.into_iter().map(|(_, b)| b).collect();
        Ok(Reply {
            message: json!({ "role": "assistant", "content": content }),
            calls,
            finish,
            usage: self.usage.priced(self.model),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::SseLines;
    use crate::turn::{Assembler as _, ToolDef};

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
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Checking where "}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"you are."}}

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
        let mut a = Box::new(Assembler::new(Model::Sonnet));
        let deltas: Vec<Delta> = events(TOOL_TURN)
            .iter()
            .filter_map(|e| a.apply(e).unwrap())
            .collect();
        assert_eq!(
            deltas,
            vec![
                Delta::Note("Checking where ".into()),
                Delta::Note("Checking where you are.".into()),
                Delta::Tool("passive_value".into())
            ]
        );
        let reply = a.finish().unwrap();
        let content = &reply.message["content"];
        assert_eq!(content[0]["thinking"], "Checking where you are.");
        assert_eq!(content[0]["signature"], "sig");
        assert_eq!(content[1]["input"], json!({"limit": 5}));
        assert_eq!(reply.finish, Finish::ToolUse);
        assert_eq!(reply.calls[0].input, Ok(json!({"limit": 5})));
        assert_eq!(reply.usage.cache_read_tokens, 3000);
        assert_eq!(reply.usage.output_tokens, 80);
        assert!(reply.usage.cost.unwrap() > 0.0);
    }

    #[test]
    fn broken_tool_input_is_kept_aside() {
        let mut a = Box::new(Assembler::new(Model::Sonnet));
        for e in [
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t","name":"what_if","input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"changes\": [\"a"}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"}}),
            json!({"type":"message_stop"}),
        ] {
            a.apply(&e).unwrap();
        }
        let reply = a.finish().unwrap();
        assert_eq!(reply.message["content"][0]["input"], json!({}));
        assert_eq!(reply.calls[0].input, Err("{\"changes\": [\"a".into()));
    }

    #[test]
    fn second_text_block_starts_a_paragraph() {
        let mut a = Assembler::new(Model::Sonnet);
        let start = |i: u64| json!({"type":"content_block_start","index":i,"content_block":{"type":"text","text":""}});
        assert_eq!(a.apply(&start(0)).unwrap(), None);
        assert_eq!(a.apply(&start(2)).unwrap(), Some(Delta::Paragraph));
    }

    #[test]
    fn stream_error_is_an_api_error() {
        let mut a = Assembler::new(Model::Sonnet);
        let e = json!({"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}});
        assert!(
            matches!(a.apply(&e), Err(ChatError::Api { kind, .. }) if kind == "overloaded_error")
        );
    }

    #[test]
    fn history_drops_the_declined_models_calls_before_a_fallback() {
        let mut a = Assembler::new(Model::Sonnet);
        a.content = vec![
            json!({"type":"thinking","thinking":"","signature":"a"}),
            json!({"type":"text","text":"Partial"}),
            json!({"type":"tool_use","id":"t0","name":"x","input":{}}),
            json!({"type":"fallback","from":{"model":"a"},"to":{"model":"b"}}),
            json!({"type":"thinking","thinking":"","signature":"b"}),
            json!({"type":"tool_use","id":"t1","name":"y","input":{}}),
        ];
        a.stopped = true;
        a.stop_reason = Some("tool_use".into());
        let reply = Box::new(a).finish().unwrap();
        let kept: Vec<&str> = reply.message["content"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["type"].as_str().unwrap())
            .collect();
        assert_eq!(kept, ["text", "fallback", "thinking", "tool_use"]);
        assert_eq!(reply.calls.len(), 1);
        assert_eq!(reply.calls[0].id, "t1");
    }

    #[test]
    fn body_has_no_sampling_params_or_forced_tools() {
        let tools = [ToolDef {
            name: "player_status".into(),
            description: "Where".into(),
            schema: json!({"type": "object"}),
        }];
        let setup = Setup {
            system: "sys",
            tools: &tools,
        };
        let b = body(
            Model::Opus,
            Effort::High,
            &setup,
            &[json!({"role": "user", "content": "hi"})],
        );
        assert_eq!(b["model"], "claude-opus-5-5");
        assert_eq!(b["output_config"]["effort"], "high");
        assert_eq!(b["tools"][0]["eager_input_streaming"], true);
        assert_eq!(b["tools"][0]["input_schema"], json!({"type": "object"}));
        assert_eq!(b["cache_control"]["ttl"], "1h");
        assert_eq!(b["thinking"]["display"], "updates");
        assert_eq!(b["fallbacks"], "default");
        let haiku = body(Model::Haiku, Effort::Low, &setup, &[]);
        assert!(haiku.get("fallbacks").is_none());
        assert!(haiku["thinking"].get("display").is_none());
        for banned in ["temperature", "top_p", "top_k", "tool_choice"] {
            assert!(b.get(banned).is_none(), "{banned}");
        }
    }
}
