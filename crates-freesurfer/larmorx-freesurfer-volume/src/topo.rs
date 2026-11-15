// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The voxel topology of `utils/mri2.cpp` (3.1-37, relay 29): `QuadEulerCharChange` (5965), the
//! change of the Euler characteristic of a quad mesh tessellated on a binary volume when one voxel
//! is turned on, and the one-layer erosion and dilation that keep it, `MRIvolTopoErodeOne` (5885)
//! and `MRIvolTopoDilateOne` (5917).
//!
//! A voxel is "set" when its value (`MRIgetVoxVal`, frame 0) is above 0.5. Neighbours outside the
//! volume count as unset where FreeSurfer tests the index; the reads it makes without a test go
//! through the voxel functions (their notices for memory outside the volume). `MRIvolTopoFix`,
//! which tessellates surfaces (`MRIStessellate`), belongs above this crate (`fs.surf`).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::Pair;
use crate::labels::int_of;
use crate::mriset::{NEAREST_NEIGHBOR_FACE, mri_dilate6, mri_erode_nn};

/// Whether the voxel is set (`MRIgetVoxVal(...) > 0.5`, the `float` widened).
fn set(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    c: i32,
    r: i32,
    s: i32,
) -> Result<bool, Fatal> {
    Ok(f64::from(mri_get_vox_val(g, err, m, c, r, s, 0, 0)?) > 0.5)
}

/// `QuadEulerCharChange(vol, mask, c, r, s)`: the change of the Euler characteristic of the quad
/// mesh of `vol`'s set voxels if the voxel `(c, r, s)` were turned on (computed as if it were off;
/// its own value is not read). For each of the 26 neighbours (columns outermost, each offset from
/// -1 to 1; with a mask, a neighbour whose mask value is below 0.5 is skipped): a face neighbour
/// that is set removes a face (-1), else a face is added (+1); an edge whose three other voxels
/// are all unset adds an edge (-1), all set loses one (+1); a corner whose seven other voxels are
/// all unset adds a vertex (+1), all set loses one (-1). Voxels outside the volume along a tested
/// axis are unset. When `(c, r, s)` is `(Gx, Gy, Gz)`: `  debug vox c r s`, a line per change
/// (`face`, `edge` or `vertex` with the offsets `%2d` and `added` or `removed dEC=<n>`) and
/// `  debug vox c r s <dEC>` on standard output.
#[allow(clippy::too_many_arguments)]
pub fn quad_euler_char_change(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vol: &Mri,
    mask: Option<&Mri>,
    c: i32,
    r: i32,
    s: i32,
) -> Result<i32, Fatal> {
    let debug = c == g.gx && r == g.gy && s == g.gz;
    if debug {
        let _ = writeln!(out, "  debug vox {c} {r} {s}");
    }
    let (w, h, d) = (vol.vg.width, vol.vg.height, vol.vg.depth);
    let mut dec = 0i32;
    for dc in -1..=1i32 {
        for dr in -1..=1i32 {
            for ds in -1..=1i32 {
                if let Some(m) = mask {
                    let v = mri_get_vox_val(g, err, m, c + dc, r + dr, s + ds, 0, 0)?;
                    if f64::from(v) < 0.5 {
                        continue;
                    }
                }
                let dsum = dc.abs() + dr.abs() + ds.abs();
                if dsum == 0 {
                    continue;
                }
                let mut nhits = 0;
                let coob = c + dc < 0 || c + dc >= w;
                let roob = r + dr < 0 || r + dr >= h;
                let soob = s + ds < 0 || s + ds >= d;
                if dsum == 1 {
                    if !coob && !roob && !soob && set(g, err, vol, c + dc, r + dr, s + ds)? {
                        dec -= 1;
                        if debug {
                            let _ = writeln!(out, "face {dc:2} {dr:2} {ds:2} removed dEC={dec}");
                        }
                        continue;
                    }
                    dec += 1;
                    if debug {
                        let _ = writeln!(out, "face {dc:2} {dr:2} {ds:2} added dEC={dec}");
                    }
                }
                if dsum == 2 {
                    if !coob && !roob && !soob && set(g, err, vol, c + dc, r + dr, s + ds)? {
                        nhits += 1;
                    }
                    if dc == 0 {
                        if !roob && set(g, err, vol, c, r + dr, s)? {
                            nhits += 1;
                        }
                        if !soob && set(g, err, vol, c, r, s + ds)? {
                            nhits += 1;
                        }
                    }
                    if dr == 0 {
                        if !coob && set(g, err, vol, c + dc, r, s)? {
                            nhits += 1;
                        }
                        if !soob && set(g, err, vol, c, r, s + ds)? {
                            nhits += 1;
                        }
                    }
                    if ds == 0 {
                        if !coob && set(g, err, vol, c + dc, r, s)? {
                            nhits += 1;
                        }
                        if !roob && set(g, err, vol, c, r + dr, s)? {
                            nhits += 1;
                        }
                    }
                    if nhits == 0 {
                        dec -= 1;
                        if debug {
                            let _ = writeln!(out, "edge {dc:2} {dr:2} {ds:2} added dEC={dec}");
                        }
                    }
                    if nhits == 3 {
                        dec += 1;
                        if debug {
                            let _ = writeln!(out, "edge {dc:2} {dr:2} {ds:2} removed dEC={dec}");
                        }
                    }
                }
                if dsum == 3 {
                    if !soob && set(g, err, vol, c, r, s + ds)? {
                        nhits += 1;
                    }
                    if !roob && set(g, err, vol, c, r + dr, s)? {
                        nhits += 1;
                    }
                    if !roob && !soob && set(g, err, vol, c, r + dr, s + ds)? {
                        nhits += 1;
                    }
                    if !coob && set(g, err, vol, c + dc, r, s)? {
                        nhits += 1;
                    }
                    if !coob && !soob && set(g, err, vol, c + dc, r, s + ds)? {
                        nhits += 1;
                    }
                    if !coob && !roob && set(g, err, vol, c + dc, r + dr, s)? {
                        nhits += 1;
                    }
                    if !coob && !roob && !soob && set(g, err, vol, c + dc, r + dr, s + ds)? {
                        nhits += 1;
                    }
                    if nhits == 0 {
                        dec += 1;
                        if debug {
                            let _ = writeln!(out, "vertex {dc:2} {dr:2} {ds:2} added dEC={dec}");
                        }
                    }
                    if nhits == 7 {
                        dec -= 1;
                        if debug {
                            let _ =
                                writeln!(out, "vertex {dc:2} {dr:2} {ds:2} removed dEC={dec}");
                        }
                    }
                }
            }
        }
    }
    if debug {
        let _ = writeln!(out, "  debug vox {c} {r} {s} {dec}");
    }
    Ok(dec)
}

