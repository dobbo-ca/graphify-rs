//! Port of graphify-go `cmd/graphify/budget_test.go` `TestParseBudgetEndToEnd`: Go sets
//! `ts.Budget` for whole runs (65536, then 70000); the binary reads it from
//! `GRAPHIFY_PARSE_BUDGET`. Go's `runOut(root, cmdBuild)` is `build <root>` here.

mod common;

use common::{TempDir, ok_in, read, s};

/// Go `stallJulia`: makes the julia parser retry one byte without end.
const STALL_JULIA: &str = "@testset \"t0\" begin\n    @testset \"t1\" begin\n        l2";

const BUDGET_WARNING: &str = "  warning: skipped s.jl (parser made no progress, GRAPHIFY_PARSE_BUDGET=65536; 0 turns the limit off)\n";
const BUDGET_LINE: &str = "- 1 file(s) skipped: parser made no progress (GRAPHIFY_PARSE_BUDGET)\n";

/// Go `runOut`: `graphify <cmd> <root>` with the budget; (stdout, stderr, graph.json, report).
fn run_out(root: &TempDir, cmd: &str, budget: &str) -> (String, String, String, String) {
    let r = ok_in(
        root.path(),
        &[cmd, s(root.path())],
        &[("GRAPHIFY_PARSE_BUDGET", budget)],
    );
    (
        r.stdout,
        r.stderr,
        read(&root.join("graphify-out/graph.json")),
        read(&root.join("graphify-out/GRAPH_REPORT.md")),
    )
}

// Also needs julia (graphify-rs-ihc.38) for the stall.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_parse_budget_end_to_end() {
    let root = TempDir::new();
    root.write("s.jl", STALL_JULIA);
    root.write("a.go", "package p\n\nfunc A() { B() }\n\nfunc B() {}\n");

    let (_, stderr, graph, report) = run_out(&root, "build", "65536");
    assert_eq!(stderr, BUDGET_WARNING, "build: stderr");
    assert!(
        report.contains("communities\n- ") && report.contains(BUDGET_LINE),
        "build: report lacks {BUDGET_LINE:?}:\n{report}"
    );
    for want in [
        r#""label": "A()""#,
        r#""label": "B()""#,
        r#""label": "s.jl""#,
    ] {
        assert!(graph.contains(want), "build: graph.json lacks {want}");
    }

    let (stdout, stderr, graph2, report2) = run_out(&root, "update", "65536");
    assert!(
        stdout.contains("(0 reparsed, 2 reused") && stderr == BUDGET_WARNING,
        "update: stdout = {stdout:?}, stderr = {stderr:?}, want all reused and the warning"
    );
    assert!(
        graph2 == graph && report2 == report,
        "update changed the outputs of an unchanged tree"
    );

    // Another limit is another cache.
    let (stdout, stderr, graph2, _) = run_out(&root, "update", "70000");
    assert!(
        stdout.contains("(2 reparsed, 0 reused")
            && stderr.contains("GRAPHIFY_PARSE_BUDGET=70000")
            && stderr.contains("rebuilding it once"),
        "changed budget: stdout = {stdout:?}, stderr = {stderr:?}, want a full reparse with a note"
    );
    assert!(graph2 == graph, "changed budget: graph.json differs");
    run_out(&root, "update", "65536");

    // A fixed file is parsed and no longer reported.
    root.write("s.jl", "function f()\nend\n");
    let (stdout, stderr, graph2, report2) = run_out(&root, "update", "65536");
    assert!(
        stdout.contains("(1 reparsed, 1 reused") && stderr.is_empty(),
        "fixed file: stdout = {stdout:?}, stderr = {stderr:?}, want one reparse and no warning"
    );
    assert!(
        !report2.contains("skipped") && graph2.contains(r#""label": "f()""#),
        "fixed file: report still has the line or the graph lacks f():\n{report2}"
    );

    // A good file that turns into a stopped one loses nodes without --force.
    root.write("s.jl", STALL_JULIA);
    let (stdout, stderr, graph2, report2) = run_out(&root, "update", "65536");
    assert!(
        stdout.contains("(1 reparsed, 1 reused") && stderr == BUDGET_WARNING,
        "file turned bad: stdout = {stdout:?}, stderr = {stderr:?}"
    );
    assert!(
        graph2 == graph && report2 == report,
        "file turned bad: outputs differ from the first build"
    );
}
