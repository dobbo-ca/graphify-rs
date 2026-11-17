//! Upstream graphify v0.9.107 `tests/test_out_dir_evidence.py` (6 tests, all ported): a bare
//! `out` directory is pruned only on build-output evidence (Go `skipDir` and
//! `hasBuildOutputMarkers`, detect.go:123-238). Upstream calls `detect(td)`; the Go walk is
//! `detect::collect_files`, so these need only detect (graphify-rs-ihc.14). Upstream's
//! `monkeypatch.chdir` does not reach the walk of an absolute root, so it is dropped.

use super::core_util::write_tree;
use crate::detect::collect_files;
use crate::testutil::TempDir;

/// The collected files, each with a leading `/`: upstream checks absolute paths for `/out/`,
/// and Go's are relative to the root.
fn scanned(root: &TempDir) -> Vec<String> {
    collect_files(root.path())
        .unwrap_or_else(|e| panic!("collect_files: {e}"))
        .into_iter()
        .map(|f| format!("/{f}"))
        .collect()
}

// adapter/out/ and port/out/ holding only source stay in the corpus.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_hexagonal_out_layers_are_scanned() {
    let root = TempDir::new();
    write_tree(
        root.path(),
        &[
            (
                "src/main/java/demo/adapter/out/persistence/OrderEntity.java",
                "package demo.adapter.out.persistence;\npublic class OrderEntity { }\n",
            ),
            (
                "src/main/java/demo/port/out/OrderRepositoryPort.java",
                "package demo.port.out;\npublic interface OrderRepositoryPort { }\n",
            ),
        ],
    );
    let files = scanned(&root);
    assert!(
        files
            .iter()
            .any(|f| f.contains("adapter/out/persistence/OrderEntity.java")),
        "{files:?}"
    );
    assert!(
        files
            .iter()
            .any(|f| f.contains("port/out/OrderRepositoryPort.java")),
        "{files:?}"
    );
}

// out/production/... (IntelliJ compile output) stays out of the corpus. Go collects no `.class`
// file anyway; the `production` marker is what prunes `out`.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_intellij_out_production_is_pruned() {
    let root = TempDir::new();
    root.write("out/production/demo/Order.class", b"\xca\xfe\xba\xbe");
    root.write("src/Order.java", b"public class Order { }\n");
    let files = scanned(&root);
    assert!(
        files.iter().any(|f| f.contains("src/Order.java")),
        "{files:?}"
    );
    assert!(!files.iter().any(|f| f.contains("/out/")), "{files:?}");
}

// A `next export` static-site out/ (marked by _next/) stays out. Go collects no `.html` file
// anyway; the `_next` marker is what prunes `out`.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_next_export_out_is_pruned() {
    let root = TempDir::new();
    std::fs::create_dir_all(root.join("out/_next")).expect("mkdir out/_next");
    root.write("out/index.html", b"<html></html>");
    root.write(
        "pages/index.tsx",
        b"export default function Home() { return null; }\n",
    );
    let files = scanned(&root);
    assert!(
        files.iter().any(|f| f.contains("pages/index.tsx")),
        "{files:?}"
    );
    assert!(
        !files.iter().any(|f| f.contains("out/index.html")),
        "{files:?}"
    );
}

// A TS outDir shape (compiled .js.map one level inside out/) is pruned.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_compiled_artifacts_one_level_down_prune_out() {
    let root = TempDir::new();
    root.write("out/lib/index.js", b"module.exports = {};\n");
    root.write("out/lib/index.js.map", b"{}");
    root.write("src/index.ts", b"export const x = 1;\n");
    let files = scanned(&root);
    assert!(
        files.iter().any(|f| f.contains("src/index.ts")),
        "{files:?}"
    );
    assert!(!files.iter().any(|f| f.contains("out/lib")), "{files:?}");
}

// A wide outDir whose compiled files sit only under a late-sorted subdirectory is still
// recognized as build output. (Go collects no `.txt` file, so `/out/` stays absent either way.)
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_wide_out_dir_markers_beyond_twenty_subdirs_still_prune() {
    let root = TempDir::new();
    for i in 0..30 {
        root.write(&format!("out/mod{i:02}/notes.txt"), b"no artifacts here\n");
    }
    root.write("out/zz-final/index.js.map", b"{}");
    root.write("src/index.ts", b"export const x = 1;\n");
    let files = scanned(&root);
    assert!(
        files.iter().any(|f| f.contains("src/index.ts")),
        "{files:?}"
    );
    assert!(!files.iter().any(|f| f.contains("/out/")), "{files:?}");
}

// The gate is scoped to `out`: build/, dist/, target/ prune by name.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_other_skip_dirs_stay_unconditional() {
    let root = TempDir::new();
    for name in ["build", "dist", "target"] {
        root.write(&format!("{name}/Thing.java"), b"public class Thing { }\n");
    }
    root.write("src/Real.java", b"public class Real { }\n");
    let files = scanned(&root);
    assert!(
        files.iter().any(|f| f.contains("src/Real.java")),
        "{files:?}"
    );
    assert!(
        !files.iter().any(|f| ["build", "dist", "target"]
            .iter()
            .any(|n| f.contains(&format!("/{n}/")))),
        "{files:?}"
    );
}
