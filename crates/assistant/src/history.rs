//! Past chats, one JSON file each, so the player can read them again and go on with one.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{Model, Provider};
use crate::turn::{Conversation, Dialect};

const VERSION: u32 = 1;
/// Both providers keep the prompt cache for an hour after its last use, as we ask them to.
pub const CACHE_SECS: u64 = 60 * 60;
const TITLE_CHARS: usize = 80;

/// One question and how it ended, as the chat window shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedExchange {
    pub question: String,
    pub answer: String,
    /// The same names as the chat's turn ends: "done", "cut", "error" and so on.
    pub stop: String,
    pub message: Option<String>,
    pub dollars: Option<f64>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedChat {
    pub version: u32,
    pub id: String,
    pub title: String,
    /// Unix seconds.
    pub created: u64,
    /// When the last question went out, which is when the provider's cache was last used.
    pub updated: u64,
    pub provider: Provider,
    pub model: String,
    pub dialect: Option<Dialect>,
    /// The conversation in the provider's own format, to go on with.
    pub messages: Vec<Value>,
    pub exchanges: Vec<SavedExchange>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub updated: u64,
    pub model: String,
    pub questions: usize,
    /// What the answers cost, when every provider said.
    pub dollars: Option<f64>,
}

/// What going on with a saved chat would cost on top of a new question.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Warmth {
    /// The provider still has the conversation cached.
    Warm,
    /// The cache is gone, so the next question sends the whole conversation at full price once.
    Cold {
        tokens: u64,
        /// Known only for Claude through Anthropic, where we have the prices.
        dollars: Option<f64>,
    },
}

impl SavedChat {
    pub fn new(id: String, now: u64, provider: Provider, model: String) -> Self {
        Self {
            version: VERSION,
            id,
            title: String::new(),
            created: now,
            updated: now,
            provider,
            model,
            dialect: None,
            messages: Vec::new(),
            exchanges: Vec::new(),
        }
    }

    /// Adds a finished question and takes the conversation as it is after it.
    pub fn record(&mut self, exchange: SavedExchange, conversation: &Conversation, now: u64) {
        if self.title.is_empty() {
            self.title = title(&exchange.question);
        }
        if let Some(m) = &exchange.model {
            self.model = m.clone();
        }
        self.exchanges.push(exchange);
        self.messages = conversation.messages().to_vec();
        self.dialect = conversation.dialect();
        self.updated = now;
    }

    pub fn conversation(&self) -> Conversation {
        Conversation::restore(self.messages.clone(), self.dialect)
    }

    pub fn dollars(&self) -> Option<f64> {
        self.exchanges.iter().map(|e| e.dollars).sum()
    }

    pub fn summary(&self) -> ChatSummary {
        ChatSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            updated: self.updated,
            model: self.model.clone(),
            questions: self.exchanges.len(),
            dollars: self.dollars(),
        }
    }

    /// Whether a provider of this dialect can go on with the conversation. Anthropic and the
    /// OpenAI-style servers keep messages in different shapes.
    pub fn continues_with(&self, dialect: Dialect) -> bool {
        self.dialect.is_none_or(|d| d == dialect)
    }

    /// A token is about four characters of the JSON the provider gets.
    pub fn tokens(&self) -> u64 {
        let chars: usize = self.messages.iter().map(|m| m.to_string().len()).sum();
        (chars / 4) as u64
    }

    /// `model` is the Claude model the chat would go on with through Anthropic, if it would.
    pub fn warmth(&self, now: u64, model: Option<Model>) -> Warmth {
        if self.messages.is_empty() || now.saturating_sub(self.updated) < CACHE_SECS {
            return Warmth::Warm;
        }
        let tokens = self.tokens();
        Warmth::Cold {
            tokens,
            dollars: model.map(|m| tokens as f64 * m.cache_write_price() / 1_000_000.0),
        }
    }
}

fn title(question: &str) -> String {
    let line = question
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    let line = line.trim();
    if line.chars().count() <= TITLE_CHARS {
        return line.to_string();
    }
    let cut: String = line.chars().take(TITLE_CHARS).collect();
    format!("{}…", cut.trim_end())
}

/// A folder of saved chats.
pub struct ChatStore {
    dir: PathBuf,
}

