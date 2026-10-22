//! Build links from sites that host PoB codes. The list and download URLs follow Path of
//! Building's own `src/Modules/BuildSiteTools.lua`.

/// The URL that returns the raw import code behind a build link, or `None` if `text` isn't a link
/// to a known build site.
pub fn code_url(text: &str) -> Option<String> {
    let text = text.trim();
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
        .unwrap_or(text);
    let rest = rest.split(['?', '#']).next()?;
    let (host, path) = rest.split_once('/')?;
    let host = host
        .strip_prefix("www.")
        .unwrap_or(host)
        .to_ascii_lowercase();
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

    let url = match (host.as_str(), parts.as_slice()) {
        ("pobb.in", [id]) => format!("https://pobb.in/pob/{}", id_of(id)?),
        ("poe.ninja", ["pob", id] | ["poe1", "pob", id]) => {
            format!("https://poe.ninja/poe1/pob/raw/{}", id_of(id)?)
        }
        ("maxroll.gg", ["poe", "pob", id]) => {
            format!("https://maxroll.gg/poe/api/pob/{}", id_of(id)?)
        }
        ("pob.codes", ["b", id]) => format!("https://api.pob.codes/{}/raw", id_of(id)?),
        ("poedb.tw", ["pob", id]) => format!("https://poedb.tw/pob/{}/raw", id_of(id)?),
        ("pastebin.com", [id]) => format!("https://pastebin.com/raw/{}", id_of(id)?),
        ("pastebinp.com", [id]) => format!("https://pastebinp.com/raw/{}", id_of(id)?),
        ("rentry.co", [id]) => format!("https://rentry.co/paste/{}/raw", id_of(id)?),
        _ => return None,
    };
    Some(url)
}

fn id_of(id: &str) -> Option<&str> {
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        .then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_sites_map_to_their_raw_code() {
        let cases = [
            (
                "https://pobb.in/NlUa8ZnB3YGK",
                "https://pobb.in/pob/NlUa8ZnB3YGK",
            ),
            (
                "https://poe.ninja/pob/a1b2",
                "https://poe.ninja/poe1/pob/raw/a1b2",
            ),
            (
                "https://poe.ninja/poe1/pob/a1b2",
                "https://poe.ninja/poe1/pob/raw/a1b2",
            ),
            (
                "https://maxroll.gg/poe/pob/xy12ab",
                "https://maxroll.gg/poe/api/pob/xy12ab",
            ),
            ("https://pob.codes/b/abc", "https://api.pob.codes/abc/raw"),
            ("https://poedb.tw/pob/abc", "https://poedb.tw/pob/abc/raw"),
            (
                "https://pastebin.com/Ab12Cd",
                "https://pastebin.com/raw/Ab12Cd",
            ),
            ("https://rentry.co/abc", "https://rentry.co/paste/abc/raw"),
        ];
        for (link, raw) in cases {
            assert_eq!(code_url(link).as_deref(), Some(raw), "{link}");
        }
    }

    #[test]
    fn copied_links_are_tidied_up() {
        let want = Some("https://pobb.in/pob/NlUa8ZnB3YGK".to_string());
        assert_eq!(code_url("  pobb.in/NlUa8ZnB3YGK/\n"), want);
        assert_eq!(code_url("https://www.pobb.in/NlUa8ZnB3YGK?x=1#top"), want);
        assert_eq!(code_url("http://POBB.IN/NlUa8ZnB3YGK"), want);
    }

    #[test]
    fn codes_and_other_links_are_not_build_links() {
        assert_eq!(code_url("eNrtvXl32ziTPvr3-FPwze-8c7p"), None);
        assert_eq!(code_url("<PathOfBuilding>"), None);
        assert_eq!(code_url("https://example.com/NlUa8ZnB3YGK"), None);
        assert_eq!(code_url("https://pobb.in/"), None);
        assert_eq!(code_url("https://pobb.in/a/b"), None);
        assert_eq!(code_url("https://pobb.in/abc%2F..%2Fx"), None);
    }
}
