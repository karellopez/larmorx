// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Neighbourhoods and labels of a volume (`utils/mri.cpp`, 3.1-37's piece b):
//! `MRIsmoothParcellation`, `MRIeraseBorderPlanes`, `MRImaxInNbhd`, `MRIcountNonzeroInNbhd`,
//! `MRIareNonzeroInNbhd`, `MRIfindNearestNonzeroLocation`, the label means
//! (`MRImeanInLabelMultispectral`, `MRImeanInLabel`, `MRImeanAndStdInLabel`,
//! `MRImeanInLabelInRegion`, `MRImaxInLabelInRegion`), `MRImakePositive`,
//! `MRImarkLabelBorderVoxels`, `MRIcomputeLabelNbhd`, `MRIlabelsInNbhd`, `MRIlabelsInPlanarNbhd`,
//! `MRIlabelBoundingBox`, `MRIcountThreshInNbhd`, `MRIfillBox`, `MRImaskZero` and
//! `MRIclipBrightWM`.
//!
//! A neighbourhood `-whalf..=whalf` around a voxel goes through the volume's boundary index tables
//! (`mri->xi[x + k]`: the coordinate clamped to the volume); an index more than `MAX_INDEX` outside
//! reads past the tables (FS-323, an error). A voxel value converted to an integer label with
//! `(int)` truncates; beyond the `int` range and NaN FS-12's rule (x86-64's -2147483648 with
//! `reproduce`, clamped by default).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §12):
//! - **FS-467** (memory): the parcellation smoothing counts each neighbour's label in an array
//!   indexed by the label: a negative label counts outside it; both modes count it in a bin of its
//!   own, noted.
//! - **FS-468**: the clipping of bright white matter stores each voxel through a byte (`BUFTYPE`):
//!   a value outside [0, 255] or NaN converts as x86-64 converts it (the low byte of the 32-bit
//!   truncation; -2147483648 for NaN and beyond the `int` range) with `reproduce`, clamped (NaN 0)
//!   by default.

use std::collections::HashMap;
use std::io::Write;

use larmorx_core::defects::{FS_12, FS_467, FS_468};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_NOMEMORY, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt64, sub, sub64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_SHORT, MRI_UCHAR, Mri, MriRegion};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::{Pair, mri_clone, mri_val_range};
use crate::coords::{MRI_CORONAL, MRI_SAGITTAL};
use crate::deriv::{Axis, index};
use crate::resize::MRI_HORIZONTAL;
use crate::rows::{self, Num, need_voxels};

/// `WM_MIN_VAL` (`include/mri.h`).
pub const WM_MIN_VAL: i32 = 5;
/// `DEFAULT_DESIRED_WHITE_MATTER_VALUE` (`include/mrinorm.h`).
pub const DEFAULT_DESIRED_WHITE_MATTER_VALUE: i32 = 110;

/// `(int) x` of a voxel value (C's truncation); FS-12 beyond the `int` range and for NaN.
pub(crate) fn int_of(g: &mut Globals, what: &str, x: f64) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
        g.notices
            .add(&FS_12, format!("{what}: a value of {x} converted to int"));
        if g.notices.reproduce() {
            return i32::MIN;
        }
        return if x.is_nan() {
            0
        } else if x > 0.0 {
            i32::MAX
        } else {
            i32::MIN
        };
    }
    x.trunc() as i32
}

