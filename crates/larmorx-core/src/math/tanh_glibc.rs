// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-fdlibm
//! The hyperbolic tangent for binary64 with the bits of glibc 2.35's `tanh` on x86-64:
//! fdlibm's `tanh` over fdlibm's `expm1`, compiled without fused multiply-adds.
//!
//! FSL's `bet2` takes its smoothness weight from the C library's `tanh` (`specs/bet.md` §4.4,
//! §8.1); the reruns of a self-intersecting surface amplify its last bit (§4.5). glibc's `tanh`
//! is not correctly rounded (GLIBC-06); [`super::tanh`] is. This one serves `reproduce` where an
//! original program's result depends on glibc's last bit.
//!
//! Port of fdlibm 5.3's `s_tanh.c` (`@(#)s_tanh.c 1.3 95/01/18`) and `s_expm1.c`
//! (`@(#)s_expm1.c 1.5 04/04/22`) as netlib distributes them (<https://www.netlib.org/fdlibm/>),
//! under Sun's notice reproduced below; written from those files only (never from a C library's
//! own source), with one change found by black-box testing: `expm1`'s polynomial in the
//! pipelined order of Naohiko Shimizu's 1997 revision, which `specs/bet.md` §8.1 names (see
//! [`poly`]). Every operation is one IEEE operation rounded to nearest, in that order (plain
//! `*`, `+`, `/`; never `mul_add`), so the bits are the same on every platform. Proved equal to
//! the system's `tanh` (glibc 2.35, x86-64) as a black box on a large random and edge sample
//! (`validation/investigations/cleanroom-bet/tanh-verify/`).
//!
//! The changes from the C: the high and low words are read with `to_bits`; fdlibm's `__HI(y) +=
//! k << 20` (adding `k` to the exponent field) is the same integer addition on the bits; the
//! exception flags (`huge*huge`, `tiny`) are not modelled, their values are.

// ====================================================
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//
// Developed at SunSoft, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.
// ====================================================
//
// ====================================================
// Copyright (C) 2004 by Sun Microsystems, Inc. All rights reserved.
//
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.
// ====================================================

#![allow(clippy::unreadable_literal, clippy::many_single_char_names)]

const HUGE: f64 = 1.0e+300;
const TINY: f64 = 1.0e-300;
/// `0x40862E42 FEFA39EF`
const O_THRESHOLD: f64 = f64::from_bits(0x40862E42_FEFA39EF);
/// `0x3fe62e42 fee00000`
const LN2_HI: f64 = f64::from_bits(0x3fe62e42_fee00000);
/// `0x3dea39ef 35793c76`
const LN2_LO: f64 = f64::from_bits(0x3dea39ef_35793c76);
/// `0x3ff71547 652b82fe`
const INVLN2: f64 = f64::from_bits(0x3ff71547_652b82fe);
/// The scaled coefficients of `expm1`'s rational approximation.
const Q1: f64 = f64::from_bits(0xBFA11111_111110F4);
const Q2: f64 = f64::from_bits(0x3F5A01A0_19FE5585);
const Q3: f64 = f64::from_bits(0xBF14CE19_9EAADBB7);
const Q4: f64 = f64::from_bits(0x3ED0CFCA_86E65239);
const Q5: f64 = f64::from_bits(0xBE8AFDB7_6E09C32D);

#[inline]
fn hi_word(x: f64) -> u32 {
    (x.to_bits() >> 32) as u32
}

#[inline]
fn lo_word(x: f64) -> u32 {
    x.to_bits() as u32
}

/// `x` with `k` added to its exponent field (fdlibm's `__HI(y) += (k << 20)`).
#[inline]
fn add_to_exponent(y: f64, k: i32) -> f64 {
    let hi = hi_word(y).wrapping_add((k << 20) as u32);
    f64::from_bits((u64::from(hi) << 32) | u64::from(lo_word(y)))
}

/// `x` with its high word replaced (fdlibm's `__HI(t) = w`, the low word kept).
#[inline]
fn with_hi_word(t: f64, w: u32) -> f64 {
    f64::from_bits((u64::from(w) << 32) | u64::from(lo_word(t)))
}

