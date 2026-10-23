//! GET requests to the build assistant's outside sources (the wiki, guide sites), the way a
//! polite tool makes them: a User-Agent that says who we are, requests spaced out, and answers
//! cached on disk so the same question doesn't go out twice.

mod cache;

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct FetchError(pub String);

pub struct Fetcher {
    agent: ureq::Agent,
    cache: Option<PathBuf>,
    ttl: Duration,
    spacing: Duration,
    last: Mutex<Option<Instant>>,
}

impl Fetcher {
    /// `cache` is a folder for raw answers kept for `ttl`. None asks every time.
    pub fn new(user_agent: &str, cache: Option<PathBuf>, ttl: Duration, spacing: Duration) -> Self {
        // The default roots are a bundled Mozilla list that schannel rejects. Use the OS store.
        let tls = ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::NativeTls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        let agent = ureq::Agent::config_builder()
            .tls_config(tls)
            .timeout_global(Some(Duration::from_secs(20)))
            .user_agent(user_agent)
            .build()
            .into();
        Self {
            agent,
            cache,
            ttl,
            spacing,
            last: Mutex::new(None),
        }
    }

    pub fn get(&self, url: &str, query: &[(&str, &str)]) -> Result<String, FetchError> {
        let key = cache::key(url, query);
        if let Some(body) = self
            .cache
            .as_deref()
            .and_then(|dir| cache::read(dir, &key, self.ttl))
        {
            return Ok(body);
        }
        let mut req = self.agent.get(url);
        for (k, v) in query {
            req = req.query(*k, *v);
        }
        self.wait_turn();
        let body = req
            .call()
            .and_then(|mut r| r.body_mut().read_to_string())
            .map_err(|e| FetchError(e.to_string()))?;
        if let Some(dir) = &self.cache {
            cache::write(dir, &key, &body);
        }
        Ok(body)
    }

    fn wait_turn(&self) {
        let mut last = self.last.lock().unwrap();
        if let Some(t) = *last {
            let since = t.elapsed();
            if since < self.spacing {
                std::thread::sleep(self.spacing - since);
            }
        }
        *last = Some(Instant::now());
    }
}
