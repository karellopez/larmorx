// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! MATLAB `.mat` files as FreeSurfer 8.2.0 reads and writes them (`utils/matfile.cpp`, queue item
//! 3.1-27): `MatlabWrite`/`MatFileWrite` (a version 4 file of one real matrix in doubles),
//! `MatlabRead`/`MatlabRead2` (the first variable of a version 4 or 5 file, compressed or not),
//! `MatFileRead` (version 4, as bytes or floats), `ReadMatlabFileContents` (every variable of a
//! version 4 file), `ReadMatlabFileVariable`, `MLFCprint`, `MLFCfree`, the header readers.
//!
//! `reproduce` gives FreeSurfer's results, defects included; where FreeSurfer's result depends on
//! memory never written or past what it allocated, larmorx uses 0 and notes it ("memory never
//! written"/"memory contents"). The default corrects the readers (FS-230 to FS-251, rules A to D,
//! `docs/findings/freesurfer-io.md` Part 5): version 4 files of every type and both byte orders,
//! 16-bit, 32-bit and unsigned numbers, complex matrices, MATLAB's small elements and names, and
//! compressed (MATLAB 7) variables decoded whole.

use std::io::Write;
use std::path::Path;

use flate2::{Decompress, FlushDecompress, Status};
use larmorx_core::defects::{
    FS_230, FS_231, FS_232, FS_233, FS_234, FS_235, FS_236, FS_237, FS_238, FS_239, FS_240, FS_241,
    FS_243, FS_244, FS_245, FS_246, FS_247, FS_248,
};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE, diag_printf};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_NO_MEMORY, Fatal};
use larmorx_freesurfer_core::matrix::{
    MATRIX_COMPLEX, MATRIX_REAL, Matrix, matrix_alloc, matrix_copy,
};
use larmorx_freesurfer_core::matrix_ops::matrix_print;
use larmorx_freesurfer_core::ret::Ret;

use crate::cfile::CFile;

pub const MAT_BYTE: i32 = 0;
pub const MAT_DOUBLE: i32 = 1;
pub const MAT_INT: i32 = 2;
pub const MAT_SHORT: i32 = 3;
pub const MAT_FLOAT: i32 = 4;
pub const MATFILE_PC: i32 = 0;
pub const MATFILE_SPARC: i32 = 1000;
pub const MATFILE_PC5: u8 = b'I';
pub const MATFILE_SPARC5: u8 = b'M';
pub const MATFILE_DOUBLE: i32 = 0;
pub const MATFILE_FULL: i32 = 0;

/// `MatProgname`.
const PROG: &str = "matfile";

/// The bytes FreeSurfer allocates for what a file states above which larmorx notes that its outcome
/// depends on the machine's memory (FS-243), and the bytes of numbers above which larmorx refuses
/// (as an allocation that failed) so that it never holds more than 256 MiB.
const NOTE_BYTES: i64 = 1 << 26;
const MAX_BYTES: i64 = 1 << 28;

/// The fields of [`MatFile`] FreeSurfer may hold in memory never written.
const U_TYPE: u32 = 1;
const U_MROWS: u32 = 2;
const U_NCOLS: u32 = 4;
const U_IMAGF: u32 = 8;
const U_NAMLEN: u32 = 16;
const U_VERSION: u32 = 32;

/// `MATFILE`: what a header says of a variable (`MatFileRead` adds its numbers in `data`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MatFile {
    pub mtype: i32,
    pub mrows: i32,
    pub ncols: i32,
    pub imagf: i32,
    pub namlen: i32,
    pub version: i32,
    /// `data` (`MatFileRead`: the numbers as bytes or floats); `None` for `NULL`.
    pub data: Option<Vec<u8>>,
    pub endian: u8,
    /// The fields FreeSurfer holds in memory never written (`U_*` bits; larmorx's value 0).
    unknown: u32,
    /// The corrected version 5 reader's real part: where its numbers start, their bytes and
    /// whether MATLAB stored them in a small element.
    real: Option<(i64, i64, bool)>,
}

impl MatFile {
    /// A `MATFILE` on the stack, never written (`MatlabRead2`'s).
    fn unwritten() -> MatFile {
        MatFile {
            unknown: U_TYPE | U_MROWS | U_NCOLS | U_IMAGF | U_NAMLEN | U_VERSION,
            ..MatFile::default()
        }
    }
}

/// `MLFC`: every variable of a version 4 file.
#[derive(Clone, Debug, PartialEq)]
pub struct Mlfc {
    /// `mfile`: the file's name.
    pub mfile: Vec<u8>,
    /// `varname[n]`, `varmtx[n]`.
    pub vars: Vec<(Vec<u8>, Matrix)>,
}

/// The bytes of a C string up to its first zero.
fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// A message of text and a C string's bytes: `head`, the string, `tail`.
fn with_name(head: &str, name: &[u8], tail: &str) -> Vec<u8> {
    let mut m = head.as_bytes().to_vec();
    m.extend_from_slice(cstr(name));
    m.extend_from_slice(tail.as_bytes());
    m
}

/// `DiagPrintf(bits, ...)` of a message of bytes.
fn diag_bytes(g: &Globals, out: &mut dyn Write, bits: u64, msg: &[u8]) {
    if bits == 0 || bits & g.gdiag != 0 {
        let _ = out.write_all(msg);
    }
}

/// A `long32` printed with `%ld`: the official build passes it zero-extended.
fn ld(v: i32) -> u32 {
    v as u32
}

/// A file read with C's `ungetc`: the bytes given back are read first, and a relative seek
/// counts them as unread.
struct Pb {
    fp: CFile,
    back: Vec<u8>,
}

impl Pb {
    fn getc(&mut self, errno: &mut i32) -> i32 {
        if let Some(b) = self.back.pop() {
            return i32::from(b);
        }
        let c = self.fp.getc();
        if c < 0
            && let Some(e) = self.fp.read_errno()
        {
            *errno = e;
        }
        c
    }

    /// `ungetc(c, fp)` of a `char` value: `EOF` (a byte 0xff read as a `char`) is not given back.
    fn ungetc(&mut self, c: i8) {
        if c != -1 {
            self.back.push(c as u8);
        }
    }

    /// `fread` of `buf.len()` bytes into `buf` (the bytes after the file's end keep their
    /// values): the bytes read.
    fn read(&mut self, errno: &mut i32, buf: &mut [u8]) -> usize {
        let mut got = 0;
        while got < buf.len() {
            let Some(b) = self.back.pop() else { break };
            buf[got] = b;
            got += 1;
        }
        if got < buf.len() {
            got += self.fp.read_bytes(&mut buf[got..]);
            if got < buf.len()
                && let Some(e) = self.fp.read_errno()
            {
                *errno = e;
            }
        }
        got
    }

    fn fseek(&mut self, errno: &mut i32, off: i64, whence: i32) {
        let off = if whence == 1 {
            off - self.back.len() as i64
        } else {
            off
        };
        self.back.clear();
        let _ = self.fp.fseek(errno, off, whence);
    }

    fn ferror(&self) -> bool {
        self.fp.ferror()
    }
}

/// `swapLong32`.
fn swap32(v: i32) -> i32 {
    v.swap_bytes()
}

/// `DIFFERENT_ENDIAN(mf)` on a little-endian machine. Version 5: the file's byte-order letter
/// is not `I`. Version 4: FreeSurfer takes every type but 0 (little-endian doubles) for the other
/// byte order (FS-230); the default takes a type outside 0 to 999 (MATLAB's thousands digit
/// not 0) for it.
fn different_endian(g: &mut Globals, mf: &MatFile) -> bool {
    let version = known(g, mf, U_VERSION, mf.version);
    if version == 5 {
        return mf.endian != MATFILE_PC5;
    }
    let t = known(g, mf, U_TYPE, mf.mtype);
    let official = t != MATFILE_PC;
    let right = !(0..1000).contains(&t);
    if official != right {
        g.notices.add(
            &FS_230,
            format!(
                "a version 4 type {t} taken for the {} byte order",
                if official { "other" } else { "machine's" }
            ),
        );
        if g.notices.reproduce() {
            return official;
        }
    }
    right
}

/// FS-245: this result of FreeSurfer's depends on memory contents (`what`); the cause is noted
/// with its own defect as well.
fn memory(g: &mut Globals, what: &str) {
    g.notices.add(&FS_245, what.to_owned());
}

/// A field's value; one FreeSurfer holds in memory never written is noted and read as 0.
fn known(g: &mut Globals, mf: &MatFile, bit: u32, v: i32) -> i32 {
    if mf.unknown & bit != 0 {
        g.notices.add(
            &FS_241,
            "a .mat header field read from memory never written (FreeSurfer's value is memory contents; larmorx's 0)",
        );
        memory(
            g,
            "a .mat header field read from memory never written (FreeSurfer's value is memory contents; larmorx's 0)",
        );
        return 0;
    }
    v
}

/// `fread(&field, 4, 1, fp)` into a `long32` field: the bytes the file gave replace the field's
/// first bytes; a field never written stays so unless all four came.
fn read_field(g: &mut Globals, pb: &mut Pb, mf: &mut MatFile, bit: u32) -> usize {
    let mut b = field(mf, bit).to_le_bytes();
    let got = pb.read(&mut g.errno, &mut b);
    *field_mut(mf, bit) = i32::from_le_bytes(b);
    if got == 4 {
        mf.unknown &= !bit;
    }
    got
}

