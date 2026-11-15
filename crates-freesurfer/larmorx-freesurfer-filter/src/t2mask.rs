// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRImask_with_T2_and_aparc_aseg` (`utils/mri2.cpp` line 4000; fs.volume's file, here because it
//! needs fs.filter's `GetLargestCC6`): the T2 mask `mri_em_register`, `mri_ca_register` and
//! `mri_ca_normalize` apply with `-T2MASK` and `-AMASK` (their `MM_FROM_EXTERIOR` is 5).
//!
//! The T2 volume binarized (`MRIbinarize(T2, NULL, thresh, 255, 0)`: 255 below the threshold) is the
//! dark mask, and a copy of it is made and freed unused (`mri_bright`). With an aparc+aseg, that
//! volume is changed in place: binarized at 1 (`0` below, `255` from 1), dilated, inverted
//! (`MRInot`), reduced to its largest face-connected region (the exterior); the dark mask is
//! `MRIand`ed with it (threshold 1, into the mask) and opened three times (`MRIopenN(.., 3)`).
//! Without one, the dark mask is reduced to its largest face-connected region. The mask is then
//! inverted (0 marks what is erased). Each of `mm_from_exterior` passes copies the mask (the first
//! into a new volume), and for every voxel (x outermost, then y, then z) still kept, outside the
//! aparc+aseg's brain (its value not 0), as bright as the threshold in the T2 (`>=` in `float`)
//! and with a face neighbour erased (`MRIneighborsOff(mask, x, y, z, 1)`, the mask's bytes) erases
//! it in the copy; `<n> T2-bright exterior voxels removed` (the count over all passes so far) on
//! standard output; the copy back into the mask. Last, `MRImask(dst, mask, dst, 0, 0)` and the
//! three work volumes freed.
//!
//! FS-615 (no aparc+aseg: FreeSurfer crashes in the passes), FS-616 (a destination other than the
//! source: FreeSurfer masks the destination), FS-617 (no pass: `MRIfree: null pointer`), FS-618
//! (no destination: FreeSurfer crashes at the end).

use std::io::Write;

use larmorx_core::defects::{FS_615, FS_616, FS_617, FS_618};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{Mri, mri_free};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, Tri, mri_binarize};
use larmorx_freesurfer_volume::deriv::neighbors_off;
use larmorx_freesurfer_volume::mriset::{mri_and, mri_dilate, mri_mask, mri_not, mri_open_n};

use crate::cc::get_largest_cc6;

const WHAT: &str = "MRImask_with_T2_and_aparc_aseg";

