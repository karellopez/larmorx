// SPDX-License-Identifier: Apache-2.0 AND MIT
// Translated to Rust from Arm's Optimized Routines v19.11 (github.com/ARM-software/optimized-routines,
// tag v19.11): math/sincosf.h (abstop12, sincosf_poly, sinf_poly, reduce_fast, reduce_large,
// pi63, pio4), math/sincosf.c, math/sinf.c, math/cosf.c and math/sincosf_data.c
// (__sincosf_table, __inv_pio4). Copyright (c) 2018, Arm Limited, under the MIT licence (its text
// and notice are in LICENSES/MIT-arm-optimized-routines.txt and NOTICE).
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
//! The single-precision sine and cosine with the bits of glibc 2.35's `sinf`, `cosf` and
//! `sincosf` on x86-64, on every platform.
//!
//! glibc 2.35 took these functions from Arm's Optimized Routines (2018) and kept their
//! arithmetic; this module is translated from Arm's MIT-licensed originals, not from glibc. On
//! x86-64 glibc compiles the same C code twice and picks one build at load time: with FMA and
//! AVX2 (`__sinf_fma`, `__cosf_fma`, `__sincosf_fma`; every x86-64 CPU since 2013, and the build
//! the oracles run), GCC fuses each `a + b * c` into one rounding; without them (`__sinf_sse2`,
//! ...), each product and each sum rounds. Both are here (`FUSED`); [`sinf_glibc`],
//! [`cosf_glibc`] and [`sincosf_glibc`] are the FMA build. `f64::mul_add` is correctly rounded
//! on every platform (in hardware or in the platform's `fma`), and the NaN results are SSE's
//! (`super::x86_nan`), so the results are glibc's x86-64 bits on every machine.
//!
//! They are not correctly rounded (at most 0.56 ulp): [`super::sinf`] and [`super::cosf`] are,
//! and are larmorx's default. These serve `reproduce`, where an original program's result
//! depends on glibc's last bit (AFNI's weighted sinc, mcflirt's rotations and sinc kernels).
//! Until 2026-11-04 the FSL replicas' core (`larmorx-nc-core`'s `glibc_float`) and the AFNI
//! replicas (`larmorx-afni-replica`'s `sincosf`) carried their own translations; both now call
//! this one. Proved equal to the system's functions (glibc 2.35, x86-64, FMA build) and to both
//! earlier translations on all 2^32 inputs, NaN bits included
//! (`validation/investigations/glibc-maths/`).

use super::x86_nan::{invalid_f32, sub_f32};

/// One entry of `__sincosf_table`: the signs of the sine by quadrant, the reduction constants
/// (`TOINT_INTRINSICS` is 0 on x86-64: `hpi_inv` is 2/π scaled by 2^24), the cosine polynomial
/// `c0..c4` and the sine polynomial `s1..s3`.
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

/// `__sincosf_table` (`sincosf_data.c`); the second entry negates the cosine polynomial, so that
/// quadrants 2 and 3 get their sign for free. Arm's hexadecimal constants are written as their
/// IEEE-754 bit patterns, with the C literal beside each.
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

/// The top 12 bits (`abstop12`) of the limits the functions compare with: `pio4` (π/4, a double
/// converted to float, `0x3F490FDB`), `0x1p-12f`, `120.0f` and infinity.
const TOP12_PIO4: u32 = abstop12(f32::from_bits(0x3F49_0FDB));
const TOP12_TINY: u32 = abstop12(f32::from_bits(0x3980_0000));
const TOP12_FAST: u32 = abstop12(120.0);
const TOP12_INF: u32 = abstop12(f32::INFINITY);

/// `abstop12`: the top 12 bits of the float's representation, its sign bit cleared.
#[inline(always)]
const fn abstop12(x: f32) -> u32 {
    (x.to_bits() >> 20) & 0x7ff
}

/// `a * b + c` as the build computes the C expression `c + a * b`: one fused multiply-add in the
/// FMA build (`FUSED`); a rounded product, then a rounded sum, in the SSE2 build.
#[inline(always)]
fn mul_add<const FUSED: bool>(a: f64, b: f64, c: f64) -> f64 {
    if FUSED { a.mul_add(b, c) } else { c + a * b }
}

/// `sinf_poly(x, x2, p, n)`: the sine polynomial of `x` (`x2` its square) for an even quadrant
/// `n`, the cosine polynomial for an odd one, rounded to float. Products that feed another
/// product are plain; each one added to something is fused with that addition in the FMA build.
#[inline(always)]
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

