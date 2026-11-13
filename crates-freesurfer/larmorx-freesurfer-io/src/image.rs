// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's 2-D images (`IMAGE`, the HIPS-style `struct header` of `include/hips.h`): the
//! functions FreeSurfer 8.2.0's programs reach (queue items 3.1-26's line 65 and 3.1-27's
//! `utils/image.cpp` 104-3815, `docs/freesurfer/queue.md`), translated from `utils/image.cpp`:
//! `ImageAlloc`, `ImageAllocBuffer`, `ImageFree`, `ImageCopy`, `ImageClearArea`, `ImageCheckSize`,
//! `ImageSetSize`, `ImageCopyFrames`, `ImageRescale`, `ImageScaleDown`, `ImageScaleUp`,
//! `init_header`, `h_copy`, `free_header` (Rust's drop); `utils/hips.cpp`'s conversion stubs
//! (`h_tob`, `h_toc`, `h_tod`, `h_todc`, `h_tof`, `h_toi`: the HIPS library is gone, they print
//! `error: hips is not supported` and return 0); and from `utils/filter.cpp` the pyramid
//! reduction `ImageScaleDown` calls (`ImageReduce`, `ImageConvolve1d`, `ImageReduce1d`; they live
//! here because `fs.filter`'s crate sits above this one). The callers are fs.volume's
//! `MRItoImage`/`MRItoImageView`, the surface parameterisations (`mrisp.cpp`) and fs.filter.
//!
//! The pixels are one byte buffer (`image`, `None` for FreeSurfer's NULL) of the length FreeSurfer
//! allocated; FreeSurfer's pointer walks (a frame's `image += sizeimage`, restored at the end) are
//! byte offsets into it. Every pixel read and write is checked against the buffer: FreeSurfer's
//! code reads and writes past it when a caller gives a frame, an area or a size the image does not
//! have (FS-405): both modes then skip the write and read 0, noted. Floats are kept as their
//! little-endian bytes, as on x86-64.
//!
//! The arithmetic follows the official build (GCC 8.4.0 at `-O3`, SSE2, no fused operations):
//! the reduction's sums `total + k * pixel` in `float`, the total the first operand (its NaN
//! wins, `larmorx_freesurfer_core::sse`), the scale's coordinates `nint(float / scale)`, the
//! conversions to bytes and coordinates as `cvttss2si` makes them (FS-406 where C leaves them
//! undefined). `ISPOW2` (`macros.h`: `pow(2, nint(log2f(x))) == x`) is decided exactly: it holds
//! for a positive power of two, and, under `reproduce`, for 0 (`log2f(0)` is `-inf`, `nint` of it
//! -2147483648, `pow(2, -2147483648)` 0); the reductions' count `nint(log2(1.0 / scale))` is
//! exact for the values it is computed on (within a few units of a power of two).

use std::io::Write;

use larmorx_core::defects::{FS_405, FS_406, FS_407, FS_408, FS_409, FS_410};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    ERROR_BADPARM, ERROR_NO_MEMORY, ERROR_NOMEMORY, ERROR_UNSUPPORTED, Fatal, NO_ERROR, error_exit,
    error_printf,
};
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::nint;

/// The pixel formats (`include/hips.h`).
pub const PFBYTE: i32 = 0;
pub const PFSHORT: i32 = 1;
pub const PFINT: i32 = 2;
pub const PFFLOAT: i32 = 3;
pub const PFCOMPLEX: i32 = 4;
pub const PFASCII: i32 = 5;
pub const PFDOUBLE: i32 = 6;
pub const PFDBLCOM: i32 = 7;
pub const PFMSBF: i32 = 30;
pub const PFLSBF: i32 = 31;
pub const PFSBYTE: i32 = 32;
pub const PFUSHORT: i32 = 33;
pub const PFUINT: i32 = 34;
pub const PFRGB: i32 = 35;
pub const PFRGBZ: i32 = 36;
pub const PFZRGB: i32 = 37;
pub const PFMIXED: i32 = 40;
pub const PFBGR: i32 = 41;
pub const PFBGRZ: i32 = 42;
pub const PFZBGR: i32 = 43;
pub const PFSTEREO: i32 = 44;
pub const PFINTPYR: i32 = 50;
pub const PFFLOATPYR: i32 = 51;
/// `IMAGE_VERTICAL`, `IMAGE_HORIZONTAL` (`image.h`: `ImageConvolve1d`'s and `ImageReduce1d`'s axis).
pub const IMAGE_VERTICAL: i32 = 0;
pub const IMAGE_HORIZONTAL: i32 = 1;
/// `HIPS_OK`.
pub const HIPS_OK: i32 = 0;

/// The size above which `calloc` is taken to fail (128 TiB, the x86-64 user address space).
const CALLOC_LIMIT: u64 = 1 << 47;
/// `ENOMEM`, what a failed `calloc` leaves in `errno`.
const ENOMEM: i32 = 12;

/// `IMAGE` (`struct header`): the fields FreeSurfer's reached code uses. The names, dates and
/// descriptions, the history and the extra parameters are never set (`init_header` leaves them
/// as `calloc` made them, NULL) and are not kept.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Image {
    pub num_frame: i32,
    pub orows: i32,
    pub ocols: i32,
    pub rows: i32,
    pub cols: i32,
    pub frow: i32,
    pub fcol: i32,
    pub pixel_format: i32,
    pub numcolor: i32,
    /// `numpix` (`fs_hsize_t`, `unsigned long`).
    pub numpix: u64,
    pub sizepix: i32,
    /// `sizeimage` (`unsigned long`): the bytes of one frame.
    pub sizeimage: u64,
    /// `image`: the pixels (`None`: NULL); the vector's length is what FreeSurfer allocated.
    pub image: Option<Vec<u8>>,
    pub imdealloc: i32,
    /// `firstpix` as an offset from `image` (`None`: NULL).
    pub firstpix: Option<i64>,
    pub xsize: f32,
    pub ysize: f32,
}

/// `calloc(n, 1)` as glibc answers it on x86-64 Linux: `None` when it fails (`errno` `ENOMEM`).
fn calloc(g: &mut Globals, n: u64) -> Option<Vec<u8>> {
    let mut v = Vec::new();
    if n > CALLOC_LIMIT || v.try_reserve_exact(n as usize).is_err() {
        g.errno = ENOMEM;
        return None;
    }
    v.resize(n as usize, 0);
    Some(v)
}

/// The bytes `[off, off + n)` of `im`'s buffer, or `None` (and FS-405 noted) when they are not
/// all inside it.
fn span(g: &mut Globals, im: &Image, off: i64, n: u64, what: &str) -> Option<usize> {
    let len = im.image.as_ref().map_or(0, Vec::len) as i128;
    let (o, e) = (i128::from(off), i128::from(off) + i128::from(n));
    if im.image.is_some() && o >= 0 && e <= len {
        return Some(off as usize);
    }
    g.notices.add(
        &FS_405,
        format!(
            "{what}: bytes {o} to {e} of an image of {} bytes{}",
            len,
            if im.image.is_none() {
                " (no pixels: NULL)"
            } else {
                ""
            }
        ),
    );
    None
}

