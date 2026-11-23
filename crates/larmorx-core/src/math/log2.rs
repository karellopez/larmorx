// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded base-2 logarithm of binary64 values: [`log2`].
//!
//! `log2(x)` is the binary64 value nearest to the exact log2 x (round-to-nearest, ties-to-even),
//! so the result is the same on every platform and equals any other correctly rounded `log2`.
//!
//! Port of CORE-MATH's `src/binary64/log2/log2.c` (`cr_log2`, `as_log2_refine`) at commit
//! 040ee482a8ca (MIT licence, © the CORE-MATH authors named below), re-expressed in Rust for
//! round-to-nearest only. The algorithm and the order of every floating-point operation are
//! CORE-MATH's:
//!
//! 1. An exact power of two `2^e` returns `e`.
//! 2. Fast path: `x = 2^e t` with `t` in [1, 2); a 10-bit index from `t`'s top mantissa bits
//!    (corrected by the table `B`) chooses two reciprocals `r1`, `r2` whose product brings `t`
//!    near 1/log(2); log2(t) is then `log2` of the two table entries (double-doubles `l1`, `l2`)
//!    plus a degree-5 polynomial of the remainder. If rounding `lh + ll ± 2.64e-22` gives the same
//!    double for both signs, that double is the correctly rounded result.
//! 3. Accurate path, when that test fails: four 4-bit table reductions (`t1` to `t4`, `LL`) and
//!    a double-double polynomial, with a final correction of the last bit where the result's
//!    trailing bits are all zero.
//!
//! Differences from the C code, none of which changes a non-NaN result in round-to-nearest:
//! `errno` handling is dropped; `__builtin_fma` is `f64::mul_add` (a correctly rounded fused
//! multiply-add on every target); hex-float constants are written as their exact bit patterns
//! (transcribed mechanically); a NaN argument gives itself quieted (sign and payload kept: what
//! the C's `x + x` gives on x86-64 and AArch64) and `x < 0` (including `-inf`) gives
//! `f64::NAN` (`0x7ff8...`), where the C's `0.0 / 0.0` returns the target's default NaN (whose
//! sign bit differs between x86-64 and AArch64), as `super::log` does; glibc 2.35's `log2` gives
//! SSE's default NaN `0xfff8...` there (`super::log2_glibc`).
//
// Original copyright and licence (MIT) notice of log2.c:
//
//   Copyright (c) 2023 Alexei Sibidanov.
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

#![allow(clippy::unreadable_literal, clippy::excessive_precision)]

use super::log::{f64_pair_table, f64_table};

/// Base-2 logarithm of `x`, correctly rounded to nearest (CORE-MATH's `cr_log2`).
///
/// Special values: `log2(±0) = -∞`, `log2(+∞) = +∞`, `log2(2^e) = e` exactly, `log2(x)` is
/// `f64::NAN` for `x < 0` (including `-∞`), and a NaN argument gives itself quieted.
pub fn log2(x: f64) -> f64 {
    let mut t = x.to_bits();
    let ex = (t >> 52) as i32;
    let mut e = ex - 0x3ff;
    if ex == 0 {
        // 0 or subnormal
        if t == 0 {
            // +0: C `-1.0 / 0.0`
            return f64::NEG_INFINITY;
        }
        let k = t.leading_zeros() as i32;
        e -= k - 12;
        t <<= k - 11;
    }
    if ex >= 0x7ff {
        if t << 1 == 0 {
            // -0
            return f64::NEG_INFINITY;
        }
        if (t << 1) > (0x7ffu64 << 53) {
            // NaN: C `x + x`, the NaN quieted
            return f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000);
        }
        if t >> 63 != 0 {
            // x < 0: C `0.0 / 0.0`
            return f64::NAN;
        }
        return x; // +inf
    }
    t &= !0u64 >> 12;
    if t == 0 {
        // C: `as_log2_exact(e)`
        return f64::from(e);
    }
    let ed = f64::from(e);
    let i = (t >> (52 - 5)) as usize;
    let d = (t & (!0u64 >> 17)) as i64;
    // C: `(t.u + ((u64)B[i].c0<<33) + ((int64_t)B[i].c1*(d>>16)))>>(52-10)`, unsigned wrapping
    let j = t
        .wrapping_add(u64::from(B[i].0) << 33)
        .wrapping_add((i64::from(B[i].1).wrapping_mul(d >> 16)) as u64)
        >> (52 - 10);
    t |= 0x3ffu64 << 52;
    let tf = f64::from_bits(t);
    let i1 = (j >> 5) as usize;
    let i2 = (j & 0x1f) as usize;
    const L2H: f64 = f64::from_bits(0x3ff7154800000000); // 0x1.71548p+0
    const L2L: f64 = f64::from_bits(0xbe9ad47a2f472159); // -0x1.ad47a2f472159p-22
    let r = R1[i1] * R2[i2];
    let o = r * tf;
    let dxl = r.mul_add(tf, -o);
    let dxh = o - L2H;
    let dx = dxh + dxl;
    let dx2 = dx * dx;
    let f = dx2 * ((C[0] + dx * C[1]) + dx2 * (C[2] + dx * C[3]));
    let lt = (L1[i1][1] + L2[i2][1]) + ed;
    let lh = lt + dxh;
    let mut ll = (lt - lh) + dxh;
    ll += ((L1[i1][0] + L2[i2][0]) + dxl) + dxh * L2L;
    ll += f;
    const EPS: f64 = 2.64e-22;
    let lb = lh + (ll - EPS);
    let ub = lh + (ll + EPS);
    if lb == ub {
        return lb;
    }
    log2_refine(x, ub)
}

/// C `fasttwosum(x, y, &e)`: `(s, e)`.
#[inline(always)]
fn fasttwosum(x: f64, y: f64) -> (f64, f64) {
    let s = x + y;
    let z = s - x;
    (s, y - z)
}

/// C `adddd(xh, xl, ch, cl, &l)`: `(s, l)`.
#[inline(always)]
fn adddd(xh: f64, xl: f64, ch: f64, cl: f64) -> (f64, f64) {
    let s = xh + ch;
    let d = s - xh;
    let l = ((ch - d) + (xh + (d - s))) + (xl + cl);
    (s, l)
}

