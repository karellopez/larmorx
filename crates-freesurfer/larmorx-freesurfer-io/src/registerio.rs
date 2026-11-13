// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Registration files (`utils/registerio.cpp`, item 3.1-34, taken early for track D's surfaces:
//! the orchestrator, 2026-11-13): `regio_read_register`, with `float2int_code` (`utils/resample.cpp`,
//! the method names it accepts).
//!
//! A name ending in `.lta` (any case) is read as a linear transform array through the caller's
//! [`RegisterLta`] (`LTAread` and `TransformLTA2RegDat` belong to fs.volume, a crate above this
//! one); any other name as tkregister's `register.dat`.
//!
//! **FS-258** (memory): FreeSurfer reads the subject's name and the method's name into a buffer of
//! 1,000 bytes (a longer word writes past it), and the sizes of an `.lta` without transforms from
//! memory past its list. Both modes keep the whole word, and 0 for the sizes, noted.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::FS_258;
use larmorx_freesurfer_core::cscan::Scanf;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, strerror};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::utils;
use larmorx_freesurfer_mri::lta::Lta;

use crate::cfile::CFile;

/// `FLT2INT_ROUND`, `FLT2INT_FLOOR`, `FLT2INT_TKREG` (`resample.h`): how a coordinate becomes a
/// voxel index.
pub const FLT2INT_ROUND: i32 = 0;
pub const FLT2INT_FLOOR: i32 = 1;
pub const FLT2INT_TKREG: i32 = 2;

/// The word buffer's size (`char tmp[1000]`).
const WORD: usize = 1000;

/// `float2int_code(name)`: `round` or `rint` [`FLT2INT_ROUND`], `floor` [`FLT2INT_FLOOR`] (any
/// case), a name starting with `tkreg` (any case) [`FLT2INT_TKREG`], anything else -1.
pub fn float2int_code(name: &[u8]) -> i32 {
    let l = name.to_ascii_lowercase();
    if l == b"round" || l == b"rint" {
        FLT2INT_ROUND
    } else if l == b"floor" {
        FLT2INT_FLOOR
    } else if l.starts_with(b"tkreg") {
        FLT2INT_TKREG
    } else {
        -1
    }
}

/// fs.volume's functions `regio_read_register` calls for an `.lta` (the caller's).
pub trait RegisterLta {
    /// `LTAread(name)`: the transform array, or `None` (`NULL`, its messages written).
    fn lta_read(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        dir: Option<&Path>,
        name: &str,
    ) -> Result<Option<Lta>, Fatal>;
    /// `TransformLTA2RegDat(lta)`: the tkregister matrix of the array's first transform.
    fn lta_to_reg_dat(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        lta: &mut Lta,
    ) -> Result<Option<Matrix>, Fatal>;
}

/// A [`RegisterLta`] for callers without fs.volume: an `.lta` is not read (`larmorx-freesurfer:
/// reading an .lta registration needs fs.volume's LTAread` on standard error, `NULL`).
pub struct NoLta;

impl RegisterLta for NoLta {
    fn lta_read(
        &mut self,
        _g: &mut Globals,
        _out: &mut dyn Write,
        err: &mut dyn Write,
        _dir: Option<&Path>,
        _name: &str,
    ) -> Result<Option<Lta>, Fatal> {
        let _ = err.write_all(
            b"larmorx-freesurfer: reading an .lta registration needs fs.volume's LTAread\n",
        );
        Ok(None)
    }
    fn lta_to_reg_dat(
        &mut self,
        _g: &mut Globals,
        _out: &mut dyn Write,
        _err: &mut dyn Write,
        _lta: &mut Lta,
    ) -> Result<Option<Matrix>, Fatal> {
        Ok(None)
    }
}

/// `regio_read_register`'s outputs (the caller's variables: each keeps its value until the reader
/// sets it, as FreeSurfer's pointers do).
#[derive(Debug, Default)]
pub struct Register {
    /// `*subject` (`None`: never set).
    pub subject: Option<Vec<u8>>,
    pub inplaneres: f32,
    pub betplaneres: f32,
    pub intensity: f32,
    /// `*R`, the 4 x 4 registration matrix (`None`: never set).
    pub r: Option<Matrix>,
    pub float2int: i32,
}

fn perror(g: &Globals, err: &mut dyn Write, then: &str) {
    let _ = writeln!(err, "regio_read_register(): {}", strerror(g.errno));
    let _ = err.write_all(then.as_bytes());
}

fn word_fits(g: &mut Globals, w: &[u8]) {
    if w.len() >= WORD {
        g.notices.add(
            &FS_258,
            format!(
                "regio_read_register: a word of {} bytes kept whole (FreeSurfer's buffer holds 999)",
                w.len()
            ),
        );
    }
}

