// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surf's `utils/mrisurf_vals.cpp`: the marks, the per-vertex settings and clearings, the rip
//! flags and the face tests (queue item 3.4-03, taken early; relay 40 of track D):
//! `MRIScountMarked` (line 70), `MRISmarkedVertices` (88), `MRISsetAllMarks` (103),
//! `MRISinvertMarks` (136), `MRISnotMarked` (152), `MRISthresholdValIntoMarked` (158),
//! `MRISexpandMarked` (176), `MRISerodeMarked` (245), `MRIScloseMarked` (283), `MRISopenMarked`
//! (291), `MRISclearMark2s` (368), `MRISsetFlags` (418), `MRISclearFlags` (429), `mrisSetVal`
//! (443), `mrisSetValAndClearVal2` (450), `mrisClearGradient` (458), `MRISnegateValues` (512),
//! `MRISsetVals` (528), `MRISscaleVals` (544), `MRISmulVal` (560), `MRISsqrtVal` (577),
//! `MRISsetVal2` (623), `MRISsetValues` (639), `MRISclearFixedValFlags` (655), `MRISexportCurv`
//! (671), `MRISextractCurvatureVectorDouble` (688), `MRISextractCurvatureDoubleVector` (700),
//! `MRISclearGradient` (774), `mrisClearExtraGradient` (791), `mrisClearMomentum` (810),
//! `MRISabsVals` (826), `MRISabsCurvature` (841), `MRISripLabel` (1404), `MRISripNotLabel` (1417),
//! `MRISripMarked` (1448), `MRISripUnmarked` (1463), `MRISripZeroThicknessRegions` (1529),
//! `mrisMarkBadEdgeVertices` (1686), `mrisAnyVertexOfFaceMarked` (1745), `triangleMarked` (1759),
//! `findNonMarkedFace` (1766), `MRISmarkVerticesWithValOverThresh` (1857), `MRISclearD` (3435) and
//! `detectContrast` (3077). `MRISsetMarks`, `MRISclearMarks`, `MRIScountAllMarked` and
//! `MRISdilateMarked` are [`crate::discrete`]'s, `MRISclearAnnotations` [`crate::annot`]'s,
//! `MRISextractCurvatureVector` [`crate::metric`]'s, the copies [`crate::valcopy`]'s.
//!
//! The arithmetic is the official build's: `MRISnegateValues` flips the sign bit (`xorps`, a
//! NaN's too), `MRISabsVals` clears it; `detectContrast`'s sums and quotients in `double` with
//! SSE's NaNs ([`larmorx_freesurfer_core::sse`]). Defects (`larmorx_core::defects`):
//! - **FS-917** (out of bounds): ripping by a label (`MRISripLabel`, `MRISripNotLabel`) writes
//!   outside the surface's vertices for a label point that names no vertex (-1) or one beyond the
//!   surface's: both modes skip the point (noted).

use std::io::Write;

use larmorx_core::defects::FS_917;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_io::label::Label;

use crate::mris::{Mris, Vertex};
use crate::props8::is_face;
use crate::topology::vertex_neighbor;
use crate::valfilter::widen;

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// The first `vnum` neighbours of vertex `vno`.
fn neighbours(mris: &Mris, vno: usize) -> Vec<i32> {
    let vt = mris.topology(vno);
    let n = usize::try_from(vt.vnum).unwrap_or(0);
    match vt.v.as_deref() {
        Some(v) if n > 0 => v[..n].to_vec(),
        _ => Vec::new(),
    }
}

/// Every vertex whose rip flag is 0 changed by `f`.
fn unripped(mris: &mut Mris, f: impl Fn(&mut Vertex)) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            f(v);
        }
    }
    NO_ERROR
}

/// Every vertex changed by `f`.
fn every(mris: &mut Mris, f: impl Fn(&mut Vertex)) -> i32 {
    for vno in 0..nverts(mris) {
        f(mris.vertex_mut(vno));
    }
    NO_ERROR
}

/// `MRIScountMarked(mris, mark_threshold)`: the unripped vertices whose `marked` is at least
/// `mark_threshold`.
pub fn count_marked(mris: &Mris, mark_threshold: i32) -> i32 {
    (0..nverts(mris))
        .filter(|&vno| {
            let v = mris.vertex(vno);
            v.ripflag == 0 && i32::from(v.marked) >= mark_threshold
        })
        .count() as i32
}

/// `MRISmarkedVertices(mris)`: the unripped vertices whose `marked` is above 0.
pub fn marked_vertices(mris: &Mris) -> i32 {
    count_marked(mris, 1)
}

