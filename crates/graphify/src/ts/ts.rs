//! Port of `ts.go`. Extractors use `gositter::Node` directly: the Go adapter's methods map 1:1.

use std::cell::RefCell;
use std::sync::OnceLock;

use gositter::{Language, Node, Parser};

/// Go `ts.Budget`: the most parser steps in a row without progress for a source of up to 4 KB;
/// 0 is no limit. `GRAPHIFY_PARSE_BUDGET` (Go `strconv.ParseUint(s, 10, 32)`), default 65,536,
/// read once.
pub fn budget() -> u32 {
    static BUDGET: OnceLock<u32> = OnceLock::new();
    *BUDGET.get_or_init(|| {
        std::env::var("GRAPHIFY_PARSE_BUDGET")
            .ok()
            .and_then(|s| gostd::strconv::parse_uint(&s, 10, 32).ok())
            .map_or(65536, |n| n as u32)
    })
}

/// Go `limit`: the budget scaled by the source size (`ts.go:100-104`), at most `u32::MAX`.
pub fn limit(n: usize) -> u32 {
    limit_with(budget(), n)
}

/// `limit` for a given budget: Go's `TestLimit` sets the `Budget` var, which here is read once.
/// 4 times at 16 KB, back to 1 from 64 KB: a stalled step can cost time in proportion to the
/// size, so a large file gets no more steps.
pub(crate) fn limit_with(budget: u32, n: usize) -> u32 {
    let n = n.max(4096);
    let f = n.min(16384).min(((1usize << 28) / n).max(4096));
    (u64::from(budget) * f as u64 / 4096).min(u64::from(u32::MAX)) as u32
}

thread_local! {
    /// Go's `sync.Pool` of parsers: they keep their arenas between files. A stack, so an `f`
    /// that parses again gets a parser of its own.
    static PARSERS: RefCell<Vec<Parser>> = const { RefCell::new(Vec::new()) };
}

/// Go `ts.Parse`: parses `src` with `lang` on a pooled parser (`set_language` before every
/// parse) and runs `f` on the root; `None` is Go's `ErrBudget`. The tree lives in the parser's
/// arenas, so no node outlives `f`; the parser is reset before it goes back to the pool (an idle
/// parser holds no chunks another could use).
pub fn parse<R>(src: &[u8], lang: &'static Language, f: impl FnOnce(Node<'_>) -> R) -> Option<R> {
    let mut p = PARSERS.with(|ps| ps.borrow_mut().pop()).unwrap_or_default();
    // Go ignores the error too: every grammar here has a supported ABI version.
    let _ = p.set_language(lang);
    let out = p
        .parse_budget(src, limit(src.len()))
        .map(|tree| f(tree.root_node()));
    p.reset();
    PARSERS.with(|ps| ps.borrow_mut().push(p));
    out
}
