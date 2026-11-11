// SPDX-License-Identifier: Apache-2.0
//! Bindings of LAPACK's divide-and-conquer bidiagonal SVD (`DBDSDC` with `COMPQ = 'I'` and the
//! routines under it: `DLASD0` to `DLASD5`, `DLASDQ`, `DLASDT`, `DLAMRG`, `DLAED6`, `DLASCL`),
//! the bidiagonal reduction's `Q` and `P` (`DORMBR`, `DORGBR`) and the thin SVD (`DGESDD` with
//! `JOBZ = 'S'`) at their Fortran interface (`lapack_*`, as
//! `larmorx_transform::numpy_linalg::{bdsdc, blocked, svd}` compute them), and numpy's `svd(a,
//! full_matrices=False)` and `pinv(a)` on top of them (`numpy_svd`, `numpy_pinv`); the validation
//! compares them with numpy's own library and functions. Matrices cross as flat column-major
//! storage with their leading dimension (the `lapack_*` functions) or as 2-D arrays (the `numpy_*`
//! ones); index arrays as int64 (1-based, as LAPACK keeps them). The GIL is released while
//! computing.

use super::{contiguous, lapack_release, lapack_run, lapack_settings, one_of, trans, value_err};
use larmorx_transform::numpy_linalg::bdsdc::{self, MergeWork};
use larmorx_transform::numpy_linalg::blocked::{self, Lapack, Vect};
use larmorx_transform::numpy_linalg::svd::{self as tsvd, DgesddError};
use larmorx_transform::openblas::level3::Trans;
use larmorx_transform::openblas::matmul::Array2 as View;
use larmorx_transform::openblas::trmm::{Side, Uplo};
use numpy::ndarray::Array2;
use numpy::{PyArray, PyArray1, PyArray2, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

pub(super) type F64s<'py> = Bound<'py, PyArray1<f64>>;
type I64s<'py> = Bound<'py, PyArray1<i64>>;

pub(super) fn uplo(v: &str) -> PyResult<Uplo> {
    one_of("uplo", v, &[("U", Uplo::Upper), ("L", Uplo::Lower)])
}

pub(super) fn lp(lapack: &str, gemm_q: usize) -> PyResult<Lapack> {
    lapack_settings(lapack, gemm_q)
}

pub(super) fn indices(x: &PyReadonlyArray1<'_, i64>) -> PyResult<Vec<usize>> {
    x.as_slice()
        .map_err(value_err)?
        .iter()
        .map(|&v| usize::try_from(v).map_err(value_err))
        .collect()
}

pub(super) fn to_i64<'py>(py: Python<'py>, v: &[usize]) -> Bound<'py, PyArray1<i64>> {
    PyArray1::from_vec(py, v.iter().map(|&x| x as i64).collect())
}

/// `DBDSDC(UPLO, 'I', N, D, E, U, LDU, VT, LDVT, ...)`: the singular values (descending) and
/// vectors of an `n × n` bidiagonal matrix (`d`: `n` values, `e`: `n - 1`); `u` and `vt` start
/// as zeros (`ldu × n`, `ldvt × n`). Only `compq = "I"` is ported. Returns `(D, E, U, VT,
/// INFO)`.
#[pyfunction]
#[pyo3(signature = (uplo_, compq, n, d, e, ldu, ldvt, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dbdsdc<'py>(
    py: Python<'py>,
    uplo_: &str,
    compq: &str,
    n: usize,
    d: PyReadonlyArray1<'_, f64>,
    e: PyReadonlyArray1<'_, f64>,
    ldu: usize,
    ldvt: usize,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<(F64s<'py>, F64s<'py>, F64s<'py>, F64s<'py>, usize)> {
    let ul = uplo(uplo_)?;
    if !compq.eq_ignore_ascii_case("I") {
        return Err(PyValueError::new_err(format!(
            "DBDSDC: only COMPQ = 'I' is ported, got {compq:?}"
        )));
    }
    let lp = lp(lapack, gemm_q)?;
    let mut dv = contiguous(&d)?.to_vec();
    let mut ev = contiguous(&e)?.to_vec();
    let mut u = vec![0.0f64; (ldu * n).max(1)];
    let mut vt = vec![0.0f64; (ldvt * n).max(1)];
    let info = lapack_run(py, "DBDSDC", n_threads, || {
        bdsdc::dbdsdc(lp, ul, n, &mut dv, &mut ev, &mut u, ldu, &mut vt, ldvt)
    })?
    .map_err(value_err)?;
    Ok((
        PyArray1::from_vec(py, dv),
        PyArray1::from_vec(py, ev),
        PyArray1::from_vec(py, u),
        PyArray1::from_vec(py, vt),
        info,
    ))
}

