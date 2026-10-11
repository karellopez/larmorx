// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded sine and cosine for binary32 (CORE-MATH's `cr_sinf` and `cr_cosf`), in
//! pure Rust.
//!
//! The results are `sin(x)` and `cos(x)` rounded to the nearest `f32` (ties to even) for every
//! input, so they are bit-identical on every platform. ITK calls them through
//! `std::sin(float)`/`std::cos(float)` (glibc's `sinf`, `cosf` and, where GCC pairs them,
//! `sincosf`), for example when `Euler3DTransform<float>` builds its rotation matrix under
//! `antsApplyTransforms --float 1`. glibc 2.35's versions are not correctly rounded for every
//! input (`docs/findings/platform-math.md`).
//!
//! Port of CORE-MATH's `src/binary32/sin/sinf.c` and `src/binary32/cos/cosf.c` at commit
//! 040ee482a8ca (<https://core-math.gitlabpages.inria.fr/>), MIT licence, Copyright (c)
//! 2022-2023 Alexei Sibidanov (the copyright and permission notice are reproduced below).
//! The algorithm, tables and operation order are kept exactly; it is re-expressed in Rust for
//! round-to-nearest only. Differences from the C files: floating-point exception flags and
//! `errno` are not modelled. Hex float literals are written as `f64` bit patterns, generated
//! from the C source, each with the original literal in a comment.

// Copyright (c) 2022-2023 Alexei Sibidanov.
//
// This file is part of the CORE-MATH project
// (https://core-math.gitlabpages.inria.fr/).
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

/// The bits of `2/π` used by the big-argument reduction (`ipi`, least significant first).
const IPI: [u64; 4] = [
    0xfe51_63ab_debb_c562,
    0xdb62_9599_3c43_9041,
    0xfc27_57d1_f534_ddc0,
    0xa2f9_836e_4e44_1529,
];

/// The reduction of `x` (as bits `u`) for `|x| ≥ 2^28` (`rbig`): returns `z` and the
/// quadrant-like index `q`, with `x·16/π = q + z` (mod 32).
fn rbig(u: u32) -> (f64, i32) {
    let e = ((u >> 23) & 0xff) as i32;
    let m = u64::from((u & (!0u32 >> 9)) | 1 << 23);
    let p0 = u128::from(m) * u128::from(IPI[0]);
    let mut p1 = u128::from(m) * u128::from(IPI[1]);
    p1 += p0 >> 64;
    let mut p2 = u128::from(m) * u128::from(IPI[2]);
    p2 += p1 >> 64;
    let mut p3 = u128::from(m) * u128::from(IPI[3]);
    p3 += p2 >> 64;
    let (p3h, p3l, p2l, p1l) = ((p3 >> 64) as u64, p3 as u64, p2 as u64, p1 as u64);
    let k = e - 124;
    let s = k - 23;
    // In cr_sinf()/cr_cosf(), rbig() is called with 155 <= e <= 254, so 5 <= s <= 104.
    let (mut i, a): (i32, i64) = if s < 64 {
        (
            ((p3h << s) | (p3l >> (64 - s))) as i32,
            ((p3l << s) | (p2l >> (64 - s))) as i64,
        )
    } else if s == 64 {
        (p3l as i32, p2l as i64)
    } else {
        (
            ((p3l << (s - 64)) | (p2l >> (128 - s))) as i32,
            ((p2l << (s - 64)) | (p1l >> (128 - s))) as i64,
        )
    };
    let sgn = (u as i32) >> 31;
    let sm = a >> 63;
    i = i.wrapping_sub(sm as i32);
    let z = ((a ^ i64::from(sgn)) as f64) * f64::from_bits(0x3bf0_0000_0000_0000); // 0x1p-64
    i = (i ^ sgn).wrapping_sub(sgn);
    (z, i)
}

/// `0x1.8p52`: adding it to an integral double puts the integer in the low mantissa bits.
const SHIFT: f64 = 6_755_399_441_055_744.0;

/// The reduction for `2^-12 ≤ |x| < 2^28` beyond the first range (`rltl`).
#[inline]
fn rltl(z: f32) -> (f64, i32) {
    let x = f64::from(z);
    let idl = f64::from_bits(0xbe2b_1bbe_ad60_3d8b) * x; // -0x1.b1bbead603d8bp-29
    let idh = f64::from_bits(0x4014_5f30_6e00_0000) * x; // 0x1.45f306ep+2
    let id = idh.round_ties_even();
    let q = (SHIFT + id).to_bits() as i32;
    ((idh - id) + idl, q)
}

