//! Port of the two `cmd/graphify/explain_path_test.go` tests over `explainLines`
//! (`TestExplainLinesCapsAndGroups`, `TestExplainLinesShortListUngrouped`). `explainLines` is
//! in `main.go` (here `cli::main::explain_lines`) but takes `[]query.Neighbor`, so the tests
//! moved from cli-core (graphify-rs-ihc.20) to this PR; the other six tests of the file are in
//! `crates/graphify/tests/cli_explain_path.rs`.

use crate::cli::main::{EXPLAIN_CONN_CAP, explain_lines};
use crate::query::Neighbor;

/// Go `query.Neighbor{Label: "n", Relation: "calls", Direction: dir, File: file}`.
fn nb(dir: &str, file: &str) -> Neighbor {
    Neighbor {
        label: "n".to_string(),
        relation: "calls".to_string(),
        direction: dir.to_string(),
        file: file.to_string(),
        ..Neighbor::default()
    }
}

/// Go `TestExplainLinesCapsAndGroups`: explain shows at most `explainConnCap` connections in
/// full and folds the rest into per-file counts.
#[test]
fn test_explain_lines_caps_and_groups() {
    let mut nbrs = Vec::new();
    for _ in 0..EXPLAIN_CONN_CAP + 5 {
        nbrs.push(nb("<-", "a.go"));
    }
    for _ in 0..3 {
        nbrs.push(nb("->", "b.go"));
    }
    let lines = explain_lines(&nbrs);
    // 20 full lines + summary + 2 grouped file lines.
    assert_eq!(
        lines.len(),
        EXPLAIN_CONN_CAP + 3,
        "lines = {} ({lines:?})",
        lines.len()
    );
    assert_eq!(
        lines[EXPLAIN_CONN_CAP], "  ... 8 more connections. Grouped by file:",
        "summary"
    );
    // Highest count first: 5 cut neighbours in a.go, then 3 in b.go.
    let first = &lines[EXPLAIN_CONN_CAP + 1];
    assert!(
        first.contains("a.go") && first.ends_with(" 5"),
        "first group = {first:?}, want a.go with 5"
    );
    let second = &lines[EXPLAIN_CONN_CAP + 2];
    assert!(
        second.contains("b.go") && second.ends_with(" 3"),
        "second group = {second:?}, want b.go with 3"
    );
}

/// Go `TestExplainLinesShortListUngrouped`: a node under the cap prints every connection with
/// no grouped tail.
#[test]
fn test_explain_lines_short_list_ungrouped() {
    let lines = explain_lines(&[nb("->", "a.go")]);
    assert_eq!(lines.len(), 1, "lines = {lines:?}, want 1 connection line");
    let got = explain_lines(&[]);
    assert_eq!(
        got,
        ["  (no connections)"],
        "explainLines(nil) = {got:?}, want (no connections)"
    );
}
