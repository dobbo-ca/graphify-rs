//! Port of `sigpipe_unix.go` and `sigpipe_other.go`: a downstream reader that closes the pipe
//! early is not a failure (exit 0). Rust binaries start with SIGPIPE ignored, so a write to a
//! closed pipe returns EPIPE; `run` maps it to exit 0 (plan hazard "EPIPE -> exit 0").

use std::io;

/// Go `isBrokenPipe`: whether `e` is EPIPE, raw or with a path (`*os.PathError`; `goerr` keeps
/// the kind). Always false off unix (Go `sigpipe_other.go`).
pub(crate) fn is_broken_pipe(e: &io::Error) -> bool {
    cfg!(unix) && e.kind() == io::ErrorKind::BrokenPipe
}

#[cfg(all(test, unix))]
#[path = "tests/brokenpipe.rs"]
mod brokenpipe_tests;
