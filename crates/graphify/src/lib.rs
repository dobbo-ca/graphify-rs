//! Code knowledge graph builder. Rust port of graphify-go v0.12.0 (35fcd9e).
//!
//! One module per Go package (`cmd/graphify` is [`cli`]); each Go file `x.go` is ported to
//! `<module>/x.rs`, so the two read side by side. Plan: `docs/superpowers/plans/2026-11-13-p2-graphify.md`.

pub mod analyze;
pub mod cache;
pub mod cli;
pub mod cluster;
pub mod detect;
pub mod export;
pub mod extract;
pub mod fsutil;
pub mod graph;
pub mod idutil;
pub mod langfamily;
pub mod model;
pub mod query;
pub mod report;
pub mod security;
pub mod semantic;
pub mod skills;
pub mod ts;

#[cfg(test)]
mod testutil;
