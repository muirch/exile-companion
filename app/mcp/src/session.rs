//! The engine with the build loaded. Started on the first question about the build, since PoB
//! takes a couple of seconds and half a gigabyte, and restarted once if it died.

use std::path::PathBuf;
use std::time::Duration;

use exile_core::build_history::{BuildHistory, HistoryError};
use exile_core::followed_guide::{GuideStore, Stage};
use exile_pob_engine::{locate, Engine};
use serde_json::{json, Value};

const STARTUP: Duration = Duration::from_secs(120);
const CALL: Duration = Duration::from_secs(90);
const NO_BUILD: &str =
    "No build yet. Import one in Exile Companion's settings, or give me a PoB code or link.";
const LOADED: &str = "A build loaded with load_build is in use. Only the player's own build in Exile Companion keeps changes. back_to_my_build switches back.";
const NO_GUIDE: &str = "No guide given, and the player follows none. Ask which guide they follow, or help them pick one with find_guides.";

pub struct Options {
    pub engine: Option<PathBuf>,
    pub pob: Option<PathBuf>,
    pub build: Option<PathBuf>,
    pub log: Option<PathBuf>,
}

pub struct Session {
    opts: Options,
    /// Reads guide pages for their PoB link when the player gives a page instead of the link.
    guides: std::sync::Arc<exile_guides::Guides>,
    engine: Option<Engine>,
    /// A build from load_build. It replaces the app's build until back_to_my_build, or until the
    /// server stops, which in Claude Desktop is when the app quits, not when a chat ends.
    build_xml: Option<String>,
    /// Reference builds (guides) in the order they came, so a restarted engine gets them back.
    references: Vec<String>,
    /// How many of them the running engine has. Reset when the engine starts again.
    references_loaded: usize,
    /// Whether the running engine has the player's build. A new player may have none yet, and
    /// load_build and guides still have to work then.
    has_build: bool,
    /// The build file as the running engine read it, to notice an import made in the app since.
    disk_xml: Option<String>,
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
            disk_xml: None,
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
                    .ok_or("I couldn't find the guide's own Path of Building link on that page. It may have none, or link several builds. Copy the guide's PoB link or code instead.")?;
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

    /// Makes changes to the player's build file for good and records them with the reason.
    pub fn apply(&mut self, changes: Value, reason: &str) -> Result<Value, String> {
        if self.build_xml.is_some() {
            return Err(LOADED.to_string());
        }
        let path = self.build_path().ok_or(NO_BUILD)?;
        self.engine()?;
        if !self.has_build {
            return Err(NO_BUILD.to_string());
        }
        let disk = read(&path)?;
        if self.disk_xml.as_deref() != Some(disk.as_str()) {
            self.reload(&disk)?;
            return Err("The build in Exile Companion changed since I last read it, the player may have imported another one. I loaded it now: measure the change on this build again before applying it.".to_string());
        }
        let r = self
            .engine()?
            .call("apply_changes", json!({ "changes": changes }), CALL);
        let mut effect = self.keep_alive(r)?;
        let xml = effect
            .as_object_mut()
            .and_then(|o| o.remove("xml"))
            .and_then(|x| x.as_str().map(str::to_string))
            .ok_or("The engine didn't return the changed build.")?;
        let entry = BuildHistory::new(&path)
            .record(
                disk.as_bytes(),
                xml.as_bytes(),
                reason,
                changes,
                effect.clone(),
            )
            .map_err(|e| format!("Couldn't save the build: {e}"))?;
        self.disk_xml = Some(xml);
        effect["change"] = json!(entry.id);
        Ok(effect)
    }

    /// The changes kept for the player's build, oldest first.
    pub fn history(&self) -> Result<Value, String> {
        let path = self.build_path().ok_or(NO_BUILD)?;
        let entries = BuildHistory::new(&path)
            .entries()
            .map_err(|e| e.to_string())?;
        let list: Vec<Value> = entries
            .into_iter()
            .map(|e| {
                json!({
                    "change": e.id,
                    "time": e.time,
                    "reason": e.reason,
                    "changes": e.changes,
                    "dps_pct": e.effect["dps_pct"],
                    "ehp_pct": e.effect["ehp_pct"],
                    "life": e.effect["life"],
                })
            })
            .collect();
        Ok(json!({ "changes": list }))
    }