fn field(mf: &MatFile, bit: u32) -> i32 {
    match bit {
        U_TYPE => mf.mtype,
        U_MROWS => mf.mrows,
        U_NCOLS => mf.ncols,
        U_IMAGF => mf.imagf,
        U_NAMLEN => mf.namlen,
        _ => mf.version,
    }
}

fn field_mut(mf: &mut MatFile, bit: u32) -> &mut i32 {
    match bit {
        U_TYPE => &mut mf.mtype,
        U_MROWS => &mut mf.mrows,
        U_NCOLS => &mut mf.ncols,
        U_IMAGF => &mut mf.imagf,
        U_NAMLEN => &mut mf.namlen,
        _ => &mut mf.version,
    }
}

fn badfile(g: &mut Globals, err: &mut dyn Write, msg: &str) {
    log::error_printf(g, err, ERROR_BADFILE, msg);
}

/// An allocation of `bytes` the file asked for: larmorx notes it above 256 MiB (FreeSurfer's
/// outcome depends on the machine's memory) and refuses above 1 GiB (FS-243).
fn big(g: &mut Globals, what: &str, bytes: i64) -> bool {
    if bytes > NOTE_BYTES {
        g.notices.add(
            &FS_243,
            format!("{what}: {bytes} bytes asked for by the file (FreeSurfer's outcome depends on the machine's memory)"),
        );
    }
    bytes > MAX_BYTES
}

// ------------------------------------------------------------------ writing

/// `MatlabWrite(mat, fname, name)`: [`mat_file_write`] of the matrix's numbers, named `matrix`
/// when `name` is `None`. A complex matrix: FreeSurfer writes its real and imaginary parts
/// interleaved as the first half of a real matrix's numbers (FS-247); the default writes a
/// complex matrix.
pub fn matlab_write(
    g: &mut Globals,
    dir: Option<&Path>,
    mat: &Matrix,
    fname: &str,
    name: Option<&[u8]>,
) -> i32 {
    let name = name.unwrap_or(b"matrix");
    let n = (mat.rows.max(0) as usize) * (mat.cols.max(0) as usize);
    if mat.mtype == MATRIX_COMPLEX {
        g.notices.add(
            &FS_247,
            "MatlabWrite of a complex matrix (FreeSurfer writes its parts interleaved as real numbers)",
        );
        if !g.notices.reproduce() {
            let (mut re, mut im) = (Vec::with_capacity(n), Vec::with_capacity(n));
            for r in 1..=mat.rows {
                for c in 1..=mat.cols {
                    let (a, b) = mat.celt(r, c);
                    re.push(a);
                    im.push(b);
                }
            }
            return write_v4(dir, fname, &re, Some(&im), mat.rows, mat.cols, name);
        }
    }
    let data = mat.data();
    mat_file_write(
        dir,
        fname,
        &data[..n.min(data.len())],
        mat.rows,
        mat.cols,
        name,
    )
}

/// `MatFileWrite(fname, data, rows, cols, name)`: a version 4 file (this machine's byte order) of
/// one real matrix of doubles, `data` row by row (`data[row * cols + col]`) written column by
/// column, named `name`. 0; -1 when the file cannot be created; -2 or -3 when a write fails.
pub fn mat_file_write(
    dir: Option<&Path>,
    fname: &str,
    data: &[f32],
    rows: i32,
    cols: i32,
    name: &[u8],
) -> i32 {
    write_v4(dir, fname, data, None, rows, cols, name)
}

fn write_v4(
    dir: Option<&Path>,
    fname: &str,
    re: &[f32],
    im: Option<&[f32]>,
    rows: i32,
    cols: i32,
    name: &[u8],
) -> i32 {
    let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())];
    let namlen = name.len() as i32 + 1;
    let Ok(mut fp) = CFile::open(dir, fname, "wb") else {
        return -1;
    };
    let mut hd = Vec::with_capacity(20);
    for v in [
        MATFILE_PC + MATFILE_DOUBLE + MATFILE_FULL,
        rows,
        cols,
        i32::from(im.is_some()),
        namlen,
    ] {
        hd.extend_from_slice(&v.to_le_bytes());
    }
    if fp.fwrite(&hd, 1, 20) != 20 {
        fp.close();
        return -2;
    }
    let mut nm = name.to_vec();
    nm.push(0);
    if fp.fwrite(&nm, 1, nm.len()) != nm.len() {
        fp.close();
        return -3;
    }
    for part in std::iter::once(re).chain(im) {
        for col in 0..cols.max(0) {
            for row in 0..rows.max(0) {
                let i = (row as i64 * cols as i64 + col as i64) as usize;
                let d = f64::from(part.get(i).copied().unwrap_or(0.0));
                if fp.fwrite(&d.to_le_bytes(), 1, 8) != 8 {
                    fp.close();
                    return -2;
                }
            }
        }
    }
    fp.close();
    0
}

// ------------------------------------------------------------------ numbers

/// The numbers of a matrix as `matAlloc` makes them (`rows` rows of `cols` zeros), stored flat.
#[derive(Clone, Debug)]
struct Nums {
    cols: usize,
    v: Vec<f64>,
}

impl std::ops::Index<usize> for Nums {
    type Output = [f64];
    fn index(&self, row: usize) -> &[f64] {
        &self.v[row * self.cols..(row + 1) * self.cols]
    }
}

impl std::ops::IndexMut<usize> for Nums {
    fn index_mut(&mut self, row: usize) -> &mut [f64] {
        &mut self.v[row * self.cols..(row + 1) * self.cols]
    }
}

/// `matAlloc(rows, cols)`: `None` after `could not allocate <rows> x <cols> matrix` (an error)
/// for a negative size.
fn mat_alloc(g: &mut Globals, err: &mut dyn Write, rows: i32, cols: i32) -> Option<Nums> {
    let msg = format!("could not allocate {rows} x {cols} matrix\n");
    if rows < 0 {
        g.errno = 12;
        badfile(g, err, &msg);
        return None;
    }
    if rows > 0 && cols < 0 {
        g.errno = 12;
        badfile(g, err, &msg);
        return None;
    }
    let cols = cols.max(0) as usize;
    Some(Nums {
        cols,
        v: vec![0.0; rows as usize * cols],
    })
}

/// The bytes `matAlloc` takes for `rows` x `cols` in glibc (a pointer and a row's block each).
fn nums_bytes(rows: i32, cols: i32) -> i64 {
    // saturating: a header's sizes up to 2^31 each overflow 64 bits (a refusal either way)
    i64::from(rows.max(0)).saturating_mul(8 + usable(8 * i64::from(cols.max(0))) + 8)
}

/// The element of a version 4 or 5 type: its `MAT_*` code (FreeSurfer's switch value: an
/// unsupported type keeps its own value), its bytes, signed or not, and supported.
fn element(version: i32, t: i32) -> (i32, usize, bool, bool) {
    if version == 5 {
        match t {
            9 => (MAT_DOUBLE, 8, true, true),
            7 => (MAT_FLOAT, 4, true, true),
            5 => (MAT_INT, 4, true, true),
            6 => (MAT_INT, 4, false, true),
            3 => (MAT_SHORT, 2, true, true),
            4 => (MAT_SHORT, 2, false, true),
            1 => (MAT_BYTE, 1, true, true),
            2 => (MAT_BYTE, 1, false, true),
            _ => (t, 0, true, false),
        }
    } else {
        let mut d = t.wrapping_sub(t / 1000 * 1000);
        d -= d / 100 * 100;
        d = d / 10 * 10;
        match d {
            0 => (MAT_DOUBLE, 8, true, true),
            10 => (MAT_FLOAT, 4, true, true),
            20 => (MAT_INT, 4, true, true),
            30 => (MAT_SHORT, 2, true, true),
            40 => (MAT_SHORT, 2, false, true),
            50 => (MAT_BYTE, 1, false, true),
            _ => (d, 0, true, false),
        }
    }
}

/// The defects of reading `n` numbers of this element (both modes note them).
fn note_element(g: &mut Globals, ty: i32, size: usize, other_order: bool, imag: bool, n: i64) {
    if n <= 0 {
        return;
    }
    if ty == MAT_SHORT {
        g.notices.add(
            &FS_231,
            "16-bit numbers (FreeSurfer fails on each, or keeps a byte of the last)",
        );
    }
    if ty == MAT_INT {
        g.notices.add(
            &FS_232,
            "32-bit integers (FreeSurfer reads 8 bytes for each)",
        );
    }
    if ty == MAT_FLOAT && other_order {
        g.notices.add(
            &FS_234,
            "single-precision numbers of the other byte order (FreeSurfer swaps them as integers)",
        );
    }
    if imag && ty != MAT_DOUBLE && size > 0 {
        g.notices.add(&FS_235, "the imaginary part of a matrix that is not of doubles (FreeSurfer's: memory contents or ones)");
        memory(
            g,
            "the imaginary part of a matrix that is not of doubles (FreeSurfer's: memory contents or ones)",
        );
    }
}

