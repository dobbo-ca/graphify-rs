//! Go source: cases/fold_test.go and cases/context_test.go (golang.org/x/text v0.27.0)
//!
//! fold_test.go:
//! - TestFold: ported (the testtext.AllocsPerRun / Transform check left out: Transformer not ported)
//!
//! context_test.go:
//! - TestCaseProperties: not ported: tests x/text internals not reached by graphify-go
//! - TestMapping: not ported: Lower/Upper/Title casers not called by graphify-go
//! - TestFoldData: ported (see the comment on `test_fold_data` for how it differs)
//! - TestCCC: not ported: tests x/text internals not reached by graphify-go
//! - TestWordBreaks: not ported: needs the Title caser (word-break title casing), which graphify-go doesn't call
//! - TestContext: not ported: tests x/text internals not reached by graphify-go
//!
//! Plus `fold_specials`, which is not from Go.
//!
//! Table rows kept: 5 of 5 (foldTestCases). Rows left out: 0.

use gostd::unicode::to_lower;
use gostd::xtext::cases::fold;
use std::collections::HashMap;

const FOLD_TEST_CASES: [&str; 5] = [
    "βß\u{13f8}",
    "ab\u{13fc}\u{ab7a}ꭰ",
    "aﬃﬄaﬆ",
    "Iİiı\u{345}",
    "µµΜΜςσΣΣ",
];

/// Maps each code point to its C/F folding in `$GOSTD_UCD/15.0.0/CaseFolding.txt`.
fn case_folding() -> HashMap<char, String> {
    let dir = std::env::var("GOSTD_UCD")
        .expect("GOSTD_UCD is not set: the test stage bind-mounts the ucd stage at /ucd");
    let path = std::path::Path::new(&dir).join("15.0.0/CaseFolding.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(';').map(str::trim).collect();
        if fields[1] != "C" && fields[1] != "F" {
            continue;
        }
        let cp = char::from_u32(u32::from_str_radix(fields[0], 16).unwrap()).unwrap();
        let mapping: String = fields[2]
            .split_whitespace()
            .map(|h| char::from_u32(u32::from_str_radix(h, 16).unwrap()).unwrap())
            .collect();
        map.insert(cp, mapping);
    }
    map
}

#[test]
#[ignore = "graphify-rs-aa6.4"]
fn test_fold() {
    // Go's runeFoldData: the C/F folding of r, else ToLower(r).
    let fold_map = case_folding();
    for tc in FOLD_TEST_CASES {
        let mut want = String::new();
        for r in tc.chars() {
            match fold_map.get(&r) {
                Some(m) => want.push_str(m),
                None => want.push(to_lower(r)),
            }
        }
        let got = fold(tc);
        assert_eq!(got, want, "FullFold({tc})");
    }
}

// Go's TestFoldData runs the internal foldFull context function over every assigned code point and
// compares with runeFoldData (CaseFolding C/F, else ToLower). Here it is expressed through the
// public API: for each C or F line of CaseFolding.txt (15.0.0), fold(code point) == mapping. The
// ToLower fallback for code points without a C/F line, and the assigned-range loop, are not covered.
#[test]
#[ignore = "graphify-rs-aa6.4"]
fn test_fold_data() {
    let fold_map = case_folding();
    assert!(!fold_map.is_empty(), "CaseFolding.txt had no C/F lines");
    for (r, want) in fold_map {
        let got = fold(&r.to_string());
        assert_eq!(got, want, "full:{r:?} (U+{:04X})", r as u32);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.4"]
fn fold_specials() {
    assert_eq!(fold("ß"), "ss");
    assert_eq!(fold("İ"), "i\u{307}");
    assert_eq!(fold("ﬃ"), "ffi");
    assert_eq!(fold("\u{212A}"), "k");
    assert_eq!(fold("ΣΑΣ"), "σασ");
    assert_eq!(fold(""), "");
}
