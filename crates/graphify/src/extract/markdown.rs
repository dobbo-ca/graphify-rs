//! Port of `markdown.go`: the regexp-based Markdown extractor. Owner: the extractors-and-CLI
//! stream; filled by graphify-rs-ihc.17.

use std::collections::BTreeMap;

use super::extract::Result;

/// Go `extractMarkdown` (no parse, so no parse budget).
pub(crate) fn extract_markdown(rel: &str, src: &[u8]) -> Result {
    todo!("graphify-rs-ihc.17")
}

/// Go `splitFrontmatter`: the flat keys of a leading `---` YAML block and the body after it (an
/// exact suffix of `src`). With no block, or an unterminated one, no keys and all of `src`.
// graphify-rs-ihc.17: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub(crate) fn split_frontmatter(src: &str) -> (BTreeMap<String, String>, &str) {
    todo!("graphify-rs-ihc.17")
}

#[cfg(test)]
#[path = "tests/markdown.rs"]
mod markdown_tests;

#[cfg(test)]
#[path = "tests/upstream_markdown_code_spans.rs"]
mod upstream_markdown_code_spans_tests;
