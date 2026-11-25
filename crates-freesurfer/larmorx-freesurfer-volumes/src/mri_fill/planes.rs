// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_fill`'s cutting planes and seeds (queue item 3.2-36 with `find_cutting_plane`,
//! `mri_fill/mri_fill.cpp` 3848-5126 and 6267-6460, 6698-6727): `find_cutting_plane`,
//! `find_slice_center`, `find_corpus_callosum`, `find_pons`, `find_cc_slice`, `neighbors_on`,
//! `extend_to_lateral_borders`, `find_cc_seed_with_segmentation`, `find_rh_seed_point`,
//! `mri_erase_nonmidline_voxels` and `MRIreplaceCCwithWM`. These are `mri_fill`'s own copies:
//! `mri_cc`'s `find_corpus_callosum` and `find_cc_slice` (`crate::mri_cc::wm`) search differently.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-fill.md`):
//! FS-1291 (voxels outside a volume, [`super::local`]), FS-1293 (voxels smaller than 0.5 mm: more
//! slices than FreeSurfer's tables hold, an error in both modes), FS-1294 (the pons' backup
//! searches in the input's space, kept), FS-1295 (`-D`'s statistics file that does not open:
//! FreeSurfer crashes).

use std::io::Write;
use std::path::PathBuf;

use larmorx_core::defects::{FS_1293, FS_1294, FS_1295};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE, DIAG_WRITE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils::{nint, resolve};
use larmorx_freesurfer_filter::fastmarch::mri_distance_transform;
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::lta::Lta;
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, Mri, MriRegion};
use larmorx_freesurfer_mri::vol_geom::{voxel_to_world, world_to_voxel};
use larmorx_freesurfer_mri::voxel::{mri_bounding_box, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, mri_clone};
use larmorx_freesurfer_volume::cma::{
    Left_Cerebral_White_Matter, Right_Cerebral_White_Matter, is_cc, is_wm,
};
use larmorx_freesurfer_volume::coords::{MRI_CORONAL, MRI_SAGITTAL};
use larmorx_freesurfer_volume::deriv::neighbors_on_3x3;
use larmorx_freesurfer_volume::labels::mri_find_nearest_nonzero_location;
use larmorx_freesurfer_volume::mriset::{mri_close, mri_total_voxels_on};
use larmorx_freesurfer_volume::resize::{mri_extract_plane, mri_fill_fg, mri_fill_plane};

use super::local::{Ax, MRI_HORIZONTAL, SEGFAULT, Vals, WM_MIN_VAL, int64, ix, set_vox, vox};
use super::main::{NAME, Run};
use super::topofix::find_min_size;

/// `MAX_SLICES`, `SEARCH_STEP`, `MAX_OFFSET` (`mri_fill.cpp`).
const MAX_SLICES: i32 = 15;
const SEARCH_STEP: i32 = 3;
const MAX_OFFSET: i32 = 50;
/// `MAX_SLICES * 2`: the size of the slice tables of `find_cutting_plane` and `find_cc_slice`.
const SLICE_TABLE: i32 = MAX_SLICES * 2;
/// `MIN_CC_AREA`, `MAX_CC_AREA`, `MIN_CC_ASPECT`, `MAX_CC_ASPECT`.
const MIN_CC_AREA: i32 = 350;
const MAX_CC_AREA: i32 = 1400;
const MIN_CC_ASPECT: f64 = 0.1;
const MAX_CC_ASPECT: f64 = 0.75;
/// `MIN_PONS_AREA`, `MAX_PONS_AREA`, `MIN_PONS_ASPECT`, `MAX_PONS_ASPECT`.
const MIN_PONS_AREA: i32 = 350;
const MAX_PONS_AREA: i32 = 790;
const MIN_PONS_ASPECT: f64 = 0.6;
const MAX_PONS_ASPECT: f64 = 1.2;
/// `CC_SPREAD`, `MIN_THICKNESS`, `MAX_THICKNESS` (`find_corpus_callosum`).
const CC_SPREAD: i32 = 17;
const MIN_THICKNESS: i32 = 3;
const MAX_THICKNESS: i32 = 20;
/// The brainstem's sizes (`find_pons`).
const MIN_BRAINSTEM_THICKNESS: i32 = 15;
const MAX_BRAINSTEM_THICKNESS: i32 = 35;
const MIN_DELTA_THICKNESS: i32 = 6;
const MIN_BRAINSTEM_HEIGHT: i32 = 25;
const MIN_CONT_BRAINSTEM_THICKNESS: i32 = 8;
const MIN_CONT_BRAINSTEM_HEIGHT: i32 = 3;
/// `PONS_LOG_FILE`, `CC_LOG_FILE`: `-D`'s statistics files, in the working folder.
const PONS_LOG_FILE: &str = "pons.log";
const CC_LOG_FILE: &str = "cc.log";
/// `WHALF` of `find_cc_seed_with_segmentation`: the sagittal window's half width.
const WHALF: i32 = (11 - 1) / 2;

fn diag_verbose(g: &Globals, flag: u64) -> bool {
    g.gdiag & flag != 0 && g.gdiag & DIAG_VERBOSE != 0
}

/// `ceil(c / v)` or `floor(c / v)` as an `int` (FS-17 beyond its range).
fn to_int(g: &mut Globals, v: f64) -> i32 {
    int64(g, v)
}

/// `MRIwrite(mri, fname)` of a diagnostic volume (`Gdiag & DIAG_WRITE && DIAG_VERBOSE_ON`).
pub fn write_diag(g: &mut Globals, r: &mut Run, mri: &mut Mri, fname: &str) -> Result<(), Fatal> {
    mri_write(
        g,
        r.out,
        r.err,
        r.env,
        r.dir,
        mri,
        fname,
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    Ok(())
}

/// The slice (`where`) of the final plane for an orientation (`find_cutting_plane`'s last switch:
/// an unknown orientation is horizontal there).
fn where_of(orientation: i32, xv: i32, yv: i32, zv: i32) -> i32 {
    match orientation {
        MRI_SAGITTAL => xv,
        MRI_CORONAL => zv,
        _ => yv,
    }
}

/// `region.dy / region.dx` in `double`.
fn aspect_of(region: &MriRegion) -> f64 {
    f64::from(region.dy) / f64::from(region.dx)
}

/// A statistics file of `-D` (`fopen(name, "a")`): its path and what was written to it since; the
/// file is created at the open (`None` when it does not open).
pub struct StatLog {
    path: PathBuf,
    buf: Vec<u8>,
}

impl StatLog {
    fn open(r: &Run, name: &str) -> Option<StatLog> {
        let path = resolve(r.dir, name);
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .ok()
            .map(|_| StatLog {
                path,
                buf: Vec::new(),
            })
    }

    /// `fclose(fp)`: what was written appended to the file.
    fn close(self) {
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&self.path) {
            let _ = f.write_all(&self.buf);
        }
    }
}

