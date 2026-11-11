//! The player's character from pathofexile.com, through the character-window API by account name,
//! the way Path of Building's Import tab reads it. Works for public profiles only.

use std::time::Duration;

use serde_json::Value;

const HOST: &str = "https://www.pathofexile.com/character-window";

/// One character from get-characters: name, class, level, league and the rest PoB's import reads.
pub type Entry = Value;

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn characters_url(account: &str, realm: &str) -> String {
    format!(
        "{HOST}/get-characters?accountName={}&realm={}",
        encode(account),
        encode(realm)
    )
}

pub fn passives_url(account: &str, character: &str, realm: &str) -> String {
    format!(
        "{HOST}/get-passive-skills?accountName={}&character={}&realm={}",
        encode(account),
        encode(character),
        encode(realm)
    )
}

pub fn items_url(account: &str, character: &str, realm: &str) -> String {
    format!(
        "{HOST}/get-items?accountName={}&character={}&realm={}",
        encode(account),
        encode(character),
        encode(realm)
    )
}

/// What went wrong, in the player's words.
pub fn status_message(status: u16, account: &str) -> String {
    match status {
        403 => format!(
            "pathofexile.com keeps {account}'s characters private. Turn off Hide characters in the profile's privacy settings, or import the character in Path of Building."
        ),
        404 => format!(
            "pathofexile.com has no account {account}. Use the name from your profile page, with the #1234 at the end."
        ),
        429 => "pathofexile.com asks to slow down. Try again in a minute.".into(),
        _ => format!("pathofexile.com answered with status {status}. Try again later."),
    }
}

fn get(url: &str, account: &str) -> Result<Value, String> {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(crate::USER_AGENT)
        .build()
        .into();
    let body = match agent.get(url).call() {
        Ok(mut r) => r
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Couldn't read pathofexile.com's answer: {e}"))?,
        Err(ureq::Error::StatusCode(status)) => return Err(status_message(status, account)),
        Err(e) => return Err(format!("Couldn't reach pathofexile.com: {e}")),
    };
    parse_body(&body, account)
}

/// The API answers `false` for a profile it won't show.
fn parse_body(body: &str, account: &str) -> Result<Value, String> {
    if body.trim() == "false" {
        return Err(status_message(403, account));
    }
    serde_json::from_str(body)
        .map_err(|e| format!("pathofexile.com answered something unreadable ({e})."))
}

pub fn characters(account: &str, realm: &str) -> Result<Vec<Entry>, String> {
    match get(&characters_url(account, realm), account)? {
        Value::Array(list) => Ok(list),
        _ => Err("pathofexile.com didn't list any characters.".into()),
    }
}

/// The character's entry, by name ignoring case, or what names there are.
pub fn pick<'a>(list: &'a [Entry], name: &str) -> Result<&'a Entry, String> {
    list.iter()
        .find(|c| {
            c["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
        .ok_or_else(|| {
            let names: Vec<&str> = list.iter().filter_map(|c| c["name"].as_str()).collect();
            format!(
                "The account has no character {name}. It has {}.",
                if names.is_empty() {
                    "none".to_string()
                } else {
                    names.join(", ")
                }
            )
        })
}

/// The tree and the gear, as PoB's import takes them.
pub fn passives_and_items(
    account: &str,
    character: &str,
    realm: &str,
) -> Result<(Value, Value), String> {
    let passives = get(&passives_url(account, character, realm), account)?;
    let items = get(&items_url(account, character, realm), account)?;
    Ok((passives, items))
}

/// Name, class, level and league of each character, for listing.
pub fn outline(list: &[Entry]) -> Vec<Value> {
    list.iter()
        .map(|c| {
            serde_json::json!({
                "name": c["name"],
                "class": c["class"],
                "level": c["level"],
                "league": c["league"],
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn urls_encode_the_account_tag_and_names() {
        assert_eq!(
            characters_url("Exile#1234", "pc"),
            "https://www.pathofexile.com/character-window/get-characters?accountName=Exile%231234&realm=pc"
        );
        assert!(items_url("Exile#1234", "Big Bonk", "pc").contains("character=Big%20Bonk"));
        assert!(passives_url("Exile#1234", "RealOne", "xbox").ends_with("&realm=xbox"));
    }

    #[test]
    fn says_what_went_wrong() {
        assert!(status_message(403, "Exile#1").contains("private"));
        assert!(status_message(404, "Exile").contains("#1234"));
        assert!(status_message(429, "Exile#1").contains("slow down"));
        assert!(parse_body("false", "Exile#1")
            .unwrap_err()
            .contains("private"));
        assert_eq!(parse_body("[]", "Exile#1").unwrap(), json!([]));
    }

    #[test]
    fn picks_a_character_by_name_ignoring_case() {
        let list = vec![
            json!({"name": "MuleGuy", "class": "Ranger", "level": 4, "league": "Standard"}),
            json!({"name": "RealOne", "class": "Guardian", "level": 30, "league": "Standard"}),
        ];
        assert_eq!(pick(&list, "realone").unwrap()["class"], "Guardian");
        let err = pick(&list, "Nobody").unwrap_err();
        assert!(err.contains("MuleGuy, RealOne"), "{err}");
        assert_eq!(outline(&list)[1]["level"], 30);
    }
}
