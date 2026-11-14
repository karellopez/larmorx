// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's SGI image library (`utils/rgb.cpp`, `include/rgb.h`: Paul Haeberli's 1984 image
//! library as FreeSurfer keeps it) as `RGBReadImage` and `RGBwrite` (`utils/imageio.cpp`) use it:
//! `iopen` for reading and for writing, `getrow`, `putrow`, `iclose`, `img_seek`, `img_read`,
//! `img_write`, `img_optseek`, `img_badrow`, `img_getrowsize`, `img_rle_expand`, `cvtimage`,
//! `cvtshorts`, `swapImage`, `isetname` and `i_errhdlr`.
//!
//! The file's header is the library's `RGB_IMAGE` structure itself, read and written as its 176
//! bytes in x86-64's layout: `imagic`, `type`, `dim`, `xsize`, `ysize`, `zsize` (16 bits each),
//! `min`, `max`, `wastebytes` (32 bits), `name` (80 bytes), `colormap`, `file` (the descriptor),
//! `flags`, `dorev`, `x`, `y`, `z`, `cnt` (16 bits), 4 bytes of padding, the pointers `ptr`,
//! `base`, `tmpbuf`, `offset`, `rleend`, the pointers `rowstart`, `rowsize`. The writer converts
//! the first six, `min`, `max`, `wastebytes` (as 16 bits) and `colormap` to big-endian (SGI's order)
//! and leaves the rest as the machine has them; the pixels start at byte 512 (the bytes between
//! are the gap the seek leaves, zeros). A file whose magic number (474, `IMAGIC`) is read swapped
//! has its first six fields swapped back (`dorev`): FreeSurfer's own files and SGI's standard ones.
//!
//! Every message is `i_errhdlr`'s: FreeSurfer never installs an error function, so the message
//! goes to standard error and the program ends with status 1.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-io.md` Part 25):
//! - **FS-720**: reading a header in the swapped order converts 8 bytes past the header's memory;
//!   both modes convert nothing past it, noted.
//! - **FS-721**: run-length (RLE) images in the machine's order: FreeSurfer reads the tables as
//!   8-byte entries, so the row lengths come from the bytes after twice the row-start table (the
//!   image data). The default reads SGI's tables (32-bit entries in the file's order); `reproduce`
//!   reads FreeSurfer's, noted.
//! - **FS-722**: the writer's header holds the file's descriptor number and the address of a buffer
//!   (memory contents); both modes write 0 there, noted.
//! - **FS-723**: a row of 16,384 to 32,767 2-byte pixels is read with a negative 16-bit byte count:
//!   the rest of the file is read into the row's buffer and the read fails (`getrow: error on read
//!   of row`). The default reads the row; `reproduce` fails as FreeSurfer, noted. **FS-724**: the
//!   rest of the file is longer than the row's buffer: written past it (memory corruption); both
//!   modes as FS-723, noted.
//! - **FS-725**: a writer that cannot create its file names it `(null)` in its message (the
//!   format's `%s` given an `int` 0); the default prints the name.
//! - **FS-726**: a run-length row whose bytes or expansion leave the library's buffers (a length
//!   above the buffer, an expansion that does not stop at the row's width or runs past the
//!   buffer): FreeSurfer reads and writes past them; both modes stay within them (bytes outside
//!   read as nothing, values past the row dropped), noted.
//! - **FS-727**: pixels FreeSurfer takes from memory contents: a buffer's values never read (a
//!   run-length row of length -1 or shorter than the width, FS-728's rows), or a run-length row
//!   expanded from bytes no read gave; both modes 0, noted.
//! - **FS-728**: a row of 32,768 or more 2-byte pixels is read with a 16-bit byte count: only part
//!   of the row is read, the rest is the buffer's earlier contents. The default reads the row;
//!   `reproduce` reads FreeSurfer's part, noted.
//! - **FS-729**: run-length images in the swapped order (SGI's standard files): FreeSurfer converts
//!   the tables past their memory (memory corruption, a crash or a hang); both modes read SGI's
//!   tables, noted.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{
    FS_720, FS_721, FS_722, FS_723, FS_724, FS_725, FS_726, FS_727, FS_728, FS_729,
};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;

