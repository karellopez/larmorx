// SPDX-License-Identifier: GPL-3.0-or-later AND LGPL-2.1-or-later
// Translated to Rust from the GNU C Library 2.35: sysdeps/ieee754/flt-32/s_sinf.c,
// s_cosf.c, s_sincosf.h, sysdeps/x86/fpu/sincosf_poly.h and s_sincosf_data.c, as compiled
// for x86_64 by sysdeps/x86_64/fpu/multiarch/s_sinf-fma.c and s_cosf-fma.c.
// Copyright (C) 2018-2022 Free Software Foundation, Inc. The GNU C Library is free software,
// licensed under the GNU Lesser General Public License version 2.1 or any later version; this
// translation is also distributed under the GNU GPL version 3 or later as part of larmorx-gpl
// (LGPL-2.1 section 3 permits applying the GPL instead).
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! glibc's single-precision `sinf` and `cosf`, bit for bit.
//!
//! AFNI's weighted-sinc interpolation (`thd_shift2.c`) calls the C library's `sinf` and
//! `cosf`. On Linux that is glibc, whose float sine and cosine are fast but not correctly
//! rounded (worst case 0.56 ulp), so a correctly rounded function gives a different `f32` for
//! some arguments. To reproduce AFNI's output on every platform, larmorx-gpl carries this port
//! of glibc 2.35's code (the version AFNI 25.2.09 runs with on Ubuntu 22.04).
//!
//! On x86-64, glibc selects one of two builds of the same C code at load time (`ifunc`):
//! `__sinf_fma` on CPUs with FMA and AVX2, `__sinf_sse2` otherwise. The FMA build lets the
//! compiler fuse each `a + b*c` into one rounding. This port reproduces the FMA build, which
//! is what glibc runs on every x86-64 CPU since 2013 (Haswell, Piledriver) and, with the same
//! fused operations, on aarch64. `f64::mul_add` is a correctly rounded fused multiply-add on
//! every platform (in hardware or in the platform's `fma`), so the results do not depend on
//! the machine running larmorx.
//!
//! Verified exhaustively against glibc 2.35 on x86-64 with FMA (all 2^32 inputs, both
//! functions; `tools/sincosf-verify`).

/// The constants of one quadrant pair: `sincos_t` in `sysdeps/x86/fpu/sincosf_poly.h`, with
/// the x86 layout's pairs (`s1c2`, `s2c3`, `s3c4`) split into fields.
struct SinCos {
    /// Sign of the sine in quadrants 0 to 3.
    sign: [f64; 4],
    /// 2/π scaled by 2^24 (`TOINT_INTRINSICS` is 0 on x86-64).
    hpi_inv: f64,
    /// π/2.
    hpi: f64,
    c0: f64,
    c1: f64,
    c2: f64,
    c3: f64,
    c4: f64,
    s1: f64,
    s2: f64,
    s3: f64,
}