/// `(long32)f` as the official build converts it (`cvttss2si`): out of range or NaN gives
/// -2147483648.
fn cvtt_f32(f: f32) -> i32 {
    if f.is_nan() || f >= 2_147_483_648.0 || f < -2_147_483_648.0 {
        i32::MIN
    } else {
        f as i32
    }
}

fn read_fail(g: &mut Globals, err: &mut dyn Write, row: i64, col: i64, imag: bool) -> i32 {
    let tail = if imag { " - imag" } else { "" };
    badfile(
        g,
        err,
        &format!("{PROG}: could not read val[{row}, {col}] from .mat file{tail}\n"),
    );
    -1
}

/// `readMatFile(fp, mf, real, imag)`: the numbers, column by column, real part first; the
/// element code, or -1 after a message when the file ends.
fn read_mat_file(
    g: &mut Globals,
    err: &mut dyn Write,
    pb: &mut Pb,
    mf: &MatFile,
    real: &mut Option<Nums>,
    imag: &mut Option<Nums>,
) -> i32 {
    let version = known(g, mf, U_VERSION, mf.version);
    let t = known(g, mf, U_TYPE, mf.mtype);
    let (ty, size, signed, ok) = element(version, t);
    if !ok {
        badfile(
            g,
            err,
            &format!("unsupported matlab format {ty} (unknown)\n"),
        );
    }
    let rows = known(g, mf, U_MROWS, mf.mrows);
    let cols = known(g, mf, U_NCOLS, mf.ncols);
    let n = i64::from(rows.max(0)) * i64::from(cols.max(0));
    let other = different_endian(g, mf);
    note_element(g, ty, size, other, imag.is_some(), n);
    if g.notices.reproduce() {
        official_numbers(g, err, pb, mf, ty, (rows, cols), real, imag)
    } else {
        fixed_numbers(
            g,
            err,
            pb,
            mf,
            (ty, size, signed, ok),
            (rows, cols),
            real,
            imag,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn official_numbers(
    g: &mut Globals,
    err: &mut dyn Write,
    pb: &mut Pb,
    mf: &MatFile,
    ty: i32,
    (rows, cols): (i32, i32),
    real: &mut Option<Nums>,
    imag: &mut Option<Nums>,
) -> i32 {
    // the reader's locals: never written before their first read (memory contents)
    let mut sval: i16 = 0;
    let mut sval_known = false;
    let mut dval: f64 = 0.0;
    let mut dval_known = false;
    // an unsupported type's loop does nothing
    let real_cols = if (0..=4).contains(&ty) { cols } else { 0 };
    for col in 0..real_cols {
        for row in 0..rows {
            let v = match ty {
                MAT_BYTE => {
                    let mut b = [0u8; 1];
                    if pb.read(&mut g.errno, &mut b) != 1 {
                        return read_fail(g, err, row.into(), col.into(), false);
                    }
                    f64::from(b[0] as i8)
                }
                MAT_SHORT => {
                    let mut b = sval.to_le_bytes();
                    let got = pb.read(&mut g.errno, &mut b);
                    if got != 1 {
                        return read_fail(g, err, row.into(), col.into(), false);
                    }
                    if !sval_known {
                        g.notices.add(
                            &FS_231,
                            "a 16-bit number's second byte from memory never written",
                        );
                        memory(g, "a 16-bit number's second byte from memory never written");
                    }
                    sval = i16::from_le_bytes(b);
                    sval_known = true;
                    if different_endian(g, mf) {
                        sval = sval.swap_bytes();
                    }
                    f64::from(sval)
                }
                MAT_INT => {
                    let mut b = [0u8; 8];
                    if pb.read(&mut g.errno, &mut b) != 8 {
                        return read_fail(g, err, row.into(), col.into(), false);
                    }
                    let mut l = i64::from_le_bytes(b);
                    if different_endian(g, mf) {
                        l = i64::from(swap32(l as i32));
                    }
                    l as f64
                }
                MAT_FLOAT => {
                    let mut b = [0u8; 4];
                    if pb.read(&mut g.errno, &mut b) != 4 {
                        return read_fail(g, err, row.into(), col.into(), false);
                    }
                    let mut f = f32::from_le_bytes(b);
                    if different_endian(g, mf) {
                        f = swap32(cvtt_f32(f)) as f32;
                    }
                    f64::from(f)
                }
                MAT_DOUBLE => {
                    let mut b = [0u8; 8];
                    if pb.read(&mut g.errno, &mut b) != 8 {
                        return read_fail(g, err, row.into(), col.into(), false);
                    }
                    let mut d = f64::from_le_bytes(b);
                    if different_endian(g, mf) {
                        d = f64::from_bits(d.to_bits().swap_bytes());
                    }
                    dval = d;
                    dval_known = true;
                    d
                }
                _ => continue,
            };
            if let Some(m) = real.as_mut() {
                m[row as usize][col as usize] = v;
            }
        }
    }
    if let Some(im) = imag.as_mut() {
        for col in 0..cols {
            for row in 0..rows {
                if ty == MAT_DOUBLE {
                    let mut b = [0u8; 8];
                    if pb.read(&mut g.errno, &mut b) != 8 {
                        return read_fail(g, err, row.into(), col.into(), true);
                    }
                    dval = f64::from_le_bytes(b);
                    dval_known = true;
                }
                if !dval_known {
                    g.notices
                        .add(&FS_235, "an imaginary number from memory never written");
                    memory(g, "an imaginary number from memory never written");
                }
                if different_endian(g, mf) {
                    dval = f64::from_bits(dval.to_bits().swap_bytes());
                }
                im[row as usize][col as usize] = dval;
            }
        }
    }
    ty
}

/// The numbers as MATLAB wrote them: every type in its own size, signed or not, either byte
/// order; the imaginary part in the real part's type (version 5: after its own element tag).
#[allow(clippy::too_many_arguments)]
fn fixed_numbers(
    g: &mut Globals,
    err: &mut dyn Write,
    pb: &mut Pb,
    mf: &MatFile,
    (ty, size, signed, ok): (i32, usize, bool, bool),
    (rows, cols): (i32, i32),
    real: &mut Option<Nums>,
    imag: &mut Option<Nums>,
) -> i32 {
    // an unsupported type reads nothing, but version 5's type 0 takes the bytes' code (as FreeSurfer)
    let (size, signed) = match (ok, ty) {
        (true, _) => (size, signed),
        (false, MAT_BYTE) => (1, true),
        _ => return ty,
    };
    let other = different_endian(g, mf);
    let mut part = |g: &mut Globals, pb: &mut Pb, m: &mut Nums, imag: bool| -> bool {
        for col in 0..cols {
            for row in 0..rows {
                let mut b = [0u8; 8];
                if pb.read(&mut g.errno, &mut b[..size]) != size {
                    read_fail(g, err, row.into(), col.into(), imag);
                    return false;
                }
                if other {
                    b[..size].reverse();
                }
                let top = b[size - 1] & 0x80 != 0;
                if top && !signed && ty != MAT_FLOAT && ty != MAT_DOUBLE {
                    g.notices
                        .add(&FS_233, "an unsigned number read as signed by FreeSurfer");
                }
                let v = match (ty, signed) {
                    (MAT_BYTE, true) => f64::from(b[0] as i8),
                    (MAT_BYTE, false) => f64::from(b[0]),
                    (MAT_SHORT, true) => f64::from(i16::from_le_bytes([b[0], b[1]])),
                    (MAT_SHORT, false) => f64::from(u16::from_le_bytes([b[0], b[1]])),
                    (MAT_INT, true) => f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                    (MAT_INT, false) => f64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                    (MAT_FLOAT, _) => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                    _ => f64::from_le_bytes(b),
                };
                m[row as usize][col as usize] = v;
            }
        }
        true
    };
    if let Some(m) = real.as_mut()
        && !part(g, pb, m, false)
    {
        return -1;
    }
    if let Some(im) = imag.as_mut() {
        if let Some((start, bytes, small)) = mf.real {
            // version 5: the imaginary part's own element after the real part's (8-byte aligned)
            let end = if small {
                start + 4
            } else {
                start + ((bytes + 7) & !7)
            };
            pb.fseek(&mut g.errno, end, 0);
            let mut tag = [0u8; 8];
            pb.read(&mut g.errno, &mut tag[..4]);
            let w = word(&tag[..4], other);
            if w as u32 >> 16 == 0 {
                pb.read(&mut g.errno, &mut tag[4..]);
            }
        }
        if !part(g, pb, im, true) {
            return -1;
        }
    }
    ty
}

/// Four bytes as a 32-bit number of the file's byte order.
fn word(b: &[u8], other: bool) -> i32 {
    let v = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    if other { v.swap_bytes() } else { v }
}

// ------------------------------------------------------------------ headers

fn diag(g: &Globals, out: &mut dyn Write, bits: u64, msg: &str) {
    diag_printf(g, bits, out, msg);
}

/// `Gdiag & DIAG_SHOW && DIAG_VERBOSE_ON`.
fn show_verbose(g: &Globals) -> bool {
    g.gdiag & (DIAG_SHOW | DIAG_VERBOSE) == DIAG_SHOW | DIAG_VERBOSE
}

/// The bytes glibc's `calloc(n)` gives a program on x86-64 (all zero past `n`): FreeSurfer
/// prints a name of `n` bytes with no zero after it past its end, where these bytes stop.
fn usable(n: i64) -> i64 {
    if n >= 128 * 1024 {
        ((n + 16 + 4095) & !4095) - 16
    } else {
        ((n + 8 + 15) & !15).max(32) - 8
    }
}

/// A name FreeSurfer allocated with `calloc(n)` and filled with `bytes`, as `%s` prints it: to
/// its first zero; one with no zero in its allocation runs into memory contents (FS-239).
fn name_shown<'a>(g: &mut Globals, bytes: &'a [u8], n: i64) -> &'a [u8] {
    if !bytes.contains(&0) && usable(n) <= bytes.len() as i64 {
        g.notices
            .add(&FS_239, "a name printed past its end (memory contents)");
        memory(g, "a name printed past its end (memory contents)");
    }
    cstr(bytes)
}

/// The version 4 header (`MatReadHeader0`, and `MatReadHeader`'s version 4 part when `five`):
/// the name, or `None` after a message.
fn v4_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    pb: &mut Pb,
    mf: &mut MatFile,
    five: bool,
) -> Result<Option<Vec<u8>>, Fatal> {
    let mut b = [0u8; 20];
    for (k, bit) in [U_TYPE, U_MROWS, U_NCOLS, U_IMAGF, U_NAMLEN]
        .into_iter()
        .enumerate()
    {
        b[4 * k..4 * k + 4].copy_from_slice(&field(mf, bit).to_le_bytes());
    }
    let got = pb.read(&mut g.errno, &mut b);
    for (k, bit) in [U_TYPE, U_MROWS, U_NCOLS, U_IMAGF, U_NAMLEN]
        .into_iter()
        .enumerate()
    {
        *field_mut(mf, bit) =
            i32::from_le_bytes([b[4 * k], b[4 * k + 1], b[4 * k + 2], b[4 * k + 3]]);
        if got >= 4 * k + 4 {
            mf.unknown &= !bit;
        }
    }
    if got != 20 {
        badfile(
            g,
            err,
            &format!("{PROG}: only read {got} bytes of header\n"),
        );
        return Ok(None);
    }
    diag(g, out, DIAG_VERBOSE, &format!("type = {}\n", ld(mf.mtype)));
    if different_endian(g, mf) {
        diag(
            g,
            out,
            DIAG_VERBOSE,
            "mat file generated with different endian\n",
        );
        mf.mtype = swap32(mf.mtype);
        mf.mrows = swap32(mf.mrows);
        mf.ncols = swap32(mf.ncols);
        mf.namlen = swap32(mf.namlen);
    }
    diag(
        g,
        out,
        DIAG_VERBOSE,
        &format!("after swap, type = {}\n", ld(mf.mtype)),
    );
    let n1 = mf.namlen.wrapping_add(1);
    diag(
        g,
        out,
        DIAG_VERBOSE,
        &format!("MatReadHeader: nitems = {got}, namelen={n1}\n"),
    );
    if n1 < 0 || big(g, "a .mat variable's name", i64::from(n1)) {
        g.errno = 12;
        log::error_exit(
            g,
            err,
            ERROR_NO_MEMORY,
            &format!("matReadHeader: couldn't allocate {n1} bytes for name"),
        )?;
        return Ok(None);
    }
    let mut name = vec![0u8; n1 as usize];
    let want = mf.namlen;
    let nitems = if want < 0 {
        // fread of SIZE_MAX bytes into a buffer of none: FreeSurfer writes the rest of the file
        // past it
        let mut rest = [0u8; 1];
        let n = pb.read(&mut g.errno, &mut rest);
        if n > 0 {
            g.notices.add(&FS_243, "a name of -1 bytes: FreeSurfer reads the rest of the file past its buffer (a crash or corruption)");
        }
        n as i32
    } else {
        pb.read(&mut g.errno, &mut name[..want as usize]) as i32
    };
    if nitems != want {
        badfile(
            g,
            err,
            &format!(
                "{PROG}: only read {nitems} bytes of name ({} specified)\n",
                ld(want)
            ),
        );
        return Ok(None);
    }
    let head = if five {
        format!(
            "MATFILE5: {} x {}, imag {}, name '",
            ld(mf.mrows),
            ld(mf.ncols),
            mf.imagf
        )
    } else {
        format!(
            "MATFILE: {} x {}, type {}, imagf {}, name '",
            ld(mf.mrows),
            ld(mf.ncols),
            ld(mf.mtype),
            ld(mf.imagf)
        )
    };
    diag_bytes(g, out, DIAG_VERBOSE, &with_name(&head, &name, "'\n"));
    name.truncate(want as usize);
    Ok(Some(name))
}

