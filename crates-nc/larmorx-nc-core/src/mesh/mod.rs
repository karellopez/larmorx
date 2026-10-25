// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! FSL's triangle meshes (fsl-meshclass 2111.0: `point`, `mpoint`, `triangle`, `mesh`,
//! `pt_special`), as BET and betsurf deform them.
//!
//! The results depend on the order of sums over a vertex's triangles and neighbours, so the
//! C++ lists are kept as lists in the same order: a triangle registers itself with its three
//! vertices when created (after removing and re-adding the vertices from each other's
//! neighbour lists), unregisters when deleted, and a copied mesh rebuilds every list by creating
//! its triangles again in the original's order. Points live in the mesh's point vector (their
//! number is their index); triangles in an arena, with the mesh's triangle list holding their
//! ids in list order.

pub mod load;
pub mod profile;

use std::fmt::Write as _;

use crate::cnum;

/// A point or a vector (meshclass's `Pt` and `Vec`, both three doubles).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl V3 {
    pub const fn new(x: f64, y: f64, z: f64) -> V3 {
        V3 { x, y, z }
    }

    /// `Vec::norm`.
    #[inline]
    pub fn norm(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// `Vec::normalize` (unchanged when the norm is 0).
    #[inline]
    pub fn normalize(&mut self) {
        let n = self.norm();
        if n != 0.0 {
            self.x /= n;
            self.y /= n;
            self.z /= n;
        }
    }

    /// `operator|`: the dot product.
    #[inline]
    pub fn dot(self, o: V3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// `operator*(Vec, Vec)`: the cross product, as meshclass writes it.
    #[inline]
    pub fn cross(self, v2: V3) -> V3 {
        V3::new(
            self.y * v2.z - self.z * v2.y,
            v2.x * self.z - v2.z * self.x,
            self.x * v2.y - v2.x * self.y,
        )
    }

    /// `d * v` (and `v * d`).
    #[inline]
    pub fn scale(self, d: f64) -> V3 {
        V3::new(d * self.x, d * self.y, d * self.z)
    }

    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, o: V3) -> V3 {
        V3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, o: V3) -> V3 {
        V3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }

    /// `Vec / d`: unchanged (with meshclass's "division by zero" message) when `d` is 0.
    pub fn div(self, d: f64, err: &mut String) -> V3 {
        if d != 0.0 {
            V3::new(self.x / d, self.y / d, self.z / d)
        } else {
            err.push_str("division by zero\n");
            self
        }
    }

    /// `Pt::operator==`: every coordinate within 1e-8.
    #[inline]
    pub fn pt_eq(self, p: V3) -> bool {
        (self.x - p.x).abs() < 1e-8 && (self.y - p.y).abs() < 1e-8 && (self.z - p.z).abs() < 1e-8
    }
}

/// `Mpoint`.
#[derive(Clone, Debug)]
pub struct MPoint {
    pub coord: V3,
    pub update_coord: V3,
    /// `_triangles` (triangle ids).
    pub triangles: Vec<usize>,
    /// `_neighbours` (point ids).
    pub neighbours: Vec<usize>,
    pub no: i32,
    pub value: f32,
    /// `data` (a list of doubles).
    pub data: Vec<f64>,
}

impl MPoint {
    pub fn new(coord: V3, no: i32, value: f32) -> MPoint {
        MPoint {
            coord,
            update_coord: V3::default(),
            triangles: Vec::new(),
            neighbours: Vec::new(),
            no,
            value,
            data: Vec::new(),
        }
    }
}

/// `Triangle`.
#[derive(Clone, Debug)]
pub struct Triangle {
    pub v: [usize; 3],
    pub value: f32,
    pub data: Vec<f64>,
    pub oriented: bool,
}

/// `Mesh`.
#[derive(Debug, Default)]
pub struct Mesh {
    /// `_points`.
    pub points: Vec<MPoint>,
    /// The triangle arena (deleted triangles stay, unlisted).
    pub tris: Vec<Triangle>,
    /// `_triangles`: the ids of the listed triangles, in list order.
    pub tri_list: Vec<usize>,
}

/// `list::remove(value)`: every element equal to `value`.
#[inline]
fn list_remove(list: &mut Vec<usize>, value: usize) {
    list.retain(|&x| x != value);
}

