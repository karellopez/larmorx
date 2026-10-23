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
use numpy::{PyReadonlyArrayDyn, PyUntypedArrayMethods};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use super::gaussian::checked;
use super::resample::{ants_input, output_with_spacing};
use super::{array_like, value_err};
use crate::defects::{mode_from_py, with_notices};
use larmorx_core::defects::Notices;

/// `ants::Morphological`: `operation` is ImageMath's name (`MD`, `ME`, `MO`, `MC`, `GD`, `GE`,
/// `GO`, `GC`), `radius` in voxels, `value` the binary operations' foreground: `(data,
/// notices)` (ANTS-17).
#[pyfunction]
#[pyo3(signature = (a, operation, radius = 1, value = 1.0, n_threads = 1, upstream_defects = "fix"))]
fn ants_morphology<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    operation: &str,
    radius: usize,
    value: f32,
    n_threads: usize,
    upstream_defects: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    let mode = mode_from_py(upstream_defects)?;
    let op = Morphology::from_name(operation).ok_or_else(|| {
        PyValueError::new_err(format!(
            "unknown morphological operation {operation:?} (MD, ME, MO, MC, GD, GE, GO, GC)"
        ))
    })?;
    let spacing = vec![1.0; a.shape().len()];
    let (shape, data) = checked(&a, &spacing)?;
    let (out, notices) = py
        .detach(|| {
            morphological(
                VolumeRef::new(&data, &shape, &spacing),
                op,
                radius,
                value,
                n_threads,
                mode,
            )
        })
        .map_err(value_err)?;
    with_notices(py, array_like(py, out, &shape)?.into_any(), notices)
}

/// ImageMath `FillHoles` with `hole_param` (2: every hole): `(data, notices)`.
#[pyfunction]
#[pyo3(signature = (a, hole_param = 2.0, n_threads = 1, upstream_defects = "fix"))]
fn ants_fill_holes<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    hole_param: f32,
    n_threads: usize,
    upstream_defects: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    let mode = mode_from_py(upstream_defects)?;
    let spacing = vec![1.0; a.shape().len()];
    let (shape, data) = checked(&a, &spacing)?;
    let (out, notices) = py
        .detach(|| {
            fill_holes(
                VolumeRef::new(&data, &shape, &spacing),
                hole_param,
                n_threads,
                mode,
            )
        })
        .map_err(|e: FillHolesError| value_err(e))?;
    with_notices(py, array_like(py, out, &shape)?.into_any(), notices)
}

/// ImageMath `PadImage`: the image (a path, read as a `dim`-dimensional float image, or an
/// in-memory tuple) padded by `pad` voxels of `value` on both sides of every axis (cropped if
/// negative). Returns `((data, ras_affine, descrip, spacing), notices)` (ANTS-22).
#[pyfunction]
#[pyo3(signature = (image, dim, pad, value = 0.0, n_threads = 1, upstream_defects = "fix"))]
fn ants_pad_image<'py>(
    py: Python<'py>,
    image: &Bound<'py, PyAny>,
    dim: usize,
    pad: f32,
    value: f32,
    n_threads: usize,
    upstream_defects: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    let mode = mode_from_py(upstream_defects)?;
    let mut notices = Notices::new(mode);
    let input: AntsImage<f32> = ants_input(py, image, dim, n_threads, &mut notices)?;
    let (out, pad_notices) = py
        .detach(|| pad_image(&input, pad, value, mode))
        .map_err(value_err)?;
    notices.extend(pad_notices);
    with_notices(
        py,
        output_with_spacing(py, OutputImage::F32(out))?.into_any(),
        notices.into_vec(),
    )
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ants_morphology, m)?)?;
    m.add_function(wrap_pyfunction!(ants_fill_holes, m)?)?;
    m.add_function(wrap_pyfunction!(ants_pad_image, m)?)?;
    Ok(())
}
