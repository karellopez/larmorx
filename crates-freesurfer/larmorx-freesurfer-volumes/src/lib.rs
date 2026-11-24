// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-volumes: wave 3.2's programs of FreeSurfer 8.2.0's replica
//! (docs/freesurfer/waves.md section 4): the volume programs both streams run (`mri_convert`,
//! `mri_info`, `mri_normalize`, `mri_segment`, `mri_fill`, ...), fMRIPrep's `mri_coreg`,
//! `mri_robust_register` and `mri_robust_template`, and the scripts around them
//! (`mri_nu_correct.mni`, `seg2cc`, ...).
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-freesurfer` program as a
//! separate process (`docs/licensing.md`). Empty until its queue items bring the code
//! (`docs/freesurfer/queue.md`); `PROVENANCE.md` lists what each item translates.
//!
//! [`PROGRAMS`] lists the programs of this crate that `larmorx-freesurfer` runs (one line each).
#![forbid(unsafe_code)]

pub mod mri_normalize;

use larmorx_freesurfer_core::run::Program;

pub mod csh;
pub mod fs_temp_file;
pub mod fscalc;
pub mod getfullpath;
pub mod lta_convert;
pub mod mergeseg;
pub mod mri_add_xform_to_header;
pub mod mri_binarize;
pub mod mri_cc;
pub mod mri_concat;
pub mod mri_convert;
pub mod mri_edit_wm_with_aseg;
pub mod mri_info;
pub mod mri_label2vol;
pub mod mri_make_uchar;
pub mod mri_mask;
pub mod mri_robust_register;
pub mod mri_segment;
pub mod mri_segstats;
pub mod mri_vol2vol;
pub mod mris_calc;
pub mod script;
pub mod seg2cc;

/// The programs of this crate, by their original names.
pub const PROGRAMS: &[Program] = &[
    Program {
        name: fs_temp_file::main::NAME,
        run: fs_temp_file::main::run,
    },
    Program {
        name: fscalc::main::NAME,
        run: fscalc::main::run,
    },
    Program {
        name: getfullpath::main::NAME,
        run: getfullpath::main::run,
    },
    Program {
        name: lta_convert::main::NAME,
        run: lta_convert::main::run,
    },
    Program {
        name: mergeseg::main::NAME,
        run: mergeseg::main::run,
    },
    Program {
        name: mri_add_xform_to_header::main::NAME,
        run: mri_add_xform_to_header::main::run,
    },
    Program {
        name: mri_binarize::main::NAME,
        run: mri_binarize::main::run,
    },
    Program {
        name: mri_cc::main::NAME,
        run: mri_cc::main::run,
    },
    Program {
        name: mri_concat::main::NAME,
        run: mri_concat::main::run,
    },
    Program {
        name: mri_convert::main::NAME,
        run: mri_convert::main::run,
    },
    Program {
        name: mri_edit_wm_with_aseg::main::NAME,
        run: mri_edit_wm_with_aseg::main::run,
    },
    Program {
        name: mri_info::main::NAME,
        run: mri_info::main::run,
    },
    Program {
        name: mri_label2vol::main::NAME,
        run: mri_label2vol::main::run,
    },
    Program {
        name: mri_make_uchar::main::NAME,
        run: mri_make_uchar::main::run,
    },
    Program {
        name: mri_mask::main::NAME,
        run: mri_mask::main::run,
    },
    Program {
        name: mri_normalize::main::NAME,
        run: mri_normalize::main::run,
    },
    Program {
        name: mri_robust_register::main::NAME,
        run: mri_robust_register::main::run,
    },
    Program {
        name: mri_segment::main::NAME,
        run: mri_segment::main::run,
    },
    Program {
        name: mri_segstats::main::NAME,
        run: mri_segstats::main::run,
    },
    Program {
        name: mri_vol2vol::main::NAME,
        run: mri_vol2vol::main::run,
    },
    Program {
        name: mris_calc::main::NAME,
        run: mris_calc::main::run,
    },
    Program {
        name: seg2cc::main::NAME,
        run: seg2cc::main::run,
    },
];
