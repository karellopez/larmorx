// SPDX-License-Identifier: LicenseRef-FSL AND Apache-2.0
// Copyright 2026 Karel Lopez Vilaret (the translation).
//
// The formulas follow Armadillo 10.5.1 (Apache-2.0; Copyright 2008-2016 Conrad Sanderson,
// Copyright 2008-2016 National ICT Australia (NICTA)), as fsl-armawrap 0.6.0 bundles it:
// op_det_meat.hpp (apply_tiny), op_inv_meat.hpp (apply_tiny_noalias), mul_gemv.hpp
// (gemv_emul_tinysq), mul_gemm.hpp (gemm_emul_large), op_dot_meat.hpp (direct_dot_arma).
//! The 4x4 matrix arithmetic of FSL's `newimage` (NEWMAT on top of Armadillo, `armawrap`): the
//! voxel-to-world matrices that the left-right swap rewrites, the determinant that decides it,
//! and the voxel conversions of `-c` and `fslstats -C`.
//!
//! Armadillo computes matrices of at most 4x4 with closed formulas: the determinant
//! (`det`, falling back to the diagonal's product for a diagonal or triangular matrix when the
//! formula's value is below epsilon or above its inverse), the inverse (`inv`, the cofactor
//! formula, falling back to the diagonal's reciprocals), and products (`A * B`: each element
//! `((a0*b0 + a1*b1) + a2*b2) + a3*b3`; `inv(A) * B` is a solve: the tiny inverse times `B`
//! with two interleaved accumulators). Products of more than two matrices are evaluated left
//! to right (all costs are equal for 4x4). Matrices are column-major: `m[c][r]`.

// index loops, as the C and LAPACK code they translate
#![allow(clippy::needless_range_loop)]

/// A 4x4 matrix, column-major (`m[col][row]`), as Armadillo stores it.
pub type Mat4 = [[f64; 4]; 4];

pub const IDENTITY: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// `X(r, c)`.
#[inline]
fn at(x: &Mat4, r: usize, c: usize) -> f64 {
    x[c][r]
}

/// A matrix from rows (`rows[r][c]`).
pub fn from_rows(rows: [[f64; 4]; 4]) -> Mat4 {
    let mut m = [[0.0; 4]; 4];
    for (r, row) in rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            m[c][r] = *v;
        }
    }
    m
}

/// The rows of `m` (`rows[r][c]`).
pub fn to_rows(m: &Mat4) -> [[f64; 4]; 4] {
    let mut rows = [[0.0; 4]; 4];
    for (r, row) in rows.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            *v = m[c][r];
        }
    }
    rows
}

/// `op_det::apply_tiny` for N = 4.
pub fn det_tiny(x: &Mat4) -> f64 {
    let a = |r, c| at(x, r, c);
    let val_03_12 = a(0, 3) * a(1, 2);
    let val_02_13 = a(0, 2) * a(1, 3);
    let val_03_11 = a(0, 3) * a(1, 1);
    let val_01_13 = a(0, 1) * a(1, 3);
    let val_02_11 = a(0, 2) * a(1, 1);
    let val_01_12 = a(0, 1) * a(1, 2);
    let val_03_10 = a(0, 3) * a(1, 0);
    let val_00_13 = a(0, 0) * a(1, 3);
    let val_02_10 = a(0, 2) * a(1, 0);
    let val_00_12 = a(0, 0) * a(1, 2);
    let val_01_10 = a(0, 1) * a(1, 0);
    let val_00_11 = a(0, 0) * a(1, 1);
    let val_21_30 = a(2, 1) * a(3, 0);
    let val_22_30 = a(2, 2) * a(3, 0);
    let val_23_30 = a(2, 3) * a(3, 0);
    let val_20_31 = a(2, 0) * a(3, 1);
    let val_22_31 = a(2, 2) * a(3, 1);
    let val_23_31 = a(2, 3) * a(3, 1);
    let val_20_32 = a(2, 0) * a(3, 2);
    let val_21_32 = a(2, 1) * a(3, 2);
    let val_23_32 = a(2, 3) * a(3, 2);
    let val_20_33 = a(2, 0) * a(3, 3);
    let val_21_33 = a(2, 1) * a(3, 3);
    let val_22_33 = a(2, 2) * a(3, 3);
    val_03_12 * val_21_30 - val_02_13 * val_21_30 - val_03_11 * val_22_30
        + val_01_13 * val_22_30
        + val_02_11 * val_23_30
        - val_01_12 * val_23_30
        - val_03_12 * val_20_31
        + val_02_13 * val_20_31
        + val_03_10 * val_22_31
        - val_00_13 * val_22_31
        - val_02_10 * val_23_31
        + val_00_12 * val_23_31
        + val_03_11 * val_20_32
        - val_01_13 * val_20_32
        - val_03_10 * val_21_32
        + val_00_13 * val_21_32
        + val_01_10 * val_23_32
        - val_00_11 * val_23_32
        - val_02_11 * val_20_33
        + val_01_12 * val_20_33
        + val_02_10 * val_21_33
        - val_00_12 * val_21_33
        - val_01_10 * val_22_33
        + val_00_11 * val_22_33
}

