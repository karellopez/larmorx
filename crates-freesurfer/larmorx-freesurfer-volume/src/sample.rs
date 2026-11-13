// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The volume samplers of `utils/mri.cpp` (3.1-36): nearest, trilinear, cubic and windowed-sinc
//! samples of a volume at a point given in voxel coordinates (`MRIsampleVolume`,
//! `MRIsampleVolumeType`, `MRIsampleVolumeFrame`, `MRIsampleVolumeFrameType`, `MRIcubicCoeff`,
//! `MRIcubicSampleVolume`, `MRIcubicSampleVolumeFrame`, `ham_sinc`, `MRIsincSampleVolume`,
//! `MRIsincSampleVolumeFrame`).
//!
//! Every sampler takes FreeSurfer's `double *pval` as `pval`: FreeSurfer leaves it as it was when
//! it returns an error before setting it, and so does the port. The voxels are read through the row
//! pointers with the element type the code names (`MRIvox`, `MRIFvox`, ...), as [`crate::rows`]
//! models them (FS-289, FS-290 for frames and volumes without voxels).
//!
//! The NaNs (the porting guide's rule "NaN bits are part of the output"): a sample of a float
//! volume sums products of weights and voxels in `double`; where two NaN products meet, the one
//! the official build keeps is decided by its operand order, read from the object code
//! (`docs/findings/freesurfer-volume.md` §5) and followed with SSE's rules
//! ([`larmorx_freesurfer_core::sse`]).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §5):
//! - **FS-298** (rule A): the samplers read a volume of 64-bit integers (`MRI_LONG`) as 32-bit
//!   integers at the voxel's index: the low and high halves of other voxels. The default reads
//!   the voxel; `reproduce` FreeSurfer's halves (noted where they differ).
//! - **FS-299** (rule A): `MRIcubicSampleVolume` divides the weighted sum by the number of points
//!   it added (64 inside the volume), where its weights sum to 8: a constant volume samples to an
//!   eighth of its value. The default divides by 8, as `MRIcubicSampleVolumeFrame` does.
//! - **FS-300** (memory): `MRIcubicSampleVolumeFrame` adds the 4 x 4 x 4 points of its window
//!   whatever the volume's edge, the points outside never written (stack contents). Both modes
//!   take them as 0 (the source's own comment: "accept zeroes for the border expansion"), noted.
//! - **FS-302** (crash): the sinc samplers keep at most 128 weights per axis; a half-width whose
//!   window holds more writes past the arrays. An error in both modes.

use std::io::Write;

use larmorx_core::defects::{FS_298, FS_299, FS_300, FS_302};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::{add64, div64, mul64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri,
};

use crate::coords::{index_not_in_volume, sample_volume_nearest};
use crate::rows;

/// `SAMPLE_NEAREST` and the other sampling types (`mri.h`).
pub use crate::coords::SAMPLE_NEAREST;
pub const SAMPLE_TRILINEAR: i32 = 1;
pub const SAMPLE_SINC: i32 = 2;
pub const SAMPLE_CUBIC: i32 = 3;
pub const SAMPLE_WEIGHTED: i32 = 4;
pub const SAMPLE_CUBIC_BSPLINE: i32 = 5;
pub const SAMPLE_VOTE: i32 = 6;

/// x86-64's `(int) x` (`cvttsd2si`): `INT_MIN` for NaN and values outside the `int` range.
fn cvtt(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        i32::MIN
    } else {
        x.trunc() as i32
    }
}

/// `FEQUAL((int)x, x)`: `fabs((int)x - x) < FLT_EPSILON`, the difference in `double`. (The
/// conversion is undefined for NaN and for values outside the `int` range; x86-64's `INT_MIN`
/// and any other `int` give the same answer there: not whole.)
fn whole(x: f64) -> bool {
    (f64::from(cvtt(x)) - x).abs() < f64::from(f32::EPSILON)
}

