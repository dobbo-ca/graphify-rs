//! graphify-go `internal/extract/lua_test.go`, and the cases of the upstream graphify v0.9.107
//! `test_lua_import.py` and `test_lua_method_calls.py` whose assertion holds for graphify-go
//! v0.12.0 (see the `extract-lua` block of `parity/README.md`). The upstream sources are written
//! inline, so no fixture is copied. Upstream labels a colon method `Animal:speak()`; Go labels
//! every method `Table.name()`. A dotted `require("config.lazy")` names no file in Go: it is an
//! external dependency node labelled `config.lazy`, never an `imports_from` edge to
//! `config.lazy.lua`.

use std::collections::{BTreeMap, BTreeSet};

use super::extract_lua;
use crate::extract::{file_from_bytes, kinds, resolve};
use crate::model::Extraction;
use crate::testutil::testdata;

// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_extract_lua() {
    kinds::register_kinds();
    let root = testdata().join("extract").join("luaproj");
    let files = ["util/math.lua", "web/server.lua"];

    // Go calls extractLua directly: the central File() dispatch is wired elsewhere.
    let results: Vec<_> = files
        .iter()
        .map(|f| {
            let src = std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("read {f}: {e}"));
            extract_lua(f, &src).unwrap_or_else(|| panic!("extract_lua({f}): over budget"))
        })
        .collect();
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in [
        "math.lua",
        "server.lua",
        "add()",
        "boot()",
        "M",
        "M.double()",
        "Server",
        "Server.start()",
        "socket", // external dependency from require
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

    // Method scoped under its table, with a contains edge.
    assert!(
        has("M", "contains", "M.double()"),
        "expected M --contains--> M.double()"
    );
    assert!(
        has("Server", "contains", "Server.start()"),
        "expected Server --contains--> Server.start()"
    );
    // Same-file call: M.double -> add.
    assert!(
        has("M.double()", "calls", "add()"),
        "expected M.double --calls--> add"
    );
    // Same-file call: Server.start -> boot.
    assert!(
        has("Server.start()", "calls", "boot()"),
        "expected Server.start --calls--> boot"
    );
    // Cross-file call: boot (server.lua) -> add (unique global in math.lua).
    assert!(
        has("boot()", "calls", "add()"),
        "expected cross-file boot --calls--> add"
    );
    // require("socket") recorded as an external import.
    let has_import = ext.edges.iter().any(|e| {
        e.relation.as_str() == "imports" && id2label.get(e.target.as_str()) == Some(&"socket")
    });
    assert!(
        has_import,
        "expected an imports edge to external dependency socket"
    );
}

// ---- upstream graphify v0.9.107 `test_lua_*.py` ----

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

/// Every `import` of `init.lua` in `srcs` is an `imports` edge to an external node of the module
/// name; none is an `imports_from` edge.
fn assert_external_imports(srcs: &[(&str, &str)], modules: &[&str]) {
    let ext = run(srcs);
    for m in modules {
        assert!(
            has_edge(&ext, "init.lua", "imports", m),
            "expected init.lua --imports--> {m}; got {:?}",
            pairs(&ext, "imports")
        );
    }
    assert!(
        pairs(&ext, "imports_from").is_empty(),
        "unexpected imports_from edges: {:?}",
        pairs(&ext, "imports_from")
    );
}

// test_lua_import.py: bare `require` statements (#3320).

// pins Go v0.12.0 (external node `config.lazy`, no module-path resolution); upstream 0.9.107 emits an imports edge to the `config.lazy.lua` file node -> P3
// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lua_bare_require_emits_import_edge() {
    assert_external_imports(
        &[
            ("init.lua", "require(\"config.lazy\")\n"),
            ("config.lazy.lua", "-- lazy\n"),
        ],
        &["config.lazy"],
    );
}

// pins Go v0.12.0 (external node `config.lazy`, no module-path resolution); upstream 0.9.107 emits an imports edge to the `config.lazy.lua` file node -> P3
// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lua_assigned_require_still_emits_import_edge() {
    assert_external_imports(
        &[
            ("init.lua", "local ok = require(\"config.lazy\")\n"),
            ("config.lazy.lua", "-- lazy\n"),
        ],
        &["config.lazy"],
    );
}

// pins Go v0.12.0 (external nodes `config.options` and `config.keymaps`, no module-path resolution); upstream 0.9.107 emits imports edges to the two file nodes -> P3
// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lua_multiple_bare_requires_each_emit_an_import_edge() {
    assert_external_imports(
        &[
            (
                "init.lua",
                "require(\"config.options\")\nrequire(\"config.keymaps\")\n",
            ),
            ("config.options.lua", "-- options\n"),
            ("config.keymaps.lua", "-- keymaps\n"),
        ],
        &["config.options", "config.keymaps"],
    );
}

