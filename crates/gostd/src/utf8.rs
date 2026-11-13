//! Go `unicode/utf8` decoding on bytes that may be invalid UTF-8. Go decodes each invalid byte
//! as `(RUNE_ERROR, 1)`; Rust's `from_utf8_lossy` folds a bad sequence into one U+FFFD, so it is
//! not used here. `utf8.Valid` is `std::str::from_utf8(b).is_ok()` (the same set: no surrogates,
//! no overlongs, nothing above U+10FFFF).

/// U+FFFD, Go `utf8.RuneError`.
pub const RUNE_ERROR: char = '\u{FFFD}';

/// Go `utf8.UTFMax`.
const UTF_MAX: usize = 4;

/// Go `utf8.DecodeRune`: the first rune of `b` and its width. Empty input gives
/// `(RUNE_ERROR, 0)`; an invalid byte gives `(RUNE_ERROR, 1)`.
pub fn decode_rune(b: &[u8]) -> (char, usize) {
    if b.is_empty() {
        return (RUNE_ERROR, 0);
    }
    // Rust and Go accept the same sequences, so the first char of the valid prefix is Go's rune,
    // and an invalid first sequence is Go's (RuneError, 1).
    let head = &b[..b.len().min(UTF_MAX)];
    match head
        .utf8_chunks()
        .next()
        .and_then(|c| c.valid().chars().next())
    {
        Some(c) => (c, c.len_utf8()),
        None => (RUNE_ERROR, 1),
    }
}

/// Go `utf8.DecodeLastRune`: the last rune of `b` and its width.
pub fn decode_last_rune(b: &[u8]) -> (char, usize) {
    let end = b.len();
    if end == 0 {
        return (RUNE_ERROR, 0);
    }
    if b[end - 1] < 0x80 {
        return (b[end - 1] as char, 1);
    }
    // Back up to the start byte, at most UTFMax bytes (Go: lim = end - UTFMax).
    let lim = end.saturating_sub(UTF_MAX);
    let mut start = end - 1;
    while start > lim {
        start -= 1;
        if b[start] & 0xC0 != 0x80 {
            break;
        }
    }
    let (r, size) = decode_rune(&b[start..end]);
    if start + size != end {
        return (RUNE_ERROR, 1);
    }
    (r, size)
}

/// Go `utf8.RuneCount`: each invalid byte counts as one rune.
pub fn rune_count(b: &[u8]) -> usize {
    let (mut i, mut n) = (0, 0);
    while i < b.len() {
        i += decode_rune(&b[i..]).1;
        n += 1;
    }
    n
}
