// SPDX-License-Identifier: Apache-2.0
//! Bindings for `larmorx_ndimage` (SciPy's filters, morphology and labelling) and for the parts
//! of `larmorx_core::numpy` that `lx.ndimage`, the field-map ports and their checks call
//! (percentiles, medians, `clip`, float32 `exp`).
//!
//! Arrays of any layout are read in place when they are contiguous in some axis order, else
//! copied once; results are C-ordered (SciPy's). Boolean arrays come in as `uint8` views. The
//! GIL is released while computing.

use std::borrow::Cow;

use larmorx_core::numpy::{self as np, NpFloat, NumpyRelease};
use larmorx_ndimage::{
    self as nd, BinaryErosion, Extend, Footprint, Labels, Layout, MaskElement, NdElement,
    NdimageError,
};
use numpy::ndarray::{ArrayD, ArrayViewD, IxDyn};
use numpy::{Element, PyArray, PyArrayDyn, PyReadonlyArrayDyn};
use pyo3::exceptions::{PyIndexError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;

fn nd_err(e: NdimageError) -> PyErr {
    match e {
        NdimageError::Origin | NdimageError::EmptyFootprint | NdimageError::LabelStructure => {
            PyValueError::new_err(e.to_string())
        }
        NdimageError::CentreIndex(_) => PyIndexError::new_err(e.to_string()),
        _ => PyRuntimeError::new_err(e.to_string()),
    }
}

fn threads_err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

/// The array's values and layout: borrowed when it is contiguous in some axis order with
/// non-negative strides, else a C-ordered copy.
fn strided<'a, T: Copy>(a: &ArrayViewD<'a, T>) -> (Cow<'a, [T]>, Layout) {
    let shape = a.shape().to_vec();
    if a.strides().iter().all(|&s| s >= 0)
        && let Some(s) = a.to_slice_memory_order()
    {
        let strides = a.strides().iter().map(|&s| s as usize).collect();
        return (Cow::Borrowed(s), Layout { shape, strides });
    }
    (Cow::Owned(a.iter().copied().collect()), Layout::c(&shape))
}

fn to_py<'py, T: Element>(
    py: Python<'py>,
    shape: &[usize],
    v: Vec<T>,
) -> PyResult<Bound<'py, PyArrayDyn<T>>> {
    let a = ArrayD::from_shape_vec(IxDyn(shape), v)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(PyArray::from_owned_array(py, a))
}

fn extend(mode: &str) -> PyResult<Extend> {
    Extend::from_name(mode).ok_or_else(|| PyRuntimeError::new_err("boundary mode not supported"))
}

/// SciPy's last cast of a filter's double result into the output's element type: `safe` for
/// `CASE_FILTER_OUT_SAFE` (the n-dimensional and rank filters), plain for the line filters.
fn cast_out<'py>(
    py: Python<'py>,
    shape: &[usize],
    v: Vec<f64>,
    dtype: &str,
    safe: bool,
) -> PyResult<Bound<'py, PyAny>> {
    fn conv<O: NdElement>(v: Vec<f64>, safe: bool) -> Vec<O> {
        v.into_iter()
            .map(|x| {
                if safe {
                    O::from_f64_safe(x)
                } else {
                    O::from_f64(x)
                }
            })
            .collect()
    }
    Ok(match dtype {
        "float64" => to_py(py, shape, v)?.into_any(),
        "float32" => to_py(py, shape, conv::<f32>(v, safe))?.into_any(),
        "uint8" | "bool" => to_py(py, shape, conv::<u8>(v, safe))?.into_any(),
        "int8" => to_py(py, shape, conv::<i8>(v, safe))?.into_any(),
        "uint16" => to_py(py, shape, conv::<u16>(v, safe))?.into_any(),
        "int16" => to_py(py, shape, conv::<i16>(v, safe))?.into_any(),
        "uint32" => to_py(py, shape, conv::<u32>(v, safe))?.into_any(),
        "int32" => to_py(py, shape, conv::<i32>(v, safe))?.into_any(),
        "uint64" => to_py(py, shape, conv::<u64>(v, safe))?.into_any(),
        "int64" => to_py(py, shape, conv::<i64>(v, safe))?.into_any(),
        _ => return Err(PyRuntimeError::new_err("array type not supported")),
    })
}

