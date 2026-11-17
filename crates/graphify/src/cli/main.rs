//! Port of `main.go`: the command dispatch and the build, update and query-side commands.
//! Owner: the extractors-and-CLI stream (graphify-rs-ihc.20, plan Task 20). The `cmd_*`
//! signatures are shared API.
//!
//! The query-side commands (`query`, `ask`, `explain`, `path`, `god-nodes`, `affected`, `diff`,
//! `merge-driver`, `validate`) read the graph through the `query` package, which is
//! graphify-rs-ihc.13 (plan Task 12) and not on main yet: each is ported up to its first `query`
//! call, which is [`query_load`] or a `todo!` naming the Go lines left.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufRead, Read};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use std::thread::{self, JoinHandle};

use gostd::filepath::{abs, clean, dir, eval_symlinks, is_abs, join, rel};
use gostd::strconv::{atoi, quote};
use gostd::strings::trim_space;

use super::githead::git_head;
use super::outfp::{out_flags, out_fp, outputs_current, same_files, sig_of, start_stat_check};
use super::semantic::{SemanticOpts, enrich};
use super::{Error, Out, msg, path_error};
use crate::cache::{self, Cache, Entry, Output, StatEntry, StatIndex};
use crate::detect::{self, WalkReport, go_lossy};
use crate::extract::{self, Result as ExtractResult};
use crate::model::Graph;
use crate::{STACK_BYTES, cluster, export, fsutil, graph, report, security, ts};

/// Go `version`: build metadata, injected via `-ldflags` at release time.
pub(crate) const VERSION: &str = "dev";

/// Go `commit`.
pub(crate) const COMMIT: &str = "none";

/// Go `date`.
pub(crate) const DATE: &str = "unknown";

/// Go `rootFileName`: the sidecar in the out directory that records the scanned tree.
pub(crate) const ROOT_FILE_NAME: &str = ".graphify_root";

/// Go `bigFile`: the size from which a file is parsed ahead of the rest.
pub(crate) const BIG_FILE: usize = 256 << 10;

/// Go `bigInFlight`: how many bytes of large files parse at once. A file over it parses alone.
pub(crate) const BIG_IN_FLIGHT: usize = 8 << 20;

/// Go `parseFile`'s type: the per-file parse `assemble` runs (a variable in Go so a test can
/// watch what parses side by side; a parameter of [`assemble_with`] here).
/// `'a` lets a test's closure borrow local state (a bare `dyn` alias would be `'static`).
pub(crate) type ParseFn<'a> = dyn Fn(&str, &[u8]) -> ExtractResult + Sync + 'a;

/// The worker count of every fan-out: Go's GOMAXPROCS, which is `runtime.NumCPU()` (plan R5).
fn workers() -> usize {
    thread::available_parallelism().map_or(1, |n| n.get())
}

/// A joined thread's value; its panic goes on.
fn joined<T>(r: thread::Result<T>) -> T {
    r.unwrap_or_else(|p| std::panic::resume_unwind(p))
}

/// A thread with [`STACK_BYTES`] of stack, for work that parses or walks (plan hazard "Stack
/// depth").
fn spawn_big<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> JoinHandle<T> {
    thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(f)
        .unwrap_or_else(|e| panic!("start a thread: {e}"))
}

/// The Go strings of the arguments (plan R3).
fn strs(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|a| go_lossy(a.as_encoded_bytes()))
        .collect()
}

/// A path as the Go string (plan R3).
fn path_str(p: &Path) -> String {
    go_lossy(p.as_os_str().as_encoded_bytes())
}

/// Go `os.Getenv`.
fn getenv(k: &str) -> String {
    std::env::var_os(k).map_or_else(String::new, |v| go_lossy(v.as_encoded_bytes()))
}

/// Go `os.Getwd`.
fn getwd() -> io::Result<String> {
    abs(Path::new(".")).map(|p| path_str(&p))
}

/// Whether a `filepath.Rel` result leaves its base (`..` or `../...`).
fn escapes(rel: &str) -> bool {
    rel == ".." || rel.starts_with("../")
}

/// Go `outDir`: the graphify-out directory of the read commands. `GRAPHIFY_OUT` wins; else the
/// first ancestor of the cwd holding `graphify-out/graph.json`; else `graphify-out`.
pub(crate) fn out_dir() -> String {
    let v = getenv("GRAPHIFY_OUT");
    if !v.is_empty() {
        return v;
    }
    let Ok(mut d) = getwd() else {
        return "graphify-out".to_string();
    };
    loop {
        let cand = join(&[&d, "graphify-out"]);
        if fs::metadata(join(&[&cand, "graph.json"])).is_ok() {
            return cand;
        }
        let parent = dir(&d);
        if parent == d {
            return "graphify-out".to_string();
        }
        d = parent;
    }
}

/// Go `outDirFor`: the out directory of a command that takes a root. `GRAPHIFY_OUT` wins, else
/// the root's own graphify-out; it never walks upward.
pub(crate) fn out_dir_for(root: &str) -> String {
    let v = getenv("GRAPHIFY_OUT");
    if !v.is_empty() {
        return v;
    }
    join(&[root, "graphify-out"])
}

/// Go `defaultGraphPath`.
fn default_graph_path() -> String {
    join(&[&out_dir(), "graph.json"])
}

/// Go `scanRoot`.
fn scan_root(stderr: Out<'_>) -> Result<String, Error> {
    Ok(scan_root_marked(stderr)?.0)
}

/// Go `scanRootMarked`: the source tree the resolved graph was built from, and whether it came
/// from a trusted absolute marker (not the out directory's parent fallback).
fn scan_root_marked(stderr: Out<'_>) -> Result<(String, bool), Error> {
    let out = out_dir();
    let base = dir(&out);
    let mut rec = read_root_marker(&join(&[&out, ROOT_FILE_NAME]));
    if rec.is_empty() {
        return Ok((base, false));
    }
    let is_abs_rec = is_abs(&rec);
    if !is_abs_rec {
        rec = join(&[&base, &rec]);
    }
    if !root_usable(&rec, &base) {
        w!(
            stderr,
            "warning: ignoring {ROOT_FILE_NAME} recording {} (not a directory containing the current directory, or outside the git work tree)\n",
            quote(rec.as_bytes())
        )?;
        return Ok((base, false));
    }
    Ok((rec, is_abs_rec))
}

