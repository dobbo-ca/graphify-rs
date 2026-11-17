//! Upstream graphify v0.9.107 `tests/test_manifest_ingest.py`: 12 of 17 tests, asserting
//! graphify-go v0.12.0 (sources inline, no fixture). Helpers from `manifest.rs`.
//!
//! Upstream's `extract_package_manifest(path)` parses one manifest; Go's pass is
//! `IntrospectManifests` over the manifests of a root, so each case runs `introspect_manifests`
//! on the manifest it wrote. Go's nodes differ in fields upstream reads: `file_type` "package"
//! (upstream: "code" plus `type` "package"), no `version` or `ecosystem`, `source_file` relative
//! to the root (upstream: the path as given), and a stub node per external dependency (upstream
//! emits only the edge). Ids are `make_id("pkg", name)` in both.
//!
//! Not ported (5): `test_manifests_classify_as_code_not_document` and
//! `test_cargo_classifies_as_code_manifest` (`classify_file` and `is_package_manifest_path`:
//! Go has no such function; detect's `CollectManifests` is tested in detect, ihc.14, and Go has
//! no `apm.yml`); `test_apm_parses_name_and_deps`, `test_apm_dependency_collapses_to_single_
//! canonical_node`, `test_external_dependency_edge_pruned_not_orphaned` (`apm.yml`: Go has no
//! apm ecosystem, `manifest.go:17`).

use std::collections::BTreeSet;

use super::manifest_tests::{introspect, write_manifest_file};
use crate::extract::Result;
use crate::idutil::make_id;
use crate::model::Relation;
use crate::testutil::TempDir;

const POM_NS: &str = "http://maven.apache.org/POM/4.0.0";

