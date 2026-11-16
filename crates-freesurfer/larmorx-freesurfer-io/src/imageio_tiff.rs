// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's TIFF images (`utils/imageio.cpp`): `TiffReadImage` with `TiffReadDirectoryInfo`,
//! `TiffReadImageDirectory`, `TiffReadStrippedImage` and `TiffReadTiledImage`, and
//! `TiffWriteImage`, over track C's libtiff 3.6.1 (`larmorx_io::tiff`, the main package's: its
//! messages, its notices FS-814 onwards).
//!
//! A TIFF file is read page by page (a TIFF directory is a page): the pages from `frame0` to
//! `end_frame` (inclusive; every page from `frame0` on when `end_frame` is below 0) become the
//! image's frames, of the size, samples and bits of the first page read. Grey pages of 8, 16, 32
//! or 64 bits give images of bytes, shorts, floats or doubles (any other number of bits: bytes);
//! 1-bit pages are unpacked to bytes of 0 and 1; 3-sample pages give RGB images, 4-sample pages
//! RGB images without the fourth sample; other sample counts end the program. The rows are
//! stored bottom row first (FreeSurfer's image rows are upside down) for the top-left
//! orientation, as they come for the bottom-left one; other orientations end the program
//! (stripped pages; tiled pages are read as top-left).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-io.md` Part 35):
//! FS-863 to FS-874.

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{
    FS_863, FS_864, FS_865, FS_866, FS_867, FS_868, FS_869, FS_870, FS_871, FS_872, FS_873, FS_874,
};
use larmorx_core::libtiff::TiffMessages;
use larmorx_freesurfer_core::alloc::MACHINE_BYTES;
use larmorx_freesurfer_core::diag::verbose_on;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NOFILE, ERROR_UNSUPPORTED, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::matrix_ops::fzero;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils;
use larmorx_io::tiff::{self, Options, SetValue, Tiff, Unported, Value, tag, values};

use crate::image::{Image, PFBYTE, PFDOUBLE, PFFLOAT, PFINT, PFRGB, PFSHORT, image_alloc};
use crate::minc::Std;

use super::{Read, Wrote};

/// The address `%p` prints for the image's pixels in `FS_TIFF_DEBUG`'s lines (FreeSurfer's are
/// the heap's: these lines differ from run to run).
const SHOWN_PIXELS: u64 = 0x7f3c_5a00_0010;
/// The address `%p` prints for the tile buffer.
const SHOWN_TILES: u64 = 0x7f3c_5900_0010;

/// glibc's `%p`.
fn ptr(x: u64) -> String {
    if x == 0 {
        "(nil)".to_owned()
    } else {
        format!("0x{x:x}")
    }
}

/// libtiff's settings for FreeSurfer's calls: D13's mode, the machine's memory.
fn options(g: &Globals) -> Options {
    Options {
        upstream_defects: g.mode(),
        max_alloc: Some(MACHINE_BYTES),
        messages: TiffMessages::default(),
    }
}

/// The notices of the TIFF handles closed since the last call, into the run's.
fn absorb(g: &mut Globals) {
    g.notices.extend(tiff::take_closed_notices());
}

/// A libtiff call's result; a codec this port does not decode (none since track C's relay 33).
fn ported<T>(r: Result<T, Unported>) -> Result<T, Fatal> {
    r.map_err(|_| Fatal::Exit(1))
}

/// `TIFFGetFieldDefaulted(tif, tag, &v)` (`defaulted`) or `TIFFGetField`: the value, `None` when
/// the C function returns 0 (the variable keeps what it held).
fn field(t: &mut Tiff, tg: u32, defaulted: bool, s: &mut dyn Streams) -> Option<Value> {
    if defaulted {
        t.get_field_defaulted(tg, s)
    } else {
        t.get_field(tg, s)
    }
}

/// The value as a C `int` written whole (a `uint32` field).
fn int_of(v: &Option<Value>) -> Option<i32> {
    match v {
        Some(Value::U32(x)) => Some(*x as i32),
        Some(Value::I32(x)) => Some(*x),
        Some(Value::U16(x)) => Some(i32::from(*x)),
        _ => None,
    }
}

/// The value as a C `short` (a `uint16` field).
fn short_of(v: &Option<Value>) -> Option<i16> {
    match v {
        Some(Value::U16(x)) => Some(*x as i16),
        Some(Value::U32(x)) => Some(*x as u16 as i16),
        Some(Value::I32(x)) => Some(*x as u16 as i16),
        _ => None,
    }
}

/// The value as a C `float`.
fn float_of(v: &Option<Value>) -> Option<f32> {
    match v {
        Some(Value::F32(x)) => Some(*x),
        Some(Value::F64(x)) => Some(*x as f32),
        _ => None,
    }
}

/// FS-873: a C variable that `TIFFGetFieldDefaulted` leaves unset (stack contents in
/// FreeSurfer): 0 here, noted.
fn unset<T: Default>(g: &mut Globals, v: Option<T>, what: &str) -> T {
    match v {
        Some(x) => x,
        None => {
            g.notices.add(
                &FS_873,
                format!("{what}: a TIFF field FreeSurfer's variable never receives (its stack contents; 0 here)"),
            );
            T::default()
        }
    }
}

/// FS-867: whether a 16-bit field written into the low two bytes of a C `int` equals `want`
/// (`PLANARCONFIG_SEPARATE`, `FILLORDER_LSB2MSB`): FreeSurfer's upper two bytes are stack
/// contents; the default compares the field's value; `reproduce` takes them as 0, noted where
/// they decide.
fn int16_is(g: &mut Globals, v: &Option<Value>, want: u16, what: &str) -> bool {
    let low = match v {
        Some(Value::U16(x)) => *x,
        Some(Value::U32(x)) => *x as u16,
        Some(Value::I32(x)) => *x as u16,
        _ => {
            g.notices.add(
                &FS_867,
                format!("{what}: a field never written into FreeSurfer's int (its stack contents; 0 here)"),
            );
            return want == 0;
        }
    };
    if low == want {
        g.notices.add(
            &FS_867,
            format!(
                "{what} {low}: FreeSurfer's int holds it in its low two bytes, the upper two its stack contents (taken as 0)"
            ),
        );
    }
    low == want
}

/// `(int)f` as x86-64 converts it (`cvttss2si`, `cvttsd2si`): -2147483648 beyond the range and
/// for NaN. Only `sizepix`, which nothing reads after `TiffReadImage`, takes it.
fn cvtt(f: f64) -> i32 {
    if f.is_nan() || f >= 2_147_483_648.0 || f < -2_147_483_648.0 {
        i32::MIN
    } else {
        f as i32
    }
}

/// One entry of `TiffReadImage`'s page table (`TiffDirectoryInfo`), the fields FreeSurfer reads
/// back.
#[derive(Clone, Copy, Debug, Default)]
struct DirInfo {
    toread: bool,
    sizeimage: u64,
    scanlinesize: u64,
    tilesize: u64,
    is_tiled: bool,
    width: i32,
    height: i32,
    nsamples: i16,
    bits: i16,
}

