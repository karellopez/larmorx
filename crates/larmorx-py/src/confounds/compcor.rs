// SPDX-License-Identifier: Apache-2.0
//! Bindings of nipype's CompCor numerics, as `larmorx_confounds::compcor` computes them:
//! `_cosine_drift` (`nipype_cosine_drift`), `regress_poly` (`nipype_regress_poly`),
//! `cosine_filter` (`nipype_cosine_filter`), `TCompCor`'s high-variance mask
//! (`nipype_tcompcor_mask`) and `compute_noise_components` (`nipype_compute_noise_components`),
//! which `larmorx.confounds` calls. The validation
//! (`validation/investigations/confounds/check_compcor.py`) compares them with each environment's
//! own nipype, numpy and SciPy. The GIL is released while computing.

use super::dvars::numpy_settings;
use super::svd::{Mat, linalg_error, mat};
use crate::defects::{PyNotice, mode_from_py, notices_to_py};
use larmorx_confounds::compcor::{
    self, Basis, CompcorError, Criterion, FailureMode, FilterType, Filtered, Libraries, MaskRows,
    NoiseOptions,
};
use larmorx_core::math::Libm;
use larmorx_core::numpy::NpFloat;
use larmorx_core::numpy::polynomial::LegvalForm;
use larmorx_core::numpy::quantile::PercentileError;
use larmorx_core::parallel::with_threads;
use larmorx_transform::numpy_linalg::gesvd::ScipyLibrary;
use numpy::{PyArray1, PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::exceptions::{
    PyIndexError, PyMemoryError, PyNotImplementedError, PyOverflowError, PyRuntimeError,
    PyTypeError, PyUnboundLocalError, PyValueError, PyZeroDivisionError,
};
use pyo3::prelude::*;

/// numpy `numpy`'s and SciPy `scipy`'s arithmetic (SciPy 1.15's library for every 1.15 release:
/// fMRIPrep 25.2.6's 1.15.2 included).
fn libraries(numpy: &str, scipy: &str) -> PyResult<Libraries> {
    let (release, sum_order, lapack) = numpy_settings(numpy)?;
    let legval = LegvalForm::from_version(numpy).ok_or_else(|| {
        PyValueError::new_err(format!("numpy must be a 2.x release, got {numpy:?}"))
    })?;
    let scipy = if scipy.starts_with("1.18") {
        ScipyLibrary::V1_18
    } else if scipy.starts_with("1.15") {
        ScipyLibrary::V1_15
    } else {
        return Err(PyValueError::new_err(format!(
            "scipy must be a 1.15 or 1.18 release, got {scipy:?}"
        )));
    };
    Ok(Libraries {
        numpy: release,
        sum_order,
        lapack,
        legval,
        scipy,
    })
}

/// The original's exception for each failure.
fn err(py: Python<'_>, e: CompcorError) -> PyErr {
    let msg = e.to_string();
    match e {
        CompcorError::ShortSeries(_) => PyIndexError::new_err(msg),
        CompcorError::ZeroPeriod => PyZeroDivisionError::new_err(msg),
        CompcorError::InfToInteger => PyOverflowError::new_err(msg),
        CompcorError::TooLarge(..) => PyMemoryError::new_err(msg),
        CompcorError::Svd(s) if s.is_linalg_error() => linalg_error(py, &msg),
        CompcorError::Lstsq(_) | CompcorError::Pinv => linalg_error(py, &msg),
        CompcorError::Percentile(PercentileError::Empty) => PyIndexError::new_err(msg),
        CompcorError::NoMask => PyUnboundLocalError::new_err(msg),
        CompcorError::FloatCount => PyTypeError::new_err(msg),
        CompcorError::NotPorted(_) => PyNotImplementedError::new_err(msg),
        CompcorError::Threads(_) => PyRuntimeError::new_err(msg),
        _ => PyValueError::new_err(msg),
    }
}

/// A 2-D float32 or float64 array's values in C order.
enum Rows {
    F32(Vec<f32>, usize, usize),
    F64(Vec<f64>, usize, usize),
}

fn rows_of(a: &Bound<'_, PyAny>) -> PyResult<Rows> {
    fn read<T: Copy + numpy::Element>(x: PyReadonlyArray2<'_, T>) -> (Vec<T>, usize, usize) {
        let (r, c) = (x.shape()[0], x.shape()[1]);
        let v = match x.as_slice() {
            Ok(s) => s.to_vec(),
            Err(_) => x.as_array().iter().copied().collect(),
        };
        (v, r, c)
    }
    if let Ok(x) = a.extract::<PyReadonlyArray2<'_, f32>>() {
        let (v, r, c) = read(x);
        return Ok(Rows::F32(v, r, c));
    }
    if let Ok(x) = a.extract::<PyReadonlyArray2<'_, f64>>() {
        let (v, r, c) = read(x);
        return Ok(Rows::F64(v, r, c));
    }
    Err(PyTypeError::new_err(
        "expected a 2-D float32 or float64 array",
    ))
}

