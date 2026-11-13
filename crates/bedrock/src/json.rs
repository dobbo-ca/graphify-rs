//! A minimal JSON encoder and decoder, private to this crate. It swaps for `gostd::json` when that
//! lands; other modules use only the items below.

use std::ops::Range;

#[cfg(test)]
mod tests;

/// Appends `s` as a JSON string with Go's encoding (anthropic-sdk-go internal/encoding/json with
/// HTML escaping on): `<` `>` `&` as `\u003c` `\u003e` `\u0026`, U+2028/U+2029 escaped, and each
/// byte of invalid UTF-8 as `\ufffd`.
pub(crate) fn write_string(out: &mut String, s: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push('"');
    for chunk in s.utf8_chunks() {
        for c in chunk.valid().chars() {
            match c {
                '\\' | '"' => {
                    out.push('\\');
                    out.push(c);
                }
                '\u{8}' => out.push_str("\\b"),
                '\u{c}' => out.push_str("\\f"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                // encode.go:1051-1056: other control bytes, and < > & (HTML escaping).
                '\0'..='\u{1f}' | '<' | '>' | '&' => {
                    let b = c as usize;
                    out.push_str("\\u00");
                    out.push(HEX[b >> 4] as char);
                    out.push(HEX[b & 0xf] as char);
                }
                // encode.go:1081-1088.
                '\u{2028}' => out.push_str("\\u2028"),
                '\u{2029}' => out.push_str("\\u2029"),
                _ => out.push(c),
            }
        }
        // encode.go:1072-1077: one \ufffd per invalid byte.
        for _ in chunk.invalid() {
            out.push_str("\\ufffd");
        }
    }
    out.push('"');
}

/// A parsed JSON value and the byte range of its text in the source.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Value {
    pub kind: Kind,
    pub span: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Null,
    #[cfg_attr(not(test), allow(dead_code))]
    Bool(bool),
    /// The number's text, as written.
    Number(String),
    String(String),
    Array(Vec<Value>),
    /// Entries in source order; duplicate keys are kept.
    Object(Vec<(String, Value)>),
}

/// Parses one JSON value; only whitespace may follow it.
pub(crate) fn parse(src: &str) -> Result<Value, String> {
    let mut p = Parser {
        src,
        b: src.as_bytes(),
        pos: 0,
    };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.pos != p.b.len() {
        return Err(format!("trailing data at byte {}", p.pos));
    }
    Ok(v)
}

/// Go `json.Decoder.Decode` (requestconfig.go:560): the first value; whatever follows it is ignored.
pub(crate) fn parse_first(src: &str) -> Result<Value, String> {
    let mut p = Parser {
        src,
        b: src.as_bytes(),
        pos: 0,
    };
    p.ws();
    p.value(0)
}

/// Go encoding/json's nesting limit (scanner.go maxNestingDepth).
// Go allows 10000, but each level is a Rust stack frame; 1000 stays safe on small test stacks.
const MAX_DEPTH: usize = 1000;