/// The polynomial `1 + Q1 z + ... + Q5 z^5` of `expm1`'s rational approximation, in the
/// pipelined order of Shimizu's 1997 revision (the spec's §8.1): Estrin's scheme, the three
/// pairs `1 + Q1 z`, `Q2 + Q3 z`, `Q4 + Q5 z` combined with `z^2` and `z^4` left to right.
/// fdlibm 5.3's Horner form (`1 + z(Q1 + z(Q2 + ...)))`) differs from glibc in the last bit
/// or two for about 0.03 % of arguments; this order agrees on every input tested
/// (`validation/investigations/cleanroom-bet/tanh-verify/`: the three orders tried there).
#[inline]
fn poly(hxs: f64) -> f64 {
    let r1 = 1.0 + hxs * Q1;
    let h2 = hxs * hxs;
    let r2 = Q2 + hxs * Q3;
    let h4 = h2 * h2;
    let r3 = Q4 + hxs * Q5;
    (r1 + h2 * r2) + h4 * r3
}

/// fdlibm's `expm1(x) = exp(x) - 1`.
#[allow(clippy::too_many_lines)]
fn expm1_fdlibm(x: f64) -> f64 {
    let mut x = x;
    let mut hx = hi_word(x);
    let xsb = hx & 0x8000_0000;
    hx &= 0x7fff_ffff;

    // The huge and non-finite arguments.
    if hx >= 0x4043687A {
        // |x| >= 56 ln2
        if hx >= 0x40862E42 {
            // |x| >= 709.78...
            if hx >= 0x7ff0_0000 {
                if ((hx & 0xfffff) | lo_word(x)) != 0 {
                    return x + x; // NaN
                }
                return if xsb == 0 { x } else { -1.0 }; // exp(±inf) = {inf, -1}
            }
            if x > O_THRESHOLD {
                return HUGE * HUGE; // overflow
            }
        }
        if xsb != 0 && x + TINY < 0.0 {
            // x < -56 ln2: -1 (inexact)
            return TINY - 1.0;
        }
    }

    // The argument reduction.
    let k: i32;
    let mut c = 0.0;
    if hx > 0x3fd62e42 {
        // |x| > 0.5 ln2
        let (hi, lo);
        if hx < 0x3FF0A2B2 {
            // and |x| < 1.5 ln2
            if xsb == 0 {
                hi = x - LN2_HI;
                lo = LN2_LO;
                k = 1;
            } else {
                hi = x + LN2_HI;
                lo = -LN2_LO;
                k = -1;
            }
        } else {
            // C's conversion of a double to int truncates toward zero; the value is in range.
            #[allow(clippy::cast_possible_truncation)]
            {
                k = (INVLN2 * x + if xsb == 0 { 0.5 } else { -0.5 }) as i32;
            }
            let t = f64::from(k);
            hi = x - t * LN2_HI; // t*ln2_hi is exact here
            lo = t * LN2_LO;
        }
        x = hi - lo;
        c = (hi - x) - lo;
    } else if hx < 0x3c90_0000 {
        // |x| < 2^-54: x (inexact when x != 0)
        let t = HUGE + x;
        return x - (t - (HUGE + x));
    } else {
        k = 0;
    }

    // x is now in the primary range.
    let hfx = 0.5 * x;
    let hxs = x * hfx;
    let r1 = poly(hxs);
    let t = 3.0 - r1 * hfx;
    let mut e = hxs * ((r1 - t) / (6.0 - x * t));
    if k == 0 {
        return x - (x * e - hxs); // c is 0
    }
    e = x * (e - c) - c;
    e -= hxs;
    if k == -1 {
        return 0.5 * (x - e) - 0.5;
    }
    if k == 1 {
        if x < -0.25 {
            return -2.0 * (e - (x + 0.5));
        }
        return 1.0 + 2.0 * (x - e);
    }
    if k <= -2 || k > 56 {
        // exp(x) - 1 suffices
        let y = 1.0 - (e - x);
        return add_to_exponent(y, k) - 1.0;
    }
    if k < 20 {
        let t = with_hi_word(1.0, 0x3ff0_0000 - (0x0020_0000 >> k)); // 1 - 2^-k
        let y = t - (e - x);
        add_to_exponent(y, k)
    } else {
        #[allow(clippy::cast_sign_loss)]
        let t = with_hi_word(1.0, ((0x3ff - k) << 20) as u32); // 2^-k
        let mut y = x - (e + t);
        y += 1.0;
        add_to_exponent(y, k)
    }
}

