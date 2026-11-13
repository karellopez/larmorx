// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's 4 x 4 affine helpers (`fs.maths`, queue item 3.1-23), translated from
//! FreeSurfer 8.2.0's `include/affine.h`: `AffineVector`, `AffineMatrix`, `SetAffineVector`,
//! `GetAffineVector`, `GetFloorAffineVector`, `SetAffineMatrix`, `GetAffineMatrix`,
//! `AffineMatrixFree`, `AffineMatrixAlloc`, `AffineMatrixCopy`, `AffineMV`, `AffineMM`.
//!
//! The official build compiles the SSE branch (`AFFINE_MATRIX_USE_SSE`: GCC above 3, no
//! `mcheck`, not ARM64): the matrix is stored column by column, and `AffineMV` computes each
//! element as `((x0 A0 + x1 A1) + x2 A2) + x3 A3` in `float`, starting from the first product
//! (the plain C branch starts from 0, so where all four products are -0 it gives +0; the SSE
//! branch, the one built, gives -0). `AffineMM` is `AffineMV` column by column. There is no fused
//! multiply-add (x86-64's baseline instructions have none). NaNs follow x86-64's rules
//! (`crate::sse`) in the official build's operand order, measured where FreeSurfer inlines
//! `AffineMV` (`MRIcalcCRASforExtractedVolume`'s object code: each product with the vector's
//! number in the destination register, each sum with the running sum there; the fourth product,
//! by 1, folded away, which gives the same bits); `AffineMM` takes the same order.
//!
//! `SetAffineMatrix` and `GetAffineMatrix` stop the program (`abort`, exit status 134) when the
//! `MATRIX` is not 4 x 4, after `<function>: Bad matrix size` on standard error: FreeSurfer's own
//! check, kept in both modes. `AffineMatrixAlloc`'s memory is not cleared in FreeSurfer (its
//! callers set it); the translation's is zero. `GetFloorAffineVector`'s `(int) floorf(v)` is
//! FS-17's conversion.

use std::io::Write;

use larmorx_core::defects::FS_17;

use crate::globals::Globals;
use crate::log::Fatal;
use crate::matrix::Matrix;
use crate::sse;

/// `kAffineVectorSize`, `kAffineMatrixSize`.
pub const AFFINE_VECTOR_SIZE: usize = 4;
pub const AFFINE_MATRIX_SIZE: usize = 16;

/// `AffineVector`: `x`, `y`, `z`, 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AffineVector {
    pub vec: [f32; 4],
}

/// `AffineMatrix`: 16 floats, column by column (`mat[i + 4 j]` is row `i`, column `j`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AffineMatrix {
    pub mat: [f32; 16],
}

/// `SetAffineVector(av, x, y, z)`.
pub fn set_affine_vector(av: &mut AffineVector, x: f32, y: f32, z: f32) {
    av.vec = [x, y, z, 1.0];
}

/// `GetAffineVector(av, &x, &y, &z)`.
pub fn get_affine_vector(av: &AffineVector) -> (f32, f32, f32) {
    (av.vec[0], av.vec[1], av.vec[2])
}

/// `GetFloorAffineVector(av, &x, &y, &z)`: `(int) floorf(v)` of each (FS-17 beyond `int`).
pub fn get_floor_affine_vector(g: &mut Globals, av: &AffineVector) -> (i32, i32, i32) {
    let f = |g: &mut Globals, v: f32| float_to_int(g, f64::from(v.floor()), "GetFloorAffineVector");
    (f(g, av.vec[0]), f(g, av.vec[1]), f(g, av.vec[2]))
}

/// `(int) x` for a value from a `float` or `double` (x86-64's `cvttss2si`/`cvttsd2si`): the
/// value truncated toward zero; beyond the `int` range or NaN, C leaves it undefined and x86-64
/// gives `INT_MIN`: FS-17 (by default saturated, NaN 0; `reproduce` `INT_MIN`).
pub fn float_to_int(g: &mut Globals, x: f64, what: &str) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
        g.notices.add(
            &FS_17,
            format!("{what}: (int) {x}, outside the int range: an undefined conversion"),
        );
        if g.notices.reproduce() {
            return i32::MIN;
        }
        if x.is_nan() {
            return 0;
        }
        return if x > 0.0 { i32::MAX } else { i32::MIN };
    }
    x.trunc() as i32
}

/// The check of `SetAffineMatrix` and `GetAffineMatrix`.
fn check_size(err: &mut dyn Write, m: &Matrix, function: &str) -> Result<(), Fatal> {
    if m.rows != AFFINE_VECTOR_SIZE as i32 || m.cols != AFFINE_VECTOR_SIZE as i32 {
        let _ = writeln!(err, "{function}: Bad matrix size");
        return Err(Fatal::Exit(134));
    }
    Ok(())
}

/// `SetAffineMatrix(am, src)`: `am->mat[i + 4 j] = src->data[j + 4 i]`.
pub fn set_affine_matrix(
    err: &mut dyn Write,
    am: &mut AffineMatrix,
    src: &Matrix,
) -> Result<(), Fatal> {
    check_size(err, src, "SetAffineMatrix")?;
    let data = src.data();
    for i in 0..4 {
        for j in 0..4 {
            am.mat[i + 4 * j] = data[j + 4 * i];
        }
    }
    Ok(())
}

