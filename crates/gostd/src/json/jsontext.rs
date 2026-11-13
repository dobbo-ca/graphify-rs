//! The syntax walk of Go's active `encoding/json`. Go 1.27.4 builds it with GOEXPERIMENT=jsonv2,
//! so `Valid`, `Compact`, `Indent` and a `MarshalJSON` result go through `jsontext`:
//! `encoderState.reformatValue` (jsontext/encode.go:668-894) and `decoderState.consumeValue`
//! (jsontext/decode.go:857-1134) over the `jsonwire` readers (internal/jsonwire/decode.go), and
//! the error is made v1-like by `wrapSyntacticError` (jsontext/errors.go:72-117) and
//! `transformSyntacticError` (v2_scanner.go:56-91). Go recurses per container; this walk keeps an
//! explicit stack so that 10000 levels do not need 10000 stack frames.

use super::Error;
use super::indent::push_u00;
use crate::utf8::{RUNE_ERROR, decode_rune, rune_count};

/// Go `maxNestingDepth` (jsontext/state.go:53).
pub(super) const MAX_NESTING_DEPTH: usize = 10000;

/// The options of one walk.
pub(super) struct Opts<'a> {
    /// Go's decoder (`Valid`, `checkValid`): "after array element" and a truncated number
    /// reported at its start. Otherwise Go's encoder (`AppendFormat`, `WriteValue`).
    pub(super) decoder: bool,
    /// Go `Multiline` with `WithIndentPrefix` and `WithIndent`: `(prefix, indent)`.
    pub(super) indent: Option<(&'a str, &'a str)>,
    /// Go `EscapeForHTML`: `<`, `>` and `&` in strings become `\u00XX` escapes.
    pub(super) escape_html: bool,
    /// Go `EscapeForJS`: U+2028 and U+2029 in strings become `\u202X` escapes.
    pub(super) escape_js: bool,
}

/// The cause of a syntax error.
enum Fail {
    /// Go `io.ErrUnexpectedEOF`.
    Eof,
    /// Go `errMaxDepth` (jsontext/state.go:46).
    MaxDepth,
    /// Go `jsonwire.InvalidTextError` (internal/jsonwire/wire.go:141): the text is the `len`
    /// bytes at the error position.
    Text {
        label: &'static str,
        len: usize,
        at: String,
    },
}

/// A syntax error at byte `pos` of the input.
struct SynErr {
    pos: usize,
    fail: Fail,
}

type Res<T> = Result<T, SynErr>;

/// Appends `src` to `dst` reformatted per `o`, Go `jsontext.AppendFormat` (jsontext/value.go:45)
/// for the encoder and `ReadValue` then `CheckEOF` (v2_scanner.go:27-43) for the decoder. On an
/// error `dst` is left as it was and the error is Go's legacy `SyntaxError`.
pub(super) fn append_format(dst: &mut Vec<u8>, src: &[u8], o: &Opts) -> Result<(), Error> {
    let orig = dst.len();
    let mut w = Walk {
        src,
        pos: 0,
        dst: &mut *dst,
        o,
    };
    // Go `encoderState.WriteValue` (jsontext/encode.go:527-552) and `checkEOF`
    // (jsontext/decode.go:810).
    let r = w.walk().and_then(|()| {
        w.ws();
        match w.src.get(w.pos) {
            Some(_) => w.invalid_char(w.pos, "after top-level value"),
            None => Ok(()),
        }
    });
    r.map_err(|e| {
        dst.truncate(orig);
        legacy(src, e)
    })
}

struct Walk<'a> {
    src: &'a [u8],
    pos: usize,
    dst: &'a mut Vec<u8>,
    o: &'a Opts<'a>,
}