fn rd_f32(g: &mut Globals, im: &Image, off: i64, what: &str) -> f32 {
    match span(g, im, off, 4, what) {
        Some(o) => {
            let b = im.image.as_ref().unwrap();
            f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
        }
        None => 0.0,
    }
}

fn rd_u32(g: &mut Globals, im: &Image, off: i64, what: &str) -> u32 {
    match span(g, im, off, 4, what) {
        Some(o) => {
            let b = im.image.as_ref().unwrap();
            u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
        }
        None => 0,
    }
}

fn rd_u8(g: &mut Globals, im: &Image, off: i64, what: &str) -> u8 {
    match span(g, im, off, 1, what) {
        Some(o) => im.image.as_ref().unwrap()[o],
        None => 0,
    }
}

fn wr(g: &mut Globals, im: &mut Image, off: i64, bytes: &[u8], what: &str) {
    if let Some(o) = span(g, im, off, bytes.len() as u64, what) {
        im.image.as_mut().unwrap()[o..o + bytes.len()].copy_from_slice(bytes);
    }
}

/// `memmove(dst + doff, src + soff, n)` between two images (FS-405 when either side leaves its
/// buffer: nothing copied).
fn copy_between(
    g: &mut Globals,
    src: &Image,
    soff: i64,
    dst: &mut Image,
    doff: i64,
    n: u64,
    what: &str,
) {
    let (Some(s), Some(d)) = (span(g, src, soff, n, what), span(g, dst, doff, n, what)) else {
        return;
    };
    let n = n as usize;
    let from = src.image.as_ref().unwrap()[s..s + n].to_vec();
    dst.image.as_mut().unwrap()[d..d + n].copy_from_slice(&from);
}

/// `cvttss2si`'s result for `f` (-2147483648 beyond the `int` range and for NaN).
fn cvtt(f: f32) -> i32 {
    if f.is_nan() || f >= 2_147_483_648.0 || f < -2_147_483_648.0 {
        i32::MIN
    } else {
        f as i32
    }
}

/// `(int)f` for a pixel coordinate: undefined beyond the `int` range and for NaN (FS-406):
/// `reproduce` gives x86-64's -2147483648, the default clamps (NaN: 0).
fn f2i(g: &mut Globals, f: f32, what: &str) -> i32 {
    if !(f.is_nan() || f >= 2_147_483_648.0 || f < -2_147_483_648.0) {
        return f as i32;
    }
    g.notices.add(
        &FS_406,
        format!("{what}: the coordinate {f} beyond the int range"),
    );
    if g.notices.reproduce() {
        return i32::MIN;
    }
    if f.is_nan() {
        0
    } else if f > 0.0 {
        i32::MAX
    } else {
        i32::MIN
    }
}

/// `(unsigned char)f` (the official build: `cvttss2si` into 32 bits, the low byte kept): defined
/// for `-1 < f < 256` (FS-406 beyond: `reproduce` keeps the low byte, the default clamps to 0 and
/// 255, NaN 0).
fn f2u8(g: &mut Globals, f: f32, what: &str) -> u8 {
    if f > -1.0 && f < 256.0 {
        return f as u8;
    }
    g.notices.add(
        &FS_406,
        format!("{what}: the value {f} beyond the byte range"),
    );
    if g.notices.reproduce() {
        return cvtt(f) as u8;
    }
    if f.is_nan() || f <= -1.0 { 0 } else { 255 }
}

/// `(char)f` stored in a byte (`char` is signed on x86-64): defined for `-129 < f < 128` (FS-406
/// beyond: `reproduce` keeps `cvttss2si`'s low byte, the default clamps to -128 and 127, NaN 0).
fn f2i8(g: &mut Globals, f: f32, what: &str) -> u8 {
    if f > -129.0 && f < 128.0 {
        return f as i32 as u8;
    }
    g.notices.add(
        &FS_406,
        format!("{what}: the value {f} beyond the char range"),
    );
    if g.notices.reproduce() {
        return cvtt(f) as u8;
    }
    if f.is_nan() {
        0
    } else if f > 0.0 {
        127
    } else {
        (-128i8) as u8
    }
}

/// `init_header(I, onm, snm, nfr, odt, rw, cl, pfmt, nc, desc, alloc)`: the frames, extents, pixel
/// format and its size (unknown formats, `PFCOMPLEX` and `PFDBLCOM` included, take 1 byte), the
/// pixel count (`(unsigned long)rw * cl`) and a frame's bytes; `image` NULL; with `alloc`,
/// `firstpix` NULL and `rw * cl * nfr * sizepix` bytes of zeros (`ErrorExit(ERROR_NOMEMORY,
/// "init_header: could not allocate %d bytes")` when they cannot be had).
#[allow(clippy::too_many_arguments)]
pub fn init_header(
    g: &mut Globals,
    err: &mut dyn Write,
    i: &mut Image,
    nfr: i32,
    rw: i32,
    cl: i32,
    pfmt: i32,
    alloc: bool,
) -> Result<(), Fatal> {
    i.num_frame = nfr;
    i.orows = rw;
    i.rows = rw;
    i.ocols = cl;
    i.cols = cl;
    i.pixel_format = pfmt;
    i.sizepix = match pfmt {
        PFFLOAT | PFINT | PFINTPYR | PFFLOATPYR => 4,
        PFDOUBLE => 8,
        PFSHORT => 2,
        PFRGB | PFBGR => 3,
        PFRGBZ | PFZRGB | PFBGRZ | PFZBGR => 4,
        _ => 1,
    };
    i.numpix = (rw as i64 as u64).wrapping_mul(cl as i64 as u64);
    i.sizeimage = i.numpix.wrapping_mul(i.sizepix as i64 as u64);
    i.image = None;
    if alloc {
        let bytes = (rw as i64 as u64)
            .wrapping_mul(cl as i64 as u64)
            .wrapping_mul(nfr as i64 as u64)
            .wrapping_mul(i.sizepix as i64 as u64);
        i.firstpix = None;
        match calloc(g, bytes) {
            Some(v) => i.image = Some(v),
            None => {
                error_exit(
                    g,
                    err,
                    ERROR_NOMEMORY,
                    &format!(
                        "init_header: could not allocate {} bytes",
                        bytes as u32 as i32
                    ),
                )?;
            }
        }
    }
    Ok(())
}