/// `__sincosf_table` (`sysdeps/x86/fpu/s_sincosf_data.c`). The second entry computes
/// `-cos(x)` to get the negation for free. The hexadecimal floats of the C source are given as
/// their bit patterns.
const TABLE: [SinCos; 2] = [
    SinCos {
        sign: [1.0, -1.0, -1.0, 1.0],
        hpi_inv: f64::from_bits(0x4164_5F30_6DC9_C883), // 0x1.45F306DC9C883p+23
        hpi: f64::from_bits(0x3FF9_21FB_5444_2D18),     // 0x1.921FB54442D18p0
        c0: f64::from_bits(0x3FF0_0000_0000_0000),      // 0x1p0
        c1: f64::from_bits(0xBFDF_FFFF_FD0C_621C),      // -0x1.ffffffd0c621cp-2
        c2: f64::from_bits(0x3FA5_5553_E106_8F19),      // 0x1.55553e1068f19p-5
        c3: f64::from_bits(0xBF56_C087_E89A_359D),      // -0x1.6c087e89a359dp-10
        c4: f64::from_bits(0x3EF9_9343_027B_F8C3),      // 0x1.99343027bf8c3p-16
        s1: f64::from_bits(0xBFC5_5554_5995_A603),      // -0x1.555545995a603p-3
        s2: f64::from_bits(0x3F81_1076_0523_0BC4),      // 0x1.1107605230bc4p-7
        s3: f64::from_bits(0xBF29_94EB_3774_CF24),      // -0x1.994eb3774cf24p-13
    },
    SinCos {
        sign: [1.0, -1.0, -1.0, 1.0],
        hpi_inv: f64::from_bits(0x4164_5F30_6DC9_C883), // 0x1.45F306DC9C883p+23
        hpi: f64::from_bits(0x3FF9_21FB_5444_2D18),     // 0x1.921FB54442D18p0
        c0: f64::from_bits(0xBFF0_0000_0000_0000),      // -0x1p0
        c1: f64::from_bits(0x3FDF_FFFF_FD0C_621C),      // 0x1.ffffffd0c621cp-2
        c2: f64::from_bits(0xBFA5_5553_E106_8F19),      // -0x1.55553e1068f19p-5
        c3: f64::from_bits(0x3F56_C087_E89A_359D),      // 0x1.6c087e89a359dp-10
        c4: f64::from_bits(0xBEF9_9343_027B_F8C3),      // -0x1.99343027bf8c3p-16
        s1: f64::from_bits(0xBFC5_5554_5995_A603),      // -0x1.555545995a603p-3
        s2: f64::from_bits(0x3F81_1076_0523_0BC4),      // 0x1.1107605230bc4p-7
        s3: f64::from_bits(0xBF29_94EB_3774_CF24),      // -0x1.994eb3774cf24p-13
    },
];

/// `__inv_pio4`: 4/π to 192 bits; 8 new bits per entry, to avoid unaligned accesses.
const INV_PIO4: [u32; 24] = [
    0xa2, 0xa2f9, 0xa2f983, 0xa2f9836e, 0xf9836e4e, 0x836e4e44, 0x6e4e4415, 0x4e441529, 0x441529fc,
    0x1529fc27, 0x29fc2757, 0xfc2757d1, 0x2757d1f5, 0x57d1f534, 0xd1f534dd, 0xf534ddc0, 0x34ddc0db,
    0xddc0db62, 0xc0db6295, 0xdb629599, 0x6295993c, 0x95993c43, 0x993c4390, 0x3c439041,
];

/// `pi63`: 2π · 2^-64.
const PI63: f64 = f64::from_bits(0x3C19_21FB_5444_2D18); // 0x1.921FB54442D18p-62
/// `pio4`: π/4 as a float.
const PIO4: f32 = f32::from_bits(0x3F49_0FDB); // 0x1.921FB6p-1f
/// `0x1p-12f`.
const TINY: f32 = f32::from_bits(0x3980_0000);
/// `120.0f`.
const FAST_LIMIT: f32 = 120.0;

/// `abstop12`: the top 12 bits of the float's representation with the sign bit cleared.
#[inline]
fn abstop12(x: f32) -> u32 {
    (x.to_bits() >> 20) & 0x7ff
}

/// `c + a*b` as the build computes it: one fused multiply-add in the FMA build (`FUSED`), a
/// rounded product and a rounded sum in the SSE2 build.
#[inline(always)]
fn madd<const FUSED: bool>(a: f64, b: f64, c: f64) -> f64 {
    if FUSED { a.mul_add(b, c) } else { c + a * b }
}

/// `reduce_fast` without `TOINT_INTRINSICS`: `x` modulo π/2, between -π/4 and π/4, and the
/// quadrant.
#[inline]
fn reduce_fast<const FUSED: bool>(x: f64, p: &SinCos) -> (f64, i32) {
    let r = x * p.hpi_inv;
    // `(int32_t)r` truncates; `r` is below 2^31 in magnitude for |x| < 120.
    let n = ((r as i32).wrapping_add(0x80_0000)) >> 24;
    // `x - n * p->hpi`.
    (madd::<FUSED>(-f64::from(n), p.hpi, x), n)
}

