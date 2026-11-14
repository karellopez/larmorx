// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/volcluster.cpp`'s volume clusters (queue items 3.1-43, lines 40-165, and 3.1-44, lines
//! 181-925): `clustAllocCluster` (40), `clustAllocClusterList` (60), `clustFreeCluster` (72),
//! `clustFreeClusterList` (110), `clustValueInRange` (165), `clustInitHitMap` (181),
//! `clustAddMember` (297), `clustGrowOneVoxel` (357), `clustGrow` (398), `clustMaxMember` (437),
//! `clustPruneBySize` (458), `clustPruneByDistance` (486), `clustCopyCluster` (580),
//! `clustCopyClusterList` (609), `clustCompareCluster` (623), `clustSortClusterList` (646),
//! `clustComputeXYZ` (667), `clustComputeTal` (680), `clustClusterList2Vol` (693),
//! `clustGetClusters` (778), `ConvertCRS2XYZ` (925), and the island and hole removal
//! `MRIremoveVolumeIslands` (2056) and `MRIremoveVolumeHoles` (2114) built on them.
//!
//! A cluster's members are kept in vectors whose length is `nmembers`; allocation, freeing and
//! FreeSurfer's realloc failures (never on a machine that can run its programs) have no
//! counterpart. FreeSurfer's coordinates of a member added by `clustAddMember` stay as `realloc`
//! left them until `clustComputeXYZ` or `clustComputeTal` sets them; here they are 0 and marked
//! unset (FS-570 where they are read). `VolClustFixMNI` is 0 in every program that reaches this
//! code (only `mri_volcluster` sets it), so `clustComputeTal` is `clustComputeXYZ`.
//! FS-570, FS-571.

use std::io::Write;

use larmorx_core::defects::{FS_570, FS_571};
use larmorx_core::glibc_qsort::qsort;
use larmorx_freesurfer_core::cstd::format_double;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{
    MRI_INT, Mri, mri_alloc, mri_alloc_sequence, mri_copy_header, mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::Pair;
use larmorx_freesurfer_volume::combine::mri_dim_mismatch;

/// `VOLCLUSTER`: the members' column, row and slice, their coordinates (`set`: whether each was
/// computed), the member of the largest value and that value, the voxel size and the clusterwise
/// p-values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VolCluster {
    pub col: Vec<i32>,
    pub row: Vec<i32>,
    pub slc: Vec<i32>,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub z: Vec<f32>,
    /// Whether each member's `x`, `y`, `z` were computed (FreeSurfer's are memory contents
    /// until then).
    pub set: Vec<bool>,
    pub maxmember: i32,
    pub maxval: f32,
    pub voxsize: f32,
    pub pval_clusterwise: f64,
    pub pval_clusterwise_low: f64,
    pub pval_clusterwise_hi: f64,
}

impl VolCluster {
    /// `nmembers`.
    pub fn nmembers(&self) -> i32 {
        self.col.len() as i32
    }
}

fn gv(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
) -> Result<f32, Fatal> {
    mri_get_vox_val(g, err, m, c, r, s, f, 0)
}

#[allow(clippy::too_many_arguments)]
fn sv(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &mut Mri,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
    v: f32,
) -> Result<(), Fatal> {
    mri_set_vox_val(g, err, m, c, r, s, f, v, 0)?;
    Ok(())
}

/// `clustAllocCluster(nmembers)`: a cluster of `nmembers` members at (0, 0, 0) with coordinates
/// 0 (`calloc`'s zeros: set).
pub fn clust_alloc_cluster(nmembers: i32) -> VolCluster {
    let n = nmembers.max(0) as usize;
    VolCluster {
        col: vec![0; n],
        row: vec![0; n],
        slc: vec![0; n],
        x: vec![0.0; n],
        y: vec![0.0; n],
        z: vec![0.0; n],
        set: vec![true; n],
        ..VolCluster::default()
    }
}

