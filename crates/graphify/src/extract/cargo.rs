//! Port of `cargo.go`: the Cargo workspace pass (`--cargo`). Owner: the extractors-and-CLI
//! stream; filled by graphify-rs-ihc.17.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::io::Read;
use std::path::Path;

use gostd::filepath::{abs, clean, dir, glob, is_abs, join};
use gostd::toml::{self, Table, Value};

use super::extract::Result;
use super::manifest::as_table;
use crate::goerr::path_error;
use crate::model::{Confidence, Edge, Node, Relation, file_of, line};

/// An error of [`introspect_cargo`]; `Display` is Go's error text (a TOML `ParseError`, a
/// `filepath.Abs` or `Rel` error, or the `open` error of a missing root manifest).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// Go `crate`: a discovered package and the manifest it came from (relative to the root,
/// slash-separated).
struct Crate {
    id: String,
    manifest: String,
    data: Table,
}

/// A path as the Go string (plan R3).
fn path_string(p: &Path) -> String {
    crate::detect::go_lossy(p.as_os_str().as_encoded_bytes())
}

/// Go `filepath.Abs`; its only error is `os.Getwd`'s, which stats `.` first.
fn abs_string(p: &str) -> std::result::Result<String, Error> {
    abs(Path::new(p))
        .map(|a| path_string(&a))
        .map_err(|e| Error(path_error("stat", Path::new("."), &e).to_string()))
}

/// Go `extract.IntrospectCargo`: a `crate:<name>` node per workspace member with a
/// `[package].name`, and `crate_depends_on` edges for workspace-internal dependencies only. With
/// no `root/Cargo.toml` it takes the shallowest `Cargo.toml` that `detect::collect_manifests`
/// keeps. A TOML syntax error in any manifest is an error.
pub fn introspect_cargo(root: &Path) -> std::result::Result<Result, Error> {
    let root = path_string(root);
    let mut root_manifest = join(&[&root, "Cargo.toml"]);
    if std::fs::metadata(&root_manifest).is_err()
        && let Some(found) = shallowest_cargo(&root)
    {
        root_manifest = found;
    }
    let ws_root = dir(&root_manifest);
    let abs_root = abs_string(&ws_root)?; // relative roots break path comparison
    let scan_abs = abs_string(&root)?;
    let root_data = load_toml(&root_manifest)?;

    let manifests = member_manifest_paths(&ws_root, &root_data)?;

    let mut crates: BTreeMap<String, Crate> = BTreeMap::new();
    for manifest in &manifests {
        let data = if *manifest == root_manifest {
            root_data.clone()
        } else {
            load_toml(manifest)?
        };
        let Some(name) = package_name(&data).map(str::to_string) else {
            continue;
        };
        let rel = gostd::filepath::rel(&root, manifest).map_err(|e| Error(e.to_string()))?;
        crates.insert(
            name.clone(),
            Crate {
                id: format!("crate:{name}"),
                manifest: rel,
                data,
            },
        );
    }

    let mut res = Result::default();
    for (name, c) in &crates {
        res.nodes.push(Node {
            id: c.id.clone(),
            label: name.clone(),
            file_type: "code".to_string(),
            source_file: c.manifest.clone(),
            source_location: "L1".to_string(),
            ..Node::default()
        });
    }
    let ws_deps =
        as_table(root_data.get("workspace")).and_then(|w| as_table(w.get("dependencies")));
    for c in crates.values() {
        let Some(deps) = as_table(c.data.get("dependencies")) else {
            continue;
        };
        for (dep_name, dep) in deps {
            // Honor a dep-table rename: `db = { path = "...", package = "real" }` binds to the
            // workspace crate named "real", not the dep key "db" (#1858). A rename pointing at a
            // registry/external crate simply misses `crates` and stays a no-op.
            let mut lookup = dep_name.as_str();
            let mut spec = as_table(Some(dep));
            let mut inherited = false;
            if let Some(s) = spec
                && let Some(ws) = s.get("workspace")
            {
                if *ws != Value::Bool(true) {
                    continue;
                }
                inherited = true;
                spec = as_table(ws_deps.and_then(|w| w.get(dep_name)));
                if spec_path(spec).is_empty() {
                    continue; // registry/git dep: name match proves nothing
                }
            }
            if let Some(Value::String(pkg)) = spec.and_then(|s| s.get("package"))
                && !pkg.is_empty()
            {
                lookup = pkg.as_str();
            }
            let Some(target) = crates.get(lookup) else {
                continue; // registry dep or unknown crate: not an internal edge
            };
            if inherited {
                // Shared paths are relative to the workspace root.
                let mut want = spec_path(spec).to_string();
                if !is_abs(&want) {
                    want = join(&[&abs_root, &want]);
                }
                if resolve_path(&join(&[&want, "Cargo.toml"]))
                    != resolve_path(&join(&[&scan_abs, &target.manifest]))
                {
                    continue;
                }
            }
            res.edges.push(Edge {
                source: c.id.clone(),
                target: target.id.clone(),
                relation: Relation::CRATE_DEPENDS_ON,
                confidence: Confidence::EXTRACTED,
                weight: 1.0,
                source_file: file_of(&c.manifest),
                source_location: line(1),
                ..Edge::default()
            });
        }
    }
    Ok(res)
}

