// SPDX-License-Identifier: Apache-2.0
//! The deformation of the sphere onto the brain's surface and its self-intersection test
//! (`specs/bet.md` §4.4, §4.5).
//!
//! Every vertex's new position is computed from the previous iteration's positions only, so
//! the vertices are updated in parallel with the same result at any thread count. The
//! self-intersection measure looks for close pairs in a grid of cells as wide as the distance
//! that counts, then sums the pairs in index order.

use rayon::prelude::*;

use super::Vol;
use super::mesh::{Mesh, V3, add, cross, dist, dot, scale, sub};
use super::params::Initial;

/// The options of the deformation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Deformation {
    /// The fractional intensity threshold `f` (`-f`).
    pub frac: f64,
    /// The vertical gradient of the threshold (`-g`).
    pub gradient: f64,
    /// The smoothness factor (`-w`).
    pub smoothness: f64,
    /// The search depths of the intensity term (`--parameter_d1`, `--parameter_d2`).
    pub d1: f64,
    pub d2: f64,
    /// The exact self-intersection test (`-e`).
    pub exact: bool,
}

impl Default for Deformation {
    fn default() -> Self {
        Deformation {
            frac: 0.5,
            gradient: 0.0,
            smoothness: 1.0,
            d1: 7.0,
            d2: 3.0,
            exact: false,
        }
    }
}

/// The number of iterations of one pass.
pub const ITERATIONS: usize = 1000;
/// The last rerun pass.
pub const LAST_PASS: u32 = 10;
/// The self-intersection total above which the surface is rerun.
pub const THRESHOLD: f64 = 4000.0;

/// The initial surface: the unit sphere scaled to half the radius, centred on `c`.
pub fn initial_surface(sphere: &Mesh, init: &Initial) -> Vec<V3> {
    let r = init.radius / 2.0;
    sphere
        .points
        .iter()
        .map(|p| add(scale(*p, r), init.centre))
        .collect()
}

struct Sampler<'a> {
    vol: &'a Vol,
    /// The sampling grid's spacing: the voxel size, or `2 r` along an axis whose field of view
    /// is smaller than `r`.
    spacing: [f64; 3],
    ds: f64,
}

impl Sampler<'_> {
    /// The value at voxel `trunc(x)` (C's conversion), if inside.
    #[inline]
    fn at(&self, x: [f64; 3]) -> Option<f32> {
        let [nx, ny, nz] = self.vol.shape;
        let i = c_int(x[0])?;
        let j = c_int(x[1])?;
        let k = c_int(x[2])?;
        if i < 0 || j < 0 || k < 0 || i as usize >= nx || j as usize >= ny || k as usize >= nz {
            return None;
        }
        Some(self.vol.data[(k as usize * ny + j as usize) * nx + i as usize])
    }
}

/// C's `(int)` of a double: truncation toward zero, `None` for what does not fit (x86-64's
/// `INT_MIN`, outside every image).
#[inline]
pub fn c_int(x: f64) -> Option<i64> {
    if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        None
    } else {
        Some(x as i64)
    }
}

