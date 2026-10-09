// SPDX-License-Identifier: Apache-2.0
//! Unit quaternions ⇄ 3×3 rotation matrices.
//!
//! The quaternion is `[w, x, y, z]` (w is the scalar part), as in NIfTI's qform and nibabel.

use crate::linalg::{IDENTITY3, Mat3};

/// The rotation matrix of quaternion `q`, which need not be normalised.
///
/// Same algorithm and operation order as `nibabel.quaternions.quat2mat`; returns the identity
/// when `|q|²` is below f64 epsilon.
pub fn quaternion_to_matrix(q: [f64; 4]) -> Mat3 {
    let [w, x, y, z] = q;
    let nq = w * w + x * x + y * y + z * z;
    if nq < f64::EPSILON {
        return IDENTITY3;
    }
    let s = 2.0 / nq;
    let (xs, ys, zs) = (x * s, y * s, z * s);
    let (wx, wy, wz) = (w * xs, w * ys, w * zs);
    let (xx, xy, xz) = (x * xs, x * ys, x * zs);
    let (yy, yz, zz) = (y * ys, y * zs, z * zs);
    [
        [1.0 - (yy + zz), xy - wz, xz + wy],
        [xy + wz, 1.0 - (xx + zz), yz - wx],
        [xz - wy, yz + wx, 1.0 - (xx + yy)],
    ]
}

/// The unit quaternion of rotation matrix `r`, with `w >= 0`.
///
/// `r` must be orthogonal with determinant +1. Uses Shepperd's method (branch on the largest
/// of the trace and the diagonal), which is accurate for every rotation angle.
pub fn matrix_to_quaternion(r: &Mat3) -> [f64; 4] {
    let trace = r[0][0] + r[1][1] + r[2][2];
    let q = if trace > r[0][0].max(r[1][1]).max(r[2][2]) {
        let s = 2.0 * (1.0 + trace).sqrt();
        [
            0.25 * s,
            (r[2][1] - r[1][2]) / s,
            (r[0][2] - r[2][0]) / s,
            (r[1][0] - r[0][1]) / s,
        ]
    } else if r[0][0] >= r[1][1] && r[0][0] >= r[2][2] {
        let s = 2.0 * (1.0 + r[0][0] - r[1][1] - r[2][2]).sqrt();
        [
            (r[2][1] - r[1][2]) / s,
            0.25 * s,
            (r[0][1] + r[1][0]) / s,
            (r[0][2] + r[2][0]) / s,
        ]
    } else if r[1][1] >= r[2][2] {
        let s = 2.0 * (1.0 + r[1][1] - r[0][0] - r[2][2]).sqrt();
        [
            (r[0][2] - r[2][0]) / s,
            (r[0][1] + r[1][0]) / s,
            0.25 * s,
            (r[1][2] + r[2][1]) / s,
        ]
    } else {
        let s = 2.0 * (1.0 + r[2][2] - r[0][0] - r[1][1]).sqrt();
        [
            (r[1][0] - r[0][1]) / s,
            (r[0][2] + r[2][0]) / s,
            (r[1][2] + r[2][1]) / s,
            0.25 * s,
        ]
    };
    let norm = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    let sign = if q[0] < 0.0 { -1.0 } else { 1.0 };
    q.map(|v| sign * v / norm)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis_angle(axis: [f64; 3], angle: f64) -> [f64; 4] {
        let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        let (s, c) = ((angle / 2.0).sin(), (angle / 2.0).cos());
        [c, s * axis[0] / n, s * axis[1] / n, s * axis[2] / n]
    }

    #[test]
    fn round_trips_for_many_rotations() {
        let axes = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 2.0, -0.5],
            [-0.3, 0.1, 0.9],
        ];
        for axis in axes {
            for k in 0..=24 {
                let q = axis_angle(
                    axis,
                    -std::f64::consts::PI + k as f64 * std::f64::consts::PI / 12.0,
                );
                let r = quaternion_to_matrix(q);
                let back = matrix_to_quaternion(&r);
                let r2 = quaternion_to_matrix(back);
                assert!(back[0] >= 0.0);
                for i in 0..3 {
                    for j in 0..3 {
                        assert!((r[i][j] - r2[i][j]).abs() < 1e-14, "{axis:?} {k}");
                    }
                }
            }
        }
    }

    #[test]
    fn half_turns_and_degenerate_input() {
        // 180° about z (the rotation of an LPS-oriented affine): w = 0.
        let q = matrix_to_quaternion(&[[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]]);
        assert_eq!(q, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(quaternion_to_matrix([0.0; 4]), IDENTITY3);
        assert_eq!(quaternion_to_matrix([1.0, 0.0, 0.0, 0.0]), IDENTITY3);
    }
}
