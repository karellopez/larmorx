// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_cc`'s segmentation from the white matter (`-a 0`; `mri_cc.cpp` 288-436 in `main`,
//! `find_corpus_callosum` 764, `find_cc_slice` 882, `remove_fornix` 1069, `edge_detection` 1343):
//! the path recon-all never runs.
//!
//! `<sdir>/<subject>/mri/wm` (no suffix: a COR folder, or a file whose format the readers find
//! without one) and `mri/transforms/talairach.xfm` read; the fill labels (200, 210, 220, 230, 240)
//! cleared; the white matter resampled into Talairach space (the transform as it is) and upright
//! (its rotation only: the transform without its translation divided by its column lengths, the
//! translation put back), both binarized at 54 (0 and 110). In Talairach space: the column of the
//! callosum's top found from the voxel of Talairach (0, 0, 0) (the topmost start of a run of white
//! matter between 3 and 20 mm thick within 10 mm of it), then 21 sagittal slices around it
//! (`MRIfillFG` from the seed, moved up to five voxels down until 100 voxels fill), each slice's
//! fornix cut off (`remove_fornix`) unless it is large and well placed, the slices' areas choosing
//! the midline's offset, every slice pasted into the callosum volume; that volume brought back to
//! the white matter's space, binarized at 49 (0 and 100) and written as `mri/cc_org.mgz`; the
//! callosum's centre found in the upright volume.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-cc.md`):
//! - **FS-1205** (crash): `mri/wm` that does not read (none, or a file without a format FreeSurfer
//!   finds: recon-all writes `wm.mgz`); FreeSurfer goes on and crashes once the transform is read.
//!   By default `mri_cc: could not read white matter volume from <f>`, `ERROR_NOFILE`.
//! - **FS-1206** (memory): voxels smaller than 0.5 mm give more than 42 slices, past the slice
//!   tables: an error in both modes (rule B).
//! - **FS-1207** (memory: never written): voxels larger than 1.3125 mm give fewer than 17 slices,
//!   and the choice of the midline reads slices that were never made: 0 in both modes, noted.
//! - **FS-1208** (memory): the fornix cut's first row compares its section with the row before
//!   it, which does not exist (the table's element -1).
//! - **FS-1209** (hang): the fornix cut's edge searches stop at voxel 256; a sagittal slice
//!   taller than 258 voxels or wider than 257 never ends there: an error in both modes (rule B).
//! - **FS-1210** (rule A): a seed search that fails (no column, or no slice with a callosum's area)
//!   leaves the seed in Talairach millimetres where its voxel is used: by default the seed's voxel,
//!   noted; `reproduce` FreeSurfer's.

use std::io::Write;

use larmorx_core::defects::{FS_1205, FS_1206, FS_1207, FS_1208, FS_1209, FS_1210};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE, DIAG_WRITE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NOFILE, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_gca::transform::GcaLtaIo;
use larmorx_freesurfer_io::mriio::mri_read_volume;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::lta::Lta;
use larmorx_freesurfer_mri::mri::{Mri, mri_alloc, mri_alloc_header, mri_copy_header};
use larmorx_freesurfer_mri::vol_geom::{voxel_to_world, world_to_voxel};
use larmorx_freesurfer_mri::voxel::{mri_bounding_box, mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, mri_binarize, mri_replace_values};
use larmorx_freesurfer_volume::coords::MRI_SAGITTAL;
use larmorx_freesurfer_volume::ltaread::lta_read_ex;
use larmorx_freesurfer_volume::mritransform::trns_extract_rotation_matrix;
use larmorx_freesurfer_volume::resize::{
    mri_extract_plane, mri_fill_fg, mri_fill_plane, mri_value_fill,
};
use larmorx_freesurfer_volume::talairachex::{
    modify_talairach_cras, mri_from_talairach_ex, mri_talairach_voxel_to_voxel_ex,
    mri_to_talairach_ex, mri_voxel_to_talairach_voxel_ex,
};

use super::aseg::{CC_VAL, write_volume};
use super::main::{NAME, Options, Run, path_in};

/// `WM_MIN_VAL` (`mri.h`).
const WM_MIN_VAL: i32 = 5;
/// `DEFAULT_DESIRED_WHITE_MATTER_VALUE / 2 - 1` (`mrinorm.h`: 110).
const WM_THRESH: f32 = 54.0;
/// The fill labels `mri_fill` leaves in `wm` (`THICKEN_FILL`, `NBHD_FILL`, `VENTRICLE_FILL`,
/// `DIAGONAL_FILL`, `DEGENERATE_FILL`).
const FILL_LABELS: [f32; 5] = [200.0, 210.0, 220.0, 230.0, 240.0];
/// `MAX_SLICES * 2`: the size of `find_cc_slice`'s tables.
const SLICE_TABLE: i32 = 42;
/// `section1[150]`'s first row (`remove_fornix`'s second search).
const SECTIONS: usize = 150;
/// FreeSurfer's default seed (`cc_tal_x`, `cc_tal_y`, `cc_tal_z`), in Talairach millimetres.
const DEFAULT_SEED: [f64; 3] = [0.0, 0.0, 27.0];

