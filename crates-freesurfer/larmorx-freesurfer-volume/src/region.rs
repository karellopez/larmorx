// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Regions of a volume (`utils/region.cpp`, 3.1-38): `REGIONalloc`, `REGIONclear`, `REGIONcopy`,
//! `REGIONgetBoundingBox`, `REGIONgetBoundingBoxM`, `REGIONgetBoundingBoxEqOdd` and `REGIONprint`.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §16):
//! - **FS-269** (measured 2026-11-23; rules A and C): the bounding boxes' size is `max - min + 2
//!   npad` (`REGIONgetBoundingBoxM`: `+ npad[lo] + npad[hi]`), one voxel less than the voxels from
//!   `min - npad` to `max + npad` the box should hold. `REGIONgetBoundingBox`: where the pad after
//!   the mask is 0 or less (and the start is not clamped at 0), the mask's last voxels on that
//!   axis are left out (rule A): by default that axis's size is one more (kept in the volume as
//!   FreeSurfer keeps it); `reproduce` FreeSurfer's; noted. With a pad of 1 or more after it,
//!   FreeSurfer's box holds the whole mask and only its last padding voxel (outside the mask) is
//!   missing: FreeSurfer's box in both modes (rule C: recon-all's v8 SynthMorph crop, `mri_mask
//!   -bb 3` of `orig.mgz`, would change on every recorded head). `REGIONgetBoundingBoxM` (whose
//!   one caller is `mri_mask`'s `-bb`, `-crop`, `-crop-crs` and `-BBM`) keeps FreeSurfer's box in
//!   both modes for every pad (rule C), noted where it leaves out mask voxels: recon-all's v8
//!   `mri_vsinus_seg` places its network's 144^3 crop at the centre of `mri_mask -T .001 -crop 0`'s
//!   box, and the corrected box moved that crop by one voxel on every recorded head, changing
//!   553 to 1,105 voxels of `vsinus.mgz` (a network trained on crops placed by FreeSurfer's box).
//!   A mask with no voxel keeps FreeSurfer's box (from 1000000, of a negative size) in both modes.

use std::io::Write;

use larmorx_core::defects::FS_269;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_mri::mri::{Mri, MriRegion};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

/// `FZERO`'s threshold in `region.cpp`: FreeSurfer's `macros.h` defines `FLT_EPSILON` as `1e-5`
/// when no `<float.h>` came before it, as in this file (the comparison in `double`).
const ZERO: f64 = 1e-5;

/// `REGIONalloc()`: a region of zeros.
pub fn region_alloc() -> MriRegion {
    MriRegion::default()
}

/// `REGIONgetBoundingBox(mask, npad)`: the smallest box holding every voxel of frame 0 whose
/// value is not within `1e-5` of 0 ([`ZERO`]) (`MRIgetVoxVal`; a NaN counts), widened by `npad` on
/// each side: start `max(min - npad, 0)` per axis, size `min(max - min + 2 npad, extent -
/// start)` (32-bit integer arithmetic), one more by default on an axis where that box ends before
/// the mask's last voxel (FS-269; `reproduce` FreeSurfer's). With no such voxel the minimum is
/// 1000000 and the maximum 0.
pub fn region_get_bounding_box(
    g: &mut Globals,
    err: &mut dyn Write,
    mask: &Mri,
    npad: i32,
) -> Result<MriRegion, Fatal> {
    let (lo, hi) = extremes(g, err, mask)?;
    let npad = [npad; 6];
    Ok(padded_box(g, "REGIONgetBoundingBox", mask, lo, hi, npad, false))
}

