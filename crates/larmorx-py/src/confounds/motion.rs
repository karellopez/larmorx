// SPDX-License-Identifier: Apache-2.0
//! Bindings of fMRIPrep's motion parameters and root mean square deviation, as
//! `larmorx_confounds::motion` computes them: nitransforms' ITK-to-RAS and RAS-to-FSL
//! conversions (`nitransforms_itk_to_ras`, `nitransforms_fsl_from_ras`, `numpy_first_singular4`,
//! `numpy_det4_is_positive`),
//! SciPy's `Rotation.from_matrix` and `as_euler` (`scipy_rotation_from_matrix`,
//! `scipy_rotation_as_euler`) and fMRIPrep's two interfaces' arithmetic
//! (`fmriprep_motion_params`, `fmriprep_rms_deviation`), which `larmorx.confounds.motion` calls.
//! The validation (`validation/investigations/confounds/check_motion.py`) compares them with
//! each environment's nitransforms, SciPy, numpy and fMRIPrep. Matrices cross as flat C-ordered
//! rows (`n × 16` for 4 × 4, `n × 9` for 3 × 3). The GIL is released while computing.

use super::dvars::numpy_settings;
use super::svd::{Mat, linalg_error, mat};
use larmorx_confounds::motion::{
    self, EulerSeq, FslSpace, MotionError, MotionRelease, SliceOrder,
};
use larmorx_core::linalg::{Mat3, Mat4};
use numpy::{PyArray1, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

fn rows4(a: &PyReadonlyArray2<'_, f64>) -> PyResult<Vec<Mat4>> {
    let v = a.as_array();
    if v.ncols() != 16 {
        return Err(PyValueError::new_err("expected n x 16 rows (4 x 4 matrices, C order)"));
    }
    Ok(v.rows()
        .into_iter()
        .map(|r| std::array::from_fn(|i| std::array::from_fn(|j| r[4 * i + j])))
        .collect())
}

fn rows3(a: &PyReadonlyArray2<'_, f64>) -> PyResult<Vec<Mat3>> {
    let v = a.as_array();
    if v.ncols() != 9 {
        return Err(PyValueError::new_err("expected n x 9 rows (3 x 3 matrices, C order)"));
    }
    Ok(v.rows()
        .into_iter()
        .map(|r| std::array::from_fn(|i| std::array::from_fn(|j| r[3 * i + j])))
        .collect())
}

fn flat4<'py>(py: Python<'py>, m: &[Mat4]) -> PyResult<Mat<'py>> {
    mat(py, m.len(), 16, m.iter().flatten().flatten().copied().collect())
}

fn err(py: Python<'_>, e: MotionError) -> PyErr {
    match e {
        MotionError::Singular | MotionError::Svd => linalg_error(py, &e.to_string()),
        MotionError::NonPositiveDeterminant(_) => PyValueError::new_err(e.to_string()),
    }
}

fn lapack(numpy: &str) -> PyResult<larmorx_transform::numpy_linalg::LapackRelease> {
    Ok(numpy_settings(numpy)?.2.release)
}

/// nitransforms' `ITKLinearTransform.to_ras()` of each row of ITK's 12 parameters (`n × 12`)
/// with its fixed parameters (`n × 3`): the `n × 16` RAS matrices.
#[pyfunction]
fn nitransforms_itk_to_ras<'py>(
    py: Python<'py>,
    params: PyReadonlyArray2<'py, f64>,
    offsets: PyReadonlyArray2<'py, f64>,
) -> PyResult<Mat<'py>> {
    let (p, o) = (params.as_array(), offsets.as_array());
    if p.ncols() != 12 || o.ncols() != 3 || p.nrows() != o.nrows() {
        return Err(PyValueError::new_err(
            "expected n x 12 parameters and n x 3 fixed parameters",
        ));
    }
    let out: Vec<Mat4> = p
        .rows()
        .into_iter()
        .zip(o.rows())
        .map(|(pr, or)| {
            let pp: [f64; 12] = std::array::from_fn(|i| pr[i]);
            let oo: [f64; 3] = std::array::from_fn(|i| or[i]);
            motion::itk_to_ras(&pp, &oo)
        })
        .collect();
    flat4(py, &out)
}

