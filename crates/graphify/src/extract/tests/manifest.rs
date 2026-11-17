//! graphify-go `internal/extract/manifest_test.go`, and the plan's added test
//! `poetry_deps_sorted_d002` (D-002: Go ranges the Poetry dependency map in random order and
//! keeps the first spelling of a package id; Rust takes the keys sorted). Go's
//! `IntrospectManifests(root)` walks `root` with `detect.CollectManifests`; Rust's
//! `introspect_manifests` takes that list, so each test passes the manifests it wrote.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::introspect_manifests;
use crate::extract::Result;
use crate::idutil::make_id;
use crate::model::{Node, Relation};
use crate::testutil::TempDir;

/// Go `writeManifestFile`.
pub(super) fn write_manifest_file(root: &TempDir, rel: &str, content: &str) {
    root.write(rel, content.as_bytes());
}

/// `introspect_manifests` over `files` (slash-relative, as `detect::collect_manifests` gives).
pub(super) fn introspect(root: &Path, files: &[&str]) -> Result {
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    introspect_manifests(root, &files)
}

// assertPackageGraph runs IntrospectManifests over a root holding a single
// manifest and asserts it produced one canonical package node for module, one
// package node per dependency in deps, and a depends_on edge module->dep for each.
fn assert_package_graph(root: &Path, files: &[&str], module: &str, deps: &[&str]) {
    let res = introspect(root, files);

    let nodes_by_id: BTreeMap<&str, &Node> = res.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let want = 1 + deps.len();
    assert_eq!(
        res.nodes.len(),
        want,
        "nodes = {}, want {want}: {:?}",
        res.nodes.len(),
        res.nodes
    );

    let mod_id = make_id(&["pkg", module]);
    let Some(m) = nodes_by_id.get(mod_id.as_str()) else {
        panic!("missing module node {mod_id:?}; have {:?}", res.nodes);
    };
    assert!(
        m.file_type == "package" && m.label == module,
        "module node = {m:?}, want file_type=package label={module:?}"
    );

    let mut edges = BTreeSet::new();
    for e in &res.edges {
        assert_eq!(
            e.relation,
            Relation::DEPENDS_ON,
            "unexpected relation {:?} on edge {e:?}",
            e.relation.as_str()
        );
        edges.insert(format!("{}->{}", e.source, e.target));
    }
    assert_eq!(
        res.edges.len(),
        deps.len(),
        "edges = {}, want {}: {:?}",
        res.edges.len(),
        deps.len(),
        res.edges
    );
    for dep in deps {
        let dep_id = make_id(&["pkg", dep]);
        let Some(n) = nodes_by_id.get(dep_id.as_str()) else {
            panic!(
                "missing dependency node {dep_id:?} ({dep}); have {:?}",
                res.nodes
            );
        };
        assert!(
            n.file_type == "package" && n.label == *dep,
            "dependency node = {n:?}, want file_type=package label={dep:?}"
        );
        assert!(
            edges.contains(&format!("{mod_id}->{dep_id}")),
            "missing depends_on edge {mod_id}->{dep_id}; have {:?}",
            res.edges
        );
    }
}

#[test]
fn test_introspect_manifests_go_mod() {
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "go.mod",
        concat!(
            "module github.com/example/app\n\ngo 1.21\n\n",
            "require (\n\tgithub.com/pkg/errors v0.9.1\n\tgolang.org/x/text v0.3.0 // indirect\n)\n"
        ),
    );
    assert_package_graph(
        root.path(),
        &["go.mod"],
        "github.com/example/app",
        &["github.com/pkg/errors", "golang.org/x/text"],
    );
}

#[test]
fn test_introspect_manifests_pyproject() {
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "pyproject.toml",
        "[project]\nname = \"myproj\"\nversion = \"1.0.0\"\ndependencies = [\"requests>=2.0\", \"flask\"]\n",
    );
    assert_package_graph(
        root.path(),
        &["pyproject.toml"],
        "myproj",
        &["requests", "flask"],
    );
}

#[test]
fn test_introspect_manifests_pom() {
    let root = TempDir::new();
    // A default xmlns exercises the namespace-agnostic (local-name) matching.
    write_manifest_file(
        &root,
        "pom.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <groupId>com.example</groupId>
  <artifactId>myapp</artifactId>
  <version>1.0</version>
  <dependencies>
    <dependency>
      <groupId>org.apache.commons</groupId>
      <artifactId>commons-lang3</artifactId>
      <version>3.12.0</version>
    </dependency>
    <dependency>
      <groupId>com.google.guava</groupId>
      <artifactId>guava</artifactId>
      <version>31.0</version>
    </dependency>
  </dependencies>
</project>
"#,
    );
    assert_package_graph(
        root.path(),
        &["pom.xml"],
        "com.example:myapp",
        &["org.apache.commons:commons-lang3", "com.google.guava:guava"],
    );
}

