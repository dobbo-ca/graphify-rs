//! Port of `ts.go`. Extractors use `gositter::Node` directly: the Go adapter's methods map 1:1.

use gositter::{Language, Node};

/// Go `ts.Budget`: `GRAPHIFY_PARSE_BUDGET` (default 65,536), read once.
pub fn budget() -> u32 {
    todo!("graphify-rs-ihc.15")
}

/// Go `limit`: the budget scaled by the source size (`ts.go:100-104`), at most `u32::MAX`.
pub fn limit(n: usize) -> u32 {
    todo!("graphify-rs-ihc.15")
}

/// `limit` for a given budget: Go's `TestLimit` sets the `Budget` var, which here is read once.
pub(crate) fn limit_with(budget: u32, n: usize) -> u32 {
    todo!("graphify-rs-ihc.15")
}

/// Go `ts.Parse`: parses `src` with `lang` on a pooled parser (`set_language` before every
/// parse) and runs `f` on the root; `None` is Go's `ErrBudget`.
pub fn parse<R>(src: &[u8], lang: &'static Language, f: impl FnOnce(Node<'_>) -> R) -> Option<R> {
    todo!("graphify-rs-ihc.15")
}
