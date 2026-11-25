// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_coreg`'s options that do their work while the command line is read and end the program
//! (`mri_coreg.cpp`'s `parse_commandline`, 703-901), and `--init-reg` (946-956), which reads its
//! transform there.
//!
//! - `--mat2par reg.lta` / `--mat2par-vox reg.lta`: the transform (as RAS to RAS, or voxel to
//!   voxel), inverted, and its twelve affine parameters (`%g ` each, then a newline); status 0.
//! - `--mat2rot reg.lta rotreg.lta`: the transform reduced to its rotation (`LTAmat2RotMat`),
//!   written; the writer's status.
//! - `--par2mat p1 .. p12 src trg reg.lta`: the parameters (`%lf`, one per line), the two volumes
//!   read, the matrix of the parameters inverted as a RAS to RAS transform from `src` to `trg`
//!   (subject from an earlier `--s`, else `unknown`), written; the writer's status.
//! - `--lrrev reg.lta out.lta`: the transform a left-right reversal of the source's voxels would
//!   give (FreeSurfer's approximation); the writer's status.
//! - `--rms radius file reg1 reg2`: Jenkinson's RMS difference of the two registrations within the
//!   radius, written to `file` (unless it is `nofile`) and printed; status 0.
//! - `--landmarks`, `--landmarks-2d`: not ported yet (no recorded step uses them).
//!
//! Defects: FS-1316 (a number `sscanf` does not read where FreeSurfer goes on with the
//! variable it never wrote: `--par2mat`'s parameters, `--rms`'s radius; printed as 0 here,
//! noted), FS-1317 (`--rms` with 2 or 3 arguments reads its missing names past the command line
//! and crashes; `--rms`'s output file that does not open: FreeSurfer writes through a `NULL`
//! file and crashes), FS-1318 (`--lrrev` of a source whose axes hold no left-right one: FreeSurfer
//! writes outside its vector).

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1316, FS_1317, FS_1318};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::matrix_multiply;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_copy};
use larmorx_freesurfer_core::matrix_ops::matrix_inverse;
use larmorx_freesurfer_gca::transform::GcaLtaIo;
use larmorx_freesurfer_io::mriio::mri_read_volume;
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, Lta, REGISTER_DAT};
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc_from_vol_geom};
use larmorx_freesurfer_volume::coords::dircos_to_orientation_string;
use larmorx_freesurfer_volume::ltaconv::lta_create;
use larmorx_freesurfer_volume::ltaops::{InvertInto, lta_change_type, lta_invert};
use larmorx_freesurfer_volume::ltawrite::{lta_read, lta_write};
use larmorx_freesurfer_volume::rotmat::{
    lta_mat2rot_mat, rms_reg_diff_mj, tranform_affine_params2_matrix,
    tranform_extract_affine_params,
};
use larmorx_freesurfer_volume::vgeom::{vg_i_to_r, vg_r_to_i};

use super::main::{NAME, Run};
use super::options::CmdArgs;
use crate::mri_segstats::options::scan_lf;

/// `LTAread(name)`.
pub fn read_lta(g: &mut Globals, r: &mut Run, name: &str) -> Result<Option<Lta>, Fatal> {
    let env = r.env.clone();
    let mut io = GcaLtaIo { env: &env };
    lta_read(g, r.out, r.err, r.dir, name, &mut io)
}

/// `LTAwrite(lta, name)`: the writer's status.
pub fn write_lta(g: &mut Globals, r: &mut Run, lta: &Lta, name: &str) -> Result<i32, Fatal> {
    let env = r.env.clone();
    lta_write(g, r.out, r.err, &env, r.dir, lta, name)
}

/// `LTAchangeType(lta, ltype)`.
pub fn change_type(g: &mut Globals, r: &mut Run, lta: &mut Lta, ltype: i32) -> Result<(), Fatal> {
    let env = r.env.clone();
    lta_change_type(g, r.out, r.err, &env, lta, ltype)
}

/// `MRIread(name)`.
pub fn read_volume(g: &mut Globals, r: &mut Run, name: &str) -> Result<Option<Mri>, Fatal> {
    let env = r.env.clone();
    mri_read_volume(g, r.out, r.err, &env, r.dir, name)
}

