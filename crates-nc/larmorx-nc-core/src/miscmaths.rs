// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.17: fsl-miscmaths 2203.2 (miscmaths.{h,cc}); `mean`, `var`,
// `quantile`, `percentile`, `interp1` and `seq` from FSL 6.0.7.23: fsl-miscmaths 2412.6. Copyright (C)
// 1999-2018 University of Oxford (Mark Jenkinson et al.). Rust translation Copyright 2026 Karel
// Lopez Vilaret; modified from the original. Distributed under the FSL Licence (see LICENSE).
//! The transform algebra of FSL's registration tools: Euler rotations about a centre
//! (`make_rot`, `construct_rotmat_euler`, `rotmat2euler`), the composition and decomposition of
//! affine matrices into rotations, translations, scales and skews (`compose_aff`,
//! `decompose_aff`), the RMS deviation between two transforms (`rms_deviation`), and FSL's
//! reader of matrix text files (`read_ascii_matrix`).
//!
//! Every expression follows Armadillo's evaluation as FSL's binaries perform it ([`crate::arma`]);
//! the rotations' sines and cosines are `float`s (`sincosf` of a `float` angle) and the Euler
//! angles come from glibc's `atan2f` ([`crate::fdlibm`]).

use crate::arma::{self, Mat};
use crate::fdlibm::atan2f;
use crate::sse::{S32, S64};

/// `norm2(x)`: `sqrt(SumSquare(x))`.
pub fn norm2(x: &Mat) -> f64 {
    S64(x.sum_square()).sqrt().0
}

/// `MISCMATHS::dot(a, b)`: `Sum(SP(a, b))` (Armadillo's two-accumulator dot product),
/// returned as `float`.
pub fn dot_f(a: &Mat, b: &Mat) -> f32 {
    arma::dot(a.as_slice(), b.as_slice()) as f32
}

/// `cross(a, b)` of 3-vectors.
pub fn cross(a: &Mat, b: &Mat) -> Mat {
    let (a1, a2, a3) = (S64(a.get(0)), S64(a.get(1)), S64(a.get(2)));
    let (b1, b2, b3) = (S64(b.get(0)), S64(b.get(1)), S64(b.get(2)));
    Mat::col(&[
        (a2 * b3 - a3 * b2).0,
        (a3 * b1 - a1 * b3).0,
        (a1 * b2 - a2 * b1).0,
    ])
}

/// `sincosf` of a `float` angle (GCC merges `sin(theta)` and `cos(theta)` into one call): the C
/// library's, as FSL's Linux build computes it (glibc 2.35, FMA build;
/// [`crate::glibc_float::sincosf`]).
fn sincosf(theta: f32) -> (f32, f32) {
    crate::glibc_float::sincosf(theta)
}

/// The 3 × 3 matrix-vector product of `make_rot` (`gemv_emul_tinysq` for three rows, inlined in
/// make_rot): the same value as [`arma::gemv_tinysq`] for numbers; for NaN operands, the first
/// row passes on the vector's NaN before the matrix's and adds each new product before the
/// running sum (the order FSL's build gives that row, measured on makerot's NaN and infinite
/// angles, axes and centres: `validation/probes/fsl-flirt-programs/makerot_nan.py`), the other
/// rows the matrix's first and the running sum first.
fn gemv3_rot(a: &Mat, x: &[f64]) -> Vec<f64> {
    (0..3)
        .map(|r| {
            if r == 0 {
                let mut acc = S64(x[0]) * a.at(0, 0);
                for k in 1..3 {
                    acc = (S64(x[k]) * a.at(0, k)) + acc.0;
                }
                acc.0
            } else {
                let mut acc = S64(a.at(r, 0)) * x[0];
                for k in 1..3 {
                    acc += S64(a.at(r, k)) * x[k];
                }
                acc.0
            }
        })
        .collect()
}

/// `A B` for 3 × 3 operands with [`gemv3_rot`] per column (`gemm_emul_tinysq`).
fn mul3_rot(a: &Mat, b: &Mat) -> Mat {
    let mut out = Mat::zeros(3, 3);
    for c in 0..3 {
        let y = gemv3_rot(a, b.colptr(c));
        for (r, v) in y.into_iter().enumerate() {
            out.set(r, c, v);
        }
    }
    out
}

/// `make_rot(angl, centre, rot)`: the rotation by `|angl|` about the axis `angl` through
/// `centre`.
pub fn make_rot(angl: &Mat, centre: &Mat) -> Mat {
    let mut rot = Mat::eye(4);
    let theta = norm2(angl) as f32;
    if f64::from(theta) < 1e-8 {
        return rot;
    }
    let axis = angl.div_scalar(f64::from(theta));
    let x1 = axis.clone();
    let mut x2 = Mat::col(&[-axis.get(1), axis.get(0), 0.0]);
    if norm2(&x2) <= 0.0 {
        x2 = Mat::col(&[1.0, 0.0, 0.0]);
    }
    let n2 = norm2(&x2);
    let x2 = x2.div_scalar(n2);
    let x3 = cross(&x1, &x2);
    let n3 = norm2(&x3);
    let x3 = x3.div_scalar(n3);
    let mut basis = Mat::zeros(3, 3);
    for r in 0..3 {
        basis.set(r, 0, x2.get(r));
        basis.set(r, 1, x3.get(r));
        basis.set(r, 2, x1.get(r));
    }
    let (s, c) = sincosf(theta);
    let mut rotcore = Mat::eye(3);
    rotcore.set(0, 0, f64::from(c));
    rotcore.set(1, 1, f64::from(c));
    rotcore.set(0, 1, f64::from(s));
    rotcore.set(1, 0, -f64::from(s));
    // basischange * rotcore * basischange.t(): (B R) Bᵀ, the transpose applied by the product.
    let br = mul3_rot(&basis, &rotcore);
    let r3 = mul3_rot(&br, &basis.t());
    rot.set_sub(0, 0, &r3);
    // trans = (I - rot(1:3,1:3)) * centre
    let im = Mat::eye(3).minus(&r3);
    let trans = gemv3_rot(&im, centre.as_slice());
    for (r, t) in trans.into_iter().enumerate() {
        rot.set(r, 3, t);
    }
    rot
}

