// SPDX-License-Identifier: Apache-2.0
//! Bindings for `larmorx_sdcflows` (sdcflows' field-map estimation, ported): the B-spline
//! control grid and the collocation matrix of one or more control grids on a sampling grid;
//! numpy's float32 dot product (OpenBLAS's Haswell `sdot`), SciPy's `cg` and scikit-learn's
//! `sparse_cg` ridge solver. Matrices are float64 4×4 arrays; the GIL is released while
//! computing.

use larmorx_core::linalg::Mat4;
use larmorx_sdcflows::SdcError;
use larmorx_sdcflows::approx::{self, ApproxParams, Fitting, Grid, LogRecord, Recenter};
use larmorx_sdcflows::blas;
use larmorx_sdcflows::bspline::{self, Collocation, Csr, WeightType};
use larmorx_sdcflows::ridge::{self, CsrOperator, RidgeFit, RidgeSystem};
use larmorx_sdcflows::tools;
use larmorx_sdcflows::unwarp::{self, ReadoutTime, UnwarpOptions};
use numpy::{PyArray1, PyArray2, PyArray3, PyReadonlyArray1, PyReadonlyArray2, PyReadonlyArray3};
use numpy::{PyReadonlyArrayDyn, PyUntypedArrayMethods};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

fn sdc_err(e: SdcError) -> PyErr {
    match e {
        SdcError::Threads(m) | SdcError::Runtime(m) => PyRuntimeError::new_err(m),
        // numpy.linalg.LinAlgError is a ValueError subclass; the wrapper re-raises it as one.
        other => PyValueError::new_err(other.to_string()),
    }
}

fn mat4(a: &PyReadonlyArray2<'_, f64>) -> PyResult<Mat4> {
    let a = a.as_array();
    if a.shape() != [4, 4] {
        return Err(PyValueError::new_err("affines must be 4x4"));
    }
    Ok(std::array::from_fn(|i| std::array::from_fn(|j| a[[i, j]])))
}

fn dtype_from_py(dtype: &str) -> PyResult<WeightType> {
    match dtype {
        "float32" => Ok(WeightType::Float32),
        "float64" => Ok(WeightType::Float64),
        other => Err(PyValueError::new_err(format!(
            "the weights are ported for float32 and float64, not {other:?}"
        ))),
    }
}

/// sdcflows' `bspline_grid`: the control grid's shape and affine for an image of `shape` and
/// `affine` whose header zooms are `zooms`.
#[pyfunction]
fn sdcflows_bspline_grid<'py>(
    py: Python<'py>,
    shape: [usize; 3],
    affine: PyReadonlyArray2<'_, f64>,
    zooms: Vec<f64>,
    control_zooms_mm: [f64; 3],
) -> PyResult<([usize; 3], Bound<'py, PyArray2<f64>>)> {
    let m = mat4(&affine)?;
    let g = bspline::bspline_grid(shape, &m, &zooms, control_zooms_mm).map_err(sdc_err)?;
    let a: Vec<Vec<f64>> = g.affine.iter().map(|r| r.to_vec()).collect();
    Ok((g.shape, PyArray2::from_vec2(py, &a)?))
}

fn collocation(
    target_shape: [usize; 3],
    target_affine: &Mat4,
    levels: &[([usize; 3], Mat4)],
    dtype: WeightType,
) -> Result<(Collocation, Vec<bool>), SdcError> {
    let mut weights = Vec::with_capacity(levels.len());
    for (shape, affine) in levels {
        weights.push(bspline::grid_bspline_weights(
            target_shape,
            target_affine,
            *shape,
            affine,
            dtype,
        )?);
    }
    let aligned = weights.iter().map(|w| w.aligned).collect();
    Ok((Collocation::new(weights)?, aligned))
}

