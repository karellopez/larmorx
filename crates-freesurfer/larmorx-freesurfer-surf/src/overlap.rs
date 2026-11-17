// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Two walks through a face table's buckets (queue items 3.1-47 and 3.1-50, relay 45 of track
//! D): FreeSurfer 8.2.0's `utils/mrisurf_base.cpp`'s `mrisDirectionTriangleIntersection` (1433:
//! whether the ray along a direction from a point meets a face of the bucket a step along it),
//! with `utils/mrisurf_metricProperties.cpp`'s `MRISdistanceToSurface` (5112: the ray tried at
//! quarter-millimetre steps) and `mrisFindAllOverlappingFaces` (4899: the faces whose edges cross
//! a face's, through the buckets of its bounding box).
//!
//! The arithmetic is the official build's (`mrisurf_base.cpp.o`, `mrisurf_metricProperties.cpp.o`):
//! the step's point is formed in `double` from the `float` point and direction and rounded to
//! `float` for the bucket; the distance and the direction's dot product of a hit are formed in
//! `float` from the hit's `double` offsets rounded to `float`.
//!
//! Defects (`larmorx_core::defects`):
//! - **FS-1012** (memory outside the faces): a table of vertices walked as a table of faces: its
//!   vertex numbers beyond the surface's faces read memory outside them. An error in both modes.
//! - **FS-1013** (a hang): the overlapping faces' walk of a face kilometres across, or with an
//!   infinite coordinate: more than [`WALK_LIMIT`] buckets, refused in both modes (a clear error).
//! - the hash table's FS-909 and FS-910 through its lookups.

use std::io::Write;

use larmorx_core::defects::{FS_1012, FS_1013};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::sse::{add, add64, div64, mul, mul64, sqrt, sub64};

use crate::coords::Io;
use crate::edges::{Edge, edges_intersect};
use crate::metric::load_triangle_vertices;
use crate::mht::Mht;
use crate::mris::Mris;
use crate::tritri::{narrow, triangle_ray_intersect, widen};
use crate::views::CURRENT_VERTICES;

/// `MAX_INT_FACES`: the overlapping faces' list's capacity.
pub const MAX_INT_FACES: usize = 100_000;
/// The capacity of the overlapping faces' first list (`all_faces[1000000]`).
pub const MAX_ALL_FACES: usize = 1_000_000;
/// FS-1013: the buckets of one face's bounding-box walk past which it is refused (2^28: a face
/// about 300 mm across in each direction at half a millimetre).
pub const WALK_LIMIT: f64 = (1u64 << 28) as f64;

/// FS-1012 for a face number from the table outside the surface's faces (a table of vertices).
fn table_face(
    g: &mut Globals,
    err: &mut dyn Write,
    mris: &Mris,
    fno: i32,
    who: &str,
) -> Result<usize, Fatal> {
    if (0..mris.nfaces).contains(&fno) {
        return Ok(fno as usize);
    }
    let message = format!(
        "{who}: the face table lists {fno}, outside the surface's {} faces (a table of vertices: FreeSurfer reads memory outside them)",
        mris.nfaces
    );
    g.notices.add(&FS_1012, message.clone());
    log::error_exit(g, err, ERROR_BADPARM, &message)?;
    unreachable!("ErrorExit with a non-zero code always ends the call")
}

