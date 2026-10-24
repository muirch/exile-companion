//! Tool answers stay in the model's context for the rest of the conversation and are paid for
//! again on every request, so they carry no more digits or markup than an answer needs.

use serde_json::{Number, Value};

/// Rounds fractions to what a player reads (-75.6, 1.53, 0.004) and, with `strip_codes`, drops Path of
/// Building's colour codes (^4, ^xFFAA00) from text. Wiki text keeps its carets.
pub fn compact(v: Value, strip_codes: bool) -> Value {
    match v {
        Value::Number(n) => Value::Number(round(n)),
        Value::String(s) if strip_codes => Value::String(strip(&s)),
        Value::Array(a) => Value::Array(a.into_iter().map(|x| compact(x, strip_codes)).collect()),
        Value::Object(o) => Value::Object(
            o.into_iter()
                .map(|(k, x)| (k, compact(x, strip_codes)))
                .collect(),
        ),
        other => other,
    }
}

fn round(n: Number) -> Number {
    let Some(f) = n.as_f64().filter(|_| n.is_f64()) else {
        return n;
    };
    let places = match f.abs() {
        a if a >= 100.0 => 0,
        a if a >= 10.0 => 1,
        a if a >= 1.0 => 2,
        _ => 3,
    };
    let scale = 10f64.powi(places);
    let r = (f * scale).round() / scale;
    if r.fract() == 0.0 && r.abs() < 1e15 {
        Number::from(r as i64)
    } else {
        Number::from_f64(r).unwrap_or(n)
    }
}

fn strip(s: &str) -> String {
    if !s.contains('^') && !s.contains("  ") {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '^' {
            if chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()) {
                i += 2;
                continue;
            }
            if chars.get(i + 1) == Some(&'x')
                && chars.len() >= i + 8
                && chars[i + 2..i + 8].iter().all(|h| h.is_ascii_hexdigit())
            {
                i += 8;
                continue;
            }
        }
        if c == ' ' && out.ends_with(' ') {
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn rounds_fractions_and_leaves_whole_numbers() {
        let v = compact(
            json!({"dps_pct": -75.62893182401743, "ehp_pct": -1.6192264059055776, "hit": 1209, "x": 0.004, "big": 123456.7, "id": 31961}),
            true,
        );
        assert_eq!(
            v,
            json!({"dps_pct": -75.6, "ehp_pct": -1.62, "hit": 1209, "x": 0.004, "big": 123457, "id": 31961})
        );
    }

    #[test]
    fn drops_pob_colour_codes() {
        let v = compact(
            json!([
                "^4Level 1-30 {1}",
                "Lvl 90 ^2Going Block Based ^7{6}",
                "^xFFAA00Red^7 text",
                "Lvl 41-60  {3}"
            ]),
            true,
        );
        assert_eq!(
            v,
            json!([
                "Level 1-30 {1}",
                "Lvl 90 Going Block Based {6}",
                "Red text",
                "Lvl 41-60 {3}"
            ])
        );
    }

    #[test]
    fn wiki_text_keeps_its_carets() {
        assert_eq!(compact(json!("x^2 + y"), false), json!("x^2 + y"));
    }
}
