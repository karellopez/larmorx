// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's float matrices in text and binary files (`fs.maths`, queue item 3.1-24),
//! translated from FreeSurfer 8.2.0's `utils/matrix.cpp`: `MatrixReadTxt` (991-1050),
//! `MatrixAsciiWriteInto` (2179-2194), `MatrixAsciiReadFrom` (2196-2232), `MatrixReadFrom`
//! (2255-2287) and `MatrixWriteTxt` (2775-2790), with `utils/fio.cpp`'s `freadInt` and
//! `freadDouble` (169-193) as `MatrixReadFrom` calls them. The C library's `fscanf` and `sscanf`
//! conversions are [`crate::cscan`]'s (glibc 2.35's behaviour, measured).
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-maths.md`):
//! - **FS-49** (an error by default): `freadInt` and `freadDouble` at the end of the file return
//!   a value they never read (the stack's contents; `freadInt` without a message), so
//!   `MatrixReadFrom` of a short file fills the type, the sizes or the elements with memory
//!   contents. By default `MatrixReadFrom` gives the error `MatrixReadFrom: the file ends before
//!   the matrix does` and `NULL`; `reproduce` reads the missing bytes as 0 (`freadDouble`'s
//!   message kept).
//! - FS-25 for `MatrixWriteTxt` (it prints a complex matrix's floats as if real).

use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};

use larmorx_core::defects::FS_49;

use crate::cscan::{Bytes, Call, Reader, Scanf, Stream};
use crate::cstd;
use crate::globals::Globals;
use crate::log::{self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NO_FILE, Fatal, NO_ERROR};
use crate::matrix::{
    MATRIX_COMPLEX, MATRIX_REAL, Matrix, matrix_alloc, read_relt, rptr, write_at, write_relt,
};
use crate::matrix_ops::real_only;
use crate::ret::Ret;
use crate::utils;

fn resolve(dir: Option<&Path>, fname: &str) -> PathBuf {
    match dir {
        Some(d) => d.join(fname),
        None => PathBuf::from(fname),
    }
}

/// `fopen`'s failure as FreeSurfer's next message sees it: `errno` set.
fn set_errno(g: &mut Globals, e: &std::io::Error) {
    g.errno = log::errno_of(e).unwrap_or(5);
}

/// `fscanf(fp, "%f", &x)`: its value, or `None` (`errno` as glibc leaves it: [`cscan::Call`]).
fn scan_value(g: &mut Globals, st: &mut dyn Stream) -> Option<f32> {
    Call::new().f32(st, &mut g.errno)
}

/// `MatrixReadTxt(fname, mat)`: a real matrix from a text file, its column count the number of
/// items on the first line (`ItemsInString` of the first 998 bytes, `fgets(line, 999)`), its row
/// count the number of such pieces of line in the file, its elements read by `fscanf("%f")` row by
/// row from the start (`mat` is not used). `Null` without a message when the first line has no
/// item; errors: `MatrixReadTxt(<fname>) - file open failed` (`ERROR_NO_FILE`), `MatrixReadTxt:
/// could not read 1st line from <fname>`, `MatrixReadTxT: could not scan value [<row>][<col>]`
/// (`ERROR_BADPARM`), each `Null`.
pub fn matrix_read_txt(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Ret<Matrix>, Fatal> {
    let bytes = match std::fs::read(resolve(dir, fname)) {
        Ok(b) => b,
        Err(e) => {
            set_errno(g, &e);
            log::error_printf(
                g,
                err,
                ERROR_NO_FILE,
                &format!("MatrixReadTxt({fname}) - file open failed\n"),
            );
            return Ok(Ret::Null);
        }
    };
    let mut fp: &[u8] = &bytes;
    let Some(line) = utils::fgets(&mut fp, 999) else {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MatrixReadTxt: could not read 1st line from {fname}\n"),
        );
        return Ok(Ret::Null);
    };
    let cols = utils::items_in_string(c_str(&line));
    if cols < 1 {
        return Ok(Ret::Null);
    }
    let mut rows = 1;
    while utils::fgets(&mut fp, 999).is_some() {
        rows += 1;
    }
    let mut mat = matrix_alloc(g, out, err, rows, cols, MATRIX_REAL)?.expect("a real matrix");
    let mut rd: &[u8] = &bytes;
    let mut st = Reader(&mut rd);
    for row in 1..=rows {
        for col in 1..=cols {
            let Some(v) = scan_value(g, &mut st) else {
                log::error_printf(
                    g,
                    err,
                    ERROR_BADPARM,
                    &format!("MatrixReadTxT: could not scan value [{row}][{col}]\n"),
                );
                return Ok(Ret::Null);
            };
            write_relt(g, &mut mat, row, col, v);
        }
    }
    Ok(Ret::New(mat))
}

