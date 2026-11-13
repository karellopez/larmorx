// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/tags.cpp` of FreeSurfer 8.2.0 (queue item 3.1-30's tags, taken with 3.1-25's second
//! piece, the orchestrator's decision of 2026-11-13): the tagged fields that follow a volume or a
//! surface in FreeSurfer's files (a tag number, usually a 64-bit data length, the data), on C streams
//! (`TAGreadStart`, `TAGread`, `TAGskip`, `TAGwriteStart`, `TAGwrite`, `TAGwriteEnd`,
//! `TAGmakeCommandLineString`, `TAGreadMatrix`, `TAGwriteMatrix`) and on znzlib files
//! (`znzTAGreadStart`, `znzTAGwriteStart`, `znzTAGwriteEnd`, `znzWriteMatrix`, `znzReadMatrix`),
//! with the matrix text both families share (FS-177 to FS-182).

use std::io::Write;

use larmorx_core::defects::{FS_177, FS_178, FS_179, FS_180, FS_181, FS_182};
use larmorx_freesurfer_core::cscan::{self, Bytes, Call, Stream};
use larmorx_freesurfer_core::cstd::{fmt_f, is_space};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_NOMEMORY, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_io::znz::ZnzFile;

use crate::cfile::CFile;
use crate::fio;

pub const TAG_OLD_COLORTABLE: i32 = 1;
pub const TAG_OLD_USEREALRAS: i32 = 2;
pub const TAG_CMDLINE: i32 = 3;
pub const TAG_USEREALRAS: i32 = 4;
pub const TAG_DOF: i32 = 7;
pub const TAG_RAS_XFORM: i32 = 8;
pub const TAG_RAS_XFORM_EXTRA: i32 = 9;
pub const TAG_GCAMORPH_GEOM: i32 = 10;
pub const TAG_GCAMORPH_TYPE: i32 = 11;
pub const TAG_GCAMORPH_LABELS: i32 = 12;
pub const TAG_GCAMORPH_META: i32 = 13;
pub const TAG_GCAMORPH_AFFINE: i32 = 14;
pub const TAG_GCAMORPH_GEOM_PLUSSHEAR: i32 = 15;
pub const TAG_OLD_SURF_GEOM: i32 = 20;
pub const TAG_SURF_DATASPACE: i32 = 22;
pub const TAG_SURF_MATRIXDATA: i32 = 23;
pub const TAG_SURF_TRANSFORMEDSPACE: i32 = 24;
pub const TAG_OLD_MGH_XFORM: i32 = 30;
pub const TAG_MGH_XFORM: i32 = 31;
pub const TAG_GROUP_AVG_SURFACE_AREA: i32 = 32;
pub const TAG_AUTO_ALIGN: i32 = 33;
pub const TAG_SCALAR_DOUBLE: i32 = 40;
pub const TAG_PEDIR: i32 = 41;
pub const TAG_MRI_FRAME: i32 = 42;
pub const TAG_FIELDSTRENGTH: i32 = 43;
pub const TAG_ORIG_RAS2VOX: i32 = 44;
pub const TAG_SCAN_PARAMETERS: i32 = 45;
pub const TAG_END_NIIHDREXTENSION: i32 = -1;

/// `MATRIX_STRLEN` (`4 * 4 * 100`): the bytes of a matrix tag's text in the compressed forms.
pub const MATRIX_STRLEN: usize = 1600;

/// The bytes the skippers read at a time (FreeSurfer allocates the whole length: FS-178).
const CHUNK: usize = 1 << 16;

/// `errno`'s `ENOMEM` on Linux.
const ENOMEM: i32 = 12;

/// A 4 x 4 real matrix of zeros (`MatrixAlloc(4, 4, MATRIX_REAL)`, which cannot fail).
pub(crate) fn m44(g: &mut Globals) -> Matrix {
    let mut sink = Vec::new();
    let mut sink2 = Vec::new();
    matrix_alloc(g, &mut sink, &mut sink2, 4, 4, MATRIX_REAL)
        .ok()
        .flatten()
        .expect("a 4 x 4 matrix")
}

