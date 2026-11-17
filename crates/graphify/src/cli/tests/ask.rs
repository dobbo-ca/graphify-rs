//! Port of `ask_test.go`: `TestCmdAskNegativeBudget` here; `TestCmdAskGraphTraversal` changes
//! the working directory, so it runs the binary (`crates/graphify/tests/cli_ask.rs`).

use super::cmd_ask;
use crate::cli::maintenance::maint_util::args;

// A non-positive --budget is rejected at parse time rather than panicking the engine on a
// negative slice bound.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_cmd_ask_negative_budget() {
    const CASES: [&[&str]; 4] = [
        &["q", "--budget", "-1"],
        &["q", "--budget=-1"],
        &["q", "--budget", "0"],
        &["q", "--budget=0"],
    ];
    for a in CASES {
        let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
        match cmd_ask(&args(a), &mut out, &mut err) {
            Err(e) if e.to_string().contains("positive integer") => {}
            r => panic!("cmd_ask({a:?}) = {r:?}, want positive-integer error"),
        }
    }
}
