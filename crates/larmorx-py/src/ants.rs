// SPDX-License-Identifier: Apache-2.0
//! Bindings for `larmorx_ants`, `larmorx_transform` and the ITK readers of `larmorx_io`.
//!
//! Transforms cross the boundary in ITK's stored form, as lists of
//! `(class name, parameters, fixed parameters)` tuples (a leading `CompositeTransform` entry
//! nests the ones after it). Grids are dicts with `size`, `spacing`, `origin` and `direction`
//! (LPS, as ITK). The GIL is released while images are read or resampled.

use std::path::PathBuf;

use larmorx_ants::{ApplyTransformsOptions, apply_transforms, transform_points};
use larmorx_core::{Grid3, Precision};
use larmorx_interp::{Interpolation, Window};
use larmorx_io::itk_transform::{self, ItkTransformParts, Parameters};
use larmorx_io::nifti;
use larmorx_io::nifti::itk::ItkVoxels;
use larmorx_transform::{Transform, TransformChain};
use numpy::ndarray::{Array2, ArrayD, IxDyn, ShapeBuilder};
use numpy::{PyArray, PyReadonlyArray1, PyReadonlyArray2, PyReadonlyArrayDyn};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use crate::nifti::{affine_from_py, affine_to_py};

fn value_err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

// ------------------------------------------------------------------------------------------------
// Grids

fn grid_to_dict<'py>(py: Python<'py>, g: &Grid3) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("size", PyTuple::new(py, g.size)?)?;
    d.set_item("spacing", PyTuple::new(py, g.spacing)?)?;
    d.set_item("origin", PyTuple::new(py, g.origin)?)?;
    let direction = Array2::from_shape_fn((3, 3), |(i, j)| g.direction[i][j]);
    d.set_item("direction", PyArray::from_owned_array(py, direction))?;
    Ok(d)
}

fn grid_from_dict(d: &Bound<'_, PyDict>) -> PyResult<Grid3> {
    let item = |key: &str| {
        d.get_item(key)?
            .ok_or_else(|| PyValueError::new_err(format!("grid field '{key}' is missing")))
    };
    let size: [usize; 3] = item("size")?.extract()?;
    let spacing: [f64; 3] = item("spacing")?.extract()?;
    let origin: [f64; 3] = item("origin")?.extract()?;
    let direction: PyReadonlyArray2<'_, f64> = item("direction")?.extract()?;
    let direction = direction.as_array();
    if direction.shape() != [3, 3] {
        return Err(PyValueError::new_err("the grid direction must be 3x3"));
    }
    let direction = std::array::from_fn(|i| std::array::from_fn(|j| direction[[i, j]]));
    Grid3::new(size, spacing, origin, direction)
        .ok_or_else(|| PyValueError::new_err("singular grid"))
}

/// The LPS grid of an image with `shape` (first three axes) and RAS+ `affine`.
#[pyfunction]
fn grid_from_ras_affine<'py>(
    py: Python<'py>,
    shape: [usize; 3],
    affine: PyReadonlyArray2<'_, f64>,
) -> PyResult<Bound<'py, PyDict>> {
    let grid = Grid3::from_ras_affine(shape, &affine_from_py(&affine)?)
        .ok_or_else(|| PyValueError::new_err("the affine is singular"))?;
    grid_to_dict(py, &grid)
}

/// The RAS+ affine of an LPS grid.
#[pyfunction]
fn grid_ras_affine<'py>(
    py: Python<'py>,
    grid: &Bound<'_, PyDict>,
) -> PyResult<Bound<'py, PyArray<f64, numpy::Ix2>>> {
    Ok(affine_to_py(py, &grid_from_dict(grid)?.ras_affine()))
}

// ------------------------------------------------------------------------------------------------
// Images as ITK reads them

