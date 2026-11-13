//! The shared extractor toolkit of `extract.go:329-623`: the per-file builder and the tree
//! helpers every language extractor uses. Shared API (owner: the extractors-and-CLI stream).
//!
//! Go keeps every language's per-file state on one `builder`. Here the builder holds only what
//! its own methods read; a language's extra state (Go `goPkgs`, `javaRets`, `cppTParams`, ...)
//! lives in that extractor's own file, beside a `Builder`.
//!
//! Tests: `src/extract/tests/builder.rs`.
//!
//! Extractor convention: `pub(crate) fn extract_<lang>(rel: &str, src: &[u8]) -> Option<Result>`
//! in `src/extract/<go file>.rs`; `None` means the parse budget stopped the parse (Go's
//! `ts.ErrBudget` panic), and `file_from_bytes` then returns the file node with `over_budget`
//! set, as Go's `FileFromBytes` recover does.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use gositter::{Language, Node};
use gostd::strings::trim_space;

use super::extract::{Call, DYN_DEFERRED, Def, Imp, Result, TypeRef};
use crate::idutil::make_id;
use crate::model::{Confidence, Edge, File, Loc, Relation, file_of, line as model_line};

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
        let file_id = make_id(&[rel]);
        let mut b = Builder {
            file: rel.to_string(),
            src: file_of(rel),
            file_id: file_id.clone(),
            stem: rel.to_string(),
            res: Result {
                file: rel.to_string(),
                ..Result::default()
            },
            seen: HashSet::new(),
            js_ns: Vec::new(),
            js_ns_defs: Vec::new(),
            strs: HashMap::new(),
        };
        b.add_node(&file_id, &gostd::filepath::base(rel), model_line(1));
        b
    }

    /// Go `addNode`: a `code` node once per id.
    pub(crate) fn add_node(&mut self, id: &str, label: &str, loc: Loc) {
        if !self.seen.insert(id.to_string()) {
            return;
        }
        self.res.nodes.push(crate::model::Node {
            id: id.to_string(),
            label: label.to_string(),
            file_type: "code".to_string(),
            source_file: self.file.clone(),
            source_location: loc.to_string(),
            ..crate::model::Node::default()
        });
    }

    /// Go `id`: `make_id(stem, parts...)`, inside a JS namespace `make_id(stem, "ns", ns...,
    /// parts...)`. A name that is all punctuation must not take the file's id: it becomes
    /// `id("blank")`.
    pub(crate) fn id(&self, parts: &[&str]) -> String {
        let mut all: Vec<&str> = vec![&self.stem];
        if !self.js_ns.is_empty() {
            // ponytail: the "ns" part keeps members off plain ids, a name "ns_x" still collides
            all.push("ns");
            all.extend(self.js_ns.iter().map(String::as_str));
        }
        all.extend_from_slice(parts);
        let id = make_id(&all);
        if id == self.file_id {
            // ponytail: every such name in a file shares one node, as before
            return self.id(&["blank"]);
        }
        id
    }

    /// Go `contains`: a `contains` edge from the file node.
    pub(crate) fn contains(&mut self, child_id: &str, loc: Loc) {
        self.res.edges.push(Edge {
            source: self.file_id.clone(),
            target: child_id.to_string(),
            relation: Relation::CONTAINS,
            confidence: Confidence::EXTRACTED,
            source_file: self.src,
            source_location: loc,
            ..Edge::default()
        });
    }

    /// Go `def`: the node, a `contains` edge from the file, and a definition for resolution
    /// (inside a JS namespace, kept aside in `js_ns_defs`).
    pub(crate) fn def(&mut self, id: &str, name: &str, display_label: &str, loc: Loc) {
        self.add_node(id, display_label, loc);
        self.contains(id, loc);
        let d = Def {
            id: id.to_string(),
            name: name.to_string(),
            file: self.file.clone(),
            ..Def::default()
        };
        if !self.js_ns.is_empty() {
            self.js_ns_defs.push(d);
            return;
        }
        self.res.defs.push(d);
    }

    /// Go `call`: a bare call; nothing when the caller or the callee is empty.
    pub(crate) fn call(&mut self, caller_id: &str, callee: &str, loc: Loc) {
        self.call_recv_text(caller_id, callee, None, loc);
    }

    /// Go `text`: the number of `s` in `res.call_text`, added on first use (0 is `""`).
    pub(crate) fn text(&mut self, s: &str) -> u32 {
        if self.strs.is_empty() {
            self.strs.insert(String::new(), 0);
            self.res.call_text = vec![String::new()];
        }
        if let Some(&i) = self.strs.get(s) {
            return i;
        }
        let i = self.res.call_text.len() as u32;
        self.strs.insert(s.to_string(), i);
        self.res.call_text.push(s.to_string());
        i
    }

    /// Go `callRecv`: a member or qualified call with its receiver text.
    pub(crate) fn call_recv(&mut self, caller_id: &str, callee: &str, recv: &str, loc: Loc) {
        self.call_recv_text(caller_id, callee, Some(recv), loc);
    }

    fn call_recv_text(&mut self, caller_id: &str, callee: &str, recv: Option<&str>, loc: Loc) {
        if callee.is_empty() || caller_id.is_empty() {
            return;
        }
        // Go numbers the caller, then the callee, then the receiver.
        let caller_id = self.text(caller_id);
        let callee = self.text(callee);
        let recv = recv.map_or(0, |r| self.text(r));
        self.res.calls.push(Call {
            caller_id,
            callee,
            loc,
            recv,
        });
    }

    /// Go `typeRef`: a supertype reference for resolve to bind by name.
    pub(crate) fn type_ref(&mut self, from_id: &str, name: &str, relation: &str, loc: Loc) {
        if from_id.is_empty() || name.is_empty() {
            return;
        }
        self.res.type_refs.push(TypeRef {
            from_id: from_id.to_string(),
            name: name.to_string(),
            relation: relation.to_string(),
            file: self.file.clone(),
            loc,
            ..TypeRef::default()
        });
    }

    /// Go `ref` (`ref` is a Rust keyword): a name in a type position or a decorator; resolve
    /// binds it or points the edge at a stub node.
    pub(crate) fn ref_(&mut self, from_id: &str, name: &str, relation: &str, deco: bool, loc: Loc) {
        if from_id.is_empty() || name.is_empty() {
            return;
        }
        self.res.type_refs.push(TypeRef {
            from_id: from_id.to_string(),
            name: name.to_string(),
            relation: relation.to_string(),
            file: self.file.clone(),
            loc,
            stub: true,
            deco,
            ..TypeRef::default()
        });
    }

    /// Go `refQ`: `ref_` for a name written with the qualifier `q`.
    pub(crate) fn ref_q(&mut self, from_id: &str, name: &str, relation: &str, q: &str, loc: Loc) {
        let before = self.res.type_refs.len();
        self.ref_(from_id, name, relation, false, loc);
        if let Some(t) = self.res.type_refs.get_mut(before) {
            t.qual = q.to_string();
        }
    }

    /// Go `imp`.
    pub(crate) fn imp(&mut self, spec: &str, loc: Loc) {
        self.imp_typed(spec, loc, false);
    }

    /// Go `impTyped`: `imp` with the TypeScript `import type` flag.
    pub(crate) fn imp_typed(&mut self, spec: &str, loc: Loc, type_only: bool) {
        if spec.is_empty() {
            return;
        }
        self.res.imps.push(Imp {
            file_id: self.file_id.clone(),
            file: self.file.clone(),
            spec: spec.to_string(),
            loc,
            type_only,
            ..Imp::default()
        });
    }

    /// Go `impDeferred`: an `import()` inside `caller`, a function of the file.
    pub(crate) fn imp_deferred(&mut self, spec: &str, loc: Loc, caller: &str) {
        if spec.is_empty() || caller.is_empty() {
            return;
        }
        self.res.imps.push(Imp {
            file_id: caller.to_string(),
            file: self.file.clone(),
            spec: spec.to_string(),
            loc,
            dynamic: DYN_DEFERRED,
            ..Imp::default()
        });
    }

    /// Go `impRel`: `imp` for a spec relative to the importing file's directory.
    pub(crate) fn imp_rel(&mut self, spec: &str, loc: Loc) {
        if spec.is_empty() {
            return;
        }
        self.res.imps.push(Imp {
            file_id: self.file_id.clone(),
            file: self.file.clone(),
            spec: spec.to_string(),
            loc,
            rel: true,
            ..Imp::default()
        });
    }

    /// Go `addRationale`: a `rationale` node keyed on its line and a `rationale_for` edge from
    /// it to `parent_id`.
    pub(crate) fn add_rationale(&mut self, text: &str, line_num: i64, parent_id: &str) {
        let rid = self.id(&["rationale", &itoa(line_num)]);
        let loc = model_line(line_num);
        if self.seen.insert(rid.clone()) {
            self.res.nodes.push(crate::model::Node {
                id: rid.clone(),
                label: rationale_label(text),
                file_type: "rationale".to_string(),
                source_file: self.file.clone(),
                source_location: loc.to_string(),
                ..crate::model::Node::default()
            });
        }
        self.res.edges.push(Edge {
            source: rid,
            target: parent_id.to_string(),
            relation: Relation::RATIONALE_FOR,
            confidence: Confidence::EXTRACTED,
            source_file: self.src,
            source_location: loc,
            ..Edge::default()
        });
    }
}

