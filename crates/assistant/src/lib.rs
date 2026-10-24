//! The build assistant's chat in the overlay. Claude is called with the player's own API key and
//! uses the tools of exile-companion-mcp, the server the player's own agent would start.

mod config;
mod http;
mod mcp;
mod request;
mod stream;
mod turn;

pub use config::{Config, Effort, Model, Usage};
pub use http::Anthropic;
pub use mcp::Server;
pub use turn::{ChatEvent, Conversation, Llm, Setup, Stop, ToolOutput, Tools, TurnEnd};

/// Goes after the server's own instructions. Those say how to use the tools, this says how the
/// answer is read.
pub const CHAT_INSTRUCTIONS: &str = "You answer in Exile Companion's chat window, which sits over the game. \
The player reads it between fights or in town, in short glances.

The first line is the answer, what to do or the number they asked for. Then a blank line and the details in a few short lines. \
Stay under about 120 words unless the player asks for more. \
Write plain text without Markdown headings, bold, tables or code blocks. A list is lines that start with \"- \". \
Name passives, gems and items as the game does. \
Don't announce tool calls, the window shows which tool runs. \
Answer in the player's language.";

pub fn system_prompt(server_instructions: &str) -> String {
    format!("{server_instructions}\n\n{CHAT_INSTRUCTIONS}")
}

#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    #[error("{0}")]
    Network(String),
    #[error("{kind} ({status}): {message}")]
    Api {
        /// 0 for an error event inside the stream.
        status: u16,
        kind: String,
        message: String,
    },
    #[error("the answer broke off")]
    Truncated,
    #[error("{0}")]
    Tools(String),
}

impl ChatError {
    /// What the chat window says, with what the player can do about it.
    pub fn player_message(&self) -> String {
        match self {
            ChatError::Network(_) => {
                "Couldn't reach Anthropic. Check the connection and ask again.".into()
            }
            ChatError::Truncated => "The answer broke off. Ask again.".into(),
            ChatError::Tools(e) => format!("The assistant's tools didn't start. {e}"),
            ChatError::Api {
                status,
                kind,
                message,
                ..
            } => match (kind.as_str(), *status) {
                ("authentication_error", _) | (_, 401) => {
                    "Anthropic refused the API key. Check it in the settings.".into()
                }
                ("permission_error", _) | (_, 403) => {
                    format!("This API key can't use the model. {message}")
                }
                ("rate_limit_error", _) | (_, 429) => {
                    "Anthropic's rate limit for this key is reached. Wait a minute and ask again."
                        .into()
                }
                ("overloaded_error", _) | (_, 529) => {
                    "Anthropic is overloaded right now. Ask again in a minute.".into()
                }
                _ if message.contains("credit balance") => {
                    "The API key's account is out of credit. Top it up in the Anthropic Console."
                        .into()
                }
                _ => format!("Anthropic returned an error. {message}"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api(status: u16, kind: &str, message: &str) -> ChatError {
        ChatError::Api {
            status,
            kind: kind.into(),
            message: message.into(),
        }
    }

    #[test]
    fn errors_say_what_to_do() {
        assert!(api(401, "authentication_error", "invalid x-api-key")
            .player_message()
            .contains("settings"));
        assert!(api(0, "overloaded_error", "Overloaded")
            .player_message()
            .contains("overloaded"));
        assert!(api(
            400,
            "invalid_request_error",
            "Your credit balance is too low to access the Anthropic API."
        )
        .player_message()
        .contains("out of credit"));
    }

    #[test]
    fn chat_rules_come_after_the_servers() {
        let p = system_prompt("Use player_status first.");
        assert!(p.starts_with("Use player_status first."));
        assert!(p.ends_with("Answer in the player's language."));
    }
}
