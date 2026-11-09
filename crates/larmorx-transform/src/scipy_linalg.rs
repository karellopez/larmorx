// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! `scipy.linalg.inv` of a 4×4 float64 matrix, as SciPy 1.18.1 and 1.15.3 compute it with the
//! OpenBLAS their wheels bundle (0.3.31.dev and 0.3.28, `DYNAMIC_ARCH`) on its **Haswell**
//! kernels (`OPENBLAS_CORETYPE=Haswell`, the order this project pins: `docs/findings/numpy-blas.md`).
//!
//! nilearn's `resample_img` inverts the image's affine with it (`np.dot(linalg.inv(affine),
//! target_affine)`); niworkflows' `GenerateSamplingReference` and sdcflows' SyN preprocessing
//! reach it there.
//!
//! - **SciPy 1.15.3** (`scipy/linalg/_basic.py`, `inv`): LAPACK `getrf` (OpenBLAS's own:
//!   `getrf_single`, which for this size is the unblocked `getf2_k`, the same as numpy's
//!   [`crate::openblas::inv4`] takes), then `getri` with `lwork = int(1.01 * 256)`. `dgetri` is
//!   the reference LAPACK's Fortran (`lapack-netlib/SRC/dgetri.f`): OpenBLAS's own `dtrtri`
//!   (`interface/lapack/trtri.c`: `trtri_U_single`, here `trti2_U` with `trmv_NUN`'s `daxpy` and
//!   `dscal` tails), then the unblocked loop (the block size 64 is at least n) with `dgemv`
//!   (`dgemv_n_4.c` for four rows: the Haswell 4 × 2 kernel, `dgemv_n_microk_haswell-4.c`, sums
//!   two products and adds them with one fused multiply-add by alpha; the SSE2 4 × 1 kernel
//!   multiplies x by alpha first), then the column interchanges (`dswap`).
//! - **SciPy 1.18.1** (`_basic.py`, `src/_linalg_inv.hh`, `_common_array_utils.hh`): the matrix
//!   is copied to column-major order and its structure detected (`bandwidth`, `is_sym_or_herm`):
//!   a **diagonal** matrix has its diagonal replaced by `1 / a` (C++); an **upper** or **lower
//!   triangular** one goes to OpenBLAS's `dtrtri` (`trti2_U`, or `trti2_L` with `trmv_NLN`), its
//!   other triangle set to +0; a **general** one to `getrf`, `gecon` (the condition estimate,
//!   which changes nothing but the warning) and `getri`, as 1.15. A **symmetric** one (every
//!   non-diagonal, non-triangular `m == m.T`) goes to Cholesky (`potrf`, `pocon`, `potri`) and,
//!   when that fails, to `sytrf`/`sycon`/`sytri`: not ported ([`Outcome::SymmetricNotPorted`];
//!   an image affine is symmetric only with a zero translation).
//! - **The warning** 1.18 raises for an ill-conditioned matrix (`rcond < eps`): exact for the
//!   diagonal path (`max|a| · max|1/a| > 1/eps`, the value SciPy prints; SCI-20 in
//!   `larmorx.transforms._linalg`); for the other paths it depends on LAPACK's condition
//!   estimate (`dgecon`, `dtrcon`: Hager's estimator with `dlatrs`), which is not ported. The
//!   estimate of `‖A⁻¹‖₁` is at most its true value, so `rcond ≥ 1 / (‖A‖ · ‖A⁻¹‖₁)` computed
//!   from the inverse; when that bound is at least `1000 · eps` there is no warning
//!   ([`Condition::WellConditioned`]), else [`Condition::EstimateNeeded`].
//! - 1.18's `gecon` returns `info = -5` without estimating when `‖A‖` (SciPy's `norm1_`, the
//!   largest row sum of the column-major copy) overflows, and the inverse is then never
//!   computed: SciPy returns the LU factors ([`Outcome::NormOverflow`]; SCI-18).
//!
//! Upstream files: SciPy 1.18.1 and 1.15.3 (BSD-3-Clause) as named; OpenBLAS 0.3.30 release
//! tarball (the kernels on these paths are those of 0.3.28 and 0.3.31.dev; validated against
//! both SciPy wheels: `validation/investigations/sdcflows-fieldmap-b/check_scipy_inv.py`):
//! `interface/lapack/getrf.c`, `trtri.c`, `interface/gemv.c`, `lapack/getrf/getrf_single.c`,
//! `lapack/getf2/getf2_k.c`, `lapack/trtri/trtri_U_single.c`, `trtri_L_single.c`,
//! `lapack/trti2/trti2_U.c`, `trti2_L.c`, `driver/level2/trmv_U.c`, `trmv_L.c`,
//! `kernel/x86_64/daxpy.c`, `dscal.c`, `dgemv_n_4.c`, `dgemv_n_microk_haswell-4.c`,
//! `KERNEL.HASWELL`; the reference LAPACK in `lapack-netlib/SRC/dgetri.f`, `dgecon.f`. The
//! logic is re-expressed in Rust; no code was copied. OpenBLAS's notice is in
//! [`crate::openblas`] and NOTICE; SciPy's (BSD-3-Clause, Copyright (c) 2001-2002 Enthought, Inc.
//! 2003, SciPy Developers) in NOTICE.

