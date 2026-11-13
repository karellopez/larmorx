// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Queue item 3.1-51, the rest (relay 20), fourth group: `utils/mrisurf_metricProperties.cpp`'s
//! surface bookkeeping (lines 11545-13543): a group surface's metric properties rescaled
//! (`MRISrescaleMetricProperties`), per-vertex values divided by the degrees of freedom
//! (`MRISnormalize`), the metric properties and second fundamental form together
//! (`MRISupdateSurface`), the face orientation check (`MRISmarkOrientationChanges`, with
//! `mrisurf_topology.cpp`'s `findFace`, `isFace` and `computeOrientation` of fs.surf taken early),
//! the face-list check (`mrisCheckSurface`), the subsampling by hops (`MRISsubsampleDist`), the
//! extended neighbours on a sphere (`MRISextendedNeighbors`), two surfaces in one (`MRISunion`),
//! a copy of a surface (`MRISclone`) and the two vertex reports (`MRISprintVertexStats`,
//! `MRISprintVertexInfo`).
//!
//! Arithmetic through fs.core's `sse` operations in the official build's operand order (the unit
//! oracle's object code; `docs/findings/freesurfer-surfio.md` §22). Defect
//! (`larmorx_core::defects`):
//! - **FS-380** (memory): `MRISprintVertexInfo` lists a vertex's neighbours by its face count, so
//!   a vertex with more faces than room in its neighbour list reads past it. Both modes stop at
//!   the list's end (noted).

use std::io::Write;

use larmorx_core::defects::FS_380;
use larmorx_freesurfer_core::cstd::{double_to_int_x86, fmt_f};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt, sqrt64, sub};
use larmorx_freesurfer_volume::vgeom::vg_is_equal;

use crate::aaa::SurfGlobals;
use crate::alloc;
use crate::coords::{self, Io, metric_properties_cpp};
use crate::dist::{make_dist, make_dist_orig};
use crate::metric::clear_curvature;
use crate::mp;
use crate::mris::Mris;
use crate::nbhd::set_nsize_cur;
use crate::props6::compute_second_fundamental_form;
use crate::topology::{mod_vnum, vertex_vsize};
use crate::views::{CURRENT_VERTICES, ORIGINAL_VERTICES, WHITE_VERTICES, vertex_coord2xyz_float};

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

fn nfaces(mris: &Mris) -> usize {
    usize::try_from(mris.nfaces).unwrap_or(0)
}

/// `EDOM` (Linux).
const EDOM: i32 = 33;

/// `VERTEX_AREA`, `VERTEX_CURV` (`mrisurf.h`).
pub const VERTEX_AREA: i32 = 2;
pub const VERTEX_CURV: i32 = 3;

/// `MRISrescaleMetricProperties(surf)`: 0 (nothing) when `group_avg_surface_area` is 0; else
/// `scale = sqrt(group_avg_surface_area / total_area)` (`double`; glibc's `sqrt`, `EDOM` for a
/// negative ratio), `scale = %lf` on standard output, each unripped vertex's area times `scale²`
/// and its distances to unripped neighbours (the first `vnum`) times `scale` (each in `double`,
/// back to `float`), `total_area` times `scale²`, `avg_vertex_dist` and `std_vertex_dist` times
/// `scale`, `avg_vertex_area` times `scale²`. Returns `scale`.
pub fn rescale_metric_properties(g: &mut Globals, io: &mut Io, surf: &mut Mris) -> f64 {
    if surf.group_avg_surface_area == 0.0 {
        return 0.0;
    }
    let ratio = div64(
        f64::from(surf.group_avg_surface_area),
        f64::from(surf.total_area),
    );
    if ratio < 0.0 {
        g.errno = EDOM;
    }
    let scale = sqrt64(ratio);
    let _ = writeln!(io.out, "scale = {}", fmt_f(scale, 0, 6));
    let s2 = mul64(scale, scale);
    for vno in 0..nverts(surf) {
        if surf.vertex(vno).ripflag != 0 {
            continue;
        }
        let v = surf.vertex_mut(vno);
        v.area = mul64(f64::from(v.area), s2) as f32;
        let vt = surf.topology(vno);
        let n = usize::try_from(vt.vnum).unwrap_or(0);
        let nbrs: Vec<usize> = vt.v.as_deref().unwrap_or(&[])[..n]
            .iter()
            .map(|&x| x as usize)
            .collect();
        for (k, nb) in nbrs.into_iter().enumerate() {
            if surf.vertex(nb).ripflag != 0 {
                continue;
            }
            if let Some(d) = surf.vertex_mut(vno).dist.as_mut() {
                d[k] = mul64(f64::from(d[k]), scale) as f32;
            }
        }
    }
    surf.total_area = mul64(f64::from(surf.total_area), s2) as f32;
    surf.avg_vertex_dist = mul64(scale, surf.avg_vertex_dist);
    surf.std_vertex_dist = mul64(surf.std_vertex_dist, scale);
    surf.avg_vertex_area = mul64(s2, surf.avg_vertex_area);
    scale
}

