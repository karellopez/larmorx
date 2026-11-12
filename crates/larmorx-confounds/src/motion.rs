// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause AND MIT
// Translated to Rust from fMRIPrep 25.2.7 and 25.2.6 (fmriprep/interfaces/confounds.py:
// `FSLMotionParams`, `FSLRMSDeviation`; niworkflows 1.15.4 and 1.15.2 carry the same two in
// niworkflows/interfaces/confounds.py), Copyright the NiPreps developers, under the Apache-2.0
// licence; from nitransforms 25.1.0 (nitransforms/io/itk.py `ITKLinearTransform.to_ras`,
// nitransforms/io/fsl.py `FSLLinearTransform.from_ras` and `_fsl_aff_adapt`), Copyright (c)
// 2021 The NiPy developers, and nibabel 5.4.2 (nibabel/affines.py `voxel_sizes`,
// `from_matvec`), Copyright (c) 2009-2019 Matthew Brett and the nibabel developers, both under
// the MIT licence; and from SciPy 1.18.1 (scipy/spatial/transform/_rotation_cy.pyx:
// `from_matrix`, `_from_matrix_orthogonal`, `_normalize4`, `_argmax4`, `as_euler`,
// `_get_angles`; SciPy 1.15.3's _rotation.pyx has the same arithmetic), Copyright (c) 2001-2002
// Enthought, Inc. 2003-2024, SciPy Developers, under the BSD-3-Clause licence (the notices
// below). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals
// (numpy's and OpenBLAS's arithmetic written out).
//
// SciPy's notice: Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met: redistributions
// of source code must retain the above copyright notice, this list of conditions and the
// following disclaimer; redistributions in binary form must reproduce the above copyright
// notice, this list of conditions and the following disclaimer in the documentation and/or
// other materials provided with the distribution; neither the name of the copyright holder nor
// the names of its contributors may be used to endorse or promote products derived from this
// software without specific prior written permission. THIS SOFTWARE IS PROVIDED BY THE
// COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING,
// BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
// PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR
// ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
// NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR
// BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// nitransforms' and nibabel's notice (MIT): Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the
// Software, and to permit persons to whom the Software is furnished to do so, subject to the
// following conditions: The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software. THE SOFTWARE IS PROVIDED "AS IS",
// WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES
// OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
// ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE
// OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
//! fMRIPrep's motion parameters and root mean square deviation from the head-motion transforms
//! (`FSLMotionParams`, `FSLRMSDeviation`), bit for bit as fMRIPrep 25.2.7 and 25.2.6 compute
//! them with nitransforms, SciPy and numpy:
//!
//! - [`itk_to_ras`]: nitransforms' `ITKLinearTransform.to_ras` of one `AffineTransform`
//!   (`LPS · (c_pos · (P · (c_neg · LPS)))`, numpy's `dot`: OpenBLAS's Haswell `dgemm`).
//! - [`FslSpace`], [`fsl_from_ras`]: `FSLLinearTransformArray.from_ras(ras, reference=boldref,
//!   moving=boldref)`, the transform in FSL's scaled-voxel coordinates: `_fsl_aff_adapt` (the
//!   voxel sizes `sqrt(sum(A[:3, :3] ** 2, axis=0))`, the first axis mirrored when
//!   `det(A) > 0`), `pre = A @ inv(swp @ spc)`, `post = swp @ spc @ inv(A)`, the parameters
//!   `inv((post @ ras @ pre).T).T` (numpy's `inv`: OpenBLAS's `dgesv`, [`inv4`]).
//! - [`scipy_from_matrix`], [`scipy_as_euler`]: SciPy's `Rotation.from_matrix(stack)` (the
//!   determinants, the Gramian `m @ m.T` (numpy's `dsyrk`) checked with `isclose(atol=1e-12)`,
//!   the polar factor `U @ Vt` of `numpy.linalg.svd` for the matrices that are not orthogonal,
//!   Shepperd's quaternion with `_normalize4`) and `as_euler(seq)` (Bernardes and Viollet's
//!   method, `_get_angles`).
//! - [`motion_params`]: fMRIPrep 25.2.7's `trans = M @ cog + b - cog`, `rot =
//!   -as_euler(from_matrix(M), "XYZ")`, or fMRIPrep 25.2.6's `trans = b - M.T @ cog + cog`, `rot
//!   = as_euler(from_matrix(M.T), "XYZ")` ([`MotionRelease`]); `M @ c` is numpy's `dgemv` (the
//!   transposed kernel for `M`, the plain one for the strided `M.T`).
//! - [`rms_deviation`]: `FSLRMSDeviation`'s `sqrt(diag(t @ t.T) + trace(M.T @ M) · 80² / 5)` of
//!   the differences `F[i] @ inv(F[i - 1]) - I` (Jenkinson's formula with a sphere of 80 mm),
//!   the first row NaN.
//!
//! **The C library's functions.** SciPy's `as_euler` calls the C library's `atan2` and `hypot`,
//! glibc's on the oracles, which are not correctly rounded on about 0.1 % (`atan2`) and 0.6 %
//! (`hypot`) of uniform pairs (`docs/findings/confounds-*.md`). larmorx takes the correctly
//! rounded ones in both modes (the orchestrator's D4 of 2026-11-11: glibc's own code is LGPL and
//! never translated), an expected divergence that the validation counts and names
//! (`validation/investigations/confounds/check_motion.py`).
//!
//! numpy's float64 products and inverses of 3 × 3 and 4 × 4 matrices follow OpenBLAS's Haswell
//! kernels on every platform ([`larmorx_transform::openblas`]), numpy's 3 × 3 SVDs LAPACK
//! 3.12.0's or 3.12.1's arithmetic by the numpy release ([`LapackRelease`]). The determinants
//! numpy compares with zero are `sign · exp(Σ log|u_ii|)` of OpenBLAS's LU factors, with the
//! correctly rounded `log` and `exp` (only their signs and zeros are used).

