// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Resolution, planes, fills and type changes of `utils/mri.cpp` (3.1-36; `MRIlimits` from
//! 3.1-37, which `MRIchangeType` needs): `MRIsetResolution`, `MRIvalueFill`, `MRIfillFG`,
//! `MRIextractPlane`, `MRIfillPlane`, `MRIupsampleN`, `MRIupsample2`, `MRIupsampleNConserve`,
//! `MRIdownsample2`, `MRIdownsampleN`, `MRImeanFrameThresh`, `MRIlimits`, `MRIchangeType`,
//! `MRISeqchangeType`.
//!
//! The NaNs: where two NaN operands meet, the one the official build keeps is decided by its
//! operand order, read from the object code (`docs/findings/freesurfer-volume.md` §9) and followed
//! with SSE's rules ([`larmorx_freesurfer_core::sse`]).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §9):
//! - **FS-316** (rule A): `MRIfillFG` clamps a seed row or slice at or past the volume's height or
//!   depth to the volume's width minus one. The default clamps it to the height or depth minus one.
//! - **FS-317** (D13): `MRIdownsampleN`'s extent and windows and `MRIchangeType`'s histogram bins and
//!   counts convert floats beyond the `int` range (or NaN) to `int`: undefined, x86-64 gives
//!   `INT_MIN`. The default clamps (NaN to 0); `reproduce` gives `INT_MIN`. Noted.
//! - **FS-318** (rule A): `MRImeanFrameThresh` samples frame 0 whatever its frame. The default
//!   samples the frame.
//! - **FS-319** (crash): `MRIupsampleNConserve` of a volume `MRIchangeType` refuses to convert to
//!   floats goes on with no volume. An error in both modes.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_316, FS_317, FS_318, FS_319};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::matrix_identity;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, sub};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri, MriFrame, MriRegion,
    mri_alloc, mri_alloc_sequence, mri_copy_header,
};
use larmorx_freesurfer_mri::vol_geom::{
    extract_i_to_r, p0_to_cras, reinit_cache, set_vox2ras_from_matrix, xfm_crs2xyz,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::{Pair, mri_clear, mri_clone, mri_extract_into};
use crate::coords::sample_volume_nearest;
use crate::rows::{self, Num, need_voxels};
use crate::sample::sample_volume_frame_nearest;

/// `MRI_CORONAL`, `MRI_SAGITTAL`, `MRI_HORIZONTAL` (`mri.h`).
pub const MRI_CORONAL: i32 = 0;
pub const MRI_SAGITTAL: i32 = 1;
pub const MRI_HORIZONTAL: i32 = 2;

/// `N_HIST_BINS` (`mri.cpp`).
const N_HIST_BINS: i32 = 1000;

/// FreeSurfer's `MAX(a, b)`: `a >= b ? a : b` (`macros.h`; +0 over -0 when first, a NaN loses when
/// first and wins when second).
fn cmax(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// x86-64's `(int) x` of a value, with FS-317 for one beyond the `int` range or NaN: clamped by
/// default (NaN to 0), `INT_MIN` with `reproduce`.
fn to_int(g: &mut Globals, x: f64, what: &str) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
        g.notices.add(
            &FS_317,
            format!("{what}: {x} converted to an int, outside the int range"),
        );
        if g.notices.reproduce() {
            return i32::MIN;
        }
        if x.is_nan() {
            return 0;
        }
        return if x > 0.0 { i32::MAX } else { i32::MIN };
    }
    x.trunc() as i32
}

/// `MRIsetResolution(mri, xres, yres, zres)`: the voxel sizes; `ps` the mean of the three
/// (`(xres + yres) + zres` over 3, in `float`), the starts and ends scaled by their axis'
/// resolution, `thick` the largest (`MAX(MAX(xres, yres), zres)`, `a >= b ? a : b`); the cached
/// matrices recomputed. `NO_ERROR`.
pub fn mri_set_resolution(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &mut Mri,
    xres: f32,
    yres: f32,
    zres: f32,
) -> Result<i32, Fatal> {
    mri.ps = div(add(add(xres, yres), zres), 3.0);
    mri.vg.xsize = xres;
    mri.vg.ysize = yres;
    mri.vg.zsize = zres;
    // the official build's operands: the resolution first for xstart, ystart, zstart and xend
    // (one vector product), the field first for yend and zend
    mri.xstart = mul(xres, mri.xstart);
    mri.xend = mul(xres, mri.xend);
    mri.ystart = mul(yres, mri.ystart);
    mri.yend = mul(mri.yend, yres);
    mri.zstart = mul(zres, mri.zstart);
    mri.zend = mul(mri.zend, zres);
    mri.thick = cmax(cmax(xres, yres), zres);
    reinit_cache(g, out, err, &mut mri.vg)?;
    Ok(NO_ERROR)
}

/// `MRIvalueFill(mri, value)`: every voxel of every frame set to `value`: `(type) nint(value)` for
/// bytes, shorts, unsigned shorts and ints, the float itself for floats (its bits kept); other
/// types untouched. 0.
pub fn mri_value_fill(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    value: f32,
) -> Result<i32, Fatal> {
    let ty = mri.mtype;
    let written = matches!(ty, MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_INT | MRI_FLOAT);
    let (w, h, d, nf) = (mri.vg.width, mri.vg.height, mri.vg.depth, mri.nframes);
    if written && w > 0 && h > 0 && d > 0 && nf > 0 {
        need_voxels(g, err, "MRIvalueFill", mri)?;
    }
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                for f in 0..nf {
                    let v = match ty {
                        MRI_FLOAT => Num::F(value),
                        MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_INT => {
                            Num::I(i64::from(nint(g, f64::from(value))))
                        }
                        _ => continue,
                    };
                    let sl = i64::from(s) + i64::from(f) * i64::from(d);
                    rows::write(
                        g,
                        "MRIvalueFill",
                        mri,
                        sl,
                        i64::from(r),
                        i64::from(c),
                        ty,
                        v,
                    );
                }
            }
        }
    }
    Ok(0)
}

