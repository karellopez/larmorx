// SPDX-License-Identifier: Apache-2.0
//! Typed binding of `N4BiasFieldCorrection` (`larmorx_ants::n4_bias_field_correction`).
//!
//! The image, mask and weight image are paths (read as ANTs reads them into
//! `itk::Image<float, dim>`) or in-memory `(data, ras_affine, descrip, extra_spacing)` tuples.

use larmorx_ants::image::{AntsImage, OutputImage};
use larmorx_ants::n4_bias_field_correction::{
    BSplineMesh, N4Error, N4Options, n4_bias_field_correction,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};

use super::resample::{ants_input, output_with_spacing};

/// `N4BiasFieldCorrection` with ANTs' options: returns `(corrected, bias_field, iterations)`,
/// the images as `(data, ras_affine, descrip, spacing)` and the iterations as
/// `(level, iteration, convergence)` tuples. `spline_distance` and `mesh` are exclusive (`-b
/// [distance]` or `-b [AxBxC]`); without either, 4 control points per axis.
#[pyfunction]
#[pyo3(signature = (image, dim, mask = None, weight = None, shrink_factor = 4, iterations = vec![50, 50, 50, 50], convergence_threshold = 0.0, spline_distance = None, mesh = None, spline_order = 3, fwhm = 0.15, wiener_noise = 0.01, bins = 200, rescale_intensities = false, n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn ants_n4_bias_field_correction<'py>(
    py: Python<'py>,
    image: &Bound<'py, PyAny>,
    dim: usize,
    mask: Option<&Bound<'py, PyAny>>,
    weight: Option<&Bound<'py, PyAny>>,
    shrink_factor: i32,
    iterations: Vec<u32>,
    convergence_threshold: f32,
    spline_distance: Option<f32>,
    mesh: Option<Vec<f32>>,
    spline_order: u32,
    fwhm: f32,
    wiener_noise: f32,
    bins: u32,
    rescale_intensities: bool,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    if !(2..=4).contains(&dim) {
        return Err(PyValueError::new_err(format!(
            "N4 runs on 2, 3 or 4 dimensions, not {dim}"
        )));
    }
    let mesh = match (spline_distance, mesh) {
        (Some(_), Some(_)) => {
            return Err(PyValueError::new_err(
                "give a spline distance or a mesh, not both",
            ));
        }
        (Some(distance), None) => BSplineMesh::Distance(distance),
        (None, Some(m)) => BSplineMesh::Elements(m),
        (None, None) => BSplineMesh::Default,
    };
    let input: AntsImage<f32> = ants_input(py, image, dim, n_threads)?;
    let mask: Option<AntsImage<f32>> = mask
        .map(|m| ants_input(py, m, dim, n_threads))
        .transpose()?;
    let weight: Option<AntsImage<f32>> = weight
        .map(|w| ants_input(py, w, dim, n_threads))
        .transpose()?;
    let options = N4Options {
        shrink_factor,
        iterations,
        convergence_threshold,
        mesh,
        spline_order,
        fwhm,
        wiener_noise,
        bins,
        rescale_intensities,
    };
    let out = py
        .detach(|| {
            n4_bias_field_correction(
                &input,
                mask.as_ref(),
                weight.as_ref(),
                &options,
                n_threads,
                None,
            )
        })
        .map_err(|e| match e {
            N4Error::MeshResolution => {
                PyValueError::new_err(format!("the mesh needs one value per axis ({dim})"))
            }
            other => PyValueError::new_err(other.to_string()),
        })?;
    let iterations = PyList::new(
        py,
        out.iterations
            .iter()
            .map(|it| (it.level, it.iteration, it.convergence)),
    )?;
    PyTuple::new(
        py,
        [
            output_with_spacing(py, OutputImage::F32(out.corrected))?.into_any(),
            output_with_spacing(py, OutputImage::F32(out.bias_field))?.into_any(),
            iterations.into_any(),
        ],
    )
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ants_n4_bias_field_correction, m)?)?;
    Ok(())
}