use larmorx_core::linalg::{Mat3, Mat4};
use larmorx_core::math;
use larmorx_transform::numpy_linalg::{LapackRelease, svd3};
use larmorx_transform::openblas::matmul::{Array2, numpy_matmul, numpy_syrk, syrk_case};
use larmorx_transform::openblas::{self, inv4, matmul4};

/// x86-64's default NaN (SSE's `0 / 0`): the NaN a computed result takes on every platform.
const DEFAULT_NAN: f64 = f64::from_bits(0xfff8_0000_0000_0000);

/// The errors numpy and SciPy raise on the way.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum MotionError {
    /// numpy's `LinAlgError("Singular matrix")` of `numpy.linalg.inv` (an exactly zero pivot).
    #[error("Singular matrix")]
    Singular,
    /// SciPy's `ValueError` for the first matrix whose determinant is zero or negative (its index
    /// in the stack); the Python layer adds the matrix, as SciPy prints it.
    #[error(
        "Non-positive determinant (left-handed or null coordinate frame) in rotation matrix {0}"
    )]
    NonPositiveDeterminant(usize),
    /// numpy's `LinAlgError("SVD did not converge")` (a matrix that is not finite).
    #[error("SVD did not converge")]
    Svd,
}

/// The release of fMRIPrep's `FSLMotionParams` (fMRIPrep 25.2.7 changed it: the step per case).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MotionRelease {
    /// fMRIPrep 25.2.6 (niworkflows 1.15.2 and 1.15.3): the rotation of `M.T`, `trans = b -
    /// M.T @ cog + cog`.
    V25_2_6,
    /// fMRIPrep 25.2.7 (niworkflows 1.15.4): the rotation of `M` negated, `trans = M @ cog + b
    /// - cog`. The default.
    #[default]
    V25_2_7,
}

fn eye4() -> Mat4 {
    std::array::from_fn(|r| std::array::from_fn(|c| if r == c { 1.0 } else { 0.0 }))
}

fn transpose4(m: &Mat4) -> Mat4 {
    std::array::from_fn(|r| std::array::from_fn(|c| m[c][r]))
}

