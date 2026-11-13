//! Reader side: `Unmarshal` into the `xmlElem` shape plus a test-only token API.
//!
//! A port of Go 1.27.4 `encoding/xml` `Decoder` (xml.go), strict mode only, no `CharsetReader`,
//! no `AutoClose`. Go file:line comments name the rule being matched.

use super::tables::{FIRST, SECOND};
use crate::strconv::quote;
use crate::utf8::{RUNE_ERROR, decode_rune};
use std::collections::HashMap;
use std::fmt;

/// Go `xml.Name`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Name {
    pub space: String,
    pub local: String,
}

/// Go `xmlElem{XMLName; Chardata ",chardata"; Children ",any"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: Name,
    pub chardata: String,
    pub children: Vec<Element>,
}

/// Go `*xml.SyntaxError`; Display is `XML syntax error on line N: msg`.
///
/// Errors that are not `*SyntaxError` in Go (`errUnmarshalDepth`, the
/// CharsetReader-nil error at xml.go:646, the unsupported-version error at xml.go:640, and
/// `io.EOF` from an input with no element) are carried here with `line == 0`
/// and `msg` holding Go's `err.Error()` text. Display then prints just
/// `msg`, without the "XML syntax error on line N:" prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub msg: String,
    pub line: usize,
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // xml.go:31 `Error`; line 0 is the not-a-SyntaxError marker (Go lines start at 1).
        if self.line == 0 {
            f.write_str(&self.msg)
        } else {
            write!(f, "XML syntax error on line {}: {}", self.line, self.msg)
        }
    }
}

impl std::error::Error for SyntaxError {}

/// Go `maxUnmarshalDepth` (read.go:319).
const MAX_UNMARSHAL_DEPTH: usize = 10000;

/// xml.go:338 `xmlURL`.
const XML_URL: &str = "http://www.w3.org/XML/1998/namespace";

/// Go `xml.Unmarshal(data, &xmlElem)`: first root element only.
///
/// Go recurses (read.go:326 `unmarshal`); this keeps an explicit stack so a 10,000-deep
/// document needs no deep call stack. Text and CDATA tokens directly under an element are
/// appended to its chardata (read.go:582); comments, processing instructions and directives
/// are ignored; attributes are parsed but dropped (`xmlElem` has no attr field). Everything
/// after the root's end tag is never read (`Unmarshal` returns there).
pub fn unmarshal_element(data: &[u8]) -> Result<Element, SyntaxError> {
    let mut d = Decoder::new(data, &[]);
    let mut open: Vec<(Element, Vec<u8>)> = Vec::new();
    loop {
        match d.token() {
            Ok(Token::StartElement { name, .. }) => {
                // read.go:327, checked on entry to every (child) unmarshal.
                if d.stk_depth > MAX_UNMARSHAL_DEPTH {
                    return Err(plain("exceeded max depth"));
                }
                let e = Element {
                    name,
                    chardata: String::new(),
                    children: Vec::new(),
                };
                open.push((e, Vec::new()));
            }
            Ok(Token::EndElement { .. }) => {
                let Some((mut e, text)) = open.pop() else {
                    continue;
                };
                e.chardata = String::from_utf8_lossy(&text).into_owned();
                match open.last_mut() {
                    Some((parent, _)) => parent.children.push(e),
                    None => return Ok(e),
                }
            }
            Ok(Token::CharData(t)) => {
                if let Some((_, text)) = open.last_mut() {
                    text.extend_from_slice(&t);
                }
            }
            Ok(_) => {}
            // read.go:329: `Token()` before the first start element returns io.EOF as is.
            Err(Stop::Eof) => return Err(plain("EOF")),
            Err(Stop::Syntax(e)) => return Err(e),
        }
    }
}

/// An error that is not a `*SyntaxError` in Go.
fn plain(msg: impl Into<String>) -> SyntaxError {
    SyntaxError {
        msg: msg.into(),
        line: 0,
    }
}

