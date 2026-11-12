// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! vnl's QR decomposition, as ITK's `LandmarkBasedTransformInitializer` solves its affine
//! least-squares system with it (`vnl_qr<double>(Q).solve(C)`).
//!
//! `vnl_qr<double>` transposes its matrix into LINPACK's column-major storage and calls
//! `dqrdc` without pivoting (Householder reflections, the norms from the reference BLAS
//! `dnrm2`); `solve` calls `dqrsl` with `job = 100` (`Qᵀ·b`, then the back-substitution) once
//! per right-hand side column. Both are followed statement by statement as f2c translated
//! them, with LINPACK's 1-based indices, so that every intermediate is rounded as the C code
//! rounds it. Only `sqrt` and the four basic operations are used, which IEEE arithmetic rounds
//! exactly: the result is the same on every platform.
//!
//! Ported from VXL as bundled with ITK v5.4.5 (BSD licence; LINPACK and BLAS from netlib):
//! `core/vnl/algo/vnl_qr.hxx`, `v3p/netlib/linpack/{dqrdc,dqrsl}.c` and the BLAS kernels of
//! [`crate::vnl_svd`].
//!
//! [`Qr`] is the same decomposition for `vnl_qr<float>` and `vnl_qr<double>` as FreeSurfer 8.2.0
//! calls it through ITK 4.13.2's VNL (numerics.cpp's `OpenLUMatrixInverse` above 4 × 4 and
//! `OpenQRdecomposition`; `vnl_determinant` above 4 × 4; `mri_robust_register`'s regression):
//! LINPACK's `sqrdc`/`dqrdc` and `sqrsl`/`dqrsl`, `Q()`, `R()`, `determinant()`, `solve`,
//! `QtB`, `inverse()` and `tinverse()`. For `float`, f2c's single-precision BLAS return
//! `doublereal`: `snrm2` rounds `scale · sqrt(ssq)` (a double square root) to float once,
//! `sdot`'s float sum is widened exactly, and `sqrdc`'s norm downdate `qraux *= sqrt(t)` is a
//! double product rounded to float; every other operation is single precision (`FLT_EVAL_METHOD`
//! 0 on x86-64). The f2c macro `abs` keeps its C semantics. Ported from ITK v4.13.2's
//! VXL (`core/vnl/algo/vnl_qr.hxx`, `v3p/netlib/linpack/{sqrdc,sqrsl,dqrdc,dqrsl}.c`,
//! `v3p/netlib/blas/{snrm2,sdot,saxpy,sscal,scopy}.c` and their double versions; the same code
//! in v5.4.5).

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
// LINPACK (dqrdc, dqrsl) and the reference BLAS are netlib software, distributed by VXL as
// above.

// The loops keep LINPACK's 1-based Fortran indices, so the port can be checked line by line
// against dqrdc.c and dqrsl.c.
#![allow(clippy::needless_range_loop)]

use crate::itk_real::ItkReal;
use crate::vnl_svd::{d_sign, daxpy, ddot, dnrm2, dscal};

/// The message vnl prints on standard error when `solve` meets a zero diagonal entry of `R`
/// (`__FILE__` is the build's path to the header; ITK's own path from `Modules/` is used
/// here), followed by the index LINPACK returns.
pub const RANK_DEFICIENT: &str = "Modules/ThirdParty/VNL/src/vxl/core/vnl/algo/vnl_qr.hxx: vnl_qr<T>::solve() : matrix is rank-deficient by ";

/// `vnl_qr<double>`: LINPACK's factorization of an `n × p` matrix (`n` its rows).
#[derive(Clone, Debug, PartialEq)]
pub struct VnlQr {
    /// `qrdc_out_`, LINPACK's `x` after `dqrdc`: column-major, `n` rows (leading dimension).
    x: Vec<f64>,
    /// `qraux_` (`p` entries).
    qraux: Vec<f64>,
    n: usize,
    p: usize,
}

