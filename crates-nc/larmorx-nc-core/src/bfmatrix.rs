// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-miscmaths 2412.6: bfmatrix.h, bfmatrix.cpp). FSL is
// Copyright (C) 2007 University of Oxford. Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! fsl-miscmaths' `BFMatrix`: the matrix type of the non-linear optimisers' Hessians, either
//! dense (`FullBFMatrix`, a NEWMAT matrix) or sparse (`SparseBFMatrix<float>` or `<double>`,
//! an [`SpMat`]). An operation between two kinds converts the way FSL's `dynamic_cast` chains
//! do; a pair with no conversion (a `float` and a `double` sparse matrix) is FSL's "dynamic
//! cast error". Indices are 1-based, as FSL's.

use crate::arma::{self, Mat};
use crate::spmat::{MatrixType, SpMat};
use crate::volume::FslError;

/// `BFMatrixException(msg)`: `what()` is `"BFMatrix::" + msg`.
pub fn bfmatrix_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "MISCMATHS::BFMatrixException",
        what: format!("BFMatrix::{msg}"),
    }
}

/// `BFMatrixPrecisionType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    Double,
    Float,
}

/// A `BFMatrix`.
#[derive(Clone, Debug)]
pub enum BFMatrix {
    Full(Mat),
    Float(SpMat<f32>),
    Double(SpMat<f64>),
}

/// NEWMAT's element access out of range (`Mat::operator()` with bounds checks).
fn index_error() -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "std::logic_error",
        what: "Mat::operator(): index out of bounds".to_owned(),
    }
}

/// `A | B` of two dense matrices (Armadillo's `insert_cols` at the end).
pub fn hcat(a: &Mat, b: &Mat) -> Mat {
    if a.is_empty() && a.rows() == 0 {
        return b.clone();
    }
    let mut v = a.as_slice().to_vec();
    v.extend_from_slice(b.as_slice());
    Mat::from_col_major(a.rows(), a.cols() + b.cols(), &v)
}

/// `A & B` of two dense matrices (Armadillo's `insert_rows` at the end).
pub fn vcat(a: &Mat, b: &Mat) -> Mat {
    if a.is_empty() && a.cols() == 0 {
        return b.clone();
    }
    let (ra, rb, n) = (a.rows(), b.rows(), a.cols());
    let mut out = Mat::zeros(ra + rb, n);
    for c in 0..n {
        for r in 0..ra {
            out.set(r, c, a.at(r, c));
        }
        for r in 0..rb {
            out.set(ra + r, c, b.at(r, c));
        }
    }
    out
}

impl BFMatrix {
    /// `SparseBFMatrix<T>(m, n, irp, jcp, sp, nt)` of the requested precision.
    pub fn sparse_from_csc(
        c: &crate::basisfield::Csc,
        prec: Precision,
        nt: u32,
    ) -> Result<BFMatrix, FslError> {
        Ok(match prec {
            Precision::Float => BFMatrix::Float(SpMat::from_csc_struct(c, nt)?),
            Precision::Double => BFMatrix::Double(SpMat::from_csc_struct(c, nt)?),
        })
    }

    pub fn nrows(&self) -> u32 {
        match self {
            BFMatrix::Full(m) => m.rows() as u32,
            BFMatrix::Float(s) => s.nrows(),
            BFMatrix::Double(s) => s.nrows(),
        }
    }
    pub fn ncols(&self) -> u32 {
        match self {
            BFMatrix::Full(m) => m.cols() as u32,
            BFMatrix::Float(s) => s.ncols(),
            BFMatrix::Double(s) => s.ncols(),
        }
    }
    /// `NZ()` (a dense matrix: rows × columns).
    pub fn nz(&self) -> u32 {
        match self {
            BFMatrix::Full(m) => (m.rows() * m.cols()) as u32,
            BFMatrix::Float(s) => s.nz(),
            BFMatrix::Double(s) => s.nz(),
        }
    }
    pub fn nthreads(&self) -> u32 {
        match self {
            BFMatrix::Full(_) => 1,
            BFMatrix::Float(s) => s.nthreads(),
            BFMatrix::Double(s) => s.nthreads(),
        }
    }
    pub fn set_nthreads(&mut self, nt: u32) {
        match self {
            BFMatrix::Full(_) => {}
            BFMatrix::Float(s) => s.set_nthreads(nt),
            BFMatrix::Double(s) => s.set_nthreads(nt),
        }
    }

