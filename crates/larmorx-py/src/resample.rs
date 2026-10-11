// SPDX-License-Identifier: Apache-2.0
//! Bindings for `larmorx_transform::nitransforms`, `::openblas` (numpy's 4×4 products and
//! inverses in a fixed order) and `::resample_series` (fMRIPrep's one-shot resampler).
//!
//! A chain crosses the boundary as a list of steps: `("affine", matrix)` or
//! `("field", deltas, affine, inverse)` with `deltas` of shape `(x, y, z, 3)` (RAS+ mm).
//! Matrices are float64 4×4 arrays. The GIL is released while mapping and resampling.

use larmorx_core::linalg::Mat4;
use larmorx_core::parallel;
use larmorx_transform::nitransforms::{self, DenseField, Step};
use larmorx_transform::openblas;
use larmorx_transform::resample_series::{PeInfo, ResampleError, ResampleOptions, SeriesResampler};
use numpy::ndarray::{Array2, Array3, ArrayD, Axis, IxDyn, ShapeBuilder};
use numpy::{
    PyArray, PyArray1, PyArray2, PyArray3, PyReadonlyArray2, PyReadonlyArray3, PyReadonlyArrayDyn,
};
use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use crate::ndimage::{fortran_values, mode_from_py};

fn value_err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

fn resample_err(e: ResampleError) -> PyErr {
    let msg = e.to_string();
    if msg.contains("out of bounds") {
        PyIndexError::new_err(msg)
    } else {
        PyValueError::new_err(msg)
    }
}

fn mat4(a: &PyReadonlyArray2<'_, f64>) -> PyResult<Mat4> {
    let a = a.as_array();
    if a.shape() != [4, 4] {
        return Err(PyValueError::new_err("affines must be 4x4"));
    }
    Ok(std::array::from_fn(|i| std::array::from_fn(|j| a[[i, j]])))
}

fn steps_from_py(steps: &Bound<'_, PyAny>) -> PyResult<Vec<Step>> {
    let mut out = Vec::new();
    for item in steps.try_iter()? {
        let item = item?;
        let t = item.cast::<PyTuple>()?;
        let kind: String = t.get_item(0)?.extract()?;
        match kind.as_str() {
            "affine" => {
                let m: PyReadonlyArray2<'_, f64> = t.get_item(1)?.extract()?;
                out.push(Step::Affine(mat4(&m)?));
            }
            "field" => {
                let deltas: PyReadonlyArrayDyn<'_, f64> = t.get_item(1)?.extract()?;
                let affine: PyReadonlyArray2<'_, f64> = t.get_item(2)?.extract()?;
                let inverse: PyReadonlyArray2<'_, f64> = t.get_item(3)?.extract()?;
                let d = deltas.as_array();
                if d.ndim() != 4 || d.shape()[3] != 3 {
                    return Err(PyValueError::new_err(
                        "a displacement field must have shape (x, y, z, 3)",
                    ));
                }
                let shape = [d.shape()[0], d.shape()[1], d.shape()[2]];
                let comps: [Vec<f64>; 3] = std::array::from_fn(|c| {
                    let v = d.index_axis(Axis(3), c);
                    v.t().iter().copied().collect()
                });
                let field =
                    DenseField::from_deltas(shape, &mat4(&affine)?, &mat4(&inverse)?, comps)
                        .map_err(value_err)?;
                out.push(Step::DenseField(field));
            }
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown chain step {other:?}"
                )));
            }
        }
    }
    Ok(out)
}

/// `ImageGrid(img).ndcoords` for a grid of `shape` and `affine`: `(x, y, z, 3)` float64, in
/// the grid's index order.
#[pyfunction]
fn nitransforms_ndcoords<'py>(
    py: Python<'py>,
    shape: [usize; 3],
    affine: PyReadonlyArray2<'_, f64>,
) -> PyResult<Bound<'py, PyAny>> {
    let m = mat4(&affine)?;
    let pts = py.detach(|| nitransforms::ndcoords(shape, &m));
    points_to_grid(py, shape, pts)
}

fn points_to_grid<'py>(
    py: Python<'py>,
    shape: [usize; 3],
    pts: Vec<[f64; 3]>,
) -> PyResult<Bound<'py, PyAny>> {
    let flat: Vec<f64> = (0..3).flat_map(|c| pts.iter().map(move |p| p[c])).collect();
    let a = ArrayD::from_shape_vec(IxDyn(&[shape[0], shape[1], shape[2], 3]).f(), flat)
        .map_err(value_err)?;
    Ok(PyArray::from_owned_array(py, a).into_any())
}