/// `MatReadHeader0(fp, mf)`.
fn mat_read_header0(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    pb: &mut Pb,
    mf: &mut MatFile,
) -> Result<Option<Vec<u8>>, Fatal> {
    if show_verbose(g) {
        diag(g, out, DIAG_VERBOSE, "MatReadHeader: fp=?, mf=?\n");
    }
    v4_header(g, out, err, pb, mf, false)
}

/// The four bytes of a `long32` local FreeSurfer reads with `fread(&x, 4, 1, fp)`: its value
/// (0 and noted where the file ended before it, FS-241) and whether the read failed with an error.
fn read_local(g: &mut Globals, pb: &mut Pb, err: &mut dyn Write, msg: &str) -> i32 {
    let mut b = [0u8; 4];
    let got = pb.read(&mut g.errno, &mut b);
    if got != 4 {
        if pb.ferror() {
            badfile(g, err, msg);
        }
        g.notices.add(
            &FS_241,
            "a .mat header number read from memory never written (FreeSurfer's value is memory contents; larmorx's 0)",
        );
        memory(
            g,
            "a .mat header number read from memory never written (FreeSurfer's value is memory contents; larmorx's 0)",
        );
    }
    i32::from_le_bytes(b)
}

/// `MatReadHeader(fp, mf, &compressed)`: the version (4 when one of the first four bytes is 0),
/// then the header of the first variable; `(name, compressed)`; a compressed variable (MATLAB 7)
/// closes the file and gives no name.
fn mat_read_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    pb: &mut Pb,
    mf: &mut MatFile,
) -> Result<(Option<Vec<u8>>, bool), Fatal> {
    if show_verbose(g) {
        diag(g, out, DIAG_VERBOSE, "MatReadHeader: fp=?, mf=?\n");
    }
    let mut c4 = [0i8; 4];
    for c in c4.iter_mut() {
        *c = pb.getc(&mut g.errno) as i8;
    }
    if c4.contains(&-1) {
        badfile(
            g,
            err,
            &format!("{PROG}: could not detect the version of the Matfile\n"),
        );
        return Ok((None, false));
    }
    mf.version = if c4.contains(&0) { 4 } else { 5 };
    mf.unknown &= !U_VERSION;
    for &c in c4.iter().rev() {
        pb.ungetc(c);
    }
    if mf.version == 4 {
        return Ok((v4_header(g, out, err, pb, mf, true)?, false));
    }
    pb.fseek(&mut g.errno, 126, 0);
    mf.endian = pb.getc(&mut g.errno) as u8;
    pb.fseek(&mut g.errno, 1, 1);
    if g.notices.reproduce() {
        v5_official(g, out, err, pb, mf)
    } else {
        v5_fixed(g, out, err, pb, mf)
    }
}

const READ_ERROR: &str = "matfile: encountered error while reading Matfile\n";