/// C `muldd_acc(xh, xl, ch, cl, &l)`: `(h, l)`.
#[inline(always)]
fn muldd_acc(xh: f64, xl: f64, ch: f64, cl: f64) -> (f64, f64) {
    let ahlh = ch * xl;
    let alhh = cl * xh;
    let ahhh = ch * xh;
    let mut ahhl = ch.mul_add(xh, -ahhh);
    ahhl += alhh + ahlh;
    fasttwosum(ahhh, ahhl)
}

/// C `polydd(xh, xl, n, c, &l)` with `*l` in and out: `(h, l)`.
#[inline(always)]
fn polydd(xh: f64, xl: f64, c: &[[f64; 2]], l: f64) -> (f64, f64) {
    let mut i = c.len() - 1;
    let mut ch = c[i][0] + l;
    let mut cl = ((c[i][0] - ch) + l) + c[i][1];
    while i > 0 {
        i -= 1;
        let (h, lo) = muldd_acc(xh, xl, ch, cl);
        ch = h;
        cl = lo;
        let th = ch + c[i][0];
        let tl = (c[i][0] - th) + ch;
        ch = th;
        cl += tl + c[i][1];
    }
    (ch, cl)
}

/// C `as_log2_refine(x, a)`: the accurate path (`a` the fast path's upper bound).
#[inline(never)]
fn log2_refine(x: f64, a: f64) -> f64 {
    let mut t = x.to_bits();
    let ex = (t >> 52) as i32;
    let mut e = ex - 0x3ff;
    if ex == 0 {
        // subnormal
        let k = t.leading_zeros() as i32;
        e -= k - 12;
        t <<= k - 11;
    }
    t &= !0u64 >> 12;
    t |= 0x3ffu64 << 52;
    let tf = f64::from_bits(t);
    let ed = f64::from(e);
    let v = (a - ed + f64::from_bits(0x3ff0000800000000)).to_bits(); // 0x1.00008p+0
    let i = (v.wrapping_sub(0x3ffu64 << 52)) >> (52 - 16);
    let i1 = (i >> 12) as usize;
    let i2 = ((i >> 8) & 0xf) as usize;
    let i3 = ((i >> 4) & 0xf) as usize;
    let i4 = (i & 0xf) as usize;
    let l0 = LL[0][i1][0] + LL[1][i2][0] + (LL[2][i3][0] + LL[3][i4][0]) + ed;
    let l1 = LL[0][i1][1] + LL[1][i2][1] + (LL[2][i3][1] + LL[3][i4][1]);
    let l2 = LL[0][i1][2] + LL[1][i2][2] + (LL[2][i3][2] + LL[3][i4][2]);
    let t12 = T1[i1] * T2[i2];
    let t34 = T3[i3] * T4[i4];
    let th = t12 * t34;
    let tl = t12.mul_add(t34, -th);
    let dh = th * tf;
    let dl = th.mul_add(tf, -dh);
    let sh = tl * tf;
    let sl = tl.mul_add(tf, -sh);
    let (xh, xl) = fasttwosum(dh - 1.0, dl);
    let (xh, xl) = adddd(xh, xl, sh, sl);
    let sl = xh * (CL[0] + xh * (CL[1] + xh * CL[2]));
    let (sh, sl) = polydd(xh, xl, &CY, sl);
    let (sh, sl) = muldd_acc(xh, xl, sh, sl);
    let (sh, sl) = adddd(sh, sl, l1, l2);
    let (v0, v2) = fasttwosum(l0, sh);
    let (mut v1, v2) = fasttwosum(v2, sl);
    let mut tb = v1.to_bits();
    if tb & (!0u64 >> 12) == 0 {
        let w = v2.to_bits();
        if (w ^ tb) >> 63 != 0 {
            tb = tb.wrapping_sub(1);
        } else {
            tb = tb.wrapping_add(1);
        }
        v1 = f64::from_bits(tb);
    }
    v1 + v0
}

/// `B`: (`c0`, `c1`).
const B: [(u16, i16); 32] = [
    (301, 27565),
    (7189, 24786),
    (13383, 22167),
    (18923, 19696),
    (23845, 17361),
    (28184, 15150),
    (31969, 13054),
    (35231, 11064),
    (37996, 9173),
    (40288, 7372),
    (42129, 5657),
    (43542, 4020),
    (44546, 2457),
    (45160, 962),
    (45399, -468),
    (45281, -1838),
    (44821, -3151),
    (44032, -4412),
    (42929, -5622),
    (41522, -6786),
    (39825, -7905),
    (37848, -8982),
    (35602, -10020),
    (33097, -11020),
    (30341, -11985),
    (27345, -12916),
    (24115, -13816),
    (20661, -14685),
    (16989, -15526),
    (13107, -16339),
    (9022, -17126),
    (4740, -17889),
];

/// `r1`.
const R1: [f64; 33] = f64_table([
    0x3ff7154800000000, // 0x1.71548p+0
    0x3ff696af49200000, // 0x1.696af492p+0
    0x3ff61ab3fb680000, // 0x1.61ab3fb68p+0
    0x3ff5a18441680000, // 0x1.5a1844168p+0
    0x3ff52adadb480000, // 0x1.52adadb48p+0
    0x3ff4b6b7c9080000, // 0x1.4b6b7c908p+0
    0x3ff4451b0aa80000, // 0x1.4451b0aa8p+0
    0x3ff3d5ed8ae00000, // 0x1.3d5ed8aep+0
    0x3ff3691834680000, // 0x1.369183468p+0
    0x3ff2fe9b07400000, // 0x1.2fe9b074p+0
    0x3ff2967603680000, // 0x1.296760368p+0
    0x3ff2307afe500000, // 0x1.2307afe5p+0
    0x3ff1cca9f7f80000, // 0x1.1cca9f7f8p+0
    0x3ff16b02f0600000, // 0x1.16b02f06p+0
    0x3ff10b85e7880000, // 0x1.10b85e788p+0
    0x3ff0ae04b2e00000, // 0x1.0ae04b2ep+0
    0x3ff0527f52680000, // 0x1.0527f5268p+0
    0x3feff1eb8c400000, // 0x1.ff1eb8c4p-1
    0x3fef42a1f1800000, // 0x1.f42a1f18p-1
    0x3fee9721d4900000, // 0x1.e9721d49p-1
    0x3fedef6b35700000, // 0x1.def6b357p-1
    0x3fed4b21bf000000, // 0x1.d4b21bfp-1
    0x3fecaa739bd00000, // 0x1.caa739bdp-1
    0x3fec0d32a1500000, // 0x1.c0d32a15p-1
    0x3feb735ecf800000, // 0x1.b735ecf8p-1
    0x3feadcc9fbd00000, // 0x1.adcc9fbdp-1
    0x3fea497426400000, // 0x1.a4974264p-1
    0x3fe9b92f24400000, // 0x1.9b92f244p-1
    0x3fe92c2920600000, // 0x1.92c29206p-1
    0x3fe8a205c5800000, // 0x1.8a205c58p-1
    0x3fe81af33e300000, // 0x1.81af33e3p-1
    0x3fe796c35fe00000, // 0x1.796c35fep-1
    0x3fe7154800000000, // 0x1.71548p-1
]);

