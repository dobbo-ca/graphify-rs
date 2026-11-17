//! graphify-go `internal/extract/cargo_test.go`. The helpers are shared with `cargo_rename.rs`
//! and `upstream_cargo_introspect.rs` (one Go package). A root with no `Cargo.toml` makes
//! `introspect_cargo` walk it with `detect::collect_manifests` (`cargo.go:153`), so those tests
//! wait for detect (graphify-rs-ihc.14).

use std::collections::BTreeSet;
use std::path::Path;

use super::introspect_cargo;
use crate::extract::Result;
use crate::model::{Confidence, Edge, Node, Relation, file_of, line};
use crate::testutil::TempDir;

/// Go `writeManifest`: creates `dir` and writes its `Cargo.toml`, the leading newline of
/// `body` trimmed so test literals can stay indented.
pub(super) fn write_manifest(dir: &Path, body: &str) {
    std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("mkdir {}: {e}", dir.display()));
    let body = body.strip_prefix('\n').unwrap_or(body);
    std::fs::write(dir.join("Cargo.toml"), body)
        .unwrap_or_else(|e| panic!("write {}: {e}", dir.display()));
}

/// Go `nodeIDs`.
pub(super) fn node_ids(res: &Result) -> BTreeSet<&str> {
    res.nodes.iter().map(|n| n.id.as_str()).collect()
}

/// Go `hasNode`.
pub(super) fn has_node(res: &Result, want: &Node) -> bool {
    res.nodes.iter().any(|n| n == want)
}

/// Go `hasEdge`.
pub(super) fn has_edge(res: &Result, want: &Edge) -> bool {
    res.edges.iter().any(|e| e == want)
}

/// A crate node as `IntrospectCargo` writes one.
pub(super) fn crate_node(id: &str, label: &str, source_file: &str) -> Node {
    Node {
        id: id.to_string(),
        label: label.to_string(),
        file_type: "code".to_string(),
        source_file: source_file.to_string(),
        source_location: "L1".to_string(),
        ..Node::default()
    }
}

/// A `crate_depends_on` edge as `IntrospectCargo` writes one.
pub(super) fn crate_edge(source: &str, target: &str, manifest: &str) -> Edge {
    Edge {
        source: source.to_string(),
        target: target.to_string(),
        relation: Relation::CRATE_DEPENDS_ON,
        confidence: Confidence::EXTRACTED,
        weight: 1.0,
        source_file: file_of(manifest),
        source_location: line(1),
        ..Edge::default()
    }
}

/// `introspect_cargo`, panicking with `what` on an error.
pub(super) fn introspect(root: &Path, what: &str) -> Result {
    introspect_cargo(root).unwrap_or_else(|e| panic!("{what}: {e}"))
}

#[test]
fn test_introspect_cargo_workspace_internal_dependency_only() {
    let root = TempDir::new();
    write_manifest(
        root.path(),
        r#"
[workspace]
members = ["app", "core"]
"#,
    );
    write_manifest(
        &root.join("app"),
        r#"
[package]
name = "app"
version = "0.1.0"
edition = "2021"

[dependencies]
core = { path = "../core" }
serde = "1"
"#,
    );
    write_manifest(
        &root.join("core"),
        r#"
[package]
name = "core"
version = "0.1.0"
edition = "2021"
"#,
    );

    let res = introspect(root.path(), "IntrospectCargo");

    let ids = node_ids(&res);
    assert!(
        ids.contains("crate:app") && ids.contains("crate:core") && ids.len() == 2,
        "node ids = {ids:?}, want only crate:app and crate:core"
    );
    assert!(
        !ids.contains("crate:serde"),
        "registry dep serde must not be a node"
    );
    assert!(
        has_node(&res, &crate_node("crate:app", "app", "app/Cargo.toml")),
        "missing crate:app node with expected fields"
    );
    let want_edge = crate_edge("crate:app", "crate:core", "app/Cargo.toml");
    assert!(
        has_edge(&res, &want_edge),
        "missing internal edge {want_edge:?}; got {:?}",
        res.edges
    );
    for e in &res.edges {
        assert!(
            e.target != "crate:serde",
            "must not emit an edge to registry dep serde"
        );
    }
}