// ---- Test-only token API (Go `Decoder`, Strict = true, no CharsetReader) ----

/// Go `xml.Attr`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub name: Name,
    pub value: String,
}

/// Go `xml.Token` (`StartElement`, `EndElement`, `CharData`, `Comment`,
/// `ProcInst`, `Directive`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    StartElement { name: Name, attr: Vec<Attr> },
    EndElement { name: Name },
    CharData(Vec<u8>),
    Comment(Vec<u8>),
    ProcInst { target: String, inst: Vec<u8> },
    Directive(Vec<u8>),
}

/// All tokens from `Decoder.Token()` until EOF (namespace-translated, start
/// and end tags matched, `Entity` empty). Err on the first syntax error.
#[doc(hidden)]
pub fn tokens(data: &[u8]) -> Result<Vec<Token>, SyntaxError> {
    tokens_with_entity(data, &[])
}

/// As `tokens`, with `Decoder.Entity` set to the given (name, value) pairs.
#[doc(hidden)]
pub fn tokens_with_entity(data: &[u8], entity: &[(&str, &str)]) -> Result<Vec<Token>, SyntaxError> {
    match collect(&mut Decoder::new(data, entity), false) {
        (v, None) => Ok(v),
        (_, Some(e)) => Err(e),
    }
}

/// Tokens read before the first error, and that error (None on clean EOF).
/// `raw` selects `RawToken` (no namespace translation or tag matching) over `Token`.
#[doc(hidden)]
pub fn tokens_partial(data: &[u8], raw: bool) -> (Vec<Token>, Option<SyntaxError>) {
    collect(&mut Decoder::new(data, &[]), raw)
}

/// All tokens from `Decoder.RawToken()` until EOF (no namespace translation,
/// no start/end matching).
#[doc(hidden)]
pub fn raw_tokens(data: &[u8]) -> Result<Vec<Token>, SyntaxError> {
    match tokens_partial(data, true) {
        (v, None) => Ok(v),
        (_, Some(e)) => Err(e),
    }
}

/// `RawToken` stream (with `Decoder.Entity` set to `entity`) with `(token, start, end)` where start/end are
/// `Decoder.InputOffset()` before and after each call (Go `testRawToken`).
#[doc(hidden)]
pub fn raw_token_spans(
    data: &[u8],
    entity: &[(&str, &str)],
) -> Result<Vec<(Token, i64, i64)>, SyntaxError> {
    let mut d = Decoder::new(data, entity);
    let mut out = Vec::new();
    loop {
        let start = d.pos as i64;
        let r = d.raw_token();
        let end = d.pos as i64;
        match r {
            Ok(t) => out.push((t, start, end)),
            Err(Stop::Eof) => return Ok(out),
            Err(Stop::Syntax(e)) => return Err(e),
        }
    }
}

/// `Decoder.InputPos()` (line, column; both 1-based) after each `Token()` call
/// (Go `TestInputLinePos`).
#[doc(hidden)]
pub fn input_pos_after_tokens(data: &[u8]) -> Result<Vec<(usize, usize)>, SyntaxError> {
    let mut d = Decoder::new(data, &[]);
    let mut out = Vec::new();
    loop {
        match d.token() {
            // xml.go:948 `InputPos`.
            Ok(_) => out.push((d.line, d.pos - d.linestart + 1)),
            Err(Stop::Eof) => return Ok(out),
            Err(Stop::Syntax(e)) => return Err(e),
        }
    }
}

/// Go `procInst(param, s)`: the value of `param` in a `<?xml ...?>` instruction.
#[doc(hidden)]
pub fn proc_inst(param: &str, s: &str) -> String {
    String::from_utf8_lossy(&proc_inst_bytes(param, s.as_bytes())).into_owned()
}

/// Go `isInCharacterRange(r)`; takes a rune value (may exceed char range).
#[doc(hidden)]
pub fn is_in_character_range(r: u32) -> bool {
    // xml.go:1164
    matches!(r, 0x09 | 0x0A | 0x0D | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)
}