/// `M->rptr[r][c]` of a matrix the tag writers expect to be 4 x 4: 0 outside a smaller one
/// (FS-181: memory outside it in FreeSurfer).
pub(crate) fn elt44(g: &mut Globals, m: &Matrix, r: i32, c: i32, who: &str) -> f32 {
    if r <= m.rows
        && c <= m.cols
        && let Some(i) = m.relt_index(r, c)
    {
        return m.raw(i).unwrap_or(0.0);
    }
    g.notices.add(
        &FS_181,
        format!(
            "{who} of a {} x {} matrix: element ({r}, {c}) written as 0",
            m.rows, m.cols
        ),
    );
    0.0
}

/// The text `sprintf("<word> %10lf ... %10lf", M[1][1] ... M[4][4])` (16 numbers, row by row).
pub(crate) fn matrix_text(g: &mut Globals, m: &Matrix, word: &str, who: &str) -> Vec<u8> {
    let mut s = String::from(word);
    for r in 1..=4 {
        for c in 1..=4 {
            let v = elt44(g, m, r, c, who);
            s.push(' ');
            s.push_str(&fmt_f(f64::from(v), 10, 6));
        }
    }
    s.into_bytes()
}

/// `%s`: white space skipped, then the characters up to the next white space or the end; `None` at
/// the end of the input before a character (`errno` 0 when `fresh`, as glibc's first conversion of
/// a call at the end of its input).
pub(crate) fn scan_word(st: &mut dyn Stream, errno: &mut i32, fresh: bool) -> Option<Vec<u8>> {
    cscan::skip_space(st);
    if st.peek().is_none() {
        if fresh {
            *errno = 0;
        }
        return None;
    }
    let mut w = Vec::new();
    while let Some(c) = st.peek() {
        if is_space(c) {
            break;
        }
        w.push(c);
        st.bump();
    }
    Some(w)
}

/// `sscanf(buf, "%s %f %f ... %f", ch, &M[1][1], ..., &M[4][4])` on the `got` bytes a read gave into
/// a `MATRIX_STRLEN`-byte buffer (the compressed readers): the numbers converted go into `m` in
/// order until a conversion fails. FS-179: a first word of 100 bytes or more (the size of the
/// word's buffer `ch`); FS-180: bytes without a NUL (glibc's `sscanf` measures the text first and
/// reads past them; a scan that reaches their end takes memory contents).
pub(crate) fn scan_matrix_text(g: &mut Globals, buf: &[u8], m: &mut Matrix, who: &str) {
    let has_nul = buf.contains(&0);
    let mut st = Bytes::new(buf);
    let mut errno = g.errno;
    let mut call = Call::new();
    let word = scan_word(&mut st, &mut errno, true);
    if let Some(w) = &word {
        if w.len() >= 100 {
            g.notices.add(
                &FS_179,
                format!(
                    "{who}: a first word of {} bytes read whole (FreeSurfer's buffer holds 100)",
                    w.len()
                ),
            );
        }
        'all: for r in 1..=4 {
            for c in 1..=4 {
                cscan::skip_space(&mut st);
                match call.f32(&mut st, &mut errno) {
                    Some(v) => m.set_relt(r, c, v),
                    None => break 'all,
                }
            }
        }
    }
    if !has_nul {
        let touched = st.i >= st.s.len();
        let detail = if touched {
            format!(
                "{who}: the matrix text reached the end of its {} bytes without a NUL byte: FreeSurfer reads on into memory contents (the text ends there here)",
                buf.len()
            )
        } else {
            format!(
                "{who}: the matrix's {} bytes have no NUL byte: FreeSurfer reads past them (no effect on the numbers)",
                buf.len()
            )
        };
        g.notices.add(&FS_180, detail);
    }
    g.errno = errno;
}

