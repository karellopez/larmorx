// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! Writing: `tif_write.c` (`TIFFWriteScanline`, `TIFFWriteCheck`, `TIFFSetupStrips`,
//! `TIFFWriteBufferSetup`, `TIFFGrowStrips`, `TIFFAppendToStrip`, `TIFFFlushData1`),
//! `tif_flush.c` (`TIFFFlush`, `TIFFFlushData`), `tif_dirwrite.c` (`TIFFWriteDirectory` and the
//! writers of the fields this port keeps: sizes, formats, resolutions and positions, strips,
//! the per-sample values, the integer, rational, float and double fields), and the encoders
//! (None, PackBits, Deflate over [`larmorx_core::zlib::Deflate`], the predictor's
//! differencing). A directory whose fields this port keeps only as present (strings, colour
//! maps, transfer functions: none of which [`Tiff::set_field`] can set) writes no entry for
//! them.

use super::codec::{Encode, PFunc, PostEncode, PreEncode, SetupEncode, codec_name};
use super::dir::Value;
use super::fields::{self, bit, tag};
use super::io::Whence;
use super::{
    Mode, TIFF_BEENWRITING, TIFF_BIGENDIAN, TIFF_BUFFERSETUP, TIFF_CODERSETUP, TIFF_DIRTYDIRECT,
    TIFF_INSUBIFD, TIFF_MYBUFFER, TIFF_NOBITREV, TIFF_POSTENCODE, Tiff, Unported, values,
};
use larmorx_core::cfmt::{self, Streams};
use larmorx_core::defects::{FS_827, FS_828};
use larmorx_core::zlib::{self as z, Deflate};

/// A directory entry being written (`TIFFDirEntry`).
#[derive(Clone, Copy, Debug, Default)]
struct Entry {
    tag: u16,
    ty: u16,
    count: u32,
    offset: u32,
}

impl Tiff {
    /// `TIFFWriteCheck(tif, tiles, module)`.
    fn write_check(&mut self, tiles: bool, module: &str, s: &mut dyn Streams) -> bool {
        if self.mode == Mode::RdOnly {
            let m = format!("{}: File not open for writing", self.name);
            self.error(Some(module), &m, s);
            return false;
        }
        if tiles != self.is_tiled() {
            let m = if tiles {
                "Can not write tiles to a stripped image"
            } else {
                "Can not write scanlines to a tiled image"
            };
            self.error(Some(&self.name.clone()), m, s);
            return false;
        }
        if self.dir.stripoffset.is_some() && self.dir.compression != super::compression::NONE {
            let m = format!(
                "{}:\nIn place update to compressed TIFF images not supported.",
                self.name
            );
            self.error(Some(module), &m, s);
            return false;
        }
        if !self.dir.is_set(bit::IMAGEDIMENSIONS) {
            let m = format!("{}: Must set \"ImageWidth\" before writing data", self.name);
            self.error(Some(module), &m, s);
            return false;
        }
        if !self.dir.is_set(bit::PLANARCONFIG) {
            let m = format!(
                "{}: Must set \"PlanarConfiguration\" before writing data",
                self.name
            );
            self.error(Some(module), &m, s);
            return false;
        }
        if self.dir.stripoffset.is_none() && !self.setup_strips(s) {
            self.dir.nstrips = 0;
            let what = if self.is_tiled() { "tile" } else { "strip" };
            let m = format!("{}: No space for {what} arrays", self.name);
            self.error(Some(module), &m, s);
            return false;
        }
        self.tilesize = self.tile_size();
        self.scanlinesize = self.scanline_size();
        self.flags |= TIFF_BEENWRITING;
        true
    }

    /// `TIFFSetupStrips(tif)`.
    fn setup_strips(&mut self, s: &mut dyn Streams) -> bool {
        let unspecified = |t: &Tiff, b: u16| t.dir.is_set(b) && t.dir.imagelength == 0;
        let spi = if self.is_tiled() {
            if unspecified(self, bit::TILEDIMENSIONS) {
                u32::from(self.dir.samplesperpixel)
            } else {
                self.number_of_tiles()
            }
        } else if unspecified(self, bit::ROWSPERSTRIP) {
            u32::from(self.dir.samplesperpixel)
        } else {
            self.number_of_strips()
        };
        self.dir.stripsperimage = spi;
        self.dir.nstrips = spi;
        if self.dir.planarconfig == values::PLANARCONFIG_SEPARATE {
            self.dir.stripsperimage /= u32::from(self.dir.samplesperpixel);
        }
        let n = u64::from(self.dir.nstrips) * 4;
        let _ = s;
        if !self.can_alloc(n) {
            return false;
        }
        self.dir.stripoffset = Some(vec![0; self.dir.nstrips as usize]);
        self.dir.stripbytecount = Some(vec![0; self.dir.nstrips as usize]);
        self.dir.set_bit(bit::STRIPOFFSETS);
        self.dir.set_bit(bit::STRIPBYTECOUNTS);
        true
    }

    /// `TIFFWriteBufferSetup(tif, NULL, -1)`.
    fn write_buffer_setup(&mut self, s: &mut dyn Streams) -> bool {
        self.rawdata = Vec::new();
        self.flags &= !TIFF_MYBUFFER;
        let mut size = if self.is_tiled() {
            self.tilesize
        } else {
            self.scanlinesize
        };
        if size < 8 * 1024 {
            size = 8 * 1024;
        }
        if !self.can_alloc(size as u64) {
            let m = format!("{}: No space for output buffer", self.name);
            self.error(Some("TIFFWriteBufferSetup"), &m, s);
            return false;
        }
        self.flags |= TIFF_MYBUFFER;
        self.rawdata = vec![0; size as usize];
        self.rawcc = 0;
        self.rawcp = 0;
        self.flags |= TIFF_BUFFERSETUP;
        true
    }

    /// `TIFFWriteScanline(tif, buf, row, sample)`: the encoder's status (1, 0 or -1). The
    /// caller's buffer holds `TIFFScanlineSize` bytes; libtiff byte-swaps it in place when the
    /// file's byte order is not the machine's.
    pub fn write_scanline(
        &mut self,
        buf: &mut [u8],
        row: u32,
        sample: u16,
        s: &mut dyn Streams,
    ) -> Result<i32, Unported> {
        let r = self.write_scanline_inner(buf, row, sample, s);
        match self.unported {
            Some(u) => Err(u),
            None => Ok(r),
        }
    }