/// The voxel `x` of row `y` of slice `sl` (`z + frame depth`) as the samplers read it, in
/// `double`: the volume's type's element; for `MRI_LONG` FreeSurfer's `long32` (FS-298).
fn vox(g: &mut Globals, what: &str, m: &Mri, sl: i64, y: i64, x: i64) -> f64 {
    match m.mtype {
        MRI_LONG => {
            let half = rows::read(g, what, m, sl, y, x, MRI_INT).f64();
            let full = rows::read(g, what, m, sl, y, x, MRI_LONG).f64();
            if half.to_bits() != full.to_bits() {
                g.notices.add(
                    &FS_298,
                    format!("{what} reads a 64-bit integer voxel as a 32-bit integer"),
                );
                if g.notices.reproduce() {
                    return half;
                }
            }
            full
        }
        ty => rows::read(g, what, m, sl, y, x, ty).f64(),
    }
}

/// The types the trilinear, cubic and sinc samplers read.
fn sampled_type(ty: i32) -> bool {
    matches!(
        ty,
        MRI_UCHAR | MRI_FLOAT | MRI_SHORT | MRI_USHRT | MRI_INT | MRI_LONG
    )
}

/// `MRIsampleVolumeType(mri, x, y, z, &val, type)`: `SAMPLE_TRILINEAR` [`sample_volume`],
/// `SAMPLE_CUBIC` [`cubic_sample_volume`], `SAMPLE_SINC` [`sinc_sample_volume`] with a half-width
/// of 5, any other type the nearest voxel ([`sample_volume_nearest`]).
#[allow(clippy::too_many_arguments)]
pub fn sample_volume_type(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    pval: &mut f64,
    ty: i32,
) -> Result<i32, Fatal> {
    match ty {
        SAMPLE_TRILINEAR => Ok(sample_volume(g, err, mri, x, y, z, pval)),
        SAMPLE_CUBIC => Ok(cubic_sample_volume(g, err, mri, x, y, z, pval)),
        SAMPLE_SINC => sinc_sample_volume(g, err, mri, x, y, z, 5, pval),
        _ => {
            let (r, v) = sample_volume_nearest(g, err, mri, x, y, z);
            *pval = v;
            Ok(r)
        }
    }
}

/// `MRIsampleVolumeFrameType(mri, x, y, z, frame, type, &val)`: whole coordinates (each within
/// `FLT_EPSILON` of its `int`) take the nearest voxel whatever the type; `SAMPLE_TRILINEAR`
/// [`sample_volume_frame`]; `SAMPLE_CUBIC_BSPLINE` the error `MRIsampleVolumeFrameType(5): First
/// create coeff image and then use it to sample (see mriBSpline).` and `SAMPLE_SINC` the error
/// `MRIsampleVolumeFrameType(2): unsupported interpolation type` (`ERROR_UNSUPPORTED`, `pval`
/// unchanged); any other type the nearest voxel of the frame ([`sample_volume_frame_nearest`]).
#[allow(clippy::too_many_arguments)]
pub fn sample_volume_frame_type(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    frame: i32,
    ty: i32,
    pval: &mut f64,
) -> i32 {
    let ty = if whole(x) && whole(y) && whole(z) {
        SAMPLE_NEAREST
    } else {
        ty
    };
    match ty {
        SAMPLE_TRILINEAR => sample_volume_frame(g, err, mri, x, y, z, frame, pval),
        SAMPLE_CUBIC_BSPLINE => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!(
                    "MRIsampleVolumeFrameType({ty}): First create coeff image and then use it to sample (see mriBSpline)."
                ),
            );
            ERROR_UNSUPPORTED
        }
        SAMPLE_SINC => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("MRIsampleVolumeFrameType({ty}): unsupported interpolation type"),
            );
            ERROR_UNSUPPORTED
        }
        _ => {
            let (r, v) = sample_volume_frame_nearest(g, err, mri, x, y, z, frame);
            *pval = v;
            r
        }
    }
}