/// `MRISsetAllMarks(mris, mark)`: every vertex's `marked` (ripped ones too; the `int` into the
/// vertex's `short`). `NO_ERROR`.
pub fn set_all_marks(mris: &mut Mris, mark: i32) -> i32 {
    every(mris, |v| v.marked = mark as i16)
}

/// `MRISinvertMarks(mris)`: every unripped vertex's `marked` becomes 1 where it was 0, else 0.
/// `NO_ERROR`.
pub fn invert_marks(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.marked = i16::from(v.marked == 0))
}

/// `MRISnotMarked(mris)`: [`invert_marks`].
pub fn not_marked(mris: &mut Mris) -> i32 {
    invert_marks(mris)
}

/// `MRISthresholdValIntoMarked(mris, thresh)`: every unripped vertex's `marked` 1 where its `val`
/// is at least `thresh`, else 0 (NaN: 0). `NO_ERROR`.
pub fn threshold_val_into_marked(mris: &mut Mris, thresh: f32) -> i32 {
    unripped(mris, |v| v.marked = i16::from(v.val >= thresh))
}

/// `MRISmarkVerticesWithValOverThresh(mris, thresh)`: every unripped vertex's `marked` 1 where its
/// `val` is above `thresh`, else 0. `NO_ERROR`.
pub fn mark_vertices_with_val_over_thresh(mris: &mut Mris, thresh: f32) -> i32 {
    unripped(mris, |v| v.marked = i16::from(v.val > thresh))
}

/// `MRISexpandMarked(mris)`: in vertex order (ripped ones too), each vertex whose `marked` is 1
/// gives each of its 1-hop neighbours whose `marked` is 0 the mark 2 (so a neighbour marked later
/// in the pass does not spread); then every vertex marked 2 (those it had before too) becomes 1.
/// `NO_ERROR`.
pub fn expand_marked(mris: &mut Mris) -> i32 {
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).marked != 1 {
            continue;
        }
        for n in neighbours(mris, vno) {
            let vn = mris.vertex_mut(n as usize);
            if vn.marked == 0 {
                vn.marked = 2;
            }
        }
    }
    every(mris, |v| {
        if v.marked == 2 {
            v.marked = 1;
        }
    })
}

/// `MRISerodeMarked(mris, num)`: `num` times, each unripped vertex's `tx` becomes the smallest
/// `marked` of it and its 1-hop neighbours (ripped ones included, as a `float`), then its
/// `marked` that `tx`. `NO_ERROR`.
pub fn erode_marked(mris: &mut Mris, num: i32) -> i32 {
    for _ in 0..num {
        unripped(mris, |v| v.tx = 0.0);
        for vno in 0..nverts(mris) {
            if mris.vertex(vno).ripflag != 0 {
                continue;
            }
            let mut mn = i32::from(mris.vertex(vno).marked);
            for n in neighbours(mris, vno) {
                let m = i32::from(mris.vertex(n as usize).marked);
                // MIN(vn->marked, mn): `a < b ? a : b`
                mn = if m < mn { m } else { mn };
            }
            mris.vertex_mut(vno).tx = mn as f32;
        }
        unripped(mris, |v| v.marked = (v.tx as i32) as i16);
    }
    NO_ERROR
}

/// `MRIScloseMarked(mris, order)`: [`crate::discrete::dilate_marked`] then [`erode_marked`],
/// `order` times each. `NO_ERROR`.
pub fn close_marked(mris: &mut Mris, order: i32) -> i32 {
    crate::discrete::dilate_marked(mris, order);
    erode_marked(mris, order);
    NO_ERROR
}

/// `MRISopenMarked(mris, order)`: [`erode_marked`] then [`crate::discrete::dilate_marked`].
/// `NO_ERROR`.
pub fn open_marked(mris: &mut Mris, order: i32) -> i32 {
    erode_marked(mris, order);
    crate::discrete::dilate_marked(mris, order);
    NO_ERROR
}

/// `MRISclearMark2s(mris)`: every unripped vertex's `marked2` 0. `NO_ERROR`.
pub fn clear_mark2s(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.marked2 = 0)
}

/// `MRISsetFlags(mris, flags)`: `flags` set in every vertex's `flags` (ripped ones too).
pub fn set_flags(mris: &mut Mris, flags: i32) -> i32 {
    every(mris, |v| v.flags |= flags as u32)
}

/// `MRISclearFlags(mris, flags)`: `flags` cleared in every vertex's `flags`.
pub fn clear_flags(mris: &mut Mris, flags: i32) -> i32 {
    every(mris, |v| v.flags &= !(flags as u32))
}

