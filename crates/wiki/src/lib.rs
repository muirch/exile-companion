//! The Path of Exile Wiki through its MediaWiki API. The wiki asks tools to use the API, Cargo
//! in particular, instead of scraping pages, and the HTML pages sit behind a bot check anyway.
//! Its content is CC BY-NC-SA 3.0, so every result carries the page URL for attribution.

mod cache;
pub mod text;

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Map, Value};

pub use text::Section;

pub const API: &str = "https://www.poewiki.net/w/api.php";
pub const ATTRIBUTION: &str = "Path of Exile Wiki (poewiki.net), CC BY-NC-SA 3.0";

/// Space between requests. The wiki runs on donations, one tool shouldn't hammer it.
const MIN_INTERVAL: Duration = Duration::from_millis(300);
/// Pages change with each patch, a week is fresh enough between patches.
const CACHE_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Debug, thiserror::Error)]
pub enum WikiError {
    #[error("couldn't reach the wiki: {0}")]
    Http(String),
    #[error("the wiki said {code}: {info}")]
    Api { code: String, info: String },
    #[error("unexpected answer from the wiki: {0}")]
    Json(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchHit {
    pub title: String,
    pub snippet: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Page {
    pub title: String,
    pub url: String,
    pub revision: Option<u64>,
    pub text: String,
}

impl Page {
    pub fn sections(&self) -> Vec<Section> {
        text::sections(&self.text)
    }
}

/// A read-only Cargo query. Field and table names are the wiki's (Special:CargoTables).
#[derive(Debug, Clone, Default)]
pub struct CargoQuery {
    pub tables: String,
    pub fields: String,
    pub where_: Option<String>,
    pub join_on: Option<String>,
    pub group_by: Option<String>,
    pub order_by: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

pub struct Wiki {
    agent: ureq::Agent,
    api: String,
    cache: Option<PathBuf>,
    last: Mutex<Option<Instant>>,
}

impl Wiki {
    /// `cache` is a folder for raw answers. None asks the wiki every time.
    pub fn new(user_agent: &str, cache: Option<PathBuf>) -> Self {
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
            api: API.to_string(),
            cache,
            last: Mutex::new(None),
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, WikiError> {
        let limit = limit.clamp(1, 50).to_string();
        let v = self.get(&[
            ("action", "query"),
            ("list", "search"),
            ("srsearch", query),
            ("srlimit", &limit),
            ("srprop", "snippet"),
        ])?;
        parse_search(&v)
    }

    /// The page as plain text with `== Heading ==` lines, following redirects. None if missing.
    pub fn page(&self, title: &str) -> Result<Option<Page>, WikiError> {
        let v = self.get(&[
            ("action", "query"),
            ("prop", "extracts|info"),
            ("explaintext", "1"),
            ("exsectionformat", "wiki"),
            ("redirects", "1"),
            ("inprop", "url"),
            ("titles", title),
        ])?;
        parse_page(&v)
    }

    pub fn cargo(&self, q: &CargoQuery) -> Result<Vec<Map<String, Value>>, WikiError> {
        let limit = q.limit.clamp(1, 500).to_string();
        let offset = q.offset.to_string();
        let mut params = vec![
            ("action", "cargoquery"),
            ("tables", q.tables.as_str()),
            ("fields", q.fields.as_str()),
            ("limit", limit.as_str()),
            ("offset", offset.as_str()),
        ];
        for (key, value) in [
            ("where", &q.where_),
            ("join_on", &q.join_on),
            ("group_by", &q.group_by),
            ("order_by", &q.order_by),
        ] {
            if let Some(v) = value {
                params.push((key, v.as_str()));
            }
        }
        let v = self.get(&params)?;
        parse_cargo(&v)
    }

    fn get(&self, params: &[(&str, &str)]) -> Result<Value, WikiError> {
        let mut req = self.agent.get(&self.api);
        for (k, v) in params {
            req = req.query(*k, *v);
        }
        // maxlag makes the wiki refuse us first when its database replicas are behind.
        req = req
            .query("format", "json")
            .query("formatversion", "2")
            .query("maxlag", "5");
        let key = cache::key(&self.api, params);
        if let Some(body) = self
            .cache
            .as_deref()
            .and_then(|dir| cache::read(dir, &key, CACHE_TTL))
        {
            return parse_json(&body);
        }
        self.wait_turn();
        let body = req
            .call()
            .and_then(|mut r| r.body_mut().read_to_string())
            .map_err(|e| WikiError::Http(e.to_string()))?;
        let v = parse_json(&body)?;
        if let Some(dir) = &self.cache {
            cache::write(dir, &key, &body);
        }
        Ok(v)
    }

    fn wait_turn(&self) {
        let mut last = self.last.lock().unwrap();
        if let Some(t) = *last {
            let since = t.elapsed();
            if since < MIN_INTERVAL {
                std::thread::sleep(MIN_INTERVAL - since);
            }
        }
        *last = Some(Instant::now());
    }
}

fn parse_json(body: &str) -> Result<Value, WikiError> {
    let v: Value = serde_json::from_str(body).map_err(|e| WikiError::Json(e.to_string()))?;
    if let Some(err) = v.get("error") {
        return Err(WikiError::Api {
            code: str_of(err, "code"),
            info: str_of(err, "info"),
        });
    }
    Ok(v)
}

fn str_of(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn page_url(title: &str) -> String {
    format!("https://www.poewiki.net/wiki/{}", title.replace(' ', "_"))
}

fn parse_search(v: &Value) -> Result<Vec<SearchHit>, WikiError> {
    let hits = v
        .pointer("/query/search")
        .and_then(Value::as_array)
        .ok_or_else(|| WikiError::Json("no query.search".into()))?;
    Ok(hits
        .iter()
        .map(|h| {
            let title = str_of(h, "title");
            SearchHit {
                url: page_url(&title),
                snippet: text::plain(&str_of(h, "snippet")),
                title,
            }
        })
        .collect())
}

fn parse_page(v: &Value) -> Result<Option<Page>, WikiError> {
    let page = v
        .pointer("/query/pages/0")
        .ok_or_else(|| WikiError::Json("no query.pages".into()))?;
    if page.get("missing").and_then(Value::as_bool) == Some(true) {
        return Ok(None);
    }
    let title = str_of(page, "title");
    let url = page
        .get("fullurl")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| page_url(&title));
    Ok(Some(Page {
        title,
        url,
        revision: page.get("lastrevid").and_then(Value::as_u64),
        text: str_of(page, "extract"),
    }))
}

fn parse_cargo(v: &Value) -> Result<Vec<Map<String, Value>>, WikiError> {
    let rows = v
        .get("cargoquery")
        .and_then(Value::as_array)
        .ok_or_else(|| WikiError::Json("no cargoquery".into()))?;
    Ok(rows
        .iter()
        .filter_map(|r| r.get("title").and_then(Value::as_object))
        .map(|row| {
            // Cargo answers with spaces where the field names have underscores.
            row.iter()
                .map(|(k, v)| {
                    let v = match v {
                        Value::String(s) => Value::String(text::decode_entities(s)),
                        other => other.clone(),
                    };
                    (k.replace(' ', "_"), v)
                })
                .collect()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_search_hits() {
        let v = json!({"query": {"search": [
            {"ns": 0, "title": "Withered", "snippet": "stats related to <span class=\"searchmatch\">withered</span>"}
        ]}});
        assert_eq!(
            parse_search(&v).unwrap(),
            [SearchHit {
                title: "Withered".into(),
                snippet: "stats related to withered".into(),
                url: "https://www.poewiki.net/wiki/Withered".into(),
            }]
        );
    }

    #[test]
    fn reads_a_page_and_a_missing_one() {
        let v = json!({"query": {"pages": [{"title": "Withered", "extract": "Withered is a debuff.",
            "lastrevid": 1702206, "fullurl": "https://www.poewiki.net/wiki/Withered"}]}});
        let page = parse_page(&v).unwrap().unwrap();
        assert_eq!(page.title, "Withered");
        assert_eq!(page.revision, Some(1702206));
        assert_eq!(page.text, "Withered is a debuff.");

        let v = json!({"query": {"pages": [{"title": "No such page", "missing": true}]}});
        assert_eq!(parse_page(&v).unwrap(), None);
    }

    #[test]
    fn reads_cargo_rows_with_the_real_field_names() {
        let v = json!({"cargoquery": [{"title": {"name": "Admiral&#039;s Arrogance", "drop enabled": "1"}}]});
        let rows = parse_cargo(&v).unwrap();
        assert_eq!(rows[0]["name"], "Admiral's Arrogance");
        assert_eq!(rows[0]["drop_enabled"], "1");
    }

    #[test]
    fn reports_api_errors() {
        let err =
            parse_json(r#"{"error":{"code":"maxlag","info":"Waiting for a database server"}}"#)
                .unwrap_err();
        assert!(matches!(err, WikiError::Api { ref code, .. } if code == "maxlag"));
    }

    // Talks to poewiki.net, so it only runs by hand: cargo test -p exile-wiki -- --ignored
    #[test]
    #[ignore]
    fn reads_the_live_wiki() {
        let dir = tempfile::tempdir().unwrap();
        let wiki = Wiki::new(
            "exile-companion-tests (https://github.com/muirch/exile-companion)",
            Some(dir.path().into()),
        );
        assert!(wiki
            .search("withered", 3)
            .unwrap()
            .iter()
            .any(|h| h.title == "Withered"));
        let page = wiki.page("Withered").unwrap().unwrap();
        assert!(page.text.contains("chaos damage taken"), "{}", page.text);
        assert!(wiki
            .page("No such page for exile companion")
            .unwrap()
            .is_none());
        let rows = wiki
            .cargo(&CargoQuery {
                tables: "items".into(),
                fields: "items.name,items.base_item,items.drop_enabled".into(),
                where_: Some("items.name=\"Widowhail\"".into()),
                limit: 5,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(rows[0]["base_item"], "Crude Bow");
        // The second call comes from the cache.
        assert_eq!(wiki.page("Withered").unwrap().unwrap().text, page.text);
    }
}