impl VnlQr {
    /// `vnl_qr<double>(M)` of the `rows × cols` matrix `m` (row-major): `dqrdc` without
    /// pivoting (`job = 0`).
    pub fn new(m: &[f64], rows: usize, cols: usize) -> VnlQr {
        assert!(
            rows > 0 && cols > 0 && m.len() == rows * cols,
            "not a rows × cols matrix"
        );
        let (n, p) = (rows, cols);
        // qrdc_out_(j, i) = M(i, j): row-major p × n, that is column-major n × p.
        let mut x = vec![0.0f64; n * p];
        for i in 0..n {
            for j in 0..p {
                x[i + n * j] = m[i * p + j];
            }
        }
        let mut qraux = vec![0.0f64; p];
        // x(i, j), 1-based, column-major with leading dimension n.
        let at = |i: usize, j: usize| (i - 1) + (j - 1) * n;
        // job == 0: pl = 1, pu = 0, so no column is a pivot candidate and the norms are not
        // computed (`goto L60`, then `pu < pl`).
        let lup = n.min(p);
        for l in 1..=lup {
            qraux[l - 1] = 0.0;
            if l == n {
                continue;
            }
            let len = n - l + 1;
            let ll = at(l, l);
            let mut nrmxl = dnrm2(&x[ll..ll + len]);
            if nrmxl == 0.0 {
                continue;
            }
            if x[ll] != 0.0 {
                nrmxl = d_sign(nrmxl, x[ll]);
            }
            dscal(1.0 / nrmxl, &mut x[ll..ll + len]);
            x[ll] += 1.0;
            for j in l + 1..=p {
                let lj = at(l, j);
                let t = -ddot(&x[ll..ll + len], &x[lj..lj + len]) / x[ll];
                let (head, tail) = x.split_at_mut(lj);
                daxpy(t, &head[ll..ll + len], &mut tail[..len]);
                // `j < pl || j > pu`: the norm updates of pivoting are skipped.
            }
            qraux[l - 1] = x[ll];
            x[ll] = -nrmxl;
        }
        VnlQr { x, qraux, n, p }
    }

    /// `vnl_qr<double>::solve(b)`: `dqrsl` with `job = 100` (`Qᵀb`, then `R x = Qᵀb` by
    /// back-substitution); `b` has `n` entries, the solution `p`. Also returns LINPACK's
    /// `info`: 0, or the index of the first zero diagonal entry of `R` met from the bottom
    /// (vnl then prints [`RANK_DEFICIENT`] and that index on standard error and returns the
    /// solution as far as LINPACK computed it).
    pub fn solve(&self, b: &[f64]) -> (Vec<f64>, usize) {
        assert_eq!(b.len(), self.n, "right-hand side of the wrong length");
        let (n, k) = (self.n, self.p);
        let at = |i: usize, j: usize| (i - 1) + (j - 1) * n;
        let mut x = self.x.clone();
        let qraux = &self.qraux;
        let mut info = 0usize;
        let ju = k.min(n - 1);
        let mut qty = b.to_vec();
        let mut sol = vec![0.0f64; k];
        if ju == 0 {
            // n == 1.
            qty[0] = b[0];
            if x[at(1, 1)] != 0.0 {
                sol[0] = b[0] / x[at(1, 1)];
            } else {
                info = 1;
            }
            return (sol, info);
        }
        // Qᵀ y.
        for j in 1..=ju {
            if qraux[j - 1] == 0.0 {
                continue;
            }
            let jj = at(j, j);
            let temp = x[jj];
            x[jj] = qraux[j - 1];
            let len = n - j + 1;
            let t = -ddot(&x[jj..jj + len], &qty[j - 1..j - 1 + len]) / x[jj];
            daxpy(t, &x[jj..jj + len], &mut qty[j - 1..j - 1 + len]);
            x[jj] = temp;
        }
        // b = the first k entries of Qᵀy, then the back-substitution.
        sol.copy_from_slice(&qty[..k]);
        for jj in 1..=k {
            let j = k - jj + 1;
            let d = x[at(j, j)];
            if d == 0.0 {
                info = j;
                break;
            }
            sol[j - 1] /= d;
            if j == 1 {
                continue;
            }
            let t = -sol[j - 1];
            let c = at(1, j);
            daxpy(t, &x[c..c + j - 1], &mut sol[..j - 1]);
        }
        (sol, info)
    }

