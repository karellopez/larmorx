// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surf's `utils/mrisurf_vals.cpp`, the functions of one program each (queue item 3.4-03, taken
//! early; relay 41 of track D): `MRISdilateRipped` (line 1547) and `MRISerodeRipped` (1582) with
//! `utils/mrisurf_topology.cpp`'s `MRISsetRipInFacesWithRippedVertices` (2665), the vector
//! registration's value setters (`MRISsetCurvaturesToValues` 1790, `MRISsetCurvaturesToOrigValues`
//! 1803, `MRISsetOrigValuesToCurvatures` 1816, `MRISsetOrigValuesToValues` 1829,
//! `MRISsetValuesToCurvatures` 1843, on `include/mrisurf.h`'s `VALS_VP`), the mode filter of the
//! annotations (`MRISmodeFilterAnnotations` 2901, `mris_ca_label`) and the dilation of a vertex to
//! a target sum (`MRISdilateVertexToSum` 4150, `mri_binarize --dilate-vertex-sum`).
//!
//! `MRISmodeFilterAnnotations` counts, for each unripped vertex, its neighbours in use (`vtotal`,
//! ripped ones too) and itself by the annotation table's index of their annotations (those the
//! table does not hold skipped), in a table of `max + 1` entries (`max` the largest index over
//! every vertex, at least 0); the vertex's own index first, then indices 1 to `max` in order, the
//! larger count winning (ties to the earlier), give the vertex's new annotation (0 when its own
//! is not in the table and no neighbour's wins); here only the indices met are counted (the same
//! answer), so a large index costs no memory. The annotation table is the program's
//! ([`SurfGlobals::atable`], read from `$FREESURFER_HOME/Simple_surface_labels2009.txt` on first
//! use when the program has none, `annotation.cpp`'s rule); the surface's colour table sets it
//! when the program has none after the largest index is taken (`set_atable_from_ctable`).
//! `MRISdilateVertexToSum`'s sums are the official build's: each measure value added before the
//! running sum (`val + sum`, its NaN passed on), each area after it.
//!
//! Defects (`larmorx_core::defects`):
//! - **FS-978** (wrong result): `MRISmodeFilterAnnotations` takes its largest index before it sets
//!   the program's table from the surface's colour table, and that table is set only when the
//!   program has none: a surface whose annotations come from its own colour table is filtered
//!   through another table (the default file's, or one the program used before), and when the
//!   program has no table and the default file cannot be read the filter changes nothing but
//!   annotations it does not know (FS-979). larmorx indexes through the surface's own colour table
//!   when it has one (set first; the program's table set from it as FreeSurfer sets it); `reproduce`
//!   FreeSurfer's order.
//! - **FS-979** (out of bounds): in FS-978's second case FreeSurfer counts every index above 0
//!   outside its one-entry tables (memory corrupted; the counts it reads there decide nothing, as
//!   no index above 0 is compared): `reproduce` gives the result its defined part decides (each
//!   vertex keeps an annotation the table holds, the others become 0), noted.
//! - **FS-980** (out of bounds): the value setters read or write outside a vertex's value arrays for
//!   a field number outside them: both modes skip that vertex (noted).
//! - **FS-981** (crash): the value setters crash the program for an unripped vertex without value
//!   arrays (`vp` NULL): `reproduce` ends the program so (signal 11), the default skips the vertex
//!   (noted).
//! - **FS-982** (wrong result): `MRISdilateVertexToSum` counts the vertices of every pass together
//!   (its count is never reset), so it reports `not possible to achieve target sum` once the
//!   passes together have counted as many vertices as the surface has, though the disk of the
//!   last pass holds fewer; and when the vertex's part of the surface is used up before the target
//!   is reached it goes on with a pass per vertex counted. larmorx counts each pass's own vertices
//!   and stops at the first pass that adds none (the same message); the hop lines keep
//!   FreeSurfer's running count. `reproduce` FreeSurfer's.
//! - **FS-983** (crash): `MRISdilateVertexToSum` with a target no pass is needed for (not above 0
//!   with a measure, not a number) builds its mask from hop lists of 0 hops: the program crashes
//!   (FS-530). `reproduce` ends the program so; the default reports `ERROR:
//!   MRISdilateVertexToSum(): targetSum = <g>, must be > 0` (with ` with area` when the areas are
//!   summed) on standard output and returns no mask.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use larmorx_core::defects::{FS_978, FS_979, FS_980, FS_981, FS_982, FS_983, FS_984, FS_985};
use larmorx_freesurfer_core::cstd::{fmt_f, format_double};
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NOMEMORY, Fatal, NO_ERROR};
use larmorx_freesurfer_core::random::random_number;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_io::annotation::{self, AnnotationTables};
use larmorx_freesurfer_io::colortab;
use larmorx_freesurfer_io::label::{Label, label_alloc};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::aaa::{Opaque, SurfGlobals};
use crate::coords::Io;
use crate::mp::compute_metric_properties;
use crate::mris::{Mris, Vertex};
use crate::props9::set_surf_hop_list;

