// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! B-spline prefilter poles, interpolation weights and the recursive prefilter, ported from
//! SciPy 1.15.2 `scipy/ndimage/src/ni_splines.c` (BSD-3-Clause; notice below and in NOTICE).
//!
//! Every expression keeps SciPy's operation order: the weights and the filter recursions are
//! bit-identical to SciPy's C code compiled without floating-point contraction (x86-64).
//!
//! Copyright (c) 2001-2002 Enthought, Inc. 2003-2024, SciPy Developers.
//! All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without modification, are
//! permitted provided that the following conditions are met:
//!
//! 1. Redistributions of source code must retain the above copyright notice, this list of
//!    conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright notice, this list of
//!    conditions and the following disclaimer in the documentation and/or other materials
//!    provided with the distribution.
//! 3. Neither the name of the copyright holder nor the names of its contributors may be used
//!    to endorse or promote products derived from this software without specific prior
//!    written permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS
//! OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
//! MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
//! COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
//! EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
//! SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
//! HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR
//! TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
//! SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use larmorx_core::math;

use super::Extend;

/// The poles of the B-spline prefilter of `order` (2 to 5), as `get_filter_poles`. SciPy's
/// literals are kept as written; they round to the same doubles.
#[allow(clippy::excessive_precision)]
pub(crate) fn filter_poles(order: u32) -> &'static [f64] {
    match order {
        // sqrt(8.0) - 3.0
        2 => &[-0.171_572_875_253_809_902_396_622_551_580_603_843],
        // sqrt(3.0) - 2.0
        3 => &[-0.267_949_192_431_122_706_472_553_658_494_127_633],
        4 => &[
            // sqrt(664.0 - sqrt(438976.0)) + sqrt(304.0) - 19.0
            -0.361_341_225_900_220_177_092_212_841_325_675_255,
            // sqrt(664.0 + sqrt(438976.0)) - sqrt(304.0) - 19.0
            -0.013_725_429_297_339_121_360_331_226_939_128_204,
        ],
        5 => &[
            // sqrt(67.5 - sqrt(4436.25)) + sqrt(26.25) - 6.5
            -0.430_575_347_099_973_791_851_434_783_493_520_110,
            // sqrt(67.5 + sqrt(4436.25)) - sqrt(26.25) - 6.5
            -0.043_096_288_203_264_653_822_712_376_822_550_182,
        ],
        _ => &[],
    }
}

/// The interpolation weights at `x` for `order` 1 to 5 (`get_spline_interpolation_weights`):
/// `weights[0..=order]`. Order 0 has no weights (SciPy returns early and never reads them).
#[inline(always)]
pub(crate) fn interpolation_weights(x: f64, order: u32, weights: &mut [f64; 6]) {
    // Convert x to the delta to the middle knot.
    let knot = if (order & 1) == 1 { x } else { x + 0.5 };
    let x = x - super::floor(knot);
    let mut y = x;
    let mut z = 1.0 - x;
    match order {
        1 => {
            weights[0] = 1.0 - x;
        }
        2 => {
            weights[1] = 0.75 - x * x;
            y = 0.5 - x;
            weights[0] = 0.5 * y * y;
        }
        3 => {
            weights[1] = (y * y * (y - 2.0) * 3.0 + 4.0) / 6.0;
            weights[2] = (z * z * (z - 2.0) * 3.0 + 4.0) / 6.0;
            weights[0] = z * z * z / 6.0;
        }
        4 => {
            let t = x * x;
            weights[2] = t * (t * 0.25 - 0.625) + 115.0 / 192.0;
            y = 1.0 + x;
            weights[1] = y * (y * (y * (5.0 - y) / 6.0 - 1.25) + 5.0 / 24.0) + 55.0 / 96.0;
            weights[3] = z * (z * (z * (5.0 - z) / 6.0 - 1.25) + 5.0 / 24.0) + 55.0 / 96.0;
            y = 0.5 - x;
            let t = y * y;
            weights[0] = t * t / 24.0;
        }
        5 => {
            let t = y * y;
            weights[2] = t * (t * (0.25 - y / 12.0) - 0.5) + 0.55;
            let t = z * z;
            weights[3] = t * (t * (0.25 - z / 12.0) - 0.5) + 0.55;
            y += 1.0;
            weights[1] = y * (y * (y * (y * (y / 24.0 - 0.375) + 1.25) - 1.75) + 0.625) + 0.425;
            z += 1.0;
            weights[4] = z * (z * (z * (z * (z / 24.0 - 0.375) + 1.25) - 1.75) + 0.625) + 0.425;
            y = 1.0 - x;
            let t = y * y;
            weights[0] = y * t * t / 120.0;
        }
        _ => return, // order 0: unsupported in SciPy's table; the caller never multiplies
    }
    // All interpolation weights add to 1.0, so use it for the last one.
    let o = order as usize;
    weights[o] = 1.0;
    for i in 0..o {
        weights[o] -= weights[i];
    }
}

/// The boundary initialisation of the recursive filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Init {
    Mirror,
    Wrap,
    Reflect,
}

