// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause AND MIT
// Translated to Rust from nipy 0.6.1 (github.com/nipy/nipy, tag 0.6.1):
// nipy/algorithms/registration/affine.py (`threshold`, `rotation_vec2mat`, `to_matrix44` and the
// constants `MAX_ANGLE`, `SMALL_ANGLE`, `MAX_DIST`, `LOG_MAX_DIST`) and
// nipy/algorithms/registration/scripting.py (`aff2euler`, `aff2rot_zooms`), Copyright (c)
// 2006-2024, NIPY Developers, under the BSD-3-Clause licence; and from nibabel 5.4.2
// (nibabel/eulerangles.py, `mat2euler`; the same function in nibabel 5.3.2), Copyright (c)
// 2009-2019 Matthew Brett and the nibabel developers, under the MIT licence (the notices below).
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals (one float64
// row at a time, numpy's arithmetic written out).
//
// nipy's notice: Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met: redistributions
// of source code must retain the above copyright notice, this list of conditions and the
// following disclaimer; redistributions in binary form must reproduce the above copyright
// notice, this list of conditions and the following disclaimer in the documentation and/or
// other materials provided with the distribution; neither the name of the NIPY Developers nor
// the names of any contributors may be used to endorse or promote products derived from this
// software without specific prior written permission. THIS SOFTWARE IS PROVIDED BY THE
// COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING,
// BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
// PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
// LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR
// BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// nibabel's notice: Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal in the Software
// without restriction, including without limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to
// whom the Software is furnished to do so, subject to the following conditions: The above
// copyright notice and this permission notice shall be included in all copies or substantial
// portions of the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR
// A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//! nipype 1.12.0's `normalize_mc_params(params, "NIPY")` (`nipype/utils/misc.py`), the motion
//! parameters `FramewiseDisplacement(parameter_source="NIPY")` takes: a row of nipy's affine
//! parameters (translations in mm, a rotation vector in radians, then optionally an isotropic
//! scale, or three log-scales and a second rotation vector) as SPM's six (`x`, `y`, `z` in mm,
//! then the Euler angles in radians): `matrix = to_matrix44(params)`, its translations
//! `matrix[:3, 3]` and nibabel's Euler angles of its rotation (`params[-1:2:-1] =
//! aff2euler(matrix)`, so the last three are `x`, `y`, `z`), bit for bit as nipy 0.6.1 computes
//! them with numpy and nibabel:
//!
//! - [`to_matrix44`]: a row of 6 values is a rigid transform (`rotation_vec2mat(t[3:6])`), of 7
//!   that times the scale `t[6]`, of 12 or more `R · diag(exp(threshold(t[6:9], LOG_MAX_DIST))) ·
//!   Q` with `Q = rotation_vec2mat(t[9:12])` (numpy's `dot`: OpenBLAS's Haswell `dgemm`); the
//!   translations are `threshold(t[0:3], MAX_DIST)`. Other lengths stop with numpy's
//!   `IndexError` where nipy indexes a rotation vector past its end ([`NipyError::Index`]),
//!   except that a rotation vector of one or two values whose norm is above `MAX_ANGLE` is the
//!   identity before any index: so 10 or 11 values with such a second vector are a valid row.
//! - [`rotation_vec2mat`]: `theta = sqrt(sum(r ** 2))` (numpy's pairwise sum), the identity above
//!   [`MAX_ANGLE`], Rodrigues' formula `I + sin θ · Sn + (1 - cos θ) · Sn · Sn` (`Sn` the skew
//!   matrix of `r / θ`) above [`SMALL_ANGLE`], else its Taylor form `I + (1 - θ²/6) · Sr + (1/2 -
//!   θ²/24) · Sr · Sr`, each sum and product in numpy's order.
//! - [`aff2rot_zooms`]: the zooms `sqrt(sum(RZS * RZS, axis=0))` (an outer-axis sum: in index
//!   order), `RS = RZS / zooms`, its first column negated when `numpy.linalg.det(RS) < 0`
//!   (OpenBLAS's `dgetrf`), the polar factor `P · Qs` of `numpy.linalg.svd(RS)` (`dgesdd` with
//!   `JOBZ='A'`; numpy's `LinAlgError("SVD did not converge")` for a matrix that is not finite:
//!   [`NipyError::Svd`]).
//! - [`mat2euler`]: nibabel's `cy = sqrt(r33 · r33 + r23 · r23)` and three `math.atan2` (`x = 0`
//!   when `cy` is at most `4 · eps`), with CPython's special cases ([`py_atan2`]).
//!
//! **The C library's functions.** numpy's `sin` and `cos` of a float64 are glibc's (as `math.sin`
//! and `math.cos`, AVX-512 on or off), numpy's `exp` of a float64 array glibc's on x86-64 CPUs
//! without AVX-512 (on CPUs with AVX512_SKX numpy takes Intel's SVML kernel, NP-04: larmorx
//! follows the path without, as [`larmorx_core::numpy::umath`]), CPython's `math.atan2` glibc's.
//! By default larmorx takes the correctly rounded `sin`, `cos` and `exp` ([`Libm`]: CORE-MATH's);
//! with `reproduce` glibc 2.35's (GLIBC-07). `atan2` is correctly rounded in both modes (the
//! orchestrator's D4 of 2026-11-11, as for SciPy's `as_euler`: glibc's is not correctly rounded
//! on about 0.1 % of uniform pairs, an expected divergence under `reproduce`, counted by the
//! check). [`LOG_MAX_DIST`] is numpy's `log(1e10)`, the correctly rounded value, which every
//! library here gives (a test checks it).
//!
//! The only float operations are IEEE-754's in round to nearest, in nipy's order; the numbers
//! never reach a NaN-producing operation on a path that returns (a rotation vector with a NaN
//! makes a matrix of NaNs, which numpy's SVD refuses), so no NaN sign is left to the platform.

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::math::{Libm, atan2};
use larmorx_core::numpy::reduce::{add_reduce, add_reduce_axis};
use larmorx_transform::numpy_linalg::{LapackRelease, det3_is_negative, matmul3, svd3};

/// A 3 × 3 matrix, row-major (numpy's C order).
pub type M3 = [[f64; 3]; 3];
/// A 4 × 4 matrix, row-major.
pub type M4 = [[f64; 4]; 4];

/// nipy's `MAX_ANGLE = 1e10 * 2 * np.pi`: a rotation vector longer than this is the identity.
pub const MAX_ANGLE: f64 = 1e10 * 2.0 * std::f64::consts::PI;
/// nipy's `SMALL_ANGLE`: at or below it `rotation_vec2mat` takes the Taylor form.
pub const SMALL_ANGLE: f64 = 1e-30;
/// nipy's `MAX_DIST`: the translations' bound.
pub const MAX_DIST: f64 = 1e10;
/// nipy's `LOG_MAX_DIST = np.log(MAX_DIST)` (`0x1.7069e2aa2aa5bp+4`, correctly rounded): the
/// log-scales' bound.
pub const LOG_MAX_DIST: f64 = f64::from_bits(0x4037_069e_2aa2_aa5b);
/// nibabel's `cy_thresh` for float64: `np.finfo(np.float64).eps * 4`.
const CY_THRESH: f64 = f64::EPSILON * 4.0;

const EYE3: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Where nipy's arithmetic stops, with numpy's exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NipyError {
    /// numpy's `IndexError` where `rotation_vec2mat` indexes a rotation vector of this many
    /// values at 2 (a row of fewer than 6 values, or of 8 to 11).
    #[error("index 2 is out of bounds for axis 0 with size {0}")]
    Index(usize),
    /// numpy's `LinAlgError` of `numpy.linalg.svd`: the rotation and scale part is not finite (a
    /// NaN in a rotation vector or a scale, an infinite or zero scale).
    #[error("SVD did not converge")]
    Svd,
}

/// nipy's `threshold(x, th) = np.maximum(np.minimum(x, th), -th)` of one value (numpy's
/// `minimum` and `maximum` keep a NaN).
#[inline]
#[must_use]
pub fn threshold(x: f64, th: f64) -> f64 {
    let m = if x <= th || x.is_nan() { x } else { th };
    if m >= -th || m.is_nan() { m } else { -th }
}

/// The skew-symmetric matrix nipy builds from `v` (`[[0, -v2, v1], [v2, 0, -v0], [-v1, v0, 0]]`,
/// its zeros `+0.0`).
fn skew(v: [f64; 3]) -> M3 {
    [[0.0, -v[2], v[1]], [v[2], 0.0, -v[0]], [-v[1], v[0], 0.0]]
}

/// `I + a · S + b · D`, as numpy evaluates `np.eye(3) + a * S + b * D` (left to right).
fn eye_plus(a: f64, s: &M3, b: f64, d: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (EYE3[i][j] + a * s[i][j]) + b * d[i][j]))
}

