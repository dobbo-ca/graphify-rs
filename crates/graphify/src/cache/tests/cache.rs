//! Port of `cache_test.go` (8 tests), then the upstream cases of `test_cache.py` and
//! `test_stat_index_portability.py` that hold for graphify-go v0.12.0. None of them parses a
//! source file (the `.go` and `.tf` names are only cache keys), so every test waits for the
//! cache code alone.

use std::fs;

use super::{WORKERS, cache_of, chtimes, entry, hour_ago, hour_ahead, mtime_ns};
use crate::cache::cache::{hash_file_with, parse_granularity};
use crate::cache::{
    Cache, Entry, FILE_NAME, STAT_FILE_NAME, StatEntry, StatIndex, hash_bytes, hash_file, load,
    load_stat, save, save_stat, stale, stamp,
};
use crate::extract::Result as ExtractResult;
use crate::model::Node;
use crate::testutil::TempDir;

/// `StatIndex{"a.go": e}`.
fn stat_index(e: StatEntry) -> StatIndex {
    StatIndex::from([("a.go".to_string(), e)])
}

/// Go `TestStatIndexRoundTrip`: the stat sidecar survives a save and load, and a missing or
/// corrupt file is an empty index.
#[test]
fn test_stat_index_round_trip() {
    let dir = TempDir::new();
    let path = dir.join(STAT_FILE_NAME);

    let want = stat_index(StatEntry {
        size: 12,
        mtime_ns: 345,
        hash: "deadbeef".to_string(),
        ..StatEntry::default()
    });
    save_stat(&path, &stamp("v1"), &want, None).expect("SaveStat");
    let got = load_stat(&path, &stamp("v1"));
    assert_eq!(
        got.get("a.go"),
        want.get("a.go"),
        "round-trip mismatch: got {:?}, want {:?}",
        got.get("a.go"),
        want.get("a.go")
    );

    assert!(
        load_stat(&dir.join("missing.json"), &stamp("v1")).is_empty(),
        "LoadStat of a missing file should be empty"
    );
    fs::write(&path, "{not json").unwrap();
    assert!(
        load_stat(&path, &stamp("v1")).is_empty(),
        "LoadStat of corrupt JSON should be empty"
    );
}

/// Go `TestHashFileFastpath`: a matching size+mtime entry reuses the stored hash without
/// reading the file (`src` is `None`), while a stat change reads and re-hashes.
#[test]
fn test_hash_file_fastpath() {
    let dir = TempDir::new();
    let f = dir.write("x.go", b"package p\n");
    // Age the file past the mtime granularity so its stat signature can be trusted; a
    // just-written file is racily clean and never takes the fastpath.
    chtimes(&f, hour_ago());

    // First encounter: no prev entry, so a full read: hash computed, bytes returned.
    let Some((h1, e1, src1)) = hash_file(&f, &StatEntry::default(), false) else {
        panic!("first HashFile: ok=false, want true");
    };
    assert!(src1.is_some(), "first HashFile: src!=nil=false, want true");
    assert!(
        e1.hash == h1 && e1.size == "package p\n".len() as i64,
        "first stat entry mismatch: {e1:?} (hash {h1})"
    );

    // Corrupt the contents but restore the original size and mtime, so the fastpath must
    // match on stat and serve the stored hash without reading.
    let fi = fs::metadata(&f).unwrap();
    fs::write(&f, "XXXXXXXXXX").unwrap();
    chtimes(&f, fi.modified().unwrap());
    let Some((h2, _, src2)) = hash_file(&f, &e1, true) else {
        panic!("fastpath HashFile: ok=false");
    };
    assert!(
        src2.is_none(),
        "fastpath should not read the file (src should be nil)"
    );
    assert_eq!(h2, h1, "fastpath should reuse stored hash {h1}, got {h2}");

    // Bump the mtime so size+mtime no longer match: now it must read and hash.
    chtimes(&f, hour_ahead());
    let Some((h3, _, src3)) = hash_file(&f, &e1, true) else {
        panic!("post-mtime-change HashFile: ok=false src!=nil=false");
    };
    assert!(
        src3.is_some(),
        "post-mtime-change HashFile: ok=true src!=nil=false"
    );
    assert_ne!(
        h3, h1,
        "changed file should produce a different hash than the stale entry"
    );
}

