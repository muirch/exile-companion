//! The build assistant's chat in the overlay. A model is called with the player's own key, from
//! Anthropic, OpenRouter or a server the player names, and uses the tools of exile-companion-mcp,
//! the server the player's own agent would start.

mod anthropic;
mod config;
mod http;
mod mcp;
mod openai;
mod stream;
mod turn;

pub use anthropic::Anthropic;
pub use config::{Config, Effort, Model, Provider, Usage};
pub use mcp::Server;
pub use openai::OpenAi;
pub use turn::{
    ChatEvent, Conversation, Dialect, Llm, Setup, Stop, ToolDef, ToolOutput, Tools, TurnEnd,
};

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
    pub fn player_message(&self, provider: Provider) -> String {
        let who = provider.name();
        match self {
            ChatError::Network(_) => match provider {
                Provider::Custom => {
                    "Couldn't reach the model's server. Check the address in the settings and that the server runs.".into()
                }
                _ => format!("Couldn't reach {who}. Check the connection and ask again."),
            },
            ChatError::Truncated => "The answer broke off. Ask again.".into(),
            ChatError::Tools(e) => format!("The assistant's tools didn't start. {e}"),
            ChatError::Api {
                status,
                kind,
                message,
            } => match (kind.as_str(), *status) {
                ("authentication_error", _) | (_, 401) => {
                    format!("{who} refused the API key. Check it in the settings.")
                }
                ("permission_error", _) | (_, 403) => {
                    format!("This API key can't use the model. {message}")
                }
                (_, 402) => format!("The account at {who} is out of credit."),
                _ if message.contains("credit balance") => {
                    format!("The account at {who} is out of credit.")
                }
                ("rate_limit_error", _) | (_, 429) => {
                    format!("{who} limits how often this key can ask. Wait a minute and ask again.")
                }
                ("overloaded_error", _) | (_, 529) | (_, 503) => {
                    format!("{who} is overloaded right now. Ask again in a minute.")
                }
                (_, 404) => format!("{who} doesn't know this model. Check its name in the settings. {message}"),
                _ => format!("{who} returned an error. {message}"),
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
        assert_eq!(
            api(401, "authentication_error", "invalid x-api-key")
                .player_message(Provider::Anthropic),
            "Anthropic refused the API key. Check it in the settings."
        );
        assert!(api(0, "overloaded_error", "Overloaded")
            .player_message(Provider::Anthropic)
            .contains("overloaded"));
        assert!(api(
            400,
            "invalid_request_error",
            "Your credit balance is too low to access the Anthropic API."
        )
        .player_message(Provider::Anthropic)
        .contains("out of credit"));
        assert_eq!(
            api(402, "", "Insufficient credits").player_message(Provider::OpenRouter),
            "The account at OpenRouter is out of credit."
        );
        assert!(ChatError::Network("refused".into())
            .player_message(Provider::Custom)
            .contains("address"));
    }

    #[test]
    fn chat_rules_come_after_the_servers() {
        let p = system_prompt("Use player_status first.");
        assert!(p.starts_with("Use player_status first."));
        assert!(p.ends_with("Answer in the player's language."));
    }
}
