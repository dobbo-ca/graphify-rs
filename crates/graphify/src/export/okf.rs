//! Port of `okf.go`: the Open Knowledge Format bundle.

use std::path::{Path, PathBuf};

use super::export::Error;

/// Go `OKFFromJSON` (`okf.go:42`).
pub fn okf_from_json(json_path: &Path, out_dir: &Path) -> Result<(), Error> {
    todo!("graphify-rs-ihc.12")
}

/// Go `conceptDir` (`okf.go:248`).
pub(super) fn concept_dir(src: &str) -> String {
    todo!("graphify-rs-ihc.12")
}

/// Go `safeJoin` (`okf.go:274`).
pub(super) fn safe_join(base: &Path, dir: &str, name: &str) -> Result<PathBuf, Error> {
    todo!("graphify-rs-ihc.12")
}