fn basis_out<'py>(py: Python<'py>, t: usize, b: Basis) -> PyResult<Option<Mat<'py>>> {
    match b {
        Basis::Empty => Ok(None),
        Basis::Matrix { data, cols } => Ok(Some(mat(py, t, cols, data)?)),
    }
}

/// The residuals (`n × t`, float64), the basis (`None`: nipype's `np.array([])`) and the notices.
type FilterOut<'py> = (Mat<'py>, Option<Mat<'py>>, Vec<PyNotice>);

fn filtered<'py>(
    py: Python<'py>,
    n: usize,
    t: usize,
    f: Filtered,
) -> PyResult<(Mat<'py>, Option<Mat<'py>>)> {
    Ok((mat(py, n, t, f.data)?, basis_out(py, t, f.basis)?))
}

/// nipype's `_cosine_drift(period_cut, frametimes)`: the `T × order` basis (float64, the constant
/// in the last column) and the notices (GLIBC-07 when the correctly rounded `cos` and glibc's
/// differ on a value; `reproduce` takes glibc's, as numpy does).
#[pyfunction]
#[pyo3(signature = (period_cut, frametimes, upstream_defects = "fix"))]
fn nipype_cosine_drift<'py>(
    py: Python<'py>,
    period_cut: f64,
    frametimes: PyReadonlyArray1<'_, f64>,
    upstream_defects: &str,
) -> PyResult<(Mat<'py>, Vec<PyNotice>)> {
    let mode = mode_from_py(upstream_defects)?;
    let ft: Vec<f64> = frametimes.as_array().iter().copied().collect();
    let libm = Libm::new(mode);
    let (d, k) = py
        .detach(|| compcor::cosine_drift(period_cut, &ft, &libm))
        .map_err(|e| err(py, e))?;
    let notices = glibc07(mode, libm.differed());
    Ok((mat(py, ft.len(), k, d)?, notices))
}

fn glibc07(mode: larmorx_core::defects::UpstreamDefects, differed: bool) -> Vec<PyNotice> {
    let mut n = larmorx_core::defects::Notices::new(mode);
    if differed {
        n.add(
            &larmorx_core::defects::GLIBC_07,
            if mode.fix() {
                "the cosine basis takes the correctly rounded cos, which differs from glibc's \
                 (numpy's np.cos) in the last bit on some values"
            } else {
                "the cosine basis takes glibc's cos (numpy's np.cos), which differs from the \
                 correctly rounded value in the last bit on some values"
            },
        );
    }
    notices_to_py(n.into_vec())
}

/// nipype's `regress_poly(degree, data, remove_mean)` of a 2-D float32 or float64 array `data`
/// (one series per row; the early return for an empty `data` with a `failure_mode` other than
/// `"error"` is the caller's): `(residuals, basis, notices)`, the residuals float64, the basis
/// nipype's `X[:, :-1]` (PIPE-52, by default `X[:, 1:]`) or `None` (`np.array([])`).
#[pyfunction]
#[pyo3(signature = (degree, data, remove_mean = true, numpy = "2.5.4", upstream_defects = "fix", n_threads = 1, fortran = false))]
#[allow(clippy::too_many_arguments)]
fn nipype_regress_poly<'py>(
    py: Python<'py>,
    degree: usize,
    data: &Bound<'py, PyAny>,
    remove_mean: bool,
    numpy: &str,
    upstream_defects: &str,
    n_threads: usize,
    fortran: bool,
) -> PyResult<FilterOut<'py>> {
    let mode = mode_from_py(upstream_defects)?;
    let libs = libraries(numpy, "1.18.1")?;
    let rows = rows_of(data)?;
    let (n, t) = match &rows {
        Rows::F32(_, n, t) | Rows::F64(_, n, t) => (*n, *t),
    };
    let fix = mode.fix();
    let r = py.detach(|| {
        with_threads(n_threads, || match &rows {
            Rows::F32(v, ..) => {
                compcor::regress_poly_layout(degree, v, n, t, fortran, remove_mean, fix, &libs)
            }
            Rows::F64(v, ..) => {
                compcor::regress_poly_layout(degree, v, n, t, fortran, remove_mean, fix, &libs)
            }
        })
    });
    let f = r
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?
        .map_err(|e| err(py, e))?;
    let mut notices = larmorx_core::defects::Notices::new(mode);
    if degree > 0 {
        notices.add(
            &larmorx_core::defects::PIPE_52,
            if fix {
                format!(
                    "the non-constant regressors are the Legendre polynomials of degree 1 to \
                     {degree} (nipype's: the constant and degrees 1 to {})",
                    degree - 1
                )
            } else {
                format!(
                    "the non-constant regressors are the constant and the Legendre polynomials of \
                     degree 1 to {} (the polynomials are of degree 1 to {degree})",
                    degree - 1
                )
            },
        );
    }
    let (res, basis) = filtered(py, n, t, f)?;
    Ok((res, basis, notices_to_py(notices.into_vec())))
}

