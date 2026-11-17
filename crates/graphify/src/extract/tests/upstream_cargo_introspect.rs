//! Upstream graphify v0.9.107 `tests/test_cargo_introspect.py`: all 16 tests, asserting
//! graphify-go v0.12.0 (sources inline, no fixture). Helpers from `cargo.rs`.
//!
//! Every node and edge is Go's: a crate node has `file_type` "code" (upstream's has none) and a
//! `crate_depends_on` edge has no `context` (upstream's is "cargo_dependency"), so each "pins
//! Go v0.12.0" for those fields (no P3 bead). Two cases assert Go's value where upstream differs:
//! `test_cargo_introspect_discovery_does_not_search_past_the_depth_cap` (Go has no depth cap)
//! and the `""` case of `test_cargo_inherited_dependency_can_target_root_package` (Go reads an
//! empty `path` as a registry dep). The four tests with no root `Cargo.toml` need detect
//! (`shallowestCargo`, `cargo.go:153`): ignored on graphify-rs-ihc.14.

use std::collections::BTreeSet;

use super::cargo_tests::{crate_edge, crate_node, has_edge, has_node, introspect, node_ids};
use super::introspect_cargo;
use crate::extract::Result;
use crate::testutil::TempDir;

/// Upstream `_write_manifest`: `content.lstrip()` written to `path`.
fn write(root: &TempDir, rel: &str, content: &str) {
    root.write(rel, content.trim_start_matches(['\n', ' ']).as_bytes());
}

