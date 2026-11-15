// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! `tif_read.c`, `tif_strip.c`, `tif_tile.c` and `tif_swab.c`'s bit reversal: the sizes
//! (`TIFFScanlineSize`, `TIFFVStripSize`, `TIFFTileRowSize`, `TIFFVTileSize`, `TIFFTileSize`,
//! `TIFFNumberOfStrips`, `TIFFNumberOfTiles`, `TIFFComputeTile`, `TIFFCheckTile`), reading
//! scanlines (`TIFFReadScanline` with `TIFFSeek`, `TIFFFillStrip`, `TIFFStartStrip`) and tiles
//! (`TIFFReadTile`, `TIFFReadEncodedTile`, `TIFFFillTile`, `TIFFStartTile`), the raw buffer
//! (`TIFFReadBufferSetup`), unmapped.

use super::{Mode, TIFF_CODERSETUP, TIFF_MYBUFFER, TIFF_NOBITREV, Tiff, Unported, values};
use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{FS_826, FS_829, FS_830};

/// `TIFFhowmany(x, y)` in `uint32` (a division by zero is the caller's).
fn howmany(x: u32, y: u32) -> u32 {
    x.wrapping_add(y.wrapping_sub(1)) / y
}

/// `TIFFReverseBits`'s table.
fn reverse(b: u8) -> u8 {
    b.reverse_bits()
}

impl Tiff {
    /// `TIFFScanlineSize(tif)`.
    pub fn scanline_size(&self) -> i32 {
        let d = &self.dir;
        let mut scanline = (u32::from(d.bitspersample).wrapping_mul(d.imagewidth)) as i32;
        if d.planarconfig == values::PLANARCONFIG_CONTIG {
            scanline = scanline.wrapping_mul(i32::from(d.samplesperpixel));
        }
        howmany(scanline as u32, 8) as i32
    }

    /// `TIFFNumberOfStrips(tif)`.
    pub(crate) fn number_of_strips(&self) -> u32 {
        let d = &self.dir;
        let mut n = if d.rowsperstrip == u32::MAX {
            u32::from(d.imagelength != 0)
        } else if d.rowsperstrip == 0 {
            // TIFFhowmany by zero: SIGFPE in C; rowsperstrip 0 is refused by TIFFSetField.
            0
        } else {
            howmany(d.imagelength, d.rowsperstrip)
        };
        if d.planarconfig == values::PLANARCONFIG_SEPARATE {
            n = n.wrapping_mul(u32::from(d.samplesperpixel));
        }
        n
    }

    /// `TIFFNumberOfTiles(tif)`.
    pub(crate) fn number_of_tiles(&self) -> u32 {
        let d = &self.dir;
        let dx = if d.tilewidth == u32::MAX {
            d.imagewidth
        } else {
            d.tilewidth
        };
        let dy = if d.tilelength == u32::MAX {
            d.imagelength
        } else {
            d.tilelength
        };
        let dz = if d.tiledepth == u32::MAX {
            d.imagedepth
        } else {
            d.tiledepth
        };
        let mut n = if dx == 0 || dy == 0 || dz == 0 {
            0
        } else {
            howmany(d.imagewidth, dx)
                .wrapping_mul(howmany(d.imagelength, dy))
                .wrapping_mul(howmany(d.imagedepth, dz))
        };
        if d.planarconfig == values::PLANARCONFIG_SEPARATE {
            n = n.wrapping_mul(u32::from(d.samplesperpixel));
        }
        n
    }

    /// The YCbCr subsampling as `TIFFVStripSize` reads it with `TIFFGetField`: unset, the
    /// C function uses two `uint16`s it never wrote (FS-829; the directory's defaults, 2 and 2,
    /// here).
    fn ycbcr_subsampling_for_size(&mut self) -> [u16; 2] {
        if !self.dir.is_set(super::fields::bit::YCBCRSUBSAMPLING) {
            let name = self.name.clone();
            self.notices.add(
                &FS_829,
                format!(
                    "{name}: a YCbCr image without YCbCrSubsampling: libtiff 3.6.1 computes its \
                     strip size from two values it never set (stack contents); the defaults 2 \
                     and 2 are used"
                ),
            );
        }
        self.dir.ycbcrsubsampling
    }

