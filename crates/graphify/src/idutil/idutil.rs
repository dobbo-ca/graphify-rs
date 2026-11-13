//! Port of `idutil.go`: node ids.

use std::sync::LazyLock;

use gostd::regexp::Regexp;
use gostd::xtext::cases::fold;
use gostd::xtext::norm::nfkc;

/// Go `nonWord` (`idutil.go:17`).
static NON_WORD: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile(r"[^\p{L}\p{N}_]+"));
/// Go `underscores` (`idutil.go:21`).
static UNDERSCORES: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile("_+"));

/// Go `idutil.MakeID`: joins the name parts into a canonical node id. Empty parts are skipped;
/// each part loses its leading and trailing `_` and `.` before the join.
pub fn make_id(parts: &[&str]) -> String {
    let kept: Vec<&str> = parts
        .iter()
        .map(|p| p.trim_matches(['_', '.']))
        .filter(|p| !p.is_empty())
        .collect();
    clean(&kept.join("_"))
}

/// Go `idutil.NormalizeID`: canonicalizes an already-joined id the same way [`make_id`] does.
pub fn normalize_id(s: &str) -> String {
    clean(s)
}

/// Go `clean`: casefold then NFKC to a fixpoint before the non-word filter (the two do not
/// commute: folding can expand a letter into a base plus a combining mark).
fn clean(s: &str) -> String {
    clean_ascii(s).unwrap_or_else(|| clean_unicode(s))
}

/// Go `cleanASCII`: [`clean_unicode`] for an ASCII-only string, where folding lowercases A-Z,
/// NFKC changes nothing and the regexps leave runs of `[a-z0-9]` joined by one `_`; `None` for
/// any other string.
pub(super) fn clean_ascii(s: &str) -> Option<String> {
    let mut b = String::with_capacity(s.len());
    let mut gap = false;
    for &c in s.as_bytes() {
        let c = match c {
            0x80.. => return None,
            b'A'..=b'Z' => c + (b'a' - b'A'),
            b'a'..=b'z' | b'0'..=b'9' => c,
            _ => {
                gap = true;
                continue;
            }
        };
        if gap && !b.is_empty() {
            b.push('_');
        }
        gap = false;
        b.push(char::from(c));
    }
    Some(b)
}

/// Go `cleanUnicode`: `NFKC(fold(s))` until it no longer changes, at most 6 rounds, then every
/// run of non-letter, non-digit, non-underscore runes becomes `_`, runs of `_` become one, and
/// the ends lose their `_`.
pub(super) fn clean_unicode(s: &str) -> String {
    let mut s = s.to_string();
    for _ in 0..6 {
        let next = nfkc(&fold(&s));
        if next == s {
            break;
        }
        s = next;
    }
    let s = NON_WORD.replace_all_string(&s, "_");
    let s = UNDERSCORES.replace_all_string(&s, "_");
    s.trim_matches('_').to_string()
}
