// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_fill`'s fills (queue item 3.2-36, `mri_fill/mri_fill.cpp` 3363-3498, 5160-5501,
//! 5858-6183, 6463-6697): `fill_brain`, `fill_holes`, `MRIfillVolume`, `mriFindBoundingBox`,
//! `MRIcombineHemispheres`, `MRIfillDegenerateLocations` (`fillDiagonals`, `is_diagonal`,
//! `count_voxel_diagonals`, `count_diagonals`) and `fill_with_aseg`, recon-all's path.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-fill.md`):
//! FS-1291 and FS-1300 (voxels outside a volume), FS-1296 (fills that never end: an error in both modes, rule
//! B), FS-1297 (the diagonals' third derivative is always 0, kept: rule D, to be measured).

#![allow(non_upper_case_globals)]

use std::io::Write;

use larmorx_core::defects::{FS_1296, FS_1297, FS_1300};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_filter::cc::{get_largest_cc18, remove_holes};
use larmorx_freesurfer_filter::norm::mri_build_voronoi_diagram;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc, mri_copy_header};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volume::arith::{Pair, mri_clone};
use larmorx_freesurfer_volume::cma::*;
use larmorx_freesurfer_volume::labels::mri_labels_in_nbhd;
use larmorx_freesurfer_volume::mriset::mri_set_label_values;

use super::local::{
    Ax, SEGFAULT, Vals, WM_EDITED_OFF_VAL, WM_EDITED_ON_VAL, WM_MIN_VAL, bytes, bytes_mut, ix,
    set_vox, vox,
};
use super::main::{NAME, Run};

/// `MAX_ITERATIONS` of `fill_brain`.
const MAX_ITERATIONS: i32 = 1000;

/// The settings of the fills (`mri_fill.cpp`'s file-scope variables) and the bounding box
/// `mriFindBoundingBox` leaves in `xlim0`, `xlim1`, `ylim0`, `ylim1`.
#[derive(Debug, Clone)]
pub struct FillState {
    pub neighbor_threshold: i32,
    pub min_filled: i32,
    pub fill_holes_flag: bool,
    pub lhonly: bool,
    pub rhonly: bool,
    pub lh_fill_val: i32,
    pub rh_fill_val: i32,
    pub xlim0: i32,
    pub xlim1: i32,
    pub ylim0: i32,
    pub ylim1: i32,
}

fn verbose(g: &Globals) -> bool {
    g.gdiag & DIAG_VERBOSE != 0
}

/// FS-1296: a loop of FreeSurfer's that would never end on this input: an error in both modes.
fn hang(g: &mut Globals, r: &mut Run, what: &str) -> Result<(), Fatal> {
    g.notices.add(
        &FS_1296,
        format!("mri_fill: {what}: FreeSurfer's loop never ends on this input"),
    );
    log::error_exit(
        g,
        r.err,
        ERROR_BADPARM,
        &format!("{NAME}: {what} would never end (FreeSurfer runs forever here)"),
    )
}

/// A volume of one slice (`fill_brain`), or of one slice, row or column (`fill_holes`, `all`):
/// FreeSurfer's sweeps start at 1 and stop at the extent less one, which an extent of 1 never
/// meets: they run off the volume and crash (FS-1300).
fn check_extent(g: &mut Globals, r: &mut Run, m: &Mri, all: bool) -> Result<(), Fatal> {
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    if d >= 2 && (!all || (w >= 2 && h >= 2)) {
        return Ok(());
    }
    g.notices.add(
        &FS_1300,
        format!(
            "mri_fill fills a volume of {w} x {h} x {d} voxels: FreeSurfer's sweeps run past its end and crash"
        ),
    );
    if g.notices.reproduce() {
        return Err(Fatal::Exit(SEGFAULT));
    }
    log::error_exit(
        g,
        r.err,
        ERROR_BADPARM,
        &format!(
            "{NAME}: the volume to fill has {w} x {h} x {d} voxels; 2 or more are needed along each axis"
        ),
    )
}

/// `mriFindBoundingBox(mri_im)`: the rows' and columns' extent of the voxels at or above
/// `WM_MIN_VAL` (into `s`: `ylim0 = height`, `xlim0 = width`, `ylim1 = xlim1 = 0` when none).
pub fn find_bounding_box(g: &mut Globals, s: &mut FillState, m: &Mri) {
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    s.ylim0 = h;
    s.xlim0 = w;
    s.ylim1 = 0;
    s.xlim1 = 0;
    if let Some((b, row, slice)) = bytes(m) {
        for z in 0..d as usize {
            for y in 0..h as usize {
                let base = z * slice + y * row;
                for x in 0..w as usize {
                    if i32::from(b[base + x]) >= WM_MIN_VAL {
                        let (xi, yi) = (x as i32, y as i32);
                        s.ylim0 = s.ylim0.min(yi);
                        s.ylim1 = s.ylim1.max(yi);
                        s.xlim0 = s.xlim0.min(xi);
                        s.xlim1 = s.xlim1.max(xi);
                    }
                }
            }
        }
        return;
    }
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if i32::from(vox(g, m, x, y, z)) >= WM_MIN_VAL {
                    s.ylim0 = s.ylim0.min(y);
                    s.ylim1 = s.ylim1.max(y);
                    s.xlim0 = s.xlim0.min(x);
                    s.xlim1 = s.xlim1.max(x);
                }
            }
        }
    }
}

