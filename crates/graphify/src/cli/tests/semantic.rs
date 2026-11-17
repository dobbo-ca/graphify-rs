//! Port of `cmd/graphify/semantic_test.go` (2 tests).

use super::core_util::{read_graph, write_tree};
use super::{cmd_build, parse_build_opts};
use crate::cli::maintenance::maint_util::args;
use crate::testutil::TempDir;

#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_parse_build_args_semantic() {
    // (args, root, semantic, backend, want_err)
    type Case = (
        &'static [&'static str],
        &'static str,
        bool,
        &'static str,
        bool,
    );
    const CASES: [Case; 5] = [
        (&[], ".", false, "", false),
        (
            &["--semantic", "--backend", "bedrock"],
            ".",
            true,
            "bedrock",
            false,
        ),
        (
            &["path", "--semantic", "--backend=bedrock"],
            "path",
            true,
            "bedrock",
            false,
        ),
        // --semantic with no --backend errors
        (&["--semantic"], ".", true, "", true),
        // backend without semantic is inert (semantic off)
        (&["--backend", "bedrock"], ".", false, "bedrock", false),
    ];
    for (a, root, semantic, backend, want_err) in CASES {
        let got = parse_build_opts(&args(a));
        assert_eq!(got.is_err(), want_err, "parse_build_opts({a:?}) = {got:?}");
        let Ok(opts) = got else { continue };
        assert!(
            opts.root == root && opts.semantic == semantic && opts.backend == backend,
            "parse_build_opts({a:?}) = {opts:?}, want root={root:?} semantic={semantic} backend={backend:?}"
        );
    }
}

// A build WITHOUT --semantic produces byte-identical graph.json across runs (the
// deterministic-core invariant the semantic stage must never disturb).
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_build_off_is_deterministic() {
    let root = TempDir::new();
    write_tree(
        root.path(),
        &[
            ("a.md", "# A\n\nSee [B](b.md).\n"),
            (
                "b.md",
                "# B\n\nIsolated note about deployment bake times.\n",
            ),
        ],
    );
    let graph_path = root.join("graphify-out/graph.json");
    let r = root.path().to_str().expect("UTF-8 temp path");
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    if let Err(e) = cmd_build(&args(&[r]), &mut out, &mut err) {
        panic!("build 1: {e}");
    }
    let first = read_graph(&graph_path);
    if let Err(e) = cmd_build(&args(&[r]), &mut out, &mut err) {
        panic!("build 2: {e}");
    }
    let second = read_graph(&graph_path);
    assert!(
        first == second,
        "build without --semantic is not deterministic across runs"
    );
}
