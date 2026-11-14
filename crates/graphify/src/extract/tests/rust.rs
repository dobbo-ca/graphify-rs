//! graphify-go `internal/extract/rust_test.go`, and the cases of the upstream graphify v0.9.107
//! `test_rust_*.py` files whose assertion holds for graphify-go v0.12.0 (see the `extract-rust`
//! block of `parity/README.md`). The upstream sources are written inline, so no fixture is copied.
//! Upstream names the nodes of an impl method `.name()`; Go labels them `Type.name()`.

use std::collections::{BTreeMap, BTreeSet};

use crate::extract::{file, file_from_bytes, kinds, resolve};
use crate::model::{Edge, Extraction, Node};
use crate::testutil::testdata;

/// Go `File` on each file of `testdata/extract/<proj>`, then `Resolve` (GOMAXPROCS workers).
fn extract_proj(proj: &str, files: &[&str]) -> Extraction {
    kinds::register_kinds();
    let root = testdata().join("extract").join(proj);
    let results: Vec<_> = files
        .iter()
        .map(|f| file(&root, f).unwrap_or_else(|e| panic!("File({f}): {e}")))
        .collect();
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_extract_rust() {
    let ext = extract_proj("rsproj", &["util/math.rs", "web/server.rs"]);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in [
        "math.rs",
        "server.rs",
        "add()",
        "Server",
        "Server.start()",
        "boot()",
    ] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }

    let has = |src: &str, rel: &str, tgt: &str| {
        ext.edges.iter().any(|e| {
            e.relation.as_str() == rel
                && id2label.get(e.source.as_str()) == Some(&src)
                && id2label.get(e.target.as_str()) == Some(&tgt)
        })
    };
    // Inherent impl method scoped under its type, with a contains edge.
    assert!(
        has("Server", "contains", "Server.start()"),
        "expected Server --contains--> Server.start"
    );
    // Same-file call: Server.start -> boot.
    assert!(
        has("Server.start()", "calls", "boot()"),
        "expected Server.start --calls--> boot"
    );
    // Cross-file call: boot -> add (unique global definition).
    assert!(
        has("boot()", "calls", "add()"),
        "expected cross-file boot --calls--> add"
    );
}

// ---- upstream graphify v0.9.107 `test_rust_*.py` ----

/// A call edge as (source label, target label, target `source_location`).
type CallAt = (String, String, String);

/// Go `FileFromBytes` on each `(rel, src)`, then `Resolve` (GOMAXPROCS workers).
fn run(srcs: &[(&str, &str)]) -> Extraction {
    kinds::register_kinds();
    let results: Vec<_> = srcs
        .iter()
        .map(|(rel, src)| file_from_bytes(rel, src.as_bytes()))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(rel, _)| (*rel).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

fn label_set(ext: &Extraction) -> BTreeSet<&str> {
    ext.nodes.iter().map(|n| n.label.as_str()).collect()
}

fn id2label(ext: &Extraction) -> BTreeMap<&str, &str> {
    ext.nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect()
}

/// (source label, target label) of every edge of `relation`; a missing node is "".
fn pairs(ext: &Extraction, relation: &str) -> BTreeSet<(String, String)> {
    let label = id2label(ext);
    let of = |id: &str| label.get(id).copied().unwrap_or("").to_string();
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == relation)
        .map(|e| (of(&e.source), of(&e.target)))
        .collect()
}

fn has_edge(ext: &Extraction, src: &str, relation: &str, tgt: &str) -> bool {
    pairs(ext, relation).contains(&(src.to_string(), tgt.to_string()))
}

/// Every `calls` edge as (source label, target label, target location).
fn calls_at(ext: &Extraction) -> BTreeSet<CallAt> {
    let by_id: BTreeMap<&str, &Node> = ext.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == "calls")
        .filter_map(|e| {
            let s = by_id.get(e.source.as_str())?;
            let t = by_id.get(e.target.as_str())?;
            Some((s.label.clone(), t.label.clone(), t.source_location.clone()))
        })
        .collect()
}

