// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's 2-D image files (`utils/imageio.cpp`), as `mri_read` and `MRIwriteType` reach
//! them for the image type (`IMAGE_FILE`, names ending `.mat`, `.tif`, `.tiff`, `.svs`, `.jpg`,
//! `.jpeg`, `.pgm`, `.ppm`, `.pbm`, `.rgb`, or a name with `@<one of those>`): `ImageUnpackFileName`,
//! `ImageRead`, `ImageFRead`, `ImageWrite`, `ImageFWrite`; from `utils/image.cpp` the pieces they
//! call (`ImageFromMatrix`, `ImageInvert`); the formats' readers and writers FreeSurfer's own code
//! holds: the SGI images (`RGBReadImage`, `RGBwrite`, over [`crate::rgb`]), the PGM, PPM and PBM
//! stubs (FreeSurfer is built without netpbm: `pgm not supported`, ...), the HIPS stubs (the HIPS
//! library is gone: `utils/hips.cpp`'s functions print `error: hips is not supported` and return
//! 0, HIPS's "no error"), and the JPEG writer's refusal of an image that is not of bytes.
//!
//! TIFF images (libtiff) and the JPEG codec (the IJG's library) are third-party code FreeSurfer
//! bundles (`docs/freesurfer/third-party.md`, P-05: the main package's, track C's): reading a TIFF
//! or a JPEG image, and writing a TIFF or a JPEG image of bytes, say `... is not ported yet` until
//! they are.
//!
//! The image a reader returns is `ImageToMRI`'s input, the one a writer takes `MRItoImage`'s
//! output: both are fs.volume's, called through [`crate::upper`].
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-io.md` Part 25):
//! - **FS-718**: `mri_read` of an image whose reader fails (a missing or unreadable file, a MATLAB
//!   file that cannot be read, a HIPS frame, a PGM, PPM or PBM file, an SGI image of more than
//!   three planes) converts a null image: a crash. The default fails (`mri_read(): could not read
//!   the image <name>`); `reproduce` ends with status 139.
//! - **FS-719**: a complex MATLAB image is copied into an image of 1 byte per pixel, past its
//!   memory, before the program ends on the complex format; both modes copy nothing past it.
//! - **FS-731**: `MRIwriteType` of an image returns 0 when the image is not written (the file
//!   cannot be opened, the format refuses the image, HIPS writes nothing). The default returns the
//!   error; `reproduce` 0.
//! - **FS-732**: `MRIwriteType` of a volume whose type images do not take (16-bit, 64-bit, RGB
//!   ...) goes on with a null image after `MRItoImage`'s error: a crash (status 139), or for PGM
//!   and PPM names the format's refusal and `ImageFree: null pointer`. The default stops after the
//!   error (nothing written, `ERROR_UNSUPPORTED`); `reproduce` follows FreeSurfer.
//! - **FS-730**: an SGI image wider or taller than 65,535 pixels is written with its sizes modulo
//!   65,536 (a wrong file, or `img_seek: row number out of range`). The default refuses it;
//!   `reproduce` writes FreeSurfer's file.
//! - [`crate::rgb`]'s defects for the SGI images.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_718, FS_719, FS_727, FS_730, FS_731, FS_732};
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::diag::{DIAG_WRITE, diag_printf};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BAD_PARM, ERROR_BADFILE, ERROR_BADPARM, ERROR_NO_FILE, ERROR_UNSUPPORTED, Fatal,
    NO_ERROR,
};
use larmorx_freesurfer_core::matrix::{MATRIX_COMPLEX, Matrix};
use larmorx_freesurfer_core::utils::str_upper;
use larmorx_freesurfer_mri::mri::Mri;

use crate::cfile::CFile;
use crate::image::{Image, PFBYTE, PFCOMPLEX, PFFLOAT, PFRGB, image_alloc, image_free};
use crate::rgb::RgbImage;
use crate::upper;

