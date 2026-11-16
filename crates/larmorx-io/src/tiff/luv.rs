// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! `tif_luv.c` and `uvcode.h`: the SGILog decoders (Greg Ward Larson's LogL and LogLuv
//! encodings: 16-bit LogL, 24-bit and 32-bit LogLuv), their conversions to the formats a reader
//! gets (floating-point Y or XYZ, 16-bit, 8-bit grey or RGB, raw) and their pseudo-tags. The
//! encoders are not ported: FreeSurfer writes Deflate only.
//!
//! Copyright (c) 1997 Greg Ward Larson, Copyright (c) 1997 Silicon Graphics, Inc. (`tif_luv.c`'s
//! notice and its no-warranty disclaimer are in `LICENSES/libtiff.txt`).
//!
//! The conversions take `exp` of the codes: by default the correctly rounded one of
//! [`larmorx_core::math`], with `reproduce` glibc 2.35's; a converted value whose bits the other
//! choice would change is named (GLIBC-07).
//!
//! libtiff 3.6.1 sizes its translation buffer by the rows per strip (the default 2^32 - 1 makes
//! the size negative: every SGILog image without RowsPerStrip fails with "No space"; FS-882),
//! and its run and literal loops neither stop at the row's pixel count nor check that a run's
//! two bytes or a 24-bit pixel's three are there (FS-883; writes past the translation buffer:
//! FS-884). By default the buffer is a strip's (or a tile's) and the loops are checked, as
//! libtiff 4.0.10 does.

use super::Tiff;
use super::fields::tag;
use super::next::Out;
use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{FS_882, FS_883, FS_884, GLIBC_07};

pub(crate) const FMT_UNKNOWN: i32 = -1;
const FMT_FLOAT: i32 = 0;
const FMT_16BIT: i32 = 1;
const FMT_RAW: i32 = 2;
const FMT_8BIT: i32 = 3;
const ENCODE_NODITHER: i32 = 0;
const ENCODE_RANDITHER: i32 = 1;
const PHOTOMETRIC_LOGL: u16 = 32844;
const PHOTOMETRIC_LOGLUV: u16 = 32845;

