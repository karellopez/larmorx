// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! vnl's singular value decomposition and the matrix inverse ITK computes with it.
//!
//! `itk::Matrix::GetInverse` (every point → index conversion of an image, every inverted
//! affine) is `vnl_matrix_inverse`, an SVD pseudo-inverse: LINPACK's `dsvdc` as translated by
//! f2c in VXL's `v3p/netlib`, then `V · diag(1/w) · Uᵀ`. For oblique matrices it differs in the
//! last bits from a cofactor inverse in most entries, so it is ported here operation for
//! operation, with the reference BLAS kernels `dsvdc` calls. Only `sqrt` is used, which IEEE
//! rounds exactly, so the result is the same on every platform.
//!
//! Ported from VXL as bundled with ITK v5.4.5 (BSD licence; LINPACK and BLAS from netlib):
//! `core/vnl/algo/vnl_svd.hxx`, `vnl_matrix_inverse.h`, `v3p/netlib/linpack/dsvdc.c` and
//! `v3p/netlib/blas/{dnrm2,ddot,daxpy,dscal,dswap,drot,drotg}.c`.

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
//
// LINPACK (dsvdc) and the reference BLAS are netlib software, distributed by VXL as above.

// The loops keep LINPACK's 1-based Fortran indices, so the port can be checked line by line
// against dsvdc.c.
#![allow(clippy::needless_range_loop)]

use crate::linalg::Mat3;

// ------------------------------------------------------------------------------------------------
// Reference BLAS (unit stride), as in v3p/netlib/blas.

fn dnrm2(x: &[f64]) -> f64 {
    match x.len() {
        0 => 0.0,
        1 => x[0].abs(),
        _ => {
            let (mut scale, mut ssq) = (0.0f64, 1.0f64);
            for &xi in x {
                if xi != 0.0 {
                    let absxi = xi.abs();
                    if scale < absxi {
                        let r = scale / absxi;
                        ssq = ssq * (r * r) + 1.0;
                        scale = absxi;
                    } else {
                        let r = absxi / scale;
                        ssq += r * r;
                    }
                }
            }
            scale * ssq.sqrt()
        }
    }
}

fn ddot(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    let m = n % 5;
    let mut t = 0.0f64;
    for i in 0..m {
        t += x[i] * y[i];
    }
    let mut i = m;
    while i + 5 <= n {
        t = t
            + x[i] * y[i]
            + x[i + 1] * y[i + 1]
            + x[i + 2] * y[i + 2]
            + x[i + 3] * y[i + 3]
            + x[i + 4] * y[i + 4];
        i += 5;
    }
    t
}

/// `y += a·x` (elementwise, so unrolling does not change the result).
fn daxpy(a: f64, x: &[f64], y: &mut [f64]) {
    if a == 0.0 {
        return;
    }
    for (yi, &xi) in y.iter_mut().zip(x) {
        *yi += a * xi;
    }
}

fn dscal(a: f64, x: &mut [f64]) {
    for xi in x {
        *xi *= a;
    }
}

/// `drotg`: the Givens rotation zeroing `b`; returns `(r, z, c, s)`.
fn drotg(a: f64, b: f64) -> (f64, f64, f64, f64) {
    let roe = if a.abs() > b.abs() { a } else { b };
    let scale = a.abs() + b.abs();
    if scale == 0.0 {
        return (0.0, 0.0, 1.0, 0.0);
    }
    let (da, db) = (a / scale, b / scale);
    let mut r = scale * (da * da + db * db).sqrt();
    r *= if roe >= 0.0 { 1.0 } else { -1.0 };
    let c = a / r;
    let s = b / r;
    let mut z = 1.0;
    if a.abs() > b.abs() {
        z = s;
    }
    if b.abs() >= a.abs() && c != 0.0 {
        z = 1.0 / c;
    }
    (r, z, c, s)
}

/// `d_sign(a, b)`: |a| with the sign of b (b ≥ 0 counts as positive).
fn d_sign(a: f64, b: f64) -> f64 {
    let x = if a >= 0.0 { a } else { -a };
    if b >= 0.0 { x } else { -x }
}