/// `tanh(x)` for `f64` with glibc 2.35's bits (x86-64), not correctly rounded (GLIBC-06):
/// fdlibm's method (`|x| < 2^-55`: `x·(1 + x)`; below 1: `−t/(t + 2)` with `t = expm1(−2|x|)`;
/// from 1 to 22: `1 − 2/(t + 2)` with `t = expm1(2|x|)`; beyond: ±1).
#[must_use]
pub fn tanh_glibc(x: f64) -> f64 {
    let jx = hi_word(x);
    let ix = jx & 0x7fff_ffff;
    let negative = jx & 0x8000_0000 != 0;

    // x is ±inf or NaN.
    if ix >= 0x7ff0_0000 {
        return if negative { 1.0 / x - 1.0 } else { 1.0 / x + 1.0 };
    }

    let z = if ix < 0x4036_0000 {
        // |x| < 22
        if ix < 0x3c80_0000 {
            // |x| < 2^-55 (±0 included: x·(1 + x) keeps the sign)
            return x * (1.0 + x);
        }
        if ix >= 0x3ff0_0000 {
            // |x| >= 1
            let t = expm1_fdlibm(2.0 * x.abs());
            1.0 - 2.0 / (t + 2.0)
        } else {
            let t = expm1_fdlibm(-2.0 * x.abs());
            -t / (t + 2.0)
        }
    } else {
        // |x| >= 22: ±1 (inexact)
        1.0 - TINY
    };
    if negative { -z } else { z }
}

#[cfg(test)]
mod tests {
    use super::{expm1_fdlibm, tanh_glibc};