impl Mesh {
    pub fn nvertices(&self) -> usize {
        self.points.len()
    }

    /// `new Triangle(p1, p2, p3, val)`: registers with its vertices; returns its id (not yet in
    /// the mesh's list).
    pub fn new_triangle(&mut self, p1: usize, p2: usize, p3: usize, value: f32) -> usize {
        let id = self.tris.len();
        self.tris.push(Triangle {
            v: [p1, p2, p3],
            value,
            data: Vec::new(),
            oriented: false,
        });
        self.points[p1].triangles.push(id);
        self.points[p2].triangles.push(id);
        self.points[p3].triangles.push(id);
        list_remove(&mut self.points[p1].neighbours, p2);
        list_remove(&mut self.points[p1].neighbours, p3);
        list_remove(&mut self.points[p2].neighbours, p3);
        list_remove(&mut self.points[p2].neighbours, p1);
        list_remove(&mut self.points[p3].neighbours, p1);
        list_remove(&mut self.points[p3].neighbours, p2);
        self.points[p1].neighbours.push(p2);
        self.points[p1].neighbours.push(p3);
        self.points[p2].neighbours.push(p3);
        self.points[p2].neighbours.push(p1);
        self.points[p3].neighbours.push(p1);
        self.points[p3].neighbours.push(p2);
        id
    }

    /// `delete t`: unregisters from its vertices.
    fn delete_triangle(&mut self, t: usize) {
        let v = self.tris[t].v;
        for p in v {
            list_remove(&mut self.points[p].triangles, t);
        }
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.tris.clear();
        self.tri_list.clear();
    }

    /// `Triangle::normal`: `(v2 - v0) * (v1 - v0)`.
    #[inline]
    pub fn tri_normal(&self, t: usize) -> V3 {
        let [a, b, c] = self.tris[t].v;
        let p0 = self.points[a].coord;
        self.points[c]
            .coord
            .sub(p0)
            .cross(self.points[b].coord.sub(p0))
    }

    /// `Triangle::centroid`.
    pub fn tri_centroid(&self, t: usize) -> V3 {
        let [a, b, c] = self.tris[t].v;
        let (p, q, r) = (
            self.points[a].coord,
            self.points[b].coord,
            self.points[c].coord,
        );
        V3::new(
            (p.x + q.x + r.x) / 3.0,
            (p.y + q.y + r.y) / 3.0,
            (p.z + q.z + r.z) / 3.0,
        )
    }

    /// `Mpoint::local_normal`: the sum of its triangles' normals, normalized.
    #[inline]
    pub fn local_normal(&self, p: usize) -> V3 {
        let mut v = V3::default();
        for &t in &self.points[p].triangles {
            let n = self.tri_normal(t);
            v.x += n.x;
            v.y += n.y;
            v.z += n.z;
        }
        v.normalize();
        v
    }

    /// `Mpoint::medium_neighbours`.
    #[inline]
    pub fn medium_neighbours(&self, p: usize) -> V3 {
        let mut r = V3::default();
        let nb = &self.points[p].neighbours;
        for &n in nb {
            let c = self.points[n].coord;
            r.x += c.x;
            r.y += c.y;
            r.z += c.z;
        }
        let counter = nb.len() as i32;
        V3::new(
            r.x / f64::from(counter),
            r.y / f64::from(counter),
            r.z / f64::from(counter),
        )
    }

    /// `Mpoint::difference_vector`.
    #[inline]
    pub fn difference_vector(&self, p: usize) -> V3 {
        self.medium_neighbours(p).sub(self.points[p].coord)
    }

    /// `Mpoint::medium_distance_of_neighbours`.
    pub fn medium_distance_of_neighbours(&self, p: usize) -> f64 {
        let mut l = 0.0;
        let me = self.points[p].coord;
        for &n in &self.points[p].neighbours {
            l += self.points[n].coord.sub(me).norm();
        }
        l / self.points[p].neighbours.len() as f64
    }

