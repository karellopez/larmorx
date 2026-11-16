// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! TIFF files read and written as libtiff 3.6.1 (Sam Leffler, Silicon Graphics; 2003-12-26)
//! reads and writes them, in FreeSurfer 8.2.0's build of it (its `packages/tiff`, libtiff 3.6.1
//! but two comment lines and a cast in printing: `PIXARLOG_SUPPORT` and `ZIP_SUPPORT` on, the
//! CCITT, PackBits, LZW, ThunderScan, NeXT and LogLuv codecs on, JPEG, old JPEG and JBIG not
//! configured, files read without memory mapping) over zlib 1.2.11
//! ([`larmorx_core::zlib`]): a Rust translation of the library's directory reading and writing,
//! its strip and tile reading, its scanline writing and the codecs below.
//!
//! Copyright (c) 1988-1997 Sam Leffler, Copyright (c) 1991-1997 Silicon Graphics, Inc. libtiff's
//! notice and its no-warranty disclaimer are `LICENSES/libtiff.txt`. The changes to the original
//! files: the C library's modules are translated into Rust (`tif_open.c`, `tif_unix.c`,
//! `tif_dir.c`, `tif_dirinfo.c`, `tif_dirread.c`, `tif_aux.c`, `tif_read.c`, `tif_strip.c`,
//! `tif_tile.c`, `tif_swab.c`, `tif_compress.c`, `tif_codec.c`, `tif_dumpmode.c`,
//! `tif_packbits.c`, `tif_lzw.c`, `tif_zip.c`, `tif_predict.c`, `tif_close.c`, `tif_write.c`,
//! `tif_dirwrite.c`, `tif_flush.c`, `tif_fax3.c` with `mkg3states.c`, `tif_next.c`,
//! `tif_thunder.c`, `tif_pixarlog.c`, `tif_luv.c`); the method tables become enums; what no caller of this port
//! can reach (memory-mapped files, client procedures, tag extenders, `TIFFPrintDirectory`, the
//! RGBA image interface) is left out; corrections of the defects found are on by default
//! (FS-814 onwards: `docs/findings/libtiff-3.6.1.md`). `PROVENANCE.md` lists the files.
//!
//! The functions follow the C library's (`TIFFOpen`, `TIFFNumberOfDirectories`,
//! `TIFFSetDirectory`, `TIFFReadDirectory`, `TIFFGetField`, `TIFFGetFieldDefaulted`,
//! `TIFFScanlineSize`, `TIFFTileSize`, `TIFFReadScanline`, `TIFFReadTile`, `TIFFSetField`,
//! `TIFFWriteScanline`, `TIFFWriteDirectory`, `TIFFClose`): each writes libtiff's messages to
//! standard error through [`larmorx_core::libtiff`]'s handlers and returns what the C function
//! returns.
//!
//! The codecs: None, PackBits, LZW, Deflate (with the predictor), the CCITT decoders (RLE,
//! RLE/W, Group 3, Group 4: `fax3.rs`), NeXT, ThunderScan, PixarLog and SGILog (`next.rs`,
//! `thunder.rs`, `pixarlog.rs`, `luv.rs`); their encoders but Deflate's, PackBits' and None's
//! are left out (no caller of this port writes them). JPEG, old JPEG and JBIG are not configured
//! in FreeSurfer's build (libtiff's own error). [`Unported`] stays in the signatures for the
//! callers; no file gives it now.
//!
//! The behaviour is proved by the differential probe `validation/probes/fs-tiff` against
//! libtiff 3.6.1 built from its release and FreeSurfer 8.2.0's copy of it.
// The translation keeps libtiff's control flow where clippy prefers other forms: indexed loops
// over the directory entries it changes, its checks before divisions, chunked byte swaps.
#![allow(
    clippy::needless_range_loop,
    clippy::manual_checked_ops,
    clippy::chunks_exact_to_as_chunks
)]

