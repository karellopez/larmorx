// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The faster metric properties (`fs.surfio`, queue item 3.1-52): FreeSurfer 8.2.0's
//! `utils/mrisurf_metricProperties_faster.cpp`, the `MetricPropertiesCalculator` that
//! `MRIScomputeMetricProperties` runs instead of its default when `FS_FASTER_MP` is set (no
//! FreeSurfer script sets it). Its own arithmetic: the face normals `(p1 - p0) x (p2 - p0)` made
//! unit length before the vertices sum them (weighted by the corner angle with
//! `FS_WEIGHTS_NORMS_BY_ANGLE` set), the vertex areas from the faces' areas, up to five rounds in
//! which every vertex whose normal has no length moves with its neighbours (each by its own
//! `fnv_hash`), the distances by the status (a sphere's or a parameterized sphere's arcs, any
//! other's straight lines), the sums of the plane's orientation reproducible too.
//!
//! FreeSurfer sums the faces' areas with OpenMP's `reduction(+:total_area)`, whose order depends
//! on the threads and their timing (FS-125); here the faces are summed in order, the value of one
//! thread.

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use larmorx_core::defects::{FS_123, FS_124};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt, sqrt64, sub, sub64};
use larmorx_freesurfer_core::utils::fast_approx_atan2f;

use crate::aaa::{MrisStatus, SurfGlobals, get_fix_vertex_area_value};
use crate::coords::{Io, centre};
use crate::dist;
use crate::facenorm;
use crate::mp::{
    RAN, diff3, fnv_hash, nbrs, orients_as_ellipsoid, romp_partials, romp_total,
    sphere_area, sphere_distance, sphere_info, takes_sphere_area, vface, vix,
};
use crate::mris::Mris;

fn usz(n: i32) -> usize {
    usize::try_from(n).unwrap_or(0)
}

/// `vec_cross(a, b, c)`: `a1 b2 - a2 b1`, `a2 b0 - a0 b2`, `a0 b1 - a1 b0`, the second
/// component's second product compiled as `b2 a0` (both uses).
fn vec_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        sub(mul(a[1], b[2]), mul(a[2], b[1])),
        sub(mul(a[2], b[0]), mul(b[2], a[0])),
        sub(mul(a[0], b[1]), mul(a[1], b[0])),
    ]
}

/// `vec_length(v)` of the face normal: `sqrt((x² + y²) + z²)`.
fn face_length(v: [f32; 3]) -> f32 {
    sqrt(add(add(mul(v[0], v[0]), mul(v[1], v[1])), mul(v[2], v[2])))
}

/// `vec_length(v)` of a vertex normal and of a distance's difference, as compiled there:
/// `sqrt(z² + (x² + y²))`.
fn length_zfirst(v: [f32; 3]) -> f32 {
    sqrt(add(mul(v[2], v[2]), add(mul(v[0], v[0]), mul(v[1], v[1]))))
}

/// The calculator's arrays (`MetricPropertiesCalculator`'s members).
struct Calc {
    weight_norms_by_angle: bool,
    vertices_have_moved: bool,
    vertex_area_fix_value: f32,
    surf_is_3d: bool,
    nvertices: usize,
    nfaces: usize,
    vertices: Vec<[f32; 3]>,
    vertex_ripped: Vec<bool>,
    faces: Vec<[i32; 3]>,
    face_ripped: Vec<bool>,
    face_areas: Vec<f32>,
    face_normals: Vec<[f32; 3]>,
    face_angles: Vec<[f32; 3]>,
    vertex_areas: Vec<f32>,
    vertex_normals: Vec<[f32; 3]>,
}

