//! The engine with the build loaded. Started on the first question about the build, since PoB
//! takes a couple of seconds and half a gigabyte, and restarted once if it died.

use std::path::PathBuf;
use std::time::Duration;

use exile_pob_engine::{locate, Engine};
use serde_json::{json, Value};

const STARTUP: Duration = Duration::from_secs(120);
const CALL: Duration = Duration::from_secs(90);
const NO_BUILD: &str =
    "No build yet. Import one in Exile Companion's settings, or give me a PoB code or link.";

pub struct Options {
    pub engine: Option<PathBuf>,
    pub pob: Option<PathBuf>,
    pub build: Option<PathBuf>,
}

pub struct Session {
    opts: Options,
    /// Reads guide pages for their PoB link when the player gives a page instead of the link.
    guides: std::sync::Arc<exile_guides::Guides>,
    engine: Option<Engine>,
    /// A build loaded during this conversation replaces the app's build until the server stops.
    build_xml: Option<String>,
    /// Reference builds (guides) in the order they came, so a restarted engine gets them back.
    references: Vec<String>,
    /// How many of them the running engine has. Reset when the engine starts again.
    references_loaded: usize,
    /// Whether the running engine has the player's build. A new player may have none yet, and
    /// load_build and guides still have to work then.
    has_build: bool,
}

impl Session {
    pub fn new(opts: Options, guides: std::sync::Arc<exile_guides::Guides>) -> Self {
        Self {
            opts,
            guides,
            engine: None,
            build_xml: None,
            references: Vec::new(),
            references_loaded: 0,
            has_build: false,
        }
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let had_engine = self.engine.is_some();
        match self.try_call(method, params.clone()) {
            // The engine died or hung and was dropped. One fresh start, then give up.
            Err(_) if had_engine && self.engine.is_none() => self.try_call(method, params),
            other => other,
        }
    }

    /// Loads a build for the rest of the conversation: an import code, a build link or XML.
    pub fn load(&mut self, text: &str) -> Result<Value, String> {
        let xml = self.to_xml(text)?;
        let summary = self
            .engine()?
            .call("load_build", json!({ "xml": xml }), CALL);
        let summary = self.keep_alive(summary)?;
        self.build_xml = Some(xml);
        self.has_build = true;
        Ok(summary)
    }

    /// The engine's index of a reference build, loading it the first time it's asked for.
    pub fn reference(&mut self, text: &str) -> Result<usize, String> {
        let xml = self.to_xml(text)?;
        let index = match self.references.iter().position(|r| *r == xml) {
            Some(i) => i,
            None => {
                self.references.push(xml);
                self.references.len() - 1
            }
        };
        self.engine()?;
        while self.references_loaded < self.references.len() {
            let xml = self.references[self.references_loaded].clone();
            let r = self.engine()?.call(
                "load_reference",
                json!({ "xml": xml, "label": "Guide" }),
                CALL,
            );
            if let Err(e) = self.keep_alive(r) {
                // A build PoB can't open shouldn't stay in the list and fail every later call.
                self.references.truncate(self.references_loaded);
                return Err(e);
            }
            self.references_loaded += 1;
        }
        // The engine counts from 1, like Lua.
        Ok(index + 1)
    }

    /// An engine method that doesn't read the player's build, such as a guide's own stages.
    pub fn call_any(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let result = self.engine()?.call(method, params, CALL);
        self.keep_alive(result)
    }

    /// A PoB code, a build link, build XML, or a guide page with a build link on it.
    fn to_xml(&self, text: &str) -> Result<String, String> {
        let text = text.trim();
        let link = match exile_pob::code_url(text) {
            Some(url) => Some(url),
            None if text.starts_with("http://") || text.starts_with("https://") => {
                let page_link = self.guides.pob_link(text).map_err(|e| match e {
                    exile_guides::GuideError::Blocked(host) => format!(
                        "{host} doesn't let programs read its pages. Open the guide in your browser and copy its Path of Building link (pobb.in or poe.ninja) or code, or look the guide up with find_guides."
                    ),
                    e => e.to_string(),
                })?;
                let page_link = page_link
                    .ok_or("That page has no Path of Building link. Copy the guide's PoB link or code from it instead.")?;
                Some(
                    exile_pob::code_url(&page_link)
                        .ok_or(format!("Couldn't use the link {page_link} from that page."))?,
                )
            }
            None => None,
        };
        let text = match link {
            Some(url) => fetch(&url)?,
            None => text.to_string(),
        };
        let xml = exile_pob::xml_from_text(&text).map_err(|_| {
            "That's not a Path of Building code, a build link, build XML or a guide page with a build link.".to_string()
        })?;
        Ok(String::from_utf8_lossy(&xml).into_owned())
    }

    /// Every engine method behind `call` reads the player's build.
    fn try_call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.engine()?;
        if !self.has_build {
            return Err(NO_BUILD.to_string());
        }
        let result = self.engine()?.call(method, params, CALL);
        self.keep_alive(result)
    }

    /// The build this conversation loaded, else the one in the app's settings, else none.
    fn player_xml(&self) -> Result<Option<String>, String> {
        if let Some(xml) = &self.build_xml {
            return Ok(Some(xml.clone()));
        }
        match self.opts.build.clone().or_else(crate::paths::saved_build) {
            Some(path) => std::fs::read_to_string(&path)
                .map(Some)
                .map_err(|e| format!("{}: {e}", path.display())),
            None => Ok(None),
        }
    }

    fn keep_alive(
        &mut self,
        result: Result<Value, exile_pob_engine::EngineError>,
    ) -> Result<Value, String> {
        use exile_pob_engine::EngineError::*;
        match result {
            Ok(v) => Ok(v),
            Err(e @ (Exited(_) | Timeout(_) | Io(_) | Protocol(_))) => {
                self.engine = None;
                Err(e.to_string())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    fn engine(&mut self) -> Result<&mut Engine, String> {
        if self.engine.is_none() {
            let exe = match &self.opts.engine {
                Some(exe) if exe.is_file() => exe.clone(),
                Some(exe) => return Err(format!("The calculation engine isn't at {}.", exe.display())),
                None => crate::paths::engine_exe().ok_or(
                    "The calculation engine (exile-pob-engine) isn't installed next to this server.",
                )?,
            };
            let saved = self.opts.pob.clone().or_else(crate::paths::saved_pob);
            let pob = locate::find_pob(saved.as_deref(), &[]).ok_or(
                "Path of Building wasn't found. Pick its folder in Exile Companion's settings.",
            )?;
            let xml = self.player_xml()?;
            let mut engine = Engine::start(&exe, &pob.root, STARTUP).map_err(|e| e.to_string())?;
            self.has_build = false;
            if let Some(xml) = xml {
                engine
                    .call("load_build", json!({ "xml": xml }), CALL)
                    .map_err(|e| e.to_string())?;
                self.has_build = true;
            }
            self.engine = Some(engine);
            self.references_loaded = 0;
        }
        Ok(self.engine.as_mut().expect("started above"))
    }
}

fn fetch(url: &str) -> Result<String, String> {
    // The default roots are a bundled Mozilla list that schannel rejects. Use the OS store.
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(15)))
        .user_agent(crate::USER_AGENT)
        .build()
        .into();
    agent
        .get(url)
        .call()
        .and_then(|mut r| r.body_mut().read_to_string())
        .map_err(|e| format!("Couldn't download the build from {url}: {e}"))
}
