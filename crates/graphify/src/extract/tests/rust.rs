//! graphify-go `internal/extract/rust_test.go`, and the cases of the upstream graphify v0.9.107
//! `test_rust_*.py` files whose assertion holds for graphify-go v0.12.0 (see the `extract-rust`
//! block of `parity/README.md`). A case that asserts another value for something Go also
//! produces is ported asserting Go's value, under a `// pins Go v0.12.0 (...)` line. The upstream
//! sources are written inline, so no fixture is copied.
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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

/// The confidences of the `calls` edges from `src` to `tgt`, both as `source_file:label`.
fn call_conf(ext: &Extraction, src: &str, tgt: &str) -> Vec<String> {
    let at: BTreeMap<&str, String> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), format!("{}:{}", n.source_file, n.label)))
        .collect();
    ext.edges
        .iter()
        .filter(|e| {
            e.relation.as_str() == "calls"
                && at.get(e.source.as_str()).is_some_and(|s| s == src)
                && at.get(e.target.as_str()).is_some_and(|t| t == tgt)
        })
        .map(|e| e.confidence.as_str().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The distinct confidences of `edges`.
fn confidences<'a>(edges: &[&'a Edge]) -> BTreeSet<&'a str> {
    edges.iter().map(|e| e.confidence.as_str()).collect()
}

// test_rust_anonymous_const.py: Go extracts no `const` or `static` item, so the WIDTH and READY
// labels are not asserted.

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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

// pins Go v0.12.0 (a tuple-struct construction is a `calls` edge, INFERRED, and no `references`);
// upstream 0.9.107 emits `references` with context constructor -> P3
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_rust_newtype_construction_is_reference_not_call() {
    let ext = run(&[
        ("ids.rs", "pub struct ClientId(pub u32);\n"),
        (
            "use_ids.rs",
            "use crate::ids::ClientId;\n\
             pub fn wrap(raw: u32) -> u32 {\n    let _id = ClientId(raw);\n    raw\n}\n",
        ),
    ]);
    assert_eq!(
        call_conf(&ext, "use_ids.rs:wrap()", "ids.rs:ClientId"),
        ["INFERRED"]
    );
    assert!(!has_edge(&ext, "wrap()", "references", "ClientId"));
}

// pins Go v0.12.0 (a tuple-struct construction is a `calls` edge, INFERRED, and no `references`);
// upstream 0.9.107 emits `references` with context constructor -> P3
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_rust_same_file_construction_is_reference_not_call() {
    let ext = run(&[(
        "ids.rs",
        "pub struct ClientId(pub u32);\n\
         pub fn wrap(raw: u32) -> u32 {\n    let _id = ClientId(raw);\n    raw\n}\n",
    )]);
    assert_eq!(
        call_conf(&ext, "ids.rs:wrap()", "ids.rs:ClientId"),
        ["INFERRED"]
    );
    assert!(!has_edge(&ext, "wrap()", "references", "ClientId"));
}

// test_rust_case_collisions.py: Go keeps one node for names that fold to one id, the first
// declared, so each case that asserts two nodes is ported asserting the one.

const IGNORE_RS: &str = "pub struct Ignore {\n    parent: u32,\n}\n\n\
     impl Ignore {\n    pub fn matched(&self) -> bool {\n        true\n    }\n}\n\n\
     pub fn ignore() -> Ignore {\n    build()\n}\n\n\
     fn build() -> Ignore {\n    Ignore { parent: 0 }\n}\n";

// pins Go v0.12.0 (one node `Ignore` at L1, no `ignore()`); upstream 0.9.107 emits both -> P3 (B01)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_function_and_struct_differing_by_case_are_both_extracted() {
    let ext = run(&[("dir.rs", IGNORE_RS)]);
    let at = |label: &str| -> Vec<&str> {
        ext.nodes
            .iter()
            .filter(|n| n.label == label && !n.source_file.is_empty())
            .map(|n| n.source_location.as_str())
            .collect()
    };
    assert_eq!(at("Ignore"), ["L1"]);
    assert!(at("ignore()").is_empty());
}

// pins Go v0.12.0 (the type keeps its id and Go labels the method `Ignore.matched()`, with a
// `contains` edge); upstream 0.9.107 emits `.matched()` with a `method` edge -> P3 (B28)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_struct_keeps_its_id_and_methods_when_a_function_shares_it() {
    let ids = |ext: &Extraction| -> BTreeSet<String> {
        ext.nodes
            .iter()
            .filter(|n| n.label == "Ignore" || n.label == "Ignore.matched()")
            .map(|n| n.id.clone())
            .collect()
    };
    let solo = run(&[(
        "dir.rs",
        IGNORE_RS
            .replace("pub fn ignore() -> Ignore {\n    build()\n}\n\n", "")
            .as_str(),
    )]);
    let solo_ids = ids(&solo);
    assert_eq!(solo_ids.len(), 2);
    let ext = run(&[("dir.rs", IGNORE_RS)]);
    assert_eq!(ids(&ext), solo_ids);
    assert!(has_edge(&ext, "Ignore", "contains", "Ignore.matched()"));
}

// pins Go v0.12.0 (the call inside `ignore()` is credited to the node `Ignore`, and `ignore()`
// has no references edge); upstream 0.9.107 emits `ignore()` -> `build()` -> P3 (B01)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_salted_function_owns_its_own_body() {
    let ext = run(&[("dir.rs", IGNORE_RS)]);
    assert!(has_edge(&ext, "Ignore", "calls", "build()"));
    assert!(!label_set(&ext).contains("ignore()"));
}

