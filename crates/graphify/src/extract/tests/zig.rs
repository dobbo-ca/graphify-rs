//! graphify-go `internal/extract/zig_test.go`.

use std::collections::{BTreeMap, BTreeSet};

use crate::extract::{file, kinds, resolve};
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
#[ignore = "graphify-rs-ihc.18"]
fn test_extract_zig() {
    let ext = extract_proj("zigproj", &["util/math.zig", "web/server.zig"]);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in [
        "math.zig",
        "server.zig",
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
    // Method scoped under its container type, with a contains edge.
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

    // External import edge for std (@import("std")).
    let imports = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "imports")
        .count();
    assert!(imports > 0, "no external import edges (expected std)");
}

/// What `extractZig` itself records (zig.go:20-193), before `resolve`.
#[test]
fn test_extract_zig_unresolved() {
    kinds::register_kinds();
    let run = |f: &str| {
        let path = testdata().join("extract/zigproj").join(f);
        super::extract_zig(f, &std::fs::read(path).unwrap()).expect("parse budget")
    };
    let id_of = |res: &crate::extract::Result, label: &str| {
        res.nodes
            .iter()
            .find(|n| n.label == label)
            .unwrap_or_else(|| panic!("missing node label {label:?}"))
            .id
            .clone()
    };
    let calls = |res: &crate::extract::Result, caller: &str| -> Vec<(String, String, String)> {
        let id = id_of(res, caller);
        res.calls
            .iter()
            .map(|c| res.call(c))
            .filter(|c| c.caller_id == id)
            .map(|c| (c.callee, c.recv, c.loc.to_string()))
            .collect()
    };

    // zig.go:36 zigFunc: a def and a contains edge per top-level fn; zig.go:55 imports.
    let math = run("util/math.zig");
    let labels: Vec<&str> = math.nodes.iter().map(|n| n.label.as_str()).collect();
    assert_eq!(labels, ["math.zig", "add()", "double()"]);
    assert_eq!(math.imps.len(), 1);
    assert_eq!(math.imps[0].spec, "std");
    // zig.go:187 zigCalls: a direct call records the identifier, no receiver.
    assert_eq!(
        calls(&math, "double()"),
        [("add".to_string(), String::new(), "L8".to_string())]
    );

    // zig.go:66 zigType: the container and its method.
    let server = run("web/server.zig");
    let labels: Vec<&str> = server.nodes.iter().map(|n| n.label.as_str()).collect();
    assert_eq!(labels, ["server.zig", "Server", "Server.start()", "boot()"]);
    assert_eq!(server.imps[0].spec, "std");
    let (server_id, start_id) = (id_of(&server, "Server"), id_of(&server, "Server.start()"));
    assert!(
        server
            .edges
            .iter()
            .any(|e| e.relation.as_str() == "contains"
                && e.source == server_id
                && e.target == start_id),
        "expected Server --contains--> Server.start"
    );
    // zig.go:87 the method is registered under its bare name.
    assert!(
        server
            .defs
            .iter()
            .any(|d| d.id == start_id && d.name == "start")
    );
    // zig.go:187 the call inside the method body is attributed to the method.
    assert_eq!(
        calls(&server, "Server.start()"),
        [("boot".to_string(), String::new(), "L8".to_string())]
    );
    assert_eq!(
        calls(&server, "boot()"),
        [("add".to_string(), String::new(), "L13".to_string())]
    );
}

/// Go `unquote` (golang.go:394) is `strings.Trim(s, "`\"'")`.
#[test]
fn test_unquote() {
    assert_eq!(crate::extract::golang::unquote("\"std\""), "std");
    assert_eq!(crate::extract::golang::unquote("`a'"), "a");
    assert_eq!(crate::extract::golang::unquote("\"a\"b\""), "a\"b");
}
