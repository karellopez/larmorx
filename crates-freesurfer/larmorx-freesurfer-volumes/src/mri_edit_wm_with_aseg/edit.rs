// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `edit_segmentation(mri_wm, mri_T1, mri_seg)` (`mri_edit_wm_with_aseg.cpp` 440-1984): the white
//! matter edited from the segmentation's labels in nine passes, `SEG EDIT: <n> voxels turned on,
//! <n> voxels turned off.` on standard output.

#![allow(non_upper_case_globals)]

use std::io::Write;

use larmorx_core::defects::FS_1237;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_volume::arith::mri_clone;
use larmorx_freesurfer_volume::cma::*;
use larmorx_freesurfer_volume::labels::mri_labels_in_nbhd;

use super::local::{
    AUTO_FILL, Ax, Seg, WM_MIN_VAL, anterior_edge_of_amygdala, at_debug, distance_to_label,
    distance_to_wm, ix, medial_edge_of_hippocampus, neighbor_label, nonfilled_in_nbhd, set_vox,
    vox,
};

/// What the passes share.
struct Ed<'a, 'b> {
    seg: &'b Seg<'a>,
    t1: &'b Mri,
    non: i32,
    noff: i32,
}

/// The white matter and the filled marks turned on at `(x, y, z)` (`AUTO_FILL`), counted.
fn fill(g: &mut Globals, e: &mut Ed, wm: &mut Mri, filled: &mut Mri, x: i32, y: i32, z: i32) {
    set_vox(g, wm, x, y, z, AUTO_FILL);
    set_vox(g, filled, x, y, z, AUTO_FILL);
    e.non += 1;
}

/// `printf("changing label (%d, %d, %d) to wm (gm lateral to inf-lat-vent)\n", ...)` at the debug
/// voxel.
fn changing(g: &Globals, out: &mut dyn Write, x: i32, y: i32, z: i32) {
    if at_debug(g, x, y, z) {
        let _ = writeln!(
            out,
            "changing label ({x}, {y}, {z}) to wm (gm lateral to inf-lat-vent)"
        );
    }
}

