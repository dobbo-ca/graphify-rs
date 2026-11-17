//! Helpers of the integration tests that run the binary (std only). Each test file uses a part.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// `graphify <args>` as a `Command`. Under qemu-user (the arm64 CI stage) a child aarch64 binary
/// cannot be exec'd directly (no binfmt_misc in BuildKit): `GRAPHIFY_TEST_RUNNER` names the
/// emulator to run it with.
pub fn graphify(args: &[&str]) -> Command {
    let bin = env!("CARGO_BIN_EXE_graphify");
    let mut cmd = match std::env::var("GRAPHIFY_TEST_RUNNER") {
        Ok(runner) if !runner.is_empty() => {
            // Space-separated; clippy.toml bans split_whitespace.
            let mut parts = runner.split(' ').filter(|p| !p.is_empty());
            let mut c = Command::new(parts.next().unwrap());
            c.args(parts).arg(bin);
            c
        }
        _ => Command::new(bin),
    };
    cmd.args(args);
    cmd
}

/// What a run printed and how it ended.
pub struct Run {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `graphify <args>` in `cwd` with the extra environment `env`.
pub fn run_in(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Run {
    let mut cmd = graphify(args);
    cmd.current_dir(cwd);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out: Output = cmd.output().expect("run graphify");
    Run {
        code: out.status.code(),
        stdout: String::from_utf8(out.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(out.stderr).expect("stderr is UTF-8"),
    }
}

/// [`run_in`] that must exit 0.
pub fn ok_in(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Run {
    let r = run_in(cwd, args, env);
    assert_eq!(
        r.code,
        Some(0),
        "graphify {args:?} in {}: stderr {:?}",
        cwd.display(),
        r.stderr
    );
    r
}

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A fresh directory under `std::env::temp_dir()`, symlinks resolved (Go tests take the
/// canonical cwd, `/private/var` on macOS), removed on drop (Go `t.TempDir()`).
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> TempDir {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("graphify-it-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("create temp dir");
        TempDir(std::fs::canonicalize(&p).expect("canonical temp dir"))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    /// Writes `data` to `rel`, creating its parent directories, and returns the full path.
    pub fn write(&self, rel: &str, data: &str) -> PathBuf {
        write(&self.join(rel), data);
        self.join(rel)
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

/// Writes `data` to `path`, creating its parent directories.
pub fn write(path: &Path, data: &str) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).expect("create parent dir");
    }
    std::fs::write(path, data).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

/// Reads `path` as text.
pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// `git -C dir <args>`, fatal on failure (Go `gitRun`).
pub fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}: {}",
        out.status,
        std::str::from_utf8(&out.stderr).unwrap_or("<not UTF-8>")
    );
}

/// The arguments of a test commit with a fixed identity (Go `gitRun(... "commit" ...)`); the
/// caller adds `-m <msg>` and any flag.
pub const COMMIT: [&str; 8] = [
    "-c",
    "commit.gpgsign=false",
    "-c",
    "user.name=t",
    "-c",
    "user.email=t@t",
    "commit",
    "-q",
];

/// The `str` of a path (temp paths are UTF-8).
pub fn s(p: &Path) -> &str {
    p.to_str().expect("UTF-8 path")
}
