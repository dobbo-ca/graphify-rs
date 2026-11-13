//! Port of graphify-go `cmd/graphify`: the command line.
//!
//! Shared API (plan Task 1): `Error`, `Out`, `run` and the `cmd_*` signatures are fixed. The
//! `hook`, `watch`, `install` and `save-result` commands are graphify-rs-ihc.21; the rest is
//! graphify-rs-ihc.20 (plan Tasks 21 and 20).
#![allow(dead_code, unused_variables)]

pub mod install;
pub mod main;
pub mod maintenance;
pub mod save_result;

use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};
use std::path::Path;

/// Where a command writes: stdout or stderr, or a buffer in tests.
pub type Out<'a> = &'a mut dyn Write;

/// A command error. `Display` is Go's error text, printed as `graphify: <err>` (`main.go:216-222`).
#[derive(Debug)]
pub enum Error {
    /// Any Go error, as its text.
    Msg(String),
    /// A write hit a closed pipe (Go `isBrokenPipe`): the process exits 0.
    BrokenPipe,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Msg(s) => f.write_str(s),
            Error::BrokenPipe => f.write_str("write /dev/stdout: broken pipe"),
        }
    }
}

impl std::error::Error for Error {}

/// Go's `*os.PathError` text, `<op> <path>: <errno text>` (for example
/// `open x.json: no such file or directory`), for an I/O error; `BrokenPipe` for EPIPE.
pub fn path_error(op: &str, path: &Path, e: &io::Error) -> Error {
    todo!("graphify-rs-ihc.20")
}

/// Go `main`: runs the command in `args` (`args[0]` is the program) and returns the exit code
/// (`main.go:159-223`: 2 for usage, 1 for an error, 0 otherwise and on a broken pipe).
pub fn run(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> i32 {
    let cmd = args.get(1).and_then(|a| a.to_str()).unwrap_or("");
    let rest = args.get(2..).unwrap_or(&[]);
    let result = match cmd {
        "build" => main::cmd_build(rest, stdout, stderr),
        "update" => main::cmd_update(rest, stdout, stderr),
        "hook" => maintenance::cmd_hook(rest, stdout, stderr),
        "watch" => maintenance::cmd_watch(rest, stdout, stderr),
        "install" => install::cmd_install(rest, stdout, stderr),
        "save-result" => save_result::cmd_save_result(rest, stdout, stderr),
        _ => todo!("graphify-rs-ihc.20"),
    };
    todo!("graphify-rs-ihc.20")
}