/// `ENOMEM`.
const ENOMEM: i32 = 12;

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// The first `n` neighbours of vertex `vno` (`vnum` or `vtotal`).
fn nbrs(mris: &Mris, vno: usize, n: i16) -> Vec<i32> {
    let vt = mris.topology(vno);
    let n = usize::try_from(n).unwrap_or(0);
    match vt.v.as_deref() {
        Some(v) if n > 0 => v[..n.min(v.len())].to_vec(),
        _ => Vec::new(),
    }
}

// ---- the rip flags ---------------------------------------------------------------------------

/// `MRISsetRipInFacesWithRippedVertices(mris)`: every face ripped when one of its vertices is,
/// else not; every vertex a border vertex when it is a vertex of a ripped face, else not.
pub fn set_rip_in_faces_with_ripped_vertices(mris: &mut Mris) {
    let nf = usize::try_from(mris.nfaces).unwrap_or(0);
    for k in 0..nf {
        mris.face_mut(k).ripflag = 0;
    }
    for k in 0..nf {
        let v = mris.face(k).v;
        if v.iter().any(|&x| mris.vertex(x as usize).ripflag != 0) {
            mris.face_mut(k).ripflag = 1;
        }
    }
    for k in 0..nverts(mris) {
        mris.vertex_mut(k).border = 0;
    }
    for k in 0..nf {
        if mris.face(k).ripflag != 0 {
            let v = mris.face(k).v;
            for x in v {
                mris.vertex_mut(x as usize).border = 1;
            }
        }
    }
}

/// `MRISdilateRipped(mris, ndil)`: `ndil` times, every unripped 1-hop neighbour of a ripped vertex
/// ripped (flag 1; the others keep theirs, through `tx`); then
/// [`set_rip_in_faces_with_ripped_vertices`]. `NO_ERROR`.
pub fn dilate_ripped(mris: &mut Mris, ndil: i32) -> i32 {
    for _ in 0..ndil.max(0) {
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            v.tx = f32::from(v.ripflag);
        }
        for vno in 0..nverts(mris) {
            if mris.vertex(vno).ripflag == 0 {
                continue;
            }
            for n in nbrs(mris, vno, mris.topology(vno).vnum) {
                let vn = mris.vertex_mut(n as usize);
                if vn.ripflag == 0 {
                    vn.tx = 1.0;
                }
            }
        }
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            v.ripflag = v.tx as i32 as i8;
        }
    }
    set_rip_in_faces_with_ripped_vertices(mris);
    NO_ERROR
}

/// `MRISerodeRipped(mris, ndil)`: `ndil` times, every vertex's flag the smallest of its own and
/// its 1-hop neighbours' (through `tx`); then [`set_rip_in_faces_with_ripped_vertices`].
/// `NO_ERROR`.
pub fn erode_ripped(mris: &mut Mris, ndil: i32) -> i32 {
    for _ in 0..ndil.max(0) {
        for vno in 0..nverts(mris) {
            mris.vertex_mut(vno).tx = 0.0;
        }
        for vno in 0..nverts(mris) {
            let mut mn = i32::from(mris.vertex(vno).ripflag);
            for n in nbrs(mris, vno, mris.topology(vno).vnum) {
                mn = mn.min(i32::from(mris.vertex(n as usize).ripflag));
            }
            mris.vertex_mut(vno).tx = mn as f32;
        }
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            v.ripflag = v.tx as i32 as i8;
        }
    }
    set_rip_in_faces_with_ripped_vertices(mris);
    NO_ERROR
}

// ---- the vector registration's values --------------------------------------------------------

/// `VALS_VP` (`include/mrisurf.h` 565-576): a vertex's fields for the vector registration, hung on
/// its `vp`: the values in use (`vals`, for instance blurred) and the original ones
/// (`orig_vals`), `nvals` each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValsVp {
    pub vals: Vec<f32>,
    pub orig_vals: Vec<f32>,
    pub nvals: i32,
}

/// `v->vp = vp`: the values hung on the vertex (shared by its copies, as C shares the pointer).
pub fn set_vals_vp(v: &mut Vertex, vp: ValsVp) {
    v.vp = Opaque(Some(Arc::new(Mutex::new(vp))));
}

/// The values hung on the vertex, if they are a [`ValsVp`].
pub fn vals_vp(v: &Vertex) -> Option<&Mutex<ValsVp>> {
    v.vp.0.as_ref()?.downcast_ref::<Mutex<ValsVp>>()
}

/// Which field of a [`ValsVp`] a setter reads or writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Curv,
    Vals,
    Orig,
}