    /// `AsMatrix()`.
    pub fn as_matrix(&self) -> Mat {
        match self {
            BFMatrix::Full(m) => m.clone(),
            BFMatrix::Float(s) => s.as_dense(),
            BFMatrix::Double(s) => s.as_dense(),
        }
    }

    /// `Peek(r, c)` (a `float` element widened).
    pub fn peek(&self, r: u32, c: u32) -> Result<f64, FslError> {
        match self {
            BFMatrix::Full(m) => {
                if r < 1 || c < 1 || r as usize > m.rows() || c as usize > m.cols() {
                    return Err(index_error());
                }
                Ok(m.at(r as usize - 1, c as usize - 1))
            }
            BFMatrix::Float(s) => Ok(f64::from(s.peek(r, c)?)),
            BFMatrix::Double(s) => s.peek(r, c),
        }
    }

    /// `Set(r, c, v)` (and `Insert`): a sparse matrix stores `(T)v`.
    pub fn set(&mut self, r: u32, c: u32, v: f64) -> Result<(), FslError> {
        match self {
            BFMatrix::Full(m) => {
                if r < 1 || c < 1 || r as usize > m.rows() || c as usize > m.cols() {
                    return Err(index_error());
                }
                m.set(r as usize - 1, c as usize - 1, v);
                Ok(())
            }
            BFMatrix::Float(s) => s.set(r, c, v as f32),
            BFMatrix::Double(s) => s.set(r, c, v),
        }
    }

    /// `AddTo(r, c, v)`: a sparse `float` matrix adds `(float)v` in `float`.
    pub fn add_to(&mut self, r: u32, c: u32, v: f64) -> Result<(), FslError> {
        match self {
            BFMatrix::Full(m) => {
                if r < 1 || c < 1 || r as usize > m.rows() || c as usize > m.cols() {
                    return Err(index_error());
                }
                let (ri, ci) = (r as usize - 1, c as usize - 1);
                m.set(ri, ci, m.at(ri, ci) + v);
                Ok(())
            }
            BFMatrix::Float(s) => s.add_to(r, c, v as f32),
            BFMatrix::Double(s) => s.add_to(r, c, v),
        }
    }

    /// `SubMatrix(fr, lr, fc, lc)`.
    pub fn sub_matrix(&self, fr: u32, lr: u32, fc: u32, lc: u32) -> Result<Mat, FslError> {
        if fr < 1 || fc < 1 || lr > self.nrows() || lc > self.ncols() || fr > lr || fc > lc {
            return Err(bfmatrix_error("BFMatrix::SubMatrix: index out of range"));
        }
        let mut o = Mat::zeros((lr - fr + 1) as usize, (lc - fc + 1) as usize);
        for r in fr..=lr {
            for c in fc..=lc {
                o.set((r - fr) as usize, (c - fc) as usize, self.peek(r, c)?);
            }
        }
        Ok(o)
    }

    /// `Transpose()` (a sparse transpose takes FSL's default thread count `nt`).
    pub fn transpose(&self, nt: u32) -> BFMatrix {
        match self {
            BFMatrix::Full(m) => BFMatrix::Full(m.t()),
            BFMatrix::Float(s) => BFMatrix::Float(s.t(nt)),
            BFMatrix::Double(s) => BFMatrix::Double(s.t(nt)),
        }
    }

    /// `MulMeByScalar(s)`.
    pub fn mul_me_by_scalar(&mut self, s: f64) {
        match self {
            BFMatrix::Full(m) => {
                for v in m.as_mut_slice() {
                    *v *= s;
                }
            }
            BFMatrix::Float(sp) => sp.scale(s),
            BFMatrix::Double(sp) => sp.scale(s),
        }
    }

    /// `MulByVec(v)`.
    pub fn mul_by_vec(&self, v: &[f64]) -> Result<Vec<f64>, FslError> {
        match self {
            BFMatrix::Full(m) => {
                if v.len() != m.cols() {
                    return Err(bfmatrix_error(
                        "FullBFMatrix::MulByVec: Matrix-vector size mismatch",
                    ));
                }
                Ok(arma::mul(m, &Mat::col(v), false).as_slice().to_vec())
            }
            BFMatrix::Float(s) => {
                if v.len() != s.ncols() as usize {
                    return Err(bfmatrix_error("Matrix-vector size mismatch"));
                }
                s.mul_vec(v)
            }
            BFMatrix::Double(s) => {
                if v.len() != s.ncols() as usize {
                    return Err(bfmatrix_error("Matrix-vector size mismatch"));
                }
                s.mul_vec(v)
            }
        }
    }