/// nipy's `rotation_vec2mat(r)`: the rotation matrix of the rotation vector `r` (its first three
/// values; fewer than three stop with [`NipyError::Index`] unless the norm is above
/// [`MAX_ANGLE`]).
pub fn rotation_vec2mat(r: &[f64], libm: &Libm) -> Result<M3, NipyError> {
    let squares: Vec<f64> = r.iter().map(|&v| v * v).collect();
    let theta = add_reduce(&squares).sqrt();
    if theta > MAX_ANGLE {
        return Ok(EYE3);
    }
    if r.len() < 3 {
        return Err(NipyError::Index(r.len()));
    }
    if theta > SMALL_ANGLE {
        let n = [r[0] / theta, r[1] / theta, r[2] / theta];
        let sn = skew(n);
        let s = libm.sin(theta);
        let c = libm.cos(theta);
        let d = matmul3(&sn, &sn);
        Ok(eye_plus(s, &sn, 1.0 - c, &d))
    } else {
        let sr = skew([r[0], r[1], r[2]]);
        let theta2 = theta * theta;
        let d = matmul3(&sr, &sr);
        Ok(eye_plus(1.0 - theta2 / 6.0, &sr, 0.5 - theta2 / 24.0, &d))
    }
}

/// nipy's `to_matrix44(t)`: the 4 × 4 affine of a row of parameters (module documentation).
pub fn to_matrix44(t: &[f64], libm: &Libm) -> Result<M4, NipyError> {
    let k = t.len();
    let part = |a: usize, b: usize| &t[a.min(k)..b.min(k)];
    let r = rotation_vec2mat(part(3, 6), libm)?;
    let m: M3 = if k == 6 {
        r
    } else if k == 7 {
        let s = t[6];
        std::array::from_fn(|i| std::array::from_fn(|j| s * r[i][j]))
    } else {
        // np.diag(np.exp(threshold(t[6:9], LOG_MAX_DIST))), then the second rotation; a row of
        // fewer than 9 values has no t[9:12], so that rotation stops first (Index(0)).
        let e: Vec<f64> = part(6, 9)
            .iter()
            .map(|&v| libm.exp(threshold(v, LOG_MAX_DIST)))
            .collect();
        let q = rotation_vec2mat(part(9, 12), libm)?;
        debug_assert_eq!(e.len(), 3);
        let s: M3 = [[e[0], 0.0, 0.0], [0.0, e[1], 0.0], [0.0, 0.0, e[2]]];
        matmul3(&r, &matmul3(&s, &q))
    };
    let mut out = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    for i in 0..3 {
        out[i][..3].copy_from_slice(&m[i]);
        out[i][3] = threshold(t[i], MAX_DIST);
    }
    Ok(out)
}