const U_NEU: f64 = 0.210526316;
const V_NEU: f64 = 0.473684211;
const UVSCALE: f64 = 410.0;
/// `M_LN2` (`0.69314718055994530942`, the double nearest ln 2).
const M_LN2: f64 = std::f64::consts::LN_2;
/// `UV_SQSIZ`, `(float)0.003500`.
const UV_SQSIZ: f32 = f32::from_bits(0x3b65_6042);
/// `UV_VSTART`, `(float)0.016940`.
const UV_VSTART: f32 = f32::from_bits(0x3c8a_c5c1);
const UV_NDIVS: i32 = 16289;
const UV_NVS: i32 = 163;
/// uvcode.h's `uv_row`: `ustart` (a `float`, as its bit pattern), `nus`, `ncum`.
#[rustfmt::skip]
const UV_ROW: [(u32, i16, i16); 163] = [
    (0x3e7d9b5f, 4, 0), (0x3e79a134, 6, 4), (0x3e777c03, 7, 10), (0x3e73953e, 9, 17),
    (0x3e719158, 10, 26), (0x3e6db984, 12, 36), (0x3e69d51b, 14, 48), (0x3e67b070, 15, 62),
    (0x3e63b539, 17, 77), (0x3e61b329, 18, 94), (0x3e5bdcf0, 21, 112), (0x3e59d1b3, 22, 133),
    (0x3e57c73f, 23, 155), (0x3e51e53b, 26, 178), (0x3e4fdb90, 27, 204), (0x3e4c06e2, 29, 231),
    (0x3e4837b5, 31, 260), (0x3e463498, 32, 291), (0x3e426f61, 34, 323), (0x3e3eaf68, 36, 357),
    (0x3e3eaf68, 36, 393), (0x3e3af966, 38, 429), (0x3e374c90, 40, 467), (0x3e33ad5c, 42, 507),
    (0x3e302108, 44, 549), (0x3e302108, 44, 593), (0x3e2ca8a0, 46, 637), (0x3e2ca8a0, 46, 683),
    (0x3e278034, 49, 729), (0x3e22730c, 52, 778), (0x3e22730c, 52, 830), (0x3e22730c, 52, 882),
    (0x3e1d81ae, 55, 934), (0x3e1d81ae, 55, 989), (0x3e18ace2, 58, 1044), (0x3e18ace2, 58, 1102),
    (0x3e122c02, 62, 1160), (0x3e122c02, 62, 1222), (0x3e122c02, 62, 1284), (0x3e0d96a7, 65, 1346),
    (0x3e0d96a7, 65, 1411), (0x3e0d96a7, 65, 1476), (0x3e075686, 69, 1541), (0x3e075686, 69, 1610),
    (0x3e013b9f, 73, 1679), (0x3e013b9f, 73, 1752), (0x3e013b9f, 73, 1825), (0x3df68a50, 77, 1898),
    (0x3df68a50, 77, 1975), (0x3df68a50, 77, 2052), (0x3df68a50, 77, 2129), (0x3de7589f, 82, 2206),
    (0x3de7589f, 82, 2288), (0x3de7589f, 82, 2370), (0x3ddc0ebf, 86, 2452), (0x3ddc0ebf, 86, 2538),
    (0x3ddc0ebf, 86, 2624), (0x3ddc0ebf, 86, 2710), (0x3dcd80a1, 91, 2796), (0x3dcd80a1, 91, 2887),
    (0x3dcd80a1, 91, 2978), (0x3dc2d16c, 95, 3069), (0x3dc2d16c, 95, 3164), (0x3dc2d16c, 95, 3259),
    (0x3dc2d16c, 95, 3354), (0x3db4ca0c, 100, 3449), (0x3db4ca0c, 100, 3549), (0x3db4ca0c, 100, 3649),
    (0x3db4ca0c, 100, 3749), (0x3da6f588, 105, 3849), (0x3da6f588, 105, 3954), (0x3da6f588, 105, 4059),
    (0x3da6f588, 105, 4164), (0x3d9950b9, 110, 4269), (0x3d9950b9, 110, 4379), (0x3d9950b9, 110, 4489),
    (0x3d9950b9, 110, 4599), (0x3d8bdba1, 115, 4709), (0x3d8bdba1, 115, 4824), (0x3d8bdba1, 115, 4939),
    (0x3d8bdba1, 115, 5054), (0x3d823290, 119, 5169), (0x3d823290, 119, 5288), (0x3d823290, 119, 5407),
    (0x3d823290, 119, 5526), (0x3d6a5e78, 124, 5645), (0x3d6a5e78, 124, 5769), (0x3d6a5e78, 124, 5893),
    (0x3d6a5e78, 124, 6017), (0x3d50d5a6, 129, 6141), (0x3d50d5a6, 129, 6270), (0x3d50d5a6, 129, 6399),
    (0x3d50d5a6, 129, 6528), (0x3d50d5a6, 129, 6657), (0x3d37be12, 134, 6786), (0x3d37be12, 134, 6920),
    (0x3d37be12, 134, 7054), (0x3d37be12, 134, 7188), (0x3d262dc7, 138, 7322), (0x3d262dc7, 138, 7460),
    (0x3d262dc7, 138, 7598), (0x3d262dc7, 138, 7736), (0x3d14d834, 142, 7874), (0x3d14d834, 142, 8016),
    (0x3d14d834, 142, 8158), (0x3d14d834, 142, 8300), (0x3d03a42f, 146, 8442), (0x3d03a42f, 146, 8588),
    (0x3d03a42f, 146, 8734), (0x3d03a42f, 146, 8880), (0x3ce4f11b, 150, 9026), (0x3ce4f11b, 150, 9176),
    (0x3ce4f11b, 150, 9326), (0x3cc2784b, 154, 9476), (0x3cc2784b, 154, 9630), (0x3cc2784b, 154, 9784),
    (0x3cc2784b, 154, 9938), (0x3c9fc6da, 158, 10092), (0x3c9fc6da, 158, 10250), (0x3c9fc6da, 158, 10408),
    (0x3c8b1141, 161, 10566), (0x3c8b1141, 161, 10727), (0x3c8b1141, 161, 10888), (0x3c8b1141, 161, 11049),
    (0x3c4f13cf, 165, 11210), (0x3c4f13cf, 165, 11375), (0x3c4f13cf, 165, 11540), (0x3c23b14b, 168, 11705),
    (0x3c23b14b, 168, 11873), (0x3c23b14b, 168, 12041), (0x3c13b7d8, 170, 12209), (0x3c13b7d8, 170, 12379),
    (0x3c13b7d8, 170, 12549), (0x3bcbb7fa, 173, 12719), (0x3bcbb7fa, 173, 12892), (0x3ba704bc, 175, 13065),
    (0x3ba704bc, 175, 13240), (0x3ba704bc, 175, 13415), (0x3b801712, 177, 13590), (0x3b801712, 177, 13767),
    (0x3b195aaf, 177, 13944), (0x3b1c90c5, 170, 14121), (0x3a8bfc22, 164, 14291), (0x3ad8a97a, 157, 14455),
    (0x3a3bf50e, 150, 14612), (0x3ad38cda, 143, 14762), (0x398d8ec9, 136, 14905), (0x39fdc161, 129, 15041),
    (0x3a90928a, 123, 15170), (0x3aa2ca9b, 115, 15293), (0x3a9bb6aa, 109, 15408), (0x3a848388, 103, 15517),
    (0x3a39dc2f, 97, 15620), (0x399dcf89, 89, 15717), (0x3b1e55c1, 82, 15806), (0x3b550ebb, 76, 15888),
    (0x3b54bad8, 69, 15964), (0x3b87b13a, 62, 16033), (0x3bc36545, 55, 16095), (0x3c10d174, 47, 16150),
    (0x3c2bde40, 40, 16197), (0x3c8b3700, 31, 16237), (0x3cc1d085, 21, 16268),
];

