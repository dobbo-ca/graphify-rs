//! ARMv8 SHA-2 block kernel (as Go's `sha256block_arm64.s`).
#![allow(unsafe_code)]

use super::scalar::K;
use core::arch::aarch64::*;

/// # Safety
/// The CPU must support `sha2`.
#[target_feature(enable = "sha2")]
unsafe fn compress_ni(h: &mut [u32; 8], blocks: &[[u8; 64]]) {
    // SAFETY: `h` holds 8 u32 (two 4-lane loads in bounds).
    let (mut s0, mut s1) = unsafe { (vld1q_u32(h.as_ptr()), vld1q_u32(h[4..].as_ptr())) };
    for block in blocks {
        let (save0, save1) = (s0, s1);
        let mut m = [vdupq_n_u32(0); 4];
        for (j, mj) in m.iter_mut().enumerate() {
            // SAFETY: 16 bytes at offset 16 * j < 64 of the 64-byte block.
            let v = unsafe { vld1q_u8(block[16 * j..].as_ptr()) };
            *mj = vreinterpretq_u32_u8(vrev32q_u8(v));
        }
        for i in 0..16 {
            let cur = i % 4;
            // SAFETY: K has 64 words; 4 * i + 4 <= 64.
            let k = unsafe { vld1q_u32(K[4 * i..].as_ptr()) };
            let wk = vaddq_u32(m[cur], k);
            let prev0 = s0;
            s0 = vsha256hq_u32(s0, s1, wk);
            s1 = vsha256h2q_u32(s1, prev0, wk);
            if i < 12 {
                let su0 = vsha256su0q_u32(m[cur], m[(i + 1) % 4]);
                m[cur] = vsha256su1q_u32(su0, m[(i + 2) % 4], m[(i + 3) % 4]);
            }
        }
        s0 = vaddq_u32(s0, save0);
        s1 = vaddq_u32(s1, save1);
    }
    // SAFETY: `h` holds 8 u32 (two 4-lane stores in bounds).
    unsafe {
        vst1q_u32(h.as_mut_ptr(), s0);
        vst1q_u32(h[4..].as_mut_ptr(), s1);
    }
}

/// Run the ARMv8 SHA-256 kernel; `false` (h untouched) when the CPU lacks it.
pub(super) fn compress(h: &mut [u32; 8], blocks: &[[u8; 64]]) -> bool {
    if !std::arch::is_aarch64_feature_detected!("sha2") {
        return false;
    }
    // SAFETY: sha2 detected above.
    unsafe { compress_ni(h, blocks) };
    true
}
