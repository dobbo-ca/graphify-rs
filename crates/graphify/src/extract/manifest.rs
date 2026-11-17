//! Port of `manifest.go`: the package-manifest pass (`pyproject.toml`, `Cargo.toml`, `go.mod`,
//! `pom.xml`). Owner: the extractors-and-CLI stream; filled by graphify-rs-ihc.17.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use gostd::regexp::Regexp;
use gostd::strings::trim_space;
use gostd::toml::{self, Table, Value};
use gostd::xml::Element;

use super::extract::Result;
use crate::idutil::make_id;
use crate::model::{Confidence, Edge, Node, Relation, file_of, line};

/// Go `manifestInfo`: one parsed manifest.
struct ManifestInfo {
    pkg_id: String,
    name: String,
    rel: String,
    deps: Vec<String>,
}

/// Go `manifestEcosystems`.
fn manifest_ecosystem(base: &str) -> Option<&'static str> {
    Some(match base {
        "pyproject.toml" => "python",
        "Cargo.toml" => "cargo",
        "go.mod" => "go",
        "pom.xml" => "maven",
        _ => return None,
    })
}

/// Go `extract.IntrospectManifests`: one `package` node per manifest (keyed by package name), a
/// stub `package` node per external dependency, and `depends_on` edges. `files` are the manifests
/// under `root` as `detect::collect_manifests` lists them (Go calls it here; the caller passes
/// the list, so this pass has no error). A malformed or nameless manifest is skipped.
pub fn introspect_manifests(root: &Path, files: &[String]) -> Result {
    let mut files = files.to_vec();
    files.sort();

    // Pass 1: parse each manifest, keyed by canonical package id (last one wins on a name
    // collision, deterministic given the sorted file order).
    let mut modules: BTreeMap<String, ManifestInfo> = BTreeMap::new();
    let mut go_mods: BTreeMap<String, String> = BTreeMap::new();
    for rel in &files {
        let Some(eco) = manifest_ecosystem(&gostd::filepath::base(rel)) else {
            continue;
        };
        let (name, deps) = parse_manifest(eco, &root.join(rel));
        if name.is_empty() {
            continue;
        }
        let id = make_id(&["pkg", &name]);
        if eco == "go" {
            go_mods.insert(gostd::filepath::dir(rel), name.clone());
        }
        modules.insert(
            id.clone(),
            ManifestInfo {
                pkg_id: id,
                name,
                rel: rel.clone(),
                deps,
            },
        );
    }

    let mut res = Result {
        go_mods,
        ..Result::default()
    };
    // Module nodes first, then dependency stubs, for a deterministic node order.
    for m in modules.values() {
        res.nodes.push(package_node(&m.pkg_id, &m.name, &m.rel));
    }
    let mut stubbed: HashSet<String> = HashSet::new(); // external dep ids already emitted
    for m in modules.values() {
        let mut dep_names: Vec<&str> = Vec::with_capacity(m.deps.len());
        let mut seen: HashSet<String> = HashSet::new();
        for dep in &m.deps {
            if dep.is_empty() {
                continue;
            }
            let dep_id = make_id(&["pkg", dep]);
            if dep_id == m.pkg_id || !seen.insert(dep_id) {
                continue;
            }
            dep_names.push(dep);
        }
        dep_names.sort();
        for dep in dep_names {
            let dep_id = make_id(&["pkg", dep]);
            if !modules.contains_key(&dep_id) && stubbed.insert(dep_id.clone()) {
                res.nodes.push(package_node(&dep_id, dep, &m.rel));
            }
            res.edges.push(Edge {
                source: m.pkg_id.clone(),
                target: dep_id,
                relation: Relation::DEPENDS_ON,
                confidence: Confidence::EXTRACTED,
                weight: 1.0,
                source_file: file_of(&m.rel),
                source_location: line(1),
                ..Edge::default()
            });
        }
    }
    res
}

/// A `package` node anchored to line 1 of `rel`.
fn package_node(id: &str, label: &str, rel: &str) -> Node {
    Node {
        id: id.to_string(),
        label: label.to_string(),
        file_type: "package".to_string(),
        source_file: rel.to_string(),
        source_location: "L1".to_string(),
        ..Node::default()
    }
}

