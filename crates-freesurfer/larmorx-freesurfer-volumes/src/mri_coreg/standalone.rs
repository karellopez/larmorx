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
//! - `--landmarks sxyz txyz coords mov targ outlta [pointset]`, `--landmarks-2d ...`: the
//!   least-squares affine map of matching points (`RegLandmarks`: an `stvp` pair file or two JSON
//!   point sets; `RAS`, `VOX` or `TKR` coordinates) as an LTA; with a seventh argument the mapped
//!   source points as a JSON point set (FS-1326, FS-1327).
//!
//! Defects: FS-1316 (a number `sscanf` does not read where FreeSurfer goes on with the
//! variable it never wrote: `--par2mat`'s parameters, `--rms`'s radius; printed as 0 here,
//! noted), FS-1317 (`--rms` with 2 or 3 arguments reads its missing names past the command line
//! and crashes; `--rms`'s output file that does not open: FreeSurfer writes through a `NULL`
//! file and crashes), FS-1318 (`--lrrev` of a source whose axes hold no left-right one: FreeSurfer
//! writes outside its vector: an error in both modes).

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1316, FS_1317, FS_1318, FS_1326, FS_1327};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{
    MATRIX_REAL, Matrix, matrix_alloc, matrix_copy, matrix_multiply, matrix_multiply_d,
};
use larmorx_freesurfer_core::matrix_decomp::matrix_ns_condition_number;
use larmorx_freesurfer_core::matrix_ops::{matrix_inverse, matrix_normalize_col, matrix_transpose};
use larmorx_freesurfer_gca::transform::GcaLtaIo;
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::mriio::{mri_read_header, mri_read_volume};
use larmorx_freesurfer_io::pointset::{Point as PsPoint, PointSet, load_fs_point_set};
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
fn first_matrix(lta: &Lta) -> Option<&Matrix> {
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

/// A crash of FreeSurfer's (FS-1315, FS-1317, FS-1324: measured): `reproduce` status 139, after
/// the output FreeSurfer's buffer had written out; a clear error by default (status 1).
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

/// What FreeSurfer does from memory it corrupts or never wrote (FS-1318, FS-1321, FS-1322,
/// FS-1325): a clear error in both modes (status 1), named.
pub fn refuse(
    g: &mut Globals,
    r: &mut Run,
    id: &'static larmorx_core::defects::Defect,
    what: &str,
) -> Fatal {
    g.notices.add(id, format!("mri_coreg {what}"));
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
        return Ok(refuse(
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

/// `RegLandmarks`'s coordinates: the source's and the target's points.
struct Landmarks {
    sxyz: Vec<[f64; 3]>,
    txyz: Vec<[f64; 3]>,
}

/// `ReadSTVPairFile(file)`: each line up to the first empty one `<sno> <sx> <sy> <sz> <tno> <tx>
/// <ty> <tz>` (`sscanf`; a number that does not read keeps the previous line's, FS-1326 for one
/// never read: 0); a file that does not open: `ReadSTVPairFile(): <strerror> <file>`, `None`.
fn read_stvp(g: &mut Globals, r: &mut Run, file: &str, lm: &mut Landmarks) -> bool {
    let path = match r.dir {
        Some(d) => d.join(file),
        None => std::path::PathBuf::from(file),
    };
    if path.is_dir() {
        // an `ifstream` opens a folder; its first read fails as at the end: no point
        return true;
    }
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            let code = larmorx_freesurfer_core::log::errno_of(&e).unwrap_or(2);
            let _ = writeln!(
                r.out,
                "ReadSTVPairFile(): {} {file}",
                larmorx_freesurfer_core::log::strerror(code)
            );
            return false;
        }
    };
    // `int sno, tno; double sx, ...` declared once: a value `sscanf` leaves keeps the last line's
    let mut v: [Option<f64>; 8] = [None; 8];
    for line in bytes.split(|&b| b == b'\n') {
        if line.is_empty() {
            break;
        }
        let text = String::from_utf8_lossy(line);
        let mut b = larmorx_freesurfer_core::cscan::Bytes::new(text.as_bytes());
        let mut sc = larmorx_freesurfer_core::cscan::Scanf::new(&mut b);
        for (k, slot) in v.iter_mut().enumerate() {
            let x = if k == 0 || k == 4 {
                sc.d(&mut g.errno, true).map(f64::from)
            } else {
                sc.lf(&mut g.errno, true)
            };
            match x {
                Some(x) => *slot = Some(x),
                None => break,
            }
        }
        let _ = sc.finish(&mut g.errno);
        let mut get = |k: usize| -> f64 {
            v[k].unwrap_or_else(|| {
                g.notices.add(
                    &FS_1326,
                    format!("mri_coreg --landmarks: {file}: a number never read (FreeSurfer uses a variable it never wrote): 0"),
                );
                0.0
            })
        };
        let (sx, sy, sz) = (get(1), get(2), get(3));
        let (tx, ty, tz) = (get(5), get(6), get(7));
        lm.sxyz.push([sx, sy, sz]);
        lm.txyz.push([tx, ty, tz]);
    }
    true
}

/// `ComputeReg(NULL)` / `ComputeReg2D(NULL)`: the least-squares affine map of the source points
/// onto the target's (`beta = inverse(Xt X) Xt y`, `float` matrices), its condition printed
/// (`normalized condition = <%g>`); `None` when `Xt X` is singular.
fn compute_reg(
    g: &mut Globals,
    r: &mut Run,
    lm: &Landmarks,
    two_d: bool,
) -> Result<Option<Matrix>, Fatal> {
    let n = lm.sxyz.len() as i32;
    let d = if two_d { 2 } else { 3 };
    let mut x = matrix_alloc(g, r.out, r.err, n, d + 1, MATRIX_REAL)?.expect("a matrix");
    let mut y = matrix_alloc(g, r.out, r.err, n, d, MATRIX_REAL)?.expect("a matrix");
    for (row, (s, t)) in lm.sxyz.iter().zip(&lm.txyz).enumerate() {
        let row = row as i32 + 1;
        x.set_relt(row, d + 1, 1.0);
        for c in 0..d as usize {
            x.set_relt(row, c as i32 + 1, s[c] as f32);
            y.set_relt(row, c as i32 + 1, t[c] as f32);
        }
    }
    let xnorm = matrix_normalize_col(g, r.out, r.err, &x, None, None)?.into_new();
    let xcond = match xnorm.as_ref() {
        Some(m) => matrix_ns_condition_number(g, r.out, r.err, m)?,
        None => 0.0,
    };
    let _ = writeln!(
        r.out,
        "normalized condition = {}",
        cfmt::g(f64::from(xcond), 6)
    );
    let xt = matrix_transpose(g, r.out, r.err, &x, None)?.into_new();
    let xtx = matrix_multiply(g, r.out, r.err, xt.as_ref(), Some(&x), None)?.into_new();
    let Some(ixtx) = matrix_inverse(g, r.out, r.err, xtx.as_ref(), None)?.into_new() else {
        return Ok(None);
    };
    let ixtxxt = matrix_multiply(g, r.out, r.err, Some(&ixtx), xt.as_ref(), None)?.into_new();
    let beta = matrix_multiply(g, r.out, r.err, ixtxxt.as_ref(), Some(&y), None)?
        .into_new()
        .expect("a product");
    let mut rm = matrix_alloc(g, r.out, r.err, 4, 4, MATRIX_REAL)?.expect("a 4 x 4 matrix");
    rm.set_relt(4, 4, 1.0);
    for rr in 0..d {
        for c in 0..d + if two_d { 0 } else { 1 } {
            rm.set_relt(rr + 1, c + 1, beta.relt(c + 1, rr + 1));
        }
    }
    if two_d {
        rm.set_relt(3, 3, 1.0);
        rm.set_relt(1, 4, beta.relt(3, 1));
        rm.set_relt(2, 4, beta.relt(3, 2));
    }
    Ok(Some(rm))
}

/// `--landmarks sxyz txyz coords mov targ outlta [pointset]` (`--landmarks-2d`: the 2-D map,
/// `two_d`): `RegLandmarks::ComputeLTA` (the volumes' headers, the coordinates' type `RAS`, `VOX`
/// or `TKR`, the points from an `stvp` pair file or two JSON point sets, the least-squares map,
/// inverted for `TKR`, as an LTA converted to RAS to RAS), the subject (`--s`'s or `unknown`), the
/// LTA written; with exactly seven arguments the source points mapped by it saved as a JSON point
/// set. The writer's status; status 1 where the LTA cannot be made.
pub fn landmarks(
    g: &mut Globals,
    r: &mut Run,
    c: &CmdArgs,
    a: &[String],
    two_d: bool,
) -> Result<Fatal, Fatal> {
    let env = r.env.clone();
    let Some(src) = mri_read_header(g, r.out, r.err, &env, r.dir, &a[3], MRI_VOLUME_TYPE_UNKNOWN)?
    else {
        return Ok(Fatal::Exit(1));
    };
    let Some(trg) = mri_read_header(g, r.out, r.err, &env, r.dir, &a[4], MRI_VOLUME_TYPE_UNKNOWN)?
    else {
        return Ok(Fatal::Exit(1));
    };
    let coordtype = match a[2].as_str() {
        "RAS" => LINEAR_RAS_TO_RAS,
        "VOX" => LINEAR_VOX_TO_VOX,
        "TKR" => REGISTER_DAT,
        other => {
            let _ = writeln!(r.out, "ERROR: unrecognized coord type name {other}");
            let _ = writeln!(r.out, "   Expecting RAS, VOX, or TKR");
            return Ok(Fatal::Exit(1));
        }
    };
    let mut lm = Landmarks {
        sxyz: Vec::new(),
        txyz: Vec::new(),
    };
    // `ReadCoords()`: `stvp` as the second name reads the first as a pair file; otherwise both
    // as JSON point sets unless a name starts with `json` (`find("json") > 0`: a name without it
    // also passes)
    if a[1] == "stvp" {
        if !read_stvp(g, r, &a[0], &mut lm) {
            return Ok(Fatal::Exit(1));
        }
    } else if !a[0].starts_with("json") && !a[1].starts_with("json") {
        let ps1 = load_fs_point_set(g, r.err, r.dir, &a[0])?;
        let ps2 = load_fs_point_set(g, r.err, r.dir, &a[1])?;
        if ps1.npoints() != ps2.npoints() {
            let _ = writeln!(
                r.out,
                "ERROR: RegLandmarks::ReadJsonPair(): dim mismatch {} {}",
                ps1.npoints(),
                ps2.npoints()
            );
            return Ok(Fatal::Exit(1));
        }
        for (p1, p2) in ps1.points.iter().zip(&ps2.points) {
            lm.sxyz.push([p1.x, p1.y, p1.z]);
            lm.txyz.push([p2.x, p2.y, p2.z]);
        }
    } else {
        let _ = writeln!(
            r.out,
            "ERROR: reglandmarks only accepts stvp files right now"
        );
        return Ok(Fatal::Exit(1));
    }
    let Some(mut rm) = compute_reg(g, r, &lm, two_d)? else {
        // FS-1327: FreeSurfer goes on with no matrix: for `TKR` its inverse ends the program
        // (`MatrixInverse: NULL input matrix!`); for `VOX` the change of type's product does
        // (`MatrixMultiply(): m1 is null`); for `RAS` the writer opens the file and crashes (the
        // file left empty: its buffer never written)
        g.notices.add(
            &FS_1327,
            "mri_coreg --landmarks: the points' least-squares system is singular (FreeSurfer goes on with no matrix)",
        );
        if !g.notices.reproduce() {
            let _ = writeln!(
                r.err,
                "{NAME}: --landmarks: the points' least-squares system is singular (FreeSurfer goes on with no matrix and crashes)"
            );
            return Ok(Fatal::Exit(1));
        }
        if coordtype == REGISTER_DAT {
            matrix_inverse(g, r.out, r.err, None, None)?;
            return Ok(Fatal::Exit(1));
        }
        if coordtype == LINEAR_VOX_TO_VOX {
            // the change of type multiplies the missing matrix (`MatrixMultiply(): m1 is null`)
            let id = Matrix::real(4, 4);
            let mut lta = lta_create(g, r.out, r.err, &src, &trg, &id, coordtype)?;
            lta.xforms[0].m_l = None;
            change_type(g, r, &mut lta, LINEAR_RAS_TO_RAS)?;
            return Ok(Fatal::Exit(139));
        }
        if coordtype == LINEAR_RAS_TO_RAS {
            let path = match r.dir {
                Some(d) => d.join(&a[5]),
                None => std::path::PathBuf::from(&a[5]),
            };
            let _ = std::fs::File::create(path);
        }
        return Ok(Fatal::Exit(139));
    };
    if coordtype == REGISTER_DAT {
        let rin = rm.clone();
        if matrix_inverse(g, r.out, r.err, Some(&rin), Some(&mut rm))?.is_null() {
            return Ok(Fatal::Exit(1));
        }
    }
    let mut lta = lta_create(g, r.out, r.err, &src, &trg, &rm, coordtype)?;
    change_type(g, r, &mut lta, LINEAR_RAS_TO_RAS)?;
    set_subject(&mut lta, c.subject.as_deref());
    let st = write_lta(g, r, &lta, &a[5])?;
    // `Coords2PointSet(lta)`: each source point mapped by the transform (`float` matrices,
    // `MatrixMultiplyD`)
    let mut ps = PointSet::default();
    if let Some(m) = first_matrix(&lta).cloned() {
        for s in &lm.sxyz {
            let mut ras =
                matrix_alloc(g, r.out, r.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 matrix");
            ras.set_relt(4, 1, 1.0);
            for (k, v) in s.iter().enumerate() {
                ras.set_relt(k as i32 + 1, 1, *v as f32);
            }
            let nr = matrix_multiply_d(g, r.out, r.err, Some(&m), Some(&ras), None)?
                .into_new()
                .expect("a product");
            ps.add(PsPoint {
                x: f64::from(nr.relt(1, 1)),
                y: f64::from(nr.relt(2, 1)),
                z: f64::from(nr.relt(3, 1)),
                ..PsPoint::default()
            });
        }
    }
    if a.len() == 7 {
        ps.save(r.dir, &a[6]);
    }
    Ok(Fatal::Exit(st))
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
