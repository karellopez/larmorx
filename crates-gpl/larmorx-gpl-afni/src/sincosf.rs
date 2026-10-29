// SPDX-License-Identifier: Apache-2.0 AND MIT
// Translated to Rust from Arm's Optimized Routines v19.11 (github.com/ARM-software/optimized-routines,
// tag v19.11): math/sincosf.h (abstop12, sinf_poly, reduce_fast, reduce_large, pi63, pio4),
// math/sinf.c, math/cosf.c and math/sincosf_data.c (__sincosf_table, __inv_pio4).
// Copyright (c) 2018, Arm Limited. SPDX-License-Identifier: MIT (the licence and notice are in
// LICENSES/MIT-arm-optimized-routines.txt and NOTICE).
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
//! The single-precision `sinf` and `cosf` AFNI's Linux builds call, bit for bit.
//!
//! AFNI's weighted-sinc interpolation (`thd_shift2.c`) calls the C library's `sinf` and `cosf`.
//! On Linux that is glibc, whose float sine and cosine are fast but not correctly rounded (at
//! most 0.56 ulp), so a correctly rounded function gives a different `f32` for some arguments.
//! glibc 2.35 (the C library AFNI runs with on Ubuntu 22.04) took these two functions from Arm's
//! Optimized Routines (2018) and kept their arithmetic; this module is translated from Arm's
//! MIT-licensed originals, not from glibc.
//!
//! On x86-64, glibc compiles the same C code twice and picks one build at load time: with FMA
//! and AVX2 (every x86-64 CPU since 2013, and the build AFNI's oracle runs), the compiler fuses
//! each `a + b * c` into one rounding; without them (SSE2), each product and sum rounds. Both
//! are here (`FUSED`); [`sinf`] and [`cosf`] are the FMA build. `f64::mul_add` is a correctly
//! rounded fused multiply-add on every platform (in hardware or in the platform's `fma`), so
//! the results do not depend on the machine running larmorx.
//!
//! Proved equal, on all 2^32 inputs of each function and each build, to the earlier translation
//! from glibc 2.35 that the replica used until 2026-10-29 (itself checked against glibc on all
//! inputs), and checked against the C library of the development machine (glibc 2.35, both
//! builds): `crates-gpl/tools/sincosf-verify`.

