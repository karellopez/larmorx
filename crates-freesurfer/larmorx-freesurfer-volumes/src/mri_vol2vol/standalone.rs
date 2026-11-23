// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The three options of `mri_vol2vol` that do their whole work while the command line is read
//! and end the program (`parse_commandline`, `mri_vol2vol.cpp` 1570-1683; queue item 3.2-38 part
//! 2, track A relay 54): `--spm-warp`, `--gcam` (`--gcam0`) and `--map-point`
//! (`--map-point-inv-lta`). The options before them on the command line apply (`--vsm-pedir`,
//! `--vg-thresh`, `--interp`'s code is not computed yet: the interpolation is their own
//! argument); `FREESURFER_HOME` must be set, `SUBJECTS_DIR` need not be.
//!
//! Defects (`larmorx_core::defects`): **FS-1184** (`--map-point`'s arguments that are not
//! numbers: memory contents), and those of [`super::gcam`].

use std::io::Write;

use larmorx_core::defects::FS_1184;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc, matrix_multiply_d};
use larmorx_freesurfer_gca::gcam::gcam_read;
use larmorx_freesurfer_gca::ltamatrix::lta_get_matrix;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_volume::aseg2vol::mri_apply_spm_warp;
use larmorx_freesurfer_volume::ltaops::{InvertInto, lta_invert};

use super::gcam::{Io, mri_vol2vol_gcam, mri_vol2vol_gcam0};
use super::main::{Run, vmpc, write_volume};
use super::options::{Options, istringnmatch, read_lta, read_volume};
use crate::mri_segstats::options::{scan_d, scan_lf};

/// `--spm-warp mov movlta warp interp out`: the moving volume resampled through an SPM VBM
/// deformation (`MRIapplySpmWarp`, no left-right reversal; `movlta` `0`: none), written; `mri_vol2vol
/// spm-vbm done`, the `VmPeak` line and status 0. A volume or LTA that does not read, a refused
/// interpolation (`MRIapplySpmWarp` takes 0 or 1) or a failed write: status 1.
pub fn spm_warp(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    rest: &[String],
) -> Result<(), Fatal> {
    let Some(mov) = read_volume(g, r, &rest[0])? else {
        return Err(Fatal::Exit(1));
    };
    let mut srclta = None;
    if rest[1] != "0" {
        let _ = writeln!(r.out, "Loading source LTA {}", rest[1]);
        srclta = read_lta(g, r, &rest[1])?;
        if srclta.is_none() {
            return Err(Fatal::Exit(1));
        }
    }
    let Some(warp) = read_volume(g, r, &rest[2])? else {
        return Err(Fatal::Exit(1));
    };
    if let Some(v) = scan_d(g, &rest[3]) {
        o.interpcode = v;
    }
    let _ = writeln!(
        r.out,
        "Running MRIapplySpmVbmWarp() interp = {}",
        o.interpcode
    );
    let env = r.env.clone();
    let out = mri_apply_spm_warp(
        g,
        r.out,
        r.err,
        &env,
        &mov,
        srclta.as_mut(),
        &warp,
        false,
        o.interpcode,
        None,
    )?;
    let Some(mut out) = out.into_new() else {
        return Err(Fatal::Exit(1));
    };
    let _ = writeln!(r.out, "Writing to {}", rest[4]);
    if write_volume(g, r, &mut out, &rest[4])? != 0 {
        return Err(Fatal::Exit(1));
    }
    let _ = writeln!(r.out, "mri_vol2vol spm-vbm done");
    vmpc(g, r);
    Err(Fatal::Exit(0))
}

/// `--gcam mov srclta gcam dstlta vsm interp out` (and `--gcam0`): the moving volume resampled
/// through the chain of [`super::gcam`] (`0` for any transform or map: none), written; `mri_vol2vol
/// gcam done`, the `VmPeak` line and status 0. `--gcam0`, or `MY_MORPHS_DO_NOT_CONFORM_DEAL_WITH_IT`
/// set, takes the older `MRIvol2volGCAM0`. A destination LTA that does not read is taken as none
/// (FreeSurfer does not check it); every other input that does not read, a refused chain or a
/// failed write: status 1.
pub fn gcam(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    option: &str,
    rest: &[String],
) -> Result<(), Fatal> {
    let _ = writeln!(r.out, "Loading mov {}", rest[0]);
    let Some(mov) = read_volume(g, r, &rest[0])? else {
        return Err(Fatal::Exit(1));
    };
    let mut srclta = None;
    if rest[1] != "0" {
        let _ = writeln!(r.out, "Loading source LTA {}", rest[1]);
        srclta = read_lta(g, r, &rest[1])?;
        if srclta.is_none() {
            return Err(Fatal::Exit(1));
        }
    }
    let mut gc = None;
    if rest[2] != "0" {
        let _ = writeln!(r.out, "Loading GCAM {}", rest[2]);
        let env = r.env.clone();
        gc = gcam_read(g, r.out, r.err, &env, r.dir, &rest[2])?;
        if gc.is_none() {
            return Err(Fatal::Exit(1));
        }
    }
    let mut dstlta = None;
    if rest[3] != "0" {
        let _ = writeln!(r.out, "Loading destination LTA {}", rest[3]);
        dstlta = read_lta(g, r, &rest[3])?;
    }
    let mut vsm = None;
    if rest[4] != "0" {
        vsm = read_volume(g, r, &rest[4])?;
        if vsm.is_none() {
            return Err(Fatal::Exit(1));
        }
    }
    if let Some(v) = scan_d(g, &rest[5]) {
        o.interpcode = v;
    }
    let targvolfile = rest[6].clone();
    let env = r.env.clone();
    let old = env.get("MY_MORPHS_DO_NOT_CONFORM_DEAL_WITH_IT").is_some()
        || istringnmatch(option, "--gcam0", 0);
    let out = {
        let mut io = Io {
            out: &mut *r.out,
            err: &mut *r.err,
            env: &env,
        };
        if old {
            mri_vol2vol_gcam0(
                g,
                &mut io,
                &mov,
                srclta.as_mut(),
                gc.as_ref(),
                dstlta.as_mut(),
                vsm.as_ref(),
                o.interpcode,
            )?
        } else {
            mri_vol2vol_gcam(
                g,
                &mut io,
                &mov,
                srclta.as_ref(),
                gc.as_ref(),
                dstlta.as_ref(),
                vsm.as_ref(),
                o.interpcode,
                o.pedir,
            )?
        }
    };
    let Some(mut out) = out else {
        return Err(Fatal::Exit(1));
    };
    let _ = writeln!(r.out, "Writing to {targvolfile}");
    if write_volume(g, r, &mut out, &targvolfile)? != 0 {
        return Err(Fatal::Exit(1));
    }
    let _ = writeln!(r.out, "mri_vol2vol gcam done");
    vmpc(g, r);
    Err(Fatal::Exit(0))
}