/// The source and destination of a function that may be given the same volume twice, and the
/// bytes written where the destination holds no voxel (FS-289, FS-290: FreeSurfer writes memory it
/// does not own and reads them back; kept here so the fill reads back what it wrote and ends).
struct Two<'a> {
    src: Option<&'a Mri>,
    dst: Option<&'a mut Mri>,
    fresh: Option<Mri>,
    shadow: std::collections::HashMap<(i32, i32, i32), i64>,
}

impl Two<'_> {
    fn d(&mut self) -> &mut Mri {
        match &mut self.dst {
            Some(d) => d,
            None => self.fresh.as_mut().expect("a destination"),
        }
    }
    fn s(&self) -> &Mri {
        match (self.src, &self.dst) {
            (Some(s), _) => s,
            (None, Some(d)) => d,
            (None, None) => self.fresh.as_ref().expect("a destination"),
        }
    }
    /// The destination's byte at (x, y, z) of frame 0 (`MRIvox`).
    fn dget(&mut self, g: &mut Globals, x: i32, y: i32, z: i32) -> i64 {
        let v = byte_at(g, self.d(), x, y, z);
        *self.shadow.get(&(x, y, z)).unwrap_or(&v)
    }
    /// The source's byte (the destination's when they are one volume).
    fn sget(&mut self, g: &mut Globals, x: i32, y: i32, z: i32) -> i64 {
        if self.src.is_none() {
            return self.dget(g, x, y, z);
        }
        byte_at(g, self.s(), x, y, z)
    }
    fn dput(&mut self, g: &mut Globals, x: i32, y: i32, z: i32, v: i64) {
        let reproduce = g.notices.reproduce();
        let d = self.d();
        let stored = match rows::at(d, i64::from(z), i64::from(y), i64::from(x), 1) {
            rows::At::In(_) => true,
            rows::At::Wrapped(_) => reproduce,
            rows::At::Outside => false,
        };
        put_byte(g, d, x, y, z, v);
        if stored {
            self.shadow.remove(&(x, y, z));
        } else {
            self.shadow.insert((x, y, z), v & 0xff);
        }
    }
}

fn byte_at(g: &mut Globals, m: &Mri, x: i32, y: i32, z: i32) -> i64 {
    match rows::read(
        g,
        "MRIfillFG",
        m,
        i64::from(z),
        i64::from(y),
        i64::from(x),
        MRI_UCHAR,
    ) {
        Num::I(v) => v,
        Num::F(_) => unreachable!("bytes"),
    }
}

fn put_byte(g: &mut Globals, m: &mut Mri, x: i32, y: i32, z: i32, v: i64) {
    rows::write(
        g,
        "MRIfillFG",
        m,
        i64::from(z),
        i64::from(y),
        i64::from(x),
        MRI_UCHAR,
        Num::I(v & 0xff),
    );
}

/// `MRIfillFG(mri_src, mri_dst, seed_x, seed_y, seed_z, threshold, fill_val, &npix)`: a flood fill
/// of frame 0's bytes (`MRIvox`, whatever the volume's type) from the seed (clamped to the volume:
/// FS-316) over the 6-connected voxels of the source at or above `threshold`: the destination (a
/// clone of the source when none is given: its voxels zero) has every byte equal to `fill_val`
/// lowered by one, the seed set to `fill_val`, then sweeps of a box that grows around the filled
/// voxels set each voxel at or above the threshold next to a filled one, until a sweep fills none.
/// The result and the number of voxels filled (the seed included).
#[allow(clippy::too_many_arguments)]
pub fn mri_fill_fg(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    seed_x: i32,
    seed_y: i32,
    seed_z: i32,
    threshold: i32,
    fill_val: i32,
) -> Result<(Ret<Mri>, i32), Fatal> {
    let mut v = match p {
        Pair::New(s) => {
            let fresh = mri_clone(g, out, err, s, None)?.into_new();
            Two {
                src: Some(s),
                dst: None,
                fresh,
                shadow: Default::default(),
            }
        }
        Pair::Given(s, d) => Two {
            src: Some(s),
            dst: Some(d),
            fresh: None,
            shadow: Default::default(),
        },
        Pair::Same(d) => Two {
            src: None,
            dst: Some(d),
            fresh: None,
            shadow: Default::default(),
        },
    };
    let (width, height, depth) = (v.s().vg.width, v.s().vg.height, v.s().vg.depth);
    let mut sx = seed_x;
    let mut sy = seed_y;
    let mut sz = seed_z;
    if sx < 0 {
        sx = 0;
    } else if sx >= width {
        sx = width - 1;
    }
    let wrong = |g: &mut Globals, axis: &str, n: i32| {
        g.notices.add(
            &FS_316,
            format!(
                "MRIfillFG clamps a seed {axis} past the volume's {n} to the width minus one ({})",
                width - 1
            ),
        );
        if g.notices.reproduce() {
            width - 1
        } else {
            n - 1
        }
    };
    if sy < 0 {
        sy = 0;
    } else if sy >= height {
        sy = wrong(g, "row", height);
    }
    if sz < 0 {
        sz = 0;
    } else if sz >= depth {
        sz = wrong(g, "slice", depth);
    }
    let mut xmin = (sx - 1).max(0);
    let mut xmax = (sx + 1).min(width - 1);
    let mut ymin = (sy - 1).max(0);
    let mut ymax = (sy + 1).min(height - 1);
    let mut zmin = (sz - 1).max(0);
    let mut zmax = (sz + 1).min(depth - 1);
    if width > 0 && height > 0 && depth > 0 {
        need_voxels(g, err, "MRIfillFG", v.d())?;
    }
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let val = v.dget(g, x, y, z);
                if val == i64::from(fill_val) {
                    v.dput(g, x, y, z, val - 1);
                }
            }
        }
    }
    need_voxels(g, err, "MRIfillFG", v.d())?;
    v.dput(g, sx, sy, sz, i64::from(fill_val));
    let fv = i64::from(fill_val);
    let mut total = 1i32;
    loop {
        let (mut z0, mut z1, mut y0, mut y1, mut x0, mut x1) = (zmin, zmax, ymin, ymax, xmin, xmax);
        let mut nfilled = 0i32;
        for z in zmin..=zmax {
            for y in ymin..=ymax {
                for x in xmin..=xmax {
                    let val = v.sget(g, x, y, z);
                    if val < i64::from(threshold) || v.dget(g, x, y, z) == fv {
                        continue;
                    }
                    let mut on = false;
                    if x > 0 {
                        on = v.dget(g, x - 1, y, z) == fv;
                    }
                    if !on && x < width - 1 {
                        on = v.dget(g, x + 1, y, z) == fv;
                    }
                    if !on && y > 0 {
                        on = v.dget(g, x, y - 1, z) == fv;
                    }
                    if !on && y < height - 1 {
                        on = v.dget(g, x, y + 1, z) == fv;
                    }
                    if !on && z > 0 {
                        on = v.dget(g, x, y, z - 1) == fv;
                    }
                    if !on && z < depth - 1 {
                        on = v.dget(g, x, y, z + 1) == fv;
                    }
                    if on {
                        if x <= x0 {
                            x0 = (x - 1).max(0);
                        }
                        if x >= x1 {
                            x1 = (x + 1).min(width - 1);
                        }
                        if y <= y0 {
                            y0 = (y - 1).max(0);
                        }
                        if y >= y1 {
                            y1 = (y + 1).min(height - 1);
                        }
                        if z <= z0 {
                            z0 = (z - 1).max(0);
                        }
                        if z >= z1 {
                            z1 = (z + 1).min(depth - 1);
                        }
                        nfilled += 1;
                        v.dput(g, x, y, z, fv);
                    }
                }
            }
        }
        zmin = z0;
        zmax = z1;
        ymin = y0;
        ymax = y1;
        xmin = x0;
        xmax = x1;
        total = total.wrapping_add(nfilled);
        if nfilled <= 0 {
            break;
        }
    }
    let r = match v.fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    };
    Ok((r, total))
}