/// The first transform's matrix (FreeSurfer reads it without a check: an array that holds none
/// is FS-504's, fs.volume's).
fn first_matrix<'a>(lta: &'a Lta) -> Option<&'a Matrix> {
    lta.xforms.first().and_then(|x| x.m_l.as_ref())
}

/// `TranformExtractAffineParams(lta->xforms[0].m_L, p)` into `p` (unchanged when FreeSurfer's
/// function refuses the matrix).
fn extract(g: &mut Globals, r: &mut Run, lta: &Lta, p: &mut [f64; 12]) -> Result<(), Fatal> {
    let m = first_matrix(lta).cloned();
    if let Some(v) = tranform_extract_affine_params(g, r.out, r.err, m.as_ref())? {
        *p = v;
    }
    Ok(())
}

/// The twelve parameters with `%g ` each.
fn print_params(out: &mut dyn Write, p: &[f64; 12]) {
    for v in p {
        let _ = write!(out, "{} ", cfmt::g(*v, 6));
    }
}

/// The subject an `LTA` gets: `--s`'s, else `unknown` (`strncpy` into `subject`, 1,023 bytes).
pub fn set_subject(lta: &mut Lta, subject: Option<&str>) {
    let s = subject.unwrap_or("unknown").as_bytes();
    let n = s.len().min(lta.subject.buffer().len() - 1);
    let buf = lta.subject.buffer_mut();
    buf[..n].copy_from_slice(&s[..n]);
    let last = buf.len() - 1;
    buf[n..last].fill(0);
}

/// `--mat2par reg.lta` (`ras`) or `--mat2par-vox reg.lta`.
pub fn mat2par(g: &mut Globals, r: &mut Run, name: &str, ras: bool) -> Result<Fatal, Fatal> {
    let Some(mut lta) = read_lta(g, r, name)? else {
        return Ok(Fatal::Exit(1));
    };
    change_type(
        g,
        r,
        &mut lta,
        if ras {
            LINEAR_RAS_TO_RAS
        } else {
            LINEAR_VOX_TO_VOX
        },
    )?;
    lta_invert(g, r.out, r.err, InvertInto::Same(&mut lta))?;
    let mut p = [0.0f64; 12];
    let had = first_matrix(&lta).is_some();
    extract(g, r, &lta, &mut p)?;
    if !had {
        fs_1316(
            g,
            "--mat2par: the transform has no matrix (FreeSurfer prints numbers it never wrote)",
        );
    }
    print_params(r.out, &p);
    let _ = writeln!(r.out);
    Ok(Fatal::Exit(0))
}

/// `--mat2rot reg.lta rotreg.lta`.
pub fn mat2rot(g: &mut Globals, r: &mut Run, name: &str, out_name: &str) -> Result<Fatal, Fatal> {
    let Some(mut lta) = read_lta(g, r, name)? else {
        return Ok(Fatal::Exit(1));
    };
    lta_mat2rot_mat(g, r.out, r.err, &mut lta)?;
    let st = write_lta(g, r, &lta, out_name)?;
    Ok(Fatal::Exit(st))
}

/// FS-1316's notice: a value FreeSurfer reads from a variable it never wrote, 0 here.
fn fs_1316(g: &mut Globals, what: &str) {
    g.notices
        .add(&FS_1316, format!("mri_coreg {what}: taken as 0"));
}

/// `--par2mat p1 .. p12 src trg reg.lta` (`a` holds the 15 arguments).
pub fn par2mat(g: &mut Globals, r: &mut Run, c: &CmdArgs, a: &[String]) -> Result<Fatal, Fatal> {
    let mut par = [0.0f64; 12];
    for (k, p) in par.iter_mut().enumerate() {
        match scan_lf(g, &a[k]) {
            Some(v) => *p = v,
            None => fs_1316(
                g,
                &format!(
                    "--par2mat: parameter {} '{}' is no number (FreeSurfer uses the variable it never wrote)",
                    k + 1,
                    a[k]
                ),
            ),
        }
    }
    for p in &par {
        let _ = writeln!(r.out, "{}", cfmt::f(*p, 6));
    }
    let Some(src) = read_volume(g, r, &a[12])? else {
        return Ok(Fatal::Exit(1));
    };
    let Some(trg) = read_volume(g, r, &a[13])? else {
        return Ok(Fatal::Exit(1));
    };
    let mut t = tranform_affine_params2_matrix(g, r.out, r.err, &par, None)?.unwrap_new();
    let tin = t.clone();
    matrix_inverse(g, r.out, r.err, Some(&tin), Some(&mut t))?;
    let mut lta = lta_create(g, r.out, r.err, &src, &trg, &t, LINEAR_RAS_TO_RAS)?;
    set_subject(&mut lta, c.subject.as_deref());
    let st = write_lta(g, r, &lta, &a[14])?;
    Ok(Fatal::Exit(st))
}

