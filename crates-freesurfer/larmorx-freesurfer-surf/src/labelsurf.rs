// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A label's vertices on a surface (fs.surf's piece of `utils/label.cpp` that needs the surface;
//! the `LABEL` structure, its reader and writer are fs.io's `label`): FreeSurfer 8.2.0's
//! `LabelArea` (line 1373), `LabelMark` (1480), `LabelMark2` (1498), `LabelUnmark` (1604),
//! `LabelMarkSurface` (1781), `LabelFillMarked` (943), the file-local `update_vertex_indices`
//! (3938), `LabelErode` (2195), `LabelDilate` (2259), `MRISlabelInvert` (2850) and
//! `utils/mrisurf_mri.cpp`'s `MRISlabel2Mask` (5955). FS-630 to FS-632.
//!
//! A label's surface handle (`LABEL::mris`, which `LabelInit` sets) is taken to be the surface the
//! caller passes, as every FreeSurfer caller does; its template volume (`LABEL::mri_template`) is
//! the caller's `template` argument where a function needs it.

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use larmorx_core::defects::{FS_630, FS_631, FS_632};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::add64;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_io::label::{Label, label_alloc, label_realloc};
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::frames::mri_const;

use crate::coords::Io;
use crate::discrete::clear_marks;
use crate::mris::Mris;
use crate::sras;
use crate::views::vertex_coord2xyz_float;

fn npoints(area: &Label) -> usize {
    usize::try_from(area.n_points).unwrap_or(0)
}

/// The vertex a label point names, when it is one of the surface's; FS-630 noted for one that is
/// not (FreeSurfer reads or writes outside the surface's vertices there).
fn vertex_of(g: &mut Globals, mris: &Mris, vno: i32, what: &str) -> Option<usize> {
    if (0..mris.nvertices).contains(&vno) {
        return Some(vno as usize);
    }
    g.notices.add(
        &FS_630,
        format!(
            "{what}: label point at vertex {vno} of a surface of {} vertices skipped (FreeSurfer reads or writes outside the vertices)",
            mris.nvertices
        ),
    );
    None
}

/// `LabelMark(area, mris)`: each point not deleted marks its vertex (`marked = 1`). `NO_ERROR`.
/// FS-630: a point naming no vertex of the surface is skipped (FreeSurfer writes outside).
pub fn label_mark(g: &mut Globals, area: &Label, mris: &mut Mris) -> i32 {
    for n in 0..npoints(area) {
        let lv = area.lv[n];
        if lv.deleted > 0 {
            continue;
        }
        if let Some(vno) = vertex_of(g, mris, lv.vno, "LabelMark") {
            mris.vertex_mut(vno).marked = 1;
        }
    }
    NO_ERROR
}

/// `LabelMark2(area, mris)`: as [`label_mark`], `marked2 = 1`.
pub fn label_mark2(g: &mut Globals, area: &Label, mris: &mut Mris) -> i32 {
    for n in 0..npoints(area) {
        let lv = area.lv[n];
        if lv.deleted > 0 {
            continue;
        }
        if let Some(vno) = vertex_of(g, mris, lv.vno, "LabelMark2") {
            mris.vertex_mut(vno).marked2 = 1;
        }
    }
    NO_ERROR
}

/// `LabelUnmark(area, mris)`: each point's vertex (deleted ones too; a negative vertex skipped)
/// unmarked (`marked = 0`). `NO_ERROR`. FS-630: a vertex beyond the surface's is skipped.
pub fn label_unmark(g: &mut Globals, area: &Label, mris: &mut Mris) -> i32 {
    for n in 0..npoints(area) {
        let vno = area.lv[n].vno;
        if vno < 0 {
            continue;
        }
        if let Some(vno) = vertex_of(g, mris, vno, "LabelUnmark") {
            mris.vertex_mut(vno).marked = 0;
        }
    }
    NO_ERROR
}