    /// `vnl_qr<double>::solve(rhs)` for a matrix: [`VnlQr::solve`] of each column of the
    /// `n × cols` matrix `rhs` (row-major); the result is `p × cols` (row-major), with the
    /// `info` of each column (vnl prints one message per rank-deficient column).
    pub fn solve_matrix(&self, rhs: &[f64], cols: usize) -> (Vec<f64>, Vec<usize>) {
        assert_eq!(
            rhs.len(),
            self.n * cols,
            "right-hand side of the wrong size"
        );
        let mut out = vec![0.0f64; self.p * cols];
        let mut infos = Vec::with_capacity(cols);
        for c in 0..cols {
            let b: Vec<f64> = (0..self.n).map(|r| rhs[r * cols + c]).collect();
            let (x, info) = self.solve(&b);
            for r in 0..self.p {
                out[r * cols + c] = x[r];
            }
            infos.push(info);
        }
        (out, infos)
    }
}

/// The message of `vnl_qr<T>::QtB` for a rank-deficient matrix (never printed: `QtB` does not
/// ask LINPACK for the solution, so `info` stays 0).
pub const QTB_RANK_DEFICIENT: &str = "Modules/ThirdParty/VNL/src/vxl/core/vnl/algo/vnl_qr.hxx: vnl_qr<T>::QtB() -- matrix is rank-deficient by ";

/// f2c's `abs(x)`: `x >= 0 ? x : -x`.
#[inline]
fn abs_m<T: ItkReal>(x: T) -> T {
    if x >= T::ZERO { x } else { -x }
}

/// `r_sign`/`d_sign(a, b)`: `|a|` with the sign of `b` (`b >= 0` counts as positive).
#[inline]
fn sign_t<T: ItkReal>(a: T, b: T) -> T {
    let x = if a >= T::ZERO { a } else { -a };
    if b >= T::ZERO { x } else { -x }
}

/// `snrm2`/`dnrm2` (unit stride): the scaled sum of squares in `T`, the norm `scale·sqrt(ssq)`
/// in double rounded to `T`; one element: `abs`.
fn nrm2_t<T: ItkReal>(x: &[T]) -> T {
    match x.len() {
        0 => T::ZERO,
        1 => abs_m(x[0]),
        _ => {
            let (mut scale, mut ssq) = (T::ZERO, T::ONE);
            for &xi in x {
                if xi != T::ZERO {
                    let absxi = abs_m(xi);
                    if scale < absxi {
                        let r = scale / absxi;
                        ssq = ssq * (r * r) + T::ONE;
                        scale = absxi;
                    } else {
                        let r = absxi / scale;
                        ssq = ssq + r * r;
                    }
                }
            }
            T::from_f64(scale.to_f64() * ssq.to_f64().sqrt())
        }
    }
}

/// `sdot`/`ddot` (unit stride) with the reference BLAS's unrolling by five.
fn dot_t<T: ItkReal>(x: &[T], y: &[T]) -> T {
    let n = x.len();
    let m = n % 5;
    let mut t = T::ZERO;
    for i in 0..m {
        t = t + x[i] * y[i];
    }
    let mut i = m;
    while i + 5 <= n {
        t = t + x[i] * y[i] + x[i + 1] * y[i + 1] + x[i + 2] * y[i + 2] + x[i + 3] * y[i + 3] + x[i + 4] * y[i + 4];
        i += 5;
    }
    t
}

/// `saxpy`/`daxpy`: `y += a·x`, nothing when `a == 0`.
fn axpy_t<T: ItkReal>(a: T, x: &[T], y: &mut [T]) {
    if a == T::ZERO {
        return;
    }
    for (yi, &xi) in y.iter_mut().zip(x) {
        *yi = *yi + a * xi;
    }
}

