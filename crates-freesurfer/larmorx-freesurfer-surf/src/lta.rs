// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRIS::lta`'s type (`LTA *`, `include/transform.h`'s `LINEAR_TRANSFORM_ARRAY`) and the two
//! things a surface does with it: `LTAcopy(lta, NULL)` in `MRIScopyMetadata` and `LTAfree` in
//! `MRISdtr`.
//!
//! **A placeholder** (the orchestrator's decision of 2026-11-13): FreeSurfer's `LTA` structure is
//! fs.volume's (`utils/transform.cpp`, queue item 3.1-40), which track A brings into
//! `larmorx-freesurfer-mri` early. Until then [`Lta`] holds the fields a surface's copy and free
//! carry over (the number of transforms, the type, the scale and the subject); the transforms
//! themselves are not here. When the structure arrives this module re-exports it with its
//! `LTAcopy` and `LTAfree` instead, and nothing else in the crate changes.

/// `LTA` (placeholder: the fields named in the module's documentation).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lta {
    pub num_xforms: i32,
    pub r#type: i32,
    pub fscale: f32,
    /// `subject[STRLEN]` (the bytes before the NUL).
    pub subject: Vec<u8>,
}

/// `LTAcopy(lta, NULL)`: a new transform array with as many transforms as `lta`, its type, scale,
/// subject and transforms.
pub fn lta_copy(lta: &Lta) -> Lta {
    lta.clone()
}

/// `LTAfree(&lta)`: the array freed, `lta` `NULL`.
pub fn lta_free(lta: &mut Option<Lta>) {
    *lta = None;
}