/// `MRImask_with_T2_and_aparc_aseg(mri_src, mri_dst, mri_T2, mri_aparc_aseg, T2_thresh,
/// mm_from_exterior)`: the destination masked where the T2 mask erases (see the module), the
/// aparc+aseg changed in place. `Pair::Same` is FreeSurfer's every call. `Pair::Given`: the source
/// masked into the destination (FS-616 with `reproduce`: the destination's own voxels);
/// `Pair::New`: a new volume (FS-618 with `reproduce`: FreeSurfer's crash, an error). Without an
/// aparc+aseg the passes skip its test (FS-615 with `reproduce`: the crash, an error).
#[allow(clippy::too_many_arguments)]
pub fn mri_mask_with_t2_and_aparc_aseg(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    p: Pair,
    mri_t2: &Mri,
    mut mri_aparc_aseg: Option<&mut Mri>,
    t2_thresh: f32,
    mm_from_exterior: i32,
) -> Result<Ret<Mri>, Fatal> {
    let mut mask =
        mri_binarize(g, out, err, Pair::New(mri_t2), t2_thresh, 255.0, 0.0)?.unwrap_new();
    let bright = mri_copy(g, out, err, &mask, None)?;
    if let Some(ap) = mri_aparc_aseg.as_deref_mut() {
        mri_binarize(g, out, err, Pair::Same(ap), 1.0, 0.0, 255.0)?;
        mri_dilate(g, out, err, Pair::Same(ap))?;
        mri_not(g, out, err, Pair::Same(ap))?;
        get_largest_cc6(g, out, err, ap)?;
        mri_and(g, out, err, Tri::First(&mut mask, ap), 1)?;
        mri_open_n(g, out, err, Pair::Same(&mut mask), 3)?;
    } else {
        get_largest_cc6(g, out, err, &mut mask)?;
    }
    mri_not(g, out, err, Pair::Same(&mut mask))?;

    let mut tmp: Option<Mri> = None;
    let mut nremoved: i32 = 0;
    let mut noted = false;
    for _ in 0..mm_from_exterior {
        match mri_copy(g, out, err, &mask, tmp.as_mut())? {
            Ret::New(m) => tmp = Some(m),
            Ret::Given => {}
            Ret::Null => tmp = None,
        }
        let (w, h, d) = (mask.vg.width, mask.vg.height, mask.vg.depth);
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    if mri_get_vox_val(g, err, &mask, x, y, z, 0, 0)? == 0.0 {
                        continue;
                    }
                    match mri_aparc_aseg.as_deref() {
                        Some(ap) => {
                            if mri_get_vox_val(g, err, ap, x, y, z, 0, 0)? == 0.0 {
                                continue;
                            }
                        }
                        None => {
                            if !noted {
                                noted = true;
                                g.notices.add(
                                    &FS_615,
                                    format!(
                                        "{WHAT} without an aparc+aseg: its pass reads it at voxel \
                                         ({x}, {y}, {z})"
                                    ),
                                );
                            }
                            if g.notices.reproduce() {
                                log::error_exit(
                                    g,
                                    err,
                                    ERROR_BADPARM,
                                    &format!(
                                        "{WHAT}(): no aparc+aseg for the bright exterior voxels \
                                         (FreeSurfer crashes here)\n"
                                    ),
                                )?;
                                return Ok(Ret::Null);
                            }
                        }
                    }
                    if mri_get_vox_val(g, err, mri_t2, x, y, z, 0, 0)? >= t2_thresh
                        && neighbors_off(g, &mask, x, y, z, 1) > 0
                    {
                        if let Some(t) = tmp.as_mut() {
                            mri_set_vox_val(g, err, t, x, y, z, 0, 0.0, 0)?;
                        }
                        nremoved = nremoved.wrapping_add(1);
                    }
                }
            }
        }
        let _ = writeln!(out, "{nremoved} T2-bright exterior voxels removed");
        if let Some(t) = tmp.as_ref() {
            mri_copy(g, out, err, t, Some(&mut mask))?;
        }
    }

    let r = match p {
        Pair::Same(dst) => mri_mask(g, out, err, env, Pair::Same(dst), &mask, 0, 0.0)?,
        Pair::Given(src, dst) => {
            g.notices.add(
                &FS_616,
                format!("{WHAT} with a destination other than its source"),
            );
            if g.notices.reproduce() {
                mri_mask(g, out, err, env, Pair::Same(dst), &mask, 0, 0.0)?
            } else {
                mri_mask(g, out, err, env, Pair::Given(src, dst), &mask, 0, 0.0)?
            }
        }
        Pair::New(src) => {
            g.notices
                .add(&FS_618, format!("{WHAT} without a destination"));
            if g.notices.reproduce() {
                log::error_exit(
                    g,
                    err,
                    ERROR_BADPARM,
                    &format!("{WHAT}(): no destination (FreeSurfer crashes here)\n"),
                )?;
                return Ok(Ret::Null);
            }
            mri_mask(g, out, err, env, Pair::New(src), &mask, 0, 0.0)?
        }
    };
    drop(bright);
    let mut m = Some(mask);
    mri_free(g, err, &mut m);
    if tmp.is_none() {
        g.notices.add(
            &FS_617,
            format!("{WHAT} with {mm_from_exterior} passes: 'MRIfree: null pointer'"),
        );
        if g.notices.reproduce() {
            mri_free(g, err, &mut tmp);
        }
    }
    Ok(r)
}