/// A setter: every unripped vertex's `to` from its `from` at field `fno`. FS-981: a vertex
/// without values (`vp` NULL, or not a [`ValsVp`]) crashes FreeSurfer: `reproduce` ends the
/// program so, the default skips it; FS-980: a field outside the arrays is skipped.
fn set_side(
    g: &mut Globals,
    mris: &mut Mris,
    fno: i32,
    from: Side,
    to: Side,
    who: &str,
) -> Result<(), Fatal> {
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let curv = v.curv;
        let Some(m) = vals_vp(v) else {
            g.notices.add(
                &FS_981,
                format!("{who}: vertex {vno} has no value arrays (FreeSurfer crashes)"),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
            continue;
        };
        let mut vp = m.lock().unwrap_or_else(|e| e.into_inner());
        let ok = |a: &[f32]| usize::try_from(fno).is_ok_and(|i| i < a.len());
        let fits = match (from, to) {
            (Side::Curv, Side::Vals) | (Side::Vals, Side::Curv) => ok(&vp.vals),
            (Side::Curv, Side::Orig) | (Side::Orig, Side::Curv) => ok(&vp.orig_vals),
            _ => ok(&vp.vals) && ok(&vp.orig_vals),
        };
        if !fits {
            g.notices.add(
                &FS_980,
                format!(
                    "{who}: field {fno} outside vertex {vno}'s {} values (FreeSurfer reads or writes outside them): skipped",
                    vp.vals.len().min(vp.orig_vals.len())
                ),
            );
            continue;
        }
        let i = fno as usize;
        let x = match from {
            Side::Curv => curv,
            Side::Vals => vp.vals[i],
            Side::Orig => vp.orig_vals[i],
        };
        match to {
            Side::Vals => vp.vals[i] = x,
            Side::Orig => vp.orig_vals[i] = x,
            Side::Curv => {
                drop(vp);
                mris.vertex_mut(vno).curv = x;
            }
        }
    }
    Ok(())
}

/// `MRISsetCurvaturesToValues(mris, fno)`: every unripped vertex's `vals[fno]` its `curv`.
pub fn set_curvatures_to_values(g: &mut Globals, mris: &mut Mris, fno: i32) -> Result<(), Fatal> {
    set_side(
        g,
        mris,
        fno,
        Side::Curv,
        Side::Vals,
        "MRISsetCurvaturesToValues",
    )
}

/// `MRISsetCurvaturesToOrigValues(mris, fno)`: every unripped vertex's `orig_vals[fno]` its
/// `curv`.
pub fn set_curvatures_to_orig_values(
    g: &mut Globals,
    mris: &mut Mris,
    fno: i32,
) -> Result<(), Fatal> {
    set_side(
        g,
        mris,
        fno,
        Side::Curv,
        Side::Orig,
        "MRISsetCurvaturesToOrigValues",
    )
}

/// `MRISsetOrigValuesToCurvatures(mris, fno)`: every unripped vertex's `curv` its
/// `orig_vals[fno]`.
pub fn set_orig_values_to_curvatures(
    g: &mut Globals,
    mris: &mut Mris,
    fno: i32,
) -> Result<(), Fatal> {
    set_side(
        g,
        mris,
        fno,
        Side::Orig,
        Side::Curv,
        "MRISsetOrigValuesToCurvatures",
    )
}

/// `MRISsetOrigValuesToValues(mris, fno)`: every unripped vertex's `vals[fno]` its
/// `orig_vals[fno]`.
pub fn set_orig_values_to_values(g: &mut Globals, mris: &mut Mris, fno: i32) -> Result<(), Fatal> {
    set_side(
        g,
        mris,
        fno,
        Side::Orig,
        Side::Vals,
        "MRISsetOrigValuesToValues",
    )
}

/// `MRISsetValuesToCurvatures(mris, fno)`: every unripped vertex's `curv` its `vals[fno]`.
pub fn set_values_to_curvatures(g: &mut Globals, mris: &mut Mris, fno: i32) -> Result<(), Fatal> {
    set_side(
        g,
        mris,
        fno,
        Side::Vals,
        Side::Curv,
        "MRISsetValuesToCurvatures",
    )
}

// ---- the mode filter of the annotations ------------------------------------------------------

/// The index of the first entry of `annotation` in a table built here (no file read), or -1.
fn own_index(t: &AnnotationTables, annotation: i32) -> i32 {
    let n = usize::try_from(t.num_entries)
        .unwrap_or(0)
        .min(t.atable.len());
    t.atable[..n]
        .iter()
        .find(|e| e.annotation == annotation)
        .map_or(-1, |e| e.index)
}

/// A table's `(index, annotation)` pairs, the part of it the mode filter reads.
fn pairs(t: &AnnotationTables) -> Vec<(i32, i32)> {
    let n = usize::try_from(t.num_entries)
        .unwrap_or(0)
        .min(t.atable.len());
    t.atable[..n]
        .iter()
        .map(|e| (e.index, e.annotation))
        .collect()
}

