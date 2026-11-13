//! Shared helpers (Go: the test files of package cache share one namespace).

use std::fs::{File, Metadata};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::cache::{Cache, Entry};

mod cache;
mod stream;

/// The decoder count of `load` and `load_split` (Go: `min(GOMAXPROCS, 8)`).
const WORKERS: usize = 4;

/// An entry with only a hash.
fn entry(hash: &str) -> Entry {
    Entry {
        hash: hash.to_string(),
        ..Entry::default()
    }
}

/// A cache from `(key, entry)` literals.
fn cache_of(entries: Vec<(&str, Entry)>) -> Cache {
    entries
        .into_iter()
        .map(|(k, e)| (k.to_string(), e))
        .collect()
}

/// Go `os.Chtimes(p, t, t)` (the access time is not set).
fn chtimes(p: &Path, t: SystemTime) {
    File::options()
        .write(true)
        .open(p)
        .expect("open for chtimes")
        .set_modified(t)
        .expect("set mtime");
}

/// Go `fi.ModTime().UnixNano()`.
fn mtime_ns(fi: &Metadata) -> i64 {
    let d = fi
        .modified()
        .expect("mtime")
        .duration_since(UNIX_EPOCH)
        .expect("mtime after 1970");
    i64::try_from(d.as_nanos()).expect("mtime fits i64")
}

/// Go `time.Now().Add(-time.Hour)`.
fn hour_ago() -> SystemTime {
    SystemTime::now() - Duration::from_secs(3600)
}

/// Go `time.Now().Add(time.Hour)`.
fn hour_ahead() -> SystemTime {
    SystemTime::now() + Duration::from_secs(3600)
}
