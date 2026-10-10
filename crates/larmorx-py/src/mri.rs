// SPDX-License-Identifier: Apache-2.0
//! Bindings for `larmorx_mri` (`lx.mri`).
//!
//! A series crosses as a path (read with the tool's rules: float32 values, scaling, the x axis
//! reversed for positive-determinant images) or as a float32 `(x, y, z, t)` array with the
//! header that places it. Results come back as a dict of numpy arrays. The GIL is released
//! while files are read and volumes registered.

use std::path::PathBuf;

use larmorx_mri::hmc::estimate::{EstimateParams, InPlane};
use larmorx_mri::hmc::image::{self, Series, read_reference, read_series, storage_values};
use larmorx_mri::hmc::pipeline::{self, HmcOptions, HmcOutput, ReferenceChoice};
use larmorx_mri::hmc::report::parse_mat;
use larmorx_mri::hmc::rigid::Mat4;
use larmorx_mri::hmc::{CostFunction, Interpolation};
use numpy::ndarray::{Array1, Array2, Array3, ArrayD, IxDyn, ShapeBuilder};
use numpy::{PyArray, PyArrayDyn, PyArrayMethods, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::nifti::{header_from_dict, header_to_dict, to_py_err};

fn value_err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

/// The options as Python passes them.
struct Request {
    ref_index: Option<usize>,
    mean: bool,
    stages: usize,
    final_interp: String,
    dof: usize,
    cost: String,
    smooth: f64,
    rotation: f64,
    bins: usize,
    fudge: bool,
    in_plane: String,
    fov: i64,
    init: Option<Mat4>,
    resample: bool,
    n_threads: usize,
}

impl Request {
    fn options(&self) -> PyResult<HmcOptions> {
        if !(1..=4).contains(&self.stages) {
            return Err(value_err(format!(
                "stages must be 1 to 4, not {}",
                self.stages
            )));
        }
        if !(6..=12).contains(&self.dof) {
            return Err(value_err(format!("dof must be 6 to 12, not {}", self.dof)));
        }
        let cost = CostFunction::from_name(&self.cost)
            .ok_or_else(|| value_err(format!("unknown cost {:?}", self.cost)))?;
        let interpolation = Interpolation::from_name(&self.final_interp)
            .ok_or_else(|| value_err(format!("unknown interpolation {:?}", self.final_interp)))?;
        let in_plane = match self.in_plane.as_str() {
            "auto" => InPlane::Auto { fov: self.fov },
            "always" => InPlane::Force,
            "never" => InPlane::Never,
            other => {
                return Err(value_err(format!(
                    "in_plane must be 'auto', 'always' or 'never', not {other:?}"
                )));
            }
        };
        if self.smooth.is_nan()
            || self.smooth < 0.0
            || self.rotation.is_nan()
            || self.rotation <= 0.0
        {
            return Err(value_err("smooth must be >= 0 and rotation > 0"));
        }
        Ok(HmcOptions {
            estimate: EstimateParams {
                stages: self.stages,
                dof: self.dof,
                cost,
                smooth: self.smooth as f32,
                rotation: self.rotation,
                bins: self.bins.max(1),
                fudge: self.fudge,
                in_plane,
                n_threads: self.n_threads,
            },
            interpolation,
            init: self.init,
            resample: self.resample,
        })
    }

    fn choice<'a>(
        &self,
        external: Option<&'a (larmorx_mri::hmc::Volume, larmorx_mri::hmc::world::Geometry)>,
    ) -> ReferenceChoice<'a> {
        match external {
            Some((v, g)) => ReferenceChoice::External(v, *g),
            None if self.mean => ReferenceChoice::Mean(self.ref_index),
            None => ReferenceChoice::Index(self.ref_index),
        }
    }
}

fn stack<'py>(py: Python<'py>, ms: &[Mat4]) -> Bound<'py, PyAny> {
    let a = Array3::from_shape_fn((ms.len(), 4, 4), |(t, i, j)| ms[t][i][j]);
    PyArray::from_owned_array(py, a).into_any()
}

