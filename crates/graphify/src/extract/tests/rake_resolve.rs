//! graphify-go `internal/extract/rake_resolve_test.go`.

use crate::extract::{file, kinds, resolve};
use crate::graph;
use crate::model::Extraction;
use crate::testutil::TempDir;

/// Go `resolveFiles` (`bash_scriptinvocation_test.go:13`): `File` on each of `files` under
/// `root`, then `Resolve` (GOMAXPROCS workers).
fn resolve_files(root: &TempDir, files: &[&str]) -> Extraction {
    kinds::register_kinds();
    let results: Vec<_> = files
        .iter()
        .map(|f| file(root.path(), f).unwrap_or_else(|e| panic!("File({f}): {e}")))
        .collect();
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

// A call made in a .rake file must cross-resolve to a definition in a .rb file:
// .rake is plain Ruby, so both share the ruby language family and the call edge
// must survive resolution rather than being dropped as a cross-family phantom
// (#1784). Mirrors the ruby cross-file boot()->add() shape in ruby_test.go.
// ruby test (ihc.33): needs resolve
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_rake_cross_resolves_to_rb() {
    let root = TempDir::new();
    root.write("lib/helpers.rb", b"def add(a, b)\n  a + b\nend\n");
    root.write("tasks/build.rake", b"def run\n  add(1, 2)\nend\n");

    let ext = resolve_files(&root, &["lib/helpers.rb", "tasks/build.rake"]);
    let g = graph::build(ext.clone());

    let mut run_id = "";
    let mut add_id = "";
    for n in &ext.nodes {
        match n.label.as_str() {
            "run()" => run_id = &n.id,
            "add()" => add_id = &n.id,
            _ => {}
        }
    }
    assert!(
        !run_id.is_empty() && !add_id.is_empty(),
        "missing node ids: run()={run_id:?} add()={add_id:?}"
    );
    assert!(
        g.has_edge(run_id, add_id),
        ".rake call run() did not cross-resolve to .rb def add(): ruby-family grouping dropped the edge"
    );
}