/// What the white-matter path leaves for `main`: the upright callosum (`mri_cc`), its centre's
/// voxel, and `cc_org.mgz` when it could not be written (FS-1204).
pub struct WmFound {
    pub cc: Mri,
    pub xc: f64,
    pub yc: f64,
    pub zc: f64,
    pub unwritten: Option<String>,
}

fn diag_verbose(g: &Globals, flag: u64) -> bool {
    g.gdiag & flag != 0 && g.gdiag & DIAG_VERBOSE != 0
}

/// `nint(MRIgetVoxVal(mri, x, y, z, 0))`.
fn nv(g: &mut Globals, r: &mut Run, mri: &Mri, x: i32, y: i32, z: i32) -> Result<i32, Fatal> {
    let v = mri_get_vox_val(g, r.err, mri, x, y, z, 0, 0)?;
    Ok(nint(g, f64::from(v)))
}

/// `MRIsetVoxVal(mri, x, y, z, 0, v)`.
fn set(g: &mut Globals, r: &mut Run, mri: &mut Mri, x: i32, y: i32, v: f32) -> Result<(), Fatal> {
    mri_set_vox_val(g, r.err, mri, x, y, 0, 0, v, 0)?;
    Ok(())
}

/// `findMinSize(mri)`: the smallest voxel size (as `mri_cc.cpp` orders the three).
fn find_min_size(mri: &Mri) -> f64 {
    let (x, y, z) = (
        f64::from(mri.vg.xsize),
        f64::from(mri.vg.ysize),
        f64::from(mri.vg.zsize),
    );
    if x > y {
        if y > z { z } else { y }
    } else if z > x {
        x
    } else {
        z
    }
}

/// `(int)` of a `double` (FS-17 beyond `int`).
fn to_int(g: &mut Globals, v: f64) -> i32 {
    float_to_int(g, v, NAME)
}

/// `MRIbinarize(mri, mri, thresh, 0, hi)` in place.
fn binarize(
    g: &mut Globals,
    r: &mut Run,
    mri: &mut Mri,
    thresh: f32,
    hi: f32,
) -> Result<(), Fatal> {
    mri_binarize(g, r.out, r.err, Pair::Same(mri), thresh, 0.0, hi)?;
    Ok(())
}

/// `LTAreadEx(<sdir>/<subject>/mri/transforms/talairach.xfm)`.
fn read_talairach(g: &mut Globals, r: &mut Run, fname: &str) -> Result<Option<Lta>, Fatal> {
    let mut io = GcaLtaIo { env: r.env };
    lta_read_ex(g, r.out, r.err, r.dir, fname, &mut io)
}