/// The C string of a buffer `fgets` filled (up to its first NUL byte).
fn c_str(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// `MatrixAsciiWriteInto(fp, m)`: `<type> <rows> <cols>`, then each row on a line: a complex
/// element as `%+f %+f` and three spaces, any other as `%+f` and two spaces (the floats as
/// `double`s). Returns `NO_ERROR`.
pub fn matrix_ascii_write_into(g: &mut Globals, fp: &mut dyn Write, m: &Matrix) -> i32 {
    let mut s = format!("{} {} {}\n", m.mtype, m.rows, m.cols);
    let f = |x: f32| cstd::format_double("%+f", f64::from(x)).expect("%+f");
    for row in 1..=m.rows {
        for col in 1..=m.cols {
            if m.mtype == MATRIX_COMPLEX {
                let i = rptr(m, row) + 2 * col as isize;
                let re = crate::matrix::read_at(g, m, i);
                let im = crate::matrix::read_at(g, m, i + 1);
                s.push_str(&format!("{} {}   ", f(re), f(im)));
            } else {
                s.push_str(&format!("{}  ", f(read_relt(g, m, row, col))));
            }
        }
        s.push('\n');
    }
    let _ = fp.write_all(s.as_bytes());
    NO_ERROR
}

/// `MatrixAsciiReadFrom(fp, m)`: a matrix in [`matrix_ascii_write_into`]'s form. The first line
/// that is not empty or a comment (`fgetl(line, 199)`) holds `<type> <rows> <cols>`
/// (`sscanf("%d %d %d\n")`); the elements follow, read by `fscanf` (`%f  `, or `%f %f   ` for a
/// complex matrix), each row's end skipped. Into `m` when given (it must have the file's type and
/// sizes: otherwise the error `MatrixAsciiReadFrom: specified matrix does not match file` and `m`
/// is returned unchanged); else a new matrix. Errors (`ERROR_BADFILE`, `Null`): `MatrixAsciiReadFrom:
/// could not scanf parms`, `... could not allocate matrix` (an unknown type, after `MatrixAlloc`'s
/// own error), `... could not scan element (<row>, <col>)` (a given `m` keeps what was read).
pub fn matrix_ascii_read_from(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fp: &mut dyn BufRead,
    m: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    // `sscanf(cp, "%d %d %d\n", &type, &rows, &cols) != 3`
    let parms = utils::fgetl(fp, 199).and_then(|line| {
        let mut b = Bytes::new(&line);
        let mut sc = Scanf::new(&mut b);
        let mut v = [0i32; 3];
        for (k, x) in v.iter_mut().enumerate() {
            if k > 0 {
                sc.text(&mut g.errno, b" ");
            }
            if let Some(d) = sc.d(&mut g.errno, true) {
                *x = d;
            }
        }
        sc.text(&mut g.errno, b"\n");
        (sc.finish(&mut g.errno) == 3).then_some(v)
    });
    let Some([mtype, rows, cols]) = parms else {
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            "MatrixAsciiReadFrom: could not scanf parms",
        );
        return Ok(Ret::Null);
    };
    let mut fresh = None;
    let m: &mut Matrix = match m {
        None => match matrix_alloc(g, out, err, rows, cols, mtype as i16)? {
            Some(x) => fresh.insert(x),
            None => {
                log::error_printf(
                    g,
                    err,
                    ERROR_BADFILE,
                    "MatrixAsciiReadFrom: could not allocate matrix",
                );
                return Ok(Ret::Null);
            }
        },
        Some(x) => {
            if x.rows != rows || x.cols != cols || i32::from(x.mtype) != mtype {
                log::error_printf(
                    g,
                    err,
                    ERROR_BADFILE,
                    "MatrixAsciiReadFrom: specified matrix does not match file",
                );
                return Ok(Ret::Given);
            }
            x
        }
    };
    let mut st = Reader(fp);
    for row in 1..=rows {
        for col in 1..=cols {
            let fail = |g: &mut Globals, err: &mut dyn Write| {
                log::error_printf(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("MatrixAsciiReadFrom: could not scan element ({row}, {col})"),
                );
            };
            if m.mtype == MATRIX_COMPLEX {
                // `fscanf(fp, "%f %f   ", &re, &im) != 2`: each value assigned as it is read
                let i = rptr(m, row) + 2 * col as isize;
                let mut sc = Scanf::new(&mut st);
                let re = sc.f(&mut g.errno, true);
                sc.text(&mut g.errno, b" ");
                let im = sc.f(&mut g.errno, true);
                sc.text(&mut g.errno, b"   ");
                let n = sc.finish(&mut g.errno);
                if let Some(re) = re {
                    write_at(g, m, i, re);
                }
                if let Some(im) = im {
                    write_at(g, m, i + 1, im);
                }
                if n != 2 {
                    fail(g, err);
                    return Ok(Ret::Null);
                }
            } else {
                // `fscanf(fp, "%f  ", &x) != 1`
                let mut sc = Scanf::new(&mut st);
                let v = sc.f(&mut g.errno, true);
                sc.text(&mut g.errno, b"  ");
                let n = sc.finish(&mut g.errno);
                let Some(v) = v.filter(|_| n == 1) else {
                    fail(g, err);
                    return Ok(Ret::Null);
                };
                write_relt(g, m, row, col, v);
            }
        }
        // `fscanf(fp, "\n")`: 0 (it assigns nothing; a read error is not possible on this path)
        let mut sc = Scanf::new(&mut st);
        sc.text(&mut g.errno, b"\n");
        sc.finish(&mut g.errno);
    }
    Ok(match fresh {
        Some(x) => Ret::New(x),
        None => Ret::Given,
    })
}

