// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! C's `long double` on x86-64 Linux (the x87 80-bit format), for the few expressions of FSL's
//! NIfTI layer that mix `long double` literals into double formulas (`1.0l - x`, `0.25l * x / a`
//! in NewNifti's quaternion routines, `1.0l / det` in its matrix inverses).
//!
//! GCC evaluates such an expression with x87 instructions: the operation's exact result is
//! rounded to a 64-bit significand (Linux's default precision control), and that value is
//! rounded again to the type it is stored in (`double` or `float`). The two roundings can give
//! another value than one rounding would; these functions reproduce both, with integer
//! arithmetic. Every operation here takes two `f64` operands (exactly representable as
//! `long double`), which is all NewNifti's expressions need (checked with `objdump`:
//! `fldl`/`fmull`/`fdivl`/`fstpl` in `nifti_mat44_to_quatern`).

/// An exact binary value: `(-1)^neg * mant * 2^exp`, and whether nonzero bits were cut below
/// `mant` (`sticky`). `mant` may be 0 (a zero).
#[derive(Clone, Copy, Debug)]
struct Exact {
    neg: bool,
    mant: u128,
    exp: i32,
    sticky: bool,
}

/// `(neg, mant, exp)` of a finite `f64`: `x = (-1)^neg * mant * 2^exp`.
fn split(x: f64) -> (bool, u64, i32) {
    let bits = x.to_bits();
    let neg = bits >> 63 != 0;
    let e = ((bits >> 52) & 0x7ff) as i32;
    let f = bits & ((1u64 << 52) - 1);
    if e == 0 {
        (neg, f, -1074)
    } else {
        (neg, f | (1u64 << 52), e - 1075)
    }
}

fn bit_len(m: u128) -> i32 {
    128 - m.leading_zeros() as i32
}

/// Rounds the magnitude of `v` to nearest-even at the bit of weight `2^lsb_exp`, where
/// `lsb_exp` is chosen so that the result has at most `bits` significant bits and its last bit
/// has a weight of at least `2^min_lsb` (the format's smallest step, for subnormals). Returns an
/// exact value (`sticky` false).
fn round(v: Exact, bits: i32, min_lsb: i32) -> Exact {
    if v.mant == 0 {
        return Exact { sticky: false, ..v };
    }
    let top = v.exp + bit_len(v.mant) - 1; // exponent of the leading bit
    let lsb = (top - bits + 1).max(min_lsb);
    let shift = lsb - v.exp;
    if shift <= 0 {
        // Exactly representable at this precision, unless bits were cut below.
        debug_assert!(!v.sticky, "a cut value must carry guard bits");
        return Exact {
            sticky: false,
            ..v
        };
    }
    let (keep, rem, half) = if shift >= 128 {
        (0u128, v.mant, None)
    } else {
        let keep = v.mant >> shift;
        let rem = v.mant & ((1u128 << shift) - 1);
        (keep, rem, Some(1u128 << (shift - 1)))
    };
    let up = match half {
        None => false, // far below half a step (shift >= 128 never happens for these inputs)
        Some(h) => rem > h || (rem == h && (v.sticky || keep & 1 == 1)),
    };
    let keep = keep + u128::from(up);
    Exact {
        neg: v.neg,
        mant: keep,
        exp: lsb,
        sticky: false,
    }
}

/// The x87 rounding: a 64-bit significand, exponents down to the format's subnormals.
fn to_extended(v: Exact) -> Exact {
    round(v, 64, -16445)
}

/// An exact value with at most 53 significant bits, in `f64`'s range, as `f64`.
fn assemble_f64(v: Exact) -> f64 {
    if v.mant == 0 {
        return if v.neg { -0.0 } else { 0.0 };
    }
    // mant * 2^exp, mant < 2^54 (a carry can make it 2^53): build it in two exact steps.
    let m = v.mant as f64; // exact: at most 54 bits, a power of two when 54
    let r = scale(m, v.exp);
    if v.neg { -r } else { r }
}