/// `TransformChain(steps).map(points)` for `(n, 3)` float64 points.
#[pyfunction]
#[pyo3(signature = (points, steps, n_threads = 1))]
fn nitransforms_map<'py>(
    py: Python<'py>,
    points: PyReadonlyArray2<'_, f64>,
    steps: &Bound<'_, PyAny>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArray2<f64>>> {
    let steps = steps_from_py(steps)?;
    let a = points.as_array();
    if a.ncols() != 3 {
        return Err(PyValueError::new_err("points must have shape (n, 3)"));
    }
    let mut pts: Vec<[f64; 3]> = a.rows().into_iter().map(|r| [r[0], r[1], r[2]]).collect();
    py.detach(|| parallel::with_threads(n_threads, || nitransforms::map_points(&steps, &mut pts)))
        .map_err(value_err)?
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("out of bounds") {
                PyIndexError::new_err(msg)
            } else {
                PyValueError::new_err(msg)
            }
        })?;
    Ok(PyArray::from_owned_array(
        py,
        Array2::from_shape_fn((pts.len(), 3), |(i, j)| pts[i][j]),
    ))
}

/// fMRIPrep's `resample_image` after loading: resamples `source` (float32, 3D or 4D, any
/// layout) onto the target grid (`target_shape`, `target_affine`) through `steps` (target RAS
/// to source RAS) and `ras2vox`, with per-volume voxel-to-voxel head-motion affines `hmc`
/// (`(t, 4, 4)`), a field map in Hz on the target grid, and readout vectors `pe`
/// (`[(axis, signed readout time)]`, one per volume or one for all). Returns a Fortran-ordered
/// array: the target shape, plus the volumes for a 4D source.
#[pyfunction]
#[pyo3(signature = (source, target_shape, target_affine, steps, ras2vox, hmc = None, fieldmap = None, pe = None, order = 3, mode = "grid-constant", cval = 0.0, prefilter = true, jacobian = true, output = "float32", n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn resample_series<'py>(
    py: Python<'py>,
    source: PyReadonlyArrayDyn<'py, f32>,
    target_shape: [usize; 3],
    target_affine: PyReadonlyArray2<'py, f64>,
    steps: &Bound<'py, PyAny>,
    ras2vox: PyReadonlyArray2<'py, f64>,
    hmc: Option<PyReadonlyArray3<'py, f64>>,
    fieldmap: Option<PyReadonlyArrayDyn<'py, f32>>,
    pe: Option<Vec<(usize, f64)>>,
    order: u32,
    mode: &str,
    cval: f64,
    prefilter: bool,
    jacobian: bool,
    output: &str,
    n_threads: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let mode = mode_from_py(mode)?;
    let steps = steps_from_py(steps)?;
    let target_affine = mat4(&target_affine)?;
    let ras2vox = mat4(&ras2vox)?;
    let src = source.as_array();
    if src.ndim() != 3 && src.ndim() != 4 {
        return Err(PyValueError::new_err("the source must be 3D or 4D"));
    }
    let source_shape = [src.shape()[0], src.shape()[1], src.shape()[2]];
    let n_volumes = if src.ndim() == 4 { src.shape()[3] } else { 1 };
    let values = fortran_values(&src);
    let hmc: Option<Vec<Mat4>> = hmc.map(|h| {
        let h = h.as_array();
        (0..h.shape()[0])
            .map(|t| std::array::from_fn(|i| std::array::from_fn(|j| h[[t, i, j]])))
            .collect()
    });
    if let Some(h) = &hmc
        && h.len() < n_volumes
    {
        return Err(PyIndexError::new_err("list index out of range"));
    }
    let fmap: Option<Vec<f32>> = match fieldmap {
        None => None,
        Some(f) => {
            let v = f.as_array();
            if v.shape() != target_shape {
                return Err(PyValueError::new_err(format!(
                    "the field map has shape {:?}, the target {target_shape:?}",
                    v.shape()
                )));
            }
            Some(fortran_values(&v).into_owned())
        }
    };
    let pe: Vec<PeInfo> = pe
        .unwrap_or_else(|| vec![(0, 0.0)])
        .into_iter()
        .map(|(axis, readout)| PeInfo { axis, readout })
        .collect();
    let options = ResampleOptions {
        order,
        mode,
        cval,
        prefilter,
        jacobian,
    };
    let n_out: usize = target_shape.iter().product::<usize>() * n_volumes;
    let mut shape = target_shape.to_vec();
    if src.ndim() == 4 {
        shape.push(n_volumes);
    }
    macro_rules! run {
        ($t:ty) => {{
            let mut out = vec![<$t>::default(); n_out];
            py.detach(|| -> Result<(), ResampleError> {
                let plan = SeriesResampler::new(
                    target_shape,
                    &target_affine,
                    &steps,
                    &ras2vox,
                    source_shape,
                    fmap,
                    options,
                    n_threads,
                )?;
                plan.resample_series(&values, n_volumes, hmc.as_deref(), &pe, &mut out, n_threads)
            })
            .map_err(resample_err)?;
            let a = ArrayD::from_shape_vec(IxDyn(&shape).f(), out).map_err(value_err)?;
            Ok(PyArray::from_owned_array(py, a).into_any())
        }};
    }
    match output {
        "float32" => run!(f32),
        "float64" => run!(f64),
        other => Err(PyValueError::new_err(format!(
            "output type {other:?} not supported (float32 or float64)"
        ))),
    }
}

