//! Port of `brokenpipe_test.go`: `TestIsBrokenPipe` here (unix, as Go's `//go:build unix`);
//! `TestQueryExitsZeroOnBrokenPipe` runs the binary (`crates/graphify/tests/cli_brokenpipe.rs`).

use std::io;
use std::path::Path;

use super::is_broken_pipe;
use crate::cli::{Error, path_error};

/// EPIPE: 32 on linux and darwin.
const EPIPE: i32 = 32;

// is_broken_pipe recognises a raw EPIPE and an EPIPE with a path (Go's `*os.PathError`), and
// rejects unrelated errors, so only genuine broken-pipe writes take the exit-0 shortcut (#1807).
#[test]
fn test_is_broken_pipe() {
    let epipe = io::Error::from_raw_os_error(EPIPE);
    assert!(
        is_broken_pipe(&epipe),
        "is_broken_pipe(EPIPE) = false, want true"
    );
    let wrapped = crate::goerr::path_error("write", Path::new("x"), &epipe);
    assert!(
        is_broken_pipe(&wrapped),
        "is_broken_pipe(wrapped PathError EPIPE) = false, want true"
    );
    assert!(
        !is_broken_pipe(&io::Error::other("boom")),
        "is_broken_pipe(\"boom\") = true, want false"
    );
    // The command error a write to a closed pipe becomes.
    assert!(
        matches!(
            path_error("write", Path::new("x"), &epipe),
            Error::BrokenPipe
        ),
        "path_error(EPIPE) is not Error::BrokenPipe"
    );
}