    /// `operator<(Mpoint, Mpoint)`: whether a neighbour of `p1` has `p2`'s coordinates
    /// (within 1e-8).
    fn adjacent(&self, p1: usize, p2: usize) -> bool {
        let c2 = self.points[p2].coord;
        let mut r = false;
        for &n in &self.points[p1].neighbours {
            let c = self.points[n].coord;
            if (c2.x - c.x).abs() < 1e-8 && (c2.y - c.y).abs() < 1e-8 && (c2.z - c.z).abs() < 1e-8 {
                r = true;
            }
        }
        r
    }

    /// `Mesh::update`.
    pub fn update(&mut self) {
        for p in &mut self.points {
            p.coord = p.update_coord;
        }
    }

    /// `Mesh::translation`.
    pub fn translation(&mut self, x: f64, y: f64, z: f64) {
        for p in &mut self.points {
            p.coord = V3::new(p.coord.x + x, p.coord.y + y, p.coord.z + z);
        }
    }

    /// `Mesh::rescale(t, Pt)` and `rescale(t, x, y, z)`: `c + t * (p - c)`.
    pub fn rescale(&mut self, t: f64, c: V3) {
        for p in &mut self.points {
            p.coord = c.add(p.coord.sub(c).scale(t));
        }
    }

    /// The copy constructor (`Mesh(const Mesh&)`): points with their values; the triangles
    /// created again in list order.
    pub fn copy(&self) -> Mesh {
        let mut m = Mesh::default();
        for p in &self.points {
            m.points.push(MPoint::new(p.coord, p.no, p.value));
        }
        for &t in &self.tri_list {
            let v = self.tris[t].v.map(|i| self.points[i].no as usize);
            let id = m.new_triangle(v[0], v[1], v[2], 0.0);
            m.tri_list.push(id);
        }
        m
    }

    /// The assignment (`Mesh::operator=`): like the copy, but the points keep their `data` and
    /// not their values.
    pub fn assign(&mut self, src: &Mesh) {
        let mut m = Mesh::default();
        for p in &src.points {
            let mut q = MPoint::new(p.coord, p.no, 0.0);
            q.data = p.data.clone();
            m.points.push(q);
        }
        for &t in &src.tri_list {
            let v = src.tris[t].v.map(|i| src.points[i].no as usize);
            let id = m.new_triangle(v[0], v[1], v[2], 0.0);
            m.tri_list.push(id);
        }
        *self = m;
    }

    /// `Mesh::retessellate`: each triangle in four, the new vertices on the edges' midpoints
    /// (shared between triangles when their coordinates agree within 1e-8).
    pub fn retessellate(&mut self) {
        let tr = self.tri_list.clone();
        let mut added: Vec<usize> = Vec::new();
        for &t in &tr {
            let [v0, v1, v2] = self.tris[t].v;
            let (c0, c1, c2) = (
                self.points[v0].coord,
                self.points[v1].coord,
                self.points[v2].coord,
            );
            let pt0 = V3::new(
                (c1.x + c2.x) / 2.0,
                (c1.y + c2.y) / 2.0,
                (c1.z + c2.z) / 2.0,
            );
            let pt2 = V3::new(
                (c0.x + c1.x) / 2.0,
                (c0.y + c1.y) / 2.0,
                (c0.z + c1.z) / 2.0,
            );
            let pt1 = V3::new(
                (c0.x + c2.x) / 2.0,
                (c0.y + c2.y) / 2.0,
                (c0.z + c2.z) / 2.0,
            );
            let (mut p0, mut p1, mut p2) = (usize::MAX, usize::MAX, usize::MAX);
            let (mut b0, mut b1, mut b2) = (true, true, true);
            for &mi in &added {
                let current = self.points[mi].coord;
                if pt0.pt_eq(current) {
                    b0 = false;
                    p0 = mi;
                }
                if pt1.pt_eq(current) {
                    b1 = false;
                    p1 = mi;
                }
                if pt2.pt_eq(current) {
                    b2 = false;
                    p2 = mi;
                }
            }
            let mut count = 0;
            let n = self.nvertices();
            // new Mpoint(pt, nvertices() + count): created, pushed into _points below
            let mut pending: Vec<(usize, V3, i32)> = Vec::new();
            if b0 {
                p0 = n + count;
                pending.push((p0, pt0, (n + count) as i32));
                count += 1;
            }
            if b1 {
                p1 = n + count;
                pending.push((p1, pt1, (n + count) as i32));
                count += 1;
            }
            if b2 {
                p2 = n + count;
                pending.push((p2, pt2, (n + count) as i32));
            }
            // The points must exist before the triangles reference them; their order in
            // _points (p0, p1, p2) matches their numbers.
            for (_, pt, no) in &pending {
                self.points.push(MPoint::new(*pt, *no, 0.0));
            }
            let t0 = self.new_triangle(p2, p0, p1, 0.0);
            let t1 = self.new_triangle(p1, v0, p2, 0.0);
            let t2 = self.new_triangle(p0, v2, p1, 0.0);
            let t3 = self.new_triangle(p2, v1, p0, 0.0);
            self.tri_list.extend_from_slice(&[t0, t1, t2, t3]);
            for (id, _, _) in &pending {
                added.push(*id);
            }
            list_remove(&mut self.points[v0].neighbours, v1);
            list_remove(&mut self.points[v0].neighbours, v2);
            list_remove(&mut self.points[v1].neighbours, v0);
            list_remove(&mut self.points[v1].neighbours, v2);
            list_remove(&mut self.points[v2].neighbours, v1);
            list_remove(&mut self.points[v2].neighbours, v0);
        }
        for &t in &tr {
            list_remove(&mut self.tri_list, t);
            self.delete_triangle(t);
        }
    }

