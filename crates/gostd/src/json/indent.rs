//! Go `indent.go`.

use super::Error;
use super::scanner::{Op, Scanner};

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Appends Go's `\u00XX` escape of the byte `c`, as `appendHTMLEscape` and `appendCompact` do for
/// `<`, `>` and `&`.
fn push_u00(dst: &mut Vec<u8>, c: u8) {
    dst.extend_from_slice(&[
        b'\\',
        b'u',
        b'0',
        b'0',
        HEX[usize::from(c >> 4)],
        HEX[usize::from(c & 0xF)],
    ]);
}

/// True at `src[i]` when `src[i..i + 3]` is U+2028 or U+2029 (E2 80 A8 and E2 80 A9).
fn is_line_sep(src: &[u8], i: usize) -> bool {
    src[i] == 0xE2 && i + 2 < src.len() && src[i + 1] == 0x80 && src[i + 2] & !1 == 0xA8
}

/// Go `appendHTMLEscape` (`indent.go:21`); Go `HTMLEscape` (`indent.go:16`) is not reached,
/// this exists for the Go tests that call it.
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
        if is_line_sep(src, i) {
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

/// Go `appendCompact` (`indent.go:51`); `escape_html` is its `escape` argument.
pub fn compact(dst: &mut Vec<u8>, src: &[u8], escape_html: bool) -> Result<(), Error> {
    let orig_len = dst.len();
    let mut scan = Scanner::new();
    let mut start = 0;
    for (i, &c) in src.iter().enumerate() {
        if escape_html && matches!(c, b'<' | b'>' | b'&') {
            dst.extend_from_slice(&src[start..i]);
            push_u00(dst, c);
            start = i + 1;
        }
        // Convert U+2028 and U+2029 (E2 80 A8 and E2 80 A9).
        if escape_html && is_line_sep(src, i) {
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
        let v = scan.step(c);
        if v >= Op::SkipSpace {
            if v == Op::Error {
                break;
            }
            dst.extend_from_slice(&src[start.min(i)..i]);
            start = i + 1;
        }
    }
    if scan.eof() == Op::Error {
        dst.truncate(orig_len);
        return Err(scan.err.expect("scanError sets err"));
    }
    if start < src.len() {
        dst.extend_from_slice(&src[start..]);
    }
    Ok(())
}

/// Go `appendNewline` (`indent.go:96`).
fn append_newline(dst: &mut Vec<u8>, prefix: &str, indent: &str, depth: usize) {
    dst.push(b'\n');
    dst.extend_from_slice(prefix.as_bytes());
    for _ in 0..depth {
        dst.extend_from_slice(indent.as_bytes());
    }
}

/// Go `appendIndent` (`indent.go:128`).
pub fn indent(dst: &mut Vec<u8>, src: &[u8], prefix: &str, indent: &str) -> Result<(), Error> {
    let orig_len = dst.len();
    let mut scan = Scanner::new();
    let mut need_indent = false;
    let mut depth = 0usize;
    for &c in src {
        scan.bytes += 1;
        let v = scan.step(c);
        if v == Op::SkipSpace {
            continue;
        }
        if v == Op::Error {
            break;
        }
        if need_indent && v != Op::EndObject && v != Op::EndArray {
            need_indent = false;
            depth += 1;
            append_newline(dst, prefix, indent, depth);
        }

        // Emit semantically uninteresting bytes
        // (in particular, punctuation in strings) unmodified.
        if v == Op::Continue {
            dst.push(c);
            continue;
        }

        // Add spacing around real punctuation.
        match c {
            b'{' | b'[' => {
                // delay indent so that empty object and array are formatted as {} and [].
                need_indent = true;
                dst.push(c);
            }
            b',' => {
                dst.push(c);
                append_newline(dst, prefix, indent, depth);
            }
            b':' => dst.extend_from_slice(&[c, b' ']),
            b'}' | b']' => {
                if need_indent {
                    // suppress indent in empty object/array
                    need_indent = false;
                } else {
                    // Go's int goes to -1 on a `}` or `]` after a complete top-level value
                    // (`{}}`, `1]`); the scanner has already recorded the error, so the output is
                    // discarded below. Saturate instead of underflowing the usize.
                    depth = depth.saturating_sub(1);
                    append_newline(dst, prefix, indent, depth);
                }
                dst.push(c);
            }
            _ => dst.push(c),
        }
    }
    if scan.eof() == Op::Error {
        dst.truncate(orig_len);
        return Err(scan.err.expect("scanError sets err"));
    }
    Ok(())
}
