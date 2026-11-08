// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy's (and a few of SciPy's) array reductions and order statistics, with the operation
//! order and the intermediate types numpy 2.5.4 uses, so that a replica of a Python program gets
//! numpy's bits (tedana's `t2smap`: temporal means, the adaptive mask's percentile, the RMSE
//! table's `nanstd` and `nanpercentile`, the optimal combination's `average`, the T2* cap's
//! `scipy.stats.scoreatpercentile`).
//!
//! - [`reduce`]: `numpy.add.reduce` (pairwise summation along a contiguous axis, accumulation in
//!   order along an outer one), `mean`, `var`, `std`, `nansum`, `nanvar`, `nanstd` and the
//!   weighted `average`, over one axis of a C-ordered array.
//! - [`quantile`]: `numpy.percentile(..., method="higher")`, `numpy.nanpercentile` (the default
//!   `"linear"` method) along the first axis, SciPy's `scoreatpercentile(...,
//!   interpolation_method="lower")`; `numpy.percentile` with the default method of a whole array
//!   (a Python-number `q`, as numpy 2.3 or 2.5 computes it, [`NumpyRelease`], or a list of
//!   them), `numpy.median` of a whole array and along an axis (sdcflows' `brain_masker`,
//!   niworkflows' `IntensityClip` and `RobustAverage`).
//!
//! - [`umath`]: numpy's elementwise `exp` and `log` as its x86-64 Linux wheels compute them
//!   (for `reproduce`), and `clip` with scalar bounds.
//! - [`dot`]: `np.dot` and `np.linalg.norm` of float32 and float64 vectors (OpenBLAS's Haswell
//!   `sdot` and `ddot`), under SciPy's conjugate gradients.
//! - [`histograms`]: `numpy.linspace` and `numpy.histogram` with equal bins (scikit-image's Otsu
//!   threshold).
//!
//! The arithmetic is x86-64's (SSE) in every respect, NaNs included (`math::x86_nan`), so the
//! results are the same on every platform.
//!
//! numpy: Copyright (c) 2005-2025, NumPy Developers; SciPy: Copyright (c) 2001-2002 Enthought,
//! Inc. 2003, SciPy Developers. Both BSD-3-Clause: redistribution and use in source and binary
//! forms, with or without modification, are permitted provided that the following conditions
//! are met: redistributions of source code must retain the above copyright notice, this list of
//! conditions and the following disclaimer; redistributions in binary form must reproduce the
//! above copyright notice, this list of conditions and the following disclaimer in the
//! documentation and/or other materials provided with the distribution; neither the name of the
//! copyright holder nor the names of its contributors may be used to endorse or promote products
//! derived from this software without specific prior written permission. THIS SOFTWARE IS
//! PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED
//! WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
//! FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
//! CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
//! CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//! SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
//! THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR
//! OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
//! POSSIBILITY OF SUCH DAMAGE.

pub mod dot;
pub mod histograms;
pub mod quantile;
pub mod reduce;
pub mod umath;

use crate::math::x86_nan;

/// The numpy release whose arithmetic a call follows where numpy's releases differ.
///
/// numpy 2.5 changed `percentile` and `quantile` for a Python-number `q` (`weak_q`): numpy 2.2
/// and 2.3 divide `q` by the array's own type's 100 and compute the virtual index and the
/// interpolation weight in that type (`float32` for `float32` data); numpy 2.5 computes them in
/// `float64` and rounds the weight to the array's type only in the interpolation
/// ([`quantile::percentile_linear_weak`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NumpyRelease {
    /// numpy 2.2.6 (fMRIPrep 25.2.6's image) and 2.3.5 (the shared environment).
    V2_3,
    /// numpy 2.5.4 (the newest environment). The default.
    #[default]
    V2_5,
}

impl NumpyRelease {
    /// The release for a version string (`"2.3.5"`, `"2.5.4"`; 2.2 and 2.3 are `V2_3`, 2.4 and
    /// later `V2_5`). `None` for numpy 1.x or text that is not a version.
    pub fn from_version(version: &str) -> Option<Self> {
        let mut it = version.split('.');
        let major: u32 = it.next()?.parse().ok()?;
        let minor: u32 = it
            .next()?
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok()?;
        match (major, minor) {
            (2, 0..=3) => Some(NumpyRelease::V2_3),
            (2, _) | (3.., _) => Some(NumpyRelease::V2_5),
            _ => None,
        }
    }
}

/// The two numpy floating types the reductions run in, with x86-64's arithmetic.
pub trait NpFloat: Copy + PartialOrd + Send + Sync + 'static {
    /// `+0.0`, numpy's identity of `add`.
    const ZERO: Self;
    /// `-0.0`, the start of numpy's short pairwise sums.
    const NEG_ZERO: Self;
    /// numpy's `np.nan` in this type (Python's `float('nan')`: the sign bit clear).
    const NAN: Self;
    /// `a + b` as SSE computes it (a NaN result: `a`'s, else `b`'s, else the default NaN).
    fn add(self, b: Self) -> Self;
    /// `a - b` as SSE computes it.
    fn sub(self, b: Self) -> Self;
    /// `a * b` as SSE computes it.
    fn mul(self, b: Self) -> Self;
    /// `a / b` as SSE computes it.
    fn div(self, b: Self) -> Self;
    /// numpy's elementwise `exp` in this type ([`umath::exp_f32`], [`umath::exp_f64`]).
    fn np_exp(self) -> Self;
    /// `np.dot(x, y)` of two contiguous vectors of this type ([`dot::dot_f32`],
    /// [`dot::dot_f64`]).
    fn np_dot(x: &[Self], y: &[Self]) -> Self;
    /// The square root (IEEE: exact rounding; a NaN passes on).
    fn sqrt(self) -> Self;
    /// The value as `f64` (exact).
    fn to_f64(self) -> f64;
    /// An `f64` rounded to this type as SSE's `cvtsd2ss` does (NaNs keep their sign and the
    /// top of their payload, quieted).
    fn from_f64(v: f64) -> Self;
    /// Whether the value is a NaN.
    fn is_nan(self) -> bool;
}