/// Reads a NIfTI image as ITK 5.4.5 (and so ANTs) does. Returns `(data, info)`: the voxel
/// values (float32 or float64, Fortran order, shaped by ITK's size) and a dict with ITK's
/// `ndim`, `size`, `spacing`, `origin`, `direction`, `intent_code`, the 3D `grid` and its
/// RAS+ `ras_affine`.
#[pyfunction]
#[pyo3(signature = (path, n_threads = 1))]
fn itk_read_image<'py>(
    py: Python<'py>,
    path: PathBuf,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    let image = py
        .detach(|| nifti::itk::read_itk_image(&path, n_threads))
        .map_err(value_err)?;
    let g = &image.geometry;
    let mut shape = g.size.clone();
    let n: usize = shape.iter().product();
    if image.voxels.len() != n {
        // Vector images: components are the slowest axis.
        shape.push(image.voxels.len() / n.max(1));
    }
    let data = match image.voxels {
        ItkVoxels::F32(v) => PyArray::from_owned_array(
            py,
            ArrayD::from_shape_vec(IxDyn(&shape).f(), v).map_err(value_err)?,
        )
        .into_any(),
        ItkVoxels::F64(v) => PyArray::from_owned_array(
            py,
            ArrayD::from_shape_vec(IxDyn(&shape).f(), v).map_err(value_err)?,
        )
        .into_any(),
    };
    let info = PyDict::new(py);
    info.set_item("ndim", g.ndim)?;
    info.set_item("size", PyTuple::new(py, &g.size)?)?;
    info.set_item("spacing", PyTuple::new(py, &g.spacing)?)?;
    info.set_item("origin", PyTuple::new(py, &g.origin)?)?;
    let direction = Array2::from_shape_fn((g.ndim, g.ndim), |(i, j)| g.direction[i][j]);
    info.set_item("direction", PyArray::from_owned_array(py, direction))?;
    info.set_item("intent_code", image.intent_code)?;
    let grid = g
        .grid3()
        .ok_or_else(|| PyValueError::new_err("singular image grid"))?;
    info.set_item("grid", grid_to_dict(py, &grid)?)?;
    info.set_item("ras_affine", affine_to_py(py, &grid.ras_affine()))?;
    PyTuple::new(py, [data, info.into_any()])
}

/// The 3D grid ITK reads from a NIfTI file's header (as a reference image).
#[pyfunction]
fn itk_grid<'py>(py: Python<'py>, path: PathBuf) -> PyResult<Bound<'py, PyDict>> {
    let header = nifti::read_header(&path).map_err(value_err)?;
    let geometry = nifti::itk::itk_geometry(&header)
        .map_err(|e| PyValueError::new_err(format!("{}: {e}", path.display())))?;
    let grid = geometry
        .grid3()
        .ok_or_else(|| PyValueError::new_err("singular image grid"))?;
    grid_to_dict(py, &grid)
}

// ------------------------------------------------------------------------------------------------
// Transforms

fn parts_to_py<'py>(py: Python<'py>, parts: &[ItkTransformParts]) -> PyResult<Bound<'py, PyList>> {
    let items = parts
        .iter()
        .map(|p| {
            let params = match &p.parameters {
                Parameters::F32(v) => PyArray::from_slice(py, v).into_any(),
                Parameters::F64(v) => PyArray::from_slice(py, v).into_any(),
            };
            PyTuple::new(
                py,
                [
                    p.name.clone().into_pyobject(py)?.into_any(),
                    params,
                    PyArray::from_slice(py, &p.fixed_parameters).into_any(),
                ],
            )
        })
        .collect::<PyResult<Vec<_>>>()?;
    PyList::new(py, items)
}

fn parts_from_py(parts: &Bound<'_, PyAny>) -> PyResult<Vec<ItkTransformParts>> {
    let mut out = Vec::new();
    for item in parts.try_iter()? {
        let item = item?;
        let (name, params, fixed): (String, Bound<'_, PyAny>, Vec<f64>) = item.extract()?;
        let parameters = if let Ok(a) = params.extract::<PyReadonlyArray1<'_, f32>>() {
            Parameters::F32(a.as_array().to_vec())
        } else {
            let a: PyReadonlyArray1<'_, f64> = params.extract()?;
            Parameters::F64(a.as_array().to_vec())
        };
        out.push(ItkTransformParts {
            name,
            parameters,
            fixed_parameters: fixed,
        });
    }
    Ok(out)
}

fn transform_from_py(parts: &Bound<'_, PyAny>) -> PyResult<Transform> {
    itk_transform::transform_from_parts(&parts_from_py(parts)?).map_err(value_err)
}

