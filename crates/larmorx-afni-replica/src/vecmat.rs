// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05: the double-precision matrix and vector macros of
// src/vecmat.h (LOAD_ROT_DMAT, DMAT_MUL, DMATVEC, DMATVEC_ADD, TRANSPOSE_DMAT, DMAT_INV,
// DMAT_DET, DMAT_TRACE, MUL_DVECMAT, SIZE_DFVEC3, MATVEC with a double matrix), src/thd_read_vecmat.c
// (invert_dvecmat) and the C library functions AFNI's build calls on these paths. Major portions
// Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.), released by
// AFNI 26.3.05 under the Creative Commons Attribution 4.0 International licence (CC BY 4.0).
// thd_read_vecmat.c carries no such notice: it was written at the NIH and is in the public domain.
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05.
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! AFNI's 3 x 3 matrices and 3-vectors in double precision (`THD_dmat33`, `THD_dfvec3`,
//! `THD_dvecmat`), with the operation order of `vecmat.h`'s macros, and the C library functions
//! the rotation code calls ([`Libc`]).
//!
//! Every macro evaluates its sums left to right, as written: `ROW_DOT_COL(A,B,i,j)` is
//! `A[i][0]·B[0][j] + A[i][1]·B[1][j] + A[i][2]·B[2][j]`, `DMATVEC` the same along a row.

use larmorx_core::defects::UpstreamDefects;

/// `THD_dmat33`: `m[i][j]` is row `i`, column `j`.
pub type DMat33 = [[f64; 3]; 3];

/// `THD_dfvec3`.
pub type DVec3 = [f64; 3];

/// `THD_fvec3`.
pub type FVec3 = [f32; 3];

/// `THD_dvecmat`: the map `x -> mm·x + vv`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DVecMat {
    pub mm: DMat33,
    pub vv: DVec3,
}

/// The identity matrix (`LOAD_DIAG_DMAT(A,1,1,1)`).
pub const IDENTITY: DMat33 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// `LOAD_DIAG_DMAT`.
pub fn diag(x: f64, y: f64, z: f64) -> DMat33 {
    [[x, 0.0, 0.0], [0.0, y, 0.0], [0.0, 0.0, z]]
}

/// `DMAT_MUL(A,B)`.
pub fn mul(a: &DMat33, b: &DMat33) -> DMat33 {
    let mut c = [[0.0; 3]; 3];
    for (i, row) in c.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    c
}

/// `DMATVEC(A,x)`.
pub fn matvec(a: &DMat33, x: &DVec3) -> DVec3 {
    [
        a[0][0] * x[0] + a[0][1] * x[1] + a[0][2] * x[2],
        a[1][0] * x[0] + a[1][1] * x[1] + a[1][2] * x[2],
        a[2][0] * x[0] + a[2][1] * x[1] + a[2][2] * x[2],
    ]
}

/// `DMATVEC_ADD(A,x,b)`: `A·x + b`, the sum along the row first.
pub fn matvec_add(a: &DMat33, x: &DVec3, b: &DVec3) -> DVec3 {
    [
        a[0][0] * x[0] + a[0][1] * x[1] + a[0][2] * x[2] + b[0],
        a[1][0] * x[0] + a[1][1] * x[1] + a[1][2] * x[2] + b[1],
        a[2][0] * x[0] + a[2][1] * x[1] + a[2][2] * x[2] + b[2],
    ]
}

/// `MATVEC(A,x)` with a double matrix and a float vector (3dvolreg's `-1Dmatrix_save`): the
/// products and sums in double, each component stored as float.
pub fn matvec_f(a: &DMat33, x: &FVec3) -> FVec3 {
    let x = x.map(f64::from);
    matvec(a, &x).map(|v| v as f32)
}

/// `TRANSPOSE_DMAT(A)`.
pub fn transpose(a: &DMat33) -> DMat33 {
    let mut t = [[0.0; 3]; 3];
    for (i, row) in a.iter().enumerate() {
        for (j, &v) in row.iter().enumerate() {
            t[j][i] = v;
        }
    }
    t
}

