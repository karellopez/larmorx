// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The binary colour tables of `utils/colortab.cpp` (queue item 3.1-33's binary functions, taken
//! with 3.1-25's second piece, the orchestrator's decision of 2026-11-13): `CTABreadFromBinary`,
//! `CTABwriteIntoBinary` and their two versions on C streams, `znzCTABreadFromBinary`,
//! `znzCTABwriteIntoBinary` and theirs on znzlib files (an `.mgz`'s `TAG_OLD_COLORTABLE`, a surface
//! annotation's table), on [`larmorx_freesurfer_mri::ctab`]'s structure.
//!
//! Version 1 starts with the entry count (positive) and stores every entry in order; version 2
//! starts with -2 and stores the non-empty entries with their structure numbers. Names are stored
//! as a length (the C string's length plus one) and the bytes with their NUL. FS-187 to FS-192.

use std::io::Write;

use larmorx_core::defects::{FS_187, FS_188, FS_189, FS_190, FS_191, FS_192};
use larmorx_freesurfer_core::diag::{DIAG_INFO, DIAG_SHOW};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NO_MEMORY, NO_ERROR};
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};
use larmorx_io::znz::ZnzFile;

use crate::cfile::CFile;
use crate::fio;

/// The largest entry count larmorx reads (FS-191: FreeSurfer allocates 8 bytes per entry, and in
/// the first version a 1,064-byte entry for each, before reading; beyond what the machine can
/// allocate its outcome depends on the machine). No FreeSurfer table comes near: the largest
/// structure number of `FreeSurferColorLUT.txt` is 14,175.
pub const MAX_ENTRIES: i32 = 1 << 24;

/// A name length above which FreeSurfer's allocation of the length plus one, made before the
/// read, may fail on a machine with little memory (FS-191's note where the file holds fewer).
const NOTE_LENGTH: i64 = 1 << 24;

/// `STRLEN - 1`: the longest name a table keeps.
const NAME_MAX: usize = 1023;

/// The stream a reader reads from.
pub enum In<'a> {
    File(&'a mut CFile),
    Znz(&'a mut ZnzFile),
}

impl In<'_> {
    fn int(&mut self, g: &mut Globals, err: &mut dyn Write) -> i32 {
        match self {
            In::File(fp) => fio::fread_int(g, fp),
            In::Znz(fp) => fio::znz_read_int(g, err, fp),
        }
    }

    /// `fread(buf, 1, n, fp)` / `znzread(buf, 1, n, fp)` of `n` bytes, keeping the first
    /// `buf.len()` and dropping the rest: the bytes the file gave.
    fn bytes(&mut self, buf: &mut [u8], n: u64) -> u64 {
        let mut got = 0u64;
        let keep = (buf.len() as u64).min(n) as usize;
        let mut chunk = vec![0u8; 1 << 16];
        let r = self.read(&mut buf[..keep]);
        got += r as u64;
        if r < keep {
            return got;
        }
        while got < n {
            let k = ((n - got).min(chunk.len() as u64)) as usize;
            let r = self.read(&mut chunk[..k]);
            got += r as u64;
            if r < k {
                break;
            }
        }
        got
    }

    fn read(&mut self, b: &mut [u8]) -> usize {
        if b.is_empty() {
            return 0;
        }
        match self {
            In::File(fp) => fp.fread(b, 1, b.len()),
            In::Znz(fp) => {
                let before = fp.tell();
                let r = fio::fs_znzread(fp, b, 1, b.len());
                let after = fp.tell();
                if before >= 0 && after >= before {
                    ((after - before) as usize).min(b.len())
                } else {
                    r.clamp(0, b.len() as i64) as usize
                }
            }
        }
    }

    fn eof(&self) -> bool {
        match self {
            In::File(fp) => fp.feof(),
            In::Znz(fp) => fp.eof(),
        }
    }

    fn znz(&self) -> bool {
        matches!(self, In::Znz(_))
    }
}

