// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded base-10 logarithm of binary64 values: [`log10`].
//!
//! `log10(x)` is the binary64 value nearest to the exact log10 x (round-to-nearest,
//! ties-to-even), so the result is the same on every platform and equals any other correctly
//! rounded `log10`.
//!
//! Port of CORE-MATH's `src/binary64/log10/log10.c` (`cr_log10`) and `dint.h` at commit
//! 040ee482a8ca (MIT licence, © the CORE-MATH authors named below), re-expressed in Rust for
//! round-to-nearest only. The algorithm and the order of every floating-point operation are
//! CORE-MATH's:
//!
//! 1. The powers of ten `10^n`, `0 <= n <= 22` (the exact doubles), return `n`.
//! 2. Fast path: a double-double approximation of log(x) (a 363-entry table of 10-bit inverses
//!    with `x` normalised into [1/sqrt(2), sqrt(2)), a degree-6 polynomial), multiplied by a
//!    double-double 1/log(10); its absolute error is below 2^-68.98 (the C file's bound). If
//!    rounding `h + l ± err` gives the same double for both signs, that double is the correctly
//!    rounded result.
//! 3. Accurate path, when that test fails (about one input in 2^11.5): log(x) in 128-bit "dint"
//!    arithmetic (`super::dint`; the accurate logarithm, its tables `_INVERSE_2`, `_LOG_INV_2`,
//!    `P_2` and the conversion from a double are log.c's, so `super::log`'s are used),
//!    multiplied by a dint 1/log(10), and a final rounding that is correct for every binary64
//!    input.
//!
//! log10.c's fast-path tables differ from log.c's in their entries for `i = 511` and `i = 512`
//! (log.c forces them to `r = 1`, `-log(r) = 0`), and its normalisation and final rounding
//! (`dint_tod`, which adds the sticky 2^-54 only when trailing bits are set) are its own: they
//! are here. Differences from the C code, none of which changes a non-NaN result in
//! round-to-nearest: rounding-mode, `errno` and floating-point-exception handling are dropped;
//! `__builtin_fma` is `f64::mul_add` (a correctly rounded fused multiply-add on every target);
//! hex-float constants are written as their exact bit patterns; a NaN argument gives itself
//! quieted (sign and payload kept: what the C's `x + x` gives on x86-64 and AArch64) and `x < 0`
//! (including `-inf`) gives `f64::NAN` (`0x7ff8...`), where the C's `0.0 / 0.0` returns the
//! target's default NaN (whose sign bit differs between x86-64 and AArch64); these NaNs are also
//! glibc 2.35's (`super::log10_glibc`).
//
// Original copyright and licence (MIT) notices. log10.c:
//
//   Copyright (c) 2022-2025 INRIA and CERN.
//   Authors: Paul Zimmermann and Tom Hubrecht.
//
// dint.h (whose accurate-path tables are log.c's, in `super::log`, and whose `ONE_OVER_LOG10`
// is below):
//
//   Copyright (c) 2022 CERN.
//   Author: Tom Hubrecht
//
// Both files:
//
//   This file is part of the CORE-MATH project
//   (https://core-math.gitlabpages.inria.fr/).
//
//   Permission is hereby granted, free of charge, to any person obtaining a copy
//   of this software and associated documentation files (the "Software"), to deal
//   in the Software without restriction, including without limitation the rights
//   to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//   copies of the Software, and to permit persons to whom the Software is
//   furnished to do so, subject to the following conditions:
//
//   The above copyright notice and this permission notice shall be included in all
//   copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//   AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//   OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//   SOFTWARE.

use super::dint::{Dint64, mul_dint};
use super::log::{P, dint_fromd, f64_pair_table, f64_table, fast_two_sum, log_2};

/// Base-10 logarithm of `x`, correctly rounded to nearest (CORE-MATH's `cr_log10`).
///
/// Special values: `log10(±0) = -∞`, `log10(+∞) = +∞`, `log10(10^n) = n` for the exact
/// powers `0 <= n <= 22`, `log10(x)` is `f64::NAN` for `x < 0` (including `-∞`), and a NaN
/// argument gives itself quieted.
pub fn log10(x: f64) -> f64 {
    let mut v = x.to_bits();
    // C: `int e = (v.u >> 52) - 0x3ff;`
    let mut e: i32 = (v >> 52) as i32 - 0x3ff;
    if e >= 0x400 || e == -0x3ff {
        // x <= 0 or NaN/Inf or subnormal
        const MINF: u64 = 0xfff << 52;
        if e == 0x400 || (e == 0xc00 && x != f64::from_bits(MINF)) {
            // +Inf or NaN. C: `x + x`, which is +Inf for +Inf and the NaN quieted.
            return f64::from_bits(if x.is_nan() {
                v | 0x0008_0000_0000_0000
            } else {
                v
            });
        }
        if x <= 0.0 {
            // log10(x<0) is NaN, f(+/-0) is -Inf and raises DivByZero
            if x < 0.0 {
                // C: `0.0 / 0.0`
                return f64::NAN;
            }
            // C: `1.0 / -0.0`
            return f64::NEG_INFINITY;
        }
        // subnormal
        v = (f64::from_bits(v) * TWO_POW_52).to_bits();
        e = (v >> 52) as i32 - 0x3ff - 52;
    }

    // For x = 10^n for 0 <= n <= 22 we should return n without a spurious inexact exception.
    // C: `unsigned int n = 0x267cfa2u*((unsigned int)e+1u)>>27;` (32-bit unsigned, wrapping)
    let n = 0x267_cfa2u32.wrapping_mul((e as u32).wrapping_add(1)) >> 27;
    if v == POW10[n as usize] {
        return f64::from(n);
    }

    // now x > 0
    let (h, l) = cr_log10_fast(e, v);

    // Maximal absolute error from cr_log10_fast: 2^-68.98 < 1.04p-69. This bound is
    // relatively tight, since for 0x1.59p-70 it fails for x=0x1.8301ae420f027p+864 (rndz).
    const ERR: f64 = f64::from_bits(0x3ba0_4000_0000_0000); // 0x1.04p-69

    let left = h + (l - ERR);
    let right = h + (l + ERR);
    if left == right {
        return left;
    }
    // the probability of failure of the fast path is about 2^-11.5
    cr_log10_accurate(x)
}

/// 2^52 (C: `0x1p52`).
const TWO_POW_52: f64 = f64::from_bits(0x4330_0000_0000_0000);

/// `pow10`, as the bits the C compares with (`pow10u`): 10^n for `0 <= n <= 22`, then nine
/// zeros (no positive argument has those bits).
const POW10: [u64; 32] = [
    0x3ff0000000000000, // 0x1p+0
    0x4024000000000000, // 0x1.4p+3
    0x4059000000000000, // 0x1.9p+6
    0x408f400000000000, // 0x1.f4p+9
    0x40c3880000000000, // 0x1.388p+13
    0x40f86a0000000000, // 0x1.86ap+16
    0x412e848000000000, // 0x1.e848p+19
    0x416312d000000000, // 0x1.312dp+23
    0x4197d78400000000, // 0x1.7d784p+26
    0x41cdcd6500000000, // 0x1.dcd65p+29
    0x4202a05f20000000, // 0x1.2a05f2p+33
    0x42374876e8000000, // 0x1.74876e8p+36
    0x426d1a94a2000000, // 0x1.d1a94a2p+39
    0x42a2309ce5400000, // 0x1.2309ce54p+43
    0x42d6bcc41e900000, // 0x1.6bcc41e9p+46
    0x430c6bf526340000, // 0x1.c6bf52634p+49
    0x4341c37937e08000, // 0x1.1c37937e08p+53
    0x4376345785d8a000, // 0x1.6345785d8ap+56
    0x43abc16d674ec800, // 0x1.bc16d674ec8p+59
    0x43e158e460913d00, // 0x1.158e460913dp+63
    0x4415af1d78b58c40, // 0x1.5af1d78b58c4p+66
    0x444b1ae4d6e2ef50, // 0x1.b1ae4d6e2ef5p+69
    0x4480f0cf064dd592, // 0x1.0f0cf064dd592p+73
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
];

/// `a_mul`: `(hi, lo)` with `hi + lo = a * b` exactly.
#[inline(always)]
fn a_mul(a: f64, b: f64) -> (f64, f64) {
    let hi = a * b;
    (hi, a.mul_add(b, -hi))
}

/// `d_mul`: `(ah + al) * (bh + bl) - (al * bl)` as a double-double (the term `al * bl` is
/// negligible when `al <= ulp(ah)` and `bl <= ulp(bh)`).
#[inline(always)]
fn d_mul(ah: f64, al: f64, bh: f64, bl: f64) -> (f64, f64) {
    let (hi, s) = a_mul(ah, bh); // exact
    let t = al.mul_add(bh, s);
    (hi, ah.mul_add(bl, t))
}

/// `cr_log10_fast`: given the bits `v` of a double `2^e*x` (`x` in [1, 2), `v`'s exponent field
/// ignored), a double-double `(h, l)` approximating log10(2^e*x), with absolute error bounded
/// by 2^-68.98 (the C file details the bound of every operation below).
#[inline(always)]
fn cr_log10_fast(e: i32, v: u64) -> (f64, f64) {
    // log(2) = LOG2_H + LOG2_L with |log(2) - (h+l)| < 2^-102.01; LOG2_H is exactly
    // representable on 42 bits, so that e*LOG2_H is exact.
    const LOG2_H: f64 = f64::from_bits(0x3fe6_2e42_fefa_3800); // 0x1.62e42fefa38p-1
    const LOG2_L: f64 = f64::from_bits(0x3d2e_f357_93c7_6730); // 0x1.ef35793c7673p-45
    // |1/log(10) - (ONE_OVER_LOG10_H + ONE_OVER_LOG10_L)| < 2^-111.05
    const ONE_OVER_LOG10_H: f64 = f64::from_bits(0x3fdb_cb7b_1526_e50e); // 0x1.bcb7b1526e50ep-2
    const ONE_OVER_LOG10_L: f64 = f64::from_bits(0x3c69_5355_baaa_fad3); // 0x1.95355baaafad3p-57
    const OFFSET: usize = 362;

    let m = v & 0xf_ffff_ffff_ffff;
    // x = m/2^52; if x > sqrt(2), we divide it by 2 to avoid cancellation
    let c = u64::from(m >= 0x6_a09e_667f_3bcd);
    // normalize v in [1/sqrt(2), sqrt(2))
    let y = f64::from_bits(((0x3ff - c) << 52) + m);
    let e = e + c as i32; // now -1074 <= e <= 1024
    let em = 0x10_0000_0000_0000 + m;
    let i = (em >> (43 + c)) as usize;
    let r = INVERSE[i - OFFSET];
    let l1 = LOG_INV[i - OFFSET][0];
    let l2 = LOG_INV[i - OFFSET][1];
    let z = r.mul_add(y, -1.0); // exact
    // evaluate P(z), for |z| < 0.00212097167968735
    let z2 = z * z; // rounding error bounded by 2^-70
    let p45 = P[5].mul_add(z, P[4]);
    let p23 = P[3].mul_add(z, P[2]);
    let mut ph = p45.mul_add(z2, p23);
    ph = ph.mul_add(z, P[1]);
    ph *= z2; // P(z)-z

    // Add e*log(2) to (h,l), where -1074 <= e <= 1023, thus e has at most 11 bits; hh+l1 is an
    // integer multiple of 2^-42 with 2^42*|hh+l1| < 2^52, thus exact.
    let ee = f64::from(e);
    let (h, mut l) = fast_two_sum(ee.mul_add(LOG2_H, l1), z);
    // add ph + l2 to l
    l = ph + (l + l2);
    l = ee.mul_add(LOG2_L, l);
    // absolute error on h + l < 2^-68.22; divide h+l by log(10), multiplying by an
    // approximation of 1/log(10): total absolute error < 2^-68.98
    d_mul(h, l, ONE_OVER_LOG10_H, ONE_OVER_LOG10_L)
}

/// `ONE_OVER_LOG10`: 1/log(10) as a dint, with absolute error less than 2^-131.72.
const ONE_OVER_LOG10: Dint64 = Dint64::new(0xde5bd8a937287195, 0x355baaafad33dc32, -2, 0x0);

/// `cr_log10_accurate`: the accurate path, Tom Hubrecht's accurate logarithm (from CORE-MATH's
/// correctly rounded `pow`, as log.c's) multiplied by 1/log(10).
#[cold]
fn cr_log10_accurate(x: f64) -> f64 {
    let xd = dint_fromd(x);
    // x = (-1)^sgn*2^ex*(hi/2^63+lo/2^127)
    let y = log_2(xd);
    // multiply by 1/log(10). C: `mul_dint (&Y, &ONE_OVER_LOG10, &Y)`
    let y = mul_dint(&ONE_OVER_LOG10, &y);
    dint_tod(&y)
}

