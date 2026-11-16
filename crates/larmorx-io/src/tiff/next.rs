// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! `tif_next.c`: the NeXT 2-bit grey-scale decoder (no encoder in libtiff 3.6.1).
//!
//! libtiff 3.6.1 reads a row's code byte and a literal span's four-byte header without checking
//! that the strip has them, does not check a span against the row, and lets a run of pixels go
//! past the image width (FS-836): by default the decoder is libtiff 4.0.10's, which checks all
//! three (and, for tiles, counts a tile row's pixels, not the image's); with `reproduce` it is
//! 3.6.1's, its reads past the strip's data those of the raw buffer (FS-817, FS-819) and its
//! writes past the caller's buffer dropped (FS-835).

use super::Tiff;
use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{FS_835, FS_836};

/// The decoders' output: writes and reads past the caller's buffer are dropped (read as 0) and
/// noted.
pub(crate) struct Out<'a> {
    pub buf: &'a mut [u8],
    pub past: bool,
}

impl Out<'_> {
    #[inline]
    pub fn set(&mut self, at: i64, v: u8) {
        if at >= 0 && (at as usize) < self.buf.len() {
            self.buf[at as usize] = v;
        } else {
            self.past = true;
        }
    }
    #[inline]
    pub fn get(&mut self, at: i64) -> u8 {
        if at >= 0 && (at as usize) < self.buf.len() {
            self.buf[at as usize]
        } else {
            self.past = true;
            0
        }
    }
    #[inline]
    pub fn or(&mut self, at: i64, v: u8) {
        let b = self.get(at);
        self.set(at, b | v);
    }
}

impl Tiff {
    /// FS-835's notice: a decoder wrote past the caller's buffer.
    pub(crate) fn past_buffer_notice(&mut self, codec: &str) {
        let name = self.name.clone();
        self.notices.add(
            &FS_835,
            format!(
                "the {codec} decoder wrote past the buffer it was given while decoding {name} \
                 (libtiff 3.6.1 writes into the memory after it); only the buffer is written"
            ),
        );
    }

    /// `NeXTDecode` (libtiff 4.0.10's by default, 3.6.1's with `reproduce`).
    pub(crate) fn next_decode(&mut self, buf: &mut [u8], occ0: i32, s: &mut dyn Streams) -> i32 {
        let mut o = Out { buf, past: false };
        let r = if self.fix() {
            self.next_decode_4(&mut o, occ0, s)
        } else {
            self.next_decode_3(&mut o, occ0, s)
        };
        if o.past {
            self.past_buffer_notice("NeXT");
        }
        r
    }

    fn next_bad(&self, s: &mut dyn Streams) -> i32 {
        let m = format!(
            "NeXTDecode: Not enough data for scanline {}",
            i64::from(self.row as i32)
        );
        self.error(Some(&self.name.clone()), &m, s);
        0
    }

    fn next_notice(&mut self, what: &str) {
        let name = self.name.clone();
        self.notices.add(
            &FS_836,
            format!("NeXT data of {name}: {what} (corrected as libtiff 4.0.10 does)"),
        );
    }

    /// libtiff 3.6.1's `NeXTDecode`.
    fn next_decode_3(&mut self, o: &mut Out, occ0: i32, s: &mut dyn Streams) -> i32 {
        let mut occ = i64::from(occ0);
        for k in 0..occ.max(0) {
            o.set(k, 0xff);
        }
        let mut bp = self.rawcp;
        let end = self.rawcp + self.rawcc.max(0) as usize;
        let mut cc = self.rawcc;
        let scanline = self.scanlinesize;
        let mut row: i64 = 0;
        while occ > 0 {
            let n0 = self.raw_byte(bp, end);
            bp += 1;
            cc = cc.wrapping_sub(1);
            match n0 {
                0x00 => {
                    if cc < scanline {
                        return self.next_bad(s);
                    }
                    for k in 0..scanline.max(0) as usize {
                        let b = self.raw_byte(bp + k, end);
                        o.set(row + k as i64, b);
                    }
                    bp += scanline.max(0) as usize;
                    cc = cc.wrapping_sub(scanline);
                }
                0x40 => {
                    let off = i64::from(self.raw_byte(bp, end)) * 256
                        + i64::from(self.raw_byte(bp + 1, end));
                    let n = i32::from(self.raw_byte(bp + 2, end)) * 256
                        + i32::from(self.raw_byte(bp + 3, end));
                    if cc < 4 + n {
                        return self.next_bad(s);
                    }
                    for k in 0..n as usize {
                        let b = self.raw_byte(bp + 4 + k, end);
                        o.set(row + off + k as i64, b);
                    }
                    bp += 4 + n as usize;
                    cc = cc.wrapping_sub(4 + n);
                }
                _ => {
                    let mut n = i32::from(n0);
                    let mut npixels: i32 = 0;
                    let imagewidth = self.dir.imagewidth;
                    let mut op = row;
                    loop {
                        let grey = ((n >> 6) & 3) as u8;
                        n &= 0x3f;
                        while n > 0 {
                            n -= 1;
                            match npixels & 3 {
                                0 => o.set(op, grey << 6),
                                1 => o.or(op, grey << 4),
                                2 => o.or(op, grey << 2),
                                _ => {
                                    o.or(op, grey);
                                    op += 1;
                                }
                            }
                            npixels = npixels.wrapping_add(1);
                        }
                        // `npixels >= (int) imagewidth`
                        if npixels >= imagewidth as i32 {
                            break;
                        }
                        if cc == 0 {
                            return self.next_bad(s);
                        }
                        n = i32::from(self.raw_byte(bp, end));
                        bp += 1;
                        cc = cc.wrapping_sub(1);
                    }
                }
            }
            occ -= i64::from(scanline);
            row += i64::from(scanline);
        }
        self.rawcp = bp;
        self.rawcc = cc;
        1
    }

