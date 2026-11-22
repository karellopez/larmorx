// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `lta_convert`'s readers and writers of the transform formats (`lta_convert.cpp` 196-921):
//! `shallowCopyLTA`, `readLTA`, `readFSL`, `readMNI`, `readREG`, `readNIFTYREG`, `readITK`,
//! `readVOX`, `writeFSL`, `writeMNI`, `writeREG`, `writeNIFTYREG`, `writeITK`, `writeVOX`.
//!
//! Every reader gives a `LINEAR_RAS_TO_RAS` array whose first transform's geometries are the
//! source's and the target's (`--src`, `--trg`, read with `MRIreadHeader`; for `--inlta` only when
//! given, the file's otherwise). The `.lta`, FSL, `.xfm` and `register.dat` files are read and
//! written by fs.volume's `LTAreadExType`/`LTAwrite` (their type changes `LTAchangeType`'s); the
//! NiftyReg, ITK and voxel-space text formats here, with C++'s streams ([`super::cxx`]):
//! - NiftyReg (`--inniftyreg`, `--inras`; `--outniftyreg`, `--outras`): the 4 x 4 matrix from the
//!   target to the source (its inverse taken), four numbers a row; `--inniftyreg2d` two rows of
//!   three (the first two columns and the translation). Written with 7 significant digits.
//! - ITK (`--initk`, `--inlps`; `--outitk`, `--outlps`): `Transform:` (`AffineTransform_double_3_3`,
//!   `MatrixOffsetTransformBase_double_3_3`, or `AffineTransform_double_2_2` for images flat along
//!   one axis), `Parameters:` (the 3 x 3 matrix by rows and the translation, LPS from the target to
//!   the source) and `FixedParameters:` (the centre), lines starting with `#` skipped; written as
//!   `AffineTransform_double_3_3` with 17 significant digits and the centre 0.
//! - Voxel space (`--invox`, `--outvox`): the 4 x 4 matrix in the source's voxels from the target
//!   to the source; written with 17 significant digits.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-lta-convert.md`):
//! - **FS-1161**: the ITK reader takes the end of a file after white space (the usual new line
//!   after the last line) as a repeat of the last line: a last `FixedParameters:` applies its centre
//!   twice (a wrong translation), a last `Parameters:` rewrites the matrix's rows with the last three
//!   numbers, a last `Transform:` or comment ends the program as unknown. By default the reading
//!   ends there (rule A).
//! - **FS-1162**: the ITK reader never ends when a number of `Parameters:` or `FixedParameters:`
//!   cannot be read before the end of the file (it repeats the line for ever). larmorx ends with a
//!   message, status 1, in both modes.
//! - **FS-1163**: the NiftyReg, ITK and voxel-space readers given fewer numbers than they need, or
//!   text that is not a number: a number not read keeps the one read before it, one before any
//!   number is memory contents. By default the program ends with a message, status 1 (rule B);
//!   `reproduce` takes FreeSurfer's values (0 for memory contents).
//! - **FS-1164**: a matrix that cannot be inverted (the NiftyReg, ITK and voxel-space readers, the
//!   NiftyReg, ITK and voxel-space writers): FreeSurfer goes on without a matrix (or, reading voxel
//!   space, with the inverse's leftovers) and crashes where it uses it. By default the program ends
//!   with a message, status 1 (rule B); `reproduce` crashes where FreeSurfer does (status 139).
//! - **FS-1165**: a transform the FSL or `register.dat` reader could not read, or an `.lta` without
//!   a transform: FreeSurfer goes on and crashes (after reading both volumes). By default the
//!   program ends with a message, status 1 (rule B).

use std::io::Write;

use larmorx_core::defects::{FS_1161, FS_1162, FS_1163, FS_1164, FS_1165};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{ERROR_BADFILE, Fatal, NO_ERROR, error_exit};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply_d};
use larmorx_freesurfer_core::matrix_ops::{matrix_identity, matrix_inverse};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_gca::transform::GcaLtaIo;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::mriio::mri_read_header;
use larmorx_freesurfer_mri::lta::{
    FSLREG_TYPE, LINEAR_RAS_TO_RAS, Lta, MNI_TRANSFORM_TYPE, REGISTER_DAT, lta_alloc,
};
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::vol_geom::extract_i_to_r;
use larmorx_freesurfer_volume::ltaops::lta_change_type;
use larmorx_freesurfer_volume::ltaread::lta_read_ex_type;
use larmorx_freesurfer_volume::ltawrite::{lta_read, lta_write};
use larmorx_freesurfer_volume::vgeom::{
    get_vol_geom, vg_get_ras_to_voxel_xform, vg_get_voxel_to_ras_xform,
};