/// The `(source, target)` pairs of the edges, in order.
fn pairs(res: &Result) -> Vec<(&str, &str)> {
    res.edges
        .iter()
        .map(|e| (e.source.as_str(), e.target.as_str()))
        .collect()
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_introspect_workspace_internal_dependency_only() {
    let root = TempDir::new();
    write(
        &root,
        "Cargo.toml",
        "\n[workspace]\nmembers = [\"app\", \"core\"]\n",
    );
    write(
        &root,
        "app/Cargo.toml",
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
    write(
        &root,
        "core/Cargo.toml",
        r#"
[package]
name = "core"
version = "0.1.0"
edition = "2021"
"#,
    );

    let res = introspect(root.path(), "introspect_cargo");

    assert_eq!(node_ids(&res), BTreeSet::from(["crate:app", "crate:core"]));
    // pins Go v0.12.0 (file_type "code"); upstream 0.9.107 emits no file_type -> P3
    assert!(has_node(
        &res,
        &crate_node("crate:app", "app", "app/Cargo.toml")
    ));
    // pins Go v0.12.0 (no edge context); upstream 0.9.107 emits context "cargo_dependency" -> P3
    assert!(has_edge(
        &res,
        &crate_edge("crate:app", "crate:core", "app/Cargo.toml")
    ));
    assert!(!pairs(&res).contains(&("crate:app", "crate:serde")));
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_introspect_malformed_toml_reports_parser_error() {
    let root = TempDir::new();
    write(&root, "Cargo.toml", "\n[package\nname = \"broken\"\n");
    // Upstream checks the class name `TOMLDecodeError`; Go returns the BurntSushi ParseError.
    let err = introspect_cargo(root.path()).expect_err("a TOML parse error");
    assert!(err.to_string().starts_with("toml: "), "got {err}");
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_introspect_degenerate_manifests_return_empty_or_skip_bad_deps() {
    let root = TempDir::new();
    write(&root, "empty/Cargo.toml", "");
    let empty = introspect(&root.join("empty"), "empty");
    assert!(empty.nodes.is_empty());
    assert!(empty.edges.is_empty());

    write(
        &root,
        "nameless/Cargo.toml",
        "\n[package]\nversion = \"0.1.0\"\n",
    );
    let nameless = introspect(&root.join("nameless"), "nameless");
    assert!(nameless.nodes.is_empty());
    assert!(nameless.edges.is_empty());

    write(
        &root,
        "scalar-dependencies/Cargo.toml",
        r#"
[package]
name = "app"
version = "0.1.0"

dependencies = "not-a-table"
"#,
    );
    let scalar = introspect(&root.join("scalar-dependencies"), "scalar");
    // pins Go v0.12.0 (file_type "code"); upstream 0.9.107 emits no file_type -> P3
    assert_eq!(scalar.nodes, [crate_node("crate:app", "app", "Cargo.toml")]);
    assert!(scalar.edges.is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_introspect_old_manifest_keeps_internal_path_dep_and_skips_external() {
    let root = TempDir::new();
    write(
        &root,
        "Cargo.toml",
        "\n[workspace]\nmembers = [\"legacy\", \"internal\"]\n",
    );
    write(
        &root,
        "legacy/Cargo.toml",
        r#"
[package]
name = "legacy"
version = "0.1.0"

[dependencies]
rand = "0.8"
internal = { path = "../internal" }
"#,
    );
    write(
        &root,
        "internal/Cargo.toml",
        "\n[package]\nname = \"internal\"\nversion = \"0.1.0\"\n",
    );

    let res = introspect(root.path(), "introspect_cargo");

    assert_eq!(
        node_ids(&res),
        BTreeSet::from(["crate:legacy", "crate:internal"])
    );
    assert_eq!(res.edges.len(), 1);
    assert!(pairs(&res).contains(&("crate:legacy", "crate:internal")));
    assert!(!pairs(&res).contains(&("crate:legacy", "crate:rand")));
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_introspect_modern_virtual_and_root_package_workspaces() {
    let root = TempDir::new();
    write(
        &root,
        "virtual/Cargo.toml",
        r#"
[workspace]
members = ["crates/*"]

[workspace.dependencies]
beta = { path = "crates/beta" }
serde = "1"
"#,
    );
    write(
        &root,
        "virtual/crates/alpha/Cargo.toml",
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
    write(
        &root,
        "virtual/crates/beta/Cargo.toml",
        "\n[package]\nname = \"beta\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    let virtual_result = introspect(&root.join("virtual"), "virtual");
    assert_eq!(
        node_ids(&virtual_result),
        BTreeSet::from(["crate:alpha", "crate:beta"])
    );
    assert_eq!(virtual_result.nodes.len(), 2);
    assert_eq!(virtual_result.edges.len(), 1);
    // pins Go v0.12.0 (no edge context); upstream 0.9.107 emits context "cargo_dependency" -> P3
    assert!(has_edge(
        &virtual_result,
        &crate_edge("crate:alpha", "crate:beta", "crates/alpha/Cargo.toml")
    ));

    write(
        &root,
        "package-root/Cargo.toml",
        r#"
[package]
name = "root_pkg"
version = "0.1.0"
edition = "2021"

[workspace]
members = ["crates/*"]
"#,
    );
    write(
        &root,
        "package-root/crates/member/Cargo.toml",
        r#"
[package]
name = "member"
version = "0.1.0"
edition = "2021"

[dependencies]
root_pkg = { path = "../.." }
"#,
    );

    let package_result = introspect(&root.join("package-root"), "package-root");
    assert_eq!(
        node_ids(&package_result),
        BTreeSet::from(["crate:root_pkg", "crate:member"])
    );
    assert_eq!(package_result.nodes.len(), 2);
    assert_eq!(package_result.edges.len(), 1);
    // pins Go v0.12.0 (no edge context); upstream 0.9.107 emits context "cargo_dependency" -> P3
    assert!(has_edge(
        &package_result,
        &crate_edge("crate:member", "crate:root_pkg", "crates/member/Cargo.toml")
    ));
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_introspect_large_workspace_dependency_chain() {
    const CRATE_COUNT: usize = 200;
    let root = TempDir::new();
    write(
        &root,
        "Cargo.toml",
        "\n[workspace]\nmembers = [\"crates/*\"]\n",
    );
    for index in 0..CRATE_COUNT {
        let dependency_block = if index + 1 < CRATE_COUNT {
            format!(
                "\n\n[dependencies]\ncrate_{:03} = {{ path = \"../crate_{:03}\" }}\n",
                index + 1,
                index + 1
            )
        } else {
            String::new()
        };
        write(
            &root,
            &format!("crates/crate_{index:03}/Cargo.toml"),
            &format!(
                "\n[package]\nname = \"crate_{index:03}\"\nversion = \"0.1.0\"\nedition = \"2021\"{dependency_block}\n"
            ),
        );
    }

    let res = introspect(root.path(), "introspect_cargo");

    assert_eq!(res.nodes.len(), CRATE_COUNT);
    assert_eq!(res.edges.len(), CRATE_COUNT - 1);
    let want: BTreeSet<String> = (0..CRATE_COUNT)
        .map(|index| format!("crate:crate_{index:03}"))
        .collect();
    let got: BTreeSet<String> = res.nodes.iter().map(|n| n.id.clone()).collect();
    assert_eq!(got, want);
    // pins Go v0.12.0 (no edge context); upstream 0.9.107 emits context "cargo_dependency" -> P3
    assert!(has_edge(
        &res,
        &crate_edge(
            "crate:crate_000",
            "crate:crate_001",
            "crates/crate_000/Cargo.toml"
        )
    ));
    assert!(has_edge(
        &res,
        &crate_edge(
            "crate:crate_198",
            "crate:crate_199",
            "crates/crate_198/Cargo.toml"
        )
    ));
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_introspect_honors_package_rename_on_internal_dep() {
    let root = TempDir::new();
    write(
        &root,
        "Cargo.toml",
        "\n[workspace]\nmembers = [\"app\", \"storage\"]\n",
    );
    write(
        &root,
        "app/Cargo.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
edition = "2021"

[dependencies]
db = { path = "../storage", package = "internal-storage" }
"#,
    );
    write(
        &root,
        "storage/Cargo.toml",
        "\n[package]\nname = \"internal-storage\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    let res = introspect(root.path(), "introspect_cargo");

    assert_eq!(
        node_ids(&res),
        BTreeSet::from(["crate:app", "crate:internal-storage"])
    );
    // pins Go v0.12.0 (no edge context); upstream 0.9.107 emits context "cargo_dependency" -> P3
    assert!(has_edge(
        &res,
        &crate_edge("crate:app", "crate:internal-storage", "app/Cargo.toml")
    ));
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_introspect_package_rename_falls_through_when_unresolved() {
    let root = TempDir::new();
    write(&root, "Cargo.toml", "\n[workspace]\nmembers = [\"app\"]\n");
    write(
        &root,
        "app/Cargo.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio_rt = { version = "1", package = "tokio" }
"#,
    );

    let res = introspect(root.path(), "introspect_cargo");

    assert_eq!(node_ids(&res), BTreeSet::from(["crate:app"]));
    assert!(res.edges.is_empty());
}

// nogrammar test (ihc.17): needs detect (no root Cargo.toml: `shallowestCargo` walks the root)
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_cargo_introspect_discovers_a_manifest_in_a_subdirectory() {
    let root = TempDir::new();
    write(
        &root,
        "src-tauri/Cargo.toml",
        "\n[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    let res = introspect(root.path(), "introspect_cargo");

    assert_eq!(node_ids(&res), BTreeSet::from(["crate:app"]));
    let node = res.nodes.iter().find(|n| n.id == "crate:app").unwrap();
    assert_eq!(
        node.source_file, "src-tauri/Cargo.toml",
        "the discovered manifest's source_file should stay relative to the scan root, not the crate subdirectory; got {:?}",
        node.source_file
    );
}

// nogrammar test (ihc.17): needs detect (no root Cargo.toml: `shallowestCargo` walks the root)
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_cargo_introspect_discovery_resolves_workspace_members_from_the_manifest() {
    let root = TempDir::new();
    write(
        &root,
        "src-tauri/Cargo.toml",
        "\n[workspace]\nmembers = [\"core\"]\n",
    );
    write(
        &root,
        "src-tauri/core/Cargo.toml",
        "\n[package]\nname = \"core\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    let res = introspect(root.path(), "introspect_cargo");

    assert_eq!(node_ids(&res), BTreeSet::from(["crate:core"]));
    let node = res.nodes.iter().find(|n| n.id == "crate:core").unwrap();
    assert_eq!(node.source_file, "src-tauri/core/Cargo.toml");
}

// nogrammar test (ihc.17): needs detect (no root Cargo.toml; node_modules is a skip dir)
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_cargo_introspect_discovery_skips_noise_directories() {
    let root = TempDir::new();
    write(
        &root,
        "node_modules/some-pkg/Cargo.toml",
        "\n[package]\nname = \"decoy\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        &root,
        "src-tauri/Cargo.toml",
        "\n[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    let res = introspect(root.path(), "introspect_cargo");

    assert_eq!(
        node_ids(&res),
        BTreeSet::from(["crate:app"]),
        "discovery must never search into node_modules/"
    );
}

// pins Go v0.12.0 (crate:too_deep: `shallowestCargo` takes the shallowest Cargo.toml at any
// depth); upstream 0.9.107 raises FileNotFoundError past its depth cap -> P3
// nogrammar test (ihc.17): needs detect (no root Cargo.toml: `shallowestCargo` walks the root)
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn test_cargo_introspect_discovery_does_not_search_past_the_depth_cap() {
    let root = TempDir::new();
    write(
        &root,
        "a/b/c/d/Cargo.toml",
        "\n[package]\nname = \"too_deep\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    let res = introspect(root.path(), "introspect_cargo");

    assert_eq!(node_ids(&res), BTreeSet::from(["crate:too_deep"]));
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_inherited_rename_uses_workspace_dependency_identity() {
    // ids=["virtual", "root-package"]
    for (case, root_package) in [("virtual", false), ("root-package", true)] {
        let root = TempDir::new();
        let root_header = if root_package {
            "[package]\nname = \"root-app\"\nversion = \"0.1.0\"\n"
        } else {
            ""
        };
        let root_dependency = if root_package {
            "[dependencies]\ndb.workspace = true\n"
        } else {
            ""
        };
        write(
            &root,
            "Cargo.toml",
            &format!(
                "{root_header}[workspace]\nmembers = [\"app\", \"storage\"]\n[workspace.dependencies]\ndb = {{ path = \"storage\", package = \"internal-storage\" }}\n{root_dependency}"
            ),
        );
        write(
            &root,
            "app/Cargo.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n[dependencies]\ndb = { workspace = true, features = [\"fast\"] }\n",
        );
        write(
            &root,
            "storage/Cargo.toml",
            "[package]\nname = \"internal-storage\"\nversion = \"0.1.0\"\n[features]\nfast = []\n",
        );

        let result = introspect(root.path(), case);
        let mut expected_sources = BTreeSet::from(["crate:app"]);
        if root_package {
            expected_sources.insert("crate:root-app");
        }
        let mut want_nodes = expected_sources.clone();
        want_nodes.insert("crate:internal-storage");
        assert_eq!(node_ids(&result), want_nodes, "{case}");
        assert_eq!(result.edges.len(), expected_sources.len(), "{case}");
        let got: BTreeSet<(&str, &str)> = pairs(&result).into_iter().collect();
        let want: BTreeSet<(&str, &str)> = expected_sources
            .iter()
            .map(|s| (*s, "crate:internal-storage"))
            .collect();
        assert_eq!(got, want, "{case}");
        for edge in &result.edges {
            let want_file = if edge.source == "crate:root-app" {
                "Cargo.toml"
            } else {
                "app/Cargo.toml"
            };
            assert_eq!(edge.source_file.as_str(), want_file, "{case}");
            assert_eq!(edge.relation.as_str(), "crate_depends_on", "{case}");
            // pins Go v0.12.0 (no edge context); upstream 0.9.107 checks context
            // "cargo_dependency" -> P3
            assert_eq!(edge.confidence.as_str(), "EXTRACTED", "{case}");
        }
        assert_eq!(
            introspect(root.path(), case),
            result,
            "{case}: deterministic"
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_inherited_dependency_requires_matching_local_path() {
    const CASES: [(&str, &str, bool); 12] = [
        (r#"db = { path = "db" }"#, "db.workspace = true", true),
        (r#"db = "1""#, "db.workspace = true", false),
        (r#"db = { version = "1" }"#, "db.workspace = true", false),
        (
            r#"db = { git = "https://example.com/db" }"#,
            "db.workspace = true",
            false,
        ),
        (r#"db = { path = "missing" }"#, "db.workspace = true", false),
        (r#"db = { path = "app" }"#, "db.workspace = true", false),
        ("db = { path = 42 }", "db.workspace = true", false),
        ("db = false", "db.workspace = true", false),
        ("", "db.workspace = true", false),
        (r#"db = { path = "db" }"#, "db.workspace = false", false),
        (r#"db = { path = "db" }"#, r#"db.workspace = "true""#, false),
        (
            r#"db = { path = "missing" }"#,
            r#"db = { path = "../db" }"#,
            true,
        ),
    ];
    for (root_declaration, member_declaration, expected) in CASES {
        let case = format!("{root_declaration:?} / {member_declaration:?}");
        let root = TempDir::new();
        write(
            &root,
            "Cargo.toml",
            &format!(
                "[workspace]\nmembers = [\"app\", \"db\"]\n[workspace.dependencies]\n{root_declaration}\n"
            ),
        );
        for name in ["app", "db"] {
            let deps = if name == "app" {
                format!("[dependencies]\n{member_declaration}\n")
            } else {
                String::new()
            };
            write(
                &root,
                &format!("{name}/Cargo.toml"),
                &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n{deps}"),
            );
        }

        let result = introspect(root.path(), &case);
        assert_eq!(
            node_ids(&result),
            BTreeSet::from(["crate:app", "crate:db"]),
            "{case}"
        );
        let want: Vec<(&str, &str)> = if expected {
            vec![("crate:app", "crate:db")]
        } else {
            vec![]
        };
        assert_eq!(pairs(&result), want, "{case}");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_inherited_dependency_can_target_root_package() {
    // pins Go v0.12.0 for `""` (no edge: `cargo.go:117` reads an empty path as a registry
    // dep); upstream 0.9.107 emits crate:app -> crate:db -> P3
    const CASES: [(&str, bool); 2] = [(".", true), ("", false)];
    for (root_path, edge) in CASES {
        let root = TempDir::new();
        write(
            &root,
            "Cargo.toml",
            &format!(
                "[package]\nname = \"db\"\nversion = \"0.1.0\"\n[workspace]\nmembers = [\"app\"]\n[workspace.dependencies]\ndb = {{ path = \"{root_path}\" }}\n"
            ),
        );
        write(
            &root,
            "app/Cargo.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n[dependencies]\ndb.workspace = true\n",
        );
        let result = introspect(root.path(), root_path);
        let want: Vec<(&str, &str)> = if edge {
            vec![("crate:app", "crate:db")]
        } else {
            vec![]
        };
        assert_eq!(pairs(&result), want, "root_path {root_path:?}");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_missing_or_malformed_workspace_dependencies_are_ignored() {
    const CASES: [&str; 4] = [
        "",
        "workspace = false\n",
        "[workspace]\n",
        "[workspace]\ndependencies = false\n",
    ];
    for workspace in CASES {
        let root = TempDir::new();
        write(
            &root,
            "Cargo.toml",
            &format!(
                "{workspace}[package]\nname = \"db\"\nversion = \"0.1.0\"\n[dependencies]\ndb.workspace = true\n"
            ),
        );
        assert!(
            introspect(root.path(), workspace).edges.is_empty(),
            "workspace {workspace:?}"
        );
    }
}