impl ChatStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, id: &str) -> Option<PathBuf> {
        let safe = !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        safe.then(|| self.dir.join(format!("{id}.json")))
    }

    /// Writes next to the file and renames, so a crash never leaves half a chat.
    pub fn save(&self, chat: &SavedChat) -> std::io::Result<()> {
        let path = self.path(&chat.id).ok_or_else(|| bad_id(&chat.id))?;
        std::fs::create_dir_all(&self.dir)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(chat)?)?;
        std::fs::rename(&tmp, &path)
    }

    pub fn load(&self, id: &str) -> std::io::Result<SavedChat> {
        let path = self.path(id).ok_or_else(|| bad_id(id))?;
        read(&path)
    }

    pub fn delete(&self, id: &str) -> std::io::Result<()> {
        let path = self.path(id).ok_or_else(|| bad_id(id))?;
        std::fs::remove_file(path)
    }

    /// Newest first. Files that don't read as a chat are left out.
    pub fn list(&self) -> Vec<ChatSummary> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut chats: Vec<ChatSummary> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter_map(|p| read(&p).ok())
            .map(|c| c.summary())
            .collect();
        chats.sort_by(|a, b| b.updated.cmp(&a.updated).then(b.id.cmp(&a.id)));
        chats
    }
}

fn read(path: &Path) -> std::io::Result<SavedChat> {
    let chat: SavedChat = serde_json::from_slice(&std::fs::read(path)?)?;
    if chat.version > VERSION {
        return Err(std::io::Error::other(format!(
            "{} is from a newer Exile Companion",
            path.display()
        )));
    }
    Ok(chat)
}

fn bad_id(id: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("not a chat id: {id}"),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn exchange(question: &str, dollars: Option<f64>) -> SavedExchange {
        SavedExchange {
            question: question.into(),
            answer: "Help Alira.".into(),
            stop: "done".into(),
            message: None,
            dollars,
            model: Some("anthropic/claude-sonnet-5.5".into()),
        }
    }

    fn chat_with(messages: Vec<Value>, at: u64) -> SavedChat {
        let mut chat = SavedChat::new("chat-1".into(), at, Provider::OpenRouter, String::new());
        let conversation = Conversation::restore(messages, Some(Dialect::OpenAi));
        chat.record(
            exchange("Which bandit?\nThe guide is RF", Some(0.02)),
            &conversation,
            at,
        );
        chat
    }

    #[test]
    fn records_a_question_with_the_conversation_after_it() {
        let msgs = vec![json!({"role": "user", "content": "Which bandit?"})];
        let chat = chat_with(msgs.clone(), 100);
        assert_eq!(chat.title, "Which bandit?");
        assert_eq!(chat.model, "anthropic/claude-sonnet-5.5");
        assert_eq!(chat.messages, msgs);
        assert_eq!(chat.dialect, Some(Dialect::OpenAi));
        assert_eq!(chat.conversation().messages(), msgs.as_slice());
        assert!(chat.continues_with(Dialect::OpenAi));
        assert!(!chat.continues_with(Dialect::Anthropic));
    }

    #[test]
    fn long_titles_are_cut() {
        let t = title(&"word ".repeat(40));
        assert_eq!(t.chars().count(), TITLE_CHARS);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn cost_is_known_only_when_every_answer_said() {
        let mut chat = chat_with(Vec::new(), 0);
        assert_eq!(chat.dollars(), Some(0.02));
        chat.exchanges.push(exchange("More?", None));
        assert_eq!(chat.dollars(), None);
    }

    #[test]
    fn the_cache_goes_cold_after_an_hour() {
        let msgs = vec![json!({"role": "user", "content": "x".repeat(4000)})];
        let chat = chat_with(msgs, 1_000);
        assert_eq!(chat.warmth(1_000 + CACHE_SECS - 1, None), Warmth::Warm);
        let Warmth::Cold { tokens, dollars } = chat.warmth(1_000 + CACHE_SECS, Some(Model::Sonnet))
        else {
            panic!("still warm");
        };
        assert!((1_000..1_100).contains(&tokens), "{tokens}");
        // Sonnet writes the cache at $4 per million tokens.
        assert!((dollars.unwrap() - tokens as f64 * 4e-6).abs() < 1e-9);
        assert_eq!(
            chat.warmth(1_000 + CACHE_SECS, None),
            Warmth::Cold {
                tokens,
                dollars: None
            }
        );
    }

    #[test]
    fn store_saves_lists_newest_first_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let store = ChatStore::new(dir.path().join("chats"));
        assert!(store.list().is_empty());
        let old = chat_with(Vec::new(), 10);
        let mut new = chat_with(Vec::new(), 20);
        new.id = "chat-2".into();
        store.save(&old).unwrap();
        store.save(&new).unwrap();
        std::fs::write(dir.path().join("chats/broken.json"), "{").unwrap();
        let ids: Vec<String> = store.list().into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["chat-2", "chat-1"]);
        assert_eq!(store.load("chat-1").unwrap(), old);
        store.delete("chat-1").unwrap();
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn store_refuses_ids_that_leave_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let store = ChatStore::new(dir.path());
        assert!(store.load("../settings").is_err());
        assert!(store.delete("").is_err());
    }
}