/// `MRIextractPlane(mri_src, mri_dst, orientation, where)`: frame 0's plane `where` of the source
/// (`MRI_CORONAL` and any unknown orientation: slice `where`, x by y; `MRI_HORIZONTAL`: row
/// `where`, x by z; `MRI_SAGITTAL`: column `where`, z by y) into slice 0 of the destination by
/// `MRIgetVoxVal` and `MRIsetVoxVal`, over the source's extent; without a destination, a new
/// one-slice volume of the plane's extent and the source's type with the source's header, `zstart =
/// where`, `zend = where + 1`, `imnr0 = 0`, `imnr1 = 1`.
pub fn mri_extract_plane(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    orientation: i32,
    at: i32,
) -> Result<Ret<Mri>, Fatal> {
    let (sw, sh, sd) = (src.vg.width, src.vg.height, src.vg.depth);
    let (width, height) = match orientation {
        MRI_HORIZONTAL => (sw, sd),
        MRI_SAGITTAL => (sd, sh),
        _ => (sw, sh),
    };
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut m = mri_alloc(g, out, err, width, height, 1, src.mtype)?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            m.zstart = at as f32;
            m.zend = at.wrapping_add(1) as f32;
            m.imnr0 = 0;
            m.imnr1 = 1;
            fresh.insert(m)
        }
    };
    let mut copy = |g: &mut Globals, sx: i32, sy: i32, sz: i32, dx: i32, dy: i32| {
        // the official build stores the float it read (no round trip through `double`: a
        // signalling NaN stays signalling)
        let val = mri_get_vox_val(g, err, src, sx, sy, sz, 0, 0)?;
        mri_set_vox_val(g, err, d, dx, dy, 0, 0, val, 0).map(|_| ())
    };
    match orientation {
        MRI_HORIZONTAL => {
            for x in 0..sw {
                for z in 0..sd {
                    copy(g, x, at, z, x, z)?;
                }
            }
        }
        MRI_SAGITTAL => {
            for z in 0..sd {
                for y in 0..sh {
                    copy(g, at, y, z, z, y)?;
                }
            }
        }
        _ => {
            for x in 0..sw {
                for y in 0..sh {
                    copy(g, x, y, at, x, y)?;
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIfillPlane(mri_mask, mri_dst, orientation, where, fillval)`: over the destination's extent,
/// each voxel of the mask's slice 0 that is not 0 (`MRIgetVoxVal`; NaN counts as not 0) sets the
/// destination's voxel of plane `where` (`MRI_CORONAL` and unknown orientations: (x, y, where);
/// `MRI_HORIZONTAL`: (x, where, z) from the mask's (x, z); `MRI_SAGITTAL`: (where, y, z) from the
/// mask's (z, y)) of frame 0 to `fillval` (`MRIsetVoxVal`).
pub fn mri_fill_plane(
    g: &mut Globals,
    err: &mut dyn Write,
    mask: &Mri,
    dst: &mut Mri,
    orientation: i32,
    at: i32,
    fillval: i32,
) -> Result<(), Fatal> {
    let (w, h, d) = (dst.vg.width, dst.vg.height, dst.vg.depth);
    let fv = fillval as f32;
    match orientation {
        MRI_HORIZONTAL => {
            for x in 0..w {
                for z in 0..d {
                    if mri_get_vox_val(g, err, mask, x, z, 0, 0, 0)? != 0.0 {
                        mri_set_vox_val(g, err, dst, x, at, z, 0, fv, 0)?;
                    }
                }
            }
        }
        MRI_SAGITTAL => {
            for z in 0..d {
                for y in 0..h {
                    if mri_get_vox_val(g, err, mask, z, y, 0, 0, 0)? != 0.0 {
                        mri_set_vox_val(g, err, dst, at, y, z, 0, fv, 0)?;
                    }
                }
            }
        }
        _ => {
            for x in 0..w {
                for y in 0..h {
                    if mri_get_vox_val(g, err, mask, x, y, 0, 0, 0)? != 0.0 {
                        mri_set_vox_val(g, err, dst, x, y, at, 0, fv, 0)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// A new 4 x 1 vector (`VectorAlloc(4, MATRIX_REAL)`, `MatrixZero(4, 1, NULL)`).
fn vec4(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    v: [f32; 4],
) -> Result<Matrix, Fatal> {
    let mut c = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    for (i, x) in v.into_iter().enumerate() {
        c.set_relt(i as i32 + 1, 1, x);
    }
    Ok(c)
}

/// `M * C` (`MatrixMultiply(M, C, NULL)`), its three first elements.
fn times(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &Matrix,
    c: &Matrix,
) -> Result<[f32; 3], Fatal> {
    let p = matrix_multiply(g, out, err, Some(m), Some(c), None)?
        .into_new()
        .expect("a product");
    Ok([p.relt(1, 1), p.relt(2, 1), p.relt(3, 1)])
}

/// `MRIupsampleN(mri_src, mri_dst, N)`: each voxel repeated N times on each axis (voxel (x, y, z)
/// of every source frame is the source's (x / N, y / N, z / N), by `MRIgetVoxVal` and
/// `MRIsetVoxVal`, over N times the source's extent); without a destination, a new one of N times
/// the extent, the source's type and frames, with the source's header. The destination's voxel
/// sizes are the source's over N, its direction cosines, starts and ends the source's, `imnr0` the
/// source's, `imnr1 = imnr0 + depth - 1`, and its centre placed so that its first voxel's centre is
/// at the source's voxel coordinates `-0.5 + 1 / (2 N)` (`MRIp0ToCRAS`); the cached matrices
/// recomputed.
pub fn mri_upsample_n(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    n: i32,
) -> Result<Ret<Mri>, Fatal> {
    let width = n.wrapping_mul(src.vg.width);
    let height = n.wrapping_mul(src.vg.height);
    let depth = n.wrapping_mul(src.vg.depth);
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut m =
                mri_alloc_sequence(g, out, err, width, height, depth, src.mtype, src.nframes)?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    let nf = n as f32;
    d.vg.xsize = div(src.vg.xsize, nf);
    d.vg.ysize = div(src.vg.ysize, nf);
    d.vg.zsize = div(src.vg.zsize, nf);
    d.vg.x_r = src.vg.x_r;
    d.vg.x_a = src.vg.x_a;
    d.vg.x_s = src.vg.x_s;
    d.vg.y_r = src.vg.y_r;
    d.vg.y_a = src.vg.y_a;
    d.vg.y_s = src.vg.y_s;
    d.vg.z_r = src.vg.z_r;
    d.vg.z_a = src.vg.z_a;
    d.vg.z_s = src.vg.z_s;
    d.xstart = src.xstart;
    d.ystart = src.ystart;
    d.zstart = src.zstart;
    d.xend = src.xend;
    d.yend = src.yend;
    d.zend = src.zend;
    d.imnr0 = src.imnr0;
    d.imnr1 = src.imnr0.wrapping_add(d.vg.depth).wrapping_sub(1);
    let v2r = xfm_crs2xyz(g, out, err, &src.vg, 0, false)?;
    let c0 = (-0.5 + 1.0 / f64::from(n.wrapping_mul(2))) as f32;
    let crs0 = vec4(g, out, err, [c0, c0, c0, 1.0])?;
    let ras0 = times(g, out, err, &v2r, &crs0)?;
    p0_to_cras(
        g,
        out,
        err,
        &mut d.vg,
        f64::from(ras0[0]),
        f64::from(ras0[1]),
        f64::from(ras0[2]),
    )?;
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                for f in 0..src.nframes {
                    let val = mri_get_vox_val(g, err, src, x / n, y / n, z / n, f, 0)?;
                    mri_set_vox_val(g, err, d, x, y, z, f, val, 0)?;
                }
            }
        }
    }
    reinit_cache(g, out, err, &mut d.vg)?;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIupsample2(mri_src, mri_dst)`: [`mri_upsample_n`] with N = 2.
pub fn mri_upsample2(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    mri_upsample_n(g, out, err, src, dst, 2)
}

/// `MRIupsampleNConserve(mri_src, mri_dst, N)`: [`mri_upsample_n`] of the source as floats
/// (`MRISeqchangeType(src, MRI_FLOAT, 0, 0, 0)` unless it is one), then every voxel divided by
/// `N * N * N` (an `int`, in `double`). FS-319 where the change of type gives no volume.
pub fn mri_upsample_n_conserve(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &mut Mri,
    dst: Option<&mut Mri>,
    n: i32,
) -> Result<Ret<Mri>, Fatal> {
    let dd = f64::from(n.wrapping_mul(n).wrapping_mul(n));
    let tmp = if src.mtype != MRI_FLOAT {
        match mri_seq_change_type(g, out, err, env, src, MRI_FLOAT, 0.0, 0.0, 0)? {
            Some(m) => Some(m),
            None => {
                g.notices.add(
                    &FS_319,
                    "MRIupsampleNConserve goes on with no volume after MRIchangeType's error",
                );
                log::error_exit(
                    g,
                    err,
                    ERROR_BADPARM,
                    "MRIupsampleNConserve(): no volume to upsample (FreeSurfer crashes here)\n",
                )?;
                unreachable!("error_exit returns an error")
            }
        }
    } else {
        None
    };
    let s: &Mri = tmp.as_ref().unwrap_or(&*src);
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => {
            mri_upsample_n(g, out, err, s, Some(d), n)?;
            d
        }
        None => fresh.insert(
            mri_upsample_n(g, out, err, s, None, n)?
                .into_new()
                .expect("a new volume"),
        ),
    };
    for c in 0..d.vg.width {
        for r in 0..d.vg.height {
            for sl in 0..d.vg.depth {
                for f in 0..d.nframes {
                    let v = f64::from(mri_get_vox_val(g, err, d, c, r, sl, f, 0)?);
                    mri_set_vox_val(g, err, d, c, r, sl, f, div64(v, dd) as f32, 0)?;
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIdownsample2(mri_src, mri_dst)`: each 2 x 2 x 2 block of frame 0 averaged (the eight voxels
/// summed in `float` in the order z, y, x, the sum times 1/8; bytes, shorts and unsigned shorts
/// rounded with `nint` and cast) into a volume of half the extent (rounded down); without a
/// destination, a new one of the source's type with the source's header (a given one must have the
/// source's type: `MRIdownsample2: source and dst must be same type`, `NULL`). The destination is
/// cleared first; a type other than bytes, shorts, unsigned shorts and floats gives
/// `MRIdownsample2: unsupported input type <t>` and `NULL` when a block is summed. Then `imnr0` the
/// source's, `imnr1 = imnr0 + depth - 1`, the voxel sizes, `thick` and `ps` doubled, the centre
/// the source's voxel `(width / 2 + 0.5, height / 2 + 0.5, depth / 2 + 0.5)` (integer halves), the
/// cached matrices recomputed.
pub fn mri_downsample2(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    if let Some(d) = &dst
        && src.mtype != d.mtype
    {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            "MRIdownsample2: source and dst must be same type",
        );
        return Ok(Ret::Null);
    }
    let (width, height, depth) = (src.vg.width / 2, src.vg.height / 2, src.vg.depth / 2);
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let mut m = mri_alloc(g, out, err, width, height, depth, src.mtype)?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    mri_clear(g, err, d)?;
    let ty = src.mtype;
    if width > 0 && height > 0 && depth > 0 {
        need_voxels(g, err, "MRIdownsample2", src)?;
    }
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let mut val = 0.0f32;
                for z1 in 2 * z..=2 * z + 1 {
                    for y1 in 2 * y..=2 * y + 1 {
                        if !matches!(ty, MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_FLOAT) {
                            log::error_printf(
                                g,
                                err,
                                ERROR_UNSUPPORTED,
                                &format!("MRIdownsample2: unsupported input type {ty}"),
                            );
                            return Ok(Ret::Null);
                        }
                        for x1 in 2 * x..=2 * x + 1 {
                            let v = rows::read(
                                g,
                                "MRIdownsample2",
                                src,
                                i64::from(z1),
                                i64::from(y1),
                                i64::from(x1),
                                ty,
                            )
                            .f();
                            val = add(val, v);
                        }
                    }
                }
                let q = mul(val, 0.125);
                let v = match ty {
                    MRI_FLOAT => Num::F(q),
                    MRI_UCHAR => Num::I(i64::from(nint(g, f64::from(q)) as u8)),
                    MRI_SHORT => Num::I(i64::from(nint(g, f64::from(q)) as i16)),
                    _ => Num::I(i64::from(nint(g, f64::from(q)) as u16)),
                };
                need_voxels(g, err, "MRIdownsample2", d)?;
                rows::write(
                    g,
                    "MRIdownsample2",
                    d,
                    i64::from(z),
                    i64::from(y),
                    i64::from(x),
                    ty,
                    v,
                );
            }
        }
    }
    d.imnr0 = src.imnr0;
    d.imnr1 = src.imnr0.wrapping_add(d.vg.depth).wrapping_sub(1);
    d.vg.xsize = add(src.vg.xsize, src.vg.xsize);
    d.vg.ysize = add(src.vg.ysize, src.vg.ysize);
    d.vg.zsize = add(src.vg.zsize, src.vg.zsize);
    d.thick = add(src.thick, src.thick);
    d.ps = add(src.ps, src.ps);
    let half = |n: i32| (f64::from(n / 2) + 0.5) as f32;
    let c = vec4(
        g,
        out,
        err,
        [
            half(src.vg.width),
            half(src.vg.height),
            half(src.vg.depth),
            1.0,
        ],
    )?;
    let v2r = extract_i_to_r(g, out, err, &src.vg)?;
    let p = times(g, out, err, &v2r, &c)?;
    d.vg.c_r = p[0];
    d.vg.c_a = p[1];
    d.vg.c_s = p[2];
    reinit_cache(g, out, err, &mut d.vg)?;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIdownsampleN(src, dst, Fc, Fr, Fs, KeepType)`: each window of the source averaged into a
/// voxel: the extent `(int) ceilf(width / Fc)` (and the rows', slices'), voxel (c, r, s) of each
/// frame the sum in `double` of the source's voxels (`MRIgetVoxVal`) at columns from `(int) (c Fc)`
/// while below `(c + 1) Fc` (in `float`), likewise rows and slices (slices innermost), over `(Fc Fr)
/// Fs` (in `float`), stored by `MRIsetVoxVal`. Without a destination, a new one of floats (the
/// source's type with `KeepType`) and the source's frames with the source's header, `imnr0` the
/// source's, `imnr1 = imnr0 + depth - 1`, the voxel sizes times the factors, `thick` times `Fs`,
/// `ps` times `Fc`, and its first voxel's centre at the source's voxel `(-0.5 + Fc / 2, -0.5 + Fr /
/// 2, -0.5 + Fs / 2)` (`MRIp0ToCRAS`, the cached matrices recomputed). A destination whose extent
/// or frames differ: `ERROR: MRIdownsampleN: dimension mismatch` on standard output and `NULL`.
#[allow(clippy::too_many_arguments)]
pub fn mri_downsample_n(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    fc: f32,
    fr: f32,
    fs: f32,
    keep_type: i32,
) -> Result<Ret<Mri>, Fatal> {
    let ext = |g: &mut Globals, n: i32, f: f32| {
        to_int(
            g,
            f64::from(div(n as f32, f).ceil()),
            "MRIdownsampleN's extent",
        )
    };
    let width = ext(g, src.vg.width, fc);
    let height = ext(g, src.vg.height, fr);
    let depth = ext(g, src.vg.depth, fs);
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => {
            let ty = if keep_type != 0 { src.mtype } else { MRI_FLOAT };
            let mut m = mri_alloc_sequence(g, out, err, width, height, depth, ty, src.nframes)?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            m.imnr0 = src.imnr0;
            m.imnr1 = src.imnr0.wrapping_add(m.vg.depth).wrapping_sub(1);
            // the official build's operands: the source's field first, but Fr * ysize
            m.vg.xsize = mul(src.vg.xsize, fc);
            m.vg.ysize = mul(fr, src.vg.ysize);
            m.vg.zsize = mul(src.vg.zsize, fs);
            m.thick = mul(src.thick, fs);
            m.ps = mul(src.ps, fc);
            let h = |f: f32| (f64::from(f) * 0.5 - 0.5) as f32;
            let c = vec4(g, out, err, [h(fc), h(fr), h(fs), 1.0])?;
            let v2r = xfm_crs2xyz(g, out, err, &src.vg, 0, false)?;
            let p0 = times(g, out, err, &v2r, &c)?;
            p0_to_cras(
                g,
                out,
                err,
                &mut m.vg,
                f64::from(p0[0]),
                f64::from(p0[1]),
                f64::from(p0[2]),
            )?;
            reinit_cache(g, out, err, &mut m.vg)?;
            fresh.insert(m)
        }
    };
    if d.vg.width != width
        || d.vg.height != height
        || d.vg.depth != depth
        || d.nframes != src.nframes
    {
        let _ = writeln!(out, "ERROR: MRIdownsampleN: dimension mismatch");
        return Ok(Ret::Null);
    }
    let div_by = f64::from(mul(mul(fc, fr), fs));
    let start = |g: &mut Globals, i: i32, f: f32| {
        to_int(g, f64::from(mul(i as f32, f)), "MRIdownsampleN's window")
    };
    for f in 0..d.nframes {
        for c in 0..d.vg.width {
            for r in 0..d.vg.height {
                for s in 0..d.vg.depth {
                    let mut val = 0.0f64;
                    let cend = mul(c.wrapping_add(1) as f32, fc);
                    let mut c0 = start(g, c, fc);
                    while (c0 as f32) < cend {
                        let rend = mul(r.wrapping_add(1) as f32, fr);
                        let mut r0 = start(g, r, fr);
                        while (r0 as f32) < rend {
                            let send = mul(s.wrapping_add(1) as f32, fs);
                            let mut s0 = start(g, s, fs);
                            while (s0 as f32) < send {
                                let v = mri_get_vox_val(g, err, src, c0, r0, s0, f, 0)?;
                                val = add64(f64::from(v), val);
                                s0 = s0.wrapping_add(1);
                            }
                            r0 = r0.wrapping_add(1);
                        }
                        c0 = c0.wrapping_add(1);
                    }
                    mri_set_vox_val(g, err, d, c, r, s, f, div64(val, div_by) as f32, 0)?;
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRImeanFrameThresh(mri, frame, thresh)`: the mean (in `double`) of the voxels at or above
/// `thresh` (NaN voxels left out), each by the nearest sampler at its whole coordinates; FreeSurfer
/// samples frame 0 whatever `frame` (FS-318: the default samples the frame). No voxel gives `0 /
/// 0`. A frame at or past the volume's frames gives `MRImeanFrame: frame <f> out of bounds (<n>)`
/// (`ERROR_BADPARM`) and 0.
pub fn mri_mean_frame_thresh(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    frame: i32,
    thresh: f32,
) -> f64 {
    if mri.nframes <= frame {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MRImeanFrame: frame {frame} out of bounds ({})",
                mri.nframes
            ),
        );
        return 0.0;
    }
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let mut mean = 0.0f64;
    let mut num = 0i32;
    if frame != 0 && w > 0 && h > 0 && d > 0 {
        g.notices.add(
            &FS_318,
            format!("MRImeanFrameThresh samples frame 0 instead of frame {frame}"),
        );
    }
    let frame0 = frame == 0 || g.notices.reproduce();
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let (fx, fy, fz) = (f64::from(x), f64::from(y), f64::from(z));
                let (_, val) = if frame0 {
                    sample_volume_nearest(g, err, mri, fx, fy, fz)
                } else {
                    sample_volume_frame_nearest(g, err, mri, fx, fy, fz, frame)
                };
                if val < f64::from(thresh) {
                    continue;
                }
                mean = add64(val, mean);
                num = num.wrapping_add(1);
            }
        }
    }
    div64(mean, f64::from(num))
}

/// `MRIlimits(mri, &min, &max)`: the smallest and largest voxel (`MRIgetVoxVal`, C's comparisons
/// from voxel (0, 0, 0) of frame 0: a NaN first voxel stays); a volume without voxels leaves them
/// as they were. `NO_ERROR`.
pub fn mri_limits(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    min: &mut f32,
    max: &mut f32,
) -> Result<i32, Fatal> {
    if mri.chunk.is_none() {
        return Ok(NO_ERROR);
    }
    let v0 = mri_get_vox_val(g, err, mri, 0, 0, 0, 0, 0)?;
    *min = v0;
    *max = v0;
    for f in 0..mri.nframes {
        for i in 0..mri.vg.width {
            for j in 0..mri.vg.height {
                for k in 0..mri.vg.depth {
                    let val = mri_get_vox_val(g, err, mri, i, j, k, f, 0)?;
                    if val < *min {
                        *min = val;
                    }
                    if val > *max {
                        *max = val;
                    }
                }
            }
        }
    }
    Ok(NO_ERROR)
}

/// `MRIchangeType(src, dest_type, f_low, f_high, no_scale_option_flag)`: a copy of the volume as
/// `dest_type`. The same type: `MRIcopy`. Bytes to shorts, ints, longs or floats, shorts and
/// unsigned shorts to ints, longs or floats, longs to ints or floats, ints to longs or floats: the
/// values (`MRIgetVoxVal`, `MRIsetVoxVal`). Otherwise `MRIlimits`; `no_scale_option_flag` above 1,
/// or 1 (with, when the range does not fit the type, `******* WARNING - forcing scaling of values to
/// prevent cropping of input [<min>, <max>]` and `***** DISABLING forcing and not doing scaling
/// ********` on standard output): the values; 0: the values rescaled from a robust range: a
/// histogram of 1,000 bins over [min, max] (`MRIchangeType: Building histogram ...` on standard
/// output; with `FS_FORCE_BIN_CHECK` set, narrower bins when frame 0's mean of the voxels at or
/// above 1e-7 falls in the first fifth), the low end past `f_low` of the voxels, the high end
/// before `1 - f_high` of the voxels not 0 (`fabs >= 1e-15`), `new = dest_min + scale (val - min)`
/// in `float` with the type's range (0 to 0 for floats) over the robust range (1 when it is
/// empty). A robust range whose low end is above its high end: `MRIchangeType(): after hist:
/// src_min = <g>, src_max = <g> (f_low = <g>, f_high = <g>)` (`ERROR_BADPARM`) and `None`.
#[allow(clippy::too_many_arguments)]
pub fn mri_change_type(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    dest_type: i32,
    f_low: f32,
    f_high: f32,
    no_scale_option_flag: i32,
) -> Result<Option<Mri>, Fatal> {
    if src.mtype == dest_type {
        return Ok(mri_copy(g, out, err, src, None)?.into_new());
    }
    let st = src.mtype;
    let to = |ts: &[i32]| ts.contains(&dest_type);
    let mut no_scale = false;
    let (mut src_min, mut src_max) = (0.0f32, 0.0f32);
    if (st == MRI_UCHAR && to(&[MRI_SHORT, MRI_INT, MRI_LONG, MRI_FLOAT]))
        || ((st == MRI_SHORT || st == MRI_USHRT) && to(&[MRI_INT, MRI_LONG, MRI_FLOAT]))
        || (st == MRI_LONG && to(&[MRI_INT, MRI_FLOAT]))
        || (st == MRI_INT && to(&[MRI_LONG, MRI_FLOAT]))
    {
        no_scale = true;
    } else {
        mri_limits(g, err, src, &mut src_min, &mut src_max)?;
        if no_scale_option_flag > 1 {
            no_scale = true;
        } else if no_scale_option_flag == 1 {
            let (lo, hi) = (f64::from(src_min), f64::from(src_max));
            let fits = |a: f64, b: f64| lo >= a && hi <= b;
            if (dest_type == MRI_UCHAR && fits(0.0, 255.0))
                || (dest_type == MRI_SHORT && fits(-32768.0, 32767.0))
                || (dest_type == MRI_INT
                    && src_min >= i32::MIN as f32
                    && src_max <= i32::MAX as f32)
                || (dest_type == MRI_LONG
                    && src_min >= i64::MIN as f32
                    && src_max <= i64::MAX as f32)
            {
                no_scale = true;
            }
            if !no_scale {
                let _ = writeln!(
                    out,
                    "******* WARNING - forcing scaling of values to prevent cropping of input [{}, {}]",
                    cfmt::pad(&cfmt::f(lo, 0), 2),
                    cfmt::pad(&cfmt::f(hi, 0), 2)
                );
                let _ = writeln!(
                    out,
                    "***** DISABLING forcing and not doing scaling ********"
                );
                no_scale = true;
            }
        }
    }
    let (w, h, d, nf) = (src.vg.width, src.vg.height, src.vg.depth, src.nframes);
    if no_scale {
        let mut dest = mri_alloc_sequence(g, out, err, w, h, d, dest_type, nf)?;
        mri_copy_header(g, out, err, src, Some(&mut dest))?;
        dest.mtype = dest_type;
        for frame in 0..nf {
            for k in 0..d {
                for j in 0..h {
                    for i in 0..w {
                        let val = mri_get_vox_val(g, err, src, i, j, k, frame, 0)?;
                        mri_set_vox_val(g, err, &mut dest, i, j, k, frame, val, 0)?;
                    }
                }
            }
        }
        return Ok(Some(dest));
    }
    let mut nonzero: i64 = 0;
    let _ = writeln!(
        out,
        "MRIchangeType: Building histogram {} {} {N_HIST_BINS}, flo={}, fhi={}, dest_type={dest_type}",
        cfmt::g(f64::from(src_min), 6),
        cfmt::g(f64::from(src_max), 6),
        cfmt::g(f64::from(f_low), 6),
        cfmt::g(f64::from(f_high), 6)
    );
    let mut bin_size = div(sub(src_max, src_min), N_HIST_BINS as f32);
    if env.get("FS_FORCE_BIN_CHECK").is_some() {
        let mn = mri_mean_frame_thresh(g, err, src, 0, 1e-7f64 as f32);
        let mn_bin = to_int(
            g,
            div64(
                larmorx_freesurfer_core::sse::sub64(mn, f64::from(src_min)),
                f64::from(bin_size),
            ),
            "MRIchangeType's mean bin",
        );
        let bin_threshold = N_HIST_BINS as f32 / 5.0;
        if (mn_bin as f32) < bin_threshold {
            let old = bin_size;
            bin_size = div64(
                larmorx_freesurfer_core::sse::sub64(mn, f64::from(src_min)),
                f64::from(bin_threshold),
            ) as f32;
            let p = |v: f64| cfmt::f(v, 2);
            let _ = writeln!(
                out,
                "MRIchangeType: original bin size {} (max {}) too big for mean/min {}/{}, scaling down to {}",
                p(f64::from(old)),
                cfmt::f(f64::from(src_max), 1),
                p(mn),
                p(f64::from(src_min)),
                p(f64::from(bin_size))
            );
        }
    }
    let mut hist = vec![0i32; N_HIST_BINS as usize];
    for frame in 0..nf {
        for i in 0..w {
            for j in 0..h {
                for k in 0..d {
                    let val = mri_get_vox_val(g, err, src, i, j, k, frame, 0)?;
                    // DZERO(val): fabs(val) < 1e-15 (NaN is not zero)
                    let dzero = f64::from(val).abs() < 1e-15;
                    if !dzero {
                        nonzero += 1;
                    }
                    let mut bin = to_int(
                        g,
                        f64::from(div(sub(val, src_min), bin_size)),
                        "MRIchangeType's histogram bin",
                    );
                    if bin < 0 {
                        bin = 0;
                    }
                    if bin >= N_HIST_BINS {
                        bin = N_HIST_BINS - 1;
                    }
                    hist[bin as usize] = hist[bin as usize].wrapping_add(1);
                }
            }
        }
    }
    let nth = to_int(
        g,
        f64::from(mul(mul(mul(w as f32, f_low), h as f32), d as f32)),
        "MRIchangeType's low count",
    );
    let (mut n_passed, mut bin) = (0i32, 0i32);
    while n_passed < nth && bin < N_HIST_BINS {
        n_passed = n_passed.wrapping_add(hist[bin as usize]);
        bin += 1;
    }
    src_min = add(src_min, mul(bin as f32, bin_size));
    let nth = to_int(
        g,
        (nonzero as f64) * (1.0 - f64::from(f_high)),
        "MRIchangeType's high count",
    );
    let (mut n_passed, mut bin) = (0i32, N_HIST_BINS - 1);
    while n_passed < nth && bin > 0 {
        n_passed = n_passed.wrapping_add(hist[bin as usize]);
        bin -= 1;
    }
    src_max = add(mul(bin as f32, bin_size), src_min);
    if src_min > src_max {
        let p = |v: f32| cfmt::g(f64::from(v), 6);
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "MRIchangeType(): after hist: src_min = {}, src_max = {} (f_low = {}, f_high = {})",
                p(src_min),
                p(src_max),
                p(f_low),
                p(f_high)
            ),
        );
        return Ok(None);
    }
    let (dest_min, dest_max): (f32, f32) = match dest_type {
        MRI_UCHAR => (0.0, 255.0),
        MRI_SHORT => (-32768.0, 32767.0),
        MRI_USHRT => (0.0, 65535.0),
        MRI_INT => (i32::MIN as f32, i32::MAX as f32),
        MRI_LONG => (i64::MIN as f32, i64::MAX as f32),
        _ => (0.0, 0.0),
    };
    let scale = if src_max == src_min {
        1.0
    } else {
        div(sub(dest_max, dest_min), sub(src_max, src_min))
    };
    let mut dest = mri_alloc_sequence(g, out, err, w, h, d, dest_type, nf)?;
    mri_copy_header(g, out, err, src, Some(&mut dest))?;
    dest.mtype = dest_type;
    for frame in 0..nf {
        for i in 0..w {
            for j in 0..h {
                for k in 0..d {
                    let val = mri_get_vox_val(g, err, src, i, j, k, frame, 0)?;
                    let val = add(mul(sub(val, src_min), scale), dest_min);
                    mri_set_vox_val(g, err, &mut dest, i, j, k, frame, val, 0)?;
                }
            }
        }
    }
    Ok(Some(dest))
}

/// `MRISeqchangeType(vol, dest_type, f_low, f_high, no_scale_option_flag)`: [`mri_change_type`] of
/// the volume seen as one frame of `depth x nframes` slices, the result given back the volume's
/// depth and frames (and a fresh frame list); `None` after `ERROR: MRISeqchangeType:
/// MRIchangeType` on standard error when it gives none.
#[allow(clippy::too_many_arguments)]
pub fn mri_seq_change_type(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    vol: &mut Mri,
    dest_type: i32,
    f_low: f32,
    f_high: f32,
    no_scale_option_flag: i32,
) -> Result<Option<Mri>, Fatal> {
    let (nslices, nframes) = (vol.vg.depth, vol.nframes);
    vol.vg.depth = nslices.wrapping_mul(nframes);
    vol.nframes = 1;
    let r = mri_change_type(
        g,
        out,
        err,
        env,
        vol,
        dest_type,
        f_low,
        f_high,
        no_scale_option_flag,
    );
    vol.vg.depth = nslices;
    vol.nframes = nframes;
    let Some(mut m) = r? else {
        let _ = writeln!(err, "ERROR: MRISeqchangeType: MRIchangeType");
        return Ok(None);
    };
    m.vg.depth = nslices;
    m.nframes = nframes;
    m.vox_per_row = m.vg.width as u64;
    m.vox_per_slice = m.vox_per_row.wrapping_mul(m.vg.height as u64);
    m.vox_per_vol = m.vox_per_slice.wrapping_mul(m.vg.depth as u64);
    m.frames = (0..nframes.max(0))
        .map(|_| MriFrame {
            m_ras2vox: Some(Matrix::real(4, 4)),
            ..MriFrame::default()
        })
        .collect();
    Ok(Some(m))
}

/// `MRIextractRegionAndPad(mri_src, mri_dst, region, pad)`: a new volume (a given destination is
/// never used) of the region's extent plus `2 pad` on each axis, the source's type, frames and
/// header, holding the region (`MRIextractInto` into a volume of its own, then into the new one
/// at `(pad, pad, pad)`), its voxel-to-RAS matrix the source's (`extract_i_to_r`) times a
/// translation by `-pad` (`float`; `MRIsetVox2RASFromMatrix`, the cached matrices kept). The
/// whole source without a region (printing `(box.dx, box.dy, box.dz) = (<dx>, <dy>, <dz>)`); the
/// region's extent printed twice (`(region->dx, region->dy, region->dz) = (...)`).
pub fn mri_extract_region_and_pad(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &mut Mri,
    region: Option<&MriRegion>,
    pad: i32,
) -> Result<Mri, Fatal> {
    let r = match region {
        Some(r) => *r,
        None => {
            let b = MriRegion {
                x: 0,
                y: 0,
                z: 0,
                dx: src.vg.width,
                dy: src.vg.height,
                dz: src.vg.depth,
            };
            let _ = writeln!(
                out,
                "(box.dx, box.dy, box.dz) = ({}, {}, {})",
                b.dx, b.dy, b.dz
            );
            b
        }
    };
    let line = format!(
        "(region->dx, region->dy, region->dz) = ({}, {}, {})\n",
        r.dx, r.dy, r.dz
    );
    let _ = out.write_all(line.as_bytes());
    let p2 = pad.wrapping_mul(2);
    let mut dst = mri_alloc_sequence(
        g,
        out,
        err,
        r.dx.wrapping_add(p2),
        r.dy.wrapping_add(p2),
        r.dz.wrapping_add(p2),
        src.mtype,
        src.nframes,
    )?;
    mri_copy_header(g, out, err, src, Some(&mut dst))?;
    let _ = out.write_all(line.as_bytes());
    let size = [r.dx, r.dy, r.dz];
    let mut tmp =
        mri_extract_into(g, out, err, src, None, [r.x, r.y, r.z], size, [0, 0, 0])?.unwrap_new();
    mri_extract_into(
        g,
        out,
        err,
        &mut tmp,
        Some(&mut dst),
        [0, 0, 0],
        size,
        [pad; 3],
    )?;
    let m_src = extract_i_to_r(g, out, err, &src.vg)?;
    let mut m_trans = matrix_identity(g, out, err, 4, None)?.unwrap_new();
    for row in 1..=3 {
        m_trans.set_relt(row, 4, pad.wrapping_neg() as f32);
    }
    let m_dst = matrix_multiply(g, out, err, Some(&m_src), Some(&m_trans), None)?.unwrap_new();
    set_vox2ras_from_matrix(g, out, err, &mut dst.vg, &m_dst)?;
    Ok(dst)
}