    /// `TIFFVStripSize(tif, nrows)`.
    pub(crate) fn vstrip_size(&mut self, nrows: u32) -> i32 {
        let mut nrows = nrows;
        if nrows == u32::MAX {
            nrows = self.dir.imagelength;
        }
        if self.dir.planarconfig == values::PLANARCONFIG_CONTIG
            && self.dir.photometric == values::PHOTOMETRIC_YCBCR
            && self.flags & super::TIFF_UPSAMPLED == 0
        {
            let ss = self.ycbcr_subsampling_for_size();
            let d = &self.dir;
            let w = howmany(d.imagewidth, u32::from(ss[0])).wrapping_mul(u32::from(ss[0])) as i32;
            let scanline = howmany((w as u32).wrapping_mul(u32::from(d.bitspersample)), 8) as i32;
            let samplingarea = i32::from(ss[0]) * i32::from(ss[1]);
            let nrows = howmany(nrows, u32::from(ss[1])).wrapping_mul(u32::from(ss[1]));
            let a = (nrows as i32).wrapping_mul(scanline);
            if samplingarea == 0 {
                return a;
            }
            return a.wrapping_add(2i32.wrapping_mul(a / samplingarea));
        }
        (nrows.wrapping_mul(self.scanline_size() as u32)) as i32
    }

    /// `TIFFTileRowSize(tif)`.
    pub fn tile_row_size(&self) -> i32 {
        let d = &self.dir;
        if d.tilelength == 0 || d.tilewidth == 0 {
            return 0;
        }
        let mut rowsize = u32::from(d.bitspersample).wrapping_mul(d.tilewidth) as i32;
        if d.planarconfig == values::PLANARCONFIG_CONTIG {
            rowsize = rowsize.wrapping_mul(i32::from(d.samplesperpixel));
        }
        howmany(rowsize as u32, 8) as i32
    }

    /// `TIFFVTileSize(tif, nrows)`.
    pub(crate) fn vtile_size(&self, nrows: u32) -> i32 {
        let d = &self.dir;
        if d.tilelength == 0 || d.tilewidth == 0 || d.tiledepth == 0 {
            return 0;
        }
        let tilesize = if d.planarconfig == values::PLANARCONFIG_CONTIG
            && d.photometric == values::PHOTOMETRIC_YCBCR
            && self.flags & super::TIFF_UPSAMPLED == 0
        {
            let s0 = u32::from(d.ycbcrsubsampling[0]);
            let s1 = u32::from(d.ycbcrsubsampling[1]);
            if s0 == 0 || s1 == 0 {
                // TIFFroundup by zero: SIGFPE in C.
                return 0;
            }
            let w = howmany(d.tilewidth, s0).wrapping_mul(s0) as i32;
            let rowsize = howmany((w as u32).wrapping_mul(u32::from(d.bitspersample)), 8) as i32;
            let samplingarea = (s0 * s1) as i32;
            let nrows = howmany(nrows, s1).wrapping_mul(s1) as i32;
            let a = nrows.wrapping_mul(rowsize);
            a.wrapping_add(2i32.wrapping_mul(a / samplingarea))
        } else {
            (nrows as i32).wrapping_mul(self.tile_row_size())
        };
        tilesize.wrapping_mul(d.tiledepth as i32)
    }

    /// `TIFFTileSize(tif)`.
    pub fn tile_size(&self) -> i32 {
        self.vtile_size(self.dir.tilelength)
    }

    /// `TIFFComputeTile(tif, x, y, z, s)`.
    pub(crate) fn compute_tile(&self, x: u32, y: u32, z: u32, s: u16) -> u32 {
        let d = &self.dir;
        let (mut dx, mut dy, mut dz) = (d.tilewidth, d.tilelength, d.tiledepth);
        let z = if d.imagedepth == 1 { 0 } else { z };
        if dx == u32::MAX {
            dx = d.imagewidth;
        }
        if dy == u32::MAX {
            dy = d.imagelength;
        }
        if dz == u32::MAX {
            dz = d.imagedepth;
        }
        let mut tile = 1u32;
        if dx != 0 && dy != 0 && dz != 0 {
            let xpt = howmany(d.imagewidth, dx);
            let ypt = howmany(d.imagelength, dy);
            let zpt = howmany(d.imagedepth, dz);
            let base = xpt
                .wrapping_mul(ypt)
                .wrapping_mul(z / dz)
                .wrapping_add(xpt.wrapping_mul(y / dy))
                .wrapping_add(x / dx);
            tile = if d.planarconfig == values::PLANARCONFIG_SEPARATE {
                xpt.wrapping_mul(ypt)
                    .wrapping_mul(zpt)
                    .wrapping_mul(u32::from(s))
                    .wrapping_add(base)
            } else {
                base
            };
        }
        tile
    }