/// The paint of `find_cutting_plane`: in each column of the region, every voxel from the first
/// one above 0 down to the region's end set to 1.
fn paint(g: &mut Globals, m: &mut Mri, region: &MriRegion) {
    let mut xv = region.x;
    while xv < region.x.wrapping_add(region.dx) {
        let mut found = false;
        let mut yv = region.y;
        while yv < region.y.wrapping_add(region.dy) {
            if !found {
                found = vox(g, m, xv, yv, 0) > 0;
            }
            if found {
                set_vox(g, m, xv, yv, 0, 1);
            }
            yv += 1;
        }
        xv += 1;
    }
}

/// FS-1293: `max_slices` slices do not fit FreeSurfer's tables of `MAX_SLICES * 2` (voxels
/// smaller than 0.5 mm): an error in both modes (rule B).
fn check_slices(
    g: &mut Globals,
    r: &mut Run,
    what: &str,
    max_slices: i32,
    voxsize: f64,
) -> Result<(), Fatal> {
    if max_slices <= SLICE_TABLE {
        return Ok(());
    }
    g.notices.add(
        &FS_1293,
        format!(
            "mri_fill: {what} with voxels of {voxsize} mm takes {max_slices} slices, past FreeSurfer's tables of {SLICE_TABLE} (memory it does not own)"
        ),
    );
    log::error_exit(
        g,
        r.err,
        ERROR_BADPARM,
        &format!(
            "{NAME}: {what} takes voxels of 0.5 mm or more (the smallest here {voxsize} mm: {max_slices} slices, at most {SLICE_TABLE})"
        ),
    )?;
    Ok(())
}