    #[test]
    fn special_values() {
        assert_eq!(tanh_glibc(0.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(tanh_glibc(-0.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(tanh_glibc(f64::INFINITY), 1.0);
        assert_eq!(tanh_glibc(f64::NEG_INFINITY), -1.0);
        assert!(tanh_glibc(f64::NAN).is_nan());
        assert_eq!(tanh_glibc(22.0), 1.0);
        assert_eq!(tanh_glibc(-30.0), -1.0);
        assert_eq!(tanh_glibc(1e-300), 1e-300);
        assert_eq!(expm1_fdlibm(0.0), 0.0);
        assert_eq!(expm1_fdlibm(f64::NEG_INFINITY), -1.0);
        assert_eq!(expm1_fdlibm(-100.0), -1.0);
    }

    /// Values of the system's `tanh` (glibc 2.35, x86-64), recorded as bits by the black-box
    /// verifier: arguments from each branch of the method, BET's smoothness arguments among them.
    #[test]
    fn matches_glibc_on_recorded_values() {
        for &(x, y) in RECORDED {
            let got = tanh_glibc(f64::from_bits(x));
            assert_eq!(got.to_bits(), y, "tanh({:e})", f64::from_bits(x));
        }
    }

    /// SplitMix64 (the verifier's).
    fn splitmix(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The verifier's input of family `f` at index `i`.
    #[allow(clippy::cast_precision_loss)]
    fn input(f: u32, i: u64) -> f64 {
        let mut s = i ^ (u64::from(f) << 56) ^ 0x5DEE_CE66_D1CE_4E5B;
        let r = splitmix(&mut s);
        match f {
            0 => {
                let e = 0x3ff - 80 + (r >> 52) % 86;
                let m = r & 0x000f_ffff_ffff_ffff;
                let sign = (splitmix(&mut s) & 1) << 63;
                f64::from_bits(sign | (e << 52) | m)
            }
            1 => ((r >> 11) as f64 / (1u64 << 53) as f64) * 50.0 - 25.0,
            _ => ((r >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0,
        }
    }

    /// FNV-1a over the results of 2^20 inputs of the verifier's three families: the system's
    /// `tanh` (glibc 2.35, x86-64) gives `1e2cf3306439ce3c` (`tanh-glibc-verify record`).
    #[test]
    fn matches_glibc_on_a_sequence() {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for i in 0..(1u64 << 20) {
            #[allow(clippy::cast_possible_truncation)]
            let x = input((i % 3) as u32, i);
            h ^= tanh_glibc(x).to_bits();
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        assert_eq!(h, 0x1e2c_f330_6439_ce3c);
    }

    /// `(x, glibc's tanh(x))` as bits; the last eight are arguments where fdlibm 5.3's Horner
    /// polynomial gives another last bit.
    const RECORDED: &[(u64, u64)] = &[
        (0x3bc79ca10c924223, 0x3bc79ca10c924223),
        (0xbc814b37f4b51f71, 0xbc814b37f4b51f71),
        (0x3e112e0be826d695, 0x3e112e0be826d695),
        (0x3f847ae147ae147b, 0x3f847ab48ae4595e),
        (0xbfb999999999999a, 0xbfb983d7795f413a),
        (0x3fc5c28f5c28f5c3, 0x3fc58d8296a405bc),
        (0x3fd62eb1c432ca58, 0x3fd555b7cb87b1cb),
        (0x3fe0a3d70a3d70a4, 0x3fde92a312640003),
        (0xbfe6666666666666, 0xbfe356fb17af2e92),
        (0x3fefae147ae147ae, 0x3fe83c4fe9b770e0),
        (0x3ff0000000000000, 0x3fe85efab514f394),
        (0x3ff8000000000000, 0x3fecf6f9786df577),
        (0xc002000000000000, 0xbfef4bfd6c2dfd1e),
        (0x4008000000000000, 0x3fefd77d111a0b00),
        (0x4016000000000000, 0x3fefffb9f2fc1e91),
        (0x401b99999999999a, 0x3feffffbbd79a16d),
        (0x401c666666666666, 0x3feffffd24ffc2dc),
        (0x4028000000000000, 0x3feffffffff59f7c),
        (0xc033666666666666, 0xbff0000000000000),
        (0x4035fd70a3d70a3d, 0x3ff0000000000000),
        (0xc023df33c7d8a77a, 0xbfeffffffd7bfe4e),
        (0xc028ef978179623c, 0xbfeffffffffbee09),
        (0xc03347f6057caffe, 0xbff0000000000000),
        (0xc01376cb536de440, 0xbfefff0712c175f3),
        (0x402996405a813120, 0x3feffffffffde099),
        (0x40159da9b6147864, 0x3fefffab1dc48d70),
        (0x3fcac5aadb14c800, 0x3fca6371a064c758),
        (0x4027d54497e64114, 0x3feffffffff3bce6),
        (0x3ff374fc03df1bb0, 0x3fead4e566b69a90),
        (0x40323d54a5d1fb14, 0x3feffffffffffffd),
        (0xc00f28852b288350, 0xbfeff93825efafe7),
        (0x3ffd9c12399abb40, 0x3fee752b07aa862a),
        (0xbfca9f38541f3d81, 0xbfca3e9f26309826),
        (0xbfc89583536827b3, 0xbfc84940b55aa42a),
        (0x3fc5694ed0851fd7, 0x3fc536bfb7a35fd8),
        (0x3fdebaf73640d2b1, 0x3fdc915fbda92a96),
        (0x3fb6f90c459dd15c, 0x3fb6e94fedf1182e),
        (0xbfd0a565498a253e, 0xbfd047d5413ab364),
        (0x3fcd2897e6e456db, 0x3fccaa18e944ff14),
        (0xbfc6cb7fb3404533, 0xbfc68e94ee9c8985),
    ];
}