/// `construct_rotmat_euler(params, n, aff, centre)`: `R = Rx Ry Rz` about `centre`, then the
/// translations (`n` parameters used).
pub fn construct_rotmat_euler(params: &Mat, n: i32, centre: &Mat) -> Mat {
    let mut aff = Mat::eye(4);
    if n <= 0 {
        return aff;
    }
    for k in 0..3 {
        let mut angl = Mat::zeros(3, 1);
        angl.put(k, params.get(k));
        let newaff = make_rot(&angl, centre);
        aff = arma::mul_tinysq(&aff, &newaff);
        if n == k as i32 + 1 {
            return aff;
        }
    }
    for k in 0..3 {
        let v = (S64(aff.at(k, 3)) + params.get(3 + k)).0;
        aff.set(k, 3, v);
        if n == k as i32 + 4 {
            return aff;
        }
    }
    aff
}

/// `construct_rotmat_quat(params, n, aff, centre)`: the rotation of the unit quaternion whose
/// last three components are `params(1..3)` (the first, `w`, the square root of what is left,
/// in `float`), about `centre`, then the translations. Its messages go to `msgs` (FSL prints
/// them on standard error at every call): fewer than 3 parameters, and components whose squares
/// sum above 1 (the identity is kept then). With `n == 3` the translations are added too (FSL
/// returns after them only from 4 parameters on).
pub fn construct_rotmat_quat(params: &Mat, n: i32, centre: &Mat, msgs: &mut Vec<String>) -> Mat {
    let mut aff = Mat::eye(4);
    if n <= 0 {
        return aff;
    }
    if (1..3).contains(&n) {
        msgs.push(format!("Can only do 3 or more, not {n}"));
    }
    let (p1, p2, p3) = (S64(params.get(0)), S64(params.get(1)), S64(params.get(2)));
    // float w2 = 1.0 - Sqr(p1) - Sqr(p2) - Sqr(p3): double arithmetic, stored to a float
    let w2 = (((S64(1.0) - p1 * p1) - p2 * p2) - p3 * p3).0 as f32;
    if f64::from(w2) < 0.0 {
        msgs.push("Parameters do not form a valid axis - greater than unity".to_owned());
        return aff;
    }
    let w = S32(w2).sqrt();
    let (x, y, z) = (S32(p1.0 as f32), S32(p2.0 as f32), S32(p3.0 as f32));
    let two = S32(2.0);
    let one = S32(1.0);
    let set = |aff: &mut Mat, r: usize, c: usize, v: S32| aff.set(r, c, f64::from(v.0));
    set(&mut aff, 0, 0, (one - two * y * y) - two * z * z);
    set(&mut aff, 1, 1, (one - two * x * x) - two * z * z);
    set(&mut aff, 2, 2, (one - two * x * x) - two * y * y);
    set(&mut aff, 0, 1, two * x * y - two * w * z);
    set(&mut aff, 1, 0, two * x * y + two * w * z);
    set(&mut aff, 0, 2, two * x * z + two * w * y);
    set(&mut aff, 2, 0, two * x * z - two * w * y);
    set(&mut aff, 1, 2, two * y * z - two * w * x);
    set(&mut aff, 2, 1, two * y * z + two * w * x);
    // x' = R x + (I - R) centre
    let trans = arma::gemv_tinysq(&aff.sub(0, 3, 0, 3), centre.as_slice(), false);
    for (r, t) in trans.into_iter().enumerate() {
        aff.set(r, 3, (S64(centre.get(r)) - t).0);
    }
    for k in 0..3 {
        let v = (S64(aff.at(k, 3)) + params.get(3 + k)).0;
        aff.set(k, 3, v);
        if n == k as i32 + 4 {
            return aff;
        }
    }
    aff
}

/// `rotmat2quat(quaternion, rotmat)`: the quaternion's `(b, c, d)` of `nifti_mat44_to_quatern`
/// (FSL's legacy NIfTI routine, x87 precision) of the rotation stored in a `float` `mat44`.
pub fn rotmat2quat(rotmat: &Mat) -> [f64; 3] {
    let mut rot = [[0.0f32; 4]; 4];
    for (i, row) in rot.iter_mut().enumerate().take(3) {
        for (j, v) in row.iter_mut().enumerate().take(3) {
            *v = rotmat.at(i, j) as f32;
        }
    }
    // FSL leaves the mat44's last row and column unset: they reach only outputs it discards.
    let q = crate::nifti::mat44_to_quatern(&rot);
    [q.qb, q.qc, q.qd]
}

/// `compose_aff(params, n, centre, aff, construct_rotmat_quat)` (`-anglerep quaternion`); the
/// rotation's messages go to `msgs`.
pub fn compose_aff_quat(params: &Mat, n: i32, centre: &Mat, msgs: &mut Vec<String>) -> Mat {
    if n <= 0 {
        return Mat::zeros(4, 4);
    }
    let aff = construct_rotmat_quat(params, n, centre, msgs);
    scale_skew(aff, params, n, centre)
}

