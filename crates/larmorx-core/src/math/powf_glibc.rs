// SPDX-License-Identifier: Apache-2.0 AND MIT
// Translated to Rust from Arm's Optimized Routines v19.11 (github.com/ARM-software/optimized-routines,
// tag v19.11): math/powf.c and math/powf_log2_data.c (__powf_log2_data), with math/exp2f_data.c's
// table (in `expf_glibc`). Copyright (c) 2017-2018, Arm Limited, under the MIT licence (its text
// and notice are in LICENSES/MIT-arm-optimized-routines.txt and NOTICE).
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
//! The single-precision power with the bits of glibc 2.35's `powf` on x86-64 (the FMA build,
//! `__powf_fma`, Arm's `powf.c` compiled with `-mfma -mavx2`; `WANT_ROUNDING`, `WANT_ERRNO`,
//! `WANT_ERRNO_UFLOW`), on every platform.
//!
//! GCC fuses the six multiply-adds of its `log2_inline` and the three of its `exp2_inline` in
//! that build (read from `objdump -d` of Ubuntu 22.04's libm.so.6); each is a `mul_add` here,
//! correctly rounded on every platform, and the NaN results are SSE's (`super::x86_nan`). Not
//! correctly rounded (Arm: 0.82 ulp); [`super::powf`] is, and is larmorx's default. This one
//! serves `reproduce` (FSL's `fslmaths -tfce`: TFCE's `powf(h, H)` and `powf(n, E)`). Until
//! 2026-11-04 the FSL replicas' core carried its own translation (`larmorx-nc-core`'s
//! `glibc_float`); it now calls this one. Proved equal to the system's `powf` (glibc 2.35,
//! x86-64) and to that translation on more than 10^9 pairs, NaN bits included
//! (`validation/investigations/glibc-maths/`).
//!
//! The older symbol FSL's binaries bind (`powf@GLIBC_2.2.5`, the SVID-compatible wrapper) gives
//! the same values: its replacements (the overflow's and pole's `±HUGE_VAL`, the underflow's
//! `±0`, `zero / zero` for a negative number to a non-integer power, SSE's `-nan`) are this
//! function's own.

use super::expf_glibc::exp2f_data;
use super::x86_nan::{add_f32, div_f32, invalid_f32, mul_f32};

/// `__powf_log2_data` (Arm's `powf_log2_data.c`, `POWF_LOG2_TABLE_BITS` 4; `POWF_SCALE` 1 on
/// x86-64, which has no `TOINT_INTRINSICS`): `(invc, logc)` per subinterval and the polynomial,
/// as 64-bit patterns (every word found as one block in Ubuntu 22.04's libm.so.6, glibc
/// 2.35-0ubuntu3: `validation/investigations/fsl-maths/arm_glibc_powf.py`).
#[rustfmt::skip]
mod powf_log2_data {
    /// `tab`.
    pub(super) const TAB: [(u64, u64); 16] = [
        (0x3ff661ec79f8f3be, 0xbfdefec65b963019), (0x3ff571ed4aaf883d, 0xbfdb0b6832d4fca4),
        (0x3ff49539f0f010b0, 0xbfd7418b0a1fb77b), (0x3ff3c995b0b80385, 0xbfd39de91a6dcf7b),
        (0x3ff30d190c8864a5, 0xbfd01d9bf3f2b631), (0x3ff25e227b0b8ea0, 0xbfc97c1d1b3b7af0),
        (0x3ff1bb4a4a1a343f, 0xbfc2f9e393af3c9f), (0x3ff12358f08ae5ba, 0xbfb960cbbf788d5c),
        (0x3ff0953f419900a7, 0xbfaa6f9db6475fce), (0x3ff0000000000000, 0x0000000000000000),
        (0x3fee608cfd9a47ac, 0x3fb338ca9f24f53d), (0x3feca4b31f026aa0, 0x3fc476a9543891ba),
        (0x3feb2036576afce6, 0x3fce840b4ac4e4d2), (0x3fe9c2d163a1aa2d, 0x3fd40645f0c6651c),
        (0x3fe886e6037841ed, 0x3fd88e9c2c1b9ff8), (0x3fe767dcf5534862, 0x3fdce0a44eb17bcc),
    ];
    /// `poly`.
    pub(super) const POLY: [u64; 5] = [
        0x3fd27616c9496e0b, 0xbfd71969a075c67a, 0x3fdec70a6ca7badd, 0xbfe7154748bef6c8,
        0x3ff71547652ab82b,
    ];
    /// `0x1.fffffffd1d571p+6`: the limit of `y log2 x` above which `|x^y|` overflows.
    pub(super) const OFLOW: u64 = 0x405fffffffd1d571;
}