/// `find_cutting_plane(mri, x_tal, y_tal, z_tal, orientation, &xv, &yv, &zv, seed_set, lta)`:
/// the cutting plane of the corpus callosum (`MRI_SAGITTAL`) or the pons (`MRI_HORIZONTAL`)
/// through the seed (Talairach millimetres of `mri`'s world): a volume like `mri` whose plane
/// holds 200 where the blob of the chosen slice lies, and the plane's voxel in `pv`; `None` when
/// no slice qualifies.
#[allow(clippy::too_many_arguments)]
pub fn find_cutting_plane(
    g: &mut Globals,
    r: &mut Run,
    mri_tal: &mut Mri,
    x_tal: f64,
    y_tal: f64,
    z_tal: f64,
    orientation: i32,
    pv: &mut (i32, i32, i32),
    seed_set: bool,
    lta: Option<&Lta>,
    logging: bool,
) -> Result<Option<Mri>, Fatal> {
    let (mut x_tal, mut y_tal, mut z_tal) = (x_tal, y_tal, z_tal);
    let voxsize = find_min_size(mri_tal);
    let (mut xv, mut yv, mut zv);
    if mri_tal.linear_transform.is_some() || lta.is_some() {
        let (x, y, z) = world_to_voxel(g, r.out, r.err, &mut mri_tal.vg, x_tal, y_tal, z_tal)?;
        xv = nint(g, x);
        yv = nint(g, y);
        zv = nint(g, z);
    } else {
        let region = mri_bounding_box(g, r.err, mri_tal, 1)?.1;
        xv = region.x.wrapping_add(region.dx / 2);
        yv = region.y.wrapping_add(region.dy / 2);
        zv = region.z.wrapping_add(region.dz / 2);
    }
    if i32::from(vox(g, mri_tal, xv, yv, zv)) < WM_MIN_VAL {
        let _ = writeln!(
            r.out,
            "find_cutting_plane:seed point not in structure! Searching neighborhood..."
        );
        let mut p = (xv, yv, zv);
        mri_find_nearest_nonzero_location(
            g,
            r.err,
            mri_tal,
            9,
            f64::from(xv),
            f64::from(yv),
            f64::from(zv),
            Some(&mut p),
        )?;
        (xv, yv, zv) = p;
    }

    let (dx, dy, dz, name, min_a, max_a, min_aspect, max_aspect, mut fp);
    let (mut where_, xo, yo);
    match orientation {
        MRI_HORIZONTAL => {
            (dx, dy, dz) = (0.0, 0.0, 1.0);
            name = "pons";
            (min_a, max_a, min_aspect, max_aspect) = (
                MIN_PONS_AREA,
                MAX_PONS_AREA,
                MIN_PONS_ASPECT,
                MAX_PONS_ASPECT,
            );
            fp = logging.then(|| StatLog::open(r, PONS_LOG_FILE));
            (where_, yo, xo) = (yv, zv, xv);
        }
        MRI_CORONAL => {
            (dx, dy, dz) = (0.0, 1.0, 0.0);
            (min_a, max_a, min_aspect, max_aspect) =
                (MIN_CC_AREA, MAX_CC_AREA, MIN_CC_ASPECT, MAX_CC_ASPECT);
            name = "coronal";
            // no statistics file: FreeSurfer's `fp` stays NULL
            fp = logging.then_some(None);
            (where_, yo, xo) = (zv, yv, xv);
        }
        _ => {
            (dx, dy, dz) = (1.0, 0.0, 0.0);
            name = "corpus callosum";
            (min_a, max_a, min_aspect, max_aspect) =
                (MIN_CC_AREA, MAX_CC_AREA, MIN_CC_ASPECT, MAX_CC_ASPECT);
            fp = logging.then(|| StatLog::open(r, CC_LOG_FILE));
            (where_, yo, xo) = (xv, yv, zv);
        }
    }
    let vv = voxsize * voxsize;
    let mut min_area = to_int(g, (f64::from(min_a) / vv).floor());
    if min_area <= 0 {
        min_area = 1;
    }
    let max_area = to_int(g, (f64::from(max_a) / vv).ceil());
    let max_slices = to_int(g, (f64::from(MAX_SLICES) / voxsize).ceil());
    let half_slices = max_slices.wrapping_sub(1) / 2;
    let search_step = to_int(g, (f64::from(SEARCH_STEP) / voxsize).ceil());
    let max_offset = to_int(g, (f64::from(MAX_OFFSET) / voxsize).ceil());
    let _ = writeln!(
        r.err,
        "Looking for area (min, max) = ({min_area}, {max_area})"
    );
    let mut slice0 =
        mri_extract_plane(g, r.out, r.err, mri_tal, None, orientation, where_)?.unwrap_new();
    let (f, mut area0) = mri_fill_fg(
        g,
        r.out,
        r.err,
        Pair::New(&slice0),
        xo,
        yo,
        0,
        WM_MIN_VAL,
        127,
    )?;
    let mut filled0 = f.unwrap_new();
    let mut region = mri_bounding_box(g, r.err, &filled0, 1)?.1;
    if g.gdiag & DIAG_WRITE != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let tag = if orientation == MRI_SAGITTAL {
            "cc"
        } else {
            "pons"
        };
        write_diag(
            g,
            r,
            &mut slice0,
            &format!("cutting_plane_{tag}_slice0.mgz"),
        )?;
        write_diag(
            g,
            r,
            &mut filled0,
            &format!("cutting_plane_{tag}_filled0.mgz"),
        )?;
    }
    let slice_size = filled0.vg.width;
    let ok = |area: i32, aspect: f64, region: &MriRegion| {
        area >= min_area
            && area <= max_area
            && aspect >= min_aspect
            && aspect <= max_aspect
            && region.y > 0
            && region.x > 0
            && region.x.wrapping_add(region.dx) < slice_size.wrapping_sub(1)
            && region.y.wrapping_add(region.dy) < slice_size.wrapping_sub(1)
    };
    let aspect = aspect_of(&region);
    let mut done = ok(area0, aspect, &region);
    let _ = writeln!(
        r.err,
        "area[0] = {area0} (min = {min_area}, max = {max_area}), aspect = {} (min = {}, max = {})",
        fmt_f(aspect, 0, 2),
        fmt_f(min_aspect, 0, 2),
        fmt_f(max_aspect, 0, 2)
    );
    let _ = write!(
        r.err,
        "{}",
        if done {
            "no need to search\n"
        } else {
            "need search nearby\n"
        }
    );

    let mut cut: Option<Mri> = None;
    if seed_set {
        done = true;
        region = mri_bounding_box(g, r.err, &filled0, 1)?.1;
        if orientation == MRI_SAGITTAL {
            region.dy = filled0.vg.height.wrapping_sub(region.y);
        }
        let (x, y, z) = world_to_voxel(g, r.out, r.err, &mut mri_tal.vg, x_tal, y_tal, z_tal)?;
        pv.0 = nint(g, x);
        pv.1 = nint(g, y);
        pv.2 = nint(g, z);
        let _ = writeln!(
            r.err,
            "using seed ({}, {}, {}), TAL = ({}, {}, {})",
            pv.0,
            pv.1,
            pv.2,
            fmt_f(x_tal, 2, 1),
            fmt_f(y_tal, 2, 1),
            fmt_f(z_tal, 2, 1)
        );
        let mut c = mri_copy(g, r.out, r.err, &filled0, None)?.unwrap_new();
        paint(g, &mut c, &region);
        (xv, yv, zv) = *pv;
        if diag_verbose(g, DIAG_WRITE) {
            let tag = if orientation == MRI_SAGITTAL {
                "cc"
            } else {
                "pons"
            };
            write_diag(g, r, &mut slice0, &format!("{tag}_filled.mgz"))?;
            write_diag(g, r, &mut c, &format!("{tag}_cut.mgz"))?;
        }
        cut = Some(c);
    }
    if !seed_set && done {
        let (xi, yi) = find_slice_center(g, &filled0);
        match orientation {
            MRI_SAGITTAL => {
                zv = zv.wrapping_add(xi.wrapping_sub(xo));
                yv = yv.wrapping_add(yi.wrapping_sub(yo));
            }
            _ => {
                xv = xv.wrapping_add(xi.wrapping_sub(xo));
                zv = zv.wrapping_add(yi.wrapping_sub(yo));
            }
        }
        (x_tal, y_tal, z_tal) = voxel_to_world(
            g,
            r.out,
            r.err,
            &mut mri_tal.vg,
            f64::from(xv),
            f64::from(yv),
            f64::from(zv),
        )?;
    }
    drop(slice0);
    drop(filled0);

    let mut offset = 0i32;
    if !seed_set {
        while !done {
            offset = offset.wrapping_add(search_step);
            if offset >= max_offset {
                return Ok(None);
            }
            let _ = writeln!(
                r.err,
                "using +/- offset search region where offset is {offset}....."
            );
            let (mut xoo, mut yoo) = (0i32, 0i32);
            let mut z0 = zv.wrapping_sub(offset);
            while !done && z0 <= zv.wrapping_add(offset) {
                let mut zi = ix(g, mri_tal, Ax::Z, z0);
                let mut y0 = yv.wrapping_sub(offset);
                while !done && y0 <= yv.wrapping_add(offset) {
                    let mut yi = ix(g, mri_tal, Ax::Y, y0);
                    let mut x0 = xv.wrapping_sub(offset);
                    while !done && x0 <= xv.wrapping_add(offset) {
                        let mut xi = ix(g, mri_tal, Ax::X, x0);
                        match orientation {
                            MRI_SAGITTAL => {
                                where_ = xi;
                                xoo = zi;
                                yoo = yi;
                            }
                            MRI_CORONAL => where_ = zi,
                            _ => {
                                where_ = yi;
                                xoo = xi;
                                yoo = zi;
                            }
                        }
                        let mut s0 =
                            mri_extract_plane(g, r.out, r.err, mri_tal, None, orientation, where_)?
                                .unwrap_new();
                        let (f, a) = mri_fill_fg(
                            g,
                            r.out,
                            r.err,
                            Pair::New(&s0),
                            xoo,
                            yoo,
                            0,
                            WM_MIN_VAL,
                            127,
                        )?;
                        area0 = a;
                        let mut f0 = f.unwrap_new();
                        let region = mri_bounding_box(g, r.err, &f0, 1)?.1;
                        if diag_verbose(g, DIAG_WRITE) {
                            let tag = if orientation == MRI_SAGITTAL {
                                "cc"
                            } else {
                                "pons"
                            };
                            write_diag(g, r, &mut s0, &format!("{tag}_seed.mgz"))?;
                            write_diag(g, r, &mut f0, &format!("{tag}_seed_fill.mgz"))?;
                        }
                        let aspect = aspect_of(&region);
                        if ok(area0, aspect, &region) {
                            let _ = writeln!(
                                r.err,
                                "area[0] = {area0} (min = {min_area}, max = {max_area}), aspect = {} (min = {}, max = {})",
                                fmt_f(aspect, 0, 2),
                                fmt_f(min_aspect, 0, 2),
                                fmt_f(max_aspect, 0, 2)
                            );
                            let (cx, cy) = find_slice_center(g, &f0);
                            xv = cx;
                            yv = cy;
                            match orientation {
                                MRI_SAGITTAL => {
                                    zi = xv;
                                    yi = yv;
                                }
                                _ => {
                                    xi = xv;
                                    zi = yv;
                                }
                            }
                            (x_tal, y_tal, z_tal) = voxel_to_world(
                                g,
                                r.out,
                                r.err,
                                &mut mri_tal.vg,
                                f64::from(xi),
                                f64::from(yi),
                                f64::from(zi),
                            )?;
                            done = true;
                            xv = xi;
                            yv = yi;
                            zv = zi;
                        }
                        x0 = x0.wrapping_add(offset);
                    }
                    y0 = y0.wrapping_add(offset);
                }
                z0 = z0.wrapping_add(offset);
            }
        }
    }
    if g.gdiag & DIAG_SHOW != 0 {
        let (x, y, z) = voxel_to_world(
            g,
            r.out,
            r.err,
            &mut mri_tal.vg,
            f64::from(xv),
            f64::from(yv),
            f64::from(zv),
        )?;
        let _ = writeln!(
            r.err,
            "{name} seed point found at Tal vol: ({xv}, {yv}, {zv}) TAL: ({}, {}, {})",
            fmt_f(x, 0, 2),
            fmt_f(y, 0, 2),
            fmt_f(z, 0, 2)
        );
    }
    if !seed_set {
        check_slices(g, r, "the cutting plane's search", max_slices, voxsize)?;
        let n = max_slices.max(0) as usize;
        let mut slices: Vec<Mri> = Vec::with_capacity(n);
        let mut filled: Vec<Mri> = Vec::with_capacity(n);
        let mut area = vec![0i32; n];
        let mut aspects = vec![0f64; n];
        for slice in 0..max_slices {
            let offset = slice.wrapping_sub(half_slices);
            let o = f64::from(offset);
            let (x, y, z) = world_to_voxel(
                g,
                r.out,
                r.err,
                &mut mri_tal.vg,
                x_tal + dx * o,
                y_tal + dy * o,
                z_tal + dz * o,
            )?;
            let xv = nint(g, x);
            let yv = nint(g, y);
            let zv = nint(g, z);
            let where_ = match orientation {
                MRI_SAGITTAL => xv,
                MRI_CORONAL => zv,
                _ => yv,
            };
            let mut s = mri_extract_plane(g, r.out, r.err, mri_tal, None, orientation, where_)?
                .unwrap_new();
            let (f, a) = mri_fill_fg(g, r.out, r.err, Pair::New(&s), xo, yo, 0, WM_MIN_VAL, 127)?;
            let mut f = f.unwrap_new();
            let s_ = slice as usize;
            area[s_] = a;
            let mut region = mri_bounding_box(g, r.err, &f, 1)?.1;
            aspects[s_] = aspect_of(&region);
            if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
                let _ = writeln!(
                    r.err,
                    "cutting_plane_slice[{slice}] @ ({xv}, {yv}, {zv}): area = {a}"
                );
            }
            if orientation == MRI_SAGITTAL {
                region.dy = slice_size.wrapping_sub(region.y);
            }
            paint(g, &mut f, &region);
            if g.gdiag & DIAG_WRITE != 0 && g.gdiag & DIAG_VERBOSE != 0 {
                let tag = if orientation == MRI_SAGITTAL {
                    "cc"
                } else {
                    "pons"
                };
                write_diag(
                    g,
                    r,
                    &mut s,
                    &format!("cutting_plane_{tag}_slice{slice}.mgz"),
                )?;
                write_diag(
                    g,
                    r,
                    &mut f,
                    &format!("cutting_plane_{tag}_filled{slice}.mgz"),
                )?;
            }
            slices.push(s);
            filled.push(f);
        }
        let mut min_slice = -1i32;
        let mut min_area0;
        if orientation == MRI_HORIZONTAL {
            min_area0 = to_int(g, (10000.0 / vv).floor());
            for slice in 1..max_slices - 1 {
                let a = area[slice as usize];
                let _ = writeln!(
                    r.err,
                    "area[{slice}]={a}, min_area0 = {min_area0}, max_area = {max_area}"
                );
                if a < min_area0 && a >= min_area && a <= max_area {
                    min_area0 = a;
                    min_slice = slice;
                }
            }
        } else {
            let mut valid = vec![false; n];
            for slice in 1..max_slices - 1 {
                let s = slice as usize;
                valid[s] = area[s] >= min_area
                    && area[s] <= max_area
                    && aspects[s] >= min_aspect
                    && aspects[s] <= max_aspect;
            }
            let Some(s1) = slices.get(1) else {
                // FreeSurfer reads the size of a slice it never made (memory contents)
                g.notices.add(
                    &FS_1293,
                    format!(
                        "mri_fill: the corpus callosum's search with voxels of {voxsize} mm makes {max_slices} slice(s) and reads the size of the second (memory it does not own)"
                    ),
                );
                log::error_exit(
                    g,
                    r.err,
                    ERROR_BADPARM,
                    &format!(
                        "{NAME}: the corpus callosum's search takes voxels of 7.5 mm or less (the smallest here {voxsize} mm)"
                    ),
                )?;
                return Ok(None);
            };
            min_area0 = s1
                .vg
                .width
                .wrapping_mul(s1.vg.height)
                .wrapping_mul(s1.vg.depth);
            for slice in 1..max_slices - 1 {
                let s = slice as usize;
                if !valid[s] {
                    continue;
                }
                let slice_area = mri_total_voxels_on(g, r.err, &slices[s], WM_MIN_VAL)? as f32;
                let _ = writeln!(
                    r.err,
                    "slice={slice}, slice_area={}, min_area0 = {min_area0}, max_area = {max_area}",
                    fmt_f(f64::from(slice_area), 0, 1)
                );
                if slice_area < min_area0 as f32 {
                    min_area0 = slice_area as i32;
                    min_slice = slice;
                }
            }
        }
        let _ = writeln!(r.err, "min_slice = {min_slice}, min_area = {min_area0}");
        if min_slice < 0 {
            // FreeSurfer's statistics file stays open, empty (the process ends with it)
            return Ok(None);
        }
        let ms = min_slice as usize;
        if diag_verbose(g, DIAG_WRITE) {
            let tag = if orientation == MRI_SAGITTAL {
                "cc"
            } else {
                "pons"
            };
            write_diag(
                g,
                r,
                &mut slices[ms],
                &format!("cutting_plane_{tag}_minslice.mgz"),
            )?;
            write_diag(
                g,
                r,
                &mut filled[ms],
                &format!("cutting_plane_{tag}_minfilled.mgz"),
            )?;
        }
        if logging {
            match fp.take().flatten() {
                Some(mut f) => {
                    let _ = writeln!(
                        f.buf,
                        "area {}, aspect {}",
                        area[ms],
                        fmt_f(aspects[ms], 2, 2)
                    );
                    f.close();
                }
                None => {
                    g.notices.add(
                        &FS_1295,
                        format!(
                            "mri_fill -D: the statistics file of the {name} search did not open: FreeSurfer writes to it anyway and crashes"
                        ),
                    );
                    if g.notices.reproduce() {
                        return Err(Fatal::Exit(SEGFAULT));
                    }
                    let what = if orientation == MRI_HORIZONTAL {
                        PONS_LOG_FILE
                    } else {
                        CC_LOG_FILE
                    };
                    log::error_exit(
                        g,
                        r.err,
                        ERROR_BADPARM,
                        &format!("{NAME}: could not open the statistics file {what}"),
                    )?;
                }
            }
        }
        let offset = min_slice.wrapping_sub(half_slices);
        let o = f64::from(offset);
        let (x, y, z) = world_to_voxel(
            g,
            r.out,
            r.err,
            &mut mri_tal.vg,
            x_tal + dx * o,
            y_tal + dy * o,
            z_tal + dz * o,
        )?;
        pv.0 = nint(g, x);
        pv.1 = nint(g, y);
        pv.2 = nint(g, z);
        cut = Some(mri_copy(g, r.out, r.err, &filled[ms], None)?.unwrap_new());
        if g.gdiag & DIAG_SHOW != 0 {
            let _ = writeln!(
                r.err,
                "{name}: cutting at slice {min_slice}, area {min_area0}, ({}, {}, {})",
                pv.0, pv.1, pv.2
            );
        }
    }
    // with a seed FreeSurfer's statistics file stays open, empty
    drop(fp);
    let cut = cut.expect("made above");
    let mut cut_vol = mri_clone(g, r.out, r.err, mri_tal, None)?.unwrap_new();
    let where_ = where_of(orientation, pv.0, pv.1, pv.2);
    mri_fill_plane(g, r.err, &cut, &mut cut_vol, orientation, where_, 200)?;
    Ok(Some(cut_vol))
}