use larmorx_core::linalg::Mat4;

use crate::openblas::getf2;

/// The SciPy release whose `linalg.inv` is followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScipyRelease {
    /// SciPy 1.15 (1.15.2 in fMRIPrep 25.2.6's image, 1.15.3 in the shared environment):
    /// `getrf` + `getri` for every matrix, no condition warning.
    V1_15,
    /// SciPy 1.18.1 (the newest): structure detection, condition warnings.
    V1_18,
}

/// The structure SciPy 1.18 detects (1.15 treats every matrix as general).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Structure {
    Diagonal,
    UpperTriangular,
    LowerTriangular,
    Symmetric,
    General,
}

impl Structure {
    /// The name used in messages and by the Python binding.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Structure::Diagonal => "diagonal",
            Structure::UpperTriangular => "upper triangular",
            Structure::LowerTriangular => "lower triangular",
            Structure::Symmetric => "symmetric",
            Structure::General => "general",
        }
    }
}

/// What 1.18 decides about its ill-conditioned warning (1.15 never warns).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Condition {
    /// No warning (exact for the diagonal path; proved by the bound for the others).
    WellConditioned,
    /// The diagonal path's warning, with the value SciPy prints as `rcond` (the product
    /// `max|a| · max|1/a|`).
    IllConditioned(f64),
    /// The warning depends on LAPACK's condition estimate, not ported (see the module).
    EstimateNeeded,
}

/// The result of [`inv4`].
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// The inverse (row-major) and the warning decision.
    Inverse(Mat4, Condition),
    /// SciPy raises `LinAlgError` (an exactly zero pivot or diagonal element).
    Singular,
    /// SCI-18 (1.18, general path): `‖A‖` overflows, `gecon` returns `info = -5` and SciPy
    /// returns `lu`, the LU factors; `inverse` is what `getri` gives (1.15's result).
    NormOverflow { lu: Mat4, inverse: Mat4 },
    /// 1.18's symmetric path (Cholesky, Bunch-Kaufman): not ported.
    SymmetricNotPorted,
}

/// `scipy.linalg.inv(m)` of a finite 4×4 matrix (SciPy's `check_finite` refuses others before
/// this point; the caller checks), with the structure 1.18 detects ([`Structure::General`] for
/// 1.15).
#[must_use]
pub fn inv4(m: &Mat4, release: ScipyRelease) -> (Structure, Outcome) {
    // Column-major copy (f2py's Fortran copy in 1.15, `swap_cf` in 1.18): a[c][r] = m[r][c].
    let a: [[f64; 4]; 4] = std::array::from_fn(|c| std::array::from_fn(|r| m[r][c]));
    let structure = match release {
        ScipyRelease::V1_15 => Structure::General,
        ScipyRelease::V1_18 => detect(m),
    };
    let outcome = match (release, structure) {
        (ScipyRelease::V1_15, _) => match general(a) {
            General::Inverse(inv) => Outcome::Inverse(row_major(&inv), Condition::WellConditioned),
            General::Singular => Outcome::Singular,
        },
        (_, Structure::Diagonal) => diagonal(a),
        (_, Structure::UpperTriangular | Structure::LowerTriangular) => {
            triangular(a, m, structure == Structure::UpperTriangular)
        }
        (_, Structure::Symmetric) => Outcome::SymmetricNotPorted,
        (_, Structure::General) => {
            let anorm = norm1_rows(&a);
            let mut lu = a;
            let mut ipiv = [0usize; 4];
            if getf2(&mut lu, &mut ipiv) != 0 {
                return (structure, Outcome::Singular);
            }
            let mut inv = lu;
            getri(&mut inv, &ipiv);
            if anorm.is_infinite() {
                // dgecon: ANORM > HUGEVAL -> INFO = -5, so SciPy skips getri.
                return (
                    structure,
                    Outcome::NormOverflow {
                        lu: row_major(&lu),
                        inverse: row_major(&inv),
                    },
                );
            }
            let inv = row_major(&inv);
            Outcome::Inverse(inv, bound(anorm, &inv))
        }
    };
    (structure, outcome)
}