/// The page table `TiffDirectoryInfo tiffdirectoryinfo[nframe]` (a stack array of the count
/// `TIFFNumberOfDirectories` gives): the entries written.
struct Table {
    /// The array's length.
    len: i32,
    entries: Vec<Option<DirInfo>>,
}

impl Table {
    fn new(len: i32) -> Table {
        Table {
            len,
            entries: Vec::new(),
        }
    }

    /// The entry of page `i`, written (FS-863: past the array: FreeSurfer writes it on its stack
    /// beyond the array; kept here in both modes, noted).
    fn put(&mut self, g: &mut Globals, i: usize, e: DirInfo) {
        if i as i64 >= i64::from(self.len) {
            g.notices.add(
                &FS_863,
                format!(
                    "TiffReadDirectoryInfo: page {i} written past FreeSurfer's table of {} pages (its stack beyond the table)",
                    self.len
                ),
            );
        }
        if self.entries.len() <= i {
            self.entries.resize(i + 1, None);
        }
        self.entries[i] = Some(e);
    }

    /// Whether page `i`'s entry was written.
    fn written(&self, i: i32) -> bool {
        i >= 0 && self.entries.get(i as usize).is_some_and(Option::is_some)
    }

    /// The entry of page `i` (FS-863: one never written, or past the array: FreeSurfer reads
    /// its stack contents; zeros here, noted).
    fn get(&self, g: &mut Globals, i: i32) -> DirInfo {
        let past = i64::from(i) >= i64::from(self.len) || i < 0;
        match (past, self.entries.get(i.max(0) as usize).copied().flatten()) {
            (false, Some(e)) => e,
            (true, Some(e)) if i >= 0 && g.notices.fix() => e,
            (_, e) => {
                g.notices.add(
                    &FS_863,
                    format!(
                        "TiffReadImage: the table entry of page {i} {} (FreeSurfer reads its stack contents; zeros here)",
                        if e.is_some() {
                            "lies past the table"
                        } else {
                            "was never written"
                        }
                    ),
                );
                e.unwrap_or_default()
            }
        }
    }

    /// The pages read.
    fn pages(&self) -> usize {
        self.entries.len()
    }
}

/// `TiffReadImage(fname, frame0, nthreads, end_frame)`: the frames `frame0` to `end_frame`
/// (inclusive; to the last page when `end_frame` is below 0) of the TIFF file `fname` (relative
/// to `dir`). `Read::Null` when libtiff cannot open it (its message given).
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn read_image(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    frame0: i32,
    end_frame: i32,
) -> Result<Read, Fatal> {
    let path = utils::resolve(dir, fname).to_string_lossy().into_owned();
    let opts = options(g);
    let mut s = Std { out, err };
    let opened = Tiff::open(&path, "r", opts, &mut s);
    let Some(mut tif) = ported(opened)? else {
        absorb(g);
        return Ok(Read::Null);
    };
    if env.get("FS_TIFF_DEBUG").is_some() {
        g.fs_tiff_debug = true;
    }
    let nframe = i32::from(tif.number_of_directories(&mut s));
    let frame0 = frame0.max(0);
    let end_frame = if end_frame < 0 {
        nframe
    } else {
        end_frame.wrapping_add(1)
    };
    let mut table = Table::new(nframe);
    let r = read_directory_info(g, &mut s, &mut tif, &mut table, frame0, end_frame);
    let imagebufsize = match r {
        Ok(Some(n)) => n,
        Ok(None) => {
            // FS-868's default: the page that cannot be read stops the reading
            tif.close(&mut s);
            absorb(g);
            return Ok(Read::Null);
        }
        Err(e) => {
            tif.close(&mut s);
            absorb(g);
            return Err(e);
        }
    };
    let pages = table.pages() as i64;
    if i64::from(frame0) >= pages || i64::from(end_frame) > pages || end_frame <= frame0 {
        g.notices.add(
            &FS_864,
            format!(
                "TiffReadImage: frames [{frame0}, {end_frame}) of a file of {pages} pages read ({nframe} counted)"
            ),
        );
        if g.notices.fix() {
            log::error_printf(
                g,
                s.err,
                ERROR_BADPARM,
                &format!(
                    "TiffReadImage({fname}): frames {frame0} to {} asked of a file of {pages} pages",
                    i64::from(end_frame) - 1
                ),
            );
            tif.close(&mut s);
            absorb(g);
            return Ok(Read::Null);
        }
    }
    let nframe_toread = end_frame.wrapping_sub(frame0);
    if g.fs_tiff_debug {
        let _ = write!(
            s.out,
            "[DEBUG] TiffReadImage(): pages found={nframe}, pages to read={nframe_toread} [{frame0}, {end_frame}), image buf size={imagebufsize}\n"
        );
    }
    let _ = ported(tif.set_directory(frame0 as u16, &mut s))?;
    let v = field(&mut tif, tag::IMAGEWIDTH, true, &mut s);
    let width: i32 = unset(g, int_of(&v), "TiffReadImage: the image width");
    let v = field(&mut tif, tag::IMAGELENGTH, true, &mut s);
    let height: i32 = unset(g, int_of(&v), "TiffReadImage: the image length");
    let v = field(&mut tif, tag::SAMPLESPERPIXEL, true, &mut s);
    let mut nsamples: i16 = unset(g, short_of(&v), "TiffReadImage: the samples per pixel");
    let v = field(&mut tif, tag::BITSPERSAMPLE, true, &mut s);
    let bits: i16 = unset(g, short_of(&v), "TiffReadImage: the bits per sample");
    let photometric = short_of(&field(&mut tif, tag::PHOTOMETRIC, true, &mut s));
    let _fillorder = field(&mut tif, tag::FILLORDER, true, &mut s);
    let compression = short_of(&field(&mut tif, tag::COMPRESSION, true, &mut s));
    let v = field(&mut tif, tag::RESOLUTIONUNIT, true, &mut s);
    let resunit: i16 = unset(g, short_of(&v), "TiffReadImage: the resolution unit");
    let unit_default = if resunit == 1 {
        0.1f64 as f32
    } else {
        (10.0f64 * 2.54) as f32
    };
    let xres = float_of(&field(&mut tif, tag::XRESOLUTION, true, &mut s)).unwrap_or(unit_default);
    let yres = float_of(&field(&mut tif, tag::YRESOLUTION, true, &mut s)).unwrap_or(unit_default);
    let v = field(&mut tif, tag::ORIENTATION, true, &mut s);
    let orientation: i16 = unset(g, short_of(&v), "TiffReadImage: the orientation");
    tif.close(&mut s);
    absorb(g);
    if verbose_on(g) {
        let photometric = unset(
            g,
            photometric,
            "TiffReadImage: the photometric interpretation",
        );
        let compression = unset(g, compression, "TiffReadImage: the compression");
        let e = &mut s.err;
        let _ = write!(e, "\ntiff info\n");
        let _ = writeln!(e, "  size: ({width}, {height})");
        let _ = writeln!(e, "  samples/pixel: {nsamples}");
        let _ = writeln!(e, "  bits/sample: {bits}");
        let _ = writeln!(e, "  orientation: {orientation}");
        // the switch takes the short as an int: 32844 and 32845 never match
        let p = match i32::from(photometric) {
            0 => "  photometric: min value is white.\n",
            1 => "  photometric: min value is black.\n",
            2 => "  photometric: RGB color model.\n",
            3 => "  photometric: use palette.\n",
            4 => "  photometric: $holdout mask.\n",
            5 => "  photometric: color separations.\n",
            6 => "  photometric: YCbCr6 CCIR 601.\n",
            8 => "  photometric: 1976 CIE L*a*b* \n",
            10 => "  photometric: ITU L*a*b* \n",
            32844 => "  photometric: CIE Log2(L) \n",
            32845 => "  photometric: CIE Log2(L) (u',v') \n",
            _ => "  photometric: unknown type\n",
        };
        let _ = e.write_all(p.as_bytes());
        match i32::from(compression) {
            1 => {
                let _ = e.write_all(b"  compression: no compression\n");
            }
            5 => {
                let _ = e.write_all(b"  compression: Lempel-Ziv & Welch\n");
            }
            7 => {
                let _ = e.write_all(b"  compression: JPEG DCT compression\n");
            }
            32773 => {
                let _ = e.write_all(b"  compression: Macintosh RLE\n");
            }
            c => {
                let _ = writeln!(e, "  compression: {c} see /usr/include/tiff.h for meaning");
            }
        }
    }
    let ty = match nsamples {
        1 => match bits {
            16 => PFSHORT,
            32 => PFFLOAT,
            64 => PFDOUBLE,
            _ => PFBYTE,
        },
        3 | 4 => {
            nsamples = 3;
            PFRGB
        }
        _ => {
            log::error_exit(
                g,
                s.err,
                ERROR_BADPARM,
                &format!(
                    "IMAGE: nsamples={nsamples}, bits_per_sample={bits}.  only grey scale or RGB image is supported\n"
                ),
            )?;
            PFBYTE
        }
    };
    if nsamples != 1 && nsamples != 3 {
        log::error_exit(
            g,
            s.err,
            ERROR_BADPARM,
            &format!("IMAGE: nsamples = {nsamples}.  only grey scale or RGB image is supported\n"),
        )?;
    }
    // FS-872: layouts the image cannot hold
    let held = if ty == PFRGB {
        bits == 8
    } else {
        matches!(bits, 1 | 8 | 16 | 32 | 64)
    };
    if !held {
        g.notices.add(
            &FS_872,
            format!(
                "TiffReadImage: {} samples of {bits} bits in an image of {} bytes a pixel",
                if ty == PFRGB { "RGB" } else { "grey" },
                if ty == PFRGB { 3 } else { 1 }
            ),
        );
        if g.notices.fix() {
            log::error_printf(
                g,
                s.err,
                ERROR_UNSUPPORTED,
                &format!(
                    "TiffReadImage({fname}): pages of {} samples of {bits} bits are not supported",
                    if ty == PFRGB { 3 } else { 1 }
                ),
            );
            return Ok(Read::Null);
        }
    }
    let mut im = image_alloc(g, s.err, height, width, ty, nframe_toread, imagebufsize)?;
    let res = (xres + yres) / 2.0;
    match resunit {
        3 | 1 => {
            im.sizepix = cvtt(f64::from(100.0f32 / res));
            im.xsize = (100.0 / f64::from(xres)) as f32;
            im.ysize = (100.0 / f64::from(yres)) as f32;
        }
        _ => {
            im.sizepix = cvtt(2.54 / f64::from(res));
            im.xsize = (10.0 * 2.54 / f64::from(xres)) as f32;
            im.ysize = (10.0 * 2.54 / f64::from(yres)) as f32;
        }
    }
    let frame_bytes = im.sizeimage;
    let shape = (width, height, nsamples_page(&table, frame0), bits);
    let mut frame = frame0;
    while frame < end_frame {
        // reproduce: the frames from the first entry FreeSurfer reads from its stack on are not
        // read (FS-863: their outcome is the stack's)
        if g.notices.reproduce() && (0..=frame).any(|i| !table_defined(&table, i)) {
            let _ = table.get(g, frame);
            break;
        }
        read_image_directory(
            g,
            &mut s,
            &path,
            &mut im,
            frame,
            frame0,
            orientation,
            &table,
            frame_bytes,
            shape,
        )?;
        frame += 1;
    }
    Ok(Read::Image(im))
}

