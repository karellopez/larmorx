//! Bindings for `larmorx_afni` (`lx.afni`).
//!
//! Images cross as numpy arrays in AFNI's storage types (`uint8`, `int16`, `float32`) and
//! header dicts; the GIL is released while files are read and series are shifted.

use std::path::PathBuf;

use larmorx_afni::dataset::{self, AfniImage, BrickData, Bricks, OutputTiming, ReadError};
use larmorx_afni::timing::{Pattern, mean_time};
use larmorx_afni::tshift::{Method, Restore, TshiftParams, tshift};
use larmorx_io::nifti::NiftiHeader;
use numpy::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use numpy::{PyArray, PyArrayDyn, PyArrayMethods};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use crate::nifti::{header_from_dict, header_to_dict, to_py_err};

fn value_err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

/// The shift parameters as Python gives them.
struct Request {
    pattern: Option<String>,
    times: Option<Vec<f64>>,
    tr: Option<f64>,
    tzero: Option<f64>,
    slice: Option<usize>,
    ignore: usize,
    method: String,
    restore: String,
    detrend: bool,
    n_threads: usize,
}

impl Request {
    /// The [`TshiftParams`] for a dataset of `shape` whose header gives `header_tr`, checked
    /// as `3dTshift` checks its options.
    fn params(&self, shape: [usize; 4], header_tr: f32) -> PyResult<TshiftParams> {
        let [_, _, nz, nt] = shape;
        let tr = self.tr.map_or(header_tr, |t| t as f32);
        if tr.is_nan() || tr <= 0.0 || tr.is_infinite() {
            return Err(PyValueError::new_err(format!(
                "the TR must be positive, not {tr}"
            )));
        }
        let slice_times = match (&self.pattern, &self.times) {
            (Some(name), _) => Pattern::from_name(name)
                .ok_or_else(|| PyValueError::new_err(format!("unknown slice pattern {name:?}")))?
                .times(nz, tr),
            (None, Some(times)) => {
                if times.len() != nz {
                    return Err(PyValueError::new_err(format!(
                        "{} slice times for {nz} slices",
                        times.len()
                    )));
                }
                // Parsed as doubles, then stored as float32, as 3dTshift does.
                let times: Vec<f32> = times.iter().map(|&t| t as f32).collect();
                if let Some(bad) = times.iter().find(|&&t| !(0.0..=tr).contains(&t)) {
                    return Err(PyValueError::new_err(format!(
                        "slice time {bad} is outside 0..TR ({tr})"
                    )));
                }
                times
            }
            (None, None) => return Err(PyValueError::new_err("slice_times is required")),
        };
        let tzero = match (self.slice, self.tzero) {
            (Some(k), _) => *slice_times.get(k).ok_or_else(|| {
                PyValueError::new_err(format!("slice {k} is out of range for {nz} slices"))
            })?,
            (None, Some(z)) if z < 0.0 || !z.is_finite() => {
                return Err(PyValueError::new_err(format!(
                    "tzero must be a non-negative number, not {z}"
                )));
            }
            (None, Some(z)) => z as f32,
            (None, None) => mean_time(&slice_times),
        };
        let method = Method::from_name(&self.method)
            .ok_or_else(|| PyValueError::new_err(format!("unknown method {:?}", self.method)))?;
        let restore = match self.restore.as_str() {
            "trend" => Restore::Trend,
            "none" => Restore::None,
            "intercept" => Restore::Intercept,
            other => {
                return Err(PyValueError::new_err(format!(
                    "restore must be 'trend', 'none' or 'intercept', not {other:?}"
                )));
            }
        };
        if nt >= 2 && self.ignore + 5 > nt {
            return Err(PyValueError::new_err(format!(
                "ignore={} is too large for {nt} time points (at most nt - 5)",
                self.ignore
            )));
        }
        Ok(TshiftParams {
            tr,
            slice_times,
            tzero,
            ignore: self.ignore,
            method,
            restore,
            detrend: self.detrend,
            n_threads: self.n_threads,
        })
    }
}

/// `slice_times` is a pattern name or a sequence of numbers.
fn slice_spec(slice_times: &Bound<'_, PyAny>) -> PyResult<(Option<String>, Option<Vec<f64>>)> {
    if let Ok(name) = slice_times.extract::<String>() {
        return Ok((Some(name), None));
    }
    let times: Vec<f64> = slice_times.extract().map_err(|_| {
        PyTypeError::new_err("slice_times must be a pattern name or a sequence of numbers")
    })?;
    Ok((None, Some(times)))
}

fn read_err(e: ReadError) -> PyErr {
    match e {
        ReadError::Nifti(e) => to_py_err(e),
        e => value_err(e),
    }
}

/// The voxels as a Fortran-ordered `(nx, ny, nz, nt)` array of the storage type.
fn bricks_to_py<'py>(py: Python<'py>, bricks: Bricks) -> PyResult<Bound<'py, PyAny>> {
    fn array<'py, T: numpy::Element>(
        py: Python<'py>,
        shape: [usize; 4],
        v: Vec<T>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let a = ArrayD::from_shape_vec(IxDyn(&shape).f(), v).map_err(value_err)?;
        Ok(PyArray::from_owned_array(py, a).into_any())
    }
    match bricks.data {
        BrickData::Byte(v) => array(py, bricks.shape, v),
        BrickData::Short(v) => array(py, bricks.shape, v),
        BrickData::Float(v) => array(py, bricks.shape, v),
    }
}

