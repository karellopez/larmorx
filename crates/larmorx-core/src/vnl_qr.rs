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
}
