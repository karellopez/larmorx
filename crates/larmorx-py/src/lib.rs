// SPDX-License-Identifier: Apache-2.0
//! The `larmorx._core` extension module: Python bindings of the larmorx crates.
//!
//! Built by maturin into the `larmorx` wheel (abi3, CPython >= 3.12). The idiomatic Python
//! API lives in `python/larmorx`; this module only exposes the compiled entry points.
#![forbid(unsafe_code)]

use pyo3::prelude::*;

mod afni;
mod ants;
mod ants_filters;
mod mri;
mod ndimage;
mod nifti;
mod resample;

/// Runs the `larmorx` command line with `argv` (program name first).
///
/// Returns `(exit_code, stdout, stderr)`; the Python caller writes the streams so that they go
/// through `sys.stdout` and `sys.stderr`. The GIL is released while the command runs.
#[pyfunction]
fn cli_main(py: Python<'_>, argv: Vec<String>) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = py.detach(|| larmorx_cli::run_with(&argv, &ants::PyLoader, &mut out, &mut err));
    (
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

/// The command line's tool families and their tools: `[(family, [tool, ...]), ...]`
/// (`larmorx_cli::FAMILIES`). The golden coverage test enumerates them.
#[pyfunction]
fn cli_tools() -> Vec<(&'static str, Vec<&'static str>)> {
    larmorx_cli::FAMILIES
        .iter()
        .map(|(family, tools)| (*family, tools.to_vec()))
        .collect()
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", larmorx_core::VERSION)?;
    m.add_function(wrap_pyfunction!(cli_main, m)?)?;
    m.add_function(wrap_pyfunction!(cli_tools, m)?)?;
    nifti::register(m)?;
    ants::register(m)?;
    ants_filters::register(m)?;
    afni::register(m)?;
    mri::register(m)?;
    ndimage::register(m)?;
    resample::register(m)?;
    Ok(())
}