fn has_call(calls: &BTreeSet<CallAt>, src: &str, tgt: &str, loc: &str) -> bool {
    calls.contains(&(src.to_string(), tgt.to_string(), loc.to_string()))
}

/// The labels the `calls` edges from the node `label` of `file` point at.
fn calls_from(ext: &Extraction, file: &str, label: &str) -> Vec<String> {
    let id = ext
        .nodes
        .iter()
        .find(|n| n.source_file == file && n.label == label)
        .map(|n| n.id.as_str())
        .unwrap_or_else(|| panic!("no node {label:?} in {file}"));
    let name = id2label(ext);
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == "calls" && e.source == id)
        .map(|e| {
            name.get(e.target.as_str())
                .copied()
                .unwrap_or("")
                .to_string()
        })
        .collect()
}

/// The first path segment of a node's source file: the crate of `crate_use_scope`.
fn crate_of<'a>(file: &BTreeMap<&str, &'a str>, id: &str) -> Option<&'a str> {
    file.get(id)
        .copied()
        .and_then(|f| f.split('/').next())
        .filter(|c| !c.is_empty())
}

/// Edges other than contains, case_of and method from a node of crate `src` to one of `dst`.
fn edges_between<'a>(ext: &'a Extraction, src: &str, dst: &str) -> Vec<&'a Edge> {
    let file: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.source_file.as_str()))
        .collect();
    ext.edges
        .iter()
        .filter(|e| {
            !matches!(e.relation.as_str(), "contains" | "case_of" | "method")
                && crate_of(&file, &e.source) == Some(src)
                && crate_of(&file, &e.target) == Some(dst)
        })
        .collect()
}

// test_rust_anonymous_const.py: Go extracts no `const` or `static` item, so the WIDTH and READY
// labels are not asserted.

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_anonymous_constants_preserve_file_identity() {
    let ext = run(&[(
        "codec.rs",
        "pub const WIDTH: usize = 8;\n\
         const _: () = assert!(WIDTH == 8);\n\
         const _: () = ();\n\
         static READY: bool = true;\n\
         pub fn encode() {}\n",
    )]);
    let ids: BTreeSet<&str> = ext.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids.len(), ext.nodes.len(), "duplicate node ids");
    let labels = label_set(&ext);
    for want in ["codec.rs", "encode()"] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }
    assert!(!labels.contains("_"));
    assert!(ext.edges.iter().all(|e| e.source != e.target));
}

