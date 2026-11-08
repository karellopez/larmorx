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

/// AFNI-89: a basis whose least squares could not be set up (AFNI crashes at the first fit).
pub fn afni_89(mode: UpstreamDefects) -> Notice {
    Notice::new(
        &defects::AFNI_89,
        mode,
        "the registration's least squares could not be set up (see the messages above): AFNI \
         crashes (segmentation fault) at the first sub-brick it registers",
    )
}

/// AFNI-89's error, in both modes (rule B).
pub const AFNI_89_FATAL: &str = "the base's least squares could not be set up (the messages \
    above say why: all-zero weights, or a base whose rotated and shifted copies do not change \
    under the weight); AFNI crashes here (AFNI-89)";

/// AFNI-90: a name of 256 bytes or more (`what`: "-prefix name", "-weight dataset name", ...).
pub fn afni_90(what: &str, bytes: usize, mode: UpstreamDefects) -> Notice {
    let d = pick(
        mode,
        &format!(
            "the {what} has {bytes} bytes: kept whole; AFNI copies it into a 256-byte buffer and \
             aborts (buffer overflow detected)"
        ),
        &format!(
            "the {what} has {bytes} bytes, more than AFNI's 256-byte buffer holds: AFNI aborts \
             (buffer overflow detected)"
        ),
    );
    Notice::new(&defects::AFNI_90, mode, d)
}

/// AFNI-90's error with `reproduce` (AFNI aborts at the option).
pub fn afni_90_fatal(what: &str, bytes: usize) -> String {
    format!(
        "the {what} has {bytes} bytes; AFNI's buffer holds 255 and AFNI aborts here (*** buffer \
         overflow detected ***, AFNI-90)"
    )
}

/// AFNI-94: the subsampled coarse search's step of 0.
pub fn afni_94(mode: UpstreamDefects) -> Notice {
    Notice::new(
        &defects::AFNI_94,
        mode,
        "-twopass's coarse search with a step of 1 voxel on a volume of 120 or more voxels along \
         every axis: the subsampled search's step is 0, which AFNI repeats forever",
    )
}

/// AFNI-94's error, in both modes (rule B).
pub const AFNI_94_FATAL: &str = "-coarse with a step of 1 voxel on a volume of 120 or more \
    voxels along every axis: the subsampled coarse search would have a step of 0 (AFNI loops \
    forever; AFNI-94); use -coarse 2 or more";

/// AFNI-96: a `-rotparent` without its centre attribute (or with fewer than 3 numbers).
pub fn afni_96(dataset: &str, attribute: &str, mode: UpstreamDefects) -> Notice {
    Notice::new(
        &defects::AFNI_96,
        mode,
        format!(
            "the -rotparent dataset {dataset} has VOLREG_MATVEC_000000 but no {attribute} \
             attribute of 3 numbers: refused; AFNI reads through a NULL pointer (a crash) or past \
             the attribute"
        ),
    )
}

/// AFNI-96's error, in both modes (rule B).
pub fn afni_96_fatal(dataset: &str, attribute: &str) -> String {
    format!("-rotparent dataset {dataset} has no {attribute} attribute (AFNI-96: AFNI crashes)")
}
