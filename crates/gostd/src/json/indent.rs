//! Go `v2_indent.go` (the active `HTMLEscape`, `Compact` and `Indent` under GOEXPERIMENT=jsonv2).

use super::Error;
use super::jsontext::{Opts, append_format};

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Appends Go's `\u00XX` escape of the byte `c`, as `appendHTMLEscape` and
/// `jsonwire.appendEscapedASCII` do for `<`, `>` and `&`.
pub(super) fn push_u00(dst: &mut Vec<u8>, c: u8) {
    dst.extend_from_slice(&[
        b'\\',
        b'u',
        b'0',
        b'0',
        HEX[usize::from(c >> 4)],
        HEX[usize::from(c & 0xF)],
    ]);
}

/// Go `appendHTMLEscape` (`v2_indent.go:26`); Go `HTMLEscape` (`v2_indent.go:21`) is not
/// reached, this exists for the Go tests that call it.
pub fn html_escape(dst: &mut Vec<u8>, src: &[u8]) {
    // The characters can only appear in string literals,
    // so just scan the string one byte at a time.
    let mut start = 0;
    for (i, &c) in src.iter().enumerate() {
        if matches!(c, b'<' | b'>' | b'&') {
            dst.extend_from_slice(&src[start..i]);
            push_u00(dst, c);
            start = i + 1;
        }
        // Convert U+2028 and U+2029 (E2 80 A8 and E2 80 A9).
        if c == 0xE2 && i + 2 < src.len() && src[i + 1] == 0x80 && src[i + 2] & !1 == 0xA8 {
            dst.extend_from_slice(&src[start..i]);
            dst.extend_from_slice(&[
                b'\\',
                b'u',
                b'2',
                b'0',
                b'2',
                HEX[usize::from(src[i + 2] & 0xF)],
            ]);
            start = i + 3;
        }
    }
    dst.extend_from_slice(&src[start..]);
}

/// Go `Compact` (`v2_indent.go:49`): `jsontext.AppendFormat` with duplicate names and invalid
/// UTF-8 allowed and raw strings kept. Go's `Compact` has no escaping; `escape_html` turns on
/// `EscapeForHTML` and `EscapeForJS` (`<`, `>`, `&`, U+2028, U+2029 in strings), as v1's
/// `appendCompact(.., true)` did. On an error `dst` is left as it was.
pub fn compact(dst: &mut Vec<u8>, src: &[u8], escape_html: bool) -> Result<(), Error> {
    append_format(
        dst,
        src,
        &Opts {
            decoder: false,
            indent: None,
            escape_html,
            escape_js: escape_html,
        },
    )
}

/// Go `appendIndent` (`v2_indent.go:91`). A prefix or indent with a byte other than space or tab
/// is formatted with spaces of the same length, which are then overwritten (`:94-111`, run over
/// the whole output, trailing whitespace included); the trailing whitespace of `src` is put back
/// (`:127-129`). On an error `dst` is left as it was.
pub fn indent(dst: &mut Vec<u8>, src: &[u8], prefix: &str, indent: &str) -> Result<(), Error> {
    let orig = dst.len();
    let not_space = |s: &str| s.bytes().any(|c| c != b' ' && c != b'\t');
    let placeholder = not_space(prefix) || not_space(indent);
    let (p, i) = if placeholder {
        (" ".repeat(prefix.len()), " ".repeat(indent.len()))
    } else {
        (prefix.to_string(), indent.to_string())
    };
    append_format(
        dst,
        src,
        &Opts {
            decoder: false,
            indent: Some((p.as_str(), i.as_str())),
            escape_html: false,
            escape_js: false,
        },
    )?;

    // In v2, trailing whitespace is discarded, while v1 preserved it.
    let n = src
        .iter()
        .rev()
        .take_while(|&&c| matches!(c, b' ' | b'\n' | b'\r' | b'\t'))
        .count();
    dst.extend_from_slice(&src[src.len() - n..]);

    if placeholder {
        let b = &mut dst[orig..];
        let mut at = 0;
        while let Some(j) = b[at..].iter().position(|&c| c == b'\n') {
            at += j + 1;
            let n = b[at..].iter().take_while(|&&c| c == b' ').count(); // len(prefix)+n*len(indent)
            let spaces = &mut b[at..at + n];
            let k = prefix.len().min(n);
            spaces[..k].copy_from_slice(&prefix.as_bytes()[..k]);
            // Go copies `indent` until the spaces are filled; with an empty indent and spaces left
            // (only from trailing whitespace) Go never returns. This stops instead.
            for (s, &c) in spaces[k..].iter_mut().zip(indent.as_bytes().iter().cycle()) {
                *s = c;
            }
            at += n;
        }
    }
    Ok(())
}