/// `MRIsampleVolumeFrameType(mri, x, y, z, frame, SAMPLE_NEAREST, &val)`: `outside_val` for a
/// point unambiguously outside ([`index_not_in_volume`] 1); else the voxel of `frame` nearest to
/// it (`nint`, clamped to the volume) as a `float`, read through the row pointers (a frame the
/// volume does not have reads memory it does not own: FS-289, FS-290); another type than bytes,
/// shorts, unsigned shorts, ints and floats gives 0 and the error `MRIsampleVolumeFrameType:
/// unsupported volume type <t>` (`ERROR_UNSUPPORTED`).
#[allow(clippy::too_many_arguments)]
pub fn sample_volume_frame_nearest(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    frame: i32,
) -> (i32, f64) {
    if index_not_in_volume(mri, x, y, z) == 1 {
        return (NO_ERROR, mri.outside_val);
    }
    let clamp = |v: i32, n: i32| {
        let v = if v < 0 { 0 } else { v };
        if v >= n { n - 1 } else { v }
    };
    let xv = clamp(nint(g, x), mri.vg.width);
    let yv = clamp(nint(g, y), mri.vg.height);
    let zv = clamp(nint(g, z), mri.vg.depth);
    let ty = mri.mtype;
    if !matches!(ty, MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_INT | MRI_FLOAT) {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("MRIsampleVolumeFrameType: unsupported volume type {ty}"),
        );
        return (ERROR_UNSUPPORTED, 0.0);
    }
    let sl = i64::from(zv) + i64::from(frame) * i64::from(mri.vg.depth);
    let v = rows::read(
        g,
        "MRIsampleVolumeFrameType",
        mri,
        sl,
        i64::from(yv),
        i64::from(xv),
        ty,
    );
    (NO_ERROR, f64::from(v.f()))
}

/// `MRIsampleVolume(mri, x, y, z, &val)`: the trilinear sample of frame 0 (see
/// [`sample_volume_frame`]); whole coordinates take [`sample_volume_type`]'s nearest voxel; another
/// type gives the error `MRIsampleVolume: unsupported type <t>` (`pval` unchanged).
pub fn sample_volume(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    pval: &mut f64,
) -> i32 {
    if whole(x) && whole(y) && whole(z) {
        let (r, v) = sample_volume_nearest(g, err, mri, x, y, z);
        *pval = v;
        return r;
    }
    trilinear(g, err, "MRIsampleVolume", mri, x, y, z, 0, pval)
}

/// `MRIsampleVolumeFrame(mri, x, y, z, frame, &val)`: whole coordinates take the frame's nearest
/// voxel ([`sample_volume_frame_type`]); a frame at or past the volume's frames gives
/// `outside_val`, as does a point unambiguously outside; else the point is clamped to `[0, extent -
/// 1]` on each axis and the eight voxels around it are weighted by the distances (`x - (float)
/// xm`, `1 - that`, in `double`): `sum of ((wx wy) wz) v` over the corners in the order (xm, ym,
/// zm), (xm, ym, zp), (xm, yp, zm), ... (xp, yp, zp); another type gives the error
/// `MRIsampleVolumeFrame: unsupported type <t>` (`pval` unchanged).
#[allow(clippy::too_many_arguments)]
pub fn sample_volume_frame(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    frame: i32,
    pval: &mut f64,
) -> i32 {
    if whole(x) && whole(y) && whole(z) {
        return sample_volume_frame_type(g, err, mri, x, y, z, frame, SAMPLE_NEAREST, pval);
    }
    if frame >= mri.nframes {
        *pval = mri.outside_val;
        return NO_ERROR;
    }
    trilinear(g, err, "MRIsampleVolumeFrame", mri, x, y, z, frame, pval)
}

