// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Regions of a volume (`utils/region.cpp`, 3.1-38): `REGIONalloc`, `REGIONgetBoundingBox` and
//! `REGIONprint` (taken first, for `MRImaskAndUpsample`).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_mri::mri::{Mri, MriRegion};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

/// `REGIONalloc()`: a region of zeros.
pub fn region_alloc() -> MriRegion {
    MriRegion::default()
}

/// `REGIONgetBoundingBox(mask, npad)`: the smallest box holding every voxel of frame 0 whose
/// value is not within `FLT_EPSILON` of 0 (`MRIgetVoxVal`; a NaN counts), widened by `npad` on
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
                if v.abs() < f32::EPSILON {
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

/// `REGIONprint(fp, r)`: `x y z  dx dy dz` and a newline.
pub fn region_print(fp: &mut dyn Write, r: &MriRegion) {
    let _ = writeln!(fp, "{} {} {}  {} {} {}", r.x, r.y, r.z, r.dx, r.dy, r.dz);
}
