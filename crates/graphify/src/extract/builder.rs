//! The shared extractor toolkit of `extract.go:329-623`: the per-file builder and the tree
//! helpers every language extractor uses. Shared API (owner: the extractors-and-CLI stream);
//! bodies with graphify-rs-ihc.16 (plan Task 15).
//!
//! Go keeps every language's per-file state on one `builder`. Here the builder holds only what
//! its own methods read; a language's extra state (Go `goPkgs`, `javaRets`, `cppTParams`, ...)
//! lives in that extractor's own file, beside a `Builder`.
//!
//! Extractor convention: `pub(crate) fn extract_<lang>(rel: &str, src: &[u8]) -> Option<Result>`
//! in `src/extract/<go file>.rs`; `None` means the parse budget stopped the parse (Go's
//! `ts.ErrBudget` panic), and `file_from_bytes` then returns the file node with `over_budget`
//! set, as Go's `FileFromBytes` recover does.

use std::collections::{HashMap, HashSet};

use gositter::{Language, Node};

use super::extract::{Def, Result};
use crate::model::{File, Loc};

/// Go `builder` (`extract.go:352`): the fields its own methods read.
pub(crate) struct Builder {
    /// Go `file`: the slash-relative path.
    pub(crate) file: String,
    /// Go `src`: `file` as edges hold it.
    pub(crate) src: File,
    pub(crate) file_id: String,
    /// The id prefix of the symbols: the file path.
    pub(crate) stem: String,
    pub(crate) res: Result,
    pub(crate) seen: HashSet<String>,
    /// Go `jsNS`: enclosing JS/TS namespace names (read by `id` and `def`).
    pub(crate) js_ns: Vec<String>,
    /// Go `jsNSDefs`: namespace members, kept if no plain def has the name.
    pub(crate) js_ns_defs: Vec<Def>,
    /// Go `strs`: text -> its number in `res.call_text`.
    strs: HashMap<String, u32>,
}

impl Builder {
    /// Go `newBuilder`: the file node (`idutil.MakeID(rel)`, label `filepath.Base(rel)`, L1).
    pub(crate) fn new(rel: &str) -> Builder {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `addNode`: a node once per id.
    pub(crate) fn add_node(&mut self, id: &str, label: &str, loc: Loc) {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `id`: `make_id(stem, parts...)` inside any JS namespace; the file id itself becomes
    /// `id("blank")`.
    pub(crate) fn id(&self, parts: &[&str]) -> String {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `contains`: a `contains` edge from the file node.
    pub(crate) fn contains(&mut self, child_id: &str, loc: Loc) {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `def`.
    pub(crate) fn def(&mut self, id: &str, name: &str, display_label: &str, loc: Loc) {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `call`.
    pub(crate) fn call(&mut self, caller_id: &str, callee: &str, loc: Loc) {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `text`: the number of `s` in `res.call_text`, added on first use.
    pub(crate) fn text(&mut self, s: &str) -> u32 {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `callRecv`.
    pub(crate) fn call_recv(&mut self, caller_id: &str, callee: &str, recv: &str, loc: Loc) {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `typeRef`.
    pub(crate) fn type_ref(&mut self, from_id: &str, name: &str, relation: &str, loc: Loc) {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `ref` (`ref` is a Rust keyword).
    pub(crate) fn ref_(&mut self, from_id: &str, name: &str, relation: &str, deco: bool, loc: Loc) {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `refQ`.
    pub(crate) fn ref_q(&mut self, from_id: &str, name: &str, relation: &str, q: &str, loc: Loc) {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `imp`.
    pub(crate) fn imp(&mut self, spec: &str, loc: Loc) {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `impTyped`.
    pub(crate) fn imp_typed(&mut self, spec: &str, loc: Loc, type_only: bool) {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `impDeferred`.
    pub(crate) fn imp_deferred(&mut self, spec: &str, loc: Loc, caller: &str) {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `impRel`.
    pub(crate) fn imp_rel(&mut self, spec: &str, loc: Loc) {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `addRationale`.
    pub(crate) fn add_rationale(&mut self, text: &str, line_num: i64, parent_id: &str) {
        todo!("graphify-rs-ihc.16")
    }
}

/// Go `parseRoot`: parses `src` with `lang` and runs `f` on the root; `None` when the parse
/// budget stops the parse.
pub(crate) fn parse_root<R>(
    src: &[u8],
    lang: &'static Language,
    f: impl FnOnce(Node<'_>) -> R,
) -> Option<R> {
    todo!("graphify-rs-ihc.16")
}

/// Go `n.Utf8Text(src)` as a `&str`. gositter-rs returns the bytes; `src` went through
/// `to_utf8`, and a node's byte range ends on a rune boundary (the lexer advances by runes), so
/// the slice is valid UTF-8. A slice that is not is a bug, and panics with the node's range.
pub(crate) fn node_text<'s>(n: Node<'_>, src: &'s [u8]) -> &'s str {
    todo!("graphify-rs-ihc.16")
}

/// Go `line`: the node's start row + 1.
pub(crate) fn line(n: Node<'_>) -> Loc {
    todo!("graphify-rs-ihc.16")
}

/// Go `itoa`.
pub(crate) fn itoa(i: i64) -> String {
    todo!("graphify-rs-ihc.16")
}

/// Go `bindType`.
pub(crate) fn bind_type(types: &mut HashMap<String, String>, name: &str, typ: &str) {
    todo!("graphify-rs-ihc.16")
}

/// Go `recvText` (`expr` and `name` may be the null node).
pub(crate) fn recv_text(expr: Option<Node<'_>>, name: Option<Node<'_>>, src: &[u8]) -> String {
    todo!("graphify-rs-ihc.16")
}

/// Go `baseTypeName`.
pub(crate) fn base_type_name(text: &str) -> String {
    todo!("graphify-rs-ihc.16")
}

/// Go `rationaleLabel`: the first 80 runes, line breaks made spaces, trimmed.
pub(crate) fn rationale_label(text: &str) -> String {
    todo!("graphify-rs-ihc.16")
}

/// Go `walk`: preorder over `n` and its descendants; `f` returns false to skip a node's children.
pub(crate) fn walk<'t>(n: Option<Node<'t>>, f: impl FnMut(Node<'t>) -> bool) {
    todo!("graphify-rs-ihc.16")
}

/// Go `walkKinds`: [`walk`] that calls `f` only for kinds in `mask` (from
/// `extract::kinds`, registered before any parse).
pub(crate) fn walk_kinds<'t>(n: Option<Node<'t>>, mask: u32, f: impl FnMut(Node<'t>) -> bool) {
    todo!("graphify-rs-ihc.16")
}
