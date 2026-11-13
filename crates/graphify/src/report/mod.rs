//! Port of graphify-go `internal/report`. Filled by graphify-rs-ihc.11 (plan Task 10).
#![allow(dead_code, unused_variables)]

#[allow(clippy::module_inception)]
mod report;

pub use report::generate;

#[cfg(test)]
mod tests;