/// Go `parseRoot`: parses `src` with `lang` and runs `f` on the root; `None` when the parse
/// budget stops the parse.
pub(crate) fn parse_root<R>(
    src: &[u8],
    lang: &'static Language,
    f: impl FnOnce(Node<'_>) -> R,
) -> Option<R> {
    crate::ts::parse(src, lang, f)
}

/// Go `n.Utf8Text(src)` as text. gositter-rs returns the bytes; `src` went through `to_utf8`
/// and the lexer advances by whole runes, so a node's range ends on a rune boundary and the text
/// is borrowed. If it ever is not (a bug, or an odd ERROR/MISSING range), a debug build fails
/// naming the range; a release build keeps going with [`go_text`], as Go keeps going with a
/// string of invalid bytes.
pub(crate) fn node_text<'s>(n: Node<'_>, src: &'s [u8]) -> Cow<'s, str> {
    let b = n.utf8_text(src);
    if std::str::from_utf8(b).is_err() && cfg!(debug_assertions) {
        panic!(
            "node text {}..{} is not valid UTF-8",
            n.start_byte(),
            n.end_byte()
        );
    }
    go_text(b)
}

/// The text Go holds for these bytes: borrowed when they are valid UTF-8, else
/// `detect::go_lossy` (one U+FFFD per byte Go's `DecodeRune` reads as `RuneError` of width 1,
/// which graph.json writes as Go does, plan R3). No assert: [`node_text`] has it.
pub(crate) fn go_text(b: &[u8]) -> Cow<'_, str> {
    match std::str::from_utf8(b) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => Cow::Owned(crate::detect::go_lossy(b)),
    }
}