/// Go `readRootMarker`: the recorded scan root, at most 4096 bytes read; `""` when missing.
fn read_root_marker(path: &str) -> String {
    let Ok(f) = File::open(path) else {
        return String::new();
    };
    let mut b = Vec::new();
    if f.take(4096).read_to_end(&mut b).is_err() {
        return String::new();
    }
    let s = go_lossy(&b);
    trim_space(s.strip_prefix('\u{feff}').unwrap_or(&s)).to_string()
}

/// Go `rootUsable`: whether a recorded scan root may be trusted.
fn root_usable(rec: &str, base: &str) -> bool {
    if !fs::metadata(rec).is_ok_and(|m| m.is_dir()) {
        return false;
    }
    if clean(rec) == clean(base) {
        return true;
    }
    let Ok(cwd) = getwd() else {
        return false;
    };
    // A marker shipped in a clone must not widen the scan above it.
    if let Some(top) = git_toplevel(&cwd) {
        let real = eval_symlinks(Path::new(rec)).map_or_else(|_| String::new(), |p| path_str(&p));
        if !rel(&top, &real).is_ok_and(|r| !escapes(&r)) {
            return false;
        }
    }
    rel(rec, &cwd).is_ok_and(|r| !escapes(&r))
}

/// Go `gitPath(root, "--show-toplevel")` (`maintenance.go:59`), `None` where Go errors.
fn git_toplevel(root: &str) -> Option<String> {
    let out = Command::new("git")
        .args(["-C", root, "rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let mut p = trim_space(&go_lossy(&out.stdout)).to_string();
    if !is_abs(&p) {
        p = join(&[root, &p]);
    }
    abs(Path::new(&p)).ok().map(|p| path_str(&p))
}

/// Go `assembleStats`: how an assemble run used the cache. `warnings` are the
/// `  warning: skipped <file>` lines Go prints from its workers, in file order here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AssembleStats {
    pub parsed: i64,
    pub reused: i64,
    pub dropped: i64,
    pub warnings: Vec<String>,
}

/// Go `assemble`: one result per file in `files` order, re-parsing only files whose hash differs
/// from `prev` (`None`: Go's nil cache, a cold build, which parses the large files first).
/// `workers` is Go's `runtime.NumCPU()` (R5).
pub(crate) fn assemble(
    root: &Path,
    files: &[String],
    prev: Option<&Cache>,
    prev_stat: &StatIndex,
    workers: usize,
) -> (Vec<ExtractResult>, Cache, StatIndex, AssembleStats) {
    assemble_with(
        root,
        files,
        prev,
        prev_stat,
        workers,
        &extract::file_from_bytes,
    )
}

/// One file's outcome in [`assemble_with`] (Go `slot`).
enum Slot {
    /// Not read: the warning line.
    Skipped(String),
    /// The cache entry, the stat entry, and whether the entry was reused.
    Done(Box<Entry>, StatEntry, bool),
}

/// Go `bigGate`: holds the large files back to [`BIG_IN_FLIGHT`] source bytes in flight.
#[derive(Default)]
struct BigGate {
    held: Mutex<usize>,
    free: Condvar,
}

impl BigGate {
    /// Go `bigGate.enter`; dropping the guard is `leave`.
    fn enter(&self, n: usize) -> BigHeld<'_> {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        while *held > 0 && *held + n > BIG_IN_FLIGHT {
            held = self.free.wait(held).unwrap_or_else(PoisonError::into_inner);
        }
        *held += n;
        BigHeld(self, n)
    }
}

/// `n` bytes inside a [`BigGate`], released on drop (Go `bigGate.leave`).
struct BigHeld<'a>(&'a BigGate, usize);

impl Drop for BigHeld<'_> {
    fn drop(&mut self) {
        *self.0.held.lock().unwrap_or_else(PoisonError::into_inner) -= self.1;
        self.0.free.notify_all();
    }
}

/// Go `os.ReadFile` with Go's error text (`open <path>: ...` or `read <path>: ...`).
fn read_file(path: &str) -> io::Result<Vec<u8>> {
    let p = Path::new(path);
    let mut f = File::open(p).map_err(|e| crate::goerr::path_error("open", p, &e))?;
    let mut b = Vec::new();
    f.read_to_end(&mut b)
        .map_err(|e| crate::goerr::path_error("read", p, &e))?;
    Ok(b)
}

