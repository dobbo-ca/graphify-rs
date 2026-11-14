//! graphify-go `internal/extract/kotlin_test.go`, and the cases of the upstream graphify v0.9.107
//! `test_kotlin_*.py` files whose assertion holds for graphify-go v0.12.0 (see the `extract-kotlin`
//! block of `parity/README.md`). The upstream sources are written inline, so no fixture is copied.
//! Upstream names the nodes of a method `.name()`; Go labels them `Owner.name()`. A case that holds
//! only because Go lacks the feature carries the `pins Go v0.12.0` comment.

use std::collections::{BTreeMap, BTreeSet};

use crate::extract::{file, file_from_bytes, kinds, resolve};
use crate::model::Extraction;
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
#[ignore = "graphify-rs-ihc.37"]
fn test_extract_kotlin() {
    let ext = extract_proj("ktproj", &["util/math.kt", "web/server.kt"]);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in [
        "math.kt",
        "server.kt",
        "add()",
        "Server",
        "Server.start()",
        "Server.boot()",
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
    // Method scoped under its class, with a contains edge.
    assert!(
        has("Server", "contains", "Server.start()"),
        "expected Server --contains--> Server.start"
    );
    // Same-file call: Server.start -> boot (method, resolved locally).
    assert!(
        has("Server.start()", "calls", "Server.boot()"),
        "expected Server.start --calls--> Server.boot"
    );
    // Cross-file call: boot -> add (unique global definition).
    assert!(
        has("Server.boot()", "calls", "add()"),
        "expected cross-file Server.boot --calls--> add"
    );
    // External import edge for kotlin.math.sqrt.
    let imports = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "imports")
        .count();
    assert!(
        imports > 0,
        "no external import edges (expected kotlin.math.sqrt)"
    );
}

/// Go `TestExtractKotlinEnum`: enum entries become nodes linked to the enum class by a `case_of`
/// edge, and an entry's anonymous class body still contributes its methods (attached to the
/// entry).
#[test]
#[ignore = "graphify-rs-ihc.37"]
fn test_extract_kotlin_enum() {
    kinds::register_kinds();
    let src = b"enum class Color {
  RED,
  GREEN {
    fun greet() {}
  };
}";
    let res = super::extract_kotlin("Color.kt", src).expect("parse budget");

    let id2label: BTreeMap<&str, &str> = res
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    let labels: BTreeSet<&str> = res.nodes.iter().map(|n| n.label.as_str()).collect();
    for want in ["Color", "RED", "GREEN", "GREEN.greet()"] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }

    let has = |src: &str, rel: &str, tgt: &str| {
        res.edges.iter().any(|e| {
            e.relation.as_str() == rel
                && id2label.get(e.source.as_str()) == Some(&src)
                && id2label.get(e.target.as_str()) == Some(&tgt)
        })
    };
    assert!(
        has("Color", "case_of", "RED"),
        "expected Color --case_of--> RED"
    );
    assert!(
        has("Color", "case_of", "GREEN"),
        "expected Color --case_of--> GREEN"
    );
    // Anonymous-body method attaches to the entry, not the enum class.
    assert!(
        has("GREEN", "contains", "GREEN.greet()"),
        "expected GREEN --contains--> GREEN.greet"
    );
}

// ---- upstream graphify v0.9.107 `test_kotlin_*.py` ----

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

fn id2label(ext: &Extraction) -> BTreeMap<&str, &str> {
    ext.nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect()
}

/// Upstream `_find`: the id of the first node with this label.
fn find<'a>(ext: &'a Extraction, label: &str) -> &'a str {
    ext.nodes
        .iter()
        .find(|n| n.label == label)
        .unwrap_or_else(|| panic!("no node labelled {label:?}"))
        .id
        .as_str()
}

/// The labels of the targets of every `relation` edge whose source has the label `src`.
fn targets_from<'a>(ext: &'a Extraction, src: &str, relation: &str) -> Vec<&'a str> {
    let label = id2label(ext);
    let ids: BTreeSet<&str> = ext
        .nodes
        .iter()
        .filter(|n| n.label == src)
        .map(|n| n.id.as_str())
        .collect();
    ext.edges
        .iter()
        .filter(|e| e.relation.as_str() == relation && ids.contains(e.source.as_str()))
        .map(|e| label.get(e.target.as_str()).copied().unwrap_or(""))
        .collect()
}