/// `r2`.
const R2: [f64; 33] = f64_table([
    0x3ff0000000000000, // 0x1p+0
    0x3feffa7000000000, // 0x1.ffa7p-1
    0x3feff4f000000000, // 0x1.ff4fp-1
    0x3fefef6000000000, // 0x1.fef6p-1
    0x3fefe9e000000000, // 0x1.fe9ep-1
    0x3fefe45000000000, // 0x1.fe45p-1
    0x3fefded000000000, // 0x1.fdedp-1
    0x3fefd94000000000, // 0x1.fd94p-1
    0x3fefd3c000000000, // 0x1.fd3cp-1
    0x3fefce4000000000, // 0x1.fce4p-1
    0x3fefc8c000000000, // 0x1.fc8cp-1
    0x3fefc34000000000, // 0x1.fc34p-1
    0x3fefbdc000000000, // 0x1.fbdcp-1
    0x3fefb84000000000, // 0x1.fb84p-1
    0x3fefb2c000000000, // 0x1.fb2cp-1
    0x3fefad4000000000, // 0x1.fad4p-1
    0x3fefa7c000000000, // 0x1.fa7cp-1
    0x3fefa24000000000, // 0x1.fa24p-1
    0x3fef9cd000000000, // 0x1.f9cdp-1
    0x3fef975000000000, // 0x1.f975p-1
    0x3fef91e000000000, // 0x1.f91ep-1
    0x3fef8c6000000000, // 0x1.f8c6p-1
    0x3fef86f000000000, // 0x1.f86fp-1
    0x3fef817000000000, // 0x1.f817p-1
    0x3fef7c0000000000, // 0x1.f7cp-1
    0x3fef769000000000, // 0x1.f769p-1
    0x3fef711000000000, // 0x1.f711p-1
    0x3fef6ba000000000, // 0x1.f6bap-1
    0x3fef663000000000, // 0x1.f663p-1
    0x3fef60c000000000, // 0x1.f60cp-1
    0x3fef5b5000000000, // 0x1.f5b5p-1
    0x3fef55e000000000, // 0x1.f55ep-1
    0x3fef507000000000, // 0x1.f507p-1
]);

/// `c`.
const C: [f64; 4] = f64_table([
    0xbfd62e41d56c6400, // -0x1.62e41d56c64p-2
    0x3fc47fd2632d2d32, // 0x1.47fd2632d2d32p-3
    0xbfb5504497831ba7, // -0x1.5504497831ba7p-4
    0x3fa7a3314c5bef3c, // 0x1.7a3314c5bef3cp-5
]);

/// `l1` (low, high as in the C).
const L1: [[f64; 2]; 33] = f64_pair_table([
    [0x0000000000000000, 0x0000000000000000], // {0x0p+0, 0x0p+0}
    [0x3e51435edc775b51, 0x3f9ffe3800000000], // {0x1.1435edc775b51p-26, 0x1.ffe38p-6}
    [0x3e5c8f1cbf9e4073, 0x3fb000bc00000000], // {0x1.c8f1cbf9e4073p-26, 0x1.000bcp-4}
    [0xbe57bf30fa53957b, 0x3fb7ff9400000000], // {-0x1.7bf30fa53957bp-26, 0x1.7ff94p-4}
    [0x3e4674d30b6276ed, 0x3fbfffa600000000], // {0x1.674d30b6276edp-27, 0x1.fffa6p-4}
    [0x3e399fcf0d796ace, 0x3fc4000580000000], // {0x1.99fcf0d796acep-28, 0x1.400058p-3}
    [0xbe55ffd8b92706d2, 0x3fc7ffe500000000], // {-0x1.5ffd8b92706d2p-26, 0x1.7ffe5p-3}
    [0x3e5bef90bc5a116d, 0x3fcbffc300000000], // {0x1.bef90bc5a116dp-26, 0x1.bffc3p-3}
    [0x3e5fca73b3d53f0d, 0x3fcffffd00000000], // {0x1.fca73b3d53f0dp-26, 0x1.ffffdp-3}
    [0xbe441024e560e04e, 0x3fd2000d00000000], // {-0x1.41024e560e04ep-27, 0x1.2000dp-2}
    [0x3e5484024fad8461, 0x3fd3ffcc00000000], // {0x1.484024fad8461p-26, 0x1.3ffccp-2}
    [0x3e52d9ab90ba7694, 0x3fd5ffe180000000], // {0x1.2d9ab90ba7694p-26, 0x1.5ffe18p-2}
    [0xbe52c998ea30ba7b, 0x3fd80013c0000000], // {-0x1.2c998ea30ba7bp-26, 0x1.80013cp-2}
    [0xbe475fc8682f918e, 0x3fda0024c0000000], // {-0x1.75fc8682f918ep-27, 0x1.a0024cp-2}
    [0xbe5f02f268a85fb8, 0x3fdbffd300000000], // {-0x1.f02f268a85fb8p-26, 0x1.bffd3p-2}
    [0xbe450030ea7fae4b, 0x3fddffd7c0000000], // {-0x1.50030ea7fae4bp-27, 0x1.dffd7cp-2}
    [0xbe3bffb8da5b849d, 0x3fdffff980000000], // {-0x1.bffb8da5b849dp-28, 0x1.ffff98p-2}
    [0x3e3f71993ff95475, 0x3fe0fffd40000000], // {0x1.f71993ff95475p-28, 0x1.0fffd4p-1}
    [0x3e288028e67f78fa, 0x3fe20010c0000000], // {0x1.88028e67f78fap-29, 0x1.20010cp-1}
    [0x3e5f915f5a0b4e89, 0x3fe30018a0000000], // {0x1.f915f5a0b4e89p-26, 0x1.30018ap-1}
    [0x3e1c2fe288f968f8, 0x3fe3fff480000000], // {0x1.c2fe288f968f8p-30, 0x1.3fff48p-1}
    [0x3e57375a75ae0837, 0x3fe5001300000000], // {0x1.7375a75ae0837p-26, 0x1.50013p-1}
    [0x3e291e48be920323, 0x3fe6000ea0000000], // {0x1.91e48be920323p-29, 0x1.6000eap-1}
    [0x3e2ee7bc0d39a3db, 0x3fe7001120000000], // {0x1.ee7bc0d39a3dbp-29, 0x1.700112p-1}
    [0xbe2899e2ac5f778c, 0x3fe7fffb40000000], // {-0x1.899e2ac5f778cp-29, 0x1.7fffb4p-1}
    [0x3e5f1f20176130a7, 0x3fe8fffae0000000], // {0x1.f1f20176130a7p-26, 0x1.8fffaep-1}
    [0x3e403fc59d34a4f3, 0x3fe9fff1e0000000], // {0x1.03fc59d34a4f3p-27, 0x1.9fff1ep-1}
    [0xbdf68722010e4653, 0x3feb0012a0000000], // {-0x1.68722010e4653p-32, 0x1.b0012ap-1}
    [0xbe558783d505a6ec, 0x3febffeb80000000], // {-0x1.58783d505a6ecp-26, 0x1.bffeb8p-1}
    [0x3e4b212ab9f8d51d, 0x3fed0004e0000000], // {0x1.b212ab9f8d51dp-27, 0x1.d0004ep-1}
    [0xbe5b3ecd767be776, 0x3fedffeb20000000], // {-0x1.b3ecd767be776p-26, 0x1.dffeb2p-1}
    [0x3e4b1ad41f07fc10, 0x3feeffd520000000], // {0x1.b1ad41f07fc1p-27, 0x1.effd52p-1}
    [0x0000000000000000, 0x3ff0000000000000], // {0x0p+0, 0x1p+0}
]);