// ---- Decoder ----

/// Why the decoder stopped: Go's sticky `d.err`.
#[derive(Debug, Clone)]
enum Stop {
    /// `io.EOF`.
    Eof,
    /// A `*SyntaxError`, or (line 0) another Go error.
    Syntax(SyntaxError),
}

/// Go `stack` (xml.go:380) entries; `stkEOF` is only for `DecodeElement`, not reached.
enum Frame {
    /// `stkStart`: an open element, by its raw (untranslated) name.
    Start(Name),
    /// `stkNs`: the old value of `ns[local]` to restore when the element ends.
    Ns(String, Option<String>),
}

struct Decoder<'a> {
    data: &'a [u8],
    /// Go `d.offset`; also the read position, because Go's `ungetc` backs `offset` up.
    pos: usize,
    buf: Vec<u8>,
    stk: Vec<Frame>,
    /// Go `d.stkDepth`: open elements.
    stk_depth: usize,
    need_close: bool,
    to_close: Name,
    ns: HashMap<String, String>,
    err: Option<Stop>,
    line: usize,
    linestart: usize,
    entity: &'a [(&'a str, &'a str)],
}

fn collect(d: &mut Decoder<'_>, raw: bool) -> (Vec<Token>, Option<SyntaxError>) {
    let mut out = Vec::new();
    loop {
        let r = if raw { d.raw_token() } else { d.token() };
        match r {
            Ok(t) => out.push(t),
            Err(Stop::Eof) => return (out, None),
            Err(Stop::Syntax(e)) => return (out, Some(e)),
        }
    }
}

impl<'a> Decoder<'a> {
    fn new(data: &'a [u8], entity: &'a [(&'a str, &'a str)]) -> Self {
        Decoder {
            data,
            pos: 0,
            buf: Vec::new(),
            stk: Vec::new(),
            stk_depth: 0,
            need_close: false,
            to_close: Name::default(),
            ns: HashMap::new(),
            err: None,
            line: 1,
            linestart: 0,
            entity,
        }
    }

    /// xml.go:475 `syntaxError`: a SyntaxError with the current line (not stored).
    fn syntax_error(&self, msg: &str) -> Stop {
        Stop::Syntax(SyntaxError {
            msg: msg.to_string(),
            line: self.line,
        })
    }

    /// `d.err = d.syntaxError(msg); return nil, d.err`.
    fn fail<T>(&mut self, msg: impl AsRef<str>) -> Result<T, Stop> {
        let e = self.syntax_error(msg.as_ref());
        self.err = Some(e.clone());
        Err(e)
    }

    /// `d.err = fmt.Errorf(..); return nil, d.err`: a non-SyntaxError (line 0).
    fn fail_plain<T>(&mut self, msg: String) -> Result<T, Stop> {
        let e = Stop::Syntax(plain(msg));
        self.err = Some(e.clone());
        Err(e)
    }

    /// The stored error (callers only ask after a failure stored one).
    fn cur(&self) -> Stop {
        self.err.clone().unwrap_or(Stop::Eof)
    }

    /// `if d.err == nil { d.err = d.syntaxError(msg) }; return d.err` after a failed name read.
    fn name_failed(&mut self, msg: &str) -> Stop {
        if self.err.is_none() {
            self.err = Some(self.syntax_error(msg));
        }
        self.cur()
    }

    /// xml.go:914 `getc`: one byte, maintaining the line number; None leaves the error in `err`.
    fn getc(&mut self) -> Option<u8> {
        if self.err.is_some() {
            return None;
        }
        let Some(&b) = self.data.get(self.pos) else {
            self.err = Some(Stop::Eof);
            return None;
        };
        self.pos += 1;
        if b == b'\n' {
            self.line += 1;
            self.linestart = self.pos;
        }
        Some(b)
    }