/// `log2_inline(ix)`: `log2(x)` in double for the representation `ix` of a positive normal (or
/// normalised subnormal) `x`. Its six multiply-adds fused.
#[inline(always)]
fn log2_inline(ix: u32) -> f64 {
    use powf_log2_data::{POLY, TAB};
    const OFF: u32 = 0x3f33_0000;
    let tmp = ix.wrapping_sub(OFF);
    let i = ((tmp >> (23 - 4)) % 16) as usize;
    let top = tmp & 0xff80_0000;
    let iz = ix.wrapping_sub(top);
    let k = (top as i32) >> 23;
    let invc = f64::from_bits(TAB[i].0);
    let logc = f64::from_bits(TAB[i].1);
    let z = f64::from(f32::from_bits(iz));
    let a = |n: usize| f64::from_bits(POLY[n]);
    let r = z.mul_add(invc, -1.0);
    let y0 = logc + f64::from(k);
    let r2 = r * r;
    let y = a(0).mul_add(r, a(1));
    let p = a(2).mul_add(r, a(3));
    let r4 = r2 * r2;
    let q = a(4).mul_add(r, y0);
    let q = p.mul_add(r2, q);
    y.mul_add(r4, q)
}

/// `exp2_inline(xd, sign_bias)` (no `TOINT_INTRINSICS`): `2^xd` rounded to float, negated when
/// `sign_bias` holds `SIGN_BIAS`. Three multiply-adds fused.
#[inline(always)]
fn exp2_inline(xd: f64, sign_bias: u32) -> f32 {
    use exp2f_data::{POLY, SHIFT_SCALED, TAB};
    let shift = f64::from_bits(SHIFT_SCALED);
    let kd = xd + shift;
    let ki = kd.to_bits();
    let kd = kd - shift;
    let r = xd - kd;
    let ski = ki.wrapping_add(u64::from(sign_bias));
    let t = TAB[(ki % 32) as usize].wrapping_add(ski << (52 - 5));
    let s = f64::from_bits(t);
    let c = |n: usize| f64::from_bits(POLY[n]);
    let z = c(0).mul_add(r, c(1));
    let r2 = r * r;
    let y = c(2).mul_add(r, 1.0);
    let y = z.mul_add(r2, y);
    (y * s) as f32
}

/// `checkint(iy)`: 0 if `y` is not an integer, 1 if an odd one, 2 if an even one (`y` finite and
/// not zero).
#[inline(always)]
fn checkint(iy: u32) -> i32 {
    let e = ((iy >> 23) & 0xff) as i32;
    if e < 0x7f {
        return 0;
    }
    if e > 0x7f + 23 {
        return 2;
    }
    let sh = (0x7f + 23 - e) as u32;
    if iy & ((1u32 << sh) - 1) != 0 {
        return 0;
    }
    if iy & (1u32 << sh) != 0 {
        return 1;
    }
    2
}

/// `zeroinfnan(i)`: the representation of 0, an infinity or a NaN.
#[inline(always)]
fn zeroinfnan(i: u32) -> bool {
    i.wrapping_mul(2).wrapping_sub(1) >= 2u32.wrapping_mul(0x7f80_0000).wrapping_sub(1)
}

/// `issignalingf_inline(x)` with IEEE 754-2008 signalling NaNs (x86-64).
#[inline(always)]
fn issignalingf(x: f32) -> bool {
    (x.to_bits() ^ 0x0040_0000).wrapping_mul(2) > 2u32.wrapping_mul(0x7fc0_0000)
}

