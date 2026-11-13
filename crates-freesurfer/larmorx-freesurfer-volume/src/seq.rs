// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The frame-range samplers, the trilinear kernel and trilinear splatting of `utils/mri.cpp`
//! (3.1-36): `MRIinterpolateIntoVolume`, `MRIinterpolateIntoVolumeFrame`, `MRIsampleSeqVolume`,
//! `MRIsampleSeqVolumeType`, `MRItrilinKernel`, `MRIsampleVolumeFrame_xyzInt_nRange_floats` and
//! `MRIsampleVolumeFrameType_xyzInt_nRange_SAMPLE_NEAREST_floats` (compiled in: FreeSurfer's
//! `faster_variants.h` defines `FASTER_MRI_EM_REGISTER`).
//!
//! The voxels are read and written through the row pointers with the element type the code names
//! ([`crate::rows`]: FS-289, FS-290 for frames past the volume's); no frame is checked against the
//! volume's frames. The NaNs follow the official build's operand order (the object code,
//! `docs/findings/freesurfer-volume.md` §10): a splatted float voxel is `(float)(share + voxel)`,
//! the share first; a frame-range trilinear sample sums its eight terms left to right, the running
//! sum first.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §10):
//! - **FS-298** (rule A): the frame-range samplers read a volume of 64-bit integers as 32-bit
//!   integers at the voxel's index, as the other samplers do.
//! - **FS-320** (rule A): splatting into an integer volume adds each corner's rounded share in the
//!   voxel's type; a sum beyond the type's range wraps around. The default clamps it to the range;
//!   `reproduce` wraps as FreeSurfer does.
//! - **FS-321** (rule A): splatting into a volume of 64-bit integers adds into 32-bit integers at
//!   the voxel's index (halves of other voxels). The default adds into the voxel; `reproduce` into
//!   FreeSurfer's halves.

use std::io::Write;

use larmorx_core::defects::{FS_320, FS_321};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::{add64, mul64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri,
};

use crate::coords::index_not_in_volume;
use crate::rows::{self, Num};
use crate::sample::{
    SAMPLE_CUBIC, SAMPLE_CUBIC_BSPLINE, SAMPLE_SINC, SAMPLE_TRILINEAR, cvtt, sampled_type, vox,
};

/// The trilinear corners of a point once a point unambiguously outside is dealt with: the point
/// clamped to `[0, extent - 1]` on each axis, `xm = MAX((int) x, 0)`, `xp = MIN(width - 1, xm +
/// 1)` (likewise y and z), and the distances `xmd = x - m` (`m` the corner as a `float` or a
/// `double`, the same number) and `xpd = 1 - xmd`, in `double`.
#[derive(Clone, Copy)]
pub(crate) struct Corners {
    pub xm: i32,
    pub xp: i32,
    pub ym: i32,
    pub yp: i32,
    pub zm: i32,
    pub zp: i32,
    pub xmd: f64,
    pub ymd: f64,
    pub zmd: f64,
    pub xpd: f64,
    pub ypd: f64,
    pub zpd: f64,
}

impl Corners {
    pub(crate) fn of(mri: &Mri, x: f64, y: f64, z: f64) -> Corners {
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
        let lo = |v: f64| cvtt(v).max(0);
        let (xm, ym, zm) = (lo(x), lo(y), lo(z));
        let xmd = x - f64::from(xm as f32);
        let ymd = y - f64::from(ym as f32);
        let zmd = z - f64::from(zm as f32);
        Corners {
            xm,
            xp: (xm + 1).min(w - 1),
            ym,
            yp: (ym + 1).min(h - 1),
            zm,
            zp: (zm + 1).min(d - 1),
            xmd,
            ymd,
            zmd,
            xpd: 1.0 - xmd,
            ypd: 1.0 - ymd,
            zpd: 1.0 - zmd,
        }
    }

    /// The eight corners in the source's order, each with its weight `(wx wy) wz`: (xm, ym, zm),
    /// (xm, ym, zp), (xm, yp, zm), (xm, yp, zp), (xp, ym, zm), ... (xp, yp, zp).
    pub(crate) fn eight(&self) -> [(i32, i32, i32, f64); 8] {
        let c = self;
        [
            (c.xm, c.ym, c.zm, c.xpd * c.ypd * c.zpd),
            (c.xm, c.ym, c.zp, c.xpd * c.ypd * c.zmd),
            (c.xm, c.yp, c.zm, c.xpd * c.ymd * c.zpd),
            (c.xm, c.yp, c.zp, c.xpd * c.ymd * c.zmd),
            (c.xp, c.ym, c.zm, c.xmd * c.ypd * c.zpd),
            (c.xp, c.ym, c.zp, c.xmd * c.ypd * c.zmd),
            (c.xp, c.yp, c.zm, c.xmd * c.ymd * c.zpd),
            (c.xp, c.yp, c.zp, c.xmd * c.ymd * c.zmd),
        ]
    }
}