/// Column-major matrix with LINPACK's 1-based indexing.
struct Fortran {
    data: Vec<f64>,
    ld: usize,
}

impl Fortran {
    fn new(rows: usize, cols: usize) -> Self {
        Fortran {
            data: vec![0.0; rows * cols],
            ld: rows,
        }
    }

    #[inline]
    fn idx(&self, i: usize, j: usize) -> usize {
        (i - 1) + (j - 1) * self.ld
    }

    #[inline]
    fn at(&self, i: usize, j: usize) -> f64 {
        self.data[self.idx(i, j)]
    }

    #[inline]
    fn set(&mut self, i: usize, j: usize, v: f64) {
        let k = self.idx(i, j);
        self.data[k] = v;
    }

    /// Rows `i..=last` of column `j`.
    fn col(&self, i: usize, j: usize, len: usize) -> &[f64] {
        let k = self.idx(i, j);
        &self.data[k..k + len]
    }

    fn col_mut(&mut self, i: usize, j: usize, len: usize) -> &mut [f64] {
        let k = self.idx(i, j);
        &mut self.data[k..k + len]
    }

    /// Two disjoint column segments: (rows i.. of column ja, rows i.. of column jb), ja ≠ jb.
    fn two_cols(&mut self, i: usize, ja: usize, jb: usize, len: usize) -> (&mut [f64], &mut [f64]) {
        let (ka, kb) = (self.idx(i, ja), self.idx(i, jb));
        if ka < kb {
            let (lo, hi) = self.data.split_at_mut(kb);
            (&mut lo[ka..ka + len], &mut hi[..len])
        } else {
            let (lo, hi) = self.data.split_at_mut(ka);
            (&mut hi[..len], &mut lo[kb..kb + len])
        }
    }
}

fn drot(x: &mut [f64], y: &mut [f64], c: f64, s: f64) {
    for (xi, yi) in x.iter_mut().zip(y.iter_mut()) {
        let t = c * *xi + s * *yi;
        *yi = c * *yi - s * *xi;
        *xi = t;
    }
}

