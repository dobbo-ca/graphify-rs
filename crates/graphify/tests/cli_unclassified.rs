//! Port of graphify-go `cmd/graphify/unclassified_test.go`: 3 of its 4 tests, through the binary.
//! The two build tests read the graph with `query.Load` and `UnclassifiedSummary()`; here
//! `validate` in the root prints that summary (`cmdValidate`), which `TestCmdValidateReports
//! Unclassified` pins. `TestToolGraphStatsReportsUnclassified` calls the MCP server
//! (`newMCPServer`, serve.go): it lands with the cli-serve tests PR (graphify-rs-ihc.22).

mod common;

use common::{TempDir, ok_in, s};

const COVERAGE_LINE: &str = "Unclassified: 40 file(s) no extractor handles (.swift 40)";

// A corpus written mostly in a language no extractor covers says so: the count and the
// extension ride in graph.json's graph-level attributes.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_build_records_unclassified_corpus() {
    let root = TempDir::new();
    for i in 0..40 {
        root.write(&format!("View{i}.swift"), "struct V {}\n");
    }
    root.write("main.go", "package p\n\nfunc A() {}\n");
    ok_in(root.path(), &["build", s(root.path())], &[]);
    let out = ok_in(root.path(), &["validate"], &[]).stdout;
    assert!(
        out.contains("40") && out.contains(".swift"),
        "summary {out:?}: want the count 40 and the extension .swift named"
    );
}

// A fully-classified corpus reports no unclassified line at all.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_build_fully_classified_corpus_has_no_unclassified_line() {
    let root = TempDir::new();
    root.write("main.go", "package p\n\nfunc A() {}\n");
    ok_in(root.path(), &["build", s(root.path())], &[]);
    let out = ok_in(root.path(), &["validate"], &[]).stdout;
    assert!(
        !out.contains("Unclassified"),
        "summary in {out:?}, want none"
    );
}

// `graphify validate` prints the coverage line alongside its verdict, and a thinly-covered
// corpus is still a structurally OK graph (exit 0).
#[test]
fn test_cmd_validate_reports_unclassified() {
    let root = TempDir::new();
    root.write(
        "graphify-out/graph.json",
        r#"{"directed":false,"multigraph":false,
  "graph":{"unclassified_files":40,"unclassified_extensions":{".swift":40}},
  "nodes":[{"id":"a","label":"a()","file_type":"code","source_file":"main.go","source_location":"L1","community":0,"norm_label":"a()"}],
  "links":[]}"#,
    );
    let out = ok_in(root.path(), &["validate"], &[]).stdout;
    assert!(
        out.contains(COVERAGE_LINE),
        "validate missing the coverage line:\n{out}"
    );
}
