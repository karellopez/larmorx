// SPDX-License-Identifier: Apache-2.0
//! Bindings of the confounds' part (fMRIPrep's confound estimation and the numerics under it).
//! Today: OpenBLAS's level-1, level-2 and level-3 kernels at the BLAS interface (`blas_*`),
//! numpy's `matmul` of two 2-D float64 arrays (`numpy_matmul`), as `larmorx_transform::openblas`
//! computes them, and LAPACK's QR and LQ routines with their `Q` (`DORGQR`, `DORMQR`, ...) and
//! the bidiagonal reduction (`DGEBRD`) at their Fortran interface (`lapack_*`, as
//! `larmorx_transform::numpy_linalg::blocked` computes them), and the divide-and-conquer
//! bidiagonal SVD (`DBDSDC` and its routines, [`svd`]), numpy's reductions and order
//! statistics at the confounds' call sites ([`reductions`]), SciPy's single-precision FFTs
//! ([`fft`]) and nipype's DVARS with nitime's AR(1) under it ([`dvars`]); the validation
//! compares them with numpy's, SciPy's, nitime's and nipype's own libraries. Matrices cross as
//! flat column-major storage with their leading dimension, as BLAS takes them. The GIL is
//! released while computing.

use larmorx_core::parallel;
use larmorx_transform::numpy_linalg::blocked::{self, Direct, DlarftForm, Lapack, Storev};
use larmorx_transform::numpy_linalg::{self, LapackRelease};
use larmorx_transform::openblas::level3::{self, Operand, Trans};
use larmorx_transform::openblas::matmul::{self, Array2 as View};
use larmorx_transform::openblas::trmm::{self, Diag, Side, Uplo};
use larmorx_transform::openblas::{level1, level2};
use numpy::ndarray::Array2;
use numpy::{PyArray, PyArray1, PyArray2, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

mod compcor;
mod dvars;
mod expansion;
mod fft;
mod gelsd;
mod gesvd;
mod motion;
mod nipy;
mod pandas;
mod reductions;
mod svd;

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

fn one_of<T: Copy>(name: &str, v: &str, table: &[(&str, T)]) -> PyResult<T> {
    table
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(v))
        .map(|&(_, t)| t)
        .ok_or_else(|| {
            PyValueError::new_err(format!(
                "{name} must be one of {:?}, got {v:?}",
                table.iter().map(|x| x.0).collect::<Vec<_>>()
            ))
        })
}

/// `DTRMM(SIDE, UPLO, TRANSA, DIAG, M, N, ALPHA, A, LDA, B, LDB)` on flat column-major storage
/// (`larmorx_transform::openblas::trmm::dtrmm_with`; `gemm_q` as in `blas_dgemm`); returns the
/// new `B` storage.
#[pyfunction]
#[pyo3(signature = (side, uplo, transa, diag, m, n, alpha, a, lda, b, ldb, n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn blas_dtrmm<'py>(
    py: Python<'py>,
    side: &str,
    uplo: &str,
    transa: &str,
    diag: &str,
    m: usize,
    n: usize,
    alpha: f64,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    b: PyReadonlyArray1<'_, f64>,
    ldb: usize,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let s = one_of("side", side, &[("L", Side::Left), ("R", Side::Right)])?;
    let u = one_of("uplo", uplo, &[("U", Uplo::Upper), ("L", Uplo::Lower)])?;
    let d = one_of("diag", diag, &[("U", Diag::Unit), ("N", Diag::NonUnit)])?;
    let t = trans(transa)?;
    let a = contiguous(&a)?;
    let mut bv = contiguous(&b)?.to_vec();
    py.detach(|| {
        parallel::with_threads(n_threads, || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                trmm::dtrmm_with(s, u, t, d, m, n, alpha, a, 0, lda, &mut bv, 0, ldb, gemm_q)
            }))
        })
    })
    .map_err(value_err)?
    .map_err(|_| PyValueError::new_err("DTRMM: invalid arguments"))?;
    Ok(PyArray1::from_vec(py, bv))
}

/// Runs `f`, turning its panic (a BLAS parameter error) into `ValueError(name: invalid
/// arguments)`, with the GIL released.
fn guarded<R: Send>(py: Python<'_>, name: &str, f: impl FnOnce() -> R + Send) -> PyResult<R> {
    py.detach(|| std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)))
        .map_err(|_| PyValueError::new_err(format!("{name}: invalid arguments")))
}