/// How a reader copies a name into the table (see [`take_name`]).
#[derive(Clone, Copy, PartialEq)]
enum Copy {
    /// `strncpy(ct->fname, tmp, STRLEN - 1)` into the zeroed table.
    StrncpyTable,
    /// `strncpy(entry->name, tmp, STRLEN - 1); entry->name[len] = 0` (the first version's).
    StrncpyEntryCut,
    /// `strncpy(entry->name, tmp, STRLEN - 1)` (the second version on C streams).
    StrncpyEntry,
    /// `znzread(ct->fname, 1, min(len, STRLEN - 1), fp)` into the zeroed table.
    DirectTable,
    /// `znzread(entry->name, 1, min(len, STRLEN - 1), fp)` into the new entry.
    DirectEntry,
}

/// The name a reader keeps from `len` bytes of which the file gave `got` (`bytes` the first ones):
/// C's copy up to a NUL byte, at most 1,023 bytes. FS-187: where FreeSurfer's copy reaches bytes
/// it never wrote (the file's bytes end without a NUL, or 1,023 bytes are copied into an entry
/// whose last byte was never written), the name ends there.
fn take_name(g: &mut Globals, who: &str, bytes: &[u8], got: u64, len: i64, how: Copy) -> Vec<u8> {
    let known = (got as usize).min(bytes.len());
    let limit = match how {
        Copy::DirectTable | Copy::DirectEntry => known.min(NAME_MAX),
        _ => NAME_MAX,
    };
    let mut end = None;
    let mut memory = false;
    for i in 0..limit {
        if i >= known {
            memory = true;
            end = Some(i);
            break;
        }
        if bytes[i] == 0 {
            end = Some(i);
            break;
        }
        if how == Copy::StrncpyEntryCut && (i as i64) == len {
            end = Some(i);
            break;
        }
    }
    let end = match end {
        Some(e) => e,
        None => {
            // `limit` bytes copied without a NUL
            match how {
                Copy::StrncpyTable | Copy::DirectTable => limit,
                Copy::StrncpyEntryCut if len == NAME_MAX as i64 => limit,
                _ => {
                    memory = true;
                    limit
                }
            }
        }
    };
    if memory {
        g.notices.add(
            &FS_187,
            format!(
                "{who}: a name of {len} bytes ({got} in the file) without a NUL byte: it ends after {end} bytes (FreeSurfer's has memory contents after them)"
            ),
        );
    }
    bytes[..end].to_vec()
}

/// A name of `len` bytes read and kept as `how` copies it; the file name's or an entry's.
fn read_name(
    g: &mut Globals,
    err: &mut dyn Write,
    fp: &mut In<'_>,
    who: &str,
    len: i32,
    how: Copy,
) -> Vec<u8> {
    let len64 = i64::from(len);
    let direct = matches!(how, Copy::DirectTable | Copy::DirectEntry);
    let want = if direct {
        len64.min(NAME_MAX as i64) as u64
    } else {
        len64 as u64
    };
    let mut buf = vec![0u8; (want as usize).min(NAME_MAX + 1)];
    let got = fp.bytes(&mut buf, want);
    if !fp.znz() && got != want {
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            "CTABreadFromBinaryV1: could not read parameter(s)",
        );
    }
    if !direct && len64 > NOTE_LENGTH && got < want {
        g.notices.add(
            &FS_191,
            format!(
                "{who}: a name length of {len} ({got} bytes in the file): FreeSurfer allocates the length plus one first"
            ),
        );
    }
    if direct && len64 > NAME_MAX as i64 {
        let rest = (len64 - NAME_MAX as i64) as u64;
        if g.notices.reproduce() {
            g.notices.add(
                &FS_190,
                format!("{who}: the {rest} bytes of a name after its first 1023 left to be read as numbers"),
            );
        } else {
            g.notices.add(
                &FS_190,
                format!("{who}: the {rest} bytes of a name after its first 1023 skipped"),
            );
            let mut none = [0u8; 0];
            fp.bytes(&mut none, rest);
        }
    }
    take_name(g, who, &buf, got, len64, how)
}

/// `(float)i / 255.0` stored in a `float`.
fn unit(i: i32) -> f32 {
    (f64::from(i as f32) / 255.0) as f32
}

