//! cargo run -p exile-assistant --example ask -- [options] "question" ["follow-up" ...]
//!
//! Asks questions in one conversation through the chat's own loop and the real MCP tools, and
//! prints the tool calls, the answer, the time and what it cost. For comparing models.
//!
//!   --mcp <path>          exile-companion-mcp (default target/release/exile-companion-mcp)
//!   --pob <dir>           Path of Building folder for the tools
//!   --provider <p>        anthropic, openrouter or custom (default custom)
//!   --model <id>          model id, or for anthropic: haiku, sonnet, opus
//!   --url <base>          custom server, e.g. http://127.0.0.1:11434/v1
//!   --key-env <NAME>      environment variable that holds the API key
//!   --effort <e>          low, medium or high (default low, and none for a custom server)
//!   --raw                 print every stream event to stderr

use std::time::Instant;

use exile_assistant::{
    Anthropic, ChatEvent, Conversation, Effort, Llm, Model, OpenAi, Server, Setup,
};
use serde_json::Value;

const USER_AGENT: &str = "ExileCompanion-ask/0 (https://github.com/muirch/exile-companion)";

fn arg(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.remove(i);
    (i < args.len()).then(|| args.remove(i))
}

fn short(v: &Value, n: usize) -> String {
    let s = v.to_string();
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s
    }
}

/// The tool calls and results the turn added, in either message format.
fn print_calls(messages: &[Value]) {
    for m in messages {
        if let Some(blocks) = m["content"].as_array() {
            for b in blocks {
                match b["type"].as_str() {
                    Some("tool_use") => println!(
                        "  call {} {}",
                        b["name"].as_str().unwrap_or(""),
                        short(&b["input"], 160)
                    ),
                    Some("tool_result") => println!(
                        "    -> {} chars{}",
                        b["content"].as_str().map_or(0, str::len),
                        if b["is_error"] == true { ", error" } else { "" }
                    ),
                    _ => {}
                }
            }
        }
        for c in m["tool_calls"].as_array().into_iter().flatten() {
            println!(
                "  call {} {}",
                c["function"]["name"].as_str().unwrap_or(""),
                short(&c["function"]["arguments"], 160)
            );
        }
        if m["role"] == "tool" {
            println!("    -> {} chars", m["content"].as_str().map_or(0, str::len));
        }
    }
}

/// --raw prints every stream event to stderr, to see what a model sent when an answer is odd.
struct Raw(Box<dyn Llm + Send>);

impl Llm for Raw {
    fn dialect(&self) -> exile_assistant::Dialect {
        self.0.dialect()
    }

    fn body(&self, setup: &Setup<'_>, messages: &[Value]) -> Value {
        self.0.body(setup, messages)
    }

    fn send(
        &self,
        body: Value,
    ) -> tokio::sync::mpsc::Receiver<Result<Value, exile_assistant::ChatError>> {
        let mut inner = self.0.send(body);
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        tokio::spawn(async move {
            while let Some(e) = inner.recv().await {
                if let Ok(v) = &e {
                    eprintln!("RAW {v}");
                }
                if tx.send(e).await.is_err() {
                    break;
                }
            }
        });
        rx
    }

    fn assembler(&self) -> Box<dyn exile_assistant::Assembler> {
        self.0.assembler()
    }
}

#[tokio::main]
async fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mcp =
        arg(&mut args, "--mcp").unwrap_or_else(|| "target/release/exile-companion-mcp".into());
    let pob = arg(&mut args, "--pob");
    let raw = match args.iter().position(|a| a == "--raw") {
        Some(i) => {
            args.remove(i);
            true
        }
        None => false,
    };
    let provider = arg(&mut args, "--provider").unwrap_or_else(|| "custom".into());
    let model = arg(&mut args, "--model").unwrap_or_default();
    let url = arg(&mut args, "--url").unwrap_or_else(|| "http://127.0.0.1:11434/v1".into());
    let key = arg(&mut args, "--key-env").and_then(|name| std::env::var(name).ok());
    let effort_arg = arg(&mut args, "--effort");
    let effort = match effort_arg.as_deref() {
        Some("medium") => Effort::Medium,
        Some("high") => Effort::High,
        _ => Effort::Low,
    };
    // A custom server gets an effort only when asked for one, as the app does for Ollama.
    let custom_effort = effort_arg.is_some().then_some(effort);
    if args.is_empty() {
        eprintln!("give at least one question");
        std::process::exit(2);
    }

    let server_args: Vec<String> = pob.map(|p| vec!["--pob".into(), p]).unwrap_or_default();
    let server = Server::start(mcp.as_ref(), &server_args)
        .await
        .expect("MCP server");
    let llm: Box<dyn Llm + Send> = match provider.as_str() {
        "anthropic" => {
            let m = match model.as_str() {
                "haiku" => Model::Haiku,
                "opus" => Model::Opus,
                _ => Model::Sonnet,
            };
            Box::new(Anthropic::new(
                key.expect("--key-env"),
                m,
                effort,
                USER_AGENT,
            ))
        }
        "openrouter" => Box::new(OpenAi::openrouter(
            key.expect("--key-env"),
            model.clone(),
            effort,
            USER_AGENT,
        )),
        _ => Box::new(OpenAi::custom(
            &url,
            key,
            model.clone(),
            custom_effort,
            USER_AGENT,
        )),
    };
    let llm: Box<dyn Llm + Send> = if raw { Box::new(Raw(llm)) } else { llm };
    let system = exile_assistant::system_prompt(&server.instructions);
    let setup = Setup {
        system: &system,
        tools: &server.tools,
    };
    let mut conv = Conversation::default();

    for question in &args {
        println!("\n=== {question}");
        let before = conv.messages().len();
        let started = Instant::now();
        let mut answer = String::new();
        let result = conv
            .ask(&setup, llm.as_ref(), &server, question, |e| match e {
                ChatEvent::Text { text } => answer.push_str(&text),
                ChatEvent::Paragraph => answer.push_str("\n\n"),
                ChatEvent::Note { .. } | ChatEvent::Tool { .. } => {}
            })
            .await;
        let secs = started.elapsed().as_secs_f64();
        print_calls(&conv.messages()[before.min(conv.messages().len())..]);
        println!("--- answer\n{}", answer.trim());
        match result {
            Ok(end) => println!(
                "--- {:?} in {secs:.1}s, {} in / {} out / {} cached, {}",
                end.stop,
                end.usage.input_tokens,
                end.usage.output_tokens,
                end.usage.cache_read_tokens,
                end.usage
                    .cost
                    .map_or("cost unknown".into(), |c| format!("${c:.4}"))
            ),
            Err(e) => println!("--- error after {secs:.1}s: {e}"),
        }
    }
}
