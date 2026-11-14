//! graphify-go `internal/extract/budget_test.go`. Go's tests set the package variable `ts.Budget`
//! (and restore it); here `ts::with_budget` sets it for this thread while a closure runs.

use crate::extract::file_from_bytes;
use crate::extract::tests::util::extract_bytes;
use crate::ts::with_budget;

/// Go `stallJulia`: makes the julia parser retry one byte without end.
const STALL_JULIA: &str = "@testset \"t0\" begin\n    @testset \"t1\" begin\n        l2";

#[test]
#[ignore = "graphify-rs-ihc.38"]
fn test_over_budget_keeps_file_node() {
    let r = extract_bytes("a.jl", STALL_JULIA.as_bytes());
    assert!(
        r.over_budget
            && r.nodes.len() == 1
            && r.nodes[0].source_file == "a.jl"
            && r.edges.is_empty(),
        "over_budget {}, nodes {:?}, edges {}; want a stopped parse with the file node only",
        r.over_budget,
        r.nodes,
        r.edges.len()
    );
    // The pool still gives working parsers.
    let r = extract_bytes("a.go", b"package p\n\nfunc A() {}\n");
    assert!(
        !r.over_budget && r.nodes.len() == 2,
        "after a stopped parse: over_budget {}, {} nodes, want 2",
        r.over_budget,
        r.nodes.len()
    );
}

#[test]
fn test_budget_setting() {
    // The if with an init clause takes three steps at one byte.
    let src = b"package p\n\nfunc A(m map[string]int) {\n\tif v, ok := m[\"a\"]; ok {\n\t\tB(v)\n\t}\n}\n\nfunc B(int) {}\n";
    crate::extract::kinds::register_kinds();
    let r = with_budget(2, || file_from_bytes("a.go", src));
    assert!(
        r.over_budget && r.nodes.len() == 1,
        "budget 2: over_budget {}, {} nodes, want a skip with 1",
        r.over_budget,
        r.nodes.len()
    );
    let r = with_budget(0, || file_from_bytes("a.go", src));
    assert!(
        !r.over_budget && r.nodes.len() == 3,
        "budget 0: over_budget {}, {} nodes, want 3",
        r.over_budget,
        r.nodes.len()
    );
}

#[test]
#[ignore = "graphify-rs-ihc.38"]
fn test_budget_scales_with_size() {
    crate::extract::kinds::register_kinds();
    // About 98,000 steps in a row at the last byte, and no endless retry.
    let kt = "<".repeat(12000);
    let r = with_budget(65536, || file_from_bytes("a.kt", kt.as_bytes()));
    assert!(
        !r.over_budget,
        "12000 bytes of kotlin \"<\": stopped, want a parse"
    );
    let r = with_budget(16384, || file_from_bytes("a.kt", kt.as_bytes()));
    assert!(
        r.over_budget,
        "12000 bytes of kotlin \"<\" at a quarter of the limit: parsed, want a stop"
    );
    // A file of 64 KB or more gets the plain limit.
    let big = format!("{}{STALL_JULIA}", "x = 1\n".repeat(1 << 16));
    let r = with_budget(65536, || file_from_bytes("a.jl", big.as_bytes()));
    assert!(
        r.over_budget && r.nodes.len() == 1,
        "stall after 384 KB: over_budget {}, {} nodes, want a stopped parse",
        r.over_budget,
        r.nodes.len()
    );
}