/// `DGER(M, N, ALPHA, X, INCX, Y, INCY, A, LDA)`; returns the new `A` storage.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn blas_dger<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    alpha: f64,
    x: PyReadonlyArray1<'_, f64>,
    incx: isize,
    y: PyReadonlyArray1<'_, f64>,
    incy: isize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let (x, y) = (contiguous(&x)?, contiguous(&y)?);
    let mut av = contiguous(&a)?.to_vec();
    guarded(py, "DGER", || {
        level2::dger(m, n, alpha, x, 0, incx, y, 0, incy, &mut av, 0, lda)
    })?;
    Ok(PyArray1::from_vec(py, av))
}

/// `DTRMV(UPLO, TRANS, DIAG, N, A, LDA, X, INCX)`; returns the new `X` storage.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn blas_dtrmv<'py>(
    py: Python<'py>,
    uplo: &str,
    trans_: &str,
    diag: &str,
    n: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    x: PyReadonlyArray1<'_, f64>,
    incx: isize,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let u = one_of("uplo", uplo, &[("U", Uplo::Upper), ("L", Uplo::Lower)])?;
    let d = one_of("diag", diag, &[("U", Diag::Unit), ("N", Diag::NonUnit)])?;
    let t = trans(trans_)?;
    let a = contiguous(&a)?;
    let mut xv = contiguous(&x)?.to_vec();
    guarded(py, "DTRMV", || {
        level2::dtrmv(u, t, d, n, a, 0, lda, &mut xv, 0, incx)
    })?;
    Ok(PyArray1::from_vec(py, xv))
}

/// `DAXPY(N, ALPHA, X, INCX, Y, INCY)`; returns the new `Y` storage.
#[pyfunction]
fn blas_daxpy<'py>(
    py: Python<'py>,
    n: usize,
    alpha: f64,
    x: PyReadonlyArray1<'_, f64>,
    incx: isize,
    y: PyReadonlyArray1<'_, f64>,
    incy: isize,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let x = contiguous(&x)?;
    let mut yv = contiguous(&y)?.to_vec();
    guarded(py, "DAXPY", || {
        level1::daxpy(n, alpha, x, 0, incx, &mut yv, 0, incy)
    })?;
    Ok(PyArray1::from_vec(py, yv))
}

/// `DSCAL(N, ALPHA, X, INCX)`; returns the new `X` storage.
#[pyfunction]
fn blas_dscal<'py>(
    py: Python<'py>,
    n: usize,
    alpha: f64,
    x: PyReadonlyArray1<'_, f64>,
    incx: isize,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let mut xv = contiguous(&x)?.to_vec();
    guarded(py, "DSCAL", || level1::dscal(n, alpha, &mut xv, 0, incx))?;
    Ok(PyArray1::from_vec(py, xv))
}

/// `DCOPY(N, X, INCX, Y, INCY)`; returns the new `Y` storage.
#[pyfunction]
fn blas_dcopy<'py>(
    py: Python<'py>,
    n: usize,
    x: PyReadonlyArray1<'_, f64>,
    incx: isize,
    y: PyReadonlyArray1<'_, f64>,
    incy: isize,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let x = contiguous(&x)?;
    let mut yv = contiguous(&y)?.to_vec();
    guarded(py, "DCOPY", || {
        level1::dcopy(n, x, 0, incx, &mut yv, 0, incy)
    })?;
    Ok(PyArray1::from_vec(py, yv))
}

/// `DSWAP(N, X, INCX, Y, INCY)`; returns the new `(X, Y)` storage.
#[pyfunction]
#[allow(clippy::type_complexity)]
fn blas_dswap<'py>(
    py: Python<'py>,
    n: usize,
    x: PyReadonlyArray1<'_, f64>,
    incx: isize,
    y: PyReadonlyArray1<'_, f64>,
    incy: isize,
) -> PyResult<(Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>)> {
    let mut xv = contiguous(&x)?.to_vec();
    let mut yv = contiguous(&y)?.to_vec();
    guarded(py, "DSWAP", || {
        level1::dswap(n, &mut xv, 0, incx, &mut yv, 0, incy)
    })?;
    Ok((PyArray1::from_vec(py, xv), PyArray1::from_vec(py, yv)))
}

