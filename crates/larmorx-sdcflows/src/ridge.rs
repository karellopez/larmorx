// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! scikit-learn's ridge regression on a sparse float32 matrix without an intercept, the fit
//! sdcflows' `BSplineApprox` runs (`Ridge(alpha, fit_intercept=False).fit(colmat, data)`), with
//! SciPy's conjugate gradients under it, in their operation order.
//!
//! - **scikit-learn 1.9.1** (BSD-3-Clause; identical on this path in 1.6.1):
//!   `sklearn/linear_model/_ridge.py`. `Ridge.fit` keeps float32 (`validate_data` with dtypes
//!   float64, float32), `_preprocess_data` copies X with offsets 0 and scales 1 when
//!   `fit_intercept=False`, and `resolve_solver_for_numpy` picks **`sparse_cg`** for a sparse X
//!   (no `positive`, no intercept); `_ridge_regression` makes alpha a float32 array and calls
//!   `_solve_sparse_cg` without offsets, so `X1 = aslinearoperator(X)`. With more features than
//!   samples it solves the **kernel** system `(X Xᵀ + αI) w = y` on the samples (SciPy's `cg`
//!   with `rtol=tol` and no `maxiter`, so scikit-learn's `max_iter` is not used there) and
//!   returns `Xᵀ w`; otherwise the **normal equations** `(XᵀX + αI) c = Xᵀy` (`cg` with
//!   `maxiter=max_iter`, `rtol=tol`). The operator is `X1.rmatvec(X1.matvec(v)) + α·v` (kernel:
//!   `X1.matvec(X1.rmatvec(v)) + α·v`), α the float32 scalar, every step a float32 array
//!   operation. `info < 0` cannot happen; a non-converged solve (`info > 0`) only warns when
//!   `verbose`, which `Ridge` never sets. `intercept_` is 0.0.
//! - **SciPy 1.18.1** (BSD-3-Clause; the same arithmetic in 1.15.3, which also passed the
//!   result through an identity `postprocess`): `scipy/sparse/linalg/_isolve/iterative.py`
//!   (`cg`, `_get_atol_rtol`), `utils.py` (`make_system`: x0 = float32 zeros, the identity
//!   preconditioner, which returns its argument itself). `bnrm2 = norm(b)`; `atol = max(atol,
//!   rtol · float(bnrm2))` in double (Python's `max`: the first argument unless the second is
//!   greater); `bnrm2 == 0` returns `b` itself with info 0; `maxiter = 10 n` by default; each
//!   iteration tests `norm(r) < atol` (numpy compares a float32 with a Python float in
//!   **float32**, NEP 50: atol is rounded to float32), `z = r`, `rho = dot(r, z)`, `p = z` or
//!   `p *= beta; p += z`, `q = A p`, `alpha = rho / dot(p, q)`, `x += alpha·p`, `r -= alpha·q`
//!   (each product a float32 temporary); an exhausted loop returns info = maxiter (0 when
//!   maxiter is 0: the `for ... else` runs).
//! - Inner products and norms: numpy's `np.dot` / `np.linalg.norm` of float32 vectors, over
//!   OpenBLAS's Haswell `sdot` ([`crate::blas`]). Matrix products: scipy's `csr_matvec` and
//!   `csc_matvec` order ([`Operator`]).
//!
//! The logic is re-expressed in Rust; no code was copied.
//!
//! scikit-learn: Copyright (c) 2007-2025 The scikit-learn developers. All rights reserved.
//! SciPy: Copyright (c) 2001-2002 Enthought, Inc. 2003, SciPy Developers. All rights reserved.
//! Redistribution and use in source and binary forms, with or without modification, are
//! permitted provided that the following conditions are met: redistributions of source code
//! must retain the above copyright notice, this list of conditions and the following
//! disclaimer; redistributions in binary form must reproduce the above copyright notice, this
//! list of conditions and the following disclaimer in the documentation and/or other materials
//! provided with the distribution; neither the name of the copyright holder nor the names of
//! its contributors may be used to endorse or promote products derived from this software
//! without specific prior written permission. THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT
//! HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
//! LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
//! ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY
//! DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
//! NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
//! OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
//! CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
//! OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use larmorx_core::parallel::with_threads;
use rayon::prelude::*;

use crate::blas::{dot_f32, norm_f32};
use crate::bspline::{Collocation, Csr};
use crate::{SdcError, npy};

