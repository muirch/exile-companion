//! exile-companion-mcp [--pob <dir>] [--build <file.xml>] [--engine <exile-pob-engine>]
//!
//! An MCP server on stdin and stdout. Claude Desktop or Claude Code start it, and Claude asks
//! Path of Building (through exile-pob-engine) and the Path of Exile Wiki about the player's build.
//! By default it uses the build and the PoB folder from Exile Companion's settings.

mod paths;
mod session;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use exile_wiki::{CargoQuery, Wiki};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde::Deserialize;
use serde_json::{json, Value};

use session::{Options, Session};

pub const USER_AGENT: &str = concat!(
    "ExileCompanion/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/muirch/exile-companion)"
);

/// Long wiki pages are cut here. The section list tells the model what else there is.
const PAGE_LIMIT: usize = 12_000;

const INSTRUCTIONS: &str = "Tools for one Path of Exile 1 build, the one the player imported in Exile Companion, \
calculated by the player's own Path of Building, and for the Path of Exile Wiki.

Numbers come from Path of Building with the build's own Configuration (boss, buffs, charges, custom modifiers). \
Check build_details for those assumptions before you judge damage or defences, and tell the player when an answer depends on them. \
To weigh a change, measure it with what_if, compare_item, passive_value or gem_value instead of estimating. \
A percent is not a recommendation by itself. Keystones and big notables have drawbacks Path of Building doesn't price in \
(read the node's own text, and the wiki for the keystone), they cost points on the way, and players rarely take some of them for good reasons. \
Say the trade-off and how sure you are, and point out what you couldn't measure. \
When the player follows a guide, or you found one for them and they loaded it, advise from the difference to the guide (compare_with_guide) \
and pass the guide to passive_value. Don't suggest passives, keystones or items the guide doesn't take unless the player asks to explore. \
Nothing you try changes the player's build. load_build switches to another build for this conversation only.

Use the wiki for mechanics, crafting, league content and anything Path of Building doesn't model. \
Search first, then read one section at a time. Wiki content is CC BY-NC-SA 3.0: link the pages you used.";

fn text(v: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(v)])
}

fn failure(v: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(v)])
}