/// `mrisSetVal(mris, val)`: every vertex's `val` (ripped ones too).
pub fn set_val_all(mris: &mut Mris, val: f32) {
    every(mris, |v| v.val = val);
}

/// `mrisSetValAndClearVal2(mris, val)`: every vertex's `val`, and its `val2` 0.
pub fn set_val_and_clear_val2(mris: &mut Mris, val: f32) {
    every(mris, |v| {
        v.val = val;
        v.val2 = 0.0;
    });
}

/// `MRISclearGradient(mris)` and `mrisClearGradient(mris)` (the two are the same): every unripped
/// vertex's `dx`, `dy`, `dz` 0. `NO_ERROR`.
pub fn clear_gradient(mris: &mut Mris) -> i32 {
    unripped(mris, |v| {
        v.dx = 0.0;
        v.dy = 0.0;
        v.dz = 0.0;
    })
}

/// `mrisClearExtraGradient(mris)`: with the extra gradients (`dx2`), every unripped vertex's
/// entries of `dx2`, `dy2`, `dz2` 0. `NO_ERROR`.
pub fn clear_extra_gradient(mris: &mut Mris) -> i32 {
    if mris.dx2.is_none() {
        return NO_ERROR;
    }
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        for a in [&mut mris.dz2, &mut mris.dy2, &mut mris.dx2]
            .into_iter()
            .flatten()
        {
            if let Some(x) = a.get_mut(vno) {
                *x = 0.0;
            }
        }
    }
    NO_ERROR
}

/// `mrisClearMomentum(mris)`: every unripped vertex's `odx`, `ody`, `odz` 0. `NO_ERROR`.
pub fn clear_momentum(mris: &mut Mris) -> i32 {
    unripped(mris, |v| {
        v.odx = 0.0;
        v.ody = 0.0;
        v.odz = 0.0;
    })
}

/// `MRISnegateValues(mris)`: every unripped vertex's `val` with its sign bit flipped (`val *=
/// -1.0`, which the official build compiles to `xorps`: a NaN's sign flips too). `NO_ERROR`.
pub fn negate_values(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.val = -v.val)
}

/// `MRISsetVals(mris, val)` and `MRISsetValues(mris, val)`: every unripped vertex's `val`.
/// `NO_ERROR`.
pub fn set_vals(mris: &mut Mris, val: f32) -> i32 {
    unripped(mris, |v| v.val = val)
}

/// `MRISscaleVals(mris, scale)` and `MRISmulVal(mris, mul)`: every unripped vertex's `val` times
/// the factor (`float`). `NO_ERROR`.
pub fn scale_vals(mris: &mut Mris, scale: f32) -> i32 {
    unripped(mris, |v| v.val = sse::mul(v.val, scale))
}

/// `MRISsqrtVal(mris)`: every unripped vertex's `val` above 0 replaced by its square root.
/// `NO_ERROR`.
pub fn sqrt_val(mris: &mut Mris) -> i32 {
    unripped(mris, |v| {
        if v.val > 0.0 {
            v.val = v.val.sqrt();
        }
    })
}

/// `MRISsetVal2(mris, val)`: every unripped vertex's `val2`. `NO_ERROR`.
pub fn set_val2(mris: &mut Mris, val: f32) -> i32 {
    unripped(mris, |v| v.val2 = val)
}

/// `MRISclearFixedValFlags(mris)`: every unripped vertex's `fixedval` 0. `NO_ERROR`.
pub fn clear_fixed_val_flags(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.fixedval = 0)
}

/// `MRISextractCurvatureVectorDouble(mris, curvs, offset)`: each vertex's `curv` as a `double`
/// at `offset + vno` (the vector grown with zeros when shorter). `NO_ERROR`.
pub fn extract_curvature_vector_double(mris: &Mris, curvs: &mut Vec<f64>, offset: usize) -> i32 {
    let nv = nverts(mris);
    if curvs.len() < offset + nv {
        curvs.resize(offset + nv, 0.0);
    }
    for vno in 0..nv {
        curvs[vno + offset] = f64::from(mris.vertex(vno).curv);
    }
    NO_ERROR
}

/// `MRISextractCurvatureDoubleVector(mris, curvs)`: each vertex's `curv` as a `double`.
pub fn extract_curvature_double_vector(mris: &Mris) -> Vec<f64> {
    (0..nverts(mris))
        .map(|vno| f64::from(mris.vertex(vno).curv))
        .collect()
}

/// `MRISabsVals(mris)`: every unripped vertex's `val` with its sign bit cleared. `NO_ERROR`.
pub fn abs_vals(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.val = v.val.abs())
}