/// `IDAMAX(N, X, INCX)` (1-based; 0 for `n = 0` or `incx <= 0`); `aligned`: whether `x(1)` is
/// on a 16-byte boundary in the original's memory (only NaN input can tell).
#[pyfunction]
fn blas_idamax(
    py: Python<'_>,
    n: usize,
    x: PyReadonlyArray1<'_, f64>,
    incx: isize,
    aligned: bool,
) -> PyResult<usize> {
    let x = contiguous(&x)?;
    guarded(py, "IDAMAX", || level1::idamax(n, x, 0, incx, aligned))
}

/// `DNRM2(N, X, INCX)`.
#[pyfunction]
fn blas_dnrm2(
    py: Python<'_>,
    n: usize,
    x: PyReadonlyArray1<'_, f64>,
    incx: isize,
) -> PyResult<f64> {
    let x = contiguous(&x)?;
    guarded(py, "DNRM2", || level1::dnrm2(n, x, 0, incx))
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

/// The LAPACK release a `lapack_*` call follows ("3.12.1": numpy 2.5.4's OpenBLAS; "3.12.0":
/// numpy 2.3.5's OpenBLAS 0.3.30).
/// The `lapack=` keyword's library: `"3.12.1"` (OpenBLAS 0.3.34's LAPACK: numpy 2.5.4),
/// `"3.12.0"` (OpenBLAS 0.3.30's: numpy 2.3.5; also SciPy 1.18.1's 0.3.31 snapshot) or
/// `"3.12.0-openblas-0.3.28"` (SciPy 1.15.3's OpenBLAS 0.3.28: LAPACK 3.12.0 with the
/// column-by-column `DLARFT`).
const OPENBLAS_0_3_28: &str = "3.12.0-openblas-0.3.28";

fn lapack_release(name: &str) -> PyResult<LapackRelease> {
    match name {
        "3.12.1" => Ok(LapackRelease::V3_12_1),
        "3.12.0" | OPENBLAS_0_3_28 => Ok(LapackRelease::V3_12_0),
        other => Err(PyValueError::new_err(format!(
            "lapack must be '3.12.1', '3.12.0' or '{OPENBLAS_0_3_28}', not {other:?}"
        ))),
    }
}

/// The blocked routines' settings for the `lapack=` keyword ([`lapack_release`]) and the BLAS
/// depth block `gemm_q`.
fn lapack_settings(name: &str, gemm_q: usize) -> PyResult<Lapack> {
    let mut lp = Lapack::with_gemm_q(lapack_release(name)?, gemm_q);
    if name == OPENBLAS_0_3_28 {
        lp.dlarft = DlarftForm::ColumnByColumn;
    }
    Ok(lp)
}

fn direct_storev(direct: &str, storev: &str) -> PyResult<(Direct, Storev)> {
    Ok((
        one_of(
            "direct",
            direct,
            &[("F", Direct::Forward), ("B", Direct::Backward)],
        )?,
        one_of(
            "storev",
            storev,
            &[("C", Storev::Columnwise), ("R", Storev::Rowwise)],
        )?,
    ))
}

/// Runs a LAPACK routine with the GIL released in a pool of `n_threads`; a panic (an array too
/// small for its arguments) becomes `ValueError(name: invalid arguments)`.
fn lapack_run<R: Send>(
    py: Python<'_>,
    name: &str,
    n_threads: usize,
    f: impl FnOnce() -> R + Send,
) -> PyResult<R> {
    py.detach(|| {
        parallel::with_threads(n_threads, || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        })
    })
    .map_err(value_err)?
    .map_err(|_| PyValueError::new_err(format!("{name}: invalid arguments")))
}

type Factored<'py> = (Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>, usize);

/// `DGEQRF` (`lq = false`) or `DGELQF` (`lq = true`) on flat column-major storage.
#[allow(clippy::too_many_arguments)]
fn factor<'py>(
    py: Python<'py>,
    lq: bool,
    m: usize,
    n: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Factored<'py>> {
    let lp = lapack_settings(lapack, gemm_q)?;
    let mut av = contiguous(&a)?.to_vec();
    let mut tau = vec![0.0f64; m.min(n)];
    let name = if lq { "DGELQF" } else { "DGEQRF" };
    let work1 = lapack_run(py, name, n_threads, || {
        if lq {
            blocked::dgelqf(lp, m, n, &mut av, 0, lda, &mut tau, lwork)
        } else {
            blocked::dgeqrf(lp, m, n, &mut av, 0, lda, &mut tau, lwork)
        }
    })?
    .map_err(value_err)?;
    Ok((
        PyArray1::from_vec(py, av),
        PyArray1::from_vec(py, tau),
        work1,
    ))
}

