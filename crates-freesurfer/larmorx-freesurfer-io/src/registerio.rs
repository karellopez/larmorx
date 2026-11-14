// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Registration files (`utils/registerio.cpp`, item 3.1-34; `regio_read_register` taken early for
//! track D's surfaces, the orchestrator, 2026-11-13): `regio_read_register` with `float2int_code`
//! (`utils/resample.cpp`, the method names it accepts), `regio_print_register`,
//! `regio_write_register`, `regio_read_mincxfm`, `regio_write_mincxfm`, `regio_read_registermat`.
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

/// `fgets` into a buffer of `n` bytes, as the C string it leaves (`None` at the end: the buffer
/// unchanged); a failed read's `errno` kept.
fn fgets_c(g: &mut Globals, fp: &mut CFile, n: usize) -> Option<Vec<u8>> {
    let l = fp.fgets(n);
    if l.is_none()
        && let Some(e) = fp.read_errno()
    {
        g.errno = e;
    }
    l.map(|l| {
        let end = l.iter().position(|&b| b == 0).unwrap_or(l.len());
        l[..end].to_vec()
    })
}

/// `fgetl(buf, n, fp)` with its return value unused: what the caller's buffer holds as a C string
/// afterwards (its leading white space kept: `fgetl` returns a pointer past it). Lines empty or
/// starting with `#` after white space are read and skipped; the line returned is cut at its first
/// `#` and its newline. At the end of the file the buffer holds the last line read, unchanged.
fn fgetl_into(g: &mut Globals, fp: &mut CFile, buf: &mut Vec<u8>, n: usize) -> bool {
    loop {
        let Some(line) = fgets_c(g, fp, n) else {
            return false;
        };
        *buf = line;
        let start = buf
            .iter()
            .position(|&b| !larmorx_freesurfer_core::cstd::is_space(b))
            .unwrap_or(buf.len());
        let cp = &buf[start..];
        if cp.is_empty() || cp[0] == b'#' || cp[0] == b'\n' {
            continue;
        }
        if let Some(h) = buf[start..].iter().position(|&b| b == b'#') {
            buf.truncate(start + h);
        }
        if buf.last() == Some(&b'\n') {
            buf.pop();
        }
        return true;
    }
}

/// `printf(fmt, x)` of one `double` (`%f`, `%e`, `%18.15e `).
fn fmt(f: &str, x: f64) -> String {
    larmorx_freesurfer_core::cstd::format_double(f, x).expect("a double's format")
}

/// `regio_print_register(fp, subject, inplaneres, betplaneres, intensity, R, float2int)`: the
/// subject (`subject-unknown` when none or empty), the three numbers (`%f`), the first three rows
/// of `R` (each number `%18.15e` and a space, then a newline), `0 0 0 1`, then the method's name
/// (`tkregister`, `floor`, `round`); 0. Another method: `ERROR: regio_print_register: f2i code =
/// <code>, unrecoginized` on standard output (FreeSurfer's spelling) after the matrix, no method
/// line, 1. `R` has at least 3 rows and 4 columns (every caller's 4 x 4).
#[allow(clippy::too_many_arguments)]
pub fn regio_print_register(
    fp: &mut dyn Write,
    out: &mut dyn Write,
    subject: Option<&[u8]>,
    inplaneres: f32,
    betplaneres: f32,
    intensity: f32,
    r: &Matrix,
    float2int: i32,
) -> i32 {
    let mut w: Vec<u8> = Vec::new();
    match subject.map(|s| &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())]) {
        Some(s) if !s.is_empty() => {
            w.extend_from_slice(s);
            w.push(b'\n');
        }
        _ => w.extend_from_slice(b"subject-unknown\n"),
    }
    for v in [inplaneres, betplaneres, intensity] {
        w.extend_from_slice(fmt("%f\n", f64::from(v)).as_bytes());
    }
    for row in 1..=3 {
        for col in 1..=4 {
            w.extend_from_slice(fmt("%18.15e ", f64::from(r.relt(row, col))).as_bytes());
        }
        w.push(b'\n');
    }
    w.extend_from_slice(b"0 0 0 1\n");
    let method = match float2int {
        FLT2INT_TKREG => "tkregister",
        FLT2INT_FLOOR => "floor",
        FLT2INT_ROUND => "round",
        _ => {
            let _ = fp.write_all(&w);
            let _ = writeln!(
                out,
                "ERROR: regio_print_register: f2i code = {float2int}, unrecoginized"
            );
            return 1;
        }
    };
    w.extend_from_slice(method.as_bytes());
    w.push(b'\n');
    let _ = fp.write_all(&w);
    0
}