/// `edit_segmentation(mri_wm, mri_T1, mri_seg)`.
pub fn edit_segmentation(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    wm: &mut Mri,
    t1: &Mri,
    seg: &Seg,
    fcd: bool,
    fillven: i32,
    fill_seg_wm: bool,
) -> Result<(), Fatal> {
    let mut filled = mri_clone(g, out, err, wm, None)?.unwrap_new();
    let (width, height, depth) = (wm.vg.width, wm.vg.height, wm.vg.depth);
    let mut e = Ed {
        seg,
        t1,
        non: 0,
        noff: 0,
    };
    let on = |g: &mut Globals, m: &Mri, x, y, z| vox(g, m, x, y, z) >= WM_MIN_VAL;

    // the first pass: erase, fill and spackle by label
    for z in 0..depth {
        for y in (1..=height - 2).rev() {
            for x in 1..width - 1 {
                let mut label = seg.label(g, err, x, y, z)?;
                let mut left = false;
                let nl = |g: &mut Globals, err: &mut dyn Write, l| {
                    neighbor_label(g, err, seg, x, y, z, 1, l)
                };
                let mut erase = false;
                let mut lesion = false;
                let mut ilv = false;
                match label {
                    Unknown => {
                        // erased when only unknowns surround it
                        erase = mri_labels_in_nbhd(g, err, seg.m, x, y, z, 2, Unknown)?
                            >= 5 * 5 * 5 - 1;
                    }
                    Left_Cerebellum_White_Matter
                    | Left_Cerebellum_Exterior
                    | Left_Cerebellum_Cortex
                    | Right_Cerebellum_White_Matter
                    | Right_Cerebellum_Exterior
                    | Right_Cerebellum_Cortex
                    | Optic_Chiasm => erase = true,
                    non_WM_hypointensities
                    | Left_non_WM_hypointensities
                    | Right_non_WM_hypointensities => lesion = !fcd,
                    Left_Lesion
                    | Right_Lesion
                    | WM_hypointensities
                    | Left_WM_hypointensities
                    | Right_WM_hypointensities => lesion = true,
                    Left_choroid_plexus | Right_choroid_plexus => {
                        if !nl(g, err, Left_Inf_Lat_Vent)?
                            && !nl(g, err, Right_Inf_Lat_Vent)?
                            && !on(g, wm, x, y, z)
                        {
                            if at_debug(g, x, y, z) {
                                let _ = writeln!(
                                    out,
                                    "filling choroid adjacent to ventricle at ({x}, {y}, {z})"
                                );
                            }
                            fill(g, &mut e, wm, &mut filled, x, y, z);
                        }
                    }
                    Left_Lateral_Ventricle | Right_Lateral_Ventricle => {
                        if (nl(g, err, Left_Cerebral_White_Matter)?
                            || nl(g, err, Right_Cerebral_White_Matter)?)
                            && !on(g, wm, x, y, z)
                        {
                            if at_debug(g, x, y, z) {
                                let _ = writeln!(
                                    out,
                                    "filling ventricle adjacent to wm at ({x}, {y}, {z})"
                                );
                            }
                            fill(g, &mut e, wm, &mut filled, x, y, z);
                        } else if fillven != 0 {
                            if !on(g, wm, x, y, z) {
                                fill(g, &mut e, wm, &mut filled, x, y, z);
                            }
                            ilv = true;
                        }
                    }
                    Left_Inf_Lat_Vent | Right_Inf_Lat_Vent => ilv = true,
                    Left_Hippocampus | Right_Hippocampus => {
                        left = label == Left_Hippocampus;
                        if medial_edge_of_hippocampus(
                            g,
                            err,
                            seg,
                            x,
                            y,
                            z,
                            if left { -1 } else { 1 },
                        )? {
                            continue;
                        }
                        if distance_to_wm(g, err, wm, x, y, z, 0, -1, 0, 5, true)? <= 3 {
                            continue;
                        }
                        let near = |g: &mut Globals, err: &mut dyn Write, l| {
                            distance_to_label(g, err, seg, l, x, y, z, 0, -1, 0, 5)
                        };
                        if near(
                            g,
                            err,
                            if left {
                                Left_Cerebral_White_Matter
                            } else {
                                Right_Cerebral_White_Matter
                            },
                        )? < 3
                            || near(g, err, if left { Left_Thalamus } else { Right_Thalamus })? < 3
                            || near(
                                g,
                                err,
                                if left {
                                    Left_VentralDC
                                } else {
                                    Right_VentralDC
                                },
                            )? < 3
                        {
                            continue;
                        }
                        let xi = ix(
                            g,
                            wm,
                            Ax::X,
                            if label == Right_Hippocampus {
                                x - 1
                            } else {
                                x + 1
                            },
                        );
                        let mut yi = ix(g, wm, Ax::Y, y + 1);
                        // diagonally lateral and inferior
                        let olabel = seg.label(g, err, xi, yi, z)?;
                        if is_cortex(olabel) && !on(g, wm, xi, yi, z) {
                            fill(g, &mut e, wm, &mut filled, xi, yi, z);
                        }
                        label = seg.label(g, err, x, yi, z)?;
                        if (label == Left_Cerebral_Cortex
                            || label == Right_Cerebral_Cortex
                            || label == Left_Cerebral_White_Matter
                            || label == Right_Cerebral_White_Matter)
                            && !on(g, wm, x, yi, z)
                        {
                            fill(g, &mut e, wm, &mut filled, x, yi, z);
                            yi = ix(g, wm, Ax::Y, y + 2);
                            if !on(g, wm, x, yi, z) {
                                fill(g, &mut e, wm, &mut filled, x, yi, z);
                            }
                        }
                    }
                    Left_Accumbens_area | Right_Accumbens_area | Left_Caudate | Left_vessel
                    | Right_vessel | Right_Caudate | Left_Putamen | Right_Putamen
                    | Left_Pallidum | Right_Pallidum | Right_Thalamus | Left_Thalamus
                    | Left_VentralDC | Right_VentralDC => {
                        if !on(g, wm, x, y, z) {
                            fill(g, &mut e, wm, &mut filled, x, y, z);
                        }
                    }
                    Left_Cerebral_White_Matter | Right_Cerebral_White_Matter => {
                        let yi = ix(g, wm, Ax::Y, y - 1);
                        let slabel = seg.label(g, err, x, yi, z)?;
                        if is_inf_lat_vent(slabel) && !on(g, wm, x, y, z) {
                            fill(g, &mut e, wm, &mut filled, x, y, z);
                        }
                        // only those not next to cortex
                        if fill_seg_wm
                            && !nl(g, err, Left_Cerebral_Cortex)?
                            && !nl(g, err, Right_Cerebral_Cortex)?
                        {
                            fill(g, &mut e, wm, &mut filled, x, y, z);
                        }
                    }
                    _ => {}
                }
                let _ = left;
                if erase {
                    if !nl(g, err, Left_Cerebral_Cortex)?
                        && !nl(g, err, Right_Cerebral_Cortex)?
                        && on(g, wm, x, y, z)
                    {
                        set_vox(g, wm, x, y, z, 0);
                        e.noff += 1;
                    }
                    continue;
                }
                if lesion {
                    if !nl(g, err, Left_Cerebral_Cortex)?
                        && !nl(g, err, Right_Cerebral_Cortex)?
                        && !on(g, wm, x, y, z)
                    {
                        fill(g, &mut e, wm, &mut filled, x, y, z);
                    }
                    continue;
                }
                if !ilv {
                    continue;
                }
                // the inferior lateral ventricle (and the lateral ventricles falling into it)
                let xi = ix(
                    g,
                    wm,
                    Ax::X,
                    if label == Left_Inf_Lat_Vent {
                        x + 1
                    } else {
                        x - 1
                    },
                );
                let olabel = seg.label(g, err, xi, y, z)?;
                if is_cortex(olabel) && !on(g, wm, xi, y, z) {
                    changing(g, out, xi, y, z);
                    fill(g, &mut e, wm, &mut filled, xi, y, z);
                }
                let mut yi = ix(g, wm, Ax::Y, y + 1);
                let olabel = seg.label(g, err, xi, yi, z)?;
                if is_cortex(olabel) && !on(g, wm, xi, yi, z) {
                    changing(g, out, xi, yi, z);
                    fill(g, &mut e, wm, &mut filled, xi, yi, z);
                }
                if distance_to_wm(g, err, wm, x, y, z, 0, -1, 0, 5, true)? <= 3 {
                    continue;
                }
                for dz in [-1, 1] {
                    let zi = ix(g, wm, Ax::Z, z + dz);
                    let olabel = seg.label(g, err, x, yi, zi)?;
                    if is_cortex(olabel) && !on(g, wm, x, yi, zi) {
                        changing(g, out, x, yi, zi);
                        fill(g, &mut e, wm, &mut filled, x, yi, zi);
                    }
                }
                let hlabel = if label == Left_Lateral_Ventricle || label == Left_Inf_Lat_Vent {
                    Left_Hippocampus
                } else {
                    Right_Hippocampus
                };
                if distance_to_label(g, err, seg, hlabel, x, y, z, 0, 1, 0, 10)? < 10 {
                    continue;
                }
                if nl(g, err, Left_Cerebral_Cortex)?
                    && nl(g, err, Right_Cerebral_Cortex)?
                    && !on(g, wm, x, y, z)
                {
                    fill(g, &mut e, wm, &mut filled, x, y, z);
                }
                yi = ix(g, wm, Ax::Y, y + 1);
                label = seg.label(g, err, x, yi, z)?;
                if (label == Left_Cerebral_Cortex
                    || label == Right_Cerebral_Cortex
                    || label == Left_Cerebral_White_Matter
                    || label == Right_Cerebral_White_Matter
                    || label == Unknown)
                    && !on(g, wm, x, yi, z)
                {
                    fill(g, &mut e, wm, &mut filled, x, yi, z);
                }
                if label == Left_Inf_Lat_Vent || label == Right_Inf_Lat_Vent {
                    // the same voxel read again: never cortex or white matter
                    yi = ix(g, wm, Ax::Y, y + 1);
                    label = seg.label(g, err, x, yi, z)?;
                    if (label == Left_Cerebral_Cortex
                        || label == Right_Cerebral_Cortex
                        || label == Left_Cerebral_White_Matter
                        || label == Right_Cerebral_White_Matter)
                        && !on(g, wm, x, yi, z)
                    {
                        fill(g, &mut e, wm, &mut filled, x, yi, z);
                        yi = ix(g, wm, Ax::Y, y + 2);
                        if !on(g, wm, x, yi, z) {
                            fill(g, &mut e, wm, &mut filled, x, yi, z);
                        }
                    }
                }
            }
        }
    }

    // the borders of the ventricles, 2 mm thick
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let label = seg.label(g, err, x, y, z)?;
                let nl = |g: &mut Globals, err: &mut dyn Write, w, l| {
                    neighbor_label(g, err, seg, x, y, z, w, l)
                };
                match label {
                    Unknown => {
                        if (nl(g, err, 1, Left_Lateral_Ventricle)?
                            && nl(g, err, 1, Left_Cerebral_White_Matter)?)
                            || (nl(g, err, 1, Right_Lateral_Ventricle)?
                                && nl(g, err, 1, Right_Cerebral_White_Matter)?)
                        {
                            fill(g, &mut e, wm, &mut filled, x, y, z);
                        }
                    }
                    Left_Lateral_Ventricle | Right_Lateral_Ventricle => {
                        let olabel = if label == Left_Lateral_Ventricle {
                            Left_Cerebral_White_Matter
                        } else {
                            Right_Cerebral_White_Matter
                        };
                        if nl(g, err, 2, olabel)? {
                            fill(g, &mut e, wm, &mut filled, x, y, z);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // the segmentation's white matter next to the filled voxels
    loop {
        let mut nchanged = 0;
        for z in 0..depth {
            for y in 0..height {
                for x in 0..width {
                    if vox(g, &filled, x, y, z) == 0 {
                        continue;
                    }
                    for xk in -1..=1 {
                        let xi = ix(g, &filled, Ax::X, x + xk);
                        for yk in -1..=1 {
                            let yi = ix(g, &filled, Ax::Y, y + yk);
                            for zk in -1..=1 {
                                let zi = ix(g, &filled, Ax::Z, z + zk);
                                let label = seg.label(g, err, xi, yi, zi)?;
                                if is_wm(label) && !on(g, wm, xi, yi, zi) {
                                    nchanged += 1;
                                    set_vox(g, wm, xi, yi, zi, AUTO_FILL);
                                    e.non += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        let _ = writeln!(out, "{nchanged} additional wm voxels added");
        if nchanged <= 0 {
            break;
        }
    }

    // voxels with no white matter inferior to the hippocampus: lateral to the amygdala and
    // inferior to white matter
    for z in 1..depth - 1 {
        for y in (1..=height - 2).rev() {
            for x in 2..width - 2 {
                if on(g, wm, x, y, z) {
                    continue;
                }
                let label = seg.label(g, err, x, y, z)?;
                if label == Left_Cerebral_Cortex
                    || label == Right_Cerebral_Cortex
                    || label == Unknown
                {
                    let left = label == Left_Cerebral_Cortex;
                    lateral_to_amygdala(g, err, &mut e, wm, x, y, z, left)?;
                }
            }
        }
    }

    // hippocampal or ventricular voxels with white matter inferior but diagonally connected to
    // voxels that are not
    for z in 0..depth {
        for y in (1..=height - 2).rev() {
            for x in 2..width - 2 {
                if on(g, wm, x, y, z) {
                    continue;
                }
                let label = seg.label(g, err, x, y, z)?;
                let left = match label {
                    Left_Inf_Lat_Vent | Left_Hippocampus | Left_Amygdala => true,
                    Right_Inf_Lat_Vent | Right_Hippocampus | Right_Amygdala => false,
                    _ => continue,
                };
                diagonal_spackle(g, err, &mut e, wm, x, y, z, left)?;
            }
        }
    }

    // spackle the amygdala
    for z in 0..depth {
        for y in 1..height - 1 {
            for x in 2..width - 2 {
                let label = seg.label(g, err, x, y, z)?;
                if label != Left_Cerebral_Cortex && label != Right_Cerebral_Cortex {
                    continue;
                }
                let left = label == Left_Cerebral_Cortex;
                if on(g, wm, x, y, z) {
                    continue;
                }
                if medial_edge_of_hippocampus(g, err, seg, x, y, z, if left { -1 } else { 1 })? {
                    continue;
                }
                lateral_to_amygdala(g, err, &mut e, wm, x, y, z, left)?;
            }
        }
    }

    // cortex below the hippocampus or amygdala, diagonally anterior (+1), then posterior (-1:
    // the amygdala's anterior edge excluded, the hippocampus asked first)
    for (dz, anterior, hippo_first) in [(1, false, false), (-1, true, true)] {
        for z in 1..depth - 1 {
            for y in 1..height - 1 {
                for x in 2..width - 2 {
                    let label = seg.label(g, err, x, y, z)?;
                    if label != Left_Cerebral_Cortex && label != Right_Cerebral_Cortex {
                        continue;
                    }
                    let left = label == Left_Cerebral_Cortex;
                    below_mtl(
                        g,
                        err,
                        &mut e,
                        wm,
                        (x, y, z),
                        left,
                        dz,
                        anterior,
                        hippo_first,
                    )?;
                }
            }
        }
    }

    // unknown voxels diagonally connected to the hippocampus, and the right cortex below it
    for z in 1..depth - 1 {
        for y in 1..height - 1 {
            for x in 2..width - 2 {
                let label = seg.label(g, err, x, y, z)?;
                if label == Unknown {
                    unknown_by_hippocampus(g, err, &mut e, wm, x, y, z)?;
                } else if label == Right_Cerebral_Cortex {
                    // FreeSurfer's case for the right cortex only (`left` stays 0)
                    below_mtl(g, err, &mut e, wm, (x, y, z), false, 1, true, false)?;
                }
            }
        }
    }

    // the segmentation's white matter below the hippocampus
    for z in 1..depth {
        for y in 1..height - 1 {
            for x in 2..width - 2 {
                if on(g, wm, x, y, z) || !is_white_class(seg.get(g, err, x, y, z)?) {
                    continue;
                }
                let label = seg.label(g, err, x, y, z)?;
                if label != Left_Cerebral_White_Matter && label != Right_Cerebral_White_Matter {
                    continue;
                }
                let hlabel = if label == Left_Cerebral_White_Matter {
                    Left_Hippocampus
                } else {
                    Right_Hippocampus
                };
                // any hippocampus superior to this voxel
                let yi = y - 1;
                'search: for xi in x - 1..=x + 1 {
                    for zi in z - 1..=z + 1 {
                        if seg.get(g, err, xi, yi, zi)? == hlabel as f32 {
                            set_vox(g, wm, x, y, z, AUTO_FILL);
                            e.non += 1;
                            break 'search;
                        }
                    }
                }
            }
        }
    }

    // FreeSurfer's last pass (cortex directly lateral to the hippocampus or the inferior lateral
    // ventricle) only reads voxels inside the volume and changes none: not run
    let _ = writeln!(
        out,
        "SEG EDIT: {} voxels turned on, {} voxels turned off.",
        e.non, e.noff
    );
    Ok(())
}

/// A cortex (or unknown) voxel lateral to the amygdala (left `x - 1`, right `x + 1`), with white
/// matter above it and no amygdala within 7 below: turned on.
fn lateral_to_amygdala(
    g: &mut Globals,
    err: &mut dyn Write,
    e: &mut Ed,
    wm: &mut Mri,
    x: i32,
    y: i32,
    z: i32,
    left: bool,
) -> Result<(), Fatal> {
    if vox(g, wm, x, y - 1, z) < WM_MIN_VAL {
        return Ok(());
    }
    let xi = if left { x - 1 } else { x + 1 };
    let olabel = if left { Left_Amygdala } else { Right_Amygdala };
    if e.seg.get(g, err, xi, y, z)? != olabel as f32 {
        return Ok(());
    }
    if distance_to_label(g, err, e.seg, olabel, x, y, z, 0, 1, 0, 8)? < 8 {
        return Ok(());
    }
    e.non += 1;
    set_vox(g, wm, x, y, z, AUTO_FILL);
    Ok(())
}

/// The second spackling of the hippocampus: with white matter below, thin there (its first zero
/// within 2 below), cortex or unknown within 5 below, no white matter within 3 above, no white
/// matter, thalamus or ventral DC within 2 above and not on the hippocampus' medial edge: of each
/// diagonal neighbour below that is off (posterior, anterior, lateral, medial), the brighter
/// voxel of the two turned on. FS-1237: for the `x + 1` neighbour FreeSurfer turns on `x - 1`.
#[allow(clippy::too_many_arguments)]
fn diagonal_spackle(
    g: &mut Globals,
    err: &mut dyn Write,
    e: &mut Ed,
    wm: &mut Mri,
    x: i32,
    y: i32,
    z: i32,
    left: bool,
) -> Result<(), Fatal> {
    let seg = e.seg;
    if vox(g, wm, x, y + 1, z) < WM_MIN_VAL {
        return Ok(());
    }
    if distance_to_wm(g, err, wm, x, y, z, 0, 1, 0, 4, false)? > 2 {
        return Ok(());
    }
    let olabel = if left {
        Left_Cerebral_Cortex
    } else {
        Right_Cerebral_Cortex
    };
    if distance_to_label(g, err, seg, Unknown, x, y, z, 0, 1, 0, 10)? > 5
        && distance_to_label(g, err, seg, olabel, x, y, z, 0, 1, 0, 10)? > 5
    {
        return Ok(());
    }
    if distance_to_wm(g, err, wm, x, y, z, 0, -1, 0, 5, true)? <= 3 {
        return Ok(());
    }
    for l in [
        if left {
            Left_Cerebral_White_Matter
        } else {
            Right_Cerebral_White_Matter
        },
        if left { Left_Thalamus } else { Right_Thalamus },
        if left {
            Left_VentralDC
        } else {
            Right_VentralDC
        },
    ] {
        if distance_to_label(g, err, seg, l, x, y, z, 0, -1, 0, 5)? < 3 {
            return Ok(());
        }
    }
    if medial_edge_of_hippocampus(g, err, seg, x, y, z, if left { -1 } else { 1 })? {
        return Ok(());
    }
    let t1 = e.t1;
    for (nx, ny, nz) in [
        (x, y + 1, z - 1),
        (x, y + 1, z + 1),
        (x - 1, y + 1, z),
        (x + 1, y + 1, z),
    ] {
        if vox(g, wm, nx, ny, nz) >= WM_MIN_VAL {
            continue;
        }
        let (mut fx, mut fy, mut fz) = if vox(g, t1, x, y, z) > vox(g, t1, nx, ny, nz) {
            (x, y, z)
        } else {
            (nx, ny, nz)
        };
        if nx == x + 1 && (fx, fy, fz) == (nx, ny, nz) {
            g.notices.add(
                &FS_1237,
                if g.notices.reproduce() {
                    "mri_edit_wm_with_aseg: the hippocampus' diagonal spackling turns on x - 1 for its x + 1 neighbour, as in FreeSurfer"
                } else {
                    "mri_edit_wm_with_aseg: the hippocampus' diagonal spackling turns on its x + 1 neighbour (FreeSurfer turns on x - 1)"
                },
            );
            if g.notices.reproduce() {
                (fx, fy, fz) = (x - 1, y + 1, z);
            }
        }
        set_vox(g, wm, fx, fy, fz, AUTO_FILL);
        e.non += 1;
    }
    Ok(())
}

/// Cortex with the hippocampus or amygdala diagonally above it (`y - 1`, `z + dz`) that is off,
/// not on the hippocampus' medial edge, (with `anterior`) not the amygdala's anterior edge, at
/// the structures' inferior border (neither within 7 below; `hippo_first`: the hippocampus asked
/// first) and with at least 2 voxels of white matter that are not `AUTO_FILL` around: turned on.
#[allow(clippy::too_many_arguments)]
fn below_mtl(
    g: &mut Globals,
    err: &mut dyn Write,
    e: &mut Ed,
    wm: &mut Mri,
    (x, y, z): (i32, i32, i32),
    left: bool,
    dz: i32,
    anterior: bool,
    hippo_first: bool,
) -> Result<(), Fatal> {
    let seg = e.seg;
    if medial_edge_of_hippocampus(g, err, seg, x, y, z, if left { -1 } else { 1 })? {
        return Ok(());
    }
    if vox(g, wm, x, y - 1, z + dz) >= WM_MIN_VAL {
        return Ok(());
    }
    let alabel = if left { Left_Amygdala } else { Right_Amygdala };
    let hlabel = if left {
        Left_Hippocampus
    } else {
        Right_Hippocampus
    };
    if seg.get(g, err, x, y - 1, z + dz)? != alabel as f32
        && seg.get(g, err, x, y - 1, z + dz)? != hlabel as f32
    {
        return Ok(());
    }
    if anterior
        && seg.get(g, err, x, y - 1, z + dz)? == alabel as f32
        && anterior_edge_of_amygdala(g, err, seg, x, y - 1, z + dz)?
    {
        return Ok(());
    }
    let (first, second) = if hippo_first {
        (hlabel, alabel)
    } else {
        (alabel, hlabel)
    };
    if distance_to_label(g, err, seg, first, x, y, z, 0, 1, 0, 8)? < 8
        || distance_to_label(g, err, seg, second, x, y, z, 0, 1, 0, 8)? < 8
    {
        return Ok(());
    }
    if nonfilled_in_nbhd(g, err, wm, x, y, z, 1)? < 2 {
        return Ok(());
    }
    e.non += 1;
    set_vox(g, wm, x, y, z, AUTO_FILL);
    Ok(())
}

/// An unknown voxel with the hippocampus diagonally above it (left `x - 1` first, else right `x +
/// 1`): unless an unknown voxel within 9 laterally is off in the white matter, the hippocampus is
/// within 7 below, or white matter within 6 below, the hippocampus voxel turned on.
fn unknown_by_hippocampus(
    g: &mut Globals,
    err: &mut dyn Write,
    e: &mut Ed,
    wm: &mut Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<(), Fatal> {
    let seg = e.seg;
    let (xi, left, hlabel) = if seg.get(g, err, x - 1, y - 1, z)? == Left_Hippocampus as f32 {
        (x - 1, true, Left_Hippocampus)
    } else if seg.get(g, err, x + 1, y - 1, z)? != Right_Hippocampus as f32 {
        return Ok(());
    } else {
        (x + 1, false, Right_Hippocampus)
    };
    let dx = if left { -1 } else { 1 };
    let i = distance_to_label(g, err, seg, Unknown, x, y, z, dx, 0, 0, 10)?;
    if i < 10 && vox(g, wm, x + dx * i, y, z) < WM_MIN_VAL {
        return Ok(());
    }
    if distance_to_label(g, err, seg, hlabel, x, y, z, 0, 1, 0, 8)? < 8 {
        return Ok(());
    }
    if distance_to_wm(g, err, wm, x, y, z, 0, 1, 0, 8, true)? < 7 {
        return Ok(());
    }
    e.non += 1;
    set_vox(g, wm, xi, y - 1, z, AUTO_FILL);
    Ok(())
}