/// `compose_aff(params, n, centre, aff, construct_rotmat_euler)`.
pub fn compose_aff(params: &Mat, n: i32, centre: &Mat) -> Mat {
    if n <= 0 {
        // compose_aff returns before touching aff; callers pass a fresh matrix.
        return Mat::zeros(4, 4);
    }
    let aff = construct_rotmat_euler(params, n, centre);
    scale_skew(aff, params, n, centre)
}

/// `compose_aff`'s scales and skews after the rotation `aff` (none up to 6 parameters).
fn scale_skew(aff: Mat, params: &Mat, n: i32, centre: &Mat) -> Mat {
    if n <= 6 {
        return aff;
    }
    let mut scale = Mat::eye(4);
    scale.set(0, 0, params.get(6));
    scale.set(1, 1, if n >= 8 { params.get(7) } else { params.get(6) });
    scale.set(2, 2, if n >= 9 { params.get(8) } else { params.get(6) });
    let sc = arma::gemv_tinysq(&scale.sub(0, 3, 0, 3), centre.as_slice(), false);
    for r in 0..3 {
        scale.set(r, 3, (S64(centre.get(r)) - sc[r]).0);
    }
    let mut skew = Mat::eye(4);
    if n >= 10 {
        skew.set(0, 1, params.get(9));
        if n >= 11 {
            skew.set(0, 2, params.get(10));
        }
        if n >= 12 {
            skew.set(1, 2, params.get(11));
        }
    }
    let kc = arma::gemv_tinysq(&skew.sub(0, 3, 0, 3), centre.as_slice(), false);
    for r in 0..3 {
        skew.set(r, 3, (S64(centre.get(r)) - kc[r]).0);
    }
    // aff * skew * scale: (aff skew) scale.
    arma::mul_tinysq(&arma::mul_tinysq(&aff, &skew), &scale)
}

/// `rotmat2euler(angles, rotmat)` (`R = Rx Ry Rz`): float sines and cosines, glibc's `atan2f`.
pub fn rotmat2euler(rotmat: &Mat) -> [f64; 3] {
    let r = |i: usize, j: usize| S64(rotmat.at(i - 1, j - 1));
    let cy = (r(1, 1) * r(1, 1) + r(1, 2) * r(1, 2)).sqrt().0 as f32;
    if f64::from(cy) < 1e-4 {
        let cx = r(2, 2).0 as f32;
        let sx = (-r(3, 2)).0 as f32;
        let sy = (-r(1, 3)).0 as f32;
        [f64::from(atan2f(sx, cx)), f64::from(atan2f(sy, 0.0)), 0.0]
    } else {
        let cz = (r(1, 1) / f64::from(cy)).0 as f32;
        let sz = (r(1, 2) / f64::from(cy)).0 as f32;
        let cx = (r(3, 3) / f64::from(cy)).0 as f32;
        let sx = (r(2, 3) / f64::from(cy)).0 as f32;
        let sy = (-r(1, 3)).0 as f32;
        [
            f64::from(atan2f(sx, cx)),
            f64::from(atan2f(sy, cy)),
            f64::from(atan2f(sz, cz)),
        ]
    }
}

/// `decompose_aff(params, affmat, centre, rotmat2euler)`: the 12 parameters (3 rotations, 3
/// translations about `centre`, 3 scales, 3 skews) of `affmat = rotmat * skew * scale`.
pub fn decompose_aff(affmat: &Mat, centre: &Mat) -> [f64; 12] {
    decompose_aff_with(affmat, centre, rotmat2euler)
}

/// `decompose_aff(params, affmat, centre, rotmat2quat)` (`-anglerep quaternion`): the
/// rotation's parameters are the quaternion's last three components.
pub fn decompose_aff_quat(affmat: &Mat, centre: &Mat) -> [f64; 12] {
    decompose_aff_with(affmat, centre, rotmat2quat)
}

fn decompose_aff_with(
    affmat: &Mat,
    centre: &Mat,
    rotmat2params: fn(&Mat) -> [f64; 3],
) -> [f64; 12] {
    decompose_aff_impl(affmat, centre, rotmat2params, false).unwrap_or([f64::NAN; 12])
}

/// The message of the `std::runtime_error` Armadillo throws when `inv(A) * B`'s solve finds
/// `A` singular.
pub const SOLVE_ERROR: &str =
    "matrix multiplication: problem with matrix inverse; suggest to use solve() instead";
/// The message of the `std::runtime_error` Armadillo's `inv()` throws on a singular matrix.
pub const INV_ERROR: &str = "inv(): matrix is singular";

/// [`decompose_aff`] (`rotmat2euler`, or with `quat` `rotmat2quat`) where FSL throws: a scale
/// of 0 makes the solve's matrix singular, and Armadillo throws [`SOLVE_ERROR`] (the
/// translations and rotations are never computed).
pub fn decompose_aff_try(
    affmat: &Mat,
    centre: &Mat,
    quat: bool,
) -> Result<[f64; 12], &'static str> {
    let f = if quat { rotmat2quat } else { rotmat2euler };
    decompose_aff_impl(affmat, centre, f, true)
}

