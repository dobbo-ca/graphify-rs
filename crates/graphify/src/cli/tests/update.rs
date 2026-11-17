//! Port of `update_test.go`: 6 of its 7 tests. `TestReportTitleMatchesBetweenBuildAndUpdate`
//! changes the working directory and sets `GRAPHIFY_OUT`, so it runs the binary, as does the
//! `GRAPHIFY_FORCE=1` step of `TestForceBypassesCache` (`crates/graphify/tests/cli_update.rs`).
//! Go's `runtime.NumCPU()` worker counts are a parameter here (R5): these pass 8.

use std::path::Path;

use super::core_util::{graph_node_count, write_tree};
use super::{VERSION, assemble, cache_stamp, cmd_build, cmd_update, write_outputs};
use crate::cache::{self, Cache, Entry, FILE_NAME, STAT_FILE_NAME, StatIndex};
use crate::cli::maintenance::maint_util::{args, names};
use crate::cli::semantic::SemanticOpts;
use crate::detect::{WalkReport, collect_files};
use crate::extract;
use crate::testutil::TempDir;

const WORKERS: usize = 8;

fn build(root: &Path, extra: &[&str]) {
    let mut a = vec![root.to_str().expect("UTF-8 temp path")];
    a.extend_from_slice(extra);
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    if let Err(e) = cmd_build(&args(&a), &mut out, &mut err) {
        panic!("build: {e}");
    }
}

/// `update <root> <extra...>`: (stdout, stderr).
fn update(root: &Path, extra: &[&str]) -> (String, String) {
    let mut a = vec![root.to_str().expect("UTF-8 temp path")];
    a.extend_from_slice(extra);
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    if let Err(e) = cmd_update(&args(&a), &mut out, &mut err) {
        panic!("update {extra:?}: {e}");
    }
    (
        String::from_utf8(out).expect("stdout is UTF-8"),
        String::from_utf8(err).expect("stderr is UTF-8"),
    )
}

/// The (device, inode) of `path`: Go `os.SameFile`.
#[cfg(unix)]
fn file_id(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

/// Off unix there is no inode: the modification time stands in for it.
#[cfg(not(unix))]
fn file_id(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

// `update` reuses the cache and produces the same graph a full build would: byte-identical
// output when nothing changed, and a single re-parse when one file changes.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_incremental_update() {
    let root = TempDir::new();
    let r = root.path();
    write_tree(
        r,
        &[
            ("a.go", "package p\n\nfunc A() { B() }\n"),
            ("b.go", "package p\n\nfunc B() {}\n"),
        ],
    );
    build(r, &[]);
    // build writes the stat sidecar with an entry per collected file.
    let stat_path = root.join(&format!("graphify-out/{STAT_FILE_NAME}"));
    let stat_idx = cache::load_stat(&stat_path, &cache_stamp());
    assert_eq!(stat_idx.len(), 2, "stat sidecar entries");
    let graph_path = root.join("graphify-out/graph.json");
    let built = std::fs::read(&graph_path).expect("read graph.json");

    // An unchanged update reuses every file and re-parses none.
    let cache_path = root.join(&format!("graphify-out/{FILE_NAME}"));
    let files = collect_files(r).expect("collect files");
    let prev = cache::load(&cache_path, &cache_stamp(), WORKERS);
    let prev_stat = cache::load_stat(&stat_path, &cache_stamp());
    let (_, _, _, stats) = assemble(r, &files, Some(&prev), &prev_stat, WORKERS);
    let n = files.len() as i64;
    assert!(
        stats.parsed == 0 && stats.reused == n && stats.dropped == 0,
        "unchanged update: got {stats:?}, want 0 reparsed / {n} reused / 0 removed"
    );

    let cache_built = file_id(&cache_path).expect("stat cache file");
    update(r, &[]);
    // The cache already holds every entry, so the file is left as it is.
    assert_eq!(
        file_id(&cache_path),
        Some(cache_built),
        "unchanged update replaced the cache file"
    );
    let updated = std::fs::read(&graph_path).expect("read graph.json");
    assert!(
        built == updated,
        "incremental update changed graph.json despite no source change"
    );

    // Changing one file re-parses exactly that file.
    write_tree(r, &[("b.go", "package p\n\nfunc B() {}\n\nfunc C() {}\n")]);
    let files = collect_files(r).expect("collect files");
    let prev = cache::load(&cache_path, &cache_stamp(), WORKERS);
    let prev_stat = cache::load_stat(&stat_path, &cache_stamp());
    let (_, _, _, stats) = assemble(r, &files, Some(&prev), &prev_stat, WORKERS);
    let n = files.len() as i64;
    assert!(
        stats.parsed == 1 && stats.reused == n - 1,
        "one-file change: got {stats:?}, want 1 reparsed / {} reused",
        n - 1
    );
    update(r, &[]);
    let changed = std::fs::read(&graph_path).expect("read graph.json");
    assert!(
        built != changed,
        "expected graph.json to change after editing a source file"
    );
    // The re-parsed file went into a new cache file.
    let id = file_id(&cache_path);
    assert!(
        id.is_some() && id != Some(cache_built),
        "update after a change kept the old cache file"
    );
    let prev = cache::load(&cache_path, &cache_stamp(), WORKERS);
    let (_, _, _, stats) = assemble(r, &files, Some(&prev), &StatIndex::new(), WORKERS);
    assert_eq!(stats.parsed, 0, "cache after a change: reparsed");
}

// Regression for the #479/#1116 interaction: removing symbols from a re-parsed file legitimately
// shrinks the graph, and `update` must refresh graph.json rather than warn and keep the larger
// stale one.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_update_allows_legit_shrink() {
    let root = TempDir::new();
    let r = root.path();
    write_tree(
        r,
        &[(
            "a.go",
            "package p\n\nfunc A() {}\n\nfunc B() {}\n\nfunc C() {}\n",
        )],
    );
    build(r, &[]);
    let graph_path = root.join("graphify-out/graph.json");
    let report_path = root.join("graphify-out/GRAPH_REPORT.md");
    let before = graph_node_count(&graph_path);
    let report_before = std::fs::read(&report_path).expect("read report");

    // Remove B and C: the graph must shrink and the write must not be refused.
    write_tree(r, &[("a.go", "package p\n\nfunc A() {}\n")]);
    update(r, &[]);
    let after = graph_node_count(&graph_path);
    assert!(
        after < before,
        "legit shrink not applied: before={before} after={after} (graph.json should have refreshed to fewer nodes)"
    );
    // The legit write path refreshes every output, not just graph.json.
    let report_after = std::fs::read(&report_path).expect("read report");
    assert!(
        report_before != report_after,
        "legit shrink did not refresh GRAPH_REPORT.md"
    );
}