    fn write_scanline_inner(&mut self, buf: &mut [u8], row: u32, sample: u16, s: &mut dyn Streams) -> i32 {
        const MODULE: &str = "TIFFWriteScanline";
        if self.flags & TIFF_BEENWRITING == 0 && !self.write_check(false, MODULE, s) {
            return -1;
        }
        if !(self.flags & TIFF_BUFFERSETUP != 0 && !self.rawdata.is_empty())
            && !self.write_buffer_setup(s)
        {
            return -1;
        }
        let mut imagegrew = false;
        if row >= self.dir.imagelength {
            if self.dir.planarconfig == values::PLANARCONFIG_SEPARATE {
                self.error(
                    Some(&self.name.clone()),
                    "Can not change \"ImageLength\" when using separate planes",
                    s,
                );
                return -1;
            }
            self.dir.imagelength = row.wrapping_add(1);
            imagegrew = true;
        }
        let strip;
        if self.dir.planarconfig == values::PLANARCONFIG_SEPARATE {
            if sample >= self.dir.samplesperpixel {
                let m = format!(
                    "{sample}: Sample out of range, max {}",
                    self.dir.samplesperpixel
                );
                self.error(Some(&self.name.clone()), &m, s);
                return -1;
            }
            strip = u32::from(sample)
                .wrapping_mul(self.dir.stripsperimage)
                .wrapping_add(row / self.dir.rowsperstrip);
        } else {
            strip = row / self.dir.rowsperstrip;
        }
        if strip != self.curstrip {
            if !self.flush_data(s) {
                return -1;
            }
            self.curstrip = strip;
            if strip >= self.dir.stripsperimage && imagegrew {
                let rps = self.dir.rowsperstrip;
                self.dir.stripsperimage = self
                    .dir
                    .imagelength
                    .wrapping_add(rps.wrapping_sub(1))
                    / rps;
            }
            let spi = self.dir.stripsperimage;
            self.row = if spi == 0 {
                0
            } else {
                (strip % spi).wrapping_mul(self.dir.rowsperstrip)
            };
            if self.flags & TIFF_CODERSETUP == 0 {
                if !self.setup_encode(s) {
                    return -1;
                }
                self.flags |= TIFF_CODERSETUP;
            }
            if !self.pre_encode(s) {
                return -1;
            }
            self.flags |= TIFF_POSTENCODE;
        }
        if strip >= self.dir.nstrips && !self.grow_strips(1, MODULE, s) {
            return -1;
        }
        if row != self.row {
            if row < self.row {
                let spi = self.dir.stripsperimage;
                self.row = if spi == 0 {
                    0
                } else {
                    (strip % spi).wrapping_mul(self.dir.rowsperstrip)
                };
                self.rawcp = 0;
            }
            if !self.codec_seek(row.wrapping_sub(self.row), s) {
                return -1;
            }
            self.row = row;
        }
        let cc = self.scanlinesize;
        self.post_decode(buf, cc);
        let m = self.codec.encoderow;
        let status = self.encode(m, buf, cc, sample, s);
        self.row = row.wrapping_add(1);
        status
    }

    /// `TIFFGrowStrips(tif, delta, module)`.
    fn grow_strips(&mut self, delta: u32, module: &str, s: &mut dyn Streams) -> bool {
        let n = self.dir.nstrips.wrapping_add(delta);
        if !self.can_alloc(u64::from(n) * 4) {
            self.dir.nstrips = 0;
            let m = format!("{}: No space to expand strip arrays", self.name);
            self.error(Some(module), &m, s);
            return false;
        }
        let d = &mut self.dir;
        d.stripoffset.get_or_insert_with(Vec::new).resize(n as usize, 0);
        d.stripbytecount
            .get_or_insert_with(Vec::new)
            .resize(n as usize, 0);
        d.nstrips = n;
        true
    }

    /// `TIFFAppendToStrip(tif, strip, data, cc)` for the raw buffer's `[from, from + cc)`.
    fn append_to_strip(&mut self, strip: u32, from: usize, cc: i32, s: &mut dyn Streams) -> bool {
        const MODULE: &str = "TIFFAppendToStrip";
        let off = self.dir.stripoffset.as_ref().and_then(|v| v.get(strip as usize)).copied();
        let Some(off) = off else {
            return false;
        };
        if off == 0 || self.curoff == 0 {
            if off != 0 {
                if !self.seek_ok(off) {
                    let m = format!("{}: Seek error at scanline {}", self.name, self.row);
                    self.error(Some(MODULE), &m, s);
                    return false;
                }
            } else {
                let e = self.fd.seek(0, Whence::End);
                if let Some(v) = self.dir.stripoffset.as_mut() {
                    v[strip as usize] = e;
                }
            }
            self.curoff = self.dir.stripoffset.as_ref().map(|v| v[strip as usize]).unwrap_or(0);
        }
        let raw = std::mem::take(&mut self.rawdata);
        let end = (from + cc.max(0) as usize).min(raw.len());
        let w = self.fd.write(&raw[from..end], cc);
        self.rawdata = raw;
        if w != cc {
            let m = format!("{}: Write error at scanline {}", self.name, self.row);
            self.error(Some(MODULE), &m, s);
            return false;
        }
        self.curoff = self.curoff.wrapping_add(cc as u32);
        if let Some(v) = self.dir.stripbytecount.as_mut() {
            v[strip as usize] = v[strip as usize].wrapping_add(cc as u32);
        }
        true
    }

    /// `TIFFFlushData1(tif)`.
    pub(crate) fn flush_data1(&mut self, s: &mut dyn Streams) -> bool {
        if self.rawcc > 0 {
            let fillorder = u32::from(self.dir.fillorder);
            if self.flags & fillorder == 0 && self.flags & TIFF_NOBITREV == 0 {
                let n = (self.rawcc as usize).min(self.rawdata.len());
                for b in &mut self.rawdata[..n] {
                    *b = b.reverse_bits();
                }
            }
            let strip = if self.is_tiled() {
                self.curtile
            } else {
                self.curstrip
            };
            let cc = self.rawcc;
            if !self.append_to_strip(strip, 0, cc, s) {
                return false;
            }
            self.rawcc = 0;
            self.rawcp = 0;
        }
        true
    }

