use std::io::BufRead;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::mpsc;

use crate::request::{API_URL, API_VERSION, BETAS};
use crate::stream::SseLines;
use crate::turn::Llm;
use crate::ChatError;

/// Claude over the Messages API with the player's own key. Each request runs on its own thread,
/// and dropping the receiver ends it at the next event.
pub struct Anthropic {
    agent: ureq::Agent,
    key: String,
}

const RETRIES: u32 = 2;

impl Anthropic {
    pub fn new(key: String, user_agent: &str) -> Self {
        // The default roots are a bundled Mozilla list that schannel rejects. Use the OS store.
        let tls = ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::NativeTls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        // No overall timeout. An answer with several tool rounds streams for minutes, and the
        // API sends pings while it thinks.
        let agent = ureq::Agent::config_builder()
            .tls_config(tls)
            .timeout_connect(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
            .user_agent(user_agent)
            .build()
            .into();
        Self { agent, key }
    }
}

fn retryable(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504 | 529)
}

fn api_error(status: u16, body: &str) -> ChatError {
    let v: Value = serde_json::from_str(body).unwrap_or_default();
    ChatError::Api {
        status,
        kind: v["error"]["type"].as_str().unwrap_or("").to_string(),
        message: v["error"]["message"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| body.chars().take(300).collect()),
    }
}

impl Llm for Anthropic {
    fn send(&self, body: Value) -> mpsc::Receiver<Result<Value, ChatError>> {
        let (tx, rx) = mpsc::channel(256);
        let agent = self.agent.clone();
        let key = self.key.clone();
        std::thread::spawn(move || {
            let payload = body.to_string();
            let mut attempt = 0;
            let response = loop {
                let sent = agent
                    .post(API_URL)
                    .header("x-api-key", &key)
                    .header("anthropic-version", API_VERSION)
                    .header("anthropic-beta", BETAS)
                    .header("content-type", "application/json")
                    .send(payload.as_bytes());
                let wait = match sent {
                    Ok(r) if r.status().is_success() => break r,
                    Ok(mut r) => {
                        let status = r.status().as_u16();
                        if !retryable(status) || attempt == RETRIES {
                            let text = r.body_mut().read_to_string().unwrap_or_default();
                            let _ = tx.blocking_send(Err(api_error(status, &text)));
                            return;
                        }
                        r.headers()
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok())
                            .unwrap_or(2u64.pow(attempt + 1))
                            .min(20)
                    }
                    Err(e) => {
                        if attempt == RETRIES {
                            let _ = tx.blocking_send(Err(ChatError::Network(e.to_string())));
                            return;
                        }
                        2u64.pow(attempt + 1)
                    }
                };
                attempt += 1;
                std::thread::sleep(Duration::from_secs(wait));
            };
            let reader = std::io::BufReader::new(response.into_body().into_reader());
            let mut sse = SseLines::default();
            let mut lines = reader.lines();
            loop {
                let data = match lines.next() {
                    Some(Ok(line)) => sse.line(&line),
                    Some(Err(e)) => {
                        let _ = tx.blocking_send(Err(ChatError::Network(e.to_string())));
                        return;
                    }
                    None => match sse.finish() {
                        Some(d) => Some(d),
                        None => return,
                    },
                };
                let Some(data) = data else {
                    continue;
                };
                let event = serde_json::from_str::<Value>(&data)
                    .map_err(|e| ChatError::Network(format!("unreadable event: {e}")));
                if tx.blocking_send(event).is_err() {
                    return;
                }
            }
        });
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_api_error_body() {
        let e = api_error(
            401,
            r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
        );
        assert!(
            matches!(e, ChatError::Api { status: 401, ref kind, .. } if kind == "authentication_error")
        );
        assert!(
            matches!(api_error(502, "<html>bad gateway</html>"), ChatError::Api { ref message, .. } if message.contains("bad gateway"))
        );
    }
}