/// The SVD of a `rows × cols` matrix (row-major input), as `vnl_svd` computes it:
/// `(U (rows × cols, column-major), w (singular values, ≥ 0), V (cols × cols, column-major))`,
/// or `None` if LINPACK fails to converge. Requires `rows ≥ cols` (economy size, `job = 21`).
#[allow(clippy::many_single_char_names)]
pub fn vnl_svd(m: &[f64], rows: usize, cols: usize) -> Option<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    let (n, p) = (rows, cols);
    assert!(n >= p && p > 0 && m.len() == n * p);
    // vnl_fortran_copy: column-major.
    let mut x = Fortran::new(n, p);
    for i in 0..n {
        for j in 0..p {
            x.set(i + 1, j + 1, m[i * p + j]);
        }
    }
    let mm_len = (n + 1).min(p);
    let mut s = vec![0.0f64; mm_len + 1]; // 1-based
    let mut e = vec![0.0f64; p + 1];
    let mut u = Fortran::new(n, p);
    let mut v = Fortran::new(p, p);
    let mut work = vec![0.0f64; n + 1];
    let ncu = n.min(p); // job = 21: jobu = 2
    let (wantu, wantv) = (true, true);

    // Householder reduction to bidiagonal form.
    let nct = (n - 1).min(p);
    let nrt = 0.max((p as isize - 2).min(n as isize)) as usize;
    let lu = nct.max(nrt);
    for l in 1..=lu {
        let lp1 = l + 1;
        if l <= nct {
            // Transformation for column l: s[l] is the diagonal element.
            s[l] = dnrm2(x.col(l, l, n - l + 1));
            if s[l] != 0.0 {
                if x.at(l, l) != 0.0 {
                    s[l] = d_sign(s[l], x.at(l, l));
                }
                let scale = 1.0 / s[l];
                dscal(scale, x.col_mut(l, l, n - l + 1));
                x.set(l, l, x.at(l, l) + 1.0);
            }
            s[l] = -s[l];
        }
        if p >= lp1 {
            for j in lp1..=p {
                if l <= nct && s[l] != 0.0 {
                    let t = -ddot(x.col(l, l, n - l + 1), x.col(l, j, n - l + 1)) / x.at(l, l);
                    let (xl, xj) = x.two_cols(l, l, j, n - l + 1);
                    daxpy(t, xl, xj);
                }
                // The row of the bidiagonal factor to be transformed.
                e[j] = x.at(l, j);
            }
        }
        if wantu && l <= nct {
            for i in l..=n {
                u.set(i, l, x.at(i, l));
            }
        }
        if l <= nrt {
            // Transformation for row l: e[l] is the superdiagonal element.
            e[l] = dnrm2(&e[lp1..=p]);
            if e[l] != 0.0 {
                if e[lp1] != 0.0 {
                    e[l] = d_sign(e[l], e[lp1]);
                }
                let scale = 1.0 / e[l];
                dscal(scale, &mut e[lp1..=p]);
                e[lp1] += 1.0;
            }
            e[l] = -e[l];
            if lp1 <= n && e[l] != 0.0 {
                for w in work.iter_mut().take(n + 1).skip(lp1) {
                    *w = 0.0;
                }
                for j in lp1..=p {
                    daxpy(e[j], x.col(lp1, j, n - l), &mut work[lp1..=n]);
                }
                for j in lp1..=p {
                    let a = -e[j] / e[lp1];
                    daxpy(a, &work[lp1..=n], x.col_mut(lp1, j, n - l));
                }
            }
            if wantv {
                for i in lp1..=p {
                    v.set(i, l, e[i]);
                }
            }
        }
    }

    // The final bidiagonal matrix of order m.
    let mut m_ = p.min(n + 1);
    let nctp1 = nct + 1;
    let nrtp1 = nrt + 1;
    if nct < p {
        s[nctp1] = x.at(nctp1, nctp1);
    }
    if n < m_ {
        s[m_] = 0.0;
    }
    if nrtp1 < m_ {
        e[nrtp1] = x.at(nrtp1, m_);
    }
    e[m_] = 0.0;

    // Generate U.
    if wantu {
        if ncu >= nctp1 {
            for j in nctp1..=ncu {
                for i in 1..=n {
                    u.set(i, j, 0.0);
                }
                u.set(j, j, 1.0);
            }
        }
        for ll in 1..=nct {
            let l = nct - ll + 1;
            if s[l] != 0.0 {
                let lp1 = l + 1;
                if ncu >= lp1 {
                    for j in lp1..=ncu {
                        let t = -ddot(u.col(l, l, n - l + 1), u.col(l, j, n - l + 1)) / u.at(l, l);
                        let (ul, uj) = u.two_cols(l, l, j, n - l + 1);
                        daxpy(t, ul, uj);
                    }
                }
                dscal(-1.0, u.col_mut(l, l, n - l + 1));
                u.set(l, l, u.at(l, l) + 1.0);
                for i in 1..l {
                    u.set(i, l, 0.0);
                }
            } else {
                for i in 1..=n {
                    u.set(i, l, 0.0);
                }
                u.set(l, l, 1.0);
            }
        }
    }

    // Generate V.
    if wantv {
        for ll in 1..=p {
            let l = p - ll + 1;
            let lp1 = l + 1;
            if l <= nrt && e[l] != 0.0 {
                for j in lp1..=p {
                    let t = -ddot(v.col(lp1, l, p - l), v.col(lp1, j, p - l)) / v.at(lp1, l);
                    let (vl, vj) = v.two_cols(lp1, l, j, p - l);
                    daxpy(t, vl, vj);
                }
            }
            for i in 1..=p {
                v.set(i, l, 0.0);
            }
            v.set(l, l, 1.0);
        }
    }

    // Main iteration loop for the singular values.
    let mm = m_;
    let mut iter = 0;
    let maxit = 1000;
    while m_ != 0 {
        if iter >= maxit {
            return None;
        }
        // Find l: e[l] is negligible, or l = 0.
        let mut l = 0usize;
        for ll in 1..=m_ {
            l = m_ - ll;
            if l == 0 {
                break;
            }
            let test = s[l].abs() + s[l + 1].abs();
            let ztest = test + e[l].abs();
            if ztest == test {
                e[l] = 0.0;
                break;
            }
        }
        let kase;
        if l == m_ - 1 {
            kase = 4;
        } else {
            let lp1 = l + 1;
            let mp1 = m_ + 1;
            let mut ls = 0usize;
            for lls in lp1..=mp1 {
                ls = m_ + lp1 - lls;
                if ls == l {
                    break;
                }
                let mut test = 0.0;
                if ls != m_ {
                    test += e[ls].abs();
                }
                if ls != l + 1 {
                    test += e[ls - 1].abs();
                }
                let ztest = test + s[ls].abs();
                if ztest == test {
                    s[ls] = 0.0;
                    break;
                }
            }
            if ls == l {
                kase = 3;
            } else if ls == m_ {
                kase = 1;
            } else {
                kase = 2;
                l = ls;
            }
        }
        l += 1;
        match kase {
            1 => {
                // Deflate negligible s[m].
                let mm1 = m_ - 1;
                let mut f = e[m_ - 1];
                e[m_ - 1] = 0.0;
                for kk in l..=mm1 {
                    let k = mm1 - kk + l;
                    let (r, _, cs, sn) = drotg(s[k], f);
                    s[k] = r;
                    if k != l {
                        f = -sn * e[k - 1];
                        e[k - 1] *= cs;
                    }
                    if wantv {
                        let (vk, vm) = v.two_cols(1, k, m_, p);
                        drot(vk, vm, cs, sn);
                    }
                }
            }
            2 => {
                // Split at negligible s[l].
                let mut f = e[l - 1];
                e[l - 1] = 0.0;
                for k in l..=m_ {
                    let (r, _, cs, sn) = drotg(s[k], f);
                    s[k] = r;
                    f = -sn * e[k];
                    e[k] *= cs;
                    if wantu {
                        let (uk, ul) = u.two_cols(1, k, l - 1, n);
                        drot(uk, ul, cs, sn);
                    }
                }
            }
            3 => {
                // One QR step: the shift.
                let scale = s[m_]
                    .abs()
                    .max(s[m_ - 1].abs())
                    .max(e[m_ - 1].abs())
                    .max(s[l].abs())
                    .max(e[l].abs());
                let sm = s[m_] / scale;
                let smm1 = s[m_ - 1] / scale;
                let emm1 = e[m_ - 1] / scale;
                let sl = s[l] / scale;
                let el = e[l] / scale;
                let b = ((smm1 + sm) * (smm1 - sm) + emm1 * emm1) / 2.0;
                let c = (sm * emm1) * (sm * emm1);
                let mut shift = 0.0;
                if !(b == 0.0 && c == 0.0) {
                    shift = (b * b + c).sqrt();
                    if b < 0.0 {
                        shift = -shift;
                    }
                    shift = c / (b + shift);
                }
                let mut f = (sl + sm) * (sl - sm) + shift;
                let mut g = sl * el;
                // Chase zeros.
                let mm1 = m_ - 1;
                for k in l..=mm1 {
                    let (r, _, cs, sn) = drotg(f, g);
                    f = r;
                    if k != l {
                        e[k - 1] = f;
                    }
                    f = cs * s[k] + sn * e[k];
                    e[k] = cs * e[k] - sn * s[k];
                    g = sn * s[k + 1];
                    s[k + 1] *= cs;
                    if wantv {
                        let (vk, vk1) = v.two_cols(1, k, k + 1, p);
                        drot(vk, vk1, cs, sn);
                    }
                    let (r, _, cs, sn) = drotg(f, g);
                    f = r;
                    s[k] = f;
                    f = cs * e[k] + sn * s[k + 1];
                    s[k + 1] = -sn * e[k] + cs * s[k + 1];
                    g = sn * e[k + 1];
                    e[k + 1] *= cs;
                    if wantu && k < n {
                        let (uk, uk1) = u.two_cols(1, k, k + 1, n);
                        drot(uk, uk1, cs, sn);
                    }
                }
                e[m_ - 1] = f;
                iter += 1;
            }
            _ => {
                // Convergence: make the singular value positive, then order them.
                if s[l] < 0.0 {
                    s[l] = -s[l];
                    if wantv {
                        dscal(-1.0, v.col_mut(1, l, p));
                    }
                }
                while l != mm && s[l] < s[l + 1] {
                    s.swap(l, l + 1);
                    if wantv && l < p {
                        let (a, b) = v.two_cols(1, l, l + 1, p);
                        a.swap_with_slice(b);
                    }
                    if wantu && l < n {
                        let (a, b) = u.two_cols(1, l, l + 1, n);
                        a.swap_with_slice(b);
                    }
                    l += 1;
                }
                iter = 0;
                m_ -= 1;
            }
        }
    }
    let w = (0..p)
        .map(|j| if j < mm_len { s[j + 1].abs() } else { 0.0 })
        .collect();
    Some((u.data, w, v.data))
}

