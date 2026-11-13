//! Cross-tier parity: every available block kernel gives the scalar result.

use gostd::sha256::{Tier, block_with, sum256};

const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// FIPS 180-4 padding: 0x80, zeros, 64-bit big-endian bit length.
fn pad(input: &[u8]) -> Vec<u8> {
    let mut m = input.to_vec();
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&((input.len() as u64) << 3).to_be_bytes());
    m
}

fn digest(tier: Tier, padded: &[u8]) -> Option<[u8; 32]> {
    let mut h = H0;
    if !block_with(tier, &mut h, padded) {
        return None;
    }
    let mut out = [0u8; 32];
    for (o, w) in out.chunks_mut(4).zip(h) {
        o.copy_from_slice(&w.to_be_bytes());
    }
    Some(out)
}

struct XorShift64(u64);

impl XorShift64 {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn tiers_agree() {
    let mut rng = XorShift64(0x9e3779b97f4a7c15);
    let mut inputs: Vec<Vec<u8>> = (0..=1024usize)
        .map(|n| (0..n).map(|_| rng.next() as u8).collect())
        .collect();
    for _ in 0..100 {
        let n = (rng.next() % 8192) as usize;
        inputs.push((0..n).map(|_| rng.next() as u8).collect());
    }
    let mut ran = [false; 3];
    for input in &inputs {
        let padded = pad(input);
        let scalar = digest(Tier::Scalar, &padded).expect("scalar tier is always available");
        assert_eq!(
            sum256(input),
            scalar,
            "sum256 vs scalar, len {}",
            input.len()
        );
        for (i, tier) in [Tier::Scalar, Tier::ShaNi, Tier::Armv8]
            .into_iter()
            .enumerate()
        {
            if let Some(d) = digest(tier, &padded) {
                ran[i] = true;
                assert_eq!(d, scalar, "{tier:?} vs scalar, len {}", input.len());
            }
        }
    }
    let names = ["Scalar", "ShaNi", "Armv8"];
    let ran: Vec<_> = names
        .iter()
        .zip(ran)
        .filter(|(_, r)| *r)
        .map(|(n, _)| *n)
        .collect();
    eprintln!("sha256 tiers that ran: {ran:?}");
}
