//! Port of graphify-go `internal/idutil/idutil_test.go` (FuzzCleanASCII as its seed table), and
//! the plan's `make_id_cgj_run` (Review Focus 4).

use gostd::xtext::cases::fold;

use crate::idutil::idutil::{clean_ascii, clean_unicode};
use crate::idutil::{make_id, normalize_id};

const MAKE_ID_CASES: &[(&[&str], &str)] = &[
    (&["main.go"], "main_go"),
    (&["server", "HandleRequest"], "server_handlerequest"),
    (&["  foo  ", "Bar()"], "foo_bar"),
    (&["a__b"], "a_b"),
    (&["_._leading"], "leading"),
    (&["café"], "café"), // NFKC keeps the composed letter, fold lowercases
    (&["", "x"], "x"),
];

#[test]
#[ignore = "graphify-rs-ihc.4"]
fn test_make_id() {
    for &(parts, want) in MAKE_ID_CASES {
        assert_eq!(make_id(parts), want, "make_id({parts:?})");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.4"]
fn test_normalize_id_matches_make_id() {
    // An ID produced by MakeID must be a fixed point of NormalizeID, so edge endpoints
    // reconcile to the same key the node was stored under.
    let id = make_id(&["pkg", "SomeType"]);
    assert_eq!(normalize_id(&id), id, "normalize_id({id:?}) not stable");
    assert_eq!(normalize_id("Some-Type"), "some_type");
}

#[test]
#[ignore = "graphify-rs-ihc.4"]
fn test_clean_is_idempotent_and_caseless_stable() {
    // Casefolding expands some characters into a base letter plus a combining mark; folding
    // after the non-word filter left that mark unfiltered, so the result was not a fixed point
    // of NormalizeID (upstream's ghost-node class).
    for input in [
        "\u{130}slemYap", // Turkish dotted capital I -> "i" + U+0307
        "\u{1fb4}",       // Greek alpha with oxia and ypogegrammeni
        "\u{345}\u{301}", // bare ypogegrammeni + combining acute
        "pkg.SomeType",   // ASCII control
        "café",
    ] {
        let id = make_id(&[input]);
        assert_eq!(normalize_id(&id), id, "normalize_id(make_id({input:?}))");
        assert_eq!(make_id(&[&fold(input)]), id, "make_id(fold({input:?}))");
    }
}

fn check_ascii(s: &str) {
    let got =
        clean_ascii(s).unwrap_or_else(|| panic!("clean_ascii({s:?}) refused an ASCII string"));
    assert_eq!(
        got,
        clean_unicode(s),
        "clean_ascii({s:?}) differs from clean_unicode"
    );
}

/// The ASCII fast path must give exactly what the general path gives.
#[test]
#[ignore = "graphify-rs-ihc.4"]
fn test_clean_ascii_matches_unicode() {
    // Every string of up to 3 bytes over an alphabet with one of each class, then every single
    // byte and every pair around a letter.
    const ALPHABET: [u8; 10] = [b'A', b'z', b'0', b'_', b'.', b' ', 0, 0x7f, b'/', b'\n'];
    let s = |b: &[u8]| String::from_utf8(b.to_vec()).unwrap();
    check_ascii("");
    for a in ALPHABET {
        check_ascii(&s(&[a]));
        for b in ALPHABET {
            check_ascii(&s(&[a, b]));
            for c in ALPHABET {
                check_ascii(&s(&[a, b, c]));
            }
        }
    }
    for c in 0u8..0x80 {
        check_ascii(&s(&[c]));
        check_ascii(&s(&[b'a', c, b'B']));
        check_ascii(&s(&[c, b'x', c, c, b'Y', c]));
    }
    check_ascii("k8s.io/kubernetes/pkg/Kubelet.(*Server).ServeHTTP");
    assert_eq!(
        clean_ascii("café"),
        None,
        "clean_ascii accepted a non-ASCII string"
    );
}

/// The `f.Add` seeds of Go `FuzzCleanASCII`.
const FUZZ_CLEAN_ASCII_SEEDS: &[&str] = &["pkg__Some-Type.method()"];

/// FuzzCleanASCII, its seed corpus: where the fast path accepts, it equals the general path.
#[test]
#[ignore = "graphify-rs-ihc.4"]
fn fuzz_clean_ascii_seeds() {
    for &s in FUZZ_CLEAN_ASCII_SEEDS {
        if let Some(got) = clean_ascii(s) {
            assert_eq!(got, clean_unicode(s), "clean_ascii({s:?})");
        }
    }
}

/// Plan Review Focus 4: x/text's stream-safe rule puts U+034F (CGJ, a starter) before the 31st
/// non-starter in a row, so the final U+0301 cannot compose with the base letter. Input: "a",
/// 30 x U+0334 (ccc 1, no composite with "a"), U+0301. With the CGJ the letter stays "a";
/// without it U+0301 composes to "á". The marks and the CGJ are then one "_" run.
#[test]
#[ignore = "graphify-rs-ihc.4"]
fn make_id_cgj_run() {
    let part = format!("a{}\u{301}", "\u{334}".repeat(30));
    assert_eq!(make_id(&["x", &part]), "x_a");
    // 29 overlays leave 30 non-starters: no CGJ, so U+0301 composes with "a".
    let part = format!("a{}\u{301}", "\u{334}".repeat(29));
    assert_eq!(make_id(&["x", &part]), "x_\u{e1}");
}