    /// xml.go:966 `mustgetc`: EOF becomes `SyntaxError("unexpected EOF")`.
    fn mustgetc(&mut self) -> Result<u8, Stop> {
        if let Some(b) = self.getc() {
            return Ok(b);
        }
        if matches!(self.err, Some(Stop::Eof)) {
            self.err = Some(self.syntax_error("unexpected EOF"));
        }
        Err(self.cur())
    }

    /// xml.go:976 `ungetc`.
    fn ungetc(&mut self, b: u8) {
        if b == b'\n' {
            self.line -= 1;
        }
        self.pos -= 1;
    }

    /// xml.go:895 `space`.
    fn space(&mut self) {
        while let Some(b) = self.getc() {
            if !matches!(b, b' ' | b'\r' | b'\n' | b'\t') {
                self.ungetc(b);
                return;
            }
        }
    }

    /// xml.go:1212 `readName`: appends a name to `buf`; false on EOF (error stored) or when
    /// the first byte is not a name byte.
    fn read_name(&mut self) -> bool {
        let Ok(b) = self.mustgetc() else {
            return false;
        };
        if b < 0x80 && !is_name_byte(b) {
            self.ungetc(b);
            return false;
        }
        self.buf.push(b);
        loop {
            let Ok(b) = self.mustgetc() else {
                return false;
            };
            if b < 0x80 && !is_name_byte(b) {
                self.ungetc(b);
                break;
            }
            self.buf.push(b);
        }
        true
    }

    /// xml.go:1194 `name`: does not set `err` for a missing name (the caller adds context).
    fn name(&mut self) -> Option<String> {
        self.buf.clear();
        if !self.read_name() {
            return None;
        }
        if !is_name(&self.buf) {
            let msg = format!("invalid XML name: {}", String::from_utf8_lossy(&self.buf));
            self.err = Some(self.syntax_error(&msg));
            return None;
        }
        Some(String::from_utf8_lossy(&self.buf).into_owned())
    }

    /// xml.go:1175 `nsname`: more than one `:`, or an empty side, is not a prefix split.
    fn nsname(&mut self) -> Option<Name> {
        let s = self.name()?;
        if s.matches(':').count() > 1 {
            return None;
        }
        let split = s
            .split_once(':')
            .filter(|(a, b)| !a.is_empty() && !b.is_empty())
            .map(|(a, b)| (a.to_string(), b.to_string()));
        Some(match split {
            Some((space, local)) => Name { space, local },
            None => Name {
                space: String::new(),
                local: s,
            },
        })
    }

    /// xml.go:860 `attrval`, strict: only quoted values.
    fn attrval(&mut self) -> Result<Vec<u8>, Stop> {
        let b = self.mustgetc()?;
        if b == b'"' || b == b'\'' {
            return self.text(i32::from(b), false);
        }
        self.fail("unquoted or missing attribute value in element")
    }