/// `DGEQRF(M, N, A, LDA, TAU, WORK, LWORK, INFO)` on flat column-major storage
/// (`numpy_linalg::blocked::dgeqrf`; `lwork=None` is the optimal workspace of the query, a
/// smaller one lowers the block size as LAPACK does); returns `(A, TAU, WORK(1))`.
#[pyfunction]
#[pyo3(signature = (m, n, a, lda, lwork = None, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dgeqrf<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Factored<'py>> {
    factor(py, false, m, n, a, lda, lwork, lapack, n_threads, gemm_q)
}

/// `DGELQF(M, N, A, LDA, TAU, WORK, LWORK, INFO)`, as `lapack_dgeqrf`.
#[pyfunction]
#[pyo3(signature = (m, n, a, lda, lwork = None, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dgelqf<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Factored<'py>> {
    factor(py, true, m, n, a, lda, lwork, lapack, n_threads, gemm_q)
}

type Unblocked<'py> = (Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>);

/// `DGEQR2(M, N, A, LDA, TAU, WORK, INFO)`; returns `(A, TAU)`.
#[pyfunction]
#[pyo3(signature = (m, n, a, lda, lapack = "3.12.1"))]
fn lapack_dgeqr2<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    lapack: &str,
) -> PyResult<Unblocked<'py>> {
    let release = lapack_release(lapack)?;
    let mut av = contiguous(&a)?.to_vec();
    let mut tau = vec![0.0f64; m.min(n)];
    lapack_run(py, "DGEQR2", 1, || {
        blocked::dgeqr2(release, m, n, &mut av, 0, lda, &mut tau)
    })?
    .map_err(value_err)?;
    Ok((PyArray1::from_vec(py, av), PyArray1::from_vec(py, tau)))
}

/// `DGELQ2(M, N, A, LDA, TAU, WORK, INFO)`; returns `(A, TAU)`.
#[pyfunction]
#[pyo3(signature = (m, n, a, lda, lapack = "3.12.1"))]
fn lapack_dgelq2<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    lapack: &str,
) -> PyResult<Unblocked<'py>> {
    let release = lapack_release(lapack)?;
    let mut av = contiguous(&a)?.to_vec();
    let mut tau = vec![0.0f64; m.min(n)];
    lapack_run(py, "DGELQ2", 1, || {
        blocked::dgelq2(release, m, n, &mut av, 0, lda, &mut tau)
    })?
    .map_err(value_err)?;
    Ok((PyArray1::from_vec(py, av), PyArray1::from_vec(py, tau)))
}

/// `DLARFT(DIRECT, STOREV, N, K, V, LDV, TAU, T, LDT)`; `t` is the initial `T` storage (the
/// triangle LAPACK does not write keeps its values); returns the new `T` storage.
#[pyfunction]
#[pyo3(signature = (direct, storev, n, k, v, ldv, tau, t, ldt, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dlarft<'py>(
    py: Python<'py>,
    direct: &str,
    storev: &str,
    n: usize,
    k: usize,
    v: PyReadonlyArray1<'_, f64>,
    ldv: usize,
    tau: PyReadonlyArray1<'_, f64>,
    t: PyReadonlyArray1<'_, f64>,
    ldt: usize,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let (d, sv) = direct_storev(direct, storev)?;
    let lp = lapack_settings(lapack, gemm_q)?;
    let (v, tau) = (contiguous(&v)?, contiguous(&tau)?);
    let mut tv = contiguous(&t)?.to_vec();
    lapack_run(py, "DLARFT", n_threads, || {
        blocked::dlarft(lp, d, sv, n, k, v, 0, ldv, tau, &mut tv, 0, ldt)
    })?;
    Ok(PyArray1::from_vec(py, tv))
}

/// `DLARFB(SIDE, TRANS, DIRECT, STOREV, M, N, K, V, LDV, T, LDT, C, LDC, WORK, LDWORK)`;
/// returns the new `C` storage.
#[pyfunction]
#[pyo3(signature = (side, trans_, direct, storev, m, n, k, v, ldv, t, ldt, c, ldc, n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dlarfb<'py>(
    py: Python<'py>,
    side: &str,
    trans_: &str,
    direct: &str,
    storev: &str,
    m: usize,
    n: usize,
    k: usize,
    v: PyReadonlyArray1<'_, f64>,
    ldv: usize,
    t: PyReadonlyArray1<'_, f64>,
    ldt: usize,
    c: PyReadonlyArray1<'_, f64>,
    ldc: usize,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let s = one_of("side", side, &[("L", Side::Left), ("R", Side::Right)])?;
    let tr = trans(trans_)?;
    let (d, sv) = direct_storev(direct, storev)?;
    let (v, t) = (contiguous(&v)?, contiguous(&t)?);
    let mut cv = contiguous(&c)?.to_vec();
    lapack_run(py, "DLARFB", n_threads, || {
        blocked::dlarfb(
            gemm_q, s, tr, d, sv, m, n, k, v, 0, ldv, t, 0, ldt, &mut cv, 0, ldc,
        )
    })?;
    Ok(PyArray1::from_vec(py, cv))
}