/// The skip of a tag's `len` bytes by an allocation of the length (`TAGskip`, `znzTAGskip`,
/// `FStagsIO::skip_tag`): the message and `ERROR_NOMEMORY` exit where the allocation cannot be
/// made (a negative length, or one beyond the largest object), FS-177's number in it; the bytes
/// read otherwise, read here a chunk at a time (FS-178: FreeSurfer allocates the whole length).
pub(crate) fn skip_by_alloc(
    g: &mut Globals,
    err: &mut dyn Write,
    len: i64,
    msg: impl Fn(String) -> String,
    mut read: impl FnMut(&mut [u8]) -> usize,
) -> Result<i32, Fatal> {
    if len < 0 || len as u64 > isize::MAX as u64 {
        let low = len as u32;
        let shown = if g.notices.reproduce() {
            low.to_string()
        } else {
            len.to_string()
        };
        if i64::from(low) != len {
            g.notices.add(
                &FS_177,
                format!("a tag length {len} shown as {low} in FreeSurfer's message"),
            );
        }
        let text = msg(shown);
        // the failed calloc leaves errno ENOMEM, which ErrorExit prints first
        g.errno = ENOMEM;
        log::error_exit(g, err, ERROR_NOMEMORY, &text)?;
        return Ok(0);
    }
    let want = len as u64;
    let mut buf = vec![0u8; CHUNK.min(want as usize)];
    let mut total: u64 = 0;
    while total < want {
        let n = ((want - total) as usize).min(CHUNK);
        let r = read(&mut buf[..n]);
        total += r as u64;
        if r < n {
            break;
        }
    }
    if total < want {
        g.notices.add(
            &FS_178,
            format!(
                "a tag of {want} bytes skipped where the file held {total}: FreeSurfer allocates the whole length first"
            ),
        );
    }
    Ok(total as i32)
}

/// `TAGskip(fp, tag, len)`: `len` bytes read and dropped; the count read (C's `int`), or the end
/// of the program after `TAGskip: tag=<tag>, failed to calloc <len> bytes!` (FS-177, FS-178).
pub fn tag_skip(
    g: &mut Globals,
    err: &mut dyn Write,
    fp: &mut CFile,
    tag: i32,
    len: i64,
) -> Result<i32, Fatal> {
    skip_by_alloc(
        g,
        err,
        len,
        |n| format!("TAGskip: tag={tag}, failed to calloc {n} bytes!\n"),
        |b| fp.fread(b, 1, b.len()),
    )
}

/// `TAGreadStart(fp, &len)`: the tag number (0 at the end of the file, `len` then untouched) and
/// its data length: `TAG_OLD_MGH_XFORM`'s 32-bit length less one, none (0) for
/// `TAG_OLD_SURF_GEOM`, `TAG_OLD_USEREALRAS` and `TAG_OLD_COLORTABLE`, a 64-bit length for the
/// others.
pub fn tag_read_start(g: &mut Globals, err: &mut dyn Write, fp: &mut CFile, plen: &mut i64) -> i32 {
    let tag = fio::fread_int(g, fp);
    if fp.feof() {
        return 0;
    }
    match tag {
        TAG_OLD_MGH_XFORM => *plen = i64::from(fio::fread_int(g, fp)) - 1,
        TAG_OLD_SURF_GEOM | TAG_OLD_USEREALRAS | TAG_OLD_COLORTABLE => *plen = 0,
        _ => *plen = fio::fread_long(g, err, fp),
    }
    tag
}

/// `TAGread(fp, buf, len)`: `len` bytes into `buf` (a negative length, as C's unsigned size, reads
/// to the end of the file); bytes beyond `buf` are read and dropped (FreeSurfer writes them past
/// the caller's buffer: its caller's concern). `NO_ERROR`.
pub fn tag_read(fp: &mut CFile, buf: &mut [u8], len: i64) -> i32 {
    let want = if len < 0 { u64::MAX } else { len as u64 };
    let n = (buf.len() as u64).min(want) as usize;
    let got = fp.fread(&mut buf[..n], 1, n);
    if got == n && want > n as u64 {
        let mut rest = vec![0u8; CHUNK];
        let mut left = want - n as u64;
        while left > 0 {
            let k = (left.min(CHUNK as u64)) as usize;
            let r = fp.fread(&mut rest[..k], 1, k);
            left -= r as u64;
            if r < k {
                break;
            }
        }
    }
    NO_ERROR
}