/// The structure 1.18's `bandwidth` and `is_sym_or_herm` find (exact comparisons with zero:
/// -0 counts as zero).
fn detect(m: &Mat4) -> Structure {
    let lower = (0..4).any(|c| (c + 1..4).any(|r| m[r][c] != 0.0));
    let upper = (0..4).any(|c| (0..c).any(|r| m[r][c] != 0.0));
    match (lower, upper) {
        (false, false) => Structure::Diagonal,
        (false, true) => Structure::UpperTriangular,
        (true, false) => Structure::LowerTriangular,
        (true, true) => {
            if (0..4).all(|i| (0..4).all(|j| m[i][j] == m[j][i])) {
                Structure::Symmetric
            } else {
                Structure::General
            }
        }
    }
}

fn row_major(a: &[[f64; 4]; 4]) -> Mat4 {
    std::array::from_fn(|r| std::array::from_fn(|c| a[c][r]))
}

enum General {
    Inverse([[f64; 4]; 4]),
    Singular,
}

/// `getrf` then `getri` on the column-major copy.
fn general(mut a: [[f64; 4]; 4]) -> General {
    let mut ipiv = [0usize; 4];
    if getf2(&mut a, &mut ipiv) != 0 {
        return General::Singular;
    }
    getri(&mut a, &ipiv);
    General::Inverse(a)
}

/// SciPy's `norm1_` on the column-major copy: `rwork[j] = |a(row j, col 0)|`, then `+=` the
/// other columns in order; the largest by `>` from 0. It sums rows: the infinity norm.
fn norm1_rows(a: &[[f64; 4]; 4]) -> f64 {
    let mut rwork: [f64; 4] = std::array::from_fn(|j| a[0][j].abs());
    for col in a.iter().skip(1) {
        for (w, v) in rwork.iter_mut().zip(col.iter()) {
            *w += v.abs();
        }
    }
    rwork.iter().fold(0.0, |t, &w| if w > t { w } else { t })
}

/// The largest column sum of `|m|` (row-major input).
fn norm1_columns(m: &Mat4) -> f64 {
    (0..4)
        .map(|c| (0..4).map(|r| m[r][c].abs()).sum::<f64>())
        .fold(0.0, f64::max)
}

/// The warning decision from the bound `rcond >= 1 / (anorm · ‖A⁻¹‖₁)` (see the module).
fn bound(anorm: f64, inverse: &Mat4) -> Condition {
    let lower = (1.0 / norm1_columns(inverse)) / anorm;
    if lower >= 1000.0 * f64::EPSILON {
        Condition::WellConditioned
    } else {
        Condition::EstimateNeeded
    }
}

/// 1.18's `invert_slice_diagonal`: `1 / a(j, j)` in order, stopping at a zero element; the
/// condition is `max|a| · max|1/a| > 1/eps`.
#[allow(clippy::needless_range_loop)]
fn diagonal(mut a: [[f64; 4]; 4]) -> Outcome {
    let (mut maxa, mut maxinva) = (0.0f64, 0.0f64);
    for j in 0..4 {
        let ajj = a[j][j];
        if ajj == 0.0 {
            return Outcome::Singular;
        }
        let inv = 1.0 / ajj;
        a[j][j] = inv;
        if ajj.abs() > maxa {
            maxa = ajj.abs();
        }
        if inv.abs() > maxinva {
            maxinva = inv.abs();
        }
    }
    let product = maxa * maxinva;
    let condition = if product > 1.0 / f64::EPSILON {
        Condition::IllConditioned(product)
    } else {
        Condition::WellConditioned
    };
    Outcome::Inverse(row_major(&a), condition)
}