/// The intensity term `f3` of a vertex at `p` with normal `n` (§4.4).
#[inline]
#[allow(clippy::too_many_arguments)]
fn intensity_term(
    s: &Sampler<'_>,
    p: V3,
    n: V3,
    init: &Initial,
    bt: f64,
    gradient: f64,
    d1: f64,
    d2: f64,
) -> f64 {
    let d = s.spacing;
    let q = sub(p, n);
    let base = [q[0] / d[0] + 0.5, q[1] / d[1] + 0.5, q[2] / d[2] + 0.5];
    let Some(v) = s.at(base) else {
        return 0.0;
    };
    let mut imin = init.median;
    let mut imax = init.t;
    let v = f64::from(v);
    imin = imin.min(v);
    imax = imax.max(v);
    let nd = [n[0] / d[0], n[1] / d[1], n[2] / d[2]];
    let deep = [
        base[0] - (d1 - 1.0) * nd[0],
        base[1] - (d1 - 1.0) * nd[1],
        base[2] - (d1 - 1.0) * nd[2],
    ];
    let Some(v) = s.at(deep) else {
        return 0.0;
    };
    imin = imin.min(f64::from(v));
    let step = scale(nd, s.ds);
    let mut pos = base;
    let mut k = 1usize;
    loop {
        let g = 2.0 + (k - 1) as f64 * s.ds;
        if g >= d1 {
            break;
        }
        pos = sub(pos, step);
        if let Some(v) = s.at(pos) {
            let v = f64::from(v);
            imin = imin.min(v);
            if g < d2 {
                imax = imax.max(v);
            }
        }
        k += 1;
    }
    let imin = imin.max(init.t2);
    let imax = imax.min(init.median);
    let b = if gradient != 0.0 {
        (bt + gradient * (p[2] - init.centre[2]) / init.radius).clamp(0.0, 1.0)
    } else {
        bt
    };
    let tl = (imax - init.t2) * b + init.t2;
    let span = imax - init.t2;
    if span > 0.0 {
        2.0 * (imin - tl) / span
    } else {
        2.0 * (imin - tl)
    }
}

/// `tanh`, from the `libm` crate: `larmorx_core::math` has no correctly rounded `tanh` in
/// double precision yet (CLAUDE.md rule 5).
#[inline]
fn tanh(x: f64) -> f64 {
    libm::tanh(x)
}

/// One pass of [`ITERATIONS`] deformation steps from `start`; `pass` 0 is the first, a rerun
/// `P >= 1` raises the smoothness of concave places (§4.5).
pub fn deform(
    mesh: &Mesh,
    start: &[V3],
    vol: &Vol,
    init: &Initial,
    opts: &Deformation,
    pass: u32,
) -> Vec<V3> {
    let r = init.radius;
    let mut spacing = vol.voxel;
    for a in 0..3 {
        if (vol.shape[a] as f64) * vol.voxel[a] < r {
            spacing[a] = 2.0 * r;
        }
    }
    let ds = spacing.iter().copied().fold(1.0f64, f64::min);
    let sampler = Sampler { vol, spacing, ds };
    let bt = larmorx_core::math::pow(opts.frac, 0.275);
    let rmin = 3.33 * opts.smoothness;
    let rmax = 10.0 * opts.smoothness;
    let e = (1.0 / rmin + 1.0 / rmax) / 2.0;
    let f = 6.0 / (1.0 / rmin - 1.0 / rmax);
    let boost = if pass >= 1 {
        Some(10f64.powi(pass as i32 + 1))
    } else {
        None
    };
    let mut points = start.to_vec();
    let mut next = vec![[0.0; 3]; points.len()];
    let mut l = 0.0;
    for i in 0..ITERATIONS {
        if i == 50 || i % 100 == 0 {
            l = mesh.mean_neighbour_distance(&points);
        }
        let factor = boost.map(|b| {
            if i > 750 {
                4.0 * (1.0 - i as f64 / 1000.0) * (b - 1.0) + 1.0
            } else {
                b
            }
        });
        let normals = mesh.triangle_normals(&points);
        let l2 = l * l;
        let pts = &points;
        next.par_iter_mut()
            .with_min_len(64)
            .enumerate()
            .for_each(|(v, out)| {
                let p = pts[v];
                let n = mesh.vertex_normal(v, &normals);
                let m = mesh.neighbour_mean(v, pts);
                let s = sub(m, p);
                let sn = dot(s, n);
                let s_n = scale(n, sn);
                let s_t = sub(s, s_n);
                let u1 = scale(s_t, 0.5);
                let rinv = 2.0 * sn.abs() / l2;
                let mut f2 = (1.0 + tanh(f * (rinv - e))) / 2.0;
                if let Some(factor) = factor
                    && sn > 0.0
                {
                    f2 = (f2 * factor).min(1.0);
                }
                let u2 = scale(s_n, f2);
                let f3 = intensity_term(
                    &sampler,
                    p,
                    n,
                    init,
                    bt,
                    opts.gradient,
                    opts.d1,
                    opts.d2,
                );
                let u3 = scale(n, 0.05 * l * f3);
                *out = add(add(add(p, u1), u2), u3);
            });
        std::mem::swap(&mut points, &mut next);
    }
    points
}