/// `m * 2^e` for an exactly representable result (or an overflow to infinity).
fn scale(mut m: f64, mut e: i32) -> f64 {
    while e > 1000 {
        m *= 2f64.powi(1000);
        e -= 1000;
    }
    while e < -1000 {
        m *= 2f64.powi(-1000);
        e += 1000;
    }
    m * 2f64.powi(e)
}

/// The `long double` value `v` stored in a `double`.
fn store_f64(v: Exact) -> f64 {
    let e = to_extended(v);
    let top = e.exp + bit_len(e.mant) - 1;
    if e.mant != 0 && top > 1023 {
        return if e.neg {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    assemble_f64(round(e, 53, -1074))
}

/// The `long double` value `v` stored in a `float`.
fn store_f32(v: Exact) -> f32 {
    let e = to_extended(v);
    let top = e.exp + bit_len(e.mant) - 1;
    if e.mant != 0 && top > 127 {
        return if e.neg {
            f32::NEG_INFINITY
        } else {
            f32::INFINITY
        };
    }
    let r = round(e, 24, -149);
    // At most 25 bits (a carry), within f32's range: exact in f64, then exact in f32.
    assemble_f64(r) as f32
}

fn exact(x: f64) -> Exact {
    let (neg, m, e) = split(x);
    Exact {
        neg,
        mant: u128::from(m),
        exp: e,
        sticky: false,
    }
}

/// The exact sum `a + b` (with a sticky bit when the smaller operand is cut).
fn add_exact(a: f64, b: f64) -> Exact {
    let (xa, xb) = (exact(a), exact(b));
    if xa.mant == 0 {
        return if xb.mant == 0 {
            Exact {
                neg: xa.neg && xb.neg,
                ..xa
            }
        } else {
            xb
        };
    }
    if xb.mant == 0 {
        return xa;
    }
    // Order by magnitude: `big` has the larger leading bit (or the larger value at equal ones).
    let lead = |v: &Exact| v.exp + bit_len(v.mant);
    let (big, small) = if lead(&xa) > lead(&xb) || (lead(&xa) == lead(&xb) && a.abs() >= b.abs())
    {
        (xa, xb)
    } else {
        (xb, xa)
    };
    // `big` with 70 guard bits: at most 53 + 70 = 123 bits.
    let e = big.exp - 70;
    let big_m = big.mant << 70;
    let d = small.exp - e;
    let (small_m, cut) = if d >= 0 {
        (small.mant << d, false)
    } else {
        let s = -d;
        if s >= 128 {
            (0, true)
        } else {
            (small.mant >> s, small.mant & ((1u128 << s) - 1) != 0)
        }
    };
    if big.neg == small.neg {
        Exact {
            neg: big.neg,
            mant: big_m + small_m,
            exp: e,
            sticky: cut,
        }
    } else if cut {
        // big - (small_m + a fraction of a step) = (big_m - small_m - 1) + (1 - fraction).
        Exact {
            neg: big.neg,
            mant: big_m - small_m - 1,
            exp: e,
            sticky: true,
        }
    } else {
        let m = big_m - small_m;
        Exact {
            neg: if m == 0 { false } else { big.neg },
            mant: m,
            exp: e,
            sticky: false,
        }
    }
}

fn mul_exact(a: f64, b: f64) -> Exact {
    let (xa, xb) = (exact(a), exact(b));
    Exact {
        neg: xa.neg != xb.neg,
        mant: xa.mant * xb.mant,
        exp: xa.exp + xb.exp,
        sticky: false,
    }
}

/// The quotient `a / b` with at least 70 significant bits and a sticky bit.
fn div_exact(a: f64, b: f64) -> Exact {
    let (xa, xb) = (exact(a), exact(b));
    let neg = xa.neg != xb.neg;
    // Both significands have at most 53 bits: a << 74 < 2^127, and the quotient has 74 or more.
    let num = xa.mant << 74;
    let q = num / xb.mant;
    let r = num % xb.mant;
    Exact {
        neg,
        mant: q,
        exp: xa.exp - 74 - xb.exp,
        sticky: r != 0,
    }
}

/// `(long double) a + b`, stored in a `double`.
pub fn add_to_f64(a: f64, b: f64) -> f64 {
    if !(a.is_finite() && b.is_finite()) {
        return a + b;
    }
    store_f64(add_exact(a, b))
}

/// `(long double) a - b`, stored in a `double`.
pub fn sub_to_f64(a: f64, b: f64) -> f64 {
    add_to_f64(a, -b)
}

/// `(long double) a * b`, stored in a `float`.
pub fn mul_to_f32(a: f64, b: f64) -> f32 {
    if !(a.is_finite() && b.is_finite()) {
        return (a * b) as f32;
    }
    store_f32(mul_exact(a, b))
}

/// `(long double) a * b`, stored in a `double`.
pub fn mul_to_f64(a: f64, b: f64) -> f64 {
    if !(a.is_finite() && b.is_finite()) {
        return a * b;
    }
    store_f64(mul_exact(a, b))
}

/// `(long double) a / b`, stored in a `double`.
pub fn div_to_f64(a: f64, b: f64) -> f64 {
    if !(a.is_finite() && b.is_finite()) || b == 0.0 || a == 0.0 {
        return a / b;
    }
    store_f64(div_exact(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_cases_are_the_double_results() {
        for (a, b) in [(1.0, 2.0), (0.5, 0.25), (3.0, -7.0), (1e300, 1e-300), (-2.5, 4.0)] {
            assert_eq!(add_to_f64(a, b), a + b);
            assert_eq!(mul_to_f64(a, b), a * b);
            assert_eq!(div_to_f64(a, b), a / b);
        }
        assert_eq!(sub_to_f64(1.0, 1.0), 0.0);
        assert_eq!(mul_to_f32(3.0, 0.5), 1.5f32);
    }

    #[test]
    fn double_rounding_differs_from_one_rounding() {
        // 1 + 2^-53 + 2^-64: rounded to 64 bits it is 1 + 2^-53 + 2^-64 (exact, 65 bits? no:
        // its lowest bit has weight 2^-64, within 64 bits), then to 53 bits the tie 1 + 2^-53
        // goes to even (1). One rounding of the exact sum gives 1 + 2^-52.
        let a = 1.0 + 2f64.powi(-52); // 1 + 2^-52
        let b = -(2f64.powi(-53) - 2f64.powi(-64)); // -(2^-53 - 2^-64)
        // exact: 1 + 2^-53 + 2^-64: one rounding to double gives 1 + 2^-52
        assert_eq!(a + b, 1.0 + 2f64.powi(-52));
        // x87: to 64 bits it stays exact (lsb 2^-63 > 2^-64? the value needs 65 bits, so it is
        // rounded to 1 + 2^-53 + 0 or + 2^-63; 2^-64 is half of 2^-63: a tie to even gives
        // 1 + 2^-53), then 1 + 2^-53 to 53 bits is a tie to even: 1.
        assert_eq!(add_to_f64(a, b), 1.0);
    }

    #[test]
    fn division_matches_double_where_double_rounding_is_harmless() {
        // A quotient whose 64-bit rounding cannot create a tie: the results agree.
        let mut s = 0x9e3779b97f4a7c15u64;
        let mut differ = 0;
        for _ in 0..100_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let a = f64::from_bits(0x3ff0000000000000 | (s >> 12));
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let b = f64::from_bits(0x3ff0000000000000 | (s >> 12));
            if div_to_f64(a, b) != a / b {
                differ += 1;
            }
        }
        // Double rounding of a quotient through 64 bits is rare (about 1 in 2^11 for random
        // operands): a few, never most.
        assert!(differ < 200, "{differ}");
    }
}
