//! exile-companion-mcp [--pob <dir>] [--build <file.xml>] [--log <Client.txt>] [--engine <exile-pob-engine>]
//!
//! An MCP server on stdin and stdout. Claude Desktop or Claude Code start it, and Claude asks
//! Path of Building (through exile-pob-engine) and the Path of Exile Wiki about the player's build.
//! By default it uses the build and the PoB folder from Exile Companion's settings.

mod paths;
mod progress;
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
calculated by the player's own Path of Building, and for the Path of Exile Wiki and build guides.

Start a conversation with player_status: where the player is in the game (from the game's log), the guide they follow and the build the tools read. \
It costs nothing, so call it again when the player asks what to do now. A level-up is the only time the log names the level, so a fresh character may show none yet.

Check that the build you have is the one the player is talking about: game (these tools are Path of Exile 1 only), \
main skill, class and ascendancy, league. If build_overview shows something else than the player describes, say so and ask \
before you measure anything. Don't answer about one build with numbers from another.

Numbers come from Path of Building with the build's own Configuration (boss, buffs, charges, custom modifiers). \
Check build_details for those assumptions before you judge damage or defences, and tell the player when an answer depends on them. \
Guides often add placeholder custom mods (+200% resistances and the like), so their numbers are higher than a real character's. \
To weigh a change, measure it with what_if, compare_item, passive_value or gem_value instead of estimating. \
A percent is not a recommendation by itself. Keystones and big notables have drawbacks Path of Building doesn't price in \
(read the node's own text, and the wiki for the keystone), they cost points on the way, and players rarely take some of them for good reasons. \
Say the trade-off and how sure you are, and point out what you couldn't measure. \
When the player picks a guide, save it with follow_guide so later conversations know it. \
compare_with_guide, passive_value, guide_stages and guide_stage_diff use the followed guide when you pass none. \
Advise from the difference to the guide. guide_stages and guide_stage_diff walk a guide stage by stage, also for a player without a build yet. \
When the player moves on to another stage, update it with follow_guide. Don't suggest passives, keystones or items the guide doesn't take unless the player asks to explore. \
Compare with the guide stage closest to the player's level and progress, and keep endgame gear out of a beginner's next steps. \
Measuring changes nothing. apply_changes keeps changes in the player's build, and the app and later conversations see them: \
use it only when the player says to keep a change, after you measured it, with the reason in a few words. \
build_history lists what was kept and undo_change takes the last change back. \
A new player who wants to level with a guide has no build of their own yet. With their OK, use_as_my_build makes the guide their build in Exile Companion, \
so the overlay shows its gems along the campaign route. Follow the guide too. undo_change takes that back as well. \
load_build switches the tools to another build until back_to_my_build. That build doesn't keep changes, and it stays in use across conversations \
until the agent restarts, so switch back when you're done with it.

When a player asks what to play, don't pick for them at once. Ask how they like to play (ranged, melee, spells, minions), \
their budget, and whether they start fresh in a league. Then use find_guides and offer two or three current-league guides \
with a Path of Building link, each with why it fits and what it's bad at, and let them choose. \
New players usually do better with spells or minions than with bows or melee, say so when it matters. \
Take guides from find_guides, the guide sites they link (Mobalytics, PoE Vault, Maxroll) and pobb.in. \
Don't build advice on SEO tier lists or on sites that sell currency or items.

