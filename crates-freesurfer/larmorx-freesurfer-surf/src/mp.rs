// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The metric properties (`fs.surfio`, queue items 3.1-51 and 3.1-52): FreeSurfer 8.2.0's
//! `MRIScomputeMetricProperties(MRIS *)` (`utils/mrisurf_metricProperties.cpp` lines 1839-1881)
//! and what its default runs, `utils/mrisurf_metricProperties_fast.h`: the `MRIS_MP` copy's life
//! (`MRISMP_ctr`, `MRISMP_dtr`, `MRISMP_copy`, `MRISMP_load`, `MRISMP_unload` with its
//! comparison lines) and `MRIScomputeMetricProperties(MRIS_MP *)`: the vertex normals and areas
//! (with the random moves of degenerate vertices), the bounding box, the neighbour distances
//! (`utils/mrisComputeVertexDistancesWkr_extracted.h` on the copy), the faces' areas, normals and
//! angles (`utils/MRIScomputeTriangleProperties_extracted.h` on the copy), the mean distance
//! between neighbours and its deviation, the orientation of a sphere's or a plane's faces.
//!
//! `MRIScomputeMetricProperties(mris)` takes one of three routes by the environment, as
//! FreeSurfer reads it at every call: `FS_FASTER_MP` set, the `MetricPropertiesCalculator` of
//! [`crate::mp_faster`]; else the copy's route above unless only
//! `FREESURFER_OLD_MRIScomputeMetricProperties` is set, which runs `MRIScomputeMetricPropertiesWkr`
//! on the surface itself ([`crate::mp_slow`]); with both that and
//! `FREESURFER_NEW_MRIScomputeMetricProperties` set, both run and `MRISMP_unload` compares them
//! instead of copying back.
//!
//! The loops that FreeSurfer runs in parallel give results that do not depend on the thread count
//! (the faces and vertices are independent, the vertices to retry are sorted, the sums are the
//! 128 partial sums of `ROMP_Distributor`, [`romp_partials`]); here they run in order.
//!
//! What the copy never writes C leaves as the allocator's memory: the copy's normal of a vertex
//! whose normal cannot be computed in five tries, which `MRISMP_unload` copies into the surface
//! (FS-121). Here the copy starts with the surface's own normals, which such a vertex keeps.

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use std::sync::Arc;

use larmorx_core::defects::{FS_120, FS_121, FS_122};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{self, DIAG_SHOW};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix_ops::{Xyz, xyz_normalized_load};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt, sqrt64, sub, sub64};
use larmorx_freesurfer_core::utils::{cope_with_logic_problem2, fast_approx_atan2f};

use crate::aaa::{
    self, AnglesPerTriangle, DistanceFormula, FloatXyz, MrisStatus, SurfGlobals,
    copy_angles_per_triangle,
};
use crate::base::mris_normalize;
use crate::coords::{Io, centre};
use crate::dist;
use crate::facenorm;
use crate::mp_faster;
use crate::mp_slow;
use crate::mris::Mris;
use crate::mris_mp::MrisMp;
use crate::mrisurf::{FNV_INIT, fnv_add};
use crate::topology::vertex_vsize;

/// `ROMP_DISTRIBUTOR_PARTIAL_CAPACITY` (`include/romp_support.h`).
pub const ROMP_PARTIAL_CAPACITY: usize = 128;

/// `ROMP_Distributor_begin(lo, hi)`'s ranges (`utils/romp_support.cpp` lines 459-488): at most
/// 128 consecutive ranges covering `lo..hi`, the `i`-th of `(remaining) / (ranges left)` indices.
/// A reproducible sum adds its terms range by range from 0, then the ranges' sums in order to the
/// variable's starting value (`ROMP_Distributor_end`, [`romp_total`]).
pub fn romp_partials(lo: i32, hi: i32) -> Vec<(i32, i32)> {
    let size = hi - lo;
    if size <= 0 {
        return Vec::new();
    }
    let size = size.min(ROMP_PARTIAL_CAPACITY as i32);
    let mut lo = lo;
    let mut out = Vec::with_capacity(size as usize);
    for i in 0..size {
        let step = (hi - lo) / (size - i);
        out.push((lo, lo + step));
        lo += step;
    }
    out
}

/// A reproducible sum's total: `start` plus the ranges' sums in order.
pub fn romp_total(start: f64, partials: &[f64]) -> f64 {
    partials.iter().fold(start, |s, &p| add64(s, p))
}

/// `fnv_hash(i, k, &random_counter, lo, hi)` (`include/fnv_hash.h`): the FNV hash of the three
/// `unsigned int`s `i`, `k` and the counter (then incremented), its low 32 bits as a fraction of
/// `0xffffffff`, mapped to `lo..hi` in double and rounded to `float`.
pub fn fnv_hash(i: i32, k: i32, counter: &mut i32, lo: f64, hi: f64) -> f32 {
    let mut b = [0u8; 12];
    b[0..4].copy_from_slice(&(i as u32).to_le_bytes());
    b[4..8].copy_from_slice(&(k as u32).to_le_bytes());
    b[8..12].copy_from_slice(&(*counter as u32).to_le_bytes());
    *counter = counter.wrapping_add(1);
    let hash = fnv_add(FNV_INIT, &b);
    let f = (hash & 0xffff_ffff) as f64 / f64::from(0xffff_ffffu32);
    ((hi - lo) * f + lo) as f32
}

/// `RAN` (`MRIScomputeNormals`): the random moves' range, a thousandth of a millimetre.
pub const RAN: f64 = 0.001;

fn fast_h() -> String {
    format!("{}/utils/mrisurf_metricProperties_fast.h", diag::BUILD_ROOT)
}

pub(crate) fn extracted_h() -> String {
    format!(
        "{}/utils/mrisComputeVertexDistancesWkr_extracted.h",
        diag::BUILD_ROOT
    )
}

fn usz(n: i32) -> usize {
    usize::try_from(n).unwrap_or(0)
}

fn arr<'a, T>(a: &'a Option<Vec<T>>, what: &str) -> &'a [T] {
    a.as_deref()
        .unwrap_or_else(|| panic!("MRIS_MP::{what} is NULL"))
}

fn arr_mut<'a, T>(a: &'a mut Option<Vec<T>>, what: &str) -> &'a mut [T] {
    a.as_deref_mut()
        .unwrap_or_else(|| panic!("MRIS_MP::{what} is NULL"))
}

fn shared<'a, T>(a: &'a Option<Arc<Vec<T>>>, what: &str) -> &'a [T] {
    a.as_deref()
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("MRIS_MP::{what} is NULL"))
}

/// A face's vertex number as an index (FreeSurfer reads it unchecked).
pub(crate) fn vix(v: i32) -> usize {
    usize::try_from(v).expect("a vertex number outside the surface")
}

/// The `n`-th face of a vertex.
pub(crate) fn vface(mris: &Mris, vno: usize, n: usize) -> usize {
    let f = mris
        .topology(vno)
        .f
        .as_ref()
        .expect("VERTEX_TOPOLOGY::f is NULL")[n];
    usize::try_from(f).expect("a face number outside the surface")
}

/// The vertex's corner in its `n`-th face.
pub(crate) fn vcorner(mris: &Mris, vno: usize, n: usize) -> usize {
    usize::from(
        mris.topology(vno)
            .n
            .as_ref()
            .expect("VERTEX_TOPOLOGY::n is NULL")[n],
    )
}

/// A vertex's first `count` neighbours.
pub(crate) fn nbrs(mris: &Mris, vno: usize, count: i32) -> Vec<usize> {
    let n = usz(count);
    if n == 0 {
        return Vec::new();
    }
    mris.topology(vno)
        .v
        .as_ref()
        .expect("VERTEX_TOPOLOGY::v is NULL")[..n]
        .iter()
        .map(|&x| vix(x))
        .collect()
}

// ---- the copy's life ---------------------------------------------------------------------------

/// `MRISMP_ctr(mp)`: an empty copy (every array `NULL`, every number 0).
pub fn mp_ctr() -> MrisMp {
    MrisMp::default()
}

/// `MRISMP_dtr(mp)`: frees the copy's arrays and empties it.
pub fn mp_dtr(mp: &mut MrisMp) {
    *mp = MrisMp::default();
}

/// `MRISMP_makeDist2(mp, vno, neededCapacity)`: the vertex's distances in the copy, room for
/// the larger of `needed` and its size in use: the buffer it has when large enough; else that
/// buffer grown (its values kept), or the surface's stored buffer taken (`mrisStealDistStore`).
///
/// The buffer is in `v_dist[vno]` while the copy's distances point at it and in
/// `v_dist_buffer[vno]` while they do not (C keeps it in `v_dist_buffer` and points `v_dist` at
/// it). Where C would read the buffers' array before making it (only `MRISMP_load` with its
/// outputs and a vertex of size 0, a route no FreeSurfer 8.2.0 program takes), the vertex has none.
fn make_dist2(mp: &mut MrisMp, mris: &mut Mris, vno: usize, needed: i32) {
    let vsize = shared(&mp.v_vsize, "v_VSize")[vno];
    let needed = needed.max(vsize);
    let cap = arr(&mp.v_dist_capacity, "v_dist_capacity")[vno];
    if needed <= cap {
        if arr(&mp.v_dist, "v_dist")[vno].is_none() {
            let buf = mp.v_dist_buffer.as_mut().and_then(|b| b[vno].take());
            arr_mut(&mut mp.v_dist, "v_dist")[vno] = buf;
        }
        return;
    }
    let nv = usz(mp.nvertices);
    let bufs = mp.v_dist_buffer.get_or_insert_with(|| vec![None; nv]);
    let parked = bufs[vno].take();
    let existing = arr_mut(&mut mp.v_dist, "v_dist")[vno].take().or(parked);
    let buf = match existing {
        Some(mut b) => {
            b.resize(usz(needed), 0.0);
            b
        }
        None => dist::steal_dist_store(mris, vno, needed),
    };
    arr_mut(&mut mp.v_dist, "v_dist")[vno] = Some(buf);
    arr_mut(&mut mp.v_dist_capacity, "v_dist_capacity")[vno] = needed;
}

/// `MRISMP_makeDist(mp, vno)`: [`make_dist2`] with the vertex's present capacity.
fn make_dist(mp: &mut MrisMp, mris: &mut Mris, vno: usize) {
    let cap = arr(&mp.v_dist_capacity, "v_dist_capacity")[vno];
    make_dist2(mp, mris, vno, cap);
}

