//! Port of graphify-go `internal/cache`. Filled by graphify-rs-ihc.19 (plan Task 18).

#[allow(clippy::module_inception)]
mod cache;

pub use cache::{
    Cache, Entry, Error, FILE_NAME, Output, STAT_FILE_NAME, StatEntry, StatIndex, hash_bytes,
    hash_file, load, load_stat, load_stat_out, save, save_drain, save_stat, stale, stamp,
    stat_fresh,
};

#[cfg(test)]
mod tests;
