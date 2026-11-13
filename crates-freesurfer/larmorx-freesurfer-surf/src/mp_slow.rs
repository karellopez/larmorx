// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The metric properties computed on the surface itself (`fs.surfio`, queue item 3.1-52):
//! FreeSurfer 8.2.0's `utils/mrisurf_metricProperties_slow.h` lines 1-732: `MRIScomputeNormals`,
//! `MRIScomputeTriangleProperties` (from `utils/MRIScomputeTriangleProperties_extracted.h`),
//! `MRIScomputeAvgInterVertexDist`, `mrisOrientEllipsoid`, `MRISupdateEllipsoidSurface`,
//! `mrisOrientPlane`, `mrisOrientSurface` and `MRIScomputeMetricPropertiesWkr`, the computation
//! `MRIScomputeMetricProperties` runs with `FREESURFER_OLD_MRIScomputeMetricProperties` set
//! (`mrisComputeSurfaceDimensions` is [`crate::coords::compute_surface_dimensions`],
//! `mrisComputeVertexDistances` [`crate::metric::compute_vertex_distances`]). The same arithmetic
//! as the copy's ([`crate::mp`]), on the surface's fields and its face-normal cache. Lines
//! 734-1008 (the sampled energies) need the surface hash and the face sampling (fs.surf).

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]


use larmorx_freesurfer_core::diag::{self, DIAG_SHOW};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::sse::{add, add64, div, sub};

use crate::aaa::{MrisStatus, SurfGlobals};
use crate::coords::{self, Io};
use crate::facenorm;
use crate::metric;
use crate::mp::{
    Instance, RAN, avg_dist, face_area_normal, fnv_hash, nbrs, orients_as_ellipsoid, points_inwards,
    reports_degenerate, romp_partials, romp_total, sphere_area, takes_sphere_area,
    triangle_props, vertex_area_share, vertex_face_area, vertex_normal, vface, vix,
};
use crate::mris::Mris;

fn usz(n: i32) -> usize {
    usize::try_from(n).unwrap_or(0)
}

fn xyz(mris: &Mris, vno: usize) -> [f32; 3] {
    let v = mris.vertex(vno);
    [v.x, v.y, v.z]
}

fn face_xyz(mris: &Mris, fno: usize) -> [[f32; 3]; 3] {
    let v = mris.face(fno).v;
    [xyz(mris, vix(v[0])), xyz(mris, vix(v[1])), xyz(mris, vix(v[2]))]
}

fn set_area_normal(mris: &mut Mris, fno: usize) {
    let n = face_area_normal(face_xyz(mris, fno));
    facenorm::set_face_norm(mris, fno, n[0], n[1], n[2]);
}