    /// libtiff 4.0.10's `NeXTDecode`: the reads and writes checked.
    fn next_decode_4(&mut self, o: &mut Out, occ0: i32, s: &mut dyn Streams) -> i32 {
        let mut occ = i64::from(occ0);
        for k in 0..occ.max(0) {
            o.set(k, 0xff);
        }
        let mut bp = self.rawcp;
        let end = self.rawcp + self.rawcc.max(0) as usize;
        let mut cc = i64::from(self.rawcc);
        let scanline = i64::from(self.scanlinesize);
        if scanline != 0 && occ % scanline != 0 {
            self.next_notice("a buffer that is not a whole number of scanlines");
            self.error(Some("NeXTDecode"), "Fractional scanlines cannot be read", s);
            return 0;
        }
        let imagewidth = if self.is_tiled() {
            self.dir.tilewidth
        } else {
            self.dir.imagewidth
        };
        if self.is_tiled() && self.dir.tilewidth != self.dir.imagewidth {
            self.next_notice("a tile's runs counted in tile pixels, not image pixels");
        }
        let mut row: i64 = 0;
        while cc > 0 && occ > 0 {
            let n0 = self.raw_byte(bp, end);
            bp += 1;
            cc -= 1;
            match n0 {
                0x00 => {
                    if cc < scanline {
                        return self.next_bad(s);
                    }
                    for k in 0..scanline.max(0) {
                        let b = self.raw_byte(bp + k as usize, end);
                        o.set(row + k, b);
                    }
                    bp += scanline.max(0) as usize;
                    cc -= scanline;
                }
                0x40 => {
                    if cc < 4 {
                        self.next_notice("a literal span's header past the strip's data");
                        return self.next_bad(s);
                    }
                    let off = i64::from(self.raw_byte(bp, end)) * 256
                        + i64::from(self.raw_byte(bp + 1, end));
                    let n = i64::from(self.raw_byte(bp + 2, end)) * 256
                        + i64::from(self.raw_byte(bp + 3, end));
                    if cc < 4 + n || off + n > scanline {
                        if cc >= 4 + n {
                            self.next_notice("a literal span past the end of its row");
                        }
                        return self.next_bad(s);
                    }
                    for k in 0..n {
                        let b = self.raw_byte(bp + 4 + k as usize, end);
                        o.set(row + off + k, b);
                    }
                    bp += 4 + n as usize;
                    cc -= 4 + n;
                }
                _ => {
                    let mut n = i32::from(n0);
                    let mut npixels: u32 = 0;
                    let mut op_offset: i64 = 0;
                    let mut overshoot = false;
                    loop {
                        let grey = ((n >> 6) & 3) as u8;
                        n &= 0x3f;
                        while n > 0 {
                            n -= 1;
                            if npixels >= imagewidth || op_offset >= scanline {
                                overshoot = true;
                                break;
                            }
                            let op = row + op_offset;
                            match npixels & 3 {
                                0 => o.set(op, grey << 6),
                                1 => o.or(op, grey << 4),
                                2 => o.or(op, grey << 2),
                                _ => {
                                    o.or(op, grey);
                                    op_offset += 1;
                                }
                            }
                            npixels += 1;
                        }
                        if overshoot {
                            self.next_notice("a run of pixels past the end of its row");
                            overshoot = false;
                        }
                        if npixels >= imagewidth {
                            break;
                        }
                        if op_offset >= scanline {
                            self.next_notice("runs past the end of their row");
                            let m =
                                format!("Invalid data for scanline {}", i64::from(self.row as i32));
                            self.error(Some("NeXTDecode"), &m, s);
                            return 0;
                        }
                        if cc == 0 {
                            return self.next_bad(s);
                        }
                        n = i32::from(self.raw_byte(bp, end));
                        bp += 1;
                        cc -= 1;
                    }
                }
            }
            occ -= scanline;
            row += scanline;
        }
        if occ > 0 {
            self.next_notice("a strip that ends before its rows: the rest left white");
        }
        self.rawcp = bp;
        self.rawcc = cc as i32;
        1
    }
}
