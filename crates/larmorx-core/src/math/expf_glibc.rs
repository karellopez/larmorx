// SPDX-License-Identifier: Apache-2.0 AND MIT
// Translated to Rust from Arm's Optimized Routines v19.11 (github.com/ARM-software/optimized-routines,
// tag v19.11): math/expf.c and math/exp2f_data.c (__exp2f_data). Copyright (c) 2017-2018, Arm
// Limited, under the MIT licence (its text and notice are in LICENSES/MIT-arm-optimized-routines.txt
// and NOTICE). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
//! The single-precision exponential with the bits of glibc 2.35's `expf` on x86-64 (the FMA
//! build, `__expf_fma`, Arm's `expf.c` compiled with `-mfma -mavx2`), on every platform.
//!
//! GCC fuses five multiply-adds of that build (read from `objdump -d` of Ubuntu 22.04's
//! libm.so.6, glibc 2.35-0ubuntu3): the three of the polynomial and both uses of `InvLn2N * xd`
//! (`z + SHIFT` and `z - kd`), so `r` is `InvLn2N * xd - kd` rounded once. Each is a `mul_add`
//! here, correctly rounded on every platform; the NaN results are SSE's (`super::x86_nan`). Not
//! correctly rounded (Arm: 0.502 ulp; 170,648 of the finite floats); [`super::expf`] is, and is
//! larmorx's default. This one serves `reproduce` (FSL's FAST: its Gaussian energies). Until
//! 2026-11-04 the FSL replicas' core carried its own translation (`larmorx-nc-core`'s
//! `glibc_float`); it now calls this one. Proved equal to the system's `expf` (glibc 2.35,
//! x86-64) and to that translation on all 2^32 inputs, NaN bits included
//! (`validation/investigations/glibc-maths/`).
//!
//! The limits as glibc builds them (`WANT_ERRNO_UFLOW`): below `-0x1.9fe368p6` the result is
//! `+0` (`__math_uflowf`), down to `-0x1.9d1d9ep6` it is `0x1.4p-75f * 0x1.4p-75f`, the smallest
//! subnormal (`__math_may_uflowf`), above `0x1.62e42ep6` infinity (`__math_oflowf`); `-inf`
//! gives `+0`, a NaN or `+inf` `x + x`.

use super::x86_nan::add_f32;

/// `__exp2f_data` (Arm's `exp2f_data.c`, `EXP2F_TABLE_BITS` 5, `EXP2F_POLY_ORDER` 3): the parts
/// `expf` and `powf` read, as 64-bit patterns (every word identical to Ubuntu 22.04's libm.so.6,
/// glibc 2.35-0ubuntu3: `validation/investigations/fsl-fast/arm_glibc_expf_log.py`).
#[rustfmt::skip]
pub(super) mod exp2f_data {
    /// `tab`: `2^(i/32)` with the bits `i << 47` subtracted, so that adding `k << 47` scales it.
    pub(in super::super) const TAB: [u64; 32] = [
        0x3ff0000000000000, 0x3fefd9b0d3158574, 0x3fefb5586cf9890f, 0x3fef9301d0125b51,
        0x3fef72b83c7d517b, 0x3fef54873168b9aa, 0x3fef387a6e756238, 0x3fef1e9df51fdee1,
        0x3fef06fe0a31b715, 0x3feef1a7373aa9cb, 0x3feedea64c123422, 0x3feece086061892d,
        0x3feebfdad5362a27, 0x3feeb42b569d4f82, 0x3feeab07dd485429, 0x3feea47eb03a5585,
        0x3feea09e667f3bcd, 0x3fee9f75e8ec5f74, 0x3feea11473eb0187, 0x3feea589994cce13,
        0x3feeace5422aa0db, 0x3feeb737b0cdc5e5, 0x3feec49182a3f090, 0x3feed503b23e255d,
        0x3feee89f995ad3ad, 0x3feeff76f2fb5e47, 0x3fef199bdd85529c, 0x3fef3720dcef9069,
        0x3fef5818dcfba487, 0x3fef7c97337b9b5f, 0x3fefa4afa2a490da, 0x3fefd0765b6e4540,
    ];
    /// `shift` (0x1.8p+52).
    pub(in super::super) const SHIFT: u64 = 0x4338000000000000;
    /// `shift_scaled` (0x1.8p+52 / 32).
    pub(in super::super) const SHIFT_SCALED: u64 = 0x42e8000000000000;
    /// `invln2_scaled` (0x1.71547652b82fep+0 * 32).
    pub(in super::super) const INVLN2_SCALED: u64 = 0x40471547652b82fe;
    /// `poly`.
    pub(in super::super) const POLY: [u64; 3] = [0x3fac6af84b912394, 0x3fcebfce50fac4f3, 0x3fe62e42ff0c52d6];
    /// `poly_scaled`.
    pub(in super::super) const POLY_SCALED: [u64; 3] = [0x3ebc6af84b912394, 0x3f2ebfce50fac4f3, 0x3f962e42ff0c52d6];
}