/// [`assemble`] with Go's `parseFile` given: `assemble` is this with `extract::file_from_bytes`.
///
/// Go's goroutines read two channels: the large files (buffered) and the rest (fed in file
/// order); worker 0 drains the large files first, the others the rest first. Here each lane is
/// a list with a shared cursor, and the workers are scoped threads of [`STACK_BYTES`]. The
/// results land by index, so they do not depend on `workers` (plan Review Focus 2).
pub(crate) fn assemble_with(
    root: &Path,
    files: &[String],
    prev: Option<&Cache>,
    prev_stat: &StatIndex,
    workers: usize,
    parse: &ParseFn<'_>,
) -> (Vec<ExtractResult>, Cache, StatIndex, AssembleStats) {
    // Every kind is named before any worker parses (plan hazard "KindMask registration order").
    extract::kinds::register_kinds();
    let workers = workers.min(files.len()).max(1);
    // Cold build only: an update parses few files and already stats each.
    let big = if prev.is_none() {
        large_first(root, files)
    } else {
        Vec::new()
    };
    let mut is_big = vec![false; files.len()];
    for &i in &big {
        is_big[i] = true;
    }
    let rest: Vec<usize> = (0..files.len()).filter(|&i| !is_big[i]).collect();
    let lanes = [
        (&rest[..], AtomicUsize::new(0)),
        (&big[..], AtomicUsize::new(0)),
    ];
    let gate = BigGate::default();
    let root_s = path_str(root);
    let no_stat = StatEntry::default();

    let one = |i: usize| -> Slot {
        let key = &files[i];
        let (ps, ps_ok) = prev_stat.get(key).map_or((&no_stat, false), |p| (p, true));
        let Some((h, stat, src)) = cache::hash_file(&root.join(key), ps, ps_ok) else {
            return Slot::Skipped(format!("  warning: skipped {key}\n"));
        };
        if let Some(e) = prev.and_then(|p| p.get(key))
            && e.hash == h
        {
            return Slot::Done(Box::new(e.clone()), stat, true);
        }
        // Not a cache hit, so it must parse. If the stat fastpath skipped the read, read now.
        let src = match src {
            Some(b) => b,
            None => match read_file(&join(&[&root_s, key])) {
                Ok(b) => b,
                Err(e) => return Slot::Skipped(format!("  warning: skipped {key} ({e})\n")),
            },
        };
        let held = is_big[i].then(|| gate.enter(src.len()));
        let result = parse(key.as_str(), src.as_slice());
        drop(held);
        Slot::Done(Box::new(Entry { hash: h, result }), stat, false)
    };

    let mut slots: Vec<Option<Slot>> = (0..files.len()).map(|_| None).collect();
    thread::scope(|s| {
        let (one, lanes) = (&one, &lanes);
        let handles: Vec<_> = (0..workers)
            .map(|w| {
                // One worker starts on the large files while the rest start on the others; the
                // gate keeps their parsers from adding up side by side.
                let order = if w == 0 { [1, 0] } else { [0, 1] };
                thread::Builder::new()
                    .stack_size(STACK_BYTES)
                    .spawn_scoped(s, move || {
                        let mut done = Vec::new();
                        for l in order {
                            let (idx, next) = &lanes[l];
                            while let Some(&i) = idx.get(next.fetch_add(1, Ordering::Relaxed)) {
                                done.push((i, one(i)));
                            }
                        }
                        done
                    })
                    .unwrap_or_else(|e| panic!("start an assemble worker: {e}"))
            })
            .collect();
        for h in handles {
            for (i, slot) in joined(h.join()) {
                slots[i] = Some(slot);
            }
        }
    });

    let mut results = Vec::with_capacity(files.len());
    let mut new_cache = Cache::new();
    let mut new_stat = StatIndex::new();
    let mut stats = AssembleStats::default();
    for (key, slot) in files.iter().zip(slots) {
        match slot {
            None => {}
            Some(Slot::Skipped(w)) => stats.warnings.push(w),
            Some(Slot::Done(entry, stat, reused)) => {
                results.push(entry.result.clone());
                new_cache.insert(key.clone(), *entry);
                // Skip files hash_file could not stat (empty hash) so the index keeps only
                // entries that can match on a later run.
                if !stat.hash.is_empty() {
                    new_stat.insert(key.clone(), stat);
                }
                if reused {
                    stats.reused += 1;
                } else {
                    stats.parsed += 1;
                }
            }
        }
    }
    if let Some(prev) = prev {
        stats.dropped = prev.keys().filter(|f| !new_cache.contains_key(*f)).count() as i64;
    }
    (results, new_cache, new_stat, stats)
}