/// log10.c's `dint_tod`: a dint as a double, rounded to nearest, assuming the input is not in
/// the subnormal range (the round bit adds 2^-53, any later bit 2^-54).
#[inline(always)]
fn dint_tod(a: &Dint64) -> f64 {
    const TWO_POW_M54: f64 = f64::from_bits(0x3c90_0000_0000_0000); // 0x1p-54
    const TWO_POW_M53: f64 = f64::from_bits(0x3ca0_0000_0000_0000); // 0x1p-53

    let mut r = (a.hi >> 11) | (0x3ff << 52);
    // r contains the upper 53 bits of a->hi, 1 <= r < 2

    let mut rd = 0.0;
    // if round bit is 1, add 2^-53
    if (a.hi >> 10) & 0x1 != 0 {
        rd += TWO_POW_M53;
    }
    // if trailing bits after the rounding bit are non zero, add 2^-54
    if a.hi & 0x3ff != 0 || a.lo != 0 {
        rd += TWO_POW_M54;
    }

    r |= a.sgn << 63;
    let mut rf = f64::from_bits(r);
    rf += if a.sgn == 0 { rd } else { -rd };

    // For log, the result is always in the normal range, thus a->ex > -1023. Similarly, we
    // cannot have a->ex > 1023.
    let e = (((a.ex + 1023) & 0x7ff) as u64) << 52;

    rf * f64::from_bits(e)
}

// ------------------------------------------------------------------------------------------------
// Tables of log10.c, transcribed mechanically (`validation/probes/coremath-log10/
// coremath_tables.py`). Each entry is the exact bit pattern of the C hex-float literal in its
// comment.