use crate::cfile::CFile;

/// `IMAGIC`: the magic number (0732).
pub const IMAGIC: u16 = 0o732;
/// `ITYPE_VERBATIM`, `ITYPE_RLE`.
pub const ITYPE_VERBATIM: u16 = 0x0000;
pub const ITYPE_RLE: u16 = 0x0100;
/// `sizeof(RGB_IMAGE)` on x86-64: the header's bytes.
pub const HEADER: usize = 176;
/// Where the pixels (and the run-length tables) start.
const DATA: u64 = 512;
/// `_IOREAD`, `_IOWRT`, `_IORW`.
const IOREAD: u16 = 0o1;
const IOWRT: u16 = 0o2;
const IORW: u16 = 0o200;

fn is_rle(t: u16) -> bool {
    t & 0xff00 == ITYPE_RLE
}

fn is_verbatim(t: u16) -> bool {
    t & 0xff00 == ITYPE_VERBATIM
}

fn bpp(t: u16) -> u16 {
    t & 0x00ff
}

/// `IBUFSIZE(pixels)`: the bytes of the library's row buffers.
fn ibufsize(px: u16) -> usize {
    let p = i32::from(px);
    ((p + (p >> 6)) << 2) as usize
}

/// `i_errhdlr(fmt, ...)` without an error function: the message on standard error, `exit(1)`.
fn errhdlr(err: &mut dyn Write, msg: &str) -> Fatal {
    let _ = err.write_all(msg.as_bytes());
    Fatal::Exit(1)
}

/// The run-length tables, as the reader uses them.
struct Rle {
    rowstart: Vec<u32>,
    rowsize: Vec<i32>,
}

/// An open SGI image (`RGB_IMAGE`).
pub struct RgbImage {
    pub itype: u16,
    pub dim: u16,
    pub xsize: u16,
    pub ysize: u16,
    pub zsize: u16,
    min: u32,
    max: u32,
    flags: u16,
    dorev: bool,
    y: u16,
    z: u16,
    /// `offset` (`unsigned int`): where the library believes the file is.
    offset: u32,
    rle: Option<Rle>,
    /// `tmpbuf`: [`ibufsize`] bytes; `filled` of them ever written.
    tmpbuf: Vec<u8>,
    filled: usize,
    f: CFile,
}

impl RgbImage {
    /// `img_optseek(image, offset)`.
    fn optseek(&mut self, g: &mut Globals, off: u64) {
        if u64::from(self.offset) != off {
            self.offset = off as u32;
            let mut e = g.errno;
            if self.f.fseek(&mut e, off.min(i64::MAX as u64) as i64, 0) != 0 {
                g.errno = e;
            }
        }
    }

    /// `img_read(image, buffer, count)`: `read(2)` of `count` bytes (a negative count is a huge
    /// one: everything left); the bytes read, the offset moved on or set to -1.
    fn read(&mut self, buf: &mut Vec<u8>, count: i64) -> i64 {
        let want = if count < 0 {
            usize::MAX
        } else {
            count as usize
        };
        let mut got = Vec::new();
        let mut chunk = vec![0u8; 65536];
        while got.len() < want {
            let n = (want - got.len()).min(chunk.len());
            let k = self.f.read_bytes(&mut chunk[..n]);
            got.extend_from_slice(&chunk[..k]);
            if k < n {
                break;
            }
        }
        let n = got.len() as i64;
        if n == count {
            self.offset = self.offset.wrapping_add(count as u32);
        } else {
            self.offset = u32::MAX;
        }
        *buf = got;
        n
    }