// pins Go v0.12.0 (the first declared keeps the node: `ignore()` when the function comes first,
// `Ignore` otherwise); upstream 0.9.107 emits the same two nodes in both orders -> P3 (B01)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_type_keeps_the_plain_id_whatever_the_declaration_order() {
    let forward = run(&[("dir.rs", IGNORE_RS)]);
    let reordered = run(&[(
        "dir.rs",
        "pub fn ignore() -> Ignore {\n    build()\n}\n\n\
         fn build() -> Ignore {\n    Ignore { parent: 0 }\n}\n\n\
         impl Ignore {\n    pub fn matched(&self) -> bool {\n        true\n    }\n}\n\n\
         pub struct Ignore {\n    parent: u32,\n}\n",
    )]);
    assert!(label_set(&forward).contains("Ignore"));
    assert!(!label_set(&forward).contains("ignore()"));
    assert!(label_set(&reordered).contains("ignore()"));
    assert!(!label_set(&reordered).contains("Ignore"));
    assert!(has_edge(
        &reordered,
        "ignore()",
        "contains",
        "Ignore.matched()"
    ));
}

// pins Go v0.12.0 (one node `e()`, no `E()`); upstream 0.9.107 emits both under salted ids
// -> P3 (B01)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_case_only_functions_are_both_salted() {
    let ext = run(&[(
        "literal.rs",
        "fn e(s: &str) -> u32 {\n    1\n}\n\nfn E(s: &str) -> u32 {\n    2\n}\n",
    )]);
    let labels = label_set(&ext);
    assert!(labels.contains("e()"));
    assert!(!labels.contains("E()"));
}

// pins Go v0.12.0 (the twins are one node at L2 and the case-only `Device_Num` struct folds into
// it: no `Device_Num` node); upstream 0.9.107 emits `Device_Num` as a node of its own -> P3 (B01)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_cfg_twins_still_share_one_node() {
    let twins = "#[cfg(unix)]\nfn device_num(p: &str) -> u64 {\n    1\n}\n\n\
                 #[cfg(windows)]\nfn device_num(p: &str) -> u64 {\n    2\n}\n";
    let at = |ext: &Extraction, label: &str| -> Vec<String> {
        ext.nodes
            .iter()
            .filter(|n| n.label == label && !n.source_file.is_empty())
            .map(|n| n.source_location.clone())
            .collect()
    };
    let solo = run(&[("walk.rs", twins)]);
    assert_eq!(at(&solo, "device_num()"), ["L2"]);

    let ext = run(&[("walk.rs", format!("{twins}\nstruct Device_Num;\n").as_str())]);
    assert_eq!(at(&ext, "device_num()"), ["L2"]);
    assert!(at(&ext, "Device_Num").is_empty());
}