/// `MRISnormalize(mris, dof, which)`: for each unripped vertex, the value `which` names
/// (`VERTEX_AREA`: `origarea`; `VERTEX_CURV`: `curv`; anything else: `val`) divided by `dof` (as
/// `float`) and `val2 = val2 / dof - value²`. A `dof` of 0 or less: `MRISnormalize: invalid dof
/// %d` as an error, `ERROR_BADPARM`.
pub fn normalize(g: &mut Globals, io: &mut Io, mris: &mut Mris, dof: i32, which: i32) -> i32 {
    if dof <= 0 {
        log::error_printf(
            g,
            io.err,
            ERROR_BADPARM,
            &format!("MRISnormalize: invalid dof {dof}"),
        );
        return ERROR_BADPARM;
    }
    let fdof = dof as f32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        let mean = match which {
            VERTEX_AREA => {
                v.origarea = div(v.origarea, fdof);
                v.origarea
            }
            VERTEX_CURV => {
                v.curv = div(v.curv, fdof);
                v.curv
            }
            _ => {
                v.val = div(v.val, fdof);
                v.val
            }
        };
        v.val2 = sub(div(v.val2, fdof), mul(mean, mean));
    }
    NO_ERROR
}

/// `MRISupdateSurface(mris)`: the metric properties (`MRIScomputeMetricProperties`), then the
/// second fundamental form (`MRIScomputeSecondFundamentalForm`; `dir` the folder of its
/// diagnostic file).
pub fn update_surface(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&std::path::Path>,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    mp::compute_metric_properties(g, sg, io, mris)?;
    compute_second_fundamental_form(g, sg, io, dir, mris)?;
    Ok(NO_ERROR)
}

/// `findFace(mris, vno0, vno1, vno2)` (`mrisurf_topology.cpp`, fs.surf's, taken early): the first
/// face of `vno0` (its face list) whose three vertices are all among `vno0`, `vno1`, `vno2`; -1
/// when none.
pub fn find_face(mris: &Mris, vno0: i32, vno1: i32, vno2: i32) -> i32 {
    let vt = mris.topology(vno0 as usize);
    let num = usize::from(vt.num);
    for &fno in &vt.f.as_deref().unwrap_or(&[])[..num] {
        let f = mris.face(fno as usize);
        if f.v.iter().all(|&v| v == vno0 || v == vno1 || v == vno2) {
            return fno;
        }
    }
    -1
}

/// `isFace(mris, vno0, vno1, vno2)`: [`find_face`] found one.
pub fn is_face(mris: &Mris, vno0: i32, vno1: i32, vno2: i32) -> bool {
    find_face(mris, vno0, vno1, vno2) >= 0
}

/// `computeOrientation(mris, f, v0, v1)` (`mrisurf_topology.cpp`, taken early): 1 when face `f`
/// goes from `v0` to `v1` (`v0` at corner 0 and `v1` at 1, `v0` at 1 and `v1` at 2, or else `v1`
/// at 0), -1 otherwise.
pub fn compute_orientation(mris: &Mris, f: i32, v0: i32, v1: i32) -> i32 {
    let v = mris.face(f as usize).v;
    let one = |b: bool| if b { 1 } else { -1 };
    if v[0] == v0 {
        return one(v[1] == v1);
    }
    if v[1] == v0 {
        return one(v[2] == v1);
    }
    one(v[0] == v1)
}