use super::cxx::{CxxIn, put};
use super::main::Run;

/// The status of a crash FreeSurfer's build meets (SIGSEGV).
pub const CRASH: i32 = 139;

/// `std::ifstream(name)`: the file's bytes, or `None` when it does not open (a folder opens, and
/// reads as empty).
fn open_in(r: &Run, name: &str) -> Option<CxxIn> {
    let mut f = CFile::open(r.dir, name, "r").ok()?;
    let mut data = Vec::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let n = f.read_bytes(&mut buf);
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
    }
    f.close();
    Some(CxxIn::new(data))
}

/// `std::ofstream(name)` and its text, written whole; `false` when it does not open.
fn write_out(r: &Run, name: &str, text: &[u8]) -> bool {
    match CFile::open(r.dir, name, "w") {
        Ok(mut f) => {
            // the C++ stream's write errors are not checked
            let _ = f.write_all(text);
            f.close();
            true
        }
        Err(_) => false,
    }
}

/// `MRIreadHeader(name, MRI_VOLUME_TYPE_UNKNOWN)`, or the program's end with `<what><name>` on
/// standard error (status 1).
fn header(g: &mut Globals, r: &mut Run, name: &str, what: &str) -> Result<Mri, Fatal> {
    match mri_read_header(g, r.out, r.err, r.env, r.dir, name, MRI_VOLUME_TYPE_UNKNOWN)? {
        Some(m) => Ok(m),
        None => {
            let _ = writeln!(r.err, "{what}{name}");
            Err(Fatal::Exit(1))
        }
    }
}

/// A `float` variable of the readers: `None` while never assigned (memory contents in
/// FreeSurfer's build; read as 0 with FS-1163).
struct Var(Option<f32>);

impl Var {
    fn read(&mut self, s: &mut CxxIn) {
        if let Some(v) = s.float() {
            self.0 = Some(v);
        }
    }
    fn get(&self, g: &mut Globals, what: &str) -> f32 {
        match self.0 {
            Some(v) => v,
            None => {
                g.notices.add(
                    &FS_1163,
                    format!("lta_convert: {what} read before any number (memory contents in FreeSurfer's build; 0 here)"),
                );
                0.0
            }
        }
    }
}

/// FS-1163's end by default: a reader that could not read all its numbers.
fn short_read(g: &mut Globals, r: &mut Run, reader: &str, name: &str) -> Result<(), Fatal> {
    g.notices.add(
        &FS_1163,
        format!("lta_convert: {reader} of {name}: a number missing or not a number (FreeSurfer keeps the one read before it)"),
    );
    if g.notices.reproduce() {
        return Ok(());
    }
    let _ = writeln!(
        r.err,
        "ERROR {reader}: cannot read {name}: a number is missing or is not a number"
    );
    Err(Fatal::Exit(1))
}

/// FS-1164: a matrix that could not be inverted. By default the program's end (status 1); with
/// `reproduce` the caller goes on as FreeSurfer does.
pub fn not_invertible(g: &mut Globals, r: &mut Run, what: &str) -> Result<(), Fatal> {
    g.notices.add(
        &FS_1164,
        format!("lta_convert: {what}: the matrix cannot be inverted (FreeSurfer goes on without it and crashes where it uses it)"),
    );
    if g.notices.reproduce() {
        return Ok(());
    }
    let _ = writeln!(r.err, "ERROR {what}: the matrix cannot be inverted");
    Err(Fatal::Exit(1))
}

/// FS-1165: a reader that gave no transform where FreeSurfer goes on with one.
pub fn no_transform(g: &mut Globals, r: &mut Run, reader: &str, name: &str) -> Fatal {
    g.notices.add(
        &FS_1165,
        format!(
            "lta_convert: {reader} gave no transform from {name} (FreeSurfer goes on and crashes)"
        ),
    );
    if g.notices.reproduce() {
        return Fatal::Exit(CRASH);
    }
    let _ = writeln!(r.err, "ERROR {reader}: cannot read {name}");
    Fatal::Exit(1)
}

/// `MatrixInverse(m, NULL)`.
fn inverse(g: &mut Globals, r: &mut Run, m: Option<&Matrix>) -> Result<Option<Matrix>, Fatal> {
    Ok(matrix_inverse(g, r.out, r.err, m, None)?.into_new())
}

/// `MatrixInverse(m, lt->m_L)` assigned to `lt->m_L` (`NULL` when it fails).
fn invert_into(
    g: &mut Globals,
    r: &mut Run,
    m: &Matrix,
    target: &mut Option<Matrix>,
) -> Result<(), Fatal> {
    match matrix_inverse(g, r.out, r.err, Some(m), target.as_mut())? {
        Ret::New(x) => *target = Some(x),
        Ret::Given => {}
        Ret::Null => *target = None,
    }
    Ok(())
}