/// `fill_brain(mri_fill, mri_im, threshold)`: sweeps back and forth over the volume (slices, rows,
/// columns; forwards from 1, backwards from the end), each voxel of `fill` still 0 whose `im` value
/// is above `threshold` (below `-threshold` for a negative one) taking the largest of its three
/// neighbours already swept when one is on, until a sweep fills `min_filled` voxels or fewer (at
/// most 1000 sweeps). `total of <n> voxels filled...` on standard error.
pub fn fill_brain(
    g: &mut Globals,
    r: &mut Run,
    s: &mut FillState,
    fill: &mut Mri,
    im: &Mri,
    threshold: i32,
) -> Result<(), Fatal> {
    find_bounding_box(g, s, im);
    check_extent(g, r, fill, false)?;
    let (w, h, d) = (fill.vg.width, fill.vg.height, fill.vg.depth);
    let im_b: Vec<u8> = match bytes(im) {
        Some((b, _, _)) if im.vg.width == w && im.vg.height == h && im.vg.depth == d => b.to_vec(),
        _ => {
            let mut v = Vec::with_capacity((w * h * d).max(0) as usize);
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        v.push(vox(g, im, x, y, z));
                    }
                }
            }
            v
        }
    };
    let Some((fb, row, slice)) = bytes_mut(fill) else {
        return Ok(());
    };
    let (wu, hu) = (w as usize, h as usize);
    let at_im = |x: i32, y: i32, z: i32| (z as usize * hu + y as usize) * wu + x as usize;
    let at = |x: i32, y: i32, z: i32| z as usize * slice + y as usize * row + x as usize;
    let mut dir = -1i32;
    let mut nfilled = 10000;
    let mut ntotal = 0i32;
    let mut iter = 0;
    while nfilled > s.min_filled && iter < MAX_ITERATIONS {
        iter += 1;
        nfilled = 0;
        dir = -dir;
        let (im0, im1, i0, i1, j0, j1) = if dir == 1 {
            (1, d - 1, 1, h, 1, w)
        } else {
            (d - 2, -1, h - 2, -1, w - 2, -1)
        };
        let mut imnr = im0;
        while imnr != im1 {
            let mut i = i0;
            while i != i1 {
                let mut j = j0;
                while j != j1 {
                    let p = at(j, i, imnr);
                    if fb[p] == 0 {
                        let v = i32::from(im_b[at_im(j, i, imnr)]);
                        if (threshold < 0 && v < -threshold) || (threshold >= 0 && v > threshold) {
                            let v1 = i32::from(fb[at(j, i, imnr - dir)]);
                            let v2 = i32::from(fb[at(j, i - dir, imnr)]);
                            let v3 = i32::from(fb[at(j - dir, i, imnr)]);
                            if v1 > 0 || v2 > 0 || v3 > 0 {
                                let vmax = if v1 >= v2 && v1 >= v3 {
                                    v1
                                } else if v2 >= v1 && v2 >= v3 {
                                    v2
                                } else {
                                    v3
                                };
                                fb[p] = vmax as u8;
                                nfilled += 1;
                                ntotal += 1;
                            }
                        }
                    }
                    j += dir;
                }
                i += dir;
            }
            imnr += dir;
        }
        if g.gdiag & DIAG_SHOW != 0 && verbose(g) {
            let _ = writeln!(r.err, "{nfilled} voxels filled");
        }
    }
    let _ = write!(r.err, "total of {ntotal} voxels filled...");
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(r.err);
    }
    Ok(())
}

