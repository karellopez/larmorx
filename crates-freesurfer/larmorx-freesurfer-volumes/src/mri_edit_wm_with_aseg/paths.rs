// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `remove_paths_to_cortex(mri_wm, mri_T1, mri_aseg)` (`mri_edit_wm_with_aseg.cpp` 1965-2306) and
//! the region's helpers (`remove_anterior_and_superior_amygdala`,
//! `remove_lateral_and_anterior_hippocampus`, `remove_medial_voxels`, `remove_unknown_voxels`,
//! `remove_gray_matter_voxels`).
//!
//! FS-1234 (kept, rule C): the marks of the search (`mri_hippo`, `mri_roi`, `mri_filled`) are
//! clones of the segmentation, which FreeSurfer writes (and on the right also reads) through
//! byte pointers (`MRIvox`) and on the left reads by their type: with a segmentation that is not
//! of bytes (recon-all's `aseg.presurf.mgz`: `int` in the classic stream, `float` in v8) the marks
//! land in other voxels and the search finds no path (`0 voxels added ...` on every recorded
//! head). Ported as FreeSurfer runs it, both modes.

#![allow(non_upper_case_globals)]

use std::io::Write;

use larmorx_core::defects::FS_1234;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE, DIAG_WRITE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri,
};
use larmorx_freesurfer_mri::voxel::{mri_bounding_box, mri_get_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, mri_binarize, mri_clear, mri_clone};
use larmorx_freesurfer_volume::cma::*;
use larmorx_freesurfer_volume::deriv::neighbors_on;
use larmorx_freesurfer_volume::mriset::{mri_copy_label, mri_dilate};

use super::local::{
    AUTO_FILL, Ax, Seg, WM_MIN_VAL, distance_to_anterior_edge, distance_to_inferior_edge,
    distance_to_label, distance_to_lateral_edge, distance_to_superior_edge, distance_to_wm, ix,
    set_vox, vox,
};

/// `MRIgetVoxVal(m, x, y, z, 0)`: a voxel inside the volume of the common types read from its
/// bytes, any other through `MRIgetVoxVal` itself.
#[inline]
fn tv(g: &mut Globals, err: &mut dyn Write, m: &Mri, x: i32, y: i32, z: i32) -> Result<f32, Fatal> {
    if x >= 0
        && y >= 0
        && z >= 0
        && x < m.vg.width
        && y < m.vg.height
        && z < m.vg.depth
        && let Some(c) = m.chunk.as_ref()
    {
        let row = m.vox_per_row as usize * m.bytes_per_vox;
        let b =
            (z as usize * m.vg.height as usize + y as usize) * row + x as usize * m.bytes_per_vox;
        match m.mtype {
            MRI_UCHAR => return Ok(f32::from(c[b])),
            MRI_INT => return Ok(i32::from_le_bytes([c[b], c[b + 1], c[b + 2], c[b + 3]]) as f32),
            MRI_FLOAT => {
                return Ok(f32::from_bits(u32::from_le_bytes([
                    c[b],
                    c[b + 1],
                    c[b + 2],
                    c[b + 3],
                ])));
            }
            MRI_SHORT => return Ok(f32::from(i16::from_le_bytes([c[b], c[b + 1]]))),
            _ => {}
        }
    }
    mri_get_vox_val(g, err, m, x, y, z, 0, 0)
}

/// The search's working volumes, and where to write its diagnostic volumes.
pub struct Ctx<'a> {
    pub env: &'a Environment,
    pub dir: Option<&'a std::path::Path>,
}