/// The index of the first `n × 16` matrix numpy's `inv` finds singular, or `None`.
#[pyfunction]
fn numpy_first_singular4(mats: PyReadonlyArray2<'_, f64>) -> PyResult<Option<usize>> {
    Ok(motion::first_singular(&rows4(&mats)?))
}

/// numpy's `det(affine) > 0` of a `1 × 16` matrix (OpenBLAS's LU factors).
#[pyfunction]
fn numpy_det4_is_positive(affine: PyReadonlyArray2<'_, f64>) -> PyResult<bool> {
    let a = rows4(&affine)?;
    if a.len() != 1 {
        return Err(PyValueError::new_err("expected one matrix (1 x 16)"));
    }
    Ok(motion::det4(&a[0]) > 0.0)
}

/// nitransforms' `FSLLinearTransformArray.from_ras(ras, reference=img, moving=img)` for an image
/// of affine `affine` (`1 × 16`) and `shape0` voxels along its first axis: the `n × 16` FSL
/// parameters; numpy's `LinAlgError("Singular matrix")` where its `inv` raises.
#[pyfunction]
fn nitransforms_fsl_from_ras<'py>(
    py: Python<'py>,
    ras: PyReadonlyArray2<'py, f64>,
    affine: PyReadonlyArray2<'py, f64>,
    shape0: i64,
) -> PyResult<Mat<'py>> {
    let ras = rows4(&ras)?;
    let aff = rows4(&affine)?;
    if aff.len() != 1 {
        return Err(PyValueError::new_err("expected one affine (1 x 16)"));
    }
    let out = py
        .detach(|| {
            let space = FslSpace::new(&aff[0], shape0)?;
            ras.iter()
                .map(|r| motion::fsl_from_ras(r, &space))
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|e| err(py, e))?;
    flat4(py, &out)
}

fn slice_order(order: &str) -> PyResult<SliceOrder> {
    match order {
        "C" => Ok(SliceOrder::C),
        "F" => Ok(SliceOrder::F),
        _ => Err(PyValueError::new_err(format!(
            "order must be 'C' or 'F', not {order:?}"
        ))),
    }
}

/// SciPy's `Rotation.from_matrix(mats)` of `n × 9` matrices (the float64 copy SciPy makes in
/// `order`), with numpy `numpy`'s LAPACK: `(quats, None)` (`n × 4`, `(x, y, z, w)`), or `(None,
/// i)` for the first matrix whose determinant is at most 0 (SciPy's `ValueError`); numpy's
/// `LinAlgError("SVD did not converge")` for a matrix that is not finite.
#[pyfunction]
#[pyo3(signature = (mats, numpy = "2.5.4", order = "C"))]
fn scipy_rotation_from_matrix<'py>(
    py: Python<'py>,
    mats: PyReadonlyArray2<'py, f64>,
    numpy: &str,
    order: &str,
) -> PyResult<(Option<Mat<'py>>, Option<usize>)> {
    let m = rows3(&mats)?;
    let (order, lapack) = (slice_order(order)?, lapack(numpy)?);
    match py.detach(|| motion::scipy_from_matrix(&m, order, lapack)) {
        Ok(q) => Ok((
            Some(mat(py, q.len(), 4, q.into_iter().flatten().collect())?),
            None,
        )),
        Err(MotionError::NonPositiveDeterminant(i)) => Ok((None, Some(i))),
        Err(e) => Err(err(py, e)),
    }
}

fn euler_seq(seq: &str) -> PyResult<EulerSeq> {
    let b = seq.as_bytes();
    let bad = || PyValueError::new_err(format!("not an Euler sequence SciPy takes: {seq:?}"));
    if b.len() != 3 {
        return Err(bad());
    }
    let extrinsic = b.iter().all(|c| b"xyz".contains(c));
    let intrinsic = b.iter().all(|c| b"XYZ".contains(c));
    if !(extrinsic || intrinsic) || b[0].eq_ignore_ascii_case(&b[1]) || b[1].eq_ignore_ascii_case(&b[2])
    {
        return Err(bad());
    }
    let axes = std::array::from_fn(|i| (b[i].to_ascii_lowercase() - b'x') as usize);
    Ok(EulerSeq { axes, extrinsic })
}

