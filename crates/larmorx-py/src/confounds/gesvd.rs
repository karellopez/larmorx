// SPDX-License-Identifier: Apache-2.0
//! Bindings of LAPACK's QR-iteration SVD (`DGESVD` with `JOBU = JOBVT = 'S'` or `'A'`, and its
//! bidiagonal solver `DBDSQR`) at their Fortran interface (`lapack_*`, as
//! `larmorx_transform::numpy_linalg::gesvd` computes them), SciPy's `linalg.svd(a, full_matrices,
//! lapack_driver='gesvd')` on top of them with SciPy 1.18.1's or 1.15.3's own library
//! (`scipy_svd`), and nipype's `fallback_svd` and `_full_rank` (`nipype_fallback_svd`,
//! `nipype_full_rank`); the validation compares them with SciPy's own library and functions and
//! with nipype's. Matrices cross as flat column-major storage with their leading dimension (the
//! `lapack_*` functions) or as 2-D arrays. The GIL is released while computing.

use super::svd::{F64s, Mat, c_view, linalg_error, lp, mat, uplo};
use super::{contiguous, lapack_release, lapack_run, one_of, value_err};
use larmorx_transform::numpy_linalg::gesvd::{self, DgesvdError, Job, ScipyLibrary, ScipySvdError};
use numpy::{PyArray1, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::{PyNotImplementedError, PyValueError};
use pyo3::prelude::*;

fn job(v: &str) -> PyResult<Job> {
    if v.eq_ignore_ascii_case("N") || v.eq_ignore_ascii_case("O") {
        return Err(PyNotImplementedError::new_err(format!(
            "DGESVD: only JOBU = JOBVT = 'S' or 'A' are ported, got {v:?}"
        )));
    }
    one_of("job", v, &[("S", Job::Some), ("A", Job::All)])
}

fn gesvd_err(e: DgesvdError) -> PyErr {
    match e {
        DgesvdError::ValuesOnlyNotPorted => PyNotImplementedError::new_err(e.to_string()),
        other => value_err(other),
    }
}

fn scipy_library(v: &str) -> PyResult<ScipyLibrary> {
    one_of(
        "scipy",
        v,
        &[
            ("1.18.1", ScipyLibrary::V1_18),
            ("1.15.3", ScipyLibrary::V1_15),
        ],
    )
}

fn scipy_err(py: Python<'_>, e: ScipySvdError) -> PyErr {
    if e.is_linalg_error() {
        linalg_error(py, &e.to_string())
    } else {
        PyValueError::new_err(e.to_string())
    }
}

/// `DGESVD(JOBU, JOBVT, M, N, A, LDA, S, U, LDU, VT, LDVT, WORK, LWORK, INFO)` with `JOBU = JOBVT`
/// `"S"` or `"A"` on flat column-major storage: `u` and `vt` start as given and keep their entries
/// outside the blocks LAPACK writes; `lwork=None` is the optimal workspace (SciPy's query).
/// `lapack`: `"3.12.0"` (SciPy 1.18.1's library), `"3.12.0-openblas-0.3.28"` (SciPy 1.15.3's) or
/// `"3.12.1"`. Returns `(A, S, U, VT, WORK(1), INFO, WORK(2:MIN(M, N)))` (the last empty unless
/// `INFO > 0`). A non-finite entry is refused (`ValueError`).
#[pyfunction]
#[pyo3(signature = (jobu, jobvt, m, n, a, lda, u, ldu, vt, ldvt, lwork = None, lapack = "3.12.0", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn lapack_dgesvd<'py>(
    py: Python<'py>,
    jobu: &str,
    jobvt: &str,
    m: usize,
    n: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
    u: PyReadonlyArray1<'_, f64>,
    ldu: usize,
    vt: PyReadonlyArray1<'_, f64>,
    ldvt: usize,
    lwork: Option<usize>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<(
    F64s<'py>,
    F64s<'py>,
    F64s<'py>,
    F64s<'py>,
    usize,
    usize,
    F64s<'py>,
)> {
    let j = job(jobu)?;
    if job(jobvt)? != j {
        return Err(PyNotImplementedError::new_err(
            "DGESVD: only JOBU = JOBVT is ported",
        ));
    }
    let lp = lp(lapack, gemm_q)?;
    let mut av = contiguous(&a)?.to_vec();
    let mut uv = contiguous(&u)?.to_vec();
    let mut vtv = contiguous(&vt)?.to_vec();
    let mut s = vec![0.0f64; m.min(n)];
    let out = lapack_run(py, "DGESVD", n_threads, || {
        gesvd::dgesvd(
            lp, j, m, n, &mut av, lda, &mut s, &mut uv, ldu, &mut vtv, ldvt, lwork,
        )
    })?
    .map_err(gesvd_err)?;
    Ok((
        PyArray1::from_vec(py, av),
        PyArray1::from_vec(py, s),
        PyArray1::from_vec(py, uv),
        PyArray1::from_vec(py, vtv),
        out.work1,
        out.info,
        PyArray1::from_vec(py, out.superdiag),
    ))
}

/// `DGESVD(JOB, JOB, M, N, ...)`'s workspace: `(MINWRK, MAXWRK)`, the smallest `LWORK` it accepts
/// and the optimal one (`WORK(1)` of the `LWORK = -1` query).
#[pyfunction]
fn lapack_dgesvd_lwork(job_: &str, m: usize, n: usize) -> PyResult<(usize, usize)> {
    Ok(gesvd::dgesvd_lwork(job(job_)?, m, n))
}

/// `DBDSQR(UPLO, N, NCVT, NRU, NCC, D, E, VT, LDVT, U, LDU, C, LDC, WORK, INFO)` on flat
/// column-major `vt` (`ldvt × ncvt`), `u` (`ldu × n`) and `c` (`ldc × ncc`), whose entries outside
/// the blocks LAPACK rotates are kept; `lapack` as `lapack_dgesvd`'s (the release decides whether a
/// `-0` value stays). Returns `(D, E, VT, U, C, INFO)`. Without vectors (`N >= 2`) LAPACK takes
/// `DLASQ1`, not ported (`NotImplementedError`).
#[pyfunction]
#[pyo3(signature = (uplo_, n, ncvt, nru, ncc, d, e, vt, ldvt, u, ldu, c, ldc, lapack = "3.12.0", n_threads = 1))]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn lapack_dbdsqr<'py>(
    py: Python<'py>,
    uplo_: &str,
    n: usize,
    ncvt: usize,
    nru: usize,
    ncc: usize,
    d: PyReadonlyArray1<'_, f64>,
    e: PyReadonlyArray1<'_, f64>,
    vt: PyReadonlyArray1<'_, f64>,
    ldvt: usize,
    u: PyReadonlyArray1<'_, f64>,
    ldu: usize,
    c: PyReadonlyArray1<'_, f64>,
    ldc: usize,
    lapack: &str,
    n_threads: usize,
) -> PyResult<(F64s<'py>, F64s<'py>, F64s<'py>, F64s<'py>, F64s<'py>, usize)> {
    let ul = uplo(uplo_)?;
    let release = lapack_release(lapack)?;
    let mut dv = contiguous(&d)?.to_vec();
    let mut ev = contiguous(&e)?.to_vec();
    if dv.len() < n || ev.len() < n.saturating_sub(1) {
        return Err(PyValueError::new_err("DBDSQR: d or e is too short"));
    }
    let mut vtv = contiguous(&vt)?.to_vec();
    let mut uv = contiguous(&u)?.to_vec();
    let mut cv = contiguous(&c)?.to_vec();
    let info = lapack_run(py, "DBDSQR", n_threads, || {
        gesvd::dbdsqr(
            release,
            ul,
            n,
            ncvt,
            nru,
            ncc,
            &mut dv,
            &mut ev,
            (&mut vtv, ldvt),
            (&mut uv, ldu),
            (&mut cv, ldc),
        )
    })?
    .map_err(gesvd_err)?;
    Ok((
        PyArray1::from_vec(py, dv),
        PyArray1::from_vec(py, ev),
        PyArray1::from_vec(py, vtv),
        PyArray1::from_vec(py, uv),
        PyArray1::from_vec(py, cv),
        info,
    ))
}

