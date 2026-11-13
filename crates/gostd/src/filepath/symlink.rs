//! Go 1.27.4 `path/filepath/symlink.go` and `symlink_unix.go`: `EvalSymlinks`. Call sites:
//! graphify-go's `EvalSymlinks` uses.
#![allow(unused_variables)]

use std::path::{Path, PathBuf};

/// Go `filepath.EvalSymlinks`.
pub fn eval_symlinks(p: &Path) -> std::io::Result<PathBuf> {
    todo!("graphify-rs-aa6.15")
}