/// The image file types (`include/image.h`).
pub const HIPS_IMAGE: i32 = 0;
pub const MATLAB_IMAGE: i32 = 1;
pub const TIFF_IMAGE: i32 = 2;
pub const JPEG_IMAGE: i32 = 3;
pub const PGM_IMAGE: i32 = 4;
pub const PPM_IMAGE: i32 = 5;
pub const PBM_IMAGE: i32 = 6;
pub const RGBI_IMAGE: i32 = 7;

/// `UNCOMPRESSED(1)`: the SGI type FreeSurfer writes on Linux.
const SGI_UNCOMPRESSED_1: u16 = 0x0001;

/// A HIPS function (`utils/hips.cpp`): its message on standard error; it returns 0.
fn hips(err: &mut dyn Write) {
    let _ = err.write_all(b"error: hips is not supported\n");
}

/// `ImageUnpackFileName(inFname, &frame, &type, outFname)`: the name up to its last `#` (when it
/// has one), the frame `%d` reads after it (-1 when none, or when it reads no number) and the type
/// the extension gives: what follows the name's last `.` (found before the `#` is cut, so a `.`
/// after the `#` still counts), upper-cased: `MAT`, `TIF`/`TIFF`/`SVS`, `JPG`/`JPEG`, `PGM`, `PPM`,
/// `PBM`, `RGB`; anything else (or no `.`) HIPS.
pub fn image_unpack_file_name(g: &mut Globals, name: &str) -> (String, i32, i32) {
    let number = name.rfind('#');
    let dot = name.rfind('.');
    let mut frame = -1;
    let mut out = name.to_owned();
    if let Some(n) = number {
        let mut b = Bytes::new(&name.as_bytes()[n + 1..]);
        let mut sc = Scanf::new(&mut b);
        if let Some(v) = sc.d(&mut g.errno, true) {
            frame = v;
        }
        out.truncate(n);
    }
    let ty = match dot {
        Some(d) => {
            // the extension: up to the end of the string the dot is in (the `#` cut ends the
            // name before a dot that came before it)
            let rest = &name[d + 1..];
            let ext = match number {
                Some(n) if n > d => &name[d + 1..n],
                _ => rest,
            };
            match str_upper(ext).as_str() {
                "MAT" => MATLAB_IMAGE,
                "TIF" | "TIFF" | "SVS" => TIFF_IMAGE,
                "JPG" | "JPEG" => JPEG_IMAGE,
                "PGM" => PGM_IMAGE,
                "PPM" => PPM_IMAGE,
                "PBM" => PBM_IMAGE,
                "RGB" => RGBI_IMAGE,
                _ => HIPS_IMAGE,
            }
        }
        None => HIPS_IMAGE,
    };
    (out, frame, ty)
}

fn not_ported(err: &mut dyn Write, what: &str, name: &str) {
    let _ = writeln!(
        err,
        "larmorx-freesurfer: {what} is not ported yet ({name}; the codec is third-party code, track C's)"
    );
}

/// What [`image_read`] gives.
pub enum Read {
    /// The image.
    Image(Image),
    /// FreeSurfer's `NULL` (after the reader's message, if it has one).
    Null,
    /// A format whose codec is not ported yet (its message given).
    NotPorted,
}

