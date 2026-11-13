//! Generated Unicode tables (crates/gostd-gen) and the types they use. Do not edit the
//! generated files; the `ci` stage regenerates them and fails on a diff.

/// A range of 16-bit code points `lo..=hi` in steps of `stride` (Go `unicode.Range16`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range16 {
    pub lo: u16,
    pub hi: u16,
    pub stride: u16,
}

/// A range of code points above U+FFFF (Go `unicode.Range32`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range32 {
    pub lo: u32,
    pub hi: u32,
    pub stride: u32,
}

/// A set of code points (Go `unicode.RangeTable`). `latin_offset` is the number of `r16`
/// entries with `hi <= U+00FF`.
#[derive(Debug)]
pub struct RangeTable {
    pub r16: &'static [Range16],
    pub r32: &'static [Range32],
    pub latin_offset: usize,
}

/// A case mapping range (Go `unicode.CaseRange`): deltas to upper, lower and title case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaseRange {
    pub lo: u32,
    pub hi: u32,
    pub delta: [i32; 3],
}

/// The delta of an alternating upper/lower run (Go `unicode.UpperLower`, `MaxRune + 1`).
pub const UPPER_LOWER: i32 = 0x110000;

pub(crate) const fn r16(lo: u16, hi: u16, stride: u16) -> Range16 {
    Range16 { lo, hi, stride }
}

pub(crate) const fn r32(lo: u32, hi: u32, stride: u32) -> Range32 {
    Range32 { lo, hi, stride }
}

pub(crate) const fn cr(lo: u32, hi: u32, delta: [i32; 3]) -> CaseRange {
    CaseRange { lo, hi, delta }
}

#[rustfmt::skip]
pub mod unicode17;
