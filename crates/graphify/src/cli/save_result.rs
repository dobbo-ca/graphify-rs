//! Port of `save_result.go`: `save-result`. Filled by graphify-rs-ihc.21.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{Error, Out};

/// Go `memoryDirPath`: where the Q&A docs go, relative to the working directory.
const MEMORY_DIR_PATH: &str = "graphify-out/memory";

/// Go `cmdSaveResult`.
pub fn cmd_save_result(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.21")
}

/// Go `cmdSaveResult` with the working directory replaced by `cwd`, so a test does not
/// `set_current_dir` (Go's `t.Chdir`, process-global, races the other tests). `cmd_save_result` is
/// this with `.`.
fn cmd_save_result_in(cwd: &Path, args: &[OsString], stdout: Out<'_>) -> Result<(), Error> {
    todo!("graphify-rs-ihc.21")
}

/// Go `saveResult`: writes the memory doc into `dir` and returns its path.
fn save_result(
    dir: &Path,
    question: &str,
    answer: &str,
    query_type: &str,
    outcome: &str,
    correction: &str,
    nodes: &[&str],
) -> Result<PathBuf, Error> {
    todo!("graphify-rs-ihc.21")
}

#[cfg(test)]
#[path = "tests/save_result.rs"]
mod save_result_tests;
