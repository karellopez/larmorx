// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! vnl's error function, as ITK's Gaussian interpolators use it.
//!
//! `vnl_erf(x) = sign(x) · P(½, x²)`, with the regularised incomplete gamma function `P`
//! computed by a series or a continued fraction to a relative error of 3e-7, and a Lanczos
//! approximation of log Γ. Ported from VXL's `core/vnl/vnl_gamma.cxx` and `vnl_erf.h` (BSD
//! licence) as bundled with ITK v5.4.5, in the same operation order, so interpolated values
//! match ITK's rather than an exact erf.

// Original copyright and licence (BSD-3-Clause) notice of VXL
// (Modules/ThirdParty/VNL/src/vxl/core/vxl_copyright.h in ITK v5.4.5):
//
//   Copyright 2000-2013 VXL Contributors
//   All rights reserved.
//
//   Redistribution and use in source and binary forms, with or without
//   modification, are permitted provided that the following conditions
//   are met:
//
//   * Redistributions of source code must retain the above copyright
//     notice, this list of conditions and the following disclaimer.
//
//   * Redistributions in binary form must reproduce the above copyright
//     notice, this list of conditions and the following disclaimer in the
//     documentation and/or other materials provided with the distribution.
//
//   * Neither the names of the copyright holders nor the names of their
//     contributors may be used to endorse or promote products derived
//     from this software without specific prior written permission.
//
//   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
//   "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
//   LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
//   FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
//   COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
//   INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
//   (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//   SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
//   HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
//   STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
//   OF THE POSSIBILITY OF SUCH DAMAGE.

use larmorx_core::math;

// The constants are written exactly as in vnl, so they parse to the same doubles.
#[allow(clippy::excessive_precision)]
fn log_gamma(x: f64) -> f64 {
    let mut zp = 2.50662827563479526904;
    zp += 225.525584619175212544 / x;
    zp -= 268.295973841304927459 / (x + 1.0);
    zp += 80.9030806934622512966 / (x + 2.0);
    zp -= 5.00757863970517583837 / (x + 3.0);
    zp += 0.0114684895434781459556 / (x + 4.0);
    let x1 = x + 4.65;
    math::log(zp) + (x - 0.5) * math::log(x1) - x1
}

const MAX_ITS: usize = 100;
const MAX_REL_ERROR: f64 = 3.0e-7;
const VERY_SMALL: f64 = 1.0e-30;

/// `vnl_gamma_series(a, x)`; `lg` is `vnl_log_gamma(a)`, which vnl evaluates on every call.
fn gamma_series(a: f64, x: f64, lg: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut a_i = a;
    let mut term = 1.0 / a;
    let mut sum = term;
    for _ in 1..=MAX_ITS {
        a_i += 1.0;
        term *= x / a_i;
        sum += term;
        if term.abs() < sum.abs() * MAX_REL_ERROR {
            break;
        }
    }
    sum * math::exp(-x + a * math::log(x) - lg)
}

/// `vnl_gamma_cont_frac(a, x)`; `lg` is `vnl_log_gamma(a)`.
fn gamma_cont_frac(a: f64, x: f64, lg: f64) -> f64 {
    let mut b_i = x + 1.0 - a;
    let mut c = 1.0 / VERY_SMALL;
    let mut d = 1.0 / b_i;
    let mut cf = d;
    for i in 1..=MAX_ITS {
        let i = i as f64;
        let a_i = i * (a - i);
        b_i += 2.0;
        d = a_i * d + b_i;
        if d.abs() < VERY_SMALL {
            d = VERY_SMALL;
        }
        c = b_i + a_i / c;
        if c.abs() < VERY_SMALL {
            c = VERY_SMALL;
        }
        d = 1.0 / d;
        let delta = d * c;
        cf *= delta;
        if (delta - 1.0).abs() < MAX_REL_ERROR {
            break;
        }
    }
    math::exp(-x + a * math::log(x) - lg) * cf
}

fn gamma_p(a: f64, x: f64, lg: f64) -> f64 {
    if x < a + 1.0 {
        gamma_series(a, x, lg)
    } else {
        1.0 - gamma_cont_frac(a, x, lg)
    }
}