/// `fill_holes(mri_fill)`: sweeps over the inner voxels within 10 of the bounding box
/// ([`find_bounding_box`]): a voxel of 0 whose 3 x 3 x 3 window (itself included) holds at most
/// `neighbor_threshold` zeros takes the window's largest value; until a sweep fills none.
pub fn fill_holes(
    g: &mut Globals,
    r: &mut Run,
    s: &FillState,
    fill: &mut Mri,
) -> Result<(), Fatal> {
    check_extent(g, r, fill, true)?;
    let (w, h, d) = (fill.vg.width, fill.vg.height, fill.vg.depth);
    let cntmax = s.neighbor_threshold;
    let mut ntotal = 0i32;
    let (yl0, yl1, xl0, xl1) = (
        s.ylim0.wrapping_sub(10),
        s.ylim1.wrapping_add(10),
        s.xlim0.wrapping_sub(10),
        s.xlim1.wrapping_add(10),
    );
    loop {
        let mut nfilled = 0;
        let mut zero_fill = false;
        {
            let Some((fb, row, slice)) = bytes_mut(fill) else {
                return Ok(());
            };
            let at = |x: i32, y: i32, z: i32| z as usize * slice + y as usize * row + x as usize;
            let mut z = 1;
            while z != d - 1 {
                let mut i = 1;
                while i != h - 1 {
                    let mut x = 1;
                    while x != w - 1 {
                        if fb[at(x, i, z)] == 0 && i > yl0 && i < yl1 && x > xl0 && x < xl1 {
                            let mut cnt = 0;
                            let mut vmax = 0u8;
                            'win: for im0 in -1..=1 {
                                for i0 in -1..=1 {
                                    for x0 in -1..=1 {
                                        let v = fb[at(x + x0, i + i0, z + im0)];
                                        if v > vmax {
                                            vmax = v;
                                        }
                                        if v == 0 {
                                            cnt += 1;
                                        }
                                        if cnt > cntmax {
                                            break 'win;
                                        }
                                    }
                                }
                            }
                            if cnt <= cntmax {
                                if vmax == 0 {
                                    zero_fill = true;
                                }
                                fb[at(x, i, z)] = vmax;
                                nfilled += 1;
                                ntotal += 1;
                            }
                        }
                        x += 1;
                    }
                    i += 1;
                }
                z += 1;
            }
        }
        if zero_fill {
            // a hole "filled" with 0 is counted again on every sweep
            hang(
                g,
                r,
                &format!("the hole filling with a neighbour threshold of {cntmax} (27 or more)"),
            )?;
        }
        if g.gdiag & DIAG_SHOW != 0 && verbose(g) {
            let _ = writeln!(r.err, "{nfilled} holes filled");
        }
        if nfilled == 0 {
            break;
        }
    }
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(r.err, "total of {ntotal} holes filled");
    }
    Ok(())
}

/// The voxels of `src`'s frame 0 copied into `dst`'s (`MRIcopy(src, dst)` of two byte volumes of
/// the same extent and header).
fn copy_voxels(g: &mut Globals, r: &mut Run, src: &Mri, dst: &mut Mri) -> Result<(), Fatal> {
    match (src.chunk.as_ref(), dst.chunk.as_mut()) {
        (Some(a), Some(b)) if a.len() == b.len() => {
            b.copy_from_slice(a);
            Ok(())
        }
        _ => {
            mri_copy(g, r.out, r.err, src, Some(dst))?;
            Ok(())
        }
    }
}

fn clear(m: &mut Mri) {
    if let Some(c) = m.chunk.as_mut() {
        c.fill(0);
    }
}

/// `MRIfillVolume(mri_fill, mri_im, x, y, z, fill_val)`: the connected component of `im` at or
/// above `WM_MIN_VAL` holding the seed filled outwards, its background's connected component from
/// voxel (1, 1, 1) filled, holes filled, the complement filled from the seed again, holes filled;
/// the first and last slices cleared. `im` is used as working space.
#[allow(clippy::too_many_arguments)]
pub fn mri_fill_volume(
    g: &mut Globals,
    r: &mut Run,
    s: &mut FillState,
    fill: &mut Mri,
    im: &mut Mri,
    x_seed: i32,
    y_seed: i32,
    z_seed: i32,
    fill_val: i32,
) -> Result<(), Fatal> {
    set_vox(g, fill, x_seed, y_seed, z_seed, fill_val as u8);
    if g.gdiag == 0 {
        let _ = write!(r.err, "filling volume: pass 1 of 3...");
    }
    fill_brain(g, r, s, fill, im, WM_MIN_VAL)?;
    copy_voxels(g, r, fill, im)?;
    clear(fill);
    set_vox(g, fill, 1, 1, 1, 255);
    if g.gdiag == 0 {
        let _ = write!(r.err, "\rfilling volume: pass 2 of 3...");
    }
    fill_brain(g, r, s, fill, im, -WM_MIN_VAL)?;
    if s.fill_holes_flag {
        fill_holes(g, r, s, fill)?;
    }
    copy_voxels(g, r, fill, im)?;
    clear(fill);
    set_vox(g, fill, x_seed, y_seed, z_seed, fill_val as u8);
    if g.gdiag == 0 {
        let _ = write!(r.err, "\rfilling volume: pass 3 of 3...");
    }
    fill_brain(g, r, s, fill, im, -WM_MIN_VAL)?;
    find_bounding_box(g, s, fill);
    if s.fill_holes_flag {
        fill_holes(g, r, s, fill)?;
    }
    if g.gdiag == 0 {
        let _ = writeln!(r.err, "done.");
    }
    let (w, h, d) = (fill.vg.width, fill.vg.height, fill.vg.depth);
    for z in [0, d - 1] {
        if z < 0 {
            continue;
        }
        for y in 0..h {
            for x in 0..w {
                set_vox(g, fill, x, y, z, 0);
            }
        }
    }
    Ok(())
}