/// The self-intersection total of `points` against the initial surface `initial` (§4.5):
/// over ordered pairs of distinct, non-adjacent vertices closer than the mean neighbour
/// distance `ml`, the sum of `(d / ml - d_o / mlo)²`.
pub fn self_intersection(mesh: &Mesh, points: &[V3], initial: &[V3]) -> f64 {
    let ml = mesh.mean_neighbour_distance(points);
    let mlo = mesh.mean_neighbour_distance(initial);
    if !(ml > 0.0) || !ml.is_finite() {
        return if ml.is_nan() { f64::NAN } else { 0.0 };
    }
    let grid = Grid::new(points, ml);
    let terms: Vec<Vec<(u32, f64)>> = (0..points.len())
        .into_par_iter()
        .with_min_len(64)
        .map(|i| {
            let p = points[i];
            let nb = mesh.neighbours.row(i);
            let mut out = Vec::new();
            grid.near(p, |j| {
                if j as usize == i || nb.contains(&j) {
                    return;
                }
                let d = dist(p, points[j as usize]);
                if d < ml {
                    let d0 = dist(initial[i], initial[j as usize]);
                    let x = d / ml - d0 / mlo;
                    out.push((j, x * x));
                }
            });
            out.sort_unstable_by_key(|t| t.0);
            out
        })
        .collect();
    let mut total = 0.0;
    for t in terms {
        for (_, x) in t {
            total += x;
        }
    }
    total
}

/// Points binned in cubic cells.
struct Grid {
    origin: V3,
    cell: f64,
    dims: [usize; 3],
    start: Vec<u32>,
    items: Vec<u32>,
}

impl Grid {
    fn new(points: &[V3], cell: f64) -> Grid {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in points {
            for a in 0..3 {
                if p[a].is_finite() {
                    lo[a] = lo[a].min(p[a]);
                    hi[a] = hi[a].max(p[a]);
                }
            }
        }
        for a in 0..3 {
            if !lo[a].is_finite() {
                lo[a] = 0.0;
                hi[a] = 0.0;
            }
        }
        // at most 128 cells per axis: a coarser grid when the points spread far
        let span = (0..3).map(|a| hi[a] - lo[a]).fold(0.0f64, f64::max);
        let cell = cell.max(span / 128.0).max(f64::MIN_POSITIVE);
        let dims = [0, 1, 2].map(|a| (((hi[a] - lo[a]) / cell) as usize + 1).min(129));
        let index = |p: &V3| -> Option<usize> {
            if !(p[0].is_finite() && p[1].is_finite() && p[2].is_finite()) {
                return None;
            }
            let c = [0, 1, 2].map(|a| (((p[a] - lo[a]) / cell) as usize).min(dims[a] - 1));
            Some((c[2] * dims[1] + c[1]) * dims[0] + c[0])
        };
        let ncell = dims[0] * dims[1] * dims[2];
        let mut count = vec![0u32; ncell + 1];
        for p in points {
            if let Some(c) = index(p) {
                count[c + 1] += 1;
            }
        }
        for c in 0..ncell {
            count[c + 1] += count[c];
        }
        let mut fill = count.clone();
        let mut items = vec![0u32; count[ncell] as usize];
        for (i, p) in points.iter().enumerate() {
            if let Some(c) = index(p) {
                items[fill[c] as usize] = i as u32;
                fill[c] += 1;
            }
        }
        Grid {
            origin: lo,
            cell,
            dims,
            start: count,
            items,
        }
    }