/// `MetricPropertiesCalculator::init()`: the surface's vertices, rip flags and faces copied; each
/// vertex's distances made (`MRISmakeDist`: zeros of its size in use), ripped ones too; each
/// ripped face's vertices made border vertices. The arrays C allocates without values (the ripped
/// faces' areas, normals and angles) are 0 here (FS-123).
fn init(io: &Io, sg: &SurfGlobals, mris: &mut Mris) -> Calc {
    let nv = usz(mris.nvertices);
    let nf = usz(mris.nfaces);
    let mut vertices = Vec::with_capacity(nv);
    let mut vertex_ripped = Vec::with_capacity(nv);
    for vno in 0..nv {
        let v = mris.vertex(vno);
        vertices.push([v.x, v.y, v.z]);
        vertex_ripped.push(v.ripflag != 0);
        dist::make_dist(mris, vno);
    }
    let mut faces = Vec::with_capacity(nf);
    let mut face_ripped = Vec::with_capacity(nf);
    for fno in 0..nf {
        let f = mris.face(fno);
        let (v, ripped) = (f.v, f.ripflag != 0);
        faces.push(v);
        face_ripped.push(ripped);
        if ripped {
            for n in 0..3 {
                mris.vertex_mut(vix(v[n])).border = 1;
            }
        }
    }
    Calc {
        weight_norms_by_angle: io.env.get("FS_WEIGHTS_NORMS_BY_ANGLE").is_some(),
        vertices_have_moved: false,
        vertex_area_fix_value: if get_fix_vertex_area_value(sg) != 0 {
            3.0
        } else {
            2.0
        },
        surf_is_3d: !(mris.status == MrisStatus::PLANE || mris.status == MrisStatus::CUT),
        nvertices: nv,
        nfaces: nf,
        vertices,
        vertex_ripped,
        faces,
        face_ripped,
        face_areas: vec![0.0; nf],
        face_normals: vec![[0.0; 3]; nf],
        face_angles: vec![[0.0; 3]; nf],
        vertex_areas: vec![0.0; nv],
        vertex_normals: vec![[0.0; 3]; nv],
    }
}

impl Calc {
    fn corners(&self, fno: usize) -> [[f32; 3]; 3] {
        let v = self.faces[fno];
        [
            self.vertices[vix(v[0])],
            self.vertices[vix(v[1])],
            self.vertices[vix(v[2])],
        ]
    }

    /// `moveVertexRandomly(vno, trial, &counter)`: x and y (and z for a surface other than a
    /// plane or a cut, else `+ 0.0`) moved by `fnv_hash(trial, vno, ...)` in `-0.001..0.001`.
    fn move_vertex_randomly(&mut self, vno: usize, trial: i32, counter: &mut i32) {
        let k = vno as i32;
        let r0 = fnv_hash(trial, k, counter, -RAN, RAN);
        let r1 = fnv_hash(trial, k, counter, -RAN, RAN);
        let r2 = if self.surf_is_3d {
            fnv_hash(trial, k, counter, -RAN, RAN)
        } else {
            0.0
        };
        let p = &mut self.vertices[vno];
        p[0] = add(p[0], r0);
        p[1] = add(p[1], r1);
        p[2] = add(p[2], r2);
    }

