//! SHA-NI block kernel (Intel SHA extensions reference flow, as Go's `sha256block_amd64.s`).
#![allow(unsafe_code)]

use super::scalar::K;
use core::arch::x86_64::*;

/// # Safety
/// The CPU must support `sha`, `sse2`, `ssse3` and `sse4.1`.
#[target_feature(enable = "sha,sse2,ssse3,sse4.1")]
unsafe fn compress_ni(h: &mut [u32; 8], blocks: &[[u8; 64]]) {
    // Byte swap of each 32-bit word (big-endian message words).
    let mask = _mm_set_epi64x(0x0c0d0e0f08090a0b, 0x0405060700010203);
    // SAFETY: `h` holds 8 u32 (two unaligned 16-byte loads in bounds).
    let (dcba, hgfe) = unsafe {
        (
            _mm_loadu_si128(h.as_ptr().cast()),
            _mm_loadu_si128(h[4..].as_ptr().cast()),
        )
    };
    // Pack ABCD/EFGH into the ABEF/CDGH layout `sha256rnds2` works on.
    let tmp = _mm_shuffle_epi32::<0xB1>(dcba);
    let hgfe = _mm_shuffle_epi32::<0x1B>(hgfe);
    let mut s0 = _mm_alignr_epi8::<8>(tmp, hgfe);
    let mut s1 = _mm_blend_epi16::<0xF0>(hgfe, tmp);

    for block in blocks {
        let (save0, save1) = (s0, s1);
        let mut m = [_mm_setzero_si128(); 4];
        for i in 0..16 {
            let cur = i % 4;
            if i < 4 {
                // SAFETY: 16 bytes at offset 16 * i < 64 of the 64-byte block.
                let v = unsafe { _mm_loadu_si128(block[16 * i..].as_ptr().cast()) };
                m[cur] = _mm_shuffle_epi8(v, mask);
            }
            // SAFETY: K has 64 words; 4 * i + 4 <= 64.
            let k = unsafe { _mm_loadu_si128(K[4 * i..].as_ptr().cast()) };
            let wk = _mm_add_epi32(m[cur], k);
            s1 = _mm_sha256rnds2_epu32(s1, s0, wk);
            if (3..15).contains(&i) {
                let next = (i + 1) % 4;
                let t = _mm_alignr_epi8::<4>(m[cur], m[(i + 3) % 4]);
                m[next] = _mm_sha256msg2_epu32(_mm_add_epi32(m[next], t), m[cur]);
            }
            s0 = _mm_sha256rnds2_epu32(s0, s1, _mm_shuffle_epi32::<0x0E>(wk));
            if (1..13).contains(&i) {
                let prev = (i + 3) % 4;
                m[prev] = _mm_sha256msg1_epu32(m[prev], m[cur]);
            }
        }
        s0 = _mm_add_epi32(s0, save0);
        s1 = _mm_add_epi32(s1, save1);
    }

    // Unpack ABEF/CDGH back to ABCD/EFGH.
    let tmp = _mm_shuffle_epi32::<0x1B>(s0);
    let s1 = _mm_shuffle_epi32::<0xB1>(s1);
    let dcba = _mm_blend_epi16::<0xF0>(tmp, s1);
    let hgfe = _mm_alignr_epi8::<8>(s1, tmp);
    // SAFETY: `h` holds 8 u32 (two unaligned 16-byte stores in bounds).
    unsafe {
        _mm_storeu_si128(h.as_mut_ptr().cast(), dcba);
        _mm_storeu_si128(h[4..].as_mut_ptr().cast(), hgfe);
    }
}

/// Run the SHA-NI kernel; `false` (h untouched) when the CPU lacks it.
pub(super) fn compress(h: &mut [u32; 8], blocks: &[[u8; 64]]) -> bool {
    if !(is_x86_feature_detected!("sha")
        && is_x86_feature_detected!("sse4.1")
        && is_x86_feature_detected!("ssse3"))
    {
        return false;
    }
    // SAFETY: sha, sse4.1 and ssse3 detected above (sse2 is x86_64 baseline).
    unsafe { compress_ni(h, blocks) };
    true
}