/// The white-matter path of `main` (`mri_cc.cpp` 288-436), after `-a 0`.
pub fn find_cc_with_wm(
    g: &mut Globals,
    r: &mut Run,
    _o: &Options,
    data_dir: &str,
    subject: &str,
) -> Result<WmFound, Fatal> {
    let ifname = path_in(g, r, format!("{data_dir}/{subject}/mri/wm"), 293);
    let _ = writeln!(r.out, "reading white matter volume from {ifname}");
    let wm = mri_read_volume(g, r.out, r.err, r.env, r.dir, &ifname)?;
    let xfm = path_in(
        g,
        r,
        format!("{data_dir}/{subject}/mri/transforms/talairach.xfm"),
        300,
    );
    let Some(lta) = read_talairach(g, r, &xfm)? else {
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!("ERROR: cound not load lta from {xfm}.\n"),
        )?;
        unreachable!("ErrorExit ends the program")
    };
    let _ = writeln!(
        r.out,
        "INFO: Using {xfm} and its offset for Talairach volume ..."
    );
    let Some(mut wm) = wm else {
        g.notices.add(
            &FS_1205,
            format!(
                "mri_cc -a 0: the white matter volume {ifname} did not read: FreeSurfer crashes"
            ),
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        log::error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{NAME}: could not read white matter volume from {ifname}"),
        )?;
        unreachable!("ErrorExit ends the program")
    };
    for v in FILL_LABELS {
        mri_replace_values(g, r.out, r.err, Pair::Same(&mut wm), v, 0.0)?;
    }
    let (w, h, d, t) = (wm.vg.width, wm.vg.height, wm.vg.depth, wm.mtype);
    let mut talheader = mri_alloc_header(g, r.out, r.err, w, h, d, t, 1)?;
    mri_copy_header(g, r.out, r.err, &wm, Some(&mut talheader))?;
    modify_talairach_cras(g, r.out, r.err, r.env, &mut talheader, Some(&lta))?;
    let mut tal = mri_alloc(g, r.out, r.err, w, h, d, t)?;
    mri_copy_header(g, r.out, r.err, &talheader, Some(&mut tal))?;
    mri_to_talairach_ex(g, r.out, r.err, r.env, &wm, Some(&mut tal), Some(&lta))?;
    binarize(g, r, &mut tal, WM_THRESH, 110.0)?;
    if diag_verbose(g, DIAG_WRITE) {
        let ofname = path_in(g, r, format!("{data_dir}/{subject}/mri/wm_tal.mgz"), 342);
        let _ = writeln!(
            r.out,
            "writing talairach transformed white matter volume to {ofname}..."
        );
        write_volume(g, r, &mut tal, &ofname)?;
    }

    // the transform's rotation (`Trns_ExtractRotationMatrix`) with its translation
    let xfm = path_in(
        g,
        r,
        format!("{data_dir}/{subject}/mri/transforms/talairach.xfm"),
        357,
    );
    let Some(mut lta2) = read_talairach(g, r, &xfm)? else {
        // FreeSurfer dereferences the second read's NULL; it read the same file a moment ago
        g.notices.add(
            &FS_1205,
            format!(
                "mri_cc -a 0: the transform {xfm} did not read a second time: FreeSurfer crashes"
            ),
        );
        return Err(Fatal::Exit(if g.notices.reproduce() { 139 } else { 1 }));
    };
    let mut mrot = matrix_alloc(g, r.out, r.err, 4, 4, MATRIX_REAL)?.expect("a real matrix");
    let mtrans = lta2.xforms.first().and_then(|x| x.m_l.clone());
    trns_extract_rotation_matrix(g, r.out, r.err, mtrans.as_ref(), Some(&mut mrot))?;
    if let Some(mt) = mtrans.as_ref() {
        for row in 1..=3 {
            mrot.set_relt(row, 4, mt.relt(row, 4));
        }
    }
    if let Some(x) = lta2.xforms.first_mut() {
        x.m_l = Some(mrot);
    }
    let mut header = mri_alloc_header(g, r.out, r.err, w, h, d, t, 1)?;
    mri_copy_header(g, r.out, r.err, &wm, Some(&mut header))?;
    modify_talairach_cras(g, r.out, r.err, r.env, &mut header, Some(&lta2))?;
    let mut cc = mri_copy(g, r.out, r.err, &wm, None)?.unwrap_new();
    mri_copy_header(g, r.out, r.err, &header, Some(&mut cc))?;
    mri_to_talairach_ex(g, r.out, r.err, r.env, &wm, Some(&mut cc), Some(&lta2))?;
    binarize(g, r, &mut cc, WM_THRESH, 110.0)?;
    if diag_verbose(g, DIAG_WRITE) {
        let ofname = path_in(g, r, format!("{data_dir}/{subject}/mri/wm.mgz"), 385);
        let _ = writeln!(r.out, "writing rotated white matter volume to {ofname}...");
        write_volume(g, r, &mut cc, &ofname)?;
    }

    // the callosum in Talairach space
    let mut cc_tal = mri_copy(g, r.out, r.err, &tal, None)?.unwrap_new();
    mri_copy_header(g, r.out, r.err, &talheader, Some(&mut cc_tal))?;
    mri_value_fill(g, r.err, &mut cc_tal, 0.0)?;
    let mut seed = DEFAULT_SEED;
    let found = find_corpus_callosum(g, r, &mut tal, &mut seed, &mut cc_tal)?;
    if found != Found::Slice {
        seed_in_voxels(g, r, &mut tal, &mut seed, found)?;
    }
    if diag_verbose(g, DIAG_WRITE) {
        let ofname = path_in(g, r, format!("{data_dir}/{subject}/mri/cc_tal.mgz"), 406);
        let _ = writeln!(r.out, "writing output to {ofname}...");
        write_volume(g, r, &mut cc_tal, &ofname)?;
    }

    // back in the white matter's space
    mri_from_talairach_ex(g, r.out, r.err, r.env, &cc_tal, Some(&mut wm), Some(&lta))?;
    binarize(g, r, &mut wm, (CC_VAL / 2 - 1) as f32, 100.0)?;
    let ofname = path_in(g, r, format!("{data_dir}/{subject}/mri/cc_org.mgz"), 420);
    let _ = writeln!(
        r.out,
        "writing corpus callosum in original space to {ofname}..."
    );
    let unwritten = (write_volume(g, r, &mut wm, &ofname)? != 0).then_some(ofname);

    // the midsagittal plane's position, in the upright volume
    mri_copy_header(g, r.out, r.err, &talheader, Some(&mut cc_tal))?;
    let (_, p) = mri_talairach_voxel_to_voxel_ex(
        g,
        r.out,
        r.err,
        r.env,
        &cc_tal,
        seed[0],
        seed[1],
        seed[2],
        Some(&lta),
    )?;
    let Some([xv, yv, zv]) = p else {
        return Err(Fatal::Exit(1));
    };
    mri_copy_header(g, r.out, r.err, &header, Some(&mut cc))?;
    mri_to_talairach_ex(g, r.out, r.err, r.env, &wm, Some(&mut cc), Some(&lta2))?;
    binarize(g, r, &mut cc, (CC_VAL / 2 - 1) as f32, 100.0)?;
    let (_, p) =
        mri_voxel_to_talairach_voxel_ex(g, r.out, r.err, r.env, &cc, xv, yv, zv, Some(&lta2))?;
    let Some([xc, yc, zc]) = p else {
        return Err(Fatal::Exit(1));
    };
    Ok(WmFound {
        cc,
        xc,
        yc,
        zc,
        unwritten,
    })
}