/// nipy's `aff2rot_zooms(affine)`: the rotation (the polar factor of the normalized
/// rotation-and-scale part) and the zooms (the first negative where the part's determinant is).
pub fn aff2rot_zooms(a: &M4, release: LapackRelease) -> Result<(M3, [f64; 3]), NipyError> {
    let rzs: M3 = std::array::from_fn(|i| std::array::from_fn(|j| a[i][j]));
    let squares: Vec<f64> = rzs.iter().flatten().map(|&v| v * v).collect();
    let sums = add_reduce_axis(&squares, [1, 3, 3]);
    let mut zooms = [sums[0].sqrt(), sums[1].sqrt(), sums[2].sqrt()];
    let mut rs: M3 = std::array::from_fn(|i| std::array::from_fn(|j| rzs[i][j] / zooms[j]));
    if det3_is_negative(&rs) {
        zooms[0] = -zooms[0];
        for row in &mut rs {
            row[0] = -row[0];
        }
    }
    let (p, _s, qs) = svd3(&rs, release).ok_or(NipyError::Svd)?;
    Ok((matmul3(&p, &qs), zooms))
}

/// CPython 3.12's `math.atan2(y, x)` (`m_atan2`): a NaN gives `nan` (`0x7ff8000000000000`), the
/// infinite and zero cases their exact values, every other pair the C library's `atan2`, here
/// correctly rounded ([`atan2`]: D4).
#[must_use]
pub fn py_atan2(y: f64, x: f64) -> f64 {
    use std::f64::consts::PI;
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if y.is_infinite() {
        if x.is_infinite() {
            return if 1f64.copysign(x) == 1.0 {
                (0.25 * PI).copysign(y)
            } else {
                (0.75 * PI).copysign(y)
            };
        }
        return (0.5 * PI).copysign(y);
    }
    if x.is_infinite() || y == 0.0 {
        return if 1f64.copysign(x) == 1.0 {
            0f64.copysign(y)
        } else {
            PI.copysign(y)
        };
    }
    atan2(y, x)
}