/// The box from the extremes `lo`, `hi` with the pads `npad` (before and after the columns, the
/// rows, the slices): start `max(lo - before, 0)`, size `min(hi - lo + before + after, extent -
/// start)` (32-bit integer arithmetic), plus 1 by default on an axis whose box would end before
/// `hi` (FS-269), unless `keep` (FreeSurfer's box in both modes, noted where it leaves out mask
/// voxels: [`region_get_bounding_box_m`]).
fn padded_box(
    g: &mut Globals,
    what: &str,
    mask: &Mri,
    lo: [i32; 3],
    hi: [i32; 3],
    npad: [i32; 6],
    keep: bool,
) -> MriRegion {
    let extent = [mask.vg.width, mask.vg.height, mask.vg.depth];
    let start: [i32; 3] = std::array::from_fn(|a| lo[a].wrapping_sub(npad[2 * a]).max(0));
    // FreeSurfer's size and the size that holds every voxel from lo - before to hi + after
    let size = |a: usize, one: i32| {
        hi[a]
            .wrapping_sub(lo[a])
            .wrapping_add(one)
            .wrapping_add(npad[2 * a])
            .wrapping_add(npad[2 * a + 1])
            .min(extent[a].wrapping_sub(start[a]))
    };
    let fs: [i32; 3] = std::array::from_fn(|a| size(a, 0));
    let any = lo[0] <= hi[0];
    // FS-269: FreeSurfer's box ends one voxel before `hi + after`; the box is widened only on an
    // axis where that end falls before the mask's last voxel (a pad after it of 0 or less): there
    // mask voxels are left out (rule A). With a pad after it of 1 or more FreeSurfer's box holds
    // every voxel of the mask and only its last padding voxel (outside the mask) is missing: kept
    // in both modes (rule C: recon-all's SynthMorph crop, `mri_mask -bb 3`, relies on it).
    let cuts = |a: usize| {
        any && i64::from(start[a]) + i64::from(fs[a]) - 1 < i64::from(hi[a]) && size(a, 1) != fs[a]
    };
    let full: [i32; 3] = std::array::from_fn(|a| if cuts(a) { size(a, 1) } else { fs[a] });
    if full != fs {
        let kept = if keep {
            ", FreeSurfer's kept in both modes"
        } else {
            ""
        };
        g.notices.add(
            &FS_269,
            format!(
                "{what}: the box leaves out the mask's last voxel on an axis (FreeSurfer's size {} {} {}, the voxels' {} {} {}{kept})",
                fs[0], fs[1], fs[2], full[0], full[1], full[2]
            ),
        );
    }
    let d = if keep || g.notices.reproduce() {
        fs
    } else {
        full
    };
    let mut region = region_alloc();
    region.x = start[0];
    region.y = start[1];
    region.z = start[2];
    region.dx = d[0];
    region.dy = d[1];
    region.dz = d[2];
    region
}

/// `REGIONclear(r)`: every field 0.
pub fn region_clear(r: &mut MriRegion) {
    *r = MriRegion::default();
}

/// `REGIONcopy(rsrc, rdst)`: a copy.
pub fn region_copy(rsrc: &MriRegion) -> MriRegion {
    *rsrc
}

/// `REGIONgetBoundingBoxM(mask, npad)`: [`region_get_bounding_box`] with a pad of its own on each
/// side (`npad[0]` before and `npad[1]` after the columns, then the rows, then the slices; the
/// size `max - min + npad[lo] + npad[hi]`): FreeSurfer's box in both modes, noted where it ends
/// before the mask's last voxel (FS-269, rule C: `mri_mask`'s crops place recon-all's v8
/// networks).
pub fn region_get_bounding_box_m(
    g: &mut Globals,
    err: &mut dyn Write,
    mask: &Mri,
    npad: [i32; 6],
) -> Result<MriRegion, Fatal> {
    let (lo, hi) = extremes(g, err, mask)?;
    Ok(padded_box(g, "REGIONgetBoundingBoxM", mask, lo, hi, npad, true))
}

/// The smallest and largest column, row and slice of the voxels of frame 0 not within
/// `1e-5` of 0 (1000000 and 0 when there is none).
fn extremes(
    g: &mut Globals,
    err: &mut dyn Write,
    mask: &Mri,
) -> Result<([i32; 3], [i32; 3]), Fatal> {
    let mut lo = [1_000_000i32; 3];
    let mut hi = [0i32; 3];
    for c in 0..mask.vg.width {
        for r in 0..mask.vg.height {
            for s in 0..mask.vg.depth {
                let v = mri_get_vox_val(g, err, mask, c, r, s, 0, 0)?;
                if f64::from(v.abs()) < ZERO {
                    continue;
                }
                for (k, p) in [c, r, s].into_iter().enumerate() {
                    lo[k] = lo[k].min(p);
                    hi[k] = hi[k].max(p);
                }
            }
        }
    }
    Ok((lo, hi))
}