/// `MRIcombineHemispheres(mri_lh, mri_rh, NULL, lh seed, rh seed)`: the larger of the two fills
/// at each voxel; where both are on somewhere (the first such voxel, slices outermost, gives the
/// two labels and their mean, the "ambiguous" value), every voxel on in either is ambiguous, the
/// 7 x 7 x 7 windows of the seeds take their hemisphere's label, and passes give each ambiguous
/// voxel the label of most of its 26 neighbours (from 26 down; ties alternate) until a pass fills
/// none and the needed count reaches 0. The voxels given to one side are cleared in the other's
/// fill.
#[allow(clippy::too_many_arguments)]
pub fn mri_combine_hemispheres(
    g: &mut Globals,
    r: &mut Run,
    lh_m: &mut Mri,
    rh_m: &mut Mri,
    wm_lh: (i32, i32, i32),
    wm_rh: (i32, i32, i32),
) -> Result<Mri, Fatal> {
    let (w, h, d) = (lh_m.vg.width, lh_m.vg.height, lh_m.vg.depth);
    let mut dst = mri_clone(g, r.out, r.err, lh_m, None)?.unwrap_new();
    let (mut ambiguous, mut lh, mut rh) = (0i32, 0i32, 0i32);
    'first: for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let vlh = i32::from(vox(g, lh_m, x, y, z));
                let vrh = i32::from(vox(g, rh_m, x, y, z));
                if vlh >= WM_MIN_VAL && vrh >= WM_MIN_VAL {
                    ambiguous = (vlh + vrh) / 2;
                    lh = vlh;
                    rh = vrh;
                    break 'first;
                }
                set_vox(g, &mut dst, x, y, z, vlh.max(vrh) as u8);
            }
        }
    }
    if ambiguous != 0 {
        let mut min_on = 3 * 3 * 3 - 1;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let vlh = i32::from(vox(g, lh_m, x, y, z));
                    let vrh = i32::from(vox(g, rh_m, x, y, z));
                    let v = if vlh >= WM_MIN_VAL || vrh >= WM_MIN_VAL {
                        ambiguous
                    } else {
                        0
                    };
                    set_vox(g, &mut dst, x, y, z, v as u8);
                }
            }
        }
        let _ = writeln!(
            r.err,
            "using variable coefficient diffusion to correct hemispheric overlap..."
        );
        for ((sx, sy, sz), label) in [(wm_lh, lh), (wm_rh, rh)] {
            for zk in -3..=3 {
                let zi = ix(g, &dst, Ax::Z, sz.wrapping_add(zk));
                for yk in -3..=3 {
                    let yi = ix(g, &dst, Ax::Y, sy.wrapping_add(yk));
                    for xk in -3..=3 {
                        let xi = ix(g, &dst, Ax::X, sx.wrapping_add(xk));
                        if i32::from(vox(g, &dst, xi, yi, zi)) >= WM_MIN_VAL {
                            set_vox(g, &mut dst, xi, yi, zi, label as u8);
                        }
                    }
                }
            }
        }
        if g.gdiag & larmorx_freesurfer_core::diag::DIAG_WRITE != 0 && verbose(g) {
            let _ = writeln!(r.err, "writing ambiguous voxel image to amb.mgz");
            super::planes::write_diag(g, r, &mut dst, "amb.mgz")?;
        }
        let mut tmp = mri_copy(g, r.out, r.err, &dst, None)?.unwrap_new();
        // the index tables of a voxel's neighbours, per axis
        let tab = |g: &mut Globals, m: &Mri, a: Ax, n: i32| -> Vec<[i32; 3]> {
            (0..n)
                .map(|i| [ix(g, m, a, i - 1), ix(g, m, a, i), ix(g, m, a, i + 1)])
                .collect()
        };
        let (tx, ty, tz) = (
            tab(g, &dst, Ax::X, w),
            tab(g, &dst, Ax::Y, h),
            tab(g, &dst, Ax::Z, d),
        );
        let (lhb, rhb) = (lh as u8, rh as u8);
        loop {
            let mut max_on = 0;
            let mut i = 0i32;
            let mut nambiguous = 0;
            let mut nfilled = 0;
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        if i32::from(vox(g, &dst, x, y, z)) != ambiguous {
                            continue;
                        }
                        let (mut nleft, mut nright) = (0i32, 0i32);
                        for &zi in &tz[z as usize] {
                            for &yi in &ty[y as usize] {
                                for &xi in &tx[x as usize] {
                                    let v = vox(g, &dst, xi, yi, zi);
                                    if i32::from(v) == lh {
                                        nleft += 1;
                                    } else if i32::from(v) == rh {
                                        nright += 1;
                                    }
                                }
                            }
                        }
                        if nright.max(nleft) > max_on {
                            max_on = min_on.min(nright.max(nleft));
                        }
                        if nright >= min_on && nleft >= min_on && nright == nleft {
                            if i % 2 != 0 {
                                nleft -= 1;
                            } else {
                                nright -= 1;
                            }
                            i += 1;
                        }
                        if nright > nleft && nright >= min_on {
                            nfilled += 1;
                            set_vox(g, &mut tmp, x, y, z, rhb);
                            set_vox(g, lh_m, x, y, z, 0);
                        } else if nleft > nright && nleft >= min_on {
                            nfilled += 1;
                            set_vox(g, &mut tmp, x, y, z, lhb);
                            set_vox(g, rh_m, x, y, z, 0);
                        } else {
                            nambiguous += 1;
                        }
                    }
                }
            }
            let unchanged = tmp.chunk == dst.chunk;
            copy_voxels(g, r, &tmp, &mut dst)?;
            if min_on != max_on || verbose(g) {
                let _ = writeln!(r.err, "{min_on}:  {nambiguous} ambiguous voxels remaining");
            }
            let same_min = min_on == max_on;
            min_on = max_on;
            if !(nfilled > 0 || min_on > 0) {
                break;
            }
            if unchanged && same_min {
                hang(
                    g,
                    r,
                    "the hemispheres' overlap correction (its labels do not change)",
                )?;
            }
        }
    }
    Ok(dst)
}

