//! Shared helpers of the cli-core unit tests (graphify-go `cmd/graphify/*_test.go` helpers).

use std::path::Path;

use crate::cli::Error;

/// The function type of `cmd_build` and `cmd_update`.
pub(crate) type Cmd = fn(
    &[std::ffi::OsString],
    &mut dyn std::io::Write,
    &mut dyn std::io::Write,
) -> Result<(), Error>;

/// Go `writeTree` (`budget_test.go`): writes each `(rel, content)` under `root`.
pub(crate) fn write_tree(root: &Path, files: &[(&str, &str)]) {
    for (rel, content) in files {
        let p = root.join(rel);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).expect("create parent dir");
        }
        std::fs::write(&p, content).unwrap_or_else(|e| panic!("write {}: {e}", p.display()));
    }
}

/// Copies the tree at `from` into `to` (std has no recursive copy).
#[allow(clippy::disallowed_methods)] // a test copy; the order of the entries does not matter
pub(crate) fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create dir");
    for e in std::fs::read_dir(from).expect("read dir") {
        let e = e.expect("dir entry");
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type().expect("file type").is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).expect("copy file");
        }
    }
}

/// Go `readGraph` (`cargo_test.go`).
pub(crate) fn read_graph(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read graph: {e}"))
}

/// Go `graphNodeCount` (`update_test.go`): the number of nodes in a graph.json.
pub(crate) fn graph_node_count(path: &Path) -> usize {
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let v: gostd::json::Value = gostd::json::unmarshal(&data)
        .unwrap_or_else(|e| panic!("unmarshal {}: {e}", path.display()));
    match &v {
        gostd::json::Value::Object(m) => match m.get("nodes") {
            Some(gostd::json::Value::Array(a)) => a.len(),
            _ => 0,
        },
        _ => panic!("{}: not a JSON object", path.display()),
    }
}

/// The `nodes` of a graph.json, each an object.
pub(crate) fn graph_nodes(
    path: &Path,
) -> Vec<std::collections::BTreeMap<String, gostd::json::Value>> {
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut v: gostd::json::Value = gostd::json::unmarshal(&data)
        .unwrap_or_else(|e| panic!("unmarshal {}: {e}", path.display()));
    let gostd::json::Value::Object(m) = &mut v else {
        panic!("{}: not a JSON object", path.display());
    };
    let Some(gostd::json::Value::Array(nodes)) = m.get_mut("nodes") else {
        return Vec::new();
    };
    std::mem::take(nodes)
        .into_iter()
        .filter_map(|mut n| match &mut n {
            gostd::json::Value::Object(o) => Some(std::mem::take(o)),
            _ => None,
        })
        .collect()
}

/// A JSON string field of a graph node, `""` when absent or not a string.
pub(crate) fn str_field<'a>(
    n: &'a std::collections::BTreeMap<String, gostd::json::Value>,
    key: &str,
) -> &'a str {
    match n.get(key) {
        Some(gostd::json::Value::String(s)) => s,
        _ => "",
    }
}