/// `MRISmodeFilterAnnotations(mris, niter)`: the largest annotation index over every vertex (at
/// least 0; `calloc`'s failure for a table of a negative size: `Cannot allocate memory` and
/// `MRISmodeFilterVals: could not allocate histo array of <n> ints`, `ErrorExit`); the program's
/// table set from the surface's colour table when it has one (`set_atable_from_ctable`); up to
/// `niter` passes: each unripped vertex given in `undefval` the annotation its neighbourhood's
/// counts choose, then each unripped vertex given its `undefval` (those that changed counted); a
/// pass without change ends them; `<k> filter iterations complete (<niter> requested, <changed>
/// changed)` on standard output (`k` the pass that changed nothing, or `niter`). `NO_ERROR`.
/// FS-978, FS-979.
pub fn mode_filter_annotations(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    niter: i32,
) -> Result<i32, Fatal> {
    let nv = nverts(mris);
    // FS-978: the surface's own colour table, set first (`reproduce`: after the largest index)
    let mut own: Option<AnnotationTables> = None;
    if !g.notices.reproduce()
        && let Some(ct) = mris.ct.as_ref()
    {
        let mut t = AnnotationTables::default();
        annotation::set_atable_from_ctable(g, &mut Vec::new(), &mut Vec::new(), &mut t, Some(ct));
        if t.num_entries > 0 {
            let empty = sg.atable.num_entries <= 0;
            if (empty && nv > 0) || (!empty && pairs(&t) != pairs(&sg.atable)) {
                g.notices.add(
                    &FS_978,
                    if empty {
                        "MRISmodeFilterAnnotations: the surface's annotations indexed through its own colour table (FreeSurfer takes the largest index through the default table file before it sets the table from that colour table)"
                    } else {
                        "MRISmodeFilterAnnotations: the surface's annotations indexed through its own colour table (FreeSurfer keeps the table the program used before)"
                    },
                );
            }
            annotation::set_atable_from_ctable(g, io.out, io.err, &mut sg.atable, Some(ct));
            own = Some(t);
        }
    }
    let index_of = |g: &mut Globals, sg: &mut SurfGlobals, io: &mut Io, a: i32| -> i32 {
        match &own {
            Some(t) => own_index(t, a),
            None => {
                annotation::annotation_to_index(g, io.env, io.out, io.err, dir, &mut sg.atable, a)
            }
        }
    };
    let mut max_index = 0i32;
    for vno in 0..nv {
        let a = mris.vertex(vno).annotation;
        let index = index_of(g, sg, io, a);
        if index > max_index {
            max_index = index;
        }
    }
    let n = max_index.wrapping_add(1);
    if n < 0 {
        g.errno = ENOMEM;
        log::error_exit(
            g,
            io.err,
            ERROR_NOMEMORY,
            &format!("MRISmodeFilterVals: could not allocate histo array of {n} ints"),
        )?;
    }
    if own.is_none()
        && let Some(ct) = mris.ct.as_ref()
    {
        annotation::set_atable_from_ctable(g, io.out, io.err, &mut sg.atable, Some(ct));
    }
    let mut nchanged = 0i32;
    let mut ino = 0i32;
    while ino < niter {
        nchanged = 0;
        for vno in 0..nv {
            if mris.vertex(vno).ripflag != 0 {
                continue;
            }
            // index -> (count, the last annotation of that index met)
            let mut counts: BTreeMap<i32, (i32, i32)> = BTreeMap::new();
            for nb in nbrs(mris, vno, mris.topology(vno).vtotal) {
                let a = mris.vertex(nb as usize).annotation;
                let index = index_of(g, sg, io, a);
                if index < 0 {
                    continue;
                }
                if index > max_index {
                    g.notices.add(
                        &FS_979,
                        format!(
                            "MRISmodeFilterAnnotations at vertex {vno}: index {index} counted outside FreeSurfer's {} entries (memory corrupted)",
                            max_index + 1
                        ),
                    );
                }
                let e = counts.entry(index).or_insert((0, 0));
                e.0 += 1;
                e.1 = a;
            }
            let a = mris.vertex(vno).annotation;
            let index = index_of(g, sg, io, a);
            let (mut max_histo, mut max_annotation) = if index >= 0 {
                if index > max_index {
                    g.notices.add(
                        &FS_979,
                        format!(
                            "MRISmodeFilterAnnotations at vertex {vno}: index {index} counted outside FreeSurfer's {} entries (memory corrupted)",
                            max_index + 1
                        ),
                    );
                }
                let e = counts.entry(index).or_insert((0, 0));
                e.1 = a;
                e.0 += 1;
                (e.0, a)
            } else {
                (0, 0)
            };
            if max_index >= 1 {
                for &(c, ann) in counts.range(1..=max_index).map(|(_, x)| x) {
                    if c > max_histo {
                        max_histo = c;
                        max_annotation = ann;
                    }
                }
            }
            mris.vertex_mut(vno).undefval = max_annotation;
        }
        for vno in 0..nv {
            let v = mris.vertex_mut(vno);
            if v.ripflag != 0 {
                continue;
            }
            if v.annotation != v.undefval {
                nchanged += 1;
            }
            v.annotation = v.undefval;
        }
        if nchanged == 0 {
            break;
        }
        ino += 1;
    }
    let _ = writeln!(
        io.out,
        "{ino} filter iterations complete ({niter} requested, {nchanged} changed)"
    );
    Ok(NO_ERROR)
}