    /// `AddToMe(M, s)`. Sparse `+= s·M`: a scaled copy of `M` (`T *= s`, unless `s` is 1)
    /// added with `+=`; a dense `M` into a sparse matrix is made sparse first (FSL's default
    /// thread count `nt`, which does not matter here). Dense `+= s·M` element by element.
    pub fn add_to_me(&mut self, other: &BFMatrix, s: f64, nt: u32) -> Result<(), FslError> {
        if self.ncols() != other.ncols() || self.nrows() != other.nrows() {
            return Err(bfmatrix_error(match self {
                BFMatrix::Full(_) => "FullBFMatrix::AddToMe: Matrix size mismatch",
                _ => "SparseBFMatrix::AddToMe: Matrix size mismatch",
            }));
        }
        match (self, other) {
            (BFMatrix::Full(m), o) => {
                let om = match o {
                    BFMatrix::Full(x) => x.clone(),
                    BFMatrix::Float(x) => x.as_dense(),
                    BFMatrix::Double(x) => x.as_dense(),
                };
                for (a, &b) in m.as_mut_slice().iter_mut().zip(om.as_slice()) {
                    *a += b * s;
                }
                Ok(())
            }
            (BFMatrix::Float(m), BFMatrix::Float(o)) => add_sparse(m, o, s),
            (BFMatrix::Double(m), BFMatrix::Double(o)) => add_sparse(m, o, s),
            (BFMatrix::Float(m), BFMatrix::Full(o)) => add_sparse(m, &SpMat::from_dense(o, nt), s),
            (BFMatrix::Double(m), BFMatrix::Full(o)) => add_sparse(m, &SpMat::from_dense(o, nt), s),
            _ => Err(bfmatrix_error(
                "SparseBFMatrix::AddToMe: dynamic cast error",
            )),
        }
    }

    /// `HorConcat2MyRight(B)`: nothing for a `B` with no rows.
    pub fn hor_concat2_my_right(&mut self, b: &BFMatrix, nt: u32) -> Result<(), FslError> {
        if b.nrows() == 0 {
            return Ok(());
        }
        match self {
            BFMatrix::Full(m) => {
                if m.rows() as u32 != b.nrows() {
                    return Err(bfmatrix_error(
                        "FullBFMatrix::HorConcat2MyRight: Matrices must have same # of rows",
                    ));
                }
                *m = hcat(m, &b.as_matrix());
                Ok(())
            }
            BFMatrix::Float(sp) => {
                if sp.nrows() != b.nrows() {
                    return Err(bfmatrix_error(
                        "SparseBFMatrix::HorConcat2MyRight: Matrices must have same # of rows",
                    ));
                }
                match b {
                    BFMatrix::Float(o) => sp.hor_concat(o),
                    BFMatrix::Full(o) => sp.hor_concat(&SpMat::from_dense(o, nt)),
                    _ => Err(bfmatrix_error(
                        "SparseBFMatrix::HorConcat2MyRight: dynamic cast error",
                    )),
                }
            }
            BFMatrix::Double(sp) => {
                if sp.nrows() != b.nrows() {
                    return Err(bfmatrix_error(
                        "SparseBFMatrix::HorConcat2MyRight: Matrices must have same # of rows",
                    ));
                }
                match b {
                    BFMatrix::Double(o) => sp.hor_concat(o),
                    BFMatrix::Full(o) => sp.hor_concat(&SpMat::from_dense(o, nt)),
                    _ => Err(bfmatrix_error(
                        "SparseBFMatrix::HorConcat2MyRight: dynamic cast error",
                    )),
                }
            }
        }
    }