fn has_edge(ext: &Extraction, src: &str, relation: &str, tgt: &str) -> bool {
    targets_from(ext, src, relation).contains(&tgt)
}

/// A node without a `source_file` (a stub).
fn sourceless(ext: &Extraction) -> Vec<(&str, &str)> {
    let mut v: Vec<(&str, &str)> = ext
        .nodes
        .iter()
        .filter(|n| n.source_file.is_empty())
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    v.sort();
    v
}

// ---- test_kotlin_grammar.py ----

const IMPORT_MODEL_FILES: [(&str, &str); 2] = [
    (
        "model/Money.kt",
        "package com.demo.model\n\nclass Money(val amount: Int)\n",
    ),
    (
        "model/Ledger.kt",
        "package com.demo.model\n\nclass Ledger {\n    fun record(m: Money) { }\n}\n",
    ),
];

#[test]
#[ignore = "graphify-rs-ihc.37"]
fn test_kotlin_wildcard_import_emits_no_symbol_edge() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 B24
    // Go records the package path `com.demo.model` as an external dependency node, never the
    // last segment, `*` or an empty name.
    let ext = run(&[
        IMPORT_MODEL_FILES[0],
        IMPORT_MODEL_FILES[1],
        (
            "app/Main.kt",
            "package com.demo.app\n\nimport com.demo.model.*\n\nfun main() { }\n",
        ),
    ]);
    let main_file = find(&ext, "Main.kt");
    let label = id2label(&ext);
    for e in &ext.edges {
        if e.relation.as_str() == "imports" && e.source == main_file {
            let tgt = label.get(e.target.as_str()).copied().unwrap_or("");
            assert!(
                !["model", "*", ""].contains(&tgt),
                "a wildcard import names a PACKAGE; a symbol-level edge to the last segment is a phantom"
            );
        }
    }
}

const FQ_CORPUS: [(&str, &str); 3] = [
    (
        "lib/Lib.kt",
        "package com.demo.lib\n\nfun BetaScreen() { }\n\nobject Help {\n    fun help() { }\n}\n",
    ),
    (
        "feature/Feature.kt",
        "package com.demo.feature\n\nfun DeltaScreen() { }\n\nfun SamePackageCaller() {\n    com.demo.feature.DeltaScreen()\n}\n",
    ),
    (
        "nav/Nav.kt",
        "package com.demo.nav\n\nfun NavGraph() {\n    com.demo.lib.BetaScreen()\n    com.demo.feature.DeltaScreen()\n    com.demo.lib.Help.help()\n    com.nonexistent.pkg.Thing()\n}\n",
    ),
];