/// `mrisDirectionTriangleIntersection(mris, x0, y0, z0, nx, ny, nz, mht, &dist, vno)`: the bucket
/// of the point `p + n dist` (each coordinate `n dist + p` in `double`, rounded to `float`); none:
/// 0. Else each face listed in it in order, but those with vertex `vno` as a corner: the ray from
/// `p` along `n` (widened to `double`) against the face's current positions (fs.surf's
/// `triangle_ray_intersect`); for a hit, its offset from `p` rounded to `float` per coordinate, its
/// length `sqrt((dx² + dy²) + dz²)` and its dot product with `n` `(dx nx + dy ny) + dz nz` in
/// `float`; a hit whose dot product is not negative and whose length is below the nearest so far
/// (from 10,000) becomes the nearest and is stored in `dist`. Returns 1 when one was found, else 0
/// (`dist` unchanged).
#[allow(clippy::too_many_arguments)]
pub fn direction_triangle_intersection(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    p: [f32; 3],
    n: [f32; 3],
    mht: &Mht,
    pdist: &mut f64,
    vno: i32,
) -> Result<i32, Fatal> {
    const WHO: &str = "mrisDirectionTriangleIntersection";
    let dist = *pdist;
    let at = |k: usize| narrow(add64(mul64(widen(n[k]), dist), widen(p[k])));
    let (x, y, z) = (at(0), at(1), at(2));
    let Some((_, bins)) = mht.acq_bucket(g, io.err, x, y, z)? else {
        return Ok(0);
    };
    let bins = bins.to_vec();
    let pt = [widen(p[0]), widen(p[1]), widen(p[2])];
    let dir = [widen(n[0]), widen(n[1]), widen(n[2])];
    let mut min_dist = 10000.0f64;
    let mut found = 0;
    for fno in bins {
        let fno = table_face(g, io.err, mris, fno, WHO)?;
        if mris.face(fno).v.contains(&vno) {
            continue;
        }
        let [u0, u1, u2] = load_triangle_vertices(mris, fno, CURRENT_VERTICES);
        let mut hit = [0f64; 3];
        if triangle_ray_intersect(g, io.out, io.err, pt, dir, u0, u1, u2, &mut hit)? == 0 {
            continue;
        }
        let dx = narrow(sub64(hit[0], pt[0]));
        let dy = narrow(sub64(hit[1], pt[1]));
        let dz = narrow(sub64(hit[2], pt[2]));
        let s = add(add(mul(dx, dx), mul(dy, dy)), mul(dz, dz));
        let dot = add(add(mul(dx, n[0]), mul(dy, n[1])), mul(dz, n[2]));
        if dot >= 0.0 {
            let d = widen(sqrt(s));
            if min_dist > d {
                found = 1;
                min_dist = d;
                *pdist = d;
            }
        }
    }
    Ok(found)
}

/// `MRISdistanceToSurface(mris, mht, x0, y0, z0, nx, ny, nz)`: the direction made a unit one
/// (`n / sqrt((nx² + ny²) + nz²)`, the root in `float`, each quotient in `double` rounded to
/// `float`); then [`direction_triangle_intersection`] at `dist` 0, 0.25, 0.5, ... while `dist <
/// 128` (no vertex excluded): the first hit's distance, as `float`; 0 without one.
pub fn distance_to_surface(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    mht: &Mht,
    p: [f32; 3],
    n: [f32; 3],
) -> Result<f32, Fatal> {
    let len = widen(sqrt(add(
        add(mul(n[0], n[0]), mul(n[1], n[1])),
        mul(n[2], n[2]),
    )));
    let n = n.map(|c| narrow(div64(widen(c), len)));
    let mut dist = 0.0f64;
    loop {
        if direction_triangle_intersection(g, io, mris, p, n, mht, &mut dist, -1)? != 0 {
            return Ok(narrow(dist));
        }
        dist = add64(0.25, dist);
        if !(128.0 > dist) {
            return Ok(0.0);
        }
    }
}

/// The number of values `for (x = lo; x <= hi; x += 0.5)` takes (0 when it takes none; infinite
/// for a bound that is not finite).
fn steps(lo: f64, hi: f64) -> f64 {
    if !(lo <= hi) {
        0.0
    } else {
        ((hi - lo) / 0.5).floor() + 1.0
    }
}

