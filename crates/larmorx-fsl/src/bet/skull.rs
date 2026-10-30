// SPDX-License-Identifier: Apache-2.0
//! The approximate skull image of `bet2 -s` (`specs/bet.md` §4.7): from the surface voxels
//! nearest each vertex, a profile outward along the vertex's normal; the skull's outer edge is
//! the end of the steepest rise after the profile's darkest point, before its maximum.
//!
//! Each step is the spec's ("The skull search, exactly"): voxels outside the image count as
//! surface voxels, the candidates are not clipped to the image, the profile starts at the
//! voxel's own integer position, the three scans have grids of their own, and the trilinear
//! interpolation is §8.1's.

use rayon::prelude::*;

use super::Vol;
use super::mesh::{Mesh, V3, dist};
use super::raster::{self, Rounding};

/// Trilinear interpolation of `vol` at the voxel coordinates `x` (§8.1): the coordinates
/// rounded to float, `i = floor(x)`, `f = x − i` in float; the eight corners, 0 outside the
/// image (the `+1` corners past the last voxel too); in float, x first, then y, then z, each
/// step `(b − a)·f + a`.
#[inline]
pub fn trilinear(vol: &Vol, x: [f64; 3]) -> f32 {
    let [nx, ny, nz] = vol.shape;
    let c = x.map(|v| v as f32);
    let fl = c.map(f32::floor);
    if fl.iter().any(|v| !v.is_finite() || v.abs() > 1e9) {
        return 0.0;
    }
    let (i, j, k) = (fl[0] as i64, fl[1] as i64, fl[2] as i64);
    let (fx, fy, fz) = (c[0] - fl[0], c[1] - fl[1], c[2] - fl[2]);
    let at = |a: i64, b: i64, cc: i64| -> f32 {
        if a < 0 || b < 0 || cc < 0 || a as usize >= nx || b as usize >= ny || cc as usize >= nz {
            0.0
        } else {
            vol.data[(cc as usize * ny + b as usize) * nx + a as usize]
        }
    };
    let v000 = at(i, j, k);
    let v100 = at(i + 1, j, k);
    let v001 = at(i, j, k + 1);
    let v101 = at(i + 1, j, k + 1);
    let v010 = at(i, j + 1, k);
    let v110 = at(i + 1, j + 1, k);
    let v011 = at(i, j + 1, k + 1);
    let v111 = at(i + 1, j + 1, k + 1);
    let t1 = (v100 - v000) * fx + v000;
    let t2 = (v101 - v001) * fx + v001;
    let t3 = (v110 - v010) * fx + v010;
    let t4 = (v111 - v011) * fx + v011;
    let t5 = (t3 - t1) * fy + t1;
    let t6 = (t4 - t2) * fy + t2;
    (t6 - t5) * fz + t5
}