/// `freadInt(fp)`: a big-endian `int`, and whether the file held its 4 bytes (FS-49: the missing
/// bytes are 0 here; FreeSurfer's are the stack's, and it prints nothing).
pub fn fread_int(fp: &mut dyn Read) -> (i32, bool) {
    let mut b = [0u8; 4];
    let n = read_full(fp, &mut b);
    (i32::from_be_bytes(b), n == 4)
}

/// `freadDouble(fp)`: a big-endian `double`, and whether the file held its 8 bytes (when not:
/// `freadDouble: fread failed` as an error, FS-49's missing bytes 0 here).
pub fn fread_double(g: &mut Globals, err: &mut dyn Write, fp: &mut dyn Read) -> (f64, bool) {
    let mut b = [0u8; 8];
    let n = read_full(fp, &mut b);
    if n != 8 {
        log::error_printf(g, err, ERROR_BADFILE, "freadDouble: fread failed");
    }
    (f64::from_be_bytes(b), n == 8)
}

fn read_full(fp: &mut dyn Read, b: &mut [u8]) -> usize {
    let mut n = 0;
    while n < b.len() {
        match fp.read(&mut b[n..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => n += k,
        }
    }
    n
}

/// FS-49 for a read past the file's end: `true` (refused, after the error) by default.
fn short_file(g: &mut Globals, err: &mut dyn Write) -> bool {
    if g.notices.reproduce() {
        return false;
    }
    log::error_printf(
        g,
        err,
        ERROR_BADFILE,
        "MatrixReadFrom: the file ends before the matrix does",
    );
    g.notices.add(
        &FS_49,
        "MatrixReadFrom: the file ends before the matrix: refused",
    );
    true
}

/// The stack slot `freadDouble` and `freadInt` read into, as the official build has it when
/// `MatrixReadFrom` calls them (track E's amended spec §18.2): `freadDouble`'s `double` is bytes
/// 0 to 7 (it stores the swapped value back, so the slot holds the value least significant byte
/// first), `freadInt`'s `int` bytes 4 to 7 (the file's bytes, in file order). A short read leaves
/// the rest of the slot as the call before left it; `known[i]` is false for a byte that is memory
/// FreeSurfer never wrote (0 here).
struct Slot {
    b: [u8; 8],
    known: [bool; 8],
    /// A number took a byte from memory.
    memory: bool,
    /// The file ended before the matrix.
    short: bool,
}

impl Slot {
    fn new() -> Slot {
        Slot {
            b: [0; 8],
            known: [false; 8],
            memory: false,
            short: false,
        }
    }

    /// `freadInt`: the file's bytes over bytes 4 to 7, read big-endian.
    fn int(&mut self, fp: &mut dyn Read) -> i32 {
        let mut t = [0u8; 4];
        let k = read_full(fp, &mut t);
        self.b[4..4 + k].copy_from_slice(&t[..k]);
        self.known[4..4 + k].fill(true);
        if k < 4 {
            self.short = true;
            self.memory |= self.known[4 + k..].iter().any(|&x| !x);
        }
        i32::from_be_bytes(self.b[4..8].try_into().unwrap())
    }

    /// `freadDouble`: the file's bytes over bytes 0 to 7 read big-endian, then the slot reversed
    /// (the value stored back).
    fn double(&mut self, g: &mut Globals, err: &mut dyn Write, fp: &mut dyn Read) -> f64 {
        let mut t = [0u8; 8];
        let k = read_full(fp, &mut t);
        if k != 8 {
            log::error_printf(g, err, ERROR_BADFILE, "freadDouble: fread failed");
            self.short = true;
            self.memory |= self.known[k..].iter().any(|&x| !x);
        }
        self.b[..k].copy_from_slice(&t[..k]);
        self.known[..k].fill(true);
        let v = f64::from_be_bytes(self.b);
        self.b.reverse();
        self.known.reverse();
        v
    }

    /// `MatrixAlloc` between the header and the elements: the slot becomes memory.
    fn clobbered(&mut self) {
        self.b = [0; 8];
        self.known = [false; 8];
    }
}

/// `MatrixReadFrom(fp, m)`: a matrix in `MatrixWriteInto`'s binary form: its type, rows and
/// columns as big-endian `int`s, then each element as a big-endian `double` (a complex one as two),
/// each rounded to `float`. Into `m` when given (it must have the file's type and sizes: otherwise
/// the error `MatrixReadFrom: specified matrix does not match file` and `m` returned unchanged);
/// else a new matrix (`MatrixReadFrom: could not allocate matrix` and `Null` for an unknown type).
///
/// **FS-49:** a file that ends early. By default the error `MatrixReadFrom: the file ends before
/// the matrix does` and `Null`. With `reproduce` FreeSurfer's numbers: each number cut short keeps
/// the bytes the number before it left in the same place ([`Slot`]: an integer the previous
/// integer's file bytes, a double the previous double's value least significant byte first; the
/// first element after a matrix `MatrixReadFrom` made, and the first integer, memory: 0 here,
/// noted as memory).
pub fn matrix_read_from(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fp: &mut dyn Read,
    m: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let mut slot = Slot::new();
    let mtype = slot.int(fp);
    let rows = slot.int(fp);
    let cols = slot.int(fp);
    if slot.short && short_file(g, err) {
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    let m: &mut Matrix = match m {
        None => {
            slot.clobbered();
            match matrix_alloc(g, out, err, rows, cols, mtype as i16)? {
                Some(x) => fresh.insert(x),
                None => {
                    log::error_printf(
                        g,
                        err,
                        ERROR_BADFILE,
                        "MatrixReadFrom: could not allocate matrix",
                    );
                    fs49_reproduced(g, &slot);
                    return Ok(Ret::Null);
                }
            }
        }
        Some(x) => {
            if x.rows != rows || x.cols != cols || i32::from(x.mtype) != mtype {
                log::error_printf(
                    g,
                    err,
                    ERROR_BADFILE,
                    "MatrixReadFrom: specified matrix does not match file",
                );
                fs49_reproduced(g, &slot);
                return Ok(Ret::Given);
            }
            x
        }
    };
    let mut refused = false;
    'rows: for row in 1..=m.rows {
        for col in 1..=m.cols {
            let n = if m.mtype == MATRIX_COMPLEX { 2 } else { 1 };
            for k in 0..n {
                let was_short = slot.short;
                let d = slot.double(g, err, fp);
                if slot.short && !was_short && short_file(g, err) {
                    refused = true;
                    break 'rows;
                }
                if m.mtype == MATRIX_COMPLEX {
                    let i = rptr(m, row) + 2 * col as isize + k;
                    write_at(g, m, i, d as f32);
                } else {
                    write_relt(g, m, row, col, d as f32);
                }
            }
        }
    }
    if refused {
        return Ok(Ret::Null);
    }
    fs49_reproduced(g, &slot);
    Ok(match fresh {
        Some(x) => Ret::New(x),
        None => Ret::Given,
    })
}

/// FS-49's notice under `reproduce` for a file that ended early (`memory` when a number took a
/// byte FreeSurfer never wrote).
fn fs49_reproduced(g: &mut Globals, slot: &Slot) {
    if slot.short && g.notices.reproduce() {
        g.notices.add(
            &FS_49,
            if slot.memory {
                "MatrixReadFrom: the file ends before the matrix: FreeSurfer's numbers, some bytes memory contents (0 here)"
            } else {
                "MatrixReadFrom: the file ends before the matrix: FreeSurfer's numbers from the earlier numbers' bytes"
            },
        );
    }
}

/// `MatrixWriteTxt(fname, mat)`: each row on a line, each element as `%+4.5f` and a space.
/// `ERROR_NO_FILE` after `MatrixWriteTxt(<fname>) - file open failed`; else `NO_ERROR`. FS-25.
pub fn matrix_write_txt(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    mat: &Matrix,
) -> i32 {
    let mut fp = match std::fs::File::create(resolve(dir, fname)) {
        Ok(f) => f,
        Err(e) => {
            set_errno(g, &e);
            log::error_printf(
                g,
                err,
                ERROR_NO_FILE,
                &format!("MatrixWriteTxt({fname}) - file open failed\n"),
            );
            return ERROR_NO_FILE;
        }
    };
    if real_only(g, err, "MatrixWriteTxt", &[mat]) {
        return ERROR_BADPARM;
    }
    let mut s = String::new();
    for row in 1..=mat.rows {
        for col in 1..=mat.cols {
            let v = f64::from(read_relt(g, mat, row, col));
            s.push_str(&cstd::format_double("%+4.5f ", v).expect("%+4.5f"));
        }
        s.push('\n');
    }
    let _ = fp.write_all(s.as_bytes());
    NO_ERROR
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn ascii_round_trip_and_binary() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let m = Matrix::from_rows(&[vec![1.5, -2.0], vec![0.25, 3.0]]);
        let mut text = Vec::new();
        matrix_ascii_write_into(&mut g, &mut text, &m);
        assert_eq!(
            String::from_utf8(text.clone()).unwrap(),
            "1 2 2\n+1.500000  -2.000000  \n+0.250000  +3.000000  \n"
        );
        let mut rd: &[u8] = &text;
        let back = matrix_ascii_read_from(&mut g, &mut o, &mut e, &mut rd, None)
            .unwrap()
            .unwrap_new();
        assert_eq!(back.data(), m.data());
        let mut bin = Vec::new();
        for v in [1i32, 1, 2] {
            bin.extend_from_slice(&v.to_be_bytes());
        }
        bin.extend_from_slice(&0.5f64.to_be_bytes());
        let mut rd: &[u8] = &bin;
        let r = matrix_read_from(&mut g, &mut o, &mut e, &mut rd, None).unwrap();
        assert!(r.is_null());
        assert!(g.notices.contains(&FS_49));
    }
}