fn v5_official(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    pb: &mut Pb,
    mf: &mut MatFile,
) -> Result<(Option<Vec<u8>>, bool), Fatal> {
    let other = mf.endian != MATFILE_PC5;
    let mut dt = read_local(g, pb, err, READ_ERROR);
    if other {
        dt = swap32(dt);
    }
    if dt != 14 && dt != 15 {
        let _ = writeln!(
            out,
            "WARNING: matlab elment type is {dt}, which is not a matrix."
        );
    }
    if dt == 15 {
        return Ok((None, true));
    }
    pb.fseek(&mut g.errno, 12, 1);
    let mut tmp = read_local(g, pb, err, READ_ERROR);
    if other {
        tmp = swap32(tmp);
    }
    mf.imagf = 0x10 & i32::from(tmp.to_le_bytes()[3] as i8);
    mf.unknown &= !U_IMAGF;
    pb.fseek(&mut g.errno, 12, 1);
    for bit in [U_MROWS, U_NCOLS] {
        if read_field(g, pb, mf, bit) != 4 && pb.ferror() {
            badfile(g, err, READ_ERROR);
        }
    }
    if other {
        mf.mrows = swap32(mf.mrows);
        mf.ncols = swap32(mf.ncols);
    }
    let (c, d) = if other {
        let c = pb.getc(&mut g.errno) as i8;
        let d = pb.getc(&mut g.errno) as i8;
        pb.ungetc(d);
        pb.ungetc(c);
        (c, d)
    } else {
        let mut x = [0i8; 4];
        for v in x.iter_mut() {
            *v = pb.getc(&mut g.errno) as i8;
        }
        for &v in x.iter().rev() {
            pb.ungetc(v);
        }
        (x[2], x[3])
    };
    let normal = c == 0 && d == 0;
    if normal {
        pb.fseek(&mut g.errno, 4, 1);
    }
    if read_field(g, pb, mf, U_NAMLEN) != 4 && pb.ferror() {
        badfile(g, err, READ_ERROR);
    }
    if other {
        mf.namlen = swap32(mf.namlen);
    }
    if !normal {
        mf.namlen = i32::from(mf.namlen as i16);
    }
    let namlen = known(g, mf, U_NAMLEN, mf.namlen);
    let mut name = Vec::new();
    if namlen < 0 || big(g, "a .mat variable's name", i64::from(namlen)) {
        g.notices.add(
            &FS_243,
            "a name of a negative or huge length (FreeSurfer reads into no buffer: a crash)",
        );
    } else {
        name = vec![0u8; namlen as usize];
        let got = pb.read(&mut g.errno, &mut name);
        if (namlen == 0 || got != namlen as usize) && pb.ferror() {
            badfile(g, err, READ_ERROR);
        }
    }
    let padding = if normal {
        8i32.wrapping_sub(namlen.wrapping_sub(namlen / 8 * 8))
    } else {
        4i32.wrapping_sub(namlen)
    };
    pb.fseek(&mut g.errno, i64::from(padding), 1);
    if read_field(g, pb, mf, U_TYPE) != 4 && pb.ferror() {
        badfile(g, err, READ_ERROR);
    }
    if other {
        mf.mtype = swap32(mf.mtype);
    }
    pb.fseek(&mut g.errno, 4, 1);
    if g.gdiag & DIAG_VERBOSE != 0 {
        let shown = name_shown(g, &name, i64::from(namlen)).to_vec();
        let head = format!(
            "MATFILE5: {} x {}, imag {}, name '",
            ld(known(g, mf, U_MROWS, mf.mrows)),
            ld(known(g, mf, U_NCOLS, mf.ncols)),
            mf.imagf
        );
        diag_bytes(g, out, DIAG_VERBOSE, &with_name(&head, &shown, "'\n"));
    }
    Ok((Some(name), false))
}

/// A version 5 element's tag at the stream's position: (type, bytes, small, where its data
/// starts), or `None` when the file ends in it.
fn sub_tag(g: &mut Globals, pb: &mut Pb, other: bool) -> Option<(i32, i64, bool, i64)> {
    let at = pb.fp.tell() - pb.back.len() as i64;
    let mut b = [0u8; 4];
    if pb.read(&mut g.errno, &mut b) != 4 {
        return None;
    }
    let w = word(&b, other);
    if (w as u32) >> 16 != 0 {
        return Some((w & 0xffff, i64::from((w as u32) >> 16), true, at + 4));
    }
    if pb.read(&mut g.errno, &mut b) != 4 {
        return None;
    }
    Some((w, i64::from(word(&b, other) as u32), false, at + 8))
}

/// The end of an element whose data starts at `data` (small: 4 bytes in its tag's 8).
fn sub_end(data: i64, bytes: i64, small: bool) -> i64 {
    if small {
        data + 4
    } else {
        data + ((bytes + 7) & !7)
    }
}

fn short_header(g: &mut Globals, err: &mut dyn Write) -> Result<(Option<Vec<u8>>, bool), Fatal> {
    g.notices.add(
        &FS_241,
        "the file ends in a version 5 variable's header (FreeSurfer goes on with memory contents)",
    );
    memory(
        g,
        "the file ends in a version 5 variable's header (FreeSurfer goes on with memory contents)",
    );
    badfile(g, err, "matfile: the file ends in the variable's header\n");
    Ok((None, false))
}

/// MATLAB's version 5 element read as its format defines it (the stream at the variable's tag).
fn v5_fixed(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    pb: &mut Pb,
    mf: &mut MatFile,
) -> Result<(Option<Vec<u8>>, bool), Fatal> {
    let other = mf.endian != MATFILE_PC5;
    let mut b = [0u8; 8];
    if pb.read(&mut g.errno, &mut b) != 8 {
        return short_header(g, err);
    }
    let dt = word(&b[..4], other);
    if dt != 14 && dt != 15 {
        let _ = writeln!(
            out,
            "WARNING: matlab elment type is {dt}, which is not a matrix."
        );
    }
    if dt == 15 {
        return Ok((None, true));
    }
    // array flags
    let Some((_, fbytes, fsmall, fdata)) = sub_tag(g, pb, other) else {
        return short_header(g, err);
    };
    let mut fl = [0u8; 4];
    if pb.read(&mut g.errno, &mut fl) != 4 {
        return short_header(g, err);
    }
    let flags = word(&fl, other);
    // FreeSurfer tests bit 28 of the word (0x10 of its high byte) instead of the complex bit 11
    if (flags & 0x0800 != 0) != (flags & 0x1000_0000 != 0) {
        g.notices.add(
            &FS_236,
            "a version 5 matrix's complex flag misread by FreeSurfer",
        );
    }
    mf.imagf = i32::from(flags & 0x0800 != 0);
    // FreeSurfer's value: 0x10 of the word's high byte (16 for a complex flag it sees)
    let fs_imagf = 0x10 & i32::from((flags >> 24) as u8 as i8);
    pb.fseek(&mut g.errno, sub_end(fdata, fbytes, fsmall), 0);
    // dimensions
    let Some((_, dbytes, dsmall, ddata)) = sub_tag(g, pb, other) else {
        return short_header(g, err);
    };
    let ndims = dbytes / 4;
    let mut dims = Vec::new();
    for _ in 0..ndims.min(2) {
        let mut x = [0u8; 4];
        if pb.read(&mut g.errno, &mut x) != 4 {
            return short_header(g, err);
        }
        dims.push(word(&x, other));
    }
    if ndims != 2 {
        g.notices
            .add(&FS_246, format!("a version 5 array of {ndims} dimensions"));
        badfile(
            g,
            err,
            &format!("matfile: an array of {ndims} dimensions is not a matrix\n"),
        );
        return Ok((None, false));
    }
    (mf.mrows, mf.ncols) = (dims[0], dims[1]);
    pb.fseek(&mut g.errno, sub_end(ddata, dbytes, dsmall), 0);
    // name
    let Some((_, nbytes, nsmall, ndata)) = sub_tag(g, pb, other) else {
        return short_header(g, err);
    };
    if big(g, "a .mat variable's name", nbytes) {
        return short_header(g, err);
    }
    let mut name = vec![0u8; nbytes as usize];
    if pb.read(&mut g.errno, &mut name) != name.len() {
        return short_header(g, err);
    }
    if !nsmall && nbytes % 8 == 0 {
        g.notices.add(
            &FS_238,
            "a version 5 name of a multiple of 8 bytes, none included (FreeSurfer skips 8 bytes too many)",
        );
    }
    mf.namlen = nbytes as i32;
    pb.fseek(&mut g.errno, sub_end(ndata, nbytes, nsmall), 0);
    // the real part's tag
    let Some((t, rbytes, rsmall, rdata)) = sub_tag(g, pb, other) else {
        return short_header(g, err);
    };
    if rsmall {
        g.notices.add(
            &FS_237,
            "numbers in a small version 5 element (FreeSurfer reads its tag as their type)",
        );
    }
    mf.mtype = t;
    mf.version = 5;
    mf.unknown = 0;
    mf.real = Some((rdata, rbytes, rsmall));
    pb.fseek(&mut g.errno, rdata, 0);
    if g.gdiag & DIAG_VERBOSE != 0 {
        if nsmall || !name.contains(&0) {
            g.notices.add(&FS_239, "a version 5 name printed whole");
        }
        if fs_imagf != mf.imagf {
            // the message prints the flag: FreeSurfer's 16 (or 0) where larmorx reads 1 (or 0)
            g.notices.add(
                &FS_236,
                "a version 5 matrix's complex flag printed as larmorx reads it (FreeSurfer prints its own bit's value)",
            );
        }
        let head = format!(
            "MATFILE5: {} x {}, imag {}, name '",
            ld(mf.mrows),
            ld(mf.ncols),
            mf.imagf
        );
        diag_bytes(g, out, DIAG_VERBOSE, &with_name(&head, &name, "'\n"));
    }
    Ok((Some(name), false))
}

// ------------------------------------------------------------------ compressed variables

/// zlib's `uncompress(dest, &destLen, source, sourceLen)` (zlib 1.2.11, as FreeSurfer links it):
/// the bytes written to `dest`, zlib's result (`Z_OK` 0, `Z_BUF_ERROR` -5, `Z_DATA_ERROR` -3) and the
/// bytes of `source` it read.
fn uncompress(dest_len: usize, source: &[u8]) -> (Vec<u8>, i32, usize) {
    let mut z = Decompress::new(true);
    let mut dest = vec![0u8; dest_len];
    // zlib's own one-byte buffer when destLen is 0: nothing reaches dest
    let mut one = [0u8; 1];
    let room: &mut [u8] = if dest_len == 0 { &mut one } else { &mut dest };
    let r = loop {
        let (i, o) = (z.total_in() as usize, z.total_out() as usize);
        match z.decompress(&source[i..], &mut room[o..], FlushDecompress::None) {
            Ok(Status::Ok) if z.total_in() as usize != i || z.total_out() as usize != o => {}
            Ok(Status::StreamEnd) => break 0,
            Ok(_) => {
                // no progress: the output is full (Z_BUF_ERROR) or the input ended (Z_DATA_ERROR)
                let full = z.total_out() as usize == room.len();
                break if full && dest_len != 0 { -5 } else { -3 };
            }
            Err(_) => break -3,
        }
    };
    let used = z.total_in() as usize;
    (dest, r, used)
}