/// `ImageAllocBuffer(I, imagebufsize)`: an image of 0 bytes per frame gets no pixels
/// (`imdealloc` FALSE); otherwise `imagebufsize` bytes (0: `sizeimage * num_frame`) of zeros, or
/// `ERROR_NO_MEMORY` when they cannot be had; `firstpix` at the first row and column; then
/// `sizeimage * num_frame` bytes are cleared (past the buffer when `imagebufsize` is smaller:
/// FS-405).
pub fn image_alloc_buffer(g: &mut Globals, i: &mut Image, imagebufsize: u64) -> i32 {
    if i.sizeimage == 0 {
        i.imdealloc = 0;
        return NO_ERROR;
    }
    let mut npix = imagebufsize;
    if npix == 0 {
        npix = i.sizeimage.wrapping_mul(i.num_frame as i64 as u64);
    }
    i.image = None;
    match calloc(g, npix) {
        Some(v) => i.image = Some(v),
        None => return ERROR_NO_MEMORY,
    }
    i.firstpix = Some(if i.pixel_format == PFMSBF || i.pixel_format == PFLSBF {
        let fcb = i.fcol / 8;
        let cb = i.ocols.wrapping_add(7) / 8;
        i64::from(cb.wrapping_mul(i.frow).wrapping_add(fcb))
    } else {
        (i64::from(i.ocols) * i64::from(i.frow) + i64::from(i.fcol))
            .wrapping_mul(i64::from(i.sizepix))
    });
    i.imdealloc = 1;
    let clear = i.sizeimage.wrapping_mul(i.num_frame as i64 as u64);
    if clear > npix {
        g.notices.add(
            &FS_405,
            format!(
                "ImageAllocBuffer: clearing {clear} bytes of a buffer of {npix} (the rest not written)"
            ),
        );
    }
    NO_ERROR
}

/// `ImageAlloc(rows, cols, format, nframes, imagebufsize)`: the header ([`init_header`] without
/// pixels) and [`image_alloc_buffer`]. When the pixels cannot be had, `ErrorExit(Gerror,
/// "ImageAlloc: could not allocate %dx%d buffer\n")`: FreeSurfer's previous error code, 0 when
/// none, which exits with status 0 (FS-410: the default exits with `ERROR_NO_MEMORY`, 254); with
/// exceptions on and code 0 the program goes on with an image without pixels.
pub fn image_alloc(
    g: &mut Globals,
    err: &mut dyn Write,
    rows: i32,
    cols: i32,
    format: i32,
    nframes: i32,
    imagebufsize: u64,
) -> Result<Image, Fatal> {
    let mut i = Image::default();
    init_header(g, err, &mut i, nframes, rows, cols, format, false)?;
    let ecode = image_alloc_buffer(g, &mut i, imagebufsize);
    if ecode != NO_ERROR {
        let mut code = g.gerror;
        if code != ERROR_NO_MEMORY {
            g.notices.add(
                &FS_410,
                format!(
                    "ImageAlloc: no memory for a {rows}x{cols} image, ended with FreeSurfer's previous error code {code}"
                ),
            );
            if !g.notices.reproduce() {
                code = ERROR_NO_MEMORY;
            }
        }
        error_exit(
            g,
            err,
            code,
            &format!("ImageAlloc: could not allocate {rows}x{cols} buffer\n"),
        )?;
    }
    Ok(i)
}

/// `ImageFree(&I)`: a NULL image ends the program (`ErrorExit(ERROR_BADPARM, "ImageFree: null
/// pointer")`); otherwise the pixels and the header are freed (`free_header`).
pub fn image_free(g: &mut Globals, err: &mut dyn Write, i: Option<Image>) -> Result<(), Fatal> {
    if i.is_none() {
        error_exit(g, err, ERROR_BADPARM, "ImageFree: null pointer")?;
    }
    Ok(())
}

/// `ImageCheckSize(inImage, outImage, rows, cols, nframes)`: 1 when the output holds `rows *
/// cols * nframes` pixels (0 of each: the input's), the pixels' sizes left out; 0 for no output.
pub fn image_check_size(
    in_image: &Image,
    out: Option<&Image>,
    rows: i32,
    cols: i32,
    nframes: i32,
) -> i32 {
    let Some(out) = out else {
        return 0;
    };
    let rows = if rows == 0 { in_image.rows } else { rows };
    let cols = if cols == 0 { in_image.cols } else { cols };
    let nframes = if nframes == 0 {
        in_image.num_frame
    } else {
        nframes
    };
    let in_pix = i64::from(rows)
        .wrapping_mul(i64::from(cols))
        .wrapping_mul(i64::from(nframes));
    let out_pix = (out.numpix as i64).wrapping_mul(i64::from(out.num_frame));
    i32::from(out_pix >= in_pix)
}

/// `ImageSetSize(I, rows, cols)`: when the image holds `rows * cols` pixels a frame
/// ([`image_check_size`] against itself), its extents become those, the first row and column 0,
/// `numpix` and `sizeimage` the products (in `int`); 1. Otherwise nothing changes; 0.
pub fn image_set_size(i: &mut Image, rows: i32, cols: i32) -> i32 {
    if image_check_size(i, Some(i), rows, cols, 0) == 0 {
        return 0;
    }
    i.frow = 0;
    i.fcol = 0;
    i.rows = rows;
    i.orows = rows;
    i.cols = cols;
    i.ocols = cols;
    i.numpix = rows.wrapping_mul(cols) as i64 as u64;
    i.sizeimage = rows.wrapping_mul(cols).wrapping_mul(i.sizepix) as i64 as u64;
    1
}

/// The image's `firstpix = image` (FreeSurfer's pointer restorations).
fn reset_firstpix(i: &mut Image) {
    i.firstpix = i.image.as_ref().map(|_| 0);
}

/// `h_copy(Isrc, Idst)` at the frames' offsets: `numpix * sizepix` bytes (in `int`: a frame of
/// 2 GiB or more copies the bytes' count modulo 2^32, a negative count crashes: FS-405).
fn h_copy(g: &mut Globals, src: &Image, soff: i64, dst: &mut Image, doff: i64) {
    let bytes = src.numpix.wrapping_mul(src.sizepix as i64 as u64) as u32 as i32;
    let n = bytes as i64 as u64;
    copy_between(g, src, soff, dst, doff, n, "ImageCopy (h_copy)");
}

/// The HIPS conversions FreeSurfer 8.2.0 keeps as stubs (`hips.cpp`): the message, then 0.
fn hips_stub(err: &mut dyn Write) -> i32 {
    let _ = err.write_all(b"error: hips is not supported\n");
    HIPS_OK
}

/// `ImageCopy`'s frame loop into `dst` with its `image` pointer at `dbase` (a caller's frame).
fn copy_frames(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &mut Image,
    dst: &mut Image,
    dbase: i64,
) -> Result<(), Fatal> {
    let nframes = src.num_frame;
    dst.rows = src.rows;
    dst.cols = src.cols;
    let mut soff = 0i64;
    let mut doff = dbase;
    for _ in 0..nframes.max(0) {
        if dst.pixel_format == src.pixel_format {
            h_copy(g, src, soff, dst, doff);
        } else {
            match dst.pixel_format {
                PFDOUBLE | PFFLOAT | PFCOMPLEX | PFDBLCOM | PFINT | PFBYTE => {
                    hips_stub(err);
                }
                _ => {
                    error_exit(
                        g,
                        err,
                        ERROR_UNSUPPORTED,
                        &format!(
                            "ImageCopy {}-->{}, unsupported conversion\n",
                            src.pixel_format, dst.pixel_format
                        ),
                    )?;
                }
            }
        }
        soff = soff.wrapping_add(src.sizeimage as i64);
        doff = doff.wrapping_add(dst.sizeimage as i64);
    }
    reset_firstpix(src);
    reset_firstpix(dst);
    // `Isrc->num_frame = Idst->num_frame = nframes`: the output takes the source's frame count
    dst.num_frame = nframes;
    Ok(())
}

