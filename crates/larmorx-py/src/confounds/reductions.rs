// SPDX-License-Identifier: Apache-2.0
//! Bindings of numpy's reductions and order statistics at the confounds' call sites, as
//! `larmorx_core::numpy` computes them (`reduce`, `quantile`, `umath`, `polynomial`): sums,
//! means, variances and standard deviations over an axis or a whole array in a release's order
//! (`numpy_reduce`), `percentile(..., method="lower" | "higher")` (`numpy_percentile_rank`),
//! `min`/`max` (`numpy_extreme`), `cumsum`, `searchsorted`, `nan_to_num`, `diff` and
//! `Legendre.basis(deg)(x)`. Arrays cross as C- or Fortran-contiguous float32 or float64 arrays
//! of any number of axes; numpy's summation order follows the memory order (pairwise along the
//! innermost axis in memory, in index order along an outer one), so the bindings take the
//! array's own layout, as numpy does. The validation (`validation/investigations/confounds/
//! check_numpy_reductions.py`) builds every operand the way nipype, niworkflows and fMRIPrep
//! build it and compares the bits.

use super::{one_of, value_err};
use larmorx_core::numpy::polynomial::{self, LegvalForm};
use larmorx_core::numpy::quantile::{self, PercentileError, RankMethod, Side};
use larmorx_core::numpy::reduce::{self, Extreme, SumOrder};
use larmorx_core::numpy::{NpFloat, NumpyRelease, umath};
use numpy::ndarray::{ArrayD, IxDyn};
use numpy::{Element, PyArray, PyArray1, PyArrayDyn, PyReadonlyArray1, PyReadonlyArrayDyn};
use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::*;

fn order(v: &str) -> PyResult<SumOrder> {
    SumOrder::from_version(v)
        .ok_or_else(|| PyValueError::new_err(format!("not a numpy version: {v:?}")))
}

fn release(v: &str) -> PyResult<NumpyRelease> {
    NumpyRelease::from_version(v)
        .ok_or_else(|| PyValueError::new_err(format!("not a numpy 2 version: {v:?}")))
}

fn pct_err(e: PercentileError) -> PyErr {
    match e {
        PercentileError::Range => PyValueError::new_err(e.to_string()),
        PercentileError::Empty => PyIndexError::new_err(e.to_string()),
    }
}

/// A C- or Fortran-contiguous array as numpy iterates it: its values in memory order, its shape
/// in memory order (the axes reversed for Fortran order) and whether it is Fortran-ordered (a
/// one-axis or both-ordered array counts as C).
struct Memory<T> {
    values: Vec<T>,
    shape: Vec<usize>,
    fortran: bool,
}

fn memory<T: Element + Copy>(a: &PyReadonlyArrayDyn<'_, T>) -> PyResult<Memory<T>> {
    let v = a.as_array();
    let shape = v.shape().to_vec();
    if v.is_standard_layout() {
        return Ok(Memory {
            values: v.iter().copied().collect(),
            shape,
            fortran: false,
        });
    }
    if v.t().is_standard_layout() {
        return Ok(Memory {
            values: v.t().iter().copied().collect(),
            shape: shape.iter().rev().copied().collect(),
            fortran: true,
        });
    }
    Err(PyValueError::new_err(
        "the array must be C- or Fortran-contiguous (numpy's order of a strided view is not \
         followed)",
    ))
}

impl<T> Memory<T> {
    /// `(outer, n, inner)` of numpy axis `axis` in memory order, and the memory-order shape
    /// without it.
    fn split(&self, axis: usize) -> PyResult<([usize; 3], Vec<usize>)> {
        let nd = self.shape.len();
        if axis >= nd {
            return Err(PyValueError::new_err(format!(
                "axis {axis} is out of bounds for array of dimension {nd}"
            )));
        }
        let m = if self.fortran { nd - 1 - axis } else { axis };
        let outer = self.shape[..m].iter().product();
        let inner = self.shape[m + 1..].iter().product();
        let mut rest = self.shape.clone();
        rest.remove(m);
        Ok(([outer, self.shape[m], inner], rest))
    }

    /// An array of numpy's shape from values in this layout's memory order.
    fn array<'py, U: Element>(
        &self,
        py: Python<'py>,
        mshape: &[usize],
        v: Vec<U>,
    ) -> PyResult<Bound<'py, PyArrayDyn<U>>> {
        let a = ArrayD::from_shape_vec(IxDyn(mshape), v).map_err(value_err)?;
        let a = if self.fortran { a.reversed_axes() } else { a };
        Ok(PyArray::from_owned_array(py, a))
    }
}

