// SPDX-License-Identifier: Apache-2.0
//! Typed bindings of `ThresholdImage`.

use larmorx_ants::image::x86_to_i32;
use larmorx_ants::threshold_image::{ThresholdMode, threshold_image};
use numpy::{PyArray, PyArrayDyn, PyReadonlyArrayDyn, PyUntypedArrayMethods};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use super::{array_like, slice_f32, value_err};

/// `ThresholdImage d in out lower upper inside outside` on float32 voxels.
#[pyfunction]
#[pyo3(signature = (a, lower, upper, inside = 1.0, outside = 0.0, n_threads = 1))]
fn ants_threshold_image<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    lower: f32,
    upper: f32,
    inside: f32,
    outside: f32,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let shape = a.shape().to_vec();
    let av = slice_f32(&a);
    let mode = ThresholdMode::Range {
        lower,
        upper,
        inside,
        outside,
    };
    let out = py
        .detach(|| threshold_image(&av, mode, n_threads))
        .map_err(value_err)?;
    array_like(py, out.data, &shape)
}

/// `ThresholdImage d in out Otsu n [mask]`: returns `(labels, thresholds)`. The mask values
/// are converted to `int` as ANTs reads them; the region is where they are 1.
#[pyfunction]
#[pyo3(signature = (a, n_thresholds, mask = None, n_threads = 1))]
fn ants_otsu_threshold<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    n_thresholds: usize,
    mask: Option<PyReadonlyArrayDyn<'py, f64>>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    let shape = a.shape().to_vec();
    if let Some(m) = &mask
        && m.shape() != shape.as_slice()
    {
        return Err(PyValueError::new_err(format!(
            "the mask shape {:?} differs from the image shape {shape:?}",
            m.shape()
        )));
    }
    let av = slice_f32(&a);
    let mask: Option<Vec<i32>> = mask
        .as_ref()
        .map(|m| m.as_array().t().iter().map(|&v| x86_to_i32(v)).collect());
    let out = py
        .detach(|| {
            threshold_image(
                &av,
                ThresholdMode::Otsu {
                    thresholds: n_thresholds,
                    mask: mask.as_deref(),
                },
                n_threads,
            )
        })
        .map_err(value_err)?;
    PyTuple::new(
        py,
        [
            array_like(py, out.data, &shape)?.into_any(),
            PyArray::from_vec(py, out.thresholds).into_any(),
        ],
    )
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ants_threshold_image, m)?)?;
    m.add_function(wrap_pyfunction!(ants_otsu_threshold, m)?)?;
    Ok(())
}
