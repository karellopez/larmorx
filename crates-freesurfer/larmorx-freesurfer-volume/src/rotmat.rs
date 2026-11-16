// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Rotations and affine parameters (`utils/transform.cpp`, 3.1-40): `MRIangles2RotMat` (4591),
//! `TranformAffineParams2Matrix` (5228, FreeSurfer's spelling) and `TranformExtractAffineParams`
//! (5298), `LTAmat2RotMat` (4956) and `RMSregDiffMJ` (4989, Jenkinson's RMS difference of two
//! registrations within a sphere).
//!
//! The C library's `sin`, `cos` and `pow` are glibc's with `reproduce`, correctly rounded by
//! default (GLIBC-07 where they differ); `atan2` is correctly rounded in both modes (the
//! project's decision for glibc's IBM routines, `docs/findings/freesurfer-libm.md` §2). A NaN
//! argument comes back quieted, a NaN the function makes is SSE's default NaN.

use std::f64::consts::PI;
use std::io::Write;

use larmorx_core::math::{self, Libm};
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply, matrix_multiply_d};
use larmorx_freesurfer_core::matrix_ops::{
    matrix_determinant, matrix_identity, matrix_qr_decomposition, matrix_subtract, matrix_trace,
    matrix_transpose,
};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_mri::lta::Lta;

use crate::ltaops::{InvertInto, lta_invert, outside};

/// A C library function's value as the official build gets it: a NaN argument quieted, a NaN
/// made from a number SSE's default NaN.
fn nan_of(r: f64, a: f64) -> f64 {
    if !r.is_nan() {
        r
    } else if a.is_nan() {
        f64::from_bits(a.to_bits() | 0x0008_0000_0000_0000)
    } else {
        sse::DEFAULT_NAN64
    }
}

/// `atan2(y, x)`: correctly rounded; the NaN of `x + y` when either is a NaN (glibc's rule).
fn atan2(y: f64, x: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return sse::add64(x, y);
    }
    math::atan2(y, x)
}

/// A product into its own left operand (`MatrixMultiply(a, b, a)`: FreeSurfer multiplies a copy).
fn mul_into_left(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    a: &mut Matrix,
    b: &Matrix,
    double: bool,
) -> Result<bool, Fatal> {
    let a0 = a.clone();
    let r = if double {
        matrix_multiply_d(g, out, err, Some(&a0), Some(b), Some(a))?
    } else {
        matrix_multiply(g, out, err, Some(&a0), Some(b), Some(a))?
    };
    Ok(!r.is_null())
}

/// `MRIangles2RotMat(angles)`: the 4 x 4 rotation `Rz(alpha) Ry(beta) Rx(gamma)` (`angles` =
/// gamma, beta, alpha in radians; each matrix's cosines and sines in `double`, stored as
/// `float`; `float` products, `Rz Ry` first), 1 at (4, 4).
pub fn mri_angles2rot_mat(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    angles: &[f64; 3],
) -> Result<Matrix, Fatal> {
    let (gamma, beta, alpha) = (angles[0], angles[1], angles[2]);
    let libm = Libm::new(g.mode());
    let cs = |x: f64| (nan_of(libm.cos(x), x), nan_of(libm.sin(x), x));
    let (cg, sg) = cs(gamma);
    let (cb, sb) = cs(beta);
    let (ca, sa) = cs(alpha);
    let mut rx = Matrix::real(3, 3);
    rx.set_relt(1, 1, 1.0);
    rx.set_relt(2, 2, cg as f32);
    rx.set_relt(2, 3, -sg as f32);
    rx.set_relt(3, 2, sg as f32);
    rx.set_relt(3, 3, cg as f32);
    let mut ry = Matrix::real(3, 3);
    ry.set_relt(1, 1, cb as f32);
    ry.set_relt(1, 3, sb as f32);
    ry.set_relt(2, 2, 1.0);
    ry.set_relt(3, 1, -sb as f32);
    ry.set_relt(3, 3, cb as f32);
    let mut rz = Matrix::real(3, 3);
    rz.set_relt(1, 1, ca as f32);
    rz.set_relt(1, 2, -sa as f32);
    rz.set_relt(2, 1, sa as f32);
    rz.set_relt(2, 2, ca as f32);
    rz.set_relt(3, 3, 1.0);
    note_glibc07(g, &libm, "sin and cos");
    let mut r3 = matrix_multiply(g, out, err, Some(&rz), Some(&ry), None)?.unwrap_new();
    mul_into_left(g, out, err, &mut r3, &rx, false)?;
    let mut r = Matrix::real(4, 4);
    for c in 1..=3 {
        for row in 1..=3 {
            r.set_relt(row, c, r3.relt(row, c));
        }
    }
    r.set_relt(4, 4, 1.0);
    Ok(r)
}