/// Go `largeFirst`: the indexes of the files of [`BIG_FILE`] bytes or more, largest first.
pub(crate) fn large_first(root: &Path, files: &[String]) -> Vec<usize> {
    // As cache.HashFile: more stat threads only add time in the kernel.
    const WORKERS: usize = 4;
    let mut sizes = vec![0u64; files.len()];
    thread::scope(|s| {
        let handles: Vec<_> = (0..WORKERS)
            .map(|w| {
                s.spawn(move || {
                    (w..files.len())
                        .step_by(WORKERS)
                        .map(|i| (i, fs::metadata(root.join(&files[i])).map_or(0, |m| m.len())))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in handles {
            for (i, n) in joined(h.join()) {
                sizes[i] = n;
            }
        }
    });
    let mut big: Vec<usize> = (0..files.len())
        .filter(|&i| sizes[i] >= BIG_FILE as u64)
        .collect();
    big.sort_by_key(|&i| std::cmp::Reverse(sizes[i]));
    big
}

/// Go `cacheStamp`: `cache::stamp(VERSION)` plus `-b<parse budget>`.
pub(crate) fn cache_stamp() -> String {
    format!("{}-b{}", cache::stamp(VERSION), ts::budget())
}

/// The cache encode of [`write_outputs`], running beside the build (Go's `saved` channel).
struct PendingCache(Option<JoinHandle<Result<(), cache::Error>>>);

impl PendingCache {
    /// Go `<-saved`.
    fn wait(&mut self) -> Result<(), Error> {
        match self.0.take() {
            Some(h) => joined(h.join()).map_err(msg),
            None => Ok(()),
        }
    }
}

/// Go `writeOutputs`: resolves `results` into a graph and writes graph.json, GRAPH_REPORT.md, the
/// cache and the stat sidecar under the out directory of `root`. A refused shrink is a warning
/// on `stderr` and `Ok`, with nothing written. `workers` is the fan-out of the build (R5).
#[allow(clippy::too_many_arguments)] // Go's parameter list
pub(crate) fn write_outputs(
    root: &str,
    walk: &WalkReport,
    results: Vec<ExtractResult>,
    mut new_cache: Cache,
    new_stat: StatIndex,
    sem: &SemanticOpts,
    force: bool,
    no_cluster: bool,
    keep_cache: bool,
    flags: &str,
    workers: usize,
    stdout: Out<'_>,
    stderr: Out<'_>,
) -> Result<(Graph, BTreeMap<i64, Vec<String>>), Error> {
    let files = &walk.files;
    let out_dir = out_dir_for(root);
    fs::create_dir_all(&out_dir).map_err(|e| path_error("mkdir", Path::new(&out_dir), &e))?;
    let cache_path = join(&[&out_dir, cache::FILE_NAME]);
    let pending = format!("{cache_path}.pending");
    // Taken now: the cache encode below empties new_cache.
    let skipped = skipped_files(files, &new_cache);
    let mut over_budget = 0;
    for r in &results {
        if r.over_budget {
            over_budget += 1;
            w!(
                stderr,
                "  warning: skipped {} (parser made no progress, GRAPHIFY_PARSE_BUDGET={}; 0 turns the limit off)\n",
                r.file,
                ts::budget()
            )?;
        }
    }
    let (mut nodes, mut edges) = extract::resolve_parts(&results, files, workers);
    drop(results);
    // The cache is encoded beside build and cluster; the file is only moved into place after
    // graph.json and the report are written, and a run that writes nothing drops it.
    let mut saved = PendingCache(if keep_cache {
        new_cache.clear();
        None
    } else {
        let pending = pending.clone();
        Some(thread::spawn(move || {
            cache::save_drain(Path::new(&pending), &cache_stamp(), &mut new_cache)
        }))
    });
    let drop_cache = |saved: &mut PendingCache| {
        let _ = saved.wait();
        let _ = fs::remove_file(&pending);
    };
    let force = force || env_force();
    let graph_path = join(&[&out_dir, "graph.json"]);
    let guard = if force {
        None
    } else {
        export::read_shrink_guard(Path::new(&graph_path))
    };
    if sem.enabled {
        let ext = match enrich(root, extract::join(nodes, edges), sem, stdout, stderr) {
            Ok(ext) => ext,
            Err(e) => {
                drop_cache(&mut saved);
                return Err(e);
            }
        };
        (nodes, edges) = (vec![ext.nodes], vec![ext.edges]);
    }
    let mut g = graph::build_parts(nodes, edges, workers);
    g.unclassified = walk.skipped_exts.clone();
    g.over_budget = over_budget;
    // --no-cluster writes the raw extraction: every node lands with no community.
    let communities = if no_cluster {
        BTreeMap::new()
    } else {
        cluster::cluster(&g)
    };
    let commit = git_head(Path::new(root));

    // Anti-shrink guard first, before any output is written: on a refused shrink every output
    // is left as it is, so graphify-out stays consistent.
    if let Some(guard) = &guard
        && let Err(e) = guard.check(&g, &skipped, root)
    {
        drop_cache(&mut saved);
        // A shrink/unverifiable refusal is a fail-safe, not a build failure.
        return match e {
            export::Error::GraphShrink(_) | export::Error::GraphUnverifiable(_) => {
                w!(stderr, "[graphify] WARNING: {e}\n")?;
                Ok((g, communities))
            }
            e => Err(msg(e)),
        };
    }
    // The report only reads the graph, so it is made while graph.json is encoded.
    let (mut res, md) = thread::scope(|s| {
        let md = thread::Builder::new()
            .stack_size(STACK_BYTES)
            .spawn_scoped(s, || report::generate(&g, &communities, root, &commit))
            .unwrap_or_else(|e| panic!("start the report thread: {e}"));
        let res = export::to_json(
            &g,
            &communities,
            Path::new(&graph_path),
            &commit,
            true,
            workers,
        )
        .map_err(msg);
        (res, joined(md.join()))
    });
    let json_sig = sig_of(Path::new(&graph_path));
    let report_path = join(&[&out_dir, "GRAPH_REPORT.md"]);
    if res.is_ok() {
        res = fsutil::write_file_atomic(Path::new(&report_path), md.as_bytes(), 0o644).map_err(msg);
    }
    let report_sig = sig_of(Path::new(&report_path));
    if let Err(e) = saved.wait()
        && res.is_ok()
    {
        res = Err(e);
    }
    if res.is_ok() && !keep_cache {
        res = fs::rename(&pending, &cache_path).map_err(|e| {
            msg(crate::goerr::link_error(
                "rename",
                Path::new(&pending),
                Path::new(&cache_path),
                &e,
            ))
        });
    }
    if let Err(e) = res {
        let _ = fs::remove_file(&pending);
        return Err(e);
    }
    // A changed output means another run raced this one. Over-budget warnings must repeat each
    // update.
    let out = (!sem.enabled
        && over_budget == 0
        && sig_of(Path::new(&graph_path)) == json_sig
        && sig_of(Path::new(&report_path)) == report_sig)
        .then(|| Output {
            fp: out_fp(root, walk, &commit, flags),
            nodes: g.num_nodes() as i64,
            edges: g.num_edges() as i64,
            communities: communities.len() as i64,
        });
    cache::save_stat(
        Path::new(&join(&[&out_dir, cache::STAT_FILE_NAME])),
        &cache_stamp(),
        &new_stat,
        out.as_ref(),
    )
    .map_err(msg)?;
    // Record the scanned tree, absolute, so `update` from a subdirectory (or with a relocated
    // GRAPHIFY_OUT) refreshes this graph.
    let abs_root = abs(Path::new(root)).map_or_else(|_| root.to_string(), |p| path_str(&p));
    let marker = join(&[&out_dir, ROOT_FILE_NAME]);
    fs::write(&marker, format!("{abs_root}\n"))
        .map_err(|e| path_error("open", Path::new(&marker), &e))?;
    Ok((g, communities))
}

/// Go `envForce`: `GRAPHIFY_FORCE` set to 1, true or yes (any case, spaces trimmed).
fn env_force() -> bool {
    matches!(
        gostd::strings::to_lower(trim_space(&getenv("GRAPHIFY_FORCE"))).as_str(),
        "1" | "true" | "yes"
    )
}

/// Go `skippedFiles`: the files of the walk with no entry in `new_cache` (they failed to process
/// this run).
fn skipped_files(files: &[String], new_cache: &Cache) -> BTreeSet<String> {
    files
        .iter()
        .filter(|f| !new_cache.contains_key(*f))
        .cloned()
        .collect()
}

/// Go `warnWalkReport`: a warning per unreadable directory, and a note counting the files with
/// no extractor.
fn warn_walk_report(rep: &WalkReport, stderr: Out<'_>) -> Result<(), Error> {
    for we in &rep.walk_errors {
        w!(
            stderr,
            "[graphify] WARNING: could not scan {we}; its files are missing from this run's enumeration.\n"
        )?;
    }
    if rep.skipped > 0 {
        w!(
            stderr,
            "[graphify] note: {} file(s) skipped (no extractor for their type).\n",
            rep.skipped
        )?;
    }
    Ok(())
}

/// The error of a walk that found nothing.
fn no_files(root: &str) -> Error {
    Error::Msg(format!("no supported source files found under {root}"))
}

/// Go `cmdBuild`: `build [path] [--cargo] [--no-manifests] [--force] [--no-cluster]
/// [--semantic --backend X]`.
pub fn cmd_build(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    cmd_build_with(args, workers(), stdout, stderr)
}

/// [`cmd_build`] with the worker count given (`cmd_build` passes `available_parallelism`), so a
/// test can build with 1 and with n workers (R5, Go's GOMAXPROCS tests).
pub(crate) fn cmd_build_with(
    args: &[OsString],
    workers: usize,
    stdout: Out<'_>,
    stderr: Out<'_>,
) -> Result<(), Error> {
    let opts = parse_build_opts(args)?;
    let root = &opts.root;
    let rep = detect::collect_files_report(Path::new(root), workers).map_err(msg)?;
    let files = &rep.files;
    warn_walk_report(&rep, stderr)?;
    if files.is_empty() {
        return Err(no_files(root));
    }
    let mut manifests = start_manifests(root, opts.no_manifests);
    let (mut results, new_cache, new_stat, stats) =
        assemble(Path::new(root), files, None, &StatIndex::new(), workers);
    for w in &stats.warnings {
        w!(stderr, "{w}")?;
    }
    if opts.cargo {
        results = with_cargo(root, results)?;
    }
    let results = manifests.append(results)?;
    let flags = out_flags(opts.no_manifests, opts.cargo, opts.no_cluster, &results);
    let (g, communities) = write_outputs(
        root,
        &rep,
        results,
        new_cache,
        new_stat,
        &opts.semantic_opts(),
        opts.force,
        opts.no_cluster,
        false,
        &flags,
        workers,
        stdout,
        stderr,
    )?;
    w!(
        stdout,
        "built graph: {} files · {} nodes · {} edges · {} communities → {}\n",
        files.len(),
        g.num_nodes(),
        g.num_edges(),
        communities.len(),
        out_dir_for(root)
    )
}

/// Go `buildOpts`: the parsed build/update arguments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BuildOpts {
    pub root: String,
    /// An explicit path argument was given (vs the `.` default).
    pub root_set: bool,
    pub cargo: bool,
    pub no_manifests: bool,
    pub semantic: bool,
    pub backend: String,
    pub force: bool,
    pub no_cluster: bool,
}

impl BuildOpts {
    /// Go `buildOpts.semanticOpts`.
    fn semantic_opts(&self) -> SemanticOpts {
        SemanticOpts {
            enabled: self.semantic,
            backend: self.backend.clone(),
        }
    }
}

/// Go `parseBuildOpts` returning both of Go's values: the options parsed so far and the error.
fn parse_build_opts_partial(args: &[OsString]) -> (BuildOpts, Option<Error>) {
    let args = strs(args);
    let mut opts = BuildOpts {
        root: ".".to_string(),
        ..BuildOpts::default()
    };
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "--cargo" => opts.cargo = true,
            "--no-manifests" => opts.no_manifests = true,
            "--force" => opts.force = true,
            "--no-cluster" => opts.no_cluster = true,
            "--semantic" => opts.semantic = true,
            "--backend" => {
                let Some(v) = args.get(i + 1) else {
                    let e = "--backend requires a value (e.g. --backend bedrock)";
                    return (opts, Some(Error::Msg(e.to_string())));
                };
                opts.backend = v.clone();
                i += 1;
            }
            _ => {
                if let Some(v) = a.strip_prefix("--backend=") {
                    opts.backend = v.to_string();
                } else {
                    opts.root = a.to_string();
                    opts.root_set = true;
                }
            }
        }
        i += 1;
    }
    if opts.semantic && opts.backend.is_empty() {
        let e = "--semantic requires an explicit --backend (e.g. --backend bedrock)";
        return (opts, Some(Error::Msg(e.to_string())));
    }
    (opts, None)
}

/// Go `parseBuildOpts`: `--semantic` without `--backend` is an error.
pub(crate) fn parse_build_opts(args: &[OsString]) -> Result<BuildOpts, Error> {
    match parse_build_opts_partial(args) {
        (_, Some(e)) => Err(e),
        (opts, None) => Ok(opts),
    }
}

/// Go `parseBuildArgs`: `(root, cargo, no_manifests, force, no_cluster)`.
#[allow(dead_code)] // as in Go, only its test (`cargo_test.go`) calls it
pub(crate) fn parse_build_args(args: &[OsString]) -> (String, bool, bool, bool, bool) {
    let (o, _) = parse_build_opts_partial(args);
    (o.root, o.cargo, o.no_manifests, o.force, o.no_cluster)
}

/// Go `withCargo`: the Cargo pass's result appended to `results`.
fn with_cargo(root: &str, mut results: Vec<ExtractResult>) -> Result<Vec<ExtractResult>, Error> {
    results.push(extract::introspect_cargo(Path::new(root)).map_err(msg)?);
    Ok(results)
}

/// Go `startManifests`: the package-manifest pass, on a thread while the caller parses; `None`
/// with `--no-manifests`.
struct Manifests {
    run: Option<JoinHandle<Result<ExtractResult, String>>>,
    done: Option<Result<ExtractResult, String>>,
}

fn start_manifests(root: &str, skip: bool) -> Manifests {
    let root = root.to_string();
    let run = (!skip).then(|| {
        spawn_big(move || {
            let root = Path::new(&root);
            let files = detect::collect_manifests(root).map_err(|e| e.to_string())?;
            Ok(extract::introspect_manifests(root, &files))
        })
    });
    Manifests { run, done: None }
}

impl Manifests {
    /// The func Go's `startManifests` returns: waits for the pass and appends its result. It
    /// may be called more than once (`update`'s no-change check, then the full path).
    fn append(&mut self, mut results: Vec<ExtractResult>) -> Result<Vec<ExtractResult>, Error> {
        if let Some(h) = self.run.take() {
            self.done = Some(joined(h.join()));
        }
        match &self.done {
            None => Ok(results),
            Some(Ok(r)) => {
                results.push(r.clone());
                Ok(results)
            }
            Some(Err(e)) => Err(Error::Msg(e.clone())),
        }
    }
}

/// Go `updateRoot`: the tree `update` really scans (`watch` polls the same root).
pub(crate) fn update_root(root: &str, root_set: bool, stderr: Out<'_>) -> Result<String, Error> {
    if !root_set {
        // No explicit target: update the graph the read commands would load.
        let sr = scan_root(stderr)?;
        // Build titles the cwd ".", so update does too.
        if getwd().is_ok_and(|wd| clean(&sr) == wd) {
            return Ok(".".to_string());
        }
        return Ok(sr);
    }
    if fs::metadata(join(&[root, "graphify-out", "graph.json"])).is_err() {
        // An explicit path inside the recorded scan root must not fork a partial graph or
        // shrink the repo one.
        let (mut sr, marked) = scan_root_marked(stderr)?;
        // With GRAPHIFY_OUT the parent fallback is not a scan root.
        if !getenv("GRAPHIFY_OUT").is_empty() && !marked {
            sr = root.to_string();
        }
        if let Ok(a) = abs(Path::new(root))
            && let Ok(r) = rel(&sr, &path_str(&a))
            && !escapes(&r)
        {
            // The scan root itself keeps the title build gave it.
            return Ok(if r == "." { root.to_string() } else { sr });
        }
    }
    Ok(root.to_string())
}

/// Go `cmdUpdate`: `update [path]` with the `build` flags.
pub fn cmd_update(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    let workers = workers();
    let (opts, _) = parse_build_opts_partial(args);
    let (cargo, no_manifests, force, no_cluster) =
        (opts.cargo, opts.no_manifests, opts.force, opts.no_cluster);
    let root = update_root(&opts.root, opts.root_set, stderr)?;
    // Started here so the walk and the cache load hide it.
    let check = start_stat_check(&root, !cargo && !force && !env_force(), workers);
    let mut manifests = start_manifests(&root, no_manifests);
    let rep = detect::collect_files_report(Path::new(&root), workers).map_err(msg)?;
    let files = &rep.files;
    warn_walk_report(&rep, stderr)?;
    if files.is_empty() {
        return Err(no_files(&root));
    }
    let (prev_stat, out) = check.map(|h| joined(h.join())).unwrap_or_default();
    if let Some(out) = &out
        && same_files(files, &prev_stat)
        // Fall through on a manifest error: the full path reports it.
        && let Ok(mres) = manifests.append(Vec::new())
        && outputs_current(
            &root,
            &rep,
            out,
            &out_flags(no_manifests, cargo, no_cluster, &mres),
        )
    {
        return w!(
            stdout,
            "updated graph: {} files (0 reparsed, {} reused, 0 removed) · {} nodes · {} edges · {} communities → {}\n",
            files.len(),
            files.len(),
            out.nodes,
            out.edges,
            out.communities,
            out_dir_for(&root)
        );
    }
    let out_dir = out_dir_for(&root);
    let cache_path = join(&[&out_dir, cache::FILE_NAME]);
    let mut prev = Some(cache::load(Path::new(&cache_path), &cache_stamp(), workers));
    if cache::stale(Path::new(&cache_path), &cache_stamp()) {
        w!(
            stderr,
            "[graphify] note: the cache was written by another graphify version or with other settings; rebuilding it once.\n"
        )?;
    }
    let stat_path = join(&[&out_dir, cache::STAT_FILE_NAME]);
    let mut prev_stat = cache::load_stat(Path::new(&stat_path), &cache_stamp());
    // --force / GRAPHIFY_FORCE means a full re-scan: drop the caches so every file is re-read
    // and re-parsed.
    if force || env_force() {
        (prev, prev_stat) = (None, StatIndex::new());
    }
    let (mut results, new_cache, new_stat, stats) =
        assemble(Path::new(&root), files, prev.as_ref(), &prev_stat, workers);
    for w in &stats.warnings {
        w!(stderr, "{w}")?;
    }
    if cargo {
        results = with_cargo(&root, results)?;
    }
    let results = manifests.append(results)?;
    // Nothing parsed and nothing dropped: every entry came from the cache file and the file has
    // no other, so encoding it again would write the same.
    let keep_cache = prev.is_some() && stats.parsed == 0 && stats.dropped == 0;
    drop(prev);
    let flags = out_flags(no_manifests, cargo, no_cluster, &results);
    let (g, communities) = write_outputs(
        &root,
        &rep,
        results,
        new_cache,
        new_stat,
        &SemanticOpts::default(),
        force,
        no_cluster,
        keep_cache,
        &flags,
        workers,
        stdout,
        stderr,
    )?;
    w!(
        stdout,
        "updated graph: {} files ({} reparsed, {} reused, {} removed) · {} nodes · {} edges · {} communities → {}\n",
        files.len(),
        stats.parsed,
        stats.reused,
        stats.dropped,
        g.num_nodes(),
        g.num_edges(),
        communities.len(),
        out_dir_for(&root)
    )
}

/// Go `query.Load`. The query package is graphify-rs-ihc.13 (plan Task 12) and not on main yet:
/// this stands where Go loads the graph.
fn query_load(_path: &str) -> Result<(), Error> {
    todo!("graphify-rs-ihc.13: query.Load")
}

/// Go `cmdQuery`: `query <pattern>` (`run` checks the pattern is there).
pub fn cmd_query(args: &[OsString], _stdout: Out<'_>, _stderr: Out<'_>) -> Result<(), Error> {
    let _pattern = go_lossy(args[0].as_encoded_bytes());
    query_load(&default_graph_path())?;
    todo!("graphify-rs-ihc.13: query.Query and the printout (main.go:860-871)")
}

/// Go `strconv.Atoi` of a `--budget` value: a positive integer.
fn ask_budget(s: &str) -> Result<i64, Error> {
    let n = atoi(s).map_err(|_| Error::Msg("--budget must be an integer".to_string()))?;
    if n <= 0 {
        return Err(Error::Msg(
            "--budget must be a positive integer".to_string(),
        ));
    }
    Ok(n)
}

/// Go `cmdAsk`: `ask "<question>" [--dfs] [--budget N] [--context REL] [--graph path]`.
pub fn cmd_ask(args: &[OsString], _stdout: Out<'_>, _stderr: Out<'_>) -> Result<(), Error> {
    let args = strs(args);
    let Some(question) = args.first() else {
        return Err(Error::Msg(
            r#"usage: graphify ask "<question>" [--dfs] [--budget N] [--context REL] [--graph path]"#
                .to_string(),
        ));
    };
    let (mut dfs, mut budget, mut relations) = (false, 2000, Vec::new());
    let mut graph_path = default_graph_path();
    let rest = &args[1..];
    let mut i = 0;
    while i < rest.len() {
        let a = rest[i].as_str();
        let next = rest.get(i + 1);
        if a == "--dfs" {
            dfs = true;
        } else if let (Some(v), "--budget") = (next, a) {
            budget = ask_budget(v)?;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--budget=") {
            budget = ask_budget(v)?;
        } else if let (Some(v), "--context") = (next, a) {
            relations.push(v.clone());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--context=") {
            relations.push(v.to_string());
        } else if let (Some(v), "--graph") = (next, a) {
            graph_path = v.clone();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--graph=") {
            graph_path = v.to_string();
        }
        i += 1;
    }
    if graph_path != default_graph_path() {
        graph_path = safe_graph_path(&graph_path)?;
    }
    query_load(&graph_path)?;
    let _ = (question, dfs, budget, relations);
    todo!("graphify-rs-ihc.13: query.Ask and the printout (main.go:934)")
}

/// Go `safeGraphPath`: a user-supplied `--graph` path must resolve inside the graphify-out
/// directory.
fn safe_graph_path(path: &str) -> Result<String, Error> {
    security::validate_graph_path(path, &out_dir()).map_err(msg)
}

/// Go `cmdExplain`: `explain <node> [--graph path]`.
pub fn cmd_explain(args: &[OsString], _stdout: Out<'_>, _stderr: Out<'_>) -> Result<(), Error> {
    let (positionals, graph_path) = parse_graph_flag(strs(args));
    if positionals.len() != 1 {
        return Err(Error::Msg(
            "usage: graphify explain <node> [--graph path]".to_string(),
        ));
    }
    load_graph_at(graph_path)?;
    todo!("graphify-rs-ihc.13: query.Explain, explainLines and the printout (main.go:954-1025)")
}

/// Go `cmdPath`: `path <from> <to> [--undirected] [--graph path]`.
pub fn cmd_path(args: &[OsString], _stdout: Out<'_>, _stderr: Out<'_>) -> Result<(), Error> {
    let mut args = strs(args);
    let n = args.len();
    args.retain(|a| a != "--undirected");
    let undirected = args.len() != n;
    let (positionals, graph_path) = parse_graph_flag(args);
    if positionals.len() != 2 {
        return Err(Error::Msg(
            "usage: graphify path <from> <to> [--undirected] [--graph path]".to_string(),
        ));
    }
    load_graph_at(graph_path)?;
    let _ = undirected;
    todo!("graphify-rs-ihc.13: query.PathEdges and renderPathChain (main.go:1045-1053)")
}

/// Go `cmdGodNodes`: `god-nodes [--top N] [--graph path] [--json]`.
pub fn cmd_god_nodes(args: &[OsString], _stdout: Out<'_>, _stderr: Out<'_>) -> Result<(), Error> {
    let (positionals, graph_path) = parse_graph_flag(strs(args));
    let (mut top, mut as_json) = (10, false);
    let mut i = 0;
    while i < positionals.len() {
        let a = positionals[i].as_str();
        let val = if a == "--json" {
            as_json = true;
            i += 1;
            continue;
        } else if let (Some(v), "--top") = (positionals.get(i + 1), a) {
            i += 1;
            v.as_str()
        } else if let Some(v) = a.strip_prefix("--top=") {
            v
        } else {
            return Err(Error::Msg(
                "usage: graphify god-nodes [--top N] [--graph path] [--json]".to_string(),
            ));
        };
        match atoi(val) {
            Ok(n) if n >= 1 => top = n,
            _ => {
                return Err(Error::Msg("--top must be a positive integer".to_string()));
            }
        }
        i += 1;
    }
    load_graph_at(graph_path)?;
    let _ = (top, as_json);
    todo!("graphify-rs-ihc.13: analyze.GodNodes(modelOf(g)) and the printout (main.go:1087-1104)")
}

/// Go `parseGraphFlag`: the positionals and the `--graph` path (default `default_graph_path`).
fn parse_graph_flag(args: Vec<String>) -> (Vec<String>, String) {
    let mut graph_path = default_graph_path();
    let mut positionals = Vec::new();
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        // A trailing `--graph` has no value and is a positional, as in Go.
        if a == "--graph"
            && let Some(v) = args.next()
        {
            graph_path = v;
        } else if let Some(v) = a.strip_prefix("--graph=") {
            graph_path = v.to_string();
        } else {
            positionals.push(a);
        }
    }
    (positionals, graph_path)
}