/// A choice of `exp`.
type ExpFn = fn(f64) -> f64;

/// Which row decoder setup chose (`tif_decoderow`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Row {
    #[default]
    None,
    L16,
    D24,
    D32,
}

/// The conversion of the decoded row (`tfunc`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TFunc {
    #[default]
    Nop,
    L16toY,
    L16toGry,
    Luv24toXyz,
    Luv24toLuv48,
    Luv24toRgb,
    Luv32toXyz,
    Luv32toLuv48,
    Luv32toRgb,
}

/// `LogLuvState` (the decoder's part).
#[derive(Default)]
pub(crate) struct LogLuv {
    pub user_datafmt: i32,
    pub encode_meth: i32,
    pixel_size: i32,
    /// The translation buffer (`tbuf`, 16- or 32-bit entries) and its allocated length.
    tbuf: Vec<u32>,
    tbuflen: u64,
    row: Row,
    tfunc: TFunc,
}

/// The notes of one row's decoding.
#[derive(Default)]
struct Notes {
    overrun: bool,
    beyond_tbuf: bool,
    unchecked: bool,
}

/// Where a decoder puts its pixels: the caller's buffer (16- or 32-bit values at byte `at`) or
/// the translation buffer.
struct Target<'a, 'b> {
    out: &'a mut Out<'b>,
    at: i64,
    direct: bool,
    wide: bool,
    tbuf: &'a mut Vec<u32>,
    cap: u64,
    beyond: bool,
}

impl Target<'_, '_> {
    fn get(&mut self, i: i64) -> u32 {
        if self.direct {
            let w = if self.wide { 4 } else { 2 };
            let mut v = 0u32;
            for b in 0..w {
                v |= u32::from(self.out.get(self.at + i * w + b)) << (8 * b);
            }
            v
        } else if i >= 0 && (i as u64) < self.cap {
            self.tbuf.get(i as usize).copied().unwrap_or(0)
        } else {
            self.beyond = true;
            0
        }
    }
    fn set(&mut self, i: i64, v: u32) {
        if self.direct {
            let w = if self.wide { 4 } else { 2 };
            for b in 0..w {
                self.out.set(self.at + i * w + b, (v >> (8 * b)) as u8);
            }
        } else if i >= 0 && (i as u64) < self.cap {
            let k = i as usize;
            if k >= self.tbuf.len() {
                self.tbuf.resize(k + 1, 0);
            }
            self.tbuf[k] = if self.wide { v } else { v & 0xffff };
        } else {
            self.beyond = true;
        }
    }
    fn or(&mut self, i: i64, v: u32) {
        let x = self.get(i);
        self.set(i, x | v);
    }
}

/// `LogL16toY` with the given `exp`.
fn logl16_to_y(p16: i32, exp: fn(f64) -> f64) -> f64 {
    let le = p16 & 0x7fff;
    if le == 0 {
        return 0.0;
    }
    let y = exp(M_LN2 / 256.0 * (f64::from(le) + 0.5) - M_LN2 * 64.0);
    if p16 & 0x8000 == 0 { y } else { -y }
}

/// `LogL10toY`.
fn logl10_to_y(p10: i32, exp: fn(f64) -> f64) -> f64 {
    if p10 == 0 {
        return 0.0;
    }
    exp(M_LN2 / 64.0 * (f64::from(p10) + 0.5) - M_LN2 * 12.0)
}

/// `uv_decode`: `None` for an index out of range.
fn uv_decode(c: i32) -> Option<(f64, f64)> {
    if !(0..UV_NDIVS).contains(&c) {
        return None;
    }
    let (mut lower, mut upper) = (0i32, UV_NVS);
    while upper - lower > 1 {
        let vi = (lower + upper) >> 1;
        let ui = c - i32::from(UV_ROW[vi as usize].2);
        if ui > 0 {
            lower = vi;
        } else if ui < 0 {
            upper = vi;
        } else {
            lower = vi;
            break;
        }
    }
    let vi = lower;
    let ui = c - i32::from(UV_ROW[vi as usize].2);
    let ustart = f64::from(f32::from_bits(UV_ROW[vi as usize].0));
    let u = ustart + (f64::from(ui) + 0.5) * f64::from(UV_SQSIZ);
    let v = f64::from(UV_VSTART) + (f64::from(vi) + 0.5) * f64::from(UV_SQSIZ);
    Some((u, v))
}

