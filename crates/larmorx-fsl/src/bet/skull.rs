// SPDX-License-Identifier: Apache-2.0
//! The approximate skull image of `bet2 -s` (`specs/bet.md` §4.7): from the surface voxels
//! nearest each vertex, a profile outward along the vertex's normal; the skull's outer edge is
//! the steepest rise before the profile's maximum, after its darkest point.

use rayon::prelude::*;

use super::Vol;
use super::mesh::{Mesh, V3, add, dist, scale};
use super::raster::{self, Rounding};

/// Trilinear interpolation of `vol` at the voxel coordinates `x` (in float, as FSL samples):
/// `None` outside the image.
#[inline]
pub fn trilinear(vol: &Vol, x: [f64; 3]) -> Option<f32> {
    let [nx, ny, nz] = vol.shape;
    let c = x.map(|v| v as f32);
    let f = c.map(f32::floor);
    if f.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let (i, j, k) = (f[0] as i64, f[1] as i64, f[2] as i64);
    if i < 0 || j < 0 || k < 0 || i as usize >= nx || j as usize >= ny || k as usize >= nz {
        return None;
    }
    let at = |a: i64, b: i64, c: i64| -> f32 {
        let a = (a as usize).min(nx - 1);
        let b = (b as usize).min(ny - 1);
        let c = (c as usize).min(nz - 1);
        vol.data[(c * ny + b) * nx + a]
    };
    let (dx, dy, dz) = (c[0] - f[0], c[1] - f[1], c[2] - f[2]);
    let c00 = at(i, j, k) * (1.0 - dx) + at(i + 1, j, k) * dx;
    let c10 = at(i, j + 1, k) * (1.0 - dx) + at(i + 1, j + 1, k) * dx;
    let c01 = at(i, j, k + 1) * (1.0 - dx) + at(i + 1, j, k + 1) * dx;
    let c11 = at(i, j + 1, k + 1) * (1.0 - dx) + at(i + 1, j + 1, k + 1) * dx;
    let c0 = c00 * (1.0 - dy) + c10 * dy;
    let c1 = c01 * (1.0 - dy) + c11 * dy;
    Some(c0 * (1.0 - dz) + c1 * dz)
}

/// The skull image (`true` at the skull's estimated outer edge).
pub fn skull(mesh: &Mesh, points: &[V3], vol: &Vol, t: f64, t2: f64, t98: f64) -> Vec<bool> {
    let shape = vol.shape;
    let d = vol.voxel;
    let [nx, ny, nz] = shape;
    let n = nx * ny * nz;
    let mut surface = vec![false; n];
    raster::draw(mesh, points, shape, d, Rounding::Floor, |k| surface[k] = true);
    let normals = mesh.triangle_normals(points);
    let step = d.iter().copied().fold(f64::INFINITY, f64::min) / 2.0;
    let range = t98 - t2;
    let hits: Vec<Vec<usize>> = (0..points.len())
        .into_par_iter()
        .with_min_len(32)
        .map(|v| {
            let p = points[v];
            let nrm = mesh.vertex_normal(v, &normals);
            let nb = mesh.neighbours.row(v);
            let h = nb
                .iter()
                .map(|&j| dist(p, points[j as usize]))
                .fold(0.0f64, f64::max)
                / 2.0;
            let h = h.ceil();
            let lo = [0, 1, 2].map(|a| ((p[a] - h) / d[a]).floor().max(0.0));
            let hi = [0, 1, 2].map(|a| ((p[a] + h) / d[a]).floor().min(shape[a] as f64 - 1.0));
            let mut out = Vec::new();
            if !(0..3).all(|a| lo[a].is_finite() && hi[a].is_finite() && lo[a] <= hi[a]) {
                return out;
            }
            for k in lo[2] as usize..=hi[2] as usize {
                for j in lo[1] as usize..=hi[1] as usize {
                    for i in lo[0] as usize..=hi[0] as usize {
                        let idx = (k * ny + j) * nx + i;
                        if !surface[idx] {
                            continue;
                        }
                        let c = [i as f64 * d[0], j as f64 * d[1], k as f64 * d[2]];
                        let dp = dist(c, p);
                        if nb.iter().any(|&q| dist(c, points[q as usize]) < dp) {
                            continue;
                        }
                        if let Some(s) = profile_edge(vol, c, nrm, step, t, range) {
                            let q = add(c, scale(nrm, s));
                            let x = [0, 1, 2].map(|a| (q[a] / d[a]).trunc());
                            if (0..3).all(|a| x[a] >= 0.0 && x[a] < shape[a] as f64) {
                                out.push(
                                    (x[2] as usize * ny + x[1] as usize) * nx + x[0] as usize,
                                );
                            }
                        }
                    }
                }
            }
            out
        })
        .collect();
    let mut skull = vec![false; n];
    for h in hits {
        for k in h {
            skull[k] = true;
        }
    }
    skull
}

/// The position along the profile from `c` (mm) along `nrm` of the skull's outer edge.
fn profile_edge(vol: &Vol, c: V3, nrm: V3, step: f64, t: f64, range: f64) -> Option<f64> {
    let d = vol.voxel;
    let sample = |s: f64| -> Option<f64> {
        let q = add(c, scale(nrm, s));
        trilinear(vol, [q[0] / d[0], q[1] / d[1], q[2] / d[2]]).map(f64::from)
    };
    let start = -3.0;
    let count = ((30.0 - start) / step).floor() as usize;
    let mut prof: Vec<(f64, f64)> = Vec::with_capacity(count);
    for k in 0..count {
        let s = start + k as f64 * step;
        if let Some(v) = sample(s) {
            prof.push((s, v));
        }
    }
    let (imax, &(smax, vmax)) = prof
        .iter()
        .enumerate()
        .filter(|(_, (s, _))| *s >= 0.0)
        .max_by(|a, b| a.1.1.total_cmp(&b.1.1))?;
    if !(vmax > t) {
        return None;
    }
    let _ = smax;
    let (imin, _) = prof[..imax]
        .iter()
        .enumerate()
        .map(|(k, &(s, v))| (k, s / 30.0 - v / range))
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    let mut best = None::<(usize, f64)>;
    for k in imin..imax {
        let rise = prof[k + 1].1 - prof[k].1;
        if best.is_none_or(|(_, b)| rise > b) {
            best = Some((k, rise));
        }
    }
    // the sample the steepest rise reaches (observed: closest to FSL's skull voxels)
    best.map(|(k, _)| prof[k + 1].0)
}