/// Go `loadGraphAt`: the containment guard for a path other than the default, then the load.
fn load_graph_at(graph_path: String) -> Result<(), Error> {
    let graph_path = if graph_path != default_graph_path() {
        safe_graph_path(&graph_path)?
    } else {
        graph_path
    };
    query_load(&graph_path)
}

/// Go `cmdExtract`: `extract <file>` (`run` checks the file is there).
pub fn cmd_extract(args: &[OsString], stdout: Out<'_>, _stderr: Out<'_>) -> Result<(), Error> {
    let file = go_lossy(args[0].as_encoded_bytes());
    let (mut root, mut name) = (".".to_string(), file.clone());
    let d = dir(&file);
    if d != "." {
        (root, name) = (d, gostd::filepath::base(&file));
    }
    let r = extract::file(Path::new(&root), &name).map_err(msg)?;
    let ext = extract::resolve(&[r], &[name], workers());
    for n in &ext.nodes {
        w!(
            stdout,
            "node  {:<40} {} {}\n",
            n.label,
            n.source_file,
            n.source_location
        )?;
    }
    for e in &ext.edges {
        w!(
            stdout,
            "edge  {:<12} {} -> {}\n",
            e.relation.as_str(),
            e.source,
            e.target
        )?;
    }
    Ok(())
}