fn resized<T: Clone>(a: &mut Option<Vec<T>>, n: usize, fill: T) {
    a.get_or_insert_with(Vec::new).resize(n, fill);
}

/// `MRISMP_copy(dst, src, only_inputs, no_need_to_copy_xyz)`: `dst` shares `src`'s input arrays
/// and takes copies of the rest: the surface's numbers (its outputs too unless `only_inputs`), the
/// vertices' coordinates (unless `no_need_to_copy_xyz`), borders, canonical coordinates,
/// curvatures, original areas and face numbers; unless `only_inputs`, their signs (a plane's),
/// distances (as many as `src` has room for) and outputs, the faces' normals, angles and areas.
/// `dst`'s distances start empty; `dst`'s `v_neg` exists only for a plane.
///
/// Its only caller in FreeSurfer 8.2.0 (`MRIScomputeSSE_asThoughGradientApplied`) never takes the
/// route that calls it. Where C leaves memory as it was (the capacities and outputs of a copy
/// never loaded), they are 0 here. `mris` is the surface whose stored buffers a new distance
/// buffer takes.
pub fn mp_copy(
    dst: &mut MrisMp,
    src: &MrisMp,
    mris: &mut Mris,
    only_inputs: bool,
    no_need_to_copy_xyz: bool,
) {
    let nv = usz(src.nvertices);
    dst.status = src.status;
    dst.origxyz_status = src.origxyz_status;
    dst.nvertices = src.nvertices;
    dst.nfaces = src.nfaces;
    dst.nsize = src.nsize;
    dst.radius = src.radius;
    dst.faces_topology = src.faces_topology.clone();
    dst.dist_nsize = src.dist_nsize;
    if !only_inputs {
        (dst.xlo, dst.xhi, dst.ylo, dst.yhi, dst.zlo, dst.zhi) =
            (src.xlo, src.xhi, src.ylo, src.yhi, src.zlo, src.zhi);
        (dst.xctr, dst.yctr, dst.zctr) = (src.xctr, src.yctr, src.zctr);
        dst.total_area = src.total_area;
        dst.avg_vertex_area = src.avg_vertex_area;
        dst.avg_vertex_dist = src.avg_vertex_dist;
        dst.std_vertex_dist = src.std_vertex_dist;
        dst.neg_orig_area = src.neg_orig_area;
        dst.neg_area = src.neg_area;
    }
    dst.v_ripflag = src.v_ripflag.clone();
    dst.v_vsize = src.v_vsize.clone();
    dst.v_whitex = src.v_whitex.clone();
    dst.v_whitey = src.v_whitey.clone();
    dst.v_whitez = src.v_whitez.clone();
    dst.v_pialx = src.v_pialx.clone();
    dst.v_pialy = src.v_pialy.clone();
    dst.v_pialz = src.v_pialz.clone();
    dst.v_wnx = src.v_wnx.clone();
    dst.v_wny = src.v_wny.clone();
    dst.v_wnz = src.v_wnz.clone();
    for a in [
        &mut dst.v_x,
        &mut dst.v_y,
        &mut dst.v_z,
        &mut dst.v_cx,
        &mut dst.v_cy,
        &mut dst.v_cz,
        &mut dst.v_curv,
        &mut dst.v_origarea,
        &mut dst.v_area,
        &mut dst.v_nx,
        &mut dst.v_ny,
        &mut dst.v_nz,
    ] {
        resized(a, nv, 0.0);
    }
    resized(&mut dst.v_dist_capacity, nv, 0);
    resized(&mut dst.v_border, nv, 0);
    resized(&mut dst.v_fno, nv, 0);
    resized(&mut dst.v_dist, nv, None);
    if dst.status != MrisStatus::PLANE {
        dst.v_neg = None;
    } else {
        resized(&mut dst.v_neg, nv, 0);
    }
    for vno in 0..nv {
        // the buffer the distances pointed at stays in the buffers
        if let Some(b) = arr_mut(&mut dst.v_dist, "v_dist")[vno].take() {
            dst.v_dist_buffer.get_or_insert_with(|| vec![None; nv])[vno] = Some(b);
        }
        if !no_need_to_copy_xyz {
            arr_mut(&mut dst.v_x, "v_x")[vno] = arr(&src.v_x, "v_x")[vno];
            arr_mut(&mut dst.v_y, "v_y")[vno] = arr(&src.v_y, "v_y")[vno];
            arr_mut(&mut dst.v_z, "v_z")[vno] = arr(&src.v_z, "v_z")[vno];
        }
        arr_mut(&mut dst.v_border, "v_border")[vno] = arr(&src.v_border, "v_border")[vno];
        arr_mut(&mut dst.v_cx, "v_cx")[vno] = arr(&src.v_cx, "v_cx")[vno];
        arr_mut(&mut dst.v_cy, "v_cy")[vno] = arr(&src.v_cy, "v_cy")[vno];
        arr_mut(&mut dst.v_cz, "v_cz")[vno] = arr(&src.v_cz, "v_cz")[vno];
        arr_mut(&mut dst.v_curv, "v_curv")[vno] = arr(&src.v_curv, "v_curv")[vno];
        arr_mut(&mut dst.v_origarea, "v_origarea")[vno] = arr(&src.v_origarea, "v_origarea")[vno];
        arr_mut(&mut dst.v_fno, "v_fno")[vno] = arr(&src.v_fno, "v_fno")[vno];
        if !only_inputs {
            if let Some(neg) = dst.v_neg.as_mut() {
                neg[vno] = arr(&src.v_neg, "v_neg")[vno];
            }
            if let Some(sd) = arr(&src.v_dist, "v_dist")[vno].as_ref() {
                let scap = arr(&src.v_dist_capacity, "v_dist_capacity")[vno];
                make_dist2(dst, mris, vno, scap);
                let d = arr_mut(&mut dst.v_dist, "v_dist")[vno]
                    .as_mut()
                    .expect("made");
                d[..usz(scap)].copy_from_slice(&sd[..usz(scap)]);
            }
            arr_mut(&mut dst.v_area, "v_area")[vno] = arr(&src.v_area, "v_area")[vno];
            arr_mut(&mut dst.v_nx, "v_nx")[vno] = arr(&src.v_nx, "v_nx")[vno];
            arr_mut(&mut dst.v_ny, "v_ny")[vno] = arr(&src.v_ny, "v_ny")[vno];
            arr_mut(&mut dst.v_nz, "v_nz")[vno] = arr(&src.v_nz, "v_nz")[vno];
        }
    }
    let nf = usz(dst.nfaces);
    resized(&mut dst.f_area, nf, 0.0);
    resized(&mut dst.f_norm_set, nf, 0);
    resized(&mut dst.f_norm, nf, FloatXyz::default());
    resized(&mut dst.f_angle, nf, [0.0; 3]);
    dst.f_ripflag = src.f_ripflag.clone();
    dst.f_norm_orig_area = src.f_norm_orig_area.clone();
    dst.f_orig_angle = src.f_orig_angle.clone();
    if !only_inputs {
        for fno in 0..nf {
            arr_mut(&mut dst.f_norm_set, "f_normSet")[fno] = arr(&src.f_norm_set, "f_normSet")[fno];
            arr_mut(&mut dst.f_norm, "f_norm")[fno] = arr(&src.f_norm, "f_norm")[fno];
            let a = arr(&src.f_angle, "f_angle")[fno];
            copy_angles_per_triangle(&mut arr_mut(&mut dst.f_angle, "f_angle")[fno], &a);
            arr_mut(&mut dst.f_area, "f_area")[fno] = arr(&src.f_area, "f_area")[fno];
        }
    }
}