/// `l2` (low, high as in the C).
const L2: [[f64; 2]; 33] = f64_pair_table([
    [0x0000000000000000, 0x0000000000000000], // {0x0p+0, 0x0p+0}
    [0xbe4e2b19f9c7b840, 0x3f500e4000000000], // {-0x1.e2b19f9c7b84p-27, 0x1.00e4p-10}
    [0xbe4b1d68137631fe, 0x3f5ff10000000000], // {-0x1.b1d68137631fep-27, 0x1.ff1p-10}
    [0xbe4aa92227513fc3, 0x3f68026000000000], // {-0x1.aa92227513fc3p-27, 0x1.8026p-9}
    [0xbe452918e3ab6f5e, 0x3f6ff68000000000], // {-0x1.52918e3ab6f5ep-27, 0x1.ff68p-9}
    [0x3e41a0b9b9010a9c, 0x3f74019000000000], // {0x1.1a0b9b9010a9cp-27, 0x1.4019p-8}
    [0x3e509b87f57867ec, 0x3f77fd0000000000], // {0x1.09b87f57867ecp-26, 0x1.7fdp-8}
    [0xbe33d6b70c673be6, 0x3f7c04c000000000], // {-0x1.3d6b70c673be6p-28, 0x1.c04cp-8}
    [0x3e476d340a6780ab, 0x3f8000c800000000], // {0x1.76d340a6780abp-27, 0x1.000c8p-7}
    [0x3e5e4181e37d9e05, 0x3f81ff8800000000], // {0x1.e4181e37d9e05p-26, 0x1.1ff88p-7}
    [0xbe48c181f042b901, 0x3f83fea800000000], // {-0x1.8c181f042b901p-27, 0x1.3fea8p-7}
    [0x3e49890ae7761d66, 0x3f85fe1800000000], // {0x1.9890ae7761d66p-27, 0x1.5fe18p-7}
    [0xbe555977ae613d5f, 0x3f87fde800000000], // {-0x1.55977ae613d5fp-26, 0x1.7fde8p-7}
    [0x3e4c75f49acf5e56, 0x3f89fe0800000000], // {0x1.c75f49acf5e56p-27, 0x1.9fe08p-7}
    [0xbe3e1fc84d0d42bc, 0x3f8bfe8800000000], // {-0x1.e1fc84d0d42bcp-28, 0x1.bfe88p-7}
    [0xbe55ab8a182ed279, 0x3f8dff6000000000], // {-0x1.5ab8a182ed279p-26, 0x1.dff6p-7}
    [0xbe5b389a7d8a21d9, 0x3f90004800000000], // {-0x1.b389a7d8a21d9p-26, 0x1.00048p-6}
    [0xbe57376f1a891fa5, 0x3f91010c00000000], // {-0x1.7376f1a891fa5p-26, 0x1.1010cp-6}
    [0xbe30915581b87a8a, 0x3f91ff1000000000], // {-0x1.0915581b87a8ap-28, 0x1.1ff1p-6}
    [0x3e4b7f2eaa894fca, 0x3f93002c00000000], // {0x1.b7f2eaa894fcap-27, 0x1.3002cp-6}
    [0xbe5f8c41bdd38c23, 0x3f93fe8c00000000], // {-0x1.f8c41bdd38c23p-26, 0x1.3fe8cp-6}
    [0x3e304ffb1f3c8215, 0x3f95000000000000], // {0x1.04ffb1f3c8215p-28, 0x1.5p-6}
    [0x3e5a398ccf2b137d, 0x3f95feb400000000], // {0x1.a398ccf2b137dp-26, 0x1.5feb4p-6}
    [0x3e5373b5bce07f39, 0x3f97008400000000], // {0x1.373b5bce07f39p-26, 0x1.70084p-6}
    [0xbe4e42040fd9d454, 0x3f97ff9400000000], // {-0x1.e42040fd9d454p-27, 0x1.7ff94p-6}
    [0x3e4ea00eb5770526, 0x3f98fecc00000000], // {0x1.ea00eb5770526p-27, 0x1.8feccp-6}
    [0xbe53e2002bb54bb8, 0x3f9a012400000000], // {-0x1.3e2002bb54bb8p-26, 0x1.a0124p-6}
    [0x3e5797ec835db8d3, 0x3f9b00b400000000], // {0x1.797ec835db8d3p-26, 0x1.b00b4p-6}
    [0x3e3bdcd45fedd285, 0x3f9c007400000000], // {0x1.bdcd45fedd285p-28, 0x1.c0074p-6}
    [0xbe333b8f09d1a210, 0x3f9d006000000000], // {-0x1.33b8f09d1a21p-28, 0x1.d006p-6}
    [0xbe458937a8eea36f, 0x3f9e007800000000], // {-0x1.58937a8eea36fp-27, 0x1.e0078p-6}
    [0xbe43e76203ff54cd, 0x3f9f00bc00000000], // {-0x1.3e76203ff54cdp-27, 0x1.f00bcp-6}
    [0xbe16560f19fc3f41, 0x3fa0009600000000], // {-0x1.6560f19fc3f41p-30, 0x1.00096p-5}
]);