fn init_for(mode: Extend) -> Init {
    // `apply_filter`'s switch: the modes without an analytic prefilter use mirror.
    match mode {
        Extend::GridConstant | Extend::Constant | Extend::Mirror | Extend::Wrap => Init::Mirror,
        Extend::GridWrap => Init::Wrap,
        Extend::Nearest | Extend::Reflect => Init::Reflect,
    }
}

/// The prefilter for lines of one length: poles, gain and the powers `pow(z, n - 1)` /
/// `pow(z, n)` SciPy computes for every line (here once per length).
#[derive(Clone, Debug)]
pub(crate) struct LineFilter {
    poles: &'static [f64],
    gain: f64,
    init: Init,
    len: usize,
    /// Per pole: `z^(n-1)` (mirror) or `z^n` (reflect); unused for wrap.
    powers: [f64; 2],
}

impl LineFilter {
    pub(crate) fn new(order: u32, mode: Extend, len: usize) -> Self {
        let poles = filter_poles(order);
        let mut gain = 1.0;
        for &z in poles {
            gain *= (1.0 - z) * (1.0 - 1.0 / z);
        }
        let init = init_for(mode);
        let mut powers = [0.0; 2];
        for (p, &z) in powers.iter_mut().zip(poles) {
            // SciPy calls the C library's pow; `math::powi` is its correctly rounded value.
            *p = match init {
                Init::Mirror => math::powi(z, len.saturating_sub(1) as u32),
                Init::Reflect => math::powi(z, len as u32),
                Init::Wrap => 0.0,
            };
        }
        LineFilter {
            poles,
            gain,
            init,
            len,
            powers,
        }
    }

    /// Filters one line in place (`apply_filter`); lines of length 1 are left alone, as
    /// `NI_SplineFilter1D` does.
    #[inline]
    pub(crate) fn apply(&self, c: &mut [f64]) {
        let n = self.len;
        debug_assert_eq!(c.len(), n);
        if n <= 1 || self.poles.is_empty() {
            return;
        }
        for v in c.iter_mut() {
            *v *= self.gain;
        }
        for (k, &z) in self.poles.iter().enumerate() {
            match self.init {
                Init::Mirror => init_causal_mirror(c, z, self.powers[k]),
                Init::Wrap => init_causal_wrap(c, z),
                Init::Reflect => init_causal_reflect(c, z, self.powers[k]),
            }
            for i in 1..n {
                c[i] += z * c[i - 1];
            }
            match self.init {
                Init::Mirror => init_anticausal_mirror(c, z),
                Init::Wrap => init_anticausal_wrap(c, z),
                Init::Reflect => init_anticausal_reflect(c, z),
            }
            for i in (0..n - 1).rev() {
                c[i] = z * (c[i + 1] - c[i]);
            }
        }
    }
}

