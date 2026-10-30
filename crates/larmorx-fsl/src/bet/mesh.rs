// SPDX-License-Identifier: Apache-2.0
//! The tessellated sphere BET deforms (`specs/bet.md` §4.4): an icosahedron subdivided four
//! times (2562 vertices, 5120 triangles), with its vertex and triangle numbering, adjacency and
//! the per-vertex quantities of each deformation step.
//!
//! The numbering is the one FSL's mesh files show (observed from `bet2 -e`; §4.4): the twelve
//! vertices of the icosahedron first, in the order of [`ICOSAHEDRON`], its twenty faces in the
//! order of [`FACES`], and each subdivision visiting the triangles `(v0, v1, v2)` in order,
//! making the midpoints `m0 = (v1 + v2)/2`, `m1 = (v0 + v2)/2`, `m2 = (v0 + v1)/2` not made yet
//! (numbered in that order) and the four triangles `(m2, m0, m1)`, `(m1, v0, m2)`,
//! `(m0, v2, m1)`, `(m2, v1, m0)`. Triangles are held in the mesh file's order `(v0, v1, v2)`;
//! a triangle's outward normal is `(v2 - v0) × (v1 - v0)`.
//!
//! The arithmetic is §8.1's, operation by operation (the sums in the order given, the normal
//! and the mean by three divisions): the reruns of a self-intersecting surface amplify any
//! last-bit difference (§4.5).

use rayon::prelude::*;

/// `τ` and `o` of the icosahedron's vertices (§4.4).
const TAU: f64 = 0.8506508084;
const ONE: f64 = 0.5257311121;

/// The icosahedron's twelve vertices, in FSL's order (§4.4).
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

