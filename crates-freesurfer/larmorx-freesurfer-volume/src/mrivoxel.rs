// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Voxel neighbourhood measures of `utils/mrivoxel.cpp` (3.1-38): `MRIvoxelDx`, `MRIvoxelDy`,
//! `MRIvoxelDz`, `MRIvoxelMean`, `MRIvoxelDirection` and `MRIvoxelGradientDir2ndDerivative`.
//!
//! The in-plane derivatives weigh the face neighbour twice and the two edge neighbours once on
//! each side (`float`: `e2 + (2 f + e1)`, then `(after - before) / 8`), their coordinates through
//! the volume's index tables (`mri->xi`, clamped to the volume: [`crate::deriv`]'s FS-323 past
//! them), frame 0 by `MRIgetVoxVal`.
//!
//! Defect (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §16):
//! - **FS-485** (memory): `MRIvoxelDirection`'s window stops at the volume's last column, row and
//!   slice but is read back as a full `wsize`-wide cube: within half a window of those faces the
//!   gradients pair with the wrong offsets and the last entries were never written. With
//!   `reproduce` FreeSurfer's pairing with 0 for the entries never written; by default the window
//!   is not cut on those sides either (the index tables clamp it, as on the near sides), noted.
//! - **FS-486** (memory): `MRIvoxelGradientDir2ndDerivative` subtracts samples into one `double`
//!   that a sampler refusing the volume's type never writes: the previous sample, and at the
//!   first a value never written. Both modes take 0 there, noted.

use std::io::Write;

use larmorx_core::defects::{FS_485, FS_486};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::sse::{add, div, mul, mul64, sqrt, sub, sub64};
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

use crate::deriv::{Axis, index};
use crate::sample::sample_volume;

/// `MAX_WINDOW`: the largest window `MRIvoxelDirection` takes.
const MAX_WINDOW: i32 = 7;

/// One side's weighted sum `e2 + (2 f + e1)` of frame 0 at three voxels (face, then the edges).
fn side(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    f: (i32, i32, i32),
    e1: (i32, i32, i32),
    e2: (i32, i32, i32),
) -> Result<f32, Fatal> {
    let v = |g: &mut Globals, err: &mut dyn Write, p: (i32, i32, i32)| {
        mri_get_vox_val(g, err, mri, p.0, p.1, p.2, 0, 0)
    };
    let a = v(g, err, f)?;
    let s = add(add(a, a), v(g, err, e1)?);
    Ok(add(v(g, err, e2)?, s))
}

/// The table entries `t[i - 1]`, `t[i]` (after `i = t[i]`) and `t[i + 1]`.
fn around(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    mri: &Mri,
    axis: Axis,
    i: i32,
) -> Result<(i32, i32, i32), Fatal> {
    let c = index(g, err, what, mri, axis, i)?;
    let m = index(g, err, what, mri, axis, c.wrapping_sub(1))?;
    let p = index(g, err, what, mri, axis, c.wrapping_add(1))?;
    Ok((m, c, p))
}