/// The skull image (`true` at the skull's estimated outer edge).
pub fn skull(mesh: &Mesh, points: &[V3], vol: &Vol, t: f64, t2: f64, t98: f64) -> Vec<bool> {
    let shape = vol.shape;
    let d = vol.voxel;
    let [nx, ny, nz] = shape;
    let n = nx * ny * nz;
    // the surface drawn as for the mask (floor); outside the volume reads as surface too
    let mut surface = vec![false; n];
    raster::draw(mesh, points, shape, d, Rounding::Floor, |k| {
        surface[k] = true
    });
    let is_surface = |i: i64, j: i64, k: i64| -> bool {
        if i < 0 || j < 0 || k < 0 || i as usize >= nx || j as usize >= ny || k as usize >= nz {
            true
        } else {
            surface[(k as usize * ny + j as usize) * nx + i as usize]
        }
    };
    let dims = [nx as f64, ny as f64, nz as f64];
    let inside = |x: V3| (0..3).all(|a| x[a] >= 0.0 && x[a] < dims[a]);
    let normals = mesh.triangle_normals(points);
    let hs = d.iter().copied().fold(f64::INFINITY, f64::min);
    let half = hs / 2.0;
    let range = t98 - t2;
    let hits: Vec<Vec<usize>> = (0..points.len())
        .into_par_iter()
        .with_min_len(32)
        .map(|v| {
            let mut out = Vec::new();
            let p = points[v];
            let nrm = mesh.vertex_normal(v, &normals);
            let nu = [nrm[0] / d[0], nrm[1] / d[1], nrm[2] / d[2]];
            let nb = mesh.neighbours.row(v);
            let mut far = 0.0f64;
            for &q in nb {
                let e = dist(p, points[q as usize]);
                if e > far {
                    far = e;
                }
            }
            let h = (far / 2.0).ceil();
            let m = [p[0] / d[0], p[1] / d[1], p[2] / d[2]];
            let lo = [0, 1, 2].map(|a| (m[a] - h / d[a]).floor());
            let hi = [0, 1, 2].map(|a| (m[a] + h / d[a]).floor());
            if !(0..3).all(|a| lo[a].is_finite() && hi[a].is_finite() && hi[a] - lo[a] < 1e6) {
                return out;
            }
            let (lo, hi) = (lo.map(|x| x as i64), hi.map(|x| x as i64));
            for k in lo[2]..=hi[2] {
                for j in lo[1]..=hi[1] {
                    for i in lo[0]..=hi[0] {
                        if !is_surface(i, j, k) {
                            continue;
                        }
                        let x = [i as f64 * d[0], j as f64 * d[1], k as f64 * d[2]];
                        let dp = dist(x, p);
                        let mut nearest = f64::INFINITY;
                        for &q in nb {
                            let e = dist(x, points[q as usize]);
                            if e < nearest {
                                nearest = e;
                            }
                        }
                        if !(nearest >= dp) {
                            continue;
                        }
                        let o = [i as f64, j as f64, k as f64];
                        if let Some(s) = profile(vol, o, nu, half, hs, t, range, &inside) {
                            let q = [o[0] + s * nu[0], o[1] + s * nu[1], o[2] + s * nu[2]];
                            let x = q.map(|c| c.trunc() as usize);
                            out.push((x[2] * ny + x[1]) * nx + x[0]);
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

/// The skull's depth along the profile from the voxel position `o` along `nu` (voxels per
/// mm), or `None` (§4.7, steps 5 to 8); the returned depth's point lies in the image.
#[allow(clippy::too_many_arguments)]
fn profile(
    vol: &Vol,
    o: V3,
    nu: V3,
    half: f64,
    hs: f64,
    t: f64,
    range: f64,
    inside: &impl Fn(V3) -> bool,
) -> Option<f64> {
    let at = |s: f64| -> V3 { [o[0] + s * nu[0], o[1] + s * nu[1], o[2] + s * nu[2]] };
    let sample = |x: V3| -> f64 { f64::from(trilinear(vol, x)) };
    // 5. the first position of the strict maximum, from t (no bounds test)
    let mut big_m = t;
    let mut d_max = 0.0;
    let mut raised = false;
    let mut s = 0.0;
    while s < 30.0 {
        let v = sample(at(s));
        if v > big_m {
            big_m = v;
            d_max = s;
            raised = true;
        }
        s += half;
    }
    if !raised {
        return None;
    }
    // 6. the darkest point before it, on a grid of its own from -3 mm
    let mut d_min = -3.0;
    let mut j_max = -1.0e6;
    let mut s = -3.0;
    while s < d_max {
        let x = at(s);
        if inside(x) {
            let j = s / 30.0 - sample(x) / range;
            if j > j_max {
                j_max = j;
                d_min = s;
            }
        }
        s += half;
    }
    // 7. the steepest rise from there (a smaller rise ends the scan; falls do not)
    let start = at(d_min);
    if !inside(start) {
        return None;
    }
    let mut prev = sample(start);
    let mut big_g = 0.0;
    let mut d_skull = 0.0;
    let mut s = d_min + hs;
    while s < d_max {
        let x = at(s);
        if inside(x) {
            let v = sample(x);
            let g = v - prev;
            prev = v;
            if g > 0.0 {
                if g > big_g {
                    big_g = g;
                    d_skull = s;
                } else {
                    break;
                }
            }
        }
        s += half;
    }
    // 8.
    (big_g > 0.0 && inside(at(d_skull))).then_some(d_skull)
}