/// `reduce_large`: the reduction of `|y| >= 120` with a 32x96 -> 128-bit product with 4/π.
/// Takes the float's bits (the sign is ignored) and returns the modulo and the quadrant.
#[inline]
fn reduce_large(xi: u32) -> (f64, i32) {
    let arr = &INV_PIO4[((xi >> 26) & 15) as usize..];
    let shift = (xi >> 23) & 7;
    let xi = ((xi & 0xff_ffff) | 0x80_0000) << shift;
    // `xi * arr[0]` is a 32-bit product in C (both operands are uint32_t).
    let res0 = u64::from(xi.wrapping_mul(arr[0]));
    let res1 = u64::from(xi) * u64::from(arr[4]);
    let res2 = u64::from(xi) * u64::from(arr[8]);
    let mut res0 = (res2 >> 32) | (res0 << 32);
    res0 = res0.wrapping_add(res1);
    let n = (res0.wrapping_add(1 << 61)) >> 62;
    res0 = res0.wrapping_sub(n << 62);
    let x = res0 as i64 as f64;
    (x * PI63, n as i32)
}

/// `sinf_poly` (`sysdeps/x86/fpu/sincosf_poly.h`): the sine polynomial for even `n`, the
/// cosine polynomial for odd `n`. Products that feed another product stay separate; each
/// product that feeds an addition is fused with it in the FMA build.
#[inline]
fn sinf_poly<const FUSED: bool>(x: f64, x2: f64, p: &SinCos, n: i32) -> f32 {
    if n & 1 == 0 {
        let x3 = x * x2;
        let s1 = madd::<FUSED>(x2, p.s3, p.s2);
        let x7 = x3 * x2;
        let s = madd::<FUSED>(x3, p.s1, x);
        madd::<FUSED>(x7, s1, s) as f32
    } else {
        let x4 = x2 * x2;
        let c2 = madd::<FUSED>(x2, p.c4, p.c3);
        let c1 = madd::<FUSED>(x2, p.c1, p.c0);
        let x6 = x4 * x2;
        let c = madd::<FUSED>(x4, p.c2, c1);
        madd::<FUSED>(x6, c2, c) as f32
    }
}

/// `__math_invalidf(y)` for infinities and NaNs: `(y - y) / (y - y)`, a NaN.
#[allow(clippy::eq_op)]
fn invalid(y: f32) -> f32 {
    (y - y) / (y - y)
}

/// `SINF_FUNC` (`s_sinf.c`), for the FMA (`FUSED`) or SSE2 build.
#[inline]
fn sinf_impl<const FUSED: bool>(y: f32) -> f32 {
    let x = f64::from(y);
    let top = abstop12(y);
    if top < abstop12(PIO4) {
        let s = x * x;
        if top < abstop12(TINY) {
            // glibc forces an underflow exception for tiny `y`; the value is `y`.
            return y;
        }
        sinf_poly::<FUSED>(x, s, &TABLE[0], 0)
    } else if top < abstop12(FAST_LIMIT) {
        let (x, n) = reduce_fast::<FUSED>(x, &TABLE[0]);
        let s = TABLE[0].sign[(n & 3) as usize];
        let p = &TABLE[usize::from(n & 2 != 0)];
        sinf_poly::<FUSED>(x * s, x * x, p, n)
    } else if top < abstop12(f32::INFINITY) {
        let xi = y.to_bits();
        let sign = (xi >> 31) as i32;
        let (x, n) = reduce_large(xi);
        let q = n.wrapping_add(sign);
        let s = TABLE[0].sign[(q & 3) as usize];
        let p = &TABLE[usize::from(q & 2 != 0)];
        sinf_poly::<FUSED>(x * s, x * x, p, n)
    } else {
        invalid(y)
    }
}