/// `MRISabsCurvature(mris)`: every unripped vertex's `curv` with its sign bit cleared.
pub fn abs_curvature(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.curv = v.curv.abs())
}

/// `MRISclearD(mris)`: every vertex's `d` 0 (ripped ones too).
pub fn clear_d(mris: &mut Mris) {
    every(mris, |v| v.d = 0.0);
}

/// The vertex a label point names, when it is one of the surface's; FS-917 noted for one that is
/// not (FreeSurfer writes outside the surface's vertices there).
fn label_vertex(g: &mut Globals, mris: &Mris, vno: i32, what: &str) -> Option<usize> {
    if (0..mris.nvertices).contains(&vno) {
        return Some(vno as usize);
    }
    g.notices.add(
        &FS_917,
        format!(
            "{what}: label point at vertex {vno} of a surface of {} vertices skipped (FreeSurfer writes outside the vertices)",
            mris.nvertices
        ),
    );
    None
}

fn npoints(area: &Label) -> usize {
    usize::try_from(area.n_points)
        .unwrap_or(0)
        .min(area.lv.len())
}

/// `MRISripLabel(mris, area)`: each point's vertex ripped (deleted points too). FS-917: a point
/// naming no vertex of the surface is skipped. `NO_ERROR`.
pub fn rip_label(g: &mut Globals, mris: &mut Mris, area: &Label) -> i32 {
    for i in 0..npoints(area) {
        if let Some(vno) = label_vertex(g, mris, area.lv[i].vno, "MRISripLabel") {
            mris.vertex_mut(vno).ripflag = 1;
        }
    }
    NO_ERROR
}

/// `MRISripNotLabel(mris, area)`: each point's vertex marked 1; every vertex then not marked
/// ripped (ripped ones again; a vertex marked before stays as it is); each point's vertex's mark
/// 0 again; `MRISripNotLabel() ripped <n>/<nvertices> vertices (<k> unripped)` on standard
/// output (`n` the vertices ripped by it, `k` the marked ones). FS-917: a point naming no vertex
/// of the surface is skipped. `NO_ERROR`.
pub fn rip_not_label(g: &mut Globals, out: &mut dyn Write, mris: &mut Mris, area: &Label) -> i32 {
    for i in 0..npoints(area) {
        if let Some(vno) = label_vertex(g, mris, area.lv[i].vno, "MRISripNotLabel") {
            mris.vertex_mut(vno).marked = 1;
        }
    }
    let (mut nripped, mut nok) = (0, 0);
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.marked != 0 {
            nok += 1;
            continue;
        }
        v.ripflag = 1;
        nripped += 1;
    }
    for i in 0..npoints(area) {
        if let Some(vno) = label_vertex(g, mris, area.lv[i].vno, "MRISripNotLabel") {
            mris.vertex_mut(vno).marked = 0;
        }
    }
    let _ = writeln!(
        out,
        "MRISripNotLabel() ripped {nripped}/{} vertices ({nok} unripped)",
        mris.nvertices
    );
    NO_ERROR
}

/// `MRISripMarked(mris)`: every vertex whose `marked` is not 0 ripped (none unripped). `NO_ERROR`.
pub fn rip_marked(mris: &mut Mris) -> i32 {
    every(mris, |v| {
        if v.marked != 0 {
            v.ripflag = 1;
        }
    })
}

/// `MRISripUnmarked(mris)`: every vertex whose `marked` is 0 ripped. `NO_ERROR`.
pub fn rip_unmarked(mris: &mut Mris) -> i32 {
    every(mris, |v| {
        if v.marked == 0 {
            v.ripflag = 1;
        }
    })
}

/// `FZERO(f)` of a `float` difference: `|f| < FLT_EPSILON` (NaN: no).
fn fzero(f: f32) -> bool {
    f.abs() < f32::EPSILON
}

/// `MRISripZeroThicknessRegions(mris)`: every unripped vertex whose white and pial coordinates
/// differ by less than `FLT_EPSILON` along each axis ripped. `NO_ERROR`.
pub fn rip_zero_thickness_regions(mris: &mut Mris) -> i32 {
    unripped(mris, |v| {
        if fzero(v.whitex - v.pialx) && fzero(v.whitey - v.pialy) && fzero(v.whitez - v.pialz) {
            v.ripflag = 1;
        }
    })
}