#[test]
fn test_introspect_cargo_malformed_toml_reports_error() {
    let root = TempDir::new();
    write_manifest(
        root.path(),
        r#"
[package
name = "broken"
"#,
    );

    assert!(
        introspect_cargo(root.path()).is_err(),
        "expected a TOML parse error for a malformed manifest"
    );
}

#[test]
fn test_introspect_cargo_degenerate_manifests() {
    // Empty manifest: no package, no workspace -> nothing.
    let t1 = TempDir::new();
    let empty = t1.join("empty");
    write_manifest(&empty, "");
    let res = introspect(&empty, "empty");
    assert!(
        res.nodes.is_empty() && res.edges.is_empty(),
        "empty manifest produced {} nodes / {} edges, want 0/0",
        res.nodes.len(),
        res.edges.len()
    );

    // Nameless package: a [package] table without a name yields no crate.
    let t2 = TempDir::new();
    let nameless = t2.join("nameless");
    write_manifest(
        &nameless,
        r#"
[package]
version = "0.1.0"
"#,
    );
    let res = introspect(&nameless, "nameless");
    assert!(
        res.nodes.is_empty() && res.edges.is_empty(),
        "nameless package produced {} nodes / {} edges, want 0/0",
        res.nodes.len(),
        res.edges.len()
    );

    // Scalar dependencies: a non-table dependencies value is ignored, the node stays.
    let t3 = TempDir::new();
    let scalar = t3.join("scalar");
    write_manifest(
        &scalar,
        r#"
[package]
name = "app"
version = "0.1.0"

dependencies = "not-a-table"
"#,
    );
    let res = introspect(&scalar, "scalar");
    assert!(
        res.nodes.len() == 1 && has_node(&res, &crate_node("crate:app", "app", "Cargo.toml")),
        "scalar deps: nodes = {:?}, want single crate:app",
        res.nodes
    );
    assert!(
        res.edges.is_empty(),
        "scalar deps produced {} edges, want 0",
        res.edges.len()
    );
}

#[test]
fn test_introspect_cargo_virtual_and_root_package_workspaces() {
    // Virtual root: a glob member pattern + workspace.dependencies, deps via { workspace = true }.
    let virtual_root = TempDir::new();
    write_manifest(
        virtual_root.path(),
        r#"
[workspace]
members = ["crates/*"]

[workspace.dependencies]
beta = { path = "crates/beta" }
serde = "1"
"#,
    );
    write_manifest(
        &virtual_root.join("crates/alpha"),
        r#"
[package]
name = "alpha"
version = "0.1.0"
edition = "2021"

[dependencies]
beta = { workspace = true }
serde = { workspace = true }
"#,
    );
    write_manifest(
        &virtual_root.join("crates/beta"),
        r#"
[package]
name = "beta"
version = "0.1.0"
edition = "2021"
"#,
    );
    let res = introspect(virtual_root.path(), "virtual");
    let ids = node_ids(&res);
    assert!(
        ids.contains("crate:alpha") && ids.contains("crate:beta") && ids.len() == 2,
        "virtual node ids = {ids:?}, want crate:alpha + crate:beta"
    );
    assert_eq!(
        res.edges.len(),
        1,
        "virtual edges = {}, want 1",
        res.edges.len()
    );
    assert!(
        has_edge(
            &res,
            &crate_edge("crate:alpha", "crate:beta", "crates/alpha/Cargo.toml")
        ),
        "missing alpha->beta edge; got {:?}",
        res.edges
    );

    // Root-package workspace: the root manifest is itself a package member.
    let pkg_root = TempDir::new();
    write_manifest(
        pkg_root.path(),
        r#"
[package]
name = "root_pkg"
version = "0.1.0"
edition = "2021"

[workspace]
members = ["crates/*"]
"#,
    );
    write_manifest(
        &pkg_root.join("crates/member"),
        r#"
[package]
name = "member"
version = "0.1.0"
edition = "2021"

[dependencies]
root_pkg = { path = "../.." }
"#,
    );
    let res = introspect(pkg_root.path(), "pkgRoot");
    let ids = node_ids(&res);
    assert!(
        ids.contains("crate:root_pkg") && ids.contains("crate:member") && ids.len() == 2,
        "pkgRoot node ids = {ids:?}, want crate:root_pkg + crate:member"
    );
    assert!(
        has_edge(
            &res,
            &crate_edge("crate:member", "crate:root_pkg", "crates/member/Cargo.toml")
        ),
        "missing member->root_pkg edge; got {:?}",
        res.edges
    );
}

#[test]
fn test_introspect_cargo_large_workspace_chain() {
    const CRATE_COUNT: usize = 200;
    let root = TempDir::new();
    write_manifest(
        root.path(),
        r#"
[workspace]
members = ["crates/*"]
"#,
    );
    for i in 0..CRATE_COUNT {
        let dep = if i + 1 < CRATE_COUNT {
            format!(
                "\n[dependencies]\ncrate_{:03} = {{ path = \"../crate_{:03}\" }}\n",
                i + 1,
                i + 1
            )
        } else {
            String::new()
        };
        write_manifest(
            &root.join(&format!("crates/crate_{i:03}")),
            &format!(
                r#"
[package]
name = "crate_{i:03}"
version = "0.1.0"
edition = "2021"
{dep}"#
            ),
        );
    }

    let res = introspect(root.path(), "IntrospectCargo");
    assert_eq!(
        res.nodes.len(),
        CRATE_COUNT,
        "nodes = {}, want {CRATE_COUNT}",
        res.nodes.len()
    );
    assert_eq!(
        res.edges.len(),
        CRATE_COUNT - 1,
        "edges = {}, want {}",
        res.edges.len(),
        CRATE_COUNT - 1
    );
    assert!(
        has_edge(
            &res,
            &crate_edge(
                "crate:crate_000",
                "crate:crate_001",
                "crates/crate_000/Cargo.toml"
            )
        ),
        "missing head-of-chain edge crate_000->crate_001"
    );
    assert!(
        has_edge(
            &res,
            &crate_edge(
                "crate:crate_198",
                "crate:crate_199",
                "crates/crate_198/Cargo.toml"
            )
        ),
        "missing tail-of-chain edge crate_198->crate_199"
    );
}

// A corpus with no Cargo.toml at the root yields a clean error, so the build
// command can surface it rather than silently producing nothing.
// nogrammar test (ihc.17): needs detect (no root Cargo.toml: `shallowestCargo` walks the root)
#[test]
fn test_introspect_cargo_no_root_manifest() {
    let root = TempDir::new();
    assert!(
        introspect_cargo(root.path()).is_err(),
        "expected an error when root has no Cargo.toml"
    );
}

// Go skips the test when symlinks are unavailable; Rust makes them on unix only.
#[cfg(unix)]
#[test]
fn test_introspect_cargo_inherited_path_symlink_and_absolute() {
    let root = TempDir::new();
    let abs = root.join("crates/abs");
    let abs_q = gostd::strconv::quote(abs.to_str().expect("utf-8 temp path").as_bytes());
    write_manifest(
        root.path(),
        &format!(
            r#"
[workspace]
members = ["crates/*"]

[workspace.dependencies]
sym = {{ path = "link/sym" }}
abs = {{ path = {abs_q} }}
"#
        ),
    );
    for n in ["app", "sym", "abs"] {
        let mut body = format!("[package]\nname = \"{n}\"\nversion = \"0.1.0\"\n");
        if n == "app" {
            body += "\n[dependencies]\nsym = { workspace = true }\nabs = { workspace = true }\n";
        }
        write_manifest(&root.join(&format!("crates/{n}")), &body);
    }
    std::os::unix::fs::symlink(root.join("crates"), root.join("link"))
        .unwrap_or_else(|e| panic!("symlink: {e}"));
    let res = introspect(root.path(), "IntrospectCargo");
    for tgt in ["crate:sym", "crate:abs"] {
        assert!(
            has_edge(&res, &crate_edge("crate:app", tgt, "crates/app/Cargo.toml")),
            "missing app->{tgt} edge; got {:?}",
            res.edges
        );
    }
}

// nogrammar test (ihc.17): needs detect (no root Cargo.toml: `shallowestCargo` walks the root)
#[test]
fn test_introspect_cargo_subdir() {
    let root = TempDir::new();
    root.write(
        "rust/Cargo.toml",
        b"[workspace]\nmembers = [\"app\", \"core\"]\n",
    );
    root.write(
        "rust/app/Cargo.toml",
        b"[package]\nname = \"app\"\n\n[dependencies]\ncore = { path = \"../core\" }\n",
    );
    root.write("rust/core/Cargo.toml", b"[package]\nname = \"core\"\n");
    let res = introspect(root.path(), "IntrospectCargo");
    assert!(
        res.nodes.len() == 2 && res.edges.len() == 1,
        "nodes={} edges={}, want 2/1",
        res.nodes.len(),
        res.edges.len()
    );
    let got = &res.nodes[0].source_file;
    assert_eq!(got, "rust/app/Cargo.toml", "SourceFile = {got:?}");
}

// nogrammar test (ihc.17): needs detect (no root Cargo.toml: `shallowestCargo` walks the root)
#[test]
fn test_introspect_cargo_subdir_inherited_dep() {
    let root = TempDir::new();
    root.write(
        "rust/Cargo.toml",
        b"[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.dependencies]\ncore = { path = \"crates/core\" }\n",
    );
    root.write(
        "rust/crates/app/Cargo.toml",
        b"[package]\nname = \"app\"\n\n[dependencies]\ncore = { workspace = true }\n",
    );
    root.write(
        "rust/crates/core/Cargo.toml",
        b"[package]\nname = \"core\"\n",
    );
    let res = introspect(root.path(), "IntrospectCargo");
    assert_eq!(res.edges.len(), 1, "edges={}, want 1", res.edges.len());
}
