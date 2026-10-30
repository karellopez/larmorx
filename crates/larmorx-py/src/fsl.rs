// SPDX-License-Identifier: Apache-2.0
//! Bindings of the clean-room FSL-compatible tools (`larmorx-fsl`): BET's original, run as its
//! command line inside this process.

use std::path::PathBuf;

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;

/// Runs the clean-room original of `fsl tool` (`bet` or `bet2`) with FSL's arguments inside
/// this process, without the GIL: `(exit code, standard output, standard error)`. `env` stands
/// for the process environment (`FSLOUTPUTTYPE`, `FSL_LOAD_NIFTI_EXTENSIONS`,
/// `LARMORX_UPSTREAM_DEFECTS`, `OMP_NUM_THREADS`); the process's own is neither read nor
/// changed. Relative file names start in `cwd`. `KeyError` for another tool.
#[pyfunction]
#[pyo3(signature = (tool, args, env, cwd = None))]
fn fsl_run_original(
    py: Python<'_>,
    tool: &str,
    args: Vec<String>,
    env: Vec<(String, String)>,
    cwd: Option<PathBuf>,
) -> PyResult<(u8, String, String)> {
    if !larmorx_fsl::cli::TOOLS.contains(&tool) {
        return Err(PyKeyError::new_err(format!(
            "fsl {tool} has no clean-room original"
        )));
    }
    let (code, out, err) = py.detach(|| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = larmorx_fsl::cli::run_in(tool, &args, env, cwd, &mut out, &mut err);
        (code.unwrap_or(1), out, err)
    });
    Ok((
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    ))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(fsl_run_original, m)?)?;
    Ok(())
}