/// A float32 matrix as scipy applies it: `matvec` is `X @ v` (`csr_matvec`: each row a float32
/// sum from +0 of the rounded products in storage order), `rmatvec` is `X.T @ y` through the
/// transposed matrix (`csc_matvec`: each column's sum from +0 in ascending row order).
pub trait Operator: Sync {
    /// `(rows, cols)`.
    fn shape(&self) -> [usize; 2];
    /// `X @ v`.
    fn matvec(&self, v: &[f32], n_threads: usize) -> Result<Vec<f32>, SdcError>;
    /// `X.T @ y`.
    fn rmatvec(&self, y: &[f32], n_threads: usize) -> Result<Vec<f32>, SdcError>;
}

impl Operator for Collocation {
    fn shape(&self) -> [usize; 2] {
        Collocation::shape(self)
    }

    fn matvec(&self, v: &[f32], n_threads: usize) -> Result<Vec<f32>, SdcError> {
        self.matvec_f32(v, None, n_threads)
    }

    fn rmatvec(&self, y: &[f32], n_threads: usize) -> Result<Vec<f32>, SdcError> {
        self.rmatvec_f32(y, n_threads)
    }
}

/// A stored float32 CSR matrix with its transpose, for [`Operator`]: rows in storage order
/// (duplicates and unsorted indices as stored), the transpose's columns in ascending row order
/// and, within a row, storage order (what `csc_matvec` gives on scipy's `X.T`).
#[derive(Clone, Debug)]
pub struct CsrOperator {
    csr: Csr<f32>,
    t_indptr: Vec<usize>,
    t_rows: Vec<usize>,
    t_data: Vec<f32>,
}

impl CsrOperator {
    /// Checks the arrays (scipy's `check_format` rules that matter here: `indptr` of length
    /// rows + 1, from 0, non-decreasing, ending at the entry count; every index below the
    /// column count).
    pub fn new(csr: Csr<f32>) -> Result<CsrOperator, SdcError> {
        let [rows, cols] = csr.shape;
        let nnz = csr.data.len();
        if csr.indptr.len() != rows + 1
            || csr.indptr[0] != 0
            || csr.indptr[rows] != nnz
            || csr.indices.len() != nnz
            || csr.indptr.windows(2).any(|w| w[0] > w[1])
        {
            return Err(SdcError::Invalid(
                "the CSR arrays do not describe a matrix of this shape".into(),
            ));
        }
        if csr.indices.iter().any(|&c| c >= cols) {
            return Err(SdcError::Invalid(
                "column index exceeds matrix dimensions".into(),
            ));
        }
        // Stable counting sort by column: per column, rows ascending, storage order kept.
        let mut t_indptr = vec![0usize; cols + 1];
        for &c in &csr.indices {
            t_indptr[c + 1] += 1;
        }
        for c in 0..cols {
            t_indptr[c + 1] += t_indptr[c];
        }
        let mut next = t_indptr.clone();
        let mut t_rows = vec![0usize; nnz];
        let mut t_data = vec![0.0f32; nnz];
        for r in 0..rows {
            for jj in csr.indptr[r]..csr.indptr[r + 1] {
                let c = csr.indices[jj];
                t_rows[next[c]] = r;
                t_data[next[c]] = csr.data[jj];
                next[c] += 1;
            }
        }
        Ok(CsrOperator {
            csr,
            t_indptr,
            t_rows,
            t_data,
        })
    }
}

impl Operator for CsrOperator {
    fn shape(&self) -> [usize; 2] {
        self.csr.shape
    }

    fn matvec(&self, v: &[f32], n_threads: usize) -> Result<Vec<f32>, SdcError> {
        let [rows, cols] = self.csr.shape;
        if v.len() != cols {
            return Err(SdcError::Invalid("dimension mismatch".into()));
        }
        let c = &self.csr;
        Ok(with_threads(n_threads, || {
            (0..rows)
                .into_par_iter()
                .with_min_len(1024)
                .map(|r| {
                    let mut sum = 0.0f32;
                    for jj in c.indptr[r]..c.indptr[r + 1] {
                        sum += c.data[jj] * v[c.indices[jj]];
                    }
                    sum
                })
                .collect()
        })?)
    }

    fn rmatvec(&self, y: &[f32], n_threads: usize) -> Result<Vec<f32>, SdcError> {
        let [rows, cols] = self.csr.shape;
        if y.len() != rows {
            return Err(SdcError::Invalid("dimension mismatch".into()));
        }
        Ok(with_threads(n_threads, || {
            (0..cols)
                .into_par_iter()
                .with_min_len(256)
                .map(|col| {
                    let mut sum = 0.0f32;
                    for k in self.t_indptr[col]..self.t_indptr[col + 1] {
                        sum += self.t_data[k] * y[self.t_rows[k]];
                    }
                    sum
                })
                .collect()
        })?)
    }
}