/// Go `TestHashFileMissing`: `ok=false` for a file that cannot be read.
#[test]
fn test_hash_file_missing() {
    let dir = TempDir::new();
    assert!(
        hash_file(&dir.join("nope.go"), &StatEntry::default(), false).is_none(),
        "HashFile of a missing file should return ok=false"
    );
}

/// Go `TestHashFileRacilyClean`: a same-length edit inside the file's mtime tick leaves size
/// and mtime untouched, so the stat signature alone cannot prove the cached hash still
/// describes the content.
#[test]
fn test_hash_file_racily_clean() {
    let dir = TempDir::new();
    let f = dir.write("x.go", b"func AAAA() {}\n");

    let Some((h1, e1, _)) = hash_file(&f, &StatEntry::default(), false) else {
        panic!("first HashFile: ok=false");
    };

    // Same-length rewrite, mtime restored: stat signature is identical.
    let fi = fs::metadata(&f).unwrap();
    fs::write(&f, "func BBBB() {}\n").unwrap();
    chtimes(&f, fi.modified().unwrap());
    let Some((h2, _, src2)) = hash_file(&f, &e1, true) else {
        panic!("racily-clean HashFile: ok=false src!=nil=false, want true/true");
    };
    assert!(
        src2.is_some(),
        "racily-clean HashFile: ok=true src!=nil=false, want true/true"
    );
    assert_ne!(
        h2, h1,
        "same-size edit within the mtime tick must produce a changed hash"
    );

    // GRAPHIFY_MTIME_GRANULARITY_MS=0 disables the guard, restoring the old (unsafe)
    // fastpath. Go sets the variable; `set_var` is unsafe in edition 2024, so the
    // granularity it parses to is passed to `hash_file_with`.
    let Some((_, _, src3)) = hash_file_with(&f, &e1, true, 0) else {
        panic!("guard disabled: ok=false, want true");
    };
    assert!(
        src3.is_none(),
        "guard disabled: ok=true src!=nil=true, want true/false"
    );
}

/// Added: the parse of `GRAPHIFY_MTIME_GRANULARITY_MS` (Go `mtimeGranularity`), which
/// `test_hash_file_racily_clean` cannot reach without `set_var`.
#[test]
fn parse_granularity_cases() {
    const DEFAULT: i64 = 2_000_000_000;
    let cases: [(&str, i64); 7] = [
        ("", DEFAULT),
        ("abc", DEFAULT),
        ("-1", DEFAULT),
        ("0", 0),
        ("1.5", 1_500_000),
        ("2000", DEFAULT),
        ("15.6", 15_600_000),
    ];
    for (raw, want) in cases {
        assert_eq!(parse_granularity(raw), want, "parse_granularity({raw:?})");
    }
}

/// Go `TestHashFileLegacyEntry`: a sidecar entry from an older graphify-go (no
/// `indexed_at_ns`) is distrusted once and re-read.
#[test]
fn test_hash_file_legacy_entry() {
    let dir = TempDir::new();
    let f = dir.write("x.go", b"package p\n");
    chtimes(&f, hour_ago());
    let fi = fs::metadata(&f).unwrap();
    let legacy = StatEntry {
        size: i64::try_from(fi.len()).unwrap(),
        mtime_ns: mtime_ns(&fi),
        hash: "stale".to_string(),
        ..StatEntry::default()
    };
    let Some((h, rewritten, src)) = hash_file(&f, &legacy, true) else {
        panic!("legacy entry: ok=false src!=nil=false, want true/true");
    };
    assert!(
        src.is_some(),
        "legacy entry: ok=true src!=nil=false, want true/true"
    );
    assert_ne!(
        h, "stale",
        "legacy entry without indexed_at_ns must not be trusted"
    );
    assert_ne!(
        rewritten.indexed_at_ns, 0,
        "rewritten entry should carry indexed_at_ns"
    );
}