fn decompose_aff_impl(
    affmat: &Mat,
    centre: &Mat,
    rotmat2params: fn(&Mat) -> [f64; 3],
    strict: bool,
) -> Result<[f64; 12], &'static str> {
    let mut params = [0.0f64; 12];
    let aff3 = affmat.sub(0, 3, 0, 3);
    let x = affmat.sub(0, 3, 0, 1);
    let y = affmat.sub(0, 3, 1, 2);
    let z = affmat.sub(0, 3, 2, 3);
    let sx = S32(norm2(&x) as f32);
    let dxy = S32(dot_f(&x, &y));
    let sy = (S32(dot_f(&y, &y)) - ((dxy * dxy) / (sx * sx))).sqrt();
    let a = dxy / (sx * sy);
    let (sx, sy, a) = (sx.0, sy.0, a.0);
    let x0 = x.div_scalar(f64::from(sx));
    let y0 = y.div_scalar(f64::from(sy)).minus(&x0.scale(f64::from(a)));
    let d0 = S32(dot_f(&x0, &z));
    let d1 = S32(dot_f(&y0, &z));
    let sz = (S32(dot_f(&z, &z)) - d0 * d0 - d1 * d1).sqrt();
    let b = (S32(dot_f(&x0, &z)) / sz).0;
    let c = (S32(dot_f(&y0, &z)) / sz).0;
    let sz = sz.0;
    params[6] = f64::from(sx);
    params[7] = f64::from(sy);
    params[8] = f64::from(sz);
    let mut scales = Mat::zeros(3, 3);
    scales.set(0, 0, f64::from(sx));
    scales.set(1, 1, f64::from(sy));
    scales.set(2, 2, f64::from(sz));
    let skew = Mat::from_rows(
        3,
        3,
        &[
            1.0,
            f64::from(a),
            f64::from(b),
            0.0,
            1.0,
            f64::from(c),
            0.0,
            0.0,
            1.0,
        ],
    );
    params[9] = f64::from(a);
    params[10] = f64::from(b);
    params[11] = f64::from(c);
    // aff3 * scales.i() * skew(1:3,1:3).i(): Armadillo evaluates A * solve(B, inv(C)).
    let skew_inv = match arma::inv(&skew) {
        Ok(m) => m,
        Err(_) if strict => return Err(INV_ERROR),
        Err(_) => Mat::from_rows(3, 3, &[f64::NAN; 9]),
    };
    let solved = match arma::solve(&scales, &skew_inv) {
        Ok(m) => m,
        Err(_) if strict => return Err(SOLVE_ERROR),
        Err(_) => Mat::from_rows(3, 3, &[f64::NAN; 9]),
    };
    let rotmat = arma::mul_tinysq(&aff3, &solved);
    // transl = aff3*centre + affmat(1:3,4) - centre
    let ac = arma::gemv_tinysq(&aff3, centre.as_slice(), false);
    for i in 0..3 {
        params[3 + i] = ((S64(ac[i]) + affmat.at(i, 3)) - centre.get(i)).0;
    }
    let rp = rotmat2params(&rotmat);
    params[..3].copy_from_slice(&rp);
    // An affine all of one NaN (FSL-14's matrices): with Armadillo 14.2.1's LAPACK path (the
    // skew's triangular inverse, the solve by `dgesv`) FSL 6.0.7.23's binary gives every
    // rotation the affine's own NaN, as this order does (FSL 6.0.7.17's closed-form inverse gave
    // the y rotation the other sign: validation/probes/fsl-mcflirt-affine-trace, nan_decompose.cc
    // built against each release).
    Ok(params)
}

/// `sqrtaff(mat)`: an approximate square root of an affine matrix: the quaternion rotation
/// halved (its axis scaled by `1 / (2 cos(t/2))`, `t = asin(|q|)`), the square roots of the
/// scales, half the skews, and the translation that both halves share (`(mat S⁻¹ K⁻¹ R⁻¹ + I)`'s
/// 3 × 3 part solved against the translation). Armadillo's errors are returned: [`INV_ERROR`]
/// from an inverse, [`SOLVE_ERROR`] from the solve. The double `cos` comes from `libm`:
/// correctly rounded by default, glibc 2.35's with `reproduce` (GLIBC-07; IBM's code, LGPL:
/// [`larmorx_core::math::cos_glibc`], the clean-room implementation of `specs/glibc-dbl.md`).
/// `asin` is the correctly rounded one in both modes (the user's decision of 2026-11-04: glibc's
/// `asin` stays out of larmorx; it differs from it in the last bit on 0.16 % of arguments).
pub fn sqrtaff(mat: &Mat, libm: &larmorx_core::math::Libm) -> Result<Mat, &'static str> {
    let zero = Mat::col(&[0.0, 0.0, 0.0]);
    let mut params = decompose_aff_try(mat, &zero, true)?;
    let sint =
        (S64(params[0]) * params[0] + S64(params[1]) * params[1] + S64(params[2]) * params[2])
            .sqrt()
            .0;
    let t = larmorx_core::math::asin(sint);
    let factor = (S64(1.0) / (S64(2.0) * libm.cos((S64(0.5) * t).0))).0;
    for k in 0..3 {
        params[k] = (S64(factor) * params[k]).0;
    }
    for k in 6..9 {
        params[k] = S64(params[k]).sqrt().0;
    }
    for k in 9..12 {
        params[k] = (S64(0.5) * params[k]).0;
    }
    let mut msgs = Vec::new();
    let mut rot = construct_rotmat_quat(&Mat::col(&params), 3, &zero, &mut msgs);
    rot.set(0, 3, 0.0);
    rot.set(1, 3, 0.0);
    rot.set(2, 3, 0.0);
    let mut scale = Mat::eye(4);
    scale.set(0, 0, params[6]);
    scale.set(1, 1, params[7]);
    scale.set(2, 2, params[8]);
    let mut skew = Mat::eye(4);
    skew.set(0, 1, params[9]);
    skew.set(0, 2, params[10]);
    skew.set(1, 2, params[11]);
    let trans = Mat::col(&[params[3], params[4], params[5]]);
    // (mat * scale.i() * skew.i() * rot.i() + id4): four operands, each inverse evaluated, the
    // product ((A B) C) D (Armadillo's glue_times_redirect<4>, equal sizes).
    let si = arma::inv(&scale).map_err(|_| INV_ERROR)?;
    let ki = arma::inv(&skew).map_err(|_| INV_ERROR)?;
    let ri = arma::inv(&rot).map_err(|_| INV_ERROR)?;
    let prod = arma::mul_tinysq(&arma::mul_tinysq(&arma::mul_tinysq(mat, &si), &ki), &ri);
    let sum = prod.plus(&Mat::eye(4));
    // .SubMatrix(1,3,1,3).i() * trans: inv(A) * B is solve(A, B).
    let th = arma::solve(&sum.sub(0, 3, 0, 3), &trans).map_err(|_| SOLVE_ERROR)?;
    // matnew = rot * skew * scale: (rot * skew) * scale
    let mut matnew = arma::mul_tinysq(&arma::mul_tinysq(&rot, &skew), &scale);
    matnew.set(0, 3, th.get(0));
    matnew.set(1, 3, th.get(1));
    matnew.set(2, 3, th.get(2));
    Ok(matnew)
}