/// `DMAT_DET(A)`.
pub fn det(a: &DMat33) -> f64 {
    a[0][0] * a[1][1] * a[2][2] - a[0][0] * a[1][2] * a[2][1] - a[1][0] * a[0][1] * a[2][2]
        + a[1][0] * a[0][2] * a[2][1]
        + a[2][0] * a[0][1] * a[1][2]
        - a[2][0] * a[0][2] * a[1][1]
}

/// `DMAT_TRACE(A)`.
pub fn trace(a: &DMat33) -> f64 {
    a[0][0] + a[1][1] + a[2][2]
}

/// `DMAT_INV(A)`: the adjugate times `1/det`, each entry `(products) * (1/det)`.
pub fn inv(a: &DMat33) -> DMat33 {
    let d = 1.0 / det(a);
    let mut t = [[0.0; 3]; 3];
    t[1][1] = (a[0][0] * a[2][2] - a[0][2] * a[2][0]) * d;
    t[2][2] = (a[0][0] * a[1][1] - a[0][1] * a[1][0]) * d;
    t[2][0] = (a[1][0] * a[2][1] - a[1][1] * a[2][0]) * d;
    t[1][2] = (-a[0][0] * a[1][2] + a[0][2] * a[1][0]) * d;
    t[0][1] = (-a[0][1] * a[2][2] + a[0][2] * a[2][1]) * d;
    t[0][0] = (a[1][1] * a[2][2] - a[1][2] * a[2][1]) * d;
    t[2][1] = (-a[0][0] * a[2][1] + a[0][1] * a[2][0]) * d;
    t[1][0] = (-a[1][0] * a[2][2] + a[1][2] * a[2][0]) * d;
    t[0][2] = (a[0][1] * a[1][2] - a[0][2] * a[1][1]) * d;
    t
}

/// `SUB_DFVEC3(a,b)`.
pub fn sub(a: &DVec3, b: &DVec3) -> DVec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `ADD_DFVEC3(a,b)`.
pub fn add(a: &DVec3, b: &DVec3) -> DVec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `SIZE_DFVEC3(a)`: `sqrt(a0·a0 + a1·a1 + a2·a2)`.
pub fn size(a: &DVec3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// `MUL_DVECMAT(A,B)`: the map `A ∘ B` (`A.mm·B.mm`, `A.mm·B.vv + A.vv`).
pub fn mul_vecmat(a: &DVecMat, b: &DVecMat) -> DVecMat {
    let mm = mul(&a.mm, &b.mm);
    let t = matvec(&a.mm, &b.vv);
    DVecMat {
        mm,
        vv: add(&t, &a.vv),
    }
}

/// `invert_dvecmat` (`thd_read_vecmat.c`): `inv(mm)` and `-(inv(mm)·vv)`.
pub fn invert_vecmat(a: &DVecMat) -> DVecMat {
    let mm = inv(&a.mm);
    let v = matvec(&mm, &a.vv);
    DVecMat {
        mm,
        vv: [-v[0], -v[1], -v[2]],
    }
}

/// The C library's double functions on the rotation and blur paths: correctly rounded by
/// default ([`UpstreamDefects::Fix`]), glibc 2.35's (the functions AFNI's x86-64 build calls)
/// with `reproduce` (GLIBC-07). A fit or a filter, so it names nothing (the user's decision of
/// 2026-11-04, `larmorx_core::math::Libm::quiet`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Libc {
    glibc: bool,
}

impl Libc {
    /// The functions `mode` asks for.
    pub fn new(mode: UpstreamDefects) -> Libc {
        Libc {
            glibc: mode.reproduce(),
        }
    }

    /// Whether these are glibc's functions (`reproduce`).
    pub fn is_glibc(self) -> bool {
        self.glibc
    }

    /// `exp(x)`.
    pub fn exp(self, x: f64) -> f64 {
        if self.glibc {
            larmorx_core::math::exp_glibc(x)
        } else {
            larmorx_core::math::exp(x)
        }
    }

