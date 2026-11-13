//! Float parsing. Go `internal/strconv/atof.go` (1.27.4): the syntax layer (`special`,
//! `readFloat`), `atofHex`, and for decimal strings Rust's correctly rounded `str::parse::<f64>`.

use super::NumErrorKind::{self, Range, Syntax};
use super::atoi::{lower, underscore_ok};

/// Go `commonPrefixLenIgnoreCase` (atof.go:29); `prefix` is lower case.
fn common_prefix_len_ignore_case(s: &[u8], prefix: &[u8]) -> usize {
    s.iter()
        .zip(prefix)
        .take_while(|&(&c, &p)| c.to_ascii_lowercase() == p)
        .count()
}

/// Go `special` (atof.go:48): the value and consumed length of a leading `inf`, `infinity`
/// (signed) or `nan` (unsigned, any case).
fn special(s: &[u8]) -> Option<(f64, usize)> {
    let (&first, _) = s.split_first()?;
    let (sign, nsign, rest) = match first {
        b'+' => (1.0, 1, &s[1..]),
        b'-' => (-1.0, 1, &s[1..]),
        b'i' | b'I' => (1.0, 0, s),
        b'n' | b'N' => {
            // Go nan() is Float64frombits(0x7FF8000000000001), not Rust's 0x7FF8000000000000.
            let nan = f64::from_bits(0x7FF8_0000_0000_0001);
            return (common_prefix_len_ignore_case(s, b"nan") == 3).then_some((nan, 3));
        }
        _ => return None,
    };
    let mut n = common_prefix_len_ignore_case(rest, b"infinity");
    if 3 < n && n < 8 {
        n = 3;
    }
    (n == 3 || n == 8).then_some((sign * f64::INFINITY, nsign + n))
}

struct Parsed {
    mantissa: u64,
    exp: i64,
    neg: bool,
    trunc: bool,
    hex: bool,
    /// bytes consumed
    n: usize,
}

/// Go `readFloat` (atof.go:182). `None` when not valid.
fn read_float(s: &[u8]) -> Option<Parsed> {
    let mut underscores = false;
    let mut i = 0;
    let mut neg = false;
    match *s.first()? {
        b'+' => i += 1,
        b'-' => {
            i += 1;
            neg = true;
        }
        _ => {}
    }

    let mut base = 10u64;
    let mut max_mant_digits = 19;
    let mut exp_char = b'e';
    let mut hex = false;
    if i + 2 < s.len() && s[i] == b'0' && lower(s[i + 1]) == b'x' {
        base = 16;
        max_mant_digits = 16;
        i += 2;
        exp_char = b'p';
        hex = true;
    }
    let mut sawdot = false;
    let mut sawdigits = false;
    let mut trunc = false;
    let mut mantissa = 0u64;
    let (mut nd, mut nd_mant, mut dp) = (0i64, 0i64, 0i64);
    while i < s.len() {
        let c = s[i];
        match c {
            b'_' => underscores = true,
            b'.' => {
                if sawdot {
                    break;
                }
                sawdot = true;
                dp = nd;
            }
            b'0'..=b'9' => {
                sawdigits = true;
                if c == b'0' && nd == 0 {
                    dp -= 1;
                } else {
                    nd += 1;
                    if nd_mant < max_mant_digits {
                        mantissa = mantissa * base + u64::from(c - b'0');
                        nd_mant += 1;
                    } else if c != b'0' {
                        trunc = true;
                    }
                }
            }
            _ if base == 16 && (b'a'..=b'f').contains(&lower(c)) => {
                sawdigits = true;
                nd += 1;
                if nd_mant < max_mant_digits {
                    mantissa = mantissa * 16 + u64::from(lower(c) - b'a' + 10);
                    nd_mant += 1;
                } else {
                    trunc = true;
                }
            }
            _ => break,
        }
        i += 1;
    }
    if !sawdigits {
        return None;
    }
    if !sawdot {
        dp = nd;
    }
    if base == 16 {
        dp *= 4;
        nd_mant *= 4;
    }

    if i < s.len() && lower(s[i]) == exp_char {
        i += 1;
        let mut esign = 1;
        match *s.get(i)? {
            b'+' => i += 1,
            b'-' => {
                i += 1;
                esign = -1;
            }
            _ => {}
        }
        if !s.get(i)?.is_ascii_digit() {
            return None;
        }
        let mut e = 0i64;
        while i < s.len() && (s[i].is_ascii_digit() || s[i] == b'_') {
            if s[i] == b'_' {
                underscores = true;
            } else if e < 10000 {
                e = e * 10 + i64::from(s[i] - b'0');
            }
            i += 1;
        }
        dp += e * esign;
    } else if base == 16 {
        return None;
    }

    let exp = if mantissa != 0 { dp - nd_mant } else { 0 };
    if underscores && !underscore_ok(&s[..i]) {
        return None;
    }
    Some(Parsed {
        mantissa,
        exp,
        neg,
        trunc,
        hex,
        n: i,
    })
}