// pins Go v0.12.0 (`caller()` -> `Ignore` and `caller()` -> `e()`: `ignore()` and `E()` are the
// nodes `Ignore` and `e()`); upstream 0.9.107 emits `ignore()`, `e()` and `E()` -> P3 (B01)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_calls_to_salted_functions_keep_their_in_file_edges() {
    let ext = run(&[(
        "dir.rs",
        format!(
            "{IGNORE_RS}\nfn e(s: &str) -> u32 {{\n    1\n}}\n\
             \nfn E(s: &str) -> u32 {{\n    2\n}}\n\
             \nfn caller() -> u32 {{\n    let _ = ignore();\n    e(\"a\") + E(\"b\")\n}}\n"
        )
        .as_str(),
    )]);
    let targets: BTreeSet<String> = calls_from(&ext, "dir.rs", "caller()").into_iter().collect();
    assert_eq!(
        targets,
        BTreeSet::from(["Ignore".to_string(), "e()".to_string()])
    );
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B05
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_external_crate_use_alone_does_not_reach_a_local_crate() {
    // Holds because nothing is referenced: the `use` is an import of an external node.
    let ext = run(&[
        ("a/src/lib.rs", "pub enum Field {\n    Value(String),\n}\n"),
        ("b/src/lib.rs", "use serde_json::Value;\n\npub fn f() {}\n"),
    ]);
    assert!(edges_between(&ext, "b", "a").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_annotation_without_any_use_does_not_reach_a_local_crate() {
    let ext = run(&[
        ("a/src/lib.rs", "pub enum Field {\n    Value(String),\n}\n"),
        ("b/src/lib.rs", USE_VALUE),
    ]);
    assert!(edges_between(&ext, "b", "a").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B05
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_same_named_types_in_two_crates_have_no_edge_without_a_use() {
    // Holds because nothing references either type.
    let ext = run(&[
        ("a/src/lib.rs", STRUCT_VALUE),
        ("b/src/lib.rs", STRUCT_VALUE),
    ]);
    assert!(edges_between(&ext, "a", "b").is_empty());
    assert!(edges_between(&ext, "b", "a").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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

// pins Go v0.12.0 (b's get() references a's Value); upstream 0.9.107 emits no edge -> P3 (B05)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_external_crate_use_and_annotation_do_not_reach_a_local_crate_struct() {
    // The `struct` row of the parametrized upstream test.
    let ext = run(&[
        ("a/src/lib.rs", STRUCT_VALUE),
        (
            "b/src/lib.rs",
            format!("use serde_json::Value;\n\n{USE_VALUE}").as_str(),
        ),
    ]);
    assert!(!edges_between(&ext, "b", "a").is_empty());
    assert!(has_edge(&ext, "get()", "references", "Value"));
}

// pins Go v0.12.0 (b's get() references a's Value, EXTRACTED; Go binds `Map` the same way); upstream
// 0.9.107 emits no edge -> P3 (B05)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_grouped_external_use_does_not_reach_a_local_crate() {
    let ext = run(&[
        (
            "a/src/lib.rs",
            "pub enum Field {\n    Value(String),\n}\npub struct Map;\n",
        ),
        (
            "b/src/lib.rs",
            "use serde_json::{Map, Value};\n\npub fn f(m: &Map<String, Value>) {}\n",
        ),
    ]);
    assert_eq!(
        confidences(&edges_between(&ext, "b", "a")),
        BTreeSet::from(["EXTRACTED"])
    );
}

// pins Go v0.12.0 (no edge to a or c: two `Value` and no scope, so b's get() keeps a stub);
// upstream 0.9.107 emits an edge to a -> P3 (B05)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_workspace_crate_use_reaches_the_named_crate_not_a_namesake() {
    let ext = run(&[
        ("a/src/lib.rs", STRUCT_VALUE),
        ("c/src/lib.rs", STRUCT_VALUE),
        (
            "b/src/lib.rs",
            format!("use a::Value;\n\n{USE_VALUE}").as_str(),
        ),
    ]);
    assert!(edges_between(&ext, "b", "a").is_empty());
    assert!(edges_between(&ext, "b", "c").is_empty());
    assert!(has_edge(&ext, "get()", "references", "Value"));
}

// pins Go v0.12.0 (plugin's f() references server's Config, EXTRACTED); upstream 0.9.107 emits no
// edge -> P3 (B05)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_workspace_crate_use_does_not_reach_another_crates_namesake() {
    // `osiris_types` (package `osiris-types`) has no `Config`; `server` has one.
    let ext = run(&[
        ("osiris-types/src/lib.rs", "pub struct Other;\n"),
        ("server/src/lib.rs", "pub struct Config;\n"),
        (
            "plugin/src/lib.rs",
            "use osiris_types::Config;\n\npub fn f(c: &Config) {}\n",
        ),
    ]);
    assert_eq!(
        confidences(&edges_between(&ext, "plugin", "server")),
        BTreeSet::from(["EXTRACTED"])
    );
}

// pins Go v0.12.0 (no edge to either crate: two `Config` and no scope, so f() keeps a stub);
// upstream 0.9.107 emits an edge to osiris-types -> P3 (B05)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_workspace_crate_use_reaches_its_own_crate_when_two_crates_share_a_name() {
    let ext = run(&[
        ("osiris-types/src/lib.rs", "pub struct Config;\n"),
        ("server/src/lib.rs", "pub struct Config;\n"),
        (
            "plugin/src/lib.rs",
            "use osiris_types::Config;\n\npub fn f(c: &Config) {}\n",
        ),
    ]);
    assert!(edges_between(&ext, "plugin", "osiris-types").is_empty());
    assert!(edges_between(&ext, "plugin", "server").is_empty());
    assert!(has_edge(&ext, "f()", "references", "Config"));
}

// pins Go v0.12.0 (EXTRACTED); upstream 0.9.107 emits INFERRED -> P3 (B05)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_a_crate_less_binding_is_flagged_inferred_not_extracted() {
    // No Cargo.toml is read: Go binds by name, and a `references` edge is EXTRACTED.
    let ext = run(&[
        ("a/src/lib.rs", STRUCT_VALUE),
        (
            "b/src/lib.rs",
            format!("use serde_json::Value;\n\n{USE_VALUE}").as_str(),
        ),
    ]);
    assert_eq!(
        confidences(&edges_between(&ext, "b", "a")),
        BTreeSet::from(["EXTRACTED"])
    );
}

// pins Go v0.12.0 (no edge to corelib or other: two `FromRequestParts`, so f() keeps a stub);
// upstream 0.9.107 emits an edge to corelib -> P3 (B05)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_a_reexport_does_not_reach_an_unrelated_namesake() {
    let ext = run(&[
        ("corelib/src/lib.rs", "pub struct FromRequestParts;\n"),
        ("other/src/lib.rs", "pub struct FromRequestParts;\n"),
        ("api/src/lib.rs", "pub use corelib::FromRequestParts;\n"),
        (
            "plugin/src/lib.rs",
            "use api::FromRequestParts;\n\npub fn f(x: &FromRequestParts) {}\n",
        ),
    ]);
    assert!(edges_between(&ext, "plugin", "corelib").is_empty());
    assert!(edges_between(&ext, "plugin", "other").is_empty());
}

// pins Go v0.12.0 (plugin's f() references corelib's type, EXTRACTED); upstream 0.9.107 emits no
// edge -> P3 (B05)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_a_crate_restricted_reexport_does_not_leak_out_of_its_crate() {
    let ext = run(&[
        ("corelib/src/lib.rs", "pub struct FromRequestParts;\n"),
        (
            "api/src/lib.rs",
            "pub(crate) use corelib::FromRequestParts;\n",
        ),
        (
            "plugin/src/lib.rs",
            "use api::FromRequestParts;\n\npub fn f(x: &FromRequestParts) {}\n",
        ),
    ]);
    assert_eq!(
        confidences(&edges_between(&ext, "plugin", "corelib")),
        BTreeSet::from(["EXTRACTED"])
    );
}

// test_rust_in_file_call_binding.py

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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

// pins Go v0.12.0 (INFERRED edge close() -> Child.drop(), no edge from count()); upstream 0.9.107
// emits no edge from either -> P3
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_bare_call_never_binds_to_a_method_and_member_call_never_to_a_free_fn() {
    // Go binds a bare `drop(c)` to the one same-file method named `drop`; `v.total()` has no
    // receiver qualifier that matches the free `total()`.
    let ext = run(&[(
        "lib.rs",
        "struct Child;\n\
         impl Drop for Child { fn drop(&mut self) {} }\n\
         fn total() -> usize { 0 }\n\
         fn close(c: Child) { drop(c); }\n\
         fn count(v: &Stats) -> usize { v.total() }\n",
    )]);
    assert_eq!(
        call_conf(&ext, "lib.rs:close()", "lib.rs:Child.drop()"),
        ["INFERRED"]
    );
    assert!(calls_from(&ext, "lib.rs", "count()").is_empty());
}