/// A new 4 x 4 identity.
fn identity(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write) -> Result<Matrix, Fatal> {
    Ok(matrix_identity(g, out, err, 4, None)?.unwrap_new())
}

/// `TranformAffineParams2Matrix(p, M)`: `T R1 R2 R3 Z S` from twelve parameters (`p[0..3]` the
/// translation, `p[3..6]` the rotations about x, y and z in degrees, `p[6..9]` the scales,
/// `p[9..12]` the shears xy, xz, yz): each rotation's cosine and sine of `p * pi / 180` in
/// `double`, stored as `float`; the products `MatrixMultiplyD` (`double` sums, `float` results),
/// `R1 R2` then `R3`, then `T R`, `Z`, `S`. Into `m` when given (a given matrix of another size:
/// the products' messages and the program's end, as FreeSurfer's), else a new matrix.
pub fn tranform_affine_params2_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: &[f64; 12],
    m: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let mut t = identity(g, out, err)?;
    t.set_relt(1, 4, p[0] as f32);
    t.set_relt(2, 4, p[1] as f32);
    t.set_relt(3, 4, p[2] as f32);
    let libm = Libm::new(g.mode());
    let cs = |x: f64| {
        let a = sse::div64(sse::mul64(x, PI), 180.0);
        (nan_of(libm.cos(a), a), nan_of(libm.sin(a), a))
    };
    let mut r1 = identity(g, out, err)?;
    let (c, s) = cs(p[3]);
    r1.set_relt(2, 2, c as f32);
    r1.set_relt(2, 3, s as f32);
    r1.set_relt(3, 2, -s as f32);
    r1.set_relt(3, 3, c as f32);
    let mut r2 = identity(g, out, err)?;
    let (c, s) = cs(p[4]);
    r2.set_relt(1, 1, c as f32);
    r2.set_relt(1, 3, s as f32);
    r2.set_relt(3, 1, -s as f32);
    r2.set_relt(3, 3, c as f32);
    let mut r3 = identity(g, out, err)?;
    let (c, s) = cs(p[5]);
    r3.set_relt(1, 1, c as f32);
    r3.set_relt(1, 2, s as f32);
    r3.set_relt(2, 1, -s as f32);
    r3.set_relt(2, 2, c as f32);
    note_glibc07(g, &libm, "sin and cos");
    let mut r = matrix_multiply_d(g, out, err, Some(&r1), Some(&r2), None)?.unwrap_new();
    mul_into_left(g, out, err, &mut r, &r3, true)?;
    let mut zz = identity(g, out, err)?;
    zz.set_relt(1, 1, p[6] as f32);
    zz.set_relt(2, 2, p[7] as f32);
    zz.set_relt(3, 3, p[8] as f32);
    zz.set_relt(4, 4, 1.0);
    let mut sh = identity(g, out, err)?;
    sh.set_relt(1, 2, p[9] as f32);
    sh.set_relt(1, 3, p[10] as f32);
    sh.set_relt(2, 3, p[11] as f32);
    let given = m.is_some();
    let mut fresh = None;
    let dst: &mut Matrix = match m {
        Some(m) => {
            if matrix_multiply_d(g, out, err, Some(&t), Some(&r), Some(m))?.is_null() {
                // M = NULL: the next product's m1 is null (the program's end)
                matrix_multiply_d(g, out, err, None, Some(&zz), None)?;
                return Ok(Ret::Null);
            }
            m
        }
        None => fresh.insert(matrix_multiply_d(g, out, err, Some(&t), Some(&r), None)?.unwrap_new()),
    };
    mul_into_left(g, out, err, dst, &zz, true)?;
    mul_into_left(g, out, err, dst, &sh, true)?;
    Ok(match fresh {
        Some(f) => Ret::New(f),
        None if given => Ret::Given,
        None => Ret::Null,
    })
}