    /// `img_write(image, buffer, count)`.
    fn write(&mut self, b: &[u8]) -> bool {
        let ok = self.f.fwrite(b, 1, b.len()) == b.len() || b.is_empty();
        if ok {
            self.offset = self.offset.wrapping_add(b.len() as u32);
        } else {
            self.offset = u32::MAX;
        }
        ok
    }

    /// `img_seek(image, y, z)`: a row outside the image or an unknown dimension or type ends the
    /// program.
    fn seek(&mut self, g: &mut Globals, err: &mut dyn Write, y: u32, z: u32) -> Result<(), Fatal> {
        if y >= u32::from(self.ysize) || z >= u32::from(self.zsize) {
            return Err(errhdlr(err, "img_seek: row number out of range\n"));
        }
        self.y = y as u16;
        self.z = z as u16;
        let (xs, ys, b) = (
            u32::from(self.xsize),
            u32::from(self.ysize),
            u32::from(bpp(self.itype)),
        );
        let off = if is_verbatim(self.itype) {
            match self.dim {
                1 => DATA,
                2 => DATA + u64::from(y.wrapping_mul(xs).wrapping_mul(b)),
                3 => {
                    DATA + u64::from(
                        y.wrapping_mul(xs)
                            .wrapping_add(z.wrapping_mul(xs).wrapping_mul(ys))
                            .wrapping_mul(b),
                    )
                }
                _ => return Err(errhdlr(err, "img_seek: weird dim\n")),
            }
        } else if is_rle(self.itype) {
            let rle = self.rle.as_ref().expect("the tables of a run-length image");
            let k = match self.dim {
                1 => 0,
                2 => y as usize,
                3 => (y + z * ys) as usize,
                _ => return Err(errhdlr(err, "img_seek: weird dim\n")),
            };
            u64::from(rle.rowstart[k])
        } else {
            return Err(errhdlr(err, "img_seek: weird image type\n"));
        };
        self.optseek(g, off);
        Ok(())
    }

    /// The row's index in the run-length tables (`img_getrowsize`).
    fn row_index(&self) -> usize {
        match self.dim {
            1 => 0,
            2 => usize::from(self.y),
            _ => usize::from(self.y) + usize::from(self.z) * usize::from(self.ysize),
        }
    }

