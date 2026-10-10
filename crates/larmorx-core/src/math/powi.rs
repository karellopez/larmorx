// SPDX-License-Identifier: Apache-2.0
//! `x^n` for an integer `n`, correctly rounded, computed exactly with integer arithmetic.
//!
//! SciPy's and ITK's B-spline prefilters call the C library's `pow(z, n)` with a pole `z` and a
//! line length `n`. glibc's `pow` is correctly rounded for those arguments except in rare cases
//! (5 of 12,012 pole powers, all below 1e-100; `docs/findings/scipy-ndimage.md`). This
//! function is the exactly rounded value: the mantissa is raised to the power `n` as a big
//! integer and rounded once, to nearest with ties to even, subnormals included.

/// `x^n`, correctly rounded (ties to even), for finite `x`.
///
/// Exact integer arithmetic costs O(n²) word operations, so results that certainly overflow
/// or underflow are recognised first from an estimate of their exponent. Within the range of
/// `f64` the work is bounded: the exponent of the result limits `n` to about 1075 / |log2 x|
/// (about 570 for the cubic B-spline pole). `x` close to 1 with a huge `n` is slow; the
/// prefilters never ask for that.
#[must_use]
pub fn powi(x: f64, n: u32) -> f64 {
    if n == 0 {
        return 1.0;
    }
    if !x.is_finite() {
        return libm_like_nonfinite(x, n);
    }
    let negative = x.is_sign_negative() && n % 2 == 1;
    let signed = |v: f64| if negative { -v } else { v };
    if x == 0.0 {
        return signed(0.0);
    }
    // |x| = m · 2^e with m odd.
    let bits = x.abs().to_bits();
    let raw_exp = ((bits >> 52) & 0x7ff) as i64;
    let frac = bits & ((1u64 << 52) - 1);
    let (mut m, mut e) = if raw_exp == 0 {
        (frac, -1074i64)
    } else {
        (frac | (1u64 << 52), raw_exp - 1075)
    };
    let tz = m.trailing_zeros();
    m >>= tz;
    e += i64::from(tz);
    if m == 1 {
        // A power of two: exact unless it leaves the range.
        return signed(pow2(e * i64::from(n)));
    }
    // log2 of the result, within a few units: m^n has between n·(L-1) and n·L bits.
    let len = i64::from(64 - m.leading_zeros());
    let n64 = i64::from(n);
    let lead_min = n64 * (len - 1) + e * n64; // exponent of the leading bit, lower bound
    let lead_max = n64 * len + e * n64; // upper bound
    if lead_max < -1076 {
        return signed(0.0);
    }
    if lead_min > 1024 {
        return signed(f64::INFINITY);
    }
    // m^n exactly, as little-endian 64-bit limbs.
    let mut big: Vec<u64> = vec![1];
    for _ in 0..n {
        let mut carry = 0u128;
        for limb in &mut big {
            let p = u128::from(*limb) * u128::from(m) + carry;
            *limb = p as u64;
            carry = p >> 64;
        }
        if carry != 0 {
            big.push(carry as u64);
        }
    }
    let top = *big.last().expect("non-empty");
    let bit_len = (big.len() as i64 - 1) * 64 + i64::from(64 - top.leading_zeros());
    let lead = bit_len - 1 + e * n64; // exponent of the leading bit of the result
    if lead > 1023 {
        return signed(f64::INFINITY);
    }
    // Keep 53 bits (normal) or as many as the subnormal range allows; the rest rounds.
    let kept = if lead >= -1022 { 53 } else { lead + 1075 };
    if kept <= 0 {
        // Below half the smallest subnormal (or exactly half: ties to even gives 0)...
        // except when strictly above half: kept == 0 means the leading bit is worth 2^-1075.
        let above_half = kept == 0 && !is_power_of_two(&big);
        return signed(if above_half { f64::from_bits(1) } else { 0.0 });
    }
    let shift = bit_len - kept; // bits dropped
    let mut q = if shift > 0 {
        shr(&big, shift as u64)
    } else {
        // Fewer than `kept` bits: exact.
        big[0] << (-shift)
    };
    if shift > 0 {
        let half = bit(&big, (shift - 1) as u64);
        let sticky = any_below(&big, (shift - 1) as u64);
        if half && (sticky || q & 1 == 1) {
            q += 1;
        }
    }
    // The result is q · 2^(shift + e·n), with q < 2^53 (or = 2^53 after a carry). When no bits
    // were dropped, q = M << -shift, so the same scale holds.
    let scale = shift + e * n64;
    let value = if lead >= -1022 {
        // Normal: q in [2^52, 2^53].
        let (q, scale) = if q == 1u64 << 53 {
            (q >> 1, scale + 1)
        } else {
            (q, scale)
        };
        let exp = scale + 52 + 1023;
        if exp >= 2047 {
            f64::INFINITY
        } else {
            f64::from_bits(((exp as u64) << 52) | (q & ((1u64 << 52) - 1)))
        }
    } else {
        // Subnormal: the last kept bit is worth 2^-1074, so q is the bit pattern (a carry
        // into 2^52 gives the smallest normal number, also correct).
        f64::from_bits(q)
    };
    signed(value)
}