/// `ImageCopy(Isrc, Idst)`: an output with fewer pixels or frames than the source is freed and a
/// new one allocated like the source (as is one for no output); the output takes the source's
/// extents (`rows`, `cols`, not the stored ones) and, frame by frame, its bytes (`h_copy`, the
/// source's frame size) when the formats are the same, otherwise HIPS's message for each frame
/// (`error: hips is not supported`, nothing converted) or, for a format HIPS never converted to,
/// `ErrorExit(ERROR_UNSUPPORTED, "ImageCopy %d-->%d, unsupported conversion\n")`. Both images'
/// `firstpix` end at their first pixel.
pub fn image_copy(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &mut Image,
    dst: Option<Image>,
) -> Result<Image, Fatal> {
    let dst = match dst {
        Some(d) if d.numpix < src.numpix || d.num_frame < src.num_frame => None,
        d => d,
    };
    let mut dst = match dst {
        Some(d) => d,
        None => image_alloc(
            g,
            err,
            src.rows,
            src.cols,
            src.pixel_format,
            src.num_frame,
            0,
        )?,
    };
    copy_frames(g, err, src, &mut dst, 0)?;
    Ok(dst)
}

/// `ImageClearArea(I, r0, c0, rows, cols, val, frame)`: `val` into rows `[max(r0, 0), min(I->rows,
/// r0 + rows))` and columns likewise of one frame (`frame` < 0: every frame), a negative `rows` or
/// `cols` meaning the image's; floats, or bytes (`(char)val`, FS-406 beyond the `char` range);
/// another format: `ErrorReturn(ERROR_UNSUPPORTED, "ImageClearArea: unsupported image format %d")`
/// at the first row. Rows and frames are placed with the stored extents (`ocols`, `orows`).
#[allow(clippy::too_many_arguments)]
pub fn image_clear_area(
    g: &mut Globals,
    err: &mut dyn Write,
    i: &mut Image,
    r0: i32,
    c0: i32,
    rows: i32,
    cols: i32,
    val: f32,
    frame: i32,
) -> i32 {
    let r0 = r0.max(0);
    let c0 = c0.max(0);
    let rows = if rows < 0 { i.rows } else { rows };
    let cols = if cols < 0 { i.cols } else { cols };
    let rows = i.rows.min(r0.wrapping_add(rows));
    let cols = i.cols.min(c0.wrapping_add(cols));
    let (start, end) = if frame < 0 {
        (0, i.num_frame.wrapping_sub(1))
    } else {
        (frame, frame)
    };
    let frame_px = |i: &Image, f: i32| {
        i64::from(f)
            .wrapping_mul(i64::from(i.ocols))
            .wrapping_mul(i64::from(i.orows))
    };
    let mut f = start;
    while f <= end {
        for row in r0..rows {
            let px = frame_px(i, f)
                .wrapping_add(i64::from(row).wrapping_mul(i64::from(i.ocols)))
                .wrapping_add(i64::from(c0));
            match i.pixel_format {
                PFFLOAT => {
                    let bytes = val.to_le_bytes();
                    for k in 0..i64::from(cols - c0).max(0) {
                        wr(g, i, (px + k).wrapping_mul(4), &bytes, "ImageClearArea");
                    }
                }
                PFBYTE => {
                    let cval = f2i8(g, val, "ImageClearArea");
                    for k in 0..i64::from(cols - c0).max(0) {
                        wr(g, i, px + k, &[cval], "ImageClearArea");
                    }
                }
                _ => {
                    error_printf(
                        g,
                        err,
                        ERROR_UNSUPPORTED,
                        &format!(
                            "ImageClearArea: unsupported image format {}",
                            i.pixel_format
                        ),
                    );
                    return ERROR_UNSUPPORTED;
                }
            }
        }
        if f == i32::MAX {
            break;
        }
        f += 1;
    }
    0
}