/// `sscal`/`dscal`: `x = a·x`.
fn scal_t<T: ItkReal>(a: T, x: &mut [T]) {
    for xi in x {
        *xi = a * *xi;
    }
}

/// `vnl_qr<T>` for `T` = `f32` or `f64`: LINPACK's QR factorization of a `rows × cols` matrix
/// without pivoting (`job = 0`), and what VNL computes from it.
#[derive(Clone, Debug, PartialEq)]
pub struct Qr<T> {
    /// `qrdc_out_`: LINPACK's `x` after `xqrdc`, column-major, `n` (the matrix's rows) a column.
    x: Vec<T>,
    /// `qraux_` (`p` entries).
    qraux: Vec<T>,
    n: usize,
    p: usize,
}

/// The result of LINPACK's `xqrsl` as `vnl_qr` asks for it: the vector, and `info` (0, or the
/// index of the zero diagonal entry of `R` the back-substitution stopped at).
pub type Solved<T> = (Vec<T>, usize);

impl<T: ItkReal> Qr<T> {
    /// `vnl_qr<T>(M)` of the `rows × cols` matrix `m` (row-major).
    pub fn new(m: &[T], rows: usize, cols: usize) -> Qr<T> {
        assert!(rows > 0 && cols > 0 && m.len() == rows * cols, "not a rows × cols matrix");
        let (n, p) = (rows, cols);
        let mut x = vec![T::ZERO; n * p];
        for i in 0..n {
            for j in 0..p {
                x[i + n * j] = m[i * p + j];
            }
        }
        let mut qraux = vec![T::ZERO; p];
        let at = |i: usize, j: usize| (i - 1) + (j - 1) * n;
        // job == 0: pl = 1 > pu = 0, so the norms are not computed and no column pivots.
        let lup = n.min(p);
        for l in 1..=lup {
            qraux[l - 1] = T::ZERO;
            if l == n {
                continue;
            }
            let len = n - l + 1;
            let ll = at(l, l);
            let mut nrmxl = nrm2_t(&x[ll..ll + len]);
            if nrmxl == T::ZERO {
                continue;
            }
            if x[ll] != T::ZERO {
                nrmxl = sign_t(nrmxl, x[ll]);
            }
            scal_t(T::ONE / nrmxl, &mut x[ll..ll + len]);
            x[ll] = x[ll] + T::ONE;
            for j in l + 1..=p {
                let lj = at(l, j);
                // -dot / x(l, l): a double quotient of two T values rounded to T, which is T's.
                let t = -dot_t(&x[ll..ll + len], &x[lj..lj + len]) / x[ll];
                let (head, tail) = x.split_at_mut(lj);
                axpy_t(t, &head[ll..ll + len], &mut tail[..len]);
            }
            qraux[l - 1] = x[ll];
            x[ll] = -nrmxl;
        }
        Qr { x, qraux, n, p }
    }

    /// The matrix's rows (`n`) and columns (`p`).
    pub fn shape(&self) -> (usize, usize) {
        (self.n, self.p)
    }

    #[inline]
    fn xat(&self, i: usize, j: usize) -> T {
        self.x[i + self.n * j]
    }

    /// `determinant()`: `R(0, 0)·Π(−R(i, i))` over the square part, in `T`, left to right.
    pub fn determinant(&self) -> T {
        let m = self.n.min(self.p);
        let mut det = self.xat(0, 0);
        for i in 1..m {
            det = det * -self.xat(i, i);
        }
        det
    }

    /// `Q()`: the `rows × rows` orthogonal factor (row-major), the Householder reflections applied
    /// to the identity from the last one, as VNL accumulates them (in `T`).
    pub fn q(&self) -> Vec<T> {
        let (m, n) = (self.n, self.p);
        let mut q = vec![T::ZERO; m * m];
        for i in 0..m {
            q[i * m + i] = T::ONE;
        }
        let mut v = vec![T::ZERO; m];
        let mut w = vec![T::ZERO; m];
        for k in (0..n).rev() {
            if k >= m {
                continue;
            }
            v[k] = self.qraux[k];
            let mut sq = v[k] * v[k];
            for j in k + 1..m {
                v[j] = self.xat(j, k);
                sq = sq + v[j] * v[j];
            }
            if sq > T::ZERO {
                let scale = T::from_f64(2.0) / sq;
                for i in k..m {
                    w[i] = T::ZERO;
                    for j in k..m {
                        w[i] = w[i] + scale * v[j] * q[j * m + i];
                    }
                }
                for i in k..m {
                    for j in k..m {
                        q[i * m + j] = q[i * m + j] - v[i] * w[j];
                    }
                }
            }
        }
        q
    }