/// `find_slice_center(mri, &xo, &yo)`: of the voxels of slice 0 at or above `WM_MIN_VAL`, the
/// first (rows outermost) whose summed squared distance to all of them is smallest (`int` sums,
/// wrapping as FreeSurfer's do); then, of its 3 x 3 neighbourhood (rows outermost), the last
/// voxel at or above `WM_MIN_VAL` whose 8 neighbours (through the index tables) are all not 0,
/// the search stopping at the first such voxel.
pub fn find_slice_center(g: &mut Globals, mri: &Mri) -> (i32, i32) {
    let (width, height) = (mri.vg.width, mri.vg.height);
    let mut on: Vec<(i32, i32)> = Vec::new();
    for y in 0..height {
        for x in 0..width {
            if i32::from(vox(g, mri, x, y, 0)) >= WM_MIN_VAL {
                on.push((x, y));
            }
        }
    }
    let (mut xo, mut yo) = (0i32, 0i32);
    let mut min_total_dist = -1i32;
    for &(x, y) in &on {
        let mut total_dist = 0i32;
        for &(x1, y1) in &on {
            let (ddx, ddy) = (x1.wrapping_sub(x), y1.wrapping_sub(y));
            let dist = ddx.wrapping_mul(ddx).wrapping_add(ddy.wrapping_mul(ddy));
            total_dist = total_dist.wrapping_add(dist);
        }
        if min_total_dist < 0 || total_dist < min_total_dist {
            min_total_dist = total_dist;
            xo = x;
            yo = y;
        }
    }
    let mut border = true;
    let mut y = yo - 1;
    while border && y <= yo + 1 {
        if y < 0 || y >= height {
            y += 1;
            continue;
        }
        let mut x = xo - 1;
        while border && x <= xo + 1 {
            if x < 0 || x >= width {
                x += 1;
                continue;
            }
            if i32::from(vox(g, mri, x, y, 0)) >= WM_MIN_VAL {
                border = false;
                for y1 in y - 1..=y + 1 {
                    let yi = ix(g, mri, Ax::Y, y1);
                    for x1 in x - 1..=x + 1 {
                        let xi = ix(g, mri, Ax::X, x1);
                        if vox(g, mri, xi, yi, 0) == 0 {
                            border = true;
                        }
                    }
                }
            }
            if !border {
                xo = x;
                yo = y;
            }
            x += 1;
        }
        y += 1;
    }
    (xo, yo)
}