/// `sincosf_poly(x, x2, p, n, sinp, cosp)`: `(sin, cos)`, swapped for an odd quadrant `n`.
#[inline(always)]
fn sincosf_poly<const FUSED: bool>(x: f64, x2: f64, p: &Coefficients, n: i32) -> (f32, f32) {
    let x4 = x2 * x2;
    let x3 = x2 * x;
    let c2 = mul_add::<FUSED>(x2, p.c4, p.c3);
    let s1 = mul_add::<FUSED>(x2, p.s3, p.s2);
    let c1 = mul_add::<FUSED>(x2, p.c1, p.c0);
    let x5 = x3 * x2;
    let x6 = x4 * x2;
    let s = mul_add::<FUSED>(x3, p.s1, x);
    let c = mul_add::<FUSED>(x4, p.c2, c1);
    let sv = mul_add::<FUSED>(x5, s1, s) as f32;
    let cv = mul_add::<FUSED>(x6, c2, c) as f32;
    if n & 1 != 0 { (cv, sv) } else { (sv, cv) }
}

/// `reduce_fast` (no `TOINT_INTRINSICS`): `x` modulo π/2, in [-π/4, π/4], and its quadrant.
/// `hpi_inv` carries a factor 2^24, so the quadrant is in bits 24 to 31 of the truncated
/// product, rounded by adding half of that unit.
#[inline(always)]
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
#[inline(always)]
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

/// The argument reduced to [-π/4, π/4] with its sign for the quadrant, the polynomial table and
/// the quadrant, for finite `y` of magnitude at least π/4 (`top`: `abstop12(y)`).
#[inline(always)]
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
#[inline(always)]
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
        invalid_f32(y)
    }
}

/// `cosf(y)` of the FMA (`FUSED`) or SSE2 build.
#[inline(always)]
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
        invalid_f32(y)
    }
}

/// `sincosf(y)` of the FMA (`FUSED`) or SSE2 build: `(sin y, cos y)`.
#[inline(always)]
fn sincosf_build<const FUSED: bool>(y: f32) -> (f32, f32) {
    let top = abstop12(y);
    if top < TOP12_PIO4 {
        if top < TOP12_TINY {
            // Below 2^-126 Arm forces an underflow exception; the values are `y` and 1.
            return (y, 1.0);
        }
        let x = f64::from(y);
        sincosf_poly::<FUSED>(x, x * x, &SINCOSF_TABLE[0], 0)
    } else if top < TOP12_INF {
        let (x, p, n) = reduced::<FUSED>(y, top);
        sincosf_poly::<FUSED>(x, x * x, p, n)
    } else {
        // `*sinp = *cosp = y - y`: SSE's default NaN for an infinity, the NaN quieted for a NaN.
        let v = sub_f32(y, y);
        (v, v)
    }
}

/// glibc 2.35's `sinf(y)` on x86-64 (the FMA build, `__sinf_fma`), with its bits on every
/// platform: `±0` and every `|y| < 2^-12` as given, SSE's default NaN (`0xffc00000`) for `±∞`, a
/// NaN quieted.
#[inline(always)]
#[must_use]
pub fn sinf_glibc(y: f32) -> f32 {
    sinf_build::<true>(y)
}

/// glibc 2.35's `cosf(y)` on x86-64 (the FMA build, `__cosf_fma`), with its bits on every
/// platform: 1 for every `|y| < 2^-12`, SSE's default NaN for `±∞`, a NaN quieted.
#[inline(always)]
#[must_use]
pub fn cosf_glibc(y: f32) -> f32 {
    cosf_build::<true>(y)
}

/// glibc 2.35's `sincosf(y)` on x86-64 (the FMA build, `__sincosf_fma`): `(sin y, cos y)`, the
/// same bits as [`sinf_glibc`] and [`cosf_glibc`].
#[inline(always)]
#[must_use]
pub fn sincosf_glibc(y: f32) -> (f32, f32) {
    sincosf_build::<true>(y)
}

/// glibc 2.35's `sinf` as its x86-64 build without FMA runs it (`__sinf_sse2`, on x86-64 CPUs
/// without FMA): each product and sum rounded. It differs from [`sinf_glibc`] on 12 inputs.
#[inline]
#[must_use]
pub fn sinf_glibc_sse2(y: f32) -> f32 {
    sinf_build::<false>(y)
}

/// glibc 2.35's `cosf` without FMA (`__cosf_sse2`); it differs from [`cosf_glibc`] on 22 inputs.
#[inline]
#[must_use]
pub fn cosf_glibc_sse2(y: f32) -> f32 {
    cosf_build::<false>(y)
}

