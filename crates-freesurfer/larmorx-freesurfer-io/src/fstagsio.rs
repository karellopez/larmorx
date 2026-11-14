// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/fstagsio.cpp` (`FStagsIO`) and `include/mri.h`'s `VOL_GEOM::write`/`VOL_GEOM::read` of
//! FreeSurfer 8.2.0 (queue item 3.1-25, piece 2): the tags that follow an `.mgz` volume's voxels and
//! fill FreeSurfer's NIfTI header extension, written and read on a znzlib file.
//!
//! In an `.mgz` most tags are `tag number, 64-bit data length, data`; `TAG_OLD_COLORTABLE`,
//! `TAG_GCAMORPH_GEOM`, `TAG_GCAMORPH_TYPE` and `TAG_GCAMORPH_LABELS` carry no length. In the NIfTI
//! extension (`niftiheaderext`) every tag has a length, and the matrices, frames and geometries
//! have shorter forms. The `getlen_*` functions give a tag's length as its writer writes it
//! (`addtaglength`: with the tag number and the length field). FS-183 to FS-186, FS-193.

use std::io::Write;

use larmorx_core::defects::{FS_183, FS_184, FS_185, FS_186, FS_193};
use larmorx_freesurfer_core::cname::STRLEN;
use larmorx_freesurfer_core::diag::DIAG_INFO;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::matrix_ops::matrix_print;
use larmorx_freesurfer_mri::ctab::ColorTable;
use larmorx_freesurfer_mri::mri::{Mri, MriFrame};
use larmorx_freesurfer_mri::vol_geom::{self, VolGeom};
use larmorx_io::znz::ZnzFile;

use crate::colortab_bin;
use crate::fio;
use crate::tags::{
    self, MATRIX_STRLEN, TAG_DOF, TAG_END_NIIHDREXTENSION, TAG_GCAMORPH_GEOM,
    TAG_GCAMORPH_GEOM_PLUSSHEAR, TAG_GCAMORPH_LABELS, TAG_GCAMORPH_META, TAG_GCAMORPH_TYPE,
    TAG_MRI_FRAME, TAG_OLD_COLORTABLE, TAG_OLD_MGH_XFORM, TAG_OLD_SURF_GEOM, TAG_OLD_USEREALRAS,
    TAG_RAS_XFORM, TAG_RAS_XFORM_EXTRA, TAG_SCAN_PARAMETERS,
};

/// `sizeof(MRI_FRAME)` on x86-64 (GCC's layout of `include/mri.h`'s structure): 1,288 bytes.
pub const SIZEOF_MRI_FRAME: i64 = 1288;

/// `FRAME_TYPE_DIFFUSION_AUGMENTED`: a frame that also stores its diffusion parameters.
pub const FRAME_TYPE_DIFFUSION_AUGMENTED: i32 = 1;

/// The name bytes a geometry stores.
const GEOM_NAME: usize = 512;

/// A stack array beyond which FreeSurfer's skip of a long geometry name may not fit its stack
/// (FS-185's note).
const STACK_NOTE: i64 = 1 << 20;

/// Whether `tag` has no length field in an `.mgz`.
fn lengthless(tag: i32) -> bool {
    matches!(
        tag,
        TAG_OLD_COLORTABLE | TAG_GCAMORPH_GEOM | TAG_GCAMORPH_TYPE | TAG_GCAMORPH_LABELS
    )
}

/// The C string's length of a name's bytes.
fn clen(b: &[u8]) -> usize {
    b.iter().position(|&c| c == 0).unwrap_or(b.len())
}

/// `FStagsIO::getlen_tag(tag, len, niftiheaderext, addtaglength)`.
pub fn getlen_tag(tag: i32, len: i64, niftiheaderext: bool, addtaglength: bool) -> i64 {
    let mut d = 0;
    if addtaglength {
        d += 4;
        if niftiheaderext || !lengthless(tag) {
            d += 8;
        }
    }
    d + len
}

/// `FStagsIO::getlen_matrix(niftiheaderext, addtaglength)`: 64 bytes of floats in the extension,
/// `MATRIX_STRLEN` of text in an `.mgz` (12 more with the tag and length).
pub fn getlen_matrix(niftiheaderext: bool, addtaglength: bool) -> i64 {
    let head = if addtaglength { 12 } else { 0 };
    head + if niftiheaderext {
        64
    } else {
        MATRIX_STRLEN as i64
    }
}

/// `FStagsIO::getlen_old_colortable(ctab, niftiheaderext, addtaglength)`: the binary table's bytes
/// as `znzCTABwriteIntoBinary` writes it (the second version's when `version` is 2, the first's
/// otherwise).
pub fn getlen_old_colortable(ctab: &ColorTable, niftiheaderext: bool, addtaglength: bool) -> i64 {
    let mut d: i64 = 0;
    if addtaglength {
        d += 4;
        if niftiheaderext {
            d += 8;
        }
    }
    let v2 = ctab.version == 2;
    if v2 {
        d += 4;
    }
    d += 4 + 4 + clen(&ctab.fname) as i64 + 1;
    if v2 {
        d += 4;
    }
    for e in ctab.entries.iter().flatten() {
        if v2 {
            d += 4;
        }
        d += 4 + clen(&e.name) as i64 + 1 + 16;
    }
    d
}

/// `FStagsIO::getlen_mri_frames(mri, niftiheaderext, addtaglength)`: the extension's 1,032 bytes
/// per frame (label, name, threshold); an `.mgz`'s `10 * nframes * sizeof(MRI_FRAME)`.
pub fn getlen_mri_frames(mri: &Mri, niftiheaderext: bool, addtaglength: bool) -> i64 {
    let head = if addtaglength { 12 } else { 0 };
    head + if niftiheaderext {
        i64::from(mri.nframes.max(0)) * (4 + STRLEN as i64 + 4)
    } else {
        i64::from(10i32.wrapping_mul(mri.nframes)) * SIZEOF_MRI_FRAME
    }
}