/// `ImageRead(fname, nthreads, start_frame, end_frame)`: the image the name's type gives
/// ([`image_unpack_file_name`]). The threads and frames concern TIFF images only.
pub fn image_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Read, Fatal> {
    let (buf, frame0, ty) = image_unpack_file_name(g, fname);
    match ty {
        TIFF_IMAGE => {
            not_ported(err, "reading TIFF images", &buf);
            Ok(Read::NotPorted)
        }
        MATLAB_IMAGE => {
            diag_printf(
                g,
                DIAG_WRITE,
                out,
                &format!(
                    "ImageRead: buf={buf}, frame={frame0}, type={ty} (M={MATLAB_IMAGE},H={HIPS_IMAGE})\n"
                ),
            );
            let Some(mat) = crate::matfile::matlab_read(g, out, err, dir, &buf)? else {
                log::error_printf(g, err, ERROR_NO_FILE, &format!("ImageRead({buf}) failed\n"));
                return Ok(Read::Null);
            };
            let mut i = image_from_matrix(g, err, &mat)?;
            image_invert_in_place(g, err, &mut i)?;
            Ok(Read::Image(i))
        }
        HIPS_IMAGE => {
            let fp = match CFile::open(dir, &buf, "rb") {
                Ok(f) => f,
                Err(e) => {
                    g.errno = e;
                    log::error_printf(
                        g,
                        err,
                        ERROR_NO_FILE,
                        &format!("ImageRead({buf}, {frame0}) failed\n"),
                    );
                    return Ok(Read::Null);
                }
            };
            let i = image_f_read_hips(g, err, &buf, frame0, 1);
            let _ = fp.close();
            Ok(i.map_or(Read::Null, Read::Image))
        }
        JPEG_IMAGE => {
            // JPEGReadImage: the file opened before the codec reads it
            match CFile::open(dir, &buf, "rb") {
                Ok(f) => {
                    let _ = f.close();
                    not_ported(err, "reading JPEG images", &buf);
                    Ok(Read::NotPorted)
                }
                Err(e) => {
                    g.errno = e;
                    log::error_exit(
                        g,
                        err,
                        ERROR_NO_FILE,
                        "JPEGReadImage:  Input file does not exist\n",
                    )?;
                    Ok(Read::Null)
                }
            }
        }
        PGM_IMAGE | PPM_IMAGE | PBM_IMAGE => {
            let what = match ty {
                PGM_IMAGE => "pgm",
                PPM_IMAGE => "ppm",
                _ => "pbm",
            };
            log::error_printf(g, err, ERROR_UNSUPPORTED, &format!("{what} not supported"));
            Ok(Read::Null)
        }
        RGBI_IMAGE => Ok(rgb_read_image(g, err, dir, &buf)?.map_or(Read::Null, Read::Image)),
        _ => Ok(Read::Null),
    }
}

/// `ImageFRead(fp, fname, start, nframes)` over the HIPS stubs: the header read is HIPS's (its
/// message; an empty header: no rows, columns or frames, bytes), the `endian` parameter's look-up
/// too (a message, none found). Without a frame (`start` below 0) the image is that empty header;
/// with one, the frame is beyond the header's 0 frames: `ImageFRead(<name>, <frame>) - frame out of
/// bounds` and `None`.
fn image_f_read_hips(
    g: &mut Globals,
    err: &mut dyn Write,
    fname: &str,
    start: i32,
    nframes: i32,
) -> Option<Image> {
    let mut i = Image::default();
    hips(err); // fread_header
    hips(err); // findparam(I, "endian")
    let (start, nframes) = if start < 0 {
        (0, i.num_frame)
    } else {
        // fseek(fp, sizeimage * start, SEEK_CUR): 0 bytes
        (start, nframes)
    };
    let end_frame = start.wrapping_add(nframes).wrapping_sub(1);
    if end_frame >= i.num_frame {
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("ImageFRead({fname}, {end_frame}) - frame out of bounds"),
        );
        return None;
    }
    i.num_frame = nframes;
    // ImageAllocBuffer: 0 bytes a frame, no pixels; ImageValid: bytes are valid (no swap)
    i.imdealloc = 0;
    Some(i)
}