/// `DLARF(SIDE, M, N, V, INCV, TAU, C, LDC, WORK)` (`implicit_one=False`) or LAPACK 3.12.1's
/// `DLARF1F` (`implicit_one=True`), positive `incv`; returns the new `C` storage.
#[pyfunction]
#[pyo3(signature = (side, m, n, v, incv, tau, c, ldc, implicit_one = false))]
#[allow(clippy::too_many_arguments)]
fn lapack_dlarf<'py>(
    py: Python<'py>,
    side: &str,
    m: usize,
    n: usize,
    v: PyReadonlyArray1<'_, f64>,
    incv: usize,
    tau: f64,
    c: PyReadonlyArray1<'_, f64>,
    ldc: usize,
    implicit_one: bool,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let s = one_of("side", side, &[("L", Side::Left), ("R", Side::Right)])?;
    let v = contiguous(&v)?;
    let mut cv = contiguous(&c)?.to_vec();
    lapack_run(py, "DLARF", 1, || {
        if implicit_one {
            blocked::dlarf1f(s, m, n, v, 0, incv, tau, &mut cv, 0, ldc)
        } else {
            blocked::dlarf(s, m, n, v, 0, incv, tau, &mut cv, 0, ldc)
        }
    })?;
    Ok(PyArray1::from_vec(py, cv))
}

/// `DLARFG(N, ALPHA, X, INCX, TAU)` (positive `incx`); returns `(beta, X, tau)`.
#[pyfunction]
fn lapack_dlarfg<'py>(
    py: Python<'py>,
    n: usize,
    alpha: f64,
    x: PyReadonlyArray1<'_, f64>,
    incx: usize,
) -> PyResult<(f64, Bound<'py, PyArray1<f64>>, f64)> {
    let mut xv = contiguous(&x)?.to_vec();
    let mut beta = alpha;
    let tau = lapack_run(py, "DLARFG", 1, || {
        numpy_linalg::dlarfg(n, &mut beta, &mut xv, 0, incx)
    })?;
    Ok((beta, PyArray1::from_vec(py, xv), tau))
}

type Generated<'py> = (Bound<'py, PyArray1<f64>>, usize);

/// `DORGQR` (`lq = false`) or `DORGLQ` (`lq = true`) on flat column-major storage.
#[allow(clippy::too_many_arguments)]
fn generate<'py>(
    py: Python<'py>,
    lq: bool,
    m: usize,
    n: usize,
    k: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    tau: PyReadonlyArray1<'_, f64>,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Generated<'py>> {
    let lp = lapack_settings(lapack, gemm_q)?;
    let mut av = contiguous(&a)?.to_vec();
    let tau = contiguous(&tau)?;
    let name = if lq { "DORGLQ" } else { "DORGQR" };
    let work1 = lapack_run(py, name, n_threads, || {
        if lq {
            blocked::dorglq(lp, m, n, k, &mut av, 0, lda, tau, lwork)
        } else {
            blocked::dorgqr(lp, m, n, k, &mut av, 0, lda, tau, lwork)
        }
    })?
    .map_err(value_err)?;
    Ok((PyArray1::from_vec(py, av), work1))
}

/// `DORGQR(M, N, K, A, LDA, TAU, WORK, LWORK, INFO)` on flat column-major storage
/// (`numpy_linalg::blocked::dorgqr`; `lwork=None` the optimal workspace); returns `(A,
/// WORK(1))`.
#[pyfunction]
#[pyo3(signature = (m, n, k, a, lda, tau, lwork = None, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dorgqr<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    k: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    tau: PyReadonlyArray1<'_, f64>,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Generated<'py>> {
    generate(
        py, false, m, n, k, a, lda, tau, lwork, lapack, n_threads, gemm_q,
    )
}