/// `mrisMarkBadEdgeVertices(mris, mark)`: for each unripped vertex, the first 1-hop neighbour
/// numbered at least as high (itself included when it is its own neighbour) whose edge with it
/// lies in a number of faces other than 2 (its other 1-hop neighbours that are neighbours of that
/// one and make a face with the two): both vertices marked `mark`; the vertex's other edges are
/// not looked at. Returns twice the vertices that found one.
pub fn mark_bad_edge_vertices(mris: &mut Mris, mark: i32) -> i32 {
    let mut nmarked = 0;
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let nb = neighbours(mris, vno);
        for &vno2 in &nb {
            if vno2 < vno as i32 {
                continue;
            }
            let mut nfaces = 0;
            for &m in &nb {
                if m == vno2 {
                    continue;
                }
                if vertex_neighbor(mris, vno2 as usize, m) != 0
                    && is_face(mris, vno as i32, vno2, m)
                {
                    nfaces += 1;
                }
            }
            if nfaces != 2 {
                mris.vertex_mut(vno).marked = mark as i16;
                mris.vertex_mut(vno2 as usize).marked = mark as i16;
                nmarked += 2;
                break;
            }
        }
    }
    nmarked
}

/// `mrisAnyVertexOfFaceMarked(mris, fno)`: whether a vertex of face `fno` has a `marked` other
/// than 0 (its three marks or'ed).
pub fn any_vertex_of_face_marked(mris: &Mris, fno: usize) -> bool {
    let f = mris.face(fno);
    let mut marked = 0i32;
    for &v in &f.v {
        marked |= i32::from(mris.vertex(v as usize).marked);
    }
    marked != 0
}

/// `triangleMarked(mris, fno)`: [`any_vertex_of_face_marked`].
pub fn triangle_marked(mris: &Mris, fno: usize) -> bool {
    any_vertex_of_face_marked(mris, fno)
}

/// `findNonMarkedFace(mris, vno, vn1)`: the first face of vertex `vno` (its face list) whose
/// `marked` is 0 and that has `vn1` as a vertex; -1 when none. A `vno` outside the surface, or
/// `vn1` equal to `vno`, is a failed assertion (`ASSERTION FAILED:
/// <official source>/utils/mrisurf_vals.cpp:1768 (0 <= vno) && (vno < mris->nvertices)`, or
/// `:1769 vno != vn1`) that ends the program (FS-03).
pub fn find_non_marked_face(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &Mris,
    vno: i32,
    vn1: i32,
) -> Result<i32, Fatal> {
    let file = format!("{}/utils/mrisurf_vals.cpp", diag::BUILD_ROOT);
    if !((0 <= vno) && (vno < mris.nvertices)) {
        return Err(diag::assert_failed(
            g,
            out,
            err,
            &file,
            1768,
            "(0 <= vno) && (vno < mris->nvertices)",
        ));
    }
    if vno == vn1 {
        return Err(diag::assert_failed(g, out, err, &file, 1769, "vno != vn1"));
    }
    let vt = mris.topology(vno as usize);
    let num = usize::from(vt.num);
    for &fn_ in &vt.f.as_deref().unwrap_or(&[])[..num] {
        let f = mris.face(fn_ as usize);
        if f.marked != 0 {
            continue;
        }
        if f.v.contains(&vn1) {
            return Ok(fn_);
        }
    }
    Ok(-1)
}

/// `detectContrast(mris)`: `Detecting contrast direction:` on standard error, then the means of
/// the unripped vertices' `val2` (white matter) and `val2bak` (grey matter), summed in `double`
/// in vertex order: ` right direction [ GM (<gm>) < WM (<wm>) ]` and 1 when the white mean is
/// above the grey one, else ` inverted [ WM (<wm>) < GM (<gm>) ]` and -1 (`%3.2f` each; no
/// unripped vertex: `0 / 0`, SSE's NaN, printed `-nan`).
pub fn detect_contrast(err: &mut dyn Write, mris: &Mris) -> i32 {
    let _ = write!(err, "Detecting contrast direction:");
    let (mut wm, mut gm, mut nv) = (0.0f64, 0.0f64, 0i32);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        nv += 1;
        wm = sse::add64(wm, widen(v.val2));
        gm = sse::add64(gm, widen(v.val2bak));
    }
    wm = sse::div64(wm, f64::from(nv));
    gm = sse::div64(gm, f64::from(nv));
    if wm > gm {
        let _ = writeln!(
            err,
            " right direction [ GM ({}) < WM ({}) ]",
            fmt_f(gm, 3, 2),
            fmt_f(wm, 3, 2)
        );
        1
    } else {
        let _ = writeln!(
            err,
            " inverted [ WM ({}) < GM ({}) ]",
            fmt_f(wm, 3, 2),
            fmt_f(gm, 3, 2)
        );
        -1
    }
}