    /// `TIFFCheckTile(tif, x, y, z, s)`.
    fn check_tile(&self, x: u32, y: u32, z: u32, sm: u16, s: &mut dyn Streams) -> bool {
        let d = &self.dir;
        let name = self.name.clone();
        let m = if x >= d.imagewidth {
            format!("Col {x} out of range, max {}", d.imagewidth)
        } else if y >= d.imagelength {
            format!("Row {y} out of range, max {}", d.imagelength)
        } else if z >= d.imagedepth {
            format!("Depth {z} out of range, max {}", d.imagedepth)
        } else if d.planarconfig == values::PLANARCONFIG_SEPARATE && sm >= d.samplesperpixel {
            format!("Sample {sm} out of range, max {}", d.samplesperpixel)
        } else {
            return true;
        };
        self.error(Some(&name), &m, s);
        false
    }

    /// `TIFFCheckRead(tif, tiles)`.
    fn check_read(&self, tiles: bool, s: &mut dyn Streams) -> bool {
        // tif_mode is never O_WRONLY (libtiff opens "w" as O_RDWR).
        let _ = Mode::RdWr;
        if tiles != self.is_tiled() {
            let m = if tiles {
                "Can not read tiles from a stripped image"
            } else {
                "Can not read scanlines from a tiled image"
            };
            self.error(Some(&self.name.clone()), m, s);
            return false;
        }
        true
    }

    /// The strip array entry `i`, as C reads it (a null array: FS-826's crash, 0 here).
    fn strip_entry(&mut self, counts: bool, i: u32) -> Option<u32> {
        let a = if counts {
            &self.dir.stripbytecount
        } else {
            &self.dir.stripoffset
        };
        match a.as_ref().map(|v| v.get(i as usize).copied()) {
            Some(Some(v)) => Some(v),
            _ => {
                let name = self.name.clone();
                self.notices.add(
                    &FS_826,
                    format!(
                        "{name}: reading strip or tile {i} past its directory's arrays (a \
                         directory that failed to read, or an index out of range): libtiff \
                         3.6.1 reads memory outside them (a crash); refused"
                    ),
                );
                None
            }
        }
    }

    /// A buffer of `cc` bytes when the caller's is shorter (libtiff writes `cc` bytes there:
    /// FS-830 when the size is the previous directory's), its start the caller's bytes.
    fn caller_buffer(&mut self, buf: &[u8], cc: i32, what: &str) -> Option<Vec<u8>> {
        let cc = cc.max(0) as usize;
        if cc <= buf.len() {
            return None;
        }
        let name = self.name.clone();
        self.notices.add(
            &FS_830,
            format!(
                "{name}: libtiff writes a {what} of {cc} bytes into a buffer of {} (the size \
                 TIFFScanlineSize or TIFFTileSize gives now; libtiff keeps the previous \
                 directory's after a TIFFReadDirectory that failed): memory past the buffer",
                buf.len()
            ),
        );
        let mut t = vec![0u8; cc];
        t[..buf.len()].copy_from_slice(buf);
        Some(t)
    }

    /// `TIFFReadScanline(tif, buf, row, sample)`: 1, or -1. `buf` receives
    /// `TIFFScanlineSize` bytes (a shorter slice is filled as far as it goes: the C function
    /// writes past the caller's buffer).
    pub fn read_scanline(
        &mut self,
        buf: &mut [u8],
        row: u32,
        sample: u16,
        s: &mut dyn Streams,
    ) -> Result<i32, Unported> {
        if !self.check_read(false, s) {
            return Ok(-1);
        }
        let mut e: i32 = i32::from(self.seek_row(row, sample, s));
        if let Some(u) = self.unported {
            return Err(u);
        }
        if e != 0 {
            let m = self.codec.decoderow;
            let cc = self.scanlinesize;
            let mut tmp = self.caller_buffer(buf, cc, "scanline");
            let b: &mut [u8] = match tmp.as_mut() {
                Some(t) => t,
                None => buf,
            };
            e = self.decode(m, b, cc, sample, s);
            self.row = row.wrapping_add(1);
            if e != 0 {
                self.post_decode(b, cc);
            }
            if let Some(t) = tmp {
                let n = buf.len();
                buf.copy_from_slice(&t[..n]);
            }
        }
        Ok(if e > 0 { 1 } else { -1 })
    }