/// `DLASD0(N, SQRE, D, E, U, LDU, VT, LDVT, SMLSIZ, IWORK, WORK, INFO)` on flat column-major
/// `u` (`ldu × n`) and `vt` (`ldvt × (n + sqre)`), whose entries outside the blocks LAPACK
/// writes are kept. Returns `(D, E, U, VT, INFO)`.
#[pyfunction]
#[pyo3(signature = (n, sqre, d, e, u, ldu, vt, ldvt, smlsiz, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dlasd0<'py>(
    py: Python<'py>,
    n: usize,
    sqre: usize,
    d: PyReadonlyArray1<'_, f64>,
    e: PyReadonlyArray1<'_, f64>,
    u: PyReadonlyArray1<'_, f64>,
    ldu: usize,
    vt: PyReadonlyArray1<'_, f64>,
    ldvt: usize,
    smlsiz: usize,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<(F64s<'py>, F64s<'py>, F64s<'py>, F64s<'py>, usize)> {
    let lp = lp(lapack, gemm_q)?;
    let mut dv = contiguous(&d)?.to_vec();
    let mut ev = contiguous(&e)?.to_vec();
    let mut uv = contiguous(&u)?.to_vec();
    let mut vtv = contiguous(&vt)?.to_vec();
    let info = lapack_run(py, "DLASD0", n_threads, || {
        bdsdc::dlasd0(
            lp, n, sqre, &mut dv, &mut ev, &mut uv, 0, ldu, &mut vtv, 0, ldvt, smlsiz,
        )
    })?
    .map_err(value_err)?;
    Ok((
        PyArray1::from_vec(py, dv),
        PyArray1::from_vec(py, ev),
        PyArray1::from_vec(py, uv),
        PyArray1::from_vec(py, vtv),
        info,
    ))
}

type Merged<'py> = (F64s<'py>, f64, f64, F64s<'py>, F64s<'py>, I64s<'py>, usize);

/// `DLASD1(NL, NR, SQRE, D, ALPHA, BETA, U, LDU, VT, LDVT, IDXQ, IWORK, WORK, INFO)`; returns
/// `(D, ALPHA, BETA, U, VT, IDXQ, INFO)`.
#[pyfunction]
#[pyo3(signature = (nl, nr, sqre, d, alpha, beta, u, ldu, vt, ldvt, idxq, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dlasd1<'py>(
    py: Python<'py>,
    nl: usize,
    nr: usize,
    sqre: usize,
    d: PyReadonlyArray1<'_, f64>,
    alpha: f64,
    beta: f64,
    u: PyReadonlyArray1<'_, f64>,
    ldu: usize,
    vt: PyReadonlyArray1<'_, f64>,
    ldvt: usize,
    idxq: PyReadonlyArray1<'_, i64>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Merged<'py>> {
    let lp = lp(lapack, gemm_q)?;
    let mut dv = contiguous(&d)?.to_vec();
    let mut uv = contiguous(&u)?.to_vec();
    let mut vtv = contiguous(&vt)?.to_vec();
    let mut iq = indices(&idxq)?;
    let (mut a, mut b) = (alpha, beta);
    let info = lapack_run(py, "DLASD1", n_threads, || {
        bdsdc::dlasd1(
            lp, nl, nr, sqre, &mut dv, &mut a, &mut b, &mut uv, 0, ldu, &mut vtv, 0, ldvt, &mut iq,
        )
    })?
    .map_err(value_err)?;
    Ok((
        PyArray1::from_vec(py, dv),
        a,
        b,
        PyArray1::from_vec(py, uv),
        PyArray1::from_vec(py, vtv),
        to_i64(py, &iq),
        info,
    ))
}