    /// `computeNormalsAreasAndAngles()`.
    fn normals_areas_angles(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        mris: &mut Mris,
    ) {
        for trial in 0..5 {
            let mut total_area = 0.0f64;
            self.vertex_areas.iter_mut().for_each(|a| *a = 0.0);
            self.vertex_normals.iter_mut().for_each(|n| *n = [0.0; 3]);
            for fno in 0..self.nfaces {
                if self.face_ripped[fno] {
                    continue;
                }
                let [p0, p1, p2] = self.corners(fno);
                let mut fnorm = vec_cross(diff3(p1, p0), diff3(p2, p0));
                let mut area = face_length(fnorm);
                if f64::from(area) > 0.0 {
                    fnorm = [div(fnorm[0], area), div(fnorm[1], area), div(fnorm[2], area)];
                }
                area = div64(f64::from(area), 2.0) as f32;
                self.face_areas[fno] = area;
                self.face_normals[fno] = fnorm;
                // `total_area += area` compiled as `area + total`
                total_area = add64(f64::from(area), total_area);
                for corner in 0..3 {
                    let (a, b) = match corner {
                        0 => (diff3(p2, p0), diff3(p1, p0)),
                        1 => (diff3(p0, p1), diff3(p2, p1)),
                        _ => (diff3(p1, p2), diff3(p0, p2)),
                    };
                    let c = vec_cross(b, a);
                    // `vec_dot(c, fnorm)` compiled as `c2 n2 + (c1 n1 + c0 n0)`, `vec_dot(a, b)`
                    // as `(b1 a1 + a0 b0) + b2 a2`
                    let d1 = add(
                        mul(c[2], fnorm[2]),
                        add(mul(c[1], fnorm[1]), mul(c[0], fnorm[0])),
                    );
                    let d2 = add(add(mul(b[1], a[1]), mul(a[0], b[0])), mul(b[2], a[2]));
                    self.face_angles[fno][corner] = fast_approx_atan2f(g, io.env, io.out, d1, d2);
                }
            }
            mris.total_area = total_area as f32;
            let mut must_move = false;
            for vno in 0..self.nvertices {
                if self.vertex_ripped[vno] {
                    continue;
                }
                let mut norm = [0.0f32; 3];
                let mut area = 0.0f32;
                for nf in 0..usize::from(mris.topology(vno).num) {
                    let fno = vface(mris, vno, nf);
                    if self.face_ripped[fno] {
                        g.notices.add(
                            &FS_123,
                            "a ripped face counts with area and normal 0, not memory contents",
                        );
                    }
                    area = add(area, self.face_areas[fno]);
                    let fnrm = self.face_normals[fno];
                    if !self.weight_norms_by_angle {
                        for k in 0..3 {
                            norm[k] = add(norm[k], fnrm[k]);
                        }
                    } else {
                        let v = self.faces[fno];
                        let index = v.iter().position(|&x| x == vno as i32).unwrap_or(3);
                        let angle = if index < 3 {
                            self.face_angles[fno][index]
                        } else {
                            // one past the corners: the next face's first angle in C's array
                            self.face_angles
                                .get(fno + 1)
                                .map_or(0.0, |a| a[0])
                        };
                        // `norm[k] += face_norm[k] * angle`, each product first (the third as
                        // `angle * face_norm[2]`)
                        norm[0] = add(mul(fnrm[0], angle), norm[0]);
                        norm[1] = add(mul(fnrm[1], angle), norm[1]);
                        norm[2] = add(mul(angle, fnrm[2]), norm[2]);
                    }
                }
                area = div(area, self.vertex_area_fix_value);
                let length = length_zfirst(norm);
                if length > 0.0 {
                    norm = [div(norm[0], length), div(norm[1], length), div(norm[2], length)];
                } else {
                    must_move = true;
                    area = -1.0;
                }
                self.vertex_areas[vno] = area;
                self.vertex_normals[vno] = norm;
            }
            if !must_move {
                return;
            }
            for vno in 0..self.nvertices {
                if self.vertex_areas[vno] < 0.0 {
                    let mut counter = 0;
                    self.move_vertex_randomly(vno, trial, &mut counter);
                    let vnum = i32::from(mris.topology(vno).vnum);
                    for n in nbrs(mris, vno, vnum) {
                        self.move_vertex_randomly(n, trial, &mut counter);
                    }
                }
            }
            self.vertices_have_moved = true;
        }
        // five rounds without every normal: those vertices keep area -1 (FS-124)
        if self
            .vertex_areas
            .iter()
            .zip(&self.vertex_ripped)
            .any(|(&a, &r)| !r && a == -1.0)
        {
            g.notices.add(
                &FS_124,
                "a vertex whose normal could not be computed has area -1, as in FreeSurfer",
            );
        }
    }

    /// `computeDimensions()`: the bounding box from the first vertex's coordinates. A surface with
    /// no vertex is read past its arrays in C (FS-123); here its box is the default computation's
    /// (low ends 10000, high ends -10000).
    fn dimensions(&self, g: &mut Globals, mris: &mut Mris) {
        let (mut lo, mut hi) = match self.vertices.first() {
            Some(&p) => (p, p),
            None => {
                g.notices.add(
                    &FS_123,
                    "the box of a surface with no vertex is the default one, not memory contents",
                );
                ([10000.0; 3], [-10000.0; 3])
            }
        };
        for p in self.vertices.iter().skip(1) {
            for k in 0..3 {
                if p[k] > hi[k] {
                    hi[k] = p[k];
                }
                if p[k] < lo[k] {
                    lo[k] = p[k];
                }
            }
        }
        mris.xctr = centre(lo[0], hi[0]);
        mris.yctr = centre(lo[1], hi[1]);
        mris.zctr = centre(lo[2], hi[2]);
        (mris.xhi, mris.yhi, mris.zhi) = (hi[0], hi[1], hi[2]);
        (mris.xlo, mris.ylo, mris.zlo) = (lo[0], lo[1], lo[2]);
    }