    /// `pow(x, y)`.
    pub fn pow(self, x: f64, y: f64) -> f64 {
        if self.glibc {
            larmorx_core::math::pow_glibc(x, y)
        } else {
            larmorx_core::math::pow(x, y)
        }
    }

    /// `sin(x)` as its own call.
    pub fn sin(self, x: f64) -> f64 {
        if self.glibc {
            larmorx_core::math::sin_glibc(x)
        } else {
            larmorx_core::math::sin(x)
        }
    }

    /// `cos(x)` as its own call.
    pub fn cos(self, x: f64) -> f64 {
        if self.glibc {
            larmorx_core::math::cos_glibc(x)
        } else {
            larmorx_core::math::cos(x)
        }
    }

    /// `sin(x)` and `cos(x)` where GCC merged the two calls into one `sincos` (glibc's own
    /// paths, whose sine can differ from `sin`'s in the last bit).
    pub fn sincos(self, x: f64) -> (f64, f64) {
        crate::csfft::sincos(x, self.glibc)
    }
}

/// `LOAD_ROT_DMAT(A, th, i)`: the rotation by `th` about axis `i` (0, 1, 2; any other axis
/// gives the zero matrix), from axis `aa` toward `bb` (`LOAD_ROTGEN_DMAT`: x from y to z, y
/// from z to x, z from x to y): `A[aa][aa] = A[bb][bb] = cos th`, `A[aa][bb] = sin th`,
/// `A[bb][aa] = -sin th`, `A[ff][ff] = 1`.
///
/// AFNI's build (GCC 11 at `-O2`) computes `cos` and `sin` with two calls for axes 0 and 1 and
/// with one `sincos` call for axis 2 in `rot_to_matrix` (its object code;
/// `docs/findings/afni-volreg.md`), which matters only for glibc's bits.
pub fn rot_matrix(th: f64, axis: i32, libc: Libc) -> DMat33 {
    let (ff, aa, bb) = match axis {
        0 => (0, 1, 2),
        1 => (1, 2, 0),
        2 => (2, 0, 1),
        _ => return [[0.0; 3]; 3],
    };
    let (s, c) = if axis == 2 {
        libc.sincos(th)
    } else {
        (libc.sin(th), libc.cos(th))
    };
    let mut a = [[0.0; 3]; 3];
    a[aa][aa] = c;
    a[bb][bb] = c;
    a[aa][bb] = s;
    a[bb][aa] = -s;
    a[ff][ff] = 1.0;
    a
}

/// `rot_to_matrix(ax1,th1, ax2,th2, ax3,th3)` (`thd_shear3d.c`): `R3·R2·R1`, `Ri` the rotation
/// by `thi` about axis `axi`.
pub fn rot_to_matrix(ax1: i32, th1: f64, ax2: i32, th2: f64, ax3: i32, th3: f64, libc: Libc) -> DMat33 {
    let mut q = rot_matrix(th1, ax1, libc);
    let p = rot_matrix(th2, ax2, libc);
    q = mul(&p, &q);
    let p = rot_matrix(th3, ax3, libc);
    mul(&p, &q)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotations_compose() {
        let libc = Libc::default();
        let q = rot_to_matrix(0, 0.1, 1, -0.2, 2, 0.3, libc);
        // A rotation: orthogonal with determinant 1.
        let qtq = mul(&transpose(&q), &q);
        for (i, row) in qtq.iter().enumerate() {
            for (j, &v) in row.iter().enumerate() {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!((v - want).abs() < 1e-15);
            }
        }
        assert!((det(&q) - 1.0).abs() < 1e-15);
        assert_eq!(rot_matrix(0.5, 7, libc), [[0.0; 3]; 3]);
        let z = rot_matrix(0.5, 2, libc);
        assert_eq!(z[0][1], 0.5f64.sin());
        assert_eq!(z[1][0], -(0.5f64.sin()));
        let a = DVecMat {
            mm: q,
            vv: [1.0, -2.0, 3.0],
        };
        let b = invert_vecmat(&a);
        let id = mul_vecmat(&b, &a);
        for i in 0..3 {
            assert!(id.vv[i].abs() < 1e-14);
        }
    }
}
