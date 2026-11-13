//! Go `crypto/sha256` (go1.27.4): SHA-256 only, as graphify-go calls it.
//!
//! Call sites in graphify-go: `cache.go:417` (`sha256.Sum256`), `outfp.go:39` (`Sum256` printed
//! with `%x`) and `outfp.go:45` (`sha256.New` with many small `Fprintf` writes).

#[cfg(target_arch = "aarch64")]
mod aarch64;
mod scalar;
#[cfg(target_arch = "x86_64")]
mod x86;

use std::io;
use std::sync::OnceLock;

/// Go `sha256.Size`: digest length in bytes.
pub const SIZE: usize = 32;
/// Go `sha256.BlockSize`: block length in bytes.
pub const BLOCK_SIZE: usize = 64;

const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Go `sha256.Sum256`.
pub fn sum256(data: &[u8]) -> [u8; 32] {
    let mut d = Digest::new();
    d.write(data);
    d.sum()
}

/// Go `sha256.New()`: a running SHA-256 (Go's `digest` layout).
#[derive(Clone)]
pub struct Digest {
    h: [u32; 8],
    x: [u8; 64],
    nx: usize,
    len: u64,
}

impl Digest {
    /// Go `sha256.New`.
    pub fn new() -> Self {
        Digest {
            h: H0,
            x: [0; 64],
            nx: 0,
            len: 0,
        }
    }

    /// Go `Write`: never fails.
    pub fn write(&mut self, mut p: &[u8]) {
        self.len = self.len.wrapping_add(p.len() as u64);
        if self.nx > 0 {
            let n = p.len().min(64 - self.nx);
            self.x[self.nx..self.nx + n].copy_from_slice(&p[..n]);
            self.nx += n;
            if self.nx == 64 {
                block(&mut self.h, std::slice::from_ref(&self.x));
                self.nx = 0;
            }
            p = &p[n..];
        }
        let (blocks, rest) = p.as_chunks::<64>();
        block(&mut self.h, blocks);
        self.x[..rest.len()].copy_from_slice(rest);
        self.nx += rest.len();
    }

    /// Go `Sum(nil)`: the digest of what was written so far; does not change the state.
    pub fn sum(&self) -> [u8; 32] {
        let mut d = self.clone();
        let len = d.len;
        let mut tmp = [0u8; 64];
        tmp[0] = 0x80;
        let pad = (if len % 64 < 56 {
            56 - len % 64
        } else {
            64 + 56 - len % 64
        }) as usize;
        d.write(&tmp[..pad]);
        d.write(&(len << 3).to_be_bytes());
        debug_assert_eq!(d.nx, 0);
        let mut out = [0u8; 32];
        for (o, w) in out.as_chunks_mut::<4>().0.iter_mut().zip(d.h) {
            *o = w.to_be_bytes();
        }
        out
    }

    /// Go `Reset`.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for Digest {
    fn default() -> Self {
        Self::new()
    }
}

impl io::Write for Digest {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Digest::write(self, buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
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
    static TIER: OnceLock<Tier> = OnceLock::new();
    *TIER.get_or_init(|| {
        if std::env::var("GOSTD_SIMD").is_ok_and(|v| v == "scalar") {
            Tier::Scalar
        } else if available(Tier::ShaNi) {
            Tier::ShaNi
        } else if available(Tier::Armv8) {
            Tier::Armv8
        } else {
            Tier::Scalar
        }
    })
}

/// Whether this CPU can run `tier` (Go: `sha256block_amd64.go:12`, `sha256block_arm64.go:12`).
fn available(tier: Tier) -> bool {
    match tier {
        Tier::Scalar => true,
        #[cfg(target_arch = "x86_64")]
        Tier::ShaNi => {
            is_x86_feature_detected!("sha")
                && is_x86_feature_detected!("sse4.1")
                && is_x86_feature_detected!("ssse3")
        }
        #[cfg(target_arch = "aarch64")]
        Tier::Armv8 => std::arch::is_aarch64_feature_detected!("sha2"),
        _ => false,
    }
}

/// Run `tier` over whole blocks on state `h`; `false` when the CPU lacks the tier.
fn run_tier(tier: Tier, h: &mut [u32; 8], blocks: &[[u8; 64]]) -> bool {
    if !available(tier) {
        return false;
    }
    match tier {
        Tier::Scalar => scalar::compress(h, blocks),
        #[cfg(target_arch = "x86_64")]
        Tier::ShaNi => return x86::compress(h, blocks),
        #[cfg(target_arch = "aarch64")]
        Tier::Armv8 => return aarch64::compress(h, blocks),
        _ => return false,
    }
    true
}

fn block(h: &mut [u32; 8], blocks: &[[u8; 64]]) {
    let ok = run_tier(active_tier(), h, blocks);
    debug_assert!(ok);
}

/// Run `tier`'s block function over `blocks` (a multiple of 64 bytes) on state `h`.
/// Returns `false` when the CPU lacks the tier. A tail shorter than 64 bytes is ignored.
#[doc(hidden)]
pub fn block_with(tier: Tier, h: &mut [u32; 8], blocks: &[u8]) -> bool {
    run_tier(tier, h, blocks.as_chunks::<64>().0)
}