/// `clustValueInRange(val, thmin, thmax, thsign)`: the value (its magnitude for `thsign` 0, its
/// negative for -1) at or above `thmin` and, when `thmax` is above 0, at or below `thmax`.
pub fn clust_value_in_range(val: f32, thmin: f32, thmax: f32, thsign: i32) -> bool {
    let mut val = val;
    if thsign == 0 {
        val = val.abs();
    }
    if thsign == -1 {
        val = -val;
    }
    if thmax > 0.0 {
        val >= thmin && val <= thmax
    } else {
        val >= thmin
    }
}

/// A mask value as `int maskval = MRIgetVoxVal(...)` sees it: 0 when it truncates to 0 (x86-64
/// gives `INT_MIN`, not 0, for NaN and values outside the `int` range).
fn mask_zero(v: f32) -> bool {
    v > -1.0 && v < 1.0
}

/// The hits of [`clust_init_hit_map`]: the map and the hits' column, row and slice in order.
pub struct HitMap {
    pub map: Mri,
    pub hits: Vec<[i32; 3]>,
}

/// `clustInitHitMap(vol, frame, thmin, thmax, thsign, ..., binmask, maskframe)`: an int map of the
/// volume's extent (no header) in which the voxels of `frame` in range ([`clust_value_in_range`])
/// and inside the mask (its `maskframe` value not truncating to 0) are 0 and every other voxel 1,
/// and those voxels' places (columns outermost, then rows, then slices). Without a hit the map is
/// all 0. With `Gdiag_no` above 0 the parameters (`clustInitHitMap: `, `   thmin  = <%f>`, `
/// thmax  = <%f>`, `   thsign = <d>`, `   maskframe = <d>`) and `no hits in hit map`; above 1
/// `INFO: clustInitHitMap: found <n> hits`.
#[allow(clippy::too_many_arguments)]
pub fn clust_init_hit_map(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vol: &Mri,
    frame: i32,
    thmin: f32,
    thmax: f32,
    thsign: i32,
    binmask: Option<&Mri>,
    maskframe: i32,
) -> Result<HitMap, Fatal> {
    if g.gdiag_no > 0 {
        let f = |x: f32| format_double("%f", f64::from(x)).unwrap_or_default();
        let _ = write!(
            out,
            "clustInitHitMap: \n   thmin  = {}\n   thmax  = {}\n   thsign = {thsign}\n   maskframe = {maskframe}\n",
            f(thmin),
            f(thmax)
        );
    }
    let (w, h, d) = (vol.vg.width, vol.vg.height, vol.vg.depth);
    let mut map = mri_alloc(g, out, err, w, h, d, MRI_INT)?;
    let mut nh = 0usize;
    for col in 0..w {
        for row in 0..h {
            for slc in 0..d {
                if let Some(m) = binmask
                    && mask_zero(gv(g, err, m, col, row, slc, maskframe)?)
                {
                    continue;
                }
                let val = gv(g, err, vol, col, row, slc, frame)?;
                if clust_value_in_range(val, thmin, thmax, thsign) {
                    nh += 1;
                }
            }
        }
    }
    if nh == 0 {
        if g.gdiag_no > 0 {
            let _ = writeln!(out, "no hits in hit map");
        }
        return Ok(HitMap {
            map,
            hits: Vec::new(),
        });
    }
    let mut hits = Vec::with_capacity(nh);
    for col in 0..w {
        for row in 0..h {
            for slc in 0..d {
                let val = gv(g, err, vol, col, row, slc, frame)?;
                if let Some(m) = binmask
                    && mask_zero(gv(g, err, m, col, row, slc, maskframe)?)
                {
                    sv(g, err, &mut map, col, row, slc, 0, 1.0)?;
                    continue;
                }
                if clust_value_in_range(val, thmin, thmax, thsign) {
                    hits.push([col, row, slc]);
                    sv(g, err, &mut map, col, row, slc, 0, 0.0)?;
                } else {
                    sv(g, err, &mut map, col, row, slc, 0, 1.0)?;
                }
            }
        }
    }
    if g.gdiag_no > 1 {
        let _ = writeln!(out, "INFO: clustInitHitMap: found {} hits", hits.len());
    }
    Ok(HitMap { map, hits })
}

