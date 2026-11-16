// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The conversions `mri_read` and `MRIwriteType` call that live in crates above fs.io (larmorx's
//! own plumbing; FreeSurfer links its whole library into every program, so these calls are plain
//! function calls there).
//!
//! FreeSurfer's `utils/mriio.cpp` reads an image file as a volume through `ImageToMRI` and writes
//! one through `MRItoImage` (`utils/mri.cpp`, ported in fs.volume, which sits above fs.io), and
//! reads a GCA atlas as a volume through `readGCA` and a GCA morph through `GCAMread`, and writes a
//! three-frame volume as a morph through `GCAMwrite` (their GCA functions in fs.gca). The
//! crate that holds a conversion registers it here once ([`register`]; idempotent), and every
//! program that links that crate calls its registration before it runs (the `larmorx-freesurfer`
//! program does so for every crate it links; the fs.io probe and its tests too). A conversion that
//! is not registered makes `mri_read` and `MRIwriteType` say so and fail (`larmorx-freesurfer:
//! ... needs <crate>'s <function>, which this program does not link`).

use std::io::Write;
use std::path::Path;
use std::sync::RwLock;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::mri::Mri;

use crate::image::Image;

/// `ImageToMRI(I)` (fs.volume): the volume of an image.
pub type ImageToMri =
    fn(&mut Globals, &mut dyn Write, &mut dyn Write, &mut Image) -> Result<Mri, Fatal>;

/// `MRItoImage(mri, NULL, slice)` (fs.volume): a slice as a new image, or `None` (FreeSurfer's
/// `NULL`, after its message).
pub type MriToImage = fn(&mut Globals, &mut dyn Write, &Mri, i32) -> Result<Option<Image>, Fatal>;

/// `readGCA(fname, start_frame, end_frame)` (fs.gca): a GCA atlas as a volume, or `None`
/// (FreeSurfer's `NULL`, after its messages).
pub type ReadGca = fn(
    &mut Globals,
    &mut dyn Write,
    &mut dyn Write,
    Option<&Path>,
    &str,
    i32,
    i32,
) -> Result<Option<Mri>, Fatal>;

/// `mri_read`'s branch of type `MGH_MORPH` (fs.gca: `GCAMread` and the volume of a GCA morph):
/// the volume, or `None` (after FreeSurfer's messages), and the start and end frames `mri_read`
/// goes on with.
pub type ReadMorph = fn(
    &mut Globals,
    &mut dyn Write,
    &mut dyn Write,
    &Environment,
    Option<&Path>,
    &str,
    i32,
    i32,
) -> Result<(Option<Mri>, i32, i32), Fatal>;

/// `MRIwriteType`'s branch of type `MGH_MORPH` (fs.gca: a GCA morph made from a three-frame volume
/// and written by `GCAMwrite`): the status `MRIwriteType` returns.
pub type WriteMorph = fn(
    &mut Globals,
    &mut dyn Write,
    &mut dyn Write,
    &Environment,
    Option<&Path>,
    &Mri,
    &str,
) -> Result<i32, Fatal>;

/// The registered conversions.
#[derive(Clone, Copy, Default)]
pub struct Upper {
    pub image_to_mri: Option<ImageToMri>,
    pub mri_to_image: Option<MriToImage>,
    pub read_gca: Option<ReadGca>,
    pub read_morph: Option<ReadMorph>,
    pub write_morph: Option<WriteMorph>,
}

static UPPER: RwLock<Upper> = RwLock::new(Upper {
    image_to_mri: None,
    mri_to_image: None,
    read_gca: None,
    read_morph: None,
    write_morph: None,
});

/// Registers conversions: `f` sets the fields its crate provides.
pub fn register(f: impl FnOnce(&mut Upper)) {
    let mut u = UPPER.write().unwrap_or_else(|e| e.into_inner());
    f(&mut u);
}

/// The registered conversions.
pub fn get() -> Upper {
    *UPPER.read().unwrap_or_else(|e| e.into_inner())
}

/// The message of a conversion that is not registered.
pub fn missing(err: &mut dyn Write, what: &str, krate: &str, function: &str, name: &str) {
    let _ = writeln!(
        err,
        "larmorx-freesurfer: {what} needs {krate}'s {function}, which this program does not link ({name})"
    );
}