    /// `make_mesh_from_icosa(n, m)`.
    pub fn icosa(n: i32) -> Mesh {
        let mut m = Mesh::default();
        const TAU: f64 = 0.8506508084;
        const ONE: f64 = 0.5257311121;
        let pts = [
            (TAU, ONE, 0.0),
            (-TAU, ONE, 0.0),
            (-TAU, -ONE, 0.0),
            (TAU, -ONE, 0.0),
            (ONE, 0.0, TAU),
            (ONE, 0.0, -TAU),
            (-ONE, 0.0, -TAU),
            (-ONE, 0.0, TAU),
            (0.0, TAU, ONE),
            (0.0, -TAU, ONE),
            (0.0, -TAU, -ONE),
            (0.0, TAU, -ONE),
        ];
        for (i, &(x, y, z)) in pts.iter().enumerate() {
            m.points.push(MPoint::new(V3::new(x, y, z), i as i32, 0.0));
        }
        let (za, zb, zc, zd, ya, yb, yc, yd, xa, xb, xc, xd) =
            (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11);
        let tris = [
            (ya, xa, yd),
            (ya, yd, xb),
            (yb, yc, xd),
            (yb, xc, yc),
            (za, ya, zd),
            (za, zd, yb),
            (zc, yd, zb),
            (zc, zb, yc),
            (xa, za, xd),
            (xa, xd, zb),
            (xb, xc, zd),
            (xb, zc, xc),
            (xa, ya, za),
            (xd, za, yb),
            (ya, xb, zd),
            (yb, zd, xc),
            (yd, xa, zb),
            (yc, zb, xd),
            (yd, zc, xb),
            (yc, xc, zc),
        ];
        for &(a, b, c) in &tris {
            let id = m.new_triangle(a, b, c, 0.0);
            m.tri_list.push(id);
        }
        for &t in &m.tri_list {
            m.tris[t].v.swap(1, 2);
        }
        for _ in 1..n {
            m.retessellate();
            for p in &mut m.points {
                let c = p.coord;
                let norm = (c.x * c.x + c.y * c.y + c.z * c.z).sqrt();
                let f = 1.0 / norm;
                p.update_coord = V3::new(c.x * f, c.y * f, c.z * f);
            }
            m.update();
        }
        m
    }

