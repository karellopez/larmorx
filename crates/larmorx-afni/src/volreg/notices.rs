// SPDX-License-Identifier: Apache-2.0
//! The upstream-defect notices of 3dvolreg (`specs/3dvolreg.md` §12), worded as the replica
//! program words them (observed by the clean-room implementer in black-box runs of
//! `larmorx afni 3dvolreg` with `LARMORX_IMPLEMENTATION=replica`), so that both implementations
//! say the same.

use larmorx_core::defects::{self, Notice, UpstreamDefects};

fn pick(mode: UpstreamDefects, fix: &str, reproduce: &str) -> String {
    match mode {
        UpstreamDefects::Fix => fix.to_owned(),
        UpstreamDefects::Reproduce => reproduce.to_owned(),
    }
}

/// AFNI-86: `-nocoarserot` read as `-noclip`.
pub fn afni_86(mode: UpstreamDefects) -> Notice {
    let d = pick(
        mode,
        "-nocoarserot turns the coarse rotation search off, as its help says; AFNI reads it as \
         -noclip (its first four letters), which turns the clipping off instead",
        "-nocoarserot is read as -noclip (its first four letters): the clipping is turned off and \
         the coarse rotation search stays on, as AFNI does",
    );
    Notice::new(&defects::AFNI_86, mode, d)
}

/// AFNI-87: a byte dataset's registered volumes stretched.
pub fn afni_87(mode: UpstreamDefects) -> Notice {
    let d = pick(
        mode,
        "a byte dataset's registered sub-bricks are written as rounded bytes; AFNI stretches each \
         to 0..255.9 from its own minimum and maximum",
        "a byte dataset's registered sub-bricks are stretched to 0..255.9 from their own minimum \
         and maximum, as AFNI does",
    );
    Notice::new(&defects::AFNI_87, mode, d)
}

/// AFNI-88: an option given last without its value.
pub fn afni_88(option: &str, mode: UpstreamDefects) -> Notice {
    Notice::new(
        &defects::AFNI_88,
        mode,
        format!(
            "{option} is the last argument and needs a value: AFNI reads past the end of its \
             arguments (a crash)"
        ),
    )
}

/// AFNI-92: `-nomaxdisp` changes the coarse rotation search's automask.
pub fn afni_92(mode: UpstreamDefects) -> Notice {
    let d = pick(
        mode,
        "-nomaxdisp: the coarse rotation search's automask keeps clip fraction 0.333, as with the \
         default -maxdisp; AFNI uses 0.5 then, so its registration depends on -nomaxdisp",
        "-nomaxdisp leaves the automask's clip fraction 0.5 for -twopass's coarse rotation \
         search, where the default (-maxdisp) sets 0.333: the registration depends on \
         -nomaxdisp, as in AFNI",
    );
    Notice::new(&defects::AFNI_92, mode, d)
}

/// AFNI-93: the coarse search's region is empty.
pub fn afni_93(mode: UpstreamDefects) -> Notice {
    let d = pick(
        mode,
        "the coarse search's region (the voxels at least (num+1)*del from every face) is empty \
         for this volume: those shifts were not taken; AFNI scores each 0 and keeps the first (a \
         large initial shift and rotation)",
        "the coarse search's region (the voxels at least (num+1)*del from every face) is empty \
         for this volume: every candidate scores 0 and the first is kept, as AFNI does",
    );
    Notice::new(&defects::AFNI_93, mode, d)
}

/// AFNI-95: a second `-base` after `-base 0`.
pub fn afni_95(mode: UpstreamDefects) -> Notice {
    let d = pick(
        mode,
        "a second -base after -base 0: refused; AFNI accepts it (the last -base wins)",
        "a second -base after -base 0 replaces it, as AFNI does",
    );
    Notice::new(&defects::AFNI_95, mode, d)
}

/// AFNI-99 for the maximum displacements.
pub fn afni_99_maxdisp(mode: UpstreamDefects) -> Notice {
    let d = pick(
        mode,
        "the maximum displacements are those of the voxels themselves; AFNI converts the matrix \
         with the signs of the axes stored L2R, P2A or S2I but not the voxel positions, and so \
         reports the displacements of the mirrored positions",
        "the maximum displacements are those of the positions mirrored along the axes stored \
         L2R, P2A or S2I, as AFNI computes them",
    );
    Notice::new(&defects::AFNI_99, mode, d)
}

/// AFNI-99 for `-savedisp`'s displacements.
pub fn afni_99_savedisp(mode: UpstreamDefects) -> Notice {
    let d = pick(
        mode,
        "-savedisp's displacements are those of the voxels themselves; AFNI converts the matrix \
         with the signs of the axes stored L2R, P2A or S2I but not the voxel positions, and so \
         writes the displacements of the mirrored positions",
        "-savedisp's displacements are those of the positions mirrored along the axes stored \
         L2R, P2A or S2I, as AFNI computes them",
    );
    Notice::new(&defects::AFNI_99, mode, d)
}
