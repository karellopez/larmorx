// SPDX-License-Identifier: Apache-2.0
//! Bindings for `larmorx_interp::ndimage` (SciPy's `map_coordinates` and spline filters).
//!
//! Arrays of any memory layout are accepted; they are read in Fortran order (copied when they
//! are not Fortran-contiguous). The GIL is released while filtering and interpolating.

use larmorx_interp::ndimage::{self, Mode, Output, Sample, Spline};
use numpy::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use numpy::{Element, PyArray, PyArrayDyn, PyReadonlyArray2, PyReadonlyArrayDyn};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

pub(crate) fn mode_from_py(name: &str) -> PyResult<Mode> {
    Mode::from_name(name).ok_or_else(|| PyRuntimeError::new_err("boundary mode not supported"))
}

fn err(e: ndimage::NdimageError) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// The array's values in Fortran order, borrowed when it is Fortran-contiguous.
pub(crate) fn fortran_values<'a, T: Copy>(
    a: &numpy::ndarray::ArrayViewD<'a, T>,
) -> std::borrow::Cow<'a, [T]> {
    if a.t().is_standard_layout()
        && let Some(s) = a.to_slice_memory_order()
    {
        return std::borrow::Cow::Borrowed(s);
    }
    std::borrow::Cow::Owned(a.t().iter().copied().collect())
}

#[allow(clippy::too_many_arguments)]
fn interpolate<T: Sample + Element, O: Output + Element, const D: usize>(
    py: Python<'_>,
    input: PyReadonlyArrayDyn<'_, T>,
    coordinates: &[[f64; D]],
    order: u32,
    mode: Mode,
    cval: f64,
    prefilter: bool,
    n_threads: usize,
) -> PyResult<Vec<O>> {
    let view = input.as_array();
    let shape: [usize; D] = view
        .shape()
        .try_into()
        .map_err(|_| PyRuntimeError::new_err("invalid shape for coordinate array"))?;
    let values = fortran_values(&view);
    let mut out = vec![O::default(); coordinates.len()];
    py.detach(|| {
        larmorx_core::parallel::with_threads(n_threads, || {
            ndimage::map_coordinates(
                &values,
                shape,
                coordinates,
                &mut out,
                order,
                mode,
                cval,
                prefilter,
            )
        })
    })
    .map_err(|e| PyValueError::new_err(e.to_string()))?
    .map_err(err)?;
    Ok(out)
}

macro_rules! dispatch_input {
    ($py:expr, $input:expr, $coords:expr, $out:ty, $d:literal, $($rest:expr),*) => {{
        let input = $input;
        if let Ok(a) = input.extract::<PyReadonlyArrayDyn<'_, f32>>() {
            interpolate::<f32, $out, $d>($py, a, $coords, $($rest),*)
        } else if let Ok(a) = input.extract::<PyReadonlyArrayDyn<'_, f64>>() {
            interpolate::<f64, $out, $d>($py, a, $coords, $($rest),*)
        } else if let Ok(a) = input.extract::<PyReadonlyArrayDyn<'_, i16>>() {
            interpolate::<i16, $out, $d>($py, a, $coords, $($rest),*)
        } else if let Ok(a) = input.extract::<PyReadonlyArrayDyn<'_, u8>>() {
            interpolate::<u8, $out, $d>($py, a, $coords, $($rest),*)
        } else if let Ok(a) = input.extract::<PyReadonlyArrayDyn<'_, i8>>() {
            interpolate::<i8, $out, $d>($py, a, $coords, $($rest),*)
        } else if let Ok(a) = input.extract::<PyReadonlyArrayDyn<'_, u16>>() {
            interpolate::<u16, $out, $d>($py, a, $coords, $($rest),*)
        } else if let Ok(a) = input.extract::<PyReadonlyArrayDyn<'_, i32>>() {
            interpolate::<i32, $out, $d>($py, a, $coords, $($rest),*)
        } else if let Ok(a) = input.extract::<PyReadonlyArrayDyn<'_, u32>>() {
            interpolate::<u32, $out, $d>($py, a, $coords, $($rest),*)
        } else if let Ok(a) = input.extract::<PyReadonlyArrayDyn<'_, i64>>() {
            interpolate::<i64, $out, $d>($py, a, $coords, $($rest),*)
        } else if let Ok(a) = input.extract::<PyReadonlyArrayDyn<'_, u64>>() {
            interpolate::<u64, $out, $d>($py, a, $coords, $($rest),*)
        } else {
            Err(PyRuntimeError::new_err("data type not supported"))
        }
    }};
}