/// The samples per pixel of `frame0`'s page as its table entry holds them (the default's page
/// comparisons).
fn nsamples_page(t: &Table, frame0: i32) -> i16 {
    t.entries
        .get(frame0.max(0) as usize)
        .copied()
        .flatten()
        .map_or(0, |e| e.nsamples)
}

/// Whether FreeSurfer's table entry `i` is defined (written, inside the array).
fn table_defined(t: &Table, i: i32) -> bool {
    t.written(i) && i < t.len
}

/// `TiffReadDirectoryInfo(tif, info, start_page, end_page)`: every page's entry, from the first
/// until `TIFFReadDirectory` fails; the bytes of the pages in `[start_page, end_page)`. Separate
/// planes: `TiffReadDirectoryInfo():  PLANARCONFIG_SEPARATE unsupported` and FreeSurfer's
/// `ERROR_UNSUPPORTED` as the size (FS-868; the default: `None`).
fn read_directory_info(
    g: &mut Globals,
    s: &mut Std,
    tif: &mut Tiff,
    table: &mut Table,
    start: i32,
    end: i32,
) -> Result<Option<u64>, Fatal> {
    let _ = ported(tif.set_directory(0, s))?;
    let mut imagebufsize: u64 = 0;
    let mut npage: usize = 0;
    // the table's entry is written field by field: `is_tiled` before the planar check
    let mut last = DirInfo::default();
    loop {
        let mut e = DirInfo {
            is_tiled: tif.tiled(),
            ..last
        };
        let pc = field(tif, tag::PLANARCONFIG, true, s);
        if int16_is(
            g,
            &pc,
            values::PLANARCONFIG_SEPARATE,
            "TiffReadDirectoryInfo: the planar configuration",
        ) {
            // the entry holds only `is_tiled`: its other fields are memory contents
            let _ = e;
            log::error_printf(
                g,
                s.err,
                ERROR_UNSUPPORTED,
                "TiffReadDirectoryInfo():  PLANARCONFIG_SEPARATE unsupported",
            );
            g.notices.add(
                &FS_868,
                "TiffReadDirectoryInfo: its error code -3 returned as the image's size (2^64 - 3 bytes)",
            );
            if g.notices.fix() {
                return Ok(None);
            }
            return Ok(Some(ERROR_UNSUPPORTED as i64 as u64));
        }
        let v = field(tif, tag::IMAGEWIDTH, true, s);
        let width = int_of(&v).unwrap_or(last.width);
        let v = field(tif, tag::IMAGELENGTH, true, s);
        let height = int_of(&v).unwrap_or(last.height);
        let v = field(tif, tag::SAMPLESPERPIXEL, true, s);
        let nsamples = short_of(&v).unwrap_or(last.nsamples);
        let v = field(tif, tag::BITSPERSAMPLE, true, s);
        let bits = short_of(&v).unwrap_or(last.bits);
        for tg in [
            tag::PHOTOMETRIC,
            tag::FILLORDER,
            tag::COMPRESSION,
            tag::RESOLUTIONUNIT,
            tag::ORIENTATION,
            tag::XRESOLUTION,
            tag::YRESOLUTION,
        ] {
            let _ = field(tif, tg, true, s);
        }
        e.scanlinesize = tif.scanline_size() as i64 as u64;
        e.tilesize = tif.tile_size() as i64 as u64;
        let numpix = (width as i64 as u64).wrapping_mul(height as i64 as u64);
        e.sizeimage = numpix
            .wrapping_mul(i64::from(nsamples) as u64)
            .wrapping_mul(i64::from(i32::from(bits) / 8) as u64);
        let n = npage as i64;
        e.toread = n >= i64::from(start) && n < i64::from(end);
        if e.toread {
            imagebufsize = imagebufsize.wrapping_add(e.sizeimage);
        }
        e.width = width;
        e.height = height;
        e.nsamples = nsamples;
        e.bits = bits;
        table.put(g, npage, e);
        last = e;
        npage += 1;
        if !ported(tif.read_directory(s))? {
            break;
        }
    }
    Ok(Some(imagebufsize))
}