    /// xml.go:996 `text`: plain text (`quote < 0`), a quoted attribute value, or CDATA.
    fn text(&mut self, quote: i32, cdata: bool) -> Result<Vec<u8>, Stop> {
        let (mut b0, mut b1) = (0u8, 0u8);
        let mut trunc = 0;
        self.buf.clear();
        loop {
            let Some(b) = self.getc() else {
                if cdata {
                    if matches!(self.err, Some(Stop::Eof)) {
                        self.err = Some(self.syntax_error("unexpected EOF in CDATA section"));
                    }
                    return Err(self.cur());
                }
                break;
            };

            // xml.go:1013: `]]>` ends CDATA; it is an error in plain text, fine in quotes.
            if quote < 0 && b0 == b']' && b1 == b']' && b == b'>' {
                if cdata {
                    trunc = 2;
                    break;
                }
                return self.fail("unescaped ]]> not in CDATA section");
            }

            // xml.go:1025: stop at `<`.
            if b == b'<' && !cdata {
                if quote >= 0 {
                    return self.fail("unescaped < inside quoted string");
                }
                self.ungetc(b'<');
                break;
            }
            if quote >= 0 && i32::from(b) == quote {
                break;
            }
            if b == b'&' && !cdata {
                // xml.go:1038: entity up to the semicolon.
                let before = self.buf.len();
                self.buf.push(b'&');
                let mut text: Option<String> = None;
                let mut c = self.mustgetc()?;
                if c == b'#' {
                    self.buf.push(c);
                    c = self.mustgetc()?;
                    let mut base = 10;
                    if c == b'x' {
                        base = 16;
                        self.buf.push(c);
                        c = self.mustgetc()?;
                    }
                    let start = self.buf.len();
                    while c.is_ascii_digit() || (base == 16 && c.is_ascii_hexdigit()) {
                        self.buf.push(c);
                        c = self.mustgetc()?;
                    }
                    if c != b';' {
                        self.ungetc(c);
                    } else {
                        // ParseUint(s, base, 64) fails on empty input and overflow.
                        let n = std::str::from_utf8(&self.buf[start..])
                            .ok()
                            .and_then(|s| u64::from_str_radix(s, base).ok());
                        self.buf.push(b';');
                        if let Some(n) = n.filter(|&n| n <= 0x10FFFF) {
                            // `string(rune(n))`: a surrogate becomes U+FFFD.
                            let r = char::from_u32(n as u32).unwrap_or(RUNE_ERROR);
                            text = Some(r.to_string());
                        }
                    }
                } else {
                    self.ungetc(c);
                    if !self.read_name() && self.err.is_some() {
                        return Err(self.cur());
                    }
                    c = self.mustgetc()?;
                    if c != b';' {
                        self.ungetc(c);
                    } else {
                        let name = self.buf[before + 1..].to_vec();
                        self.buf.push(b';');
                        if is_name(&name) {
                            // xml.go:984 `entity` map, then `d.Entity`.
                            text = match name.as_slice() {
                                b"lt" => Some("<".to_string()),
                                b"gt" => Some(">".to_string()),
                                b"amp" => Some("&".to_string()),
                                b"apos" => Some("'".to_string()),
                                b"quot" => Some("\"".to_string()),
                                n => self
                                    .entity
                                    .iter()
                                    .find(|(k, _)| k.as_bytes() == n)
                                    .map(|(_, v)| (*v).to_string()),
                            };
                        }
                    }
                }

                if let Some(t) = text {
                    self.buf.truncate(before);
                    self.buf.extend_from_slice(t.as_bytes());
                    b0 = 0;
                    b1 = 0;
                    continue;
                }
                // xml.go:1117: strict mode rejects an unknown or malformed entity.
                let mut ent = String::from_utf8_lossy(&self.buf[before..]).into_owned();
                if self.buf.last() != Some(&b';') {
                    ent.push_str(" (no semicolon)");
                }
                return self.fail(format!("invalid character entity {ent}"));
            }

            // xml.go:1129: unescaped `\r` and `\r\n` become `\n`.
            if b == b'\r' {
                self.buf.push(b'\n');
            } else if !(b1 == b'\r' && b == b'\n') {
                self.buf.push(b);
            }

            b0 = b1;
            b1 = b;
        }
        self.buf.truncate(self.buf.len() - trunc);

        // xml.go:1143: each rune must be valid UTF-8 and in the XML character range.
        let mut i = 0;
        while i < self.buf.len() {
            let b = self.buf[i];
            if b < 0x80 {
                if !is_in_character_range(u32::from(b)) {
                    return self.fail(format!("illegal character code U+{b:04X}"));
                }
                i += 1;
                continue;
            }
            let (r, size) = decode_rune(&self.buf[i..]);
            if r == RUNE_ERROR && size == 1 {
                return self.fail("invalid UTF-8");
            }
            i += size;
            if !is_in_character_range(r as u32) {
                return self.fail(format!("illegal character code U+{:04X}", r as u32));
            }
        }
        Ok(std::mem::take(&mut self.buf))
    }