    /// `Mesh::self_intersection(original)`: the sum, over pairs of vertices closer than the
    /// mean edge length that are not neighbours, of the squared change of their normalized
    /// distance.
    pub fn self_intersection(&self, original: &Mesh, err: &mut String) -> f64 {
        if original.points.len() != self.points.len() {
            err.push_str("error, parameter for self_intersection should be the original mesh\n");
            return -1.0;
        }
        let n = self.points.len();
        let (mut ml, mut mlo) = (0.0, 0.0);
        let mut counter = 0i32;
        for i in 0..n {
            counter += 1;
            ml += self.medium_distance_of_neighbours(i);
            mlo += original.medium_distance_of_neighbours(i);
        }
        ml /= f64::from(counter);
        mlo /= f64::from(counter);
        let mut intersection = 0.0;
        for i in 0..n {
            let ci = self.points[i].coord;
            for j in 0..n {
                if i == j || self.adjacent(i, j) {
                    continue;
                }
                let cj = self.points[j].coord;
                let d2 = (ci.x - cj.x) * (ci.x - cj.x)
                    + (ci.y - cj.y) * (ci.y - cj.y)
                    + (ci.z - cj.z) * (ci.z - cj.z);
                if d2 < ml * ml {
                    let dist = ci.sub(cj).norm() / ml;
                    let disto = original.points[i]
                        .coord
                        .sub(original.points[j].coord)
                        .norm()
                        / mlo;
                    intersection += (dist - disto) * (dist - disto);
                }
            }
        }
        intersection
    }

    /// `Triangle::intersect(const Triangle&)`.
    fn tri_intersect(&self, a: usize, b: usize) -> bool {
        let pa = self.tris[a].v.map(|i| self.points[i].coord);
        let pb = self.tris[b].v.map(|i| self.points[i].coord);
        let normal = pa[0].sub(pa[1]).cross(pa[0].sub(pa[2]));
        let side = |t: &[V3; 3], k: usize| normal.dot(t[k].sub(pa[0]));
        let mut result = false;
        let test = |me: &[V3; 3], other: &[V3; 3], nme: V3, result: &mut bool| {
            for i in 0..3 {
                if nme.dot(other[i].sub(me[0])) * nme.dot(other[(i + 1) % 3].sub(me[0])) < 0.0 {
                    let v1 = me[1].sub(me[0]);
                    let v2 = me[2].sub(me[0]);
                    let v3 = me[2].sub(me[1]);
                    let v = v1.cross(v2);
                    let p1 = other[i].sub(me[0]);
                    let d1 = other[(i + 1) % 3].sub(other[i]);
                    let denom = d1.x * v.x + d1.y * v.y + d1.z * v.z;
                    if denom != 0.0 {
                        let lambda1 = -(p1.x * v.x + p1.y * v.y + p1.z * v.z) / denom;
                        let proj1 = p1.add(d1.scale(lambda1));
                        let n1 = v1.cross(proj1);
                        let n2 = proj1.cross(v2);
                        let n3 = v3.cross(proj1.add(v1.scale(-1.0)));
                        let (a13, a23, a12) = (n1.dot(n3), n2.dot(n3), n1.dot(n2));
                        let inside = (a13 > 0.0 && a23 > 0.0 && a12 > 0.0)
                            || (a13 < 0.0 && a23 < 0.0 && a12 < 0.0);
                        *result |= inside;
                    }
                }
            }
        };
        if side(&pb, 0) * side(&pb, 1) < 0.0 || side(&pb, 0) * side(&pb, 2) < 0.0 {
            // possible intersection: the full test from this triangle, then from the other
            test(&pa, &pb, normal, &mut result);
            let normalt = pb[0].sub(pb[1]).cross(pb[0].sub(pb[2]));
            test(&pb, &pa, normalt, &mut result);
            result
        } else {
            false
        }
    }

