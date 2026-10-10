// SPDX-License-Identifier: Apache-2.0
//! Bindings of the ANTs image programs (`larmorx_ants`: ImageMath, ThresholdImage,
//! MultiplyImages, ...), one module per filter group.
//!
//! Two kinds of entry points:
//! - typed functions per filter (`ants_threshold_image`, ...): numpy arrays in and out, the
//!   voxels in any shape (Fortran order is used without copying);
//! - [`ants_program`]: runs an ANTs program's command line on in-memory images. Inputs are
//!   registered under placeholder names, outputs written to placeholder names are returned
//!   instead of written. It runs exactly the command-line code, so the Python generic
//!   `lx.ants.image_math` behaves as `larmorx ants ImageMath`.
//!
//! **Adding a filter group:** write `ants_filters/<group>.rs` with `#[pyfunction]`s that call
//! the group's Rust API through [`slice_f32`] and [`array_like`] (GIL released with
//! `py.detach`, explicit `n_threads`), give it a `register(m)` function, and call it from
//! [`register`] below. Then add the stubs to `python/larmorx/_core.pyi` and the wrapper to
//! `python/larmorx/ants/`.

mod image_math;
mod threshold;

use std::collections::HashMap;
use std::path::PathBuf;

use larmorx_ants::image::{FileStore, ImageStore, OutputImage, ReadError};
use larmorx_core::Grid3;
use larmorx_io::nifti::itk::{GeometrySource, ItkGeometry, ItkImage, ItkMeta, ItkVoxels};
use numpy::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use numpy::{PyArray, PyArrayDyn, PyReadonlyArray2, PyReadonlyArrayDyn, PyUntypedArrayMethods};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use crate::nifti::{affine_from_py, affine_to_py};

pub(crate) fn value_err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

/// The voxels of a float32 array in Fortran order: borrowed when the array is
/// Fortran-contiguous (as arrays read from files are), else copied.
pub(crate) fn slice_f32<'a>(a: &'a PyReadonlyArrayDyn<'_, f32>) -> std::borrow::Cow<'a, [f32]> {
    let view = a.as_array();
    if view.t().is_standard_layout()
        && let Some(s) = view.to_slice_memory_order()
    {
        return std::borrow::Cow::Borrowed(s);
    }
    std::borrow::Cow::Owned(view.t().iter().copied().collect())
}

/// `values` as a Fortran-ordered array shaped like `like`.
pub(crate) fn array_like<'py, T: numpy::Element>(
    py: Python<'py>,
    values: Vec<T>,
    shape: &[usize],
) -> PyResult<Bound<'py, PyArrayDyn<T>>> {
    Ok(PyArray::from_owned_array(
        py,
        ArrayD::from_shape_vec(IxDyn(shape).f(), values).map_err(value_err)?,
    ))
}

// ------------------------------------------------------------------------------------------------
// In-memory images for the command-line code

/// Images held in memory under placeholder names; other names are files.
struct MemoryStore {
    inputs: HashMap<String, ItkImage>,
    outputs: HashMap<String, OutputImage>,
    capture: Vec<String>,
}

impl ImageStore for MemoryStore {
    fn load(&mut self, name: &str, n_threads: usize) -> Result<ItkImage, ReadError> {
        match self.inputs.get(name) {
            Some(image) => Ok(image.clone()),
            None => FileStore.load(name, n_threads),
        }
    }

    fn save(&mut self, name: &str, image: &OutputImage, n_threads: usize) -> Result<(), String> {
        if self.capture.iter().any(|c| c == name) {
            self.outputs.insert(name.to_owned(), image.clone());
            Ok(())
        } else {
            FileStore.save(name, image, n_threads)
        }
    }
}

/// An in-memory image (`data`, RAS+ `affine`, `descrip`) as ITK's reader would give it: the
/// spatial geometry from the affine (LPS), further axes with spacing 1 and origin 0.
fn itk_image_from_py(
    data: &Bound<'_, PyAny>,
    affine: PyReadonlyArray2<'_, f64>,
    descrip: Option<Vec<u8>>,
) -> PyResult<ItkImage> {
    let (shape, voxels) = if let Ok(a) = data.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        (
            a.shape().to_vec(),
            ItkVoxels::F32(slice_f32(&a).into_owned()),
        )
    } else {
        let a: PyReadonlyArrayDyn<'_, f64> = data.extract()?;
        let view = a.as_array();
        (
            a.shape().to_vec(),
            ItkVoxels::F64(view.t().iter().copied().collect()),
        )
    };
    if !(2..=4).contains(&shape.len()) {
        return Err(PyValueError::new_err(format!(
            "in-memory images have 2 to 4 dimensions, not {}",
            shape.len()
        )));
    }
    let mut shape3 = [1usize; 3];
    for (i, s) in shape.iter().take(3).enumerate() {
        shape3[i] = *s;
    }
    let grid = Grid3::from_ras_affine(shape3, &affine_from_py(&affine)?)
        .ok_or_else(|| PyValueError::new_err("the affine is singular"))?;
    let mut geometry = ItkGeometry::from_grid3(&grid);
    if shape.len() == 2 {
        geometry = ItkGeometry {
            ndim: 2,
            size: shape.clone(),
            spacing: geometry.spacing[..2].to_vec(),
            origin: geometry.origin[..2].to_vec(),
            direction: geometry.direction[..2]
                .iter()
                .map(|r| r[..2].to_vec())
                .collect(),
            source: GeometrySource::Default,
            flipped: vec![false; 2],
        };
    } else if shape.len() == 4 {
        geometry = geometry.with_axis(shape[3], 1.0, 0.0);
    }
    Ok(ItkImage {
        geometry: geometry.stored_in_new_image(),
        voxels,
        intent_code: 0,
        meta: ItkMeta {
            descrip: descrip.unwrap_or_default(),
            aux_file: Vec::new(),
        },
    })
}

