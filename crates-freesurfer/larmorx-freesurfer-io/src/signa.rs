// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! GE Signa's header as FreeSurfer 8.2.0 reads it (`utils/signa.cpp`, taken early from queue
//! item 3.1-30 for `mri_identify`): `ghi` (a 16-bit integer, FS-225), `ghf` (a Data General float
//! converted to IEEE, GE's `mvtsunf`), `ghc`, `get_signa_header_info` and `is_signa` (FS-224).
//! `signaRead` stays with item 3.1-30.

use std::path::Path;

use larmorx_core::defects::FS_225;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::utils;

use crate::cfile::CFile;

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
