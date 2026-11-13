// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Directional derivatives, gradients and neighbour counts of `utils/mri.cpp` (3.1-36):
//! `MRIsampleVolumeDerivativeScale`, `MRIsampleVolumeDerivative`, `MRIsampleVolumeGradient`,
//! `MRIsampleVolumeGradientFrame`, `MRIneighborsOn`, `MRIneighborsOn3x3`, `MRIneighborsInWindow`,
//! `MRIneighbors`, `MRIneighborsInRange`, `MRIneighbors3x3`, `MRIneighborsOff`,
//! `MRIneighborsOff3x3`.
//!
//! The derivatives sample the volume with [`crate::sample`]'s trilinear samplers along a direction
//! and combine the samples in `double`; the official build's operand orders where NaNs meet are
//! the source's (`x += y` keeps `x`'s NaN; `docs/findings/freesurfer-volume.md` §10). The
//! neighbour counts read the boundary index tables (`xi`, `yi`, `zi`: a coordinate one past an
//! edge gives the edge) and the voxels through `MRIgetVoxVal` or, for the `Off` counts, as bytes
//! through the row pointers.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §10):
//! - **FS-322** (memory): a sample the trilinear sampler refuses (a volume of another type) leaves
//!   FreeSurfer's value never written. Both modes take 0 (the value of the last sample that wrote
//!   one, as FreeSurfer's variable keeps it), noted.
//! - **FS-323** (memory): a voxel more than 500 voxels outside the volume reads the index tables
//!   past their ends. An error in both modes.
//! - **FS-324** (hang): `MRIsampleVolumeDerivativeScale` with a scale whose double is infinite
//!   never returns. An error in both modes.

use std::io::Write;

