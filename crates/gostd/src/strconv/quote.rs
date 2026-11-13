//! Go `strconv.Quote` (quote.go, Go 1.27.4) with quote `"`, ASCIIonly false, graphicOnly false.

use crate::unicode::is_print;
use crate::utf8::{RUNE_ERROR, decode_rune};

const LOWERHEX: &[u8; 16] = b"0123456789abcdef";

/// Go `appendQuotedWith` (quote.go:36), invalid bytes as `\xNN`.
pub(super) fn append_quote(buf: &mut Vec<u8>, mut s: &[u8]) {
    buf.push(b'"');
    while !s.is_empty() {
        let (r, width) = decode_rune(s);
        if width == 1 && r == RUNE_ERROR {
            buf.extend_from_slice(br"\x");
            buf.push(LOWERHEX[usize::from(s[0] >> 4)]);
            buf.push(LOWERHEX[usize::from(s[0] & 0xF)]);
        } else {
            append_escaped_rune(buf, r);
        }
        s = &s[width..];
    }
    buf.push(b'"');
}

/// Go `appendEscapedRune` (quote.go:70) for quote `"`.
fn append_escaped_rune(buf: &mut Vec<u8>, r: char) {
    if r == '"' || r == '\\' {
        buf.push(b'\\');
        buf.push(r as u8);
        return;
    }
    if is_print(r) {
        let mut b = [0; 4];
        buf.extend_from_slice(r.encode_utf8(&mut b).as_bytes());
        return;
    }
    let c = r as u32;
    match r {
        '\x07' => buf.extend_from_slice(br"\a"),
        '\x08' => buf.extend_from_slice(br"\b"),
        '\x0c' => buf.extend_from_slice(br"\f"),
        '\n' => buf.extend_from_slice(br"\n"),
        '\r' => buf.extend_from_slice(br"\r"),
        '\t' => buf.extend_from_slice(br"\t"),
        '\x0b' => buf.extend_from_slice(br"\v"),
        _ if c < 0x20 || c == 0x7f => {
            buf.extend_from_slice(br"\x");
            buf.push(LOWERHEX[(c >> 4) as usize & 0xF]);
            buf.push(LOWERHEX[(c & 0xF) as usize]);
        }
        _ if c < 0x10000 => {
            buf.extend_from_slice(br"\u");
            for sh in [12, 8, 4, 0] {
                buf.push(LOWERHEX[(c >> sh) as usize & 0xF]);
            }
        }
        _ => {
            buf.extend_from_slice(br"\U");
            for sh in [28, 24, 20, 16, 12, 8, 4, 0] {
                buf.push(LOWERHEX[(c >> sh) as usize & 0xF]);
            }
        }
    }
}