impl Walk<'_> {
    /// Go `jsonwire.NewInvalidCharacterError(src[pos:], at)` (internal/jsonwire/wire.go:129):
    /// the text is the first rune at `pos`, one byte if invalid.
    fn invalid_char<T>(&self, pos: usize, at: &str) -> Res<T> {
        Err(SynErr {
            pos,
            fail: Fail::Text {
                label: "character",
                len: decode_rune(&self.src[pos..]).1,
                at: at.to_string(),
            },
        })
    }

    /// Go `jsonwire.NewInvalidEscapeSequenceError` (internal/jsonwire/wire.go:134) for the `len`
    /// bytes at `pos`. Its "surrogate pair" label needs a text longer than 6 bytes, which only
    /// comes with UTF-8 validation; `Valid`, `Compact` and `Indent` allow invalid UTF-8.
    fn invalid_escape<T>(pos: usize, len: usize) -> Res<T> {
        Err(SynErr {
            pos,
            fail: Fail::Text {
                label: "escape sequence",
                len,
                at: "in string".to_string(),
            },
        })
    }

    /// Go `jsonwire.ConsumeWhitespace` (internal/jsonwire/decode.go:33).
    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\r' | b'\n') = self.src.get(self.pos) {
            self.pos += 1;
        }
    }

    /// The byte at `pos`, or Go's `io.ErrUnexpectedEOF` there.
    fn peek(&self) -> Res<u8> {
        self.src.get(self.pos).copied().ok_or(SynErr {
            pos: self.pos,
            fail: Fail::Eof,
        })
    }

    /// Go `encoderState.AppendIndent(b, n + 1)` (jsontext/encode.go:653) when multiline:
    /// a newline, the prefix and `n` indents.
    fn newline(&mut self, n: usize) {
        if let Some((prefix, indent)) = self.o.indent {
            self.dst.push(b'\n');
            self.dst.extend_from_slice(prefix.as_bytes());
            for _ in 0..n {
                self.dst.extend_from_slice(indent.as_bytes());
            }
        }
    }

    /// One top-level value, Go `reformatValue`, `reformatObject`, `reformatArray`
    /// (jsontext/encode.go:668-894) and `consumeValue`, `consumeObject`, `consumeArray`
    /// (jsontext/decode.go:857-1134). `stack` holds the closer of each open container; its length
    /// is Go's `depth` less one.
    fn walk(&mut self) -> Res<()> {
        let mut stack: Vec<u8> = Vec::new();
        self.ws();
        loop {
            match self.peek()? {
                b'{' | b'[' if stack.len() == MAX_NESTING_DEPTH => {
                    return Err(SynErr {
                        pos: self.pos,
                        fail: Fail::MaxDepth,
                    });
                }
                c @ (b'{' | b'[') => {
                    let close = c + 2; // '}' or ']'
                    self.dst.push(c);
                    self.pos += 1;
                    self.ws();
                    if self.peek()? != close {
                        stack.push(close);
                        self.newline(stack.len());
                        if close == b'}' {
                            self.name()?;
                        }
                        continue;
                    }
                    self.dst.push(close);
                    self.pos += 1;
                }
                b'n' => self.literal("null")?,
                b'f' => self.literal("false")?,
                b't' => self.literal("true")?,
                b'"' => self.string()?,
                b'-' | b'0'..=b'9' => self.number()?,
                _ => return self.invalid_char(self.pos, "at start of value"),
            }
            // After a value: a comma, or the close of the container (and maybe of its parents).
            loop {
                let Some(&close) = stack.last() else {
                    return Ok(());
                };
                self.ws();
                match self.peek()? {
                    b',' => {
                        self.dst.push(b',');
                        self.pos += 1;
                        self.newline(stack.len());
                        if close == b'}' {
                            self.name()?;
                        } else {
                            self.ws();
                        }
                        break;
                    }
                    c if c == close => {
                        self.newline(stack.len() - 1);
                        self.dst.push(close);
                        self.pos += 1;
                        stack.pop();
                    }
                    _ if close == b'}' => {
                        return self
                            .invalid_char(self.pos, "after object value (expecting ',' or '}')");
                    }
                    // jsontext/decode.go:1127 and jsontext/encode.go:891.
                    _ if self.o.decoder => {
                        return self
                            .invalid_char(self.pos, "after array element (expecting ',' or ']')");
                    }
                    _ => {
                        return self
                            .invalid_char(self.pos, "after array value (expecting ',' or ']')");
                    }
                }
            }
        }
    }

    /// An object name, the colon and the space before the value (jsontext/encode.go:745-785).
    fn name(&mut self) -> Res<()> {
        self.ws();
        self.peek()?;
        self.string()?;
        self.ws();
        if self.peek()? != b':' {
            return self.invalid_char(self.pos, "after object name (expecting ':')");
        }
        self.dst.push(b':');
        if self.o.indent.is_some() {
            // Go `InitializeMultiline` turns on `SpaceAfterColon`
            // (internal/jsonopts/options.go:184).
            self.dst.push(b' ');
        }
        self.pos += 1;
        self.ws();
        Ok(())
    }

    /// Go `ConsumeNull` etc., then `jsonwire.ConsumeLiteral` (internal/jsonwire/decode.go:76).
    fn literal(&mut self, lit: &str) -> Res<()> {
        let rest = &self.src[self.pos..];
        for (i, (&c, &l)) in rest.iter().zip(lit.as_bytes()).enumerate() {
            if c != l {
                let at = format!("in literal {lit} (expecting '{}')", char::from(l));
                return self.invalid_char(self.pos + i, &at);
            }
        }
        if rest.len() < lit.len() {
            return Err(SynErr {
                pos: self.src.len(),
                fail: Fail::Eof,
            });
        }
        self.dst.extend_from_slice(lit.as_bytes());
        self.pos += lit.len();
        Ok(())
    }

    /// Go `jsonwire.ConsumeString` with `validateUTF8` false (internal/jsonwire/decode.go:123-251),
    /// then the output of `ReformatString` under `PreserveRawStrings`
    /// (internal/jsonwire/encode.go:150-187).
    fn string(&mut self) -> Res<()> {
        let src = self.src;
        let start = self.pos;
        if src[start] != b'"' {
            return self.invalid_char(start, "at start of string (expecting '\"')");
        }
        let eof = |pos| {
            Err(SynErr {
                pos,
                fail: Fail::Eof,
            })
        };
        let mut n = start + 1;
        loop {
            // Optimize for long sequences of unescaped characters.
            while n < src.len() && (b' '..0x80).contains(&src[n]) && !matches!(src[n], b'\\' | b'"')
            {
                n += 1;
            }
            let Some(&c) = src.get(n) else {
                return eof(n);
            };
            if c == b'"' {
                n += 1;
                break;
            }
            match decode_rune(&src[n..]) {
                (_, rn @ 2..) => n += rn,
                ('\\', _) => n = self.escape(n)?,
                (RUNE_ERROR, _) if !crate::utf8::full_rune(&src[n..]) => return eof(n),
                (RUNE_ERROR, _) => n += 1,
                _ => {
                    return self.invalid_char(n, "in string (expecting non-control character)");
                }
            }
        }
        self.pos = n;
        self.write_string(&src[start..n]);
        Ok(())
    }

    /// The escape sequence at `n` (internal/jsonwire/decode.go:162-232); returns the position
    /// after it.
    fn escape(&self, n: usize) -> Res<usize> {
        let src = self.src;
        let eof = Err(SynErr {
            pos: n,
            fail: Fail::Eof,
        });
        match src.get(n + 1) {
            None => eof,
            Some(b'/' | b'"' | b'\\' | b'b' | b'f' | b'n' | b'r' | b't') => Ok(n + 2),
            Some(b'u') if src.len() < n + 6 && has_escaped_utf16_prefix(&src[n..]) => eof,
            Some(b'u') if src.len() < n + 6 => Self::invalid_escape(n, src.len() - n),
            Some(b'u') if !src[n + 2..n + 6].iter().all(u8::is_ascii_hexdigit) => {
                Self::invalid_escape(n, 6)
            }
            Some(b'u') => Ok(n + 6),
            Some(_) => Self::invalid_escape(n, 2),
        }
    }

    /// Writes the string `s` (quotes included) with the HTML and JS escapes that are on.
    fn write_string(&mut self, s: &[u8]) {
        if !self.o.escape_html && !self.o.escape_js {
            self.dst.extend_from_slice(s);
            return;
        }
        let (mut last, mut i) = (0, 0);
        while i < s.len() {
            let c = s[i];
            if c < 0x80 {
                if self.o.escape_html && matches!(c, b'<' | b'>' | b'&') {
                    self.dst.extend_from_slice(&s[last..i]);
                    push_u00(self.dst, c);
                    last = i + 1;
                }
                i += 1;
                continue;
            }
            let (r, rn) = decode_rune(&s[i..]);
            if self.o.escape_js && matches!(r, '\u{2028}' | '\u{2029}') {
                self.dst.extend_from_slice(&s[last..i]);
                self.dst.extend_from_slice(if r == '\u{2028}' {
                    b"\\u2028"
                } else {
                    b"\\u2029"
                });
                last = i + rn;
            }
            i += rn;
        }
        self.dst.extend_from_slice(&s[last..]);
    }

    /// Go `jsonwire.ConsumeNumber` (internal/jsonwire/decode.go:472-560), copied verbatim.
    fn number(&mut self) -> Res<()> {
        let src = self.src;
        let start = self.pos;
        let decoder = self.o.decoder;
        // A truncated number: Go's encoder reports the start of the part it was in (its
        // `resumeOffset`), the decoder the start of the number (`consumeNumber`,
        // jsontext/decode.go:946-968).
        let eof = |resume: usize| {
            Err(SynErr {
                pos: if decoder { start } else { resume },
                fail: Fail::Eof,
            })
        };
        let digits = |mut n: usize| {
            while src.get(n).is_some_and(u8::is_ascii_digit) {
                n += 1;
            }
            n
        };
        let mut n = start;
        if src[n] == b'-' {
            n += 1;
        }
        match src.get(n) {
            None => return eof(start),
            Some(b'0') => n += 1,
            Some(b'1'..=b'9') => n = digits(n + 1),
            Some(_) => return self.invalid_char(n, "in number (expecting digit)"),
        }
        if src.get(n) == Some(&b'.') {
            let resume = n;
            n += 1;
            match src.get(n) {
                None => return eof(resume),
                Some(c) if c.is_ascii_digit() => n = digits(n + 1),
                Some(_) => return self.invalid_char(n, "in number (expecting digit)"),
            }
        }
        if matches!(src.get(n), Some(b'e' | b'E')) {
            let resume = n;
            n += 1;
            if matches!(src.get(n), Some(b'+' | b'-')) {
                n += 1;
            }
            match src.get(n) {
                None => return eof(resume),
                Some(c) if c.is_ascii_digit() => n = digits(n + 1),
                Some(_) => return self.invalid_char(n, "in number (expecting digit)"),
            }
        }
        self.dst.extend_from_slice(&src[start..n]);
        self.pos = n;
        Ok(())
    }
}

