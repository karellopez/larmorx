// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded arc cosine for binary32 (CORE-MATH's `cr_acosf`), in pure Rust.
//!
//! glibc 2.35's `acosf` (fdlibm's, [`super::acosf_glibc`]) is not correctly rounded on some
//! arguments; this one is, on every input, so it gives the same bits on every platform. The
//! `double` arc cosine rounded to `float` is not always correctly rounded: on two of the
//! 2,130,706,434 floats in [-1, 1] its result lies exactly halfway between two floats and rounds
//! to the wrong one (this function's own exceptions, `0x328885a3` and `0x39826222`).
//!
//! Port of CORE-MATH's `src/binary32/acos/acosf.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), MIT licence, Copyright (c) 2023 Alexei Sibidanov
//! (the copyright and permission notice are reproduced below). The operations and their order
//! are kept exactly (the C has no fused multiply-adds); hex-float constants are written as their
//! exact bit patterns, each with the original literal in a comment. Errno and the exception
//! flags are not modelled; an argument outside [-1, 1] gives the positive quiet NaN (the C's
//! `0.0f/0.0f`, whose sign is the processor's), a NaN argument itself quieted. Checked against
//! the C function (compiled without fused multiply-adds) on every one of the 2^32 floats
//! (`validation/investigations/coremath-acosf/`).

// Copyright (c) 2023 Alexei Sibidanov.
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

#![allow(clippy::unreadable_literal)]

const fn fb(bits: u64) -> f64 {
    f64::from_bits(bits)
}

const fn ff(bits: u32) -> f32 {
    f32::from_bits(bits)
}

/// `pi2` of acosf.c: 0x1.921fb54442d18p+0, π/2 rounded.
const PI2: f64 = fb(0x3ff921fb54442d18);

/// `o` of acosf.c: 0 and 0x1.921fb54442d18p+1 (π rounded).
const O: [f64; 2] = [0.0, fb(0x400921fb54442d18)];

/// `b` of acosf.c: the fast path's odd polynomial for |x| < 0x1.c2a1dcp-1.
const B: [f64; 16] = [
    fb(0x3fefffffffd9ccb8), // 0x1.fffffffd9ccb8p-1
    fb(0x3fc5555c94838007), // 0x1.5555c94838007p-3
    fb(0x3fb32ded4b7c20fa), // 0x1.32ded4b7c20fap-4
    fb(0x3fa8566df703309e), // 0x1.8566df703309ep-5
    fb(0xbf9980c959bec9a3), // -0x1.980c959bec9a3p-6
    fb(0x3fe56fbb04998344), // 0x1.56fbb04998344p-1
    fb(0xc01403d8e4c49f52), // -0x1.403d8e4c49f52p+2
    fb(0x403b06c3e9f311ea), // 0x1.b06c3e9f311eap+4
    fb(0xc059ea97c4e2c21f), // -0x1.9ea97c4e2c21fp+6
    fb(0x407200b8261cc61b), // 0x1.200b8261cc61bp+8
    fb(0xc082274c2799a5c7), // -0x1.2274c2799a5c7p+9
    fb(0x408a558a59cc19d3), // 0x1.a558a59cc19d3p+9
    fb(0xc08aca4b6a529ff0), // -0x1.aca4b6a529ffp+9
    fb(0x408228744703f813), // 0x1.228744703f813p+9
    fb(0xc06d7dbb0b322228), // -0x1.d7dbb0b322228p+7
    fb(0x4045c2018c0c0105), // 0x1.5c2018c0c0105p+5
];

/// `c` of acosf.c's accurate path for |x| < 0.5.
const C_SMALL: [f64; 12] = [
    fb(0x3fc555555555529c), // 0x1.555555555529cp-3
    fb(0x3fb333333337e0dd), // 0x1.333333337e0ddp-4
    fb(0x3fa6db6db3b4465e), // 0x1.6db6db3b4465ep-5
    fb(0x3f9f1c72e13ac306), // 0x1.f1c72e13ac306p-6
    fb(0x3f96e89cebe06bc4), // 0x1.6e89cebe06bc4p-6
    fb(0x3f91c6dcf5289094), // 0x1.1c6dcf5289094p-6
    fb(0x3f8c6dbbcc7c6315), // 0x1.c6dbbcc7c6315p-7
    fb(0x3f88f8dc2615e996), // 0x1.8f8dc2615e996p-7
    fb(0x3f7a5833b7bf15e8), // 0x1.a5833b7bf15e8p-8
    fb(0x3f943f44ace1665c), // 0x1.43f44ace1665cp-6
    fb(0xbf90fb17df881c73), // -0x1.0fb17df881c73p-6
    fb(0x3fa07520c026b2d6), // 0x1.07520c026b2d6p-5
];