/// `_INVERSE`: for `362 <= i <= 724`, `INVERSE[i-362]` is a 10-bit approximation of `1/x[i]`,
/// where `i*2^-9 <= x[i] < (i+1)*2^-9`. More precisely `r[i]` is a 10-bit value such that
/// `r[i]*y-1` is representable exactly on 53 bits for any y, `i*2^-9 <= y < (i+1)*2^-9`.
/// Moreover `|r[i]*y-1| <= 0.00212097167968735`.
static INVERSE: [f64; 363] = f64_table([
    0x3ff6980000000000, // 0x1.698p+0
    0x3ff6880000000000, // 0x1.688p+0
    0x3ff6780000000000, // 0x1.678p+0
    0x3ff6680000000000, // 0x1.668p+0
    0x3ff6580000000000, // 0x1.658p+0
    0x3ff6480000000000, // 0x1.648p+0
    0x3ff6380000000000, // 0x1.638p+0
    0x3ff6300000000000, // 0x1.63p+0
    0x3ff6200000000000, // 0x1.62p+0
    0x3ff6100000000000, // 0x1.61p+0
    0x3ff6000000000000, // 0x1.6p+0
    0x3ff5f00000000000, // 0x1.5fp+0
    0x3ff5e00000000000, // 0x1.5ep+0
    0x3ff5d00000000000, // 0x1.5dp+0
    0x3ff5c00000000000, // 0x1.5cp+0
    0x3ff5b00000000000, // 0x1.5bp+0
    0x3ff5a80000000000, // 0x1.5a8p+0
    0x3ff5980000000000, // 0x1.598p+0
    0x3ff5880000000000, // 0x1.588p+0
    0x3ff5780000000000, // 0x1.578p+0
    0x3ff5680000000000, // 0x1.568p+0
    0x3ff5600000000000, // 0x1.56p+0
    0x3ff5500000000000, // 0x1.55p+0
    0x3ff5400000000000, // 0x1.54p+0
    0x3ff5300000000000, // 0x1.53p+0
    0x3ff5200000000000, // 0x1.52p+0
    0x3ff5180000000000, // 0x1.518p+0
    0x3ff5080000000000, // 0x1.508p+0
    0x3ff4f80000000000, // 0x1.4f8p+0
    0x3ff4f00000000000, // 0x1.4fp+0
    0x3ff4e00000000000, // 0x1.4ep+0
    0x3ff4d00000000000, // 0x1.4dp+0
    0x3ff4c00000000000, // 0x1.4cp+0
    0x3ff4b80000000000, // 0x1.4b8p+0
    0x3ff4a80000000000, // 0x1.4a8p+0
    0x3ff4a00000000000, // 0x1.4ap+0
    0x3ff4900000000000, // 0x1.49p+0
    0x3ff4800000000000, // 0x1.48p+0
    0x3ff4780000000000, // 0x1.478p+0
    0x3ff4680000000000, // 0x1.468p+0
    0x3ff4580000000000, // 0x1.458p+0
    0x3ff4500000000000, // 0x1.45p+0
    0x3ff4400000000000, // 0x1.44p+0
    0x3ff4300000000000, // 0x1.43p+0
    0x3ff4280000000000, // 0x1.428p+0
    0x3ff4180000000000, // 0x1.418p+0
    0x3ff4100000000000, // 0x1.41p+0
    0x3ff4000000000000, // 0x1.4p+0
    0x3ff3f80000000000, // 0x1.3f8p+0
    0x3ff3e80000000000, // 0x1.3e8p+0
    0x3ff3e00000000000, // 0x1.3ep+0
    0x3ff3d00000000000, // 0x1.3dp+0
    0x3ff3c00000000000, // 0x1.3cp+0
    0x3ff3b80000000000, // 0x1.3b8p+0
    0x3ff3a80000000000, // 0x1.3a8p+0
    0x3ff3a00000000000, // 0x1.3ap+0
    0x3ff3900000000000, // 0x1.39p+0
    0x3ff3880000000000, // 0x1.388p+0
    0x3ff3780000000000, // 0x1.378p+0
    0x3ff3700000000000, // 0x1.37p+0
    0x3ff3600000000000, // 0x1.36p+0
    0x3ff3580000000000, // 0x1.358p+0
    0x3ff3500000000000, // 0x1.35p+0
    0x3ff3400000000000, // 0x1.34p+0
    0x3ff3380000000000, // 0x1.338p+0
    0x3ff3280000000000, // 0x1.328p+0
    0x3ff3200000000000, // 0x1.32p+0
    0x3ff3100000000000, // 0x1.31p+0
    0x3ff3080000000000, // 0x1.308p+0
    0x3ff3000000000000, // 0x1.3p+0
    0x3ff2f00000000000, // 0x1.2fp+0
    0x3ff2e80000000000, // 0x1.2e8p+0
    0x3ff2d80000000000, // 0x1.2d8p+0
    0x3ff2d00000000000, // 0x1.2dp+0
    0x3ff2c80000000000, // 0x1.2c8p+0
    0x3ff2b80000000000, // 0x1.2b8p+0
    0x3ff2b00000000000, // 0x1.2bp+0
    0x3ff2a00000000000, // 0x1.2ap+0
    0x3ff2980000000000, // 0x1.298p+0
    0x3ff2900000000000, // 0x1.29p+0
    0x3ff2800000000000, // 0x1.28p+0
    0x3ff2780000000000, // 0x1.278p+0
    0x3ff2700000000000, // 0x1.27p+0
    0x3ff2600000000000, // 0x1.26p+0
    0x3ff2580000000000, // 0x1.258p+0
    0x3ff2500000000000, // 0x1.25p+0
    0x3ff2400000000000, // 0x1.24p+0
    0x3ff2380000000000, // 0x1.238p+0
    0x3ff2300000000000, // 0x1.23p+0
    0x3ff2280000000000, // 0x1.228p+0
    0x3ff2180000000000, // 0x1.218p+0
    0x3ff2100000000000, // 0x1.21p+0
    0x3ff2080000000000, // 0x1.208p+0
    0x3ff2000000000000, // 0x1.2p+0
    0x3ff1f00000000000, // 0x1.1fp+0
    0x3ff1e80000000000, // 0x1.1e8p+0
    0x3ff1e00000000000, // 0x1.1ep+0
    0x3ff1d00000000000, // 0x1.1dp+0
    0x3ff1c80000000000, // 0x1.1c8p+0
    0x3ff1c00000000000, // 0x1.1cp+0
    0x3ff1b80000000000, // 0x1.1b8p+0
    0x3ff1b00000000000, // 0x1.1bp+0
    0x3ff1a00000000000, // 0x1.1ap+0
    0x3ff1980000000000, // 0x1.198p+0
    0x3ff1900000000000, // 0x1.19p+0
    0x3ff1880000000000, // 0x1.188p+0
    0x3ff1800000000000, // 0x1.18p+0
    0x3ff1700000000000, // 0x1.17p+0
    0x3ff1680000000000, // 0x1.168p+0
    0x3ff1600000000000, // 0x1.16p+0
    0x3ff1580000000000, // 0x1.158p+0
    0x3ff1500000000000, // 0x1.15p+0
    0x3ff1400000000000, // 0x1.14p+0
    0x3ff1380000000000, // 0x1.138p+0
    0x3ff1300000000000, // 0x1.13p+0
    0x3ff1280000000000, // 0x1.128p+0
    0x3ff1200000000000, // 0x1.12p+0
    0x3ff1180000000000, // 0x1.118p+0
    0x3ff1100000000000, // 0x1.11p+0
    0x3ff1000000000000, // 0x1.1p+0
    0x3ff0f80000000000, // 0x1.0f8p+0
    0x3ff0f00000000000, // 0x1.0fp+0
    0x3ff0e80000000000, // 0x1.0e8p+0
    0x3ff0e00000000000, // 0x1.0ep+0
    0x3ff0d80000000000, // 0x1.0d8p+0
    0x3ff0d00000000000, // 0x1.0dp+0
    0x3ff0c80000000000, // 0x1.0c8p+0
    0x3ff0c00000000000, // 0x1.0cp+0
    0x3ff0b00000000000, // 0x1.0bp+0
    0x3ff0a80000000000, // 0x1.0a8p+0
    0x3ff0a00000000000, // 0x1.0ap+0
    0x3ff0980000000000, // 0x1.098p+0
    0x3ff0900000000000, // 0x1.09p+0
    0x3ff0880000000000, // 0x1.088p+0
    0x3ff0800000000000, // 0x1.08p+0
    0x3ff0780000000000, // 0x1.078p+0
    0x3ff0700000000000, // 0x1.07p+0
    0x3ff0680000000000, // 0x1.068p+0
    0x3ff0600000000000, // 0x1.06p+0
    0x3ff0580000000000, // 0x1.058p+0
    0x3ff0500000000000, // 0x1.05p+0
    0x3ff0480000000000, // 0x1.048p+0
    0x3ff0400000000000, // 0x1.04p+0
    0x3ff0380000000000, // 0x1.038p+0
    0x3ff0300000000000, // 0x1.03p+0
    0x3ff0280000000000, // 0x1.028p+0
    0x3ff0200000000000, // 0x1.02p+0
    0x3ff0180000000000, // 0x1.018p+0
    0x3ff0100000000000, // 0x1.01p+0
    0x3ff0080000000000, // 0x1.008p+0
    0x3feff80000000000, // 0x1.ff8p-1
    0x3fefe80000000000, // 0x1.fe8p-1
    0x3fefd80000000000, // 0x1.fd8p-1
    0x3fefc80000000000, // 0x1.fc8p-1
    0x3fefb80000000000, // 0x1.fb8p-1
    0x3fefa80000000000, // 0x1.fa8p-1
    0x3fef980000000000, // 0x1.f98p-1
    0x3fef880000000000, // 0x1.f88p-1
    0x3fef780000000000, // 0x1.f78p-1
    0x3fef680000000000, // 0x1.f68p-1
    0x3fef580000000000, // 0x1.f58p-1
    0x3fef500000000000, // 0x1.f5p-1
    0x3fef400000000000, // 0x1.f4p-1
    0x3fef300000000000, // 0x1.f3p-1
    0x3fef200000000000, // 0x1.f2p-1
    0x3fef100000000000, // 0x1.f1p-1
    0x3fef000000000000, // 0x1.fp-1
    0x3feef00000000000, // 0x1.efp-1
    0x3feee00000000000, // 0x1.eep-1
    0x3feed00000000000, // 0x1.edp-1
    0x3feec80000000000, // 0x1.ec8p-1
    0x3feeb80000000000, // 0x1.eb8p-1
    0x3feea80000000000, // 0x1.ea8p-1
    0x3fee980000000000, // 0x1.e98p-1
    0x3fee880000000000, // 0x1.e88p-1
    0x3fee780000000000, // 0x1.e78p-1
    0x3fee700000000000, // 0x1.e7p-1
    0x3fee600000000000, // 0x1.e6p-1
    0x3fee500000000000, // 0x1.e5p-1
    0x3fee400000000000, // 0x1.e4p-1
    0x3fee300000000000, // 0x1.e3p-1
    0x3fee280000000000, // 0x1.e28p-1
    0x3fee180000000000, // 0x1.e18p-1
    0x3fee080000000000, // 0x1.e08p-1
    0x3fedf80000000000, // 0x1.df8p-1
    0x3fedf00000000000, // 0x1.dfp-1
    0x3fede00000000000, // 0x1.dep-1
    0x3fedd00000000000, // 0x1.ddp-1
    0x3fedc00000000000, // 0x1.dcp-1
    0x3fedb80000000000, // 0x1.db8p-1
    0x3feda80000000000, // 0x1.da8p-1
    0x3fed980000000000, // 0x1.d98p-1
    0x3fed900000000000, // 0x1.d9p-1
    0x3fed800000000000, // 0x1.d8p-1
    0x3fed700000000000, // 0x1.d7p-1
    0x3fed600000000000, // 0x1.d6p-1
    0x3fed580000000000, // 0x1.d58p-1
    0x3fed480000000000, // 0x1.d48p-1
    0x3fed380000000000, // 0x1.d38p-1
    0x3fed300000000000, // 0x1.d3p-1
    0x3fed200000000000, // 0x1.d2p-1
    0x3fed100000000000, // 0x1.d1p-1
    0x3fed080000000000, // 0x1.d08p-1
    0x3fecf80000000000, // 0x1.cf8p-1
    0x3fece80000000000, // 0x1.ce8p-1
    0x3fece00000000000, // 0x1.cep-1
    0x3fecd00000000000, // 0x1.cdp-1
    0x3fecc80000000000, // 0x1.cc8p-1
    0x3fecb80000000000, // 0x1.cb8p-1
    0x3feca80000000000, // 0x1.ca8p-1
    0x3feca00000000000, // 0x1.cap-1
    0x3fec900000000000, // 0x1.c9p-1
    0x3fec880000000000, // 0x1.c88p-1
    0x3fec780000000000, // 0x1.c78p-1
    0x3fec680000000000, // 0x1.c68p-1
    0x3fec600000000000, // 0x1.c6p-1
    0x3fec500000000000, // 0x1.c5p-1
    0x3fec480000000000, // 0x1.c48p-1
    0x3fec380000000000, // 0x1.c38p-1
    0x3fec300000000000, // 0x1.c3p-1
    0x3fec200000000000, // 0x1.c2p-1
    0x3fec180000000000, // 0x1.c18p-1
    0x3fec080000000000, // 0x1.c08p-1
    0x3febf80000000000, // 0x1.bf8p-1
    0x3febf00000000000, // 0x1.bfp-1
    0x3febe00000000000, // 0x1.bep-1
    0x3febd80000000000, // 0x1.bd8p-1
    0x3febc80000000000, // 0x1.bc8p-1
    0x3febc00000000000, // 0x1.bcp-1
    0x3febb00000000000, // 0x1.bbp-1
    0x3feba80000000000, // 0x1.ba8p-1
    0x3feb980000000000, // 0x1.b98p-1
    0x3feb900000000000, // 0x1.b9p-1
    0x3feb800000000000, // 0x1.b8p-1
    0x3feb780000000000, // 0x1.b78p-1
    0x3feb680000000000, // 0x1.b68p-1
    0x3feb600000000000, // 0x1.b6p-1
    0x3feb580000000000, // 0x1.b58p-1
    0x3feb480000000000, // 0x1.b48p-1
    0x3feb400000000000, // 0x1.b4p-1
    0x3feb300000000000, // 0x1.b3p-1
    0x3feb280000000000, // 0x1.b28p-1
    0x3feb180000000000, // 0x1.b18p-1
    0x3feb100000000000, // 0x1.b1p-1
    0x3feb000000000000, // 0x1.bp-1
    0x3feaf80000000000, // 0x1.af8p-1
    0x3feaf00000000000, // 0x1.afp-1
    0x3feae00000000000, // 0x1.aep-1
    0x3fead80000000000, // 0x1.ad8p-1
    0x3feac80000000000, // 0x1.ac8p-1
    0x3feac00000000000, // 0x1.acp-1
    0x3feab80000000000, // 0x1.ab8p-1
    0x3feaa80000000000, // 0x1.aa8p-1
    0x3feaa00000000000, // 0x1.aap-1
    0x3fea900000000000, // 0x1.a9p-1
    0x3fea880000000000, // 0x1.a88p-1
    0x3fea800000000000, // 0x1.a8p-1
    0x3fea700000000000, // 0x1.a7p-1
    0x3fea680000000000, // 0x1.a68p-1
    0x3fea600000000000, // 0x1.a6p-1
    0x3fea500000000000, // 0x1.a5p-1
    0x3fea480000000000, // 0x1.a48p-1
    0x3fea400000000000, // 0x1.a4p-1
    0x3fea300000000000, // 0x1.a3p-1
    0x3fea280000000000, // 0x1.a28p-1
    0x3fea200000000000, // 0x1.a2p-1
    0x3fea100000000000, // 0x1.a1p-1
    0x3fea080000000000, // 0x1.a08p-1
    0x3fea000000000000, // 0x1.ap-1
    0x3fe9f00000000000, // 0x1.9fp-1
    0x3fe9e80000000000, // 0x1.9e8p-1
    0x3fe9e00000000000, // 0x1.9ep-1
    0x3fe9d00000000000, // 0x1.9dp-1
    0x3fe9c80000000000, // 0x1.9c8p-1
    0x3fe9c00000000000, // 0x1.9cp-1
    0x3fe9b00000000000, // 0x1.9bp-1
    0x3fe9a80000000000, // 0x1.9a8p-1
    0x3fe9a00000000000, // 0x1.9ap-1
    0x3fe9980000000000, // 0x1.998p-1
    0x3fe9880000000000, // 0x1.988p-1
    0x3fe9800000000000, // 0x1.98p-1
    0x3fe9780000000000, // 0x1.978p-1
    0x3fe9680000000000, // 0x1.968p-1
    0x3fe9600000000000, // 0x1.96p-1
    0x3fe9580000000000, // 0x1.958p-1
    0x3fe9500000000000, // 0x1.95p-1
    0x3fe9400000000000, // 0x1.94p-1
    0x3fe9380000000000, // 0x1.938p-1
    0x3fe9300000000000, // 0x1.93p-1
    0x3fe9280000000000, // 0x1.928p-1
    0x3fe9200000000000, // 0x1.92p-1
    0x3fe9100000000000, // 0x1.91p-1
    0x3fe9080000000000, // 0x1.908p-1
    0x3fe9000000000000, // 0x1.9p-1
    0x3fe8f80000000000, // 0x1.8f8p-1
    0x3fe8e80000000000, // 0x1.8e8p-1
    0x3fe8e00000000000, // 0x1.8ep-1
    0x3fe8d80000000000, // 0x1.8d8p-1
    0x3fe8d00000000000, // 0x1.8dp-1
    0x3fe8c80000000000, // 0x1.8c8p-1
    0x3fe8b80000000000, // 0x1.8b8p-1
    0x3fe8b00000000000, // 0x1.8bp-1
    0x3fe8a80000000000, // 0x1.8a8p-1
    0x3fe8a00000000000, // 0x1.8ap-1
    0x3fe8980000000000, // 0x1.898p-1
    0x3fe8880000000000, // 0x1.888p-1
    0x3fe8800000000000, // 0x1.88p-1
    0x3fe8780000000000, // 0x1.878p-1
    0x3fe8700000000000, // 0x1.87p-1
    0x3fe8680000000000, // 0x1.868p-1
    0x3fe8600000000000, // 0x1.86p-1
    0x3fe8500000000000, // 0x1.85p-1
    0x3fe8480000000000, // 0x1.848p-1
    0x3fe8400000000000, // 0x1.84p-1
    0x3fe8380000000000, // 0x1.838p-1
    0x3fe8300000000000, // 0x1.83p-1
    0x3fe8280000000000, // 0x1.828p-1
    0x3fe8200000000000, // 0x1.82p-1
    0x3fe8100000000000, // 0x1.81p-1
    0x3fe8080000000000, // 0x1.808p-1
    0x3fe8000000000000, // 0x1.8p-1
    0x3fe7f80000000000, // 0x1.7f8p-1
    0x3fe7f00000000000, // 0x1.7fp-1
    0x3fe7e80000000000, // 0x1.7e8p-1
    0x3fe7e00000000000, // 0x1.7ep-1
    0x3fe7d80000000000, // 0x1.7d8p-1
    0x3fe7c80000000000, // 0x1.7c8p-1
    0x3fe7c00000000000, // 0x1.7cp-1
    0x3fe7b80000000000, // 0x1.7b8p-1
    0x3fe7b00000000000, // 0x1.7bp-1
    0x3fe7a80000000000, // 0x1.7a8p-1
    0x3fe7a00000000000, // 0x1.7ap-1
    0x3fe7980000000000, // 0x1.798p-1
    0x3fe7900000000000, // 0x1.79p-1
    0x3fe7880000000000, // 0x1.788p-1
    0x3fe7800000000000, // 0x1.78p-1
    0x3fe7780000000000, // 0x1.778p-1
    0x3fe7700000000000, // 0x1.77p-1
    0x3fe7600000000000, // 0x1.76p-1
    0x3fe7580000000000, // 0x1.758p-1
    0x3fe7500000000000, // 0x1.75p-1
    0x3fe7480000000000, // 0x1.748p-1
    0x3fe7400000000000, // 0x1.74p-1
    0x3fe7380000000000, // 0x1.738p-1
    0x3fe7300000000000, // 0x1.73p-1
    0x3fe7280000000000, // 0x1.728p-1
    0x3fe7200000000000, // 0x1.72p-1
    0x3fe7180000000000, // 0x1.718p-1
    0x3fe7100000000000, // 0x1.71p-1
    0x3fe7080000000000, // 0x1.708p-1
    0x3fe7000000000000, // 0x1.7p-1
    0x3fe6f80000000000, // 0x1.6f8p-1
    0x3fe6f00000000000, // 0x1.6fp-1
    0x3fe6e80000000000, // 0x1.6e8p-1
    0x3fe6e00000000000, // 0x1.6ep-1
    0x3fe6d80000000000, // 0x1.6d8p-1
    0x3fe6d00000000000, // 0x1.6dp-1
    0x3fe6c80000000000, // 0x1.6c8p-1
    0x3fe6c00000000000, // 0x1.6cp-1
    0x3fe6b80000000000, // 0x1.6b8p-1
    0x3fe6b00000000000, // 0x1.6bp-1
    0x3fe6a80000000000, // 0x1.6a8p-1
    0x3fe6a00000000000, // 0x1.6ap-1
]);
/// `_LOG_INV`: for `362 <= i <= 724`, `(h, l) = LOG_INV[i-362]` is a double-double
/// approximation of `-log(r)` with `r = INVERSE[i-362]`, with `h` an integer multiple of
/// 2^-42 and `|l| < 2^-43`. The maximal difference between `-log(r)` and `h+l` is bounded by
/// 1/2 ulp(l) < 2^-97.
static LOG_INV: [[f64; 2]; 363] = f64_pair_table([
    [0xbfd615ddb4bec000, 0xbd13c7ca90bc04b2], // {-0x1.615ddb4becp-2, -0x1.3c7ca90bc04b2p-46}
    [0xbfd5e87b20c29000, 0xbd3527d18f7738fa], // {-0x1.5e87b20c29p-2, -0x1.527d18f7738fap-44}
    [0xbfd5baf846aa2000, 0x3d339ae8f873fa41], // {-0x1.5baf846aa2p-2, 0x1.39ae8f873fa41p-44}
    [0xbfd58d54f86e0000, 0xbd2791f30a795215], // {-0x1.58d54f86ep-2, -0x1.791f30a795215p-45}
    [0xbfd55f9107a44000, 0x3d11e64778df4a62], // {-0x1.55f9107a44p-2, 0x1.1e64778df4a62p-46}
    [0xbfd531ac457ee000, 0xbd3df83b7d931501], // {-0x1.531ac457eep-2, -0x1.df83b7d931501p-44}
    [0xbfd503a682cb2000, 0x3d2a68c8f16f9b5d], // {-0x1.503a682cb2p-2, 0x1.a68c8f16f9b5dp-45}
    [0xbfd4ec9732600000, 0xbd234d7aaf04d104], // {-0x1.4ec97326p-2, -0x1.34d7aaf04d104p-45}
    [0xbfd4be5f95778000, 0x3d3d7c92cd9ad824], // {-0x1.4be5f95778p-2, 0x1.d7c92cd9ad824p-44}
    [0xbfd4900680401000, 0x3d38bccffe1a0f8c], // {-0x1.4900680401p-2, 0x1.8bccffe1a0f8cp-44}
    [0xbfd4618bc21c6000, 0x3d13d82f484c84cc], // {-0x1.4618bc21c6p-2, 0x1.3d82f484c84ccp-46}
    [0xbfd432ef2a04f000, 0x3d3fb129931715ad], // {-0x1.432ef2a04fp-2, 0x1.fb129931715adp-44}
    [0xbfd404308686a000, 0xbd3f8ef43049f7d3], // {-0x1.404308686ap-2, -0x1.f8ef43049f7d3p-44}
    [0xbfd3d54fa5c1f000, 0xbd3c3e1cd9a395e3], // {-0x1.3d54fa5c1fp-2, -0x1.c3e1cd9a395e3p-44}
    [0xbfd3a64c55694000, 0xbd37a71cbcd735d0], // {-0x1.3a64c55694p-2, -0x1.7a71cbcd735dp-44}
    [0xbfd3772662bfe000, 0x3d3e9436ac53b023], // {-0x1.3772662bfep-2, 0x1.e9436ac53b023p-44}
    [0xbfd35f865c933000, 0x3d3b07de4ea1a54a], // {-0x1.35f865c933p-2, 0x1.b07de4ea1a54ap-44}
    [0xbfd3302c16586000, 0xbd36217dc2a3e08b], // {-0x1.3302c16586p-2, -0x1.6217dc2a3e08bp-44}
    [0xbfd300aead063000, 0xbd342f568b75fcac], // {-0x1.300aead063p-2, -0x1.42f568b75fcacp-44}
    [0xbfd2d10dec508000, 0xbd360c61f7088353], // {-0x1.2d10dec508p-2, -0x1.60c61f7088353p-44}
    [0xbfd2a1499f763000, 0x3d30dbbf51f3aadc], // {-0x1.2a1499f763p-2, 0x1.0dbbf51f3aadcp-44}
    [0xbfd2895a13de8000, 0xbd3a8d7ad24c13f0], // {-0x1.2895a13de8p-2, -0x1.a8d7ad24c13fp-44}
    [0xbfd2596010df7000, 0xbd38e7bc224ea3e3], // {-0x1.2596010df7p-2, -0x1.8e7bc224ea3e3p-44}
    [0xbfd22941fbcf8000, 0x3d3a6976f5eb0963], // {-0x1.22941fbcf8p-2, 0x1.a6976f5eb0963p-44}
    [0xbfd1f8ff9e48a000, 0xbd27946c040cbe77], // {-0x1.1f8ff9e48ap-2, -0x1.7946c040cbe77p-45}
    [0xbfd1c898c169a000, 0x3d381410e5c62aff], // {-0x1.1c898c169ap-2, 0x1.81410e5c62affp-44}
    [0xbfd1b05791f08000, 0x3d32dd466dc55e2d], // {-0x1.1b05791f08p-2, 0x1.2dd466dc55e2dp-44}
    [0xbfd17fb98e151000, 0x3d3a8a8ba74a2684], // {-0x1.17fb98e151p-2, 0x1.a8a8ba74a2684p-44}
    [0xbfd14ef67f887000, 0x3d3e97a65dfc9794], // {-0x1.14ef67f887p-2, 0x1.e97a65dfc9794p-44}
    [0xbfd136870293b000, 0x3d3d3e8499d67123], // {-0x1.136870293bp-2, 0x1.d3e8499d67123p-44}
    [0xbfd1058bf9ae5000, 0x3d34ab9d817d52cd], // {-0x1.1058bf9ae5p-2, 0x1.4ab9d817d52cdp-44}
    [0xbfd0d46b579ab000, 0xbd3d2c81f640e1e6], // {-0x1.0d46b579abp-2, -0x1.d2c81f640e1e6p-44}
    [0xbfd0a324e2739000, 0xbd0c6bee7ef4030e], // {-0x1.0a324e2739p-2, -0x1.c6bee7ef4030ep-47}
    [0xbfd08a73667c5000, 0xbd3ebc1d40c5a329], // {-0x1.08a73667c5p-2, -0x1.ebc1d40c5a329p-44}
    [0xbfd058f3c703f000, 0x3d30e866bcd236ad], // {-0x1.058f3c703fp-2, 0x1.0e866bcd236adp-44}
    [0xbfd0402594b4d000, 0xbcf036b89ef42d7f], // {-0x1.0402594b4dp-2, -0x1.036b89ef42d7fp-48}
    [0xbfd00e6c45ad5000, 0xbcdcc68d52e01203], // {-0x1.00e6c45ad5p-2, -0x1.cc68d52e01203p-50}
    [0xbfcfb9186d5e4000, 0x3d0d572aab993c87], // {-0x1.fb9186d5e4p-3, 0x1.d572aab993c87p-47}
    [0xbfcf871b28956000, 0x3d3f75fd6a526efe], // {-0x1.f871b28956p-3, 0x1.f75fd6a526efep-44}
    [0xbfcf22e5e72f2000, 0x3d3f454f1417e41f], // {-0x1.f22e5e72f2p-3, 0x1.f454f1417e41fp-44}
    [0xbfcebe61f4dd8000, 0x3d23d45330fdca4d], // {-0x1.ebe61f4dd8p-3, 0x1.3d45330fdca4dp-45}
    [0xbfce8c0252aa6000, 0x3d26805b80e8e6ff], // {-0x1.e8c0252aa6p-3, 0x1.6805b80e8e6ffp-45}
    [0xbfce27076e2b0000, 0x3d3a342c2af0003c], // {-0x1.e27076e2bp-3, 0x1.a342c2af0003cp-44}
    [0xbfcdc1bca0abe000, 0xbd38fac1a628ccc6], // {-0x1.dc1bca0abep-3, -0x1.8fac1a628ccc6p-44}
    [0xbfcd8ef91af32000, 0x3d15105fc364c784], // {-0x1.d8ef91af32p-3, 0x1.5105fc364c784p-46}
    [0xbfcd293581b6c000, 0x3d383270128aaa5f], // {-0x1.d293581b6cp-3, 0x1.83270128aaa5fp-44}
    [0xbfccf6354e09c000, 0xbd2771239a07d55b], // {-0x1.cf6354e09cp-3, -0x1.771239a07d55bp-45}
    [0xbfcc8ff7c79aa000, 0x3d27794f689f8434], // {-0x1.c8ff7c79aap-3, 0x1.7794f689f8434p-45}
    [0xbfcc5cba543ae000, 0xbd20929decb454fc], // {-0x1.c5cba543aep-3, -0x1.0929decb454fcp-45}
    [0xbfcbf601bb0e4000, 0xbd2386a947c378b5], // {-0x1.bf601bb0e4p-3, -0x1.386a947c378b5p-45}
    [0xbfcbc286742d8000, 0xbd39ac53f39d121c], // {-0x1.bc286742d8p-3, -0x1.9ac53f39d121cp-44}
    [0xbfcb5b519e8fc000, 0x3d34b722ec011f31], // {-0x1.b5b519e8fcp-3, 0x1.4b722ec011f31p-44}
    [0xbfcaf3c94e80c000, 0x3cba4e633fcd9066], // {-0x1.af3c94e80cp-3, 0x1.a4e633fcd9066p-52}
    [0xbfcabfe5ae462000, 0x3d3b68f5395f139d], // {-0x1.abfe5ae462p-3, 0x1.b68f5395f139dp-44}
    [0xbfca57df28244000, 0xbd3b99c8ca1d9abb], // {-0x1.a57df28244p-3, -0x1.b99c8ca1d9abbp-44}
    [0xbfca23bc1fe2c000, 0x3d3539cd91dc9f0b], // {-0x1.a23bc1fe2cp-3, 0x1.539cd91dc9f0bp-44}
    [0xbfc9bb362e7e0000, 0x3d21f2a8a1ce0ffc], // {-0x1.9bb362e7ep-3, 0x1.1f2a8a1ce0ffcp-45}
    [0xbfc986d322818000, 0xbcf93b564dd44000], // {-0x1.986d322818p-3, -0x1.93b564dd44p-48}
    [0xbfc91dcc8c340000, 0xbd37bc6abddeff46], // {-0x1.91dcc8c34p-3, -0x1.7bc6abddeff46p-44}
    [0xbfc8e928de886000, 0xbd3a8154b13d72d5], // {-0x1.8e928de886p-3, -0x1.a8154b13d72d5p-44}
    [0xbfc87fa06520c000, 0xbd322120401202fc], // {-0x1.87fa06520cp-3, -0x1.22120401202fcp-44}
    [0xbfc84abb75866000, 0x3d3d8daadf4e2bd2], // {-0x1.84abb75866p-3, 0x1.d8daadf4e2bd2p-44}
    [0xbfc815c0a1436000, 0x3d302a52f9201ce8], // {-0x1.815c0a1436p-3, 0x1.02a52f9201ce8p-44}
    [0xbfc7ab890210e000, 0x3d2bdb9072534a58], // {-0x1.7ab890210ep-3, 0x1.bdb9072534a58p-45}
    [0xbfc7764c128f2000, 0xbd0274903479e3d1], // {-0x1.7764c128f2p-3, -0x1.274903479e3d1p-47}
    [0xbfc70b8f97a1a000, 0xbd34ea64f6a95bef], // {-0x1.70b8f97a1ap-3, -0x1.4ea64f6a95befp-44}
    [0xbfc6d60fe719e000, 0x3d3bc6e557134767], // {-0x1.6d60fe719ep-3, 0x1.bc6e557134767p-44}
    [0xbfc66acd4272a000, 0xbd3aa1bdbfc6c785], // {-0x1.66acd4272ap-3, -0x1.aa1bdbfc6c785p-44}
    [0xbfc6350a28aaa000, 0xbd2d5ec0ab8163af], // {-0x1.6350a28aaap-3, -0x1.d5ec0ab8163afp-45}
    [0xbfc5ff3070a7a000, 0x3d38586f183bebf2], // {-0x1.5ff3070a7ap-3, 0x1.8586f183bebf2p-44}
    [0xbfc59338d9982000, 0xbcf0ba68b7555d4a], // {-0x1.59338d9982p-3, -0x1.0ba68b7555d4ap-48}
    [0xbfc55d1ad4232000, 0xbd3add94dda647e8], // {-0x1.55d1ad4232p-3, -0x1.add94dda647e8p-44}
    [0xbfc4f099f4a24000, 0x3d3e9bf2fafeaf27], // {-0x1.4f099f4a24p-3, 0x1.e9bf2fafeaf27p-44}
    [0xbfc4ba36f39a6000, 0x3d34354bb3f219e5], // {-0x1.4ba36f39a6p-3, 0x1.4354bb3f219e5p-44}
    [0xbfc483bccce6e000, 0xbd1eea52723f6369], // {-0x1.483bccce6ep-3, -0x1.eea52723f6369p-46}
    [0xbfc41682bf728000, 0x3d210047081f849d], // {-0x1.41682bf728p-3, 0x1.10047081f849dp-45}
    [0xbfc3dfc2b0ecc000, 0xbd28a72a62b8c13f], // {-0x1.3dfc2b0eccp-3, -0x1.8a72a62b8c13fp-45}
    [0xbfc371fc201e8000, 0xbd3ee8779b2d8abc], // {-0x1.371fc201e8p-3, -0x1.ee8779b2d8abcp-44}
    [0xbfc33af575770000, 0xbd3c9ecca2fe72a5], // {-0x1.33af57577p-3, -0x1.c9ecca2fe72a5p-44}
    [0xbfc303d718e48000, 0x3cd680b5ce3ecb05], // {-0x1.303d718e48p-3, 0x1.680b5ce3ecb05p-50}
    [0xbfc29552f8200000, 0x3d35b967f4471dfc], // {-0x1.29552f82p-3, 0x1.5b967f4471dfcp-44}
    [0xbfc25ded0abc6000, 0xbd35a3854f176449], // {-0x1.25ded0abc6p-3, -0x1.5a3854f176449p-44}
    [0xbfc2266f190a6000, 0x3d24d20ab840e7f6], // {-0x1.2266f190a6p-3, 0x1.4d20ab840e7f6p-45}
    [0xbfc1b72ad52f6000, 0xbd2e80a41811a396], // {-0x1.1b72ad52f6p-3, -0x1.e80a41811a396p-45}
    [0xbfc17f6458fca000, 0xbd2843fad093c8dc], // {-0x1.17f6458fcap-3, -0x1.843fad093c8dcp-45}
    [0xbfc1478584674000, 0xbd1563451027c750], // {-0x1.1478584674p-3, -0x1.563451027c75p-46}
    [0xbfc0d77e7cd08000, 0xbd3cb2cd2ee2f482], // {-0x1.0d77e7cd08p-3, -0x1.cb2cd2ee2f482p-44}
    [0xbfc09f561ee72000, 0x3d28f3057157d1a8], // {-0x1.09f561ee72p-3, 0x1.8f3057157d1a8p-45}
    [0xbfc0671512ca6000, 0x3d2a47579cdc0a3d], // {-0x1.0671512ca6p-3, 0x1.a47579cdc0a3dp-45}
    [0xbfc02ebb42bf4000, 0x3d15a8fa5ce00e5d], // {-0x1.02ebb42bf4p-3, 0x1.5a8fa5ce00e5dp-46}
    [0xbfbf7b79fec38000, 0x3d010987e897ed01], // {-0x1.f7b79fec38p-4, 0x1.10987e897ed01p-47}
    [0xbfbf0a30c0118000, 0x3d3d599e83368e91], // {-0x1.f0a30c0118p-4, 0x1.d599e83368e91p-44}
    [0xbfbe98b549670000, 0xbd34677489c50e97], // {-0x1.e98b54967p-4, -0x1.4677489c50e97p-44}
    [0xbfbe27076e2b0000, 0x3d2a342c2af0003c], // {-0x1.e27076e2bp-4, 0x1.a342c2af0003cp-45}
    [0xbfbd4313d66cc000, 0x3d29454379135713], // {-0x1.d4313d66ccp-4, 0x1.9454379135713p-45}
    [0xbfbcd0cdbf8c0000, 0xbd33e14db50dd743], // {-0x1.cd0cdbf8cp-4, -0x1.3e14db50dd743p-44}
    [0xbfbc5e548f5bc000, 0xbd1d0c57585fbe06], // {-0x1.c5e548f5bcp-4, -0x1.d0c57585fbe06p-46}
    [0xbfbb78c82bb10000, 0x3d325ef7bc3987e7], // {-0x1.b78c82bb1p-4, 0x1.25ef7bc3987e7p-44}
    [0xbfbb05b49bee4000, 0xbd0ff22c18f84a5e], // {-0x1.b05b49bee4p-4, -0x1.ff22c18f84a5ep-47}
    [0xbfba926d3a4ac000, 0xbd3563650bd22a9c], // {-0x1.a926d3a4acp-4, -0x1.563650bd22a9cp-44}
    [0xbfba1ef1d8060000, 0xbd3cd4176df97bcb], // {-0x1.a1ef1d806p-4, -0x1.cd4176df97bcbp-44}
    [0xbfb9ab4246204000, 0x3d28a64826787061], // {-0x1.9ab4246204p-4, 0x1.8a64826787061p-45}
    [0xbfb8c345d6318000, 0xbd3b20f5acb42a66], // {-0x1.8c345d6318p-4, -0x1.b20f5acb42a66p-44}
    [0xbfb84ef898e84000, 0x3d37d5cd246977c9], // {-0x1.84ef898e84p-4, 0x1.7d5cd246977c9p-44}
    [0xbfb7da766d7b0000, 0xbd32cc844480c89b], // {-0x1.7da766d7bp-4, -0x1.2cc844480c89bp-44}
    [0xbfb765bf23a6c000, 0x3cfecbc035c4256a], // {-0x1.765bf23a6cp-4, 0x1.ecbc035c4256ap-48}
    [0xbfb6f0d28ae58000, 0x3d34b4641b664613], // {-0x1.6f0d28ae58p-4, 0x1.4b4641b664613p-44}
    [0xbfb60658a9374000, 0xbd30c3b1dee9c4f8], // {-0x1.60658a9374p-4, -0x1.0c3b1dee9c4f8p-44}
    [0xbfb590cafdf00000, 0xbd3c284f5722abaa], // {-0x1.590cafdfp-4, -0x1.c284f5722abaap-44}
    [0xbfb51b073f060000, 0xbd383f69278e686a], // {-0x1.51b073f06p-4, -0x1.83f69278e686ap-44}
    [0xbfb4a50d3aa1c000, 0x3d2f7fe1308973e2], // {-0x1.4a50d3aa1cp-4, 0x1.f7fe1308973e2p-45}
    [0xbfb42edcbea64000, 0xbd1bc0eeea7c9acd], // {-0x1.42edcbea64p-4, -0x1.bc0eeea7c9acdp-46}
    [0xbfb341d7961bc000, 0xbd31d09299837610], // {-0x1.341d7961bcp-4, -0x1.1d0929983761p-44}
    [0xbfb2cb0283f5c000, 0xbd3e1ee2ca657021], // {-0x1.2cb0283f5cp-4, -0x1.e1ee2ca657021p-44}
    [0xbfb253f62f0a0000, 0xbd3416f8fb69a701], // {-0x1.253f62f0ap-4, -0x1.416f8fb69a701p-44}
    [0xbfb1dcb263db0000, 0xbd39444f5e9e8981], // {-0x1.1dcb263dbp-4, -0x1.9444f5e9e8981p-44}
    [0xbfb16536eea38000, 0x3d147c5e768fa309], // {-0x1.16536eea38p-4, 0x1.47c5e768fa309p-46}
    [0xbfb0ed839b554000, 0x3d3901f46d48abb4], // {-0x1.0ed839b554p-4, 0x1.901f46d48abb4p-44}
    [0xbfb0759835990000, 0x3d3b8ecfe4b59987], // {-0x1.075983599p-4, 0x1.b8ecfe4b59987p-44}
    [0xbfaf0a30c0118000, 0x3d2d599e83368e91], // {-0x1.f0a30c0118p-5, 0x1.d599e83368e91p-45}
    [0xbfae19070c278000, 0x3d2fea4664629e86], // {-0x1.e19070c278p-5, 0x1.fea4664629e86p-45}
    [0xbfad276b8adb0000, 0xbd16a423c78a64b0], // {-0x1.d276b8adbp-5, -0x1.6a423c78a64bp-46}
    [0xbfac355dd0920000, 0xbd2f2ccc9abf8388], // {-0x1.c355dd092p-5, -0x1.f2ccc9abf8388p-45}
    [0xbfab42dd71198000, 0x3d1c827ae5d6704c], // {-0x1.b42dd71198p-5, 0x1.c827ae5d6704cp-46}
    [0xbfaa4fe9ffa40000, 0x3d36e584a0402925], // {-0x1.a4fe9ffa4p-5, 0x1.6e584a0402925p-44}
    [0xbfa95c830ec90000, 0x3d2c148297c5feb8], // {-0x1.95c830ec9p-5, 0x1.c148297c5feb8p-45}
    [0xbfa868a830840000, 0x3d12623a134ac693], // {-0x1.868a83084p-5, 0x1.2623a134ac693p-46}
    [0xbfa77458f6330000, 0x3d3181dce586af09], // {-0x1.77458f633p-5, 0x1.181dce586af09p-44}
    [0xbfa58a5bafc90000, 0x3d2b2b739570ad39], // {-0x1.58a5bafc9p-5, 0x1.b2b739570ad39p-45}
    [0xbfa494acc34d8000, 0xbd211c78a56fd247], // {-0x1.494acc34d8p-5, -0x1.11c78a56fd247p-45}
    [0xbfa39e87b9fe8000, 0xbd3eafd480ad9015], // {-0x1.39e87b9fe8p-5, -0x1.eafd480ad9015p-44}
    [0xbfa2a7ec22150000, 0x3d278ce77a9163fe], // {-0x1.2a7ec2215p-5, 0x1.78ce77a9163fep-45}
    [0xbfa1b0d989240000, 0x3d33401e9ae889bb], // {-0x1.1b0d98924p-5, 0x1.3401e9ae889bbp-44}
    [0xbfa0b94f7c198000, 0x3d2e89896f022783], // {-0x1.0b94f7c198p-5, 0x1.e89896f022783p-45}
    [0xbf9f829b0e780000, 0xbd2980267c7e09e4], // {-0x1.f829b0e78p-6, -0x1.980267c7e09e4p-45}
    [0xbf9d91a66c540000, 0xbd2e61f1658cfb9a], // {-0x1.d91a66c54p-6, -0x1.e61f1658cfb9ap-45}
    [0xbf9b9fc027b00000, 0x3d3b9a010ae6922a], // {-0x1.b9fc027bp-6, 0x1.b9a010ae6922ap-44}
    [0xbf99ace7551d0000, 0x3d2d75d97ec7c410], // {-0x1.9ace7551dp-6, 0x1.d75d97ec7c41p-45}
    [0xbf97b91b07d60000, 0x3d33b955b602ace4], // {-0x1.7b91b07d6p-6, 0x1.3b955b602ace4p-44}
    [0xbf95c45a51b90000, 0x3d263bb6216d87d8], // {-0x1.5c45a51b9p-6, 0x1.63bb6216d87d8p-45}
    [0xbf93cea443470000, 0x3d36a2c432d6a40b], // {-0x1.3cea44347p-6, 0x1.6a2c432d6a40bp-44}
    [0xbf91d7f7eb9f0000, 0x3d14193a83fcc7a6], // {-0x1.1d7f7eb9fp-6, 0x1.4193a83fcc7a6p-46}
    [0xbf8fc0a8b0fc0000, 0xbcdf1e7cf6d3a69c], // {-0x1.fc0a8b0fcp-7, -0x1.f1e7cf6d3a69cp-50}
    [0xbf8bcf712c740000, 0xbd1c25e097bd9771], // {-0x1.bcf712c74p-7, -0x1.c25e097bd9771p-46}
    [0xbf87dc475f820000, 0x3d3eb1245b5da1f5], // {-0x1.7dc475f82p-7, 0x1.eb1245b5da1f5p-44}
    [0xbf83e7295d260000, 0x3d2609c1ff29a114], // {-0x1.3e7295d26p-7, 0x1.609c1ff29a114p-45}
    [0xbf7fe02a6b100000, 0xbd19e23f0dda40e4], // {-0x1.fe02a6b1p-8, -0x1.9e23f0dda40e4p-46}
    [0xbf77ee11ebd80000, 0xbd0749d3c2d23a07], // {-0x1.7ee11ebd8p-8, -0x1.749d3c2d23a07p-47}
    [0xbf6ff00aa2b00000, 0xbd20bc04a086b56a], // {-0x1.ff00aa2bp-9, -0x1.0bc04a086b56ap-45}
    [0xbf5ff802a9b00000, 0x3d33bc661d61c5eb], // {-0x1.ff802a9bp-10, 0x1.3bc661d61c5ebp-44}
    [0x3f50020055600000, 0x3d356224cd5f35f8], // {0x1.00200556p-10, 0x1.56224cd5f35f8p-44}
    [0x3f68090482880000, 0x3d285c0696a70c0c], // {0x1.809048288p-9, 0x1.85c0696a70c0cp-45}
    [0x3f740c8a74780000, 0x3d1e3871df070002], // {0x1.40c8a7478p-8, 0x1.e3871df070002p-46}
    [0x3f7c189cbb100000, 0xbd3d805512588560], // {0x1.c189cbb1p-8, -0x1.d80551258856p-44}
    [0x3f82145e939e0000, 0x3d3e3d1238c4ea00], // {0x1.2145e939ep-7, 0x1.e3d1238c4eap-44}
    [0x3f861e77e8b60000, 0xbd38073eeaf8eaf3], // {0x1.61e77e8b6p-7, -0x1.8073eeaf8eaf3p-44}
    [0x3f8a2a9c6c180000, 0xbd3f73bc4d6d3472], // {0x1.a2a9c6c18p-7, -0x1.f73bc4d6d3472p-44}
    [0x3f8e38ce30340000, 0xbd39de88a3da281a], // {0x1.e38ce3034p-7, -0x1.9de88a3da281ap-44}
    [0x3f912487a5500000, 0x3d3fdbe5fed4b393], // {0x1.12487a55p-6, 0x1.fdbe5fed4b393p-44}
    [0x3f932db0ea130000, 0x3d2710cb130895fc], // {0x1.32db0ea13p-6, 0x1.710cb130895fcp-45}
    [0x3f9537e3f45f0000, 0x3d2ab259d2d7f253], // {0x1.537e3f45fp-6, 0x1.ab259d2d7f253p-45}
    [0x3f963d6178690000, 0x3d07abf389596542], // {0x1.63d617869p-6, 0x1.7abf389596542p-47}
    [0x3f98492528c90000, 0xbd2aa0ba325a0c34], // {0x1.8492528c9p-6, -0x1.aa0ba325a0c34p-45}
    [0x3f9a55f548c60000, 0xbd2de0709f2d03c9], // {0x1.a55f548c6p-6, -0x1.de0709f2d03c9p-45}
    [0x3f9c63d2ec150000, 0xbd35439ce030a687], // {0x1.c63d2ec15p-6, -0x1.5439ce030a687p-44}
    [0x3f9e72bf28140000, 0xbd28d75149774d47], // {0x1.e72bf2814p-6, -0x1.8d75149774d47p-45}
    [0x3fa0415d89e78000, 0xbd3dddc7f461c516], // {0x1.0415d89e78p-5, -0x1.dddc7f461c516p-44}
    [0x3fa149e3e4008000, 0xbd32b98a9a4168fd], // {0x1.149e3e4008p-5, -0x1.2b98a9a4168fdp-44}
    [0x3fa252f32f8d0000, 0x3d283e9ae021b67b], // {0x1.252f32f8dp-5, 0x1.83e9ae021b67bp-45}
    [0x3fa35c8bfaa10000, 0x3d38357d5ef9eb35], // {0x1.35c8bfaa1p-5, 0x1.8357d5ef9eb35p-44}
    [0x3fa3e18c1ca08000, 0x3d3748ed3f6e378e], // {0x1.3e18c1ca08p-5, 0x1.748ed3f6e378ep-44}
    [0x3fa4ebf4334a0000, 0xbd2d9150f73be773], // {0x1.4ebf4334ap-5, -0x1.d9150f73be773p-45}
    [0x3fa5f6e730790000, 0xbd20485a8012494c], // {0x1.5f6e73079p-5, -0x1.0485a8012494cp-45}
    [0x3fa70265a5510000, 0xbd2888df11fd5ce7], // {0x1.70265a551p-5, -0x1.888df11fd5ce7p-45}
    [0x3fa80e7023d90000, 0xbd399dc16f28bf45], // {0x1.80e7023d9p-5, -0x1.99dc16f28bf45p-44}
    [0x3fa91b073efd8000, 0xbd19d7c53f76ca96], // {0x1.91b073efd8p-5, -0x1.9d7c53f76ca96p-46}
    [0x3fa9a187b5740000, 0xbd30c22e4ec4d90d], // {0x1.9a187b574p-5, -0x1.0c22e4ec4d90dp-44}
    [0x3faaaef2d0fb0000, 0x3d20fc1a353bb42e], // {0x1.aaef2d0fbp-5, 0x1.0fc1a353bb42ep-45}
    [0x3fabbcebfc690000, 0xbd17bf868c317c2a], // {0x1.bbcebfc69p-5, -0x1.7bf868c317c2ap-46}
    [0x3faccb73cddd8000, 0x3d3965c36e09f5fe], // {0x1.ccb73cddd8p-5, 0x1.965c36e09f5fep-44}
    [0x3fadda8adc680000, 0xbd21b1ac64d9e42f], // {0x1.dda8adc68p-5, -0x1.1b1ac64d9e42fp-45}
    [0x3fae624c4a0b8000, 0xbd30f25c74676689], // {0x1.e624c4a0b8p-5, -0x1.0f25c74676689p-44}
    [0x3faf723b51800000, 0xbd3d6eb0dd5610d3], // {0x1.f723b518p-5, -0x1.d6eb0dd5610d3p-44}
    [0x3fb0415d89e74000, 0x3d1111c05cf1d753], // {0x1.0415d89e74p-4, 0x1.111c05cf1d753p-46}
    [0x3fb0c9e615ac4000, 0x3d2c2da80974d976], // {0x1.0c9e615ac4p-4, 0x1.c2da80974d976p-45}
    [0x3fb10e45b3cb0000, 0xbd37cf69284a3465], // {0x1.10e45b3cbp-4, -0x1.7cf69284a3465p-44}
    [0x3fb1973bd1464000, 0x3d3566d154f930b3], // {0x1.1973bd1464p-4, 0x1.566d154f930b3p-44}
    [0x3fb2207b5c784000, 0x3d349d8cfc10c7bf], // {0x1.2207b5c784p-4, 0x1.49d8cfc10c7bfp-44}
    [0x3fb2aa04a4470000, 0x3d37a48ba8b1cb41], // {0x1.2aa04a447p-4, 0x1.7a48ba8b1cb41p-44}
    [0x3fb2eee507b40000, 0x3d08081edd77c860], // {0x1.2eee507b4p-4, 0x1.8081edd77c86p-47}
    [0x3fb378dd7f748000, 0x3d37141128f1faca], // {0x1.378dd7f748p-4, 0x1.7141128f1facap-44}
    [0x3fb403207b414000, 0x3d26fd84aa8157c0], // {0x1.403207b414p-4, 0x1.6fd84aa8157cp-45}
    [0x3fb4485e03dbc000, 0x3d3fad46e8d26ab7], // {0x1.4485e03dbcp-4, 0x1.fad46e8d26ab7p-44}
    [0x3fb4d3115d208000, 0xbcf53a2582f4e1ef], // {0x1.4d3115d208p-4, -0x1.53a2582f4e1efp-48}
    [0x3fb55e10050e0000, 0x3d0c1d740c53c72e], // {0x1.55e10050ep-4, 0x1.c1d740c53c72ep-47}
    [0x3fb5e95a4d978000, 0x3d31cb7ce1d17171], // {0x1.5e95a4d978p-4, 0x1.1cb7ce1d17171p-44}
    [0x3fb62f1be7d78000, 0xbd2179957ed63c4e], // {0x1.62f1be7d78p-4, -0x1.179957ed63c4ep-45}
    [0x3fb6bad83c188000, 0x3d0daf3cc08926ae], // {0x1.6bad83c188p-4, 0x1.daf3cc08926aep-47}
    [0x3fb746e100228000, 0xbd3126d16e1e21d2], // {0x1.746e100228p-4, -0x1.126d16e1e21d2p-44}
    [0x3fb78d02263d8000, 0x3d069b5794b69fb7], // {0x1.78d02263d8p-4, 0x1.69b5794b69fb7p-47}
    [0x3fb8197e2f410000, 0xbd3c0fe460d20041], // {0x1.8197e2f41p-4, -0x1.c0fe460d20041p-44}
    [0x3fb8a6477a91c000, 0x3d3c28c0af9bd6df], // {0x1.8a6477a91cp-4, 0x1.c28c0af9bd6dfp-44}
    [0x3fb8ecc933aec000, 0xbd222f39be67f7aa], // {0x1.8ecc933aecp-4, -0x1.22f39be67f7aap-45}
    [0x3fb97a07024cc000, 0xbcf8bcc1732093ce], // {0x1.97a07024ccp-4, -0x1.8bcc1732093cep-48}
    [0x3fba0792e9278000, 0xbd0a9ce6c9ad51bf], // {0x1.a0792e9278p-4, -0x1.a9ce6c9ad51bfp-47}
    [0x3fba4e7640b1c000, 0xbd0e42b6b94407c8], // {0x1.a4e7640b1cp-4, -0x1.e42b6b94407c8p-47}
    [0x3fbadc77ee5b0000, 0xbd3573b209c31904], // {0x1.adc77ee5bp-4, -0x1.573b209c31904p-44}
    [0x3fbb23965a530000, 0xbceff64eea137079], // {0x1.b23965a53p-4, -0x1.ff64eea137079p-49}
    [0x3fbbb20e936d8000, 0xbd368ba835459b8e], // {0x1.bb20e936d8p-4, -0x1.68ba835459b8ep-44}
    [0x3fbc40d6425a4000, 0x3d3cb1121d1930dd], // {0x1.c40d6425a4p-4, 0x1.cb1121d1930ddp-44}
    [0x3fbc885801bc4000, 0x3d2646d1c65aacd3], // {0x1.c885801bc4p-4, 0x1.646d1c65aacd3p-45}
    [0x3fbd179788218000, 0x3d336433b5efbeed], // {0x1.d179788218p-4, 0x1.36433b5efbeedp-44}
    [0x3fbd5f5565920000, 0x3d30e239cc185469], // {0x1.d5f556592p-4, 0x1.0e239cc185469p-44}
    [0x3fbdef0d8d468000, 0xbd324750412e9a74], // {0x1.def0d8d468p-4, -0x1.24750412e9a74p-44}
    [0x3fbe7f1691a34000, 0xbd32c1c59bc77bfa], // {0x1.e7f1691a34p-4, -0x1.2c1c59bc77bfap-44}
    [0x3fbec739830a0000, 0x3d311fcba80cdd10], // {0x1.ec739830ap-4, 0x1.11fcba80cdd1p-44}
    [0x3fbf57bc7d900000, 0x3d176a6c9ea8b04e], // {0x1.f57bc7d9p-4, 0x1.76a6c9ea8b04ep-46}
    [0x3fbfa01c9db58000, 0xbd08f351fa48a730], // {0x1.fa01c9db58p-4, -0x1.8f351fa48a73p-47}
    [0x3fc0188d2ecf6000, 0x3d03f9651cff9dfe], // {0x1.0188d2ecf6p-3, 0x1.3f9651cff9dfep-47}
    [0x3fc03cdc0a51e000, 0x3d381a9cf169fc5c], // {0x1.03cdc0a51ep-3, 0x1.81a9cf169fc5cp-44}
    [0x3fc08598b59e4000, 0xbd27e5dd7009902c], // {0x1.08598b59e4p-3, -0x1.7e5dd7009902cp-45}
    [0x3fc0aa0691268000, 0xbd345519d7032129], // {0x1.0aa0691268p-3, -0x1.45519d7032129p-44}
    [0x3fc0f301717d0000, 0xbd3e09b441ae86c5], // {0x1.0f301717dp-3, -0x1.e09b441ae86c5p-44}
    [0x3fc13c2605c3a000, 0xbd2cf5fdd94f6509], // {0x1.13c2605c3ap-3, -0x1.cf5fdd94f6509p-45}
    [0x3fc160c8024b2000, 0x3d2ec2d2a9009e3d], // {0x1.160c8024b2p-3, 0x1.ec2d2a9009e3dp-45}
    [0x3fc1aa2b7e240000, 0xbd31ac38dde3b366], // {0x1.1aa2b7e24p-3, -0x1.1ac38dde3b366p-44}
    [0x3fc1ceed09854000, 0xbd315c1c39192af9], // {0x1.1ceed09854p-3, -0x1.15c1c39192af9p-44}
    [0x3fc2188fd9808000, 0xbd3b3a1e7f50c701], // {0x1.2188fd9808p-3, -0x1.b3a1e7f50c701p-44}
    [0x3fc23d712a49c000, 0x3d100d238fd3df5c], // {0x1.23d712a49cp-3, 0x1.00d238fd3df5cp-46}
    [0x3fc28753bc11a000, 0x3d37494e359302e6], // {0x1.28753bc11ap-3, 0x1.7494e359302e6p-44}
    [0x3fc2ac55095f6000, 0xbd1d3466d0c6c8a8], // {0x1.2ac55095f6p-3, -0x1.d3466d0c6c8a8p-46}
    [0x3fc2f677cbbc0000, 0x3d352b302160f40d], // {0x1.2f677cbbcp-3, 0x1.52b302160f40dp-44}
    [0x3fc31b994d3a4000, 0x3d3f098ee3a50810], // {0x1.31b994d3a4p-3, 0x1.f098ee3a5081p-44}
    [0x3fc365fcb015a000, 0xbd3fd3a0afb9691b], // {0x1.365fcb015ap-3, -0x1.fd3a0afb9691bp-44}
    [0x3fc38b3e9e028000, 0xbd370ef0545c17f9], // {0x1.38b3e9e028p-3, -0x1.70ef0545c17f9p-44}
    [0x3fc3d5e3126bc000, 0x3d13fb2f85096c4b], // {0x1.3d5e3126bcp-3, 0x1.3fb2f85096c4bp-46}
    [0x3fc3fb45a5992000, 0x3d319713c0cae559], // {0x1.3fb45a5992p-3, 0x1.19713c0cae559p-44}
    [0x3fc420b327410000, 0xbd116282c85a0884], // {0x1.420b32741p-3, -0x1.16282c85a0884p-46}
    [0x3fc46baf0f9f6000, 0xbd1249cd0790841a], // {0x1.46baf0f9f6p-3, -0x1.249cd0790841ap-46}
    [0x3fc4913d8333c000, 0xbd353e43558124c4], // {0x1.4913d8333cp-3, -0x1.53e43558124c4p-44}
    [0x3fc4dc7b897bc000, 0x3d0c79b60ae1ff0f], // {0x1.4dc7b897bcp-3, 0x1.c79b60ae1ff0fp-47}
    [0x3fc5022b292f6000, 0x3d348a05ff36a25b], // {0x1.5022b292f6p-3, 0x1.48a05ff36a25bp-44}
    [0x3fc54dabc2610000, 0x3d2746fee5c8d0d8], // {0x1.54dabc261p-3, 0x1.746fee5c8d0d8p-45}
    [0x3fc5737cc9018000, 0x3d39baa7a6b887f6], // {0x1.5737cc9018p-3, 0x1.9baa7a6b887f6p-44}
    [0x3fc5bf406b544000, 0xbd127023eb68981c], // {0x1.5bf406b544p-3, -0x1.27023eb68981cp-46}
    [0x3fc5e533144c2000, 0xbd31ce0bf3b290ea], // {0x1.5e533144c2p-3, -0x1.1ce0bf3b290eap-44}
    [0x3fc60b3100b0a000, 0xbd371456c988f814], // {0x1.60b3100b0ap-3, -0x1.71456c988f814p-44}
    [0x3fc6574ebe8c2000, 0xbd398c1d34f0f462], // {0x1.6574ebe8c2p-3, -0x1.98c1d34f0f462p-44}
    [0x3fc67d6e9d786000, 0xbd311e8830a706d3], // {0x1.67d6e9d786p-3, -0x1.11e8830a706d3p-44}
    [0x3fc6c9d07d204000, 0xbcdc73fafd9b2dca], // {0x1.6c9d07d204p-3, -0x1.c73fafd9b2dcap-50}
    [0x3fc6f0128b756000, 0x3d3577390d31ef0f], // {0x1.6f0128b756p-3, 0x1.577390d31ef0fp-44}
    [0x3fc716600c914000, 0x3ce51b157cec3838], // {0x1.716600c914p-3, 0x1.51b157cec3838p-49}
    [0x3fc7631d82936000, 0xbd25e77dc7c5f3e1], // {0x1.7631d82936p-3, -0x1.5e77dc7c5f3e1p-45}
    [0x3fc7898d85444000, 0x3d38e67be3dbaf3f], // {0x1.7898d85444p-3, 0x1.8e67be3dbaf3fp-44}
    [0x3fc7d6903caf6000, 0xbd24c06b17c301d7], // {0x1.7d6903caf6p-3, -0x1.4c06b17c301d7p-45}
    [0x3fc7fd22ff59a000, 0xbd158bebf457b7d2], // {0x1.7fd22ff59ap-3, -0x1.58bebf457b7d2p-46}
    [0x3fc823c16551a000, 0x3d1e0ddb9a631e83], // {0x1.823c16551ap-3, 0x1.e0ddb9a631e83p-46}
    [0x3fc871213750e000, 0x3d3328eb42f9af75], // {0x1.871213750ep-3, 0x1.328eb42f9af75p-44}
    [0x3fc897e2b17b2000, 0xbd296b37380cbe9e], // {0x1.897e2b17b2p-3, -0x1.96b37380cbe9ep-45}
    [0x3fc8beafeb390000, 0xbd073d54aae92cd1], // {0x1.8beafeb39p-3, -0x1.73d54aae92cd1p-47}
    [0x3fc90c6db9fcc000, 0xbd1935f57718d7ca], // {0x1.90c6db9fccp-3, -0x1.935f57718d7cap-46}
    [0x3fc9335e5d594000, 0x3d33115c3abd47da], // {0x1.9335e5d594p-3, 0x1.3115c3abd47dap-44}
    [0x3fc95a5adcf70000, 0x3d07f22858a0ff6f], // {0x1.95a5adcf7p-3, 0x1.7f22858a0ff6fp-47}
    [0x3fc9a8778deba000, 0x3d3470fa3efec390], // {0x1.9a8778debap-3, 0x1.470fa3efec39p-44}
    [0x3fc9cf97cdce0000, 0x3d3d862f10c414e3], // {0x1.9cf97cdcep-3, 0x1.d862f10c414e3p-44}
    [0x3fc9f6c40708a000, 0xbd3337d94bcd3f43], // {0x1.9f6c40708ap-3, -0x1.337d94bcd3f43p-44}
    [0x3fca454082e6a000, 0x3d360a77c81f7171], // {0x1.a454082e6ap-3, 0x1.60a77c81f7171p-44}
    [0x3fca6c90d44b8000, 0xbd3f63b7f037b0c6], // {0x1.a6c90d44b8p-3, -0x1.f63b7f037b0c6p-44}
    [0x3fca93ed3c8ae000, 0xbd28724350562169], // {0x1.a93ed3c8aep-3, -0x1.8724350562169p-45}
    [0x3fcae2ca6f672000, 0x3d37a8d5ae54f550], // {0x1.ae2ca6f672p-3, 0x1.7a8d5ae54f55p-44}
    [0x3fcb0a4b48fc2000, 0xbd22e72d5c3998ed], // {0x1.b0a4b48fc2p-3, -0x1.2e72d5c3998edp-45}
    [0x3fcb31d8575bc000, 0x3d3c794e562a63cb], // {0x1.b31d8575bcp-3, 0x1.c794e562a63cbp-44}
    [0x3fcb811730b82000, 0x3d1e90683b9cd768], // {0x1.b811730b82p-3, 0x1.e90683b9cd768p-46}
    [0x3fcba8c90ae4a000, 0x3d3a32e7f44432da], // {0x1.ba8c90ae4ap-3, 0x1.a32e7f44432dap-44}
    [0x3fcbd087383be000, 0xbd2d4bc4595412b6], // {0x1.bd087383bep-3, -0x1.d4bc4595412b6p-45}
    [0x3fcc2028ab180000, 0xbd292e0ee55c7ac6], // {0x1.c2028ab18p-3, -0x1.92e0ee55c7ac6p-45}
    [0x3fcc480c0005c000, 0x3d39a294d5e44e76], // {0x1.c480c0005cp-3, 0x1.9a294d5e44e76p-44}
    [0x3fcc6ffbc6f00000, 0x3d3ee138d3a69d43], // {0x1.c6ffbc6fp-3, 0x1.ee138d3a69d43p-44}
    [0x3fcc97f8079d4000, 0x3d23b161a8c6e6c5], // {0x1.c97f8079d4p-3, 0x1.3b161a8c6e6c5p-45}
    [0x3fcce816157f2000, 0xbd29e0aba2099515], // {0x1.ce816157f2p-3, -0x1.9e0aba2099515p-45}
    [0x3fcd1037f2656000, 0xbd084a7e75b6f6e4], // {0x1.d1037f2656p-3, -0x1.84a7e75b6f6e4p-47}
    [0x3fcd386668720000, 0xbd373650b38932bc], // {0x1.d38666872p-3, -0x1.73650b38932bcp-44}
    [0x3fcd88e93fb30000, 0xbd375f280234bf51], // {0x1.d88e93fb3p-3, -0x1.75f280234bf51p-44}
    [0x3fcdb13db0d48000, 0x3d32806a847527e6], // {0x1.db13db0d48p-3, 0x1.2806a847527e6p-44}
    [0x3fcdd99edaf6e000, 0xbd302ec669c756eb], // {0x1.dd99edaf6ep-3, -0x1.02ec669c756ebp-44}
    [0x3fce020cc6236000, 0xbd252b00adb91424], // {0x1.e020cc6236p-3, -0x1.52b00adb91424p-45}
    [0x3fce530effe72000, 0xbd3fdbdbb13f7c18], // {0x1.e530effe72p-3, -0x1.fdbdbb13f7c18p-44}
    [0x3fce7ba35eb78000, 0xbd0d5eee23793649], // {0x1.e7ba35eb78p-3, -0x1.d5eee23793649p-47}
    [0x3fcea4449f04a000, 0x3d35e91663732a36], // {0x1.ea4449f04ap-3, 0x1.5e91663732a36p-44}
    [0x3fceccf2c8fea000, 0xbd3bec63a3e75640], // {0x1.eccf2c8feap-3, -0x1.bec63a3e7564p-44}
    [0x3fcef5ade4dd0000, 0xbcca211565bb8e11], // {0x1.ef5ade4ddp-3, -0x1.a211565bb8e11p-51}
    [0x3fcf474b134e0000, 0xbd3bae49f1df7b5e], // {0x1.f474b134ep-3, -0x1.bae49f1df7b5ep-44}
    [0x3fcf702d36778000, 0xbd10819516673e23], // {0x1.f702d36778p-3, -0x1.0819516673e23p-46}
    [0x3fcf991c6cb3c000, 0xbd390d04cd7cc834], // {0x1.f991c6cb3cp-3, -0x1.90d04cd7cc834p-44}
    [0x3fcfc218be620000, 0x3d34bba46f1cf6a0], // {0x1.fc218be62p-3, 0x1.4bba46f1cf6ap-44}
    [0x3fd00a1c6adda000, 0x3d31cd8d688b9e18], // {0x1.00a1c6addap-2, 0x1.1cd8d688b9e18p-44}
    [0x3fd01eae5626c000, 0x3d3a43dcfade85ae], // {0x1.01eae5626cp-2, 0x1.a43dcfade85aep-44}
    [0x3fd03346e0106000, 0x3cf89ff8a966395c], // {0x1.03346e0106p-2, 0x1.89ff8a966395cp-48}
    [0x3fd047e60cde8000, 0x3d2dbdf10d397f3c], // {0x1.047e60cde8p-2, 0x1.dbdf10d397f3cp-45}
    [0x3fd05c8be0d96000, 0x3d2ad0f1c77ccb58], // {0x1.05c8be0d96p-2, 0x1.ad0f1c77ccb58p-45}
    [0x3fd085eb8f8ae000, 0x3d3e5d513f45fe7b], // {0x1.085eb8f8aep-2, 0x1.e5d513f45fe7bp-44}
    [0x3fd09aa572e6c000, 0x3d3b50a1e1734342], // {0x1.09aa572e6cp-2, 0x1.b50a1e1734342p-44}
    [0x3fd0af660eb9e000, 0x3d23c7c3f528d80a], // {0x1.0af660eb9ep-2, 0x1.3c7c3f528d80ap-45}
    [0x3fd0c42d67616000, 0x3d27188b163ceae9], // {0x1.0c42d67616p-2, 0x1.7188b163ceae9p-45}
    [0x3fd0d8fb813eb000, 0x3d1ee8c88753fa35], // {0x1.0d8fb813ebp-2, 0x1.ee8c88753fa35p-46}
    [0x3fd102ac0a35d000, 0xbd2f1fbddfdfd686], // {0x1.102ac0a35dp-2, -0x1.f1fbddfdfd686p-45}
    [0x3fd1178e8227e000, 0x3d31ef78ce2d07f2], // {0x1.1178e8227ep-2, 0x1.1ef78ce2d07f2p-44}
    [0x3fd12c77cd007000, 0x3d13b2948a11f797], // {0x1.12c77cd007p-2, 0x1.3b2948a11f797p-46}
    [0x3fd14167ef367000, 0x3d3e0c07824daaf5], // {0x1.14167ef367p-2, 0x1.e0c07824daaf5p-44}
    [0x3fd1565eed456000, 0xbcee75adfb6aba25], // {0x1.1565eed456p-2, -0x1.e75adfb6aba25p-49}
    [0x3fd16b5ccbad0000, 0xbd323299042d74bf], // {0x1.16b5ccbadp-2, -0x1.23299042d74bfp-44}
    [0x3fd1956d3b9bc000, 0x3d27d2f73ad1aa14], // {0x1.1956d3b9bcp-2, 0x1.7d2f73ad1aa14p-45}
    [0x3fd1aa7fd638d000, 0x3d29f60a9616f7a0], // {0x1.1aa7fd638dp-2, 0x1.9f60a9616f7ap-45}
    [0x3fd1bf99635a7000, 0xbd31ac89575c2125], // {0x1.1bf99635a7p-2, -0x1.1ac89575c2125p-44}
    [0x3fd1d4b9e796c000, 0x3d222a667c42e56d], // {0x1.1d4b9e796cp-2, 0x1.22a667c42e56dp-45}
    [0x3fd1e9e16788a000, 0xbd382eaed3c8b65e], // {0x1.1e9e16788ap-2, -0x1.82eaed3c8b65ep-44}
    [0x3fd1ff0fe7cf4000, 0x3d3e9d5b513ff0c1], // {0x1.1ff0fe7cf4p-2, 0x1.e9d5b513ff0c1p-44}
    [0x3fd214456d0ec000, 0xbd3caf0428b728a3], // {0x1.214456d0ecp-2, -0x1.caf0428b728a3p-44}
    [0x3fd23ec5991ec000, 0xbd36dbe448a2e522], // {0x1.23ec5991ecp-2, -0x1.6dbe448a2e522p-44}
    [0x3fd25410494e5000, 0x3d3b1d7ac0ef77f2], // {0x1.25410494e5p-2, 0x1.b1d7ac0ef77f2p-44}
    [0x3fd269621134e000, 0xbd31b61f10522625], // {0x1.269621134ep-2, -0x1.1b61f10522625p-44}
    [0x3fd27ebaf58d9000, 0xbd2b198800b4bda7], // {0x1.27ebaf58d9p-2, -0x1.b198800b4bda7p-45}
    [0x3fd2941afb187000, 0xbd3210c2b730e28b], // {0x1.2941afb187p-2, -0x1.210c2b730e28bp-44}
    [0x3fd2a982269a4000, 0xbd22058e557285cf], // {0x1.2a982269a4p-2, -0x1.2058e557285cfp-45}
    [0x3fd2bef07cdc9000, 0x3d2a9cfa4a5004f4], // {0x1.2bef07cdc9p-2, 0x1.a9cfa4a5004f4p-45}
    [0x3fd2d46602add000, 0xbd288d0ddcd54196], // {0x1.2d46602addp-2, -0x1.88d0ddcd54196p-45}
    [0x3fd2ff66b04eb000, 0xbd38aed2541e6e2e], // {0x1.2ff66b04ebp-2, -0x1.8aed2541e6e2ep-44}
    [0x3fd314f1e1d36000, 0xbd28e27ad3213cb8], // {0x1.314f1e1d36p-2, -0x1.8e27ad3213cb8p-45}
    [0x3fd32a8456512000, 0x3d04f928139af5d6], // {0x1.32a8456512p-2, 0x1.4f928139af5d6p-47}
    [0x3fd3401e12aed000, 0xbd317c73556e291d], // {0x1.3401e12aedp-2, -0x1.17c73556e291dp-44}
    [0x3fd355bf1bd83000, 0xbd2ba99b8964f0e8], // {0x1.355bf1bd83p-2, -0x1.ba99b8964f0e8p-45}
    [0x3fd36b6776be1000, 0x3d116ecdb0f177c8], // {0x1.36b6776be1p-2, 0x1.16ecdb0f177c8p-46}
    [0x3fd3811728565000, 0xbd2a71e493a0702b], // {0x1.3811728565p-2, -0x1.a71e493a0702bp-45}
    [0x3fd396ce359bc000, 0xbd05839c5663663d], // {0x1.396ce359bcp-2, -0x1.5839c5663663dp-47}
    [0x3fd3ac8ca38e6000, 0xbd2d0befbc02be4a], // {0x1.3ac8ca38e6p-2, -0x1.d0befbc02be4ap-45}
    [0x3fd3c25277333000, 0x3d183b54b606bd5c], // {0x1.3c25277333p-2, 0x1.83b54b606bd5cp-46}
    [0x3fd3d81fb5947000, 0xbd222c7c2a9d37a4], // {0x1.3d81fb5947p-2, -0x1.22c7c2a9d37a4p-45}
    [0x3fd3edf463c17000, 0xbd3f067c297f2c3f], // {0x1.3edf463c17p-2, -0x1.f067c297f2c3fp-44}
    [0x3fd419b423d5f000, 0xbd3ce379226de3ec], // {0x1.419b423d5fp-2, -0x1.ce379226de3ecp-44}
    [0x3fd42f9f3ff62000, 0x3d3906440f7d3354], // {0x1.42f9f3ff62p-2, 0x1.906440f7d3354p-44}
    [0x3fd44591e053a000, 0xbd06e95892923d88], // {0x1.44591e053ap-2, -0x1.6e95892923d88p-47}
    [0x3fd45b8c0a17e000, 0xbd0d9120e7d0a853], // {0x1.45b8c0a17ep-2, -0x1.d9120e7d0a853p-47}
    [0x3fd4718dc271c000, 0x3d306c18fb4c14c5], // {0x1.4718dc271cp-2, 0x1.06c18fb4c14c5p-44}
    [0x3fd487970e958000, 0x3d3dc1b8465cf25f], // {0x1.487970e958p-2, 0x1.dc1b8465cf25fp-44}
    [0x3fd49da7f3bcc000, 0x3d307b334daf4b9a], // {0x1.49da7f3bccp-2, 0x1.07b334daf4b9ap-44}
    [0x3fd4b3c077268000, 0xbd165b4681052b9f], // {0x1.4b3c077268p-2, -0x1.65b4681052b9fp-46}
    [0x3fd4c9e09e173000, 0xbd2e20891b0ad8a4], // {0x1.4c9e09e173p-2, -0x1.e20891b0ad8a4p-45}
    [0x3fd4e0086dd8c000, 0xbd34d692a1e44788], // {0x1.4e0086dd8cp-2, -0x1.4d692a1e44788p-44}
    [0x3fd4f637ebbaa000, 0xbd3fc158cb3124b9], // {0x1.4f637ebbaap-2, -0x1.fc158cb3124b9p-44}
    [0x3fd50c6f1d11c000, 0xbd3a0e6b7e827c2c], // {0x1.50c6f1d11cp-2, -0x1.a0e6b7e827c2cp-44}
    [0x3fd522ae0738a000, 0x3d2ebe708164c759], // {0x1.522ae0738ap-2, 0x1.ebe708164c759p-45}
    [0x3fd538f4af8f7000, 0x3d27ec02e45547ce], // {0x1.538f4af8f7p-2, 0x1.7ec02e45547cep-45}
    [0x3fd54f431b7be000, 0x3d1a8954c0910952], // {0x1.54f431b7bep-2, 0x1.a8954c0910952p-46}
    [0x3fd5659950695000, 0x3d14c5fd2badc774], // {0x1.5659950695p-2, 0x1.4c5fd2badc774p-46}
    [0x3fd57bf753c8d000, 0x3d1fadedee5d40ef], // {0x1.57bf753c8dp-2, 0x1.fadedee5d40efp-46}
    [0x3fd5925d2b113000, 0xbd369bf5a7a56f34], // {0x1.5925d2b113p-2, -0x1.69bf5a7a56f34p-44}
    [0x3fd5a8cadbbee000, 0xbcf7c79b0af7ecf8], // {0x1.5a8cadbbeep-2, -0x1.7c79b0af7ecf8p-48}
    [0x3fd5bf406b544000, 0xbd227023eb68981c], // {0x1.5bf406b544p-2, -0x1.27023eb68981cp-45}
    [0x3fd5d5bddf596000, 0xbd0a0b2a08a465dc], // {0x1.5d5bddf596p-2, -0x1.a0b2a08a465dcp-47}
    [0x3fd5ec433d5c3000, 0x3d36b71a1229d17f], // {0x1.5ec433d5c3p-2, 0x1.6b71a1229d17fp-44}
    [0x3fd602d08af09000, 0x3d1ebe9176df3f65], // {0x1.602d08af09p-2, 0x1.ebe9176df3f65p-46}
    [0x3fd61965cdb03000, 0xbd2f08ad603c488e], // {0x1.61965cdb03p-2, -0x1.f08ad603c488ep-45}
    [0x3fd630030b3ab000, 0xbd2db623e731ae00], // {0x1.630030b3abp-2, -0x1.db623e731aep-45}
]);