    /// `TIFFFlushData(tif)`.
    fn flush_data(&mut self, s: &mut dyn Streams) -> bool {
        if self.flags & TIFF_BEENWRITING == 0 {
            return false;
        }
        if self.flags & TIFF_POSTENCODE != 0 {
            self.flags &= !TIFF_POSTENCODE;
            if !self.post_encode(s) {
                return false;
            }
        }
        self.flush_data1(s)
    }

    /// `TIFFFlush(tif)`.
    pub fn flush(&mut self, s: &mut dyn Streams) -> bool {
        if self.mode != Mode::RdOnly {
            if !self.flush_data(s) {
                return false;
            }
            if self.flags & TIFF_DIRTYDIRECT != 0 && !self.write_directory_inner(s) {
                return false;
            }
        }
        true
    }

    // ---- encoders ----

    /// `(*tif->tif_setupencode)(tif)` (with the predictor's wrapper).
    fn setup_encode(&mut self, s: &mut dyn Streams) -> bool {
        let ok = match self.codec.setupencode {
            SetupEncode::True => true,
            SetupEncode::NotConfigured => {
                let n = codec_name(self.dir.compression).unwrap_or("");
                let m = format!("{n} compression support is not configured");
                self.error(Some(&self.name.clone()), &m, s);
                false
            }
            SetupEncode::LzwRefused => {
                self.error(
                    Some(&self.name.clone()),
                    "LZW compression is not available to due to Unisys patent enforcement",
                    s,
                );
                false
            }
            SetupEncode::Zip => {
                let q = self.codec.zip.as_ref().map(|z| z.zipquality_pub()).unwrap_or(-1);
                match Deflate::init(q) {
                    Ok(d) => {
                        let zs = self.codec.zip.as_mut().expect("ZIP");
                        zs.set_deflate(d);
                        true
                    }
                    Err(_) => {
                        let m = format!("{}: (null)", self.name);
                        self.error(Some("ZIPSetupEncode"), &m, s);
                        false
                    }
                }
            }
        };
        if !ok {
            return false;
        }
        if self.codec.pred_setup {
            return self.predictor_setup_encode(s);
        }
        true
    }

    fn predictor_setup_encode(&mut self, s: &mut dyn Streams) -> bool {
        if !self.predictor_setup_pub(s) {
            return false;
        }
        if self.codec.predictor.predictor == 2 {
            let k = &mut self.codec;
            k.predictor.pfunc = match self.dir.bitspersample {
                8 => Some(PFunc::HorDiff8),
                16 => Some(PFunc::HorDiff16),
                _ => k.predictor.pfunc,
            };
            k.predictor.ecoderow = k.encoderow;
            k.encoderow = Encode::PredRow;
            k.predictor.ecodestrip = k.encodestrip;
            k.encodestrip = Encode::PredTile;
            k.predictor.ecodetile = k.encodetile;
            k.encodetile = Encode::PredTile;
        }
        true
    }

    /// `(*tif->tif_preencode)(tif, sample)`.
    fn pre_encode(&mut self, _s: &mut dyn Streams) -> bool {
        match self.codec.preencode {
            PreEncode::None => true,
            PreEncode::PackBits => {
                self.codec.packbits_rowsize = if self.is_tiled() {
                    self.tile_row_size()
                } else {
                    self.scanline_size()
                };
                true
            }
            PreEncode::Zip => {
                let n = self.rawdata.len() as u32;
                let zs = self.codec.zip.as_mut().expect("ZIP");
                zs.out_reset(n);
                zs.deflate_mut().reset() == z::Z_OK
            }
        }
    }

    /// `(*tif->tif_postencode)(tif)`.
    fn post_encode(&mut self, s: &mut dyn Streams) -> bool {
        match self.codec.postencode {
            PostEncode::True => true,
            PostEncode::Zip => self.zip_post_encode(s),
        }
    }

    /// One of the encoding methods on `buf[..cc]`.
    fn encode(&mut self, m: Encode, buf: &mut [u8], cc: i32, sample: u16, s: &mut dyn Streams) -> i32 {
        match m {
            Encode::NoRow => self.no_encode("scanline", s),
            Encode::NoStrip => self.no_encode("strip", s),
            Encode::NoTile => self.no_encode("tile", s),
            Encode::Dump => self.dump_encode(buf, cc, s),
            Encode::PackBits => self.packbits_encode(buf, cc, s),
            Encode::PackBitsChunk => {
                let rowsize = self.codec.packbits_rowsize;
                let mut at = 0usize;
                let mut cc = i64::from(cc);
                while cc > 0 {
                    let chunk = (rowsize as i64).min(cc);
                    let end = (at + chunk as usize).min(buf.len());
                    if self.packbits_encode(&buf[at..end], chunk as i32, s) < 0 {
                        return -1;
                    }
                    at += chunk.max(0) as usize;
                    cc -= chunk;
                    if chunk <= 0 {
                        break;
                    }
                }
                1
            }
            Encode::Zip => self.zip_encode(buf, cc, s),
            Encode::PredRow => {
                let pf = self.codec.predictor.pfunc;
                self.pfunc(pf, buf, cc);
                let inner = self.codec.predictor.ecoderow;
                self.encode(inner, buf, cc, sample, s)
            }
            Encode::PredTile => {
                let pf = self.codec.predictor.pfunc;
                let rowsize = self.codec.predictor.rowsize;
                let mut at = 0usize;
                let mut left = i64::from(cc);
                while left > 0 && rowsize > 0 && at < buf.len() {
                    self.pfunc(pf, &mut buf[at..], rowsize);
                    left -= i64::from(rowsize);
                    at += rowsize as usize;
                }
                let inner = self.codec.predictor.ecodetile;
                self.encode(inner, buf, cc, sample, s)
            }
        }
    }

    /// `TIFFNoEncode(tif, method)`.
    fn no_encode(&self, method: &str, s: &mut dyn Streams) -> i32 {
        let m = match codec_name(self.dir.compression) {
            Some(n) if n.starts_with("LZW") => {
                format!("{n} {method} encoding is no longer implemented due to Unisys patent enforcement")
            }
            Some(n) => format!("{n} {method} encoding is not implemented"),
            None => format!(
                "Compression scheme {} {method} encoding is not implemented",
                self.dir.compression
            ),
        };
        self.error(Some(&self.name.clone()), &m, s);
        -1
    }

