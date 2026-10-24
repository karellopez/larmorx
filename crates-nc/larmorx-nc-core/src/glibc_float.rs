// SPDX-License-Identifier: LicenseRef-FSL AND MIT
// Translated to Rust from Arm's Optimized Routines v19.11 (math/sincosf.h, sincosf.c, sinf.c,
// cosf.c, sincosf_data.c, logf.c, logf_data.c), Copyright (c) 2017-2018, Arm Limited,
// SPDX-License-Identifier: MIT (the notice is in LICENSES/MIT-arm-optimized-routines.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
// Distributed as part of larmorx-nc under the FSL Licence (see LICENSE) and the MIT terms.
//! The single-precision `sinf`, `cosf`, `sincosf` and `logf` that FSL's Linux binaries call:
//! glibc 2.35's, as its x86-64 build selects them on a CPU with FMA (`__sinf_fma`,
//! `__cosf_fma`, `__sincosf_fma`, `__logf_fma`, compiled with `-mfma -mavx2`).
//!
//! glibc took these functions from Arm's Optimized Routines (2018) and kept their arithmetic;
//! this module is translated from Arm's MIT-licensed originals, not from glibc. GCC contracts
//! every `a + b * c` of them into one fused multiply-add in the FMA build; the translation
//! writes each as `mul_add`, which is correctly rounded on every platform, so the results are
//! the same bits everywhere. `tools/glibc-float-verify` compares them with the C library on all
//! 2^32 inputs.
//!
//! They are not correctly rounded (sinf and cosf to 0.56 ulp, logf to 0.82 ulp): mcflirt's
//! results as built depend on their roundings (`docs/findings/fsl-mcflirt.md`), so the
//! replica uses them where FSL calls the C library's float functions.