/// `remove_paths_to_cortex(mri_wm, mri_T1, mri_aseg)`: for each hemisphere, the region around
/// its hippocampus, amygdala and inferior lateral ventricle (5 dilations, trimmed); in it, from the
/// structures' voxels with white matter below, a search inferior and lateral for a path to cortex;
/// the brightest voxel near its end turned on in the white matter, until no path is found;
/// `<n> voxels added to wm to prevent paths from MTL structures to cortex`.
pub fn remove_paths_to_cortex(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: &Ctx,
    wm: &mut Mri,
    t1: &Mri,
    seg: &Seg,
) -> Result<(), Fatal> {
    let aseg = seg.m;
    if aseg.mtype != MRI_UCHAR {
        g.notices.add(
            &FS_1234,
            format!(
                "mri_edit_wm_with_aseg: the search for paths from the hippocampus to cortex marks a clone of the segmentation (type {}) through its bytes: as in FreeSurfer, kept",
                aseg.mtype
            ),
        );
    }
    let mut total_filled = 0;
    let mut filled: Option<Mri> = None;
    for left in [true, false] {
        let mut hippo = mri_clone(g, out, err, aseg, None)?.unwrap_new();
        let mut roi = mri_clone(g, out, err, aseg, None)?.unwrap_new();
        let labels = if left {
            [Left_Hippocampus, Left_Amygdala, Left_Inf_Lat_Vent]
        } else {
            [Right_Hippocampus, Right_Amygdala, Right_Inf_Lat_Vent]
        };
        for l in labels {
            mri_copy_label(g, err, aseg, &mut roi, l)?;
        }
        mri_binarize(g, out, err, Pair::Same(&mut roi), 1.0, 0.0, 128.0)?;
        for _ in 0..5 {
            mri_dilate(g, out, err, Pair::Same(&mut roi))?;
        }
        remove_anterior_and_superior_amygdala(g, err, &mut roi, seg)?;
        // remove_lateral_and_anterior_hippocampus(mri_roi, mri_aseg, left): its lateral border is
        // found anew for each column (the column itself, or none), so every voxel it would remove
        // is skipped: it changes nothing
        remove_medial_voxels(g, err, &mut roi, seg, left)?;
        remove_unknown_voxels(g, err, &mut roi, seg, left)?;
        remove_gray_matter_voxels(g, err, &mut roi, seg)?;
        let (_, b) = mri_bounding_box(g, err, &roi, 1)?;
        let (w, h, d) = (aseg.vg.width, aseg.vg.height, aseg.vg.depth);
        let x1 = b.x.max(1);
        let x2 = (b.x + b.dx - 1).min(w - 2);
        let y1 = b.y.max(1);
        let y2 = (b.y + b.dy - 1).min(h - 2);
        let z1 = b.z.max(1);
        let z2 = (b.z + b.dz - 1).min(d - 2);
        if g.gdiag & DIAG_WRITE != 0 {
            let name = if left { "lh_roi.mgz" } else { "rh_roi.mgz" };
            mri_write(
                g,
                out,
                err,
                ctx.env,
                ctx.dir,
                &mut roi,
                name,
                None,
                MGZ_INTENT_UNKNOWN,
            )?;
        }
        let (mut xgm, mut ygm, mut zgm) = (-1, -1, -1);
        let mut niter = 0;
        loop {
            let mut xmedial = if left { hippo.vg.width } else { 0 };
            let mut zanterior = 0;
            mri_clear(g, err, &mut hippo)?;
            for x in x1..=x2 {
                for y in y1..=y2 {
                    for z in z1..=z2 {
                        if tv(g, err, wm, x, y, z)? > f32::from(WM_MIN_VAL) {
                            continue;
                        }
                        let label = seg.label(g, err, x, y, z)?;
                        if labels.contains(&label)
                            && tv(g, err, wm, x, y + 1, z)? > f32::from(WM_MIN_VAL)
                        {
                            set_vox(g, &mut hippo, x, y, z, 128);
                            if (left && x < xmedial) || (!left && x > xmedial) {
                                xmedial = x;
                            }
                            if z > zanterior {
                                zanterior = z;
                            }
                        }
                    }
                }
            }
            for x in x1..=x2 {
                for y in y1..=y2 {
                    for z in z1..=z2 {
                        let near = if left {
                            x < xmedial + 10
                        } else {
                            x > xmedial - 10
                        };
                        if vox(g, &hippo, x, y, z) != 0 && (near || z > zanterior - 15) {
                            set_vox(g, &mut hippo, x, y, z, 0);
                        }
                    }
                }
            }
            if niter == 0 && g.gdiag & DIAG_WRITE != 0 {
                mri_write(
                    g,
                    out,
                    err,
                    ctx.env,
                    ctx.dir,
                    &mut hippo,
                    "f1.mgz",
                    None,
                    MGZ_INTENT_UNKNOWN,
                )?;
            }
            let fl = match filled.as_mut() {
                Some(f) => {
                    mri_copy(g, out, err, &hippo, Some(f))?;
                    f
                }
                None => filled.insert(mri_copy(g, out, err, &hippo, None)?.unwrap_new()),
            };
            let mut path = false;
            loop {
                let mut nfilled = 0;
                let xs = if left { xmedial..=x2 } else { x1..=xmedial };
                'search: for x in xs {
                    for y in y1..=y2 {
                        for z in 0..=z2 {
                            // the left reads the marks by their type, the right through bytes
                            let (f0, r0) = if left {
                                (
                                    tv(g, err, fl, x, y, z)? == 0.0,
                                    tv(g, err, &roi, x, y, z)? == 0.0,
                                )
                            } else {
                                (vox(g, fl, x, y, z) == 0, vox(g, &roi, x, y, z) == 0)
                            };
                            if f0 || r0 {
                                continue;
                            }
                            for xk in -1..=1 {
                                let xi = ix(g, fl, Ax::X, xk + x);
                                for yk in 0..=1 {
                                    let yi = ix(g, fl, Ax::Y, yk + y);
                                    for zk in -1..=1 {
                                        let zi = ix(g, fl, Ax::Z, zk + z);
                                        let (r, f) = if left {
                                            (
                                                tv(g, err, &roi, xi, yi, zi)? == 0.0,
                                                tv(g, err, fl, xi, yi, zi)? != 0.0,
                                            )
                                        } else {
                                            (
                                                vox(g, &roi, xi, yi, zi) == 0,
                                                vox(g, fl, xi, yi, zi) != 0,
                                            )
                                        };
                                        // FreeSurfer tests the region first on the left, the mark first on the right
                                        if r || f {
                                            continue;
                                        }
                                        if tv(g, err, wm, xi, yi, zi)? > f32::from(WM_MIN_VAL) {
                                            continue;
                                        }
                                        if is_cortex(seg.get(g, err, xi, yi, zi)?) {
                                            path = true;
                                            (xgm, ygm, zgm) = (xi, yi, zi);
                                            break 'search;
                                        }
                                        let lateral = if left {
                                            xi > xmedial + 10
                                        } else {
                                            xi < xmedial - 10
                                        };
                                        if !lateral {
                                            continue;
                                        }
                                        let fills = if left {
                                            matches!(
                                                seg.label(g, err, xi, yi, zi)?,
                                                Unknown
                                                    | Left_Cerebral_Cortex
                                                    | Left_Hippocampus
                                                    | Left_Amygdala
                                                    | Left_Inf_Lat_Vent
                                                    | Left_Cerebral_White_Matter
                                            )
                                        } else {
                                            true
                                        };
                                        if fills && tv(g, err, &roi, xi, yi, zi)? > 0.0 {
                                            nfilled += 1;
                                            set_vox(g, fl, xi, yi, zi, 128);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if !(nfilled > 0 && !path) {
                    break;
                }
            }
            if path {
                // the brightest voxel of the path near its end, on in the white matter
                let mut brightest = tv(g, err, t1, xgm, ygm, zgm)?;
                let (mut xb, mut yb, mut zb) = (xgm, ygm, zgm);
                for xk in -3..=3 {
                    let xi = ix(g, fl, Ax::X, xk + xgm);
                    for yk in -3..=0 {
                        let yi = ix(g, fl, Ax::Y, yk + ygm);
                        for zk in -3..=3 {
                            let zi = ix(g, fl, Ax::Z, zk + zgm);
                            if tv(g, err, fl, xi, yi, zi)? == 0.0 {
                                continue;
                            }
                            if tv(g, err, wm, xi, yi, zi)? > f32::from(WM_MIN_VAL) {
                                continue;
                            }
                            if neighbors_on(g, err, wm, xi, yi, zi, i32::from(WM_MIN_VAL))? == 0 {
                                continue;
                            }
                            let intensity = tv(g, err, t1, xi, yi, zi)?;
                            if intensity > brightest {
                                brightest = intensity;
                                (xb, yb, zb) = (xi, yi, zi);
                            }
                        }
                    }
                }
                if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
                    let _ = writeln!(out, "filling voxel ({xb}, {yb}, {zb})");
                }
                set_vox(g, wm, xb, yb, zb, AUTO_FILL);
                set_vox(g, &mut hippo, xb, yb, zb, 0);
                total_filled += 1;
            }
            let more = path && {
                let n = niter;
                niter += 1;
                n < 100000
            };
            if !more {
                break;
            }
        }
    }
    let _ = writeln!(
        out,
        "{total_filled} voxels added to wm to prevent paths from MTL structures to cortex"
    );
    Ok(())
}

/// `remove_anterior_and_superior_amygdala(mri_roi, mri_aseg)`: each amygdala voxel (x outermost,
/// y from 1, z below the last slice) within 3 of its label's anterior edge or 2 of its superior
/// edge, more than 3 from its inferior and lateral edges: off in the region, with the voxel
/// anterior to it and the one above.
fn remove_anterior_and_superior_amygdala(
    g: &mut Globals,
    err: &mut dyn Write,
    roi: &mut Mri,
    seg: &Seg,
) -> Result<(), Fatal> {
    let (w, h, d) = (roi.vg.width, roi.vg.height, roi.vg.depth);
    for x in 0..w {
        for y in 1..h {
            for z in 0..d - 1 {
                let label = seg.label(g, err, x, y, z)?;
                if !is_amygdala(label) {
                    continue;
                }
                let left = label == Left_Amygdala;
                if distance_to_anterior_edge(g, err, seg, label, x, y, z)? > 3
                    && distance_to_superior_edge(g, err, seg, label, x, y, z)? > 2
                {
                    continue;
                }
                if distance_to_inferior_edge(g, err, seg, label, x, y, z)? <= 3 {
                    continue;
                }
                if distance_to_lateral_edge(g, err, seg, label, x, y, z, left)? <= 3 {
                    continue;
                }
                set_vox(g, roi, x, y, z, 0);
                set_vox(g, roi, x, y, z + 1, 0);
                set_vox(g, roi, x, y - 1, z, 0);
            }
        }
    }
    Ok(())
}

/// `remove_medial_voxels(mri_roi, mri_aseg, left)`: in each slice, the hemisphere's hippocampus
/// and amygdala from `xmin` to `xmax`; the amygdala, hippocampus and cortex voxels medial to a
/// third of the way (`(2 xmin + xmax) / 3` on the left, `(2 xmax + xmin) / 3` on the right) and
/// the unknown ones within 2 of the medial end: off in the region.
fn remove_medial_voxels(
    g: &mut Globals,
    err: &mut dyn Write,
    roi: &mut Mri,
    seg: &Seg,
    left: bool,
) -> Result<(), Fatal> {
    let (w, h, d) = (roi.vg.width, roi.vg.height, roi.vg.depth);
    for z in 0..d {
        let (mut xmin, mut xmax) = (w - 1, 0);
        for x in 0..w {
            for y in 0..h {
                let label = seg.label(g, err, x, y, z)?;
                let mtl = if left {
                    label == Left_Hippocampus || label == Left_Amygdala
                } else {
                    label == Right_Amygdala || label == Right_Hippocampus
                };
                if mtl {
                    xmin = xmin.min(x);
                    xmax = xmax.max(x);
                }
            }
        }
        let xmid = if left {
            (2 * xmin + xmax) / 3
        } else {
            (2 * xmax + xmin) / 3
        };
        for x in 0..w {
            for y in 0..h {
                let label = seg.label(g, err, x, y, z)?;
                if left {
                    if (label == Left_Amygdala
                        || label == Left_Hippocampus
                        || label == Left_Cerebral_Cortex)
                        && x < xmid
                    {
                        set_vox(g, roi, x, y, z, 0);
                    }
                    if label == Unknown && x < xmin + 3 {
                        set_vox(g, roi, x, y, z, 0);
                    }
                } else {
                    if (label == Right_Amygdala
                        || label == Right_Hippocampus
                        || label == Right_Cerebral_Cortex)
                        && x > xmid
                    {
                        set_vox(g, roi, x, y, z, 0);
                    }
                    if label == Unknown && x > xmax - 3 {
                        set_vox(g, roi, x, y, z, 0);
                    }
                }
            }
        }
    }
    Ok(())
}

/// `remove_unknown_voxels(mri_roi, mri_aseg, left)`: in each slice, over the lateral half of the
/// hemisphere's hippocampus and amygdala, the unknown voxels of the region with no label of at
/// least 5 within 5 below: off.
fn remove_unknown_voxels(
    g: &mut Globals,
    err: &mut dyn Write,
    roi: &mut Mri,
    seg: &Seg,
    left: bool,
) -> Result<(), Fatal> {
    let (w, h, d) = (roi.vg.width, roi.vg.height, roi.vg.depth);
    let (hl, al) = if left {
        (Left_Hippocampus, Left_Amygdala)
    } else {
        (Right_Hippocampus, Right_Amygdala)
    };
    for z in 0..d {
        let (mut xmin, mut xmax) = (w - 1, 0);
        for x in 0..w {
            for y in 0..h {
                let label = seg.label(g, err, x, y, z)?;
                if label == hl || label == al {
                    xmin = xmin.min(x);
                    xmax = xmax.max(x);
                }
            }
        }
        let xmid = (xmin + xmax) / 2;
        if left {
            xmax = xmid;
        } else {
            xmin = xmid;
        }
        for x in xmin..=xmax {
            for y in 0..h {
                if vox(g, roi, x, y, z) == 0 {
                    continue;
                }
                if seg.label(g, err, x, y, z)? != Unknown {
                    continue;
                }
                if distance_to_wm(g, err, seg.m, x, y, z, 0, 1, 0, 6, true)? >= 6 {
                    set_vox(g, roi, x, y, z, 0);
                }
            }
        }
    }
    Ok(())
}

/// `remove_gray_matter_voxels(mri_roi, mri_aseg)`: the cortex voxels of the region (slices
/// outermost, then columns) with their hemisphere's amygdala within 2 posterior and none within
/// 9 anterior: off.
fn remove_gray_matter_voxels(
    g: &mut Globals,
    err: &mut dyn Write,
    roi: &mut Mri,
    seg: &Seg,
) -> Result<(), Fatal> {
    let (w, h, d) = (roi.vg.width, roi.vg.height, roi.vg.depth);
    for z in 0..d {
        for x in 0..w {
            for y in 0..h {
                if vox(g, roi, x, y, z) == 0 {
                    continue;
                }
                let label = seg.label(g, err, x, y, z)?;
                if !is_cortex(label) {
                    continue;
                }
                let alabel = if label == Left_Cerebral_Cortex {
                    Left_Amygdala
                } else {
                    Right_Amygdala
                };
                if distance_to_label(g, err, seg, alabel, x, y, z, 0, 0, -1, 10)? <= 2
                    && distance_to_label(g, err, seg, alabel, x, y, z, 0, 0, 1, 10)? >= 10
                {
                    set_vox(g, roi, x, y, z, 0);
                }
            }
        }
    }
    Ok(())
}