// ---- the dilation of a vertex to a target sum ------------------------------------------------

/// `MRISdilateVertexToSum(vno, surf, measure, targetSum)`: `MRISdilateVertexToSum(): vno=<vno>,
/// targetSum = <g>` on standard output; `None` after `ERROR: MRISdilateVertexToSum(): surf is
/// NULL`, `... vno=<vno> out of range 0 to <n>`, `... measure->width=<w> != nvertices = <n>`;
/// without a measure the surface's metric properties computed (`MRIScomputeMetricProperties`) and
/// its vertex areas summed (`None` after `... targetSum = <g>, must be > 0 with area` for a target
/// not above 0). Then passes of 1, 2, ... hops around the vertex (`SetSurfHopList`, which sets each
/// listed vertex's `undefval` to its hop) while the sum of the pass's vertices' measures (frame 0
/// of the measure's first row, a column per vertex) is below the target: each pass's sum and the
/// vertices counted printed as `hop <k> <sum> <count>` (`%3d %8.3f %6d`); `None` after `ERROR:
/// MRISdilateVertexToSum(): not possible to achieve target sum <g>` once the count reaches the
/// surface's vertex count (FS-982). The mask: a new `MRI_FLOAT` volume of one voxel per vertex,
/// 1 at the vertices of the last pass's hops, 0 elsewhere (FS-983 when no pass ran).
pub fn dilate_vertex_to_sum(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    vno: i32,
    surf: Option<&mut Mris>,
    measure: Option<&Mri>,
    target_sum: f64,
) -> Result<Option<Mri>, Fatal> {
    const WHO: &str = "MRISdilateVertexToSum()";
    let tg = format_double("%g", target_sum).unwrap_or_default();
    let _ = writeln!(io.out, "{WHO}: vno={vno}, targetSum = {tg}");
    let Some(surf) = surf else {
        let _ = writeln!(io.out, "ERROR: {WHO}: surf is NULL");
        return Ok(None);
    };
    let n = surf.nvertices;
    if vno < 0 || vno >= n {
        let _ = writeln!(io.out, "ERROR: {WHO}: vno={vno} out of range 0 to {n}");
        return Ok(None);
    }
    match measure {
        Some(m) => {
            if m.vg.width != n {
                let _ = writeln!(
                    io.out,
                    "ERROR: {WHO}: measure->width={} != nvertices = {n}",
                    m.vg.width
                );
                return Ok(None);
            }
        }
        None => {
            compute_metric_properties(g, sg, io, surf)?;
            if target_sum <= 0.0 {
                let _ = writeln!(
                    io.out,
                    "ERROR: {WHO}: targetSum = {tg}, must be > 0 with area"
                );
                return Ok(None);
            }
        }
    }
    // FS-983: a target no pass is needed for (not above 0, or not a number): 0 hops
    if !(target_sum > 0.0) {
        g.notices.add(
            &FS_983,
            format!(
                "{WHO}: targetSum = {tg}: no pass needed, the mask built from 0 hops (FreeSurfer crashes)"
            ),
        );
        if !g.notices.reproduce() {
            let with = if measure.is_none() { " with area" } else { "" };
            let _ = writeln!(io.out, "ERROR: {WHO}: targetSum = {tg}, must be > 0{with}");
            return Ok(None);
        }
    }
    let reproduce = g.notices.reproduce();
    let mut measuresum = 0.0f64;
    let mut nhops = 1i32;
    // FreeSurfer's running count over every pass, and each pass's own (FS-982)
    let mut nvertices = 0i32;
    let mut before: Option<usize> = None;
    while measuresum < target_sum {
        let Some(shl) = set_surf_hop_list(g, io, vno, surf, nhops)? else {
            let _ = writeln!(io.out, "ERROR: {WHO}: could not alloc hoplist");
            return Ok(None);
        };
        let total: usize = shl.vtxlist.iter().map(Vec::len).sum();
        if !reproduce && before.is_some_and(|b| total <= b) {
            g.notices.add(
                &FS_982,
                format!(
                    "{WHO}: pass {nhops} adds no vertex: stopped (FreeSurfer goes on, a pass per vertex counted, until its running count reaches {n})"
                ),
            );
            let _ = writeln!(
                io.out,
                "ERROR: {WHO}: not possible to achieve target sum {tg}"
            );
            return Ok(None);
        }
        before = Some(total);
        measuresum = 0.0;
        let mut own = 0i32;
        for list in &shl.vtxlist {
            for &hvno in list {
                measuresum = match measure {
                    Some(m) => {
                        let val = f64::from(mri_get_vox_val(g, io.err, m, hvno, 0, 0, 0, 0)?);
                        sse::add64(val, measuresum)
                    }
                    None => sse::add64(measuresum, f64::from(surf.vertex(hvno as usize).area)),
                };
                nvertices = nvertices.wrapping_add(1);
                own += 1;
                let fs_stop = nvertices >= n;
                let stop = if reproduce { fs_stop } else { own >= n };
                if fs_stop && !stop {
                    g.notices.add(
                        &FS_982,
                        format!(
                            "{WHO}: pass {nhops}: FreeSurfer's running count reaches {n} after {own} of the pass's vertices: not stopped"
                        ),
                    );
                }
                if stop {
                    let _ = writeln!(
                        io.out,
                        "ERROR: {WHO}: not possible to achieve target sum {tg}"
                    );
                    return Ok(None);
                }
            }
        }
        let _ = writeln!(
            io.out,
            "hop {nhops:3} {} {nvertices:6}",
            fmt_f(measuresum, 8, 3)
        );
        nhops += 1;
    }
    nhops -= 1;
    let mut mask = mri_alloc(g, io.out, io.err, n, 1, 1, MRI_FLOAT)?;
    let Some(shl) = set_surf_hop_list(g, io, vno, surf, nhops)? else {
        // FS-530's refusal (0 hops): only `reproduce` reaches here (FS-983), and it has ended
        return Ok(Some(mask));
    };
    for list in &shl.vtxlist {
        for &hvno in list {
            mri_set_vox_val(g, io.err, &mut mask, hvno, 0, 0, 0, 1.0, 0)?;
        }
    }
    Ok(Some(mask))
}