/// `sincos_t`: the signs, the reduction constants and the two polynomials.
struct SinCos {
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

/// `__sincosf_table`: the second entry computes `-cos` (its cosine polynomial negated).
/// `hpi_inv` is 2/pi scaled by 2^24 (no `TOINT_INTRINSICS` on x86-64).
const TABLE: [SinCos; 2] = [
    SinCos {
        sign: [1.0, -1.0, -1.0, 1.0],
        hpi_inv: hexf(0x1_45F3_06DC_9C883, 23),
        hpi: hexf(0x1_921F_B544_42D18, 0),
        c0: 1.0,
        c1: -hexf(0x1_ffff_ffd0_c621c, -2),
        c2: hexf(0x1_5555_3e10_68f19, -5),
        c3: -hexf(0x1_6c08_7e89_a359d, -10),
        c4: hexf(0x1_9934_3027_bf8c3, -16),
        s1: -hexf(0x1_5555_4599_5a603, -3),
        s2: hexf(0x1_1107_6052_30bc4, -7),
        s3: -hexf(0x1_994e_b377_4cf24, -13),
    },
    SinCos {
        sign: [1.0, -1.0, -1.0, 1.0],
        hpi_inv: hexf(0x1_45F3_06DC_9C883, 23),
        hpi: hexf(0x1_921F_B544_42D18, 0),
        c0: -1.0,
        c1: hexf(0x1_ffff_ffd0_c621c, -2),
        c2: -hexf(0x1_5555_3e10_68f19, -5),
        c3: hexf(0x1_6c08_7e89_a359d, -10),
        c4: -hexf(0x1_9934_3027_bf8c3, -16),
        s1: -hexf(0x1_5555_4599_5a603, -3),
        s2: hexf(0x1_1107_6052_30bc4, -7),
        s3: -hexf(0x1_994e_b377_4cf24, -13),
    },
];

/// `__inv_pio4`: 4/pi to 192 bits, 8 new bits per entry.
const INV_PIO4: [u32; 24] = [
    0xa2, 0xa2f9, 0xa2f983, 0xa2f9836e, 0xf9836e4e, 0x836e4e44, 0x6e4e4415, 0x4e441529, 0x441529fc,
    0x1529fc27, 0x29fc2757, 0xfc2757d1, 0x2757d1f5, 0x57d1f534, 0xd1f534dd, 0xf534ddc0, 0x34ddc0db,
    0xddc0db62, 0xc0db6295, 0xdb629599, 0x6295993c, 0x95993c43, 0x993c4390, 0x3c439041,
];

/// The double `1.mantissa * 2^exp`, `m` holding the leading 1 and the 52 fraction bits (a
/// C hexadecimal float literal `0x1.<13 hex digits>p<exp>`).
const fn hexf(m: u64, exp: i32) -> f64 {
    let frac = m & ((1u64 << 52) - 1);
    f64::from_bits((((1023 + exp) as u64) << 52) | frac)
}

/// 2pi * 2^-64.
const PI63: f64 = hexf(0x1_921F_B544_42D18, -62);

/// The top 12 bits of the float's representation, sign cleared.
#[inline]
fn abstop12(x: f32) -> u32 {
    (x.to_bits() >> 20) & 0x7ff
}

/// abstop12(pi/4): `pio4` is the double pi/4, rounded to float by the call.
const PIO4_TOP12: u32 = (0x3f49_0fdb_u32 >> 20) & 0x7ff;
/// abstop12(0x1p-12f), abstop12(0x1p-126f), abstop12(120.0f), abstop12(INFINITY).
const TOP12_2M12: u32 = (0x3980_0000_u32 >> 20) & 0x7ff;
const TOP12_2M126: u32 = (0x0080_0000_u32 >> 20) & 0x7ff;
const TOP12_120: u32 = (0x42f0_0000_u32 >> 20) & 0x7ff;
const TOP12_INF: u32 = (0x7f80_0000_u32 >> 20) & 0x7ff;

/// `sincosf_poly(x, x2, p, n, sinp, cosp)`: (sin, cos), swapped for an odd quadrant.
#[inline]
fn sincosf_poly(x: f64, x2: f64, p: &SinCos, n: i32) -> (f32, f32) {
    let x4 = x2 * x2;
    let x3 = x2 * x;
    let c2 = x2.mul_add(p.c4, p.c3);
    let s1 = x2.mul_add(p.s3, p.s2);
    let c1 = x2.mul_add(p.c1, p.c0);
    let x5 = x3 * x2;
    let x6 = x4 * x2;
    let s = x3.mul_add(p.s1, x);
    let c = x4.mul_add(p.c2, c1);
    let sv = x5.mul_add(s1, s) as f32;
    let cv = x6.mul_add(c2, c) as f32;
    if n & 1 != 0 { (cv, sv) } else { (sv, cv) }
}

/// `sinf_poly(x, x2, p, n)`: the sine polynomial for an even quadrant, the cosine's for an odd.
#[inline]
fn sinf_poly(x: f64, x2: f64, p: &SinCos, n: i32) -> f32 {
    if n & 1 == 0 {
        let x3 = x * x2;
        let s1 = x2.mul_add(p.s3, p.s2);
        let x7 = x3 * x2;
        let s = x3.mul_add(p.s1, x);
        x7.mul_add(s1, s) as f32
    } else {
        let x4 = x2 * x2;
        let c2 = x2.mul_add(p.c4, p.c3);
        let c1 = x2.mul_add(p.c1, p.c0);
        let x6 = x4 * x2;
        let c = x4.mul_add(p.c2, c1);
        x6.mul_add(c2, c) as f32
    }
}

/// `reduce_fast(x, p, &n)`: `x` modulo pi/2 and the quadrant, for |x| < 120.
#[inline]
fn reduce_fast(x: f64, p: &SinCos) -> (f64, i32) {
    let r = x * p.hpi_inv;
    // (int32_t)r: truncation; |r| < 2^31 here.
    let n = ((r as i32).wrapping_add(0x80_0000)) >> 24;
    ((-f64::from(n)).mul_add(p.hpi, x), n)
}

/// `reduce_large(xi, &n)`: the reduction by the 192-bit 4/pi for |x| >= 120.
#[inline]
fn reduce_large(xi: u32) -> (f64, i32) {
    let k = ((xi >> 26) & 15) as usize;
    let arr = &INV_PIO4[k..];
    let shift = (xi >> 23) & 7;
    let xi = ((xi & 0xff_ffff) | 0x80_0000) << shift;
    // res0 = xi * arr[0]: a 32-bit product, then widened.
    let mut res0 = u64::from(xi.wrapping_mul(arr[0]));
    let res1 = u64::from(xi) * u64::from(arr[4]);
    let res2 = u64::from(xi) * u64::from(arr[8]);
    res0 = (res2 >> 32) | (res0 << 32);
    res0 = res0.wrapping_add(res1);
    let n = (res0.wrapping_add(1u64 << 61)) >> 62;
    res0 = res0.wrapping_sub(n << 62);
    let x = res0 as i64 as f64;
    (x * PI63, n as i32)
}

/// glibc's `sinf` (x86-64, FMA build).
#[must_use]
pub fn sinf(y: f32) -> f32 {
    let x = f64::from(y);
    let top = abstop12(y);
    if top < PIO4_TOP12 {
        if top < TOP12_2M12 {
            return y;
        }
        return sinf_poly(x, x * x, &TABLE[0], 0);
    }
    if top < TOP12_120 {
        let (x, n) = reduce_fast(x, &TABLE[0]);
        let s = TABLE[0].sign[(n & 3) as usize];
        let p = if n & 2 != 0 { &TABLE[1] } else { &TABLE[0] };
        return sinf_poly(x * s, x * x, p, n);
    }
    if top < TOP12_INF {
        let xi = y.to_bits();
        let sign = (xi >> 31) as i32;
        let (x, n) = reduce_large(xi);
        let s = TABLE[0].sign[((n + sign) & 3) as usize];
        let p = if (n + sign) & 2 != 0 { &TABLE[1] } else { &TABLE[0] };
        return sinf_poly(x * s, x * x, p, n);
    }
    // __math_invalidf: (x - x) / (x - x), NaN for an infinity or a NaN.
    (y - y) / (y - y)
}

/// glibc's `cosf` (x86-64, FMA build).
#[must_use]
pub fn cosf(y: f32) -> f32 {
    let x = f64::from(y);
    let top = abstop12(y);
    if top < PIO4_TOP12 {
        if top < TOP12_2M12 {
            return 1.0;
        }
        return sinf_poly(x, x * x, &TABLE[0], 1);
    }
    if top < TOP12_120 {
        let (x, n) = reduce_fast(x, &TABLE[0]);
        let s = TABLE[0].sign[(n & 3) as usize];
        let p = if n & 2 != 0 { &TABLE[1] } else { &TABLE[0] };
        return sinf_poly(x * s, x * x, p, n ^ 1);
    }
    if top < TOP12_INF {
        let xi = y.to_bits();
        let sign = (xi >> 31) as i32;
        let (x, n) = reduce_large(xi);
        let s = TABLE[0].sign[((n + sign) & 3) as usize];
        let p = if (n + sign) & 2 != 0 { &TABLE[1] } else { &TABLE[0] };
        return sinf_poly(x * s, x * x, p, n ^ 1);
    }
    (y - y) / (y - y)
}

/// glibc's `sincosf` (x86-64, FMA build): `(sin y, cos y)`.
#[must_use]
pub fn sincosf(y: f32) -> (f32, f32) {
    let x = f64::from(y);
    let top = abstop12(y);
    if top < PIO4_TOP12 {
        if top < TOP12_2M12 {
            // Below 2^-126 glibc forces an underflow (no effect on the value).
            let _ = TOP12_2M126;
            return (y, 1.0);
        }
        return sincosf_poly(x, x * x, &TABLE[0], 0);
    }
    if top < TOP12_120 {
        let (x, n) = reduce_fast(x, &TABLE[0]);
        let s = TABLE[0].sign[(n & 3) as usize];
        let p = if n & 2 != 0 { &TABLE[1] } else { &TABLE[0] };
        return sincosf_poly(x * s, x * x, p, n);
    }
    if top < TOP12_INF {
        let xi = y.to_bits();
        let sign = (xi >> 31) as i32;
        let (x, n) = reduce_large(xi);
        let s = TABLE[0].sign[((n + sign) & 3) as usize];
        let p = if (n + sign) & 2 != 0 { &TABLE[1] } else { &TABLE[0] };
        return sincosf_poly(x * s, x * x, p, n);
    }
    let v = y - y;
    (v, v)
}

/// `__logf_data`: 16 table entries (1/c, log c), ln 2, and the polynomial.
const LOGF_TAB: [(f64, f64); 16] = [
    (hexf(0x1_661e_c79f_8f3be, 0), -hexf(0x1_57bf_7808_caade, -2)),
    (hexf(0x1_571e_d4aa_f883d, 0), -hexf(0x1_2bef_0a7c_06ddb, -2)),
    (hexf(0x1_4953_9f0f_010b0, 0), -hexf(0x1_01ea_e7f5_13a67, -2)),
    (hexf(0x1_3c99_5b0b_80385, 0), -hexf(0x1_b31d_8a68_224e9, -3)),
    (hexf(0x1_30d1_90c8_864a5, 0), -hexf(0x1_6574_f0ac_07758, -3)),
    (hexf(0x1_25e2_27b0_b8ea0, 0), -hexf(0x1_1aa2_bc79_c8100, -3)),
    (hexf(0x1_1bb4_a4a1_a343f, 0), -hexf(0x1_a4e7_6ce8_c0e5e, -4)),
    (hexf(0x1_1235_8f08_ae5ba, 0), -hexf(0x1_1973_c5a6_11ccc, -4)),
    (hexf(0x1_0953_f419_900a7, 0), -hexf(0x1_252f_438e_10c1e, -5)),
    (1.0, 0.0),
    (hexf(0x1_e608_cfd9_a47ac, -1), hexf(0x1_aa5a_a5df_25984, -5)),
    (hexf(0x1_ca4b_31f0_26aa0, -1), hexf(0x1_c5e5_3aa3_62eb4, -4)),
    (hexf(0x1_b203_6576_afce6, -1), hexf(0x1_526e_5772_0db08, -3)),
    (hexf(0x1_9c2d_163a_1aa2d, -1), hexf(0x1_bc28_60d2_24770, -3)),
    (hexf(0x1_886e_6037_841ed, -1), hexf(0x1_1058_bc8a_07ee1, -2)),
    (hexf(0x1_767d_cf55_34862, -1), hexf(0x1_4043_057b_6ee09, -2)),
];
const LOGF_LN2: f64 = hexf(0x1_62e4_2fef_a39ef, -1);
const LOGF_POLY: [f64; 3] = [
    -hexf(0x1_00ea_348b_88334, -2),
    hexf(0x1_5575_b0be_00b6a, -2),
    -hexf(0x1_ffff_ef20_a4123, -2),
];

/// glibc's `logf` (x86-64, FMA build).
#[must_use]
pub fn logf(x: f32) -> f32 {
    const OFF: u32 = 0x3f33_0000;
    let mut ix = x.to_bits();
    // WANT_ROUNDING: log(1) = +0 in every rounding mode.
    if ix == 0x3f80_0000 {
        return 0.0;
    }
    if ix.wrapping_sub(0x0080_0000) >= 0x7f80_0000 - 0x0080_0000 {
        if ix.wrapping_mul(2) == 0 {
            // __math_divzerof(1): -1/0.
            return f32::NEG_INFINITY;
        }
        if ix == 0x7f80_0000 {
            return x;
        }
        if (ix & 0x8000_0000) != 0 || ix.wrapping_mul(2) >= 0xff00_0000 {
            // __math_invalidf(x): (x - x) / (x - x).
            return (x - x) / (x - x);
        }
        // A subnormal: normalized.
        ix = (x * f32::from_bits(0x4b00_0000)).to_bits();
        ix = ix.wrapping_sub(23 << 23);
    }
    let tmp = ix.wrapping_sub(OFF);
    let i = ((tmp >> (23 - 4)) % 16) as usize;
    let k = (tmp as i32) >> 23;
    let iz = ix.wrapping_sub(tmp & (0x1ff << 23));
    let (invc, logc) = LOGF_TAB[i];
    let z = f64::from(f32::from_bits(iz));
    let r = z.mul_add(invc, -1.0);
    let y0 = f64::from(k).mul_add(LOGF_LN2, logc);
    let r2 = r * r;
    let mut y = LOGF_POLY[1].mul_add(r, LOGF_POLY[2]);
    y = LOGF_POLY[0].mul_add(r2, y);
    y = y.mul_add(r2, y0 + r);
    y as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_the_c_literals() {
        assert_eq!(hexf(0x1_921F_B544_42D18, 0), std::f64::consts::FRAC_PI_2);
        assert_eq!(hexf(0x1_62e4_2fef_a39ef, -1), std::f64::consts::LN_2);
        assert_eq!(PIO4_TOP12, abstop12(std::f32::consts::FRAC_PI_4));
        assert_eq!(TOP12_120, abstop12(120.0));
        assert_eq!(TOP12_2M12, abstop12(f32::from_bits(0x3980_0000)));
        assert_eq!(TOP12_INF, abstop12(f32::INFINITY));
    }

    #[test]
    fn special_values() {
        assert!(sinf(f32::INFINITY).is_nan());
        assert!(cosf(f32::NAN).is_nan());
        assert_eq!(sinf(-0.0).to_bits(), (-0.0f32).to_bits());
        assert_eq!(cosf(0.0), 1.0);
        assert_eq!(logf(1.0).to_bits(), 0);
        assert_eq!(logf(0.0), f32::NEG_INFINITY);
        assert!(logf(-1.0).is_nan());
        assert_eq!(logf(f32::INFINITY), f32::INFINITY);
        let (s, c) = sincosf(1e-30);
        assert_eq!((s, c), (1e-30, 1.0));
    }

    /// Values glibc 2.35 (FMA build) returns, from the verification tool on Linux x86-64; among
    /// them inputs where it is not correctly rounded.
    #[test]
    fn known_values() {
        let near = |a: f32, b: f32| (a - b).abs() <= 2.0 * f32::EPSILON * b.abs().max(1e-30);
        for x in [0.3f32, 1.0, 2.5, 100.0, 1e6, -7.25] {
            let (s, c) = sincosf(x);
            assert_eq!(s.to_bits(), sinf(x).to_bits(), "{x}");
            assert_eq!(c.to_bits(), cosf(x).to_bits(), "{x}");
            assert!(near(s, f64::from(x).sin() as f32), "{x}");
            assert!(near(c, f64::from(x).cos() as f32), "{x}");
        }
        for x in [1e-30f32, 0.5, 1.0, 2.0, 1579.0, 1e30] {
            assert!(near(logf(x), f64::from(x).ln() as f32), "{x}");
        }
    }
}
