// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mriflood.cpp`'s volume functions (queue item 3.1-43): `MRISfloodoutside` (253),
//! `MRISaccentuate` (433) and `distance` (1129). The surface functions of the file
//! (`MRISshell`, `MRISribbon`, `MRISmakeBoundingVolume`, `MRISfillInterior`) wait for the surface
//! library's voxel maps (`mrisurf_mri.cpp`, queue item 3.4-02) and `mriset.cpp`'s
//! `MRIcomplement` and `MRIintersect`.

use std::io::Write;

use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, mul, sqrt};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{Mri, mri_alloc, mri_copy_header};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::Pair;

/// `FILL_VAL`: the value of the flooded voxels.
const FILL_VAL: i32 = 1;

/// The destination of a [`Pair`]: the given volume or the source itself, or a new one made by
/// `make` from the source.
fn dest<'a>(
    p: Pair<'a>,
    fresh: &'a mut Option<Mri>,
    make: impl FnOnce(&Mri) -> Result<Mri, Fatal>,
) -> Result<(Option<&'a Mri>, &'a mut Mri), Fatal> {
    Ok(match p {
        Pair::New(s) => (Some(s), fresh.insert(make(s)?)),
        Pair::Given(s, d) => (Some(s), d),
        Pair::Same(d) => (None, d),
    })
}

/// `MRISfloodoutside(mri_src, mri_dst)`: the source copied into the destination (`MRIcopy`), the
/// eight corners set to 1, then sweeps until one fills nothing: eight passes, one from each
/// corner (columns outermost, then rows, then slices, each from its corner's side), each setting
/// to 1 every voxel whose value (truncated to `int`, FS-17 beyond `int`) is 0 and whose neighbour
/// towards the corner along any axis (the voxel itself at the volume's edge) is 1. Values other
/// than 0 and 1 are walls.
pub fn mris_flood_outside(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let given = matches!(p, Pair::Given(..));
    let d: &mut Mri = match p {
        Pair::New(s) => fresh.insert(mri_copy(g, out, err, s, None)?.unwrap_new()),
        Pair::Given(s, d) => {
            mri_copy(g, out, err, s, Some(&mut *d))?;
            d
        }
        Pair::Same(d) => d,
    };
    let (w, h, dep) = (d.vg.width, d.vg.height, d.vg.depth);
    for (i, j, k) in [
        (0, 0, 0),
        (w - 1, 0, 0),
        (0, h - 1, 0),
        (0, 0, dep - 1),
        (w - 1, h - 1, 0),
        (w - 1, 0, dep - 1),
        (0, h - 1, dep - 1),
        (w - 1, h - 1, dep - 1),
    ] {
        mri_set_vox_val(g, err, d, i, j, k, 0, FILL_VAL as f32, 0)?;
    }
    // every voxel's (int) value; a voxel set to 1 reads back as 1 in every type
    let (wu, hu) = (w.max(0) as usize, h.max(0) as usize);
    let n = wu * hu * dep.max(0) as usize;
    let mut v = vec![0i32; n];
    let idx = |i: i32, j: i32, k: i32| (k as usize * hu + j as usize) * wu + i as usize;
    for k in 0..dep {
        for j in 0..h {
            for i in 0..w {
                let f = mri_get_vox_val(g, err, d, i, j, k, 0, 0)?;
                v[idx(i, j, k)] = float_to_int(g, f64::from(f), "MRISfloodoutside");
            }
        }
    }
    let up = |a: i32, len: i32| -> Vec<i32> {
        if a > 0 {
            (0..len).collect()
        } else {
            (0..len).rev().collect()
        }
    };
    let mut newfilled = 1;
    while newfilled > 0 {
        newfilled = 0;
        // the corners in FreeSurfer's order: (dir i, dir j, dir k), +1 sweeping up from 0
        for (di, dj, dk) in [
            (1, 1, 1),
            (-1, 1, 1),
            (1, -1, 1),
            (1, 1, -1),
            (-1, -1, 1),
            (-1, 1, -1),
            (1, -1, -1),
            (-1, -1, -1),
        ] {
            let (is, js, ks) = (up(di, w), up(dj, h), up(dk, dep));
            for &i in &is {
                for &j in &js {
                    for &k in &ks {
                        if v[idx(i, j, k)] != 0 {
                            continue;
                        }
                        // the neighbour towards the sweep's corner, or the voxel at the edge
                        let ni = if di > 0 {
                            if i == 0 { i } else { i - 1 }
                        } else if i == w - 1 {
                            i
                        } else {
                            i + 1
                        };
                        let nj = if dj > 0 {
                            if j == 0 { j } else { j - 1 }
                        } else if j == h - 1 {
                            j
                        } else {
                            j + 1
                        };
                        let nk = if dk > 0 {
                            if k == 0 { k } else { k - 1 }
                        } else if k == dep - 1 {
                            k
                        } else {
                            k + 1
                        };
                        if v[idx(ni, j, k)] == FILL_VAL
                            || v[idx(i, nj, k)] == FILL_VAL
                            || v[idx(i, j, nk)] == FILL_VAL
                        {
                            mri_set_vox_val(g, err, d, i, j, k, 0, FILL_VAL as f32, 0)?;
                            v[idx(i, j, k)] = FILL_VAL;
                            newfilled += 1;
                        }
                    }
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) if !given => Ret::New(m),
        _ => Ret::Given,
    })
}

/// `MRISaccentuate(mri_src, mri_dst, lo_thresh, hi_thresh)`: frame 0 of each voxel set to 255
/// where the source's value truncated to `int` (FS-17 beyond `int`) is within `[lo_thresh,
/// hi_thresh]`, 0 elsewhere; a new destination is `MRIalloc` of the source's extent and type with
/// its header.
pub fn mris_accentuate(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    lo_thresh: i32,
    hi_thresh: i32,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let given = matches!(p, Pair::Given(..));
    let (src, d) = {
        let mut make = |s: &Mri| -> Result<Mri, Fatal> {
            let mut m = mri_alloc(g, out, err, s.vg.width, s.vg.height, s.vg.depth, s.mtype)?;
            mri_copy_header(g, out, err, s, Some(&mut m))?;
            Ok(m)
        };
        dest(p, &mut fresh, &mut make)?
    };
    let (w, h, dep) = (d.vg.width, d.vg.height, d.vg.depth);
    let (w, h, dep) = match src {
        Some(s) => (s.vg.width, s.vg.height, s.vg.depth),
        None => (w, h, dep),
    };
    for k in 0..dep {
        for j in 0..h {
            for i in 0..w {
                let f = match src {
                    Some(s) => mri_get_vox_val(g, err, s, i, j, k, 0, 0)?,
                    None => mri_get_vox_val(g, err, d, i, j, k, 0, 0)?,
                };
                let vox = float_to_int(g, f64::from(f), "MRISaccentuate");
                let val = if vox >= lo_thresh && vox <= hi_thresh {
                    255.0
                } else {
                    0.0
                };
                mri_set_vox_val(g, err, d, i, j, k, 0, val, 0)?;
            }
        }
    }
    Ok(match fresh {
        Some(m) if !given => Ret::New(m),
        _ => Ret::Given,
    })
}

/// `distance(x, y, z)`: `sqrtf((x*x + y*y) + z*z)` in `float`.
pub fn distance(x: f32, y: f32, z: f32) -> f32 {
    sqrt(add(add(mul(x, x), mul(y, y)), mul(z, z)))
}
