//! graphify-go `internal/extract/cpp_test.go`. Not ported here: `TestCppHeaderRoutesToCppExtractor`
//! moved to the extract-core tests (ihc.16): it tests `isCppHeader`, `extract.go:316`.

use std::collections::{BTreeMap, BTreeSet};

use super::{blank_cpp_export_macros, extract_cpp};
use crate::extract::{file_from_bytes, resolve};
use crate::testutil::testdata;

#[test]
#[ignore = "graphify-rs-ihc.30"]
fn test_extract_cpp() {
    crate::extract::kinds::register_kinds();
    let root = testdata().join("extract").join("cppproj");
    let files = ["util/math.cpp", "web/server.cpp"];

    // C++ dispatch is wired centrally; call extractCpp directly here so the test
    // is self-contained.
    let mut results = Vec::new();
    for f in files {
        let src = std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("ReadFile({f}): {e}"));
        results.push(extract_cpp(f, &src).expect("parse budget"));
    }
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&results, &files, workers);

    let labels: BTreeSet<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    let id2label: BTreeMap<&str, &str> = ext
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str()))
        .collect();
    for want in [
        "math.cpp",
        "server.cpp",
        "add()",
        "Server",
        "Server.start()",
        "boot()",
    ] {
        assert!(labels.contains(want), "missing node label {want:?}");
    }

    let has = |src: &str, rel: &str, tgt: &str| {
        ext.edges.iter().any(|e| {
            e.relation.as_str() == rel
                && id2label.get(e.source.as_str()) == Some(&src)
                && id2label.get(e.target.as_str()) == Some(&tgt)
        })
    };
    // Inline method scoped under its class, with a contains edge.
    assert!(
        has("Server", "contains", "Server.start()"),
        "expected Server --contains--> Server.start"
    );
    // Same-file call: Server.start -> boot.
    assert!(
        has("Server.start()", "calls", "boot()"),
        "expected Server.start --calls--> boot"
    );
    // Cross-file call: boot -> add (unique global definition).
    assert!(
        has("boot()", "calls", "add()"),
        "expected cross-file boot --calls--> add"
    );

    // External import edge for <string>.
    let imports = ext
        .edges
        .iter()
        .filter(|e| e.relation.as_str() == "imports")
        .count();
    assert!(imports > 0, "no external import edges (expected string)");
}

// An export macro between `class`/`struct` and the name must not hide the type.
#[test]
#[ignore = "graphify-rs-ihc.30"]
fn test_cpp_export_macro_class() {
    crate::extract::kinds::register_kinds();
    let src = "class Base {};\n".to_string()
        + "class Q_CORE_EXPORT\nWidget : public Base { public: void show() {} };\n"
        + "struct API Thing { int x; };\n"
        + "class MY_WIDGET final : public Base {};\n"
        + "class MACRO OTHER final : public Base {};\n"
        + "void F() {\n  for (class API v : items) {}\n  class API w{1};\n}\n";
    let out = blank_cpp_export_macros(src.as_bytes());
    let out_s = std::str::from_utf8(&out).expect("blanking keeps ASCII input ASCII");
    assert!(
        out.len() == src.len()
            && out_s.contains("class              \nWidget")
            && out_s.contains("(class API v :")
            && out_s.contains("class API w{1}"),
        "blanking changed offsets or touched a variable:\n{out_s}"
    );

    let res = file_from_bytes("w.h", src.as_bytes());
    let labels: BTreeMap<&str, &str> = res
        .nodes
        .iter()
        .map(|n| (n.label.as_str(), n.source_location.as_str()))
        .collect();
    for want in ["Widget", "Widget.show()", "Thing", "MY_WIDGET", "OTHER"] {
        assert!(
            labels.contains_key(want),
            "missing node {want:?} in {labels:?}"
        );
    }
    assert_eq!(labels.get("Thing"), Some(&"L4"), "Thing at wrong line");
    let inherits = res
        .type_refs
        .iter()
        .any(|r| r.from_id == "w_h_widget" && r.name == "Base" && r.relation == "inherits");
    assert!(
        inherits,
        "no Widget inherits Base ref in {:?}",
        res.type_refs
    );
}

// The keyword scan must blank exactly what one unanchored regexp pass did.
// (Go draws the inputs from math/rand seed 1; this draws them from gostd's PCG with seed 1. The
// test is a property over the same token set, so the source does not matter. Go's
// `strings.TrimSpace` is `trim_ascii` here: no token holds a non-ASCII space.)
#[test]
#[ignore = "graphify-rs-ihc.30"]
fn test_cpp_export_macro_scan_matches_regexp() {
    let re = gostd::regexp::Regexp::must_compile(
        r"\b(?:class|struct)\s+((?:[A-Z][A-Z0-9_]*\s+)+)([A-Za-z_]\w*(?:\s+final)?\s*)([:{])",
    );
    let reference = |src: &[u8]| -> Vec<u8> {
        let mut out = src.to_vec();
        for m in re.find_all_submatch_index(src, -1) {
            let g = |i: usize| m[i].expect("group");
            let (m0, (m2, m3), (m4, m5), (m6, m7)) = (g(0).0, g(1), g(2), g(3));
            if src[..m0].trim_ascii().ends_with(b"(") {
                continue;
            }
            let rest = src[m7..].trim_ascii();
            if src[m6] == b'{' && !rest.is_empty() && b"0123456789\"'-".contains(&rest[0]) {
                continue;
            }
            let mut end = m3;
            if src[m4..m5].trim_ascii() == b"final" {
                let t = src[m2..m3].trim_ascii();
                end = m2
                    + t.iter()
                        .rposition(|c| b" \t\r\n".contains(c))
                        .expect("space")
                    + 1;
            }
            for c in &mut out[m2..end] {
                if *c != b'\r' && *c != b'\n' {
                    *c = b' ';
                }
            }
        }
        out
    };
    let toks: [&[u8]; 22] = [
        b"class ",
        b"struct\n",
        b"class",
        b" ",
        b"\n",
        b"\t",
        b"API ",
        b"A_B2 ",
        b"API",
        b"Foo",
        b"Foo ",
        b"final ",
        b":",
        b"{",
        b" {",
        b"(",
        b"1",
        b"x",
        b"_",
        "\u{e9}".as_bytes(),
        b"\xff",
        b";",
    ];
    let mut rng = gostd::pcg::Rand::new(gostd::pcg::PcgSource::new(1));
    let mut blanked = 0;
    for _ in 0..200_000 {
        let mut src = Vec::new();
        let mut n = rng.intn(14);
        while n >= 0 {
            src.extend_from_slice(toks[rng.intn(toks.len() as i64) as usize]);
            n -= 1;
        }
        let (got, want) = (blank_cpp_export_macros(&src), reference(&src));
        assert!(got == want, "{src:?}: got {got:?}, want {want:?}");
        if got != src {
            blanked += 1;
        }
    }
    assert!(
        blanked >= 100,
        "only {blanked} inputs blanked: the generator no longer reaches the match"
    );
}
