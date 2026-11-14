//! Port of `json.go`: the JSON extractor (gositter's json grammar). Owner: the
//! extractors-and-CLI stream; filled by graphify-rs-ihc.16 part 2 (it parses through `ts::parse`,
//! graphify-rs-ihc.15).

use super::extract::Result;

/// Go `extractJSON`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_json(rel: &str, src: &[u8]) -> Option<Result> {
    todo!("graphify-rs-ihc.16")
}