#[cfg(test)]
mod tests {
    use super::log10;

    fn check(x: f64, expected_bits: u64) {
        let y = log10(x);
        assert_eq!(
            y.to_bits(),
            expected_bits,
            "log10({x:e}) = {y:e} ({:#018x}), expected {:e} ({expected_bits:#018x})",
            y.to_bits(),
            f64::from_bits(expected_bits),
        );
    }

    // The expected bits below are CORE-MATH's C `cr_log10` (commit 040ee482a8ca, gcc 11.4,
    // `-O2 -ffp-contract=off`) and, for every one, mpmath's log10 at 400 bits rounded to nearest
    // (`validation/probes/coremath-log10/`).

    #[test]
    fn exact_and_known_values() {
        check(1.0, 0x0000000000000000); // +0, not -0
        for n in 0..=22 {
            let p: f64 = format!("1e{n}").parse().unwrap();
            check(p, f64::from(n).to_bits()); // the exact powers of ten
        }
        check(1e23, 0x4037000000000000); // not an exact power: 23 after rounding
        check(0.1, 0xbff0000000000000);
        check(2.0, 0x3fd34413509f79ff);
        check(0.5, 0xbfd34413509f79ff);
        check(3.0, 0x3fde8927964fd5fd);
        check(f64::MAX, 0x40734413509f79ff); // 308.25471555991675
        check(f64::MIN_POSITIVE, 0xc0733a7146f72a42); // -307.6526555685888
    }