/// `MRISMP_load(mp, mris, loadOutputs, dx, dy, dz)`: the copy emptied (`MRISMP_dtr`) and filled
/// from the surface: its numbers (`status`, `origxyz_status`, the counts, `nsize`, `radius`,
/// `dist_nsize`; with `load_outputs` also `avg_vertex_dist` and the outputs), the faces' vertex
/// numbers, each vertex's rip flag, size in use, white and pial coordinates and white normal
/// (shared inputs), coordinates, border flag, canonical coordinates, curvature, original area and
/// face number, its distances empty (capacity 0); each face's rip flag, original area (through
/// the face-normal cache, which computes what is deferred) and original angles, its normal not
/// set. With `load_outputs` the vertices' areas, normals, signs (a plane's) and distances (as
/// many as their size in use, into room for the surface's capacity) and the faces' areas, normals
/// (`f->norm`, 0 without one) and angles. With `dxyz`, each vertex's `dx, dy, dz` into the three
/// arrays (0 for a ripped vertex). The copy's `v_neg` exists only for a plane.
///
/// C leaves the outputs it does not load as the allocator's memory; here the vertices' normals
/// and the faces' areas are the surface's (FS-121), the vertices' areas and signs and the faces'
/// normals and angles 0.
/// The route that loads the outputs (`MRIScomputeSSE_asThoughGradientApplied`'s) is never taken
/// in FreeSurfer 8.2.0.
pub fn mp_load(
    mp: &mut MrisMp,
    mris: &mut Mris,
    load_outputs: bool,
    mut dxyz: Option<(&mut [f32], &mut [f32], &mut [f32])>,
) {
    mp_dtr(mp);
    let nv = usz(mris.nvertices);
    let nf = usz(mris.nfaces);
    mp.faces_topology = Some((0..nf).map(|fno| mris.face(fno).v).collect());
    mp.status = mris.status;
    mp.origxyz_status = mris.origxyz_status;
    mp.nvertices = mris.nvertices;
    mp.nfaces = mris.nfaces;
    mp.nsize = mris.nsize;
    mp.radius = mris.radius;
    mp.dist_nsize = mris.dist_nsize;
    if load_outputs {
        mp.avg_vertex_dist = mris.avg_vertex_dist;
        (mp.xlo, mp.xhi, mp.ylo, mp.yhi, mp.zlo, mp.zhi) =
            (mris.xlo, mris.xhi, mris.ylo, mris.yhi, mris.zlo, mris.zhi);
        (mp.xctr, mp.yctr, mp.zctr) = (mris.xctr, mris.yctr, mris.zctr);
        mp.total_area = mris.total_area;
        mp.avg_vertex_area = mris.avg_vertex_area;
        mp.std_vertex_dist = mris.std_vertex_dist;
        mp.neg_orig_area = mris.neg_orig_area;
        mp.neg_area = mris.neg_area;
    }
    {
        let m: &Mris = mris;
        let col = |f: fn(&crate::mris::Vertex) -> f32| -> Vec<f32> {
            (0..nv).map(|vno| f(m.vertex(vno))).collect()
        };
        mp.v_ripflag = Some(Arc::new((0..nv).map(|v| m.vertex(v).ripflag).collect()));
        mp.v_vsize = Some(Arc::new((0..nv).map(|v| vertex_vsize(m, v)).collect()));
        mp.v_whitex = Some(Arc::new(col(|v| v.whitex)));
        mp.v_whitey = Some(Arc::new(col(|v| v.whitey)));
        mp.v_whitez = Some(Arc::new(col(|v| v.whitez)));
        mp.v_pialx = Some(Arc::new(col(|v| v.pialx)));
        mp.v_pialy = Some(Arc::new(col(|v| v.pialy)));
        mp.v_pialz = Some(Arc::new(col(|v| v.pialz)));
        mp.v_wnx = Some(Arc::new(col(|v| v.wnx)));
        mp.v_wny = Some(Arc::new(col(|v| v.wny)));
        mp.v_wnz = Some(Arc::new(col(|v| v.wnz)));
        mp.v_x = Some(col(|v| v.x));
        mp.v_y = Some(col(|v| v.y));
        mp.v_z = Some(col(|v| v.z));
        mp.v_dist_capacity = Some(vec![0; nv]);
        mp.v_border = Some((0..nv).map(|v| m.vertex(v).border).collect());
        mp.v_cx = Some(col(|v| v.cx));
        mp.v_cy = Some(col(|v| v.cy));
        mp.v_cz = Some(col(|v| v.cz));
        mp.v_curv = Some(col(|v| v.curv));
        mp.v_origarea = Some(col(|v| v.origarea));
        mp.v_fno = Some((0..nv).map(|v| m.vertex(v).fno).collect());
        mp.v_area = Some(if load_outputs {
            col(|v| v.area)
        } else {
            vec![0.0; nv]
        });
        // FS-121: the normals start as the surface's
        mp.v_nx = Some(col(|v| v.nx));
        mp.v_ny = Some(col(|v| v.ny));
        mp.v_nz = Some(col(|v| v.nz));
        mp.v_neg = if m.status != MrisStatus::PLANE {
            None
        } else if load_outputs {
            Some((0..nv).map(|v| m.vertex(v).neg).collect())
        } else {
            Some(vec![0; nv])
        };
    }
    mp.v_dist = Some(vec![None; nv]);
    for vno in 0..nv {
        if load_outputs {
            let cap = mris.vertex(vno).dist_capacity;
            make_dist2(mp, mris, vno, cap);
            let vsize = usz(shared(&mp.v_vsize, "v_VSize")[vno]);
            if vsize > 0 {
                let src = mris
                    .vertex(vno)
                    .dist
                    .as_ref()
                    .expect("VERTEX::dist is NULL")[..vsize]
                    .to_vec();
                let d = arr_mut(&mut mp.v_dist, "v_dist")[vno]
                    .as_mut()
                    .expect("made");
                d[..vsize].copy_from_slice(&src);
            }
        }
        if let Some((dx, dy, dz)) = dxyz.as_mut() {
            let v = mris.vertex(vno);
            if v.ripflag != 0 {
                (dx[vno], dy[vno], dz[vno]) = (0.0, 0.0, 0.0);
            } else {
                (dx[vno], dy[vno], dz[vno]) = (v.dx, v.dy, v.dz);
            }
        }
    }
    let mut f_norm_orig_area = Vec::with_capacity(nf);
    let mut f_orig_angle = Vec::with_capacity(nf);
    let mut f_ripflag = Vec::with_capacity(nf);
    let mut f_area = vec![0.0f32; nf];
    let mut f_norm = vec![FloatXyz::default(); nf];
    let mut f_angle: Vec<AnglesPerTriangle> = vec![[0.0; 3]; nf];
    for fno in 0..nf {
        let e = facenorm::get_face_norm(mris, fno);
        let f = mris.face(fno);
        f_norm_orig_area.push(e.orig_area);
        f_orig_angle.push(f.orig_angle);
        f_ripflag.push(f.ripflag);
        // FS-121: the copy's face areas start as the surface's (C leaves them as memory contents
        // where the copy computes none: a ripped face's, which a plane's vertices add)
        f_area[fno] = f.area;
        if load_outputs {
            f_norm[fno] = match &f.norm {
                None => FloatXyz::default(),
                Some(d) => FloatXyz {
                    x: d.get(1, 1) as f32,
                    y: d.get(2, 1) as f32,
                    z: d.get(3, 1) as f32,
                },
            };
            f_angle[fno] = f.angle;
        }
    }
    mp.f_ripflag = Some(Arc::new(f_ripflag));
    mp.f_norm_orig_area = Some(Arc::new(f_norm_orig_area));
    mp.f_orig_angle = Some(Arc::new(f_orig_angle));
    mp.f_area = Some(f_area);
    mp.f_norm_set = Some(vec![0; nf]);
    mp.f_norm = Some(f_norm);
    mp.f_angle = Some(f_angle);
}

/// `comparisonWkr(no, lhs, rhs, expr, line, &errorCount)`: when `lhs != rhs` and fewer than 12
/// differences were counted in this part, `no:<no>  <expr>  <lhs> != <rhs>  during
/// count_MRIScomputeMetricProperties_calls:<calls>` (`%f`) on standard output, and the first time
/// in the run `b <fast.h>:335  needed`.
///
/// FS-122 (corrected by default, rule A): FreeSurfer compares with `==`, so a NaN, never equal to
/// itself, is reported as a difference where both sides hold the same value (a NaN white or pial
/// coordinate or white normal, a NaN radius: at every computation of the metric properties).
/// larmorx takes two equal bit patterns as equal and prints nothing for them; `reproduce` prints
/// FreeSurfer's lines.
struct Compare<'a, 'b, 'c> {
    g: &'a mut Globals,
    sg: &'a mut SurfGlobals,
    io: &'a mut Io<'b>,
    errors: i32,
    _c: std::marker::PhantomData<&'c ()>,
}

impl Compare<'_, '_, '_> {
    fn cmp(&mut self, no: i32, lhs: f64, rhs: f64, expr: &str) {
        if lhs == rhs {
            return;
        }
        if lhs.to_bits() == rhs.to_bits() {
            self.g.notices.add(
                &FS_122,
                "two equal NaNs of the metric properties' copy are not reported as a difference",
            );
            if self.g.notices.fix() {
                return;
            }
        }
        let e = self.errors;
        self.errors += 1;
        if e > 10 {
            return;
        }
        let _ = writeln!(
            self.io.out,
            "no:{no}  {expr}  {} != {}  during count_MRIScomputeMetricProperties_calls:{}",
            fmt_f(lhs, 0, 6),
            fmt_f(rhs, 0, 6),
            self.sg.mp_calls
        );
        self.sg.mp_comparisons += 1;
        if self.sg.mp_comparisons == 1 {
            let _ = writeln!(self.io.out, "b {}:335  needed", fast_h());
        }
    }
}

/// `memcmp` of two faces' 12 bytes of vertex numbers, as glibc 2.35's x86-64 `memcmp` answers
/// (0, or the difference of the first differing bytes as unsigned values).
fn memcmp_ints(a: &[i32; 3], b: &[i32; 3]) -> i32 {
    let ab: Vec<u8> = a.iter().flat_map(|x| x.to_le_bytes()).collect();
    let bb: Vec<u8> = b.iter().flat_map(|x| x.to_le_bytes()).collect();
    for (x, y) in ab.iter().zip(bb.iter()) {
        if x != y {
            return i32::from(*x) - i32::from(*y);
        }
    }
    0
}