use larmorx_core::defects::{FS_322, FS_323, FS_324};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sub64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::mri::{MAX_INDEX, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

use crate::coords::index_not_in_volume;
use crate::rows::{self, need_voxels};
use crate::sample::{sample_volume, sample_volume_frame};

/// An unused bit pattern (a signalling NaN no arithmetic makes) marking a sample not written.
const UNWRITTEN: u64 = 0x7ff4_dead_beef_0001;

/// A `double` FreeSurfer passes to a sampler that may leave it as it was: the value of the last
/// sample that wrote it, 0 before any (FS-322 noted, memory never written).
struct Slot(f64, bool);

impl Slot {
    fn new() -> Slot {
        Slot(0.0, false)
    }

    /// The value after a sampler's call `r` that wrote `v` (or left [`UNWRITTEN`]).
    fn take(&mut self, g: &mut Globals, what: &str, v: f64) -> f64 {
        if v.to_bits() == UNWRITTEN {
            if !self.1 {
                g.notices.add(
                    &FS_322,
                    format!("{what} uses a sample the trilinear sampler never wrote"),
                );
            }
        } else {
            self.0 = v;
            self.1 = true;
        }
        self.0
    }
}

/// `MRIsampleVolume(mri, x, y, z, &val)` with FreeSurfer's variable `val` ([`Slot`]).
#[allow(clippy::too_many_arguments)]
fn sample(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    slot: &mut Slot,
) -> f64 {
    let mut v = f64::from_bits(UNWRITTEN);
    sample_volume(g, err, mri, x, y, z, &mut v);
    slot.take(g, what, v)
}

/// The point clamped to `[0, extent - 1]` on each axis (`x >= width` first, then `x < 0`).
fn clamp_point(mri: &Mri, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let c = |v: f64, n: i32| {
        let v = if v >= f64::from(n) {
            f64::from(n) - 1.0
        } else {
            v
        };
        if v < 0.0 { 0.0 } else { v }
    };
    (c(x, mri.vg.width), c(y, mri.vg.height), c(z, mri.vg.depth))
}

/// macros.h's `MAX(a, b)`: `a >= b ? a : b`.
fn max(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

/// `FZERO(f)`: `fabs(f) < FLT_EPSILON`.
fn fzero(f: f64) -> bool {
    f.abs() < f64::from(f32::EPSILON)
}

/// `MRIsampleVolumeDerivativeScale(mri, x, y, z, dx, dy, dz, &mag, sigma)`: the point clamped to
/// the volume; steps of `MAX(0.25, sigma / 5)` from one step out to `MAX(2 sigma, step)`; at each
/// distance `d` the samples at `point + d dir` and `point - d dir` ([`sample_volume`]) weighted by
/// `k = exp(-d d / (2 sigma sigma))` (1 when `FZERO(sigma)`) and summed (`vp`, `vm`), the weights
/// summed (`ktotal`) and the distances (`len`); then `vm /= ktotal`, `vp /= ktotal`, `len /=
/// ktotal` and the magnitude `(vp - vm) / (2 len)` times `MRIsampleVolumeDerivativeSign`; a zero
/// step ends after the first distance. A scale whose `2 sigma` is infinite never ends in
/// FreeSurfer (FS-324): an error here.
#[allow(clippy::too_many_arguments)]
pub fn sample_volume_derivative_scale(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    dx: f64,
    dy: f64,
    dz: f64,
    pmag: &mut f64,
    sigma: f64,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIsampleVolumeDerivativeScale";
    let (x, y, z) = clamp_point(mri, x, y, z);
    let step = max(0.25, sigma / 5.0);
    if (2.0 * sigma) == f64::INFINITY {
        g.notices.add(
            &FS_324,
            format!("{WHAT} with the scale {sigma} never returns"),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!("{WHAT}: the scale {sigma} is too large (FreeSurfer never returns)\n"),
        )?;
    }
    let libm = Libm::quiet(g.mode());
    let (mut ktotal, mut len, mut vp, mut vm) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut slot = Slot::new();
    let mut dist = step;
    while dist <= max(2.0 * sigma, step) {
        let k = if fzero(sigma) {
            1.0
        } else {
            // a huge sigma makes inf / inf: SSE's default NaN, passed on by exp
            let a = div64(mul64(-dist, dist), mul64(mul64(2.0, sigma), sigma));
            let e = libm.exp(a);
            if e.is_nan() && a.is_nan() {
                f64::from_bits(a.to_bits() | 0x0008_0000_0000_0000)
            } else {
                e
            }
        };
        ktotal = add64(ktotal, k);
        len = add64(len, dist);
        let v = sample(
            g,
            err,
            WHAT,
            mri,
            add64(x, mul64(dist, dx)),
            add64(y, mul64(dist, dy)),
            add64(z, mul64(dist, dz)),
            &mut slot,
        );
        vp = add64(vp, mul64(k, v));
        let v = sample(
            g,
            err,
            WHAT,
            mri,
            sub64(x, mul64(dist, dx)),
            sub64(y, mul64(dist, dy)),
            sub64(z, mul64(dist, dz)),
            &mut slot,
        );
        vm = add64(vm, mul64(k, v));
        if fzero(step) {
            break;
        }
        dist += step;
    }
    vm = div64(vm, ktotal);
    vp = div64(vp, ktotal);
    len = div64(len, ktotal);
    *pmag = mul64(
        div64(sub64(vp, vm), mul64(2.0, len)),
        g.sample_volume_derivative_sign,
    );
    Ok(NO_ERROR)
}

/// `MRIsampleVolumeDerivative(mri, x, y, z, dx, dy, dz, &mag)`: the point clamped to the volume;
/// at the distances 0.5, 1, 1.5 and 2 the samples at `point + d dir` and `point - d dir`
/// ([`sample_volume`]) summed (`vp`, `vm`) with the distances (`len`), each divided by 4; the
/// magnitude `(vp - vm) / (2 len)` times `MRIsampleVolumeDerivativeSign`.
#[allow(clippy::too_many_arguments)]
pub fn sample_volume_derivative(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    dx: f64,
    dy: f64,
    dz: f64,
    pmag: &mut f64,
) -> i32 {
    const WHAT: &str = "MRIsampleVolumeDerivative";
    let (x, y, z) = clamp_point(mri, x, y, z);
    let (mut len, mut vp, mut vm) = (0.0f64, 0.0f64, 0.0f64);
    let mut slot = Slot::new();
    let mut n = 0i32;
    let mut dist = 0.5f64;
    while dist <= 2.0 {
        len += dist;
        let v = sample(
            g,
            err,
            WHAT,
            mri,
            add64(x, mul64(dist, dx)),
            add64(y, mul64(dist, dy)),
            add64(z, mul64(dist, dz)),
            &mut slot,
        );
        vp = add64(vp, v);
        let v = sample(
            g,
            err,
            WHAT,
            mri,
            sub64(x, mul64(dist, dx)),
            sub64(y, mul64(dist, dy)),
            sub64(z, mul64(dist, dz)),
            &mut slot,
        );
        vm = add64(vm, v);
        dist += 0.5;
        n += 1;
    }
    let n = f64::from(n);
    vm = div64(vm, n);
    vp = div64(vp, n);
    len /= n;
    *pmag = mul64(
        div64(sub64(vp, vm), 2.0 * len),
        g.sample_volume_derivative_sign,
    );
    NO_ERROR
}

/// `MRIsampleVolumeGradient(mri, x, y, z, &dx, &dy, &dz)`: on each axis the samples one voxel
/// either side ([`sample_volume`]), the point itself where that neighbour is not plainly inside
/// ([`index_not_in_volume`] not 0); `d = (plus - minus) / (2 size)` with the axis's voxel size.
#[allow(clippy::too_many_arguments)]
pub fn sample_volume_gradient(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    pdx: &mut f64,
    pdy: &mut f64,
    pdz: &mut f64,
) -> i32 {
    const WHAT: &str = "MRIsampleVolumeGradient";
    let one = |g: &mut Globals, err: &mut dyn Write, p: (f64, f64, f64)| {
        let at = if index_not_in_volume(mri, p.0, p.1, p.2) != 0 {
            (x, y, z)
        } else {
            p
        };
        sample(g, err, WHAT, mri, at.0, at.1, at.2, &mut Slot::new())
    };
    let xp1 = one(g, err, (x + 1.0, y, z));
    let xm1 = one(g, err, (x - 1.0, y, z));
    let yp1 = one(g, err, (x, y + 1.0, z));
    let ym1 = one(g, err, (x, y - 1.0, z));
    let zp1 = one(g, err, (x, y, z + 1.0));
    let zm1 = one(g, err, (x, y, z - 1.0));
    gradient(mri, [xp1, xm1, yp1, ym1, zp1, zm1], pdx, pdy, pdz);
    NO_ERROR
}

fn gradient(mri: &Mri, s: [f64; 6], pdx: &mut f64, pdy: &mut f64, pdz: &mut f64) {
    *pdx = div64(sub64(s[0], s[1]), mul64(2.0, f64::from(mri.vg.xsize)));
    *pdy = div64(sub64(s[2], s[3]), mul64(2.0, f64::from(mri.vg.ysize)));
    *pdz = div64(sub64(s[4], s[5]), mul64(2.0, f64::from(mri.vg.zsize)));
}

/// `MRIsampleVolumeGradientFrame(mri, x, y, z, &dx, &dy, &dz, frame)`: as
/// [`sample_volume_gradient`] with [`sample_volume_frame`] at the six neighbours always (no test
/// of the volume's edge).
#[allow(clippy::too_many_arguments)]
pub fn sample_volume_gradient_frame(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
    pdx: &mut f64,
    pdy: &mut f64,
    pdz: &mut f64,
    frame: i32,
) -> i32 {
    const WHAT: &str = "MRIsampleVolumeGradientFrame";
    let one = |g: &mut Globals, err: &mut dyn Write, p: (f64, f64, f64)| {
        let mut v = f64::from_bits(UNWRITTEN);
        sample_volume_frame(g, err, mri, p.0, p.1, p.2, frame, &mut v);
        Slot::new().take(g, WHAT, v)
    };
    let xp1 = one(g, err, (x + 1.0, y, z));
    let xm1 = one(g, err, (x - 1.0, y, z));
    let yp1 = one(g, err, (x, y + 1.0, z));
    let ym1 = one(g, err, (x, y - 1.0, z));
    let zp1 = one(g, err, (x, y, z + 1.0));
    let zm1 = one(g, err, (x, y, z - 1.0));
    gradient(mri, [xp1, xm1, yp1, ym1, zp1, zm1], pdx, pdy, pdz);
    NO_ERROR
}

/// Which index table a neighbour count reads.
#[derive(Clone, Copy)]
pub(crate) enum Axis {
    X,
    Y,
    Z,
}

/// `mri->xi[i]` (or `yi`, `zi`): the coordinate `i` clamped to the volume's extent on that axis;
/// past the table (more than `MAX_INDEX` outside) FreeSurfer reads memory it does not own
/// (FS-323): an error.
pub(crate) fn index(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    mri: &Mri,
    axis: Axis,
    i: i32,
) -> Result<i32, Fatal> {
    need_voxels(g, err, what, mri)?;
    let (t, n) = match axis {
        Axis::X => (&mri.xi, mri.vg.width),
        Axis::Y => (&mri.yi, mri.vg.height),
        Axis::Z => (&mri.zi, mri.vg.depth),
    };
    let t = t.as_deref().unwrap_or(&[]);
    let k = i64::from(i) + i64::from(MAX_INDEX);
    if k < 0 || k >= t.len() as i64 || i64::from(i) >= i64::from(n) + i64::from(MAX_INDEX) {
        g.notices.add(
            &FS_323,
            format!("{what} reads a boundary index table past its end (index {i})"),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{what}: the voxel index {i} lies past the volume's index tables (FreeSurfer reads memory it does not own)\n"
            ),
        )?;
    }
    Ok(Mri::index(t, i))
}

/// The six face neighbours of `(x0, y0, z0)` as `MRIneighbors*` visit them: x - 1 and x + 1
/// (through `xi`), y + 1 and y - 1 (`yi`), z + 1 and z - 1 (`zi`).
fn faces(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    mri: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
) -> Result<[(i32, i32, i32); 6], Fatal> {
    Ok([
        (index(g, err, what, mri, Axis::X, x0 - 1)?, y0, z0),
        (index(g, err, what, mri, Axis::X, x0 + 1)?, y0, z0),
        (x0, index(g, err, what, mri, Axis::Y, y0 + 1)?, z0),
        (x0, index(g, err, what, mri, Axis::Y, y0 - 1)?, z0),
        (x0, y0, index(g, err, what, mri, Axis::Z, z0 + 1)?),
        (x0, y0, index(g, err, what, mri, Axis::Z, z0 - 1)?),
    ])
}

/// The window neighbours of `(x, y, z)` (the voxel itself excluded), z outermost, then y, then
/// x, each through its index table, `-whalf..=whalf` on each axis.
#[allow(clippy::too_many_arguments)]
fn window(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
    whalf: i32,
) -> Result<Vec<(i32, i32, i32)>, Fatal> {
    let mut v = Vec::new();
    for zk in -whalf..=whalf {
        let zi = index(g, err, what, mri, Axis::Z, z + zk)?;
        for yk in -whalf..=whalf {
            let yi = index(g, err, what, mri, Axis::Y, y + yk)?;
            for xk in -whalf..=whalf {
                let xi = index(g, err, what, mri, Axis::X, x + xk)?;
                if zk == 0 && yk == 0 && xk == 0 {
                    continue;
                }
                v.push((xi, yi, zi));
            }
        }
    }
    Ok(v)
}

/// The count of `MRIgetVoxVal` values (of `frame`) at `pts` for which `test` holds.
fn count(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    pts: &[(i32, i32, i32)],
    frame: i32,
    test: &mut dyn FnMut(&mut Globals, f32) -> bool,
) -> Result<i32, Fatal> {
    let mut n = 0;
    for &(x, y, z) in pts {
        let v = mri_get_vox_val(g, err, mri, x, y, z, frame, 0)?;
        if test(g, v) {
            n += 1;
        }
    }
    Ok(n)
}

/// `MRIneighborsOn(mri, x0, y0, z0, min_val)`: the face neighbours whose value (frame 0) is at
/// least `min_val` (compared as `float`s).
pub fn neighbors_on(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
    min_val: i32,
) -> Result<i32, Fatal> {
    let p = faces(g, err, "MRIneighborsOn", mri, x0, y0, z0)?;
    count(g, err, mri, &p, 0, &mut |_, v| v >= min_val as f32)
}

/// `MRIneighborsOn3x3(mri, x, y, z, min_val)`: the 26 neighbours whose value is above `min_val`.
pub fn neighbors_on_3x3(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
    min_val: i32,
) -> Result<i32, Fatal> {
    let p = window(g, err, "MRIneighborsOn3x3", mri, x, y, z, 1)?;
    count(g, err, mri, &p, 0, &mut |_, v| v > min_val as f32)
}

/// `MRIneighborsInWindow(mri, x, y, z, wsize, val)`: the neighbours in the window of `wsize`
/// (half-width `(wsize - 1) / 2`) whose value equals `val`.
#[allow(clippy::too_many_arguments)]
pub fn neighbors_in_window(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
    wsize: i32,
    val: i32,
) -> Result<i32, Fatal> {
    let p = window(
        g,
        err,
        "MRIneighborsInWindow",
        mri,
        x,
        y,
        z,
        (wsize - 1) / 2,
    )?;
    count(g, err, mri, &p, 0, &mut |_, v| v == val as f32)
}

/// `MRIneighbors(mri, x0, y0, z0, val)`: the face neighbours whose value rounds (`nint`) to
/// `val`.
pub fn neighbors(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
    val: i32,
) -> Result<i32, Fatal> {
    let p = faces(g, err, "MRIneighbors", mri, x0, y0, z0)?;
    count(g, err, mri, &p, 0, &mut |g, v| nint(g, f64::from(v)) == val)
}

/// `MRIneighborsInRange(mri, x0, y0, z0, frame, low, high)`: the face neighbours whose value (of
/// `frame`) lies in `[low, high]`.
#[allow(clippy::too_many_arguments)]
pub fn neighbors_in_range(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
    frame: i32,
    low_val: f32,
    hi_val: f32,
) -> Result<i32, Fatal> {
    let p = faces(g, err, "MRIneighborsInRange", mri, x0, y0, z0)?;
    count(g, err, mri, &p, frame, &mut |_, v| {
        v >= low_val && v <= hi_val
    })
}

/// `MRIneighbors3x3(mri, x, y, z, val)`: the 26 neighbours whose value rounds to `val`.
pub fn neighbors_3x3(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
    val: i32,
) -> Result<i32, Fatal> {
    let p = window(g, err, "MRIneighbors3x3", mri, x, y, z, 1)?;
    count(g, err, mri, &p, 0, &mut |g, v| nint(g, f64::from(v)) == val)
}

/// The count of bytes (`MRIvox`, whatever the volume's type) at `pts` below `min_val`.
fn count_bytes_below(
    g: &mut Globals,
    what: &str,
    mri: &Mri,
    pts: &[(i32, i32, i32)],
    min_val: i32,
) -> i32 {
    let mut n = 0;
    for &(x, y, z) in pts {
        let v = rows::read(
            g,
            what,
            mri,
            i64::from(z),
            i64::from(y),
            i64::from(x),
            MRI_UCHAR,
        );
        if (v.f() as i32) < min_val {
            n += 1;
        }
    }
    n
}

/// `MRIneighborsOff(mri, x0, y0, z0, min_val)`: the face neighbours (no index tables: x - 1,
/// x + 1, y + 1, y - 1, z + 1, z - 1 as they are) whose byte (`MRIvox`, whatever the volume's
/// type) is below `min_val`; a neighbour outside the volume reads another voxel or memory
/// FreeSurfer does not own ([`crate::rows`]: FS-289, FS-290).
pub fn neighbors_off(g: &mut Globals, mri: &Mri, x0: i32, y0: i32, z0: i32, min_val: i32) -> i32 {
    let p = [
        (x0 - 1, y0, z0),
        (x0 + 1, y0, z0),
        (x0, y0 + 1, z0),
        (x0, y0 - 1, z0),
        (x0, y0, z0 + 1),
        (x0, y0, z0 - 1),
    ];
    count_bytes_below(g, "MRIneighborsOff", mri, &p, min_val)
}

/// `MRIneighborsOff3x3(mri, x, y, z, min_val)`: the 26 neighbours (through the index tables) whose
/// byte is below `min_val`.
pub fn neighbors_off_3x3(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
    min_val: i32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIneighborsOff3x3";
    let p = window(g, err, WHAT, mri, x, y, z, 1)?;
    Ok(count_bytes_below(g, WHAT, mri, &p, min_val))
}