/// `ImageFromMatrix(matrix, NULL)`: an image of the matrix's rows and columns (`PFFLOAT`, or
/// `PFCOMPLEX` for a complex matrix) holding its numbers row by row. FS-719: FreeSurfer gives a
/// complex image 1 byte per pixel and copies 8 bytes per pixel into it; both modes copy what fits.
fn image_from_matrix(g: &mut Globals, err: &mut dyn Write, m: &Matrix) -> Result<Image, Fatal> {
    let complex = m.mtype == MATRIX_COMPLEX;
    let format = if complex { PFCOMPLEX } else { PFFLOAT };
    let mut i = image_alloc(g, err, m.rows, m.cols, format, 1, 0)?;
    let bytes: Vec<u8> = m.data().iter().flat_map(|v| v.to_le_bytes()).collect();
    let want = (i64::from(m.rows) * i64::from(m.cols) * 4 * if complex { 2 } else { 1 }) as usize;
    if let Some(pix) = i.image.as_mut() {
        let n = want.min(pix.len()).min(bytes.len());
        if want > pix.len() {
            g.notices.add(
                &FS_719,
                format!(
                    "a complex MATLAB image: FreeSurfer copies {want} bytes into its {} bytes of pixels",
                    pix.len()
                ),
            );
        }
        pix[..n].copy_from_slice(&bytes[..n]);
    }
    Ok(i)
}

/// `ImageInvert(I, I)`: HIPS's `h_invert` (its message; nothing flipped). A complex image is
/// split into two float images first and combined back (two messages).
fn image_invert_in_place(g: &mut Globals, err: &mut dyn Write, i: &mut Image) -> Result<(), Fatal> {
    if i.pixel_format == PFCOMPLEX {
        let _imag = image_alloc(g, err, i.rows, i.cols, PFFLOAT, 1, 0)?;
        let _real = image_alloc(g, err, i.rows, i.cols, PFFLOAT, 1, 0)?;
        hips(err);
        hips(err);
    } else {
        hips(err);
    }
    Ok(())
}

/// `RGBReadImage(fname)`: an SGI image as an RGB image (`PFRGB`, 3 bytes a pixel, in the file's
/// row order): rows of planes 0, 1 and 2 (of an image of fewer than three dimensions, plane 0
/// three times: grey), each value's low byte. `None` when the file cannot be opened (FreeSurfer
/// goes on with a null image: FS-718) or has more than three planes (`Too many color planes in
/// RGBReadImage (<name>)`).
fn rgb_read_image(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Image>, Fatal> {
    let Some(mut rgb) = RgbImage::open_read(g, err, dir, fname)? else {
        return Ok(None);
    };
    let (rows, cols) = (rgb.ysize, rgb.xsize);
    if rgb.zsize > 3 {
        log::error_printf(
            g,
            err,
            ERROR_BAD_PARM,
            &format!("Too many color planes in RGBReadImage ({fname})\n"),
        );
        return Ok(None);
    }
    let mut im = image_alloc(g, err, i32::from(rows), i32::from(cols), PFRGB, 1, 0)?;
    let n = usize::from(cols);
    let mut planes = [vec![0u16; n], vec![0u16; n], vec![0u16; n]];
    let mut filled = [vec![false; n], vec![false; n], vec![false; n]];
    let mut unread = false;
    for i in 0..u32::from(rows) {
        for (z, (p, f)) in planes.iter_mut().zip(filled.iter_mut()).enumerate() {
            rgb.getrow(g, err, p, f, i, z as u32)?;
        }
        let Some(pix) = im.image.as_mut() else {
            continue; // no columns: no pixels
        };
        let base = i as usize * n * 3;
        for j in 0..n {
            for z in 0..3 {
                unread |= !filled[z][j];
                pix[base + 3 * j + z] = planes[z][j] as u8;
            }
        }
    }
    if unread {
        g.notices.add(
            &FS_727,
            "an SGI image's pixels never read: FreeSurfer's are memory contents (0 here)",
        );
    }
    rgb.close_read();
    Ok(Some(im))
}

/// `mri_read`'s image branch: [`image_read`], then `ImageToMRI` (fs.volume's, through
/// [`upper`]) and `ImageFree`. FS-718 when the reader fails.
pub fn read_as_volume(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Mri>, Fatal> {
    let mut i = match image_read(g, out, err, dir, fname)? {
        Read::Image(i) => i,
        Read::NotPorted => return Ok(None),
        Read::Null => {
            null_image(g, err, fname)?;
            return Ok(None);
        }
    };
    let Some(conv) = upper::get().image_to_mri else {
        upper::missing(
            err,
            "reading image volumes",
            "fs.volume",
            "ImageToMRI",
            fname,
        );
        return Ok(None);
    };
    let m = conv(g, out, err, &mut i)?;
    image_free(g, err, Some(i))?;
    Ok(Some(m))
}

/// FS-718: the null image `ImageRead` gave, converted.
fn null_image(g: &mut Globals, err: &mut dyn Write, fname: &str) -> Result<(), Fatal> {
    g.notices.add(
        &FS_718,
        "mri_read of an image whose reader failed: FreeSurfer converts a null image (a crash)",
    );
    if g.notices.reproduce() {
        return Err(Fatal::Exit(139));
    }
    g.errno = 0;
    log::error_printf(
        g,
        err,
        ERROR_BADFILE,
        &format!("mri_read(): could not read the image {fname}"),
    );
    Ok(())
}

/// What [`image_f_write`] did: FreeSurfer's result (always 0) and whether the image was written.
struct Wrote {
    ok: bool,
    code: i32,
}

/// `ImageWrite(I, fname)`: `fname` opened for writing (`ImageWrite(<name>) failed` and -1 when it
/// cannot be), then [`image_f_write`] (the format's writer opens its own file, the name without
/// its `#` part), the first file closed; 0. `None` is FreeSurfer's null image (FS-732's
/// `reproduce`).
fn image_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    im: Option<&Image>,
    fname: &str,
) -> Result<Wrote, Fatal> {
    let fp = match CFile::open(dir, fname, "wb") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_printf(
                g,
                err,
                ERROR_NO_FILE,
                &format!("ImageWrite({fname}) failed\n"),
            );
            return Ok(Wrote {
                ok: false,
                code: -1,
            });
        }
    };
    let w = image_f_write(g, out, err, dir, im, fname)?;
    let _ = fp.close();
    Ok(w)
}

