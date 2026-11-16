// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRIinterpolateIntoVolume` (`utils/mri.cpp` 9573) and `MRIinterpolateIntoVolumeFrame` (9577)
//! of FreeSurfer 8.2.0 (track A's relay 38 of FreeSurfer's foundations: `GCAMinvert`, fs.gca,
//! calls them): a value spread over the eight voxels around a point with trilinear weights and
//! added to them, the volume's own type (FreeSurfer's direct voxel access).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_UNSUPPORTED, NO_ERROR};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri,
};

use crate::coords::index_not_in_volume;
use crate::rows::{self, Num};

/// `MRIinterpolateIntoVolume(mri, x, y, z, val)`: [`mri_interpolate_into_volume_frame`] of frame 0.
#[allow(clippy::too_many_arguments)]
pub fn mri_interpolate_into_volume(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    x: f64,
    y: f64,
    z: f64,
    val: f64,
) -> i32 {
    mri_interpolate_into_volume_frame(g, err, mri, x, y, z, 0, val)
}

/// `MRIinterpolateIntoVolumeFrame(mri, x, y, z, frame, val)`: nothing for a point plainly outside
/// the volume (`MRIindexNotInVolume` 1, NaN included); else the point clamped to `[0, extent - 1]`
/// on each axis, its lower corner `(int)` of each coordinate and the upper one the next voxel
/// (the last at the edge, so both corners may be the same voxel), the distances `d = x -
/// (float)xm` and `1 - d` in `double`, and to each of the eight voxels, in the order (xm, ym, zm),
/// (xm, ym, zp), (xm, yp, zm), (xm, yp, zp), (xp, ym, zm), ... (xp, yp, zp), the product of its
/// three weights and `val` (`((wx wy) wz) val` in `double`) added: a float voxel in `double`
/// (rounded back to `float`), an integer voxel `nint` of the product in the type's own wrapping
/// arithmetic. Another type: `MRIsampleVolume: unsupported type <t>` and `ERROR_UNSUPPORTED`.
#[allow(clippy::too_many_arguments)]
pub fn mri_interpolate_into_volume_frame(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    mut x: f64,
    mut y: f64,
    mut z: f64,
    frame: i32,
    val: f64,
) -> i32 {
    if index_not_in_volume(mri, x, y, z) == 1 {
        return NO_ERROR;
    }
    let (width, height, depth) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    if x >= f64::from(width) {
        x = f64::from(width) - 1.0;
    }
    if y >= f64::from(height) {
        y = f64::from(height) - 1.0;
    }
    if z >= f64::from(depth) {
        z = f64::from(depth) - 1.0;
    }
    if x < 0.0 {
        x = 0.0;
    }
    if y < 0.0 {
        y = 0.0;
    }
    if z < 0.0 {
        z = 0.0;
    }
    // (int) of a coordinate inside [0, extent - 1]
    let xm = (x as i32).max(0);
    let xp = (width - 1).min(xm + 1);
    let ym = (y as i32).max(0);
    let yp = (height - 1).min(ym + 1);
    let zm = (z as i32).max(0);
    let zp = (depth - 1).min(zm + 1);
    let xmd = x - f64::from(xm as f32);
    let ymd = y - f64::from(ym as f32);
    let zmd = z - f64::from(zm as f32);
    let xpd = 1.0 - xmd;
    let ypd = 1.0 - ymd;
    let zpd = 1.0 - zmd;
    let ty = mri.mtype;
    if !matches!(
        ty,
        MRI_UCHAR | MRI_FLOAT | MRI_SHORT | MRI_USHRT | MRI_INT | MRI_LONG
    ) {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("MRIsampleVolume: unsupported type {ty}"),
        );
        return ERROR_UNSUPPORTED;
    }
    let corners = [
        (xm, ym, zm, xpd * ypd * zpd * val),
        (xm, ym, zp, xpd * ypd * zmd * val),
        (xm, yp, zm, xpd * ymd * zpd * val),
        (xm, yp, zp, xpd * ymd * zmd * val),
        (xp, ym, zm, xmd * ypd * zpd * val),
        (xp, ym, zp, xmd * ypd * zmd * val),
        (xp, yp, zm, xmd * ymd * zpd * val),
        (xp, yp, zp, xmd * ymd * zmd * val),
    ];
    const WHAT: &str = "MRIinterpolateIntoVolumeFrame";
    for (cx, cy, cz, w) in corners {
        let sl = i64::from(cz) + i64::from(frame) * i64::from(depth);
        let (sy, sx) = (i64::from(cy), i64::from(cx));
        let old = rows::read(g, WHAT, mri, sl, sy, sx, ty);
        let new = match old {
            Num::F(f) => Num::F((f64::from(f) + w) as f32),
            Num::I(i) => {
                let add = i64::from(nint(g, w));
                // the type's own arithmetic: the sum converted back to the voxel's type (its low
                // bits), an int's sum wrapping
                Num::I(match ty {
                    MRI_INT => i64::from((i as i32).wrapping_add(add as i32)),
                    MRI_LONG => i.wrapping_add(add),
                    _ => i + add,
                })
            }
        };
        rows::write(g, WHAT, mri, sl, sy, sx, ty, new);
    }
    NO_ERROR
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;
    use larmorx_freesurfer_mri::mri::mri_alloc;
    use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

    #[test]
    fn weights_sum_to_the_value_and_edges_collapse() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut m = mri_alloc(&mut g, &mut o, &mut e, 3, 3, 3, MRI_FLOAT).unwrap();
        mri_interpolate_into_volume(&mut g, &mut e, &mut m, 0.5, 1.0, 2.0, 8.0);
        let v = |g: &mut Globals, m: &Mri, x, y, z| {
            mri_get_vox_val(g, &mut Vec::new(), m, x, y, z, 0, 0).unwrap()
        };
        assert_eq!(v(&mut g, &m, 0, 1, 2), 4.0);
        assert_eq!(v(&mut g, &m, 1, 1, 2), 4.0);
        // outside: nothing
        mri_interpolate_into_volume(&mut g, &mut e, &mut m, -3.0, 0.0, 0.0, 8.0);
        mri_interpolate_into_volume(&mut g, &mut e, &mut m, f64::NAN, 0.0, 0.0, 8.0);
        assert_eq!(v(&mut g, &m, 0, 0, 0), 0.0);
        // a byte volume wraps
        let mut b = mri_alloc(&mut g, &mut o, &mut e, 2, 2, 2, MRI_UCHAR).unwrap();
        for _ in 0..3 {
            mri_interpolate_into_volume(&mut g, &mut e, &mut b, 0.0, 0.0, 0.0, 100.0);
        }
        assert_eq!(v(&mut g, &b, 0, 0, 0), 44.0);
    }
}