fn levels_from_py(
    levels: Vec<([usize; 3], PyReadonlyArray2<'_, f64>)>,
) -> PyResult<Vec<([usize; 3], Mat4)>> {
    levels.iter().map(|(s, a)| Ok((*s, mat4(a)?))).collect()
}

/// The stored collocation matrix of `levels` (`[(shape, affine)]`) on the sampling grid, as
/// sdcflows builds it (`sparse_hstack([grid_bspline_weights(target, l) for l in levels])
/// .tocsr()`): `(data, indices, indptr, (rows, cols), aligned)`, data float64 holding the values
/// of the matrix type, indices and indptr int64, `aligned` one flag per level.
#[pyfunction]
#[pyo3(signature = (target_shape, target_affine, levels, dtype = "float32", n_threads = 1))]
#[allow(clippy::type_complexity)]
fn sdcflows_collocation_csr<'py>(
    py: Python<'py>,
    target_shape: [usize; 3],
    target_affine: PyReadonlyArray2<'_, f64>,
    levels: Vec<([usize; 3], PyReadonlyArray2<'_, f64>)>,
    dtype: &str,
    n_threads: usize,
) -> PyResult<(
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<i64>>,
    Bound<'py, PyArray1<i64>>,
    (usize, usize),
    Vec<bool>,
)> {
    let t = mat4(&target_affine)?;
    let levels = levels_from_py(levels)?;
    let dtype = dtype_from_py(dtype)?;
    let (csr, aligned) = py
        .detach(|| -> Result<_, SdcError> {
            let (col, aligned) = collocation(target_shape, &t, &levels, dtype)?;
            Ok((col.to_csr(n_threads)?, aligned))
        })
        .map_err(sdc_err)?;
    let to_i64 = |v: Vec<usize>| v.into_iter().map(|x| x as i64).collect::<Vec<i64>>();
    Ok((
        PyArray1::from_vec(py, csr.data),
        PyArray1::from_vec(py, to_i64(csr.indices)),
        PyArray1::from_vec(py, to_i64(csr.indptr)),
        (csr.shape[0], csr.shape[1]),
        aligned,
    ))
}

/// `X @ v` (`transpose = False`, `v` of the columns' length) or `X.T @ v` (`transpose = True`)
/// for sdcflows' float32 collocation matrix of `levels` on the sampling grid, in scipy's
/// summation order, without storing the matrix. `rows` (int64) restricts `X @ v` to those rows.
#[pyfunction]
#[pyo3(signature = (target_shape, target_affine, levels, v, transpose = false, rows = None, n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn sdcflows_collocation_apply<'py>(
    py: Python<'py>,
    target_shape: [usize; 3],
    target_affine: PyReadonlyArray2<'_, f64>,
    levels: Vec<([usize; 3], PyReadonlyArray2<'_, f64>)>,
    v: PyReadonlyArray1<'_, f32>,
    transpose: bool,
    rows: Option<PyReadonlyArray1<'_, i64>>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArray1<f32>>> {
    let t = mat4(&target_affine)?;
    let levels = levels_from_py(levels)?;
    let v = v.as_array().to_vec();
    let rows: Option<Vec<usize>> = match rows {
        None => None,
        Some(r) => Some(
            r.as_array()
                .iter()
                .map(|&i| usize::try_from(i).map_err(|_| PyValueError::new_err("negative row")))
                .collect::<PyResult<_>>()?,
        ),
    };
    let out = py
        .detach(|| -> Result<Vec<f32>, SdcError> {
            let (col, _) = collocation(target_shape, &t, &levels, WeightType::Float32)?;
            if let Some(r) = &rows
                && r.iter().any(|&i| i >= col.shape()[0])
            {
                return Err(SdcError::Invalid("row index out of range".into()));
            }
            if transpose {
                col.rmatvec_f32(&v, n_threads)
            } else {
                col.matvec_f32(&v, rows.as_deref(), n_threads)
            }
        })
        .map_err(sdc_err)?;
    Ok(PyArray1::from_vec(py, out))
}

/// `np.dot(x, y)` of two float32 vectors (numpy's `FLOAT_dot` over OpenBLAS's Haswell `sdot`).
#[pyfunction]
fn sdcflows_dot_f32(x: PyReadonlyArray1<'_, f32>, y: PyReadonlyArray1<'_, f32>) -> PyResult<f32> {
    let (x, y) = (x.as_array(), y.as_array());
    if x.len() != y.len() {
        return Err(PyValueError::new_err("shapes not aligned"));
    }
    let (x, y): (Vec<f32>, Vec<f32>) = (x.to_vec(), y.to_vec());
    Ok(blas::dot_f32(&x, &y))
}

fn csr_from_py(
    data: PyReadonlyArray1<'_, f32>,
    indices: PyReadonlyArray1<'_, i64>,
    indptr: PyReadonlyArray1<'_, i64>,
    shape: (usize, usize),
) -> PyResult<Csr<f32>> {
    let idx = |a: PyReadonlyArray1<'_, i64>| -> PyResult<Vec<usize>> {
        a.as_array()
            .iter()
            .map(|&i| usize::try_from(i).map_err(|_| PyValueError::new_err("negative index")))
            .collect()
    };
    Ok(Csr {
        shape: [shape.0, shape.1],
        indptr: idx(indptr)?,
        indices: idx(indices)?,
        data: data.as_array().to_vec(),
    })
}