/// The XYZ of a luminance and chromaticity (the end of `LogLuv24toXYZ`, `LogLuv32toXYZ`).
fn uv_to_xyz(l: f64, u: f64, v: f64) -> [f32; 3] {
    let s = 1.0 / (6.0 * u - 16.0 * v + 12.0);
    let x = 9.0 * u * s;
    let y = 4.0 * v * s;
    [(x / y * l) as f32, l as f32, ((1.0 - x - y) / y * l) as f32]
}

/// `LogLuv24toXYZ`.
fn luv24_to_xyz(p: u32, exp: fn(f64) -> f64) -> [f32; 3] {
    let l = logl10_to_y((p >> 14 & 0x3ff) as i32, exp);
    if l <= 0.0 {
        return [0.0; 3];
    }
    let (u, v) = uv_decode((p & 0x3fff) as i32).unwrap_or((U_NEU, V_NEU));
    uv_to_xyz(l, u, v)
}

/// `LogLuv32toXYZ`.
fn luv32_to_xyz(p: u32, exp: fn(f64) -> f64) -> [f32; 3] {
    let l = logl16_to_y((p as i32) >> 16, exp);
    if l <= 0.0 {
        return [0.0; 3];
    }
    let u = 1.0 / UVSCALE * (f64::from(p >> 8 & 0xff) + 0.5);
    let v = 1.0 / UVSCALE * (f64::from(p & 0xff) + 0.5);
    uv_to_xyz(l, u, v)
}

/// `(uint8)((r <= 0) ? 0 : (r >= 1) ? 255 : (int)(256 sqrt(r)))`.
fn gamma8(r: f64) -> u8 {
    if r <= 0.0 {
        0
    } else if r >= 1.0 {
        255
    } else {
        (256.0 * r.sqrt()) as i32 as u8
    }
}

/// `XYZtoRGB24`.
fn xyz_to_rgb(xyz: [f32; 3]) -> [u8; 3] {
    let (x, y, z) = (f64::from(xyz[0]), f64::from(xyz[1]), f64::from(xyz[2]));
    let r = 2.690 * x + -1.276 * y + -0.414 * z;
    let g = -1.022 * x + 1.978 * y + 0.044 * z;
    let b = 0.061 * x + -0.224 * y + 1.163 * z;
    [gamma8(r), gamma8(g), gamma8(b)]
}

/// The output bytes of one pixel `p` (a 16-bit LogL, sign-extended, or a 32-bit entry) by
/// `f`.
fn convert(f: TFunc, p: u32, exp: fn(f64) -> f64) -> Vec<u8> {
    let le = |v: f32| v.to_le_bytes();
    match f {
        TFunc::Nop => Vec::new(),
        TFunc::L16toY => le(logl16_to_y(p as u16 as i16 as i32, exp) as f32).to_vec(),
        TFunc::L16toGry => vec![gamma8(logl16_to_y(p as u16 as i16 as i32, exp))],
        TFunc::Luv24toXyz => luv24_to_xyz(p, exp).iter().flat_map(|&v| le(v)).collect(),
        TFunc::Luv32toXyz => luv32_to_xyz(p, exp).iter().flat_map(|&v| le(v)).collect(),
        TFunc::Luv24toRgb => xyz_to_rgb(luv24_to_xyz(p, exp)).to_vec(),
        TFunc::Luv32toRgb => xyz_to_rgb(luv32_to_xyz(p, exp)).to_vec(),
        TFunc::Luv24toLuv48 => {
            let l = ((p >> 12 & 0xffd).wrapping_add(13314)) as i16;
            let (u, v) = uv_decode((p & 0x3fff) as i32).unwrap_or((U_NEU, V_NEU));
            [l, (u * 32768.0) as i16, (v * 32768.0) as i16]
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect()
        }
        TFunc::Luv32toLuv48 => {
            let l = (p >> 16) as i16;
            let u = 1.0 / UVSCALE * (f64::from(p >> 8 & 0xff) + 0.5);
            let v = 1.0 / UVSCALE * (f64::from(p & 0xff) + 0.5);
            [l, (u * 32768.0) as i16, (v * 32768.0) as i16]
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect()
        }
    }
}

