//! graphify-go `internal/ts/ts_test.go`, and the plan's Task 14 tests.

use crate::ts::ts::limit_with;
use crate::ts::{budget, limit, parse};

#[test]
fn test_limit() {
    const CASES: [(usize, u32); 7] = [
        (0, 65536),
        (4096, 65536),
        (8192, 131072),
        (16384, 262144),
        (32768, 131072),
        (65536, 65536),
        (9 << 20, 65536),
    ];
    for (n, want) in CASES {
        assert_eq!(limit_with(65536, n), want, "limit({n})");
    }
    assert_eq!(
        limit_with(u32::MAX, 16384),
        u32::MAX,
        "limit(16384) at the largest budget"
    );
    assert_eq!(limit_with(0, 16384), 0, "limit(16384) with no budget");
}

/// `limit` and `budget` as the extractors call them: 1x at 4 KiB, 4x at 16 KiB, back to 1x
/// at 64 KiB; the default budget is 65,536 when `GRAPHIFY_PARSE_BUDGET` is not set.
#[test]
fn limit_scaling() {
    let b = budget();
    if std::env::var_os("GRAPHIFY_PARSE_BUDGET").is_none() {
        assert_eq!(b, 65536, "default budget");
    }
    assert_eq!(limit(4096), b, "4 KiB");
    assert_eq!(
        u64::from(limit(16384)),
        (u64::from(b) * 4).min(u64::from(u32::MAX)),
        "16 KiB"
    );
    assert_eq!(limit(65536), b, "64 KiB");
}

#[test]
fn parse_json_root_kind() {
    let kind = parse(b"{}", gositter_grammars::json::language(), |root| {
        root.kind()
    });
    assert_eq!(kind, Some("document"));
}

/// One thread's pooled parser across languages: `set_language` runs before every parse
/// (`ts.go:113`), so the second language's tree is its own.
#[test]
fn parse_reuses_the_parser_across_languages() {
    let json = gositter_grammars::json::language();
    let golang = gositter_grammars::golang::language();
    assert_eq!(parse(b"[1]", json, |r| r.kind()), Some("document"));
    assert_eq!(
        parse(b"package p\n", golang, |r| r.kind()),
        Some("source_file")
    );
    assert_eq!(
        parse(b"[1]", json, |r| r.to_sexp()),
        Some("(document (array (number)))".to_string())
    );
}