/// The image's pixels from byte `at` on, `n` bytes, given to `f` (FS-865: bytes outside the
/// pixels: zeros given, nothing kept, noted).
fn with_bytes<R>(
    g: &mut Globals,
    im: &mut Image,
    at: i128,
    n: usize,
    what: &str,
    f: impl FnOnce(&mut [u8]) -> R,
) -> R {
    let len = im.image.as_ref().map_or(0, Vec::len) as i128;
    if n == 0 || (at >= 0 && at + n as i128 <= len) {
        let pix = im.image.as_deref_mut().unwrap_or(&mut []);
        let a = at.clamp(0, len) as usize;
        let b = (a + n).min(pix.len());
        return f(&mut pix[a..b]);
    }
    g.notices.add(
        &FS_865,
        format!(
            "{what}: {n} bytes at byte {at} of an image of {len} bytes (FreeSurfer reads or writes its memory past them)"
        ),
    );
    let mut tmp = vec![0u8; n];
    let lo = at.max(0);
    let hi = (at + n as i128).min(len);
    if let Some(pix) = im.image.as_deref() {
        if lo < hi {
            tmp[(lo - at) as usize..(hi - at) as usize]
                .copy_from_slice(&pix[lo as usize..hi as usize]);
        }
    }
    let r = f(&mut tmp);
    if let Some(pix) = im.image.as_deref_mut() {
        if lo < hi {
            pix[lo as usize..hi as usize]
                .copy_from_slice(&tmp[(lo - at) as usize..(hi - at) as usize]);
        }
    }
    r
}

/// One byte of the image written (FS-865 outside it).
fn put_byte(g: &mut Globals, im: &mut Image, at: i128, v: u8, what: &str) {
    with_bytes(g, im, at, 1, what, |b| b[0] = v);
}

/// `TiffReadImageDirectory(fname, I, nframe, orientation, info)`: page `frame` into the image at
/// the bytes of the selected pages before it (FreeSurfer's; the default: `frame - frame0`
/// frames, FS-869), the file opened again for it; separate planes end the program
/// (`TiffReadImageDirectory():  PLANARCONFIG_SEPARATE unsupported`, status 1); a tiled page
/// through [`read_tiled`], a stripped one through [`read_stripped`].
#[allow(clippy::too_many_arguments)]
fn read_image_directory(
    g: &mut Globals,
    s: &mut Std,
    path: &str,
    im: &mut Image,
    frame: i32,
    frame0: i32,
    orientation: i16,
    table: &Table,
    frame_bytes: u64,
    shape: (i32, i32, i16, i16),
) -> Result<i32, Fatal> {
    let mut base: u64 = 0;
    let mut prev: u64 = 0;
    for i in 0..frame {
        let e = table.get(g, i);
        if !e.toread {
            continue;
        }
        if prev > 0 && prev != e.sizeimage {
            let _ = s.out.write_all(
                b"TiffReadImageDirectory():  pages have different image size. open them individually.",
            );
            return Err(Fatal::Exit(1));
        }
        prev = e.sizeimage;
        base = base.wrapping_add(e.sizeimage);
    }
    let me = table.get(g, frame);
    // FS-869: the pages' offsets and sizes
    let fixed = (i64::from(frame) - i64::from(frame0)) as u64 * frame_bytes;
    let same = (me.width, me.height, me.nsamples, me.bits) == shape;
    let size_differs = prev > 0 && prev != me.sizeimage;
    if fixed != base || !same || size_differs {
        g.notices.add(
            &FS_869,
            format!(
                "TiffReadImageDirectory: page {frame} at byte {base} of the image (its frame begins at byte {fixed}); \
                 its shape {}x{}, {} samples of {} bits (the first page's {}x{}, {} of {})",
                me.width, me.height, me.nsamples, me.bits, shape.0, shape.1, shape.2, shape.3
            ),
        );
        if g.notices.fix() {
            if !same || size_differs {
                let _ = s.out.write_all(
                    b"TiffReadImageDirectory():  pages have different image size. open them individually.",
                );
                return Err(Fatal::Exit(1));
            }
            base = fixed;
        }
    }
    if g.fs_tiff_debug {
        let pix = if im.image.is_some() { SHOWN_PIXELS } else { 0 };
        let _ = writeln!(
            s.out,
            "[DEBUG] TiffReadImageDirectory(): #{frame:03} I_tmp->image={} (I->image={}) {:4} x {:4} (nsamples={}, bits/sample={}, imagesize={}, tiled={}, scanlinesize={}, tilesize={})",
            ptr(pix.wrapping_add(base)),
            ptr(pix),
            me.width,
            me.height,
            me.nsamples,
            me.bits,
            me.sizeimage,
            i32::from(me.is_tiled),
            me.scanlinesize,
            me.tilesize
        );
    }
    let opts = options(g);
    let opened = Tiff::open(path, "r", opts, s);
    let Some(mut tif) = ported(opened)? else {
        // the file opened before: FreeSurfer then crashes on the null handle
        absorb(g);
        g.notices.add(
            &FS_865,
            "TiffReadImageDirectory: the file no longer opens (FreeSurfer crashes on the null handle)",
        );
        return Err(Fatal::Exit(139));
    };
    let _ = ported(tif.set_directory(frame as u16, s))?;
    let pc = field(&mut tif, tag::PLANARCONFIG, true, s);
    if int16_is(
        g,
        &pc,
        values::PLANARCONFIG_SEPARATE,
        "TiffReadImageDirectory: the planar configuration",
    ) {
        tif.close(s);
        absorb(g);
        let _ = s
            .out
            .write_all(b"TiffReadImageDirectory():  PLANARCONFIG_SEPARATE unsupported");
        return Err(Fatal::Exit(1));
    }
    let r = if me.is_tiled {
        read_tiled(g, s, &mut tif, im, base, orientation)
    } else {
        read_stripped(g, s, &mut tif, im, base, orientation)
    };
    tif.close(s);
    absorb(g);
    r
}

