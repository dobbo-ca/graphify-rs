//! Port of graphify-go `internal/detect`: the corpus walk, ignore files and sensitive files.

#[allow(clippy::module_inception)]
mod detect;
mod gitignore;

pub use detect::{
    Error, SUPPORTED_EXTENSIONS, WalkReport, collect_files, collect_files_report,
    collect_manifests, go_lossy, is_sensitive, shebang_ext,
};

#[cfg(test)]
mod tests;