fn chain_from_py(transforms: &Bound<'_, PyAny>) -> PyResult<TransformChain> {
    let mut items = Vec::new();
    for item in transforms.try_iter()? {
        let (parts, invert): (Bound<'_, PyAny>, bool) = item?.extract()?;
        items.push((transform_from_py(&parts)?, invert));
    }
    TransformChain::new(items).map_err(value_err)
}

fn precision_from(name: &str) -> PyResult<Precision> {
    Precision::parse(name).ok_or_else(|| {
        PyValueError::new_err(format!(
            "precision must be 'float64' or 'float32', not {name:?}"
        ))
    })
}

/// The transforms stored in an ITK transform file (text `.txt`, `.tfm`; MATLAB `.mat`; HDF5
/// `.h5`; NIfTI displacement fields `.nii`, `.nii.gz`), as `(name, parameters, fixed)` tuples
/// in the file's order, read as ITK's reader for `precision` (`float64` or `float32`) reads
/// them: in float, the numbers of a text file are rounded once from their text to float (then
/// widened to float64). Returns `(parts, notices)`.
#[pyfunction]
#[pyo3(signature = (path, precision = "float64", upstream_defects = "fix"))]
fn itk_transform_read<'py>(
    py: Python<'py>,
    path: PathBuf,
    precision: &str,
    upstream_defects: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    let precision = precision_from(precision)?;
    let mode = crate::defects::mode_from_py(upstream_defects)?;
    let (parts, notices) = py
        .detach(|| {
            let mut notices = larmorx_core::defects::Notices::new(mode);
            itk_transform::read_itk_transform_parts(&path, precision, &mut notices)
                .map(|p| (p, notices))
        })
        .map_err(transform_file_err)?;
    crate::defects::with_notices(py, parts_to_py(py, &parts)?.into_any(), notices.into_vec())
}

/// A transform file error as Python's exception: a missing file is `FileNotFoundError`, an
/// I/O failure `OSError`, anything else `ValueError`.
fn transform_file_err(e: itk_transform::TransformFileError) -> PyErr {
    use itk_transform::TransformFileError as E;
    match &e {
        E::Missing(p) => pyo3::exceptions::PyFileNotFoundError::new_err(format!(
            "[Errno 2] No such file or directory: '{}'",
            p.display()
        )),
        E::Io { .. } => pyo3::exceptions::PyOSError::new_err(e.to_string()),
        _ => PyValueError::new_err(e.to_string()),
    }
}

/// Writes linear transforms as an ITK text (`.txt`, `.tfm`) or MATLAB (`.mat`, one transform)
/// file.
#[pyfunction]
fn itk_transform_write(path: PathBuf, parts: &Bound<'_, PyAny>) -> PyResult<()> {
    let parts = parts_from_py(parts)?;
    let linear = parts
        .iter()
        .map(|p| {
            larmorx_transform::LinearTransform::from_itk(
                &p.name,
                &p.parameters.to_f64(),
                &p.fixed_parameters,
            )
            .map_err(value_err)
        })
        .collect::<PyResult<Vec<_>>>()?;
    let name = path.to_string_lossy().to_ascii_lowercase();
    let bytes = if name.ends_with(".mat") {
        match linear.as_slice() {
            [one] => itk_transform::write_itk_matlab(one),
            _ => {
                return Err(PyValueError::new_err(
                    "a .mat file holds exactly one linear transform",
                ));
            }
        }
    } else if name.ends_with(".txt") || name.ends_with(".tfm") {
        itk_transform::write_itk_text(&linear).into_bytes()
    } else {
        return Err(PyValueError::new_err(
            "linear transforms are written as .txt, .tfm or .mat",
        ));
    };
    std::fs::write(&path, bytes)
        .map_err(|e| PyRuntimeError::new_err(format!("{}: {e}", path.display())))
}

// ------------------------------------------------------------------------------------------------
// antsApplyTransforms

fn interpolation(
    name: &str,
    sigma: Option<[f64; 3]>,
    alpha: Option<f64>,
    order: u32,
    spacing: [f64; 3],
    notices: &mut larmorx_core::defects::Notices,
) -> PyResult<Interpolation> {
    Ok(match name {
        "linear" => Interpolation::Linear,
        "nearestneighbor" => Interpolation::NearestNeighbor,
        "bspline" => Interpolation::BSpline { order },
        "gaussian" => Interpolation::Gaussian {
            sigma: sigma.unwrap_or(spacing),
            alpha: alpha.unwrap_or(1.0),
        },
        // ANTS-33: antsApplyTransforms ignores MultiLabel's alpha (always 4).
        "multilabel" => Interpolation::MultiLabel {
            sigma: sigma.unwrap_or(spacing),
            alpha: match alpha {
                Some(a) if a.to_bits() != 4.0f64.to_bits() && notices.reproduce() => {
                    notices.add(
                        &larmorx_core::defects::ANTS_33,
                        format!("MultiLabel's alpha {a} is ignored: 4, as in ANTs"),
                    );
                    4.0
                }
                Some(a) => a,
                None => 4.0,
            },
        },
        "genericlabel" => Interpolation::GenericLabel,
        "cosinewindowedsinc" => Interpolation::WindowedSinc(Window::Cosine),
        "hammingwindowedsinc" => Interpolation::WindowedSinc(Window::Hamming),
        "lanczoswindowedsinc" => Interpolation::WindowedSinc(Window::Lanczos),
        "blackmanwindowedsinc" => Interpolation::WindowedSinc(Window::Blackman),
        "welchwindowedsinc" => Interpolation::WindowedSinc(Window::Welch),
        other => {
            return Err(PyValueError::new_err(format!(
                "unknown interpolation {other:?}"
            )));
        }
    })
}

