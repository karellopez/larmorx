// SPDX-License-Identifier: Apache-2.0
//! Slices as 2-D images (spec §34, part 7b): FreeSurfer's HIPS image header (`IMAGE`) and its
//! allocation, `MRItoImageView`, `MRItoImage` and `ImageToMRI`; FS-465 and FS-466.

use larmorx_core::defects::{FS_465, FS_466};

use super::geometry::{MRI_AXIAL, MRI_CORONAL, MRI_SAGITTAL, MRI_UNDEFINED};
use super::sample::{FLT_EPSILON, cvtt};
use crate::mri::{Mri, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// The pixel formats (§34).
pub mod pixel_format {
    pub const PFBYTE: i32 = 0;
    pub const PFSHORT: i32 = 1;
    pub const PFINT: i32 = 2;
    pub const PFFLOAT: i32 = 3;
    pub const PFCOMPLEX: i32 = 4;
    pub const PFDOUBLE: i32 = 6;
    pub const PFRGB: i32 = 35;
    pub const PFRGBZ: i32 = 36;
}
use pixel_format::*;

/// The bytes of one pixel of a format (§34); a format outside the spec's list: one byte.
pub fn bytes_per_pixel(format: i32) -> usize {
    match format {
        PFSHORT => 2,
        PFINT | PFFLOAT | PFRGBZ => 4,
        PFDOUBLE => 8,
        PFRGB => 3,
        _ => 1,
    }
}

/// An image (`IMAGE`, §34): the stored rows and columns, the region's rows and columns, the
/// pixel format, the frame count, the pixel sizes and the pixels' buffer (none for an image of no
/// pixels). The header's numbers may count more than the buffer holds (FS-465).
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub orows: i32,
    pub ocols: i32,
    pub rows: i32,
    pub cols: i32,
    pub pixel_format: i32,
    pub num_frame: i32,
    pub xsize: f32,
    pub ysize: f32,
    pub pixels: Option<Vec<u8>>,
}

/// Which image a function returned: none (`NULL`), the given one, or one it made (a given image
/// it did not keep is freed).
#[derive(Debug)]
pub enum ImageOut {
    Nothing,
    Given,
    New(Image),
}

/// `ImageAlloc(rows, cols, format, frames)` (§34): `orows = rows`, `ocols = cols`, a zeroed buffer
/// of `rows cols (bytes per pixel) frames` bytes (none when `rows cols` is 0).
pub fn image_alloc(rows: i32, cols: i32, format: i32, frames: i32) -> Image {
    let n = i64::from(rows) * i64::from(cols);
    let pixels = (n != 0).then(|| {
        let len = n.max(0) as usize * bytes_per_pixel(format) * frames.max(0) as usize;
        vec![0u8; len]
    });
    Image {
        orows: rows,
        ocols: cols,
        rows,
        cols,
        pixel_format: format,
        num_frame: frames,
        xsize: 1.0,
        ysize: 1.0,
        pixels,
    }
}

/// Where image pixel `(x', y')` and the slice put a voxel (§34 step 2's last column).
#[derive(Clone, Copy)]
enum ViewMap {
    /// `(x', y', slice)`
    XYS,
    /// `(slice, y', x')`
    SYX,
    /// `(x', slice, y')`
    XSY,
    /// `(y', slice, x')`
    YSX,
    /// `(slice, x', y')`
    SXY,
}

impl ViewMap {
    fn voxel(self, x: i32, y: i32, s: i32) -> (i32, i32, i32) {
        match self {
            ViewMap::XYS => (x, y, s),
            ViewMap::SYX => (s, y, x),
            ViewMap::XSY => (x, s, y),
            ViewMap::YSX => (y, s, x),
            ViewMap::SXY => (s, x, y),
        }
    }
}

/// §34 step 2: the view of a slice direction (`w`, `h`, `d`, columns against, rows against, the
/// voxel map), all 0 for a view that is none of the three.
struct View {
    w: i32,
    h: i32,
    d: i32,
    cols_against: bool,
    rows_against: bool,
    map: ViewMap,
}