    /// xml.go:346 `translate`: the default namespace applies to element names only.
    fn translate(&self, n: &mut Name, is_element_name: bool) {
        if n.space == "xmlns" || (n.space.is_empty() && !is_element_name) {
            return;
        }
        if n.space == "xml" {
            n.space = XML_URL.to_string();
        } else if n.space.is_empty() && n.local == "xmlns" {
            return;
        }
        if let Some(v) = self.ns.get(&n.space) {
            n.space = v.clone();
        }
        // else-if n.Space == "": DefaultSpace is "" here, so nothing to do.
    }

    /// xml.go:485 `popElement`: match the end tag against the open element by raw prefix and
    /// local name, translate it, then undo the namespace bindings of the closed element.
    fn pop_element(&mut self, t: &mut Name) -> Result<(), Stop> {
        let sn = match self.stk.pop() {
            Some(Frame::Start(sn)) => {
                self.stk_depth -= 1;
                sn
            }
            _ => return self.fail(format!("unexpected end element </{}>", t.local)),
        };
        if sn.local != t.local {
            return self.fail(format!("element <{}> closed by </{}>", sn.local, t.local));
        }
        if sn.space != t.space {
            let ns = if t.space.is_empty() {
                "\"\""
            } else {
                t.space.as_str()
            };
            return self.fail(format!(
                "element <{}> in space {} closed by </{}> in space {}",
                sn.local, sn.space, t.local, ns
            ));
        }
        self.translate(t, true);
        while let Some(Frame::Ns(..)) = self.stk.last() {
            if let Some(Frame::Ns(local, old)) = self.stk.pop() {
                match old {
                    Some(v) => {
                        self.ns.insert(local, v);
                    }
                    None => {
                        self.ns.remove(&local);
                    }
                }
            }
        }
        Ok(())
    }

    /// xml.go:275 `Token`.
    fn token(&mut self) -> Result<Token, Stop> {
        let mut t = match self.raw_token() {
            Ok(t) => t,
            // xml.go:287: EOF with elements still open (not stored in `err`).
            Err(Stop::Eof) if !self.stk.is_empty() => {
                return Err(self.syntax_error("unexpected EOF"));
            }
            Err(e) => return Err(e),
        };
        match &mut t {
            Token::StartElement { name, attr } => {
                // xml.go:303: xmlns attributes bind before the names are translated.
                for a in attr.iter() {
                    if a.name.space == "xmlns" {
                        let old = self.ns.insert(a.name.local.clone(), a.value.clone());
                        self.stk.push(Frame::Ns(a.name.local.clone(), old));
                    }
                    if a.name.space.is_empty() && a.name.local == "xmlns" {
                        let old = self.ns.insert(String::new(), a.value.clone());
                        self.stk.push(Frame::Ns(String::new(), old));
                    }
                }
                self.stk.push(Frame::Start(name.clone()));
                self.stk_depth += 1;
                self.translate(name, true);
                for a in attr.iter_mut() {
                    self.translate(&mut a.name, false);
                }
            }
            Token::EndElement { name } => self.pop_element(name)?,
            _ => {}
        }
        Ok(t)
    }