/// Resamples `data` (3D, or 4D for a time series; float32 or float64) from `input_grid` onto
/// `reference_grid` through `transforms` (`[(parts, invert), ...]` in `-t` order), computing in
/// `precision` (`float64`, or `float32` as `antsApplyTransforms --float 1`). Returns data of
/// that type shaped like the reference grid (plus the time axis), with the notices:
/// `(data, notices)`.
#[pyfunction]
#[pyo3(signature = (data, input_grid, reference_grid, transforms, interpolation_name = "linear", sigma = None, alpha = None, order = 3, default_value = 0.0, n_threads = 1, precision = "float64", upstream_defects = "fix"))]
#[allow(clippy::too_many_arguments)]
fn ants_apply_transforms<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    input_grid: &Bound<'_, PyDict>,
    reference_grid: &Bound<'_, PyDict>,
    transforms: &Bound<'_, PyAny>,
    interpolation_name: &str,
    sigma: Option<[f64; 3]>,
    alpha: Option<f64>,
    order: u32,
    default_value: f64,
    n_threads: usize,
    precision: &str,
    upstream_defects: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    let mode = crate::defects::mode_from_py(upstream_defects)?;
    let mut notices = larmorx_core::defects::Notices::new(mode);
    let precision = precision_from(precision)?;
    let input_grid = grid_from_dict(input_grid)?;
    let reference = grid_from_dict(reference_grid)?;
    let chain = chain_from_py(transforms)?;
    let options = ApplyTransformsOptions {
        interpolation: interpolation(
            interpolation_name,
            sigma,
            alpha,
            order,
            input_grid.spacing,
            &mut notices,
        )?,
        default_value,
        n_threads,
        precision,
        upstream_defects: mode,
    };
    let n_in = input_grid.len().max(1);
    let (n_volumes, result) = if let Ok(a) = data.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        let view = a.as_array();
        let owned: Vec<f32>;
        let values: &[f32] = match fortran_slice(&view) {
            Some(v) => v,
            None => {
                owned = view.t().iter().copied().collect();
                &owned
            }
        };
        let r = py.detach(|| apply_transforms(values, &input_grid, &reference, &chain, &options));
        (values.len() / n_in, r)
    } else {
        let a: PyReadonlyArrayDyn<'_, f64> = data.extract()?;
        let view = a.as_array();
        let owned: Vec<f64>;
        let values: &[f64] = match fortran_slice(&view) {
            Some(v) => v,
            None => {
                owned = view.t().iter().copied().collect();
                &owned
            }
        };
        let r = py.detach(|| apply_transforms(values, &input_grid, &reference, &chain, &options));
        (values.len() / n_in, r)
    };
    let (values, result_notices) = result.map_err(value_err)?;
    notices.extend(result_notices);
    let mut shape = reference.size.to_vec();
    if data.getattr("ndim")?.extract::<usize>()? > 3 {
        shape.push(n_volumes);
    }
    let array = if precision == Precision::Float {
        // Float values: exact as float32.
        let values: Vec<f32> = values.iter().map(|&v| v as f32).collect();
        PyArray::from_owned_array(
            py,
            ArrayD::from_shape_vec(IxDyn(&shape).f(), values).map_err(value_err)?,
        )
        .into_any()
    } else {
        PyArray::from_owned_array(
            py,
            ArrayD::from_shape_vec(IxDyn(&shape).f(), values).map_err(value_err)?,
        )
        .into_any()
    };
    crate::defects::with_notices(py, array, notices.into_vec())
}