Use the wiki for mechanics, crafting, league content and anything Path of Building doesn't model. \
Search first, then read one section at a time. Wiki content is CC BY-NC-SA 3.0: link the pages you used. \
Items list the mod lines Path of Building doesn't calculate (not_calculated) and properties it keeps but doesn't use. \
Look those up in the wiki instead of guessing, and say the numbers leave them out.";

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
    /// A guide's build (code, link or XML). Then only notables the guide takes are offered. The followed guide when left out.
    pub guide: Option<String>,
    /// Offer any notable, not only the guide's. Only when the player asks to explore.
    pub explore: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GuideParams {
    /// The guide's build: a PoB import code, a build link (pobb.in, poe.ninja and the other sites PoB imports from), build XML, or the guide's page when it has a build link on it. The followed guide when left out.
    pub guide: Option<String>,
    /// Guide stage to compare with, by id from the guide's lists: its tree, item set and skill set. The followed stage, or the guide's saved choice, by default.
    pub tree: Option<u32>,
    pub item_set: Option<u32>,
    pub skill_set: Option<u32>,
    /// How many passives and gear slots to list. Default 12.
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StagesParams {
    /// The guide's build: a PoB code, a build link, build XML, or the guide's page with a build link on it. The followed guide when left out.
    pub guide: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct Stage {
    /// Ids from guide_stages. The guide's saved choice when left out.
    pub tree: Option<u32>,
    pub item_set: Option<u32>,
    pub skill_set: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StageDiffParams {
    /// The guide's build, as for guide_stages. The followed guide when left out.
    pub guide: Option<String>,
    pub from: Stage,
    pub to: Stage,
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
pub struct FollowParams {
    /// The guide's build: a PoB code, a build link, build XML, or the guide's page with a build link on it. Leave it out to change only the stage of the followed guide.
    pub guide: Option<String>,
    /// The stage the player is at, by ids from guide_stages. The guide's saved choice when left out.
    pub tree: Option<u32>,
    pub item_set: Option<u32>,
    pub skill_set: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReplaceParams {
    /// A PoB code, a build link, build XML, or a guide page with a build link on it.
    pub build: String,
    /// Why, in a few words the player will recognise in the history, e.g. "leveling with the guide's PoB".
    pub reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ApplyParams {
    /// Changes in the same form as what_if takes.
    pub changes: Vec<Value>,
    /// Why, in a few words the player will recognise in the history, e.g. "guide's lvl 40 tree: Herbalism".
    pub reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LoadParams {
    /// A Path of Building import code, a build link (pobb.in, poe.ninja, pastebin and the other sites PoB imports from), build XML, or a guide page (PoE Vault, poebuilds.cc and others) with a build link on it.
    pub build: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FindGuidesParams {
    /// Base class: Marauder, Ranger, Witch, Duelist, Templar, Shadow or Scion.
    pub class: String,
    /// Narrows to one ascendancy, e.g. Pathfinder.
    pub ascendancy: Option<String>,
    /// Words the guide's title must have, e.g. "Poison Concoction".
    pub skill: Option<String>,
    /// Include guides from older leagues. Off by default.
    pub any_league: Option<bool>,
    /// Default 10.
    pub limit: Option<u32>,
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
    guides: Arc<exile_guides::Guides>,
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
        let guides = Arc::new(exile_guides::Guides::new(
            USER_AGENT,
            paths::cache_dir().map(|d| d.join("guides")),
        ));
        Self {
            session: Arc::new(Mutex::new(Session::new(opts, guides.clone()))),
            wiki: Arc::new(Wiki::new(USER_AGENT, cache)),
            guides,
        }
    }

    #[tool(
        description = "Start here. Where the player is in the game, read from the game's log: character, class, level, act, area, deaths and how long ago the log changed. Also the guide they follow with its stage, and which build the tools read. Fast, doesn't start Path of Building."
    )]
    async fn player_status(&self) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || Ok(session.lock().unwrap().status()))
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
    }

    #[tool(
        description = "Saves the guide the player follows, and the stage they're at, so later conversations know them. Without a guide, changes only the stage of the followed one."
    )]
    async fn follow_guide(
        &self,
        Parameters(p): Parameters<FollowParams>,
    ) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || {
            let stage = exile_core::followed_guide::Stage {
                tree: p.tree,
                item_set: p.item_set,
                skill_set: p.skill_set,
            };
            session.lock().unwrap().follow(p.guide.as_deref(), stage)
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
    }

    #[tool(description = "The player stops following their guide.")]
    async fn forget_guide(&self) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || session.lock().unwrap().forget_guide())
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
    }

    #[tool(
        description = "Makes a build, usually the guide the player levels with, their build in Exile Companion. The overlay then shows its gems along the campaign route. The old build goes into the history, undo_change brings it back. Only with the player's OK."
    )]
    async fn use_as_my_build(
        &self,
        Parameters(p): Parameters<ReplaceParams>,
    ) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || {
            session.lock().unwrap().replace_build(&p.build, &p.reason)
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
    }

    #[tool(description = "Stops using a build from load_build and goes back to the player's own.")]
    async fn back_to_my_build(&self) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || session.lock().unwrap().back_to_mine())
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
    }

    #[tool(
        description = "The build's left sidebar in Path of Building: every stat row as PoB formats it, PoB's warnings, and the main skill."
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
            if !p.explore.unwrap_or(false) {
                // Without any guide every notable is fair game.
                if let Ok((guide, _)) = s.guide_or_followed(p.guide) {
                    params["reference"] = json!(s.reference(&guide)?);
                }
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
            let (guide, followed) = s.guide_or_followed(p.guide)?;
            let reference = s.reference(&guide)?;
            let given = p.tree.is_some() || p.item_set.is_some() || p.skill_set.is_some();
            let (tree, item_set, skill_set) = match followed {
                Some(st) if !given => (st.tree, st.item_set, st.skill_set),
                _ => (p.tree, p.item_set, p.skill_set),
            };
            if tree.is_some() || item_set.is_some() || skill_set.is_some() {
                s.call(
                    "set_reference_stage",
                    json!({ "reference": reference, "tree": tree, "item_set": item_set, "skill_set": skill_set }),
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
        description = "Keeps changes in the player's build for good, in the same form as what_if: the build in Exile Companion changes and the change goes into the history with its reason. Only when the player asked to keep it. Returns what it did to DPS, EHP and the stats PoB shows."
    )]
    async fn apply_changes(
        &self,
        Parameters(p): Parameters<ApplyParams>,
    ) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || {
            session.lock().unwrap().apply(json!(p.changes), &p.reason)
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
    }

    #[tool(
        description = "Changes kept in the player's build since it was imported, oldest first: when, why, the changes and what they did to DPS, EHP and life."
    )]
    async fn build_history(&self) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || session.lock().unwrap().history())
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
    }

    #[tool(
        description = "Takes the last kept change back and puts the player's build as it was before it. Call it again to go further back."
    )]
    async fn undo_change(&self) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || session.lock().unwrap().undo())
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
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
        description = "Switches the tools to another build, from a PoB code, a build link or XML, until back_to_my_build. The build in Exile Companion stays as it is."
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
        description = "A guide's stages as its author saved them: each passive tree in order with its points and the ascendancy notables it adds, and ascendancy_order as one group per tree (the lab order when the guide has a tree per lab; within a group the order is unknown), each item set's gear, each skill set's skills. Guides often have fewer item sets than trees, then an early tree comes with late gear. Works without the player's build."
    )]
    async fn guide_stages(
        &self,
        Parameters(p): Parameters<StagesParams>,
    ) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || {
            let mut s = session.lock().unwrap();
            let (guide, _) = s.guide_or_followed(p.guide)?;
            let reference = s.reference(&guide)?;
            s.call_any("guide_stages", json!({ "reference": reference }))
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
    }

    #[tool(
        description = "What changes between two stages of a guide: notables added and dropped, points, ascendancy notables, gear by slot, skills and supports, and every stat PoB shows for each stage. Stats are at the guide's one saved character level, so early stages read stronger than a real character at that point. Works without the player's build."
    )]
    async fn guide_stage_diff(
        &self,
        Parameters(p): Parameters<StageDiffParams>,
    ) -> Result<CallToolResult, McpError> {
        let session = self.session.clone();
        let r = tokio::task::spawn_blocking(move || {
            let mut s = session.lock().unwrap();
            let (guide, _) = s.guide_or_followed(p.guide)?;
            let reference = s.reference(&guide)?;
            let stage = |st: &Stage| json!({ "tree": st.tree, "item_set": st.item_set, "skill_set": st.skill_set });
            s.call_any(
                "guide_stage_diff",
                json!({ "reference": reference, "from": stage(&p.from), "to": stage(&p.to) }),
            )
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        Ok(json_result(r))
    }

    #[tool(
        description = "Build guides for a class or ascendancy from poebuilds.cc, a fan index of guides (mostly videos), most watched first, current league by default. Each has its league, views, author, video, guide page and, when it has one, a Path of Building link for load_build or compare_with_guide."
    )]
    async fn find_guides(
        &self,
        Parameters(p): Parameters<FindGuidesParams>,
    ) -> Result<CallToolResult, McpError> {
        let guides = self.guides.clone();
        let r = tokio::task::spawn_blocking(move || {
            let list = guides
                .list(&p.class, p.ascendancy.as_deref())
                .map_err(|e| e.to_string())?;
            let picked = exile_guides::pick(
                list,
                p.skill.as_deref(),
                p.any_league.unwrap_or(false),
                p.limit.unwrap_or(10) as usize,
            );
            serde_json::to_value(picked).map_err(|e| e.to_string())
        })
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
        log: None,
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
            "--log" => opts.log = Some(value()?),
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
            eprintln!("{e}\nusage: exile-companion-mcp [--pob <dir>] [--build <file.xml>] [--log <Client.txt>] [--engine <path>]");
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
