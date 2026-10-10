// SPDX-License-Identifier: Apache-2.0
//! Typed bindings of ImageMath's morphology and mask operations: `MD`, `ME`, `MO`, `MC`,
//! `GD`, `GE`, `GO`, `GC` (`ants_morphology`), `FillHoles` (`ants_fill_holes`) and
//! `PadImage` (`ants_pad_image`).
//!
//! The voxel filters take the voxels (2 to 4 dimensions, Fortran order without copying): the
//! structuring element is in voxels, so they need no spacing. `PadImage` changes the
//! geometry, so it takes the image as a path or an in-memory tuple, as the resampling
//! bindings do.

use larmorx_ants::VolumeRef;
use larmorx_ants::image::{AntsImage, OutputImage};
use larmorx_ants::image_math::{FillHolesError, Morphology, fill_holes, morphological, pad_image};
use numpy::{PyArrayDyn, PyReadonlyArrayDyn, PyUntypedArrayMethods};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use super::gaussian::checked;
use super::resample::{ants_input, output_with_spacing};
use super::{array_like, value_err};

/// `ants::Morphological`: `operation` is ImageMath's name (`MD`, `ME`, `MO`, `MC`, `GD`, `GE`,
/// `GO`, `GC`), `radius` in voxels, `value` the binary operations' foreground.
#[pyfunction]
#[pyo3(signature = (a, operation, radius = 1, value = 1.0, n_threads = 1))]
fn ants_morphology<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    operation: &str,
    radius: usize,
    value: f32,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let op = Morphology::from_name(operation).ok_or_else(|| {
        PyValueError::new_err(format!(
            "unknown morphological operation {operation:?} (MD, ME, MO, MC, GD, GE, GO, GC)"
        ))
    })?;
    let spacing = vec![1.0; a.shape().len()];
    let (shape, data) = checked(&a, &spacing)?;
    let out = py
        .detach(|| {
            morphological(
                VolumeRef::new(&data, &shape, &spacing),
                op,
                radius,
                value,
                n_threads,
            )
        })
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

/// ImageMath `FillHoles` with `hole_param` (2: every hole).
#[pyfunction]
#[pyo3(signature = (a, hole_param = 2.0, n_threads = 1))]
fn ants_fill_holes<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    hole_param: f32,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let spacing = vec![1.0; a.shape().len()];
    let (shape, data) = checked(&a, &spacing)?;
    let out = py
        .detach(|| {
            fill_holes(
                VolumeRef::new(&data, &shape, &spacing),
                hole_param,
                n_threads,
            )
        })
        .map_err(|e: FillHolesError| value_err(e))?;
    array_like(py, out, &shape)
}

/// ImageMath `PadImage`: the image (a path, read as a `dim`-dimensional float image, or an
/// in-memory tuple) padded by `pad` voxels of `value` on both sides of every axis (cropped if
/// negative). Returns `(data, ras_affine, descrip, spacing)`.
#[pyfunction]
#[pyo3(signature = (image, dim, pad, value = 0.0, n_threads = 1))]
fn ants_pad_image<'py>(
    py: Python<'py>,
    image: &Bound<'py, PyAny>,
    dim: usize,
    pad: f32,
    value: f32,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    let input: AntsImage<f32> = ants_input(py, image, dim, n_threads)?;
    let out = py
        .detach(|| pad_image(&input, pad, value))
        .map_err(value_err)?;
    output_with_spacing(py, OutputImage::F32(out))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ants_morphology, m)?)?;
    m.add_function(wrap_pyfunction!(ants_fill_holes, m)?)?;
    m.add_function(wrap_pyfunction!(ants_pad_image, m)?)?;
    Ok(())
}