fn json_result(r: Result<Value, String>) -> CallToolResult {
    match r {
        Ok(v) => text(v.to_string()),
        Err(e) => failure(e),
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GroupParams {
    /// Socket group: its index, its label, or the name of a gem in it. The main skill's group by default.
    pub group: Option<Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PassiveParams {
    /// How many nodes to list in each list. Default 10.
    pub limit: Option<u32>,
    /// How many points away an unallocated notable may be. Default 3, or any distance with a guide.
    pub max_points: Option<u32>,
    /// A guide's build (code, link or XML). Then only notables the guide takes are offered.
    pub guide: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GuideParams {
    /// The guide's build: a PoB import code, a build link (pobb.in, poe.ninja and the other sites PoB imports from) or build XML.
    pub guide: String,
    /// Guide stage to compare with, by id from the guide's lists: its tree, item set and skill set. The guide's saved choice by default.
    pub tree: Option<u32>,
    pub item_set: Option<u32>,
    pub skill_set: Option<u32>,
    /// How many passives and gear slots to list. Default 12.
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ItemParams {
    /// The item as the game copies it with Ctrl+C, or as Path of Building shows it.
    pub text: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WhatIfParams {
    /// Changes tried together. Each is one object, one of:
    /// {"add_nodes": [ids or names]} allocates them with the path to them,
    /// {"remove_nodes": [ids or names]},
    /// {"item": "<item text>", "slot": "Helmet"},
    /// {"gem": "Name", "group": <group>, "enabled": bool, "level": n, "quality": n},
    /// {"add_gem": "Name", "group": <group>, "level": n, "quality": n},
    /// {"config": "<var from build_details>", "value": v},
    /// {"custom_mod": "<a mod line, e.g. +1 to Level of all Skill Gems>"},
    /// {"level": n} for the character level.
    pub changes: Vec<Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LoadParams {
    /// A Path of Building import code, a build link (pobb.in, poe.ninja, pastebin and the other sites PoB imports from) or build XML.
    pub build: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WikiSearchParams {
    /// Keywords, as you would type them into the wiki's search box.
    pub query: String,
    /// Default 8, at most 20.
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WikiPageParams {
    /// Page title, as wiki_search returns it.
    pub title: String,
    /// A section heading from the page's section list. Without it you get the page from the top, cut at a size limit.
    pub section: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CargoParams {
    /// Cargo tables, comma separated: items, mods, skill, skill_levels, areas, versions and others listed on Special:CargoTables.
    pub tables: String,
    /// Fields, comma separated, e.g. items.name,items.base_item,items.drop_enabled.
    pub fields: String,
    /// SQL-like condition, e.g. items.rarity="Unique" AND items.class="Gloves".
    pub r#where: Option<String>,
    pub join_on: Option<String>,
    pub order_by: Option<String>,
    /// Default 50, at most 500.
    pub limit: Option<u32>,
}

#[derive(Clone)]
pub struct Companion {
    session: Arc<Mutex<Session>>,
    wiki: Arc<Wiki>,
}

impl Companion {
    async fn pob_value(&self, method: &'static str, params: Value) -> Result<Value, String> {
        let session = self.session.clone();
        tokio::task::spawn_blocking(move || session.lock().unwrap().call(method, params))
            .await
            .unwrap_or_else(|e| Err(e.to_string()))
    }

    async fn pob(&self, method: &'static str, params: Value) -> CallToolResult {
        json_result(self.pob_value(method, params).await)
    }

    async fn wiki<T, F>(&self, f: F) -> CallToolResult
    where
        T: serde::Serialize + Send + 'static,
        F: FnOnce(&Wiki) -> Result<T, exile_wiki::WikiError> + Send + 'static,
    {
        let wiki = self.wiki.clone();
        let r = tokio::task::spawn_blocking(move || f(&wiki))
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|e| e.to_string()))
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()));
        json_result(r)
    }
}

#[tool_router]
impl Companion {
    pub fn new(opts: Options) -> Self {
        let cache = paths::cache_dir().map(|d| d.join("wiki"));
        Self {
            session: Arc::new(Mutex::new(Session::new(opts))),
            wiki: Arc::new(Wiki::new(USER_AGENT, cache)),
        }
    }

    #[tool(
        description = "The build's left sidebar in Path of Building: every stat row as PoB formats it, PoB's warnings, and the main skill. Start here."
    )]
    async fn build_overview(&self) -> Result<CallToolResult, McpError> {
        Ok(self.pob("sidebar_stats", json!({})).await)
    }

    #[tool(
        description = "Everything in the build: gear with all mod lines, socket groups with gem levels and quality, keystones, notables, ascendancy, and the Configuration options and custom modifiers that differ from PoB's defaults."
    )]
    async fn build_details(&self) -> Result<CallToolResult, McpError> {
        Ok(self.pob("build_info", json!({})).await)
    }

    #[tool(
        description = "Defences: max hit per damage type, resistances and overcap, damage over time EHP, avoidance, ailment and stun avoidance, life, energy shield, recovery."
    )]
    async fn defences(&self) -> Result<CallToolResult, McpError> {
        Ok(self.pob("defences", json!({})).await)
    }

    #[tool(
        description = "What each gem in a socket group is worth: the DPS lost, in percent, when it's turned off."
    )]
    async fn gem_value(
        &self,
        Parameters(p): Parameters<GroupParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.pob("gem_value", json!({ "group": p.group })).await)
    }

    #[tool(
        description = "What passive notables and keystones are worth: DPS and EHP lost without each one the build has, and the best unallocated ones per point spent. Pass the player's guide so only notables the guide takes are offered."
    )]
    async fn passive_value(
        &self,
        Parameters(p): Parameters<PassiveParams>,
    ) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || {
            let mut s = session.lock().unwrap();
            let mut params = json!({ "limit": p.limit.unwrap_or(10) });
            if let Some(points) = p.max_points {
                params["max_points"] = json!(points);
            }
            if let Some(guide) = &p.guide {
                params["reference"] = json!(s.reference(guide)?);
            }
            s.call("node_power", params)
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
    }

    #[tool(
        description = "Compares the player's build with a guide's build, measured on the player's build: notables the guide takes and the player doesn't (with the points on the way), the player's notables the guide skips, ascendancy, gear slot by slot, supports of the main skill, other skills the guide uses, and Configuration assumptions that differ. Returns the guide's stages (trees, item sets, skill sets) too. Pick the stage closest to the player's level and progress."
    )]
    async fn compare_with_guide(
        &self,
        Parameters(p): Parameters<GuideParams>,
    ) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || {
            let mut s = session.lock().unwrap();
            let reference = s.reference(&p.guide)?;
            if p.tree.is_some() || p.item_set.is_some() || p.skill_set.is_some() {
                s.call(
                    "set_reference_stage",
                    json!({ "reference": reference, "tree": p.tree, "item_set": p.item_set, "skill_set": p.skill_set }),
                )?;
            }
            s.call("compare_reference", json!({ "reference": reference, "limit": p.limit.unwrap_or(12) }))
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
    }

    #[tool(
        description = "Is this item an upgrade: compares a pasted item against every slot it fits, with the stat changes PoB would show. Best slot first."
    )]
    async fn compare_item(
        &self,
        Parameters(p): Parameters<ItemParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.pob("compare_item", json!({ "text": p.text })).await)
    }

    #[tool(
        description = "Tries changes together and measures them against the build: passive nodes, an item in a slot, gem on/off, gem level and quality, an added support gem, Configuration options, custom mod lines, character level. Returns DPS and EHP change in percent and every stat PoB shows that changed. Nothing is kept."
    )]
    async fn what_if(
        &self,
        Parameters(p): Parameters<WhatIfParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.pob("what_if", json!({ "changes": p.changes })).await)
    }

    #[tool(
        description = "The build's Path of Building import code, to paste into PoB's Import/Export tab."
    )]
    async fn export_build(&self) -> Result<CallToolResult, McpError> {
        // The XML is long and the code carries the same build.
        let r = self.pob_value("export_build", json!({})).await;
        Ok(json_result(r.map(|v| json!({ "code": v["code"] }))))
    }

    #[tool(
        description = "Switches to another build for the rest of this conversation, from a PoB code, a build link or XML. The build in Exile Companion stays as it is."
    )]
    async fn load_build(
        &self,
        Parameters(p): Parameters<LoadParams>,
    ) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || session.lock().unwrap().load(&p.build))
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
    }

    #[tool(
        description = "Searches the Path of Exile Wiki. Returns page titles with a snippet and the page URL."
    )]
    async fn wiki_search(
        &self,
        Parameters(p): Parameters<WikiSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let limit = p.limit.unwrap_or(8).min(20) as usize;
        Ok(self.wiki(move |w| w.search(&p.query, limit)).await)
    }

    #[tool(
        description = "Reads a Path of Exile Wiki page as plain text, one section at a time. Returns the page's section headings so you can ask for the one you need."
    )]
    async fn wiki_page(
        &self,
        Parameters(p): Parameters<WikiPageParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self
            .wiki(move |w| {
                let Some(page) = w.page(&p.title)? else {
                    return Ok(json!({ "missing": true, "title": p.title }));
                };
                let sections = page.sections();
                let headings: Vec<&str> = sections.iter().map(|s| s.heading.as_str()).filter(|h| !h.is_empty()).collect();
                let (body, cut) = match &p.section {
                    Some(name) => match sections.iter().find(|s| s.heading.eq_ignore_ascii_case(name)) {
                        Some(s) => (s.body.clone(), false),
                        None => return Ok(json!({ "title": page.title, "url": page.url, "no_such_section": name, "sections": headings })),
                    },
                    None => {
                        let mut t = page.text.clone();
                        let cut = t.len() > PAGE_LIMIT;
                        if cut {
                            let mut end = PAGE_LIMIT;
                            while !t.is_char_boundary(end) {
                                end -= 1;
                            }
                            t.truncate(end);
                        }
                        (t, cut)
                    }
                };
                Ok(json!({
                    "title": page.title,
                    "url": page.url,
                    "sections": headings,
                    "section": p.section,
                    "text": body,
                    "cut": cut,
                    "license": exile_wiki::ATTRIBUTION,
                }))
            })
            .await)
    }

    #[tool(
        description = "Queries the wiki's Cargo tables, structured game data: items (with drop_enabled and league-only flags), mods, skills, areas, versions. Read-only."
    )]
    async fn wiki_data(
        &self,
        Parameters(p): Parameters<CargoParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self
            .wiki(move |w| {
                w.cargo(&CargoQuery {
                    tables: p.tables,
                    fields: p.fields,
                    where_: p.r#where,
                    join_on: p.join_on,
                    order_by: p.order_by,
                    limit: p.limit.unwrap_or(50) as usize,
                    ..Default::default()
                })
            })
            .await)
    }
}

#[tool_handler]
impl ServerHandler for Companion {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("exile-companion", env!("CARGO_PKG_VERSION"))
                    .with_title("Exile Companion"),
            )
            .with_instructions(INSTRUCTIONS)
    }
}

fn options() -> Result<Options, String> {
    let mut opts = Options {
        engine: None,
        pob: None,
        build: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || {
            args.next()
                .map(PathBuf::from)
                .ok_or(format!("{a} needs a path"))
        };
        match a.as_str() {
            "--engine" => opts.engine = Some(value()?),
            "--pob" => opts.pob = Some(value()?),
            "--build" => opts.build = Some(value()?),
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    Ok(opts)
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let opts = match options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}\nusage: exile-companion-mcp [--pob <dir>] [--build <file.xml>] [--engine <path>]");
            return std::process::ExitCode::FAILURE;
        }
    };
    let served = match Companion::new(opts).serve(rmcp::transport::stdio()).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("couldn't start: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    if let Err(e) = served.waiting().await {
        eprintln!("{e}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
