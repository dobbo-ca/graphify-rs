//! Ported from `unicode/norm/composition_test.go` (golang.org/x/text v0.27.0).
//!
//! Go Test funcs:
//! - TestFlush: not ported: tests x/text internals not reached by graphify-go
//! - TestInsert: ported (NFD rows, as NFKD: no input has a compatibility mapping)
//! - TestDecomposition: ported (NFD rows without compatibility mappings, as NFKD; NFKD rows verbatim)
//! - TestComposition: ported (NFC rows, as NFKC: no input has a compatibility mapping)
//!
//! The Go tests drive `reorderBuffer` directly with NFD/NFKD/NFC. Only the string-level
//! result is observable through `nfkd`/`nfkc`. NFKD(x) == NFD(x) and NFKC(x) == NFC(x)
//! whenever x has no compatibility decomposition, so those rows are kept.
//!
//! Rows kept / left out:
//! - insertTests (NFD): 8 kept, 0 left out
//! - decompositionNFDTest: 3 kept (U+00C0, U+AC00, the Hangul/ẻ string), 2 left out
//!   (U+01C4, U+320E: NFD leaves them unchanged but NFKD decomposes them; the
//!   NFKD table covers those inputs)
//! - decompositionNFKDTest: 4 kept, 0 left out
//! - compositionTest (NFC): 7 kept, 0 left out

use gostd::xtext::norm::{nfkc, nfkd};

fn run(name: &str, f: fn(&str) -> String, tests: &[(&str, &str)]) {
    for (i, (input, want)) in tests.iter().enumerate() {
        let was = f(input);
        assert_eq!(&was, want, "{name}:{i}: input {input:?}");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.4"]
fn test_insert() {
    let tests = [
        ("a", "a"),
        ("\u{300}", "\u{300}"),
        ("\u{300}\u{316}", "\u{316}\u{300}"), // CCC(0x300)==230; CCC(0x316)==220
        ("\u{316}\u{300}", "\u{316}\u{300}"),
        ("\u{41}\u{316}\u{300}", "\u{41}\u{316}\u{300}"),
        ("\u{41}\u{300}\u{316}", "\u{41}\u{316}\u{300}"),
        ("\u{300}\u{316}\u{41}", "\u{316}\u{300}\u{41}"),
        ("\u{41}\u{300}\u{40}\u{316}", "\u{41}\u{300}\u{40}\u{316}"),
    ];
    run("TestInsert", nfkd, &tests);
}

#[test]
#[ignore = "graphify-rs-aa6.4"]
fn test_decomposition() {
    // decompositionNFDTest rows without compatibility mappings, run as NFKD.
    let nfd_rows = [
        ("\u{C0}", "\u{41}\u{300}"),
        ("\u{AC00}", "\u{1100}\u{1161}"),
        // not ported: {0x01C4} -> {0x01C4}, {0x320E} -> {0x320E} (NFD-only: NFKD decomposes them)
        (
            "\u{C74C}\u{1EBB}\u{ACFC}",
            "\u{110B}\u{1173}\u{11B7}\u{65}\u{309}\u{1100}\u{116A}",
        ),
    ];
    run("TestDecompositionNFD", nfkd, &nfd_rows);

    let nfkd_rows = [
        ("\u{C0}", "\u{41}\u{300}"),
        ("\u{AC00}", "\u{1100}\u{1161}"),
        ("\u{01C4}", "\u{44}\u{5A}\u{030C}"),
        ("\u{320E}", "\u{28}\u{1100}\u{1161}\u{29}"),
    ];
    run("TestDecompositionNFKD", nfkd, &nfkd_rows);
}

#[test]
#[ignore = "graphify-rs-aa6.4"]
fn test_composition() {
    let tests = [
        ("\u{41}\u{300}", "\u{C0}"),
        ("\u{41}\u{316}", "\u{41}\u{316}"),
        ("\u{41}\u{300}\u{35D}", "\u{C0}\u{35D}"),
        ("\u{41}\u{316}\u{300}", "\u{C0}\u{316}"),
        // blocking starter
        ("\u{41}\u{316}\u{40}\u{300}", "\u{41}\u{316}\u{40}\u{300}"),
        ("\u{1100}\u{1161}", "\u{AC00}"),
        // parenthesized Hangul, alternate between ASCII and Hangul.
        ("\u{28}\u{1100}\u{1161}\u{29}", "\u{28}\u{AC00}\u{29}"),
    ];
    run("TestComposition", nfkc, &tests);
}