    /// `R()`: the `rows × cols` upper-triangular factor (row-major).
    pub fn r(&self) -> Vec<T> {
        let (m, n) = (self.n, self.p);
        let mut r = vec![T::ZERO; m * n];
        for i in 0..m {
            for j in 0..n {
                r[i * n + j] = if i > j { T::ZERO } else { self.xat(i, j) };
            }
        }
        r
    }

    /// LINPACK's `xqrsl` for `y` with VNL's two jobs: `qty_only` (`QtB`, `job = 1000`) or the
    /// least-squares solution (`solve`, `job = 100`). Returns `(Qᵀy, the solution, info)`.
    fn qrsl(&self, y: &[T], solve: bool) -> (Vec<T>, Vec<T>, usize) {
        let (n, k) = (self.n, self.p);
        assert_eq!(y.len(), n, "a right-hand side of the matrix's rows");
        let at = |i: usize, j: usize| (i - 1) + (j - 1) * n;
        let mut x = self.x.clone();
        let qraux = &self.qraux;
        let mut info = 0usize;
        let ju = k.min(n - 1);
        let mut qty = y.to_vec();
        let mut b = vec![T::ZERO; k];
        if ju == 0 {
            qty[0] = y[0];
            if solve {
                if x[at(1, 1)] != T::ZERO {
                    b[0] = y[0] / x[at(1, 1)];
                } else {
                    // VNL's solution is never written: memory contents, 0 here (ITK-113).
                    info = 1;
                }
            }
            return (qty, b, info);
        }
        for j in 1..=ju {
            if qraux[j - 1] == T::ZERO {
                continue;
            }
            let jj = at(j, j);
            let temp = x[jj];
            x[jj] = qraux[j - 1];
            let len = n - j + 1;
            let t = -dot_t(&x[jj..jj + len], &qty[j - 1..j - 1 + len]) / x[jj];
            axpy_t(t, &x[jj..jj + len], &mut qty[j - 1..j - 1 + len]);
            x[jj] = temp;
        }
        if solve {
            assert!(k <= n, "vnl_qr::solve of a matrix with more columns than rows is refused (VNL reads past Qᵀb)");
            b.copy_from_slice(&qty[..k]);
            for jj in 1..=k {
                let j = k - jj + 1;
                let d = x[at(j, j)];
                if d == T::ZERO {
                    info = j;
                    break;
                }
                b[j - 1] = b[j - 1] / d;
                if j == 1 {
                    continue;
                }
                let t = -b[j - 1];
                let c = at(1, j);
                axpy_t(t, &x[c..c + j - 1], &mut b[..j - 1]);
            }
        }
        (qty, b, info)
    }

    /// `solve(b)`: the least-squares solution (`cols` entries) and `info`; VNL prints
    /// [`RANK_DEFICIENT`] and `info` on standard error when it is not 0. For one row whose entry
    /// is 0 VNL leaves the solution unwritten (memory contents): 0 here (ITK-113).
    pub fn solve(&self, b: &[T]) -> Solved<T> {
        let (_, x, info) = self.qrsl(b, true);
        (x, info)
    }

    /// `QtB(b)`: `Qᵀb` (`rows` entries).
    pub fn qtb(&self, b: &[T]) -> Vec<T> {
        self.qrsl(b, false).0
    }

    /// `inverse()` of a square matrix: [`Qr::solve`] of each column of the identity, set as the
    /// columns (row-major result); each column's `info`.
    pub fn inverse(&self) -> (Vec<T>, Vec<usize>) {
        self.inverse_impl(false)
    }

