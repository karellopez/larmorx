// SPDX-License-Identifier: Apache-2.0
//! x87 extended precision, as AFNI's x86-64 build evaluates a few steps of its NIfTI
//! quaternion arithmetic (`specs/3dTshift.md` §6.1 and §7.1, `AFNI_NIFTI_WRITE_TYPE`).
//!
//! An operation on two doubles carried out in x87's 80-bit format and stored back into a
//! double is rounded twice: first to a 64-bit significand (the extended format's precision),
//! then to 53 bits, both to nearest with ties to even. The result differs from the directly
//! rounded double when the first rounding lands exactly on a midpoint of the double grid.
//!
//! Written from the public descriptions of IEEE 754 and of the x87 extended format, with
//! integer arithmetic only, so the result is the same on every platform. Checked against
//! GCC's `long double` on x86-64 Linux (tests below).

/// A finite, non-zero double as `(negative, significand, exponent)`: the value is
/// `±significand · 2^exponent`, with the significand below 2^53.
fn decompose(x: f64) -> (bool, u64, i32) {
    let bits = x.to_bits();
    let negative = bits >> 63 != 0;
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    if biased == 0 {
        (negative, fraction, -1074)
    } else {
        (negative, fraction | (1u64 << 52), biased - 1075)
    }
}

/// Rounds `sig · 2^exp` to `bits` significant bits, to nearest with ties to even. `sticky`
/// says that non-zero bits lie below `sig` (the value is a little more than `sig · 2^exp`).
fn round_to(sig: u128, exp: i32, sticky: bool, bits: u32) -> (u128, i32) {
    let length = 128 - sig.leading_zeros();
    if length <= bits {
        // Callers keep more bits than they round to, so `sticky` cannot be lost here.
        debug_assert!(!sticky);
        return (sig, exp);
    }
    let shift = length - bits;
    let kept = sig >> shift;
    let rest = sig & ((1u128 << shift) - 1);
    let half = 1u128 << (shift - 1);
    let up = rest > half || (rest == half && (sticky || kept & 1 == 1));
    let kept = kept + u128::from(up);
    if kept >> bits != 0 {
        (kept >> 1, exp + shift as i32 + 1)
    } else {
        (kept, exp + shift as i32)
    }
}

/// `±sig · 2^exp` rounded to 64 significant bits, then to a double; `None` outside the range
/// of normal doubles (callers then use the directly rounded result).
fn to_double_via_extended(negative: bool, sig: u128, exp: i32, sticky: bool) -> Option<f64> {
    let (s64, e64) = round_to(sig, exp, sticky, 64);
    let (s53, e53) = round_to(s64, e64, false, 53);
    // The value is s53 · 2^e53 with s53 < 2^53; normal when its leading bit's exponent is.
    let top = e53 + (128 - s53.leading_zeros() as i32) - 1;
    if !(-1022..=1023).contains(&top) || s53 == 0 {
        return None;
    }
    let value = (s53 as f64) * power_of_two(e53)?;
    Some(if negative { -value } else { value })
}

/// `2^e` as a double (normal or subnormal), if it is one.
fn power_of_two(e: i32) -> Option<f64> {
    match e {
        -1022..=1023 => Some(f64::from_bits(((e + 1023) as u64) << 52)),
        -1074..=-1023 => Some(f64::from_bits(1u64 << (e + 1074))),
        _ => None,
    }
}

/// `x / y` evaluated in x87 extended precision and stored as a double (C's
/// `(double)((long double)x / y)` on x86-64).
pub fn div(x: f64, y: f64) -> f64 {
    let direct = x / y;
    if x == 0.0 || y == 0.0 || !x.is_finite() || !y.is_finite() {
        return direct;
    }
    let (nx, mut mx, mut ex) = decompose(x);
    let (ny, mut my, mut ey) = decompose(y);
    // Normalise subnormal significands to 53 bits.
    while mx < 1 << 52 {
        mx <<= 1;
        ex -= 1;
    }
    while my < 1 << 52 {
        my <<= 1;
        ey -= 1;
    }
    // mx/my lies in (1/2, 2): 75 extra bits leave a quotient of 75 or 76 bits, more than
    // enough to round to 64 with a sticky remainder.
    let numerator = u128::from(mx) << 75;
    let quotient = numerator / u128::from(my);
    let remainder = numerator % u128::from(my);
    to_double_via_extended(nx != ny, quotient, ex - ey - 75, remainder != 0).unwrap_or(direct)
}

