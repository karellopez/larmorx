//! The `larmorx._core` extension module: Python bindings of the larmorx crates.
//!
//! Built by maturin into the `larmorx` wheel (abi3, CPython >= 3.12). The idiomatic Python
//! API lives in `python/larmorx`; this module only exposes the compiled entry points.
#![forbid(unsafe_code)]

use pyo3::prelude::*;

mod nifti;

/// Runs the `larmorx` command line with `argv` (program name first).
///
/// Returns `(exit_code, stdout, stderr)`; the Python caller writes the streams so that they go
/// through `sys.stdout` and `sys.stderr`. The GIL is released while the command runs.
#[pyfunction]
fn cli_main(py: Python<'_>, argv: Vec<String>) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = py.detach(|| larmorx_cli::run(&argv, &mut out, &mut err));
    (
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", larmorx_core::VERSION)?;
    m.add_function(wrap_pyfunction!(cli_main, m)?)?;
    nifti::register(m)?;
    Ok(())
}
