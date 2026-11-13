//! Test helpers, std only.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A fresh directory under `std::env::temp_dir()`, removed on drop (Go `t.TempDir()`).
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> TempDir {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("graphify-test-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("create temp dir");
        TempDir(p)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    /// Writes `data` to `rel`, creating its parent directories, and returns the full path.
    pub fn write(&self, rel: &str, data: &[u8]) -> PathBuf {
        let p = self.join(rel);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).expect("create parent dir");
        }
        std::fs::write(&p, data).expect("write file");
        p
    }
}

impl Default for TempDir {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs `f` on a thread with the production stack ([`crate::STACK_BYTES`]), as a deep-input test
/// must (plan hazard "Stack depth").
pub fn on_worker_stack<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(crate::STACK_BYTES)
            .spawn_scoped(s, f)
            .expect("spawn worker thread")
            .join()
            .expect("worker thread panicked")
    })
}

/// `crates/graphify/testdata`.
pub fn testdata() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

#[test]
fn temp_dir_round_trip() {
    let d = TempDir::new();
    let p = d.write("a/b.txt", b"x");
    assert_eq!(p, d.path().join("a/b.txt"));
    assert_eq!(std::fs::read(d.join("a/b.txt")).unwrap(), b"x");
    let kept = d.path().to_path_buf();
    drop(d);
    assert!(!kept.exists());
}

#[test]
fn testdata_present() {
    let d = testdata();
    assert!(d.join("extract/proj/util/math.go").is_file());
    assert!(d.join("extract/tf/label/main.tf").is_file());
    let golden = std::fs::metadata(d.join("cluster/louvain_gonum.golden")).unwrap();
    assert_eq!(golden.len(), 29_680);
}

#[test]
fn worker_stack_holds_deep_recursion() {
    // 100,000 frames of a few hundred bytes each: more than a 2 MiB default thread stack holds.
    fn depth(n: u32, pad: [u8; 256]) -> u32 {
        if n == 0 {
            return u32::from(pad[0]);
        }
        depth(n - 1, std::hint::black_box(pad)) + 1
    }
    assert_eq!(on_worker_stack(|| depth(100_000, [0; 256])), 100_000);
}
