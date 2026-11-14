//! graphify-go `internal/extract/extract_test.go`, and the plan's Task 15 added tests
//! `to_utf8_cases` and `non_utf8_file_name`.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use crate::extract::extract::to_utf8;
use crate::extract::tests::util::extract_bytes;
use crate::extract::{file, kinds, resolve};
use crate::model::append_json_string;
use crate::testutil::testdata;

#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_extract_and_resolve() {
    kinds::register_kinds();
    let root = testdata().join("extract").join("proj");
    let files = ["util/math.go", "web/server.ts"];

    let results: Vec<_> = files
        .iter()
        .map(|f| file(&root, f).unwrap_or_else(|e| panic!("File({f}): {e}")))
        .collect();
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    for want in [
        "math.go",
        "server.ts",
        "Add()",
        "Calc",
        "Calc.Sum()",
        "Server",
        "Server.start()",
        "boot()",
    ] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }

    // Relation presence checks.
    let mut rels: BTreeMap<&str, usize> = BTreeMap::new();
    for e in &ext.edges {
        *rels.entry(e.relation.as_str()).or_default() += 1;
    }
    assert!(
        rels.get("contains").copied().unwrap_or(0) != 0,
        "no contains edges"
    );
    assert!(
        rels.get("calls").copied().unwrap_or(0) != 0,
        "no calls edges (expected Sum->Add and boot->start)"
    );
    assert!(
        rels.get("imports").copied().unwrap_or(0) != 0,
        "no external import edges (expected express)"
    );

    // Specific: Sum calls Add (same-file resolution).
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    let found = ext.edges.iter().any(|e| {
        e.relation.as_str() == "calls"
            && id2label.get(e.source.as_str()) == Some(&"Calc.Sum()")
            && id2label.get(e.target.as_str()) == Some(&"Add()")
    });
    assert!(found, "expected Calc.Sum --calls--> Add");
}

#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_file_from_bytes_non_utf8() {
    let mut utf16 = vec![0xFF, 0xFE];
    for r in "def hello():\n    pass\n".chars() {
        utf16.push(r as u8);
        utf16.push(0);
    }
    let cases: [(&str, Vec<u8>); 2] = [
        ("hello()", utf16),
        ("café()", b"def caf\xe9():\n    pass\n".to_vec()),
    ];
    for (want, src) in &cases {
        let found = extract_bytes("a.py", src)
            .nodes
            .iter()
            .any(|n| n.label == *want);
        assert!(found, "missing label {want:?}");
    }
}

/// `toUTF8` (`extract.go:625`): valid UTF-8 kept; UTF-16 with a BOM decoded (an unpaired
/// surrogate is U+FFFD, an odd trailing byte is dropped); anything else is latin-1.
#[test]
fn to_utf8_cases() {
    const CASES: [(&str, &[u8], &str); 6] = [
        (
            "valid kept",
            "h\u{e9}llo \u{1f600}".as_bytes(),
            "h\u{e9}llo \u{1f600}",
        ),
        (
            "utf-16le, unpaired high surrogate",
            // FF FE, "a", D800 (unpaired, followed by "b"), "b"
            b"\xff\xfe\x61\x00\x00\xd8\x62\x00",
            "a\u{fffd}b",
        ),
        (
            "utf-16le, unpaired low surrogate",
            // FF FE, DC00 alone
            b"\xff\xfe\x00\xdc",
            "\u{fffd}",
        ),
        (
            "utf-16le, odd trailing byte dropped",
            b"\xff\xfe\x61\x00\x62\x00\x63",
            "ab",
        ),
        ("utf-16be", b"\xfe\xff\x00\x61\x00\x62", "ab"),
        ("latin-1", b"caf\xe9", "caf\u{e9}"),
    ];
    for (name, input, want) in CASES {
        let got = to_utf8(input);
        assert_eq!(&*got, want.as_bytes(), "{name}");
    }
    assert!(
        matches!(to_utf8("h\u{e9}llo".as_bytes()), Cow::Borrowed(_)),
        "valid UTF-8 is returned as is, not copied"
    );
}

/// Review Focus 1: `file_from_bytes` for the rel `"\u{FFFD}.py"` that plan R3 makes of
/// `b"\xff.py"` gives a file node whose `source_file` is written as `EF BF BD` `.py`, the bytes
/// Go 1.27 writes for the bad byte.
#[test]
#[ignore = "graphify-rs-ihc.25"]
fn non_utf8_file_name() {
    let res = extract_bytes("\u{fffd}.py", b"def f():\n    pass\n");
    let node = res
        .nodes
        .first()
        .expect("the file node comes first, even for an odd name");
    assert_eq!(node.label, "\u{fffd}.py", "file node label");
    let mut json = Vec::new();
    append_json_string(&mut json, &node.source_file);
    assert_eq!(
        json,
        b"\"\xef\xbf\xbd.py\"".to_vec(),
        "source_file JSON bytes"
    );
}