/// FS-865: a page of rows read into an image of no pixels (a width of 0): FreeSurfer gives
/// libtiff null buffers (zlib refuses them: `zlib error: (null)` for Deflate pages); here the
/// rows of 0 bytes are read into empty buffers.
fn no_pixels(g: &mut Globals, im: &Image, height: i32, what: &str) {
    if im.image.is_none() && height > 0 {
        g.notices.add(
            &FS_865,
            format!("{what}: {height} rows read into an image of no pixels (FreeSurfer gives libtiff null buffers)"),
        );
    }
}

/// The pixel's bytes in FreeSurfer's pointer of a grey row (`IMAGEpix`, `IMAGESpix`,
/// `IMAGEFpix`, `IMAGEDpix` by the bits).
fn grey_bytes(bits: i16) -> i128 {
    match bits {
        16 => 2,
        32 => 4,
        64 => 8,
        _ => 1,
    }
}

/// `TiffReadStrippedImage(tif, I_tmp, orientation)`: `TiffReadStrippedImage(): read strip image
/// <w> x <h>, nsamples = <n>, bits_per_sample = <b>`, then each scanline into its image row (row
/// `h - 1 - r` for the top-left orientation, `r` for the bottom-left one; others:
/// `IMAGE: orientation = <o>. we support only topleft or botleft` and the end); a scanline that
/// fails: `TiffReadStrippedImage():  TIFFReadScanline returned error`, the page's reading ends.
/// 1-bit rows unpacked (FS-866), 4-sample rows without the fourth sample.
fn read_stripped(
    g: &mut Globals,
    s: &mut Std,
    tif: &mut Tiff,
    im: &mut Image,
    base: u64,
    orientation: i16,
) -> Result<i32, Fatal> {
    let fillorder = field(tif, tag::FILLORDER, true, s);
    let v = field(tif, tag::IMAGEWIDTH, true, s);
    let width: i32 = unset(g, int_of(&v), "TiffReadStrippedImage: the image width");
    let v = field(tif, tag::IMAGELENGTH, true, s);
    let height: i32 = unset(g, int_of(&v), "TiffReadStrippedImage: the image length");
    let v = field(tif, tag::SAMPLESPERPIXEL, true, s);
    let nsamples: i16 = unset(
        g,
        short_of(&v),
        "TiffReadStrippedImage: the samples per pixel",
    );
    let v = field(tif, tag::BITSPERSAMPLE, true, s);
    let bits: i16 = unset(
        g,
        short_of(&v),
        "TiffReadStrippedImage: the bits per sample",
    );
    let _ = writeln!(
        s.out,
        "TiffReadStrippedImage(): read strip image {width} x {height}, nsamples = {nsamples}, bits_per_sample = {bits}"
    );
    let sl = tif.scanline_size();
    let scanlinesize = sl as u32;
    no_pixels(g, im, height, "TiffReadStrippedImage");
    let ocols = i128::from(im.ocols);
    let base = i128::from(base);
    let what = "TiffReadStrippedImage";
    // the 1-bit unpacking's order: FreeSurfer's int compared with FILLORDER_LSB2MSB (libtiff has
    // already put the bits in the machine's order: FS-866)
    let lsb = bits == 1
        && nsamples == 1
        && int16_is(
            g,
            &fillorder,
            values::FILLORDER_LSB2MSB,
            "TiffReadStrippedImage: the fill order",
        );
    for row in 0..height.max(0) {
        let index = match orientation {
            1 => height - row - 1,
            4 => row,
            o => {
                log::error_exit(
                    g,
                    s.err,
                    ERROR_BADPARM,
                    &format!("IMAGE: orientation = {o}. we support only topleft or botleft\n"),
                )?;
                0
            }
        };
        let idx = i128::from(index);
        let n = sl.max(0) as usize;
        let failed = |g: &mut Globals, s: &mut Std| -> Result<i32, Fatal> {
            log::error_printf(
                g,
                s.err,
                ERROR_BADFILE,
                "TiffReadStrippedImage():  TIFFReadScanline returned error",
            );
            Ok(ERROR_BADFILE)
        };
        match nsamples {
            1 => {
                let at = base + idx * ocols * grey_bytes(bits);
                let mut line = Vec::new();
                let r = with_bytes(g, im, at, n, what, |b| {
                    let r = tif.read_scanline(b, row as u32, 0, s);
                    line = b.to_vec();
                    r
                });
                if ported(r)? < 0 {
                    return failed(g, s);
                }
                if bits == 1 {
                    unpack_bits(g, im, base, idx, ocols, &line, scanlinesize, width, lsb);
                }
            }
            4 => {
                let mut ipix = vec![0u8; scanlinesize as usize];
                if ported(tif.read_scanline(&mut ipix, row as u32, 0, s))? < 0 {
                    return failed(g, s);
                }
                let w = width.max(0) as usize;
                if w * 4 > ipix.len() {
                    g.notices.add(
                        &FS_865,
                        format!(
                            "{what}: {w} pixels of 4 samples read from a scanline of {} bytes (FreeSurfer reads its memory past it; zeros here)",
                            ipix.len()
                        ),
                    );
                }
                for x in 0..w {
                    let at = base + (idx * ocols + x as i128) * 3;
                    let px = [0, 1, 2].map(|k| ipix.get(4 * x + k).copied().unwrap_or(0));
                    with_bytes(g, im, at, 3, what, |b| b.copy_from_slice(&px));
                }
            }
            3 => {
                let at = base + idx * ocols * 3;
                let r = with_bytes(g, im, at, n, what, |b| {
                    tif.read_scanline(b, row as u32, 0, s)
                });
                if ported(r)? < 0 {
                    return failed(g, s);
                }
            }
            _ => {}
        }
    }
    Ok(NO_ERROR)
}

/// FreeSurfer's unpacking of a 1-bit row: each of the scanline's bytes gives 8 pixels of 0 or 1
/// from column `8 * b` (up to 7 past the width, into the next row: FS-866), the low bit first
/// when FreeSurfer's fill order says so (libtiff has already put the bits high bit first). The
/// default: `width` pixels, the high bit first.
#[allow(clippy::too_many_arguments)]
fn unpack_bits(
    g: &mut Globals,
    im: &mut Image,
    base: i128,
    idx: i128,
    ocols: i128,
    line: &[u8],
    scanlinesize: u32,
    width: i32,
    lsb: bool,
) {
    let what = "TiffReadStrippedImage (1-bit rows)";
    let n = scanlinesize as usize;
    let spill = n as i64 * 8 > i64::from(width.max(0));
    if spill || lsb {
        g.notices.add(
            &FS_866,
            format!(
                "a 1-bit row of {width} pixels unpacked to {} pixels{}",
                n * 8,
                if lsb { ", low bit first" } else { "" }
            ),
        );
    }
    let fix = g.notices.fix();
    let count = if fix {
        (width.max(0) as usize).min(n * 8)
    } else {
        n * 8
    };
    for col in 0..count {
        let byte = line.get(col / 8).copied().unwrap_or(0);
        let bit = col % 8;
        let mask = if lsb && !fix {
            1u8 << bit
        } else {
            0x80u8 >> bit
        };
        let at = base + idx * ocols + col as i128;
        put_byte(g, im, at, u8::from(byte & mask > 0), what);
    }
}