/// Go `cmdExport`: `export <graphml|dot|csv|okf> [path]` (`run` checks the format is there).
pub fn cmd_export(args: &[OsString], stdout: Out<'_>, _stderr: Out<'_>) -> Result<(), Error> {
    let args = strs(args);
    let format = args[0].as_str();
    let root = args.get(1).map_or(".", String::as_str);
    let out_dir = out_dir_for(root);
    let json_path = join(&[&out_dir, "graph.json"]);
    if fs::metadata(&json_path).is_err() {
        return Err(Error::Msg(format!(
            "no graph at {json_path} — run `graphify build` first"
        )));
    }
    let jp = Path::new(&json_path);
    match format {
        "graphml" => {
            let out = join(&[&out_dir, "graph.graphml"]);
            export::graphml_from_json(jp, Path::new(&out)).map_err(msg)?;
            w!(stdout, "wrote {out}\n")
        }
        "dot" => {
            let out = join(&[&out_dir, "graph.dot"]);
            export::dot_from_json(jp, Path::new(&out)).map_err(msg)?;
            w!(stdout, "wrote {out}\n")
        }
        "csv" => {
            let nodes = join(&[&out_dir, "graph.nodes.csv"]);
            let edges = join(&[&out_dir, "graph.edges.csv"]);
            export::csv_from_json(jp, Path::new(&nodes), Path::new(&edges)).map_err(msg)?;
            w!(stdout, "wrote {nodes} and {edges}\n")
        }
        "okf" => {
            let bundle = join(&[&out_dir, "okf"]);
            export::okf_from_json(jp, Path::new(&bundle)).map_err(msg)?;
            w!(stdout, "wrote OKF bundle to {bundle}\n")
        }
        _ => Err(Error::Msg(format!(
            "unknown export format {} (want: graphml, dot, csv, okf)",
            quote(format.as_bytes())
        ))),
    }
}