// test_rust_builtin_type_hubs.py

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_rust_prelude_types_produce_no_nodes() {
    let ext = run(&[
        (
            "a.rs",
            "pub fn parse(items: Vec<String>, name: Option<String>) -> Result<u32, String> {\n    \
             let mut seen: std::collections::HashMap<String, u32> = std::collections::HashMap::new();\n    \
             let _ = &mut seen;\n    \
             Ok(name.map(|_| 1u32).unwrap_or(0))\n\
             }\n",
        ),
        (
            "b.rs",
            "pub struct Widget {\n    pub name: String,\n    pub parts: Vec<u32>,\n}\n\
             pub fn widget() -> Widget {\n    \
             Widget { name: String::new(), parts: Vec::new() }\n\
             }\n",
        ),
    ]);
    let labels = label_set(&ext);
    for builtin in ["Option", "Vec", "String", "Result", "HashMap"] {
        assert!(
            !labels.contains(builtin),
            "prelude type {builtin} must not become a node"
        );
    }
    assert!(
        labels.contains("Widget"),
        "a user-defined type must still become a node"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_rust_locally_defined_prelude_name_is_kept() {
    // A crate may shadow a prelude name (its own `struct Result<T>`). The filter must not erase
    // references to a type the file actually defines.
    let ext = run(&[(
        "shadow.rs",
        "pub struct Result<T>(pub T);\npub struct Holder {\n    pub value: Result<u32>,\n}\n",
    )]);
    assert!(
        label_set(&ext).contains("Result"),
        "a locally defined Result must still become a node"
    );
    assert!(
        has_edge(&ext, "Holder", "references", "Result"),
        "references to the locally defined Result must survive"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_rust_function_call_still_emits_calls() {
    let ext = run(&[(
        "lib.rs",
        "pub fn helper() -> u32 { 1 }\npub fn run() -> u32 { helper() }\n",
    )]);
    assert!(
        has_edge(&ext, "run()", "calls", "helper()"),
        "a real function call must stay `calls`"
    );
}

// test_rust_case_collisions.py: Go keeps one node for names that fold to one id, so only the
// reference test holds.

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_forward_reference_to_the_retained_type_lands_on_it() {
    // A reference above `struct Ignore` in a file that also has `fn ignore` resolves to the struct
    // even when another file declares an `Ignore` too.
    let ext = run(&[
        (
            "dir.rs",
            "fn before() -> Ignore {\n    ignore()\n}\n\n\
             pub struct Ignore {\n    parent: u32,\n}\n\n\
             impl Ignore {\n    pub fn matched(&self) -> bool {\n        true\n    }\n}\n\n\
             pub fn ignore() -> Ignore {\n    build()\n}\n\n\
             fn build() -> Ignore {\n    Ignore { parent: 0 }\n}\n",
        ),
        ("other.rs", "pub struct Ignore;\n"),
    ]);
    let before = ext
        .nodes
        .iter()
        .find(|n| n.label == "before()")
        .map(|n| n.id.as_str())
        .expect("node before()");
    let by_id: BTreeMap<&str, &Node> = ext.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let targets: Vec<(&str, &str)> = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "references" && e.source == before)
        .map(|e| {
            let n = by_id[e.target.as_str()];
            (n.label.as_str(), n.source_file.as_str())
        })
        .collect();
    assert_eq!(targets, [("Ignore", "dir.rs")]);
}

// test_rust_crate_use_scope.py: Go binds a type name by its one definition in the corpus, with no
// crate scope. The Cargo.toml files of the upstream fixtures play no part in `extract` and are not
// written; the `.rs` files sit under the same crate directories.

const STRUCT_VALUE: &str = "pub struct Value;\n";
const USE_VALUE: &str = "pub fn get(v: &Value) -> Option<&Value> {\n    v.get(\"x\")\n}\n";

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_external_crate_use_and_annotation_do_not_reach_a_local_crate() {
    // The `variant` row: Go has no node for an enum variant, so `Value` stays a stub.
    let ext = run(&[
        ("a/src/lib.rs", "pub enum Field {\n    Value(String),\n}\n"),
        (
            "b/src/lib.rs",
            format!("use serde_json::Value;\n\n{USE_VALUE}").as_str(),
        ),
    ]);
    assert!(edges_between(&ext, "b", "a").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_annotation_without_any_use_does_not_reach_a_local_crate() {
    let ext = run(&[
        ("a/src/lib.rs", "pub enum Field {\n    Value(String),\n}\n"),
        ("b/src/lib.rs", USE_VALUE),
    ]);
    assert!(edges_between(&ext, "b", "a").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_std_use_does_not_reach_a_same_named_local_struct() {
    let ext = run(&[
        ("a/src/lib.rs", "pub struct HashMap;\n"),
        (
            "b/src/lib.rs",
            "use std::collections::HashMap;\n\npub fn f(m: &HashMap<u8, u8>) {}\n",
        ),
    ]);
    assert!(edges_between(&ext, "b", "a").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_workspace_crate_use_reaches_that_crate() {
    let ext = run(&[
        ("a/src/lib.rs", STRUCT_VALUE),
        (
            "b/src/lib.rs",
            format!("use a::Value;\n\n{USE_VALUE}").as_str(),
        ),
    ]);
    assert!(!edges_between(&ext, "b", "a").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_crate_relative_use_reaches_its_own_module() {
    let ext = run(&[
        (
            "b/src/lib.rs",
            format!("mod model;\nuse crate::model::Value;\n\n{USE_VALUE}").as_str(),
        ),
        ("b/src/model.rs", STRUCT_VALUE),
    ]);
    assert!(!edges_between(&ext, "b", "b").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_a_use_of_this_crate_keeps_the_binding_extracted() {
    let ext = run(&[
        (
            "b/src/lib.rs",
            format!("mod model;\nuse crate::model::Value;\n\n{USE_VALUE}").as_str(),
        ),
        ("b/src/model.rs", STRUCT_VALUE),
    ]);
    let edges = edges_between(&ext, "b", "b");
    assert!(!edges.is_empty());
    let confidence: BTreeSet<&str> = edges.iter().map(|e| e.confidence.as_str()).collect();
    assert_eq!(confidence, BTreeSet::from(["EXTRACTED"]));
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_a_use_of_a_reexporting_crate_reaches_the_defining_crate() {
    // `api` re-exports `corelib`'s type, as axum re-exports axum-core's.
    let ext = run(&[
        ("corelib/src/lib.rs", "pub struct FromRequestParts;\n"),
        ("api/src/lib.rs", "pub use corelib::FromRequestParts;\n"),
        (
            "plugin/src/lib.rs",
            "use api::FromRequestParts;\n\npub fn f(x: &FromRequestParts) {}\n",
        ),
    ]);
    assert!(!edges_between(&ext, "plugin", "corelib").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_a_super_use_of_a_reexported_type_reaches_the_defining_crate() {
    // axum/src/extract/connect_info.rs: `use super::{.., FromRequestParts}` where the parent
    // module re-exports an axum-core type.
    let ext = run(&[
        ("corelib/src/lib.rs", "pub struct FromRequestParts;\n"),
        (
            "api/src/lib.rs",
            "pub mod sub;\npub use corelib::FromRequestParts;\n",
        ),
        (
            "api/src/sub.rs",
            "use super::FromRequestParts;\n\npub fn f(x: &FromRequestParts) {}\n",
        ),
    ]);
    assert!(!edges_between(&ext, "api", "corelib").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_a_use_of_a_scanned_crate_keeps_the_binding_extracted() {
    let ext = run(&[
        ("corelib/src/lib.rs", "pub struct FromRequestParts;\n"),
        (
            "plugin/src/lib.rs",
            "use corelib::FromRequestParts;\n\npub fn f(x: &FromRequestParts) {}\n",
        ),
    ]);
    let edges = edges_between(&ext, "plugin", "corelib");
    assert!(!edges.is_empty());
    let confidence: BTreeSet<&str> = edges.iter().map(|e| e.confidence.as_str()).collect();
    assert_eq!(confidence, BTreeSet::from(["EXTRACTED"]));
}

// test_rust_in_file_call_binding.py

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_type_qualified_call_binds_to_that_types_method() {
    let ext = run(&[(
        "lib.rs",
        "struct A;\n\
         struct B;\n\
         impl A { fn new() -> A { A } }\n\
         impl B { fn new() -> B { B } }\n\
         fn make_a() -> A { A::new() }\n\
         fn make_b() -> B { crate::B::new() }\n\
         impl A { fn again() -> A { Self::new() } }\n",
    )]);
    let calls = calls_at(&ext);
    assert!(has_call(&calls, "make_a()", "A.new()", "L3"), "{calls:?}");
    assert!(has_call(&calls, "make_b()", "B.new()", "L4"), "{calls:?}");
    assert!(has_call(&calls, "A.again()", "A.new()", "L3"), "{calls:?}");
    assert!(!has_call(&calls, "make_a()", "B.new()", "L4"), "{calls:?}");
    assert!(!has_call(&calls, "make_b()", "A.new()", "L3"), "{calls:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_call_on_a_type_the_file_does_not_implement_gets_no_edge() {
    let ext = run(&[(
        "lib.rs",
        "use std::sync::Arc;\n\
         struct Builder;\n\
         impl Builder { fn new() -> Builder { Builder } fn default() -> Builder { Builder } }\n\
         fn shared() -> Arc<u8> { Arc::new(1) }\n\
         fn options() -> Vec<u8> { Vec::default() }\n",
    )]);
    let calls = calls_at(&ext);
    let bad: Vec<_> = calls
        .iter()
        .filter(|c| c.0 == "shared()" || c.0 == "options()")
        .collect();
    assert!(bad.is_empty(), "{calls:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_self_call_binds_to_the_callers_own_type() {
    let ext = run(&[(
        "lib.rs",
        "struct A;\n\
         struct B;\n\
         impl A {\n\
         \x20   fn run(&self) { self.go(); self.stop(); }\n\
         \x20   fn go(&self) {}\n\
         }\n\
         impl B {\n\
         \x20   fn go(&self) {}\n\
         \x20   fn stop(&self) {}\n\
         }\n",
    )]);
    let calls = calls_at(&ext);
    assert!(has_call(&calls, "A.run()", "A.go()", "L5"), "{calls:?}");
    assert!(!has_call(&calls, "A.run()", "B.go()", "L8"), "{calls:?}");
    // A has no `stop`: B's is never the target.
    assert!(!has_call(&calls, "A.run()", "B.stop()", "L9"), "{calls:?}");
}

// test_rust_scoped_call_resolution.py: Go binds `module::f()` by the receiver's last segment
// against the path tokens of the defining file, and marks the edge INFERRED.

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_bare_super_and_self_are_skipped() {
    // Bare `super::`/`self::` names no module file: the call has no type to start from.
    let ext = run(&[
        (
            "src/a.rs",
            "pub fn run() { super::helper(); self::helper2(); }\n",
        ),
        ("src/b.rs", "pub fn helper() {}\npub fn helper2() {}\n"),
    ]);
    assert!(calls_from(&ext, "src/a.rs", "run()").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_rust_scoped_call_cannot_bind_across_languages() {
    // Rust `b::helper()` with a Python b.py defining helper: no cross-language edge.
    let ext = run(&[
        ("src/a.rs", "pub fn run() { b::helper(); }\n"),
        ("src/b.py", "def helper():\n    pass\n"),
    ]);
    assert!(calls_from(&ext, "src/a.rs", "run()").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_bare_name_calls_keep_their_existing_resolution() {
    let ext = run(&[
        ("src/a.rs", "pub fn run() { helper(); }\n"),
        ("src/b.rs", "pub fn helper() {}\n"),
    ]);
    assert!(has_edge(&ext, "run()", "calls", "helper()"));
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_underscore_module_resolves() {
    let ext = run(&[
        ("src/_internal.rs", "pub fn helper() {}\n"),
        (
            "src/main.rs",
            "mod _internal;\nfn run() { crate::_internal::helper(); }\n",
        ),
    ]);
    let pairs: BTreeSet<(String, String)> = {
        let by_id: BTreeMap<&str, &Node> = ext.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        let where_ = |id: &str| {
            by_id
                .get(id)
                .map(|n| format!("{}:{}", n.source_file, n.label))
                .unwrap_or_default()
        };
        ext.edges
            .iter()
            .filter(|e| e.relation.as_str() == "calls")
            .map(|e| (where_(&e.source), where_(&e.target)))
            .collect()
    };
    assert!(
        pairs.contains(&(
            "src/main.rs:run()".to_string(),
            "src/_internal.rs:helper()".to_string()
        )),
        "{pairs:?}"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_path_call_does_not_bind_to_a_same_named_local_function() {
    let ext = run(&[
        (
            "src/a.rs",
            "fn helper() {}\nfn run() { crate::b::helper(); }\n",
        ),
        ("src/b.rs", "pub fn helper() {}\n"),
    ]);
    let by_id: BTreeMap<&str, &Node> = ext.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let where_ = |id: &str| {
        by_id
            .get(id)
            .map(|n| format!("{}:{}", n.source_file, n.label))
            .unwrap_or_default()
    };
    let pairs: BTreeSet<(String, String)> = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "calls")
        .map(|e| (where_(&e.source), where_(&e.target)))
        .collect();
    let call = |s: &str, t: &str| pairs.contains(&(s.to_string(), t.to_string()));
    assert!(call("src/a.rs:run()", "src/b.rs:helper()"), "{pairs:?}");
    assert!(!call("src/a.rs:run()", "src/a.rs:helper()"), "{pairs:?}");
}

// test_rust_self_member_calls.py: Go binds `self.m()` to a method of the caller's own type node,
// which is per file: it never resolves a self call to an impl block of another file.

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_self_call_same_file_control_is_unaffected() {
    let ext = run(&[(
        "lib.rs",
        "struct Widget { n: u32 }\n\
         impl Widget {\n\
         \x20   fn get(&self) -> u32 { self.n }\n\
         \x20   fn show(&self) -> u32 { self.get() }\n\
         }\n",
    )]);
    assert!(has_edge(&ext, "Widget.show()", "calls", "Widget.get()"));
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_generic_self_call_does_not_cross_unrelated_same_named_families() {
    let ext = run(&[
        ("a/state.rs", "pub struct Bucket<T> { value: T }\n"),
        (
            "a/read.rs",
            "impl<T> Bucket<T> {\n    pub fn fetch_value(&self) {}\n}\n",
        ),
        ("b/state.rs", "pub struct Bucket<T> { value: T }\n"),
        (
            "b/run.rs",
            "impl<U> Bucket<U> {\n    pub fn run(&self) { self.fetch_value(); }\n}\n",
        ),
    ]);
    assert!(calls_from(&ext, "b/run.rs", "Bucket.run()").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_generic_self_call_without_a_declaration_fails_closed() {
    // Impl blocks alone do not prove that same-named owners are one family.
    let ext = run(&[
        (
            "method.rs",
            "impl<T> Bucket<T> { pub fn fetch_value(&self) {} }\n",
        ),
        (
            "caller.rs",
            "impl<U> Bucket<U> {\n    pub fn run(&self) { self.fetch_value(); }\n}\n",
        ),
    ]);
    assert!(calls_from(&ext, "caller.rs", "Bucket.run()").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_scoped_self_call_remains_deferred_across_impl_files() {
    // `Self::method()` is a scoped-call form, outside this resolver slice.
    let ext = run(&[
        ("state.rs", "pub struct Bucket<T> { value: T }\n"),
        (
            "method.rs",
            "impl<T> Bucket<T> {\n    pub fn fetch_value(&self) {}\n}\n",
        ),
        (
            "caller.rs",
            "impl<U> Bucket<U> {\n    pub fn run(&self) { Self::fetch_value(self); }\n}\n",
        ),
    ]);
    assert!(calls_from(&ext, "caller.rs", "Bucket.run()").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_self_call_to_ambiguous_type_name_yields_no_edge() {
    // Two different structs share the bare name `Config`, and both define a same-named method.
    let ext = run(&[
        (
            "a.rs",
            "struct Config { x: i32 }\nimpl Config {\n    fn load(&self) -> i32 { self.x }\n}\n",
        ),
        (
            "b.rs",
            "struct Config { y: i32 }\nimpl Config {\n    fn load(&self) -> i32 { self.y }\n}\n",
        ),
        (
            "c.rs",
            "struct Config { z: i32 }\nimpl Config {\n    fn start(&self) -> i32 { self.load() }\n}\n",
        ),
    ]);
    assert!(calls_from(&ext, "c.rs", "Config.start()").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_self_call_to_undefined_method_yields_no_edge() {
    let ext = run(&[(
        "lib.rs",
        "struct Widget { n: u32 }\nimpl Widget {\n    fn show(&self) -> u32 { self.missing() }\n}\n",
    )]);
    assert!(calls_from(&ext, "lib.rs", "Widget.show()").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_self_call_does_not_link_across_two_unrelated_types_sharing_a_name() {
    // Two unrelated structs share the bare name `Config`; each defines a different method. A
    // caller in one type's impl must not link to a method that only exists on the other.
    let ext = run(&[
        (
            "a.rs",
            "struct Config { x: i32 }\nimpl Config {\n    fn foo(&self) -> i32 { self.x }\n}\n",
        ),
        (
            "b.rs",
            "struct Config { y: i32 }\nimpl Config {\n    fn bar(&self) -> i32 { self.y }\n    fn start(&self) -> i32 { self.foo() }\n}\n",
        ),
    ]);
    assert!(calls_from(&ext, "b.rs", "Config.start()").is_empty());
}

// test_rust_type_alias_union.py

const ALIAS_SRC: &str = "pub type Callback = fn(i32) -> i32;\n\
     pub type Res<T> = Result<T, Error>;\n\
     pub struct Error;\n\
     pub union Bits {\n    i: u32,\n    e: Error,\n}\n\
     pub struct Button {\n    cb: Callback,\n    bits: Bits,\n}\n\
     pub fn run(cb: Callback, b: Bits) -> Res<Callback> {\n    Ok(cb)\n}\n";

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_alias_and_union_are_sourced_nodes_in_their_file() {
    let ext = run(&[("lib.rs", ALIAS_SRC)]);
    for (label, line) in [("Callback", "L1"), ("Res", "L2"), ("Bits", "L4")] {
        let nodes: Vec<&Node> = ext.nodes.iter().filter(|n| n.label == label).collect();
        assert_eq!(nodes.len(), 1, "{label}");
        assert_eq!(nodes[0].source_file, "lib.rs", "{label}");
        assert_eq!(nodes[0].source_location, line, "{label}");
    }
    let sourceless: Vec<&str> = ext
        .nodes
        .iter()
        .filter(|n| n.source_file.is_empty())
        .map(|n| n.label.as_str())
        .collect();
    assert!(sourceless.is_empty(), "{sourceless:?}");
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_alias_and_union_are_contained_by_the_file() {
    let ext = run(&[("lib.rs", ALIAS_SRC)]);
    let contained: BTreeSet<String> = pairs(&ext, "contains").into_iter().map(|p| p.1).collect();
    for want in ["Callback", "Res", "Bits"] {
        assert!(contained.contains(want), "{want}: {contained:?}");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_references_bind_to_the_local_declarations() {
    let ext = run(&[("lib.rs", ALIAS_SRC)]);
    let refs = pairs(&ext, "references");
    for (s, t) in [
        ("Button", "Callback"),
        ("Button", "Bits"),
        ("run()", "Callback"),
        ("run()", "Bits"),
        ("run()", "Res"),
    ] {
        assert!(
            refs.contains(&(s.to_string(), t.to_string())),
            "{s} -> {t}: {refs:?}"
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_an_impl_associated_type_is_not_a_file_level_node() {
    let ext = run(&[(
        "lib.rs",
        "use std::ops::Add;\npub struct Meters(pub f64);\n\
         impl Add for Meters {\n    type Output = Meters;\n    \
         fn add(self, o: Meters) -> Meters { Meters(self.0 + o.0) }\n}\n",
    )]);
    assert!(!label_set(&ext).contains("Output"));
}

#[test]
#[ignore = "graphify-rs-ihc.32"]
fn test_a_cross_file_reference_lands_on_the_alias() {
    // The reference from another file binds to the alias instead of a stub.
    let ext = run(&[
        ("src/lib.rs", "pub mod types;\npub mod user;\n"),
        ("src/types.rs", "pub type Callback = fn(i32) -> i32;\n"),
        (
            "src/user.rs",
            "use crate::types::Callback;\npub struct Button {\n    cb: Callback,\n}\n",
        ),
    ]);
    let aliases: Vec<&Node> = ext.nodes.iter().filter(|n| n.label == "Callback").collect();
    assert_eq!(aliases.len(), 1, "{aliases:?}");
    assert_eq!(aliases[0].source_file, "src/types.rs");
    let by_id: BTreeMap<&str, &Node> = ext.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let targets: BTreeSet<&str> = ext
        .edges
        .iter()
        .filter(|e| {
            e.relation.as_str() == "references"
                && by_id.get(e.source.as_str()).map(|n| n.label.as_str()) == Some("Button")
        })
        .map(|e| e.target.as_str())
        .collect();
    assert_eq!(targets, BTreeSet::from([aliases[0].id.as_str()]));
}