/// 1.18's `invert_slice_triangular`: OpenBLAS's `dtrtri` (a zero diagonal element is singular:
/// its `AMIN_K` check), then `zero_other_triangle` (+0). `m` is the row-major input.
fn triangular(mut a: [[f64; 4]; 4], m: &Mat4, upper: bool) -> Outcome {
    if (0..4).any(|j| a[j][j] == 0.0) {
        return Outcome::Singular;
    }
    if upper {
        trti2_upper(&mut a);
    } else {
        trti2_lower(&mut a);
    }
    for (c, col) in a.iter_mut().enumerate() {
        for (r, v) in col.iter_mut().enumerate() {
            if (upper && r > c) || (!upper && r < c) {
                *v = 0.0;
            }
        }
    }
    let inv = row_major(&a);
    // dtrcon on the inverse: rcond = (1 / ‖A⁻¹‖₁) / est‖A‖₁, est ≤ ‖A‖₁.
    Outcome::Inverse(inv, bound(norm1_columns(m), &inv))
}

/// OpenBLAS's `trti2_U` (non-unit) on a column-major 4×4 upper triangle, in place: for each
/// column, the reciprocal of its diagonal, `trmv_NUN` of the leading block (already inverted)
/// with the column (`daxpy`'s scalar tail `y += da·x`, then `x[i] *= a(i, i)`), and `dscal` by
/// `-1 / a(j, j)`. The C kernels round every product and sum (no FMA).
// The loops keep OpenBLAS's indices, so the port can be checked line by line.
#[allow(clippy::needless_range_loop)]
fn trti2_upper(a: &mut [[f64; 4]; 4]) {
    for j in 0..4 {
        let ajj = 1.0 / a[j][j];
        a[j][j] = ajj;
        for i in 0..j {
            let xi = a[j][i];
            for k in 0..i {
                a[j][k] += xi * a[i][k];
            }
            a[j][i] = xi * a[i][i];
        }
        let s = -ajj;
        for k in 0..j {
            a[j][k] *= s;
        }
    }
}

/// OpenBLAS's `trti2_L` (non-unit), from the last column: the reciprocal of the diagonal,
/// `trmv_NLN` of the trailing block with the column below the diagonal (from its last element
/// up: `daxpy` into the elements below, then `x[e] *= a(e, e)`), and `dscal` by `-1 / a(j, j)`.
#[allow(clippy::needless_range_loop)]
fn trti2_lower(a: &mut [[f64; 4]; 4]) {
    for j in (0..4).rev() {
        let ajj = 1.0 / a[j][j];
        a[j][j] = ajj;
        let m = 3 - j;
        for i in 0..m {
            let e = j + m - i;
            let xe = a[j][e];
            if i > 0 {
                for k in e + 1..4 {
                    a[j][k] += xe * a[e][k];
                }
            }
            a[j][e] = xe * a[e][e];
        }
        let s = -ajj;
        for k in j + 1..4 {
            a[j][k] *= s;
        }
    }
}