/// `t1`.
const T1: [f64; 17] = f64_table([
    0x3ff0000000000000, // 0x1p+0
    0x3feea4afa0000000, // 0x1.ea4afap-1
    0x3fed5818e0000000, // 0x1.d5818ep-1
    0x3fec199be0000000, // 0x1.c199bep-1
    0x3feae89f98000000, // 0x1.ae89f98p-1
    0x3fe9c49180000000, // 0x1.9c4918p-1
    0x3fe8ace540000000, // 0x1.8ace54p-1
    0x3fe7a11470000000, // 0x1.7a1147p-1
    0x3fe6a09e68000000, // 0x1.6a09e68p-1
    0x3fe5ab07e0000000, // 0x1.5ab07ep-1
    0x3fe4bfdad8000000, // 0x1.4bfdad8p-1
    0x3fe3dea650000000, // 0x1.3dea65p-1
    0x3fe306fe08000000, // 0x1.306fe08p-1
    0x3fe2387a70000000, // 0x1.2387a7p-1
    0x3fe172b840000000, // 0x1.172b84p-1
    0x3fe0b55870000000, // 0x1.0b5587p-1
    0x3fe0000000000000, // 0x1p-1
]);

/// `t2`.
const T2: [f64; 16] = f64_table([
    0x3ff0000000000000, // 0x1p+0
    0x3fefe9d968000000, // 0x1.fe9d968p-1
    0x3fefd3c228000000, // 0x1.fd3c228p-1
    0x3fefbdba38000000, // 0x1.fbdba38p-1
    0x3fefa7c180000000, // 0x1.fa7c18p-1
    0x3fef91d800000000, // 0x1.f91d8p-1
    0x3fef7bfdb0000000, // 0x1.f7bfdbp-1
    0x3fef663278000000, // 0x1.f663278p-1
    0x3fef507658000000, // 0x1.f507658p-1
    0x3fef3ac948000000, // 0x1.f3ac948p-1
    0x3fef252b38000000, // 0x1.f252b38p-1
    0x3fef0f9c20000000, // 0x1.f0f9c2p-1
    0x3feefa1bf0000000, // 0x1.efa1bfp-1
    0x3feee4aaa0000000, // 0x1.ee4aaap-1
    0x3feecf4830000000, // 0x1.ecf483p-1
    0x3feeb9f488000000, // 0x1.eb9f488p-1
]);

/// `t3`.
const T3: [f64; 16] = f64_table([
    0x3ff0000000000000, // 0x1p+0
    0x3feffe9d20000000, // 0x1.ffe9d2p-1
    0x3feffd3a58000000, // 0x1.ffd3a58p-1
    0x3feffbd798000000, // 0x1.ffbd798p-1
    0x3feffa74e8000000, // 0x1.ffa74e8p-1
    0x3feff91248000000, // 0x1.ff91248p-1
    0x3feff7afb8000000, // 0x1.ff7afb8p-1
    0x3feff64d38000000, // 0x1.ff64d38p-1
    0x3feff4eac8000000, // 0x1.ff4eac8p-1
    0x3feff38868000000, // 0x1.ff38868p-1
    0x3feff22618000000, // 0x1.ff22618p-1
    0x3feff0c3d0000000, // 0x1.ff0c3dp-1
    0x3fefef61a0000000, // 0x1.fef61ap-1
    0x3fefedff78000000, // 0x1.fedff78p-1
    0x3fefec9d68000000, // 0x1.fec9d68p-1
    0x3fefeb3b60000000, // 0x1.feb3b6p-1
]);

/// `t4`.
const T4: [f64; 16] = f64_table([
    0x3ff0000000000000, // 0x1p+0
    0x3fefffe9d0000000, // 0x1.fffe9dp-1
    0x3fefffd3a0000000, // 0x1.fffd3ap-1
    0x3fefffbd78000000, // 0x1.fffbd78p-1
    0x3fefffa748000000, // 0x1.fffa748p-1
    0x3fefff9118000000, // 0x1.fff9118p-1
    0x3fefff7ae8000000, // 0x1.fff7ae8p-1
    0x3fefff64c0000000, // 0x1.fff64cp-1
    0x3fefff4e90000000, // 0x1.fff4e9p-1
    0x3fefff3860000000, // 0x1.fff386p-1
    0x3fefff2238000000, // 0x1.fff2238p-1
    0x3fefff0c08000000, // 0x1.fff0c08p-1
    0x3feffef5d8000000, // 0x1.ffef5d8p-1
    0x3feffedfa8000000, // 0x1.ffedfa8p-1
    0x3feffec980000000, // 0x1.ffec98p-1
    0x3feffeb350000000, // 0x1.ffeb35p-1
]);

/// `cl`.
const CL: [f64; 3] = f64_table([
    0xbfd71547652b82fe, // -0x1.71547652b82fep-2
    0x3fd2776c50f1ff14, // 0x1.2776c50f1ff14p-2
    0xbfcec709dc3eca5d, // -0x1.ec709dc3eca5dp-3
]);

