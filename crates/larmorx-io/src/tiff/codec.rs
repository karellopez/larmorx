// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! The codecs: `tif_compress.c` (the method table's defaults and `TIFFSetCompressionScheme`),
//! `tif_codec.c` (the built-in schemes and `NotConfigured`), `tif_dumpmode.c`,
//! `tif_packbits.c`, `tif_lzw.c` (the decoders; encoding is refused, as libtiff 3.6.1 refuses
//! it), `tif_zip.c` (over [`larmorx_core::zlib`]) and `tif_predict.c`. The method pointers
//! of `struct tiff` become the enums below.

use super::dir::{Layer, PostDecode};
use super::fax3::{Fax, FaxKind};
use super::pixarlog::PixarLog;
use super::fields::Table;
use super::{TIFF_NOBITREV, Tiff, Unported, compression as c};
use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{FS_817, FS_818, FS_819, FS_831, FS_832, FS_833};
use larmorx_core::zlib::{self as z, Deflate, Inflate};

/// `tif_setupdecode`'s codec part (the predictor wraps it: [`Codec::pred_setup`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SetupDecode {
    #[default]
    True,
    NotConfigured,
    Lzw,
    Zip,
    Fax,
    PixarLog,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PreDecode {
    #[default]
    None,
    Lzw,
    Zip,
    Fax,
    PixarLog,
}

/// `tif_decoderow`, `tif_decodestrip`, `tif_decodetile`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decode {
    NoRow,
    NoStrip,
    NoTile,
    Dump,
    PackBits,
    Lzw,
    LzwCompat,
    Zip,
    Fax(FaxKind),
    Next,
    Thunder,
    PixarLog,
    PredRow,
    PredTile,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SeekM {
    #[default]
    No,
    Dump,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Cleanup {
    #[default]
    Void,
    Lzw,
    Zip,
    Fax,
    PixarLog,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SetupEncode {
    #[default]
    True,
    NotConfigured,
    LzwRefused,
    Zip,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PreEncode {
    #[default]
    None,
    PackBits,
    Zip,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PostEncode {
    #[default]
    True,
    Zip,
}

/// `tif_encoderow`, `tif_encodestrip`, `tif_encodetile`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Encode {
    NoRow,
    NoStrip,
    NoTile,
    Dump,
    PackBits,
    PackBitsChunk,
    Zip,
    PredRow,
    PredTile,
}

/// The predictor's routine (`pfunc`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PFunc {
    HorAcc8,
    HorAcc16,
    SwabHorAcc16,
    HorDiff8,
    HorDiff16,
}

/// `TIFFPredictorState`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Predictor {
    pub predictor: i32,
    pub stride: i32,
    pub rowsize: i32,
    pub pfunc: Option<PFunc>,
    pub coderow: Decode,
    pub codestrip: Decode,
    pub codetile: Decode,
    pub ecoderow: Encode,
    pub ecodestrip: Encode,
    pub ecodetile: Encode,
}

impl Default for Predictor {
    fn default() -> Predictor {
        Predictor {
            predictor: 1,
            stride: 0,
            rowsize: 0,
            pfunc: None,
            coderow: Decode::NoRow,
            codestrip: Decode::NoStrip,
            codetile: Decode::NoTile,
            ecoderow: Encode::NoRow,
            ecodestrip: Encode::NoStrip,
            ecodetile: Encode::NoTile,
        }
    }
}

const BITS_MIN: i64 = 9;
const BITS_MAX: i64 = 12;
const CODE_CLEAR: u16 = 256;
const CODE_EOI: u16 = 257;
const CODE_FIRST: i32 = 258;
const CSIZE: i32 = 4095 + 1024;
/// A null `next` pointer.
const NIL: i32 = i32::MIN;

/// `code_t`.
#[derive(Clone, Copy, Debug)]
struct CodeEnt {
    next: i32,
    length: u16,
    value: u8,
    firstchar: u8,
}

/// An entry set to zero bytes (`next` a null pointer).
const ZERO: CodeEnt = CodeEnt {
    next: NIL,
    length: 0,
    value: 0,
    firstchar: 0,
};

/// `LZWCodecState`'s decoding part.
#[derive(Clone, Debug)]
pub(crate) struct Lzw {
    nbits: u16,
    nextdata: i64,
    nextbits: i64,
    dec_nbitsmask: i64,
    dec_restart: i64,
    dec_bitsleft: i64,
    /// `dec_decode`: none yet, `LZWDecode`, `LZWDecodeCompat`.
    dec_decode: Option<Decode>,
    dec_codep: i32,
    dec_oldcodep: i32,
    dec_free_entp: i32,
    dec_maxcodep: i32,
    codetab: Option<Vec<CodeEnt>>,
    /// `LZWPreDecode` has run (the state is set).
    started: bool,
}

impl Lzw {
    fn new() -> Lzw {
        Lzw {
            nbits: 0,
            nextdata: 0,
            nextbits: 0,
            dec_nbitsmask: 0,
            dec_restart: 0,
            dec_bitsleft: 0,
            dec_decode: None,
            dec_codep: 0,
            dec_oldcodep: -1,
            dec_free_entp: 0,
            dec_maxcodep: 0,
            codetab: None,
            started: false,
        }
    }
}

/// `ZIPState`.
pub(crate) struct Zip {
    zipquality: i32,
    init: bool,
    inflate: Option<Inflate>,
    deflate: Option<Deflate>,
    /// The stream's input: an offset into the raw buffer and the bytes left (`next_in`,
    /// `avail_in`).
    in_at: usize,
    in_left: u32,
    /// The encoder's output: an offset into the raw buffer and the space left.
    out_at: usize,
    out_left: u32,
    /// `ZIPPreDecode` has run (the stream's input is set).
    started: bool,
}

/// The codec part of `struct tiff`: its method table and private state.
pub(crate) struct Codec {
    pub decodestatus: bool,
    pub setupdecode: SetupDecode,
    /// `PredictorSetupDecode` and `PredictorSetupEncode` wrap the codec's setup.
    pub pred_setup: bool,
    pub predecode: PreDecode,
    pub decoderow: Decode,
    pub decodestrip: Decode,
    pub decodetile: Decode,
    pub seek: SeekM,
    pub cleanup: Cleanup,
    pub encodestatus: bool,
    pub setupencode: SetupEncode,
    pub preencode: PreEncode,
    pub postencode: PostEncode,
    pub encoderow: Encode,
    pub encodestrip: Encode,
    pub encodetile: Encode,
    pub postdecode: PostDecode,
    pub layers: Vec<Layer>,
    pub layers_stale: bool,
    pub predictor: Predictor,
    pub has_pred: bool,
    pub lzw: Option<Box<Lzw>>,
    pub zip: Option<Box<Zip>>,
    pub fax: Option<Box<Fax>>,
    pub pixarlog: Option<Box<PixarLog>>,
    /// PackBits' row size for its encoder (`tif_data`).
    pub packbits_rowsize: i32,
}

impl Default for Codec {
    fn default() -> Codec {
        Codec {
            decodestatus: true,
            setupdecode: SetupDecode::True,
            pred_setup: false,
            predecode: PreDecode::None,
            decoderow: Decode::NoRow,
            decodestrip: Decode::NoStrip,
            decodetile: Decode::NoTile,
            seek: SeekM::No,
            cleanup: Cleanup::Void,
            encodestatus: true,
            setupencode: SetupEncode::True,
            preencode: PreEncode::None,
            postencode: PostEncode::True,
            encoderow: Encode::NoRow,
            encodestrip: Encode::NoStrip,
            encodetile: Encode::NoTile,
            postdecode: PostDecode::None,
            layers: Vec::new(),
            layers_stale: false,
            predictor: Predictor::default(),
            has_pred: false,
            lzw: None,
            zip: None,
            fax: None,
            pixarlog: None,
            packbits_rowsize: 0,
        }
    }
}

impl Codec {
    pub(crate) fn has_predictor(&self) -> bool {
        self.has_pred
    }
    pub(crate) fn zip_quality(&self) -> i32 {
        self.zip.as_ref().map(|z| z.zipquality).unwrap_or(0)
    }
}

/// The built-in schemes (`_TIFFBuiltinCODECS`): the name, or `None` for no codec.
pub(crate) fn codec_name(scheme: u16) -> Option<&'static str> {
    Some(match scheme {
        c::NONE => "None",
        c::LZW => "LZW",
        c::PACKBITS => "PackBits",
        c::THUNDERSCAN => "ThunderScan",
        c::NEXT => "NeXT",
        c::JPEG => "JPEG",
        c::OJPEG => "Old-style JPEG",
        c::CCITTRLE => "CCITT RLE",
        c::CCITTRLEW => "CCITT RLE/W",
        c::CCITTFAX3 => "CCITT Group 3",
        c::CCITTFAX4 => "CCITT Group 4",
        c::JBIG => "ISO JBIG",
        c::DEFLATE => "Deflate",
        c::ADOBE_DEFLATE => "AdobeDeflate",
        c::PIXARLOG => "PixarLog",
        c::SGILOG => "SGILog",
        c::SGILOG24 => "SGILog24",
        _ => return None,
    })
}