fn points<const D: usize>(c: &numpy::ndarray::ArrayView2<'_, f64>) -> Vec<[f64; D]> {
    (0..c.ncols())
        .map(|n| std::array::from_fn(|d| c[[d, n]]))
        .collect()
}

fn to_py<'py, O: Element>(py: Python<'py>, v: Vec<O>) -> Bound<'py, PyAny> {
    PyArray::from_vec(py, v).into_any()
}

macro_rules! dispatch_rank {
    ($py:expr, $input:expr, $c:expr, $out:ty, $($rest:expr),*) => {{
        let c = $c;
        match c.nrows() {
            1 => dispatch_input!($py, $input, &points::<1>(&c), $out, 1, $($rest),*).map(|v| to_py($py, v)),
            2 => dispatch_input!($py, $input, &points::<2>(&c), $out, 2, $($rest),*).map(|v| to_py($py, v)),
            3 => dispatch_input!($py, $input, &points::<3>(&c), $out, 3, $($rest),*).map(|v| to_py($py, v)),
            4 => dispatch_input!($py, $input, &points::<4>(&c), $out, 4, $($rest),*).map(|v| to_py($py, v)),
            _ => Err(PyRuntimeError::new_err("larmorx interpolates arrays of 1 to 4 dimensions")),
        }
    }};
}

/// `scipy.ndimage.map_coordinates` for 1- to 4-dimensional `input` and `coordinates` of shape
/// `(ndim, n)` (float64). Returns the `n` interpolated values in `output` dtype.
#[pyfunction]
#[pyo3(signature = (input, coordinates, order = 3, mode = "constant", cval = 0.0, prefilter = true, output = "float64", n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn ndimage_map_coordinates<'py>(
    py: Python<'py>,
    input: &Bound<'py, PyAny>,
    coordinates: PyReadonlyArray2<'py, f64>,
    order: u32,
    mode: &str,
    cval: f64,
    prefilter: bool,
    output: &str,
    n_threads: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let mode = mode_from_py(mode)?;
    let ndim: usize = input.getattr("ndim")?.extract()?;
    let c = coordinates.as_array();
    if c.nrows() != ndim {
        return Err(PyRuntimeError::new_err(
            "invalid shape for coordinate array",
        ));
    }
    let (o, m, cv, p, t) = (order, mode, cval, prefilter, n_threads);
    match output {
        "float64" => dispatch_rank!(py, input, c, f64, o, m, cv, p, t),
        "float32" => dispatch_rank!(py, input, c, f32, o, m, cv, p, t),
        "int16" => dispatch_rank!(py, input, c, i16, o, m, cv, p, t),
        "uint8" => dispatch_rank!(py, input, c, u8, o, m, cv, p, t),
        "int32" => dispatch_rank!(py, input, c, i32, o, m, cv, p, t),
        "uint16" => dispatch_rank!(py, input, c, u16, o, m, cv, p, t),
        "int8" => dispatch_rank!(py, input, c, i8, o, m, cv, p, t),
        "uint32" => dispatch_rank!(py, input, c, u32, o, m, cv, p, t),
        "int64" => dispatch_rank!(py, input, c, i64, o, m, cv, p, t),
        "uint64" => dispatch_rank!(py, input, c, u64, o, m, cv, p, t),
        other => Err(PyRuntimeError::new_err(format!(
            "output type {other:?} not supported"
        ))),
    }
}

