// SPDX-License-Identifier: Apache-2.0
//! fslmaths' `-tensor_decomp` (`specs/fslmaths.md` §16): the eigen-decomposition of a diffusion
//! tensor image (6 volumes: `xx, xy, xz, yy, yz, zz`) per voxel, with the eigenvalues, the
//! fractional anisotropy, the mean diffusivity, the mode and the eigenvectors.
//!
//! The symmetric 3 × 3 eigenproblem is solved by cyclic Jacobi rotations in double precision
//! (to the double's precision; FSL calls LAPACK's divide-and-conquer solver). The eigenvectors'
//! signs are not defined by the problem: the original makes each one's largest component
//! positive (LAPACK's signs differ; §21 compares them up to their sign). Voxels run in
//! parallel, each on its own.

use larmorx_core::defects::{self, Notices};
use rayon::prelude::*;

use super::image::Img;
use super::num::{self, Num};
use super::ops::Stop;

/// The message of an input that is not a tensor (standard output, status 1).
pub const NOT_A_TENSOR: &str = "Error: input to tensor option is not a tensor!";

/// The nine outputs, in single precision (`v1`, `v2`, `v3`: 3 volumes each, x, y, z).
pub struct Decomposition {
    pub l1: Vec<f32>,
    pub l2: Vec<f32>,
    pub l3: Vec<f32>,
    pub fa: Vec<f32>,
    pub md: Vec<f32>,
    pub mo: Vec<f32>,
    pub v1: Vec<f32>,
    pub v2: Vec<f32>,
    pub v3: Vec<f32>,
}