/// `TiffReadTiledImage(tif, I_tmp)`: `TiffReadTiledImage(): read tile images <w> x <h>, tile size
/// <tw> x <tl>, nsamples = <n>, bits_per_sample = <b>`, then each tile into the image: FreeSurfer
/// counts `w / tw + 1` tiles a row and copies each tile row of `tw * n * (b / 8)` bytes as 3-byte
/// pixels at the row `h - 1 - y` (FS-870); a tile read's -1 is taken as a byte count (FS-871).
fn read_tiled(
    g: &mut Globals,
    s: &mut Std,
    tif: &mut Tiff,
    im: &mut Image,
    base: u64,
    orientation: i16,
) -> Result<i32, Fatal> {
    let v = field(tif, tag::IMAGEWIDTH, true, s);
    let width: i32 = unset(g, int_of(&v), "TiffReadTiledImage: the image width");
    let v = field(tif, tag::IMAGELENGTH, true, s);
    let height: i32 = unset(g, int_of(&v), "TiffReadTiledImage: the image length");
    let v = field(tif, tag::SAMPLESPERPIXEL, true, s);
    let nsamples: i16 = unset(g, short_of(&v), "TiffReadTiledImage: the samples per pixel");
    let v = field(tif, tag::BITSPERSAMPLE, true, s);
    let bits: i16 = unset(g, short_of(&v), "TiffReadTiledImage: the bits per sample");
    let v = field(tif, tag::TILEWIDTH, false, s);
    let tile_width: i32 = unset(g, int_of(&v), "TiffReadTiledImage: the tile width");
    let v = field(tif, tag::TILELENGTH, false, s);
    let tile_length: i32 = unset(g, int_of(&v), "TiffReadTiledImage: the tile length");
    let _ = writeln!(
        s.out,
        "TiffReadTiledImage(): read tile images {width} x {height}, tile size {tile_width} x {tile_length}, nsamples = {nsamples}, bits_per_sample = {bits}"
    );
    let tilesize = tif.tile_size() as i64 as u64;
    if g.fs_tiff_debug {
        let shown = if tilesize > MACHINE_BYTES {
            0
        } else {
            SHOWN_TILES
        };
        let _ = writeln!(
            s.out,
            "[DEBUG] TiffReadTiledImage(): tilebuf={}",
            ptr(shown)
        );
    }
    if tile_width <= 0 || tile_length <= 0 || tilesize > MACHINE_BYTES {
        // a division by 0, a loop that never ends or a null tile buffer
        g.notices.add(
            &FS_871,
            format!(
                "TiffReadTiledImage: tiles of {tile_width} x {tile_length} ({tilesize} bytes): FreeSurfer divides by zero, never ends or reads into a null buffer"
            ),
        );
        if g.notices.fix() {
            log::error_printf(
                g,
                s.err,
                ERROR_BADFILE,
                &format!(
                    "TiffReadTiledImage(): tiles of {tile_width} x {tile_length} cannot be read"
                ),
            );
            return Ok(ERROR_BADFILE);
        }
        // reproduce: the page is not read (FreeSurfer's outcome is a crash or a hang)
        return Ok(NO_ERROR);
    }
    no_pixels(g, im, height, "TiffReadTiledImage");
    let num_tile_x = width / tile_width + 1;
    let tiles_x = (i64::from(width) + i64::from(tile_width) - 1) / i64::from(tile_width);
    if i64::from(num_tile_x) != tiles_x || nsamples != 3 || bits != 8 || orientation != 1 {
        g.notices.add(
            &FS_870,
            format!(
                "TiffReadTiledImage: {width} pixels a row in tiles of {tile_width} counted as {num_tile_x} tiles (of {tiles_x}), {nsamples} samples of {bits} bits placed as 3-byte pixels, orientation {orientation} read as top-left"
            ),
        );
    }
    if g.notices.fix() {
        return read_tiled_fixed(
            g,
            s,
            tif,
            im,
            base,
            orientation,
            width,
            height,
            nsamples,
            bits,
            tile_width,
            tile_length,
        );
    }
    let what = "TiffReadTiledImage";
    let mut tilebuf = vec![0u8; tilesize as usize];
    let mut ntiles_read: i32 = -1;
    let bps = i64::from(i32::from(bits) / 8) as u64;
    let tile_bytes_per_row = (tile_width as i64 as u64)
        .wrapping_mul(i64::from(nsamples) as u64)
        .wrapping_mul(bps);
    let base = i128::from(base);
    let ocols = i128::from(im.ocols);
    let mut y: i32 = 0;
    while y < height {
        let mut x: i32 = 0;
        while x < width {
            let r = ported(tif.read_tile(&mut tilebuf, x as u32, y as u32, 0, 0, s))?;
            let mut bytes_left = r as i64 as u64;
            ntiles_read += 1;
            if g.fs_tiff_debug {
                let _ = writeln!(
                    s.out,
                    "[DEBUG] TiffReadTiledImage(): <<< TILE #{ntiles_read:03}, x={x}, y={y} >>>"
                );
            }
            if r < 0 {
                // FreeSurfer copies from past its tile buffer for 2^64 - 1 bytes
                g.notices.add(
                    &FS_871,
                    format!(
                        "TiffReadTiledImage: tile {ntiles_read} not read: FreeSurfer copies from past its tile buffer"
                    ),
                );
                return Ok(NO_ERROR);
            }
            let im_idx_height_offset = (ntiles_read / num_tile_x).wrapping_mul(tile_length);
            let mut nrow_tilebuf: i32 = -1;
            while bytes_left != 0 {
                nrow_tilebuf += 1;
                let im_idx_width = (ntiles_read % num_tile_x).wrapping_mul(tile_width);
                let im_idx_height = im_idx_height_offset.wrapping_add(nrow_tilebuf);
                if im_idx_height > height - 1 {
                    if g.fs_tiff_debug {
                        let _ = writeln!(
                            s.out,
                            "[DEBUG] TiffReadTiledImage(): ***END OF IMAGE ROW*** TILE #{ntiles_read:03}, bytes_left_tilebuf={bytes_left}, im_idx_width={im_idx_width}, im_idx_height={im_idx_height} (offset={im_idx_height_offset})"
                        );
                    }
                    break;
                }
                let index = height - im_idx_height - 1;
                let at = base + i128::from(index) * ocols * 3 + i128::from(im_idx_width) * 3;
                let mut bytes_tocopy = tile_bytes_per_row;
                if i64::from(im_idx_width) + i64::from(tile_width) > i64::from(width) {
                    let extra =
                        (im_idx_width.wrapping_add(tile_width).wrapping_sub(width)) as i64 as u64;
                    bytes_tocopy = bytes_tocopy.wrapping_sub(
                        extra
                            .wrapping_mul(i64::from(nsamples) as u64)
                            .wrapping_mul(bps),
                    );
                    if g.fs_tiff_debug {
                        let _ = writeln!(
                            s.out,
                            "[DEBUG] TiffReadTiledImage(): @@@DISCARD ROW PIXEL TILE #{ntiles_read:03}, bytes_left_tilebuf={bytes_left}, bytes_tocopy={bytes_tocopy}, im_idx_width={im_idx_width}, im_idx_height={im_idx_height} (offset={im_idx_height_offset})"
                        );
                    }
                }
                let idx_tilebuf = tilesize.wrapping_sub(bytes_left) as i32;
                if g.fs_tiff_debug {
                    let shown = SHOWN_PIXELS as i128 + at;
                    let _ = writeln!(
                        s.out,
                        "[DEBUG] TiffReadTiledImage(): TILE #{ntiles_read:03}, bytes_left_tilebuf={bytes_left}, idx_tilebuf={idx_tilebuf}, memcpy({}, {}, {bytes_tocopy}), im_idx_width={im_idx_width}, im_idx_height={im_idx_height} (offset={im_idx_height_offset})",
                        ptr(shown as u64),
                        ptr(SHOWN_TILES.wrapping_add(idx_tilebuf as i64 as u64))
                    );
                }
                // the copy: from the tile buffer (bytes past it: FreeSurfer's memory)
                let src = i128::from(idx_tilebuf);
                let n = bytes_tocopy;
                if n > 1 << 31 {
                    g.notices.add(
                        &FS_865,
                        format!(
                            "{what}: a copy of {n} bytes (FreeSurfer's memory past its buffers)"
                        ),
                    );
                    return Ok(NO_ERROR);
                }
                let n = n as usize;
                let lo = src.max(0);
                let hi = (src + n as i128).min(tilebuf.len() as i128);
                if src < 0 || src + n as i128 > tilebuf.len() as i128 {
                    g.notices.add(
                        &FS_865,
                        format!(
                            "{what}: {n} bytes copied from byte {src} of a tile buffer of {} bytes (FreeSurfer's memory past it; zeros here)",
                            tilebuf.len()
                        ),
                    );
                }
                let mut chunk = vec![0u8; n];
                if lo < hi {
                    chunk[(lo - src) as usize..(hi - src) as usize]
                        .copy_from_slice(&tilebuf[lo as usize..hi as usize]);
                }
                with_bytes(g, im, at, n, what, |b| b.copy_from_slice(&chunk));
                bytes_left = bytes_left.wrapping_sub(tile_bytes_per_row);
            }
            x = x.wrapping_add(tile_width);
            if x < 0 {
                break;
            }
        }
        y = y.wrapping_add(tile_length);
        if y < 0 {
            break;
        }
    }
    Ok(NO_ERROR)
}