/// What `lapack_dlasd2` returns: `(K, (D, Z, U, VT, DSIGMA, U2, VT2), (IDXP, IDX, IDXC, IDXQ,
/// COLTYP))`.
type Deflated<'py> = (
    usize,
    (
        F64s<'py>,
        F64s<'py>,
        F64s<'py>,
        F64s<'py>,
        F64s<'py>,
        F64s<'py>,
        F64s<'py>,
    ),
    (I64s<'py>, I64s<'py>, I64s<'py>, I64s<'py>, I64s<'py>),
);

/// `DLASD2(NL, NR, SQRE, K, D, Z, ALPHA, BETA, U, LDU, VT, LDVT, DSIGMA, U2, LDU2, VT2, LDVT2,
/// IDXP, IDX, IDXC, IDXQ, COLTYP, INFO)`; the work arrays start as zeros (`Z`: `m`, `DSIGMA`,
/// the index arrays: `n`, `COLTYP`: `max(n, 4)`, `U2`: `ldu2 × n`, `VT2`: `ldvt2 × m`).
#[pyfunction]
#[pyo3(signature = (nl, nr, sqre, d, alpha, beta, u, ldu, vt, ldvt, ldu2, ldvt2, idxq, lapack = "3.12.1"))]
#[allow(clippy::too_many_arguments)]
fn lapack_dlasd2<'py>(
    py: Python<'py>,
    nl: usize,
    nr: usize,
    sqre: usize,
    d: PyReadonlyArray1<'_, f64>,
    alpha: f64,
    beta: f64,
    u: PyReadonlyArray1<'_, f64>,
    ldu: usize,
    vt: PyReadonlyArray1<'_, f64>,
    ldvt: usize,
    ldu2: usize,
    ldvt2: usize,
    idxq: PyReadonlyArray1<'_, i64>,
    lapack: &str,
) -> PyResult<Deflated<'py>> {
    let release = lapack_release(lapack)?;
    let n = nl + nr + 1;
    let m = n + sqre;
    let mut dv = contiguous(&d)?.to_vec();
    let mut uv = contiguous(&u)?.to_vec();
    let mut vtv = contiguous(&vt)?.to_vec();
    let mut iq = indices(&idxq)?;
    let mut z = vec![0.0f64; m];
    let mut dsigma = vec![0.0f64; n];
    let mut u2 = vec![0.0f64; ldu2 * n];
    let mut vt2 = vec![0.0f64; ldvt2 * m];
    let (mut idxp, mut idx, mut idxc, mut coltyp) = (
        vec![0usize; n],
        vec![0usize; n],
        vec![0usize; n],
        vec![0usize; n.max(4)],
    );
    let defl = lapack_run(py, "DLASD2", 1, || {
        bdsdc::dlasd2(
            release,
            nl,
            nr,
            sqre,
            &mut dv,
            alpha,
            beta,
            &mut uv,
            0,
            ldu,
            &mut vtv,
            0,
            ldvt,
            MergeWork {
                z: &mut z,
                dsigma: &mut dsigma,
                u2: &mut u2,
                ldu2,
                vt2: &mut vt2,
                ldvt2,
                idxp: &mut idxp,
                idx: &mut idx,
                idxc: &mut idxc,
                coltyp: &mut coltyp,
            },
            &mut iq,
        )
    })?
    .map_err(value_err)?;
    Ok((
        defl.k,
        (
            PyArray1::from_vec(py, dv),
            PyArray1::from_vec(py, z),
            PyArray1::from_vec(py, uv),
            PyArray1::from_vec(py, vtv),
            PyArray1::from_vec(py, dsigma),
            PyArray1::from_vec(py, u2),
            PyArray1::from_vec(py, vt2),
        ),
        (
            to_i64(py, &idxp),
            to_i64(py, &idx),
            to_i64(py, &idxc),
            to_i64(py, &iq),
            to_i64(py, &coltyp),
        ),
    ))
}

