//! Port of `affected_stdin_test.go` (1 test).

use super::read_path_list;

// `gh pr diff <n> --name-only | graphify affected -` feeds a path per line; blank and
// whitespace-padded lines must not become bogus seeds.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_read_path_list() {
    let got = read_path_list("a/b.go\n\n  c.go  \n\n".as_bytes());
    assert_eq!(got, ["a/b.go", "c.go"], "read_path_list");
    let got = read_path_list("".as_bytes());
    assert!(got.is_empty(), "empty stdin = {got:?}, want none");
}