/// `MatrixMultiplyD(m1, m2, m3)` with `m3` one of the inputs (FreeSurfer copies it first).
fn mul_into(
    g: &mut Globals,
    r: &mut Run,
    m1: Option<&Matrix>,
    m2: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    Ok(matrix_multiply_d(g, r.out, r.err, m1, m2, None)?.into_new())
}

/// A new transform array as the NiftyReg, ITK and voxel-space readers start it: one transform,
/// `sigma` 1, centre 0, `LINEAR_RAS_TO_RAS`.
fn new_ras_lta(g: &mut Globals, r: &mut Run) -> Result<Lta, Fatal> {
    let mut lta = lta_alloc(g, r.out, r.err, 1, None)?;
    let lt = &mut lta.xforms[0];
    lt.sigma = 1.0;
    lt.x0 = 0.0;
    lt.y0 = 0.0;
    lt.z0 = 0.0;
    lta.ltype = LINEAR_RAS_TO_RAS;
    Ok(lta)
}

/// `readLTA(xfname, sname, tname)` (`LTAread`).
pub fn read_lta(
    g: &mut Globals,
    r: &mut Run,
    xfname: &str,
    sname: &str,
    tname: &str,
) -> Result<Lta, Fatal> {
    let env = r.env;
    let mut io = GcaLtaIo { env };
    let Some(mut lta) = lta_read(g, r.out, r.err, r.dir, xfname, &mut io)? else {
        let _ = writeln!(r.err, "ERROR readLTA: cannot read {xfname}");
        return Err(Fatal::Exit(1));
    };
    if lta.ltype != LINEAR_RAS_TO_RAS {
        lta_change_type(g, r.out, r.err, env, &mut lta, LINEAR_RAS_TO_RAS)?;
    }
    if lta.ltype != LINEAR_RAS_TO_RAS {
        let _ = writeln!(r.err, "ERROR readLTA: cannot change type to RAS_TO_RAS.");
        return Err(Fatal::Exit(1));
    }
    if lta.xforms.is_empty() && (!sname.is_empty() || !tname.is_empty()) {
        return Err(no_transform(g, r, "readLTA", xfname));
    }
    if !sname.is_empty() {
        let src = header(g, r, sname, "ERROR readLTA: cannot read src MRI")?;
        lta.xforms[0].src.assign(g, &src.vg);
        lta.xforms[0].src.valid = 1;
    }
    if !tname.is_empty() {
        let trg = header(g, r, tname, "ERROR readFSL: cannot read trg MRI")?;
        lta.xforms[0].dst.assign(g, &trg.vg);
        lta.xforms[0].dst.valid = 1;
    }
    Ok(lta)
}

/// The FSL and `register.dat` readers: the file read as `ty` (`LTAreadExType`), both volumes'
/// headers, their geometries, `LINEAR_RAS_TO_RAS`.
fn read_typed(
    g: &mut Globals,
    r: &mut Run,
    xfname: &str,
    sname: &str,
    tname: &str,
    ty: i32,
    reader: &str,
) -> Result<Lta, Fatal> {
    let env = r.env;
    let mut io = GcaLtaIo { env };
    let lta = lta_read_ex_type(g, r.out, r.err, r.dir, xfname, ty, &mut io)?;
    let src = header(g, r, sname, &format!("ERROR {reader}: cannot read src MRI"))?;
    let trg = header(g, r, tname, &format!("ERROR {reader}: cannot read trg MRI"))?;
    let Some(mut lta) = lta.filter(|l| !l.xforms.is_empty()) else {
        return Err(no_transform(g, r, reader, xfname));
    };
    get_vol_geom(&src.vg, &mut lta.xforms[0].src);
    get_vol_geom(&trg.vg, &mut lta.xforms[0].dst);
    lta_change_type(g, r.out, r.err, env, &mut lta, LINEAR_RAS_TO_RAS)?;
    Ok(lta)
}

/// `readFSL(xfname, sname, tname)`.
pub fn read_fsl(
    g: &mut Globals,
    r: &mut Run,
    xfname: &str,
    sname: &str,
    tname: &str,
) -> Result<Lta, Fatal> {
    read_typed(g, r, xfname, sname, tname, FSLREG_TYPE, "readFSL")
}

/// `readREG(xfname, sname, tname)`.
pub fn read_reg(
    g: &mut Globals,
    r: &mut Run,
    xfname: &str,
    sname: &str,
    tname: &str,
) -> Result<Lta, Fatal> {
    read_typed(g, r, xfname, sname, tname, REGISTER_DAT, "readREG")
}

