// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Slices of a volume as 2-D images and back (`utils/mri.cpp`, 3.1-35's image part):
//! `MRItoImageView`, `MRItoImage` and `ImageToMRI`, on fs.io's `IMAGE`
//! ([`larmorx_freesurfer_io::image`]).
//!
//! An image's pixel `(x, y)` is element `y * ocols + x` of its buffer (FreeSurfer's `IMAGEpix`
//! family; an RGB pixel is the 3 bytes at `3 (y * ocols + x)`); image row 0 is the volume's last row
//! (`yp = height - (y + 1)`: the image's rows run upwards).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §12):
//! - **FS-465** (memory): a given image smaller than the slice (or of a wider pixel format than the
//!   one the slice needs), or an image whose frames or pixels are fewer than its header counts, is
//!   written or read past its pixels; both modes write only the bytes inside it and read 0
//!   outside, noted.
//! - **FS-466**: an RGB image's frames after the first are read at `frame * rows * cols` bytes from
//!   the start, not 3 bytes per pixel: they overlap the first frame; the default reads each frame's
//!   own pixels.

use std::io::Write;

use larmorx_core::defects::{FS_465, FS_466};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{div64, mul, mul64, sub, sub64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_io::image::{Image, PFBYTE, PFFLOAT, PFINT, PFRGB, PFSHORT, image_alloc};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_RGB, MRI_UCHAR, Mri, mri_alloc, mri_alloc_sequence,
};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;

use crate::coords::{MRI_CORONAL, MRI_SAGITTAL, MRI_UNDEFINED, get_slice_direction};
use crate::resize::{MRI_HORIZONTAL, mri_set_resolution};
use crate::rows::{self, Num, need_voxels};
use crate::sample::sample_volume_frame;

/// Writes `b` at byte `off` of the image's pixels; the bytes outside them are dropped (FS-465).
fn put(g: &mut Globals, im: &mut Image, off: i64, b: &[u8], what: &str) {
    let len = im.image.as_ref().map_or(0, Vec::len) as i64;
    let mut outside = false;
    for (k, &v) in b.iter().enumerate() {
        let o = off + k as i64;
        if (0..len).contains(&o) {
            im.image.as_mut().expect("pixels")[o as usize] = v;
        } else {
            outside = true;
        }
    }
    if outside {
        g.notices.add(
            &FS_465,
            format!("{what} writes past an image's pixels (the image is smaller than the slice)"),
        );
    }
}

/// The `n` bytes at byte `off` of the image's pixels; 0 for bytes outside them (FS-465).
fn get<const N: usize>(g: &mut Globals, im: &Image, off: i64, what: &str) -> [u8; N] {
    let buf = im.image.as_deref().unwrap_or(&[]);
    let mut r = [0u8; N];
    let mut outside = false;
    for (k, x) in r.iter_mut().enumerate() {
        let o = off + k as i64;
        if o >= 0 && (o as usize) < buf.len() {
            *x = buf[o as usize];
        } else {
            outside = true;
        }
    }
    if outside {
        g.notices.add(
            &FS_465,
            format!("{what} reads past an image's pixels (its frames or pixels are fewer than its header counts)"),
        );
    }
    r
}

/// `(ubyte)` of a `double` as the official build converts it (`cvttsd2si` to 32 bits, the low
/// byte): here always within [0, 255] or NaN (0).
fn to_ubyte(v: f64) -> u8 {
    let i = if v.is_nan() || !(-2147483649.0..2147483648.0).contains(&v) {
        i32::MIN
    } else {
        v as i32
    };
    i as u8
}