/// Whether the numpy dtype name is the element type `T`'s.
fn same_type<T: Element>(py: Python<'_>, dtype: &str) -> bool {
    let d = numpy::dtype::<T>(py);
    let name = d.to_string();
    name == dtype || (dtype == "bool" && name == "uint8")
}

fn footprint(fp: &PyReadonlyArrayDyn<'_, bool>) -> PyResult<Footprint> {
    let v = fp.as_array();
    let shape = v.shape().to_vec();
    Footprint::new(&shape, v.iter().copied().collect()).map_err(nd_err)
}

/// Calls `$f::<T>(...)` for the element type of the numpy array `$a` (`uint8` covers `bool`).
macro_rules! by_dtype {
    ($a:expr, $f:ident, $($args:expr),*) => {{
        let a = $a;
        if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, f32>>() {
            $f::<f32>(x, $($args),*)
        } else if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, f64>>() {
            $f::<f64>(x, $($args),*)
        } else if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, u8>>() {
            $f::<u8>(x, $($args),*)
        } else if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, i8>>() {
            $f::<i8>(x, $($args),*)
        } else if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, u16>>() {
            $f::<u16>(x, $($args),*)
        } else if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, i16>>() {
            $f::<i16>(x, $($args),*)
        } else if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, u32>>() {
            $f::<u32>(x, $($args),*)
        } else if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, i32>>() {
            $f::<i32>(x, $($args),*)
        } else if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, u64>>() {
            $f::<u64>(x, $($args),*)
        } else if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, i64>>() {
            $f::<i64>(x, $($args),*)
        } else {
            Err(PyRuntimeError::new_err("array type not supported"))
        }
    }};
}

#[allow(clippy::too_many_arguments)]
fn min_or_max_t<'py, T: NdElement + Element>(
    input: PyReadonlyArrayDyn<'py, T>,
    py: Python<'py>,
    fp: &Footprint,
    structure: Option<&[f64]>,
    mode: Extend,
    cval: f64,
    origins: &[isize],
    minimum: bool,
    n_threads: usize,
    output: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let view = input.as_array();
    let (data, layout) = strided(&view);
    if same_type::<T>(py, output) {
        let out = py
            .detach(|| {
                larmorx_core::parallel::with_threads(n_threads, || {
                    nd::min_or_max_filter::<T, T>(
                        &data, &layout, fp, structure, mode, cval, origins, minimum,
                    )
                })
            })
            .map_err(threads_err)?
            .map_err(nd_err)?;
        return Ok(to_py(py, &layout.shape, out)?.into_any());
    }
    let out = py
        .detach(|| {
            larmorx_core::parallel::with_threads(n_threads, || {
                nd::min_or_max_filter::<T, f64>(
                    &data, &layout, fp, structure, mode, cval, origins, minimum,
                )
            })
        })
        .map_err(threads_err)?
        .map_err(nd_err)?;
    cast_out(py, &layout.shape, out, output, true)
}

/// `_nd_image.min_or_max_filter(input, footprint, structure, output, mode, cval, origins,
/// minimum)`: the non-separable minimum or maximum filter (`larmorx_ndimage::min_or_max_filter`).
#[pyfunction]
#[pyo3(signature = (input, footprint, structure, mode, cval, origins, minimum, n_threads = 1, output = None))]
#[allow(clippy::too_many_arguments)]
fn ndimage_min_or_max_filter<'py>(
    py: Python<'py>,
    input: &Bound<'py, PyAny>,
    footprint: PyReadonlyArrayDyn<'py, bool>,
    structure: Option<PyReadonlyArrayDyn<'py, f64>>,
    mode: &str,
    cval: f64,
    origins: Vec<isize>,
    minimum: bool,
    n_threads: usize,
    output: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let fp = self::footprint(&footprint)?;
    let st: Option<Vec<f64>> = structure.map(|s| s.as_array().iter().copied().collect());
    let mode = extend(mode)?;
    let out_name = output_name(input, output)?;
    by_dtype!(
        input,
        min_or_max_t,
        py,
        &fp,
        st.as_deref(),
        mode,
        cval,
        &origins,
        minimum,
        n_threads,
        &out_name
    )
}

