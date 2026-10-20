// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A strict JSON reader and the writer of the group mapping.
//!
//! The mapping needs three things a general-purpose JSON library does not give:
//! a number kept as the **text it was written as** (so a 30-digit amount is
//! never narrowed to a binary float), **duplicate object keys reported** rather
//! than silently resolved, and a **bounded depth and size** so a hostile
//! payload cannot exhaust the stack or the heap. RFC 8259, nothing more: no
//! comments, no trailing commas, no `NaN`, no leading zeros, no lone
//! surrogates.

/// A parsed JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    /// The number exactly as written, e.g. `-12.50` or `1e3`.
    Number(String),
    String(String),
    Array(Vec<Json>),
    /// Members in document order. Duplicates are rejected by the parser.
    Object(Vec<(String, Json)>),
}

/// Why a text is not acceptable JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    /// Byte offset of the fault.
    pub at: usize,
    pub message: String,
    /// Set when the fault is a repeated object key (the key).
    pub duplicate_key: Option<String>,
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid JSON at byte {}: {}", self.at, self.message)
    }
}

impl std::error::Error for JsonError {}

impl Json {
    /// The compact text of this value: the numbers exactly as they were read,
    /// strings by the rule of [`write_string`].
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Number(n) => out.push_str(n),
            Json::String(s) => write_string(out, s),
            Json::Array(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Json::Object(m) => {
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(out, k);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// The deepest nesting accepted. A group of groups of tables seldom exceeds ten.
pub const MAX_DEPTH: usize = 64;

/// Parse `text`. `max_bytes` bounds the input; larger is an error.
pub fn parse(text: &str, max_bytes: usize) -> Result<Json, JsonError> {
    if text.len() > max_bytes {
        return Err(JsonError { at: 0, message: format!("{} bytes is over the limit of {max_bytes}", text.len()), duplicate_key: None });
    }
    let mut p = Parser { b: text.as_bytes(), s: text, i: 0 };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.b.len() {
        return Err(p.fail("unexpected text after the value"));
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    s: &'a str,
    i: usize,
}

impl<'a> Parser<'a> {
    fn fail(&self, message: &str) -> JsonError {
        JsonError { at: self.i, message: message.to_string(), duplicate_key: None }
    }

    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.fail("nesting is deeper than the limit"));
        }
        match self.b.get(self.i) {
            None => Err(self.fail("unexpected end of text")),
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Json::String(self.string()?)),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'n') => self.word("null", Json::Null),
            Some(b'-') | Some(b'0'..=b'9') => self.number(),
            Some(_) => Err(self.fail("unexpected character")),
        }
    }

    fn word(&mut self, w: &str, v: Json) -> Result<Json, JsonError> {
        if self.s[self.i..].starts_with(w) {
            self.i += w.len();
            Ok(v)
        } else {
            Err(self.fail("unexpected word"))
        }
    }

    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.i;
        self.eat(b'-');
        match self.b.get(self.i) {
            Some(b'0') => {
                self.i += 1;
                if matches!(self.b.get(self.i), Some(b'0'..=b'9')) {
                    return Err(self.fail("a number has no leading zeros"));
                }
            }
            Some(b'1'..=b'9') => {
                while matches!(self.b.get(self.i), Some(b'0'..=b'9')) {
                    self.i += 1;
                }
            }
            _ => return Err(self.fail("a number needs a digit")),
        }
        if self.eat(b'.') {
            let fs = self.i;
            while matches!(self.b.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
            if fs == self.i {
                return Err(self.fail("a fraction needs a digit"));
            }
        }
        if matches!(self.b.get(self.i), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.b.get(self.i), Some(b'+' | b'-')) {
                self.i += 1;
            }
            let es = self.i;
            while matches!(self.b.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
            if es == self.i {
                return Err(self.fail("an exponent needs a digit"));
            }
        }
        Ok(Json::Number(self.s[start..self.i].to_string()))
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        if self.i + 4 > self.b.len() {
            return Err(self.fail("a \\u escape needs four hex digits"));
        }
        let mut v = 0u32;
        for k in 0..4 {
            let d = (self.b[self.i + k] as char).to_digit(16).ok_or_else(|| self.fail("a \\u escape needs four hex digits"))?;
            v = v * 16 + d;
        }
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.i += 1; // opening quote
        let mut out = String::new();
        loop {
            let Some(&c) = self.b.get(self.i) else {
                return Err(self.fail("unterminated string"));
            };
            match c {
                b'"' => {
                    self.i += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.i += 1;
                    let Some(&e) = self.b.get(self.i) else {
                        return Err(self.fail("unterminated escape"));
                    };
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xD800..0xDC00).contains(&hi) {
                                if !(self.eat(b'\\') && self.eat(b'u')) {
                                    return Err(self.fail("a high surrogate needs its low surrogate"));
                                }
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(self.fail("a high surrogate needs its low surrogate"));
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else if (0xDC00..0xE000).contains(&hi) {
                                return Err(self.fail("a lone low surrogate"));
                            } else {
                                hi
                            };
                            out.push(char::from_u32(cp).ok_or_else(|| self.fail("not a character"))?);
                        }
                        _ => return Err(self.fail("unknown escape")),
                    }
                }
                0x00..=0x1F => return Err(self.fail("a control character in a string must be escaped")),
                _ => {
                    // Copy one whole UTF-8 character (the input is a &str, so valid).
                    let ch_len = utf8_len(c);
                    out.push_str(&self.s[self.i..self.i + ch_len]);
                    self.i += ch_len;
                }
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.i += 1;
        let mut items = Vec::new();
        self.ws();
        if self.eat(b']') {
            return Ok(Json::Array(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth + 1)?);
            self.ws();
            if self.eat(b',') {
                continue;
            }
            if self.eat(b']') {
                return Ok(Json::Array(items));
            }
            return Err(self.fail("expected , or ]"));
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.i += 1;
        let mut members: Vec<(String, Json)> = Vec::new();
        self.ws();
        if self.eat(b'}') {
            return Ok(Json::Object(members));
        }
        loop {
            self.ws();
            if self.b.get(self.i) != Some(&b'"') {
                return Err(self.fail("an object key must be a string"));
            }
            let key_at = self.i;
            let key = self.string()?;
            if members.iter().any(|(k, _)| *k == key) {
                return Err(JsonError { at: key_at, message: format!("the key \"{key}\" is repeated"), duplicate_key: Some(key) });
            }
            self.ws();
            if !self.eat(b':') {
                return Err(self.fail("expected :"));
            }
            self.ws();
            let v = self.value(depth + 1)?;
            members.push((key, v));
            self.ws();
            if self.eat(b',') {
                continue;
            }
            if self.eat(b'}') {
                return Ok(Json::Object(members));
            }
            return Err(self.fail("expected , or }"));
        }
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// Append `s` to `out` as a JSON string literal.
///
/// The rule the golden vectors pin: `"` and `\` are escaped, the control
/// characters U+0000-U+001F are written as `\b \t \n \f \r` or `\u00xx`
/// (lower-case hex), and **everything else is written as is** — `/` and
/// non-ASCII are not escaped.
pub fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_what_it_should() {
        let v = parse(r#" {"a":[1,2.50,-3e2,"x\n\u00e9\ud83d\ude00"],"b":{"c":true,"d":null}} "#, 1000).unwrap();
        let Json::Object(m) = v else { panic!() };
        assert_eq!(m[0].0, "a");
        let Json::Array(a) = &m[0].1 else { panic!() };
        assert_eq!(a[1], Json::Number("2.50".into()));
        assert_eq!(a[2], Json::Number("-3e2".into()));
        assert_eq!(a[3], Json::String("x\n\u{e9}\u{1F600}".into()));
    }

    #[test]
    fn refuses_what_it_should() {
        for bad in ["", "{", "[1,]", "{\"a\":1,}", "01", "1.", ".5", "+1", "NaN", "\"a\nb\"", "\"\\ud800\"", "\"\\udc00\"", "{'a':1}", "[1] x", "tru", "{\"a\" 1}", "\"\\x\"", "-", "1e"] {
            assert!(parse(bad, 1000).is_err(), "{bad:?} should be refused");
        }
    }

    #[test]
    fn a_repeated_key_is_named() {
        let e = parse("{\"a\":1,\"b\":2,\"a\":3}", 1000).unwrap_err();
        assert_eq!(e.duplicate_key.as_deref(), Some("a"));
    }

    #[test]
    fn depth_and_size_are_bounded() {
        let deep = "[".repeat(MAX_DEPTH + 2) + &"]".repeat(MAX_DEPTH + 2);
        assert!(parse(&deep, 100_000).is_err());
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse(&ok, 100_000).is_ok());
        assert!(parse("[1,2,3]", 3).is_err());
    }

    #[test]
    fn strings_are_written_by_the_pinned_rule() {
        let mut s = String::new();
        write_string(&mut s, "a\"b\\c/\u{1}\u{1f}\t\n\u{e9}\u{1F600}");
        assert_eq!(s, "\"a\\\"b\\\\c/\\u0001\\u001f\\t\\n\u{e9}\u{1F600}\"");
    }
}