/// `MRISmarkOrientationChanges(mris)`: the curvatures cleared (`MRISclearCurvature`), then for
/// each unripped vertex (on standard error): `vertex %d without neighbors ! ` (curvature -5),
/// `vertex %d without faces ! `, and for each neighbour (the first `vnum`) a ripped one's `vertex
/// %d connected to ripped vertex %d !` followed by a blank, the faces through the edge among the
/// vertex's other neighbours' triangles: one (`edge (%d<-->%d) has one single face %d !`,
/// curvature -2), none (`... has no face !`, -3), more than two (`... has too many faces (at least
/// : %d %d) !`, -4), or two whose orientations along the edge agree (curvature -1, the line
/// `defective orientation at vertex %d(%d) with faces %d and %d` after an empty one, counted);
/// at the end `%2.3f %% of the vertices (%d vertices) exhibit an orientation change` after an
/// empty line. Returns the count.
pub fn mark_orientation_changes(mris: &mut Mris, io: &mut Io) -> i32 {
    clear_curvature(mris);
    let (mut face1, mut face2) = (0i32, 0i32);
    let mut count = 0i32;
    for vn0 in 0..nverts(mris) {
        if mris.vertex(vn0).ripflag != 0 {
            continue;
        }
        let vt = mris.topology(vn0);
        let vnum = usize::try_from(vt.vnum).unwrap_or(0);
        let nbrs: Vec<i32> = vt.v.as_deref().unwrap_or(&[])[..vnum].to_vec();
        if vt.vnum == 0 {
            let _ = write!(io.err, "vertex {vn0} without neighbors ! \n");
            mris.vertex_mut(vn0).curv = -5.0;
        }
        if mris.topology(vn0).num == 0 {
            let _ = write!(io.err, "vertex {vn0} without faces ! \n");
        }
        let v0 = vn0 as i32;
        for &vn1 in &nbrs {
            if mris.vertex(vn1 as usize).ripflag != 0 {
                let _ = write!(io.err, "vertex {vn0} connected to ripped vertex {vn1} !\n ");
            }
            let mut nf = 0;
            for &vn2 in &nbrs {
                if vn2 == vn1 {
                    continue;
                }
                if is_face(mris, v0, vn1, vn2) {
                    nf += 1;
                    if nf == 1 {
                        face1 = find_face(mris, v0, vn1, vn2);
                    } else {
                        face2 = find_face(mris, v0, vn1, vn2);
                    }
                }
            }
            if nf == 1 {
                let _ = writeln!(
                    io.err,
                    "edge ({vn0}<-->{vn1}) has one single face {face1} !"
                );
                mris.vertex_mut(vn0).curv = -2.0;
            }
            if nf == 0 {
                let _ = writeln!(io.err, "edge ({vn0}<-->{vn1}) has no face !");
                mris.vertex_mut(vn0).curv = -3.0;
            }
            if nf > 2 {
                let _ = writeln!(
                    io.err,
                    "edge ({vn0}<-->{vn1}) has too many faces (at least : {face1} {face2}) !"
                );
                mris.vertex_mut(vn0).curv = -4.0;
            }
            if nf == 2
                && compute_orientation(mris, face1, v0, vn1)
                    * compute_orientation(mris, face2, v0, vn1)
                    > 0
            {
                mris.vertex_mut(vn0).curv = -1.0;
                let _ = writeln!(
                    io.err,
                    "\ndefective orientation at vertex {vn0}({vn1}) with faces {face1} and {face2}"
                );
                count += 1;
            }
        }
    }
    let pct = div64(mul64(100.0, f64::from(count)), f64::from(mris.nvertices));
    let _ = writeln!(
        io.err,
        "\n{} % of the vertices ({count} vertices) exhibit an orientation change",
        fmt_f(pct, 2, 3)
    );
    count
}