#[test]
#[ignore = "graphify-rs-ihc.37"]
fn test_kotlin_fq_call_to_unknown_package_yields_no_edge() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 B24
    // Go has no package table; `Thing` has no definition, so no edge and no node.
    let ext = run(&FQ_CORPUS);
    find(&ext, "NavGraph()");
    for t in targets_from(&ext, "NavGraph()", "calls") {
        assert!(
            !t.contains("Thing") && !t.contains("thing"),
            "`com.nonexistent.pkg.Thing()` is external: no edge, no fabricated node; got {t:?}"
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.37"]
fn test_kotlin_fq_call_to_ambiguous_name_yields_no_edge() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 B24
    // Go qualifies a receiver by path tokens: `dup` names neither `dup1/` nor `dup2/`, so
    // nothing binds (upstream: the exactly-one-candidate guard refuses to pick).
    let ext = run(&[
        ("dup1/D1.kt", "package com.demo.dup\n\nfun Same() { }\n"),
        ("dup2/D2.kt", "package com.demo.dup\n\nfun Same() { }\n"),
        (
            "callr/Caller.kt",
            "package com.demo.callr\n\nfun Caller() {\n    com.demo.dup.Same()\n}\n",
        ),
    ]);
    find(&ext, "Caller()");
    assert!(
        targets_from(&ext, "Caller()", "calls").is_empty(),
        "`Same` is defined twice in com.demo.dup: no pick"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.37"]
fn test_kotlin_object_member_call_same_file_control_unaffected() {
    let ext = run(&[(
        "Same.kt",
        "package com.demo.same\n\nobject Config {\n    fun load() { }\n}\n\nfun Start() {\n    Config.load()\n}\n",
    )]);
    assert!(
        has_edge(&ext, "Start()", "calls", "Config.load()"),
        "the pre existing same file object member call resolution must be unaffected"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.37"]
fn test_kotlin_object_member_call_ambiguous_receiver_yields_no_edge() {
    let ext = run(&[
        (
            "one/One.kt",
            "package com.demo.one\n\nobject Config {\n    fun load() { }\n}\n",
        ),
        (
            "two/Two.kt",
            "package com.demo.two\n\nobject Config {\n    fun load() { }\n}\n",
        ),
        (
            "callr/Caller.kt",
            "package com.demo.callr\n\nfun Start() {\n    Config.load()\n}\n",
        ),
    ]);
    find(&ext, "Start()");
    assert!(
        targets_from(&ext, "Start()", "calls").is_empty(),
        "`Config` exists in two packages: the exactly one candidate guard must refuse to pick"
    );
}

const INIT_CORPUS: [(&str, &str); 3] = [
    (
        "lib/Lib.kt",
        r#"package com.demo.lib

class Repo

class HttpClient(val url: String)

fun createRepo(): Repo {
    return Repo()
}

fun base(): String {
    return ""
}

fun compute(): Int {
    return 1
}

fun companionInit(): Int {
    return 2
}
"#,
    ),
    (
        "app/Service.kt",
        r#"package com.demo.app

import com.demo.lib.HttpClient
import com.demo.lib.base
import com.demo.lib.companionInit
import com.demo.lib.compute
import com.demo.lib.createRepo

class Service {
    val repo = createRepo()
    private val client = HttpClient(base())
    val x by lazy {
        compute()
    }
    val plain = 5
    companion object {
        val shared = companionInit()
    }
    fun go() {
        val r = createRepo()
    }
}
"#,
    ),
    (
        "app/TopLevel.kt",
        "package com.demo.app\n\nval topRepo = createRepo()\n",
    ),
];

#[test]
#[ignore = "graphify-rs-ihc.37"]
fn test_kotlin_literal_initializer_emits_nothing() {
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 B43
    // Go records no call of a property initializer at all, a literal or not.
    let ext = run(&INIT_CORPUS);
    let service = find(&ext, "Service");
    let plain_line = INIT_CORPUS[1]
        .1
        .lines()
        .position(|l| l == "    val plain = 5")
        .expect("the `val plain` line")
        + 1;
    let at = format!("L{plain_line}");
    assert!(
        !ext.edges
            .iter()
            .any(|e| e.source == service && e.source_location.to_string() == at),
        "`val plain = 5` contains no call: nothing to emit"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.37"]
fn test_kotlin_function_body_calls_unchanged() {
    let ext = run(&INIT_CORPUS);
    assert!(has_edge(&ext, "Service.go()", "calls", "createRepo()"));
    assert!(has_edge(&ext, "createRepo()", "calls", "Repo"));
}

// ---- test_kotlin_generic_type_params.py ----

#[test]
#[ignore = "graphify-rs-ihc.37"]
fn test_generic_class_and_method_type_params_mint_no_stub() {
    let ext = run(&[(
        "K.kt",
        "class Box<T>(val v: T) {\n    fun get(): T? = v\n    fun <R> map(f: (T) -> R): R? = null\n}\n",
    )]);
    assert_eq!(sourceless(&ext), Vec::<(&str, &str)>::new());
    assert!(
        !ext.edges.iter().any(|e| e.target == "t" || e.target == "r"),
        "an edge targets a bare `t` or `r`"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.37"]
fn test_user_and_container_types_still_referenced() {
    let ext = run(&[(
        "K.kt",
        "class User(val name: String)\nclass Box<T>(val v: T)\nfun wrap(u: User): Box<User> = Box(u)\n",
    )]);
    let label = id2label(&ext);
    let targets: BTreeSet<&str> = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "references")
        .filter_map(|e| label.get(e.target.as_str()).copied())
        .collect();
    assert!(
        targets.contains("User"),
        "no reference to User: {targets:?}"
    );
    assert!(targets.contains("Box"), "no reference to Box: {targets:?}");
    assert!(!targets.contains("T"), "a reference to T: {targets:?}");
    assert_eq!(sourceless(&ext), Vec::<(&str, &str)>::new());
}
