// SPDX-License-Identifier: Apache-2.0
//! Bindings for `larmorx_cli::replica`: the registry of tools with a replica, the search for a
//! separate replica's program, and the run of a built-in replica (`larmorx/_replica.py`).
//!
//! A built-in replica (AFNI's programs, `larmorx-afni-replica`, Apache-2.0 with CC BY 4.0's
//! attribution) runs inside this process; a replica in another package (`larmorx-nc`) is never
//! loaded: the Python wrappers run its program as a separate process.

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

/// Every tool with a replica: dicts with `family`, `tool`, `builtin` (whether the replica is
/// part of larmorx and runs in this process), `package`, `program`, `env`, `licence`, `install`
/// and `original` (whether larmorx has a clean-room original of it). A built-in replica's
/// `package` is `"larmorx"`, and its `program`, `env` and `install` are `None`.
#[pyfunction]
fn replica_registry(py: Python<'_>) -> PyResult<Vec<Bound<'_, PyDict>>> {
    replica::REPLICAS
        .iter()
        .map(|r| {
            let d = PyDict::new(py);
            d.set_item("family", r.family)?;
            d.set_item("tool", r.tool)?;
            let package = r.package();
            d.set_item("builtin", package.is_none())?;
            d.set_item("package", package.map_or("larmorx", |p| p.name))?;
            d.set_item("program", package.map(|p| p.program))?;
            d.set_item("env", package.map(|p| p.env))?;
            d.set_item("licence", r.licence())?;
            d.set_item("install", package.map(|p| p.install))?;
            d.set_item("original", r.original)?;
            Ok(d)
        })
        .collect()
}

/// The path of the replica program of `family tool`: its package's environment variable,
/// else the first match in `dirs`, else on `PATH`; `None` if not found or built in.
/// `ValueError` if the environment variable names something that is not a file; `KeyError` if
/// the tool has no replica.
#[pyfunction]
fn replica_find(
    py: Python<'_>,
    family: &str,
    tool: &str,
    dirs: Vec<PathBuf>,
) -> PyResult<Option<PathBuf>> {
    let r = replica::lookup(family, tool)
        .ok_or_else(|| PyKeyError::new_err(format!("{family} {tool} has no replica")))?;
    let Some(package) = r.package() else {
        return Ok(None);
    };
    py.detach(|| replica::find(package, &dirs))
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Runs the built-in replica of `family tool` with `args` inside this process, without the
/// GIL: `(exit code, standard output, standard error)`. `env` stands for the process
/// environment (`LARMORX_UPSTREAM_DEFECTS`, the upstream's variables, `OMP_NUM_THREADS`); the
/// process's own is neither read nor changed. Relative file names start in `cwd`. `KeyError`
/// if the tool has no built-in replica.
#[pyfunction]
#[pyo3(signature = (family, tool, args, env, cwd = None))]
fn replica_run_builtin(
    py: Python<'_>,
    family: &str,
    tool: &str,
    args: Vec<String>,
    env: Vec<(String, String)>,
    cwd: Option<PathBuf>,
) -> PyResult<(u8, String, String)> {
    let builtin = replica::lookup(family, tool).is_some_and(|r| r.package().is_none());
    if !builtin {
        return Err(PyKeyError::new_err(format!(
            "{family} {tool} has no built-in replica"
        )));
    }
    let (code, out, err) = py.detach(|| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = replica::run_builtin_in(family, tool, &args, env, cwd, &mut out, &mut err);
        (code, out, err)
    });
    let code = code
        .ok_or_else(|| PyKeyError::new_err(format!("{family} {tool} has no built-in replica")))?;
    Ok((
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    ))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(replica_registry, m)?)?;
    m.add_function(wrap_pyfunction!(replica_find, m)?)?;
    m.add_function(wrap_pyfunction!(replica_run_builtin, m)?)?;
    Ok(())
}