/// The result of SciPy's `cg`.
#[derive(Clone, Debug, PartialEq)]
pub struct CgResult {
    /// The solution (`b` itself when `norm(b) == 0`).
    pub x: Vec<f32>,
    /// SciPy's `info`: 0 when converged, else the iteration limit.
    pub info: usize,
    /// The iterations run (each one product with the operator).
    pub iterations: usize,
}

/// SciPy's `cg(A, b, rtol=rtol, atol=atol, maxiter=maxiter)` for a float32 system, `matvec`
/// the operator `A @ v`, from x0 = 0 without a preconditioner.
pub fn cg_f32<F>(
    mut matvec: F,
    b: &[f32],
    rtol: f64,
    atol: f64,
    maxiter: Option<usize>,
) -> Result<CgResult, SdcError>
where
    F: FnMut(&[f32]) -> Result<Vec<f32>, SdcError>,
{
    let bnrm2 = norm_f32(b);
    // _get_atol_rtol: `atol < 0` is refused (NaN passes), then Python's max.
    if atol < 0.0 {
        return Err(SdcError::Invalid(format!(
            "'scipy.sparse.linalg.cg' called with invalid `atol`={}; if set, `atol` must be a \
             real, non-negative number.",
            npy::py_repr(atol)
        )));
    }
    let candidate = rtol * f64::from(bnrm2);
    let atol = if candidate > atol { candidate } else { atol };
    if bnrm2 == 0.0 {
        return Ok(CgResult {
            x: b.to_vec(),
            info: 0,
            iterations: 0,
        });
    }
    let n = b.len();
    let maxiter = maxiter.unwrap_or(10 * n);
    // The comparison `norm(r) < atol` happens in float32 (NEP 50).
    let atol32 = atol as f32;
    let mut x = vec![0.0f32; n];
    let mut r = b.to_vec();
    let mut p: Vec<f32> = Vec::new();
    let mut rho_prev = 0.0f32;
    for iteration in 0..maxiter {
        if norm_f32(&r) < atol32 {
            return Ok(CgResult {
                x,
                info: 0,
                iterations: iteration,
            });
        }
        // z = psolve(r) is r itself.
        let rho_cur = dot_f32(&r, &r);
        if iteration > 0 {
            let beta = rho_cur / rho_prev;
            for (pi, &ri) in p.iter_mut().zip(&r) {
                *pi *= beta;
                *pi += ri;
            }
        } else {
            p = r.clone();
        }
        let q = matvec(&p)?;
        if q.len() != n {
            return Err(SdcError::Invalid("dimension mismatch".into()));
        }
        let alpha = rho_cur / dot_f32(&p, &q);
        for (xi, &pi) in x.iter_mut().zip(&p) {
            *xi += alpha * pi;
        }
        for (ri, &qi) in r.iter_mut().zip(&q) {
            *ri -= alpha * qi;
        }
        rho_prev = rho_cur;
    }
    Ok(CgResult {
        x,
        info: maxiter,
        iterations: maxiter,
    })
}

/// Which system scikit-learn's `_solve_sparse_cg` solved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RidgeSystem {
    /// `(XᵀX + αI) c = Xᵀy` on the features.
    Normal,
    /// `(X Xᵀ + αI) w = y` on the samples, `c = Xᵀw` (more features than samples).
    Kernel,
}

/// The result of [`ridge_sparse_cg`].
#[derive(Clone, Debug, PartialEq)]
pub struct RidgeFit {
    /// `coef_` (float32, one per column of X).
    pub coef: Vec<f32>,
    /// SciPy's `info` of the solve (0 when converged).
    pub info: usize,
    /// Conjugate-gradient iterations.
    pub iterations: usize,
    pub system: RidgeSystem,
}

