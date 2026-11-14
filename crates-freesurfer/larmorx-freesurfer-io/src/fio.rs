// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/fio.cpp` of FreeSurfer 8.2.0 as its reached functions use it (queue item 3.1-25):
//!
//! - the big-endian readers and writers of one number on a C stream (`fread1`, `fread2`,
//!   `fread3`, `fread4`, `getf`, `freadDouble`, `freadInt`, `freadLong`, `freadShort`,
//!   `freadIntEx`, `freadFloatEx`, `freadDoubleEx`, `freadShortEx`, `freadFloat`, `fwrite1`,
//!   `fwrite2`, `fwrite3`, `fwrite4`, `putf`, `fwriteShort`, `fwriteInt`, `fwriteLong`,
//!   `fwriteFloat`, `fwriteDouble`) and on a znzlib file (`znzread1` to `znzread4`,
//!   `znzreadShort`, `znzreadInt`, `znzreadLong`, `znzreadFloat`, `znzreadDouble`,
//!   `znzreadIntEx`, `znzreadFloatEx`, `znzreadDoubleEx`, `znzreadShortEx`, `znzwrite1` to
//!   `znzwrite4`, `znzwriteShort`, `znzwriteUShort`, `znzwriteInt`, `znzwriteLong`,
//!   `znzwriteFloat`, `znzwriteDouble`): the number's bytes as x86-64's memory holds them,
//!   reversed; a read the file cannot complete keeps the bytes it got (FS-49 and FS-167: the
//!   others are memory contents in FreeSurfer, 0 here);
//! - the name helpers `fio_dirname`, `fio_basename`, `fio_extension` (FS-168) and the file tests
//!   `fio_DirIsWritable` (FS-169), `fio_FileExistsReadable`, `fio_IsDirectory`;
//! - `TSV::read`, `TSV::print`, `TSV::write` (FS-170 to FS-173) on
//!   `larmorx_freesurfer_core::fio::Tsv`.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{
    Defect, FS_49, FS_167, FS_168, FS_169, FS_170, FS_171, FS_172, FS_173,
};
use larmorx_freesurfer_core::cscan::{self, Scan};
use larmorx_freesurfer_core::cstd;
use larmorx_freesurfer_core::fio::Tsv;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE};
use larmorx_freesurfer_core::machine;
use larmorx_freesurfer_core::utils;
use larmorx_io::znz::ZnzFile;

use crate::cfile::CFile;

/// A read of `n` bytes that the file ended after `got`: the missing bytes are 0 here, memory
/// contents in FreeSurfer (`defect`).
pub(crate) fn memory(g: &mut Globals, defect: &'static Defect, what: &str, got: usize, n: usize) {
    g.notices.add(
        defect,
        format!(
            "{what}: the file ended after {got} of the number's {n} bytes: the others read as 0 (FreeSurfer's are memory contents)"
        ),
    );
}

/// `fread(&x, N, 1, fp)` into memory that holds `init`: the count returned, the memory after it
/// (the file's bytes first, `init`'s where the file ended) and how many bytes the file gave.
fn fread_n<const N: usize>(fp: &mut CFile, init: [u8; N]) -> (i32, [u8; N], usize) {
    let mut b = init;
    let got = fp.read_bytes(&mut b);
    (i32::from(got == N), b, got)
}

/// `fread1(&v, fp)`: one unsigned byte; the count read (1 or 0) and the value.
pub fn fread1(g: &mut Globals, fp: &mut CFile) -> (i32, i32) {
    let (ret, b, got) = fread_n(fp, [0u8; 1]);
    if got < 1 {
        memory(g, &FS_167, "fread1", got, 1);
    }
    (ret, i32::from(b[0]))
}

/// `fread2(&v, fp)`: a big-endian `short`, widened with its sign.
pub fn fread2(g: &mut Globals, fp: &mut CFile) -> (i32, i32) {
    let (ret, b, got) = fread_n(fp, [0u8; 2]);
    if got < 2 {
        memory(g, &FS_167, "fread2", got, 2);
    }
    (ret, i32::from(i16::from_be_bytes(b)))
}

/// `fread3(&v, fp)`: a big-endian 3-byte unsigned number (the missing bytes of a short read are
/// 0: FreeSurfer clears the number first).
pub fn fread3(fp: &mut CFile) -> (i32, i32) {
    let (ret, b, _) = fread_n(fp, [0u8; 3]);
    let i = u32::from_be_bytes([b[0], b[1], b[2], 0]);
    (ret, ((i >> 8) & 0xff_ffff) as i32)
}

/// `fread4(&v, fp)`: a big-endian `float`; the count read (1 or 0) and the value (FS-167: the
/// missing bytes of a short read).
pub fn fread4(g: &mut Globals, fp: &mut CFile) -> (i32, f32) {
    let (ret, b, got) = fread_n(fp, [0u8; 4]);
    if got < 4 {
        memory(g, &FS_167, "fread4", got, 4);
    }
    (ret, f32::from_be_bytes(b))
}

/// `getf(fp)`: a big-endian `float`; `getf: fread failed` (an error) when the file does not hold
/// it (FS-167).
pub fn getf(g: &mut Globals, err: &mut dyn Write, fp: &mut CFile) -> f32 {
    let (ret, b, got) = fread_n(fp, [0u8; 4]);
    if ret != 1 {
        log::error_printf(g, err, ERROR_BADFILE, "getf: fread failed");
    }
    if got < 4 {
        memory(g, &FS_167, "getf", got, 4);
    }
    f32::from_be_bytes(b)
}

/// `freadShort(fp)`: a big-endian `short`; `freadShort: fread failed` (an error) when the file
/// does not hold it (FS-167).
pub fn fread_short(g: &mut Globals, err: &mut dyn Write, fp: &mut CFile) -> i16 {
    let (ret, b, got) = fread_n(fp, [0u8; 2]);
    if ret != 1 {
        log::error_printf(g, err, ERROR_BADFILE, "freadShort: fread failed");
    }
    if got < 2 {
        memory(g, &FS_167, "freadShort", got, 2);
    }
    i16::from_be_bytes(b)
}

/// `freadFloatEx(&f, fp)`: as [`fread_int_ex`] for a `float` (its bits are only moved).
pub fn fread_float_ex(fp: &mut CFile, pf: &mut f32) -> i32 {
    let (ret, b, _) = fread_n(fp, pf.to_le_bytes());
    *pf = f32::from_be_bytes(b);
    ret
}

/// `freadDoubleEx(&d, fp)`: as [`fread_int_ex`] for a `double`.
pub fn fread_double_ex(fp: &mut CFile, pd: &mut f64) -> i32 {
    let (ret, b, _) = fread_n(fp, pd.to_le_bytes());
    *pd = f64::from_be_bytes(b);
    ret
}