fn stack4(a: &PyReadonlyArray3<'_, f64>) -> PyResult<Vec<Mat4>> {
    let a = a.as_array();
    if a.shape()[1..] != [4, 4] {
        return Err(PyValueError::new_err("matrices must be 4x4"));
    }
    Ok((0..a.shape()[0])
        .map(|t| std::array::from_fn(|i| std::array::from_fn(|j| a[[t, i, j]])))
        .collect())
}

fn to_stack<'py>(py: Python<'py>, ms: &[Mat4]) -> Bound<'py, PyArray3<f64>> {
    PyArray::from_owned_array(
        py,
        Array3::from_shape_fn((ms.len(), 4, 4), |(t, i, j)| ms[t][i][j]),
    )
}

/// `a[t] @ b[t]` for two `(n, 4, 4)` float64 stacks, as numpy computes it with OpenBLAS's
/// Haswell `dgemm` (`larmorx_transform::openblas::matmul4`).
#[pyfunction]
fn linalg_matmul4<'py>(
    py: Python<'py>,
    a: PyReadonlyArray3<'_, f64>,
    b: PyReadonlyArray3<'_, f64>,
) -> PyResult<Bound<'py, PyArray3<f64>>> {
    let (a, b) = (stack4(&a)?, stack4(&b)?);
    if a.len() != b.len() {
        return Err(PyValueError::new_err(
            "the stacks must have the same length",
        ));
    }
    let out: Vec<Mat4> = a
        .iter()
        .zip(&b)
        .map(|(x, y)| openblas::matmul4(x, y))
        .collect();
    Ok(to_stack(py, &out))
}

/// The inverses and the singular flags of [`linalg_inv4`].
type Inverses<'py> = (Bound<'py, PyArray3<f64>>, Bound<'py, PyArray1<bool>>);

/// `np.linalg.inv` of each matrix of an `(n, 4, 4)` float64 stack, as OpenBLAS computes it
/// (`larmorx_transform::openblas::inv4`). Returns the inverses (NaN for a singular matrix, as
/// numpy fills them) and which matrices were singular.
#[pyfunction]
fn linalg_inv4<'py>(py: Python<'py>, a: PyReadonlyArray3<'_, f64>) -> PyResult<Inverses<'py>> {
    let a = stack4(&a)?;
    let (out, singular): (Vec<Mat4>, Vec<bool>) = a
        .iter()
        .map(|m| match openblas::inv4(m) {
            Some(x) => (x, false),
            None => ([[f64::NAN; 4]; 4], true),
        })
        .unzip();
    Ok((to_stack(py, &out), PyArray1::from_vec(py, singular)))
}

/// nibabel's `io_orientation` polar step for a 3×3 matrix of unit columns
/// (`larmorx_transform::nitransforms::closest_orthogonal`).
#[pyfunction]
fn linalg_closest_orthogonal<'py>(
    py: Python<'py>,
    rs: PyReadonlyArray2<'_, f64>,
) -> PyResult<Bound<'py, PyArray2<f64>>> {
    let a = rs.as_array();
    if a.shape() != [3, 3] {
        return Err(PyValueError::new_err("the matrix must be 3x3"));
    }
    let m = std::array::from_fn(|i| std::array::from_fn(|j| a[[i, j]]));
    let r = nitransforms::closest_orthogonal(&m)
        .ok_or_else(|| PyValueError::new_err("SVD did not converge"))?;
    Ok(PyArray::from_owned_array(
        py,
        Array2::from_shape_fn((3, 3), |(i, j)| r[i][j]),
    ))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(linalg_matmul4, m)?)?;
    m.add_function(wrap_pyfunction!(linalg_inv4, m)?)?;
    m.add_function(wrap_pyfunction!(linalg_closest_orthogonal, m)?)?;
    m.add_function(wrap_pyfunction!(nitransforms_ndcoords, m)?)?;
    m.add_function(wrap_pyfunction!(nitransforms_map, m)?)?;
    m.add_function(wrap_pyfunction!(resample_series, m)?)?;
    Ok(())
}