// test_rust_macro_argument_calls.py: Go scans no macro argument (tokens only) and records the
// macro name as the call. Only the negatives are ported: each holds because no argument is read.

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B44
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_method_path_and_constructor_tokens_are_not_bare_calls() {
    let ext = run(&[(
        "lib.rs",
        "struct Wrap(u8);\n\
         fn len() -> usize { 0 }\n\
         fn new() -> u8 { 0 }\n\
         fn show(v: Vec<u8>) { println!(\"{} {} {:?}\", v.len(), Vec::<u8>::new().len(), Wrap(1).0); }\n",
    )]);
    assert!(calls_from(&ext, "lib.rs", "show()").is_empty());
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B44
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_nested_macro_name_is_not_a_call() {
    let ext = run(&[(
        "lib.rs",
        "fn format() -> String { String::new() }\n\
         fn show() { println!(\"{}\", format!(\"x\")); }\n",
    )]);
    assert!(!has_edge(&ext, "show()", "calls", "format()"));
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B44
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_macro_rules_body_is_not_scanned() {
    let ext = run(&[(
        "lib.rs",
        "fn helper() {}\n\
         macro_rules! twice { ($e:expr) => { helper(); helper(); }; }\n\
         fn idle() {}\n",
    )]);
    assert!(pairs(&ext, "calls").iter().all(|p| p.1 != "helper()"));
}

// pins Go v0.12.0; upstream 0.9.107 differs -> P3 B44
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_dsl_macros_are_not_scanned() {
    let ext = run(&[(
        "lib.rs",
        "fn users() {}\n\
         fn name() {}\n\
         table! { users (id) { id -> Int4, } }\n\
         fn gen() { quote! { #name(1) }; html! { <div>{ users() }</div> }; }\n",
    )]);
    assert!(
        pairs(&ext, "calls")
            .iter()
            .all(|p| p.1 != "users()" && p.1 != "name()")
    );
}

// pins Go v0.12.0 (INFERRED); upstream 0.9.107 emits EXTRACTED -> P3 (0td.32)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_a_parsed_call_keeps_extracted_when_the_macro_comes_first() {
    // The call inside `println!` is not read; `compute(2)` is the one call edge.
    let ext = run(&[(
        "lib.rs",
        "fn compute(x: u8) -> u8 { x }\n\
         fn run() { println!(\"{}\", compute(1)); let _y = compute(2); }\n",
    )]);
    assert_eq!(
        call_conf(&ext, "lib.rs:run()", "lib.rs:compute()"),
        ["INFERRED"]
    );
}

// test_rust_macro_rules.py: Go has no `macro_definition` node and no macro-name binding. Only the
// negative is ported.

// pins Go v0.12.0; upstream 0.9.107 differs -> P3
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_scoped_macro_invocation_stays_unresolved() {
    // Holds because no `info` is defined: the macro name is a call to `info`, with nothing to bind.
    let ext = run(&[("lib.rs", "fn run() {\n    log::info!(\"hi\");\n}\n")]);
    assert!(pairs(&ext, "calls").is_empty());
}

// test_rust_scoped_call_resolution.py: Go binds `module::f()` by the receiver's last segment
// against the path tokens of the defining file, and marks the edge INFERRED.

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_rust_scoped_call_cannot_bind_across_languages() {
    // Rust `b::helper()` with a Python b.py defining helper: no cross-language edge.
    let ext = run(&[
        ("src/a.rs", "pub fn run() { b::helper(); }\n"),
        ("src/b.py", "def helper():\n    pass\n"),
    ]);
    assert!(calls_from(&ext, "src/a.rs", "run()").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_bare_name_calls_keep_their_existing_resolution() {
    let ext = run(&[
        ("src/a.rs", "pub fn run() { helper(); }\n"),
        ("src/b.rs", "pub fn helper() {}\n"),
    ]);
    assert!(has_edge(&ext, "run()", "calls", "helper()"));
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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

// pins Go v0.12.0 (INFERRED, to b.rs only); upstream 0.9.107 emits EXTRACTED -> P3 (B26)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_module_qualified_call_resolves_to_the_named_module() {
    // c.rs defines a same-named helper: the qualifier, not the name, decides.
    let ext = run(&[
        ("src/a.rs", "pub fn run() { b::helper(); }\n"),
        ("src/b.rs", "pub fn helper() {}\n"),
        ("src/c.rs", "pub fn helper() {}\n"),
    ]);
    assert_eq!(
        call_conf(&ext, "src/a.rs:run()", "src/b.rs:helper()"),
        ["INFERRED"]
    );
    assert!(call_conf(&ext, "src/a.rs:run()", "src/c.rs:helper()").is_empty());
}

// pins Go v0.12.0 (INFERRED); upstream 0.9.107 emits EXTRACTED -> P3 (B26)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_mod_rs_form_resolves() {
    let ext = run(&[
        ("src/a.rs", "pub fn run() { util::helper(); }\n"),
        ("src/util/mod.rs", "pub fn helper() {}\n"),
    ]);
    assert_eq!(
        call_conf(&ext, "src/a.rs:run()", "src/util/mod.rs:helper()"),
        ["INFERRED"]
    );
}

// pins Go v0.12.0 (INFERRED edge to src/b.rs, the file in the caller's directory); upstream
// 0.9.107 emits no edge (two survivors) -> P3 (B26)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_two_matching_module_files_emit_nothing() {
    let ext = run(&[
        ("src/a.rs", "pub fn run() { b::helper(); }\n"),
        ("src/b.rs", "pub fn helper() {}\n"),
        ("src/b/mod.rs", "pub fn helper() {}\n"),
    ]);
    assert_eq!(
        call_conf(&ext, "src/a.rs:run()", "src/b.rs:helper()"),
        ["INFERRED"]
    );
    assert!(call_conf(&ext, "src/a.rs:run()", "src/b/mod.rs:helper()").is_empty());
}

// pins Go v0.12.0 (INFERRED edge to the impl method Helper.helper()); upstream 0.9.107 emits no
// edge -> P3 (B26)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_uppercase_qualifier_is_a_type_not_a_module() {
    let ext = run(&[
        ("src/a.rs", "pub fn run() { Helper::helper(); }\n"),
        (
            "src/helper.rs",
            "pub struct Helper;\nimpl Helper { pub fn helper() {} }\npub fn helper() {}\n",
        ),
    ]);
    assert_eq!(
        call_conf(&ext, "src/a.rs:run()", "src/helper.rs:Helper.helper()"),
        ["INFERRED"]
    );
}