/// `MRIsmoothParcellation(mri, count)`: only for shorts (else the error `MRIsmoothParcellation():
/// only supported for shorts data`, `ERROR_UNSUPPORTED`, and `NULL`); a copy of the volume
/// (`MRIcopy`) in which every voxel not on the border takes, of its 26 neighbours' labels (frame
/// 0, the voxel itself not counted), the last in the order (x + 1, y + 1, z + 1), (x + 1, y + 1,
/// z), ..., (x - 1, y - 1, z - 1) that at least `count` of them carry; FS-467 for a negative label.
pub fn mri_smooth_parcellation(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    count: i32,
) -> Result<Option<Mri>, Fatal> {
    const WHAT: &str = "MRIsmoothParcellation";
    if mri.mtype != MRI_SHORT {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            "MRIsmoothParcellation(): only supported for shorts data",
        );
        return Ok(None);
    }
    let Some(mut mri2) = mri_copy(g, out, err, mri, None)?.into_new() else {
        log::error_printf(
            g,
            err,
            ERROR_NOMEMORY,
            "MRIsmoothParcellation(): error copying structre",
        );
        return Ok(None);
    };
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    if w > 2 && h > 2 && d > 2 {
        need_voxels(g, err, WHAT, mri)?;
    }
    let s = |g: &mut Globals, x: i32, y: i32, z: i32| match rows::read(
        g,
        WHAT,
        mri,
        i64::from(z),
        i64::from(y),
        i64::from(x),
        MRI_SHORT,
    ) {
        Num::I(v) => v as i16,
        Num::F(_) => 0,
    };
    let order: [(i32, i32, i32); 26] = {
        let mut v = [(0, 0, 0); 26];
        let mut n = 0;
        for di in [1, 0, -1] {
            for dj in [1, 0, -1] {
                for dk in [1, 0, -1] {
                    if (di, dj, dk) != (0, 0, 0) {
                        v[n] = (di, dj, dk);
                        n += 1;
                    }
                }
            }
        }
        v
    };
    for i in 1..w - 1 {
        for j in 1..h - 1 {
            for k in 1..d - 1 {
                let mut vals = [0i16; 26];
                for (c, &(di, dj, dk)) in order.iter().enumerate() {
                    vals[c] = s(g, i + di, j + dj, k + dk);
                }
                let mut counts: HashMap<i16, i32> = HashMap::new();
                for &v in &vals {
                    *counts.entry(v).or_insert(0) += 1;
                }
                if vals.iter().any(|&v| v < 0) {
                    g.notices.add(
                        &FS_467,
                        "the parcellation smoothing counts a negative label outside its array (counted in a bin of its own here)",
                    );
                }
                for &v in &vals {
                    if counts[&v] >= count {
                        rows::write(
                            g,
                            WHAT,
                            &mut mri2,
                            i64::from(k),
                            i64::from(j),
                            i64::from(i),
                            MRI_SHORT,
                            Num::I(i64::from(v)),
                        );
                    }
                }
            }
        }
    }
    Ok(Some(mri2))
}

/// `MRIeraseBorderPlanes(mri, border_size)`: every frame's voxels within `border_size` of each face
/// set to 0 (`MRIsetVoxVal`: the z faces, then x, then y); `NO_ERROR`.
pub fn mri_erase_border_planes(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    border: i32,
) -> Result<i32, Fatal> {
    let (w, h, d, nf) = (mri.vg.width, mri.vg.height, mri.vg.depth, mri.nframes);
    for f in 0..nf {
        for x in 0..w {
            for y in 0..h {
                for i in 0..border {
                    mri_set_vox_val(g, err, mri, x, y, i, f, 0.0, 0)?;
                    mri_set_vox_val(g, err, mri, x, y, d - i - 1, f, 0.0, 0)?;
                }
            }
        }
    }
    for f in 0..nf {
        for y in 0..h {
            for z in 0..d {
                for i in 0..border {
                    mri_set_vox_val(g, err, mri, i, y, z, f, 0.0, 0)?;
                    mri_set_vox_val(g, err, mri, w - i - 1, y, z, f, 0.0, 0)?;
                }
            }
        }
    }
    for f in 0..nf {
        for x in 0..w {
            for z in 0..d {
                for i in 0..border {
                    mri_set_vox_val(g, err, mri, x, i, z, f, 0.0, 0)?;
                    mri_set_vox_val(g, err, mri, x, h - i - 1, z, f, 0.0, 0)?;
                }
            }
        }
    }
    Ok(NO_ERROR)
}

/// A window's visitor: `true` stops the walk.
type Visit<'a> = dyn FnMut(&mut Globals, &mut dyn Write, i32, i32, i32) -> Result<bool, Fatal> + 'a;