/// `TAGwriteStart(fp, tag, &here, len)`: the tag number, then (`here` set to the position after
/// it) the 64-bit length. `NO_ERROR`.
pub fn tag_write_start(
    g: &mut Globals,
    fp: &mut CFile,
    tag: i32,
    phere: &mut i64,
    len: i64,
) -> i32 {
    fio::fwrite_int(fp, tag);
    *phere = fp.tell();
    fio::fwrite_long(g, fp, len);
    NO_ERROR
}

/// `TAGwriteEnd(fp, there)`: nothing (FreeSurfer's length rewrite is compiled out). `NO_ERROR`.
pub fn tag_write_end(fp: &mut CFile, _there: i64) -> i32 {
    let _ = fp.tell();
    NO_ERROR
}

/// `TAGwrite(fp, tag, buf, len)`: [`tag_write_start`], `len` bytes of `buf`, [`tag_write_end`].
pub fn tag_write(g: &mut Globals, fp: &mut CFile, tag: i32, buf: &[u8], len: i64) -> i32 {
    let mut here = 0;
    tag_write_start(g, fp, tag, &mut here, len);
    let n = (len.max(0) as u64).min(buf.len() as u64) as usize;
    fp.fwrite(&buf[..n], 1, n);
    tag_write_end(fp, here);
    NO_ERROR
}

/// `TAGmakeCommandLineString(argc, argv, cmd_line)`: each argument followed by a space. FS-182: the
/// caller's buffer (`cap` bytes, `CMD_LINE_LEN` = 4,096 in `mri_segment`) is not checked; the
/// whole line here.
pub fn tag_make_command_line_string(g: &mut Globals, argv: &[&[u8]], cap: usize) -> Vec<u8> {
    let mut s = Vec::new();
    for a in argv {
        let end = a.iter().position(|&c| c == 0).unwrap_or(a.len());
        s.extend_from_slice(&a[..end]);
        s.push(b' ');
    }
    if s.len() >= cap {
        g.notices.add(
            &FS_182,
            format!(
                "a command line of {} bytes kept whole (the caller's buffer holds {cap})",
                s.len() + 1
            ),
        );
    }
    s
}

/// `TAGreadMatrix(fp)`: a 4 x 4 matrix of zeros, then `fscanf(fp, "%s", ...)` (the word, `Matrix`
/// or `AutoAlign`) and sixteen separate `fscanf(fp, "%f", ...)`, row by row: an element whose
/// conversion fails stays 0 and the next conversion starts where it stopped. FS-179: a word of
/// 1,600 bytes or more.
pub fn tag_read_matrix(g: &mut Globals, fp: &mut CFile) -> Matrix {
    let mut m = m44(g);
    let mut errno = g.errno;
    if let Some(w) = scan_word(fp, &mut errno, true)
        && w.len() >= MATRIX_STRLEN
    {
        g.notices.add(
            &FS_179,
            format!(
                "TAGreadMatrix: a first word of {} bytes read whole (FreeSurfer's buffer holds 1600)",
                w.len()
            ),
        );
    }
    for r in 1..=4 {
        for c in 1..=4 {
            if let Some(v) = Call::new().f32(fp, &mut errno) {
                m.set_relt(r, c, v);
            }
        }
    }
    g.errno = errno;
    m
}

/// `TAGwriteMatrix(fp, M, tag)`: the tag with the text `Matrix %10lf ... %10lf` (its length, no NUL
/// byte). `NO_ERROR`.
pub fn tag_write_matrix(g: &mut Globals, fp: &mut CFile, m: &Matrix, tag: i32) -> i32 {
    let text = matrix_text(g, m, "Matrix", "TAGwriteMatrix");
    let mut here = 0;
    tag_write_start(g, fp, tag, &mut here, text.len() as i64);
    fp.fwrite(&text, 1, text.len());
    tag_write_end(fp, here);
    NO_ERROR
}

