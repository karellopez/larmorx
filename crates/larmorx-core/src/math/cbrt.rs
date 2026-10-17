// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded cube root for binary64 (`cbrt`).
//!
//! Port of CORE-MATH `src/binary64/cbrt/cbrt.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), re-expressed in Rust.
//! Round-to-nearest only (`rm = 0`): the rounding-mode detection and the flag handling of
//! the C file are dropped, with the code that only directed roundings reach. The operations
//! and their order are kept exactly; `__builtin_fma` is [`f64::mul_add`], and hex-float
//! constants are written as their exact binary64 bit patterns.
//!
//! Original licence of cbrt.c (MIT):
//!
//! Copyright (c) 2021-2022 Alexei Sibidanov.
//!
//! This file is part of the CORE-MATH project
//! (<https://core-math.gitlabpages.inria.fr/>).
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

/// `f64` from its IEEE-754 bit pattern (constants are stored as exact bit patterns).
const fn fb(bits: u64) -> f64 {
    f64::from_bits(bits)
}

/// 1, 2^(1/3), 2^(2/3).
const ESCALE: [f64; 3] = [1.0, fb(0x3ff428a2f98d728b), fb(0x3ff965fea53d6e3d)];
/// `c0 + c1·x + c2·x² + c3·x³` approximates `x^(1/3)` on `[1, 2]`.
const C: [f64; 4] = [
    fb(0x3fe1b0babccfef9c),
    fb(0x3fe2c9a3e94d1da5),
    fb(0xbfc4dc30b1a1ddba),
    fb(0x3f97a8d3e4ec9b07),
];
const RSC: [f64; 6] = [1.0, -1.0, 0.5, -0.5, 0.25, -0.25];
/// `off[0]`, rounding to nearest.
const OFF0: f64 = fb(0x3ca0000000000000);

/// `cr_cbrt`.
#[inline(always)]
pub(crate) fn cbrt(x: f64) -> f64 {
    let u0 = fb(0x3fd5555555555555);
    let u1 = fb(0x3fcc71c71c71c71c);
    let hx = x.to_bits();
    let mut mant = hx & ((!0u64) >> 12);
    let sign = hx >> 63;
    let mut e = ((hx >> 52) & 0x7ff) as u32;
    if (e.wrapping_add(1) & 0x7ff) < 2 {
        let ix = hx & ((!0u64) >> 1);
        if e == 0x7ff || ix == 0 {
            return x + x; // 0, inf, nan
        }
        let nz = ix.leading_zeros() as i32 - 11; // subnormal
        mant <<= nz;
        mant &= (!0u64) >> 12;
        e = e.wrapping_sub((nz - 1) as u32);
    }
    e = e.wrapping_add(3072);
    let cvt1 = mant | (0x3ffu64 << 52);
    let mut cvt5 = cvt1;
    let et = e / 3;
    let it = e % 3;
    cvt5 = cvt5.wrapping_add(u64::from(it) << 52);
    cvt5 |= sign << 63;
    let zz = f64::from_bits(cvt5);
    let isc = ESCALE[it as usize].to_bits() | (sign << 63);
    let cvt2 = f64::from_bits(isc);
    let z = f64::from_bits(cvt1);
    let r = 1.0 / z;
    let rr = r * RSC[((it << 1) as u64 | sign) as usize];
    let z2 = z * z;
    let c0 = C[0] + z * C[1];
    let c2 = C[2] + z * C[3];
    let mut y = c0 + z2 * c2;
    let mut y2 = y * y;
    let mut h = y2 * (y * r) - 1.0;
    y -= (h * y) * (u0 - u1 * h);
    y *= cvt2;
    y2 = y * y;
    let mut y2l = y.mul_add(y, -y2);
    let mut y3 = y2 * y;
    let mut y3l = y.mul_add(y2, -y3) + y * y2l;
    h = ((y3 - zz) + y3l) * rr;
    let mut dy = h * (y * u0);
    let mut y1 = y - dy;
    dy = (y - y1) - dy;
    let mut ady = dy.abs();
    let mut ady0 = (ady - OFF0).abs();
    let mut ady1 = (ady - (fb(0x3cb0000000000000) + OFF0)).abs();
    if ady0 < fb(0x3b40000000000000) || ady1 < fb(0x3b40000000000000) {
        y2 = y1 * y1;
        y2l = y1.mul_add(y1, -y2);
        y3 = y2 * y1;
        y3l = y1.mul_add(y2, -y3) + y1 * y2l;
        h = ((y3 - zz) + y3l) * rr;
        dy = h * (y1 * u0);
        y = y1 - dy;
        dy = (y1 - y) - dy;
        y1 = y;
        ady = dy.abs();
        ady0 = (ady - OFF0).abs();
        ady1 = (ady - (fb(0x3cb0000000000000) + OFF0)).abs();
        if ady0 < fb(0x39d0000000000000) || ady1 < fb(0x39d0000000000000) {
            let azz = zz.abs();
            if azz == fb(0x4009b78223aa307c) {
                y1 = fb(0x3ff79d15d0e8d59c).copysign(zz);
            }
            if azz == fb(0x401a202bfc89ddff) {
                y1 = fb(0x3ffde87aa837820f).copysign(zz);
            }
        }
    }
    let mut cvt3 = y1.to_bits();
    cvt3 = cvt3.wrapping_add(u64::from(et.wrapping_sub(342 + 1023)) << 52);
    let m0 = (cvt3 << 30) as i64;
    let m1 = m0 >> 63;
    if ((m0 ^ m1) as u64) <= (1u64 << 30) {
        let cvt4 = (y1.to_bits().wrapping_add(1u64 << 15)) & 0xffffffffffff0000;
        if ((f64::from_bits(cvt4) - y1) - dy).abs() < fb(0x3c30000000000000) || zz.abs() == 1.0 {
            cvt3 = (cvt3.wrapping_add(1u64 << 15)) & 0xffffffffffff0000;
        }
    }
    f64::from_bits(cvt3)
}
