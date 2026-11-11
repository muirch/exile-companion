//! The items a build carries, by name, to tell whether two builds have the same gear.

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::error::PobError;

/// "Name, Base" for rares and uniques, the one name line for the rest, sorted.
pub fn item_names(xml: &[u8]) -> Result<Vec<String>, PobError> {
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut names = Vec::new();
    let mut text: Option<String> = None;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) if e.local_name().as_ref() == b"Item" => text = Some(String::new()),
            Event::Text(t) => {
                if let Some(text) = text.as_mut() {
                    text.push_str(&t.xml10_content().map_err(quick_xml::Error::from)?);
                }
            }
            Event::End(e) if e.local_name().as_ref() == b"Item" => {
                if let Some(name) = text.take().as_deref().and_then(item_name) {
                    names.push(name);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    names.sort();
    Ok(names)
}

fn item_name(text: &str) -> Option<String> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let rarity = lines.find_map(|l| l.strip_prefix("Rarity:"))?.trim();
    let name = lines.next()?;
    if rarity.eq_ignore_ascii_case("rare") || rarity.eq_ignore_ascii_case("unique") {
        let base = lines.next()?;
        return Some(format!("{name}, {base}"));
    }
    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = "<PathOfBuilding><Items activeItemSet=\"1\">
        <Item id=\"2\">
            Rarity: RARE
Death Song
Short Bow
Quality: 0
            <ModRange id=\"1\" range=\"0.5\"/>
        </Item>
        <Item id=\"1\">Rarity: MAGIC
Seething Divine Life Flask of Alleviation
</Item>
        <Item id=\"3\">Rarity: UNIQUE
Goldrim
Leather Cap</Item>
        <ItemSet id=\"1\"><Slot name=\"Weapon 1\" itemId=\"2\"/></ItemSet>
    </Items></PathOfBuilding>";

    #[test]
    fn names_items_the_way_the_game_does() {
        assert_eq!(
            item_names(XML.as_bytes()).unwrap(),
            [
                "Death Song, Short Bow",
                "Goldrim, Leather Cap",
                "Seething Divine Life Flask of Alleviation"
            ]
        );
    }

    #[test]
    fn a_build_without_items_has_none() {
        let names = item_names(b"<PathOfBuilding><Build/></PathOfBuilding>").unwrap();
        assert!(names.is_empty());
    }
}