/// FS-1317 and FS-1318: a crash of FreeSurfer's (`reproduce`: status 139, after the defined
/// output), a clear error by default (status 1).
pub fn crash(
    g: &mut Globals,
    r: &mut Run,
    id: &'static larmorx_core::defects::Defect,
    what: &str,
) -> Fatal {
    g.notices.add(id, format!("mri_coreg {what}"));
    if g.notices.reproduce() {
        return Fatal::Exit(139);
    }
    let _ = writeln!(r.err, "{NAME}: {what}");
    Fatal::Exit(1)
}

/// `--lrrev reg.lta reg.lrrev.lta`.
pub fn lrrev(g: &mut Globals, r: &mut Run, name: &str, out_name: &str) -> Result<Fatal, Fatal> {
    let Some(mut lta) = read_lta(g, r, name)? else {
        return Ok(Fatal::Exit(1));
    };
    let srctype = lta.ltype;
    change_type(g, r, &mut lta, LINEAR_RAS_TO_RAS)?;
    let m_l = first_matrix(&lta).cloned();
    let tinv0 = matrix_inverse(g, r.out, r.err, m_l.as_ref(), None)?.into_new();
    let src_vg = lta.xforms[0].src.clone();
    let mvox2ras = vg_i_to_r(g, r.out, r.err, &src_vg)?;
    let mras2vox = vg_r_to_i(g, r.out, r.err, &src_vg)?;
    let m = matrix_multiply(g, r.out, r.err, mras2vox.as_ref(), tinv0.as_ref(), None)?.into_new();
    let mri = mri_alloc_from_vol_geom(g, r.out, r.err, &src_vg, MRI_UCHAR, 1, true)?;
    let (_, o) = dircos_to_orientation_string(&mri.vg);
    let ostr = String::from_utf8_lossy(&o).into_owned();
    let _ = writeln!(r.out, "Orientation {ostr}");
    let (mut k, mut ndim) = (-1i32, 0i32);
    if o[0] == b'L' || o[0] == b'R' {
        k = 1;
        ndim = src_vg.width;
    }
    if o[1] == b'L' || o[1] == b'R' {
        k = 2;
        ndim = src_vg.height;
    }
    if o[2] == b'L' || o[2] == b'R' {
        k = 3;
        ndim = src_vg.depth;
    }
    let _ = writeln!(r.out, "k={k} ndim={ndim}");
    if k < 0 {
        return Ok(crash(
            g,
            r,
            &FS_1318,
            &format!(
                "--lrrev: the source's orientation {ostr} has no left-right axis (FreeSurfer writes outside its vector)"
            ),
        ));
    }
    let mut k2 = matrix_alloc(g, r.out, r.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 matrix");
    if let Some(m) = m.as_ref() {
        for n in 1..=4 {
            k2.set_relt(n, 1, m.relt(n, 4));
        }
    }
    let v = (ndim - 1) as f32 - k2.relt(k, 1);
    k2.set_relt(k, 1, v);
    let p2 = matrix_multiply(g, r.out, r.err, Some(&mvox2ras), Some(&k2), None)?.into_new();
    lta_invert(g, r.out, r.err, InvertInto::Same(&mut lta))?;
    let mut p = [0.0f64; 12];
    extract(g, r, &lta, &mut p)?;
    let _ = write!(r.out, "Input Parameters: ");
    print_params(r.out, &p);
    let _ = writeln!(r.out);
    p[0] = p2.as_ref().map_or(0.0, |m| f64::from(m.relt(1, 1)));
    p[4] = -p[4];
    p[5] = -p[5];
    p[9] = -p[9];
    p[10] = -p[10];
    let _ = write!(r.out, "New Parameters:   ");
    print_params(r.out, &p);
    let _ = writeln!(r.out);
    let t = tranform_affine_params2_matrix(g, r.out, r.err, &p, None)?.unwrap_new();
    if let Some(dst) = lta.xforms[0].m_l.as_mut() {
        matrix_copy(g, r.out, r.err, Some(&t), Some(dst))?;
    }
    lta_invert(g, r.out, r.err, InvertInto::Same(&mut lta))?;
    change_type(g, r, &mut lta, srctype)?;
    let st = write_lta(g, r, &lta, out_name)?;
    Ok(Fatal::Exit(st))
}

/// `--landmarks` / `--landmarks-2d`: not ported yet.
pub fn landmarks(_g: &mut Globals, r: &mut Run, option: &str) -> Fatal {
    let _ = writeln!(r.err, "{NAME}: {option} is not ported yet");
    Fatal::Exit(1)
}

/// `--rms radius file reg1 reg2` (`a`: the arguments after the option, at least 2).
pub fn rms(g: &mut Globals, r: &mut Run, a: &[String]) -> Result<Fatal, Fatal> {
    if a[0].as_bytes().first().is_some_and(u8::is_ascii_alphabetic) {
        let _ = writeln!(r.out, "ERROR: first arg to --rms must be the radius");
        return Ok(Fatal::Exit(1));
    }
    let rad = match scan_lf(g, &a[0]) {
        Some(v) => v,
        None => {
            fs_1316(
                g,
                &format!(
                    "--rms: the radius '{}' is no number (FreeSurfer uses the variable it never wrote)",
                    a[0]
                ),
            );
            0.0
        }
    };
    if rad <= 0.0 {
        let _ = writeln!(r.out, "ERROR: radius is {}, must be > 0", cfmt::f(rad, 6));
        return Ok(Fatal::Exit(1));
    }
    let file = a[1].clone();
    let Some(name1) = a.get(2).cloned() else {
        return Ok(crash(
            g,
            r,
            &FS_1317,
            "--rms: no first registration (FreeSurfer reads its name past the command line and crashes)",
        ));
    };
    let Some(mut lta1) = read_lta(g, r, &name1)? else {
        return Ok(Fatal::Exit(1));
    };
    change_type(g, r, &mut lta1, REGISTER_DAT)?;
    let Some(name2) = a.get(3).cloned() else {
        return Ok(crash(
            g,
            r,
            &FS_1317,
            "--rms: no second registration (FreeSurfer reads its name past the command line and crashes)",
        ));
    };
    let Some(mut lta2) = read_lta(g, r, &name2)? else {
        return Ok(Fatal::Exit(1));
    };
    change_type(g, r, &mut lta2, REGISTER_DAT)?;
    let (Some(m1), Some(m2)) = (first_matrix(&lta1).cloned(), first_matrix(&lta2).cloned()) else {
        return Ok(crash(
            g,
            r,
            &FS_1317,
            "--rms: a registration without a matrix (FreeSurfer reads through it and crashes)",
        ));
    };
    let v = rms_reg_diff_mj(g, r.out, r.err, &m1, &m2, rad)?;
    let line = format!("{}\n", cfmt::pad(&cfmt::f(v, 10), 20));
    if file != "nofile" {
        let path = match r.dir {
            Some(d) => d.join(&file),
            None => std::path::PathBuf::from(&file),
        };
        if std::fs::write(&path, line.as_bytes()).is_err() {
            return Ok(crash(
                g,
                r,
                &FS_1317,
                &format!(
                    "--rms: {file} does not open for writing (FreeSurfer writes through a NULL file and crashes)"
                ),
            ));
        }
    }
    let _ = writeln!(r.out, "rms {}", cfmt::pad(&cfmt::f(v, 10), 20));
    Ok(Fatal::Exit(0))
}

/// `--init-reg reg0.lta`: the transform as RAS to RAS, inverted, its parameters into
/// `c.params`; `LTAread`'s failure ends the program (status 1).
pub fn init_reg(g: &mut Globals, r: &mut Run, c: &mut CmdArgs, name: &str) -> Result<(), Fatal> {
    let Some(mut lta) = read_lta(g, r, name)? else {
        return Err(Fatal::Exit(1));
    };
    change_type(g, r, &mut lta, LINEAR_RAS_TO_RAS)?;
    lta_invert(g, r.out, r.err, InvertInto::Same(&mut lta))?;
    extract(g, r, &lta, &mut c.params)?;
    Ok(())
}