/// `find_corpus_callosum(mri_tal, &ccx, &ccy, &ccz, lta)`: in the coronal slice of Talairach
/// (0, 0, 0), the column within `CC_SPREAD` mm of the bounding box's centre whose first voxel at
/// or above `WM_MIN_VAL` starts lowest (largest row) and begins a run between 3 and 20 mm thick;
/// its middle in Talairach millimetres, refined by [`find_cc_slice`]. `ERROR_BADPARM` when no
/// column qualifies (the seed unchanged).
pub fn find_corpus_callosum(
    g: &mut Globals,
    r: &mut Run,
    mri_tal: &mut Mri,
    cc: &mut [f64; 3],
    lta: Option<&Lta>,
) -> Result<i32, Fatal> {
    let voxsize = find_min_size(mri_tal);
    let cc_spread = to_int(g, (f64::from(CC_SPREAD) / voxsize).ceil());
    let min_thickness = to_int(g, (f64::from(MIN_THICKNESS) / voxsize).ceil());
    let max_thickness = to_int(g, (f64::from(MAX_THICKNESS) / voxsize).ceil());
    let slice_size = mri_tal.vg.width;
    let region = mri_bounding_box(g, r.err, mri_tal, 1)?.1;
    let x0 = region.x.wrapping_add(region.dx / 2);
    let zv = if mri_tal.linear_transform.is_some() || lta.is_some() {
        let (xr, yr, zr) = world_to_voxel(g, r.out, r.err, &mut mri_tal.vg, 0.0, 0.0, 0.0)?;
        // the column's and row's voxels are not used (FreeSurfer computes them)
        nint(g, xr);
        nint(g, yr);
        nint(g, zr)
    } else {
        region.z.wrapping_add(region.dz / 2)
    };
    let (mut xcc, mut ycc, mut max_y) = (0i32, 0i32, 0i32);
    let mut x = x0.wrapping_sub(cc_spread);
    while x <= x0.wrapping_add(cc_spread) {
        let mut y = 0;
        while y < slice_size {
            if i32::from(vox(g, mri_tal, x, y, zv)) >= WM_MIN_VAL {
                break;
            }
            y += 1;
        }
        if y < slice_size && y > max_y {
            let mut y1 = y;
            let mut thickness = 0i32;
            while y1 < slice_size {
                if vox(g, mri_tal, x, y1, zv) == 0 {
                    break;
                }
                y1 += 1;
                thickness += 1;
            }
            if thickness > min_thickness && thickness < max_thickness {
                xcc = x;
                ycc = y + thickness / 2;
                max_y = y;
                if diag_verbose(g, DIAG_SHOW) {
                    let _ = writeln!(
                        r.err,
                        "potential cc found at ({xcc}, {ycc}), thickness = {thickness}"
                    );
                }
            }
        }
        x = x.wrapping_add(1);
    }
    if max_y == 0 {
        return Ok(ERROR_BADPARM);
    }
    let (a, b, c) = voxel_to_world(
        g,
        r.out,
        r.err,
        &mut mri_tal.vg,
        f64::from(xcc),
        f64::from(ycc),
        f64::from(zv),
    )?;
    *cc = [a, b, c];
    find_cc_slice(g, r, mri_tal, cc)?;
    Ok(NO_ERROR)
}