/// nibabel's `mat2euler(M)` of a float64 matrix: `(z, y, x)`.
#[must_use]
pub fn mat2euler(m: &M3) -> (f64, f64, f64) {
    let [[r11, r12, r13], [r21, r22, r23], [_, _, r33]] = *m;
    let cy = (r33 * r33 + r23 * r23).sqrt();
    if cy > CY_THRESH {
        (py_atan2(-r12, r11), py_atan2(r13, cy), py_atan2(-r23, r33))
    } else {
        (py_atan2(r21, r22), py_atan2(r13, cy), 0.0)
    }
}

/// nipype's `normalize_mc_params(row, "NIPY")`: SPM's six parameters `[tx, ty, tz, x, y, z]` of
/// one row (module documentation), with `libm`'s `sin`, `cos` and `exp` and numpy `release`'s
/// LAPACK.
pub fn normalize(row: &[f64], release: LapackRelease, libm: &Libm) -> Result<[f64; 6], NipyError> {
    let t = to_matrix44(row, libm)?;
    let (r, _zooms) = aff2rot_zooms(&t, release)?;
    let (z, y, x) = mat2euler(&r);
    Ok([t[0][3], t[1][3], t[2][3], x, y, z])
}

/// The first row where nipy's arithmetic stops (nipype's `numpy.apply_along_axis` goes row by
/// row), with numpy's exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("row {row}: {error}")]
pub struct RowError {
    /// The row (0-based).
    pub row: usize,
    /// numpy's exception.
    pub error: NipyError,
}

/// [`normalize_rows`]' result.
#[derive(Debug, Clone, PartialEq)]
pub struct NipyRows {
    /// `n × 6`, C order: each row's [`normalize`] in the call's mode.
    pub params: Vec<f64>,
    /// The same in the other mode, where some row's parameters differ in a bit (the other mode's
    /// `sin`, `cos` or `exp` gave another value on an argument and that reached the result);
    /// `None` when every row is the same in both modes. A row the other mode could not compute
    /// (unreachable: both modes stop on the same rows) is NaN there.
    pub other: Option<Vec<f64>>,
}