fn is_diag(x: &Mat4) -> bool {
    (0..4).all(|c| (0..4).all(|r| r == c || x[c][r] == 0.0))
}

fn is_triu(x: &Mat4) -> bool {
    (0..4).all(|c| (c + 1..4).all(|r| x[c][r] == 0.0))
}

fn is_tril(x: &Mat4) -> bool {
    (0..4).all(|c| (0..c).all(|r| x[c][r] == 0.0))
}

/// `arma::det` of a 4x4 matrix (`op_det::apply_direct`).
pub fn det(x: &Mat4) -> f64 {
    let d = det_tiny(x);
    let ad = d.abs();
    if ad > f64::EPSILON && ad < 1.0 / f64::EPSILON {
        return d;
    }
    if is_diag(x) || is_triu(x) || is_tril(x) {
        // apply_diagmat / apply_trimat: the product of the diagonal.
        let mut v = x[0][0];
        for i in 1..4 {
            v *= x[i][i];
        }
        return v;
    }
    lapack_det(x)
}

/// `auxlib::det`: LAPACK's LU (netlib `dgetf2`, partial pivoting) and the product of its
/// diagonal, its sign flipped once per row interchange.
fn lapack_det(x: &Mat4) -> f64 {
    let mut a = *x;
    let mut piv = [0usize; 4];
    getf2(&mut a, &mut piv);
    let mut val = a[0][0];
    for i in 1..4 {
        val *= a[i][i];
    }
    let mut sign = 1i32;
    for (i, p) in piv.iter().enumerate() {
        if i != *p {
            sign = -sign;
        }
    }
    if sign < 0 { -val } else { val }
}

/// netlib LAPACK's unblocked LU with partial pivoting (`dgetf2`, as `dgetrf` runs it for a
/// 4x4 matrix), in place; `piv[j]` is the row swapped with row `j`.
fn getf2(a: &mut Mat4, piv: &mut [usize; 4]) {
    let sfmin = f64::MIN_POSITIVE;
    for j in 0..4 {
        // idamax: the first row of largest absolute value
        let mut jp = j;
        let mut best = a[j][j].abs();
        for i in j + 1..4 {
            if a[j][i].abs() > best {
                best = a[j][i].abs();
                jp = i;
            }
        }
        piv[j] = jp;
        if a[j][jp] != 0.0 {
            if jp != j {
                for col in a.iter_mut() {
                    col.swap(j, jp);
                }
            }
            if j < 3 {
                if a[j][j].abs() >= sfmin {
                    let r = 1.0 / a[j][j];
                    for i in j + 1..4 {
                        a[j][i] *= r;
                    }
                } else {
                    for i in j + 1..4 {
                        a[j][i] /= a[j][j];
                    }
                }
            }
        }
        if j < 3 {
            // dger: A(j+1:, j+1:) -= A(j+1:, j) * A(j, j+1:)
            for c in j + 1..4 {
                let t = a[c][j];
                if t != 0.0 {
                    let t = -t;
                    for r in j + 1..4 {
                        a[c][r] += a[j][r] * t;
                    }
                }
            }
        }
    }
}

