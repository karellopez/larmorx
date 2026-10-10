// SPDX-License-Identifier: Apache-2.0
//! Typed bindings of ImageMath's arithmetic and intensity operations (and MultiplyImages,
//! which is the same product).

use larmorx_ants::image::{Pixel, x86_to_i32};
use larmorx_ants::image_math::{
    Arithmetic, Normalization, Operand, TruncateOptions, arithmetic, negative, normalize, rescale,
    truncate_image_intensity,
};
use numpy::{PyArrayDyn, PyReadonlyArrayDyn, PyUntypedArrayMethods};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use super::{array_like, slice_f32, value_err};

/// `ImageMath`'s `ImageMath<DIM>` (`op` is ImageMath's name: `m`, `+`, `-`, `/`, `^`, `exp`,
/// `max`, `abs`, `addtozero`, `overadd`, `Decision`, `total`, `mean`, `vtotal`) on float32
/// `a` and `b` (an array, which may be larger than `a`, or a number). Returns
/// `(data, result, count)`: the output, ANTs' final running `result` and voxel count.
#[pyfunction]
#[pyo3(signature = (op, a, b, n_threads = 1))]
fn ants_arithmetic<'py>(
    py: Python<'py>,
    op: &str,
    a: PyReadonlyArrayDyn<'py, f32>,
    b: &Bound<'py, PyAny>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    let op = Arithmetic::from_name(op)
        .ok_or_else(|| PyValueError::new_err(format!("unknown ImageMath operation {op:?}")))?;
    let shape = a.shape().to_vec();
    let av = slice_f32(&a);
    let out = if let Ok(barr) = b.extract::<PyReadonlyArrayDyn<'py, f32>>() {
        let bshape = barr.shape().to_vec();
        let bv = slice_f32(&barr);
        py.detach(|| {
            arithmetic(
                op,
                &av,
                &shape,
                Operand::Image {
                    data: &bv,
                    size: &bshape,
                },
                n_threads,
            )
        })
    } else {
        let s: f32 = b.extract()?;
        py.detach(|| arithmetic(op, &av, &shape, Operand::Scalar(s), n_threads))
    }
    .map_err(value_err)?;
    PyTuple::new(
        py,
        [
            array_like(py, out.data, &shape)?.into_any(),
            out.result.into_pyobject(py)?.into_any(),
            out.count.into_pyobject(py)?.into_any(),
        ],
    )
}

/// `NegativeImage` (`ImageMath Neg`).
#[pyfunction]
#[pyo3(signature = (a, n_threads = 1))]
fn ants_negative_image<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let shape = a.shape().to_vec();
    let av = slice_f32(&a);
    let out = py.detach(|| negative(&av, n_threads)).map_err(value_err)?;
    array_like(py, out, &shape)
}

/// A mask read as ANTs reads it into an `int` image (a C cast of each value).
fn int_mask(mask: &PyReadonlyArrayDyn<'_, f64>) -> Vec<i32> {
    mask.as_array().t().iter().map(|&v| x86_to_i32(v)).collect()
}

/// `TruncateImageIntensity`: returns `(data, lower, upper)`.
#[pyfunction]
#[pyo3(signature = (a, lower_quantile = 0.025, upper_quantile = None, bins = 64, mask = None, n_threads = 1))]
fn ants_truncate_image_intensity<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    lower_quantile: f32,
    upper_quantile: Option<f32>,
    bins: usize,
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
    let mask = mask.as_ref().map(int_mask);
    let options = TruncateOptions {
        lower_quantile,
        upper_quantile: upper_quantile.unwrap_or(1.0 - lower_quantile),
        bins,
    };
    let out = py
        .detach(|| truncate_image_intensity(&av, mask.as_deref(), &options, n_threads))
        .map_err(value_err)?;
    PyTuple::new(
        py,
        [
            array_like(py, out.data, &shape)?.into_any(),
            out.lower.into_pyobject(py)?.into_any(),
            out.upper.into_pyobject(py)?.into_any(),
        ],
    )
}

/// `Normalize`: `mode` is `"range"`, `"mean"` or `"mask"` (with `mask`, read as float).
#[pyfunction]
#[pyo3(signature = (a, mode = "range", mask = None, n_threads = 1))]
fn ants_normalize_image<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    mode: &str,
    mask: Option<PyReadonlyArrayDyn<'py, f32>>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let shape = a.shape().to_vec();
    let av = slice_f32(&a);
    let mask_values = match (&mask, mode) {
        (Some(m), "mask") => {
            if m.shape() != shape.as_slice() {
                return Err(PyValueError::new_err(format!(
                    "the mask shape {:?} differs from the image shape {shape:?}",
                    m.shape()
                )));
            }
            Some(slice_f32(m).into_owned())
        }
        (None, "range" | "mean") => None,
        _ => {
            return Err(PyValueError::new_err(
                "mode is 'range' or 'mean' without a mask, 'mask' with one",
            ));
        }
    };
    let out = py
        .detach(|| {
            let normalization = match (&mask_values, mode) {
                (Some(m), _) => Normalization::MaskMean(m),
                (None, "mean") => Normalization::Mean,
                _ => Normalization::Range,
            };
            normalize(&av, normalization, n_threads)
        })
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

/// `RescaleImage` (ITK's `RescaleIntensityImageFilter`).
#[pyfunction]
#[pyo3(signature = (a, minimum, maximum, n_threads = 1))]
fn ants_rescale_image<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    minimum: f32,
    maximum: f32,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let shape = a.shape().to_vec();
    let av = slice_f32(&a);
    let out = py
        .detach(|| rescale(&av, minimum, maximum, n_threads))
        .map_err(value_err)?;
    array_like(py, out, &shape)
}

/// Converts float64 values as ANTs converts them to `float` pixels (`static_cast`).
#[pyfunction]
fn ants_to_float<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f64>,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let shape = a.shape().to_vec();
    let values: Vec<f32> = a.as_array().t().iter().map(|&v| f32::from_f64(v)).collect();
    array_like(py, values, &shape)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ants_arithmetic, m)?)?;
    m.add_function(wrap_pyfunction!(ants_negative_image, m)?)?;
    m.add_function(wrap_pyfunction!(ants_truncate_image_intensity, m)?)?;
    m.add_function(wrap_pyfunction!(ants_normalize_image, m)?)?;
    m.add_function(wrap_pyfunction!(ants_rescale_image, m)?)?;
    m.add_function(wrap_pyfunction!(ants_to_float, m)?)?;
    Ok(())
}
