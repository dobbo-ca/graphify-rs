//! Integer parsing: Go `internal/strconv/atoi.go` (1.27.4). Errors are bare kinds; `mod.rs` wraps
//! them in `NumError` with the public function name and the input.

use super::NumErrorKind::{self, BitSize, Range, Syntax};

/// Go `lower` (atoi.go:11).
pub(super) fn lower(c: u8) -> u8 {
    c | (b'x' - b'X')
}

/// Go `ParseUint` (atoi.go:47). On `Range` the value is the maximum for `bit_size` (Go returns it
/// beside the error and `ParseInt` reads it); on any other error it is 0.
fn parse_uint_raw(s: &[u8], base: u32, bit_size: u32) -> (u64, Option<NumErrorKind>) {
    let Some(&first) = s.first() else {
        return (0, Some(Syntax));
    };
    let base0 = base == 0;
    let s0 = s;
    let mut s = s;
    let mut base = base;
    match base {
        2..=36 => {}
        0 => {
            base = 10;
            if first == b'0' {
                match s.get(1).map(|&c| lower(c)) {
                    Some(b'b') if s.len() >= 3 => {
                        base = 2;
                        s = &s[2..];
                    }
                    Some(b'o') if s.len() >= 3 => {
                        base = 8;
                        s = &s[2..];
                    }
                    Some(b'x') if s.len() >= 3 => {
                        base = 16;
                        s = &s[2..];
                    }
                    _ => {
                        base = 8;
                        s = &s[1..];
                    }
                }
            }
        }
        _ => return (0, Some(NumErrorKind::Base(base))),
    }

    let bit_size = if bit_size == 0 { 64 } else { bit_size };
    if bit_size > 64 {
        return (0, Some(BitSize(bit_size)));
    }

    let cutoff = u64::MAX / u64::from(base) + 1;
    let max_val = u64::MAX >> (64 - bit_size);

    let mut underscores = false;
    let mut n: u64 = 0;
    for &c in s {
        let d = match c {
            b'_' if base0 => {
                underscores = true;
                continue;
            }
            b'0'..=b'9' => c - b'0',
            _ if (b'a'..=b'z').contains(&lower(c)) => lower(c) - b'a' + 10,
            _ => return (0, Some(Syntax)),
        };
        if u32::from(d) >= base {
            return (0, Some(Syntax));
        }
        if n >= cutoff {
            return (max_val, Some(Range));
        }
        n *= u64::from(base);
        let n1 = n.wrapping_add(u64::from(d));
        if n1 < n || n1 > max_val {
            return (max_val, Some(Range));
        }
        n = n1;
    }

    if underscores && !underscore_ok(s0) {
        return (0, Some(Syntax));
    }
    (n, None)
}

/// Go `ParseUint`.
pub(super) fn parse_uint(s: &[u8], base: u32, bit_size: u32) -> Result<u64, NumErrorKind> {
    match parse_uint_raw(s, base, bit_size) {
        (n, None) => Ok(n),
        (_, Some(e)) => Err(e),
    }
}

/// Go `ParseInt` (atoi.go:172), including its quirk that a `Range` from `ParseUint` is dropped
/// when the value still fits the signed cutoff (bit size 1).
pub(super) fn parse_int(s: &[u8], base: u32, bit_size: u32) -> Result<i64, NumErrorKind> {
    let Some(&first) = s.first() else {
        return Err(Syntax);
    };
    let mut s = s;
    let mut neg = false;
    match first {
        b'+' => s = &s[1..],
        b'-' => {
            s = &s[1..];
            neg = true;
        }
        _ => {}
    }

    let (un, err) = parse_uint_raw(s, base, bit_size);
    if let Some(e) = err
        && e != Range
    {
        return Err(e);
    }

    let bit_size = if bit_size == 0 { 64 } else { bit_size };
    let cutoff = 1u64 << (bit_size - 1);
    if !neg && un >= cutoff {
        return Err(Range);
    }
    if neg && un > cutoff {
        return Err(Range);
    }
    let n = un as i64;
    Ok(if neg { n.wrapping_neg() } else { n })
}

/// Go `underscoreOK` (atoi.go:252): underscores only between digits or after a base prefix.
pub(super) fn underscore_ok(s: &[u8]) -> bool {
    // saw: b'^' start, b'0' digit or base prefix, b'_' underscore, b'!' anything else.
    let mut saw = b'^';
    let mut i = 0;
    let mut s = s;

    if let Some(&c) = s.first()
        && (c == b'-' || c == b'+')
    {
        s = &s[1..];
    }

    let mut hex = false;
    if s.len() >= 2 && s[0] == b'0' && matches!(lower(s[1]), b'b' | b'o' | b'x') {
        i = 2;
        saw = b'0';
        hex = lower(s[1]) == b'x';
    }

    while i < s.len() {
        let c = s[i];
        i += 1;
        if c.is_ascii_digit() || hex && (b'a'..=b'f').contains(&lower(c)) {
            saw = b'0';
            continue;
        }
        if c == b'_' {
            if saw != b'0' {
                return false;
            }
            saw = b'_';
            continue;
        }
        if saw == b'_' {
            return false;
        }
        saw = b'!';
    }
    saw != b'_'
}
