// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! GE Signa's header as FreeSurfer 8.2.0 reads it (`utils/signa.cpp`, taken early from queue
//! item 3.1-30 for `mri_identify`): `ghi` (a 16-bit integer, FS-225), `ghf` (a Data General float
//! converted to IEEE, GE's `mvtsunf`), `ghc`, `get_signa_header_info` and `is_signa` (FS-224);
//! `signaRead` ([`signa_read`], relay 28 of track A: FS-686, FS-687, FS-684's messages), wired
//! into [`crate::mriio::mri_read`] (`SIGNA_FILE`).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_225, FS_249, FS_684, FS_686, FS_687, Notices};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_NOFILE, Fatal};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::{sse, utils};
use larmorx_freesurfer_mri::mri::{self, MRI_SHORT, Mri};

use crate::cfile::CFile;
use crate::mriio::{MRI_CORONAL, MRI_HORIZONTAL, MRI_SAGITTAL, set_direction_cosine};

/// The header's word offsets of its blocks (`signa.h`): series and image headers.
pub const SEHDR_START: usize = 8 * 256;
pub const IHDR_START: usize = 10 * 256;
/// The bytes of a Signa file's header (`HLENGTH`: 28 blocks of 256 words).
pub const HLENGTH: usize = 28 * 256 * 2;

pub const SEHDR_PTYPE: usize = 138;
pub const SEHDR_FOV: usize = 151;
pub const SEHDR_CENTER: usize = 153;
pub const SEHDR_C_R: usize = 153;
pub const SEHDR_C_A: usize = 155;
pub const SEHDR_C_S: usize = 157;
pub const SEHDR_ORIEN: usize = 159;
pub const SEHDR_POS: usize = 160;
pub const SEHDR_IALLOC: usize = 202;
pub const IHDR_STRTX: usize = 52;
pub const IHDR_ENDX: usize = 54;
pub const IHDR_STRTY: usize = 56;
pub const IHDR_ENDY: usize = 58;
pub const IHDR_STRTZ: usize = 60;
pub const IHDR_ENDZ: usize = 62;
pub const IHDR_LOCATN: usize = 73;
pub const IHDR_THICK: usize = 77;
pub const IHDR_TR: usize = 82;
pub const IHDR_TE: usize = 86;
pub const IHDR_TI: usize = 88;
pub const IHDR_NECHO: usize = 98;
pub const IHDR_X: usize = 137;
pub const IHDR_Y: usize = 138;
pub const IHDR_PIXSIZ: usize = 139;

/// The header's fields FreeSurfer keeps (`HINFO`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hinfo {
    pub imnr0: i32,
    pub imnr1: i32,
    pub x: i32,
    pub y: i32,
    pub ptype: i32,
    pub tr: f32,
    pub ti: f32,
    pub te: f32,
    pub strtx: f32,
    pub endx: f32,
    pub strty: f32,
    pub endy: f32,
    pub strtz: f32,
    pub endz: f32,
    pub locatn: f32,
    pub fov: f32,
    pub center: f32,
    pub psiz: f32,
    pub thick: f32,
    pub c_r: f32,
    pub c_a: f32,
    pub c_s: f32,
    pub orien: f32,
    pub pos: f32,
    pub plane_type: i32,
    pub num_echoes: i32,
}

/// The header's byte at `i` (0 past its end: the reads stay within `HLENGTH` for every field).
fn at(h: &[u8], i: usize) -> u8 {
    h.get(i).copied().unwrap_or(0)
}

/// `ghi(header, offset)`: the 16-bit big-endian integer at word `offset`. FreeSurfer combines the
/// two bytes as C `char`s, signed on x86-64, so a second byte of 0x80 or more makes the whole
/// value that byte minus 256 whatever the first byte (FS-225, rule A: by default the 16-bit
/// two's-complement value; `reproduce` FreeSurfer's).
pub fn ghi(g: &mut Globals, header: &[u8], offset: usize) -> i32 {
    let (b0, b1) = (at(header, 2 * offset), at(header, 2 * offset + 1));
    let freesurfer = ((i32::from(b0 as i8)) << 8) | i32::from(b1 as i8);
    let value = i32::from(i16::from_be_bytes([b0, b1]));
    if freesurfer != value {
        g.notices.add(
            &FS_225,
            format!(
                "a Signa header integer {b0:02x}{b1:02x} read as {} (FreeSurfer's {freesurfer})",
                if g.notices.reproduce() {
                    freesurfer
                } else {
                    value
                }
            ),
        );
        if g.notices.reproduce() {
            return freesurfer;
        }
    }
    value
}

