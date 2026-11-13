//! Writer side: `xml.MarshalIndent` of a GraphML-shaped tree and escaping.

use super::is_in_character_range;
use crate::utf8::{RUNE_ERROR, decode_rune};

/// Go `xml.Header`.
pub const HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";

/// One element: string `,attr` fields, `,chardata`, child elements, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub chardata: String,
    pub children: Vec<Node>,
}

/// Go `printer` state used by `writeIndent` (marshal.go:1060).
struct Printer<'a> {
    out: Vec<u8>,
    prefix: &'a str,
    indent: &'a str,
    depth: usize,
    indented_in: bool,
    put_newline: bool,
}

impl Printer<'_> {
    /// marshal.go:1060 `writeIndent`.
    fn write_indent(&mut self, depth_delta: i32) {
        if self.prefix.is_empty() && self.indent.is_empty() {
            return;
        }
        if depth_delta < 0 {
            self.depth -= 1;
            if self.indented_in {
                self.indented_in = false;
                return;
            }
            self.indented_in = false;
        }
        if self.put_newline {
            self.out.push(b'\n');
        } else {
            self.put_newline = true;
        }
        self.out.extend_from_slice(self.prefix.as_bytes());
        for _ in 0..self.depth {
            self.out.extend_from_slice(self.indent.as_bytes());
        }
        if depth_delta > 0 {
            self.depth += 1;
            self.indented_in = true;
        }
    }

    /// marshal.go:421 `marshalValue` of a struct: `writeStart`, the chardata field
    /// (`EscapeText`, newline escaped), the child elements, `writeEnd`. A Go struct emits its
    /// fields in declaration order; graphify's structs have either chardata or children, so
    /// chardata goes first here.
    fn marshal(&mut self, n: &Node) {
        // marshal.go:732 `writeStart`.
        self.write_indent(1);
        self.out.push(b'<');
        self.out.extend_from_slice(n.name.as_bytes());
        for (k, v) in &n.attrs {
            if k.is_empty() {
                continue;
            }
            self.out.push(b' ');
            self.out.extend_from_slice(k.as_bytes());
            self.out.extend_from_slice(b"=\"");
            escape(&mut self.out, v.as_bytes(), true);
            self.out.push(b'"');
        }
        self.out.push(b'>');

        escape(&mut self.out, n.chardata.as_bytes(), true);
        for c in &n.children {
            self.marshal(c);
        }

        // marshal.go:770 `writeEnd`.
        self.write_indent(-1);
        self.out.extend_from_slice(b"</");
        self.out.extend_from_slice(n.name.as_bytes());
        self.out.push(b'>');
    }
}

/// Go `xml.MarshalIndent(n, prefix, indent)`.
pub fn marshal_indent(n: &Node, prefix: &str, indent: &str) -> Vec<u8> {
    let mut p = Printer {
        out: Vec::new(),
        prefix,
        indent,
        depth: 0,
        indented_in: false,
        put_newline: false,
    };
    p.marshal(n);
    p.out
}

/// Go `xml.EscapeString` (appends to `dst`).
pub fn escape_attr(dst: &mut Vec<u8>, s: &[u8]) {
    // xml.go:1971 `printer.EscapeString` is `escapeText(.., true)` on a string.
    escape(dst, s, true);
}

/// Go `escapeText(w, s, false)` (appends to `dst`; newline kept raw).
pub fn escape_text(dst: &mut Vec<u8>, s: &[u8]) {
    escape(dst, s, false);
}

/// xml.go:1924 `escapeText`: each bad UTF-8 byte (Go `DecodeRune` gives `(RuneError, 1)`) and
/// each rune outside the XML character range becomes U+FFFD.
fn escape(dst: &mut Vec<u8>, s: &[u8], escape_newline: bool) {
    let mut last = 0;
    let mut i = 0;
    while i < s.len() {
        let (r, width) = decode_rune(&s[i..]);
        i += width;
        let esc: &[u8] = match r {
            '"' => b"&#34;",
            '\'' => b"&#39;",
            '&' => b"&amp;",
            '<' => b"&lt;",
            '>' => b"&gt;",
            '\t' => b"&#x9;",
            '\n' if escape_newline => b"&#xA;",
            '\n' => continue,
            '\r' => b"&#xD;",
            _ if !is_in_character_range(r as u32) || (r == RUNE_ERROR && width == 1) => {
                "\u{FFFD}".as_bytes()
            }
            _ => continue,
        };
        dst.extend_from_slice(&s[last..i - width]);
        dst.extend_from_slice(esc);
        last = i;
    }
    dst.extend_from_slice(&s[last..]);
}