fn system_name(s: RidgeSystem) -> &'static str {
    match s {
        RidgeSystem::Normal => "normal",
        RidgeSystem::Kernel => "kernel",
    }
}

type FitOut<'py> = (Bound<'py, PyArray1<f32>>, usize, usize, &'static str);

fn fit_out<'py>(py: Python<'py>, fit: RidgeFit) -> FitOut<'py> {
    (
        PyArray1::from_vec(py, fit.coef),
        fit.info,
        fit.iterations,
        system_name(fit.system),
    )
}

/// SciPy's `cg(A, b, rtol=rtol, atol=atol, maxiter=maxiter)` for a float32 CSR matrix `A`:
/// `(x, info, iterations)`.
#[pyfunction]
#[pyo3(signature = (data, indices, indptr, shape, b, rtol = 1e-5, atol = 0.0, maxiter = None))]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sdcflows_cg_csr<'py>(
    py: Python<'py>,
    data: PyReadonlyArray1<'_, f32>,
    indices: PyReadonlyArray1<'_, i64>,
    indptr: PyReadonlyArray1<'_, i64>,
    shape: (usize, usize),
    b: PyReadonlyArray1<'_, f32>,
    rtol: f64,
    atol: f64,
    maxiter: Option<usize>,
) -> PyResult<(Bound<'py, PyArray1<f32>>, usize, usize)> {
    let csr = csr_from_py(data, indices, indptr, shape)?;
    if shape.0 != shape.1 {
        return Err(PyValueError::new_err(format!(
            "expected square matrix, but got shape=({:?},)",
            shape
        )));
    }
    let b = b.as_array().to_vec();
    let res = py
        .detach(|| -> Result<_, SdcError> {
            let op = CsrOperator::new(csr)?;
            use ridge::Operator;
            ridge::cg_f32(|v| op.matvec(v, 1), &b, rtol, atol, maxiter)
        })
        .map_err(sdc_err)?;
    Ok((PyArray1::from_vec(py, res.x), res.info, res.iterations))
}

/// scikit-learn's `Ridge(alpha, fit_intercept=False, tol=tol, max_iter=max_iter).fit(X, y)
/// .coef_` for a float32 CSR matrix `X` (the `sparse_cg` solver): `(coef, info, iterations,
/// system)`, system "normal" or "kernel".
#[pyfunction]
#[pyo3(signature = (data, indices, indptr, shape, y, alpha = 1.0, tol = 1e-4, max_iter = None, n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn sdcflows_ridge_csr<'py>(
    py: Python<'py>,
    data: PyReadonlyArray1<'_, f32>,
    indices: PyReadonlyArray1<'_, i64>,
    indptr: PyReadonlyArray1<'_, i64>,
    shape: (usize, usize),
    y: PyReadonlyArray1<'_, f32>,
    alpha: f64,
    tol: f64,
    max_iter: Option<usize>,
    n_threads: usize,
) -> PyResult<FitOut<'py>> {
    let csr = csr_from_py(data, indices, indptr, shape)?;
    let y = y.as_array().to_vec();
    let fit = py
        .detach(|| -> Result<_, SdcError> {
            let op = CsrOperator::new(csr)?;
            ridge::ridge_sparse_cg(&op, &y, alpha, tol, max_iter, n_threads)
        })
        .map_err(sdc_err)?;
    Ok(fit_out(py, fit))
}

/// The same fit on sdcflows' float32 collocation matrix of `levels` on the sampling grid
/// (`Ridge(alpha, fit_intercept=False).fit(colmat, y)`), applied without storing the matrix.
#[pyfunction]
#[pyo3(signature = (target_shape, target_affine, levels, y, alpha = 1e-4, tol = 1e-4, max_iter = None, n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn sdcflows_ridge_collocation<'py>(
    py: Python<'py>,
    target_shape: [usize; 3],
    target_affine: PyReadonlyArray2<'_, f64>,
    levels: Vec<([usize; 3], PyReadonlyArray2<'_, f64>)>,
    y: PyReadonlyArray1<'_, f32>,
    alpha: f64,
    tol: f64,
    max_iter: Option<usize>,
    n_threads: usize,
) -> PyResult<FitOut<'py>> {
    let t = mat4(&target_affine)?;
    let levels = levels_from_py(levels)?;
    let y = y.as_array().to_vec();
    let fit = py
        .detach(|| -> Result<_, SdcError> {
            let (col, _) = collocation(target_shape, &t, &levels, WeightType::Float32)?;
            ridge::ridge_sparse_cg(&col, &y, alpha, tol, max_iter, n_threads)
        })
        .map_err(sdc_err)?;
    Ok(fit_out(py, fit))
}

