// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09: src/thd_mattor.c (THD_matrix_to_orientation),
// src/thd_coords.c (THD_dicom_card_xform, THD_compute_oblique_angle), src/thd_dsetatr.c (the
// obliquity update of THD_set_dataset_attributes), src/thd_niftiwrite.c (the spatial transforms
// of populate_nifti_image), the ORIENT_xyz, ORIENT_sign and ORIENT_xyzint tables of
// src/3ddata.h, the DMAT_DET and DMAT_MUL macros of src/vecmat.h, and
// src/nifti/nifti2/nifti2_io.c (nifti_dmat44_to_orientation, nifti_dmat44_to_quatern,
// nifti_dmat33_polar, nifti_dmat33_inverse, nifti_dmat33_determ, nifti_dmat33_rownorm,
// nifti_dmat33_colnorm).
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released under the GNU GPL version 2 or any later version. thd_niftiwrite.c and the NIfTI
// library were written at the NIH and are in the public domain.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! A dataset's spatial axes as AFNI holds them (`THD_dataxes`), and the geometry AFNI writes
//! back to a NIfTI header.
//!
//! AFNI keeps two descriptions of the grid:
//!
//! - **the axes**: each axis's orientation (`xxorient`, ...), signed step (`xxdel`, ...) and
//!   origin (`xxorg`, ...), floats, in DICOM (RAI) coordinates;
//! - **`ijk_to_dicom_real`**: the voxel-to-world matrix as read (float), which keeps any
//!   obliquity.
//!
//! When a dataset is written, `THD_set_dataset_attributes` replaces `ijk_to_dicom_real` by the
//! *cardinal* matrix of the axes ([`cardinal`]) whenever the matrix is plumb by AFNI's test
//! ([`is_plumb`]: nominally 0.01°, about 0.014° in float). `populate_nifti_image` then writes that matrix, with x and y negated,
//! as the sform, its quaternion ([`dmat44_to_quatern`]) as the qform, the axes' origins as the
//! qform offset and the axes' steps as the voxel sizes ([`written`]).
//!
//! Two consequences show in AFNI's output (`docs/findings/afni-tshift.md`, "Geometry"):
//! for a plumb input in metres or micrometres the sform's linear part is in millimetres (the
//! steps are converted on reading, the origins are not); and an input less than about 0.014°
//! from plumb comes out exactly plumb.
//!
//! **Upstream defect AFNI-16 (D13, decided 2026-10-12).** The second consequence discards a
//! small obliquity. By default ([`UpstreamDefects::Fix`]) larmorx-gpl keeps the input's matrix
//! wherever AFNI would replace an oblique matrix by a plumb one ([`snaps`]), and writes it as
//! AFNI writes an oblique matrix; [`UpstreamDefects::Reproduce`] writes the plumb matrix, as
//! AFNI does. A matrix with no obliquity at all (every entry off its axis zero) is written from
//! its axes in both modes, as AFNI writes it.

use larmorx_core::defects::UpstreamDefects;

/// AFNI's orientation codes (`ORI_*_TYPE`): the direction an axis points to, in DICOM
/// coordinates (+x = left, +y = posterior, +z = superior).
pub const ORI_R2L: u8 = 0;
pub const ORI_L2R: u8 = 1;
pub const ORI_P2A: u8 = 2;
pub const ORI_A2P: u8 = 3;
pub const ORI_I2S: u8 = 4;
pub const ORI_S2I: u8 = 5;

/// `ORIENT_xyzint[o] - 1`: the DICOM coordinate (0 x, 1 y, 2 z) orientation `o` runs along.
pub fn coordinate(o: u8) -> usize {
    usize::from(o / 2)
}

/// `ORIENT_sign[o] == '+'`.
pub fn positive(o: u8) -> bool {
    matches!(o, ORI_R2L | ORI_A2P | ORI_I2S)
}