fn libm_like_nonfinite(x: f64, n: u32) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x.is_sign_negative() && n % 2 == 1 {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    }
}

/// 2^k as an f64 (0 below the subnormal range, infinity above).
fn pow2(k: i64) -> f64 {
    if k > 1023 {
        f64::INFINITY
    } else if k >= -1022 {
        f64::from_bits(((k + 1023) as u64) << 52)
    } else if k >= -1074 {
        f64::from_bits(1u64 << (k + 1074))
    } else {
        0.0
    }
}

fn is_power_of_two(big: &[u64]) -> bool {
    big.iter().map(|l| l.count_ones()).sum::<u32>() == 1
}

fn bit(big: &[u64], i: u64) -> bool {
    let (w, b) = ((i / 64) as usize, i % 64);
    big.get(w).is_some_and(|l| (l >> b) & 1 == 1)
}

fn any_below(big: &[u64], i: u64) -> bool {
    let (w, b) = ((i / 64) as usize, i % 64);
    big[..w.min(big.len())].iter().any(|&l| l != 0)
        || (b > 0 && w < big.len() && big[w] & ((1u64 << b) - 1) != 0)
}

/// The low 64 bits of `big >> s` (the caller keeps at most 53 bits).
fn shr(big: &[u64], s: u64) -> u64 {
    let (w, b) = ((s / 64) as usize, s % 64);
    let lo = big.get(w).copied().unwrap_or(0);
    if b == 0 {
        lo
    } else {
        let hi = big.get(w + 1).copied().unwrap_or(0);
        (lo >> b) | (hi << (64 - b))
    }
}

#[cfg(test)]
mod tests {
    use super::powi;

    /// Exact powers of small binary fractions can be checked against exact products.
    #[test]
    fn exact_small_cases() {
        assert_eq!(powi(3.0, 5), 243.0);
        assert_eq!(powi(-3.0, 5), -243.0);
        assert_eq!(powi(-3.0, 4), 81.0);
        assert_eq!(powi(0.5, 1074), f64::from_bits(1));
        assert_eq!(powi(0.5, 1075), 0.0);
        assert_eq!(powi(2.0, 1024), f64::INFINITY);
        assert_eq!(powi(1.5, 2), 2.25);
        assert_eq!(powi(-0.0, 3).to_bits(), (-0.0f64).to_bits());
        assert_eq!(powi(0.0, 0), 1.0);
        assert_eq!(powi(7.0, 0), 1.0);
        // 1.1^2 = 1.2100000000000002 (the exact square rounds up).
        assert_eq!(powi(1.1, 2), 1.1 * 1.1);
    }

    /// For n = 2 the correctly rounded square equals the IEEE product.
    #[test]
    fn squares_match_ieee_products() {
        let mut s = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..100_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let x = f64::from_bits((s >> 2) | 0x3000_0000_0000_0000) * 0.37;
            assert_eq!(powi(x, 2).to_bits(), (x * x).to_bits(), "{x:e}");
            let y = f64::from_bits(s % 0x0010_0000_0000_0000); // subnormal inputs
            assert_eq!(powi(y, 1).to_bits(), y.to_bits());
        }
    }

    /// The cubic B-spline pole's powers, against values from exact rational arithmetic
    /// (Python `fractions.Fraction`, rounded once), including subnormal results.
    #[test]
    fn cubic_pole_powers() {
        let z = -0.267_949_192_431_122_7_f64;
        for (n, bits) in [
            (1u32, 0xbfd1_2614_5e9e_cd56u64),
            (2, 0x3fb2_6145_e9ec_d563),
            (35, 0xbbc6_a507_7ec3_0312),
            (100, 0x3410_08ea_baa5_094b),
            (300, 0x1c50_1acf_1b3e_dd51),
            (500, 0x0490_2cc7_72c1_6358),
            (560, 0x0000_0000_0000_040d),
            (566, 0),
        ] {
            assert_eq!(powi(z, n).to_bits(), bits, "n = {n}");
        }
    }
}