/// `freadShortEx(&s, fp)`: as [`fread_int_ex`] for a `short`.
pub fn fread_short_ex(fp: &mut CFile, ps: &mut i16) -> i32 {
    let (ret, b, _) = fread_n(fp, ps.to_le_bytes());
    *ps = i16::from_be_bytes(b);
    ret
}

/// The memory `freadInt` and `freadDouble` read into, as FreeSurfer's official build has it for
/// the calls one function makes (FS-49): eight bytes, `freadDouble`'s number over all of them,
/// `freadInt`'s over bytes 4 to 7. A read the file cannot complete keeps there the bytes the
/// earlier number left: `freadInt` its file's bytes in file order, `freadDouble` its value least
/// significant byte first. A caller that models its own calls keeps one slot across them and
/// calls [`Slot49::clobber`] where FreeSurfer calls anything else between two of them (that memory
/// then holds what the other function left: unknown here). Unknown bytes read as 0 and are noted
/// as memory contents; with `reproduce` the known ones are FreeSurfer's.
#[derive(Clone, Debug, Default)]
pub struct Slot49 {
    b: [u8; 8],
    known: [bool; 8],
    /// A number cut by the file took a byte of memory FreeSurfer never wrote (or another
    /// function's).
    memory: bool,
}

impl Slot49 {
    /// A slot that holds memory FreeSurfer never wrote (a caller's first call).
    pub fn new() -> Slot49 {
        Slot49::default()
    }

    /// Another function ran between two calls: the slot's bytes are unknown again.
    pub fn clobber(&mut self) {
        self.b = [0; 8];
        self.known = [false; 8];
    }

    /// Whether a number the file cut took a byte this slot did not know (memory contents in
    /// FreeSurfer).
    pub fn took_memory(&self) -> bool {
        self.memory
    }

    /// `freadInt`'s `fread` gave `got` (the bytes `b[..got]`): the slot's bytes 4 to 7 after it.
    fn put_int(&mut self, b: &[u8; 4], got: usize) {
        self.b[4..4 + got].copy_from_slice(&b[..got]);
        self.known[4..4 + got].fill(true);
    }

    /// A number of `n` bytes at `at` that the file cut after `got`: FreeSurfer's bytes, 0 where the
    /// slot is unknown or where the default reads 0; whether a missing byte was unknown.
    fn take(&mut self, g: &Globals, at: usize, n: usize, got: usize) -> (Vec<u8>, bool) {
        let mut v = self.b[at..at + n].to_vec();
        let mut unknown = false;
        for i in got..n {
            unknown |= !self.known[at + i];
            if !self.known[at + i] || !g.notices.reproduce() {
                v[i] = 0;
            }
        }
        self.memory |= unknown;
        (v, unknown)
    }
}

/// FS-49's notice for a number of `n` bytes the file ended after `got` of.
fn note49(g: &mut Globals, what: &str, got: usize, n: usize, unknown: bool) {
    if unknown {
        memory(g, &FS_49, what, got, n);
    } else if g.notices.reproduce() {
        g.notices.add(
            &FS_49,
            format!("{what}: the file ended after {got} of the number's {n} bytes: the others are the earlier number's, as in FreeSurfer"),
        );
    } else {
        g.notices.add(
            &FS_49,
            format!("{what}: the file ended after {got} of the number's {n} bytes: the others read as 0 (FreeSurfer's are the earlier number's)"),
        );
    }
}

/// `freadDouble(fp)`: a big-endian `double`; `freadDouble: fread failed` (an error) when the file
/// does not hold it (FS-49: its missing bytes, memory contents).
pub fn fread_double(g: &mut Globals, err: &mut dyn Write, fp: &mut CFile) -> f64 {
    fread_double_in(g, err, fp, &mut Slot49::new())
}

/// [`fread_double`] into the caller's model of FreeSurfer's memory ([`Slot49`]: FS-49's missing
/// bytes are the earlier number's where it knows them).
pub fn fread_double_in(
    g: &mut Globals,
    err: &mut dyn Write,
    fp: &mut CFile,
    slot: &mut Slot49,
) -> f64 {
    let (ret, b, got) = fread_n(fp, [0u8; 8]);
    slot.b[..got].copy_from_slice(&b[..got]);
    slot.known[..got].fill(true);
    let (v, unknown) = slot.take(g, 0, 8, got);
    if got < 8 {
        note49(g, "freadDouble", got, 8, unknown);
    }
    if ret != 1 {
        log::error_printf(g, err, ERROR_BADFILE, "freadDouble: fread failed");
    }
    // the swapped value is stored back over the number's memory
    slot.b.reverse();
    slot.known.reverse();
    f64::from_be_bytes(v.try_into().unwrap())
}

/// `freadInt(fp)`: a big-endian `int`, with no message when the file does not hold it (its test
/// can never be true; FS-49: its missing bytes, memory contents).
pub fn fread_int(g: &mut Globals, fp: &mut CFile) -> i32 {
    fread_int_in(g, fp, &mut Slot49::new())
}

/// [`fread_int`] into the caller's model of FreeSurfer's memory ([`Slot49`]: FS-49's missing
/// bytes are the earlier number's where it knows them).
pub fn fread_int_in(g: &mut Globals, fp: &mut CFile, slot: &mut Slot49) -> i32 {
    let (_, b, got) = fread_n(fp, [0u8; 4]);
    slot.put_int(&b, got);
    let (v, unknown) = slot.take(g, 4, 4, got);
    if got < 4 {
        note49(g, "freadInt", got, 4, unknown);
    }
    i32::from_be_bytes(v.try_into().unwrap())
}

/// `freadInt`'s read when the caller tests the end of the file before it uses the number
/// (`TAGreadStart`): the four bytes into `slot`, the count the file gave; nothing noted.
pub fn fread_int_raw(fp: &mut CFile, slot: &mut Slot49) -> ([u8; 4], usize) {
    let (_, b, got) = fread_n(fp, [0u8; 4]);
    slot.put_int(&b, got);
    (b, got)
}

/// `freadLong(fp)`: a big-endian `long long`; `freadLong: fread failed` when the file does not
/// hold it (FS-167; FS-176: FreeSurfer rounds it through a `double`).
pub fn fread_long(g: &mut Globals, err: &mut dyn Write, fp: &mut CFile) -> i64 {
    let (ret, b, got) = fread_n(fp, [0u8; 8]);
    if got < 8 {
        memory(g, &FS_167, "freadLong", got, 8);
    }
    if ret != 1 {
        log::error_printf(g, err, ERROR_BADFILE, "freadLong: fread failed");
    }
    machine::swap_long64(g, i64::from_le_bytes(b))
}

/// `freadIntEx(&i, fp)`: reads into the caller's `int`, then reverses its bytes, also when the
/// file held fewer than 4 (the caller's own bytes stay where the file ended: at the end of the
/// file the caller's value comes back byte-reversed); the count read.
pub fn fread_int_ex(fp: &mut CFile, pi: &mut i32) -> i32 {
    let (ret, b, _) = fread_n(fp, pi.to_le_bytes());
    *pi = i32::from_be_bytes(b);
    ret
}

