// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrisurf_base.cpp`'s small functions (`fs.surfio`, queue item 3.1-46, lines 80-157;
//! with lines 505-512, the frozen-vertex counter they use): the borrowed temporaries
//! (`MRIS_acquireTemp`, `MRIS_checkAcquiredTemp`, `MRIS_releaseTemp`), the per-vertex floats
//! (`MRISmakeFloatPerVertex`, `MRISfreeFloatPerVertex`), and the file names the surface
//! integrator reads (`MRISsetInflatedFileName` (FS-111), `MRISsetSulcFileName`,
//! `MRISsetOriginalFileName`; the names are [`SurfGlobals`]'s).
//!
//! Not here: the active realm trees' list (`insertActiveRealmTree`, `removeActiveRealmTree`,
//! lines 159-195), which holds fs.surf's `RealmTree`s: with that module (wave 3.4).

use std::io::Write;

use larmorx_core::defects::FS_111;
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;

use crate::aaa::SurfGlobals;
use crate::mris::Mris;

fn base_cpp() -> String {
    format!("{}/utils/mrisurf_base.cpp", diag::BUILD_ROOT)
}

/// `MRIS_TempAssigned`: the vertex fields a function may borrow as temporaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MrisTempAssigned {
    /// `MRIS_TempAssigned_Vertex_marked` (0).
    VertexMarked = 0,
    /// `MRIS_TempAssigned_Vertex_marked2` (1).
    VertexMarked2 = 1,
    /// `MRIS_TempAssigned__end` (2).
    End = 2,
}

impl MrisTempAssigned {
    fn bits(self) -> i32 {
        1 << (self as i32)
    }
}

/// `MRIS_acquireTemp(mris, temp)`: marks the temporary taken in `tempsAssigned` and returns 123;
/// one already taken fails `cheapAssert(!(bits & *tc))` (line 83), which ends the program (FS-03).
pub fn acquire_temp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &mut Mris,
    temp: MrisTempAssigned,
) -> Result<i32, Fatal> {
    let bits = temp.bits();
    if bits & mris.tempsAssigned != 0 {
        return Err(diag::assert_failed(
            g,
            out,
            err,
            &base_cpp(),
            83,
            "!(bits & *tc)",
        ));
    }
    mris.tempsAssigned |= bits;
    Ok(123)
}

/// `MRIS_checkAcquiredTemp(mris, temp, result)`: a temporary not taken fails `cheapAssert((bits
/// & *tc))` (line 92). The result of [`acquire_temp`] is not looked at.
pub fn check_acquired_temp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &Mris,
    temp: MrisTempAssigned,
    _acquire_result: i32,
) -> Result<(), Fatal> {
    if temp.bits() & mris.tempsAssigned == 0 {
        return Err(diag::assert_failed(
            g,
            out,
            err,
            &base_cpp(),
            92,
            "(bits & *tc)",
        ));
    }
    Ok(())
}

/// `MRIS_releaseTemp(mris, temp, result)`: a temporary not taken fails `cheapAssert((bits &
/// *tc))` (line 99); else its mark is cleared.
pub fn release_temp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &mut Mris,
    temp: MrisTempAssigned,
    _acquire_result: i32,
) -> Result<(), Fatal> {
    let bits = temp.bits();
    if bits & mris.tempsAssigned == 0 {
        return Err(diag::assert_failed(
            g,
            out,
            err,
            &base_cpp(),
            99,
            "(bits & *tc)",
        ));
    }
    mris.tempsAssigned &= !bits;
    Ok(())
}

/// `MRISacquireNverticesFrozen(mris)`: `nverticesFrozen` one up (the vertex count may not change
/// while it is above 0).
pub fn acquire_nvertices_frozen(mris: &mut Mris) {
    mris.nverticesFrozen = mris.nverticesFrozen.wrapping_add(1);
}

/// `MRISreleaseNverticesFrozen(mris)`: `nverticesFrozen` one down.
pub fn release_nvertices_frozen(mris: &mut Mris) {
    mris.nverticesFrozen = mris.nverticesFrozen.wrapping_sub(1);
}

/// `MRISmakeFloatPerVertex(mris)`: freezes the vertex count and gives an array of `nvertices`
/// floats. FreeSurfer's are uninitialised (`malloc`): here 0. A negative count is a size
/// `malloc` refuses: `None` (`NULL`).
pub fn make_float_per_vertex(mris: &mut Mris) -> Option<Vec<f32>> {
    acquire_nvertices_frozen(mris);
    usize::try_from(mris.nvertices)
        .ok()
        .map(|n| vec![0.0f32; n])
}

/// `MRISfreeFloatPerVertex(mris, &p)`: frees the array, sets the caller's pointer to `NULL` and
/// unfreezes the vertex count.
pub fn free_float_per_vertex(mris: &mut Mris, p: &mut Option<Vec<f32>>) {
    *p = None;
    release_nvertices_frozen(mris);
}

/// `MRISsetInflatedFileName(name)`: the inflated surface's name becomes `name` and the first
/// curvature file `<name>.H`. Returns `NO_ERROR`.
///
/// FreeSurfer builds `<name>.H` in a 1,024-byte buffer whose size the official build checks
/// (`_FORTIFY_SOURCE`): a name of 1,022 bytes or more ends the program with `*** buffer overflow
/// detected ***: terminated` on standard error and signal 6 (FS-111). larmorx takes any length,
/// with FS-111's notice; `reproduce` gives the original's end (exit status 134).
pub fn set_inflated_file_name(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    err: &mut dyn Write,
    name: &[u8],
) -> Result<i32, Fatal> {
    let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())];
    sg.surface_names[0] = Some(name.to_vec());
    if name.len() + 3 > 1024 {
        g.notices.add(
            &FS_111,
            "an inflated-surface name of 1,022 bytes or more is taken whole (the original aborts)",
        );
        if g.notices.reproduce() {
            let _ = err.write_all(b"*** buffer overflow detected ***: terminated\n");
            return Err(Fatal::Exit(134));
        }
    }
    let mut copy = name.to_vec();
    copy.extend_from_slice(b".H");
    sg.curvature_names[0] = Some(copy);
    Ok(0)
}

/// `MRISsetSulcFileName(name)`: the second curvature file becomes `name`. Returns `NO_ERROR`.
pub fn set_sulc_file_name(sg: &mut SurfGlobals, name: &[u8]) -> i32 {
    let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())];
    sg.curvature_names[1] = Some(name.to_vec());
    0
}

/// `MRISsetOriginalFileName(name)`: the second and third surface names become `name`. Returns
/// `NO_ERROR`.
pub fn set_original_file_name(sg: &mut SurfGlobals, name: &[u8]) -> i32 {
    let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())];
    sg.surface_names[1] = Some(name.to_vec());
    sg.surface_names[2] = Some(name.to_vec());
    0
}