/// `LogL16GuessDataFmt` (its `PACK` keeps the original's overlapping bit fields).
fn guess_l16(spp: u16, bps: u16, sf: u16) -> i32 {
    let pack = |s: u32, b: u32, f: u32| (b << 6) | (s << 3) | f;
    let p = pack(u32::from(spp), u32::from(bps), u32::from(sf));
    if p == pack(1, 32, 3) {
        FMT_FLOAT
    } else if p == pack(1, 16, 4) || p == pack(1, 16, 2) || p == pack(1, 16, 1) {
        FMT_16BIT
    } else if p == pack(1, 8, 4) || p == pack(1, 8, 1) {
        FMT_8BIT
    } else {
        FMT_UNKNOWN
    }
}

/// `LogLuvGuessDataFmt`.
fn guess_luv(spp: u16, bps: u16, sf: u16) -> i32 {
    let pack = |b: u32, f: u32| (b << 3) | f;
    let p = pack(u32::from(bps), u32::from(sf));
    let guess = if p == pack(32, 3) {
        FMT_FLOAT
    } else if p == pack(32, 4) || p == pack(32, 1) || p == pack(32, 2) {
        FMT_RAW
    } else if p == pack(16, 4) || p == pack(16, 2) || p == pack(16, 1) {
        FMT_16BIT
    } else if p == pack(8, 4) || p == pack(8, 1) {
        FMT_8BIT
    } else {
        FMT_UNKNOWN
    };
    match spp {
        1 if guess != FMT_RAW => FMT_UNKNOWN,
        1 => guess,
        3 if guess == FMT_RAW => FMT_UNKNOWN,
        3 => guess,
        _ => FMT_UNKNOWN,
    }
}

impl Tiff {
    /// `TIFFInitSGILog`.
    pub(crate) fn logluv_init(&mut self, scheme: u16) {
        use super::codec::{Cleanup, Decode, SetupDecode};
        use super::dir::Layer;
        use super::fields::Table;
        self.codec.logluv = Some(Box::new(LogLuv {
            user_datafmt: FMT_UNKNOWN,
            encode_meth: if scheme == super::compression::SGILOG24 {
                ENCODE_RANDITHER
            } else {
                ENCODE_NODITHER
            },
            ..LogLuv::default()
        }));
        let k = &mut self.codec;
        k.setupdecode = SetupDecode::LogLuv;
        k.decodestrip = Decode::LuvStrip;
        k.decodetile = Decode::LuvTile;
        k.cleanup = Cleanup::LogLuv;
        self.fields.merge(Table::LogLuv);
        self.codec.layers.insert(0, Layer::LogLuv);
    }

    /// `LogLuvVSetField` for its pseudo-tags (only a caller's `TIFFSetField` reaches them: a
    /// file's tags are 16-bit).
    pub(crate) fn logluv_vsetfield(&mut self, t: u32, v: u32, s: &mut dyn Streams) -> Option<bool> {
        use super::dir::Arg;
        let name = self.name.clone();
        let sp = self.codec.logluv.as_mut()?;
        match t {
            tag::SGILOGDATAFMT => {
                sp.user_datafmt = v as i32;
                let (bps, fmt) = match sp.user_datafmt {
                    FMT_FLOAT => (32, 3),
                    FMT_16BIT => (16, 2),
                    FMT_RAW => (32, 1),
                    FMT_8BIT => (8, 1),
                    d => {
                        let m = format!("Unknown data format {d} for LogLuv compression");
                        self.error(Some(&name), &m, s);
                        return Some(false);
                    }
                };
                if sp.user_datafmt == FMT_RAW {
                    self.set_field_arg(tag::SAMPLESPERPIXEL, Arg::Int(1), s);
                }
                self.set_field_arg(tag::BITSPERSAMPLE, Arg::Int(bps), s);
                self.set_field_arg(tag::SAMPLEFORMAT, Arg::Int(fmt), s);
                self.tilesize = self.tile_size();
                self.scanlinesize = self.scanline_size();
                Some(true)
            }
            tag::SGILOGENCODE => {
                sp.encode_meth = v as i32;
                if sp.encode_meth != ENCODE_NODITHER && sp.encode_meth != ENCODE_RANDITHER {
                    let m = format!("Unknown encoding {} for LogLuv compression", sp.encode_meth);
                    self.error(Some(&name), &m, s);
                    return Some(false);
                }
                Some(true)
            }
            _ => None,
        }
    }

    /// `LogLuvVGetField`.
    pub(crate) fn logluv_vgetfield(&self, t: u32) -> Option<super::Value> {
        let sp = self.codec.logluv.as_ref()?;
        (t == tag::SGILOGDATAFMT).then_some(super::Value::I32(sp.user_datafmt))
    }