/// `ImageFWrite(I, fp, fname)`: by the type of the name ([`image_unpack_file_name`]): SGI images
/// (`RGBwrite`), TIFF (not ported yet), PGM and PPM (`pgm not supported`, `ppm not supported`),
/// HIPS (its stubs: the `endian` parameter's look-up and setting, the header and each frame, each a
/// message; nothing written); every other type (JPEG, MATLAB, PBM) the JPEG writer, which takes
/// images of bytes only (`JPEGWrite: only PFBYTE currently supported`).
fn image_f_write(
    g: &mut Globals,
    _out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    im: Option<&Image>,
    fname: &str,
) -> Result<Wrote, Fatal> {
    let (buf, _frame, ty) = image_unpack_file_name(g, fname);
    let refused = |g: &mut Globals, err: &mut dyn Write, msg: &str| {
        log::error_printf(g, err, ERROR_UNSUPPORTED, msg);
        Wrote {
            ok: false,
            code: ERROR_UNSUPPORTED,
        }
    };
    match ty {
        RGBI_IMAGE => {
            let Some(im) = im else {
                return Err(Fatal::Exit(139)); // I->cols of the null image
            };
            rgb_write(g, err, dir, im, &buf)
        }
        TIFF_IMAGE => {
            not_ported(err, "writing TIFF images", &buf);
            Ok(Wrote {
                ok: false,
                code: -1,
            })
        }
        PGM_IMAGE => Ok(refused(g, err, "pgm not supported")),
        PPM_IMAGE => Ok(refused(g, err, "ppm not supported")),
        HIPS_IMAGE => {
            hips(err); // findparam(I, "endian")
            hips(err); // setparam(I, "endian", ...)
            hips(err); // fwrite_header
            let Some(im) = im else {
                return Err(Fatal::Exit(139)); // I->image of the null image
            };
            for _ in 0..im.num_frame.max(0) {
                hips(err); // fwrite_image
            }
            Ok(Wrote {
                ok: false,
                code: ERROR_UNSUPPORTED,
            })
        }
        _ => {
            let Some(im) = im else {
                return Err(Fatal::Exit(139)); // I->pixel_format of the null image
            };
            if im.pixel_format != PFBYTE {
                return Ok(refused(
                    g,
                    err,
                    "JPEGWrite: only PFBYTE currently supported",
                ));
            }
            not_ported(err, "writing JPEG images", &buf);
            Ok(Wrote {
                ok: false,
                code: -1,
            })
        }
    }
}