// pins Go v0.12.0 (external node `config.lazy`: the spec is never mapped to `lua/config/lazy.lua`); upstream 0.9.107 emits an imports edge to the `lua/config/lazy.lua` file node -> P3
// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lua_bare_require_resolves_lua_directory_layout() {
    let srcs = [
        ("init.lua", "require(\"config.lazy\")\n"),
        ("lua/config/lazy.lua", "-- lazy\n"),
    ];
    assert_external_imports(&srcs, &["config.lazy"]);
    assert!(
        !has_edge(&run(&srcs), "init.lua", "imports", "lazy.lua"),
        "the require must not bind to lua/config/lazy.lua"
    );
}

// pins Go v0.12.0 (external node `config.lazy`, no module-path resolution); upstream 0.9.107 emits an imports edge to the `config.lazy.lua` file node -> P3
// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lua_bare_require_without_parens_single_quotes() {
    assert_external_imports(
        &[
            ("init.lua", "require 'config.lazy'\n"),
            ("config.lazy.lua", "-- lazy\n"),
        ],
        &["config.lazy"],
    );
}

// pins Go v0.12.0 (`foo.require("mod")` and `bar:require("mod")` are imports of `mod`: the callee name `require` is all it checks); upstream 0.9.107 emits no import -> P3
// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lua_method_style_require_is_not_an_import() {
    assert_external_imports(
        &[
            ("mod.lua", "-- mod\n"),
            ("init.lua", "foo.require(\"mod\")\nbar:require(\"mod\")\n"),
        ],
        &["mod"],
    );
}

// pins Go v0.12.0 (external node `config.lazy`, no module-path resolution); upstream 0.9.107 emits an imports edge to the `config.lazy.lua` file node -> P3
// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lua_bare_require_without_parens_double_quotes() {
    assert_external_imports(
        &[
            ("init.lua", "require \"config.lazy\"\n"),
            ("config.lazy.lua", "-- lazy\n"),
        ],
        &["config.lazy"],
    );
}

// test_lua_method_calls.py: `self:other()` and dot calls (#3991).

// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lua_self_colon_call_resolves_to_sibling_method() {
    let ext = run(&[(
        "animal.lua",
        "local Animal = {}\n\
         function Animal:speak()\n\
         \x20   return self:describe()\n\
         end\n\
         function Animal:describe()\n\
         \x20   return self.name\n\
         end\n",
    )]);
    assert!(
        has_edge(&ext, "Animal.speak()", "calls", "Animal.describe()"),
        "got {:?}",
        pairs(&ext, "calls")
    );
}

// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lua_self_colon_call_resolves_dot_defined_sibling() {
    let ext = run(&[(
        "animal.lua",
        "local Animal = {}\n\
         function Animal.helper()\n\
         \x20   return 1\n\
         end\n\
         function Animal:run()\n\
         \x20   return self:helper()\n\
         end\n",
    )]);
    assert!(
        has_edge(&ext, "Animal.run()", "calls", "Animal.helper()"),
        "got {:?}",
        pairs(&ext, "calls")
    );
}

// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lua_self_colon_call_does_not_bind_to_unrelated_bare_function() {
    let ext = run(&[(
        "animal.lua",
        "function process()\n\
         \x20   return 1\n\
         end\n\
         local Animal = {}\n\
         function Animal:run()\n\
         \x20   return self:process()\n\
         end\n",
    )]);
    assert!(
        !has_edge(&ext, "Animal.run()", "calls", "process()"),
        "self:process() bound to the bare function process()"
    );
}

// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lua_colon_call_on_unknown_receiver_is_fail_closed() {
    let ext = run(&[(
        "animal.lua",
        "function process()\n\
         \x20   return 1\n\
         end\n\
         local Animal = {}\n\
         function Animal:run()\n\
         \x20   local obj = make()\n\
         \x20   return obj:process()\n\
         end\n",
    )]);
    assert!(
        !has_edge(&ext, "Animal.run()", "calls", "process()"),
        "obj:process() bound by bare name to process()"
    );
}

// lua test (ihc.39): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_lua_dot_and_plain_calls_still_resolve() {
    let ext = run(&[(
        "mod.lua",
        "local M = {}\n\
         function M.helper()\n\
         \x20   return 1\n\
         end\n\
         function M.main()\n\
         \x20   return M.helper()\n\
         end\n",
    )]);
    assert!(
        has_edge(&ext, "M.main()", "calls", "M.helper()"),
        "got {:?}",
        pairs(&ext, "calls")
    );
}