fn view_of(m: &Mri, dir: i32, view: i32) -> View {
    let g = &m.geom;
    let (wd, ht, dp) = (g.width, g.height, g.depth);
    let (x_r, x_a) = (g.x[0], g.x[1]);
    let (y_a, y_s) = (g.y[1], g.y[2]);
    let (z_r, z_a, z_s) = (g.z[0], g.z[1], g.z[2]);
    use ViewMap::*;
    let (w, h, d, ca, ra, map) = match (dir, view) {
        (MRI_CORONAL, MRI_CORONAL) => (wd, ht, dp, x_r > 0.0, y_s > 0.0, XYS),
        (MRI_CORONAL, MRI_SAGITTAL) => (dp, ht, wd, !(z_a > 0.0), y_s > 0.0, SYX),
        (MRI_CORONAL, MRI_AXIAL) => (wd, dp, ht, x_r > 0.0, !(z_a > 0.0), XSY),
        (MRI_SAGITTAL, MRI_CORONAL) => (dp, ht, wd, z_r > 0.0, y_s > 0.0, SYX),
        (MRI_SAGITTAL, MRI_SAGITTAL) => (wd, ht, dp, !(x_a > 0.0), y_s > 0.0, XYS),
        (MRI_SAGITTAL, MRI_AXIAL) => (dp, wd, ht, z_r > 0.0, !(x_a > 0.0), YSX),
        (MRI_AXIAL, MRI_CORONAL) => (wd, dp, ht, x_r > 0.0, z_s > 0.0, XSY),
        (MRI_AXIAL, MRI_SAGITTAL) => (ht, dp, wd, !(y_a > 0.0), z_s > 0.0, SXY),
        (MRI_AXIAL, MRI_AXIAL) => (wd, ht, dp, x_r > 0.0, !(y_a > 0.0), XYS),
        _ => (0, 0, 0, false, false, XYS),
    };
    View {
        w,
        h,
        d,
        cols_against: ca,
        rows_against: ra,
        map,
    }
}

impl View {
    /// The voxel image pixel `(x, y)` reads in slice `s`.
    fn voxel(&self, x: i32, y: i32, s: i32) -> (i32, i32, i32) {
        let xp = if self.cols_against { self.w - 1 - x } else { x };
        let yp = if self.rows_against { self.h - 1 - y } else { y };
        self.map.voxel(xp, yp, s)
    }
}

impl Process {
    /// FS-465: a write of the image's bytes from byte `at`, only the bytes inside its buffer.
    fn image_write(&mut self, img: &mut Image, at: i64, b: &[u8], function: &str) {
        let len = img.pixels.as_ref().map_or(0, Vec::len) as i64;
        let inside = at >= 0 && at + b.len() as i64 <= len;
        if !inside {
            self.note(
                &FS_465,
                format!("{function}: the bytes past the image's pixels not written (FreeSurfer writes past them)"),
            );
        }
        if let Some(p) = img.pixels.as_mut() {
            for (k, &v) in b.iter().enumerate() {
                let o = at + k as i64;
                if (0..len).contains(&o) {
                    p[o as usize] = v;
                }
            }
        }
    }

    /// FS-465: `n` bytes of the image from byte `at`, those past its buffer read as 0.
    fn image_read(&mut self, img: &Image, at: i64, n: usize, function: &str) -> [u8; 4] {
        let mut out = [0u8; 4];
        let len = img.pixels.as_ref().map_or(0, Vec::len) as i64;
        if !(at >= 0 && at + n as i64 <= len) {
            self.note(
                &FS_465,
                format!("{function}: the pixels past the image's buffer read as 0 (FreeSurfer reads past them)"),
            );
        }
        if let Some(p) = img.pixels.as_ref() {
            for (k, o) in out.iter_mut().enumerate().take(n) {
                let at = at + k as i64;
                if (0..len).contains(&at) {
                    *o = p[at as usize];
                }
            }
        }
        out
    }