/// `mrisCheckSurface(mris)`: for each unripped face, each corner whose vertex's face list lacks
/// the face prints `mrisCheckSurface: %s: face[%d].v[%d] = %d, but face %d not in vertex %d face
/// list` (the surface's file name; standard output). Returns `NO_ERROR` (the rest of FreeSurfer's
/// body is never reached).
pub fn check_surface(mris: &Mris, io: &mut Io) -> i32 {
    for fno in 0..nfaces(mris) {
        let f = mris.face(fno);
        if f.ripflag != 0 {
            continue;
        }
        for (m, &vno) in f.v.iter().enumerate() {
            let vt = mris.topology(vno as usize);
            let num = usize::from(vt.num);
            let found = vt.f.as_deref().unwrap_or(&[])[..num]
                .iter()
                .any(|&x| x as usize == fno);
            if !found {
                let _ = writeln!(
                    io.out,
                    "mrisCheckSurface: {}: face[{fno}].v[{m}] = {vno}, but face {fno} not in vertex {vno} face list",
                    mris.fname
                );
            }
        }
    }
    NO_ERROR
}

/// `MRISsubsampleDist(mris, spacing)`: every vertex's `d` 10000 and `val` 0; a forward pass (each
/// vertex's `d` lowered to a neighbour's `d + 1`; a vertex whose `d` is then at least `spacing`
/// chosen: `d` 0, `val` 1, counted; its neighbours' `d` lowered to its `d + 1`), a backward pass of
/// both lowerings per neighbour, and with `spacing` 2 a pass over the unchosen vertices choosing
/// (also `fixedval`) those with at most two chosen neighbours (and lowering their neighbours'
/// `d`). In `float`; the first `vnum` neighbours. Returns the count.
pub fn subsample_dist(mris: &mut Mris, spacing: f32) -> i32 {
    let mut sub_num = 0;
    let n = nverts(mris);
    for vno in 0..n {
        let v = mris.vertex_mut(vno);
        v.d = 10000.0;
        v.val = 0.0;
    }
    let nbrs = |mris: &Mris, vno: usize| -> Vec<usize> {
        let vt = mris.topology(vno);
        let k = usize::try_from(vt.vnum).unwrap_or(0);
        vt.v.as_deref().unwrap_or(&[])[..k]
            .iter()
            .map(|&x| x as usize)
            .collect()
    };
    for k in 0..n {
        let nb = nbrs(mris, k);
        for &m in &nb {
            let dn = add(mris.vertex(m).d, 1.0);
            if dn < mris.vertex(k).d {
                mris.vertex_mut(k).d = dn;
            }
        }
        if mris.vertex(k).d >= spacing {
            let v = mris.vertex_mut(k);
            v.d = 0.0;
            v.val = 1.0;
            sub_num += 1;
        }
        for &m in &nb {
            let d1 = add(mris.vertex(k).d, 1.0);
            if mris.vertex(m).d > d1 {
                mris.vertex_mut(m).d = d1;
            }
        }
    }
    for k in (0..n).rev() {
        for m in nbrs(mris, k) {
            let dn = add(mris.vertex(m).d, 1.0);
            if dn < mris.vertex(k).d {
                mris.vertex_mut(k).d = dn;
            }
            let d1 = add(mris.vertex(k).d, 1.0);
            if mris.vertex(m).d > d1 {
                mris.vertex_mut(m).d = d1;
            }
        }
    }
    if spacing == 2.0 {
        for k in 0..n {
            if mris.vertex(k).d > 0.0 {
                let nb = nbrs(mris, k);
                let chosen = nb.iter().filter(|&&m| mris.vertex(m).d == 0.0).count();
                if chosen <= 2 {
                    let v = mris.vertex_mut(k);
                    v.d = 0.0;
                    v.val = 1.0;
                    v.fixedval = 1;
                    sub_num += 1;
                }
                for &m in &nb {
                    let d1 = add(mris.vertex(k).d, 1.0);
                    if mris.vertex(m).d > d1 {
                        mris.vertex_mut(m).d = d1;
                    }
                }
            }
        }
    }
    sub_num
}

