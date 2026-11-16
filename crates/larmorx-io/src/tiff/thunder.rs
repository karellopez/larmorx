// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! `tif_thunder.c`: the ThunderScan 4-bit decoder (rows only; no encoder in libtiff 3.6.1).
//!
//! libtiff 3.6.1 writes a run of pixels in full, and the deltas of a byte, even when they go past
//! the row's pixel count, into the next row or past the caller's buffer (FS-837): by default
//! nothing is written past the row (the pixel count, and so the "Too much data" error, as
//! 3.6.1's); with `reproduce` it is 3.6.1's, its writes past the caller's buffer dropped
//! (FS-835). (libtiff 4.0.10 also stops at the row's end, but leaves a run that ends exactly at
//! it unwritten.)

use super::Tiff;
use super::next::Out;
use larmorx_core::cfmt::Streams;
use larmorx_core::defects::FS_837;

const THUNDER_CODE: i32 = 0xc0;
const THUNDER_RUN: i32 = 0x00;
const THUNDER_2BITDELTAS: i32 = 0x40;
const DELTA2_SKIP: i32 = 2;
const THUNDER_3BITDELTAS: i32 = 0x80;
const DELTA3_SKIP: i32 = 4;

const TWOBITDELTAS: [i32; 4] = [0, 1, 0, -1];
const THREEBITDELTAS: [i32; 8] = [0, 1, 2, 3, 0, -3, -2, -1];

impl Tiff {
    /// `ThunderDecodeRow` (libtiff 4.0.10's by default, 3.6.1's with `reproduce`).
    pub(crate) fn thunder_decode_row(
        &mut self,
        buf: &mut [u8],
        occ0: i32,
        s: &mut dyn Streams,
    ) -> i32 {
        let fix = self.fix();
        let scanline = self.scanlinesize;
        let mut o = Out { buf, past: false };
        let mut occ = i64::from(occ0);
        let mut row: i64 = 0;
        let mut r = 1;
        while occ > 0 {
            // `ThunderDecode(tif, row, tif->tif_dir.td_imagewidth)`: a tsize_t
            let maxpixels = self.dir.imagewidth as i32;
            if !self.thunder_decode(&mut o, row, maxpixels, fix, s) {
                r = 0;
                break;
            }
            occ -= i64::from(scanline);
            row += i64::from(scanline);
        }
        if o.past {
            self.past_buffer_notice("ThunderScan");
        }
        r
    }

    /// `ThunderDecode` of one row at `op`.
    fn thunder_decode(
        &mut self,
        o: &mut Out,
        mut op: i64,
        maxpixels: i32,
        fix: bool,
        s: &mut dyn Streams,
    ) -> bool {
        let mut bp = self.rawcp;
        let end = self.rawcp + self.rawcc.max(0) as usize;
        let mut cc = self.rawcc;
        let mut lastpixel: u32 = 0;
        let mut npixels: i32 = 0;
        let mut beyond = false;
        // SETPIXEL; by default nothing written past the row
        macro_rules! setpixel {
            ($v:expr) => {{
                lastpixel = ($v as u32) & 0xf;
                let odd = npixels & 1 != 0;
                if npixels >= maxpixels {
                    beyond = true;
                }
                if !fix || npixels < maxpixels {
                    if odd {
                        o.or(op, lastpixel as u8);
                    } else {
                        o.set(op, (lastpixel << 4) as u8);
                    }
                }
                if odd {
                    op += 1;
                }
                npixels = npixels.wrapping_add(1);
            }};
        }
        while cc > 0 && npixels < maxpixels {
            let mut n = i32::from(self.raw_byte(bp, end));
            bp += 1;
            cc -= 1;
            match n & THUNDER_CODE {
                THUNDER_RUN if fix => {
                    // the run's pixels one by one, none past the row
                    let v = lastpixel & 0xf;
                    for _ in 0..n & 0x3f {
                        setpixel!(v);
                    }
                    lastpixel = v;
                }
                THUNDER_RUN => {
                    if npixels & 1 != 0 {
                        o.or(op, lastpixel as u8);
                        lastpixel = u32::from(o.get(op));
                        op += 1;
                        npixels = npixels.wrapping_add(1);
                        n -= 1;
                    } else {
                        lastpixel |= lastpixel << 4;
                    }
                    npixels = npixels.wrapping_add(n);
                    if npixels > maxpixels {
                        beyond = true;
                    }
                    while n > 0 {
                        o.set(op, lastpixel as u8);
                        op += 1;
                        n -= 2;
                    }
                    if n == -1 {
                        op -= 1;
                        let b = o.get(op);
                        o.set(op, b & 0xf0);
                    }
                    lastpixel &= 0xf;
                }
                THUNDER_2BITDELTAS => {
                    let d = (n >> 4) & 3;
                    if d != DELTA2_SKIP {
                        setpixel!((lastpixel as i32).wrapping_add(TWOBITDELTAS[d as usize]));
                    }
                    let d = (n >> 2) & 3;
                    if d != DELTA2_SKIP {
                        setpixel!((lastpixel as i32).wrapping_add(TWOBITDELTAS[d as usize]));
                    }
                    let d = n & 3;
                    if d != DELTA2_SKIP {
                        setpixel!((lastpixel as i32).wrapping_add(TWOBITDELTAS[d as usize]));
                    }
                }
                THUNDER_3BITDELTAS => {
                    let d = (n >> 3) & 7;
                    if d != DELTA3_SKIP {
                        setpixel!((lastpixel as i32).wrapping_add(THREEBITDELTAS[d as usize]));
                    }
                    let d = n & 7;
                    if d != DELTA3_SKIP {
                        setpixel!((lastpixel as i32).wrapping_add(THREEBITDELTAS[d as usize]));
                    }
                }
                _ => setpixel!(n),
            }
        }
        self.rawcp = bp;
        self.rawcc = cc;
        if beyond {
            let name = self.name.clone();
            self.notices.add(
                &FS_837,
                format!(
                    "ThunderScan data of {name}: pixels past the end of a row{}",
                    if fix {
                        " not written (corrected)"
                    } else {
                        " written into the bytes after it"
                    }
                ),
            );
        }
        if npixels != maxpixels {
            let m = format!(
                "ThunderDecode: {} data at scanline {} ({} != {})",
                if npixels < maxpixels {
                    "Not enough"
                } else {
                    "Too much"
                },
                i64::from(self.row as i32),
                i64::from(npixels) as u64,
                i64::from(maxpixels) as u64
            );
            self.error(Some(&self.name.clone()), &m, s);
            return false;
        }
        true
    }
}