/// scikit-learn's `Ridge(alpha, fit_intercept=False, tol=tol, max_iter=max_iter).fit(X, y)`
/// for a sparse float32 `X` (the `sparse_cg` solver its `auto` picks): `coef_`.
///
/// `alpha` and `tol` are scikit-learn's parameters (validated by its rules: non-negative
/// numbers); `max_iter` is passed to SciPy's `cg` in the normal-equation branch only, as
/// scikit-learn does.
pub fn ridge_sparse_cg<O: Operator>(
    x: &O,
    y: &[f32],
    alpha: f64,
    tol: f64,
    max_iter: Option<usize>,
    n_threads: usize,
) -> Result<RidgeFit, SdcError> {
    let [n_samples, n_features] = x.shape();
    if y.len() != n_samples {
        return Err(SdcError::Invalid(format!(
            "Found input variables with inconsistent numbers of samples: [{n_samples}, {}]",
            y.len()
        )));
    }
    if alpha.is_nan() || alpha < 0.0 {
        return Err(SdcError::Invalid(format!(
            "The 'alpha' parameter of Ridge must be a float in the range [0.0, inf) or an \
             array-like. Got {} instead.",
            npy::py_repr(alpha)
        )));
    }
    if tol.is_nan() || tol < 0.0 {
        return Err(SdcError::Invalid(format!(
            "The 'tol' parameter of Ridge must be a float in the range [0.0, inf). Got {} \
             instead.",
            npy::py_repr(tol)
        )));
    }
    let a32 = alpha as f32;
    let regularised = |t: Vec<f32>, v: &[f32]| -> Vec<f32> {
        t.into_iter()
            .zip(v)
            .map(|(ti, &vi)| ti + a32 * vi)
            .collect()
    };
    if n_features > n_samples {
        let res = cg_f32(
            |v| {
                Ok(regularised(
                    x.matvec(&x.rmatvec(v, n_threads)?, n_threads)?,
                    v,
                ))
            },
            y,
            tol,
            0.0,
            None,
        )?;
        Ok(RidgeFit {
            coef: x.rmatvec(&res.x, n_threads)?,
            info: res.info,
            iterations: res.iterations,
            system: RidgeSystem::Kernel,
        })
    } else {
        let b = x.rmatvec(y, n_threads)?;
        let res = cg_f32(
            |v| {
                Ok(regularised(
                    x.rmatvec(&x.matvec(v, n_threads)?, n_threads)?,
                    v,
                ))
            },
            &b,
            tol,
            0.0,
            max_iter,
        )?;
        Ok(RidgeFit {
            coef: res.x,
            info: res.info,
            iterations: res.iterations,
            system: RidgeSystem::Normal,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csr(rows: &[&[(usize, f32)]], cols: usize) -> CsrOperator {
        let mut indptr = vec![0];
        let (mut indices, mut data) = (Vec::new(), Vec::new());
        for r in rows {
            for &(c, v) in *r {
                indices.push(c);
                data.push(v);
            }
            indptr.push(indices.len());
        }
        CsrOperator::new(Csr {
            shape: [rows.len(), cols],
            indptr,
            indices,
            data,
        })
        .unwrap()
    }

    #[test]
    fn the_transpose_keeps_row_order() {
        let m = csr(&[&[(1, 2.0), (0, 1.0)], &[(1, 3.0)], &[(0, 4.0)]], 2);
        assert_eq!(m.matvec(&[1.0, 10.0], 2).unwrap(), vec![21.0, 30.0, 4.0]);
        assert_eq!(m.rmatvec(&[1.0, 1.0, 1.0], 2).unwrap(), vec![5.0, 5.0]);
        assert!(
            CsrOperator::new(Csr {
                shape: [1, 1],
                indptr: vec![0, 1],
                indices: vec![1],
                data: vec![1.0],
            })
            .is_err()
        );
    }

    #[test]
    fn identity_systems_converge_in_one_step() {
        let m = csr(&[&[(0, 1.0)], &[(1, 1.0)], &[(2, 1.0)]], 3);
        let fit = ridge_sparse_cg(&m, &[1.0, 2.0, 3.0], 0.0, 1e-4, None, 1).unwrap();
        assert_eq!(fit.coef, vec![1.0, 2.0, 3.0]);
        assert_eq!(
            (fit.info, fit.iterations, fit.system),
            (0, 1, RidgeSystem::Normal)
        );
        // More columns than rows: the kernel system.
        let wide = csr(&[&[(0, 1.0)], &[(1, 1.0)]], 3);
        let fit = ridge_sparse_cg(&wide, &[1.0, 2.0], 0.0, 1e-4, None, 1).unwrap();
        assert_eq!(fit.coef, vec![1.0, 2.0, 0.0]);
        assert_eq!(fit.system, RidgeSystem::Kernel);
    }

    #[test]
    fn scipy_edge_cases() {
        // norm(b) == 0 returns b; maxiter = 0 returns zeros with info 0.
        let id = |v: &[f32]| Ok(v.to_vec());
        let r = cg_f32(id, &[0.0, -0.0], 1e-4, 0.0, None).unwrap();
        assert_eq!(r.x[1].to_bits(), (-0.0f32).to_bits());
        let r = cg_f32(id, &[1.0, 2.0], 1e-4, 0.0, Some(0)).unwrap();
        assert_eq!((r.x, r.info), (vec![0.0, 0.0], 0));
        let e = cg_f32(id, &[1.0], 1e-4, -1e-5, None).unwrap_err();
        assert!(e.to_string().contains("`atol`=-1e-05;"), "{e}");
    }
}