/// `MRISMP_unload(mris, mp, check)`: the copy's results into the surface (`check` false) or
/// compared with the surface's (`check` true, [`Compare`]'s lines). In order: `avg_vertex_dist`
/// set (compared, after the faces' vertex numbers, when checking); the copy's inputs compared with
/// the surface's (`status`, `origxyz_status`, `nvertices`, `nfaces`, `nsize`, `radius`, the
/// topology: `1` against whether they are equal); `dist_nsize` and the outputs (`xlo` ... `zctr`,
/// `total_area`, `avg_vertex_area`, `std_vertex_dist`, `neg_orig_area`, `neg_area`) copied or
/// compared. Each vertex not ripped in both: its sign (a plane's), its distances (moved into the
/// surface with their capacity, `mrisSetDist`: a capacity below the surface's fails an assertion,
/// FS-03; or the first `vtotal` compared, after `cheapAssert(vtotal <= v->dist_capacity)`), its
/// inputs compared (rip flag, white and pial coordinates, white normal), its coordinates, border
/// flag, canonical coordinates, curvature, original area, face number, area and normal copied or
/// compared. Each face not ripped in both: its original area (the face-normal cache's, computed
/// when deferred) and original angles set (or compared), its normal when the copy set one, its rip
/// flag compared (checking), its area copied or compared.
///
/// FS-120 (corrected by default, rule A): FreeSurfer 8.2.0 never copies the faces' angles back,
/// so after the default computation of the metric properties a surface's face angles are those it
/// had before (0 for a new surface), where its two other computations (the old one, the faster
/// one) set them. larmorx copies them; `reproduce` leaves them.
pub fn mp_unload(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    mp: &mut MrisMp,
    check: bool,
) -> Result<(), Fatal> {
    let mut c = Compare {
        g,
        sg,
        io,
        errors: 0,
        _c: std::marker::PhantomData,
    };
    if check {
        let ft = arr(&mp.faces_topology, "faces_topology");
        for fno in 0..usz(mris.nfaces) {
            let d = memcmp_ints(&ft[fno], &mris.face(fno).v);
            c.cmp(fno as i32, 0.0, f64::from(d), "0");
        }
        c.cmp(
            -1,
            mris.avg_vertex_dist,
            mp.avg_vertex_dist,
            "mris->avg_vertex_dist",
        );
    } else {
        mris.avg_vertex_dist = mp.avg_vertex_dist;
    }
    // MRIS_MP__LIST_MRIS_IN: 1 against whether equal
    let same = |a: bool| f64::from(i32::from(a));
    c.cmp(-1, 1.0, same(mris.status == mp.status), "1");
    c.cmp(-1, 1.0, same(mris.origxyz_status == mp.origxyz_status), "1");
    c.cmp(-1, 1.0, same(mris.nvertices == mp.nvertices), "1");
    c.cmp(-1, 1.0, same(mris.nfaces == mp.nfaces), "1");
    c.cmp(-1, 1.0, same(mris.nsize == mp.nsize), "1");
    if mris.radius != mp.radius && mris.radius.to_bits() == mp.radius.to_bits() {
        // a NaN radius: FS-122
        c.g.notices.add(
            &FS_122,
            "a NaN radius of the metric properties' copy is not reported as a difference",
        );
        if c.g.notices.reproduce() {
            c.cmp(-1, 1.0, 0.0, "1");
        }
    } else {
        c.cmp(-1, 1.0, same(mris.radius == mp.radius), "1");
    }
    c.cmp(-1, 1.0, 1.0, "1");
    macro_rules! surface_field {
        ($f:ident, $name:literal) => {
            if check {
                c.cmp(-1, f64::from(mris.$f), f64::from(mp.$f), $name);
            } else {
                mris.$f = mp.$f;
            }
        };
    }
    surface_field!(dist_nsize, "mris->dist_nsize");
    surface_field!(xlo, "mris->xlo");
    surface_field!(xhi, "mris->xhi");
    surface_field!(ylo, "mris->ylo");
    surface_field!(yhi, "mris->yhi");
    surface_field!(zlo, "mris->zlo");
    surface_field!(zhi, "mris->zhi");
    surface_field!(xctr, "mris->xctr");
    surface_field!(yctr, "mris->yctr");
    surface_field!(zctr, "mris->zctr");
    surface_field!(total_area, "mris->total_area");
    surface_field!(avg_vertex_area, "mris->avg_vertex_area");
    surface_field!(std_vertex_dist, "mris->std_vertex_dist");
    surface_field!(neg_orig_area, "mris->neg_orig_area");
    surface_field!(neg_area, "mris->neg_area");

    c.errors = 0;
    for vno in 0..usz(mris.nvertices) {
        if shared(&mp.v_ripflag, "v_ripflag")[vno] != 0 && mris.vertex(vno).ripflag != 0 {
            continue;
        }
        if let Some(neg) = mp.v_neg.as_ref() {
            if check {
                let a = f64::from(mris.vertex(vno).neg);
                c.cmp(vno as i32, a, f64::from(neg[vno]), "v->neg");
            } else {
                mris.vertex_mut(vno).neg = neg[vno];
            }
        }
        if arr(&mp.v_dist, "v_dist")[vno].is_some() {
            if !check {
                let d = arr_mut(&mut mp.v_dist, "v_dist")[vno]
                    .take()
                    .expect("present");
                if let Some(b) = mp.v_dist_buffer.as_mut() {
                    b[vno] = None;
                }
                let cap = arr(&mp.v_dist_capacity, "v_dist_capacity")[vno];
                arr_mut(&mut mp.v_dist_capacity, "v_dist_capacity")[vno] = 0;
                dist::set_dist(c.g, c.io.out, c.io.err, mris, vno, d, cap)?;
            } else {
                let vtotal = i32::from(mris.topology(vno).vtotal);
                if vtotal > mris.vertex(vno).dist_capacity {
                    return Err(diag::assert_failed(
                        c.g,
                        c.io.out,
                        c.io.err,
                        &fast_h(),
                        390,
                        "vtotal <= v->dist_capacity",
                    ));
                }
                let md = arr(&mp.v_dist, "v_dist")[vno].as_ref().expect("present");
                let sd = mris
                    .vertex(vno)
                    .dist
                    .as_ref()
                    .expect("VERTEX::dist is NULL");
                for n in 0..usz(vtotal) {
                    c.cmp(
                        vno as i32,
                        f64::from(sd[n]),
                        f64::from(md[n]),
                        "mris_dist[n]",
                    );
                }
            }
        }
        {
            let v = mris.vertex(vno);
            let pairs = [
                (
                    v.ripflag as f32,
                    shared(&mp.v_ripflag, "")[vno] as f32,
                    "v->ripflag",
                ),
                (v.whitex, shared(&mp.v_whitex, "")[vno], "v->whitex"),
                (v.whitey, shared(&mp.v_whitey, "")[vno], "v->whitey"),
                (v.whitez, shared(&mp.v_whitez, "")[vno], "v->whitez"),
                (v.pialx, shared(&mp.v_pialx, "")[vno], "v->pialx"),
                (v.pialy, shared(&mp.v_pialy, "")[vno], "v->pialy"),
                (v.pialz, shared(&mp.v_pialz, "")[vno], "v->pialz"),
                (v.wnx, shared(&mp.v_wnx, "")[vno], "v->wnx"),
                (v.wny, shared(&mp.v_wny, "")[vno], "v->wny"),
                (v.wnz, shared(&mp.v_wnz, "")[vno], "v->wnz"),
            ];
            for (a, b, e) in pairs {
                c.cmp(vno as i32, f64::from(a), f64::from(b), e);
            }
        }
        macro_rules! vertex_field {
            ($f:ident, $a:ident, $name:literal) => {
                let m = arr(&mp.$a, stringify!($a))[vno];
                if check {
                    let a = f64::from(mris.vertex(vno).$f);
                    c.cmp(vno as i32, a, f64::from(m), $name);
                } else {
                    mris.vertex_mut(vno).$f = m;
                }
            };
        }
        vertex_field!(x, v_x, "v->x");
        vertex_field!(y, v_y, "v->y");
        vertex_field!(z, v_z, "v->z");
        vertex_field!(border, v_border, "v->border");
        vertex_field!(cx, v_cx, "v->cx");
        vertex_field!(cy, v_cy, "v->cy");
        vertex_field!(cz, v_cz, "v->cz");
        vertex_field!(curv, v_curv, "v->curv");
        vertex_field!(origarea, v_origarea, "v->origarea");
        vertex_field!(fno, v_fno, "v->fno");
        vertex_field!(area, v_area, "v->area");
        vertex_field!(nx, v_nx, "v->nx");
        vertex_field!(ny, v_ny, "v->ny");
        vertex_field!(nz, v_nz, "v->nz");
    }

    c.errors = 0;
    for fno in 0..usz(mris.nfaces) {
        if shared(&mp.f_ripflag, "f_ripflag")[fno] != 0 && mris.face(fno).ripflag != 0 {
            continue;
        }
        let e = facenorm::get_face_norm(mris, fno);
        let oa = shared(&mp.f_norm_orig_area, "f_norm_orig_area")[fno];
        let oang = shared(&mp.f_orig_angle, "f_orig_angle")[fno];
        if check {
            c.cmp(
                fno as i32,
                f64::from(e.orig_area),
                f64::from(oa),
                "fNorm->orig_area",
            );
            let d = aaa::cmp_angles_per_triangle(&mris.face(fno).orig_angle, &oang);
            c.cmp(
                fno as i32,
                f64::from(d),
                0.0,
                "cmpAnglesPerTriangle(f->orig_angle,mp->f_orig_angle[fno])",
            );
        } else {
            facenorm::set_face_orig_area(mris, fno, oa);
            copy_angles_per_triangle(&mut mris.face_mut(fno).orig_angle, &oang);
        }
        if arr(&mp.f_norm_set, "f_normSet")[fno] != 0 {
            let n = arr(&mp.f_norm, "f_norm")[fno];
            if !check {
                facenorm::set_face_norm(mris, fno, n.x, n.y, n.z);
            } else {
                c.cmp(fno as i32, f64::from(e.nx), f64::from(n.x), "fNorm->nx");
                c.cmp(fno as i32, f64::from(e.ny), f64::from(n.y), "fNorm->ny");
                c.cmp(fno as i32, f64::from(e.nz), f64::from(n.z), "fNorm->nz");
            }
        }
        let fa = arr(&mp.f_area, "f_area")[fno];
        if check {
            let rf = shared(&mp.f_ripflag, "f_ripflag")[fno];
            let r = f64::from(mris.face(fno).ripflag);
            c.cmp(fno as i32, r, f64::from(rf), "f->ripflag");
            let a = f64::from(mris.face(fno).area);
            c.cmp(fno as i32, a, f64::from(fa), "f->area");
        } else {
            mris.face_mut(fno).area = fa;
            let ang = arr(&mp.f_angle, "f_angle")[fno];
            if mris.face(fno).angle.map(f32::to_bits) != ang.map(f32::to_bits) {
                c.g.notices.add(
                    &FS_120,
                    "the faces' angles computed with the metric properties are kept in the surface",
                );
                if c.g.notices.fix() {
                    mris.face_mut(fno).angle = ang;
                }
            }
        }
    }
    Ok(())
}

// ---- MRIScomputeMetricProperties(MRIS_MP *) --------------------------------------------------

fn mp_xyz(mp: &MrisMp, vno: usize) -> [f32; 3] {
    [
        arr(&mp.v_x, "v_x")[vno],
        arr(&mp.v_y, "v_y")[vno],
        arr(&mp.v_z, "v_z")[vno],
    ]
}

fn mp_face(mp: &MrisMp, fno: usize) -> [i32; 3] {
    arr(&mp.faces_topology, "faces_topology")[fno]
}

/// The three corners' coordinates of a face of the copy.
fn mp_face_xyz(mp: &MrisMp, fno: usize) -> [[f32; 3]; 3] {
    let v = mp_face(mp, fno);
    [
        mp_xyz(mp, vix(v[0])),
        mp_xyz(mp, vix(v[1])),
        mp_xyz(mp, vix(v[2])),
    ]
}

/// The bounding box of points, from 10000 and -10000 (each coordinate above the high end or below
/// the low end replacing it), and its centre: `MRISMP_computeSurfaceDimensions`.
pub(crate) fn bounding_box(points: impl Iterator<Item = [f32; 3]>) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([10000.0f32; 3], [-10000.0f32; 3]);
    for p in points {
        for k in 0..3 {
            if p[k] > hi[k] {
                hi[k] = p[k];
            }
            if p[k] < lo[k] {
                lo[k] = p[k];
            }
        }
    }
    (lo, hi)
}

fn mp_surface_dimensions(mp: &mut MrisMp) {
    let (lo, hi) = bounding_box((0..usz(mp.nvertices)).map(|v| mp_xyz(mp, v)));
    (mp.xlo, mp.ylo, mp.zlo) = (lo[0], lo[1], lo[2]);
    (mp.xhi, mp.yhi, mp.zhi) = (hi[0], hi[1], hi[2]);
    mp.xctr = centre(lo[0], hi[0]);
    mp.yctr = centre(lo[1], hi[1]);
    mp.zctr = centre(lo[2], hi[2]);
}