/// `cy` (low, high as in the C).
const CY: [[f64; 2]; 3] = f64_pair_table([
    [0x3ff71547652b82fe, 0x3c7777d0ffda0d24], // {0x1.71547652b82fep+0, 0x1.777d0ffda0d24p-56}
    [0xbfe71547652b82fe, 0xbc6777d0ffda0d24], // {-0x1.71547652b82fep-1, -0x1.777d0ffda0d24p-57}
    [0x3fdec709dc3a03fd, 0x3c7d27e96be541e5], // {0x1.ec709dc3a03fdp-2, 0x1.d27e96be541e5p-56}
]);

/// `LL` (four tables of 17 triples).
const LL_BITS: [[[u64; 3]; 17]; 4] = [
    [
        [0x0000000000000000, 0x0000000000000000, 0x0000000000000000], // {0x0p+0, 0x0p+0, 0x0p+0}
        [0x3fb000001fda0000, 0x3d55ed58a7ff2c40, 0x39f512acbb219717], // {0x1.000001fdap-4, 0x1.5ed58a7ff2c4p-42, 0x1.512acbb219717p-96}
        [0x3fbfffffda070000, 0xbd0b38ae1c2d5400, 0xba210f11908c5c8d], // {0x1.fffffda07p-4, -0x1.b38ae1c2d54p-47, -0x1.10f11908c5c8dp-93}
        [0x3fc7ffffefb50000, 0x3d4785d91bb08320, 0x3a2f5ded30ca48c0], // {0x1.7ffffefb5p-3, 0x1.785d91bb0832p-43, 0x1.f5ded30ca48cp-93}
        [0x3fd0000004a60000, 0x3d5380abf8fe7eb0, 0xba2ebd3f567df886], // {0x1.0000004a6p-2, 0x1.380abf8fe7ebp-42, -0x1.ebd3f567df886p-93}
        [0x3fd4000009760000, 0x3d4d321385bf0a10, 0xba1c95a3d59b6c33], // {0x1.400000976p-2, 0x1.d321385bf0a1p-43, -0x1.c95a3d59b6c33p-94}
        [0x3fd80000081b4000, 0x3d5b6e8f25ff2610, 0xba24072d39d44270], // {0x1.80000081b4p-2, 0x1.b6e8f25ff261p-42, -0x1.4072d39d4427p-93}
        [0x3fdc00000f4f4000, 0x3d4a5fed334580a0, 0xb9f6f8fdabadd77a], // {0x1.c00000f4f4p-2, 0x1.a5fed334580ap-43, -0x1.6f8fdabadd77ap-96}
        [0x3fdffffff9de0000, 0xbd4f85a54fb4a600, 0xb9f50083897ee638], // {0x1.ffffff9dep-2, -0x1.f85a54fb4a6p-43, -0x1.50083897ee638p-96}
        [0x3fe1fffffa35c000, 0x3d5795db9d185140, 0xba1594f07d4d693f], // {0x1.1fffffa35cp-1, 0x1.795db9d18514p-42, -0x1.594f07d4d693fp-94}
        [0x3fe3fffff9cbc000, 0x3d32fb09a99477a0, 0x3a1b2c6248382152], // {0x1.3fffff9cbcp-1, 0x1.2fb09a99477ap-44, 0x1.b2c6248382152p-94}
        [0x3fe5fffff6df2000, 0xbd53ecf24077c3a0, 0xba2382c6b4e2daa6], // {0x1.5fffff6df2p-1, -0x1.3ecf24077c3ap-42, -0x1.382c6b4e2daa6p-93}
        [0x3fe800000552e000, 0x3d47044806f72380, 0xba26da3eb126c999], // {0x1.800000552ep-1, 0x1.7044806f7238p-43, -0x1.6da3eb126c999p-93}
        [0x3fe9fffffc182000, 0x3d45cf031d5d4cf0, 0x3a2302c44da79f4b], // {0x1.9fffffc182p-1, 0x1.5cf031d5d4cfp-43, 0x1.302c44da79f4bp-93}
        [0x3febfffff6b62000, 0x3d548d11d13372a8, 0x3a2f045032543c0d], // {0x1.bfffff6b62p-1, 0x1.48d11d13372a8p-42, 0x1.f045032543c0dp-93}
        [0x3fedfffff7a42000, 0xbd40cbf3e62fa860, 0x3a21cfaf84227211], // {0x1.dfffff7a42p-1, -0x1.0cbf3e62fa86p-43, 0x1.1cfaf84227211p-93}
        [0x3ff0000000000000, 0x0000000000000000, 0x0000000000000000], // {1, 0, 0}
    ],
    [
        [0x0000000000000000, 0x0000000000000000, 0x0000000000000000], // {0x0p+0, 0x0p+0, 0x0p+0}
        [0x3f7000024a000000, 0xbd5799935d030650, 0xba0cacad0b61c964], // {0x1.000024ap-8, -0x1.799935d03065p-42, -0x1.cacad0b61c964p-95}
        [0x3f8000014a880000, 0xbd586dd926d35ee8, 0xba2277ab54c00e64], // {0x1.000014a88p-7, -0x1.86dd926d35ee8p-42, -0x1.277ab54c00e64p-93}
        [0x3f87ffff7b380000, 0x3d15ffc7ff00e600, 0x3a25870383c1225d], // {0x1.7ffff7b38p-7, 0x1.5ffc7ff00e6p-46, 0x1.5870383c1225dp-93}
        [0x3f9000004b940000, 0xbd44aa9da7ea0120, 0xba247decfe981022], // {0x1.000004b94p-6, -0x1.4aa9da7ea012p-43, -0x1.47decfe981022p-93}
        [0x3f94000064380000, 0xbd5015d79fd24c70, 0x3a2ec26c231ddb4c], // {0x1.400006438p-6, -0x1.015d79fd24c7p-42, 0x1.ec26c231ddb4cp-93}
        [0x3f97ffff8ff80000, 0xbd5a62a2b755f048, 0xba2eebe936e9a3f9], // {0x1.7ffff8ff8p-6, -0x1.a62a2b755f048p-42, -0x1.eebe936e9a3f9p-93}
        [0x3f9c000048180000, 0x3d1d1a1bbce5f580, 0xba24dd25e141ad16], // {0x1.c00004818p-6, 0x1.d1a1bbce5f58p-46, -0x1.4dd25e141ad16p-93}
        [0x3fa0000050ee0000, 0xbd5b32450688d5e0, 0x3a2577a1c80a65b0], // {0x1.0000050eep-5, -0x1.b32450688d5ep-42, 0x1.577a1c80a65bp-93}
        [0x3fa2000014760000, 0xbd3bac8f6c103d40, 0xba1157a2472c9532], // {0x1.200001476p-5, -0x1.bac8f6c103d4p-44, -0x1.157a2472c9532p-94}
        [0x3fa3fffff2440000, 0xbd4fbab0c9d05aa0, 0xba13680ffabf1a5b], // {0x1.3fffff244p-5, -0x1.fbab0c9d05aap-43, -0x1.3680ffabf1a5bp-94}
        [0x3fa5ffffb1ce0000, 0x3d4ef9ec2ba49140, 0xba224850720b73ab], // {0x1.5ffffb1cep-5, 0x1.ef9ec2ba4914p-43, -0x1.24850720b73abp-93}
        [0x3fa7ffffd9600000, 0x3d514e3a91abeff0, 0xba140e3565c114dd], // {0x1.7ffffd96p-5, 0x1.14e3a91abeffp-42, -0x1.40e3565c114ddp-94}
        [0x3faa0000321c0000, 0x3d3e33c6a0732340, 0xba27e8c937a981d8], // {0x1.a0000321cp-5, 0x1.e33c6a073234p-44, -0x1.7e8c937a981d8p-93}
        [0x3fabffffc56a0000, 0xbd52076781ff0460, 0xba296e054ec8f31e], // {0x1.bffffc56ap-5, -0x1.2076781ff046p-42, -0x1.96e054ec8f31ep-93}
        [0x3fadffffdba40000, 0xbd4ea8b4476edde0, 0xb9e07bfb9fde770f], // {0x1.dffffdba4p-5, -0x1.ea8b4476eddep-43, -0x1.07bfb9fde770fp-97}
        [0x3fb000001fda0000, 0x3d55ed58a7ff2c40, 0x39f512acbb219717], // {0x1.000001fdap-4, 0x1.5ed58a7ff2c4p-42, 0x1.512acbb219717p-96}
    ],
    [
        [0x0000000000000000, 0x0000000000000000, 0x0000000000000000], // {0x0p+0, 0x0p+0, 0x0p+0}
        [0x3f30002866000000, 0xbd5329e31412b688, 0x3a2279ddc3585863], // {0x1.0002866p-12, -0x1.329e31412b688p-42, 0x1.279ddc3585863p-93}
        [0x3f3fffed31000000, 0x3d5546dcb40b0518, 0xba26d048f41debec], // {0x1.fffed31p-12, 0x1.546dcb40b0518p-42, -0x1.6d048f41debecp-93}
        [0x3f48000388000000, 0x3d5fce6789678c88, 0x3a24959dc2fea030], // {0x1.8000388p-11, 0x1.fce6789678c88p-42, 0x1.4959dc2fea03p-93}
        [0x3f50000668400000, 0x3d5c5caec0aa6620, 0xba289d58a2e0d287], // {0x1.00006684p-10, 0x1.c5caec0aa662p-42, -0x1.89d58a2e0d287p-93}
        [0x3f54000937000000, 0xbd5df91bf5db20f8, 0x3a2d4fc35005498a], // {0x1.4000937p-10, -0x1.df91bf5db20f8p-42, 0x1.d4fc35005498ap-93}
        [0x3f58000a2d800000, 0x3d3e7464f400a700, 0xba26365a147ce4a8], // {0x1.8000a2d8p-10, 0x1.e7464f400a7p-44, -0x1.6365a147ce4a8p-93}
        [0x3f5c00094a400000, 0xbd5c55c526e65a68, 0xba2a7ea2cf16d2ce], // {0x1.c00094a4p-10, -0x1.c55c526e65a68p-42, -0x1.a7ea2cf16d2cep-93}
        [0x3f60000345400000, 0x3d48e815121136d0, 0xba168f8f7a924fd6], // {0x1.00003454p-9, 0x1.8e815121136dp-43, -0x1.68f8f7a924fd6p-94}
        [0x3f620000f6400000, 0x3d57fb19a44da298, 0xba2a05fc6ed4124d], // {0x1.20000f64p-9, 0x1.7fb19a44da298p-42, -0x1.a05fc6ed4124dp-93}
        [0x3f63fffdb7000000, 0x3d5bfac3b1abe2e8, 0xba09ab1f45a1efb1], // {0x1.3fffdb7p-9, 0x1.bfac3b1abe2e8p-42, -0x1.9ab1f45a1efb1p-95}
        [0x3f66000516a00000, 0xbd402544d9faf830, 0x3a2286c39adffa8c], // {0x1.6000516ap-9, -0x1.02544d9faf83p-43, 0x1.286c39adffa8cp-93}
        [0x3f67fffff4200000, 0xbd52878b893da790, 0x3a2b55c5e09022b1], // {0x1.7fffff42p-9, -0x1.2878b893da79p-42, 0x1.b55c5e09022b1p-93}
        [0x3f6a00056f400000, 0xbd59d7ccf4019608, 0x3a2f4b05553fe340], // {0x1.a00056f4p-9, -0x1.9d7ccf4019608p-42, 0x1.f4b05553fe34p-93}
        [0x3f6bfffe65200000, 0xbd5ee85f47c2b5a0, 0x3a253fa913d4e9a6], // {0x1.bfffe652p-9, -0x1.ee85f47c2b5ap-42, 0x1.53fa913d4e9a6p-93}
        [0x3f6e0001f7600000, 0x3d42e594fb2af5e0, 0xb9cafaec77ca8d86], // {0x1.e0001f76p-9, 0x1.2e594fb2af5ep-43, -0x1.afaec77ca8d86p-99}
        [0x3f7000024a000000, 0xbd5799935d030650, 0xba0cacad0b61c964], // {0x1.000024ap-8, -0x1.799935d03065p-42, -0x1.cacad0b61c964p-95}
    ],
    [
        [0x0000000000000000, 0x0000000000000000, 0x0000000000000000], // {0x0p+0, 0x0p+0, 0x0p+0}
        [0x3ef0014690000000, 0xbd4743d8ea40a4d0, 0xb9e75086a21c12ab], // {0x1.001469p-16, -0x1.743d8ea40a4dp-43, -0x1.75086a21c12abp-97}
        [0x3f00014c18000000, 0x3d48e7fa3cf59080, 0x3a2a11bdcdb1ace3], // {0x1.0014c18p-15, 0x1.8e7fa3cf5908p-43, 0x1.a11bdcdb1ace3p-93}
        [0x3f07ff17c8000000, 0x3d5118dbbcce8470, 0x3a009806ecd19760], // {0x1.7ff17c8p-15, 0x1.118dbbcce847p-42, 0x1.09806ecd1976p-95}
        [0x3f0fffcbb8000000, 0xbd5eaa70b2051e38, 0x39e0d287c5aa12b4], // {0x1.fffcbb8p-15, -0x1.eaa70b2051e38p-42, 0x1.0d287c5aa12b4p-97}
        [0x3f14004294000000, 0x3d5602b04d79b0c8, 0x39f8d2c8ddf31648], // {0x1.4004294p-14, 0x1.602b04d79b0c8p-42, 0x1.8d2c8ddf31648p-96}
        [0x3f1800a218000000, 0xbd50a23112178630, 0xba2c4652435118f1], // {0x1.800a218p-14, -0x1.0a2311217863p-42, -0x1.c4652435118f1p-93}
        [0x3f1bff9304000000, 0xbd4385dc475c16b0, 0xb9fe7288f74bbbda], // {0x1.bff9304p-14, -0x1.385dc475c16bp-43, -0x1.e7288f74bbbdap-96}
        [0x3f1ffff810000000, 0x3d4355edcff4e190, 0x3a0f8daae6c1db11], // {0x1.ffff81p-14, 0x1.355edcff4e19p-43, 0x1.f8daae6c1db11p-95}
        [0x3f22002ff2000000, 0x3d23fbb7cd366140, 0x3a293c0e5b4dca92], // {0x1.2002ff2p-13, 0x1.3fbb7cd36614p-45, 0x1.93c0e5b4dca92p-93}
        [0x3f23ffac90000000, 0xbd39bde416de25c0, 0xba2f4df1d74776b3], // {0x1.3ffac9p-13, -0x1.9bde416de25cp-44, -0x1.f4df1d74776b3p-93}
        [0x3f25ffe340000000, 0xbd521735d0955dc0, 0xba21b0e10765842f], // {0x1.5ffe34p-13, -0x1.21735d0955dcp-42, -0x1.1b0e10765842fp-93}
        [0x3f28001b52000000, 0x3d40c0949e968810, 0xba1d3771c0b220f3], // {0x1.8001b52p-13, 0x1.0c0949e96881p-43, -0x1.d3771c0b220f3p-94}
        [0x3f2a0054c8000000, 0x3d426936197b1050, 0x3a2e59caa334554a], // {0x1.a0054c8p-13, 0x1.26936197b105p-43, 0x1.e59caa334554ap-93}
        [0x3f2bffd6f0000000, 0x3d427097b0b27d40, 0x3a278a37896a0d1f], // {0x1.bffd6fp-13, 0x1.27097b0b27d4p-43, 0x1.78a37896a0d1fp-93}
        [0x3f2e00132c000000, 0x3d3df1e084120280, 0x3a1c61f449a4b5d5], // {0x1.e00132cp-13, 0x1.df1e08412028p-44, 0x1.c61f449a4b5d5p-94}
        [0x3f30002866000000, 0xbd5329e31412b688, 0x3a2279ddc3585863], // {0x1.0002866p-12, -0x1.329e31412b688p-42, 0x1.279ddc3585863p-93}
    ],
];

