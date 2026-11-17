//! Added tests (plan Task 13, Review Focus 1, R3): non-UTF-8 file names.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;

use crate::detect::{collect_files, go_lossy};
use crate::testutil::TempDir;

/// Review Focus 1: a file named `b"\xff.py"` is collected as `"\u{FFFD}.py"` (R3), in the byte
/// order of the original names (`z` 0x7A sorts before 0xFF). macOS (APFS) refuses a name that is
/// not UTF-8, so the test has nothing to walk there.
#[test]
fn non_utf8_file_name_walk() {
    let root = TempDir::new();
    if std::fs::write(root.path().join(OsStr::from_bytes(b"\xff.py")), "x = 1\n").is_err() {
        return; // the filesystem refuses non-UTF-8 names (APFS)
    }
    std::fs::write(root.join("z.py"), "x = 1\n").expect("write");
    let files = collect_files(root.path()).expect("CollectFiles");
    assert_eq!(files, ["z.py", "\u{FFFD}.py"]);
}

/// R3: `go_lossy` gives one U+FFFD per byte Go's `for range` reads as `RuneError` of width 1,
/// and differs from `String::from_utf8_lossy` on a truncated sequence.
#[test]
#[allow(clippy::disallowed_methods)] // from_utf8_lossy: the one call R7 allows, as the contrast
fn go_lossy_rune_by_rune() {
    const CASES: &[(&str, &[u8], &str)] = &[
        ("truncated 2-byte", b"\xc3", "\u{FFFD}"),
        ("truncated 3-byte", b"\xe2\x82\x41", "\u{FFFD}\u{FFFD}A"),
        (
            "truncated 4-byte",
            b"\xf0\x9f\x98",
            "\u{FFFD}\u{FFFD}\u{FFFD}",
        ),
        ("overlong 2-byte", b"\xc0\xaf", "\u{FFFD}\u{FFFD}"),
        (
            "overlong 3-byte",
            b"\xe0\x80\xaf",
            "\u{FFFD}\u{FFFD}\u{FFFD}",
        ),
        ("surrogate", b"\xed\xa0\x80", "\u{FFFD}\u{FFFD}\u{FFFD}"),
        ("lone continuation", b"\x80", "\u{FFFD}"),
        ("ff", b"\xff", "\u{FFFD}"),
        ("valid 2-byte", "é".as_bytes(), "é"),
        ("valid 4-byte", "😀".as_bytes(), "😀"),
    ];
    for &(name, input, want) in CASES {
        assert_eq!(go_lossy(input), want, "{name}");
    }
    assert_ne!(
        go_lossy(b"\xe2\x82\x41"),
        String::from_utf8_lossy(b"\xe2\x82\x41"),
        "go_lossy must replace each bad byte, not each maximal subpart"
    );
}