/// `FStagsIO::getlen_gcamorph_geom(source_fname, target_fname, niftiheaderext, addtaglength,
/// shearless)`: two geometries of 588 bytes in an `.mgz`; in the extension, 80 bytes each and the
/// names' bytes (FS-193: FreeSurfer counts them whole, its writer stores at most 512 of each); 24
/// more for the shears.
pub fn getlen_gcamorph_geom(
    g: &mut Globals,
    source_fname: &[u8],
    target_fname: &[u8],
    niftiheaderext: bool,
    addtaglength: bool,
    shearless: bool,
) -> i64 {
    let mut d: i64 = 0;
    if addtaglength {
        d += 4;
        if niftiheaderext {
            d += 8;
        }
    }
    if !niftiheaderext {
        d += 2 * (16 + 60 + GEOM_NAME as i64);
    } else {
        let (s, t) = (clen(source_fname), clen(target_fname));
        let whole = (s + t) as i64;
        let stored = (s.min(GEOM_NAME) + t.min(GEOM_NAME)) as i64;
        let names = if whole != stored {
            g.notices.add(
                &FS_193,
                format!(
                    "a geometry tag's length with names of {s} and {t} bytes: {stored} stored, {whole} counted by FreeSurfer"
                ),
            );
            if g.notices.reproduce() { whole } else { stored }
        } else {
            whole
        };
        d += (2 * (16 + 60 + 4) + names as i32) as i64;
    }
    if !shearless {
        d += 24;
    }
    d
}

/// `FStagsIO::getlen_gcamorph_meta(addtaglength)`: 12 bytes (and 12 for the tag and length).
pub fn getlen_gcamorph_meta(addtaglength: bool) -> i64 {
    if addtaglength { 24 } else { 12 }
}

/// `FStagsIO::getlen_gcamorph_labels(x, y, z, len, niftiheaderext, addtaglength)`: `x * y * z *
/// len` bytes. FS-184: FreeSurfer multiplies in 32-bit `int`s (a product beyond 2^31 wraps); in 64
/// bits by default.
pub fn getlen_gcamorph_labels(
    g: &mut Globals,
    x: i32,
    y: i32,
    z: i32,
    len: i32,
    niftiheaderext: bool,
    addtaglength: bool,
) -> i64 {
    let mut d: i64 = 0;
    if addtaglength {
        d += 4;
        if niftiheaderext {
            d += 8;
        }
    }
    let exact = i64::from(x)
        .wrapping_mul(i64::from(y))
        .wrapping_mul(i64::from(z))
        .wrapping_mul(i64::from(len));
    let wrapped = i64::from(x.wrapping_mul(y).wrapping_mul(z).wrapping_mul(len));
    let v = if exact != wrapped {
        g.notices.add(
            &FS_184,
            format!("a label tag's length of {x} x {y} x {z} x {len} bytes: {exact}, {wrapped} in FreeSurfer's int"),
        );
        if g.notices.reproduce() {
            wrapped
        } else {
            exact
        }
    } else {
        exact
    };
    d + v
}

/// `FStagsIO::getlen_dof(dof, addtaglength)`: 4 bytes.
pub fn getlen_dof(addtaglength: bool) -> i64 {
    if addtaglength { 16 } else { 4 }
}

/// `FStagsIO::getlen_scan_parameters(mri, addtaglength)`: 20 bytes of numbers and the direction
/// with its NUL (`UNKNOWN`'s 7 bytes without one when there is none).
pub fn getlen_scan_parameters(mri: &Mri, addtaglength: bool) -> i64 {
    let head = if addtaglength { 12 } else { 0 };
    head + 20
        + match &mri.pedir {
            Some(p) => clen(p) as i64 + 1,
            None => 7,
        }
}

/// `FStagsIO::getlen_ras_xform(vg, addtaglength)`: 48 bytes.
pub fn getlen_ras_xform(addtaglength: bool) -> i64 {
    if addtaglength { 60 } else { 48 }
}

/// `FStagsIO::getlen_ras_xform_extra(vg, addtaglength)`: 36 bytes.
pub fn getlen_ras_xform_extra(addtaglength: bool) -> i64 {
    if addtaglength { 48 } else { 36 }
}

/// `FStagsIO::getlen_endtag(addtaglength)`: 1 byte.
pub fn getlen_endtag(addtaglength: bool) -> i64 {
    if addtaglength { 13 } else { 1 }
}

/// `MatrixPrint(stdout, m)`.
pub(crate) fn print_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: Option<&Matrix>,
) {
    let mut buf = Vec::new();
    matrix_print(g, Some(&mut buf), out, err, m, 5);
    let _ = out.write_all(&buf);
}

/// `VOL_GEOM::write(fp, niftiheaderext, shearless)`: `valid`, the extent (`int`s), voxel sizes,
/// direction cosines and centre (`float`s), the shears unless `shearless`, then the name: 512 bytes
/// of its buffer in an `.mgz` (the bytes after its NUL as an earlier name left them: rule C, the
/// orchestrator's decision of 2026-11-13), its length (at most 512) and bytes in the extension.
pub fn vol_geom_write(fp: &mut ZnzFile, vg: &VolGeom, niftiheaderext: bool, shearless: bool) {
    for v in [vg.valid, vg.width, vg.height, vg.depth] {
        fio::znz_write_int(fp, v);
    }
    for v in [
        vg.xsize, vg.ysize, vg.zsize, vg.x_r, vg.x_a, vg.x_s, vg.y_r, vg.y_a, vg.y_s, vg.z_r,
        vg.z_a, vg.z_s, vg.c_r, vg.c_a, vg.c_s,
    ] {
        fio::znz_write_float(fp, v);
    }
    if !shearless {
        for v in [vg.s_r, vg.s_a, vg.s_s] {
            fio::znz_write_float(fp, v);
        }
    }
    if !niftiheaderext {
        fio::fs_znzwrite(fp, &vg.fname.buffer()[..GEOM_NAME], 1, GEOM_NAME);
    } else {
        let n = vg.fname.len().min(GEOM_NAME);
        fio::znz_write_int(fp, n as i32);
        if n > 0 {
            fio::fs_znzwrite(fp, &vg.fname.buffer()[..n], 1, n);
        }
    }
}

