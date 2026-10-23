// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded cube root for binary32 (CORE-MATH's `cr_cbrtf`), in pure Rust.
//!
//! glibc's `cbrtf` is not correctly rounded on some arguments; this one is, on every input
//! (3dUnifize's `-EPI` radius, `specs/3dUnifize.md` §7).
//!
//! Port of CORE-MATH's `src/binary32/cbrt/cbrtf.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), MIT licence, Copyright (c) 2022-2025 Alexei
//! Sibidanov (the copyright and permission notice are reproduced below). The operations,
//! their order and the integer manipulations of the bit patterns are kept exactly (C's
//! unsigned wrap-around as wrapping arithmetic); hex-float constants are written as their exact
//! bit patterns, each with the original literal in a comment. The inexact-flag handling is not
//! modelled. Checked against the C function (compiled without fused multiply-adds) on every
//! one of the 2^32 floats.

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

#![allow(clippy::unreadable_literal, clippy::manual_range_contains)]

const fn fb(bits: u64) -> f64 {
    f64::from_bits(bits)
}

/// 1, 2^(1/3), 2^(2/3), as bit patterns.
const ESCALE: [u64; 3] = [
    0x3ff0000000000000, // 1.0
    0x3ff428a2f98d728b, // 0x1.428a2f98d728bp+0
    0x3ff965fea53d6e3d, // 0x1.965fea53d6e3dp+0
];

const C: [f64; 8] = [
    fb(0x3fe2319d352ea5d5), // 0x1.2319d352ea5d5p-1
    fb(0x3fe67ad8ee258d1a), // 0x1.67ad8ee258d1ap-1
    fb(0xbfd9342edf9cbad9), // -0x1.9342edf9cbad9p-2
    fb(0x3fcb6388fc510a75), // 0x1.b6388fc510a75p-3
    fb(0xbfb6002455599e2f), // -0x1.6002455599e2fp-4
    fb(0x3f97b096936192c4), // 0x1.7b096936192c4p-6
    fb(0xbf6e5577187e8bf8), // -0x1.e5577187e8bf8p-9
    fb(0x3f3169ef81d6c34e), // 0x1.169ef81d6c34ep-12
];

/// The cube root of `x` for `f32`, correctly rounded (CORE-MATH's `cr_cbrtf`).
#[must_use]
pub fn cbrtf(x: f32) -> f32 {
    let u = x.to_bits();
    let mut au = u << 1;
    let sgn = u >> 31;
    let mut e = au >> 24;
    if au < 1 << 24 || au >= 0xff << 24 {
        if au >= 0xff << 24 {
            return x + x; // inf, nan
        }
        if au == 0 {
            return x; // ±0
        }
        let nz = au.leading_zeros() as i32 - 7; // subnormal
        au <<= nz;
        e = e.wrapping_sub((nz - 1) as u32);
    }
    let mant = au & 0xffffff;
    let mut cvt1 = f64::from_bits((u64::from(mant) << 28) | (0x3ffu64 << 52));
    e = e.wrapping_add(899);
    let et = e / 3;
    let it = e % 3;
    let mut isc = ESCALE[it as usize];
    isc = isc.wrapping_add(u64::from(et.wrapping_sub(342)) << 52);
    isc |= u64::from(sgn) << 63;
    let cvt2 = f64::from_bits(isc);
    let z = cvt1;
    let r0 = fb(0xbf99931c6c2d19d1) / z; // -0x1.9931c6c2d19d1p-6
    let z2 = z * z;
    let z4 = z2 * z2;
    let mut f = ((C[0] + z * C[1]) + z2 * (C[2] + z * C[3]))
        + z4 * ((C[4] + z * C[5]) + z2 * (C[6] + z * C[7]))
        + r0;
    let mut r = f * cvt2;
    let mut ub = r as f32;
    let lb = (r - cvt2 * fb(0x3e185d4f3f7556ec)) as f32; // 1.4182e-9
    if ub == lb {
        return ub;
    }
    let u0 = fb(0xc02ab16ec65d138f); // -0x1.ab16ec65d138fp+3
    let h = f * f * f - z;
    f -= (f * r0 * u0) * h;
    r = f * cvt2;
    cvt1 = r;
    ub = r as f32;
    let m0 = (cvt1.to_bits() << 19) as i64;
    let m1 = m0 >> 63;
    if (m0 ^ m1) < (1i64 << 31) {
        let bits = cvt1.to_bits().wrapping_add(1u64 << 31) & 0xffffffff00000000;
        ub = f64::from_bits(bits) as f32;
    }
    ub
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::logf::tests::checksum;

    /// Bit-identical to CORE-MATH's C code (gcc 11.4, `-O2 -ffp-contract=off`) on every 4099th
    /// bit pattern.
    #[test]
    fn matches_the_c_code_on_a_stride() {
        assert_eq!(checksum(cbrtf, 4099), 0xd8db_7e18_6a25_ff90);
    }

    /// Every one of the 2^32 inputs (`cargo test --release -- --ignored`).
    #[test]
    #[ignore]
    fn matches_the_c_code_everywhere() {
        assert_eq!(checksum(cbrtf, 1), 0x3095_6ee9_e157_9229);
    }

    #[test]
    fn special_values() {
        assert_eq!(cbrtf(0.0).to_bits(), 0);
        assert_eq!(cbrtf(-0.0).to_bits(), (-0.0f32).to_bits());
        assert_eq!(cbrtf(f32::INFINITY), f32::INFINITY);
        assert!(cbrtf(f32::NAN).is_nan());
        assert_eq!(cbrtf(-8.0), -2.0);
    }

    /// Values computed with mpmath at 60 digits, rounded to the nearest float.
    #[test]
    fn against_mpmath() {
        for (x, bits) in [
            (2.0f32, 0x3fa14518u32),
            (3.0, 0x3fb89ba2),
            (27.0, 0x40400000),
            (0.001, 0x3dcccccd),
            (64.0, 0x40800000),
            (5.5, 0x3fe1f13a),
        ] {
            assert_eq!(cbrtf(x).to_bits(), bits, "{x}");
        }
    }
}
