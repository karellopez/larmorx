// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! `tif_dir.c` and `tif_aux.c`: the directory (`TIFFDirectory`), `TIFFDefaultDirectory`,
//! `TIFFFreeDirectory`, `TIFFSetField` (`OkToChangeTag`, `_TIFFVSetField` and the codecs'
//! `vsetfield` hooks), `TIFFGetField` (`_TIFFVGetField`) and `TIFFGetFieldDefaulted`.
//!
//! The directory keeps every value a later step of the library or a caller reads (sizes,
//! formats, strips, the values the writer writes); the strings and the arrays no step reads
//! (descriptions, colour maps, transfer functions, profiles) keep only their presence bit, as
//! the bit is what the library consults.

use super::fields::{FieldInfo, bit, tag};
use super::{
    Mode, TIFF_BEENWRITING, TIFF_CODERSETUP, TIFF_DIRTYDIRECT, TIFF_ISTILED, TIFF_SWAB, Tiff,
    Unported, values,
};
use larmorx_core::cfmt::{self, Streams};
use larmorx_core::defects::FS_816;

/// The postdecode routine (`tif_postdecode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PostDecode {
    #[default]
    None,
    Swab16,
    Swab32,
    Swab64,
}

/// `TIFFDirectory`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Dir {
    pub fieldsset: [u64; 4],
    pub imagewidth: u32,
    pub imagelength: u32,
    pub imagedepth: u32,
    pub tilewidth: u32,
    pub tilelength: u32,
    pub tiledepth: u32,
    pub subfiletype: u32,
    pub bitspersample: u16,
    pub sampleformat: u16,
    pub compression: u16,
    pub photometric: u16,
    pub threshholding: u16,
    pub fillorder: u16,
    pub orientation: u16,
    pub samplesperpixel: u16,
    pub rowsperstrip: u32,
    pub minsamplevalue: u16,
    pub maxsamplevalue: u16,
    pub sminsamplevalue: f64,
    pub smaxsamplevalue: f64,
    pub xresolution: f32,
    pub yresolution: f32,
    pub resolutionunit: u16,
    pub planarconfig: u16,
    pub xposition: f32,
    pub yposition: f32,
    pub pagenumber: [u16; 2],
    pub halftonehints: [u16; 2],
    pub extrasamples: u16,
    pub sampleinfo: Vec<u16>,
    pub stonits: f64,
    pub stripsperimage: u32,
    pub nstrips: u32,
    pub stripoffset: Option<Vec<u32>>,
    pub stripbytecount: Option<Vec<u32>>,
    pub nsubifd: u16,
    pub subifd: Vec<u32>,
    pub ycbcrsubsampling: [u16; 2],
    pub ycbcrpositioning: u16,
    pub inkset: u16,
    pub ninks: u16,
    pub dotrange: [u16; 2],
    pub imagefullwidth: u32,
    pub imagefulllength: u32,
    pub fovcot: f32,
    /// The custom fields set (`td_customValues`): tag and type.
    pub custom: Vec<(u32, u16)>,
}

impl Dir {
    pub fn is_set(&self, b: u16) -> bool {
        let b = usize::from(b);
        self.fieldsset[b / 32] & (1u64 << (b & 0x1f)) != 0
    }
    pub fn set_bit(&mut self, b: u16) {
        let b = usize::from(b);
        self.fieldsset[b / 32] |= 1u64 << (b & 0x1f);
    }
    pub fn clear_bit(&mut self, b: u16) {
        let b = usize::from(b);
        self.fieldsset[b / 32] &= !(1u64 << (b & 0x1f));
    }
}