/// `MRIvoxelDx(mri, x, y, z)`: `(right - left) / 8` with each side's face neighbour (same row)
/// weighted twice and its edge neighbours (rows before and after) once.
pub fn mri_voxel_dx(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<f32, Fatal> {
    const WHAT: &str = "MRIvoxelDx";
    let (xm1, _, xp1) = around(g, err, WHAT, mri, Axis::X, x)?;
    let (ym1, y, yp1) = around(g, err, WHAT, mri, Axis::Y, y)?;
    let z = index(g, err, WHAT, mri, Axis::Z, z)?;
    let left = side(g, err, mri, (xm1, y, z), (xm1, ym1, z), (xm1, yp1, z))?;
    let right = side(g, err, mri, (xp1, y, z), (xp1, ym1, z), (xp1, yp1, z))?;
    Ok(mul(sub(right, left), 0.125))
}

/// `MRIvoxelDy(mri, x, y, z)`: `(bottom - top) / 8`, the column before weighted twice (as
/// FreeSurfer writes it: `2 v(x-1) + v(x) + v(x+1)` on the rows before and after).
pub fn mri_voxel_dy(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<f32, Fatal> {
    const WHAT: &str = "MRIvoxelDy";
    let (xm1, x, xp1) = around(g, err, WHAT, mri, Axis::X, x)?;
    let (ym1, _, yp1) = around(g, err, WHAT, mri, Axis::Y, y)?;
    let z = index(g, err, WHAT, mri, Axis::Z, z)?;
    let top = side(g, err, mri, (xm1, ym1, z), (x, ym1, z), (xp1, ym1, z))?;
    let bottom = side(g, err, mri, (xm1, yp1, z), (x, yp1, z), (xp1, yp1, z))?;
    Ok(mul(sub(bottom, top), 0.125))
}

/// `MRIvoxelDz(mri, x, y, z)`: `(bottom - top) / 8` over the slices before and after, the column
/// before weighted twice.
pub fn mri_voxel_dz(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<f32, Fatal> {
    const WHAT: &str = "MRIvoxelDz";
    let (xm1, x, xp1) = around(g, err, WHAT, mri, Axis::X, x)?;
    let (zm1, _, zp1) = around(g, err, WHAT, mri, Axis::Z, z)?;
    let y = index(g, err, WHAT, mri, Axis::Y, y)?;
    let top = side(g, err, mri, (xm1, y, zm1), (x, y, zm1), (xp1, y, zm1))?;
    let bottom = side(g, err, mri, (xm1, y, zp1), (x, y, zp1), (xp1, y, zp1))?;
    Ok(mul(sub(bottom, top), 0.125))
}

/// `MRIvoxelMean(mri, x0, y0, z0, wsize, frame)`: the mean (`float`, `v + total`) of the frame's
/// voxels in the `wsize` cube around the voxel cut to the volume; 0 for an empty cube.
#[allow(clippy::too_many_arguments)]
pub fn mri_voxel_mean(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
    wsize: i32,
    frame: i32,
) -> Result<f32, Fatal> {
    let whalf = wsize / 2;
    let lo = |c: i32| 0.max(c.wrapping_sub(whalf));
    let hi = |n: i32, c: i32| {
        let a = n - 1;
        let b = c.wrapping_add(whalf);
        if a < b { a } else { b }
    };
    let (zmin, zmax) = (lo(z0), hi(mri.vg.depth, z0));
    let (ymin, ymax) = (lo(y0), hi(mri.vg.height, y0));
    let (xmin, xmax) = (lo(x0), hi(mri.vg.width, x0));
    let npix = (zmax.wrapping_sub(zmin).wrapping_add(1))
        .wrapping_mul(ymax.wrapping_sub(ymin).wrapping_add(1))
        .wrapping_mul(xmax.wrapping_sub(xmin).wrapping_add(1));
    let mut total = 0.0f32;
    for z in zmin..=zmax {
        for y in ymin..=ymax {
            for x in xmin..=xmax {
                total = add(mri_get_vox_val(g, err, mri, x, y, z, frame, 0)?, total);
            }
        }
    }
    Ok(if npix > 0 {
        div(total, npix as f32)
    } else {
        0.0
    })
}

/// The window product `t1 t2` of `MRIvoxelDirection` with the official build's operand order for
/// the element's place in its row (`lane`: 0 to 3 the vector block of a row of at least four, 4
/// to 6 the single elements after it or of a shorter row), `t1 = x odx + y ody + z odz`, `t2 = dx
/// odx + dy ody + dz odz`.
fn dir_term(
    lane: usize,
    (x, y, z): (f32, f32, f32),
    d: (f32, f32, f32),
    o: (f32, f32, f32),
) -> f32 {
    let (px, py, pz) = (mul(x, o.0), mul(y, o.1), mul(z, o.2));
    let (gx, gy, gz) = (mul(d.0, o.0), mul(d.1, o.1), mul(d.2, o.2));
    match lane {
        0..=3 => mul(add(add(py, px), pz), add(add(gx, gy), gz)),
        4 => mul(add(add(gx, gy), gz), add(add(px, py), pz)),
        5 => mul(add(add(px, py), pz), add(gz, add(gx, gy))),
        _ => mul(add(add(py, px), pz), add(add(gx, gy), gz)),
    }
}

/// `MRIvoxelDirection(mri, x0, y0, z0, wsize)`: the gradients (`MRIvoxelDx`, `Dy`, `Dz`) of the
/// voxels of the `wsize` cube around the voxel, each paired with its offset: the sum (`float`,
/// `dir + term`, rows of the cube in order) of `(offset . g0) (g . g0)`, `g0` the voxel's own
/// gradient, times `1 / 27000`. A window above 7 gives 0 and the error `MRIvoxelDirection: window
/// size <w> too big`. FS-485.
pub fn mri_voxel_direction(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
    wsize: i32,
) -> Result<f32, Fatal> {
    if wsize > MAX_WINDOW {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIvoxelDirection: window size {wsize} too big"),
        );
        return Ok(0.0);
    }
    let whalf = wsize / 2;
    let rep = g.notices.reproduce();
    let cut = |n: i32, c: i32| {
        let b = c.wrapping_add(whalf);
        if rep && n - 1 < b { n - 1 } else { b }
    };
    let far = |n: i32, c: i32| whalf >= 0 && n - 1 < c.wrapping_add(whalf);
    if far(mri.vg.depth, z0) || far(mri.vg.height, y0) || far(mri.vg.width, x0) {
        g.notices.add(
            &FS_485,
            "MRIvoxelDirection's window, cut at the volume's far faces, is read as a full cube (gradients paired with the wrong offsets; entries never written: memory contents)",
        );
    }
    let (zmax, ymax, xmax) = (
        cut(mri.vg.depth, z0),
        cut(mri.vg.height, y0),
        cut(mri.vg.width, x0),
    );
    let mut win: Vec<(f32, f32, f32)> = Vec::new();
    for z in z0.wrapping_sub(whalf)..=zmax {
        for y in y0.wrapping_sub(whalf)..=ymax {
            for x in x0.wrapping_sub(whalf)..=xmax {
                let dx = mri_voxel_dx(g, err, mri, x, y, z)?;
                let dy = mri_voxel_dy(g, err, mri, x, y, z)?;
                let dz = mri_voxel_dz(g, err, mri, x, y, z)?;
                win.push((dx, dy, dz));
            }
        }
    }
    let o = (
        mri_voxel_dx(g, err, mri, x0, y0, z0)?,
        mri_voxel_dy(g, err, mri, x0, y0, z0)?,
        mri_voxel_dz(g, err, mri, x0, y0, z0)?,
    );
    let n = if whalf >= 0 {
        (2 * whalf + 1) as usize
    } else {
        0
    };
    let want = n * n * n;
    if win.len() < want {
        win.resize(want, (0.0, 0.0, 0.0));
    }
    let mut dir = 0.0f32;
    let mut k = 0usize;
    for z in -whalf..=whalf {
        for y in -whalf..=whalf {
            for (i, x) in (-whalf..=whalf).enumerate() {
                let lane = if n >= 4 { i } else { i + 4 };
                let t = dir_term(lane.min(6), (x as f32, y as f32, z as f32), win[k], o);
                dir = add(dir, t);
                k += 1;
            }
        }
    }
    Ok(mul(dir, 1.0 / (1000.0 * 27.0)))
}

/// An unused bit pattern (a signalling NaN no arithmetic makes) marking a sample not written.
const UNWRITTEN: u64 = 0x7ff4_dead_beef_0003;

/// `MRIsampleVolume` into FreeSurfer's one `double val`: the sample, or the last value the
/// sampler wrote when it writes none (a type it refuses); before any, memory contents (FS-486:
/// 0 in both modes, noted).
fn sampled(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    (x, y, z): (f64, f64, f64),
    last: &mut Option<f64>,
) -> f64 {
    let mut v = f64::from_bits(UNWRITTEN);
    sample_volume(g, err, mri, x, y, z, &mut v);
    if v.to_bits() != UNWRITTEN {
        *last = Some(v);
        return v;
    }
    match *last {
        Some(p) => p,
        None => {
            g.notices.add(
                &FS_486,
                "MRIvoxelGradientDir2ndDerivative uses a sample its sampler never wrote (memory contents in FreeSurfer)",
            );
            0.0
        }
    }
}

/// `MRIvoxelGradientDir2ndDerivative(mri, x0, y0, z0, wsize)`: the voxel's gradient `g0`
/// (`MRIvoxelDx`, `Dy`, `Dz`), its length (`float`, `sqrt(dx^2 + dy^2 + dz^2)`; below
/// `1e-5`, `mrivoxel.cpp`'s `FZERO`, the result is 0), the unit direction; then `(wsize - 1) v0` (the voxel's
/// `MRIsampleVolume`) minus the samples at `p0 +- d u` for `d = 1 .. wsize / 2` (each subtraction
/// in `double` rounded to `float`), divided by `wsize`. A sample the sampler does not write (a
/// type it refuses) leaves the previous value (FS-486 before any).
pub fn mri_voxel_gradient_dir_2nd_derivative(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
    wsize: i32,
) -> Result<f32, Fatal> {
    let whalf = wsize / 2;
    let mut odx = mri_voxel_dx(g, err, mri, x0, y0, z0)?;
    let mut ody = mri_voxel_dy(g, err, mri, x0, y0, z0)?;
    let mut odz = mri_voxel_dz(g, err, mri, x0, y0, z0)?;
    let len = sqrt(add(add(mul(odx, odx), mul(ody, ody)), mul(odz, odz)));
    // FZERO with this file's FLT_EPSILON: macros.h's 1e-5 (no <float.h> before it)
    if f64::from(len.abs()) < 1e-5 {
        return Ok(0.0);
    }
    odx = div(odx, len);
    ody = div(ody, len);
    odz = div(odz, len);
    let mut last = None;
    let val = sampled(
        g,
        err,
        mri,
        (f64::from(x0), f64::from(y0), f64::from(z0)),
        &mut last,
    );
    let mut d2 = mul64(f64::from(wsize - 1), val) as f32;
    let (xf0, yf0, zf0) = (x0 as f32, y0 as f32, z0 as f32);
    let wh = whalf as f32;
    let mut d = 1.0f32;
    while d <= wh {
        let (ax, ay, az) = (mul(odx, d), mul(ody, d), mul(odz, d));
        let p = (add(xf0, ax), add(yf0, ay), add(zf0, az));
        let val = sampled(
            g,
            err,
            mri,
            (f64::from(p.0), f64::from(p.1), f64::from(p.2)),
            &mut last,
        );
        d2 = sub64(f64::from(d2), val) as f32;
        let m = (sub(xf0, ax), sub(yf0, ay), sub(zf0, az));
        let val = sampled(
            g,
            err,
            mri,
            (f64::from(m.0), f64::from(m.1), f64::from(m.2)),
            &mut last,
        );
        d2 = sub64(f64::from(d2), val) as f32;
        d = add(d, 1.0);
    }
    Ok(div(d2, wsize as f32))
}