/// A grid's data, shape, affine and mask, owned.
type OwnedGrid = (Vec<f32>, [usize; 3], Mat4, Option<Vec<bool>>);

type GridIn<'a> = (
    PyReadonlyArray1<'a, f32>,
    [usize; 3],
    PyReadonlyArray2<'a, f64>,
    Option<PyReadonlyArray1<'a, bool>>,
);

/// sdcflows' `BSplineApprox` on a loaded field map: `data` (float32, C order, flat) on the grid
/// `shape` / `affine` with header `zooms`, `mask` (bool, flat; None: every voxel), `spacings`
/// (`bs_spacing`), `recenter` ("median", "mode", "mean" or "none"); `resampled` = `(data,
/// shape, affine, mask)` of the grid `resample_to_zooms` gave when sdcflows resizes. A dict
/// with `grids` ([(shape, affine)]), `coefficients`, `center`, `n_inside`, `field`, `error`,
/// `extrapolated` (None without `extrapolate`), `extreme`, `info`, `n_iter`, `system`, `log`
/// ([(level, message)]).
#[pyfunction]
#[pyo3(signature = (data, shape, affine, zooms, spacings, mask = None, ridge_alpha = 1e-4, recenter = "median", extrapolate = true, name = "", resampled = None, n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn sdcflows_bspline_approx<'py>(
    py: Python<'py>,
    data: PyReadonlyArray1<'_, f32>,
    shape: [usize; 3],
    affine: PyReadonlyArray2<'_, f64>,
    zooms: Vec<f64>,
    spacings: Vec<[f64; 3]>,
    mask: Option<PyReadonlyArray1<'_, bool>>,
    ridge_alpha: f64,
    recenter: &str,
    extrapolate: bool,
    name: &str,
    resampled: Option<GridIn<'_>>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyDict>> {
    let recenter = match recenter {
        "median" => Recenter::Median,
        "mode" => Recenter::Mode,
        "mean" => Recenter::Mean,
        "none" => Recenter::None,
        other => {
            return Err(PyValueError::new_err(format!(
                "recenter must be 'median', 'mode', 'mean' or 'none', not {other:?}"
            )));
        }
    };
    let aff = mat4(&affine)?;
    let data = data.as_array().to_vec();
    let mask: Option<Vec<bool>> = mask.map(|m| m.as_array().to_vec());
    let res: Option<OwnedGrid> = match resampled {
        None => None,
        Some((d, s, a, m)) => Some((
            d.as_array().to_vec(),
            s,
            mat4(&a)?,
            m.map(|m| m.as_array().to_vec()),
        )),
    };
    let params = ApproxParams {
        zooms,
        spacings,
        ridge_alpha,
        recenter,
        extrapolate,
        name: name.to_string(),
    };
    let out = py.detach(|| {
        let original = Grid {
            data: &data,
            shape,
            affine: &aff,
            mask: mask.as_deref(),
        };
        let fitting = match &res {
            None => Fitting::Original,
            Some((d, s, a, m)) => Fitting::Resampled(Grid {
                data: d,
                shape: *s,
                affine: a,
                mask: m.as_deref(),
            }),
        };
        let mut log = Vec::new();
        let r = approx::bspline_approx_logged(original, fitting, &params, n_threads, &mut log);
        (r, log)
    });
    let (out, log) = out;
    let log: Vec<(&str, String)> = log
        .into_iter()
        .map(|r| match r {
            LogRecord::Info(m) => ("info", m),
            LogRecord::Debug(m) => ("debug", m),
        })
        .collect();
    let out = match out {
        Ok(o) => o,
        Err(e) => {
            // the records sdcflows wrote before it raised, on the exception (`err.log`)
            let err = sdc_err(e);
            err.value(py).setattr("log", log)?;
            return Err(err);
        }
    };
    let d = PyDict::new(py);
    let grids: Vec<([usize; 3], Bound<'py, PyArray2<f64>>)> = out
        .grids
        .iter()
        .map(|g| {
            let a: Vec<Vec<f64>> = g.affine.iter().map(|r| r.to_vec()).collect();
            Ok((g.shape, PyArray2::from_vec2(py, &a)?))
        })
        .collect::<PyResult<_>>()?;
    d.set_item("grids", grids)?;
    let coefs: Vec<Bound<'py, PyArray1<f32>>> = out
        .coefficients
        .into_iter()
        .map(|c| PyArray1::from_vec(py, c))
        .collect();
    d.set_item("coefficients", coefs)?;
    d.set_item("center", out.center)?;
    d.set_item("n_inside", out.n_inside)?;
    d.set_item("field", PyArray1::from_vec(py, out.field))?;
    d.set_item("error", PyArray1::from_vec(py, out.error))?;
    d.set_item(
        "extrapolated",
        out.extrapolated.map(|e| PyArray1::from_vec(py, e)),
    )?;
    d.set_item("extreme", out.extreme)?;
    d.set_item("info", out.fit.info)?;
    d.set_item("n_iter", out.fit.iterations)?;
    d.set_item("system", system_name(out.fit.system))?;
    d.set_item("log", log)?;
    Ok(d)
}