    /// Puts the player's build back as it was before the last change.
    pub fn undo(&mut self) -> Result<Value, String> {
        if self.build_xml.is_some() {
            return Err(LOADED.to_string());
        }
        let path = self.build_path().ok_or(NO_BUILD)?;
        let (entry, before) = BuildHistory::new(&path).undo().map_err(|e| match e {
            HistoryError::Empty => "There's no change to undo.".to_string(),
            HistoryError::Changed => "The build was imported again since the last change, so there's nothing of mine to undo.".to_string(),
            e => format!("Couldn't undo: {e}"),
        })?;
        if before.is_empty() {
            // There was no build before. A fresh engine reads the missing file as none.
            self.engine = None;
        } else if self.engine.is_some() {
            self.reload(&String::from_utf8_lossy(&before))?;
        }
        Ok(json!({ "undone": entry.id, "reason": entry.reason, "changes": entry.changes }))
    }

    fn reload(&mut self, xml: &str) -> Result<(), String> {
        let r = self
            .engine()?
            .call("load_build", json!({ "xml": xml }), CALL);
        self.keep_alive(r)?;
        self.disk_xml = Some(xml.to_string());
        self.has_build = true;
        Ok(())
    }

    /// Makes a build, often a guide, the player's build in Exile Companion. It goes into the
    /// history like a kept change, so undo_change brings the old one back.
    pub fn replace_build(&mut self, text: &str, reason: &str) -> Result<Value, String> {
        let path = self
            .build_file()
            .ok_or("Exile Companion's settings folder wasn't found.")?;
        let xml = self.to_xml(text)?;
        let before = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        self.build_xml = None;
        let summary = self
            .engine()?
            .call("load_build", json!({ "xml": xml }), CALL);
        let summary = match self.keep_alive(summary) {
            Ok(v) => v,
            Err(e) => {
                // The engine may hold neither build now. The next call starts it from the file.
                self.engine = None;
                return Err(e);
            }
        };
        let source = text.trim().starts_with("http").then(|| text.trim());
        let entry = BuildHistory::new(&path)
            .record(
                &before,
                xml.as_bytes(),
                reason,
                json!([{ "replaced_build": source.unwrap_or("pasted build") }]),
                json!({}),
            )
            .map_err(|e| format!("Couldn't save the build: {e}"))?;
        self.disk_xml = Some(xml);
        self.has_build = true;
        Ok(json!({ "change": entry.id, "build": summary }))
    }

    /// Drops a build from load_build and goes back to the player's own.
    pub fn back_to_mine(&mut self) -> Result<Value, String> {
        if self.build_xml.take().is_none() {
            return Ok(json!({ "already": "The player's own build was in use." }));
        }
        self.engine = None;
        self.engine()?;
        if !self.has_build {
            return Ok(json!({ "build": null, "note": NO_BUILD }));
        }
        let summary = self.call("summary", json!({}))?;
        Ok(json!({ "build": summary }))
    }

    /// Follows a guide from now on, also in later conversations. Without `text`, moves the
    /// followed guide to another stage.
    pub fn follow(&mut self, text: Option<&str>, stage: Stage) -> Result<Value, String> {
        let store = self.guide_store()?;
        let followed = match text {
            Some(text) => {
                let xml = self.to_xml(text)?;
                // PoB has to open it before it's worth keeping.
                let reference = self.reference(&xml)?;
                self.call_any("reference_info", json!({ "reference": reference }))?;
                let source = text.trim().starts_with("http").then(|| text.trim());
                store.follow(&xml, source, stage)
            }
            None => store
                .set_stage(stage)
                .and_then(|f| f.ok_or_else(|| std::io::Error::other(NO_GUIDE))),
        }
        .map_err(|e| e.to_string())?;
        Ok(json!({ "following": followed }))
    }

