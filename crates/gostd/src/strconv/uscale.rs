//! Go 1.27.4 `internal/strconv/uscale.go`: shortest float digits by unrounded scaling
//! (`shortFloat` and the helpers it reaches). Rust's `{:e}` is not used: on a tie between two
//! shortest candidates it does not pick the digits Go picks (ftoa_test.go row
//! 1.0867823635813762e+14).

use super::pow10tab::{POW10_MIN, POW10_TAB};

const MANT_BITS: i32 = 52; // ftoa.go float64MantBits
const MIN_EXP: i32 = -1085; // ftoa.go float64MinExp

/// Go `unrounded` (uscale.go:51): a value times 4, with a sticky low bit.
#[derive(Clone, Copy)]
struct Unrounded(u64);

impl Unrounded {
    /// Go `unrounded.floor` (uscale.go:53).
    fn floor(self) -> u64 {
        self.0 >> 2
    }

    /// Go `unrounded.round` (uscale.go:55): round half to even.
    fn round(self) -> u64 {
        self.0.wrapping_add(1 + ((self.0 >> 2) & 1)) >> 2
    }

    /// Go `unrounded.ceil` (uscale.go:57).
    fn ceil(self) -> u64 {
        self.0.wrapping_add(3) >> 2
    }

    /// Go `unrounded.nudge` (uscale.go:58); Go's uint64 arithmetic wraps.
    fn nudge(self, delta: i64) -> Unrounded {
        Unrounded(self.0.wrapping_add_signed(delta))
    }
}

/// Go `log10Pow2` (uscale.go:70): floor(x * log10 2).
fn log10_pow2(x: i32) -> i32 {
    (x * 78913) >> 18
}

/// Go `log2Pow10` (uscale.go:76): floor(x * log2 10).
fn log2_pow10(x: i32) -> i32 {
    (x * 108853) >> 15
}

/// Go `skewed` (uscale.go:220).
fn skewed(e: i32) -> i32 {
    (e * 631305 - 261663) >> 21
}

/// Go `scaler` (uscale.go:231).
struct Scaler {
    pm_hi: u64,
    pm_lo: u64,
    s: i32,
}

/// Go `prescale` (uscale.go:246).
fn prescale(e: i32, p: i32, lp: i32) -> Scaler {
    let (pm_hi, pm_lo) = POW10_TAB[(p - POW10_MIN) as usize];
    Scaler {
        pm_hi,
        pm_lo,
        s: -(e + lp + 3),
    }
}

/// Go `uscale` (uscale.go:258): unround(x * 2**e * 10**p).
fn uscale(x: u64, c: &Scaler) -> Unrounded {
    let full = u128::from(x) * u128::from(c.pm_hi);
    let (mut hi, mid) = ((full >> 64) as u64, full as u64);
    let s = (c.s & 63) as u32;
    if (hi >> s) << s != hi {
        return Unrounded((hi >> s) | 1);
    }
    let mid2 = ((u128::from(x) * u128::from(c.pm_lo)) >> 64) as u64;
    hi = hi.wrapping_sub(u64::from(mid < mid2));
    Unrounded((hi >> s) | u64::from(mid.wrapping_sub(mid2) > 1))
}

/// Go `shortFloat[float64]` (uscale.go:169): the shortest decimal d * 10**p that rounds back to
/// m * 2**e (m left-justified, high bit set).
pub(super) fn short_float(m: u64, e: i32) -> (u64, i32) {
    let mut z = 63 - MANT_BITS;
    let (p, min, max);
    if m == 1 << 63 && e > MIN_EXP {
        p = -skewed(e + z);
        min = m.wrapping_sub(1 << (z - 2));
        max = m.wrapping_add(1 << (z - 1));
    } else if e >= MIN_EXP {
        p = -log10_pow2(e + z);
        min = m.wrapping_sub(1 << (z - 1));
        max = m.wrapping_add(1 << (z - 1));
    } else {
        z += MIN_EXP - e;
        p = -log10_pow2(e + z);
        min = m.wrapping_sub(1 << (z - 1));
        max = m.wrapping_add(1 << (z - 1));
    }
    let odd = ((m >> z) & 1) as i64;

    let pre = prescale(e, p, log2_pow10(p));
    let dmin = uscale(min, &pre).nudge(odd).ceil();
    let dmax = uscale(max, &pre).nudge(-odd).floor();

    let d = dmax / 10;
    if d * 10 >= dmin {
        return (d, -(p - 1));
    }
    let mut d = dmin;
    if d < dmax {
        d = uscale(m, &pre).round();
    }
    (d, -p)
}
