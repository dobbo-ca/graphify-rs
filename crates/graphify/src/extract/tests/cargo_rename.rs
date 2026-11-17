//! graphify-go `internal/extract/cargo_rename_test.go`. Helpers from `cargo.rs`.
//! `TestIntrospectCargoInheritedRelativeRoot` runs Go's `t.Chdir(root)` then
//! `IntrospectCargo(".")`; the Rust tests share one process cwd, so it passes `root` as a path
//! relative to the cwd instead (the same relative-root case, without the chdir).

use std::path::{Path, PathBuf};

use super::cargo_tests::{crate_edge, has_edge, introspect, write_manifest};
use crate::testutil::TempDir;

// A dependency renamed via `package = "..."` must bind to the workspace crate of
// that real name, not the dep-table key. Before the fix the edge was dropped
// because the lookup used the key "db" (#1858).
#[test]
fn test_introspect_cargo_honors_package_rename() {
    let root = TempDir::new();
    write_manifest(
        root.path(),
        r#"
[workspace]
members = ["app", "storage"]
"#,
    );
    write_manifest(
        &root.join("app"),
        r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
db = { path = "../storage", package = "internal-storage" }
"#,
    );
    write_manifest(
        &root.join("storage"),
        r#"
[package]
name = "internal-storage"
version = "0.1.0"
"#,
    );

    let res = introspect(root.path(), "IntrospectCargo");
    let want_edge = crate_edge("crate:app", "crate:internal-storage", "app/Cargo.toml");
    assert!(
        has_edge(&res, &want_edge),
        "rename not honored: missing edge {want_edge:?}; got {:?}",
        res.edges
    );
    // Exactly one edge — no spurious edge to a phantom crate:db (the raw dep key).
    assert_eq!(
        res.edges.len(),
        1,
        "want exactly 1 edge, got {}: {:?}",
        res.edges.len(),
        res.edges
    );
}

// A rename pointing at a registry/external crate (no workspace member of that
// name) must stay a no-op — no edge (#1858).
#[test]
fn test_introspect_cargo_rename_to_external_is_no_edge() {
    let root = TempDir::new();
    write_manifest(
        root.path(),
        r#"
[workspace]
members = ["app"]
"#,
    );
    write_manifest(
        &root.join("app"),
        r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
tokio_rt = { version = "1", package = "tokio" }
"#,
    );
    let res = introspect(root.path(), "IntrospectCargo");
    assert!(
        res.edges.is_empty(),
        "rename to a non-workspace crate must yield no edge, got {:?}",
        res.edges
    );
}

// A member inheriting `db = { workspace = true }` takes its package rename from
// the root [workspace.dependencies] table.
#[test]
fn test_introspect_cargo_inherited_rename() {
    let root = TempDir::new();
    write_manifest(
        root.path(),
        r#"
[workspace]
members = ["app", "crates/real"]

[workspace.dependencies]
db = { path = "crates/real", package = "real" }
"#,
    );
    write_manifest(
        &root.join("app"),
        r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
db = { workspace = true }
"#,
    );
    write_manifest(
        &root.join("crates/real"),
        r#"
[package]
name = "real"
version = "0.1.0"
"#,
    );
    let res = introspect(root.path(), "IntrospectCargo");
    let want = crate_edge("crate:app", "crate:real", "app/Cargo.toml");
    assert!(
        has_edge(&res, &want) && res.edges.len() == 1,
        "want only {want:?}, got {:?}",
        res.edges
    );
}

// An inherited registry dep sharing a workspace crate's name is not internal.
#[test]
fn test_introspect_cargo_inherited_registry_name_is_no_edge() {
    let root = TempDir::new();
    write_manifest(
        root.path(),
        r#"
[workspace]
members = ["app", "util"]

[workspace.dependencies]
util = "1"
"#,
    );
    write_manifest(
        &root.join("app"),
        r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
util = { workspace = true }
"#,
    );
    write_manifest(
        &root.join("util"),
        r#"
[package]
name = "util"
version = "0.1.0"
"#,
    );
    let res = introspect(root.path(), "IntrospectCargo");
    assert!(
        res.edges.is_empty(),
        "registry dep must yield no edge, got {:?}",
        res.edges
    );
}

/// `p` (absolute) as a path relative to the current directory: `..` up to `/`, then `p`.
fn relative_to_cwd(p: &Path) -> PathBuf {
    let cwd = std::env::current_dir().expect("cwd");
    let mut rel = PathBuf::new();
    for _ in cwd.components().skip(1) {
        rel.push("..");
    }
    rel.join(p.strip_prefix("/").expect("absolute temp path"))
}

// Relative roots (`build .`) must compare paths like absolute ones.
#[cfg(unix)]
#[test]
fn test_introspect_cargo_inherited_relative_root() {
    let base = TempDir::new();
    let root = base.join("E");
    let dd = gostd::filepath::join(&[root.to_str().expect("utf-8 temp path"), "crates", "dd"]);
    write_manifest(
        &root,
        &format!(
            r#"
[workspace]
members = ["app", "crates/dd"]

[workspace.dependencies]
dd = {{ path = "../E/crates/dd" }}
abs = {{ path = "{dd}", package = "dd" }}
"#
        ),
    );
    write_manifest(
        &root.join("app"),
        r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
dd = { workspace = true }
abs = { workspace = true }
"#,
    );
    write_manifest(
        &root.join("crates/dd"),
        r#"
[package]
name = "dd"
version = "0.1.0"
"#,
    );
    // Go: t.Chdir(root); IntrospectCargo(".")
    let res = introspect(&relative_to_cwd(&root), "IntrospectCargo");
    assert_eq!(
        res.edges.len(),
        2,
        "want 2 edges (dd, abs), got {:?}",
        res.edges
    );
}