type Solved<'py> = (
    F64s<'py>,
    F64s<'py>,
    F64s<'py>,
    F64s<'py>,
    F64s<'py>,
    F64s<'py>,
    usize,
);

/// `DLASD3(NL, NR, SQRE, K, D, Q, LDQ, DSIGMA, U, LDU, U2, LDU2, VT, LDVT, VT2, LDVT2, IDXC,
/// CTOT, Z, INFO)` (`Q` starts as zeros, `ldq × k`); returns `(D, Q, U, VT, VT2, Z, INFO)`.
#[pyfunction]
#[pyo3(signature = (nl, nr, sqre, k, d, ldq, dsigma, u, ldu, u2, ldu2, vt, ldvt, vt2, ldvt2, idxc, ctot, z, n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dlasd3<'py>(
    py: Python<'py>,
    nl: usize,
    nr: usize,
    sqre: usize,
    k: usize,
    d: PyReadonlyArray1<'_, f64>,
    ldq: usize,
    dsigma: PyReadonlyArray1<'_, f64>,
    u: PyReadonlyArray1<'_, f64>,
    ldu: usize,
    u2: PyReadonlyArray1<'_, f64>,
    ldu2: usize,
    vt: PyReadonlyArray1<'_, f64>,
    ldvt: usize,
    vt2: PyReadonlyArray1<'_, f64>,
    ldvt2: usize,
    idxc: PyReadonlyArray1<'_, i64>,
    ctot: [usize; 4],
    z: PyReadonlyArray1<'_, f64>,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Solved<'py>> {
    let mut dv = contiguous(&d)?.to_vec();
    let ds = contiguous(&dsigma)?.to_vec();
    let mut uv = contiguous(&u)?.to_vec();
    let u2v = contiguous(&u2)?.to_vec();
    let mut vtv = contiguous(&vt)?.to_vec();
    let mut vt2v = contiguous(&vt2)?.to_vec();
    let ic = indices(&idxc)?;
    let mut zv = contiguous(&z)?.to_vec();
    let mut q = vec![0.0f64; ldq * k];
    let info = lapack_run(py, "DLASD3", n_threads, || {
        bdsdc::dlasd3(
            gemm_q, nl, nr, sqre, k, &mut dv, &mut q, ldq, &ds, &mut uv, 0, ldu, &u2v, ldu2,
            &mut vtv, 0, ldvt, &mut vt2v, ldvt2, &ic, &ctot, &mut zv,
        )
    })?
    .map_err(value_err)?;
    Ok((
        PyArray1::from_vec(py, dv),
        PyArray1::from_vec(py, q),
        PyArray1::from_vec(py, uv),
        PyArray1::from_vec(py, vtv),
        PyArray1::from_vec(py, vt2v),
        PyArray1::from_vec(py, zv),
        info,
    ))
}