/// `scipy.ndimage.spline_filter1d(input, order, axis, output=float64, mode)` on a float64 array
/// (any layout); returns a Fortran-ordered float64 array.
#[pyfunction]
#[pyo3(signature = (input, order = 3, axis = -1, mode = "mirror", n_threads = 1))]
fn ndimage_spline_filter1d<'py>(
    py: Python<'py>,
    input: PyReadonlyArrayDyn<'py, f64>,
    order: u32,
    axis: isize,
    mode: &str,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f64>>> {
    let mode = mode_from_py(mode)?;
    let view = input.as_array();
    let shape = view.shape().to_vec();
    let ndim = shape.len() as isize;
    let axis = if axis < 0 { axis + ndim } else { axis };
    if axis < 0 || axis >= ndim {
        return Err(PyValueError::new_err(format!(
            "axis {axis} is out of bounds for array of dimension {ndim}"
        )));
    }
    let mut data = fortran_values(&view).into_owned();
    py.detach(|| {
        larmorx_core::parallel::with_threads(n_threads, || {
            ndimage::spline_filter1d(&mut data, &shape, axis as usize, order, mode, true)
        })
    })
    .map_err(|e| PyValueError::new_err(e.to_string()))?
    .map_err(err)?;
    let a = ArrayD::from_shape_vec(IxDyn(&shape).f(), data)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(PyArray::from_owned_array(py, a))
}

/// `scipy.ndimage.spline_filter(input, order, output=float64, mode)` on a float64 array (any
/// layout); returns a Fortran-ordered float64 array.
#[pyfunction]
#[pyo3(signature = (input, order = 3, mode = "mirror", n_threads = 1))]
fn ndimage_spline_filter<'py>(
    py: Python<'py>,
    input: PyReadonlyArrayDyn<'py, f64>,
    order: u32,
    mode: &str,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArrayDyn<f64>>> {
    let mode = mode_from_py(mode)?;
    let view = input.as_array();
    let shape = view.shape().to_vec();
    let mut data = fortran_values(&view).into_owned();
    py.detach(|| {
        larmorx_core::parallel::with_threads(n_threads, || {
            ndimage::spline_filter(&mut data, &shape, order, mode, true)
        })
    })
    .map_err(|e| PyValueError::new_err(e.to_string()))?
    .map_err(err)?;
    let a = ArrayD::from_shape_vec(IxDyn(&shape).f(), data)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(PyArray::from_owned_array(py, a))
}

/// The coefficients `map_coordinates` interpolates for a float32 3D volume (padded and
/// prefiltered as SciPy does), for testing.
#[pyfunction]
#[pyo3(signature = (input, order = 3, mode = "grid-constant", cval = 0.0))]
fn ndimage_prepared_coefficients<'py>(
    py: Python<'py>,
    input: PyReadonlyArrayDyn<'py, f32>,
    order: u32,
    mode: &str,
    cval: f64,
) -> PyResult<Bound<'py, PyArrayDyn<f64>>> {
    let mode = mode_from_py(mode)?;
    let view = input.as_array();
    let shape: [usize; 3] = view
        .shape()
        .try_into()
        .map_err(|_| PyValueError::new_err("a 3D volume is needed"))?;
    let values = fortran_values(&view);
    let spline = Spline::new(&values, shape, order, mode, cval, true, true).map_err(err)?;
    let (c, dims) = spline.coefficients();
    let a = ArrayD::from_shape_vec(IxDyn(&dims).f(), c.to_vec())
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(PyArray::from_owned_array(py, a))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ndimage_map_coordinates, m)?)?;
    m.add_function(wrap_pyfunction!(ndimage_spline_filter1d, m)?)?;
    m.add_function(wrap_pyfunction!(ndimage_spline_filter, m)?)?;
    m.add_function(wrap_pyfunction!(ndimage_prepared_coefficients, m)?)?;
    Ok(())
}
