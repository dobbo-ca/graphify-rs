//! Port of `budget_test.go`: `TestParseBudgetSameAtAnyGOMAXPROCS` here, and the added
//! `build_output_independent_of_workers` (plan Review Focus 2). `TestParseBudgetEndToEnd` sets
//! `ts.Budget` to 70000 for whole runs, which reaches the worker threads only through
//! `GRAPHIFY_PARSE_BUDGET`, so it runs the binary (`crates/graphify/tests/cli_budget.rs`).

use std::path::Path;

use super::cmd_build_with;
use super::core_util::{copy_tree, write_tree};
use crate::cli::maintenance::maint_util::args;
use crate::testutil::{TempDir, testdata};

/// Go `stallJulia`: makes the julia parser retry one byte without end.
const STALL_JULIA: &str = "@testset \"t0\" begin\n    @testset \"t1\" begin\n        l2";

/// `build <root>` with `workers` workers: (stderr, graph.json, GRAPH_REPORT.md).
fn build(root: &Path, workers: usize) -> (String, String, String) {
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    let a = args(&[root.to_str().expect("UTF-8 temp path")]);
    if let Err(e) = cmd_build_with(&a, workers, &mut out, &mut err) {
        panic!("build with {workers} workers: {e}");
    }
    let read = |rel: &str| {
        std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
    };
    (
        String::from_utf8(err).expect("stderr is UTF-8"),
        read("graphify-out/graph.json"),
        read("graphify-out/GRAPH_REPORT.md"),
    )
}

// Go runs the build at GOMAXPROCS 1 and 8; the worker count is a parameter here (plan R5). The
// budget is Go's default, 65536, which `ts::budget` gives when GRAPHIFY_PARSE_BUDGET is unset.
// Also needs julia (graphify-rs-ihc.38) for the stall.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_parse_budget_same_at_any_gomaxprocs() {
    let mut want: Option<(String, String)> = None;
    for procs in [1, 8] {
        let root = TempDir::new();
        let t_jl = format!("{STALL_JULIA}\n");
        write_tree(
            root.path(),
            &[
                ("s.jl", STALL_JULIA),
                ("t.jl", &t_jl),
                ("a.go", "package p\n\nfunc A() { B() }\n"),
                ("b.go", "package p\n\nfunc B() {}\n"),
            ],
        );
        let (stderr, graph, report) = build(root.path(), procs);
        let (s, t) = (stderr.find("s.jl"), stderr.find("t.jl"));
        assert!(
            stderr.matches("warning: skipped").count() == 2 && s.is_some() && t.is_some() && s < t,
            "GOMAXPROCS {procs}: stderr = {stderr:?}, want s.jl then t.jl"
        );
        let r = root.path().to_str().expect("UTF-8 temp path");
        let got = (graph.replace(r, ""), report.replace(r, ""));
        match &want {
            None => want = Some(got),
            Some(w) => assert!(
                *w == got,
                "GOMAXPROCS {procs}: outputs differ from GOMAXPROCS 1"
            ),
        }
    }
}

// Plan Review Focus 2: Go's output does not depend on GOMAXPROCS, so ours must not depend on the
// worker count. The `testdata/extract` corpus (every extractor) built with 1 and with 8 workers
// writes the same graph.json bytes. The corpus holds every grammar: csharp (ihc.28), hcl (ihc.31),
// ruby (ihc.33), php (ihc.34), scala (ihc.36), kotlin (ihc.37), julia (ihc.38) and lua (ihc.39)
// must have landed as well.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn build_output_independent_of_workers() {
    let root = TempDir::new();
    copy_tree(&testdata().join("extract"), root.path());
    let (_, one, _) = build(root.path(), 1);
    let (_, eight, _) = build(root.path(), 8);
    assert!(one == eight, "graph.json differs between 1 and 8 workers");
}
