// SPDX-License-Identifier: Apache-2.0
//! Bindings for `larmorx_sdcflows` (sdcflows' field-map estimation, ported): the B-spline
//! control grid and the collocation matrix of one or more control grids on a sampling grid.
//! Matrices are float64 4×4 arrays; the GIL is released while computing.

use larmorx_core::linalg::Mat4;
use larmorx_sdcflows::SdcError;
use larmorx_sdcflows::bspline::{self, Collocation, WeightType};
use numpy::{PyArray1, PyArray2, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

fn sdc_err(e: SdcError) -> PyErr {
    match e {
        SdcError::Threads(m) => PyRuntimeError::new_err(m),
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

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(sdcflows_bspline_grid, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_collocation_csr, m)?)?;
    m.add_function(wrap_pyfunction!(sdcflows_collocation_apply, m)?)?;
    Ok(())
}
