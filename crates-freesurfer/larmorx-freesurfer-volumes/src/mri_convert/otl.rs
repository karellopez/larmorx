// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mriio.cpp`'s outline reader `MRIreadOtl` (7144) with `read_otl_file` (6556) and the
//! unknown-label helpers (6521-6554), on `cma.cpp`'s outline fields (queue item 3.2-06):
//! `mri_convert`'s `.otl` input.

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_mri::mri::Mri;

use super::options::Run;

/// `READ_OTL_READ_VOLUME_FLAG`: read the outlines (otherwise only the header).
pub const READ_OTL_READ_VOLUME_FLAG: i32 = 0x01;
/// `READ_OTL_FILL_FLAG`: fill the outlines.
pub const READ_OTL_FILL_FLAG: i32 = 0x02;
/// `READ_OTL_TRANSLATE_LABELS_FLAG`: the labels' names translated to the colour table's.
pub const READ_OTL_TRANSLATE_LABELS_FLAG: i32 = 0x04;
/// `READ_OTL_ZERO_OUTLINES_FLAG`: the outlines themselves set to 0.
pub const READ_OTL_ZERO_OUTLINES_FLAG: i32 = 0x08;

/// `MRIreadOtl(fname, width, height, slices, color_file_name, flags)`.
#[allow(clippy::too_many_arguments)]
pub fn mri_read_otl(
    _g: &mut Globals,
    r: &mut Run,
    fname: &str,
    _width: i32,
    _height: i32,
    _slices: i32,
    _color_file_name: &str,
    _flags: i32,
) -> Result<Option<Mri>, Fatal> {
    let _ = writeln!(
        r.err,
        "larmorx-freesurfer: reading outline (.otl) volumes is not ported yet ({fname})"
    );
    Ok(None)
}