/// `rms_deviation(affmat1, affmat2, centre, rmax)` of two 4 × 4 matrices.
pub fn rms_deviation(a1: &Mat, a2: &Mat, centre: &Mat, rmax: f32) -> Result<f32, arma::Singular> {
    // isodiff = a1 * a2.i() - I
    let a2i = arma::inv(a2)?;
    let isodiff = arma::mul_tinysq(a1, &a2i).minus(&Mat::eye(4));
    let adiff = isodiff.sub(0, 3, 0, 3);
    let ac = arma::gemv_tinysq(&adiff, centre.as_slice(), false);
    let tr: Vec<f64> = (0..3).map(|i| (S64(isodiff.at(i, 3)) + ac[i]).0).collect();
    let tt = S64(arma::dot(&tr, &tr));
    let r2 = f64::from(rmax * rmax) / 5.0;
    let trace = arma::trace_at_b(&adiff, &adiff);
    // GCC adds the dot product to the trace term (`(r2 * trace) + tt`: the trace's NaN first;
    // FSL 6.0.7.23's libfsl-miscmaths, disassembled).
    Ok((S64(r2) * trace + tt).sqrt().0 as f32)
}

// ------------------------------------------------------------------------------------------------
// read_ascii_matrix (C++ stream parsing)

/// `isNumber(token)`: `strtod` consumes the whole (non-empty) token.
fn is_number(token: &str) -> bool {
    !token.is_empty() && strtod_len(token) == token.len()
}

/// The number of bytes `strtod` would consume from `s` (glibc: decimal and hexadecimal forms,
/// `inf`, `infinity`, `nan`, `nan(...)`, after optional white space and a sign).
fn strtod_len(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i] == b' ' || (9..=13).contains(&b[i])) {
        i += 1;
    }
    let start_ws = i;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let lower = s[i..].to_ascii_lowercase();
    if lower.starts_with("infinity") {
        return i + 8;
    }
    if lower.starts_with("inf") {
        return i + 3;
    }
    if lower.starts_with("nan") {
        let mut j = i + 3;
        if j < b.len() && b[j] == b'(' {
            let mut k = j + 1;
            while k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'_') {
                k += 1;
            }
            if k < b.len() && b[k] == b')' {
                j = k + 1;
            }
        }
        return j;
    }
    let hex = lower.starts_with("0x");
    let digit = |c: u8| {
        if hex {
            c.is_ascii_hexdigit()
        } else {
            c.is_ascii_digit()
        }
    };
    let mut j = if hex { i + 2 } else { i };
    let mut mant = 0;
    while j < b.len() && digit(b[j]) {
        j += 1;
        mant += 1;
    }
    if j < b.len() && b[j] == b'.' {
        j += 1;
        while j < b.len() && digit(b[j]) {
            j += 1;
            mant += 1;
        }
    }
    if mant == 0 {
        if hex {
            // "0x" alone: strtod reads the "0".
            return i + 1;
        }
        let _ = start_ws;
        return 0;
    }
    let exp_char = if hex { b'p' } else { b'e' };
    if j < b.len() && b[j].to_ascii_lowercase() == exp_char {
        let mut k = j + 1;
        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            k += 1;
        }
        let ds = k;
        while k < b.len() && b[k].is_ascii_digit() {
            k += 1;
        }
        if k > ds {
            j = k;
        }
    }
    j
}

/// One `istream >> double` extraction as libstdc++ performs it on a line: skip white space,
/// collect `[+-]digits[.digits][(e|E)[+-]digits]`, convert with `strtod` (correctly rounded).
/// `None` when nothing numeric is read (the stream's failbit); `eof` is whether the end of the
/// line was reached while reading.
fn extract_double(line: &[u8], pos: &mut usize) -> (Option<f64>, bool) {
    let b = line;
    while *pos < b.len() && (b[*pos] == b' ' || (9..=13).contains(&b[*pos])) {
        *pos += 1;
    }
    if *pos >= b.len() {
        return (None, true);
    }
    let start = *pos;
    let mut j = start;
    if b[j] == b'+' || b[j] == b'-' {
        j += 1;
    }
    let mut digits = 0;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
        digits += 1;
    }
    if j < b.len() && b[j] == b'.' {
        j += 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
            digits += 1;
        }
    }
    if digits > 0 && j < b.len() && (b[j] == b'e' || b[j] == b'E') {
        let mut k = j + 1;
        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            k += 1;
        }
        let ds = k;
        while k < b.len() && b[k].is_ascii_digit() {
            k += 1;
        }
        // num_get consumes the exponent characters it accepted; without digits the conversion
        // fails.
        if k == ds {
            *pos = k;
            return (None, k >= b.len());
        }
        j = k;
    }
    *pos = j;
    let eof = j >= b.len();
    if digits == 0 {
        return (None, eof);
    }
    let text = std::str::from_utf8(&b[start..j]).unwrap_or("0");
    (text.parse::<f64>().ok(), eof)
}