/// `readMNI(xfname, sname, tname)`: a file that does not read ends the program (`can't read input
/// file <name>`, `ERROR_BADFILE`).
pub fn read_mni(
    g: &mut Globals,
    r: &mut Run,
    xfname: &str,
    sname: &str,
    tname: &str,
) -> Result<Lta, Fatal> {
    let mut io = GcaLtaIo { env: r.env };
    let lta = lta_read_ex_type(g, r.out, r.err, r.dir, xfname, MNI_TRANSFORM_TYPE, &mut io)?;
    let Some(mut lta) = lta else {
        error_exit(
            g,
            r.err,
            ERROR_BADFILE,
            &format!("can't read input file {xfname}"),
        )?;
        unreachable!("ErrorExit returns only for code 0");
    };
    let src = header(g, r, sname, "ERROR readMNI: cannot read src MRI")?;
    let trg = header(g, r, tname, "ERROR readMNI: cannot read trg MRI")?;
    if lta.xforms.is_empty() {
        return Err(no_transform(g, r, "readMNI", xfname));
    }
    get_vol_geom(&src.vg, &mut lta.xforms[0].src);
    get_vol_geom(&trg.vg, &mut lta.xforms[0].dst);
    Ok(lta)
}

/// `readNIFTYREG(xfname, sname, tname, dim)`.
pub fn read_niftyreg(
    g: &mut Globals,
    r: &mut Run,
    xfname: &str,
    sname: &str,
    tname: &str,
    dim: i32,
) -> Result<Lta, Fatal> {
    let mut lta = new_ras_lta(g, r)?;
    let Some(mut s) = open_in(r, xfname) else {
        let _ = writeln!(r.err, "readNIFTYREG Error opening {xfname}");
        return Err(Fatal::Exit(1));
    };
    let mut m_l = matrix_identity(g, r.out, r.err, 4, None)?
        .into_new()
        .expect("MatrixIdentity without an output returns a new matrix");
    let mut v = [Var(None), Var(None), Var(None), Var(None)];
    let mut row = 1;
    while !s.eof() {
        if dim == 3 {
            for x in v.iter_mut() {
                x.read(&mut s);
            }
            for (c, x) in v.iter().enumerate() {
                let val = x.get(g, "readNIFTYREG: a matrix element");
                m_l.set_relt(row, c as i32 + 1, val);
            }
        }
        if dim == 2 {
            for x in v[..3].iter_mut() {
                x.read(&mut s);
            }
            for (c, x) in [1, 2, 4].into_iter().zip(&v[..3]) {
                let val = x.get(g, "readNIFTYREG: a matrix element");
                m_l.set_relt(row, c, val);
            }
        }
        row += 1;
        if (dim == 3 && row > 4) || (dim == 2 && row > 2) {
            break;
        }
    }
    if s.fail() {
        short_read(g, r, "readNIFTYREG", xfname)?;
    }
    invert_into(g, r, &m_l, &mut lta.xforms[0].m_l)?;
    if lta.xforms[0].m_l.is_none() {
        not_invertible(g, r, "readNIFTYREG")?;
    }
    let src = header(g, r, sname, "ERROR readNIFTYREG: cannot read src MRI")?;
    let trg = header(g, r, tname, "ERROR readNIFTYREG: cannot read trg MRI")?;
    get_vol_geom(&src.vg, &mut lta.xforms[0].src);
    get_vol_geom(&trg.vg, &mut lta.xforms[0].dst);
    Ok(lta)
}

/// The ITK reader's state after a line: `Some` of the program's end.
enum ItkLine {
    Go,
    End(Fatal),
}

/// The translation `FixedParameters:` makes: `t_i + f_i - sum_j R_ij f_j` (in `float`, in that
/// order).
fn fixed_update(m_r: &Matrix, v_t: &Matrix, v_f: &Matrix) -> [f32; 3] {
    [1, 2, 3].map(|i| {
        let mut ti = v_t.relt(i, 1) + v_f.relt(i, 1);
        for j in 1..=3 {
            ti -= m_r.relt(i, j) * v_f.relt(j, 1);
        }
        ti
    })
}

