// SPDX-License-Identifier: Apache-2.0
//! x87 extended precision, as FSL's NIfTI library evaluates a few steps of its quaternion
//! arithmetic on x86-64 (`specs/bet.md` §8.2).
//!
//! An operation on two doubles carried out in x87's 80-bit format and stored back into a
//! double (or a float) is rounded twice: first to a 64-bit significand (the extended format's
//! precision), then to 53 (or 24) bits, both to nearest with ties to even. The result differs
//! from the directly rounded one when the first rounding lands exactly on a midpoint.
//!
//! The same method as `larmorx-afni`'s `x87` module (larmorx's own clean-room code, Apache-2.0:
//! written from the public descriptions of IEEE 754 and of the x87 extended format, checked
//! there against GCC's `long double` on x86-64 Linux), with the rounding to float added. Integer
//! arithmetic only, so the result is the same on every platform.

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
/// says that non-zero bits lie below `sig`.
fn round_to(sig: u128, exp: i32, sticky: bool, bits: u32) -> (u128, i32) {
    let length = 128 - sig.leading_zeros();
    if length <= bits {
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

/// `2^e` as a double, if it is a normal one.
fn power_of_two(e: i32) -> Option<f64> {
    (-1022..=1023)
        .contains(&e)
        .then(|| f64::from_bits(((e + 1023) as u64) << 52))
}

/// `±sig · 2^exp` rounded to 64 significant bits, then to `bits` (53: a double, 24: a float);
/// `None` outside the normal range of the target (callers then use the direct result).
fn via_extended(negative: bool, sig: u128, exp: i32, sticky: bool, bits: u32) -> Option<f64> {
    let (s64, e64) = round_to(sig, exp, sticky, 64);
    let (s, e) = round_to(s64, e64, false, bits);
    let top = e + (128 - s.leading_zeros() as i32) - 1;
    let range = if bits == 24 { -126..=127 } else { -1022..=1023 };
    if !range.contains(&top) || s == 0 {
        return None;
    }
    // s has at most `bits` bits: exact as a double, and the power of two is exact too
    let value = (s as f64)
        * power_of_two(e).or_else(|| {
            // a tiny power of two times a large significand: split the scaling
            Some(power_of_two(e + 64)? * power_of_two(-64)?)
        })?;
    Some(if negative { -value } else { value })
}

/// `x / y` in x87 extended precision, stored as a double.
pub fn div(x: f64, y: f64) -> f64 {
    let direct = x / y;
    if x == 0.0 || y == 0.0 || !x.is_finite() || !y.is_finite() {
        return direct;
    }
    let (nx, mut mx, mut ex) = decompose(x);
    let (ny, mut my, mut ey) = decompose(y);
    while mx < 1 << 52 {
        mx <<= 1;
        ex -= 1;
    }
    while my < 1 << 52 {
        my <<= 1;
        ey -= 1;
    }
    let numerator = u128::from(mx) << 75;
    let quotient = numerator / u128::from(my);
    let remainder = numerator % u128::from(my);
    via_extended(nx != ny, quotient, ex - ey - 75, remainder != 0, 53).unwrap_or(direct)
}

/// `x · y` in x87 extended precision, rounded once from extended to a float (C's
/// `(float)((long double)x * y)`).
pub fn mul_to_f32(x: f64, y: f64) -> f32 {
    let direct = (x * y) as f32;
    if x == 0.0 || y == 0.0 || !x.is_finite() || !y.is_finite() {
        return direct;
    }
    let (nx, mx, ex) = decompose(x);
    let (ny, my, ey) = decompose(y);
    let product = u128::from(mx) * u128::from(my);
    via_extended(nx != ny, product, ex + ey, false, 24).map_or(direct, |v| v as f32)
}

/// `x + y` in x87 extended precision, stored as a double.
pub fn add(x: f64, y: f64) -> f64 {
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
    let d = eh - el;
    if d > 70 {
        return hi;
    }
    let big = (u128::from(mh) << d) as i128;
    let small = i128::from(ml);
    let sum = if nh == nl { big + small } else { big - small };
    if sum <= 0 {
        return hi;
    }
    via_extended(nh, sum as u128, el, false, 53).unwrap_or(hi)
}

/// `x − y` in x87 extended precision, stored as a double.
pub fn sub(x: f64, y: f64) -> f64 {
    add(x, -y)
}

/// Whether the double `a` is below `1e-7` as an extended constant (`1e-7l`): that constant
/// (`15474250491067253436 · 2^-87`) lies above the double nearest `1e-7` and below the next
/// double, so the comparison is `a ≤ 1e-7` in double.
pub fn below_ext_1e7(a: f64) -> bool {
    a <= 1.0e-7
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_operations_are_unchanged() {
        assert_eq!(div(1.0, 4.0), 0.25);
        assert_eq!(add(1.0, 0.5), 1.5);
        assert_eq!(sub(1.0, 1.0), 0.0);
        assert_eq!(mul_to_f32(1.5, -2.0), -3.0);
        assert!(below_ext_1e7(1.0e-7));
        assert!(!below_ext_1e7(f64::from_bits(1.0e-7f64.to_bits() + 1)));
    }

    #[test]
    fn rounded_once_from_extended_to_float() {
        // x · y = 1 + 2^-24 + 2^-51 + 2^-75 (x = 1 + 2^-24, y = 1 + 2^-51): in extended the
        // 2^-51 is kept and the float rounding goes up; through a double too
        let (x, y) = (1.0 + 2f64.powi(-24), 1.0 + 2f64.powi(-51));
        assert_eq!(mul_to_f32(x, y), 1.0 + 2f32.powi(-23));
        // x · y = 1 + 2^-24 exactly: a tie, to even
        assert_eq!(mul_to_f32(1.0 + 2f64.powi(-24), 1.0), 1.0);
        // (1 + 2^-23 + 2^-24) · (1 + 2^-52): the extended keeps the 2^-52 term above the tie
        // (up, to 1 + 2^-22); a double drops it, and its tie goes to even (also 1 + 2^-22)
        let a = 1.0 + 2f64.powi(-23) + 2f64.powi(-24);
        assert_eq!(mul_to_f32(a, 1.0 + 2f64.powi(-52)), 1.0 + 2f32.powi(-22));
    }

    /// Cases of `larmorx-afni`'s module, from GCC 13.3's `long double` on x86-64 Linux.
    #[test]
    fn matches_gcc_long_double() {
        let div_cases: &[(u64, u64, u64)] = &[
            (0xc1a7406348972795, 0xc0a2c93937a7be32, 0x40f3cda11bc1d064),
            (0x3fb4364480059984, 0x3fbbd35d31a3cd80, 0x3fe73e80cd136478),
        ];
        for &(x, y, q) in div_cases {
            assert_eq!(div(f64::from_bits(x), f64::from_bits(y)).to_bits(), q);
        }
        let add_cases: &[(u64, u64, u64)] = &[
            (0xc10064fda3555013, 0xc02eb52690b0e004, 0xc100657877ef92d6),
            (0x3ff0000000000000, 0xbed55696f4c9001e, 0x3feffff554b4859c),
        ];
        for &(x, y, s) in add_cases {
            assert_eq!(add(f64::from_bits(x), f64::from_bits(y)).to_bits(), s);
        }
    }
}
