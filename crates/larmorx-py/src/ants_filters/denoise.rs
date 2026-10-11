// SPDX-License-Identifier: Apache-2.0
//! Typed binding of `DenoiseImage` (`larmorx_ants::denoise_image`).
//!
//! The image and mask are paths (read as ANTs reads them into `itk::Image<float, dim>` and
//! `itk::Image<unsigned char, dim>`) or in-memory `(data, ras_affine, descrip,
//! extra_spacing)` tuples.

use larmorx_ants::denoise_image::{DenoiseError, DenoiseOptions, NoiseModel, denoise_image};
use larmorx_ants::image::{AntsImage, OutputImage};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use super::resample::{ants_input, output_with_spacing};

/// `DenoiseImage` with ANTs' options: returns `(denoised, noise)`, each as `(data, ras_affine,
/// descrip, spacing)`. `patch_radius` and `search_radius` hold one value for every axis or one
/// per axis.
#[pyfunction]
#[pyo3(signature = (image, dim, mask = None, rician = true, shrink_factor = 1, patch_radius = vec![1], search_radius = vec![2], n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn ants_denoise_image<'py>(
    py: Python<'py>,
    image: &Bound<'py, PyAny>,
    dim: usize,
    mask: Option<&Bound<'py, PyAny>>,
    rician: bool,
    shrink_factor: i32,
    patch_radius: Vec<u32>,
    search_radius: Vec<u32>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    if !(2..=4).contains(&dim) {
        return Err(PyValueError::new_err(format!(
            "DenoiseImage runs on 2, 3 or 4 dimensions, not {dim}"
        )));
    }
    let input: AntsImage<f32> = ants_input(py, image, dim, n_threads)?;
    let mask: Option<AntsImage<u8>> = mask
        .map(|m| ants_input(py, m, dim, n_threads))
        .transpose()?;
    let options = DenoiseOptions {
        noise_model: if rician {
            NoiseModel::Rician
        } else {
            NoiseModel::Gaussian
        },
        shrink_factor,
        patch_radius,
        search_radius,
    };
    let out = py
        .detach(|| denoise_image(&input, mask.as_ref(), &options, n_threads))
        .map_err(|e| match e {
            DenoiseError::Geometry => PyValueError::new_err(
                "the mask is not on the grid the filter runs on (the image's, or the shrunk \
                 image's with a shrink factor above 1)",
            ),
            other => PyValueError::new_err(other.to_string()),
        })?;
    PyTuple::new(
        py,
        [
            output_with_spacing(py, OutputImage::F32(out.denoised))?,
            output_with_spacing(py, OutputImage::F32(out.noise))?,
        ],
    )
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ants_denoise_image, m)?)?;
    Ok(())
}