#[doc(hidden)]
pub mod cases;
mod codec;
#[doc(hidden)]
pub mod codecgen;
mod dir;
mod dirread;
pub mod fax3;
pub mod fields;
mod io;
mod luv;
mod next;
mod pixarlog;
mod read;
#[cfg(test)]
mod tests;
mod thunder;
mod write;

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_core::libtiff::TiffMessages;

pub use dir::{SetValue, Value};
pub use fields::tag;

use codec::Codec;
use dir::Dir;
use fields::Fields;
use io::{Fd, Whence};

/// libtiff's version string (`TIFFGetVersion`).
pub const TIFFLIB_VERSION_STR: &str = "LIBTIFF, Version 3.6.1\nCopyright (c) 1988-1996 Sam Leffler\nCopyright (c) 1991-1996 Silicon Graphics, Inc.";

/// Compression schemes (`COMPRESSION_*`).
pub mod compression {
    pub const NONE: u16 = 1;
    pub const CCITTRLE: u16 = 2;
    pub const CCITTFAX3: u16 = 3;
    pub const CCITTFAX4: u16 = 4;
    pub const LZW: u16 = 5;
    pub const OJPEG: u16 = 6;
    pub const JPEG: u16 = 7;
    pub const ADOBE_DEFLATE: u16 = 8;
    pub const NEXT: u16 = 32766;
    pub const CCITTRLEW: u16 = 32771;
    pub const PACKBITS: u16 = 32773;
    pub const THUNDERSCAN: u16 = 32809;
    pub const PIXARLOG: u16 = 32909;
    pub const DEFLATE: u16 = 32946;
    pub const JBIG: u16 = 34661;
    pub const SGILOG: u16 = 34676;
    pub const SGILOG24: u16 = 34677;
}

/// Other tag values (`tiff.h`).
#[allow(missing_docs)]
pub mod values {
    pub const FILLORDER_MSB2LSB: u16 = 1;
    pub const FILLORDER_LSB2MSB: u16 = 2;
    pub const ORIENTATION_TOPLEFT: u16 = 1;
    pub const ORIENTATION_BOTLEFT: u16 = 4;
    pub const ORIENTATION_LEFTBOT: u16 = 8;
    pub const PLANARCONFIG_CONTIG: u16 = 1;
    pub const PLANARCONFIG_SEPARATE: u16 = 2;
    pub const PHOTOMETRIC_MINISWHITE: u16 = 0;
    pub const PHOTOMETRIC_MINISBLACK: u16 = 1;
    pub const PHOTOMETRIC_RGB: u16 = 2;
    pub const PHOTOMETRIC_PALETTE: u16 = 3;
    pub const PHOTOMETRIC_YCBCR: u16 = 6;
    pub const RESUNIT_NONE: u16 = 1;
    pub const RESUNIT_INCH: u16 = 2;
    pub const RESUNIT_CENTIMETER: u16 = 3;
    pub const SAMPLEFORMAT_UINT: u16 = 1;
    pub const SAMPLEFORMAT_INT: u16 = 2;
    pub const SAMPLEFORMAT_IEEEFP: u16 = 3;
    pub const SAMPLEFORMAT_VOID: u16 = 4;
    pub const SAMPLEFORMAT_COMPLEXINT: u16 = 5;
    pub const SAMPLEFORMAT_COMPLEXIEEEFP: u16 = 6;
    pub const EXTRASAMPLE_ASSOCALPHA: u16 = 1;
    pub const EXTRASAMPLE_UNASSALPHA: u16 = 2;
    pub const THRESHHOLD_BILEVEL: u16 = 1;
    pub const YCBCRPOSITION_CENTERED: u16 = 1;
    pub const INKSET_CMYK: u16 = 1;
    pub const OFILETYPE_REDUCEDIMAGE: u32 = 2;
    pub const OFILETYPE_PAGE: u32 = 3;
    pub const FILETYPE_REDUCEDIMAGE: u32 = 1;
    pub const FILETYPE_PAGE: u32 = 2;
}