    /// `DumpModeEncode`.
    fn dump_encode(&mut self, buf: &[u8], cc: i32, s: &mut dyn Streams) -> i32 {
        let mut cc = cc;
        let mut at = 0usize;
        let size = self.rawdata.len() as i32;
        while cc > 0 {
            let mut n = cc;
            if self.rawcc + n > size {
                n = size - self.rawcc;
            }
            let p = self.rawcp;
            for k in 0..n as usize {
                self.rawdata[p + k] = buf.get(at + k).copied().unwrap_or(0);
            }
            self.rawcp += n as usize;
            self.rawcc += n;
            at += n as usize;
            cc -= n;
            if self.rawcc >= size && !self.flush_data1(s) {
                return -1;
            }
        }
        1
    }

    /// `PackBitsEncode`.
    fn packbits_encode(&mut self, buf: &[u8], cc: i32, s: &mut dyn Streams) -> i32 {
        #[derive(PartialEq, Clone, Copy)]
        enum St {
            Base,
            Literal,
            Run,
            LiteralRun,
        }
        let mut op = self.rawcp;
        let ep = self.rawdata.len();
        let mut state = St::Base;
        let mut lastliteral = 0usize;
        let mut cc = i64::from(cc);
        let mut bp = 0usize;
        let get = |i: usize| buf.get(i).copied().unwrap_or(0);
        while cc > 0 {
            let b = get(bp);
            bp += 1;
            cc -= 1;
            let mut n: i64 = 1;
            while cc > 0 && b == get(bp) {
                cc -= 1;
                bp += 1;
                n += 1;
            }
            loop {
                // again:
                if op + 2 >= ep {
                    if state == St::Literal || state == St::LiteralRun {
                        let slop = op - lastliteral;
                        self.rawcc += (lastliteral - self.rawcp) as i32;
                        if !self.flush_data1(s) {
                            return -1;
                        }
                        op = self.rawcp;
                        for k in 0..slop {
                            self.rawdata[op + k] = self.rawdata[lastliteral + k];
                        }
                        op += slop;
                        lastliteral = self.rawcp;
                    } else {
                        self.rawcc += (op - self.rawcp) as i32;
                        if !self.flush_data1(s) {
                            return -1;
                        }
                        op = self.rawcp;
                    }
                }
                match state {
                    St::Base | St::Run => {
                        if n > 1 {
                            state = St::Run;
                            if n > 128 {
                                self.rawdata[op] = (-127i8) as u8;
                                self.rawdata[op + 1] = b;
                                op += 2;
                                n -= 128;
                                continue;
                            }
                            self.rawdata[op] = (-(n - 1)) as i8 as u8;
                            self.rawdata[op + 1] = b;
                            op += 2;
                        } else {
                            lastliteral = op;
                            self.rawdata[op] = 0;
                            self.rawdata[op + 1] = b;
                            op += 2;
                            state = St::Literal;
                        }
                    }
                    St::Literal => {
                        if n > 1 {
                            state = St::LiteralRun;
                            if n > 128 {
                                self.rawdata[op] = (-127i8) as u8;
                                self.rawdata[op + 1] = b;
                                op += 2;
                                n -= 128;
                                continue;
                            }
                            self.rawdata[op] = (-(n - 1)) as i8 as u8;
                            self.rawdata[op + 1] = b;
                            op += 2;
                        } else {
                            self.rawdata[lastliteral] = self.rawdata[lastliteral].wrapping_add(1);
                            if self.rawdata[lastliteral] == 127 {
                                state = St::Base;
                            }
                            self.rawdata[op] = b;
                            op += 1;
                        }
                    }
                    St::LiteralRun => {
                        if n == 1
                            && self.rawdata[op - 2] == (-1i8) as u8
                            && (self.rawdata[lastliteral] as i8) < 126
                        {
                            self.rawdata[lastliteral] = self.rawdata[lastliteral].wrapping_add(2);
                            state = if self.rawdata[lastliteral] == 127 {
                                St::Base
                            } else {
                                St::Literal
                            };
                            self.rawdata[op - 2] = self.rawdata[op - 1];
                        } else {
                            state = St::Run;
                        }
                        continue;
                    }
                }
                break;
            }
        }
        self.rawcc += (op - self.rawcp) as i32;
        self.rawcp = op;
        1
    }

    /// `ZIPEncode`.
    fn zip_encode(&mut self, buf: &[u8], cc: i32, s: &mut dyn Streams) -> i32 {
        let mut zs = self.codec.zip.take().expect("ZIP");
        let mut input = &buf[..(cc.max(0) as usize).min(buf.len())];
        let size = self.rawdata.len();
        let mut r = 1;
        loop {
            let (at, left) = zs.out();
            let (st, used, made) = zs
                .deflate_mut()
                .deflate(input, &mut self.rawdata[at..at + left as usize], z::Z_NO_FLUSH);
            input = &input[used..];
            zs.out_advance(made as u32);
            if st != z::Z_OK {
                let msg = zs.deflate_mut().msg.unwrap_or("(null)");
                let m = format!("{}: Encoder error: {msg}", self.name);
                self.error(Some("ZIPEncode"), &m, s);
                r = 0;
                break;
            }
            if zs.out().1 == 0 {
                self.rawcc = size as i32;
                self.codec.zip = Some(zs);
                self.flush_data1(s);
                zs = self.codec.zip.take().expect("ZIP");
                zs.out_reset(size as u32);
            }
            if input.is_empty() {
                break;
            }
        }
        self.codec.zip = Some(zs);
        r
    }

    /// `ZIPPostEncode`.
    fn zip_post_encode(&mut self, s: &mut dyn Streams) -> bool {
        let size = self.rawdata.len();
        loop {
            let mut zs = self.codec.zip.take().expect("ZIP");
            let (at, left) = zs.out();
            let (st, _, made) =
                zs.deflate_mut()
                    .deflate(&[], &mut self.rawdata[at..at + left as usize], z::Z_FINISH);
            zs.out_advance(made as u32);
            match st {
                z::Z_STREAM_END | z::Z_OK => {
                    let left = zs.out().1;
                    if left as i32 != size as i32 {
                        self.rawcc = size as i32 - left as i32;
                        self.codec.zip = Some(zs);
                        self.flush_data1(s);
                        zs = self.codec.zip.take().expect("ZIP");
                        zs.out_reset(size as u32);
                    }
                    self.codec.zip = Some(zs);
                    if st == z::Z_STREAM_END {
                        return true;
                    }
                }
                _ => {
                    let msg = zs.deflate_mut().msg.unwrap_or("(null)");
                    let m = format!("{}: zlib error: {msg}", self.name);
                    self.codec.zip = Some(zs);
                    self.error(Some("ZIPPostEncode"), &m, s);
                    return false;
                }
            }
        }
    }

