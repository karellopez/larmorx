// SPDX-License-Identifier: Apache-2.0
//! The principal-axes alignment of the schedule's `alignpaxes` (`specs/flirt.md` §8.7): each
//! image's intensity-weighted covariance in millimetres, its eigenvectors, and the rotation
//! that carries the input's axes onto the reference's.

use larmorx_mri::hmc::volume::Volume;

use super::mat::{IDENTITY, Mat4};

type M3 = [[f64; 3]; 3];

/// The principal axes of `v` (§8.7): the eigenvectors of its covariance in mm as the columns of
/// `R`, eigenvalues ascending; `Err` for a non-finite moment; the warning when the total is 0.
pub fn axes(v: &Volume, warn: &mut Vec<String>) -> Result<M3, String> {
    let [nx, ny, _] = v.shape;
    let n = v.len();
    let m = v.data.iter().copied().fold(f32::INFINITY, f32::min);
    let block = (1000usize.max((n as f64).sqrt() as usize)) + 1;
    let mut tot = [0.0f64; 10];
    let mut part = [0.0f64; 10];
    for (i, &val) in v.data.iter().enumerate() {
        let w = f64::from(val - m);
        let (x, y, z) = (
            (i % nx) as f64,
            ((i / nx) % ny) as f64,
            (i / (nx * ny)) as f64,
        );
        let terms = [
            w,
            w * x,
            w * y,
            w * z,
            (w * x) * x,
            (w * x) * y,
            (w * x) * z,
            (w * y) * y,
            (w * y) * z,
            (w * z) * z,
        ];
        for k in 0..10 {
            part[k] += terms[k];
        }
        if (i + 1) % block == 0 {
            for k in 0..10 {
                tot[k] += part[k];
                part[k] = 0.0;
            }
        }
    }
    for k in 0..10 {
        tot[k] += part[k];
    }
    if tot.iter().any(|t| !t.is_finite()) {
        return Err("Mat::operator(): index out of bounds".into());
    }
    let mut total = tot[0];
    if total.abs() < 1e-5 {
        warn.push("WARNING::in calculating Principle Axes, total = 0.0".into());
        total = 1.0;
    }
    let mu = [tot[1] / total, tot[2] / total, tot[3] / total];
    let sec = [
        [tot[4], tot[5], tot[6]],
        [tot[5], tot[7], tot[8]],
        [tot[6], tot[8], tot[9]],
    ]
    .map(|r| r.map(|x| x / total));
    let d = v.voxel_size.map(f64::from);
    let mut c = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..=i {
            let a = (d[i] * sec[i][j]) * d[j] - (d[i] * mu[i]) * (d[j] * mu[j]);
            c[i][j] = a;
            c[j][i] = a;
        }
    }
    let (vals, vecs) = jacobi_eigen(c);
    // eigenvalues ascending: the columns sorted by them
    let mut order = [0usize, 1, 2];
    order.sort_by(|&a, &b| {
        vals[a]
            .partial_cmp(&vals[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut r = [[0.0; 3]; 3];
    for (col, &k) in order.iter().enumerate() {
        for i in 0..3 {
            r[i][col] = vecs[i][k];
        }
    }
    Ok(r)
}

/// The eigenvalues and eigenvectors (columns) of a symmetric 3 × 3 matrix by Jacobi rotations.
fn jacobi_eigen(mut a: M3) -> ([f64; 3], M3) {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..100 {
        let off = a[0][1].abs() + a[0][2].abs() + a[1][2].abs();
        if off < 1e-300 {
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
            for k in 0..3 {
                let (vkp, vkq) = (v[k][p], v[k][q]);
                v[k][p] = c * vkp - s * vkq;
                v[k][q] = s * vkp + c * vkq;
            }
        }
    }
    ([a[0][0], a[1][1], a[2][2]], v)
}

fn det3(r: &M3) -> f64 {
    r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
        - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
        + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0])
}

/// The alignment of the input's axes `ri` onto the reference's `rr` (§8.7), with the centres of
/// mass `cr` and `ci` (scaled mm): `L = R_r·R_iᵀ`, `t = c_r − L·c_i`.
pub fn align(mut rr: M3, mut ri: M3, cr: [f64; 3], ci: [f64; 3]) -> Mat4 {
    for n in 0..2 {
        let dot: f64 = (0..3).map(|k| rr[k][n] * ri[k][n]).sum();
        if dot < 0.0 {
            for row in rr.iter_mut() {
                row[n] = -row[n];
            }
        }
    }
    for r in [&mut rr, &mut ri] {
        if det3(r) < 0.0 {
            for row in r.iter_mut() {
                row[2] = -row[2];
            }
        }
    }
    let mut m = IDENTITY;
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = (0..3).map(|k| rr[i][k] * ri[j][k]).sum();
        }
    }
    for i in 0..3 {
        m[i][3] = cr[i] - (m[i][0] * ci[0] + m[i][1] * ci[1] + m[i][2] * ci[2]);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_elongated_blob_and_its_rotation() {
        // an ellipsoid elongated along x, and the same along y: a quarter turn about z
        let make = |swap: bool| {
            let n = 21usize;
            let mut data = vec![0.0f32; n * n * n];
            for z in 0..n {
                for y in 0..n {
                    for x in 0..n {
                        let (a, b) = if swap { (y, x) } else { (x, y) };
                        let u = (a as f64 - 10.0) / 9.0;
                        let w = (b as f64 - 10.0) / 4.0;
                        let q = (z as f64 - 10.0) / 2.0;
                        if u * u + w * w + q * q <= 1.0 {
                            data[x + n * (y + n * z)] = 1.0;
                        }
                    }
                }
            }
            Volume::new([n, n, n], [1.0; 3], data)
        };
        let (a, b) = (make(false), make(true));
        let mut w = Vec::new();
        let (ra, rb) = (axes(&a, &mut w).unwrap(), axes(&b, &mut w).unwrap());
        let m = align(rb, ra, b.centre_of_mass(), a.centre_of_mass());
        // the input's x axis goes to the reference's y axis (up to its sign)
        assert!(m[1][0].abs() > 0.99, "{m:?}");
        assert!(w.is_empty());
    }
}
