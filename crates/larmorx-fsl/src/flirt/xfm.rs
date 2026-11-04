// SPDX-License-Identifier: Apache-2.0
//! `convert_xfm`'s matrix algebra (`specs/flirt.md` §10): concatenation, the scales and skews of
//! another matrix, the inverse.

use larmorx_core::math::Libm;

use super::mat::{self, Mat4};

/// `m`'s rotations and translations with `other`'s scales and skews (§10): both decomposed about
/// the origin, combined, composed with 12 parameters (`libm`'s functions: glibc's with
/// `reproduce`, GLIBC-07).
pub fn fix_scale_skew(m: &Mat4, other: &Mat4, libm: &Libm) -> Mat4 {
    let a = super::param::euler_decompose(m, [0.0; 3], libm);
    let b = super::param::euler_decompose(other, [0.0; 3], libm);
    let mut p = a;
    p[6..12].copy_from_slice(&b[6..12]);
    super::param::euler_compose(&p, 12, [0.0; 3], libm)
}

/// `second · m` (§10).
pub fn concat(m: &Mat4, second: &Mat4) -> Mat4 {
    mat::mul(second, m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixing_scales_and_skews() {
        let a: Mat4 = [
            [1.05, 0.02, -0.03, 2.0],
            [-0.01, 0.97, 0.04, -3.0],
            [0.02, -0.05, 1.1, 1.5],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let b: Mat4 = [
            [0.99, 0.1, 0.0, 1.0],
            [-0.1, 0.99, 0.05, 2.0],
            [0.0, -0.05, 1.2, -3.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        // FSL 6.0.7.23's convert_xfm -fixscaleskew b a (observed)
        let want = [
            [0.9948121327, 0.01045570067, -0.02738253071, 2.0],
            [-0.009474399331, 0.9949077902, 0.05179466561, -3.0],
            [0.01894880027, -0.05147051078, 1.199611428, 1.5],
        ];
        let got = fix_scale_skew(&a, &b, &Libm::default());
        for i in 0..3 {
            for j in 0..4 {
                assert!(
                    (got[i][j] - want[i][j]).abs() < 1e-6,
                    "{i} {j}: {}",
                    got[i][j]
                );
            }
        }
    }
}