/// A dataset's spatial axes (`THD_dataxes`), as `THD_open_nifti` sets them.
#[derive(Clone, Debug, PartialEq)]
pub struct Axes {
    /// `ijk_to_dicom_real`: rows x, y and z of the voxel-to-world matrix in DICOM coordinates
    /// (NIfTI's x and y negated), float.
    pub real: [[f32; 4]; 3],
    /// `xxorient`, `yyorient`, `zzorient`. `None` where `nifti_dmat44_to_orientation` fails
    /// (a degenerate sform: a zero column, or two parallel ones); AFNI then indexes its tables
    /// out of bounds, and larmorx-gpl writes the matrix as read.
    pub orient: Option<[u8; 3]>,
    /// `xxdel`, `yydel`, `zzdel`: the signed steps, in millimetres.
    pub del: [f32; 3],
    /// `xxorg`, `yyorg`, `zzorg`: the DICOM coordinate of the first voxel along each axis.
    pub org: [f32; 3],
}

/// `DMAT_DET`.
fn dmat_det(a: &[[f64; 3]; 3]) -> f64 {
    a[0][0] * a[1][1] * a[2][2] - a[0][0] * a[1][2] * a[2][1] - a[1][0] * a[0][1] * a[2][2]
        + a[1][0] * a[0][2] * a[2][1]
        + a[2][0] * a[0][1] * a[1][2]
        - a[2][0] * a[0][2] * a[1][1]
}

/// `nifti_dmat33_determ`.
fn nifti_det(r: &[[f64; 3]; 3]) -> f64 {
    let [[r11, r12, r13], [r21, r22, r23], [r31, r32, r33]] = *r;
    r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
        - r31 * r22 * r13
}

/// `DMAT_MUL` / `nifti_dmat33_mul`: `C[i][j] = A[i][0]·B[0][j] + A[i][1]·B[1][j] + A[i][2]·B[2][j]`.
fn mul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut c = [[0.0; 3]; 3];
    for (i, row) in c.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    c
}

/// The best signed permutation for the rotation `q` (the search both orientation functions
/// share): for each of rows 1, 2, 3 the column (1, 2, 3) and the sign of the permutation
/// matrix `P` whose product `P·q` has the largest trace, `P`'s determinant having `q`'s sign.
/// Returns `(ibest, jbest, kbest, pbest, qbest, rbest)`.
fn best_permutation(q: &[[f64; 3]; 3], det_q: f64, det: fn(&[[f64; 3]; 3]) -> f64) -> [i32; 6] {
    let mut vbest = -666.0;
    let mut best = [1, 2, 3, 1, 1, 1];
    for i in 1..=3 {
        for j in 1..=3 {
            if i == j {
                continue;
            }
            for k in 1..=3 {
                if i == k || j == k {
                    continue;
                }
                let mut p = [[0.0f64; 3]; 3];
                for ps in [-1, 1] {
                    for qs in [-1, 1] {
                        for rs in [-1, 1] {
                            p[0][i - 1] = f64::from(ps);
                            p[1][j - 1] = f64::from(qs);
                            p[2][k - 1] = f64::from(rs);
                            let det_p = det(&p);
                            if det_p * det_q <= 0.0 {
                                continue;
                            }
                            let m = mul(&p, q);
                            let val = m[0][0] + m[1][1] + m[2][2];
                            if val > vbest {
                                vbest = val;
                                best = [i as i32, j as i32, k as i32, ps, qs, rs];
                            }
                        }
                    }
                }
            }
        }
    }
    best
}

