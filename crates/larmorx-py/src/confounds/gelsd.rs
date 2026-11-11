// SPDX-License-Identifier: Apache-2.0
//! Bindings of LAPACK's minimum-norm least squares (`DGELSD`, its bidiagonal solver `DLALSD` and
//! `DLASRT`) at their Fortran interface (`lapack_*`, as
//! `larmorx_transform::numpy_linalg::gelsd` computes them) and numpy's `lstsq(a, b, rcond)` on top
//! of them (`numpy_lstsq`); the validation compares them with numpy's own library and function.
//! Matrices cross as flat column-major storage with their leading dimension (the `lapack_*`
//! functions) or as arrays (`numpy_lstsq`). The GIL is released while computing.

use super::svd::{F64s, c_view, linalg_error, lp, uplo};
use super::{contiguous, lapack_run, one_of};
use larmorx_transform::numpy_linalg::LstsqError;
use larmorx_transform::numpy_linalg::gelsd::{self, DgelsdError, Sort};
use larmorx_transform::openblas::matmul::Array2 as View;
use numpy::{PyArray1, PyArrayDyn, PyArrayMethods, PyReadonlyArray1, PyReadonlyArray2};
use numpy::{PyReadonlyArrayDyn, PyUntypedArrayMethods};
use pyo3::exceptions::{PyNotImplementedError, PyValueError};
use pyo3::prelude::*;

fn gelsd_err(e: DgelsdError) -> PyErr {
    match e {
        DgelsdError::LargePathNotPorted { .. } => PyNotImplementedError::new_err(e.to_string()),
        other => PyValueError::new_err(other.to_string()),
    }
}

/// `DLASRT(ID, N, D, INFO)`: `d` sorted increasing (`id = "I"`) or decreasing (`"D"`).
#[pyfunction]
fn lapack_dlasrt<'py>(
    py: Python<'py>,
    id: &str,
    d: PyReadonlyArray1<'_, f64>,
) -> PyResult<F64s<'py>> {
    let order = one_of(
        "id",
        id,
        &[("I", Sort::Increasing), ("D", Sort::Decreasing)],
    )?;
    let mut dv = contiguous(&d)?.to_vec();
    gelsd::dlasrt(order, &mut dv);
    Ok(PyArray1::from_vec(py, dv))
}

/// `DLALSD(UPLO, SMLSIZ, N, NRHS, D, E, B, LDB, RCOND, RANK, ...)`: the minimum-norm solution of an
/// `n × n` bidiagonal system (`d`: `n` values, `e`: `n - 1`) for the `nrhs` columns of `b` (flat
/// column-major, `ldb × nrhs`). Returns `(D, E, B, RANK, INFO)`. Only `N <= SMLSIZ` is ported
/// (`NotImplementedError` otherwise).
#[pyfunction]
#[pyo3(signature = (uplo_, smlsiz, n, nrhs, d, e, b, ldb, rcond, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dlalsd<'py>(
    py: Python<'py>,
    uplo_: &str,
    smlsiz: usize,
    n: usize,
    nrhs: usize,
    d: PyReadonlyArray1<'_, f64>,
    e: PyReadonlyArray1<'_, f64>,
    b: PyReadonlyArray1<'_, f64>,
    ldb: usize,
    rcond: f64,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<(F64s<'py>, F64s<'py>, F64s<'py>, usize, i64)> {
    let lp = lp(lapack, gemm_q)?;
    let up = uplo(uplo_)?;
    let mut dv = contiguous(&d)?.to_vec();
    let mut ev = contiguous(&e)?.to_vec();
    let mut bv = contiguous(&b)?.to_vec();
    if dv.len() < n || ev.len() + 1 < n || (nrhs > 0 && bv.len() < ldb * (nrhs - 1) + n) {
        return Err(PyValueError::new_err(
            "DLALSD: arrays too short for N, NRHS and LDB",
        ));
    }
    let (rank, info) = lapack_run(py, "DLALSD", n_threads, || {
        gelsd::dlalsd(
            lp, up, smlsiz, n, nrhs, &mut dv, &mut ev, &mut bv, 0, ldb, rcond,
        )
    })?
    .map_err(gelsd_err)?;
    Ok((
        PyArray1::from_vec(py, dv),
        PyArray1::from_vec(py, ev),
        PyArray1::from_vec(py, bv),
        rank,
        info,
    ))
}