/// `vnl_matrix_inverse<double>(m).as_matrix()` for a 3×3 matrix: `V · W⁻¹ · Uᵀ` from
/// [`vnl_svd`], zero singular values dropped (`zero_out_absolute(0)`), each product summed
/// from 0 in index order as `vnl_matrix::operator*` does. `None` if the SVD fails.
pub fn vnl_inverse3(m: &Mat3) -> Option<Mat3> {
    let flat: Vec<f64> = m.iter().flatten().copied().collect();
    let (u, w, v) = vnl_svd(&flat, 3, 3)?;
    // Column-major U and V: U(i, j) = u[i + 3j].
    let winv: [f64; 3] = std::array::from_fn(|k| if w[k].abs() <= 0.0 { 0.0 } else { 1.0 / w[k] });
    // V · diag(winv): a full product with the zero entries, as vnl computes it.
    let mut vw = [[0.0f64; 3]; 3];
    for (i, row) in vw.iter_mut().enumerate() {
        for (k, out) in row.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (j, &wj) in winv.iter().enumerate() {
                let d = if j == k { wj } else { 0.0 };
                sum += v[i + 3 * j] * d;
            }
            *out = sum;
        }
    }
    // (V · W⁻¹) · Uᵀ.
    Some(std::array::from_fn(|i| {
        std::array::from_fn(|k| {
            let mut sum = 0.0;
            for j in 0..3 {
                sum += vw[i][j] * u[k + 3 * j];
            }
            sum
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matmul(a: &Mat3, b: &Mat3) -> Mat3 {
        std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
    }

    #[test]
    fn inverts_and_decomposes() {
        let m = [[2.0, 0.3, -0.1], [0.2, 1.5, 0.4], [-0.3, 0.1, 3.2]];
        let inv = vnl_inverse3(&m).unwrap();
        let id = matmul(&m, &inv);
        for (i, row) in id.iter().enumerate() {
            for (j, &x) in row.iter().enumerate() {
                assert!((x - if i == j { 1.0 } else { 0.0 }).abs() < 1e-14, "{id:?}");
            }
        }
        let (_, w, _) = vnl_svd(&m.iter().flatten().copied().collect::<Vec<_>>(), 3, 3).unwrap();
        assert!(w[0] >= w[1] && w[1] >= w[2] && w[2] > 0.0);
    }

    #[test]
    fn signed_permutations_of_diagonals_invert_exactly() {
        let m = [[0.0, -2.7, 0.0], [0.0, 0.0, 3.3], [1.7, 0.0, 0.0]];
        let inv = vnl_inverse3(&m).unwrap();
        assert_eq!(
            inv,
            [
                [0.0, 0.0, 1.0 / 1.7],
                [-1.0 / 2.7, 0.0, 0.0],
                [0.0, 1.0 / 3.3, 0.0]
            ]
        );
    }

    #[test]
    fn singular_matrices_get_a_pseudo_inverse() {
        let m = [[1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 2.0]];
        let inv = vnl_inverse3(&m).unwrap();
        assert_eq!(inv[1][1], 0.0);
        assert_eq!(inv[2][2], 0.5);
    }
}