    /// `MRItoImageView(mri, I, slice, view, frame)` (§34): the slice of the frame in the view as a
    /// byte image scaled to the slice's extremes (steps 1 to 8). A given image is kept when it has
    /// the view's rows and columns and is of bytes, else a new one is made (the given one freed).
    pub fn mri_to_image_view(
        &mut self,
        m: &Mri,
        given: Option<&mut Image>,
        slice: i32,
        view: i32,
        frame: i32,
    ) -> Result<ImageOut, Stop> {
        let dir = self.get_slice_direction(&m.geom);
        if dir == MRI_UNDEFINED {
            self.error_printf(
                codes::UNSUPPORTED,
                format!("MRItoImageView({slice}, {view}): unsupported view/slice direction {dir}")
                    .as_bytes(),
            );
            return Ok(ImageOut::Nothing);
        }
        let v = view_of(m, dir, view);
        let mut s = slice;
        if slice < 0 {
            let frac: f32 = if view == dir { 0.4 } else { 0.6 };
            s = self.nint(sse::widen(sse::mul32(frac, v.d as f32)));
        } else if slice >= v.d {
            self.error_printf(
                codes::BAD_PARM,
                format!("MRItoImageView: bad slice {slice}\n").as_bytes(),
            );
            return Ok(ImageOut::Nothing);
        }
        let keep =
            matches!(&given, Some(i) if i.rows == v.h && i.cols == v.w && i.pixel_format == PFBYTE);
        let mut own = None;
        let img: &mut Image = match given {
            Some(i) if keep => i,
            _ => own.insert(image_alloc(v.h, v.w, PFBYTE, 1)),
        };
        let g = &m.geom;
        (img.xsize, img.ysize) = if dir == MRI_CORONAL {
            match view {
                MRI_CORONAL => (g.xsize, g.ysize),
                MRI_SAGITTAL => (g.zsize, g.ysize),
                MRI_AXIAL => (g.xsize, g.zsize),
                _ => (1.0, 1.0),
            }
        } else {
            (1.0, 1.0)
        };
        let (mut fmin, mut fmax) = (1e7f32, -1e7f32);
        for y in 0..v.h {
            for x in 0..v.w {
                let val = self.view_sample(m, &v, x, y, s, frame)?;
                if val > sse::widen(fmax) {
                    fmax = sse::narrow(val);
                }
                if val < sse::widen(fmin) {
                    fmin = sse::narrow(val);
                }
            }
        }
        let range = sse::sub32(fmax, fmin);
        if sse::widen(range).abs() < FLT_EPSILON {
            self.error_printf(codes::BAD_PARM, b"MRItoImageView: constant image");
        } else {
            let ocols = i64::from(img.ocols);
            for y in 0..v.h {
                for x in 0..v.w {
                    let val = self.view_sample(m, &v, x, y, s, frame)?;
                    let q = sse::div64(
                        sse::mul64(255.0, sse::sub64(val, sse::widen(fmin))),
                        sse::widen(range),
                    );
                    let at = i64::from(v.h - (y + 1)) * ocols + i64::from(x);
                    self.image_write(img, at, &[cvtt(q) as u8], "MRItoImageView");
                }
            }
        }
        Ok(match own {
            Some(i) => ImageOut::New(i),
            None => ImageOut::Given,
        })
    }

    /// One pixel's sample of §34 step 5 (`MRIsampleVolumeFrame` at the pixel's voxel; 0 where the
    /// sampler writes nothing).
    fn view_sample(
        &mut self,
        m: &Mri,
        v: &View,
        x: i32,
        y: i32,
        s: i32,
        frame: i32,
    ) -> Result<f64, Stop> {
        let (vx, vy, vz) = v.voxel(x, y, s);
        let mut val = 0.0f64;
        self.mri_sample_volume_frame(
            m,
            f64::from(vx),
            f64::from(vy),
            f64::from(vz),
            frame,
            &mut val,
        )?;
        Ok(val)
    }

    /// `MRItoImage(mri, I, slice)` (§34): slice `slice` of frame 0 copied row by row (bytes,
    /// 32-bit integers and floats as they are) into an image that runs upwards.
    pub fn mri_to_image(
        &mut self,
        m: &Mri,
        given: Option<&mut Image>,
        slice: i32,
    ) -> Result<ImageOut, Stop> {
        let g = &m.geom;
        if slice < 0 || slice >= g.depth {
            self.error_printf(
                codes::BAD_PARM,
                format!("MRItoImage: bad slice {slice}\n").as_bytes(),
            );
            return Ok(ImageOut::Nothing);
        }
        let mut own = None;
        let img: &mut Image = match given {
            Some(i) => i,
            None => {
                let f = match m.kind {
                    types::INT => PFINT,
                    types::FLOAT => PFFLOAT,
                    _ => PFBYTE,
                };
                own.insert(image_alloc(g.height, g.width, f, 1))
            }
        };
        img.xsize = g.xsize;
        img.ysize = g.ysize;
        let es: i64 = match m.kind {
            types::UCHAR => 1,
            types::INT | types::FLOAT => 4,
            _ => 0,
        };
        for y in 0..g.height {
            if es == 0 {
                self.error_printf(
                    codes::UNSUPPORTED,
                    format!("MRItoImage: unsupported type {}", m.kind).as_bytes(),
                );
                return Ok(ImageOut::Nothing);
            }
            self.direct_needs_voxels(m, "MRItoImage")?;
            let n = i64::from(g.width) * es;
            let place = self.row_place(m, 0, i64::from(y), i64::from(slice), 0, n.max(0) as u64);
            let row = self.direct_read(m, place, n.max(0) as usize, "MRItoImage");
            let at = i64::from(g.height - (y + 1)) * i64::from(img.ocols) * es;
            self.image_write(img, at, &row, "MRItoImage");
        }
        Ok(match own {
            Some(i) => ImageOut::New(i),
            None => ImageOut::Given,
        })
    }