/// `DLASD4(N, I, D, Z, DELTA, RHO, SIGMA, WORK, INFO)`; returns `(DELTA, SIGMA, WORK, INFO)`.
#[pyfunction]
fn lapack_dlasd4<'py>(
    py: Python<'py>,
    n: usize,
    i: usize,
    d: PyReadonlyArray1<'_, f64>,
    z: PyReadonlyArray1<'_, f64>,
    rho: f64,
) -> PyResult<(F64s<'py>, f64, F64s<'py>, usize)> {
    let dv = contiguous(&d)?.to_vec();
    let zv = contiguous(&z)?.to_vec();
    let (mut delta, mut work) = (vec![0.0f64; n], vec![0.0f64; n]);
    let (sigma, info) = lapack_run(py, "DLASD4", 1, || {
        bdsdc::dlasd4(n, i, &dv, &zv, &mut delta, rho, &mut work)
    })?;
    Ok((
        PyArray1::from_vec(py, delta),
        sigma,
        PyArray1::from_vec(py, work),
        info,
    ))
}

/// `DLASD5(I, D, Z, DELTA, RHO, DSIGMA, WORK)`; returns `(DELTA, DSIGMA, WORK)`.
#[pyfunction]
fn lapack_dlasd5<'py>(
    py: Python<'py>,
    i: usize,
    d: PyReadonlyArray1<'_, f64>,
    z: PyReadonlyArray1<'_, f64>,
    rho: f64,
) -> PyResult<(F64s<'py>, f64, F64s<'py>)> {
    let dv = contiguous(&d)?.to_vec();
    let zv = contiguous(&z)?.to_vec();
    let (mut delta, mut work) = (vec![0.0f64; 2], vec![0.0f64; 2]);
    let s = lapack_run(py, "DLASD5", 1, || {
        bdsdc::dlasd5(i, &dv, &zv, &mut delta, rho, &mut work)
    })?;
    Ok((
        PyArray1::from_vec(py, delta),
        s,
        PyArray1::from_vec(py, work),
    ))
}

/// `DLAED6(KNITER, ORGATI, RHO, D, Z, FINIT, TAU, INFO)`; returns `(TAU, INFO)`.
#[pyfunction]
fn lapack_dlaed6(
    kniter: usize,
    orgati: bool,
    rho: f64,
    d: [f64; 3],
    z: [f64; 3],
    finit: f64,
) -> (f64, usize) {
    bdsdc::dlaed6(kniter, orgati, rho, &d, &z, finit)
}

type Leaf<'py> = (F64s<'py>, F64s<'py>, F64s<'py>, F64s<'py>, F64s<'py>, usize);

/// `DLASDQ(UPLO, SQRE, N, NCVT, NRU, NCC, D, E, VT, LDVT, U, LDU, C, LDC, WORK, INFO)`; returns
/// `(D, E, VT, U, C, INFO)`.
#[pyfunction]
#[pyo3(signature = (uplo_, sqre, n, ncvt, nru, ncc, d, e, vt, ldvt, u, ldu, c, ldc, lapack = "3.12.1"))]
#[allow(clippy::too_many_arguments)]
fn lapack_dlasdq<'py>(
    py: Python<'py>,
    uplo_: &str,
    sqre: usize,
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
) -> PyResult<Leaf<'py>> {
    let ul = uplo(uplo_)?;
    let release = lapack_release(lapack)?;
    let mut dv = contiguous(&d)?.to_vec();
    let mut ev = contiguous(&e)?.to_vec();
    let mut vtv = contiguous(&vt)?.to_vec();
    let mut uv = contiguous(&u)?.to_vec();
    let mut cv = contiguous(&c)?.to_vec();
    let info = lapack_run(py, "DLASDQ", 1, || {
        bdsdc::dlasdq(
            release, ul, sqre, n, ncvt, nru, ncc, &mut dv, &mut ev, &mut vtv, 0, ldvt, &mut uv, 0,
            ldu, &mut cv, 0, ldc,
        )
    })?
    .map_err(value_err)?;
    Ok((
        PyArray1::from_vec(py, dv),
        PyArray1::from_vec(py, ev),
        PyArray1::from_vec(py, vtv),
        PyArray1::from_vec(py, uv),
        PyArray1::from_vec(py, cv),
        info,
    ))
}