/// The default's tiled reading (FS-870): `ceil(w / tw)` tiles a row, each tile row's pixels
/// (the tile's columns inside the image) at the image's row for the orientation (top-left:
/// `h - 1 - y`; bottom-left: `y`; others as for strips), by the image's pixel size (1-bit samples
/// unpacked high bit first, the fourth sample of 4 dropped); a tile that cannot be read ends the
/// page (`TiffReadTiledImage():  TIFFReadTile returned error`, FS-871).
#[allow(clippy::too_many_arguments)]
fn read_tiled_fixed(
    g: &mut Globals,
    s: &mut Std,
    tif: &mut Tiff,
    im: &mut Image,
    base: u64,
    orientation: i16,
    width: i32,
    height: i32,
    nsamples: i16,
    bits: i16,
    tile_width: i32,
    tile_length: i32,
) -> Result<i32, Fatal> {
    let what = "TiffReadTiledImage";
    let tilesize = tif.tile_size().max(0) as usize;
    let mut tilebuf = vec![0u8; tilesize];
    let row_bytes = tif.tile_row_size().max(0) as usize;
    let base = i128::from(base);
    let ocols = i128::from(im.ocols);
    let (tw, tl) = (i64::from(tile_width), i64::from(tile_length));
    let (w, h) = (i64::from(width.max(0)), i64::from(height.max(0)));
    let mut ntiles: i64 = 0;
    let mut y = 0i64;
    while y < h {
        let mut x = 0i64;
        while x < w {
            let r = ported(tif.read_tile(&mut tilebuf, x as u32, y as u32, 0, 0, s))?;
            if g.fs_tiff_debug {
                let _ = writeln!(
                    s.out,
                    "[DEBUG] TiffReadTiledImage(): <<< TILE #{ntiles:03}, x={x}, y={y} >>>"
                );
            }
            ntiles += 1;
            if r < 0 {
                g.notices.add(
                    &FS_871,
                    "TiffReadTiledImage: a tile not read (FreeSurfer copies from past its tile buffer)",
                );
                log::error_printf(
                    g,
                    s.err,
                    ERROR_BADFILE,
                    "TiffReadTiledImage():  TIFFReadTile returned error",
                );
                return Ok(ERROR_BADFILE);
            }
            let cols = tw.min(w - x) as usize;
            for r in 0..tl.min(h - y) {
                let im_row = y + r;
                let index = match orientation {
                    4 => im_row,
                    1 => h - im_row - 1,
                    o => {
                        log::error_exit(
                            g,
                            s.err,
                            ERROR_BADPARM,
                            &format!(
                                "IMAGE: orientation = {o}. we support only topleft or botleft\n"
                            ),
                        )?;
                        0
                    }
                };
                let src = &tilebuf[(r as usize * row_bytes).min(tilebuf.len())..];
                let dst_row = base + i128::from(index) * ocols;
                if nsamples == 1 && bits == 1 {
                    for c in 0..cols {
                        let byte = src.get(c / 8).copied().unwrap_or(0);
                        let v = u8::from(byte & (0x80 >> (c % 8)) > 0);
                        put_byte(g, im, dst_row + i128::from(x) + c as i128, v, what);
                    }
                } else if nsamples == 1 {
                    let b = grey_bytes(bits) as usize;
                    let n = cols * b;
                    let mut chunk = vec![0u8; n];
                    let k = n.min(src.len());
                    chunk[..k].copy_from_slice(&src[..k]);
                    let at = base + (i128::from(index) * ocols + i128::from(x)) * b as i128;
                    with_bytes(g, im, at, n, what, |d| d.copy_from_slice(&chunk));
                } else {
                    let step = if nsamples == 4 { 4 } else { 3 };
                    let at = base + (i128::from(index) * ocols + i128::from(x)) * 3;
                    let mut chunk = vec![0u8; cols * 3];
                    for c in 0..cols {
                        for k in 0..3 {
                            chunk[3 * c + k] = src.get(step * c + k).copied().unwrap_or(0);
                        }
                    }
                    with_bytes(g, im, at, cols * 3, what, |d| d.copy_from_slice(&chunk));
                }
            }
            x += tw;
        }
        y += tl;
    }
    Ok(NO_ERROR)
}