/// `readITK(xfname, sname, tname)`.
pub fn read_itk(
    g: &mut Globals,
    r: &mut Run,
    xfname: &str,
    sname: &str,
    tname: &str,
) -> Result<Lta, Fatal> {
    let src = header(g, r, sname, "ERROR readITK: cannot read src MRI ")?;
    let trg = header(g, r, tname, "ERROR readITK: cannot read trg MRI ")?;
    let (s, t) = (&src.vg, &trg.vg);
    let src3d = s.width > 1 && s.height > 1 && s.depth > 1;
    let trg3d = t.width > 1 && t.height > 1 && t.depth > 1;
    let mri3d = src3d || trg3d;
    let mut flatdim = -1;
    if !mri3d {
        if s.width == 1 && t.width == 1 {
            flatdim = 1;
        } else if s.height == 1 && t.height == 1 {
            flatdim = 2;
        } else if s.depth == 1 && t.depth == 1 {
            flatdim = 3;
        } else {
            let _ = writeln!(
                r.err,
                "ERROR readITK: 2D images but plane does not agree. {tname}"
            );
            return Err(Fatal::Exit(1));
        }
    }
    let o1 = i32::from(flatdim == 1);
    let o2 = i32::from(flatdim == 2);
    let mut lta = new_ras_lta(g, r)?;
    let Some(mut st) = open_in(r, xfname) else {
        let _ = writeln!(r.err, "ERROR readITK:  opening {xfname}");
        return Err(Fatal::Exit(1));
    };
    let mut m_l = matrix_identity(g, r.out, r.err, 4, None)?
        .into_new()
        .expect("MatrixIdentity without an output returns a new matrix");
    let mut m_r = Matrix::real(3, 3);
    let mut v_t = Matrix::real(3, 1);
    let mut v_f = Matrix::real(3, 1);
    let mut word: Vec<u8> = Vec::new();
    let mut is3d = true;
    let (mut v1, mut v2, mut v3) = (Var(None), Var(None), Var(Some(0.0)));
    let what = "readITK: a parameter";
    // the last section read (`Transform:`, `Parameters:`, `FixedParameters:`)
    let mut last: Option<Vec<u8>> = None;
    while !st.eof() {
        while st.peek() == Some(b'#') {
            st.getline(&mut word);
        }
        let got = st.word(&mut word);
        if !got && st.eof() && last.is_some() {
            // FS-1161: FreeSurfer takes the end after white space as the word it read last
            let changes = if word == b"FixedParameters:" {
                is3d && fixed_update(&m_r, &v_t, &v_f) != [1, 2, 3].map(|i| v_t.relt(i, 1))
            } else {
                true
            };
            if changes {
                let shown = String::from_utf8_lossy(&word).into_owned();
                g.notices.add(
                    &FS_1161,
                    if g.notices.reproduce() {
                        format!("lta_convert: readITK: the end of {xfname} taken as a repeat of {shown}, as in FreeSurfer")
                    } else {
                        format!("lta_convert: readITK: the end of {xfname} ends the reading (FreeSurfer repeats {shown})")
                    },
                );
            }
            if g.notices.fix() {
                break;
            }
        }
        if got {
            last = Some(word.clone());
        }
        let line = match word.as_slice() {
            b"Transform:" => {
                st.word(&mut word);
                let _ = r.out.write_all(b"Transform: ");
                let _ = r.out.write_all(&word);
                let _ = r.out.write_all(b"\n");
                if word == b"AffineTransform_double_2_2" {
                    is3d = false;
                } else if word != b"AffineTransform_double_3_3"
                    && word != b"MatrixOffsetTransformBase_double_3_3"
                {
                    let _ = r.out.write_all(b"ERROR readITK: Transform type unknown!\n");
                    return Err(Fatal::Exit(1));
                }
                if !is3d && mri3d {
                    let _ = r
                        .out
                        .write_all(b"ERROR readITK: Transform is 2D but images 3D?\n");
                    return Err(Fatal::Exit(1));
                }
                ItkLine::Go
            }
            b"Parameters:" => {
                if is3d {
                    // LPS to RAS: m_L = diag(-1,-1,1,1) m_R diag(-1,-1,1,1); m_R the input
                    for row in 1..=3 {
                        v1.read(&mut st);
                        v2.read(&mut st);
                        v3.read(&mut st);
                        let (a, b, c) = (v1.get(g, what), v2.get(g, what), v3.get(g, what));
                        m_r.set_relt(row, 1, a);
                        m_r.set_relt(row, 2, b);
                        m_r.set_relt(row, 3, c);
                        if row < 3 {
                            m_l.set_relt(row, 1, a);
                            m_l.set_relt(row, 2, b);
                            m_l.set_relt(row, 3, -c);
                        } else {
                            m_l.set_relt(row, 1, -a);
                            m_l.set_relt(row, 2, -b);
                            m_l.set_relt(row, 3, c);
                        }
                    }
                    v1.read(&mut st);
                    v2.read(&mut st);
                    v3.read(&mut st);
                    let (a, b, c) = (v1.get(g, what), v2.get(g, what), v3.get(g, what));
                    v_t.set_relt(1, 1, a);
                    v_t.set_relt(2, 1, b);
                    v_t.set_relt(3, 1, c);
                    m_l.set_relt(1, 4, -a);
                    m_l.set_relt(2, 4, -b);
                    m_l.set_relt(3, 4, c);
                } else {
                    let (r1, c1, r2, c2) = (1 + o1, 1 + o1, 2 + o1 + o2, 2 + o1 + o2);
                    v1.read(&mut st);
                    v2.read(&mut st);
                    m_l.set_relt(r1, c1, v1.get(g, what));
                    m_l.set_relt(r1, c2, v2.get(g, what));
                    v1.read(&mut st);
                    v2.read(&mut st);
                    m_l.set_relt(r2, c1, v1.get(g, what));
                    m_l.set_relt(r2, c2, v2.get(g, what));
                    v1.read(&mut st);
                    v2.read(&mut st);
                    m_l.set_relt(r1, 4, -v1.get(g, what));
                    m_l.set_relt(r2, 4, -v2.get(g, what));
                }
                ItkLine::Go
            }
            b"FixedParameters:" => {
                if is3d {
                    v1.read(&mut st);
                    v2.read(&mut st);
                    v3.read(&mut st);
                    v_f.set_relt(1, 1, v1.get(g, what));
                    v_f.set_relt(2, 1, v2.get(g, what));
                    v_f.set_relt(3, 1, v3.get(g, what));
                    let t = fixed_update(&m_r, &v_t, &v_f);
                    for (i, ti) in (1..=3).zip(t) {
                        v_t.set_relt(i, 1, ti);
                    }
                    m_l.set_relt(1, 4, -v_t.relt(1, 1));
                    m_l.set_relt(2, 4, -v_t.relt(2, 1));
                    m_l.set_relt(3, 4, v_t.relt(3, 1));
                    ItkLine::Go
                } else {
                    v1.read(&mut st);
                    v2.read(&mut st);
                    let (a, b) = (v1.get(g, what), v2.get(g, what));
                    if a != 0.0 || b != 0.0 {
                        let _ = writeln!(
                            r.out,
                            "ERROR readITK: 2D fixedParameters not equal to zero (not implemented): {} {}",
                            put(f64::from(a), 6),
                            put(f64::from(b), 6)
                        );
                        ItkLine::End(Fatal::Exit(1))
                    } else {
                        ItkLine::Go
                    }
                }
            }
            _ => {
                let _ = r.out.write_all(b"ERROR readITK: ");
                let _ = r.out.write_all(&word);
                let _ = r.out.write_all(b" unknown!\n");
                ItkLine::End(Fatal::Exit(1))
            }
        };
        if let ItkLine::End(f) = line {
            return Err(f);
        }
        if st.fail() && !st.eof() {
            // a number that did not read: FreeSurfer repeats this line for ever
            let section = String::from_utf8_lossy(&word).into_owned();
            g.notices.add(
                &FS_1162,
                format!("lta_convert: readITK: a number of {section} in {xfname} is not a number (FreeSurfer never ends)"),
            );
            let _ = writeln!(
                r.err,
                "ERROR readITK: cannot read {xfname}: a number of {section} is not a number (FreeSurfer's lta_convert never ends here)"
            );
            return Err(Fatal::Exit(1));
        }
        if st.fail() && got {
            short_read(g, r, "readITK", xfname)?;
        }
    }
    invert_into(g, r, &m_l, &mut lta.xforms[0].m_l)?;
    if lta.xforms[0].m_l.is_none() {
        not_invertible(g, r, "readITK")?;
    }
    get_vol_geom(s, &mut lta.xforms[0].src);
    get_vol_geom(t, &mut lta.xforms[0].dst);
    Ok(lta)
}