/// An output image as `(data, ras_affine, descrip)`.
fn output_to_py<'py>(py: Python<'py>, image: OutputImage) -> PyResult<Bound<'py, PyTuple>> {
    let g = image.geometry().clone();
    let affine = affine_to_py(py, &g.ras_affine());
    let shape = g.size.clone();
    let (data, descrip) = match image {
        OutputImage::F32(i) => (array_like(py, i.data, &shape)?.into_any(), i.meta.descrip),
        OutputImage::F64(i) => (array_like(py, i.data, &shape)?.into_any(), i.meta.descrip),
        OutputImage::I32(i) => (array_like(py, i.data, &shape)?.into_any(), i.meta.descrip),
        OutputImage::U8(i) => (array_like(py, i.data, &shape)?.into_any(), i.meta.descrip),
    };
    PyTuple::new(
        py,
        [
            data,
            affine.into_any(),
            pyo3::types::PyBytes::new(py, &descrip).into_any(),
        ],
    )
}

/// Runs ANTs program `program` (`ImageMath`, `ThresholdImage`, `MultiplyImages`) with
/// `args` (after the program name). `inputs` maps placeholder names to
/// `(data, ras_affine, descrip)`; writes to the names in `outputs` are returned instead of
/// written. Returns `(exit_code, stdout, stderr, {name: (data, ras_affine, descrip)})`.
#[pyfunction]
#[pyo3(signature = (program, args, inputs, outputs, n_threads = 1))]
fn ants_program<'py>(
    py: Python<'py>,
    program: &str,
    args: Vec<String>,
    inputs: &Bound<'py, PyDict>,
    outputs: Vec<String>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    let mut store = MemoryStore {
        inputs: HashMap::new(),
        outputs: HashMap::new(),
        capture: outputs,
    };
    for (name, value) in inputs.iter() {
        let name: String = name.extract()?;
        let (data, affine, descrip): (
            Bound<'_, PyAny>,
            PyReadonlyArray2<'_, f64>,
            Option<Vec<u8>>,
        ) = value.extract()?;
        store
            .inputs
            .insert(name, itk_image_from_py(&data, affine, descrip)?);
    }
    type Run = fn(
        &[String],
        &mut dyn ImageStore,
        usize,
        &mut dyn std::io::Write,
        &mut dyn std::io::Write,
    ) -> u8;
    let run: Run = match program {
        "ImageMath" => larmorx_ants::cli::image_math::run,
        "ThresholdImage" => larmorx_ants::cli::threshold_image::run,
        "MultiplyImages" => larmorx_ants::cli::multiply_images::run,
        other => {
            return Err(PyValueError::new_err(format!(
                "unknown ANTs image program {other:?}"
            )));
        }
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = py.detach(|| run(&args, &mut store, n_threads, &mut out, &mut err));
    let produced = PyDict::new(py);
    for (name, image) in store.outputs {
        produced.set_item(name, output_to_py(py, image)?)?;
    }
    PyTuple::new(
        py,
        [
            code.into_pyobject(py)?.into_any(),
            String::from_utf8_lossy(&out).into_pyobject(py)?.into_any(),
            String::from_utf8_lossy(&err).into_pyobject(py)?.into_any(),
            produced.into_any(),
        ],
    )
}

/// The ImageMath operations larmorx implements and their usage lines.
#[pyfunction]
fn ants_image_math_operations<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
    let items = larmorx_ants::cli::image_math::GROUPS
        .iter()
        .flat_map(|g| g.iter())
        .map(|o| PyTuple::new(py, [o.name, o.usage]))
        .collect::<PyResult<Vec<_>>>()?;
    PyList::new(py, items)
}

/// Reads a NIfTI image as an ANTs program reads it into `itk::Image<float, dim>`:
/// `(data float32, ras_affine, descrip)`.
#[pyfunction]
#[pyo3(signature = (path, dim = 3, n_threads = 1))]
fn ants_read_float<'py>(
    py: Python<'py>,
    path: PathBuf,
    dim: usize,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    let name = path.to_string_lossy().into_owned();
    let image = py
        .detach(|| larmorx_ants::image::read_with::<f32, _>(&mut FileStore, &name, dim, n_threads))
        .map_err(|e| match e {
            ReadError::Missing(n) => {
                pyo3::exceptions::PyFileNotFoundError::new_err(format!("{n}: no such file"))
            }
            other => value_err(other),
        })?;
    output_to_py(py, OutputImage::F32(image))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ants_program, m)?)?;
    m.add_function(wrap_pyfunction!(ants_image_math_operations, m)?)?;
    m.add_function(wrap_pyfunction!(ants_read_float, m)?)?;
    image_math::register(m)?;
    threshold::register(m)?;
    Ok(())
}
