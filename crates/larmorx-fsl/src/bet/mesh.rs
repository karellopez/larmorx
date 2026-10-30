// SPDX-License-Identifier: Apache-2.0
//! The tessellated sphere BET deforms (`specs/bet.md` §4.4): an icosahedron subdivided four
//! times (2562 vertices, 5120 triangles), with its vertex and triangle numbering, adjacency and
//! the per-vertex quantities of each deformation step.
//!
//! The numbering is the one FSL's mesh files show (observed from `bet2 -e`): the twelve
//! vertices of the icosahedron first, in the order of [`ICOSAHEDRON`], its twenty faces in the
//! order of [`FACES`], and each subdivision appending, for every triangle `(p0, p1, p2)` in
//! order, the midpoints `m01`, `m12`, `m20` not made yet, and the four triangles
//! `(m01, m12, m20)`, `(p2, m20, m12)`, `(p1, m12, m01)`, `(p0, m01, m20)`. A triangle
//! `(a, b, c)` is written to a mesh file as `c a b`, and its outward normal is
//! `(c - a) × (b - a)`.

use rayon::prelude::*;

/// `τ` and `o` of the icosahedron's vertices (§4.4).
const TAU: f64 = 0.8506508084;
const ONE: f64 = 0.5257311121;

/// The icosahedron's twelve vertices, in FSL's order (observed).
pub const ICOSAHEDRON: [[f64; 3]; 12] = [
    [TAU, ONE, 0.0],
    [-TAU, ONE, 0.0],
    [-TAU, -ONE, 0.0],
    [TAU, -ONE, 0.0],
    [ONE, 0.0, TAU],
    [ONE, 0.0, -TAU],
    [-ONE, 0.0, -TAU],
    [-ONE, 0.0, TAU],
    [0.0, TAU, ONE],
    [0.0, -TAU, ONE],
    [0.0, -TAU, -ONE],
    [0.0, TAU, -ONE],
];

/// The icosahedron's twenty faces, in FSL's order (observed).
pub const FACES: [[u32; 3]; 20] = [
    [7, 8, 4],
    [9, 7, 4],
    [11, 6, 5],
    [6, 10, 5],
    [3, 4, 0],
    [5, 3, 0],
    [1, 7, 2],
    [6, 1, 2],
    [11, 0, 8],
    [1, 11, 8],
    [3, 10, 9],
    [10, 2, 9],
    [0, 4, 8],
    [5, 0, 11],
    [3, 9, 4],
    [10, 3, 5],
    [1, 8, 7],
    [11, 1, 6],
    [9, 2, 7],
    [2, 10, 6],
];

pub type V3 = [f64; 3];

#[inline]
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
#[inline]
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
#[inline]
pub fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
#[inline]
pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
#[inline]
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
#[inline]
pub fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}
#[inline]
pub fn dist(a: V3, b: V3) -> f64 {
    norm(sub(a, b))
}
/// `a / |a|` (a zero vector stays zero).
#[inline]
pub fn normalised(a: V3) -> V3 {
    let n = norm(a);
    if n > 0.0 { scale(a, 1.0 / n) } else { a }
}

/// A list of lists in one buffer (compressed rows).
#[derive(Clone, Debug, Default)]
pub struct Rows {
    pub start: Vec<u32>,
    pub items: Vec<u32>,
}

impl Rows {
    fn of(lists: &[Vec<u32>]) -> Rows {
        let mut start = Vec::with_capacity(lists.len() + 1);
        let mut items = Vec::new();
        start.push(0);
        for l in lists {
            items.extend_from_slice(l);
            start.push(items.len() as u32);
        }
        Rows { start, items }
    }
    #[inline]
    pub fn row(&self, i: usize) -> &[u32] {
        &self.items[self.start[i] as usize..self.start[i + 1] as usize]
    }
}

/// A triangle mesh with its adjacency.
#[derive(Clone, Debug)]
pub struct Mesh {
    pub points: Vec<V3>,
    pub triangles: Vec<[u32; 3]>,
    /// Each vertex's neighbours (the vertices it shares an edge with), in the order the
    /// triangles meet them.
    pub neighbours: Rows,
    /// Each vertex's triangles, in creation order.
    pub vertex_triangles: Rows,
}

impl Mesh {
    /// A mesh from points and triangles, its adjacency computed.
    pub fn new(points: Vec<V3>, triangles: Vec<[u32; 3]>) -> Mesh {
        let n = points.len();
        let mut nb: Vec<Vec<u32>> = vec![Vec::new(); n];
        let mut vt: Vec<Vec<u32>> = vec![Vec::new(); n];
        for (t, tri) in triangles.iter().enumerate() {
            for k in 0..3 {
                let v = tri[k] as usize;
                if v >= n {
                    continue;
                }
                vt[v].push(t as u32);
                for o in [tri[(k + 1) % 3], tri[(k + 2) % 3]] {
                    if (o as usize) < n && !nb[v].contains(&o) {
                        nb[v].push(o);
                    }
                }
            }
        }
        Mesh {
            points,
            triangles,
            neighbours: Rows::of(&nb),
            vertex_triangles: Rows::of(&vt),
        }
    }

