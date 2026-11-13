// SPDX-License-Identifier: Apache-2.0
//! Binding of pandas' default float parser of `read_csv` (`precise_xstrtod`, pandas 3.0.6 and
//! 2.2.3), as `larmorx_confounds::pandas` computes it: `pandas_try_double`, which
//! `larmorx.confounds._tables` calls for every column it reads. The validation
//! (`validation/investigations/confounds/check_tables.py`) compares it with each environment's
//! pandas. The GIL is released while parsing.

use larmorx_confounds::pandas::{self, PandasRelease};
use numpy::PyArray1;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

type Pair<'py> = (Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>);

/// pandas' `_try_double` of one column's tokens (the default missing values filtered, the
/// infinity words, `decimal='.'`, no thousands separator): `(pandas' values, the correctly
/// rounded values)`, or `None` when a token is not a float (pandas then tries the next type).
#[pyfunction]
#[pyo3(signature = (tokens, pandas_version = "3.0.6"))]
fn pandas_try_double<'py>(
    py: Python<'py>,
    tokens: Vec<String>,
    pandas_version: &str,
) -> PyResult<Option<Pair<'py>>> {
    let release = PandasRelease::parse(pandas_version).ok_or_else(|| {
        PyValueError::new_err(format!(
            "not a pandas 2.2 or 3.0 version: {pandas_version:?}"
        ))
    })?;
    let out = py.detach(|| pandas::try_double(tokens.iter().map(|t| t.as_bytes()), release));
    Ok(out.map(|(pd, ex)| (PyArray1::from_vec(py, pd), PyArray1::from_vec(py, ex))))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(pandas_try_double, m)?)?;
    Ok(())
}