/// nipype's `cosine_filter(data, timestep, period_cut, remove_mean)` of a 2-D float32 or float64
/// array `data` (one series per row; the early return is the caller's, as for
/// [`nipype_regress_poly`]), with numpy `numpy`'s and SciPy `scipy`'s libraries (`_full_rank`'s
/// `fallback_svd`): `(residuals, basis, notices)`.
#[pyfunction]
#[pyo3(signature = (data, timestep, period_cut, remove_mean = true, numpy = "2.5.4", scipy = "1.18.1", upstream_defects = "fix", n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn nipype_cosine_filter<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    timestep: f64,
    period_cut: f64,
    remove_mean: bool,
    numpy: &str,
    scipy: &str,
    upstream_defects: &str,
    n_threads: usize,
) -> PyResult<FilterOut<'py>> {
    let mode = mode_from_py(upstream_defects)?;
    let libs = libraries(numpy, scipy)?;
    let rows = rows_of(data)?;
    let (n, t) = match &rows {
        Rows::F32(_, n, t) | Rows::F64(_, n, t) => (*n, *t),
    };
    let libm = Libm::new(mode);
    let r = py.detach(|| {
        with_threads(n_threads, || match &rows {
            Rows::F32(v, ..) => {
                compcor::cosine_filter(v, n, t, timestep, period_cut, remove_mean, &libs, &libm)
            }
            Rows::F64(v, ..) => {
                compcor::cosine_filter(v, n, t, timestep, period_cut, remove_mean, &libs, &libm)
            }
        })
    });
    let f = r
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?
        .map_err(|e| err(py, e))?;
    let (res, basis) = filtered(py, n, t, f)?;
    Ok((res, basis, glibc07(mode, libm.differed())))
}

/// `TCompCor._process_masks` for one mask: the high-variance voxels (`True` at or above the
/// percentile) of `rows` (2-D float32 or float64, the mask's voxels in its C order, the series'
/// values: a float32 series' rows as they are, so no float64 copy is held beside the series).
#[pyfunction]
#[pyo3(signature = (rows, percentile_threshold = 0.02, numpy = "2.5.4", n_threads = 1))]
fn nipype_tcompcor_mask<'py>(
    py: Python<'py>,
    rows: &Bound<'py, PyAny>,
    percentile_threshold: f64,
    numpy: &str,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArray1<bool>>> {
    let libs = libraries(numpy, "1.18.1")?;
    let m = match rows_of(rows)? {
        Rows::F32(v, n, t) => {
            py.detach(|| compcor::tcompcor_mask(&v, n, t, percentile_threshold, &libs, n_threads))
        }
        Rows::F64(v, n, t) => {
            py.detach(|| compcor::tcompcor_mask(&v, n, t, percentile_threshold, &libs, n_threads))
        }
    }
    .map_err(|e| err(py, e))?;
    Ok(PyArray1::from_vec(py, m))
}

/// One mask's metadata: `(mask index, singular_value, variance_explained,
/// cumulative_variance_explained, retained count, float32)`.
type MaskMd<'py> = (
    usize,
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
    usize,
    bool,
);

/// `(components (T × cols, float64), components are float32, basis or None, metadata per mask
/// processed, notices)`.
type NoiseOut<'py> = (
    Mat<'py>,
    bool,
    Option<Mat<'py>>,
    Vec<MaskMd<'py>>,
    Vec<PyNotice>,
);

