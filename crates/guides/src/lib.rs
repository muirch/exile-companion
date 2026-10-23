//! Build guides from poebuilds.cc, a fan index of Path of Exile guides (mostly videos) by class,
//! ascendancy and league, with view counts and links to the guide's PoB. We read the page a
//! browser would, one per question, and keep it for a day. robots.txt allows these pages, only
//! its /top and /new lists are closed to crawlers.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use exile_fetch::Fetcher;
use exile_wiki::text::decode_entities;
use regex::Regex;
use serde::Serialize;

pub const SITE: &str = "https://www.poebuilds.cc";
pub const CLASSES: [&str; 7] = [
    "marauder", "ranger", "witch", "duelist", "templar", "shadow", "scion",
];

const CACHE_TTL: Duration = Duration::from_secs(24 * 3600);
const SPACING: Duration = Duration::from_secs(1);

#[derive(Debug, thiserror::Error)]
pub enum GuideError {
    #[error("{0} isn't a Path of Exile 1 class")]
    UnknownClass(String),
    #[error("couldn't reach poebuilds.cc: {0}")]
    Http(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    pub site: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Guide {
    pub title: String,
    pub author: Option<String>,
    pub class: String,
    /// None when the guide is for the base class.
    pub ascendancy: Option<String>,
    pub views: u64,
    pub published: String,
    /// As the site shows it, e.g. "3.29 Allflame".
    pub league: String,
    pub current_league: bool,
    pub tags: Vec<String>,
    pub video: Option<String>,
    pub page: Option<String>,
    /// A link load_build takes (pobb.in or poe.ninja's PoB page), if the guide has one.
    pub pob: Option<String>,
    pub links: Vec<Link>,
}

pub struct Guides {
    fetch: Fetcher,
}

impl Guides {
    pub fn new(user_agent: &str, cache: Option<PathBuf>) -> Self {
        Self {
            fetch: Fetcher::new(user_agent, cache, CACHE_TTL, SPACING),
        }
    }

    /// Guides for a class, or one of its ascendancies, as poebuilds.cc lists them.
    pub fn list(&self, class: &str, ascendancy: Option<&str>) -> Result<Vec<Guide>, GuideError> {
        let class = slug(class);
        if !CLASSES.contains(&class.as_str()) {
            return Err(GuideError::UnknownClass(class));
        }
        let url = match ascendancy.map(slug).filter(|a| !a.is_empty()) {
            Some(a) => format!("{SITE}/poe1/{class}/{a}"),
            None => format!("{SITE}/poe1/{class}"),
        };
        let html = self
            .fetch
            .get(&url, &[])
            .map_err(|e| GuideError::Http(e.to_string()))?;
        Ok(parse_cards(&html))
    }
}

fn slug(s: &str) -> String {
    s.trim().to_lowercase().replace(' ', "-")
}

/// The most watched guides, of the current league unless `any_league`, whose title or tags
/// mention `skill` when one is given.
pub fn pick(
    mut guides: Vec<Guide>,
    skill: Option<&str>,
    any_league: bool,
    limit: usize,
) -> Vec<Guide> {
    let words: Vec<String> = skill
        .map(|s| {
            s.to_lowercase()
                .split_whitespace()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    guides.retain(|g| {
        let text = format!("{} {}", g.title, g.tags.join(" ")).to_lowercase();
        (any_league || g.current_league) && words.iter().all(|w| text.contains(w.as_str()))
    });
    guides.sort_by_key(|g| std::cmp::Reverse(g.views));
    // The class page shows a guide once per section it fits, the same video can come twice.
    let mut seen = std::collections::HashSet::new();
    guides.retain(|g| seen.insert(g.video.clone().unwrap_or_else(|| g.title.clone())));
    guides.truncate(limit);
    guides
}

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("a valid pattern"))
}

pub fn parse_cards(html: &str) -> Vec<Guide> {
    static ATTR: OnceLock<Regex> = OnceLock::new();
    static PORTRAIT: OnceLock<Regex> = OnceLock::new();
    static TITLE: OnceLock<Regex> = OnceLock::new();
    static VERSION: OnceLock<Regex> = OnceLock::new();
    static TAG: OnceLock<Regex> = OnceLock::new();
    static AUTHOR: OnceLock<Regex> = OnceLock::new();
    static EXT: OnceLock<Regex> = OnceLock::new();
    static PAGE: OnceLock<Regex> = OnceLock::new();
    let attr = re(&ATTR, r#"data-([a-z-]+)="([^"]*)""#);
    let portrait = re(&PORTRAIT, r#"build-card-portrait"><img [^>]*alt="([^"]*)""#);
    let title = re(
        &TITLE,
        r#"<a href="([^"]*)"[^>]*class="build-card-title"[^>]*title="([^"]*)""#,
    );
    let version = re(
        &VERSION,
        r#"build-card-tag--version( build-card-tag--version-current)?" title="([^"]*)""#,
    );
    let tag = re(&TAG, r#"<span class="build-card-tag">([^<]+)</span>"#);
    let author = re(&AUTHOR, r#"build-card-tag--author">([^<]+)</a>"#);
    let ext = re(
        &EXT,
        r#"<a href="([^"]+)"[^>]*class="build-card-tag build-card-tag--ext build-card-tag--([a-z]+)""#,
    );
    let page = re(&PAGE, r#"href="(/poe1/build/[^"]+)""#);

    html.split(r#"<div class="build-card" "#)
        .skip(1)
        .filter_map(|card| {
            // A card's attributes come first, in its opening tag.
            let head = &card[..card.find('>')?];
            let mut class = String::new();
            let mut views = 0;
            let mut published = String::new();
            for c in attr.captures_iter(head) {
                match &c[1] {
                    "class-slug" => class = c[2].to_string(),
                    "views" => views = c[2].parse().unwrap_or(0),
                    "published" => published = c[2].to_string(),
                    _ => {}
                }
            }
            let t = title.captures(card)?;
            let v = version.captures(card);
            let pictured = portrait.captures(card).map(|c| decode_entities(&c[1]));
            let links: Vec<Link> = ext
                .captures_iter(card)
                .map(|c| Link {
                    site: c[2].to_string(),
                    url: decode_entities(&c[1]),
                })
                .collect();
            let pob = links
                .iter()
                .find(|l| l.site == "pobbin" || (l.site == "poeninja" && l.url.contains("/pob/")))
                .map(|l| l.url.clone());
            let video = Some(decode_entities(&t[1]))
                .filter(|u| u.contains("youtube.com") || u.contains("youtu.be"));
            Some(Guide {
                title: decode_entities(&t[2]),
                author: author.captures(card).map(|c| decode_entities(c[1].trim())),
                ascendancy: pictured.filter(|a| !a.eq_ignore_ascii_case(&class)),
                class,
                views,
                published,
                league: v.as_ref().map(|v| v[2].to_string()).unwrap_or_default(),
                current_league: v.is_some_and(|v| v.get(1).is_some()),
                tags: tag
                    .captures_iter(card)
                    .map(|c| decode_entities(c[1].trim()))
                    .collect(),
                video,
                page: page.captures(card).map(|c| format!("{SITE}{}", &c[1])),
                pob,
                links,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two cards in the site's markup, with made-up guides.
    const PAGE: &str = r#"<section><div class="build-group-wrap"><div class="build-card" data-class-slug="ranger" data-views="9699" data-published="2026-07-21 05:52:28"><div class="build-card-portrait"><img src="/img/portraits/poe1-pathfinder.webp" alt="Pathfinder" width="130"></div><div class="build-card-body"><a href="https://www.youtube.com/watch?v=abc" target="_blank" class="build-card-title" rel="noopener" title="Poison Concoction Pathfinder &amp; Friends - League Starter">Poison Concoction Pathfinder</a><div class="build-card-tags-line"><span class="build-card-tag build-card-tag--metric" title="9,699 views">9.7k</span><span class="build-card-tag build-card-tag--version build-card-tag--version-current" title="3.29 Allflame">3.29 Allflame</span><span class="build-card-tag build-card-tag--class" data-class-slug="ranger">Ranger</span><span class="build-card-tag">Projectile</span></div><div class="build-card-tags-line"><a href="/poe1/author/someone" class="build-card-tag build-card-tag--author">Someone</a><a href="https://pobb.in/AbC_123" target="_blank" rel="noopener" class="build-card-tag build-card-tag--ext build-card-tag--pobbin">pobb.in</a><a href="https://mobalytics.gg/poe/builds/x" target="_blank" class="build-card-tag build-card-tag--ext build-card-tag--mobalytics">Mobalytics</a><a href="/poe1/build/abc" class="build-card-tag build-card-tag--page">Details</a></div></div></div></div>
<div class="build-group-wrap"><div class="build-card" data-class-slug="ranger" data-views="120000" data-published="2026-01-02 10:00:00"><div class="build-card-portrait"><img src="/img/portraits/poe1-ranger.webp" alt="Ranger"></div><div class="build-card-body"><a href="https://www.youtube.com/watch?v=def" class="build-card-title" title="Old Toxic Rain guide">Old</a><div class="build-card-tags-line"><span class="build-card-tag build-card-tag--version" title="3.27 Keepers">3.27 Keepers</span><span class="build-card-tag">Projectile</span></div><div class="build-card-tags-line"><a href="https://poe.ninja/poe1/pob/12ab" target="_blank" class="build-card-tag build-card-tag--ext build-card-tag--poeninja">poe.ninja</a></div></div></div></div></section>"#;

    #[test]
    fn reads_each_guide_card() {
        let guides = parse_cards(PAGE);
        assert_eq!(guides.len(), 2);
        let g = &guides[0];
        assert_eq!(
            g.title,
            "Poison Concoction Pathfinder & Friends - League Starter"
        );
        assert_eq!(g.author.as_deref(), Some("Someone"));
        assert_eq!(g.class, "ranger");
        assert_eq!(g.ascendancy.as_deref(), Some("Pathfinder"));
        assert_eq!(g.views, 9699);
        assert_eq!(g.league, "3.29 Allflame");
        assert!(g.current_league);
        assert_eq!(g.tags, ["Projectile"]);
        assert_eq!(g.pob.as_deref(), Some("https://pobb.in/AbC_123"));
        assert_eq!(
            g.video.as_deref(),
            Some("https://www.youtube.com/watch?v=abc")
        );
        assert_eq!(
            g.page.as_deref(),
            Some("https://www.poebuilds.cc/poe1/build/abc")
        );
        assert_eq!(g.links.len(), 2);

        let old = &guides[1];
        assert_eq!(old.ascendancy, None);
        assert!(!old.current_league);
        assert_eq!(old.pob.as_deref(), Some("https://poe.ninja/poe1/pob/12ab"));
    }

    #[test]
    fn picks_the_current_league_by_views_and_skill() {
        let guides = parse_cards(PAGE);
        let current = pick(guides.clone(), None, false, 10);
        assert_eq!(current.len(), 1);
        let all = pick(guides.clone(), None, true, 10);
        assert_eq!(all[0].title, "Old Toxic Rain guide", "most watched first");
        assert_eq!(
            pick(guides.clone(), Some("poison concoction"), true, 10).len(),
            1
        );
        assert_eq!(pick(guides.clone(), Some("cyclone"), true, 10).len(), 0);
        let twice = [guides.clone(), guides].concat();
        assert_eq!(pick(twice, None, true, 10).len(), 2);
    }

    #[test]
    fn refuses_a_class_that_is_not_one() {
        let g = Guides::new("test", None);
        assert!(matches!(
            g.list("Warrior", None),
            Err(GuideError::UnknownClass(_))
        ));
    }

    // Reads poebuilds.cc, so it only runs by hand: cargo test -p exile-guides -- --ignored
    #[test]
    #[ignore]
    fn reads_the_live_site() {
        let dir = std::env::temp_dir().join("exile-guides-test");
        let g = Guides::new(
            "exile-companion-tests (https://github.com/muirch/exile-companion)",
            Some(dir),
        );
        let list = g.list("Ranger", Some("Pathfinder")).unwrap();
        assert!(list.len() > 5, "{}", list.len());
        assert!(list.iter().any(|g| g.pob.is_some()));
    }
}