/// What [`extended_neighbors`] keeps between the calls of one search: the neighbours found, their
/// dot products (or squared distances), and the search's own call count (FreeSurfer's `static
/// ncalls`).
#[derive(Clone, Debug, Default)]
pub struct ExtendedNeighbors {
    pub vno: Vec<i32>,
    pub dot: Vec<f64>,
    pub n: i32,
}

/// `MRISextendedNeighbors(surf, targ, cur, thresh, XNbrVtxNo, XNbrDotProd, &nXNbrs, max,
/// DistType)`: 0 for a vertex already reached in this search (`(int) val2bak` is `targ`) or
/// ripped; at the target the count starts at 0. With `DistType` 1 the vertices' dot product
/// (`float`, then `|.|` in `double`) must be above `thresh`, otherwise their squared distance
/// (`double`) below it (0 otherwise). With `max - 1` neighbours found: 1. Else the vertex is added
/// (its `val2bak` set to `targ`), and the search goes on through its neighbours (the first `vnum`)
/// depth first, stopping at the first that returns 1.
#[allow(clippy::too_many_arguments)]
pub fn extended_neighbors(
    surf: &mut Mris,
    targ: i32,
    cur: i32,
    thresh: f64,
    found: &mut ExtendedNeighbors,
    max: i32,
    dist_type: i32,
) -> i32 {
    let vcur = surf.vertex(cur as usize);
    if double_to_int_x86(f64::from(vcur.val2bak)) == targ {
        return 0;
    }
    if vcur.ripflag != 0 {
        return 0;
    }
    if cur == targ {
        found.n = 0;
        found.vno.clear();
        found.dot.clear();
    }
    let vt = surf.vertex(targ as usize);
    let (tx, ty, tz) = (vt.x, vt.y, vt.z);
    let (cx, cy, cz) = (vcur.x, vcur.y, vcur.z);
    let dot = if dist_type == 1 {
        let d = add(add(mul(cy, ty), mul(tx, cx)), mul(tz, cz));
        let d = f64::from(d).abs();
        if d <= thresh {
            return 0;
        }
        d
    } else {
        let dx = f64::from(sub(tx, cx));
        let dy = f64::from(sub(ty, cy));
        let dz = f64::from(sub(tz, cz));
        let d = add64(add64(mul64(dx, dx), mul64(dy, dy)), mul64(dz, dz)).abs();
        if d >= thresh {
            return 0;
        }
        d
    };
    if found.n >= max - 1 {
        return 1;
    }
    found.vno.push(cur);
    found.dot.push(dot);
    found.n += 1;
    surf.vertex_mut(cur as usize).val2bak = targ as f32;
    let vt = surf.topology(cur as usize);
    let k = usize::try_from(vt.vnum).unwrap_or(0);
    let nbrs: Vec<i32> = vt.v.as_deref().unwrap_or(&[])[..k].to_vec();
    for nb in nbrs {
        let err = extended_neighbors(surf, targ, nb, thresh, found, max, dist_type);
        if err != 0 {
            return err;
        }
    }
    0
}