/// `count_diagonals(mri, x0, y0, z0)`'s and `is_diagonal`'s derivatives of the 2 x 2 x 2 block at
/// `(x0, y0, z0)`: for each of its voxels below `WM_MIN_VAL` (z outermost), the x, then the y,
/// then the "z" difference across the block (FreeSurfer's third subtracts a voxel from itself:
/// always 0, FS-1297); a sign change within one direction is a diagonal.
fn block_derivs(
    g: &mut Globals,
    m: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
    dir: usize,
    x: i32,
    y: i32,
    z: i32,
) -> i32 {
    match dir {
        0 => i32::from(vox(g, m, x0 + 1, y, z)) - i32::from(vox(g, m, x0, y, z)),
        1 => i32::from(vox(g, m, x, y0 + 1, z)) - i32::from(vox(g, m, x, y0, z)),
        _ => i32::from(vox(g, m, x, y, z0 + 1)) - i32::from(vox(g, m, x, y, z0 + 1)),
    }
}

/// The count of voxels not 0 in the block at `(x0, y0, z0)`.
fn block_on(g: &mut Globals, m: &Mri, x0: i32, y0: i32, z0: i32) -> i32 {
    let mut non = 0;
    for z in z0..=z0 + 1 {
        for y in y0..=y0 + 1 {
            for x in x0..=x0 + 1 {
                if vox(g, m, x, y, z) != 0 {
                    non += 1;
                }
            }
        }
    }
    non
}

/// `is_diagonal(mri, x0, y0, z0, block)`: whether the 2 x 2 x 2 block at `(x0, y0, z0)` (2 to 6
/// voxels on) has opposite derivatives in the x or the y direction among its voxels below
/// `WM_MIN_VAL`; `block` holds the derivatives of the direction that decided it (the y's when
/// none did: the third direction's are always 0).
fn is_diagonal(
    g: &mut Globals,
    m: &Mri,
    x0: i32,
    y0: i32,
    z0: i32,
    block: &mut [[[i32; 2]; 2]; 2],
) -> bool {
    let non = block_on(g, m, x0, y0, z0);
    if !(2..=6).contains(&non) {
        return false;
    }
    let mut diagonal = false;
    for dir in 0..3 {
        let mut dsign = 0i32;
        for z in z0..=z0 + 1 {
            for y in y0..=y0 + 1 {
                for x in x0..=x0 + 1 {
                    let (bx, by, bz) = ((x - x0) as usize, (y - y0) as usize, (z - z0) as usize);
                    if i32::from(vox(g, m, x, y, z)) < WM_MIN_VAL {
                        let d = block_derivs(g, m, x0, y0, z0, dir, x, y, z);
                        block[bx][by][bz] = d;
                        if d.wrapping_mul(dsign) < 0 {
                            diagonal = true;
                        } else {
                            dsign = d;
                        }
                    } else {
                        block[bx][by][bz] = 0;
                    }
                }
            }
        }
        if diagonal {
            return true;
        }
    }
    diagonal
}

