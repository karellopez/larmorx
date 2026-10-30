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

/// The initial surface: the unit sphere scaled to half the radius, centred on `c`, placed as
/// §4.4 gives it (translated by `c`, then scaled about `c`: `c + (r/2)·((p + c) − c)`).
pub fn initial_surface(sphere: &Mesh, init: &Initial) -> Vec<V3> {
    let half = init.radius / 2.0;
    let c = init.centre;
    sphere
        .points
        .iter()
        .map(|p| add(c, scale(sub(add(*p, c), c), half)))
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
    // the depth `g` accumulated too (2, then `+ ds` per sample: §4.4)
    let mut g = 2.0;
    while g < d1 {
        pos = sub(pos, step);
        if let Some(v) = s.at(pos) {
            let v = f64::from(v);
            imin = imin.min(v);
            if g < d2 {
                imax = imax.max(v);
            }
        }
        g += s.ds;
    }
    let imin = imin.max(init.t2);
    let imax = imax.min(init.median);
    let b = if gradient != 0.0 {
        1f64.min(0f64.max(bt + (gradient * (p[2] - init.centre[2])) / init.radius))
    } else {
        bt
    };
    let tl = (imax - init.t2) * b + init.t2;
    let span = imax - init.t2;
    if span > 0.0 {
        (2.0 * (imin - tl)) / span
    } else {
        (imin - tl) * 2.0
    }
}

/// The C library's `tanh` with glibc 2.35's bits (fdlibm's, §8.1): the smoothness weight's
/// last bit decides the reruns (§4.5). `reproduce` needs it; the default uses it too until
/// `larmorx_core::math` has a correctly rounded `tanh` in double precision.
#[inline]
fn tanh(x: f64) -> f64 {
    larmorx_core::math::tanh_glibc(x)
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
        // the field of view `n·d` in float; an axis keeps its voxel size only when it exceeds r
        let fov = f64::from(vol.shape[a] as f32 * vol.voxel[a] as f32);
        if !(fov > r) {
            spacing[a] = 2.0 * r;
        }
    }
    let ds = spacing.iter().copied().fold(1.0f64, f64::min);
    let sampler = Sampler { vol, spacing, ds };
    let bt = larmorx_core::math::pow(opts.frac, 0.275);
    // the options are floats (§8.1): rmin in double, rmax's product in float
    let rmin = 3.33 * opts.smoothness;
    let rmax = f64::from(10.0f32 * opts.smoothness as f32);
    let e = (1.0 / rmin + 1.0 / rmax) / 2.0;
    let f = 6.0 / (1.0 / rmin - 1.0 / rmax);
    // the search depths are the options truncated to integers (§4.4)
    let (d1, d2) = (opts.d1.trunc(), opts.d2.trunc());
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
        // a rerun's smoothness factor, lowered linearly to 1 over the last quarter
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
                // §8.1, step by step
                let s = sub(m, p);
                let sigma = dot(s, n);
                let s_n = scale(n, sigma);
                let s_t = sub(s, s_n);
                let u1 = scale(s_t, 0.5);
                // 1/R from s_n·n computed again (not σ: they differ in the last bits)
                let rinv = (2.0 * dot(s_n, n).abs()) / l2;
                let mut f2 = (1.0 + tanh(f * (rinv - e))) * 0.5;
                if let Some(factor) = factor
                    && sigma > 0.0
                {
                    f2 = (f2 * factor).min(1.0);
                }
                let u2 = scale(s_n, f2);
                let f3 = intensity_term(&sampler, p, n, init, bt, opts.gradient, d1, d2);
                let u3 = scale(n, f3 * ((0.5 * 0.1) * l));
                // the three terms summed first, then added to p
                *out = add(p, add(add(u1, u2), u3));
            });
        std::mem::swap(&mut points, &mut next);
    }
    points
}