/// `LabelMarkSurface(area, mris)`: each point not deleted and naming a vertex (not negative)
/// marks it when its rip flag is 0; a vertex beyond the surface's stops with `ERROR:
/// LabelMarkSurface: label point <vno> exceeds nvertices <n>` (standard output) and 1 (the
/// points before it marked). Else `NO_ERROR`.
pub fn label_mark_surface(io: &mut Io, area: &Label, mris: &mut Mris) -> i32 {
    for n in 0..npoints(area) {
        let lv = area.lv[n];
        if lv.vno < 0 || lv.deleted != 0 {
            continue;
        }
        if lv.vno >= mris.nvertices {
            let _ = writeln!(
                io.out,
                "ERROR: LabelMarkSurface: label point {} exceeds nvertices {}",
                lv.vno, mris.nvertices
            );
            return 1;
        }
        let v = mris.vertex_mut(lv.vno as usize);
        if v.ripflag != 0 {
            continue;
        }
        v.marked = 1;
    }
    NO_ERROR
}

/// `LabelArea(area, mris)`: the sum (`double`, in order) of the areas of the points' vertices
/// whose rip flag is 0, deleted points left out. FS-630: a point naming no vertex of the surface
/// is left out (FreeSurfer reads outside the vertices).
pub fn label_area(g: &mut Globals, area: &Label, mris: &Mris) -> f64 {
    let mut total = 0.0f64;
    for n in 0..npoints(area) {
        let lv = area.lv[n];
        let Some(vno) = vertex_of(g, mris, lv.vno, "LabelArea") else {
            continue;
        };
        let v = mris.vertex(vno);
        if v.ripflag != 0 || lv.deleted != 0 {
            continue;
        }
        total = add64(total, f64::from(v.area));
    }
    total
}

/// `LabelFillMarked(area, mris)`: passes until one adds nothing or the label is full; in each,
/// every point present before the pass (while the label has room) has its vertex's mark set to 2
/// and, when that vertex's rip flag is 0, each unripped neighbour marked 1 is marked 2 and added
/// as a new point (its vertex and current position; the place's other fields as they were) while
/// places remain. `NO_ERROR`. FS-630: a point naming no vertex of the surface is skipped.
pub fn label_fill_marked(g: &mut Globals, area: &mut Label, mris: &mut Mris) -> i32 {
    loop {
        let mut nfilled = 0i32;
        let mut n = 0usize;
        while (n as i32) < area.n_points && area.n_points + nfilled < area.max_points {
            let lvno = area.lv[n].vno;
            n += 1;
            let Some(vno) = vertex_of(g, mris, lvno, "LabelFillMarked") else {
                continue;
            };
            mris.vertex_mut(vno).marked = 2;
            if mris.vertex(vno).ripflag != 0 {
                continue;
            }
            let nbrs: Vec<i32> = {
                let vt = mris.topology(vno);
                let v = vt.v.as_deref().unwrap_or(&[]);
                v[..usize::try_from(vt.vnum).unwrap_or(0).min(v.len())].to_vec()
            };
            for nb in nbrs {
                let vn = mris.vertex_mut(nb as usize);
                if vn.ripflag != 0 {
                    continue;
                }
                if vn.marked == 1 {
                    vn.marked = 2;
                    let (x, y, z) = (vn.x, vn.y, vn.z);
                    let lvn = &mut area.lv[(area.n_points + nfilled) as usize];
                    nfilled += 1;
                    lvn.vno = nb;
                    (lvn.x, lvn.y, lvn.z) = (x, y, z);
                }
                if area.n_points + nfilled >= area.max_points {
                    break;
                }
            }
            if area.n_points + nfilled >= area.max_points {
                break;
            }
        }
        area.n_points += nfilled;
        if !(nfilled > 0 && area.n_points < area.max_points) {
            break;
        }
    }
    NO_ERROR
}