/// `clustAddMember(vc, col, row, slc)`: the member appended (its coordinates unset). 0.
pub fn clust_add_member(vc: &mut VolCluster, col: i32, row: i32, slc: i32) -> i32 {
    vc.col.push(col);
    vc.row.push(row);
    vc.slc.push(slc);
    vc.x.push(0.0);
    vc.y.push(0.0);
    vc.z.push(0.0);
    vc.set.push(false);
    0
}

/// `clustGrowOneVoxel(vc, col0, row0, slc0, HitMap, AllowDiag)`: every neighbour inside the map
/// (columns, then rows, then slices from -1 to +1; faces only unless `AllowDiag`) whose map value is
/// 0 is added to the cluster and marked 1. The number added.
#[allow(clippy::too_many_arguments)]
pub fn clust_grow_one_voxel(
    g: &mut Globals,
    err: &mut dyn Write,
    vc: &mut VolCluster,
    col0: i32,
    row0: i32,
    slc0: i32,
    map: &mut Mri,
    allow_diag: bool,
) -> Result<i32, Fatal> {
    let mut nadded = 0;
    for dcol in -1..=1 {
        for drow in -1..=1 {
            for dslc in -1..=1 {
                let col = col0.wrapping_add(dcol);
                if col < 0 || col >= map.vg.width {
                    continue;
                }
                let row = row0.wrapping_add(drow);
                if row < 0 || row >= map.vg.height {
                    continue;
                }
                let slc = slc0.wrapping_add(dslc);
                if slc < 0 || slc >= map.vg.depth {
                    continue;
                }
                if !allow_diag && dcol.abs() + drow.abs() + dslc.abs() != 1 {
                    continue;
                }
                if gv(g, err, map, col, row, slc, 0)? != 0.0 {
                    continue;
                }
                clust_add_member(vc, col, row, slc);
                sv(g, err, map, col, row, slc, 0, 1.0)?;
                nadded += 1;
            }
        }
    }
    Ok(nadded)
}

/// `clustGrow(col0, row0, slc0, HitMap, AllowDiag, npassesmax)`: a cluster seeded at the voxel
/// (marked 1), its voxel size the map's (`xsize * ysize * zsize` in `float`), grown pass after pass
/// by [`clust_grow_one_voxel`] from each member added by the previous pass until a pass adds none,
/// or after `npassesmax` passes when that is above 0.
#[allow(clippy::too_many_arguments)]
pub fn clust_grow(
    g: &mut Globals,
    err: &mut dyn Write,
    col0: i32,
    row0: i32,
    slc0: i32,
    map: &mut Mri,
    allow_diag: bool,
    npassesmax: i32,
) -> Result<VolCluster, Fatal> {
    let mut vc = VolCluster::default();
    clust_add_member(&mut vc, col0, row0, slc0);
    sv(g, err, map, col0, row0, slc0, 0, 1.0)?;
    vc.voxsize = map.vg.xsize * map.vg.ysize * map.vg.zsize;
    let mut nthmember = 0usize;
    let mut nthpass = 0i32;
    let mut nadded = 1;
    while nadded > 0 {
        nadded = 0;
        let now = vc.col.len();
        while nthmember < now {
            let (c, r, s) = (vc.col[nthmember], vc.row[nthmember], vc.slc[nthmember]);
            nadded += clust_grow_one_voxel(g, err, &mut vc, c, r, s, map, allow_diag)?;
            nthmember += 1;
        }
        nthpass += 1;
        if npassesmax > 0 && nthpass > npassesmax {
            break;
        }
    }
    Ok(vc)
}