/// The entry an entry's numbers make (its tissue type and voxel count are memory contents in
/// FreeSurfer, which no reached code reads for a table read from a file: 0 here).
fn entry(name: Vec<u8>, ri: i32, gi: i32, bi: i32, t: i32) -> Box<ColorTableEntry> {
    let ai = 255i32.wrapping_sub(t);
    Box::new(ColorTableEntry {
        name,
        ri,
        gi,
        bi,
        ai,
        rf: unit(ri),
        gf: unit(gi),
        bf: unit(bi),
        af: unit(ai),
        tissue_type: 0,
        count: 0,
    })
}

/// FS-192 at the end of a read: the file ended inside the table.
fn ended(
    g: &mut Globals,
    err: &mut dyn Write,
    fp: &In<'_>,
    who: &str,
    ct: ColorTable,
) -> Option<ColorTable> {
    if !fp.eof() {
        return Some(ct);
    }
    if g.notices.reproduce() {
        g.notices.add(
            &FS_192,
            format!("{who}: the file ends inside the table: its missing numbers read as 0 (memory contents in FreeSurfer)"),
        );
        Some(ct)
    } else {
        g.notices.add(
            &FS_192,
            format!("{who}: the file ends inside the table: an error"),
        );
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("{who}: the file ends inside the colour table"),
        );
        None
    }
}

/// The table of `nentries` empty places, or the reader's allocation error (FS-191).
fn table(g: &mut Globals, err: &mut dyn Write, v: u32, nentries: i32) -> Option<ColorTable> {
    if nentries > MAX_ENTRIES {
        g.notices.add(
            &FS_191,
            format!(
                "CTABreadFromBinaryV{v}: an entry count of {nentries}, above larmorx's {MAX_ENTRIES}, refused"
            ),
        );
        log::error_printf(
            g,
            err,
            ERROR_NO_MEMORY,
            &format!("CTABreadFromBinaryV{v}: could not allocate {nentries} entries"),
        );
        return None;
    }
    Some(ColorTable {
        entries: vec![None; nentries as usize],
        version: v as i32,
        ..ColorTable::default()
    })
}

/// `CTABreadFromBinaryV1(fp, nentries)` / `znzCTABreadFromBinaryV1`.
fn read_v1(
    g: &mut Globals,
    err: &mut dyn Write,
    fp: &mut In<'_>,
    nentries: i32,
) -> Option<ColorTable> {
    const W: &str = "CTABreadFromBinaryV1";
    if nentries < 0 {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("{W}: nentries was {nentries}"),
        );
        return None;
    }
    let mut ct = table(g, err, 1, nentries)?;
    let len = fp.int(g, err);
    if len < 0 {
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("{W}: file name length was {len}"),
        );
        return None;
    }
    ct.fname = read_name(g, err, fp, W, len, Copy::StrncpyTable);
    for structure in 0..nentries {
        let len = fp.int(g, err);
        if len < 0 {
            log::error_printf(
                g,
                err,
                ERROR_BADFILE,
                &format!("{W}: structure {structure} name length was {len}"),
            );
            return None;
        }
        if len >= 1024 {
            g.notices.add(
                &FS_188,
                format!("{W}: a name length of {len}: FreeSurfer writes a NUL byte {len} bytes into a 1024-byte name (not written here)"),
            );
        }
        let name = read_name(g, err, fp, W, len, Copy::StrncpyEntryCut);
        let ri = fp.int(g, err);
        let gi = fp.int(g, err);
        let bi = fp.int(g, err);
        let t = fp.int(g, err);
        ct.entries[structure as usize] = Some(entry(name, ri, gi, bi, t));
    }
    ended(g, err, fp, W, ct)
}