/// The trilinear sample of `MRIsampleVolume` and `MRIsampleVolumeFrame` once the whole
/// coordinates and the frames past the volume's are dealt with.
#[allow(clippy::too_many_arguments)]
fn trilinear(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    frame: i32,
    pval: &mut f64,
) -> i32 {
    if index_not_in_volume(mri, x, y, z) == 1 {
        *pval = mri.outside_val;
        return NO_ERROR;
    }
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let clampf = |v: f64, n: i32| {
        let v = if v >= f64::from(n) {
            f64::from(n) - 1.0
        } else {
            v
        };
        if v < 0.0 { 0.0 } else { v }
    };
    let (x, y, z) = (clampf(x, w), clampf(y, h), clampf(z, d));
    // xm = MAX((int)x, 0), xp = MIN(width - 1, xm + 1); the distances against (float) xm
    let lo = |v: f64| cvtt(v).max(0);
    let (xm, ym, zm) = (lo(x), lo(y), lo(z));
    let (xp, yp, zp) = (
        (xm + 1).min(w - 1),
        (ym + 1).min(h - 1),
        (zm + 1).min(d - 1),
    );
    let xmd = x - f64::from(xm as f32);
    let ymd = y - f64::from(ym as f32);
    let zmd = z - f64::from(zm as f32);
    let (xpd, ypd, zpd) = (1.0 - xmd, 1.0 - ymd, 1.0 - zmd);
    if !sampled_type(mri.mtype) {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("{what}: unsupported type {}", mri.mtype),
        );
        return ERROR_UNSUPPORTED;
    }
    let base = i64::from(frame) * i64::from(d);
    let v = |g: &mut Globals, xi: i32, yi: i32, zi: i32| {
        vox(
            g,
            what,
            mri,
            base + i64::from(zi),
            i64::from(yi),
            i64::from(xi),
        )
    };
    // the corners in the source's order; the weights are finite (only a voxel can be NaN)
    let t1 = mul64(xpd * ypd * zpd, v(g, xm, ym, zm));
    let t2 = mul64(xpd * ypd * zmd, v(g, xm, ym, zp));
    let t3 = mul64(xpd * ymd * zpd, v(g, xm, yp, zm));
    let t4 = mul64(xpd * ymd * zmd, v(g, xm, yp, zp));
    let t5 = mul64(xmd * ypd * zpd, v(g, xp, ym, zm));
    let t6 = mul64(xmd * ypd * zmd, v(g, xp, ym, zp));
    let t7 = mul64(xmd * ymd * zpd, v(g, xp, yp, zm));
    let t8 = mul64(xmd * ymd * zmd, v(g, xp, yp, zp));
    // the official build's additions (the first operand kept where two NaNs meet)
    let mut s = add64(t1, t2);
    s = add64(s, t3);
    s = add64(t4, s);
    s = add64(s, t5);
    s = add64(t6, s);
    s = add64(t7, s);
    s = add64(s, t8);
    *pval = s;
    NO_ERROR
}

/// `MRIcubicCoeff(x, iter)`: the cubic weights `((2 - x) x - 1) x`, `(3 x - 5) x x + 2`, `((4 - 3
/// x) x + 1) x` and `(x - 1) x x` for `iter` 0 to 3 (they sum to 2); another `iter` gives the error
/// `MRIcubicCoeff: called wrong by MRIcubicSampleVolume!` and `ERROR_UNSUPPORTED` as the value.
pub fn cubic_coeff(g: &mut Globals, err: &mut dyn Write, x: f64, iter: i32) -> f64 {
    match iter {
        0 => ((2.0 - x) * x - 1.0) * x,
        1 => (3.0 * x - 5.0) * x * x + 2.0,
        2 => ((4.0 - 3.0 * x) * x + 1.0) * x,
        3 => (x - 1.0) * x * x,
        _ => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                "MRIcubicCoeff: called wrong by MRIcubicSampleVolume!",
            );
            f64::from(ERROR_UNSUPPORTED)
        }
    }
}

/// The cubic window of a coordinate clamped to `[0, n - 1]`: the first of the four points
/// (`floor(x) - 1`, one less at the last voxel) and the fraction (`x - floor(x)`, 1 at the last
/// voxel).
fn cubic_axis(v: f64, n: i32) -> (i32, f64) {
    let v = if v > f64::from(n) - 1.0 {
        f64::from(n) - 1.0
    } else {
        v
    };
    let v = if v < 0.0 { 0.0 } else { v };
    let low = cvtt(v.floor());
    if low < n - 1 {
        (low, v - f64::from(low))
    } else {
        (low - 1, 1.0)
    }
}

