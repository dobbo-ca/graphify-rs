//! Port of `idutil.go`: node ids.

/// Go `idutil.MakeID`: the parts joined, case-folded and NFKC-normalised to a fixpoint, with
/// every run of non-letter, non-digit, non-underscore runes made one `_`.
pub fn make_id(parts: &[&str]) -> String {
    todo!("graphify-rs-ihc.4")
}

/// Go `idutil.NormalizeID`: the cleaning step of [`make_id`] on one string.
pub fn normalize_id(s: &str) -> String {
    todo!("graphify-rs-ihc.4")
}

/// Go `cleanASCII`: [`clean_unicode`] for an ASCII-only string; `None` for any other.
pub(super) fn clean_ascii(s: &str) -> Option<String> {
    todo!("graphify-rs-ihc.4")
}

/// Go `cleanUnicode`: fold and NFKC to a fixpoint (at most 6 rounds), then the two regexps.
pub(super) fn clean_unicode(s: &str) -> String {
    todo!("graphify-rs-ihc.4")
}