/// A `--map-point` argument read as a number (`sscanf`): FS-1184 when it is not one (FreeSurfer
/// keeps the variable's memory contents): by default `ERROR: --map-point: <argument> is not a
/// number`, status 1; with `reproduce` 0.
fn number<T: Default>(g: &mut Globals, r: &mut Run, v: Option<T>, arg: &str) -> Result<T, Fatal> {
    if let Some(v) = v {
        return Ok(v);
    }
    g.notices.add(
        &FS_1184,
        format!("mri_vol2vol --map-point: {arg} is not a number (FreeSurfer maps memory contents)"),
    );
    if g.notices.reproduce() {
        return Ok(T::default());
    }
    let _ = writeln!(r.out, "ERROR: --map-point: {arg} is not a number");
    Err(Fatal::Exit(1))
}

/// `--map-point a b c incoords lta outcoords outfile` (and `--map-point-inv-lta`, the LTA
/// inverted first): the point `(a, b, c)` in the input kind of coordinates (1 tkregister RAS, 2
/// scanner RAS, 3 voxels) mapped by the LTA's matrix for the two kinds (`LTA::get_matrix`,
/// `MatrixMultiplyD`; the point stored as `float`), printed (`%8.4f %8.4f %8.4f `) and, unless
/// `outfile` is `nofile`, written to it (`writing to <outfile>`; a file that does not open:
/// `ERROR: could not open <outfile>`, status 1); status 0. An LTA that does not read or a matrix
/// that cannot be made: status 1.
pub fn map_point(g: &mut Globals, r: &mut Run, option: &str, rest: &[String]) -> Result<(), Fatal> {
    let a = scan_lf(g, &rest[0]);
    let a = number(g, r, a, &rest[0])?;
    let b = scan_lf(g, &rest[1]);
    let b = number(g, r, b, &rest[1])?;
    let c = scan_lf(g, &rest[2]);
    let c = number(g, r, c, &rest[2])?;
    let incoords = scan_d(g, &rest[3]);
    let incoords = number(g, r, incoords, &rest[3])?;
    let Some(mut lta) = read_lta(g, r, &rest[4])? else {
        return Err(Fatal::Exit(1));
    };
    if istringnmatch(option, "--map-point-inv-lta", 0) {
        lta_invert(g, r.out, r.err, InvertInto::Same(&mut lta))?;
    }
    let outcoords = scan_d(g, &rest[5]);
    let outcoords = number(g, r, outcoords, &rest[5])?;
    let env = r.env.clone();
    let Some(m) = lta_get_matrix(g, r.out, r.err, &env, &lta, incoords, outcoords)? else {
        return Err(Fatal::Exit(1));
    };
    let mut src = matrix_alloc(g, r.out, r.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    src.set_relt(1, 1, a as f32);
    src.set_relt(2, 1, b as f32);
    src.set_relt(3, 1, c as f32);
    src.set_relt(4, 1, 1.0);
    let Some(trg) = matrix_multiply_d(g, r.out, r.err, Some(&m), Some(&src), None)?.into_new()
    else {
        return Err(Fatal::Exit(139));
    };
    let line = format!(
        "{} {} {} \n",
        fmt_f(f64::from(trg.relt(1, 1)), 8, 4),
        fmt_f(f64::from(trg.relt(2, 1)), 8, 4),
        fmt_f(f64::from(trg.relt(3, 1)), 8, 4)
    );
    let _ = r.out.write_all(line.as_bytes());
    if rest[6] != "nofile" {
        let mut fp = match CFile::open(r.dir, &rest[6], "w") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                let _ = writeln!(r.out, "ERROR: could not open {}", rest[6]);
                return Err(Fatal::Exit(1));
            }
        };
        let _ = writeln!(r.out, "writing to {}", rest[6]);
        let _ = fp.write_all(line.as_bytes());
    }
    Err(Fatal::Exit(0))
}