/// `count_diagonals(mri, x0, y0, z0)`: the sign changes of the block's x, y and "z" derivatives
/// (see [`block_derivs`]); 0 unless 2 to 6 of its voxels are on.
fn count_diagonals(g: &mut Globals, m: &Mri, x0: i32, y0: i32, z0: i32) -> i32 {
    let non = block_on(g, m, x0, y0, z0);
    if !(2..=6).contains(&non) {
        return 0;
    }
    let mut diagonals = 0;
    for dir in 0..3 {
        let mut dsign = 0i32;
        for z in z0..=z0 + 1 {
            for y in y0..=y0 + 1 {
                for x in x0..=x0 + 1 {
                    if i32::from(vox(g, m, x, y, z)) < WM_MIN_VAL {
                        let d = block_derivs(g, m, x0, y0, z0, dir, x, y, z);
                        if d.wrapping_mul(dsign) < 0 {
                            diagonals += 1;
                        } else {
                            dsign = d;
                        }
                    }
                }
            }
        }
    }
    diagonals
}

/// `count_voxel_diagonals(mri, x, y, z)`: [`count_diagonals`] of the eight blocks holding the voxel.
fn count_voxel_diagonals(g: &mut Globals, m: &Mri, x0: i32, y0: i32, z0: i32) -> i32 {
    let mut diagonals = 0;
    for zk in -1..=0 {
        for yk in -1..=0 {
            for xk in -1..=0 {
                diagonals += count_diagonals(g, m, x0 + xk, y0 + yk, z0 + zk);
            }
        }
    }
    diagonals
}

/// `fillDiagonals(mri, fillval)`: each inner block (slices outermost) that is diagonal gets, while
/// it stays so, one voxel set to `fillval`: of its candidates (derivative not 0), the last with the
/// fewest diagonals once set (FreeSurfer sets each candidate to try it and resets it to 0 after);
/// the count of voxels set.
fn fill_diagonals(g: &mut Globals, r: &mut Run, m: &mut Mri, fillval: i32) -> Result<i32, Fatal> {
    let mut nfilled = 0;
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    let fb = fillval as u8;
    let mut block = [[[0i32; 2]; 2]; 2];
    for z in 1..d - 1 {
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                while is_diagonal(g, m, x, y, z, &mut block) {
                    let before: Vec<u8> = (0..8)
                        .map(|k| vox(g, m, x + (k & 1), y + ((k >> 1) & 1), z + (k >> 2)))
                        .collect();
                    let mut min_diagonals = 10000;
                    let (mut mxk, mut myk, mut mzk) = (0, 0, 0);
                    for zk in 0..=1 {
                        let z1 = z + zk;
                        for yk in 0..=1 {
                            let y1 = y + yk;
                            for xk in 0..=1 {
                                if block[xk as usize][yk as usize][zk as usize] != 0 {
                                    let x1 = x + xk;
                                    let filled = if i32::from(vox(g, m, x1, y1, z1)) != fillval {
                                        set_vox(g, m, x1, y1, z1, fb);
                                        true
                                    } else {
                                        false
                                    };
                                    let diagonals = count_voxel_diagonals(g, m, x1, y1, z1);
                                    if diagonals <= min_diagonals {
                                        min_diagonals = diagonals;
                                        mxk = xk;
                                        myk = yk;
                                        mzk = zk;
                                    }
                                    if filled {
                                        set_vox(g, m, x1, y1, z1, 0);
                                    }
                                }
                            }
                        }
                    }
                    set_vox(g, m, x + mxk, y + myk, z + mzk, fb);
                    nfilled += 1;
                    let after: Vec<u8> = (0..8)
                        .map(|k| vox(g, m, x + (k & 1), y + ((k >> 1) & 1), z + (k >> 2)))
                        .collect();
                    if before == after {
                        hang(
                            g,
                            r,
                            &format!(
                                "the degenerate locations' filling with the value {fillval} (stored as {fb}: it does not change the block)"
                            ),
                        )?;
                    }
                }
            }
        }
    }
    Ok(nfilled)
}

/// `MRIfillDegenerateLocations(mri, fillval)`: [`fill_diagonals`] until it sets nothing (`<n> voxels
/// filled` after each pass, on standard error). The third derivative of FreeSurfer's diagonal test
/// is always 0 (FS-1297, kept).
pub fn mri_fill_degenerate_locations(
    g: &mut Globals,
    r: &mut Run,
    m: &mut Mri,
    fillval: i32,
) -> Result<(), Fatal> {
    g.notices.add(
        &FS_1297,
        "mri_fill: the diagonal configurations are looked for along x and y only (FreeSurfer's z derivative subtracts a voxel from itself): kept",
    );
    loop {
        let n = fill_diagonals(g, r, m, fillval)?;
        let _ = writeln!(r.err, "{n} voxels filled");
        if n <= 0 {
            break;
        }
    }
    Ok(())
}