    /// `LogLuvSetupDecode` with `LogL16InitState` and `LogLuvInitState`.
    pub(crate) fn logluv_setup(&mut self, s: &mut dyn Streams) -> bool {
        use super::codec::Decode;
        self.codec.postdecode = super::dir::PostDecode::None;
        let d = &self.dir;
        let (photometric, planar, spp, bps, sf) = (
            d.photometric,
            d.planarconfig,
            d.samplesperpixel,
            d.bitspersample,
            d.sampleformat,
        );
        let name = self.name.clone();
        let luv = match photometric {
            PHOTOMETRIC_LOGLUV => true,
            PHOTOMETRIC_LOGL => false,
            _ => {
                let m = format!(
                    "Inappropriate photometric interpretation {photometric} for SGILog \
                     compression; must be either LogLUV or LogL"
                );
                self.error(Some(&name), &m, s);
                return false;
            }
        };
        let module = if luv {
            "LogLuvInitState"
        } else {
            "LogL16InitState"
        };
        if luv && planar != super::values::PLANARCONFIG_CONTIG {
            self.error(
                Some(module),
                "SGILog compression cannot handle non-contiguous data",
                s,
            );
            return false;
        }
        let sp = self.codec.logluv.as_mut().expect("LogLuv state");
        if sp.user_datafmt == FMT_UNKNOWN {
            sp.user_datafmt = if luv {
                guess_luv(spp, bps, sf)
            } else {
                guess_l16(spp, bps, sf)
            };
        }
        let fmt = sp.user_datafmt;
        let pixel_size = match (luv, fmt) {
            (false, FMT_FLOAT) => 4,
            (false, FMT_16BIT) => 2,
            (false, FMT_8BIT) => 1,
            (true, FMT_FLOAT) => 12,
            (true, FMT_16BIT) => 6,
            (true, FMT_RAW) => 4,
            (true, FMT_8BIT) => 3,
            _ => {
                let m = format!(
                    "No support for converting user data format to {}",
                    if luv { "LogLuv" } else { "LogL" }
                );
                self.error(Some(&name), &m, s);
                return false;
            }
        };
        sp.pixel_size = pixel_size;
        // libtiff 3.6.1: `tbuflen = width * rowsperstrip` in an int
        let d = &self.dir;
        let tbuflen = d.imagewidth.wrapping_mul(d.rowsperstrip) as i32;
        let esize: u64 = if luv { 4 } else { 2 };
        let bytes = (i64::from(tbuflen) as u64).wrapping_mul(esize);
        let fixed = if self.is_tiled() {
            u64::from(d.tilewidth) * u64::from(d.tilelength)
        } else {
            u64::from(d.imagewidth) * u64::from(d.rowsperstrip.min(d.imagelength))
        };
        let fails = |n: u64| n >= 1 << 47 || !self.can_alloc(n);
        let (theirs_fail, ours_fail) = (fails(bytes), fails(fixed * esize));
        let fix = self.fix();
        let differs = theirs_fail != ours_fail || (!theirs_fail && tbuflen as u64 != fixed);
        if differs {
            self.notices.add(
                &FS_882,
                format!(
                    "the SGILog translation buffer of {name}: libtiff 3.6.1 asks for {} \
                     pixels ({} x {} rows per strip in an int){}, a strip or tile holds {fixed}{}",
                    tbuflen,
                    self.dir.imagewidth,
                    self.dir.rowsperstrip,
                    if theirs_fail { ", which fails" } else { "" },
                    if fix { " (corrected: that size)" } else { "" }
                ),
            );
        }
        if (fix && ours_fail) || (!fix && theirs_fail) {
            let m = format!("{name}: No space for SGILog translation buffer");
            self.error(Some(module), &m, s);
            return false;
        }
        let compression = self.dir.compression;
        let sp = self.codec.logluv.as_mut().expect("LogLuv state");
        sp.tbuf.clear();
        sp.tbuflen = if fix { fixed } else { tbuflen as u64 };
        let (row, tfunc) = if !luv {
            (
                Row::L16,
                match fmt {
                    FMT_FLOAT => TFunc::L16toY,
                    FMT_8BIT => TFunc::L16toGry,
                    _ => TFunc::Nop,
                },
            )
        } else if compression == super::compression::SGILOG24 {
            (
                Row::D24,
                match fmt {
                    FMT_FLOAT => TFunc::Luv24toXyz,
                    FMT_16BIT => TFunc::Luv24toLuv48,
                    FMT_8BIT => TFunc::Luv24toRgb,
                    _ => TFunc::Nop,
                },
            )
        } else {
            (
                Row::D32,
                match fmt {
                    FMT_FLOAT => TFunc::Luv32toXyz,
                    FMT_16BIT => TFunc::Luv32toLuv48,
                    FMT_8BIT => TFunc::Luv32toRgb,
                    _ => TFunc::Nop,
                },
            )
        };
        sp.row = row;
        sp.tfunc = tfunc;
        self.codec.decoderow = Decode::Luv;
        true
    }

