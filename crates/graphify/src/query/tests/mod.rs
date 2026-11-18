//! Shared helpers (Go: the test files of package query share one namespace: `loadJSON` and
//! `affectedJSON` of affected_test.go, `nodeJSON`, `linkJSON` and `graphJSON` of ask_test.go).

use std::path::PathBuf;

use crate::query::{Graph, Node, load};
use crate::testutil::TempDir;

mod added;
mod affected;
mod ask;
mod diff;
mod explain_path;
mod merge;
mod query;
mod unclassified;
mod upstream_affected;
mod upstream_path;
mod upstream_query;
mod upstream_validate;
mod validate;

/// Go `loadJSON`: writes `content` to `<tmp>/graphify-out/graph.json` and loads it.
pub(super) fn load_json(content: &str) -> Graph {
    let dir = TempDir::new();
    let p = dir.write("graphify-out/graph.json", content.as_bytes());
    load(&p).unwrap_or_else(|e| panic!("load: {e}"))
}

/// Go's `os.WriteFile(filepath.Join(t.TempDir(), "graph.json"), ...)` then `Load` (`loadSample`,
/// `loadDup`, `loadAsk` and the inline loads).
pub(super) fn load_str(body: &str) -> Graph {
    let dir = TempDir::new();
    let p = dir.write("graph.json", body.as_bytes());
    load(&p).unwrap_or_else(|e| panic!("load: {e}"))
}

/// Go `writeGraphJSON` (validate_test.go): the temp dir (kept alive by the caller) and the path
/// of `<tmp>/graphify-out/graph.json`.
pub(super) fn write_graph_json(content: &[u8]) -> (TempDir, PathBuf) {
    let dir = TempDir::new();
    let p = dir.write("graphify-out/graph.json", content);
    (dir, p)
}

/// Go `affectedJSON`: util.go defines add(); math.go defines compute() which calls add();
/// main.go defines run() which calls compute(). Changing util.go should impact compute (caller
/// of add) and run (caller of compute), transitively.
pub(super) const AFFECTED_JSON: &str = r#"{"directed":true,"multigraph":false,"graph":{},
"nodes":[
 {"id":"add","label":"add()","file_type":"code","source_file":"util.go","source_location":"L1"},
 {"id":"compute","label":"compute()","file_type":"code","source_file":"math.go","source_location":"L1"},
 {"id":"run","label":"run()","file_type":"code","source_file":"main.go","source_location":"L1"},
 {"id":"lonely","label":"lonely()","file_type":"code","source_file":"other.go","source_location":"L1"}
],
"links":[
 {"source":"compute","target":"add","relation":"calls","confidence":"INFERRED"},
 {"source":"run","target":"compute","relation":"calls","confidence":"INFERRED"}
],
"hyperedges":[]}"#;

/// Go `nodeJSON`.
pub(super) fn node_json(id: &str, label: &str, file: &str, loc: &str) -> String {
    format!(
        r#"{{"id":"{id}","label":"{label}","file_type":"code","source_file":"{file}","source_location":"{loc}","community":0}}"#
    )
}

/// Go `linkJSON`.
pub(super) fn link_json(
    src: &str,
    tgt: &str,
    rel: &str,
    conf: &str,
    file: &str,
    loc: &str,
    extra: &str,
) -> String {
    format!(
        r#"{{"source":"{src}","target":"{tgt}","relation":"{rel}","confidence":"{conf}","source_file":"{file}","source_location":"{loc}"{extra}}}"#
    )
}

/// Go `graphJSON`.
pub(super) fn graph_json(nodes: &[String], links: &[String]) -> String {
    format!(
        r#"{{"directed":false,"multigraph":true,"graph":{{}},"nodes":[{}],"links":[{}]}}"#,
        nodes.join(","),
        links.join(",")
    )
}

/// The ids of `nodes`, in order.
pub(super) fn ids(nodes: &[Node]) -> Vec<&str> {
    nodes.iter().map(|n| n.id.as_str()).collect()
}

/// For the `*_ties_use_go_pdqsort` tests (plan hazard "sort.Slice with ties"): a key per element,
/// `(j * a) % m` for the first `(n, m, a)` (n >= 13) where Go's pdqsort (`gostd::sort::slice`
/// over the keys) puts the elements in another order than a stable sort does. The search is in
/// the test, not done by hand: what is asserted is that the code under test sorts like Go.
pub(super) fn tie_keys() -> Vec<usize> {
    for n in 13..=64usize {
        for m in 2..=5usize {
            for a in 1..n {
                let keys: Vec<usize> = (0..n).map(|j| (j * a) % m).collect();
                let mut go: Vec<usize> = (0..n).collect();
                gostd::sort::slice(&mut go, |&x, &y| keys[x] < keys[y]);
                let mut stable: Vec<usize> = (0..n).collect();
                stable.sort_by_key(|&x| keys[x]);
                if go != stable {
                    return keys;
                }
            }
        }
    }
    panic!("no key pattern where pdqsort differs from a stable sort")
}