/// `ghf(header, offset)`: the Data General float at word `offset` (4 bytes, big endian: sign,
/// 7-bit exponent of 16, 24-bit fraction) as an IEEE `float` by GE's conversion (the fraction
/// normalised, the exponent `4 (e - 64) + 126` less the shifts, clamped to 0..255), divided by
/// 1,000.
pub fn ghf(header: &[u8], offset: usize) -> f32 {
    let o = 2 * offset;
    let ii = i64::from(u32::from_be_bytes([
        at(header, o),
        at(header, o + 1),
        at(header, o + 2),
        at(header, o + 3),
    ]));
    let sign_bit: i64 = 0x8000_0000;
    let dg_exp = (ii >> 24) & 0o177;
    let dg_sign = ii & sign_bit;
    let mut dg_mantissa = (ii & 0o77777777) << 8;
    let mut sun_exp = 4 * (dg_exp - 64);
    while dg_mantissa & sign_bit == 0 && dg_mantissa != 0 {
        sun_exp -= 1;
        dg_mantissa <<= 1;
    }
    sun_exp += 126;
    sun_exp = sun_exp.clamp(0, 255);
    dg_mantissa <<= 1;
    let bits = dg_sign | (sun_exp << 23) | ((dg_mantissa >> 9) & 0o37777777);
    f32::from_bits(bits as u32) / 1000.0
}

/// `ghc(destin, header, offset, n)`: `strncpy` of `n` bytes from word `offset`: the bytes up to
/// the first zero, then zeros, `n` in all.
pub fn ghc(header: &[u8], offset: usize, n: usize) -> Vec<u8> {
    let mut d = vec![0u8; n];
    for (k, b) in d.iter_mut().enumerate() {
        let c = at(header, 2 * offset + k);
        if c == 0 {
            break;
        }
        *b = c;
    }
    d
}

/// `get_signa_header_info(h, &hinfo)`: the fields of a Signa header (`imnr0` 1).
pub fn get_signa_header_info(g: &mut Globals, h: &[u8]) -> Hinfo {
    Hinfo {
        plane_type: ghi(g, h, SEHDR_START + SEHDR_PTYPE),
        x: ghi(g, h, IHDR_START + IHDR_X),
        y: ghi(g, h, IHDR_START + IHDR_Y),
        tr: ghf(h, IHDR_START + IHDR_TR),
        ti: ghf(h, IHDR_START + IHDR_TI),
        te: ghf(h, IHDR_START + IHDR_TE),
        strtx: ghf(h, IHDR_START + IHDR_STRTX),
        endx: ghf(h, IHDR_START + IHDR_ENDX),
        strty: ghf(h, IHDR_START + IHDR_STRTY),
        endy: ghf(h, IHDR_START + IHDR_ENDY),
        strtz: ghf(h, IHDR_START + IHDR_STRTZ),
        endz: ghf(h, IHDR_START + IHDR_ENDZ),
        locatn: ghf(h, IHDR_START + IHDR_LOCATN),
        fov: ghf(h, SEHDR_START + SEHDR_FOV),
        center: ghf(h, SEHDR_START + SEHDR_CENTER),
        psiz: ghf(h, IHDR_START + IHDR_PIXSIZ),
        thick: ghf(h, IHDR_START + IHDR_THICK),
        ptype: ghi(g, h, SEHDR_START + SEHDR_PTYPE),
        imnr0: 1,
        imnr1: ghi(g, h, SEHDR_START + SEHDR_IALLOC),
        c_r: ghf(h, SEHDR_START + SEHDR_C_R),
        c_a: ghf(h, SEHDR_START + SEHDR_C_A),
        c_s: ghf(h, SEHDR_START + SEHDR_C_S),
        orien: ghf(h, SEHDR_START + SEHDR_ORIEN),
        pos: ghf(h, SEHDR_START + SEHDR_POS),
        num_echoes: ghi(g, h, IHDR_START + IHDR_NECHO),
    }
}