/// `clustMaxMember(vc, vol, frame, thsign)`: `maxval` from 0; each member's value `v` of `frame`
/// is compared as itself (`thsign` 1), its magnitude (0) or its negative (-1) (another `thsign`
/// keeps the last compared value, from 0): when `|maxval|` is below it, `maxval = v` and
/// `maxmember` the member (otherwise `maxmember` stays). 0.
pub fn clust_max_member(
    g: &mut Globals,
    err: &mut dyn Write,
    vc: &mut VolCluster,
    vol: &Mri,
    frame: i32,
    thsign: i32,
) -> Result<i32, Fatal> {
    let mut val = 0.0f32;
    vc.maxval = 0.0;
    for n in 0..vc.col.len() {
        let val0 = gv(g, err, vol, vc.col[n], vc.row[n], vc.slc[n], frame)?;
        match thsign {
            1 => val = val0,
            0 => val = val0.abs(),
            -1 => val = -val0,
            _ => {}
        }
        if vc.maxval.abs() < val {
            vc.maxval = val0;
            vc.maxmember = n as i32;
        }
    }
    Ok(0)
}

/// `clustCopyCluster(vc)`: a copy of the members and coordinates, `maxmember`, `maxval`,
/// `voxsize` and the p-values.
pub fn clust_copy_cluster(vc: &VolCluster) -> VolCluster {
    vc.clone()
}

/// `clustPruneBySize(vclist, nlist, voxsize, sizethresh)`: copies of the clusters whose
/// `nmembers * voxsize` (in `float`) is at or above `sizethresh`, in order.
pub fn clust_prune_by_size(list: &[VolCluster], voxsize: f32, sizethresh: f32) -> Vec<VolCluster> {
    list.iter()
        .filter(|vc| vc.nmembers() as f32 * voxsize >= sizethresh)
        .map(clust_copy_cluster)
        .collect()
}

/// A member's coordinates as FreeSurfer reads them; FS-570 where they were never computed (0).
fn coords(g: &mut Globals, what: &str, vc: &VolCluster, n: i32) -> [f32; 3] {
    let i = n as usize;
    if n < 0 || i >= vc.col.len() || !vc.set[i] {
        g.notices.add(
            &FS_570,
            format!(
                "{what}: member {n}'s coordinates were never computed (memory contents); taken as 0"
            ),
        );
        if n < 0 || i >= vc.col.len() {
            return [0.0; 3];
        }
    }
    [vc.x[i], vc.y[i], vc.z[i]]
}

/// `clustPruneByDistance(vclist, nlist, distthresh)`: copies of the clusters kept, in order: a
/// cluster is dropped when another's largest member lies closer than `distthresh` to its own
/// (the distance in `float`, `sqrtf` of the squared differences of x, y and z added in that order)
/// and has the larger `maxval`. `None` when none is kept.
pub fn clust_prune_by_distance(
    g: &mut Globals,
    list: &[VolCluster],
    distthresh: f32,
) -> Option<Vec<VolCluster>> {
    const WHAT: &str = "clustPruneByDistance";
    let mut keep_list = Vec::new();
    for (n1, a) in list.iter().enumerate() {
        let [x1, y1, z1] = coords(g, WHAT, a, a.maxmember);
        let max1 = a.maxval;
        let mut keep = true;
        for (n2, b) in list.iter().enumerate() {
            if n1 == n2 {
                continue;
            }
            let [x2, y2, z2] = coords(g, WHAT, b, b.maxmember);
            let max2 = b.maxval;
            let d = ((x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2) + (z1 - z2) * (z1 - z2)).sqrt();
            if d < distthresh && max1 < max2 {
                keep = false;
                break;
            }
        }
        if keep {
            keep_list.push(clust_copy_cluster(a));
        }
    }
    if keep_list.is_empty() {
        None
    } else {
        Some(keep_list)
    }
}