/// Go `line`: the node's start row + 1.
pub(crate) fn line(n: Node<'_>) -> Loc {
    model_line(i64::from(n.start_position().row) + 1)
}

/// Go `itoa`: decimal digits of a positive number, `"0"` for zero, `""` for a negative one
/// (Go's loop never runs).
pub(crate) fn itoa(i: i64) -> String {
    if i < 0 {
        return String::new();
    }
    i.to_string()
}

/// Go `bindType`: `name` holds a `typ` value; a second, different binding clears it.
pub(crate) fn bind_type(types: &mut HashMap<String, String>, name: &str, typ: &str) {
    let typ = match types.get(name) {
        Some(old) if old != typ => "",
        _ => typ,
    };
    types.insert(name.to_string(), typ.to_string());
}

/// Go `recvSeps`: the member-access operators that end a receiver, longest first.
const RECV_SEPS: [&str; 6] = ["?->", "?.", "->", "::", ".", ":"];

/// Go `recvText`: the source between the start of `expr` and its trailing `name`, minus the
/// access operator.
pub(crate) fn recv_text(expr: Option<Node<'_>>, name: Option<Node<'_>>, src: &[u8]) -> String {
    let (Some(expr), Some(name)) = (expr, name) else {
        return String::new();
    };
    let raw = go_text(&src[expr.start_byte() as usize..name.start_byte() as usize]);
    let s = trim_space(&raw);
    for sep in RECV_SEPS {
        if let Some(rest) = s.strip_suffix(sep) {
            return trim_space(rest).to_string();
        }
    }
    s.to_string()
}

/// Go `baseTypeName`: generic or constructor arguments cut (`Foo<Bar>` -> `Foo`), a qualified
/// path's last segment kept (`p.q.A` -> `A`).
pub(crate) fn base_type_name(text: &str) -> String {
    let text = match text.find(['<', '[', '(']) {
        Some(i) => &text[..i],
        None => text,
    };
    let text = match text.rfind('.') {
        Some(i) => &text[i + 1..],
        None => text,
    };
    trim_space(text).to_string()
}

/// Go `rationaleLabel`: the first 80 runes, line breaks made spaces, trimmed.
pub(crate) fn rationale_label(text: &str) -> String {
    let text: String = text.chars().take(80).collect();
    let text = text
        .replace("\r\n", " ")
        .replace('\r', " ")
        .replace('\n', " ");
    trim_space(&text).to_string()
}

/// Go `walk`: preorder over `n` and its descendants; `f` returns false to skip a node's
/// children. A null node visits nothing.
pub(crate) fn walk<'t>(n: Option<Node<'t>>, f: impl FnMut(Node<'t>) -> bool) {
    if let Some(n) = n {
        n.preorder(f);
    }
}

/// Go `walkKinds`: [`walk`] that calls `f` only for kinds in `mask` (from the language's
/// `register_kinds`, named before any parse).
pub(crate) fn walk_kinds<'t>(n: Option<Node<'t>>, mask: u32, f: impl FnMut(Node<'t>) -> bool) {
    if let Some(n) = n {
        n.preorder_mask(mask, f);
    }
}