/// The reference `dgetri` (unblocked: the block size 64 is at least 4) on `getf2`'s factors,
/// column-major, in place: `dtrtri` of U, then from the last column `inv(A)·L = inv(U)`
/// solved by `dgemv('N', 4, 3 - j, -1, A(:, j+1..), work(j+1..), 1, A(:, j))`, then the
/// column interchanges from the second-last pivot down.
#[allow(clippy::needless_range_loop)]
fn getri(a: &mut [[f64; 4]; 4], ipiv: &[usize; 4]) {
    trti2_upper(a);
    let mut work = [0.0f64; 4];
    for j in (0..4).rev() {
        for i in j + 1..4 {
            work[i] = a[j][i];
            a[j][i] = 0.0;
        }
        let k = 3 - j;
        let mut c = j + 1;
        if k & 2 != 0 {
            // dgemv_kernel_4x2 (Haswell): (a0·x0 + a1·x1), then y = fma(alpha, t, y).
            let (x0, x1) = (work[c], work[c + 1]);
            for r in 0..4 {
                let t = a[c][r] * x0 + a[c + 1][r] * x1;
                a[j][r] = (-1.0f64).mul_add(t, a[j][r]);
            }
            c += 2;
        }
        if k & 1 != 0 {
            // dgemv_kernel_4x1 (SSE2): x0·alpha, then y + a·x0.
            // The kernel's `mulsd` by alpha (a NaN keeps its sign, as `-x` would not).
            #[allow(clippy::neg_multiply)]
            let x0 = work[c] * -1.0;
            for r in 0..4 {
                a[j][r] += a[c][r] * x0;
            }
        }
    }
    for j in (0..3).rev() {
        let jp = ipiv[j];
        if jp != j {
            a.swap(j, jp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Mat4 = [
        [0.98, 0.07, -0.02, 2.5],
        [-0.06, 1.01, 0.05, -1.75],
        [0.03, -0.04, 0.99, 3.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    fn product(a: &Mat4, b: &Mat4) -> Mat4 {
        std::array::from_fn(|i| std::array::from_fn(|j| (0..4).map(|k| a[i][k] * b[k][j]).sum()))
    }

    fn near_identity(p: &Mat4, tol: f64) -> bool {
        (0..4).all(|i| (0..4).all(|j| (p[i][j] - if i == j { 1.0 } else { 0.0 }).abs() <= tol))
    }

    fn inverse(m: &Mat4, release: ScipyRelease) -> (Structure, Mat4) {
        match inv4(m, release) {
            (s, Outcome::Inverse(x, _)) => (s, x),
            other => panic!("not an inverse: {other:?}"),
        }
    }

    #[test]
    fn every_structure_inverts() {
        for release in [ScipyRelease::V1_15, ScipyRelease::V1_18] {
            let (s, x) = inverse(&A, release);
            assert_eq!(s, Structure::General);
            assert!(near_identity(&product(&A, &x), 1e-15));
        }
        // A plumb affine is upper triangular for 1.18 (and general for 1.15).
        let plumb: Mat4 = [
            [-2.0, 0.0, 0.0, 90.0],
            [0.0, 2.0, 0.0, -126.0],
            [0.0, 0.0, 2.5, -72.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let (s, x) = inverse(&plumb, ScipyRelease::V1_18);
        assert_eq!(s, Structure::UpperTriangular);
        assert!(near_identity(&product(&plumb, &x), 0.0));
        let (_, y) = inverse(&plumb, ScipyRelease::V1_15);
        assert!(near_identity(&product(&plumb, &y), 0.0));
        let lower: Mat4 = [
            [2.0, 0.0, 0.0, 0.0],
            [0.5, 1.5, 0.0, 0.0],
            [-0.25, 0.75, 3.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let (s, x) = inverse(&lower, ScipyRelease::V1_18);
        assert_eq!(s, Structure::LowerTriangular);
        assert!(near_identity(&product(&lower, &x), 1e-15));
        assert!((0..4).all(|r| (r + 1..4).all(|c| x[r][c].to_bits() == 0)));
        let diag: Mat4 = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                if i == j {
                    [2.0, -4.0, 0.5, 1.0][i]
                } else {
                    0.0
                }
            })
        });
        let (s, x) = inverse(&diag, ScipyRelease::V1_18);
        assert_eq!(s, Structure::Diagonal);
        assert_eq!(x[1][1], -0.25);
    }

    #[test]
    fn singular_symmetric_and_overflow() {
        let mut s = A;
        s[2] = s[0];
        for release in [ScipyRelease::V1_15, ScipyRelease::V1_18] {
            assert_eq!(inv4(&s, release).1, Outcome::Singular);
        }
        let mut d = [[0.0; 4]; 4];
        d[0][0] = 1.0;
        d[1][1] = 2.0;
        d[3][3] = 1.0;
        assert_eq!(inv4(&d, ScipyRelease::V1_18).1, Outcome::Singular);
        let sym: Mat4 = [
            [0.0, 1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert_eq!(
            inv4(&sym, ScipyRelease::V1_18),
            (Structure::Symmetric, Outcome::SymmetricNotPorted)
        );
        let mut big = A;
        big[0][3] = f64::MAX;
        big[0][0] = f64::MAX;
        assert!(matches!(
            inv4(&big, ScipyRelease::V1_18).1,
            Outcome::NormOverflow { .. }
        ));
        // The diagonal warning: max|a| · max|1/a| > 1/eps.
        let mut ill = d;
        ill[2][2] = 1e-17;
        match inv4(&ill, ScipyRelease::V1_18).1 {
            Outcome::Inverse(_, Condition::IllConditioned(c)) => assert!(c > 1e16),
            other => panic!("{other:?}"),
        }
    }
}
