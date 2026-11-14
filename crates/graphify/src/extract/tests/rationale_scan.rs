//! graphify-go `internal/extract/rationale_scan_test.go`. Go calls `(*builder).jsRationale`,
//! `pyRationale` and `addDocRef`, which live in `javascript.rs` and `python.rs` (other owners,
//! not stubbed here). The three functions below stand in for them with `todo!` bodies; the
//! javascript (ihc.26) and python (ihc.25) PRs replace them with their `Builder` methods.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use gositter::Node;
use gostd::regexp::Regexp;
use gostd::strings::trim_space;

use crate::detect::go_lossy;
use crate::extract::builder::{Builder, parse_root};
use crate::extract::kinds;
use crate::testutil::Rng;

/// Go `(*builder).jsRationale` (`javascript.go:1441`).
fn js_rationale(b: &mut Builder, src: &[u8]) {
    todo!("graphify-rs-ihc.26")
}

/// Go `(*builder).pyRationale` (`python.go:721`).
fn py_rationale(b: &mut Builder, root: Node<'_>, src: &[u8]) {
    crate::extract::python::py_rationale(b, root, src)
}

/// Go `(*builder).addDocRef` (`javascript.go:1489`).
fn add_doc_ref(b: &mut Builder, token: &str, line_num: i64, seen_doc_refs: &mut BTreeSet<String>) {
    todo!("graphify-rs-ihc.26")
}

/// Go `jsRationalePrefixes` (`javascript.go:1419`).
const JS_RATIONALE_PREFIXES: [&str; 14] = [
    "// NOTE:",
    "// IMPORTANT:",
    "// HACK:",
    "// WHY:",
    "// RATIONALE:",
    "// TODO:",
    "// FIXME:",
    "* NOTE:",
    "* IMPORTANT:",
    "* HACK:",
    "* WHY:",
    "* RATIONALE:",
    "* TODO:",
    "* FIXME:",
];

/// Go `pyRationalePrefixes` (`python.go:710`).
const PY_RATIONALE_PREFIXES: [&str; 7] = [
    "# NOTE:",
    "# IMPORTANT:",
    "# HACK:",
    "# WHY:",
    "# RATIONALE:",
    "# TODO:",
    "# FIXME:",
];

/// Go `jsCommentLineRe` (`javascript.go:1428`).
static JS_COMMENT_LINE_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"^\s*(//|/\*|\*)"));

/// Go `jsDocRefRe` (`javascript.go:1433`).
static JS_DOC_REF_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"(?i)\b(ADR[- ]?\d{1,5}|RFC[- ]?\d{1,5})\b"));

/// Go's `jsRef`: the whole-file split the line scan replaced. The text is what Go holds for the
/// bytes (`go_lossy`).
fn js_ref(b: &mut Builder, src: &[u8]) {
    let mut seen = BTreeSet::new();
    let text = go_lossy(src);
    for (i, line_text) in text.split('\n').enumerate() {
        let line_num = i as i64 + 1;
        let stripped = trim_space(line_text);
        for p in JS_RATIONALE_PREFIXES {
            if stripped.starts_with(p) {
                let file_id = b.file_id.clone();
                b.add_rationale(
                    stripped.trim_start_matches(['/', '*', ' ']),
                    line_num,
                    &file_id,
                );
                break;
            }
        }
        if JS_COMMENT_LINE_RE.match_string(line_text) {
            for m in JS_DOC_REF_RE.find_all_string_submatch(stripped, -1) {
                add_doc_ref(b, m[1].unwrap_or(""), line_num, &mut seen);
            }
        }
    }
}

/// Go's `pyRef`.
fn py_ref(b: &mut Builder, src: &[u8]) {
    let text = go_lossy(src);
    for (i, line_text) in text.split('\n').enumerate() {
        let stripped = trim_space(line_text);
        for p in PY_RATIONALE_PREFIXES {
            if stripped.starts_with(p) {
                let file_id = b.file_id.clone();
                b.add_rationale(stripped, i as i64 + 1, &file_id);
                break;
            }
        }
    }
}

/// The line scans must record what the whole-file split they replaced did. Go seeds
/// `rand.NewSource(1)`; the generator here is `testutil::Rng` (any sequence serves). The tokens
/// are bytes, as Go's are: `\xc2\xa0` is ` `, `\xe2\x84\xaa` is `K`, `\xff` is not UTF-8.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_rationale_scan_matches_split() {
    kinds::register_kinds();
    let toks: &[&[u8]] = &[
        b"// NOTE: a",
        b"* WHY: b",
        b"# TODO: c",
        b"// ",
        b"/* ",
        b"* ",
        b"#",
        b"x",
        b" ",
        b"\t",
        b"\x0b",
        b"\xc2\xa0",
        b"\r",
        b"\n",
        b"\n",
        b"ADR-12",
        b"adr 7",
        b"rfc793",
        b"RFC-1",
        b"ADRx",
        b"RF C",
        b"\xe2\x84\xaa",
        b"\xff",
        b"see ",
        b"= 1;",
    ];
    // pyRationale reads docstrings off the tree first; an empty one has none.
    parse_root(&[], gositter_grammars::python::language(), |root| {
        let mut rng = Rng::new(1);
        let mut found = 0;
        for _ in 0..50000 {
            let mut src: Vec<u8> = Vec::new();
            for _ in 0..rng.intn(12) {
                src.extend_from_slice(toks[rng.intn(toks.len())]);
            }
            let (mut got, mut want) = (Builder::new("a.ts"), Builder::new("a.ts"));
            js_rationale(&mut got, &src);
            js_ref(&mut want, &src);
            assert!(
                got.res == want.res,
                "js {src:?}:\n got {:?}\nwant {:?}",
                got.res,
                want.res
            );
            found += got.res.nodes.len();
            let (mut got, mut want) = (Builder::new("a.py"), Builder::new("a.py"));
            py_rationale(&mut got, root, &src);
            py_ref(&mut want, &src);
            assert!(
                got.res == want.res,
                "py {src:?}:\n got {:?}\nwant {:?}",
                got.res,
                want.res
            );
            found += got.res.nodes.len();
        }
        assert!(
            found >= 1000,
            "only {found} nodes recorded: the generator no longer reaches the scans"
        );
    })
    .expect("parse of the empty python source");
}