    /// `computeVertexDistances()`: a sphere's or a parameterized sphere's arcs, any other
    /// surface's straight lines, into each unripped vertex's first `vtotal` distances.
    fn vertex_distances(&self, mris: &mut Mris) {
        let sphere = mris.status == MrisStatus::SPHERE
            || mris.status == MrisStatus::PARAMETERIZED_SPHERE;
        let info: Vec<_> = if sphere {
            (0..self.nvertices)
                .map(|v| sphere_info(self.vertex_ripped[v], self.vertices[v]))
                .collect()
        } else {
            Vec::new()
        };
        for vno in 0..self.nvertices {
            if self.vertex_ripped[vno] {
                continue;
            }
            let vtotal = i32::from(mris.topology(vno).vtotal);
            let ds: Vec<f32> = nbrs(mris, vno, vtotal)
                .into_iter()
                .map(|n| {
                    if sphere {
                        sphere_distance(&info[vno], &info[n])
                    } else {
                        length_zfirst(diff3(self.vertices[vno], self.vertices[n]))
                    }
                })
                .collect();
            if ds.is_empty() {
                continue;
            }
            let d = mris
                .vertex_mut(vno)
                .dist
                .as_mut()
                .expect("VERTEX::dist is NULL");
            d[..ds.len()].copy_from_slice(&ds);
        }
    }

    /// `computeAverageVertexDistance()`: the reproducible sums of the 1-hop distances between
    /// unripped neighbours; `avg_vertex_dist` their mean, `std_vertex_dist` their deviation.
    fn average_vertex_distance(&self, mris: &mut Mris) {
        let (mut s, mut s2, mut c) = (Vec::new(), Vec::new(), Vec::new());
        for (lo, hi) in romp_partials(0, self.nvertices as i32) {
            let (mut sum, mut sum2, mut n) = (0.0f64, 0.0f64, 0.0f64);
            for vno in lo as usize..hi as usize {
                if self.vertex_ripped[vno] {
                    continue;
                }
                let vnum = i32::from(mris.topology(vno).vnum);
                let d = mris.vertex(vno).dist.as_ref();
                for (m, nb) in nbrs(mris, vno, vnum).into_iter().enumerate() {
                    if self.vertex_ripped[nb] {
                        continue;
                    }
                    let x = f64::from(d.expect("VERTEX::dist is NULL")[m]);
                    sum = add64(sum, x);
                    sum2 = add64(mul64(x, x), sum2);
                    n = add64(n, 1.0);
                }
            }
            s.push(sum);
            s2.push(sum2);
            c.push(n);
        }
        let (sum, sum2, n) = (romp_total(0.0, &s), romp_total(0.0, &s2), romp_total(0.0, &c));
        let avg = div64(sum, n);
        let std = sqrt64(div64(
            mul64(n, sub64(div64(sum2, n), mul64(avg, avg))),
            sub64(n, 1.0),
        ));
        mris.avg_vertex_dist = avg;
        mris.std_vertex_dist = std;
    }

    /// The faces' reproducible sums of `orientPlane` and `orientEllipsoid`, after `flip` decides
    /// which faces' areas, normals and angles are negated (times -1): `total_area` the
    /// non-negative areas, `neg_area` minus the negative ones, `neg_orig_area` their faces'
    /// cached original areas.
    fn orient_faces(&mut self, mris: &mut Mris, flip: &dyn Fn(&Calc, usize) -> bool) {
        let (mut ta, mut na, mut noa) = (Vec::new(), Vec::new(), Vec::new());
        for (lo, hi) in romp_partials(0, self.nfaces as i32) {
            let (mut t, mut n, mut o) = (0.0f64, 0.0f64, 0.0f64);
            for fno in lo as usize..hi as usize {
                if self.face_ripped[fno] {
                    continue;
                }
                if flip(self, fno) {
                    // `*= -1.0f` compiled as a sign flip
                    self.face_areas[fno] = -self.face_areas[fno];
                    for x in self.face_normals[fno].iter_mut() {
                        *x = -*x;
                    }
                    for x in self.face_angles[fno].iter_mut() {
                        *x = -*x;
                    }
                }
                let area = self.face_areas[fno];
                // `total += area` as `area + partial`, `neg -= area`, `neg_orig += orig` as
                // `orig + partial`
                if area >= 0.0 {
                    t = add64(f64::from(area), t);
                } else {
                    n = sub64(n, f64::from(area));
                    let e = facenorm::get_face_norm(mris, fno);
                    o = add64(f64::from(e.orig_area), o);
                }
            }
            ta.push(t);
            na.push(n);
            noa.push(o);
        }
        mris.total_area = romp_total(0.0, &ta) as f32;
        mris.neg_area = romp_total(0.0, &na) as f32;
        mris.neg_orig_area = romp_total(0.0, &noa) as f32;
    }