/// `(int) MRIgetVoxVal(m, c, r, s, 0)` (C's truncation; FS-12 beyond `int`).
fn int_val(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    m: &Mri,
    c: i32,
    r: i32,
    s: i32,
) -> Result<i32, Fatal> {
    let v = mri_get_vox_val(g, err, m, c, r, s, 0, 0)?;
    Ok(int_of(g, what, f64::from(v)))
}

/// `MRIvolTopoErodeOne(binvol, keepmask)`: one layer of boundary voxels turned off in place where
/// that keeps the Euler characteristic ([`quad_euler_char_change`] 0). The interior voxels (1 to
/// extent - 2 on each axis; columns outermost) that are set (their value truncated to an `int` at
/// least 1), not in the keep mask (its value above 0.5), and on the boundary (off in
/// `MRIerodeNN(binvol, NULL, NEAREST_NEIGHBOR_FACE, 1)`) are visited in order, each decided on the
/// volume as the earlier ones left it; returns how many were turned off (set to 0).
pub fn mri_vol_topo_erode_one(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    binvol: &mut Mri,
    keepmask: Option<&Mri>,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIvolTopoErodeOne";
    let Some(mrie6) = mri_erode_nn(g, out, err, Pair::New(binvol), NEAREST_NEIGHBOR_FACE, 1)?
        .into_new()
    else {
        return Ok(0);
    };
    let mut nhits = 0;
    for c in 1..binvol.vg.width - 1 {
        for r in 1..binvol.vg.height - 1 {
            for s in 1..binvol.vg.depth - 1 {
                if let Some(k) = keepmask
                    && f64::from(mri_get_vox_val(g, err, k, c, r, s, 0, 0)?) > 0.5
                {
                    continue;
                }
                if f64::from(int_val(g, err, WHAT, binvol, c, r, s)?) < 0.5 {
                    continue;
                }
                if f64::from(int_val(g, err, WHAT, &mrie6, c, r, s)?) > 0.5 {
                    continue;
                }
                if quad_euler_char_change(g, out, err, binvol, None, c, r, s)? != 0 {
                    continue;
                }
                mri_set_vox_val(g, err, binvol, c, r, s, 0, 0.0, 0)?;
                nhits += 1;
            }
        }
    }
    Ok(nhits)
}

/// `MRIvolTopoDilateOne(binvol, onval)`: one layer of voxels turned on (to `onval`) in place where
/// that changes the Euler characteristic ([`quad_euler_char_change`] not 0). Every voxel (columns
/// outermost) that is unset (its value truncated to an `int` at most 0) and next to the set ones
/// (on in `MRIdilate6(binvol, NULL)`) is visited in order, each decided on the volume as the
/// earlier ones left it; returns how many were turned on.
pub fn mri_vol_topo_dilate_one(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    binvol: &mut Mri,
    onval: i32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIvolTopoDilateOne";
    let Some(mrid6) = mri_dilate6(g, out, err, Pair::New(binvol))?.into_new() else {
        return Ok(0);
    };
    let mut nhits = 0;
    for c in 0..binvol.vg.width {
        for r in 0..binvol.vg.height {
            for s in 0..binvol.vg.depth {
                if f64::from(int_val(g, err, WHAT, binvol, c, r, s)?) > 0.5 {
                    continue;
                }
                if f64::from(int_val(g, err, WHAT, &mrid6, c, r, s)?) < 0.5 {
                    continue;
                }
                if quad_euler_char_change(g, out, err, binvol, None, c, r, s)? == 0 {
                    continue;
                }
                mri_set_vox_val(g, err, binvol, c, r, s, 0, onval as f32, 0)?;
                nhits += 1;
            }
        }
    }
    Ok(nhits)
}