/// `MRIcubicSampleVolume(mri, x, y, z, &val)`: whole coordinates take the nearest voxel
/// ([`sample_volume_type`]); a point unambiguously outside gives `outside_val`; else the point is
/// clamped to `[0, extent - 1]` and the points of the 4 x 4 x 4 window inside the volume (`ix` from
/// `max(0, 1 - low)` below `min(4, extent + 1 - low)`, each axis) are summed: `val = ((v fx) fy)
/// fz + val`, z outermost, x innermost, with [`cubic_coeff`]'s weights; the sum is divided by the
/// number of points (FS-299: by 8 by default). Another type gives the error
/// `MRIcubicSampleVolume: unsupported type <t>` (`pval` unchanged).
pub fn cubic_sample_volume(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    pval: &mut f64,
) -> i32 {
    if whole(x) && whole(y) && whole(z) {
        let (r, v) = sample_volume_nearest(g, err, mri, x, y, z);
        *pval = v;
        return r;
    }
    cubic(g, err, "MRIcubicSampleVolume", mri, x, y, z, None, pval)
}

/// `MRIcubicSampleVolumeFrame(mri, x, y, z, frame, &val)`: as [`cubic_sample_volume`] on `frame`
/// (whole coordinates its nearest voxel, [`sample_volume_frame_type`]; no check of the frame),
/// but every point of the 4 x 4 x 4 window is added (those outside the volume as 0: FS-300) and
/// the sum is divided by 8 (`val * 0.125`).
#[allow(clippy::too_many_arguments)]
pub fn cubic_sample_volume_frame(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    frame: i32,
    pval: &mut f64,
) -> i32 {
    if whole(x) && whole(y) && whole(z) {
        return sample_volume_frame_type(g, err, mri, x, y, z, frame, SAMPLE_NEAREST, pval);
    }
    cubic(
        g,
        err,
        "MRIcubicSampleVolumeFrame",
        mri,
        x,
        y,
        z,
        Some(frame),
        pval,
    )
}

#[allow(clippy::too_many_arguments)]
fn cubic(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    frame: Option<i32>,
    pval: &mut f64,
) -> i32 {
    if index_not_in_volume(mri, x, y, z) == 1 {
        *pval = mri.outside_val;
        return NO_ERROR;
    }
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let (ixl, xx) = cubic_axis(x, w);
    let (iyl, yy) = cubic_axis(y, h);
    let (izl, zz) = cubic_axis(z, d);
    let range = |low: i32, n: i32| (1 - low).max(0)..(n + 1 - low).min(4);
    let (rx, ry, rz) = (range(ixl, w), range(iyl, h), range(izl, d));
    let base = i64::from(frame.unwrap_or(0)) * i64::from(d);
    let mut vv = [[[0.0f64; 4]; 4]; 4];
    for iz in rz.clone() {
        for iy in ry.clone() {
            for ix in rx.clone() {
                if !sampled_type(mri.mtype) {
                    log::error_printf(
                        g,
                        err,
                        ERROR_UNSUPPORTED,
                        &format!("{what}: unsupported type {}", mri.mtype),
                    );
                    return ERROR_UNSUPPORTED;
                }
                vv[ix as usize][iy as usize][iz as usize] = vox(
                    g,
                    what,
                    mri,
                    base + i64::from(izl - 1 + iz),
                    i64::from(iyl - 1 + iy),
                    i64::from(ixl - 1 + ix),
                );
            }
        }
    }
    let full = frame.is_some();
    if full && (rx.len() < 4 || ry.len() < 4 || rz.len() < 4) {
        g.notices.add(
            &FS_300,
            format!("{what} adds window points outside the volume that it never read (memory contents); taken as 0"),
        );
    }
    let (rx, ry, rz) = if full {
        (0..4, 0..4, 0..4)
    } else {
        (rx, ry, rz)
    };
    let mut val = 0.0f64;
    let mut nvals = 0i32;
    for iz in rz {
        let fz = cubic_coeff(g, err, zz, iz);
        for iy in ry.clone() {
            let fy = cubic_coeff(g, err, yy, iy);
            for ix in rx.clone() {
                let fx = cubic_coeff(g, err, xx, ix);
                // the official build: fx * v first, then fy, fz; the term first in the sum
                let t = mul64(
                    mul64(mul64(fx, vv[ix as usize][iy as usize][iz as usize]), fy),
                    fz,
                );
                val = add64(t, val);
                nvals += 1;
            }
        }
    }
    *pval = if full {
        mul64(val, 0.125)
    } else if nvals == 8 {
        div64(val, 8.0)
    } else {
        g.notices.add(
            &FS_299,
            format!("{what} divides its sum by {nvals} points where its weights sum to 8"),
        );
        if g.notices.reproduce() {
            div64(val, f64::from(nvals))
        } else {
            div64(val, 8.0)
        }
    };
    NO_ERROR
}