// pins Go v0.12.0 (INFERRED); upstream 0.9.107 emits EXTRACTED -> P3 (B26)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_crate_qualified_path_verifies_against_directories() {
    let ext = run(&[
        ("src/a.rs", "pub fn run() { crate::util::helper(); }\n"),
        ("src/util.rs", "pub fn helper() {}\n"),
    ]);
    assert_eq!(
        call_conf(&ext, "src/a.rs:run()", "src/util.rs:helper()"),
        ["INFERRED"]
    );
}

// pins Go v0.12.0 (INFERRED edge to the local fs.rs read()); upstream 0.9.107 emits no edge
// -> P3 (B26)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_std_qualified_call_does_not_bind_to_local_shadow() {
    let ext = run(&[
        (
            "src/a.rs",
            "pub fn run() { let _ = std::fs::read(\"x\"); }\n",
        ),
        (
            "src/fs.rs",
            "pub fn read(p: &str) -> Vec<u8> { Vec::new() }\n",
        ),
    ]);
    assert_eq!(
        call_conf(&ext, "src/a.rs:run()", "src/fs.rs:read()"),
        ["INFERRED"]
    );
}

// pins Go v0.12.0 (INFERRED edge to the local fs.rs read()); upstream 0.9.107 emits no edge
// -> P3 (B26)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_use_bound_qualifier_from_std_is_skipped() {
    let ext = run(&[
        (
            "src/a.rs",
            "use std::fs;\npub fn run() { let _ = fs::read(\"x\"); }\n",
        ),
        (
            "src/fs.rs",
            "pub fn read(p: &str) -> Vec<u8> { Vec::new() }\n",
        ),
    ]);
    assert_eq!(
        call_conf(&ext, "src/a.rs:run()", "src/fs.rs:read()"),
        ["INFERRED"]
    );
}

// pins Go v0.12.0 (INFERRED); upstream 0.9.107 emits EXTRACTED -> P3 (B26)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_use_bound_qualifier_from_crate_still_resolves() {
    let ext = run(&[
        (
            "src/a.rs",
            "use crate::util;\npub fn run() { util::helper(); }\n",
        ),
        ("src/util.rs", "pub fn helper() {}\n"),
    ]);
    assert_eq!(
        call_conf(&ext, "src/a.rs:run()", "src/util.rs:helper()"),
        ["INFERRED"]
    );
}

// pins Go v0.12.0 (INFERRED edge into crate_b/src/util.rs f()); upstream 0.9.107 emits no edge
// -> P3 (B26)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_crate_path_does_not_cross_into_another_workspace_crate() {
    let ext = run(&[
        (
            "crate_a/src/main.rs",
            "mod util;\nfn run() { crate::util::f(); }\n",
        ),
        ("crate_a/src/util.rs", "pub fn g() {}\n"),
        ("crate_b/src/util.rs", "pub fn f() {}\n"),
    ]);
    assert_eq!(
        call_conf(&ext, "crate_a/src/main.rs:run()", "crate_b/src/util.rs:f()"),
        ["INFERRED"]
    );
}

// pins Go v0.12.0 (INFERRED edge to the impl method Db.open()); upstream 0.9.107 emits no edge
// -> P3 (B26)
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_module_path_never_names_an_impl_method() {
    let ext = run(&[
        (
            "src/store.rs",
            "pub struct Db;\nimpl Db { pub fn open(&self) {} }\n",
        ),
        ("src/main.rs", "mod store;\nfn run() { store::open(); }\n"),
    ]);
    assert_eq!(
        call_conf(&ext, "src/main.rs:run()", "src/store.rs:Db.open()"),
        ["INFERRED"]
    );
}

// test_rust_self_member_calls.py: Go binds `self.m()` to a method of the caller's own type node,
// which is per file: it never resolves a self call to an impl block of another file.

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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