    /// `TIFFSeek(tif, row, sample)`.
    fn seek_row(&mut self, row: u32, sample: u16, s: &mut dyn Streams) -> bool {
        let d = &self.dir;
        if row >= d.imagelength {
            let m = format!("{row}: Row out of range, max {}", d.imagelength);
            self.error(Some(&self.name.clone()), &m, s);
            return false;
        }
        let strip;
        if d.planarconfig == values::PLANARCONFIG_SEPARATE {
            if sample >= d.samplesperpixel {
                let m = format!("{sample}: Sample out of range, max {}", d.samplesperpixel);
                self.error(Some(&self.name.clone()), &m, s);
                return false;
            }
            strip = u32::from(sample)
                .wrapping_mul(d.stripsperimage)
                .wrapping_add(row / d.rowsperstrip);
        } else {
            strip = row / d.rowsperstrip;
        }
        if strip != self.curstrip {
            if !self.fill_strip(strip, s) {
                return false;
            }
        } else if row < self.row && !self.start_strip(strip, s) {
            return false;
        }
        if row != self.row {
            if !self.codec_seek(row.wrapping_sub(self.row), s) {
                return false;
            }
            self.row = row;
        }
        true
    }

    /// `TIFFReadRawStrip1` / `TIFFReadRawTile1` into the raw buffer.
    fn read_raw1(
        &mut self,
        strip: u32,
        size: i32,
        module: &str,
        tile: bool,
        s: &mut dyn Streams,
    ) -> i32 {
        let Some(off) = self.strip_entry(false, strip) else {
            return -1;
        };
        let name = self.name.clone();
        if !self.seek_ok(off) {
            let m = if tile {
                format!(
                    "{name}: Seek error at row {}, col {}, tile {strip}",
                    self.row, self.col
                )
            } else {
                format!("{name}: Seek error at scanline {}, strip {strip}", self.row)
            };
            self.error(Some(module), &m, s);
            return -1;
        }
        let mut raw = std::mem::take(&mut self.rawdata);
        let n = (size.max(0) as usize).min(raw.len());
        let cc = self.fd.read(&mut raw[..n], size);
        if cc > 0 {
            self.raw_defined = self.raw_defined.max(cc as usize);
        }
        self.rawdata = raw;
        if cc != size {
            let m = if tile {
                format!(
                    "{name}: Read error at row {}, col {}; got {} bytes, expected {}",
                    self.row as i64, self.col as i64, cc as i64 as u64, size as i64 as u64
                )
            } else {
                format!(
                    "{name}: Read error at scanline {}; got {} bytes, expected {}",
                    self.row, cc as i64 as u64, size as i64 as u64
                )
            };
            self.error(Some(module), &m, s);
            return -1;
        }
        size
    }

    /// `TIFFReadBufferSetup(tif, 0, size)`.
    fn read_buffer_setup(&mut self, size: u32, s: &mut dyn Streams) -> bool {
        self.rawdata = Vec::new();
        self.raw_defined = 0;
        // size and tif_rawdatasize are tsize_t: a size of 2 GiB is negative and malloc fails.
        let n = howmany(size, 1024).wrapping_mul(1024);
        self.flags |= TIFF_MYBUFFER;
        if (n as i32) < 0 || !self.can_alloc(u64::from(n)) {
            let m = format!(
                "{}: No space for data buffer at scanline {}",
                self.name, self.row
            );
            self.error(Some("TIFFReadBufferSetup"), &m, s);
            return false;
        }
        self.rawdata = vec![0u8; n as usize];
        true
    }

    /// `TIFFFillStrip(tif, strip)`.
    pub(crate) fn fill_strip(&mut self, strip: u32, s: &mut dyn Streams) -> bool {
        self.fill(strip, false, s)
    }

    fn fill(&mut self, strip: u32, tile: bool, s: &mut dyn Streams) -> bool {
        let module = if tile {
            "TIFFFillTile"
        } else {
            "TIFFFillStrip"
        };
        let Some(bc) = self.strip_entry(true, strip) else {
            return false;
        };
        let bytecount = bc as i32;
        if bytecount <= 0 {
            let what = if tile { "tile" } else { "strip" };
            let m = format!(
                "{}: Invalid {what} byte count, {what} {strip}",
                bytecount as i64 as u64
            );
            self.error(Some(&self.name.clone()), &m, s);
            return false;
        }
        if bytecount as usize > self.rawdata.len() {
            if tile {
                self.curtile = u32::MAX;
            } else {
                self.curstrip = u32::MAX;
            }
            if self.flags & TIFF_MYBUFFER == 0 {
                let m = if tile {
                    format!("{}: Data buffer too small to hold tile {strip}", self.name)
                } else {
                    format!("{}: Data buffer too small to hold strip {strip}", self.name)
                };
                self.error(Some(module), &m, s);
                return false;
            }
            if !self.read_buffer_setup(howmany(bytecount as u32, 1024).wrapping_mul(1024), s) {
                return false;
            }
        }
        if self.read_raw1(strip, bytecount, module, tile, s) != bytecount {
            return false;
        }
        let fillorder = u32::from(self.dir.fillorder);
        if self.flags & fillorder == 0 && self.flags & TIFF_NOBITREV == 0 {
            for b in &mut self.rawdata[..bytecount as usize] {
                *b = reverse(*b);
            }
        }
        if tile {
            self.start_tile(strip, s)
        } else {
            self.start_strip(strip, s)
        }
    }