/// `find_cc_slice(mri_tal, &ccx, &ccy, &ccz, lta)`: the sagittal slices `x + offset` around the
/// seed (1 mm apart), each filled from the seed's voxel; the slice of smallest area within the
/// callosum's bounds (slices whose blob reaches the image's border do not count) moves the seed's
/// x. `ERROR_BADPARM` (`could not find valid seed for the cc`) when none qualifies.
pub fn find_cc_slice(
    g: &mut Globals,
    r: &mut Run,
    mri_tal: &mut Mri,
    cc: &mut [f64; 3],
) -> Result<i32, Fatal> {
    let voxsize = find_min_size(mri_tal);
    let slice_size = mri_tal.vg.width;
    let max_slices = to_int(g, (f64::from(MAX_SLICES) / voxsize).ceil());
    let vv = voxsize * voxsize;
    let max_cc_area = to_int(g, (f64::from(MAX_CC_AREA) / vv).ceil());
    let min_cc_area = to_int(g, (f64::from(MIN_CC_AREA) / vv).floor());
    let mut half_slices = to_int(g, (f64::from((MAX_SLICES - 1) / 2) / voxsize).floor());
    if half_slices <= 0 {
        half_slices = 1;
    }
    check_slices(g, r, "the corpus callosum's slices", max_slices, voxsize)?;
    let [x_tal, y_tal, z_tal] = *cc;
    let mut area = vec![0i32; max_slices.max(0) as usize];
    for slice in 0..max_slices {
        let offset = slice.wrapping_sub(half_slices);
        let (x, y, z) = world_to_voxel(
            g,
            r.out,
            r.err,
            &mut mri_tal.vg,
            x_tal + f64::from(offset),
            y_tal,
            z_tal,
        )?;
        let xv = nint(g, x);
        let yv = nint(g, y);
        let zv = nint(g, z);
        let mut s =
            mri_extract_plane(g, r.out, r.err, mri_tal, None, MRI_SAGITTAL, xv)?.unwrap_new();
        let (f, mut a) = mri_fill_fg(g, r.out, r.err, Pair::New(&s), zv, yv, 0, WM_MIN_VAL, 127)?;
        let mut f = f.unwrap_new();
        let region = mri_bounding_box(g, r.err, &f, 1)?.1;
        if region.x == 0
            || region.y == 0
            || region.x.wrapping_add(region.dx) >= slice_size.wrapping_sub(1)
            || region.y.wrapping_add(region.dy) >= slice_size.wrapping_sub(1)
        {
            a = 0;
        }
        area[slice as usize] = a;
        if diag_verbose(g, DIAG_SHOW) {
            let _ = writeln!(r.err, "slice[{slice}] @ ({xv}, {yv}, {zv}): area = {a}");
        }
        if diag_verbose(g, DIAG_WRITE) {
            write_diag(g, r, &mut s, &format!("cc_slice{slice}.mgz"))?;
            write_diag(g, r, &mut f, &format!("cc_filled{slice}.mgz"))?;
        }
    }
    let mut min_area = 10000;
    let mut min_slice = -1i32;
    for slice in 1..max_slices - 1 {
        let a = area[slice as usize];
        if a < min_area && a >= min_cc_area && a <= max_cc_area {
            min_area = a;
            min_slice = slice;
        }
    }
    if min_slice < 0 {
        let msg = format!("{}: could not find valid seed for the cc", g.progname);
        log::error_printf(g, r.err, ERROR_BADPARM, &msg);
        return Ok(ERROR_BADPARM);
    }
    let offset = min_slice.wrapping_sub(half_slices);
    let (x, y, z) = (x_tal + f64::from(offset), y_tal, z_tal);
    *cc = [x, y, z];
    let (xvv, yvv, zvv) = world_to_voxel(g, r.out, r.err, &mut mri_tal.vg, x, y, z)?;
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(
            r.err,
            "updating initial cc seed to Tal vol ({}, {}, {}) TAL ({}, {}, {})",
            fmt_f(xvv, 0, 2),
            fmt_f(yvv, 0, 2),
            fmt_f(zvv, 0, 2),
            fmt_f(x, 0, 2),
            fmt_f(y, 0, 2),
            fmt_f(z, 0, 2)
        );
    }
    Ok(NO_ERROR)
}

