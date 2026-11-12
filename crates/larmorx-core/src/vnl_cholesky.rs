// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! `vnl_cholesky`: LINPACK's Cholesky factorization of a symmetric positive-definite matrix, as
//! FreeSurfer 8.2.0 calls it through ITK 4.13.2's VNL (numerics.cpp's
//! `sc_linalg_cholesky_decomp`: the default `verbose` mode, `rank_deficiency()`,
//! `upper_triangle()`), with the class's other members: `solve`, `determinant`, `inverse`,
//! `lower_triangle` and the `estimate_condition` mode's `rcond`.
//!
//! VNL hands its row-major data to LINPACK, which reads it column-major: `dpofa` factors the
//! upper triangle of `Mᵀ`, that is `M`'s lower triangle as stored, and leaves the rest of the
//! array as it was. The f2c translations (`dpofa`, `dposl`, `dpodi`, `dpoco`) are followed
//! statement by statement with LINPACK's 1-based indices and the reference BLAS (`ddot`,
//! `daxpy`, `dscal` from [`crate::vnl_svd`], `dasum` here, unrolled by six); the f2c macros
//! `abs` and `max` keep their C semantics. The messages VNL writes to standard error go to
//! [`Streams::err`], with the matrix printed by `vnl_matrix`'s `operator<<` (`%g`, a blank after
//! each element, a newline after each row).
//!
//! Two defects, both of `vnl_cholesky` in ITK 4.13.2 and 5.4.5:
//! - **ITK-108**: `rcond_` is never initialised, and `dpoco` leaves it unset when the matrix is
//!   not positive definite: `rcond()` (and the `estimate_condition` mode's message) then gives
//!   memory contents. The port gives 0 there, with a notice, in both modes (memory is never
//!   reproduced, D13).
//! - **ITK-109**: `determinant()`'s `dpodi` keeps its running mantissa in [1, 10) with loops that
//!   never end when it is not finite (an infinite or NaN diagonal entry of the factor, or one
//!   whose square overflows, above about 1.3e154). The port stops there: the mantissa itself for
//!   a non-finite factor; for a finite factor, the square taken in two steps. Both modes (a hang is
//!   never reproduced, D13), with a notice.
//!
//! Ported from VXL as bundled with ITK v4.13.2 (BSD licence; LINPACK and the reference BLAS from
//! netlib, translated by f2c): `core/vnl/algo/vnl_cholesky.{h,cxx}`,
//! `v3p/netlib/linpack/{dpofa,dposl,dpodi,dpoco}.c`, `v3p/netlib/blas/dasum.c`.

// Original copyright and licence (BSD-3-Clause) notice of VXL
// (Modules/ThirdParty/VNL/src/vxl/core/vxl_copyright.h in ITK v4.13.2):
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
//
// LINPACK and the reference BLAS are netlib software, distributed by VXL as above.

#![allow(clippy::needless_range_loop)]
// The expressions are kept as the C writes them (`-1.0 * x` is not `-x` for a NaN; the
// constants are VXL's spellings; the negated comparisons are the f2c loops' NaN behaviour).
#![allow(
    clippy::neg_multiply,
    clippy::manual_range_contains,
    clippy::excessive_precision,
    clippy::approx_constant,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::assign_op_pattern,
    clippy::collapsible_if,
    clippy::needless_late_init,
    clippy::too_many_arguments
)]

use crate::cfmt::{self, Streams};
use crate::defects::{ITK_108, ITK_109, Notice, UpstreamDefects};
use crate::math;
use crate::vnl_svd::{d_sign, daxpy, ddot, dscal};

/// f2c's `abs(x)`: `x >= 0 ? x : -x`.
#[inline]
fn abs_m(x: f64) -> f64 {
    if x >= 0.0 { x } else { -x }
}