/// `DLASDT(N, LVL, ND, INODE, NDIML, NDIMR, MSUB)`; returns `(LVL, ND, INODE, NDIML, NDIMR)`
/// (`max(n, 1)` entries each).
#[pyfunction]
fn lapack_dlasdt(
    py: Python<'_>,
    n: usize,
    msub: usize,
) -> (i64, usize, I64s<'_>, I64s<'_>, I64s<'_>) {
    let t = bdsdc::dlasdt(n, msub);
    (
        t.lvl,
        t.nd,
        PyArray1::from_vec(py, t.inode),
        PyArray1::from_vec(py, t.ndiml),
        PyArray1::from_vec(py, t.ndimr),
    )
}

/// `DLAMRG(N1, N2, A, DTRD1, DTRD2, INDEX)`; returns `INDEX` (1-based).
#[pyfunction]
fn lapack_dlamrg<'py>(
    py: Python<'py>,
    n1: usize,
    n2: usize,
    a: PyReadonlyArray1<'_, f64>,
    dtrd1: i64,
    dtrd2: i64,
) -> PyResult<I64s<'py>> {
    let av = contiguous(&a)?;
    if av.len() < n1 + n2 || dtrd1.abs() != 1 || dtrd2.abs() != 1 {
        return Err(PyValueError::new_err("DLAMRG: invalid arguments"));
    }
    let mut index = vec![0usize; n1 + n2];
    bdsdc::dlamrg(n1, n2, av, dtrd1, dtrd2, &mut index);
    Ok(to_i64(py, &index))
}

/// `DLASCL('G', 0, 0, CFROM, CTO, M, N, A, LDA, INFO)` (only `type_ = "G"` is ported); returns
/// the new `A` storage.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn lapack_dlascl<'py>(
    py: Python<'py>,
    type_: &str,
    kl: i64,
    ku: i64,
    cfrom: f64,
    cto: f64,
    m: usize,
    n: usize,
    a: PyReadonlyArray1<'_, f64>,
    lda: usize,
) -> PyResult<F64s<'py>> {
    if !type_.eq_ignore_ascii_case("G") {
        return Err(PyValueError::new_err(format!(
            "DLASCL: only TYPE = 'G' is ported, got {type_:?}"
        )));
    }
    let _ = (kl, ku);
    let mut av = contiguous(&a)?.to_vec();
    lapack_run(py, "DLASCL", 1, || {
        bdsdc::dlascl_g(cfrom, cto, m, n, &mut av, 0, lda)
    })?
    .map_err(value_err)?;
    Ok(PyArray1::from_vec(py, av))
}

fn vect(v: &str) -> PyResult<Vect> {
    one_of("vect", v, &[("Q", Vect::Q), ("P", Vect::P)])
}

/// `DORMBR(VECT, SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, LWORK, INFO)` on flat
/// column-major storage (`lwork=None`: `DORMBR`'s own optimal workspace, handed to
/// `DORMQR`/`DORMLQ` as it is); returns `(A, C, WORK(1))` (`A` as LAPACK restores it).
#[pyfunction]
#[pyo3(signature = (vect_, side, trans_, m, n, k, a, lda, tau, c, ldc, lwork = None, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dormbr<'py>(
    py: Python<'py>,
    vect_: &str,
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
) -> PyResult<(F64s<'py>, F64s<'py>, usize)> {
    let v = vect(vect_)?;
    let sd = one_of("side", side, &[("L", Side::Left), ("R", Side::Right)])?;
    let tr: Trans = trans(trans_)?;
    let lp = lp(lapack, gemm_q)?;
    let mut av = contiguous(&a)?.to_vec();
    let tv = contiguous(&tau)?.to_vec();
    let mut cv = contiguous(&c)?.to_vec();
    let work1 = lapack_run(py, "DORMBR", n_threads, || {
        blocked::dormbr(
            lp, v, sd, tr, m, n, k, &mut av, 0, lda, &tv, &mut cv, 0, ldc, lwork,
        )
    })?
    .map_err(value_err)?;
    Ok((
        PyArray1::from_vec(py, av),
        PyArray1::from_vec(py, cv),
        work1,
    ))
}