/// `COSF_FUNC` (`s_cosf.c`), for the FMA (`FUSED`) or SSE2 build.
#[inline]
fn cosf_impl<const FUSED: bool>(y: f32) -> f32 {
    let x = f64::from(y);
    let top = abstop12(y);
    if top < abstop12(PIO4) {
        let x2 = x * x;
        if top < abstop12(TINY) {
            return 1.0;
        }
        sinf_poly::<FUSED>(x, x2, &TABLE[0], 1)
    } else if top < abstop12(FAST_LIMIT) {
        let (x, n) = reduce_fast::<FUSED>(x, &TABLE[0]);
        let s = TABLE[0].sign[(n & 3) as usize];
        let p = &TABLE[usize::from(n & 2 != 0)];
        sinf_poly::<FUSED>(x * s, x * x, p, n ^ 1)
    } else if top < abstop12(f32::INFINITY) {
        let xi = y.to_bits();
        let sign = (xi >> 31) as i32;
        let (x, n) = reduce_large(xi);
        let q = n.wrapping_add(sign);
        let s = TABLE[0].sign[(q & 3) as usize];
        let p = &TABLE[usize::from(q & 2 != 0)];
        sinf_poly::<FUSED>(x * s, x * x, p, n ^ 1)
    } else {
        invalid(y)
    }
}

/// glibc 2.35's `sinf(y)`: the x86-64 FMA build (`__sinf_fma`).
pub fn sinf(y: f32) -> f32 {
    sinf_impl::<true>(y)
}

/// glibc 2.35's `cosf(y)`: the x86-64 FMA build (`__cosf_fma`).
pub fn cosf(y: f32) -> f32 {
    cosf_impl::<true>(y)
}

/// The SSE2 builds (`__sinf_sse2`, `__cosf_sse2`), which glibc runs on x86-64 CPUs without
/// FMA. For the verification harness only (`tools/sincosf-verify`), which measures where they
/// differ from the FMA build.
#[cfg(feature = "verification")]
#[doc(hidden)]
pub mod verification {
    /// `__sinf_sse2`.
    pub fn sinf_sse2(y: f32) -> f32 {
        super::sinf_impl::<false>(y)
    }