/// `antsApplyTransforms -u`'s cast of `values` (float64, any shape) to `dtype` (`int8`,
/// `uint8`, `int16`, `int32`, `float32`, `float64`): `(array, notices)`. Out of an integer
/// type's range, x86-64's wraparound with `reproduce`, clamped with `fix` (ANTS-06).
#[pyfunction]
#[pyo3(signature = (values, dtype, upstream_defects = "fix"))]
fn ants_cast_output<'py>(
    py: Python<'py>,
    values: numpy::PyReadonlyArrayDyn<'py, f64>,
    dtype: &str,
    upstream_defects: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    use larmorx_ants::OutputType;
    use larmorx_core::array::DynArray;
    let mode = crate::defects::mode_from_py(upstream_defects)?;
    let ty = match dtype {
        "int8" => OutputType::Char,
        "uint8" => OutputType::UChar,
        "int16" => OutputType::Short,
        "int32" => OutputType::Int,
        "float32" => OutputType::Float,
        "float64" => OutputType::Double,
        other => {
            return Err(PyValueError::new_err(format!(
                "antsApplyTransforms writes int8, uint8, int16, int32, float32 or float64, not {other}"
            )));
        }
    };
    let view = values.as_array();
    let shape = view.shape().to_vec();
    let flat: Vec<f64> = view.t().iter().copied().collect();
    let (array, undefined) =
        larmorx_ants::cli::apply_transforms::cast_output(flat, &shape, ty, mode);
    let mut notices = larmorx_core::defects::Notices::new(mode);
    if undefined > 0 {
        notices.add(
            &larmorx_core::defects::ANTS_06,
            if mode.fix() {
                format!(
                    "{undefined} value(s) are NaN or outside the range of {dtype}: clamped to \
                     the range (NaN to 0); ANTs on x86-64 wraps them around"
                )
            } else {
                format!(
                    "{undefined} value(s) are NaN or outside the range of {dtype}: wrapped \
                     around as ANTs on x86-64 converts them"
                )
            },
        );
    }
    let out = match array {
        DynArray::I8(a) => PyArray::from_owned_array(py, a).into_any(),
        DynArray::U8(a) => PyArray::from_owned_array(py, a).into_any(),
        DynArray::I16(a) => PyArray::from_owned_array(py, a).into_any(),
        DynArray::I32(a) => PyArray::from_owned_array(py, a).into_any(),
        DynArray::F32(a) => PyArray::from_owned_array(py, a).into_any(),
        DynArray::F64(a) => PyArray::from_owned_array(py, a).into_any(),
        _ => unreachable!("cast_output makes only these types"),
    };
    crate::defects::with_notices(py, out, notices.into_vec())
}

/// The array's memory when it is Fortran-contiguous (the layout of arrays read from files),
/// so the data are not copied.
fn fortran_slice<'a, T>(a: &numpy::ndarray::ArrayViewD<'a, T>) -> Option<&'a [T]> {
    if a.t().is_standard_layout() {
        a.to_slice_memory_order()
    } else {
        None
    }
}

/// Maps LPS points (`(n, 3)` float64) through `transforms` (`[(parts, invert), ...]` in `-t`
/// order).
#[pyfunction]
fn ants_transform_points<'py>(
    py: Python<'py>,
    points: PyReadonlyArray2<'_, f64>,
    transforms: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyArray<f64, numpy::Ix2>>> {
    let chain = chain_from_py(transforms)?;
    let a = points.as_array();
    if a.ncols() != 3 {
        return Err(PyValueError::new_err("points must have shape (n, 3)"));
    }
    let pts: Vec<[f64; 3]> = a.rows().into_iter().map(|r| [r[0], r[1], r[2]]).collect();
    let out = py.detach(|| transform_points(&pts, &chain));
    Ok(PyArray::from_owned_array(
        py,
        Array2::from_shape_fn((out.len(), 3), |(i, j)| out[i][j]),
    ))
}

// ------------------------------------------------------------------------------------------------
// Transform files on the command line

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(grid_from_ras_affine, m)?)?;
    m.add_function(wrap_pyfunction!(grid_ras_affine, m)?)?;
    m.add_function(wrap_pyfunction!(itk_read_image, m)?)?;
    m.add_function(wrap_pyfunction!(itk_grid, m)?)?;
    m.add_function(wrap_pyfunction!(itk_transform_read, m)?)?;
    m.add_function(wrap_pyfunction!(itk_transform_write, m)?)?;
    m.add_function(wrap_pyfunction!(ants_apply_transforms, m)?)?;
    m.add_function(wrap_pyfunction!(ants_cast_output, m)?)?;
    m.add_function(wrap_pyfunction!(ants_transform_points, m)?)?;
    Ok(())
}