/// `MRItoImageView(mri, I, slice, view, frame)`: the volume's principal slice direction
/// (`getSliceDirection`) must be coronal, sagittal or horizontal, else the error
/// `MRItoImageView(<slice>, <view>): unsupported view/slice direction <d>` (`ERROR_UNSUPPORTED`)
/// and `NULL`. The view picks the image's extent `w x h` and the slices' axis `d` from the
/// volume's (a view that is none of the three: 0 x 0 and no slices) and, from the direction cosines,
/// whether each image axis runs along or against the volume's; a negative slice is `nint(frac *
/// d)` (`frac` 0.4 when the view is the volume's own direction, else 0.6, in `float`), one at or
/// past `d` the error `MRItoImageView: bad slice <s>` (`ERROR_BADPARM`) and `NULL`. A given image
/// of another size or format is replaced by a new byte image (`ImageAlloc(h, w, PFBYTE, 1)`); its
/// `xsize`, `ysize` become the volume's in-plane voxel sizes for a coronal volume, 1 otherwise. Each
/// pixel samples the frame at the voxel (`MRIsampleVolumeFrame`, whole coordinates: the nearest
/// voxel); the smallest and largest samples (as `float`s, from 1e7 and -1e7) scale the samples to
/// bytes, `(ubyte)(255.0 * (val - min) / (max - min))` (`double`; `max - min` in `float`). A
/// range within `FLT_EPSILON` of 0: the error `MRItoImageView: constant image` and the image as it
/// is.
#[allow(clippy::too_many_arguments)]
pub fn mri_to_image_view(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    given: Option<&mut Image>,
    slice: i32,
    view: i32,
    frame: i32,
) -> Result<Ret<Image>, Fatal> {
    let dir = get_slice_direction(&mri.vg);
    let unsupported = |g: &mut Globals, err: &mut dyn Write| {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("MRItoImageView({slice}, {view}): unsupported view/slice direction {dir}"),
        );
    };
    if dir == MRI_UNDEFINED {
        unsupported(g, err);
        return Ok(Ret::Null);
    }
    let vg = &mri.vg;
    let (width, height, depth) = (vg.width, vg.height, vg.depth);
    let (mut w, mut h, mut d) = (0, 0, 0);
    let (mut xres, mut yres) = (1.0f32, 1.0f32);
    let (mut xsign, mut ysign) = (1, 1);
    let mut frac = 0.6f32;
    let pos = |v: f32| v > 0.0;
    let s = |c: bool| if c { -1 } else { 1 };
    let t = |c: bool| if c { 1 } else { -1 };
    match (dir, view) {
        (MRI_CORONAL, MRI_CORONAL) => {
            frac = 0.4;
            (w, h, d) = (width, height, depth);
            (xres, yres) = (vg.xsize, vg.ysize);
            (xsign, ysign) = (s(pos(vg.x_r)), s(pos(vg.y_s)));
        }
        (MRI_CORONAL, MRI_SAGITTAL) => {
            (w, h, d) = (depth, height, width);
            (xres, yres) = (vg.zsize, vg.ysize);
            (xsign, ysign) = (t(pos(vg.z_a)), s(pos(vg.y_s)));
        }
        (MRI_CORONAL, MRI_HORIZONTAL) => {
            (w, h, d) = (width, depth, height);
            (xres, yres) = (vg.xsize, vg.zsize);
            (xsign, ysign) = (s(pos(vg.x_r)), t(pos(vg.z_a)));
        }
        (MRI_SAGITTAL, MRI_CORONAL) => {
            (w, h, d) = (depth, height, width);
            (xsign, ysign) = (s(pos(vg.z_r)), s(pos(vg.y_s)));
        }
        (MRI_SAGITTAL, MRI_SAGITTAL) => {
            frac = 0.4;
            (w, h, d) = (width, height, depth);
            (xsign, ysign) = (t(pos(vg.x_a)), s(pos(vg.y_s)));
        }
        (MRI_SAGITTAL, MRI_HORIZONTAL) => {
            (w, h, d) = (depth, width, height);
            (xsign, ysign) = (s(pos(vg.z_r)), t(pos(vg.x_a)));
        }
        (MRI_HORIZONTAL, MRI_CORONAL) => {
            (w, h, d) = (width, depth, height);
            (xsign, ysign) = (s(pos(vg.x_r)), s(pos(vg.z_s)));
        }
        (MRI_HORIZONTAL, MRI_SAGITTAL) => {
            (w, h, d) = (height, depth, width);
            (xsign, ysign) = (t(pos(vg.y_a)), s(pos(vg.z_s)));
        }
        (MRI_HORIZONTAL, MRI_HORIZONTAL) => {
            frac = 0.4;
            (w, h, d) = (width, height, depth);
            (xsign, ysign) = (s(pos(vg.x_r)), t(pos(vg.y_a)));
        }
        (MRI_CORONAL | MRI_SAGITTAL | MRI_HORIZONTAL, _) => {}
        _ => {
            unsupported(g, err);
            return Ok(Ret::Null);
        }
    }
    let slice = if slice < 0 {
        nint(g, f64::from(mul(frac, d as f32)))
    } else if slice >= d {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRItoImageView: bad slice {slice}\n"),
        );
        return Ok(Ret::Null);
    } else {
        slice
    };
    let format = PFBYTE;
    let given = given.filter(|i| i.rows == h && i.cols == w && i.pixel_format == format);
    let mut fresh = None;
    let im: &mut Image = match given {
        Some(i) => i,
        None => fresh.insert(image_alloc(g, err, h, w, format, 1, 0)?),
    };
    im.xsize = xres;
    im.ysize = yres;
    let coords = |x: i32, y: i32| {
        let fx = if xsign > 0 { x } else { w - 1 - x };
        let fy = if ysign > 0 { y } else { h - 1 - y };
        match (dir, view) {
            (MRI_CORONAL, MRI_CORONAL)
            | (MRI_SAGITTAL, MRI_SAGITTAL)
            | (MRI_HORIZONTAL, MRI_HORIZONTAL) => (fx, fy, slice),
            (MRI_CORONAL, MRI_SAGITTAL) | (MRI_SAGITTAL, MRI_CORONAL) => (slice, fy, fx),
            (MRI_CORONAL, MRI_HORIZONTAL) | (MRI_HORIZONTAL, MRI_CORONAL) => (fx, slice, fy),
            (MRI_SAGITTAL, MRI_HORIZONTAL) => (fy, slice, fx),
            (MRI_HORIZONTAL, MRI_SAGITTAL) => (slice, fx, fy),
            _ => (0, 0, 0),
        }
    };
    let mut val = 0.0f64;
    let mut sample = |g: &mut Globals, err: &mut dyn Write, x: i32, y: i32| {
        let (xm, ym, zm) = coords(x, y);
        sample_volume_frame(
            g,
            err,
            mri,
            f64::from(xm),
            f64::from(ym),
            f64::from(zm),
            frame,
            &mut val,
        );
        val
    };
    let (mut fmin, mut fmax) = (10_000_000.0f32, -10_000_000.0f32);
    for y in 0..h {
        for x in 0..w {
            let v = sample(g, err, x, y);
            if v > f64::from(fmax) {
                fmax = v as f32;
            }
            if v < f64::from(fmin) {
                fmin = v as f32;
            }
        }
    }
    let range = sub(fmax, fmin);
    let ret = |fresh: Option<Image>| match fresh {
        Some(i) => Ret::New(i),
        None => Ret::Given,
    };
    if f64::from(range).abs() < f64::from(f32::EPSILON) {
        log::error_printf(g, err, ERROR_BADPARM, "MRItoImageView: constant image");
        return Ok(ret(fresh));
    }
    for y in 0..h {
        for x in 0..w {
            let v = sample(g, err, x, y);
            let yp = h - (y + 1);
            let b = to_ubyte(div64(
                mul64(255.0, sub64(v, f64::from(fmin))),
                f64::from(range),
            ));
            let off = i64::from(yp) * i64::from(im.ocols) + i64::from(x);
            put(g, im, off, &[b], "MRItoImageView");
        }
    }
    Ok(ret(fresh))
}