/// `MRIScomputeNormals(mris)`: as the copy's ([`crate::mp`]'s `MRISMP_computeNormals`) on the
/// surface: each ripped face makes its vertices border vertices; each other face with an unripped
/// vertex gets the cross product of two edges as its cached normal; each unripped vertex, in up
/// to five rounds, its unit normal and area (its original area too when negative), a degenerate
/// vertex moved with its 1-hop neighbours and retried. A vertex whose normal cannot be computed in
/// five rounds keeps its normal. Returns `NO_ERROR`.
pub fn compute_normals(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> i32 {
    let nv = usz(mris.nvertices);
    for k in 0..usz(mris.nfaces) {
        let f = mris.face(k);
        let v = f.v;
        if f.ripflag != 0 {
            for n in 0..3 {
                mris.vertex_mut(vix(v[n])).border = 1;
            }
        } else if (0..3).any(|vi| mris.vertex(vix(v[vi])).ripflag == 0) {
            set_area_normal(mris, k);
        }
    }
    let mut pending: Vec<usize> = (0..nv).filter(|&k| mris.vertex(k).ripflag == 0).collect();
    let mut trial = 0;
    while trial < 5 && !pending.is_empty() {
        let mut next = Vec::new();
        for &k in &pending {
            let r = {
                let m: &Mris = mris;
                vertex_normal(
                    sg,
                    m,
                    k,
                    &|f| m.face(f).ripflag != 0,
                    &|f| m.face(f).v,
                    &|v| xyz(m, v),
                )
            };
            match r {
                Some((snorm, area)) => {
                    let v = mris.vertex_mut(k);
                    v.area = vertex_area_share(sg, area);
                    if v.origarea < 0.0 {
                        v.origarea = v.area;
                    }
                    (v.nx, v.ny, v.nz) = (snorm[0], snorm[1], snorm[2]);
                }
                None => next.push(k),
            }
        }
        if next.is_empty() {
            break;
        }
        next.sort_unstable();
        for &k in &next {
            if reports_degenerate(mris.status) && diag::verbose_on(g) {
                let _ = writeln!(io.err, "vertex {k}: degenerate normal");
            }
            let mut counter = 0;
            let kk = k as i32;
            let r = |c: &mut i32| fnv_hash(trial, kk, c, -RAN, RAN);
            let d = r(&mut counter);
            mris.vertex_mut(k).x = add(mris.vertex(k).x, d);
            let d = r(&mut counter);
            mris.vertex_mut(k).y = add(mris.vertex(k).y, d);
            if mris.status == MrisStatus::PLANE || mris.status == MrisStatus::CUT {
                continue;
            }
            let d = r(&mut counter);
            mris.vertex_mut(k).z = add(mris.vertex(k).z, d);
            let vnum = mris.topology(k).vnum;
            for (n, vn) in nbrs(mris, k, i32::from(vnum)).into_iter().enumerate() {
                if g.gdiag & DIAG_SHOW != 0 && diag::verbose_on(g) {
                    let _ = writeln!(io.out, "   k={k:5} {n} nbr = {vn:5} / {vnum}");
                }
                let (dx, dy, dz) = (r(&mut counter), r(&mut counter), r(&mut counter));
                let v = mris.vertex_mut(vn);
                v.x = add(v.x, dx);
                v.y = add(v.y, dy);
                v.z = add(v.z, dz);
            }
            for n in 0..usize::from(mris.topology(k).num) {
                let fno = vface(mris, k, n);
                if mris.face(fno).ripflag != 0 {
                    continue;
                }
                set_area_normal(mris, fno);
            }
        }
        pending = next;
        trial += 1;
    }
    0
}

/// `MRIScomputeTriangleProperties(mris)`: each unripped face's area, cached unit normal and
/// angles (as the copy's, [`triangle_props`]); `total_area` the reproducible sum of the areas in
/// double, rounded to `float`; each unripped vertex's area the `float` sum of its unripped faces'
/// areas divided by 3 (2 without `fix_vertex_area`). Returns `NO_ERROR`.
pub fn compute_triangle_properties(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> i32 {
    let mut partial = Vec::new();
    for (lo, hi) in romp_partials(0, mris.nfaces) {
        let mut s = 0.0f64;
        for fno in lo as usize..hi as usize {
            if mris.face(fno).ripflag != 0 {
                continue;
            }
            let t = triangle_props(g, io, face_xyz(mris, fno), Instance::Surface);
            mris.face_mut(fno).area = t.area;
            // `reduction_total_area += area` compiled as `area + partial`
            s = add64(f64::from(t.area), s);
            facenorm::set_face_norm(mris, fno, t.norm[0], t.norm[1], t.norm[2]);
            mris.face_mut(fno).angle = t.angle;
        }
        partial.push(s);
    }
    mris.total_area = romp_total(0.0, &partial) as f32;
    for vno in 0..usz(mris.nvertices) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let a = {
            let m: &Mris = mris;
            vertex_face_area(sg, m, vno, &|f| m.face(f).ripflag != 0, &|f| m.face(f).area)
        };
        mris.vertex_mut(vno).area = a;
    }
    0
}

/// `MRIScomputeAvgInterVertexDist(mris, StdDev)`: the mean of the 1-hop distances between
/// unripped neighbours (`v->dist`) into `avg_vertex_dist` (`mrisSetAvgInterVertexDist`), and their
/// standard deviation, returned for `StdDev` (as the copy's, [`avg_dist`]).
pub fn compute_avg_inter_vertex_dist(mris: &mut Mris) -> f64 {
    let (avg, std) = {
        let m: &Mris = mris;
        avg_dist(m, &|v| m.vertex(v).ripflag != 0, &|v, k| {
            m.vertex(v).dist.as_ref().expect("VERTEX::dist is NULL")[k]
        })
    };
    mris.avg_vertex_dist = avg;
    std
}

/// `mrisOrientEllipsoid(mris)`: each unripped face whose cached normal points inwards (as the
/// copy's) gets its area and angles negated (times -1) and its cached normal negated; then the
/// reproducible sums of the non-negative areas into `total_area`, of the negated negative ones into
/// `neg_area`, of those faces' cached original areas into `neg_orig_area`. Returns `NO_ERROR`.
fn orient_ellipsoid(mris: &mut Mris) -> i32 {
    for fno in 0..usz(mris.nfaces) {
        if mris.face(fno).ripflag != 0 {
            continue;
        }
        let e = facenorm::get_face_norm(mris, fno);
        if points_inwards(face_xyz(mris, fno), [e.nx, e.ny, e.nz]) {
            // `*= -1.0f` compiled as a sign flip
            let f = mris.face_mut(fno);
            f.area = -f.area;
            facenorm::set_face_norm(mris, fno, -e.nx, -e.ny, -e.nz);
            for a in mris.face_mut(fno).angle.iter_mut() {
                *a = -*a;
            }
        }
    }
    mris.total_area = 0.0;
    mris.neg_orig_area = 0.0;
    mris.neg_area = 0.0;
    let (mut ta, mut na, mut noa) = (Vec::new(), Vec::new(), Vec::new());
    for (lo, hi) in romp_partials(0, mris.nfaces) {
        let (mut t, mut n, mut o) = (0.0f64, 0.0f64, 0.0f64);
        for fno in lo as usize..hi as usize {
            if mris.face(fno).ripflag != 0 {
                continue;
            }
            let a = mris.face(fno).area;
            // each term first (`term + partial`)
            if a >= 0.0 {
                t = add64(f64::from(a), t);
            } else {
                let e = facenorm::get_face_norm(mris, fno);
                n = add64(f64::from(-a), n);
                o = add64(f64::from(e.orig_area), o);
            }
        }
        ta.push(t);
        na.push(n);
        noa.push(o);
    }
    mris.total_area = romp_total(0.0, &ta) as f32;
    mris.neg_orig_area = romp_total(0.0, &noa) as f32;
    mris.neg_area = romp_total(0.0, &na) as f32;
    0
}

/// `MRISupdateEllipsoidSurface(mris)`: [`orient_ellipsoid`] but for an unoriented sphere.
/// Returns `NO_ERROR`.
pub fn update_ellipsoid_surface(mris: &mut Mris) -> i32 {
    if mris.status != MrisStatus::UNORIENTED_SPHERE {
        orient_ellipsoid(mris);
    }
    0
}

/// `mrisOrientPlane(mris)`: each unripped face whose cached normal's z is negative gets its area
/// and angles negated (times -1) and its cached normal negated; `total_area`, `neg_area` and
/// `neg_orig_area` the `float` sums of the non-negative areas, the negated negative ones and
/// their faces' cached original areas. Each unripped vertex: a negative normal z negated (times
/// -1) with `neg` 1, else `neg` 0; its area the `float` sum of all its faces' areas (ripped ones
/// too) divided by 3 (2 without `fix_vertex_area`). Returns `NO_ERROR`.
pub fn orient_plane(sg: &SurfGlobals, mris: &mut Mris) -> i32 {
    for fno in 0..usz(mris.nfaces) {
        if mris.face(fno).ripflag != 0 {
            continue;
        }
        let e = facenorm::get_face_norm(mris, fno);
        if e.nz < 0.0 {
            // `*= -1.0f` compiled as a sign flip
            let f = mris.face_mut(fno);
            f.area = -f.area;
            facenorm::set_face_norm(mris, fno, -e.nx, -e.ny, -e.nz);
            for a in mris.face_mut(fno).angle.iter_mut() {
                *a = -*a;
            }
        }
    }
    mris.total_area = 0.0;
    mris.neg_orig_area = 0.0;
    mris.neg_area = 0.0;
    for fno in 0..usz(mris.nfaces) {
        if mris.face(fno).ripflag != 0 {
            continue;
        }
        let a = mris.face(fno).area;
        // `total += area` as `area + total`; `neg += -area` as `neg - area`
        if a >= 0.0 {
            mris.total_area = add(a, mris.total_area);
        } else {
            let e = facenorm::get_face_norm(mris, fno);
            mris.neg_area = sub(mris.neg_area, a);
            mris.neg_orig_area = add(mris.neg_orig_area, e.orig_area);
        }
    }
    for vno in 0..usz(mris.nvertices) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let v = mris.vertex_mut(vno);
        if v.nz < 0.0 {
            v.nz = -v.nz;
            v.neg = 1;
        } else {
            v.neg = 0;
        }
        let mut area = 0.0f32;
        for n in 0..usize::from(mris.topology(vno).num) {
            area = add(area, mris.face(vface(mris, vno, n)).area);
        }
        mris.vertex_mut(vno).area = vertex_area_share(sg, area);
    }
    0
}