/// `update_vertex_indices(area)`: with a vertex index table and a surface handle, the table's
/// entries for the surface's vertices set to -1, then each point not deleted writes its place at
/// its vertex's entry. FS-630: an entry outside the table is skipped (FreeSurfer writes outside
/// it).
fn update_vertex_indices(g: &mut Globals, area: &mut Label, mris: &Mris) {
    if area.mris.is_none() {
        return;
    }
    let n_points = npoints(area);
    let Some(ind) = area.vertex_label_ind.as_mut() else {
        return;
    };
    let nv = usize::try_from(mris.nvertices).unwrap_or(0);
    if nv > ind.len() {
        g.notices.add(
            &FS_630,
            format!(
                "update_vertex_indices: a vertex index table of {} entries for a surface of {nv} vertices: the entries beyond it skipped (FreeSurfer writes outside the table)",
                ind.len()
            ),
        );
    }
    for e in ind.iter_mut().take(nv) {
        *e = -1;
    }
    for n in 0..n_points {
        let lv = area.lv[n];
        if lv.deleted != 0 {
            continue;
        }
        match usize::try_from(lv.vno).ok().filter(|&v| v < ind.len()) {
            Some(v) => ind[v] = n as i32,
            None => g.notices.add(
                &FS_630,
                format!(
                    "update_vertex_indices: label point at vertex {} outside the vertex index table of {} entries skipped (FreeSurfer writes outside the table)",
                    lv.vno,
                    ind.len()
                ),
            ),
        }
    }
}

/// `LabelErode(area, mris, num_times)`: `num_times` below 1 is the error `LabelErode: num_times
/// < 1` and `ERROR_BADPARM`. The surface's marks cleared; `num_times` passes, each marking the
/// label ([`label_mark`]), then deleting (`deleted = 1`) every point not deleted whose vertex (not
/// negative) has a neighbour (not negative) that is not marked, then unmarking the label
/// ([`label_unmark`]); a vertex or neighbour beyond the surface's ends the program (`LabelErode:
/// label vertex <v> too big for surface (<n>)`, `LabelErode: neighbor label vertex <v> too big for
/// surface (<n>)`, `ERROR_BADPARM`). Then the vertex index table updated and the marks cleared.
/// `NO_ERROR`.
pub fn label_erode(
    g: &mut Globals,
    io: &mut Io,
    area: &mut Label,
    mris: &mut Mris,
    num_times: i32,
) -> Result<i32, Fatal> {
    if num_times < 1 {
        log::error_printf(g, io.err, ERROR_BADPARM, "LabelErode: num_times < 1");
        return Ok(ERROR_BADPARM);
    }
    clear_marks(mris);
    for _ in 0..num_times {
        label_mark(g, area, mris);
        for i in 0..npoints(area) {
            if area.lv[i].deleted != 0 {
                continue;
            }
            let vno = area.lv[i].vno;
            if vno < 0 {
                continue;
            }
            if vno >= mris.nvertices {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_BADPARM,
                    &format!(
                        "LabelErode: label vertex {vno} too big for surface ({})",
                        mris.nvertices
                    ),
                )?;
                continue;
            }
            let mut off = false;
            let vt = mris.topology(vno as usize);
            let nbrs = vt.v.as_deref().unwrap_or(&[]);
            for &nb in &nbrs[..usize::try_from(vt.vnum).unwrap_or(0).min(nbrs.len())] {
                if nb < 0 {
                    continue;
                }
                if nb >= mris.nvertices {
                    log::error_exit(
                        g,
                        io.err,
                        ERROR_BADPARM,
                        &format!(
                            "LabelErode: neighbor label vertex {nb} too big for surface ({})",
                            mris.nvertices
                        ),
                    )?;
                    continue;
                }
                if mris.vertex(nb as usize).marked == 0 {
                    off = true;
                    break;
                }
            }
            if off {
                area.lv[i].deleted = 1;
            }
        }
        label_unmark(g, area, mris);
    }
    update_vertex_indices(g, area, mris);
    clear_marks(mris);
    Ok(NO_ERROR)
}