fn mat4_to_py<'py>(py: Python<'py>, m: &Mat4) -> PyResult<Bound<'py, PyArray2<f64>>> {
    let a: Vec<Vec<f64>> = m.iter().map(|r| r.to_vec()).collect();
    Ok(PyArray2::from_vec2(py, &a)?)
}

/// nibabel's `rescale_affine(affine, shape, zooms, new_shape)`.
#[pyfunction]
fn sdcflows_rescale_affine<'py>(
    py: Python<'py>,
    affine: PyReadonlyArray2<'_, f64>,
    shape: [usize; 3],
    zooms: [f64; 3],
    new_shape: [f64; 3],
) -> PyResult<Bound<'py, PyArray2<f64>>> {
    let m = mat4(&affine)?;
    mat4_to_py(py, &tools::rescale_affine(&m, shape, zooms, new_shape))
}

/// The grid sdcflows' `resample_to_zooms` resamples onto: `(np.rint(shape * pre_zooms /
/// zooms), rescale_affine(...))`, the shape as float64 values.
#[pyfunction]
fn sdcflows_resample_to_zooms_grid<'py>(
    py: Python<'py>,
    shape: [usize; 3],
    affine: PyReadonlyArray2<'_, f64>,
    pre_zooms: [f64; 3],
    zooms: [f64; 3],
) -> PyResult<([f64; 3], Bound<'py, PyArray2<f64>>)> {
    let m = mat4(&affine)?;
    let (n, a) = tools::resample_to_zooms_grid(shape, &m, pre_zooms, zooms);
    Ok((n, mat4_to_py(py, &a)?))
}

/// The `(3, n)` voxel coordinates in the moving image of every reference voxel (Fortran
/// order) for nitransforms' `Affine(matrix, reference=ref).apply(moving)` (the identity when
/// `matrix` is None).
#[pyfunction]
#[pyo3(signature = (ref_shape, ref_affine, moving_affine, matrix = None, n_threads = 1))]
fn sdcflows_affine_coordinates<'py>(
    py: Python<'py>,
    ref_shape: [usize; 3],
    ref_affine: PyReadonlyArray2<'_, f64>,
    moving_affine: PyReadonlyArray2<'_, f64>,
    matrix: Option<PyReadonlyArray2<'_, f64>>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArray2<f64>>> {
    let r = mat4(&ref_affine)?;
    let m = mat4(&moving_affine)?;
    let x = match matrix {
        Some(a) => mat4(&a)?,
        None => tools::IDENTITY,
    };
    let pts = py
        .detach(|| tools::affine_coordinates(ref_shape, &r, &x, &m, n_threads))
        .map_err(sdc_err)?;
    let n = pts.len();
    let flat: Vec<f64> = (0..3).flat_map(|c| pts.iter().map(move |p| p[c])).collect();
    let arr = numpy::ndarray::Array2::from_shape_vec((3, n), flat)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(PyArray2::from_owned_array(py, arr))
}

/// nibabel's `apply_affine(aff, pts)` for `(n, 3)` float64 points (numpy's BLAS order).
#[pyfunction]
fn sdcflows_apply_affine<'py>(
    py: Python<'py>,
    affine: PyReadonlyArray2<'_, f64>,
    points: PyReadonlyArray2<'_, f64>,
) -> PyResult<Bound<'py, PyArray2<f64>>> {
    let m = mat4(&affine)?;
    let a = points.as_array();
    if a.ncols() != 3 {
        return Err(PyValueError::new_err("points must have shape (n, 3)"));
    }
    let pts: Vec<[f64; 3]> = a.rows().into_iter().map(|r| [r[0], r[1], r[2]]).collect();
    let out = tools::apply_affine(&m, &pts);
    Ok(PyArray2::from_owned_array(
        py,
        numpy::ndarray::Array2::from_shape_fn((out.len(), 3), |(i, j)| out[i][j]),
    ))
}