/// f2c's `max(a, b)`: `a >= b ? a : b`.
#[inline]
fn max_m(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

/// `dasum` (unit stride): the sum of `abs`, unrolled by six.
fn dasum(x: &[f64]) -> f64 {
    let n = x.len();
    let m = n % 6;
    let mut t = 0.0f64;
    for i in 0..m {
        t += abs_m(x[i]);
    }
    let mut i = m;
    while i + 6 <= n {
        t = t
            + abs_m(x[i])
            + abs_m(x[i + 1])
            + abs_m(x[i + 2])
            + abs_m(x[i + 3])
            + abs_m(x[i + 4])
            + abs_m(x[i + 5]);
        i += 6;
    }
    t
}

/// `vnl_cholesky::Operation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Quiet,
    /// The default: a message when the matrix is not positive definite.
    Verbose,
    /// `dpoco`: the factorization and the reciprocal condition number `rcond`.
    EstimateCondition,
}

impl Mode {
    /// The enumerator's value (`quiet` 0, `verbose` 1, `estimate_condition` 2).
    pub fn from_value(v: i32) -> Mode {
        match v {
            0 => Mode::Quiet,
            2 => Mode::EstimateCondition,
            _ => Mode::Verbose,
        }
    }
}

/// `vnl_matrix<double>`'s `operator<<`: each element as `%g` and a blank, a newline per row.
pub fn matrix_text(m: &[f64], rows: usize, cols: usize) -> String {
    let mut s = String::new();
    for i in 0..rows {
        for j in 0..cols {
            s.push_str(&cfmt::ostream(m[i * cols + j]));
            s.push(' ');
        }
        s.push('\n');
    }
    s
}

/// A `vnl_cholesky` of an `n × n` matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct VnlCholesky {
    /// `A_`: the matrix as VNL stores it (row-major), after LINPACK: `a[(j-1)·n + (i-1)]` is
    /// Fortran's `a(i, j)`.
    a: Vec<f64>,
    n: usize,
    /// `num_dims_rank_def_`: LINPACK's `info` (0, or the order of the first leading minor that
    /// is not positive definite).
    rank_def: i64,
    /// `rcond_`, set only by a successful `estimate_condition` (ITK-108).
    rcond: Option<f64>,
    pub notices: Vec<Notice>,
}

impl VnlCholesky {
    /// `vnl_cholesky(M, mode)` of the `n × n` matrix `m` (row-major). Writes VNL's messages to
    /// `out.err`: the non-symmetry warning when `|M(0, n−1) − M(n−1, 0)| > 1e-8`, then, in
    /// `Verbose`, the rank deficiency, or in `EstimateCondition` the condition number and the
    /// rank deficiency when it is not 0.
    pub fn new(m: &[f64], n: usize, mode: Mode, out: &mut dyn Streams) -> VnlCholesky {
        assert!(n >= 1 && m.len() == n * n, "not an n × n matrix");
        let mut ch = VnlCholesky {
            a: m.to_vec(),
            n,
            rank_def: -1,
            rcond: None,
            notices: Vec::new(),
        };
        if (m[n - 1] - m[(n - 1) * n]).abs() > 1e-8 {
            out.err(&format!(
                "vnl_cholesky: WARNING: non-symmetric: {}\n",
                matrix_text(m, n, n)
            ));
        }
        if mode != Mode::EstimateCondition {
            ch.rank_def = dpofa(&mut ch.a, n);
            if mode == Mode::Verbose && ch.rank_def != 0 {
                out.err(&format!(
                    "vnl_cholesky: {} dimensions of non-posdeffness\n",
                    ch.rank_def
                ));
            }
        } else {
            let (rcond, info) = dpoco(&mut ch.a, n);
            ch.rank_def = info;
            ch.rcond = rcond;
            if info != 0 {
                ch.notices.push(Notice::new(
                    &ITK_108,
                    UpstreamDefects::Fix,
                    "rcond is not computed for a matrix that is not positive definite (VNL prints memory contents); printed as 0",
                ));
                out.err(&format!(
                    "vnl_cholesky: rcond={} so {} dimensions of non-posdeffness\n",
                    cfmt::ostream(0.0),
                    info
                ));
            }
        }
        ch
    }

    /// `rank_deficiency()`.
    pub fn rank_deficiency(&self) -> i64 {
        self.rank_def
    }

    /// `rcond()`: the reciprocal condition number of `EstimateCondition`, or `None` where VNL
    /// returns memory contents (ITK-108).
    pub fn rcond(&self) -> Option<f64> {
        self.rcond
    }