/// The output dtype's name (the input's when none is given).
fn output_name(input: &Bound<'_, PyAny>, output: Option<&str>) -> PyResult<String> {
    match output {
        Some(o) => Ok(o.to_owned()),
        None => Ok(input.getattr("dtype")?.getattr("name")?.extract()?),
    }
}

#[allow(clippy::too_many_arguments)]
fn min_or_max_1d_t<'py, T: NdElement + Element>(
    input: PyReadonlyArrayDyn<'py, T>,
    py: Python<'py>,
    size: usize,
    axis: usize,
    mode: Extend,
    cval: f64,
    origin: isize,
    minimum: bool,
    n_threads: usize,
    output: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let view = input.as_array();
    let (data, layout) = strided(&view);
    if same_type::<T>(py, output) {
        let out = py
            .detach(|| {
                larmorx_core::parallel::with_threads(n_threads, || {
                    nd::min_or_max_filter1d::<T, T>(
                        &data, &layout, size, axis, mode, cval, origin, minimum,
                    )
                })
            })
            .map_err(threads_err)?
            .map_err(nd_err)?;
        return Ok(to_py(py, &layout.shape, out)?.into_any());
    }
    let out = py
        .detach(|| {
            larmorx_core::parallel::with_threads(n_threads, || {
                nd::min_or_max_filter1d::<T, f64>(
                    &data, &layout, size, axis, mode, cval, origin, minimum,
                )
            })
        })
        .map_err(threads_err)?
        .map_err(nd_err)?;
    cast_out(py, &layout.shape, out, output, false)
}

/// `_nd_image.min_or_max_filter1d(input, size, axis, output, mode, cval, origin, minimum)`.
#[pyfunction]
#[pyo3(signature = (input, size, axis, mode, cval, origin, minimum, n_threads = 1, output = None))]
#[allow(clippy::too_many_arguments)]
fn ndimage_min_or_max_filter1d<'py>(
    py: Python<'py>,
    input: &Bound<'py, PyAny>,
    size: usize,
    axis: usize,
    mode: &str,
    cval: f64,
    origin: isize,
    minimum: bool,
    n_threads: usize,
    output: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let mode = extend(mode)?;
    let out_name = output_name(input, output)?;
    by_dtype!(
        input,
        min_or_max_1d_t,
        py,
        size,
        axis,
        mode,
        cval,
        origin,
        minimum,
        n_threads,
        &out_name
    )
}

#[allow(clippy::too_many_arguments)]
fn rank_t<'py, T: NdElement + Element>(
    input: PyReadonlyArrayDyn<'py, T>,
    py: Python<'py>,
    rank: usize,
    fp: &Footprint,
    mode: Extend,
    cval: f64,
    origins: &[isize],
    n_threads: usize,
    output: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let view = input.as_array();
    let (data, layout) = strided(&view);
    if same_type::<T>(py, output) {
        let out = py
            .detach(|| {
                larmorx_core::parallel::with_threads(n_threads, || {
                    nd::rank_filter::<T, T>(&data, &layout, rank, fp, mode, cval, origins)
                })
            })
            .map_err(threads_err)?
            .map_err(nd_err)?;
        return Ok(to_py(py, &layout.shape, out)?.into_any());
    }
    let out = py
        .detach(|| {
            larmorx_core::parallel::with_threads(n_threads, || {
                nd::rank_filter::<T, f64>(&data, &layout, rank, fp, mode, cval, origins)
            })
        })
        .map_err(threads_err)?
        .map_err(nd_err)?;
    cast_out(py, &layout.shape, out, output, true)
}