impl Tiff {
    /// `_TIFFSetDefaultCompressionState(tif)`.
    pub(crate) fn set_default_compression_state(&mut self) {
        let k = &mut self.codec;
        k.decodestatus = true;
        k.setupdecode = SetupDecode::True;
        k.pred_setup = false;
        k.predecode = PreDecode::None;
        k.decoderow = Decode::NoRow;
        k.decodestrip = Decode::NoStrip;
        k.decodetile = Decode::NoTile;
        k.encodestatus = true;
        k.setupencode = SetupEncode::True;
        k.preencode = PreEncode::None;
        k.postencode = PostEncode::True;
        k.encoderow = Encode::NoRow;
        k.encodestrip = Encode::NoStrip;
        k.encodetile = Encode::NoTile;
        k.seek = SeekM::No;
        k.cleanup = Cleanup::Void;
        self.flags &= !TIFF_NOBITREV;
    }

    /// `TIFFSetCompressionScheme(tif, scheme)`: the codec's init status.
    pub(crate) fn set_compression_scheme(&mut self, scheme: u16, s: &mut dyn Streams) -> bool {
        self.set_default_compression_state();
        let k = &mut self.codec;
        match scheme {
            c::NONE => {
                k.decoderow = Decode::Dump;
                k.decodestrip = Decode::Dump;
                k.decodetile = Decode::Dump;
                k.encoderow = Encode::Dump;
                k.encodestrip = Encode::Dump;
                k.encodetile = Encode::Dump;
                k.seek = SeekM::Dump;
            }
            c::PACKBITS => {
                k.decoderow = Decode::PackBits;
                k.decodestrip = Decode::PackBits;
                k.decodetile = Decode::PackBits;
                k.preencode = PreEncode::PackBits;
                k.encoderow = Encode::PackBits;
                k.encodestrip = Encode::PackBitsChunk;
                k.encodetile = Encode::PackBitsChunk;
            }
            c::LZW => {
                k.lzw = Some(Box::new(Lzw::new()));
                k.setupencode = SetupEncode::LzwRefused;
                k.setupdecode = SetupDecode::Lzw;
                k.predecode = PreDecode::Lzw;
                k.decoderow = Decode::Lzw;
                k.decodestrip = Decode::Lzw;
                k.decodetile = Decode::Lzw;
                k.cleanup = Cleanup::Lzw;
                self.predictor_init();
            }
            c::DEFLATE | c::ADOBE_DEFLATE => {
                k.zip = Some(Box::new(Zip {
                    zipquality: z::Z_DEFAULT_COMPRESSION,
                    init: false,
                    inflate: None,
                    deflate: None,
                    in_at: 0,
                    in_left: 0,
                    out_at: 0,
                    out_left: 0,
                    started: false,
                }));
                self.fields.merge(Table::Zip);
                let k = &mut self.codec;
                k.layers.insert(0, Layer::Zip);
                k.setupdecode = SetupDecode::Zip;
                k.predecode = PreDecode::Zip;
                k.decoderow = Decode::Zip;
                k.decodestrip = Decode::Zip;
                k.decodetile = Decode::Zip;
                k.setupencode = SetupEncode::Zip;
                k.preencode = PreEncode::Zip;
                k.postencode = PostEncode::Zip;
                k.encoderow = Encode::Zip;
                k.encodestrip = Encode::Zip;
                k.encodetile = Encode::Zip;
                k.cleanup = Cleanup::Zip;
                self.predictor_init();
            }
            c::THUNDERSCAN => {
                k.decoderow = Decode::Thunder;
                k.decodestrip = Decode::Thunder;
            }
            c::NEXT => {
                k.decoderow = Decode::Next;
                k.decodestrip = Decode::Next;
                k.decodetile = Decode::Next;
            }
            c::JPEG | c::OJPEG | c::JBIG => {
                k.decodestatus = false;
                k.setupdecode = SetupDecode::NotConfigured;
                k.encodestatus = false;
                k.setupencode = SetupEncode::NotConfigured;
            }
            c::CCITTRLE | c::CCITTRLEW | c::CCITTFAX3 | c::CCITTFAX4 => {
                return self.fax_init(scheme, s);
            }
            c::PIXARLOG => self.pixarlog_init(),
            c::SGILOG | c::SGILOG24 => {
                // These codecs register private tags and tag methods at this point; not
                // ported yet.
                self.unported = Some(Unported(codec_name(scheme).unwrap_or("?")));
            }
            _ => {}
        }
        true
    }

    /// `TIFFPredictorInit(tif)`.
    pub(crate) fn predictor_init(&mut self) {
        self.fields.merge(Table::Predict);
        let k = &mut self.codec;
        k.layers.insert(0, Layer::Predictor);
        k.pred_setup = true;
        k.has_pred = true;
        k.predictor = Predictor::default();
    }