/// `ImageCopyFrames(inImage, outImage, start, nframes, dst_frame)`: the input's frames `start` to
/// `start + nframes - 1` into the output from `dst_frame`, after `ImageCheckSize(in, out, 0, 0,
/// dst_frame + nframes)` (else `ErrorReturn(-1, (ERROR_NO_MEMORY, "ImageCopyFrames: outImage
/// not large enough\n"))`): doubles and floats into the same format only (else `ErrorExit`), bytes
/// into bytes, or converted into floats and unsigned integers, integers into floats, integers
/// and bytes (their low byte), RGB into RGB (the whole run of frames, once per frame); each frame
/// `rows * cols` pixels of the input. FreeSurfer writes the converted frames (and integers into
/// integers) into the output's frame of the source frame's number (FS-407: the default writes
/// them from `dst_frame`, as the other copies). Finally `ImageSetSize(out, in->rows, in->cols)`.
pub fn image_copy_frames(
    g: &mut Globals,
    err: &mut dyn Write,
    in_image: &Image,
    out: Option<&mut Image>,
    start: i32,
    nframes: i32,
    dst_frame: i32,
) -> Result<i32, Fatal> {
    let fits = image_check_size(
        in_image,
        out.as_deref(),
        0,
        0,
        dst_frame.wrapping_add(nframes),
    );
    let Some(out) = out.filter(|_| fits != 0) else {
        error_printf(
            g,
            err,
            ERROR_NO_MEMORY,
            "ImageCopyFrames: outImage not large enough\n",
        );
        return Ok(-1);
    };
    let unsupported = |inf: i32, outf: i32, nl: bool| {
        format!(
            "ImageCopyFrames: unsupported image pixel format {inf} -> {outf}{}",
            if nl { "\n" } else { "" }
        )
    };
    let end = start.wrapping_add(nframes).wrapping_sub(1);
    let ppf = in_image.rows.wrapping_mul(in_image.cols);
    let seq = |im: &Image, n: i32| {
        i64::from(n)
            .wrapping_mul(i64::from(im.ocols))
            .wrapping_mul(i64::from(im.orows))
    };
    let iseq = |im: &Image, n: i32| i64::from(n.wrapping_mul(im.ocols).wrapping_mul(im.orows));
    let what = "ImageCopyFrames";
    let mut frameno = start;
    while frameno <= end {
        let size = in_image.rows.wrapping_mul(in_image.cols);
        let dst = dst_frame.wrapping_add(frameno).wrapping_sub(start);
        // where a conversion writes: FreeSurfer's place `c` (the source frame's number, FS-407),
        // the default's `fix` (the destination frame, placed as the frame copies place it)
        let conv = |g: &mut Globals, c: i64, fix: i64| {
            if c != fix {
                g.notices.add(
                    &FS_407,
                    format!(
                        "ImageCopyFrames: frame {frameno} written at output byte {c}, not at {fix} (frame {dst})"
                    ),
                );
                if g.notices.reproduce() {
                    return c;
                }
            }
            fix
        };
        // the pixels a conversion loop takes (`while (size--)`: a negative count runs past
        // every buffer, FS-405)
        let n_px = |g: &mut Globals| {
            if size < 0 {
                g.notices.add(
                    &FS_405,
                    format!("ImageCopyFrames: a negative pixel count {size}"),
                );
            }
            i64::from(size.max(0))
        };
        let count = |n: i32| (n as i64 as u64).wrapping_mul(1);
        match in_image.pixel_format {
            PFDOUBLE | PFFLOAT => {
                if out.pixel_format != in_image.pixel_format {
                    error_exit(
                        g,
                        err,
                        ERROR_UNSUPPORTED,
                        &unsupported(in_image.pixel_format, out.pixel_format, true),
                    )?;
                }
                let s = if in_image.pixel_format == PFDOUBLE {
                    8
                } else {
                    4
                };
                copy_between(
                    g,
                    in_image,
                    seq(in_image, frameno).wrapping_mul(s),
                    out,
                    seq(out, dst).wrapping_mul(s),
                    count(ppf).wrapping_mul(s as u64),
                    what,
                );
            }
            PFBYTE => {
                if out.pixel_format == PFBYTE {
                    copy_between(
                        g,
                        in_image,
                        seq(in_image, frameno),
                        out,
                        seq(out, dst),
                        count(ppf),
                        what,
                    );
                } else {
                    let cin = seq(in_image, frameno);
                    match out.pixel_format {
                        PFFLOAT => {
                            let o = conv(
                                g,
                                seq(out, frameno).wrapping_mul(4),
                                seq(out, dst).wrapping_mul(4),
                            );
                            for k in 0..n_px(g) {
                                let v = f32::from(rd_u8(g, in_image, cin + k, what));
                                wr(g, out, o + 4 * k, &v.to_le_bytes(), what);
                            }
                        }
                        PFINT => {
                            let o = conv(
                                g,
                                iseq(out, frameno).wrapping_mul(4),
                                seq(out, dst).wrapping_mul(4),
                            );
                            for k in 0..n_px(g) {
                                let v = u32::from(rd_u8(g, in_image, cin + k, what));
                                wr(g, out, o + 4 * k, &v.to_le_bytes(), what);
                            }
                        }
                        _ => {
                            error_exit(
                                g,
                                err,
                                ERROR_UNSUPPORTED,
                                &unsupported(in_image.pixel_format, out.pixel_format, false),
                            )?;
                        }
                    }
                }
            }
            PFINT => {
                let iin = i64::from(ppf.wrapping_mul(frameno)).wrapping_mul(4);
                let at = |g: &mut Globals, unit: i64| {
                    conv(
                        g,
                        i64::from(ppf.wrapping_mul(frameno)).wrapping_mul(unit),
                        seq(out, dst).wrapping_mul(unit),
                    )
                };
                match out.pixel_format {
                    PFFLOAT => {
                        let o = at(g, 4);
                        for k in 0..n_px(g) {
                            let v = rd_u32(g, in_image, iin + 4 * k, what) as f32;
                            wr(g, out, o + 4 * k, &v.to_le_bytes(), what);
                        }
                    }
                    PFINT => {
                        let o = at(g, 4);
                        copy_between(g, in_image, iin, out, o, count(ppf).wrapping_mul(4), what);
                    }
                    PFBYTE => {
                        let o = at(g, 1);
                        for k in 0..n_px(g) {
                            let v = rd_u32(g, in_image, iin + 4 * k, what) as u8;
                            wr(g, out, o + k, &[v], what);
                        }
                    }
                    _ => {
                        error_exit(
                            g,
                            err,
                            ERROR_UNSUPPORTED,
                            &unsupported(in_image.pixel_format, out.pixel_format, false),
                        )?;
                    }
                }
            }
            PFRGB => {
                if out.pixel_format == PFRGB {
                    copy_between(
                        g,
                        in_image,
                        i64::from(start).wrapping_mul(in_image.sizeimage as i64),
                        out,
                        i64::from(dst_frame).wrapping_mul(out.sizeimage as i64),
                        in_image.sizeimage.wrapping_mul(nframes as i64 as u64),
                        what,
                    );
                } else {
                    error_exit(
                        g,
                        err,
                        ERROR_UNSUPPORTED,
                        &unsupported(in_image.pixel_format, out.pixel_format, false),
                    )?;
                }
            }
            _ => {
                error_exit(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    &unsupported(in_image.pixel_format, out.pixel_format, true),
                )?;
            }
        }
        if frameno == i32::MAX {
            break;
        }
        frameno += 1;
    }
    image_set_size(out, in_image.rows, in_image.cols);
    Ok(0)
}

/// `nint((float)n * scale)`.
fn scaled(g: &mut Globals, n: i32, scale: f32) -> i32 {
    nint(g, f64::from(n as f32 * scale))
}

/// `ISPOW2(x)` (`macros.h`), decided exactly (see the module's notes): a positive power of two;
/// 0 and -0 too, as FreeSurfer's `nint(-inf)` is -2147483648 under `reproduce` and the default's
/// clamped -2147483648 alike (FS-12 noted); +infinity only under the default's clamped `nint`,
/// where FreeSurfer's expression would then reduce 2^31 times: it is not a power of two there.
fn ispow2(g: &mut Globals, x: f32) -> bool {
    if x == 0.0 {
        // log2f(+-0) = -inf: nint's FS-12 notice
        let _ = nint(g, f64::NEG_INFINITY);
        return true;
    }
    if x.is_nan() || x.is_infinite() || x < 0.0 {
        let _ = nint(
            g,
            if x == f32::INFINITY {
                x.into()
            } else {
                f64::NAN
            },
        );
        return false;
    }
    let b = x.to_bits();
    let (exp, man) = ((b >> 23) & 0xff, b & 0x7f_ffff);
    if exp == 0 {
        man.is_power_of_two()
    } else {
        man == 0
    }
}

/// `nint(log2(y))` for a positive finite `y` within a few units of a power of two (or 0, whose
/// `log2` is `-inf`: FS-12's -2147483648).
fn nint_log2(g: &mut Globals, y: f64) -> i32 {
    if y == 0.0 {
        return nint(g, f64::NEG_INFINITY);
    }
    let b = y.to_bits();
    let exp = ((b >> 52) & 0x7ff) as i32;
    let (e, m) = if exp == 0 {
        // a subnormal double: its leading bit's place
        let lz = (b & 0x000f_ffff_ffff_ffff).leading_zeros() as i32 - 12;
        (
            -1023 - lz,
            f64::from_bits((b << (lz + 1)) & 0x000f_ffff_ffff_ffff | 0x3ff0 << 52),
        )
    } else {
        (
            exp - 1023,
            f64::from_bits(b & 0x000f_ffff_ffff_ffff | 0x3ff0 << 52),
        )
    };
    if m < std::f64::consts::SQRT_2 {
        e
    } else {
        e + 1
    }
}