/// `clustCompareCluster`: the smaller clusterwise p-value first, then the more members, then the
/// larger `|maxval|`; 0 when none decides (NaNs decide nothing).
pub fn clust_compare_cluster(a: &VolCluster, b: &VolCluster) -> i32 {
    if a.pval_clusterwise < b.pval_clusterwise {
        return -1;
    }
    if a.pval_clusterwise > b.pval_clusterwise {
        return 1;
    }
    if a.nmembers() > b.nmembers() {
        return -1;
    }
    if a.nmembers() < b.nmembers() {
        return 1;
    }
    if a.maxval.abs() > b.maxval.abs() {
        return -1;
    }
    if a.maxval.abs() < b.maxval.abs() {
        return 1;
    }
    0
}

/// `clustSortClusterList(vclist, nlist, NULL)`: copies sorted by [`clust_compare_cluster`] with C's
/// `qsort` (glibc's merge order: ties keep their order).
pub fn clust_sort_cluster_list(list: &[VolCluster]) -> Vec<VolCluster> {
    let mut idx: Vec<usize> = (0..list.len()).collect();
    qsort(&mut idx, |&a, &b| clust_compare_cluster(&list[a], &list[b]));
    idx.into_iter()
        .map(|i| clust_copy_cluster(&list[i]))
        .collect()
}

/// `ConvertCRS2XYZ(col, row, slc, CRS2XYZ)`: the first three elements of `CRS2XYZ * [col, row,
/// slc, 1]'` (`MatrixMultiply`, in `float`). FS-571: a transform that is not 4 x 4 (FreeSurfer
/// crashes after `MatrixMultiply`'s message; an error).
#[allow(clippy::too_many_arguments)]
fn convert_crs2xyz(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    col: i32,
    row: i32,
    slc: i32,
    crs2xyz: &Matrix,
) -> Result<[f32; 3], Fatal> {
    let crs = Matrix::from_rows(&[
        vec![col as f32],
        vec![row as f32],
        vec![slc as f32],
        vec![1.0],
    ]);
    let r = matrix_multiply(g, out, err, Some(crs2xyz), Some(&crs), None)?;
    let Ret::New(xyz) = r else {
        g.notices.add(
            &FS_571,
            format!(
                "ConvertCRS2XYZ: a {} x {} transform (FreeSurfer reads a NULL product)",
                crs2xyz.rows, crs2xyz.cols
            ),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "ConvertCRS2XYZ(): the transform is not 4 x 4 (FreeSurfer crashes here)\n",
        )?;
        return Ok([0.0; 3]);
    };
    if xyz.rows < 3 {
        g.notices.add(
            &FS_571,
            format!(
                "ConvertCRS2XYZ: a {} x {} transform",
                crs2xyz.rows, crs2xyz.cols
            ),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "ConvertCRS2XYZ(): the transform has fewer than 3 rows (FreeSurfer reads outside it here)\n",
        )?;
        return Ok([0.0; 3]);
    }
    Ok([xyz.relt(1, 1), xyz.relt(2, 1), xyz.relt(3, 1)])
}

/// `clustComputeXYZ(vc, CRS2XYZ)`, and `clustComputeTal` (with `VolClustFixMNI` 0): each member's
/// coordinates from [`convert_crs2xyz`]. 0.
pub fn clust_compute_xyz(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vc: &mut VolCluster,
    crs2xyz: &Matrix,
) -> Result<i32, Fatal> {
    for n in 0..vc.col.len() {
        let [x, y, z] = convert_crs2xyz(g, out, err, vc.col[n], vc.row[n], vc.slc[n], crs2xyz)?;
        vc.x[n] = x;
        vc.y[n] = y;
        vc.z[n] = z;
        vc.set[n] = true;
    }
    Ok(0)
}

