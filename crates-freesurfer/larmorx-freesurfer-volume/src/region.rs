// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Regions of a volume (`utils/region.cpp`, 3.1-38): `REGIONalloc`, `REGIONclear`, `REGIONcopy`,
//! `REGIONgetBoundingBox`, `REGIONgetBoundingBoxM`, `REGIONgetBoundingBoxEqOdd` and `REGIONprint`.

use std::io::Write;

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
/// each side: start `max(min - npad, 0)` per axis, size `min(max - min + 2 npad, extent - start)`
/// (32-bit integer arithmetic). With no such voxel the minimum is 1000000 and the maximum 0.
pub fn region_get_bounding_box(
    g: &mut Globals,
    err: &mut dyn Write,
    mask: &Mri,
    npad: i32,
) -> Result<MriRegion, Fatal> {
    let (mut cmin, mut rmin, mut smin) = (1_000_000i32, 1_000_000i32, 1_000_000i32);
    let (mut cmax, mut rmax, mut smax) = (0i32, 0i32, 0i32);
    for c in 0..mask.vg.width {
        for r in 0..mask.vg.height {
            for s in 0..mask.vg.depth {
                let v = mri_get_vox_val(g, err, mask, c, r, s, 0, 0)?;
                if f64::from(v.abs()) < ZERO {
                    continue;
                }
                cmin = cmin.min(c);
                rmin = rmin.min(r);
                smin = smin.min(s);
                cmax = cmax.max(c);
                rmax = rmax.max(r);
                smax = smax.max(s);
            }
        }
    }
    let mut region = region_alloc();
    let two = npad.wrapping_mul(2);
    region.x = cmin.wrapping_sub(npad).max(0);
    region.y = rmin.wrapping_sub(npad).max(0);
    region.z = smin.wrapping_sub(npad).max(0);
    let size = |lo: i32, hi: i32, extent: i32, start: i32| {
        hi.wrapping_sub(lo)
            .wrapping_add(two)
            .min(extent.wrapping_sub(start))
    };
    region.dx = size(cmin, cmax, mask.vg.width, region.x);
    region.dy = size(rmin, rmax, mask.vg.height, region.y);
    region.dz = size(smin, smax, mask.vg.depth, region.z);
    Ok(region)
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
/// side (`npad[0]` before and `npad[1]` after the columns, then the rows, then the slices).
pub fn region_get_bounding_box_m(
    g: &mut Globals,
    err: &mut dyn Write,
    mask: &Mri,
    npad: [i32; 6],
) -> Result<MriRegion, Fatal> {
    let (lo, hi) = extremes(g, err, mask)?;
    let mut region = region_alloc();
    region.x = lo[0].wrapping_sub(npad[0]).max(0);
    region.y = lo[1].wrapping_sub(npad[2]).max(0);
    region.z = lo[2].wrapping_sub(npad[4]).max(0);
    let size = |a: usize, extent: i32, start: i32| {
        hi[a]
            .wrapping_sub(lo[a])
            .wrapping_add(npad[2 * a])
            .wrapping_add(npad[2 * a + 1])
            .min(extent.wrapping_sub(start))
    };
    region.dx = size(0, mask.vg.width, region.x);
    region.dy = size(1, mask.vg.height, region.y);
    region.dz = size(2, mask.vg.depth, region.z);
    Ok(region)
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