/// `regio_read_register(regfile, &subject, &inplaneres, &betplaneres, &intensity, &R,
/// &float2int)`: 0, or 1 on failure (the outputs set so far kept).
///
/// An `.lta` (`regio_read_register: loading lta` on standard output): its subject
/// (`subject-unknown` when empty), `fscale` as the intensity, the first transform's source voxel
/// sizes (x in-plane, z between planes), [`FLT2INT_ROUND`], the tkregister matrix; 1 when it does
/// not read. A `register.dat`: the subject (a word), the in-plane and between-plane resolutions
/// and the intensity (`%f`), sixteen `%f` row by row into a new 4 x 4 matrix, then a word: none
/// [`FLT2INT_TKREG`], else [`float2int_code`]'s (`ERROR: regio_read_register(): float2int method
/// <w> from file <name>, match not found` on standard output and 1 when it knows none). A file that
/// does not open, or a value that does not read, prints `regio_read_register(): <errno's text>`
/// and `Could not open <name>`, `Error reading subject from <name>`, `... inplaneres ...`, `...
/// betplaneres ...`, `... intensity ...` or `Error reading R[<r>][<c>] from <name>` (standard
/// error) and gives 1.
pub fn regio_read_register(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    regfile: &str,
    reg: &mut Register,
    lta_io: &mut dyn RegisterLta,
) -> Result<i32, Fatal> {
    let ext = utils::file_name_extension(g, regfile);
    if utils::stricmp(g, &ext, "LTA") == 0 {
        let _ = out.write_all(b"regio_read_register: loading lta\n");
        let Some(mut lta) = lta_io.lta_read(g, out, err, dir, regfile)? else {
            return Ok(1);
        };
        if lta.subject.is_empty() {
            lta.subject.strcpy_bytes(b"subject-unknown");
        }
        reg.subject = Some(lta.subject.c_bytes().to_vec());
        reg.intensity = lta.fscale;
        reg.float2int = FLT2INT_ROUND;
        match lta.xforms.first() {
            Some(x) => {
                reg.inplaneres = x.src.xsize;
                reg.betplaneres = x.src.zsize;
            }
            None => {
                g.notices.add(
                    &FS_258,
                    "regio_read_register of an .lta without transforms: its sizes 0 (FreeSurfer reads memory past the list)",
                );
                reg.inplaneres = 0.0;
                reg.betplaneres = 0.0;
            }
        }
        reg.r = lta_io.lta_to_reg_dat(g, out, err, &mut lta)?;
        return Ok(0);
    }
    let mut fp = match CFile::open(dir, regfile, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            perror(g, err, &format!("Could not open {regfile}\n"));
            return Ok(1);
        }
    };
    let mut errno = g.errno;
    // the subject's name
    let (n, w) = {
        let mut s = Scanf::new(&mut fp);
        let w = s.s(&mut errno, usize::MAX, true);
        (s.finish(&mut errno), w)
    };
    g.errno = errno;
    let Some(w) = w.filter(|_| n == 1) else {
        perror(g, err, &format!("Error reading subject from {regfile}\n"));
        return Ok(1);
    };
    word_fits(g, &w);
    reg.subject = Some(w);
    let read_f = |g: &mut Globals, fp: &mut CFile| -> Option<f32> {
        let mut errno = g.errno;
        let mut s = Scanf::new(fp);
        let v = s.f(&mut errno, true);
        let n = s.finish(&mut errno);
        g.errno = errno;
        v.filter(|_| n == 1)
    };
    for (what, slot) in [
        ("inplaneres", &mut reg.inplaneres),
        ("betplaneres", &mut reg.betplaneres),
        ("intensity", &mut reg.intensity),
    ] {
        match read_f(g, &mut fp) {
            Some(v) => *slot = v,
            None => {
                perror(g, err, &format!("Error reading {what} from {regfile}\n"));
                return Ok(1);
            }
        }
    }
    let Some(mut r) = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)? else {
        let _ = err.write_all(b"regio_read_register(): could not alloc R\n");
        return Ok(1);
    };
    for row in 0..4 {
        for c in 0..4 {
            match read_f(g, &mut fp) {
                Some(v) => r.set_relt(row + 1, c + 1, v),
                None => {
                    reg.r = Some(r);
                    perror(
                        g,
                        err,
                        &format!("Error reading R[{row}][{c}] from {regfile}\n"),
                    );
                    return Ok(1);
                }
            }
        }
    }
    reg.r = Some(r);
    let mut errno = g.errno;
    let (n, w) = {
        let mut s = Scanf::new(&mut fp);
        let w = s.s(&mut errno, usize::MAX, true);
        (s.finish(&mut errno), w)
    };
    g.errno = errno;
    drop(fp);
    if n == -1 {
        reg.float2int = FLT2INT_TKREG;
    } else {
        let w = w.unwrap_or_default();
        word_fits(g, &w);
        reg.float2int = float2int_code(&w);
        if reg.float2int == -1 {
            let _ = writeln!(
                out,
                "ERROR: regio_read_register(): float2int method {} from file {regfile}, match not found",
                String::from_utf8_lossy(&w)
            );
            return Ok(1);
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names() {
        assert_eq!(float2int_code(b"ROUND"), FLT2INT_ROUND);
        assert_eq!(float2int_code(b"rint"), FLT2INT_ROUND);
        assert_eq!(float2int_code(b"Floor"), FLT2INT_FLOOR);
        assert_eq!(float2int_code(b"tkregister"), FLT2INT_TKREG);
        assert_eq!(float2int_code(b"tkreg"), FLT2INT_TKREG);
        assert_eq!(float2int_code(b"tkre"), -1);
        assert_eq!(float2int_code(b""), -1);
    }

    #[test]
    fn reads_a_register_dat() {
        let mut g = Globals::new(larmorx_core::defects::UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("register.dat"),
            "bert\n1.5\n3\n0.2\n1 0 0 1\n0 1 0 2\n0 0 1 3\n0 0 0 1\nround\n",
        )
        .unwrap();
        let mut reg = Register::default();
        let r = regio_read_register(
            &mut g,
            &mut o,
            &mut e,
            Some(dir.path()),
            "register.dat",
            &mut reg,
            &mut NoLta,
        )
        .unwrap();
        assert_eq!(r, 0);
        assert_eq!(reg.subject.as_deref(), Some(b"bert".as_slice()));
        assert_eq!(
            (reg.inplaneres, reg.betplaneres, reg.intensity),
            (1.5, 3.0, 0.2)
        );
        assert_eq!(reg.float2int, FLT2INT_ROUND);
        assert_eq!(reg.r.as_ref().unwrap().relt(2, 4), 2.0);
    }
}
