// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Written for the translation of FSL
// 6.0.7.17's NIfTI library (fsl-newnifti 4.1.0, legacyFunctions.cc); it holds no FSL code.
//! The x87 extended precision of `long double` arithmetic, as FSL's x86-64 build of its NIfTI
//! library computes it.
//!
//! FSL's quaternion routines (`nifti_quatern_to_mat44`, `nifti_mat44_to_quatern`,
//! `nifti_mat33_inverse`) mix `long double` literals (`1.0l`, `0.25l`) into `double`
//! expressions. On x86-64 Linux `long double` is the x87 80-bit format: such an operation is
//! rounded once to a 64-bit significand, and again when the result is stored to a `double` or
//! a `float`. These functions compute that double rounding exactly, with integer arithmetic,
//! for operands that are finite `double`s; results are the same on every platform.
//!
//! Each function takes the operands as they reach the x87 unit (`double`s, exactly
//! representable in extended precision) and returns the stored result. Only normal results are
//! handled by the integer path; zeros, infinities and NaN take the `f64` path, which gives the
//! same value for them.

/// A finite non-zero value as `(-1)^neg * m * 2^e` with an exact integer significand.
#[derive(Clone, Copy, Debug)]
struct Exact {
    neg: bool,
    m: u128,
    e: i32,
}

fn decompose(x: f64) -> Exact {
    let bits = x.to_bits();
    let neg = bits >> 63 == 1;
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    let (m, e) = if exp == 0 {
        (frac as u128, -1074)
    } else {
        ((frac | (1u64 << 52)) as u128, exp - 1075)
    };
    Exact { neg, m, e }
}

/// Rounds `m * 2^e` (with `sticky` set when bits below `m` were non-zero) to `bits` significant
/// bits, ties to even. Returns the rounded significand (`< 2^bits`, or `2^bits` after a carry,
/// renormalised) and exponent.
fn round_to(m: u128, e: i32, sticky: bool, bits: u32) -> (u128, i32) {
    debug_assert!(m != 0);
    let len = 128 - m.leading_zeros();
    if len <= bits {
        // Exact already (a sticky remainder below cannot exist without dropped bits).
        debug_assert!(!sticky);
        return (m, e);
    }
    let shift = len - bits;
    let kept = m >> shift;
    let dropped = m & ((1u128 << shift) - 1);
    let half = 1u128 << (shift - 1);
    let round_up = if dropped > half || (dropped == half && sticky) {
        true
    } else if dropped == half {
        kept & 1 == 1
    } else {
        false
    };
    let mut kept = kept + u128::from(round_up);
    let mut e = e + shift as i32;
    if kept >> bits != 0 {
        kept >>= 1;
        e += 1;
    }
    (kept, e)
}

/// `m * 2^e` (rounded to at most 64 bits) rounded again to `bits` bits, as an `f64` value.
fn to_f64(neg: bool, m: u128, e: i32) -> f64 {
    // m < 2^64 here; f64 holds it with a further rounding to 53 bits.
    let (m53, e53) = round_to(m, e, false, 53);
    let v = (m53 as f64) * pow2(e53);
    if neg { -v } else { v }
}

fn to_f32(neg: bool, m: u128, e: i32) -> f32 {
    let (m24, e24) = round_to(m, e, false, 24);
    let v = ((m24 as f64) * pow2(e24)) as f32;
    if neg { -v } else { v }
}

/// `2^e` as an `f64` (normal range).
fn pow2(e: i32) -> f64 {
    if e >= -1022 {
        f64::from_bits(((e + 1023) as u64) << 52)
    } else {
        // Below the normal range: scale in two steps (results are normal in the callers).
        f64::from_bits(((e + 1023 + 600) as u64) << 52)
            * f64::from_bits(((1023 - 600) as u64) << 52)
    }
}

/// The exact sum `a + b` of two finite non-zero values, rounded to 64 bits; `None` when it is
/// zero. The exponents must be at most 70 apart (see [`add_to_f64`]).
fn add_ext(a: Exact, b: Exact) -> Option<(bool, u128, i32)> {
    let (hi, lo) = if a.e >= b.e { (a, b) } else { (b, a) };
    let diff = (hi.e - lo.e) as u32;
    debug_assert!(diff <= 70);
    let mh = hi.m << diff;
    let ml = lo.m;
    let (neg, m) = if hi.neg == lo.neg {
        (hi.neg, mh + ml)
    } else if mh >= ml {
        (hi.neg, mh - ml)
    } else {
        (lo.neg, ml - mh)
    };
    if m == 0 {
        return None;
    }
    let (r, e) = round_to(m, lo.e, false, 64);
    Some((neg, r, e))
}

/// `(double)((long double)a + (long double)b)`. When the exponents are more than 70 apart the
/// smaller operand is below half a unit of the larger's 64-bit rounding, so both roundings
/// give the larger operand, as a plain `f64` sum does.
pub fn add_to_f64(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 || !a.is_finite() || !b.is_finite() {
        return a + b;
    }
    let (x, y) = (decompose(a), decompose(b));
    if (x.e - y.e).abs() > 70 {
        return a + b;
    }
    match add_ext(x, y) {
        Some((neg, m, e)) => to_f64(neg, m, e),
        None => 0.0,
    }
}