    #[test]
    fn subnormal_inputs() {
        check(f64::from_bits(1), 0xc07434e6420f4374); // 2^-1074: -323.3062153431158
        check(f64::from_bits(2), 0xc07430153d3b1b95);
        check(f64::from_bits(0x000f_ffff_ffff_ffff), 0xc0733a7146f72a42);
        // hard-to-round subnormal cases of CORE-MATH's log10.wc
        check(f64::from_bits(0x0000449fb5c8a96e), 0xc07356db4d19cab0);
        check(f64::from_bits(0x0001553c114cff2b), 0xc0734bb61e364c88);
        check(f64::from_bits(0x00027ac359afdc72), 0xc0734766019516fd);
        check(f64::from_bits(0x0004c6cf09a2382d), 0xc07342d7aed90170);
    }

    #[test]
    fn accurate_path_values() {
        // Inputs near 1 and hard-to-round cases of CORE-MATH's log10.wc fail the fast path's
        // rounding test.
        check(f64::from_bits(0x3ff0000000000001), 0x3c9bcb7b1526e50d); // 1 + 2^-52
        check(f64::from_bits(0x3fefffffffffffff), 0xbc8bcb7b1526e50f); // 1 - 2^-53
        check(f64::from_bits(0x75f8301ae420f027), 0x4070444f70bbe52f); // 0x1.8301ae420f027p+864
        check(f64::from_bits(0x274a4904906ba280), 0xc05dac3d8c2392ff);
        check(f64::from_bits(0x3b2c95548441d140), 0xc036ed64a501fa9f);
        check(f64::from_bits(0x4e5ca3d72b5c3051), 0x40515f58190a1c5a);
        check(f64::from_bits(0x5a7d3be570ec0a06), 0x405ff980cb4e6d32);
        check(f64::from_bits(0x70fc70a7f32e1c51), 0x406d883c062f28fd);
        check(f64::from_bits(0x7500520cfddbe80c), 0x406ff2a8978f2a03);
    }