    /// `tinverse()`: the same solutions set as the rows (the inverse's transpose).
    pub fn tinverse(&self) -> (Vec<T>, Vec<usize>) {
        self.inverse_impl(true)
    }

    fn inverse_impl(&self, transposed: bool) -> (Vec<T>, Vec<usize>) {
        let r = self.n;
        assert_eq!(r, self.p, "vnl_qr::inverse of a square matrix");
        let mut inv = vec![T::ZERO; r * r];
        let mut infos = Vec::with_capacity(r);
        let mut rhs = vec![T::ZERO; r];
        for i in 0..r {
            rhs[i] = T::ONE;
            let (col, info) = self.solve(&rhs);
            for k in 0..r {
                if transposed {
                    inv[i * r + k] = col[k];
                } else {
                    inv[k * r + i] = col[k];
                }
            }
            infos.push(info);
            rhs[i] = T::ZERO;
        }
        (inv, infos)
    }

    /// `solve(rhs)` for a `rows × cols` right-hand side matrix (row-major): each column solved;
    /// the result `p × cols`, and each column's `info`.
    pub fn solve_matrix(&self, rhs: &[T], cols: usize) -> (Vec<T>, Vec<usize>) {
        assert_eq!(rhs.len(), self.n * cols, "right-hand side of the wrong size");
        let mut out = vec![T::ZERO; self.p * cols];
        let mut infos = Vec::with_capacity(cols);
        for c in 0..cols {
            let b: Vec<T> = (0..self.n).map(|r| rhs[r * cols + c]).collect();
            let (x, info) = self.solve(&b);
            for r in 0..self.p {
                out[r * cols + c] = x[r];
            }
            infos.push(info);
        }
        (out, infos)
    }
}

/// `vnl_qr<T>::determinant` through [`Qr`] (`vnl_qr_determinant`).
pub fn qr_determinant<T: ItkReal>(m: &[T], n: usize) -> T {
    Qr::new(m, n, n).determinant()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square system solved exactly enough.
    #[test]
    fn solves_a_square_system() {
        let a = [4.0, 1.0, 2.0, 1.0, 3.0, 0.5, 2.0, 0.5, 5.0];
        let x_true = [1.0, -2.0, 0.5];
        let b: Vec<f64> = (0..3)
            .map(|i| (0..3).map(|j| a[i * 3 + j] * x_true[j]).sum())
            .collect();
        let (x, info) = VnlQr::new(&a, 3, 3).solve(&b);
        assert_eq!(info, 0);
        for i in 0..3 {
            assert!((x[i] - x_true[i]).abs() < 1e-14, "{x:?}");
        }
    }

    /// LINPACK's answer to a singular system: the zero diagonal entry is reported.
    #[test]
    fn reports_a_zero_pivot() {
        let a = [1.0, 2.0, 2.0, 4.0];
        let (_, info) = VnlQr::new(&a, 2, 2).solve(&[1.0, 2.0]);
        assert!(info <= 2);
        let z = [0.0, 0.0, 0.0, 0.0];
        let (_, info) = VnlQr::new(&z, 2, 2).solve(&[1.0, 2.0]);
        assert_eq!(info, 2);
    }

    /// The generic double decomposition solves exactly as the double one.
    #[test]
    fn the_generic_double_qr_is_the_double_qr() {
        let mut rng = crate::x87::cases::SplitMix64::new(7);
        for _ in 0..200 {
            let (r, c) = (1 + rng.below(7) as usize, 1 + rng.below(7) as usize);
            let (r, c) = (r.max(c), c.min(r));
            let a: Vec<f64> = (0..r * c).map(|_| rng.range(-1000, 1000) as f64 / 37.0).collect();
            let b: Vec<f64> = (0..r).map(|_| rng.range(-1000, 1000) as f64 / 13.0).collect();
            let old = VnlQr::new(&a, r, c).solve(&b);
            let new = Qr::new(&a, r, c).solve(&b);
            assert_eq!(old, new);
        }
    }
}
