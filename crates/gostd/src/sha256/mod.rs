#![allow(unused_variables)] // removed in PR B (graphify-rs-aa6.18)

//! Go `crypto/sha256` (go1.27.4): SHA-256 only, as graphify-go calls it.
//!
//! Call sites in graphify-go: `cache.go:417` (`sha256.Sum256`), `outfp.go:39` (`Sum256` printed
//! with `%x`) and `outfp.go:45` (`sha256.New` with many small `Fprintf` writes).

use std::io;

/// Go `sha256.Size`: digest length in bytes.
pub const SIZE: usize = 32;
/// Go `sha256.BlockSize`: block length in bytes.
pub const BLOCK_SIZE: usize = 64;

/// Go `sha256.Sum256`.
pub fn sum256(data: &[u8]) -> [u8; 32] {
    todo!("graphify-rs-aa6.18")
}

/// Go `sha256.New()`: a running SHA-256 (Go's `digest` layout).
#[derive(Clone)]
#[allow(dead_code)] // removed in PR B (graphify-rs-aa6.18)
pub struct Digest {
    h: [u32; 8],
    x: [u8; 64],
    nx: usize,
    len: u64,
}

impl Digest {
    /// Go `sha256.New`.
    pub fn new() -> Self {
        todo!("graphify-rs-aa6.18")
    }

    /// Go `Write`: never fails.
    pub fn write(&mut self, p: &[u8]) {
        todo!("graphify-rs-aa6.18")
    }

    /// Go `Sum(nil)`: the digest of what was written so far; does not change the state.
    pub fn sum(&self) -> [u8; 32] {
        todo!("graphify-rs-aa6.18")
    }

    /// Go `Reset`.
    pub fn reset(&mut self) {
        todo!("graphify-rs-aa6.18")
    }
}

impl Default for Digest {
    fn default() -> Self {
        Self::new()
    }
}

impl io::Write for Digest {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        todo!("graphify-rs-aa6.18")
    }

    fn flush(&mut self) -> io::Result<()> {
        todo!("graphify-rs-aa6.18")
    }
}

/// Block kernel in use.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Scalar,
    ShaNi,
    Armv8,
}

/// The tier chosen for this process.
#[doc(hidden)]
pub fn active_tier() -> Tier {
    todo!("graphify-rs-aa6.18")
}

/// Run `tier`'s block function over `blocks` (a multiple of 64 bytes) on state `h`.
/// Returns `false` when the CPU lacks the tier.
#[doc(hidden)]
pub fn block_with(tier: Tier, h: &mut [u32; 8], blocks: &[u8]) -> bool {
    todo!("graphify-rs-aa6.18")
}