/// The left hemisphere's labels of `fill_with_aseg`'s switch.
fn is_left(label: i32) -> bool {
    matches!(
        label,
        Left_Cerebral_White_Matter
            | Left_Cerebral_Cortex
            | Left_Lateral_Ventricle
            | Left_Inf_Lat_Vent
            | Left_Thalamus
            | Left_Caudate
            | Left_Putamen
            | Left_Pallidum
            | Left_Hippocampus
            | Left_Amygdala
            | Left_Insula
            | Left_Operculum
            | Left_Lesion
            | Left_vessel
            | Left_Accumbens_area
            | Left_VentralDC
            | Left_choroid_plexus
            | Left_Substancia_Nigra
            | Left_F3orb
            | Left_lOg
            | Left_aOg
            | Left_mOg
            | Left_pOg
            | Left_Stellate
            | Left_Porg
            | Left_Aorg
    )
}

/// The right hemisphere's labels of `fill_with_aseg`'s switch.
fn is_right(label: i32) -> bool {
    matches!(
        label,
        Right_Cerebral_White_Matter
            | Right_Cerebral_Cortex
            | Right_Lateral_Ventricle
            | Right_Inf_Lat_Vent
            | Right_Thalamus
            | Right_Caudate
            | Right_Putamen
            | Right_Pallidum
            | Right_Hippocampus
            | Right_Amygdala
            | Right_Insula
            | Right_Operculum
            | Right_Lesion
            | Right_vessel
            | Right_Accumbens_area
            | Right_VentralDC
            | Right_choroid_plexus
            | Right_Substancia_Nigra
            | Right_F3orb
            | Right_lOg
            | Right_aOg
            | Right_mOg
            | Right_pOg
            | Right_Stellate
            | Right_Porg
            | Right_Aorg
    )
}

/// The cerebral white matter and cortex of one side within the window (`MRIlabelsInNbhd` twice).
fn side_count(
    g: &mut Globals,
    r: &mut Run,
    seg: &Mri,
    x: i32,
    y: i32,
    z: i32,
    whalf: i32,
    labels: [i32; 2],
) -> Result<i32, Fatal> {
    let a = mri_labels_in_nbhd(g, r.err, seg, x, y, z, whalf, labels[0])?;
    let b = mri_labels_in_nbhd(g, r.err, seg, x, y, z, whalf, labels[1])?;
    Ok(a.wrapping_add(b))
}