    /// `ImageToMRI(I)` (§34): a volume of the image's `ocols x orows x 1` voxels and its frames,
    /// the image running upwards; the pixel sizes below `1e-15` become 1 in the image itself.
    pub fn image_to_mri(&mut self, img: &mut Image) -> Result<Option<Mri>, Stop> {
        let kind = match img.pixel_format {
            PFBYTE => types::UCHAR,
            PFSHORT | PFINT => types::INT,
            PFFLOAT => types::FLOAT,
            PFRGB => {
                self.put_out(b"reading RGB image....\n");
                types::RGB
            }
            f => {
                self.error_exit(
                    codes::BAD_PARM,
                    format!("IMAGE type = {f} not supported\n").as_bytes(),
                )?;
                return Ok(None);
            }
        };
        let (w, h) = (img.ocols, img.orows);
        let mut m = if img.num_frame <= 1 {
            self.mri_alloc(w, h, 1, kind, 1, true)?
        } else {
            self.mri_alloc_sequence(w, h, 1, kind, img.num_frame)?
        };
        for s in [&mut img.xsize, &mut img.ysize] {
            if sse::widen(*s).abs() < 1e-15 {
                *s = 1.0;
            }
        }
        self.mri_set_resolution(&mut m, img.xsize, img.ysize, 1.0)?;
        m.nframes = img.num_frame;
        m.imnr0 = 1;
        m.imnr1 = 1;
        let half = |n: i32, size: f32| {
            sse::narrow(sse::div64(sse::widen(sse::mul32(n as f32, size)), 2.0))
        };
        m.xend = half(m.geom.width, m.geom.xsize);
        m.xstart = -m.xend;
        m.yend = half(m.geom.height, m.geom.ysize);
        m.ystart = -m.yend;
        m.zend = half(m.geom.depth, m.geom.zsize);
        m.zstart = -m.zend;
        let (a, b) = (sse::sub32(m.xend, m.xstart), sse::sub32(m.yend, m.ystart));
        m.fov = if a > b { a } else { b };
        m.geom.x = [-1.0, 0.0, 0.0];
        m.geom.y = [0.0, 0.0, -1.0];
        m.geom.z = [0.0, 1.0, 0.0];
        m.geom.c = [0.0; 3];
        let rc = i64::from(img.rows.wrapping_mul(img.cols));
        let ocols = i64::from(img.ocols);
        let rgb_frame_bytes = if self.mode().reproduce() { rc } else { 3 * rc };
        if kind == types::RGB && m.nframes > 1 {
            if self.mode().reproduce() {
                self.note(
                    &FS_466,
                    "ImageToMRI: an RGB image's later frames read one byte per pixel from the start (overlapping the first)",
                );
            } else {
                self.note(
                    &FS_466,
                    "ImageToMRI: an RGB image's later frames read at their own pixels (FreeSurfer reads them one byte per pixel from the start)",
                );
            }
        }
        for f in 0..m.nframes {
            for y in 0..h {
                for x in 0..w {
                    let e = i64::from(h - (y + 1)) * ocols + i64::from(x);
                    let fi = i64::from(f);
                    match img.pixel_format {
                        PFRGB => {
                            let at = 3 * e + fi * rgb_frame_bytes;
                            let b = self.image_read(img, at, 3, "ImageToMRI");
                            let v =
                                i32::from(b[0]) + 256 * i32::from(b[1]) + 65536 * i32::from(b[2]);
                            self.mri_set_vox_val(&mut m, x, y, 0, f, v as f32, 0)?;
                        }
                        fmt => {
                            let es = bytes_per_pixel(fmt);
                            let at = (e + fi * rc) * es as i64;
                            let b = self.image_read(img, at, es, "ImageToMRI");
                            let bytes: Vec<u8> = if fmt == PFSHORT {
                                i32::from(u16::from_le_bytes([b[0], b[1]]))
                                    .to_le_bytes()
                                    .to_vec()
                            } else {
                                b[..es].to_vec()
                            };
                            let place = self.place_of(
                                &m,
                                i64::from(x),
                                i64::from(y),
                                0,
                                fi,
                                bytes.len() as u64,
                            );
                            self.direct_write(&mut m, place, &bytes, "ImageToMRI");
                        }
                    }
                }
            }
        }
        Ok(Some(m))
    }
}
