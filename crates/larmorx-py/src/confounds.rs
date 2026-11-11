// SPDX-License-Identifier: Apache-2.0
//! Bindings of the confounds' part (fMRIPrep's confound estimation and the numerics under it).
//! Today: OpenBLAS's level-2 and level-3 kernels at the BLAS interface (`blas_dgemm`,
//! `blas_dgemv`) and numpy's `matmul` of two 2-D float64 arrays (`numpy_matmul`), as
//! `larmorx_transform::openblas` computes them; the validation compares them with numpy's own
//! library. Matrices cross as flat column-major storage with their leading dimension, as BLAS
//! takes them. The GIL is released while computing.

use larmorx_core::parallel;
use larmorx_transform::openblas::level2;
use larmorx_transform::openblas::level3::{self, Operand, Trans};
use larmorx_transform::openblas::matmul::{self, Array2 as View};
use numpy::ndarray::Array2;
use numpy::{PyArray, PyArray1, PyArray2, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

fn value_err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

fn trans(t: &str) -> PyResult<Trans> {
    match t {
        "N" | "n" => Ok(Trans::N),
        "T" | "t" | "C" | "c" => Ok(Trans::T),
        _ => Err(PyValueError::new_err(format!(
            "trans must be 'N' or 'T', got {t:?}"
        ))),
    }
}

fn contiguous<'a>(x: &'a PyReadonlyArray1<'_, f64>) -> PyResult<&'a [f64]> {
    x.as_slice().map_err(value_err)
}

/// `DGEMM(TRANSA, TRANSB, M, N, K, ALPHA, A, LDA, B, LDB, BETA, C, LDC)` on flat column-major
/// storage (`larmorx_transform::openblas::level3::dgemm_with`; `gemm_q` 256, or 512 to follow
/// numpy 2.5.4 forced onto the Haswell kernels on an AMD Zen 4); returns the new `C` storage.
#[pyfunction]
#[pyo3(signature = (transa, transb, m, n, k, alpha, a, lda, b, ldb, beta, c, ldc, n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn blas_dgemm<'py>(
    py: Python<'py>,
    transa: &str,
    transb: &str,
    m: usize,
    n: usize,
    k: usize,
    alpha: f64,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    b: PyReadonlyArray1<'_, f64>,
    ldb: usize,
    beta: f64,
    c: PyReadonlyArray1<'_, f64>,
    ldc: usize,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let (ta, tb) = (trans(transa)?, trans(transb)?);
    let (a, b) = (contiguous(&a)?, contiguous(&b)?);
    let mut cv = contiguous(&c)?.to_vec();
    let opa = Operand {
        data: a,
        off: 0,
        ld: lda,
        trans: ta,
    };
    let opb = Operand {
        data: b,
        off: 0,
        ld: ldb,
        trans: tb,
    };
    py.detach(|| {
        parallel::with_threads(n_threads, || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                level3::dgemm_with(m, n, k, alpha, opa, opb, beta, &mut cv, 0, ldc, gemm_q)
            }))
        })
    })
    .map_err(value_err)?
    .map_err(|_| PyValueError::new_err("DGEMM: invalid arguments"))?;
    Ok(PyArray1::from_vec(py, cv))
}

/// `DGEMV(TRANS, M, N, ALPHA, A, LDA, X, INCX, BETA, Y, INCY)` on flat storage
/// (`larmorx_transform::openblas::level2::dgemv`, positive increments); returns the new `Y`.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn blas_dgemv<'py>(
    py: Python<'py>,
    trans_: &str,
    m: usize,
    n: usize,
    alpha: f64,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    x: PyReadonlyArray1<'_, f64>,
    incx: usize,
    beta: f64,
    y: PyReadonlyArray1<'_, f64>,
    incy: usize,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let t = trans(trans_)?;
    let (a, x) = (contiguous(&a)?, contiguous(&x)?);
    let mut yv = contiguous(&y)?.to_vec();
    py.detach(|| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            level2::dgemv(
                t, m, n, alpha, a, 0, lda, x, 0, incx, beta, &mut yv, 0, incy,
            )
        }))
    })
    .map_err(|_| PyValueError::new_err("DGEMV: invalid arguments"))?;
    Ok(PyArray1::from_vec(py, yv))
}

/// A view of a C- or F-contiguous 2-D array as `matmul` takes it (strides in elements).
fn view<'a>(x: &'a PyReadonlyArray2<'_, f64>) -> PyResult<View<'a>> {
    let arr = x.as_array();
    let (rows, cols) = arr.dim();
    let st = arr.strides();
    let data = x
        .as_slice()
        .or_else(|_| arr.to_slice_memory_order().ok_or(()))
        .map_err(|_| PyValueError::new_err("numpy_matmul takes C- or F-contiguous arrays"))?;
    let stride = |s: isize, d: usize| if d <= 1 { 1 } else { s as usize };
    if st.iter().any(|&s| s < 0) {
        return Err(PyValueError::new_err("numpy_matmul takes positive strides"));
    }
    // numpy's strides of a length-1 axis may be anything; give them the contiguous values.
    let (s0, s1) = if arr.is_standard_layout() {
        (cols.max(1), 1)
    } else {
        (stride(st[0], rows), stride(st[1], cols))
    };
    Ok(View {
        data,
        off: 0,
        rows,
        cols,
        s0,
        s1,
    })
}

/// `a @ b` of two 2-D float64 arrays (C- or F-contiguous) as numpy dispatches it onto
/// OpenBLAS's Haswell kernels (`larmorx_transform::openblas::matmul`); `dsyrk` when `a` and `b`
/// are one buffer as `x` and `x.T`. Returns a C-ordered array.
#[pyfunction]
#[pyo3(signature = (a, b, n_threads = 1, gemm_q = 256))]
fn numpy_matmul<'py>(
    py: Python<'py>,
    a: PyReadonlyArray2<'_, f64>,
    b: PyReadonlyArray2<'_, f64>,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Bound<'py, PyArray2<f64>>> {
    let same = a.as_array().as_ptr() == b.as_array().as_ptr();
    let (va, vb) = (view(&a)?, view(&b)?);
    if va.cols != vb.rows {
        return Err(PyValueError::new_err("matmul: inner dimensions differ"));
    }
    let (m, p) = (va.rows, vb.cols);
    let out = py
        .detach(|| {
            parallel::with_threads(n_threads, || match matmul::syrk_case(va, vb, same) {
                Some(t) => matmul::numpy_syrk_with(va, t, gemm_q),
                None => matmul::numpy_matmul_with(va, vb, gemm_q),
            })
        })
        .map_err(value_err)?;
    Ok(PyArray::from_owned_array(
        py,
        Array2::from_shape_vec((m, p), out).map_err(value_err)?,
    ))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(blas_dgemm, m)?)?;
    m.add_function(wrap_pyfunction!(blas_dgemv, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_matmul, m)?)?;
    Ok(())
}