/// `GetAffineMatrix(dst, am)`: `dst->data[j + 4 i] = am->mat[i + 4 j]`.
pub fn get_affine_matrix(
    err: &mut dyn Write,
    dst: &mut Matrix,
    am: &AffineMatrix,
) -> Result<(), Fatal> {
    check_size(err, dst, "GetAffineMatrix")?;
    let data = dst.data_mut();
    for i in 0..4 {
        for j in 0..4 {
            data[j + 4 * i] = am.mat[i + 4 * j];
        }
    }
    Ok(())
}

/// `AffineMatrixFree(&am)`.
pub fn affine_matrix_free(am: &mut Option<AffineMatrix>) {
    *am = None;
}

/// `AffineMatrixAlloc(&am)`: a new matrix in place of the old (zeros here; FreeSurfer's memory
/// is not cleared).
pub fn affine_matrix_alloc(am: &mut Option<AffineMatrix>) {
    *am = Some(AffineMatrix::default());
}

/// `AffineMatrixCopy(src, dst)`: `dst` (allocated when absent) with `src`'s 16 floats.
pub fn affine_matrix_copy(src: &AffineMatrix, dst: Option<AffineMatrix>) -> AffineMatrix {
    let mut d = dst.unwrap_or_default();
    d.mat = src.mat;
    d
}

/// One column of the SSE code: `((v0 c0 + v1 c1) + v2 c2) + v3 c3`, lane by lane, with SSE's
/// NaNs in the official operand order (the vector's number, then the running sum, first).
fn combine(a: &[f32; 16], v: [f32; 4]) -> [f32; 4] {
    let mut res = [0.0f32; 4];
    for (i, r) in res.iter_mut().enumerate() {
        let mut acc = sse::mul(v[0], a[i]);
        acc = sse::add(acc, sse::mul(v[1], a[4 + i]));
        acc = sse::add(acc, sse::mul(v[2], a[8 + i]));
        acc = sse::add(acc, sse::mul(v[3], a[12 + i]));
        *r = acc;
    }
    res
}

/// `AffineMV(y, A, x)`: `y = A x` (the SSE branch).
pub fn affine_mv(a: &AffineMatrix, x: &AffineVector) -> AffineVector {
    AffineVector {
        vec: combine(&a.mat, x.vec),
    }
}

/// `AffineMM(C, A, B)`: `C = A B`, column `j` of `C` being `A` times column `j` of `B` (the
/// SSE branch).
pub fn affine_mm(a: &AffineMatrix, b: &AffineMatrix) -> AffineMatrix {
    let mut c = AffineMatrix::default();
    for j in 0..4 {
        let col = [
            b.mat[4 * j],
            b.mat[4 * j + 1],
            b.mat[4 * j + 2],
            b.mat[4 * j + 3],
        ];
        c.mat[4 * j..4 * j + 4].copy_from_slice(&combine(&a.mat, col));
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn set_get_and_products() {
        let src = Matrix::from_rows(&[
            vec![1.0, 2.0, 3.0, 4.0],
            vec![5.0, 6.0, 7.0, 8.0],
            vec![9.0, 10.0, 11.0, 12.0],
            vec![0.0, 0.0, 0.0, 1.0],
        ]);
        let mut am = AffineMatrix::default();
        let mut e = Vec::new();
        set_affine_matrix(&mut e, &mut am, &src).unwrap();
        assert_eq!(am.mat[4], 2.0);
        let mut back = Matrix::real(4, 4);
        get_affine_matrix(&mut e, &mut back, &am).unwrap();
        assert_eq!(back, src);
        let mut v = AffineVector::default();
        set_affine_vector(&mut v, 1.0, 1.0, 1.0);
        assert_eq!(affine_mv(&am, &v).vec, [10.0, 26.0, 42.0, 1.0]);
        let id = AffineMatrix {
            mat: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        };
        assert_eq!(affine_mm(&am, &id), am);
        let bad = Matrix::real(3, 4);
        assert_eq!(
            set_affine_matrix(&mut e, &mut am, &bad),
            Err(Fatal::Exit(134))
        );
        assert_eq!(e, b"SetAffineMatrix: Bad matrix size\n");
    }

    #[test]
    fn minus_zero_kept_and_floor_conversion() {
        let mut am = AffineMatrix::default();
        for j in 0..4 {
            am.mat[4 * j] = -1.0;
        }
        let v = AffineVector::default();
        assert!(affine_mv(&am, &v).vec[0].is_sign_negative());
        let mut v = AffineVector::default();
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        set_affine_vector(&mut v, 3e9, -1.5, f32::NAN);
        assert_eq!(
            get_floor_affine_vector(&mut g, &v),
            (i32::MIN, -2, i32::MIN)
        );
        let mut g = Globals::new(UpstreamDefects::Fix);
        assert_eq!(get_floor_affine_vector(&mut g, &v), (i32::MAX, -2, 0));
        assert!(g.notices.contains(&FS_17));
    }
}
