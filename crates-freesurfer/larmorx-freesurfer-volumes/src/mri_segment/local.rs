// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_segment`'s own functions (`mri_segment/mri_segment.cpp` 706-1275): the removal of voxels
//! whose intensity grows outwards (`MRIremoveWrongDirection`), the basal ganglia's and the
//! ventricles' filling (`MRIfillBasalGanglia`, `MRIfillVentricles`), the diagonal connections'
//! repair (`MRIfilterMorphology`, `is_diagonal`), the removal of one-dimensional strands
//! (`MRIremove1dStructures`) and the recovery of bright white matter (`MRIrecoverBrightWhite`).
//!
//! Every volume here is `mri_segment`'s: bytes (the input is changed to bytes first) of the
//! input's extent, read through `larmorx_freesurfer_filter::polv`'s accessors (`MRIgetVoxVal`,
//! the boundary index tables).

use std::io::Write;

use larmorx_core::defects::{FS_1227, FS_1228};
use larmorx_freesurfer_core::cstd::{fmt_f, format_double};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE, DIAG_WRITE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_filter::conv::{mri_convolve_gaussian, mri_gaussian1d};
use larmorx_freesurfer_filter::polv::{Ctx, tables, vox_val};
use larmorx_freesurfer_filter::segment::{
    mri_segment, mri_segment_dilate_threshold, mri_segment_to_image,
};
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, Mri};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volume::arith::{
    Pair, Tri, mri_binarize, mri_clear, mri_clone, mri_replace_values,
};
use larmorx_freesurfer_volume::coords::voxel_to_talairach;
use larmorx_freesurfer_volume::labels::WM_MIN_VAL;
use larmorx_freesurfer_volume::mriset::{mri_close, mri_complement, mri_union};
use larmorx_freesurfer_volume::mrivoxel::mri_voxel_direction;

/// `DIAGONAL_FILL`, `BASAL_GANGLIA_FILL`, `VENTRICLE_FILL` (`include/mri.h`).
pub const DIAGONAL_FILL: i32 = 230;
const BASAL_GANGLIA_FILL: i32 = 50;
const VENTRICLE_FILL: i32 = 220;

/// The status of a process ended by `SIGSEGV`, as a shell reports it.
const SEGFAULT: i32 = 139;

/// `%2.2f%%` of `100.0f * (float) n / (float) (width * height * depth)`.
fn pct(n: i32, total: i32) -> String {
    fmt_f(f64::from(100.0f32 * n as f32 / total as f32), 2, 2)
}

fn pg(x: f32) -> String {
    format_double("%g", f64::from(x)).unwrap_or_default()
}

/// `MRIgetVoxVal(m, m->xi[x], m->yi[y], m->zi[z], 0)`.
fn at(g: &mut Globals, err: &mut dyn Write, m: &Mri, x: i32, y: i32, z: i32) -> Result<f32, Fatal> {
    let (xi, yi, zi) = tables(m, x, y, z);
    vox_val(g, err, m, xi, yi, zi)
}

/// `(int)` of a voxel's value (`int val = MRIgetVoxVal(...)`; `BUFTYPE` the same for a byte
/// volume's values).
fn ival(v: f32) -> i32 {
    v as i32
}