/// `ImageRescale(inImage, outImage, scale)`: an output (when none is given) of `nint(cols *
/// scale)` by `nint(rows * scale)` like the input; then `ImageCopy` for a scale of 1,
/// `ImageScaleUp` above, `ImageScaleDown` otherwise; the output's pixel sizes the input's
/// divided by the scale. With a scale of 1 and a given output too small, FreeSurfer's copy goes
/// into a new image it drops, and the freed output is returned (FS-409: both modes return the
/// copy).
pub fn image_rescale(
    g: &mut Globals,
    err: &mut dyn Write,
    in_image: &mut Image,
    out: Option<Image>,
    scale: f32,
) -> Result<Image, Fatal> {
    let cols = scaled(g, in_image.cols, scale);
    let rows = scaled(g, in_image.rows, scale);
    let mut out = match out {
        Some(o) => o,
        None => image_alloc(
            g,
            err,
            rows,
            cols,
            in_image.pixel_format,
            in_image.num_frame,
            0,
        )?,
    };
    if scale == 1.0 {
        if out.numpix < in_image.numpix || out.num_frame < in_image.num_frame {
            g.notices.add(
                &FS_409,
                "ImageRescale by 1 into a smaller output: FreeSurfer returns the output it freed",
            );
        }
        out = image_copy(g, err, in_image, Some(out))?;
    } else if scale > 1.0 {
        image_scale_up(g, err, in_image, &mut out, scale)?;
    } else {
        image_scale_down(g, err, in_image, &mut out, scale)?;
    }
    out.xsize = sse::div(in_image.xsize, scale);
    out.ysize = sse::div(in_image.ysize, scale);
    Ok(out)
}

/// A frame's offset in bytes (`frame * sizeimage`, FreeSurfer's pointer walk).
fn frame_off(i: &Image, frame: i32) -> i64 {
    i64::from(frame).wrapping_mul(i.sizeimage as i64)
}

/// The single-frame view of `in`'s frame `frame` (its bytes copied), for the reduction.
fn frame_view(g: &mut Globals, i: &Image, frame: i32) -> Image {
    let mut v = i.clone();
    v.num_frame = 1;
    let off = frame_off(i, frame);
    let len = i.image.as_ref().map_or(0, Vec::len) as i64;
    v.image = i.image.as_ref().map(|b| {
        if off >= 0 && off <= len {
            b[off as usize..].to_vec()
        } else {
            g.notices.add(
                &FS_405,
                format!("ImageScaleDown: frame {frame} past the image's buffer"),
            );
            Vec::new()
        }
    });
    v
}

/// `in` seen from byte `off` of its buffer (FreeSurfer's advanced `image` pointer).
fn from_offset(i: &Image, off: i64) -> Image {
    let mut v = i.clone();
    if off != 0 {
        let len = i.image.as_ref().map_or(0, Vec::len) as i64;
        v.image = i.image.as_ref().map(|b| {
            if (0..=len).contains(&off) {
                b[off as usize..].to_vec()
            } else {
                Vec::new()
            }
        });
    }
    v
}

/// `ImageCopy(src, out)` where `out`'s `image` pointer is at `dbase` (a frame's place): an output
/// with fewer pixels or frames than `src` is freed by FreeSurfer and the copy goes into a new image
/// the caller never sees, the caller then using the freed output (FS-405: nothing copied, both
/// modes).
fn copy_at(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &mut Image,
    out: &mut Image,
    dbase: i64,
) -> Result<(), Fatal> {
    if out.numpix < src.numpix || out.num_frame < src.num_frame {
        g.notices.add(
            &FS_405,
            "ImageCopy frees the caller's output image (smaller than its source): freed memory used",
        );
        return Ok(());
    }
    copy_frames(g, err, src, out, dbase)
}

/// `ImageScaleDown(inImage, outImage, scale)`: after `ImageCheckSize(in, out, nint(rows *
/// scale), nint(cols * scale), frames)` (else `ErrorReturn(-1, (ERROR_NO_MEMORY, "ImageScaleDown:
/// output image not big enough\n"))`) and `ImageSetSize` of the output to those extents, each
/// frame's output pixel `(row, col)` takes the input's at `(nint(row / scale), nint(col /
/// scale))`: bytes into floats or bytes (an input place outside the image ends the program after
/// `in: %d, %d --> out: %d, %d!`, status 2), floats into bytes (FS-406 beyond them), floats into
/// floats; for floats into floats by a power of two (`ISPOW2(1 / scale)`) the input is reduced
/// `nint(log2(1 / scale))` times by `ImageReduce` and copied (`ImageCopy`) instead: FreeSurfer
/// does so for the first frame only and takes the others from memory past it (FS-408: the default
/// reduces every frame). Other formats: `ErrorReturn(-1 or -2, (ERROR_UNSUPPORTED, ...))`.
pub fn image_scale_down(
    g: &mut Globals,
    err: &mut dyn Write,
    in_image: &mut Image,
    out: &mut Image,
    scale: f32,
) -> Result<i32, Fatal> {
    let r = scaled(g, in_image.rows, scale);
    let c = scaled(g, in_image.cols, scale);
    if image_check_size(in_image, Some(out), r, c, in_image.num_frame) == 0 {
        error_printf(
            g,
            err,
            ERROR_NO_MEMORY,
            "ImageScaleDown: output image not big enough\n",
        );
        return Ok(-1);
    }
    let (r, c) = (
        scaled(g, in_image.rows, scale),
        scaled(g, in_image.cols, scale),
    );
    image_set_size(out, r, c);
    let (in_rows, in_cols) = (in_image.rows, in_image.cols);
    let (out_rows, out_cols) = (out.rows, out.cols);
    let what = "ImageScaleDown";
    let place = |g: &mut Globals, n: i32| nint(g, f64::from(n as f32 / scale));
    // the input after the power-of-two reduction of frame 0 (FreeSurfer's `inImage` from then on)
    let mut reduced = false;
    // the output's frames, which each reduction's copy sets to 1 (the default keeps them all)
    let out_frames = out.num_frame;
    for frame in 0..in_image.num_frame.max(0) {
        let ioff = frame_off(in_image, frame);
        let ooff = frame_off(out, frame);
        let in_px = |im: &Image, row: i32, col: i32| {
            i64::from(row)
                .wrapping_mul(i64::from(im.ocols))
                .wrapping_add(i64::from(col))
        };
        match (in_image.pixel_format, out.pixel_format) {
            (PFBYTE, PFFLOAT) => {
                let mut k = 0i64;
                for out_row in 0..out_rows {
                    for out_col in 0..out_cols {
                        let in_row = place(g, out_row);
                        let in_col = place(g, out_col);
                        let v = rd_u8(g, in_image, ioff + in_px(in_image, in_row, in_col), what);
                        wr(g, out, ooff + 4 * k, &f32::from(v).to_le_bytes(), what);
                        k += 1;
                    }
                }
            }
            (PFBYTE, PFBYTE) => {
                let mut k = 0i64;
                for out_row in 0..out_rows {
                    for out_col in 0..out_cols {
                        let in_row = place(g, out_row);
                        let in_col = place(g, out_col);
                        if in_row >= in_rows || in_col >= in_cols {
                            let _ = writeln!(
                                err,
                                "in: {in_row}, {in_col} --> out: {out_row}, {out_col}!"
                            );
                            return Err(Fatal::Exit(2));
                        }
                        let v = rd_u8(g, in_image, ioff + in_px(in_image, in_row, in_col), what);
                        wr(g, out, ooff + k, &[v], what);
                        k += 1;
                    }
                }
            }
            (PFBYTE, f) => {
                error_printf(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    &format!("ImageScaleDown: unsupported output pixel format {f}\n"),
                );
                return Ok(-1);
            }
            (PFFLOAT, PFBYTE) => {
                let mut k = 0i64;
                for out_row in 0..out_rows {
                    for out_col in 0..out_cols {
                        let in_row = place(g, out_row);
                        let in_col = place(g, out_col);
                        let v = rd_f32(
                            g,
                            in_image,
                            ioff + in_px(in_image, in_row, in_col).wrapping_mul(4),
                            what,
                        );
                        let b = f2u8(g, v, what);
                        wr(g, out, ooff + k, &[b], what);
                        k += 1;
                    }
                }
            }
            (PFFLOAT, PFFLOAT) => {
                if ispow2(g, 1.0f32 / scale) {
                    let reductions = nint_log2(g, 1.0 / f64::from(scale));
                    if reductions > 0 {
                        let mut cur = frame_view(g, in_image, frame);
                        for _ in 0..reductions {
                            cur = image_reduce(g, err, &cur, None)?;
                        }
                        copy_at(g, err, &mut cur, out, ooff)?;
                        reduced = true;
                        // FreeSurfer's input is now the one-frame reduction: its loop ends
                        if in_image.num_frame > 1 {
                            g.notices.add(
                                &FS_408,
                                format!(
                                    "ImageScaleDown by {scale}: FreeSurfer reduces only the first of {} frames",
                                    in_image.num_frame
                                ),
                            );
                            if g.notices.reproduce() {
                                break;
                            }
                        }
                    } else {
                        // every frame of the input from this frame's place (past the buffer
                        // beyond the first frame: FS-405)
                        let mut v = from_offset(in_image, ioff);
                        copy_at(g, err, &mut v, out, ooff)?;
                    }
                } else {
                    let mut k = 0i64;
                    for out_row in 0..out_rows {
                        for out_col in 0..out_cols {
                            let in_row = place(g, out_row);
                            let in_col = place(g, out_col);
                            let v = rd_f32(
                                g,
                                in_image,
                                ioff + in_px(in_image, in_row, in_col).wrapping_mul(4),
                                what,
                            );
                            wr(g, out, ooff + 4 * k, &v.to_le_bytes(), what);
                            k += 1;
                        }
                    }
                }
            }
            (PFFLOAT, f) => {
                error_printf(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    &format!("ImageScaleDown: unsupported output pixel format {f}\n"),
                );
                return Ok(-1);
            }
            (f, _) => {
                error_printf(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    &format!("ImageScaleDown: unsupported pixel format {f}\n"),
                );
                return Ok(-2);
            }
        }
    }
    if !reduced {
        reset_firstpix(in_image);
    } else if in_image.num_frame > 1 && !g.notices.reproduce() {
        out.num_frame = out_frames;
    }
    reset_firstpix(out);
    Ok(0)
}

