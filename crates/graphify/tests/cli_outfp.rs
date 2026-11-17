//! Port of graphify-go `cmd/graphify/outfp_test.go`: the 3 tests whose `fpRoot` sets
//! `GRAPHIFY_MTIME_GRANULARITY_MS=0` for whole runs, so they run the binary with it.
//! (`TestOutFPChangesWithBinary` and `TestSigOfSeesReplacedFile` are unit tests in
//! `src/cli/tests/outfp.rs`.) Go's `runOut(root, cmd)` is `<cmd> <root>` here.

mod common;

use std::path::Path;
use std::time::SystemTime;

use common::{COMMIT, TempDir, git, ok_in, read, s, write};

const ENV: [(&str, &str); 1] = [("GRAPHIFY_MTIME_GRANULARITY_MS", "0")];

/// Go `stallJulia`.
const STALL_JULIA: &str = "@testset \"t0\" begin\n    @testset \"t1\" begin\n        l2";

/// Go `fpRoot`: a git repo with a.go and b.go committed.
fn fp_root() -> TempDir {
    let root = TempDir::new();
    git(root.path(), &["init", "-q"]);
    root.write("a.go", "package p\n\nfunc A() { B() }\n");
    root.write("b.go", "package p\n\nfunc B() {}\n");
    git(root.path(), &["add", "."]);
    let mut c = COMMIT.to_vec();
    c.extend_from_slice(&["-m", "x"]);
    git(root.path(), &c);
    root
}

fn out_mtime(root: &Path, name: &str) -> SystemTime {
    std::fs::metadata(root.join("graphify-out").join(name))
        .and_then(|m| m.modified())
        .unwrap_or_else(|e| panic!("stat {name}: {e}"))
}

/// Go `runOut`: (stdout, stderr, graph.json, report).
fn run_out(root: &Path, cmd: &str, extra: &[&str]) -> (String, String, String, String) {
    let mut a = vec![cmd, s(root)];
    a.extend_from_slice(extra);
    let r = ok_in(root, &a, &ENV);
    (
        r.stdout,
        r.stderr,
        read(&root.join("graphify-out/graph.json")),
        read(&root.join("graphify-out/GRAPH_REPORT.md")),
    )
}

#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_no_change_update_skips_writes() {
    let root = fp_root();
    let r = root.path();
    let (_, _, g0, r0) = run_out(r, "build", &[]);
    let (gt, rt) = (out_mtime(r, "graph.json"), out_mtime(r, "GRAPH_REPORT.md"));
    let (out1, _, g1, r1) = run_out(r, "update", &[]);
    assert!(
        g1 == g0
            && r1 == r0
            && out_mtime(r, "graph.json") == gt
            && out_mtime(r, "GRAPH_REPORT.md") == rt,
        "no-change update rewrote the outputs"
    );
    // A rewrite on the full path prints the same line.
    write(&r.join("graphify-out/GRAPH_REPORT.md"), &format!("{r0}\n"));
    let (out2, _, g2, r2) = run_out(r, "update", &[]);
    assert!(
        out_mtime(r, "graph.json") != gt,
        "edited report did not take the full path"
    );
    assert!(
        out2 == out1 && g2 == g0 && r2 == r0,
        "full path differs:\n{out1}{out2}"
    );
}

type Mutate = fn(&Path);

#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_changed_inputs_take_full_path() {
    let tests: [(&str, Mutate, &[&str]); 10] = [
        (
            "edit",
            |r| {
                write(
                    &r.join("b.go"),
                    "package p\n\nfunc B() { A() }\n\nfunc C() {}\n",
                )
            },
            &[],
        ),
        (
            "add",
            |r| write(&r.join("c.go"), "package p\n\nfunc C() { A() }\n"),
            &[],
        ),
        (
            "remove",
            |r| {
                let _ = std::fs::remove_file(r.join("b.go"));
            },
            &[],
        ),
        ("unclassified", |r| write(&r.join("x.zzz"), "?"), &[]),
        (
            "commit",
            |r| {
                let mut c = COMMIT.to_vec();
                c.extend_from_slice(&["--allow-empty", "-m", "y"]);
                git(r, &c);
            },
            &[],
        ),
        (
            "removed graph",
            |r| {
                let _ = std::fs::remove_file(r.join("graphify-out/graph.json"));
            },
            &[],
        ),
        (
            "edited graph",
            |r| {
                let f = r.join("graphify-out/graph.json");
                let mut b = std::fs::read(&f).unwrap_or_default();
                b.push(b'\n');
                let _ = std::fs::write(&f, b);
            },
            &[],
        ),
        ("no-cluster", |_| {}, &["--no-cluster"]),
        ("no-manifests", |_| {}, &["--no-manifests"]),
        (
            "edited manifest",
            |r| write(&r.join("go.mod"), "module example.com/p\n\ngo 1.21\n"),
            &[],
        ),
    ];
    for (name, mutate, extra) in tests {
        let root = fp_root();
        let r = root.path();
        ok_in(r, &["build", s(r)], &ENV);
        let old = out_mtime(r, "GRAPH_REPORT.md");
        mutate(r);
        let (_, _, g, rep) = run_out(r, "update", extra);
        assert!(
            out_mtime(r, "GRAPH_REPORT.md") != old,
            "{name}: update took the early path"
        );
        let (_, _, cg, cr) = run_out(r, "build", extra);
        assert!(
            g == cg && rep == cr,
            "{name}: update differs from a cold build"
        );
    }
}

// Also needs julia (graphify-rs-ihc.38) for the stall.
#[test]
// cli test (ihc.20): needs resolve
#[ignore = "graphify-rs-ihc.18"]
fn test_over_budget_update_repeats_warning() {
    let root = fp_root();
    let r = root.path();
    write(&r.join("s.jl"), STALL_JULIA);
    let budget = [ENV[0], ("GRAPHIFY_PARSE_BUDGET", "65536")];
    ok_in(r, &["build", s(r)], &budget);
    for i in 0..2 {
        let out = ok_in(r, &["update", s(r)], &budget);
        assert!(
            out.stderr.contains("parser made no progress"),
            "update {i}: stderr = {:?}, want the warning",
            out.stderr
        );
    }
}
