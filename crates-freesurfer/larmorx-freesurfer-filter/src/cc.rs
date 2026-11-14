// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/connectcomp.cpp`'s connected components (queue item 3.1-42), which `mri_fill` uses:
//! `RemoveHoles` (38), `GrassFire6` (180), `GrassFire18` (260), `GetLargestCC6` (340) and
//! `GetLargestCC18` (391). FreeSurfer's own growable list and queue (`myList*`, `myQueue*`) hold the
//! region growing's points; their order changes nothing a caller sees (the region, its size and
//! bounds), so a `VecDeque` stands for them here. Their allocation failure (`myError`: a message and
//! the end of the program) is the machine's matter.

use std::collections::VecDeque;
use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_mri::mri::{MRI_INT, MRI_UCHAR, Mri, mri_alloc, mri_copy_header};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

/// `xoff6`, `yoff6`, `zoff6`: the six face neighbours.
const OFF6: [[i32; 3]; 6] = [
    [1, 0, 0],
    [0, 1, 0],
    [0, 0, 1],
    [-1, 0, 0],
    [0, -1, 0],
    [0, 0, -1],
];

/// `xoff26`, `yoff26`, `zoff26`: the 26 neighbours (faces, then edges, then corners: the first 18
/// are the faces and edges).
const OFF26: [[i32; 3]; 26] = {
    let x = [
        0, 1, 0, 0, -1, 0, 1, 1, -1, -1, 1, 1, -1, -1, 0, 0, 0, 0, -1, -1, 1, 1, -1, -1, 1, 1,
    ];
    let y = [
        1, 0, 0, -1, 0, 0, 1, -1, 1, -1, 0, 0, 0, 0, 1, 1, -1, -1, -1, -1, -1, -1, 1, 1, 1, 1,
    ];
    let z = [
        0, 0, 1, 0, 0, -1, 0, 0, 0, 0, 1, -1, 1, -1, 1, -1, 1, -1, -1, 1, -1, 1, -1, 1, -1, 1,
    ];
    let mut o = [[0; 3]; 26];
    let mut i = 0;
    while i < 26 {
        o[i] = [x[i], y[i], z[i]];
        i += 1;
    }
    o
};

/// A grown region: its size and bounds (`curSize`, `minX` ... `maxZ`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Region {
    pub size: i32,
    pub min: [i32; 3],
    pub max: [i32; 3],
}

/// `GrassFire6` / `GrassFire18`'s region growing over `offs`.
fn grass_fire(
    g: &mut Globals,
    err: &mut dyn Write,
    orivol: &Mri,
    labels: &mut Mri,
    label: i32,
    pt: [i32; 3],
    offs: &[[i32; 3]],
) -> Result<Region, Fatal> {
    let (xn, yn, zn) = (orivol.vg.width, orivol.vg.height, orivol.vg.depth);
    let mut r = Region {
        size: 0,
        min: pt,
        max: pt,
    };
    mri_set_vox_val(g, err, labels, pt[0], pt[1], pt[2], 0, label as f32, 0)?;
    let mut q = VecDeque::from([pt]);
    while let Some(c) = q.pop_front() {
        r.size += 1;
        for k in 0..3 {
            r.min[k] = r.min[k].min(c[k]);
            r.max[k] = r.max[k].max(c[k]);
        }
        for o in offs {
            let n = [c[0] + o[0], c[1] + o[1], c[2] + o[2]];
            if n[1] >= 0 && n[1] < yn && n[0] >= 0 && n[0] < xn && n[2] >= 0 && n[2] < zn {
                let l = mri_get_vox_val(g, err, labels, n[0], n[1], n[2], 0, 0)?;
                if l == 0.0 && mri_get_vox_val(g, err, orivol, n[0], n[1], n[2], 0, 0)? > 0.0 {
                    mri_set_vox_val(g, err, labels, n[0], n[1], n[2], 0, label as f32, 0)?;
                    q.push_back(n);
                }
            }
        }
    }
    Ok(r)
}

/// `GrassFire6(orivol, Label, label, &Pt, ...)`: the face-connected region of voxels above 0 grown
/// from `pt` (which is labelled whatever its value), each voxel of it whose label is 0 given
/// `label`; its size and bounds. The labels are read and written by `MRIgetVoxVal` and
/// `MRIsetVoxVal` (frame 0).
pub fn grass_fire6(
    g: &mut Globals,
    err: &mut dyn Write,
    orivol: &Mri,
    labels: &mut Mri,
    label: i32,
    pt: [i32; 3],
) -> Result<Region, Fatal> {
    grass_fire(g, err, orivol, labels, label, pt, &OFF6)
}