impl NpFloat for f32 {
    const ZERO: Self = 0.0;
    const NEG_ZERO: Self = -0.0;
    const NAN: Self = f32::from_bits(0x7fc0_0000);
    #[inline(always)]
    fn add(self, b: Self) -> Self {
        x86_nan::add_f32(self, b)
    }
    #[inline(always)]
    fn sub(self, b: Self) -> Self {
        x86_nan::sub_f32(self, b)
    }
    #[inline(always)]
    fn mul(self, b: Self) -> Self {
        x86_nan::mul_f32(self, b)
    }
    #[inline(always)]
    fn div(self, b: Self) -> Self {
        x86_nan::div_f32(self, b)
    }
    #[inline(always)]
    fn np_exp(self) -> Self {
        umath::exp_f32(self)
    }
    #[inline]
    fn np_dot(x: &[Self], y: &[Self]) -> Self {
        dot::dot_f32(x, y)
    }
    #[inline(always)]
    fn sqrt(self) -> Self {
        let r = f32::sqrt(self);
        if r.is_nan() {
            if self.is_nan() {
                f32::from_bits(self.to_bits() | 0x0040_0000)
            } else {
                x86_nan::DEFAULT_NAN_F32
            }
        } else {
            r
        }
    }
    #[inline(always)]
    fn to_f64(self) -> f64 {
        if self.is_nan() {
            // cvtss2sd: sign and payload kept, quieted.
            let b = self.to_bits() as u64;
            f64::from_bits(
                ((b & 0x8000_0000) << 32) | 0x7ff8_0000_0000_0000 | ((b & 0x003f_ffff) << 29),
            )
        } else {
            self as f64
        }
    }
    #[inline(always)]
    fn from_f64(v: f64) -> Self {
        if v.is_nan() {
            let b = v.to_bits();
            f32::from_bits(
                (((b >> 32) as u32) & 0x8000_0000) | 0x7fc0_0000 | ((b >> 29) as u32 & 0x003f_ffff),
            )
        } else {
            v as f32
        }
    }
    #[inline(always)]
    fn is_nan(self) -> bool {
        f32::is_nan(self)
    }
}

impl NpFloat for f64 {
    const ZERO: Self = 0.0;
    const NEG_ZERO: Self = -0.0;
    const NAN: Self = f64::from_bits(0x7ff8_0000_0000_0000);
    #[inline(always)]
    fn add(self, b: Self) -> Self {
        x86_nan::add_f64(self, b)
    }
    #[inline(always)]
    fn sub(self, b: Self) -> Self {
        x86_nan::sub_f64(self, b)
    }
    #[inline(always)]
    fn mul(self, b: Self) -> Self {
        x86_nan::mul_f64(self, b)
    }
    #[inline(always)]
    fn div(self, b: Self) -> Self {
        x86_nan::div_f64(self, b)
    }
    #[inline(always)]
    fn np_exp(self) -> Self {
        umath::exp_f64(self)
    }
    #[inline]
    fn np_dot(x: &[Self], y: &[Self]) -> Self {
        dot::dot_f64(x, y)
    }
    #[inline(always)]
    fn sqrt(self) -> Self {
        let r = f64::sqrt(self);
        if r.is_nan() {
            if self.is_nan() {
                f64::from_bits(self.to_bits() | 0x0008_0000_0000_0000)
            } else {
                x86_nan::DEFAULT_NAN_F64
            }
        } else {
            r
        }
    }
    #[inline(always)]
    fn to_f64(self) -> f64 {
        self
    }
    #[inline(always)]
    fn from_f64(v: f64) -> Self {
        v
    }
    #[inline(always)]
    fn is_nan(self) -> bool {
        f64::is_nan(self)
    }
}

/// `a / b` in double precision as SSE computes it (numpy's `true_divide` of a float array by a
/// count, which it runs in `float64` and casts back).
#[inline(always)]
pub(crate) fn div_f64(a: f64, b: f64) -> f64 {
    x86_nan::div_f64(a, b)
}

#[cfg(test)]
mod tests {
    use super::NpFloat;

    #[test]
    fn nan_conversions_are_sse_s() {
        let a = f32::from_bits(0xffa0_0001); // signalling, negative
        assert_eq!(a.to_f64().to_bits(), 0xfffc_0000_2000_0000);
        assert_eq!(f32::from_f64(a.to_f64()).to_bits(), 0xffe0_0001);
        assert_eq!(f32::from_f64(f64::NAN).to_bits(), 0x7fc0_0000);
        assert_eq!(NpFloat::sqrt(-1.0f32).to_bits(), 0xffc0_0000);
        assert_eq!(NpFloat::sqrt(-0.0f64).to_bits(), (-0.0f64).to_bits());
        assert_eq!(f32::from_f64(1.0 + f64::EPSILON), 1.0);
    }
}