/// `VOL_GEOM::read(fp, niftiheaderext, shearless)`: the fields [`vol_geom_write`] writes, into
/// `vg`; an `.mgz`'s 512 name bytes after its first 512 are cleared, the extension's name length
/// and at most 512 bytes (the rest skipped). FS-186: FreeSurfer does not end the name it reads (an
/// earlier, longer name's rest stays after it; a name of length 0 leaves the earlier one): by
/// default the name ends after the bytes the file gave. FS-185: the rest of a long name is skipped
/// through a stack array of its size.
pub fn vol_geom_read(
    g: &mut Globals,
    err: &mut dyn Write,
    fp: &mut ZnzFile,
    vg: &mut VolGeom,
    niftiheaderext: bool,
    shearless: bool,
) {
    vg.valid = fio::znz_read_int(g, err, fp);
    vg.width = fio::znz_read_int(g, err, fp);
    vg.height = fio::znz_read_int(g, err, fp);
    vg.depth = fio::znz_read_int(g, err, fp);
    let mut f = || fio::znz_read_float(g, err, fp);
    vg.xsize = f();
    vg.ysize = f();
    vg.zsize = f();
    vg.x_r = f();
    vg.x_a = f();
    vg.x_s = f();
    vg.y_r = f();
    vg.y_a = f();
    vg.y_s = f();
    vg.z_r = f();
    vg.z_a = f();
    vg.z_s = f();
    vg.c_r = f();
    vg.c_a = f();
    vg.c_s = f();
    if !shearless {
        vg.s_r = f();
        vg.s_a = f();
        vg.s_s = f();
    }
    let (want, remaining) = if !niftiheaderext {
        vg.fname.memset0(GEOM_NAME);
        (GEOM_NAME as i64, 0)
    } else {
        let len = fio::znz_read_int(g, err, fp);
        let to_read = i64::from(len).min(GEOM_NAME as i64);
        (to_read, i64::from(len) - to_read)
    };
    let mut got = 0usize;
    if want > 0 {
        let n = want as usize;
        let mut b = vec![0u8; n];
        let before = fp.tell();
        let r = fio::fs_znzread(fp, &mut b, 1, n);
        let after = fp.tell();
        got = if before >= 0 && after >= before {
            ((after - before) as usize).min(n)
        } else {
            r.clamp(0, n as i64) as usize
        };
        vg.fname.buffer_mut()[..got].copy_from_slice(&b[..got]);
        if remaining > 0 {
            if remaining > STACK_NOTE {
                g.notices.add(
                    &FS_185,
                    format!("VOL_GEOM::read: a name of {} bytes: its last {remaining} skipped without FreeSurfer's stack array of that size", want + remaining),
                );
            }
            skip_geom_name_rest(fp, remaining);
        }
    }
    // FS-186: where the name read is not ended within what the file gave
    let read_ends = vg.fname.buffer()[..got].contains(&0);
    if !read_ends && got < STRLEN && vg.fname.buffer()[got] != 0 {
        let before = vg.fname.to_string_lossy();
        if g.notices.reproduce() {
            g.notices.add(
                &FS_186,
                format!("VOL_GEOM::read: the name read ({got} bytes) runs on into the earlier name's bytes ({before:?})"),
            );
        } else {
            vg.fname.buffer_mut()[got] = 0;
            g.notices.add(
                &FS_186,
                format!("VOL_GEOM::read: the name read ({got} bytes) ended after them (FreeSurfer's runs on into the earlier name's: {before:?})"),
            );
        }
    }
}

/// The rest of an extension's name longer than 512 bytes, read and dropped (FS-185).
fn skip_geom_name_rest(fp: &mut ZnzFile, remaining: i64) {
    let mut chunk = vec![0u8; 1 << 16];
    let mut left = remaining.max(0) as u64;
    while left > 0 {
        let k = left.min(chunk.len() as u64) as usize;
        let before = fp.tell();
        fio::fs_znzread(fp, &mut chunk[..k], 1, k);
        let after = fp.tell();
        let r = if before >= 0 && after >= before {
            (after - before) as u64
        } else {
            0
        };
        left -= r.min(left);
        if (r as usize) < k {
            break;
        }
    }
}

/// `FStagsIO`: the tags' writer and reader on `fp` (`niftiheaderext`: FreeSurfer's NIfTI header
/// extension's forms).
pub struct FsTagsIo<'a> {
    pub fp: &'a mut ZnzFile,
    pub niftiheaderext: bool,
}