    /// `orientPlane()`: the faces whose normal's z is negative flipped ([`Calc::orient_faces`]);
    /// each unripped vertex's normal z negated (times -1) when negative, with `neg` 1 (else 0),
    /// its area the `float` sum of all its faces' areas divided by the fix value.
    fn orient_plane(&mut self, g: &mut Globals, mris: &mut Mris) {
        self.orient_faces(mris, &|c, f| c.face_normals[f][2] < 0.0);
        for vno in 0..self.nvertices {
            if self.vertex_ripped[vno] {
                continue;
            }
            let vn = &mut self.vertex_normals[vno];
            if vn[2] < 0.0 {
                vn[2] = -vn[2];
                mris.vertex_mut(vno).neg = 1;
            } else {
                mris.vertex_mut(vno).neg = 0;
            }
            let mut area = 0.0f32;
            for n in 0..usize::from(mris.topology(vno).num) {
                let fno = vface(mris, vno, n);
                if self.face_ripped[fno] {
                    g.notices.add(
                        &FS_123,
                        "a ripped face counts with area 0, not memory contents",
                    );
                }
                area = add(area, self.face_areas[fno]);
            }
            self.vertex_areas[vno] = div(area, self.vertex_area_fix_value);
        }
    }

    /// `orientEllipsoid()`: the faces whose normal points inwards (`(x0 + x1 + x2) nx + (y0 + y1
    /// + y2) ny + (z0 + z1 + z2) nz < 0`, each product first) flipped ([`Calc::orient_faces`]).
    fn orient_ellipsoid(&mut self, mris: &mut Mris) {
        self.orient_faces(mris, &|c, fno| {
            let [p0, p1, p2] = c.corners(fno);
            let n = c.face_normals[fno];
            let s = |k: usize| mul(add(add(p0[k], p1[k]), p2[k]), n[k]);
            add(add(s(0), s(1)), s(2)) < 0.0
        });
    }

    /// `transfer()`: each unripped vertex's position (when the vertices moved), normal and area
    /// (its original area the area's magnitude when negative); each unripped face's cached
    /// normal, angles and area; `dist_nsize` becomes `nsize`.
    fn transfer(&self, mris: &mut Mris) {
        for vno in 0..self.nvertices {
            if self.vertex_ripped[vno] {
                continue;
            }
            let v = mris.vertex_mut(vno);
            if self.vertices_have_moved {
                let p = self.vertices[vno];
                (v.x, v.y, v.z) = (p[0], p[1], p[2]);
            }
            let n = self.vertex_normals[vno];
            (v.nx, v.ny, v.nz) = (n[0], n[1], n[2]);
            v.area = self.vertex_areas[vno];
            if v.origarea < 0.0 {
                v.origarea = self.vertex_areas[vno].abs();
            }
        }
        for fno in 0..self.nfaces {
            if self.face_ripped[fno] {
                continue;
            }
            let n = self.face_normals[fno];
            facenorm::set_face_norm(mris, fno, n[0], n[1], n[2]);
            let f = mris.face_mut(fno);
            f.angle = self.face_angles[fno];
            f.area = self.face_areas[fno];
        }
        mris.dist_nsize = mris.nsize;
    }
}

/// `MRIScomputeMetricPropertiesFaster(mris)`: the calculator's `init`, `compute` (normals, areas
/// and angles; the bounding box; the distances; their mean and deviation; `avg_vertex_area`
/// `total_area / nvertices` in `float`; the orientation of a rigid body's, sphere's, ellipsoid's or
/// spherical patch's faces, or a plane's; a parameterized sphere's, rigid body's or sphere's
/// `total_area` `4 pi radius²`) and `transfer`.
pub fn compute_metric_properties_faster(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) {
    let mut c = init(io, sg, mris);
    c.normals_areas_angles(g, io, mris);
    c.dimensions(g, mris);
    c.vertex_distances(mris);
    c.average_vertex_distance(mris);
    mris.avg_vertex_area = f64::from(div(mris.total_area, mris.nvertices as f32));
    if orients_as_ellipsoid(mris.status) {
        c.orient_ellipsoid(mris);
    } else if mris.status == MrisStatus::PLANE {
        c.orient_plane(g, mris);
    }
    if takes_sphere_area(mris.status) {
        mris.total_area = sphere_area(mris.radius);
    }
    c.transfer(mris);
}