/// `c` of acosf.c's accurate path for 0.5 <= |x| < 1.
const C_LARGE: [f64; 12] = [
    fb(0x3ff6a09e667f3bcb), // 0x1.6a09e667f3bcbp+0
    fb(0x3fbe2b7dddff2db9), // 0x1.e2b7dddff2db9p-4
    fb(0x3f9b27247ab42dbc), // 0x1.b27247ab42dbcp-6
    fb(0x3f802995cc4e0744), // 0x1.02995cc4e0744p-7
    fb(0x3f65ffb0276ec8ea), // 0x1.5ffb0276ec8eap-9
    fb(0x3f5033885a928dec), // 0x1.033885a928decp-10
    fb(0x3f3911f2be23f8c7), // 0x1.911f2be23f8c7p-12
    fb(0x3f24c3c55d2437fd), // 0x1.4c3c55d2437fdp-13
    fb(0x3f0af477e1d7b461), // 0x1.af477e1d7b461p-15
    fb(0x3f0abd6bdff67dcb), // 0x1.abd6bdff67dcbp-15
    fb(0xbef1717e86d0fa28), // -0x1.1717e86d0fa28p-16
    fb(0x3ef6ff526de46023), // 0x1.6ff526de46023p-16
];

/// `as_special` of acosf.c: |x| >= 1 or NaN.
#[cold]
#[inline(never)]
fn as_special(x: f32) -> f32 {
    let pih = ff(0x40490fdb); // 0x1.921fb6p+1f
    let pil = ff(0xb3800000); // -0x1p-24f
    let t = x.to_bits();
    if t == 0x7f << 23 {
        return 0.0; // x = 1
    }
    if t == 0x17f << 23 {
        return pih + pil; // x = -1
    }
    let ax = t << 1;
    if ax > 0xff << 24 {
        return x + x; // NaN
    }
    f32::NAN // |x| > 1 (the C's 0.0f/0.0f)
}

/// `poly12` of acosf.c.
#[inline(always)]
fn poly12(z: f64, c: &[f64; 12]) -> f64 {
    let z2 = z * z;
    let z4 = z2 * z2;
    let mut c0 = c[0] + z * c[1];
    let c2 = c[2] + z * c[3];
    let mut c4 = c[4] + z * c[5];
    let c6 = c[6] + z * c[7];
    let mut c8 = c[8] + z * c[9];
    let c10 = c[10] + z * c[11];
    c0 += c2 * z2;
    c4 += c6 * z2;
    c8 += z2 * c10;
    c0 += z4 * (c4 + z4 * c8);
    c0
}