// Regression for graphify-go-1fz.2: on an ILLEGITIMATE refused shrink (a lost node's source file
// was skipped this run), write_outputs leaves graphify-out untouched: graph.json, the report, the
// cache and the stat sidecar. With force set the same shrink is accepted and every output is
// rewritten.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_refused_shrink_aborts_all_outputs() {
    let root = TempDir::new();
    let r = root.path();
    write_tree(
        r,
        &[
            ("a.go", "package p\n\nfunc A() { B() }\n"),
            ("b.go", "package p\n\nfunc B() {}\n"),
        ],
    );
    build(r, &[]);

    let out_dir = root.join("graphify-out");
    let paths = [
        ("graph.json", out_dir.join("graph.json")),
        ("GRAPH_REPORT.md", out_dir.join("GRAPH_REPORT.md")),
        ("cache", out_dir.join(FILE_NAME)),
        ("stat", out_dir.join(STAT_FILE_NAME)),
    ];
    let snapshot = || -> Vec<Vec<u8>> {
        paths
            .iter()
            .map(|(_, p)| std::fs::read(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display())))
            .collect()
    };
    let before = snapshot();
    // The cache is encoded before the refusal is known; it must not be left behind.
    let names_before = names(&out_dir).expect("list graphify-out");

    // Simulate an update where b.go was skipped (failed to read/parse): its result is absent, so
    // the graph shrinks and the lost node's source (b.go) lands in the skipped set.
    let files = vec!["a.go".to_string(), "b.go".to_string()];
    let ra = extract::file(r, "a.go").expect("extract a.go");
    let new_cache = Cache::from([(
        "a.go".to_string(),
        Entry {
            hash: "h".to_string(),
            result: ra.clone(),
        },
    )]);
    let walk = WalkReport {
        files,
        ..WalkReport::default()
    };
    let root_str = r.to_str().expect("UTF-8 temp path");
    let write = |force: bool| {
        let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
        write_outputs(
            root_str,
            &walk,
            vec![ra.clone()],
            new_cache.clone(),
            StatIndex::new(),
            &SemanticOpts::default(),
            force,
            false,
            false,
            "",
            WORKERS,
            &mut out,
            &mut err,
        )
    };

    // force=false: the refusal must abort every write, leaving all four outputs byte-identical.
    if let Err(e) = write(false) {
        panic!("write_outputs (refused shrink) returned error, want a warning and Ok: {e}");
    }
    for ((name, _), (b, a)) in paths.iter().zip(before.iter().zip(snapshot())) {
        assert!(
            *b == a,
            "refused shrink rewrote {name}; every output must be left untouched"
        );
    }
    assert_eq!(
        names(&out_dir).expect("list graphify-out"),
        names_before,
        "refused shrink changed the directory listing"
    );

    // force=true: the same shrink is accepted and every output is rewritten.
    if let Err(e) = write(true) {
        panic!("write_outputs (forced shrink): {e}");
    }
    for ((name, _), (b, a)) in paths.iter().zip(before.iter().zip(snapshot())) {
        assert!(
            *b != a,
            "forced shrink did not rewrite {name}; force must write all outputs"
        );
    }
}