/// How the seed search ended: no column (`find_corpus_callosum`'s `ERROR_BADPARM`), no slice
/// (`find_cc_slice`'s), or the seed moved to the chosen slice (in voxels).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Found {
    NoColumn,
    NoSlice,
    Slice,
}

/// FS-1210: the seed left in Talairach millimetres where the next step reads a voxel. By default
/// its voxel (`MRIworldToVoxel` in the Talairach volume), noted; `reproduce` FreeSurfer's.
fn seed_in_voxels(
    g: &mut Globals,
    r: &mut Run,
    tal: &mut Mri,
    seed: &mut [f64; 3],
    found: Found,
) -> Result<(), Fatal> {
    let why = match found {
        Found::NoColumn => "no column of the callosum's thickness",
        _ => "no slice with a callosum's area",
    };
    g.notices.add(
        &FS_1210,
        if g.notices.reproduce() {
            format!(
                "mri_cc -a 0: the seed search found {why}: its seed in Talairach millimetres is read as a voxel, as in FreeSurfer"
            )
        } else {
            format!(
                "mri_cc -a 0: the seed search found {why}: the seed's voxel used (FreeSurfer reads its Talairach millimetres as a voxel)"
            )
        },
    );
    if !g.notices.reproduce() {
        let (x, y, z) = world_to_voxel(g, r.out, r.err, &mut tal.vg, seed[0], seed[1], seed[2])?;
        *seed = [x, y, z];
    }
    Ok(())
}

/// `find_corpus_callosum(mri_tal, &x, &y, &z, lta, mri_cc_tal)`: the seed's column, then
/// [`find_cc_slice`]. `seed` keeps FreeSurfer's default when no column is found.
fn find_corpus_callosum(
    g: &mut Globals,
    r: &mut Run,
    tal: &mut Mri,
    seed: &mut [f64; 3],
    cc_tal: &mut Mri,
) -> Result<Found, Fatal> {
    let voxsize = find_min_size(tal);
    let cc_spread = i64::from(to_int(g, (10.0 / voxsize).ceil()));
    let min_thickness = to_int(g, (3.0 / voxsize).ceil());
    let max_thickness = to_int(g, (20.0 / voxsize).ceil());
    let slice_size = tal.vg.width;
    mri_bounding_box(g, r.err, tal, 1)?;
    // with a transform, the voxel of Talairach (0, 0, 0)
    let (xr, yr, zr) = world_to_voxel(g, r.out, r.err, &mut tal.vg, 0.0, 0.0, 0.0)?;
    let xv = nint(g, xr);
    let yv = nint(g, yr);
    let zv = nint(g, zr);
    let _ = writeln!(r.out, "original seed found at x={xv}, y={yv} z={zv} ");
    let x0 = i64::from(xv);
    let yv64 = i64::from(yv);
    let (mut xcc, mut ycc, mut max_y) = (0i32, 0i32, 0i32);
    let (mut max_thick, mut thickness, mut flag, mut counts) = (0i32, 0i32, 0i32, 0i32);
    let mut extension: i64 = 50;
    let mut x = x0 - cc_spread;
    while x <= x0 + cc_spread {
        let xi = x as i32;
        while thickness == 0 && yv64 - extension >= 0 && yv64 + extension <= 256 {
            let mut y = yv64 - extension;
            while y < yv64 + extension {
                if nv(g, r, tal, xi, y as i32, zv)? >= WM_MIN_VAL {
                    break;
                }
                y += 1;
            }
            if y < yv64 + extension {
                let y = y as i32;
                let mut y1 = y;
                thickness = 0;
                while y1 < slice_size {
                    if nv(g, r, tal, xi, y1, zv)? == 0 {
                        break;
                    }
                    y1 += 1;
                    thickness += 1;
                }
                if thickness > min_thickness && thickness < max_thickness {
                    if y > max_y || (y == max_y && thickness > max_thick) {
                        xcc = xi;
                        ycc = y + thickness / 2;
                        max_y = y;
                        max_thick = thickness;
                        if diag_verbose(g, DIAG_SHOW) {
                            let _ = writeln!(
                                r.out,
                                "potential cc found at ({xcc}, {ycc}), thickness = {thickness}"
                            );
                        }
                    } else if y == max_y
                        && thickness == max_thick
                        && (xi == xcc.wrapping_add(1) || flag == 1)
                    {
                        flag = 1;
                        counts += 1;
                    } else if flag == 1 {
                        xcc = xcc.wrapping_add(counts / 2);
                        flag = 0;
                        counts = 0;
                    }
                }
            }
            extension += 10;
        }
        thickness = 0;
        extension = 50;
        x += 1;
    }
    if max_y == 0 {
        return Ok(Found::NoColumn);
    }
    let (wx, wy, wz) = voxel_to_world(
        g,
        r.out,
        r.err,
        &mut tal.vg,
        f64::from(xcc),
        f64::from(ycc),
        f64::from(zv),
    )?;
    *seed = [wx, wy, wz];
    let _ = writeln!(r.out, "{xcc}, {ycc}, {zv}");
    Ok(if find_cc_slice(g, r, tal, seed, cc_tal)? == NO_ERROR {
        Found::Slice
    } else {
        Found::NoSlice
    })
}