/// `readVOX(xfname, sname, tname)`.
pub fn read_vox(
    g: &mut Globals,
    r: &mut Run,
    xfname: &str,
    sname: &str,
    tname: &str,
) -> Result<Lta, Fatal> {
    let src = header(g, r, sname, "ERROR readVOX: cannot read src MRI")?;
    let trg = header(g, r, tname, "ERROR readVOX: cannot read trg MRI")?;
    let mut lta = new_ras_lta(g, r)?;
    get_vol_geom(&src.vg, &mut lta.xforms[0].src);
    get_vol_geom(&trg.vg, &mut lta.xforms[0].dst);
    let Some(mut s) = open_in(r, xfname) else {
        let _ = writeln!(r.err, "readVOX error opening {xfname}");
        return Err(Fatal::Exit(1));
    };
    let mut mat = matrix_alloc(g, r.out, r.err, 4, 4, MATRIX_REAL)?.expect("a 4 x 4 matrix");
    let mut v = [Var(None), Var(None), Var(None), Var(None)];
    let mut row = 1;
    while !s.eof() {
        for x in v.iter_mut() {
            x.read(&mut s);
        }
        for (c, x) in v.iter().enumerate() {
            let val = x.get(g, "readVOX: a matrix element");
            mat.set_relt(row, c as i32 + 1, val);
        }
        row += 1;
        if row > 4 {
            break;
        }
    }
    if s.fail() {
        short_read(g, r, "readVOX", xfname)?;
    }
    let src_to_ras = extract_i_to_r(g, r.out, r.err, &src.vg)?;
    let ras_to_src = inverse(g, r, Some(&src_to_ras))?;
    let m1 = mul_into(g, r, Some(&mat), ras_to_src.as_ref())?;
    let m2 = mul_into(g, r, Some(&src_to_ras), m1.as_ref())?
        .expect("MatrixMultiplyD of two 4 x 4 matrices");
    // the inverse's result is not checked: a failure leaves the matrix as the inverse left it
    let ok = !matches!(
        matrix_inverse(g, r.out, r.err, Some(&m2), lta.xforms[0].m_l.as_mut())?,
        Ret::Null
    );
    if !ok {
        not_invertible(g, r, "readVOX")?;
    }
    Ok(lta)
}

