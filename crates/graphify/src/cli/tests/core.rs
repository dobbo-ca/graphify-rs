//! Task 20 added tests: `path_error_texts` (plan hazard "Go I/O error text") and
//! `unknown_command_quoted` (`%q` is `gostd::strconv::quote`, R6).

use std::ffi::OsString;
use std::io;
use std::path::Path;

use crate::cli::{Error, path_error, run};
use crate::testutil::TempDir;

/// The error of `r` made a command error with `op` and `path` (Go `*os.PathError`).
fn text<T>(op: &str, path: &Path, r: io::Result<T>) -> String {
    match r {
        Ok(_) => panic!("{op} {}: succeeded, want an error", path.display()),
        Err(e) => path_error(op, path, &e).to_string(),
    }
}

// Go prints `<op> <path>: <errno text>` with `syscall.Errno.Error()`'s lowercase text; Rust's
// io::Error would print `No such file or directory (os error 2)`.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn path_error_texts() {
    let dir = TempDir::new();
    let missing = dir.join("missing.json");
    let file = dir.write("f", b"x");
    let under_file = file.join("x");
    let d = dir.path().to_path_buf();

    let cases: [(&str, String, String); 5] = [
        (
            "ENOENT on open",
            text("open", &missing, std::fs::File::open(&missing)),
            format!("open {}: no such file or directory", missing.display()),
        ),
        (
            "ENOENT on stat",
            text("stat", &missing, std::fs::metadata(&missing)),
            format!("stat {}: no such file or directory", missing.display()),
        ),
        (
            "EISDIR on read",
            text("read", &d, std::fs::read(&d)),
            format!("read {}: is a directory", d.display()),
        ),
        (
            "ENOTDIR on stat",
            text("stat", &under_file, std::fs::metadata(&under_file)),
            format!("stat {}: not a directory", under_file.display()),
        ),
        // CI runs as root, where a mode-000 file still opens: the errno is made directly.
        (
            "EACCES on open",
            text::<()>("open", &file, Err(io::Error::from_raw_os_error(13))),
            format!("open {}: permission denied", file.display()),
        ),
    ];
    for (name, got, want) in cases {
        assert_eq!(got, want, "{name}");
    }

    // EPIPE (32 on linux and darwin) is the broken-pipe error, which exits 0.
    let epipe = io::Error::from_raw_os_error(32);
    assert!(
        matches!(
            path_error("write", Path::new("/dev/stdout"), &epipe),
            Error::BrokenPipe
        ),
        "EPIPE is not Error::BrokenPipe"
    );
}

// graphify "a\tb" prints `graphify: unknown command "a\tb"` (Go `%q`: the tab is `\t`) and exits 1.
#[test]
#[ignore = "graphify-rs-ihc.20"]
fn unknown_command_quoted() {
    let a = [OsString::from("graphify"), OsString::from("a\tb")];
    let (mut out, mut err) = (Vec::<u8>::new(), Vec::<u8>::new());
    assert_eq!(run(&a, &mut out, &mut err), 1, "exit code");
    assert!(out.is_empty(), "stdout = {out:?}");
    assert_eq!(
        String::from_utf8(err).expect("stderr is UTF-8"),
        "graphify: unknown command \"a\\tb\"\n"
    );
}