    /// The unit sphere: the icosahedron subdivided `levels` times, every vertex projected
    /// onto the sphere after each subdivision.
    pub fn sphere(levels: usize) -> Mesh {
        let mut points: Vec<V3> = ICOSAHEDRON.to_vec();
        let mut triangles: Vec<[u32; 3]> = FACES.to_vec();
        for _ in 0..levels {
            let (p, t) = subdivide(&points, &triangles);
            points = p.into_iter().map(normalised).collect();
            triangles = t;
        }
        Mesh::new(points, triangles)
    }

    /// The same triangles, each split into four at its edges' midpoints (betsurf's
    /// retessellation, §6.5), not projected.
    pub fn retessellated(&self) -> Mesh {
        let (p, t) = subdivide(&self.points, &self.triangles);
        Mesh::new(p, t)
    }

    /// The vertices' centroid.
    pub fn centroid(&self) -> V3 {
        let mut c = [0.0; 3];
        for p in &self.points {
            c = add(c, *p);
        }
        scale(c, 1.0 / self.points.len() as f64)
    }

    /// The mean, over the vertices, of each one's mean distance to its neighbours (`l`, §4.4).
    pub fn mean_neighbour_distance(&self, points: &[V3]) -> f64 {
        let per: Vec<f64> = (0..points.len())
            .into_par_iter()
            .with_min_len(256)
            .map(|i| {
                let row = self.neighbours.row(i);
                if row.is_empty() {
                    return 0.0;
                }
                let mut s = 0.0;
                for &j in row {
                    s += dist(points[i], points[j as usize]);
                }
                s / row.len() as f64
            })
            .collect();
        per.iter().sum::<f64>() / points.len().max(1) as f64
    }

    /// Each triangle's (non-normalised) outward normal at `points`.
    pub fn triangle_normals(&self, points: &[V3]) -> Vec<V3> {
        self.triangles
            .par_iter()
            .with_min_len(512)
            .map(|t| {
                let (a, b, c) = (
                    points[t[0] as usize],
                    points[t[1] as usize],
                    points[t[2] as usize],
                );
                cross(sub(c, a), sub(b, a))
            })
            .collect()
    }

    /// The unit normal of vertex `i`: its triangles' normals summed, normalised.
    #[inline]
    pub fn vertex_normal(&self, i: usize, tri_normals: &[V3]) -> V3 {
        let mut s = [0.0; 3];
        for &t in self.vertex_triangles.row(i) {
            s = add(s, tri_normals[t as usize]);
        }
        normalised(s)
    }

    /// The mean of vertex `i`'s neighbours.
    #[inline]
    pub fn neighbour_mean(&self, i: usize, points: &[V3]) -> V3 {
        let row = self.neighbours.row(i);
        let mut s = [0.0; 3];
        for &j in row {
            s = add(s, points[j as usize]);
        }
        scale(s, 1.0 / row.len().max(1) as f64)
    }
}

fn subdivide(points: &[V3], triangles: &[[u32; 3]]) -> (Vec<V3>, Vec<[u32; 3]>) {
    use std::collections::HashMap;
    let mut points = points.to_vec();
    let mut mids: HashMap<(u32, u32), u32> = HashMap::with_capacity(triangles.len() * 2);
    let mut out = Vec::with_capacity(triangles.len() * 4);
    for &[p0, p1, p2] in triangles {
        let mut mid = |a: u32, b: u32| -> u32 {
            let key = (a.min(b), a.max(b));
            *mids.entry(key).or_insert_with(|| {
                let m = scale(add(points[a as usize], points[b as usize]), 0.5);
                points.push(m);
                (points.len() - 1) as u32
            })
        };
        let m01 = mid(p0, p1);
        let m12 = mid(p1, p2);
        let m20 = mid(p2, p0);
        out.push([m01, m12, m20]);
        out.push([p2, m20, m12]);
        out.push([p1, m12, m01]);
        out.push([p0, m01, m20]);
    }
    (points, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_counts_and_orientation() {
        let m = Mesh::sphere(4);
        assert_eq!(m.points.len(), 2562);
        assert_eq!(m.triangles.len(), 5120);
        let normals = m.triangle_normals(&m.points);
        for (t, n) in m.triangles.iter().zip(&normals) {
            let c = scale(
                add(
                    add(m.points[t[0] as usize], m.points[t[1] as usize]),
                    m.points[t[2] as usize],
                ),
                1.0 / 3.0,
            );
            assert!(dot(*n, c) > 0.0, "every normal points outwards");
        }
        for i in 0..m.points.len() {
            let k = m.neighbours.row(i).len();
            assert!(k == 5 || k == 6, "{k}");
            assert!((norm(m.points[i]) - 1.0).abs() < 1e-12);
        }
        // The first triangles as FSL's mesh files show them (written `c a b`).
        let w = |t: [u32; 3]| [t[2], t[0], t[1]];
        assert_eq!(w(m.triangles[0]), [644, 642, 643]);
        assert_eq!(w(m.triangles[1]), [643, 164, 644]);
        assert_eq!(w(m.triangles[7]), [646, 44, 645]);
    }
}