/// `find_pons(mri, &ponsx, &ponsy, &ponsz, x_cc, y_cc, z_cc, method)`: in the sagittal slice of
/// the callosum's column, from the bottom row up and from the front, the first run of white
/// matter of a brainstem's thickness (method 0; method 1: in the slice closed by `MRIclose`, the
/// first `MIN_CONT_BRAINSTEM_HEIGHT` such rows in a row with a thinner run); then up the brainstem's
/// contour to the first backward step, and the middle of the row's run there, in `mri`'s world
/// millimetres. `ERROR_BADPARM` (`could not find bottom of brainstem`, the pons unchanged) when the
/// contour leaves the slice.
#[allow(clippy::too_many_arguments)]
pub fn find_pons(
    g: &mut Globals,
    r: &mut Run,
    mri: &mut Mri,
    pons: &mut [f64; 3],
    x_cc: i32,
    _y_cc: i32,
    _z_cc: i32,
    method: i32,
) -> Result<i32, Fatal> {
    let voxsize = find_min_size(mri);
    let max_bt = to_int(g, (f64::from(MAX_BRAINSTEM_THICKNESS) / voxsize).ceil());
    let min_bt = to_int(g, (f64::from(MIN_BRAINSTEM_THICKNESS) / voxsize).floor());
    let min_bh = to_int(g, (f64::from(MIN_BRAINSTEM_HEIGHT) / voxsize).floor());
    let min_dt = to_int(g, (f64::from(MIN_DELTA_THICKNESS) / voxsize).floor());
    let min_cbt = to_int(
        g,
        (f64::from(MIN_CONT_BRAINSTEM_THICKNESS) / voxsize).floor(),
    );
    let min_cbh = to_int(g, (f64::from(MIN_CONT_BRAINSTEM_HEIGHT) / voxsize).floor());
    let _ = writeln!(
        r.err,
        "brainstem (min, max) thickness = ({min_bt},{max_bt})"
    );
    let _ = writeln!(r.err, "brainstem min height = {min_bh}");
    let _ = writeln!(r.err, "min_delta_thickness = {min_dt}");
    let _ = writeln!(r.err, "min_cont_brainstem_thickness = {min_cbt}");
    let _ = writeln!(r.err, "min_cont_brainstem_height = {min_cbh}");
    let mut thickness = -1i32;
    let mut xstart = -1i32;
    let region = mri_bounding_box(g, r.err, mri, 10)?.1;
    let xr = f64::from(region.x.wrapping_add(region.dx / 2));
    let yr = f64::from(region.y.wrapping_add(region.dy / 2));
    let zr = f64::from(region.z.wrapping_add(region.dz / 2));
    let xv = x_cc;
    // the box's centre in world millimetres (unused, as in FreeSurfer)
    voxel_to_world(g, r.out, r.err, &mut mri.vg, xr, yr, zr)?;
    let mut slice = mri_extract_plane(g, r.out, r.err, mri, None, MRI_SAGITTAL, xv)?.unwrap_new();
    if diag_verbose(g, DIAG_WRITE) {
        write_diag(g, r, &mut slice, "brainstem_slice.mgz")?;
    }
    let width = slice.vg.width;
    let height = slice.vg.height;
    // `while (MRIvox(m, x, y, 0) && x >= 0)`: FreeSurfer reads the voxel before x = -1 too, which
    // never changes the result
    let run_from =
        |g: &mut Globals, m: &Mri, x: &mut i32, y: i32, thickness: &mut i32, xstart: &mut i32| {
            *thickness = 0;
            while *x >= 0 && vox(g, m, *x, y, 0) != 0 {
                if *thickness == 0 {
                    *xstart = *x;
                }
                *thickness += 1;
                *x -= 1;
            }
        };
    let mut y;
    if method == 1 {
        let closed = mri_close(g, r.out, r.err, Pair::New(&slice))?.unwrap_new();
        let mut closed = closed;
        if diag_verbose(g, DIAG_WRITE) {
            write_diag(g, r, &mut closed, "brainstem_closed.mgz")?;
        }
        let mut bheight = 0;
        y = height - 1;
        while y >= 0 {
            let mut x = width - 1;
            while x >= 0 {
                run_from(g, &closed, &mut x, y, &mut thickness, &mut xstart);
                if thickness >= min_cbt && thickness <= max_bt {
                    break;
                } else {
                    thickness = 0;
                }
                x -= 1;
            }
            if thickness > 0 {
                bheight += 1;
            } else {
                bheight = 0;
            }
            if bheight >= min_cbh {
                break;
            }
            y -= 1;
        }
    } else {
        y = height - 1;
        while y >= 0 {
            let mut x = width - 1;
            while x >= 0 {
                run_from(g, &slice, &mut x, y, &mut thickness, &mut xstart);
                if thickness >= min_bt && thickness <= max_bt {
                    break;
                } else {
                    thickness = 0;
                }
                x -= 1;
            }
            if thickness > 0 {
                break;
            }
            y -= 1;
        }
    }
    let seed_x = xstart.wrapping_sub(thickness.wrapping_sub(1) / 2);
    let (f, area) = mri_fill_fg(
        g,
        r.out,
        r.err,
        Pair::New(&slice),
        seed_x,
        y,
        0,
        WM_MIN_VAL,
        127,
    )?;
    let mut filled = f.unwrap_new();
    if diag_verbose(g, DIAG_SHOW) {
        let _ = writeln!(
            r.err,
            "found brainstem at y={y}, x = ({xstart} --> {}), area={area}",
            xstart - thickness + 1
        );
    }
    if diag_verbose(g, DIAG_WRITE) {
        write_diag(g, r, &mut filled, "brainstem_filled.mgz")?;
    }
    // the first column from the front of a row that is on in the filled slice (-1: none)
    let front = |g: &mut Globals, m: &Mri, y: i32| {
        let mut x = width - 1;
        while x >= 0 && vox(g, m, x, y, 0) == 0 {
            x -= 1;
        }
        x
    };
    // FreeSurfer's `xmax = x = xstart`: x is set again before it is read
    let mut x;
    let mut xmax = xstart;
    let mut bheight = 0;
    loop {
        y -= 1;
        bheight += 1;
        if y < 0 {
            let msg = "could not find bottom of brainstem";
            log::error_printf(g, r.err, ERROR_BADPARM, msg);
            return Ok(ERROR_BADPARM);
        }
        x = front(g, &filled, y);
        if x < 0 {
            y += 1;
            x = front(g, &filled, y);
            break;
        }
        if diag_verbose(g, DIAG_SHOW) {
            let _ = writeln!(r.err, "slice {y}, xstart {x}");
        }
        if x > xmax {
            if bheight > min_bh && x - xmax >= min_dt {
                y -= 1;
                break;
            }
            xmax = x;
        }
        if !(xmax - x < min_dt && y > 0) {
            break;
        }
    }
    let mut thickness = 0;
    let xstart = x;
    while x >= 0 && thickness < max_bt && vox(g, &slice, x, y, 0) != 0 {
        thickness += 1;
        x -= 1;
    }
    let x = xstart.wrapping_sub(thickness.wrapping_sub(1) / 2);
    let (zv, yv) = (x, y);
    let (a, b, c) = voxel_to_world(
        g,
        r.out,
        r.err,
        &mut mri.vg,
        f64::from(xv),
        f64::from(yv),
        f64::from(zv),
    )?;
    *pons = [a, b, c];
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(
            r.err,
            "putative pons found at {}, {y} -> ({xv}, {yv}, {zv}) TAL=({}, {}, {})",
            xstart - thickness / 2,
            fmt_f(a, 0, 2),
            fmt_f(b, 0, 2),
            fmt_f(c, 0, 2)
        );
    }
    Ok(NO_ERROR)
}

/// FS-1294's notice: the pons' backup searches look in a volume of the input's space.
pub fn note_backup_space(g: &mut Globals, what: &str) {
    g.notices.add(
        &FS_1294,
        format!(
            "mri_fill: {what} runs in the input volume's space with Talairach coordinates, as in FreeSurfer (kept)"
        ),
    );
}

/// `neighbors_on(mri, x0, y0, z0)`: how many of the six face neighbours are at or above
/// `WM_MIN_VAL` (`MRIvox`).
pub fn neighbors_on(g: &mut Globals, mri: &Mri, x0: i32, y0: i32, z0: i32) -> i32 {
    let mut n = 0;
    for (x, y, z) in [
        (x0 - 1, y0, z0),
        (x0 + 1, y0, z0),
        (x0, y0 + 1, z0),
        (x0, y0 - 1, z0),
        (x0, y0, z0 + 1),
        (x0, y0, z0 - 1),
    ] {
        if i32::from(vox(g, mri, x, y, z)) >= WM_MIN_VAL {
            n += 1;
        }
    }
    n
}

/// `extend_to_lateral_borders(mri, mri, mask)` in place: every row (y outermost, then z) holding
/// a voxel of `mask` set to `mask` from side to side.
pub fn extend_to_lateral_borders(g: &mut Globals, mri: &mut Mri, mask: u8) {
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    for y in 0..h {
        for z in 0..d {
            let found = (0..w).any(|x| vox(g, mri, x, y, z) == mask);
            if found {
                for x in 0..w {
                    set_vox(g, mri, x, y, z, mask);
                }
            }
        }
    }
}