fn reduce_t<'py, T: NpFloat + Element>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, T>,
    op: &str,
    axis: Option<usize>,
    ord: SumOrder,
    ddof: i64,
) -> PyResult<Bound<'py, PyAny>> {
    #[derive(Clone, Copy)]
    enum Op {
        Sum,
        Mean,
        Var,
        Std,
    }
    let op = one_of(
        "op",
        op,
        &[
            ("sum", Op::Sum),
            ("mean", Op::Mean),
            ("var", Op::Var),
            ("std", Op::Std),
        ],
    )?;
    let mem = memory(&a)?;
    match axis {
        None => {
            let v = &mem.values;
            let r = py.detach(|| match op {
                Op::Sum => reduce::add_reduce_ordered(v, ord),
                Op::Mean => reduce::mean_ordered(v, ord),
                Op::Var => reduce::var_ordered(v, ddof, ord),
                Op::Std => reduce::std_ordered(v, ddof, ord),
            });
            Ok(mem.array(py, &[], vec![r])?.into_any())
        }
        Some(ax) => {
            let (shape, rest) = mem.split(ax)?;
            let v = &mem.values;
            let r = py.detach(|| match op {
                Op::Sum => reduce::add_reduce_axis_in(v, shape, ord),
                Op::Mean => reduce::mean_axis_in(v, shape, ord),
                Op::Var => reduce::var_axis_in(v, shape, ddof, ord),
                Op::Std => reduce::std_axis_in(v, shape, ddof, ord),
            });
            Ok(mem.array(py, &rest, r)?.into_any())
        }
    }
}

/// `a.sum(axis)`, `a.mean(axis)`, `a.var(axis, ddof=ddof)` or `a.std(axis, ddof=ddof)` (`op`:
/// `"sum"`, `"mean"`, `"var"`, `"std"`) of a C- or Fortran-contiguous float32 or float64 array,
/// over one axis or the whole array (`axis=None`), in the type of `a` and the summation order of
/// numpy release `numpy_version` (`larmorx_core::numpy::reduce`: pairwise along the innermost
/// axis in memory, chunks of 8192 in numpy 2.2; in index order along an outer axis). The result
/// has numpy's shape (a 0-d array for `axis=None`).
#[pyfunction]
#[pyo3(signature = (a, op, axis = None, numpy_version = "2.5.4", ddof = 0))]
fn numpy_reduce<'py>(
    py: Python<'py>,
    a: &Bound<'py, PyAny>,
    op: &str,
    axis: Option<usize>,
    numpy_version: &str,
    ddof: i64,
) -> PyResult<Bound<'py, PyAny>> {
    let ord = order(numpy_version)?;
    if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        return reduce_t(py, x, op, axis, ord, ddof);
    }
    reduce_t(
        py,
        a.extract::<PyReadonlyArrayDyn<'_, f64>>()?,
        op,
        axis,
        ord,
        ddof,
    )
}

fn rank_t<'py, T: NpFloat + Element>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, T>,
    q: f64,
    axis: Option<usize>,
    method: RankMethod,
    rel: NumpyRelease,
) -> PyResult<Bound<'py, PyAny>> {
    let mem = memory(&a)?;
    let (shape, rest) = match axis {
        None => ([1, mem.values.len(), 1], vec![]),
        Some(ax) => mem.split(ax)?,
    };
    let v = &mem.values;
    let r = py
        .detach(|| quantile::percentile_rank_axis(v, shape, q, method, rel))
        .map_err(pct_err)?;
    Ok(mem.array(py, &rest, r)?.into_any())
}

/// `numpy.percentile(a, q, axis, method=method)` for `method` `"lower"` or `"higher"` and `q` a
/// Python number, of a C- or Fortran-contiguous float32 or float64 array, as numpy release
/// `numpy_version` computes the rank (`larmorx_core::numpy::quantile::percentile_rank_axis`);
/// the result in the array's type, numpy's shape.
#[pyfunction]
#[pyo3(signature = (a, q, axis = None, method = "lower", numpy_version = "2.5.4"))]
fn numpy_percentile_rank<'py>(
    py: Python<'py>,
    a: &Bound<'py, PyAny>,
    q: f64,
    axis: Option<usize>,
    method: &str,
    numpy_version: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let m = one_of(
        "method",
        method,
        &[("lower", RankMethod::Lower), ("higher", RankMethod::Higher)],
    )?;
    let rel = release(numpy_version)?;
    if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        return rank_t(py, x, q, axis, m, rel);
    }
    rank_t(
        py,
        a.extract::<PyReadonlyArrayDyn<'_, f64>>()?,
        q,
        axis,
        m,
        rel,
    )
}

fn extreme_t<T: NpFloat + Element>(a: PyReadonlyArrayDyn<'_, T>, which: Extreme) -> PyResult<f64> {
    let mem = memory(&a)?;
    reduce::extreme(&mem.values, which)
        .map(NpFloat::to_f64)
        .ok_or_else(|| {
            PyValueError::new_err(format!(
                "zero-size array to reduction operation {} which has no identity",
                match which {
                    Extreme::Min => "minimum",
                    Extreme::Max => "maximum",
                }
            ))
        })
}

/// `a.min()` or `a.max()` (`which`: `"min"`, `"max"`) of a float32 or float64 array
/// (`larmorx_core::numpy::reduce::extreme`: a NaN propagates; `-0.0 < +0.0`, where numpy's sign
/// depends on the CPU's SIMD kernel); the value in the array's type, as a Python float.
#[pyfunction]
fn numpy_extreme(a: &Bound<'_, PyAny>, which: &str) -> PyResult<f64> {
    let w = one_of(
        "which",
        which,
        &[("min", Extreme::Min), ("max", Extreme::Max)],
    )?;
    if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        return extreme_t(x, w);
    }
    extreme_t(a.extract::<PyReadonlyArrayDyn<'_, f64>>()?, w)
}