/// The orthonormalised columns of a 3×3 matrix, as both orientation functions build them:
/// columns i and j normalised, j made orthogonal to i when their dot product exceeds `tol`,
/// k normalised (or i × j if it is zero) and made orthogonal to i and to j. `None` where
/// `nifti_dmat44_to_orientation` gives up (`strict`): a zero i or j column, or a column that
/// becomes zero; `THD_matrix_to_orientation` substitutes unit vectors instead.
fn orthonormal(m: [[f64; 3]; 3], tol: f64, strict: bool) -> Option<[[f64; 3]; 3]> {
    let [
        [mut xi, mut xj, mut xk],
        [mut yi, mut yj, mut yk],
        [mut zi, mut zj, mut zk],
    ] = m;
    let mut val = (xi * xi + yi * yi + zi * zi).sqrt();
    if val == 0.0 {
        if strict {
            return None;
        }
        (xi, yi, zi) = (1.0, 0.0, 0.0);
    } else {
        (xi, yi, zi) = (xi / val, yi / val, zi / val);
    }
    val = (xj * xj + yj * yj + zj * zj).sqrt();
    if val == 0.0 {
        if strict {
            return None;
        }
        (xj, yj, zj) = (0.0, 1.0, 0.0);
    } else {
        (xj, yj, zj) = (xj / val, yj / val, zj / val);
    }
    val = xi * xj + yi * yj + zi * zj;
    if val.abs() > tol {
        xj -= val * xi;
        yj -= val * yi;
        zj -= val * zi;
        val = (xj * xj + yj * yj + zj * zj).sqrt();
        if strict && val == 0.0 {
            return None;
        }
        (xj, yj, zj) = (xj / val, yj / val, zj / val);
    }
    val = (xk * xk + yk * yk + zk * zk).sqrt();
    if val == 0.0 {
        (xk, yk, zk) = (yi * zj - zi * yj, zi * xj - zj * xi, xi * yj - yi * xj);
    } else {
        (xk, yk, zk) = (xk / val, yk / val, zk / val);
    }
    val = xi * xk + yi * yk + zi * zk;
    if val.abs() > tol {
        xk -= val * xi;
        yk -= val * yi;
        zk -= val * zi;
        val = (xk * xk + yk * yk + zk * zk).sqrt();
        if strict && val == 0.0 {
            return None;
        }
        (xk, yk, zk) = (xk / val, yk / val, zk / val);
    }
    val = xj * xk + yj * yk + zj * zk;
    if val.abs() > tol {
        xk -= val * xj;
        yk -= val * yj;
        zk -= val * zj;
        val = (xk * xk + yk * yk + zk * zk).sqrt();
        if strict && val == 0.0 {
            return None;
        }
        (xk, yk, zk) = (xk / val, yk / val, zk / val);
    }
    Some([[xi, xj, xk], [yi, yj, yk], [zi, zj, zk]])
}

/// `THD_matrix_to_orientation(R)`: the AFNI orientation codes of the (i, j, k) axes of a
/// matrix in DICOM (RAI) coordinates, as the signed permutation nearest to it.
pub fn matrix_to_orientation(r: [[f32; 3]; 3]) -> [u8; 3] {
    let default = [ORI_R2L, ORI_A2P, ORI_I2S];
    let m = r.map(|row| row.map(f64::from));
    let q = orthonormal(m, 1.0e-5, false).expect("not strict");
    let det_q = dmat_det(&q);
    if det_q == 0.0 {
        return default;
    }
    let [ib, jb, kb, pb, qb, rb] = best_permutation(&q, det_q, dmat_det);
    let code = |v: i32| match v {
        1 => ORI_R2L,
        -1 => ORI_L2R,
        2 => ORI_A2P,
        -2 => ORI_P2A,
        3 => ORI_I2S,
        _ => ORI_S2I,
    };
    [code(ib * pb), code(jb * qb), code(kb * rb)]
}

/// `nifti_dmat44_to_orientation(R)` followed by `THD_open_nifti`'s map to AFNI's codes
/// (`orimap`): the orientation of the (i, j, k) axes of a matrix in NIfTI (RAS) coordinates.
/// `None` where the NIfTI function fails (its codes 0).
pub fn nifti_orientation(r: [[f64; 3]; 3]) -> Option<[u8; 3]> {
    let q = orthonormal(r, 1.0e-4, true)?;
    let det_q = nifti_det(&q);
    if det_q == 0.0 {
        return None;
    }
    let [ib, jb, kb, pb, qb, rb] = best_permutation(&q, det_q, nifti_det);
    // NIFTI_L2R (1) is AFNI's L2R, NIFTI_R2L (2) R2L, P2A (3) P2A, A2P (4) A2P, I2S (5) I2S,
    // S2I (6) S2I.
    let code = |v: i32| match v {
        1 => ORI_L2R,
        -1 => ORI_R2L,
        2 => ORI_P2A,
        -2 => ORI_A2P,
        3 => ORI_I2S,
        _ => ORI_S2I,
    };
    Some([code(ib * pb), code(jb * qb), code(kb * rb)])
}

/// `MINNUM` and `MAXNUM` of `thd_coords.c` (NaN propagates as the comparisons fall).
fn min_num<T: PartialOrd + Copy>(a: T, b: T) -> T {
    if a < b { a } else { b }
}

fn max_num<T: PartialOrd + Copy>(a: T, b: T) -> T {
    if a > b { a } else { b }
}