/// `ham_sinc(x, fullwidth)`: 1 for `|x| < 1e-5`, else `sin(pi x) / (pi x)` times the Hamming
/// window `0.54 + 0.46 cos(2 pi x / fullwidth)` (`sin` and `cos` of `double`: glibc 2.35's with
/// `reproduce`, the correctly rounded ones by default; an infinite argument gives SSE's default
/// NaN in both, as glibc's does on x86-64).
pub fn ham_sinc(libm: &Libm, x: f64, fullwidth: f64) -> f64 {
    if x.abs() < 1.0e-5 {
        return 1.0;
    }
    let px = mul64(std::f64::consts::PI, x);
    let ham = div64(libm_nan(libm.sin(px), px), px);
    let a = div64(mul64(2.0 * std::f64::consts::PI, x), fullwidth);
    let window = add64(mul64(0.46, libm_nan(libm.cos(a), a)), 0.54);
    mul64(ham, window)
}

/// A C library result `r` of the argument `a` with x86-64's NaN: a NaN argument passed on quieted,
/// a NaN made from a number (an infinite argument) SSE's default NaN.
fn libm_nan(r: f64, a: f64) -> f64 {
    if !r.is_nan() {
        r
    } else if a.is_nan() {
        f64::from_bits(a.to_bits() | 0x0008_0000_0000_0000)
    } else {
        larmorx_freesurfer_core::sse::DEFAULT_NAN64
    }
}

/// `MRIsincSampleVolume(mri, x, y, z, hw, &val)`: see [`sinc_sample_volume_frame`] (frame 0;
/// errors name `MRIsincSampleVolume`).
#[allow(clippy::too_many_arguments)]
pub fn sinc_sample_volume(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    hw: i32,
    pval: &mut f64,
) -> Result<i32, Fatal> {
    sinc(g, err, "MRIsincSampleVolume", mri, x, y, z, 0, hw, pval)
}

/// `MRIsincSampleVolumeFrame(mri, x, y, z, frame, hw, &val)`: a point unambiguously outside gives
/// `outside_val`; else, clamped as the trilinear sampler clamps it, the separable Hamming-windowed
/// sinc over the voxels `max(ceil(v) - hw, 0)` to below `min(floor(v) + hw, extent - 1)` on each
/// axis (the last voxel of an axis never taken), each axis's weights [`ham_sinc`]`(v - j, 2 hw)`
/// normalised by their sum: `sum_x += v (cx / sx)`, `sum_y += sum_x (cy / sy)`, `sum_z += sum_y
/// (cz / sz)`; a byte or short volume clips a negative sum to 0, a byte one a sum above 255 to
/// 255, a short one above 65535 to 65535. A point whose `ceil(z)` is the depth gives 0. More than
/// 128 weights on an axis: FS-302, an error. Another type gives the error
/// `MRIsincSampleVolumeFrame: unsupported type <t>` at its first voxel (`pval` unchanged).
#[allow(clippy::too_many_arguments)]
pub fn sinc_sample_volume_frame(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    frame: i32,
    hw: i32,
    pval: &mut f64,
) -> Result<i32, Fatal> {
    sinc(
        g,
        err,
        "MRIsincSampleVolumeFrame",
        mri,
        x,
        y,
        z,
        frame,
        hw,
        pval,
    )
}