/// Go `parseManifest`: the package name and dependency names; a read or parse failure gives an
/// empty name, so the caller skips the manifest.
fn parse_manifest(eco: &str, path: &Path) -> (String, Vec<String>) {
    let Ok(src) = std::fs::read(path) else {
        return (String::new(), Vec::new());
    };
    match eco {
        "go" => parse_go_mod(&super::builder::go_text(&src)),
        "python" | "cargo" => match toml::decode(&src) {
            Ok(data) if eco == "python" => parse_pyproject(&data),
            Ok(data) => parse_cargo(&data),
            Err(_) => (String::new(), Vec::new()),
        },
        _ => parse_pom(&src),
    }
}

static GO_MODULE_RE: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile(r"^module\s+(\S+)"));
static GO_REQUIRE_OPEN: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile(r"^require\s*\("));
static GO_BLOCK_DEP_RE: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile(r"^(\S+)\s+v\S+"));
static GO_REQ_LINE_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"^require\s+(\S+)\s+v\S+"));

/// Go `parseGoMod`: the module path and the `require` directives, single-line or block.
fn parse_go_mod(text: &str) -> (String, Vec<String>) {
    let mut name = String::new();
    let mut deps = Vec::new();
    let mut in_block = false;
    for l in text.split('\n') {
        let s = trim_space(l);
        if name.is_empty()
            && let Some(m) = GO_MODULE_RE.find_string_submatch(s)
        {
            name = m[1].unwrap_or("").to_string();
            continue;
        }
        if GO_REQUIRE_OPEN.match_string(s) {
            in_block = true;
            continue;
        }
        if in_block {
            if s.starts_with(')') {
                in_block = false;
                continue;
            }
            if let Some(m) = GO_BLOCK_DEP_RE.find_string_submatch(s) {
                deps.push(m[1].unwrap_or("").to_string());
            }
        } else if let Some(m) = GO_REQ_LINE_RE.find_string_submatch(s) {
            deps.push(m[1].unwrap_or("").to_string());
        }
    }
    (name, deps)
}

/// Go `v.(map[string]any)` on an optional value.
pub(super) fn as_table(v: Option<&Value>) -> Option<&Table> {
    match v {
        Some(Value::Table(t)) => Some(t),
        _ => None,
    }
}

/// Go `v.(string)` on an optional value, "" when it is not a string.
fn as_str(v: Option<&Value>) -> &str {
    match v {
        Some(Value::String(s)) => s,
        _ => "",
    }
}

/// Go `parseCargo`: `[package].name` and the runtime dependency names of `[dependencies]` and
/// every `[target.<cfg>.dependencies]` (cfgs sorted). A virtual workspace root has no name.
fn parse_cargo(data: &Table) -> (String, Vec<String>) {
    let pkg = as_table(data.get("package"));
    let name = as_str(pkg.and_then(|p| p.get("name")));
    if name.is_empty() {
        return (String::new(), Vec::new());
    }
    let mut deps = cargo_dep_names(data.get("dependencies"));
    if let Some(targets) = as_table(data.get("target")) {
        for t in targets.values() {
            if let Value::Table(t) = t {
                deps.extend(cargo_dep_names(t.get("dependencies")));
            }
        }
    }
    (name.to_string(), deps)
}

/// Go `cargoDepNames`: the keys of a dependency table, sorted; nothing for a non-table.
fn cargo_dep_names(value: Option<&Value>) -> Vec<String> {
    as_table(value)
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default()
}

static PEP508_SPLIT_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"[\s<>=!~;\[\(]"));

/// Go `pep508Name`: the bare distribution name of a PEP 508 requirement.
fn pep508_name(spec: &str) -> &str {
    let s = trim_space(spec);
    match PEP508_SPLIT_RE.find_string_index(s) {
        Some((start, _)) => &s[..start],
        None => s,
    }
}