/// Go `TestStampMismatchDiscards`: a cache and stat sidecar written by one binary version are
/// ignored by another, so an extractor fix shipped in a new build invalidates every cached
/// result instead of being served stale ones.
#[test]
fn test_stamp_mismatch_discards() {
    let dir = TempDir::new();
    let cache_path = dir.join(FILE_NAME);
    let stat_path = dir.join(STAT_FILE_NAME);

    let want = cache_of(vec![("a.go", entry("abc"))]);
    let want_stat = stat_index(StatEntry {
        size: 1,
        mtime_ns: 2,
        hash: "abc".to_string(),
        ..StatEntry::default()
    });
    save(&cache_path, &stamp("v1.0.0"), &want).expect("Save");
    save_stat(&stat_path, &stamp("v1.0.0"), &want_stat, None).expect("SaveStat");

    // Same stamp: round-trips.
    let got = load(&cache_path, &stamp("v1.0.0"), WORKERS);
    assert_eq!(
        got.get("a.go").map(|e| e.hash.as_str()),
        Some("abc"),
        "same-stamp Load: got {got:?}, want the saved entry"
    );
    let got = load_stat(&stat_path, &stamp("v1.0.0"));
    assert_eq!(
        got.get("a.go"),
        want_stat.get("a.go"),
        "same-stamp LoadStat: got {:?}, want {:?}",
        got.get("a.go"),
        want_stat.get("a.go")
    );

    // Different binary version: both are discarded.
    let got = load(&cache_path, &stamp("v1.1.0"), WORKERS);
    assert!(
        got.is_empty(),
        "mismatched-stamp Load should be empty, got {got:?}"
    );
    let got = load_stat(&stat_path, &stamp("v1.1.0"));
    assert!(
        got.is_empty(),
        "mismatched-stamp LoadStat should be empty, got {got:?}"
    );

    // An unstamped (pre-upgrade) file is discarded too.
    fs::write(&cache_path, r#"{"a.go":{"hash":"abc"}}"#).unwrap();
    let got = load(&cache_path, &stamp("v1.0.0"), WORKERS);
    assert!(
        got.is_empty(),
        "unstamped Load should be empty, got {got:?}"
    );
}

/// Go `TestSchemaBumpDiscardsPreFeatureCache`: a warm cache from a pre-feature binary was
/// reused verbatim: the version is "dev" for every locally built binary, so only the schema
/// number distinguishes a stale entry. A cache written under the old schema-1 stamp must be
/// discarded even though the version string ("dev") is unchanged.
#[test]
fn test_schema_bump_discards_pre_feature_cache() {
    let dir = TempDir::new();
    let cache_path = dir.join(FILE_NAME);

    let pre_feature_stamp = "dev-s1";
    save(
        &cache_path,
        pre_feature_stamp,
        &cache_of(vec![("main.tf", entry("abc"))]),
    )
    .expect("Save");

    let got = load(&cache_path, &stamp("dev"), WORKERS);
    assert!(
        got.is_empty(),
        "pre-feature schema cache should be discarded under current stamp, got {got:?}"
    );
}

/// Go `TestStale`.
#[test]
fn test_stale() {
    let dir = TempDir::new();
    let path = dir.join(FILE_NAME);
    assert!(!stale(&path, &stamp("dev")), "a missing file is stale");
    save(&path, &stamp("dev"), &cache_of(vec![("a.go", entry("h"))])).unwrap();
    assert!(
        !stale(&path, &stamp("dev")),
        "a cache under the same stamp is stale"
    );
    // A longer and a shorter stamp than the file's.
    let longer = format!("{}-b9", stamp("dev"));
    for other in ["dev-s13", longer.as_str(), "d"] {
        assert!(
            stale(&path, other),
            "a cache under {:?} is not stale for {other:?}",
            stamp("dev")
        );
    }
    fs::write(&path, "not a cache").unwrap();
    assert!(
        !stale(&path, &stamp("dev")),
        "a file that is not a cache is stale"
    );
}

// ---- upstream `test_cache.py` and `test_stat_index_portability.py` ----

/// The hash `hash_file` gives a file read whole (no previous entry).
fn hash_of(f: &std::path::Path) -> String {
    let Some((h, _, _)) = hash_file(f, &StatEntry::default(), false) else {
        panic!("hash_file({f:?}): ok=false");
    };
    h
}

/// Upstream `test_file_hash_consistent`.
#[test]
fn test_file_hash_consistent() {
    let dir = TempDir::new();
    let f = dir.write("sample.txt", b"hello world");
    let h1 = hash_of(&f);
    let h2 = hash_of(&f);
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64, "SHA256 hex digest length");
    assert_eq!(h1, hash_bytes(b"hello world"));
}