/// `DORGLQ(M, N, K, A, LDA, TAU, WORK, LWORK, INFO)`, as `lapack_dorgqr`.
#[pyfunction]
#[pyo3(signature = (m, n, k, a, lda, tau, lwork = None, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dorglq<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    k: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    tau: PyReadonlyArray1<'_, f64>,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Generated<'py>> {
    generate(
        py, true, m, n, k, a, lda, tau, lwork, lapack, n_threads, gemm_q,
    )
}

/// `DORG2R(M, N, K, A, LDA, TAU, WORK, INFO)`; returns the new `A` storage.
#[pyfunction]
#[pyo3(signature = (m, n, k, a, lda, tau, lapack = "3.12.1"))]
#[allow(clippy::too_many_arguments)]
fn lapack_dorg2r<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    k: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    tau: PyReadonlyArray1<'_, f64>,
    lapack: &str,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let release = lapack_release(lapack)?;
    let mut av = contiguous(&a)?.to_vec();
    let tau = contiguous(&tau)?;
    lapack_run(py, "DORG2R", 1, || {
        blocked::dorg2r(release, m, n, k, &mut av, 0, lda, tau)
    })?
    .map_err(value_err)?;
    Ok(PyArray1::from_vec(py, av))
}

/// `DORGL2(M, N, K, A, LDA, TAU, WORK, INFO)`; returns the new `A` storage.
#[pyfunction]
#[pyo3(signature = (m, n, k, a, lda, tau, lapack = "3.12.1"))]
#[allow(clippy::too_many_arguments)]
fn lapack_dorgl2<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    k: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    tau: PyReadonlyArray1<'_, f64>,
    lapack: &str,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let release = lapack_release(lapack)?;
    let mut av = contiguous(&a)?.to_vec();
    let tau = contiguous(&tau)?;
    lapack_run(py, "DORGL2", 1, || {
        blocked::dorgl2(release, m, n, k, &mut av, 0, lda, tau)
    })?
    .map_err(value_err)?;
    Ok(PyArray1::from_vec(py, av))
}

type Applied<'py> = (Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>, usize);

/// The four `ORM` routines: `(routine, blocked)` picks `DORMQR`/`DORMLQ` (`blocked = true`,
/// `lwork`) or `DORM2R`/`DORML2`.
#[allow(clippy::too_many_arguments)]
fn apply_q<'py>(
    py: Python<'py>,
    lq: bool,
    is_blocked: bool,
    side: &str,
    trans_: &str,
    m: usize,
    n: usize,
    k: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    tau: PyReadonlyArray1<'_, f64>,
    c: PyReadonlyArray1<'_, f64>,
    ldc: usize,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Applied<'py>> {
    let s = one_of("side", side, &[("L", Side::Left), ("R", Side::Right)])?;
    let tr = trans(trans_)?;
    let lp = lapack_settings(lapack, gemm_q)?;
    let mut av = contiguous(&a)?.to_vec();
    let tau = contiguous(&tau)?;
    let mut cv = contiguous(&c)?.to_vec();
    let name = match (lq, is_blocked) {
        (false, true) => "DORMQR",
        (true, true) => "DORMLQ",
        (false, false) => "DORM2R",
        (true, false) => "DORML2",
    };
    let work1 = lapack_run(py, name, n_threads, || {
        let (a, c) = (&mut av, &mut cv);
        match (lq, is_blocked) {
            (false, true) => blocked::dormqr(lp, s, tr, m, n, k, a, 0, lda, tau, c, 0, ldc, lwork),
            (true, true) => blocked::dormlq(lp, s, tr, m, n, k, a, 0, lda, tau, c, 0, ldc, lwork),
            (false, false) => {
                blocked::dorm2r(lp.release, s, tr, m, n, k, a, 0, lda, tau, c, 0, ldc).map(|()| 0)
            }
            (true, false) => {
                blocked::dorml2(lp.release, s, tr, m, n, k, a, 0, lda, tau, c, 0, ldc).map(|()| 0)
            }
        }
    })?
    .map_err(value_err)?;
    Ok((
        PyArray1::from_vec(py, av),
        PyArray1::from_vec(py, cv),
        work1,
    ))
}

