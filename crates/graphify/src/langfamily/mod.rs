//! Port of graphify-go `internal/langfamily`.

#[allow(clippy::module_inception)]
mod langfamily;

pub use langfamily::*;

#[cfg(test)]
mod tests;