/// `RGBwrite(I, fname, frame)`: an uncompressed SGI image of 1-byte pixels, `I->cols` by
/// `I->rows`, of one plane (FreeSurfer's Linux build), each row written three times (planes 1 and
/// 2 are plane 0 in an image of two dimensions): each pixel's first byte (`IMAGEpix`, whatever the
/// pixel format). FS-730: sizes beyond 65,535 are refused by default.
fn rgb_write(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    im: &Image,
    fname: &str,
) -> Result<Wrote, Fatal> {
    if im.cols > 65535 || im.rows > 65535 {
        g.notices.add(
            &FS_730,
            format!(
                "an SGI image of {} x {} pixels: FreeSurfer writes its sizes modulo 65,536",
                im.cols, im.rows
            ),
        );
        if g.notices.fix() {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "RGBwrite({fname}): an image of {} x {} pixels does not fit an SGI image (at most 65535 x 65535)",
                    im.cols, im.rows
                ),
            );
            return Ok(Wrote {
                ok: false,
                code: ERROR_BADPARM,
            });
        }
    }
    let (rows, cols) = (im.rows.max(0), im.cols.max(0));
    let mut img = RgbImage::open_write(
        g,
        err,
        dir,
        fname,
        SGI_UNCOMPRESSED_1,
        2,
        cols as u32,
        rows as u32,
        1,
    )?;
    let n = cols as usize;
    let mut r = vec![0u16; n];
    let pix = im.image.as_deref().unwrap_or(&[]);
    let ocols = i64::from(im.ocols);
    for y in 0..rows {
        for (x, v) in r.iter_mut().enumerate() {
            let o = i64::from(y) * ocols + x as i64;
            *v = u16::from(pix.get(o as usize).copied().unwrap_or(0));
        }
        for z in 0..3 {
            img.putrow(g, err, &r, y as u32, z)?;
        }
    }
    img.close_write(g, err)?;
    Ok(Wrote {
        ok: true,
        code: NO_ERROR,
    })
}