/// `LabelDilate(area, mris, num_times, coords)`: `num_times` below 1 is the error `LabelDilate:
/// num_times < 1` and `ERROR_BADPARM`. The surface's marks cleared; `num_times` passes, each
/// marking the label ([`label_mark`]), then adding every vertex not marked 1 that has a neighbour
/// marked above 0 as a new point at the label's end: the label first grown to `nint(1.5 x
/// places)` places when full (`LabelRealloc`), the point's vertex and its coordinates of the set
/// `coords` (an unsupported set ends the program), its place written in the vertex index table,
/// and with surface and template handles its voxel in the template (`MRISsurfaceRASToVoxel`,
/// `nint` of each); then unmarking the label. Then the vertex index table updated. `NO_ERROR`.
///
/// **FS-631:** a full label whose growth does not enlarge it (a label without places: `nint(0)` is
/// 0; it gains points when a ripped vertex keeps a mark, as `MRISclearMarks` skips ripped
/// vertices; or a growth beyond the 2^26 places `LabelRealloc` allocates, FS-426) gets its new
/// point written past its places in FreeSurfer; larmorx makes one more place in both modes,
/// noted.
#[allow(clippy::too_many_arguments)]
pub fn label_dilate(
    g: &mut Globals,
    io: &mut Io,
    area: &mut Label,
    mris: &mut Mris,
    num_times: i32,
    coords: i32,
    template: Option<&Mri>,
) -> Result<i32, Fatal> {
    if num_times < 1 {
        log::error_printf(g, io.err, ERROR_BADPARM, "LabelDilate: num_times < 1");
        return Ok(ERROR_BADPARM);
    }
    clear_marks(mris);
    let nv = usize::try_from(mris.nvertices).unwrap_or(0);
    for _ in 0..num_times {
        label_mark(g, area, mris);
        for vno in 0..nv {
            if mris.vertex(vno).marked == 1 {
                continue;
            }
            let found = {
                let vt = mris.topology(vno);
                let nbrs = vt.v.as_deref().unwrap_or(&[]);
                nbrs[..usize::try_from(vt.vnum).unwrap_or(0).min(nbrs.len())]
                    .iter()
                    .any(|&nb| mris.vertex(nb as usize).marked > 0)
            };
            if !found {
                continue;
            }
            if area.n_points >= area.max_points {
                let want = nint(g, f64::from(area.max_points) * 1.5);
                let old = std::mem::take(area);
                *area = label_realloc(g, io.env, io.err, Some(old), want)?;
                if area.n_points >= area.max_points {
                    g.notices.add(
                        &FS_631,
                        format!(
                            "LabelDilate: a full label of {} places not grown by LabelRealloc({want}): one more place made (FreeSurfer writes past the places)",
                            area.max_points
                        ),
                    );
                    let more = area.n_points + 1;
                    area.lv.resize(more as usize, Default::default());
                    area.max_points = more;
                }
            }
            let n = area.n_points as usize;
            area.n_points += 1;
            area.lv[n].vno = vno as i32;
            let (x, y, z) = vertex_coord2xyz_float(g, io.err, mris.vertex(vno), coords)?;
            (area.lv[n].x, area.lv[n].y, area.lv[n].z) = (x, y, z);
            if let Some(ind) = area.vertex_label_ind.as_mut() {
                if vno < ind.len() {
                    ind[vno] = n as i32;
                } else {
                    g.notices.add(
                        &FS_630,
                        format!(
                            "LabelDilate: vertex {vno} outside the vertex index table of {} entries skipped (FreeSurfer writes outside the table)",
                            ind.len()
                        ),
                    );
                }
            }
            if area.mris.is_some()
                && area.mri_template.is_some()
                && let Some(t) = template
            {
                let p = sras::mris_surface_ras_to_voxel(
                    g,
                    io,
                    mris,
                    t,
                    f64::from(x),
                    f64::from(y),
                    f64::from(z),
                )?;
                area.lv[n].xv = nint(g, p[0]);
                area.lv[n].yv = nint(g, p[1]);
                area.lv[n].zv = nint(g, p[2]);
            }
        }
        label_unmark(g, area, mris);
    }
    update_vertex_indices(g, area, mris);
    Ok(NO_ERROR)
}