/// The result dict. `grid` is the series that defines the output grid and its storage order.
fn result_to_py<'py>(
    py: Python<'py>,
    out: HmcOutput,
    grid: &Series,
    flipped: bool,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("matrices", stack(py, &out.estimate.matrices))?;
    d.set_item("ras", stack(py, &out.ras))?;
    d.set_item("itk", stack(py, &out.itk))?;
    let params = Array2::from_shape_fn((out.params.len(), 6), |(t, k)| out.params[t][k]);
    d.set_item("params", PyArray::from_owned_array(py, params))?;
    d.set_item(
        "rms_abs",
        PyArray::from_owned_array(py, Array1::from(out.rms_abs)),
    )?;
    d.set_item(
        "rms_rel",
        PyArray::from_owned_array(py, Array1::from(out.rms_rel)),
    )?;
    let fd = pipeline::framewise_displacement(&out.params, 50.0);
    d.set_item("fd", PyArray::from_owned_array(py, Array1::from(fd)))?;
    d.set_item("reference_index", out.estimate.reference_index)?;
    d.set_item("in_plane", out.estimate.in_plane)?;
    d.set_item("header", header_to_dict(py, &grid.header)?)?;
    if let Some(vols) = &out.corrected {
        let [nx, ny, nz] = vols[0].shape;
        let values = storage_values(vols, flipped);
        let a = ArrayD::from_shape_vec(IxDyn(&[nx, ny, nz, vols.len()]).f(), values)
            .map_err(value_err)?;
        d.set_item("data", PyArray::from_owned_array(py, a))?;
    } else {
        d.set_item("data", py.None())?;
    }
    if let Some(mean) = &out.estimate.mean {
        let [nx, ny, nz] = mean.shape;
        let values = storage_values(std::slice::from_ref(mean), flipped);
        let a = ArrayD::from_shape_vec(IxDyn(&[nx, ny, nz]).f(), values).map_err(value_err)?;
        d.set_item("mean", PyArray::from_owned_array(py, a))?;
    } else {
        d.set_item("mean", py.None())?;
    }
    Ok(d)
}

fn run<'py>(
    py: Python<'py>,
    series: Series,
    reference: Option<Series>,
    request: &Request,
) -> PyResult<Bound<'py, PyDict>> {
    let options = request.options()?;
    let external = reference.as_ref().map(|r| {
        let g = pipeline::series_geometry(r);
        (r.volumes[0].clone(), g)
    });
    let choice = request.choice(external.as_ref());
    let out = py
        .detach(|| pipeline::run(&series, &choice, &options))
        .map_err(value_err)?;
    // The output grid: the reference's with an external reference, else the series'.
    let (grid, flipped) = match &reference {
        Some(r) => (r, r.flipped),
        None => (&series, series.flipped),
    };
    result_to_py(py, out, grid, flipped)
}

fn init_from_py(init: Option<PyReadonlyArray2<'_, f64>>) -> PyResult<Option<Mat4>> {
    init.map(|a| {
        let a = a.as_array();
        if a.shape() != [4, 4] {
            return Err(value_err("init must be a 4x4 matrix"));
        }
        Ok(std::array::from_fn(|i| std::array::from_fn(|j| a[[i, j]])))
    })
    .transpose()
}

/// Head-motion correction of a NIfTI series (`lx.mri.hmc` on a path); the reference is a
/// path or `None`.
#[pyfunction]
#[pyo3(signature = (path, reference=None, ref_index=None, mean=false, stages=3, final_interp="trilinear", dof=6, cost="normcorr", smooth=1.0, rotation=1.0, bins=256, fudge=false, in_plane="auto", fov=20, init=None, resample=true, n_threads=1))]
#[allow(clippy::too_many_arguments)]
fn mri_hmc_file<'py>(
    py: Python<'py>,
    path: PathBuf,
    reference: Option<PathBuf>,
    ref_index: Option<usize>,
    mean: bool,
    stages: usize,
    final_interp: &str,
    dof: usize,
    cost: &str,
    smooth: f64,
    rotation: f64,
    bins: usize,
    fudge: bool,
    in_plane: &str,
    fov: i64,
    init: Option<PyReadonlyArray2<'py, f64>>,
    resample: bool,
    n_threads: usize,
) -> PyResult<Bound<'py, PyDict>> {
    let request = Request {
        ref_index,
        mean,
        stages,
        final_interp: final_interp.to_owned(),
        dof,
        cost: cost.to_owned(),
        smooth,
        rotation,
        bins,
        fudge,
        in_plane: in_plane.to_owned(),
        fov,
        init: init_from_py(init)?,
        resample,
        n_threads,
    };
    let read_err = |e: image::ImageError| match e {
        image::ImageError::Nifti(e) => to_py_err(e),
        e => value_err(e),
    };
    let series = py
        .detach(|| read_series(&path, n_threads))
        .map_err(read_err)?;
    let reference = match reference {
        Some(p) => {
            let (vol, mut s, _) = py
                .detach(|| read_reference(&p, n_threads))
                .map_err(read_err)?;
            s.volumes = vec![vol];
            Some(s)
        }
        None => None,
    };
    run(py, series, reference, &request)
}

