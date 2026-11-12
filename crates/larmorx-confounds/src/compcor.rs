// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! nipype's CompCor numerics (nipype 1.12.0 `algorithms/confounds.py`, the same in every
//! environment), bit for bit with the libraries they call in each environment (numpy 2.5.4,
//! 2.3.5 and 2.2.6's reductions, Legendre polynomials and LAPACK; SciPy 1.18.1's and 1.15's
//! `gesvd` for nipype's `fallback_svd`; the C library's `cos` of numpy's float64 `np.cos`):
//!
//! - [`cosine_drift`] (`_cosine_drift`): the DCT-II basis `sqrt(2 / T) cos((pi / T) (t + 0.5)
//!   k)`, `k = 1 .. order - 1`, and a constant last column, `order = max(floor(2 T dt / period),
//!   1)` with `dt = frametimes[1] - frametimes[0]`; each product in nipype's order (`(pi / T) ·
//!   (t + 0.5)`, then `· k`, then `nfct ·`).
//! - [`regress_poly`]: the design `[1, P1(x), ..., Pd(x)]` (`x = linspace(-1, 1, T)`, numpy's
//!   Legendre basis), `betas = pinv(X) · data.T` (numpy's `pinv`, then `np.dot`), `data - X ·
//!   betas` (or without the constant: `X[:, 1:] · betas[1:]`), in float64.
//! - [`cosine_filter`]: `X = _full_rank(_cosine_drift(period_cut, timestep · arange(T)))`, `betas
//!   = lstsq(X, data.T)[0]` (numpy's `lstsq`, `DGELSD`), `data - X · betas` (or without the
//!   constant, which is the basis' last column).
//! - [`tcompcor_mask`] (`TCompCor._process_masks` for one mask): `regress_poly(2, ...)`, each
//!   voxel's SD over time (0 and NaN taken as 0), `np.percentile(tSTD, round(100 (1 -
//!   percentile_threshold)))` (an `np.int64` rank, linear), the voxels at or above it.
//! - [`compute_noise_components`]: per mask, the rows whose float sum is NaN zeroed, the pre-filter
//!   (`"cosine"`, `"polynomial"`, or `False`: the mean removed), `M = residuals.T / tSTD` (0 and
//!   NaN SDs taken as 1), `fallback_svd(M, full_matrices=False)` (numpy's `svd`, else SciPy's
//!   `gesvd`), the explained variance `s² / sum(s²)` and its cumulative sum, the number of
//!   components (`int(criterion)`; a fraction: `searchsorted(cumvar, criterion) + 1`; `'all'`:
//!   every one), `failure_mode` (`"error"` raises; any other value fills NaN), the metadata.
//!
//! **Corrected by default** ([`UpstreamDefects::Fix`]; [`UpstreamDefects::Reproduce`] gives
//! nipype's result):
//!
//! - PIPE-44: `if num_components == 0: break` (an empty mask with `'all'`) silently skips every
//!   later mask (their components and metadata are lost, and "No components found" or, with
//!   `failure_mode='NaN'`, `ValueError: need at least one array to concatenate` may follow);
//!   larmorx goes on with the next mask, and with `failure_mode='NaN'` returns no component and
//!   empty metadata instead of the concatenation's error.
//! - PIPE-45: only rows whose float sum is NaN are zeroed: a voxel with an infinity (and no NaN)
//!   reaches the pre-filter and the SVD, which fails (`ValueError: array must not contain infs or
//!   NaNs`, or, with `failure_mode='NaN'`, every component of the mask NaN), and a finite row whose
//!   float32 sum overflows both ways (`inf - inf`) is zeroed; larmorx zeroes exactly the rows that
//!   hold a non-finite value.
//! - PIPE-52: `regress_poly`'s non-constant regressors are `X[:, :-1]`, the constant and every
//!   polynomial but the highest; larmorx returns `X[:, 1:]`, the polynomials of degree 1 to `d`.
//! - PIPE-54: a `components_criterion` of 0 or below (other than -1, nipype's `'all'`) breaks off
//!   the loop (0) or slices columns off the end (`u[:, :-2]`) with no component retained in the
//!   metadata; larmorx refuses it (`ValueError`). A count given as a float (`6.0`) fails with
//!   `TypeError` when the SVD fails with `failure_mode='NaN'`; larmorx takes its integer part.
//! - PIPE-55: an empty mask with `failure_mode='NaN'` returns `np.array([])` for the pre-filter's
//!   basis, so when it is the last mask the basis is lost; larmorx returns the basis (it does not
//!   depend on the voxels).
//! - PIPE-57: an unknown `filter_type` silently skips the filter, the mean removal included (and a
//!   float32 series is then decomposed in single precision); larmorx refuses it (`ValueError`).
//!   `reproduce` follows nipype for a float64 series and refuses a float32 one
//!   ([`CompcorError::NotPorted`]: numpy's single-precision `svd` is not ported).
//! - PIPE-59: a failed SVD with `failure_mode='NaN'` gets `T` NaN singular values whatever the
//!   decomposition's size `min(T, N)`, and with `'all'` one NaN component while all `T` values are
//!   marked retained; larmorx gives `min(T, N)` NaN singular values and as many NaN components as
//!   the criterion asks of that size (`'all'`: `min(T, N)`; a count `k`: `min(k, min(T, N))`; a
//!   fraction: 1).
//! - GLIBC-07: the cosine basis takes the correctly rounded `cos` by default; `reproduce` takes
//!   glibc 2.35's (numpy's float64 `np.cos` calls the C library's on x86-64); a notice names it
//!   when the two differ on a value of the basis.
//!
//! Upstream: nipype 1.12.0 (PyPI sdist `nipype-1.12.0.tar.gz`, SHA-256
//! `784098a8a54839836fa0eba2fa1c2040a365a272d0b28692578b5b15b475aeeb`), Copyright (c) 2009-2016,
//! Nipype developers, Apache-2.0 (`_cosine_drift` and `_full_rank` copied there from nipy 0.4.1,
//! BSD-3-Clause, Copyright (c) 2006-2024, NIPY Developers); numpy 2.5.4 `numpy/_core/_methods.py`
//! (`_var`), `numpy/lib/_function_base_impl.py` (`percentile`), `numpy/_core/fromnumeric.py`
//! (`searchsorted`, `cumsum`), Copyright (c) 2005-2025, NumPy Developers, BSD-3-Clause (the
//! notice: [`crate::nitime`], `PROVENANCE.md`, NOTICE). The logic is re-expressed in Rust; no code
//! was copied.

