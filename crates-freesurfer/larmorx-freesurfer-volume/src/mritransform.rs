// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The two matrix helpers of `utils/mriTransform.cpp` a program reaches (`mri_cc -a 0`, item
//! 3.2-27; track D's relay 54): `Trns_ExtractRotationMatrix` (638) and `Trns_ExtractScaleMatrix`
//! (676). The rest of the file (the `mriTransform` object of tkmedit's era) is reached by no
//! program.

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{
    MATRIX_REAL, Matrix, matrix_alloc, matrix_copy, matrix_multiply,
};
use larmorx_freesurfer_core::matrix_ops::{fzero, matrix_identity, vector_len};

/// `Trns_tErr_NoErr`.
pub const TRNS_NO_ERR: i32 = 0;
/// `Trns_tErr_InvalidParameter`.
pub const TRNS_INVALID_PARAMETER: i32 = 3;

/// `MatrixCopy(m, NULL)` with the translation column's first three rows set to 0.
fn without_translation(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
) -> Result<Matrix, Fatal> {
    let mut t = matrix_copy(g, out, err, Some(m), None)?.unwrap_new();
    for row in 1..=3 {
        t.set_relt(row, 4, 0.0);
    }
    Ok(t)
}

/// `Trns_ExtractScaleMatrix(iTransform, oScale)`: `oScale` the identity with, on the diagonal's
/// first three places, the `float` lengths (`VectorLen`) of the transform's first three columns
/// without its translation (each the product of that matrix with the unit vector, `MatrixMultiply`
/// into the vector itself, the vector's fourth element 0). `Trns_tErr_NoErr`.
pub fn trns_extract_scale_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    transform: &Matrix,
    scale: &mut Matrix,
) -> Result<i32, Fatal> {
    let tmp = without_translation(g, out, err, transform)?;
    let mut v = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a real vector");
    let mut factors = [0f32; 3];
    for (axis, factor) in factors.iter_mut().enumerate() {
        for row in 1..=4 {
            v.set_relt(row, 1, if row == axis as i32 + 1 { 1.0 } else { 0.0 });
        }
        // `MatrixMultiply(mTmp, vTmp, vTmp)`: FreeSurfer copies an input that is the output
        let input = v.clone();
        matrix_multiply(g, out, err, Some(&tmp), Some(&input), Some(&mut v))?;
        *factor = vector_len(g, err, &v);
        if axis == 0 {
            matrix_identity(g, out, err, 4, Some(&mut *scale))?;
        }
        scale.set_relt(axis as i32 + 1, axis as i32 + 1, *factor);
    }
    Ok(TRNS_NO_ERR)
}

/// `Trns_ExtractRotationMatrix(iTransform, oRotation)`: the transform without its translation
/// times the inverse of its scale ([`trns_extract_scale_matrix`]: each factor `f` of the
/// diagonal replaced by `1.0 / f` in `double`, stored as `float`, or by 1 where `FZERO(f)`), into
/// `oRotation` (`MatrixMultiply`). `Trns_tErr_NoErr`; `Trns_tErr_InvalidParameter` without either
/// matrix (nothing done).
pub fn trns_extract_rotation_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    transform: Option<&Matrix>,
    rotation: Option<&mut Matrix>,
) -> Result<i32, Fatal> {
    let (Some(transform), Some(rotation)) = (transform, rotation) else {
        return Ok(TRNS_INVALID_PARAMETER);
    };
    let tmp = without_translation(g, out, err, transform)?;
    let mut scale = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?.expect("a real matrix");
    trns_extract_scale_matrix(g, out, err, &tmp, &mut scale)?;
    for n in 1..=3 {
        let f = scale.relt(n, n);
        let inv = if !fzero(f64::from(f)) {
            (1.0 / f64::from(f)) as f32
        } else {
            1.0
        };
        scale.set_relt(n, n, inv);
    }
    matrix_multiply(g, out, err, Some(&tmp), Some(&scale), Some(rotation))?;
    Ok(TRNS_NO_ERR)
}