    /// `LogLuvDecodeStrip` and `LogLuvDecodeTile`: rows of `TIFFScanlineSize` or
    /// `TIFFTileRowSize` bytes.
    pub(crate) fn logluv_decode_rows(
        &mut self,
        buf: &mut [u8],
        cc0: i32,
        tile: bool,
        s: &mut dyn Streams,
    ) -> i32 {
        let rowlen = if tile {
            self.tile_row_size()
        } else {
            self.scanline_size()
        };
        let mut o = Out { buf, past: false };
        let (mut cc, mut at) = (cc0, 0i64);
        while cc != 0 {
            if rowlen == 0 {
                let name = self.name.clone();
                self.notices.add(
                    &FS_883,
                    format!(
                        "decoding {name}: SGILog rows of 0 bytes for {cc} bytes: libtiff 3.6.1 \
                         never returns (a hang); nothing decoded"
                    ),
                );
                break;
            }
            if self.logluv_row(&mut o, at, rowlen, s) == 0 {
                break;
            }
            at += i64::from(rowlen);
            cc = cc.wrapping_sub(rowlen);
        }
        if o.past {
            self.past_buffer_notice("SGILog");
        }
        i32::from(cc == 0)
    }

    /// The row decoder setup chose, into the buffer at `at` (`LogL16Decode`, `LogLuvDecode24`,
    /// `LogLuvDecode32`).
    pub(crate) fn logluv_decode(&mut self, buf: &mut [u8], occ: i32, s: &mut dyn Streams) -> i32 {
        let mut o = Out { buf, past: false };
        let r = self.logluv_row(&mut o, 0, occ, s);
        if o.past {
            self.past_buffer_notice("SGILog");
        }
        r
    }