/// glibc 2.35's `sincosf` without FMA (`__sincosf_sse2`).
#[inline]
#[must_use]
pub fn sincosf_glibc_sse2(y: f32) -> (f32, f32) {
    sincosf_build::<false>(y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_the_c_literals() {
        assert_eq!(SINCOSF_TABLE[0].hpi, std::f64::consts::FRAC_PI_2);
        assert_eq!(TOP12_PIO4, abstop12(std::f32::consts::FRAC_PI_4));
        assert_eq!(TOP12_FAST, abstop12(120.0));
        assert_eq!(TOP12_INF, abstop12(f32::INFINITY));
        assert_eq!(PI63, 2.0 * std::f64::consts::PI * (-64.0f64).exp2());
    }

    #[test]
    fn special_values() {
        assert_eq!(sinf_glibc(0.0).to_bits(), 0.0f32.to_bits());
        assert_eq!(sinf_glibc(-0.0).to_bits(), (-0.0f32).to_bits());
        assert_eq!(cosf_glibc(-0.0), 1.0);
        let tiny = f32::from_bits(0x3970_0000);
        assert_eq!(sinf_glibc(tiny), tiny);
        assert_eq!(cosf_glibc(tiny), 1.0);
        assert_eq!(sincosf_glibc(1e-30), (1e-30, 1.0));
        // SSE's default NaN for an infinity, the operand quieted for a NaN, on every platform.
        for v in [f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(sinf_glibc(v).to_bits(), 0xffc0_0000);
            assert_eq!(cosf_glibc(v).to_bits(), 0xffc0_0000);
            let (s, c) = sincosf_glibc(v);
            assert_eq!((s.to_bits(), c.to_bits()), (0xffc0_0000, 0xffc0_0000));
            assert_eq!(sinf_glibc_sse2(v).to_bits(), 0xffc0_0000);
        }
        let snan = f32::from_bits(0x7fa0_0001);
        assert_eq!(sinf_glibc(snan).to_bits(), 0x7fe0_0001);
        assert_eq!(cosf_glibc(snan).to_bits(), 0x7fe0_0001);
        assert_eq!(sincosf_glibc(snan).1.to_bits(), 0x7fe0_0001);
    }

    /// glibc 2.35's values (FMA build, Linux x86-64) at the branch limits and where it is not
    /// correctly rounded: `(input, sinf, cosf)` as bits (the AFNI replica's vectors, from
    /// `crates-gpl/tools/sincosf-verify --vectors 4`, and the FSL replica core's).
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
            assert_eq!(sinf_glibc(x).to_bits(), s, "sinf({x:e})");
            assert_eq!(cosf_glibc(x).to_bits(), c, "cosf({x:e})");
            let (ss, cc) = sincosf_glibc(x);
            assert_eq!((ss.to_bits(), cc.to_bits()), (s, c), "sincosf({x:e})");
        }
    }

    /// A hash of every 4099th input of the three functions, recorded from glibc 2.35 on Linux
    /// x86-64 (FMA build) by `validation/investigations/glibc-maths/`; the SSE2 build gives the
    /// same results there.
    #[test]
    fn matches_glibc_on_a_stride() {
        let mut h = [0xcbf2_9ce4_8422_2325u64; 6];
        let mut x = 0u64;
        while x <= u64::from(u32::MAX) {
            let y = f32::from_bits(x as u32);
            let (s, c) = sincosf_glibc(y);
            let both = [
                sinf_glibc(y),
                cosf_glibc(y),
                s,
                c,
                sinf_glibc_sse2(y),
                cosf_glibc_sse2(y),
            ];
            for (k, v) in both.into_iter().enumerate() {
                h[k] = (h[k] ^ u64::from(v.to_bits())).wrapping_mul(0x0100_0000_01b3);
            }
            x += 4099;
        }
        assert_eq!(h[0], h[2], "sincosf's sine is sinf's");
        assert_eq!(h[1], h[3], "sincosf's cosine is cosf's");
        assert_eq!([h[0], h[1]], [STRIDE_SINF, STRIDE_COSF]);
        assert_eq!([h[4], h[5]], [STRIDE_SINF, STRIDE_COSF]);
    }

    const STRIDE_SINF: u64 = 0xcdf9_edc2_b57b_be29;
    const STRIDE_COSF: u64 = 0x7a47_3cd6_7a5d_aa12;

    /// The first of the 34 inputs where glibc's SSE2 build differs from its FMA build (12 of
    /// `sinf`, 22 of `cosf`; `validation/investigations/glibc-maths/`, the SSE2 build proved
    /// equal to the AFNI replicas' earlier translation, itself proved against glibc's SSE2 build).
    #[test]
    fn the_two_builds_differ_where_measured() {
        for x in [
            0x418a_3adb_u32,
            0x418a_3adc,
            0x418a_3add,
            0x418a_3ade,
            0x41bc_76d9,
            0x4202_eb4b,
            0x4255_b0a9,
            0x4268_7a55,
        ] {
            let y = f32::from_bits(x);
            let fma = (sinf_glibc(y).to_bits(), cosf_glibc(y).to_bits());
            let sse2 = (sinf_glibc_sse2(y).to_bits(), cosf_glibc_sse2(y).to_bits());
            assert_ne!(fma, sse2, "{x:#010x}");
            let (s, c) = sincosf_glibc_sse2(y);
            assert_eq!((s.to_bits(), c.to_bits()), sse2, "{x:#010x}");
        }
    }
}