/// `TranformExtractAffineParams(M, p)`: the twelve parameters of
/// [`tranform_affine_params2_matrix`] from a matrix: the translation from its last column; the
/// QR decomposition (`MatrixQRdecomposition` into 4 x 4 matrices) with `Q P` and `P R`, `P` the
/// diagonal of the signs of `R`'s diagonal (its first negated again when `M`'s determinant, in
/// `float`, is below 0; `float` products); the rotations `atan2(Q23, Q33)`, `atan2(Q13,
/// sqrt(Q23² + Q33²))` (`pow(q, 2.0)`), `atan2(Q12, Q11)` in degrees: the first and the last of
/// two floats, so C++'s `atan2f` and `* 180` in `float`, the middle one in `double`, each then
/// divided by pi in `double`; the scales
/// `R`'s diagonal; the shears `R12 / R11`, `R13 / R11`, `R23 / R22` (`float` divisions). `None`
/// for no matrix (after `ERROR: TranformExtractAffineParams() input matrix is NULL` on standard
/// output).
pub fn tranform_extract_affine_params(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: Option<&Matrix>,
) -> Result<Option<[f64; 12]>, Fatal> {
    let Some(m) = m else {
        let _ = out.write_all(b"ERROR: TranformExtractAffineParams() input matrix is NULL\n");
        return Ok(None);
    };
    let mut p = [0.0f64; 12];
    p[0] = f64::from(m.relt(1, 4));
    p[1] = f64::from(m.relt(2, 4));
    p[2] = f64::from(m.relt(3, 4));
    let mut q = Matrix::real(4, 4);
    let mut r = Matrix::real(4, 4);
    matrix_qr_decomposition(g, out, err, Some(m), Some(&mut q), Some(&mut r));
    let mut pm = Matrix::real(4, 4);
    for c in 1..=4 {
        pm.set_relt(c, c, 1.0);
        if r.relt(c, c) < 0.0 {
            pm.set_relt(c, c, -1.0);
        }
    }
    let det = f64::from(matrix_determinant(g, err, m));
    if det < 0.0 {
        pm.set_relt(1, 1, sse::mul(pm.relt(1, 1), -1.0));
    }
    mul_into_left(g, out, err, &mut q, &pm, false)?;
    let r0 = r.clone();
    matrix_multiply(g, out, err, Some(&pm), Some(&r0), Some(&mut r))?;
    let libm = Libm::new(g.mode());
    let pow2 = |x: f64| nan_of(libm.pow(x, 2.0), x);
    // atan2 of two floats is C++'s float overload (atan2f), its result times 180 in float, then
    // divided by pi in double (the official build's object code)
    let atan2f = |y: f32, x: f32| {
        if x.is_nan() || y.is_nan() {
            sse::add(x, y)
        } else {
            libm.atan2f(y, x)
        }
    };
    let degf = |a: f32| sse::div64(f64::from(sse::mul(a, 180.0)), PI);
    let qf = |row, col| q.relt(row, col);
    p[3] = degf(atan2f(qf(2, 3), qf(3, 3)));
    let h = sse::sqrt64(sse::add64(pow2(f64::from(qf(2, 3))), pow2(f64::from(qf(3, 3)))));
    p[4] = sse::div64(sse::mul64(atan2(f64::from(qf(1, 3)), h), 180.0), PI);
    p[5] = degf(atan2f(qf(1, 2), qf(1, 1)));
    note_glibc07(g, &libm, "atan2f and pow");
    p[6] = f64::from(r.relt(1, 1));
    p[7] = f64::from(r.relt(2, 2));
    p[8] = f64::from(r.relt(3, 3));
    p[9] = f64::from(sse::div(r.relt(1, 2), r.relt(1, 1)));
    p[10] = f64::from(sse::div(r.relt(1, 3), r.relt(1, 1)));
    p[11] = f64::from(sse::div(r.relt(2, 3), r.relt(2, 2)));
    Ok(Some(p))
}

/// `LTAmat2RotMat(lta)`: the array inverted in place (`LTAinvert(lta, lta)`), its first matrix
/// replaced by the rotation alone of its affine parameters (translation 0, scales 1, shears 0:
/// [`tranform_affine_params2_matrix`] copied into it), inverted again. 0. FS-504.
pub fn lta_mat2rot_mat(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta: &mut Lta,
) -> Result<i32, Fatal> {
    lta_invert(g, out, err, InvertInto::Same(lta))?;
    if lta.num_xforms <= 0 || lta.xforms.is_empty() || lta.xforms[0].m_l.is_none() {
        outside(g, err, "LTAmat2RotMat", "no first transform's matrix")?;
        return Ok(0);
    }
    let mut par = tranform_extract_affine_params(g, out, err, lta.xforms[0].m_l.as_ref())?
        .expect("a matrix");
    for v in &mut par[0..3] {
        *v = 0.0;
    }
    for v in &mut par[6..9] {
        *v = 1.0;
    }
    for v in &mut par[9..12] {
        *v = 0.0;
    }
    let t = tranform_affine_params2_matrix(g, out, err, &par, None)?.unwrap_new();
    if let Some(ml) = lta.xforms[0].m_l.as_mut() {
        matrix_copy(g, out, err, Some(&t), Some(ml))?;
    }
    lta_invert(g, out, err, InvertInto::Same(lta))?;
    tranform_extract_affine_params(g, out, err, lta.xforms[0].m_l.as_ref())?;
    Ok(0)
}