/// `_nd_image.rank_filter(input, rank, footprint, output, mode, cval, origins)`.
#[pyfunction]
#[pyo3(signature = (input, rank, footprint, mode, cval, origins, n_threads = 1, output = None))]
#[allow(clippy::too_many_arguments)]
fn ndimage_rank_filter<'py>(
    py: Python<'py>,
    input: &Bound<'py, PyAny>,
    rank: usize,
    footprint: PyReadonlyArrayDyn<'py, bool>,
    mode: &str,
    cval: f64,
    origins: Vec<isize>,
    n_threads: usize,
    output: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let fp = self::footprint(&footprint)?;
    let mode = extend(mode)?;
    let out_name = output_name(input, output)?;
    by_dtype!(
        input, rank_t, py, rank, &fp, mode, cval, &origins, n_threads, &out_name
    )
}

/// `_rank_filter_1d.rank_filter(x, rank, win_len, x_out, mode, cval, origin)` of a float32,
/// float64 or int64 1-D array (`larmorx_ndimage::rank_filter_1d`).
#[pyfunction]
fn ndimage_rank_filter_1d<'py>(
    py: Python<'py>,
    input: &Bound<'py, PyAny>,
    rank: usize,
    win_len: usize,
    mode: &str,
    cval: f64,
    origin: isize,
) -> PyResult<Bound<'py, PyAny>> {
    let mode = extend(mode)?;
    if let Ok(x) = input.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        let v: Vec<f32> = x.as_array().iter().copied().collect();
        let out = py.detach(|| nd::rank_filter_1d(&v, rank, win_len, mode, cval as f32, origin));
        return Ok(to_py(py, &[out.len()], out)?.into_any());
    }
    if let Ok(x) = input.extract::<PyReadonlyArrayDyn<'_, f64>>() {
        let v: Vec<f64> = x.as_array().iter().copied().collect();
        let out = py.detach(|| nd::rank_filter_1d(&v, rank, win_len, mode, cval, origin));
        return Ok(to_py(py, &[out.len()], out)?.into_any());
    }
    let x: PyReadonlyArrayDyn<'_, i64> = input.extract()?;
    let v: Vec<i64> = x.as_array().iter().copied().collect();
    // `PyLong_AsLongLong(cval)`: SciPy passes `x.dtype.type(cval)`, an integer
    let out = py.detach(|| nd::rank_filter_1d(&v, rank, win_len, mode, cval as i64, origin));
    Ok(to_py(py, &[out.len()], out)?.into_any())
}

#[allow(clippy::too_many_arguments)]
fn erosion_with_mask<'py, T: NdElement + Element, M: MaskElement + Element>(
    py: Python<'py>,
    data: &[T],
    layout: &Layout,
    args: BinaryErosion<'_>,
    mask: Option<PyReadonlyArrayDyn<'py, M>>,
    n_threads: usize,
) -> PyResult<Vec<u8>> {
    let mview = mask.as_ref().map(|m| m.as_array());
    let mparts = mview.as_ref().map(strided);
    let m = mparts.as_ref().map(|(d, l)| (&d[..], l));
    py.detach(|| {
        larmorx_core::parallel::with_threads(n_threads, || {
            nd::binary_erosion::<T, M>(data, layout, args, m)
        })
    })
    .map_err(threads_err)?
    .map_err(nd_err)
}