fn min3<T: PartialOrd + Copy>(a: T, b: T, c: T) -> T {
    if min_num(a, b) < min_num(a, c) {
        min_num(a, b)
    } else {
        min_num(a, c)
    }
}

fn max3<T: PartialOrd + Copy>(a: T, b: T, c: T) -> T {
    if max_num(a, b) > max_num(a, c) {
        max_num(a, b)
    } else {
        max_num(a, c)
    }
}

/// Whether `THD_compute_oblique_angle(real)` is 0: the matrix is plumb by AFNI's test.
///
/// AFNI computes, per column, the largest absolute entry over the column's length (in float),
/// takes the smallest of the three (`fig_merit`) and reports `acos(fig_merit)` in degrees, or
/// 0 when that is not above 0.01. As `fig_merit` is a float, the angle is 0 exactly when it is
/// 1 or more, or NaN (a zero column; `acos` of a value above 1 is NaN too): the largest float
/// below 1, `1 − 2⁻²⁴`, is already `acos(1 − 2⁻²⁴) ≥ √(2⁻²³)` radians, 0.0198°, from plumb. So
/// no `acos` is needed to decide, and the effective threshold is the angle whose cosine rounds
/// to 1 in float, about 0.014° (`√(2⁻²⁴)` radians), not 0.01°.
pub fn is_plumb(real: &[[f32; 4]; 3]) -> bool {
    let column = |j: usize| {
        let (a, b, c) = (real[0][j], real[1][j], real[2][j]);
        // `sqrt(m0*m0 + m1*m1 + m2*m2)`: float products and sums, a double root, stored as float.
        let len = f64::from(a * a + b * b + c * c).sqrt() as f32;
        let largest = max3(f64::from(a).abs(), f64::from(b).abs(), f64::from(c).abs());
        (largest / f64::from(len)) as f32
    };
    let fig_merit = min3(column(0), column(1), column(2));
    // Not below 1: 1 or more, or NaN.
    fig_merit.partial_cmp(&1.0) != Some(std::cmp::Ordering::Less)
}

/// `THD_dicom_card_xform`: the cardinal (non-oblique) voxel-to-DICOM matrix of the axes, each
/// axis's step in the row of its coordinate and its origin as that row's offset. `None`
/// without an orientation.
pub fn cardinal(axes: &Axes) -> Option<[[f32; 4]; 3]> {
    let orient = axes.orient?;
    let mut t = [[0.0f32; 4]; 3];
    for (axis, &o) in orient.iter().enumerate() {
        let row = coordinate(o);
        t[row] = [0.0; 4];
        t[row][axis] = axes.del[axis];
        t[row][3] = axes.org[axis];
    }
    Some(t)
}

/// AFNI-16: whether AFNI, writing `axes`, discards an obliquity: the matrix is plumb by AFNI's
/// test ([`is_plumb`]) but has a non-zero entry off the row of some axis's coordinate, so its
/// cardinal matrix ([`cardinal`]) moves the grid. False for a matrix without such an entry
/// (whose cardinal matrix is itself, but for the steps' and signed zeros' representation), for
/// an oblique matrix (written as it is), without an orientation, and for a matrix with a
/// non-finite entry.
pub fn snaps(axes: &Axes) -> bool {
    let Some(orient) = axes.orient else {
        return false;
    };
    if !is_plumb(&axes.real) || axes.real.iter().flatten().any(|v| !v.is_finite()) {
        return false;
    }
    (0..3).any(|col| (0..3).any(|row| row != coordinate(orient[col]) && axes.real[row][col] != 0.0))
}

/// The largest angle, in degrees, between a column of the linear part of `real` and the
/// coordinate axis nearest to it (computed in double), for messages: how far from plumb the
/// matrix is.
pub fn obliquity_degrees(real: &[[f32; 4]; 3]) -> f64 {
    (0..3)
        .map(|j| {
            let col = [0, 1, 2].map(|i| f64::from(real[i][j]));
            let len = (col[0] * col[0] + col[1] * col[1] + col[2] * col[2]).sqrt();
            let largest = col.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            if len > 0.0 {
                (largest / len).min(1.0).acos().to_degrees()
            } else {
                0.0
            }
        })
        .fold(0.0, f64::max)
}

