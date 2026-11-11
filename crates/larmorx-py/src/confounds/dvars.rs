// SPDX-License-Identifier: Apache-2.0
//! Bindings of nipype's DVARS and the numerics under it, as `larmorx_confounds::{nitime, dvars}`
//! compute them: numpy's complex64 product (`numpy_cmul64`), nitime's `crosscov` /
//! `autocorr` (`nitime_crosscov`) and `AR_est_YW(x, 1)` (`nitime_ar_est_yw`), SciPy's 1 × 1
//! `linalg.solve` (`scipy_solve_1x1`) and nipype's `compute_dvars` on the masked series
//! (`nipype_compute_dvars`, which `larmorx.confounds.compute_dvars` calls). The validation
//! (`validation/investigations/confounds/check_dvars.py`) compares them with each environment's
//! own numpy, SciPy, nitime and nipype.

use super::svd::linalg_error;
use crate::defects::{PyNotice, mode_from_py, notices_to_py};
use larmorx_confounds::dvars::{self, DvarsError, DvarsOptions};
use larmorx_confounds::nitime::{self, CrosscovPlan, SolveError, SolveRelease};
use larmorx_confounds::scipy_fft::{C32, FftError, FftLibrary};
use larmorx_core::numpy::NumpyRelease;
use larmorx_core::numpy::reduce::SumOrder;
use larmorx_transform::numpy_linalg::LapackRelease;
use larmorx_transform::numpy_linalg::blocked::Lapack;
use numpy::{PyArray1, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::{
    PyIndexError, PyNotImplementedError, PyRuntimeError, PyRuntimeWarning, PyValueError,
};
use pyo3::prelude::*;

fn scipy_release(v: &str) -> PyResult<(FftLibrary, SolveRelease)> {
    match (
        FftLibrary::from_scipy_version(v),
        SolveRelease::from_scipy_version(v),
    ) {
        (Some(f), Some(s)) => Ok((f, s)),
        _ => Err(PyValueError::new_err(format!(
            "scipy must be a 1.15 or 1.18 release, got {v:?}"
        ))),
    }
}

/// numpy `version`'s percentile arithmetic, summation order and LAPACK.
fn numpy_settings(v: &str) -> PyResult<(NumpyRelease, SumOrder, Lapack)> {
    let bad = || PyValueError::new_err(format!("numpy must be a 2.x release, got {v:?}"));
    let release = NumpyRelease::from_version(v).ok_or_else(bad)?;
    let order = SumOrder::from_version(v).ok_or_else(bad)?;
    let lapack = match release {
        NumpyRelease::V2_5 => LapackRelease::V3_12_1,
        NumpyRelease::V2_3 => LapackRelease::V3_12_0,
    };
    Ok((release, order, Lapack::new(lapack)))
}

fn fft_err(e: FftError) -> PyErr {
    match e {
        FftError::InvalidLength(_) => PyValueError::new_err(e.to_string()),
        _ => PyNotImplementedError::new_err(e.to_string()),
    }
}

fn solve_err(py: Python<'_>, e: SolveError) -> PyErr {
    match e {
        SolveError::Singular => linalg_error(py, &e.to_string()),
        SolveError::NotFinite | SolveError::RowsMismatch => PyValueError::new_err(e.to_string()),
    }
}

/// numpy's complex64 product `a * b` element by element (two one-dimensional complex64 arrays
/// of one length), as numpy computes it on x86-64 with AVX2 and FMA3.
#[pyfunction]
fn numpy_cmul64<'py>(
    py: Python<'py>,
    a: PyReadonlyArray1<'py, C32>,
    b: PyReadonlyArray1<'py, C32>,
) -> PyResult<Bound<'py, PyArray1<C32>>> {
    let mut x: Vec<C32> = a.as_array().iter().copied().collect();
    let y: Vec<C32> = b.as_array().iter().copied().collect();
    if x.len() != y.len() {
        return Err(PyValueError::new_err("the arrays' lengths differ"));
    }
    nitime::cmul_inplace(&mut x, &y);
    Ok(PyArray1::from_vec(py, x))
}

/// nitime's `crosscov(x, y, debias=False, all_lags=all_lags)` of two float32 series of one
/// length (`autocorr(x)` is `crosscov(x, x)`) with SciPy `scipy`'s FFTs: a float32 array of the
/// lags `0 .. n` (`all_lags`: the `2 n - 1` lags `-(n - 1) .. n`).
#[pyfunction]
#[pyo3(signature = (x, y, all_lags = false, scipy = "1.18.1"))]
fn nitime_crosscov<'py>(
    py: Python<'py>,
    x: PyReadonlyArray1<'py, f32>,
    y: PyReadonlyArray1<'py, f32>,
    all_lags: bool,
    scipy: &str,
) -> PyResult<Bound<'py, PyArray1<f32>>> {
    let (lib, _) = scipy_release(scipy)?;
    let x: Vec<f32> = x.as_array().iter().copied().collect();
    let y: Vec<f32> = y.as_array().iter().copied().collect();
    if x.len() != y.len() {
        return Err(PyValueError::new_err(
            "crosscov() only works on same-length sequences for now",
        ));
    }
    let out = py.detach(|| -> Result<Vec<f32>, FftError> {
        let plan = CrosscovPlan::new(lib, x.len())?;
        Ok(if all_lags {
            plan.crosscov_all_lags(&x, &y)
        } else {
            plan.crosscov(&x, &y)
        })
    });
    Ok(PyArray1::from_vec(py, out.map_err(fft_err)?))
}