fn read2(a: &PyReadonlyArray2<'_, f64>) -> (Vec<f64>, usize, usize) {
    let v = a.as_array();
    let (rows, cols) = v.dim();
    (v.iter().copied().collect(), rows, cols)
}

/// `scipy.linalg.svd(a, full_matrices, compute_uv=True, lapack_driver='gesvd')` of a 2-D float64
/// array (any strides) with SciPy `scipy`'s own library (`"1.18.1"` or `"1.15.3"`): `(u, s, vh)`,
/// C-ordered. Raises SciPy's `ValueError` (a non-finite entry, a factor too large for 32-bit
/// LAPACK) and `LinAlgError` (`DGESVD`'s `INFO > 0`) with its messages. Only the `gesvd` driver
/// with `compute_uv=True` is ported (`NotImplementedError` otherwise).
#[pyfunction]
#[pyo3(signature = (a, full_matrices = true, compute_uv = true, lapack_driver = "gesvd", scipy = "1.18.1", n_threads = 1))]
fn scipy_svd<'py>(
    py: Python<'py>,
    a: PyReadonlyArray2<'_, f64>,
    full_matrices: bool,
    compute_uv: bool,
    lapack_driver: &str,
    scipy: &str,
    n_threads: usize,
) -> PyResult<(Mat<'py>, F64s<'py>, Mat<'py>)> {
    if lapack_driver != "gesvd" || !compute_uv {
        return Err(PyNotImplementedError::new_err(
            "scipy.linalg.svd: only lapack_driver='gesvd' with compute_uv=True is ported",
        ));
    }
    let lib = scipy_library(scipy)?;
    let (data, m, n) = read2(&a);
    let res = lapack_run(py, "scipy.linalg.svd", n_threads, || {
        gesvd::scipy_svd(lib, c_view(&data, m, n), full_matrices)
    })?;
    let r = res.map_err(|e| scipy_err(py, e))?;
    Ok((
        mat(py, m, r.ucols, r.u)?,
        PyArray1::from_vec(py, r.s),
        mat(py, r.vtrows, n, r.vt)?,
    ))
}