/// `regio_write_register(regfile, subject, ...)`: [`regio_print_register`] into a new file; 0
/// (whatever the print returned). A file that does not open: `regio_write_register: <errno's
/// text>` (standard error), 1.
#[allow(clippy::too_many_arguments)]
pub fn regio_write_register(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    regfile: &str,
    subject: Option<&[u8]>,
    inplaneres: f32,
    betplaneres: f32,
    intensity: f32,
    r: &Matrix,
    float2int: i32,
) -> i32 {
    let mut fp = match CFile::open(dir, regfile, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let _ = writeln!(err, "regio_write_register: {}", strerror(e));
            return 1;
        }
    };
    regio_print_register(
        &mut fp,
        out,
        subject,
        inplaneres,
        betplaneres,
        intensity,
        r,
        float2int,
    );
    fp.close();
    0
}

/// `regio_read_mincxfm(xfmfile, &R, &fileinfo)`'s outcome: the status, `R` (set once allocated,
/// also when a number does not read) and the file information line (when asked for and read).
#[derive(Debug, Default)]
pub struct MincXfm {
    pub status: i32,
    pub r: Option<Matrix>,
    pub fileinfo: Option<Vec<u8>>,
}

/// `regio_read_mincxfm(xfmfile, &R, fileinfo)`: an MNI `.xfm`'s 3 x 4 transform, read as twelve
/// `%f` numbers from the start of the file's last three lines (lines counted in pieces of 999
/// bytes), into a 4 x 4 matrix of zeros with 1 at (4, 4). The first line (by `fgetl`, the
/// buffer's leading white space included) must start with `MNI Transform File`, else `ERROR:
/// <name> does not start as 'MNI Transform File'` (no newline) and 1 (FS-422: FreeSurfer leaves
/// the file open; both modes close it, no notice). The next line read by `fgetl` (the first
/// line's own text again at the end of the file) is the file information: with `want_fileinfo`
/// kept and printed as a newline, the line and two newlines; else `Not reading in xfm fileinfo`.
/// A file that does not open: `regio_read_mincxfm: <errno's text>` (standard error), `ERROR: could
/// not read <name>` (standard output), 1. A number that does not read: `regio_read_mincxfm():
/// <errno's text>` and `Error reading R[<r>][<c>] from <name>` (standard error, 0-based), 1, the
/// matrix read so far given.
pub fn regio_read_mincxfm(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    xfmfile: &str,
    want_fileinfo: bool,
) -> Result<MincXfm, Fatal> {
    let mut res = MincXfm::default();
    let mut fp = match CFile::open(dir, xfmfile, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let _ = writeln!(err, "regio_read_mincxfm: {}", strerror(e));
            let _ = writeln!(out, "ERROR: could not read {xfmfile}");
            res.status = 1;
            return Ok(res);
        }
    };
    let mut buf: Vec<u8> = Vec::new();
    fgetl_into(g, &mut fp, &mut buf, 900);
    if buf.len() < 18 || &buf[..18] != b"MNI Transform File" {
        let _ = write!(
            out,
            "ERROR: {xfmfile} does not start as 'MNI Transform File'"
        );
        res.status = 1;
        return Ok(res);
    }
    fgetl_into(g, &mut fp, &mut buf, 900);
    if want_fileinfo {
        let mut m = b"\n".to_vec();
        m.extend_from_slice(&buf);
        m.extend_from_slice(b"\n\n");
        let _ = out.write_all(&m);
        res.fileinfo = Some(buf.clone());
    } else {
        let _ = out.write_all(b"Not reading in xfm fileinfo\n");
    }
    drop(fp);
    let mut fp = match CFile::open(dir, xfmfile, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            res.status = 1;
            return Ok(res);
        }
    };
    let mut nlines: i64 = 0;
    while fgets_c(g, &mut fp, 1000).is_some() {
        nlines += 1;
    }
    fp.rewind();
    for _ in 0..nlines - 3 {
        if fgets_c(g, &mut fp, 1000).is_none() {
            let _ = err.write_all(b"regio_read_mincxfm(): could not read file\n");
        }
    }
    let Some(mut r) = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)? else {
        let _ = err.write_all(b"regio_read_mincxfm(): could not alloc R\n");
        res.status = 1;
        return Ok(res);
    };
    for row in 0..3 {
        for col in 0..4 {
            let mut errno = g.errno;
            let (v, n) = {
                let mut sc = Scanf::new(&mut fp);
                let v = sc.f(&mut errno, true);
                (v, sc.finish(&mut errno))
            };
            g.errno = errno;
            if n != 1 {
                let _ = writeln!(err, "regio_read_mincxfm(): {}", strerror(g.errno));
                let _ = writeln!(err, "Error reading R[{row}][{col}] from {xfmfile}");
                res.status = 1;
                res.r = Some(r);
                return Ok(res);
            }
            r.set_relt(row + 1, col + 1, v.unwrap_or(0.0));
        }
    }
    r.set_relt(4, 4, 1.0);
    res.r = Some(r);
    Ok(res)
}