/// `REGIONgetBoundingBoxEqOdd(mask, npad)`: [`region_get_bounding_box`] (printed: `Volume size:
/// <w> <h> <d>, OrigRegion: ` and the box), then made square in x and y and odd in z for STIR's
/// PET simulator: `dx` above the height or `dy` above the width are errors; an even `dz` grows by
/// one (or starts one slice earlier) if it can (else an error); unequal sides take the larger
/// when it fits, else the smaller side grows by the difference split around it, else
/// asymmetrically against the nearer edge (printed with `xAsymRegion` or `yAsymRegion`). Errors
/// (`ERROR: REGIONgetBoundingBoxEqOdd(): ...` on standard output) give `None`.
pub fn region_get_bounding_box_eq_odd(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mask: &Mri,
    npad: i32,
) -> Result<Option<MriRegion>, Fatal> {
    let mut r = region_get_bounding_box(g, err, mask, npad)?;
    let (w, h, d) = (mask.vg.width, mask.vg.height, mask.vg.depth);
    let isodd = r.dz % 2 != 0;
    let _ = write!(out, "Volume size: {w} {h} {d}, OrigRegion: ");
    region_print(out, &r);
    if r.dx > h {
        let _ = writeln!(
            out,
            "ERROR: REGIONgetBoundingBoxEqOdd(): dx is bigger than MRI height"
        );
        return Ok(None);
    }
    if r.dy > w {
        let _ = writeln!(
            out,
            "ERROR: REGIONgetBoundingBoxEqOdd(): dy is bigger than MRI width"
        );
        return Ok(None);
    }
    if r.dx == r.dy && isodd {
        return Ok(Some(r));
    }
    if !isodd {
        if r.dz < d {
            r.dz = r.dz.wrapping_add(1);
        } else if r.z > 0 {
            r.z = r.z.wrapping_sub(1);
            r.dz = r.dz.wrapping_add(1);
        } else {
            let _ = writeln!(
                out,
                "ERROR: REGIONgetBoundingBoxEqOdd(): cannot make z odd: z={}, dz={}, nz={d}",
                r.z, r.dz
            );
            return Ok(None);
        }
    }
    if r.dx == r.dy {
        return Ok(Some(r));
    }
    let dxy = if r.dx >= r.dy { r.dx } else { r.dy };
    if r.x.wrapping_add(dxy) <= w && r.y.wrapping_add(dxy) <= h {
        r.dx = dxy;
        r.dy = dxy;
        return Ok(Some(r));
    }
    // fabs of the int difference, back to int
    let delta = (f64::from(r.dx) - f64::from(r.dy)).abs() as i32;
    let delta1 = delta / 2;
    let delta2 = delta - delta1;
    if r.dx < r.dy {
        if r.x.wrapping_sub(delta1) > 0 && r.x.wrapping_add(r.dx).wrapping_add(delta2) <= w {
            r.x = r.x.wrapping_sub(delta1);
            r.dx = r.dx.wrapping_add(delta);
            return Ok(Some(r));
        }
        let pad2 = w.wrapping_sub(r.x.wrapping_add(r.dx));
        if r.x < pad2 {
            r.x = 0;
            r.dx = r.dy;
        } else {
            r.dx = r.dy;
            r.x = w.wrapping_sub(r.dx);
        }
        let _ = write!(out, "Volume size: {w} {h} {d}, xAsymRegion: ");
        region_print(out, &r);
        return Ok(Some(r));
    }
    if r.dy < r.dx {
        if r.y.wrapping_sub(delta1) > 0 && r.y.wrapping_add(r.dy).wrapping_add(delta2) <= h {
            r.y = r.y.wrapping_sub(delta1);
            r.dy = r.dy.wrapping_add(delta);
            return Ok(Some(r));
        }
        let pad2 = h.wrapping_sub(r.y.wrapping_add(r.dy));
        if r.y < pad2 {
            r.y = 0;
            r.dy = r.dx;
        } else {
            r.dy = r.dx;
            r.y = h.wrapping_sub(r.dy);
        }
        let _ = write!(out, "Volume size: {w} {h} {d}, yAsymRegion: ");
        region_print(out, &r);
        return Ok(Some(r));
    }
    let _ = writeln!(out, "ERROR: REGIONgetBoundingBoxEqOdd(): cannot make x=y");
    let _ = write!(out, "Volume size: {w} {h} {d}, Region: ");
    region_print(out, &r);
    Ok(None)
}

/// `REGIONprint(fp, r)`: `x y z  dx dy dz` and a newline.
pub fn region_print(fp: &mut dyn Write, r: &MriRegion) {
    let _ = writeln!(fp, "{} {} {}  {} {} {}", r.x, r.y, r.z, r.dx, r.dy, r.dz);
}