fn erosion_t<'py, T: NdElement + Element>(
    input: PyReadonlyArrayDyn<'py, T>,
    py: Python<'py>,
    args: BinaryErosion<'_>,
    mask: Option<&Bound<'py, PyAny>>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let view = input.as_array();
    let (data, layout) = strided(&view);
    let out = match mask {
        None => erosion_with_mask::<T, u8>(py, &data, &layout, args, None, n_threads)?,
        Some(m) => {
            if let Ok(x) = m.extract::<PyReadonlyArrayDyn<'_, u8>>() {
                erosion_with_mask(py, &data, &layout, args, Some(x), n_threads)?
            } else if let Ok(x) = m.extract::<PyReadonlyArrayDyn<'_, f32>>() {
                erosion_with_mask(py, &data, &layout, args, Some(x), n_threads)?
            } else if let Ok(x) = m.extract::<PyReadonlyArrayDyn<'_, f64>>() {
                erosion_with_mask(py, &data, &layout, args, Some(x), n_threads)?
            } else if let Ok(x) = m.extract::<PyReadonlyArrayDyn<'_, i8>>() {
                erosion_with_mask(py, &data, &layout, args, Some(x), n_threads)?
            } else if let Ok(x) = m.extract::<PyReadonlyArrayDyn<'_, u16>>() {
                erosion_with_mask(py, &data, &layout, args, Some(x), n_threads)?
            } else if let Ok(x) = m.extract::<PyReadonlyArrayDyn<'_, i16>>() {
                erosion_with_mask(py, &data, &layout, args, Some(x), n_threads)?
            } else if let Ok(x) = m.extract::<PyReadonlyArrayDyn<'_, u32>>() {
                erosion_with_mask(py, &data, &layout, args, Some(x), n_threads)?
            } else if let Ok(x) = m.extract::<PyReadonlyArrayDyn<'_, i32>>() {
                erosion_with_mask(py, &data, &layout, args, Some(x), n_threads)?
            } else if let Ok(x) = m.extract::<PyReadonlyArrayDyn<'_, u64>>() {
                erosion_with_mask(py, &data, &layout, args, Some(x), n_threads)?
            } else if let Ok(x) = m.extract::<PyReadonlyArrayDyn<'_, i64>>() {
                erosion_with_mask(py, &data, &layout, args, Some(x), n_threads)?
            } else {
                return Err(PyRuntimeError::new_err("data type not supported"));
            }
        }
    };
    Ok(to_py(py, &layout.shape, out)?.into_any())
}

/// `_binary_erosion(input, structure, iterations, mask, None, border_value, origin, invert,
/// brute_force)` after the wrapper's checks (and, for a dilation, its reversed structure):
/// the result as `uint8` 0/1 values, C-ordered.
#[pyfunction]
#[pyo3(signature = (input, structure, origins, iterations, border_value, invert, brute_force, mask = None, n_threads = 1))]
#[allow(clippy::too_many_arguments)]
fn ndimage_binary_erosion<'py>(
    py: Python<'py>,
    input: &Bound<'py, PyAny>,
    structure: PyReadonlyArrayDyn<'py, bool>,
    origins: Vec<isize>,
    iterations: i64,
    border_value: bool,
    invert: bool,
    brute_force: bool,
    mask: Option<Bound<'py, PyAny>>,
    n_threads: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let st = self::footprint(&structure)?;
    let args = BinaryErosion {
        structure: &st,
        origins: &origins,
        iterations,
        border_value,
        invert,
        brute_force,
    };
    by_dtype!(input, erosion_t, py, args, mask.as_ref(), n_threads)
}

fn label_t<'py, T: NdElement + Element>(
    input: PyReadonlyArrayDyn<'py, T>,
    py: Python<'py>,
    st: &Footprint,
) -> PyResult<(Bound<'py, PyAny>, usize)> {
    let view = input.as_array();
    let (data, layout) = strided(&view);
    let (labels, n) = py
        .detach(|| nd::label(&data, &layout, st))
        .map_err(nd_err)?;
    let arr = match labels {
        Labels::I32(v) => to_py(py, &layout.shape, v)?.into_any(),
        Labels::I64(v) => to_py(py, &layout.shape, v)?.into_any(),
    };
    Ok((arr, n))
}

/// `_ni_label._label(input, structure, output)` with SciPy's default output (`int32`, or `intp`
/// for `2^31 - 2` elements or more): `(labels, number)`.
#[pyfunction]
fn ndimage_label<'py>(
    py: Python<'py>,
    input: &Bound<'py, PyAny>,
    structure: PyReadonlyArrayDyn<'py, bool>,
) -> PyResult<(Bound<'py, PyAny>, usize)> {
    let st = self::footprint(&structure)?;
    by_dtype!(input, label_t, py, &st)
}

fn sum_labels_l<'py, T: NdElement + Element, L: nd::LabelValue + Element>(
    py: Python<'py>,
    data: &[T],
    layout: &Layout,
    labels: PyReadonlyArrayDyn<'py, L>,
    index: &[i64],
) -> PyResult<Vec<f64>> {
    let lv = labels.as_array();
    let (ld, ll) = strided(&lv);
    py.detach(|| nd::sum_labels_bincount(data, layout, &ld, &ll, index))
        .map_err(nd_err)
}