/// `(double)((long double)a - (long double)b)`.
pub fn sub_to_f64(a: f64, b: f64) -> f64 {
    add_to_f64(a, -b)
}

/// The product `a * b`, rounded to 64 bits.
fn mul_ext(a: f64, b: f64) -> (bool, u128, i32) {
    let x = decompose(a);
    let y = decompose(b);
    let m = x.m * y.m;
    let (r, e) = round_to(m, x.e + y.e, false, 64);
    (x.neg != y.neg, r, e)
}

/// `(double)((long double)a * (long double)b)`.
pub fn mul_to_f64(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 || !a.is_finite() || !b.is_finite() {
        return a * b;
    }
    let (neg, m, e) = mul_ext(a, b);
    to_f64(neg, m, e)
}

/// `(float)((long double)a * (long double)b)`.
pub fn mul_to_f32(a: f64, b: f64) -> f32 {
    if a == 0.0 || b == 0.0 || !a.is_finite() || !b.is_finite() {
        return (a * b) as f32;
    }
    let (neg, m, e) = mul_ext(a, b);
    to_f32(neg, m, e)
}

/// The quotient `a / b`, rounded to 64 bits.
fn div_ext(a: f64, b: f64) -> (bool, u128, i32) {
    let x = decompose(a);
    let y = decompose(b);
    // Normalise both significands to 53 bits (subnormal inputs), then shift the dividend so
    // that the quotient has at least 66 bits.
    let nx = x.m << (x.m.leading_zeros() - 75);
    let ex = x.e - (x.m.leading_zeros() as i32 - 75);
    let ny = y.m << (y.m.leading_zeros() - 75);
    let ey = y.e - (y.m.leading_zeros() as i32 - 75);
    let shift = 68u32;
    let num = nx << shift;
    let q = num / ny;
    let rem = num % ny;
    let (r, e) = round_sticky(q, ex - ey - shift as i32, rem != 0, 64);
    (x.neg != y.neg, r, e)
}

/// [`round_to`] when bits below `m` (a non-zero remainder) were lost.
fn round_sticky(m: u128, e: i32, sticky: bool, bits: u32) -> (u128, i32) {
    let len = 128 - m.leading_zeros();
    assert!(len > bits, "quotient must carry guard bits");
    let shift = len - bits;
    let kept = m >> shift;
    let dropped = m & ((1u128 << shift) - 1);
    let half = 1u128 << (shift - 1);
    let round_up = dropped > half || (dropped == half && (sticky || kept & 1 == 1));
    let mut kept = kept + u128::from(round_up);
    let mut e = e + shift as i32;
    if kept >> bits != 0 {
        kept >>= 1;
        e += 1;
    }
    (kept, e)
}

/// `(double)((long double)a / (long double)b)`.
pub fn div_to_f64(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 || !a.is_finite() || !b.is_finite() {
        return a / b;
    }
    let (neg, m, e) = div_ext(a, b);
    to_f64(neg, m, e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_operations_are_plain() {
        assert_eq!(add_to_f64(1.0, 2.0), 3.0);
        assert_eq!(sub_to_f64(1.0, 0.25), 0.75);
        assert_eq!(mul_to_f64(3.0, 0.5), 1.5);
        assert_eq!(div_to_f64(1.0, 4.0), 0.25);
        assert_eq!(mul_to_f32(2.0, 1.5), 3.0f32);
    }

    #[test]
    fn single_rounding_cases_match_f64() {
        // When the exact result needs at most 64 bits, extended then double equals double.
        for &(a, b) in &[(1.1, 2.2), (0.3, 0.7), (123.456, 1e-3), (-5.5, 1e-10)] {
            assert_eq!(add_to_f64(a, b), a + b);
        }
        // Division: compare with a reference computed with 128-bit arithmetic by hand for a
        // case that rounds twice: 1/3 rounds the same in both.
        assert_eq!(div_to_f64(1.0, 3.0), 1.0 / 3.0);
    }

    #[test]
    fn double_rounding_differs_from_direct() {
        // 1 + (2^-53 + 2^-80): rounded directly to 53 bits it is above the midpoint and rounds
        // up to 1 + 2^-52; rounded first to 64 bits the 2^-80 is lost, leaving the exact tie
        // 1 + 2^-53, which rounds to even: 1.
        let b = 2f64.powi(-53) + 2f64.powi(-80);
        assert_eq!(1.0 + b, 1.0 + f64::EPSILON);
        assert_eq!(add_to_f64(1.0, b), 1.0);
        let x = 1.0 + f64::EPSILON;
        assert_eq!(div_to_f64(x, x), 1.0);
        // A product rounded to float through extended precision.
        let a = 1.0 + 2f64.powi(-24) + 2f64.powi(-50);
        assert_eq!(mul_to_f32(a, 1.0), (a as f32));
        assert_eq!(
            mul_to_f32(1.0 + 2f64.powi(-24), 1.0 + 2f64.powi(-46)),
            1.0 + f32::EPSILON
        );
    }
}