#[allow(clippy::too_many_arguments)]
fn sinc(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    frame: i32,
    hw: i32,
    pval: &mut f64,
) -> Result<i32, Fatal> {
    if index_not_in_volume(mri, x, y, z) == 1 {
        *pval = mri.outside_val;
        return Ok(NO_ERROR);
    }
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let clampf = |v: f64, n: i32| {
        let v = if v >= f64::from(n) {
            f64::from(n) - 1.0
        } else {
            v
        };
        if v < 0.0 { 0.0 } else { v }
    };
    let (x, y, z) = (clampf(x, w), clampf(y, h), clampf(z, d));
    let (ixl, ixh) = (cvtt(x.floor()), cvtt(x.ceil()));
    let (iyl, iyh) = (cvtt(y.floor()), cvtt(y.ceil()));
    let (izl, izh) = (cvtt(z.floor()), cvtt(z.ceil()));
    if !(izl >= 0 && izh < d) {
        *pval = 0.0;
        return Ok(NO_ERROR);
    }
    // the window of an axis: from IMAX(high - hw, 0) to below IMIN(low + hw, n - 1)
    let span = |low: i32, high: i32, n: i32| {
        let a = high.wrapping_sub(hw).max(0);
        let b = low.wrapping_add(hw).min(n - 1);
        a..b.max(a)
    };
    let (sx, sy, sz) = (span(ixl, ixh, w), span(iyl, iyh, h), span(izl, izh, d));
    if sx.len() > 128 || sy.len() > 128 || sz.len() > 128 {
        g.notices.add(
            &FS_302,
            format!("{what} with a half-width of {hw} writes past its 128 weights per axis"),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{what}: a half-width of {hw} needs more than 128 weights on an axis (FreeSurfer writes past its arrays here)\n"
            ),
        )?;
        return Ok(ERROR_BADPARM);
    }
    let libm = Libm::quiet(g.mode());
    let fw = f64::from(hw.wrapping_mul(2));
    let coeffs = |v: f64, s: std::ops::Range<i32>| {
        let mut c = Vec::with_capacity(s.len());
        let mut sum = 0.0f64;
        for j in s {
            let k = ham_sinc(&libm, v - f64::from(j), fw);
            c.push(k);
            sum = add64(k, sum);
        }
        (c, sum)
    };
    let (cx, cxs) = coeffs(x, sx.clone());
    let (cy, cys) = coeffs(y, sy.clone());
    let (cz, czs) = coeffs(z, sz.clone());
    let base = i64::from(frame) * i64::from(d);
    let mut sum_z = 0.0f64;
    for (kz, jz) in sz.enumerate() {
        let mut sum_y = 0.0f64;
        for (ky, jy) in sy.clone().enumerate() {
            let mut sum_x = 0.0f64;
            for (kx, jx) in sx.clone().enumerate() {
                if !sampled_type(mri.mtype) {
                    log::error_printf(
                        g,
                        err,
                        ERROR_UNSUPPORTED,
                        &format!("{what}: unsupported type {}", mri.mtype),
                    );
                    return Ok(ERROR_UNSUPPORTED);
                }
                let v = vox(
                    g,
                    what,
                    mri,
                    base + i64::from(jz),
                    i64::from(jy),
                    i64::from(jx),
                );
                sum_x = add64(sum_x, mul64(v, div64(cx[kx], cxs)));
            }
            sum_y = add64(sum_y, mul64(sum_x, div64(cy[ky], cys)));
        }
        sum_z = add64(sum_z, mul64(sum_y, div64(cz[kz], czs)));
    }
    let ty = mri.mtype;
    *pval = if (ty == MRI_UCHAR || ty == MRI_SHORT) && sum_z < 0.0 {
        0.0
    } else if ty == MRI_UCHAR && sum_z > 255.0 {
        255.0
    } else if ty == MRI_SHORT && sum_z > 65535.0 {
        65535.0
    } else {
        sum_z
    };
    Ok(NO_ERROR)
}