/// The corners `n - 1`, `n + 1` and `n` (cyclically) of a face: the vertices `mrisNormalFace` and
/// `mrisTriangleArea` call `vno0`, `vno1`, `vno2`.
pub(crate) fn corners(v: &[i32; 3], n: usize) -> (usize, usize, usize) {
    let n0 = if n == 0 { 2 } else { n - 1 };
    let n1 = if n == 2 { 0 } else { n + 1 };
    (vix(v[n0]), vix(v[n1]), vix(v[n]))
}

pub(crate) fn diff3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [sub(a[0], b[0]), sub(a[1], b[1]), sub(a[2], b[2])]
}

/// The cross product as `mrisTriangleArea`, `mrisFaceAreaNormal` and `mrisNormalFace` write it, of
/// the edges `a = p2 - p0` and `b = p1 - p2`: `-b[1] a[2] + a[1] b[2]`, `b[0] a[2] - a[0] b[2]`,
/// `-b[0] a[1] + a[0] b[1]`, which the official build computes as `a[1] b[2] - b[1] a[2]`,
/// `b[0] a[2] - a[0] b[2]`, `a[0] b[1] - b[0] a[1]` (no negation: the NaNs keep their signs).
pub(crate) fn edges_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        sub(mul(a[1], b[2]), mul(b[1], a[2])),
        sub(mul(b[0], a[2]), mul(a[0], b[2])),
        sub(mul(a[0], b[1]), mul(b[0], a[1])),
    ]
}

/// `mrismp_TriangleArea` (and `mrisTriangleArea`) of the corners `p0 = vno0`, `p1 = vno1`, `p2 =
/// vno2`: half the length of [`edges_cross`], in `float`.
pub(crate) fn triangle_area(p0: [f32; 3], p1: [f32; 3], p2: [f32; 3]) -> f32 {
    let d = edges_cross(diff3(p2, p0), diff3(p1, p2));
    let s = add(add(mul(d[0], d[0]), mul(d[1], d[1])), mul(d[2], d[2]));
    div(sqrt(s), 2.0)
}

/// `mrismp_NormalFace` (and `mrisNormalFace`): [`edges_cross`] of the two edges made unit length
/// first (`mrisNormalize`), the product too when `UnitizeNormalFace` is set.
pub(crate) fn normal_face(sg: &SurfGlobals, p0: [f32; 3], p1: [f32; 3], p2: [f32; 3]) -> [f32; 3] {
    let mut a = diff3(p2, p0);
    let mut b = diff3(p1, p2);
    mris_normalize(&mut a);
    mris_normalize(&mut b);
    let mut n = edges_cross(a, b);
    if sg.unitize_normal_face != 0 {
        mris_normalize(&mut n);
    }
    n
}

/// `mrismp_setFaceAreaNormal` (and `mrisFaceAreaNormal`): [`edges_cross`] of `p2 - p0` and
/// `p1 - p2`, the corners in the face's order.
pub(crate) fn face_area_normal(p: [[f32; 3]; 3]) -> [f32; 3] {
    edges_cross(diff3(p[2], p[0]), diff3(p[1], p[2]))
}

fn mp_set_face_norm(mp: &mut MrisMp, fno: usize, n: [f32; 3]) {
    arr_mut(&mut mp.f_norm_set, "f_normSet")[fno] = 1;
    arr_mut(&mut mp.f_norm, "f_norm")[fno] = FloatXyz {
        x: n[0],
        y: n[1],
        z: n[2],
    };
}

fn mp_set_face_area_normal(mp: &mut MrisMp, fno: usize) {
    let n = face_area_normal(mp_face_xyz(mp, fno));
    mp_set_face_norm(mp, fno, n);
}

/// `area / 3.0` (with `fix_vertex_area`) or `area / 2.0`, in double, rounded to `float`.
pub(crate) fn vertex_area_share(sg: &SurfGlobals, area: f32) -> f32 {
    let d = if sg.fix_vertex_area != 0 { 3.0 } else { 2.0 };
    div64(f64::from(area), d) as f32
}

/// The statuses whose degenerate vertices `MRIScomputeNormals` reports under `DIAG_VERBOSE`.
pub(crate) fn reports_degenerate(s: MrisStatus) -> bool {
    s == MrisStatus::SPHERICAL_PATCH
        || s == MrisStatus::PARAMETERIZED_SPHERE
        || s == MrisStatus::SPHERE
}

/// One vertex's normal and area of `MRIScomputeNormals`: over its unripped faces (`ripped` by
/// face number), the sum of the unit-edge cross products at its corner ([`normal_face`]) and of
/// the corner's triangle areas ([`triangle_area`]); `None` when it has such faces and the sum has
/// no length (or is not a number), else the sum made unit length and the area.
pub(crate) fn vertex_normal(
    sg: &SurfGlobals,
    mris: &Mris,
    k: usize,
    ripped: &dyn Fn(usize) -> bool,
    face: &dyn Fn(usize) -> [i32; 3],
    xyz: &dyn Fn(usize) -> [f32; 3],
) -> Option<([f32; 3], f32)> {
    let mut snorm = [0.0f32; 3];
    let mut area = 0.0f32;
    let mut count = 0;
    for n in 0..usize::from(mris.topology(k).num) {
        let fno = vface(mris, k, n);
        if ripped(fno) {
            continue;
        }
        count += 1;
        let (a, b, c) = corners(&face(fno), vcorner(mris, k, n));
        let (p0, p1, p2) = (xyz(a), xyz(b), xyz(c));
        let norm = normal_face(sg, p0, p1, p2);
        snorm[0] = add(snorm[0], norm[0]);
        snorm[1] = add(snorm[1], norm[1]);
        snorm[2] = add(snorm[2], norm[2]);
        area = add(area, triangle_area(p0, p1, p2));
    }
    if count == 0 || f64::from(mris_normalize(&mut snorm)) > 0.0 {
        Some((snorm, area))
    } else {
        None
    }
}

/// `MRISMP_computeNormals(mp, false)`: each ripped face makes its vertices border vertices; each
/// other face with an unripped vertex gets the cross product of two edges as its normal
/// ([`face_area_normal`]). Then each unripped vertex, in up to five rounds: its normal and area
/// ([`vertex_normal`]), the area divided by 3 (2 without `fix_vertex_area`), the original area
/// set to it too when negative; a vertex whose normal has no length is retried in the next round
/// after it and its 1-hop neighbours move by up to a thousandth of a millimetre ([`fnv_hash`] of
/// the round and the vertex number: the vertex's x and y, then for a surface other than a plane
/// or a cut its z and each neighbour's x, y and z, and its unripped faces' normals again), in
/// vertex order, with `vertex <vno>: degenerate normal` on standard error for a sphere's under
/// `DIAG_VERBOSE` and `   k=<vno> <n> nbr = <nbr> / <vnum>` on standard output for each neighbour
/// under `DIAG_SHOW` and `DIAG_VERBOSE`. Returns the vertices still without a normal after the
/// fifth round.
fn mp_compute_normals(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    mp: &mut MrisMp,
    mris: &Mris,
) -> Vec<usize> {
    let nv = usz(mp.nvertices);
    let nf = usz(mp.nfaces);
    for k in 0..nf {
        let v = mp_face(mp, k);
        if shared(&mp.f_ripflag, "f_ripflag")[k] != 0 {
            for n in 0..3 {
                arr_mut(&mut mp.v_border, "v_border")[vix(v[n])] = 1;
            }
        } else {
            let rip = shared(&mp.v_ripflag, "v_ripflag");
            if (0..3).any(|vi| rip[vix(v[vi])] == 0) {
                mp_set_face_area_normal(mp, k);
            }
        }
    }
    let mut pending: Vec<usize> = (0..nv)
        .filter(|&k| shared(&mp.v_ripflag, "v_ripflag")[k] == 0)
        .collect();
    let mut trial = 0;
    while trial < 5 && !pending.is_empty() {
        let mut next: Vec<usize> = Vec::new();
        for &k in &pending {
            let r = {
                let m: &MrisMp = mp;
                let frip = shared(&m.f_ripflag, "f_ripflag");
                vertex_normal(sg, mris, k, &|f| frip[f] != 0, &|f| mp_face(m, f), &|v| {
                    mp_xyz(m, v)
                })
            };
            match r {
                Some((snorm, area)) => {
                    let area = vertex_area_share(sg, area);
                    arr_mut(&mut mp.v_area, "v_area")[k] = area;
                    if arr(&mp.v_origarea, "v_origarea")[k] < 0.0 {
                        arr_mut(&mut mp.v_origarea, "v_origarea")[k] = area;
                    }
                    arr_mut(&mut mp.v_nx, "v_nx")[k] = snorm[0];
                    arr_mut(&mut mp.v_ny, "v_ny")[k] = snorm[1];
                    arr_mut(&mut mp.v_nz, "v_nz")[k] = snorm[2];
                }
                None => next.push(k),
            }
        }
        if next.is_empty() {
            pending.clear();
            break;
        }
        next.sort_unstable();
        for &k in &next {
            if reports_degenerate(mp.status) && diag::verbose_on(g) {
                let _ = writeln!(io.err, "vertex {k}: degenerate normal");
            }
            let mut counter = 0;
            let kk = k as i32;
            let r = |c: &mut i32| fnv_hash(trial, kk, c, -RAN, RAN);
            let d = r(&mut counter);
            arr_mut(&mut mp.v_x, "v_x")[k] = add(arr(&mp.v_x, "v_x")[k], d);
            let d = r(&mut counter);
            arr_mut(&mut mp.v_y, "v_y")[k] = add(arr(&mp.v_y, "v_y")[k], d);
            if mp.status == MrisStatus::PLANE || mp.status == MrisStatus::CUT {
                continue;
            }
            let d = r(&mut counter);
            arr_mut(&mut mp.v_z, "v_z")[k] = add(arr(&mp.v_z, "v_z")[k], d);
            let vt = mris.topology(k);
            for (n, vn) in nbrs(mris, k, i32::from(vt.vnum)).into_iter().enumerate() {
                if g.gdiag & DIAG_SHOW != 0 && diag::verbose_on(g) {
                    let _ = writeln!(io.out, "   k={k:5} {n} nbr = {vn:5} / {}", vt.vnum);
                }
                for a in [&mut mp.v_x, &mut mp.v_y, &mut mp.v_z] {
                    let d = r(&mut counter);
                    let a = arr_mut(a, "v_xyz");
                    a[vn] = add(a[vn], d);
                }
            }
            for n in 0..usize::from(vt.num) {
                let fno = vface(mris, k, n);
                if shared(&mp.f_ripflag, "f_ripflag")[fno] != 0 {
                    continue;
                }
                mp_set_face_area_normal(mp, fno);
            }
        }
        pending = next;
        trial += 1;
    }
    pending
}

