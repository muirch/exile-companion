//! Any model behind OpenAI's chat completions: OpenRouter, or a server the player names, such as
//! DeepSeek's API, LM Studio or Ollama.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::config::{Effort, Usage};
use crate::stream::Delta;
use crate::turn::{Call, Dialect, Finish, Llm, Reply, Setup};
use crate::{http, ChatError};

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

pub struct OpenAi {
    agent: ureq::Agent,
    url: String,
    key: Option<String>,
    model: String,
    effort: Option<Effort>,
    openrouter: bool,
}

impl OpenAi {
    pub fn openrouter(key: String, model: String, effort: Effort, user_agent: &str) -> Self {
        Self {
            agent: http::agent(user_agent),
            url: OPENROUTER_URL.into(),
            key: Some(key),
            model: model.trim().to_string(),
            effort: Some(effort),
            openrouter: true,
        }
    }

    /// A local server often needs no key. `effort` goes out as reasoning_effort, which Ollama
    /// takes and other servers may reject, so only pass it to a server known to take it.
    pub fn custom(
        base_url: &str,
        key: Option<String>,
        model: String,
        effort: Option<Effort>,
        user_agent: &str,
    ) -> Self {
        Self {
            agent: http::agent(user_agent),
            url: completions_url(base_url),
            key: key.filter(|k| !k.trim().is_empty()),
            model: model.trim().to_string(),
            effort,
            openrouter: false,
        }
    }
}

/// Takes the base URL as servers document it ("http://localhost:11434/v1") or the full endpoint.
pub fn completions_url(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

pub fn body(
    model: &str,
    effort: Option<Effort>,
    openrouter: bool,
    setup: &Setup<'_>,
    messages: &[Value],
) -> Value {
    let mut all = vec![json!({ "role": "system", "content": setup.system })];
    all.extend(messages.iter().cloned());
    let tools: Vec<Value> = setup
        .tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": { "name": t.name, "description": t.description, "parameters": t.schema },
            })
        })
        .collect();
    let mut b = json!({ "model": model, "stream": true, "messages": all, "tools": tools });
    if openrouter {
        if let Some(e) = effort {
            b["reasoning"] = json!({ "effort": e.as_str() });
        }
    } else {
        // OpenAI's servers only send usage when asked. OpenRouter always does.
        b["stream_options"] = json!({ "include_usage": true });
        if let Some(e) = effort {
            b["reasoning_effort"] = json!(e.as_str());
        }
    }
    b
}

impl Llm for OpenAi {
    fn dialect(&self) -> Dialect {
        Dialect::OpenAi
    }

    fn body(&self, setup: &Setup<'_>, messages: &[Value]) -> Value {
        body(&self.model, self.effort, self.openrouter, setup, messages)
    }

    fn send(&self, body: Value) -> mpsc::Receiver<Result<Value, ChatError>> {
        let mut headers = Vec::new();
        if let Some(key) = &self.key {
            headers.push(("authorization", format!("Bearer {key}")));
        }
        if self.openrouter {
            // OpenRouter's app attribution.
            headers.push((
                "http-referer",
                "https://github.com/muirch/exile-companion".into(),
            ));
            headers.push(("x-title", "Exile Companion".into()));
        }
        http::post_stream(self.agent.clone(), self.url.clone(), headers, body)
    }

    fn assembler(&self) -> Box<dyn crate::turn::Assembler> {
        Box::new(Assembler::default())
    }
}

#[derive(Debug, Default)]
struct PartialCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Debug, Default)]
pub struct Assembler {
    text: String,
    calls: BTreeMap<u64, PartialCall>,
    /// OpenRouter's reasoning_details, merged from the stream's fragments. Models that reason
    /// between tool calls need them back unchanged.
    reasoning: Vec<Value>,
    finish: Option<String>,
    usage: Usage,
    done: bool,
}

const REASONING_TEXT: [&str; 3] = ["text", "summary", "data"];

impl Assembler {
    fn add_reasoning(&mut self, piece: &Value) {
        let same = self.reasoning.last().is_some_and(|last| {
            last["type"] == piece["type"] && last.get("index") == piece.get("index")
        });
        if !same {
            self.reasoning.push(piece.clone());
            return;
        }
        let Some(last) = self.reasoning.last_mut().and_then(Value::as_object_mut) else {
            return;
        };
        let Some(piece) = piece.as_object() else {
            return;
        };
        for (k, v) in piece {
            match (REASONING_TEXT.contains(&k.as_str()), v.as_str()) {
                (true, Some(s)) => {
                    let old = last.get(k).and_then(Value::as_str).unwrap_or("");
                    last.insert(k.clone(), Value::String(format!("{old}{s}")));
                }
                _ if !v.is_null() => {
                    last.insert(k.clone(), v.clone());
                }
                _ => {}
            }
        }
    }
}