/// Upstream `test_file_hash_changes`.
#[test]
fn test_file_hash_changes() {
    let dir = TempDir::new();
    let f1 = dir.write("a.txt", b"content one");
    let f2 = dir.write("b.txt", b"content two");
    assert_ne!(hash_of(&f1), hash_of(&f2));
}

/// Upstream `test_md_body_change_different_hash`: Go hashes the whole file, so any change of
/// the body changes the hash.
#[test]
fn test_md_body_change_different_hash() {
    let dir = TempDir::new();
    let f = dir.write(
        "doc.md",
        b"---\nreviewed: 2026-01-01\n---\n\n# Title\n\nOriginal body.",
    );
    let h1 = hash_of(&f);
    fs::write(
        &f,
        "---\nreviewed: 2026-01-01\n---\n\n# Title\n\nChanged body.",
    )
    .unwrap();
    assert_ne!(h1, hash_of(&f));
}

/// Upstream `test_md_no_frontmatter_hashed_normally`.
#[test]
fn test_md_no_frontmatter_hashed_normally() {
    let dir = TempDir::new();
    let f = dir.write("doc.md", b"# Just a heading\n\nNo frontmatter here.");
    let h1 = hash_of(&f);
    fs::write(&f, "# Just a heading\n\nDifferent content.").unwrap();
    assert_ne!(h1, hash_of(&f));
}

/// Upstream `test_non_md_file_hashed_fully`.
#[test]
fn test_non_md_file_hashed_fully() {
    let dir = TempDir::new();
    let f = dir.write("script.py", b"# comment\nx = 1");
    let h1 = hash_of(&f);
    fs::write(&f, "# changed comment\nx = 1").unwrap();
    assert_ne!(h1, hash_of(&f));
}

/// Upstream `test_cache_roundtrip`: a saved result loads back equal, as a cache entry.
#[test]
fn test_cache_roundtrip() {
    let dir = TempDir::new();
    let f = dir.write("sample.txt", b"hello world");
    let result = ExtractResult {
        nodes: vec![Node {
            id: "n1".to_string(),
            label: "Node1".to_string(),
            ..Node::default()
        }],
        ..ExtractResult::default()
    };
    let path = dir.join(FILE_NAME);
    let c = cache_of(vec![(
        "sample.txt",
        Entry {
            hash: hash_of(&f),
            result: result.clone(),
        },
    )]);
    save(&path, &stamp("dev"), &c).expect("Save");
    let loaded = load(&path, &stamp("dev"), WORKERS);
    assert_eq!(loaded.get("sample.txt").map(|e| &e.result), Some(&result));
}

/// Upstream `test_cache_miss_on_change`: after the file changes its hash no longer equals the
/// cached entry's (Go's caller compares the two; there is no per-file `load_cached`).
#[test]
fn test_cache_miss_on_change() {
    let dir = TempDir::new();
    let f = dir.write("sample.txt", b"hello world");
    let path = dir.join(FILE_NAME);
    let c = cache_of(vec![("sample.txt", entry(&hash_of(&f)))]);
    save(&path, &stamp("dev"), &c).expect("Save");
    fs::write(&f, "completely different content").unwrap();
    let loaded: Cache = load(&path, &stamp("dev"), WORKERS);
    assert_ne!(loaded["sample.txt"].hash, hash_of(&f));
}

/// Upstream `test_ast_cache_invalidated_on_version_bump`: an entry written by version X is not
/// served after upgrading to version Y.
#[test]
fn test_ast_cache_invalidated_on_version_bump() {
    let dir = TempDir::new();
    let path = dir.join(FILE_NAME);
    save(
        &path,
        &stamp("0.8.0"),
        &cache_of(vec![("mod.py", entry("h"))]),
    )
    .expect("Save");
    assert!(!load(&path, &stamp("0.8.0"), WORKERS).is_empty());
    assert!(
        load(&path, &stamp("0.8.1"), WORKERS).is_empty(),
        "cache entry from a previous graphify version must not be served"
    );
    assert!(stale(&path, &stamp("0.8.1")));
}

