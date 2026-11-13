//! Added tests (coordinator relay, plan R6): each step of the atomic write fails with Go's
//! `*os.PathError` / `*os.LinkError` text. The triggers fail for root too (CI runs as root).

use std::io;

use crate::fsutil::{write_atomic, write_file_atomic};
use crate::testutil::TempDir;

/// CreateTemp: the directory does not exist.
#[test]
fn open_step_text() {
    let d = TempDir::new();
    let path = d.join("missing/graph.json");
    let e = write_file_atomic(&path, b"{}", 0o644)
        .unwrap_err()
        .to_string();
    let prefix = format!("open {}/.gfy-", d.join("missing").display());
    assert!(e.starts_with(&prefix), "{e}");
    assert!(e.ends_with(": no such file or directory"), "{e}");
}

/// The caller's own error passes through unchanged, and no temp file is left.
#[test]
fn write_callback_error_passes_through() {
    let d = TempDir::new();
    let path = d.join("graph.json");
    let e = write_atomic(&path, 0o644, |_| {
        Err(io::Error::other("json: unsupported value: NaN"))
    })
    .unwrap_err();
    assert_eq!(e.to_string(), "json: unsupported value: NaN");
    assert_eq!(super::fsutil::names(&d), Vec::<String>::new());
}

/// Rename: the target is a non-empty directory (EISDIR on Linux and macOS, even for root).
#[test]
fn rename_step_text() {
    let d = TempDir::new();
    d.write("graph.json/keep", b"x");
    let path = d.join("graph.json");
    let e = write_file_atomic(&path, b"{}", 0o644)
        .unwrap_err()
        .to_string();
    let prefix = format!("rename {}/.gfy-", d.path().display());
    assert!(e.starts_with(&prefix), "{e}");
    assert!(
        e.ends_with(&format!(" {}: is a directory", path.display())),
        "{e}"
    );
    assert_eq!(super::fsutil::names(&d), ["graph.json"], "temp file left");
}