// ---- the rest: rips by value and annotation, the confined dilation, the accumulations --------

/// `MRISripZeros(surf, mri)`: every unripped vertex ripped (flag 1) whose value is below 1e-5 in
/// magnitude: frame 0 of the volume's column of that vertex, or its `val` without a volume. 1
/// after `ERROR: MRISripZeros(): dimension mismatch` (standard output) for a volume whose width is
/// not the vertex count; else 0.
pub fn rip_zeros(
    g: &mut Globals,
    io: &mut Io,
    surf: &mut Mris,
    mri: Option<&Mri>,
) -> Result<i32, Fatal> {
    if let Some(m) = mri
        && m.vg.width != surf.nvertices
    {
        let _ = writeln!(io.out, "ERROR: MRISripZeros(): dimension mismatch");
        return Ok(1);
    }
    for k in 0..nverts(surf) {
        if surf.vertex(k).ripflag != 0 {
            continue;
        }
        let v = match mri {
            Some(m) => f64::from(mri_get_vox_val(g, io.err, m, k as i32, 0, 0, 0, 0)?),
            None => f64::from(surf.vertex(k).val),
        };
        if v.abs() < 1e-5 {
            surf.vertex_mut(k).ripflag = 1;
        }
    }
    Ok(0)
}

/// A vertex annotation's colour-table index (`CTABfindAnnotation`): the first entry of its
/// colour, or -1. FS-984: without a colour table FreeSurfer reports `CTABfindAnnotation: ct was
/// NULL` and reads an index it never wrote; both modes take -1 (noted).
fn find_annotation(
    g: &mut Globals,
    err: &mut dyn Write,
    mris: &Mris,
    annot: i32,
    who: &str,
) -> i32 {
    match mris.ct.as_ref() {
        Some(ct) => colortab::ctab_find_annotation(ct, annot),
        None => {
            log::error_printf(g, err, ERROR_BADPARM, "CTABfindAnnotation: ct was NULL");
            g.notices.add(
                &FS_984,
                format!(
                    "{who}: a surface without a colour table: its annotations' index taken as -1 (FreeSurfer reads one it never wrote)"
                ),
            );
            -1
        }
    }
}

/// `MRISripUnknown(surf)`: every vertex (ripped ones too) ripped whose annotation's colour-table
/// index is 0 or -1 (not in the table). 0. FS-984.
pub fn rip_unknown(g: &mut Globals, err: &mut dyn Write, surf: &mut Mris) -> i32 {
    for vtxno in 0..nverts(surf) {
        let annot = surf.vertex(vtxno).annotation;
        let annotid = find_annotation(g, err, surf, annot, "MRISripUnknown");
        if annotid == 0 || annotid == -1 {
            surf.vertex_mut(vtxno).ripflag = 1;
        }
    }
    0
}