    #[inline]
    fn fat(&self, i: usize, j: usize) -> f64 {
        self.a[(j - 1) * self.n + (i - 1)]
    }

    /// `L_badly_named_method()`: `A_` itself (row-major).
    pub fn l_badly_named_method(&self) -> &[f64] {
        &self.a
    }

    /// `upper_triangle()`: `U` with `M = UᵀU` (row-major).
    pub fn upper_triangle(&self) -> Vec<f64> {
        let n = self.n;
        let mut u = vec![0.0f64; n * n];
        for i in 0..n {
            u[i * n + i] = self.a[i * n + i];
            for j in i + 1..n {
                u[i * n + j] = self.a[j * n + i];
                u[j * n + i] = 0.0;
            }
        }
        u
    }

    /// `lower_triangle()`: `L = Uᵀ` (row-major).
    pub fn lower_triangle(&self) -> Vec<f64> {
        let n = self.n;
        let mut l = vec![0.0f64; n * n];
        for i in 0..n {
            l[i * n + i] = self.a[i * n + i];
            for j in i + 1..n {
                l[j * n + i] = self.a[j * n + i];
                l[i * n + j] = 0.0;
            }
        }
        l
    }

    /// `solve(b)`: `dposl` on the factor.
    pub fn solve(&self, b: &[f64]) -> Vec<f64> {
        let n = self.n;
        assert_eq!(b.len(), n, "a right-hand side of n values");
        let mut x = b.to_vec();
        for k in 1..=n {
            let c = (k - 1) * n;
            let t = ddot(&self.a[c..c + k - 1], &x[..k - 1]);
            x[k - 1] = (x[k - 1] - t) / self.fat(k, k);
        }
        for kb in 1..=n {
            let k = n + 1 - kb;
            x[k - 1] /= self.fat(k, k);
            let t = -x[k - 1];
            let c = (k - 1) * n;
            daxpy(t, &self.a[c..c + k - 1], &mut x[..k - 1]);
        }
        x
    }

    /// `determinant()`: `dpodi` with `job = 10`, `det[0]·10^det[1]` (the mantissa kept in
    /// [1, 10), `pow` glibc's); see ITK-109 for the factors whose loops never end in VNL.
    pub fn determinant(&mut self) -> f64 {
        let n = self.n;
        let (mut d1, mut d2) = (1.0f64, 0.0f64);
        let s = 10.0f64;
        let mut stalled = false;
        for i in 1..=n {
            let d = self.fat(i, i);
            let mut next = d * d * d1;
            if !next.is_finite() && d.is_finite() && d1.is_finite() {
                // VNL's loops never end on the overflowed square: take it in two steps.
                stalled = true;
                next = d * d1;
                while next.abs() >= s {
                    next /= s;
                    d2 += 1.0;
                }
                next *= d;
            }
            d1 = next;
            if d1 == 0.0 {
                break;
            }
            if !d1.is_finite() {
                stalled = true;
                break;
            }
            while !(d1 >= 1.0) {
                d1 *= s;
                d2 += -1.0;
            }
            while !(d1 < s) {
                d1 /= s;
                d2 += 1.0;
            }
        }
        if stalled {
            crate::defects::push_once(
                &mut self.notices,
                Notice::new(
                    &ITK_109,
                    UpstreamDefects::Fix,
                    "the determinant of a factor with a non-finite or overflowing diagonal entry (VNL never returns)",
                ),
            );
        }
        d1 * math::pow_glibc(10.0, d2)
    }