/// `x · y` evaluated in x87 extended precision and stored as a double.
pub fn mul(x: f64, y: f64) -> f64 {
    let direct = x * y;
    if x == 0.0 || y == 0.0 || !x.is_finite() || !y.is_finite() {
        return direct;
    }
    let (nx, mx, ex) = decompose(x);
    let (ny, my, ey) = decompose(y);
    let product = u128::from(mx) * u128::from(my);
    to_double_via_extended(nx != ny, product, ex + ey, false).unwrap_or(direct)
}

/// `x + y` evaluated in x87 extended precision and stored as a double.
pub fn add(x: f64, y: f64) -> f64 {
    // The exact sum as hi + lo (Knuth's two-sum; exact in round-to-nearest).
    let hi = x + y;
    if !hi.is_finite() {
        return hi;
    }
    let b = hi - x;
    let lo = (x - (hi - b)) + (y - b);
    if lo == 0.0 || hi == 0.0 {
        return hi;
    }
    let (nh, mh, eh) = decompose(hi);
    let (nl, ml, el) = decompose(lo);
    // |lo| is at most half an ulp of hi. Far below it, lo cannot reach a midpoint of the
    // extended grid (half its spacing is 2^(eh-13) at least): the result is hi.
    let d = eh - el;
    if d > 70 {
        return hi;
    }
    let big = (u128::from(mh) << d) as i128;
    let small = i128::from(ml);
    let sum = if nh == nl { big + small } else { big - small };
    // |lo| < ulp(hi), so the sum keeps hi's sign.
    debug_assert!(sum > 0);
    to_double_via_extended(nh, sum as u128, el, false).unwrap_or(hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_operations_are_unchanged() {
        assert_eq!(div(1.0, 4.0), 0.25);
        assert_eq!(div(-3.0, 2.0), -1.5);
        assert_eq!(mul(1.5, -2.0), -3.0);
        assert_eq!(add(1.0, 0.5), 1.5);
        assert_eq!(add(1.0, -1.0), 0.0);
        assert_eq!(div(0.0, 3.0), 0.0);
        assert!(div(1.0, 0.0).is_infinite());
    }

    #[test]
    fn a_double_rounding_of_the_sum() {
        // 1 + 2^-53 + 2^-80: to 64 bits it is 1 + 2^-53, a tie, then 1 (even); rounded
        // directly it is above the midpoint, 1 + 2^-52.
        let s = 2f64.powi(-53) * (1.0 + 2f64.powi(-27));
        assert_eq!(1.0 + s, 1.0 + 2f64.powi(-52));
        assert_eq!(add(1.0, s), 1.0);
        assert_eq!(add(s, 1.0), 1.0);
        // Below the midpoint by less than half an extended ulp: rounds up to the tie, then to
        // even (1 + 2^-52 is odd, 1 + 2·2^-52 even).
        let one_ulp = 1.0 + 2f64.powi(-52);
        let t = 2f64.powi(-53) * (1.0 - 2f64.powi(-30));
        assert_eq!(one_ulp + t, one_ulp);
        assert_eq!(add(one_ulp, t), 1.0 + 2.0 * 2f64.powi(-52));
    }

    /// Cases computed with GCC 13.3 on x86-64 Linux, `(double)((long double)x / y)`,
    /// `(double)((long double)x * y)` and `(double)((long double)x + y)`, where the result differs from the direct double
    /// operation, found among 2 million random operands of each operation, all of which these
    /// functions reproduce bit for bit ([`matches_a_file_of_gcc_results`]).
    #[test]
    fn matches_gcc_long_double() {
        for &(x, y, q) in DIV_CASES {
            let (x, y, q) = (f64::from_bits(x), f64::from_bits(y), f64::from_bits(q));
            assert_eq!(div(x, y).to_bits(), q.to_bits(), "{x:e} / {y:e}");
            assert_ne!((x / y).to_bits(), q.to_bits());
        }
        for &(x, y, p) in MUL_CASES {
            let (x, y, p) = (f64::from_bits(x), f64::from_bits(y), f64::from_bits(p));
            assert_eq!(mul(x, y).to_bits(), p.to_bits(), "{x:e} * {y:e}");
            assert_ne!((x * y).to_bits(), p.to_bits());
        }
        for &(x, y, s) in ADD_CASES {
            let (x, y, s) = (f64::from_bits(x), f64::from_bits(y), f64::from_bits(s));
            assert_eq!(add(x, y).to_bits(), s.to_bits(), "{x:e} + {y:e}");
            assert_ne!((x + y).to_bits(), s.to_bits());
        }
    }

    const DIV_CASES: &[(u64, u64, u64)] = &[
        (0xc1a7406348972795, 0xc0a2c93937a7be32, 0x40f3cda11bc1d064),
        (0x3fb4364480059984, 0x3fbbd35d31a3cd80, 0x3fe73e80cd136478),
        (0x3fe22747ef9794a0, 0x3fdedf16cb0b0e9c, 0x3ff2d12b9a2c8ffe),
        (0xbec41df1895e4ce7, 0x402e60ef05b3eedd, 0xbe8530cdb330ac00),
    ];
    const MUL_CASES: &[(u64, u64, u64)] = &[
        (0x3fe8285d410bc9bc, 0x3fd7762610810ee3, 0x3fd1b6349b04ae4a),
        (0xbf2191230762ead4, 0xbf8e960771fc1c79, 0x3ec0ca6d4171efb8),
        (0xbfadf86ad7bfcbc0, 0x3fe346236542e4a2, 0xbfa20d30005d6838),
        (0x3f3ff2010eda2a83, 0x4227f42f17cd9f58, 0x4177e9b50df2d20e),
    ];
    const ADD_CASES: &[(u64, u64, u64)] = &[
        (0xc10064fda3555013, 0xc02eb52690b0e004, 0xc100657877ef92d6),
        (0xbf2aebffa9fa3359, 0x41df3afcba229840, 0x41df3afcba2294e2),
        (0x3ff0000000000000, 0xbed55696f4c9001e, 0x3feffff554b4859c),
        (0x4039c5388d604ffe, 0x410c94a54b1889c3, 0x410c957374dcf4c6),
    ];

    /// Every line `op x y result` (hex bits; `op` is `d`, `m` or `a`) of the file named by
    /// `LARMORX_X87_CASES`, as written by a C program that evaluates `(double)((long double)x
    /// op y)` with GCC on x86-64. Run with `cargo test -p larmorx-afni x87 -- --ignored`.
    #[test]
    #[ignore = "needs a file of reference results (LARMORX_X87_CASES)"]
    fn matches_a_file_of_gcc_results() {
        let path = std::env::var("LARMORX_X87_CASES").expect("LARMORX_X87_CASES");
        let text = std::fs::read_to_string(path).unwrap();
        let (mut n, mut double_rounded) = (0, 0);
        for line in text.lines() {
            let mut it = line.split_whitespace();
            let op = it.next().unwrap();
            let mut hex = || f64::from_bits(u64::from_str_radix(it.next().unwrap(), 16).unwrap());
            let (x, y, expected) = (hex(), hex(), hex());
            let (ours, direct) = match op {
                "d" => (div(x, y), x / y),
                "m" => (mul(x, y), x * y),
                _ => (add(x, y), x + y),
            };
            assert_eq!(ours.to_bits(), expected.to_bits(), "{line}");
            n += 1;
            double_rounded += usize::from(direct.to_bits() != expected.to_bits());
        }
        eprintln!("{n} cases, {double_rounded} differ from the direct operation");
    }
}