    /// xml.go:558 `rawToken`.
    fn raw_token(&mut self) -> Result<Token, Stop> {
        if let Some(e) = &self.err {
            return Err(e.clone());
        }
        if self.need_close {
            // The last element was self-closing: return the EndElement half now.
            self.need_close = false;
            return Ok(Token::EndElement {
                name: self.to_close.clone(),
            });
        }

        let Some(b) = self.getc() else {
            return Err(self.cur());
        };
        if b != b'<' {
            // Text section.
            self.ungetc(b);
            return self.text(-1, false).map(Token::CharData);
        }

        let b = self.mustgetc()?;
        match b {
            b'/' => {
                // `</`: end element.
                let Some(name) = self.nsname() else {
                    return Err(self.name_failed("expected element name after </"));
                };
                self.space();
                let b = self.mustgetc()?;
                if b != b'>' {
                    return self.fail(format!("invalid characters between </{} and >", name.local));
                }
                return Ok(Token::EndElement { name });
            }

            b'?' => {
                // `<?`: processing instruction.
                let Some(target) = self.name() else {
                    return Err(self.name_failed("expected target name after <?"));
                };
                self.space();
                self.buf.clear();
                let mut b0 = 0u8;
                loop {
                    let b = self.mustgetc()?;
                    self.buf.push(b);
                    if b0 == b'?' && b == b'>' {
                        break;
                    }
                    b0 = b;
                }
                let data = self.buf[..self.buf.len() - 2].to_vec(); // chop ?>

                if target == "xml" {
                    // xml.go:636: the declaration is checked; CharsetReader is nil.
                    let ver = proc_inst_bytes("version", &data);
                    if !ver.is_empty() && ver != b"1.0" {
                        return self.fail_plain(format!(
                            "xml: unsupported version {}; only version 1.0 is supported",
                            quote(&ver)
                        ));
                    }
                    let enc = proc_inst_bytes("encoding", &data);
                    // Go tests "utf-8", "UTF-8", then strings.EqualFold(enc, "utf-8"); against an
                    // ASCII pattern with no fold specials that is ASCII case-insensitive.
                    if !enc.is_empty() && !enc.eq_ignore_ascii_case(b"utf-8") {
                        return self.fail_plain(format!(
                            "xml: encoding {} declared but Decoder.CharsetReader is nil",
                            quote(&enc)
                        ));
                    }
                }
                return Ok(Token::ProcInst { target, inst: data });
            }

            b'!' => {
                // `<!`: maybe comment, maybe CDATA, else a directive.
                let b = self.mustgetc()?;
                match b {
                    b'-' => {
                        // `<!-`: probably a comment.
                        let b = self.mustgetc()?;
                        if b != b'-' {
                            return self.fail("invalid sequence <!- not part of <!--");
                        }
                        self.buf.clear();
                        let (mut b0, mut b1) = (0u8, 0u8);
                        loop {
                            let b = self.mustgetc()?;
                            self.buf.push(b);
                            if b0 == b'-' && b1 == b'-' {
                                if b != b'>' {
                                    return self
                                        .fail("invalid sequence \"--\" not allowed in comments");
                                }
                                break;
                            }
                            b0 = b1;
                            b1 = b;
                        }
                        let n = self.buf.len() - 3; // chop -->
                        return Ok(Token::Comment(self.buf[..n].to_vec()));
                    }
                    b'[' => {
                        // `<![`: probably `<![CDATA[`.
                        for &c in b"CDATA[" {
                            let b = self.mustgetc()?;
                            if b != c {
                                return self.fail("invalid <![ sequence");
                            }
                        }
                        return self.text(-1, true).map(Token::CharData);
                    }
                    _ => {}
                }

                // xml.go:718: a directive; quoted angle brackets do not count for nesting, and a
                // comment inside is replaced by one space.
                self.buf.clear();
                self.buf.push(b);
                let mut inquote = 0u8;
                let mut depth = 0i32;
                loop {
                    let mut b = self.mustgetc()?;
                    if inquote == 0 && b == b'>' && depth == 0 {
                        break;
                    }
                    // Go's `HandleB:` label.
                    'handle: loop {
                        self.buf.push(b);
                        if b == inquote {
                            inquote = 0;
                        } else if inquote != 0 {
                            // in quotes, no special action
                        } else if b == b'\'' || b == b'"' {
                            inquote = b;
                        } else if b == b'>' {
                            depth -= 1;
                        } else if b == b'<' {
                            // Look for `<!--` to begin a comment.
                            for (i, &c) in b"!--".iter().enumerate() {
                                b = self.mustgetc()?;
                                if b != c {
                                    for &p in &b"!--"[..i] {
                                        self.buf.push(p);
                                    }
                                    depth += 1;
                                    continue 'handle;
                                }
                            }
                            // Remove the `<` that was written above.
                            self.buf.pop();
                            // Look for the terminator.
                            let (mut b0, mut b1) = (0u8, 0u8);
                            loop {
                                b = self.mustgetc()?;
                                if b0 == b'-' && b1 == b'-' && b == b'>' {
                                    break;
                                }
                                b0 = b1;
                                b1 = b;
                            }
                            self.buf.push(b' ');
                        }
                        break;
                    }
                }
                return Ok(Token::Directive(self.buf.clone()));
            }
            _ => {}
        }

