//! Go `math` 1.27.4: `Log`, as graphify-go calls it at `ask.go:180` (plan R10).

/// Go `math.Log` on amd64: the assembly `archLog` of `math/log_amd64.s`, ported instruction by
/// instruction. Every step is one scalar f64 operation in the assembly's order, with no fused
/// multiply-add, so the result is bit-equal to Go on amd64 on any target. On arm64 Go runs the
/// pure-Go `log` (`math/log.go`) compiled with FMA, which can differ in the last bit (D-004).
///
/// The assembly differs from the pure-Go `log` in three places, all kept here:
/// - a negative input (including -NaN) returns the NaN `0x7FF8000000000001`; a positive NaN is
///   returned unchanged;
/// - the frexp step does not normalise a subnormal: it takes the stored mantissa with exponent
///   field 0 as `f1` and `k = -1022`;
/// - `CMPSD X2, X0, 5` is `!(HSqrt2 < f1)`, so `f1` is doubled when `f1 <= HSqrt2`, not only
///   when `f1 < HSqrt2`.
///
/// Steps (the assembly line, then what it computes):
/// - `ANDQ BX, AX` with `~(1<<63)`, `JEQ isZero`: `±0` returns `-Inf`.
/// - `CMPQ AX(0), BX`, `JGT isNegative`: sign bit set returns NaN `0x7FF8000000000001`.
/// - `CMPQ AX(PosInf), BX`, `JLE isInfOrNaN`: bits `>= 0x7FF0000000000000` (+Inf, +NaN)
///   return `x`.
/// - `ANDPD` mantissa mask, `ORPD` 0.5: `f1` = the mantissa bits under the exponent of 0.5.
/// - `SHRQ $52`, `ANDL $0x7FF`, `SUBL $0x3FE`, `CVTSL2SD`: `k` = exponent field - 1022, as f64.
/// - `CMPSD X2, X0, 5`, `ANDPD` 1.0: `c` = 1.0 if `f1 <= HSqrt2`, else 0.0.
/// - `SUBSD X3, X1`: `k = k - c`. `ADDSD X0, X3`, `MULSD X3, X2`: `f1 = f1 * (c + 1.0)`.
/// - `SUBSD X0, X2`: `f = f1 - 1.0`.
/// - `ADDSD X2, X0` (2.0), `DIVSD X0, X3`: `s = f / (2.0 + f)`.
/// - `MULSD X4, X4`: `s2 = s * s`. `MULSD X5, X5`: `s4 = s2 * s2`.
/// - `L7`, `MULSD`, `ADDSD L5`, `MULSD`, `ADDSD L3`, `MULSD`, `ADDSD L1`, `MULSD X6, X4`:
///   `t1 = s2 * (L1 + s4 * (L3 + s4 * (L5 + s4 * L7)))`, one rounding per operation.
/// - `L6`, `MULSD`, `ADDSD L4`, `MULSD`, `ADDSD L2`, `MULSD X6, X5`:
///   `t2 = s4 * (L2 + s4 * (L4 + s4 * L6))`.
/// - `ADDSD X5, X4`: `r = t1 + t2`.
/// - `MOVSD $0.5`, `MULSD X2, X0` twice: `hfsq = (0.5 * f) * f`.
/// - `ADDSD X0, X4`, `MULSD X4, X3`: `s * (r + hfsq)`.
/// - `MULSD X1, X4` (Ln2Lo), `ADDSD X4, X3`: `+ k * Ln2Lo`.
/// - `SUBSD X3, X0`, `SUBSD X2, X0`: `(hfsq - that) - f`.
/// - `MULSD $Ln2Hi, X1`, `SUBSD X0, X1`: `k * Ln2Hi - that`.
pub fn log(x: f64) -> f64 {
    // The #define constants of log_amd64.s, by their bit patterns.
    const HSQRT2: f64 = f64::from_bits(0x3FE6_A09E_667F_3BCD); // 7.07106781186547524401e-01
    const LN2HI: f64 = f64::from_bits(0x3FE6_2E42_FEE0_0000);
    const LN2LO: f64 = f64::from_bits(0x3DEA_39EF_3579_3C76);
    const L1: f64 = f64::from_bits(0x3FE5_5555_5555_5593);
    const L2: f64 = f64::from_bits(0x3FD9_9999_9997_FA04);
    const L3: f64 = f64::from_bits(0x3FD2_4924_9422_9359);
    const L4: f64 = f64::from_bits(0x3FCC_71C5_1D8E_78AF);
    const L5: f64 = f64::from_bits(0x3FC7_4664_96CB_03DE);
    const L6: f64 = f64::from_bits(0x3FC3_9A09_D078_C69F);
    const L7: f64 = f64::from_bits(0x3FC2_F112_DF3E_5244);
    const NAN: u64 = 0x7FF8_0000_0000_0001;
    const POS_INF: u64 = 0x7FF0_0000_0000_0000;

    let bx = x.to_bits();
    if bx & !(1 << 63) == 0 {
        return f64::NEG_INFINITY; // isZero: NegInf 0xFFF0000000000000
    }
    if (bx as i64) < 0 {
        return f64::from_bits(NAN); // isNegative
    }
    if bx >= POS_INF {
        return x; // isInfOrNaN
    }
    let f1 = f64::from_bits((bx & 0x000F_FFFF_FFFF_FFFF) | 0x3FE0_0000_0000_0000);
    let k = f64::from(((bx >> 52) & 0x7FF) as i32 - 0x3FE);
    let c = if f1 <= HSQRT2 { 1.0 } else { 0.0 };
    let k = k - c;
    let f1 = f1 * (c + 1.0);
    let f = f1 - 1.0;
    let s = f / (2.0 + f);
    let s2 = s * s;
    let s4 = s2 * s2;
    let mut t = L7 * s4;
    t += L5;
    t *= s4;
    t += L3;
    t *= s4;
    t += L1;
    let t1 = s2 * t;
    let mut t = L6 * s4;
    t += L4;
    t *= s4;
    t += L2;
    let t2 = s4 * t;
    let r = t1 + t2;
    let hfsq = 0.5 * f * f;
    let a = s * (r + hfsq);
    let a = a + LN2LO * k;
    let a = hfsq - a;
    let a = a - f;
    k * LN2HI - a
}