    fn logluv_row(&mut self, o: &mut Out, at: i64, occ: i32, s: &mut dyn Streams) -> i32 {
        let fix = self.fix();
        let name = self.name.clone();
        let mut sp = self.codec.logluv.take().expect("LogLuv state");
        let npixels = if sp.pixel_size == 0 {
            0
        } else {
            occ / sp.pixel_size
        };
        let (planes, wide, module) = match sp.row {
            Row::L16 => (2, false, "LogL16Decode"),
            Row::D24 => (0, true, "LogLuvDecode24"),
            Row::D32 => (4, true, "LogLuvDecode32"),
            Row::None => {
                self.codec.logluv = Some(sp);
                return 0;
            }
        };
        let direct = match sp.row {
            Row::L16 => sp.user_datafmt == FMT_16BIT,
            _ => sp.user_datafmt == FMT_RAW,
        };
        let cap = sp.tbuflen;
        let mut nt = Notes::default();
        let mut bp = self.rawcp;
        let end = self.rawcp + self.rawcc.max(0) as usize;
        let mut cc = self.rawcc;
        let mut i: i64 = 0;
        let short;
        {
            let mut tp = Target {
                out: o,
                at,
                direct,
                wide,
                tbuf: &mut sp.tbuf,
                cap,
                beyond: false,
            };
            if planes == 0 {
                // 24-bit pixels, three bytes each
                while i < i64::from(npixels) && cc > 0 {
                    if cc < 3 {
                        nt.unchecked = true;
                        if fix {
                            break;
                        }
                    }
                    let v = u32::from(self.raw_byte(bp, end)) << 16
                        | u32::from(self.raw_byte(bp + 1, end)) << 8
                        | u32::from(self.raw_byte(bp + 2, end));
                    tp.set(i, v);
                    bp += 3;
                    cc -= 3;
                    i += 1;
                }
                short = i != i64::from(npixels);
            } else {
                for k in 0..npixels.max(0) {
                    tp.set(i64::from(k), 0);
                }
                let mut bad = false;
                let mut shft = 8 * planes;
                loop {
                    shft -= 8;
                    if shft < 0 {
                        break;
                    }
                    i = 0;
                    while i < i64::from(npixels) && cc > 0 {
                        let b0 = self.raw_byte(bp, end);
                        if b0 >= 128 {
                            if cc < 2 {
                                nt.unchecked = true;
                                if fix {
                                    break;
                                }
                            }
                            let mut rc = i32::from(b0) + (2 - 128);
                            let b1 = self.raw_byte(bp + 1, end);
                            bp += 2;
                            let b: u32 = if wide {
                                u32::from(b1) << shft
                            } else {
                                u32::from(((u32::from(b1) << shft) as i16) as u16)
                            };
                            cc -= 2;
                            while rc > 0 {
                                rc -= 1;
                                if i >= i64::from(npixels) {
                                    nt.overrun = true;
                                    if fix {
                                        break;
                                    }
                                }
                                tp.or(i, b);
                                i += 1;
                            }
                        } else {
                            let mut rc = i32::from(b0);
                            bp += 1;
                            loop {
                                cc -= 1;
                                if cc == 0 {
                                    break;
                                }
                                if rc == 0 {
                                    break;
                                }
                                rc -= 1;
                                if i >= i64::from(npixels) {
                                    nt.overrun = true;
                                    if fix {
                                        break;
                                    }
                                }
                                let x = self.raw_byte(bp, end);
                                bp += 1;
                                let v = if wide {
                                    u32::from(x) << shft
                                } else {
                                    u32::from(((u32::from(x) << shft) as i16) as u16)
                                };
                                tp.or(i, v);
                                i += 1;
                            }
                        }
                    }
                    if i != i64::from(npixels) {
                        bad = true;
                        break;
                    }
                }
                short = bad;
            }
            if tp.beyond {
                nt.beyond_tbuf = true;
            }
        }
        if short {
            let m = format!(
                "{module}: Not enough data at row {} (short {} pixels)",
                self.row as i32,
                (i64::from(npixels) - i) as i32
            );
            self.error(Some(&name), &m, s);
            self.rawcp = bp;
            self.rawcc = cc;
            self.codec.logluv = Some(sp);
            self.logluv_notes(&nt, fix);
            return 0;
        }
        // the conversion, each value with the mode's exp; GLIBC-07 where the other's differs
        let (mine, other): (ExpFn, ExpFn) = if fix {
            (larmorx_core::math::exp, larmorx_core::math::exp_glibc)
        } else {
            (larmorx_core::math::exp_glibc, larmorx_core::math::exp)
        };
        let mut glibc_differs = false;
        if sp.tfunc != TFunc::Nop {
            let mut a = at;
            for k in 0..npixels.max(0) as usize {
                let p = sp.tbuf.get(k).copied().unwrap_or(0);
                let bytes = convert(sp.tfunc, p, mine);
                if !glibc_differs && convert(sp.tfunc, p, other) != bytes {
                    glibc_differs = true;
                }
                for (j, &b) in bytes.iter().enumerate() {
                    o.set(a + j as i64, b);
                }
                a += bytes.len() as i64;
            }
        }
        self.rawcp = bp;
        self.rawcc = cc;
        self.codec.logluv = Some(sp);
        self.logluv_notes(&nt, fix);
        if glibc_differs {
            self.notices.add(
                &GLIBC_07,
                format!(
                    "decoding {name}: an SGILog value converted with glibc 2.35's exp differs \
                     from the correctly rounded one in its last bits"
                ),
            );
        }
        1
    }

    fn logluv_notes(&mut self, nt: &Notes, fix: bool) {
        let name = self.name.clone();
        if nt.overrun || nt.unchecked {
            self.notices.add(
                &FS_883,
                format!(
                    "decoding {name}: SGILog {}{}",
                    if nt.overrun {
                        "runs or literals past the row's pixel count"
                    } else {
                        "a run's or a pixel's bytes past the strip's data"
                    },
                    if fix {
                        " (corrected as libtiff 4.0.10 does: the loops stop there)"
                    } else {
                        " (libtiff 3.6.1 reads and writes on)"
                    }
                ),
            );
        }
        if nt.beyond_tbuf {
            self.notices.add(
                &FS_884,
                format!(
                    "decoding {name}: SGILog pixels written past the translation buffer \
                     libtiff 3.6.1 allocated (memory past it); not written"
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uv_decode_and_the_guesses() {
        assert_eq!(uv_decode(-1), None);
        assert_eq!(uv_decode(UV_NDIVS), None);
        let (u, v) = uv_decode(0).expect("in range");
        assert!(
            (u - 0.249413).abs() < 1e-5 && (v - 0.01869).abs() < 1e-5,
            "{u} {v}"
        );
        assert!(uv_decode(UV_NDIVS - 1).is_some());
        assert_eq!(guess_l16(1, 32, 3), FMT_FLOAT);
        assert_eq!(guess_l16(1, 16, 2), FMT_16BIT);
        assert_eq!(guess_l16(3, 8, 1), FMT_UNKNOWN);
        assert_eq!(guess_luv(3, 32, 3), FMT_FLOAT);
        assert_eq!(guess_luv(1, 32, 1), FMT_RAW);
        assert_eq!(guess_luv(3, 32, 1), FMT_UNKNOWN);
        assert_eq!(guess_luv(3, 8, 1), FMT_8BIT);
    }
}
