// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded hyperbolic tangent for binary32 (CORE-MATH's `cr_tanhf`), in pure Rust.
//!
//! glibc 2.35's `tanhf` is not correctly rounded on some arguments (about 2.8 % of the floats
//! 3dUnifize's squash gives it, `specs/3dUnifize.md` §10); this one is, on every input, so it
//! gives the same bits on every platform.
//!
//! Port of CORE-MATH's `src/binary32/tanh/tanhf.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), MIT licence, Copyright (c) 2022-2025 Alexei
//! Sibidanov (the copyright and permission notice are reproduced below). The operations and
//! their order are kept exactly; `__builtin_fmaf` is [`f32::mul_add`]; hex-float constants are
//! written as their exact bit patterns, each with the original literal in a comment. Errno and
//! the exception flags are not modelled. Checked against the C function (compiled without
//! fused multiply-adds) on every one of the 2^32 floats.

// Copyright (c) 2022-2025 Alexei Sibidanov.
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

const CN: [f64; 8] = [
    fb(0x3ff0000000000000), // 0x1p+0
    fb(0x3fc30877b8b72d33), // 0x1.30877b8b72d33p-3
    fb(0x3f7694aa09ae9e5e), // 0x1.694aa09ae9e5ep-8
    fb(0x3f14101377abb729), // 0x1.4101377abb729p-14
    fb(0x3e9e0392b1db0018), // 0x1.e0392b1db0018p-22
    fb(0x3e12533756e546f7), // 0x1.2533756e546f7p-30
    fb(0x3d6d62e5abe6ae8a), // 0x1.d62e5abe6ae8ap-41
    fb(0x3c9b06be534182de), // 0x1.b06be534182dep-54
];

const CD: [f64; 8] = [
    fb(0x3ff0000000000000), // 0x1p+0
    fb(0x3fded99131b0ebea), // 0x1.ed99131b0ebeap-2
    fb(0x3fa0d27ed6c95a69), // 0x1.0d27ed6c95a69p-5
    fb(0x3f47cbdaca0e9fcc), // 0x1.7cbdaca0e9fccp-11
    fb(0x3edb4e60b892578e), // 0x1.b4e60b892578ep-18
    fb(0x3e5a6f707c5c71ab), // 0x1.a6f707c5c71abp-26
    fb(0x3dc35a8b6e2cd94c), // 0x1.35a8b6e2cd94cp-35
    fb(0x3d0ca8230677aa01), // 0x1.ca8230677aa01p-47
];

/// `tanh(x)` for `f32`, correctly rounded (CORE-MATH's `cr_tanhf`).
#[must_use]
pub fn tanhf(x: f32) -> f32 {
    let z = f64::from(x);
    let ux = x.to_bits();
    let e = (ux >> 23) & 0xff;
    if e == 0xff {
        if ux << 9 != 0 {
            return x + x; // NaN
        }
        return if ux >> 31 == 0 { 1.0 } else { -1.0 }; // ±inf
    }
    if e < 115 {
        // |x| < 2^-13
        if e < 102 {
            // |x| < 2^-26
            if ux << 1 == 0 {
                return x;
            }
            return (-x).mul_add(x.abs(), x);
        }
        let x2 = x * x;
        // -0x1.555556p-2f
        let c = -f32::from_bits(0x3eaaaaab);
        return x.mul_add(c * x2, x);
    }
    if (ux << 1) > (0x41102cb3u32 << 1) {
        // 0x1p-25f
        return 1.0f32.copysign(x) - f32::from_bits(0x33000000).copysign(x);
    }
    let z2 = z * z;
    let z4 = z2 * z2;
    let z8 = z4 * z4;
    let mut n0 = CN[0] + z2 * CN[1];
    let n2 = CN[2] + z2 * CN[3];
    let mut n4 = CN[4] + z2 * CN[5];
    let n6 = CN[6] + z2 * CN[7];
    n0 += z4 * n2;
    n4 += z4 * n6;
    n0 += z8 * n4;
    let mut d0 = CD[0] + z2 * CD[1];
    let d2 = CD[2] + z2 * CD[3];
    let mut d4 = CD[4] + z2 * CD[5];
    let d6 = CD[6] + z2 * CD[7];
    d0 += z4 * d2;
    d4 += z4 * d6;
    d0 += z8 * d4;
    let r = z * n0 / d0;
    r as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::logf::tests::checksum;

    /// Bit-identical to CORE-MATH's C code (gcc 11.4, `-O2 -ffp-contract=off`) on every 4099th
    /// bit pattern.
    #[test]
    fn matches_the_c_code_on_a_stride() {
        assert_eq!(checksum(tanhf, 4099), 0x5d9a_7b84_92e5_ef7b);
    }

    /// Every one of the 2^32 inputs (`cargo test --release -- --ignored`).
    #[test]
    #[ignore]
    fn matches_the_c_code_everywhere() {
        assert_eq!(checksum(tanhf, 1), 0x9363_b80d_987f_3611);
    }

    #[test]
    fn special_values() {
        assert_eq!(tanhf(0.0).to_bits(), 0);
        assert_eq!(tanhf(-0.0).to_bits(), (-0.0f32).to_bits());
        assert_eq!(tanhf(f32::INFINITY), 1.0);
        assert_eq!(tanhf(f32::NEG_INFINITY), -1.0);
        assert!(tanhf(f32::NAN).is_nan());
        assert_eq!(tanhf(20.0), 1.0);
        assert_eq!(tanhf(-20.0), -1.0);
    }

    /// Values computed with mpmath at 60 digits, rounded to the nearest float.
    #[test]
    fn against_mpmath() {
        for (x, bits) in [
            (0.5f32, 0x3eec9a9fu32),
            (1.0, 0x3f42f7d6),
            (2.0, 0x3f76ca83),
            (0.001, 0x3a83126c),
            (3.7, 0x3f7fafee),
        ] {
            assert_eq!(tanhf(x).to_bits(), bits, "{x}");
            assert_eq!(tanhf(-x).to_bits(), bits | 0x8000_0000, "{x}");
        }
    }
}