/// The reduction for small arguments (`rltl0`).
#[inline]
fn rltl0(x: f64) -> (f64, i32) {
    let idh = f64::from_bits(0x4014_5f30_6dc9_c883) * x; // 0x1.45f306dc9c883p+2
    let id = idh.round_ties_even();
    let q = (SHIFT + id).to_bits() as i32;
    (idh - id, q)
}

const B: [f64; 4] = [
    f64::from_bits(0x3f93_bd3c_c9be_45dc), // 0x1.3bd3cc9be45dcp-6
    f64::from_bits(0xbf10_3c1f_081b_0833), // -0x1.03c1f081b0833p-14
    f64::from_bits(0x3e75_5d3c_6fc9_ac1f), // 0x1.55d3c6fc9ac1fp-24
    f64::from_bits(0xbdce_1d3f_f281_b40d), // -0x1.e1d3ff281b40dp-35
];
const A: [f64; 4] = [
    f64::from_bits(0x3fc9_21fb_5444_2d17), // 0x1.921fb54442d17p-3
    f64::from_bits(0xbf54_abbc_e625_6a39), // -0x1.4abbce6256a39p-10
    f64::from_bits(0x3ec4_66bc_5a51_8c16), // 0x1.466bc5a518c16p-19
    f64::from_bits(0xbe23_2bdc_6107_4ff6), // -0x1.32bdc61074ff6p-29
];

// sin(k·π/16), k = 0..31 (sinf.c's `tb`).
const T1: f64 = f64::from_bits(0x3fc8_f8b8_3c69_a60b); // 0x1.8f8b83c69a60bp-3
const T2: f64 = f64::from_bits(0x3fd8_7de2_a6ae_a963); // 0x1.87de2a6aea963p-2
const T3: f64 = f64::from_bits(0x3fe1_c73b_39ae_68c8); // 0x1.1c73b39ae68c8p-1
const T4: f64 = f64::from_bits(0x3fe6_a09e_667f_3bcd); // 0x1.6a09e667f3bcdp-1
const T5: f64 = f64::from_bits(0x3fea_9b66_290e_a1a3); // 0x1.a9b66290ea1a3p-1
const T6: f64 = f64::from_bits(0x3fed_906b_cf32_8d46); // 0x1.d906bcf328d46p-1
const T7: f64 = f64::from_bits(0x3fef_6297_cff7_5cb0); // 0x1.f6297cff75cbp-1
const SIN_TB: [f64; 32] = [
    0.0, T1, T2, T3, T4, T5, T6, T7, 1.0, T7, T6, T5, T4, T3, T2, T1, 0.0, -T1, -T2, -T3, -T4, -T5,
    -T6, -T7, -1.0, -T7, -T6, -T5, -T4, -T3, -T2, -T1,
];
// cos(k·π/16), k = 0..31 (cosf.c's `tb`).
const COS_TB: [f64; 32] = [
    1.0, T7, T6, T5, T4, T3, T2, T1, 0.0, -T1, -T2, -T3, -T4, -T5, -T6, -T7, -1.0, -T7, -T6, -T5,
    -T4, -T3, -T2, -T1, 0.0, T1, T2, T3, T4, T5, T6, T7,
];

/// The two polynomials of both kernels: `(aa, bb)` at `z`.
#[inline]
fn polys(z2: f64, z4: f64) -> (f64, f64) {
    let aa = (A[0] + z2 * A[1]) + z4 * (A[2] + z2 * A[3]);
    let bb = (B[0] + z2 * B[1]) + z4 * (B[2] + z2 * B[3]);
    (aa, bb)
}

/// `-0x1.555556p-3f`, the cubic term for `2^-25 <= abs(x) < 2^-12`.
const SINF_SMALL: f32 = f32::from_bits(0xbe2a_aaab); // -0x1.555556p-3f

