// SPDX-License-Identifier: Apache-2.0
//! Typed bindings of the Gaussian group: `SmoothImage`, ImageMath's `G`, `Laplacian`,
//! `Grad` and `UnsharpMask` (`ResampleImageBySpacing` is in `resample`).
//!
//! The spatial filters take the voxels (2 to 4 dimensions, Fortran order without copying) and
//! the spacing of every axis as ITK has it (the time spacing in seconds for a 4D image).

use larmorx_ants::VolumeRef;
use larmorx_ants::image_math::{
    UnsharpMaskOptions, gradient_magnitude, laplacian, smooth_discrete, unsharp_mask,
};
use larmorx_ants::smooth_image::{Smoothing, smooth_image};
use numpy::{PyArrayDyn, PyReadonlyArrayDyn, PyUntypedArrayMethods};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use super::{array_like, slice_f32, value_err};

/// The voxels of `a` and its shape, checked against `spacing`.
fn checked<'a>(
    a: &'a PyReadonlyArrayDyn<'_, f32>,
    spacing: &[f64],
) -> PyResult<(Vec<usize>, std::borrow::Cow<'a, [f32]>)> {
    let shape = a.shape().to_vec();
    if !(2..=4).contains(&shape.len()) {
        return Err(PyValueError::new_err(format!(
            "images have 2 to 4 dimensions here, not {}",
            shape.len()
        )));
    }
    if spacing.len() != shape.len() {
        return Err(PyValueError::new_err(format!(
            "{} spacings for a {}-dimensional image",
            spacing.len(),
            shape.len()
        )));
    }
    Ok((shape, slice_f32(a)))
}

/// `SmoothImage`: recursive Gaussian smoothing with `sigma` (one value or one per axis; in
/// voxels unless `physical`), or, with `median`, median filtering with radius `sigma`
/// (voxels, truncated).
#[pyfunction]
#[pyo3(signature = (a, spacing, sigma, physical = false, median = false, n_threads = 1))]
fn ants_smooth_image<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    spacing: Vec<f64>,
    sigma: Vec<f32>,
    physical: bool,
    median: bool,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let (shape, data) = checked(&a, &spacing)?;
    let radius: Vec<usize> = if median {
        sigma
            .iter()
            .map(|&s| {
                if (0.0..1.0e6).contains(&s) {
                    Ok(s as usize)
                } else {
                    Err(PyValueError::new_err(format!(
                        "a median radius of {s} voxels"
                    )))
                }
            })
            .collect::<PyResult<_>>()?
    } else {
        Vec::new()
    };
    let out = py
        .detach(|| {
            let v = VolumeRef::new(&data, &shape, &spacing);
            let smoothing = if median {
                Smoothing::Median { radius: &radius }
            } else {
                Smoothing::Gaussian {
                    sigma: Some(&sigma),
                    physical,
                }
            };
            smooth_image(v, smoothing, n_threads)
        })
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

/// ImageMath `G`: `DiscreteGaussianImageFilter` with `sigma` (one value or one per axis,
/// physical units).
#[pyfunction]
#[pyo3(signature = (a, spacing, sigma, n_threads = 1))]
fn ants_discrete_gaussian<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    spacing: Vec<f64>,
    sigma: Vec<f32>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let (shape, data) = checked(&a, &spacing)?;
    let out = py
        .detach(|| {
            smooth_discrete(
                VolumeRef::new(&data, &shape, &spacing),
                Some(&sigma),
                n_threads,
            )
        })
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

/// ImageMath `Laplacian` (`LaplacianRecursiveGaussianImageFilter`, then `[0, 1]` if
/// `normalize`).
#[pyfunction]
#[pyo3(signature = (a, spacing, sigma = 1.0, normalize = false, n_threads = 1))]
fn ants_laplacian<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    spacing: Vec<f64>,
    sigma: f32,
    normalize: bool,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let (shape, data) = checked(&a, &spacing)?;
    let out = py
        .detach(|| {
            laplacian(
                VolumeRef::new(&data, &shape, &spacing),
                sigma,
                normalize,
                n_threads,
            )
        })
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

/// ImageMath `Grad` (`GradientMagnitudeRecursiveGaussianImageFilter`, then `[0, 1]` if
/// `normalize`).
#[pyfunction]
#[pyo3(signature = (a, spacing, sigma = 1.0, normalize = false, n_threads = 1))]
fn ants_gradient_magnitude<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    spacing: Vec<f64>,
    sigma: f32,
    normalize: bool,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let (shape, data) = checked(&a, &spacing)?;
    let out = py
        .detach(|| {
            gradient_magnitude(
                VolumeRef::new(&data, &shape, &spacing),
                sigma,
                normalize,
                n_threads,
            )
        })
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

/// ImageMath `UnsharpMask` (`UnsharpMaskImageFilter`, float precision).
#[pyfunction]
#[pyo3(signature = (a, spacing, amount = 0.5, radius = 1.0, threshold = 0.0, radius_in_spacing_units = false, n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn ants_unsharp_mask<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    spacing: Vec<f64>,
    amount: f32,
    radius: f32,
    threshold: f32,
    radius_in_spacing_units: bool,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let (shape, data) = checked(&a, &spacing)?;
    let options = UnsharpMaskOptions {
        amount,
        radius,
        threshold,
        radius_in_spacing_units,
    };
    let out = py
        .detach(|| unsharp_mask(VolumeRef::new(&data, &shape, &spacing), &options, n_threads))
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ants_smooth_image, m)?)?;
    m.add_function(wrap_pyfunction!(ants_discrete_gaussian, m)?)?;
    m.add_function(wrap_pyfunction!(ants_laplacian, m)?)?;
    m.add_function(wrap_pyfunction!(ants_gradient_magnitude, m)?)?;
    m.add_function(wrap_pyfunction!(ants_unsharp_mask, m)?)?;
    Ok(())
}
