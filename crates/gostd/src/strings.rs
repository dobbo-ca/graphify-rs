//! The Go `strings` and `bytes` functions graphify-go calls whose result depends on Unicode
//! tables. Inputs are `&str` (always valid UTF-8 in Rust) except `to_valid_utf8` and
//! `bytes_trim_space`, whose Go call sites can see invalid bytes.

use crate::unicode::{self, is_space, simple_fold};
use crate::utf8::{decode_last_rune, decode_rune};

/// Go `strings.ToLower`: per-rune `unicode::to_lower`, no final sigma, no expansions.
pub fn to_lower(s: &str) -> String {
    s.chars().map(unicode::to_lower).collect()
}

/// Go `strings.ToUpper`: per-rune `unicode::to_upper`.
pub fn to_upper(s: &str) -> String {
    s.chars().map(unicode::to_upper).collect()
}

/// Go `strings.EqualFold`: equal under simple case folding.
pub fn equal_fold(a: &str, b: &str) -> bool {
    let mut t = b.chars();
    for sr in a.chars() {
        let Some(tr) = t.next() else {
            return false;
        };
        if tr == sr {
            continue;
        }
        let (sr, tr) = if tr < sr { (tr, sr) } else { (sr, tr) };
        if tr.is_ascii() {
            // ASCII only: sr and tr must be an upper/lower pair.
            if sr.is_ascii_uppercase() && tr == sr.to_ascii_lowercase() {
                continue;
            }
            return false;
        }
        // SimpleFold(x) is the next equivalent rune above x, or wraps around.
        let mut r = simple_fold(sr);
        while r != sr && r < tr {
            r = simple_fold(r);
        }
        if r != tr {
            return false;
        }
    }
    t.next().is_none()
}

/// Go `strings.TrimSpace`: trims `unicode::is_space` runes at both ends.
pub fn trim_space(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// Go `strings.Fields`: splits around runs of `unicode::is_space`.
pub fn fields(s: &str) -> Vec<&str> {
    s.split(is_space).filter(|f| !f.is_empty()).collect()
}

/// Go `strings.Map`: `None` drops the rune (Go: a negative return).
pub fn map(s: &str, f: impl FnMut(char) -> Option<char>) -> String {
    s.chars().filter_map(f).collect()
}

/// Go `strings.ToValidUTF8`: each run of invalid bytes becomes `replacement` once.
pub fn to_valid_utf8(s: &[u8], replacement: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut invalid = false; // the previous byte was part of an invalid sequence
    let mut i = 0;
    while i < s.len() {
        let (r, width) = decode_rune(&s[i..]);
        if r == crate::utf8::RUNE_ERROR && width == 1 {
            if !invalid {
                invalid = true;
                out.push_str(replacement);
            }
        } else {
            invalid = false;
            out.push(r);
        }
        i += width;
    }
    out
}

/// Go `bytes.TrimSpace`: invalid bytes decode as U+FFFD, which is not space, so they stop the
/// trim.
pub fn bytes_trim_space(s: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < s.len() {
        let (r, width) = decode_rune(&s[start..]);
        if !is_space(r) {
            break;
        }
        start += width;
    }
    let mut end = s.len();
    while end > start {
        let (r, width) = decode_last_rune(&s[start..end]);
        if !is_space(r) {
            break;
        }
        end -= width;
    }
    &s[start..end]
}
