//! graphify-go `internal/detect` tests (`detect_test.go`, `detect_parity_test.go`,
//! `detect_parity_unix_test.go`, `gitignore_test.go`, `gitignore_parity_test.go`,
//! `gitignore_infoexclude_test.go`, `walk_test.go`), the upstream graphify v0.9.107 cases that
//! hold for graphify-go v0.12.0, and the plan's added tests. Shared helpers (Go: the test files
//! of package detect share one namespace).
//!
//! Go's `t.Setenv("GRAPHIFY_OUT"|"HOME", ...)` and `t.Chdir(dir)` become an [`Env`] passed to
//! `collect_files_report_in` and `new_ignorer`: both are process-global, and `set_var` is
//! `unsafe` in edition 2024.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::detect::{Env, collect_files_report_in};
use super::gitignore::{IgnoreRule, Ignorer, apply_rules};
use crate::detect::collect_files;
use crate::testutil::TempDir;

mod added;
mod detect;
mod detect_parity;
#[cfg(unix)]
mod detect_parity_unix;
mod gitignore;
mod gitignore_infoexclude;
mod gitignore_parity;
mod upstream_detect;
mod upstream_ignore_file_encoding;
mod upstream_msbuild_dir_evidence;
#[cfg(unix)]
mod upstream_non_regular_files;
mod walk;

/// The walk's worker count (Go: `min(GOMAXPROCS, walkers)`, walkers = 4).
const WORKERS: usize = 4;

/// Go `mustWrite` / `put`: writes `content` to `path`, creating its parent directories.
fn must_write(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, content).expect("write");
}

/// Go `put(t, root, rel, body)`.
fn put(root: &Path, rel: &str, body: &str) {
    must_write(&root.join(rel), body);
}

/// Go `collect(t, root)`: `CollectFiles`, sorted.
fn collect(root: &Path) -> Vec<String> {
    let mut files = collect_files(root).expect("CollectFiles");
    files.sort();
    files
}

/// `collect` with the process state of a Go `t.Setenv` / `t.Chdir`.
fn collect_env(root: &Path, env: &Env) -> Vec<String> {
    let mut files = collect_files_report_in(root, WORKERS, env)
        .expect("CollectFiles")
        .files;
    files.sort();
    files
}

/// Go `collected(t, root)`: the set of `CollectFiles`.
fn collected(root: &Path) -> BTreeSet<String> {
    collect_files(root)
        .expect("CollectFiles")
        .into_iter()
        .collect()
}

/// `collected` with the process state of a Go `t.Setenv` / `t.Chdir`.
fn collected_env(root: &Path, env: &Env) -> BTreeSet<String> {
    collect_env(root, env).into_iter().collect()
}

/// Go `mkGit`: a `.git` dir with `HEAD`, `objects/` and `refs/`.
fn mk_git(dir: &Path) {
    must_write(&dir.join(".git").join("HEAD"), "ref: refs/heads/main\n");
    for d in ["objects", "refs"] {
        std::fs::create_dir_all(dir.join(".git").join(d)).expect("mkdir");
    }
}

/// Go `resolved(t)`: a fresh temp dir with its symlinks resolved (macOS `/var` is
/// `/private/var`). The `TempDir` must outlive the path.
fn resolved() -> (TempDir, PathBuf) {
    let d = TempDir::new();
    let r = std::fs::canonicalize(d.path()).expect("EvalSymlinks");
    (d, r)
}

/// `Env::process()` with `$HOME` replaced (Go `t.Setenv("HOME", home)`).
fn env_home(home: PathBuf) -> Env {
    Env {
        home: Some(home),
        ..Env::process()
    }
}

/// `Env::process()` with the working directory replaced (Go `t.Chdir(cwd)`).
fn env_cwd(cwd: PathBuf) -> Env {
    Env {
        cwd: Some(cwd),
        ..Env::process()
    }
}

/// Go `goIgnored` (gitignore_parity_test.go:11): mimics the pruning walk, testing each ancestor
/// as a directory before the path itself.
fn go_ignored(rules: &[IgnoreRule], path: &str, is_dir: bool) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    for i in 1..parts.len() {
        if apply_rules(rules, &parts[..i].join("/"), true, false) {
            return true;
        }
    }
    apply_rules(rules, path, is_dir, false)
}

/// `go_ignored` over a whole ignorer (every ignore file of the tree and its ancestors): the
/// verdict the pruning walk reaches for `rel`. Upstream's `_is_ignored` tests the ancestors
/// itself; Go's `ignorer.ignored` leaves that to the walk.
fn walk_ignored(ign: &Ignorer, rel: &str, is_dir: bool) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    for i in 1..parts.len() {
        if ign.ignored(&parts[..i].join("/"), true) {
            return true;
        }
    }
    ign.ignored(rel, is_dir)
}

/// Whether the process can still read `dir` after a `chmod 000` (root can: Go's tests skip
/// there, `os.Geteuid() == 0`).
fn readable_despite_chmod(dir: &Path) -> bool {
    std::fs::File::open(dir).is_ok()
}

/// `chmod` of `p` to `mode`.
fn chmod(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).expect("chmod");
}