/// A float32 array and its header as a series.
fn series_from_py(
    data: &Bound<'_, PyArrayDyn<f32>>,
    header: &Bound<'_, PyDict>,
) -> PyResult<Series> {
    let header = header_from_dict(header)?;
    let guard = data.readonly();
    let view = guard.as_array();
    let shape = view.shape().to_vec();
    let dims: [usize; 4] = match shape.as_slice() {
        [x, y, z] => [*x, *y, *z, 1],
        [x, y, z, t] => [*x, *y, *z, *t],
        _ => return Err(value_err("hmc needs a 3D or 4D (x, y, z, t) array")),
    };
    if dims.contains(&0) {
        return Err(value_err("the image has no voxels"));
    }
    let values: Vec<f32> = view.t().iter().copied().collect();
    Ok(Series::from_values(dims, &values, header))
}

/// Head-motion correction of a float32 `(x, y, z, t)` array placed by a header dict; the
/// reference is `None` or a `(array, header)` pair.
#[pyfunction]
#[pyo3(signature = (data, header, reference=None, ref_index=None, mean=false, stages=3, final_interp="trilinear", dof=6, cost="normcorr", smooth=1.0, rotation=1.0, bins=256, fudge=false, in_plane="auto", fov=20, init=None, resample=true, n_threads=1))]
#[allow(clippy::too_many_arguments)]
fn mri_hmc_array<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyArrayDyn<f32>>,
    header: &Bound<'py, PyDict>,
    reference: Option<(Bound<'py, PyArrayDyn<f32>>, Bound<'py, PyDict>)>,
    ref_index: Option<usize>,
    mean: bool,
    stages: usize,
    final_interp: &str,
    dof: usize,
    cost: &str,
    smooth: f64,
    rotation: f64,
    bins: usize,
    fudge: bool,
    in_plane: &str,
    fov: i64,
    init: Option<PyReadonlyArray2<'py, f64>>,
    resample: bool,
    n_threads: usize,
) -> PyResult<Bound<'py, PyDict>> {
    let request = Request {
        ref_index,
        mean,
        stages,
        final_interp: final_interp.to_owned(),
        dof,
        cost: cost.to_owned(),
        smooth,
        rotation,
        bins,
        fudge,
        in_plane: in_plane.to_owned(),
        fov,
        init: init_from_py(init)?,
        resample,
        n_threads,
    };
    let series = series_from_py(data, header)?;
    let reference = match reference {
        Some((a, h)) => {
            let mut s = series_from_py(&a, &h)?;
            s.volumes.truncate(1);
            Some(s)
        }
        None => None,
    };
    run(py, series, reference, &request)
}

/// Parses an FSL text matrix (`MAT_0000` and the like).
#[pyfunction]
fn mri_read_mat<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    let m = parse_mat(text).ok_or_else(|| value_err("not a 4x4 FSL matrix"))?;
    Ok(stack(py, &[m]))
}

/// The text of a `.mat` file as mcflirt writes it (`%.6f ` per number).
#[pyfunction]
fn mri_mat_text(m: PyReadonlyArray2<'_, f64>) -> PyResult<String> {
    let m = init_from_py(Some(m))?.unwrap_or(larmorx_mri::hmc::rigid::IDENTITY);
    Ok(larmorx_mri::hmc::report::mat_text(&m))
}

/// The text of a `.par` file for `(N, 6)` parameters.
#[pyfunction]
fn mri_par_text(params: PyReadonlyArray2<'_, f64>) -> PyResult<String> {
    let a = params.as_array();
    if a.ncols() != 6 {
        return Err(value_err("parameters must have shape (N, 6)"));
    }
    let rows: Vec<[f64; 6]> = a
        .rows()
        .into_iter()
        .map(|r| std::array::from_fn(|k| r[k]))
        .collect();
    Ok(larmorx_mri::hmc::report::par_text(&rows))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(mri_hmc_file, m)?)?;
    m.add_function(wrap_pyfunction!(mri_hmc_array, m)?)?;
    m.add_function(wrap_pyfunction!(mri_read_mat, m)?)?;
    m.add_function(wrap_pyfunction!(mri_mat_text, m)?)?;
    m.add_function(wrap_pyfunction!(mri_par_text, m)?)?;
    Ok(())
}