    // ---- the directory ----

    /// `TIFFWriteDirectory(tif)`.
    pub fn write_directory(&mut self, s: &mut dyn Streams) -> Result<bool, Unported> {
        let r = self.write_directory_inner(s);
        match self.unported {
            Some(u) => Err(u),
            None => Ok(r),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn write_directory_inner(&mut self, s: &mut dyn Streams) -> bool {
        if self.mode == Mode::RdOnly {
            return true;
        }
        if self.flags & TIFF_POSTENCODE != 0 {
            self.flags &= !TIFF_POSTENCODE;
            if !self.post_encode(s) {
                self.error(
                    Some(&self.name.clone()),
                    "Error post-encoding before directory write",
                    s,
                );
                return false;
            }
        }
        if self.rawcc > 0 && !self.flush_data1(s) {
            self.error(
                Some(&self.name.clone()),
                "Error flushing data before directory write",
                s,
            );
            return false;
        }
        if self.flags & TIFF_MYBUFFER != 0 {
            self.rawdata = Vec::new();
            self.rawcc = 0;
        }
        self.flags &= !(TIFF_BEENWRITING | TIFF_BUFFERSETUP);
        let mut nfields: u32 = 0;
        for b in 0..=127u16 {
            if self.dir.is_set(b) && b != bit::CUSTOM {
                nfields += if b < bit::SUBFILETYPE { 2 } else { 1 };
            }
        }
        nfields += self.dir.custom.len() as u32;
        let mut dirsize = nfields * 12;
        if self.diroff == 0 && !self.link_directory(s) {
            return false;
        }
        self.dataoff = self.diroff.wrapping_add(2 + dirsize + 4);
        if self.dataoff & 1 != 0 {
            self.dataoff += 1;
        }
        self.fd.seek(self.dataoff, Whence::Set);
        self.curdir = self.curdir.wrapping_add(1);
        let mut set = self.dir.fieldsset;
        let is = |set: &[u64; 4], b: u16| set[usize::from(b) / 32] & (1u64 << (b & 31)) != 0;
        let clr = |set: &mut [u64; 4], b: u16| set[usize::from(b) / 32] &= !(1u64 << (b & 31));
        if is(&set, bit::EXTRASAMPLES) && self.dir.extrasamples == 0 {
            clr(&mut set, bit::EXTRASAMPLES);
            nfields -= 1;
            dirsize -= 12;
        }
        let mut entries: Vec<Entry> = Vec::new();
        let sorted = self.fields.sorted.clone();
        for fr in sorted {
            let fip = self.fields.info(fr).clone();
            if fip.bit == bit::CUSTOM {
                // Custom values (unknown tags, Software): this port keeps no value for them.
                continue;
            }
            if !is(&set, fip.bit) {
                continue;
            }
            let ok = match fip.bit {
                bit::STRIPOFFSETS | bit::STRIPBYTECOUNTS => {
                    let counts = fip.bit == bit::STRIPBYTECOUNTS;
                    let t = match (self.is_tiled(), counts) {
                        (true, false) => tag::TILEOFFSETS,
                        (false, false) => tag::STRIPOFFSETS,
                        (true, true) => tag::TILEBYTECOUNTS,
                        (false, true) => tag::STRIPBYTECOUNTS,
                    };
                    if t != fip.tag {
                        continue;
                    }
                    let v = if counts {
                        self.dir.stripbytecount.clone()
                    } else {
                        self.dir.stripoffset.clone()
                    }
                    .unwrap_or_default();
                    let n = self.dir.nstrips;
                    self.write_long_array(fields::TIFF_LONG, t, n, &v, &mut entries, s)
                }
                bit::ROWSPERSTRIP => {
                    entries.push(self.setup_short_long(tag::ROWSPERSTRIP, self.dir.rowsperstrip));
                    true
                }
                bit::IMAGEDIMENSIONS => {
                    entries.push(self.setup_short_long(tag::IMAGEWIDTH, self.dir.imagewidth));
                    entries.push(self.setup_short_long(tag::IMAGELENGTH, self.dir.imagelength));
                    true
                }
                bit::TILEDIMENSIONS => {
                    entries.push(self.setup_short_long(tag::TILEWIDTH, self.dir.tilewidth));
                    entries.push(self.setup_short_long(tag::TILELENGTH, self.dir.tilelength));
                    true
                }
                bit::COMPRESSION => {
                    entries.push(self.setup_short(tag::COMPRESSION, self.dir.compression));
                    true
                }
                bit::PHOTOMETRIC => {
                    entries.push(self.setup_short(tag::PHOTOMETRIC, self.dir.photometric));
                    true
                }
                bit::POSITION => {
                    let (x, y) = (self.dir.xposition, self.dir.yposition);
                    self.write_rational_array(fields::TIFF_RATIONAL, tag::XPOSITION, &[x], &mut entries, s)
                        && self.write_rational_array(fields::TIFF_RATIONAL, tag::YPOSITION, &[y], &mut entries, s)
                }
                bit::RESOLUTION => {
                    let (x, y) = (self.dir.xresolution, self.dir.yresolution);
                    self.write_rational_array(fields::TIFF_RATIONAL, tag::XRESOLUTION, &[x], &mut entries, s)
                        && self.write_rational_array(fields::TIFF_RATIONAL, tag::YRESOLUTION, &[y], &mut entries, s)
                }
                bit::BITSPERSAMPLE | bit::MINSAMPLEVALUE | bit::MAXSAMPLEVALUE | bit::SAMPLEFORMAT => {
                    let v = match self.get_field(fip.tag, s) {
                        Some(Value::U16(v)) => v,
                        _ => 0,
                    };
                    let n = u32::from(self.dir.samplesperpixel);
                    let w = vec![v; n as usize];
                    self.write_short_array(fields::TIFF_SHORT, fip.tag, n, &w, &mut entries, s)
                }
                bit::SMINSAMPLEVALUE | bit::SMAXSAMPLEVALUE => {
                    let v = if fip.bit == bit::SMINSAMPLEVALUE {
                        self.dir.sminsamplevalue
                    } else {
                        self.dir.smaxsamplevalue
                    };
                    let n = u32::from(self.dir.samplesperpixel);
                    let ty = self.sample_to_tag_type();
                    self.write_any_array(ty, fip.tag, &vec![v; n as usize], &mut entries, s)
                }
                bit::PAGENUMBER | bit::HALFTONEHINTS | bit::YCBCRSUBSAMPLING | bit::DOTRANGE => {
                    let (a, b) = match self.get_field(fip.tag, s) {
                        Some(Value::U16Pair(a, b)) => (a, b),
                        _ => (0, 0),
                    };
                    self.write_short_array(fields::TIFF_SHORT, fip.tag, 2, &[a, b], &mut entries, s)
                }
                bit::COLORMAP | bit::INKNAMES | bit::TRANSFERFUNCTION | bit::SUBIFD => {
                    // Kept as present only: never set by a writer of this port.
                    continue;
                }
                _ => self.write_normal_tag(&fip, &mut entries, s),
            };
            if !ok {
                return false;
            }
            clr(&mut set, fip.bit);
        }
        if (entries.len() as u32) < nfields {
            let missing = nfields - entries.len() as u32;
            self.notices.add(
                &FS_828,
                format!(
                    "{}: the directory counts {nfields} entries and fills {}: libtiff 3.6.1 \
                     writes the {missing} left as memory never written; written as zeros",
                    self.name,
                    entries.len()
                ),
            );
        }
        entries.resize(nfields.max(entries.len() as u32) as usize, Entry::default());
        let swab = self.swab();
        let mut data = Vec::with_capacity(dirsize as usize);
        for e in &entries {
            let (t, ty, c, o) = if swab {
                (
                    e.tag.swap_bytes(),
                    e.ty.swap_bytes(),
                    e.count.swap_bytes(),
                    e.offset.swap_bytes(),
                )
            } else {
                (e.tag, e.ty, e.count, e.offset)
            };
            data.extend_from_slice(&t.to_le_bytes());
            data.extend_from_slice(&ty.to_le_bytes());
            data.extend_from_slice(&c.to_le_bytes());
            data.extend_from_slice(&o.to_le_bytes());
        }
        data.truncate(dirsize as usize);
        let mut dircount = nfields as u16;
        let mut diroff = self.nextdiroff;
        if swab {
            dircount = dircount.swap_bytes();
            diroff = diroff.swap_bytes();
        }
        self.fd.seek(self.diroff, Whence::Set);
        let name = self.name.clone();
        if self.fd.write(&dircount.to_le_bytes(), 2) != 2 {
            self.error(Some(&name), "Error writing directory count", s);
            return false;
        }
        if self.fd.write(&data, dirsize as i32) != dirsize as i32 {
            self.error(Some(&name), "Error writing directory contents", s);
            return false;
        }
        if self.fd.write(&diroff.to_le_bytes(), 4) != 4 {
            self.error(Some(&name), "Error writing directory link", s);
            return false;
        }
        // done: the directory freed, the codec cleaned up, a new directory set up.
        self.free_directory();
        self.flags &= !TIFF_DIRTYDIRECT;
        self.cleanup();
        let _ = self.default_directory(s);
        self.diroff = 0;
        self.nextdiroff = 0;
        self.curoff = 0;
        self.row = u32::MAX;
        self.curstrip = u32::MAX;
        true
    }

    /// `_TIFFSampleToTagType(tif)`.
    fn sample_to_tag_type(&self) -> u16 {
        let bps = (u32::from(self.dir.bitspersample) + 7) / 8;
        match self.dir.sampleformat {
            values::SAMPLEFORMAT_IEEEFP => {
                if bps == 4 {
                    fields::TIFF_FLOAT
                } else {
                    fields::TIFF_DOUBLE
                }
            }
            values::SAMPLEFORMAT_INT => {
                if bps <= 1 {
                    fields::TIFF_SBYTE
                } else if bps <= 2 {
                    fields::TIFF_SSHORT
                } else {
                    fields::TIFF_SLONG
                }
            }
            values::SAMPLEFORMAT_UINT => {
                if bps <= 1 {
                    fields::TIFF_BYTE
                } else if bps <= 2 {
                    fields::TIFF_SHORT
                } else {
                    fields::TIFF_LONG
                }
            }
            _ => fields::TIFF_UNDEFINED,
        }
    }

    /// `TIFFInsertData(tif, type, v)`.
    fn insert_data(&self, ty: u16, v: u32) -> u32 {
        let mask = match ty {
            1 | 6 | 7 => 0xff,
            3 | 8 => 0xffff,
            2 | 4 | 5 | 9 | 10 | 11 | 12 => 0xffff_ffff,
            _ => 0,
        };
        let shift = match ty {
            1 | 6 | 7 => 24,
            3 | 8 => 16,
            _ => 0,
        };
        if self.header.magic == TIFF_BIGENDIAN {
            (v & mask) << shift
        } else {
            v & mask
        }
    }

    /// `TIFFSetupShortLong(tif, tag, dir, v)`.
    fn setup_short_long(&self, t: u32, v: u32) -> Entry {
        if v > 0xffff {
            Entry {
                tag: t as u16,
                ty: fields::TIFF_LONG,
                count: 1,
                offset: v,
            }
        } else {
            Entry {
                tag: t as u16,
                ty: fields::TIFF_SHORT,
                count: 1,
                offset: self.insert_data(fields::TIFF_SHORT, v),
            }
        }
    }

    /// `TIFFSetupShort(tif, tag, dir, v)`.
    fn setup_short(&self, t: u32, v: u16) -> Entry {
        Entry {
            tag: t as u16,
            ty: fields::TIFF_SHORT,
            count: 1,
            offset: self.insert_data(fields::TIFF_SHORT, u32::from(v)),
        }
    }

    /// `TIFFWriteData(tif, dir, cp)` with the values' bytes in the machine's order.
    fn write_data(&mut self, e: &mut Entry, bytes: &[u8], s: &mut dyn Streams) -> bool {
        let mut b = bytes.to_vec();
        if self.swab() {
            match e.ty {
                fields::TIFF_SHORT | fields::TIFF_SSHORT => {
                    b.chunks_exact_mut(2).for_each(|c| c.swap(0, 1));
                }
                fields::TIFF_LONG
                | fields::TIFF_SLONG
                | fields::TIFF_FLOAT
                | fields::TIFF_RATIONAL
                | fields::TIFF_SRATIONAL => b.chunks_exact_mut(4).for_each(|c| c.reverse()),
                fields::TIFF_DOUBLE => b.chunks_exact_mut(8).for_each(|c| c.reverse()),
                _ => {}
            }
        }
        e.offset = self.dataoff;
        let cc = e.count.wrapping_mul(fields::data_width(e.ty) as u32) as i32;
        if self.seek_ok(e.offset) && self.fd.write(&b, cc) == cc {
            self.dataoff = self
                .dataoff
                .wrapping_add((cc.wrapping_add(1) & !1) as u32);
            return true;
        }
        let f = self
            .field_with_tag(u32::from(e.tag), s)
            .map(|f| f.name.into_owned())
            .unwrap_or_default();
        let m = format!("Error writing data for field \"{f}\"");
        self.error(Some(&self.name.clone()), &m, s);
        false
    }

    /// `TIFFWriteShortArray`.
    fn write_short_array(
        &mut self,
        ty: u16,
        t: u32,
        n: u32,
        v: &[u16],
        out: &mut Vec<Entry>,
        s: &mut dyn Streams,
    ) -> bool {
        let mut e = Entry {
            tag: t as u16,
            ty,
            count: n,
            offset: 0,
        };
        if n <= 2 {
            let v0 = u32::from(v.first().copied().unwrap_or(0));
            let v1 = u32::from(v.get(1).copied().unwrap_or(0));
            e.offset = if self.header.magic == TIFF_BIGENDIAN {
                (v0 << 16) | if n == 2 { v1 & 0xffff } else { 0 }
            } else {
                (v0 & 0xffff) | if n == 2 { v1 << 16 } else { 0 }
            };
            out.push(e);
            return true;
        }
        let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        let ok = self.write_data(&mut e, &bytes, s);
        out.push(e);
        ok
    }

    /// `TIFFWriteLongArray`.
    fn write_long_array(
        &mut self,
        ty: u16,
        t: u32,
        n: u32,
        v: &[u32],
        out: &mut Vec<Entry>,
        s: &mut dyn Streams,
    ) -> bool {
        let mut e = Entry {
            tag: t as u16,
            ty,
            count: n,
            offset: 0,
        };
        if n == 1 {
            e.offset = v.first().copied().unwrap_or(0);
            out.push(e);
            return true;
        }
        let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        let ok = self.write_data(&mut e, &bytes, s);
        out.push(e);
        ok
    }

    /// `TIFFWriteRationalArray` (one value per call here).
    fn write_rational_array(
        &mut self,
        ty: u16,
        t: u32,
        v: &[f32],
        out: &mut Vec<Entry>,
        s: &mut dyn Streams,
    ) -> bool {
        let mut e = Entry {
            tag: t as u16,
            ty,
            count: v.len() as u32,
            offset: 0,
        };
        let mut bytes = Vec::with_capacity(8 * v.len());
        for &x in v {
            let mut fv = x;
            let mut sign = 1.0f64;
            if fv < 0.0 {
                if ty == fields::TIFF_RATIONAL {
                    let f = self
                        .field_with_tag(t, s)
                        .map(|f| f.name.into_owned())
                        .unwrap_or_default();
                    let m = format!(
                        "\"{f}\": Information lost writing value ({}) as (unsigned) RATIONAL",
                        cfmt::g(f64::from(fv), 6)
                    );
                    self.warning(Some(&self.name.clone()), &m, s);
                    fv = 0.0;
                } else {
                    fv = -fv;
                    sign = -1.0;
                }
            }
            let mut den: u32 = 1;
            if fv > 0.0 {
                while fv < (1u32 << 28) as f32 && den < 1u32 << 28 {
                    fv *= 8.0;
                    den = den.wrapping_mul(8);
                }
            }
            let d = sign * (f64::from(fv) + 0.5);
            let num = self.double_to_u32(d);
            bytes.extend_from_slice(&num.to_le_bytes());
            bytes.extend_from_slice(&den.to_le_bytes());
        }
        let ok = self.write_data(&mut e, &bytes, s);
        out.push(e);
        ok
    }

    /// `(uint32) d` as GCC compiles it for x86-64 (`cvttsd2si` to 64 bits, the low 32 kept:
    /// NaN and values out of range give 0 or their low bits); corrected (FS-827), clamped.
    fn double_to_u32(&mut self, d: f64) -> u32 {
        let in_range = d >= 0.0 && d < 4_294_967_296.0;
        if in_range {
            return d as u32;
        }
        let x86 = if d.is_nan() || d.abs() >= 9.223_372_036_854_775_808e18 {
            0
        } else {
            (d as i64) as u32
        };
        let name = self.name.clone();
        self.notices.add(
            &FS_827,
            format!(
                "{name}: writing a rational of {d}: libtiff 3.6.1 converts it to an unsigned \
                 32-bit integer out of range (undefined; x86-64 gives {x86})"
            ),
        );
        if self.fix() {
            if d.is_nan() || d < 0.0 {
                0
            } else {
                u32::MAX
            }
        } else {
            x86
        }
    }

    /// `TIFFWriteAnyArray(tif, type, tag, dir, n, v)`.
    fn write_any_array(
        &mut self,
        ty: u16,
        t: u32,
        v: &[f64],
        out: &mut Vec<Entry>,
        s: &mut dyn Streams,
    ) -> bool {
        let n = v.len() as u32;
        match ty {
            fields::TIFF_BYTE | fields::TIFF_SBYTE => {
                let b: Vec<u8> = v
                    .iter()
                    .map(|&x| if ty == fields::TIFF_BYTE { x as u8 } else { x as i8 as u8 })
                    .collect();
                let mut e = Entry {
                    tag: t as u16,
                    ty,
                    count: n,
                    offset: 0,
                };
                let ok = self.write_byte_array(&mut e, &b, s);
                out.push(e);
                ok
            }
            fields::TIFF_SHORT | fields::TIFF_SSHORT => {
                let w: Vec<u16> = v
                    .iter()
                    .map(|&x| if ty == fields::TIFF_SHORT { x as u16 } else { x as i16 as u16 })
                    .collect();
                self.write_short_array(ty, t, n, &w, out, s)
            }
            fields::TIFF_LONG | fields::TIFF_SLONG => {
                let w: Vec<u32> = v
                    .iter()
                    .map(|&x| if ty == fields::TIFF_LONG { x as u32 } else { x as i32 as u32 })
                    .collect();
                self.write_long_array(ty, t, n, &w, out, s)
            }
            fields::TIFF_FLOAT => {
                let f: Vec<f32> = v.iter().map(|&x| x as f32).collect();
                self.write_float_array(ty, t, &f, out, s)
            }
            fields::TIFF_DOUBLE => {
                let mut e = Entry {
                    tag: t as u16,
                    ty,
                    count: n,
                    offset: 0,
                };
                let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
                let ok = self.write_data(&mut e, &bytes, s);
                out.push(e);
                ok
            }
            _ => false,
        }
    }

    /// `TIFFWriteFloatArray`.
    fn write_float_array(
        &mut self,
        ty: u16,
        t: u32,
        v: &[f32],
        out: &mut Vec<Entry>,
        s: &mut dyn Streams,
    ) -> bool {
        let mut e = Entry {
            tag: t as u16,
            ty,
            count: v.len() as u32,
            offset: 0,
        };
        if v.len() == 1 {
            e.offset = v[0].to_bits();
            out.push(e);
            return true;
        }
        let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        let ok = self.write_data(&mut e, &bytes, s);
        out.push(e);
        ok
    }

    /// `TIFFWriteByteArray(tif, dir, cp)`.
    fn write_byte_array(&mut self, e: &mut Entry, cp: &[u8], s: &mut dyn Streams) -> bool {
        if e.count > 4 {
            return self.write_data(e, cp, s);
        }
        let mut b = [0u8; 4];
        for (i, x) in cp.iter().take(e.count as usize).enumerate() {
            b[i] = *x;
        }
        // _TIFFmemcpy(&dir->tdir_offset, cp, count): the bytes in file order.
        let v = u32::from_le_bytes(b);
        e.offset = if self.swab() { v.swap_bytes() } else { v };
        true
    }

    /// `TIFFWriteNormalTag(tif, dir, fip)` for the scalar fields this port keeps.
    fn write_normal_tag(
        &mut self,
        fip: &fields::FieldInfo,
        out: &mut Vec<Entry>,
        s: &mut dyn Streams,
    ) -> bool {
        let wc = fip.writecount as u16;
        let t = fip.tag;
        let mut e = Entry {
            tag: t as u16,
            ty: fip.ty,
            count: u32::from(wc),
            offset: 0,
        };
        let v = self.get_field(t, s);
        match fip.ty {
            fields::TIFF_SHORT | fields::TIFF_SSHORT => {
                if wc > 1 || fip.passcount {
                    let w: Vec<u16> = match v {
                        Some(Value::ExtraSamples(_, info)) => info,
                        Some(Value::U16(x)) => vec![x],
                        _ => Vec::new(),
                    };
                    let n = w.len() as u32;
                    return self.write_short_array(fip.ty, t, n, &w, out, s);
                }
                let sv = match v {
                    Some(Value::U16(x)) => x,
                    _ => 0,
                };
                e.offset = self.insert_data(e.ty, u32::from(sv));
            }
            fields::TIFF_LONG | fields::TIFF_SLONG | fields::TIFF_IFD => {
                e.offset = match v {
                    Some(Value::U32(x)) => x,
                    Some(Value::U16(x)) => u32::from(x),
                    _ => 0,
                };
            }
            fields::TIFF_RATIONAL | fields::TIFF_SRATIONAL => {
                let fv = match v {
                    Some(Value::F32(x)) => x,
                    _ => 0.0,
                };
                return self.write_rational_array(fip.ty, t, &[fv], out, s);
            }
            fields::TIFF_FLOAT => {
                let fv = match v {
                    Some(Value::F32(x)) => x,
                    _ => 0.0,
                };
                return self.write_float_array(fip.ty, t, &[fv], out, s);
            }
            fields::TIFF_DOUBLE => {
                let dv = match v {
                    Some(Value::F64(x)) => x,
                    _ => 0.0,
                };
                let ok = self.write_data(&mut e, &dv.to_le_bytes(), s);
                out.push(e);
                return ok;
            }
            _ => return true,
        }
        out.push(e);
        true
    }

    /// `TIFFLinkDirectory(tif)`.
    fn link_directory(&mut self, s: &mut dyn Streams) -> bool {
        const MODULE: &str = "TIFFLinkDirectory";
        self.diroff = self.fd.seek(0, Whence::End).wrapping_add(1) & !1;
        let mut diroff = self.diroff;
        if self.swab() {
            diroff = diroff.swap_bytes();
        }
        if self.flags & TIFF_INSUBIFD != 0 {
            self.fd.seek(self.subifdoff, Whence::Set);
            if self.fd.write(&diroff.to_le_bytes(), 4) != 4 {
                let m = format!("{}: Error writing SubIFD directory link", self.name);
                self.error(Some(MODULE), &m, s);
                return false;
            }
            self.nsubifd = self.nsubifd.wrapping_sub(1);
            if self.nsubifd != 0 {
                self.subifdoff = self.subifdoff.wrapping_add(4);
            } else {
                self.flags &= !TIFF_INSUBIFD;
            }
            return true;
        }
        if self.header.diroff == 0 {
            self.header.diroff = self.diroff;
            self.fd.seek(4, Whence::Set);
            if self.fd.write(&diroff.to_le_bytes(), 4) != 4 {
                self.error(Some(&self.name.clone()), "Error writing TIFF header", s);
                return false;
            }
            return true;
        }
        let mut nextdir = self.header.diroff;
        loop {
            let mut c = [0u8; 2];
            if !self.seek_ok(nextdir) || !self.read_ok(&mut c, 2) {
                self.error(Some(MODULE), "Error fetching directory count", s);
                return false;
            }
            let mut dircount = u16::from_le_bytes(c);
            if self.swab() {
                dircount = dircount.swap_bytes();
            }
            self.fd.seek(u32::from(dircount).wrapping_mul(12), Whence::Cur);
            let mut l = [0u8; 4];
            if !self.read_ok(&mut l, 4) {
                self.error(Some(MODULE), "Error fetching directory link", s);
                return false;
            }
            nextdir = u32::from_le_bytes(l);
            if self.swab() {
                nextdir = nextdir.swap_bytes();
            }
            if nextdir == 0 {
                break;
            }
        }
        let off = self.fd.seek(0, Whence::Cur);
        self.fd.seek(off.wrapping_sub(4), Whence::Set);
        if self.fd.write(&diroff.to_le_bytes(), 4) != 4 {
            self.error(Some(MODULE), "Error writing directory link", s);
            return false;
        }
        true
    }
}
