//! Port of graphify-go `cmd/graphify/brokenpipe_test.go` `TestQueryExitsZeroOnBrokenPipe`, and
//! upstream graphify v0.9.107 `test_cli_broken_pipe.py` (2 tests): a reader that closes the pipe
//! early must leave graphify exiting 0, not dying of SIGPIPE (141) (plan hazard "EPIPE -> exit 0").
#![cfg(unix)]

mod common;

use std::io::{BufRead, BufReader};
use std::process::Stdio;

use common::{TempDir, graphify, write};

/// Go `brokenPipeGraph`: 8 nodes that all match `query Node`, so the CLI has output to write.
const BROKEN_PIPE_GRAPH: &str = concat!(
    r#"{"directed":true,"multigraph":false,"graph":{},"nodes":["#,
    r#"{"id":"n0","label":"Node0()","file_type":"code","source_file":"f0.go","source_location":"L1","norm_label":"node0()"},"#,
    r#"{"id":"n1","label":"Node1()","file_type":"code","source_file":"f1.go","source_location":"L1","norm_label":"node1()"},"#,
    r#"{"id":"n2","label":"Node2()","file_type":"code","source_file":"f2.go","source_location":"L1","norm_label":"node2()"},"#,
    r#"{"id":"n3","label":"Node3()","file_type":"code","source_file":"f3.go","source_location":"L1","norm_label":"node3()"},"#,
    r#"{"id":"n4","label":"Node4()","file_type":"code","source_file":"f4.go","source_location":"L1","norm_label":"node4()"},"#,
    r#"{"id":"n5","label":"Node5()","file_type":"code","source_file":"f5.go","source_location":"L1","norm_label":"node5()"},"#,
    r#"{"id":"n6","label":"Node6()","file_type":"code","source_file":"f6.go","source_location":"L1","norm_label":"node6()"},"#,
    r#"{"id":"n7","label":"Node7()","file_type":"code","source_file":"f7.go","source_location":"L1","norm_label":"node7()"}"#,
    r#"],"links":[]}"#,
);

/// Runs `graphify <args>` in `cwd` with stdout piped; `read_lines` lines are read before the
/// read end is closed (0: closed before the child writes). Returns the exit code.
fn run_closing_early(cwd: &std::path::Path, args: &[&str], read_lines: usize) -> Option<i32> {
    let mut child = graphify(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn graphify");
    let stdout = child.stdout.take().expect("piped stdout");
    let mut r = BufReader::new(stdout);
    for _ in 0..read_lines {
        let mut line = String::new();
        let _ = r.read_line(&mut line);
    }
    drop(r);
    child.wait().expect("wait for graphify").code()
}

// Go closes the read end before the child writes, so its stdout writes hit a broken pipe.
#[test]
fn test_query_exits_zero_on_broken_pipe() {
    let work = TempDir::new();
    write(&work.join("graphify-out/graph.json"), BROKEN_PIPE_GRAPH);
    let code = run_closing_early(work.path(), &["query", "Node"], 0);
    assert_ne!(
        code,
        Some(141),
        "query died with SIGPIPE (exit 141) on early pipe close"
    );
    assert_eq!(code, Some(0), "query exit code, want 0 on broken pipe");
}

// Upstream: `graphify --help | head -n1` exits 0, not 255.
#[test]
fn test_help_survives_reader_closing_pipe_early() {
    let dir = TempDir::new();
    assert_eq!(run_closing_early(dir.path(), &["--help"], 1), Some(0));
}

// Upstream: a short output whose reader reads nothing exits 0.
#[test]
fn test_small_buffered_output_survives_reader_that_reads_nothing() {
    let dir = TempDir::new();
    assert_eq!(run_closing_early(dir.path(), &["--version"], 0), Some(0));
}
