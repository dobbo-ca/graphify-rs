//! Go `unicode/utf8` decoding on bytes that may be invalid UTF-8. Go decodes each invalid byte
//! as `(RUNE_ERROR, 1)`; Rust's `from_utf8_lossy` folds a bad sequence into one U+FFFD, so it is
//! not used here. `utf8.Valid` is `std::str::from_utf8(b).is_ok()` (the same set: no surrogates,
//! no overlongs, nothing above U+10FFFF).
#![allow(unused_variables)] // graphify-rs-aa6.3: stubs until the code PR

/// U+FFFD, Go `utf8.RuneError`.
pub const RUNE_ERROR: char = '\u{FFFD}';

/// Go `utf8.DecodeRune`: the first rune of `b` and its width. Empty input gives
/// `(RUNE_ERROR, 0)`; an invalid byte gives `(RUNE_ERROR, 1)`.
pub fn decode_rune(b: &[u8]) -> (char, usize) {
    todo!("graphify-rs-aa6.3")
}

/// Go `utf8.DecodeLastRune`: the last rune of `b` and its width.
pub fn decode_last_rune(b: &[u8]) -> (char, usize) {
    todo!("graphify-rs-aa6.3")
}

/// Go `utf8.RuneCount`: each invalid byte counts as one rune.
pub fn rune_count(b: &[u8]) -> usize {
    todo!("graphify-rs-aa6.3")
}
