// SPDX-License-Identifier: Apache-2.0
//! Binding of numpy's float64 `a ** o` for the integer exponents of niworkflows'
//! `exponential_terms`, as `larmorx_confounds::expansion` computes it: `numpy_power_f64`, which
//! `larmorx.niworkflows.confounds` calls for every power other than 1. The validation
//! (`validation/investigations/confounds/check_model.py`) compares it with each environment's
//! numpy and pandas. The GIL is released while computing.

use crate::defects::mode_from_py;
use larmorx_confounds::expansion;
use numpy::{PyArray1, PyReadonlyArray1};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// numpy's float64 `a ** exponent` (a non-negative integral `exponent`, as niworkflows'
/// formulas give it): `(values, differ)`, the values in `upstream_defects`' mode (`"fix"`: the
/// correctly rounded `pow`; `"reproduce"`: glibc 2.35's, numpy's on CPUs without AVX-512) and
/// the number of values whose bits differ between the two modes. One thread: the confounds'
/// tables have at most a few thousand values per column.
#[pyfunction]
#[pyo3(signature = (a, exponent, upstream_defects = "fix"))]
fn numpy_power_f64<'py>(
    py: Python<'py>,
    a: PyReadonlyArray1<'py, f64>,
    exponent: f64,
    upstream_defects: &str,
) -> PyResult<(Bound<'py, PyArray1<f64>>, usize)> {
    let mode = mode_from_py(upstream_defects)?;
    if !(exponent >= 0.0 && exponent.fract() == 0.0) {
        return Err(PyValueError::new_err(format!(
            "the exponent must be a non-negative integer, not {exponent}"
        )));
    }
    let values: Vec<f64> = a.as_array().iter().copied().collect();
    let (out, differ) = py.detach(|| expansion::power(&values, exponent, mode));
    Ok((PyArray1::from_vec(py, out), differ))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(numpy_power_f64, m)?)?;
    Ok(())
}