// Regression for graphify-go-2af.6: --force means a full re-scan, so a poisoned cache entry (one
// whose hash still matches the file but whose stored result is wrong) is re-parsed rather than
// reused. A plain update keeps trusting it. The GRAPHIFY_FORCE=1 step runs the binary
// (`cli_update.rs`, `test_force_bypasses_cache_env`).
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_force_bypasses_cache() {
    let root = TempDir::new();
    let r = root.path();
    write_tree(r, &[("a.go", "package p\n\nfunc A() {}\n\nfunc C() {}\n")]);
    build(r, &[]);
    let graph_path = root.join("graphify-out/graph.json");
    let full = graph_node_count(&graph_path);

    // Poison the cache: keep the (correct) hash of a.go, but store the result of a version
    // without C. Any run that trusts the cache will drop C.
    let cache_path = root.join(&format!("graphify-out/{FILE_NAME}"));
    let poison = || {
        let mut c = cache::load(&cache_path, &cache_stamp(), WORKERS);
        let e = c.get_mut("a.go").expect("a.go cached");
        e.result = extract::file_from_bytes("a.go", b"package p\n\nfunc A() {}\n");
        if let Err(e) = cache::save(&cache_path, &cache_stamp(), &c) {
            panic!("save cache: {e}");
        }
    };

    poison();
    update(r, &[]);
    let got = graph_node_count(&graph_path);
    assert!(
        got < full,
        "plain update: got {got} nodes, want fewer than {full} (the poisoned entry should have been reused)"
    );

    poison();
    update(r, &["--force"]);
    let got = graph_node_count(&graph_path);
    assert_eq!(got, full, "update --force: --force must ignore the cache");
}

// A removed file re-parses nothing, but its entry must still leave the cache.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_update_drops_removed_file_from_cache() {
    let root = TempDir::new();
    let r = root.path();
    write_tree(
        r,
        &[
            ("a.go", "package p\n\nfunc Fa() {}\n"),
            ("b.go", "package p\n\nfunc Fb() {}\n"),
        ],
    );
    build(r, &[]);
    std::fs::remove_file(root.join("b.go")).expect("remove b.go");
    update(r, &[]);
    let c = cache::load(
        &root.join(&format!("graphify-out/{FILE_NAME}")),
        &cache_stamp(),
        WORKERS,
    );
    let ok = c.contains_key("b.go");
    assert!(
        !ok && c.len() == 1,
        "cache after removing b.go holds {} entries, b.go present: {ok}",
        c.len()
    );
}

// A cache stamped by another version is rebuilt once, and update says so on stderr only.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn test_update_says_stale_cache_is_rebuilt() {
    let root = TempDir::new();
    let r = root.path();
    write_tree(r, &[("a.go", "package p\n"), ("b.go", "package p\n")]);
    build(r, &[]);
    let cache_path = root.join(&format!("graphify-out/{FILE_NAME}"));
    let data = std::fs::read_to_string(&cache_path).expect("read cache");
    let old = data.replacen(&cache_stamp(), &format!("{VERSION}-s13"), 1);
    assert!(old != data, "stamp not found in the cache file");
    std::fs::write(&cache_path, old).expect("write cache");

    const NOTICE: &str = "rebuilding it once";
    let (stdout, stderr) = update(r, &[]);
    let n = stderr.matches(NOTICE).count();
    assert_eq!(
        n, 1,
        "stale cache: notice on stderr {n} times, want 1:\n{stderr}"
    );
    assert!(
        !stdout.contains(NOTICE) && stdout.contains("(2 reparsed, 0 reused"),
        "stale cache: stdout = {stdout:?}, want a full reparse and no notice"
    );
    let (stdout, stderr) = update(r, &[]);
    assert!(
        !stderr.contains(NOTICE) && stdout.contains("(0 reparsed, 2 reused"),
        "second update: stdout = {stdout:?}, stderr = {stderr:?}, want all reused and no notice"
    );
}