    /// `(*tif->tif_cleanup)(tif)`: the codec's state freed. libtiff 3.6.1 leaves the codec's
    /// tag methods in place (FS-816); the port drops them.
    pub(crate) fn cleanup(&mut self) {
        let k = &mut self.codec;
        match k.cleanup {
            Cleanup::Void => {}
            Cleanup::Lzw => {
                k.lzw = None;
                k.has_pred = false;
            }
            Cleanup::Zip => {
                k.zip = None;
                k.has_pred = false;
            }
            Cleanup::Fax => k.fax = None,
            Cleanup::PixarLog => {
                k.pixarlog = None;
                k.has_pred = false;
            }
        }
        k.layers.clear();
    }

    // ---- decoding ----

    /// `(*tif->tif_setupdecode)(tif)`.
    pub(crate) fn setup_decode(&mut self, s: &mut dyn Streams) -> bool {
        if !self.setup_decode_inner(s) {
            return false;
        }
        if self.codec.pred_setup {
            return self.predictor_setup_decode(s);
        }
        true
    }

    fn setup_decode_inner(&mut self, s: &mut dyn Streams) -> bool {
        match self.codec.setupdecode {
            SetupDecode::True => true,
            SetupDecode::NotConfigured => {
                let n = codec_name(self.dir.compression).unwrap_or("");
                let m = format!("{n} compression support is not configured");
                self.error(Some(&self.name.clone()), &m, s);
                false
            }
            SetupDecode::Lzw => {
                let sp = self.codec.lzw.as_mut().expect("LZW state");
                if sp.codetab.is_none() {
                    // Entries 256 and 257 are never written (malloc's memory): modelled as zero
                    // bytes (FS-818 when used).
                    let mut t = vec![ZERO; CSIZE as usize];
                    for (code, e) in t.iter_mut().enumerate().take(256) {
                        e.value = code as u8;
                        e.firstchar = code as u8;
                        e.length = 1;
                    }
                    sp.codetab = Some(t);
                }
                true
            }
            SetupDecode::Fax => self.fax_setup(s),
            SetupDecode::PixarLog => self.pixarlog_setup(s),
            SetupDecode::Zip => {
                let sp = self.codec.zip.as_mut().expect("ZIP state");
                match Inflate::init() {
                    Ok(i) => {
                        sp.inflate = Some(i);
                        sp.init = true;
                        true
                    }
                    Err(_) => {
                        let m = format!("{}: (null)", self.name);
                        self.error(Some("ZIPSetupDecode"), &m, s);
                        false
                    }
                }
            }
        }
    }

    /// `PredictorSetup(tif)`.
    fn predictor_setup(&mut self, s: &mut dyn Streams) -> bool {
        let sp = &self.codec.predictor;
        if sp.predictor == 1 {
            return true;
        }
        if sp.predictor != 2 {
            let m = format!("\"Predictor\" value {} not supported", sp.predictor);
            self.error(Some(&self.name.clone()), &m, s);
            return false;
        }
        let bps = self.dir.bitspersample;
        if bps != 8 && bps != 16 {
            let m = format!(
                "Horizontal differencing \"Predictor\" not supported with {bps}-bit samples"
            );
            self.error(Some(&self.name.clone()), &m, s);
            return false;
        }
        let stride = if self.dir.planarconfig == super::values::PLANARCONFIG_CONTIG {
            i32::from(self.dir.samplesperpixel)
        } else {
            1
        };
        let rowsize = if self.is_tiled() {
            self.tile_row_size()
        } else {
            self.scanline_size()
        };
        let sp = &mut self.codec.predictor;
        sp.stride = stride;
        sp.rowsize = rowsize;
        true
    }

    /// `PredictorSetupDecode(tif)` after the codec's setup.
    fn predictor_setup_decode(&mut self, s: &mut dyn Streams) -> bool {
        if !self.predictor_setup(s) {
            return false;
        }
        if self.codec.predictor.predictor == 2 {
            let k = &mut self.codec;
            k.predictor.pfunc = match self.dir.bitspersample {
                8 => Some(PFunc::HorAcc8),
                16 => Some(PFunc::HorAcc16),
                _ => k.predictor.pfunc,
            };
            k.predictor.coderow = k.decoderow;
            k.decoderow = Decode::PredRow;
            k.predictor.codestrip = k.decodestrip;
            k.decodestrip = Decode::PredTile;
            k.predictor.codetile = k.decodetile;
            k.decodetile = Decode::PredTile;
            if self.flags & super::TIFF_SWAB != 0 && k.predictor.pfunc == Some(PFunc::HorAcc16) {
                k.predictor.pfunc = Some(PFunc::SwabHorAcc16);
                k.postdecode = PostDecode::None;
            }
        }
        true
    }

    /// `(*tif->tif_predecode)(tif, sample)`.
    pub(crate) fn pre_decode(&mut self, s: &mut dyn Streams) -> bool {
        match self.codec.predecode {
            PreDecode::None => true,
            PreDecode::Lzw => self.lzw_pre_decode(s),
            PreDecode::Fax => self.fax_pre_decode(),
            PreDecode::PixarLog => self.pixarlog_pre_decode(),
            PreDecode::Zip => {
                let rawcc = self.rawcc;
                let sp = self.codec.zip.as_mut().expect("ZIP state");
                sp.in_at = 0;
                sp.in_left = rawcc as u32;
                sp.started = true;
                sp.inflate.as_mut().expect("inflate").reset() == z::Z_OK
            }
        }
    }

    /// FS-831: a decoder called before its strip was started (the strip's start failed in the
    /// codec's setup and the next row is read without a new start): libtiff's decoder state
    /// was never set.
    fn not_started(&mut self, which: &str) -> bool {
        let started = match which {
            "LZW" => self.codec.lzw.as_ref().is_some_and(|l| l.started),
            _ => self
                .codec
                .zip
                .as_ref()
                .is_some_and(|z| z.started && z.inflate.is_some()),
        };
        if !started {
            let name = self.name.clone();
            self.notices.add(
                &FS_831,
                format!(
                    "{name}: the {which} decoder runs on a strip it never started (its setup \
                     failed at the strip's start): libtiff 3.6.1 uses state never set (a crash); \
                     nothing decoded"
                ),
            );
        }
        !started
    }