/// `znzTAGreadStart(fp, &len, tagwithzerolen)`: 0 when the file is at its end before or after the
/// tag number (`len` untouched); `tagwithzerolen`'s tag (when not 0) with length 0; then as
/// [`tag_read_start`], with `TAG_GCAMORPH_GEOM`, `TAG_GCAMORPH_TYPE` and `TAG_GCAMORPH_LABELS` also
/// without a length.
pub fn znz_tag_read_start(
    g: &mut Globals,
    err: &mut dyn Write,
    fp: &mut ZnzFile,
    plen: &mut i64,
    tagwithzerolen: i32,
) -> i32 {
    if fp.eof() {
        return 0;
    }
    let tag = fio::znz_read_int(g, err, fp);
    if fp.eof() {
        return 0;
    }
    if tagwithzerolen != 0 && tagwithzerolen == tag {
        *plen = 0;
        return tag;
    }
    match tag {
        TAG_OLD_MGH_XFORM => *plen = i64::from(fio::znz_read_int(g, err, fp)) - 1,
        TAG_OLD_SURF_GEOM | TAG_OLD_USEREALRAS | TAG_OLD_COLORTABLE | TAG_GCAMORPH_GEOM
        | TAG_GCAMORPH_TYPE | TAG_GCAMORPH_LABELS => *plen = 0,
        _ => *plen = fio::znz_read_long(g, err, fp),
    }
    tag
}

/// `znzTAGwriteStart(fp, tag, &here, len)`: as [`tag_write_start`] (`here` the position after the
/// tag number, the uncompressed one for a compressed file).
pub fn znz_tag_write_start(
    g: &mut Globals,
    fp: &mut ZnzFile,
    tag: i32,
    phere: &mut i64,
    len: i64,
) -> i32 {
    fio::znz_write_int(fp, tag);
    *phere = fp.tell();
    fio::znz_write_long(g, fp, len);
    NO_ERROR
}

/// `znzTAGwriteEnd(fp, there)`: nothing. `NO_ERROR`.
pub fn znz_tag_write_end(fp: &mut ZnzFile, _there: i64) -> i32 {
    let _ = fp.tell();
    NO_ERROR
}

/// `znzWriteMatrix(fp, M, tag)`: the tag with length 1,600 and the text `Matrix %10lf ... %10lf`
/// in 1,600 bytes (zeros after it). `NO_ERROR`.
pub fn znz_write_matrix(g: &mut Globals, fp: &mut ZnzFile, m: &Matrix, tag: i32) -> i32 {
    let text = matrix_text(g, m, "Matrix", "znzWriteMatrix");
    let mut buf = vec![0u8; MATRIX_STRLEN];
    buf[..text.len()].copy_from_slice(&text);
    let mut here = 0;
    znz_tag_write_start(g, fp, tag, &mut here, MATRIX_STRLEN as i64);
    fio::fs_znzwrite(fp, &buf, 1, MATRIX_STRLEN);
    znz_tag_write_end(fp, here);
    NO_ERROR
}

/// `znzread(buf, 1, MATRIX_STRLEN, fp)` into a buffer that holds no earlier bytes FreeSurfer
/// defines: the bytes the file gave (the rest are memory contents in FreeSurfer: see
/// [`scan_matrix_text`]).
pub(crate) fn read_matrix_bytes(fp: &mut ZnzFile) -> Vec<u8> {
    let mut buf = vec![0u8; MATRIX_STRLEN];
    let before = fp.tell();
    let r = fio::fs_znzread(fp, &mut buf, 1, MATRIX_STRLEN);
    let after = fp.tell();
    let got = if before >= 0 && after >= before {
        ((after - before) as usize).min(MATRIX_STRLEN)
    } else {
        r.clamp(0, MATRIX_STRLEN as i64) as usize
    };
    buf.truncate(got);
    buf
}

/// `znzReadMatrix(fp)`: [`znz_tag_read_start`] (its length unused), then a 4 x 4 matrix of zeros
/// filled from the 1,600 bytes that follow as [`scan_matrix_text`] reads them.
pub fn znz_read_matrix(g: &mut Globals, err: &mut dyn Write, fp: &mut ZnzFile) -> Matrix {
    let mut len = 0;
    znz_tag_read_start(g, err, fp, &mut len, 0);
    let mut m = m44(g);
    let buf = read_matrix_bytes(fp);
    scan_matrix_text(g, &buf, &mut m, "znzReadMatrix");
    m
}