/// `shallowCopyLTA(lta)`: one transform with a copy of the first's matrix and both geometries
/// (`VOL_GEOM`'s assignment), the array's type, `fscale` and subject.
fn shallow_copy(g: &mut Globals, r: &mut Run, lta: &Lta) -> Result<Lta, Fatal> {
    let mut t = lta_alloc(g, r.out, r.err, 1, None)?;
    t.xforms[0].m_l = lta.xforms[0].m_l.clone();
    t.xforms[0].src.assign(g, &lta.xforms[0].src);
    t.xforms[0].dst.assign(g, &lta.xforms[0].dst);
    t.ltype = lta.ltype;
    t.fscale = lta.fscale;
    t.subject.strcpy(&lta.subject);
    Ok(t)
}

/// FS-1164 with `reproduce`: FreeSurfer's crash on a transform without its matrix.
fn crash_without_matrix(g: &mut Globals, lta: &Lta) -> Result<(), Fatal> {
    if lta.xforms[0].m_l.is_none() && g.notices.reproduce() {
        return Err(Fatal::Exit(CRASH));
    }
    Ok(())
}

/// `LTAwrite` of a copy of `lta` changed to `ty` (`writeFSL`, `writeREG`) or given `ty`
/// (`writeMNI`), with `<what>: cannot create file <name>` when it fails.
fn write_typed(
    g: &mut Globals,
    r: &mut Run,
    lta: &Lta,
    fname: &str,
    ty: i32,
    change: bool,
    what: &str,
) -> Result<(), Fatal> {
    crash_without_matrix(g, lta)?;
    let env = r.env;
    let mut t = shallow_copy(g, r, lta)?;
    if change {
        if t.ltype != ty {
            lta_change_type(g, r.out, r.err, env, &mut t, ty)?;
        }
    } else {
        t.ltype = ty;
    }
    if lta_write(g, r.out, r.err, env, r.dir, &t, fname)? != NO_ERROR {
        let _ = writeln!(r.err, "ERROR {what}: cannot create file {fname}");
        return Err(Fatal::Exit(1));
    }
    Ok(())
}

/// `writeFSL(fname, lta)`.
pub fn write_fsl(g: &mut Globals, r: &mut Run, fname: &str, lta: &Lta) -> Result<(), Fatal> {
    write_typed(g, r, lta, fname, FSLREG_TYPE, true, "writeFSL")
}

/// `writeMNI(fname, lta)`: the array must be `LINEAR_RAS_TO_RAS` (else `ERROR: lta should be
/// RAS_TO_RAS by now!!!`, status 1).
pub fn write_mni(g: &mut Globals, r: &mut Run, fname: &str, lta: &Lta) -> Result<(), Fatal> {
    if lta.ltype != LINEAR_RAS_TO_RAS {
        let _ = writeln!(r.err, "ERROR: lta should be RAS_TO_RAS by now!!!");
        return Err(Fatal::Exit(1));
    }
    write_typed(g, r, lta, fname, MNI_TRANSFORM_TYPE, false, "writeFSL")
}

/// `writeREG(fname, lta)`.
pub fn write_reg(g: &mut Globals, r: &mut Run, fname: &str, lta: &Lta) -> Result<(), Fatal> {
    write_typed(g, r, lta, fname, REGISTER_DAT, true, "writeREG")
}