/// `clustClusterList2Vol(vclist, nlist, tvol, frame, ValOption)`: a volume of `tvol`'s extent,
/// type and header, with `tvol`'s frames when `ValOption` is not 0 (else one), zeros but at the
/// clusters' members: with `ValOption` 1 their values of every frame of `tvol`, otherwise the
/// cluster's number from 1 in frame 0.
pub fn clust_cluster_list2vol(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    list: &[VolCluster],
    tvol: &Mri,
    val_option: i32,
) -> Result<Mri, Fatal> {
    let nf = if val_option != 0 { tvol.nframes } else { 1 };
    let (w, h, d) = (tvol.vg.width, tvol.vg.height, tvol.vg.depth);
    let mut vol = mri_alloc_sequence(g, out, err, w, h, d, tvol.mtype, nf)?;
    mri_copy_header(g, out, err, tvol, Some(&mut vol))?;
    for (nthvc, vc) in list.iter().enumerate() {
        for n in 0..vc.col.len() {
            let (c, r, s) = (vc.col[n], vc.row[n], vc.slc[n]);
            if val_option == 1 {
                for f in 0..tvol.nframes {
                    let v = gv(g, err, tvol, c, r, s, f)?;
                    sv(g, err, &mut vol, c, r, s, f, v)?;
                }
            } else {
                sv(g, err, &mut vol, c, r, s, 0, (nthvc as i32 + 1) as f32)?;
            }
        }
    }
    Ok(vol)
}

/// `clustGetClusters(vol, frame, threshmin, threshmax, threshsign, minclustsizemm3, binmask,
/// &nClusters, XFM, allowdiag)`: the hit map ([`clust_init_hit_map`], mask frame 0); each hit not
/// yet in a cluster seeds one ([`clust_grow`] without a pass limit), whose voxel size is the
/// volume's (`xsize * ysize * zsize` in `float`), whose largest member is found
/// ([`clust_max_member`]) and, with `XFM`, whose coordinates are computed; the clusters of at
/// least `minclustsizemm3` ([`clust_prune_by_size`]), sorted ([`clust_sort_cluster_list`]). With
/// `Gdiag_no` above 0 FreeSurfer's progress lines (`INFO: Found <n> voxels in threhold range`, `...
/// clusters that meet threshold criteria`, `... clusters that meet size criteria`, `... final
/// clusters`).
#[allow(clippy::too_many_arguments)]
pub fn clust_get_clusters(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vol: &Mri,
    frame: i32,
    threshmin: f32,
    threshmax: f32,
    threshsign: i32,
    minclustsizemm3: f32,
    binmask: Option<&Mri>,
    xfm: Option<&Matrix>,
    allowdiag: bool,
) -> Result<Vec<VolCluster>, Fatal> {
    let voxsizemm3 = vol.vg.xsize * vol.vg.ysize * vol.vg.zsize;
    let HitMap { mut map, hits } = clust_init_hit_map(
        g, out, err, vol, frame, threshmin, threshmax, threshsign, binmask, 0,
    )?;
    if g.gdiag_no > 0 {
        let _ = writeln!(out, "INFO: Found {} voxels in threhold range", hits.len());
    }
    let mut list = Vec::new();
    for &[col, row, slc] in &hits {
        if gv(g, err, &map, col, row, slc, 0)? != 0.0 {
            continue;
        }
        let mut vc = clust_grow(g, err, col, row, slc, &mut map, allowdiag, -1)?;
        vc.voxsize = voxsizemm3;
        clust_max_member(g, err, &mut vc, vol, frame, threshsign)?;
        if let Some(x) = xfm {
            clust_compute_xyz(g, out, err, &mut vc, x)?;
        }
        list.push(vc);
    }
    if g.gdiag_no > 0 {
        let _ = writeln!(
            out,
            "INFO: Found {} clusters that meet threshold criteria",
            list.len()
        );
    }
    let list = clust_prune_by_size(&list, voxsizemm3, minclustsizemm3);
    if g.gdiag_no > 0 {
        let _ = writeln!(
            out,
            "INFO: Found {} clusters that meet size criteria",
            list.len()
        );
    }
    let list = clust_sort_cluster_list(&list);
    if g.gdiag_no > 0 {
        let _ = writeln!(out, "INFO: Found {} final clusters", list.len());
    }
    Ok(list)
}