    /// `inverse()`: `None` (with VNL's message) for a rank-deficient matrix, else `dpodi` with
    /// `job = 01` on the factor and its upper triangle mirrored (row-major).
    pub fn inverse(&self, out: &mut dyn Streams) -> Option<Vec<f64>> {
        if self.rank_def != 0 {
            out.err("vnl_cholesky: Calling inverse() on rank-deficient matrix\n");
            return None;
        }
        let n = self.n;
        let mut a = self.a.clone();
        let idx = |i: usize, j: usize| (j - 1) * n + (i - 1);
        for k in 1..=n {
            a[idx(k, k)] = 1.0 / a[idx(k, k)];
            let t = -a[idx(k, k)];
            let c = (k - 1) * n;
            dscal(t, &mut a[c..c + k - 1]);
            for j in k + 1..=n {
                let t = a[idx(k, j)];
                a[idx(k, j)] = 0.0;
                let (kc, jc) = ((k - 1) * n, (j - 1) * n);
                let (lo, hi) = a.split_at_mut(jc);
                daxpy(t, &lo[kc..kc + k], &mut hi[..k]);
            }
        }
        for j in 1..=n {
            for k in 1..j {
                let t = a[idx(k, j)];
                let (kc, jc) = ((k - 1) * n, (j - 1) * n);
                let (lo, hi) = a.split_at_mut(jc);
                daxpy(t, &hi[..k], &mut lo[kc..kc + k]);
            }
            let t = a[idx(j, j)];
            let jc = (j - 1) * n;
            dscal(t, &mut a[jc..jc + j]);
        }
        for i in 0..n {
            for j in i + 1..n {
                a[i * n + j] = a[j * n + i];
            }
        }
        Some(a)
    }
}

/// `dpofa` on the column-major `n × n` array `a`: the factor in its upper triangle; `info`.
fn dpofa(a: &mut [f64], n: usize) -> i64 {
    let idx = |i: usize, j: usize| (j - 1) * n + (i - 1);
    for j in 1..=n {
        let mut s = 0.0f64;
        for k in 1..j {
            let (kc, jc) = ((k - 1) * n, (j - 1) * n);
            let mut t = a[idx(k, j)] - ddot(&a[kc..kc + k - 1], &a[jc..jc + k - 1]);
            t /= a[idx(k, k)];
            a[idx(k, j)] = t;
            s += t * t;
        }
        s = a[idx(j, j)] - s;
        if s <= 0.0 {
            return j as i64;
        }
        a[idx(j, j)] = s.sqrt();
    }
    0
}