/// `MRISunion(mris, mris2)` (mris_convert's): `NULL` with `ERROR: MRISunion() - VolGeom of both
/// surface are not the same!!!` when their volume geometries differ (`vg_isEqual`), or `ERROR:
/// MRISunion() - input surfaces are not in the same space!!!` when `useRealRAS` does (standard
/// output); else a new surface (`MRISalloc`) of both surfaces' vertices (positions only, through
/// `MRISsetXYZ`) and faces (the second's vertex numbers shifted by the first's count), with the
/// first's `useRealRAS` and volume geometry.
pub fn union(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    mris: &Mris,
    mris2: &Mris,
) -> Result<Option<Mris>, Fatal> {
    if vg_is_equal(g, &mris.vg, &mris2.vg) == 0 {
        let _ = writeln!(
            io.out,
            "ERROR: MRISunion() - VolGeom of both surface are not the same!!!"
        );
        return Ok(None);
    }
    if mris.useRealRAS != mris2.useRealRAS {
        let _ = writeln!(
            io.out,
            "ERROR:  MRISunion() - input surfaces are not in the same space!!!"
        );
        return Ok(None);
    }
    let mut m3 = alloc::alloc(
        g,
        io.out,
        io.err,
        sg,
        mris.nvertices + mris2.nvertices,
        mris.nfaces + mris2.nfaces,
    )?;
    m3.useRealRAS = mris.useRealRAS;
    m3.vg = mris.vg.clone();
    let file = metric_properties_cpp();
    let mut vno3 = 0i32;
    for (src, line) in [(mris, 13254), (mris2, 13261)] {
        for vno in 0..nverts(src) {
            let v = src.vertex(vno);
            coords::set_xyz(g, io, &mut m3, vno3, v.x, v.y, v.z, &file, line)?;
            vno3 += 1;
        }
    }
    let mut fno3 = 0usize;
    for (src, offset) in [(mris, 0), (mris2, mris.nvertices)] {
        for fno in 0..nfaces(src) {
            let v = src.face(fno).v;
            m3.face_mut(fno3).v = [v[0] + offset, v[1] + offset, v[2] + offset];
            fno3 += 1;
        }
    }
    Ok(Some(m3))
}

/// `MRISclone(mris_src)`: a new surface (`MRISalloc`) with the source's type, status, original
/// positions' status, neighbourhood sizes, hemisphere, centre, bounding box, curvature extremes,
/// total and original areas, radius, average vertex area, distance and its spread, transform
/// (shared in FreeSurfer: `free_transform` 0), poles and volume geometry; each vertex's rip flag,
/// position, normal, canonical position, curvature, face list, neighbour counts (`modVnum`,
/// `v2num`, `v3num`, `nsizeMax`, its clock, `nsizeCur` through `MRIS_setNsizeCur`), neighbour
/// list, distances and original distances (when the source has them), border, area and original
/// area; each face whole (its normal matrices copied). Every other vertex field is the new
/// surface's own (zero).
pub fn clone(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    src: &Mris,
) -> Result<Mris, Fatal> {
    let mut dst = alloc::alloc(g, io.out, io.err, sg, src.nvertices, src.nfaces)?;
    dst.r#type = src.r#type;
    dst.status = src.status;
    dst.origxyz_status = src.origxyz_status;
    dst.nsize = src.nsize;
    dst.max_nsize = src.max_nsize;
    dst.vtotalsMightBeTooBig = src.vtotalsMightBeTooBig;
    dst.nsizeMaxClock = src.nsizeMaxClock;
    dst.hemisphere = src.hemisphere;
    (dst.xctr, dst.yctr, dst.zctr) = (src.xctr, src.yctr, src.zctr);
    (dst.xlo, dst.ylo, dst.zlo) = (src.xlo, src.ylo, src.zlo);
    (dst.xhi, dst.yhi, dst.zhi) = (src.xhi, src.yhi, src.zhi);
    dst.min_curv = src.min_curv;
    dst.max_curv = src.max_curv;
    dst.total_area = src.total_area;
    dst.orig_area = src.orig_area;
    dst.radius = src.radius;
    dst.avg_vertex_area = src.avg_vertex_area;
    dst.avg_vertex_dist = src.avg_vertex_dist;
    dst.std_vertex_dist = src.std_vertex_dist;
    // FreeSurfer shares the source's transform; a copy of it here (`LTAcopy`)
    if let Some(l) = &src.lta {
        dst.lta = crate::lta::lta_copy(g, io.out, io.err, l, None)?.into_new();
    }
    dst.SRASToTalSRAS_ = src.SRASToTalSRAS_.clone();
    dst.TalSRASToSRAS_ = src.TalSRASToSRAS_.clone();
    dst.free_transform = 0;
    dst.v_frontal_pole = src.v_frontal_pole;
    dst.v_occipital_pole = src.v_occipital_pole;
    dst.v_temporal_pole = src.v_temporal_pole;
    for vno in 0..nverts(src) {
        let (vs, ts) = (src.vertex(vno), src.topology(vno));
        let vd = dst.vertex_mut(vno);
        vd.ripflag = vs.ripflag;
        (vd.x, vd.y, vd.z) = (vs.x, vs.y, vs.z);
        (vd.nx, vd.ny, vd.nz) = (vs.nx, vs.ny, vs.nz);
        (vd.cx, vd.cy, vd.cz) = (vs.cx, vs.cy, vs.cz);
        vd.curv = vs.curv;
        let num = usize::from(ts.num);
        let td = dst.topology_mut(vno);
        td.num = ts.num;
        if num > 0 {
            td.f = Some(ts.f.as_deref().unwrap_or(&[])[..num].to_vec());
            td.n = Some(ts.n.as_deref().unwrap_or(&[])[..num].to_vec());
        }
        mod_vnum(g, sg, io.err, &mut dst, vno, ts.vnum, true);
        let td = dst.topology_mut(vno);
        td.v2num = ts.v2num;
        td.v3num = ts.v3num;
        td.nsizeMax = ts.nsizeMax;
        td.nsizeMaxClock = ts.nsizeMaxClock;
        set_nsize_cur(g, io, &mut dst, vno, i32::from(ts.nsizeCur))?;
        let vsize = usize::try_from(vertex_vsize(src, vno)).unwrap_or(0);
        if vsize > 0 {
            dst.topology_mut(vno).v = Some(ts.v.as_deref().unwrap_or(&[])[..vsize].to_vec());
        }
        if let Some(d) = vs.dist.as_deref() {
            make_dist(&mut dst, vno);
            dst.vertex_mut(vno).dist.as_mut().expect("distances")[..vsize]
                .copy_from_slice(&d[..vsize]);
        }
        if let Some(d) = vs.dist_orig.as_deref() {
            make_dist_orig(&mut dst, vno);
            dst.vertex_mut(vno).dist_orig.as_mut().expect("distances")[..vsize]
                .copy_from_slice(&d[..vsize]);
        }
        let vd = dst.vertex_mut(vno);
        vd.border = vs.border;
        vd.area = vs.area;
        vd.origarea = vs.origarea;
    }
    for fno in 0..nfaces(src) {
        *dst.face_mut(fno) = src.face(fno).clone();
    }
    dst.vg = src.vg.clone();
    Ok(dst)
}