    /// `__cosf_sse2`.
    pub fn cosf_sse2(y: f32) -> f32 {
        super::cosf_impl::<false>(y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values of glibc 2.35's `sinf`/`cosf` on x86-64 with FMA: the branch limits, then
    /// inputs spread over [0.01, 32] where glibc is not correctly rounded. (input bits, sinf
    /// bits, cosf bits), from `tools/sincosf-verify --vectors 40`.
    const GLIBC: &[(u32, u32, u32)] = &[
        (0x39800000, 0x39800000, 0x3f800000),
        (0x397fffff, 0x397fffff, 0x3f800000),
        (0x3f490fdb, 0x3f3504f3, 0x3f3504f3),
        (0x3f490fda, 0x3f3504f3, 0x3f3504f4),
        (0x42f00000, 0x3f14a2ef, 0x3f506e2a),
        (0x42efffff, 0x3f14a287, 0x3f506e74),
        (0x3c24017e, 0x3c2400cb, 0x3f7ffcb7),
        (0x3c496b75, 0x3c496a29, 0x3f7ffb0c),
        (0x3c6edb8e, 0x3c6ed964, 0x3f7ff909),
        (0x3c945c7c, 0x3c945a69, 0x3f7ff541),
        (0x3cb9e483, 0x3cb9e06e, 0x3f7fef21),
        (0x3cdf654c, 0x3cdf5e36, 0x3f7fe7a2),
        (0x3d04e1b6, 0x3d04dbbf, 0x3f7fdd84),
        (0x3d2a5ffd, 0x3d2a536a, 0x3f7fc750),
        (0x3d4fe06f, 0x3d4fc998, 0x3f7fab9e),
        (0x3d7561f3, 0x3d753c62, 0x3f7f8a6f),
        (0x3d9ae188, 0x3d9abbc0, 0x3f7f44af),
        (0x3dc06313, 0x3dc01aac, 0x3f7edf0d),
        (0x3de5e3d3, 0x3de56850, 0x3f7e638c),
        (0x3e0b6463, 0x3e0af649, 0x3f7da1bf),
        (0x3e30e57b, 0x3e300494, 0x3f7c308b),
        (0x3e56668e, 0x3e54d66a, 0x3f7a68c1),
        (0x3e7be76e, 0x3e795efa, 0x3f784afe),
        (0x3ea16874, 0x3e9ebf6d, 0x3f736250),
        (0x3ec6e97f, 0x3ec1f22e, 0x3f6cec24),
        (0x3eec6a8f, 0x3ee41aaa, 0x3f6530ad),
        (0x3f11ebe9, 0x3f0a25a7, 0x3f57869d),
        (0x3f376c89, 0x3f2820b8, 0x3f410d4a),
        (0x3f5ced8f, 0x3f428204, 0x3f2670f6),
        (0x3f826ea9, 0x3f5a017f, 0x3f063305),
        (0x3fa7efcc, 0x3f7779a9, 0x3e8306e0),
        (0x3fcd71b5, 0x3f7fd999, 0xbd0c3449),
        (0x3ff2f1a8, 0x3f726afa, 0xbea48e08),
        (0x40187339, 0x3f304840, 0xbf39a2b6),
        (0x403df3c8, 0x3e30dcfa, 0xbf7c2715),
        (0x4063769f, 0xbecd4592, 0xbf6a8658),
        (0x4088f5d9, 0xbf6870d6, 0xbed68b48),
        (0x40ae76ca, 0xbf3d1d6f, 0x3f2c8b56),
        (0x40d3f7df, 0x3eab246e, 0x3f714660),
        (0x40f978d3, 0x3f7f91de, 0x3d6d5d16),
        (0x411efa00, 0xbefa8221, 0xbf5f43eb),
        (0x41447af2, 0xbe909e3c, 0x3f75935e),
        (0x4169fbfd, 0x3f624153, 0xbeef89b2),
        (0x418f7d64, 0xbf4aa292, 0x3f1c71ec),
        (0x41b4fe89, 0xbf177813, 0xbf4e618b),
        (0x41da7efc, 0x3f520df6, 0xbf125506),
    ];

    #[test]
    fn matches_recorded_glibc_values() {
        for &(x, s, c) in GLIBC {
            let x = f32::from_bits(x);
            assert_eq!(sinf(x).to_bits(), s, "sinf({x:e})");
            assert_eq!(cosf(x).to_bits(), c, "cosf({x:e})");
        }
    }

    #[test]
    fn special_values() {
        assert_eq!(sinf(0.0).to_bits(), 0.0f32.to_bits());
        assert_eq!(sinf(-0.0).to_bits(), (-0.0f32).to_bits());
        assert_eq!(cosf(0.0), 1.0);
        assert_eq!(cosf(-0.0), 1.0);
        for v in [f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
            assert!(sinf(v).is_nan() && cosf(v).is_nan());
        }
        let tiny = f32::from_bits(0x3970_0000);
        assert_eq!(sinf(tiny), tiny);
        assert_eq!(cosf(tiny), 1.0);
    }

    #[test]
    fn close_to_correctly_rounded() {
        // Within one float ulp of the correctly rounded double result, everywhere.
        let mut x = 1.0e-3f32;
        while x < 1.0e6 {
            for v in [x, -x] {
                let (s, c) = (f64::from(v).sin(), f64::from(v).cos());
                let ulp = |r: f64| f64::from(r as f32).abs() * f64::from(f32::EPSILON);
                assert!(
                    (f64::from(sinf(v)) - s).abs() <= ulp(s).max(1e-45),
                    "sinf({v})"
                );
                assert!(
                    (f64::from(cosf(v)) - c).abs() <= ulp(c).max(1e-45),
                    "cosf({v})"
                );
            }
            x *= 1.000_37;
        }
    }
}
