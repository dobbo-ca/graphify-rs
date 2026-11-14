//! graphify-go `internal/extract/cpp_test.go` `TestCppHeaderRoutesToCppExtractor`: it tests
//! `isCppHeader` and the `.h` routing of `fileFromBytes` (`extract.go:316`, `:270-274`), so it
//! is an extract-core test (coordinator, 2026-11-13: a Go test goes to the owner of the code it
//! tests). It parses with the c and cpp grammars, so it waits for the later of their beads.

use std::collections::BTreeSet;

use crate::extract::extract::is_cpp_header;
use crate::extract::file_from_bytes;

fn labels(src: &str) -> BTreeSet<String> {
    file_from_bytes("w.h", src.as_bytes())
        .nodes
        .into_iter()
        .map(|n| n.label)
        .collect()
}

#[test]
#[ignore = "graphify-rs-ihc.30"]
fn test_cpp_header_routes_to_cpp_extractor() {
    let got =
        labels("class Widget { public: void show(); virtual int area() const = 0; int count; };\n");
    assert!(
        got.contains("Widget") && !got.contains("Widget()"),
        "C++ header: want class node Widget, got {got:?}"
    );

    // A plain C header keeps the C extractor's output.
    let plain = "#include <stdio.h>\nstruct point { int x; };\nint add(int a, int b);\n";
    let want: BTreeSet<String> = crate::extract::c::extract_c("w.h", plain.as_bytes())
        .expect("no parse budget stop")
        .nodes
        .into_iter()
        .map(|n| n.label)
        .collect();
    let got = labels(plain);
    assert!(
        got.len() == want.len() && !is_cpp_header(plain.as_bytes()),
        "plain C header: got {got:?}, want {want:?}"
    );
}
