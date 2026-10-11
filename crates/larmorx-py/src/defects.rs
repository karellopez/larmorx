// SPDX-License-Identifier: Apache-2.0
//! Bindings for `larmorx_core::defects` (`larmorx._defects`): the D13 switch and its notices.

use larmorx_core::defects::{DEFECTS, Notice, UpstreamDefects};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// `"fix"` or `"reproduce"` as [`UpstreamDefects`].
pub(crate) fn mode_from_py(name: &str) -> PyResult<UpstreamDefects> {
    UpstreamDefects::parse(name).ok_or_else(|| {
        PyValueError::new_err(format!(
            "upstream_defects must be 'fix' or 'reproduce', not {name:?}"
        ))
    })
}

/// A notice as Python receives it: `(entry, tool, mode, detail)`.
pub(crate) type PyNotice = (&'static str, &'static str, &'static str, String);

pub(crate) fn notices_to_py(notices: Vec<Notice>) -> Vec<PyNotice> {
    notices
        .into_iter()
        .map(|n| (n.defect.id, n.defect.tool, n.mode.name(), n.detail))
        .collect()
}

/// Every register entry larmorx has a switch for: `[(id, tool, what, fixed_by_default)]`.
#[pyfunction]
fn upstream_defects() -> Vec<(&'static str, &'static str, &'static str, bool)> {
    DEFECTS
        .iter()
        .map(|d| (d.id, d.tool, d.what, d.fixed_by_default))
        .collect()
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(upstream_defects, m)?)?;
    Ok(())
}