    /// The codec's setup, once per directory (`TIFF_CODERSETUP`).
    fn coder_setup(&mut self, s: &mut dyn Streams) -> bool {
        if self.flags & TIFF_CODERSETUP == 0 {
            if !self.setup_decode(s) {
                return false;
            }
            self.flags |= TIFF_CODERSETUP;
        }
        true
    }

    /// `TIFFStartStrip(tif, strip)`.
    pub(crate) fn start_strip(&mut self, strip: u32, s: &mut dyn Streams) -> bool {
        if !self.coder_setup(s) {
            return false;
        }
        self.curstrip = strip;
        let spi = self.dir.stripsperimage;
        self.row = if spi == 0 {
            0
        } else {
            (strip % spi).wrapping_mul(self.dir.rowsperstrip)
        };
        self.rawcp = 0;
        self.rawcc = self.strip_entry(true, strip).unwrap_or(0) as i32;
        self.pre_decode(s)
    }

    /// `TIFFStartTile(tif, tile)`.
    fn start_tile(&mut self, tile: u32, s: &mut dyn Streams) -> bool {
        if !self.coder_setup(s) {
            return false;
        }
        self.curtile = tile;
        let d = &self.dir;
        let hx = howmany(d.imagewidth, d.tilewidth);
        let hy = howmany(d.imagelength, d.tilelength);
        self.row = if hx == 0 {
            0
        } else {
            (tile % hx).wrapping_mul(d.tilelength)
        };
        self.col = if hy == 0 {
            0
        } else {
            (tile % hy).wrapping_mul(d.tilewidth)
        };
        self.rawcp = 0;
        self.rawcc = self.strip_entry(true, tile).unwrap_or(0) as i32;
        self.pre_decode(s)
    }

    /// `TIFFReadTile(tif, buf, x, y, z, s)`: the bytes decoded (`TIFFTileSize`), or -1.
    pub fn read_tile(
        &mut self,
        buf: &mut [u8],
        x: u32,
        y: u32,
        z: u32,
        sample: u16,
        s: &mut dyn Streams,
    ) -> Result<i32, Unported> {
        if !self.check_read(true, s) || !self.check_tile(x, y, z, sample, s) {
            return Ok(-1);
        }
        let t = self.compute_tile(x, y, z, sample);
        let r = self.read_encoded_tile(t, buf, -1, s);
        match self.unported {
            Some(u) => Err(u),
            None => Ok(r),
        }
    }

    /// `TIFFReadEncodedTile(tif, tile, buf, size)`.
    fn read_encoded_tile(
        &mut self,
        tile: u32,
        buf: &mut [u8],
        size: i32,
        s: &mut dyn Streams,
    ) -> i32 {
        let tilesize = self.tilesize;
        if !self.check_read(true, s) {
            return -1;
        }
        if tile >= self.dir.nstrips {
            let m = format!("{tile}: Tile out of range, max {}", self.dir.nstrips);
            self.error(Some(&self.name.clone()), &m, s);
            return -1;
        }
        let size = if size == -1 || size > tilesize {
            tilesize
        } else {
            size
        };
        if self.fill(tile, true, s) {
            let spi = self.dir.stripsperimage;
            let sample = if spi == 0 { 0 } else { (tile / spi) as u16 };
            let m = self.codec.decodetile;
            let mut tmp = self.caller_buffer(buf, size, "tile");
            let b: &mut [u8] = match tmp.as_mut() {
                Some(t) => t,
                None => buf,
            };
            let ok = self.decode(m, b, size, sample, s) != 0;
            if ok {
                self.post_decode(b, size);
            }
            if let Some(t) = tmp {
                let n = buf.len();
                buf.copy_from_slice(&t[..n]);
            }
            if ok {
                return size;
            }
        }
        -1
    }
}
