// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surf's `utils/mrisurf_vals.cpp`: the long-range neighbours of every vertex sampled with
//! their distances along the surface (queue item 3.4-03, taken early; relay 40 of track D):
//! `MRISsampleAtEachDistance` (line 3444), `MRISsampleDistances` (3484) and its body
//! `MRISsampleDistances_new` (3508-4110), for `mris_register`, `mris_sphere` and
//! `mris_curvature`.
//!
//! Each unripped vertex's neighbourhood is set to its largest size (`MRIS_setNsizeCur`), its
//! neighbour list grown by the counts asked for, then its rings walked outwards through the
//! unripped vertices (the vertices' `marked` holding each one's ring, its `d` the distance found:
//! the straight one for rings 1 and 2, beyond them the shortest path through the ring before, each
//! edge divided by 1.09, or `(1 + sqrt 2) / 2` on a quadrangle surface, then shortened through the
//! same ring, all in `float`); each ring asked for is appended whole when it has no more vertices
//! than asked, else that many drawn at random (`randomNumber`, rounded by `nint`; a draw too close
//! in angle to one taken, below `0.9 * 2 pi / count`, is drawn again, the angle shrinking by a
//! quarter after as many tries as the ring has vertices), its distances the vertices' `d` as the
//! original distances. Then the immediate neighbours' original distances the straight ones, each
//! pair's two distances their mean, the zero distances reported and `avg_nbrs` the mean count of
//! neighbours in use. The marks and `d` of the vertices walked are 0 afterwards (a mark set before
//! counts as in a ring already).
//!
//! Defects (`larmorx_core::defects`):
//! - **FS-974** (crash): with `Gdiag`'s heartbeat bit, a surface of fewer than 10 vertices divides
//!   by 0 in the progress report (`vno % (nvertices / 10)`, signal 8): by default every vertex is
//!   reported; `reproduce` ends as the official build does (noted).
//! - **FS-975** (out of bounds): `MRISsampleAtEachDistance` writes its counts past its array of
//!   200 for a neighbourhood of 200 or more; both modes go on to `MRISsampleDistances`, which stops
//!   at 100 (noted).
//! - **FS-976** (crash): counts that sum below 0, or a vertex's room for its neighbours above the
//!   32,767 its count can hold, make FreeSurfer read and write outside the vertex's list: a clear
//!   error in both modes (noted).
//! - **FS-977** (out of bounds, crash): `Gdiag_no` beyond the surface's vertices, or a ripped one
//!   without original distances: the diagnostic log reads outside them (the second through a null
//!   pointer: signal 11, as `reproduce` ends); otherwise both modes skip the log (noted).

use std::path::Path;