/// An annotation of a colour-table index (`CTABannotationAtIndex`): `None` after its messages
/// (`CTABannotationAtIndex: ct was NULL`, `... index <i> was OOB`; none for an empty place), with
/// FS-985: FreeSurfer then goes on with an annotation it never wrote.
fn annotation_at(
    g: &mut Globals,
    err: &mut dyn Write,
    mris: &Mris,
    index: i32,
    who: &str,
) -> Option<i32> {
    let a = match mris.ct.as_ref() {
        Some(ct) => colortab::ctab_annotation_at_index(g, err, ct, index).ok(),
        None => {
            log::error_printf(g, err, ERROR_BADPARM, "CTABannotationAtIndex: ct was NULL");
            None
        }
    };
    if a.is_none() {
        g.notices.add(
            &FS_985,
            format!(
                "{who}: colour-table index {index} has no annotation (FreeSurfer goes on with one it never wrote)"
            ),
        );
    }
    a
}

/// `MRISdilateConfined(surf, mask, annotidmask, niters, newid)`: a copy of the mask (one voxel per
/// vertex) dilated `niters` times: each pass, every vertex outside annotation `annotidmask` (its
/// colour-table index, when not -1) set to 0, every vertex already set (not 0) set to 1, every
/// other vertex set to 1 when an unripped 1-hop neighbour is set; the pass's result the next
/// pass's input (`MRIcopy`). With `newid` above 0, every set vertex's annotation that of colour-
/// table index `newid` (FS-985: none when the table has no such annotation). The dilated mask.
/// FS-984.
pub fn dilate_confined(
    g: &mut Globals,
    io: &mut Io,
    surf: &mut Mris,
    mask: &Mri,
    annotidmask: i32,
    niters: i32,
    newid: i32,
) -> Result<Mri, Fatal> {
    const WHO: &str = "MRISdilateConfined";
    let mut mri1 = mri_copy(g, io.out, io.err, mask, None)?.unwrap_new();
    let mut mri2 = mri_copy(g, io.out, io.err, mask, None)?.unwrap_new();
    for _ in 0..niters.max(0) {
        for vtxno in 0..nverts(surf) {
            let c = vtxno as i32;
            if annotidmask > -1 {
                let annot = surf.vertex(vtxno).annotation;
                let annotid = find_annotation(g, io.err, surf, annot, WHO);
                if annotid != annotidmask {
                    mri_set_vox_val(g, io.err, &mut mri2, c, 0, 0, 0, 0.0, 0)?;
                    continue;
                }
            }
            let val = mri_get_vox_val(g, io.err, &mri1, c, 0, 0, 0, 0)?;
            if val != 0.0 {
                mri_set_vox_val(g, io.err, &mut mri2, c, 0, 0, 0, 1.0, 0)?;
                continue;
            }
            for nb in nbrs(surf, vtxno, surf.topology(vtxno).vnum) {
                if surf.vertex(nb as usize).ripflag != 0 {
                    continue;
                }
                let val = mri_get_vox_val(g, io.err, &mri1, nb, 0, 0, 0, 0)?;
                if val != 0.0 {
                    mri_set_vox_val(g, io.err, &mut mri2, c, 0, 0, 0, 1.0, 0)?;
                }
            }
        }
        mri_copy(g, io.out, io.err, &mri2, Some(&mut mri1))?;
    }
    drop(mri2);
    if newid > 0
        && let Some(new_annot) = annotation_at(g, io.err, surf, newid, WHO)
    {
        for vtxno in 0..nverts(surf) {
            if mri_get_vox_val(g, io.err, &mri1, vtxno as i32, 0, 0, 0, 0)? != 0.0 {
                surf.vertex_mut(vtxno).annotation = new_annot;
            }
        }
    }
    Ok(mri1)
}

/// `MRISaccumulateMeansOnSurface(mris, total_dof, new_dof)`: every unripped vertex's `mean` the
/// weighted mean `(mean * total_dof + val * new_dof) / (total_dof + new_dof)` (`float`). 0.
pub fn accumulate_means_on_surface(mris: &mut Mris, total_dof: i32, new_dof: i32) -> i32 {
    let ndof = total_dof.wrapping_add(new_dof) as f32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            let a = sse::mul(total_dof as f32, v.mean);
            v.mean = sse::div(sse::add(a, sse::mul(new_dof as f32, v.val)), ndof);
        }
    }
    NO_ERROR
}