/// `MRISlabel2Mask(surf, lb, mask, frame, statflag)`: into `mask` (a new float volume of one
/// voxel per vertex and `frame + 1` frames, zeros, `MRIconst`, when there is none) each point's
/// voxel (its vertex, row 0, slice 0, frame `frame`) set to 1, or to its `stat` with `statflag`
/// (`MRIsetVoxVal`). A frame the mask does not have: `ERROR: MRISlabel2Mask(): frame=<f> >=
/// mask->nframes-1=<n>` and no mask; a point beyond the surface's vertices: `ERROR:
/// MRISlabel2Mask(): label vertex <v> is >= nvertices <n>` and no mask (both on standard output; a
/// given mask keeps what was set).
#[allow(clippy::too_many_arguments)]
pub fn mris_label2mask(
    g: &mut Globals,
    io: &mut Io,
    surf: &Mris,
    lb: &Label,
    mask: Option<&mut Mri>,
    frame: i32,
    statflag: bool,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let m: &mut Mri = match mask {
        Some(m) => m,
        None => {
            let dims = [surf.nvertices, 1, 1, frame.wrapping_add(1)];
            match mri_const(g, io.out, io.err, dims, 0.0, None)? {
                Ret::New(m) => fresh.insert(m),
                _ => return Ok(Ret::Null),
            }
        }
    };
    if frame > m.nframes - 1 {
        let _ = writeln!(
            io.out,
            "ERROR: MRISlabel2Mask(): frame={frame} >= mask->nframes-1={}",
            m.nframes - 1
        );
        return Ok(Ret::Null);
    }
    for n in 0..npoints(lb) {
        let lv = lb.lv[n];
        if lv.vno >= surf.nvertices {
            let _ = writeln!(
                io.out,
                "ERROR: MRISlabel2Mask(): label vertex {} is >= nvertices {}",
                lv.vno, surf.nvertices
            );
            return Ok(Ret::Null);
        }
        let v = if statflag { lv.stat } else { 1.0 };
        mri_set_vox_val(g, io.err, m, lv.vno, 0, 0, frame, v, 0)?;
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRISlabelInvert(surf, label)`: a new label (`LabelAlloc` with the label's subject name, so
/// `SUBJECTS_DIR` must be set) of the surface's vertices the label does not hold, in order, each
/// with its current position and `stat` 0 (through [`mris_label2mask`]'s mask of the label).
///
/// **FS-632:** a label with a point beyond the surface's vertices makes the mask fail, and
/// FreeSurfer then crashes; larmorx gives `None` after the mask's error, `reproduce` the crash
/// (exit status 139).
pub fn mris_label_invert(
    g: &mut Globals,
    io: &mut Io,
    surf: &Mris,
    label: &Label,
) -> Result<Option<Label>, Fatal> {
    let mask = match mris_label2mask(g, io, surf, label, None, 0, false)? {
        Ret::New(m) => m,
        _ => {
            if g.notices.reproduce() {
                g.notices.add(
                    &FS_632,
                    "MRISlabelInvert of a label with a point beyond the surface's vertices: FreeSurfer crashes (status 139)",
                );
                return Err(Fatal::Exit(139));
            }
            g.notices.add(
                &FS_632,
                "MRISlabelInvert of a label with a point beyond the surface's vertices: no label (FreeSurfer crashes)",
            );
            return Ok(None);
        }
    };
    let nv = surf.nvertices.max(0);
    let mut nlabel = 0;
    for vtxno in 0..nv {
        if mri_get_vox_val(g, io.err, &mask, vtxno, 0, 0, 0, 0)? > 0.5 {
            nlabel += 1;
        }
    }
    let ninv = surf.nvertices - nlabel;
    let mut inv = label_alloc(g, io.env, io.err, ninv, Some(&label.subject_name), None)?;
    inv.n_points = ninv;
    let mut n = 0usize;
    for vtxno in 0..nv as usize {
        if mri_get_vox_val(g, io.err, &mask, vtxno as i32, 0, 0, 0, 0)? < 0.5 {
            let v = surf.vertex(vtxno);
            let lv = &mut inv.lv[n];
            lv.vno = vtxno as i32;
            (lv.x, lv.y, lv.z) = (v.x, v.y, v.z);
            lv.stat = 0.0;
            n += 1;
        }
    }
    Ok(Some(inv))
}