/// `MRIremoveWrongDirection(mri, mri, wsize, low_thresh, hi_thresh, NULL)` (the volume its own
/// destination): `MRIremoveWrongDirection() wsize=<d>, lowthr=<g>, hithr=<g>` and `  smoothing
/// input volume with sigma = <%2.3f>` on standard output; the volume smoothed into a clone
/// (`MRIgaussian1d(blur_sigma, 100)`, `MRIconvolveGaussian`); each voxel (as an `int`) within
/// `[low_thresh, hi_thresh]` tested, and set to 0 where the smoothed volume's direction
/// (`MRIvoxelDirection`, `wsize`) is above 0; the counts tested and changed.
#[allow(clippy::too_many_arguments)]
pub fn remove_wrong_direction(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &mut Mri,
    wsize: i32,
    low_thresh: f32,
    hi_thresh: f32,
    blur_sigma: f32,
) -> Result<(), Fatal> {
    let _ = writeln!(
        out,
        "MRIremoveWrongDirection() wsize={wsize}, lowthr={}, hithr={}",
        pg(low_thresh),
        pg(hi_thresh)
    );
    let _ = writeln!(
        out,
        "  smoothing input volume with sigma = {}",
        fmt_f(f64::from(blur_sigma), 2, 3)
    );
    let _ = out.flush();
    let kernel = mri_gaussian1d(g, out, err, blur_sigma, 100)?;
    let mut smooth = mri_clone(g, out, err, mri, None)?.unwrap_new();
    mri_convolve_gaussian(g, out, err, Pair::Given(mri, &mut smooth), &kernel)?;
    drop(kernel);
    let (width, height, depth) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let (mut nchanged, mut ntested) = (0i32, 0i32);
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let mut val = ival(vox_val(g, err, mri, x, y, z)?);
                if val as f32 >= low_thresh && val as f32 <= hi_thresh {
                    ntested += 1;
                    let dir = mri_voxel_direction(g, err, &smooth, x, y, z, wsize)?;
                    if f64::from(dir) > 0.0 {
                        nchanged += 1;
                        val = 0;
                    }
                }
                mri_set_vox_val(g, err, mri, x, y, z, 0, val as f32, 0)?;
            }
        }
    }
    let total = width.wrapping_mul(height).wrapping_mul(depth);
    let _ = writeln!(out, " {ntested:8} voxels tested ({}%)", pct(ntested, total));
    let _ = writeln!(
        out,
        " {nchanged:8} voxels changed ({}%)",
        pct(nchanged, total)
    );
    Ok(())
}

/// `MRIrecoverBrightWhite(mri_T1, mri, mri, wm_low, wm_hi, slack, pct_thresh)` (the labels their
/// own destination): its parameters on standard output (`intensity_thresh = wm_hi + slack`,
/// `nvox_thresh = 26 pct_thresh`, `float`); each voxel whose T1 byte lies in `(wm_hi,
/// intensity_thresh]` and whose label is below `WM_MIN_VAL` takes the T1's value when at least
/// `nvox_thresh` of its 27 T1 bytes (the index tables; itself included) lie in `[wm_low, wm_hi]`;
/// the counts tested and changed.
#[allow(clippy::too_many_arguments)]
pub fn recover_bright_white(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t1: &Mri,
    mri: &mut Mri,
    wm_low: f32,
    wm_hi: f32,
    slack: f32,
    pct_thresh: f32,
) -> Result<(), Fatal> {
    let _ = writeln!(out, "MRIrecoverBrightWhite()");
    let (width, height, depth) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let (mut ntested, mut nchanged) = (0i32, 0i32);
    let intensity_thresh = sse::add(wm_hi, slack);
    let nvox_thresh = (3 * 3 * 3 - 1) as f32 * pct_thresh;
    let _ = writeln!(out, " wm_low {}", pg(wm_low));
    let _ = writeln!(out, " wm_hi {}", pg(wm_hi));
    let _ = writeln!(out, " slack {}", pg(slack));
    let _ = writeln!(out, " pct_thresh {}", pg(pct_thresh));
    let _ = writeln!(out, " intensity_thresh {}", pg(intensity_thresh));
    let _ = writeln!(out, " nvox_thresh {}", pg(nvox_thresh));
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let val = f32::from(byte(vox_val(g, err, t1, x, y, z)?));
                if val > wm_hi
                    && val <= intensity_thresh
                    && vox_val(g, err, mri, x, y, z)? < WM_MIN_VAL as f32
                {
                    ntested += 1;
                    let mut nwhite = 0i32;
                    for zk in -1..=1 {
                        for yk in -1..=1 {
                            for xk in -1..=1 {
                                let (xi, yi, zi) = tables(mri, x + xk, y + yk, z + zk);
                                let v = f32::from(byte(vox_val(g, err, t1, xi, yi, zi)?));
                                if v >= wm_low && v <= wm_hi {
                                    nwhite += 1;
                                }
                            }
                        }
                    }
                    if nwhite as f32 >= nvox_thresh {
                        nchanged += 1;
                        let v = vox_val(g, err, t1, x, y, z)?;
                        mri_set_vox_val(g, err, mri, x, y, z, 0, v, 0)?;
                    }
                }
            }
        }
    }
    let total = width.wrapping_mul(height).wrapping_mul(depth);
    let _ = writeln!(out, " {ntested:8} voxels tested ({}%)", pct(ntested, total));
    let _ = writeln!(
        out,
        " {nchanged:8} voxels changed ({}%)",
        pct(nchanged, total)
    );
    Ok(())
}