use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects};
use larmorx_core::math::Libm;
use larmorx_core::numpy::histograms::linspace;
use larmorx_core::numpy::polynomial::{LegvalForm, legendre_basis};
use larmorx_core::numpy::quantile::{PercentileError, Side, percentile_linear, searchsorted};
use larmorx_core::numpy::reduce::{
    SumOrder, add_reduce_axis_in, add_reduce_ordered, cumsum, std_axis_in,
};
use larmorx_core::numpy::{NpFloat, NumpyRelease};
use larmorx_core::parallel::{ThreadPoolError, with_threads};
use larmorx_transform::numpy_linalg::blocked::Lapack;
use larmorx_transform::numpy_linalg::gesvd::{
    ScipyLibrary, ScipySvdError, nipype_fallback_svd, nipype_full_rank,
};
use larmorx_transform::numpy_linalg::lstsq::LstsqError;
use larmorx_transform::numpy_linalg::svd::numpy_pinv;
use larmorx_transform::numpy_linalg::{gelsd::numpy_lstsq, svd::Svd};
use larmorx_transform::openblas::matmul::{Array2, numpy_matmul_with};
use rayon::prelude::*;

/// The libraries of one environment: numpy's reductions, Legendre polynomials and LAPACK, and
/// SciPy's for `fallback_svd`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Libraries {
    /// numpy's percentile and `searchsorted` arithmetic.
    pub numpy: NumpyRelease,
    /// numpy's summation order (numpy 2.2: [`SumOrder::Buffered`]).
    pub sum_order: SumOrder,
    /// numpy's LAPACK (`pinv`, `lstsq`, `svd`).
    pub lapack: Lapack,
    /// numpy's `legval` (numpy 2.2: [`LegvalForm::V2_2`]).
    pub legval: LegvalForm,
    /// SciPy's library for `fallback_svd`'s `gesvd`.
    pub scipy: ScipyLibrary,
}

/// nipype's `filter_type` (CompCor's `pre_filter`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterType {
    /// `"cosine"`: the discrete cosine basis ([`cosine_filter`]).
    Cosine,
    /// `"polynomial"`: Legendre polynomials of degree `degree` ([`regress_poly`]).
    Polynomial,
    /// `False`: the mean removed (`regress_poly` of degree `degree`, which CompCor sets to 0).
    MeanOnly,
    /// Any other value: nipype filters nothing (PIPE-57).
    Other,
}

/// nipype's `failure_mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureMode {
    /// `"error"`: the SVD's error, or "No components found", is raised.
    Error,
    /// Any other value (CompCor's `"NaN"`): NaN components instead.
    NaN,
}

/// nipype's `components_criterion`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Criterion {
    /// `'all'` (nipype's -1).
    All,
    /// A number: a fraction of the variance in `(0, 1)`, else a count (`int()` of it). `is_int`:
    /// whether the caller gave a Python `int` (a float count fails in nipype's NaN branch).
    Number { value: f64, is_int: bool },
}

/// A pre-filter's basis, as nipype returns it.
#[derive(Clone, Debug, PartialEq)]
pub enum Basis {
    /// `np.array([])` (a one-dimensional empty array).
    Empty,
    /// A `T × cols` float64 matrix, C order.
    Matrix { data: Vec<f64>, cols: usize },
}

/// A pre-filter's output: the `n × t` float64 residuals (C order) and the basis.
#[derive(Clone, Debug, PartialEq)]
pub struct Filtered {
    pub data: Vec<f64>,
    pub basis: Basis,
}