/// `MRIinterpolateIntoVolume(mri, x, y, z, val)`: [`interpolate_into_volume_frame`] of frame 0.
pub fn interpolate_into_volume(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    x: f64,
    y: f64,
    z: f64,
    val: f64,
) -> i32 {
    interpolate_into_volume_frame(g, err, mri, x, y, z, 0, val)
}

/// The range of an integer voxel type the splat stores into.
fn type_range(ty: i32) -> (i64, i64) {
    match ty {
        MRI_UCHAR => (0, 255),
        MRI_SHORT => (i64::from(i16::MIN), i64::from(i16::MAX)),
        MRI_USHRT => (0, i64::from(u16::MAX)),
        MRI_INT => (i64::from(i32::MIN), i64::from(i32::MAX)),
        _ => (i64::MIN, i64::MAX),
    }
}

/// `MRIinterpolateIntoVolumeFrame(mri, x, y, z, frame, val)`: adds `val` into the eight voxels of
/// `frame` around a point, each its trilinear share `((wx wy) wz) val` (in `double`): a float voxel
/// becomes `(float)(share + voxel)`; an integer voxel gets `nint(share)` added in its type
/// (FS-320, and FS-321 for 64-bit integers). A point unambiguously outside
/// ([`index_not_in_volume`] 1) changes nothing; another point is clamped to the volume first. The
/// frame is not checked. Another type gives the error `MRIsampleVolume: unsupported type <t>`
/// (`ERROR_UNSUPPORTED`).
#[allow(clippy::too_many_arguments)]
pub fn interpolate_into_volume_frame(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    x: f64,
    y: f64,
    z: f64,
    frame: i32,
    val: f64,
) -> i32 {
    const WHAT: &str = "MRIinterpolateIntoVolume";
    if index_not_in_volume(mri, x, y, z) == 1 {
        return NO_ERROR;
    }
    let c = Corners::of(mri, x, y, z);
    let ty = mri.mtype;
    if !sampled_type(ty) {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("MRIsampleVolume: unsupported type {ty}"),
        );
        return ERROR_UNSUPPORTED;
    }
    let base = i64::from(frame) * i64::from(mri.vg.depth);
    for (xi, yi, zi, w) in c.eight() {
        let share = mul64(w, val);
        let (sl, yy, xx) = (base + i64::from(zi), i64::from(yi), i64::from(xi));
        if ty == MRI_FLOAT {
            let v = rows::read(g, WHAT, mri, sl, yy, xx, MRI_FLOAT).f();
            let nv = add64(share, f64::from(v)) as f32;
            rows::write(g, WHAT, mri, sl, yy, xx, MRI_FLOAT, Num::F(nv));
            continue;
        }
        let k = i64::from(nint(g, share));
        // FS-321: a long volume is written as 32-bit integers at the voxel's index
        let half = ty == MRI_LONG && g.notices.reproduce();
        if ty == MRI_LONG && k != 0 {
            g.notices.add(
                &FS_321,
                format!("{WHAT} adds into a 64-bit integer volume as 32-bit integers"),
            );
        }
        let ety = if half { MRI_INT } else { ty };
        let Num::I(cur) = rows::read(g, WHAT, mri, sl, yy, xx, ety) else {
            unreachable!("an integer type")
        };
        let (lo, hi) = type_range(ety);
        let sum = cur.checked_add(k);
        let stored = match sum {
            Some(s) if (lo..=hi).contains(&s) => s,
            _ => {
                g.notices.add(
                    &FS_320,
                    format!("{WHAT} adds {k} to a voxel of {cur}: beyond the voxel type's range"),
                );
                if g.notices.reproduce() {
                    // the type's low bits (rows::encode keeps them)
                    cur.wrapping_add(k)
                } else if k > 0 {
                    hi
                } else {
                    lo
                }
            }
        };
        rows::write(g, WHAT, mri, sl, yy, xx, ety, Num::I(stored));
    }
    NO_ERROR
}

/// The frames `first..=last` of `valvect`, or a port error when they do not fit it (FreeSurfer
/// writes past the caller's array: the caller's mistake).
fn frames_fit(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    n: usize,
    first: i32,
    last: i32,
) -> Result<(), Fatal> {
    if first > last || (first >= 0 && (last as i64) < n as i64) {
        return Ok(());
    }
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        &format!("{what}: frames {first} to {last} do not fit the {n} values given\n"),
    )
}

