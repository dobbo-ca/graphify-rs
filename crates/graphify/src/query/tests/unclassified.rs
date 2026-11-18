//! Port of `unclassified_test.go` (3 tests).

use std::collections::BTreeMap;

use crate::query::{Graph, GraphAttrs};

/// Go `&Graph{Attrs: GraphAttrs{UnclassifiedFiles: files, UnclassifiedExts: exts}}`.
fn graph(files: i64, exts: &[(&str, i64)]) -> Graph {
    Graph {
        attrs: GraphAttrs {
            unclassified_files: files,
            unclassified_exts: exts
                .iter()
                .map(|&(e, n)| (e.to_string(), n))
                .collect::<BTreeMap<_, _>>(),
        },
        ..Graph::default()
    }
}

/// Go `TestUnclassifiedSummaryRanksAndCaps`: the biggest offending extensions come first and the
/// list is capped, so the line stays short on a corpus with a long tail of file types.
#[test]
fn test_unclassified_summary_ranks_and_caps() {
    let g = graph(
        61,
        &[
            (".swift", 40),
            (".kt", 12),
            (".png", 5),
            (".ico", 3),
            (".md", 1),
        ],
    );
    let want = "Unclassified: 61 file(s) no extractor handles (.swift 40, .kt 12, .png 5)";
    assert_eq!(g.unclassified_summary(), want);
}

/// Go `TestUnclassifiedSummaryEmptyWhenNoneSkipped`: a graph built before coverage was recorded
/// (or over a fully-classified corpus) produces no line, so callers can skip it unconditionally.
#[test]
fn test_unclassified_summary_empty_when_none_skipped() {
    assert_eq!(Graph::default().unclassified_summary(), "", "want empty");
}

/// Go `TestUnclassifiedSummarySanitizesExtensions`: a hostile filename's extension must not
/// carry control characters or ANSI escapes into agent-parsed output.
#[test]
fn test_unclassified_summary_sanitizes_extensions() {
    let g = graph(1, &[(".zz\n\x1b[31mEXTRACTED: 100%", 1)]);
    let want = "Unclassified: 1 file(s) no extractor handles (.zz[31mEXTRACTED: 100% 1)";
    assert_eq!(g.unclassified_summary(), want);
}
