//! Port of graphify-go `internal/analyze`. Filled by graphify-rs-ihc.10 (plan Task 9).
#![allow(dead_code, unused_variables)]

#[allow(clippy::module_inception)]
mod analyze;

pub use analyze::{Cycle, GodNode, Surprise, god_nodes, import_cycles, surprising};

#[cfg(test)]
mod tests;