/// The decompressed element FreeSurfer reads with `memmove` from `unbuff`: bytes past it are
/// zero within glibc's allocation and memory contents beyond (noted).
struct Unbuff {
    bytes: Vec<u8>,
    /// The bytes `calloc` gave (all zero past `bytes`).
    usable: usize,
}

impl Unbuff {
    fn get<const N: usize>(&self, g: &mut Globals, at: i64) -> [u8; N] {
        let mut b = [0u8; N];
        for (k, v) in b.iter_mut().enumerate() {
            let i = at + k as i64;
            if i < 0 || i as usize >= self.usable {
                g.notices.add(
                    &FS_240,
                    "a compressed variable read past its buffer (memory contents)",
                );
                memory(
                    g,
                    "a compressed variable read past its buffer (memory contents)",
                );
            } else if (i as usize) < self.bytes.len() {
                *v = self.bytes[i as usize];
            }
        }
        b
    }
    fn i32(&self, g: &mut Globals, at: i64) -> i32 {
        i32::from_le_bytes(self.get::<4>(g, at))
    }
}

/// `znzMatReadHeader(fp, mf, &data)` as FreeSurfer reads a compressed variable: the name and the
/// decompressed bytes, or `None` where FreeSurfer crashes (noted).
fn znz_official(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    pb: &mut Pb,
    mf: &mut MatFile,
) -> Option<(Vec<u8>, Unbuff)> {
    const ERR: &str = "Encountered error while reading Matfile\n";
    pb.fseek(&mut g.errno, 126, 0);
    mf.endian = pb.getc(&mut g.errno) as u8;
    pb.fseek(&mut g.errno, 1, 1);
    let mut b = [0u8; 4];
    if pb.read(&mut g.errno, &mut b) != 4 && pb.ferror() {
        badfile(g, err, ERR);
    }
    let mut size = read_local(g, pb, err, ERR);
    if different_endian(g, mf) {
        size = swap32(size);
    }
    if size < 0 || big(g, "a compressed .mat variable", i64::from(size) * 101) {
        g.notices.add(
            &FS_243,
            "a compressed variable of a negative or huge size (FreeSurfer crashes)",
        );
        return None;
    }
    let mut buff = vec![0u8; size as usize];
    let got = pb.read(&mut g.errno, &mut buff);
    if got != 1 && pb.ferror() {
        badfile(g, err, ERR);
    }
    let size_unbuff = size.wrapping_mul(100);
    if size_unbuff < 0 {
        g.notices.add(
            &FS_243,
            "a compressed variable whose buffer size overflows (FreeSurfer crashes)",
        );
        return None;
    }
    let (bytes, isitok, _) = uncompress(size_unbuff as usize, &buff);
    // past the file's bytes FreeSurfer's buffer holds memory contents (larmorx's zeros): noted where
    // the file's bytes alone do not decode to the stream's end (zlib then reads on)
    if got < size as usize && uncompress(size_unbuff as usize, &buff[..got]).1 == -3 {
        g.notices.add(
            &FS_240,
            "a compressed variable cut by the file's end (FreeSurfer decodes memory contents)",
        );
        memory(
            g,
            "a compressed variable cut by the file's end (FreeSurfer decodes memory contents)",
        );
    }
    if isitok != 0 {
        let _ = writeln!(out, "ERROR: in uncompressing");
        if isitok == -5 {
            log::error_printf(
                g,
                err,
                DIAG_VERBOSE as i32,
                "    not enough memory in buffer\n",
            );
        }
    }
    let u = Unbuff {
        usable: usable(i64::from(size_unbuff)) as usize,
        bytes,
    };
    let m: i8 = 1 << 4;
    mf.imagf = i32::from(m & (u.get::<1>(g, 18)[0] as i8));
    mf.unknown &= !U_IMAGF;
    // memmove of sizeof(long), 8 bytes, into 4-byte fields: each also writes the next field
    mf.mrows = u.i32(g, 32);
    mf.ncols = u.i32(g, 36);
    mf.ncols = u.i32(g, 36);
    mf.imagf = u.i32(g, 40);
    mf.unknown &= !(U_MROWS | U_NCOLS);
    if different_endian(g, mf) {
        mf.mrows = swap32(mf.mrows);
        mf.ncols = swap32(mf.ncols);
    }
    let mut tmp = u.i32(g, 40);
    if different_endian(g, mf) {
        tmp = swap32(tmp);
    }
    let td = tmp.to_le_bytes();
    let name;
    if td[0] == 0 && td[1] == 0 {
        mf.namlen = u.i32(g, 44);
        mf.version = u.i32(g, 48);
        mf.unknown &= !(U_NAMLEN | U_VERSION);
        let n = mf.namlen;
        if n < 0 || big(g, "a .mat variable's name", i64::from(n)) {
            g.notices.add(
                &FS_243,
                "a name of a negative or huge length (FreeSurfer crashes)",
            );
            return None;
        }
        name = (0..i64::from(n))
            .map(|k| u.get::<1>(g, 48 + k)[0])
            .collect::<Vec<u8>>();
        let padding = 48i32.wrapping_add(n.wrapping_sub(1) / 8 + 1);
        mf.mtype = u.i32(g, i64::from(padding));
        mf.mrows = u.i32(g, i64::from(padding) + 4);
        mf.unknown &= !U_TYPE;
        if different_endian(g, mf) {
            mf.mtype = swap32(mf.mtype);
        }
    } else {
        mf.namlen = u.i32(g, 40);
        mf.version = u.i32(g, 44);
        mf.unknown &= !U_VERSION;
        if different_endian(g, mf) {
            mf.namlen = swap32(mf.namlen);
        }
        mf.namlen = i32::from(mf.namlen as i16);
        mf.unknown &= !U_NAMLEN;
        let n = mf.namlen;
        if n < 0 {
            g.notices
                .add(&FS_243, "a name of a negative length (FreeSurfer crashes)");
            return None;
        }
        name = (0..i64::from(n))
            .map(|k| u.get::<1>(g, 44 + k)[0])
            .collect::<Vec<u8>>();
        mf.mtype = u.i32(g, 48);
        mf.mrows = u.i32(g, 52);
        mf.unknown &= !U_TYPE;
        if different_endian(g, mf) {
            mf.mtype = swap32(mf.mtype);
        }
    }
    let shown = name_shown(g, &name, i64::from(mf.namlen)).to_vec();
    let head = format!(
        "MATFILE5: {} x {}, type {}, imagf {}, name '",
        ld(mf.mrows),
        ld(mf.ncols),
        ld(mf.mtype),
        mf.imagf
    );
    log::error_printf_bytes(
        g,
        err,
        DIAG_VERBOSE as i32,
        &with_name(&head, &shown, "'\n"),
    );
    Some((name, u))
}