impl crate::turn::Assembler for Assembler {
    fn apply(&mut self, event: &Value) -> Result<Option<Delta>, ChatError> {
        if event["done"] == true {
            self.done = true;
            return Ok(None);
        }
        if let Some(e) = event.get("error").filter(|e| !e.is_null()) {
            return Err(ChatError::Api {
                status: e["code"].as_u64().map(|c| c as u16).unwrap_or(0),
                kind: e["code"].as_str().unwrap_or("").to_string(),
                message: e["message"].as_str().unwrap_or("").to_string(),
            });
        }
        if let Some(u) = event.get("usage").filter(|u| u.is_object()) {
            let prompt = u["prompt_tokens"].as_u64().unwrap_or(0);
            let cached = u
                .pointer("/prompt_tokens_details/cached_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            self.usage = Usage {
                input_tokens: prompt.saturating_sub(cached),
                output_tokens: u["completion_tokens"].as_u64().unwrap_or(0),
                cache_read_tokens: cached,
                cache_write_tokens: 0,
                cost: u["cost"].as_f64(),
            };
        }
        let Some(choice) = event.pointer("/choices/0") else {
            return Ok(None);
        };
        if let Some(r) = choice["finish_reason"].as_str() {
            self.finish = Some(r.to_string());
        }
        let delta = &choice["delta"];
        if let Some(pieces) = delta["reasoning_details"].as_array() {
            for p in pieces {
                self.add_reasoning(p);
            }
        }
        let mut seen = None;
        if let Some(calls) = delta["tool_calls"].as_array() {
            for (n, c) in calls.iter().enumerate() {
                let index = c["index"].as_u64().unwrap_or(n as u64);
                let call = self.calls.entry(index).or_default();
                if let Some(id) = c["id"].as_str().filter(|s| !s.is_empty()) {
                    call.id = id.to_string();
                }
                if let Some(name) = c.pointer("/function/name").and_then(Value::as_str) {
                    if call.name.is_empty() && !name.is_empty() {
                        call.name = name.to_string();
                        seen = Some(Delta::Tool(call.name.clone()));
                    }
                }
                if let Some(args) = c.pointer("/function/arguments").and_then(Value::as_str) {
                    call.arguments.push_str(args);
                }
            }
        }
        if let Some(text) = delta["content"].as_str().filter(|t| !t.is_empty()) {
            self.text.push_str(text);
            return Ok(Some(Delta::Text(text.to_string())));
        }
        Ok(seen)
    }

    fn finish(self: Box<Self>) -> Result<Reply, ChatError> {
        // Some local servers end without [DONE] after the finish reason.
        if !self.done && self.finish.is_none() {
            return Err(ChatError::Truncated);
        }
        let calls: Vec<Call> = self
            .calls
            .into_iter()
            .enumerate()
            .map(|(n, (_, c))| {
                let raw = c.arguments.trim();
                let input = if raw.is_empty() {
                    Ok(json!({}))
                } else {
                    match serde_json::from_str::<Value>(raw) {
                        Ok(v) if v.is_object() => Ok(v),
                        _ => Err(c.arguments.clone()),
                    }
                };
                Call {
                    // A few local servers leave the id out. The tool result needs one.
                    id: if c.id.is_empty() {
                        format!("call_{n}")
                    } else {
                        c.id
                    },
                    name: c.name,
                    input,
                }
            })
            .collect();
        let mut message = json!({
            "role": "assistant",
            "content": if self.text.is_empty() && !calls.is_empty() { Value::Null } else { Value::String(self.text) },
        });
        if !calls.is_empty() {
            message["tool_calls"] = calls
                .iter()
                .map(|c| {
                    json!({
                        "id": c.id,
                        "type": "function",
                        "function": {
                            "name": c.name,
                            "arguments": match &c.input {
                                Ok(v) => v.to_string(),
                                Err(_) => "{}".to_string(),
                            },
                        },
                    })
                })
                .collect();
        }
        if !self.reasoning.is_empty() {
            message["reasoning_details"] = Value::Array(self.reasoning);
        }
        let finish = match self.finish.as_deref() {
            Some("length") => Finish::Cut,
            Some("content_filter") => Finish::Refused(None),
            _ if !calls.is_empty() => Finish::ToolUse,
            _ => Finish::End,
        };
        Ok(Reply {
            message,
            calls,
            finish,
            usage: self.usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::turn::{Assembler as _, ToolDef};

    fn run(events: &[Value]) -> (Vec<Delta>, Result<Reply, ChatError>) {
        let mut a = Box::new(Assembler::default());
        let mut deltas = Vec::new();
        for e in events {
            if let Some(d) = a.apply(e).unwrap() {
                deltas.push(d);
            }
        }
        (deltas, a.finish())
    }

    #[test]
    fn rebuilds_streamed_tool_calls() {
        let (deltas, reply) = run(&[
            json!({"choices":[{"delta":{"role":"assistant","content":"Checking."}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","type":"function","function":{"name":"passive_value","arguments":""}}]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"limit\""}}]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":": 5}"}},{"index":1,"id":"c2","function":{"name":"player_status","arguments":"{}"}}]}}]}),
            json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]}),
            json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":1200,"completion_tokens":40,"prompt_tokens_details":{"cached_tokens":1000},"cost":0.0021}}),
            json!({"done": true}),
        ]);
        assert_eq!(
            deltas,
            vec![
                Delta::Text("Checking.".into()),
                Delta::Tool("passive_value".into()),
                Delta::Tool("player_status".into())
            ]
        );
        let reply = reply.unwrap();
        assert_eq!(reply.finish, Finish::ToolUse);
        assert_eq!(reply.calls.len(), 2);
        assert_eq!(reply.calls[0].input, Ok(json!({"limit": 5})));
        assert_eq!(
            reply.message["tool_calls"][0]["function"]["arguments"],
            "{\"limit\":5}"
        );
        assert_eq!(reply.message["content"], "Checking.");
        assert_eq!(reply.usage.input_tokens, 200);
        assert_eq!(reply.usage.cache_read_tokens, 1000);
        assert_eq!(reply.usage.cost, Some(0.0021));
    }

