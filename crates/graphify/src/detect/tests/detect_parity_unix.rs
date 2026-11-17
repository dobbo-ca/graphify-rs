//! Port of `detect_parity_unix_test.go` (3 tests, `//go:build !windows`).

use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use super::{collect_env, env_cwd, mk_git, put, resolved};
use crate::detect::detect::has_build_output_markers;
use crate::detect::{collect_files, collect_manifests};
use crate::testutil::TempDir;

/// Go `within`: fails fast instead of hanging on a FIFO open.
fn within<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(5))
        .expect("collect hung, a non-regular file was opened")
}

/// `mkfifo(1)` (std has no `mkfifo`); `false` when it cannot run, where Go's test skips.
fn mkfifo(p: &Path) -> bool {
    std::process::Command::new("mkfifo")
        .arg(p)
        .status()
        .is_ok_and(|s| s.success())
}

/// Go `TestNonRegularFiles`: FIFOs (named like source and like a manifest) and links to them are
/// never opened or collected.
#[test]
fn test_non_regular_files() {
    let root = TempDir::new();
    put(root.path(), "ok.go", "package a\n");
    for n in ["fifo.go", "fifo", "go.mod"] {
        if !mkfifo(&root.join(n)) {
            return; // Go: t.Skip
        }
    }
    let _ = symlink(root.join("fifo"), root.join("linkfifo"));
    let _ = symlink(root.join("fifo.go"), root.join("linkfifo.go"));
    let dir = root.path().to_path_buf();
    let (files, mans) = within(move || {
        (
            collect_files(&dir).unwrap_or_default(),
            collect_manifests(&dir).unwrap_or_default(),
        )
    });
    assert_eq!(files, ["ok.go"], "files {files:?}");
    assert!(mans.is_empty(), "manifests {mans:?}");
}

/// Go `TestBuildOutputProbeAbortsOnStatError`: an entry whose stat fails with ELOOP (a symlink
/// to itself) aborts the probe, even beside a `.class` file.
#[test]
fn test_build_output_probe_aborts_on_stat_error() {
    let (_d, root) = resolved();
    put(&root, "out/Z.class", "");
    symlink("a", root.join("out/a")).expect("symlink");
    assert!(
        !has_build_output_markers(&root.join("out")),
        "ELOOP entry must abort the probe"
    );
}

/// Go `TestSymlinkedCwdParity`: a relative root under a cwd reached through a symlink (Go
/// `t.Chdir`, which sets `$PWD`) resolves like the kernel does, and the ancestor `.gitignore` of
/// the real repo applies.
#[test]
fn test_symlinked_cwd_parity() {
    let (_d, base) = resolved();
    put(&base, "repo/.gitignore", "*.py\n");
    mk_git(&base.join("repo"));
    put(&base, "repo/deep/sub/a.go", "package a\n");
    put(&base, "repo/deep/sub/h.py", "x = 1\n");
    symlink("a.go", base.join("repo/deep/sub/l.go")).expect("symlink");
    put(&base, "plain/x", "");
    symlink("../repo/deep/sub", base.join("plain/lnk")).expect("symlink");
    let got = collect_env(Path::new(".."), &env_cwd(base.join("plain/lnk")));
    assert_eq!(
        got,
        ["sub/a.go", "sub/l.go"],
        "root .. from symlinked cwd: {got:?}"
    );

    put(&base, "t4/__tests__/c.go", "package a\n");
    put(&base, "t4/__tests__/snapshots/x.go", "package a\n");
    symlink("__tests__", base.join("t4/t")).expect("symlink");
    let got = collect_env(Path::new("."), &env_cwd(base.join("t4/t")));
    assert_eq!(got, ["c.go"], "snapshots via symlinked cwd: {got:?}");
}