// pins Go v0.12.0; upstream 0.9.107 differs -> P3
#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_generic_self_call_counts_collapsed_same_file_declarations() {
    // Holds because Go never resolves a self call across files.
    let ext = run(&[
        (
            "state.rs",
            "pub mod a { pub struct Bucket<T>(pub T); }\npub mod b { pub struct Bucket<T>(pub T); }\n",
        ),
        (
            "a_impl.rs",
            "impl<T> Bucket<T> {\n    pub fn fetch_value(&self) {}\n}\n",
        ),
        (
            "fallback.rs",
            "pub trait Fallback { fn fetch_value(&self) {} }\n",
        ),
        (
            "b_impl.rs",
            "impl<T> Fallback for Bucket<T> {}\nimpl<U> Bucket<U> {\n    pub fn run(&self) { self.fetch_value(); }\n}\n",
        ),
    ]);
    assert!(calls_from(&ext, "b_impl.rs", "Bucket.run()").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_self_call_to_undefined_method_yields_no_edge() {
    let ext = run(&[(
        "lib.rs",
        "struct Widget { n: u32 }\nimpl Widget {\n    fn show(&self) -> u32 { self.missing() }\n}\n",
    )]);
    assert!(calls_from(&ext, "lib.rs", "Widget.show()").is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
fn test_alias_and_union_are_contained_by_the_file() {
    let ext = run(&[("lib.rs", ALIAS_SRC)]);
    let contained: BTreeSet<String> = pairs(&ext, "contains").into_iter().map(|p| p.1).collect();
    for want in ["Callback", "Res", "Bits"] {
        assert!(contained.contains(want), "{want}: {contained:?}");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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
#[ignore = "graphify-rs-ihc.18"] // rust test (ihc.32): needs resolve
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

// ---- smoke ----

/// Every distinct `(rel, src)` of the inline `run(&[...])` sources above, copied verbatim (the
/// `STRUCT_VALUE`, `USE_VALUE` and `ALIAS_SRC` consts expanded), identical pairs once.
const SMOKE_SRCS: &[(&str, &str)] = &[
    (
        "codec.rs",
        "pub const WIDTH: usize = 8;\n\
         const _: () = assert!(WIDTH == 8);\n\
         const _: () = ();\n\
         static READY: bool = true;\n\
         pub fn encode() {}\n",
    ),
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
    (
        "shadow.rs",
        "pub struct Result<T>(pub T);\npub struct Holder {\n    pub value: Result<u32>,\n}\n",
    ),
    (
        "lib.rs",
        "pub fn helper() -> u32 { 1 }\npub fn run() -> u32 { helper() }\n",
    ),
    (
        "dir.rs",
        "fn before() -> Ignore {\n    ignore()\n}\n\n\
         pub struct Ignore {\n    parent: u32,\n}\n\n\
         impl Ignore {\n    pub fn matched(&self) -> bool {\n        true\n    }\n}\n\n\
         pub fn ignore() -> Ignore {\n    build()\n}\n\n\
         fn build() -> Ignore {\n    Ignore { parent: 0 }\n}\n",
    ),
    ("other.rs", "pub struct Ignore;\n"),
    ("a/src/lib.rs", "pub enum Field {\n    Value(String),\n}\n"),
    (
        "b/src/lib.rs",
        "use serde_json::Value;\n\npub fn get(v: &Value) -> Option<&Value> {\n    v.get(\"x\")\n}\n",
    ),
    ("b/src/lib.rs", "use serde_json::Value;\n\npub fn f() {}\n"),
    (
        "b/src/lib.rs",
        "pub fn get(v: &Value) -> Option<&Value> {\n    v.get(\"x\")\n}\n",
    ),
    ("a/src/lib.rs", "pub struct HashMap;\n"),
    (
        "b/src/lib.rs",
        "use std::collections::HashMap;\n\npub fn f(m: &HashMap<u8, u8>) {}\n",
    ),
    ("a/src/lib.rs", "pub struct Value;\n"),
    (
        "b/src/lib.rs",
        "use a::Value;\n\npub fn get(v: &Value) -> Option<&Value> {\n    v.get(\"x\")\n}\n",
    ),
    (
        "b/src/lib.rs",
        "mod model;\nuse crate::model::Value;\n\npub fn get(v: &Value) -> Option<&Value> {\n    v.get(\"x\")\n}\n",
    ),
    ("b/src/model.rs", "pub struct Value;\n"),
    ("b/src/lib.rs", "pub struct Value;\n"),
    ("corelib/src/lib.rs", "pub struct FromRequestParts;\n"),
    ("api/src/lib.rs", "pub use corelib::FromRequestParts;\n"),
    (
        "plugin/src/lib.rs",
        "use api::FromRequestParts;\n\npub fn f(x: &FromRequestParts) {}\n",
    ),
    (
        "api/src/lib.rs",
        "pub mod sub;\npub use corelib::FromRequestParts;\n",
    ),
    (
        "api/src/sub.rs",
        "use super::FromRequestParts;\n\npub fn f(x: &FromRequestParts) {}\n",
    ),
    (
        "plugin/src/lib.rs",
        "use corelib::FromRequestParts;\n\npub fn f(x: &FromRequestParts) {}\n",
    ),
    (
        "lib.rs",
        "struct A;\n\
         struct B;\n\
         impl A { fn new() -> A { A } }\n\
         impl B { fn new() -> B { B } }\n\
         fn make_a() -> A { A::new() }\n\
         fn make_b() -> B { crate::B::new() }\n\
         impl A { fn again() -> A { Self::new() } }\n",
    ),
    (
        "lib.rs",
        "use std::sync::Arc;\n\
         struct Builder;\n\
         impl Builder { fn new() -> Builder { Builder } fn default() -> Builder { Builder } }\n\
         fn shared() -> Arc<u8> { Arc::new(1) }\n\
         fn options() -> Vec<u8> { Vec::default() }\n",
    ),
    (
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
    ),
    (
        "lib.rs",
        "struct Wrap(u8);\n\
         fn len() -> usize { 0 }\n\
         fn new() -> u8 { 0 }\n\
         fn show(v: Vec<u8>) { println!(\"{} {} {:?}\", v.len(), Vec::<u8>::new().len(), Wrap(1).0); }\n",
    ),
    (
        "lib.rs",
        "fn format() -> String { String::new() }\n\
         fn show() { println!(\"{}\", format!(\"x\")); }\n",
    ),
    (
        "lib.rs",
        "fn helper() {}\n\
         macro_rules! twice { ($e:expr) => { helper(); helper(); }; }\n\
         fn idle() {}\n",
    ),
    (
        "lib.rs",
        "fn users() {}\n\
         fn name() {}\n\
         table! { users (id) { id -> Int4, } }\n\
         fn gen() { quote! { #name(1) }; html! { <div>{ users() }</div> }; }\n",
    ),
    ("lib.rs", "fn run() {\n    log::info!(\"hi\");\n}\n"),
    (
        "src/a.rs",
        "pub fn run() { super::helper(); self::helper2(); }\n",
    ),
    ("src/b.rs", "pub fn helper() {}\npub fn helper2() {}\n"),
    ("src/a.rs", "pub fn run() { b::helper(); }\n"),
    ("src/a.rs", "pub fn run() { helper(); }\n"),
    ("src/b.rs", "pub fn helper() {}\n"),
    ("src/_internal.rs", "pub fn helper() {}\n"),
    (
        "src/main.rs",
        "mod _internal;\nfn run() { crate::_internal::helper(); }\n",
    ),
    (
        "src/a.rs",
        "fn helper() {}\nfn run() { crate::b::helper(); }\n",
    ),
    (
        "lib.rs",
        "struct Widget { n: u32 }\n\
         impl Widget {\n\
         \x20   fn get(&self) -> u32 { self.n }\n\
         \x20   fn show(&self) -> u32 { self.get() }\n\
         }\n",
    ),
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
    (
        "method.rs",
        "impl<T> Bucket<T> { pub fn fetch_value(&self) {} }\n",
    ),
    (
        "caller.rs",
        "impl<U> Bucket<U> {\n    pub fn run(&self) { self.fetch_value(); }\n}\n",
    ),
    (
        "state.rs",
        "pub mod a { pub struct Bucket<T>(pub T); }\npub mod b { pub struct Bucket<T>(pub T); }\n",
    ),
    (
        "a_impl.rs",
        "impl<T> Bucket<T> {\n    pub fn fetch_value(&self) {}\n}\n",
    ),
    (
        "fallback.rs",
        "pub trait Fallback { fn fetch_value(&self) {} }\n",
    ),
    (
        "b_impl.rs",
        "impl<T> Fallback for Bucket<T> {}\nimpl<U> Bucket<U> {\n    pub fn run(&self) { self.fetch_value(); }\n}\n",
    ),
    ("state.rs", "pub struct Bucket<T> { value: T }\n"),
    (
        "method.rs",
        "impl<T> Bucket<T> {\n    pub fn fetch_value(&self) {}\n}\n",
    ),
    (
        "caller.rs",
        "impl<U> Bucket<U> {\n    pub fn run(&self) { Self::fetch_value(self); }\n}\n",
    ),
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
    (
        "lib.rs",
        "struct Widget { n: u32 }\nimpl Widget {\n    fn show(&self) -> u32 { self.missing() }\n}\n",
    ),
    (
        "a.rs",
        "struct Config { x: i32 }\nimpl Config {\n    fn foo(&self) -> i32 { self.x }\n}\n",
    ),
    (
        "b.rs",
        "struct Config { y: i32 }\nimpl Config {\n    fn bar(&self) -> i32 { self.y }\n    fn start(&self) -> i32 { self.foo() }\n}\n",
    ),
    (
        "lib.rs",
        "pub type Callback = fn(i32) -> i32;\n\
         pub type Res<T> = Result<T, Error>;\n\
         pub struct Error;\n\
         pub union Bits {\n    i: u32,\n    e: Error,\n}\n\
         pub struct Button {\n    cb: Callback,\n    bits: Bits,\n}\n\
         pub fn run(cb: Callback, b: Bits) -> Res<Callback> {\n    Ok(cb)\n}\n",
    ),
    (
        "lib.rs",
        "use std::ops::Add;\npub struct Meters(pub f64);\n\
         impl Add for Meters {\n    type Output = Meters;\n    \
         fn add(self, o: Meters) -> Meters { Meters(self.0 + o.0) }\n}\n",
    ),
    ("src/lib.rs", "pub mod types;\npub mod user;\n"),
    ("src/types.rs", "pub type Callback = fn(i32) -> i32;\n"),
    (
        "src/user.rs",
        "use crate::types::Callback;\npub struct Button {\n    cb: Callback,\n}\n",
    ),
    ("ids.rs", "pub struct ClientId(pub u32);\n"),
    (
        "use_ids.rs",
        "use crate::ids::ClientId;\n\
         pub fn wrap(raw: u32) -> u32 {\n    let _id = ClientId(raw);\n    raw\n}\n",
    ),
    (
        "ids.rs",
        "pub struct ClientId(pub u32);\n\
         pub fn wrap(raw: u32) -> u32 {\n    let _id = ClientId(raw);\n    raw\n}\n",
    ),
    (
        "dir.rs",
        "pub struct Ignore {\n    parent: u32,\n}\n\n\
         impl Ignore {\n    pub fn matched(&self) -> bool {\n        true\n    }\n}\n\n\
         pub fn ignore() -> Ignore {\n    build()\n}\n\n\
         fn build() -> Ignore {\n    Ignore { parent: 0 }\n}\n",
    ),
    (
        "dir.rs",
        "pub struct Ignore {\n    parent: u32,\n}\n\n\
         impl Ignore {\n    pub fn matched(&self) -> bool {\n        true\n    }\n}\n\n\
         fn build() -> Ignore {\n    Ignore { parent: 0 }\n}\n",
    ),
    (
        "dir.rs",
        "pub fn ignore() -> Ignore {\n    build()\n}\n\n\
         fn build() -> Ignore {\n    Ignore { parent: 0 }\n}\n\n\
         impl Ignore {\n    pub fn matched(&self) -> bool {\n        true\n    }\n}\n\n\
         pub struct Ignore {\n    parent: u32,\n}\n",
    ),
    (
        "literal.rs",
        "fn e(s: &str) -> u32 {\n    1\n}\n\nfn E(s: &str) -> u32 {\n    2\n}\n",
    ),
    (
        "walk.rs",
        "#[cfg(unix)]\nfn device_num(p: &str) -> u64 {\n    1\n}\n\n\
         #[cfg(windows)]\nfn device_num(p: &str) -> u64 {\n    2\n}\n",
    ),
    (
        "walk.rs",
        "#[cfg(unix)]\nfn device_num(p: &str) -> u64 {\n    1\n}\n\n\
         #[cfg(windows)]\nfn device_num(p: &str) -> u64 {\n    2\n}\n\nstruct Device_Num;\n",
    ),
    (
        "dir.rs",
        "pub struct Ignore {\n    parent: u32,\n}\n\n\
         impl Ignore {\n    pub fn matched(&self) -> bool {\n        true\n    }\n}\n\n\
         pub fn ignore() -> Ignore {\n    build()\n}\n\n\
         fn build() -> Ignore {\n    Ignore { parent: 0 }\n}\n\
         \nfn e(s: &str) -> u32 {\n    1\n}\n\
         \nfn E(s: &str) -> u32 {\n    2\n}\n\
         \nfn caller() -> u32 {\n    let _ = ignore();\n    e(\"a\") + E(\"b\")\n}\n",
    ),
    (
        "a/src/lib.rs",
        "pub enum Field {\n    Value(String),\n}\npub struct Map;\n",
    ),
    (
        "b/src/lib.rs",
        "use serde_json::{Map, Value};\n\npub fn f(m: &Map<String, Value>) {}\n",
    ),
    ("c/src/lib.rs", "pub struct Value;\n"),
    ("osiris-types/src/lib.rs", "pub struct Other;\n"),
    ("server/src/lib.rs", "pub struct Config;\n"),
    (
        "plugin/src/lib.rs",
        "use osiris_types::Config;\n\npub fn f(c: &Config) {}\n",
    ),
    ("osiris-types/src/lib.rs", "pub struct Config;\n"),
    ("other/src/lib.rs", "pub struct FromRequestParts;\n"),
    (
        "api/src/lib.rs",
        "pub(crate) use corelib::FromRequestParts;\n",
    ),
    (
        "lib.rs",
        "struct Child;\n\
         impl Drop for Child { fn drop(&mut self) {} }\n\
         fn total() -> usize { 0 }\n\
         fn close(c: Child) { drop(c); }\n\
         fn count(v: &Stats) -> usize { v.total() }\n",
    ),
    (
        "lib.rs",
        "fn compute(x: u8) -> u8 { x }\n\
         fn run() { println!(\"{}\", compute(1)); let _y = compute(2); }\n",
    ),
    ("src/c.rs", "pub fn helper() {}\n"),
    ("src/a.rs", "pub fn run() { util::helper(); }\n"),
    ("src/util/mod.rs", "pub fn helper() {}\n"),
    ("src/b/mod.rs", "pub fn helper() {}\n"),
    ("src/a.rs", "pub fn run() { Helper::helper(); }\n"),
    (
        "src/helper.rs",
        "pub struct Helper;\nimpl Helper { pub fn helper() {} }\npub fn helper() {}\n",
    ),
    ("src/a.rs", "pub fn run() { crate::util::helper(); }\n"),
    ("src/util.rs", "pub fn helper() {}\n"),
    (
        "src/a.rs",
        "pub fn run() { let _ = std::fs::read(\"x\"); }\n",
    ),
    (
        "src/fs.rs",
        "pub fn read(p: &str) -> Vec<u8> { Vec::new() }\n",
    ),
    (
        "src/a.rs",
        "use std::fs;\npub fn run() { let _ = fs::read(\"x\"); }\n",
    ),
    (
        "src/a.rs",
        "use crate::util;\npub fn run() { util::helper(); }\n",
    ),
    (
        "crate_a/src/main.rs",
        "mod util;\nfn run() { crate::util::f(); }\n",
    ),
    ("crate_a/src/util.rs", "pub fn g() {}\n"),
    ("crate_b/src/util.rs", "pub fn f() {}\n"),
    (
        "src/store.rs",
        "pub struct Db;\nimpl Db { pub fn open(&self) {} }\n",
    ),
    ("src/main.rs", "mod store;\nfn run() { store::open(); }\n"),
];

/// smoke; the real checks are the ihc.18-ignored tests
///
/// Runs the extractor, without `resolve`, on the `rsproj` files and every inline source above.
/// Go's `TestExtractRust` asserts no node count, so none is asserted here.
#[test]
fn smoke_extract_rust_sources() {
    kinds::register_kinds();
    let root = testdata().join("extract").join("rsproj");
    // extract_rust directly: file/file_from_bytes first run the MCP-config check, a stub until
    // graphify-rs-ihc.17. `None` is Go's parse-budget recover.
    for f in ["util/math.rs", "web/server.rs"] {
        let src = std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("read {f}: {e}"));
        assert!(
            crate::extract::rust::extract_rust(f, &src).is_some(),
            "rsproj/{f}: parse over budget"
        );
    }
    for (i, (rel, src)) in SMOKE_SRCS.iter().enumerate() {
        assert!(
            crate::extract::rust::extract_rust(rel, src.as_bytes()).is_some(),
            "inline source #{i} ({rel}): parse over budget"
        );
    }
}