/// `DORMQR(SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, LWORK, INFO)` on flat column-major
/// storage (`numpy_linalg::blocked::dormqr`; `lwork=None` the optimal workspace); returns `(A,
/// C, WORK(1))` (`A` as it was: LAPACK restores it).
#[pyfunction]
#[pyo3(signature = (side, trans_, m, n, k, a, lda, tau, c, ldc, lwork = None, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dormqr<'py>(
    py: Python<'py>,
    side: &str,
    trans_: &str,
    m: usize,
    n: usize,
    k: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    tau: PyReadonlyArray1<'_, f64>,
    c: PyReadonlyArray1<'_, f64>,
    ldc: usize,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Applied<'py>> {
    apply_q(
        py, false, true, side, trans_, m, n, k, a, lda, tau, c, ldc, lwork, lapack, n_threads,
        gemm_q,
    )
}

/// `DORMLQ(SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, LWORK, INFO)`, as `lapack_dormqr`.
#[pyfunction]
#[pyo3(signature = (side, trans_, m, n, k, a, lda, tau, c, ldc, lwork = None, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dormlq<'py>(
    py: Python<'py>,
    side: &str,
    trans_: &str,
    m: usize,
    n: usize,
    k: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    tau: PyReadonlyArray1<'_, f64>,
    c: PyReadonlyArray1<'_, f64>,
    ldc: usize,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Applied<'py>> {
    apply_q(
        py, true, true, side, trans_, m, n, k, a, lda, tau, c, ldc, lwork, lapack, n_threads,
        gemm_q,
    )
}

/// `DORM2R(SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, INFO)`; returns `(A, C, 0)`.
#[pyfunction]
#[pyo3(signature = (side, trans_, m, n, k, a, lda, tau, c, ldc, lapack = "3.12.1"))]
#[allow(clippy::too_many_arguments)]
fn lapack_dorm2r<'py>(
    py: Python<'py>,
    side: &str,
    trans_: &str,
    m: usize,
    n: usize,
    k: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    tau: PyReadonlyArray1<'_, f64>,
    c: PyReadonlyArray1<'_, f64>,
    ldc: usize,
    lapack: &str,
) -> PyResult<Applied<'py>> {
    apply_q(
        py, false, false, side, trans_, m, n, k, a, lda, tau, c, ldc, None, lapack, 1, 256,
    )
}

/// `DORML2(SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, INFO)`; returns `(A, C, 0)`.
#[pyfunction]
#[pyo3(signature = (side, trans_, m, n, k, a, lda, tau, c, ldc, lapack = "3.12.1"))]
#[allow(clippy::too_many_arguments)]
fn lapack_dorml2<'py>(
    py: Python<'py>,
    side: &str,
    trans_: &str,
    m: usize,
    n: usize,
    k: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    tau: PyReadonlyArray1<'_, f64>,
    c: PyReadonlyArray1<'_, f64>,
    ldc: usize,
    lapack: &str,
) -> PyResult<Applied<'py>> {
    apply_q(
        py, true, false, side, trans_, m, n, k, a, lda, tau, c, ldc, None, lapack, 1, 256,
    )
}

type Bidiagonal<'py> = (
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
    usize,
);

/// `DGEBRD(M, N, A, LDA, D, E, TAUQ, TAUP, WORK, LWORK, INFO)` (`blocked = true`) or `DGEBD2`
/// on flat column-major storage; returns `(A, D, E, TAUQ, TAUP, WORK(1))` (`E` of `min(m, n) -
/// 1` values, `WORK(1)` 0 for `DGEBD2`).
#[allow(clippy::too_many_arguments)]
fn bidiagonalize<'py>(
    py: Python<'py>,
    is_blocked: bool,
    m: usize,
    n: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Bidiagonal<'py>> {
    let lp = lapack_settings(lapack, gemm_q)?;
    let mut av = contiguous(&a)?.to_vec();
    let k = m.min(n);
    let (mut d, mut e) = (vec![0.0f64; k], vec![0.0f64; k]);
    let (mut tq, mut tp) = (vec![0.0f64; k], vec![0.0f64; k]);
    let name = if is_blocked { "DGEBRD" } else { "DGEBD2" };
    let work1 = lapack_run(py, name, n_threads, || {
        if is_blocked {
            blocked::dgebrd(
                lp, m, n, &mut av, 0, lda, &mut d, &mut e, &mut tq, &mut tp, lwork,
            )
        } else {
            blocked::dgebd2(
                lp.release, m, n, &mut av, 0, lda, &mut d, &mut e, &mut tq, &mut tp,
            )
            .map(|()| 0)
        }
    })?
    .map_err(value_err)?;
    e.truncate(k.saturating_sub(1));
    Ok((
        PyArray1::from_vec(py, av),
        PyArray1::from_vec(py, d),
        PyArray1::from_vec(py, e),
        PyArray1::from_vec(py, tq),
        PyArray1::from_vec(py, tp),
        work1,
    ))
}

