//! Go `golang.org/x/exp/rand` v0.0.0-20260603202125-055de637280b: the PCG XSL RR 128/64 source
//! and the `Rand` methods graphify-go calls. Call site: graphify-go `louvain.go:47`
//! (`rand.New(rand.NewSource(seed)).Intn`, seed 42 from `cluster.go:16`).
#![allow(unused_variables, dead_code)] // stubs; removed in PR B

/// Go `rand.PCGSource`: 128 bits of state in two words.
pub struct PcgSource {
    low: u64,
    high: u64,
}

impl PcgSource {
    /// Go `rand.NewSource(seed)`.
    pub fn new(seed: u64) -> Self {
        todo!("graphify-rs-aa6.17")
    }

    /// Go `(*PCGSource).Seed`: `low = high = seed`.
    pub fn seed(&mut self, seed: u64) {
        todo!("graphify-rs-aa6.17")
    }

    /// Go `(*PCGSource).Uint64`.
    pub fn uint64(&mut self) -> u64 {
        todo!("graphify-rs-aa6.17")
    }

    /// Go unexported `add` (state += increment, mod 2^128); tests only.
    #[doc(hidden)]
    pub fn add(&mut self) {
        todo!("graphify-rs-aa6.17")
    }

    /// Go unexported `multiply` (state *= multiplier, mod 2^128); tests only.
    #[doc(hidden)]
    pub fn multiply(&mut self) {
        todo!("graphify-rs-aa6.17")
    }

    /// `(high, low)`; the Go tests read the fields.
    #[doc(hidden)]
    pub fn state(&self) -> (u64, u64) {
        todo!("graphify-rs-aa6.17")
    }

    /// Build from `(high, low)`; the Go tests set the fields.
    #[doc(hidden)]
    pub fn from_state(high: u64, low: u64) -> Self {
        todo!("graphify-rs-aa6.17")
    }
}

/// Go `rand.Rand` over a `PcgSource`.
pub struct Rand {
    src: PcgSource,
}

impl Rand {
    /// Go `rand.New`.
    pub fn new(src: PcgSource) -> Self {
        todo!("graphify-rs-aa6.17")
    }

    /// Go `(*Rand).Uint64`.
    pub fn uint64(&mut self) -> u64 {
        todo!("graphify-rs-aa6.17")
    }

    /// Go `(*Rand).Uint64n`; panics on `n == 0`.
    pub fn uint64n(&mut self, n: u64) -> u64 {
        todo!("graphify-rs-aa6.17")
    }

    /// `Uint64n` over any `Uint64` source (Go's `Source` interface); the Go modulo test uses a
    /// custom source. `uint64n` calls this with the `PcgSource`.
    #[doc(hidden)]
    pub fn uint64n_with(src: &mut impl FnMut() -> u64, n: u64) -> u64 {
        todo!("graphify-rs-aa6.17")
    }

    /// Go `(*Rand).Intn`; panics on `n <= 0`.
    pub fn intn(&mut self, n: i64) -> i64 {
        todo!("graphify-rs-aa6.17")
    }
}