/// Upstream `test_legacy_unversioned_ast_entries_not_served`: a file in the pre-versioning
/// layout (entries keyed by hash, no stamp) is not served.
#[test]
fn test_legacy_unversioned_entries_not_served() {
    let dir = TempDir::new();
    let path = dir.write(
        FILE_NAME,
        br#"{"0a1b":{"nodes":[{"id":"stale"}],"edges":[]}}"#,
    );
    assert!(load(&path, &stamp("dev"), WORKERS).is_empty());
}

/// Upstream `test_file_hash_detects_same_size_rewrite_within_one_mtime_tick`.
#[test]
fn test_file_hash_detects_same_size_rewrite_within_one_mtime_tick() {
    let dir = TempDir::new();
    let f = dir.write("mod.py", b"x = 1  # aaa\n");
    let fi = fs::metadata(&f).unwrap();
    let Some((h1, e1, _)) = hash_file(&f, &StatEntry::default(), false) else {
        panic!("first hash_file: ok=false");
    };

    // Same length, new content, inside the same tick.
    fs::write(&f, "x = 2  # bbb\n").unwrap();
    chtimes(&f, fi.modified().unwrap());
    let fi2 = fs::metadata(&f).unwrap();
    assert!(
        fi2.len() == fi.len() && mtime_ns(&fi2) == mtime_ns(&fi),
        "test setup failed to reproduce an identical stat signature"
    );

    let Some((h2, _, _)) = hash_file(&f, &e1, true) else {
        panic!("second hash_file: ok=false");
    };
    assert_ne!(
        h1, h2,
        "same-size rewrite returned the previous content's digest"
    );
}

/// Upstream `test_file_hash_fastpath_still_serves_a_settled_file`: once a file's mtime tick
/// has closed, the digest is served from the index without reading.
#[test]
fn test_file_hash_fastpath_still_serves_a_settled_file() {
    let dir = TempDir::new();
    let f = dir.write("mod.py", b"x = 1\n");
    // Backdate well past the granularity window so the entry is provably clean.
    chtimes(
        &f,
        std::time::SystemTime::now() - std::time::Duration::from_secs(60),
    );

    let Some((first, e1, _)) = hash_file(&f, &StatEntry::default(), false) else {
        panic!("first hash_file: ok=false");
    };
    let Some((second, _, src)) = hash_file(&f, &e1, true) else {
        panic!("second hash_file: ok=false");
    };
    assert_eq!(second, first);
    assert!(
        src.is_none(),
        "settled file was re-read; the stat fastpath is dead"
    );
}

/// Upstream `test_legacy_absolute_index_migrates_gracefully`, the hash half: an unstamped
/// legacy entry costs exactly one re-read, and having paid it, the healed entry is warm from
/// then on. (Go keys are the caller's strings: there is no root anchor to migrate or prune.)
#[test]
fn test_legacy_absolute_index_migrates_gracefully() {
    let dir = TempDir::new();
    let f = dir.write("f1.py", b"x = 1\n");
    chtimes(
        &f,
        std::time::SystemTime::now() - std::time::Duration::from_secs(10),
    );
    let fi = fs::metadata(&f).unwrap();
    let digest = hash_bytes(b"x = 1\n");
    let legacy = StatEntry {
        size: i64::try_from(fi.len()).unwrap(),
        mtime_ns: mtime_ns(&fi),
        hash: digest.clone(),
        ..StatEntry::default()
    };

    let Some((h, healed, src)) = hash_file(&f, &legacy, true) else {
        panic!("legacy entry: ok=false");
    };
    assert_eq!(h, digest);
    assert!(
        src.is_some(),
        "unstamped legacy entry should cost exactly one re-read"
    );
    assert_ne!(
        healed.indexed_at_ns, 0,
        "self-heal should stamp the entry so it is trustable next run"
    );

    let Some((h, _, src)) = hash_file(&f, &healed, true) else {
        panic!("healed entry: ok=false");
    };
    assert_eq!(h, digest);
    assert!(
        src.is_none(),
        "healed entry should serve from the stat index"
    );
}
