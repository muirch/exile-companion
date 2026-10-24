use std::future::Future;

use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::config::{Config, Usage};
use crate::request;
use crate::stream::{Delta, Message};
use crate::ChatError;

/// One streamed Messages API call. The receiver yields the JSON of each event.
pub trait Llm: Sync {
    fn send(&self, body: Value) -> mpsc::Receiver<Result<Value, ChatError>>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutput {
    pub text: String,
    pub is_error: bool,
}

pub trait Tools: Sync {
    fn call(&self, name: &str, input: Value) -> impl Future<Output = ToolOutput> + Send;
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChatEvent {
    Text { text: String },
    Paragraph,
    Note { text: String },
    Tool { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stop {
    Done,
    /// The answer hit max_tokens.
    Cut,
    Refused,
    TooManySteps,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnEnd {
    pub stop: Stop,
    pub usage: Usage,
    /// The refusal's category, such as "cyber", when the API gave one.
    pub refusal: Option<String>,
}

pub struct Setup<'a> {
    pub config: Config,
    pub system: &'a str,
    pub tools: &'a [Value],
}

// A typical question takes one to four tool rounds. This only stops a model that loops.
const MAX_REQUESTS: usize = 20;
// Wiki pages are cut at 12k characters by the server already. This is for anything else.
const MAX_TOOL_TEXT: usize = 40_000;

#[derive(Debug, Default, Clone)]
pub struct Conversation {
    messages: Vec<Value>,
}

impl Conversation {
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn messages(&self) -> &[Value] {
        &self.messages
    }

    /// Asks one question and runs the tools until the model answers. The conversation keeps the
    /// turn only when it ended with an answer, so an error, a refusal or a dropped future leave
    /// it as it was and the next question goes on from the last answer.
    pub async fn ask<L: Llm, T: Tools>(
        &mut self,
        setup: &Setup<'_>,
        llm: &L,
        tools: &T,
        question: &str,
        mut on: impl FnMut(ChatEvent) + Send,
    ) -> Result<TurnEnd, ChatError> {
        let mut messages = self.messages.clone();
        messages.push(json!({ "role": "user", "content": question }));
        let mut usage = Usage::default();
        let mut wrote_text = false;
        for _ in 0..MAX_REQUESTS {
            let body = request::body(setup.config, setup.system, setup.tools, &messages);
            let mut rx = llm.send(body);
            let mut msg = Message::default();
            while let Some(event) = rx.recv().await {
                match msg.apply(&event?)? {
                    Some(Delta::Text(text)) => {
                        wrote_text = true;
                        on(ChatEvent::Text { text });
                    }
                    Some(Delta::Paragraph) => on(ChatEvent::Paragraph),
                    Some(Delta::Note(text)) => on(ChatEvent::Note { text }),
                    Some(Delta::Tool(name)) => on(ChatEvent::Tool { name }),
                    None => {}
                }
            }
            if !msg.stopped {
                return Err(ChatError::Truncated);
            }
            usage.add(msg.usage);
            let content = msg.for_history();
            let calls: Vec<&Value> = content.iter().filter(|b| b["type"] == "tool_use").collect();
            let end = |stop| TurnEnd {
                stop,
                usage,
                refusal: None,
            };
            match msg.stop_reason.as_deref() {
                Some("refusal") => {
                    let category = msg
                        .stop_details
                        .as_ref()
                        .and_then(|d| d["category"].as_str())
                        .map(str::to_string);
                    return Ok(TurnEnd {
                        refusal: category,
                        ..end(Stop::Refused)
                    });
                }
                Some("max_tokens") => {
                    // A cut tool call is never run, and a turn ending on one can't be kept.
                    if calls.is_empty() {
                        messages.push(json!({ "role": "assistant", "content": content }));
                        self.messages = messages;
                    }
                    return Ok(end(Stop::Cut));
                }
                Some("pause_turn") => {
                    messages.push(json!({ "role": "assistant", "content": content }));
                    continue;
                }
                Some("tool_use") if !calls.is_empty() => {}
                _ => {
                    messages.push(json!({ "role": "assistant", "content": content }));
                    self.messages = messages;
                    return Ok(end(Stop::Done));
                }
            }
            if wrote_text {
                on(ChatEvent::Paragraph);
                wrote_text = false;
            }
            let mut results = Vec::with_capacity(calls.len());
            for (i, call) in content.iter().enumerate() {
                if call["type"] != "tool_use" {
                    continue;
                }
                let id = call["id"].clone();
                let output = match msg.bad_inputs.get(&i) {
                    Some(raw) => ToolOutput {
                        text: json!({ "INVALID_JSON": raw }).to_string(),
                        is_error: true,
                    },
                    None => {
                        let name = call["name"].as_str().unwrap_or("");
                        tools.call(name, call["input"].clone()).await
                    }
                };
                results.push(json!({
                    "type": "tool_result",
                    "tool_use_id": id,
                    "content": cut(output.text),
                    "is_error": output.is_error,
                }));
            }
            messages.push(json!({ "role": "assistant", "content": content }));
            messages.push(json!({ "role": "user", "content": results }));
        }
        Ok(TurnEnd {
            stop: Stop::TooManySteps,
            usage,
            refusal: None,
        })
    }
}

fn cut(mut text: String) -> String {
    if text.len() > MAX_TOOL_TEXT {
        let mut at = MAX_TOOL_TEXT;
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        text.truncate(at);
        text.push_str("\n[cut here, the rest was too long]");
    }
    text
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Replays canned answers, one per request, and keeps the bodies it was sent.
    struct Script {
        answers: Mutex<Vec<Vec<Value>>>,
        bodies: Mutex<Vec<Value>>,
    }

    impl Script {
        fn new(answers: Vec<Vec<Value>>) -> Self {
            Self {
                answers: Mutex::new(answers.into_iter().rev().collect()),
                bodies: Mutex::new(Vec::new()),
            }
        }
    }

    impl Llm for Script {
        fn send(&self, body: Value) -> mpsc::Receiver<Result<Value, ChatError>> {
            self.bodies.lock().unwrap().push(body);
            let events = self.answers.lock().unwrap().pop().expect("no more answers");
            let (tx, rx) = mpsc::channel(events.len().max(1));
            for e in events {
                tx.try_send(Ok(e)).unwrap();
            }
            rx
        }
    }

    struct Echo(Mutex<Vec<(String, Value)>>);

    impl Tools for Echo {
        async fn call(&self, name: &str, input: Value) -> ToolOutput {
            self.0.lock().unwrap().push((name.to_string(), input));
            ToolOutput {
                text: format!("{name} ran"),
                is_error: false,
            }
        }
    }

    fn text_answer(text: &str, stop: &str) -> Vec<Value> {
        vec![
            json!({"type":"message_start","message":{"usage":{"input_tokens":10,"output_tokens":1}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":text}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":stop},"usage":{"output_tokens":5}}),
            json!({"type":"message_stop"}),
        ]
    }

    fn tool_answer(name: &str, input: &str) -> Vec<Value> {
        vec![
            json!({"type":"message_start","message":{"usage":{"input_tokens":10,"output_tokens":1}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t1","name":name,"input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":input}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":7}}),
            json!({"type":"message_stop"}),
        ]
    }

    fn setup() -> (Config, Vec<Value>) {
        (
            Config::default(),
            vec![request::tool("player_status", "", json!({"type":"object"}))],
        )
    }

    #[tokio::test]
    async fn runs_tools_until_the_answer() {
        let llm = Script::new(vec![
            tool_answer("player_status", "{}"),
            text_answer("Take Herbalism next.", "end_turn"),
        ]);
        let tools = Echo(Mutex::new(Vec::new()));
        let (config, defs) = setup();
        let s = Setup {
            config,
            system: "sys",
            tools: &defs,
        };
        let mut conv = Conversation::default();
        let mut seen = Vec::new();
        let end = conv
            .ask(&s, &llm, &tools, "What next?", |e| seen.push(e))
            .await
            .unwrap();
        assert_eq!(end.stop, Stop::Done);
        assert_eq!(end.usage.output_tokens, 12);
        assert_eq!(
            seen,
            vec![
                ChatEvent::Tool {
                    name: "player_status".into()
                },
                ChatEvent::Text {
                    text: "Take Herbalism next.".into()
                }
            ]
        );
        assert_eq!(
            tools.0.lock().unwrap()[0],
            ("player_status".into(), json!({}))
        );
        let roles: Vec<&str> = conv
            .messages()
            .iter()
            .map(|m| m["role"].as_str().unwrap())
            .collect();
        assert_eq!(roles, ["user", "assistant", "user", "assistant"]);
        assert_eq!(conv.messages()[2]["content"][0]["tool_use_id"], "t1");
        assert_eq!(
            conv.messages()[2]["content"][0]["content"],
            "player_status ran"
        );
        // The second request carries the tool result, everything before it unchanged.
        let bodies = llm.bodies.lock().unwrap();
        assert_eq!(bodies[1]["messages"][0], bodies[0]["messages"][0]);
        assert_eq!(bodies[1]["messages"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn broken_tool_input_goes_back_as_an_error() {
        let llm = Script::new(vec![
            tool_answer("what_if", "{\"changes\": [\"a"),
            text_answer("Sorry.", "end_turn"),
        ]);
        let tools = Echo(Mutex::new(Vec::new()));
        let (config, defs) = setup();
        let s = Setup {
            config,
            system: "sys",
            tools: &defs,
        };
        let mut conv = Conversation::default();
        conv.ask(&s, &llm, &tools, "q", |_| {}).await.unwrap();
        assert!(tools.0.lock().unwrap().is_empty());
        let result = &conv.messages()[2]["content"][0];
        assert_eq!(result["is_error"], true);
        assert!(result["content"].as_str().unwrap().contains("INVALID_JSON"));
    }

    #[tokio::test]
    async fn a_refusal_leaves_the_conversation_as_it_was() {
        let mut refused = text_answer("", "refusal");
        refused[4] = json!({"type":"message_delta","delta":{"stop_reason":"refusal","stop_details":{"type":"refusal","category":"cyber"}},"usage":{"output_tokens":0}});
        let llm = Script::new(vec![text_answer("Hi.", "end_turn"), refused]);
        let tools = Echo(Mutex::new(Vec::new()));
        let (config, defs) = setup();
        let s = Setup {
            config,
            system: "sys",
            tools: &defs,
        };
        let mut conv = Conversation::default();
        conv.ask(&s, &llm, &tools, "hello", |_| {}).await.unwrap();
        let before = conv.messages().to_vec();
        let end = conv.ask(&s, &llm, &tools, "bad", |_| {}).await.unwrap();
        assert_eq!(end.stop, Stop::Refused);
        assert_eq!(end.refusal.as_deref(), Some("cyber"));
        assert_eq!(conv.messages(), before.as_slice());
    }

    #[tokio::test]
    async fn a_stream_that_breaks_off_is_an_error() {
        let mut broken = text_answer("Half", "end_turn");
        broken.truncate(3);
        let llm = Script::new(vec![broken]);
        let tools = Echo(Mutex::new(Vec::new()));
        let (config, defs) = setup();
        let s = Setup {
            config,
            system: "sys",
            tools: &defs,
        };
        let mut conv = Conversation::default();
        let r = conv.ask(&s, &llm, &tools, "q", |_| {}).await;
        assert!(matches!(r, Err(ChatError::Truncated)));
        assert!(conv.is_empty());
    }

    #[tokio::test]
    async fn a_cut_tool_call_is_not_run_or_kept() {
        let mut cut_call = tool_answer("what_if", "{\"changes\": [");
        cut_call[4] = json!({"type":"message_delta","delta":{"stop_reason":"max_tokens"},"usage":{"output_tokens":9}});
        let llm = Script::new(vec![cut_call]);
        let tools = Echo(Mutex::new(Vec::new()));
        let (config, defs) = setup();
        let s = Setup {
            config,
            system: "sys",
            tools: &defs,
        };
        let mut conv = Conversation::default();
        let end = conv.ask(&s, &llm, &tools, "q", |_| {}).await.unwrap();
        assert_eq!(end.stop, Stop::Cut);
        assert!(tools.0.lock().unwrap().is_empty());
        assert!(conv.is_empty());
    }

    #[test]
    fn long_tool_output_is_cut_on_a_char_boundary() {
        let long = "й".repeat(MAX_TOOL_TEXT);
        let out = cut(long);
        assert!(out.ends_with("too long]"));
        assert!(out.len() < MAX_TOOL_TEXT + 64);
    }
}
