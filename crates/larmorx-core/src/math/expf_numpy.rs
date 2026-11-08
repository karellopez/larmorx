// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
// Translated to Rust from numpy 2.5.4 (github.com/numpy/numpy, tag v2.5.4; the same file in
// v2.3.5): numpy/_core/src/umath/loops_exponent_log.dispatch.c.src (`simd_exp_FLOAT`, its helpers
// `simd_range_reduction`, `fma_scalef_ps`, `avx512_scalef_ps`) and the constants of
// numpy/_core/src/umath/npy_simd_data.h (`NPY_COEFF_*_EXPf`, `NPY_CODY_WAITE_LOGE_2_*f`,
// `NPY_RINT_CVT_MAGICf`) and numpy/_core/include/numpy/npy_math.h (`NPY_LOG2Ef`). Copyright (c)
// 2005-2025, NumPy Developers, under the BSD-3-Clause licence (notice below). Rust translation
// Copyright 2026 Karel Lopez Vilaret; modified from the original (one lane of the vector code).
//
// numpy's notice: Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met: redistributions
// of source code must retain the above copyright notice, this list of conditions and the
// following disclaimer; redistributions in binary form must reproduce the above copyright
// notice, this list of conditions and the following disclaimer in the documentation and/or
// other materials provided with the distribution; neither the name of the NumPy Developers nor
// the names of any contributors may be used to endorse or promote products derived from this
// software without specific prior written permission. THIS SOFTWARE IS PROVIDED BY THE
// COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING,
// BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
// PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
// LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR
// BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//! numpy's single-precision exponential (`numpy.exp` on a `float32` array) as numpy's x86-64
//! builds compute it on CPUs with AVX2 and FMA3 or with AVX-512F: numpy's own vector kernel
//! (Cody-Waite range reduction, a ratio of a degree-5 and a degree-2 polynomial, a scaling by
//! `2^k`), not the C library's `expf` (numpy: at most 2.52 ulp). The two vector widths give the
//! same bits: AVX-512's `vscalefps` and the AVX2 helper's exponent addition (with one division
//! for denormal results) both round `poly · 2^k` once. On CPUs without AVX2/FMA3 (and on other
//! architectures) numpy calls the C library's `expf` instead ([`super::expf_glibc`] on x86-64
//! Linux).
//!
//! Each `fmadd` of the kernel is `mul_add`, correctly rounded on every platform. Special inputs:
//! a NaN gives numpy's `NPY_NANF` (`0x7fc00000`), `x >= 88.72283935546875` (`+inf` included)
//! gives `+inf`, `x <= -103.97208404541015625` (`-inf` included) gives `+0`. Not correctly
//! rounded; [`super::expf`] is, and is larmorx's default. This one serves `reproduce`
//! (scikit-image's random walker: `numpy.exp(-beta * gradients)` on `float32` data).

// numpy's literals, kept as written (more digits than an f32 holds; each rounds as C's `f` suffix
// rounds it).
#![allow(clippy::excessive_precision)]

/// `xmax` of `simd_exp_FLOAT`.
const XMAX: f32 = 88.722_839_355_468_75;
/// `xmin` of `simd_exp_FLOAT`.
const XMIN: f32 = -103.972_084_045_410_156_25;
/// `NPY_CODY_WAITE_LOGE_2_HIGHf`.
const C1: f32 = -6.931_457_52e-1;
/// `NPY_CODY_WAITE_LOGE_2_LOWf`.
const C2: f32 = -1.428_606_77e-6;
const P0: f32 = 9.999999999980870924916e-01;
const P1: f32 = 7.257664613233124478488e-01;
const P2: f32 = 2.473615434895520810817e-01;
const P3: f32 = 5.114512081637298353406e-02;
const P4: f32 = 6.757896990527504603057e-03;
const P5: f32 = 5.082762527590693718096e-04;
const Q0: f32 = 1.000000000000000000000e+00;
const Q1: f32 = -2.742335390411667452936e-01;
const Q2: f32 = 2.159509375685829852307e-02;
/// `NPY_RINT_CVT_MAGICf` (`0x1.8p+23f`).
const CVT_MAGIC: f32 = 12_582_912.0;
/// `NPY_LOG2Ef`.
const LOG2E: f32 = std::f32::consts::LOG2_E;
/// numpy's `NPY_NANF` (`__builtin_nanf("")`).
const NPY_NANF: f32 = f32::from_bits(0x7fc0_0000);

/// numpy's float32 `exp` (x86-64, AVX2/FMA3 or AVX-512F path), one lane of `simd_exp_FLOAT`.
#[inline(always)]
pub(super) fn expf_numpy(x_in: f32) -> f32 {
    // The kernel's masks: NaN, `x >= xmax`, `x <= xmin` lanes compute on 0 and are overwritten.
    if x_in.is_nan() {
        return NPY_NANF;
    }
    if x_in >= XMAX {
        return f32::INFINITY;
    }
    if x_in <= XMIN {
        return 0.0;
    }
    let x = x_in;
    // quadrant = rint(x * log2(e)) by the magic-number addition (the default rounding).
    let mut quadrant = x * LOG2E;
    quadrant = (quadrant + CVT_MAGIC) - CVT_MAGIC;
    // Cody-Waite: x* = x + q·c1 + q·c2 + q·0 (c3 is the zero register).
    let mut r = quadrant.mul_add(C1, x);
    r = quadrant.mul_add(C2, r);
    r = quadrant.mul_add(0.0, r);
    let mut num = P5.mul_add(r, P4);
    num = num.mul_add(r, P3);
    num = num.mul_add(r, P2);
    num = num.mul_add(r, P1);
    num = num.mul_add(r, P0);
    let mut den = Q2.mul_add(r, Q1);
    den = den.mul_add(r, Q0);
    let poly = num / den;
    // scalef: poly · 2^quadrant with one rounding (exact in double, then rounded to single).
    let k = quadrant as i32;
    let scale = f64::from_bits(((1023 + k) as u64) << 52);
    (f64::from(poly) * scale) as f32
}

#[cfg(test)]
mod tests {
    use super::expf_numpy;

    #[test]
    fn special_inputs() {
        assert_eq!(expf_numpy(f32::NAN).to_bits(), 0x7fc0_0000);
        assert_eq!(expf_numpy(-f32::NAN).to_bits(), 0x7fc0_0000);
        assert_eq!(expf_numpy(f32::INFINITY), f32::INFINITY);
        assert_eq!(expf_numpy(f32::NEG_INFINITY).to_bits(), 0);
        assert_eq!(expf_numpy(88.722_84), f32::INFINITY);
        assert_eq!(expf_numpy(-103.972_09).to_bits(), 0);
        assert_eq!(expf_numpy(0.0), 1.0);
        assert_eq!(expf_numpy(-0.0), 1.0);
        // numpy's worst case of its own note (2.52 ulp at x = 0xc2781e37) stays finite and small
        assert!(expf_numpy(f32::from_bits(0xc278_1e37)) > 0.0);
    }
}
