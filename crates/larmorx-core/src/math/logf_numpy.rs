// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
// Translated to Rust from numpy 2.5.4 (github.com/numpy/numpy, tag v2.5.4):
// numpy/_core/src/umath/loops_exponent_log.dispatch.c.src (`simd_log_FLOAT`, its helpers
// `fma_get_exponent`, `fma_get_mantissa`, `avx512_get_exponent`, `avx512_get_mantissa`) and the
// constants of numpy/_core/src/umath/npy_simd_data.h (`NPY_COEFF_*_LOGf`) and
// numpy/_core/include/numpy/npy_math.h (`NPY_LOGE2f`, `NPY_SQRT1_2f`). Copyright (c) 2005-2025,
// NumPy Developers, under the BSD-3-Clause licence (notice below). Rust translation Copyright
// 2026 Karel Lopez Vilaret; modified from the original (one lane of the vector code).
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
//! numpy's single-precision natural logarithm (`numpy.log` on a `float32` array) as numpy's
//! x86-64 builds compute it on CPUs with AVX2 and FMA3 or with AVX-512F: numpy's own vector
//! kernel, a ratio of two degree-5 polynomials on the mantissa (numpy: at most 3.83 ulp), not the
//! C library's `logf`. The two vector widths give the same bits on every input (the AVX-512
//! helpers `getexp`/`getmant` and the AVX2 helpers' denormal scaling agree on every finite
//! positive input, and the special inputs are overwritten). On CPUs without AVX2/FMA3 (and on
//! other architectures) numpy calls the C library's `logf` instead ([`super::logf_glibc`] on
//! x86-64 Linux).
//!
//! Each `fmadd` of the kernel is `mul_add`, correctly rounded on every platform. Special inputs:
//! a NaN gives numpy's `NPY_NANF` (`0x7fc00000`, whatever the input's sign and payload), a
//! negative number `-NPY_NANF` (`0xffc00000`), `±0` gives `-inf`, `+inf` gives `+inf`.
//! Not correctly rounded; [`super::logf`] is, and is larmorx's default. This one serves
//! `reproduce` (tedana's log-linear fit: `numpy.log(numpy.abs(data) + 1)` on `float32` data).
//! Proved equal to numpy 2.3.5 and 2.5.4 on this machine's AVX-512 and AVX2 paths on all 2^32
//! inputs (`validation/investigations/t2smap/numerics/`).

// numpy's literals, kept as written (more digits than an f32 holds; each rounds as C's `f` suffix
// rounds it).
#![allow(clippy::excessive_precision)]

const P0: f32 = 0.000000000000000000000e+00;
const P1: f32 = 9.999999999999998702752e-01;
const P2: f32 = 2.112677543073053063722e+00;
const P3: f32 = 1.480000633576506585156e+00;
const P4: f32 = 3.808837741388407920751e-01;
const P5: f32 = 2.589979117907922693523e-02;
const Q0: f32 = 1.000000000000000000000e+00;
const Q1: f32 = 2.612677543073109236779e+00;
const Q2: f32 = 2.453006071784736363091e+00;
const Q3: f32 = 9.864942958519418960339e-01;
const Q4: f32 = 1.546476374983906719538e-01;
const Q5: f32 = 5.875095403124574342950e-03;
/// `NPY_LOGE2f`.
const LOGE2F: f32 = std::f32::consts::LN_2;
/// `NPY_SQRT1_2f`.
const SQRT1_2F: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// numpy's `NPY_NANF` (`__builtin_nanf("")`).
const NPY_NANF: f32 = f32::from_bits(0x7fc0_0000);

/// numpy's float32 `log` (x86-64, AVX2/FMA3 or AVX-512F path), one lane of `simd_log_FLOAT`.
#[inline(always)]
pub(super) fn logf_numpy(x_in: f32) -> f32 {
    // The kernel's masks: NaN, negative, zero, +inf lanes are overwritten at the end.
    if x_in.is_nan() {
        return NPY_NANF;
    }
    if x_in < 0.0 {
        return -NPY_NANF;
    }
    if x_in == 0.0 {
        return f32::NEG_INFINITY;
    }
    if x_in == f32::INFINITY {
        return f32::INFINITY;
    }
    // fma_get_exponent / fma_get_mantissa: denormals scaled by 2^100 first.
    let denormal = x_in < f32::MIN_POSITIVE;
    let x = if denormal {
        x_in * f32::from_bits(0x7180_0000)
    } else {
        x_in
    };
    let bits = x.to_bits();
    let mut exponent = ((bits >> 23) as i32 - 0x7e) as f32;
    if denormal {
        exponent -= 100.0;
    }
    let mut m = f32::from_bits((bits & 0x007f_ffff) | (126 << 23));
    // if x < sqrt(2) {exp = exp-1; x = 2*x} (the comparison is with 1/sqrt(2) on [0.5, 1))
    if m <= SQRT1_2F {
        m += m;
        exponent -= 1.0;
    }
    let x = m - 1.0;
    let mut num = P5.mul_add(x, P4);
    num = num.mul_add(x, P3);
    num = num.mul_add(x, P2);
    num = num.mul_add(x, P1);
    num = num.mul_add(x, P0);
    let mut den = Q5.mul_add(x, Q4);
    den = den.mul_add(x, Q3);
    den = den.mul_add(x, Q2);
    den = den.mul_add(x, Q1);
    den = den.mul_add(x, Q0);
    let poly = num / den;
    exponent.mul_add(LOGE2F, poly)
}

#[cfg(test)]
mod tests {
    use super::logf_numpy;

    /// numpy's values (numpy 2.5.4, x86-64 AVX-512 and AVX2 paths, this machine) of a few inputs,
    /// among them ones where numpy's result differs from the correctly rounded one.
    #[test]
    fn recorded_values() {
        for (x, y) in [
            (0x3f80_0000u32, 0x0000_0000u32), // 1 -> 0
            (0x4000_0000, 0x3f31_7218),       // 2 -> ln 2
            (0x3f48_6945, 0xbe7a_ae7a),       // numpy's worst case (3.83 ulp)
            (0x3f9d_70a4, 0x3e53_fb87),       // 1.23
            (0x447a_0000, 0x40dd_0c55),       // 1000
            (0x0000_0001, 0xc2ce_8ed0),       // the smallest denormal
            (0x7f7f_ffff, 0x42b1_7218),       // the largest float
        ] {
            let got = logf_numpy(f32::from_bits(x)).to_bits();
            assert_eq!(
                got,
                y,
                "log({:e}) = {:e}",
                f32::from_bits(x),
                f32::from_bits(got)
            );
        }
        assert_eq!(logf_numpy(f32::NAN).to_bits(), 0x7fc0_0000);
        assert_eq!(
            logf_numpy(f32::from_bits(0xffc0_0001)).to_bits(),
            0x7fc0_0000
        );
        assert_eq!(logf_numpy(-1.0).to_bits(), 0xffc0_0000);
        assert_eq!(logf_numpy(-0.0), f32::NEG_INFINITY);
        assert_eq!(logf_numpy(0.0), f32::NEG_INFINITY);
        assert_eq!(logf_numpy(f32::INFINITY), f32::INFINITY);
        assert_eq!(logf_numpy(f32::NEG_INFINITY).to_bits(), 0xffc0_0000);
    }
}