/// `is_signa(fname)`: 1 when the file's first `HLENGTH` (14,336) bytes are a plausible Signa
/// header: x and y in 3..10,239, TR in (0, 10^6), the field of view in (0.0002, 300), each axis's
/// start and end not both about 0 (the sums of their magnitudes in `float` above 10^-6), TI in
/// (0, 10^5), |TE| below 10^4, at least 0 images allocated and a finite pixel size; else 0. 0 when
/// the file does not open (`errno` as `fopen` left it) or is shorter: FreeSurfer then leaves the
/// file open (FS-224: closed in both modes; the descriptors FreeSurfer loses change no answer
/// until a program has lost about a thousand, and running out of them is not reproduced).
pub fn is_signa(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    let mut fp = match CFile::open(dir, fname, "rb") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            return 0;
        }
    };
    let mut h = vec![0u8; HLENGTH];
    if fp.fread(&mut h, 1, HLENGTH) != HLENGTH {
        // FS-224: FreeSurfer leaves the file open here (no answer changes: no notice)
        if let Some(e) = fp.read_errno() {
            g.errno = e;
        }
        fp.close();
        return 0;
    }
    let header = get_signa_header_info(g, &h);
    fp.close();
    let sum_above = |a: f32, b: f32| f64::from(a.abs() + b.abs()) > 0.000001;
    i32::from(
        header.x > 2
            && header.x < 1024 * 10
            && header.y > 2
            && header.y < 1024 * 10
            && header.tr > 0.0
            && header.tr < 1_000_000.0
            && f64::from(header.fov) > 0.0002
            && header.fov < 300.0
            && sum_above(header.strtx, header.endx)
            && sum_above(header.strtz, header.endy)
            && sum_above(header.strty, header.endz)
            && header.ti.abs() < 100_000.0
            && header.te.abs() < 10_000.0
            && header.ti > 0.0
            && header.imnr1 >= 0
            && utils::dev_finite(header.psiz) != 0,
    )
}

/// `SIGNA_AXIAL`, `SIGNA_SAGITTAL`: the series header's plane types (others are coronal).
pub const SIGNA_AXIAL: i32 = 0;
pub const SIGNA_SAGITTAL: i32 = 1;

/// FS-684: signaRead's messages name is_signa and read_signa.
fn names_note(g: &mut Globals) {
    g.notices.add(
        &FS_684,
        "signaRead: its messages name is_signa and read_signa, as FreeSurfer's do",
    );
}

