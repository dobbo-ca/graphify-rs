//! Upstream graphify v0.9.107 `tests/test_non_regular_files.py` (9 tests): upstream's
//! `_is_regular_file` is Go's `statIsFile` (`os.Stat`, which follows a symlink, then
//! `IsRegular`).

use std::os::unix::fs::symlink;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use super::must_write;
use crate::detect::detect::stat_is_file;
use crate::testutil::TempDir;

/// Upstream's `tree` fixture: `src/module.py`.
fn tree() -> (TempDir, PathBuf) {
    let d = TempDir::new();
    let src = d.join("src");
    must_write(&src.join("module.py"), "def f():\n    return 1\n");
    (d, src)
}

/// `mkfifo(1)` (std has no `mkfifo`); `false` where upstream's `needs_fifo` skips.
fn mkfifo(p: &Path) -> bool {
    std::process::Command::new("mkfifo")
        .arg(p)
        .status()
        .is_ok_and(|s| s.success())
}

/// `test_regular_source_file_is_accepted`.
#[test]
fn regular_source_file_is_accepted() {
    let (_d, src) = tree();
    assert!(stat_is_file(&src.join("module.py")));
}

/// `test_fifo_is_rejected`.
#[test]
fn fifo_is_rejected() {
    let (_d, src) = tree();
    let fifo = src.join("pipe.py");
    if !mkfifo(&fifo) {
        return;
    }
    assert!(!stat_is_file(&fifo));
}

/// `test_unix_socket_is_rejected`.
#[test]
fn unix_socket_is_rejected() {
    let (_d, src) = tree();
    let sock = src.join("sock.py");
    let _listener = UnixListener::bind(&sock).expect("bind");
    assert!(!stat_is_file(&sock));
}

/// `test_directory_named_like_a_source_file_is_rejected`.
#[test]
fn directory_named_like_a_source_file_is_rejected() {
    let (_d, src) = tree();
    std::fs::create_dir(src.join("package.py")).expect("mkdir");
    assert!(!stat_is_file(&src.join("package.py")));
}

/// `test_symlink_to_a_regular_file_is_accepted`.
#[test]
fn symlink_to_a_regular_file_is_accepted() {
    let (_d, src) = tree();
    symlink(src.join("module.py"), src.join("alias.py")).expect("symlink");
    assert!(stat_is_file(&src.join("alias.py")));
}

/// `test_symlink_pointing_at_a_fifo_is_rejected`.
#[test]
fn symlink_pointing_at_a_fifo_is_rejected() {
    let (_d, src) = tree();
    if !mkfifo(&src.join("real.py")) {
        return;
    }
    symlink(src.join("real.py"), src.join("link.py")).expect("symlink");
    assert!(!stat_is_file(&src.join("link.py")));
}

/// `test_broken_symlink_is_rejected_without_raising`.
#[test]
fn broken_symlink_is_rejected_without_raising() {
    let (_d, src) = tree();
    symlink(src.join("does-not-exist.py"), src.join("dangling.py")).expect("symlink");
    assert!(!stat_is_file(&src.join("dangling.py")));
}

/// `test_missing_path_is_rejected_without_raising`.
#[test]
fn missing_path_is_rejected_without_raising() {
    let (_d, src) = tree();
    assert!(!stat_is_file(&src.join("absent.py")));
}

/// `test_char_device_is_rejected`.
#[test]
fn char_device_is_rejected() {
    let dev = Path::new("/dev/null");
    if !dev.exists() {
        return;
    }
    assert!(!stat_is_file(dev));
}
