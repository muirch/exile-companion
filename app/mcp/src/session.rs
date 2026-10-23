//! The engine with the build loaded. Started on the first question about the build, since PoB
//! takes a couple of seconds and half a gigabyte, and restarted once if it died.

use std::path::PathBuf;
use std::time::Duration;

use exile_pob_engine::{locate, Engine};
use serde_json::{json, Value};

const STARTUP: Duration = Duration::from_secs(120);
const CALL: Duration = Duration::from_secs(90);

pub struct Options {
    pub engine: Option<PathBuf>,
    pub pob: Option<PathBuf>,
    pub build: Option<PathBuf>,
}

pub struct Session {
    opts: Options,
    engine: Option<Engine>,
    /// A build loaded during this conversation replaces the app's build until the server stops.
    build_xml: Option<String>,
}

impl Session {
    pub fn new(opts: Options) -> Self {
        Self {
            opts,
            engine: None,
            build_xml: None,
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
        let text = match exile_pob::code_url(text) {
            Some(url) => fetch(&url)?,
            None => text.to_string(),
        };
        let xml = exile_pob::xml_from_text(&text).map_err(|_| {
            "That's not a Path of Building code, a build link or build XML.".to_string()
        })?;
        let xml = String::from_utf8_lossy(&xml).into_owned();
        let summary = self
            .engine()?
            .call("load_build", json!({ "xml": xml }), CALL);
        let summary = self.keep_alive(summary)?;
        self.build_xml = Some(xml);
        Ok(summary)
    }

    fn try_call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let result = self.engine()?.call(method, params, CALL);
        self.keep_alive(result)
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
            let mut engine = Engine::start(&exe, &pob.root, STARTUP).map_err(|e| e.to_string())?;
            let xml = match &self.build_xml {
                Some(xml) => xml.clone(),
                None => {
                    let path = self.opts.build.clone().or_else(crate::paths::saved_build).ok_or(
                        "No build yet. Import one in Exile Companion's settings, or give me a PoB code or link.",
                    )?;
                    std::fs::read_to_string(&path)
                        .map_err(|e| format!("{}: {e}", path.display()))?
                }
            };
            engine
                .call("load_build", json!({ "xml": xml }), CALL)
                .map_err(|e| e.to_string())?;
            self.engine = Some(engine);
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
