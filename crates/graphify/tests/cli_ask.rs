//! Port of graphify-go `cmd/graphify/ask_test.go` `TestCmdAskGraphTraversal`: it changes the
//! working directory, so it runs the binary in that directory.

mod common;

use common::{TempDir, run_in, s, write};

// A user-supplied --graph path cannot escape the graphify-out directory to read arbitrary
// on-disk JSON.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_cmd_ask_graph_traversal() {
    let dir = TempDir::new();
    std::fs::create_dir_all(dir.join("graphify-out")).expect("mkdir graphify-out");
    let secret = dir.write("secrets.json", r#"{"nodes":[],"links":[]}"#);
    let cwd = dir.path();

    let in_cwd = cwd.join("secrets.json");
    for p in [s(&secret), "../secrets.json", s(&in_cwd)] {
        let r = run_in(cwd, &["ask", "q", "--graph", p], &[]);
        assert_ne!(
            r.code,
            Some(0),
            "ask --graph {p:?} = ok, want containment error"
        );
    }

    // A path inside graphify-out must clear the containment guard.
    let inside = cwd.join("graphify-out/alt.json");
    write(&inside, r#"{"nodes":[],"links":[]}"#);
    let r = run_in(cwd, &["ask", "q", "--graph", s(&inside)], &[]);
    assert_eq!(
        r.code,
        Some(0),
        "ask --graph {inside:?} (inside graphify-out): {}",
        r.stderr
    );
}
