// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! One frame of a volume written to a file (`utils/mriio.cpp`'s `MRIwriteFrame`): track A's
//! module of fs.io (3.1-30), here because it needs fs.volume's `MRIcopyFrame`.
//!
//! D13 (larmorx_core::defects): FS-438 (the write's failure not reported).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::FS_438;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, Mri};

use crate::arith::{Pair, mri_copy_frame};

/// `MRIwriteFrame(mri, fname, frame)`: frame `frame` of `mri` copied into a new one-frame volume
/// with its header (`MRIcopyFrame`) and written by `MRIwrite` (the name's format; relative names
/// in `dir`). A frame at or beyond the volume's frames ends the program: `MRIwriteFrame(<frame>) -
/// frame out of bounds (<frames>)` (`ERROR_BADPARM`); a negative one is read as `MRIgetVoxVal`
/// reads it. FS-438: FreeSurfer returns `NO_ERROR` whatever the write gave; by default the write's
/// result is returned.
#[allow(clippy::too_many_arguments)]
pub fn mri_write_frame(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    mri: &Mri,
    fname: &str,
    frame: i32,
) -> Result<i32, Fatal> {
    if frame >= mri.nframes {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MRIwriteFrame({frame}) - frame out of bounds ({})",
                mri.nframes
            ),
        )?;
    }
    let tmp = match mri_copy_frame(g, out, err, Pair::New(mri), frame, 0)? {
        Ret::New(m) => m,
        // not reached: frame 0 of a new one-frame volume is always in range
        _ => return Ok(ERROR_BADPARM),
    };
    let r = mri_write(g, out, err, env, dir, &tmp, fname, None, MGZ_INTENT_UNKNOWN)?;
    if r == NO_ERROR {
        return Ok(NO_ERROR);
    }
    g.notices.add(
        &FS_438,
        format!("one frame of a volume not written to {fname} ({r}): FreeSurfer reports success"),
    );
    Ok(if g.notices.reproduce() { NO_ERROR } else { r })
}