/// Go `spec["path"].(string)`, "" when it is not a string.
fn spec_path(spec: Option<&Table>) -> &str {
    match spec.and_then(|s| s.get("path")) {
        Some(Value::String(p)) => p,
        _ => "",
    }
}

/// Go `shallowestCargo`: the least-nested `Cargo.toml` under `root` that detect keeps (so ignore
/// rules apply), ties broken by path; `None` when there is none or the walk fails (Go's caller
/// ignores both).
fn shallowest_cargo(root: &str) -> Option<String> {
    let files = collect_manifests(Path::new(root)).ok()?;
    let mut best = "";
    for f in &files {
        if gostd::filepath::base(f) != "Cargo.toml" {
            continue;
        }
        let d = f.matches('/').count();
        let bd = best.matches('/').count();
        if best.is_empty() || d < bd || (d == bd && f.as_str() < best) {
            best = f.as_str();
        }
    }
    if best.is_empty() {
        return None;
    }
    Some(join(&[root, best]))
}

/// Go `detect.CollectManifests`, which detect (graphify-rs-ihc.14) ports; until then a root with
/// no `Cargo.toml` stops here.
fn collect_manifests(_root: &Path) -> std::io::Result<Vec<String>> {
    todo!("graphify-rs-ihc.14: detect::collect_manifests")
}

/// Go `resolvePath`: follows symlinks like Python's `Path.resolve()`; falls back to `Clean`.
fn resolve_path(p: &str) -> String {
    match gostd::filepath::eval_symlinks(Path::new(p)) {
        Ok(r) => path_string(&r),
        Err(_) => clean(p),
    }
}

/// Go `loadTOML`: BurntSushi `toml.DecodeFile` into `map[string]any`, so degenerate values like
/// `dependencies = "not-a-table"` are type-checked later rather than rejected here.
fn load_toml(path: &str) -> std::result::Result<Table, Error> {
    let p = Path::new(path);
    let go_err = |op: &str, e: std::io::Error| Error(path_error(op, p, &e).to_string());
    let mut f = std::fs::File::open(p).map_err(|e| go_err("open", e))?;
    let mut src = Vec::new();
    f.read_to_end(&mut src).map_err(|e| go_err("read", e))?;
    toml::decode(&src).map_err(|e| Error(e.to_string()))
}

/// Go `memberManifestPaths`: the root manifest when the root is itself a package, plus every
/// `Cargo.toml` under an expanded `[workspace].members` glob, as paths joined on `root`.
fn member_manifest_paths(root: &str, root_data: &Table) -> std::result::Result<Vec<String>, Error> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    let mut add = |p: String| {
        if seen.insert(p.clone()) {
            paths.push(p);
        }
    };

    if as_table(root_data.get("package")).is_some() {
        add(join(&[root, "Cargo.toml"]));
    }
    if let Some(workspace) = as_table(root_data.get("workspace"))
        && let Some(Value::Array(members)) = workspace.get("members")
    {
        for raw in members {
            let Value::String(pattern) = raw else {
                continue;
            };
            let matches =
                glob(Path::new(&join(&[root, pattern]))).map_err(|e| Error(e.to_string()))?;
            let mut matches: Vec<String> = matches.into_iter().map(|m| path_string(&m)).collect();
            matches.sort();
            for member in matches {
                let manifest = join(&[&member, "Cargo.toml"]);
                if std::fs::metadata(&manifest).is_ok_and(|m| m.is_file()) {
                    add(manifest);
                }
            }
        }
    }
    Ok(paths)
}

/// Go `packageName`: the `[package].name` string, `None` when there is no package table or no
/// string name.
fn package_name(data: &Table) -> Option<&str> {
    match as_table(data.get("package"))?.get("name")? {
        Value::String(name) => Some(name),
        _ => None,
    }
}

#[cfg(test)]
#[path = "tests/cargo.rs"]
mod cargo_tests;

#[cfg(test)]
#[path = "tests/cargo_rename.rs"]
mod cargo_rename_tests;

#[cfg(test)]
#[path = "tests/upstream_cargo_introspect.rs"]
mod upstream_cargo_introspect_tests;
