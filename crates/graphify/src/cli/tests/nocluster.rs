//! Port of `nocluster_test.go` (1 test).

use std::path::Path;

use super::cmd_build;
use super::core_util::{graph_nodes, write_tree};
use crate::cli::maintenance::maint_util::args;
use crate::testutil::TempDir;

/// Go `anyNodeClustered`: whether any node in graph.json has a non-null community.
pub(crate) fn any_node_clustered(path: &Path) -> bool {
    graph_nodes(path).iter().any(|n| {
        n.get("community")
            .is_some_and(|c| !matches!(c, gostd::json::Value::Null))
    })
}

// --no-cluster writes the raw extraction: every node lands with a null community, whereas a
// plain build over the same corpus assigns communities.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_build_no_cluster() {
    let root = TempDir::new();
    write_tree(
        root.path(),
        &[
            ("a.go", "package p\n\nfunc A() { B() }\n"),
            ("b.go", "package p\n\nfunc B() {}\n"),
        ],
    );
    let graph_path = root.join("graphify-out/graph.json");
    let r = root.path().to_str().expect("UTF-8 temp path");
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());

    // Plain build assigns communities: at least one node carries a non-null community.
    if let Err(e) = cmd_build(&args(&[r]), &mut out, &mut err) {
        panic!("build: {e}");
    }
    assert!(
        any_node_clustered(&graph_path),
        "plain build produced no community assignments; fixture cannot detect the difference"
    );

    // --no-cluster build: every node's community must be null.
    if let Err(e) = cmd_build(&args(&[r, "--no-cluster"]), &mut out, &mut err) {
        panic!("build --no-cluster: {e}");
    }
    assert!(
        !any_node_clustered(&graph_path),
        "--no-cluster build assigned a community; expected raw extraction with no clustering"
    );
}