/// The destination of the island and hole removal: a new one of the mask's type (one frame,
/// header and pulse parameters), the caller's, or the mask itself; `None` after `ERROR: <what>():
/// dimension mismatch` for one of another extent (`MRIdimMismatch` without frames).
fn removal_dst<'a>(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    what: &str,
    p: Pair<'a>,
    fresh: &'a mut Option<Mri>,
) -> Result<Option<(&'a Mri, Option<&'a mut Mri>)>, Fatal> {
    // the mask, and the destination when it is not the mask itself
    Ok(match p {
        Pair::New(mask) => {
            let (w, h, d) = (mask.vg.width, mask.vg.height, mask.vg.depth);
            let mut o = mri_alloc_sequence(g, out, err, w, h, d, mask.mtype, 1)?;
            mri_copy_header(g, out, err, mask, Some(&mut o))?;
            mri_copy_pulse_parameters(mask, &mut o);
            Some((mask, Some(fresh.insert(o))))
        }
        Pair::Given(mask, o) => {
            if mri_dim_mismatch(mask, o, false) != 0 {
                let _ = writeln!(out, "ERROR: {what}(): dimension mismatch");
                return Ok(None);
            }
            Some((mask, Some(o)))
        }
        Pair::Same(_) => unreachable!("handled by the caller"),
    })
}