/// The spatial fields AFNI writes for a dataset ([`written`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    /// `srow_x`, `srow_y`, `srow_z`: the matrix in NIfTI (RAS) coordinates.
    pub sform: [[f64; 4]; 3],
    /// `quatern_b`, `quatern_c`, `quatern_d`.
    pub quatern: [f64; 3],
    /// `qfac` (`pixdim[0]`).
    pub qfac: f64,
    /// `qoffset_x`, `qoffset_y`, `qoffset_z`.
    pub qoffset: [f64; 3],
    /// `pixdim[1..=3]`: `|xxdel|`, `|yydel|`, `|zzdel|`.
    pub voxel: [f64; 3],
}

/// What `DSET_write` puts in the NIfTI header for `axes`: `THD_set_dataset_attributes`
/// replaces a plumb `ijk_to_dicom_real` by the cardinal matrix, then `populate_nifti_image`
/// writes it with x and y negated as the sform, its quaternion as the qform, and the axes'
/// origins and steps as the qform's offset and the voxel sizes.
///
/// AFNI-16: with [`UpstreamDefects::Fix`], a matrix whose obliquity AFNI would discard
/// ([`snaps`]) is kept and written as AFNI writes an oblique one.
pub fn written(axes: &Axes, mode: UpstreamDefects) -> Written {
    let plumb = is_plumb(&axes.real) && !(mode.fix() && snaps(axes));
    let matrix = if plumb {
        cardinal(axes).unwrap_or(axes.real)
    } else {
        axes.real
    };
    let mut sform = matrix.map(|row| row.map(f64::from));
    for row in sform.iter_mut().take(2) {
        for v in row.iter_mut() {
            *v = -*v;
        }
    }
    let (quatern, qfac) = dmat44_to_quatern(&sform);
    // `qoffset_x = -axstart[nif_x_axnum]`, `qoffset_y = -axstart[nif_y_axnum]`,
    // `qoffset_z = axstart[nif_z_axnum]`: the origin of the axis that runs along each
    // coordinate.
    let qoffset = match axes.orient {
        Some(orient) => {
            let mut q = [0.0f64; 3];
            for (axis, &o) in orient.iter().enumerate() {
                let c = coordinate(o);
                let start = f64::from(axes.org[axis]);
                q[c] = if c < 2 { -start } else { start };
            }
            q
        }
        None => [sform[0][3], sform[1][3], sform[2][3]],
    };
    Written {
        sform,
        quatern,
        qfac,
        qoffset,
        voxel: axes.del.map(|d| f64::from(d.abs())),
    }
}

/// `nifti_dmat33_inverse`. AFNI computes `1/det` in x87 extended precision (`1.0l / deti`)
/// and rounds it to double; here it is a double division, which can differ in the last bit
/// of the double (it never did for a plumb matrix, where the values are exact).
fn inverse(r: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let [[r11, r12, r13], [r21, r22, r23], [r31, r32, r33]] = *r;
    let mut deti = nifti_det(r);
    if deti != 0.0 {
        deti = 1.0 / deti;
    }
    [
        [
            deti * (r22 * r33 - r32 * r23),
            deti * (-r12 * r33 + r32 * r13),
            deti * (r12 * r23 - r22 * r13),
        ],
        [
            deti * (-r21 * r33 + r31 * r23),
            deti * (r11 * r33 - r31 * r13),
            deti * (-r11 * r23 + r21 * r13),
        ],
        [
            deti * (r21 * r32 - r31 * r22),
            deti * (-r11 * r32 + r31 * r12),
            deti * (r11 * r22 - r21 * r12),
        ],
    ]
}

/// `nifti_dmat33_rownorm`: the largest row sum of absolute values.
fn rownorm(a: &[[f64; 3]; 3]) -> f64 {
    let r = a.map(|row| row[0].abs() + row[1].abs() + row[2].abs());
    let mut r1 = r[0];
    if r1 < r[1] {
        r1 = r[1];
    }
    if r1 < r[2] {
        r1 = r[2];
    }
    r1
}

/// `nifti_dmat33_colnorm`: the largest column sum of absolute values.
fn colnorm(a: &[[f64; 3]; 3]) -> f64 {
    let c = |j: usize| a[0][j].abs() + a[1][j].abs() + a[2][j].abs();
    let mut r1 = c(0);
    if r1 < c(1) {
        r1 = c(1);
    }
    if r1 < c(2) {
        r1 = c(2);
    }
    r1
}