/// Go `atofHex` (atof.go:507) for float64 (mantbits 52, expbits 11, bias -1023).
fn atof_hex(mut mantissa: u64, mut exp: i64, neg: bool, trunc: bool) -> (f64, bool) {
    const MANTBITS: u32 = 52;
    const EXPBITS: u32 = 11;
    const BIAS: i64 = -1023;
    let max_exp = (1i64 << EXPBITS) + BIAS - 2;
    let min_exp = BIAS + 1;
    exp += i64::from(MANTBITS);

    while mantissa != 0 && mantissa >> (MANTBITS + 2) == 0 {
        mantissa <<= 1;
        exp -= 1;
    }
    if trunc {
        mantissa |= 1;
    }
    while mantissa >> (1 + MANTBITS + 2) != 0 {
        mantissa = (mantissa >> 1) | (mantissa & 1);
        exp += 1;
    }
    while mantissa > 1 && exp < min_exp - 2 {
        mantissa = (mantissa >> 1) | (mantissa & 1);
        exp += 1;
    }

    let mut round = mantissa & 3;
    mantissa >>= 2;
    round |= mantissa & 1;
    exp += 2;
    if round == 3 {
        mantissa += 1;
        if mantissa == 1 << (1 + MANTBITS) {
            mantissa >>= 1;
            exp += 1;
        }
    }

    if mantissa >> MANTBITS == 0 {
        exp = BIAS;
    }
    let mut overflow = false;
    if exp > max_exp {
        mantissa = 1 << MANTBITS;
        exp = max_exp + 1;
        overflow = true;
    }

    let mut bits = mantissa & ((1 << MANTBITS) - 1);
    bits |= (((exp - BIAS) & ((1 << EXPBITS) - 1)) as u64) << MANTBITS;
    if neg {
        bits |= 1 << (MANTBITS + EXPBITS);
    }
    (f64::from_bits(bits), overflow)
}

/// Go `ParseFloat(s, 64)` (atof.go:719, `atof64` at atof.go:634). Decimal strings: syntax is
/// validated by `read_float`, then the digits (underscores removed) go to `str::parse`.
pub(super) fn parse_float(s: &str) -> Result<f64, NumErrorKind> {
    let b = s.as_bytes();
    if let Some((val, n)) = special(b) {
        return if n == b.len() { Ok(val) } else { Err(Syntax) };
    }
    let p = read_float(b).filter(|p| p.n == b.len()).ok_or(Syntax)?;
    let (f, overflow) = if p.hex {
        atof_hex(p.mantissa, p.exp, p.neg, p.trunc)
    } else {
        let clean: String = s.chars().filter(|&c| c != '_').collect();
        let f: f64 = clean.parse().map_err(|_| Syntax)?;
        (f, f.is_infinite())
    };
    if overflow { Err(Range) } else { Ok(f) }
}
