//! graphify-go `internal/extract/c_test.go`.

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
fn test_extract_c() {
    let ext = extract_proj("cproj", &["util/math.c", "web/server.c"]);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in [
        "math.c",
        "server.c",
        "add()",
        "Server",
        "Server.start()",
        "boot()",
        "run()",
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
    // Function-pointer field scoped under its struct, with a contains edge.
    assert!(
        has("Server", "contains", "Server.start()"),
        "expected Server --contains--> Server.start"
    );
    // Same-file call: run -> boot.
    assert!(
        has("run()", "calls", "boot()"),
        "expected run --calls--> boot"
    );
    // Cross-file call: boot -> add (unique global definition).
    assert!(
        has("boot()", "calls", "add()"),
        "expected cross-file boot --calls--> add"
    );

    // External import edges for the included headers.
    let imports = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "imports")
        .count();
    assert!(
        imports > 0,
        "no external import edges (expected stdio.h / helper.h)"
    );
}

/// smoke; the real checks are the ihc.18-ignored tests
#[test]
fn test_extract_c_smoke() {
    kinds::register_kinds();
    let root = testdata().join("extract").join("cproj");
    let mut srcs: Vec<(String, Vec<u8>)> = ["util/math.c", "web/server.c"]
        .iter()
        .map(|f| (f.to_string(), std::fs::read(root.join(f)).unwrap()))
        .collect();
    // The two .c-only tests of preproc_test.go.
    srcs.push(("e.c".into(), b"#ifdef A\nint split(int a) {\n#else\nint split(long a) {\n#endif\n  return after();\n}\nint after(void) { return 0; }\n".to_vec()));
    srcs.push(("a.c".into(), b"int x1(void) { return 0; }\n#ifdef A\nint dup(void) { return x1(); }\n#else\nint dup(void) { return x1(); }\n#endif\n".to_vec()));
    srcs.push(("b.c".into(), b"int user(void) { return dup(); }\n".to_vec()));
    for (rel, src) in &srcs {
        assert!(super::extract_c(rel, src).is_some(), "extract_c({rel})");
        let res = crate::extract::file_from_bytes(rel, src);
        assert!(!res.nodes.is_empty(), "{rel}: no file node");
    }
    let res = super::extract_c("web/server.c", &srcs[1].1).unwrap();
    let labels: Vec<&str> = res.nodes.iter().map(|n| n.label.as_str()).collect();
    assert_eq!(
        labels,
        ["server.c", "Server", "Server.start()", "boot()", "run()"]
    );
}
