// SPDX-License-Identifier: Apache-2.0
//! Typed bindings of `ResampleImageBySpacing` and `ResampleImage`.
//!
//! Both take the image as a path (read as ANTs reads it into a `dim`-dimensional image of the
//! program's pixel type) or as an in-memory `(data, ras_affine, descrip, extra_spacing)`
//! tuple, and return `(data, ras_affine, descrip, spacing)` of the output, `spacing` for every
//! axis.

use larmorx_ants::image::{
    AntsImage, FileStore, OutputImage, Pixel, ReadError, from_itk, read_with,
};
use larmorx_ants::resample_image::{ResampleTarget, resample_image};
use larmorx_ants::resample_image_by_spacing::{
    ResampleBySpacingOptions, resample_image_by_spacing,
};
use larmorx_core::RealElement;
use larmorx_interp::{Interpolation, Window};
use pyo3::exceptions::{PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use super::{itk_image_from_tuple, output_to_py, value_err};

/// The image a program reads: a path through ANTs' `ReadImage`, or an in-memory tuple placed
/// as ITK's reader would place it.
pub(crate) fn ants_input<T: Pixel>(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    dim: usize,
    n_threads: usize,
) -> PyResult<AntsImage<T>> {
    if let Ok(path) = image.extract::<String>() {
        return py
            .detach(|| read_with::<T, _>(&mut FileStore, &path, dim, n_threads))
            .map_err(|e| match e {
                ReadError::Missing(n) => PyFileNotFoundError::new_err(format!("{n}: no such file")),
                other => value_err(other),
            });
    }
    let itk = itk_image_from_tuple(image)?;
    if itk.geometry.ndim != dim {
        return Err(PyValueError::new_err(format!(
            "{dim} values for a {}-dimensional image",
            itk.geometry.ndim
        )));
    }
    from_itk(itk, "image", dim, n_threads).map_err(value_err)
}

/// `(data, ras_affine, descrip, spacing)` of an output image.
pub(crate) fn output_with_spacing<'py>(
    py: Python<'py>,
    image: OutputImage,
) -> PyResult<Bound<'py, PyTuple>> {
    let spacing = image.geometry().spacing.clone();
    let tuple = output_to_py(py, image)?;
    let mut items: Vec<Bound<'py, PyAny>> = tuple.iter().collect();
    items.push(PyTuple::new(py, spacing)?.into_any());
    PyTuple::new(py, items)
}

/// `ResampleImageBySpacing` (float pixels).
#[pyfunction]
#[pyo3(signature = (image, spacing, smooth = true, add_voxels = 0, nearest = false, n_threads = 1))]
fn ants_resample_image_by_spacing<'py>(
    py: Python<'py>,
    image: &Bound<'py, PyAny>,
    spacing: Vec<f64>,
    smooth: bool,
    add_voxels: i32,
    nearest: bool,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    let input: AntsImage<f32> = ants_input(py, image, spacing.len(), n_threads)?;
    let options = ResampleBySpacingOptions {
        spacing,
        smooth,
        add_voxels,
        nearest,
    };
    let (out, _) = py
        .detach(|| resample_image_by_spacing(&input, &options, n_threads))
        .map_err(value_err)?;
    output_with_spacing(py, OutputImage::F32(out))
}

/// `ResampleImage` in pixel type `T`.
fn resample_as<'py, T>(
    py: Python<'py>,
    image: &Bound<'py, PyAny>,
    dim: usize,
    target: ResampleTarget<'_>,
    interpolation: &Interpolation,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>>
where
    T: Pixel + RealElement,
    AntsImage<T>: Into<OutputImage>,
{
    let input: AntsImage<T> = ants_input(py, image, dim, n_threads)?;
    let out = py
        .detach(|| resample_image(&input, target, interpolation, n_threads))
        .map_err(value_err)?;
    output_with_spacing(py, out.into())
}

/// `ResampleImage`: `spacing` or `size` (one value or one per axis), the interpolator by
/// name (`linear`, `nearest`, `gaussian` with `sigma` and `alpha`, `sinc` with `window`,
/// `bspline` with `order`) and the pixel type (`char`, `uchar`, `short`, `ushort`, `int`,
/// `uint`, `float`, `double`).
#[pyfunction]
#[pyo3(signature = (image, dim, spacing = None, size = None, interpolation = "linear", sigma = None, alpha = 1.0, window = "hamming", order = 3, pixel_type = "float", n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn ants_resample_image<'py>(
    py: Python<'py>,
    image: &Bound<'py, PyAny>,
    dim: usize,
    spacing: Option<Vec<f64>>,
    size: Option<Vec<usize>>,
    interpolation: &str,
    sigma: Option<Vec<f64>>,
    alpha: f64,
    window: &str,
    order: u32,
    pixel_type: &str,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    let target = match (&spacing, &size) {
        (Some(s), None) => ResampleTarget::Spacing(s),
        (None, Some(n)) => ResampleTarget::Size(n),
        _ => {
            return Err(PyValueError::new_err(
                "give either the output spacing or the output size",
            ));
        }
    };
    let interpolation = match interpolation {
        "linear" => Interpolation::Linear,
        "nearest" | "nearestneighbor" => Interpolation::NearestNeighbor,
        "gaussian" => {
            let sigma = match sigma {
                Some(s) if s.len() == 3 => [s[0], s[1], s[2]],
                Some(s) if s.len() == 1 => [s[0]; 3],
                Some(s) => {
                    return Err(PyValueError::new_err(format!(
                        "{} Gaussian sigmas (one or three)",
                        s.len()
                    )));
                }
                None => {
                    return Err(PyValueError::new_err(
                        "the Gaussian sigma (the wrapper defaults it to the spacing)",
                    ));
                }
            };
            Interpolation::Gaussian { sigma, alpha }
        }
        "sinc" | "windowedsinc" => Interpolation::WindowedSincEdge(match window {
            "hamming" => Window::Hamming,
            "cosine" => Window::Cosine,
            "welch" => Window::Welch,
            "lanczos" => Window::Lanczos,
            "blackman" => Window::Blackman,
            other => {
                return Err(PyValueError::new_err(format!("unknown window {other:?}")));
            }
        }),
        "bspline" => Interpolation::BSpline { order },
        other => {
            return Err(PyValueError::new_err(format!(
                "unknown interpolation {other:?}"
            )));
        }
    };
    match pixel_type {
        "char" => resample_as::<i8>(py, image, dim, target, &interpolation, n_threads),
        "uchar" => resample_as::<u8>(py, image, dim, target, &interpolation, n_threads),
        "short" => resample_as::<i16>(py, image, dim, target, &interpolation, n_threads),
        "ushort" => resample_as::<u16>(py, image, dim, target, &interpolation, n_threads),
        "int" => resample_as::<i32>(py, image, dim, target, &interpolation, n_threads),
        "uint" => resample_as::<u32>(py, image, dim, target, &interpolation, n_threads),
        "float" => resample_as::<f32>(py, image, dim, target, &interpolation, n_threads),
        "double" => resample_as::<f64>(py, image, dim, target, &interpolation, n_threads),
        other => Err(PyValueError::new_err(format!(
            "unknown pixel type {other:?}"
        ))),
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ants_resample_image_by_spacing, m)?)?;
    m.add_function(wrap_pyfunction!(ants_resample_image, m)?)?;
    Ok(())
}