/// `DORGBR(VECT, M, N, K, A, LDA, TAU, WORK, LWORK, INFO)` on flat column-major storage
/// (`lwork=None`: `DORGBR`'s optimal workspace); returns `(A, WORK(1))`.
#[pyfunction]
#[pyo3(signature = (vect_, m, n, k, a, lda, tau, lwork = None, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dorgbr<'py>(
    py: Python<'py>,
    vect_: &str,
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
) -> PyResult<(F64s<'py>, usize)> {
    let v = vect(vect_)?;
    let lp = lp(lapack, gemm_q)?;
    let mut av = contiguous(&a)?.to_vec();
    let tv = contiguous(&tau)?.to_vec();
    let work1 = lapack_run(py, "DORGBR", n_threads, || {
        blocked::dorgbr(lp, v, m, n, k, &mut av, 0, lda, &tv, lwork)
    })?
    .map_err(value_err)?;
    Ok((PyArray1::from_vec(py, av), work1))
}

/// `DGESDD('S', M, N, A, LDA, S, U, LDU, VT, LDVT, WORK, LWORK, IWORK, INFO)` on flat
/// column-major storage: `u` (`ldu × min(m, n)`) and `vt` (`ldvt × n`) start as given and keep
/// their entries outside the blocks LAPACK writes; `lwork=None` is the optimal workspace (numpy's
/// query). Only `jobz = "S"` is ported. Returns `(A, S, U, VT, WORK(1), INFO)` (`INFO = -4`: a
/// NaN in `A`, nothing computed). An infinite entry is refused (`ValueError`: LAPACK would loop
/// forever or return garbage).
#[pyfunction]
#[pyo3(signature = (jobz, m, n, a, lda, u, ldu, vt, ldvt, lwork = None, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
#[allow(clippy::too_many_arguments)]
fn lapack_dgesdd<'py>(
    py: Python<'py>,
    jobz: &str,
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
) -> PyResult<(F64s<'py>, F64s<'py>, F64s<'py>, F64s<'py>, usize, i64)> {
    if !jobz.eq_ignore_ascii_case("S") {
        return Err(PyValueError::new_err(format!(
            "DGESDD: only JOBZ = 'S' is ported, got {jobz:?}"
        )));
    }
    let lp = lp(lapack, gemm_q)?;
    let mut av = contiguous(&a)?.to_vec();
    let mut uv = contiguous(&u)?.to_vec();
    let mut vtv = contiguous(&vt)?.to_vec();
    let mut s = vec![0.0f64; m.min(n)];
    let out = lapack_run(py, "DGESDD", n_threads, || {
        tsvd::dgesdd_s(
            lp, m, n, &mut av, lda, &mut s, &mut uv, ldu, &mut vtv, ldvt, lwork,
        )
    })?
    .map_err(|e: DgesddError| value_err(e))?;
    Ok((
        PyArray1::from_vec(py, av),
        PyArray1::from_vec(py, s),
        PyArray1::from_vec(py, uv),
        PyArray1::from_vec(py, vtv),
        out.work1,
        out.info,
    ))
}

/// `DGESDD('S', M, N, ...)`'s workspace: `(MINWRK, MAXWRK)`, the smallest `LWORK` it accepts and
/// the optimal one (`WORK(1)` of the `LWORK = -1` query).
#[pyfunction]
fn lapack_dgesdd_lwork(m: usize, n: usize) -> (usize, usize) {
    tsvd::dgesdd_s_lwork(m, n)
}

/// numpy's `LinAlgError(msg)`.
pub(super) fn linalg_error(py: Python<'_>, msg: &str) -> PyErr {
    match py
        .import("numpy.linalg")
        .and_then(|m| m.getattr("LinAlgError"))
        .and_then(|c| c.call1((msg,)))
    {
        Ok(v) => PyErr::from_value(v),
        Err(e) => e,
    }
}