    #[test]
    fn merges_reasoning_fragments_and_sends_them_back() {
        let (_, reply) = run(&[
            json!({"choices":[{"delta":{"reasoning_details":[{"type":"reasoning.text","index":0,"text":"Look at ","format":"x"}]}}]}),
            json!({"choices":[{"delta":{"reasoning_details":[{"type":"reasoning.text","index":0,"text":"the tree.","signature":null}]}}]}),
            json!({"choices":[{"delta":{"reasoning_details":[{"type":"reasoning.encrypted","index":1,"data":"abc"}]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"x","arguments":"{}"}}]},"finish_reason":"tool_calls"}]}),
            json!({"done": true}),
        ]);
        let details = &reply.unwrap().message["reasoning_details"];
        assert_eq!(
            details,
            &json!([
                {"type":"reasoning.text","index":0,"text":"Look at the tree.","format":"x"},
                {"type":"reasoning.encrypted","index":1,"data":"abc"}
            ])
        );
    }

    #[test]
    fn mid_stream_error_is_an_api_error() {
        let mut a = Assembler::default();
        let e = json!({"error":{"code":"server_error","message":"Provider disconnected unexpectedly"},"choices":[{"delta":{},"finish_reason":"error"}]});
        assert!(
            matches!(a.apply(&e), Err(ChatError::Api { ref message, .. }) if message.contains("disconnected"))
        );
    }

    #[test]
    fn finish_reasons() {
        let (_, cut) = run(&[
            json!({"choices":[{"delta":{"content":"Long"},"finish_reason":"length"}]}),
            json!({"done": true}),
        ]);
        assert_eq!(cut.unwrap().finish, Finish::Cut);
        let (_, filtered) =
            run(&[json!({"choices":[{"delta":{},"finish_reason":"content_filter"}]})]);
        assert_eq!(filtered.unwrap().finish, Finish::Refused(None));
        let (_, broken) = run(&[json!({"choices":[{"delta":{"content":"Half"}}]})]);
        assert!(matches!(broken, Err(ChatError::Truncated)));
    }

    #[test]
    fn broken_arguments_are_kept_aside() {
        let (_, reply) = run(&[
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"what_if","arguments":"{\"changes\": ["}}]},"finish_reason":"tool_calls"}]}),
            json!({"done": true}),
        ]);
        let reply = reply.unwrap();
        assert_eq!(reply.calls[0].id, "call_0");
        assert_eq!(reply.calls[0].input, Err("{\"changes\": [".into()));
        assert_eq!(
            reply.message["tool_calls"][0]["function"]["arguments"],
            "{}"
        );
    }

    #[test]
    fn body_puts_the_system_prompt_first_and_effort_only_on_openrouter() {
        let tools = [ToolDef {
            name: "player_status".into(),
            description: "Where".into(),
            schema: json!({"type": "object"}),
        }];
        let setup = Setup {
            system: "sys",
            tools: &tools,
        };
        let msgs = [json!({"role": "user", "content": "hi"})];
        let b = body("vendor/model", Some(Effort::Low), true, &setup, &msgs);
        assert_eq!(
            b["messages"][0],
            json!({"role": "system", "content": "sys"})
        );
        assert_eq!(b["messages"][1]["content"], "hi");
        assert_eq!(b["tools"][0]["function"]["name"], "player_status");
        assert_eq!(b["reasoning"]["effort"], "low");
        let local = body("qwen", None, false, &setup, &msgs);
        assert!(local.get("reasoning").is_none());
        assert!(local.get("reasoning_effort").is_none());
        assert_eq!(local["stream_options"]["include_usage"], true);
        let ollama = body("qwen", Some(Effort::Low), false, &setup, &msgs);
        assert_eq!(ollama["reasoning_effort"], "low");
    }

    #[test]
    fn takes_a_base_url_or_the_endpoint() {
        assert_eq!(
            completions_url("http://localhost:11434/v1/"),
            "http://localhost:11434/v1/chat/completions"
        );
        assert_eq!(
            completions_url("https://api.deepseek.com/chat/completions"),
            "https://api.deepseek.com/chat/completions"
        );
    }
}
