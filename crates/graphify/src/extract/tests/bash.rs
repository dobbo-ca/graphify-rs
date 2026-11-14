//! graphify-go `internal/extract/bash_test.go`.

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
#[ignore = "graphify-rs-ihc.18"] // bash test (ihc.35, bash_test.go): needs resolve
fn test_extract_bash() {
    let ext = extract_proj("bashproj", &["util/math.sh", "app/main.sh"]);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in ["math.sh", "main.sh", "add()", "double()", "boot()"] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }

    let has = |src: &str, rel: &str, tgt: &str| {
        ext.edges.iter().any(|e| {
            e.relation.as_str() == rel
                && id2label.get(e.source.as_str()) == Some(&src)
                && id2label.get(e.target.as_str()) == Some(&tgt)
        })
    };

    // File contains its function definition.
    assert!(
        has("math.sh", "contains", "add()"),
        "expected math.sh --contains--> add()"
    );
    // Same-file call: double -> add.
    assert!(
        has("double()", "calls", "add()"),
        "expected double --calls--> add"
    );
    // Cross-file call: boot (app/main.sh) -> add (util/math.sh), unique global.
    assert!(
        has("boot()", "calls", "add()"),
        "expected cross-file boot --calls--> add"
    );

    // `source "../util/math.sh"` resolves to a corpus file (imports_from).
    assert!(
        has("main.sh", "imports_from", "math.sh"),
        "expected main.sh --imports_from--> math.sh"
    );

    // `. /etc/profile` is an unresolved (external) include.
    let imports = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "imports")
        .count();
    assert!(
        imports > 0,
        "no external import edges (expected /etc/profile)"
    );
}

/// Every distinct bash `(rel, src)` of `bash_scriptinvocation.rs` (the `write_script` bodies, with
/// `INVOCATION_CMDS` and `RUNNER_CASES` expanded), verbatim. The python, js and php sources are
/// left out: their extractors are stubs.
const SMOKE_SRCS: &[(&str, &str)] = &[
    ("helpers.sh", "#!/bin/bash\necho helper\n"),
    ("deploy.sh", "#!/bin/bash\n./helpers.sh\n"),
    ("deploy.sh", "#!/bin/bash\nbash ./helpers.sh\n"),
    (
        "deploy.sh",
        "#!/bin/bash\ndeploy() { bash ./helpers.sh; }\n",
    ),
    (
        "deploy.sh",
        "#!/bin/bash\nbash() { echo custom; }\nbash ./helpers.sh\n",
    ),
    ("deploy.sh", "#!/bin/bash\nbash \"./$SCRIPT.sh\"\n"),
    ("deploy.sh", "#!/bin/bash\n./missing.sh\n"),
    ("run.sh", "#!/bin/bash\npython3 build.py\n"),
    ("run.sh", "#!/bin/bash\nnode build.js\n"),
    ("run.sh", "#!/bin/bash\npython3 -u build.py\n"),
    ("run.sh", "#!/bin/bash\n\"$PY\" build.py\n"),
    ("run.sh", "#!/bin/bash\npython3 \"$X\"\n"),
    ("b1.sh", "#!/bin/bash\npython3 -m app.main\n"),
    ("deploy.sh", "#!/bin/bash\n"),
    ("ci.sh", "#!/bin/bash\nsh -e ./deploy.sh\n"),
    ("ci2.sh", "#!/bin/bash\nbash -e deploy.sh\n"),
    ("e1.sh", "#!/bin/bash\nphp -e s.php\n"),
];

/// smoke; the real checks are the ihc.18-ignored tests
///
/// Runs the extractor, without `resolve`, on the `bashproj` files and every inline bash script of
/// the resolve-dependent tests. Go's `TestExtractBash` asserts no node count, so none is asserted
/// here.
#[test]
fn smoke_extract_bash_sources() {
    kinds::register_kinds();
    let root = testdata().join("extract").join("bashproj");
    // extract_bash directly: file/file_from_bytes first run the MCP-config check, a stub until
    // graphify-rs-ihc.17. `None` is Go's parse-budget recover.
    for f in ["util/math.sh", "app/main.sh"] {
        let src = std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("read {f}: {e}"));
        assert!(
            crate::extract::bash::extract_bash(f, &src).is_some(),
            "bashproj/{f}: parse over budget"
        );
    }
    for (i, (rel, src)) in SMOKE_SRCS.iter().enumerate() {
        assert!(
            crate::extract::bash::extract_bash(rel, src.as_bytes()).is_some(),
            "inline source #{i} ({rel}): parse over budget"
        );
    }
}