    pub fn forget_guide(&self) -> Result<Value, String> {
        self.guide_store()?.forget().map_err(|e| e.to_string())?;
        Ok(json!({ "following": null }))
    }

    /// The guide text to use: the one given, else the followed guide's build. The stage is the
    /// followed one when the guide is.
    pub fn guide_or_followed(
        &self,
        guide: Option<String>,
    ) -> Result<(String, Option<Stage>), String> {
        if let Some(g) = guide.filter(|g| !g.trim().is_empty()) {
            return Ok((g, None));
        }
        match self.guide_store()?.get().map_err(|e| e.to_string())? {
            Some((followed, xml)) => Ok((xml, Some(followed.stage))),
            None => Err(NO_GUIDE.to_string()),
        }
    }

    /// What a new conversation needs first: where the player is in the game, the guide they
    /// follow and which build the tools read. Nothing here starts Path of Building.
    pub fn status(&self) -> Value {
        let game = crate::progress::game_progress(self.opts.log.as_deref());
        let guide = match self
            .guide_store()
            .and_then(|s| s.get().map_err(|e| e.to_string()))
        {
            Ok(Some((followed, xml))) => {
                let mut v = build_outline(&xml);
                v["source"] = json!(followed.source);
                v["since"] = json!(followed.since);
                v["stage"] = json!(followed.stage);
                v
            }
            Ok(None) => json!(null),
            Err(e) => json!({ "error": e }),
        };
        let build = match &self.build_xml {
            Some(xml) => {
                let mut v = build_outline(xml);
                v["in_use"] = json!(
                    "a build from load_build, not the player's own; back_to_my_build switches back"
                );
                v
            }
            None => match self.build_path().map(|p| read(&p)) {
                Some(Ok(xml)) => build_outline(&xml),
                Some(Err(e)) => json!({ "error": e }),
                None => json!(null),
            },
        };
        json!({ "game": game, "guide": guide, "build": build })
    }

    fn guide_store(&self) -> Result<GuideStore, String> {
        let dir = self
            .build_file()
            .and_then(|f| f.parent().map(PathBuf::from))
            .ok_or("Exile Companion's settings folder wasn't found.")?;
        Ok(GuideStore::new(dir))
    }

    /// Where the player's build lives, whether or not there is one yet.
    fn build_file(&self) -> Option<PathBuf> {
        self.opts.build.clone().or_else(crate::paths::build_file)
    }

    fn build_path(&self) -> Option<PathBuf> {
        self.build_file().filter(|p| p.is_file())
    }

    /// The build this conversation loaded, else the one in the app's settings, else none.
    /// The second value says whether it came from the build file.
    fn player_xml(&self) -> Result<Option<(String, bool)>, String> {
        if let Some(xml) = &self.build_xml {
            return Ok(Some((xml.clone(), false)));
        }
        match self.build_path() {
            Some(path) => read(&path).map(|xml| Some((xml, true))),
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
            self.disk_xml = None;
            if let Some((xml, from_disk)) = xml {
                engine
                    .call("load_build", json!({ "xml": xml }), CALL)
                    .map_err(|e| e.to_string())?;
                self.has_build = true;
                self.disk_xml = from_disk.then_some(xml);
            }
            self.engine = Some(engine);
            self.references_loaded = 0;
        }
        Ok(self.engine.as_mut().expect("started above"))
    }
}

/// Class, level and the stage names of a build, read without Path of Building.
fn build_outline(xml: &str) -> Value {
    match exile_pob::parse_xml(xml.as_bytes()) {
        Ok(b) => json!({
            "class": b.class_name,
            "ascendancy": b.ascend_class_name,
            "level": b.level,
            "trees": b.specs.iter().map(|s| &s.title).collect::<Vec<_>>(),
            "skill_sets": b.skill_sets.iter().map(|s| &s.title).collect::<Vec<_>>(),
        }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

fn read(path: &std::path::Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
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