/// `MRItoImage(mri, I, slice)`: slice `slice` of frame 0 as an image (`ImageAlloc(height, width,
/// format, 1)` when none is given: `PFBYTE` for bytes and any other type, `PFINT` for 32-bit
/// integers, `PFFLOAT` for floats), each volume row copied as it is into the image row `height -
/// (y + 1)`; the image's `xsize`, `ysize` the volume's. A slice outside `[0, depth)`: the error
/// `MRItoImage: bad slice <s>` (`ERROR_BADPARM`) and `NULL`; a volume of another type than bytes,
/// 32-bit integers and floats (with at least one row): the error `MRItoImage: unsupported type <t>`
/// (`ERROR_UNSUPPORTED`) and `NULL` (FreeSurfer frees the image, a given one included).
pub fn mri_to_image(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    given: Option<&mut Image>,
    slice: i32,
) -> Result<Ret<Image>, Fatal> {
    const WHAT: &str = "MRItoImage";
    let (width, height) = (mri.vg.width, mri.vg.height);
    if slice < 0 || slice >= mri.vg.depth {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRItoImage: bad slice {slice}\n"),
        );
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    let im: &mut Image = match given {
        Some(i) => i,
        None => {
            let format = match mri.mtype {
                MRI_INT => PFINT,
                MRI_FLOAT => PFFLOAT,
                _ => PFBYTE,
            };
            fresh.insert(image_alloc(g, err, height, width, format, 1, 0)?)
        }
    };
    im.xsize = mri.vg.xsize;
    im.ysize = mri.vg.ysize;
    let size: i64 = match mri.mtype {
        MRI_INT | MRI_FLOAT => 4,
        MRI_UCHAR => 1,
        _ => 0,
    };
    for y in 0..height {
        let yp = height - (y + 1);
        if size == 0 {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("MRItoImage: unsupported type {}", mri.mtype),
            );
            return Ok(Ret::Null);
        }
        need_voxels(g, err, WHAT, mri)?;
        let n = (i64::from(width) * size).max(0) as usize;
        // a row pointer the volume does not have (no frames): FS-290, zeros
        let have = rows::touch(g, WHAT, mri, i64::from(slice), i64::from(y));
        let row = match rows::row_bytes_at(mri, i64::from(slice), i64::from(y), 0, n) {
            Some(o) if have => mri.chunk.as_ref().expect("voxels")[o..o + n].to_vec(),
            _ => vec![0; n],
        };
        let off = i64::from(yp) * i64::from(im.ocols) * size;
        put(g, im, off, &row, WHAT);
    }
    Ok(match fresh {
        Some(i) => Ret::New(i),
        None => Ret::Given,
    })
}