/// Go `hasEscapedUTF16Prefix(b, false)` (internal/jsonwire/decode.go:390): `b` may be a
/// cut-off `\uXXXX` escape.
fn has_escaped_utf16_prefix(b: &[u8]) -> bool {
    b.iter().enumerate().all(|(i, &c)| match i {
        0 => c == b'\\',
        1 => c == b'u',
        2..6 => c.is_ascii_hexdigit(),
        _ => true,
    })
}

/// Go `wrapSyntacticError` under `ReportErrorsWithLegacySemantics` (jsontext/errors.go:102-116)
/// then `transformSyntacticError` (v2_scanner.go:56-80): the offset is past the invalid text, or
/// one past a structural error, and unchanged for `io.ErrUnexpectedEOF`.
fn legacy(src: &[u8], e: SynErr) -> Error {
    let (mut msg, adj) = match e.fail {
        Fail::Eof => ("unexpected end of JSON input".to_string(), 0),
        Fail::MaxDepth => ("exceeded max depth".to_string(), 1),
        Fail::Text { label, len, at } => (invalid_text(label, &src[e.pos..e.pos + len], &at), len),
    };
    if let Some(i) = msg.find(" (expecting")
        && !msg.contains(" in literal")
    {
        msg.truncate(i);
    }
    Error::Syntax {
        msg: replace_legacy(&msg),
        offset: (e.pos + adj) as i64,
    }
}

