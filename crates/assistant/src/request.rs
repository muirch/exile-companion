use serde_json::{json, Value};

use crate::config::Config;

pub const API_URL: &str = "https://api.anthropic.com/v1/messages";
pub const API_VERSION: &str = "2026-10-21";
/// Progress notes between tool calls come back readable (thinking display "updates"), and a
/// policy decline is retried on the model Anthropic picks for its category.
pub const BETAS: &str = "thinking-display-updates-2026-08-18,server-side-fallback-2026-07-01";

// Room for thinking plus the answer. Streaming keeps a large value from timing out.
const MAX_TOKENS: u32 = 64_000;

/// One tool as the Messages API takes it.
pub fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "input_schema": input_schema,
        "eager_input_streaming": true,
    })
}

/// The system prompt and the tools come first and don't change within a conversation, so they
/// stay in the prompt cache. The top-level breakpoint moves along with the messages.
pub fn body(config: Config, system: &str, tools: &[Value], messages: &[Value]) -> Value {
    json!({
        "model": config.model.id(),
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "system": [{ "type": "text", "text": system, "cache_control": { "type": "ephemeral" } }],
        "tools": tools,
        "messages": messages,
        "thinking": { "type": "adaptive", "display": "updates" },
        "output_config": { "effort": config.effort.as_str() },
        "cache_control": { "type": "ephemeral" },
        "fallbacks": "default",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Effort, Model};

    #[test]
    fn body_has_no_sampling_params_or_forced_tools() {
        let b = body(
            Config {
                model: Model::Opus,
                effort: Effort::High,
            },
            "sys",
            &[tool("player_status", "Where", json!({"type": "object"}))],
            &[json!({"role": "user", "content": "hi"})],
        );
        assert_eq!(b["model"], "claude-opus-5-5");
        assert_eq!(b["output_config"]["effort"], "high");
        assert_eq!(b["tools"][0]["eager_input_streaming"], true);
        for banned in ["temperature", "top_p", "top_k", "tool_choice"] {
            assert!(b.get(banned).is_none(), "{banned}");
        }
    }
}