/// Upstream `_POM_NAMESPACE_FORMS`: (declaration, prefix).
fn pom_namespace_forms() -> [(String, &'static str); 5] {
    [
        (String::new(), ""),
        (format!(" xmlns=\"{POM_NS}\""), ""),
        (format!(" xmlns='{POM_NS}'"), ""),
        (format!(" xmlns = \"{POM_NS}\""), ""),
        (format!(" xmlns:m=\"{POM_NS}\""), "m:"),
    ]
}

/// Upstream `_pom_namespace_fixture`.
fn pom_namespace_fixture(decl: &str, p: &str) -> String {
    format!(
        r#"<{p}project{decl} xmlns:x="urn:other">
      <{p}parent><{p}groupId>com.acme</{p}groupId>
        <{p}artifactId>parent</{p}artifactId><{p}version>1.2</{p}version></{p}parent>
      <{p}artifactId>app</{p}artifactId>
      <{p}properties><{p}core.name>core</{p}core.name></{p}properties>
      <{p}dependencies><{p}dependency>
        <{p}groupId>${{project.groupId}}</{p}groupId>
        <{p}artifactId>${{core.name}}</{p}artifactId>
      </{p}dependency><x:dependency>
        <x:groupId>other</x:groupId><x:artifactId>foreign</x:artifactId>
      </x:dependency></{p}dependencies>
      <{p}profiles><{p}profile><{p}dependencies><{p}dependency>
        <{p}groupId>com.acme</{p}groupId><{p}artifactId>profile</{p}artifactId>
      </{p}dependency></{p}dependencies></{p}profile></{p}profiles>
      <{p}dependencyManagement><{p}dependencies><{p}dependency>
        <{p}groupId>com.acme</{p}groupId><{p}artifactId>managed</{p}artifactId>
      </{p}dependency></{p}dependencies></{p}dependencyManagement>
      <{p}build><{p}plugins><{p}plugin><{p}dependencies><{p}dependency>
        <{p}groupId>com.acme</{p}groupId><{p}artifactId>plugin</{p}artifactId>
      </{p}dependency></{p}dependencies></{p}plugin></{p}plugins></{p}build>
    </{p}project>"#
    )
}

fn pkg(name: &str) -> String {
    make_id(&["pkg", name])
}

/// The targets of the `depends_on` edges.
fn dep_targets(res: &Result) -> BTreeSet<&str> {
    res.edges
        .iter()
        .filter(|e| e.relation == Relation::DEPENDS_ON)
        .map(|e| e.target.as_str())
        .collect()
}

/// The label of the module node of the manifest (Go puts module nodes first).
fn module_label(res: &Result) -> &str {
    res.nodes.first().map_or("", |n| n.label.as_str())
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_pom_namespace_spellings_preserve_identity_properties_and_scopes() {
    for (declaration, prefix) in pom_namespace_forms() {
        let case = format!("{declaration:?} {prefix:?}");
        let root = TempDir::new();
        write_manifest_file(
            &root,
            "pom.xml",
            &pom_namespace_fixture(&declaration, prefix),
        );

        let result = introspect(root.path(), &["pom.xml"]);

        // pins Go v0.12.0 (the module node and a stub per dependency, 6 nodes; no version
        // attribute; source_file relative); upstream 0.9.107 emits 1 node, version "1.2" and the
        // path as given -> P3
        assert_eq!(result.nodes.len(), 6, "{case}: {:?}", result.nodes);
        let package = &result.nodes[0];
        assert_eq!(package.id, pkg("com.acme:app"), "{case}");
        assert_eq!(package.label, "com.acme:app", "{case}");
        assert_eq!(package.source_file, "pom.xml", "{case}");
        // pins Go v0.12.0 (every <dependencies>/<dependency> at any depth and in any namespace:
        // also other:foreign, com.acme:managed, com.acme:plugin); upstream 0.9.107 emits
        // com.acme:core and com.acme:profile only -> P3 (B06)
        let want: BTreeSet<String> = [
            "com.acme:core",
            "other:foreign",
            "com.acme:profile",
            "com.acme:managed",
            "com.acme:plugin",
        ]
        .into_iter()
        .map(pkg)
        .collect();
        let got: BTreeSet<String> = dep_targets(&result)
            .into_iter()
            .map(str::to_string)
            .collect();
        assert_eq!(got, want, "{case}");
        assert!(
            result.edges.iter().all(|e| e.source == package.id),
            "{case}"
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_namespaced_pom_dependencies_survive_extract_build_and_cache() {
    // _POM_NAMESPACE_FORMS[2::2]. Upstream runs extract twice through its cache and builds the
    // graph; Go's manifest pass has no cache, so one run asserts the same node and edge.
    let forms = pom_namespace_forms();
    for (declaration, prefix) in [&forms[2], &forms[4]] {
        let case = format!("{declaration:?} {prefix:?}");
        let root = TempDir::new();
        write_manifest_file(
            &root,
            "app/pom.xml",
            &pom_namespace_fixture(declaration, prefix),
        );
        write_manifest_file(
            &root,
            "core/pom.xml",
            "<project>\n      <groupId>com.acme</groupId><artifactId>core</artifactId>\n    </project>",
        );

        let result = introspect(root.path(), &["app/pom.xml", "core/pom.xml"]);

        let app = pkg("com.acme:app");
        let core = pkg("com.acme:core");
        let node = result.nodes.iter().find(|n| n.id == app);
        assert_eq!(
            node.map(|n| n.label.as_str()),
            Some("com.acme:app"),
            "{case}"
        );
        assert!(
            result
                .edges
                .iter()
                .any(|e| e.source == app && e.target == core && e.relation == Relation::DEPENDS_ON),
            "{case}: {:?}",
            result.edges
        );
    }
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_pyproject_parses_pep508_deps() {
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "pyproject.toml",
        concat!(
            "[project]\nname = \"cool-lib\"\nversion = \"0.1\"\n",
            "dependencies = [\"requests>=2.0\", \"rich[jupyter]==13.0\", \"tomli; python_version<'3.11'\"]\n"
        ),
    );
    let r = introspect(root.path(), &["pyproject.toml"]);
    assert_eq!(module_label(&r), "cool-lib");
    let deps = dep_targets(&r);
    for want in ["requests", "rich", "tomli"] {
        // versions/extras/markers stripped
        assert!(deps.contains(pkg(want).as_str()), "{want}: {deps:?}");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_gomod_parses_module_and_requires() {
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "go.mod",
        concat!(
            "module example.com/me/app\n\ngo 1.22\n\nrequire (\n",
            "\tgithub.com/x/y v1.2.3\n\tgithub.com/a/b v0.4.0\n)\n"
        ),
    );
    let r = introspect(root.path(), &["go.mod"]);
    assert_eq!(module_label(&r), "example.com/me/app");
    let deps = dep_targets(&r);
    assert!(
        deps.contains(pkg("github.com/x/y").as_str())
            && deps.contains(pkg("github.com/a/b").as_str())
    );
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_pom_parses_artifact_and_deps() {
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "pom.xml",
        concat!(
            "<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n",
            "  <groupId>com.acme</groupId>\n  <artifactId>widget</artifactId>\n  <version>2.0</version>\n",
            "  <dependencies>\n    <dependency><groupId>org.lib</groupId><artifactId>core</artifactId></dependency>\n",
            "  </dependencies>\n</project>\n"
        ),
    );
    let r = introspect(root.path(), &["pom.xml"]);
    assert_eq!(module_label(&r), "com.acme:widget");
    assert!(dep_targets(&r).contains(pkg("org.lib:core").as_str()));
}

// pins Go v0.12.0 (dependencyManagement and plugin dependencies count: `parsePom` collects every
// <dependencies>/<dependency>, manifest.go:363); upstream 0.9.107 emits the project and profile
// dependencies only -> P3 (B06)
#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_pom_deps_exclude_managed_and_plugin_dependencies() {
    let dep = |g: &str, a: &str| {
        format!("<dependency><groupId>{g}</groupId><artifactId>{a}</artifactId></dependency>")
    };
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "pom.xml",
        &format!(
            concat!(
                "<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n",
                "  <groupId>com.acme</groupId><artifactId>service</artifactId><version>1.0</version>\n",
                "  <dependencyManagement><dependencies>{}",
                "</dependencies></dependencyManagement>\n",
                "  <dependencies>{}</dependencies>\n",
                "  <build><plugins><plugin><artifactId>maven-surefire-plugin</artifactId>\n",
                "    <dependencies>{}</dependencies>\n",
                "  </plugin></plugins></build>\n",
                "  <profiles><profile><id>pg</id><dependencies>{}",
                "</dependencies></profile></profiles>\n",
                "</project>\n"
            ),
            dep("com.google.guava", "guava"),
            dep("org.slf4j", "slf4j-api"),
            dep("org.junit.platform", "junit-platform-surefire-provider"),
            dep("org.postgresql", "postgresql"),
        ),
    );
    let r = introspect(root.path(), &["pom.xml"]);
    let want: BTreeSet<String> = [
        "com.google.guava:guava",
        "org.slf4j:slf4j-api",
        "org.junit.platform:junit-platform-surefire-provider",
        "org.postgresql:postgresql",
    ]
    .into_iter()
    .map(pkg)
    .collect();
    let got: BTreeSet<String> = dep_targets(&r).into_iter().map(str::to_string).collect();
    assert_eq!(got, want);
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_malformed_manifest_does_not_crash() {
    let root = TempDir::new();
    write_manifest_file(&root, "pom.xml", "<project><not closed");
    let r = introspect(root.path(), &["pom.xml"]); // parse error -> empty, no panic
    assert!(r.nodes.is_empty() && r.edges.is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_parses_name_version_and_deps() {
    // A crate declares its name/version under [package]; deps appear both as a
    // bare version string and as an inline table (version + features).
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "Cargo.toml",
        concat!(
            "[package]\nname = \"my-crate\"\nversion = \"0.3.1\"\nedition = \"2021\"\n\n",
            "[dependencies]\nserde = \"1.0\"\ntokio = { version = \"1\", features = [\"full\"] }\n"
        ),
    );
    let r = introspect(root.path(), &["Cargo.toml"]);
    // pins Go v0.12.0 (no version or ecosystem attribute on a node); upstream 0.9.107 asserts
    // version "0.3.1" and ecosystem "cargo" -> P3
    assert_eq!(module_label(&r), "my-crate");
    let deps = dep_targets(&r);
    // inline-table dep keyed by name
    assert!(deps.contains(pkg("serde").as_str()) && deps.contains(pkg("tokio").as_str()));
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_virtual_workspace_manifest_emits_no_package() {
    // A virtual workspace root has no [package] table, so it declares no package
    // of its own — it must not fabricate a node. Upstream also checks its `skipped` reason,
    // the zero-node warning and its cache entry; Go's pass has none of them.
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"a\", \"b\"]\n",
    );
    let r = introspect(root.path(), &["Cargo.toml"]);
    assert!(r.nodes.is_empty() && r.edges.is_empty());
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_target_conditional_deps_are_collected() {
    // Platform-gated deps under [target.'cfg(...)'.dependencies] are common in
    // real crates and must not be dropped just because they are conditional.
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "Cargo.toml",
        concat!(
            "[package]\nname = \"portable\"\n\n",
            "[dependencies]\nserde = \"1\"\n\n",
            "[target.\"cfg(windows)\".dependencies]\nwinapi = \"0.3\"\n"
        ),
    );
    let r = introspect(root.path(), &["Cargo.toml"]);
    let deps = dep_targets(&r);
    assert!(deps.contains(pkg("serde").as_str()) && deps.contains(pkg("winapi").as_str()));
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_cargo_workspace_inherited_version_does_not_crash() {
    // `version.workspace = true` yields a table, not a string. It must be ignored
    // rather than crash the parse (Go reads no version at all).
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "Cargo.toml",
        "[package]\nname = \"member\"\nversion.workspace = true\n",
    );
    let r = introspect(root.path(), &["Cargo.toml"]);
    assert_eq!(module_label(&r), "member");
}

#[test]
#[ignore = "graphify-rs-ihc.17"]
fn test_pom_inherits_groupid_and_resolves_properties() {
    let root = TempDir::new();
    write_manifest_file(
        &root,
        "rsc/pom.xml",
        concat!(
            "<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n",
            "  <parent><groupId>org.acme</groupId><artifactId>main</artifactId><version>1.0</version></parent>\n",
            "  <groupId>org.acme</groupId>\n  <artifactId>acme-rsc</artifactId>\n</project>\n"
        ),
    );
    write_manifest_file(
        &root,
        "server/pom.xml",
        concat!(
            "<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n",
            "  <parent><groupId>org.acme</groupId><artifactId>main</artifactId><version>1.0</version></parent>\n",
            "  <artifactId>acme-server</artifactId>\n",
            "  <properties><core.suffix>2.12</core.suffix></properties>\n",
            "  <dependencies>\n",
            "    <dependency><groupId>${project.groupId}</groupId><artifactId>acme-rsc</artifactId></dependency>\n",
            "    <dependency><groupId>org.acme</groupId><artifactId>acme-core_${core.suffix}</artifactId></dependency>\n",
            "  </dependencies>\n</project>\n"
        ),
    );
    write_manifest_file(
        &root,
        "web/pom.xml",
        concat!(
            "<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n",
            "  <parent><groupId>org.acme</groupId><artifactId>main</artifactId><version>1.0</version></parent>\n",
            "  <artifactId>acme-web</artifactId>\n",
            "  <dependencies>\n",
            "    <dependency><groupId>org.acme</groupId><artifactId>acme-server</artifactId></dependency>\n",
            "  </dependencies>\n</project>\n"
        ),
    );

    let server = introspect(root.path(), &["server/pom.xml"]);
    // pins Go v0.12.0 (no version attribute); upstream 0.9.107 asserts version "1.0" -> P3
    assert_eq!(module_label(&server), "org.acme:acme-server");
    let targets = dep_targets(&server);
    for want in ["org.acme:acme-rsc", "org.acme:acme-core_2.12"] {
        assert!(targets.contains(pkg(want).as_str()), "{want}: {targets:?}");
    }

    let result = introspect(
        root.path(),
        &["rsc/pom.xml", "server/pom.xml", "web/pom.xml"],
    );
    let has = |a: &str, b: &str| {
        result
            .edges
            .iter()
            .any(|e| e.relation == Relation::DEPENDS_ON && e.source == pkg(a) && e.target == pkg(b))
    };
    assert!(
        has("org.acme:acme-server", "org.acme:acme-rsc"),
        "{:?}",
        result.edges
    );
    assert!(
        has("org.acme:acme-web", "org.acme:acme-server"),
        "{:?}",
        result.edges
    );
}
