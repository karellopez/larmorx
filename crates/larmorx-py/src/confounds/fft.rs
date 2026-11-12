// SPDX-License-Identifier: Apache-2.0
//! Bindings of SciPy's single-precision FFTs as `larmorx_confounds::scipy_fft` computes them:
//! `scipy.fftpack.fft(x, n)` and `ifft(x, n)` of a one-dimensional float32 or complex64 array of
//! power-of-two length (after `n`), with SciPy 1.15's pocketfft or SciPy 1.18.1's ducc0
//! (`scipy_fftpack`). The validation (`validation/investigations/confounds/check_fft.py`)
//! compares the bits with each environment's own `scipy.fftpack`.

use larmorx_confounds::scipy_fft::{self, C32, FftError, FftLibrary};
use numpy::{PyArray1, PyReadonlyArray1};
use pyo3::exceptions::{PyNotImplementedError, PyTypeError, PyValueError};
use pyo3::prelude::*;

fn library(v: &str) -> PyResult<FftLibrary> {
    FftLibrary::from_scipy_version(v).ok_or_else(|| {
        PyValueError::new_err(format!(
            "scipy must be a 1.15 or 1.18 release (pocketfft, ducc0), got {v:?}"
        ))
    })
}

fn fft_err(e: FftError) -> PyErr {
    match e {
        FftError::InvalidLength(_) => PyValueError::new_err(e.to_string()),
        FftError::NotPowerOfTwo(_) | FftError::TooLong { .. } => {
            PyNotImplementedError::new_err(e.to_string())
        }
    }
}

/// `scipy.fftpack.fft(x, n)` (`forward`) or `ifft(x, n)` of the one-dimensional float32 or
/// complex64 array `x` (any strides) with SciPy `scipy`'s library (`"1.18.1"`: ducc0;
/// `"1.15.3"`, `"1.15.2"`: pocketfft): a complex64 array of length `n` (default `len(x)`).
/// `ValueError` for a length below one (SciPy's message), `NotImplementedError` for a length
/// that is not a power of two, or of 131072 or more for a complex array with ducc0.
#[pyfunction]
#[pyo3(signature = (x, n = None, forward = true, scipy = "1.18.1"))]
fn scipy_fftpack<'py>(
    py: Python<'py>,
    x: &Bound<'py, PyAny>,
    n: Option<i64>,
    forward: bool,
    scipy: &str,
) -> PyResult<Bound<'py, PyArray1<C32>>> {
    let lib = library(scipy)?;
    let n = match n {
        Some(v) if v < 1 => return Err(fft_err(FftError::InvalidLength(v))),
        Some(v) => Some(usize::try_from(v).map_err(|e| PyValueError::new_err(e.to_string()))?),
        None => None,
    };
    let out = if let Ok(a) = x.extract::<PyReadonlyArray1<'py, f32>>() {
        let v: Vec<f32> = a.as_array().iter().copied().collect();
        py.detach(|| scipy_fft::fftpack_real(lib, &v, n, forward))
    } else if let Ok(a) = x.extract::<PyReadonlyArray1<'py, C32>>() {
        let v: Vec<C32> = a.as_array().iter().copied().collect();
        py.detach(|| scipy_fft::fftpack_complex(lib, &v, n, forward))
    } else {
        return Err(PyTypeError::new_err(
            "x must be a one-dimensional float32 or complex64 array",
        ));
    }
    .map_err(fft_err)?;
    Ok(PyArray1::from_vec(py, out))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(scipy_fftpack, m)?)?;
    Ok(())
}
