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
mod confounds;
mod defects;
mod fsl;
mod hdf5;
mod mri;
mod ndfilters;
mod ndimage;
mod nifti;
mod replica;
mod resample;
mod sdcflows;
mod skimage;
mod stream;

/// Runs the `larmorx` command line with `argv` (program name first).
///
/// Tools with a replica run it when `implementation` (`"auto"`, `"replica"`, `"clean-room"`,
/// or `"original"`, its old spelling; `None` reads `LARMORX_IMPLEMENTATION`; `--implementation`
/// in `argv` wins) says so: built in, in this process (AFNI's programs), or as a separate
/// program, looked for through its environment variable, then in `replica_dirs`, then on
/// `PATH`.
///
/// Returns `(exit_code, stdout, stderr)`; the Python caller writes the streams so that they go
/// through `sys.stdout` and `sys.stderr`. With `stream`, the command's output goes to
/// `sys.stdout` and `sys.stderr` while it runs instead (a replica program's too: the console
/// script), and both strings are empty; an exception a write raised is raised when the command
/// has ended. The GIL is released while the command runs.
#[pyfunction]
#[pyo3(signature = (argv, replica_dirs = Vec::new(), implementation = None, stream = false))]
fn cli_main(
    py: Python<'_>,
    argv: Vec<String>,
    replica_dirs: Vec<std::path::PathBuf>,
    implementation: Option<&str>,
    stream: bool,
) -> PyResult<(u8, String, String)> {
    let mut options = larmorx_cli::Options {
        loader: &larmorx_cli::FileLoader,
        implementation: implementation.map(replica::implementation).transpose()?,
        replica_dirs,
        replica_output: larmorx_cli::replica::Output::Capture,
    };
    if stream {
        options.replica_output = larmorx_cli::replica::Output::Stream;
        let (mut out, mut err) = (stream::PyStream::stdout(), stream::PyStream::stderr());
        let code = py.detach(|| larmorx_cli::run_with(&argv, &options, &mut out, &mut err));
        let failed = (out.finish(), err.finish());
        if let Some(e) = failed.0.or(failed.1) {
            return Err(e);
        }
        return Ok((code, String::new(), String::new()));
    }
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = py.detach(|| larmorx_cli::run_with(&argv, &options, &mut out, &mut err));
    Ok((
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    ))
}

/// `argv` (program name first) when it names a Python program of the command line (sdcflows'
/// BIDS app) after larmorx's own options: `(family, tool, arguments, environment changes)`,
/// each change `(name, value)` or `(name, None)` to remove the variable; else `None` (also when
/// the options are wrong: [`cli_main`] reports it). The arguments are parsed here, once, for the
/// console script and the standalone program alike.
#[pyfunction]
#[allow(clippy::type_complexity)]
fn cli_python_program(
    argv: Vec<String>,
) -> Option<(
    String,
    String,
    Vec<String>,
    Vec<(&'static str, Option<String>)>,
)> {
    larmorx_cli::python_program(&argv).map(|p| (p.family, p.tool, p.args, p.environment))
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
    m.add_function(wrap_pyfunction!(cli_python_program, m)?)?;
    nifti::register(m)?;
    ants::register(m)?;
    ants_filters::register(m)?;
    afni::register(m)?;
    confounds::register(m)?;
    defects::register(m)?;
    fsl::register(m)?;
    hdf5::register(m)?;
    mri::register(m)?;
    ndimage::register(m)?;
    ndfilters::register(m)?;
    skimage::register(m)?;
    replica::register(m)?;
    resample::register(m)?;
    sdcflows::register(m)?;
    Ok(())
}