/// One entry of `__sincosf_table`: the signs of the sine by quadrant, the reduction
/// constants (`TOINT_INTRINSICS` is 0 on x86-64: `hpi_inv` is 2/π scaled by 2^24), the cosine
/// polynomial `c0..c4` and the sine polynomial `s1..s3`.
struct Coefficients {
    sign: [f64; 4],
    hpi_inv: f64,
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

/// `__sincosf_table` (`sincosf_data.c`); the second entry negates the cosine polynomial, so
/// that quadrants 2 and 3 get their sign for free. Arm's hexadecimal constants are written as
/// their IEEE-754 bit patterns, with the C literal beside each.
const SINCOSF_TABLE: [Coefficients; 2] = [
    Coefficients {
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
    Coefficients {
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

/// `__inv_pio4`: 4/π to 192 bits, eight new bits per entry (to avoid unaligned reads).
const INV_PIO4: [u32; 24] = [
    0xa2, 0xa2f9, 0xa2f983, 0xa2f9836e, 0xf9836e4e, 0x836e4e44, 0x6e4e4415, 0x4e441529, 0x441529fc,
    0x1529fc27, 0x29fc2757, 0xfc2757d1, 0x2757d1f5, 0x57d1f534, 0xd1f534dd, 0xf534ddc0, 0x34ddc0db,
    0xddc0db62, 0xc0db6295, 0xdb629599, 0x6295993c, 0x95993c43, 0x993c4390, 0x3c439041,
];

/// `pi63`: 2π · 2^-64 (`0x1.921FB54442D18p-62`).
const PI63: f64 = f64::from_bits(0x3C19_21FB_5444_2D18);

/// The top 12 bits (`abstop12`) of the limits `sinf` and `cosf` compare with: `pio4` (π/4, a
/// double converted to float, `0x3F490FDB`), `0x1p-12f`, `120.0f` and infinity.
const TOP12_PIO4: u32 = abstop12(f32::from_bits(0x3F49_0FDB));
const TOP12_TINY: u32 = abstop12(f32::from_bits(0x3980_0000));
const TOP12_FAST: u32 = abstop12(120.0);
const TOP12_INF: u32 = abstop12(f32::INFINITY);

/// `abstop12`: the top 12 bits of the float's representation, its sign bit cleared.
const fn abstop12(x: f32) -> u32 {
    (x.to_bits() >> 20) & 0x7ff
}

/// `a * b + c` as the build computes the C expression `c + a * b`: one fused multiply-add in
/// the FMA build (`FUSED`); a rounded product, then a rounded sum, in the SSE2 build.
#[inline(always)]
fn mul_add<const FUSED: bool>(a: f64, b: f64, c: f64) -> f64 {
    if FUSED { a.mul_add(b, c) } else { c + a * b }
}

/// `sinf_poly(x, x2, p, n)`: the sine polynomial of `x` (`x2` its square) for an even
/// quadrant `n`, the cosine polynomial for an odd one, rounded to float. Products that feed
/// another product are plain; each one added to something is fused with that addition in the
/// FMA build.
#[inline]
fn sinf_poly<const FUSED: bool>(x: f64, x2: f64, p: &Coefficients, n: i32) -> f32 {
    if n & 1 == 0 {
        let x3 = x * x2;
        let s1 = mul_add::<FUSED>(x2, p.s3, p.s2);
        let x7 = x3 * x2;
        let s = mul_add::<FUSED>(x3, p.s1, x);
        mul_add::<FUSED>(x7, s1, s) as f32
    } else {
        let x4 = x2 * x2;
        let c2 = mul_add::<FUSED>(x2, p.c4, p.c3);
        let c1 = mul_add::<FUSED>(x2, p.c1, p.c0);
        let x6 = x4 * x2;
        let c = mul_add::<FUSED>(x4, p.c2, c1);
        mul_add::<FUSED>(x6, c2, c) as f32
    }
}

/// `reduce_fast` (no `TOINT_INTRINSICS`): `x` modulo π/2, in [-π/4, π/4], and its quadrant.
/// `hpi_inv` carries a factor 2^24, so the quadrant is in bits 24 to 31 of the truncated
/// product, rounded by adding half of that unit.
#[inline]
fn reduce_fast<const FUSED: bool>(x: f64, p: &Coefficients) -> (f64, i32) {
    let r = x * p.hpi_inv;
    // `(int32_t)r` truncates toward zero; |r| < 2^31 below 120. `>>` is arithmetic.
    let n = ((r as i32) + 0x80_0000) >> 24;
    // `x - n * p->hpi`.
    (mul_add::<FUSED>(-f64::from(n), p.hpi, x), n)
}

/// `reduce_large`: the reduction of a float of magnitude 120 or more, from its bits `xi` (the
/// sign is ignored), with a 32 × 96-bit product with 4/π that gives the exact fixed-point
/// remainder; returns the remainder times π/2 (in [-π/4, π/4]) and the quadrant.
#[inline]
fn reduce_large(xi: u32) -> (f64, i32) {
    let arr = &INV_PIO4[((xi >> 26) & 15) as usize..];
    let shift = (xi >> 23) & 7;
    let m = ((xi & 0xff_ffff) | 0x80_0000) << shift;
    // `xi * arr[0]` multiplies two uint32_t: the product wraps to 32 bits before widening.
    let lo = u64::from(m.wrapping_mul(arr[0]));
    let mid = u64::from(m) * u64::from(arr[4]);
    let hi = u64::from(m) * u64::from(arr[8]);
    let mut res = ((hi >> 32) | (lo << 32)).wrapping_add(mid);
    let n = res.wrapping_add(1 << 61) >> 62;
    res = res.wrapping_sub(n << 62);
    // `(int64_t)res`, then `* pi63`.
    ((res as i64) as f64 * PI63, n as i32)
}

/// `__math_invalidf(y)` for infinities and NaNs: `(y - y) / (y - y)`, a NaN.
#[allow(clippy::eq_op)]
#[inline]
fn invalid(y: f32) -> f32 {
    (y - y) / (y - y)
}

/// The argument reduced to [-π/4, π/4] with its sign for the quadrant, the polynomial table
/// and the quadrant, for |y| at least π/4 (`sinf.c`, `cosf.c`).
#[inline]
fn reduced<const FUSED: bool>(y: f32, top: u32) -> (f64, &'static Coefficients, i32) {
    let p = &SINCOSF_TABLE[0];
    if top < TOP12_FAST {
        let (x, n) = reduce_fast::<FUSED>(f64::from(y), p);
        let s = p.sign[(n & 3) as usize];
        let table = &SINCOSF_TABLE[usize::from(n & 2 != 0)];
        (x * s, table, n)
    } else {
        let xi = y.to_bits();
        let sign = (xi >> 31) as i32;
        let (x, n) = reduce_large(xi);
        let q = n + sign;
        let s = p.sign[(q & 3) as usize];
        let table = &SINCOSF_TABLE[usize::from(q & 2 != 0)];
        (x * s, table, n)
    }
}

/// `sinf(y)` of the FMA (`FUSED`) or SSE2 build.
#[inline]
fn sinf_build<const FUSED: bool>(y: f32) -> f32 {
    let top = abstop12(y);
    if top < TOP12_PIO4 {
        if top < TOP12_TINY {
            // Arm forces an underflow exception for the tiniest `y`; the value is `y`.
            return y;
        }
        let x = f64::from(y);
        sinf_poly::<FUSED>(x, x * x, &SINCOSF_TABLE[0], 0)
    } else if top < TOP12_INF {
        let (x, p, n) = reduced::<FUSED>(y, top);
        sinf_poly::<FUSED>(x, x * x, p, n)
    } else {
        invalid(y)
    }
}

/// `cosf(y)` of the FMA (`FUSED`) or SSE2 build.
#[inline]
fn cosf_build<const FUSED: bool>(y: f32) -> f32 {
    let top = abstop12(y);
    if top < TOP12_PIO4 {
        if top < TOP12_TINY {
            return 1.0;
        }
        let x = f64::from(y);
        sinf_poly::<FUSED>(x, x * x, &SINCOSF_TABLE[0], 1)
    } else if top < TOP12_INF {
        let (x, p, n) = reduced::<FUSED>(y, top);
        sinf_poly::<FUSED>(x, x * x, p, n ^ 1)
    } else {
        invalid(y)
    }
}

/// The C library's `sinf(y)` as AFNI's oracle runs it: glibc 2.35's FMA build (`__sinf_fma`).
pub fn sinf(y: f32) -> f32 {
    sinf_build::<true>(y)
}

/// The C library's `cosf(y)` as AFNI's oracle runs it: glibc 2.35's FMA build (`__cosf_fma`).
pub fn cosf(y: f32) -> f32 {
    cosf_build::<true>(y)
}

/// The SSE2 builds (`__sinf_sse2`, `__cosf_sse2`), which glibc runs on x86-64 CPUs without
/// FMA: for the verification tool (`crates-gpl/tools/sincosf-verify`) only.
#[cfg(feature = "verification")]
#[doc(hidden)]
pub mod sse2 {
    /// `sinf` without fused operations.
    pub fn sinf(y: f32) -> f32 {
        super::sinf_build::<false>(y)
    }

    /// `cosf` without fused operations.
    pub fn cosf(y: f32) -> f32 {
        super::cosf_build::<false>(y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The C library's values at the branch limits and where it is not correctly rounded, on
    /// the arguments 3dTshift's weighted sinc passes (in [0, 32]), measured on the development
    /// machine (glibc 2.35, FMA build): `(input, sinf, cosf)` as bits, from
    /// `crates-gpl/tools/sincosf-verify --vectors 4`.
    const C_LIBRARY: &[(u32, u32, u32)] = &[
        (0x3980_0000, 0x3980_0000, 0x3f80_0000),
        (0x3f49_0fdb, 0x3f35_04f3, 0x3f35_04f3),
        (0x3f49_0fda, 0x3f35_04f3, 0x3f35_04f4),
        (0x42f0_0000, 0x3f14_a2ef, 0x3f50_6e2a),
        (0x42ef_ffff, 0x3f14_a287, 0x3f50_6e74),
        (0x3c24_017e, 0x3c24_00cb, 0x3f7f_fcb7),
        (0x3d9a_e188, 0x3d9a_bbc0, 0x3f7f_44af),
        (0x3f11_ebe9, 0x3f0a_25a7, 0x3f57_869d),
        (0x4088_f5d9, 0xbf68_70d6, 0xbed6_8b48),
    ];

    #[test]
    fn matches_the_c_library() {
        for &(x, s, c) in C_LIBRARY {
            let x = f32::from_bits(x);
            assert_eq!(sinf(x).to_bits(), s, "sinf({x:e})");
            assert_eq!(cosf(x).to_bits(), c, "cosf({x:e})");
        }
    }

    #[test]
    fn close_to_correctly_rounded() {
        // Within one float ulp of the correctly rounded double result, everywhere.
        let mut x = 1.0e-3f32;
        while x < 1.0e6 {
            for v in [x, -x] {
                let (s, c) = (
                    larmorx_core::math::sin(f64::from(v)),
                    larmorx_core::math::cos(f64::from(v)),
                );
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