/// `MRISprintVertexStats(mris, vno, fp, which)`: nothing for a vertex below 0; else (standard
/// output) `vertex %d spacing for %s surface` (`orig`, `current`, `white` or `unknown`), each
/// neighbour's (the first `vnum`) `nbr %d = %d is ripped` or `\tvn %d: %d = %2.3f mm distant` (the
/// distance between the `which` positions, `MRISvertexCoord2XYZ_float`, in `float`), and `\tmean =
/// %2.3f` over the unripped ones (0/0 a NaN with none).
pub fn print_vertex_stats(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    vno: i32,
    which: i32,
) -> Result<i32, Fatal> {
    if vno < 0 {
        return Ok(NO_ERROR);
    }
    let vno = vno as usize;
    let (x0, y0, z0) = vertex_coord2xyz_float(g, io.err, mris.vertex(vno), which)?;
    let name = match which {
        ORIGINAL_VERTICES => "orig",
        CURRENT_VERTICES => "current",
        WHITE_VERTICES => "white",
        _ => "unknown",
    };
    let _ = writeln!(io.out, "vertex {vno} spacing for {name} surface");
    let vt = mris.topology(vno);
    let k = usize::try_from(vt.vnum).unwrap_or(0);
    let nbrs: Vec<i32> = vt.v.as_deref().unwrap_or(&[])[..k].to_vec();
    let (mut mn, mut num) = (0.0f64, 0i32);
    for (n, nb) in nbrs.into_iter().enumerate() {
        let vn = mris.vertex(nb as usize);
        if vn.ripflag != 0 {
            let _ = writeln!(io.out, "nbr {n} = {nb} is ripped");
            continue;
        }
        num += 1;
        let (x, y, z) = vertex_coord2xyz_float(g, io.err, vn, which)?;
        let (dx, dy, dz) = (sub(x, x0), sub(y, y0), sub(z, z0));
        let s = add(mul(dz, dz), add(mul(dx, dx), mul(dy, dy)));
        if s < 0.0 {
            g.errno = EDOM;
        }
        let d = f64::from(sqrt(s));
        let _ = writeln!(io.out, "\tvn {n}: {nb} = {} mm distant", fmt_f(d, 2, 3));
        mn = add64(d, mn);
    }
    let _ = writeln!(
        io.out,
        "\tmean = {}",
        fmt_f(div64(mn, f64::from(num)), 2, 3)
    );
    Ok(NO_ERROR)
}