    /// One of the decoding methods into `buf[..cc]`.
    pub(crate) fn decode(
        &mut self,
        m: Decode,
        buf: &mut [u8],
        cc: i32,
        s: &mut dyn Streams,
    ) -> i32 {
        if matches!(m, Decode::Lzw | Decode::LzwCompat) && self.not_started("LZW") {
            return 0;
        }
        if m == Decode::Zip && self.not_started("Deflate") {
            return 0;
        }
        if cc < 0 && matches!(m, Decode::Dump | Decode::Zip | Decode::PixarLog) {
            let name = self.name.clone();
            self.notices.add(
                &FS_833,
                format!(
                    "{name}: a negative size of {cc} bytes (a size that overflows 32 bits, kept \
                     after a TIFFReadDirectory that failed): libtiff 3.6.1 copies or inflates \
                     about 2^64 bytes (memory past the buffers); nothing written"
                ),
            );
            return i32::from(m == Decode::Dump);
        }
        match m {
            Decode::NoRow => self.no_decode("scanline", s),
            Decode::NoStrip => self.no_decode("strip", s),
            Decode::NoTile => self.no_decode("tile", s),
            Decode::Dump => self.dump_decode(buf, cc, s),
            Decode::PackBits => self.packbits_decode(buf, cc, s),
            Decode::Lzw => self.lzw_decode(buf, cc, s),
            Decode::LzwCompat => self.lzw_decode_compat(buf, cc, s),
            Decode::Zip => self.zip_decode(buf, cc, s),
            Decode::Fax(k) => self.fax_decode(k, buf, cc, s),
            Decode::Next => self.next_decode(buf, cc, s),
            Decode::Thunder => self.thunder_decode_row(buf, cc, s),
            Decode::PixarLog => self.pixarlog_decode(buf, cc, s),
            Decode::PredRow => {
                let inner = self.codec.predictor.coderow;
                if self.decode(inner, buf, cc, s) != 0 {
                    let pf = self.codec.predictor.pfunc;
                    self.pfunc(pf, buf, cc);
                    1
                } else {
                    0
                }
            }
            Decode::PredTile => {
                let inner = self.codec.predictor.codetile;
                if self.decode(inner, buf, cc, s) != 0 {
                    let rowsize = self.codec.predictor.rowsize;
                    let pf = self.codec.predictor.pfunc;
                    let mut occ = i64::from(cc);
                    let mut at = 0usize;
                    if rowsize <= 0 {
                        // libtiff loops forever on a row size of 0 (its assert is compiled
                        // out); never reached by the row sizes PredictorSetup computes from a
                        // directory that has data.
                        return 1;
                    }
                    while occ > 0 {
                        if at >= buf.len() {
                            break;
                        }
                        self.pfunc(pf, &mut buf[at..], rowsize);
                        occ -= i64::from(rowsize);
                        at += rowsize as usize;
                    }
                    1
                } else {
                    0
                }
            }
        }
    }

    /// `TIFFNoDecode(tif, method)`.
    fn no_decode(&self, method: &str, s: &mut dyn Streams) -> i32 {
        let m = match codec_name(self.dir.compression) {
            Some(n) => format!("{n} {method} decoding is not implemented"),
            None => format!(
                "Compression scheme {} {method} decoding is not implemented",
                self.dir.compression
            ),
        };
        self.error(Some(&self.name.clone()), &m, s);
        -1
    }

    /// `(*tif->tif_seek)(tif, nrows)`.
    pub(crate) fn codec_seek(&mut self, nrows: u32, s: &mut dyn Streams) -> bool {
        match self.codec.seek {
            SeekM::No => {
                self.error(
                    Some(&self.name.clone()),
                    "Compression algorithm does not support random access",
                    s,
                );
                false
            }
            SeekM::Dump => {
                let d = i64::from(nrows.wrapping_mul(self.scanlinesize as u32) as i32);
                self.rawcp = (self.rawcp as i64 + d) as usize;
                self.rawcc = self.rawcc.wrapping_sub(d as i32);
                true
            }
        }
    }

    /// `(*tif->tif_postdecode)(tif, buf, cc)`.
    pub(crate) fn post_decode(&mut self, buf: &mut [u8], cc: i32) {
        if cc < 0 && self.codec.postdecode != PostDecode::None {
            let name = self.name.clone();
            self.notices.add(
                &FS_833,
                format!(
                    "{name}: byte-swapping a negative size of {cc} bytes: libtiff 3.6.1 swaps about \
                     2^63 values (memory past the buffer); nothing swapped"
                ),
            );
        }
        let n = (cc.max(0) as usize).min(buf.len());
        let b = &mut buf[..n];
        match self.codec.postdecode {
            PostDecode::None => {}
            PostDecode::Swab16 => b.chunks_exact_mut(2).for_each(|w| w.swap(0, 1)),
            PostDecode::Swab32 => b.chunks_exact_mut(4).for_each(|w| w.reverse()),
            PostDecode::Swab64 => b.chunks_exact_mut(8).for_each(|w| w.reverse()),
        }
    }

    /// `DumpModeDecode`.
    fn dump_decode(&mut self, buf: &mut [u8], cc: i32, s: &mut dyn Streams) -> i32 {
        if self.rawcc < cc {
            let m = format!(
                "DumpModeDecode: Not enough data for scanline {}",
                self.row as i32
            );
            self.error(Some(&self.name.clone()), &m, s);
            return 0;
        }
        let n = cc.max(0) as usize;
        self.copy_raw(buf, n);
        self.rawcp += n;
        self.rawcc -= cc;
        1
    }

    /// Copies `n` raw bytes from `tif_rawcp` into `buf` (bytes past the buffer read as 0, with
    /// FS-817's notice).
    fn copy_raw(&mut self, buf: &mut [u8], n: usize) {
        let n = n.min(buf.len());
        let at = self.rawcp;
        let have = self.raw_defined.saturating_sub(at).min(n);
        if have > 0 {
            buf[..have].copy_from_slice(&self.rawdata[at..at + have]);
        }
        if have < n {
            buf[have..n].fill(0);
            self.past_raw_notice();
        }
    }

    /// FS-819: bytes of libtiff's raw buffer never written since it was allocated (or past
    /// its end) read as 0.
    pub(crate) fn past_raw_notice(&mut self) {
        let name = self.name.clone();
        self.notices.add(
            &FS_819,
            format!(
                "decoding {name} read bytes of libtiff's raw data buffer that were never \
                 written (memory contents): read as 0"
            ),
        );
    }

    /// A raw byte at `at` as the decoders read it: past the strip's `end`, what the buffer
    /// holds from an earlier strip (FS-817), or never written (FS-819: 0).
    pub(crate) fn raw_byte(&mut self, at: usize, end: usize) -> u8 {
        if at >= self.raw_defined {
            self.past_raw_notice();
            return 0;
        }
        if at >= end {
            let name = self.name.clone();
            self.notices.add(
                &FS_817,
                format!(
                    "decoding {name} read past the strip's data: bytes an earlier strip left in \
                     libtiff's raw buffer"
                ),
            );
        }
        self.rawdata.get(at).copied().unwrap_or(0)
    }

