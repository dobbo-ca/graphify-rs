//! Port of `fsutil.go`: atomic file writes.

use std::io::{self, Write};
use std::path::Path;

/// Go `fsutil.WriteFileAtomic`: writes `data` to a `.gfy-*` temp file in the same directory,
/// sets mode `perm`, and renames it over `path`.
pub fn write_file_atomic(path: &Path, data: &[u8], perm: u32) -> io::Result<()> {
    todo!("graphify-rs-ihc.3")
}

/// Go `fsutil.WriteAtomic`: as [`write_file_atomic`], with the content from `write`.
pub fn write_atomic(
    path: &Path,
    perm: u32,
    write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> io::Result<()> {
    todo!("graphify-rs-ihc.3")
}