/// `TiffWriteImage(I, fname, frame)`: the image (bytes, shorts, ints, floats or doubles) as a
/// TIFF file of one page per frame (`TIFFOpen(fname, "w")`; when it fails, `ERROR_NOFILE`):
/// each page's fields (the sample format, the size, the bottom-left orientation, one sample,
/// SampleFormat INT again (FS-874), the bits, contiguous planes, min-is-black, inches,
/// `nint(25.4 / size)` pixels an inch where the size is not 0, Deflate), its rows from the
/// image's first, `TIFFWriteDirectory`; `TIFFClose`. Other pixel formats: `TiffWrite: pixel
/// format <f> not supported currently supported` (`ERROR_UNSUPPORTED`, the file left with its
/// header); a row that fails: `TiffWrite: TIFFWriteScanline returned error` (`ERROR_BADFILE`, the
/// file left as it was).
pub(crate) fn write_image(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    im: Option<&Image>,
    fname: &str,
) -> Result<Wrote, Fatal> {
    let path = utils::resolve(dir, fname).to_string_lossy().into_owned();
    let opts = options(g);
    let mut s = Std { out, err };
    let opened = Tiff::open(&path, "w", opts, &mut s);
    let Some(mut t) = ported(opened)? else {
        absorb(g);
        return Ok(Wrote {
            ok: false,
            code: ERROR_NOFILE,
        });
    };
    let Some(im) = im else {
        // I->pixel_format of the null image (FS-732's reproduce)
        drop(t);
        absorb(g);
        return Err(Fatal::Exit(139));
    };
    let (sample_format, spp, bps): (u16, i64, i64) = match im.pixel_format {
        PFBYTE => (values::SAMPLEFORMAT_INT, 1, 8),
        PFSHORT => (values::SAMPLEFORMAT_INT, 1, 16),
        PFINT => (values::SAMPLEFORMAT_INT, 1, 32),
        PFFLOAT => (values::SAMPLEFORMAT_IEEEFP, 1, 32),
        PFDOUBLE => (values::SAMPLEFORMAT_IEEEFP, 1, 64),
        f => {
            log::error_printf(
                g,
                s.err,
                ERROR_UNSUPPORTED,
                &format!("TiffWrite: pixel format {f} not supported currently supported"),
            );
            // the handle is never closed: the file keeps its header
            drop(t);
            absorb(g);
            return Ok(Wrote {
                ok: false,
                code: ERROR_UNSUPPORTED,
            });
        }
    };
    let elem = (bps / 8) as i128;
    // FS-874: bytes labelled signed; the second SampleFormat (INT) replaces the first
    let (sample_format, second) = match im.pixel_format {
        PFBYTE | PFFLOAT | PFDOUBLE => {
            g.notices.add(
                &FS_874,
                if im.pixel_format == PFBYTE {
                    "TiffWriteImage: unsigned bytes labelled signed (SampleFormat 2)"
                } else {
                    "TiffWriteImage: floating-point pixels labelled as signed integers (SampleFormat 2)"
                },
            );
            if g.notices.fix() {
                let f = if im.pixel_format == PFBYTE {
                    values::SAMPLEFORMAT_UINT
                } else {
                    values::SAMPLEFORMAT_IEEEFP
                };
                (f, f)
            } else {
                (sample_format, values::SAMPLEFORMAT_INT)
            }
        }
        _ => (sample_format, values::SAMPLEFORMAT_INT),
    };
    let pix: &[u8] = im.image.as_deref().unwrap_or(&[]);
    let ocols = i128::from(im.ocols);
    let mut base: u64 = 0;
    let set = |t: &mut Tiff, tg: u32, v: SetValue, s: &mut Std| {
        let _ = t.set_field(tg, v, s);
    };
    for _ in 0..im.num_frame.max(0) {
        set(
            &mut t,
            tag::SAMPLEFORMAT,
            SetValue::Int(i64::from(sample_format)),
            &mut s,
        );
        set(
            &mut t,
            tag::IMAGEWIDTH,
            SetValue::Int(i64::from(im.cols)),
            &mut s,
        );
        set(
            &mut t,
            tag::IMAGELENGTH,
            SetValue::Int(i64::from(im.rows)),
            &mut s,
        );
        set(
            &mut t,
            tag::ORIENTATION,
            SetValue::Int(i64::from(values::ORIENTATION_BOTLEFT)),
            &mut s,
        );
        set(&mut t, tag::SAMPLESPERPIXEL, SetValue::Int(spp), &mut s);
        set(
            &mut t,
            tag::SAMPLEFORMAT,
            SetValue::Int(i64::from(second)),
            &mut s,
        );
        set(&mut t, tag::BITSPERSAMPLE, SetValue::Int(bps), &mut s);
        set(
            &mut t,
            tag::PLANARCONFIG,
            SetValue::Int(i64::from(values::PLANARCONFIG_CONTIG)),
            &mut s,
        );
        set(
            &mut t,
            tag::PHOTOMETRIC,
            SetValue::Int(i64::from(values::PHOTOMETRIC_MINISBLACK)),
            &mut s,
        );
        set(
            &mut t,
            tag::RESOLUTIONUNIT,
            SetValue::Int(i64::from(values::RESUNIT_INCH)),
            &mut s,
        );
        if !fzero(f64::from(im.xsize)) {
            let r = utils::nint(g, 2.54 * 10.0 / f64::from(im.xsize));
            set(
                &mut t,
                tag::XRESOLUTION,
                SetValue::Double(f64::from(r)),
                &mut s,
            );
        }
        if !fzero(f64::from(im.ysize)) {
            let r = utils::nint(g, 2.54 * 10.0 / f64::from(im.ysize));
            set(
                &mut t,
                tag::YRESOLUTION,
                SetValue::Double(f64::from(r)),
                &mut s,
            );
        }
        set(
            &mut t,
            tag::COMPRESSION,
            SetValue::Int(i64::from(tiff::compression::DEFLATE)),
            &mut s,
        );
        let sl = t.scanline_size().max(0) as usize;
        for row in 0..im.rows.max(0) {
            let at = i128::from(base) + i128::from(row) * ocols * elem;
            let len = pix.len() as i128;
            let mut b = vec![0u8; sl + 64];
            let lo = at.max(0);
            let hi = (at + sl as i128).min(len);
            if sl > 0 && (at < 0 || at + sl as i128 > len) {
                g.notices.add(
                    &FS_865,
                    format!(
                        "TiffWriteImage: row {row}'s {sl} bytes at byte {at} of an image of {len} bytes (FreeSurfer writes its memory past them; zeros here)"
                    ),
                );
            }
            if lo < hi {
                b[(lo - at) as usize..(hi - at) as usize]
                    .copy_from_slice(&pix[lo as usize..hi as usize]);
            }
            if ported(t.write_scanline(&mut b, row as u32, 0, &mut s))? < 0 {
                log::error_printf(
                    g,
                    s.err,
                    ERROR_BADFILE,
                    "TiffWrite: TIFFWriteScanline returned error",
                );
                // the handle is never closed: nothing more reaches the file
                drop(t);
                absorb(g);
                return Ok(Wrote {
                    ok: false,
                    code: ERROR_BADFILE,
                });
            }
        }
        let _ = ported(t.write_directory(&mut s))?;
        base = base.wrapping_add(im.sizeimage);
    }
    t.close(&mut s);
    absorb(g);
    Ok(Wrote {
        ok: true,
        code: NO_ERROR,
    })
}
