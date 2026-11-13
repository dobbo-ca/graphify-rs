//! Shortest float formatting. Go `internal/strconv/ftoa.go` (1.27.4): `ftoa64` with precision -1
//! (digits from `uscale::short_float`, as Go) and the `fmtEFG` layout.

use super::uscale::short_float;

/// Go `ftoa64` (ftoa.go:217) with `prec` -1: special values, then `fmtEFG` with the shortest
/// precision rules. `fmt` is already checked.
pub(super) fn append_float(dst: &mut Vec<u8>, f: f64, fmt: u8) {
    if f.is_nan() {
        dst.extend_from_slice(b"NaN");
        return;
    }
    if f.is_infinite() {
        dst.extend_from_slice(if f < 0.0 { b"-Inf" } else { b"+Inf" });
        return;
    }
    let neg = f.is_sign_negative();
    if f == 0.0 {
        // Go: mant == 0 goes to fmtEFG(neg, nil, 0, 0, prec -1, shortest).
        fmt_efg(dst, neg, &[], 0, 0, -1, fmt, true);
        return;
    }
    // Go ftoa64 (ftoa.go:217-283): unpack, left-justify the mantissa, shortFloat, setDigits.
    let bits = f.to_bits();
    let mut exp = ((bits >> 52) & 0x7FF) as i32;
    let mut mant = bits & ((1 << 52) - 1);
    if exp == 0 {
        exp += 1;
    } else {
        mant |= 1 << 52;
    }
    exp -= 1023; // float64Bias
    let s = mant.leading_zeros() as i32;
    let (d, p) = short_float(mant << s, exp - s - 52);
    let mut digits = d.to_string().into_bytes();
    let dp = digits.len() as i64 + i64::from(p);
    while digits.last() == Some(&b'0') {
        digits.pop();
    }
    let nd = digits.len() as i64;
    let prec = match fmt {
        b'e' | b'E' => (nd - 1).max(0),
        b'f' => (nd - dp).max(0),
        _ => nd,
    };
    fmt_efg(dst, neg, &digits, dp, nd, prec, fmt, true);
}

/// Go `fmtEFG` (ftoa.go:347), `%e`, `%f`, `%g` layouts.
#[allow(clippy::too_many_arguments)] // mirrors the argument list of Go fmtEFG
fn fmt_efg(
    dst: &mut Vec<u8>,
    neg: bool,
    s: &[u8],
    dp: i64,
    nd: i64,
    mut prec: i64,
    mut fmt: u8,
    shortest: bool,
) {
    if fmt == b'g' || fmt == b'G' {
        let mut eprec = prec;
        if eprec > nd && nd >= dp {
            eprec = nd;
        }
        if shortest {
            eprec = 6;
        }
        let exp = dp - 1;
        if exp < -4 || exp >= eprec {
            if prec > nd {
                prec = nd;
            }
            prec -= 1;
            fmt = fmt + b'e' - b'g';
        } else {
            if prec > dp {
                prec = nd;
            }
            prec = (prec - dp).max(0);
            fmt = b'f';
        }
    }

    if neg {
        dst.push(b'-');
    }
    if fmt == b'e' || fmt == b'E' {
        dst.push(if nd != 0 { s[0] } else { b'0' });
        if prec > 0 {
            dst.push(b'.');
            let mut i = 1;
            let m = nd.min(prec + 1);
            if i < m {
                dst.extend_from_slice(&s[i as usize..m as usize]);
                i = m;
            }
            dst.resize(dst.len() + (prec + 1 - i) as usize, b'0');
        }
        dst.push(fmt);
        let mut exp = if nd == 0 { 0 } else { dp - 1 };
        dst.push(if exp < 0 {
            exp = -exp;
            b'-'
        } else {
            b'+'
        });
        // at least two digits, three for 100 and up
        if exp >= 100 {
            dst.push(b'0' + (exp / 100) as u8);
        }
        dst.push(b'0' + (exp / 10 % 10) as u8);
        dst.push(b'0' + (exp % 10) as u8);
    } else {
        // 'f'
        if dp > 0 {
            let m = nd.min(dp);
            dst.extend_from_slice(&s[..m as usize]);
            dst.resize(dst.len() + (dp - m) as usize, b'0');
        } else {
            dst.push(b'0');
        }
        if prec > 0 {
            dst.push(b'.');
            let lz = prec.min((-dp).max(0));
            let off = dp + lz;
            let m = (prec - lz).min((nd - off).max(0));
            let tz = (prec - lz - m).max(0);
            dst.resize(dst.len() + lz as usize, b'0');
            for i in 0..m {
                dst.push(s[(off + i) as usize]);
            }
            dst.resize(dst.len() + tz as usize, b'0');
        }
    }
}
