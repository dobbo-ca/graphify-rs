//! Go's error texts for I/O errors (plan hazard "Go I/O error text"). Go prints an
//! `*os.PathError` as `<op> <path>: <errno text>` and an `*os.LinkError` as
//! `<op> <old> <new>: <errno text>`, with `syscall.Errno.Error()`'s lowercase text; Rust's
//! `io::Error` prints `No such file or directory (os error 2)`. These build an `io::Error` with
//! the same kind whose `Display` is Go's text, so a caller passes it through unchanged.

use std::io;
use std::path::Path;

/// Go `*os.PathError`: `<op> <path>: <errno text>`.
pub fn path_error(op: &str, path: &Path, e: &io::Error) -> io::Error {
    io::Error::new(
        e.kind(),
        format!("{op} {}: {}", path.display(), errno_text(e)),
    )
}

/// Go `*os.LinkError`: `<op> <old> <new>: <errno text>` (rename, link, symlink).
pub fn link_error(op: &str, old: &Path, new: &Path, e: &io::Error) -> io::Error {
    io::Error::new(
        e.kind(),
        format!(
            "{op} {} {}: {}",
            old.display(),
            new.display(),
            errno_text(e)
        ),
    )
}

/// Go `syscall.Errno.Error()` for an OS error (`"errno <n>"` for a number Go has no text for);
/// any other error's own text.
pub fn errno_text(e: &io::Error) -> String {
    match e.raw_os_error() {
        Some(n) => errno_name(n).map_or_else(|| format!("errno {n}"), str::to_string),
        None => e.to_string(),
    }
}

/// Go's `syscall` error strings (`zerrors_linux_amd64.go`) for the errnos file work meets.
#[cfg(target_os = "linux")]
fn errno_name(n: i32) -> Option<&'static str> {
    Some(match n {
        1 => "operation not permitted",
        2 => "no such file or directory",
        4 => "interrupted system call",
        5 => "input/output error",
        9 => "bad file descriptor",
        11 => "resource temporarily unavailable",
        12 => "cannot allocate memory",
        13 => "permission denied",
        16 => "device or resource busy",
        17 => "file exists",
        18 => "invalid cross-device link",
        20 => "not a directory",
        21 => "is a directory",
        22 => "invalid argument",
        23 => "too many open files in system",
        24 => "too many open files",
        26 => "text file busy",
        27 => "file too large",
        28 => "no space left on device",
        30 => "read-only file system",
        32 => "broken pipe",
        36 => "file name too long",
        39 => "directory not empty",
        40 => "too many levels of symbolic links",
        122 => "disk quota exceeded",
        _ => return None,
    })
}

/// Go's `syscall` error strings (`zerrors_darwin_arm64.go`) for the errnos file work meets.
#[cfg(target_os = "macos")]
fn errno_name(n: i32) -> Option<&'static str> {
    Some(match n {
        1 => "operation not permitted",
        2 => "no such file or directory",
        4 => "interrupted system call",
        5 => "input/output error",
        9 => "bad file descriptor",
        12 => "cannot allocate memory",
        13 => "permission denied",
        16 => "resource busy",
        17 => "file exists",
        18 => "cross-device link",
        20 => "not a directory",
        21 => "is a directory",
        22 => "invalid argument",
        23 => "too many open files in system",
        24 => "too many open files",
        26 => "text file busy",
        27 => "file too large",
        28 => "no space left on device",
        30 => "read-only file system",
        32 => "broken pipe",
        35 => "resource temporarily unavailable",
        62 => "too many levels of symbolic links",
        63 => "file name too long",
        66 => "directory not empty",
        69 => "disc quota exceeded",
        _ => return None,
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn errno_name(_: i32) -> Option<&'static str> {
    None
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::Path;

    use super::{errno_text, link_error, path_error};

    #[test]
    fn go_texts() {
        let enoent = io::Error::from_raw_os_error(2);
        assert_eq!(
            path_error("open", Path::new("a/b.json"), &enoent).to_string(),
            "open a/b.json: no such file or directory"
        );
        assert_eq!(
            path_error("open", Path::new("x"), &enoent).kind(),
            io::ErrorKind::NotFound
        );
        let eisdir = io::Error::from_raw_os_error(21);
        assert_eq!(
            link_error("rename", Path::new("t"), Path::new("d"), &eisdir).to_string(),
            "rename t d: is a directory"
        );
        assert_eq!(
            errno_text(&io::Error::from_raw_os_error(13)),
            "permission denied"
        );
        assert_eq!(
            errno_text(&io::Error::from_raw_os_error(9999)),
            "errno 9999"
        );
        assert_eq!(
            errno_text(&io::Error::other("encode failed")),
            "encode failed"
        );
    }
}