/// Why `read_ascii_matrix` could not read a file: FSL prints "Could not open matrix file"
/// and returns an empty matrix, or throws "Could not find enough numbers in matrix file".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatrixFileError {
    CannotOpen,
    NotEnoughNumbers,
}

/// `read_ascii_matrix(filename)`: the matrix in a text file, its size taken from the first
/// numeric line (columns) and the number of lines that start with a number (rows); lines that
/// do not start with a number are skipped. An empty name gives an empty matrix.
pub fn read_ascii_matrix(filename: &str) -> Result<Mat, MatrixFileError> {
    if filename.is_empty() {
        return Ok(Mat::zeros(0, 0));
    }
    let Ok(bytes) = std::fs::read(filename) else {
        return Err(MatrixFileError::CannotOpen);
    };
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.split('\n').collect();
    // A final empty piece after a trailing newline is not a line for getline.
    let n_lines = if text.ends_with('\n') {
        lines.len() - 1
    } else {
        lines.len()
    };
    let lines = &lines[..n_lines];
    let first_token = |l: &str| l.split_ascii_whitespace().next().unwrap_or("").to_owned();
    // skip_alpha: the lines that start with a number (FSL reads only those).
    let numeric: Vec<usize> = (0..lines.len())
        .filter(|&i| is_number(&first_token(lines[i])))
        .collect();
    let Some(&first) = numeric.first() else {
        return Ok(Mat::zeros(0, 0));
    };
    // Columns: the tokens of the first numeric line; rows: every numeric line from it on.
    let ncols = lines[first].split_ascii_whitespace().count();
    let nrows = numeric.len();
    let mut mat = Mat::zeros(nrows, ncols);
    for (r, &li) in numeric.iter().enumerate() {
        // istringstream of the line (skip_alpha appends a blank) and `sline >> newnum` per
        // column: a failed extraction sets newnum to 0 and every later one on this line does
        // nothing; running out of numbers sets eof, which FSL reports.
        let line = format!("{} ", lines[li]);
        let lb = line.as_bytes();
        let mut pos = 0;
        let mut failed = false;
        let mut newnum = 0.0f64;
        for c in 0..ncols {
            if !failed {
                let (v, eof) = extract_double(lb, &mut pos);
                if eof && v.is_none() {
                    return Err(MatrixFileError::NotEnoughNumbers);
                }
                match v {
                    Some(v) => newnum = v,
                    None => {
                        newnum = 0.0;
                        failed = true;
                    }
                }
            }
            mat.set(r, c, newnum);
        }
    }
    Ok(mat)
}

// ------------------------------------------------------------------------------------------------
// The column statistics of fslmaths' -roc (FSL 6.0.7.23: fsl-miscmaths 2412.6, miscmaths.cc)

/// `mean(v)` of a column vector (`dim` 1): `res += v(r) / N` row by row (SSE's NaN).
pub fn mean_col(v: &[f64]) -> f64 {
    use crate::sse::{add_f64, div_f64};
    let n = v.len() as i32 as f64;
    v.iter().fold(0.0, |s, &x| add_f64(s, div_f64(x, n)))
}

/// `var(v)` of a column vector: `res += (v(r) - m) * (v(r) - m) / (N - 1)` with `m` the
/// [`mean_col`], 0 for one value or none.
pub fn var_col(v: &[f64]) -> f64 {
    use crate::sse::{add_f64, div_f64, mul_f64, sub_f64};
    let n = v.len() as i32;
    if n <= 1 {
        return 0.0;
    }
    let m = mean_col(v);
    let d = f64::from(n - 1);
    v.iter().fold(0.0, |s, &x| {
        let a = sub_f64(x, m);
        add_f64(s, div_f64(mul_f64(a, a), d))
    })
}

/// `interp1(x, y, xi)`: `y` at `xi` by linear interpolation in the ascending `x`, in `float`
/// (`y` of the last row at or beyond `x`'s maximum, of the first at or below its minimum).
pub fn interp1(x: &[f64], y: &[f64], xi: f32) -> f32 {
    let xmax = crate::arma::direct_max(x);
    let xmin = crate::arma::direct_min(x);
    let xd = f64::from(xi);
    if xd >= xmax {
        return y[y.len() - 1] as f32;
    }
    if xd <= xmin {
        return y[0] as f32;
    }
    let mut ind = 1usize;
    while xd >= x[ind] {
        ind += 1;
    }
    let (xa, xb) = (x[ind - 1] as f32, x[ind] as f32);
    let (ya, yb) = (y[ind - 1] as f32, y[ind] as f32);
    use crate::sse::{add_f32, div_f32, mul_f32, sub_f32};
    let t = mul_f32(div_f32(sub_f32(xi, xa), sub_f32(xb, xa)), sub_f32(yb, ya));
    add_f32(t, ya)
}