/// `ImageScaleUp(inImage, outImage, scale)`: after `ImageCheckSize(in, out, nint(rows * scale),
/// nint(cols * scale), frames)` (else `ErrorReturn(-1, (ERROR_NO_MEMORY, "ImageScaleUp: output
/// image not large enough %d x %d -> %d x %d\n"))`) the output's extents (not the stored ones)
/// become `nint(cols * scale)` by `nint(rows * scale)`; each frame: bytes into bytes pixel by
/// output pixel from the input's `((int)(row / scale), (int)(col / scale))`; bytes, floats
/// (bytes: FS-406 beyond them) and integers (whatever the output's format) input pixel by input
/// pixel into the block of rows `nint(row * scale)` to `nint(row * scale + scale)` (excluded) and
/// the columns likewise. Other formats: `ErrorReturn(-1 or -2, (ERROR_UNSUPPORTED, ...))`.
pub fn image_scale_up(
    g: &mut Globals,
    err: &mut dyn Write,
    in_image: &mut Image,
    out: &mut Image,
    scale: f32,
) -> Result<i32, Fatal> {
    let r = scaled(g, in_image.rows, scale);
    let c = scaled(g, in_image.cols, scale);
    if image_check_size(in_image, Some(out), r, c, in_image.num_frame) == 0 {
        error_printf(
            g,
            err,
            ERROR_NO_MEMORY,
            &format!(
                "ImageScaleUp: output image not large enough {} x {} -> {} x {}\n",
                in_image.rows, in_image.cols, out.rows, out.cols
            ),
        );
        return Ok(-1);
    }
    out.cols = scaled(g, in_image.cols, scale);
    out.rows = scaled(g, in_image.rows, scale);
    let (out_rows, out_cols) = (out.rows, out.cols);
    let (in_rows, in_cols) = (in_image.rows, in_image.cols);
    let what = "ImageScaleUp";
    for frame in 0..in_image.num_frame.max(0) {
        let ioff = frame_off(in_image, frame);
        let ooff = frame_off(out, frame);
        // the block of an input pixel: rows [nint(row * s), nint(row * s + s)), columns alike
        let ends = |g: &mut Globals, n: i32| {
            let a = n as f32 * scale;
            let end = nint(g, f64::from(a + scale));
            let start = nint(g, f64::from(a));
            (start, end)
        };
        let unit = match (in_image.pixel_format, out.pixel_format) {
            (PFBYTE, PFBYTE) => {
                let mut k = 0i64;
                for out_row in 0..out_rows {
                    for out_col in 0..out_cols {
                        let in_col = f2i(g, out_col as f32 / scale, what);
                        let in_row = f2i(g, out_row as f32 / scale, what);
                        let v = rd_u8(
                            g,
                            in_image,
                            ioff + i64::from(in_row) * i64::from(in_image.ocols)
                                + i64::from(in_col),
                            what,
                        );
                        wr(g, out, ooff + k, &[v], what);
                        k += 1;
                    }
                }
                continue;
            }
            // the output's bytes per pixel as FreeSurfer writes them (integers: 4, whatever the
            // output's format)
            (PFFLOAT, PFBYTE) => 1,
            (PFBYTE, PFFLOAT) | (PFFLOAT, PFFLOAT) | (PFINT, _) => 4,
            (PFBYTE | PFFLOAT, f) => {
                error_printf(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    &format!("ImageScaleUp: unsupported output pixel format {f}\n"),
                );
                return Ok(-1);
            }
            (f, _) => {
                error_printf(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    &format!("ImageScaleUp: unsupported input pixel format {f}\n"),
                );
                return Ok(-2);
            }
        };
        let in_unit: i64 = if in_image.pixel_format == PFBYTE {
            1
        } else {
            4
        };
        let mut k = 0i64;
        for in_row in 0..in_rows {
            for in_col in 0..in_cols {
                let src = ioff + k * in_unit;
                k += 1;
                let (r0, end_row) = ends(g, in_row);
                let (_, end_col) = ends(g, in_col);
                for out_row in r0..end_row {
                    let c0 = scaled(g, in_col, scale);
                    let mut dst = ooff
                        + (i64::from(out_row) * i64::from(out.ocols) + i64::from(c0))
                            .wrapping_mul(unit);
                    let mut out_col = scaled(g, in_col, scale);
                    while out_col < end_col {
                        match (in_image.pixel_format, out.pixel_format) {
                            (PFBYTE, _) => {
                                let v = f32::from(rd_u8(g, in_image, src, what));
                                wr(g, out, dst, &v.to_le_bytes(), what);
                            }
                            (PFFLOAT, PFBYTE) => {
                                let v = rd_f32(g, in_image, src, what);
                                let b = f2u8(g, v, what);
                                wr(g, out, dst, &[b], what);
                            }
                            _ => {
                                let b = span(g, in_image, src, 4, what).map_or([0; 4], |o| {
                                    let s = in_image.image.as_ref().unwrap();
                                    [s[o], s[o + 1], s[o + 2], s[o + 3]]
                                });
                                wr(g, out, dst, &b, what);
                            }
                        }
                        dst = dst.wrapping_add(unit);
                        out_col += 1;
                    }
                }
            }
        }
    }
    reset_firstpix(in_image);
    reset_firstpix(out);
    Ok(0)
}

