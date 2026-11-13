//! Port of graphify-go `internal/fsutil`.

#[allow(clippy::module_inception)]
mod fsutil;

pub use fsutil::*;

#[cfg(test)]
mod tests;
