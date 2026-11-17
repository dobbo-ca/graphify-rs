//! Port of `outfp.go`: the fingerprint of a build's outputs, so a no-change `update` skips the
//! rewrite.

use std::fmt::Write as _;
use std::fs::{self, Metadata};
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;

use gostd::filepath::{abs, join};
use gostd::strconv::quote;

use super::githead::git_head;
use super::main::{ROOT_FILE_NAME, cache_stamp, out_dir_for};
use crate::cache::{self, Output, StatIndex};
use crate::detect::{WalkReport, go_lossy};
use crate::extract::Result as ExtractResult;

/// Go `fi.Size()` and `fi.ModTime().UnixNano()` as `<size>/<mtime ns>`, with `sep` between.
fn size_mtime(fi: &Metadata, sep: char) -> String {
    use std::os::unix::fs::MetadataExt;
    let ns = fi
        .mtime()
        .wrapping_mul(1_000_000_000)
        .wrapping_add(fi.mtime_nsec());
    format!("{}{sep}{ns}", fi.size() as i64)
}

/// Go `filepath.Abs`, `""` on an error (Go's `abs, _ :=`).
fn abs_or_empty(p: &str) -> String {
    abs(Path::new(p)).map_or_else(
        |_| String::new(),
        |a| go_lossy(a.as_os_str().as_encoded_bytes()),
    )
}

/// Go `binID`: `<size>/<mtime ns>` of `std::env::current_exe()`, or `noexe`; tells builds apart
/// when the version string is the same.
pub(crate) fn bin_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        std::env::current_exe()
            .and_then(fs::metadata)
            .map_or_else(|_| "noexe".to_string(), |fi| size_mtime(&fi, '/'))
    })
}

/// Go `outFlags`: the run options and the manifest pass result (the last of `results`) that
/// shape the outputs.
pub(crate) fn out_flags(
    no_manifests: bool,
    cargo: bool,
    no_cluster: bool,
    results: &[ExtractResult],
) -> String {
    if no_manifests {
        return format!("nomanifests cargo={cargo} nocluster={no_cluster}");
    }
    let b = gostd::json::marshal(&results[results.len() - 1]).unwrap_or_default();
    format!(
        "cargo={cargo} nocluster={no_cluster} {}",
        cache::hash_bytes(&b)
    )
}

/// Go `outFP`: hashes every input of graph.json and GRAPH_REPORT.md other than file contents.
/// It is [`out_fp_with`] with [`bin_id`].
pub(crate) fn out_fp(root: &str, walk: &WalkReport, commit: &str, flags: &str) -> String {
    out_fp_with(bin_id(), root, walk, commit, flags)
}

/// [`out_fp`] with the binary identity given (Go's test swaps the `binID` var).
pub(crate) fn out_fp_with(
    bin_id: &str,
    root: &str,
    walk: &WalkReport,
    commit: &str,
    flags: &str,
) -> String {
    let q = |s: &str| quote(s.as_bytes());
    let out = out_dir_for(root);
    let (stamp, abs_root, abs_out) = (cache_stamp(), abs_or_empty(root), abs_or_empty(&out));
    let mut h = [
        stamp.as_str(),
        bin_id,
        root,
        abs_root.as_str(),
        out.as_str(),
        abs_out.as_str(),
        commit,
        flags,
    ]
    .map(q)
    .join(" ");
    h.push('\n');
    for f in &walk.files {
        let _ = writeln!(h, "{}", q(f.as_str()));
    }
    for (e, n) in &walk.skipped_exts {
        let _ = writeln!(h, "{}={n}", q(e.as_str()));
    }
    for n in ["graph.json", "GRAPH_REPORT.md", cache::FILE_NAME] {
        match fs::metadata(join(&[&out, n])) {
            Ok(fi) => h.push_str(&size_mtime(&fi, ' ')),
            Err(_) => h.push_str("missing"),
        }
        h.push('\n');
    }
    cache::hash_bytes(h.as_bytes())
}

/// Go `sigOf`: `<size>/<mtime ns>` of `path`, or `""` when it cannot be stat'ed.
pub(crate) fn sig_of(path: &Path) -> String {
    fs::metadata(path).map_or_else(|_| String::new(), |fi| size_mtime(&fi, '/'))
}

/// The running [`start_stat_check`]: the stat index and its recorded output.
pub(crate) type StatCheck = JoinHandle<(StatIndex, Option<Output>)>;

/// Go `startStatCheck`: loads the stat sidecar and stats the files it lists while the caller
/// walks. Joining gives the index and its recorded output, both empty when `on` is false or
/// any file changed.
pub(crate) fn start_stat_check(root: &str, on: bool, workers: usize) -> Option<StatCheck> {
    let root = root.to_string();
    on.then(|| {
        std::thread::spawn(move || {
            let path = join(&[&out_dir_for(&root), cache::STAT_FILE_NAME]);
            match cache::load_stat_out(Path::new(&path), &cache_stamp()) {
                (s, Some(o)) if stats_fresh(&root, &s, workers) => (s, Some(o)),
                _ => (StatIndex::new(), None),
            }
        })
    })
}

/// Go `statsFresh`: whether no file `prev_stat` lists changed since it was recorded. No file is
/// read. `workers` is Go's `runtime.NumCPU()` (R5).
fn stats_fresh(root: &str, prev_stat: &StatIndex, workers: usize) -> bool {
    let keys: Vec<(&String, &cache::StatEntry)> = prev_stat.iter().collect();
    let next = AtomicUsize::new(0);
    let stale = AtomicBool::new(false);
    std::thread::scope(|s| {
        for _ in 0..workers.max(1) {
            s.spawn(|| {
                while !stale.load(Ordering::Relaxed) {
                    let Some((k, e)) = keys.get(next.fetch_add(1, Ordering::Relaxed)) else {
                        return;
                    };
                    if !cache::stat_fresh(&Path::new(root).join(k), e) {
                        stale.store(true, Ordering::Relaxed);
                    }
                }
            });
        }
    });
    !stale.load(Ordering::Relaxed)
}

/// Go `sameFiles`: whether the walk found exactly the files `prev_stat` lists.
pub(crate) fn same_files(files: &[String], prev_stat: &StatIndex) -> bool {
    files.len() == prev_stat.len() && files.iter().all(|f| prev_stat.contains_key(f))
}

/// Go `outputsCurrent`: whether an update of a tree `stats_fresh` passed would rewrite the same
/// bytes, so it can skip loading the cache.
pub(crate) fn outputs_current(root: &str, rep: &WalkReport, out: &Output, flags: &str) -> bool {
    // update would rewrite a lost or edited marker.
    let marker = join(&[&out_dir_for(root), ROOT_FILE_NAME]);
    match fs::read(marker) {
        Ok(b) if b == format!("{}\n", abs_or_empty(root)).as_bytes() => {}
        _ => return false,
    }
    out_fp(root, rep, &git_head(Path::new(root)), flags) == out.fp
}

#[cfg(test)]
#[path = "tests/outfp.rs"]
mod outfp_tests;