/// The per-vertex values of the sphere's distance formula (`PerVertexInfo`).
#[derive(Clone, Copy, Default)]
pub(crate) struct SphereInfo {
    pub ripped: bool,
    pub ripped_or_zero_length: bool,
    pub unnormalized: [f32; 3],
    pub length: f32,
    pub normalized: Xyz,
}

/// A vertex's [`SphereInfo`].
pub(crate) fn sphere_info(ripped: bool, p: [f32; 3]) -> SphereInfo {
    let mut i = SphereInfo {
        ripped,
        ripped_or_zero_length: ripped,
        ..SphereInfo::default()
    };
    if ripped {
        return i;
    }
    i.unnormalized = p;
    let (nrm, len) = xyz_normalized_load(p[0], p[1], p[2]);
    i.normalized = nrm;
    i.length = len;
    if len.abs() < f32::EPSILON {
        i.ripped_or_zero_length = true;
    }
    i
}

/// The sphere's distance from `vi` to `vi2`: 0 when `vi2` is ripped or at the origin, else the
/// angle between them (`|XYZApproxAngle_knownLength|`) times `vi`'s radius.
pub(crate) fn sphere_distance(vi: &SphereInfo, vi2: &SphereInfo) -> f32 {
    if vi2.ripped_or_zero_length {
        return 0.0;
    }
    let [x2, y2, z2] = vi2.unnormalized;
    let angle = crate::metric::approx_angle(&vi.normalized, x2, y2, z2, vi2.length).abs();
    mul(angle, vi.length)
}

/// The plane formula's distance: `sqrt(xd² + yd² + zd²)` in `float`, summed as the copy's worker
/// is compiled: `zd² + (yd² + xd²)`.
pub(crate) fn plane_distance(p: [f32; 3], q: [f32; 3]) -> f32 {
    let (xd, yd, zd) = (sub(p[0], q[0]), sub(p[1], q[1]), sub(p[2], q[2]));
    sqrt(add(mul(zd, zd), add(mul(yd, yd), mul(xd, xd))))
}

/// `MRISMP_computeVertexDistancesWkr(mp, new_dist_nsize, false)`: as
/// [`crate::metric::compute_vertex_distances_wkr`] on the copy (its coordinates and `status`, its
/// distances made by [`make_dist`], the buffers' array first): `new_dist_nsize` outside `1..=3`
/// fails an assertion (lines 15, 16; FS-03). The copy's only caller never checks.
fn mp_compute_vertex_distances_wkr(
    g: &mut Globals,
    io: &mut Io,
    mp: &mut MrisMp,
    mris: &mut Mris,
    new_dist_nsize: i32,
) -> Result<(), Fatal> {
    for (ok, line, tst) in [
        (0 < new_dist_nsize, 15, "0 < new_dist_nsize"),
        (new_dist_nsize <= 3, 16, "new_dist_nsize <= 3"),
    ] {
        if !ok {
            return Err(diag::assert_failed(
                g,
                io.out,
                io.err,
                &extracted_h(),
                line,
                tst,
            ));
        }
    }
    let nv = usz(mp.nvertices);
    mp.v_dist_buffer.get_or_insert_with(|| vec![None; nv]);
    let formula = aaa::status_distance_formula(g, io.out, io.err, mp.status)?;
    let mut nonzero_x = 0i32;
    match formula {
        DistanceFormula::Formula0 => {
            for vno in 0..nv {
                if shared(&mp.v_ripflag, "v_ripflag")[vno] != 0 {
                    continue;
                }
                let p = mp_xyz(mp, vno);
                make_dist(mp, mris, vno);
                if p[0] != 0.0 {
                    nonzero_x += 1;
                }
                let vtotal = i32::from(mris.topology(vno).vtotal);
                let ds: Vec<f32> = nbrs(mris, vno, vtotal)
                    .into_iter()
                    .map(|n| plane_distance(p, mp_xyz(mp, n)))
                    .collect();
                store(mp, vno, &ds);
            }
        }
        DistanceFormula::Formula1 => {
            let info: Vec<SphereInfo> = (0..nv)
                .map(|v| sphere_info(shared(&mp.v_ripflag, "v_ripflag")[v] != 0, mp_xyz(mp, v)))
                .collect();
            nonzero_x = info
                .iter()
                .filter(|i| !i.ripped && i.unnormalized[0] != 0.0)
                .count() as i32;
            for vno in 0..nv {
                if info[vno].ripped {
                    continue;
                }
                make_dist(mp, mris, vno);
                let vtotal = i32::from(mris.topology(vno).vtotal);
                let ds: Vec<f32> = nbrs(mris, vno, vtotal)
                    .into_iter()
                    .map(|n| sphere_distance(&info[vno], &info[n]))
                    .collect();
                store(mp, vno, &ds);
            }
        }
    }
    if nonzero_x == 0 {
        cope_with_logic_problem2(
            g,
            io.env,
            io.out,
            io.err,
            None,
            "all xyz inputs zero - cant fix",
            &extracted_h(),
            232,
            "MRISMP_computeVertexDistancesWkr",
        )?;
    }
    Ok(())
}

fn store(mp: &mut MrisMp, vno: usize, ds: &[f32]) {
    if ds.is_empty() {
        return;
    }
    let d = arr_mut(&mut mp.v_dist, "v_dist")[vno]
        .as_mut()
        .expect("the copy's distances are NULL");
    d[..ds.len()].copy_from_slice(ds);
}

/// A face's values of `MRIScomputeTriangleProperties`.
pub(crate) struct TriangleProps {
    pub area: f32,
    pub norm: [f32; 3],
    pub angle: AnglesPerTriangle,
}

/// `V3_CROSS_PRODUCT(a, b, c)` (`include/matrix.h`): `a.y b.z - a.z b.y`, `a.z b.x - a.x b.z`,
/// `a.x b.y - a.y b.x`, the first product compiled as `b.z a.y` (both instances).
fn v3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        sub(mul(b[2], a[1]), mul(a[2], b[1])),
        sub(mul(a[2], b[0]), mul(a[0], b[2])),
        sub(mul(a[0], b[1]), mul(a[1], b[0])),
    ]
}

/// `VectorTripleProduct(v1, v2, v3)` (`utils/matrix.cpp`, out of line; fs.core's
/// `matrix_ops::vector_triple_product` on three 3-vectors): `x3 (y1 z2 - z1 y2)`, then `+ y3 (z1
/// x2 - x1 z2)`, then `+ z3 (x1 y2 - y1 x2)`.
fn triple_product(v1: [f32; 3], v2: [f32; 3], v3: [f32; 3]) -> f32 {
    let [x1, y1, z1] = v1;
    let [x2, y2, z2] = v2;
    let [x3, y3, z3] = v3;
    let cross = |a: f32, b: f32, c: f32, d: f32| sub(mul(a, b), mul(c, d));
    let mut total = mul(x3, cross(y1, z2, z1, y2));
    total = add(total, mul(y3, cross(z1, x2, x1, z2)));
    add(total, mul(z3, cross(x1, y2, y1, x2)))
}

/// Which instance of `MRIScomputeTriangleProperties_extracted.h`: the surface's
/// (`MRIScomputeTriangleProperties`) or the copy's (`MRISMP_computeTriangleProperties`), whose
/// compiled code differ in one product.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Instance {
    Surface,
    Copy,
}

/// A face's area, unit normal and corner angles as `MRIScomputeTriangleProperties` computes them
/// from its corners: `n = (p1 - p0) x (p2 - p0)` (`V3_CROSS_PRODUCT`), the area half its length
/// (`V3_LEN`, compiled as `sqrt(z² + (x² + y²))`), the normal `n` times `s = 1 / |n|`
/// (`V3_NORMALIZE`: `s` 1 when the length, compiled as `sqrt((y² + x²) + z²)`, is below
/// `FLT_EPSILON`; the third product `n.z s` for the surface's instance, `s n.z` for the copy's); at
/// corner `k` with the edges `a` and `b` to `(p2, p1)`, `(p0, p2)`, `(p1, p0)` for `k` 0, 1, 2,
/// the angle `fastApproxAtan2f((b x a) . normal, a . b)`, the dot product compiled as `a.z b.z +
/// (a.x b.x + a.y b.y)`.
pub(crate) fn triangle_props(
    g: &mut Globals,
    io: &mut Io,
    p: [[f32; 3]; 3],
    instance: Instance,
) -> TriangleProps {
    let [p0, p1, p2] = p;
    let n = v3_cross(diff3(p1, p0), diff3(p2, p0));
    let sq = |x: f32| mul(x, x);
    let area = mul(sqrt(add(sq(n[2]), add(sq(n[0]), sq(n[1])))), 0.5);
    let len = sqrt(add(add(sq(n[1]), sq(n[0])), sq(n[2])));
    let s = if len.abs() < f32::EPSILON {
        1.0
    } else {
        div(1.0, len)
    };
    let nz = match instance {
        Instance::Surface => mul(n[2], s),
        Instance::Copy => mul(s, n[2]),
    };
    let norm = [mul(n[0], s), mul(n[1], s), nz];
    let mut angle = [0.0f32; 3];
    for ano in 0..3 {
        let (vo, va, vb) = match ano {
            0 => (p0, p2, p1),
            1 => (p1, p0, p2),
            _ => (p2, p1, p0),
        };
        let ea = diff3(va, vo);
        let eb = diff3(vb, vo);
        let cross = triple_product(eb, ea, norm);
        let dot = add(mul(ea[2], eb[2]), add(mul(ea[0], eb[0]), mul(ea[1], eb[1])));
        angle[ano] = fast_approx_atan2f(g, io.env, io.out, cross, dot);
    }
    TriangleProps { area, norm, angle }
}

/// A vertex's area of `MRIScomputeTriangleProperties`: the `float` sum of its unripped faces'
/// areas, divided by 3 (2 without `fix_vertex_area`).
pub(crate) fn vertex_face_area(
    sg: &SurfGlobals,
    mris: &Mris,
    vno: usize,
    ripped: &dyn Fn(usize) -> bool,
    area: &dyn Fn(usize) -> f32,
) -> f32 {
    let mut a = 0.0f32;
    for n in 0..usize::from(mris.topology(vno).num) {
        let fno = vface(mris, vno, n);
        if !ripped(fno) {
            a = add(a, area(fno));
        }
    }
    vertex_area_share(sg, a)
}

