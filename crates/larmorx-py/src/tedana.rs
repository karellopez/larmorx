// SPDX-License-Identifier: Apache-2.0
//! The clean-room `t2smap` (`larmorx_tedana`), bound on its own until it is wired into the
//! shared implementation choice (`lx.tedana.t2smap(..., implementation="clean-room")`).

use std::path::PathBuf;

use pyo3::prelude::*;

/// Runs the clean-room `t2smap` with tedana's arguments in this process, without the GIL:
/// `(exit code, standard output, standard error)`. `env` stands for the process environment
/// (never read or changed); relative names start in `cwd`.
#[pyfunction]
#[pyo3(signature = (args, env, cwd = None))]
fn tedana_t2smap_cleanroom(
    py: Python<'_>,
    args: Vec<String>,
    env: Vec<(String, String)>,
    cwd: Option<PathBuf>,
) -> PyResult<(u8, String, String)> {
    let settings = larmorx_tedana::cli::Settings {
        vars: env.into_iter().collect(),
        dir: cwd,
        terminal_columns: None,
    };
    let (code, out, err) = py.detach(|| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = larmorx_tedana::cli::run(&args, &settings, &mut out, &mut err);
        (code, out, err)
    });
    Ok((
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    ))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(tedana_t2smap_cleanroom, m)?)?;
    Ok(())
}