/// `op_inv::apply_tiny_noalias` for N = 4: `None` when Armadillo's checks reject the formula.
pub fn inv_tiny(x: &Mat4) -> Option<Mat4> {
    let d = det_tiny(x);
    let ad = d.abs();
    if ad < f64::EPSILON || ad > 1.0 / f64::EPSILON {
        return None;
    }
    let a = |r, c| at(x, r, c);
    let mut o = [[0.0; 4]; 4];
    let mut set = |r: usize, c: usize, v: f64| o[c][r] = v / d;
    set(
        0,
        0,
        a(1, 2) * a(2, 3) * a(3, 1) - a(1, 3) * a(2, 2) * a(3, 1) + a(1, 3) * a(2, 1) * a(3, 2)
            - a(1, 1) * a(2, 3) * a(3, 2)
            - a(1, 2) * a(2, 1) * a(3, 3)
            + a(1, 1) * a(2, 2) * a(3, 3),
    );
    set(
        1,
        0,
        a(1, 3) * a(2, 2) * a(3, 0) - a(1, 2) * a(2, 3) * a(3, 0) - a(1, 3) * a(2, 0) * a(3, 2)
            + a(1, 0) * a(2, 3) * a(3, 2)
            + a(1, 2) * a(2, 0) * a(3, 3)
            - a(1, 0) * a(2, 2) * a(3, 3),
    );
    set(
        2,
        0,
        a(1, 1) * a(2, 3) * a(3, 0) - a(1, 3) * a(2, 1) * a(3, 0) + a(1, 3) * a(2, 0) * a(3, 1)
            - a(1, 0) * a(2, 3) * a(3, 1)
            - a(1, 1) * a(2, 0) * a(3, 3)
            + a(1, 0) * a(2, 1) * a(3, 3),
    );
    set(
        3,
        0,
        a(1, 2) * a(2, 1) * a(3, 0) - a(1, 1) * a(2, 2) * a(3, 0) - a(1, 2) * a(2, 0) * a(3, 1)
            + a(1, 0) * a(2, 2) * a(3, 1)
            + a(1, 1) * a(2, 0) * a(3, 2)
            - a(1, 0) * a(2, 1) * a(3, 2),
    );
    set(
        0,
        1,
        a(0, 3) * a(2, 2) * a(3, 1) - a(0, 2) * a(2, 3) * a(3, 1) - a(0, 3) * a(2, 1) * a(3, 2)
            + a(0, 1) * a(2, 3) * a(3, 2)
            + a(0, 2) * a(2, 1) * a(3, 3)
            - a(0, 1) * a(2, 2) * a(3, 3),
    );
    set(
        1,
        1,
        a(0, 2) * a(2, 3) * a(3, 0) - a(0, 3) * a(2, 2) * a(3, 0) + a(0, 3) * a(2, 0) * a(3, 2)
            - a(0, 0) * a(2, 3) * a(3, 2)
            - a(0, 2) * a(2, 0) * a(3, 3)
            + a(0, 0) * a(2, 2) * a(3, 3),
    );
    set(
        2,
        1,
        a(0, 3) * a(2, 1) * a(3, 0) - a(0, 1) * a(2, 3) * a(3, 0) - a(0, 3) * a(2, 0) * a(3, 1)
            + a(0, 0) * a(2, 3) * a(3, 1)
            + a(0, 1) * a(2, 0) * a(3, 3)
            - a(0, 0) * a(2, 1) * a(3, 3),
    );
    set(
        3,
        1,
        a(0, 1) * a(2, 2) * a(3, 0) - a(0, 2) * a(2, 1) * a(3, 0) + a(0, 2) * a(2, 0) * a(3, 1)
            - a(0, 0) * a(2, 2) * a(3, 1)
            - a(0, 1) * a(2, 0) * a(3, 2)
            + a(0, 0) * a(2, 1) * a(3, 2),
    );
    set(
        0,
        2,
        a(0, 2) * a(1, 3) * a(3, 1) - a(0, 3) * a(1, 2) * a(3, 1) + a(0, 3) * a(1, 1) * a(3, 2)
            - a(0, 1) * a(1, 3) * a(3, 2)
            - a(0, 2) * a(1, 1) * a(3, 3)
            + a(0, 1) * a(1, 2) * a(3, 3),
    );
    set(
        1,
        2,
        a(0, 3) * a(1, 2) * a(3, 0) - a(0, 2) * a(1, 3) * a(3, 0) - a(0, 3) * a(1, 0) * a(3, 2)
            + a(0, 0) * a(1, 3) * a(3, 2)
            + a(0, 2) * a(1, 0) * a(3, 3)
            - a(0, 0) * a(1, 2) * a(3, 3),
    );
    set(
        2,
        2,
        a(0, 1) * a(1, 3) * a(3, 0) - a(0, 3) * a(1, 1) * a(3, 0) + a(0, 3) * a(1, 0) * a(3, 1)
            - a(0, 0) * a(1, 3) * a(3, 1)
            - a(0, 1) * a(1, 0) * a(3, 3)
            + a(0, 0) * a(1, 1) * a(3, 3),
    );
    set(
        3,
        2,
        a(0, 2) * a(1, 1) * a(3, 0) - a(0, 1) * a(1, 2) * a(3, 0) - a(0, 2) * a(1, 0) * a(3, 1)
            + a(0, 0) * a(1, 2) * a(3, 1)
            + a(0, 1) * a(1, 0) * a(3, 2)
            - a(0, 0) * a(1, 1) * a(3, 2),
    );
    set(
        0,
        3,
        a(0, 3) * a(1, 2) * a(2, 1) - a(0, 2) * a(1, 3) * a(2, 1) - a(0, 3) * a(1, 1) * a(2, 2)
            + a(0, 1) * a(1, 3) * a(2, 2)
            + a(0, 2) * a(1, 1) * a(2, 3)
            - a(0, 1) * a(1, 2) * a(2, 3),
    );
    set(
        1,
        3,
        a(0, 2) * a(1, 3) * a(2, 0) - a(0, 3) * a(1, 2) * a(2, 0) + a(0, 3) * a(1, 0) * a(2, 2)
            - a(0, 0) * a(1, 3) * a(2, 2)
            - a(0, 2) * a(1, 0) * a(2, 3)
            + a(0, 0) * a(1, 2) * a(2, 3),
    );
    set(
        2,
        3,
        a(0, 3) * a(1, 1) * a(2, 0) - a(0, 1) * a(1, 3) * a(2, 0) - a(0, 3) * a(1, 0) * a(2, 1)
            + a(0, 0) * a(1, 3) * a(2, 1)
            + a(0, 1) * a(1, 0) * a(2, 3)
            - a(0, 0) * a(1, 1) * a(2, 3),
    );
    set(
        3,
        3,
        a(0, 1) * a(1, 2) * a(2, 0) - a(0, 2) * a(1, 1) * a(2, 0) + a(0, 2) * a(1, 0) * a(2, 1)
            - a(0, 0) * a(1, 2) * a(2, 1)
            - a(0, 1) * a(1, 0) * a(2, 2)
            + a(0, 0) * a(1, 1) * a(2, 2),
    );
    let check = at(x, 0, 0) * o[0][0]
        + at(x, 0, 1) * o[0][1]
        + at(x, 0, 2) * o[0][2]
        + at(x, 0, 3) * o[0][3];
    if (1.0 - check).abs() >= 1e-10 {
        return None;
    }
    Some(o)
}