/// `fill_with_aseg(mri_img, mri_seg)`: the brainstem and cerebellum erased from the white matter;
/// each voxel's side from the segmentation's labels, spread to the unlabelled voxels by a Voronoi
/// diagram (not with `-lhonly` or `-rhonly`); the white matter's voxels (edited off: none; edited
/// on: the side with more cerebral white matter and cortex nearby; below `WM_MIN_VAL`: none unless
/// a lesion or a white-matter hypointensity) split by that side; each side's largest 18-connected
/// component with its holes filled; the left's fill value where the left's is on, else the right's.
pub fn fill_with_aseg(
    g: &mut Globals,
    r: &mut Run,
    s: &FillState,
    img: &mut Mri,
    seg: &Mri,
) -> Result<Mri, Fatal> {
    let (w, h, d) = (img.vg.width, img.vg.height, img.vg.depth);
    let alloc = |g: &mut Globals, r: &mut Run, img: &Mri| -> Result<Mri, Fatal> {
        let mut m = mri_alloc(g, r.out, r.err, w, h, d, MRI_UCHAR)?;
        mri_copy_header(g, r.out, r.err, img, Some(&mut m))?;
        Ok(m)
    };
    let mut ctrl = alloc(g, r, img)?;
    let mut fill = alloc(g, r, img)?;
    let mut fill_lh = alloc(g, r, img)?;
    let mut fill_rh = alloc(g, r, img)?;
    let _ = writeln!(r.out, "Erasing Brain Stem and Cerebellum ...");
    for label in [
        Brain_Stem,
        Left_Cerebellum_White_Matter,
        Left_Cerebellum_Cortex,
        Right_Cerebellum_White_Matter,
        Right_Cerebellum_Cortex,
    ] {
        mri_set_label_values(g, r.out, r.err, Pair::Same(img), seg, label, 0.0)?;
    }
    let _ = writeln!(r.out, "Define left and right masks using aseg:");
    let sv = Vals::new(g, r.err, seg)?;
    let (lhv, rhv) = (s.lh_fill_val as u8, s.rh_fill_val as u8);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                set_vox(g, &mut ctrl, x, y, z, 0);
                set_vox(g, &mut fill, x, y, z, 0);
                set_vox(g, &mut fill_lh, x, y, z, 0);
                set_vox(g, &mut fill_rh, x, y, z, 0);
                let label = sv.label(g, r.err, x, y, z)?;
                if is_left(label) {
                    set_vox(g, &mut ctrl, x, y, z, 1);
                    set_vox(g, &mut fill, x, y, z, lhv);
                } else if is_right(label) {
                    set_vox(g, &mut ctrl, x, y, z, 1);
                    set_vox(g, &mut fill, x, y, z, rhv);
                }
            }
        }
    }
    if !s.lhonly && !s.rhonly {
        let _ = writeln!(r.out, "Building Voronoi diagram ...");
        // `MRIbuildVoronoiDiagram(mri_fill, mri_ctrl, mri_fill)`: the source is read before the
        // destination is written, so a copy of it gives the same result
        let src = mri_copy(g, r.out, r.err, &fill, None)?.unwrap_new();
        mri_build_voronoi_diagram(g, r.out, r.err, &src, &mut ctrl, Some(&mut fill))?;
        let _ = write!(r.out, "Using the Voronoi diagram for ");
    }
    let _ = writeln!(r.out, "separating WM into two hemispheres ...");
    let iv = Vals::new(g, r.err, img)?;
    let lh_labels = [Left_Cerebral_White_Matter, Left_Cerebral_Cortex];
    let rh_labels = [Right_Cerebral_White_Matter, Right_Cerebral_Cortex];
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let val = iv.label(g, r.err, x, y, z)?;
                if val == WM_EDITED_OFF_VAL {
                    mri_set_vox_val(g, r.err, &mut fill_lh, x, y, z, 0, 0.0, 0)?;
                    mri_set_vox_val(g, r.err, &mut fill_rh, x, y, z, 0, 0.0, 0)?;
                    mri_set_vox_val(g, r.err, &mut fill, x, y, z, 0, 0.0, 0)?;
                    continue;
                } else if val == WM_EDITED_ON_VAL {
                    let mut whalf = super::local::int(g, (5.0f32 / seg.vg.xsize).ceil());
                    if s.lhonly || s.rhonly {
                        whalf = whalf.min(1);
                    }
                    let mut lh = side_count(g, r, seg, x, y, z, whalf, lh_labels)?;
                    let mut rh = side_count(g, r, seg, x, y, z, whalf, rh_labels)?;
                    while lh == 0 && rh == 0 {
                        whalf = whalf.wrapping_add(1);
                        lh = side_count(g, r, seg, x, y, z, whalf, lh_labels)?;
                        rh = side_count(g, r, seg, x, y, z, whalf, rh_labels)?;
                        if whalf > 20 {
                            break;
                        }
                    }
                    if lh > rh {
                        if !s.rhonly {
                            mri_set_vox_val(
                                g,
                                r.err,
                                &mut fill,
                                x,
                                y,
                                z,
                                0,
                                s.lh_fill_val as f32,
                                0,
                            )?;
                        }
                    } else if !s.lhonly {
                        mri_set_vox_val(g, r.err, &mut fill, x, y, z, 0, s.rh_fill_val as f32, 0)?;
                    }
                } else if val < WM_MIN_VAL {
                    let label = sv.label(g, r.err, x, y, z)?;
                    if label != Left_Lesion && label != Right_Lesion && !is_wmsa(label) {
                        mri_set_vox_val(g, r.err, &mut fill_lh, x, y, z, 0, 0.0, 0)?;
                        mri_set_vox_val(g, r.err, &mut fill_rh, x, y, z, 0, 0.0, 0)?;
                        mri_set_vox_val(g, r.err, &mut fill, x, y, z, 0, 0.0, 0)?;
                        continue;
                    }
                }
                if i32::from(vox(g, &fill, x, y, z)) == s.rh_fill_val {
                    if !s.lhonly {
                        set_vox(g, &mut fill_rh, x, y, z, 1);
                    }
                    set_vox(g, &mut fill_lh, x, y, z, 0);
                } else {
                    if !s.rhonly {
                        set_vox(g, &mut fill_lh, x, y, z, 1);
                    }
                    set_vox(g, &mut fill_rh, x, y, z, 0);
                }
            }
        }
    }
    let _ = writeln!(
        r.out,
        "Find the largest connected component for each hemisphere ..."
    );
    if !s.rhonly {
        get_largest_cc18(g, r.out, r.err, &mut fill_lh)?;
        remove_holes(g, r.out, r.err, &mut fill_lh)?;
    }
    if !s.lhonly {
        get_largest_cc18(g, r.out, r.err, &mut fill_rh)?;
        remove_holes(g, r.out, r.err, &mut fill_rh)?;
    }
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                set_vox(g, &mut fill, x, y, z, 0);
                if vox(g, &fill_lh, x, y, z) > 0 {
                    if !s.rhonly {
                        set_vox(g, &mut fill, x, y, z, lhv);
                    }
                } else if vox(g, &fill_rh, x, y, z) > 0 && !s.lhonly {
                    set_vox(g, &mut fill, x, y, z, rhv);
                }
            }
        }
    }
    Ok(fill)
}