/// `vnl_log_gamma(0.5)`, the term `vnl_erf` recomputes (two logarithms) on every call through
/// `vnl_gamma_p(0.5, x²)`. It is a pure function of its argument, so computing it once gives
/// the same bits and saves half of `erf`'s logarithms.
fn log_gamma_half() -> f64 {
    static VALUE: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *VALUE.get_or_init(|| log_gamma(0.5))
}

/// `vnl_erf`.
pub fn erf(x: f64) -> f64 {
    let lg = log_gamma_half();
    if x < 0.0 {
        -gamma_p(0.5, x * x, lg)
    } else {
        gamma_p(0.5, x * x, lg)
    }
}

/// `vnl_erfc` (`core/vnl/vnl_erf.cxx`, W. J. Cody's rational approximations): `1 − vnl_erf`
/// below 0.46875, Cody's fractions above, `2 − erfc(|x|)` for negative `x`.
pub fn erfc(x: f64) -> f64 {
    const THRESH: f64 = 0.46875;
    const XBIG: f64 = 26.543;
    const XHUGE: f64 = 6.71e7;
    const XMAX: f64 = 2.53e307;
    const C: [f64; 9] = [
        0.564_188_496_988_670_089,
        8.883_149_794_388_375_94,
        66.119_190_637_141_629_5,
        298.635_138_197_400_131,
        881.952_221_241_769_09,
        1_712.047_612_634_070_58,
        2_051.078_377_826_071_47,
        1_230.339_354_797_997_25,
        2.153_115_354_744_038_46e-8,
    ];
    const D: [f64; 8] = [
        15.744_926_110_709_834_7,
        117.693_950_891_312_499,
        537.181_101_862_009_858,
        1_621.389_574_566_690_19,
        3_290.799_235_733_459_63,
        4_362.619_090_143_247_16,
        3_439.367_674_143_721_64,
        1_230.339_354_803_749_42,
    ];
    const P: [f64; 6] = [
        0.305_326_634_961_232_344,
        0.360_344_899_949_804_439,
        0.125_781_726_111_229_246,
        0.016_083_785_148_742_276_6,
        6.587_491_615_298_378_03e-4,
        0.016_315_387_137_302_097_8,
    ];
    const Q: [f64; 5] = [
        2.568_520_192_289_822_42,
        1.872_952_849_923_460_47,
        0.527_905_102_951_428_412,
        0.060_518_341_312_441_319_1,
        0.002_335_204_976_268_691_85,
    ];
    const SQRPI: f64 = 0.564_189_583_547_756_286_95;
    let y = x.abs();
    let mut result;
    if y <= THRESH {
        return 1.0 - erf(x);
    } else if y <= 4.0 {
        let mut xnum = C[8] * y;
        let mut xden = y;
        for i in 0..7 {
            xnum = (xnum + C[i]) * y;
            xden = (xden + D[i]) * y;
        }
        result = (xnum + C[7]) / (xden + D[7]);
        let ysq = (y * 16.0).floor() / 16.0;
        let del = (y - ysq) * (y + ysq);
        result = math::exp(-ysq * ysq) * math::exp(-del) * result;
    } else if y >= XHUGE {
        result = if y < XMAX { SQRPI / y } else { 0.0 };
    } else if y >= XBIG {
        result = 0.0;
    } else {
        let ysq = 1.0 / (y * y);
        let mut xnum = P[5] * ysq;
        let mut xden = ysq;
        for i in 0..4 {
            xnum = (xnum + P[i]) * ysq;
            xden = (xden + Q[i]) * ysq;
        }
        result = ysq * (xnum + P[4]) / (xden + Q[4]);
        result = (SQRPI - result) / y;
        let ysq = (y * 16.0).floor() / 16.0;
        let del = (y - ysq) * (y + ysq);
        result = math::exp(-ysq * ysq) * math::exp(-del) * result;
    }
    if x < 0.0 {
        result = 2.0 - result;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_to_the_exact_erf() {
        for x in [-3.0, -1.2, -0.3, 0.0, 0.1, 0.7, 1.0, 2.5, 6.0] {
            let (ours, exact) = (erf(x), libm::erf(x));
            assert!(
                (ours - exact).abs() <= 1e-6 * exact.abs().max(1e-3),
                "{x}: {ours} vs {exact}"
            );
        }
        assert_eq!(erf(0.0), 0.0);
        assert_eq!(log_gamma_half().to_bits(), log_gamma(0.5).to_bits());
    }
}