/// glibc 2.35's `expf(x)` on x86-64 (the FMA build), with its bits on every platform.
#[inline(always)]
#[must_use]
pub fn expf_glibc(x: f32) -> f32 {
    use exp2f_data::{INVLN2_SCALED, POLY_SCALED, SHIFT, TAB};
    let xd = f64::from(x);
    let abstop = (x.to_bits() >> 20) & 0x7ff;
    if abstop >= 0x42b {
        // |x| >= 88 or x is NaN
        if x.to_bits() == f32::NEG_INFINITY.to_bits() {
            return 0.0;
        }
        if abstop >= 0x7f8 {
            return add_f32(x, x);
        }
        if x > f32::from_bits(0x42b1_7217) {
            // __math_oflowf(0): 0x1p97f * 0x1p97f
            return f32::from_bits(0x7000_0000) * f32::from_bits(0x7000_0000);
        }
        if x < f32::from_bits(0xc2cf_f1b4) {
            // __math_uflowf(0): 0x1p-95f * 0x1p-95f
            return f32::from_bits(0x1000_0000) * f32::from_bits(0x1000_0000);
        }
        if x < f32::from_bits(0xc2ce_8ecf) {
            // __math_may_uflowf(0): 0x1.4p-75f * 0x1.4p-75f
            return f32::from_bits(0x1a20_0000) * f32::from_bits(0x1a20_0000);
        }
    }
    let invln2n = f64::from_bits(INVLN2_SCALED);
    let shift = f64::from_bits(SHIFT);
    // kd = z + SHIFT with z = InvLn2N * xd, fused
    let mut kd = invln2n.mul_add(xd, shift);
    let ki = kd.to_bits();
    kd -= shift;
    // r = z - kd, fused
    let r = invln2n.mul_add(xd, -kd);
    let t = TAB[(ki % 32) as usize].wrapping_add(ki << (52 - 5));
    let s = f64::from_bits(t);
    let c = |n: usize| f64::from_bits(POLY_SCALED[n]);
    let z = c(0).mul_add(r, c(1));
    let r2 = r * r;
    let y = c(2).mul_add(r, 1.0);
    let y = z.mul_add(r2, y);
    (y * s) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_values() {
        assert_eq!(expf_glibc(0.0), 1.0);
        assert_eq!(expf_glibc(-0.0), 1.0);
        assert_eq!(expf_glibc(f32::NEG_INFINITY).to_bits(), 0);
        assert_eq!(expf_glibc(f32::INFINITY), f32::INFINITY);
        assert_eq!(expf_glibc(89.0), f32::INFINITY);
        assert_eq!(expf_glibc(-104.0).to_bits(), 0);
        assert_eq!(expf_glibc(-103.5).to_bits(), 1);
        assert_eq!(
            expf_glibc(f32::from_bits(0x7fa0_0001)).to_bits(),
            0x7fe0_0001
        );
        assert_eq!(
            expf_glibc(f32::from_bits(0xffc0_0003)).to_bits(),
            0xffc0_0003
        );
        for x in [-80.0f32, -1.5, -1e-3, 0.5, 1.0, 10.0, 88.0] {
            let want = f64::from(x).exp() as f32;
            let got = expf_glibc(x);
            assert!((got - want).abs() <= 2.0 * f32::EPSILON * want.abs(), "{x}");
        }
    }

    /// A hash of every 4099th input, recorded from glibc 2.35 on Linux x86-64
    /// (`validation/investigations/glibc-maths/`).
    #[test]
    fn matches_glibc_on_a_stride() {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut x = 0u64;
        while x <= u64::from(u32::MAX) {
            let v = expf_glibc(f32::from_bits(x as u32));
            h = (h ^ u64::from(v.to_bits())).wrapping_mul(0x0100_0000_01b3);
            x += 4099;
        }
        assert_eq!(h, STRIDE);
    }

    const STRIDE: u64 = 0xc7b6_6f60_6a70_5163;
}