/// Shifts, then returns `(data, header, warnings)` with the header `3dTshift` would write.
fn finish<'py>(
    py: Python<'py>,
    mut bricks: Bricks,
    input_header: &NiftiHeader,
    request: &Request,
    header_tr: f32,
    warnings: Vec<String>,
) -> PyResult<Bound<'py, PyTuple>> {
    let params = request.params(bricks.shape, header_tr)?;
    py.detach(|| tshift(&mut bricks, &params))
        .map_err(value_err)?;
    let timing = OutputTiming {
        tr: params.tr,
        toffset: params.tzero,
        slices: None,
    };
    let header = dataset::output_header(input_header, &bricks, &timing).map_err(value_err)?;
    PyTuple::new(
        py,
        [
            bricks_to_py(py, bricks)?,
            header_to_dict(py, &header)?.into_any(),
            warnings.into_pyobject(py)?.into_any(),
        ],
    )
}

/// Reads a NIfTI file with AFNI's rules and shifts it (`3dTshift`). Returns the voxels in
/// AFNI's storage type (`uint8`, `int16` or `float32`, before the brick factor, which is the
/// header's `scl_slope`), the output header, and warnings.
#[pyfunction]
#[pyo3(signature = (path, slice_times, tr = None, tzero = None, slice = None, ignore = 0, method = "fourier", restore = "trend", detrend = true, n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn afni_tshift_file<'py>(
    py: Python<'py>,
    path: PathBuf,
    slice_times: &Bound<'py, PyAny>,
    tr: Option<f64>,
    tzero: Option<f64>,
    slice: Option<usize>,
    ignore: usize,
    method: &str,
    restore: &str,
    detrend: bool,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    let (pattern, times) = slice_spec(slice_times)?;
    let request = Request {
        pattern,
        times,
        tr,
        tzero,
        slice,
        ignore,
        method: method.to_owned(),
        restore: restore.to_owned(),
        detrend,
        n_threads,
    };
    let AfniImage {
        bricks,
        header,
        tr: header_tr,
        warnings,
        ..
    } = py
        .detach(|| dataset::read(&path, n_threads))
        .map_err(read_err)?;
    finish(py, bricks, &header, &request, header_tr, warnings)
}

/// Copies a 4D array (any memory layout) into Fortran order, zeroing non-finite floats as
/// AFNI does when it reads them.
fn fortran_vec<T: Copy + numpy::Element>(
    a: &Bound<'_, PyArrayDyn<T>>,
    clean: impl Fn(T) -> T,
) -> PyResult<([usize; 4], Vec<T>)> {
    let guard = a.readonly();
    let view = guard.as_array();
    let shape: [usize; 4] = view
        .shape()
        .try_into()
        .map_err(|_| PyValueError::new_err("tshift needs a 4D (x, y, z, t) array"))?;
    Ok((shape, view.t().iter().map(|&x| clean(x)).collect()))
}

/// Shifts a 4D array of `uint8`, `int16` or `float32` values placed by `header` (the header
/// the image would be written with). Returns `(data, header, warnings)` as
/// [`afni_tshift_file`].
#[pyfunction]
#[pyo3(signature = (data, header, slice_times, tr = None, tzero = None, slice = None, ignore = 0, method = "fourier", restore = "trend", detrend = true, n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn afni_tshift_array<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    header: &Bound<'py, PyDict>,
    slice_times: &Bound<'py, PyAny>,
    tr: Option<f64>,
    tzero: Option<f64>,
    slice: Option<usize>,
    ignore: usize,
    method: &str,
    restore: &str,
    detrend: bool,
    n_threads: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    let (pattern, times) = slice_spec(slice_times)?;
    let request = Request {
        pattern,
        times,
        tr,
        tzero,
        slice,
        ignore,
        method: method.to_owned(),
        restore: restore.to_owned(),
        detrend,
        n_threads,
    };
    let header = header_from_dict(header)?;
    let (shape, data) = if let Ok(a) = data.cast::<PyArrayDyn<u8>>() {
        let (shape, v) = fortran_vec(a, |x| x)?;
        (shape, BrickData::Byte(v))
    } else if let Ok(a) = data.cast::<PyArrayDyn<i16>>() {
        let (shape, v) = fortran_vec(a, |x| x)?;
        (shape, BrickData::Short(v))
    } else if let Ok(a) = data.cast::<PyArrayDyn<f32>>() {
        let (shape, v) = fortran_vec(a, |x| if x.is_finite() { x } else { 0.0 })?;
        (shape, BrickData::Float(v))
    } else {
        return Err(PyTypeError::new_err(
            "tshift arrays must be uint8, int16 or float32 (native byte order)",
        ));
    };
    let bricks = Bricks::new(shape, data, vec![0.0; shape[3]]).map_err(value_err)?;
    let header_tr = dataset::header_tr(&header);
    finish(py, bricks, &header, &request, header_tr, Vec::new())
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(afni_tshift_file, m)?)?;
    m.add_function(wrap_pyfunction!(afni_tshift_array, m)?)?;
    Ok(())
}