/// Hard cases of `sinf` (`as_sinf_database`): `(|x| bits, rh, rl)`.
const SINF_DB: [(u32, f32, f32); 4] = [
    (
        0x4619_9998,
        f32::from_bits(0xbeb1_fa5d),
        f32::from_bits(0xb200_0000),
    ), // 0x1.33333p+13: -0x1.63f4bap-2, -0x1p-27
    (
        0x3f3a_dc51,
        f32::from_bits(0x3f2a_b445),
        f32::from_bits(0xb280_0000),
    ), // 0x1.75b8a2p-1: 0x1.55688ap-1, -0x1p-26
    (
        0x3fa7_832a,
        f32::from_bits(0x3f77_41b6),
        f32::from_bits(0xb280_0000),
    ), // 0x1.4f0654p+0: 0x1.ee836cp-1, -0x1p-26
    (
        0x4116_cbe4,
        f32::from_bits(0xb2cc_de2d),
        f32::from_bits(0xa600_0000),
    ), // 0x1.2d97c8p+3: -0x1.99bc5ap-26, -0x1p-51
];

#[cold]
fn sinf_database(x: f32, r: f64) -> f32 {
    let ax = x.to_bits() & (!0u32 >> 1);
    for &(arg, rh, rl) in &SINF_DB {
        if arg == ax {
            let sgn = 1.0f32.copysign(x);
            return sgn * rh + sgn * rl;
        }
    }
    r as f32
}

#[cold]
fn sinf_big(x: f32) -> f32 {
    let u = x.to_bits();
    let ax = u << 1;
    if ax >= 0xff << 24 {
        // NaN or ±inf
        return if ax << 8 != 0 { x + x } else { f32::NAN };
    }
    let (z, ia) = rbig(u);
    let z2 = z * z;
    let z4 = z2 * z2;
    let (aa, bb) = polys(z2, z4);
    let s0 = SIN_TB[(ia & 31) as usize];
    let c0 = SIN_TB[((ia as u32).wrapping_add(8) & 31) as usize];
    let r = s0 + z * (aa * c0 - bb * (z * s0));
    r as f32
}

/// `sin(x)` correctly rounded to `f32` (CORE-MATH's `cr_sinf`).
#[must_use]
pub fn sinf(x: f32) -> f32 {
    let u = x.to_bits();
    let ax = u << 1;
    let z0 = f64::from(x);
    if !(0x7300_0000..=0x9900_0000).contains(&ax) {
        // |x| > 0x1p+26 or |x| < 0x1p-12
        if ax < 0x7300_0000 {
            if ax < 0x6600_0000 {
                // |x| < 0x1p-25
                if ax == 0 {
                    return x;
                }
                return (-x).mul_add(x.abs(), x);
            }
            return (SINF_SMALL * x) * (x * x) + x;
        }
        return sinf_big(x);
    }
    let (z, ia) = if ax < 0x822d_97c8 {
        if ax == 0x7e75_b8a2 || ax == 0x7f4f_0654 {
            return sinf_database(x, 0.0);
        }
        rltl0(z0)
    } else {
        if ax == 0x8c33_3330 {
            return sinf_database(x, 0.0);
        }
        rltl(x)
    };
    let z2 = z * z;
    let z4 = z2 * z2;
    let (aa, bb) = polys(z2, z4);
    let s0 = SIN_TB[(ia & 31) as usize];
    let c0 = SIN_TB[(ia.wrapping_add(8) & 31) as usize];
    let r = s0 + aa * (z * c0) - bb * (z2 * s0);
    r as f32
}

/// Hard cases of `cosf` (`as_cosf_database`): `(|x| bits, rh, rl)`.
const COSF_DB: [(u32, f32, f32); 5] = [
    (
        0x4096_cbe4,
        f32::from_bits(0x324c_de2e),
        f32::from_bits(0xa580_0000),
    ), // 0x1.2d97c8p+2: 0x1.99bc5cp-27, -0x1p-52
    (
        0x5922_aa80,
        f32::from_bits(0x3f08_aebf),
        f32::from_bits(0xb280_0000),
    ), // 0x1.4555p+51: 0x1.115d7ep-1, -0x1p-26
    (
        0x5aa4_542c,
        f32::from_bits(0x3efa_40a4),
        f32::from_bits(0x3200_0000),
    ), // 0x1.48a858p+54: 0x1.f48148p-2, 0x1p-27
    (
        0x5f18_b878,
        f32::from_bits(0x3f7f_14bb),
        f32::from_bits(0x3280_0000),
    ), // 0x1.3170fp+63: 0x1.fe2976p-1, 0x1p-26
    (
        0x6115_cb11,
        f32::from_bits(0x3f78_142f),
        f32::from_bits(0xb280_0000),
    ), // 0x1.2b9622p+67: 0x1.f0285ep-1, -0x1p-26
];