/// `BUFTYPE val = MRIgetVoxVal(...)` of a byte volume's value (0 to 255: the value itself).
fn byte(v: f32) -> u8 {
    v as u8
}

/// `MRIremove1dStructures(mri, mri, max_iter, thresh, labels)` (the volume its own source and
/// destination): `MRIremove1dStructures(): max_iter=<d>, thresh=<d>, WM_MIN_VAL=5` on standard
/// output; then passes over the volume (slices outer), each voxel at or above `WM_MIN_VAL` with
/// fewer than `thresh` face neighbours inside the volume at or above it set to 0 (255 in
/// `labels`), as the pass reaches it (a voxel removed earlier in the pass counts as removed), until
/// a pass removes none (`max_iter`, 1,000 for 0, only counts the passes once none removes:
/// FreeSurfer's loop ends after the first such pass); `<n> sparsely connected voxels removed in
/// <i> iterations`.
pub fn remove_1d_structures(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &mut Mri,
    max_iter: i32,
    thresh: i32,
    mut labels: Option<&mut Mri>,
) -> Result<(), Fatal> {
    let (width, height, depth) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let max_iter = if max_iter == 0 { 1000 } else { max_iter };
    let _ = writeln!(
        out,
        "MRIremove1dStructures(): max_iter={max_iter}, thresh={thresh}, WM_MIN_VAL={WM_MIN_VAL}"
    );
    let _ = out.flush();
    let (mut niter, mut total) = (0i32, 0i32);
    loop {
        let mut nvox = 0i32;
        for z in 0..depth {
            for y in 0..height {
                for x in 0..width {
                    let val = byte(vox_val(g, err, mri, x, y, z)?);
                    if i32::from(val) < WM_MIN_VAL {
                        continue;
                    }
                    let mut nsix = 0i32;
                    for (xk, yk, zk) in [
                        (0, 0, -1),
                        (0, -1, 0),
                        (-1, 0, 0),
                        (1, 0, 0),
                        (0, 1, 0),
                        (0, 0, 1),
                    ] {
                        let (xi, yi, zi) = (x + xk, y + yk, z + zk);
                        if zi < 0 || zi >= depth || yi < 0 || yi >= height || xi < 0 || xi >= width
                        {
                            continue;
                        }
                        if vox_val(g, err, mri, xi, yi, zi)? >= WM_MIN_VAL as f32 {
                            nsix += 1;
                        }
                    }
                    if nsix < thresh {
                        nvox += 1;
                        mri_set_vox_val(g, err, mri, x, y, z, 0, 0.0, 0)?;
                        if let Some(l) = labels.as_deref_mut() {
                            mri_set_vox_val(g, err, l, x, y, z, 0, 255.0, 0)?;
                        }
                    }
                }
            }
        }
        total += nvox;
        // while (nvox > 0 || ++niter > max_iter)
        if nvox > 0 {
            continue;
        }
        niter = niter.wrapping_add(1);
        if niter > max_iter {
            continue;
        }
        break;
    }
    let _ = writeln!(
        out,
        " {total} sparsely connected voxels removed in {niter} iterations"
    );
    let _ = out.flush();
    Ok(())
}