/// `RMSregDiffMJ(T1, T2, radius)`: with `Q = T1 - T2` (`float`), `M` its upper-left 3 x 3 and `t`
/// its translation, `sqrt(radius² trace(MᵀM) / 5 + tᵀt)` (`MatrixMultiplyD`'s products, stored
/// as `float`, the trace summed in `float`, the rest in `double`). FS-504 for matrices of other
/// sizes than 4 x 4.
pub fn rms_reg_diff_mj(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t1: &Matrix,
    t2: &Matrix,
    radius: f64,
) -> Result<f64, Fatal> {
    let q = match matrix_subtract(g, out, err, t1, t2, None)?.into_new() {
        Some(q) if q.rows >= 3 && q.cols >= 4 => q,
        _ => {
            outside(g, err, "RMSregDiffMJ", "matrices that are not 4 x 4")?;
            return Ok(0.0);
        }
    };
    let mut mm = Matrix::real(3, 3);
    let mut t = Matrix::real(3, 1);
    for r in 1..=3 {
        for c in 1..=3 {
            mm.set_relt(r, c, q.relt(r, c));
        }
        t.set_relt(r, 1, q.relt(r, 4));
    }
    let mt = matrix_transpose(g, out, err, &mm, None)?.unwrap_new();
    let mtm = matrix_multiply_d(g, out, err, Some(&mt), Some(&mm), None)?.unwrap_new();
    let trace = f64::from(matrix_trace(g, err, &mtm));
    let tt = matrix_transpose(g, out, err, &t, None)?.unwrap_new();
    let ttt = matrix_multiply_d(g, out, err, Some(&tt), Some(&t), None)?.unwrap_new();
    let a = sse::div64(sse::mul64(sse::mul64(radius, radius), trace), 5.0);
    Ok(sse::sqrt64(sse::add64(a, f64::from(ttt.relt(1, 1)))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    /// The parameters of a matrix made from parameters come back (to float precision).
    #[test]
    fn affine_parameters_round_trip() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut g = Globals::new(mode);
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let p = [3.0, -4.5, 12.25, 10.0, -20.0, 35.0, 1.1, 0.9, 1.05, 0.02, -0.03, 0.01];
            let m = tranform_affine_params2_matrix(&mut g, &mut out, &mut err, &p, None)
                .unwrap()
                .unwrap_new();
            let q = tranform_extract_affine_params(&mut g, &mut out, &mut err, Some(&m))
                .unwrap()
                .unwrap();
            for (a, b) in p.iter().zip(q.iter()) {
                assert!((a - b).abs() < 1e-4 * a.abs().max(1.0), "{p:?} {q:?}");
            }
        }
    }

    /// A rotation about z by 90 degrees; no matrix gives no parameters.
    #[test]
    fn angles_and_no_matrix() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let r = mri_angles2rot_mat(&mut g, &mut out, &mut err, &[0.0, 0.0, PI / 2.0]).unwrap();
        assert!((r.relt(1, 2) + 1.0).abs() < 1e-7 && (r.relt(2, 1) - 1.0).abs() < 1e-7);
        assert_eq!(r.relt(4, 4), 1.0);
        let none = tranform_extract_affine_params(&mut g, &mut out, &mut err, None).unwrap();
        assert!(none.is_none());
        assert_eq!(
            out,
            b"ERROR: TranformExtractAffineParams() input matrix is NULL\n"
        );
    }

    /// Two equal registrations differ by 0; a shift of (3, 4, 0) by 5.
    #[test]
    fn rms_difference() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let a = identity(&mut g, &mut out, &mut err).unwrap();
        let mut b = a.clone();
        assert_eq!(rms_reg_diff_mj(&mut g, &mut out, &mut err, &a, &b, 70.0).unwrap(), 0.0);
        b.set_relt(1, 4, 3.0);
        b.set_relt(2, 4, 4.0);
        assert_eq!(rms_reg_diff_mj(&mut g, &mut out, &mut err, &a, &b, 70.0).unwrap(), 5.0);
    }
}