use larmorx_core::defects::{FS_974, FS_975, FS_976, FS_977};
use larmorx_freesurfer_core::cstd::{atoi, fmt_f};
use larmorx_freesurfer_core::diag::{self, DIAG_HEARTBEAT, DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_OUT_OF_BOUNDS, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::matrix_ops::vector3_angle;
use larmorx_freesurfer_core::random::random_number;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::nint;

use crate::alloc::c_realloc;
use crate::coords::Io;
use crate::curvfile::write_curvature;
use crate::dist::{grow_dist, grow_dist_orig};
use crate::gifti::LEFT_HEMISPHERE;
use crate::mris::Mris;
use crate::names::MRIS_BINARY_QUADRANGLE_FILE;
use crate::nbhd::set_nsize_cur;
use crate::topology::valid_vertices;

/// `MAX_NBHD_VERTICES` (`include/mrisurf_base.h`): the walk's lists.
pub const MAX_NBHD_VERTICES: usize = 20000;
/// `MAX_NBHD_SIZE` (`include/mrisurf_base.h`): `MRISsampleAtEachDistance`'s counts.
pub const MAX_NBHD_SIZE: i32 = 200;
/// `UNFOUND_DIST` (`include/mrisurf_base.h`).
const UNFOUND_DIST: f32 = 1_000_000.0;
/// `TRIANGLE_DISTANCE_CORRECTION`.
const TRIANGLE_DISTANCE_CORRECTION: f32 = 1.09;

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

fn vals_cpp() -> String {
    format!("{}/utils/mrisurf_vals.cpp", diag::BUILD_ROOT)
}

/// `float` distance of two points given as differences (`sqrt(xd*xd + yd*yd + zd*zd)` in
/// `float`).
fn fdist(xd: f32, yd: f32, zd: f32) -> f32 {
    sse::sqrt(sse::add(
        sse::add(sse::mul(xd, xd), sse::mul(yd, yd)),
        sse::mul(zd, zd),
    ))
}

/// The straight `float` distance between vertices `a` and `b` (`b - a`).
fn between(mris: &Mris, a: usize, b: usize) -> f32 {
    let (p, q) = (mris.vertex(a), mris.vertex(b));
    fdist(sse::sub(q.x, p.x), sse::sub(q.y, p.y), sse::sub(q.z, p.z))
}

/// The vertex's neighbour list entry `n`.
fn nb(mris: &Mris, vno: usize, n: usize) -> usize {
    mris.topology(vno)
        .v
        .as_ref()
        .expect("VERTEX_TOPOLOGY::v is NULL")[n] as usize
}

fn vec3(g: &mut Globals, io: &mut Io) -> Result<Matrix, Fatal> {
    Ok(matrix_alloc(g, io.out, io.err, 3, 1, MATRIX_REAL)?.expect("a vector"))
}

/// `VECTOR_LOAD(v, x, y, z)`.
fn load(v: &mut Matrix, x: f32, y: f32, z: f32) {
    v.set_relt(1, 1, x);
    v.set_relt(2, 1, y);
    v.set_relt(3, 1, z);
}

/// `FZERO(f)` of a `float`.
fn fzero(f: f32) -> bool {
    f.abs() < f32::EPSILON
}

/// `MRISsampleAtEachDistance(mris, nbhd_size, nbrs_per_distance)`: nothing for a size of 0;
/// with `Gdiag`'s heartbeat or show bit `sampling long-range distances` (no newline) on standard
/// output; then [`sample_distances`] with `nbrs_per_distance` at every distance up to
/// `nbhd_size`. FS-975: 200 or more counts are FreeSurfer's write past its array; noted.
pub fn sample_at_each_distance(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    nbhd_size: i32,
    nbrs_per_distance: i32,
) -> Result<i32, Fatal> {
    if nbhd_size == 0 {
        return Ok(NO_ERROR);
    }
    if g.gdiag & (DIAG_HEARTBEAT | DIAG_SHOW) != 0 {
        let _ = write!(io.out, "sampling long-range distances");
    }
    if nbhd_size >= MAX_NBHD_SIZE {
        g.notices.add(
            &FS_975,
            format!(
                "MRISsampleAtEachDistance: {} counts for a neighbourhood of {nbhd_size} (FreeSurfer writes past its array of {MAX_NBHD_SIZE})",
                nbhd_size + 1
            ),
        );
    }
    let n = usize::try_from(nbhd_size + 1).unwrap_or(0);
    let nbrs = vec![nbrs_per_distance; n.max(1)];
    sample_distances(g, io, dir, mris, &nbrs, nbhd_size)
}

/// `MRISsampleDistances(mris, nbrs, max_nbhd)`: every unripped vertex's long-range neighbours
/// sampled (the module's text), `nbrs[n]` of them at `n` hops for `n` from 1 to `max_nbhd`
/// (`nbrs` must hold `max_nbhd + 1` counts). `NO_ERROR`; `ErrorExit` (`ERROR_OUT_OF_BOUNDS`) for
/// a `max_nbhd` of 100 or more (`MRISsampleDistances: temporaries are too small`), vertices of
/// different largest neighbourhoods (`... different vertexs had different nsizeMax`), more than
/// 20,000 vertices walked from one (`... too many neighbors`); `exit(1)` after `***** WARNING -
/// surface distance not found at vno <v>, vall[<n>] = <w> (vnb[<n>] = <u> ******` for a ring
/// vertex no path reaches within 500 km; a failed assertion where FreeSurfer's fails. With
/// `VDIAG1` (and `VDIAG2`) the pair's distance is printed at each ring; with `Gdiag_no` and the
/// verbose bit its vertex's walk and samples are written to `v<n>` and `vn<n>` and every vertex's
/// `curv` set to its `d` and written to `lh.dist` (`rh.dist`); with `Gdiag_no` its samples to
/// `v<n>.log`; the heartbeat bit reports the progress (FS-974), the show and verbose bits
/// `avg_nbrs`. On standard error the zero distances (the first 49 and `further zeroes elided`,
/// `first zero distance at ...` at the first one that is not zero (FreeSurfer's wording), `Of <n>
/// neighbours, <k> are at distance zero`).
pub fn sample_distances(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    nbrs: &[i32],
    max_nbhd: i32,
) -> Result<i32, Fatal> {
    let atoi_env = |io: &Io, k: &str| io.env.get(k).map(|v| atoi(v.to_string_lossy().as_bytes()));
    let diag_vno1 = atoi_env(io, "VDIAG1").unwrap_or(-1);
    let diag_vno2 = atoi_env(io, "VDIAG2").unwrap_or(-1);
    if diag_vno1 >= 0 {
        let _ = writeln!(io.out, "\nlooking for vertex pair {diag_vno1}, {diag_vno2}");
    }
    let dist_scale: f32 = if mris.r#type == MRIS_BINARY_QUADRANGLE_FILE {
        ((1.0 + 2.0f64.sqrt()) / 2.0) as f32
    } else {
        TRIANGLE_DISTANCE_CORRECTION
    };
    if max_nbhd >= 100 {
        log::error_exit(
            g,
            io.err,
            ERROR_OUT_OF_BOUNDS,
            "MRISsampleDistances: temporaries are too small",
        )?;
    }
    let mut v1 = vec3(g, io)?;
    let mut v2 = vec3(g, io)?;
    let mut vnbrs = vec![0i32; MAX_NBHD_VERTICES];
    let mut vall = vec![0usize; MAX_NBHD_VERTICES];
    let mut vnb = vec![0usize; MAX_NBHD_VERTICES];
    let count = |n: i32| nbrs[n as usize];
    let mut max_possible: i64 = 0;
    let mut vtotal_asked: i64 = 0;
    for n in 1..=max_nbhd {
        max_possible += i64::from(count(n));
        if n > i32::from(mris.nsize) {
            vtotal_asked += i64::from(count(n));
        }
    }
    if max_possible < 0 {
        return refuse(g, io, &format!("counts that sum to {max_possible}"));
    }
    if g.gdiag & DIAG_HEARTBEAT != 0 {
        let vt = vtotal_asked as i32 as f32;
        let _ = write!(
            io.out,
            "\nsampling {} dists/vertex ({} at each dist) = {}MB\n",
            vtotal_asked as i32,
            fmt_f(
                f64::from(sse::div(
                    vt,
                    sse::sub(max_nbhd as f32, f32::from(mris.nsize))
                )),
                2,
                1
            ),
            fmt_f(
                f64::from(sse::div(
                    sse::mul(
                        sse::mul(sse::mul(vt, valid_vertices(mris) as f32), 4.0),
                        3.0
                    ),
                    1024.0 * 1024.0
                )),
                2,
                1
            ),
        );
    }
    let mut adjusted_mris_nsize = false;
    let mut total_nbrs: i64 = 0;
    for vno in 0..nverts(mris) {
        if g.gdiag & DIAG_HEARTBEAT != 0 {
            let tenth = mris.nvertices / 10;
            if tenth == 0 {
                g.notices.add(
                    &FS_974,
                    format!(
                        "MRISsampleDistances: the progress of a surface of {} vertices (FreeSurfer divides by 0: signal 8)",
                        mris.nvertices
                    ),
                );
                if g.notices.reproduce() {
                    let _ = io.out.flush();
                    return Err(Fatal::Exit(136));
                }
            }
            if tenth == 0 || vno as i32 % tenth == 0 {
                let p = sse::div(sse::mul(100.0, vno as f32), mris.nvertices as f32);
                let _ = writeln!(io.out, "%{} done", fmt_f(f64::from(p), 1, 0));
            }
        }
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        if mris.topology(vno).nsizeMax == 0 {
            return Err(diag::assert_failed(
                g,
                io.out,
                io.err,
                &vals_cpp(),
                3604,
                "vt->nsizeMax > 0",
            ));
        }
        let nsize_max = i32::from(mris.topology(vno).nsizeMax);
        set_nsize_cur(g, io, mris, vno, nsize_max)?;
        let cur = mris.topology(vno).nsizeCur;
        if !adjusted_mris_nsize {
            adjusted_mris_nsize = true;
            mris.nsize = cur as i8;
        } else if i32::from(mris.nsize) != i32::from(cur) {
            log::error_exit(
                g,
                io.err,
                ERROR_OUT_OF_BOUNDS,
                "MRISsampleDistances: different vertexs had different nsizeMax",
            )?;
        }
        let vtotal0 = i64::from(mris.topology(vno).vtotal);
        let cap = vtotal0 + max_possible;
        if cap > i64::from(i16::MAX) {
            return refuse(g, io, &format!("room for {cap} neighbours at vertex {vno}"));
        }
        let v_capacity = cap as usize;
        {
            let vt = mris.topology_mut(vno);
            let mut list = vt.v.take();
            // realloc(vt->v, vCapacity * sizeof(int))
            let ok = c_realloc(g, &mut list, v_capacity, "VERTEX_TOPOLOGY::v");
            debug_assert!(ok || v_capacity == 0);
            mris.topology_mut(vno).v = list;
        }
        grow_dist(mris, vno, v_capacity as i32);
        grow_dist_orig(mris, vno, v_capacity as i32);
        for i in vtotal0 as usize..v_capacity {
            mris.topology_mut(vno).v.as_mut().expect("v")[i] = 0;
            let v = mris.vertex_mut(vno);
            v.dist.as_mut().expect("dist")[i] = 0.0;
            v.dist_orig.as_mut().expect("dist_orig")[i] = 0.0;
        }
        vall[0] = vno;
        let mut old_vnum = 0usize;
        let mut vall_num = 1usize;
        mris.vertex_mut(vno).marked = 1;
        for nbhd_size in 1..=max_nbhd {
            let mut vnbrs_num = 0usize;
            let mut vnum = vall_num;
            let mut found = 0usize;
            for n in old_vnum..vall_num {
                let w = vall[n];
                if mris.vertex(w).ripflag != 0 {
                    continue;
                }
                let k = usize::try_from(mris.topology(w).vnum).unwrap_or(0);
                for n2 in 0..k {
                    let w2 = nb(mris, w, n2);
                    let vn2 = mris.vertex(w2);
                    if vn2.ripflag != 0 || vn2.marked != 0 {
                        continue;
                    }
                    if vnum >= MAX_NBHD_VERTICES {
                        log::error_exit(
                            g,
                            io.err,
                            ERROR_OUT_OF_BOUNDS,
                            "MRISsampleDistances: too many neighbors",
                        )?;
                    }
                    found += 1;
                    mris.vertex_mut(w2).marked = nbhd_size as i16;
                    vnb[vnum] = w;
                    vall[vnum] = w2;
                    vnum += 1;
                    if count(nbhd_size) > 0 {
                        if vnbrs_num >= vnum {
                            return Err(diag::assert_failed(
                                g,
                                io.out,
                                io.err,
                                &vals_cpp(),
                                3685,
                                "vnbrs_num < vnum",
                            ));
                        }
                        vnbrs[vnbrs_num] = w2 as i32;
                        vnbrs_num += 1;
                    }
                }
            }
            old_vnum = vall_num;
            vall_num += found;
            if vall_num != vnum {
                return Err(diag::assert_failed(
                    g,
                    io.out,
                    io.err,
                    &vals_cpp(),
                    3696,
                    "vall_num == vnum",
                ));
            }
            // the ring's distances
            for n in old_vnum..vall_num {
                let w = vall[n];
                if mris.vertex(w).ripflag != 0 {
                    continue;
                }
                let mut min_dist = UNFOUND_DIST;
                let k = usize::try_from(mris.topology(w).vnum).unwrap_or(0);
                for n2 in 0..k {
                    let w2 = nb(mris, w, n2);
                    let vn2 = mris.vertex(w2);
                    if vn2.ripflag != 0 {
                        continue;
                    }
                    if vn2.marked == 0 || i32::from(vn2.marked) == nbhd_size {
                        continue;
                    }
                    let mut dist = between(mris, w, w2);
                    if nbhd_size > 1 {
                        dist = sse::div(dist, dist_scale);
                    }
                    dist = sse::add(dist, mris.vertex(w2).d);
                    if min_dist > dist {
                        min_dist = dist;
                    }
                }
                mris.vertex_mut(w).d = min_dist;
                if nbhd_size <= 2 {
                    let dist = between(mris, vno, w);
                    mris.vertex_mut(w).d = dist;
                }
                if mris.vertex(w).d >= UNFOUND_DIST / 2.0 {
                    let _ = write!(
                        io.out,
                        "***** WARNING - surface distance not found at vno {vno}, vall[{n}] = {w} (vnb[{n}] = {} ******",
                        vnb[n]
                    );
                    let _ = io.out.flush();
                    return Err(Fatal::Exit(1));
                }
                if (w as i32 == diag_vno1 && vno as i32 == diag_vno2)
                    || (w as i32 == diag_vno2 && vno as i32 == diag_vno1)
                {
                    let _ = writeln!(
                        io.out,
                        "vn {w} is {} mm from v {vno} at ring {nbhd_size}",
                        fmt_f(f64::from(mris.vertex(w).d), 2, 3)
                    );
                }
            }
            // a path through the same ring may be shorter
            for n in old_vnum..vall_num {
                let w = vall[n];
                if mris.vertex(w).ripflag != 0 {
                    continue;
                }
                let mut min_dist = mris.vertex(w).d;
                let k = usize::try_from(mris.topology(w).vnum).unwrap_or(0);
                for n2 in 0..k {
                    let w2 = nb(mris, w, n2);
                    let vn2 = mris.vertex(w2);
                    if vn2.marked == 0 || i32::from(vn2.marked) != nbhd_size || vn2.ripflag != 0 {
                        continue;
                    }
                    let mut dist = between(mris, w, w2);
                    if nbhd_size > 1 {
                        dist = sse::div(dist, dist_scale);
                    }
                    let through = sse::add(mris.vertex(w2).d, dist);
                    if through < min_dist {
                        min_dist = through;
                    }
                }
                mris.vertex_mut(w).d = min_dist;
            }
            let asked = count(nbhd_size);
            if asked <= 0 {
                continue;
            }
            let mut min_angle =
                ((0.9f64 * 2.0) * std::f64::consts::PI / f64::from(asked as f32)) as f32;
            if found <= asked as usize {
                for n in 0..found {
                    let vt = mris.topology(vno).vtotal;
                    if i64::from(vt) >= cap {
                        return Err(diag::assert_failed(
                            g,
                            io.out,
                            io.err,
                            &vals_cpp(),
                            3844,
                            "vt->vtotal < vCapacity",
                        ));
                    }
                    let w = vnbrs[n] as usize;
                    let d = mris.vertex(w).d;
                    mris.topology_mut(vno).v.as_mut().expect("v")[vt as usize] = w as i32;
                    mris.vertex_mut(vno).dist_orig.as_mut().expect("dist_orig")[vt as usize] = d;
                    mris.topology_mut(vno).vtotal = vt + 1;
                }
            } else {
                let vstart = mris.topology(vno).vtotal;
                for _ in 0..asked {
                    let mut niter = 0usize;
                    let mut i;
                    loop {
                        loop {
                            let r =
                                random_number(g, io.env, io.out, io.err, 0.0, found as f64 - 1.0)?;
                            i = nint(g, r) as usize;
                            if vnbrs[i] >= 0 {
                                break;
                            }
                        }
                        let w = vnbrs[i] as usize;
                        let (p, q) = (mris.vertex(w), mris.vertex(vno));
                        load(
                            &mut v1,
                            sse::sub(p.x, q.x),
                            sse::sub(p.y, q.y),
                            sse::sub(p.z, q.z),
                        );
                        let mut done = true;
                        let vtotal = mris.topology(vno).vtotal;
                        let mut j = vstart;
                        while done && j < vtotal {
                            let w2 = nb(mris, vno, j as usize);
                            let (p, q) = (mris.vertex(w2), mris.vertex(vno));
                            load(
                                &mut v2,
                                sse::sub(p.x, q.x),
                                sse::sub(p.y, q.y),
                                sse::sub(p.z, q.z),
                            );
                            let angle = vector3_angle(g, io.err, &v1, &v2) as f32;
                            if angle < min_angle {
                                done = false;
                            }
                            j += 1;
                        }
                        niter += 1;
                        if niter > found {
                            min_angle = sse::mul(min_angle, 0.75);
                            niter = 0;
                        }
                        if done || fzero(min_angle) {
                            break;
                        }
                    }
                    let vt = mris.topology(vno).vtotal;
                    if i64::from(vt) >= cap {
                        return Err(diag::assert_failed(
                            g,
                            io.out,
                            io.err,
                            &vals_cpp(),
                            3889,
                            "vt->vtotal < vCapacity",
                        ));
                    }
                    let w = vnbrs[i] as usize;
                    let d = mris.vertex(w).d;
                    mris.topology_mut(vno).v.as_mut().expect("v")[vt as usize] = w as i32;
                    mris.vertex_mut(vno).dist_orig.as_mut().expect("dist_orig")[vt as usize] = d;
                    vnbrs[i] = -1;
                    mris.topology_mut(vno).vtotal = vt + 1;
                }
            }
        }
        if g.gdiag_no == vno as i32 && g.gdiag & DIAG_VERBOSE != 0 {
            let at = |name: String| match dir {
                Some(d) => d.join(name),
                None => name.into(),
            };
            let mut text = format!("{vall_num}\n");
            for &w in &vall[..vall_num] {
                text += &format!("{w}\n");
            }
            let _ = std::fs::write(at(format!("v{vno}")), text);
            let vt = mris.topology(vno);
            let mut text = format!("{}\n", vt.vtotal);
            for &w in &vt.v.as_deref().unwrap_or(&[])[..vt.vtotal.max(0) as usize] {
                text += &format!("{w}\n");
            }
            let _ = std::fs::write(at(format!("vn{vno}")), text);
            for n in 0..nverts(mris) {
                let v = mris.vertex_mut(n);
                v.curv = v.d;
            }
            let fname = if mris.hemisphere == LEFT_HEMISPHERE {
                "lh.dist"
            } else {
                "rh.dist"
            };
            write_curvature(g, io, dir, mris, fname, None)?;
        }
        for &w in &vall[..vall_num] {
            let v = mris.vertex_mut(w);
            v.marked = 0;
            v.d = 0.0;
        }
        total_nbrs += i64::from(mris.topology(vno).vtotal);
    }
    // the immediate neighbours' straight distances
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let vt = mris.topology(vno);
        let immediate = match vt.nsizeMax {
            3 => vt.v3num,
            2 => vt.v2num,
            _ => vt.vnum,
        };
        for n in 0..usize::try_from(immediate).unwrap_or(0) {
            let w = nb(mris, vno, n);
            if mris.vertex(w).ripflag != 0 {
                continue;
            }
            let (p, q) = (mris.vertex(vno), mris.vertex(w));
            let d = fdist(sse::sub(p.x, q.x), sse::sub(p.y, q.y), sse::sub(p.z, q.z));
            mris.vertex_mut(vno).dist_orig.as_mut().expect("dist_orig")[n] = d;
        }
    }
    mris.vtotalsMightBeTooBig = 1;
    // each pair's two distances their mean
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let vtotal = usize::try_from(mris.topology(vno).vtotal).unwrap_or(0);
        for n in 0..vtotal {
            let w = nb(mris, vno, n);
            if mris.vertex(w).ripflag != 0 {
                continue;
            }
            let wtotal = usize::try_from(mris.topology(w).vtotal).unwrap_or(0);
            for i in 0..wtotal {
                if nb(mris, w, i) == vno {
                    let a = mris.vertex(w).dist_orig.as_ref().expect("dist_orig")[i];
                    let b = mris.vertex(vno).dist_orig.as_ref().expect("dist_orig")[n];
                    let dist = sse::mul(sse::add(a, b), 0.5);
                    mris.vertex_mut(w).dist_orig.as_mut().expect("dist_orig")[i] = dist;
                    mris.vertex_mut(vno).dist_orig.as_mut().expect("dist_orig")[n] = dist;
                    break;
                }
            }
        }
    }
    // the zero distances
    let (mut zeroes, mut non_zeroes) = (0u64, 0u64);
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let vtotal = usize::try_from(mris.topology(vno).vtotal).unwrap_or(0);
        for n in 0..vtotal {
            let d = mris.vertex(vno).dist_orig.as_ref().expect("dist_orig")[n];
            let w = nb(mris, vno, n);
            // DZERO: |d| < 1e-15 (a NaN is not zero)
            if !(f64::from(d).abs() < 1e-15) {
                if non_zeroes == 0 && vno != 0 {
                    let _ = writeln!(io.err, "first zero distance at v {vno}, n {n} (vn = {w})");
                }
                non_zeroes += 1;
            } else {
                zeroes += 1;
                if zeroes < 50 {
                    let _ = writeln!(io.err, "zero distance at v {vno}, n {n} (vn = {w})");
                } else if zeroes == 50 {
                    let _ = writeln!(io.err, "further zeroes elided");
                }
            }
        }
    }
    if zeroes > 0 {
        let _ = writeln!(
            io.err,
            "Of {} neighbours, {zeroes} are at distance zero",
            zeroes + non_zeroes
        );
    }
    if g.gdiag_no >= 0 {
        let beyond = g.gdiag_no >= mris.nvertices;
        let no_dist = !beyond && {
            let gv = g.gdiag_no as usize;
            mris.topology(gv).vtotal > 0 && mris.vertex(gv).dist_orig.is_none()
        };
        if beyond || no_dist {
            g.notices.add(
                &FS_977,
                format!(
                    "MRISsampleDistances: Gdiag_no {} {}: its log skipped (FreeSurfer reads outside the vertices' data)",
                    g.gdiag_no,
                    if beyond {
                        format!("beyond a surface of {} vertices", mris.nvertices)
                    } else {
                        "a ripped vertex without original distances".to_owned()
                    }
                ),
            );
            // the official build reads through a null pointer there: signal 11
            if no_dist && g.notices.reproduce() {
                let _ = std::fs::File::create(match dir {
                    Some(d) => d.join(format!("v{}.log", g.gdiag_no)),
                    None => format!("v{}.log", g.gdiag_no).into(),
                });
                return Err(Fatal::Exit(139));
            }
        } else {
            let gv = g.gdiag_no as usize;
            let vt = mris.topology(gv);
            let mut text = String::new();
            for i in 0..usize::try_from(vt.vtotal).unwrap_or(0) {
                let d = mris.vertex(gv).dist_orig.as_ref().expect("dist_orig")[i];
                text += &format!(
                    "{i}: {}, {}\n",
                    vt.v.as_ref().expect("v")[i],
                    fmt_f(f64::from(d), 0, 6)
                );
            }
            let name = format!("v{gv}.log");
            let path = match dir {
                Some(d) => d.join(name),
                None => name.into(),
            };
            let _ = std::fs::write(path, text);
        }
    }
    mris.avg_nbrs = sse::div(total_nbrs as f32, valid_vertices(mris) as f32);
    if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let _ = writeln!(
            io.out,
            "avg_nbrs = {}",
            fmt_f(f64::from(mris.avg_nbrs), 2, 1)
        );
    }
    if g.gdiag & DIAG_HEARTBEAT != 0 {
        let _ = writeln!(io.out, " done.");
    }
    Ok(NO_ERROR)
}

/// FS-976: the clear error for counts FreeSurfer cannot hold.
fn refuse(g: &mut Globals, io: &mut Io, what: &str) -> Result<i32, Fatal> {
    g.notices.add(
        &FS_976,
        format!(
            "MRISsampleDistances: {what} (FreeSurfer reads and writes outside the vertices' lists)"
        ),
    );
    log::error_exit(
        g,
        io.err,
        ERROR_BADPARM,
        &format!("MRISsampleDistances: cannot sample {what}"),
    )?;
    Ok(ERROR_BADPARM)
}