/// `znzreadMatFile(unbuff, mf, real, imag)`: the numbers at FreeSurfer's fixed offset 56.
fn znz_numbers(
    g: &mut Globals,
    err: &mut dyn Write,
    u: &Unbuff,
    mf: &MatFile,
    real: &mut Option<Nums>,
    imag: &mut Option<Nums>,
) {
    let t = mf.mtype;
    let (ty, _, _, ok) = element(5, t);
    if !ok {
        badfile(
            g,
            err,
            &format!("unsupported matlab format {ty} (unknown)\n"),
        );
    }
    let (rows, cols) = (mf.mrows, mf.ncols);
    let size: i32 = match ty {
        MAT_BYTE => 1,
        MAT_SHORT => 2,
        MAT_INT | MAT_DOUBLE => 8,
        MAT_FLOAT => 4,
        _ => 0,
    };
    let real_cols = if (0..=4).contains(&ty) { cols } else { 0 };
    let off = |row: i32, col: i32| {
        56i32
            .wrapping_add(col.wrapping_mul(rows).wrapping_mul(size))
            .wrapping_add(row.wrapping_mul(size))
    };
    // no numbers kept (a failed allocation: no loop runs or FreeSurfer crashes, noted)
    if real.is_none() {
        return;
    }
    // past the decoded buffer's allocation FreeSurfer reads memory contents, then faults: the
    // reading stops at the first such number (noted)
    let beyond = |at: i32| {
        let at = i64::from(at);
        at < 0 || at + i64::from(size) > u.usable as i64
    };
    'real: for col in 0..real_cols {
        for row in 0..rows {
            if beyond(off(row, col)) {
                g.notices.add(
                    &FS_240,
                    "a compressed variable read past its buffer (memory contents)",
                );
                memory(
                    g,
                    "a compressed variable read past its buffer (memory contents)",
                );
                break 'real;
            }
            let at = i64::from(off(row, col));
            let v = match ty {
                MAT_BYTE => f64::from(u.get::<1>(g, at)[0] as i8),
                MAT_SHORT => {
                    let mut s = i16::from_le_bytes(u.get::<2>(g, at));
                    if different_endian(g, mf) {
                        s = s.swap_bytes();
                    }
                    f64::from(s)
                }
                MAT_INT => {
                    let mut l = i64::from_le_bytes(u.get::<8>(g, at));
                    if different_endian(g, mf) {
                        l = i64::from(swap32(l as i32));
                    }
                    l as f64
                }
                MAT_FLOAT => {
                    let mut f = f32::from_le_bytes(u.get::<4>(g, at));
                    if different_endian(g, mf) {
                        f = swap32(cvtt_f32(f)) as f32;
                    }
                    f64::from(f)
                }
                MAT_DOUBLE => {
                    let mut d = f64::from_le_bytes(u.get::<8>(g, at));
                    if different_endian(g, mf) {
                        d = f64::from_bits(d.to_bits().swap_bytes());
                    }
                    d
                }
                _ => continue,
            };
            if let Some(m) = real.as_mut() {
                m[row as usize][col as usize] = v;
            }
        }
    }
    if let Some(im) = imag.as_mut() {
        let mut dval = 1.0f64;
        'imag: for col in 0..cols {
            for row in 0..rows {
                if ty == MAT_DOUBLE {
                    let at = off(row, col).wrapping_add(rows.wrapping_mul(cols).wrapping_mul(8));
                    if beyond(at) {
                        g.notices.add(
                            &FS_240,
                            "a compressed variable read past its buffer (memory contents)",
                        );
                        memory(
                            g,
                            "a compressed variable read past its buffer (memory contents)",
                        );
                        break 'imag;
                    }
                    dval = f64::from_le_bytes(u.get::<8>(g, i64::from(at)));
                }
                if different_endian(g, mf) {
                    dval = f64::from_bits(dval.to_bits().swap_bytes());
                }
                im[row as usize][col as usize] = dval;
            }
        }
    }
}

/// A compressed variable decoded whole and read as an uncompressed one: the name and the
/// decompressed element as a stream at its numbers.
fn znz_fixed(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    pb: &mut Pb,
    mf: &mut MatFile,
) -> Result<Option<(Vec<u8>, Pb)>, Fatal> {
    pb.fseek(&mut g.errno, 126, 0);
    mf.endian = pb.getc(&mut g.errno) as u8;
    pb.fseek(&mut g.errno, 1, 1);
    let other = mf.endian != MATFILE_PC5;
    let mut b = [0u8; 8];
    if pb.read(&mut g.errno, &mut b) != 8 {
        short_header(g, err)?;
        return Ok(None);
    }
    let size = i64::from(word(&b[4..], other) as u32);
    if big(g, "a compressed .mat variable", size) {
        short_header(g, err)?;
        return Ok(None);
    }
    let mut buff = vec![0u8; size as usize];
    let got = pb.read(&mut g.errno, &mut buff);
    buff.truncate(got);
    // the whole stream, however large (FreeSurfer stops at 100 times the compressed size)
    let mut z = Decompress::new(true);
    let mut dec = Vec::with_capacity(size as usize * 4);
    let ok = loop {
        if dec.len() == dec.capacity() {
            if dec.len() as i64 > MAX_BYTES {
                break false;
            }
            dec.reserve(dec.len().max(4096));
        }
        let before = (z.total_in(), z.total_out());
        let i = z.total_in() as usize;
        match z.decompress_vec(&buff[i..], &mut dec, FlushDecompress::None) {
            Ok(Status::StreamEnd) => break true,
            Ok(_) if (z.total_in(), z.total_out()) != before => {}
            Ok(_) if dec.len() == dec.capacity() => {}
            _ => break false,
        }
    };
    if (dec.len() as i64) > 100 * size {
        g.notices.add(
            &FS_240,
            "a compressed variable decoded beyond 100 times its size (FreeSurfer stops there)",
        );
    }
    if !ok {
        let _ = writeln!(out, "ERROR: in uncompressing");
        return Ok(None);
    }
    let mut inner = Pb {
        fp: CFile::from_bytes(dec),
        back: Vec::new(),
    };
    let (name, nested) = v5_fixed(g, out, err, &mut inner, mf)?;
    if nested {
        badfile(
            g,
            err,
            "matfile: a compressed variable inside a compressed variable\n",
        );
        return Ok(None);
    }
    let Some(name) = name else {
        return Ok(None);
    };
    let head = format!(
        "MATFILE5: {} x {}, type {}, imagf {}, name '",
        ld(mf.mrows),
        ld(mf.ncols),
        ld(mf.mtype),
        mf.imagf
    );
    log::error_printf_bytes(g, err, DIAG_VERBOSE as i32, &with_name(&head, &name, "'\n"));
    Ok(Some((name, inner)))
}

// ------------------------------------------------------------------ readers

/// A matrix of `real` (and `imag`) as FreeSurfer fills one (`MatrixAlloc` and its numbers as
/// floats); `None` where the size is beyond the machine (noted, FS-243).
#[allow(clippy::too_many_arguments)]
fn to_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rows: i32,
    cols: i32,
    complex: bool,
    real: &Option<Nums>,
    imag: &Option<Nums>,
) -> Result<Option<Matrix>, Fatal> {
    let pair = if complex { 8 } else { 4 };
    if big(
        g,
        "a .mat matrix",
        (i64::from(rows.max(0)) * i64::from(cols.max(0))).saturating_mul(pair),
    ) {
        return Ok(None);
    }
    let ty = if complex { MATRIX_COMPLEX } else { MATRIX_REAL };
    let Some(mut mat) = matrix_alloc(g, out, err, rows, cols, ty)? else {
        return Ok(None);
    };
    // (no numbers when either size is 0: FreeSurfer's empty row loop does nothing)
    let rows_with_numbers = if cols > 0 { rows } else { 0 };
    for row in 1..=rows_with_numbers {
        for col in 1..=cols {
            let (r, c) = ((row - 1) as usize, (col - 1) as usize);
            let re = real.as_ref().map_or(0.0, |m| m[r][c]) as f32;
            if complex {
                let im = imag.as_ref().map_or(0.0, |m| m[r][c]) as f32;
                mat.set_celt(row, col, re, im);
            } else {
                mat.set_relt(row, col, re);
            }
        }
    }
    Ok(Some(mat))
}

/// `matAlloc` of a header's size, refusing one beyond the machine (FS-243: then as a failed
/// allocation).
fn alloc_for(g: &mut Globals, err: &mut dyn Write, rows: i32, cols: i32) -> Option<Nums> {
    let numbers = (8 * i64::from(rows.max(0))).saturating_mul(i64::from(cols.max(0)));
    if big(g, "a .mat matrix", nums_bytes(rows, cols)) || numbers > MAX_BYTES {
        badfile(
            g,
            err,
            &format!("could not allocate {rows} x {cols} matrix\n"),
        );
        return None;
    }
    mat_alloc(g, err, rows, cols)
}

fn open(g: &mut Globals, dir: Option<&Path>, fname: &str) -> Option<Pb> {
    match CFile::open(dir, fname, "rb") {
        Ok(fp) => Some(Pb {
            fp,
            back: Vec::new(),
        }),
        Err(e) => {
            g.errno = e;
            None
        }
    }
}

/// `MatlabRead(fname)`: `Using MatlabRead2()` on standard output when `Gdiag_no` is above 0,
/// then [`matlab_read2`].
pub fn matlab_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Matrix>, Fatal> {
    if g.gdiag_no > 0 {
        let _ = writeln!(out, "Using MatlabRead2()");
    }
    matlab_read2(g, out, err, dir, fname)
}

enum Numbers {
    Stream(Pb),
    Buffer(Unbuff),
}