/// `ImageToMRI(I)`: a volume of `ocols x orows x 1` (`MRIalloc`, or `MRIallocSequence` with the
/// image's frames when it has more than one) of the type the pixel format gives (`PFBYTE` bytes,
/// `PFSHORT` and `PFINT` 32-bit integers, `PFFLOAT` floats, `PFRGB` RGB after `reading RGB
/// image....` on standard output; any other format ends the program, `IMAGE type = <f> not
/// supported`). An image `xsize` or `ysize` within 1e-15 of 0 becomes 1 (in the image); the
/// volume's voxel sizes are the image's and 1 (`MRIsetResolution`), its frames the image's count
/// (`nframes`, even 0), `imnr0 = imnr1 = 1`, the ends half of extent times size (`float`, then
/// halved in `double`) and the starts their negatives, `fov` the larger of the x and y spans, the
/// direction cosines coronal (`x_r = -1`, `z_a = 1`, `y_s = -1`) and the centre 0. Then each
/// frame's pixels (frame `f` at `f * rows * cols` elements) into the voxels, image row `height -
/// (y + 1)` into volume row `y`: bytes and floats as they are, shorts as unsigned 16-bit numbers,
/// ints as they are, RGB pixels as the 24-bit number of their 3 bytes (`MRIsetVoxVal`).
pub fn image_to_mri(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    im: &mut Image,
) -> Result<Mri, Fatal> {
    const WHAT: &str = "ImageToMRI";
    let (width, height, nframes) = (im.ocols, im.orows, im.num_frame);
    let ty = match im.pixel_format {
        PFBYTE => MRI_UCHAR,
        PFSHORT | PFINT => MRI_INT,
        PFFLOAT => MRI_FLOAT,
        PFRGB => {
            let _ = out.write_all(b"reading RGB image....\n");
            MRI_RGB
        }
        f => {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                &format!("IMAGE type = {f} not supported\n"),
            )?;
            MRI_UCHAR
        }
    };
    let mut mri = if nframes <= 1 {
        mri_alloc(g, out, err, width, height, 1, ty)?
    } else {
        mri_alloc_sequence(g, out, err, width, height, 1, ty, nframes)?
    };
    if f64::from(im.xsize).abs() < 1e-15 {
        im.xsize = 1.0;
    }
    if f64::from(im.ysize).abs() < 1e-15 {
        im.ysize = 1.0;
    }
    mri_set_resolution(g, out, err, &mut mri, im.xsize, im.ysize, 1.0)?;
    mri.nframes = nframes;
    mri.imnr0 = 1;
    mri.imnr1 = 1;
    let half = |n: i32, size: f32| (f64::from(mul(n as f32, size)) / 2.0) as f32;
    mri.xend = half(mri.vg.width, mri.vg.xsize);
    mri.xstart = -mri.xend;
    mri.yend = half(mri.vg.height, mri.vg.ysize);
    mri.ystart = -mri.yend;
    mri.zend = half(mri.vg.depth, mri.vg.zsize);
    mri.zstart = -mri.zend;
    let (sx, sy) = (sub(mri.xend, mri.xstart), sub(mri.yend, mri.ystart));
    mri.fov = if sx > sy { sx } else { sy };
    let vg = &mut mri.vg;
    (vg.x_r, vg.y_r, vg.z_r, vg.c_r) = (-1.0, 0.0, 0.0, 0.0);
    (vg.x_a, vg.y_a, vg.z_a, vg.c_a) = (0.0, 0.0, 1.0, 0.0);
    (vg.x_s, vg.y_s, vg.z_s, vg.c_s) = (0.0, -1.0, 0.0, 0.0);
    let pix_per_frame = im.rows.wrapping_mul(im.cols);
    let ocols = i64::from(im.ocols);
    for frame in 0..nframes {
        let frame_offset = i64::from(pix_per_frame.wrapping_mul(frame));
        for y in 0..height {
            let yp = i64::from(height - (y + 1));
            for x in 0..width {
                let e = yp * ocols + i64::from(x) + frame_offset;
                let x64 = i64::from(x);
                match mri.mtype {
                    MRI_RGB => {
                        // the pixel's 3 bytes (the 4-byte read's last byte is masked off)
                        let base = yp * ocols * 3 + x64 * 3;
                        if frame > 0 && pix_per_frame != 0 {
                            g.notices.add(
                                &FS_466,
                                "an RGB image's frames after the first are read 1 byte per pixel into it",
                            );
                        }
                        let off = if g.notices.reproduce() {
                            base + frame_offset
                        } else {
                            base + 3 * frame_offset
                        };
                        let b = get::<3>(g, im, off, WHAT);
                        let rgb = i32::from(b[0]) | i32::from(b[1]) << 8 | i32::from(b[2]) << 16;
                        mri_set_vox_val(g, err, &mut mri, x, y, 0, frame, rgb as f32, 0)?;
                    }
                    MRI_UCHAR => {
                        let b = get::<1>(g, im, e, WHAT);
                        let sl = i64::from(frame);
                        rows::write(
                            g,
                            WHAT,
                            &mut mri,
                            sl,
                            i64::from(y),
                            x64,
                            MRI_UCHAR,
                            Num::I(i64::from(b[0])),
                        );
                    }
                    MRI_INT => {
                        let v = if im.pixel_format == PFSHORT {
                            i64::from(u16::from_le_bytes(get::<2>(g, im, 2 * e, WHAT)))
                        } else {
                            i64::from(i32::from_le_bytes(get::<4>(g, im, 4 * e, WHAT)))
                        };
                        rows::write(
                            g,
                            WHAT,
                            &mut mri,
                            i64::from(frame),
                            i64::from(y),
                            x64,
                            MRI_INT,
                            Num::I(v),
                        );
                    }
                    _ => {
                        let v = f32::from_le_bytes(get::<4>(g, im, 4 * e, WHAT));
                        rows::write(
                            g,
                            WHAT,
                            &mut mri,
                            i64::from(frame),
                            i64::from(y),
                            x64,
                            MRI_FLOAT,
                            Num::F(v),
                        );
                    }
                }
            }
        }
    }
    Ok(mri)
}