/// `signaRead(fname, read_volume)`: a GE Signa series, its geometry from `fname`'s header and its
/// slices from `<fname's folder>/I.001` ... (`I.<imnr1>`).
///
/// `GE_ODD` (else `GE_EVEN`) set: `only using odd # GE Signa files` (`even`), the first and last
/// image numbers lowered (raised) to odd (even) ones and every other file read. `fname` that does
/// not open: `is_signa(<fname>): could not open file`; shorter than its 14,336-byte header:
/// `is_signa(<fname>): could not read 14336 byte header` (errors after the C library's text of
/// `errno`; FS-684: the names). The header's image count below 0: `WARNING: # of slices=<n> in
/// header - assuming 124...` (standard error). The volume: shorts of the header's x by y, its depth
/// the file count (with `GE_ODD`/`GE_EVEN` half the numbers' span plus 1); several echoes (the
/// image header's count above 1): `SIGNA multi-echo file detected (<n> echoes)`, frames the
/// echoes (FS-686). The start and end coordinates, location, field of view, pixel size and
/// thickness times 1,000 (`float`), TR, TI, TE as read, the direction cosines of the plane type
/// (axial horizontal, sagittal, else coronal), the centre from the series header. The voxels:
/// each file's shorts from byte 14,336, big-endian (`read_signa(<name>): could not open file`;
/// FS-687: a file shorter than its slice).
pub fn signa_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    let (mut odd_only, mut even_only) = (false, false);
    if env.get("GE_ODD").is_some() {
        odd_only = true;
        let _ = out.write_all(b"only using odd # GE Signa files\n");
    } else if env.get("GE_EVEN").is_some() {
        even_only = true;
        let _ = out.write_all(b"only using even # GE Signa files\n");
    }
    let mut fp = match CFile::open(dir, fname, "rb") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            names_note(g);
            log::error_printf(
                g,
                err,
                ERROR_NOFILE,
                &format!("is_signa({fname}): could not open file"),
            );
            return Ok(None);
        }
    };
    let mut h = vec![0u8; HLENGTH];
    if fp.read_bytes(&mut h) != HLENGTH {
        if let Some(e) = fp.read_errno() {
            g.errno = e;
        }
        names_note(g);
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("is_signa({fname}): could not read {HLENGTH} byte header"),
        );
        fp.close();
        return Ok(None);
    }
    let mut hd = get_signa_header_info(g, &h);
    if hd.imnr1 < 0 {
        let _ = writeln!(
            err,
            "WARNING: # of slices={} in header - assuming 124...",
            hd.imnr1
        );
        hd.imnr1 = 124;
    }
    let even = |n: i32| (n / 2) * 2 == n;
    if odd_only {
        if even(hd.imnr0) {
            hd.imnr0 -= 1;
        }
        if even(hd.imnr1) {
            hd.imnr1 -= 1;
        }
    } else if even_only {
        if !even(hd.imnr0) {
            hd.imnr0 += 1;
        }
        if !even(hd.imnr1) {
            hd.imnr1 = hd.imnr1.wrapping_add(1);
        }
    }
    let span = hd.imnr1.wrapping_sub(hd.imnr0);
    let step_two = odd_only || even_only;
    let echoes = hd.num_echoes;
    // FreeSurfer's volume
    let (mut depth, mut nframes) = if step_two {
        (span / 2 + 1, 1)
    } else if echoes > 1 {
        let _ = writeln!(out, "SIGNA multi-echo file detected ({echoes} echoes)");
        (span / 2 + 1, echoes)
    } else {
        (span.wrapping_add(1), 1)
    };
    // FS-686: the files' slices and echoes
    let nfiles = if step_two {
        span / 2 + 1
    } else {
        span.wrapping_add(1)
    };
    let misplaced = echoes > 1 && (echoes != 2 || step_two);
    if misplaced {
        let fixed = (i64::from(nfiles.max(0)) + i64::from(echoes) - 1) / i64::from(echoes);
        let tail = if g.notices.reproduce() {
            ", as in FreeSurfer".to_owned()
        } else {
            format!(
                "; here {fixed} slices of {echoes} frames, file k in slice k / {echoes}, frame k % {echoes}"
            )
        };
        g.notices.add(
            &FS_686,
            format!(
                "signaRead: {nfiles} files of {echoes} echoes{}: FreeSurfer's volume has {depth} slices of {nframes} frames and places the files as for two echoes{tail}",
                if step_two { " (every other file)" } else { "" },
            ),
        );
        if !g.notices.reproduce() {
            depth = fixed as i32;
            nframes = echoes;
        }
    }
    let mut m = if nframes > 1 {
        mri::mri_alloc_sequence(g, out, err, hd.x, hd.y, depth, MRI_SHORT, nframes)?
    } else {
        mri::mri_alloc(g, out, err, hd.x, hd.y, depth, MRI_SHORT)?
    };
    m.xstart = sse::mul(hd.strtx, 1000.0);
    m.xend = sse::mul(hd.endx, 1000.0);
    m.ystart = sse::mul(hd.strty, 1000.0);
    m.yend = sse::mul(hd.endy, 1000.0);
    m.zstart = sse::mul(hd.strtz, 1000.0);
    m.zend = sse::mul(hd.endz, 1000.0);
    m.tr = hd.tr;
    m.ti = hd.ti;
    m.te = hd.te;
    m.location = sse::mul(hd.locatn, 1000.0);
    m.fov = sse::mul(hd.fov, 1000.0);
    let psiz = sse::mul(hd.psiz, 1000.0);
    m.vg.xsize = psiz;
    m.vg.ysize = psiz;
    m.ps = psiz;
    let thick = sse::mul(1000.0, hd.thick);
    m.vg.zsize = thick;
    m.thick = thick;
    let orientation = match hd.plane_type {
        SIGNA_AXIAL => MRI_HORIZONTAL,
        SIGNA_SAGITTAL => MRI_SAGITTAL,
        _ => MRI_CORONAL,
    };
    set_direction_cosine(g, err, &mut m.vg, orientation);
    m.vg.c_r = hd.c_r;
    m.vg.c_s = hd.c_s;
    m.vg.c_a = hd.c_a;
    fp.close();
    if !read_volume {
        return Ok(Some(m));
    }
    let path = utils::file_name_path(fname);
    let n = (i64::from(m.vg.width.max(0)) * i64::from(m.vg.height.max(0))) as usize;
    let table = i64::from(m.vg.depth) * i64::from(m.nframes);
    let (mut slice, mut k) = (0i32, 0i32);
    let mut i = hd.imnr0;
    while i <= hd.imnr1 {
        let name = format!("{path}/I.{i:03}");
        if name.len() >= 1024 {
            g.notices.add(
                &FS_249,
                format!(
                    "signaRead: a slice name of {} bytes written whole (past FreeSurfer's 1,024-byte buffer of the caller's name)",
                    name.len()
                ),
            );
        }
        let mut fp = match CFile::open(dir, &name, "rb") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                names_note(g);
                log::error_printf(
                    g,
                    err,
                    ERROR_NOFILE,
                    &format!("read_signa({name}): could not open file"),
                );
                return Ok(None);
            }
        };
        let mut errno = g.errno;
        fp.fseek(&mut errno, HLENGTH as i64, 0);
        g.errno = errno;
        // the slice table's entry FreeSurfer reads into (`slices[slice + frame * depth]`)
        let index = if echoes > 1 {
            let frame = if misplaced && !g.notices.reproduce() {
                k % echoes
            } else {
                i.wrapping_sub(hd.imnr0) % echoes
            };
            let s = if misplaced && !g.notices.reproduce() {
                k / echoes
            } else {
                slice
            };
            if frame == 0 {
                slice -= 1;
            }
            i64::from(s) + i64::from(frame) * i64::from(m.vg.depth)
        } else {
            i64::from(slice)
        };
        let mut buf = vec![0u8; n * 2];
        let got = fp.read_bytes(&mut buf);
        if got < buf.len() {
            if let Some(e) = fp.read_errno() {
                g.errno = e;
            }
            g.notices.add(
                &FS_687,
                format!(
                    "signaRead: {name} holds {got} of its slice's {} bytes{}",
                    buf.len(),
                    if g.notices.reproduce() {
                        ": the rest of the slice as it was, as in FreeSurfer"
                    } else {
                        " (FreeSurfer keeps the rest of the slice as it was, without a message): no volume"
                    }
                ),
            );
            if !g.notices.reproduce() {
                fp.close();
                log::error_printf(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!(
                        "signaRead({name}): could read {got} of the slice's {} bytes",
                        buf.len()
                    ),
                );
                return Ok(None);
            }
        }
        fp.close();
        if index < 0 || index >= table {
            // this detail replaces the run's earlier FS-686 notice: the result is no longer
            // FreeSurfer's
            let detail = format!(
                "signaRead: {name} goes to slice table entry {index} of {table}: FreeSurfer writes through memory past the table (a crash or corruption): not read here"
            );
            let mut list = g.notices.as_slice().to_vec();
            list.retain(|x| x.defect.id != FS_686.id);
            let mut fresh = Notices::new(g.notices.mode());
            fresh.extend(list);
            fresh.add(&FS_686, detail);
            g.notices = fresh;
        } else {
            let off = (index as u64 * m.vox_per_slice) as usize * 2;
            if let Some(c) = m.chunk.as_mut() {
                c[off..off + got].copy_from_slice(&buf[..got]);
                for v in c[off..off + n * 2].chunks_mut(2) {
                    v.swap(0, 1);
                }
            }
        }
        if step_two {
            i = i.wrapping_add(1);
        }
        i = i.wrapping_add(1);
        slice += 1;
        k += 1;
        if i <= hd.imnr0 {
            break;
        }
    }
    Ok(Some(m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn data_general_floats() {
        // 0x41100000: exponent 65, fraction 1/16 -> 16^1 / 16 = 1, then / 1000
        assert_eq!(ghf(&[0x41, 0x10, 0, 0], 0), 1.0 / 1000.0);
        assert_eq!(ghf(&[0xc1, 0x20, 0, 0], 0), -2.0 / 1000.0);
        assert_eq!(ghf(&[0, 0, 0, 0], 0), 0.0);
        // a zero fraction keeps the exponent: 16^0 * 0.5 (GE's conversion)
        assert_eq!(ghf(&[0x40, 0, 0, 0], 0), 0.5 / 1000.0);
    }

    #[test]
    fn signed_bytes() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        assert_eq!(ghi(&mut g, &[0x00, 0x80], 0), 128);
        assert!(g.notices.contains(&FS_225));
        assert_eq!(ghi(&mut g, &[0xff, 0x00], 0), -256);
        let mut r = Globals::new(UpstreamDefects::Reproduce);
        assert_eq!(ghi(&mut r, &[0x01, 0x80], 0), -128);
        assert_eq!(ghi(&mut r, &[0x01, 0x7f], 0), 383);
    }
}