/// `numpy.cumsum(a)` of a one-dimensional float32 or float64 array (`add.accumulate` in index
/// order, in the array's type).
#[pyfunction]
fn numpy_cumsum<'py>(py: Python<'py>, a: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(x) = a.extract::<PyReadonlyArray1<'_, f32>>() {
        let v: Vec<f32> = x.as_array().iter().copied().collect();
        return Ok(PyArray1::from_vec(py, reduce::cumsum(&v)).into_any());
    }
    let x: PyReadonlyArray1<'_, f64> = a.extract()?;
    let v: Vec<f64> = x.as_array().iter().copied().collect();
    Ok(PyArray1::from_vec(py, reduce::cumsum(&v)).into_any())
}

/// `numpy.searchsorted(a, v, side)` of a one-dimensional float64 array and one key, with numpy
/// release `numpy_version`'s bisection (`larmorx_core::numpy::quantile::searchsorted`).
#[pyfunction]
#[pyo3(signature = (a, v, side = "left", numpy_version = "2.5.4"))]
fn numpy_searchsorted(
    a: PyReadonlyArray1<'_, f64>,
    v: f64,
    side: &str,
    numpy_version: &str,
) -> PyResult<usize> {
    let s = one_of(
        "side",
        side,
        &[("left", Side::Left), ("right", Side::Right)],
    )?;
    let vals: Vec<f64> = a.as_array().iter().copied().collect();
    Ok(quantile::searchsorted(&vals, v, s, release(numpy_version)?))
}

fn nan_to_num_t<'py, T: NpFloat + Element>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, T>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut mem = memory(&a)?;
    umath::nan_to_num(&mut mem.values);
    let shape = mem.shape.clone();
    let v = std::mem::take(&mut mem.values);
    Ok(mem.array(py, &shape, v)?.into_any())
}

/// `numpy.nan_to_num(a)` of a C- or Fortran-contiguous float32 or float64 array (NaN to `0`,
/// `±inf` to the type's largest finite values), in the same layout.
#[pyfunction]
fn numpy_nan_to_num<'py>(py: Python<'py>, a: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        return nan_to_num_t(py, x);
    }
    nan_to_num_t(py, a.extract::<PyReadonlyArrayDyn<'_, f64>>()?)
}

fn diff_t<'py, T: NpFloat + Element>(
    py: Python<'py>,
    a: PyReadonlyArrayDyn<'py, T>,
    n: usize,
    axis: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let mem = memory(&a)?;
    let (shape, _) = mem.split(axis)?;
    let m = if mem.fortran {
        mem.shape.len() - 1 - axis
    } else {
        axis
    };
    let mut mshape = mem.shape.clone();
    mshape[m] = mshape[m].saturating_sub(n);
    let r = umath::diff_axis(&mem.values, shape, n);
    Ok(mem.array(py, &mshape, r)?.into_any())
}

/// `numpy.diff(a, n, axis)` of a C- or Fortran-contiguous float32 or float64 array.
#[pyfunction]
#[pyo3(signature = (a, n = 1, axis = 0))]
fn numpy_diff<'py>(
    py: Python<'py>,
    a: &Bound<'py, PyAny>,
    n: usize,
    axis: usize,
) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(x) = a.extract::<PyReadonlyArrayDyn<'_, f32>>() {
        return diff_t(py, x, n, axis);
    }
    diff_t(py, a.extract::<PyReadonlyArrayDyn<'_, f64>>()?, n, axis)
}

/// `numpy.polynomial.Legendre.basis(deg)(x)` of a float64 vector, in numpy release
/// `numpy_version`'s form of `legval` (`larmorx_core::numpy::polynomial`).
#[pyfunction]
#[pyo3(signature = (deg, x, numpy_version = "2.5.4"))]
fn numpy_legendre_basis<'py>(
    py: Python<'py>,
    deg: usize,
    x: PyReadonlyArray1<'_, f64>,
    numpy_version: &str,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let form = LegvalForm::from_version(numpy_version)
        .ok_or_else(|| PyValueError::new_err(format!("not a numpy version: {numpy_version:?}")))?;
    let v: Vec<f64> = x.as_array().iter().copied().collect();
    Ok(PyArray1::from_vec(
        py,
        polynomial::legendre_basis(deg, &v, form),
    ))
}

/// `numpy.linspace(start, stop, num)` of float64 bounds (`larmorx_core::numpy::histograms::
/// linspace`).
#[pyfunction]
fn numpy_linspace(py: Python<'_>, start: f64, stop: f64, num: usize) -> Bound<'_, PyArray1<f64>> {
    PyArray1::from_vec(
        py,
        larmorx_core::numpy::histograms::linspace(start, stop, num),
    )
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(numpy_reduce, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_percentile_rank, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_extreme, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_cumsum, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_searchsorted, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_nan_to_num, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_diff, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_legendre_basis, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_linspace, m)?)?;
    Ok(())
}