    /// `PackBitsDecode`.
    fn packbits_decode(&mut self, op: &mut [u8], occ0: i32, s: &mut dyn Streams) -> i32 {
        let mut bp = self.rawcp;
        let mut cc = self.rawcc;
        let end = self.rawcp + self.rawcc.max(0) as usize;
        let mut occ = i64::from(occ0);
        let mut o = 0usize;
        let fix = self.fix();
        while cc > 0 && occ > 0 {
            let mut n = i64::from(self.raw_byte(bp, end) as i8);
            bp += 1;
            cc -= 1;
            if n < 0 {
                if n == -128 {
                    continue;
                }
                n = -n + 1;
                if occ < n {
                    let m = format!(
                        "PackBitsDecode: discarding {} bytes to avoid buffer overrun",
                        (n - occ) as i32
                    );
                    self.warning(Some(&self.name.clone()), &m, s);
                    n = occ;
                }
                if fix && cc <= 0 {
                    self.notices.add(
                        &FS_817,
                        format!(
                            "a PackBits run in {} ends the strip's data: decoding stopped there \
                             (libtiff 3.6.1 repeats a byte from past the data)",
                            self.name
                        ),
                    );
                    break;
                }
                occ -= n;
                let b = self.raw_byte(bp, end);
                bp += 1;
                cc -= 1;
                for _ in 0..n {
                    if o < op.len() {
                        op[o] = b;
                    }
                    o += 1;
                }
            } else {
                if occ < n + 1 {
                    let m = format!(
                        "PackBitsDecode: discarding {} bytes to avoid buffer overrun",
                        (n - occ + 1) as i32
                    );
                    self.warning(Some(&self.name.clone()), &m, s);
                    n = occ - 1;
                }
                n += 1;
                if fix && i64::from(cc) < n {
                    self.notices.add(
                        &FS_817,
                        format!(
                            "a PackBits literal in {} runs past the strip's data: decoding \
                             stopped there (libtiff 3.6.1 copies bytes from past the data)",
                            self.name
                        ),
                    );
                    break;
                }
                for k in 0..n as usize {
                    let b = self.raw_byte(bp + k, end);
                    if o + k < op.len() {
                        op[o + k] = b;
                    }
                }
                o += n as usize;
                occ -= n;
                bp += n as usize;
                cc = cc.wrapping_sub(n as i32);
            }
        }
        self.rawcp = bp;
        self.rawcc = cc;
        if occ > 0 {
            let m = format!(
                "PackBitsDecode: Not enough data for scanline {}",
                self.row as i32
            );
            self.error(Some(&self.name.clone()), &m, s);
            return 0;
        }
        1
    }

    /// `LZWPreDecode`.
    fn lzw_pre_decode(&mut self, s: &mut dyn Streams) -> bool {
        let end = self.rawcc.max(0) as usize;
        let rawcc = self.rawcc;
        let old_style = if self.fix() && rawcc < 2 {
            // libtiff 4's check: a strip of fewer than two bytes is not old-style.
            if rawcc < 2 {
                self.notices.add(
                    &FS_817,
                    format!(
                        "the LZW strip of {} has fewer than two bytes: not taken for old-style                          codes (libtiff 3.6.1 looks at bytes past its data)",
                        self.name
                    ),
                );
            }
            false
        } else {
            let r0 = self.raw_byte(0, end);
            let r1 = self.raw_byte(1, end);
            r0 == 0 && r1 & 1 != 0
        };
        if old_style {
            if self.codec.lzw.as_ref().expect("LZW").dec_decode.is_none() {
                self.warning(
                    Some(&self.name.clone()),
                    "Old-style LZW codes, convert file",
                    s,
                );
                let k = &mut self.codec;
                k.decoderow = Decode::LzwCompat;
                k.decodestrip = Decode::LzwCompat;
                k.decodetile = Decode::LzwCompat;
                self.setup_decode(s);
                self.codec.lzw.as_mut().expect("LZW").dec_decode = Some(Decode::LzwCompat);
            }
        } else {
            self.codec.lzw.as_mut().expect("LZW").dec_decode = Some(Decode::Lzw);
        }
        let sp = self.codec.lzw.as_mut().expect("LZW");
        sp.nbits = BITS_MIN as u16;
        sp.nextbits = 0;
        sp.nextdata = 0;
        sp.dec_restart = 0;
        sp.dec_nbitsmask = (1 << BITS_MIN) - 1;
        sp.dec_bitsleft = i64::from(rawcc) << 3;
        sp.dec_free_entp = CODE_FIRST;
        if let Some(t) = sp.codetab.as_mut() {
            for e in &mut t[CODE_FIRST as usize..] {
                *e = ZERO;
            }
        }
        sp.dec_oldcodep = -1;
        sp.dec_maxcodep = (sp.dec_nbitsmask - 1) as i32;
        sp.started = true;
        true
    }

    /// `codeLoop`.
    fn code_loop(&self, s: &mut dyn Streams) {
        let m = format!(
            "LZWDecode: Bogus encoding, loop in the code table; scanline {}",
            self.row as i32
        );
        self.error(Some(&self.name.clone()), &m, s);
    }

    fn lzw_error(&self, what: &str, compat: bool, s: &mut dyn Streams) {
        let f = if compat {
            "LZWDecodeCompat"
        } else {
            "LZWDecode"
        };
        let m = format!("{f}: {what} at scanline {}", self.row as i32);
        self.error(Some(&self.name.clone()), &m, s);
    }

    /// `LZWDecode`.
    fn lzw_decode(&mut self, buf: &mut [u8], occ0: i32, s: &mut dyn Streams) -> i32 {
        let mut sp = self.codec.lzw.take().expect("LZW");
        let r = self.lzw_decode_with(&mut sp, buf, occ0, false, s);
        self.codec.lzw = Some(sp);
        r
    }

    /// `LZWDecodeCompat`.
    fn lzw_decode_compat(&mut self, buf: &mut [u8], occ0: i32, s: &mut dyn Streams) -> i32 {
        let mut sp = self.codec.lzw.take().expect("LZW");
        let r = self.lzw_decode_with(&mut sp, buf, occ0, true, s);
        self.codec.lzw = Some(sp);
        r
    }

    fn null_entry_notice(&mut self, what: &str) {
        let name = self.name.clone();
        self.notices
            .add(&FS_818, format!("decoding the LZW data of {name}: {what}"));
    }