/// `nifti_dmat33_polar`: the orthogonal matrix nearest to `a` (Higham's iteration).
fn polar(a: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut x = *a;
    let mut gam = nifti_det(&x);
    while gam == 0.0 {
        // Perturb a singular matrix.
        gam = 0.00001 * (0.001 + rownorm(&x));
        x[0][0] += gam;
        x[1][1] += gam;
        x[2][2] += gam;
        gam = nifti_det(&x);
    }
    let mut dif = 1.0f64;
    let mut k = 0;
    loop {
        let y = inverse(&x);
        let (gam, gmi) = if dif > 0.3 {
            let alp = (rownorm(&x) * colnorm(&x)).sqrt();
            let bet = (rownorm(&y) * colnorm(&y)).sqrt();
            let gam = (bet / alp).sqrt();
            (gam, 1.0 / gam)
        } else {
            (1.0, 1.0)
        };
        let mut z = [[0.0f64; 3]; 3];
        for (i, row) in z.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = 0.5 * (gam * x[i][j] + gmi * y[j][i]);
            }
        }
        dif = 0.0;
        for i in 0..3 {
            for j in 0..3 {
                dif += (z[i][j] - x[i][j]).abs();
            }
        }
        k += 1;
        if k > 100 || dif < 3.0e-6 {
            return z;
        }
        x = z;
    }
}