/// glibc 2.35's `powf(x, y)` on x86-64 (the FMA build), with its bits on every platform.
#[inline(always)]
#[must_use]
pub fn powf_glibc(x: f32, y: f32) -> f32 {
    const SIGN_BIAS: u32 = 1 << (5 + 11);
    let mut sign_bias: u32 = 0;
    let mut ix = x.to_bits();
    let iy = y.to_bits();
    if ix.wrapping_sub(0x0080_0000) >= 0x7f80_0000 - 0x0080_0000 || zeroinfnan(iy) {
        // Either (x < 0x1p-126 or inf or nan) or (y is 0 or inf or nan).
        if zeroinfnan(iy) {
            if iy.wrapping_mul(2) == 0 {
                return if issignalingf(x) { add_f32(x, y) } else { 1.0 };
            }
            if ix == 0x3f80_0000 {
                return if issignalingf(y) { add_f32(x, y) } else { 1.0 };
            }
            if ix.wrapping_mul(2) > 2u32.wrapping_mul(0x7f80_0000)
                || iy.wrapping_mul(2) > 2u32.wrapping_mul(0x7f80_0000)
            {
                return add_f32(x, y);
            }
            if ix.wrapping_mul(2) == 2 * 0x3f80_0000 {
                return 1.0;
            }
            if (ix.wrapping_mul(2) < 2 * 0x3f80_0000) == (iy & 0x8000_0000 == 0) {
                // |x| < 1 && y == inf or |x| > 1 && y == -inf
                return 0.0;
            }
            return y * y;
        }
        if zeroinfnan(ix) {
            let mut x2 = mul_f32(x, x);
            if ix & 0x8000_0000 != 0 && checkint(iy) == 1 {
                x2 = -x2;
                sign_bias = 1;
            }
            if ix.wrapping_mul(2) == 0 && iy & 0x8000_0000 != 0 {
                // __math_divzerof(sign_bias)
                return if sign_bias != 0 {
                    f32::NEG_INFINITY
                } else {
                    f32::INFINITY
                };
            }
            return if iy & 0x8000_0000 != 0 {
                div_f32(1.0, x2)
            } else {
                x2
            };
        }
        // x and y are non-zero finite.
        if ix & 0x8000_0000 != 0 {
            let yint = checkint(iy);
            if yint == 0 {
                return invalid_f32(x);
            }
            if yint == 1 {
                sign_bias = SIGN_BIAS;
            }
            ix &= 0x7fff_ffff;
        }
        if ix < 0x0080_0000 {
            // A subnormal x, normalised so that its exponent is negative.
            ix = (x * f32::from_bits(0x4b00_0000)).to_bits();
            ix &= 0x7fff_ffff;
            ix = ix.wrapping_sub(23 << 23);
        }
    }
    let logx = log2_inline(ix);
    let ylogx = f64::from(y) * logx;
    if ((ylogx.to_bits() >> 47) & 0xffff) >= (126.0f64.to_bits() >> 47) {
        // |y * log(x)| >= 126
        let sign = |v: f32| if sign_bias != 0 { -v } else { v };
        if ylogx > f64::from_bits(powf_log2_data::OFLOW) {
            // __math_oflowf: 0x1p97f * 0x1p97f
            return sign(f32::from_bits(0x7000_0000)) * f32::from_bits(0x7000_0000);
        }
        // Above 0x1.fffffffa3aae2p+6 (|x^y| > 0x1.fffffep127) Arm's code tests whether the
        // rounding mode rounds away from 0: never in round-to-nearest, so nothing happens.
        if ylogx <= -150.0 {
            // __math_uflowf: 0x1p-95f * 0x1p-95f
            return sign(f32::from_bits(0x1000_0000)) * f32::from_bits(0x1000_0000);
        }
        if ylogx < -149.0 {
            // __math_may_uflowf: 0x1.4p-75f * 0x1.4p-75f
            return sign(f32::from_bits(0x1a20_0000)) * f32::from_bits(0x1a20_0000);
        }
    }
    exp2_inline(ylogx, sign_bias)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// glibc 2.35's values on Linux x86-64 (FMA build): the main path, a negative base, poles, a
    /// subnormal base, overflow, underflow.
    #[test]
    fn known_values() {
        let cases: [(f32, f32, u32); 11] = [
            (2.0, 0.5, 0x3fb5_04f3),
            (3.7, 2.0, 0x415b_0a3e),
            (0.04, 2.5, 0x39a7_c5ac),
            (1234.0, 0.6667, 0x42e6_263e),
            (-8.0, 3.0, 0xc400_0000),
            (-8.0, 0.5, 0xffc0_0000),
            (0.0, -1.0, 0x7f80_0000),
            (-0.0, -3.0, 0xff80_0000),
            (1e-40, 0.5, 0x1e3c_e4e7),
            (10.0, 40.0, 0x7f80_0000),
            (10.0, -50.0, 0),
        ];
        for (x, y, want) in cases {
            assert_eq!(powf_glibc(x, y).to_bits(), want, "powf({x:e}, {y:e})");
        }
        assert_eq!(powf_glibc(f32::NAN, 0.0), 1.0);
        assert_eq!(powf_glibc(1.0, f32::NAN), 1.0);
        assert_eq!(powf_glibc(0.5, f32::INFINITY), 0.0);
        assert_eq!(powf_glibc(2.0, f32::INFINITY), f32::INFINITY);
        // Two NaNs: x's, quieted (SSE's `x + y`).
        let (a, b) = (f32::from_bits(0x7fa0_0001), f32::from_bits(0xffc0_0002));
        assert_eq!(powf_glibc(a, b).to_bits(), 0x7fe0_0001);
        assert_eq!(powf_glibc(b, a).to_bits(), 0xffc0_0002);
        assert_eq!(powf_glibc(b, 2.0).to_bits(), 0xffc0_0002);
    }
}
