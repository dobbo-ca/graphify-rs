//! Port of graphify-go `internal/fsutil/fsutil_test.go`.

use std::os::unix::fs::PermissionsExt;

use crate::fsutil::write_file_atomic;
use crate::testutil::TempDir;

#[test]
fn test_write_file_atomic_replaces_and_cleans_up() {
    let dir = TempDir::new();
    let path = dir.write("graph.json", b"old");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    write_file_atomic(&path, b"new", 0o644).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
    let perm = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(perm, 0o644, "perm = {perm:o}, want 644");
    assert_eq!(names(&dir), ["graph.json"], "temp file left behind");
}

/// A concurrent reader must see either the whole old file or the whole new one.
#[test]
fn test_write_file_atomic_reader_sees_no_partial_file() {
    let dir = TempDir::new();
    let old = vec![b'a'; 1 << 20];
    let fresh = vec![b'b'; 1 << 20];
    let path = dir.write("graph.json", &old);
    std::thread::scope(|s| {
        let writer = s.spawn(|| write_file_atomic(&path, &fresh, 0o644));
        for _ in 0..200 {
            let got = std::fs::read(&path).unwrap();
            assert!(
                got == old || got == fresh,
                "partial read of {} bytes",
                got.len()
            );
        }
        writer.join().unwrap().unwrap();
    });
}

/// The directory's entry names, sorted (Go `os.ReadDir`).
#[allow(clippy::disallowed_methods)] // a test listing, sorted below
pub(super) fn names(dir: &TempDir) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for e in std::fs::read_dir(dir.path()).unwrap() {
        v.push(e.unwrap().file_name().into_string().unwrap());
    }
    v.sort();
    v
}