/// nitransforms' `ITKLinearTransform.to_ras()` of an `AffineTransform` with ITK's 12
/// parameters (the 3 × 3 matrix by rows, then the translation; float32 in the files, exact in
/// float64) and the fixed parameters (the centre `offset`): `LPS.dot(c_pos.dot(matrix.dot(
/// c_neg.dot(LPS))))` with `LPS = diag(-1, -1, 1, 1)` and `c_neg`, `c_pos` nibabel's
/// `from_matvec(eye(3), ∓offset)`, every `dot` OpenBLAS's `dgemm` ([`matmul4`]).
pub fn itk_to_ras(params: &[f64; 12], offset: &[f64; 3]) -> Mat4 {
    let mut lps = eye4();
    lps[0][0] = -1.0;
    lps[1][1] = -1.0;
    let mut c_neg = eye4();
    let mut c_pos = eye4();
    for r in 0..3 {
        // `offset * -1.0`: the float32 offset negated (exact).
        c_neg[r][3] = -offset[r];
        c_pos[r][3] = offset[r];
    }
    let p = params;
    let matrix: Mat4 = [
        [p[0], p[1], p[2], p[9]],
        [p[3], p[4], p[5], p[10]],
        [p[6], p[7], p[8], p[11]],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let inner = matmul4(&c_neg, &lps);
    let inner = matmul4(&matrix, &inner);
    let inner = matmul4(&c_pos, &inner);
    matmul4(&lps, &inner)
}

/// The index of the first matrix numpy's `inv` finds singular (nitransforms' `Affine` inverts
/// every matrix it holds), or `None`.
pub fn first_singular(mats: &[Mat4]) -> Option<usize> {
    mats.iter().position(|m| inv4(m).is_none())
}

/// numpy's `det(a)` of a float64 3 × 3 matrix for a comparison with zero: OpenBLAS's LU factors
/// (`getf2_k`), then `sign · exp(Σ log|u_ii|)` (`det_from_slogdet`); 0 for an exactly zero pivot.
/// The `log` and `exp` are the correctly rounded ones (numpy's are glibc's): the value's last
/// bits may differ from numpy's, its sign, zero and NaN do not.
pub fn det3(a: &Mat3) -> f64 {
    match openblas::lu_sign_and_diagonal3(a) {
        None => 0.0,
        Some((sign, diag)) => det_from_diagonal(sign, &diag),
    }
}

/// [`det3`] of a 4 × 4 matrix (nitransforms' `np.linalg.det(affine) > 0`).
pub fn det4(a: &Mat4) -> f64 {
    match openblas::lu_sign_and_diagonal4(a) {
        None => 0.0,
        Some((sign, diag)) => det_from_diagonal(sign, &diag),
    }
}

fn det_from_diagonal(mut sign: f64, diag: &[f64]) -> f64 {
    // slogdet_from_factored_diagonal: the sign of each pivot, then Σ log|u_ii| in order.
    let mut logdet = 0.0f64;
    for &v in diag {
        let mut a = v;
        if a < 0.0 {
            sign = -sign;
            a = -a;
        }
        logdet += math::log(a);
    }
    sign * math::exp(logdet)
}

/// nitransforms' `_fsl_aff_adapt(image)` and the two matrices `FSLLinearTransform.from_ras`
/// builds from the reference (= moving) image: `pre = A @ inv(swp @ spc)` and `post = swp @
/// spc @ inv(A)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FslSpace {
    /// `reference.affine @ inv(refswp @ refspc)`.
    pub pre: Mat4,
    /// `movswp @ movspc @ inv(moving.affine)`.
    pub post: Mat4,
}