#[test]
fn test_introspect_manifests_cargo() {
    let root = TempDir::new();
    // Registry deps in both forms (bare version, inline table) plus a
    // platform-conditional dep; dev-dependencies must not appear.
    write_manifest_file(
        &root,
        "Cargo.toml",
        r#"[package]
name = "mycrate"
version = "0.1.0"

[dependencies]
serde = "1.0"
tokio = { version = "1", features = ["full"] }

[target.'cfg(unix)'.dependencies]
nix = "0.27"

[dev-dependencies]
criterion = "0.5"
"#,
    );
    assert_package_graph(
        root.path(),
        &["Cargo.toml"],
        "mycrate",
        &["serde", "tokio", "nix"],
    );
}

#[test]
fn test_introspect_manifests_cargo_virtual_workspace() {
    let root = TempDir::new();
    write_manifest_file(&root, "Cargo.toml", "[workspace]\nmembers = [\"a\"]\n");
    let res = introspect(root.path(), &["Cargo.toml"]);
    assert!(
        res.nodes.is_empty() && res.edges.is_empty(),
        "virtual workspace root emitted {:?} / {:?}, want nothing",
        res.nodes,
        res.edges
    );
}

#[test]
fn test_introspect_manifests_pom_inherited_group_and_properties() {
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "lib/pom.xml",
        r#"<project>
  <parent><groupId>com.example</groupId><artifactId>parent</artifactId><version>1</version></parent>
  <artifactId>lib</artifactId>
</project>"#,
    );
    write_manifest_file(
        &root,
        "app/pom.xml",
        r#"<project>
  <parent><groupId>com.example</groupId><artifactId>parent</artifactId><version>1</version></parent>
  <artifactId>app</artifactId>
  <properties><lib.group>com.example</lib.group></properties>
  <dependencies>
    <dependency><groupId>${project.groupId}</groupId><artifactId>lib</artifactId></dependency>
    <dependency><groupId>${lib.group}</groupId><artifactId>lib</artifactId></dependency>
    <dependency><groupId>${unknown}</groupId><artifactId>x</artifactId></dependency>
  </dependencies>
</project>"#,
    );
    // detect.CollectManifests lists them in sorted walk order.
    let res = introspect(root.path(), &["app/pom.xml", "lib/pom.xml"]);
    let app = make_id(&["pkg", "com.example:app"]);
    let lib = make_id(&["pkg", "com.example:lib"]);
    let found = res.edges.iter().any(|e| e.source == app && e.target == lib);
    assert!(found, "missing app->lib edge: {:?}", res.edges);
    for n in &res.nodes {
        assert!(
            !n.label.contains("${") || n.label == "${unknown}:x",
            "unresolved placeholder in {:?}",
            n.label
        );
    }
}

// Added (plan Task 16, D-002): Go ranges the [tool.poetry.dependencies] map (`manifest.go:279`)
// and keeps the first name that gives a package id (`manifest.go:88-99`), so of `Foo-Bar` and
// `foo_bar` (one id) Go labels the stub with either; Rust takes the sorted-first key.
#[test]
fn poetry_deps_sorted_d002() {
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "pyproject.toml",
        concat!(
            "[tool.poetry]\nname = \"app\"\n\n",
            "[tool.poetry.dependencies]\npython = \"^3.11\"\nfoo_bar = \"1\"\nFoo-Bar = \"2\"\n"
        ),
    );
    let res = introspect(root.path(), &["pyproject.toml"]);
    let dep_id = make_id(&["pkg", "Foo-Bar"]);
    assert_eq!(dep_id, make_id(&["pkg", "foo_bar"]), "one package id");
    let stubs: Vec<&str> = res
        .nodes
        .iter()
        .filter(|n| n.id == dep_id)
        .map(|n| n.label.as_str())
        .collect();
    assert_eq!(stubs, ["Foo-Bar"], "stub label is the sorted-first key");
    assert_eq!(res.nodes.len(), 2, "app and one stub: {:?}", res.nodes);
    assert_eq!(res.edges.len(), 1, "one depends_on edge: {:?}", res.edges);
}