/// The icosahedron's twenty faces `(v0, v1, v2)`, in FSL's order (§4.4).
pub const FACES: [[u32; 3]; 20] = [
    [4, 7, 8],
    [4, 9, 7],
    [5, 11, 6],
    [5, 6, 10],
    [0, 3, 4],
    [0, 5, 3],
    [2, 1, 7],
    [2, 6, 1],
    [8, 11, 0],
    [8, 1, 11],
    [9, 3, 10],
    [9, 10, 2],
    [8, 0, 4],
    [11, 5, 0],
    [4, 3, 9],
    [5, 10, 3],
    [7, 1, 8],
    [6, 11, 1],
    [7, 9, 2],
    [6, 2, 10],
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
/// `(a / s)` componentwise (three divisions, not products with `1/s`).
#[inline]
pub fn divide(a: V3, s: f64) -> V3 {
    [a[0] / s, a[1] / s, a[2] / s]
}
/// `(a_x b_x + a_y b_y) + a_z b_z`.
#[inline]
pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
/// `a × b = (a_y b_z − a_z b_y, b_x a_z − b_z a_x, a_x b_y − b_x a_y)` (§4.4).
#[inline]
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        b[0] * a[2] - b[2] * a[0],
        a[0] * b[1] - b[0] * a[1],
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
/// `a · (1/|a|)` (the reciprocal, then three products: the sphere's projection, §4.4; a zero
/// vector stays zero).
#[inline]
pub fn normalised(a: V3) -> V3 {
    let n = norm(a);
    if n > 0.0 { scale(a, 1.0 / n) } else { a }
}
/// `a / |a|` by three divisions (the vertex normal, §8.1; a zero vector stays zero).
#[inline]
pub fn unit(a: V3) -> V3 {
    let n = norm(a);
    if n > 0.0 { divide(a, n) } else { a }
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
    /// The triangles `(v0, v1, v2)`, in the mesh file's order.
    pub triangles: Vec<[u32; 3]>,
    /// Each vertex's neighbours (the vertices it shares an edge with), in §4.4's order.
    pub neighbours: Rows,
    /// Each vertex's triangles, in the order of the triangle list.
    pub vertex_triangles: Rows,
}

/// Moves `o` to the end of `list` (taken out of its place if there, never listed twice).
fn move_to_end(list: &mut Vec<u32>, o: u32) {
    if let Some(k) = list.iter().position(|&x| x == o) {
        list.remove(k);
    }
    list.push(o);
}

impl Mesh {
    /// A mesh from points and triangles, its adjacency computed: each vertex's triangles in the
    /// list's order; its neighbours by §4.4's rule (for each `(v0, v1, v2)` in order, `v1` then
    /// `v2` moved to the end of `v0`'s list, `v2` then `v0` to `v1`'s, `v0` then `v1` to `v2`'s).
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
                    if (o as usize) < n {
                        move_to_end(&mut nb[v], o);
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

    /// The vertices' centroid, `(Σ p)·(1/V)` (§4.6).
    pub fn centroid(&self) -> V3 {
        let mut c = [0.0; 3];
        for p in &self.points {
            c = add(c, *p);
        }
        scale(c, 1.0 / self.points.len() as f64)
    }

    /// The mean, over the vertices, of each one's mean distance to its neighbours (`l`, §8.1):
    /// each vertex's distances summed in its neighbours' order and divided by their count,
    /// these summed over the vertices in order and divided by the vertex count.
    pub fn mean_neighbour_distance(&self, points: &[V3]) -> f64 {
        let per: Vec<f64> = (0..points.len())
            .into_par_iter()
            .with_min_len(256)
            .map(|i| {
                let row = self.neighbours.row(i);
                let mut s = 0.0;
                for &j in row {
                    s += dist(points[j as usize], points[i]);
                }
                s / row.len() as f64
            })
            .collect();
        let mut total = 0.0;
        for x in per {
            total += x;
        }
        total / points.len() as f64
    }

    /// Each triangle's (non-normalised) outward normal at `points`, `(v2 − v0) × (v1 − v0)`.
    pub fn triangle_normals(&self, points: &[V3]) -> Vec<V3> {
        self.triangles
            .par_iter()
            .with_min_len(512)
            .map(|t| {
                let (v0, v1, v2) = (
                    points[t[0] as usize],
                    points[t[1] as usize],
                    points[t[2] as usize],
                );
                cross(sub(v2, v0), sub(v1, v0))
            })
            .collect()
    }

    /// The unit normal of vertex `i`: its triangles' normals summed in their order (from 0),
    /// divided by the sum's length (§8.1).
    #[inline]
    pub fn vertex_normal(&self, i: usize, tri_normals: &[V3]) -> V3 {
        let mut s = [0.0; 3];
        for &t in self.vertex_triangles.row(i) {
            s = add(s, tri_normals[t as usize]);
        }
        unit(s)
    }

    /// The mean of vertex `i`'s neighbours: their sum in order, divided by their count.
    #[inline]
    pub fn neighbour_mean(&self, i: usize, points: &[V3]) -> V3 {
        let row = self.neighbours.row(i);
        let mut s = [0.0; 3];
        for &j in row {
            s = add(s, points[j as usize]);
        }
        divide(s, row.len() as f64)
    }
}

fn subdivide(points: &[V3], triangles: &[[u32; 3]]) -> (Vec<V3>, Vec<[u32; 3]>) {
    use std::collections::HashMap;
    let mut points = points.to_vec();
    let mut mids: HashMap<(u32, u32), u32> = HashMap::with_capacity(triangles.len() * 2);
    let mut out = Vec::with_capacity(triangles.len() * 4);
    for &[v0, v1, v2] in triangles {
        // a midpoint of the same edge has the same coordinates (the sum is commutative), and
        // the midpoints of distinct edges lie far apart: the edge identifies it (§4.4's 1e-8)
        let mut mid = |a: u32, b: u32| -> u32 {
            let key = (a.min(b), a.max(b));
            *mids.entry(key).or_insert_with(|| {
                let m = divide(add(points[a as usize], points[b as usize]), 2.0);
                points.push(m);
                (points.len() - 1) as u32
            })
        };
        let m0 = mid(v1, v2);
        let m1 = mid(v0, v2);
        let m2 = mid(v0, v1);
        out.push([m2, m0, m1]);
        out.push([m1, v0, m2]);
        out.push([m0, v2, m1]);
        out.push([m2, v1, m0]);
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
        // The first triangles as FSL's mesh files show them.
        assert_eq!(m.triangles[0], [644, 642, 643]);
        assert_eq!(m.triangles[1], [643, 164, 644]);
        assert_eq!(m.triangles[7], [646, 44, 645]);
    }

    #[test]
    fn neighbours_moved_to_the_end() {
        // (0, 1, 2) then (0, 2, 3): 0's list 1, 2 then 2 moved, 3: [1, 2, 3]; 2's list 0, 1,
        // then 3, 0 (0 moved to the end): [1, 3, 0]
        let m = Mesh::new(vec![[0.0; 3]; 4], vec![[0, 1, 2], [0, 2, 3]]);
        assert_eq!(m.neighbours.row(0), &[1, 2, 3]);
        assert_eq!(m.neighbours.row(2), &[1, 3, 0]);
        assert_eq!(m.neighbours.row(3), &[0, 2]);
    }
}