/// The values of a 2-D float64 array (any strides) in C order, as a `matmul` view of them.
fn c_order(x: &PyReadonlyArray2<'_, f64>) -> (Vec<f64>, usize, usize) {
    let arr = x.as_array();
    let (rows, cols) = arr.dim();
    (arr.iter().copied().collect(), rows, cols)
}

pub(super) fn c_view(data: &[f64], rows: usize, cols: usize) -> View<'_> {
    View {
        data,
        off: 0,
        rows,
        cols,
        s0: cols.max(1),
        s1: 1,
    }
}

pub(super) type Mat<'py> = Bound<'py, PyArray2<f64>>;

pub(super) fn mat<'py>(
    py: Python<'py>,
    rows: usize,
    cols: usize,
    v: Vec<f64>,
) -> PyResult<Mat<'py>> {
    Ok(PyArray::from_owned_array(
        py,
        Array2::from_shape_vec((rows, cols), v).map_err(value_err)?,
    ))
}

/// `numpy.linalg.svd(a, full_matrices=False)` of a 2-D float64 array (any strides), with numpy
/// 2.5.4's LAPACK (`lapack="3.12.1"`) or numpy 2.3.5's (`"3.12.0"`): `(u, s, vt)`, C-ordered.
/// Raises numpy's `LinAlgError('SVD did not converge')` as numpy does (`DGESDD`'s `INFO != 0`, a
/// non-finite entry).
#[pyfunction]
#[pyo3(signature = (a, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
fn numpy_svd<'py>(
    py: Python<'py>,
    a: PyReadonlyArray2<'_, f64>,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<(Mat<'py>, F64s<'py>, Mat<'py>)> {
    let lp = lp(lapack, gemm_q)?;
    let (data, m, n) = c_order(&a);
    let res = lapack_run(py, "numpy.linalg.svd", n_threads, || {
        tsvd::numpy_svd(lp, c_view(&data, m, n))
    })?;
    let svd = res.map_err(|e| linalg_error(py, &e.to_string()))?;
    let k = svd.k;
    Ok((
        mat(py, m, k, svd.u)?,
        PyArray1::from_vec(py, svd.s),
        mat(py, k, n, svd.vt)?,
    ))
}

/// `numpy.linalg.pinv(a, rcond)` of a 2-D float64 array (any strides; numpy's default `rcond` is
/// `1e-15`, its `rtol=None` gives `max(m, n)·2^-52`), with numpy 2.5.4's or 2.3.5's LAPACK as
/// [`numpy_svd`]: `n × m`, C-ordered. Raises numpy's `LinAlgError` as its `svd`.
#[pyfunction]
#[pyo3(signature = (a, rcond = 1e-15, lapack = "3.12.1", n_threads = 1, gemm_q = 256))]
fn numpy_pinv<'py>(
    py: Python<'py>,
    a: PyReadonlyArray2<'_, f64>,
    rcond: f64,
    lapack: &str,
    n_threads: usize,
    gemm_q: usize,
) -> PyResult<Mat<'py>> {
    let lp = lp(lapack, gemm_q)?;
    let (data, m, n) = c_order(&a);
    let res = lapack_run(py, "numpy.linalg.pinv", n_threads, || {
        tsvd::numpy_pinv(lp, c_view(&data, m, n), rcond)
    })?;
    let p = res.map_err(|e| linalg_error(py, &e.to_string()))?;
    mat(py, n, m, p)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(lapack_dbdsdc, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlasd0, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlasd1, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlasd2, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlasd3, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlasd4, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlasd5, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlaed6, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlasdq, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlasdt, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlamrg, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dlascl, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dormbr, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dorgbr, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dgesdd, m)?)?;
    m.add_function(wrap_pyfunction!(lapack_dgesdd_lwork, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_svd, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_pinv, m)?)?;
    Ok(())
}
