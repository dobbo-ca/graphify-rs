//! Port of graphify-go `internal/idutil`.

#[allow(clippy::module_inception)]
mod idutil;

pub use idutil::*;

#[cfg(test)]
mod tests;