/// `MRIsampleSeqVolume(mri, x, y, z, valvect, firstframe, lastframe)`: the trilinear sample of
/// each frame `firstframe..=lastframe` into `valvect[frame]` (as a `float`): a point
/// unambiguously outside gives `(float) outside_val` to each; else the point is clamped to the
/// volume and the eight corners' `((wx wy) wz) voxel` (in `double`) are summed left to right. The
/// frames are not checked against the volume's. Another type gives the error `MRIsampleSeqVolume:
/// unsupported type <t>` at the first frame (`ERROR_UNSUPPORTED`, that frame's value unchanged).
/// `valvect` must hold the frames (else a port error: FreeSurfer writes past it).
#[allow(clippy::too_many_arguments)]
pub fn sample_seq_volume(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    valvect: &mut [f32],
    firstframe: i32,
    lastframe: i32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIsampleSeqVolume";
    frames_fit(g, err, WHAT, valvect.len(), firstframe, lastframe)?;
    if index_not_in_volume(mri, x, y, z) == 1 {
        for f in firstframe..=lastframe {
            valvect[f as usize] = mri.outside_val as f32;
        }
        return Ok(NO_ERROR);
    }
    let c = Corners::of(mri, x, y, z);
    let d = i64::from(mri.vg.depth);
    for f in firstframe..=lastframe {
        if !sampled_type(mri.mtype) {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("{WHAT}: unsupported type {}", mri.mtype),
            );
            return Ok(ERROR_UNSUPPORTED);
        }
        let base = i64::from(f) * d;
        let mut s = 0.0f64;
        for (k, (xi, yi, zi, w)) in c.eight().into_iter().enumerate() {
            let v = vox(
                g,
                WHAT,
                mri,
                base + i64::from(zi),
                i64::from(yi),
                i64::from(xi),
            );
            let t = mul64(w, v);
            s = if k == 0 { t } else { add64(s, t) };
        }
        valvect[f as usize] = s as f32;
    }
    Ok(NO_ERROR)
}

/// `MRIsampleSeqVolumeType(mri, x, y, z, valvect, firstframe, lastframe, type)`:
/// `SAMPLE_TRILINEAR` [`sample_seq_volume`]; `SAMPLE_CUBIC` and `SAMPLE_SINC` print `Cubic
/// interpolation is not implemented yet on multi-frame data. Going ahead with tri-linear` (`Sinc
/// ...`; no newline) on standard output, then [`sample_seq_volume`]; `SAMPLE_CUBIC_BSPLINE`
/// prints `Cubic Bspline interpolation not implemented here. Needs to be done by calling
/// function!` and ends the program (`exit(1)`); any other type the nearest voxel of each frame:
/// `(float) outside_val` for a point unambiguously outside, else the voxel at `nint` of the point,
/// clamped to the volume; another volume type gives 0 and the error `MRIsampleSeqVolumeType:
/// unsupported type <t>` at the first frame.
#[allow(clippy::too_many_arguments)]
pub fn sample_seq_volume_type(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    valvect: &mut [f32],
    firstframe: i32,
    lastframe: i32,
    ty: i32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIsampleSeqVolumeType";
    match ty {
        SAMPLE_TRILINEAR => {
            return sample_seq_volume(g, err, mri, x, y, z, valvect, firstframe, lastframe);
        }
        SAMPLE_CUBIC | SAMPLE_SINC => {
            let name = if ty == SAMPLE_CUBIC { "Cubic" } else { "Sinc" };
            let _ = write!(
                out,
                "{name} interpolation is not implemented yet on multi-frame data. Going ahead with tri-linear"
            );
            return sample_seq_volume(g, err, mri, x, y, z, valvect, firstframe, lastframe);
        }
        SAMPLE_CUBIC_BSPLINE => {
            let _ = write!(
                out,
                "Cubic Bspline interpolation not implemented here. Needs to be done by calling function!"
            );
            return Err(Fatal::Exit(1));
        }
        _ => {}
    }
    frames_fit(g, err, WHAT, valvect.len(), firstframe, lastframe)?;
    if index_not_in_volume(mri, x, y, z) == 1 {
        for f in firstframe..=lastframe {
            valvect[f as usize] = mri.outside_val as f32;
        }
        return Ok(NO_ERROR);
    }
    let clamp = |v: i32, n: i32| {
        let v = if v < 0 { 0 } else { v };
        if v >= n { n - 1 } else { v }
    };
    let xv = clamp(nint(g, x), mri.vg.width);
    let yv = clamp(nint(g, y), mri.vg.height);
    let zv = clamp(nint(g, z), mri.vg.depth);
    let d = i64::from(mri.vg.depth);
    for f in firstframe..=lastframe {
        if !sampled_type(mri.mtype) {
            valvect[f as usize] = 0.0;
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("{WHAT}: unsupported type {}", mri.mtype),
            );
            return Ok(ERROR_UNSUPPORTED);
        }
        let sl = i64::from(f) * d + i64::from(zv);
        // a float voxel is stored as it is: the official build drops the round trip through
        // `double` (a signalling NaN stays signalling)
        valvect[f as usize] = if mri.mtype == MRI_FLOAT {
            rows::read(g, WHAT, mri, sl, i64::from(yv), i64::from(xv), MRI_FLOAT).f()
        } else {
            vox(g, WHAT, mri, sl, i64::from(yv), i64::from(xv)) as f32
        };
    }
    Ok(NO_ERROR)
}