/// `DGELSD(M, N, NRHS, A, LDA, B, LDB, S, RCOND, RANK, WORK, LWORK, IWORK, INFO)` on flat
/// column-major storage (`a`: `lda × n`, `b`: `ldb × nrhs`); `lwork=None` is the optimal workspace
/// (numpy's query). Returns `(A, B, S, RANK, WORK(1), IWORK(1), INFO)`. A non-finite `A` is refused
/// (`ValueError`: LAPACK scales an infinity to NaN and may loop forever); `min(m, n) > 25` raises
/// `NotImplementedError` (`DLALSD`'s divide-and-conquer path, not ported yet).
#[pyfunction]
#[pyo3(signature = (m, n, nrhs, a, lda, b, ldb, rcond, lwork = None, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dgelsd<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    nrhs: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    b: PyReadonlyArray1<'_, f64>,
    ldb: usize,
    rcond: f64,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<(F64s<'py>, F64s<'py>, F64s<'py>, usize, usize, usize, i64)> {
    let lp = lp(lapack, gemm_q)?;
    let mut av = contiguous(&a)?.to_vec();
    let mut bv = contiguous(&b)?.to_vec();
    if (n > 0 && av.len() < lda * (n - 1) + m)
        || (nrhs > 0 && bv.len() < ldb * (nrhs - 1) + m.max(n))
    {
        return Err(PyValueError::new_err(
            "DGELSD: arrays too short for M, N, NRHS, LDA and LDB",
        ));
    }
    let mut s = vec![0.0f64; m.min(n)];
    let out = lapack_run(py, "DGELSD", n_threads, || {
        gelsd::dgelsd(
            lp, m, n, nrhs, &mut av, lda, &mut bv, ldb, &mut s, rcond, lwork,
        )
    })?
    .map_err(gelsd_err)?;
    Ok((
        PyArray1::from_vec(py, av),
        PyArray1::from_vec(py, bv),
        PyArray1::from_vec(py, s),
        out.rank,
        out.work1,
        out.iwork1,
        out.info,
    ))
}

/// `DGELSD(M, N, NRHS, ...)`'s workspace: `(MINWRK, MAXWRK, LIWORK)`, the smallest `LWORK` it
/// accepts, the optimal one (`WORK(1)` of the `LWORK = -1` query) and `IWORK(1)`.
#[pyfunction]
fn lapack_dgelsd_lwork(m: usize, n: usize, nrhs: usize) -> (usize, usize, usize) {
    gelsd::dgelsd_lwork(m, n, nrhs)
}

type Dyn<'py> = Bound<'py, PyArrayDyn<f64>>;

fn dyn_vec<'py>(py: Python<'py>, shape: &[usize], v: Vec<f64>) -> PyResult<Dyn<'py>> {
    PyArray1::from_vec(py, v).reshape(shape.to_vec())
}

/// `numpy.linalg.lstsq(a, b, rcond)` of a 2-D float64 `a` and a 1-D or 2-D float64 `b` (any
/// strides), with numpy 2.5.4's LAPACK (`lapack="3.12.1"`) or numpy 2.3.5's (`"3.12.0"`):
/// `(x, residuals, rank, s)` with numpy's shapes (`x` and the residuals 1-D for a 1-D `b`; the
/// residuals empty unless the rank is `n` and `m > n`). `rcond=None` is numpy's default. Raises
/// numpy's `LinAlgError` as numpy does (`Incompatible dimensions`; `SVD did not converge in Linear
/// Least Squares`: `DGELSD`'s `INFO != 0` or a non-finite `a`); `NotImplementedError` where
/// `min(m, n) > 25` (`DLALSD`'s divide-and-conquer path, not ported yet).
#[pyfunction]
#[pyo3(signature = (a, b, rcond = None, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
fn numpy_lstsq<'py>(
    py: Python<'py>,
    a: PyReadonlyArray2<'_, f64>,
    b: PyReadonlyArrayDyn<'_, f64>,
    rcond: Option<f64>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<(Dyn<'py>, F64s<'py>, usize, F64s<'py>)> {
    let lp = lp(lapack, gemm_q)?;
    let av = a.as_array();
    let (m, n) = av.dim();
    let adata: Vec<f64> = av.iter().copied().collect();
    let is_1d = match b.ndim() {
        1 => true,
        2 => false,
        d => {
            return Err(linalg_error(
                py,
                &format!("{d}-dimensional array given. Array must be two-dimensional"),
            ));
        }
    };
    let bv = b.as_array();
    let (bm, nrhs) = if is_1d {
        (bv.shape()[0], 1)
    } else {
        (bv.shape()[0], bv.shape()[1])
    };
    let bdata: Vec<f64> = bv.iter().copied().collect();
    let res = lapack_run(py, "numpy.linalg.lstsq", n_threads, || {
        let bview = View {
            data: &bdata,
            off: 0,
            rows: bm,
            cols: nrhs,
            s0: nrhs.max(1),
            s1: 1,
        };
        gelsd::numpy_lstsq(lp, c_view(&adata, m, n), bview, rcond)
    })?;
    let out = match res {
        Ok(out) => out,
        Err(e @ LstsqError::Unsupported { .. }) => {
            return Err(PyNotImplementedError::new_err(format!(
                "{e} (DLALSD's divide-and-conquer path)"
            )));
        }
        Err(e) => return Err(linalg_error(py, &e.to_string())),
    };
    let shape: &[usize] = if is_1d { &[n] } else { &[n, nrhs] };
    let x = dyn_vec(py, shape, out.x)?;
    Ok((
        x,
        PyArray1::from_vec(py, out.residuals),
        out.rank,
        PyArray1::from_vec(py, out.s),
    ))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(lapack_dlasrt, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlalsd, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dgelsd, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dgelsd_lwork, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_lstsq, m)?)?;
    Ok(())
}