/// `MRIwriteType`'s image branch (the volume's depth already checked): `MRItoImage(mri, NULL, 0)`
/// (fs.volume's, through [`upper`]), [`image_write`], `ImageFree`; 0 (FS-731: the default returns
/// the error when the image was not written; FS-732 for a volume `MRItoImage` refuses).
pub fn write_from_volume(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    m: &Mri,
    fname: &str,
) -> Result<i32, Fatal> {
    let Some(conv) = upper::get().mri_to_image else {
        upper::missing(
            err,
            "writing image volumes",
            "fs.volume",
            "MRItoImage",
            fname,
        );
        return Ok(-1);
    };
    let Some(image) = conv(g, err, m, 0)? else {
        g.notices.add(
            &FS_732,
            "an image written of a volume MRItoImage refuses: FreeSurfer goes on with a null image (a crash, or ImageFree's exit)",
        );
        if g.notices.fix() {
            return Ok(ERROR_UNSUPPORTED);
        }
        image_write(g, out, err, dir, None, fname)?;
        image_free(g, err, None)?;
        return Ok(0);
    };
    let w = image_write(g, out, err, dir, Some(&image), fname)?;
    image_free(g, err, Some(image))?;
    if !w.ok {
        g.notices.add(
            &FS_731,
            "an image not written: FreeSurfer's MRIwriteType returns 0",
        );
        if g.notices.fix() {
            return Ok(w.code);
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn unpack_names() {
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let mut u = |n: &str| image_unpack_file_name(&mut g, n);
        assert_eq!(u("a.rgb#2"), ("a.rgb".into(), 2, RGBI_IMAGE));
        assert_eq!(u("a#1.rgb"), ("a".into(), 1, RGBI_IMAGE));
        assert_eq!(u("w/x.dat"), ("w/x.dat".into(), -1, HIPS_IMAGE));
        assert_eq!(u("x.MAT"), ("x.MAT".into(), -1, MATLAB_IMAGE));
        assert_eq!(u("x.tiff#q"), ("x.tiff".into(), -1, TIFF_IMAGE));
        assert_eq!(u("x.Jpeg"), ("x.Jpeg".into(), -1, JPEG_IMAGE));
        assert_eq!(u("noext"), ("noext".into(), -1, HIPS_IMAGE));
        assert_eq!(u("p.pbm#-3"), ("p.pbm".into(), -3, PBM_IMAGE));
    }

    /// An SGI image written and read back: the header's fields in their byte orders, the rows
    /// three times at one place, grey pixels read back.
    #[test]
    fn sgi_round_trip() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut g = Globals::new(mode);
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let mut im = image_alloc(&mut g, &mut err, 2, 3, PFBYTE, 1, 0).unwrap();
            im.image = Some(vec![5, 9, 200, 0, 7, 255]);
            let w = image_write(
                &mut g,
                &mut out,
                &mut err,
                Some(dir.path()),
                Some(&im),
                "o.rgb",
            )
            .unwrap();
            assert!(w.ok);
            let b = std::fs::read(dir.path().join("o.rgb")).unwrap();
            assert_eq!(b.len(), 512 + 6);
            assert_eq!(&b[0..12], &[0x01, 0xda, 0, 1, 0, 2, 0, 3, 0, 2, 0, 1]);
            assert_eq!(&b[12..20], &[0, 0, 0, 0, 0, 0, 0, 255]);
            assert_eq!(&b[24..32], b"no name\0");
            assert_eq!(&b[112..114], &[2, 0]);
            assert_eq!(&b[118..120], &[1, 0]);
            assert!(b[176..512].iter().all(|&x| x == 0));
            assert_eq!(&b[512..], &[5, 9, 200, 0, 7, 255]);
            let r = rgb_read_image(&mut g, &mut err, Some(dir.path()), "o.rgb")
                .unwrap()
                .unwrap();
            assert_eq!(r.pixel_format, PFRGB);
            assert_eq!(
                r.image.unwrap(),
                vec![
                    5, 5, 5, 9, 9, 9, 200, 200, 200, 0, 0, 0, 7, 7, 7, 255, 255, 255
                ]
            );
        }
    }

    /// FS-730: an image wider than 65,535 pixels: refused by default, written modulo 65,536 by
    /// `reproduce` (a header-only file: the rows have 0 bytes).
    #[test]
    fn sgi_too_wide() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut g = Globals::new(mode);
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let mut im = image_alloc(&mut g, &mut err, 1, 65536, PFBYTE, 1, 0).unwrap();
            im.image = Some(vec![1; 65536]);
            let name = format!("w{}.rgb", mode.name());
            let w = image_write(
                &mut g,
                &mut out,
                &mut err,
                Some(dir.path()),
                Some(&im),
                &name,
            )
            .unwrap();
            let b = std::fs::read(dir.path().join(&name)).unwrap();
            if mode.fix() {
                assert!(!w.ok && w.code == ERROR_BADPARM);
                assert!(b.is_empty(), "only ImageWrite's own empty file");
            } else {
                assert!(w.ok);
                assert_eq!(b.len(), 176);
                assert_eq!(&b[6..8], &[0, 0], "65,536 stored as 0");
            }
        }
    }
}