/// A table entry of `find_cc_slice` as it reads it: FS-1207 for one never written (0).
fn entry(g: &mut Globals, t: &[Option<i32>], k: i32, what: &str) -> i32 {
    match usize::try_from(k).ok().and_then(|k| t.get(k)) {
        Some(Some(v)) => *v,
        _ => {
            g.notices.add(
                &FS_1207,
                format!(
                    "mri_cc -a 0: the midline's choice reads the {what} of slice {k}, never made (memory contents): 0 used"
                ),
            );
            0
        }
    }
}

/// `find_cc_slice(mri_tal, &x, &y, &z, lta, mri_cc_tal)`: the sagittal slices around the seed
/// (Talairach millimetres in `seed`; its voxel on return when a slice is chosen), each filled from
/// the seed and pasted into `cc_tal`; `NO_ERROR`, or `ERROR_BADPARM` when no slice has a
/// callosum's area (`seed` unchanged).
fn find_cc_slice(
    g: &mut Globals,
    r: &mut Run,
    tal: &mut Mri,
    seed: &mut [f64; 3],
    cc_tal: &mut Mri,
) -> Result<i32, Fatal> {
    let voxsize = find_min_size(tal);
    let slice_size = tal.vg.width;
    let max_slices = to_int(g, (21.0 / voxsize).ceil());
    let max_cc_area = to_int(g, (1400.0 / (voxsize * voxsize)).ceil());
    let min_cc_area = to_int(g, (350.0 / (voxsize * voxsize)).floor());
    let mut half_slices = to_int(g, (10.0 / voxsize).floor());
    if half_slices <= 0 {
        half_slices = 1;
    }
    if max_slices > SLICE_TABLE {
        g.notices.add(
            &FS_1206,
            format!(
                "mri_cc -a 0: voxels of {voxsize} mm give {max_slices} sagittal slices, past FreeSurfer's tables of {SLICE_TABLE}"
            ),
        );
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!(
                "{NAME}: the white-matter path takes voxels of 0.5 mm or more (the smallest here {voxsize} mm: {max_slices} slices, at most {SLICE_TABLE})"
            ),
        )?;
    }
    let n = usize::try_from(max_slices).unwrap_or(0);
    let mut area: Vec<Option<i32>> = vec![None; SLICE_TABLE as usize];
    let mut flag: Vec<Option<i32>> = vec![None; SLICE_TABLE as usize];
    let (x_tal, y_tal, z_tal) = (seed[0], seed[1], seed[2]);
    let mut edge = Edge::default();
    for slice in 0..n {
        let s = slice as i32;
        let offset = s.wrapping_sub(half_slices);
        let mut i = 0i32;
        let mut a;
        let (mut xv, mut yv, mut zv);
        let mut y;
        let mut plane;
        let mut filled;
        let mut region;
        loop {
            let (vx, vy, vz) = world_to_voxel(
                g,
                r.out,
                r.err,
                &mut tal.vg,
                x_tal + f64::from(offset),
                y_tal,
                z_tal,
            )?;
            y = vy;
            xv = nint(g, vx);
            yv = nint(g, vy).wrapping_sub(i);
            zv = nint(g, vz);
            plane = mri_extract_plane(g, r.out, r.err, tal, None, MRI_SAGITTAL, xv)?.unwrap_new();
            let (f, count) = mri_fill_fg(
                g,
                r.out,
                r.err,
                Pair::New(&plane),
                zv,
                yv,
                0,
                WM_MIN_VAL,
                CC_VAL,
            )?;
            filled = f.unwrap_new();
            a = count;
            region = mri_bounding_box(g, r.err, &filled, 1)?.1;
            let moved = i;
            i += 1;
            if moved != 0 {
                let _ = writeln!(r.out, "moved {moved} in slice {slice} ");
            }
            if !(a < 100 && i <= 5) {
                break;
            }
        }
        // slices that reach the border of the image are not trusted
        if region.x == 0
            || region.y == 0
            || region.x.wrapping_add(region.dx) >= slice_size.wrapping_sub(1)
            || region.y.wrapping_add(region.dy) >= slice_size.wrapping_sub(1)
        {
            a = 0;
        }
        let yi = nint(g, y);
        if !(a > 1100 && yi.wrapping_sub(region.y) > 11)
            || f64::from(region.dy) >= 3.5 * (y - f64::from(region.y))
        {
            filled = remove_fornix(g, r, &mut edge, &filled, yv, zv)?;
            a = 0;
            flag[slice] = Some(0);
            for ii in 0..filled.vg.width {
                for jj in 0..filled.vg.height {
                    if nv(g, r, &filled, ii, jj, 0)? > 0 {
                        a += 1;
                    }
                }
            }
        } else {
            flag[slice] = Some(1);
        }
        area[slice] = Some(a);
        if diag_verbose(g, DIAG_SHOW) {
            let _ = writeln!(r.out, "slice[{slice}] @ ({xv}, {yv}, {zv}): area = {a}");
        }
        if diag_verbose(g, DIAG_WRITE) {
            write_volume(g, r, &mut plane, &format!("cc_slice{slice}.mgz"))?;
            write_volume(g, r, &mut filled, &format!("cc_filled{slice}.mgz"))?;
        }
        mri_fill_plane(g, r.err, &filled, cc_tal, MRI_SAGITTAL, xv, CC_VAL)?;
    }

    let mut min_area = 10000 * 5;
    let mut min_slice = -1i32;
    for slice in 6..=14 {
        let mut total_area = 0i32;
        for i in -2..=2 {
            total_area = total_area.wrapping_add(entry(g, &area, slice + i, "area"));
        }
        if total_area < min_area
            && total_area >= min_cc_area.wrapping_mul(5)
            && total_area <= max_cc_area.wrapping_mul(5)
        {
            min_area = total_area;
            min_slice = slice;
        }
    }
    if min_slice < 0 {
        log::error_printf(
            g,
            r.err,
            ERROR_BADPARM,
            &format!("{NAME}: could not find valid seed for the cc"),
        );
        return Ok(ERROR_BADPARM);
    }
    let mut offset = min_slice.wrapping_sub(half_slices) / 2;
    let _ = writeln!(r.out, "find offset as {offset} using area");
    let (mut left, mut right) = (0i32, 0i32);
    let centre = half_slices.wrapping_add(offset);
    for slice in 0..max_slices.max(0) {
        let f = entry(g, &flag, slice, "fornix flag");
        if slice.wrapping_sub(centre) >= -6 && slice < centre && f == 1 {
            left += 1;
        } else if slice.wrapping_sub(centre) <= 6 && slice > centre && f == 1 {
            right += 1;
        }
    }
    offset = offset.wrapping_add(left).wrapping_sub(right);
    let _ = writeln!(r.out, "find offset as {} using shifting", left - right);
    if offset.wrapping_abs() >= 5 {
        offset = 5i32.wrapping_mul(offset / offset.wrapping_abs());
    }
    let x = x_tal + f64::from(offset);
    let (y, z) = (y_tal, z_tal);
    let (xvv, yvv, zvv) = world_to_voxel(g, r.out, r.err, &mut tal.vg, x, y, z)?;
    *seed = [xvv, yvv, zvv];
    let f2 = |v: f64| fmt_f(v, 0, 2);
    let _ = writeln!(
        r.out,
        "updating initial cc seed to Tal vol ({}, {}, {}) TAL ({}, {}, {})",
        f2(xvv),
        f2(yvv),
        f2(zvv),
        f2(x),
        f2(y),
        f2(z)
    );
    Ok(NO_ERROR)
}