    /// Calls `f` with every point in the 27 cells around `p`.
    fn near(&self, p: V3, mut f: impl FnMut(u32)) {
        if !(p[0].is_finite() && p[1].is_finite() && p[2].is_finite()) {
            return;
        }
        let c = [0, 1, 2].map(|a| {
            (((p[a] - self.origin[a]) / self.cell) as isize).clamp(0, self.dims[a] as isize - 1)
        });
        for z in (c[2] - 1).max(0)..=(c[2] + 1).min(self.dims[2] as isize - 1) {
            for y in (c[1] - 1).max(0)..=(c[1] + 1).min(self.dims[1] as isize - 1) {
                for x in (c[0] - 1).max(0)..=(c[0] + 1).min(self.dims[0] as isize - 1) {
                    let k = (z as usize * self.dims[1] + y as usize) * self.dims[0] + x as usize;
                    for &i in &self.items[self.start[k] as usize..self.start[k + 1] as usize] {
                        f(i);
                    }
                }
            }
        }
    }
}

/// Whether two triangles that share no vertex intersect (the exact test of `-e`, §4.5).
pub fn intersects(mesh: &Mesh, points: &[V3]) -> bool {
    let tris = &mesh.triangles;
    let boxes: Vec<(V3, V3)> = tris
        .iter()
        .map(|t| {
            let (a, b, c) = (
                points[t[0] as usize],
                points[t[1] as usize],
                points[t[2] as usize],
            );
            let lo = [0, 1, 2].map(|k| a[k].min(b[k]).min(c[k]));
            let hi = [0, 1, 2].map(|k| a[k].max(b[k]).max(c[k]));
            (lo, hi)
        })
        .collect();
    let centres: Vec<V3> = boxes
        .iter()
        .map(|(lo, hi)| scale(add(*lo, *hi), 0.5))
        .collect();
    let extent = boxes
        .iter()
        .map(|(lo, hi)| (0..3).map(|k| hi[k] - lo[k]).fold(0.0f64, f64::max))
        .fold(0.0f64, f64::max);
    if !extent.is_finite() {
        return true;
    }
    let grid = Grid::new(&centres, extent.max(1e-9));
    (0..tris.len()).into_par_iter().with_min_len(64).any(|i| {
        let ti = tris[i];
        let (lo, hi) = boxes[i];
        let mut hit = false;
        grid.near(centres[i], |j| {
            let j = j as usize;
            if hit || j <= i {
                return;
            }
            let tj = tris[j];
            if ti.iter().any(|v| tj.contains(v)) {
                return;
            }
            let (lo2, hi2) = boxes[j];
            if (0..3).any(|k| hi[k] < lo2[k] || hi2[k] < lo[k]) {
                return;
            }
            let a = ti.map(|v| points[v as usize]);
            let b = tj.map(|v| points[v as usize]);
            if triangles_intersect(&a, &b) {
                hit = true;
            }
        });
        hit
    })
}

fn triangles_intersect(a: &[V3; 3], b: &[V3; 3]) -> bool {
    (0..3).any(|k| segment_hits(a[k], a[(k + 1) % 3], b))
        || (0..3).any(|k| segment_hits(b[k], b[(k + 1) % 3], a))
}

/// Whether the segment `p q` crosses triangle `t` (Möller and Trumbore's test).
fn segment_hits(p: V3, q: V3, t: &[V3; 3]) -> bool {
    let dir = sub(q, p);
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let h = cross(dir, e2);
    let a = dot(e1, h);
    if a.abs() < 1e-12 {
        return false;
    }
    let f = 1.0 / a;
    let s = sub(p, t[0]);
    let u = f * dot(s, h);
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let qv = cross(s, e1);
    let v = f * dot(dir, qv);
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    let w = f * dot(e2, qv);
    (0.0..=1.0).contains(&w)
}