impl<'a> FsTagsIo<'a> {
    pub fn new(fp: &'a mut ZnzFile, niftiheaderext: bool) -> FsTagsIo<'a> {
        FsTagsIo { fp, niftiheaderext }
    }

    fn info(g: &Globals) -> bool {
        g.gdiag & DIAG_INFO != 0
    }

    fn start(&mut self, g: &Globals) -> i64 {
        if Self::info(g) { self.fp.tell() } else { 0 }
    }

    /// The writers' closing line under `DIAG_INFO`.
    fn done(&mut self, g: &Globals, out: &mut dyn Write, who: &str, tag: i32, fstart: i64) {
        if Self::info(g) {
            let fend = self.fp.tell();
            let _ = writeln!(
                out,
                "[DEBUG] FStagsIO::{who}() TAG = {tag:<4}, dlen = {:<6} ({fstart:<6} - {fend:<6})",
                fend - fstart
            );
        }
    }

    /// `write_tag(tag, data, dlen)`: the tag number, the length (unless the tag has none in an
    /// `.mgz`), `dlen` bytes of `data`. `NO_ERROR`.
    pub fn write_tag(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        tag: i32,
        data: &[u8],
        dlen: i64,
    ) -> i32 {
        let fstart = self.start(g);
        fio::znz_write_int(self.fp, tag);
        if self.niftiheaderext || !lengthless(tag) {
            fio::znz_write_long(g, self.fp, dlen);
        }
        let n = (dlen.max(0) as u64).min(data.len() as u64) as usize;
        fio::fs_znzwrite(self.fp, &data[..n], 1, n);
        let fend = self.fp.tell();
        if Self::info(g) {
            let _ = writeln!(
                out,
                "[DEBUG] FStagsIO::write_tag() TAG = {tag:<4}, dlen = {:<6} ({fstart:<6} - {fend:<6}) ({dlen:<6})",
                fend - fstart
            );
        }
        NO_ERROR
    }

    /// `write_matrix(M, tag)`: the extension's 16 floats, or an `.mgz`'s tag of the text `Matrix
    /// %10lf ...` in 1,600 bytes.
    pub fn write_matrix(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        m: &Matrix,
        tag: i32,
    ) -> i32 {
        if Self::info(g) {
            let _ = out.write_all(b"[DEBUG] FStagsIO::write_matrix()\n");
            print_matrix(g, out, err, Some(m));
        }
        if self.niftiheaderext {
            return self.write_matrix_niftiheaderext(g, out, m, tag);
        }
        let text = tags::matrix_text(g, m, "Matrix", "FStagsIO::write_matrix");
        let mut buf = vec![0u8; MATRIX_STRLEN];
        buf[..text.len()].copy_from_slice(&text);
        if Self::info(g) {
            let _ = writeln!(
                out,
                "[DEBUG] FStagsIO::write_matrix() TAG = {tag:<4}, len = {:<6}",
                MATRIX_STRLEN
            );
            let _ = out.write_all(b"[DEBUG] FStagsIO::write_matrix() ");
            let _ = out.write_all(&text);
            let _ = out.write_all(b"\n");
        }
        self.write_tag(g, out, tag, &buf, MATRIX_STRLEN as i64)
    }

    fn write_matrix_niftiheaderext(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        m: &Matrix,
        tag: i32,
    ) -> i32 {
        let fstart = self.start(g);
        if tag > 0 {
            fio::znz_write_int(self.fp, tag);
            fio::znz_write_long(g, self.fp, getlen_matrix(true, false));
        }
        for r in 1..=4 {
            for c in 1..=4 {
                let v = tags::elt44(g, m, r, c, "FStagsIO::write_matrix");
                fio::znz_write_float(self.fp, v);
            }
        }
        self.done(g, out, "__write_matrix_niftiheaderext", tag, fstart);
        NO_ERROR
    }

    /// `write_old_colortable(ctab)`: `TAG_OLD_COLORTABLE` (with its length in the extension) and the
    /// binary table. `NO_ERROR`.
    pub fn write_old_colortable(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        ctab: &ColorTable,
    ) -> i32 {
        let fstart = self.start(g);
        fio::znz_write_int(self.fp, TAG_OLD_COLORTABLE);
        if self.niftiheaderext {
            let d = getlen_old_colortable(ctab, true, false);
            fio::znz_write_long(g, self.fp, d);
        }
        colortab_bin::znz_ctab_write_into_binary(g, out, err, Some(ctab), Some(&mut *self.fp));
        self.done(g, out, "write_old_colortable", TAG_OLD_COLORTABLE, fstart);
        NO_ERROR
    }

    /// `write_mri_frames(mri)`: `TAG_MRI_FRAME` with the length `10 * nframes * sizeof(MRI_FRAME)`
    /// and each frame's fields (its matrix as a tag 0 of text; the diffusion parameters for a
    /// diffusion frame), zeros to the length; the extension's label, name and threshold.
    pub fn write_mri_frames(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        mri: &Mri,
    ) -> Result<i32, Fatal> {
        if self.niftiheaderext {
            return Ok(self.write_mri_frames_niftiheaderext(g, out, mri));
        }
        let fstart = self.start(g);
        let mut len = getlen_mri_frames(mri, false, false);
        fio::znz_write_int(self.fp, TAG_MRI_FRAME);
        fio::znz_write_long(g, self.fp, len);
        let here = self.fp.tell();
        for fr in mri.frames.iter().take(mri.nframes.max(0) as usize) {
            self.write_frame(g, out, err, fr);
        }
        let fend = self.fp.tell();
        len -= fend - here;
        if len > 0 {
            let zeros = vec![0u8; len as usize];
            fio::fs_znzwrite(self.fp, &zeros, len as usize, 1);
        }
        if Self::info(g) {
            let fend = self.fp.tell();
            let _ = writeln!(
                out,
                "[DEBUG] FStagsIO::write_mri_frames() TAG = {TAG_MRI_FRAME:<4}, dlen = {:<6} ({fstart:<6} - {fend:<6})",
                fend - fstart
            );
        }
        Ok(NO_ERROR)
    }

    fn write_frame(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        fr: &MriFrame,
    ) {
        let fp = &mut *self.fp;
        fio::znz_write_int(fp, fr.ftype);
        for v in [fr.te, fr.tr, fr.flip, fr.ti, fr.td, fr.tm as f32] {
            fio::znz_write_float(fp, v);
        }
        fio::znz_write_int(fp, fr.sequence_type);
        fio::znz_write_float(fp, fr.echo_spacing);
        fio::znz_write_float(fp, fr.echo_train_len);
        for v in fr.read_dir.iter().chain(&fr.pe_dir).chain(&fr.slice_dir) {
            fio::znz_write_float(fp, *v);
        }
        fio::znz_write_int(fp, fr.label);
        fio::fs_znzwrite(fp, fr.name.buffer(), 1, STRLEN);
        fio::znz_write_int(fp, fr.dof);
        match &fr.m_ras2vox {
            Some(m) if m.rows > 0 => {
                self.write_matrix(g, out, err, m, 0);
            }
            _ => {
                let m = tags::m44(g);
                self.write_matrix(g, out, err, &m, 0);
            }
        }
        let fp = &mut *self.fp;
        fio::znz_write_float(fp, fr.thresh);
        fio::znz_write_int(fp, fr.units);
        if fr.ftype == FRAME_TYPE_DIFFUSION_AUGMENTED {
            for v in [fr.dx, fr.dy, fr.dz, fr.dr, fr.dp, fr.ds, fr.bvalue, fr.tm] {
                fio::znz_write_double(fp, v);
            }
            for (ramp, flat, amp) in [
                (fr.d1_ramp, fr.d1_flat, fr.d1_amp),
                (fr.d2_ramp, fr.d2_flat, fr.d2_amp),
                (fr.d3_ramp, fr.d3_flat, fr.d3_amp),
                (fr.d4_ramp, fr.d4_flat, fr.d4_amp),
            ] {
                fio::znz_write_long(g, fp, ramp);
                fio::znz_write_long(g, fp, flat);
                fio::znz_write_double(fp, amp);
            }
        }
    }

    fn write_mri_frames_niftiheaderext(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        mri: &Mri,
    ) -> i32 {
        let fstart = self.start(g);
        fio::znz_write_int(self.fp, TAG_MRI_FRAME);
        let d = getlen_mri_frames(mri, true, false);
        fio::znz_write_long(g, self.fp, d);
        for fr in mri.frames.iter().take(mri.nframes.max(0) as usize) {
            fio::znz_write_int(self.fp, fr.label);
            fio::fs_znzwrite(self.fp, fr.name.buffer(), 1, STRLEN);
            fio::znz_write_float(self.fp, fr.thresh);
        }
        self.done(
            g,
            out,
            "__write_mri_frames_niftiheaderext",
            TAG_MRI_FRAME,
            fstart,
        );
        NO_ERROR
    }

    /// `write_gcamorph_geom(source, target, shearless)`: `TAG_GCAMORPH_GEOM` (no length in an
    /// `.mgz`) or `TAG_GCAMORPH_GEOM_PLUSSHEAR` (with one), then copies of the two geometries
    /// (made shearless first when `shearless`), each by [`vol_geom_write`].
    pub fn write_gcamorph_geom(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        source: &VolGeom,
        target: &VolGeom,
        shearless: bool,
    ) -> Result<i32, Fatal> {
        let fstart = self.start(g);
        let tag = if shearless {
            TAG_GCAMORPH_GEOM
        } else {
            TAG_GCAMORPH_GEOM_PLUSSHEAR
        };
        fio::znz_write_int(self.fp, tag);
        if self.niftiheaderext || !shearless {
            let d = getlen_gcamorph_geom(
                g,
                source.fname.c_bytes(),
                target.fname.c_bytes(),
                self.niftiheaderext,
                false,
                shearless,
            );
            fio::znz_write_long(g, self.fp, d);
        }
        let mut src = source.clone();
        let mut trg = target.clone();
        if shearless {
            vol_geom::shearless_components(g, out, err, &mut src)?;
            vol_geom::shearless_components(g, out, err, &mut trg)?;
        }
        vol_geom_write(self.fp, &src, self.niftiheaderext, shearless);
        vol_geom_write(self.fp, &trg, self.niftiheaderext, shearless);
        if Self::info(g) {
            vol_geom::vgprint(out, source, false);
            vol_geom::vgprint(out, target, false);
            let fend = self.fp.tell();
            let _ = writeln!(
                out,
                "[DEBUG] FStagsIO::write_gcamorph_geom() TAG = {tag:<4}, dlen = {:<6} ({fstart:<6} - {fend:<6})",
                fend - fstart
            );
            vol_geom::vgprint(out, &src, false);
            vol_geom::vgprint(out, &trg, false);
        }
        Ok(NO_ERROR)
    }

    /// `write_gcamorph_meta(warpFieldFormat, gcamorphSpacing, gcamorphExp_k)`: `TAG_GCAMORPH_META`,
    /// length 12, two `int`s and the exponent as a `float`.
    pub fn write_gcamorph_meta(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        fmt: i32,
        spacing: i32,
        exp_k: f64,
    ) -> i32 {
        let fstart = self.start(g);
        fio::znz_write_int(self.fp, TAG_GCAMORPH_META);
        fio::znz_write_long(g, self.fp, getlen_gcamorph_meta(false));
        fio::znz_write_int(self.fp, fmt);
        fio::znz_write_int(self.fp, spacing);
        fio::znz_write_float(self.fp, exp_k as f32);
        self.done(g, out, "write_gcamorph_meta", TAG_GCAMORPH_META, fstart);
        NO_ERROR
    }

    /// `write_gcamorph_labels(x0, y0, z0, labels)`: `TAG_GCAMORPH_LABELS` (with a length in the
    /// extension) and the labels `labels[(x * y0 + y) * z0 + z]`, `z` fastest.
    pub fn write_gcamorph_labels(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        x0: i32,
        y0: i32,
        z0: i32,
        labels: &[i32],
    ) -> i32 {
        let fstart = self.start(g);
        fio::znz_write_int(self.fp, TAG_GCAMORPH_LABELS);
        if self.niftiheaderext {
            let d = getlen_gcamorph_labels(g, x0, y0, z0, 4, true, false);
            fio::znz_write_long(g, self.fp, d);
        }
        let n = (x0.max(0) as usize) * (y0.max(0) as usize) * (z0.max(0) as usize);
        for v in labels.iter().take(n) {
            fio::znz_write_int(self.fp, *v);
        }
        self.done(g, out, "write_gcamorph_labels", TAG_GCAMORPH_LABELS, fstart);
        NO_ERROR
    }

    /// `write_dof(dof)`: `TAG_DOF`, length 4, the number.
    pub fn write_dof(&mut self, g: &mut Globals, out: &mut dyn Write, dof: i32) -> i32 {
        let fstart = self.start(g);
        fio::znz_write_int(self.fp, TAG_DOF);
        fio::znz_write_long(g, self.fp, getlen_dof(false));
        fio::znz_write_int(self.fp, dof);
        self.done(g, out, "write_dof", TAG_DOF, fstart);
        NO_ERROR
    }

    /// `write_scan_parameters(mri)`: `TAG_SCAN_PARAMETERS`, its length, TE and TI (`float`s), the
    /// flip angle (`double`), the field strength (`float`) and the phase-encoding direction with
    /// its NUL (`UNKNOWN` without one when there is none).
    pub fn write_scan_parameters(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        mri: &Mri,
    ) -> i32 {
        let fstart = self.start(g);
        fio::znz_write_int(self.fp, TAG_SCAN_PARAMETERS);
        fio::znz_write_long(g, self.fp, getlen_scan_parameters(mri, false));
        fio::znz_write_float(self.fp, mri.te);
        fio::znz_write_float(self.fp, mri.ti);
        fio::znz_write_double(self.fp, mri.flip_angle);
        fio::znz_write_float(self.fp, mri.field_strength);
        match &mri.pedir {
            Some(p) => {
                let mut b = p[..clen(p)].to_vec();
                b.push(0);
                fio::fs_znzwrite(self.fp, &b, 1, b.len());
            }
            None => {
                fio::fs_znzwrite(self.fp, b"UNKNOWN", 1, 7);
            }
        }
        self.done(g, out, "write_scan_parameters", TAG_SCAN_PARAMETERS, fstart);
        NO_ERROR
    }

    /// `write_ras_xform(vg)`: `TAG_RAS_XFORM`, length 48, the direction cosines and centre.
    pub fn write_ras_xform(&mut self, g: &mut Globals, out: &mut dyn Write, vg: &VolGeom) -> i32 {
        let fstart = self.start(g);
        fio::znz_write_int(self.fp, TAG_RAS_XFORM);
        fio::znz_write_long(g, self.fp, getlen_ras_xform(false));
        for v in [
            vg.x_r, vg.x_a, vg.x_s, vg.y_r, vg.y_a, vg.y_s, vg.z_r, vg.z_a, vg.z_s, vg.c_r, vg.c_a,
            vg.c_s,
        ] {
            fio::znz_write_float(self.fp, v);
        }
        if Self::info(g) {
            vol_geom::geomprint(
                out,
                vg,
                Some("[DEBUG] FStagsIO::write_ras_xform() ras xform info:\n"),
            );
        }
        self.done(g, out, "write_ras_xform", TAG_RAS_XFORM, fstart);
        NO_ERROR
    }

    /// `write_ras_xform_extra(vg)`: `TAG_RAS_XFORM_EXTRA`, length 36, the shears, extent and voxel
    /// sizes.
    pub fn write_ras_xform_extra(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        vg: &VolGeom,
    ) -> i32 {
        let fstart = self.start(g);
        fio::znz_write_int(self.fp, TAG_RAS_XFORM_EXTRA);
        fio::znz_write_long(g, self.fp, getlen_ras_xform_extra(false));
        for v in [vg.s_r, vg.s_a, vg.s_s] {
            fio::znz_write_float(self.fp, v);
        }
        for v in [vg.width, vg.height, vg.depth] {
            fio::znz_write_int(self.fp, v);
        }
        for v in [vg.xsize, vg.ysize, vg.zsize] {
            fio::znz_write_float(self.fp, v);
        }
        if Self::info(g) {
            vol_geom::geomprint(
                out,
                vg,
                Some("[DEBUG] FStagsIO::write_ras_xform_extra() ras xform info:\n"),
            );
        }
        self.done(g, out, "write_ras_xform_extra", TAG_RAS_XFORM_EXTRA, fstart);
        NO_ERROR
    }

    /// `write_endtag()`: `TAG_END_NIIHDREXTENSION` (-1), length 1, `*` (so that readers that drop
    /// trailing zeros keep the data).
    pub fn write_endtag(&mut self, g: &mut Globals, out: &mut dyn Write) -> i32 {
        let fstart = self.start(g);
        fio::znz_write_int(self.fp, TAG_END_NIIHDREXTENSION);
        fio::znz_write_long(g, self.fp, getlen_endtag(false));
        fio::fs_znzwrite(self.fp, b"*", 1, 1);
        self.done(g, out, "write_endtag", TAG_END_NIIHDREXTENSION, fstart);
        NO_ERROR
    }

    /// `read_tagid_len(&len, tagwithzerolen)`: 0 at the end of the file before or after the tag
    /// number (`len` untouched); the extension's every tag with its 64-bit length; otherwise as
    /// [`tags::znz_tag_read_start`].
    pub fn read_tagid_len(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        plen: &mut i64,
        tagwithzerolen: i32,
    ) -> i32 {
        if self.fp.eof() {
            return 0;
        }
        // a number cut by the end of the file is not used: no memory contents reach a result.
        // FS-167: every znzreadInt and znzreadLong here reads into the same memory (the official
        // build's), so a length the file cuts keeps the tag's bytes
        let mut slot = fio::Slot49::new();
        let before = g.notices.clone();
        let tag = fio::znz_read_int_in(g, err, self.fp, &mut slot);
        if self.fp.eof() {
            g.notices = before;
            return 0;
        }
        if self.niftiheaderext {
            *plen = fio::znz_read_long_in(g, err, self.fp, &mut slot);
            return tag;
        }
        if tagwithzerolen != 0 && tagwithzerolen == tag {
            *plen = 0;
            return tag;
        }
        match tag {
            TAG_OLD_MGH_XFORM => {
                *plen = i64::from(fio::znz_read_int_in(g, err, self.fp, &mut slot)) - 1
            }
            TAG_OLD_SURF_GEOM | TAG_OLD_USEREALRAS | TAG_OLD_COLORTABLE | TAG_GCAMORPH_GEOM
            | TAG_GCAMORPH_TYPE | TAG_GCAMORPH_LABELS => *plen = 0,
            _ => *plen = fio::znz_read_long_in(g, err, self.fp, &mut slot),
        }
        tag
    }

    /// `read_data(buf, len)`: `len` bytes into `buf` (bytes beyond it read and dropped). `NO_ERROR`.
    pub fn read_data(&mut self, buf: &mut [u8], len: i64) -> i32 {
        let want = if len < 0 { 0 } else { len as u64 };
        let n = (buf.len() as u64).min(want) as usize;
        let before = self.fp.tell();
        fio::fs_znzread(self.fp, &mut buf[..n], 1, n);
        let after = self.fp.tell();
        let got = if before >= 0 && after >= before {
            (after - before) as u64
        } else {
            0
        };
        if got == n as u64 && want > n as u64 {
            skip_geom_name_rest(self.fp, (want - n as u64) as i64);
        }
        NO_ERROR
    }

    /// `read_matrix()`: the extension's 16 floats, or 1,600 bytes of an `.mgz`'s text read as
    /// [`tags::scan_matrix_text`] (FS-179, FS-180) into a 4 x 4 matrix of zeros.
    pub fn read_matrix(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Matrix {
        let mut m = tags::m44(g);
        if self.niftiheaderext {
            for r in 1..=4 {
                for c in 1..=4 {
                    let v = fio::znz_read_float(g, err, self.fp);
                    m.set_relt(r, c, v);
                }
            }
        } else {
            let buf = tags::read_matrix_bytes(self.fp);
            tags::scan_matrix_text(g, &buf, &mut m, "FStagsIO::read_matrix");
        }
        if Self::info(g) {
            let _ = out.write_all(b"[DEBUG] FStagsIO::read_matrix()\n");
            print_matrix(g, out, err, Some(&m));
        }
        m
    }

    /// `read_old_colortable()`: [`colortab_bin::znz_ctab_read_from_binary`].
    pub fn read_old_colortable(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Option<ColorTable> {
        colortab_bin::znz_ctab_read_from_binary(g, out, err, Some(&mut *self.fp))
    }

    /// `read_mri_frames(mri, len)`: the `nframes` frames [`FsTagsIo::write_mri_frames`] writes (each
    /// frame's name over its buffer, its matrix tag read by [`FsTagsIo::read_tagid_len`] and
    /// [`FsTagsIo::read_matrix`]), then what is left of `len` skipped (FS-178).
    pub fn read_mri_frames(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        mri: &mut Mri,
        len: i64,
    ) -> i32 {
        let ext = self.niftiheaderext;
        let fstart = self.fp.tell();
        let n = mri.nframes.max(0) as usize;
        for fno in 0..n.min(mri.frames.len()) {
            if ext {
                let label = fio::znz_read_int(g, err, self.fp);
                let fr = &mut mri.frames[fno];
                fr.label = label;
                read_name_buffer(self.fp, fr);
                fr.thresh = fio::znz_read_float(g, err, self.fp);
                continue;
            }
            let mut fr = std::mem::take(&mut mri.frames[fno]);
            self.read_frame(g, out, err, &mut fr);
            mri.frames[fno] = fr;
        }
        let fend = self.fp.tell();
        let left = len - (fend - fstart);
        if left > 0 {
            if Self::info(g) {
                let who = if ext {
                    "__read_mri_frames_niftiheaderext"
                } else {
                    "read_mri_frames"
                };
                let _ = writeln!(
                    out,
                    "[DEBUG] {who}() TAG = {TAG_MRI_FRAME:<4}, bytes_read = {:<6} ({fstart:<6} - {fend:<6}), skip extra bytes {left}",
                    fend - fstart
                );
            }
            let fp = &mut *self.fp;
            let _ = tags::skip_by_alloc(
                g,
                err,
                left,
                |s| s,
                |b| {
                    let before = fp.tell();
                    fio::fs_znzread(fp, b, b.len(), 1);
                    let after = fp.tell();
                    if before >= 0 && after >= before {
                        (after - before) as usize
                    } else {
                        0
                    }
                },
            );
        }
        NO_ERROR
    }

    fn read_frame(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        fr: &mut MriFrame,
    ) {
        {
            let fp = &mut *self.fp;
            fr.ftype = fio::znz_read_int(g, err, fp);
            fr.te = fio::znz_read_float(g, err, fp);
            fr.tr = fio::znz_read_float(g, err, fp);
            fr.flip = fio::znz_read_float(g, err, fp);
            fr.ti = fio::znz_read_float(g, err, fp);
            fr.td = fio::znz_read_float(g, err, fp);
            fr.tm = f64::from(fio::znz_read_float(g, err, fp));
            fr.sequence_type = fio::znz_read_int(g, err, fp);
            fr.echo_spacing = fio::znz_read_float(g, err, fp);
            fr.echo_train_len = fio::znz_read_float(g, err, fp);
            for i in 0..3 {
                fr.read_dir[i] = fio::znz_read_float(g, err, fp);
            }
            for i in 0..3 {
                fr.pe_dir[i] = fio::znz_read_float(g, err, fp);
            }
            for i in 0..3 {
                fr.slice_dir[i] = fio::znz_read_float(g, err, fp);
            }
            fr.label = fio::znz_read_int(g, err, fp);
            read_name_buffer(fp, fr);
            fr.dof = fio::znz_read_int(g, err, fp);
        }
        let mut matlen = 0i64;
        let mattag = self.read_tagid_len(g, err, &mut matlen, 0);
        let m = self.read_matrix(g, out, err);
        if Self::info(g) {
            let _ = writeln!(
                out,
                "[DEBUG] FStagsIO::read_mri_frame() TAG = {mattag:<4}, len = {matlen:<6}"
            );
            print_matrix(g, out, err, Some(&m));
        }
        fr.m_ras2vox = Some(m);
        let fp = &mut *self.fp;
        fr.thresh = fio::znz_read_float(g, err, fp);
        fr.units = fio::znz_read_int(g, err, fp);
        if fr.ftype == FRAME_TYPE_DIFFUSION_AUGMENTED {
            fr.dx = fio::znz_read_double(g, err, fp);
            fr.dy = fio::znz_read_double(g, err, fp);
            fr.dz = fio::znz_read_double(g, err, fp);
            fr.dr = fio::znz_read_double(g, err, fp);
            fr.dp = fio::znz_read_double(g, err, fp);
            fr.ds = fio::znz_read_double(g, err, fp);
            fr.bvalue = fio::znz_read_double(g, err, fp);
            fr.tm = fio::znz_read_double(g, err, fp);
            fr.d1_ramp = fio::znz_read_long(g, err, fp);
            fr.d1_flat = fio::znz_read_long(g, err, fp);
            fr.d1_amp = fio::znz_read_double(g, err, fp);
            fr.d2_ramp = fio::znz_read_long(g, err, fp);
            fr.d2_flat = fio::znz_read_long(g, err, fp);
            fr.d2_amp = fio::znz_read_double(g, err, fp);
            fr.d3_ramp = fio::znz_read_long(g, err, fp);
            fr.d3_flat = fio::znz_read_long(g, err, fp);
            fr.d3_amp = fio::znz_read_double(g, err, fp);
            fr.d4_ramp = fio::znz_read_long(g, err, fp);
            fr.d4_flat = fio::znz_read_long(g, err, fp);
            fr.d4_amp = fio::znz_read_double(g, err, fp);
        }
    }

    /// `read_gcamorph_geom(source, target, shearless)`: the two geometries by [`vol_geom_read`].
    /// `NO_ERROR`.
    pub fn read_gcamorph_geom(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        source: &mut VolGeom,
        target: &mut VolGeom,
        shearless: bool,
    ) -> i32 {
        let ext = self.niftiheaderext;
        vol_geom_read(g, err, self.fp, source, ext, shearless);
        vol_geom_read(g, err, self.fp, target, ext, shearless);
        NO_ERROR
    }

    /// `read_gcamorph_meta(...)`: two `int`s and a `float` (as a `double`).
    pub fn read_gcamorph_meta(&mut self, g: &mut Globals, err: &mut dyn Write) -> (i32, i32, f64) {
        let a = fio::znz_read_int(g, err, self.fp);
        let b = fio::znz_read_int(g, err, self.fp);
        let c = f64::from(fio::znz_read_float(g, err, self.fp));
        (a, b, c)
    }

    /// `read_gcamorph_labels(x0, y0, z0, labels)`: `x0 * y0 * z0` `int`s into
    /// `labels[(x * y0 + y) * z0 + z]` (values beyond `labels` read and dropped).
    pub fn read_gcamorph_labels(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        x0: i32,
        y0: i32,
        z0: i32,
        labels: &mut [i32],
    ) -> i32 {
        let n = (x0.max(0) as usize) * (y0.max(0) as usize) * (z0.max(0) as usize);
        for i in 0..n {
            let v = fio::znz_read_int(g, err, self.fp);
            if let Some(p) = labels.get_mut(i) {
                *p = v;
            }
        }
        NO_ERROR
    }

    /// `read_dof(&dof)`.
    pub fn read_dof(&mut self, g: &mut Globals, err: &mut dyn Write) -> i32 {
        fio::znz_read_int(g, err, self.fp)
    }

    /// `read_scan_parameters(mri, dlen)`: TE, TI, the flip angle, the field strength, then the
    /// direction from the `dlen - 20` bytes that follow (a C string: up to its first NUL). FS-183:
    /// a `dlen` below 20 (or beyond what the machine can allocate) makes FreeSurfer read into an
    /// allocation of the wrong size or into none; here the direction is empty (below 20) or the
    /// bytes the file holds.
    pub fn read_scan_parameters(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        mri: &mut Mri,
        dlen: i64,
    ) -> i32 {
        mri.te = fio::znz_read_float(g, err, self.fp);
        mri.ti = fio::znz_read_float(g, err, self.fp);
        mri.flip_angle = fio::znz_read_double(g, err, self.fp);
        mri.field_strength = fio::znz_read_float(g, err, self.fp);
        let d = dlen.wrapping_sub(20);
        let mut kept = Vec::new();
        let mut bytesread: i64 = 0;
        if d < 0 {
            g.notices.add(
                &FS_183,
                format!("read_scan_parameters: a tag length of {dlen}, below the parameters' 20 bytes: no direction read"),
            );
        } else {
            let mut chunk = vec![0u8; (d as usize).min(1 << 16)];
            let mut left = d as u64;
            while left > 0 {
                let k = left.min(chunk.len() as u64) as usize;
                let before = self.fp.tell();
                let r = fio::fs_znzread(self.fp, &mut chunk[..k], 1, k);
                let after = self.fp.tell();
                let got = if before >= 0 && after >= before {
                    ((after - before) as usize).min(k)
                } else {
                    r.clamp(0, k as i64) as usize
                };
                kept.extend_from_slice(&chunk[..got]);
                bytesread += got as i64;
                left -= got as u64;
                if got < k {
                    break;
                }
            }
            if (bytesread as u64) < d as u64 && d > (1 << 24) {
                g.notices.add(
                    &FS_183,
                    format!("read_scan_parameters: a direction of {d} bytes ({bytesread} in the file): FreeSurfer allocates the length first"),
                );
            }
        }
        if Self::info(g) {
            let _ = writeln!(
                out,
                "[DEBUG] read_scan_parameters(): bytesread={bytesread}, dlen={d}"
            );
        }
        kept.truncate(clen(&kept));
        mri.pedir = Some(kept);
        NO_ERROR
    }

    /// `read_ras_xform(vg)`: the 12 floats [`FsTagsIo::write_ras_xform`] writes.
    pub fn read_ras_xform(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        vg: &mut VolGeom,
    ) -> i32 {
        let mut f = || fio::znz_read_float(g, err, self.fp);
        vg.x_r = f();
        vg.x_a = f();
        vg.x_s = f();
        vg.y_r = f();
        vg.y_a = f();
        vg.y_s = f();
        vg.z_r = f();
        vg.z_a = f();
        vg.z_s = f();
        vg.c_r = f();
        vg.c_a = f();
        vg.c_s = f();
        if Self::info(g) {
            vol_geom::geomprint(
                out,
                vg,
                Some("[DEBUG] FStagsIO::read_ras_xform(): from FS header extension:\n"),
            );
        }
        NO_ERROR
    }

    /// `read_ras_xform_extra(vg)`: the shears, extent and voxel sizes.
    pub fn read_ras_xform_extra(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        vg: &mut VolGeom,
    ) -> i32 {
        vg.s_r = fio::znz_read_float(g, err, self.fp);
        vg.s_a = fio::znz_read_float(g, err, self.fp);
        vg.s_s = fio::znz_read_float(g, err, self.fp);
        vg.width = fio::znz_read_int(g, err, self.fp);
        vg.height = fio::znz_read_int(g, err, self.fp);
        vg.depth = fio::znz_read_int(g, err, self.fp);
        vg.xsize = fio::znz_read_float(g, err, self.fp);
        vg.ysize = fio::znz_read_float(g, err, self.fp);
        vg.zsize = fio::znz_read_float(g, err, self.fp);
        if Self::info(g) {
            vol_geom::geomprint(
                out,
                vg,
                Some("[DEBUG] FStagsIO::read_ras_xform_extra(): from FS header extension:\n"),
            );
        }
        NO_ERROR
    }

    /// `skip_tag(tag, len)`: `len` bytes read and dropped; the count (`int`), or the end of the
    /// program after `FStagsIO::skip_tag(): tag=<tag>, failed to calloc <len> bytes!` (FS-177,
    /// FS-178).
    pub fn skip_tag(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        tag: i32,
        len: i64,
    ) -> Result<i32, Fatal> {
        let fp = &mut *self.fp;
        tags::skip_by_alloc(
            g,
            err,
            len,
            |n| format!("FStagsIO::skip_tag(): tag={tag:<4}, failed to calloc {n} bytes!\n"),
            |b| {
                let before = fp.tell();
                let r = fio::fs_znzread(fp, b, 1, b.len());
                let after = fp.tell();
                if before >= 0 && after >= before {
                    ((after - before) as usize).min(b.len())
                } else {
                    r.clamp(0, b.len() as i64) as usize
                }
            },
        )
    }
}

/// `znzread(frame->name, 1, STRLEN, fp)`: the bytes the file gives over the frame's name buffer (the
/// rest of the buffer as it was).
fn read_name_buffer(fp: &mut ZnzFile, fr: &mut MriFrame) {
    let mut b = vec![0u8; STRLEN];
    let before = fp.tell();
    let r = fio::fs_znzread(fp, &mut b, 1, STRLEN);
    let after = fp.tell();
    let got = if before >= 0 && after >= before {
        ((after - before) as usize).min(STRLEN)
    } else {
        r.clamp(0, STRLEN as i64) as usize
    };
    fr.name.buffer_mut()[..got].copy_from_slice(&b[..got]);
}