impl LineFilter {
    /// Filters many lines at once: `data` holds `len` rows of `stride` values, line `i` being
    /// column `i` (`data[t·stride + i]`), and the columns in `cols` are filtered. Every line
    /// gets exactly the operations of [`LineFilter::apply`], in the same order; working on
    /// whole rows makes the loops contiguous (and vectorisable) for strided axes.
    pub(crate) fn apply_rows(
        &self,
        data: &mut [f64],
        stride: usize,
        cols: std::ops::Range<usize>,
        acc: &mut Vec<f64>,
    ) {
        let n = self.len;
        if n <= 1 || self.poles.is_empty() || cols.is_empty() {
            return;
        }
        let (c0, w) = (cols.start, cols.len());
        debug_assert!(data.len() >= (n - 1) * stride + cols.end);
        let row = |t: usize| t * stride + c0..t * stride + c0 + w;
        for t in 0..n {
            for v in &mut data[row(t)] {
                *v *= self.gain;
            }
        }
        acc.clear();
        acc.resize(w, 0.0);
        for (k, &z) in self.poles.iter().enumerate() {
            let zp = self.powers[k];
            // Causal initialisation of row 0.
            match self.init {
                Init::Mirror => {
                    // c[0] = c[0] + z_n_1·c[n-1]; c[0] += z_i·(c[i] + z_n_1·c[n-1-i]), i < n-1
                    for (a, (&x0, &xl)) in acc
                        .iter_mut()
                        .zip(data[row(0)].iter().zip(&data[row(n - 1)]))
                    {
                        *a = x0 + zp * xl;
                    }
                    let mut z_i = z;
                    for t in 1..n - 1 {
                        let (lo, hi) = (row(t), row(n - 1 - t));
                        for ((a, &x), &y) in acc.iter_mut().zip(&data[lo]).zip(&data[hi]) {
                            *a += z_i * (x + zp * y);
                        }
                        z_i *= z;
                    }
                    let den = 1.0 - zp * zp;
                    for (d, &a) in data[row(0)].iter_mut().zip(acc.iter()) {
                        *d = a / den;
                    }
                }
                Init::Wrap => {
                    acc.copy_from_slice(&data[row(0)]);
                    let mut z_i = z;
                    for t in 1..n {
                        for (a, &x) in acc.iter_mut().zip(&data[row(n - t)]) {
                            *a += z_i * x;
                        }
                        z_i *= z;
                    }
                    let den = 1.0 - z_i;
                    for (d, &a) in data[row(0)].iter_mut().zip(acc.iter()) {
                        *d = a / den;
                    }
                }
                Init::Reflect => {
                    // c[0] is updated in place, and the last term reads the updated c[0].
                    let mut orig = std::mem::take(acc);
                    orig.copy_from_slice(&data[row(0)]);
                    {
                        let (first, rest) = data.split_at_mut(stride);
                        let last = &rest[(n - 2) * stride + c0..][..w];
                        for (d, &xl) in first[c0..c0 + w].iter_mut().zip(last) {
                            *d += zp * xl;
                        }
                    }
                    let mut z_i = z;
                    for t in 1..n {
                        let src = n - 1 - t;
                        let (first, rest) = data.split_at_mut(stride);
                        let first = &mut first[c0..c0 + w];
                        let x = &rest[(t - 1) * stride + c0..][..w];
                        if src == 0 {
                            for (d, &x) in first.iter_mut().zip(x) {
                                *d += z_i * (x + zp * *d);
                            }
                        } else {
                            let y = &rest[(src - 1) * stride + c0..][..w];
                            for ((d, &x), &y) in first.iter_mut().zip(x).zip(y) {
                                *d += z_i * (x + zp * y);
                            }
                        }
                        z_i *= z;
                    }
                    let f = z / (1.0 - zp * zp);
                    for (d, &o) in data[row(0)].iter_mut().zip(orig.iter()) {
                        *d *= f;
                        *d += o;
                    }
                    *acc = orig;
                }
            }
            // Forward recursion.
            for t in 1..n {
                let (prev, cur) = data.split_at_mut(t * stride);
                let prev = &prev[(t - 1) * stride + c0..][..w];
                for (c, &p) in cur[c0..c0 + w].iter_mut().zip(prev) {
                    *c += z * p;
                }
            }
            // Anticausal initialisation of row n-1.
            {
                let (head, last) = data.split_at_mut((n - 1) * stride);
                let last = &mut last[c0..c0 + w];
                match self.init {
                    Init::Mirror => {
                        let prev = &head[(n - 2) * stride + c0..][..w];
                        let den = z * z - 1.0;
                        for (l, &p) in last.iter_mut().zip(prev) {
                            *l = (z * p + *l) * z / den;
                        }
                    }
                    Init::Wrap => {
                        let mut z_i = z;
                        for t in 0..n - 1 {
                            let x = &head[t * stride + c0..][..w];
                            for (l, &x) in last.iter_mut().zip(x) {
                                *l += z_i * x;
                            }
                            z_i *= z;
                        }
                        let f = z / (z_i - 1.0);
                        for l in last.iter_mut() {
                            *l *= f;
                        }
                    }
                    Init::Reflect => {
                        let f = z / (z - 1.0);
                        for l in last.iter_mut() {
                            *l *= f;
                        }
                    }
                }
            }
            // Backward recursion.
            for t in (0..n - 1).rev() {
                let (cur, next) = data.split_at_mut((t + 1) * stride);
                let next = &next[c0..c0 + w];
                for (c, &x) in cur[t * stride + c0..][..w].iter_mut().zip(next) {
                    *c = z * (x - *c);
                }
            }
        }
    }
}

fn init_causal_mirror(c: &mut [f64], z: f64, z_n_1: f64) {
    let n = c.len();
    let mut z_i = z;
    c[0] += z_n_1 * c[n - 1];
    for i in 1..n - 1 {
        c[0] += z_i * (c[i] + z_n_1 * c[n - 1 - i]);
        z_i *= z;
    }
    c[0] /= 1.0 - z_n_1 * z_n_1;
}

fn init_anticausal_mirror(c: &mut [f64], z: f64) {
    let n = c.len();
    c[n - 1] = (z * c[n - 2] + c[n - 1]) * z / (z * z - 1.0);
}

fn init_causal_wrap(c: &mut [f64], z: f64) {
    let n = c.len();
    let mut z_i = z;
    for i in 1..n {
        c[0] += z_i * c[n - i];
        z_i *= z;
    }
    c[0] /= 1.0 - z_i; // z_i = pow(z, n)
}

fn init_anticausal_wrap(c: &mut [f64], z: f64) {
    let n = c.len();
    let mut z_i = z;
    for i in 0..n - 1 {
        c[n - 1] += z_i * c[i];
        z_i *= z;
    }
    c[n - 1] *= z / (z_i - 1.0); // z_i = pow(z, n)
}

fn init_causal_reflect(c: &mut [f64], z: f64, z_n: f64) {
    let n = c.len();
    let mut z_i = z;
    let c0 = c[0];
    c[0] += z_n * c[n - 1];
    for i in 1..n {
        c[0] += z_i * (c[i] + z_n * c[n - 1 - i]);
        z_i *= z;
    }
    c[0] *= z / (1.0 - z_n * z_n);
    c[0] += c0;
}

fn init_anticausal_reflect(c: &mut [f64], z: f64) {
    let n = c.len();
    c[n - 1] *= z / (z - 1.0);
}