/// `MRItrilinKernel(mri, c, r, s, kernel)`: the eight trilinear weights of a point in
/// [`Corners::eight`]'s order (the distances against the corners as `double`s, the same numbers);
/// all 0 for a point unambiguously outside.
pub fn trilin_kernel(mri: &Mri, c: f64, r: f64, s: f64) -> [f64; 8] {
    if index_not_in_volume(mri, c, r, s) == 1 {
        return [0.0; 8];
    }
    let k = Corners::of(mri, c, r, s);
    k.eight().map(|(_, _, _, w)| w)
}

/// `MRIsampleVolumeFrame_xyzInt_nRange_floats(mri, x, y, z, frameBegin, frameEnd, vals)`:
/// [`sample_volume_frame_type_xyz_int_n_range_nearest_floats`].
#[allow(clippy::too_many_arguments)]
pub fn sample_volume_frame_xyz_int_n_range_floats(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
    frame_begin: i32,
    frame_end: i32,
    vals: &mut [f32],
) -> Result<i32, Fatal> {
    sample_volume_frame_type_xyz_int_n_range_nearest_floats(
        g,
        err,
        mri,
        x,
        y,
        z,
        frame_begin,
        frame_end,
        vals,
    )
}

/// `MRIsampleVolumeFrameType_xyzInt_nRange_SAMPLE_NEAREST_floats(mri, xv, yv, zv, frameBegin,
/// frameEnd, vals)`: the voxel at a whole point of each frame `frameBegin..frameEnd` (the end
/// excluded) into `vals[frame - frameBegin]` as a `float`: `(float) outside_val` for a point
/// unambiguously outside, else the point clamped to the volume; bytes, shorts, unsigned shorts,
/// ints and floats only: another type gives 0 for every frame and the error
/// `MRIsampleVolumeFrameType: unsupported volume type <t>` (`ERROR_UNSUPPORTED`). `vals` must hold
/// the frames (else a port error: FreeSurfer writes past it).
#[allow(clippy::too_many_arguments)]
pub fn sample_volume_frame_type_xyz_int_n_range_nearest_floats(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    xv: i32,
    yv: i32,
    zv: i32,
    frame_begin: i32,
    frame_end: i32,
    vals: &mut [f32],
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIsampleVolumeFrameType";
    let n = i64::from(frame_end) - i64::from(frame_begin);
    if n > 0 && n > vals.len() as i64 {
        frames_fit(g, err, WHAT, vals.len(), 0, (n - 1) as i32)?;
    }
    let frames = frame_begin..frame_end;
    if index_not_in_volume(mri, f64::from(xv), f64::from(yv), f64::from(zv)) == 1 {
        for f in frames {
            vals[(f - frame_begin) as usize] = mri.outside_val as f32;
        }
        return Ok(NO_ERROR);
    }
    let clamp = |v: i32, n: i32| {
        let v = if v < 0 { 0 } else { v };
        if v >= n { n - 1 } else { v }
    };
    let (xv, yv, zv) = (
        clamp(xv, mri.vg.width),
        clamp(yv, mri.vg.height),
        clamp(zv, mri.vg.depth),
    );
    let ty = mri.mtype;
    if !matches!(ty, MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_INT | MRI_FLOAT) {
        for f in frames {
            vals[(f - frame_begin) as usize] = 0.0;
        }
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("{WHAT}: unsupported volume type {ty}"),
        );
        return Ok(ERROR_UNSUPPORTED);
    }
    let d = i64::from(mri.vg.depth);
    for f in frames {
        let sl = i64::from(f) * d + i64::from(zv);
        let v = rows::read(g, WHAT, mri, sl, i64::from(yv), i64::from(xv), ty);
        vals[(f - frame_begin) as usize] = v.f();
    }
    Ok(NO_ERROR)
}