impl FslSpace {
    /// The space of an image with affine `affine` and `shape0` voxels along its first axis:
    /// nibabel's `voxel_sizes` (each column's squares summed from the first row, then the square
    /// root) and `[1]` on the diagonal of `spc`; `swp` the identity, or, when numpy's
    /// `det(affine) > 0`, the first axis mirrored (`swp[0, 0] = -1`, `swp[0, 3] = (shape0 - 1)
    /// · zooms[0]`). [`MotionError::Singular`] where numpy's `inv` raises.
    pub fn new(affine: &Mat4, shape0: i64) -> Result<FslSpace, MotionError> {
        let zooms: [f64; 3] = std::array::from_fn(|c| {
            let s = (0.0 + affine[0][c] * affine[0][c]) + affine[1][c] * affine[1][c];
            (s + affine[2][c] * affine[2][c]).sqrt()
        });
        let mut swp = eye4();
        if det4(affine) > 0.0 {
            swp[0][0] = -1.0;
            swp[0][3] = (shape0 - 1) as f64 * zooms[0];
        }
        let mut spc = eye4();
        for (i, z) in zooms.iter().enumerate() {
            spc[i][i] = *z;
        }
        let swp_spc = matmul4(&swp, &spc);
        let pre = matmul4(affine, &inv4(&swp_spc).ok_or(MotionError::Singular)?);
        let post = matmul4(&swp_spc, &inv4(affine).ok_or(MotionError::Singular)?);
        Ok(FslSpace { pre, post })
    }
}

/// `FSLLinearTransform.from_ras(ras, reference, moving)`'s parameters: `mat = inv(swapaxes(post
/// @ ras @ pre, 0, 1))`, stored as `mat.T`.
pub fn fsl_from_ras(ras: &Mat4, space: &FslSpace) -> Result<Mat4, MotionError> {
    let x = matmul4(&matmul4(&space.post, ras), &space.pre);
    let mat = inv4(&transpose4(&x)).ok_or(MotionError::Singular)?;
    Ok(transpose4(&mat))
}

/// numpy's `isclose(g, eye(3), atol=1e-12)` for every element (the default `rtol = 1e-5`):
/// `abs(x - y) <= atol + rtol * abs(y)`, or `x == y`.
fn gramian_is_identity(g: &[f64]) -> bool {
    (0..9).all(|e| {
        let y = if e % 4 == 0 { 1.0 } else { 0.0 };
        let x = g[e];
        ((x - y).abs() <= 1e-12 + 1e-05 * y.abs()) || x == y
    })
}

/// SciPy's `_argmax4`: the first index of the largest value (`>`: a NaN never wins).
fn argmax4(a: &[f64; 4]) -> usize {
    let mut imax = 0;
    let mut vmax = a[0];
    for (i, &v) in a.iter().enumerate().skip(1) {
        if v > vmax {
            imax = i;
            vmax = v;
        }
    }
    imax
}

/// SciPy's `_normalize4` (in place; a zero norm leaves the quaternion as it is).
fn normalize4(q: &mut [f64; 4]) {
    let norm = ((q[0] * q[0] + q[1] * q[1]) + q[2] * q[2] + q[3] * q[3]).sqrt();
    if norm == 0.0 {
        return;
    }
    for v in q.iter_mut() {
        *v /= norm;
    }
}

/// SciPy's `_from_matrix_orthogonal` of one matrix: Shepperd's choice of the largest of the
/// diagonal and the trace, the quaternion `(x, y, z, w)` from it, normalized.
pub fn quat_from_orthogonal(m: &Mat3) -> [f64; 4] {
    let decision = [m[0][0], m[1][1], m[2][2], (m[0][0] + m[1][1]) + m[2][2]];
    let choice = argmax4(&decision);
    let mut q = [0.0f64; 4];
    if choice != 3 {
        let i = choice;
        let j = (i + 1) % 3;
        let k = (j + 1) % 3;
        q[i] = (1.0 - decision[3]) + 2.0 * m[i][i];
        q[j] = m[j][i] + m[i][j];
        q[k] = m[k][i] + m[i][k];
        q[3] = m[k][j] - m[j][k];
    } else {
        q[0] = m[2][1] - m[1][2];
        q[1] = m[0][2] - m[2][0];
        q[2] = m[1][0] - m[0][1];
        q[3] = 1.0 + decision[3];
    }
    normalize4(&mut q);
    q
}

/// The layout of the float64 copy SciPy's `from_matrix` makes (`np.array(matrix, dtype=float)`
/// keeps the input's order): the Gramian's `dsyrk` call depends on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliceOrder {
    /// Each 3 × 3 matrix in C order (fMRIPrep 25.2.7's `M`).
    C,
    /// Each 3 × 3 matrix in Fortran order (fMRIPrep 25.2.6's `M.transpose(0, 2, 1)`).
    F,
}