    /// `VertConcatBelowMe(B)`: nothing for a `B` with no columns.
    pub fn vert_concat_below_me(&mut self, b: &BFMatrix, nt: u32) -> Result<(), FslError> {
        if b.ncols() == 0 {
            return Ok(());
        }
        match self {
            BFMatrix::Full(m) => {
                if m.cols() as u32 != b.ncols() {
                    return Err(bfmatrix_error(
                        "FullBFMatrix::VertConcatBelowMe: Matrices must have same # of columns",
                    ));
                }
                *m = vcat(m, &b.as_matrix());
                Ok(())
            }
            BFMatrix::Float(sp) => {
                if sp.ncols() != b.ncols() {
                    return Err(bfmatrix_error(
                        "SparseBFMatrix::VertConcatBelowMe: Matrices must have same # of columns",
                    ));
                }
                match b {
                    BFMatrix::Float(o) => sp.vert_concat(o),
                    BFMatrix::Full(o) => sp.vert_concat(&SpMat::from_dense(o, nt)),
                    _ => Err(bfmatrix_error(
                        "SparseBFMatrix::VertConcatBelowMe: dynamic cast error",
                    )),
                }
            }
            BFMatrix::Double(sp) => {
                if sp.ncols() != b.ncols() {
                    return Err(bfmatrix_error(
                        "SparseBFMatrix::VertConcatBelowMe: Matrices must have same # of columns",
                    ));
                }
                match b {
                    BFMatrix::Double(o) => sp.vert_concat(o),
                    BFMatrix::Full(o) => sp.vert_concat(&SpMat::from_dense(o, nt)),
                    _ => Err(bfmatrix_error(
                        "SparseBFMatrix::VertConcatBelowMe: dynamic cast error",
                    )),
                }
            }
        }
    }

    /// `HorConcat2MyRight(const NEWMAT::Matrix&)`.
    pub fn hor_concat2_my_right_mat(&mut self, b: &Mat, nt: u32) -> Result<(), FslError> {
        if b.rows() == 0 {
            return Ok(());
        }
        self.hor_concat2_my_right(&BFMatrix::Full(b.clone()), nt)
    }

    /// `VertConcatBelowMe(const NEWMAT::Matrix&)`.
    pub fn vert_concat_below_me_mat(&mut self, b: &Mat, nt: u32) -> Result<(), FslError> {
        if b.cols() == 0 {
            return Ok(());
        }
        self.vert_concat_below_me(&BFMatrix::Full(b.clone()), nt)
    }

    /// `SolveForx(b, type, tol, miter)`: a sparse matrix by [`SpMat::solve_for_x`] (its
    /// warning text, if any, second); a dense one by `inv(A)*b` (Armadillo's solve; the other
    /// arguments ignored).
    pub fn solve_for_x(
        &self,
        b: &[f64],
        mtype: MatrixType,
        tol: f64,
        miter: i32,
        blas_threads: usize,
    ) -> Result<(Vec<f64>, Option<String>), FslError> {
        if b.len() != self.nrows() as usize {
            return Err(bfmatrix_error(match self {
                BFMatrix::Full(_) => "FullBFMatrix::SolveForx: Matrix-vector size mismatch",
                _ => "SparseBFMatrix::SolveForx: Matrix-vector size mismatch",
            }));
        }
        match self {
            BFMatrix::Full(m) => {
                let x = arma::solve(m, &Mat::col(b)).map_err(|_| FslError {
                    printed: Vec::new(),
                    type_name: "std::runtime_error",
                    what: "matrix multiplication: problem with matrix inverse; suggest to use solve() instead".to_owned(),
                })?;
                Ok((x.as_slice().to_vec(), None))
            }
            BFMatrix::Float(s) => s.solve_for_x(b, mtype, tol, miter as u32, None, blas_threads),
            BFMatrix::Double(s) => s.solve_for_x(b, mtype, tol, miter as u32, None, blas_threads),
        }
    }

    /// `Print(fname)`'s text for a sparse matrix (precision 8); a dense one prints NEWMAT's
    /// layout, not translated (FSL's optimisers never print).
    pub fn print_text(&self) -> Option<String> {
        match self {
            BFMatrix::Full(_) => None,
            BFMatrix::Float(s) => Some(s.print_text(8)),
            BFMatrix::Double(s) => Some(s.print_text(8)),
        }
    }
}

fn add_sparse<T: crate::spmat::SpElem>(
    m: &mut SpMat<T>,
    o: &SpMat<T>,
    s: f64,
) -> Result<(), FslError> {
    if s == 1.0 {
        m.add_assign(o)
    } else {
        m.add_assign(&o.scaled(s))
    }
}