/// `MRISprintVertexInfo(fp, surf, vertexno)`: into `fp`: `vertexno %d`, `xyz  %7.4f %7.4f %7.4f`,
/// `nxyz %7.4f %7.4f %7.4f`, `vertex_area %lf`, `neighbors %d %d` (`vnum`, `num`), and for each
/// face `n` (FreeSurfer's loop runs over the faces) `nbr %d  %5d %6.4lf %6.4lf %5d %6.4f`: `n`, the
/// `n`-th neighbour, the distance to it (`double`), its area, the `n`-th face and its area; then
/// (standard output) each of the surface's edges at the vertex `Edge %6d %d  %4d %4d %7.3lf
/// %7.3lf` (edge, end, its two vertices, length, angle). FS-380: a face count past the
/// neighbour list's room stops at its end. Returns 0.
pub fn print_vertex_info(
    g: &mut Globals,
    io: &mut Io,
    fp: &mut dyn Write,
    surf: &Mris,
    vertexno: usize,
) -> i32 {
    let v = surf.vertex(vertexno);
    let vt = surf.topology(vertexno);
    let f4 = |x: f32, w: usize| fmt_f(f64::from(x), w, 4);
    let _ = writeln!(fp, "vertexno {vertexno}");
    let _ = writeln!(fp, "xyz  {} {} {}", f4(v.x, 7), f4(v.y, 7), f4(v.z, 7));
    let _ = writeln!(fp, "nxyz {} {} {}", f4(v.nx, 7), f4(v.ny, 7), f4(v.nz, 7));
    let _ = writeln!(fp, "vertex_area {}", fmt_f(f64::from(v.area), 0, 6));
    let _ = writeln!(fp, "neighbors {} {}", vt.vnum, vt.num);
    let nbrs = vt.v.as_deref().unwrap_or(&[]);
    let faces = vt.f.as_deref().unwrap_or(&[]);
    for n in 0..usize::from(vt.num) {
        if n >= nbrs.len() {
            g.notices.add(
                &FS_380,
                format!("MRISprintVertexInfo: vertex {vertexno} has {} faces and room for {} neighbours: the lines stop there (FreeSurfer reads past the list)", vt.num, nbrs.len()),
            );
            break;
        }
        let vnno = nbrs[n];
        let vn = surf.vertex(vnno as usize);
        let dx = f64::from(sub(v.x, vn.x));
        let dy = f64::from(sub(v.y, vn.y));
        let dz = f64::from(sub(v.z, vn.z));
        let dist = sqrt64(add64(add64(mul64(dx, dx), mul64(dy, dy)), mul64(dz, dz)));
        let faceno = faces[n];
        let face = surf.face(faceno as usize);
        let _ = writeln!(
            fp,
            "nbr {n}  {vnno:5} {} {} {faceno:5} {}",
            fmt_f(dist, 6, 4),
            fmt_f(f64::from(vn.area), 6, 4),
            fmt_f(f64::from(face.area), 6, 4)
        );
    }
    if let Some(edges) = surf.edges.as_deref() {
        let ne = usize::try_from(surf.nedges).unwrap_or(0).min(edges.len());
        for (edgeno, e) in edges[..ne].iter().enumerate() {
            for evno in 0..2 {
                if e.vtxno[evno] == vertexno as i32 {
                    let _ = writeln!(
                        io.out,
                        "Edge {edgeno:6} {evno}  {:4} {:4} {} {}",
                        e.vtxno[0],
                        e.vtxno[1],
                        fmt_f(e.len, 7, 3),
                        fmt_f(e.angle, 7, 3)
                    );
                }
            }
        }
    }
    0
}