/// `x_edge` and `y_edge`, the file-scope positions `remove_fornix` and `edge_detection` share.
#[derive(Default)]
struct Edge {
    x: i32,
    y: i32,
}

/// FS-1209: an edge search whose loop FreeSurfer never leaves (no progress past voxel 256 on a
/// slice wider or taller than its limit): an error in both modes (rule B).
fn never_ends(g: &mut Globals, r: &mut Run, m: &Mri, axis: &str) -> Result<(), Fatal> {
    g.notices.add(
        &FS_1209,
        format!(
            "mri_cc -a 0: the fornix cut's search along {axis} stops at voxel 256 on a sagittal slice of {} x {} voxels: FreeSurfer never ends",
            m.vg.width, m.vg.height
        ),
    );
    log::error_exit(
        g,
        r.err,
        1,
        &format!(
            "{NAME}: the white-matter path's fornix cut does not end on a sagittal slice of {} x {} voxels (its searches stop at voxel 256)",
            m.vg.width, m.vg.height
        ),
    )
}

/// `edge_detection(mri_temp, edge_count, signal)`: along the row `y_edge` from `x_edge` (`signal`
/// 1: the run of nonzero voxels, gaps of up to two zero voxels joined) or along the column
/// `x_edge` from `y_edge` (`signal` 0: the run of nonzero voxels), up to voxel 256; the run's
/// length, the position left after it.
fn edge_detection(
    g: &mut Globals,
    r: &mut Run,
    e: &mut Edge,
    m: &Mri,
    signal: i32,
) -> Result<i32, Fatal> {
    let mut length = 0i32;
    let mut gap = 0i32;
    if signal == 1 {
        while gap <= 2 {
            gap = 0;
            while e.x < 256 {
                if nv(g, r, m, e.x, e.y, 0)? != 0 {
                    break;
                }
                e.x += 1;
            }
            while e.x < 256 {
                if nv(g, r, m, e.x, e.y, 0)? == 0 {
                    break;
                }
                length += 1;
                e.x += 1;
            }
            while e.x < 256 {
                // `if (MRIgetVoxVal(...))`: any value but 0 (a NaN too)
                if mri_get_vox_val(g, r.err, m, e.x, e.y, 0, 0, 0)? != 0.0 {
                    break;
                }
                gap += 1;
                e.x += 1;
            }
            if gap <= 2 && e.x < 256 {
                length += gap;
            } else {
                e.x -= gap;
                break;
            }
        }
    } else {
        while e.y < 256 {
            if nv(g, r, m, e.x, e.y, 0)? != 0 {
                break;
            }
            e.y += 1;
        }
        while e.y < 256 {
            if nv(g, r, m, e.x, e.y, 0)? == 0 {
                break;
            }
            length += 1;
            e.y += 1;
        }
    }
    Ok(length)
}