/// `MatlabRead2(fname)`: the first variable of a version 4 or 5 file as a real (or complex)
/// matrix of floats; `None` when the file cannot be opened (no message) or its header cannot be
/// read (`MatlabRead: readHeader returned NULL` as an error).
pub fn matlab_read2(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Matrix>, Fatal> {
    if show_verbose(g) {
        diag(
            g,
            out,
            DIAG_SHOW,
            &format!("MatlabRead: opening file {fname}\n"),
        );
    }
    let Some(mut pb) = open(g, dir, fname) else {
        return Ok(None);
    };
    let mut mf = MatFile::unwritten();
    let (mut name, compressed) = mat_read_header(g, out, err, &mut pb, &mut mf)?;
    let mut src = Numbers::Stream(pb);
    if compressed {
        g.notices.add(&FS_240, "a compressed (MATLAB 7) variable");
        let Some(mut zp) = open(g, dir, fname) else {
            return Ok(None);
        };
        if g.notices.reproduce() {
            match znz_official(g, out, err, &mut zp, &mut mf) {
                Some((n, u)) => {
                    name = Some(n);
                    src = Numbers::Buffer(u);
                }
                None => return Ok(None),
            }
        } else {
            match znz_fixed(g, out, err, &mut zp, &mut mf)? {
                Some((n, inner)) => {
                    name = Some(n);
                    src = Numbers::Stream(inner);
                }
                None => name = None,
            }
        }
    }
    if show_verbose(g) {
        diag(g, out, DIAG_SHOW, "MatlabRead: reading header\n");
    }
    if name.is_none() {
        badfile(g, err, "MatlabRead: readHeader returned NULL\n");
        return Ok(None);
    }
    let rows = known(g, &mf, U_MROWS, mf.mrows);
    let cols = known(g, &mf, U_NCOLS, mf.ncols);
    if show_verbose(g) {
        diag(
            g,
            out,
            DIAG_SHOW,
            &format!("MatlabRead: allocating real matrix (r={rows},c={cols})\n"),
        );
    }
    let mut real = alloc_for(g, err, rows, cols);
    if show_verbose(g) {
        diag(g, out, DIAG_SHOW, "MatlabRead: done real mtx alloc\n");
    }
    let complex = known(g, &mf, U_IMAGF, mf.imagf) != 0;
    let mut imag = if complex {
        alloc_for(g, err, rows, cols)
    } else {
        None
    };
    match &mut src {
        Numbers::Stream(pb) => {
            read_mat_file(g, err, pb, &mf, &mut real, &mut imag);
        }
        Numbers::Buffer(u) => znz_numbers(g, err, u, &mf, &mut real, &mut imag),
    }
    let mat = to_matrix(g, out, err, rows, cols, complex, &real, &imag)?;
    if mat.is_some() && show_verbose(g) {
        diag(g, out, DIAG_SHOW, "MatlabRead: done\n");
    }
    // FreeSurfer leaves the file open (FS-242): closed here
    Ok(mat)
}

/// `(char)x` for a double: the official build converts to a 32-bit integer (`cvttsd2si`: out of
/// range or NaN gives -2147483648) and keeps its low byte (FS-244); the default saturates to
/// -128..127 (NaN 0).
fn to_char(g: &mut Globals, x: f64) -> u8 {
    let official = if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        0
    } else {
        (x as i32) as u8
    };
    let fixed = if x.is_nan() {
        0
    } else {
        x.clamp(-128.0, 127.0) as i8 as u8
    };
    if official != fixed {
        g.notices.add(&FS_244, format!("{x} as a byte"));
        if g.notices.reproduce() {
            return official;
        }
    }
    fixed
}

/// `MatFileRead(fname, type)`: a version 4 file's first variable, its numbers in `data` as
/// bytes (`MAT_BYTE`) or floats (`MAT_FLOAT`; another type: `MatFileRead: unsupported data
/// format <type>` as an error and no data); `None` when the file cannot be opened. A header it
/// cannot read leaves the fields as far as read (and 0) and goes on (FS-250).
pub fn mat_file_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    ty: i32,
) -> Result<Option<MatFile>, Fatal> {
    let Some(mut pb) = open(g, dir, fname) else {
        return Ok(None);
    };
    let mut mf = MatFile::default();
    if mat_read_header0(g, out, err, &mut pb, &mut mf)?.is_none() {
        g.notices.add(
            &larmorx_core::defects::FS_250,
            "MatFileRead goes on after a header it could not read",
        );
    }
    let (rows, cols) = (mf.mrows, mf.ncols);
    let mut real = alloc_for(g, err, rows, cols);
    let mut imag = if mf.imagf != 0 {
        alloc_for(g, err, rows, cols)
    } else {
        None
    };
    read_mat_file(g, err, &mut pb, &mf, &mut real, &mut imag);
    if real.is_none() && rows > 0 {
        g.notices.add(
            &FS_243,
            "MatFileRead frees rows it never allocated (FreeSurfer crashes)",
        );
        return Ok(None);
    }
    // calloc(nrows * ncols, size): the product of two ints (it wraps), sign-extended
    let n = rows.wrapping_mul(cols);
    let k: i64 = if ty == MAT_FLOAT { 4 } else { 1 };
    let fits = n >= 0 && !big(g, "MatFileRead's numbers", k * i64::from(n));
    let mut data = match ty {
        MAT_BYTE | MAT_FLOAT if fits => Some(Vec::with_capacity((k * i64::from(n)) as usize)),
        MAT_BYTE | MAT_FLOAT => None,
        _ => {
            badfile(
                g,
                err,
                &format!("MatFileRead: unsupported data format {ty}\n"),
            );
            None
        }
    };
    if let (Some(d), Some(m)) = (data.as_mut(), real.as_ref()) {
        for &v in m.v.iter().take(n as usize) {
            if ty == MAT_BYTE {
                let b = to_char(g, v);
                d.push(b);
            } else {
                d.extend_from_slice(&(v as f32).to_le_bytes());
            }
        }
    }
    if let Some(d) = data.as_mut() {
        d.resize((k * i64::from(n)) as usize, 0);
    }
    mf.data = data;
    let _ = imag;
    Ok(Some(mf))
}

/// `ReadMatlabFileContents(fname)`: every variable of a version 4 file (until the file ends, a
/// header cannot be read, or a variable starts with the byte 0xff: FS-251); `None` after
/// `ERROR: could not open <fname>` or `ERROR: did not find any variable in <fname>` (standard
/// output). FreeSurfer keeps at most 1,000 (more overflow its lists: FS-248).
pub fn read_matlab_file_contents(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Mlfc>, Fatal> {
    let Some(mut pb) = open(g, dir, fname) else {
        let _ = writeln!(out, "ERROR: could not open {fname}");
        return Ok(None);
    };
    let mut mlfc = Mlfc {
        mfile: fname.as_bytes().to_vec(),
        vars: Vec::new(),
    };
    let mut mf = MatFile::default();
    loop {
        let c = pb.getc(&mut g.errno) as i8;
        if c == -1 {
            break;
        }
        pb.ungetc(c);
        let Some(name) = mat_read_header0(g, out, err, &mut pb, &mut mf)? else {
            break;
        };
        let (rows, cols) = (mf.mrows, mf.ncols);
        let mut real = alloc_for(g, err, rows, cols);
        let mut imag = if mf.imagf != 0 {
            alloc_for(g, err, rows, cols)
        } else {
            None
        };
        read_mat_file(g, err, &mut pb, &mf, &mut real, &mut imag);
        let Some(mat) = to_matrix(g, out, err, rows, cols, mf.imagf != 0, &real, &imag)? else {
            return Ok(None);
        };
        if mlfc.vars.len() >= 1000 {
            g.notices.add(
                &FS_248,
                "a .mat file of more than 1,000 variables (FreeSurfer writes past its lists)",
            );
            if g.notices.reproduce() {
                return Ok(None);
            }
        }
        mlfc.vars.push((name, mat));
    }
    if mlfc.vars.is_empty() {
        let _ = writeln!(out, "ERROR: did not find any variable in {fname}");
        return Ok(None);
    }
    Ok(Some(mlfc))
}

/// `ReadMatlabFileVariable(fname, varname)`: a copy of the first variable named `varname`;
/// `None` after [`read_matlab_file_contents`]'s messages or `ERROR: varname <varname> not found
/// in <fname>` (standard output).
pub fn read_matlab_file_variable(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    varname: &[u8],
) -> Result<Option<Matrix>, Fatal> {
    let Some(mlfc) = read_matlab_file_contents(g, out, err, dir, fname)? else {
        return Ok(None);
    };
    let want = cstr(varname);
    let mut m = None;
    for (name, mat) in &mlfc.vars {
        if cstr(name) == want {
            if let Ret::New(c) = matrix_copy(g, out, err, Some(mat), None)? {
                m = Some(c);
            }
            break;
        }
    }
    if m.is_none() {
        let _ = out.write_all(&with_name(
            "ERROR: varname ",
            want,
            &format!(" not found in {fname}\n"),
        ));
    }
    Ok(m)
}

/// `MLFCprint(fp, mlfc)`: the file's name, the number of variables, then each variable's number,
/// name and matrix (`MatrixPrint`); 0.
pub fn mlfc_print(
    g: &mut Globals,
    fp: &mut dyn Write,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mlfc: &Mlfc,
) -> i32 {
    let _ = fp.write_all(&with_name("mfile: ", &mlfc.mfile, "\n"));
    let _ = writeln!(fp, "number of variables: {}", mlfc.vars.len());
    for (n, (name, mat)) in mlfc.vars.iter().enumerate() {
        let _ = writeln!(fp, "n = {n} ---------------------");
        let _ = fp.write_all(&with_name("", name, "\n"));
        matrix_print(g, Some(&mut *fp), out, err, Some(mat), 5);
    }
    0
}

/// `MLFCfree(&mlfc)`: 0, `mlfc` `None`.
pub fn mlfc_free(mlfc: &mut Option<Mlfc>) -> i32 {
    *mlfc = None;
    0
}

/// `Matlab_Install_printf(f)`: FreeSurfer keeps the function and never calls it; 0.
pub fn matlab_install_printf() -> i32 {
    0
}

/// The defects of this module whose notices the probe counts as FreeSurfer's results depending
/// on memory (`VARY`) or not comparable (`SKIP`).
pub fn probe_class(id: &str, detail: &str) -> Option<&'static str> {
    let _ = detail;
    match id {
        "FS-243" | "FS-248" => Some("SKIP"),
        "FS-245" => Some("VARY"),
        _ => None,
    }
}
