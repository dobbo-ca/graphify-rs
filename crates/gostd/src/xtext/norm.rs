//! x/text `unicode/norm` `NFKC.String` and `NFKD.String`, UCD 15.0.0, with x/text's
//! stream-safe rule: after 30 non-starters it inserts U+034F (`composition.go:10`).
#![allow(unused_variables)] // graphify-rs-aa6.4: stubs until the code PR

/// `norm.NFKC.String(s)`.
pub fn nfkc(s: &str) -> String {
    todo!("graphify-rs-aa6.4")
}

/// `norm.NFKD.String(s)`.
pub fn nfkd(s: &str) -> String {
    todo!("graphify-rs-aa6.4")
}
