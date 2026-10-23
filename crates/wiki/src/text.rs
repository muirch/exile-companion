//! Cleaning what the API returns: search snippets carry HTML, Cargo values carry entities.

/// Drops tags and decodes entities: `<span class="searchmatch">Fossil</span>` becomes `Fossil`.
pub fn plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_entities(&out)
}

pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|&e| e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let decoded = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A page's plain text cut at its `== Heading ==` lines, as the extracts API writes them with
/// exsectionformat=wiki. The text before the first heading comes first, with an empty heading.
pub fn sections(text: &str) -> Vec<Section> {
    let mut out = vec![Section {
        heading: String::new(),
        level: 1,
        body: String::new(),
    }];
    for line in text.lines() {
        let t = line.trim();
        let level = t.chars().take_while(|&c| c == '=').count();
        if level >= 2 && t.len() > level * 2 && t.ends_with(&"=".repeat(level)) {
            out.push(Section {
                heading: t[level..t.len() - level].trim().to_string(),
                level,
                body: String::new(),
            });
            continue;
        }
        let body = &mut out.last_mut().unwrap().body;
        body.push_str(line);
        body.push('\n');
    }
    for s in &mut out {
        s.body = s.body.trim().to_string();
    }
    out.retain(|s| !(s.heading.is_empty() && s.body.is_empty()));
    out
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Section {
    pub heading: String,
    pub level: usize,
    pub body: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_search_highlights() {
        assert_eq!(
            plain(r#"craft with <span class="searchmatch">fossils</span> &amp; resonators"#),
            "craft with fossils & resonators"
        );
    }

    #[test]
    fn decodes_named_and_numeric_entities() {
        assert_eq!(
            decode_entities("Admiral&#039;s Arrogance"),
            "Admiral's Arrogance"
        );
        assert_eq!(
            decode_entities("a &lt;b&gt; &#x2014; &quot;c&quot;"),
            "a <b> \u{2014} \"c\""
        );
        assert_eq!(decode_entities("Q&A &unknown; end"), "Q&A &unknown; end");
    }

    #[test]
    fn splits_a_page_at_its_headings() {
        let text = "Withered is a debuff.\n\n== Mechanics ==\nStacks 15 times.\n\n=== Sources ===\nWithering Step\n\n== Version history ==\n3.9.0 added.";
        let s = sections(text);
        let headings: Vec<_> = s.iter().map(|s| (s.heading.as_str(), s.level)).collect();
        assert_eq!(
            headings,
            [
                ("", 1),
                ("Mechanics", 2),
                ("Sources", 3),
                ("Version history", 2)
            ]
        );
        assert_eq!(s[0].body, "Withered is a debuff.");
        assert_eq!(s[2].body, "Withering Step");
    }
}