#[cold]
fn cosf_database(x: f32, r: f64) -> f32 {
    let ax = x.to_bits() & (!0u32 >> 1);
    for &(arg, rh, rl) in &COSF_DB {
        if arg == ax {
            return rh + rl;
        }
    }
    r as f32
}

#[cold]
fn cosf_big(x: f32) -> f32 {
    let u = x.to_bits();
    let ax = u << 1;
    if ax >= 0xff << 24 {
        // NaN or ±inf
        return if ax << 8 != 0 { x + x } else { f32::NAN };
    }
    let (z, ia) = rbig(u);
    let z2 = z * z;
    let z4 = z2 * z2;
    let (aa, bb) = polys(z2, z4);
    let s0 = COS_TB[((ia as u32).wrapping_add(8) & 31) as usize];
    let c0 = COS_TB[(ia & 31) as usize];
    let r = c0 + z * (aa * s0 - bb * (z * c0));
    let tail = r.to_bits().wrapping_add(6) & (!0u64 >> 36);
    if tail <= 12 {
        return cosf_database(x, r);
    }
    r as f32
}

/// `cos(x)` correctly rounded to `f32` (CORE-MATH's `cr_cosf`).
#[must_use]
pub fn cosf(x: f32) -> f32 {
    let u = x.to_bits();
    let ax = u << 1;
    let z0 = f64::from(x);
    if !(0x7400_0000..=0x9900_0000).contains(&ax) {
        // |x| > 0x1p+26 or |x| < 0x1p-11
        if ax < 0x7400_0000 {
            // For |x| < 2^-11, cos(x) rounds to the same value as 1 - x²/2.
            return 0.5 * x.mul_add(-x, 2.0);
        }
        return cosf_big(x);
    }
    let (z, ia) = if ax < 0x82a4_1896 {
        if ax == 0x812d_97c8 {
            return cosf_database(x, 0.0);
        }
        rltl0(z0)
    } else {
        rltl(x)
    };
    let z2 = z * z;
    let z4 = z2 * z2;
    let (aa, bb) = polys(z2, z4);
    let c0 = COS_TB[(ia & 31) as usize];
    let s0 = COS_TB[(ia.wrapping_add(8) & 31) as usize];
    let r = c0 + aa * (z * s0) - bb * (z2 * c0);
    r as f32
}

#[cfg(test)]
mod tests {
    use super::{cosf, sinf};
    use crate::math::logf::tests::checksum;

    #[test]
    fn special_values() {
        assert_eq!(sinf(0.0).to_bits(), 0);
        assert_eq!(sinf(-0.0).to_bits(), (-0.0f32).to_bits());
        assert_eq!(cosf(0.0), 1.0);
        assert!(sinf(f32::INFINITY).is_nan() && cosf(f32::NEG_INFINITY).is_nan());
        assert!(sinf(f32::NAN).is_nan() && cosf(f32::NAN).is_nan());
        assert_eq!(sinf(f32::MIN_POSITIVE), f32::MIN_POSITIVE);
    }

    /// Bit-identical to CORE-MATH's C code (gcc 11, -O2) on every 4099th bit pattern.
    #[test]
    fn matches_the_c_code_on_a_stride() {
        assert_eq!(checksum(sinf, 4099), SINF_4099);
        assert_eq!(checksum(cosf, 4099), COSF_4099);
    }

    /// Every one of the 2^32 inputs (`cargo test --release -- --ignored`).
    #[test]
    #[ignore]
    fn matches_the_c_code_everywhere() {
        assert_eq!(checksum(sinf, 1), SINF_ALL);
        assert_eq!(checksum(cosf, 1), COSF_ALL);
    }

    // From CORE-MATH's C code compiled by gcc 11.4 at -O2 (checksum as in logf.rs).
    const SINF_4099: u64 = 0xa567_10bb_4f18_8c28;
    const COSF_4099: u64 = 0xe220_e47a_8e0f_4f80;
    const SINF_ALL: u64 = 0xf759_2077_0869_47e5;
    const COSF_ALL: u64 = 0x8d60_31dd_c2ab_bc79;
}
