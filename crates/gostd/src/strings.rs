//! The Go `strings` and `bytes` functions graphify-go calls whose result depends on Unicode
//! tables. Inputs are `&str` (always valid UTF-8 in Rust) except `to_valid_utf8` and
//! `bytes_trim_space`, whose Go call sites can see invalid bytes.
#![allow(unused_variables)] // graphify-rs-aa6.3: stubs until the code PR

/// Go `strings.ToLower`: per-rune `unicode::to_lower`, no final sigma, no expansions.
pub fn to_lower(s: &str) -> String {
    todo!("graphify-rs-aa6.3")
}

/// Go `strings.ToUpper`: per-rune `unicode::to_upper`.
pub fn to_upper(s: &str) -> String {
    todo!("graphify-rs-aa6.3")
}

/// Go `strings.EqualFold`: equal under simple case folding.
pub fn equal_fold(a: &str, b: &str) -> bool {
    todo!("graphify-rs-aa6.3")
}

/// Go `strings.TrimSpace`: trims `unicode::is_space` runes at both ends.
pub fn trim_space(s: &str) -> &str {
    todo!("graphify-rs-aa6.3")
}

/// Go `strings.Fields`: splits around runs of `unicode::is_space`.
pub fn fields(s: &str) -> Vec<&str> {
    todo!("graphify-rs-aa6.3")
}

/// Go `strings.Map`: `None` drops the rune (Go: a negative return).
pub fn map(s: &str, f: impl FnMut(char) -> Option<char>) -> String {
    todo!("graphify-rs-aa6.3")
}

/// Go `strings.ToValidUTF8`: each run of invalid bytes becomes `replacement` once.
pub fn to_valid_utf8(s: &[u8], replacement: &str) -> String {
    todo!("graphify-rs-aa6.3")
}

/// Go `bytes.TrimSpace`.
pub fn bytes_trim_space(s: &[u8]) -> &[u8] {
    todo!("graphify-rs-aa6.3")
}