/// The inverse of a `LINEAR_RAS_TO_RAS` copy's matrix (NiftyReg, ITK, voxel space), FS-1164 for
/// one that cannot be inverted (`reproduce`: the output created, then FreeSurfer's crash).
fn ras_inverse(
    g: &mut Globals,
    r: &mut Run,
    lta: &Lta,
    fname: &str,
    what: &str,
) -> Result<Matrix, Fatal> {
    let env = r.env;
    let mut t = shallow_copy(g, r, lta)?;
    if t.ltype != LINEAR_RAS_TO_RAS {
        lta_change_type(g, r.out, r.err, env, &mut t, LINEAR_RAS_TO_RAS)?;
    }
    let mut m = matrix_alloc(g, r.out, r.err, 4, 4, MATRIX_REAL)?.expect("a 4 x 4 matrix");
    match matrix_inverse(g, r.out, r.err, t.xforms[0].m_l.as_ref(), Some(&mut m))? {
        Ret::Null => {
            not_invertible(g, r, what)?;
            // FreeSurfer opens its output, then reads the missing matrix
            let _ = write_out(r, fname, b"");
            Err(Fatal::Exit(CRASH))
        }
        Ret::New(x) => Ok(x),
        Ret::Given => Ok(m),
    }
}

/// The rows of a 4 x 4 matrix with `precision` significant digits, one blank between.
fn rows_text(m: &Matrix, precision: usize) -> Vec<u8> {
    let mut s = Vec::new();
    for row in 1..=4 {
        for col in 1..=4 {
            s.extend_from_slice(put(f64::from(m.relt(row, col)), precision).as_bytes());
            if col < 4 {
                s.push(b' ');
            }
        }
        s.push(b'\n');
    }
    s
}

/// `writeNIFTYREG(fname, lta)`: the inverse with 7 significant digits.
pub fn write_niftyreg(g: &mut Globals, r: &mut Run, fname: &str, lta: &Lta) -> Result<(), Fatal> {
    let m = ras_inverse(g, r, lta, fname, "writeNIFTYREG")?;
    if !write_out(r, fname, &rows_text(&m, 7)) {
        let _ = writeln!(r.err, "writeNIFTYREG error opening {fname}");
        return Err(Fatal::Exit(1));
    }
    Ok(())
}

/// `writeITK(fname, lta)`: the inverse in LPS (the third column's first two rows, the third row's
/// first two columns and the translation's first two negated), 17 significant digits.
pub fn write_itk(g: &mut Globals, r: &mut Run, fname: &str, lta: &Lta) -> Result<(), Fatal> {
    let mut m = ras_inverse(g, r, lta, fname, "writeITK")?;
    for (row, col) in [(1, 3), (2, 3), (3, 1), (3, 2), (1, 4), (2, 4)] {
        m.set_relt(row, col, -m.relt(row, col));
    }
    let mut s = Vec::new();
    s.extend_from_slice(b"#Insight Transform File V1.0\n#Transform 0\n");
    s.extend_from_slice(b"Transform: AffineTransform_double_3_3\nParameters:");
    for row in 1..4 {
        for col in 1..4 {
            s.push(b' ');
            s.extend_from_slice(put(f64::from(m.relt(row, col)), 17).as_bytes());
        }
    }
    for row in 1..4 {
        s.push(b' ');
        s.extend_from_slice(put(f64::from(m.relt(row, 4)), 17).as_bytes());
    }
    s.extend_from_slice(b"\nFixedParameters: 0 0 0\n");
    if !write_out(r, fname, &s) {
        let _ = writeln!(r.err, "writeITK Error opening {fname}");
        return Err(Fatal::Exit(1));
    }
    Ok(())
}

/// `writeVOX(fname, lta)`: the inverse in the source's voxels (`VGgetRasToVoxelXform` times it
/// times `VGgetVoxelToRasXform` of the first transform's source geometry), 17 significant digits.
pub fn write_vox(g: &mut Globals, r: &mut Run, fname: &str, lta: &Lta) -> Result<(), Fatal> {
    let mat = ras_inverse(g, r, lta, fname, "writeVOX")?;
    let src = &lta.xforms[0].src;
    let src_to_ras = vg_get_voxel_to_ras_xform(g, r.out, r.err, src, 0)?;
    let ras_to_src = vg_get_ras_to_voxel_xform(g, r.out, r.err, src, 0)?;
    let m1 = mul_into(g, r, Some(&mat), Some(&src_to_ras))?;
    let m2 = mul_into(g, r, ras_to_src.as_ref(), m1.as_ref())?
        .expect("MatrixMultiplyD of two 4 x 4 matrices");
    if !write_out(r, fname, &rows_text(&m2, 17)) {
        let _ = writeln!(r.err, "writeVOX error opening {fname}");
        return Err(Fatal::Exit(1));
    }
    Ok(())
}