/// `percentile(in, p)`: `in` sorted (Armadillo's `sort`, `None` for a NaN: `sort(): detected
/// NaN`), then [`interp1`] of `[y(1); y; y(num)]` at `p` over `[0; 100 (seq - 0.5) / num; 100]`.
pub fn percentile_col(v: &[f64], p: f32) -> Option<f32> {
    let mut y = v.to_vec();
    if !crate::filters::arma_sort(&mut y, false) {
        return None;
    }
    let num = y.len();
    let mut xx = Vec::with_capacity(num + 2);
    xx.push(0.0);
    for i in 1..=num {
        xx.push(((i as f64 - 0.5) * 100.0) / num as i32 as f64);
    }
    xx.push(100.0);
    let mut yy = Vec::with_capacity(num + 2);
    yy.push(y[0]);
    yy.extend_from_slice(&y);
    yy.push(y[num - 1]);
    Some(interp1(&xx, &yy, p))
}

/// `quantile(in, which)`: the percentile 0, 25, 50, 75 or 100 (`which` 0 to 4; else 0).
pub fn quantile_col(v: &[f64], which: i32) -> Option<f32> {
    let p = match which {
        1 => 25.0,
        2 => 50.0,
        3 => 75.0,
        4 => 100.0,
        _ => 0.0,
    };
    percentile_col(v, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_statistics() {
        let v = [0.25, 0.5, 0.125, 1.0];
        assert_eq!(
            mean_col(&v),
            0.25 / 4.0 + 0.5 / 4.0 + 0.125 / 4.0 + 1.0 / 4.0
        );
        assert!(var_col(&v) > 0.0);
        assert_eq!(var_col(&[3.0]), 0.0);
        // percentile 25 of 4 sorted values: between the first (12.5) and second (37.5) points
        let q1 = quantile_col(&v, 1).unwrap();
        assert_eq!(q1, 0.125 + (25.0 - 12.5) / (37.5 - 12.5) * (0.25 - 0.125));
        assert_eq!(quantile_col(&[7.0], 3), Some(7.0));
        assert_eq!(quantile_col(&v, 0), Some(0.125));
        assert_eq!(quantile_col(&v, 4), Some(1.0));
        assert_eq!(quantile_col(&[1.0, f64::NAN], 1), None);
    }

    #[test]
    fn quaternion_round_trip_and_messages() {
        let centre = Mat::col(&[10.0, 20.0, 30.0]);
        let p = Mat::col(&[
            0.01, -0.02, 0.03, 1.0, -2.0, 0.5, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0,
        ]);
        let mut msgs = Vec::new();
        let a = compose_aff_quat(&p, 6, &centre, &mut msgs);
        assert!(msgs.is_empty());
        let q = decompose_aff_quat(&a, &centre);
        for k in 0..6 {
            assert!((q[k] - p.get(k)).abs() < 1e-6, "{k}: {} {}", q[k], p.get(k));
        }
        // components beyond the unit sphere: the message, the identity rotation
        let big = Mat::col(&[0.9, 0.5, 0.1, 1.0, 2.0, 3.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
        let r = construct_rotmat_quat(&big, 6, &centre, &mut msgs);
        assert_eq!(
            msgs,
            ["Parameters do not form a valid axis - greater than unity"]
        );
        assert_eq!(r, Mat::eye(4));
        // fewer than three parameters: the message, the rotation still built
        msgs.clear();
        let _ = construct_rotmat_quat(&p, 2, &centre, &mut msgs);
        assert_eq!(msgs, ["Can only do 3 or more, not 2"]);
    }

    #[test]
    fn identity_round_trip() {
        let centre = Mat::col(&[10.0, 20.0, 30.0]);
        let p = Mat::col(&[
            0.01, -0.02, 0.03, 1.0, -2.0, 0.5, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0,
        ]);
        let a = compose_aff(&p, 6, &centre);
        let q = decompose_aff(&a, &centre);
        for k in 0..6 {
            assert!(
                (q[k] - p.get(k)).abs() < 1e-6,
                "{k}: {} vs {}",
                q[k],
                p.get(k)
            );
        }
        let i = Mat::eye(4);
        assert_eq!(rms_deviation(&i, &i, &centre, 80.0).unwrap(), 0.0);
        let r = rms_deviation(&a, &i, &centre, 80.0).unwrap();
        assert!(r > 0.0);
    }

    #[test]
    fn number_tokens() {
        assert!(is_number("1.5"));
        assert!(is_number("-2e3"));
        assert!(is_number("nan"));
        assert!(is_number("inf"));
        assert!(!is_number("x1"));
        assert!(!is_number(""));
        assert!(!is_number("1.5a"));
    }

    /// One of mcflirt's own `decompose_aff` calls, its inputs and FSL's outputs as bits (an
    /// LD_PRELOAD interposer on FSL 6.0.7.17's `MISCMATHS::decompose_aff`,
    /// `validation/probes/fsl-mcflirt-affine-trace`): fMRIPrep's ds002422 rest run, volume 166,
    /// the start of the third stage. With fdlibm's comment value of `aT[0]` the y rotation came
    /// out one ulp larger.
    #[test]
    fn decompose_aff_mcflirt_ds002422() {
        let a: [u64; 16] = [
            0x3fef_fe13_24b2_9f58,
            0xbf8e_e623_73b6_262c,
            0x3f8f_e095_2505_67fc,
            0x0000_0000_0000_0000,
            0x3f8f_4290_5512_8b67,
            0x3fef_fe86_5744_cff5,
            0xbf87_14a4_e940_e4e7,
            0x0000_0000_0000_0000,
            0xbf8f_85f7_bd53_176a,
            0x3f87_8fd1_019f_e000,
            0x3fef_fe7c_a436_7380,
            0x0000_0000_0000_0000,
            0xbfef_718e_395b_3ecc,
            0x3ff3_4e8b_5874_8bed,
            0xbf86_b682_d587_9440,
            0x3ff0_0000_0000_0000,
        ];
        let centre: [u64; 3] = [
            0x405e_7435_1b2e_6f4b,
            0x405a_fc3d_95fd_fa12,
            0x4050_8c30_fb57_b530,
        ];
        let want: [u64; 12] = [
            0x3f87_90aa_2000_0000,
            0x3f8f_8649_4000_0000,
            0x3f8f_43d2_8000_0000,
            0xbfd8_7a8a_47d9_5b00,
            0x3fbc_617d_8228_c000,
            0x3fe5_0032_365d_5180,
            0x3fef_ffff_e000_0000,
            0x3fef_ffff_e000_0000,
            0x3fef_ffff_e000_0000,
            0x3e13_4c0f_6000_0000,
            0xbe02_e7ca_a000_0000,
            0xbda2_78fc_6000_0000,
        ];
        let mut rows = [0.0f64; 16];
        for r in 0..4 {
            for col in 0..4 {
                rows[r * 4 + col] = f64::from_bits(a[col * 4 + r]);
            }
        }
        let m = Mat::from_rows(4, 4, &rows);
        let c = Mat::col(&centre.map(f64::from_bits));
        let got = decompose_aff(&m, &c);
        for k in 0..12 {
            assert_eq!(got[k].to_bits(), want[k], "parameter {}", k + 1);
        }
    }

    /// FSL 6.0.7.23's `decompose_aff` (libfsl-miscmaths 2412.6, through `nan_decompose.cc` of
    /// validation/probes/fsl-mcflirt-affine-trace) on affines all of one NaN, with numbers and
    /// with NaN centres: "<affine, column-major> <centre> -> <params>" as bits. Every rotation
    /// keeps the affine's NaN (FSL 6.0.7.17 gave the y rotation the other sign), the float
    /// parameters lose the payload, the translations keep it.
    #[test]
    fn decompose_aff_uniform_nan_as_fsl() {
        const CASES: [&str; 4] = [
            "fff8000000000000,fff8000000000000,fff8000000000000,0000000000000000,fff8000000000000,fff8000000000000,fff8000000000000,0000000000000000,fff8000000000000,fff8000000000000,fff8000000000000,0000000000000000,fff8000000000000,fff8000000000000,fff8000000000000,3ff0000000000000 4026000000000000,4028000000000000,402a000000000000 -> fff8000000000000,fff8000000000000,fff8000000000000,fff8000000000000,fff8000000000000,fff8000000000000,fff8000000000000,fff8000000000000,fff8000000000000,fff8000000000000,fff8000000000000,fff8000000000000",
            "7ff8000000000000,7ff8000000000000,7ff8000000000000,0000000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,0000000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,0000000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,3ff0000000000000 4026000000000000,4028000000000000,402a000000000000 -> 7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000",
            "7ff8000012345000,7ff8000012345000,7ff8000012345000,0000000000000000,7ff8000012345000,7ff8000012345000,7ff8000012345000,0000000000000000,7ff8000012345000,7ff8000012345000,7ff8000012345000,0000000000000000,7ff8000012345000,7ff8000012345000,7ff8000012345000,3ff0000000000000 7ff8000000000000,fff8000000000000,fff8000000000000 -> 7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000012345000,7ff8000012345000,7ff8000012345000,7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000,7ff8000000000000",
            "fff8000012345000,fff8000012345000,fff8000012345000,0000000000000000,fff8000012345000,fff8000012345000,fff8000012345000,0000000000000000,fff8000012345000,fff8000012345000,fff8000012345000,0000000000000000,fff8000012345000,fff8000012345000,fff8000012345000,3ff0000000000000 4026000000000000,4028000000000000,402a000000000000 -> fff8000000000000,fff8000000000000,fff8000000000000,fff8000012345000,fff8000012345000,fff8000012345000,fff8000000000000,fff8000000000000,fff8000000000000,fff8000000000000,fff8000000000000,fff8000000000000",
        ];
        let bits = |s: &str| -> Vec<f64> {
            s.split(',')
                .map(|h| f64::from_bits(u64::from_str_radix(h, 16).unwrap()))
                .collect()
        };
        for line in CASES {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let (a, c, want) = (bits(parts[0]), bits(parts[1]), bits(parts[3]));
            let mut rows = [0.0f64; 16];
            for r in 0..4 {
                for col in 0..4 {
                    rows[r * 4 + col] = a[col * 4 + r];
                }
            }
            let got = decompose_aff(&Mat::from_rows(4, 4, &rows), &Mat::col(&c));
            for k in 0..12 {
                assert_eq!(
                    got[k].to_bits(),
                    want[k].to_bits(),
                    "parameter {} of {line}",
                    k + 1
                );
            }
        }
    }

    /// NaNs made from numbers are SSE's default NaN (sign set), on every platform.
    #[test]
    fn made_nans_have_the_sign_set() {
        let inf = f64::INFINITY;
        let c = cross(&Mat::col(&[inf, 0.0, 1.0]), &Mat::col(&[inf, 1.0, 0.0]));
        // a3*b1 - a1*b3 = 1*inf - inf*0: inf - NaN(0 * inf): the default NaN.
        assert_eq!(c.get(1).to_bits(), 0xfff8_0000_0000_0000);
        assert_eq!(norm2(&Mat::col(&[inf, -inf, 1.0])), inf);
    }
}