/// Go `parsePyproject`: the name and dependency names of PEP 621 `[project]` or Poetry
/// `[tool.poetry]`.
fn parse_pyproject(data: &Table) -> (String, Vec<String>) {
    let proj = as_table(data.get("project"));
    let poetry = as_table(as_table(data.get("tool")).and_then(|t| t.get("poetry")));
    let mut name = as_str(proj.and_then(|p| p.get("name")));
    if name.is_empty() {
        name = as_str(poetry.and_then(|p| p.get("name")));
    }
    if name.is_empty() {
        return (String::new(), Vec::new());
    }
    let mut deps = Vec::new();
    if let Some(Value::Array(arr)) = proj.and_then(|p| p.get("dependencies")) {
        for d in arr {
            if let Value::String(s) = d {
                let n = pep508_name(s);
                if !n.is_empty() {
                    deps.push(n.to_string());
                }
            }
        }
    }
    // Go ranges this map (`manifest.go:279`, random order); the keys are sorted here (D-002).
    if let Some(pdeps) = as_table(poetry.and_then(|p| p.get("dependencies"))) {
        for dep in pdeps.keys() {
            if gostd::strings::to_lower(dep) != "python" {
                deps.push(dep.clone());
            }
        }
    }
    (name.to_string(), deps)
}

/// Go `xmlElem.childText`: the trimmed text of the first child named `local`, else "".
fn child_text<'a>(e: &'a Element, local: &str) -> &'a str {
    e.children
        .iter()
        .find(|c| c.name.local == local)
        .map_or("", |c| trim_space(&c.chardata))
}

static POM_PLACEHOLDER: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile(r"\$\{([^}]+)\}"));

/// Go `parsePom`: `groupId:artifactId` (or the bare artifactId) and the coordinates of every
/// `<dependencies>/<dependency>` at any depth, `${...}` properties resolved.
fn parse_pom(text: &[u8]) -> (String, Vec<String>) {
    let Ok(root) = gostd::xml::unmarshal_element(text) else {
        return (String::new(), Vec::new());
    };
    let aid = child_text(&root, "artifactId");
    if aid.is_empty() {
        return (String::new(), Vec::new());
    }
    let mut parent: Option<&Element> = None;
    let mut props: BTreeMap<String, String> = BTreeMap::new();
    for c in &root.children {
        match c.name.local.as_str() {
            "parent" => parent = Some(c),
            "properties" => {
                for p in &c.children {
                    let v = trim_space(&p.chardata);
                    if !v.is_empty() {
                        props.insert(p.name.local.clone(), v.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    let parent_text = |local: &str| parent.map_or("", |p| child_text(p, local));
    let mut gid = child_text(&root, "groupId");
    if gid.is_empty() {
        gid = parent_text("groupId");
    }
    for (k, v) in [
        ("project.groupId", gid),
        ("project.artifactId", aid),
        ("project.version", child_text(&root, "version")),
        ("project.parent.groupId", parent_text("groupId")),
        ("project.parent.version", parent_text("version")),
    ] {
        if !v.is_empty() && !props.contains_key(k) {
            props.insert(k.to_string(), v.to_string());
        }
    }
    let resolve = |s: &str| {
        POM_PLACEHOLDER.replace_all_string_func(s, |m| {
            props
                .get(&m[2..m.len() - 1])
                .cloned()
                .unwrap_or_else(|| m.to_string())
        })
    };
    let gid = resolve(gid);
    let name = if gid.is_empty() {
        aid.to_string()
    } else {
        format!("{gid}:{aid}")
    };
    // Go's `collect` recurses in pre-order; this walks the same order with a stack.
    let mut deps = Vec::new();
    let mut todo: Vec<&Element> = vec![&root];
    while let Some(e) = todo.pop() {
        if e.name.local == "dependencies" {
            for c in e.children.iter().filter(|c| c.name.local == "dependency") {
                let da = resolve(child_text(c, "artifactId"));
                if da.is_empty() {
                    continue;
                }
                let dg = resolve(child_text(c, "groupId"));
                if dg.is_empty() {
                    deps.push(da);
                } else {
                    deps.push(format!("{dg}:{da}"));
                }
            }
        }
        todo.extend(e.children.iter().rev());
    }
    (name, deps)
}

#[cfg(test)]
#[path = "tests/manifest.rs"]
mod manifest_tests;

#[cfg(test)]
#[path = "tests/upstream_manifest_ingest.rs"]
mod upstream_manifest_ingest_tests;