    /// `Mesh::real_self_intersection`: whether any two triangles that share no vertex
    /// intersect, by a sweep over the vertices sorted by x.
    pub fn real_self_intersection(&mut self) -> bool {
        // Pt_special per point: the point and a copy of its triangle list.
        let mut v: Vec<(usize, Vec<usize>)> = (0..self.points.len())
            .map(|p| (p, self.points[p].triangles.clone()))
            .collect();
        for &t in &self.tri_list.clone() {
            // data: [state, centre x, y, z, max squared distance]; the centre is
            // `centre *= 1/3`, integer division: 0 (FSL-4x, harmless: see findings)
            let [a, b, c] = self.tris[t].v;
            let mut centre = V3::default();
            for &q in &[a, b, c] {
                let k = self.points[q].coord;
                centre = V3::new(centre.x + k.x, centre.y + k.y, centre.z + k.z);
            }
            centre = V3::new(centre.x * 0.0, centre.y * 0.0, centre.z * 0.0);
            let mut max = 0.0;
            for &q in &[a, b, c] {
                let k = self.points[q].coord;
                let d = (centre.x - k.x) * (centre.x - k.x)
                    + (centre.y - k.y) * (centre.y - k.y)
                    + (centre.z - k.z) * (centre.z - k.z);
                if d > max {
                    max = d;
                }
            }
            self.tris[t].data = vec![0.0, centre.x, centre.y, centre.z, max];
        }
        let xs: Vec<f64> = self.points.iter().map(|p| p.coord.x).collect();
        crate::stdsort::sort_by(&mut v, |a, b| xs[a.0] < xs[b.0]);
        let mut result = false;
        let mut l: Vec<usize> = Vec::new();
        for (_, tlist) in &v {
            if result {
                break;
            }
            for &tr in tlist {
                if result {
                    break;
                }
                let state = self.tris[tr].data[0];
                if state == 0.0 {
                    for &tr2 in &l {
                        let mut res = false;
                        for i in 0..3 {
                            for j in 0..3 {
                                if !res {
                                    let ci = self.points[self.tris[tr2].v[i]].coord;
                                    let cj = self.points[self.tris[tr].v[j]].coord;
                                    if ci.pt_eq(cj) {
                                        res = true;
                                    }
                                }
                            }
                        }
                        if !res {
                            let (d1, d2) = (&self.tris[tr].data, &self.tris[tr2].data);
                            let d = (d1[1] - d2[1]) * (d1[1] - d2[1])
                                + (d1[2] - d2[2]) * (d1[2] - d2[2])
                                + (d1[3] - d2[3]) * (d1[3] - d2[3]);
                            if d < d1[4] + d2[4] {
                                result |= self.tri_intersect(tr, tr2);
                            }
                        }
                    }
                    l.push(tr);
                    self.tris[tr].data[0] = 1.0;
                } else if state == 1.0 {
                    self.tris[tr].data[0] = 2.0;
                } else if state == 2.0 {
                    list_remove(&mut l, tr);
                }
            }
        }
        result
    }

    /// `Mesh::save(name, 3)`: the VTK polydata text (fixed notation, 6 decimals), or the
    /// message FSL prints when the file cannot be opened.
    pub fn vtk_text(&self) -> String {
        let mut s = String::new();
        s.push_str("# vtk DataFile Version 3.0\nsurface file\nASCII\nDATASET POLYDATA\nPOINTS ");
        let _ = writeln!(s, "{}  float", self.points.len());
        for p in &self.points {
            let c = p.coord;
            let _ = writeln!(
                s,
                "{} {} {}",
                cnum::fmt_f(c.x, 6),
                cnum::fmt_f(c.y, 6),
                cnum::fmt_f(c.z, 6)
            );
        }
        let _ = writeln!(
            s,
            "POLYGONS {} {}",
            self.tri_list.len(),
            self.tri_list.len() * 4
        );
        for &t in &self.tri_list {
            let [a, b, c] = self.tris[t].v;
            let _ = writeln!(
                s,
                "3 {} {} {} ",
                self.points[a].no, self.points[b].no, self.points[c].no
            );
        }
        s
    }

    /// `Mesh::save(name, 3)`.
    pub fn save_vtk(&self, path: &str, err: &mut String) {
        if path == "c" {
            err.push_str("cancelled\n");
            return;
        }
        match std::fs::write(path, self.vtk_text()) {
            Ok(()) => {}
            Err(_) => {
                let _ = writeln!(err, "error opening file {path}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icosahedron_level_five() {
        let m = Mesh::icosa(5);
        assert_eq!(m.points.len(), 2562);
        assert_eq!(m.tri_list.len(), 5120);
        // every vertex on the unit sphere, 5 or 6 neighbours and triangles
        for (i, p) in m.points.iter().enumerate() {
            assert!((p.coord.norm() - 1.0).abs() < 1e-12);
            assert!(matches!(p.neighbours.len(), 5 | 6), "{i}");
            assert_eq!(p.neighbours.len(), p.triangles.len());
        }
        let c = m.copy();
        assert_eq!(c.points.len(), 2562);
        assert_eq!(c.tri_list.len(), 5120);
    }
}