/// nipype's `numpy.apply_along_axis(normalize_mc_params, 1, rows, source="NIPY")` of a C-ordered
/// `n × k` float64 table, in `mode` (the module documentation's functions), with numpy
/// `release`'s LAPACK; and, for GLIBC-07's exact notice, the other mode's rows where they differ.
pub fn normalize_rows(
    rows: &[f64],
    n: usize,
    k: usize,
    release: LapackRelease,
    mode: UpstreamDefects,
) -> Result<NipyRows, RowError> {
    assert_eq!(
        rows.len(),
        n * k,
        "normalize_rows: {} values for {n} × {k}",
        rows.len()
    );
    let other_mode = if mode.reproduce() {
        UpstreamDefects::Fix
    } else {
        UpstreamDefects::Reproduce
    };
    let mut params = Vec::with_capacity(n * 6);
    let mut other: Option<Vec<f64>> = None;
    for i in 0..n {
        let row = &rows[i * k..(i + 1) * k];
        let libm = Libm::new(mode);
        let p = normalize(row, release, &libm).map_err(|error| RowError { row: i, error })?;
        params.extend_from_slice(&p);
        if libm.differed() {
            let q = normalize(row, release, &Libm::quiet(other_mode)).unwrap_or([f64::NAN; 6]);
            if q.iter().zip(&p).any(|(a, b)| a.to_bits() != b.to_bits()) {
                let o = other.get_or_insert_with(|| params[..i * 6].to_vec());
                o.extend_from_slice(&q);
                continue;
            }
        }
        if let Some(o) = other.as_mut() {
            o.extend_from_slice(&p);
        }
    }
    Ok(NipyRows { params, other })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn libm() -> Libm {
        Libm::quiet(UpstreamDefects::Fix)
    }

    #[test]
    fn constants_are_nipys() {
        assert_eq!(LOG_MAX_DIST, larmorx_core::math::log(MAX_DIST));
        assert_eq!(MAX_ANGLE, 62831853071.795_86);
        assert_eq!(CY_THRESH, 8.881784197001252e-16);
    }

    #[test]
    fn a_rigid_row_round_trips_its_translations_and_small_angles() {
        let row = [1.5, -2.0, 0.25, 0.01, -0.02, 0.03];
        let p = normalize(&row, LapackRelease::default(), &libm()).unwrap();
        assert_eq!(&p[..3], &row[..3]);
        // Small rotations: the Euler angles are close to the rotation vector's components.
        for (a, b) in p[3..].iter().zip(&row[3..]) {
            assert!((a - b).abs() < 1e-3, "{a} vs {b}");
        }
        // The identity: zeros (x and z are -0: atan2(-0, 1), as nipype gives them).
        let p = normalize(&[0.0; 6], LapackRelease::default(), &libm()).unwrap();
        assert_eq!(
            p.map(f64::to_bits),
            [0.0, 0.0, 0.0, -0.0, 0.0, -0.0].map(f64::to_bits)
        );
    }

    #[test]
    fn lengths_nipy_cannot_index_stop_with_numpys_index_error() {
        let l = libm();
        let rel = LapackRelease::default();
        for (k, size) in [
            (0, 0),
            (2, 0),
            (3, 0),
            (4, 1),
            (5, 2),
            (8, 0),
            (9, 0),
            (10, 1),
            (11, 2),
        ] {
            let row = vec![0.1; k];
            assert_eq!(
                normalize(&row, rel, &l),
                Err(NipyError::Index(size)),
                "{k} values"
            );
        }
        // A rotation vector of one value above MAX_ANGLE is the identity before any index: the
        // row of four values then stops at the second, empty, rotation vector.
        assert_eq!(
            normalize(&[0.0, 0.0, 0.0, 1e11], rel, &l),
            Err(NipyError::Index(0))
        );
        // ... and ten values with such a second vector are a valid row.
        let mut row = vec![0.0; 10];
        row[9] = f64::INFINITY;
        assert!(normalize(&row, rel, &l).is_ok());
        assert_eq!(
            NipyError::Index(1).to_string(),
            "index 2 is out of bounds for axis 0 with size 1"
        );
    }

    #[test]
    fn scales_and_non_finite_values() {
        let l = libm();
        let rel = LapackRelease::default();
        // A negative scale flips the determinant: the Euler angles of the polar factor of RS
        // with its first column negated.
        assert!(normalize(&[0.0, 0.0, 0.0, 0.1, 0.2, 0.3, -2.0], rel, &l).is_ok());
        for bad in [0.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                normalize(&[0.0, 0.0, 0.0, 0.1, 0.2, 0.3, bad], rel, &l),
                Err(NipyError::Svd)
            );
        }
        // A NaN rotation vector: a matrix of NaNs.
        assert_eq!(
            normalize(&[0.0, 0.0, 0.0, f64::NAN, 0.0, 0.0], rel, &l),
            Err(NipyError::Svd)
        );
        // An infinite rotation vector is the identity; a NaN translation stays.
        let p = normalize(&[f64::NAN, 1.0, 2.0, f64::INFINITY, 0.0, 0.0], rel, &l).unwrap();
        assert!(p[0].is_nan());
        assert_eq!(&p[1..], &[1.0, 2.0, 0.0, 0.0, 0.0]);
        // Translations and log-scales are bounded.
        let p = normalize(&[1e300, -1e300, 0.0, 0.0, 0.0, 0.0], rel, &l).unwrap();
        assert_eq!(&p[..2], &[MAX_DIST, -MAX_DIST]);
        let row = [
            0.0, 0.0, 0.0, 0.1, 0.0, 0.0, 400.0, -400.0, 0.0, 0.0, 0.0, 0.2,
        ];
        assert!(normalize(&row, rel, &l).is_ok());
    }

    #[test]
    fn python_atan2_special_cases() {
        use std::f64::consts::PI;
        assert_eq!(py_atan2(0.0, -0.0), PI);
        assert_eq!(py_atan2(-0.0, -1.0), -PI);
        assert_eq!(py_atan2(-0.0, 0.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(py_atan2(f64::INFINITY, f64::NEG_INFINITY), 0.75 * PI);
        assert_eq!(py_atan2(f64::NEG_INFINITY, 2.0), -0.5 * PI);
        assert_eq!(py_atan2(1.0, f64::NEG_INFINITY), PI);
        assert_eq!(py_atan2(f64::NAN, 1.0).to_bits(), 0x7ff8_0000_0000_0000);
    }

    #[test]
    fn rows_report_the_first_row_that_stops_and_the_other_mode() {
        let rows = [
            0.0,
            0.0,
            0.0,
            0.5,
            0.25,
            0.125,
            0.0,
            0.0,
            0.0,
            f64::NAN,
            0.0,
            0.0,
        ];
        assert_eq!(
            normalize_rows(&rows, 2, 6, LapackRelease::default(), UpstreamDefects::Fix),
            Err(RowError {
                row: 1,
                error: NipyError::Svd
            })
        );
        // Many rotations: the other mode is reported only where a parameter differs, with the
        // same values elsewhere.
        let n = 400;
        let rows: Vec<f64> = (0..n * 6)
            .map(|i| ((i * 7919 % 1000) as f64 - 500.0) * 1e-3)
            .collect();
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let out = normalize_rows(&rows, n, 6, LapackRelease::default(), mode).unwrap();
            let other_mode = if mode.fix() {
                UpstreamDefects::Reproduce
            } else {
                UpstreamDefects::Fix
            };
            let full = normalize_rows(&rows, n, 6, LapackRelease::default(), other_mode)
                .unwrap()
                .params;
            match out.other {
                Some(o) => assert_eq!(
                    o.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    full.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                ),
                None => assert_eq!(out.params, full),
            }
        }
    }
}