/// An array passed to `TIFFSetField`.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Data<'a> {
    U16(&'a [u16]),
    U32(&'a [u32]),
    F32(&'a [f32]),
    F64(&'a [f64]),
    Bytes(&'a [u8]),
}

/// The variable arguments of `TIFFSetField`, as the callers in this library pass them.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Arg<'a> {
    /// One integer (an `int`, a promoted `uint16`, a `uint32`).
    Int(u32),
    /// Two integers.
    Int2(u32, u32),
    /// A `double` (a promoted `float`).
    Dbl(f64),
    /// A `char*` string: its bytes before the NUL.
    Str(&'a [u8]),
    /// A count and a pointer (the fields that pass their count).
    Count(u32, Data<'a>),
    /// One array.
    Ptr(Data<'a>),
    /// Three arrays (colour map, transfer function).
    Ptr3(Data<'a>, Data<'a>, Data<'a>),
}

impl Arg<'_> {
    fn int(&self) -> u32 {
        match *self {
            Arg::Int(v) | Arg::Int2(v, _) | Arg::Count(v, _) => v,
            Arg::Dbl(d) => d as i64 as u32,
            _ => 0,
        }
    }
    fn dbl(&self) -> f64 {
        match *self {
            Arg::Dbl(d) => d,
            Arg::Int(v) => f64::from(v),
            _ => 0.0,
        }
    }
}

/// A value for [`Tiff::set_field`]: an integer argument or a `double` one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SetValue {
    Int(i64),
    Double(f64),
}

/// A value [`Tiff::get_field`] returns, typed as the C function writes it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    U16(u16),
    U32(u32),
    I32(i32),
    F32(f32),
    F64(f64),
    U16Pair(u16, u16),
    /// `TIFFTAG_EXTRASAMPLES`: the count and the types.
    ExtraSamples(u16, Vec<u16>),
    /// A strip or tile array.
    U32Array(Vec<u32>),
    /// A field whose value this port does not keep (strings, colour maps, profiles, ...).
    Present,
}

/// A codec's tag hooks, outermost first (`tif_tagmethods`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layer {
    Predictor,
    Zip,
}

impl Tiff {
    /// `TIFFFreeDirectory(tif)`.
    pub(crate) fn free_directory(&mut self) {
        let d = &mut self.dir;
        d.sampleinfo.clear();
        d.subifd.clear();
        d.stripoffset = None;
        d.stripbytecount = None;
        d.custom.clear();
    }

    /// `TIFFDefaultDirectory(tif)`.
    pub(crate) fn default_directory(&mut self, s: &mut dyn Streams) -> Result<(), Unported> {
        self.fields.setup();
        self.dir = Dir {
            fillorder: values::FILLORDER_MSB2LSB,
            bitspersample: 1,
            threshholding: values::THRESHHOLD_BILEVEL,
            orientation: values::ORIENTATION_TOPLEFT,
            samplesperpixel: 1,
            rowsperstrip: u32::MAX,
            tilewidth: u32::MAX,
            tilelength: u32::MAX,
            tiledepth: 1,
            resolutionunit: values::RESUNIT_INCH,
            sampleformat: values::SAMPLEFORMAT_UINT,
            imagedepth: 1,
            ycbcrsubsampling: [2, 2],
            ycbcrpositioning: values::YCBCRPOSITION_CENTERED,
            inkset: values::INKSET_CMYK,
            ninks: 4,
            ..Dir::default()
        };
        self.codec.postdecode = PostDecode::None;
        self.codec.layers.clear();
        self.codec.layers_stale = false;
        self.set_field_arg(tag::COMPRESSION, Arg::Int(1), s);
        if let Some(u) = self.unported {
            return Err(u);
        }
        self.flags &= !TIFF_DIRTYDIRECT;
        self.flags &= !TIFF_ISTILED;
        Ok(())
    }

    /// `_TIFFFieldWithTag(tif, tag)`.
    pub(crate) fn field_with_tag(&self, tag: u32, s: &mut dyn Streams) -> Option<FieldInfo> {
        let f = self.fields.find(tag);
        if f.is_none() {
            self.error(
                Some("TIFFFieldWithTag"),
                &format!("Internal error, unknown tag {tag:#x}"),
                s,
            );
        }
        f
    }

    fn field_name(&self, tag: u32, s: &mut dyn Streams) -> String {
        self.field_with_tag(tag, s)
            .map(|f| f.name.into_owned())
            .unwrap_or_default()
    }

    /// `TIFFSetField(tif, tag, value)` for the integer and `double` fields a caller sets: the
    /// status (1 or 0), its messages written.
    pub fn set_field(
        &mut self,
        tag: u32,
        v: SetValue,
        s: &mut dyn Streams,
    ) -> Result<bool, Unported> {
        let a = match v {
            SetValue::Int(i) => Arg::Int(i as u32),
            SetValue::Double(d) => Arg::Dbl(d),
        };
        let ok = self.set_field_arg(tag, a, s);
        match self.unported {
            Some(u) => Err(u),
            None => Ok(ok),
        }
    }

    /// `TIFFSetField` (`TIFFVSetField`: `OkToChangeTag`, then the tag methods).
    pub(crate) fn set_field_arg(&mut self, tag: u32, a: Arg, s: &mut dyn Streams) -> bool {
        let Some(fip) = self.fields.find(tag) else {
            let p = if tag > 0xffff { "pseudo-" } else { "" };
            let m = format!("{}: Unknown {p}tag {tag}", self.name);
            self.error(Some("TIFFSetField"), &m, s);
            return false;
        };
        if tag != tag::IMAGELENGTH && self.flags & TIFF_BEENWRITING != 0 && !fip.oktochange {
            let m = format!(
                "{}: Cannot modify tag \"{}\" while writing",
                self.name, fip.name
            );
            self.error(Some("TIFFSetField"), &m, s);
            return false;
        }
        self.stale_layers_notice("TIFFSetField");
        let layers = self.codec.layers.clone();
        for l in layers {
            match l {
                Layer::Zip if tag == tag::ZIPQUALITY => return self.zip_set_quality(a.int(), s),
                Layer::Predictor if tag == tag::PREDICTOR => {
                    self.codec.predictor.predictor = i32::from(a.int() as u16);
                    self.dir.set_bit(bit::PREDICTOR);
                    self.flags |= TIFF_DIRTYDIRECT;
                    return true;
                }
                _ => {}
            }
        }
        self.vsetfield(tag, a, s)
    }

    /// The notice of FS-816 when the tag methods of a codec replaced in this directory are
    /// still in place (libtiff 3.6.1 calls through them: freed or another codec's state).
    pub(crate) fn stale_layers_notice(&mut self, what: &str) {
        if self.codec.layers_stale {
            self.notices.add(
                &FS_816,
                format!(
                    "{what} after a second Compression field in one directory of {}: libtiff \
                     3.6.1 calls the replaced codec's tag methods on freed or mismatched state \
                     (a crash); the current codec's methods are used",
                    self.name
                ),
            );
        }
    }

    /// `_TIFFVSetField`.
    fn vsetfield(&mut self, t: u32, a: Arg, s: &mut dyn Streams) -> bool {
        const MODULE: &str = "_TIFFVSetField";
        let mut status = true;
        enum Bad {
            Int(i32),
            U32(u32),
            Dbl(f64),
        }
        let mut bad: Option<Bad> = None;
        match t {
            tag::SUBFILETYPE => self.dir.subfiletype = a.int(),
            tag::IMAGEWIDTH => self.dir.imagewidth = a.int(),
            tag::IMAGELENGTH => self.dir.imagelength = a.int(),
            tag::BITSPERSAMPLE => {
                self.dir.bitspersample = a.int() as u16;
                if self.flags & TIFF_SWAB != 0 {
                    match self.dir.bitspersample {
                        16 => self.codec.postdecode = PostDecode::Swab16,
                        32 => self.codec.postdecode = PostDecode::Swab32,
                        64 => self.codec.postdecode = PostDecode::Swab64,
                        _ => {}
                    }
                }
            }
            tag::COMPRESSION => {
                let v = (a.int() & 0xffff) as u16;
                let mut change = true;
                if self.dir.is_set(bit::COMPRESSION) {
                    if self.dir.compression == v {
                        change = false;
                    } else {
                        let had_layers = !self.codec.layers.is_empty();
                        self.cleanup();
                        self.flags &= !TIFF_CODERSETUP;
                        if had_layers {
                            self.codec.layers_stale = true;
                        }
                    }
                }
                if change {
                    if self.set_compression_scheme(v, s) {
                        self.dir.compression = v;
                    } else {
                        status = false;
                    }
                }
            }
            tag::PHOTOMETRIC => self.dir.photometric = a.int() as u16,
            tag::THRESHHOLDING => self.dir.threshholding = a.int() as u16,
            tag::FILLORDER => {
                let v = a.int() as i32;
                if v != 2 && v != 1 {
                    bad = Some(Bad::Int(v));
                } else {
                    self.dir.fillorder = v as u16;
                }
            }
            tag::DOCUMENTNAME
            | tag::ARTIST
            | tag::DATETIME
            | tag::HOSTCOMPUTER
            | tag::IMAGEDESCRIPTION
            | tag::MAKE
            | tag::MODEL
            | tag::COPYRIGHT
            | tag::PAGENAME
            | tag::TARGETPRINTER
            | tag::PIXAR_TEXTUREFORMAT
            | tag::PIXAR_WRAPMODES => {}
            tag::ORIENTATION => {
                let v = a.int() as i32;
                if !(1..=8).contains(&v) {
                    let m = format!(
                        "Bad value {} for \"{}\" tag ignored",
                        v as u32,
                        self.field_name(t, s)
                    );
                    self.warning(Some(&self.name.clone()), &m, s);
                } else {
                    self.dir.orientation = v as u16;
                }
            }
            tag::SAMPLESPERPIXEL => {
                let v = a.int() as i32;
                if v == 0 {
                    bad = Some(Bad::Int(v));
                } else {
                    self.dir.samplesperpixel = v as u16;
                }
            }
            tag::ROWSPERSTRIP => {
                let v = a.int();
                if v == 0 {
                    bad = Some(Bad::U32(v));
                } else {
                    self.dir.rowsperstrip = v;
                    if !self.dir.is_set(bit::TILEDIMENSIONS) {
                        self.dir.tilelength = v;
                        self.dir.tilewidth = self.dir.imagewidth;
                    }
                }
            }
            tag::MINSAMPLEVALUE => self.dir.minsamplevalue = a.int() as u16,
            tag::MAXSAMPLEVALUE => self.dir.maxsamplevalue = a.int() as u16,
            tag::SMINSAMPLEVALUE => self.dir.sminsamplevalue = a.dbl(),
            tag::SMAXSAMPLEVALUE => self.dir.smaxsamplevalue = a.dbl(),
            tag::XRESOLUTION => self.dir.xresolution = a.dbl() as f32,
            tag::YRESOLUTION => self.dir.yresolution = a.dbl() as f32,
            tag::PLANARCONFIG => {
                let v = a.int() as i32;
                if v != 1 && v != 2 {
                    bad = Some(Bad::Int(v));
                } else {
                    self.dir.planarconfig = v as u16;
                }
            }
            tag::XPOSITION => self.dir.xposition = a.dbl() as f32,
            tag::YPOSITION => self.dir.yposition = a.dbl() as f32,
            tag::RESOLUTIONUNIT => {
                let v = a.int() as i32;
                if !(1..=3).contains(&v) {
                    bad = Some(Bad::Int(v));
                } else {
                    self.dir.resolutionunit = v as u16;
                }
            }
            tag::PAGENUMBER => {
                if let Arg::Int2(x, y) = a {
                    self.dir.pagenumber = [x as u16, y as u16];
                }
            }
            tag::HALFTONEHINTS => {
                if let Arg::Int2(x, y) = a {
                    self.dir.halftonehints = [x as u16, y as u16];
                }
            }
            tag::COLORMAP | tag::TRANSFERFUNCTION => {}
            tag::EXTRASAMPLES => {
                // setExtraSamples.
                let (v, arr) = match a {
                    Arg::Count(n, Data::U16(x)) => (n as i32, Some(x)),
                    Arg::Count(n, _) => (n as i32, None),
                    _ => (a.int() as i32, None),
                };
                let mut ok = (v as u16) <= self.dir.samplesperpixel;
                if ok && v > 0 && arr.is_none() {
                    ok = false;
                }
                if ok {
                    let arr = arr.unwrap_or(&[]);
                    for i in 0..v.max(0) as usize {
                        if arr.get(i).copied().unwrap_or(0) > values::EXTRASAMPLE_UNASSALPHA {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        self.dir.extrasamples = v as u16;
                        self.dir.sampleinfo = arr
                            .iter()
                            .copied()
                            .take(usize::from(v as u16))
                            .collect();
                    }
                }
                if !ok {
                    bad = Some(Bad::Int(v));
                }
            }
            tag::MATTEING => {
                self.dir.extrasamples = u16::from(a.int() as i32 != 0);
                if self.dir.extrasamples != 0 {
                    self.dir.sampleinfo = vec![values::EXTRASAMPLE_ASSOCALPHA];
                }
            }
            tag::TILEWIDTH | tag::TILELENGTH => {
                let v = a.int();
                if v % 16 != 0 {
                    if self.mode != Mode::RdOnly {
                        bad = Some(Bad::U32(v));
                    } else {
                        let what = if t == tag::TILEWIDTH {
                            "width"
                        } else {
                            "length"
                        };
                        let m = format!("Nonstandard tile {what} {}, convert file", v as i32);
                        self.warning(Some(&self.name.clone()), &m, s);
                    }
                }
                if bad.is_none() {
                    if t == tag::TILEWIDTH {
                        self.dir.tilewidth = v;
                    } else {
                        self.dir.tilelength = v;
                    }
                    self.flags |= TIFF_ISTILED;
                }
            }
            tag::TILEDEPTH => {
                let v = a.int();
                if v == 0 {
                    bad = Some(Bad::U32(v));
                } else {
                    self.dir.tiledepth = v;
                }
            }
            tag::DATATYPE => {
                let v = a.int() as i32;
                let f = match v {
                    0 => Some(values::SAMPLEFORMAT_VOID),
                    1 => Some(values::SAMPLEFORMAT_INT),
                    2 => Some(values::SAMPLEFORMAT_UINT),
                    3 => Some(values::SAMPLEFORMAT_IEEEFP),
                    _ => None,
                };
                match f {
                    Some(f) => self.dir.sampleformat = f,
                    None => bad = Some(Bad::Int(v)),
                }
            }
            tag::SAMPLEFORMAT => {
                let v = a.int() as i32;
                if !(1..=6).contains(&v) {
                    bad = Some(Bad::Int(v));
                } else {
                    let d = &mut self.dir;
                    d.sampleformat = v as u16;
                    let pd = &mut self.codec.postdecode;
                    if d.sampleformat == values::SAMPLEFORMAT_COMPLEXINT
                        && d.bitspersample == 32
                        && *pd == PostDecode::Swab32
                    {
                        *pd = PostDecode::Swab16;
                    } else if (d.sampleformat == values::SAMPLEFORMAT_COMPLEXINT
                        || d.sampleformat == values::SAMPLEFORMAT_COMPLEXIEEEFP)
                        && d.bitspersample == 64
                        && *pd == PostDecode::Swab64
                    {
                        *pd = PostDecode::Swab32;
                    } else if d.sampleformat == values::SAMPLEFORMAT_COMPLEXIEEEFP
                        && d.bitspersample == 128
                        && *pd == PostDecode::None
                    {
                        *pd = PostDecode::Swab64;
                    }
                }
            }
            tag::IMAGEDEPTH => self.dir.imagedepth = a.int(),
            tag::STONITS => {
                let d = a.dbl();
                // NaN passes `d <= 0.` as false, as in C.
                if d <= 0.0 {
                    bad = Some(Bad::Dbl(d));
                } else {
                    self.dir.stonits = d;
                }
            }
            tag::PIXAR_IMAGEFULLWIDTH => self.dir.imagefullwidth = a.int(),
            tag::PIXAR_IMAGEFULLLENGTH => self.dir.imagefulllength = a.int(),
            tag::PIXAR_FOVCOT => self.dir.fovcot = a.dbl() as f32,
            tag::PIXAR_MATRIX_WORLDTOSCREEN | tag::PIXAR_MATRIX_WORLDTOCAMERA => {}
            tag::SUBIFD => {
                if self.flags & super::TIFF_INSUBIFD == 0 {
                    let (n, arr) = match a {
                        Arg::Count(n, Data::U32(x)) => (n as u16, x),
                        Arg::Count(n, _) => (n as u16, &[][..]),
                        _ => (a.int() as u16, &[][..]),
                    };
                    self.dir.nsubifd = n;
                    self.dir.subifd = arr.iter().copied().take(usize::from(n)).collect();
                } else {
                    let m = format!("{}: Sorry, cannot nest SubIFDs", self.name);
                    self.error(Some(MODULE), &m, s);
                    status = false;
                }
            }
            tag::YCBCRCOEFFICIENTS
            | tag::WHITEPOINT
            | tag::PRIMARYCHROMATICITIES
            | tag::REFERENCEBLACKWHITE => {}
            tag::YCBCRPOSITIONING => self.dir.ycbcrpositioning = a.int() as u16,
            tag::YCBCRSUBSAMPLING => {
                if let Arg::Int2(x, y) = a {
                    self.dir.ycbcrsubsampling = [x as u16, y as u16];
                }
            }
            tag::INKSET => self.dir.inkset = a.int() as u16,
            tag::DOTRANGE => {
                if let Arg::Int2(x, y) = a {
                    self.dir.dotrange = [x as u16, y as u16];
                }
            }
            tag::INKNAMES => {
                let (n, bytes) = match a {
                    Arg::Count(n, Data::Bytes(b)) => (n as i32, b),
                    _ => (a.int() as i32, &[][..]),
                };
                let i = self.check_ink_names(n, bytes, s);
                status = i > 0;
            }
            tag::NUMBEROFINKS => self.dir.ninks = a.int() as u16,
            tag::ICCPROFILE | tag::PHOTOSHOP | tag::RICHTIFFIPTC | tag::XMLPACKET => {}
            _ => {
                let fip = self.fields.find(t);
                match fip {
                    Some(f) if f.bit == bit::CUSTOM => {
                        if !f.passcount && f.ty != super::fields::TIFF_ASCII {
                            self.warning(Some(MODULE), " ... pass by value not implemented.", s);
                            status = false;
                        }
                        if !self.dir.custom.iter().any(|&(tg, _)| tg == f.tag) {
                            self.dir.custom.push((f.tag, f.ty));
                        }
                    }
                    _ => {
                        let p = if t > 0xffff { "pseudo-" } else { "" };
                        let m = format!(
                            "{}: Invalid {p}tag \"{}\" (not supported by codec)",
                            self.name,
                            self.field_name(t, s)
                        );
                        self.error(Some(MODULE), &m, s);
                        status = false;
                    }
                }
            }
        }
        if let Some(b) = bad {
            let name = cut(&self.name, 1000).to_owned();
            let fname = self.field_name(t, s);
            let m = match b {
                Bad::Int(v) => format!("{name}: Bad value {v} for \"{fname}\""),
                Bad::U32(v) => format!("{name}: Bad value {v} for \"{fname}\""),
                Bad::Dbl(d) => format!("{name}: Bad value {} for \"{fname}\"", cfmt::f(d, 6)),
            };
            self.error(Some(MODULE), &m, s);
            return false;
        }
        if status {
            if let Some(f) = self.field_with_tag(t, s) {
                self.dir.set_bit(f.bit);
            }
            self.flags |= TIFF_DIRTYDIRECT;
        }
        status
    }

    /// `checkInkNamesString`: the length used, or 0 with its message.
    fn check_ink_names(&self, slen: i32, s0: &[u8], s: &mut dyn Streams) -> i32 {
        let mut i = i32::from(self.dir.samplesperpixel);
        if slen > 0 {
            let ep = slen as usize;
            let mut cp = 0usize;
            let mut ok = true;
            'outer: while i > 0 {
                loop {
                    if cp >= s0.len() || s0[cp] == 0 {
                        break;
                    }
                    if cp >= ep {
                        ok = false;
                        break 'outer;
                    }
                    cp += 1;
                }
                cp += 1;
                i -= 1;
            }
            if ok {
                return cp as i32;
            }
        }
        let spp = i32::from(self.dir.samplesperpixel);
        let m = format!(
            "{}: Invalid InkNames value; expecting {spp} names, found {}",
            self.name,
            spp - i
        );
        self.error(Some("TIFFSetField"), &m, s);
        0
    }

    /// `TIFFGetField(tif, tag, ...)`: the value, or `None` when the C function returns 0.
    pub fn get_field(&mut self, t: u32, s: &mut dyn Streams) -> Option<Value> {
        let fip = self.fields.find(t)?;
        if !(t > 0xffff || self.dir.is_set(fip.bit)) {
            return None;
        }
        self.stale_layers_notice("TIFFGetField");
        for l in self.codec.layers.clone() {
            match l {
                Layer::Zip if t == tag::ZIPQUALITY => {
                    return Some(Value::I32(self.codec.zip_quality()));
                }
                Layer::Predictor if t == tag::PREDICTOR => {
                    return Some(Value::U16(self.codec.predictor.predictor as u16));
                }
                _ => {}
            }
        }
        self.vgetfield(t, s)
    }

    /// `_TIFFVGetField`.
    fn vgetfield(&mut self, t: u32, s: &mut dyn Streams) -> Option<Value> {
        let d = &self.dir;
        Some(match t {
            tag::SUBFILETYPE => Value::U32(d.subfiletype),
            tag::IMAGEWIDTH => Value::U32(d.imagewidth),
            tag::IMAGELENGTH => Value::U32(d.imagelength),
            tag::BITSPERSAMPLE => Value::U16(d.bitspersample),
            tag::COMPRESSION => Value::U16(d.compression),
            tag::PHOTOMETRIC => Value::U16(d.photometric),
            tag::THRESHHOLDING => Value::U16(d.threshholding),
            tag::FILLORDER => Value::U16(d.fillorder),
            tag::ORIENTATION => Value::U16(d.orientation),
            tag::SAMPLESPERPIXEL => Value::U16(d.samplesperpixel),
            tag::ROWSPERSTRIP => Value::U32(d.rowsperstrip),
            tag::MINSAMPLEVALUE => Value::U16(d.minsamplevalue),
            tag::MAXSAMPLEVALUE => Value::U16(d.maxsamplevalue),
            tag::SMINSAMPLEVALUE => Value::F64(d.sminsamplevalue),
            tag::SMAXSAMPLEVALUE => Value::F64(d.smaxsamplevalue),
            tag::XRESOLUTION => Value::F32(d.xresolution),
            tag::YRESOLUTION => Value::F32(d.yresolution),
            tag::PLANARCONFIG => Value::U16(d.planarconfig),
            tag::XPOSITION => Value::F32(d.xposition),
            tag::YPOSITION => Value::F32(d.yposition),
            tag::RESOLUTIONUNIT => Value::U16(d.resolutionunit),
            tag::PAGENUMBER => Value::U16Pair(d.pagenumber[0], d.pagenumber[1]),
            tag::HALFTONEHINTS => Value::U16Pair(d.halftonehints[0], d.halftonehints[1]),
            tag::STRIPOFFSETS | tag::TILEOFFSETS => {
                Value::U32Array(d.stripoffset.clone().unwrap_or_default())
            }
            tag::STRIPBYTECOUNTS | tag::TILEBYTECOUNTS => {
                Value::U32Array(d.stripbytecount.clone().unwrap_or_default())
            }
            tag::MATTEING => Value::U16(u16::from(
                d.extrasamples == 1 && d.sampleinfo.first() == Some(&1),
            )),
            tag::EXTRASAMPLES => Value::ExtraSamples(d.extrasamples, d.sampleinfo.clone()),
            tag::TILEWIDTH => Value::U32(d.tilewidth),
            tag::TILELENGTH => Value::U32(d.tilelength),
            tag::TILEDEPTH => Value::U32(d.tiledepth),
            tag::DATATYPE => match d.sampleformat {
                values::SAMPLEFORMAT_UINT => Value::U16(2),
                values::SAMPLEFORMAT_INT => Value::U16(1),
                values::SAMPLEFORMAT_IEEEFP => Value::U16(3),
                values::SAMPLEFORMAT_VOID => Value::U16(0),
                _ => Value::Present,
            },
            tag::SAMPLEFORMAT => Value::U16(d.sampleformat),
            tag::IMAGEDEPTH => Value::U32(d.imagedepth),
            tag::STONITS => Value::F64(d.stonits),
            tag::YCBCRPOSITIONING => Value::U16(d.ycbcrpositioning),
            tag::YCBCRSUBSAMPLING => Value::U16Pair(d.ycbcrsubsampling[0], d.ycbcrsubsampling[1]),
            tag::INKSET => Value::U16(d.inkset),
            tag::DOTRANGE => Value::U16Pair(d.dotrange[0], d.dotrange[1]),
            tag::NUMBEROFINKS => Value::U16(d.ninks),
            tag::PIXAR_IMAGEFULLWIDTH => Value::U32(d.imagefullwidth),
            tag::PIXAR_IMAGEFULLLENGTH => Value::U32(d.imagefulllength),
            tag::PIXAR_FOVCOT => Value::F32(d.fovcot),
            tag::SUBIFD => Value::U32Array(d.subifd.clone()),
            tag::DOCUMENTNAME
            | tag::ARTIST
            | tag::DATETIME
            | tag::HOSTCOMPUTER
            | tag::IMAGEDESCRIPTION
            | tag::MAKE
            | tag::MODEL
            | tag::COPYRIGHT
            | tag::PAGENAME
            | tag::COLORMAP
            | tag::YCBCRCOEFFICIENTS
            | tag::WHITEPOINT
            | tag::PRIMARYCHROMATICITIES
            | tag::TRANSFERFUNCTION
            | tag::REFERENCEBLACKWHITE
            | tag::INKNAMES
            | tag::TARGETPRINTER
            | tag::ICCPROFILE
            | tag::PHOTOSHOP
            | tag::RICHTIFFIPTC
            | tag::XMLPACKET
            | tag::PIXAR_TEXTUREFORMAT
            | tag::PIXAR_WRAPMODES
            | tag::PIXAR_MATRIX_WORLDTOSCREEN
            | tag::PIXAR_MATRIX_WORLDTOCAMERA => Value::Present,
            _ => {
                let fip = self.fields.find(t);
                match fip {
                    Some(f) if f.bit == bit::CUSTOM => {
                        let found = self.dir.custom.iter().any(|&(tg, _)| tg == t);
                        if !found {
                            return None;
                        }
                        if f.passcount || f.ty == super::fields::TIFF_ASCII {
                            Value::Present
                        } else {
                            s.out("TIFFVGetField ... pass by value not imp.\n");
                            return None;
                        }
                    }
                    _ => {
                        let p = if t > 0xffff { "pseudo-" } else { "" };
                        let m = format!(
                            "{}: Invalid {p}tag \"{}\" (not supported by codec)",
                            self.name,
                            self.field_name(t, s)
                        );
                        self.error(Some("_TIFFVGetField"), &m, s);
                        return None;
                    }
                }
            }
        })
    }

    /// `TIFFGetFieldDefaulted(tif, tag, ...)`.
    pub fn get_field_defaulted(&mut self, t: u32, s: &mut dyn Streams) -> Option<Value> {
        if let Some(v) = self.get_field(t, s) {
            return Some(v);
        }
        let d = &self.dir;
        Some(match t {
            tag::SUBFILETYPE => Value::U32(d.subfiletype),
            tag::BITSPERSAMPLE => Value::U16(d.bitspersample),
            tag::THRESHHOLDING => Value::U16(d.threshholding),
            tag::FILLORDER => Value::U16(d.fillorder),
            tag::ORIENTATION => Value::U16(d.orientation),
            tag::SAMPLESPERPIXEL => Value::U16(d.samplesperpixel),
            tag::ROWSPERSTRIP => Value::U32(d.rowsperstrip),
            tag::MINSAMPLEVALUE => Value::U16(d.minsamplevalue),
            tag::MAXSAMPLEVALUE => Value::U16(d.maxsamplevalue),
            tag::PLANARCONFIG => Value::U16(d.planarconfig),
            tag::RESOLUTIONUNIT => Value::U16(d.resolutionunit),
            // libtiff reads the codec's state, whatever it is (a crash with none): only codecs
            // with a predictor answer here.
            tag::PREDICTOR if self.codec.has_predictor() => {
                Value::U16(self.codec.predictor.predictor as u16)
            }
            tag::DOTRANGE => Value::U16Pair(0, ((1i64 << (d.bitspersample & 63)) - 1) as u16),
            tag::INKSET => Value::U16(d.inkset),
            tag::NUMBEROFINKS => Value::U16(d.ninks),
            tag::EXTRASAMPLES => Value::ExtraSamples(d.extrasamples, d.sampleinfo.clone()),
            tag::MATTEING => Value::U16(u16::from(
                d.extrasamples == 1 && d.sampleinfo.first() == Some(&1),
            )),
            tag::TILEDEPTH => Value::U32(d.tiledepth),
            tag::DATATYPE => Value::U16(d.sampleformat.wrapping_sub(1)),
            tag::SAMPLEFORMAT => Value::U16(d.sampleformat),
            tag::IMAGEDEPTH => Value::U32(d.imagedepth),
            tag::YCBCRSUBSAMPLING => Value::U16Pair(d.ycbcrsubsampling[0], d.ycbcrsubsampling[1]),
            tag::YCBCRPOSITIONING => Value::U16(d.ycbcrpositioning),
            tag::YCBCRCOEFFICIENTS
            | tag::WHITEPOINT
            | tag::TRANSFERFUNCTION
            | tag::REFERENCEBLACKWHITE => Value::Present,
            _ => return None,
        })
    }
}

use super::cut;