/// `CTABreadFromBinaryV2(fp)` / `znzCTABreadFromBinaryV2`.
fn read_v2(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fp: &mut In<'_>,
) -> Option<ColorTable> {
    const W: &str = "CTABreadFromBinaryV2";
    let nentries = fp.int(g, err);
    if nentries < 0 {
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("{W}: nentries was {nentries}"),
        );
        return None;
    }
    let mut ct = table(g, err, 2, nentries)?;
    let len = fp.int(g, err);
    if len < 0 {
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("{W}: file name length was {len}"),
        );
        return None;
    }
    let znz = fp.znz();
    let (fhow, ehow) = if znz {
        (Copy::DirectTable, Copy::DirectEntry)
    } else {
        (Copy::StrncpyTable, Copy::StrncpyEntry)
    };
    ct.fname = read_name(g, err, fp, W, len, fhow);
    let num = fp.int(g, err);
    if znz {
        if g.gdiag & DIAG_INFO != 0 {
            let _ = out.write_all(b"[INFO] znzCTABreadFromBinaryV2(): fname = '");
            let _ = out.write_all(&ct.fname);
            let _ = writeln!(
                out,
                "' ({} bytes), ct->nentries={nentries}, num_entries_to_read={num}",
                len.min(NAME_MAX as i32)
            );
        }
    } else if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(
            out,
            "[INFO] CTABreadFromBinaryV2(): ct->nentries={nentries}, num_entries_to_read={num}"
        );
    }
    for _ in 0..num.max(0) {
        let structure = fp.int(g, err);
        if structure < 0 {
            log::error_printf(
                g,
                err,
                ERROR_BADFILE,
                &format!("{W}: read entry index {structure}"),
            );
            return None;
        }
        if structure >= nentries {
            g.notices.add(
                &FS_189,
                format!(
                    "{W}: structure number {structure} of a table of {nentries} entries refused"
                ),
            );
            log::error_printf(
                g,
                err,
                ERROR_BADFILE,
                &format!("{W}: read entry index {structure}"),
            );
            return None;
        }
        if ct.entries[structure as usize].is_some() {
            log::error_printf(
                g,
                err,
                ERROR_BADFILE,
                &format!("{W}: Duplicate structure {structure}"),
            );
            return None;
        }
        let len = fp.int(g, err);
        if len < 0 {
            log::error_printf(
                g,
                err,
                ERROR_BADFILE,
                &format!("{W}: structure {structure} name length was {len}"),
            );
            return None;
        }
        let name = read_name(g, err, fp, W, len, ehow);
        let ri = fp.int(g, err);
        let gi = fp.int(g, err);
        let bi = fp.int(g, err);
        let t = fp.int(g, err);
        ct.entries[structure as usize] = Some(entry(name, ri, gi, bi, t));
    }
    ended(g, err, fp, W, ct)
}

fn read_any(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mut fp: In<'_>,
) -> Option<ColorTable> {
    let version = fp.int(g, err);
    if version > 0 {
        return read_v1(g, err, &mut fp, version);
    }
    if version.wrapping_neg() == 2 {
        read_v2(g, out, err, &mut fp)
    } else {
        log::error_printf(g, err, ERROR_BADFILE, "CTABreadFromBinary: unknown version");
        None
    }
}

/// `CTABreadFromBinary(fp)`: the table (its first number is the first version's entry count when
/// positive, -2 for the second version); `None` after the readers' messages
/// (`CTABreadFromBinary: fp was NULL` for `None`, `CTABreadFromBinary: unknown version`, ...).
pub fn ctab_read_from_binary(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fp: Option<&mut CFile>,
) -> Option<ColorTable> {
    let Some(fp) = fp else {
        log::error_printf(g, err, ERROR_BADPARM, "CTABreadFromBinary: fp was NULL");
        return None;
    };
    read_any(g, out, err, In::File(fp))
}

/// `znzCTABreadFromBinary(fp)`: as [`ctab_read_from_binary`] on a znzlib file (the second version
/// reads names of at most 1,023 bytes straight into the table: FS-190).
pub fn znz_ctab_read_from_binary(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fp: Option<&mut ZnzFile>,
) -> Option<ColorTable> {
    let Some(fp) = fp else {
        log::error_printf(g, err, ERROR_BADPARM, "CTABreadFromBinary: fp was NULL");
        return None;
    };
    read_any(g, out, err, In::Znz(fp))
}

