//! Upstream graphify v0.9.107 `tests/test_atomic_writes.py` and
//! `tests/test_atomic_version_stamp.py`: the cases that hold for graphify-go's `fsutil`.
//! Not ported (they do not hold for Go): a missing parent directory is created; the existing
//! mode is kept; a new file follows the umask; a symlinked destination is written through;
//! a read-only destination is refused; the Windows and JSON-helper cases.

use std::io::{self, Write};
use std::os::unix::fs::symlink;

use super::fsutil::names;
use crate::fsutil::{write_atomic, write_file_atomic};
use crate::testutil::TempDir;

/// test_write_text_atomic_preserves_existing_on_failure: a write that fails leaves the old file
/// and no temp file (Go: `write` errors before the rename; the deferred Remove runs).
#[test]
#[ignore = "graphify-rs-ihc.3"]
fn preserves_existing_on_failure() {
    let dir = TempDir::new();
    let path = dir.write("graph.json", b"original");
    let err = write_atomic(&path, 0o644, |w| {
        w.write_all(b"content-that-must-not-land")?;
        Err(io::Error::other("simulated disk full"))
    })
    .unwrap_err();
    assert_eq!(err.to_string(), "simulated disk full");
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
    assert_eq!(names(&dir), ["graph.json"]);
}

/// test_atomic_replace_temp_name_is_bounded_and_does_not_embed_long_filename, without
/// upstream's `.tmp` suffix: Go's temp name is `.gfy-` and a random number (`os.CreateTemp`).
#[test]
#[ignore = "graphify-rs-ihc.3"]
fn temp_name_is_bounded() {
    let dir = TempDir::new();
    let long_name = format!("{}.md", "a".repeat(120));
    let path = dir.join(&long_name);
    let mut seen = Vec::new();
    write_atomic(&path, 0o644, |w| {
        seen = names(&dir);
        w.write_all(b"hello")
    })
    .unwrap();
    assert_eq!(seen.len(), 1, "temp files: {seen:?}");
    let tmp = &seen[0];
    assert!(tmp.starts_with(".gfy-"), "{tmp}");
    assert!(
        tmp.len() <= 25,
        "temp filename was unexpectedly long: {tmp}"
    );
    assert!(
        !tmp.contains(&"a".repeat(50)),
        "temp filename embedded the destination name"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"hello");
}

/// test_write_version_stamp_replaces_symlink_instead_of_following: the rename replaces a
/// symlink at the destination; its target is untouched.
#[test]
#[ignore = "graphify-rs-ihc.3"]
fn replaces_symlink_instead_of_following() {
    let dir = TempDir::new();
    let target = dir.write("elsewhere.txt", b"keep");
    let path = dir.join("graph.json");
    symlink(&target, &path).unwrap();
    write_file_atomic(&path, b"new", 0o644).unwrap();
    assert!(
        !std::fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
    assert_eq!(std::fs::read(&target).unwrap(), b"keep");
}