/// `MRISMP_computeTriangleProperties(mp)`: each unripped face's area, unit normal and angles
/// ([`triangle_props`]); `total_area` the reproducible sum of the areas ([`romp_partials`]) in
/// double, rounded to `float`; each unripped vertex's area ([`vertex_face_area`]).
fn mp_compute_triangle_properties(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    mp: &mut MrisMp,
    mris: &Mris,
) {
    let mut partial = Vec::new();
    for (lo, hi) in romp_partials(0, mp.nfaces) {
        let mut s = 0.0f64;
        for fno in lo as usize..hi as usize {
            if shared(&mp.f_ripflag, "f_ripflag")[fno] != 0 {
                continue;
            }
            let t = triangle_props(g, io, mp_face_xyz(mp, fno), Instance::Copy);
            arr_mut(&mut mp.f_area, "f_area")[fno] = t.area;
            // `reduction_total_area += area` compiled as `area + partial`
            s = add64(f64::from(t.area), s);
            mp_set_face_norm(mp, fno, t.norm);
            arr_mut(&mut mp.f_angle, "f_angle")[fno] = t.angle;
        }
        partial.push(s);
    }
    mp.total_area = romp_total(0.0, &partial) as f32;
    for vno in 0..usz(mp.nvertices) {
        if shared(&mp.v_ripflag, "v_ripflag")[vno] != 0 {
            continue;
        }
        let a = {
            let frip = shared(&mp.f_ripflag, "f_ripflag");
            let fa = arr(&mp.f_area, "f_area");
            vertex_face_area(sg, mris, vno, &|f| frip[f] != 0, &|f| fa[f])
        };
        arr_mut(&mut mp.v_area, "v_area")[vno] = a;
    }
}

/// The mean and the standard deviation of the 1-hop distances between unripped neighbours, as
/// `MRIScomputeAvgInterVertexDist` sums them (reproducible sums of `d`, `d²` and the count, in
/// double) and computes them: `Sum / N`, `sqrt(N (Sum2 / N - Avg²) / (N - 1))`.
pub(crate) fn avg_dist(
    mris: &Mris,
    ripped: &dyn Fn(usize) -> bool,
    dist: &dyn Fn(usize, usize) -> f32,
) -> (f64, f64) {
    let (mut s, mut s2, mut c) = (Vec::new(), Vec::new(), Vec::new());
    for (lo, hi) in romp_partials(0, mris.nvertices) {
        let (mut sum, mut sum2, mut n) = (0.0f64, 0.0f64, 0.0f64);
        for vno in lo as usize..hi as usize {
            if ripped(vno) {
                continue;
            }
            let vnum = i32::from(mris.topology(vno).vnum);
            for (m, vno2) in nbrs(mris, vno, vnum).into_iter().enumerate() {
                if ripped(vno2) {
                    continue;
                }
                let d = f64::from(dist(vno, m));
                sum = add64(sum, d);
                // `Sum2 += d*d` compiled as `d*d + Sum2`
                sum2 = add64(mul64(d, d), sum2);
                n = add64(n, 1.0);
            }
        }
        s.push(sum);
        s2.push(sum2);
        c.push(n);
    }
    let sum = romp_total(0.0, &s);
    let sum2 = romp_total(0.0, &s2);
    let n = romp_total(0.0, &c);
    let avg = div64(sum, n);
    let std = sqrt64(div64(
        mul64(n, sub64(div64(sum2, n), mul64(avg, avg))),
        sub64(n, 1.0),
    ));
    (avg, std)
}

/// `MRISMP_computeAvgInterVertexDist(mp, &mp->std_vertex_dist)`.
fn mp_compute_avg_inter_vertex_dist(mp: &mut MrisMp, mris: &Mris) {
    let (avg, std) = {
        let rip = shared(&mp.v_ripflag, "v_ripflag");
        let d = arr(&mp.v_dist, "v_dist");
        avg_dist(mris, &|v| rip[v] != 0, &|v, m| {
            d[v].as_ref().expect("the copy's distances are NULL")[m]
        })
    };
    mp.std_vertex_dist = std;
    mp.avg_vertex_dist = avg;
}

/// The orientation test of `mrisOrientEllipsoid`: `(x0 + x1 + x2) nx + (y0 + y1 + y2) ny + (z0 +
/// z1 + z2) nz < 0` in `float`.
pub(crate) fn points_inwards(p: [[f32; 3]; 3], n: [f32; 3]) -> bool {
    let c = |k: usize| add(add(p[0][k], p[1][k]), p[2][k]);
    let dot = add(add(mul(c(0), n[0]), mul(c(1), n[1])), mul(c(2), n[2]));
    dot < 0.0
}

/// `mrismp_OrientEllipsoid(mp)`: each unripped face whose normal points inwards
/// ([`points_inwards`]) gets its area, normal and angles negated (times -1); `total_area` the
/// reproducible sum of the non-negative areas, `neg_area` of the negated negative ones,
/// `neg_orig_area` of their faces' original areas, each rounded to `float`.
fn mp_orient_ellipsoid(mp: &mut MrisMp) {
    let (mut ta, mut na, mut noa) = (Vec::new(), Vec::new(), Vec::new());
    for (lo, hi) in romp_partials(0, mp.nfaces) {
        let (mut t, mut n, mut o) = (0.0f64, 0.0f64, 0.0f64);
        for fno in lo as usize..hi as usize {
            if shared(&mp.f_ripflag, "f_ripflag")[fno] != 0 {
                continue;
            }
            let fnorm = arr(&mp.f_norm, "f_norm")[fno];
            if points_inwards(mp_face_xyz(mp, fno), [fnorm.x, fnorm.y, fnorm.z]) {
                // `*= -1.0f` compiled as a sign flip
                let a = &mut arr_mut(&mut mp.f_area, "f_area")[fno];
                *a = -*a;
                let nn = &mut arr_mut(&mut mp.f_norm, "f_norm")[fno];
                (nn.x, nn.y, nn.z) = (-nn.x, -nn.y, -nn.z);
                for x in arr_mut(&mut mp.f_angle, "f_angle")[fno].iter_mut() {
                    *x = -*x;
                }
            }
            let a = arr(&mp.f_area, "f_area")[fno];
            // each term first (`term + partial`)
            if a >= 0.0 {
                t = add64(f64::from(a), t);
            } else {
                n = add64(f64::from(-a), n);
                o = add64(f64::from(shared(&mp.f_norm_orig_area, "")[fno]), o);
            }
        }
        ta.push(t);
        na.push(n);
        noa.push(o);
    }
    mp.total_area = romp_total(0.0, &ta) as f32;
    mp.neg_area = romp_total(0.0, &na) as f32;
    mp.neg_orig_area = romp_total(0.0, &noa) as f32;
}

/// `mrismp_OrientPlane(mp)`: each unripped face whose normal's z is negative gets its area and
/// angles negated (times -1) and its normal negated; `total_area`, `neg_area` and
/// `neg_orig_area` the `float` sums of the non-negative areas, the negated negative ones and their
/// faces' original areas. Each unripped vertex: a negative normal z negated (times -1) with `neg`
/// 1, else `neg` 0; its area the `float` sum of all its faces' areas (ripped ones too) divided by
/// 3 (2 without `fix_vertex_area`): a ripped face's area is the copy's never computed one, memory
/// contents in C, the surface's here (FS-121).
fn mp_orient_plane(g: &mut Globals, sg: &SurfGlobals, mp: &mut MrisMp, mris: &Mris) {
    let mut notes: Vec<()> = Vec::new();
    mp.total_area = 0.0;
    mp.neg_orig_area = 0.0;
    mp.neg_area = 0.0;
    for fno in 0..usz(mp.nfaces) {
        if shared(&mp.f_ripflag, "f_ripflag")[fno] != 0 {
            continue;
        }
        let n = arr(&mp.f_norm, "f_norm")[fno];
        if n.z < 0.0 {
            // `*= -1.0f` compiled as a sign flip
            let a = &mut arr_mut(&mut mp.f_area, "f_area")[fno];
            *a = -*a;
            arr_mut(&mut mp.f_norm, "f_norm")[fno] = FloatXyz {
                x: -n.x,
                y: -n.y,
                z: -n.z,
            };
            for x in arr_mut(&mut mp.f_angle, "f_angle")[fno].iter_mut() {
                *x = -*x;
            }
        }
        let a = arr(&mp.f_area, "f_area")[fno];
        // `total += area` as `area + total`; `neg += -area` as `neg - area`
        if a >= 0.0 {
            mp.total_area = add(a, mp.total_area);
        } else {
            mp.neg_area = sub(mp.neg_area, a);
            let oa = shared(&mp.f_norm_orig_area, "f_norm_orig_area")[fno];
            mp.neg_orig_area = add(mp.neg_orig_area, oa);
        }
    }
    for vno in 0..usz(mp.nvertices) {
        if shared(&mp.v_ripflag, "v_ripflag")[vno] != 0 {
            continue;
        }
        let nz = arr(&mp.v_nz, "v_nz")[vno];
        let neg = if nz < 0.0 {
            arr_mut(&mut mp.v_nz, "v_nz")[vno] = -nz;
            1
        } else {
            0
        };
        arr_mut(&mut mp.v_neg, "v_neg")[vno] = neg;
        let mut area = 0.0f32;
        for n in 0..usize::from(mris.topology(vno).num) {
            let fno = vface(mris, vno, n);
            if shared(&mp.f_ripflag, "f_ripflag")[fno] != 0 {
                notes.push(());
            }
            area = add(area, arr(&mp.f_area, "f_area")[fno]);
        }
        arr_mut(&mut mp.v_area, "v_area")[vno] = vertex_area_share(sg, area);
    }
    if !notes.is_empty() {
        g.notices.add(
            &FS_121,
            "a plane's vertex adds its ripped face's area as the surface holds it, not memory contents",
        );
    }
}

/// Whether a status is oriented as an ellipsoid (`mrisOrientSurface`'s list).
pub(crate) fn orients_as_ellipsoid(s: MrisStatus) -> bool {
    s == MrisStatus::RIGID_BODY
        || s == MrisStatus::PARAMETERIZED_SPHERE
        || s == MrisStatus::SPHERE
        || s == MrisStatus::ELLIPSOID
        || s == MrisStatus::SPHERICAL_PATCH
}

