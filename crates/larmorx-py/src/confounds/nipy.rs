// SPDX-License-Identifier: Apache-2.0
//! Binding of nipype's `normalize_mc_params(params, "NIPY")` (nipy 0.6.1's `to_matrix44` and
//! `aff2euler`), as `larmorx_confounds::nipy` computes it: `nipy_motion_params`, which
//! `larmorx.confounds.normalize_mc_params` and `framewise_displacement` call. The validation
//! (`validation/investigations/confounds/check_nipype_misc.py`, section `nipy`) compares it
//! with each environment's nipype running nipy 0.6.1's own functions. The GIL is released while
//! computing.

use super::dvars::numpy_settings;
use super::svd::{Mat, linalg_error, mat};
use crate::defects::mode_from_py;
use larmorx_confounds::nipy::{self, NipyError};
use numpy::PyReadonlyArray2;
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;

/// nipype's `numpy.apply_along_axis(normalize_mc_params, 1, rows, source="NIPY")` of an
/// `n × k` float64 table (any strides): `(params, other)`, `params` the `n × 6` parameters in
/// `upstream_defects`' mode with numpy `numpy`'s LAPACK, `other` the other mode's where some row
/// differs in a bit (GLIBC-07's exact notice), else `None`. The first row nipy cannot compute
/// raises numpy's `IndexError` or `LinAlgError` with numpy's message.
#[pyfunction]
#[pyo3(signature = (rows, numpy = "2.5.4", upstream_defects = "fix"))]
fn nipy_motion_params<'py>(
    py: Python<'py>,
    rows: PyReadonlyArray2<'py, f64>,
    numpy: &str,
    upstream_defects: &str,
) -> PyResult<(Mat<'py>, Option<Mat<'py>>)> {
    let mode = mode_from_py(upstream_defects)?;
    let (_, _, lapack) = numpy_settings(numpy)?;
    let arr = rows.as_array();
    let (n, k) = arr.dim();
    let data: Vec<f64> = arr.iter().copied().collect();
    let out = py
        .detach(|| nipy::normalize_rows(&data, n, k, lapack.release, mode))
        .map_err(|e| match e.error {
            NipyError::Index(_) => PyIndexError::new_err(e.error.to_string()),
            NipyError::Svd => linalg_error(py, &e.error.to_string()),
        })?;
    let other = out.other.map(|o| mat(py, n, 6, o)).transpose()?;
    Ok((mat(py, n, 6, out.params)?, other))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(nipy_motion_params, m)?)?;
    Ok(())
}