/// nipype's `fallback_svd(a, full_matrices=False)`: numpy's `svd` with numpy's LAPACK (`lapack`:
/// `"3.12.1"` numpy 2.5.4, `"3.12.0"` numpy 2.3.5; `gemm_q` its depth block), and on its
/// `LinAlgError` SciPy `scipy`'s `svd` with the `gesvd` driver. Returns `(u, s, vt, fallback)`
/// (`fallback`: SciPy computed it); raises SciPy's errors. Only `full_matrices=False` and
/// `compute_uv=True` are ported.
#[pyfunction]
#[pyo3(signature = (a, full_matrices = false, compute_uv = true, lapack = "3.12.1", scipy = "1.18.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn nipype_fallback_svd<'py>(
    py: Python<'py>,
    a: PyReadonlyArray2<'_, f64>,
    full_matrices: bool,
    compute_uv: bool,
    lapack: &str,
    scipy: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<(Mat<'py>, F64s<'py>, Mat<'py>, bool)> {
    if full_matrices || !compute_uv {
        return Err(PyNotImplementedError::new_err(
            "fallback_svd: only full_matrices=False with compute_uv=True is ported",
        ));
    }
    let numpy = lp(lapack, gemm_q)?;
    let lib = scipy_library(scipy)?;
    let (data, m, n) = read2(&a);
    let res = lapack_run(py, "fallback_svd", n_threads, || {
        gesvd::nipype_fallback_svd(numpy, lib, c_view(&data, m, n))
    })?;
    let (svd, fallback) = res.map_err(|e| scipy_err(py, e))?;
    let k = svd.k;
    Ok((
        mat(py, m, k, svd.u)?,
        PyArray1::from_vec(py, svd.s),
        mat(py, k, n, svd.vt)?,
        fallback,
    ))
}

/// nipype's `_full_rank(X, cmax)` with `nipype_fallback_svd`'s libraries: `(X, c, regularized)`
/// (`X` C-ordered, the input's values unless regularized; `c` the condition number, or `cmax`).
#[pyfunction]
#[pyo3(signature = (x, cmax = 1e15, lapack = "3.12.1", scipy = "1.18.1", n_threads = 1, gemm_q = 256))]
fn nipype_full_rank<'py>(
    py: Python<'py>,
    x: PyReadonlyArray2<'_, f64>,
    cmax: f64,
    lapack: &str,
    scipy: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<(Mat<'py>, f64, bool)> {
    let numpy = lp(lapack, gemm_q)?;
    let lib = scipy_library(scipy)?;
    let (data, m, n) = read2(&x);
    let res = lapack_run(py, "_full_rank", n_threads, || {
        gesvd::nipype_full_rank(numpy, lib, c_view(&data, m, n), cmax)
    })?;
    let r = res.map_err(|e| scipy_err(py, e))?;
    Ok((mat(py, m, n, r.x)?, r.c, r.regularized))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(lapack_dgesvd, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dgesvd_lwork, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dbdsqr, m)?)?;
    m.add_function(wrap_pyfunction!(scipy_svd, m)?)?;
    m.add_function(wrap_pyfunction!(nipype_fallback_svd, m)?)?;
    m.add_function(wrap_pyfunction!(nipype_full_rank, m)?)?;
    Ok(())
}