/// `mrismp_OrientSurface(mp)`: a rigid body, sphere (parameterized or not), ellipsoid or
/// spherical patch oriented as an ellipsoid (`MRISMP_updateEllipsoidSurface`), a plane as a plane,
/// any other surface left alone.
fn mp_orient_surface(g: &mut Globals, sg: &SurfGlobals, mp: &mut MrisMp, mris: &Mris) {
    if orients_as_ellipsoid(mp.status) {
        if mp.status != MrisStatus::UNORIENTED_SPHERE {
            mp_orient_ellipsoid(mp);
        }
    } else if mp.status == MrisStatus::PLANE {
        mp_orient_plane(g, sg, mp, mris);
    }
}

/// `M_PI * radius * radius * 4.0`, in double, rounded to `float`.
pub(crate) fn sphere_area(radius: f64) -> f32 {
    mul64(mul64(mul64(std::f64::consts::PI, radius), radius), 4.0) as f32
}

/// Whether a status's `total_area` becomes the sphere's area at the end.
pub(crate) fn takes_sphere_area(s: MrisStatus) -> bool {
    s == MrisStatus::PARAMETERIZED_SPHERE || s == MrisStatus::RIGID_BODY || s == MrisStatus::SPHERE
}

/// `MRIScomputeMetricProperties(MRIS_MP *mp)`: the normals ([`mp_compute_normals`], which may move
/// degenerate vertices), the bounding box, the distances at `nsize` hops (`dist_nsize` becomes
/// `nsize`), the faces' areas, normals and angles, `avg_vertex_area` (`total_area / nvertices` in
/// `float`), the mean distance and its deviation, the orientation; a parameterized sphere's, rigid
/// body's or sphere's `total_area` then `4 pi radius²` ([`sphere_area`]). `mris` is the surface the
/// copy was loaded from (its topology; its stored distance buffers are taken). A vertex whose
/// normal could not be computed keeps the normal the copy had (FS-121).
pub fn compute_metric_properties_mp(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    mp: &mut MrisMp,
    mris: &mut Mris,
) -> Result<(), Fatal> {
    let left = mp_compute_normals(g, sg, io, mp, mris);
    if !left.is_empty() {
        g.notices.add(
            &FS_121,
            "a vertex whose normal could not be computed keeps its normal, not memory contents",
        );
    }
    mp_surface_dimensions(mp);
    let nsize = i32::from(mp.nsize);
    mp_compute_vertex_distances_wkr(g, io, mp, mris, nsize)?;
    mp.dist_nsize = mp.nsize;
    mp_compute_triangle_properties(g, sg, io, mp, mris);
    mp.avg_vertex_area = f64::from(div(mp.total_area, mp.nvertices as f32));
    mp_compute_avg_inter_vertex_dist(mp, mris);
    mp_orient_surface(g, sg, mp, mris);
    if takes_sphere_area(mp.status) {
        mp.total_area = sphere_area(mp.radius);
    }
    Ok(())
}

/// `MRIScomputeMetricProperties(mris)` (`utils/mrisurf_metricProperties.cpp` lines 1844-1881):
/// with `FS_FASTER_MP` set, [`mp_faster::compute_metric_properties_faster`]. Otherwise the call is
/// counted ([`SurfGlobals::mp_calls`]) and, unless only
/// `FREESURFER_OLD_MRIScomputeMetricProperties` is set, the distances freed
/// (`MRISfreeDistsButNotOrig`), the copy loaded ([`mp_load`]) and computed
/// ([`compute_metric_properties_mp`]); with `FREESURFER_OLD_MRIScomputeMetricProperties` set the
/// surface's own computation ([`mp_slow::compute_metric_properties_wkr`]); then the copy unloaded
/// into the surface, or compared with it when both ran ([`mp_unload`]). Returns `NO_ERROR`.
pub fn compute_metric_properties(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    if io.env.get("FS_FASTER_MP").is_some() {
        mp_faster::compute_metric_properties_faster(g, sg, io, mris);
        return Ok(0);
    }
    sg.mp_calls = sg.mp_calls.wrapping_add(1);
    let use_old = io
        .env
        .get("FREESURFER_OLD_MRIScomputeMetricProperties")
        .is_some();
    let use_new = io
        .env
        .get("FREESURFER_NEW_MRIScomputeMetricProperties")
        .is_some()
        || !use_old;
    let mut mp = mp_ctr();
    if use_new {
        dist::free_dists_but_not_orig(mris);
        mp_load(&mut mp, mris, false, None);
        compute_metric_properties_mp(g, sg, io, &mut mp, mris)?;
    }
    if use_old {
        mp_slow::compute_metric_properties_wkr(g, sg, io, mris)?;
    }
    if use_new {
        mp_unload(g, sg, io, mris, &mut mp, use_old)?;
    }
    mp_dtr(&mut mp);
    Ok(0)
}

// ---- the callers that compute the metric properties (3.1-49's and 3.1-51's lines) -------------

/// `MRISsmoothSurface(mris, niter, step)`: [`crate::metric::smooth_surface_positions`], then the
/// metric properties ([`compute_metric_properties`]).
pub fn smooth_surface(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    niter: i32,
    step: f32,
) -> Result<(), Fatal> {
    crate::metric::smooth_surface_positions(g, io, mris, niter, step)?;
    compute_metric_properties(g, sg, io, mris)?;
    Ok(())
}

/// `mrisDisturbVertices(mris, amount)`: the distances freed; each vertex's x, then y, moved by
/// `randomNumber(-amount, amount)` (in double, rounded to `float`); the metric properties.
pub fn disturb_vertices(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    amount: f64,
) -> Result<(), Fatal> {
    dist::free_dists_but_not_orig(mris);
    for vno in 0..usz(mris.nvertices) {
        for k in 0..2 {
            let r = larmorx_freesurfer_core::random::random_number(
                g, io.env, io.out, io.err, -amount, amount,
            )?;
            let v = mris.vertex_mut(vno);
            let c = if k == 0 { &mut v.x } else { &mut v.y };
            *c = add64(f64::from(*c), r) as f32;
        }
    }
    compute_metric_properties(g, sg, io, mris)?;
    Ok(())
}

/// `MRISrecenter(mris, which_move, which_target)`: the metric properties; the radius
/// `sqrt(total_area / 4 pi)`; the `which_move` coordinates saved and the `which_target` ones
/// restored into the current ones; `MRIScenterSphere`; `rescaling canonical coordinates to use
/// r=<radius %2.1f>`; each unripped vertex's current coordinates scaled to the radius by its
/// `which_target` coordinates' length (`radius (x / r)` in double); the mean of `target - move`
/// over the unripped vertices added to their canonical coordinates (a target other than the
/// canonical coordinates: `ErrorExit(ERROR_UNSUPPORTED, "MRISrecenter: movable vertices
/// <which_move> not supported")` at the first unripped vertex); the `which_move` coordinates
/// restored; the metric properties.
pub fn recenter(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    which_move: i32,
    which_target: i32,
) -> Result<(), Fatal> {
    use crate::views::{CANONICAL_VERTICES, vertex_coord2xyz_double};
    compute_metric_properties(g, sg, io, mris)?;
    let radius = sqrt64(div64(
        f64::from(mris.total_area),
        4.0 * std::f64::consts::PI,
    ));
    crate::positions::save_vertex_positions(g, io, mris, which_move)?;
    crate::positions::restore_vertex_positions(g, io, mris, which_target)?;
    crate::metric::center_sphere(g, io, mris)?;
    let _ = writeln!(
        io.out,
        "rescaling canonical coordinates to use r={}",
        fmt_f(radius, 2, 1)
    );
    for vno in 0..usz(mris.nvertices) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let (xt, yt, zt) = vertex_coord2xyz_double(g, io.err, mris.vertex(vno), which_target)?;
        let r = sqrt64(add64(add64(mul64(xt, xt), mul64(yt, yt)), mul64(zt, zt)));
        let v = mris.vertex_mut(vno);
        v.x = mul64(radius, div64(f64::from(v.x), r)) as f32;
        v.y = mul64(radius, div64(f64::from(v.y), r)) as f32;
        v.z = mul64(radius, div64(f64::from(v.z), r)) as f32;
    }
    let (mut tx, mut ty, mut tz) = (0.0f64, 0.0f64, 0.0f64);
    let mut n = 0i32;
    for vno in 0..usz(mris.nvertices) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let (xm, ym, zm) = vertex_coord2xyz_double(g, io.err, mris.vertex(vno), which_move)?;
        let (xt, yt, zt) = vertex_coord2xyz_double(g, io.err, mris.vertex(vno), which_target)?;
        n += 1;
        tx = add64(tx, sub64(xt, xm));
        ty = add64(ty, sub64(yt, ym));
        tz = add64(tz, sub64(zt, zm));
    }
    let nf = f64::from(n);
    (tx, ty, tz) = (div64(tx, nf), div64(ty, nf), div64(tz, nf));
    for vno in 0..usz(mris.nvertices) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        if which_target != CANONICAL_VERTICES {
            larmorx_freesurfer_core::log::error_exit(
                g,
                io.err,
                larmorx_freesurfer_core::log::ERROR_UNSUPPORTED,
                &format!("MRISrecenter: movable vertices {which_move} not supported"),
            )?;
        }
        let v = mris.vertex_mut(vno);
        v.cx = add64(f64::from(v.cx), tx) as f32;
        v.cy = add64(f64::from(v.cy), ty) as f32;
        v.cz = add64(f64::from(v.cz), tz) as f32;
    }
    crate::positions::restore_vertex_positions(g, io, mris, which_move)?;
    compute_metric_properties(g, sg, io, mris)?;
    Ok(())
}

/// `MRISscaleVertexCoordinates(mris, scale)`: `MRISscale`.
pub fn scale_vertex_coordinates(g: &mut Globals, mris: &mut Mris, scale: f64) -> i32 {
    crate::coords::scale(g, mris, scale)
}

/// `MRISscaleBrain(mris, mris, scale)` (the source and destination one surface, as FreeSurfer
/// asserts): `MRISanisotropicScale` by `scale` on each axis.
pub fn scale_brain(mris: &mut Mris, scale: f32) {
    crate::coords::anisotropic_scale(mris, scale, scale, scale);
}

/// `MRISscaleBrainArea(mris)`: the surface scaled by `sqrt(orig_area / (total_area + neg_area))`
/// (in `float`; [`scale_brain`]), then the metric properties. Returns `NO_ERROR`.
pub fn scale_brain_area(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    let scale = sqrt(div(mris.orig_area, add(mris.total_area, mris.neg_area)));
    scale_brain(mris, scale);
    compute_metric_properties(g, sg, io, mris)?;
    Ok(0)
}
