use std::io::BufRead;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::stream::SseLines;
use crate::ChatError;

const RETRIES: u32 = 2;

pub fn agent(user_agent: &str) -> ureq::Agent {
    // The default roots are a bundled Mozilla list that schannel rejects. Use the OS store.
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    // No overall timeout. An answer with several tool rounds streams for minutes, and the
    // providers send pings while the model thinks.
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_connect(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .user_agent(user_agent)
        .build()
        .into()
}

fn retryable(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504 | 529)
}

/// Anthropic puts the kind in error.type, OpenAI-style servers in error.code.
pub fn api_error(status: u16, body: &str) -> ChatError {
    let v: Value = serde_json::from_str(body).unwrap_or_default();
    let e = &v["error"];
    ChatError::Api {
        status,
        kind: e["type"]
            .as_str()
            .or(e["code"].as_str())
            .unwrap_or("")
            .to_string(),
        message: e["message"]
            .as_str()
            .or(v["message"].as_str())
            .map(str::to_string)
            .unwrap_or_else(|| body.chars().take(300).collect()),
    }
}

/// POSTs on its own thread and streams the server-sent events back as JSON. OpenAI's closing
/// `[DONE]` comes as {"done": true}. Dropping the receiver ends the request at the next event.
pub fn post_stream(
    agent: ureq::Agent,
    url: String,
    headers: Vec<(&'static str, String)>,
    body: Value,
) -> mpsc::Receiver<Result<Value, ChatError>> {
    let (tx, rx) = mpsc::channel(256);
    std::thread::spawn(move || {
        let payload = body.to_string();
        let mut attempt = 0;
        let response = loop {
            let mut req = agent.post(&url).header("content-type", "application/json");
            for (name, value) in &headers {
                req = req.header(*name, value);
            }
            let wait = match req.send(payload.as_bytes()) {
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
            let event = if data.trim() == "[DONE]" {
                Ok(json!({ "done": true }))
            } else {
                serde_json::from_str::<Value>(&data)
                    .map_err(|e| ChatError::Network(format!("unreadable event: {e}")))
            };
            if tx.blocking_send(event).is_err() {
                return;
            }
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_error_shapes() {
        let e = api_error(
            401,
            r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
        );
        assert!(
            matches!(e, ChatError::Api { status: 401, ref kind, .. } if kind == "authentication_error")
        );
        let e = api_error(
            402,
            r#"{"error":{"code":402,"message":"Insufficient credits"}}"#,
        );
        assert!(
            matches!(e, ChatError::Api { status: 402, ref message, .. } if message == "Insufficient credits")
        );
        assert!(
            matches!(api_error(502, "<html>bad gateway</html>"), ChatError::Api { ref message, .. } if message.contains("bad gateway"))
        );
    }
}
