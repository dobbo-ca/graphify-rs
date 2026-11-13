//! Go 1.27.4 `path/filepath` `WalkDir` (`path.go`) with `io/fs` `ReadDir` semantics: lexical
//! order, `SkipDir` on a file skips the rest of its directory. Call site: graphify-go
//! `detect.go:734` (`WalkDir`, `SkipDir`).
#![allow(unused_variables)]

use std::ffi::OsString;
use std::path::Path;

/// The `fs.DirEntry` parts a callback needs.
pub struct DirEntry {
    pub name: OsString,
    pub file_type: std::fs::FileType,
}

/// A callback's answer: Go `nil`, `fs.SkipDir`, or any other error (stops the walk).
pub enum Walk {
    Continue,
    SkipDir,
    Stop(std::io::Error),
}

/// Go `filepath.WalkDir`. The callback gets the path, the entry (`None` only when the root cannot
/// be stat'ed) and the error, if any. For the root, `name` is the last element of `root`.
pub fn walk_dir(
    root: &Path,
    f: impl FnMut(&Path, Option<&DirEntry>, Option<&std::io::Error>) -> Walk,
) -> std::io::Result<()> {
    todo!("graphify-rs-aa6.15")
}