/// `freadFloat(fp)`: a big-endian `float`; `freadFloat: fread failed` when the file does not
/// hold it (FS-167).
pub fn fread_float(g: &mut Globals, err: &mut dyn Write, fp: &mut CFile) -> f32 {
    let (ret, b, got) = fread_n(fp, [0u8; 4]);
    if ret != 1 {
        log::error_printf(g, err, ERROR_BADFILE, "freadFloat: fread failed");
    }
    if got < 4 {
        memory(g, &FS_167, "freadFloat", got, 4);
    }
    f32::from_be_bytes(b)
}

/// `fwrite1(v, fp)`: the low byte of `v`; the count written.
pub fn fwrite1(fp: &mut CFile, v: i32) -> i32 {
    fp.fwrite(&[v as u8], 1, 1) as i32
}

/// `fwrite2(v, fp)`: `v` clamped to [-32767, 32767] as a big-endian `short`; the count written.
pub fn fwrite2(fp: &mut CFile, v: i32) -> i32 {
    let s = v.clamp(-0x7fff, 0x7fff) as i16;
    fp.fwrite(&s.to_be_bytes(), 2, 1) as i32
}

/// `fwrite3(v, fp)`: the low 24 bits of `v`, big-endian (a negative `v` too: -1 is `ff ff ff`).
pub fn fwrite3(fp: &mut CFile, v: i32) -> i32 {
    let b = [(v >> 16) as u8, (v >> 8) as u8, v as u8];
    fp.fwrite(&b, 3, 1) as i32
}

/// `fwrite4(v, fp)`: a big-endian `int` (as `fwriteInt`).
pub fn fwrite4(fp: &mut CFile, v: i32) -> i32 {
    fp.fwrite(&v.to_be_bytes(), 4, 1) as i32
}

/// `putf(f, fp)`: a big-endian `float` (its bits are only moved); the count written.
pub fn putf(fp: &mut CFile, f: f32) -> i32 {
    fp.fwrite(&f.to_be_bytes(), 4, 1) as i32
}

/// `fwriteShort(s, fp)`: a big-endian `short`.
pub fn fwrite_short(fp: &mut CFile, s: i16) -> i32 {
    fp.fwrite(&s.to_be_bytes(), 2, 1) as i32
}

/// `fwriteInt(v, fp)`: a big-endian `int`.
pub fn fwrite_int(fp: &mut CFile, v: i32) -> i32 {
    fp.fwrite(&v.to_be_bytes(), 4, 1) as i32
}

/// `fwriteLong(v, fp)`: a big-endian `long long` (FS-176).
pub fn fwrite_long(g: &mut Globals, fp: &mut CFile, v: i64) -> i32 {
    fp.fwrite(&machine::swap_long64(g, v).to_le_bytes(), 8, 1) as i32
}

/// `fwriteFloat(f, fp)`: a big-endian `float`.
pub fn fwrite_float(fp: &mut CFile, f: f32) -> i32 {
    fp.fwrite(&f.to_be_bytes(), 4, 1) as i32
}

/// `fwriteDouble(d, fp)`: a big-endian `double`.
pub fn fwrite_double(fp: &mut CFile, d: f64) -> i32 {
    fp.fwrite(&d.to_be_bytes(), 8, 1) as i32
}

/// `znzread(buf, size, nmemb, fp)` as FreeSurfer's own copy of znzlib (`packages/nifti/znzlib.c`,
/// older than nifticlib 2.0.0, which `larmorx_io::znz::ZnzFile::read` follows) defines it: a
/// compressed file is read with one `gzread` of `size * nmemb` bytes (an `int` product) and the
/// count is the bytes read over `size` (complete members; no warning; -1 for zlib's error); a
/// plain file with `fread`. A member size of 0 divides by zero in FreeSurfer (FS-76); 0 here.
pub fn fs_znzread(fp: &mut ZnzFile, buf: &mut [u8], size: usize, nmemb: usize) -> i64 {
    if let Some(gz) = fp.gz() {
        if size == 0 {
            return 0;
        }
        let len = (size as i32).wrapping_mul(nmemb as i32);
        if len < 0 {
            // gzread's unsigned length does not fit an int: zlib's stream error, kept
            return i64::from(gz.gzread_too_long());
        }
        let len = (len as usize).min(buf.len());
        let r = gz.gzread(&mut buf[..len]);
        return if r < 0 {
            -1
        } else {
            i64::from(r) / size as i64
        };
    }
    match fp.stdio() {
        Some(p) => {
            let total = size.saturating_mul(nmemb).min(buf.len());
            p.fread(&mut buf[..total], size, nmemb) as i64
        }
        None => 0,
    }
}

/// `znzwrite(buf, size, nmemb, fp)` as FreeSurfer's copy defines it: one `gzwrite` of
/// `size * nmemb` bytes, its count over `size`; `fwrite` for a plain file.
pub fn fs_znzwrite(fp: &mut ZnzFile, buf: &[u8], size: usize, nmemb: usize) -> i64 {
    let total = size.saturating_mul(nmemb).min(buf.len());
    if let Some(gz) = fp.gz() {
        if size == 0 {
            return 0;
        }
        let r = gz.gzwrite(&buf[..total]);
        return if r < 0 {
            -1
        } else {
            i64::from(r) / size as i64
        };
    }
    match fp.stdio() {
        Some(p) => p.fwrite(&buf[..total], size, nmemb) as i64,
        None => 0,
    }
}

/// `znzread(&x, N, 1, fp)` into memory holding `init`: the count returned (-1 for zlib's error),
/// the memory after it and how many bytes the file gave (from the file's position).
fn znzread_n<const N: usize>(fp: &mut ZnzFile, init: [u8; N]) -> (i32, [u8; N], usize) {
    let mut b = init;
    let before = fp.tell();
    let ret = fs_znzread(fp, &mut b, N, 1);
    let after = fp.tell();
    let got = if before >= 0 && after >= before {
        ((after - before) as usize).min(N)
    } else if ret >= 1 {
        N
    } else {
        0
    };
    (ret as i32, b, got)
}

/// The znz readers' message: only when the read failed before the end of the file and `Gdiag_no`
/// is set (an `.mgz`'s optional fields end at the end of the file, which is no error).
fn znz_failed(g: &mut Globals, err: &mut dyn Write, fp: &ZnzFile, ret: i32, what: &str) {
    if ret != 1 && !fp.eof() && g.gdiag_no >= 0 {
        log::error_printf(g, err, ERROR_BADFILE, what);
    }
}

