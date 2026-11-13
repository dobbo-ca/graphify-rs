//! Port of graphify-go `internal/ts`: the parser pool and the parse budget over gositter-rs.
//! Owner: the gositter-rs runtime stream; filled by graphify-rs-ihc.15 (plan Task 14). The
//! signatures are shared API.
#[allow(clippy::module_inception)]
mod ts;

pub use ts::{budget, limit, parse};

#[cfg(test)]
mod tests;