fn sum_labels_t<'py, T: NdElement + Element>(
    input: PyReadonlyArrayDyn<'py, T>,
    py: Python<'py>,
    labels: &Bound<'py, PyAny>,
    index: &[i64],
) -> PyResult<Vec<f64>> {
    let view = input.as_array();
    let (data, layout) = strided(&view);
    if let Ok(l) = labels.extract::<PyReadonlyArrayDyn<'_, i32>>() {
        sum_labels_l(py, &data, &layout, l, index)
    } else if let Ok(l) = labels.extract::<PyReadonlyArrayDyn<'_, i64>>() {
        sum_labels_l(py, &data, &layout, l, index)
    } else if let Ok(l) = labels.extract::<PyReadonlyArrayDyn<'_, u8>>() {
        sum_labels_l(py, &data, &layout, l, index)
    } else if let Ok(l) = labels.extract::<PyReadonlyArrayDyn<'_, i8>>() {
        sum_labels_l(py, &data, &layout, l, index)
    } else if let Ok(l) = labels.extract::<PyReadonlyArrayDyn<'_, u16>>() {
        sum_labels_l(py, &data, &layout, l, index)
    } else if let Ok(l) = labels.extract::<PyReadonlyArrayDyn<'_, i16>>() {
        sum_labels_l(py, &data, &layout, l, index)
    } else if let Ok(l) = labels.extract::<PyReadonlyArrayDyn<'_, u32>>() {
        sum_labels_l(py, &data, &layout, l, index)
    } else if let Ok(l) = labels.extract::<PyReadonlyArrayDyn<'_, u64>>() {
        sum_labels_l(py, &data, &layout, l, index)
    } else {
        Err(PyRuntimeError::new_err("labels must be an integer array"))
    }
}

/// `scipy.ndimage.sum_labels(input, labels, index)` for a sequence `index` (`_stats`' bincount
/// and unique paths): the float64 sum per label, in C order.
#[pyfunction]
fn ndimage_sum_labels<'py>(
    py: Python<'py>,
    input: &Bound<'py, PyAny>,
    labels: &Bound<'py, PyAny>,
    index: Vec<i64>,
) -> PyResult<Vec<f64>> {
    by_dtype!(input, sum_labels_t, py, labels, &index)
}

fn release(name: &str) -> PyResult<NumpyRelease> {
    NumpyRelease::from_version(name)
        .ok_or_else(|| PyValueError::new_err(format!("not a numpy 2 version: {name:?}")))
}

fn pct_err(e: np::quantile::PercentileError) -> PyErr {
    match e {
        np::quantile::PercentileError::Range => PyValueError::new_err(e.to_string()),
        np::quantile::PercentileError::Empty => PyIndexError::new_err(e.to_string()),
    }
}

fn float_values<T: NpFloat + Element>(a: &PyReadonlyArrayDyn<'_, T>) -> Vec<T> {
    a.as_array().iter().copied().collect()
}

/// `numpy.percentile(a, q)` (linear, `axis=None`) with a Python-number `q`, as the numpy release
/// `numpy_version` computes it (`larmorx_core::numpy::quantile::percentile_linear_weak`); a
/// float32 or float64 array; the result in the array's type (as a Python float).
#[pyfunction]
fn numpy_percentile_weak(a: &Bound<'_, PyAny>, q: f64, numpy_version: &str) -> PyResult<f64> {
    let rel = release(numpy_version)?;
    if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        let v = float_values(&x);
        return np::quantile::percentile_linear_weak(&v, q, rel)
            .map(f64::from)
            .map_err(pct_err);
    }
    let x: PyReadonlyArrayDyn<'_, f64> = a.extract()?;
    let v = float_values(&x);
    np::quantile::percentile_linear_weak(&v, q, rel).map_err(pct_err)
}