/// `mrisOrientSurface(mris)`: a rigid body, sphere (parameterized or not), ellipsoid or spherical
/// patch through [`update_ellipsoid_surface`], a plane through [`orient_plane`], any other surface
/// left alone. Returns `NO_ERROR`.
pub fn orient_surface(sg: &SurfGlobals, mris: &mut Mris) -> i32 {
    if orients_as_ellipsoid(mris.status) {
        update_ellipsoid_surface(mris);
    } else if mris.status == MrisStatus::PLANE {
        orient_plane(sg, mris);
    }
    0
}

/// `MRIScomputeMetricPropertiesWkr(mris)`: [`compute_normals`], the bounding box, the distances
/// at `nsize` hops, [`compute_triangle_properties`], `avg_vertex_area` (`total_area / nvertices`
/// in `float`), the mean distance and its deviation (`std_vertex_dist`), the orientation; a
/// parameterized sphere's, rigid body's or sphere's `total_area` then `4 pi radius²`.
pub fn compute_metric_properties_wkr(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<(), Fatal> {
    compute_normals(g, sg, io, mris);
    coords::compute_surface_dimensions(mris);
    metric::compute_vertex_distances(g, io, mris)?;
    compute_triangle_properties(g, sg, io, mris);
    mris.avg_vertex_area = f64::from(div(mris.total_area, mris.nvertices as f32));
    mris.std_vertex_dist = compute_avg_inter_vertex_dist(mris);
    orient_surface(sg, mris);
    if takes_sphere_area(mris.status) {
        mris.total_area = sphere_area(mris.radius);
    }
    Ok(())
}