/// Why an inverse failed: Armadillo's `inv(): matrix is singular` (a `std::runtime_error`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Singular;

/// `arma::inv` of a 4x4 matrix (`op_inv::apply_direct`): the tiny formula, else the diagonal's
/// reciprocals for a diagonal matrix; larger fallbacks (LAPACK's triangular and general
/// inverses) are reported as [`Singular`] (never reached by BET's matrices: the swap matrices'
/// determinant is ±1 and the sampling matrices are diagonal).
pub fn inv(x: &Mat4) -> Result<Mat4, Singular> {
    if let Some(o) = inv_tiny(x) {
        return Ok(o);
    }
    if is_diag(x) {
        let mut o = [[0.0; 4]; 4];
        for (i, col) in o.iter_mut().enumerate() {
            if x[i][i] == 0.0 {
                return Err(Singular);
            }
            col[i] = 1.0 / x[i][i];
        }
        return Ok(o);
    }
    Err(Singular)
}

/// `A * B` (`gemm_emul_tinysq`: each column of `B` by `gemv_emul_tinysq`).
pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut c = [[0.0; 4]; 4];
    for j in 0..4 {
        c[j] = mul_vec(a, &b[j]);
    }
    c
}

/// `A * x` for a 4-vector (`gemv_emul_tinysq`).
pub fn mul_vec(a: &Mat4, x: &[f64; 4]) -> [f64; 4] {
    let mut y = [0.0; 4];
    for (i, yi) in y.iter_mut().enumerate() {
        *yi = at(a, i, 0) * x[0] + at(a, i, 1) * x[1] + at(a, i, 2) * x[2] + at(a, i, 3) * x[3];
    }
    y
}

