//! graphify-go `internal/model` tests (`code_test.go`, `model_test.go`) and the plan's added
//! tests.

mod code;
mod jsonenc;
mod model;

/// A small deterministic generator for the property tests (Go uses `math/rand` seed 1; the
/// tests compare two code paths, so any sequence serves).
pub(super) struct Rng(u64);

impl Rng {
    pub(super) fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    /// Go `rng.Intn(n)`: a value in `0..n`.
    pub(super) fn intn(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }

    /// Go `rng.Shuffle`.
    pub(super) fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.intn(i + 1);
            v.swap(i, j);
        }
    }
}