/// The eigenvalues (descending) and eigenvectors (columns) of a symmetric 3 × 3 matrix.
fn eigen(mut a: [[f64; 3]; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..64 {
        let off = a[0][1] * a[0][1] + a[0][2] * a[0][2] + a[1][2] * a[1][2];
        if off == 0.0 {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q] == 0.0 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let t = if theta == 0.0 { 1.0 } else { t };
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            // A <- Jᵀ A J with the rotation in the (p, q) plane
            for k in 0..3 {
                let (akp, akq) = (a[k][p], a[k][q]);
                a[k][p] = c * akp - s * akq;
                a[k][q] = s * akp + c * akq;
            }
            for k in 0..3 {
                let (apk, aqk) = (a[p][k], a[q][k]);
                a[p][k] = c * apk - s * aqk;
                a[q][k] = s * apk + c * aqk;
            }
            for row in &mut v {
                let (vp, vq) = (row[p], row[q]);
                row[p] = c * vp - s * vq;
                row[q] = s * vp + c * vq;
            }
        }
    }
    let mut order = [0usize, 1, 2];
    order.sort_by(|&i, &j| {
        a[j][j]
            .partial_cmp(&a[i][i])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let w = order.map(|i| a[i][i]);
    let mut vecs = [[0.0; 3]; 3];
    for (c, &i) in order.iter().enumerate() {
        let col = [v[0][i], v[1][i], v[2][i]];
        // the largest component positive (the sign is free)
        let big = (0..3)
            .max_by(|&x, &y| {
                col[x]
                    .abs()
                    .partial_cmp(&col[y].abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(0);
        let sign = if col[big] < 0.0 { -1.0 } else { 1.0 };
        vecs[c] = col.map(|x| x * sign);
    }
    (w, vecs)
}

/// One voxel's outputs: `[L1, L2, L3, FA, MD, MO]` and the three eigenvectors, or zeros where
/// the largest eigenvalue is not positive.
fn voxel(t: [f32; 6]) -> ([f32; 6], [[f32; 3]; 3]) {
    let d = t.map(f64::from);
    let m = [[d[0], d[1], d[2]], [d[1], d[3], d[4]], [d[2], d[4], d[5]]];
    let (w, v) = eigen(m);
    if !(w[0] > 0.0) {
        return ([0.0; 6], [[0.0; 3]; 3]);
    }
    let l = w.map(|x| x as f32);
    let md = (l[0] + l[1] + l[2]) / 3.0;
    let e = l.map(|x| x - md);
    let s1 = e[0] * e[0] + e[1] * e[1] + e[2] * e[2];
    let s2 = l[0] * l[0] + l[1] * l[1] + l[2] * l[2];
    let fa = ((3.0 * s1) / (2.0 * s2)).sqrt();
    let n = (e[0] + e[1] - 2.0 * e[2]) * (2.0 * e[0] - e[1] - e[2]) * (e[0] - 2.0 * e[1] + e[2]);
    let inner = e[0] * e[0] + e[1] * e[1] + e[2] * e[2] - e[0] * e[1] - e[1] * e[2] - e[0] * e[2];
    let r = inner.max(0.0).sqrt();
    let den = 2.0 * (r * r * r);
    let mo = if den == 0.0 {
        0.0
    } else {
        (n / den).clamp(-1.0, 1.0)
    };
    (
        [l[0], l[1], l[2], fa, md, mo],
        v.map(|c| c.map(|x| x as f32)),
    )
}

/// `-tensor_decomp` of the current image: `Err` with FSL's message when it has not 6 volumes;
/// FSL-166: a NaN or infinite tensor stops FSL (Armadillo's index error), the default gives
/// that voxel 0 in every output (named).
pub fn decompose<T: Num>(img: &Img<T>, notices: &mut Notices) -> Result<Decomposition, Stop> {
    let per = img.per_volume();
    let data = &img.data;
    let bad = (0..per)
        .into_par_iter()
        .any(|k| (0..6).any(|j| data[j * per + k].non_finite()));
    if bad {
        if !notices.fix() {
            return Err(Stop::Exception(super::filters::abort_lines(
                "std::out_of_range",
                "Mat::operator(): index out of bounds",
            )));
        }
        notices.add(
            &defects::FSL_166,
            "-tensor_decomp of a NaN or infinite tensor: that voxel 0 in every output (FSL stops: \
             Mat::operator(): index out of bounds)",
        );
    }
    let res: Vec<([f32; 6], [[f32; 3]; 3])> = (0..per)
        .into_par_iter()
        .map(|k| {
            let t: [f32; 6] = std::array::from_fn(|j| num::to_f32(data[j * per + k].to_f64()));
            if t.iter().any(|x| !x.is_finite()) {
                ([0.0; 6], [[0.0; 3]; 3])
            } else {
                voxel(t)
            }
        })
        .collect();
    let pick = |j: usize| res.iter().map(|r| r.0[j]).collect::<Vec<f32>>();
    let vec = |c: usize| {
        let mut o = vec![0.0f32; 3 * per];
        for (k, r) in res.iter().enumerate() {
            for x in 0..3 {
                o[x * per + k] = r.1[c][x];
            }
        }
        o
    };
    Ok(Decomposition {
        l1: pick(0),
        l2: pick(1),
        l3: pick(2),
        fa: pick(3),
        md: pick(4),
        mo: pick(5),
        v1: vec(0),
        v2: vec(1),
        v3: vec(2),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_diagonal_tensor() {
        let (o, v) = voxel([2e-3, 0.0, 0.0, 1e-3, 0.0, 1e-3]);
        assert_eq!(&o[..3], &[2e-3f32, 1e-3, 1e-3]);
        assert_eq!(v[0], [1.0, 0.0, 0.0]);
        assert!((o[4] - 4e-3 / 3.0).abs() < 1e-9);
        // a prolate tensor's mode is 1
        assert_eq!(o[5], 1.0);
        let (o, _) = voxel([-1.0, 0.0, 0.0, -1.0, 0.0, -1.0]);
        assert_eq!(o, [0.0; 6]);
    }

    #[test]
    fn rotations_converge() {
        let (w, v) = eigen([[4.0, 1.0, 0.5], [1.0, 3.0, 0.2], [0.5, 0.2, 1.0]]);
        let m = [[4.0, 1.0, 0.5], [1.0, 3.0, 0.2], [0.5, 0.2, 1.0]];
        for c in 0..3 {
            for r in 0..3 {
                let mv: f64 = (0..3).map(|k| m[r][k] * v[c][k]).sum();
                assert!((mv - w[c] * v[c][r]).abs() < 1e-14);
            }
        }
        assert!(w[0] >= w[1] && w[1] >= w[2]);
    }
}