/// `numpy.matmul(m, m.T)` of one 3 × 3 matrix stored in `order`: numpy's `dsyrk` case.
fn gramian(m: &Mat3, order: SliceOrder) -> Vec<f64> {
    let data: Vec<f64> = match order {
        SliceOrder::C => m.iter().flatten().copied().collect(),
        SliceOrder::F => (0..9).map(|e| m[e % 3][e / 3]).collect(),
    };
    let (s0, s1) = match order {
        SliceOrder::C => (3, 1),
        SliceOrder::F => (1, 3),
    };
    let a = Array2 {
        data: &data,
        off: 0,
        rows: 3,
        cols: 3,
        s0,
        s1,
    };
    let b = Array2 {
        data: &data,
        off: 0,
        rows: 3,
        cols: 3,
        s0: s1,
        s1: s0,
    };
    match syrk_case(a, b, true) {
        Some(trans) => numpy_syrk(a, trans),
        None => numpy_matmul(a, b),
    }
}

/// SciPy's `Rotation.from_matrix(mats)` (SciPy 1.18.1 and 1.15.x, the same arithmetic): the
/// quaternions `(x, y, z, w)`, or SciPy's errors in its order: the first matrix whose numpy
/// determinant is at most 0 ([`MotionError::NonPositiveDeterminant`]), then numpy's SVD of the
/// matrices that are not orthogonal ([`MotionError::Svd`] when one is not finite).
pub fn scipy_from_matrix(
    mats: &[Mat3],
    order: SliceOrder,
    release: LapackRelease,
) -> Result<Vec<[f64; 4]>, MotionError> {
    if let Some(i) = mats.iter().position(|m| det3(m) <= 0.0) {
        return Err(MotionError::NonPositiveDeterminant(i));
    }
    let mut work: Vec<Mat3> = mats.to_vec();
    let mut failed = false;
    for m in work.iter_mut() {
        if gramian_is_identity(&gramian(m, order)) {
            continue;
        }
        match svd3(m, release) {
            Some((u, _, vt)) => *m = larmorx_transform::numpy_linalg::matmul3(&u, &vt),
            None => failed = true,
        }
    }
    if failed {
        return Err(MotionError::Svd);
    }
    Ok(work.iter().map(quat_from_orthogonal).collect())
}

/// An Euler sequence as SciPy reads it: three axes (`0` = x, `1` = y, `2` = z) as written, and
/// whether they are extrinsic (lower case) or intrinsic (upper case).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EulerSeq {
    /// The axes in the order written.
    pub axes: [usize; 3],
    /// Lower case (`"xyz"`).
    pub extrinsic: bool,
}

impl EulerSeq {
    /// SciPy's intrinsic `"XYZ"` (fMRIPrep's).
    pub const XYZ: EulerSeq = EulerSeq {
        axes: [0, 1, 2],
        extrinsic: false,
    };
}