/// Visits the window `-whalf..=whalf` around `(x, y, z)`, z outermost, then y, then x, each
/// coordinate through its index table; stops when `f` returns `true`.
#[allow(clippy::too_many_arguments)]
fn window(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    mri: &Mri,
    whalf: i32,
    (x, y, z): (i32, i32, i32),
    f: &mut Visit,
) -> Result<(), Fatal> {
    for zk in -whalf..=whalf {
        let zi = index(g, err, what, mri, Axis::Z, z + zk)?;
        for yk in -whalf..=whalf {
            let yi = index(g, err, what, mri, Axis::Y, y + yk)?;
            for xk in -whalf..=whalf {
                let xi = index(g, err, what, mri, Axis::X, x + xk)?;
                if f(g, err, xi, yi, zi)? {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

/// `MRImaxInNbhd(mri, wsize, x, y, z, frame)`: the largest voxel of the frame in the window of
/// half-width `(wsize - 1) / 2` (from -1e10, `double`; NaNs passed over), as `(int)` (FS-12 for
/// -1e10, an empty window).
#[allow(clippy::too_many_arguments)]
pub fn mri_max_in_nbhd(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    wsize: i32,
    x: i32,
    y: i32,
    z: i32,
    frame: i32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRImaxInNbhd";
    let mut max_val = -1e10f64;
    window(
        g,
        err,
        WHAT,
        mri,
        (wsize - 1) / 2,
        (x, y, z),
        &mut |g, err, xi, yi, zi| {
            let v = f64::from(mri_get_vox_val(g, err, mri, xi, yi, zi, frame, 0)?);
            if v > max_val {
                max_val = v;
            }
            Ok(false)
        },
    )?;
    Ok(int_of(g, WHAT, max_val))
}

/// `MRIcountNonzeroInNbhd(mri, wsize, x, y, z)`: how many voxels of frame 0 in the window of
/// half-width `(wsize - 1) / 2` are above 0.
pub fn mri_count_nonzero_in_nbhd(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    wsize: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Result<i32, Fatal> {
    mri_count_thresh_in_nbhd_as(g, err, "MRIcountNonzeroInNbhd", mri, wsize, (x, y, z), 0.0)
}

/// `MRIcountThreshInNbhd(mri, wsize, x, y, z, thresh)`: how many voxels of frame 0 in the window
/// of half-width `(wsize - 1) / 2` are above `thresh`.
#[allow(clippy::too_many_arguments)]
pub fn mri_count_thresh_in_nbhd(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    wsize: i32,
    x: i32,
    y: i32,
    z: i32,
    thresh: f32,
) -> Result<i32, Fatal> {
    mri_count_thresh_in_nbhd_as(
        g,
        err,
        "MRIcountThreshInNbhd",
        mri,
        wsize,
        (x, y, z),
        thresh,
    )
}

fn mri_count_thresh_in_nbhd_as(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    mri: &Mri,
    wsize: i32,
    at: (i32, i32, i32),
    thresh: f32,
) -> Result<i32, Fatal> {
    let mut total = 0;
    window(
        g,
        err,
        what,
        mri,
        (wsize - 1) / 2,
        at,
        &mut |g, err, xi, yi, zi| {
            if mri_get_vox_val(g, err, mri, xi, yi, zi, 0, 0)? > thresh {
                total += 1;
            }
            Ok(false)
        },
    )?;
    Ok(total)
}

/// `MRIareNonzeroInNbhd(mri, wsize, x, y, z)`: 1 when a voxel of frame 0 in the window of
/// half-width `(wsize - 1) / 2` is above 0 (the window stops there), else 0.
pub fn mri_are_nonzero_in_nbhd(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    wsize: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Result<i32, Fatal> {
    let mut found = 0;
    window(
        g,
        err,
        "MRIareNonzeroInNbhd",
        mri,
        (wsize - 1) / 2,
        (x, y, z),
        &mut |g, err, xi, yi, zi| {
            let hit = mri_get_vox_val(g, err, mri, xi, yi, zi, 0, 0)? > 0.0;
            if hit {
                found = 1;
            }
            Ok(hit)
        },
    )?;
    Ok(found)
}

/// `MRIfindNearestNonzeroLocation(mri, wsize, xr, yr, zr, &xv, &yv, &zv)`: the byte voxel
/// (`MRIvox`, frame 0) at `nint` of the point when above 0 (as a `float`); otherwise, of the
/// window's voxels above 0, the value of the first nearest to the point (`float` offsets
/// `index - coordinate`, `dist = sqrt(dx dx + dy dy + dz dz)` in `float`, below 100000), its place
/// into `place` when given; 0 when there is none.
#[allow(clippy::too_many_arguments)]
pub fn mri_find_nearest_nonzero_location(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    wsize: i32,
    xr: f64,
    yr: f64,
    zr: f64,
    mut place: Option<&mut (i32, i32, i32)>,
) -> Result<f32, Fatal> {
    const WHAT: &str = "MRIfindNearestNonzeroLocation";
    let (x, y, z) = (nint(g, xr), nint(g, yr), nint(g, zr));
    need_voxels(g, err, WHAT, mri)?;
    let vox = |g: &mut Globals, x: i32, y: i32, z: i32| {
        rows::read(
            g,
            WHAT,
            mri,
            i64::from(z),
            i64::from(y),
            i64::from(x),
            MRI_UCHAR,
        )
        .f()
    };
    let v0 = vox(g, x, y, z);
    if v0 > 0.0 {
        return Ok(v0);
    }
    let (mut min_dist, mut min_val) = (100000.0f32, 0.0f32);
    let whalf = (wsize - 1) / 2;
    for zk in -whalf..=whalf {
        let zi = index(g, err, WHAT, mri, Axis::Z, z + zk)?;
        let dz = sub64(f64::from(zi), zr) as f32;
        for yk in -whalf..=whalf {
            let yi = index(g, err, WHAT, mri, Axis::Y, y + yk)?;
            let dy = sub64(f64::from(yi), yr) as f32;
            for xk in -whalf..=whalf {
                let xi = index(g, err, WHAT, mri, Axis::X, x + xk)?;
                let dx = sub64(f64::from(xi), xr) as f32;
                let v = vox(g, xi, yi, zi);
                if v > 0.0 {
                    let s = add(add(mul(dx, dx), mul(dy, dy)), mul(dz, dz));
                    let dist = sqrt64(f64::from(s)) as f32;
                    if dist < min_dist {
                        if let Some(p) = place.as_deref_mut() {
                            *p = (xi, yi, zi);
                        }
                        min_dist = dist;
                        min_val = v;
                    }
                }
            }
        }
    }
    Ok(min_val)
}

/// The label of a voxel of frame 0 as the label means read it: `nint(MRIgetVoxVal(...))`.
fn label_at(
    g: &mut Globals,
    err: &mut dyn Write,
    labels: &Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<i32, Fatal> {
    let v = mri_get_vox_val(g, err, labels, x, y, z, 0, 0)?;
    Ok(nint(g, f64::from(v)))
}

/// `MRImeanInLabelMultispectral(src, labeled, label)`: a vector of the source's frames, each the
/// mean (in `double`, rounded to `float`) of the frame's voxels whose label (`nint` of the labelled
/// volume's frame 0, at the source's places, x outermost) is `label`; 0 when there is none.
pub fn mri_mean_in_label_multispectral(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    labeled: &Mri,
    label: i32,
) -> Result<Option<Matrix>, Fatal> {
    let Some(mut v) = matrix_alloc(g, out, err, src.nframes, 1, MATRIX_REAL)? else {
        return Ok(None);
    };
    for f in 0..src.nframes {
        let (mut nvox, mut mean) = (0i32, 0.0f64);
        for x in 0..src.vg.width {
            for y in 0..src.vg.height {
                for z in 0..src.vg.depth {
                    if label_at(g, err, labeled, x, y, z)? == label {
                        let val = mri_get_vox_val(g, err, src, x, y, z, f, 0)?;
                        mean = add64(mean, f64::from(val));
                        nvox += 1;
                    }
                }
            }
        }
        if nvox == 0 {
            nvox = 1;
        }
        v.set_relt(f + 1, 1, div64(mean, f64::from(nvox)) as f32);
    }
    Ok(Some(v))
}

/// The voxels of a label in a box (`None`: the whole source): the sum of their values and of
/// their `float` squares in `double`, their count, their largest value (from 0, `double`).
struct InLabel {
    mean: f64,
    var: f64,
    nvox: i32,
    max: f64,
}

#[allow(clippy::too_many_arguments)]
fn in_label(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    labeled: &Mri,
    label: i32,
    region: Option<(i32, i32, i32, i32)>,
) -> Result<InLabel, Fatal> {
    let mut r = InLabel {
        mean: 0.0,
        var: 0.0,
        nvox: 0,
        max: 0.0,
    };
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let range = |c: i32, n: i32| match region {
        Some((.., whalf)) => (c - whalf, c + whalf, n),
        None => (0, n - 1, n),
    };
    let (x0, y0, z0) = region.map_or((0, 0, 0), |(x, y, z, _)| (x, y, z));
    let (xa, xb, _) = range(x0, w);
    let (ya, yb, _) = range(y0, h);
    let (za, zb, _) = range(z0, d);
    for x in xa..=xb {
        if x < 0 || x >= w {
            continue;
        }
        for y in ya..=yb {
            if y < 0 || y >= h {
                continue;
            }
            for z in za..=zb {
                if z < 0 || z >= d {
                    continue;
                }
                if label_at(g, err, labeled, x, y, z)? == label {
                    let val = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
                    r.mean = add64(r.mean, f64::from(val));
                    r.var = add64(r.var, f64::from(mul(val, val)));
                    if f64::from(val) > r.max {
                        r.max = f64::from(val);
                    }
                    r.nvox += 1;
                }
            }
        }
    }
    if r.nvox == 0 {
        r.nvox = 1;
    }
    Ok(r)
}

/// `MRImeanInLabel(src, labeled, label)`: the mean (`double`) of frame 0's voxels whose label
/// (`nint` of the labelled volume) is `label` (x outermost); 0 when there is none.
pub fn mri_mean_in_label(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    labeled: &Mri,
    label: i32,
) -> Result<f64, Fatal> {
    let r = in_label(g, err, src, labeled, label, None)?;
    Ok(div64(r.mean, f64::from(r.nvox)))
}

/// `MRImeanAndStdInLabel(src, labeled, label, &std)`: [`mri_mean_in_label`] and, when asked,
/// `sqrt(sum_sq / n - mean mean)` (the squares in `float`, summed in `double`).
pub fn mri_mean_and_std_in_label(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    labeled: &Mri,
    label: i32,
    pstd: Option<&mut f64>,
) -> Result<f64, Fatal> {
    let r = in_label(g, err, src, labeled, label, None)?;
    let n = f64::from(r.nvox);
    let mean = div64(r.mean, n);
    if let Some(p) = pstd {
        *p = sqrt64(sub64(div64(r.var, n), mul64(mean, mean)));
    }
    Ok(mean)
}

/// `MRImeanInLabelInRegion(src, labeled, label, x0, y0, z0, whalf, &sigma)`: as
/// [`mri_mean_and_std_in_label`] over the box of half-width `whalf` around `(x0, y0, z0)` (its
/// voxels inside the source).
#[allow(clippy::too_many_arguments)]
pub fn mri_mean_in_label_in_region(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    labeled: &Mri,
    label: i32,
    x0: i32,
    y0: i32,
    z0: i32,
    whalf: i32,
    psigma: Option<&mut f64>,
) -> Result<f64, Fatal> {
    let r = in_label(g, err, src, labeled, label, Some((x0, y0, z0, whalf)))?;
    let n = f64::from(r.nvox);
    let mean = div64(r.mean, n);
    if let Some(p) = psigma {
        *p = sqrt64(sub64(div64(r.var, n), mul64(mean, mean)));
    }
    Ok(mean)
}

/// `MRImaxInLabelInRegion(src, labeled, label, x0, y0, z0, whalf)`: the largest voxel of the label
/// in the box (from 0: a label whose voxels are all negative gives 0).
#[allow(clippy::too_many_arguments)]
pub fn mri_max_in_label_in_region(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    labeled: &Mri,
    label: i32,
    x0: i32,
    y0: i32,
    z0: i32,
    whalf: i32,
) -> Result<f64, Fatal> {
    Ok(in_label(g, err, src, labeled, label, Some((x0, y0, z0, whalf)))?.max)
}

/// `MRImakePositive(src, dst)`: a copy of the source (`MRIcopy`, into `dst` when given); when its
/// smallest voxel (`MRIvalRange`) is negative, every voxel of every frame minus that minimum
/// (`float`), stored with `MRIsetVoxVal`.
pub fn mri_make_positive(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (_, fmin, _) = mri_val_range(g, err, src)?;
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => {
            if mri_copy(g, out, err, src, Some(&mut *d))?.is_null() {
                return Ok(Ret::Null);
            }
            d
        }
        None => match mri_copy(g, out, err, src, None)? {
            Ret::New(m) => fresh.insert(m),
            _ => return Ok(Ret::Null),
        },
    };
    if fmin < 0.0 {
        for f in 0..d.nframes {
            for x in 0..d.vg.width {
                for y in 0..d.vg.height {
                    for z in 0..d.vg.depth {
                        let val = sub(mri_get_vox_val(g, err, src, x, y, z, f, 0)?, fmin);
                        mri_set_vox_val(g, err, d, x, y, z, f, val, 0)?;
                    }
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRImarkLabelBorderVoxels(src, dst, label, mark, six_connected)`: `mark` (frame 0, with
/// `MRIsetVoxVal`) at each voxel whose label (`(int)` of frame 0) is `label` and one of its 26
/// neighbours' (6 when `six_connected`; through the index tables) is not, or the other way round;
/// the destination a clone of the source when none is given (`MRIclone`).
#[allow(clippy::too_many_arguments)]
pub fn mri_mark_label_border_voxels(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    label: i32,
    mark: i32,
    six_connected: bool,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRImarkLabelBorderVoxels";
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => match mri_clone(g, out, err, src, None)? {
            Ret::New(m) => fresh.insert(m),
            _ => return Ok(Ret::Null),
        },
    };
    for x in 0..src.vg.width {
        for y in 0..src.vg.height {
            for z in 0..src.vg.depth {
                let v = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
                let this_label = int_of(g, WHAT, f64::from(v));
                let mut border = false;
                'x: for xk in -1..=1 {
                    let xi = index(g, err, WHAT, src, Axis::X, x + xk)?;
                    for yk in -1..=1 {
                        let yi = index(g, err, WHAT, src, Axis::Y, y + yk)?;
                        for zk in -1i32..=1 {
                            if six_connected && xk.abs() + yk.abs() + zk.abs() != 1 {
                                continue;
                            }
                            let zi = index(g, err, WHAT, src, Axis::Z, z + zk)?;
                            let v = mri_get_vox_val(g, err, src, xi, yi, zi, 0, 0)?;
                            let that_label = int_of(g, WHAT, f64::from(v));
                            if (this_label == label) != (that_label == label) {
                                border = true;
                                break 'x;
                            }
                        }
                    }
                }
                if border {
                    mri_set_vox_val(g, err, d, x, y, z, 0, mark as f32, 0)?;
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIcomputeLabelNbhd(labels, vals, x, y, z, counts, means, whalf, max_labels)`: `counts` (and
/// `means`) zeroed, then over the window of half-width `whalf` (x outermost, through the index
/// tables) each label (`(int)` of frame 0) in `[0, max_labels)` counted and, with `vals`, its value
/// (frame 0) added to its mean in `float`; then each counted label's mean divided by its count.
/// `max_labels` is `counts.len()`; `vals` comes with the means it fills. `NO_ERROR`.
#[allow(clippy::too_many_arguments)]
pub fn mri_compute_label_nbhd(
    g: &mut Globals,
    err: &mut dyn Write,
    labels: &Mri,
    mut vals: Option<(&Mri, &mut [f32])>,
    x: i32,
    y: i32,
    z: i32,
    counts: &mut [i32],
    whalf: i32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIcomputeLabelNbhd";
    let max_labels = counts.len();
    counts.fill(0);
    if let Some((_, means)) = vals.as_mut() {
        let n = max_labels.min(means.len());
        means[..n].fill(0.0);
    }
    for xk in -whalf..=whalf {
        let xi = index(g, err, WHAT, labels, Axis::X, x + xk)?;
        for yk in -whalf..=whalf {
            let yi = index(g, err, WHAT, labels, Axis::Y, y + yk)?;
            for zk in -whalf..=whalf {
                let zi = index(g, err, WHAT, labels, Axis::Z, z + zk)?;
                let v = mri_get_vox_val(g, err, labels, xi, yi, zi, 0, 0)?;
                let label = int_of(g, WHAT, f64::from(v));
                if label < 0 || label as usize >= max_labels {
                    continue;
                }
                counts[label as usize] += 1;
                if let Some((m, means)) = vals.as_mut() {
                    let val = mri_get_vox_val(g, err, m, xi, yi, zi, 0, 0)?;
                    means[label as usize] = add(means[label as usize], val);
                }
            }
        }
    }
    if let Some((_, means)) = vals.as_mut() {
        for (l, &c) in counts.iter().enumerate() {
            if c > 0 {
                means[l] = div(means[l], c as f32);
            }
        }
    }
    Ok(NO_ERROR)
}

/// `MRIlabelsInNbhd(mri, x, y, z, whalf, label)`: how many voxels of the window (z outermost,
/// through the index tables) carry `label` (`nint` of frame 0).
#[allow(clippy::too_many_arguments)]
pub fn mri_labels_in_nbhd(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
    whalf: i32,
    label: i32,
) -> Result<i32, Fatal> {
    let mut count = 0;
    window(
        g,
        err,
        "MRIlabelsInNbhd",
        mri,
        whalf,
        (x, y, z),
        &mut |g, err, xi, yi, zi| {
            if label_at(g, err, mri, xi, yi, zi)? == label {
                count += 1;
            }
            Ok(false)
        },
    )?;
    Ok(count)
}

/// `MRIlabelsInPlanarNbhd(mri, x, y, z, whalf, label, which)`: as [`mri_labels_in_nbhd`] in the
/// plane through the voxel: horizontal (x and z through the tables, y as given), sagittal (y and
/// z), coronal (x and y); another plane gives the error `MRIlabelsInPlanarNbhd: which=<w> not
/// supported yet` (`ERROR_UNSUPPORTED`) and 0.
#[allow(clippy::too_many_arguments)]
pub fn mri_labels_in_planar_nbhd(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
    whalf: i32,
    label: i32,
    which: i32,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIlabelsInPlanarNbhd";
    let mut count = 0;
    let (outer, inner) = match which {
        MRI_HORIZONTAL => (Axis::Z, Axis::X),
        MRI_SAGITTAL => (Axis::Z, Axis::Y),
        MRI_CORONAL => (Axis::X, Axis::Y),
        _ => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("MRIlabelsInPlanarNbhd: which={which} not supported yet"),
            );
            return Ok(0);
        }
    };
    let at = |a: Axis| match a {
        Axis::X => x,
        Axis::Y => y,
        Axis::Z => z,
    };
    for ok in -whalf..=whalf {
        let o = index(g, err, WHAT, mri, outer, at(outer) + ok)?;
        for ik in -whalf..=whalf {
            let i = index(g, err, WHAT, mri, inner, at(inner) + ik)?;
            let p = match which {
                MRI_HORIZONTAL => (i, y, o),
                MRI_SAGITTAL => (x, i, o),
                _ => (o, i, z),
            };
            if label_at(g, err, mri, p.0, p.1, p.2)? == label {
                count += 1;
            }
        }
    }
    Ok(count)
}

/// `MRIlabelBoundingBox(mri, label, region)`: the smallest box holding the voxels whose label
/// (`(int)` of frame 0) is `label`; none: the start at the volume's extent and the extents `-1 -
/// extent + 1` (32-bit). `NO_ERROR`.
pub fn mri_label_bounding_box(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    label: i32,
    region: &mut MriRegion,
) -> Result<i32, Fatal> {
    region.x = mri.vg.width;
    region.y = mri.vg.height;
    region.z = mri.vg.depth;
    let (mut x1, mut y1, mut z1) = (-1, -1, -1);
    for x in 0..mri.vg.width {
        for y in 0..mri.vg.height {
            for z in 0..mri.vg.depth {
                let v = mri_get_vox_val(g, err, mri, x, y, z, 0, 0)?;
                if int_of(g, "MRIlabelBoundingBox", f64::from(v)) == label {
                    region.x = region.x.min(x);
                    region.y = region.y.min(y);
                    region.z = region.z.min(z);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                    z1 = z1.max(z);
                }
            }
        }
    }
    region.dx = x1.wrapping_sub(region.x).wrapping_add(1);
    region.dy = y1.wrapping_sub(region.y).wrapping_add(1);
    region.dz = z1.wrapping_sub(region.z).wrapping_add(1);
    Ok(NO_ERROR)
}

/// `MRIfillBox(mri, box, fillval)`: frame 0's voxels of the box (cut to the volume; its end
/// `start + extent - 1` in 32-bit arithmetic) set to `fillval` (`MRIsetVoxVal`); `NO_ERROR`.
pub fn mri_fill_box(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    b: &MriRegion,
    fillval: f32,
) -> Result<i32, Fatal> {
    let end = |s: i32, d: i32| s.wrapping_add(d).wrapping_sub(1);
    let (xmin, xmax) = (b.x.max(0), (mri.vg.width - 1).min(end(b.x, b.dx)));
    let (ymin, ymax) = (b.y.max(0), (mri.vg.height - 1).min(end(b.y, b.dy)));
    let (zmin, zmax) = (b.z.max(0), (mri.vg.depth - 1).min(end(b.z, b.dz)));
    for x in xmin..=xmax {
        for y in ymin..=ymax {
            for z in zmin..=zmax {
                mri_set_vox_val(g, err, mri, x, y, z, 0, fillval, 0)?;
            }
        }
    }
    Ok(NO_ERROR)
}

/// `MRImaskZero(src, mask, dst)`: frame 0 of the destination (a clone of the source when none is
/// given) is the source's voxel where the mask's (frame 0) is above 0, else 0, over the source's
/// extent (`MRIgetVoxVal`, `MRIsetVoxVal`).
pub fn mri_mask_zero(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    mask: &Mri,
) -> Result<Ret<Mri>, Fatal> {
    /// The loop; `src` `None`: the destination is the source.
    fn run(
        g: &mut Globals,
        err: &mut dyn Write,
        src: Option<&Mri>,
        dst: &mut Mri,
        mask: &Mri,
    ) -> Result<(), Fatal> {
        let s = src.unwrap_or(dst);
        let (w, h, d) = (s.vg.width, s.vg.height, s.vg.depth);
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    let v = if mri_get_vox_val(g, err, mask, x, y, z, 0, 0)? > 0.0 {
                        mri_get_vox_val(g, err, src.unwrap_or(dst), x, y, z, 0, 0)?
                    } else {
                        0.0
                    };
                    mri_set_vox_val(g, err, dst, x, y, z, 0, v, 0)?;
                }
            }
        }
        Ok(())
    }
    match p {
        Pair::New(s) => match mri_clone(g, out, err, s, None)? {
            Ret::New(mut m) => {
                run(g, err, Some(s), &mut m, mask)?;
                Ok(Ret::New(m))
            }
            _ => Ok(Ret::Null),
        },
        Pair::Given(s, d) => {
            run(g, err, Some(s), d, mask)?;
            Ok(Ret::Given)
        }
        Pair::Same(d) => {
            run(g, err, None, d, mask)?;
            Ok(Ret::Given)
        }
    }
}

/// `(BUFTYPE)` of a voxel value: x86-64's low byte of the 32-bit truncation with `reproduce`
/// (-2147483648 for NaN and beyond `int`), clamped to [0, 255] (NaN 0) by default (FS-468).
fn to_buftype(g: &mut Globals, v: f32) -> u8 {
    let d = f64::from(v);
    if !d.is_nan() && (0.0..256.0).contains(&d) {
        return d as u8;
    }
    let x86 = if d.is_nan() || !(-2147483649.0..2147483648.0).contains(&d) {
        i32::MIN as u8
    } else {
        (d.trunc() as i32) as u8
    };
    let clamped = if d.is_nan() || d < 0.0 { 0 } else { 255 };
    if x86 != clamped {
        g.notices.add(
            &FS_468,
            format!("MRIclipBrightWM stores a value of {d} through a byte"),
        );
    }
    if g.notices.reproduce() { x86 } else { clamped }
}

/// `MRIclipBrightWM(T1, wm)`: each voxel of the T1 volume's frame 0 (its extent; `MRIgetVoxVal`,
/// stored as a byte, FS-468, then back with `MRIsetVoxVal`), lowered to 110 where the white-matter
/// volume's byte (`MRIvox`, row by row) is at least 5 and the value above 110; then
/// `MRIclipBrightWM(): nthresh=<n>, wmmin=5, clip=110 ` on standard output. `NO_ERROR`.
pub fn mri_clip_bright_wm(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t1: &mut Mri,
    wm: &Mri,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIclipBrightWM";
    let (w, h, d) = (t1.vg.width, t1.vg.height, t1.vg.depth);
    let mut n = 0;
    for z in 0..d {
        for y in 0..h {
            need_voxels(g, err, WHAT, wm)?;
            rows::touch(g, WHAT, wm, i64::from(z), i64::from(y));
            for x in 0..w {
                let v = mri_get_vox_val(g, err, t1, x, y, z, 0, 0)?;
                let mut val = to_buftype(g, v);
                let wmv = match rows::read(
                    g,
                    WHAT,
                    wm,
                    i64::from(z),
                    i64::from(y),
                    i64::from(x),
                    MRI_UCHAR,
                ) {
                    Num::I(b) => b,
                    Num::F(_) => 0,
                };
                if wmv >= i64::from(WM_MIN_VAL)
                    && i32::from(val) > DEFAULT_DESIRED_WHITE_MATTER_VALUE
                {
                    n += 1;
                    val = DEFAULT_DESIRED_WHITE_MATTER_VALUE as u8;
                }
                mri_set_vox_val(g, err, t1, x, y, z, 0, f32::from(val), 0)?;
            }
        }
    }
    let _ = writeln!(
        out,
        "MRIclipBrightWM(): nthresh={n}, wmmin={WM_MIN_VAL}, clip={DEFAULT_DESIRED_WHITE_MATTER_VALUE} "
    );
    Ok(NO_ERROR)
}
