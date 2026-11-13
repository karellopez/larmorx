// SPDX-License-Identifier: Apache-2.0
//! Binding of niworkflows' `SignalExtraction` means, as `larmorx_confounds::signals` computes
//! them: `niworkflows_signal_means`, which `larmorx.niworkflows.images.signal_extraction` calls
//! once per region. The validation (`validation/investigations/confounds/check_masks.py`)
//! compares it with each environment's niworkflows and numpy. The GIL is released while
//! computing.

use super::dvars::numpy_settings;
use larmorx_confounds::signals::{self, Series};
use numpy::{PyArray1, PyReadonlyArray3, PyReadonlyArray4, PyUntypedArrayMethods};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

/// The element strides of a contiguous (C- or Fortran-ordered) array, else `ValueError`.
fn strides4(strides: &[isize], contiguous: bool) -> PyResult<[usize; 4]> {
    if !contiguous || strides.iter().any(|&s| s < 0) {
        return Err(PyValueError::new_err(
            "the series must be a C- or Fortran-contiguous array",
        ));
    }
    Ok(std::array::from_fn(|i| strides[i] as usize))
}

/// `data[mask, :].mean(axis=0)` of a 4-D `float32` or `float64` series `(X, Y, Z, T)` (C- or
/// Fortran-ordered; `float32` values are widened exactly, as nibabel's `get_fdata()`) over the
/// voxels of the 3-D boolean `mask` `(X, Y, Z)` in its C order, summed as numpy release
/// `numpy_version` sums them: the `T` means (`float64`; NaN for an empty mask).
#[pyfunction]
#[pyo3(signature = (data, mask, numpy_version = "2.5.4", n_threads = 1))]
fn niworkflows_signal_means<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    mask: PyReadonlyArray3<'py, bool>,
    numpy_version: &str,
    n_threads: usize,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let order = numpy_settings(numpy_version)?.1;
    let m = mask.as_array();
    let (mx, my, mz) = m.dim();
    let flat: Vec<bool> = m.iter().copied().collect();
    macro_rules! run {
        ($arr:expr, $variant:path) => {{
            let a = $arr;
            let contiguous = a.is_c_contiguous() || a.is_fortran_contiguous();
            let shape = a.shape().to_vec();
            if shape[..3] != [mx, my, mz] {
                return Err(PyValueError::new_err(format!(
                    "the mask's shape {:?} is not the series' first three axes {:?}",
                    [mx, my, mz],
                    &shape[..3]
                )));
            }
            let view = a.as_array();
            let st = strides4(view.strides(), contiguous)?;
            let slice = view
                .as_slice_memory_order()
                .ok_or_else(|| PyValueError::new_err("the series must be contiguous"))?;
            let offsets = signals::voxel_offsets(&flat, [mx, my, mz], [st[0], st[1], st[2]]);
            let nt = shape[3];
            py.detach(|| {
                signals::signal_means($variant(slice), &offsets, st[3], nt, order, n_threads)
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?
        }};
    }
    let out = if let Ok(a) = data.extract::<PyReadonlyArray4<'py, f32>>() {
        run!(a, Series::F32)
    } else {
        let a: PyReadonlyArray4<'py, f64> = data.extract()?;
        run!(a, Series::F64)
    };
    Ok(PyArray1::from_vec(py, out))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(niworkflows_signal_means, m)?)?;
    Ok(())
}