/// SciPy's `as_euler(seq)` of each quaternion (`_compute_euler_from_quat` and `_get_angles`,
/// SciPy 1.18.1; the same arithmetic in 1.15.x), with the correctly rounded `atan2` and `hypot`
/// (module documentation): the angles, and how many rotations were in gimbal lock (SciPy's
/// warning "Gimbal lock detected. ...").
pub fn scipy_as_euler(quats: &[[f64; 4]], seq: EulerSeq) -> (Vec<[f64; 3]>, usize) {
    use std::f64::consts::PI;
    let extrinsic = seq.extrinsic;
    let axes = if extrinsic {
        seq.axes
    } else {
        [seq.axes[2], seq.axes[1], seq.axes[0]]
    };
    let (i, j, mut k) = (axes[0] as i64, axes[1] as i64, axes[2] as i64);
    let symmetric = i == k;
    if symmetric {
        k = 3 - i - j;
    }
    // (i - j)(j - k)(k - i) // 2: ±1 (Python's floor division; the values are exact).
    let sign = ((i - j) * (j - k) * (k - i)).div_euclid(2) as f64;
    let (iu, ju, ku) = (i as usize, j as usize, k as usize);
    let (first, third) = if extrinsic { (0, 2) } else { (2, 0) };
    let lamb = PI / 2.0;
    let mut gimbal = 0;
    let out = quats
        .iter()
        .map(|q| {
            let (a, b, c, d) = if symmetric {
                (q[3], q[iu], q[ju], q[ku] * sign)
            } else {
                (
                    q[3] - q[ju],
                    q[iu] + q[ku] * sign,
                    q[ju] + q[3],
                    q[ku] * sign - q[iu],
                )
            };
            let mut angles = [0.0f64; 3];
            angles[1] = 2.0 * math::atan2(math::hypot(c, d), math::hypot(a, b));
            let case = if angles[1].abs() <= 1e-7 {
                1
            } else if (angles[1] - PI).abs() <= 1e-7 {
                2
            } else {
                0
            };
            let half_sum = math::atan2(b, a);
            let half_diff = math::atan2(d, c);
            if case == 0 {
                angles[first] = half_sum - half_diff;
                angles[third] = half_sum + half_diff;
            } else {
                angles[2] = 0.0;
                angles[0] = if case == 1 {
                    2.0 * half_sum
                } else {
                    2.0 * half_diff * if extrinsic { -1.0 } else { 1.0 }
                };
            }
            if !symmetric {
                angles[third] *= sign;
                angles[1] -= lamb;
            }
            for v in angles.iter_mut() {
                if *v < -PI {
                    *v += 2.0 * PI;
                } else if *v > PI {
                    *v -= 2.0 * PI;
                }
            }
            if case != 0 {
                gimbal += 1;
            }
            angles
        })
        .collect();
    (out, gimbal)
}

/// numpy's `m @ v` for a 3 × 3 matrix stored with strides `(s0, s1)` at `off` in `data` and a
/// contiguous vector of 3: numpy's `matrix_vector` case (`dgemv`, [`numpy_matmul`]).
fn matvec3(data: &[f64], off: usize, s0: usize, s1: usize, v: &[f64; 3]) -> [f64; 3] {
    let a = Array2 {
        data,
        off,
        rows: 3,
        cols: 3,
        s0,
        s1,
    };
    let b = Array2 {
        data: v,
        off: 0,
        rows: 3,
        cols: 1,
        s0: 1,
        s1: 1,
    };
    let out = numpy_matmul(a, b);
    [out[0], out[1], out[2]]
}

/// fMRIPrep's centre of gravity: `np.diag(zooms) @ center_of_mass` (the float32 zooms cast to
/// float64; numpy's `dgemv`).
pub fn center_of_gravity(zooms: &[f64; 3], com: &[f64; 3]) -> [f64; 3] {
    let d: Vec<f64> = (0..9)
        .map(|e| if e % 4 == 0 { zooms[e / 4] } else { 0.0 })
        .collect();
    matvec3(&d, 0, 3, 1, com)
}

/// `FSLMotionParams`' table: one row `(trans_x, trans_y, trans_z, rot_x, rot_y, rot_z)` per
/// FSL matrix, and how many rotations were in gimbal lock.
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    /// The rows.
    pub params: Vec<[f64; 6]>,
    /// SciPy's gimbal-lock count (its warning).
    pub gimbal: usize,
}

fn canonical_nan(v: f64) -> f64 {
    if v.is_nan() { DEFAULT_NAN } else { v }
}