/// `remove_fornix(mri_filled, xv, yv, zv)`: a sagittal slice's fill closed (dilated, then eroded,
/// by the 4-neighbourhood), cut to 40 rows below its top when taller than 50, and the fornix cut
/// off along the line between two points the edge searches find. A new slice.
fn remove_fornix(
    g: &mut Globals,
    r: &mut Run,
    e: &mut Edge,
    src: &Mri,
    yv: i32,
    zv: i32,
) -> Result<Mri, Fatal> {
    let (height, width) = (src.vg.height, src.vg.width);
    let mut t1 = mri_copy(g, r.out, r.err, src, None)?.unwrap_new();
    let mut t2 = mri_copy(g, r.out, r.err, src, None)?.unwrap_new();
    mri_value_fill(g, r.err, &mut t1, 0.0)?;
    mri_value_fill(g, r.err, &mut t2, 0.0)?;
    for x in 1..width - 1 {
        for y in 1..height - 1 {
            if nv(g, r, src, x - 1, y, 0)? != 0
                || nv(g, r, src, x, y - 1, 0)? != 0
                || nv(g, r, src, x, y + 1, 0)? != 0
                || nv(g, r, src, x + 1, y, 0)? != 0
                || nv(g, r, src, x, y, 0)? != 0
            {
                set(g, r, &mut t1, x, y, 100.0)?;
            }
        }
    }
    for x in 1..width - 1 {
        for y in 1..height - 1 {
            if nv(g, r, &t1, x - 1, y, 0)? != 0
                && nv(g, r, &t1, x, y - 1, 0)? != 0
                && nv(g, r, &t1, x, y + 1, 0)? != 0
                && nv(g, r, &t1, x + 1, y, 0)? != 0
                && nv(g, r, &t1, x, y, 0)? != 0
            {
                set(g, r, &mut t2, x, y, 100.0)?;
            }
        }
    }
    let (mut xi_low, mut xi_high, mut yi_low, mut yi_high) = (255i32, 0i32, 255i32, 0i32);
    for x in 0..width {
        for y in 0..height {
            if nv(g, r, &t2, x, y, 0)? != 0 {
                xi_low = xi_low.min(x);
                yi_low = yi_low.min(y);
                xi_high = xi_high.max(x);
                yi_high = yi_high.max(y);
            }
        }
    }
    if yi_high > yi_low + 50 {
        yi_high = yi_low + 40;
        for x in 0..width {
            for y in yi_high..height {
                set(g, r, &mut t2, x, y, 0.0)?;
                set(g, r, &mut t1, x, y, 0.0)?;
            }
        }
    }
    let mut filled = mri_copy(g, r.out, r.err, &t2, None)?.unwrap_new();

    // the first edge of the spike
    let (mut x1, mut y1, mut x2, mut y2) = (0i32, 0i32, 0i32, 0i32);
    let (mut temp, mut temp2, mut old_temp) = (0i32, 0i32, 0i32);
    let mut flag = 0i32;
    e.x = xi_high - (xi_high - xi_low) / 4;
    while e.x >= xi_low + (xi_high - xi_low) / 7 {
        let mut edge_count = 0i32;
        e.y = 0;
        while edge_count < 3 && e.y < height - 2 {
            let length = edge_detection(g, r, e, &t2, 0)?;
            if length == 0 && e.y >= 256 && e.y < height - 2 {
                never_ends(g, r, &t2, "a column")?;
            }
            if length > 1 {
                edge_count += 1;
            }
            if edge_count == 1 && length > 1 {
                if length >= 20 && e.x > xi_low + 15 && e.x < zv.wrapping_add(20) {
                    temp = old_temp;
                    if x1 < e.x {
                        y1 = old_temp;
                        x1 = e.x;
                        for y in temp..256 {
                            set(g, r, &mut filled, e.x, y, 0.0)?;
                        }
                    }
                    flag = 2;
                } else {
                    temp = e.y;
                }
                if length <= 1 || e.y < yv.wrapping_sub(3) {
                    edge_count -= 1;
                } else if length > 2 && e.x > xi_low + 15 && e.x < zv.wrapping_add(17) {
                    for y in e.y + 1..256 {
                        set(g, r, &mut filled, e.x, y, 0.0)?;
                    }
                }
            } else if length > 1 && edge_count > 1 && e.y < yi_high + 1 && e.y > yv.wrapping_add(15)
            {
                edge_count -= 1;
            }
        }
        if edge_count >= 2 && flag == 0 {
            flag = 1;
        } else if edge_count <= 1 && flag == 1 && e.x > xi_low + 13 {
            flag = 0;
            y1 = old_temp;
            x1 = e.x;
        }
        old_temp = temp;
        e.x -= 1;
    }
    e.x = 0;
    e.y = 0;

    // the second edge of the spike
    flag = 0;
    let mut section1 = [0i32; SECTIONS];
    e.y = yv.wrapping_add(20);
    while e.y >= yv {
        let mut edge_count = 0i32;
        e.x = 0;
        let i = yv.wrapping_add(20).wrapping_sub(e.y) as usize;
        section1[i] = 0;
        while e.x < width - 1 {
            let length = edge_detection(g, r, e, &t2, 1)?;
            if length == 0 && e.x >= 256 && e.x < width - 1 {
                never_ends(g, r, &t2, "a row")?;
            }
            if length >= 2 {
                edge_count += 1;
            }
            if edge_count == 1 {
                temp = e.x;
                if section1[i] == 0 {
                    section1[i] = length;
                }
            }
            if edge_count == 2 {
                temp2 = e.x - length;
            }
        }
        if edge_count >= 3 && flag == 0 {
            flag = 1;
        } else if edge_count <= 2 && flag == 1 {
            flag = 0;
            x2 = old_temp;
            y2 = e.y;
            if y2 <= yi_low + 20 {
                break;
            }
        } else if x2 == 0 && (section1[i] >= 19 || previous_row(g, &section1, i)) {
            x2 = old_temp;
            y2 = e.y;
            if y2 <= yi_low + 22 {
                break;
            }
        }
        if edge_count >= 4 && temp2 < x1 {
            old_temp = temp2 - 1;
        } else {
            old_temp = temp;
        }
        e.y -= 1;
    }

    if x2 > 0 && x1 > xi_low + 8 && x1 > x2 && x1 < zv.wrapping_add(5) {
        if x2 < x1 {
            std::mem::swap(&mut x1, &mut x2);
            std::mem::swap(&mut y1, &mut y2);
        }
        for x in x1..=x2 {
            let start = y1 + (y2 - y1) * (x - x1) / (x2 - x1) + 1;
            for y in start..height {
                set(g, r, &mut filled, x, y, 0.0)?;
            }
        }
    }
    if diag_verbose(g, DIAG_WRITE) {
        write_volume(
            g,
            r,
            &mut filled,
            "/space/neo/2/recon/buckner/010601_vc6977/mri/cc_cut.mgz",
        )?;
    }
    Ok(filled)
}

/// `4 * section1[i - 1] < 3 * section1[i]`: the row's section a quarter longer than the row's
/// before it. FS-1208: at the first row (`i` 0) FreeSurfer reads the table's element -1.
fn previous_row(g: &mut Globals, section1: &[i32; SECTIONS], i: usize) -> bool {
    if i == 0 {
        g.notices.add(
            &FS_1208,
            "mri_cc -a 0: the fornix cut's first row is compared with the row before it, which does not exist (memory below the table): taken as 0".to_owned(),
        );
        return 0 < 3 * section1[0];
    }
    4 * section1[i - 1] < 3 * section1[i]
}
