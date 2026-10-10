// SPDX-License-Identifier: Apache-2.0
//! Typed bindings of ImageMath's component, distance-map and label-value operations:
//! `GetLargestComponent`, `D`, `MaurerDistance`, `ThresholdAtMean` and `ReplaceVoxelValue`.
//!
//! They take the voxels (2 to 4 dimensions, Fortran order without copying); the distance maps
//! also take ITK's spacing of every axis (mm; seconds for a fourth axis).

use larmorx_ants::VolumeRef;
use larmorx_ants::image_math::{
    distance_map, extract_contours, largest_component, maurer_distance, replace_voxel_value,
    threshold_at_mean,
};
use numpy::{PyArrayDyn, PyReadonlyArrayDyn, PyUntypedArrayMethods};
use pyo3::prelude::*;

use super::gaussian::checked;
use super::{array_like, slice_f32, value_err};

/// ImageMath `GetLargestComponent` with a minimum object size of `smallest` voxels.
#[pyfunction]
#[pyo3(signature = (a, smallest = 50, n_threads = 1))]
fn ants_largest_component<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    smallest: u64,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let spacing = vec![1.0; a.shape().len()];
    let (shape, data) = checked(&a, &spacing)?;
    let out = py
        .detach(|| largest_component(VolumeRef::new(&data, &shape, &spacing), smallest, n_threads))
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

/// ImageMath `D`: ITK's Danielsson distance map in physical units (sequential).
#[pyfunction]
#[pyo3(signature = (a, spacing))]
fn ants_distance_map<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    spacing: Vec<f64>,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let (shape, data) = checked(&a, &spacing)?;
    let out = py
        .detach(|| distance_map(VolumeRef::new(&data, &shape, &spacing)))
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

/// ImageMath `MaurerDistance`: ITK's signed Maurer distance map of the voxels equal to
/// `foreground`, in physical units, negative inside.
#[pyfunction]
#[pyo3(signature = (a, spacing, foreground = 1.0, n_threads = 1))]
fn ants_maurer_distance<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    spacing: Vec<f64>,
    foreground: f32,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let (shape, data) = checked(&a, &spacing)?;
    let out = py
        .detach(|| {
            maurer_distance(
                VolumeRef::new(&data, &shape, &spacing),
                foreground,
                n_threads,
            )
        })
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

/// ImageMath `ThresholdAtMean`: 1 where `mean · fraction ≤ v ≤ max`.
#[pyfunction]
#[pyo3(signature = (a, fraction = 1.0, n_threads = 1))]
fn ants_threshold_at_mean<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    fraction: f32,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let shape = a.shape().to_vec();
    let data = slice_f32(&a);
    let out = py
        .detach(|| threshold_at_mean(&data, fraction, n_threads))
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

/// ImageMath `ReplaceVoxelValue`: voxels in `[low, high]` become `value`.
#[pyfunction]
#[pyo3(signature = (a, low, high, value, n_threads = 1))]
fn ants_replace_voxel_value<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    low: f32,
    high: f32,
    value: f32,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let shape = a.shape().to_vec();
    let data = slice_f32(&a);
    let out = py
        .detach(|| replace_voxel_value(&data, low, high, value, n_threads))
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

/// ImageMath `ExtractContours`: ITK's `LabelContourImageFilter` on the values truncated to
/// integer labels.
#[pyfunction]
#[pyo3(signature = (a, fully_connected = true, n_threads = 1))]
fn ants_extract_contours<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    fully_connected: bool,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let spacing = vec![1.0; a.shape().len()];
    let (shape, data) = checked(&a, &spacing)?;
    let out = py
        .detach(|| {
            extract_contours(
                VolumeRef::new(&data, &shape, &spacing),
                fully_connected,
                n_threads,
            )
        })
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ants_extract_contours, m)?)?;
    m.add_function(wrap_pyfunction!(ants_largest_component, m)?)?;
    m.add_function(wrap_pyfunction!(ants_distance_map, m)?)?;
    m.add_function(wrap_pyfunction!(ants_maurer_distance, m)?)?;
    m.add_function(wrap_pyfunction!(ants_threshold_at_mean, m)?)?;
    m.add_function(wrap_pyfunction!(ants_replace_voxel_value, m)?)?;
    Ok(())
}
