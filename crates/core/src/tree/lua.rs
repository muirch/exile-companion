//! Reads the Lua table literal PoB keeps the passive tree in (`TreeData/<version>/tree.lua`) as
//! JSON. Only what a data file needs: tables, strings, numbers, booleans, nil and `--` comments.
//! A table with only positional entries becomes an array, any other table an object whose keys are
//! strings (`[123]=` gives "123").

use serde_json::{Map, Number, Value};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("tree.lua: {what} at byte {at}")]
pub struct LuaError {
    pub what: &'static str,
    pub at: usize,
}

pub fn parse(src: &str) -> Result<Value, LuaError> {
    let mut p = Parser {
        s: src.as_bytes(),
        i: 0,
    };
    p.skip();
    if p.word("return") {
        p.skip();
    }
    let v = p.value()?;
    p.skip();
    if p.i < p.s.len() {
        return Err(p.err("text after the table"));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn err(&self, what: &'static str) -> LuaError {
        LuaError { what, at: self.i }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn skip(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                self.i += 1;
            } else if self.s[self.i..].starts_with(b"--") {
                while self.peek().is_some_and(|c| c != b'\n') {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
    }

    fn word(&mut self, w: &str) -> bool {
        let end = self.i + w.len();
        let follows_ident = self
            .s
            .get(end)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_');
        if self.s[self.i..].starts_with(w.as_bytes()) && !follows_ident {
            self.i = end;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: u8, what: &'static str) -> Result<(), LuaError> {
        self.skip();
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err(what))
        }
    }

    fn value(&mut self) -> Result<Value, LuaError> {
        self.skip();
        match self.peek() {
            Some(b'{') => self.table(),
            Some(b'"' | b'\'') => self.string().map(Value::String),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            _ if self.word("true") => Ok(Value::Bool(true)),
            _ if self.word("false") => Ok(Value::Bool(false)),
            _ if self.word("nil") => Ok(Value::Null),
            _ => Err(self.err("expected a value")),
        }
    }

    fn table(&mut self) -> Result<Value, LuaError> {
        self.i += 1;
        let mut list = Vec::new();
        let mut map = Map::new();
        loop {
            self.skip();
            match self.peek() {
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                Some(b'[') => {
                    self.i += 1;
                    let key = match self.value()? {
                        Value::String(s) => s,
                        Value::Number(n) => n.to_string(),
                        _ => return Err(self.err("table key must be a string or number")),
                    };
                    self.expect(b']', "expected ]")?;
                    self.expect(b'=', "expected =")?;
                    let v = self.value()?;
                    map.insert(key, v);
                }
                Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                    let start = self.i;
                    while self
                        .peek()
                        .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
                    {
                        self.i += 1;
                    }
                    let name = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
                    self.skip();
                    if self.peek() == Some(b'=') {
                        self.i += 1;
                        let v = self.value()?;
                        map.insert(name, v);
                    } else {
                        self.i = start;
                        list.push(self.value()?);
                    }
                }
                None => return Err(self.err("unclosed table")),
                _ => list.push(self.value()?),
            }
            self.skip();
            match self.peek() {
                Some(b',' | b';') => self.i += 1,
                Some(b'}') => {}
                None => return Err(self.err("unclosed table")),
                _ => return Err(self.err("expected , or }")),
            }
        }
        if map.is_empty() {
            return Ok(Value::Array(list));
        }
        for (n, v) in list.into_iter().enumerate() {
            map.insert((n + 1).to_string(), v);
        }
        Ok(Value::Object(map))
    }

    fn string(&mut self) -> Result<String, LuaError> {
        let quote = self.s[self.i];
        self.i += 1;
        let mut out = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err("unclosed string"));
            };
            self.i += 1;
            match c {
                _ if c == quote => break,
                b'\\' => {
                    let Some(e) = self.peek() else {
                        return Err(self.err("unclosed string"));
                    };
                    self.i += 1;
                    match e {
                        b'n' | b'\n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'0'..=b'9' => {
                            let start = self.i - 1;
                            while self.i < start + 3
                                && self.peek().is_some_and(|c| c.is_ascii_digit())
                            {
                                self.i += 1;
                            }
                            let code: u32 = std::str::from_utf8(&self.s[start..self.i])
                                .ok()
                                .and_then(|d| d.parse().ok())
                                .unwrap_or(0);
                            out.push(code.min(255) as u8);
                        }
                        other => out.push(other),
                    }
                }
                _ => out.push(c),
            }
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }

    fn number(&mut self) -> Result<Value, LuaError> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-'))
        {
            self.i += 1;
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
        if let Ok(n) = text.parse::<i64>() {
            return Ok(Value::Number(n.into()));
        }
        text.parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map(Value::Number)
            .ok_or(LuaError {
                what: "bad number",
                at: start,
            })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_nested_tables_arrays_and_scalars() {
        let src = r#"-- generated
return {
    ["tree"]= "Default",
    ["points"]= { ["total"]= 123, ratio = 0.5 },
    ["list"]= { "a", "b", },
    [42]= { true, false, nil, -14159 },
    ["empty"]= {},
}"#;
        assert_eq!(
            parse(src).unwrap(),
            json!({
                "tree": "Default",
                "points": {"total": 123, "ratio": 0.5},
                "list": ["a", "b"],
                "42": [true, false, null, -14159],
                "empty": [],
            })
        );
    }

    #[test]
    fn reads_escapes_in_strings() {
        let v = parse(
            r#"{ "a \"b\"\n\\c", 'it\'s', "line\
two", "\65" }"#,
        )
        .unwrap();
        assert_eq!(v, json!(["a \"b\"\n\\c", "it's", "line\ntwo", "A"]));
    }

    #[test]
    fn mixed_tables_number_their_list_entries() {
        assert_eq!(
            parse(r#"{ "x", ["k"]= 1 }"#).unwrap(),
            json!({"k": 1, "1": "x"})
        );
    }

    #[test]
    fn says_where_it_broke() {
        assert_eq!(parse("{ [\"a\"]= 1 ").unwrap_err().what, "unclosed table");
        assert_eq!(parse("{ 1 2 }").unwrap_err().what, "expected , or }");
    }
}