    #[allow(clippy::too_many_lines)]
    fn lzw_decode_with(
        &mut self,
        sp: &mut Lzw,
        buf: &mut [u8],
        occ0: i32,
        compat: bool,
        s: &mut dyn Streams,
    ) -> i32 {
        let mut tab = sp.codetab.take().unwrap_or_default();
        let mut op: i64 = 0;
        let mut occ: i64 = i64::from(occ0);
        let mut bad_ptr = false;
        // Writes one byte at position p of the output (before the buffer: never written).
        let put = |buf: &mut [u8], p: i64, v: u8, bad: &mut bool| {
            if p >= 0 && (p as usize) < buf.len() {
                buf[p as usize] = v;
            } else {
                *bad = true;
            }
        };
        let ent = |tab: &Vec<CodeEnt>, i: i32| -> CodeEnt {
            if i >= 0 && (i as usize) < tab.len() {
                tab[i as usize]
            } else {
                ZERO
            }
        };
        let mut never_written = false;
        let mut null_deref = false;
        // A loop in the code table's links (an entry linked to itself): libtiff follows it for
        // ever (a hang) or, old-style, writes before the row until it crashes.
        let mut endless = false;
        let result: i32 = 'body: {
            if sp.dec_restart != 0 {
                let mut codep = sp.dec_codep;
                let mut residue = i64::from(ent(&tab, codep).length) - sp.dec_restart;
                if residue > occ {
                    sp.dec_restart += occ;
                    loop {
                        if codep == NIL {
                            null_deref = true;
                            break 'body 0;
                        }
                        codep = ent(&tab, codep).next;
                        residue -= 1;
                        if !(residue > occ && (compat || codep != NIL)) {
                            break;
                        }
                    }
                    if codep != NIL || compat {
                        let mut tp = op + occ;
                        loop {
                            if codep == NIL {
                                null_deref = true;
                                break 'body 0;
                            }
                            tp -= 1;
                            let e = ent(&tab, codep);
                            put(buf, tp, e.value, &mut bad_ptr);
                            codep = e.next;
                            occ -= 1;
                            if !(occ != 0 && (compat || codep != NIL)) {
                                break;
                            }
                        }
                    }
                    break 'body 1;
                }
                op += residue;
                occ -= residue;
                let mut tp = op;
                loop {
                    if codep == NIL {
                        null_deref = true;
                        break 'body 0;
                    }
                    tp -= 1;
                    let e = ent(&tab, codep);
                    codep = e.next;
                    put(buf, tp, e.value, &mut bad_ptr);
                    residue -= 1;
                    if !(residue != 0 && (compat || codep != NIL)) {
                        break;
                    }
                }
                sp.dec_restart = 0;
            }
            let mut bp = self.rawcp;
            let mut nbits = i64::from(sp.nbits);
            let mut nextdata = sp.nextdata;
            let mut nextbits = sp.nextbits;
            let mut nbitsmask = sp.dec_nbitsmask;
            let mut oldcodep = sp.dec_oldcodep;
            let mut free_entp = sp.dec_free_entp;
            let mut maxcodep = sp.dec_maxcodep;
            let mut ret: Option<i32> = None;
            let mut bitsleft = sp.dec_bitsleft;
            let mut save = true;
            while occ > 0 {
                let mut code = self.lzw_next_code(
                    &mut bitsleft,
                    &mut bp,
                    (&mut nextdata, &mut nextbits),
                    (nbits, nbitsmask),
                    compat,
                    s,
                );
                if code == CODE_EOI {
                    break;
                }
                if code == CODE_CLEAR {
                    free_entp = CODE_FIRST;
                    nbits = BITS_MIN;
                    nbitsmask = (1 << BITS_MIN) - 1;
                    maxcodep = if compat {
                        nbitsmask as i32
                    } else {
                        (nbitsmask - 1) as i32
                    };
                    code = self.lzw_next_code(
                        &mut bitsleft,
                        &mut bp,
                        (&mut nextdata, &mut nextbits),
                        (nbits, nbitsmask),
                        compat,
                        s,
                    );
                    if code == CODE_EOI {
                        break;
                    }
                    put(buf, op, code as u8, &mut bad_ptr);
                    op += 1;
                    occ -= 1;
                    oldcodep = i32::from(code);
                    continue;
                }
                let mut codep = i32::from(code);
                if !(0..CSIZE).contains(&free_entp) {
                    self.lzw_error("Corrupted LZW table", compat, s);
                    ret = Some(0);
                    save = false;
                    break;
                }
                if !(0..CSIZE).contains(&oldcodep) {
                    tab[free_entp as usize].next = oldcodep;
                    self.lzw_error("Corrupted LZW table", compat, s);
                    ret = Some(0);
                    save = false;
                    break;
                }
                if oldcodep == 256 || oldcodep == 257 {
                    never_written = true;
                }
                let old = tab[oldcodep as usize];
                let firstchar_codep = if codep < free_entp {
                    if codep == 256 || codep == 257 {
                        never_written = true;
                    }
                    Some(tab[codep as usize].firstchar)
                } else {
                    None
                };
                {
                    let fe = &mut tab[free_entp as usize];
                    fe.next = oldcodep;
                    fe.firstchar = old.firstchar;
                    fe.length = old.length.wrapping_add(1);
                    fe.value = firstchar_codep.unwrap_or(fe.firstchar);
                }
                free_entp += 1;
                if free_entp > maxcodep {
                    nbits += 1;
                    if nbits > BITS_MAX {
                        nbits = BITS_MAX;
                    }
                    nbitsmask = (1 << nbits) - 1;
                    maxcodep = if compat {
                        nbitsmask as i32
                    } else {
                        (nbitsmask - 1) as i32
                    };
                }
                oldcodep = codep;
                if code >= 256 {
                    if code == 256 || code == 257 {
                        never_written = true;
                    }
                    if tab[codep as usize].length == 0 {
                        self.lzw_error(
                            "Wrong length of decoded string: data probably corrupted",
                            compat,
                            s,
                        );
                        ret = Some(0);
                        save = false;
                        break;
                    }
                    if i64::from(tab[codep as usize].length) > occ {
                        sp.dec_codep = codep;
                        if compat {
                            let mut steps = 0;
                            loop {
                                if codep == NIL {
                                    null_deref = true;
                                    break;
                                }
                                codep = tab[codep as usize].next;
                                if codep == NIL {
                                    null_deref = true;
                                    break;
                                }
                                if i64::from(tab[codep as usize].length) <= occ {
                                    break;
                                }
                                steps += 1;
                                if steps > CSIZE {
                                    endless = true;
                                    break;
                                }
                            }
                            if endless {
                                ret = Some(0);
                                break;
                            }
                            if null_deref {
                                ret = Some(0);
                                break;
                            }
                            sp.dec_restart = occ;
                            let mut tp = op + occ;
                            loop {
                                if codep == NIL {
                                    null_deref = true;
                                    break;
                                }
                                tp -= 1;
                                let e = tab[codep as usize];
                                put(buf, tp, e.value, &mut bad_ptr);
                                codep = e.next;
                                occ -= 1;
                                if occ == 0 {
                                    break;
                                }
                            }
                            if null_deref {
                                ret = Some(0);
                            }
                            break;
                        }
                        let mut steps = 0;
                        loop {
                            codep = tab[codep as usize].next;
                            if !(codep != NIL && i64::from(tab[codep as usize].length) > occ) {
                                break;
                            }
                            steps += 1;
                            if steps > CSIZE {
                                endless = true;
                                break;
                            }
                        }
                        if endless {
                            ret = Some(0);
                            break;
                        }
                        if codep != NIL {
                            sp.dec_restart = occ;
                            let mut tp = op + occ;
                            loop {
                                tp -= 1;
                                let e = tab[codep as usize];
                                put(buf, tp, e.value, &mut bad_ptr);
                                codep = e.next;
                                occ -= 1;
                                if !(occ != 0 && codep != NIL) {
                                    break;
                                }
                            }
                            if codep != NIL {
                                self.code_loop(s);
                            }
                        }
                        break;
                    }
                    let len = i64::from(tab[codep as usize].length);
                    if compat {
                        op += len;
                        occ -= len;
                        let mut tp = op;
                        let mut steps = 0;
                        loop {
                            tp -= 1;
                            let e = tab[codep as usize];
                            put(buf, tp, e.value, &mut bad_ptr);
                            codep = e.next;
                            if codep == NIL {
                                break;
                            }
                            steps += 1;
                            if steps > CSIZE {
                                endless = true;
                                break;
                            }
                        }
                        if endless {
                            ret = Some(0);
                            break;
                        }
                        continue;
                    }
                    let mut tp = op + len;
                    loop {
                        tp -= 1;
                        let e = tab[codep as usize];
                        codep = e.next;
                        put(buf, tp, e.value, &mut bad_ptr);
                        if !(codep != NIL && tp > op) {
                            break;
                        }
                    }
                    if codep != NIL {
                        self.code_loop(s);
                        break;
                    }
                    op += len;
                    occ -= len;
                } else {
                    put(buf, op, code as u8, &mut bad_ptr);
                    op += 1;
                    occ -= 1;
                }
            }
            // dec_bitsleft lives in the state (NextCode updates it in place); the rest is saved
            // unless the C function returned straight away.
            sp.dec_bitsleft = bitsleft;
            if save {
                self.rawcp = bp;
                sp.nbits = nbits as u16;
                sp.nextdata = nextdata;
                sp.nextbits = nextbits;
                sp.dec_nbitsmask = nbitsmask;
                sp.dec_oldcodep = oldcodep;
                sp.dec_free_entp = free_entp;
                sp.dec_maxcodep = maxcodep;
            }
            if let Some(r) = ret {
                break 'body r;
            }
            if occ > 0 {
                let f = if compat {
                    "LZWDecodeCompat"
                } else {
                    "LZWDecode"
                };
                let m = format!(
                    "{f}: Not enough data at scanline {} (short {} bytes)",
                    self.row as i32, occ as i32
                );
                self.error(Some(&self.name.clone()), &m, s);
                break 'body 0;
            }
            1
        };
        sp.codetab = Some(tab);
        if self.past_raw.replace(false) {
            self.past_raw_notice();
        }
        if never_written {
            self.null_entry_notice(
                "it used a table entry the decoder never writes (codes 256 and 257 after a \
                 Clear code: memory contents), read as 0",
            );
        }
        if null_deref {
            self.null_entry_notice(
                "libtiff's old-style LZW decoder follows a null table link here (a crash); \
                 decoding stopped",
            );
        }
        if endless {
            self.null_entry_notice(
                "the code table's links loop back (an entry linked to itself): libtiff 3.6.1 \
                 follows them for ever (a hang, or writes before the row until it crashes); \
                 decoding stopped",
            );
        }
        if bad_ptr {
            self.null_entry_notice(
                "libtiff's old-style LZW decoder writes before the start of the row here \
                 (memory outside the buffer); not written",
            );
        }
        result
    }

    /// `NextCode` with `GetNextCode` (or `GetNextCodeCompat`).
    fn lzw_next_code(
        &self,
        bitsleft: &mut i64,
        bp: &mut usize,
        (nextdata, nextbits): (&mut i64, &mut i64),
        (nbits, nbitsmask): (i64, i64),
        compat: bool,
        s: &mut dyn Streams,
    ) -> u16 {
        if *bitsleft < nbits {
            let m = format!(
                "LZWDecode: Strip {} not terminated with EOI code",
                self.curstrip as i32
            );
            self.warning(Some(&self.name.clone()), &m, s);
            return CODE_EOI;
        }
        let defined = self.raw_defined;
        let rd = |bp: &mut usize| -> i64 {
            let b = if *bp < defined {
                self.rawdata[*bp]
            } else {
                self.past_raw.set(true);
                0
            };
            *bp += 1;
            i64::from(b)
        };
        let code;
        if compat {
            *nextdata |= rd(bp) << *nextbits;
            *nextbits += 8;
            if *nextbits < nbits {
                *nextdata |= rd(bp) << *nextbits;
                *nextbits += 8;
            }
            code = (*nextdata & nbitsmask) as u16;
            *nextdata >>= nbits;
            *nextbits -= nbits;
        } else {
            *nextdata = nextdata.wrapping_shl(8) | rd(bp);
            *nextbits += 8;
            if *nextbits < nbits {
                *nextdata = nextdata.wrapping_shl(8) | rd(bp);
                *nextbits += 8;
            }
            code = ((*nextdata >> (*nextbits - nbits)) & nbitsmask) as u16;
            *nextbits -= nbits;
        }
        *bitsleft -= nbits;
        code
    }

    /// `ZIPDecode`.
    fn zip_decode(&mut self, op: &mut [u8], occ: i32, s: &mut dyn Streams) -> i32 {
        const MODULE: &str = "ZIPDecode";
        let past = self
            .codec
            .zip
            .as_ref()
            .is_some_and(|z| z.in_at + z.in_left as usize > self.raw_defined);
        if past {
            // The strip's byte count passes the bytes the raw buffer holds (a strip left from
            // an earlier directory): zlib reads memory past them.
            self.past_raw_notice();
        }
        let mut sp = self.codec.zip.take().expect("ZIP");
        let inf = sp.inflate.as_mut().expect("inflate");
        let n = (occ.max(0) as usize).min(op.len());
        let mut out_at = 0usize;
        let mut avail_out = occ.max(0) as u32;
        let mut r = 1;
        loop {
            let input = &self.rawdata[sp.in_at.min(self.raw_defined)
                ..(sp.in_at + sp.in_left as usize).min(self.raw_defined)];
            let (state, used, wrote) = inf.inflate(input, &mut op[out_at..n], z::Z_PARTIAL_FLUSH);
            sp.in_at += used;
            sp.in_left -= used as u32;
            out_at += wrote;
            avail_out -= wrote as u32;
            if state == z::Z_STREAM_END {
                break;
            }
            if state == z::Z_DATA_ERROR {
                let m = format!(
                    "{}: Decoding error at scanline {}, {}",
                    self.name,
                    self.row as i32,
                    inf.msg.unwrap_or("(null)")
                );
                self.error(Some(MODULE), &m, s);
                let input = &self.rawdata[sp.in_at.min(self.raw_defined)
                    ..(sp.in_at + sp.in_left as usize).min(self.raw_defined)];
                let (sr, skipped) = inf.sync(input);
                sp.in_at += skipped;
                sp.in_left -= skipped as u32;
                if sr != z::Z_OK {
                    r = 0;
                    break;
                }
                if avail_out > 0 {
                    continue;
                }
                break;
            }
            if state != z::Z_OK {
                let m = format!("{}: zlib error: {}", self.name, inf.msg.unwrap_or("(null)"));
                self.error(Some(MODULE), &m, s);
                r = 0;
                break;
            }
            if avail_out == 0 {
                break;
            }
        }
        if r == 1 && avail_out != 0 {
            let m = format!(
                "{}: Not enough data at scanline {} (short {} bytes)",
                self.name, self.row as i32, avail_out as i32
            );
            self.error(Some(MODULE), &m, s);
            r = 0;
        }
        self.codec.zip = Some(sp);
        r
    }

    /// `ZIPVSetField`'s `TIFFTAG_ZIPQUALITY` (`deflateParams` on the stream's current output
    /// space in the raw buffer once the encoder is set up).
    pub(crate) fn zip_set_quality(&mut self, q: u32, s: &mut dyn Streams) -> bool {
        let rd = self.mode == super::Mode::RdOnly;
        let mut sp = self.codec.zip.take().expect("ZIP");
        sp.zipquality = q as i32;
        let mut ok = true;
        if !rd
            && sp.init
            && let Some(d) = sp.deflate.as_mut()
        {
            {
                let (at, left) = (sp.out_at, sp.out_left);
                let end = (at + left as usize).min(self.rawdata.len());
                let (r, _, made) = d.params(
                    q as i32,
                    z::Z_DEFAULT_STRATEGY,
                    &[],
                    &mut self.rawdata[at..end],
                );
                sp.out_at += made;
                sp.out_left -= made as u32;
                if r != z::Z_OK {
                    let m = format!("{}: zlib error: {}", self.name, d.msg.unwrap_or("(null)"));
                    self.error(Some("ZIPVSetField"), &m, s);
                    ok = false;
                }
            }
        }
        self.codec.zip = Some(sp);
        ok
    }

    /// The predictor's routines on `buf[..cc]`, with libtiff's loops: they run by whole
    /// pixels (`stride` samples) while the count left is positive, so a row whose length is not
    /// a multiple of the pixel runs past it (FS-832 when past the caller's buffer; bytes past
    /// `cc` within it are changed as libtiff changes them).
    pub(crate) fn pfunc(&mut self, pf: Option<PFunc>, buf: &mut [u8], cc: i32) {
        let Some(pf) = pf else {
            return;
        };
        let stride = i64::from(self.codec.predictor.stride);
        let len = buf.len() as i64;
        let mut outside = false;
        let get8 = |buf: &[u8], i: i64, outside: &mut bool| -> u8 {
            if i >= 0 && i < len {
                buf[i as usize]
            } else {
                *outside = true;
                0
            }
        };
        let put8 = |buf: &mut [u8], i: i64, v: u8, outside: &mut bool| {
            if i >= 0 && i < len {
                buf[i as usize] = v;
            } else {
                *outside = true;
            }
        };
        let get16 = |buf: &[u8], i: i64, outside: &mut bool| -> u16 {
            if i >= 0 && 2 * i + 1 < len {
                u16::from_le_bytes([buf[2 * i as usize], buf[2 * i as usize + 1]])
            } else {
                *outside = true;
                0
            }
        };
        let put16 = |buf: &mut [u8], i: i64, v: u16, outside: &mut bool| {
            if i >= 0 && 2 * i + 1 < len {
                buf[2 * i as usize..2 * i as usize + 2].copy_from_slice(&v.to_le_bytes());
            } else {
                *outside = true;
            }
        };
        match pf {
            PFunc::HorAcc8 => {
                let mut cc = i64::from(cc);
                if cc > stride && stride > 0 {
                    cc -= stride;
                    let mut cp = 0i64;
                    loop {
                        for _ in 0..stride {
                            let v = get8(buf, cp + stride, &mut outside).wrapping_add(get8(
                                buf,
                                cp,
                                &mut outside,
                            ));
                            put8(buf, cp + stride, v, &mut outside);
                            cp += 1;
                        }
                        cc -= stride;
                        if cc as i32 <= 0 {
                            break;
                        }
                    }
                }
            }
            PFunc::HorAcc16 | PFunc::SwabHorAcc16 => {
                let mut wc = i64::from(cc / 2);
                if wc > stride && stride > 0 {
                    if pf == PFunc::SwabHorAcc16 {
                        let n = (wc as usize * 2).min(buf.len());
                        buf[..n].chunks_exact_mut(2).for_each(|w| w.swap(0, 1));
                    }
                    wc -= stride;
                    let mut wp = 0i64;
                    loop {
                        for _ in 0..stride {
                            let v = get16(buf, wp + stride, &mut outside).wrapping_add(get16(
                                buf,
                                wp,
                                &mut outside,
                            ));
                            put16(buf, wp + stride, v, &mut outside);
                            wp += 1;
                        }
                        wc -= stride;
                        if wc as i32 <= 0 {
                            break;
                        }
                    }
                }
            }
            PFunc::HorDiff8 => {
                let mut cc = i64::from(cc);
                if cc > stride && stride > 0 {
                    cc -= stride;
                    if stride == 3 || stride == 4 {
                        let mut prev: Vec<u8> =
                            (0..stride).map(|k| get8(buf, k, &mut outside)).collect();
                        let mut cp = 0i64;
                        loop {
                            for k in 0..stride {
                                let r1 = get8(buf, cp + stride + k, &mut outside);
                                put8(
                                    buf,
                                    cp + stride + k,
                                    r1.wrapping_sub(prev[k as usize]),
                                    &mut outside,
                                );
                                prev[k as usize] = r1;
                            }
                            cp += stride;
                            cc -= stride;
                            if cc as i32 <= 0 {
                                break;
                            }
                        }
                    } else {
                        let mut cp = cc - 1;
                        loop {
                            for _ in 0..stride {
                                let v = get8(buf, cp + stride, &mut outside).wrapping_sub(get8(
                                    buf,
                                    cp,
                                    &mut outside,
                                ));
                                put8(buf, cp + stride, v, &mut outside);
                                cp -= 1;
                            }
                            cc -= stride;
                            if cc as i32 <= 0 {
                                break;
                            }
                        }
                    }
                }
            }
            PFunc::HorDiff16 => {
                let mut wc = i64::from(cc / 2);
                if wc > stride && stride > 0 {
                    wc -= stride;
                    let mut wp = wc - 1;
                    loop {
                        for _ in 0..stride {
                            let v = get16(buf, wp + stride, &mut outside).wrapping_sub(get16(
                                buf,
                                wp,
                                &mut outside,
                            ));
                            put16(buf, wp + stride, v, &mut outside);
                            wp -= 1;
                        }
                        wc -= stride;
                        if wc as i32 <= 0 {
                            break;
                        }
                    }
                }
            }
        }
        if outside {
            let name = self.name.clone();
            self.notices.add(
                &FS_832,
                format!(
                    "{name}: the predictor's differencing of a row of {cc} bytes with {stride} \
                     samples per pixel runs past the row (libtiff 3.6.1 works by whole pixels): \
                     memory outside the buffer, not touched"
                ),
            );
        }
    }
}

impl Zip {
    pub(crate) fn zipquality_pub(&self) -> i32 {
        self.zipquality
    }
    pub(crate) fn set_deflate(&mut self, d: Deflate) {
        self.deflate = Some(d);
        self.init = true;
    }
    pub(crate) fn deflate_mut(&mut self) -> &mut Deflate {
        self.deflate.as_mut().expect("deflate set up")
    }
    /// The encoder's output: its offset in the raw buffer and the space left.
    pub(crate) fn out(&self) -> (usize, u32) {
        (self.out_at, self.out_left)
    }
    pub(crate) fn out_advance(&mut self, n: u32) {
        self.out_at += n as usize;
        self.out_left -= n;
    }
    /// `next_out = tif_rawdata; avail_out = tif_rawdatasize`.
    pub(crate) fn out_reset(&mut self, size: u32) {
        self.out_at = 0;
        self.out_left = size;
    }
}

impl Tiff {
    pub(crate) fn predictor_setup_pub(&mut self, s: &mut dyn Streams) -> bool {
        self.predictor_setup(s)
    }
}