// tif_flags (TIFF_DIRTYHEADER, 0x0004, and TIFF_MAPPED, 0x0800, are never set here).
pub(crate) const TIFF_FILLORDER: u32 = 0x0003;
pub(crate) const TIFF_DIRTYDIRECT: u32 = 0x0008;
pub(crate) const TIFF_BUFFERSETUP: u32 = 0x0010;
pub(crate) const TIFF_CODERSETUP: u32 = 0x0020;
pub(crate) const TIFF_BEENWRITING: u32 = 0x0040;
pub(crate) const TIFF_SWAB: u32 = 0x0080;
pub(crate) const TIFF_NOBITREV: u32 = 0x0100;
pub(crate) const TIFF_MYBUFFER: u32 = 0x0200;
pub(crate) const TIFF_ISTILED: u32 = 0x0400;
pub(crate) const TIFF_POSTENCODE: u32 = 0x1000;
pub(crate) const TIFF_INSUBIFD: u32 = 0x2000;
pub(crate) const TIFF_UPSAMPLED: u32 = 0x4000;
pub(crate) const TIFF_STRIPCHOP: u32 = 0x8000;

pub(crate) const TIFF_BIGENDIAN: u16 = 0x4d4d;
pub(crate) const TIFF_LITTLEENDIAN: u16 = 0x4949;
pub(crate) const TIFF_VERSION: u16 = 42;

/// The open mode (`tif_mode`, after `O_CREAT` and `O_TRUNC` are taken away).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    RdOnly,
    RdWr,
}

/// The port's settings.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// D13's switch (FS-814 onwards).
    pub upstream_defects: UpstreamDefects,
    /// The largest single allocation the caller's machine model grants (`malloc` fails past
    /// it: libtiff's "No space ..." messages). `None`: every allocation libtiff's 32-bit sizes
    /// can ask for (at most 2 GiB) succeeds.
    pub max_alloc: Option<u64>,
    /// libtiff's library-wide message handlers.
    pub messages: TiffMessages,
}

/// A codec this port does not decode yet ([`Tiff`]'s functions return it where the original
/// would start using that codec).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unported(pub &'static str);

impl std::fmt::Display for Unported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "libtiff's {} codec", self.0)
    }
}

/// The file's header (`TIFFHeader`).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Header {
    pub magic: u16,
    pub version: u16,
    pub diroff: u32,
}

/// An open TIFF file (`TIFF`).
pub struct Tiff {
    pub(crate) name: String,
    pub(crate) fd: Fd,
    pub(crate) mode: Mode,
    pub(crate) flags: u32,
    pub(crate) diroff: u32,
    pub(crate) nextdiroff: u32,
    pub(crate) dirlist: Vec<u32>,
    pub(crate) dirnumber: u16,
    pub(crate) dir: Dir,
    pub(crate) header: Header,
    pub(crate) row: u32,
    pub(crate) curdir: u16,
    pub(crate) curstrip: u32,
    pub(crate) curoff: u32,
    pub(crate) dataoff: u32,
    pub(crate) nsubifd: u16,
    pub(crate) subifdoff: u32,
    pub(crate) col: u32,
    pub(crate) curtile: u32,
    pub(crate) tilesize: i32,
    pub(crate) codec: Codec,
    pub(crate) scanlinesize: i32,
    /// The raw data buffer (`tif_rawdata`; its length is `tif_rawdatasize`), the position in it
    /// (`tif_rawcp`) and the bytes left (`tif_rawcc`).
    pub(crate) rawdata: Vec<u8>,
    pub(crate) rawcp: usize,
    pub(crate) rawcc: i32,
    /// The bytes of the raw buffer written by reads since it was allocated (the rest is
    /// memory never written: FS-819).
    pub(crate) raw_defined: usize,
    /// A decoder read past the raw buffer's defined bytes (noted after the call).
    pub(crate) past_raw: std::cell::Cell<bool>,
    pub(crate) fields: Fields,
    pub(crate) opts: Options,
    pub(crate) notices: Notices,
    pub(crate) unported: Option<Unported>,
}