/// How the CompCor numerics fail, with the original's exception and message.
#[derive(Debug, thiserror::Error)]
pub enum CompcorError {
    /// `IndexError` of `_cosine_drift`'s `frametimes[1]`.
    #[error("index 1 is out of bounds for axis 0 with size {0}")]
    ShortSeries(usize),
    /// `ZeroDivisionError` of `1.0 / period_cut`.
    #[error("float division by zero")]
    ZeroPeriod,
    /// `ValueError` of `int()` of a NaN (the cosine basis' order, the criterion).
    #[error("cannot convert float NaN to integer")]
    NanToInteger,
    /// `OverflowError` of `int()` of an infinity.
    #[error("cannot convert float infinity to integer")]
    InfToInteger,
    /// numpy's `MemoryError` for a basis too large to allocate.
    #[error("Unable to allocate an array with shape ({0}, {1}) and data type float64")]
    TooLarge(usize, usize),
    /// `ValueError("Repetition time must be provided for cosine filter")`.
    #[error("Repetition time must be provided for cosine filter")]
    NoRepetitionTime,
    /// `fallback_svd`'s error (SciPy's `ValueError` or `LinAlgError`).
    #[error(transparent)]
    Svd(#[from] ScipySvdError),
    /// numpy `lstsq`'s `LinAlgError`.
    #[error("{0}")]
    Lstsq(LstsqError),
    /// numpy `pinv`'s `LinAlgError` (not reached: the design is finite).
    #[error("SVD did not converge")]
    Pinv,
    /// numpy `percentile`'s error on the voxels' SDs (`IndexError` for an empty mask).
    #[error("{0}")]
    Percentile(PercentileError),
    /// `ValueError("No components found")`.
    #[error("No components found")]
    NoComponents,
    /// `ValueError` of `np.hstack` of no metadata (`failure_mode='NaN'` with no component).
    #[error("need at least one array to concatenate")]
    Concatenate,
    /// `UnboundLocalError` of `M.shape` with no mask (`failure_mode='NaN'`).
    #[error("cannot access local variable 'M' where it is not associated with a value")]
    NoMask,
    /// `TypeError` of `np.full` with a float count (PIPE-54, `reproduce`).
    #[error("'float' object cannot be interpreted as an integer")]
    FloatCount,
    /// PIPE-54's refusal (`ValueError`).
    #[error(
        "components_criterion must be 'all', a fraction in (0, 1) or a number of components of \
         at least 1, not {0} (PIPE-54)"
    )]
    BadCriterion(f64),
    /// PIPE-57's refusal (`ValueError`).
    #[error("filter_type must be 'cosine', 'polynomial' or False (PIPE-57)")]
    BadFilter,
    /// `ValueError` of a mask whose shape is not the series' (the caller's message), raised when
    /// the loop reaches it.
    #[error("{0}")]
    ShapeMismatch(String),
    /// A path this port does not follow (`NotImplementedError`).
    #[error("{0}")]
    NotPorted(&'static str),
    #[error(transparent)]
    Threads(#[from] ThreadPoolError),
}

/// `int(x)` of a Python float (truncation), with Python's errors.
fn py_int(x: f64) -> Result<i64, CompcorError> {
    if x.is_nan() {
        return Err(CompcorError::NanToInteger);
    }
    if x.is_infinite() {
        return Err(CompcorError::InfToInteger);
    }
    Ok(x.trunc() as i64)
}

/// The largest basis this port allocates (elements): numpy's `MemoryError` beyond.
const MAX_BASIS: usize = 1 << 33;

/// nipype's `_cosine_drift(period_cut, frametimes)`: the `T × order` cosine drift matrix, C order,
/// the constant in the last column (module documentation); `libm` gives the cosine (correctly
/// rounded, or glibc's for `reproduce`) and records whether the other choice differed.
///
/// # Errors
/// [`CompcorError::ShortSeries`] for fewer than two frame times, [`CompcorError::ZeroPeriod`],
/// `int()`'s errors for a NaN or infinite order, [`CompcorError::TooLarge`].
pub fn cosine_drift(
    period_cut: f64,
    frametimes: &[f64],
    libm: &Libm,
) -> Result<(Vec<f64>, usize), CompcorError> {
    let len_tim = frametimes.len();
    if period_cut == 0.0 {
        return Err(CompcorError::ZeroPeriod);
    }
    let hfcut = 1.0 / period_cut;
    if len_tim < 2 {
        return Err(CompcorError::ShortSeries(len_tim));
    }
    let dt = NpFloat::sub(frametimes[1], frametimes[0]);
    // 2 * len_tim (a Python int) * hfcut (a Python float) * dt (np.float64), floored.
    let x = NpFloat::mul((2 * len_tim) as f64 * hfcut, dt).floor();
    let order = py_int(x)?.max(1) as usize;
    if len_tim.checked_mul(order).is_none_or(|e| e > MAX_BASIS) {
        return Err(CompcorError::TooLarge(len_tim, order));
    }
    let mut cdrift = vec![0.0f64; len_tim * order];
    let nfct = (2.0 / len_tim as f64).sqrt();
    let w = std::f64::consts::PI / len_tim as f64;
    // Columns in parallel: each value is one product chain and one cosine.
    let cols: Vec<Vec<f64>> = (1..order)
        .into_par_iter()
        .map(|k| {
            (0..len_tim)
                .map(|t| {
                    let a = NpFloat::mul(NpFloat::mul(w, NpFloat::add(t as f64, 0.5)), k as f64);
                    NpFloat::mul(nfct, libm.cos(a))
                })
                .collect()
        })
        .collect();
    for (c, col) in cols.into_iter().enumerate() {
        for (t, v) in col.into_iter().enumerate() {
            cdrift[t * order + c] = v;
        }
    }
    for t in 0..len_tim {
        cdrift[t * order + order - 1] = 1.0;
    }
    Ok((cdrift, order))
}

/// The rows as float64 (numpy's cast in `np.dot` and `lstsq`; exact).
fn as_f64<T: NpFloat>(rows: &[T]) -> Vec<f64> {
    rows.par_iter().map(|v| v.to_f64()).collect()
}

/// `data - hat.T` in place: `out[i·t + k] = f64(rows[i·t + k]) - hat[k·n + i]`, `hat` the `t × n`
/// product (C order).
fn subtract_hat(out: &mut [f64], hat: &[f64], n: usize, t: usize) {
    out.par_chunks_mut(t.max(1))
        .enumerate()
        .for_each(|(i, row)| {
            for (k, v) in row.iter_mut().enumerate() {
                *v = NpFloat::sub(*v, hat[k * n + i]);
            }
        });
}

/// nipype's `regress_poly(degree, data, remove_mean)` of the `n × t` rows (C order, float32 or
/// float64), when nipype does not return early (`data.shape[0] == 0` with a `failure_mode` other
/// than `"error"`: the caller's): the residuals (float64) and the basis (`X[:, :-1]`, or with
/// `fix_basis` (PIPE-52) `X[:, 1:]`).
///
/// # Errors
/// [`CompcorError::Pinv`] (not reached: the design is finite).
pub fn regress_poly<T: NpFloat>(
    degree: usize,
    rows: &[T],
    n: usize,
    t: usize,
    remove_mean: bool,
    fix_basis: bool,
    libs: &Libraries,
) -> Result<Filtered, CompcorError> {
    assert_eq!(rows.len(), n * t, "rows holds n × t values");
    let k = degree + 1;
    // X = [ones, P1(x), ..., Pd(x)], T × k, C order.
    let mut x = vec![1.0f64; t * k];
    if degree > 0 {
        let xs = linspace(-1.0f64, 1.0, t);
        for i in 1..=degree {
            let p = legendre_basis(i, &xs, libs.legval);
            for (r, v) in p.into_iter().enumerate() {
                x[r * k + i] = v;
            }
        }
    }
    let basis = if k > 1 {
        let (c0, cols) = if fix_basis { (1, k - 1) } else { (0, k - 1) };
        let mut b = vec![0.0f64; t * cols];
        for r in 0..t {
            b[r * cols..(r + 1) * cols].copy_from_slice(&x[r * k + c0..r * k + c0 + cols]);
        }
        Basis::Matrix { data: b, cols }
    } else {
        Basis::Empty
    };
    let xv = Array2 {
        data: &x,
        off: 0,
        rows: t,
        cols: k,
        s0: k,
        s1: 1,
    };
    // pinv(X): k × t, C order (empty for t = 0).
    let pinv = if t == 0 {
        Vec::new()
    } else {
        numpy_pinv(libs.lapack, xv, 1e-15).map_err(|_| CompcorError::Pinv)?
    };
    let mut data = as_f64(rows);
    // betas = pinv(X).dot(data.T): (k × t) · (t × n), data.T the transposed C rows.
    let betas = numpy_matmul_with(
        Array2 {
            data: &pinv,
            off: 0,
            rows: k,
            cols: t,
            s0: t,
            s1: 1,
        },
        Array2 {
            data: &data,
            off: 0,
            rows: t,
            cols: n,
            s0: 1,
            s1: t,
        },
        libs.lapack.gemm_q,
    );
    // datahat = X.dot(betas) (or X[:, 1:].dot(betas[1:])): t × n.
    let (c0, cols) = if remove_mean { (0, k) } else { (1, k - 1) };
    let hat = numpy_matmul_with(
        Array2 {
            data: &x,
            off: c0,
            rows: t,
            cols,
            s0: k,
            s1: 1,
        },
        Array2 {
            data: &betas,
            off: c0 * n,
            rows: cols,
            cols: n,
            s0: n,
            s1: 1,
        },
        libs.lapack.gemm_q,
    );
    subtract_hat(&mut data, &hat, n, t);
    Ok(Filtered { data, basis })
}

/// nipype's `cosine_filter(data, timestep, period_cut, remove_mean)` of the `n × t` rows (C
/// order), when nipype does not return early (the caller's, as [`regress_poly`]): the residuals
/// (float64) and the basis (`X[:, :-1]`, the cosines). `timestep` is the float64 value numpy
/// multiplies `arange(T)` by.
///
/// # Errors
/// [`cosine_drift`]'s, `_full_rank`'s SVD's ([`CompcorError::Svd`]), `lstsq`'s.
#[allow(clippy::too_many_arguments)]
pub fn cosine_filter<T: NpFloat>(
    rows: &[T],
    n: usize,
    t: usize,
    timestep: f64,
    period_cut: f64,
    remove_mean: bool,
    libs: &Libraries,
    libm: &Libm,
) -> Result<Filtered, CompcorError> {
    assert_eq!(rows.len(), n * t, "rows holds n × t values");
    let frametimes: Vec<f64> = (0..t).map(|i| NpFloat::mul(timestep, i as f64)).collect();
    let (drift, k) = cosine_drift(period_cut, &frametimes, libm)?;
    let fr = nipype_full_rank(
        libs.lapack,
        libs.scipy,
        Array2 {
            data: &drift,
            off: 0,
            rows: t,
            cols: k,
            s0: k,
            s1: 1,
        },
        1e15,
    )?;
    let x = fr.x;
    let basis = if k > 1 {
        let cols = k - 1;
        let mut b = vec![0.0f64; t * cols];
        for r in 0..t {
            b[r * cols..(r + 1) * cols].copy_from_slice(&x[r * k..r * k + cols]);
        }
        Basis::Matrix { data: b, cols }
    } else {
        Basis::Empty
    };
    let mut data = as_f64(rows);
    let betas = numpy_lstsq(
        libs.lapack,
        Array2 {
            data: &x,
            off: 0,
            rows: t,
            cols: k,
            s0: k,
            s1: 1,
        },
        Array2 {
            data: &data,
            off: 0,
            rows: t,
            cols: n,
            s0: 1,
            s1: t,
        },
        None,
    )
    .map_err(CompcorError::Lstsq)?
    .x;
    let cols = if remove_mean { k } else { k - 1 };
    let hat = numpy_matmul_with(
        Array2 {
            data: &x,
            off: 0,
            rows: t,
            cols,
            s0: k,
            s1: 1,
        },
        Array2 {
            data: &betas,
            off: 0,
            rows: cols,
            cols: n,
            s0: n,
            s1: 1,
        },
        libs.lapack.gemm_q,
    );
    subtract_hat(&mut data, &hat, n, t);
    Ok(Filtered { data, basis })
}

/// nipype's `_compute_tSTD` along time of the `n × t` rows (C order; nipype's `M`, `t × n`, is
/// their transpose, `axis=0`): `np.std` per voxel, 0 and NaN replaced by `x`.
pub fn compute_tstd(rows: &[f64], n: usize, t: usize, x: f64, order: SumOrder) -> Vec<f64> {
    let mut s = std_axis_in(rows, [n, t, 1], 0, order);
    for v in &mut s {
        if *v == 0.0 || v.is_nan() {
            *v = x;
        }
    }
    s
}

/// `TCompCor._process_masks` for one mask: the high-variance voxels of the `n × t` rows (C order,
/// the mask's voxels in its C order, the series' values as float64), `true` for a voxel whose SD
/// after `regress_poly(2, ...)` is at or above the `round(100 (1 - percentile_threshold))`th
/// percentile.
///
/// # Errors
/// numpy `percentile`'s `IndexError` for an empty mask ([`CompcorError::Percentile`]).
pub fn tcompcor_mask(
    rows: &[f64],
    n: usize,
    t: usize,
    percentile_threshold: f64,
    libs: &Libraries,
    n_threads: usize,
) -> Result<Vec<bool>, CompcorError> {
    with_threads(n_threads, || {
        let f = regress_poly(2, rows, n, t, true, false, libs)?;
        let tstd = compute_tstd(&f.data, n, t, 0.0, libs.sum_order);
        // np.round(100.0 * (1.0 - pt)).astype(int): round half to even.
        let q = (100.0 * (1.0 - percentile_threshold)).round_ties_even();
        let thr = percentile_linear(&tstd, &[q]).map_err(CompcorError::Percentile)?[0];
        Ok(tstd.iter().map(|&s| s >= thr).collect())
    })?
}

/// The settings of [`compute_noise_components`]: nipype's arguments and the environment's
/// libraries.
#[derive(Clone, Copy, Debug)]
pub struct NoiseOptions {
    pub criterion: Criterion,
    pub filter: FilterType,
    /// `degree` (the polynomial pre-filter's, and `False`'s: CompCor passes 0).
    pub degree: usize,
    /// `period_cut` (the cosine pre-filter's cutoff, seconds).
    pub period_cut: f64,
    /// `repetition_time` (`None`: nipype's `None`).
    pub repetition_time: Option<f64>,
    pub failure_mode: FailureMode,
    pub libs: Libraries,
    /// D13's switch.
    pub defects: UpstreamDefects,
    /// Threads (0: all logical CPUs); the result does not depend on it.
    pub n_threads: usize,
}

/// One mask's metadata (nipype's `md_*` entries): float32 when the SVD failed with
/// `failure_mode='NaN'` (`f32`).
#[derive(Clone, Debug, PartialEq)]
pub struct MaskMetadata {
    /// The mask's index in the input list.
    pub mask: usize,
    pub singular_value: Vec<f64>,
    pub variance_explained: Vec<f64>,
    pub cumulative_variance_explained: Vec<f64>,
    /// How many of the values are retained (the first ones).
    pub retained: usize,
    /// Whether the values are nipype's float32 NaN fill.
    pub f32: bool,
}

/// [`compute_noise_components`]' result.
#[derive(Clone, Debug, PartialEq)]
pub struct NoiseComponents {
    /// `t × cols`, C order.
    pub components: Vec<f64>,
    pub cols: usize,
    /// Whether nipype's `components` is float32 (every part a float32 NaN fill or empty
    /// float32).
    pub components_f32: bool,
    /// The last mask's basis.
    pub basis: Basis,
    /// One entry per mask processed, in order.
    pub metadata: Vec<MaskMetadata>,
    pub notices: Vec<Notice>,
}

/// One mask's rows: `n × t`, C order (the mask's voxels in its C order), float32 or float64.
pub struct MaskRows<T> {
    pub rows: Vec<T>,
    pub n: usize,
}

/// nipype's `compute_noise_components(imgseries, mask_images, components_criterion,
/// filter_type, degree, period_cut, repetition_time, failure_mode)` on each mask's rows of a
/// series of `t` volumes (module documentation; the masks' shapes and names are the caller's).
///
/// # Errors
/// The original's failures ([`CompcorError`]), and PIPE-54's and PIPE-57's refusals.
///
/// `mismatch`: the error of a mask after `masks` whose shape is not the series' (nipype raises it
/// when its loop reaches that mask, so not after a `break`).
pub fn compute_noise_components<T: NpFloat>(
    masks: Vec<MaskRows<T>>,
    t: usize,
    o: &NoiseOptions,
    mismatch: Option<String>,
) -> Result<NoiseComponents, CompcorError> {
    with_threads(o.n_threads, || noise(masks, t, o, mismatch))?
}

/// What one mask gives before the components are chosen: `u` (`t × ucols`), `s`, the float32
/// flags of nipype's arrays.
struct Decomposed {
    u: Vec<f64>,
    ucols: usize,
    s: Vec<f64>,
    s_f32: bool,
    u_f32: bool,
}

fn noise<T: NpFloat>(
    masks: Vec<MaskRows<T>>,
    t: usize,
    o: &NoiseOptions,
    mismatch: Option<String>,
) -> Result<NoiseComponents, CompcorError> {
    let fix = o.defects.fix();
    let mut notices = Notices::new(o.defects);
    let libm = Libm::new(o.defects);
    let is_f32 = std::mem::size_of::<T>() == 4;
    // 'all' is -1 to nipype.
    let crit = match o.criterion {
        Criterion::All => -1.0,
        Criterion::Number { value, .. } => value,
    };
    if fix && (crit <= 0.0 && crit != -1.0) {
        return Err(CompcorError::BadCriterion(crit));
    }
    if o.filter == FilterType::Other {
        if fix {
            return Err(CompcorError::BadFilter);
        }
        if is_f32 {
            return Err(CompcorError::NotPorted(
                "an unknown filter_type leaves a float32 series in single precision, whose SVD \
                 (numpy's sgesdd) is not ported",
            ));
        }
    }
    let float_count = matches!(o.criterion, Criterion::Number { is_int: false, .. });
    let mut basis = Basis::Empty;
    let mut parts: Vec<(Vec<f64>, usize, bool)> = Vec::new();
    let mut metadata = Vec::new();
    let mut last_m: Option<usize> = None; // M.shape[0] of the last mask (t)
    let mut num_components: i64 = 0;
    let (mut zeroed_diff, mut inf_rows) = (0usize, 0usize);
    let mut skipped_after: Option<usize> = None;
    let mut basis_kept = false;
    let mut nan_fill_fixed = false;
    let mut filled = false;
    let mut broke = false;
    let nmasks = masks.len();
    for (mi, mask) in masks.into_iter().enumerate() {
        let MaskRows { mut rows, n } = mask;
        assert_eq!(rows.len(), n * t, "rows holds n × t values");
        // Zero out the rows whose sum is NaN (nipype) or that hold a non-finite value (PIPE-45).
        let sums = add_reduce_axis_in(&rows, [n, t, 1], o.libs.sum_order);
        let bad: Vec<bool> = (0..n)
            .into_par_iter()
            .map(|i| {
                let nan_sum = sums[i].is_nan();
                if fix {
                    rows[i * t..(i + 1) * t]
                        .iter()
                        .any(|v| !v.to_f64().is_finite())
                } else {
                    nan_sum
                }
            })
            .collect();
        for (i, &b) in bad.iter().enumerate() {
            if b != sums[i].is_nan() {
                zeroed_diff += 1;
                if b {
                    inf_rows += 1;
                }
            }
            if b {
                for v in &mut rows[i * t..(i + 1) * t] {
                    *v = T::ZERO;
                }
            }
        }
        // The pre-filter. nipype returns the rows unchanged and no basis for an empty mask with
        // a failure_mode other than "error" (PIPE-55: larmorx computes the basis).
        let early = n == 0 && o.failure_mode != FailureMode::Error;
        if o.filter == FilterType::Cosine && o.repetition_time.is_none() {
            return Err(CompcorError::NoRepetitionTime);
        }
        let filter = |libm: &Libm| -> Result<Option<Filtered>, CompcorError> {
            Ok(match o.filter {
                FilterType::Cosine => Some(cosine_filter(
                    &rows,
                    n,
                    t,
                    o.repetition_time.unwrap_or(0.0),
                    o.period_cut,
                    true,
                    &o.libs,
                    libm,
                )?),
                FilterType::Polynomial | FilterType::MeanOnly => {
                    Some(regress_poly(o.degree, &rows, n, t, true, fix, &o.libs)?)
                }
                FilterType::Other => None,
            })
        };
        basis_kept = false;
        let (data, m_f32) = if early {
            // nipype: the rows (float32 stays float32) and np.array([]). PIPE-55: the basis
            // (nipype's early return when computing it fails: T < 2 for the cosines).
            basis = Basis::Empty;
            if fix && let Ok(Some(f)) = filter(&Libm::quiet(o.defects)) {
                basis_kept = !matches!(f.basis, Basis::Empty);
                basis = f.basis;
            }
            (Vec::new(), is_f32)
        } else {
            match filter(&libm)? {
                Some(f) => {
                    basis = f.basis;
                    (f.data, false)
                }
                None => (as_f64(&rows), is_f32),
            }
        };
        let mut m = data;
        // M / _compute_tSTD(M, 1.0).
        let sd = compute_tstd(&m, n, t, 1.0, o.libs.sum_order);
        m.par_chunks_mut(t.max(1)).enumerate().for_each(|(i, row)| {
            for v in row {
                *v = NpFloat::div(*v, sd[i]);
            }
        });
        last_m = Some(t);
        // fallback_svd(M, full_matrices=False).
        let dec = match svd_of(&m, n, t, &o.libs) {
            Ok(svd) => Decomposed {
                ucols: svd.k,
                u: svd.u,
                s: svd.s,
                s_f32: m_f32,
                u_f32: m_f32,
            },
            Err(e) => {
                if o.failure_mode == FailureMode::Error {
                    return Err(e.into());
                }
                filled = true;
                let k = t.min(n);
                nan_fill_fixed = nan_fill_fixed || k != t || matches!(o.criterion, Criterion::All);
                let (slen, ucols) = if fix {
                    let ucols = match o.criterion {
                        Criterion::All => k,
                        _ if crit >= 1.0 => (py_int(crit)? as usize).min(k),
                        _ => 1,
                    };
                    (k, ucols)
                } else if crit >= 1.0 {
                    if float_count {
                        return Err(CompcorError::FloatCount);
                    }
                    (t, crit as usize)
                } else {
                    (t, 1)
                };
                Decomposed {
                    u: vec![f64::NAN; t * ucols],
                    ucols,
                    s: vec![f64::NAN; slen],
                    s_f32: true,
                    u_f32: true,
                }
            }
        };
        drop(m);
        // variance_explained = s**2 / np.sum(s**2); cumsum.
        let sq: Vec<f64> = dec.s.iter().map(|&v| NpFloat::mul(v, v)).collect();
        let total = add_reduce_ordered(&sq, o.libs.sum_order);
        let var: Vec<f64> = sq.iter().map(|&v| NpFloat::div(v, total)).collect();
        let cum = cumsum(&var);
        let mut num = py_int(crit)?;
        if 0.0 < crit && crit < 1.0 {
            num = searchsorted(&cum, crit, Side::Left, o.libs.numpy) as i64 + 1;
        } else if crit == -1.0 {
            num = dec.s.len() as i64;
        }
        num_components = num;
        if num == 0 {
            if fix {
                if mi + 1 < nmasks {
                    skipped_after = skipped_after.or(Some(mi));
                }
                continue;
            }
            if mi + 1 < nmasks || mismatch.is_some() {
                skipped_after = Some(mi);
            }
            broke = true;
            break;
        }
        // u[:, :num] (numpy's slice of a negative stop included).
        let take = if num >= 0 {
            (num as usize).min(dec.ucols)
        } else {
            dec.ucols.saturating_sub(num.unsigned_abs() as usize)
        };
        let mut part = vec![0.0f64; t * take];
        for r in 0..t {
            part[r * take..(r + 1) * take]
                .copy_from_slice(&dec.u[r * dec.ucols..r * dec.ucols + take]);
        }
        parts.push((part, take, dec.u_f32));
        let retained = if num >= 0 {
            (num as usize).min(dec.s.len())
        } else {
            0
        };
        metadata.push(MaskMetadata {
            mask: mi,
            singular_value: dec.s,
            variance_explained: var,
            cumulative_variance_explained: cum,
            retained,
            f32: dec.s_f32,
        });
    }
    if let Some(msg) = mismatch
        && !broke
    {
        return Err(CompcorError::ShapeMismatch(msg));
    }
    if zeroed_diff > 0 {
        notices.add(
            &defects::PIPE_45,
            format!(
                "{inf_rows} voxels with an infinite value are zeroed and {} with finite values are \
                 kept (nipype zeroes only the rows whose float sum is NaN)",
                zeroed_diff - inf_rows
            ),
        );
    }
    if let Some(i) = skipped_after {
        notices.add(
            &defects::PIPE_44,
            format!(
                "mask {i} gives no component: the masks after it are {} (nipype stops there)",
                if fix { "still processed" } else { "skipped" }
            ),
        );
    }
    if fix
        && matches!(o.filter, FilterType::Polynomial | FilterType::MeanOnly)
        && o.degree > 0
        && !metadata.is_empty()
    {
        notices.add(
            &defects::PIPE_52,
            format!(
                "the pre-filter's basis holds the Legendre polynomials of degree 1 to {} (nipype's \
                 holds the constant and degrees 1 to {})",
                o.degree,
                o.degree - 1
            ),
        );
    }
    if fix && metadata.is_empty() && o.failure_mode == FailureMode::NaN && last_m.is_some() {
        notices.add(
            &defects::PIPE_44,
            "no mask gives a component: no component and empty metadata are returned (nipype \
             raises ValueError: need at least one array to concatenate)",
        );
    }
    if basis_kept {
        notices.add(
            &defects::PIPE_55,
            "the last mask is empty: the pre-filter's basis is kept (nipype returns an empty one)",
        );
    }
    if nan_fill_fixed {
        notices.add(
            &defects::PIPE_59,
            if fix {
                "a failed SVD's NaN fill has the decomposition's size min(T, N) (nipype's has T \
                 singular values, and one component for 'all')"
            } else {
                "a failed SVD's NaN fill has T singular values and, for 'all', one component (the \
                 decomposition's size is min(T, N))"
            },
        );
    }
    if float_count && crit >= 1.0 && fix && filled {
        notices.add(
            &defects::PIPE_54,
            "a float count of components is taken by its integer part in the NaN fill (nipype \
             raises TypeError)",
        );
    }
    if libm.differed() {
        notices.add(
            &defects::GLIBC_07,
            if fix {
                "the cosine basis takes the correctly rounded cos, which differs from glibc's \
                 (numpy's np.cos) in the last bit on some values"
            } else {
                "the cosine basis takes glibc's cos (numpy's np.cos), which differs from the \
                 correctly rounded value in the last bit on some values"
            },
        );
    }
    let (components, cols, components_f32) = if !parts.is_empty() {
        let cols: usize = parts.iter().map(|p| p.1).sum();
        let mut out = vec![0.0f64; t * cols];
        let mut c0 = 0;
        for (p, w, _) in &parts {
            for r in 0..t {
                out[r * cols + c0..r * cols + c0 + w].copy_from_slice(&p[r * w..(r + 1) * w]);
            }
            c0 += w;
        }
        (out, cols, parts.iter().all(|p| p.2))
    } else {
        if o.failure_mode == FailureMode::Error {
            return Err(CompcorError::NoComponents);
        }
        let Some(rows) = last_m else {
            return Err(CompcorError::NoMask);
        };
        let w = num_components.max(0) as usize;
        (vec![f64::NAN; rows * w], w, true)
    };
    if metadata.is_empty() && !fix {
        return Err(CompcorError::Concatenate);
    }
    Ok(NoiseComponents {
        components,
        cols,
        components_f32,
        basis,
        metadata,
        notices: notices.into_vec(),
    })
}

/// `fallback_svd(M, full_matrices=False)` of `M = rows.T` (`t × n`; the `n × t` rows are its
/// Fortran layout).
fn svd_of(rows: &[f64], n: usize, t: usize, libs: &Libraries) -> Result<Svd, ScipySvdError> {
    nipype_fallback_svd(
        libs.lapack,
        libs.scipy,
        Array2 {
            data: rows,
            off: 0,
            rows: t,
            cols: n,
            s0: 1,
            s1: t,
        },
    )
    .map(|(svd, _)| svd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(n: usize, t: usize, seed: u64) -> Vec<f32> {
        let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (0..n * t)
            .map(|i| {
                x = x
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let r = ((x >> 40) as f32) / (1u64 << 24) as f32 - 0.5;
                100.0 + (i % t) as f32 * 0.01 + r
            })
            .collect()
    }

    fn opts(filter: FilterType, criterion: Criterion, fm: FailureMode) -> NoiseOptions {
        NoiseOptions {
            criterion,
            filter,
            degree: 0,
            period_cut: 40.0,
            repetition_time: Some(2.0),
            failure_mode: fm,
            libs: Libraries::default(),
            defects: UpstreamDefects::Fix,
            n_threads: 1,
        }
    }

    #[test]
    fn cosine_basis_orthonormal_columns_and_constant() {
        let libm = Libm::quiet(UpstreamDefects::Fix);
        let ft: Vec<f64> = (0..50).map(|i| 2.0 * i as f64).collect();
        let (d, k) = cosine_drift(128.0, &ft, &libm).unwrap();
        assert_eq!(k, (2.0f64 * 50.0 * 2.0 / 128.0).floor() as usize);
        for c in 0..k - 1 {
            let norm: f64 = (0..50).map(|t| d[t * k + c] * d[t * k + c]).sum();
            assert!((norm - 1.0).abs() < 1e-12, "{norm}");
        }
        assert!((0..50).all(|t| d[t * k + k - 1] == 1.0));
        assert!(matches!(
            cosine_drift(128.0, &[0.0], &libm),
            Err(CompcorError::ShortSeries(1))
        ));
        assert!(matches!(
            cosine_drift(0.0, &ft, &libm),
            Err(CompcorError::ZeroPeriod)
        ));
    }

    #[test]
    fn polynomial_residuals_are_orthogonal_to_the_design() {
        let (n, t) = (7, 30);
        let rows = series(n, t, 3);
        let f = regress_poly(2, &rows, n, t, true, true, &Libraries::default()).unwrap();
        for i in 0..n {
            let m: f64 = f.data[i * t..(i + 1) * t].iter().sum::<f64>() / t as f64;
            assert!(m.abs() < 1e-9, "{m}");
        }
        let Basis::Matrix { cols, .. } = f.basis else {
            panic!("a basis");
        };
        assert_eq!(cols, 2);
    }

    #[test]
    fn threads_do_not_change_the_components() {
        let (t, n) = (40, 300);
        let masks = |s| {
            vec![
                MaskRows {
                    rows: series(n, t, s),
                    n,
                },
                MaskRows {
                    rows: series(25, t, s + 1),
                    n: 25,
                },
            ]
        };
        let mut o = opts(
            FilterType::Cosine,
            Criterion::Number {
                value: 0.5,
                is_int: false,
            },
            FailureMode::Error,
        );
        let a = compute_noise_components(masks(9), t, &o, None).unwrap();
        o.n_threads = 5;
        let b = compute_noise_components(masks(9), t, &o, None).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.metadata.len(), 2);
    }

    #[test]
    fn an_empty_first_mask_with_all_skips_nothing() {
        let t = 20;
        let masks = || {
            vec![
                MaskRows {
                    rows: Vec::<f32>::new(),
                    n: 0,
                },
                MaskRows {
                    rows: series(6, t, 1),
                    n: 6,
                },
            ]
        };
        let mut o = opts(FilterType::Cosine, Criterion::All, FailureMode::Error);
        let r = compute_noise_components(masks(), t, &o, None).unwrap();
        assert_eq!(r.cols, 6);
        assert!(r.notices.iter().any(|n| n.defect.id == "PIPE-44"));
        o.defects = UpstreamDefects::Reproduce;
        assert!(matches!(
            compute_noise_components(masks(), t, &o, None),
            Err(CompcorError::NoComponents)
        ));
    }

    #[test]
    fn an_infinite_voxel_is_zeroed() {
        let (t, n) = (20, 6);
        let mut rows = series(n, t, 2);
        rows[5] = f32::INFINITY;
        let masks = || {
            vec![MaskRows {
                rows: rows.clone(),
                n,
            }]
        };
        let mut o = opts(
            FilterType::MeanOnly,
            Criterion::Number {
                value: 3.0,
                is_int: true,
            },
            FailureMode::NaN,
        );
        let r = compute_noise_components(masks(), t, &o, None).unwrap();
        assert!(r.components.iter().all(|v| v.is_finite()));
        o.defects = UpstreamDefects::Reproduce;
        let r = compute_noise_components(masks(), t, &o, None).unwrap();
        assert!(r.components.iter().all(|v| v.is_nan()));
        assert_eq!(r.metadata[0].singular_value.len(), t);
    }
}