/// `op_dot::direct_dot_arma` for 4 values: two interleaved accumulators.
fn dot4(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    let mut val1 = 0.0;
    let mut val2 = 0.0;
    val1 += a[0] * b[0];
    val2 += a[1] * b[1];
    val1 += a[2] * b[2];
    val2 += a[3] * b[3];
    val1 + val2
}

/// `inv(A) * x` as Armadillo evaluates it (`solve_square_tiny`: the tiny inverse, then
/// `gemm_emul_large`).
pub fn solve_vec(a: &Mat4, x: &[f64; 4]) -> Result<[f64; 4], Singular> {
    let ai = inv_tiny(a).ok_or(Singular)?;
    let mut y = [0.0; 4];
    for (r, yr) in y.iter_mut().enumerate() {
        let row = [ai[0][r], ai[1][r], ai[2][r], ai[3][r]];
        *yr = dot4(&row, x);
    }
    Ok(y)
}

/// `inv(A) * B` as Armadillo evaluates it.
pub fn solve(a: &Mat4, b: &Mat4) -> Result<Mat4, Singular> {
    let ai = inv_tiny(a).ok_or(Singular)?;
    let mut c = [[0.0; 4]; 4];
    for (r, _) in (0..4).enumerate() {
        let row = [ai[0][r], ai[1][r], ai[2][r], ai[3][r]];
        for (j, col) in b.iter().enumerate() {
            c[j][r] = dot4(&row, col);
        }
    }
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determinant_and_inverse_of_a_grid() {
        let m = from_rows([
            [-3.125, 0.0, 0.0, 101.0],
            [0.0, 3.125, 0.0, -72.0],
            [0.0, 0.0, 4.0, -99.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        assert_eq!(det(&m), -3.125 * 3.125 * 4.0);
        let i = inv(&m).unwrap();
        let p = mul(&m, &i);
        for (c, col) in p.iter().enumerate() {
            for (r, v) in col.iter().enumerate() {
                let want = if r == c { 1.0 } else { 0.0 };
                assert!((v - want).abs() < 1e-14, "{r} {c} {v}");
            }
        }
    }

    #[test]
    fn lu_determinant_of_a_general_matrix() {
        let m = from_rows([
            [1e-9, 2e-9, 0.0, 0.0],
            [3e-9, 1e-9, 0.0, 0.0],
            [0.0, 0.0, 1e-9, 0.0],
            [0.0, 1e-9, 0.0, 1.0],
        ]);
        let d = det(&m);
        assert!((d - (-5e-27)).abs() < 1e-40, "{d}");
    }
}