thread_local! {
    static CLOSED_NOTICES: std::cell::RefCell<Vec<larmorx_core::defects::Notice>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The notices of the handles closed or dropped on this thread since the last call (a
/// [`Tiff::open`] that fails, a [`Tiff::close`]), as libtiff's messages they go with were
/// written.
pub fn take_closed_notices() -> Vec<larmorx_core::defects::Notice> {
    CLOSED_NOTICES.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

impl Drop for Tiff {
    fn drop(&mut self) {
        let list = std::mem::replace(&mut self.notices, Notices::new(self.opts.upstream_defects));
        if !list.is_empty() {
            CLOSED_NOTICES.with(|c| c.borrow_mut().extend(list.as_slice().iter().cloned()));
        }
    }
}

/// `%.1000s` and `%.100s`: the name cut to `n` bytes (at a character boundary).
pub(crate) fn cut(s: &str, n: usize) -> &str {
    if s.len() <= n {
        return s;
    }
    let mut k = n;
    while !s.is_char_boundary(k) {
        k -= 1;
    }
    &s[..k]
}

impl Tiff {
    pub(crate) fn error(&self, module: Option<&str>, msg: &str, s: &mut dyn Streams) {
        self.opts.messages.error(module, msg, s);
    }

    pub(crate) fn warning(&self, module: Option<&str>, msg: &str, s: &mut dyn Streams) {
        self.opts.messages.warning(module, msg, s);
    }

    pub(crate) fn fix(&self) -> bool {
        self.opts.upstream_defects.fix()
    }

    /// `_TIFFmalloc(n)` as `CheckMalloc` and its friends see it: whether `n` bytes (a `tsize_t`
    /// or a `size_t`) are granted.
    pub(crate) fn can_alloc(&self, n: u64) -> bool {
        self.opts.max_alloc.is_none_or(|m| n <= m)
    }

    pub(crate) fn swab(&self) -> bool {
        self.flags & TIFF_SWAB != 0
    }

    pub(crate) fn is_tiled(&self) -> bool {
        self.flags & TIFF_ISTILED != 0
    }

    pub(crate) fn big_endian_file(&self) -> bool {
        self.header.magic == TIFF_BIGENDIAN
    }

    /// `ReadOK(tif, buf, size)`: the whole of `buf[..size]` read.
    pub(crate) fn read_ok(&mut self, buf: &mut [u8], size: i32) -> bool {
        self.fd.read(buf, size) == size
    }

    /// `SeekOK(tif, off)`.
    pub(crate) fn seek_ok(&mut self, off: u32) -> bool {
        self.fd.seek(off, Whence::Set) == off
    }

    /// `TIFFOpen(name, mode)`: the open file with its first directory read ("r"), or a new
    /// file with its header written ("w", "a"); `Ok(None)` when the C function returns NULL,
    /// its messages written.
    pub fn open(
        name: &str,
        mode: &str,
        opts: Options,
        s: &mut dyn Streams,
    ) -> Result<Option<Tiff>, Unported> {
        const MODULE: &str = "TIFFOpen";
        let Some(m) = get_mode(mode, MODULE, &opts.messages, s) else {
            return Ok(None);
        };
        let Some(fd) = Fd::open(name, m.0 != Mode::RdOnly, m.2) else {
            opts.messages
                .error(Some(MODULE), &format!("{name}: Cannot open"), s);
            return Ok(None);
        };
        Tiff::client_open(name, mode, fd, opts, s)
    }

    /// `TIFFClientOpen` with the Unix procedures.
    fn client_open(
        name: &str,
        mode: &str,
        fd: Fd,
        opts: Options,
        s: &mut dyn Streams,
    ) -> Result<Option<Tiff>, Unported> {
        const MODULE: &str = "TIFFClientOpen";
        let Some((m, create, _)) = get_mode(mode, MODULE, &opts.messages, s) else {
            return Ok(None);
        };
        let mut tif = Tiff {
            name: name.to_owned(),
            fd,
            mode: m,
            flags: 0,
            diroff: 0,
            nextdiroff: 0,
            dirlist: Vec::new(),
            dirnumber: 0,
            dir: Dir::default(),
            header: Header::default(),
            row: u32::MAX,
            curdir: u16::MAX,
            curstrip: u32::MAX,
            curoff: 0,
            dataoff: 0,
            nsubifd: 0,
            subifdoff: 0,
            col: 0,
            curtile: 0,
            tilesize: 0,
            codec: Codec::default(),
            scanlinesize: 0,
            rawdata: Vec::new(),
            rawcp: 0,
            rawcc: 0,
            raw_defined: 0,
            past_raw: std::cell::Cell::new(false),
            fields: Fields::new(fields::next_serial()),
            opts,
            notices: Notices::new(opts.upstream_defects),
            unported: None,
        };
        tif.set_default_compression_state();
        tif.flags = u32::from(values::FILLORDER_MSB2LSB);
        // TIFF_MAPPED is set for "r" and cleared again below: the build maps nothing.
        // STRIPCHOP_DEFAULT for O_RDONLY and O_RDWR ("r", "r+"; not "w" or "a").
        if m == Mode::RdOnly || !create {
            tif.flags |= TIFF_STRIPCHOP;
        }
        // The machine model is x86-64: little-endian.
        let bigendian = false;
        for c in mode.bytes() {
            match c {
                b'b' if create && !bigendian => tif.flags |= TIFF_SWAB,
                b'l' if create && bigendian => tif.flags |= TIFF_SWAB,
                b'B' => {
                    tif.flags = (tif.flags & !TIFF_FILLORDER) | u32::from(values::FILLORDER_MSB2LSB)
                }
                b'L' => {
                    tif.flags = (tif.flags & !TIFF_FILLORDER) | u32::from(values::FILLORDER_LSB2MSB)
                }
                b'H' => {
                    // HOST_FILLORDER of FreeSurfer's port.h: FILLORDER_MSB2LSB.
                    tif.flags = (tif.flags & !TIFF_FILLORDER) | u32::from(values::FILLORDER_MSB2LSB)
                }
                b'C' if m == Mode::RdOnly => tif.flags |= TIFF_STRIPCHOP,
                b'c' if m == Mode::RdOnly => tif.flags &= !TIFF_STRIPCHOP,
                _ => {}
            }
        }
        // Read in the header.
        let mut hb = [0u8; 8];
        if !tif.read_ok(&mut hb, 8) {
            if tif.mode == Mode::RdOnly {
                tif.error(Some(name), "Cannot read TIFF header", s);
                tif.mode = Mode::RdOnly;
                tif.close_inner(s);
                return Ok(None);
            }
            let swab = tif.swab();
            let magic = if swab {
                if bigendian {
                    TIFF_LITTLEENDIAN
                } else {
                    TIFF_BIGENDIAN
                }
            } else if bigendian {
                TIFF_BIGENDIAN
            } else {
                TIFF_LITTLEENDIAN
            };
            tif.header = Header {
                magic,
                version: TIFF_VERSION,
                diroff: 0,
            };
            tif.fd.seek(0, Whence::Set);
            let hdr = tif.header_bytes();
            if tif.fd.write(&hdr, 8) != 8 {
                tif.error(Some(name), "Error writing TIFF header", s);
                tif.mode = Mode::RdOnly;
                tif.close_inner(s);
                return Ok(None);
            }
            tif.init_order(magic, bigendian);
            tif.default_directory(s)?;
            tif.diroff = 0;
            tif.dirlist.clear();
            tif.dirnumber = 0;
            return Ok(Some(tif));
        }
        // The header's fields as the machine reads them (little-endian on x86-64).
        let magic = u16::from_le_bytes([hb[0], hb[1]]);
        let mut version = u16::from_le_bytes([hb[2], hb[3]]);
        let mut diroff = u32::from_le_bytes([hb[4], hb[5], hb[6], hb[7]]);
        tif.header.magic = magic;
        if magic != TIFF_BIGENDIAN && magic != TIFF_LITTLEENDIAN {
            tif.error(
                Some(name),
                &format!("Not a TIFF file, bad magic number {magic} ({magic:#x})"),
                s,
            );
            tif.mode = Mode::RdOnly;
            tif.close_inner(s);
            return Ok(None);
        }
        tif.init_order(magic, bigendian);
        if tif.swab() {
            version = version.swap_bytes();
            diroff = diroff.swap_bytes();
        }
        tif.header.version = version;
        tif.header.diroff = diroff;
        if version != TIFF_VERSION {
            tif.error(
                Some(name),
                &format!("Not a TIFF file, bad version number {version} ({version:#x})"),
                s,
            );
            tif.mode = Mode::RdOnly;
            tif.close_inner(s);
            return Ok(None);
        }
        tif.flags |= TIFF_MYBUFFER;
        tif.rawcp = 0;
        tif.rawdata = Vec::new();
        match mode.as_bytes()[0] {
            b'r' => {
                tif.nextdiroff = tif.header.diroff;
                if tif.read_directory(s)? {
                    tif.rawcc = -1;
                    tif.flags |= TIFF_BUFFERSETUP;
                    return Ok(Some(tif));
                }
            }
            b'a' => {
                tif.default_directory(s)?;
                return Ok(Some(tif));
            }
            _ => {}
        }
        tif.mode = Mode::RdOnly;
        tif.close_inner(s);
        Ok(None)
    }

    /// The header's eight bytes in the file's order.
    pub(crate) fn header_bytes(&self) -> [u8; 8] {
        let mut h = [0u8; 8];
        h[..2].copy_from_slice(&self.header.magic.to_le_bytes());
        let (v, d) = if self.swab() {
            (
                self.header.version.swap_bytes(),
                self.header.diroff.swap_bytes(),
            )
        } else {
            (self.header.version, self.header.diroff)
        };
        h[2..4].copy_from_slice(&v.to_le_bytes());
        h[4..].copy_from_slice(&d.to_le_bytes());
        h
    }

    /// `TIFFInitOrder`.
    fn init_order(&mut self, magic: u16, bigendian: bool) {
        if magic == TIFF_BIGENDIAN {
            if !bigendian {
                self.flags |= TIFF_SWAB;
            }
        } else if bigendian {
            self.flags |= TIFF_SWAB;
        }
    }

    /// `TIFFIsTiled(tif)`.
    pub fn tiled(&self) -> bool {
        self.is_tiled()
    }

    /// `TIFFIsByteSwapped(tif)`.
    pub fn is_byte_swapped(&self) -> bool {
        self.swab()
    }

    /// `TIFFIsBigEndian` (libtiff 3.6.1 has no such function: the file's byte order).
    pub fn is_big_endian(&self) -> bool {
        self.big_endian_file()
    }

    /// `TIFFCurrentDirectory(tif)`.
    pub fn current_directory(&self) -> u16 {
        self.curdir
    }

    /// `TIFFFileName(tif)`.
    pub fn file_name(&self) -> &str {
        &self.name
    }

    /// The notices of the corrected (or reproduced) defects so far.
    pub fn notices(&self) -> &Notices {
        &self.notices
    }

    /// `TIFFAdvanceDirectory` without the mapped path: the next directory's offset.
    fn advance_directory(&mut self, nextdir: &mut u32, s: &mut dyn Streams) -> bool {
        const MODULE: &str = "TIFFAdvanceDirectory";
        let mut c = [0u8; 2];
        if !self.seek_ok(*nextdir) || !self.read_ok(&mut c, 2) {
            let m = format!("{}: Error fetching directory count", self.name);
            self.error(Some(MODULE), &m, s);
            return false;
        }
        let mut dircount = u16::from_le_bytes(c);
        if self.swab() {
            dircount = dircount.swap_bytes();
        }
        self.fd
            .seek(u32::from(dircount).wrapping_mul(12), Whence::Cur);
        let mut l = [0u8; 4];
        if !self.read_ok(&mut l, 4) {
            let m = format!("{}: Error fetching directory link", self.name);
            self.error(Some(MODULE), &m, s);
            return false;
        }
        let mut v = u32::from_le_bytes(l);
        if self.swab() {
            v = v.swap_bytes();
        }
        *nextdir = v;
        true
    }

    /// `TIFFNumberOfDirectories(tif)`. libtiff 3.6.1 follows the chain until a link is 0 or
    /// cannot be read and never returns on a chain that loops; corrected (FS-814), the count
    /// stops at the first directory seen again.
    pub fn number_of_directories(&mut self, s: &mut dyn Streams) -> u16 {
        let mut nextdir = self.header.diroff;
        let mut n: u16 = 0;
        let mut seen: Vec<u32> = Vec::new();
        while nextdir != 0 {
            if seen.contains(&nextdir) {
                self.notices.add(
                    &larmorx_core::defects::FS_814,
                    format!(
                        "the directory chain of {} loops back to offset {nextdir}: counted {} \
                         directories (libtiff 3.6.1 never returns)",
                        self.name,
                        seen.len()
                    ),
                );
                return seen.len() as u16;
            }
            seen.push(nextdir);
            if !self.advance_directory(&mut nextdir, s) {
                break;
            }
            n = n.wrapping_add(1);
        }
        n
    }

    /// `TIFFSetDirectory(tif, dirn)`.
    pub fn set_directory(&mut self, dirn: u16, s: &mut dyn Streams) -> Result<bool, Unported> {
        let mut nextdir = self.header.diroff;
        let mut n = dirn;
        while n > 0 && nextdir != 0 {
            if !self.advance_directory(&mut nextdir, s) {
                return Ok(false);
            }
            n -= 1;
        }
        self.nextdiroff = nextdir;
        self.curdir = dirn.wrapping_sub(n).wrapping_sub(1);
        self.dirnumber = 0;
        self.read_directory(s)
    }

    /// `TIFFClose(tif)` (a file open for writing is flushed first: [`Tiff::flush`]).
    pub fn close(mut self, s: &mut dyn Streams) {
        if self.mode != Mode::RdOnly {
            self.flush(s);
        }
        self.close_inner(s);
    }

    /// `TIFFClose` after the flush: the codec cleaned up, the directory and fields freed.
    fn close_inner(&mut self, _s: &mut dyn Streams) {
        self.cleanup();
        self.free_directory();
        self.dirlist.clear();
        self.rawdata = Vec::new();
        self.fields.free_anon();
    }
}

/// `_TIFFgetMode(mode, module)`: the mode, whether the file is created, whether truncated.
fn get_mode(
    mode: &str,
    module: &str,
    msgs: &TiffMessages,
    s: &mut dyn Streams,
) -> Option<(Mode, bool, bool)> {
    match mode.as_bytes().first() {
        Some(b'r') => Some((
            if mode.as_bytes().get(1) == Some(&b'+') {
                Mode::RdWr
            } else {
                Mode::RdOnly
            },
            false,
            false,
        )),
        Some(b'w') => Some((Mode::RdWr, true, true)),
        Some(b'a') => Some((Mode::RdWr, true, false)),
        _ => {
            msgs.error(Some(module), &format!("\"{mode}\": Bad mode"), s);
            None
        }
    }
}