/// `is_diagonal(mri, x, y, z)`: whether an edge or corner neighbour (the index tables) is not 0
/// while none of the face neighbours between them is at or above `WM_MIN_VAL`.
fn is_diagonal(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    x: i32,
    y: i32,
    z: i32,
) -> Result<bool, Fatal> {
    for zk in -1i32..=1 {
        for yk in -1i32..=1 {
            for xk in -1i32..=1 {
                if xk.abs() + yk.abs() + zk.abs() <= 1 {
                    continue;
                }
                let (xi, yi, zi) = tables(mri, x + xk, y + yk, z + zk);
                if vox_val(g, err, mri, xi, yi, zi)? == 0.0 {
                    continue;
                }
                if xk != 0 && vox_val(g, err, mri, xi, y, z)? >= WM_MIN_VAL as f32 {
                    continue;
                }
                if yk != 0 && vox_val(g, err, mri, x, yi, z)? >= WM_MIN_VAL as f32 {
                    continue;
                }
                if zk != 0 && vox_val(g, err, mri, x, y, zi)? >= WM_MIN_VAL as f32 {
                    continue;
                }
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// `MRIfilterMorphology(mri, mri)` (the volume its own source and destination):
/// `MRIfilterMorphology() WM_MIN_VAL=5, DIAGONAL_FILL=230` on standard output; then passes
/// (slices outer) until one adds none: at each voxel at or above `WM_MIN_VAL`, each edge or
/// corner neighbour (the index tables) at or above it with none of the face neighbours between
/// them at or above it gets those face neighbours set to `DIAGONAL_FILL` (230), each set back to
/// 0 when that makes it a diagonal itself ([`is_diagonal`]), else counted; `<n> diagonally
/// connected voxels added...` on standard error.
pub fn filter_morphology(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &mut Mri,
) -> Result<(), Fatal> {
    let _ = writeln!(
        out,
        "MRIfilterMorphology() WM_MIN_VAL={WM_MIN_VAL}, DIAGONAL_FILL={DIAGONAL_FILL}"
    );
    let (width, height, depth) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let mut total = 0i32;
    let min = WM_MIN_VAL as f32;
    loop {
        let mut nvox = 0i32;
        for z in 0..depth {
            for y in 0..height {
                for x in 0..width {
                    let val = byte(vox_val(g, err, mri, x, y, z)?);
                    if i32::from(val) < WM_MIN_VAL {
                        continue;
                    }
                    for zk in -1i32..=1 {
                        for yk in -1i32..=1 {
                            for xk in -1i32..=1 {
                                if xk.abs() + yk.abs() + zk.abs() <= 1 {
                                    continue;
                                }
                                let (xi, yi, zi) = tables(mri, x + xk, y + yk, z + zk);
                                if vox_val(g, err, mri, xi, yi, zi)? < min {
                                    continue;
                                }
                                if xk != 0 && vox_val(g, err, mri, xi, y, z)? >= min {
                                    continue;
                                }
                                if yk != 0 && vox_val(g, err, mri, x, yi, z)? >= min {
                                    continue;
                                }
                                if zk != 0 && vox_val(g, err, mri, x, y, zi)? >= min {
                                    continue;
                                }
                                for (on, (px, py, pz)) in
                                    [(xk, (xi, y, z)), (yk, (x, yi, z)), (zk, (x, y, zi))]
                                {
                                    if on == 0 {
                                        continue;
                                    }
                                    mri_set_vox_val(
                                        g,
                                        err,
                                        mri,
                                        px,
                                        py,
                                        pz,
                                        0,
                                        DIAGONAL_FILL as f32,
                                        0,
                                    )?;
                                    if is_diagonal(g, err, mri, px, py, pz)? {
                                        mri_set_vox_val(g, err, mri, px, py, pz, 0, 0.0, 0)?;
                                    } else {
                                        nvox += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        total += nvox;
        if nvox <= 0 {
            break;
        }
    }
    let _ = writeln!(err, "{total} diagonally connected voxels added...");
    Ok(())
}

/// `MRIvoxelToTalairach(mri, x, y, z, &xt, &yt, &zt)` into `t`: FS-1228 for a volume that is not
/// coronal (FreeSurfer's message, then coordinates it never computed: by default an error, with
/// `reproduce` the earlier ones, 0 at first).
#[allow(clippy::too_many_arguments)]
fn talairach(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &mut Mri,
    x: f64,
    y: f64,
    z: f64,
    t: &mut [f64; 3],
    what: &str,
) -> Result<(), Fatal> {
    let (_, p) = voxel_to_talairach(g, out, err, mri, x, y, z)?;
    match p {
        Some(p) => *t = p,
        None => {
            g.notices.add(
                &FS_1228,
                format!(
                    "mri_segment {what}: Talairach coordinates of a volume that is not coronal, never computed by FreeSurfer: 0"
                ),
            );
            if !g.notices.reproduce() {
                log::error_exit(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    &format!("mri_segment: {what} needs a coronal volume"),
                )?;
            }
        }
    }
    Ok(())
}

/// `MRIfillBasalGanglia(mri_T1, mri_dst)`: each T1 voxel within `[85, 105]` whose 7 x 7 x 7
/// neighbourhood (the index tables) lies within `[85, 110]` marked `BASAL_GANGLIA_FILL` (50) in a
/// clone; the marks closed (`MRIclose`), their segments (`MRIsegment(1, 255)`; `segmenting thick
/// gray regions: <n> 7 mm segments found` on standard error) dilated 14 times into the T1's
/// voxels within `[80, 100]` (`MRIsegmentDilateThreshold`); the clone cleared, each segment whose
/// centroid's Talairach point (`MRIvoxelToTalairach`) lies within 25 of (-30, 5, 5) or (30, 5, 5)
/// (`float` distances) written into it (`MRIsegmentToImage`; `filling segment <i> with <n>
/// voxels` and its centre on standard error), binarised to 0 and 50 (`MRIbinarize` at
/// `WM_MIN_VAL`) and joined to the labels (`MRIunion`, frame 0's bytes). With `DIAG_SHOW`: `<n>
/// basal ganglia points filled`.
pub fn fill_basal_ganglia(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t1: &mut Mri,
    dst: &mut Mri,
) -> Result<(), Fatal> {
    const WSIZE: i32 = 7;
    const WHALF: i32 = (WSIZE - 1) / 2;
    let mut bg = mri_clone(g, out, err, t1, None)?.unwrap_new();
    let (width, height, depth) = (t1.vg.width, t1.vg.height, t1.vg.depth);
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let val0 = ival(vox_val(g, err, t1, x, y, z)?);
                let mut fill = false;
                if (85..=105).contains(&val0) {
                    fill = true;
                    'outer: for zk in -WHALF..=WHALF {
                        for yk in -WHALF..=WHALF {
                            for xk in -WHALF..=WHALF {
                                let val = ival(at(g, err, t1, x + xk, y + yk, z + zk)?);
                                if !(85..=110).contains(&val) {
                                    fill = false;
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
                if fill {
                    mri_set_vox_val(g, err, &mut bg, x, y, z, 0, BASAL_GANGLIA_FILL as f32, 0)?;
                }
            }
        }
    }
    mri_close(g, out, err, Pair::Same(&mut bg))?;
    let mut mriseg = mri_segment(g, out, err, &bg, 1.0, 255.0)?;
    let _ = writeln!(
        err,
        "segmenting thick gray regions: {} {WSIZE} mm segments found",
        mriseg.nsegments
    );
    for _ in 0..2 * WSIZE {
        mri_segment_dilate_threshold(g, out, err, &mut mriseg, t1, t1, 80, 100)?;
    }
    mri_clear(g, err, &mut bg)?;
    let mut total_filled = 0i32;
    let mut t = [0f64; 3];
    for i in 0..mriseg.nsegments {
        let (cx, cy, cz, nvoxels) = {
            let m = &mriseg.segments[i as usize];
            (m.cx, m.cy, m.cz, m.voxels.len() as i32)
        };
        talairach(
            g,
            out,
            err,
            t1,
            f64::from(cx),
            f64::from(cy),
            f64::from(cz),
            &mut t,
            "-fillbg",
        )?;
        let [tx, ty, tz] = t;
        let dx_left = (tx - -30.0) as f32;
        let dx_right = (tx - 30.0) as f32;
        let dy = (ty - 5.0) as f32;
        let dz = (tz - 5.0) as f32;
        let dist_left = sse::sqrt(dx_left * dx_left + dy * dy + dz * dz);
        let dist_right = sse::sqrt(dx_right * dx_right + dy * dy + dz * dz);
        if dist_left > 25.0 && dist_right > 25.0 {
            continue;
        }
        let f1 = |v: f64| fmt_f(v, 2, 1);
        let _ = writeln!(
            err,
            "filling segment {i} with {nvoxels} voxels\n\tc = ({},{},{}) tal ({},{},{}), dist {},{}",
            f1(f64::from(cx)),
            f1(f64::from(cy)),
            f1(f64::from(cz)),
            f1(tx),
            f1(ty),
            f1(tz),
            f1(f64::from(dist_left)),
            f1(f64::from(dist_right))
        );
        mri_segment_to_image(g, out, err, t1, Some(&mut bg), &mriseg, i)?;
        total_filled += nvoxels;
    }
    mri_binarize(
        g,
        out,
        err,
        Pair::Same(&mut bg),
        WM_MIN_VAL as f32,
        0.0,
        BASAL_GANGLIA_FILL as f32,
    )?;
    drop(mriseg);
    mri_union(g, out, err, Tri::First(dst, &bg))?;
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(err, "{total_filled} basal ganglia points filled");
    }
    Ok(())
}

/// `MRIfillVentricles(mri, mri)` (the labels their own source and destination): a copy with
/// `VENTRICLE_FILL` (220) replaced by 219; each slice flood-filled with 220 from its first voxel
/// through the 8 in-slice neighbours (the index tables) of value 0; the copy complemented
/// (`MRIcomplement`) and 1 replaced by 220; its segments (`MRIsegment(1, 255)`; `<n> segments
/// found...` on standard error) of at least 100 voxels spanning at least 7 slices written into a
/// volume of ventricles (`MRIsegmentToImage`; `added segment ...` and its Talairach centre on
/// standard error); the ventricles' voxels within 5 mm of the Talairach midline set to 0; the
/// ventricles joined to the labels (`MRIunion`, frame 0's bytes). FS-1227: with no such segment
/// FreeSurfer crashes; by default nothing is added. With `DIAG_WRITE` and `DIAG_VERBOSE`:
/// `exterior_filled.mgh` and `ventricles.mgh`.
pub fn fill_ventricles(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: Ctx,
    mri: &mut Mri,
) -> Result<(), Fatal> {
    let (width, height, depth) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let mut filled = mri_copy(g, out, err, mri, None)?.unwrap_new();
    mri_replace_values(
        g,
        out,
        err,
        Pair::Same(&mut filled),
        VENTRICLE_FILL as f32,
        (VENTRICLE_FILL - 1) as f32,
    )?;
    for z in 0..depth {
        loop {
            let mut nfilled = 0i32;
            mri_set_vox_val(g, err, &mut filled, 0, 0, z, 0, VENTRICLE_FILL as f32, 0)?;
            for y in 0..height {
                for x in 0..width {
                    if vox_val(g, err, &filled, x, y, z)? != VENTRICLE_FILL as f32 {
                        continue;
                    }
                    for yk in -1..=1 {
                        for xk in -1..=1 {
                            let (xi, yi, _) = tables(mri, x + xk, y + yk, z);
                            if vox_val(g, err, &filled, xi, yi, z)? == 0.0 {
                                nfilled += 1;
                                mri_set_vox_val(
                                    g,
                                    err,
                                    &mut filled,
                                    xi,
                                    yi,
                                    z,
                                    0,
                                    VENTRICLE_FILL as f32,
                                    0,
                                )?;
                            }
                        }
                    }
                }
            }
            if nfilled <= 0 {
                break;
            }
        }
    }
    mri_complement(g, out, err, Pair::Same(&mut filled))?;
    mri_replace_values(
        g,
        out,
        err,
        Pair::Same(&mut filled),
        1.0,
        VENTRICLE_FILL as f32,
    )?;
    let mriseg = mri_segment(g, out, err, &filled, 1.0, 255.0)?;
    let _ = writeln!(err, "{} segments found...", mriseg.nsegments);
    let diag = g.gdiag & DIAG_WRITE != 0 && g.gdiag & DIAG_VERBOSE != 0;
    if diag {
        mri_write(
            g,
            out,
            err,
            ctx.env,
            ctx.dir,
            &mut filled,
            "exterior_filled.mgh",
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
    }
    let mut ventricles: Option<Mri> = None;
    let mut t = [0f64; 3];
    for s in 0..mriseg.nsegments {
        let m = &mriseg.segments[s as usize];
        let (nvox, z0, z1) = (m.voxels.len() as i32, i32::from(m.z0), i32::from(m.z1));
        if nvox < 100 || z1 - z0 < 7 {
            continue;
        }
        let (cx, cy, cz) = (m.cx, m.cy, m.cz);
        let (x0, x1, y0, y1) = (m.x0, m.x1, m.y0, m.y1);
        talairach(
            g,
            out,
            err,
            mri,
            f64::from(cx),
            f64::from(cy),
            f64::from(cz),
            &mut t,
            "-fillv",
        )?;
        let f1 = |v: f64| fmt_f(v, 2, 1);
        let _ = writeln!(
            err,
            "added segment {s}, nvox={nvox}, bbox [{x0}:{x1}, {y0}:{y1}, {z0}:{z1}]\n\tc = {}, {}, {} (tal = {}, {}, {})",
            f1(f64::from(cx)),
            f1(f64::from(cy)),
            f1(f64::from(cz)),
            f1(t[0]),
            f1(t[1]),
            f1(t[2])
        );
        ventricles = match ventricles {
            None => mri_segment_to_image(g, out, err, &filled, None, &mriseg, s)?.into_new(),
            Some(mut v) => {
                mri_segment_to_image(g, out, err, &filled, Some(&mut v), &mriseg, s)?;
                Some(v)
            }
        };
    }
    drop(filled);
    drop(mriseg);
    let Some(mut ventricles) = ventricles else {
        g.notices.add(
            &FS_1227,
            "mri_segment -fillv: no ventricle found: FreeSurfer crashes; none added",
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(SEGFAULT));
        }
        return Ok(());
    };
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                talairach(
                    g,
                    out,
                    err,
                    mri,
                    f64::from(x),
                    f64::from(y),
                    f64::from(z),
                    &mut t,
                    "-fillv",
                )?;
                if t[0].abs() < 5.0 {
                    mri_set_vox_val(g, err, &mut ventricles, x, y, z, 0, 0.0, 0)?;
                }
            }
        }
    }
    if diag {
        mri_write(
            g,
            out,
            err,
            ctx.env,
            ctx.dir,
            &mut ventricles,
            "ventricles.mgh",
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
    }
    mri_union(g, out, err, Tri::Second(&ventricles, mri))?;
    Ok(())
}