/// Whether two points agree within `1e-8` in every axis (FSL's test of a shared vertex).
#[inline]
fn same_point(a: V3, b: V3) -> bool {
    (a[0] - b[0]).abs() < 1e-8 && (a[1] - b[1]).abs() < 1e-8 && (a[2] - b[2]).abs() < 1e-8
}

/// The self-intersection total of `points` against the initial surface `initial` (§4.5,
/// §8.1): over ordered pairs `(i, j)`, `i ≠ j`, `j` not adjacent to `i` (no neighbour of `i`
/// at `j`'s coordinates within `1e-8`), with the squared distance below `ml·ml`, the sum of
/// `(a − b)²`, `a = |p_i − p_j| / ml`, `b = |o_i − o_j| / mlo`; one running sum, `i` then `j`
/// in order.
pub fn self_intersection(mesh: &Mesh, points: &[V3], initial: &[V3]) -> f64 {
    let ml = mesh.mean_neighbour_distance(points);
    let mlo = mesh.mean_neighbour_distance(initial);
    if !(ml > 0.0) || !ml.is_finite() {
        // no pair is closer than a distance that is not a number (observed: a total of 0)
        return 0.0;
    }
    let ml2 = ml * ml;
    // cells a hair wider than ml: a pair whose squared distance rounds below ml² is found
    let grid = Grid::new(points, ml * (1.0 + 1e-9));
    let terms: Vec<Vec<(u32, f64)>> = (0..points.len())
        .into_par_iter()
        .with_min_len(64)
        .map(|i| {
            let p = points[i];
            let nb = mesh.neighbours.row(i);
            let mut out = Vec::new();
            grid.near(p, |j| {
                if j as usize == i {
                    return;
                }
                let q = points[j as usize];
                let e = sub(p, q);
                if !(dot(e, e) < ml2) {
                    return;
                }
                if nb.iter().any(|&k| same_point(points[k as usize], q)) {
                    return;
                }
                let a = dist(p, q) / ml;
                let b = dist(initial[i], initial[j as usize]) / mlo;
                out.push((j, (a - b) * (a - b)));
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


/// Whether the surface self-intersects by the exact test of `-e` (§4.5): a sweep over the
/// vertices in increasing x, each bringing its triangles in their order; a triangle met for the
/// first time is tested against every triangle of the active list (in the order they were
/// first met) and then appended; met for the third time it leaves the list. A pair sharing a
/// vertex (coordinates within `1e-8`) is never tested. The sweep stops at the first pair that
/// intersects. Vertices of equal x keep their index order (FSL leaves them as an unstable
/// sort does; §8.1: this matters only when two vertices share an x).
pub fn intersects(mesh: &Mesh, points: &[V3]) -> bool {
    let tris = &mesh.triangles;
    let mut order: Vec<u32> = (0..points.len() as u32).collect();
    order.sort_by(|&a, &b| points[a as usize][0].total_cmp(&points[b as usize][0]));
    let mut met = vec![0u8; tris.len()];
    let mut active: Vec<u32> = Vec::new();
    for &v in &order {
        for &t in mesh.vertex_triangles.row(v as usize) {
            let ti = t as usize;
            met[ti] += 1;
            match met[ti] {
                1 => {
                    let a = tris[ti].map(|k| points[k as usize]);
                    for &u in &active {
                        let b = tris[u as usize].map(|k| points[k as usize]);
                        if shares_a_vertex(&a, &b) {
                            continue;
                        }
                        if pair_intersects(&a, &b) {
                            return true;
                        }
                    }
                    active.push(t);
                }
                3 => {
                    if let Some(k) = active.iter().position(|&x| x == t) {
                        active.remove(k);
                    }
                }
                _ => {}
            }
        }
    }
    false
}

/// Whether any vertex of `a` agrees with any vertex of `b` within `1e-8` in each axis.
fn shares_a_vertex(a: &[V3; 3], b: &[V3; 3]) -> bool {
    a.iter().any(|&p| b.iter().any(|&q| same_point(p, q)))
}

/// Whether an edge `(p0, p1)` (with `p = p0 − A0`, `d = p1 − p0`) crosses the triangle
/// `(A0, A1, A2)` strictly inside: the edge's point `P` on the plane, and the three cross
/// products `n1 = v1 × P`, `n2 = P × v2`, `n3 = v3 × (P − v1)` with dot products all of one
/// strict sign (§4.5).
fn edge_hits(t: &[V3; 3], p0: V3, p1: V3) -> bool {
    let v1 = sub(t[1], t[0]);
    let v2 = sub(t[2], t[0]);
    let v3 = sub(t[2], t[1]);
    let big_v = cross(v1, v2);
    let p = sub(p0, t[0]);
    let d = sub(p1, p0);
    let delta = dot(d, big_v);
    if delta == 0.0 {
        return false;
    }
    let lambda = -dot(p, big_v) / delta;
    let pp = add(p, scale(d, lambda));
    let n1 = cross(v1, pp);
    let n2 = cross(pp, v2);
    let n3 = cross(v3, sub(pp, v1));
    let (a, b, c) = (dot(n1, n3), dot(n2, n3), dot(n1, n2));
    (a > 0.0 && b > 0.0 && c > 0.0) || (a < 0.0 && b < 0.0 && c < 0.0)
}

/// FSL's predicate for the pair `T = a` (met later) and `U = b` (§4.5, step 3), in double.
fn pair_intersects(a: &[V3; 3], b: &[V3; 3]) -> bool {
    let n = cross(sub(a[0], a[1]), sub(a[0], a[2]));
    let sigma = b.map(|q| dot(n, sub(q, a[0])));
    if !(sigma[0] * sigma[1] < 0.0) && !(sigma[0] * sigma[2] < 0.0) {
        return false;
    }
    for i in 0..3 {
        let j = (i + 1) % 3;
        if sigma[i] * sigma[j] < 0.0 && edge_hits(a, b[i], b[j]) {
            return true;
        }
    }
    let m = cross(sub(b[0], b[1]), sub(b[0], b[2]));
    let tau = a.map(|q| dot(m, sub(q, b[0])));
    for i in 0..3 {
        let j = (i + 1) % 3;
        if tau[i] * tau[j] < 0.0 && edge_hits(b, a[i], a[j]) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_and_touching_pairs() {
        // T in the z = 0 plane; U pierces it through its interior
        let t = [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0]];
        let u = [[1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [3.0, 3.0, 1.0]];
        assert!(pair_intersects(&t, &u));
        // the other way round: t's first vertex lies on u's plane (x = y), so FSL's test says
        // no; listed from a vertex off that plane, yes
        assert!(!pair_intersects(&u, &t));
        let t2 = [[4.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 0.0]];
        assert!(pair_intersects(&u, &t2));
        // U's first vertex on T's plane: never an intersection (strict signs)
        let u0 = [[1.0, 1.0, 0.0], [1.0, 1.0, 1.0], [1.5, 1.0, -1.0]];
        assert!(!pair_intersects(&t, &u0));
        // apart
        let far = [[10.0, 10.0, -1.0], [10.0, 10.0, 1.0], [11.0, 10.0, 1.0]];
        assert!(!pair_intersects(&t, &far));
        // coplanar: not an intersection
        let flat = [[1.0, 1.0, 0.0], [2.0, 1.0, 0.0], [1.0, 2.0, 0.0]];
        assert!(!pair_intersects(&t, &flat));
    }

    #[test]
    fn the_sphere_does_not_intersect() {
        let m = Mesh::sphere(4);
        assert!(!intersects(&m, &m.points));
        // a vertex pushed through the opposite side folds the surface
        let mut p = m.points.clone();
        p[0] = [-2.0 * p[0][0], -2.0 * p[0][1], -2.0 * p[0][2]];
        assert!(intersects(&m, &p));
    }
}
