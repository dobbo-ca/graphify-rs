//! Port of `detect.go`.

/// Go `detect.IsSensitive`: whether the slash-relative path `rel` names a file that must not be
/// read (keys, credentials, env files).
pub fn is_sensitive(rel: &str) -> bool {
    todo!("graphify-rs-ihc.14")
}

/// A file name as Go holds it, made a Rust string (plan R3, D-008): each byte that Go's
/// `utf8.DecodeRune` reads as `RuneError` of width 1 becomes one U+FFFD, decoded rune by rune
/// with `gostd::utf8::decode_rune`. Not `String::from_utf8_lossy`, which makes one U+FFFD of a
/// maximal invalid subpart (`E2 82 41`: Go gives two U+FFFD, lossy one).
pub fn go_lossy(b: &[u8]) -> String {
    todo!("graphify-rs-ihc.14")
}