/// The stream a writer writes to.
pub enum Out<'a> {
    File(&'a mut CFile),
    Znz(&'a mut ZnzFile),
}

impl Out<'_> {
    fn int(&mut self, v: i32) {
        match self {
            Out::File(fp) => {
                fio::fwrite_int(fp, v);
            }
            Out::Znz(fp) => {
                fio::znz_write_int(fp, v);
            }
        }
    }
    fn bytes(&mut self, b: &[u8]) {
        match self {
            Out::File(fp) => {
                fp.fwrite(b, 1, b.len());
            }
            Out::Znz(fp) => {
                fio::fs_znzwrite(fp, b, 1, b.len());
            }
        }
    }
    /// A name's length (its C string's plus one) and its bytes with the NUL.
    fn name(&mut self, s: &[u8]) {
        let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        self.int((end as i32).wrapping_add(1));
        let mut b = s[..end].to_vec();
        b.push(0);
        self.bytes(&b);
    }
}

/// `CTABwriteIntoBinaryV1` / `znzCTABwriteIntoBinaryV1`: the entry count, the file name, then each
/// non-empty entry's name and colour (`255 - ai`).
fn write_v1(ct: &ColorTable, fp: &mut Out<'_>) -> i32 {
    fp.int(ct.nentries());
    fp.name(&ct.fname);
    for e in ct.entries.iter().flatten() {
        fp.name(&e.name);
        fp.int(e.ri);
        fp.int(e.gi);
        fp.int(e.bi);
        fp.int(255i32.wrapping_sub(e.ai));
    }
    NO_ERROR
}

/// `CTABwriteIntoBinaryV2` / `znzCTABwriteIntoBinaryV2`: -2, the entry count, the file name, the
/// count of non-empty entries, then each one's structure number, name and colour.
fn write_v2(g: &Globals, out: &mut dyn Write, ct: &ColorTable, fp: &mut Out<'_>) -> i32 {
    fp.int(-2);
    fp.int(ct.nentries());
    fp.name(&ct.fname);
    let n = ct.entries.iter().filter(|e| e.is_some()).count() as i32;
    fp.int(n);
    if matches!(fp, Out::File(_)) && g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(
            out,
            "[DEBUG] CTABwriteIntoBinaryV2(): ct->entries={}, to_write={n}",
            ct.nentries()
        );
    }
    for (structure, e) in ct.entries.iter().enumerate() {
        if let Some(e) = e {
            fp.int(structure as i32);
            fp.name(&e.name);
            fp.int(e.ri);
            fp.int(e.gi);
            fp.int(e.bi);
            fp.int(255i32.wrapping_sub(e.ai));
        }
    }
    NO_ERROR
}

fn write_any(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ct: Option<&ColorTable>,
    fp: Option<Out<'_>>,
) -> i32 {
    let Some(ct) = ct else {
        log::error_printf(g, err, ERROR_BADPARM, "CTABwriteIntoBinary: ct was NULL");
        return ERROR_BADPARM;
    };
    let Some(mut fp) = fp else {
        log::error_printf(g, err, ERROR_BADPARM, "CTABwriteIntoBinary: fp was NULL");
        return ERROR_BADPARM;
    };
    match ct.version {
        1 => write_v1(ct, &mut fp),
        2 => write_v2(g, out, ct, &mut fp),
        _ => {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                "CTABwriteIntoBinary: unknown version",
            );
            ERROR_BADPARM
        }
    }
}

/// `CTABwriteIntoBinary(ct, fp)`: the table in its version (1 or 2); `NO_ERROR`, or
/// `ERROR_BADPARM` after `CTABwriteIntoBinary: ct was NULL`, `... fp was NULL` or `... unknown
/// version`.
pub fn ctab_write_into_binary(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ct: Option<&ColorTable>,
    fp: Option<&mut CFile>,
) -> i32 {
    write_any(g, out, err, ct, fp.map(Out::File))
}

/// `znzCTABwriteIntoBinary(ct, fp)`: as [`ctab_write_into_binary`] on a znzlib file.
pub fn znz_ctab_write_into_binary(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ct: Option<&ColorTable>,
    fp: Option<&mut ZnzFile>,
) -> i32 {
    write_any(g, out, err, ct, fp.map(Out::Znz))
}
