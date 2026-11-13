//! Go `golang.org/x/exp/rand` v0.0.0-20260603202125-055de637280b: the PCG XSL RR 128/64 source
//! and the `Rand` methods graphify-go calls. Call site: graphify-go `louvain.go:47`
//! (`rand.New(rand.NewSource(seed)).Intn`, seed 42 from `cluster.go:16`).

const MULTIPLIER: u128 = 47026247687942121848144207491837523525;
const INCREMENT: u128 = 117397592171526113268558934119004209487;

/// Go `rand.PCGSource`: 128 bits of state in two words.
pub struct PcgSource {
    low: u64,
    high: u64,
}

impl PcgSource {
    /// Go `rand.NewSource(seed)`.
    pub fn new(seed: u64) -> Self {
        Self {
            low: seed,
            high: seed,
        }
    }

    /// Go `(*PCGSource).Seed`: `low = high = seed`.
    pub fn seed(&mut self, seed: u64) {
        self.low = seed;
        self.high = seed;
    }

    /// Go `(*PCGSource).Uint64`.
    pub fn uint64(&mut self) -> u64 {
        self.multiply();
        self.add();
        (self.high ^ self.low).rotate_right((self.high >> 58) as u32)
    }

    /// Go unexported `add` (state += increment, mod 2^128); tests only.
    #[doc(hidden)]
    pub fn add(&mut self) {
        let s = self.wide().wrapping_add(INCREMENT);
        self.set_wide(s);
    }

    /// Go unexported `multiply` (state *= multiplier, mod 2^128); tests only.
    #[doc(hidden)]
    pub fn multiply(&mut self) {
        let s = self.wide().wrapping_mul(MULTIPLIER);
        self.set_wide(s);
    }

    /// `(high, low)`; the Go tests read the fields.
    #[doc(hidden)]
    pub fn state(&self) -> (u64, u64) {
        (self.high, self.low)
    }

    /// Build from `(high, low)`; the Go tests set the fields.
    #[doc(hidden)]
    pub fn from_state(high: u64, low: u64) -> Self {
        Self { low, high }
    }

    fn wide(&self) -> u128 {
        (u128::from(self.high) << 64) | u128::from(self.low)
    }

    fn set_wide(&mut self, s: u128) {
        self.high = (s >> 64) as u64;
        self.low = s as u64;
    }
}

/// Go `rand.Rand` over a `PcgSource`.
pub struct Rand {
    src: PcgSource,
}

impl Rand {
    /// Go `rand.New`.
    pub fn new(src: PcgSource) -> Self {
        Self { src }
    }

    /// Go `(*Rand).Uint64`.
    pub fn uint64(&mut self) -> u64 {
        self.src.uint64()
    }

    /// Go `(*Rand).Uint64n`; panics on `n == 0`.
    pub fn uint64n(&mut self, n: u64) -> u64 {
        Self::uint64n_with(&mut || self.src.uint64(), n)
    }

    /// `Uint64n` over any `Uint64` source (Go's `Source` interface); the Go modulo test uses a
    /// custom source. `uint64n` calls this with the `PcgSource`.
    #[doc(hidden)]
    pub fn uint64n_with(src: &mut impl FnMut() -> u64, n: u64) -> u64 {
        if n & n.wrapping_sub(1) == 0 {
            // n is a power of two, can mask
            assert!(n != 0, "invalid argument to Uint64n");
            return src() & (n - 1);
        }
        // If n does not divide v, to avoid bias we must not use a v that is within
        // u64::MAX % n of the top of the range.
        let mut v = src();
        if v > u64::MAX - n {
            let ceiling = u64::MAX - u64::MAX % n;
            while v >= ceiling {
                v = src();
            }
        }
        v % n
    }

    /// Go `(*Rand).Intn`; panics on `n <= 0`.
    pub fn intn(&mut self, n: i64) -> i64 {
        assert!(n > 0, "invalid argument to Intn");
        self.uint64n(n as u64) as i64
    }
}