struct Parser<'a> {
    src: &'a str,
    b: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while matches!(self.b.get(self.pos), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("{what} at byte {}", self.pos))
    }

    fn lit(&mut self, word: &str, kind: Kind) -> Result<Kind, String> {
        if self.b[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(kind)
        } else {
            self.err("invalid literal")
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        let start = self.pos;
        let kind = match self.b.get(self.pos) {
            None => return self.err("unexpected end of input"),
            Some(b'n') => self.lit("null", Kind::Null)?,
            Some(b't') => self.lit("true", Kind::Bool(true))?,
            Some(b'f') => self.lit("false", Kind::Bool(false))?,
            Some(b'"') => Kind::String(self.string()?),
            Some(b'-' | b'0'..=b'9') => {
                self.number()?;
                Kind::Number(self.src[start..self.pos].to_string())
            }
            Some(b'[') => {
                if depth >= MAX_DEPTH {
                    return self.err("exceeded max depth");
                }
                self.pos += 1;
                let mut items = Vec::new();
                self.ws();
                if self.b.get(self.pos) == Some(&b']') {
                    self.pos += 1;
                } else {
                    loop {
                        self.ws();
                        items.push(self.value(depth + 1)?);
                        self.ws();
                        match self.b.get(self.pos) {
                            Some(b',') => self.pos += 1,
                            Some(b']') => {
                                self.pos += 1;
                                break;
                            }
                            _ => return self.err("expected , or ]"),
                        }
                    }
                }
                Kind::Array(items)
            }
            Some(b'{') => {
                if depth >= MAX_DEPTH {
                    return self.err("exceeded max depth");
                }
                self.pos += 1;
                let mut entries = Vec::new();
                self.ws();
                if self.b.get(self.pos) == Some(&b'}') {
                    self.pos += 1;
                } else {
                    loop {
                        self.ws();
                        if self.b.get(self.pos) != Some(&b'"') {
                            return self.err("expected object key");
                        }
                        let key = self.string()?;
                        self.ws();
                        if self.b.get(self.pos) != Some(&b':') {
                            return self.err("expected :");
                        }
                        self.pos += 1;
                        self.ws();
                        entries.push((key, self.value(depth + 1)?));
                        self.ws();
                        match self.b.get(self.pos) {
                            Some(b',') => self.pos += 1,
                            Some(b'}') => {
                                self.pos += 1;
                                break;
                            }
                            _ => return self.err("expected , or }"),
                        }
                    }
                }
                Kind::Object(entries)
            }
            Some(_) => return self.err("unexpected character"),
        };
        Ok(Value {
            kind,
            span: start..self.pos,
        })
    }

    fn digits(&mut self) -> bool {
        let from = self.pos;
        while matches!(self.b.get(self.pos), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        self.pos > from
    }

    /// JSON number grammar: -? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?
    fn number(&mut self) -> Result<(), String> {
        if self.b.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        match self.b.get(self.pos) {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return self.err("invalid number"),
        }
        if self.b.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            if !self.digits() {
                return self.err("invalid number");
            }
        }
        if matches!(self.b.get(self.pos), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.b.get(self.pos), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !self.digits() {
                return self.err("invalid number");
            }
        }
        Ok(())
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let Some(h) = self.src.get(self.pos..self.pos + 4) else {
            return self.err("short \\u escape");
        };
        if !h.bytes().all(|c| c.is_ascii_hexdigit()) {
            return self.err("bad \\u escape");
        }
        self.pos += 4;
        Ok(u32::from_str_radix(h, 16).unwrap_or(0))
    }

    /// Go decode.go unquote: a lone or mismatched surrogate becomes U+FFFD, and a high surrogate
    /// not followed by a low one leaves the next escape unconsumed.
    fn string(&mut self) -> Result<String, String> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            let from = self.pos;
            while matches!(self.b.get(self.pos), Some(c) if *c != b'"' && *c != b'\\' && *c >= 0x20)
            {
                self.pos += 1;
            }
            out.push_str(&self.src[from..self.pos]);
            match self.b.get(self.pos) {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let Some(&e) = self.b.get(self.pos) else {
                        return self.err("unterminated string");
                    };
                    self.pos += 1;
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
                            let r = self.hex4()?;
                            let c = match r {
                                0xD800..=0xDBFF => {
                                    let save = self.pos;
                                    let low = if self.b[self.pos..].starts_with(b"\\u") {
                                        self.pos += 2;
                                        self.hex4().ok()
                                    } else {
                                        None
                                    };
                                    match low {
                                        Some(l @ 0xDC00..=0xDFFF) => char::from_u32(
                                            0x10000 + ((r - 0xD800) << 10) + (l - 0xDC00),
                                        ),
                                        _ => {
                                            self.pos = save;
                                            None
                                        }
                                    }
                                }
                                _ => char::from_u32(r),
                            };
                            out.push(c.unwrap_or('\u{FFFD}'));
                        }
                        _ => {
                            self.pos -= 1;
                            return self.err("bad escape");
                        }
                    }
                }
                Some(_) => return self.err("control character in string"),
                None => return self.err("unterminated string"),
            }
        }
    }
}

impl Value {
    /// The exact source text of this value (Go `respjson.Field.Raw()`).
    pub(crate) fn raw<'s>(&self, src: &'s str) -> &'s str {
        &src[self.span.clone()]
    }

    /// The first entry named exactly `key` (gjson lookup, used for API responses).
    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        let Kind::Object(entries) = &self.kind else {
            return None;
        };
        entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Go `encoding/json` v1 struct-field rule: the last entry whose key equals `key` ignoring
    /// case wins (used for `credential_process` output).
    pub(crate) fn get_go(&self, key: &str) -> Option<&Value> {
        let Kind::Object(entries) = &self.kind else {
            return None;
        };
        // Go foldName: per rune ToUpper(ToLower(r)) with simple (single-char) mappings.
        let fold = |c: char| {
            let one = |mut it: std::vec::IntoIter<char>| match (it.next(), it.next()) {
                (Some(x), None) => Some(x),
                _ => None,
            };
            let lower = if c == '\u{130}' {
                'i'
            } else {
                one(c.to_lowercase().collect::<Vec<_>>().into_iter()).unwrap_or(c)
            };
            one(lower.to_uppercase().collect::<Vec<_>>().into_iter()).unwrap_or(lower)
        };
        let eq = |k: &str| k.chars().map(fold).eq(key.chars().map(fold));
        entries.iter().rev().find(|(k, _)| eq(k)).map(|(_, v)| v)
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match &self.kind {
            Kind::String(s) => Some(s),
            _ => None,
        }
    }
}
