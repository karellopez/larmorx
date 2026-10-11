// SPDX-License-Identifier: Apache-2.0
//! Bindings for `larmorx_cli::replica`: the registry of tools with a replica in another
//! package, and the search for the replica's program (`larmorx/_replica.py`).
//!
//! Nothing here loads a replica: the Python wrappers run its program as a separate process.

use std::path::PathBuf;

use larmorx_cli::replica::{self, Implementation};
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// `"auto"`, `"replica"` or `"original"`.
pub fn implementation(name: &str) -> PyResult<Implementation> {
    Implementation::parse(name).ok_or_else(|| {
        PyValueError::new_err(format!(
            "implementation must be 'auto', 'replica' or 'original', not {name:?}"
        ))
    })
}

/// Every tool with a replica: dicts with `family`, `tool`, `package`, `program`, `env`,
/// `licence` and `install`.
#[pyfunction]
fn replica_registry(py: Python<'_>) -> PyResult<Vec<Bound<'_, PyDict>>> {
    replica::REPLICAS
        .iter()
        .map(|r| {
            let d = PyDict::new(py);
            d.set_item("family", r.family)?;
            d.set_item("tool", r.tool)?;
            d.set_item("package", r.package.name)?;
            d.set_item("program", r.package.program)?;
            d.set_item("env", r.package.env)?;
            d.set_item("licence", r.package.licence)?;
            d.set_item("install", r.package.install)?;
            Ok(d)
        })
        .collect()
}

/// The path of the replica program of `family tool`: its package's environment variable,
/// else the first match in `dirs`, else on `PATH`; `None` if not found. `ValueError` if the
/// environment variable names something that is not a file; `KeyError` if the tool has no
/// replica.
#[pyfunction]
fn replica_find(
    py: Python<'_>,
    family: &str,
    tool: &str,
    dirs: Vec<PathBuf>,
) -> PyResult<Option<PathBuf>> {
    let r = replica::lookup(family, tool)
        .ok_or_else(|| PyKeyError::new_err(format!("{family} {tool} has no replica")))?;
    py.detach(|| replica::find(r.package, &dirs))
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(replica_registry, m)?)?;
    m.add_function(wrap_pyfunction!(replica_find, m)?)?;
    Ok(())
}