/// `nifti_dmat44_to_quatern(R)`: the quaternion parameters `(b, c, d)` and `qfac` of the
/// rotation nearest to `R`'s upper 3×3 corner.
///
/// AFNI forms some quotients in x87 extended precision (`0.25l * (r32-r23) / a`) and rounds
/// them to double; here they are double divisions. The two can differ in the last bit of the
/// double for an oblique matrix (never for a plumb one, whose quotients are exact), which the
/// NIfTI-1 header, storing the parameters as float, almost never shows.
pub fn dmat44_to_quatern(r: &[[f64; 4]; 3]) -> ([f64; 3], f64) {
    let [mut r11, mut r12, mut r13] = [r[0][0], r[0][1], r[0][2]];
    let [mut r21, mut r22, mut r23] = [r[1][0], r[1][1], r[1][2]];
    let [mut r31, mut r32, mut r33] = [r[2][0], r[2][1], r[2][2]];
    let mut xd = (r11 * r11 + r21 * r21 + r31 * r31).sqrt();
    let mut yd = (r12 * r12 + r22 * r22 + r32 * r32).sqrt();
    let mut zd = (r13 * r13 + r23 * r23 + r33 * r33).sqrt();
    if xd == 0.0 {
        (r11, r21, r31, xd) = (1.0, 0.0, 0.0, 1.0);
    }
    if yd == 0.0 {
        (r22, r12, r32, yd) = (1.0, 0.0, 0.0, 1.0);
    }
    if zd == 0.0 {
        (r33, r13, r23, zd) = (1.0, 0.0, 0.0, 1.0);
    }
    (r11, r21, r31) = (r11 / xd, r21 / xd, r31 / xd);
    (r12, r22, r32) = (r12 / yd, r22 / yd, r32 / yd);
    (r13, r23, r33) = (r13 / zd, r23 / zd, r33 / zd);
    let p = polar(&[[r11, r12, r13], [r21, r22, r23], [r31, r32, r33]]);
    [[r11, r12, r13], [r21, r22, r23], [r31, r32, r33]] = p;
    let det =
        r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
            - r31 * r22 * r13;
    let qfac = if det > 0.0 {
        1.0
    } else {
        (r13, r23, r33) = (-r13, -r23, -r33);
        -1.0
    };
    let mut a = r11 + r22 + r33 + 1.0;
    let (b, c, d);
    if a > 0.5 {
        a = 0.5 * a.sqrt();
        b = 0.25 * (r32 - r23) / a;
        c = 0.25 * (r13 - r31) / a;
        d = 0.25 * (r21 - r12) / a;
    } else {
        let xd = 1.0 + r11 - (r22 + r33);
        let yd = 1.0 + r22 - (r11 + r33);
        let zd = 1.0 + r33 - (r11 + r22);
        let (mut bb, mut cc, mut dd);
        if xd > 1.0 {
            bb = 0.5 * xd.sqrt();
            cc = 0.25 * (r12 + r21) / bb;
            dd = 0.25 * (r13 + r31) / bb;
            a = 0.25 * (r32 - r23) / bb;
        } else if yd > 1.0 {
            cc = 0.5 * yd.sqrt();
            bb = 0.25 * (r12 + r21) / cc;
            dd = 0.25 * (r23 + r32) / cc;
            a = 0.25 * (r13 - r31) / cc;
        } else {
            dd = 0.5 * zd.sqrt();
            bb = 0.25 * (r13 + r31) / dd;
            cc = 0.25 * (r23 + r32) / dd;
            a = 0.25 * (r21 - r12) / dd;
        }
        if a < 0.0 {
            (bb, cc, dd) = (-bb, -cc, -dd);
        }
        (b, c, d) = (bb, cc, dd);
    }
    ([b, c, d], qfac)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plumb axial grid: i to the left, j posterior, k superior (DICOM).
    fn axial(del: [f32; 3], org: [f32; 3]) -> Axes {
        Axes {
            real: [
                [del[0], 0.0, 0.0, org[0]],
                [0.0, del[1], 0.0, org[1]],
                [0.0, 0.0, del[2], org[2]],
            ],
            orient: Some([ORI_R2L, ORI_A2P, ORI_I2S]),
            del,
            org,
        }
    }

    #[test]
    fn orientation_of_plumb_and_permuted_matrices() {
        let r = [[3.0f32, 0.0, 0.0], [0.0, -3.0, 0.0], [0.0, 0.0, 4.0]];
        assert_eq!(matrix_to_orientation(r), [ORI_R2L, ORI_P2A, ORI_I2S]);
        // A sagittal grid: i along y, j along z, k along x.
        let r = [[0.0f32, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert_eq!(matrix_to_orientation(r), [ORI_A2P, ORI_I2S, ORI_L2R]);
        // NIfTI (RAS): +x is right, so an increasing x axis runs left to right.
        let n = [[3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 4.0]];
        assert_eq!(nifti_orientation(n), Some([ORI_L2R, ORI_P2A, ORI_I2S]));
        assert_eq!(nifti_orientation([[0.0; 3]; 3]), None);
    }

    #[test]
    fn the_obliquity_threshold() {
        // The smallest angle a float fig_merit below 1 gives is above 0.01°.
        let smallest = (2.0f64.powi(-23)).sqrt().to_degrees();
        assert!(smallest > 0.01, "{smallest}");
        let plumb = axial([3.0, 3.0, 4.0], [1.0, 2.0, 3.0]);
        assert!(is_plumb(&plumb.real));
        let mut tilted = plumb.real;
        tilted[1][0] = 1.0e-4; // 0.002°: still plumb for AFNI
        assert!(is_plumb(&tilted));
        tilted[1][0] = 1.0e-3; // 0.019°
        assert!(!is_plumb(&tilted));
        assert!(
            is_plumb(&[[0.0; 4]; 3]),
            "a zero column counts as plumb (NaN)"
        );
    }

    #[test]
    fn plumb_grids_are_written_from_their_axes() {
        // Steps converted from metres, origins not: AFNI's metre-unit output (verified).
        let mut axes = axial([-3.0, -3.0, 4.0], [0.1, -0.05, 0.02]);
        axes.real = [
            [-0.003, 0.0, 0.0, 0.1],
            [0.0, -0.003, 0.0, -0.05],
            [0.0, 0.0, 0.004, 0.02],
        ];
        axes.orient = Some([ORI_L2R, ORI_P2A, ORI_I2S]);
        let w = written(&axes, UpstreamDefects::Reproduce);
        let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(
            bits(&w.sform[0]),
            bits(&[3.0, -0.0, -0.0, f64::from(-0.1f32)])
        );
        assert_eq!(
            bits(&w.sform[1]),
            bits(&[-0.0, 3.0, -0.0, f64::from(0.05f32)])
        );
        assert_eq!(
            bits(&w.sform[2]),
            bits(&[0.0, 0.0, 4.0, f64::from(0.02f32)])
        );
        assert_eq!(bits(&w.quatern), bits(&[0.0, -0.0, 0.0]));
        assert_eq!(w.qfac, 1.0);
        assert_eq!(w.voxel, [3.0, 3.0, 4.0]);
        assert_eq!(
            w.qoffset,
            [f64::from(-0.1f32), f64::from(0.05f32), f64::from(0.02f32)]
        );
        // No obliquity to discard: the default writes the same (AFNI-16 does not apply).
        assert!(!snaps(&axes));
        assert_eq!(written(&axes, UpstreamDefects::Fix), w);
    }

    #[test]
    fn nearly_plumb_grids_keep_their_matrix_by_default() {
        // 1e-4 rad (0.0057°) about y: plumb for AFNI, which writes the cardinal matrix
        // (AFNI-16); the default keeps the matrix, as AFNI writes an oblique one.
        let mut axes = axial([3.0, 3.0, 4.0], [-10.0, 20.0, 30.0]);
        axes.real[2][0] = 3.0e-4;
        axes.real[0][2] = -4.0e-4;
        assert!(is_plumb(&axes.real) && snaps(&axes));
        let afni = written(&axes, UpstreamDefects::Reproduce);
        assert_eq!(afni.sform[2][0], 0.0);
        assert_eq!(afni.sform[0][2], -0.0);
        // 180° about z in NIfTI's coordinates.
        assert_eq!(afni.quatern, [0.0, 0.0, 1.0]);
        let kept = written(&axes, UpstreamDefects::Fix);
        assert_eq!(kept.sform[2][0], f64::from(3.0e-4f32));
        assert_eq!(kept.sform[0][2], f64::from(4.0e-4f32));
        // The same, tilted by 1e-4 rad about y: b = sin(0.5e-4).
        assert!((kept.quatern[0] - 5.0e-5).abs() < 1.0e-9, "{kept:?}");
        assert_eq!((kept.qoffset, kept.voxel), (afni.qoffset, afni.voxel));
        let degrees = obliquity_degrees(&axes.real);
        assert!(
            (degrees - 1.0e-4f64.to_degrees()).abs() < 1.0e-6,
            "{degrees}"
        );
        // Beyond AFNI's threshold the matrix is kept in both modes.
        axes.real[2][0] = 1.0e-3;
        assert!(!is_plumb(&axes.real) && !snaps(&axes));
        assert_eq!(
            written(&axes, UpstreamDefects::Fix),
            written(&axes, UpstreamDefects::Reproduce)
        );
    }

    #[test]
    fn oblique_grids_keep_their_matrix() {
        let (c, s) = (0.2f64.cos() as f32, 0.2f64.sin() as f32);
        let axes = Axes {
            real: [
                [3.0 * c, 0.0, -4.0 * s, -10.0],
                [0.0, 3.0, 0.0, 20.0],
                [3.0 * s, 0.0, 4.0 * c, 30.0],
            ],
            orient: Some([ORI_R2L, ORI_A2P, ORI_I2S]),
            del: [3.0, 3.0, 4.0],
            org: [-10.0, 20.0, 30.0],
        };
        let w = written(&axes, UpstreamDefects::Fix);
        assert_eq!(w.sform[0][0], -f64::from(3.0 * c));
        assert_eq!(w.sform[2][0], f64::from(3.0 * s));
        // In NIfTI's coordinates (x and y negated) the grid is turned by 180° about z and by
        // 0.2 rad about y: the quaternion (b, c, d) = (sin 0.1, 0, cos 0.1).
        let (sin, cos) = (0.0998334166468282, 0.9950041652780258);
        assert!((w.quatern[0] - sin).abs() < 1e-6, "{w:?}");
        assert_eq!(w.quatern[1], 0.0, "{w:?}");
        assert!((w.quatern[2] - cos).abs() < 1e-6, "{w:?}");
        assert_eq!((w.qfac, w.qoffset), (1.0, [10.0, -20.0, 30.0]));
    }

    #[test]
    fn quaternions_of_rotations() {
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        assert_eq!(dmat44_to_quatern(&identity), ([0.0; 3], 1.0));
        // 180° about z: the "trickier case".
        let flip = [
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        assert_eq!(dmat44_to_quatern(&flip), ([0.0, 0.0, 1.0], 1.0));
        let mirror = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
        ];
        assert_eq!(dmat44_to_quatern(&mirror).1, -1.0);
    }
}