    #[test]
    fn special_values() {
        assert_eq!(log10(0.0), f64::NEG_INFINITY);
        assert_eq!(log10(-0.0), f64::NEG_INFINITY);
        assert_eq!(log10(f64::INFINITY), f64::INFINITY);
        // x < 0 and -inf: the positive quiet NaN (glibc's too)
        for x in [-1.0, -f64::from_bits(1), -f64::MAX, f64::NEG_INFINITY] {
            assert_eq!(log10(x).to_bits(), 0x7ff8_0000_0000_0000, "{x:e}");
        }
        // a NaN comes back quieted, sign and payload kept (glibc's too)
        for (x, y) in [
            (0x7ff8_0000_0000_0000u64, 0x7ff8_0000_0000_0000u64),
            (0xfff8_0000_0000_0000, 0xfff8_0000_0000_0000),
            (0x7ff0_0000_0000_0001, 0x7ff8_0000_0000_0001),
            (0xfff0_0000_0000_0123, 0xfff8_0000_0000_0123),
        ] {
            assert_eq!(log10(f64::from_bits(x)).to_bits(), y, "{x:#018x}");
            assert_eq!(super::super::log10_glibc(f64::from_bits(x)).to_bits(), y);
        }
    }

    /// FNV-1a of the result bits on `log10_glibc`'s seeded arguments (65,536: uniform on
    /// (0, 1000), random bit patterns, subnormal numbers and zero), the C's hash with its NaN
    /// for `x < 0` written as larmorx's `0x7ff8...`: every platform must give it.
    #[test]
    fn seeded_hash() {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut s = 0x1234_5678_9abc_def0u64;
        for i in 0..65_536u64 {
            s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            let r = z ^ (z >> 31);
            let x = match i % 3 {
                0 => (r >> 11) as f64 / (1u64 << 53) as f64 * 1e3,
                1 => f64::from_bits(r),
                _ => f64::from_bits(r & 0x000f_ffff_ffff_ffff),
            };
            h = (h ^ log10(x).to_bits()).wrapping_mul(0x0100_0000_01b3);
        }
        assert_eq!(h, 0xb615_169b_046f_3d5a);
    }

    /// The same hash over 50,001 arguments next to 1, where the accurate path runs most:
    /// `1 + k ulp` and `1 - (k + 1) ulp/2` for `k < 20,000`, `1 + k 2^-30` for `|k| <= 5,000`.
    #[test]
    fn near_one_hash() {
        let fnv = |h: u64, v: f64| (h ^ v.to_bits()).wrapping_mul(0x0100_0000_01b3);
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for k in 0..20_000u64 {
            h = fnv(h, log10(f64::from_bits(0x3ff0_0000_0000_0000 + k)));
            h = fnv(h, log10(f64::from_bits(0x3fef_ffff_ffff_ffff - k)));
        }
        for k in -5_000i32..=5_000 {
            h = fnv(
                h,
                log10(1.0 + f64::from(k) * f64::from_bits(0x3e10_0000_0000_0000)),
            );
        }
        assert_eq!(h, 0xe539_b917_b7ca_6767);
    }
}
