//! `child_by_kind` and `first_named` (Go `lua.go:223-240`), the shared tree helpers.

use super::{child_by_kind, first_named};

/// In `[1, "a"]` the array's children are `[`, `number`, `,`, `string`, `]`: the first named
/// child is the number, and `child_by_kind` finds a kind past it or nothing.
#[test]
fn child_by_kind_and_first_named() {
    let got = crate::ts::parse(b"[1, \"a\"]", gositter_grammars::json::language(), |root| {
        let array = root.child(0).expect("array");
        (
            first_named(array).map(|n| n.kind()),
            child_by_kind(array, "string").map(|n| n.kind()),
            child_by_kind(array, ",").map(|n| n.start_byte()),
            child_by_kind(array, "object").is_none(),
            first_named(child_by_kind(array, "[").expect("[")).is_none(),
        )
    });
    assert_eq!(
        got,
        Some((Some("number"), Some("string"), Some(2), true, true))
    );
}
