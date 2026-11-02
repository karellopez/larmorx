// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (flirt.cc, globaloptions.{h,cc},
// defaultschedule.h, convert_xfm.cc); see PROVENANCE.md. Copyright (C) 1999-2023 University of
// Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the
// originals. Distributed under the FSL Licence (LICENSE): non-commercial use only, these
// conditions passed on to every receiver, the original and amended source included
// (fsl-source/).
//! The bit-exact replica of FSL's `flirt` (affine registration with every cost function,
//! boundary-based registration included, the search and the schedules) and `convert_xfm`, for
//! larmorx's `larmorx-nc` package.
//!
//! [`flirt`] runs the program with FLIRT's arguments: the same options, messages, exit statuses
//! and output files (`-omat`'s matrix, `-out`'s image), the same numbers. FLIRT's deterministic
//! defects are corrected by default and reproduced with `LARMORX_UPSTREAM_DEFECTS=reproduce`
//! (D13). The libraries FLIRT uses (newimage's cost functions, BBR, filters and resampling,
//! miscmaths' optimiser and transform algebra, NewNifti) are translated once, in
//! `larmorx-nc-core`.
#![forbid(unsafe_code)]
// Index loops mirror FLIRT's loops, so that each step can be checked against the source.
#![allow(clippy::needless_range_loop)]

pub mod applyxfm4d;
pub mod avscale;
pub mod convert_xfm;
pub mod coords;
pub mod makerot;
pub mod midtrans;
pub mod options;
pub mod pair;
pub mod pointflirt;
pub mod rmsdiff;
pub mod run;
pub mod schedule;
pub mod scripts;

use std::io::Write;

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::settings::Settings;

/// `0` (unset) as every logical CPU.
fn thread_count(threads: usize) -> usize {
    if threads > 0 {
        threads
    } else {
        std::thread::available_parallelism().map_or(1, usize::from)
    }
}

/// `flirt [options] -in <inputvol> -ref <refvol> ...`.
pub fn flirt(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    run::run(args, settings, mode, thread_count(threads), out, err)
}

/// `convert_xfm [options] <input-matrix-filename>`.
pub fn convert_xfm(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = threads;
    convert_xfm::run(args, settings, mode, out, err)
}

/// `avscale [--allparams/--inverteddies] matrixfile [non-reference-volume]`.
pub fn avscale(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = threads;
    avscale::run(args, settings, mode, out, err)
}

/// `img2imgcoord [options] <filename containing coordinates>`.
pub fn img2imgcoord(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = threads;
    coords::img2imgcoord(args, settings, mode, out, err)
}

/// `std2imgcoord [options] <filename containing coordinates>`.
pub fn std2imgcoord(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = threads;
    coords::std2imgcoord(args, settings, mode, out, err)
}

/// `img2stdcoord [options] <filename containing coordinates>`.
pub fn img2stdcoord(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = threads;
    coords::img2stdcoord(args, settings, mode, out, err)
}

/// `rmsdiff matrixfile1 matrixfile2 refvol [mask]`.
pub fn rmsdiff(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = threads;
    rmsdiff::run(args, settings, mode, out, err)
}

/// `applyxfm4D <input volume> <ref volume> <output volume> <transformation matrix file/dir>`.
pub fn applyxfm4d(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = threads;
    applyxfm4d::run(args, settings, mode, out, err)
}

/// `midtrans [options] transform1 transform2 ... transformN`.
pub fn midtrans(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = threads;
    midtrans::run(args, settings, mode, out, err)
}

/// `pointflirt -i <invol coords> -r <refvol coords> [options]`.
pub fn pointflirt(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = threads;
    pointflirt::run(args, settings, mode, out, err)
}

/// `makerot --theta=angle [options]`.
pub fn makerot(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = threads;
    makerot::run(args, settings, mode, out, err)
}

/// `pairreg brain1 brain2 skull1 skull2 outputmatrix [extra flirt args]` (the script).
pub fn pairreg(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    scripts::pairreg(args, settings, mode, thread_count(threads), out, err)
}

/// `extracttxt search_word file [num_trailing_lines [relative_start]]` (the script).
pub fn extracttxt(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    scripts::extracttxt(args, settings, mode, threads, out, err)
}

/// `aff2rigid <input2standard mat> <output mat>` (the script).
pub fn aff2rigid(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    scripts::aff2rigid(args, settings, mode, threads, out, err)
}