/// `DGEBRD(M, N, A, LDA, D, E, TAUQ, TAUP, WORK, LWORK, INFO)` on flat column-major storage
/// (`numpy_linalg::blocked::dgebrd`; `lwork=None` the optimal workspace); returns `(A, D, E,
/// TAUQ, TAUP, WORK(1))`.
#[pyfunction]
#[pyo3(signature = (m, n, a, lda, lwork = None, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dgebrd<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Bidiagonal<'py>> {
    bidiagonalize(py, true, m, n, a, lda, lwork, lapack, n_threads, gemm_q)
}

/// `DGEBD2(M, N, A, LDA, D, E, TAUQ, TAUP, WORK, INFO)`; returns `(A, D, E, TAUQ, TAUP, 0)`.
#[pyfunction]
#[pyo3(signature = (m, n, a, lda, lapack = "3.12.1"))]
fn lapack_dgebd2<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    lapack: &str,
) -> PyResult<Bidiagonal<'py>> {
    bidiagonalize(py, false, m, n, a, lda, None, lapack, 1, 256)
}

type Labrd<'py> = (
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
);

/// `DLABRD(M, N, NB, A, LDA, D, E, TAUQ, TAUP, X, LDX, Y, LDY)` on flat column-major storage
/// (`x`, `y`: the initial workspaces, whose entries LAPACK does not write are kept); returns
/// `(A, D, E, TAUQ, TAUP, X, Y)` (`nb` values each for `D` to `TAUP`).
#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn lapack_dlabrd<'py>(
    py: Python<'py>,
    m: usize,
    n: usize,
    nb: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    x: PyReadonlyArray1<'_, f64>,
    ldx: usize,
    y: PyReadonlyArray1<'_, f64>,
    ldy: usize,
) -> PyResult<Labrd<'py>> {
    let mut av = contiguous(&a)?.to_vec();
    let mut xv = contiguous(&x)?.to_vec();
    let mut yv = contiguous(&y)?.to_vec();
    let (mut d, mut e) = (vec![0.0f64; nb], vec![0.0f64; nb]);
    let (mut tq, mut tp) = (vec![0.0f64; nb], vec![0.0f64; nb]);
    lapack_run(py, "DLABRD", 1, || {
        blocked::dlabrd(
            m, n, nb, &mut av, 0, lda, &mut d, &mut e, &mut tq, &mut tp, &mut xv, 0, ldx, &mut yv,
            0, ldy,
        )
    })?;
    Ok((
        PyArray1::from_vec(py, av),
        PyArray1::from_vec(py, d),
        PyArray1::from_vec(py, e),
        PyArray1::from_vec(py, tq),
        PyArray1::from_vec(py, tp),
        PyArray1::from_vec(py, xv),
        PyArray1::from_vec(py, yv),
    ))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(blas_dgemm, m)?)?;
    m.add_function(wrap_pyfunction!(blas_dgemv, m)?)?;
    m.add_function(wrap_pyfunction!(blas_dtrmm, m)?)?;
    m.add_function(wrap_pyfunction!(blas_dger, m)?)?;
    m.add_function(wrap_pyfunction!(blas_dtrmv, m)?)?;
    m.add_function(wrap_pyfunction!(blas_daxpy, m)?)?;
    m.add_function(wrap_pyfunction!(blas_dscal, m)?)?;
    m.add_function(wrap_pyfunction!(blas_dcopy, m)?)?;
    m.add_function(wrap_pyfunction!(blas_dswap, m)?)?;
    m.add_function(wrap_pyfunction!(blas_idamax, m)?)?;
    m.add_function(wrap_pyfunction!(blas_dnrm2, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_matmul, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dgeqrf, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dgelqf, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dgeqr2, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dgelq2, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlarft, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlarfb, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlarf, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlarfg, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dorgqr, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dorglq, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dorg2r, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dorgl2, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dormqr, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dormlq, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dorm2r, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dorml2, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dgebrd, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dgebd2, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlabrd, m)?)?;
    svd::register(m)?;
    gelsd::register(m)?;
    gesvd::register(m)?;
    reductions::register(m)?;
    fft::register(m)?;
    dvars::register(m)?;
    compcor::register(m)?;
    nipy::register(m)?;
    pandas::register(m)?;
    expansion::register(m)?;
    motion::register(m)?;
    Ok(())
}
