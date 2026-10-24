//! Ollama keeps a model in video memory for a while after each request. The game needs that
//! memory, so the chat loads the model when its window opens and unloads it soon after it
//! closes, through Ollama's own API next to the OpenAI-compatible one.

use std::time::Duration;

use serde_json::json;

use crate::http;

/// Ollama's own API root for a base URL given for its OpenAI-compatible API
/// ("http://127.0.0.1:11434/v1" gives "http://127.0.0.1:11434").
pub fn api_root(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    let base = base.strip_suffix("/chat/completions").unwrap_or(base);
    let base = base.strip_suffix("/v1").unwrap_or(base);
    base.trim_end_matches('/').to_string()
}

pub struct Ollama {
    agent: ureq::Agent,
    root: String,
    model: String,
}

impl Ollama {
    /// None when nothing at that address answers like Ollama, such as LM Studio or a cloud API.
    pub fn detect(base_url: &str, model: &str, user_agent: &str) -> Option<Ollama> {
        let root = api_root(base_url);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(2)))
            .http_status_as_error(false)
            .user_agent(user_agent)
            .build()
            .into();
        let mut r = agent.get(format!("{root}/api/version")).call().ok()?;
        let body = r.body_mut().read_to_string().ok()?;
        let v: serde_json::Value = serde_json::from_str(&body).ok()?;
        v["version"].as_str()?;
        Some(Ollama {
            agent: http::agent(user_agent),
            root,
            model: model.trim().to_string(),
        })
    }

    /// A request without a prompt only loads the model. Blocks until it is in memory.
    pub fn load(&self, keep: Duration) -> Result<(), String> {
        self.generate(json!({ "model": self.model, "keep_alive": format!("{}s", keep.as_secs()) }))
    }

    pub fn unload(&self) -> Result<(), String> {
        self.generate(json!({ "model": self.model, "keep_alive": 0 }))
    }

    fn generate(&self, body: serde_json::Value) -> Result<(), String> {
        let mut r = self
            .agent
            .post(format!("{}/api/generate", self.root))
            .header("content-type", "application/json")
            .send(body.to_string().as_bytes())
            .map_err(|e| e.to_string())?;
        if r.status().is_success() {
            Ok(())
        } else {
            Err(r.body_mut().read_to_string().unwrap_or_default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::api_root;

    #[test]
    fn finds_the_native_api_next_to_the_openai_one() {
        assert_eq!(
            api_root("http://127.0.0.1:11434/v1"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(
            api_root("http://127.0.0.1:11434/v1/"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(
            api_root("http://192.168.1.20:11434/v1/chat/completions"),
            "http://192.168.1.20:11434"
        );
        assert_eq!(api_root("http://localhost:11434"), "http://localhost:11434");
    }
}