/// `FSLMotionParams`' arithmetic on the FSL matrices `fsl` (from [`fsl_from_ras`]) and the centre
/// of gravity `cog` ([`center_of_gravity`]), in fMRIPrep `release`'s form ([`MotionRelease`]),
/// with numpy's LAPACK `lapack` for SciPy's SVDs. A NaN result is x86-64's default NaN.
pub fn motion_params(
    fsl: &[Mat4],
    cog: &[f64; 3],
    release: MotionRelease,
    lapack: LapackRelease,
) -> Result<Motion, MotionError> {
    let flat: Vec<f64> = fsl.iter().flatten().flatten().copied().collect();
    let (mats, order): (Vec<Mat3>, SliceOrder) = match release {
        MotionRelease::V25_2_7 => (
            fsl.iter()
                .map(|f| std::array::from_fn(|r| std::array::from_fn(|c| f[r][c])))
                .collect(),
            SliceOrder::C,
        ),
        MotionRelease::V25_2_6 => (
            fsl.iter()
                .map(|f| std::array::from_fn(|r| std::array::from_fn(|c| f[c][r])))
                .collect(),
            SliceOrder::F,
        ),
    };
    let quats = scipy_from_matrix(&mats, order, lapack)?;
    let (angles, gimbal) = scipy_as_euler(&quats, EulerSeq::XYZ);
    let params = fsl
        .iter()
        .enumerate()
        .map(|(n, f)| {
            let (trans, rot) = match release {
                MotionRelease::V25_2_7 => {
                    // M @ cog: M = fsl[n, :3, :3] (row stride 4): dgemv_t.
                    let g = matvec3(&flat, 16 * n, 4, 1, cog);
                    let t: [f64; 3] = std::array::from_fn(|r| (g[r] + f[r][3]) - cog[r]);
                    (t, angles[n].map(|a| -a))
                }
                MotionRelease::V25_2_6 => {
                    // mats @ cog: mats = M.T (strides (1, 4)): dgemv_n.
                    let g = matvec3(&flat, 16 * n, 1, 4, cog);
                    let t: [f64; 3] = std::array::from_fn(|r| (f[r][3] - g[r]) + cog[r]);
                    (t, angles[n])
                }
            };
            [trans[0], trans[1], trans[2], rot[0], rot[1], rot[2]].map(canonical_nan)
        })
        .collect();
    Ok(Motion { params, gimbal })
}

/// `diag(t @ t.T)` of a `rows × 3` C-ordered table `t`: numpy's `dsyrk` for two or more rows,
/// whose diagonal is OpenBLAS's Haswell kernel's fused chain `fma(t2, t2, fma(t1, t1, t0 · t0))
/// + 0`, and numpy's `0 + ddot` (products added one at a time) for one row.
fn diag_t_tt(t: &[[f64; 3]]) -> Vec<f64> {
    if t.len() == 1 {
        let v = t[0];
        return vec![0.0 + ((0.0 + v[0] * v[0]) + v[1] * v[1] + v[2] * v[2])];
    }
    t.iter()
        .map(|v| v[2].mul_add(v[2], v[1].mul_add(v[1], v[0] * v[0])) + 0.0)
        .collect()
}

