// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `CostFunctions` (queue 3.2-02, `fs.robust` 1 of 2): the image moments of Martin Reuter's
//! robust registration that both `mri_robust_register` and `mri_robust_template` reach (the
//! registration's initial alignment), translated from FreeSurfer 8.2.0's
//! `mri_robust_register/CostFunctions.cpp`: [`centroid`] and [`orientation`].
//!
//! The cost functions proper (least squares, Tukey's biweight, the correlations, the mutual
//! information, `SBCost`) serve only `mri_robust_register`'s Powell registration (`--cost` other
//! than the robust default) and come with that program's item (3.2-13); FreeSurfer's other
//! statistics in the file (`mean`, `var`, `median`, `mad`, `moment`, ...) have no live caller in
//! either program.
//!
//! Defects (`larmorx_core::defects`):
//! - **FS-966** (memory): [`orientation`] sorts the three eigenvalues by finding the largest and
//!   the smallest; when no eigenvalue is above another (all equal, or NaN) both are the first,
//!   and the middle one's index comes out as 3: FreeSurfer reads a fourth column of the 3 x 3
//!   eigenvector matrix (two of its numbers are the first column's, the third lies past the
//!   matrix). By default the eigenvectors keep EISPACK's order (0, 1, 2); `reproduce` reads the
//!   two numbers and fills the one past the matrix with 0 (noted).
//! - **FS-967** (rule B): [`orientation`] of an image with an infinite or NaN voxel has no
//!   covariance; VNL's eigensystem then never returns (FreeSurfer hangs, ITK-115) or stops
//!   without eigenvectors (`vnl_symmetric_eigensystem: ierr = <i>`, the vectors memory never
//!   written). By default a clear error and the end of the program (exit 1, as [`centroid`] ends
//!   on such an image); `reproduce` gives FreeSurfer's result where VNL returns with eigenvectors
//!   (the same error where it hangs or leaves them unwritten).

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_966, FS_967, ITK_115};
use larmorx_core::vnl_eigen::symmetric_eigensystem;
use larmorx_core::vnl_fixed;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::sse::{add64, div64, mul, mul64, sub64};
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

use crate::mymri::wide;

/// `CostFunctions::centroid(i)`: the centre of mass of frame 0 in 1-based voxel coordinates,
/// `(sum (w + 1) v / sum v, ...)`, a voxel within `outside_val / 255` of `outside_val` counted as 0
/// (`fabs(v - outside_val) < eps`; never with an outside value of 0). When the sum of the three is
/// NaN: `CostFunctions::centroid is NAN (empty image? n = <n> )` on standard error and `exit(1)`.
pub fn centroid(g: &mut Globals, err: &mut dyn Write, i: &Mri) -> Result<[f64; 3], Fatal> {
    let mut dd = [0.0f64; 3];
    let mut n = 0.0f64;
    let ov = i.outside_val;
    let eps = div64(ov, 255.0);
    for d in 0..i.vg.depth {
        for h in 0..i.vg.height {
            for w in 0..i.vg.width {
                let mut val = wide(mri_get_vox_val(g, err, i, w, h, d, 0, 0)?);
                if sub64(val, ov).abs() < eps {
                    val = 0.0;
                }
                n = add64(n, val);
                dd[0] = add64(dd[0], mul64(f64::from(w + 1), val));
                dd[1] = add64(dd[1], mul64(f64::from(h + 1), val));
                dd[2] = add64(dd[2], mul64(f64::from(d + 1), val));
            }
        }
    }
    for x in &mut dd {
        *x = div64(*x, n);
    }
    if add64(add64(dd[0], dd[1]), dd[2]).is_nan() {
        let _ = writeln!(
            err,
            "CostFunctions::centroid is NAN (empty image? n = {} )",
            cfmt::ostream(n)
        );
        return Err(Fatal::Exit(1));
    }
    Ok(dd)
}