    /// `iopen(file, "r", ...)`: the image, or `None` when the file cannot be opened (no message;
    /// `errno` set). A header that cannot be read whole, a wrong magic number and run-length tables
    /// that cannot be read end the program.
    pub fn open_read(
        g: &mut Globals,
        err: &mut dyn Write,
        dir: Option<&Path>,
        name: &str,
    ) -> Result<Option<RgbImage>, Fatal> {
        let mut f = match CFile::open(dir, name, "rb") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                return Ok(None);
            }
        };
        let mut h = [0u8; HEADER];
        if f.read_bytes(&mut h) != HEADER {
            return Err(errhdlr(err, "iopen: error on read of image header\n"));
        }
        let le = |o: usize| u16::from_le_bytes([h[o], h[o + 1]]);
        let mut v = [le(0), le(2), le(4), le(6), le(8), le(10)];
        let dorev = v[0].swap_bytes() == IMAGIC;
        if dorev {
            for x in &mut v {
                *x = x.swap_bytes();
            }
            g.notices.add(
                &FS_720,
                "reading an SGI image header in the swapped order: FreeSurfer converts 8 bytes past the header's memory",
            );
        }
        if v[0] != IMAGIC {
            return Err(errhdlr(
                err,
                &format!("iopen: bad magic in image file {:x}\n", v[0]),
            ));
        }
        let [_, itype, dim, xsize, ysize, zsize] = v;
        let mut im = RgbImage {
            itype,
            dim,
            xsize,
            ysize,
            zsize,
            min: 0,
            max: 0,
            flags: IOREAD,
            dorev,
            y: 0,
            z: 0,
            offset: 0,
            rle: None,
            tmpbuf: Vec::new(),
            filled: 0,
            f,
        };
        if is_rle(itype) {
            im.rle = Some(im.read_tables(g, err)?);
        }
        im.tmpbuf = vec![0; ibufsize(xsize)];
        im.offset = DATA as u32;
        let mut e = g.errno;
        if im.f.fseek(&mut e, DATA as i64, 0) != 0 {
            g.errno = e;
        }
        Ok(Some(im))
    }

    /// The run-length tables after the header. FreeSurfer reads two tables of `ysize * zsize`
    /// 8-byte entries (x86-64's `long`) from byte 512, the row starts as the 32-bit numbers at the
    /// first table's start; SGI's format has 32-bit entries in the file's order (FS-721).
    fn read_tables(&mut self, g: &mut Globals, err: &mut dyn Write) -> Result<Rle, Fatal> {
        let n = i32::from(self.ysize).wrapping_mul(i32::from(self.zsize));
        let fs_size = (n as i64 as u64).wrapping_mul(8) as u32 as i32;
        if fs_size < 0 {
            return Err(errhdlr(err, "iopen: error on table alloc\n"));
        }
        let entries = n.max(0) as usize;
        let mut e = g.errno;
        if self.f.fseek(&mut e, DATA as i64, 0) != 0 {
            g.errno = e;
        }
        let reproduce = g.notices.reproduce();
        if entries > 0 && self.dorev {
            g.notices.add(
                &FS_729,
                "a run-length SGI image in the swapped order: FreeSurfer converts its tables past their memory (memory corruption, a crash or a hang); SGI's tables read",
            );
        } else if entries > 0 {
            g.notices.add(
                &FS_721,
                if reproduce {
                    "a run-length SGI image: its row lengths read from the image data, as FreeSurfer reads them (tables of 8-byte entries)"
                } else {
                    "a run-length SGI image: SGI's tables read (FreeSurfer reads its row lengths from the image data)"
                },
            );
        }
        // the bytes each read asks for, and the entries' size
        let (ask, width) = if reproduce {
            (i64::from(fs_size), 8usize)
        } else {
            (entries as i64 * 4, 4usize)
        };
        let mut first = Vec::new();
        if self.read(&mut first, ask) != ask {
            return Err(errhdlr(err, "iopen: error on read of rowstart\n"));
        }
        let mut second = Vec::new();
        if self.read(&mut second, ask) != ask {
            return Err(errhdlr(err, "iopen: error on read of rowsize\n"));
        }
        let word = |b: &[u8], k: usize, swapped: bool| {
            let w = [b[4 * k], b[4 * k + 1], b[4 * k + 2], b[4 * k + 3]];
            if swapped {
                u32::from_be_bytes(w)
            } else {
                u32::from_le_bytes(w)
            }
        };
        let rle = if reproduce && !self.dorev {
            // FreeSurfer's: the starts from the first read, the lengths from the second (the
            // bytes after the first read's 8-byte entries)
            debug_assert_eq!(width, 8);
            Rle {
                rowstart: (0..entries).map(|k| word(&first, k, false)).collect(),
                rowsize: (0..entries)
                    .map(|k| word(&second, k, false) as i32)
                    .collect(),
            }
        } else if reproduce {
            // swapped: SGI's tables from the same bytes (FreeSurfer's conversion corrupts memory)
            let both: Vec<u8> = first.iter().chain(second.iter()).copied().collect();
            Rle {
                rowstart: (0..entries).map(|k| word(&both, k, true)).collect(),
                rowsize: (0..entries)
                    .map(|k| word(&both, entries + k, true) as i32)
                    .collect(),
            }
        } else {
            Rle {
                rowstart: (0..entries).map(|k| word(&first, k, self.dorev)).collect(),
                rowsize: (0..entries)
                    .map(|k| word(&second, k, self.dorev) as i32)
                    .collect(),
            }
        };
        Ok(rle)
    }

    /// `getrow(image, buffer, y, z)`: row `y` of plane `z` into `buffer` (`xsize` 16-bit values;
    /// `filled[i]` set for each value written), its count; -1 for a run-length row of length -1
    /// (nothing read). An image of fewer than three dimensions reads plane 0 (of fewer than two,
    /// row 0).
    pub fn getrow(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        buffer: &mut [u16],
        filled: &mut [bool],
        y: u32,
        z: u32,
    ) -> Result<i32, Fatal> {
        if self.flags & (IORW | IOREAD) == 0 {
            return Ok(-1);
        }
        let z = if self.dim < 3 { 0 } else { z };
        let y = if self.dim < 2 { 0 } else { y };
        self.seek(g, err, y, z)?;
        let xs = usize::from(self.xsize);
        let reproduce = g.notices.reproduce();
        if is_verbatim(self.itype) {
            match bpp(self.itype) {
                1 => {
                    let mut b = Vec::new();
                    if self.read(&mut b, xs as i64) != xs as i64 {
                        return Err(errhdlr(err, "getrow: error on read of row\n"));
                    }
                    self.tmpbuf[..xs].copy_from_slice(&b);
                    self.filled = self.filled.max(xs);
                    for i in 0..xs {
                        buffer[i] = u16::from(b[i]);
                        filled[i] = true;
                    }
                    Ok(i32::from(self.xsize))
                }
                2 => {
                    let mut cnt = 2 * xs as i64;
                    if xs >= 16384 {
                        let short = i64::from((2 * xs) as u16 as i16);
                        if short < 0 {
                            g.notices.add(
                                &FS_723,
                                format!(
                                    "an SGI row of {xs} 2-byte pixels: FreeSurfer reads it with the byte count {short} (the rest of the file) and fails"
                                ),
                            );
                        } else {
                            g.notices.add(
                                &FS_728,
                                format!(
                                    "an SGI row of {xs} 2-byte pixels: FreeSurfer reads only {short} bytes of it"
                                ),
                            );
                        }
                        if reproduce {
                            cnt = short;
                        }
                    }
                    let mut b = Vec::new();
                    let got = self.read(&mut b, cnt);
                    if got != cnt {
                        if b.len() > 2 * xs {
                            g.notices.add(
                                &FS_724,
                                "an SGI row read past its buffer (the rest of the file is longer than the row)",
                            );
                        }
                        return Err(errhdlr(err, "getrow: error on read of row\n"));
                    }
                    for i in 0..b.len() / 2 {
                        let v = u16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
                        buffer[i] = if self.dorev { v.swap_bytes() } else { v };
                        filled[i] = true;
                    }
                    Ok(i32::from(self.xsize))
                }
                _ => Err(errhdlr(err, "getrow: weird bpp\n")),
            }
        } else if is_rle(self.itype) {
            let ibpp = bpp(self.itype);
            if ibpp != 1 && ibpp != 2 {
                return Err(errhdlr(err, "getrow: weird bpp\n"));
            }
            let k = self.row_index();
            let len = self.rle.as_ref().expect("tables").rowsize[k];
            let cnt = if reproduce {
                i64::from(len as i16)
            } else {
                i64::from(len)
            };
            if cnt == -1 {
                return Ok(-1);
            }
            let mut b = Vec::new();
            let got = self.read(&mut b, cnt);
            if b.len() > self.tmpbuf.len() {
                g.notices.add(
                    &FS_726,
                    format!(
                        "a run-length SGI row of {} bytes read into a buffer of {}",
                        b.len(),
                        self.tmpbuf.len()
                    ),
                );
            }
            if got != cnt {
                return Err(errhdlr(err, "getrow: error on read of row\n"));
            }
            let keep = b.len().min(self.tmpbuf.len());
            self.tmpbuf[..keep].copy_from_slice(&b[..keep]);
            self.filled = self.filled.max(keep);
            if ibpp == 2 && self.dorev {
                // cvtshorts(tmpbuf, cnt)
                for i in 0..keep / 2 {
                    self.tmpbuf.swap(2 * i, 2 * i + 1);
                }
            }
            self.expand(g, ibpp, buffer, filled);
            Ok(i32::from(self.xsize))
        } else {
            Err(errhdlr(err, "getrow: weird image type\n"))
        }
    }

    /// `img_rle_expand(tmpbuf, ibpp, buffer, 2)`: runs of copied values (a count with the high
    /// bit) and of a repeated value (a count without it) until a count of 0. FreeSurfer bounds
    /// neither side (FS-726): both modes stop at the buffer's end and drop the values past the
    /// row's width; bytes the buffer never held are memory contents (FS-727: read as 0).
    fn expand(&mut self, g: &mut Globals, ibpp: u16, buffer: &mut [u16], filled: &mut [bool]) {
        let unit = usize::from(ibpp);
        let units = self.tmpbuf.len() / unit;
        let (mut at, mut out) = (0usize, 0usize);
        let (mut past_buf, mut unread, mut dropped) = (false, false, false);
        loop {
            let mut take = || {
                if at >= units {
                    past_buf = true;
                    return None;
                }
                if (at + 1) * unit > self.filled {
                    unread = true;
                }
                let v = if unit == 1 {
                    u16::from(self.tmpbuf[at])
                } else {
                    u16::from_le_bytes([self.tmpbuf[2 * at], self.tmpbuf[2 * at + 1]])
                };
                at += 1;
                Some(v)
            };
            let Some(pixel) = take() else {
                break;
            };
            let count = pixel & 0x7f;
            if count == 0 {
                break;
            }
            let mut vals = Vec::with_capacity(usize::from(count));
            if pixel & 0x80 != 0 {
                for _ in 0..count {
                    match take() {
                        Some(v) => vals.push(v),
                        None => break,
                    }
                }
            } else if let Some(v) = take() {
                vals.resize(usize::from(count), v);
            }
            for v in vals {
                if out < buffer.len() {
                    buffer[out] = v;
                    filled[out] = true;
                } else {
                    dropped = true;
                }
                out += 1;
            }
            if past_buf {
                break;
            }
        }
        if past_buf || dropped {
            g.notices.add(
                &FS_726,
                "a run-length SGI row expanded past the library's buffers (FreeSurfer reads or writes past them)",
            );
        }
        if unread {
            g.notices.add(
                &FS_727,
                "a run-length SGI row expanded from bytes no read gave (FreeSurfer: memory contents; read as 0)",
            );
        }
    }

    /// `iclose(image)` of an image read: nothing is written.
    pub fn close_read(self) {
        let _ = self.f.close();
    }

    /// `iopen(file, "w", type, dim, xsize, ysize, zsize)`: the file created (`creat`, mode 0666),
    /// the header written. A file that cannot be created ends the program (`iopen: can't open
    /// output file (null)`, FS-725).
    #[allow(clippy::too_many_arguments)]
    pub fn open_write(
        g: &mut Globals,
        err: &mut dyn Write,
        dir: Option<&Path>,
        name: &str,
        itype: u16,
        dim: u32,
        xsize: u32,
        ysize: u32,
        zsize: u32,
    ) -> Result<RgbImage, Fatal> {
        let f = match CFile::open(dir, name, "wb") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                g.notices.add(
                    &FS_725,
                    "the SGI image writer's message names the file it cannot create (null)",
                );
                let shown = if g.notices.reproduce() {
                    "(null)"
                } else {
                    name
                };
                return Err(errhdlr(
                    err,
                    &format!("iopen: can't open output file {shown}\n"),
                ));
            }
        };
        let xs = xsize as u16;
        let ys = if dim > 1 { ysize as u16 } else { 1 };
        let zs = if dim > 2 { zsize as u16 } else { 1 };
        let d = if zs == 1 {
            if ys == 1 { 1 } else { 2 }
        } else {
            3
        };
        let mut im = RgbImage {
            itype,
            dim: d,
            xsize: xs,
            ysize: ys,
            zsize: zs,
            min: 10_000_000,
            max: 0,
            flags: 0,
            dorev: false,
            y: 0,
            z: 0,
            offset: 0,
            rle: None,
            tmpbuf: Vec::new(),
            filled: 0,
            f,
        };
        let h = im.header(false);
        if !im.write(&h) {
            return Err(errhdlr(err, "iopen: error on write of image header\n"));
        }
        im.flags = IOWRT;
        im.tmpbuf = vec![0; ibufsize(xs)];
        im.offset = DATA as u32;
        let mut e = g.errno;
        if im.f.fseek(&mut e, DATA as i64, 0) != 0 {
            g.errno = e;
        }
        Ok(im)
    }

    /// The header's 176 bytes as the writer writes them (`swapImage` applied). `closing`: the
    /// header `iclose` writes (the descriptor, the flags, the last row, a buffer's address set),
    /// else `iopen`'s. The descriptor and the address are written as 0 (FS-722).
    fn header(&self, closing: bool) -> [u8; HEADER] {
        let mut h = [0u8; HEADER];
        let be16 =
            |h: &mut [u8; HEADER], o: usize, v: u16| h[o..o + 2].copy_from_slice(&v.to_be_bytes());
        be16(&mut h, 0, IMAGIC);
        be16(&mut h, 2, self.itype);
        be16(&mut h, 4, self.dim);
        be16(&mut h, 6, self.xsize);
        be16(&mut h, 8, self.ysize);
        be16(&mut h, 10, self.zsize);
        h[12..16].copy_from_slice(&self.min.to_be_bytes());
        h[16..20].copy_from_slice(&self.max.to_be_bytes());
        // wastebytes (0) as 16 bits: 0
        h[24..31].copy_from_slice(b"no name");
        // colormap 0, file (FS-722: 0)
        if closing {
            h[112..114].copy_from_slice(&self.flags.to_le_bytes());
            h[118..120].copy_from_slice(&self.y.to_le_bytes());
            h[120..122].copy_from_slice(&self.z.to_le_bytes());
        }
        h
    }

    /// `putrow(image, buffer, y, z)` of an uncompressed image of 1-byte pixels (the only kind
    /// FreeSurfer writes): each value's low byte, the image's minimum and maximum updated, the row
    /// written where it belongs. A row outside the image ends the program.
    pub fn putrow(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        buffer: &[u16],
        y: u32,
        z: u32,
    ) -> Result<i32, Fatal> {
        if self.flags & (IORW | IOWRT) == 0 {
            return Ok(-1);
        }
        let z = if self.dim < 3 { 0 } else { z };
        let y = if self.dim < 2 { 0 } else { y };
        debug_assert!(is_verbatim(self.itype) && bpp(self.itype) == 1);
        let xs = usize::from(self.xsize);
        let (mut min, mut max) = (u64::from(self.min), u64::from(self.max));
        for (t, &v) in self.tmpbuf.iter_mut().zip(buffer).take(xs) {
            let c = v as u8;
            *t = c;
            max = max.max(u64::from(c));
            min = min.min(u64::from(c));
        }
        self.min = min as u32;
        self.max = max as u32;
        self.seek(g, err, y, z)?;
        let row = self.tmpbuf[..xs].to_vec();
        if !self.write(&row) {
            return Err(errhdlr(err, "putrow: error on write of row\n"));
        }
        Ok(i32::from(self.xsize))
    }

    /// `iclose(image)` of an image written: the final header at the file's start (FS-722).
    pub fn close_write(mut self, g: &mut Globals, err: &mut dyn Write) -> Result<i32, Fatal> {
        self.optseek(g, 0);
        g.notices.add(
            &FS_722,
            "an SGI image written: FreeSurfer's header holds its file descriptor and a buffer's address (written as 0)",
        );
        self.offset = 0;
        let h = self.header(true);
        if !self.write(&h) {
            return Err(errhdlr(err, "iclose: error on write of image header\n"));
        }
        if self.f.close() != 0 {
            return Err(errhdlr(err, "iclose: error on close of file\n"));
        }
        Ok(0)
    }
}