/// `MRIremoveVolumeIslands(mask, thresh, nKeep, outvol)`: the clusters of the voxels whose
/// magnitude is at or above `thresh` (as a `float`; [`clust_get_clusters`], faces only, numbered
/// from the largest); prints `MRIremoveVolumeIslands() thresh=<%g>, nKeep=<d>, nclusters = <n>`;
/// each voxel whose cluster number (in a volume of the mask's type: clamped as that type stores
/// it, 0 outside the clusters) is at most `nKeep` keeps the mask's value, every other voxel 0. A
/// new output is of the mask's type, one frame; the mask itself may be the output. `ERROR:
/// MRIremoveVolumeIslands(): nKeep==0` and `Null` for `nKeep` 0.
pub fn mri_remove_volume_islands(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    thresh: f64,
    nkeep: i32,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIremoveVolumeIslands";
    if nkeep == 0 {
        let _ = writeln!(out, "ERROR: MRIremoveVolumeIslands(): nKeep==0");
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    if let Pair::Same(m) = p {
        let copy = mri_copy(g, out, err, m, None)?.unwrap_new();
        islands_into(g, out, err, &copy, m, thresh, nkeep)?;
        return Ok(Ret::Given);
    }
    let Some((mask, Some(o))) = removal_dst(g, out, err, WHAT, p, &mut fresh)? else {
        return Ok(Ret::Null);
    };
    islands_into(g, out, err, mask, o, thresh, nkeep)?;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIremoveVolumeIslands`' work into `o` (the mask read before each voxel is written).
#[allow(clippy::too_many_arguments)]
fn islands_into(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mask: &Mri,
    o: &mut Mri,
    thresh: f64,
    nkeep: i32,
) -> Result<(), Fatal> {
    let list = clust_get_clusters(
        g,
        out,
        err,
        mask,
        0,
        thresh as f32,
        0.0,
        0,
        0.0,
        None,
        None,
        false,
    )?;
    let ocn = clust_cluster_list2vol(g, out, err, &list, mask, 0)?;
    let _ = writeln!(
        out,
        "MRIremoveVolumeIslands() thresh={}, nKeep={nkeep}, nclusters = {}",
        format_double("%g", thresh).unwrap_or_default(),
        list.len()
    );
    for c in 0..mask.vg.width {
        for r in 0..mask.vg.height {
            for s in 0..mask.vg.depth {
                let v = f64::from(gv(g, err, &ocn, c, r, s, 0)?);
                if v <= f64::from(nkeep) {
                    let v0 = f64::from(gv(g, err, mask, c, r, s, 0)?);
                    sv(g, err, o, c, r, s, 0, v0 as f32)?;
                } else {
                    sv(g, err, o, c, r, s, 0, 0.0)?;
                }
            }
        }
    }
    Ok(())
}

/// `MRIremoveVolumeHoles(mask, thresh, nKeep, FillVal, outvol)`: prints `MRIremoveVolumeHoles()
/// thresh=<%g>, nKeep=<d>`; the voxels at or below `thresh` (NaN too) as 1 in an int volume (the
/// others 0), its islands removed ([`mri_remove_volume_islands`] at 0.5, its own line); where that
/// leaves 0 the voxel keeps the mask's value when above `thresh`, else `FillVal`; elsewhere 0. A
/// new output is of the mask's type, one frame; the mask itself may be the output. `ERROR:
/// MRIremoveVolumeHoles(): nKeep==0` and `Null` for `nKeep` 0.
#[allow(clippy::too_many_arguments)]
pub fn mri_remove_volume_holes(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    thresh: f64,
    nkeep: i32,
    fill_val: f64,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIremoveVolumeHoles";
    let _ = writeln!(
        out,
        "MRIremoveVolumeHoles() thresh={}, nKeep={nkeep}",
        format_double("%g", thresh).unwrap_or_default()
    );
    if nkeep == 0 {
        let _ = writeln!(out, "ERROR: MRIremoveVolumeHoles(): nKeep==0");
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    if let Pair::Same(m) = p {
        let copy = mri_copy(g, out, err, m, None)?.unwrap_new();
        holes_into(g, out, err, &copy, m, thresh, nkeep, fill_val)?;
        return Ok(Ret::Given);
    }
    let Some((mask, Some(o))) = removal_dst(g, out, err, WHAT, p, &mut fresh)? else {
        return Ok(Ret::Null);
    };
    holes_into(g, out, err, mask, o, thresh, nkeep, fill_val)?;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIremoveVolumeHoles`' work into `o`.
#[allow(clippy::too_many_arguments)]
fn holes_into(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mask: &Mri,
    o: &mut Mri,
    thresh: f64,
    nkeep: i32,
    fill_val: f64,
) -> Result<(), Fatal> {
    let (w, h, d) = (mask.vg.width, mask.vg.height, mask.vg.depth);
    let mut inv = mri_alloc_sequence(g, out, err, w, h, d, MRI_INT, 1)?;
    mri_copy_header(g, out, err, mask, Some(&mut inv))?;
    mri_copy_pulse_parameters(mask, &mut inv);
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                let v = f64::from(gv(g, err, mask, c, r, s, 0)?);
                let v = if v > thresh { 0.0 } else { 1.0 };
                sv(g, err, &mut inv, c, r, s, 0, v)?;
            }
        }
    }
    let newmask = mri_remove_volume_islands(g, out, err, Pair::New(&inv), 0.5, nkeep)?.unwrap_new();
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                let v = f64::from(gv(g, err, &newmask, c, r, s, 0)?);
                if v == 0.0 {
                    let v0 = f64::from(gv(g, err, mask, c, r, s, 0)?);
                    let x = if v0 > thresh {
                        v0 as f32
                    } else {
                        fill_val as f32
                    };
                    sv(g, err, o, c, r, s, 0, x)?;
                } else {
                    sv(g, err, o, c, r, s, 0, 0.0)?;
                }
            }
        }
    }
    Ok(())
}