/// `znzread1(&v, fp)`: one unsigned byte; the count read and the value.
pub fn znzread1(g: &mut Globals, fp: &mut ZnzFile) -> (i32, i32) {
    let (ret, b, got) = znzread_n(fp, [0u8; 1]);
    if got < 1 {
        memory(g, &FS_167, "znzread1", got, 1);
    }
    (ret, i32::from(b[0]))
}

/// `znzread2(&v, fp)`: a big-endian `short`, widened with its sign; the count read (-1 for
/// zlib's error) and the value (FS-167).
pub fn znzread2(g: &mut Globals, fp: &mut ZnzFile) -> (i32, i32) {
    let (ret, b, got) = znzread_n(fp, [0u8; 2]);
    if got < 2 {
        memory(g, &FS_167, "znzread2", got, 2);
    }
    (ret, i32::from(i16::from_be_bytes(b)))
}

/// `znzread3(&v, fp)`: a big-endian 3-byte unsigned number (a short read's missing bytes are 0:
/// FreeSurfer clears the number first).
pub fn znzread3(fp: &mut ZnzFile) -> (i32, i32) {
    let (ret, b, _) = znzread_n(fp, [0u8; 3]);
    let i = u32::from_be_bytes([b[0], b[1], b[2], 0]);
    (ret, ((i >> 8) & 0xff_ffff) as i32)
}

/// `znzread4(&v, fp)`: a big-endian `float` (FS-167).
pub fn znzread4(g: &mut Globals, fp: &mut ZnzFile) -> (i32, f32) {
    let (ret, b, got) = znzread_n(fp, [0u8; 4]);
    if got < 4 {
        memory(g, &FS_167, "znzread4", got, 4);
    }
    (ret, f32::from_be_bytes(b))
}

/// `znzreadDouble(fp)`: a big-endian `double` (`znzreadDouble: znzread failed`: see
/// [`znz_failed`]; FS-167).
pub fn znz_read_double(g: &mut Globals, err: &mut dyn Write, fp: &mut ZnzFile) -> f64 {
    let (ret, b, got) = znzread_n(fp, [0u8; 8]);
    if got < 8 {
        memory(g, &FS_167, "znzreadDouble", got, 8);
    }
    znz_failed(g, err, fp, ret, "znzreadDouble: znzread failed");
    f64::from_be_bytes(b)
}

/// `znzreadInt(fp)`: a big-endian `int` (`znzreadInt: znzread failed`; FS-167).
pub fn znz_read_int(g: &mut Globals, err: &mut dyn Write, fp: &mut ZnzFile) -> i32 {
    let (ret, b, got) = znzread_n(fp, [0u8; 4]);
    if got < 4 {
        memory(g, &FS_167, "znzreadInt", got, 4);
    }
    znz_failed(g, err, fp, ret, "znzreadInt: znzread failed");
    i32::from_be_bytes(b)
}

/// `znzreadLong(fp)`: a big-endian `long long` (`znzreadLong: znzread failed`; FS-167).
pub fn znz_read_long(g: &mut Globals, err: &mut dyn Write, fp: &mut ZnzFile) -> i64 {
    let (ret, b, got) = znzread_n(fp, [0u8; 8]);
    if got < 8 {
        memory(g, &FS_167, "znzreadLong", got, 8);
    }
    znz_failed(g, err, fp, ret, "znzreadLong: znzread failed");
    machine::swap_long64(g, i64::from_le_bytes(b))
}

/// `znzreadShort(fp)`: a big-endian `short` (`znzreadShort: znzread failed`; FS-167).
pub fn znz_read_short(g: &mut Globals, err: &mut dyn Write, fp: &mut ZnzFile) -> i16 {
    let (ret, b, got) = znzread_n(fp, [0u8; 2]);
    if got < 2 {
        memory(g, &FS_167, "znzreadShort", got, 2);
    }
    znz_failed(g, err, fp, ret, "znzreadShort: znzread failed");
    i16::from_be_bytes(b)
}

/// `znzreadFloatEx(&f, fp)`: reads into the caller's `float`, then reverses its bytes (as
/// [`fread_int_ex`]); the count read.
pub fn znz_read_float_ex(fp: &mut ZnzFile, pf: &mut f32) -> i32 {
    let (ret, b, _) = znzread_n(fp, pf.to_le_bytes());
    *pf = f32::from_be_bytes(b);
    ret
}

/// `znzreadDoubleEx(&d, fp)`: as [`znz_read_float_ex`] for a `double`.
pub fn znz_read_double_ex(fp: &mut ZnzFile, pd: &mut f64) -> i32 {
    let (ret, b, _) = znzread_n(fp, pd.to_le_bytes());
    *pd = f64::from_be_bytes(b);
    ret
}

/// `znzreadShortEx(&s, fp)`: as [`znz_read_float_ex`] for a `short`.
pub fn znz_read_short_ex(fp: &mut ZnzFile, ps: &mut i16) -> i32 {
    let (ret, b, _) = znzread_n(fp, ps.to_le_bytes());
    *ps = i16::from_be_bytes(b);
    ret
}

/// `znzreadIntEx(&i, fp)`: as [`znz_read_float_ex`] for an `int`.
pub fn znz_read_int_ex(fp: &mut ZnzFile, pi: &mut i32) -> i32 {
    let (ret, b, _) = znzread_n(fp, pi.to_le_bytes());
    *pi = i32::from_be_bytes(b);
    ret
}

/// `znzreadFloat(fp)`: a big-endian `float`; `freadFloat: fread failed` whenever the read fails,
/// at the end of the file too (FS-167).
pub fn znz_read_float(g: &mut Globals, err: &mut dyn Write, fp: &mut ZnzFile) -> f32 {
    let (ret, b, got) = znzread_n(fp, [0u8; 4]);
    if ret != 1 {
        log::error_printf(g, err, ERROR_BADFILE, "freadFloat: fread failed");
    }
    if got < 4 {
        memory(g, &FS_167, "znzreadFloat", got, 4);
    }
    f32::from_be_bytes(b)
}

fn znzwrite_bytes(fp: &mut ZnzFile, b: &[u8]) -> i32 {
    fs_znzwrite(fp, b, b.len(), 1) as i32
}

/// `znzwrite1(v, fp)`: the low byte of `v`; the count written.
pub fn znzwrite1(fp: &mut ZnzFile, v: i32) -> i32 {
    znzwrite_bytes(fp, &[v as u8])
}

/// `znzwrite2(v, fp)`: `v` clamped to [-32767, 32767] as a big-endian `short`.
pub fn znzwrite2(fp: &mut ZnzFile, v: i32) -> i32 {
    znzwrite_bytes(fp, &(v.clamp(-0x7fff, 0x7fff) as i16).to_be_bytes())
}