/// `numpy.percentile(a, q)` (linear, `axis=None`) with a list `q`: float64 results.
#[pyfunction]
fn numpy_percentile(a: &Bound<'_, PyAny>, q: Vec<f64>) -> PyResult<Vec<f64>> {
    if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        return np::quantile::percentile_linear(&float_values(&x), &q).map_err(pct_err);
    }
    let x: PyReadonlyArrayDyn<'_, f64> = a.extract()?;
    np::quantile::percentile_linear(&float_values(&x), &q).map_err(pct_err)
}

fn median_axis_t<'py, T: NpFloat + Element>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, T>,
    axis: Option<usize>,
) -> PyResult<Bound<'py, PyAny>> {
    let view = a.as_array();
    match axis {
        None => {
            let v: Vec<T> = view.iter().copied().collect();
            Ok(to_py(py, &[], vec![np::quantile::median(&v)])?.into_any())
        }
        Some(ax) => {
            // the memory order numpy keeps (`partition` copies with order 'K'): C-order or
            // Fortran-order arrays as (outer, n, inner) of their memory
            let shape = view.shape().to_vec();
            if ax >= shape.len() {
                return Err(PyValueError::new_err("axis out of range"));
            }
            let fortran = !view.is_standard_layout() && view.t().is_standard_layout();
            let mem: Vec<T> = if fortran {
                view.t().iter().copied().collect()
            } else {
                view.iter().copied().collect()
            };
            let mshape: Vec<usize> = if fortran {
                shape.iter().rev().copied().collect()
            } else {
                shape.clone()
            };
            let max = if fortran { shape.len() - 1 - ax } else { ax };
            let outer: usize = mshape[..max].iter().product();
            let inner: usize = mshape[max + 1..].iter().product();
            let m = np::quantile::median_axis(&mem, [outer, mshape[max], inner]);
            let mut oshape = mshape.clone();
            oshape.remove(max);
            let arr = ArrayD::from_shape_vec(IxDyn(&oshape), m)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            let arr = if fortran { arr.reversed_axes() } else { arr };
            Ok(PyArray::from_owned_array(py, arr).into_any())
        }
    }
}

/// `numpy.median(a, axis)` of a float32 or float64 array (C- or Fortran-ordered for an axis).
#[pyfunction]
#[pyo3(signature = (a, axis = None))]
fn numpy_median<'py>(
    py: Python<'py>,
    a: &Bound<'py, PyAny>,
    axis: Option<usize>,
) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        return median_axis_t(py, x, axis);
    }
    let x: PyReadonlyArrayDyn<'_, f64> = a.extract()?;
    median_axis_t(py, x, axis)
}

/// numpy's float32 `exp` (`larmorx_core::numpy::umath::exp_f32`), elementwise, C-ordered.
#[pyfunction]
fn numpy_exp_f32<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let view = a.as_array();
    let shape = view.shape().to_vec();
    let v: Vec<f32> = view.iter().copied().collect();
    let out = py.detach(|| v.into_iter().map(np::umath::exp_f32).collect::<Vec<_>>());
    to_py(py, &shape, out)
}

/// `numpy.clip(a, a_min, a_max)` of a float32 array with float32 bounds, C-ordered.
#[pyfunction]
fn numpy_clip_f32<'py>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, f32>,
    a_min: f32,
    a_max: f32,
) -> PyResult<Bound<'py, PyArrayDyn<f32>>> {
    let view = a.as_array();
    let shape = view.shape().to_vec();
    let mut v: Vec<f32> = view.iter().copied().collect();
    np::umath::clip(&mut v, a_min, a_max);
    to_py(py, &shape, v)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ndimage_min_or_max_filter, m)?)?;
    m.add_function(wrap_pyfunction!(ndimage_min_or_max_filter1d, m)?)?;
    m.add_function(wrap_pyfunction!(ndimage_rank_filter, m)?)?;
    m.add_function(wrap_pyfunction!(ndimage_rank_filter_1d, m)?)?;
    m.add_function(wrap_pyfunction!(ndimage_binary_erosion, m)?)?;
    m.add_function(wrap_pyfunction!(ndimage_label, m)?)?;
    m.add_function(wrap_pyfunction!(ndimage_sum_labels, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_percentile_weak, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_percentile, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_median, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_exp_f32, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_clip_f32, m)?)?;
    Ok(())
}