/// Go `InvalidTextError.Error` (internal/jsonwire/wire.go:147-161).
fn invalid_text(label: &str, what: &[u8], at: &str) -> String {
    let mut need_escape = false;
    let mut i = 0;
    while i < what.len() {
        let (r, n) = decode_rune(&what[i..]);
        if r == '`'
            || r == RUNE_ERROR
            || crate::unicode::is_space(r)
            || !crate::unicode::is_print(r)
        {
            need_escape = true;
            break;
        }
        i += n;
    }
    let what = if rune_count(what) == 1 {
        quote_rune(what)
    } else if need_escape {
        crate::strconv::quote(what)
    } else {
        format!("`{}`", String::from_utf8_lossy(what))
    };
    let s = format!("invalid {label} {what} {at}");
    match s.strip_suffix(' ') {
        Some(t) => t.to_string(),
        None => s,
    }
}

/// Go `jsonwire.QuoteRune` (internal/jsonwire/wire.go:62): an invalid byte is `'\xNN'`, else
/// `strconv.QuoteRune`.
fn quote_rune(b: &[u8]) -> String {
    let (r, n) = decode_rune(b);
    if r == RUNE_ERROR && n == 1 {
        return format!("'\\x{:x}'", b[0]);
    }
    match r {
        // strconv.QuoteRune escapes the single quote and not the double quote.
        '\'' => r"'\''".to_string(),
        '"' => r#"'"'"#.to_string(),
        _ => {
            let mut buf = [0u8; 4];
            let q = crate::strconv::quote(r.encode_utf8(&mut buf).as_bytes());
            format!("'{}'", &q[1..q.len() - 1])
        }
    }
}

/// Go `syntaxErrorReplacer` (v2_scanner.go:85-91), a `strings.Replacer`: left to right, at
/// each position the first pair (in argument order) whose old text matches.
fn replace_legacy(msg: &str) -> String {
    const PAIRS: [(&str, &str); 5] = [
        ("object name", "object key"),
        ("at start of value", "looking for beginning of value"),
        (
            "at start of string",
            "looking for beginning of object key string",
        ),
        ("after object value", "after object key:value pair"),
        ("in number", "in numeric literal"),
    ];
    let mut out = String::with_capacity(msg.len());
    let mut rest = msg;
    'outer: while let Some(c) = rest.chars().next() {
        for (old, new) in PAIRS {
            if let Some(r) = rest.strip_prefix(old) {
                out.push_str(new);
                rest = r;
                continue 'outer;
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}