        // Must be an open element like `<a href="foo">`.
        self.ungetc(b);
        let Some(name) = self.nsname() else {
            return Err(self.name_failed("expected element name after <"));
        };
        let mut attr = Vec::new();
        let mut empty = false;
        loop {
            self.space();
            let b = self.mustgetc()?;
            if b == b'/' {
                empty = true;
                let b = self.mustgetc()?;
                if b != b'>' {
                    return self.fail("expected /> in element");
                }
                break;
            }
            if b == b'>' {
                break;
            }
            self.ungetc(b);

            let Some(aname) = self.nsname() else {
                return Err(self.name_failed("expected attribute name in element"));
            };
            self.space();
            let b = self.mustgetc()?;
            if b != b'=' {
                // Strict mode only.
                return self.fail("attribute name without = in element");
            }
            self.space();
            let value = self.attrval()?;
            attr.push(Attr {
                name: aname,
                value: String::from_utf8_lossy(&value).into_owned(),
            });
        }
        if empty {
            self.need_close = true;
            self.to_close = name.clone();
        }
        Ok(Token::StartElement { name, attr })
    }
}

/// xml.go:1236 `isNameByte`.
fn is_name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'_' | b':' | b'.' | b'-')
}

/// Go `unicode.Is(table, c)` for a sorted `(lo, hi, stride)` table.
fn in_table(t: &[(u32, u32, u32)], c: u32) -> bool {
    let i = t.partition_point(|r| r.0 <= c);
    if i == 0 {
        return false;
    }
    let (lo, hi, stride) = t[i - 1];
    c <= hi && (c - lo).is_multiple_of(stride)
}

/// xml.go:1243 `isName`: the first rune is in `first`, the rest in `first` or `second`.
fn is_name(s: &[u8]) -> bool {
    if s.is_empty() {
        return false;
    }
    let (c, mut n) = decode_rune(s);
    if c == RUNE_ERROR && n == 1 {
        return false;
    }
    if !in_table(FIRST, c as u32) {
        return false;
    }
    while n < s.len() {
        let (c, w) = decode_rune(&s[n..]);
        if c == RUNE_ERROR && w == 1 {
            return false;
        }
        if !in_table(FIRST, c as u32) && !in_table(SECOND, c as u32) {
            return false;
        }
        n += w;
    }
    true
}

/// xml.go:2056 `procInst`, on bytes (a declaration's content is not checked for UTF-8).
fn proc_inst_bytes(param: &str, s: &[u8]) -> Vec<u8> {
    // TODO in Go: this parsing is somewhat lame and not exact.
    let param = format!("{param}=").into_bytes();
    let lenp = param.len();
    let mut i = 0;
    let mut sep = 0u8;
    while i < s.len() {
        let sub = &s[i..];
        let Some(k) = sub.windows(lenp).position(|w| w == param.as_slice()) else {
            return Vec::new();
        };
        if lenp + k >= sub.len() {
            return Vec::new();
        }
        let c = sub[lenp + k];
        i += lenp + k + 1;
        if c == b'\'' || c == b'"' {
            sep = c;
            break;
        }
    }
    if sep == 0 {
        return Vec::new();
    }
    match s[i..].iter().position(|&c| c == sep) {
        Some(j) => s[i..i + j].to_vec(),
        None => Vec::new(),
    }
}