/// `CostFunctions::orientation(i)`: the principal axes of frame 0's intensity distribution. The
/// moments in `double` from `float` terms (`n += v`; `M_100 += (w + 1) v`, `M_200 += (w + 1)^2 v`,
/// `M_110 += (w + 1)(h + 1) v`, ...: the integer coordinate products times the `float` voxel in
/// `float`); the covariance `M_ij / n - mean_i mean_j` (symmetric); VNL's symmetric eigensystem;
/// its eigenvectors as columns from the largest eigenvalue to the smallest; the first column
/// negated when the determinant is negative. FS-966, FS-967.
pub fn orientation(g: &mut Globals, err: &mut dyn Write, i: &Mri) -> Result<[[f64; 3]; 3], Fatal> {
    let mut dd = [0.0f64; 3];
    let mut n = 0.0f64;
    let mut cov = [[0.0f64; 3]; 3];
    for d in 0..i.vg.depth {
        let dp1 = d + 1;
        for h in 0..i.vg.height {
            let hp1 = h + 1;
            for w in 0..i.vg.width {
                let wp1 = w + 1;
                let val = mri_get_vox_val(g, err, i, w, h, d, 0, 0)?;
                let f = |k: i32| mul(k as f32, val);
                n = add64(n, wide(val));
                dd[0] = add64(dd[0], wide(f(wp1)));
                dd[1] = add64(dd[1], wide(f(hp1)));
                dd[2] = add64(dd[2], wide(f(dp1)));
                cov[0][0] = add64(cov[0][0], wide(f(wp1.wrapping_mul(wp1))));
                cov[1][1] = add64(cov[1][1], wide(f(hp1.wrapping_mul(hp1))));
                cov[2][2] = add64(cov[2][2], wide(f(dp1.wrapping_mul(dp1))));
                cov[0][1] = add64(cov[0][1], wide(f(wp1.wrapping_mul(hp1))));
                cov[1][2] = add64(cov[1][2], wide(f(hp1.wrapping_mul(dp1))));
                cov[0][2] = add64(cov[0][2], wide(f(wp1.wrapping_mul(dp1))));
            }
        }
    }
    for x in &mut dd {
        *x = div64(*x, n);
    }
    for k in 0..3 {
        cov[k][k] = sub64(div64(cov[k][k], n), mul64(dd[k], dd[k]));
    }
    cov[0][1] = sub64(div64(cov[0][1], n), mul64(dd[0], dd[1]));
    cov[1][2] = sub64(div64(cov[1][2], n), mul64(dd[1], dd[2]));
    cov[0][2] = sub64(div64(cov[0][2], n), mul64(dd[0], dd[2]));
    cov[1][0] = cov[0][1];
    cov[2][1] = cov[1][2];
    cov[2][0] = cov[0][2];
    let flat: Vec<f64> = cov.iter().flatten().copied().collect();
    let eig = symmetric_eigensystem(&flat, 3);
    if flat.iter().any(|x| !x.is_finite()) {
        let hangs = eig.never_returns();
        let unwritten = eig.ierr != 0;
        g.notices.add(
            &FS_967,
            if hangs {
                "CostFunctions::orientation of an image with an infinite or NaN voxel: FreeSurfer hangs (VNL's eigensystem); an error"
            } else if unwritten {
                "CostFunctions::orientation of an image with an infinite or NaN voxel: VNL leaves the eigenvectors unwritten; an error"
            } else {
                "CostFunctions::orientation of an image with an infinite or NaN voxel"
            },
        );
        if hangs {
            g.notices.add(
                &ITK_115,
                "vnl_symmetric_eigensystem would never return on the covariance",
            );
        }
        if hangs || unwritten || !g.notices.reproduce() {
            if unwritten && !hangs {
                let _ = writeln!(err, "vnl_symmetric_eigensystem: ierr = {}", eig.ierr);
            }
            let _ = writeln!(
                err,
                "CostFunctions::orientation: the image has an infinite or NaN voxel (no covariance)"
            );
            return Err(Fatal::Exit(1));
        }
    }
    let dval = &eig.values;
    let (mut smallest, mut largest) = (0usize, 0usize);
    for k in 1..3 {
        if dval[largest] < dval[k] {
            largest = k;
        }
        if dval[smallest] > dval[k] {
            smallest = k;
        }
    }
    let mut order = [
        largest,
        3usize.wrapping_sub(smallest).wrapping_sub(largest),
        smallest,
    ];
    let col = |c: usize, r: usize| eig.vectors[r * 3 + c];
    let mut evec = [[0.0f64; 3]; 3];
    if largest == smallest {
        g.notices.add(
            &FS_966,
            "CostFunctions::orientation: no eigenvalue above another: FreeSurfer reads a fourth eigenvector column",
        );
        if g.notices.reproduce() {
            // column 3 of the row-major 3 x 3 block: (V[1][0], V[2][0], the number past it)
            for (c, &k) in order.iter().enumerate() {
                for (r, row) in evec.iter_mut().enumerate() {
                    row[c] = if k == 3 {
                        if r < 2 { col(0, r + 1) } else { 0.0 }
                    } else {
                        col(k, r)
                    };
                }
            }
        } else {
            order = [0, 1, 2];
        }
    }
    if !(largest == smallest && g.notices.reproduce()) {
        for (c, &k) in order.iter().enumerate() {
            for (r, row) in evec.iter_mut().enumerate() {
                row[c] = col(k, r);
            }
        }
    }
    let d = vnl_fixed::det3(&evec);
    if d < 0.0 {
        for row in &mut evec {
            row[0] = -row[0];
        }
    }
    Ok(evec)
}