/// `mrisFindAllOverlappingFaces(mris, mht, fno, flist)`: face `fno`'s bounding box over its
/// corners' current positions (in `double`, from 100,000 and -100,000: a corner that is not finite
/// leaves its bound); every bucket of the table at the points `(x, y, z)` from the box's low corner
/// up to its high one in half-millimetre steps (`double`, rounded to `float` for the lookup; `x`
/// outermost), a bucket the same as the one before skipped, collecting in order each listed face
/// whose rip flag is clear, once (its flag set as the mark; more than 1,000,000 end the program:
/// `Too many faces`, `ERROR_BADPARM`); the marks cleared. Then for each collected face in order and
/// each edge of `fno` (corner `n` to corner `n + 1`), each edge of that face while it is not marked:
/// edges that share a corner are passed over; edges that cross (fs.surf's `edgesIntersect`, on the
/// canonical positions) mark the face and list it (more than 100,000 end the program: `Too many
/// intersected faces for face <fno> (<v0>, <v1>, <v2>)`). The list: `fno` first, then the faces
/// found; their marks (`fno`'s rip flag too) cleared. FS-1013: a walk of more than
/// [`WALK_LIMIT`] buckets is refused.
pub fn find_all_overlapping_faces(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    mht: &Mht,
    fno: usize,
) -> Result<Vec<i32>, Fatal> {
    const WHO: &str = "mrisFindAllOverlappingFaces";
    let (mut x0, mut y0, mut z0) = (100000.0f64, 100000.0f64, 100000.0f64);
    let (mut x1, mut y1, mut z1) = (-100000.0f64, -100000.0f64, -100000.0f64);
    let f1 = mris.face(fno).v;
    for &vno in &f1 {
        let v = mris.vertex(vno as usize);
        let (x, y, z) = (widen(v.x), widen(v.y), widen(v.z));
        x0 = if x < x0 { x } else { x0 };
        y0 = if y < y0 { y } else { y0 };
        z0 = if z < z0 { z } else { z0 };
        x1 = if x > x1 { x } else { x1 };
        y1 = if y > y1 { y } else { y1 };
        z1 = if z > z1 { z } else { z1 };
    }
    let walk = steps(x0, x1) * steps(y0, y1) * steps(z0, z1);
    if !(walk <= WALK_LIMIT) {
        let message = format!(
            "{WHO}: face {fno}'s bounding box ({x0}, {y0}, {z0}) to ({x1}, {y1}, {z1}) needs {walk} lookups at 0.5 mm (FreeSurfer runs for hours or never ends): refused"
        );
        g.notices.add(&FS_1013, message.clone());
        log::error_exit(g, io.err, ERROR_BADPARM, &message)?;
    }
    let mut flist = vec![fno as i32];
    let mut all: Vec<i32> = Vec::new();
    let mut prev: Option<u64> = None;
    let mut x = x0;
    while x <= x1 {
        let mut y = y0;
        while y <= y1 {
            let mut z = z0;
            while z <= z1 {
                if let Some((k, bins)) =
                    mht.acq_bucket(g, io.err, narrow(x), narrow(y), narrow(z))?
                    && prev != Some(k)
                {
                    prev = Some(k);
                    for &f2 in bins {
                        let f2u = table_face(g, io.err, mris, f2, WHO)?;
                        if mris.face(f2u).ripflag == 0 {
                            if all.len() == MAX_ALL_FACES {
                                log::error_exit(g, io.err, ERROR_BADPARM, "Too many faces")?;
                            }
                            all.push(f2);
                            mris.face_mut(f2u).ripflag = 1;
                        }
                    }
                }
                z = add64(z, 0.5);
            }
            y = add64(y, 0.5);
        }
        x = add64(x, 0.5);
    }
    for &f in &all {
        mris.face_mut(f as usize).ripflag = 0;
    }
    for &f2 in &all {
        let f2u = f2 as usize;
        for n in 0..3 {
            let e1 = Edge {
                vno1: f1[n],
                vno2: f1[(n + 1) % 3],
                ..Edge::default()
            };
            for m in 0..3 {
                if mris.face(f2u).ripflag != 0 {
                    continue;
                }
                let v2 = mris.face(f2u).v;
                let e2 = Edge {
                    vno1: v2[m],
                    vno2: v2[(m + 1) % 3],
                    ..Edge::default()
                };
                if e2.vno1 == e1.vno1
                    || e2.vno1 == e1.vno2
                    || e2.vno2 == e1.vno1
                    || e2.vno2 == e1.vno2
                {
                    continue;
                }
                if edges_intersect(mris, &e1, &e2) != 0 {
                    mris.face_mut(f2u).ripflag = 1;
                    if flist.len() == MAX_INT_FACES {
                        log::error_exit(
                            g,
                            io.err,
                            ERROR_BADPARM,
                            &format!(
                                "Too many intersected faces for face {fno} ({}, {}, {})",
                                f1[0], f1[1], f1[2]
                            ),
                        )?;
                    }
                    flist.push(f2);
                }
            }
        }
    }
    for &f in &flist {
        mris.face_mut(f as usize).ripflag = 0;
    }
    Ok(flist)
}