fn unwarp_typed<'py, T: larmorx_interp::ndimage::Sample + numpy::Element>(
    py: Python<'py>,
    volume: PyReadonlyArrayDyn<'py, T>,
    fmap: &[f32],
    pe_axis: usize,
    ro: ReadoutTime,
    options: &UnwarpOptions,
    n_threads: usize,
) -> PyResult<(Vec<f32>, Vec<larmorx_core::defects::Notice>, [usize; 3])> {
    let view = volume.as_array();
    let shape: [usize; 3] = view
        .shape()
        .try_into()
        .map_err(|_| PyValueError::new_err("the volume must be 3D"))?;
    let values = crate::ndimage::fortran_values(&view);
    let (out, notices) = py
        .detach(|| {
            larmorx_core::parallel::with_threads(n_threads, || {
                unwarp::unwarp_volume(&values, shape, fmap, pe_axis, ro, options)
            })
        })
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?
        .map_err(sdc_err)?;
    Ok((out, notices, shape))
}

/// `_sdc_unwarp` for one volume (`B0FieldTransform.apply`): `volume` (3D, any numeric type
/// `map_coordinates` takes), `fmap_hz` (float32, the same grid), the phase-encoding axis and
/// the signed readout time, `ro_float64` when it is a numpy float64 (else a Python float).
/// Returns the unwarped volume (float32, C order) and the interpolation's notices.
#[pyfunction]
#[pyo3(signature = (volume, fmap_hz, pe_axis, ro_time, ro_float64, jacobian, order, mode, cval, prefilter, n_threads = 1, upstream_defects = "fix"))]
#[allow(clippy::too_many_arguments)]
fn sdcflows_unwarp_volume<'py>(
    py: Python<'py>,
    volume: &Bound<'py, PyAny>,
    fmap_hz: PyReadonlyArray3<'py, f32>,
    pe_axis: usize,
    ro_time: f64,
    ro_float64: bool,
    jacobian: bool,
    order: u32,
    mode: &str,
    cval: f64,
    prefilter: bool,
    n_threads: usize,
    upstream_defects: &str,
) -> PyResult<(Bound<'py, PyArray3<f32>>, Vec<crate::defects::PyNotice>)> {
    if order > 5 {
        return Err(PyRuntimeError::new_err("spline order not supported"));
    }
    let options = UnwarpOptions {
        order,
        mode: crate::ndimage::mode_from_py(mode)?,
        cval,
        prefilter,
        jacobian,
        defects: crate::defects::mode_from_py(upstream_defects)?,
    };
    let ro = if ro_float64 {
        ReadoutTime::Float64(ro_time)
    } else {
        ReadoutTime::Weak(ro_time)
    };
    let f = fmap_hz.as_array();
    let fmap: Vec<f32> = f.iter().copied().collect();
    macro_rules! go {
        ($($t:ty),*) => {{
            $(
                if let Ok(a) = volume.extract::<PyReadonlyArrayDyn<'py, $t>>() {
                    if a.shape() != f.shape() {
                        return Err(PyValueError::new_err(
                            "the volume and the field must have the same shape",
                        ));
                    }
                    let (out, notices, shape) =
                        unwarp_typed::<$t>(py, a, &fmap, pe_axis, ro, &options, n_threads)?;
                    let arr = numpy::ndarray::Array3::from_shape_vec(shape, out)
                        .map_err(|e| PyValueError::new_err(e.to_string()))?;
                    return Ok((
                        PyArray3::from_owned_array(py, arr),
                        crate::defects::notices_to_py(notices),
                    ));
                }
            )*
            Err(PyRuntimeError::new_err("data type not supported"))
        }};
    }
    go!(f32, f64, i16, u8, i8, u16, i32, u32, i64, u64)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(sdcflows_unwarp_volume, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_bspline_grid, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_collocation_csr, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_collocation_apply, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_dot_f32, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_cg_csr, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_ridge_csr, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_ridge_collocation, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_bspline_approx, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_rescale_affine, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_resample_to_zooms_grid, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_affine_coordinates, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_apply_affine, m)?)?;
    Ok(())
}