/// SciPy `scipy`'s `linalg.solve([[a]], [b])` of float32 values: the float32 solution, or SciPy's
/// error (`ValueError`, numpy's `LinAlgError`).
#[pyfunction]
#[pyo3(signature = (a, b, scipy = "1.18.1"))]
fn scipy_solve_1x1(py: Python<'_>, a: f32, b: f32, scipy: &str) -> PyResult<f32> {
    let (_, rel) = scipy_release(scipy)?;
    nitime::scipy_solve_1x1(rel, a, b).map_err(|e| solve_err(py, e))
}

/// nitime's `AR_est_YW(x, 1)` of a float32 series with SciPy `scipy`'s FFTs and solve: `(ak,
/// sigma_v)`, `ak` a float32 array of one value (none for a series of one value with SciPy
/// 1.18), or SciPy's error.
#[pyfunction]
#[pyo3(signature = (x, scipy = "1.18.1"))]
fn nitime_ar_est_yw<'py>(
    py: Python<'py>,
    x: PyReadonlyArray1<'py, f32>,
    scipy: &str,
) -> PyResult<(Bound<'py, PyArray1<f32>>, f32)> {
    let (lib, rel) = scipy_release(scipy)?;
    let x: Vec<f32> = x.as_array().iter().copied().collect();
    let plan = CrosscovPlan::new(lib, x.len()).map_err(fft_err)?;
    let r = py
        .detach(|| nitime::ar_est_yw1(&plan, rel, &x))
        .map_err(|e| solve_err(py, e))?;
    Ok((
        PyArray1::from_vec(py, r.ak.into_iter().collect()),
        r.sigma_v,
    ))
}

/// The three DVARS series, the AR(1) coefficients, the voxels used and the notices.
type DvarsOut<'py> = (
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f32>>,
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f32>>,
    usize,
    Vec<PyNotice>,
);

/// nipype's `compute_dvars` on `mfunc`, the masked series (a 2-D float32 array, one row per
/// voxel in the mask's C order), with numpy `numpy`'s and SciPy `scipy`'s arithmetic and D13's
/// switch: `(dvars_stdz, dvars_nstd, dvars_vx_stdz, ar1, voxels, notices)`, or nipype's error
/// (`IndexError`, `ValueError`, numpy's `LinAlgError`, the voxel-wise standardisation's
/// `RuntimeWarning` raised as an exception, `NotImplementedError` for a series SciPy 1.18's
/// FFTs would compute by Bailey's algorithm). `ValueError`'s and `LinAlgError`'s that SCI-41's
/// correction raises come with the notice as the last item of the exception's arguments.
#[pyfunction]
#[pyo3(signature = (mfunc, remove_zerovariance = false, variance_tol = 0.0, intensity_normalization = 1000.0, numpy = "2.5.4", scipy = "1.18.1", upstream_defects = "fix", n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn nipype_compute_dvars<'py>(
    py: Python<'py>,
    mfunc: PyReadonlyArray2<'py, f32>,
    remove_zerovariance: bool,
    variance_tol: f64,
    intensity_normalization: f64,
    numpy: &str,
    scipy: &str,
    upstream_defects: &str,
    n_threads: usize,
) -> PyResult<DvarsOut<'py>> {
    let (fft, solve) = scipy_release(scipy)?;
    let (release, sum_order, lapack) = numpy_settings(numpy)?;
    let o = DvarsOptions {
        remove_zerovariance,
        variance_tol,
        intensity_normalization,
        numpy: release,
        sum_order,
        lapack,
        fft,
        solve,
        defects: mode_from_py(upstream_defects)?,
        n_threads,
    };
    let view = mfunc.as_array();
    let (n, t) = view.dim();
    let data: Vec<f32> = match mfunc.as_slice() {
        Ok(s) => s.to_vec(),
        Err(_) => view.iter().copied().collect(),
    };
    let r = py.detach(|| dvars::compute_dvars(data, n, t, &o));
    match r {
        Ok(d) => Ok((
            PyArray1::from_vec(py, d.std),
            PyArray1::from_vec(py, d.nstd),
            PyArray1::from_vec(py, d.vxstd),
            PyArray1::from_vec(py, d.ar1),
            d.voxels,
            notices_to_py(d.notices),
        )),
        Err(e) => Err(match e {
            DvarsError::EmptySeries => PyIndexError::new_err(e.to_string()),
            DvarsError::NoVoxels | DvarsError::Broadcast(_) => PyValueError::new_err(e.to_string()),
            DvarsError::Solve { error, sci41, .. } => {
                let err = solve_err(py, error);
                if sci41 {
                    let _ = err.value(py).setattr("larmorx_sci41", true);
                }
                err
            }
            DvarsError::FloatingPoint(msg) => PyRuntimeWarning::new_err(msg),
            DvarsError::Fft(f) => fft_err(f),
            DvarsError::Pinv => linalg_error(py, "SVD did not converge"),
            DvarsError::Threads(t) => PyRuntimeError::new_err(t.to_string()),
        }),
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(numpy_cmul64, m)?)?;
    m.add_function(wrap_pyfunction!(nitime_crosscov, m)?)?;
    m.add_function(wrap_pyfunction!(scipy_solve_1x1, m)?)?;
    m.add_function(wrap_pyfunction!(nitime_ar_est_yw, m)?)?;
    m.add_function(wrap_pyfunction!(nipype_compute_dvars, m)?)?;
    Ok(())
}