/// `LL` as doubles.
static LL: [[[f64; 3]; 17]; 4] = {
    let mut out = [[[0.0; 3]; 17]; 4];
    let mut a = 0;
    while a < 4 {
        let mut k = 0;
        while k < 17 {
            let mut m = 0;
            while m < 3 {
                out[a][k][m] = f64::from_bits(LL_BITS[a][k][m]);
                m += 1;
            }
            k += 1;
        }
        a += 1;
    }
    out
};

#[cfg(test)]
mod tests {
    use super::log2;

    fn check(x: f64, expected_bits: u64) {
        let y = log2(x);
        assert_eq!(
            y.to_bits(),
            expected_bits,
            "log2({x:e}) = {y:e} ({:#018x}), expected {:e} ({expected_bits:#018x})",
            y.to_bits(),
            f64::from_bits(expected_bits),
        );
    }

    #[test]
    fn exact_and_special_values() {
        check(1.0, 0);
        for e in -1074..=1023 {
            let p = if e < -1022 {
                f64::from_bits(1u64 << (e + 1074))
            } else {
                f64::from_bits(((e + 1023) as u64) << 52)
            };
            check(p, f64::from(e).to_bits());
        }
        check(0.0, f64::NEG_INFINITY.to_bits());
        check(-0.0, f64::NEG_INFINITY.to_bits());
        check(f64::INFINITY, f64::INFINITY.to_bits());
        check(-1.0, f64::NAN.to_bits());
        check(f64::NEG_INFINITY, f64::NAN.to_bits());
        check(f64::from_bits(0x7ff0_0000_0000_0001), 0x7ff8_0000_0000_0001);
        check(f64::from_bits(0xfff4_0000_0000_0000), 0xfffc_0000_0000_0000);
    }

    /// FNV-1a of the result bits on 65,536 seeded arguments (SplitMix64 from 0x123456789abcdef0:
    /// one third uniform on (0, 1000), one third random bit patterns, one third subnormal or
    /// zero), the arguments of `validation/probes/coremath-log2/`'s `seeded_hash`: CORE-MATH's C `cr_log2` gives it, its NaN for `x < 0` written as larmorx's `0x7ff8...` (every platform must give it).
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
            h = (h ^ log2(x).to_bits()).wrapping_mul(0x0100_0000_01b3);
        }
        assert_eq!(h, 0x7fdf_578b_8ea6_88b3);
    }
}
