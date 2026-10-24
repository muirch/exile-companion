use std::future::Future;

use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::config::Usage;
use crate::stream::Delta;
use crate::ChatError;

/// A tool from the MCP server, before a provider puts it into its own form.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub schema: Value,
}

/// The shape of the messages a provider takes. A conversation stays in one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Anthropic,
    OpenAi,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Finish {
    End,
    ToolUse,
    /// The answer hit the output limit.
    Cut,
    /// With the refusal's category, such as "cyber", when the provider gave one.
    Refused(Option<String>),
    /// Anthropic's pause_turn: send the conversation again as it is.
    Pause,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub id: String,
    pub name: String,
    /// The streamed arguments when they didn't parse as a JSON object.
    pub input: Result<Value, String>,
}

/// One finished model message.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    /// The message as it goes back into the conversation.
    pub message: Value,
    pub calls: Vec<Call>,
    pub finish: Finish,
    pub usage: Usage,
}

/// Builds a reply from the provider's stream events.
pub trait Assembler: Send {
    fn apply(&mut self, event: &Value) -> Result<Option<Delta>, ChatError>;
    /// Fails with Truncated when the stream ended before the message did.
    fn finish(self: Box<Self>) -> Result<Reply, ChatError>;
}

/// One model behind one provider.
pub trait Llm: Sync {
    fn dialect(&self) -> Dialect;
    fn body(&self, setup: &Setup<'_>, messages: &[Value]) -> Value;
    /// Streams one request. The receiver yields the JSON of each event.
    fn send(&self, body: Value) -> mpsc::Receiver<Result<Value, ChatError>>;
    fn assembler(&self) -> Box<dyn Assembler>;
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
    Cut,
    Refused,
    TooManySteps,
    /// The model stopped without a word or a tool call. Small local models do this when they
    /// spend the answer on thinking.
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnEnd {
    pub stop: Stop,
    pub usage: Usage,
    pub refusal: Option<String>,
}

pub struct Setup<'a> {
    pub system: &'a str,
    pub tools: &'a [ToolDef],
}

// A typical question takes one to four tool rounds. This only stops a model that loops.
const MAX_REQUESTS: usize = 20;
// Wiki pages are cut at 12k characters by the server already. This is for anything else.
const MAX_TOOL_TEXT: usize = 40_000;

impl Dialect {
    fn tool_results(self, results: Vec<(String, ToolOutput)>) -> Vec<Value> {
        match self {
            // All results of one assistant message go back in one user message.
            Dialect::Anthropic => vec![json!({
                "role": "user",
                "content": results
                    .into_iter()
                    .map(|(id, out)| json!({
                        "type": "tool_result",
                        "tool_use_id": id,
                        "content": cut(out.text),
                        "is_error": out.is_error,
                    }))
                    .collect::<Vec<_>>(),
            })],
            Dialect::OpenAi => results
                .into_iter()
                .map(|(id, out)| {
                    let text = cut(out.text);
                    json!({
                        "role": "tool",
                        "tool_call_id": id,
                        "content": if out.is_error { format!("Error: {text}") } else { text },
                    })
                })
                .collect(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Conversation {
    messages: Vec<Value>,
    dialect: Option<Dialect>,
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
    /// it as it was and the next question goes on from the last answer. A provider with another
    /// message format starts the conversation over.
    pub async fn ask<L: Llm + ?Sized, T: Tools>(
        &mut self,
        setup: &Setup<'_>,
        llm: &L,
        tools: &T,
        question: &str,
        mut on: impl FnMut(ChatEvent) + Send,
    ) -> Result<TurnEnd, ChatError> {
        let dialect = llm.dialect();
        let mut messages = if self.dialect == Some(dialect) {
            self.messages.clone()
        } else {
            Vec::new()
        };
        messages.push(json!({ "role": "user", "content": question }));
        let mut usage = Usage::default();
        let mut wrote_text = false;
        for _ in 0..MAX_REQUESTS {
            let mut rx = llm.send(llm.body(setup, &messages));
            let mut assembler = llm.assembler();
            while let Some(event) = rx.recv().await {
                match assembler.apply(&event?)? {
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
            let reply = assembler.finish()?;
            usage.add(reply.usage);
            let end = |stop| TurnEnd {
                stop,
                usage,
                refusal: None,
            };
            match reply.finish {
                Finish::Refused(category) => {
                    return Ok(TurnEnd {
                        refusal: category,
                        ..end(Stop::Refused)
                    });
                }
                Finish::Cut => {
                    // A cut tool call is never run, and a turn ending on one can't be kept.
                    if reply.calls.is_empty() {
                        messages.push(reply.message);
                        self.keep(messages, dialect);
                    }
                    return Ok(end(Stop::Cut));
                }
                Finish::Pause => {
                    messages.push(reply.message);
                    continue;
                }
                Finish::ToolUse if !reply.calls.is_empty() => {}
                Finish::ToolUse | Finish::End if !wrote_text => return Ok(end(Stop::Empty)),
                Finish::ToolUse | Finish::End => {
                    messages.push(reply.message);
                    self.keep(messages, dialect);
                    return Ok(end(Stop::Done));
                }
            }
            if wrote_text {
                on(ChatEvent::Paragraph);
                wrote_text = false;
            }
            let mut results = Vec::with_capacity(reply.calls.len());
            for call in &reply.calls {
                let output = match &call.input {
                    Err(raw) => ToolOutput {
                        text: json!({ "INVALID_JSON": raw }).to_string(),
                        is_error: true,
                    },
                    Ok(input) => tools.call(&call.name, input.clone()).await,
                };
                results.push((call.id.clone(), output));
            }
            messages.push(reply.message);
            messages.extend(dialect.tool_results(results));
        }
        Ok(TurnEnd {
            stop: Stop::TooManySteps,
            usage,
            refusal: None,
        })
    }

    fn keep(&mut self, messages: Vec<Value>, dialect: Dialect) {
        self.messages = messages;
        self.dialect = Some(dialect);
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
    use crate::config::{Effort, Model};
    use crate::{anthropic, openai};

    /// Replays canned answers, one per request, and keeps the bodies it was sent.
    struct Script {
        dialect: Dialect,
        answers: Mutex<Vec<Vec<Value>>>,
        bodies: Mutex<Vec<Value>>,
    }

    impl Script {
        fn new(dialect: Dialect, answers: Vec<Vec<Value>>) -> Self {
            Self {
                dialect,
                answers: Mutex::new(answers.into_iter().rev().collect()),
                bodies: Mutex::new(Vec::new()),
            }
        }
    }

    impl Llm for Script {
        fn dialect(&self) -> Dialect {
            self.dialect
        }

        fn body(&self, setup: &Setup<'_>, messages: &[Value]) -> Value {
            match self.dialect {
                Dialect::Anthropic => {
                    anthropic::body(Model::Sonnet, Effort::Medium, setup, messages)
                }
                Dialect::OpenAi => openai::body("m", None, false, setup, messages),
            }
        }

        fn send(&self, body: Value) -> mpsc::Receiver<Result<Value, ChatError>> {
            self.bodies.lock().unwrap().push(body);
            let events = self.answers.lock().unwrap().pop().expect("no more answers");
            let (tx, rx) = mpsc::channel(events.len().max(1));
            for e in events {
                tx.try_send(Ok(e)).unwrap();
            }
            rx
        }

        fn assembler(&self) -> Box<dyn Assembler> {
            match self.dialect {
                Dialect::Anthropic => Box::new(anthropic::Assembler::new(Model::Sonnet)),
                Dialect::OpenAi => Box::new(openai::Assembler::default()),
            }
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

    fn echo() -> Echo {
        Echo(Mutex::new(Vec::new()))
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

    fn defs() -> Vec<ToolDef> {
        vec![ToolDef {
            name: "player_status".into(),
            description: String::new(),
            schema: json!({"type": "object"}),
        }]
    }

    #[tokio::test]
    async fn runs_tools_until_the_answer() {
        let llm = Script::new(
            Dialect::Anthropic,
            vec![
                tool_answer("player_status", "{}"),
                text_answer("Take Herbalism next.", "end_turn"),
            ],
        );
        let tools = echo();
        let defs = defs();
        let s = Setup {
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
        assert!(end.usage.cost.is_some());
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
    async fn openai_tool_results_are_messages_of_their_own() {
        let call = vec![
            json!({"choices":[{"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"c1","type":"function","function":{"name":"player_status","arguments":""}}]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{}"}}]},"finish_reason":"tool_calls"}]}),
            json!({"done": true}),
        ];
        let answer = vec![
            json!({"choices":[{"delta":{"content":"Herbalism."}}]}),
            json!({"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":100,"completion_tokens":5,"cost":0.0004}}),
            json!({"done": true}),
        ];
        let llm = Script::new(Dialect::OpenAi, vec![call, answer]);
        let tools = echo();
        let defs = defs();
        let s = Setup {
            system: "sys",
            tools: &defs,
        };
        let mut conv = Conversation::default();
        let end = conv.ask(&s, &llm, &tools, "q", |_| {}).await.unwrap();
        assert_eq!(end.stop, Stop::Done);
        assert_eq!(end.usage.cost, Some(0.0004));
        let roles: Vec<&str> = conv
            .messages()
            .iter()
            .map(|m| m["role"].as_str().unwrap())
            .collect();
        assert_eq!(roles, ["user", "assistant", "tool", "assistant"]);
        assert_eq!(conv.messages()[2]["tool_call_id"], "c1");
        let bodies = llm.bodies.lock().unwrap();
        assert_eq!(bodies[0]["messages"][0]["role"], "system");
    }

    #[tokio::test]
    async fn another_dialect_starts_over() {
        let tools = echo();
        let defs = defs();
        let s = Setup {
            system: "sys",
            tools: &defs,
        };
        let mut conv = Conversation::default();
        let claude = Script::new(Dialect::Anthropic, vec![text_answer("Hi.", "end_turn")]);
        conv.ask(&s, &claude, &tools, "hello", |_| {})
            .await
            .unwrap();
        let other = Script::new(
            Dialect::OpenAi,
            vec![vec![
                json!({"choices":[{"delta":{"content":"Hi."},"finish_reason":"stop"}]}),
                json!({"done": true}),
            ]],
        );
        conv.ask(&s, &other, &tools, "again", |_| {}).await.unwrap();
        assert_eq!(conv.messages().len(), 2);
        assert_eq!(conv.messages()[0]["content"], "again");
    }

    #[tokio::test]
    async fn broken_tool_input_goes_back_as_an_error() {
        let llm = Script::new(
            Dialect::Anthropic,
            vec![
                tool_answer("what_if", "{\"changes\": [\"a"),
                text_answer("Sorry.", "end_turn"),
            ],
        );
        let tools = echo();
        let defs = defs();
        let s = Setup {
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
        let llm = Script::new(
            Dialect::Anthropic,
            vec![text_answer("Hi.", "end_turn"), refused],
        );
        let tools = echo();
        let defs = defs();
        let s = Setup {
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
        let llm = Script::new(Dialect::Anthropic, vec![broken]);
        let tools = echo();
        let defs = defs();
        let s = Setup {
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
        let llm = Script::new(Dialect::Anthropic, vec![cut_call]);
        let tools = echo();
        let defs = defs();
        let s = Setup {
            system: "sys",
            tools: &defs,
        };
        let mut conv = Conversation::default();
        let end = conv.ask(&s, &llm, &tools, "q", |_| {}).await.unwrap();
        assert_eq!(end.stop, Stop::Cut);
        assert!(tools.0.lock().unwrap().is_empty());
        assert!(conv.is_empty());
    }

    #[tokio::test]
    async fn an_empty_answer_is_not_kept() {
        let llm = Script::new(
            Dialect::OpenAi,
            vec![vec![
                json!({"choices":[{"delta":{"reasoning":"Thinking only."}}]}),
                json!({"choices":[{"delta":{},"finish_reason":"stop"}]}),
                json!({"done": true}),
            ]],
        );
        let tools = echo();
        let defs = defs();
        let s = Setup {
            system: "sys",
            tools: &defs,
        };
        let mut conv = Conversation::default();
        let end = conv.ask(&s, &llm, &tools, "q", |_| {}).await.unwrap();
        assert_eq!(end.stop, Stop::Empty);
        assert!(conv.is_empty());
    }

    #[test]
    fn long_tool_output_is_cut_on_a_char_boundary() {
        let out = cut("й".repeat(MAX_TOOL_TEXT));
        assert!(out.ends_with("too long]"));
        assert!(out.len() < MAX_TOOL_TEXT + 64);
    }
}