/// `FSLRMSDeviation`'s values: NaN for the first volume, then for each later one
/// `sqrt(diag(t @ t.T) + trace(M.T @ M) * 80.0**2 / 5)` with `D = F[i] @ inv(F[i - 1]) - eye(4)`
/// (numpy's stacked `inv` first: [`MotionError::Singular`]), `M = D[:3, :3]`, `t = D[:3, 3] +
/// M @ center` (`dgemv_t`); `M.T @ M` is numpy's `dsyrk` and the trace its diagonal added in
/// order. The first value is numpy's `np.nan`; a NaN computed later x86-64's default NaN.
pub fn rms_deviation(fsl: &[Mat4], center: &[f64; 3]) -> Result<Vec<f64>, MotionError> {
    let n = fsl.len();
    if n == 0 {
        return Ok(vec![f64::NAN]);
    }
    let inverses: Vec<Mat4> = fsl[..n - 1]
        .iter()
        .map(|f| inv4(f).ok_or(MotionError::Singular))
        .collect::<Result<_, _>>()?;
    let mut ts = Vec::with_capacity(n - 1);
    let mut traces = Vec::with_capacity(n - 1);
    for i in 1..n {
        let mut d = matmul4(&fsl[i], &inverses[i - 1]);
        for (r, row) in d.iter_mut().enumerate() {
            row[r] -= 1.0;
        }
        let flat: Vec<f64> = d.iter().flatten().copied().collect();
        let g = matvec3(&flat, 0, 4, 1, center);
        ts.push(std::array::from_fn::<f64, 3, _>(|r| d[r][3] + g[r]));
        // M.T @ M: a = M.T (strides (1, 4)), b = M (strides (4, 1)), one buffer: dsyrk.
        let a = Array2 {
            data: &flat,
            off: 0,
            rows: 3,
            cols: 3,
            s0: 1,
            s1: 4,
        };
        let b = Array2 {
            data: &flat,
            off: 0,
            rows: 3,
            cols: 3,
            s0: 4,
            s1: 1,
        };
        let mtm = match syrk_case(a, b, true) {
            Some(trans) => numpy_syrk(a, trans),
            None => numpy_matmul(a, b),
        };
        // np.trace(axis1=1, axis2=2): the diagonal summed pairwise (three values: in order).
        traces.push(0.0 + ((-0.0 + mtm[0]) + mtm[4] + mtm[8]));
    }
    let tt = diag_t_tt(&ts);
    let mut out = Vec::with_capacity(n);
    out.push(f64::NAN);
    for (a, tr) in tt.iter().zip(&traces) {
        out.push(canonical_nan((a + tr * 6400.0 / 5.0).sqrt()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng(seed: u64) -> impl FnMut() -> f64 {
        let mut s = seed;
        move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        }
    }

    #[test]
    fn identity_has_zero_angles_and_no_motion() {
        let fsl = vec![eye4(); 3];
        let m = motion_params(
            &fsl,
            &[10.0, 20.0, 30.0],
            MotionRelease::V25_2_7,
            LapackRelease::V3_12_1,
        )
        .unwrap();
        for row in &m.params {
            assert_eq!(row[..3], [0.0, 0.0, 0.0]);
            // -as_euler of the identity: -0.0 in every angle.
            for a in &row[3..] {
                assert_eq!(*a, 0.0);
            }
        }
        let r = rms_deviation(&fsl, &[1.0, 2.0, 3.0]).unwrap();
        assert!(r[0].is_nan());
        assert_eq!(&r[1..], &[0.0, 0.0]);
    }

    #[test]
    fn syrk_diagonal_is_the_fused_chain() {
        let mut next = rng(7);
        let t: Vec<[f64; 3]> = (0..40).map(|_| [next(), next(), next()]).collect();
        let flat: Vec<f64> = t.iter().flatten().copied().collect();
        let a = Array2 {
            data: &flat,
            off: 0,
            rows: 40,
            cols: 3,
            s0: 3,
            s1: 1,
        };
        let b = Array2 {
            data: &flat,
            off: 0,
            rows: 3,
            cols: 40,
            s0: 1,
            s1: 3,
        };
        let full = numpy_syrk(a, syrk_case(a, b, true).unwrap());
        let diag = diag_t_tt(&t);
        for i in 0..40 {
            assert_eq!(full[i * 40 + i].to_bits(), diag[i].to_bits());
        }
    }

    #[test]
    fn a_rotation_comes_back() {
        // A rotation about z by 0.3 rad: as_euler("XYZ") gives (0, 0, 0.3) (within rounding).
        let (s, c) = 0.3f64.sin_cos();
        let m: Mat3 = [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]];
        let q = scipy_from_matrix(&[m], SliceOrder::C, LapackRelease::V3_12_1).unwrap();
        let (a, g) = scipy_as_euler(&q, EulerSeq::XYZ);
        assert_eq!(g, 0);
        assert!(a[0][0].abs() < 1e-15 && a[0][1].abs() < 1e-15);
        assert!((a[0][2] - 0.3).abs() < 1e-15);
    }

    #[test]
    fn left_handed_and_singular_matrices_are_refused() {
        let m: Mat3 = [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let e = scipy_from_matrix(&[eye3(), m], SliceOrder::C, LapackRelease::V3_12_1);
        assert_eq!(e, Err(MotionError::NonPositiveDeterminant(1)));
        let z = [[0.0; 4]; 4];
        assert_eq!(rms_deviation(&[z, eye4()], &[0.0; 3]), Err(MotionError::Singular));
    }

    fn eye3() -> Mat3 {
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    }
}