/// `GrassFire18`: as [`grass_fire6`] over the 18 face and edge neighbours.
pub fn grass_fire18(
    g: &mut Globals,
    err: &mut dyn Write,
    orivol: &Mri,
    labels: &mut Mri,
    label: i32,
    pt: [i32; 3],
) -> Result<Region, Fatal> {
    grass_fire(g, err, orivol, labels, label, pt, &OFF26[..18])
}

/// `RemoveHoles(orivol)`: the background (voxels at or below 0, NaN being object) connected by
/// faces to the voxels of the first row of the first slice that are background is kept; every
/// other voxel of the volume is set to 1 (`MRIsetVoxVal`).
pub fn remove_holes(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    orivol: &mut Mri,
) -> Result<(), Fatal> {
    let (xn, yn, zn) = (orivol.vg.width, orivol.vg.height, orivol.vg.depth);
    let mut labels = mri_alloc(g, out, err, xn, yn, zn, MRI_INT)?;
    mri_copy_header(g, out, err, orivol, Some(&mut labels))?;
    let mut tmp = mri_alloc(g, out, err, xn, yn, zn, MRI_UCHAR)?;
    mri_copy_header(g, out, err, orivol, Some(&mut tmp))?;
    for i in 0..yn {
        for j in 0..xn {
            for k in 0..zn {
                mri_set_vox_val(g, err, &mut labels, j, i, k, 0, 0.0, 0)?;
                let v = mri_get_vox_val(g, err, orivol, j, i, k, 0, 0)?;
                let idx = ((k as usize * yn as usize) + i as usize) * xn as usize + j as usize;
                tmp.chunk.as_mut().expect("voxels")[idx] = u8::from(v <= 0.0);
            }
        }
    }
    for j in 0..xn {
        let bg = tmp.chunk.as_ref().expect("voxels")[j as usize] != 0;
        if bg && mri_get_vox_val(g, err, &labels, j, 0, 0, 0, 0)? == 0.0 {
            grass_fire6(g, err, &tmp, &mut labels, 1, [j, 0, 0])?;
        }
    }
    for i in 0..yn {
        for j in 0..xn {
            for k in 0..zn {
                if mri_get_vox_val(g, err, &labels, j, i, k, 0, 0)? == 0.0 {
                    mri_set_vox_val(g, err, orivol, j, i, k, 0, 1.0, 0)?;
                }
            }
        }
    }
    Ok(())
}

/// `GetLargestCC6` / `GetLargestCC18` over `fire`.
fn largest(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    orivol: &mut Mri,
    eighteen: bool,
) -> Result<(), Fatal> {
    let (xn, yn, zn) = (orivol.vg.width, orivol.vg.height, orivol.vg.depth);
    let mut labels = mri_alloc(g, out, err, xn, yn, zn, MRI_INT)?;
    mri_copy_header(g, out, err, orivol, Some(&mut labels))?;
    for i in 0..yn {
        for j in 0..xn {
            for k in 0..zn {
                mri_set_vox_val(g, err, &mut labels, j, i, k, 0, 0.0, 0)?;
            }
        }
    }
    let (mut cur, mut max_size, mut max_label) = (1i32, 0i32, 1i32);
    for i in 0..yn {
        for j in 0..xn {
            for k in 0..zn {
                if mri_get_vox_val(g, err, orivol, j, i, k, 0, 0)? > 0.0
                    && mri_get_vox_val(g, err, &labels, j, i, k, 0, 0)? == 0.0
                {
                    let r = if eighteen {
                        grass_fire18(g, err, orivol, &mut labels, cur, [j, i, k])?
                    } else {
                        grass_fire6(g, err, orivol, &mut labels, cur, [j, i, k])?
                    };
                    if max_size < r.size {
                        max_size = r.size;
                        max_label = cur;
                    }
                    cur = cur.wrapping_add(1);
                }
            }
        }
    }
    for i in 0..yn {
        for j in 0..xn {
            for k in 0..zn {
                if mri_get_vox_val(g, err, &labels, j, i, k, 0, 0)? != max_label as f32 {
                    mri_set_vox_val(g, err, orivol, j, i, k, 0, 0.0, 0)?;
                }
            }
        }
    }
    Ok(())
}

/// `GetLargestCC6(orivol)`: the largest face-connected region of voxels above 0 kept (the first
/// found, scanning rows, then columns, then slices, among equals), every other voxel set to 0.
pub fn get_largest_cc6(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    orivol: &mut Mri,
) -> Result<(), Fatal> {
    largest(g, out, err, orivol, false)
}

/// `GetLargestCC18`: as [`get_largest_cc6`] with face and edge neighbours.
pub fn get_largest_cc18(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    orivol: &mut Mri,
) -> Result<(), Fatal> {
    largest(g, out, err, orivol, true)
}