/// SciPy's `as_euler(seq)` of `n × 4` quaternions `(x, y, z, w)` (correctly rounded `atan2` and
/// `hypot`): `(angles, gimbal)`, the `n × 3` angles in radians and the number of rotations in
/// gimbal lock.
#[pyfunction]
fn scipy_rotation_as_euler<'py>(
    py: Python<'py>,
    quats: PyReadonlyArray2<'py, f64>,
    seq: &str,
) -> PyResult<(Mat<'py>, usize)> {
    let s = euler_seq(seq)?;
    let v = quats.as_array();
    if v.ncols() != 4 {
        return Err(PyValueError::new_err("expected n x 4 quaternions"));
    }
    let q: Vec<[f64; 4]> = v
        .rows()
        .into_iter()
        .map(|r| std::array::from_fn(|i| r[i]))
        .collect();
    let (a, g) = py.detach(|| motion::scipy_as_euler(&q, s));
    Ok((mat(py, a.len(), 3, a.into_iter().flatten().collect())?, g))
}

fn release(fmriprep: &str) -> PyResult<MotionRelease> {
    match fmriprep {
        "25.2.6" => Ok(MotionRelease::V25_2_6),
        "25.2.7" => Ok(MotionRelease::V25_2_7),
        _ => Err(PyValueError::new_err(format!(
            "fmriprep must be '25.2.6' or '25.2.7', not {fmriprep:?}"
        ))),
    }
}

/// `FSLMotionParams`' arithmetic on the `n × 16` FSL matrices with the image's float32 zooms and
/// centre of mass: `(params, gimbal, None)` (`n × 6`) or `(None, 0, i)` for SciPy's
/// non-positive determinant of matrix `i`; numpy's `LinAlgError` for SciPy's SVD.
#[pyfunction]
#[pyo3(signature = (fsl, zooms, com, fmriprep = "25.2.7", numpy = "2.5.4"))]
fn fmriprep_motion_params<'py>(
    py: Python<'py>,
    fsl: PyReadonlyArray2<'py, f64>,
    zooms: [f64; 3],
    com: [f64; 3],
    fmriprep: &str,
    numpy: &str,
) -> PyResult<(Option<Mat<'py>>, usize, Option<usize>)> {
    let f = rows4(&fsl)?;
    let (rel, lapack) = (release(fmriprep)?, lapack(numpy)?);
    let out = py.detach(|| {
        let cog = motion::center_of_gravity(&zooms, &com);
        motion::motion_params(&f, &cog, rel, lapack)
    });
    match out {
        Ok(m) => Ok((
            Some(mat(py, m.params.len(), 6, m.params.into_iter().flatten().collect())?),
            m.gimbal,
            None,
        )),
        Err(MotionError::NonPositiveDeterminant(i)) => Ok((None, 0, Some(i))),
        Err(e) => Err(err(py, e)),
    }
}

/// `FSLRMSDeviation`'s values (`n`, the first NaN) for the `n × 16` FSL matrices and the centre
/// `0.5 · (shape - 1) · zooms`; numpy's `LinAlgError("Singular matrix")` where its `inv` raises.
#[pyfunction]
fn fmriprep_rms_deviation<'py>(
    py: Python<'py>,
    fsl: PyReadonlyArray2<'py, f64>,
    center: [f64; 3],
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let f = rows4(&fsl)?;
    let out = py
        .detach(|| motion::rms_deviation(&f, &center))
        .map_err(|e| err(py, e))?;
    Ok(PyArray1::from_vec(py, out))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(nitransforms_itk_to_ras, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_first_singular4, m)?)?;
    m.add_function(wrap_pyfunction!(numpy_det4_is_positive, m)?)?;
    m.add_function(wrap_pyfunction!(nitransforms_fsl_from_ras, m)?)?;
    m.add_function(wrap_pyfunction!(scipy_rotation_from_matrix, m)?)?;
    m.add_function(wrap_pyfunction!(scipy_rotation_as_euler, m)?)?;
    m.add_function(wrap_pyfunction!(fmriprep_motion_params, m)?)?;
    m.add_function(wrap_pyfunction!(fmriprep_rms_deviation, m)?)?;
    Ok(())
}