/// `dpoco`: `dpofa` with LINPACK's estimate of the reciprocal condition number (`None` when the
/// factorization fails: `rcond` is not set). Returns `(rcond, info)`.
fn dpoco(a: &mut [f64], n: usize) -> (Option<f64>, i64) {
    let idx = |i: usize, j: usize| (j - 1) * n + (i - 1);
    let mut z = vec![0.0f64; n + 1]; // 1-based
    for j in 1..=n {
        let c = (j - 1) * n;
        z[j] = dasum(&a[c..c + j]);
        for i in 1..j {
            z[i] += abs_m(a[idx(i, j)]);
        }
    }
    let mut anorm = 0.0f64;
    for j in 1..=n {
        anorm = max_m(anorm, z[j]);
    }
    let info = dpofa(a, n);
    if info != 0 {
        return (None, info);
    }
    let mut ek = 1.0f64;
    for zj in z.iter_mut().skip(1) {
        *zj = 0.0;
    }
    for k in 1..=n {
        if z[k] != 0.0 {
            ek = d_sign(ek, -z[k]);
        }
        if abs_m(ek - z[k]) > a[idx(k, k)] {
            let s = a[idx(k, k)] / abs_m(ek - z[k]);
            dscal(s, &mut z[1..=n]);
            ek *= s;
        }
        let mut wk = ek - z[k];
        let mut wkm = -ek - z[k];
        let mut s = abs_m(wk);
        let mut sm = abs_m(wkm);
        wk /= a[idx(k, k)];
        wkm /= a[idx(k, k)];
        if k < n {
            for j in k + 1..=n {
                sm += abs_m(z[j] + wkm * a[idx(k, j)]);
                z[j] += wk * a[idx(k, j)];
                s += abs_m(z[j]);
            }
            if s < sm {
                let t = wkm - wk;
                wk = wkm;
                for j in k + 1..=n {
                    z[j] += t * a[idx(k, j)];
                }
            }
        }
        z[k] = wk;
    }
    let s = 1.0 / dasum(&z[1..=n]);
    dscal(s, &mut z[1..=n]);
    for kb in 1..=n {
        let k = n + 1 - kb;
        if abs_m(z[k]) > a[idx(k, k)] {
            let s = a[idx(k, k)] / abs_m(z[k]);
            dscal(s, &mut z[1..=n]);
        }
        z[k] /= a[idx(k, k)];
        let t = -z[k];
        let c = (k - 1) * n;
        daxpy(t, &a[c..c + k - 1], &mut z[1..k]);
    }
    let s = 1.0 / dasum(&z[1..=n]);
    dscal(s, &mut z[1..=n]);
    let mut ynorm = 1.0f64;
    for k in 1..=n {
        let c = (k - 1) * n;
        z[k] -= ddot(&a[c..c + k - 1], &z[1..k]);
        if abs_m(z[k]) > a[idx(k, k)] {
            let s = a[idx(k, k)] / abs_m(z[k]);
            dscal(s, &mut z[1..=n]);
            ynorm *= s;
        }
        z[k] /= a[idx(k, k)];
    }
    let s = 1.0 / dasum(&z[1..=n]);
    dscal(s, &mut z[1..=n]);
    ynorm *= s;
    for kb in 1..=n {
        let k = n + 1 - kb;
        if abs_m(z[k]) > a[idx(k, k)] {
            let s = a[idx(k, k)] / abs_m(z[k]);
            dscal(s, &mut z[1..=n]);
            ynorm *= s;
        }
        z[k] /= a[idx(k, k)];
        let t = -z[k];
        let c = (k - 1) * n;
        daxpy(t, &a[c..c + k - 1], &mut z[1..k]);
    }
    let s = 1.0 / dasum(&z[1..=n]);
    dscal(s, &mut z[1..=n]);
    ynorm *= s;
    let mut rcond = None;
    if anorm != 0.0 {
        rcond = Some(ynorm / anorm);
    }
    if anorm == 0.0 {
        rcond = Some(0.0);
    }
    (rcond, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfmt::{Captured, Stream};

    #[test]
    fn factors_solves_and_inverts() {
        let m = [4.0, 2.0, 0.6, 2.0, 5.0, 1.0, 0.6, 1.0, 3.0];
        let mut out = Captured::default();
        let mut ch = VnlCholesky::new(&m, 3, Mode::Verbose, &mut out);
        assert_eq!(ch.rank_deficiency(), 0);
        assert!(out.messages.is_empty());
        let u = ch.upper_triangle();
        for i in 0..3 {
            for j in 0..3 {
                let s: f64 = (0..3).map(|k| u[k * 3 + i] * u[k * 3 + j]).sum();
                assert!((s - m[i * 3 + j]).abs() < 1e-12);
            }
        }
        let x = ch.solve(&[1.0, 2.0, 3.0]);
        for i in 0..3 {
            let s: f64 = (0..3).map(|k| m[i * 3 + k] * x[k]).sum();
            assert!((s - [1.0, 2.0, 3.0][i]).abs() < 1e-12);
        }
        let inv = ch.inverse(&mut out).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                let s: f64 = (0..3).map(|k| m[i * 3 + k] * inv[k * 3 + j]).sum();
                assert!((s - if i == j { 1.0 } else { 0.0 }).abs() < 1e-12);
            }
        }
        let det = 4.0 * (5.0 * 3.0 - 1.0) - 2.0 * (2.0 * 3.0 - 0.6) + 0.6 * (2.0 - 5.0 * 0.6);
        assert!((ch.determinant() - det).abs() < 1e-12);
        let bad = VnlCholesky::new(&[1.0, 2.0, 2.0, 1.0], 2, Mode::Verbose, &mut out);
        assert_eq!(bad.rank_deficiency(), 2);
        assert_eq!(
            out.text(Stream::Err),
            "vnl_cholesky: 2 dimensions of non-posdeffness\n"
        );
    }

    #[test]
    fn a_huge_factor_has_a_determinant() {
        // diag(5e307, 5e307, 1e-300, 1e-300): the second square overflows VNL's mantissa.
        let mut m = [0.0f64; 16];
        for (i, v) in [5e307, 5e307, 1e-300, 1e-300].into_iter().enumerate() {
            m[i * 5] = v;
        }
        let mut ch = VnlCholesky::new(&m, 4, Mode::Quiet, &mut crate::cfmt::Silent);
        let d = ch.determinant();
        assert!((d / 2.5e15 - 1.0).abs() < 1e-12, "{d}");
        assert_eq!(ch.notices.len(), 1);
    }
}