/// `znzwrite3(v, fp)`: the low 24 bits of `v`, big-endian.
pub fn znzwrite3(fp: &mut ZnzFile, v: i32) -> i32 {
    znzwrite_bytes(fp, &[(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// `znzwrite4(v, fp)`: a big-endian `int`.
pub fn znzwrite4(fp: &mut ZnzFile, v: i32) -> i32 {
    znzwrite_bytes(fp, &v.to_be_bytes())
}

/// `znzwriteShort(s, fp)`: a big-endian `short`; the count written.
pub fn znz_write_short(fp: &mut ZnzFile, s: i16) -> i32 {
    znzwrite_bytes(fp, &s.to_be_bytes())
}

/// `znzwriteUShort(s, fp)`.
pub fn znz_write_ushort(fp: &mut ZnzFile, s: u16) -> i32 {
    znzwrite_bytes(fp, &s.to_be_bytes())
}

/// `znzwriteInt(v, fp)`.
pub fn znz_write_int(fp: &mut ZnzFile, v: i32) -> i32 {
    znzwrite_bytes(fp, &v.to_be_bytes())
}

/// `znzwriteLong(v, fp)` (FS-176).
pub fn znz_write_long(g: &mut Globals, fp: &mut ZnzFile, v: i64) -> i32 {
    znzwrite_bytes(fp, &machine::swap_long64(g, v).to_le_bytes())
}

/// `znzwriteFloat(f, fp)`.
pub fn znz_write_float(fp: &mut ZnzFile, f: f32) -> i32 {
    znzwrite_bytes(fp, &f.to_be_bytes())
}

/// `znzwriteDouble(d, fp)`.
pub fn znz_write_double(fp: &mut ZnzFile, d: f64) -> i32 {
    znzwrite_bytes(fp, &d.to_be_bytes())
}

/// The bytes of a C string: up to its first NUL.
fn c_bytes(s: &str) -> &[u8] {
    let b = s.as_bytes();
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// `fio_dirname(pathname)`: the directory part as FreeSurfer computes it: trailing slashes
/// removed, then everything before the last slash, kept as it is (`a//b` gives `a/`); `.` when
/// there is no slash, `/` when the slash is the first character, and also for an empty name or
/// one of slashes only; `None` for `None`. FS-168: a one-character name gives `.` (FreeSurfer's
/// result depends on memory it has freed).
pub fn fio_dirname(g: &mut Globals, pathname: Option<&str>) -> Option<String> {
    let mut p = c_bytes(pathname?);
    while let [rest @ .., b'/'] = p {
        p = rest;
    }
    let l = p.len();
    if l < 2 {
        if l == 0 {
            return Some("/".into());
        }
        g.notices.add(
            &FS_168,
            "fio_dirname of a one-character name: '.' (FreeSurfer reads the name after freeing it: '.' or '/')",
        );
        return Some(".".into());
    }
    match p.iter().rposition(|&c| c == b'/') {
        None => Some(".".into()),
        Some(0) => Some("/".into()),
        Some(n) => Some(text(&p[..n])),
    }
}

/// `fio_basename(pathname, ext)`: `ext` removed from the end when it matches and is shorter than
/// the name, then trailing slashes, then everything after the last slash; `/` for an empty
/// result, the character itself for a one-character one; `None` for `None`.
pub fn fio_basename(pathname: Option<&str>, ext: Option<&str>) -> Option<String> {
    let mut p = c_bytes(pathname?);
    if let Some(e) = ext {
        let e = c_bytes(e);
        if e.len() < p.len() && p.ends_with(e) {
            p = &p[..p.len() - e.len()];
        }
    }
    while let [rest @ .., b'/'] = p {
        p = rest;
    }
    if p.is_empty() {
        return Some("/".into());
    }
    if p.len() < 2 {
        return Some(text(&p[..1]));
    }
    match p.iter().rposition(|&c| c == b'/') {
        None => Some(text(p)),
        Some(n) => Some(text(&p[n + 1..])),
    }
}

/// `fio_extension(pathname)`: what follows the last `.` (slashes included: `a.b/c` gives `b/c`);
/// `None` without a `.` or with the `.` last, or for `None`.
pub fn fio_extension(pathname: Option<&str>) -> Option<String> {
    let p = c_bytes(pathname?);
    let n = p.iter().rposition(|&c| c == b'.')?;
    if n + 1 == p.len() {
        return None;
    }
    Some(text(&p[n + 1..]))
}

/// The bytes past which `fio_DirIsWritable`'s name no longer fits its buffer.
const DIRW_BUFFER: usize = 2000;

/// `fio_DirIsWritable(dirname, fname)`: 1 when `<dirname>/.junk.54_-_sdfklj` (or, when `fname`
/// is not 0, `<dirname>.junk.54_-_sdfklj`) can be created for writing (then removed: a file of
/// that name already there is truncated and removed too), else 0. FS-169 for names that do not fit
/// FreeSurfer's buffer (both modes: the whole name).
pub fn fio_dir_is_writable(g: &mut Globals, dir: Option<&Path>, dirname: &str, fname: i32) -> i32 {
    let name = if fname != 0 {
        format!("{dirname}.junk.54_-_sdfklj")
    } else {
        format!("{dirname}/.junk.54_-_sdfklj")
    };
    if name.len() >= DIRW_BUFFER {
        g.notices.add(
            &FS_169,
            format!(
                "fio_DirIsWritable: a name of {} bytes, past FreeSurfer's {DIRW_BUFFER}-byte buffer, used whole",
                name.len()
            ),
        );
    }
    match CFile::open(dir, &name, "w") {
        Ok(f) => {
            f.close();
            let _ = std::fs::remove_file(utils::resolve(dir, &name));
            1
        }
        Err(e) => {
            g.errno = e;
            0
        }
    }
}

/// `fio_FileExistsReadable(fname)`: 1 when `fopen(fname, "r")` succeeds (a directory does), else
/// 0 (`errno` set).
pub fn fio_file_exists_readable(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    match CFile::open(dir, fname, "r") {
        Ok(f) => {
            f.close();
            1
        }
        Err(e) => {
            g.errno = e;
            0
        }
    }
}

/// `fio_IsDirectory(fname)`: 1 when it opens for reading and is a directory, else 0.
pub fn fio_is_directory(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    if fio_file_exists_readable(g, dir, fname) == 0 {
        return 0;
    }
    match std::fs::metadata(utils::resolve(dir, fname)) {
        Ok(m) => i32::from(m.is_dir()),
        Err(e) => {
            g.errno = log::errno_of(&e).unwrap_or(5);
            0
        }
    }
}

/// The C program's `printf` of bytes.
fn put(out: &mut dyn Write, b: &[u8]) {
    let _ = out.write_all(b);
}

/// `TSV::read(fname)`: the column names from the first line (`fgets` of at most 4,999 bytes, split
/// at tabs, empty fields skipped, each without one trailing newline and then one carriage
/// return), then numbers (`fscanf("%lf")`, across line ends) in rows of as many as there are
/// names, until the end of the file. 0, or 1 after `ERROR: TSV.read(): cannot open <f> for
/// reading`, `ERROR: TSV.read(): no data found in <f>` or `ERROR: TSV.read(): <f> truncated at
/// row=<r>, col=<c>` (on standard output; the rows read so far are kept). With `debug` set, the
/// name, each column name and each row's numbers (`%g`) are printed. FS-170 (an empty file),
/// FS-171 (an empty first line), FS-172 (an entry that is not a number: by default the error
/// `ERROR: TSV.read(): <f> has an entry that is not a number at row=<r>, col=<c>`).
pub fn tsv_read(
    g: &mut Globals,
    out: &mut dyn Write,
    dir: Option<&Path>,
    t: &mut Tsv,
    fname: &str,
) -> i32 {
    t.headers.clear();
    t.data.clear();
    let fb = fname.as_bytes();
    if t.debug != 0 {
        put(out, &[b"fname = ".as_slice(), fb, b"\n"].concat());
    }
    let mut fp = match CFile::open(dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            put(
                out,
                &[
                    b"ERROR: TSV.read(): cannot open ".as_slice(),
                    fb,
                    b" for reading\n",
                ]
                .concat(),
            );
            return 1;
        }
    };
    let line = match fp.fgets(5000) {
        Some(l) => l,
        None => {
            g.notices.add(
                &FS_170,
                "TSV::read of an empty file: no column names (FreeSurfer reads them from memory contents)",
            );
            Vec::new()
        }
    };
    let line = &line[..line.iter().position(|&c| c == 0).unwrap_or(line.len())];
    let mut first = true;
    let mut start = 0;
    for field in line.split(|&c| c == b'\t') {
        let at = start;
        start += field.len() + 1;
        if field.is_empty() {
            continue;
        }
        let mut tok = field;
        if let [rest @ .., b'\n'] = tok {
            tok = rest;
            if tok.is_empty() && first && at == 0 {
                g.notices.add(
                    &FS_171,
                    "TSV::read: an empty first line (FreeSurfer reads the byte before its line buffer)",
                );
            }
        }
        if let [rest @ .., b'\r'] = tok {
            tok = rest;
        }
        first = false;
        t.headers.push(tok.to_vec());
        if t.debug != 0 {
            put(
                out,
                &[
                    format!("{} header ", t.ncols()).as_bytes(),
                    tok,
                    b"\n".as_slice(),
                ]
                .concat(),
            );
        }
    }
    if t.headers.is_empty() {
        put(
            out,
            &[
                b"ERROR: TSV.read(): no data found in ".as_slice(),
                fb,
                b"\n",
            ]
            .concat(),
        );
        return 1;
    }
    let ncols = t.headers.len();
    // `val` keeps the last number read; before the first it is memory contents (None).
    let mut val: Option<f64> = None;
    let mut nrows = 0;
    while !fp.feof() {
        let mut row = Vec::with_capacity(ncols);
        if t.debug != 0 {
            put(out, format!("{} ", nrows + 1).as_bytes());
        }
        let mut done = false;
        for m in 0..ncols {
            let before = fp.taken();
            match cscan::scan_f64(&mut fp) {
                Scan::Value(v, erange) => {
                    if erange {
                        g.errno = cscan::ERANGE;
                    }
                    val = Some(v);
                }
                Scan::Eof => {
                    g.errno = 0;
                    if m == 0 {
                        done = true;
                        break;
                    }
                    put(
                        out,
                        &[
                            b"ERROR: TSV.read(): ".as_slice(),
                            fb,
                            format!(" truncated at row={}, col={m}\n", nrows + 1).as_bytes(),
                        ]
                        .concat(),
                    );
                    return 1;
                }
                Scan::Fail => {
                    let stuck = fp.taken() == before;
                    if g.notices.fix() || stuck {
                        g.notices.add(
                            &FS_172,
                            if stuck {
                                "TSV::read: an entry that is not a number, where FreeSurfer reads rows for ever: an error"
                            } else {
                                "TSV::read: an entry that is not a number: an error"
                            },
                        );
                        put(
                            out,
                            &[
                                b"ERROR: TSV.read(): ".as_slice(),
                                fb,
                                format!(
                                    " has an entry that is not a number at row={}, col={m}\n",
                                    nrows + 1
                                )
                                .as_bytes(),
                            ]
                            .concat(),
                        );
                        return 1;
                    }
                    if val.is_none() {
                        g.notices.add(
                            &FS_172,
                            "TSV::read: an entry that is not a number before any number: 0 (FreeSurfer's is memory contents)",
                        );
                    } else {
                        g.notices.add(
                            &FS_172,
                            "TSV::read: an entry that is not a number: the previous number in its place",
                        );
                    }
                }
            }
            let v = val.unwrap_or(0.0);
            if t.debug != 0 {
                put(out, format!("{} ", larmorx_core::cfmt::g(v, 6)).as_bytes());
            }
            row.push(v);
        }
        if t.debug != 0 {
            put(out, b"\n");
        }
        if done {
            break;
        }
        t.data.push(row);
        nrows += 1;
    }
    if t.debug != 0 {
        put(
            out,
            format!("rows = {} cols = {}\n", t.data.len(), t.headers.len()).as_bytes(),
        );
    }
    fp.close();
    0
}

/// Whether a conversion of `fmt` has C's `L` length (a `long double`, which `TSV::print` never
/// passes).
fn has_long_double(fmt: &str) -> bool {
    let b = fmt.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'%' {
            i += 1;
            continue;
        }
        i += 1;
        if b.get(i) == Some(&b'%') {
            i += 1;
            continue;
        }
        while i < b.len() && (b"-+ #0.".contains(&b[i]) || b[i].is_ascii_digit()) {
            i += 1;
        }
        if b.get(i) == Some(&b'L') {
            return true;
        }
    }
    false
}

/// The longest format `TSV::print` keeps in its 1,000-byte buffer with the separator after it.
const FMT_MAX: usize = 998;

/// `TSV::print(fp, fmt)`: the column names separated by tabs and ended by a newline, then each
/// row's numbers printed with `fmt` (one `double` conversion, `%f`, `%e` or `%g` with flags,
/// width and precision, or none) separated likewise; 0. FS-173: a format that would print another
/// type, more than one number or past FreeSurfer's buffer is refused (both modes) with
/// `ERROR: TSV.print(): the format <fmt> does not print one number` on standard output, 1 (`%a`,
/// which C defines for a `double`, is refused as well: larmorx does not print it). A row prints
/// as many numbers as there are names (FreeSurfer reads past a shorter row; here it ends).
pub fn tsv_print(
    g: &mut Globals,
    out: &mut dyn Write,
    fp: &mut dyn Write,
    t: &Tsv,
    fmt: &str,
) -> i32 {
    let nc = t.headers.len();
    let refuse = |g: &mut Globals, out: &mut dyn Write, why: &str| {
        g.notices
            .add(&FS_173, format!("TSV::print: {why}: refused"));
        put(
            out,
            &[
                b"ERROR: TSV.print(): the format ".as_slice(),
                fmt.as_bytes(),
                b" does not print one number\n",
            ]
            .concat(),
        );
        1
    };
    let fmt_ok = fmt.len() <= FMT_MAX
        && !has_long_double(fmt)
        && cstd::format_double(&format!("{fmt}\t"), 0.0).is_some();
    if t.nrows() > 0 && nc > 0 && !fmt_ok {
        let why = if fmt.len() > FMT_MAX {
            "a format longer than FreeSurfer's buffer"
        } else {
            "a format that does not print one double"
        };
        return refuse(g, out, why);
    }
    for (c, h) in t.headers.iter().enumerate() {
        let _ = fp.write_all(h);
        let _ = fp.write_all(if c + 1 != nc { b"\t" } else { b"\n" });
    }
    for r in &t.data {
        for (c, &v) in r.iter().enumerate().take(nc) {
            let sep = if c + 1 != nc { "\t" } else { "\n" };
            let s = cstd::format_double(&format!("{fmt}{sep}"), v).expect("checked above");
            let _ = fp.write_all(s.as_bytes());
        }
    }
    0
}

/// `TSV::write(fname, fmt)`: [`tsv_print`] into a new file; 1 after `ERROR: TSV.read(): cannot
/// open <f> for writing` (FreeSurfer's words) when it cannot be created.
pub fn tsv_write(
    g: &mut Globals,
    out: &mut dyn Write,
    dir: Option<&Path>,
    t: &Tsv,
    fname: &str,
    fmt: &str,
) -> i32 {
    let mut fp = match CFile::open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            put(
                out,
                &[
                    b"ERROR: TSV.read(): cannot open ".as_slice(),
                    fname.as_bytes(),
                    b" for writing\n",
                ]
                .concat(),
            );
            return 1;
        }
    };
    let r = tsv_print(g, out, &mut fp, t, fmt);
    fp.close();
    if r != 0 {
        return r;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn g(mode: UpstreamDefects) -> Globals {
        Globals::new(mode)
    }

    #[test]
    fn readers_and_short_reads() {
        let mut gl = g(UpstreamDefects::Fix);
        let mut e = Vec::new();
        let mut f = CFile::from_bytes(vec![0xff, 0xfe, 1, 2, 3, 0x80]);
        assert_eq!(fread2(&mut gl, &mut f), (1, -2));
        assert_eq!(fread3(&mut f), (1, 0x01_0203));
        assert_eq!(
            fread2(&mut gl, &mut f),
            (0, i32::from(i16::from_be_bytes([0x80, 0])))
        );
        assert!(gl.notices.contains(&FS_167));
        let mut f = CFile::from_bytes(vec![0x3f, 0x80]);
        assert_eq!(fread_float(&mut gl, &mut e, &mut f).to_bits(), 0x3f80_0000);
        assert!(!e.is_empty());
        let mut f = CFile::from_bytes(vec![0, 0, 0, 5]);
        let mut i = 0x1122_3344;
        assert_eq!(fread_int_ex(&mut f, &mut i), 1);
        assert_eq!(i, 5);
        // at the end: the caller's value byte-reversed
        assert_eq!(fread_int_ex(&mut f, &mut i), 0);
        assert_eq!(i, 0x0500_0000);
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// FS-49 at fs.io's callers: the numbers FreeSurfer's official build gives where the file cuts
    /// a number read right after another (observed with the fs-io probe,
    /// `validation/probes/fs-io/fs49_slot_cases.py`); the default reads the missing bytes as 0.
    #[test]
    fn fs49_slot_keeps_the_earlier_numbers_bytes() {
        use crate::colortab_bin::ctab_read_from_binary;
        use crate::tags::tag_read_start;
        for (mode, want) in [
            (
                UpstreamDefects::Reproduce,
                [29i64, -1_412_628_451, 301_989_917],
            ),
            (
                UpstreamDefects::Fix,
                [-1, 0xabcd_0000u32 as i32 as i64 - 1, 0x1200_0000 - 1],
            ),
        ] {
            for (hex, w) in ["0000001e", "0000001eabcd", "0000001e12"].iter().zip(want) {
                let mut gl = g(mode);
                let mut e = Vec::new();
                let mut f = CFile::from_bytes(unhex(hex));
                let mut len = -7;
                assert_eq!(tag_read_start(&mut gl, &mut e, &mut f, &mut len), 30);
                assert_eq!(len, w, "{hex} {mode:?}");
                assert!(gl.notices.contains(&FS_49));
            }
        }
        // the slot directly: freadDouble's value stored back reversed, freadInt over bytes 4 to 7
        let mut gl = g(UpstreamDefects::Reproduce);
        let mut e = Vec::new();
        let mut s = Slot49::new();
        let mut f = CFile::from_bytes(unhex("0102030405060708aa"));
        assert_eq!(
            fread_double_in(&mut gl, &mut e, &mut f, &mut s).to_bits(),
            0x0102_0304_0506_0708
        );
        assert_eq!(fread_int_in(&mut gl, &mut f, &mut s), 0xaa03_0201u32 as i32);
        assert!(!s.took_memory());
        s.clobber();
        assert_eq!(fread_int_in(&mut gl, &mut f, &mut s), 0);
        assert!(s.took_memory());
        // colour tables on C streams: (file, entry 1's ri gi bi ai) or the error
        let colour = |hex: &str, mode| {
            let mut gl = g(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let mut f = CFile::from_bytes(unhex(hex));
            let ct = ctab_read_from_binary(&mut gl, &mut o, &mut e, Some(&mut f));
            let rgba = ct.and_then(|c| {
                c.entries
                    .iter()
                    .flatten()
                    .last()
                    .map(|x| (x.ri, x.gi, x.bi, x.ai))
            });
            (rgba, String::from_utf8_lossy(&e).into_owned())
        };
        let r = UpstreamDefects::Reproduce;
        assert_eq!(
            colour("00000001000000010000000002780000000009", r).0,
            Some((9, 9, 9, 246))
        );
        assert_eq!(
            colour("000000010000000100000000027800000000090a0b", r).0,
            Some((9, 168_493_065, 168_493_065, -168_492_810))
        );
        let v2 = "fffffffe0000000300000001000000000100000001000000026200000102030400";
        assert_eq!(
            colour(v2, r).0,
            Some((66051, 67_109_379, 67_109_379, -67_109_124))
        );
        assert_eq!(
            colour(&format!("{v2}aabb"), r).0,
            Some((66051, 67_152_571, 67_152_571, -67_152_316))
        );
        let (none, msg) = colour(
            "fffffffe000000030000000100000000020000000100000002620000000005000000060000000700000001",
            r,
        );
        assert!(
            none.is_none() && msg.contains("Duplicate structure 1"),
            "{msg}"
        );
        assert!(colour("fffffffe", r).1.contains("nentries was -2"));
        // the default: the end of the file inside the table is an error (FS-192)
        let (none, msg) = colour(v2, UpstreamDefects::Fix);
        assert!(
            none.is_none() && msg.contains("ends inside the colour table"),
            "{msg}"
        );
    }

    #[test]
    fn writers() {
        let mut f = CFile::memory_writer();
        assert_eq!(fwrite2(&mut f, 40000), 1);
        assert_eq!(fwrite2(&mut f, -40000), 1);
        assert_eq!(fwrite3(&mut f, -1), 1);
        assert_eq!(fwrite_int(&mut f, 0x0102_0304), 1);
        assert_eq!(fwrite_float(&mut f, 1.0), 1);
        assert_eq!(
            f.into_bytes(),
            vec![
                0x7f, 0xff, 0x80, 0x01, 0xff, 0xff, 0xff, 1, 2, 3, 4, 0x3f, 0x80, 0, 0
            ]
        );
    }

    #[test]
    fn surface_readers_and_writers() {
        let mut gl = g(UpstreamDefects::Fix);
        let mut e = Vec::new();
        // a signalling NaN's bits come through unchanged (the bytes are only moved)
        let mut f = CFile::from_bytes(vec![
            0x7f, 0x80, 0, 1, 0x40, 0x49, 0x0f, 0xdb, 0x12, 0x34, 0xab,
        ]);
        let (r, v) = fread4(&mut gl, &mut f);
        assert_eq!((r, v.to_bits()), (1, 0x7f80_0001));
        assert_eq!(getf(&mut gl, &mut e, &mut f).to_bits(), 0x4049_0fdb);
        assert_eq!(fread_short(&mut gl, &mut e, &mut f), 0x1234);
        assert!(e.is_empty() && gl.notices.as_slice().is_empty());
        let (r, v) = fread4(&mut gl, &mut f);
        assert_eq!((r, v.to_bits()), (0, 0xab00_0000));
        assert!(gl.notices.contains(&FS_167));
        let mut f = CFile::from_bytes(vec![0x01]);
        assert_eq!(fread_short(&mut gl, &mut e, &mut f), 0x0100);
        assert!(String::from_utf8_lossy(&e).contains("freadShort: fread failed"));
        // the Ex readers keep the caller's bytes where the file ended
        let mut f = CFile::from_bytes(vec![0xaa]);
        let mut s: i16 = 0x1122;
        assert_eq!(fread_short_ex(&mut f, &mut s), 0);
        assert_eq!(s as u16, 0xaa11);
        let mut f = CFile::from_bytes(vec![0x3f, 0xf0, 0, 0, 0, 0, 0, 0]);
        let mut d = 0.0f64;
        assert_eq!(fread_double_ex(&mut f, &mut d), 1);
        assert_eq!(d, 1.0);
        let mut w = CFile::memory_writer();
        assert_eq!(fwrite1(&mut w, 0x1ff), 1);
        assert_eq!(fwrite4(&mut w, -2), 1);
        assert_eq!(putf(&mut w, f32::from_bits(0x7f80_0001)), 1);
        assert_eq!(fwrite_short(&mut w, -2), 1);
        assert_eq!(fwrite_double(&mut w, -2.0), 1);
        assert_eq!(
            w.into_bytes(),
            vec![
                0xff, 0xff, 0xff, 0xff, 0xfe, 0x7f, 0x80, 0, 1, 0xff, 0xfe, 0xc0, 0, 0, 0, 0, 0, 0,
                0
            ]
        );
    }

    #[test]
    fn names() {
        let mut gl = g(UpstreamDefects::Fix);
        let d = |gl: &mut Globals, s: &str| fio_dirname(gl, Some(s)).unwrap();
        assert_eq!(d(&mut gl, "a/b/c"), "a/b");
        assert_eq!(d(&mut gl, "a//b"), "a/");
        assert_eq!(d(&mut gl, "/a"), "/");
        assert_eq!(d(&mut gl, "ab"), ".");
        assert_eq!(d(&mut gl, ""), "/");
        assert_eq!(d(&mut gl, "///"), "/");
        assert!(!gl.notices.contains(&FS_168));
        assert_eq!(d(&mut gl, "a/"), ".");
        assert!(gl.notices.contains(&FS_168));
        let b = |s: &str, e: Option<&str>| fio_basename(Some(s), e).unwrap();
        assert_eq!(b("a/b.mgz", Some(".mgz")), "b");
        assert_eq!(b(".mgz", Some(".mgz")), ".mgz");
        assert_eq!(b("dir/", None), "dir");
        assert_eq!(b("", None), "/");
        assert_eq!(b("x/", None), "x");
        assert_eq!(fio_extension(Some("a.b/c")).as_deref(), Some("b/c"));
        assert_eq!(fio_extension(Some("a.")), None);
        assert_eq!(fio_extension(Some("abc")), None);
        assert_eq!(fio_dirname(&mut gl, None), None);
    }

    #[test]
    fn tsv_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("t.tsv"), b"a\tb\r\n1\t2.5\n3 4\n").unwrap();
        let mut gl = g(UpstreamDefects::Fix);
        let mut out = Vec::new();
        let mut t = Tsv::default();
        assert_eq!(
            tsv_read(&mut gl, &mut out, Some(dir.path()), &mut t, "t.tsv"),
            0
        );
        assert_eq!(t.headers, vec![b"a".to_vec(), b"b".to_vec()]);
        assert_eq!(t.data, vec![vec![1.0, 2.5], vec![3.0, 4.0]]);
        assert_eq!(
            tsv_write(&mut gl, &mut out, Some(dir.path()), &t, "o.tsv", "%g"),
            0
        );
        assert_eq!(
            std::fs::read(dir.path().join("o.tsv")).unwrap(),
            b"a\tb\n1\t2.5\n3\t4\n".to_vec()
        );
        assert!(out.is_empty());
        // FS-172: by default an error; reproduced, the previous number where the reading moves on
        std::fs::write(dir.path().join("x.tsv"), b"a\n1\n-x\n").unwrap();
        assert_eq!(
            tsv_read(&mut gl, &mut out, Some(dir.path()), &mut t, "x.tsv"),
            1
        );
        assert!(gl.notices.contains(&FS_172));
        let mut gr = g(UpstreamDefects::Reproduce);
        out.clear();
        std::fs::write(dir.path().join("y.tsv"), b"a\n1\n-.-.5\n").unwrap();
        assert_eq!(
            tsv_read(&mut gr, &mut out, Some(dir.path()), &mut t, "y.tsv"),
            0
        );
        assert_eq!(t.data, vec![vec![1.0], vec![1.0], vec![-0.5]]);
        // ...and an error where FreeSurfer never returns
        assert_eq!(
            tsv_read(&mut gr, &mut out, Some(dir.path()), &mut t, "x.tsv"),
            1
        );
    }
}