/// `MRISaccumulateImaginaryMeansOnSurface(mris, total_dof, new_dof)`: as
/// [`accumulate_means_on_surface`] for `mean_imag`. 0.
pub fn accumulate_imaginary_means_on_surface(mris: &mut Mris, total_dof: i32, new_dof: i32) -> i32 {
    let ndof = total_dof.wrapping_add(new_dof) as f32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            let a = sse::mul(total_dof as f32, v.mean_imag);
            v.mean_imag = sse::div(sse::add(a, sse::mul(new_dof as f32, v.val)), ndof);
        }
    }
    NO_ERROR
}

/// `MRISaccumulateStandardErrorsOnSurface(mris, total_dof, new_dof)`: every unripped vertex's
/// `std_error` `(std_error * total_dof^2 + val * new_dof^2) / (total_dof + new_dof)^2` (the
/// squares in `int`, wrapping; the rest in `float`). 0.
pub fn accumulate_standard_errors_on_surface(mris: &mut Mris, total_dof: i32, new_dof: i32) -> i32 {
    let ndof = total_dof.wrapping_add(new_dof);
    let (t2, n2, d2) = (
        total_dof.wrapping_mul(total_dof) as f32,
        new_dof.wrapping_mul(new_dof) as f32,
        ndof.wrapping_mul(ndof) as f32,
    );
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            let a = sse::mul(t2, v.std_error);
            v.std_error = sse::div(sse::add(a, sse::mul(n2, v.val)), d2);
        }
    }
    NO_ERROR
}

/// `VERTEX_VALS` (`include/mrisurf.h`).
pub const VERTEX_VALS: i32 = 1;
/// `VERTEX_AREA`.
pub const VERTEX_AREA: i32 = 2;
/// `VERTEX_CURV`.
pub const VERTEX_CURV: i32 = 3;
/// `VERTEX_LOGODDS`.
pub const VERTEX_LOGODDS: i32 = 10;

/// `MRISclearWhichAndVal2(mris, which)`: for `VERTEX_AREA` every unripped vertex's `origarea` and
/// `val2` 0 (`MRISclearOrigAreaAndVal2`), for `VERTEX_CURV` its `curv` and `val2`
/// (`MRISclearCurvAndVal2`), for `VERTEX_VAL` and `VERTEX_LOGODDS` every vertex's `val` and `val2`
/// (`mrisSetValAndClearVal2(mris, 0)`); any other field fails FreeSurfer's assertion at the first
/// unripped vertex (FS-03).
pub fn clear_which_and_val2(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    which: i32,
) -> Result<(), Fatal> {
    match which {
        VERTEX_AREA => crate::metric::clear_orig_area_and_val2(mris),
        VERTEX_CURV => crate::metric::clear_curv_and_val2(mris),
        VERTEX_LOGODDS | VERTEX_VALS => crate::valmarks::set_val_and_clear_val2(mris, 0.0),
        _ => {
            if (0..nverts(mris)).any(|vno| mris.vertex(vno).ripflag == 0) {
                let file = format!("{}/utils/mrisurf_vals.cpp", diag::BUILD_ROOT);
                return Err(diag::assert_failed(g, io.out, io.err, &file, 4137, "false"));
            }
        }
    }
    Ok(())
}

/// `MRISmarkRandomVertices(mris, prob_marked)`: every unripped vertex marked (1) when a draw of
/// `randomNumber(0, 1)` (as a `float`) is below `prob_marked`; the others keep their marks. 0.
pub fn mark_random_vertices(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    prob_marked: f32,
) -> Result<i32, Fatal> {
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let r = random_number(g, io.env, io.out, io.err, 0.0, 1.0)? as f32;
        if r < prob_marked {
            mris.vertex_mut(vno).marked = 1;
        }
    }
    Ok(NO_ERROR)
}

/// `MRISannotation_to_label(mris, annot_index)`: the unripped vertices whose annotation is that of
/// colour-table index `annot_index`, as a new label of as many places named as the entry
/// (`LabelAlloc`), in vertex order; `None` when there are none, and (FS-985) when the table has no
/// annotation for the index.
pub fn annotation_to_label(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    annot_index: i32,
) -> Result<Option<Label>, Fatal> {
    let Some(annot) = annotation_at(g, io.err, mris, annot_index, "MRISannotation_to_label") else {
        return Ok(None);
    };
    let hits: Vec<i32> = (0..nverts(mris))
        .filter(|&vno| {
            let v = mris.vertex(vno);
            v.ripflag == 0 && v.annotation == annot
        })
        .map(|vno| vno as i32)
        .collect();
    if hits.is_empty() {
        return Ok(None);
    }
    let name = mris
        .ct
        .as_ref()
        .and_then(|ct| ct.entries[annot_index as usize].as_ref())
        .map(|e| e.name.clone())
        .unwrap_or_default();
    let mut area = label_alloc(g, io.env, io.err, hits.len() as i32, None, Some(&name))?;
    for (k, vno) in hits.into_iter().enumerate() {
        area.lv[k].vno = vno;
        area.n_points += 1;
    }
    Ok(Some(area))
}