/// The arc cosine of `x` for `f32`, correctly rounded (CORE-MATH's `cr_acosf`): in [0, π] for
/// `x` in [-1, 1], a NaN otherwise.
#[must_use]
pub fn acosf(x: f32) -> f32 {
    let xs = f64::from(x);
    let t = x.to_bits();
    let ax = t << 1;
    if ax >= 0x7f << 24 {
        return as_special(x);
    }
    if ax < 0x7ec2a1dc {
        // |x| < 0x1.c2a1dcp-1
        if ax < 0x4000_0000 {
            // |x| < 2^-63: avoid a spurious underflow
            let pi2h = ff(0x3fc90fdb); // 0x1.921fb6p+0f
            let pi2l = ff(0xb33bbd2e); // -0x1.777a5cp-25f
            return pi2h + pi2l;
        }
        let z = xs;
        let z2 = z * z;
        let z4 = z2 * z2;
        let z8 = z4 * z4;
        let z16 = z8 * z8;
        let r = z
            * ((((B[0] + z2 * B[1]) + z4 * (B[2] + z2 * B[3]))
                + z8 * ((B[4] + z2 * B[5]) + z4 * (B[6] + z2 * B[7])))
                + z16
                    * (((B[8] + z2 * B[9]) + z4 * (B[10] + z2 * B[11]))
                        + z8 * ((B[12] + z2 * B[13]) + z4 * (B[14] + z2 * B[15]))));
        let ub = (fb(0x3ff921fb54574191) - r) as f32; // 0x1.921fb54574191p+0
        let lb = (fb(0x3ff921fb543118a0) - r) as f32; // 0x1.921fb543118ap+0
        if ub == lb {
            return ub;
        }
    }
    // accurate path
    let r = if ax < 0x7e << 24 {
        // |x| < 0.5
        if t == 0x328885a3 {
            return ff(0x3fc90fdb) + ff(0x33000000); // 0x1.921fb6p+0f + 0x1p-25f
        }
        if t == 0x39826222 {
            return ff(0x3fc907b5) + ff(0x33000000); // 0x1.920f6ap+0f + 0x1p-25f
        }
        let x2 = xs * xs;
        (PI2 - xs) - (xs * x2) * poly12(x2, &C_SMALL)
    } else {
        let bx = xs.abs();
        let z = 1.0 - bx;
        let s = z.sqrt().copysign(xs);
        O[(t >> 31) as usize] + s * poly12(z, &C_LARGE)
    };
    r as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::logf::tests::checksum;

    /// Bit-identical to CORE-MATH's C code (gcc 11.4, `-O2 -ffp-contract=off`) on every 4099th
    /// bit pattern (`validation/investigations/coremath-acosf/`).
    #[test]
    fn matches_the_c_code_on_a_stride() {
        assert_eq!(checksum(acosf, 4099), 0x5875_2c47_1588_6fcd);
    }

    /// Every one of the 2^32 inputs (`cargo test --release -- --ignored`).
    #[test]
    #[ignore]
    fn matches_the_c_code_everywhere() {
        assert_eq!(checksum(acosf, 1), 0x2513_576b_a727_10ad);
    }

    /// The correctly rounded `double` arc cosine rounded to `float` gives the same bits but on
    /// the two double-rounding floats, on every 4099th float in [-1, 1] and near both of them.
    #[test]
    fn agrees_with_the_double_one_but_on_its_two_midpoints() {
        let check = |b: u32| {
            let x = f32::from_bits(b);
            let d = crate::math::acos(f64::from(x)) as f32;
            let r = acosf(x);
            match b {
                0x3288_85a3 => assert_eq!((r.to_bits(), d.to_bits()), (0x3fc9_0fdb, 0x3fc9_0fda)),
                0x3982_6222 => assert_eq!((r.to_bits(), d.to_bits()), (0x3fc9_07b5, 0x3fc9_07b4)),
                _ => assert_eq!(r.to_bits(), d.to_bits(), "{b:#010x}"),
            }
        };
        for b in (0..=0x3f80_0000u32).step_by(4099) {
            check(b);
            check(b | 0x8000_0000);
        }
        for c in [0x3288_85a3u32, 0x3982_6222] {
            for b in c - 2000..c + 2000 {
                check(b);
                check(b | 0x8000_0000);
            }
        }
    }

    #[test]
    fn special_values() {
        assert_eq!(acosf(1.0).to_bits(), 0);
        assert_eq!(acosf(-1.0).to_bits(), 0x4049_0fdb);
        assert_eq!(acosf(0.0).to_bits(), 0x3fc9_0fdb);
        assert_eq!(acosf(-0.0).to_bits(), 0x3fc9_0fdb);
        assert_eq!(acosf(f32::from_bits(1)).to_bits(), 0x3fc9_0fdb);
        assert_eq!(acosf(f32::from_bits(0x3f7f_ffff)).to_bits(), 0x39b5_04f3);
        assert_eq!(acosf(f32::from_bits(0xbf7f_ffff)).to_bits(), 0x4049_0a32);
        assert_eq!(acosf(1.5).to_bits(), 0x7fc0_0000);
        assert_eq!(acosf(f32::INFINITY).to_bits(), 0x7fc0_0000);
        assert_eq!(acosf(f32::NEG_INFINITY).to_bits(), 0x7fc0_0000);
        assert_eq!(acosf(f32::from_bits(0xffc0_0042)).to_bits(), 0xffc0_0042);
        assert!(acosf(f32::from_bits(0x7f80_0001)).is_nan());
    }

    /// Values computed with mpmath at 60 digits, rounded to the nearest float.
    #[test]
    fn against_mpmath() {
        for (x, bits) in [
            (0.5f32, 0x3f86_0a92u32),
            (-0.5, 0x4006_0a92),
            (0.25, 0x3fa8_b807),
            (-0.75, 0x401a_ce93), // glibc 2.35's acosf: 0x401ace94
            (0.05, 0x3fc2_a8c5),
        ] {
            assert_eq!(acosf(x).to_bits(), bits, "{x}");
        }
    }
}