/// Go `readPathList`: the non-blank, space-trimmed lines of `r` (`affected -`).
pub(crate) fn read_path_list(r: impl BufRead) -> Vec<String> {
    r.split(b'\n')
        .map_while(Result::ok)
        .map(|line| trim_space(&go_lossy(&line)).to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Go `cmdAffected`: `affected [file...|-] [--depth N] [--relation R]`.
pub fn cmd_affected(args: &[OsString], stdout: Out<'_>, _stderr: Out<'_>) -> Result<(), Error> {
    query_load(&default_graph_path())?;
    let args = strs(args);
    let (mut files, mut relations, mut depth) = (Vec::new(), Vec::new(), 0);
    let depth_of =
        |s: &str| atoi(s).map_err(|_| Error::Msg("--depth must be an integer".to_string()));
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let next = args.get(i + 1);
        if let (Some(v), "--depth") = (next, a) {
            depth = depth_of(v.as_str())?;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--depth=") {
            depth = depth_of(v)?;
        } else if let (Some(v), "--relation") = (next, a) {
            relations.push(v.clone());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--relation=") {
            relations.push(v.to_string());
        } else if a == "-" {
            files.extend(read_path_list(io::stdin().lock()));
        } else {
            files.push(a.to_string());
        }
        i += 1;
    }
    if files.is_empty() {
        files = git_changed_files(".");
        if files.is_empty() {
            return Err(Error::Msg(
                "no files given and no uncommitted changes detected (usage: graphify affected [file...|-] [--depth N] [--relation R])"
                    .to_string(),
            ));
        }
        w!(stdout, "changed files (from git): {}\n", files.join(", "))?;
    }
    let _ = (files, depth, relations);
    todo!("graphify-rs-ihc.13: query.Affected and the printout (main.go:1258-1271)")
}

/// Go `cmdDiff`: `diff <old.json> <new.json> [--json]`.
pub fn cmd_diff(args: &[OsString], _stdout: Out<'_>, _stderr: Out<'_>) -> Result<(), Error> {
    let mut as_json = false;
    let mut paths = Vec::new();
    for a in strs(args) {
        if a == "--json" {
            as_json = true;
        } else {
            paths.push(a);
        }
    }
    if paths.len() != 2 {
        return Err(Error::Msg(
            "usage: graphify diff <old.json> <new.json> [--json]".to_string(),
        ));
    }
    for p in &mut paths {
        *p = safe_graph_path(p)?;
    }
    query_load(&paths[0])?;
    query_load(&paths[1])?;
    let _ = as_json;
    todo!("graphify-rs-ihc.13: query.Diff and the printout (main.go:1306-1335)")
}

/// Go `cmdMergeDriver`: `merge-driver <base> <current> <other>`.
pub fn cmd_merge_driver(
    args: &[OsString],
    _stdout: Out<'_>,
    _stderr: Out<'_>,
) -> Result<(), Error> {
    if args.len() < 3 {
        return Err(Error::Msg(
            "usage: graphify merge-driver <base> <current> <other>".to_string(),
        ));
    }
    let _ = (&args[1], &args[2]);
    todo!("graphify-rs-ihc.13: query.Merge(current, other) (main.go:1349)")
}

/// Go `cmdValidate`: `validate`.
pub fn cmd_validate(_args: &[OsString], _stdout: Out<'_>, _stderr: Out<'_>) -> Result<(), Error> {
    query_load(&default_graph_path())?;
    todo!("graphify-rs-ihc.13: query.Validate and the printout (main.go:1355-1372)")
}

/// Go `gitChangedFiles`: the files that differ from HEAD in the working tree; none if git fails.
fn git_changed_files(root: &str) -> Vec<String> {
    let Ok(out) = Command::new("git")
        .args(["-C", root, "diff", "--name-only", "HEAD"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    trim_space(&go_lossy(&out.stdout))
        .split('\n')
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
#[path = "tests/affected_stdin.rs"]
mod affected_stdin_tests;
#[cfg(test)]
#[path = "tests/ask.rs"]
mod ask_tests;
#[cfg(test)]
#[path = "tests/budget.rs"]
mod budget_tests;
#[cfg(test)]
#[path = "tests/cargo.rs"]
mod cargo_tests;
#[cfg(test)]
#[path = "tests/core.rs"]
mod core_tests;
#[cfg(test)]
#[path = "tests/core_util.rs"]
pub(crate) mod core_util;
#[cfg(test)]
#[path = "tests/largefirst.rs"]
mod largefirst_tests;
#[cfg(test)]
#[path = "tests/nocluster.rs"]
mod nocluster_tests;
#[cfg(test)]
#[path = "tests/semantic.rs"]
mod semantic_tests;
#[cfg(test)]
#[path = "tests/update.rs"]
mod update_tests;
#[cfg(test)]
#[path = "tests/upstream_out_dir_evidence.rs"]
mod upstream_out_dir_evidence_tests;
#[cfg(test)]
#[path = "tests/upstream_update.rs"]
mod upstream_update_tests;