/// `ImageReduce`'s kernel (Burt and Adelson's, `a` = 0.4), in `float` as its initialiser computes it.
pub fn reduce_kernel() -> [f32; 5] {
    let a = 0.4f32;
    [0.25 - a / 2.0, 0.25, a, 0.25, 0.25 - a / 2.0]
}

/// `ImageReduce(Isrc, Idst)` (`utils/filter.cpp`): the source blurred vertically with
/// [`reduce_kernel`] into a float image of its extents, then reduced horizontally by two (and
/// sampled at every second row) into an output of `rows / 2` by `cols / 2` floats: the one given
/// when it is large enough (`ImageSetSize`), else a new one (a given one too small is freed).
/// FreeSurfer keeps the blurred image between calls (a `static`); only its memory differs.
pub fn image_reduce(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Image,
    dst: Option<Image>,
) -> Result<Image, Fatal> {
    let (rows, cols) = (src.rows, src.cols);
    let mut tmp = image_alloc(g, err, rows, cols, PFFLOAT, 1, 0)?;
    let (rows, cols) = (rows / 2, cols / 2);
    let mut dst = match dst {
        Some(mut d) if image_check_size(src, Some(&d), rows, cols, 0) != 0 => {
            image_set_size(&mut d, rows, cols);
            d
        }
        _ => image_alloc(g, err, rows, cols, PFFLOAT, 1, 0)?,
    };
    let k = reduce_kernel();
    image_convolve1d(g, err, src, &mut tmp, &k, IMAGE_VERTICAL)?;
    image_reduce1d(g, &tmp, &mut dst, &k, IMAGE_HORIZONTAL);
    Ok(dst)
}

/// `ImageConvolve1d(I, J, k, len, axis)` (`utils/filter.cpp`): each pixel of `J` (written one
/// after the other from its first, `I->cols` by `I->rows`) the `float` sum `total + k[i] *
/// pixel` (from 0, the total the first operand) over the kernel centred on it (`len / 2` before)
/// along the axis, places beyond the edges taken at the edge; horizontally the rows placed by
/// the stored width (`ocols`), vertically by `cols`. Index tables of `cols + 2 (len / 2)` and
/// `rows + 2 (len / 2)` entries: a negative count ends the program (`ErrorExit(ERROR_NO_MEMORY,
/// "ImageConvolve1d: could not allocate LUT\n")`).
pub fn image_convolve1d(
    g: &mut Globals,
    err: &mut dyn Write,
    i: &Image,
    j: &mut Image,
    k: &[f32],
    axis: i32,
) -> Result<(), Fatal> {
    let len = k.len() as i32;
    let (width, height) = (i.cols, i.rows);
    let halflen = len / 2;
    for n in [width, height] {
        if n.wrapping_add(2 * halflen) < 0 {
            g.errno = ENOMEM;
            error_exit(
                g,
                err,
                ERROR_NO_MEMORY,
                "ImageConvolve1d: could not allocate LUT\n",
            )?;
        }
    }
    let lut = |n: i32, x: i32| {
        if x < 0 {
            0
        } else if x >= n {
            n - 1
        } else {
            x
        }
    };
    let what = "ImageConvolve1d";
    let mut out = 0i64;
    for y in 0..height {
        for x in 0..width {
            let mut total = 0.0f32;
            for (n, &kv) in k.iter().enumerate() {
                let n = n as i32;
                let px = if axis == IMAGE_HORIZONTAL {
                    i64::from(y) * i64::from(i.ocols) + i64::from(lut(width, x + n - halflen))
                } else {
                    i64::from(x) + i64::from(lut(height, y + n - halflen)) * i64::from(width)
                };
                let v = rd_f32(g, i, px.wrapping_mul(4), what);
                total = sse::add(total, sse::mul(kv, v));
            }
            wr(g, j, out.wrapping_mul(4), &total.to_le_bytes(), what);
            out += 1;
        }
    }
    Ok(())
}

/// `ImageReduce1d(I, J, k, len, axis)` (`utils/filter.cpp`): each pixel `(x, y)` of `J` (its
/// `cols` by `rows`) the `float` sum `total + pixel * k[i]` (from 0) over the kernel centred on
/// `I`'s pixel `(2x, 2y)` (`(len - 1) / 2` before) along the axis, places beyond `I`'s edges
/// taken at the edge (the other coordinate `2x` or `2y` as it is).
pub fn image_reduce1d(g: &mut Globals, i: &Image, j: &mut Image, k: &[f32], axis: i32) {
    let len = k.len() as i32;
    let (jw, jh, iw, ih) = (j.cols, j.rows, i.cols, i.rows);
    let halflen = (len - 1) / 2;
    let clamp = |v: i32, n: i32| {
        if v < 0 {
            0
        } else if v >= n {
            n - 1
        } else {
            v
        }
    };
    let what = "ImageReduce1d";
    for y in 0..jh {
        for x in 0..jw {
            let mut total = 0.0f32;
            for (n, &kv) in k.iter().enumerate() {
                let n = n as i32;
                let (xi, yi) = if axis == IMAGE_HORIZONTAL {
                    (clamp(2 * x + n - halflen, iw), 2 * y)
                } else {
                    (2 * x, clamp(2 * y + n - halflen, ih))
                };
                let px = i64::from(yi) * i64::from(i.ocols) + i64::from(xi);
                let v = rd_f32(g, i, px.wrapping_mul(4), what);
                total = sse::add(total, sse::mul(v, kv));
            }
            let px = i64::from(y) * i64::from(j.ocols) + i64::from(x);
            wr(g, j, px.wrapping_mul(4), &total.to_le_bytes(), what);
        }
    }
}