/// `regio_write_mincxfm(xfmfile, R, fileinfo)`: `MNI Transform File`, `% <fileinfo>` when given,
/// `% This file was created by <program>`, `% <currentDateTime()>`, an empty line,
/// `Transform_Type = Linear;`, `Linear_Transform =`, then `R`'s first three rows, each number `%e`
/// and a space, the rows ended by a newline, the last by `;` and a newline; 0. A file that does not
/// open: `regio_write_mincxfm: <errno's text>` (standard error), `Could open <name> for writing`
/// (standard output, FreeSurfer's words), 1.
#[allow(clippy::too_many_arguments)]
pub fn regio_write_mincxfm(
    g: &mut Globals,
    env: &larmorx_freesurfer_core::run::Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    xfmfile: &str,
    r: &Matrix,
    fileinfo: Option<&[u8]>,
) -> i32 {
    let mut fp = match CFile::open(dir, xfmfile, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let _ = writeln!(err, "regio_write_mincxfm: {}", strerror(e));
            let _ = writeln!(out, "Could open {xfmfile} for writing");
            return 1;
        }
    };
    let mut w = b"MNI Transform File\n".to_vec();
    if let Some(info) = fileinfo {
        w.extend_from_slice(b"% ");
        w.extend_from_slice(&info[..info.iter().position(|&b| b == 0).unwrap_or(info.len())]);
        w.push(b'\n');
    }
    w.extend_from_slice(format!("% This file was created by {}\n", g.progname).as_bytes());
    w.extend_from_slice(
        format!(
            "% {}\n",
            larmorx_freesurfer_core::timer::current_date_time(env, true)
        )
        .as_bytes(),
    );
    w.extend_from_slice(b"\nTransform_Type = Linear;\nLinear_Transform =\n");
    for row in 1..=3 {
        for col in 1..=4 {
            w.extend_from_slice(fmt("%e ", f64::from(r.relt(row, col))).as_bytes());
        }
        w.extend_from_slice(if row != 3 { b"\n" } else { b";\n" });
    }
    let _ = fp.write_all(&w);
    fp.close();
    0
}

/// `regio_read_registermat(regfile)`: [`regio_read_register`]'s matrix, `None` when it fails.
pub fn regio_read_registermat(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    regfile: &str,
    lta_io: &mut dyn RegisterLta,
) -> Result<Option<Matrix>, Fatal> {
    let mut reg = Register::default();
    if regio_read_register(g, out, err, dir, regfile, &mut reg, lta_io)? != 0 {
        return Ok(None);
    }
    Ok(reg.r)
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