fn masks_of<T: Copy + NpFloat + numpy::Element>(
    masks: &[Bound<'_, PyAny>],
    t: usize,
) -> PyResult<Vec<MaskRows<T>>> {
    masks
        .iter()
        .map(|m| {
            let x = m.extract::<PyReadonlyArray2<'_, T>>().map_err(|_| {
                PyTypeError::new_err("every mask's rows must be a 2-D array of the series' type")
            })?;
            let (n, tt) = (x.shape()[0], x.shape()[1]);
            if tt != t {
                return Err(PyValueError::new_err(format!(
                    "a mask's rows hold {tt} volumes, not {t}"
                )));
            }
            let rows = match x.as_slice() {
                Ok(s) => s.to_vec(),
                Err(_) => x.as_array().iter().copied().collect(),
            };
            Ok(MaskRows { rows, n })
        })
        .collect()
}

/// nipype's `compute_noise_components` on each mask's rows (2-D arrays, all float32 or all
/// float64, `t` columns), the criterion a number (`criterion_is_int`: a Python `int`) or `"all"`,
/// `filter_type` `"cosine"`, `"polynomial"`, `"false"` (nipype's `False`) or `"other"`,
/// `failure_mode` `"error"` or anything else (NaN), with numpy `numpy`'s and SciPy `scipy`'s
/// libraries and D13's switch (`larmorx_confounds::compcor`'s documentation); `mismatch`: the
/// `ValueError` text of a mask after these whose shape is not the series'.
#[pyfunction]
#[pyo3(signature = (masks, t, criterion, criterion_is_int, filter_type, degree, period_cut, repetition_time, failure_mode, numpy = "2.5.4", scipy = "1.18.1", upstream_defects = "fix", n_threads = 1, mismatch = None))]
#[allow(clippy::too_many_arguments)]
fn nipype_compute_noise_components<'py>(
    py: Python<'py>,
    masks: Vec<Bound<'py, PyAny>>,
    t: usize,
    criterion: &Bound<'py, PyAny>,
    criterion_is_int: bool,
    filter_type: &str,
    degree: usize,
    period_cut: f64,
    repetition_time: Option<f64>,
    failure_mode: &str,
    numpy: &str,
    scipy: &str,
    upstream_defects: &str,
    n_threads: usize,
    mismatch: Option<String>,
) -> PyResult<NoiseOut<'py>> {
    let criterion = match criterion.extract::<String>() {
        Ok(s) if s == "all" => Criterion::All,
        Ok(s) => return Err(PyValueError::new_err(format!("criterion {s:?}"))),
        Err(_) => Criterion::Number {
            value: criterion.extract::<f64>()?,
            is_int: criterion_is_int,
        },
    };
    let filter = match filter_type {
        "cosine" => FilterType::Cosine,
        "polynomial" => FilterType::Polynomial,
        "false" => FilterType::MeanOnly,
        _ => FilterType::Other,
    };
    let o = NoiseOptions {
        criterion,
        filter,
        degree,
        period_cut,
        repetition_time,
        failure_mode: if failure_mode == "error" {
            FailureMode::Error
        } else {
            FailureMode::NaN
        },
        libs: libraries(numpy, scipy)?,
        defects: mode_from_py(upstream_defects)?,
        n_threads,
    };
    let f32_series = masks
        .first()
        .is_some_and(|m| m.extract::<PyReadonlyArray2<'_, f32>>().is_ok());
    let r = if f32_series {
        let m = masks_of::<f32>(&masks, t)?;
        py.detach(|| compcor::compute_noise_components(m, t, &o, mismatch))
    } else {
        let m = masks_of::<f64>(&masks, t)?;
        py.detach(|| compcor::compute_noise_components(m, t, &o, mismatch))
    };
    let r = r.map_err(|e| err(py, e))?;
    let md = r
        .metadata
        .into_iter()
        .map(|m| {
            (
                m.mask,
                PyArray1::from_vec(py, m.singular_value),
                PyArray1::from_vec(py, m.variance_explained),
                PyArray1::from_vec(py, m.cumulative_variance_explained),
                m.retained,
                m.f32,
            )
        })
        .collect();
    Ok((
        mat(py, t, r.cols, r.components)?,
        r.components_f32,
        basis_out(py, t, r.basis)?,
        md,
        notices_to_py(r.notices),
    ))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(nipype_cosine_drift, m)?)?;
    m.add_function(wrap_pyfunction!(nipype_regress_poly, m)?)?;
    m.add_function(wrap_pyfunction!(nipype_cosine_filter, m)?)?;
    m.add_function(wrap_pyfunction!(nipype_tcompcor_mask, m)?)?;
    m.add_function(wrap_pyfunction!(nipype_compute_noise_components, m)?)?;
    Ok(())
}