/// `find_cc_seed_with_segmentation(mri_tal, mri_seg_tal, &x, &y, &z)`: of the segmentation's
/// white-matter voxels (x outermost) whose left and right neighbours are white matter of which one
/// differs from it, the first with the most such midline voxels in its 11 x 11 sagittal window
/// (among those on in `mri`); its Talairach millimetres. No such voxel: `ERROR_BADFILE`, the
/// program ends.
pub fn find_cc_seed_with_segmentation(
    g: &mut Globals,
    r: &mut Run,
    mri: &mut Mri,
    seg: &Mri,
    cc: &mut [f64; 3],
) -> Result<i32, Fatal> {
    let sv = Vals::new(g, r.err, seg)?;
    let (mut xcc, mut ycc, mut zcc) = (0.0f64, 0.0f64, 0.0f64);
    let mut max_num = 0i32;
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let label = sv.label(g, r.err, x, y, z)?;
                if !is_wm(label) {
                    continue;
                }
                let xr = ix(g, seg, Ax::X, x + 1);
                let xl = ix(g, seg, Ax::X, x - 1);
                let rlabel = sv.label(g, r.err, xr, y, z)?;
                let llabel = sv.label(g, r.err, xl, y, z)?;
                if !is_wm(rlabel) || !is_wm(llabel) || (rlabel == label && llabel == label) {
                    continue;
                }
                let mut num = 0;
                for yk in -WHALF..=WHALF {
                    let yi = ix(g, seg, Ax::Y, y + yk);
                    for zk in -WHALF..=WHALF {
                        let zi = ix(g, seg, Ax::Z, z + zk);
                        let rlabel = sv.label(g, r.err, xr, yi, zi)?;
                        let llabel = sv.label(g, r.err, xl, yi, zi)?;
                        let label = sv.label(g, r.err, x, yi, zi)?;
                        if i32::from(vox(g, mri, x, yi, zi)) < WM_MIN_VAL {
                            continue;
                        }
                        if is_wm(rlabel)
                            && is_wm(llabel)
                            && is_wm(label)
                            && (rlabel != label || llabel != label)
                        {
                            num += 1;
                        }
                    }
                }
                if num > max_num {
                    xcc = f64::from(x);
                    ycc = f64::from(y);
                    zcc = f64::from(z);
                    max_num = num;
                }
            }
        }
    }
    if max_num <= 0 {
        let msg = format!(
            "{}: could not find any points where lh and rh wm  are nbrs",
            g.progname
        );
        log::error_exit(g, r.err, ERROR_BADFILE, &msg)?;
    }
    let (a, b, c) = voxel_to_world(g, r.out, r.err, &mut mri.vg, xcc, ycc, zcc)?;
    *cc = [a, b, c];
    let (xi, yi, zi) = (nint(g, xcc), nint(g, ycc), nint(g, zcc));
    let _ = writeln!(
        r.out,
        "segmentation indicates cc at ({xi},  {yi},  {zi}) --> ({}, {}, {})",
        fmt_f(a, 2, 1),
        fmt_f(b, 2, 1),
        fmt_f(c, 2, 1)
    );
    Ok(NO_ERROR)
}

/// `find_rh_seed_point(mri, &x, &y, &z)`: the first voxel (x outermost) whose 26 neighbours are
/// all above `WM_MIN_VAL` (`MRIneighborsOn3x3`); none: `could not find rh seed point`, the program
/// ends.
pub fn find_rh_seed_point(
    g: &mut Globals,
    r: &mut Run,
    mri: &Mri,
) -> Result<(i32, i32, i32), Fatal> {
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                if neighbors_on_3x3(g, r.err, mri, x, y, z, WM_MIN_VAL)? >= 26 {
                    let _ = writeln!(r.out, "seed point found at ({x}, {y}, {z})");
                    return Ok((x, y, z));
                }
            }
        }
    }
    log::error_exit(g, r.err, ERROR_BADPARM, "could not find rh seed point")?;
    Ok((0, 0, 0))
}

/// `mri_erase_nonmidline_voxels(mri_cc, mri_seg_tal)`: the cutting plane's voxels (x outermost)
/// that are not where the left and right neighbours are white matter of which one differs from the
/// voxel's label set to 0. (FreeSurfer also marks a copy of the plane, written as `mid.mgz` with
/// `DIAG_WRITE`.)
pub fn mri_erase_nonmidline_voxels(
    g: &mut Globals,
    r: &mut Run,
    cc: &mut Mri,
    seg: &Mri,
) -> Result<i32, Fatal> {
    let sv = Vals::new(g, r.err, seg)?;
    let mut mid = mri_copy(g, r.out, r.err, cc, None)?.unwrap_new();
    let (w, h, d) = (cc.vg.width, cc.vg.height, cc.vg.depth);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let xr = ix(g, seg, Ax::X, x + 1);
                let xl = ix(g, seg, Ax::X, x - 1);
                let label = sv.label(g, r.err, x, y, z)?;
                let rlabel = sv.label(g, r.err, xr, y, z)?;
                let llabel = sv.label(g, r.err, xl, y, z)?;
                if is_wm(rlabel) && !is_wm(llabel) && (rlabel != label || llabel != label) {
                    set_vox(g, &mut mid, x, y, z, 128);
                }
                if vox(g, cc, x, y, z) == 0 {
                    continue;
                }
                if !is_wm(rlabel) || !is_wm(llabel) || (rlabel == label && llabel == label) {
                    set_vox(g, cc, x, y, z, 0);
                }
            }
        }
    }
    if diag_verbose(g, DIAG_WRITE) {
        write_diag(g, r, &mut mid, "mid.mgz")?;
    }
    Ok(NO_ERROR)
}

/// `MRIreplaceCCwithWM(mri_src, NULL)`: a copy of the segmentation whose corpus callosum labels
/// (`IS_CC`) become left or right cerebral white matter, whichever is nearer by the signed distance
/// transforms of the two labels (out to 100 mm; a tie goes right).
pub fn mri_replace_cc_with_wm(
    g: &mut Globals,
    r: &mut Run,
    env: &Environment,
    src: &Mri,
) -> Result<Mri, Fatal> {
    let mut dst = mri_copy(g, r.out, r.err, src, None)?.unwrap_new();
    let dist_lh = mri_distance_transform(
        g,
        r.out,
        r.err,
        env,
        src,
        None,
        Left_Cerebral_White_Matter,
        100.0,
        larmorx_freesurfer_filter::fastmarch::DTRANS_MODE_SIGNED,
        None,
    )?
    .unwrap_new();
    let dist_rh = mri_distance_transform(
        g,
        r.out,
        r.err,
        env,
        src,
        None,
        Right_Cerebral_White_Matter,
        100.0,
        larmorx_freesurfer_filter::fastmarch::DTRANS_MODE_SIGNED,
        None,
    )?
    .unwrap_new();
    let dv = Vals::new(g, r.err, src)?;
    let lv = Vals::new(g, r.err, &dist_lh)?;
    let rv = Vals::new(g, r.err, &dist_rh)?;
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let label = dv.label(g, r.err, x, y, z)?;
                if is_cc(label) {
                    let dist_lh = f64::from(lv.get(g, r.err, x, y, z)?);
                    let dist_rh = f64::from(rv.get(g, r.err, x, y, z)?);
                    let v = if dist_lh < dist_rh {
                        Left_Cerebral_White_Matter
                    } else {
                        Right_Cerebral_White_Matter
                    };
                    mri_set_vox_val(g, r.err, &mut dst, x, y, z, 0, v as f32, 0)?;
                }
            }
        }
    }
    Ok(dst)
}
