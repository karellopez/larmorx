// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The rest of queue item 3.1-51 (relay 33 of track D): `utils/mrisurf_metricProperties.cpp`'s
//! `MRISbinarizeVolume` (11773), `MRISmaskLabel` (12395), `MRISmaskNotLabel` (12407) and
//! `MRIcomputeLaminarVolumeFractions` (13131); `utils/mris_fastmarching.cpp`'s `mriFindLabel` and
//! `MRISextractOutsideDistanceMap`, which builds on the first; and `utils/mrisurf_topology.cpp`'s
//! `findOtherEdgeFace` (1970), which the first calls. `MRISmarkEdge` is in `props10.rs`, beside
//! the edge statistics.
//!
//! `MRISbinarizeVolume`'s arithmetic is the official build's (its object code): single precision
//! throughout, `sqrtf`, the bounding boxes' `MIN`/`MAX` as `macros.h` defines them (`a < b ? a :
//! b`, `a >= b ? a : b`), each box index `(int)(xsize (x - xstart) + 0.5)` with the `+ 0.5` in
//! `double` and the window `delta` subtracted and added with wrapping `int` arithmetic. A NaN
//! never reaches the volume (a voxel takes a distance only when its magnitude is below the one it
//! holds), so its bits do not matter here.
//!
//! D13 (larmorx_core::defects): FS-609 and FS-610 (the outside distance map's label volume and its
//! message), FS-611 and FS-612 (the label masks), FS-613 (the laminar fractions' white interior).

use larmorx_core::defects::{FS_610, FS_611, FS_612, FS_613};
use larmorx_freesurfer_core::diag::DIAG_VERBOSE;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, Fatal};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_filter::fastmarch::{label_matches, outside_match};
use larmorx_freesurfer_io::label::Label;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, Mri, MriRegion, mri_alloc, mri_alloc_sequence, mri_copy_header,
};
use larmorx_freesurfer_volume::arith::{Pair, Tri, mri_clone};
use larmorx_freesurfer_volume::combine::mri_compute_volume_fractions;
use larmorx_freesurfer_volume::coords::voxel_to_surface_ras;
use larmorx_freesurfer_volume::mriset::{
    mri_and, mri_copy_label, mri_not, mri_replace_values_only,
};
use larmorx_freesurfer_volume::orient::mri_get_voxel_to_voxel_xform;
use larmorx_freesurfer_volume::resize::mri_upsample_n;

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::discrete::compute_vertex_pseudo_normal;
use crate::facenorm::{compute_defect_face_normal, get_face_norm};
use crate::mp::compute_metric_properties;
use crate::mris::Mris;
use crate::positions::{restore_vertex_positions, save_vertex_positions};
use crate::views::{PIAL_VERTICES, TMP2_VERTICES, WHITE_VERTICES};

/// `NOT_PROCESSED_YET` (`NPY`): a voxel no face has reached.
const NPY: f32 = 1000.0;

/// C's `(int)` of a `float` (`cvttss2si`): truncated, `INT_MIN` for a NaN and beyond `int`.
fn int32(v: f32) -> i32 {
    if v.is_nan() || v >= 2_147_483_648.0 || v < -2_147_483_648.0 {
        i32::MIN
    } else {
        v as i32
    }
}

/// C's `(int)` of a `double` (`cvttsd2si`), as [`int32`].
fn int64(v: f64) -> i32 {
    if v.is_nan() || v >= 2_147_483_648.0 || v <= -2_147_483_649.0 {
        i32::MIN
    } else {
        v.trunc() as i32
    }
}

/// `MIN(a, b)` (`macros.h`): `a < b ? a : b`.
fn min(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// `MAX(a, b)` (`macros.h`): `a >= b ? a : b`.
fn max(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `iVOL(mri, x)`: `(int)(xsize (x - xstart) + 0.5)`, the product in `float`, the sum in `double`.
fn ivol(size: f32, start: f32, x: f32) -> i32 {
    int64(f64::from(size * (x - start)) + 0.5)
}

/// `SIGN(x) * m` (`mrisurf_base.h`: `x > 0 ? 1.0 : -1.0`; the product in `double`, exact).
fn signed(sign: f32, m: f32) -> f32 {
    if sign > 0.0 { m } else { -m }
}

fn dot(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sqr3(a: &[f32; 3]) -> f32 {
    a[0] * a[0] + a[1] * a[1] + a[2] * a[2]
}

/// `F_CROSS(a, b, d)`: `d0 = a1 b2 - b1 a2`, `d1 = a2 b0 - b2 a0`, `d2 = a0 b1 - b0 a1`.
fn cross(a: &[f32; 3], b: &[f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - b[1] * a[2],
        a[2] * b[0] - b[2] * a[0],
        a[0] * b[1] - b[0] * a[1],
    ]
}

fn nfaces(mris: &Mris) -> usize {
    usize::try_from(mris.nfaces).unwrap_or(0)
}

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// The face numbers around vertex `vno` (`f[0..num]`).
fn vertex_faces(mris: &Mris, vno: usize) -> &[i32] {
    let vt = mris.topology(vno);
    let num = usize::from(vt.num);
    vt.f.as_deref().map_or(&[], |f| &f[..num.min(f.len())])
}

/// `findOtherEdgeFace(mris, fno, vno, vn1)`: the first face of `vno`'s list other than `fno` that
/// `vn1`'s list also holds; `fno` itself when there is none (an edge of one face).
pub fn find_other_edge_face(mris: &Mris, fno: i32, vno: i32, vn1: i32) -> i32 {
    let f2 = vertex_faces(mris, vn1 as usize);
    for &a in vertex_faces(mris, vno as usize) {
        if a == fno {
            continue;
        }
        if f2.contains(&a) {
            return a;
        }
    }
    fno
}

/// One edge of a face for the distance: its corner, the edge vector, the in-plane normal
/// (`n_f x e`), the edge's pseudo-normal (the two faces' normals summed).
struct EdgeParts {
    e: [f32; 3],
    n: [f32; 3],
    ne: [f32; 3],
}

/// The distance from `vec_a` (the point less the edge's first corner) to an edge, as
/// `MRISbinarizeVolume` takes it for a point on the edge's outer side: the first corner when the
/// projection falls before it, the edge when within, the second corner (`vec_b`) after it; each
/// signed by the matching pseudo-normal and the smaller magnitude kept with `distance`'s.
fn edge_distance(
    distance: f32,
    vec_a: &[f32; 3],
    vec_b: &[f32; 3],
    p: &EdgeParts,
    nv_a: &[f32; 3],
    nv_b: &[f32; 3],
) -> f32 {
    let val = dot(vec_a, &p.e);
    if val < 0.0 {
        let sign = dot(nv_a, vec_a);
        signed(sign, min(distance.abs(), sqr3(vec_a).sqrt()))
    } else if val < sqr3(&p.e) {
        let sign = dot(&p.ne, vec_a);
        let d = sqr3(vec_a) - val * val / sqr3(&p.e);
        signed(sign, min(distance.abs(), max(0.0, d).sqrt()))
    } else {
        let sign = dot(nv_b, vec_b);
        signed(sign, min(distance.abs(), sqr3(vec_b).sqrt()))
    }
}

/// `MRISbinarizeVolume(mris, region, resolution, distance_from_surface)`: the signed distance of
/// the voxels near the surface, in a float volume over `region` (a box in tkregister RAS:
/// `x, y, z` its low corner, `dx, dy, dz` its size in mm). `resolution` below 1 is taken as 1 and
/// `distance_from_surface` below 2 as 2 (a NaN kept); `delta = (int)(distance / resolution)`, at
/// least 2. The volume: `(int) ceilf(resolution * d)` voxels per axis (`MRIalloc`, so its own
/// messages), printed on standard error as `mri volume size : <w> by <h> by <d> (resolution =
/// <(int) resolution>)`; its `xstart` etc. the region's corner and its `xsize` etc. `resolution`
/// (voxels per mm: a voxel `i` is at `xstart + i / xsize`); every voxel 1000 (`NOT_PROCESSED_YET`).
/// Every face's normal is computed and stored (`computeDefectFaceNormal`). Then for each face,
/// over the voxels of its corners' bounding box widened by `delta` (clamped to the volume): with
/// `vec_k` the voxel's point less corner `k`, the edges `e0 = v1 - v0`, `e1 = v2 - v1`, `e2 = v0 -
/// v2`, `n_k = n_f x e_k` (`n_f` the face's normal), the edges' pseudo-normals (the face's normal
/// plus the other face's of the edge, itself when it has none) and the corners' (the angle-weighted
/// sums of `computeVertexPseudoNormal`): when `vec_k . n_k >= 0` for all three the distance is `n_f
/// . vec` (`vec` the mean of the three); otherwise, from 1000, for each edge with `vec_k . n_k <= 0`
/// in turn, the distance to that edge (its first corner, its segment, or its second corner,
/// whichever the projection reaches) signed by the matching pseudo-normal (`SIGN`: positive only
/// above 0) and the smaller of its magnitude and the previous one's kept. The voxel keeps the
/// distance when its magnitude is below the magnitude it holds.
pub fn mris_binarize_volume(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    region: &MriRegion,
    resolution: f32,
    distance_from_surface: f32,
) -> Result<Mri, Fatal> {
    let resolution = if resolution < 1.0 { 1.0 } else { resolution };
    let distance_from_surface = if distance_from_surface < 2.0 {
        2.0
    } else {
        distance_from_surface
    };
    let mut delta = int32(distance_from_surface / resolution);
    if delta < 2 {
        delta = 2;
    }
    let width = int32((region.dx as f32 * resolution).ceil());
    let height = int32((region.dy as f32 * resolution).ceil());
    let depth = int32((region.dz as f32 * resolution).ceil());
    let mut m = mri_alloc(g, io.out, io.err, width, height, depth, MRI_FLOAT)?;
    let _ = writeln!(
        io.err,
        "mri volume size : {width} by {height} by {depth} (resolution = {})",
        int32(resolution)
    );
    m.xstart = region.x as f32;
    m.vg.xsize = resolution;
    m.ystart = region.y as f32;
    m.vg.ysize = resolution;
    m.zstart = region.z as f32;
    m.vg.zsize = resolution;
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    let n = (w.max(0) as usize) * (h.max(0) as usize) * (d.max(0) as usize);
    let mut vals = vec![NPY; n];
    for fno in 0..nfaces(mris) {
        compute_defect_face_normal(mris, fno);
    }
    let (sx, sy, sz) = (resolution, resolution, resolution);
    let (x0s, y0s, z0s) = (m.xstart, m.ystart, m.zstart);
    for fno in 0..nfaces(mris) {
        let fv = mris.face(fno).v;
        let p = |k: usize| {
            let v = mris.vertex(fv[k] as usize);
            [v.x, v.y, v.z]
        };
        let (c0, c1, c2) = (p(0), p(1), p(2));
        let lo = |k: usize| min(c0[k], min(c1[k], c2[k]));
        let hi = |k: usize| max(c0[k], max(c1[k], c2[k]));
        let imin = ivol(sx, x0s, lo(0)).wrapping_sub(delta);
        let imax = ivol(sx, x0s, hi(0)).wrapping_add(delta);
        let jmin = ivol(sy, y0s, lo(1)).wrapping_sub(delta);
        let jmax = ivol(sy, y0s, hi(1)).wrapping_add(delta);
        let kmin = ivol(sz, z0s, lo(2)).wrapping_sub(delta);
        let kmax = ivol(sz, z0s, hi(2)).wrapping_add(delta);
        let clamp_lo = |a: i32| if a >= 0 { a } else { 0 };
        let clamp_hi = |a: i32, top: i32| if a < top { a } else { top };
        let (imin, imax) = (clamp_lo(imin), clamp_hi(imax, w - 1));
        let (jmin, jmax) = (clamp_lo(jmin), clamp_hi(jmax, h - 1));
        let (kmin, kmax) = (clamp_lo(kmin), clamp_hi(kmax, d - 1));
        let nf = get_face_norm(mris, fno);
        let n_f = [nf.nx, nf.ny, nf.nz];
        let fnoi = fno as i32;
        let mut edge = |a: &[f32; 3], b: &[f32; 3], va: i32, vb: i32| {
            let e = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let n = cross(&n_f, &e);
            let fn1 = find_other_edge_face(mris, fnoi, va, vb);
            let o = get_face_norm(mris, fn1 as usize);
            let ne = [nf.nx + o.nx, nf.ny + o.ny, nf.nz + o.nz];
            EdgeParts { e, n, ne }
        };
        let p0 = edge(&c0, &c1, fv[0], fv[1]);
        let p1 = edge(&c1, &c2, fv[1], fv[2]);
        let p2 = edge(&c2, &c0, fv[2], fv[0]);
        let nv0 = compute_vertex_pseudo_normal(g, io, mris, fv[0] as usize, 0)?;
        let nv1 = compute_vertex_pseudo_normal(g, io, mris, fv[1] as usize, 0)?;
        let nv2 = compute_vertex_pseudo_normal(g, io, mris, fv[2] as usize, 0)?;
        let mut k = kmin;
        while k <= kmax {
            let z = z0s + k as f32 / sz;
            let mut j = jmin;
            while j <= jmax {
                let y = y0s + j as f32 / sy;
                let mut i = imin;
                while i <= imax {
                    let x = x0s + i as f32 / sx;
                    let vec0 = [x - c0[0], y - c0[1], z - c0[2]];
                    let vec1 = [x - c1[0], y - c1[1], z - c1[2]];
                    let vec2 = [x - c2[0], y - c2[1], z - c2[2]];
                    let val0 = dot(&vec0, &p0.n);
                    let val1 = dot(&vec1, &p1.n);
                    let val2 = dot(&vec2, &p2.n);
                    let distance = if val0 >= 0.0 && val1 >= 0.0 && val2 >= 0.0 {
                        let vec = [0, 1, 2].map(|c| (vec0[c] + vec1[c] + vec2[c]) / 3.0);
                        dot(&n_f, &vec)
                    } else {
                        let mut distance = NPY;
                        if val0 <= 0.0 {
                            distance = edge_distance(distance, &vec0, &vec1, &p0, &nv0, &nv1);
                        }
                        if val1 <= 0.0 {
                            distance = edge_distance(distance, &vec1, &vec2, &p1, &nv1, &nv2);
                        }
                        if val2 <= 0.0 {
                            distance = edge_distance(distance, &vec2, &vec0, &p2, &nv2, &nv0);
                        }
                        distance
                    };
                    let at = ((k as usize * h as usize) + j as usize) * w as usize + i as usize;
                    if distance.abs() < vals[at].abs() {
                        vals[at] = distance;
                    }
                    i += 1;
                }
                j += 1;
            }
            k += 1;
        }
    }
    if let Some(c) = m.chunk.as_mut() {
        for (b, v) in c[..4 * n].as_chunks_mut::<4>().0.iter_mut().zip(&vals) {
            *b = v.to_le_bytes();
        }
    }
    Ok(m)
}

/// The vertex a label point names, when the surface has it (FS-611: FreeSurfer writes outside
/// the surface's vertices otherwise; larmorx leaves the point out in both modes, noted).
fn label_vertex(g: &mut Globals, what: &str, mris: &Mris, area: &Label, i: usize) -> Option<usize> {
    let vno = area.lv.get(i).map_or(-1, |p| p.vno);
    if vno >= 0 && vno < mris.nvertices {
        return Some(vno as usize);
    }
    g.notices.add(
        &FS_611,
        format!(
            "{what}: label point {i} names vertex {vno}, outside the surface's {} vertices (FreeSurfer writes outside them; left out)",
            mris.nvertices
        ),
    );
    None
}

fn label_points(area: &Label) -> usize {
    usize::try_from(area.n_points).unwrap_or(0)
}

fn zero_values(mris: &mut Mris, vno: usize) {
    let v = mris.vertex_mut(vno);
    v.curv = 0.0;
    v.stat = 0.0;
    v.val = 0.0;
    v.imag_val = 0.0;
    v.val2 = 0.0;
    v.valbak = 0.0;
    v.val2bak = 0.0;
}

/// `MRISmaskLabel(mris, area)`: the values (`curv`, `stat`, `val`, `imag_val`, `val2`, `valbak`,
/// `val2bak`) of the label's vertices set to 0 (FS-611). Returns `NO_ERROR`.
pub fn mris_mask_label(g: &mut Globals, mris: &mut Mris, area: &Label) -> i32 {
    for i in 0..label_points(area) {
        if let Some(vno) = label_vertex(g, "MRISmaskLabel", mris, area, i) {
            zero_values(mris, vno);
        }
    }
    0
}

/// `MRISmaskNotLabel(mris, area)`: the label's vertices marked, the values (as
/// [`mris_mask_label`]) of every unmarked vertex set to 0, then the label's vertices' marks
/// cleared (FS-611). FS-612: a vertex marked before the call keeps its values, and a label vertex
/// marked before loses its mark (kept: rule C; noted). Returns `NO_ERROR`.
pub fn mris_mask_not_label(g: &mut Globals, mris: &mut Mris, area: &Label) -> i32 {
    let mut inside = vec![false; nverts(mris)];
    for i in 0..label_points(area) {
        if let Some(vno) = label_vertex(g, "MRISmaskNotLabel", mris, area, i) {
            inside[vno] = true;
        }
    }
    let mut kept = 0usize;
    let mut cleared = 0usize;
    for (vno, &l) in inside.iter().enumerate() {
        let marked = mris.vertex(vno).marked != 0;
        kept += usize::from(marked && !l);
        cleared += usize::from(marked && l);
    }
    if kept + cleared > 0 {
        g.notices.add(
            &FS_612,
            format!(
                "MRISmaskNotLabel: {kept} vertices outside the label were marked before the call and keep their values; {cleared} label vertices marked before lose their marks (kept)"
            ),
        );
    }
    for (vno, &l) in inside.iter().enumerate() {
        if l {
            mris.vertex_mut(vno).marked = 1;
        }
    }
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).marked != 0 {
            continue;
        }
        zero_values(mris, vno);
    }
    for (vno, &l) in inside.iter().enumerate() {
        if l {
            mris.vertex_mut(vno).marked = 0;
        }
    }
    0
}

/// `WM_VAL`, `GM_VAL`, `CSF_VAL`.
const GM_VAL: f32 = 2.0;
const CSF_VAL: f32 = 3.0;

/// `MRIcomputeLaminarVolumeFractions(mris, resolution, mri_src, mri_fractions)`: the fractions of
/// white matter, grey matter and CSF in each voxel of `src`, from the surface's white and pial
/// positions (`WHITE_VERTICES`, `PIAL_VERTICES`). The current positions are saved
/// (`TMP2_VERTICES`); `nvox = nint(MAX(MAX(xsize / resolution, ysize / resolution), zsize /
/// resolution))` (printed as `packing each voxel with <nvox^3> voxels to compute laminar
/// fractions` with `Gdiag`'s verbose bit); the source upsampled `nvox` times (`MRIupsampleN`) into
/// the layer volume; the white positions' interior (`MRISfillInterior` at `resolution`) into it
/// (1, else 0), the pial positions' interior into a clone; the voxels inside the pial and not the
/// white surface set to 2, the rest of the voxels that are 0 set to 3; then the fractions (three
/// frames) from the layer volume (`MRIcomputeVolumeFractions` with the vox2vox matrix from it to
/// the source) into `mri_fractions` or a new float volume of the source's extent with its header.
/// The current positions are restored and the metric properties recomputed. FS-613: FreeSurfer
/// computes the white interior at its first call only and copies it into every later call's layer
/// volume (`MRIcopy`); larmorx computes each call's own (`reproduce` keeps the first,
/// [`SurfGlobals::laminar_wm`]).
pub fn mri_compute_laminar_volume_fractions(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    resolution: f64,
    src: &Mri,
    mri_fractions: Option<&mut Mri>,
) -> Result<Option<Mri>, Fatal> {
    save_vertex_positions(g, io, mris, TMP2_VERTICES)?;
    let r = |s: f32| f64::from(s) / resolution;
    let (a, b, c) = (r(src.vg.xsize), r(src.vg.ysize), r(src.vg.zsize));
    let ab = if a >= b { a } else { b };
    let nvox = nint(g, if ab >= c { ab } else { c });
    if g.gdiag & DIAG_VERBOSE != 0 {
        let cube = f64::from(nvox) * f64::from(nvox) * f64::from(nvox);
        let _ = writeln!(
            io.out,
            "packing each voxel with {} voxels to compute laminar fractions",
            int64(cube)
        );
    }
    let mut layers = mri_upsample_n(g, io.out, io.err, src, None, nvox)?.unwrap_new();
    let mut pial = mri_clone(g, io.out, io.err, &layers, None)?.unwrap_new();
    if sg.laminar_wm.is_some() {
        g.notices.add(
            &FS_613,
            "MRIcomputeLaminarVolumeFractions: a later call (FreeSurfer reuses the first call's white interior; larmorx computes this call's own)",
        );
    }
    if g.notices.reproduce() && sg.laminar_wm.is_some() {
        let wm = sg.laminar_wm.take().expect("the first call's interior");
        mri_copy(g, io.out, io.err, &wm, Some(&mut layers))?;
        sg.laminar_wm = Some(wm);
    } else {
        restore_vertex_positions(g, io, mris, WHITE_VERTICES)?;
        compute_metric_properties(g, sg, io, mris)?;
        crate::mriflood::mris_fill_interior(g, sg, io, mris, resolution, Some(&mut layers))?;
        if sg.laminar_wm.is_none() {
            sg.laminar_wm = Some(mri_copy(g, io.out, io.err, &layers, None)?.unwrap_new());
        }
    }
    restore_vertex_positions(g, io, mris, PIAL_VERTICES)?;
    compute_metric_properties(g, sg, io, mris)?;
    crate::mriflood::mris_fill_interior(g, sg, io, mris, resolution, Some(&mut pial))?;
    let mut tmp = mri_not(g, io.out, io.err, Pair::New(&layers))?.unwrap_new();
    mri_and(g, io.out, io.err, Tri::Second(&pial, &mut tmp), 1)?;
    mri_replace_values_only(g, io.out, io.err, Pair::Same(&mut tmp), 1.0, GM_VAL)?;
    mri_copy_label(g, io.err, &tmp, &mut layers, GM_VAL as i32)?;
    drop(tmp);
    mri_replace_values_only(g, io.out, io.err, Pair::Same(&mut layers), 0.0, CSF_VAL)?;
    let vox2vox = mri_get_voxel_to_voxel_xform(g, io.out, io.err, &layers, src)?;
    let mut fresh = None;
    let fractions: &mut Mri = match mri_fractions {
        Some(f) => f,
        None => {
            let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
            let mut f = mri_alloc_sequence(g, io.out, io.err, w, h, d, MRI_FLOAT, 3)?;
            mri_copy_header(g, io.out, io.err, src, Some(&mut f))?;
            fresh.insert(f)
        }
    };
    // MatrixMultiply ends the program rather than give no matrix
    let vox2vox = vox2vox.expect("the voxel-to-voxel matrix");
    mri_compute_volume_fractions(g, io.out, io.err, src, &vox2vox, &layers, fractions)?;
    drop(layers);
    drop(pial);
    restore_vertex_positions(g, io, mris, TMP2_VERTICES)?;
    compute_metric_properties(g, sg, io, mris)?;
    Ok(fresh)
}

/// `mriFindLabel(mri, label, offset)`: the box in tkregister RAS around the label's voxels
/// (`MRIvoxelToSurfaceRAS` of each; FS-609 for a volume not of bytes), widened by `offset`:
/// `x = (int) floor(xmin - offset)`, `dx = (int) ceil(xmax - xmin + 2 offset)` per axis (in
/// `double`). No voxel of the label ends the program, `No labels <n> could be found in the volume`
/// (status 1; FS-610: FreeSurfer names the count, 0, larmorx the label).
pub fn mri_find_label(
    g: &mut Globals,
    io: &mut Io,
    mri: &mut Mri,
    label: i32,
    offset: i32,
) -> Result<MriRegion, Fatal> {
    let (mut xmin, mut ymin, mut zmin) = (100000.0f64, 100000.0f64, 100000.0f64);
    let (mut xmax, mut ymax, mut zmax) = (-100000.0f64, -100000.0f64, -100000.0f64);
    let mut nlabels = 0i32;
    const WHAT: &str = "MRISextractOutsideDistanceMap";
    for k in 0..mri.vg.depth {
        for j in 0..mri.vg.height {
            for i in 0..mri.vg.width {
                if !label_matches(g, io.err, WHAT, mri, i, j, k, label)? {
                    continue;
                }
                nlabels += 1;
                let (_, [xw, yw, zw]) = voxel_to_surface_ras(
                    g,
                    io.out,
                    io.err,
                    &mut mri.vg,
                    f64::from(i),
                    f64::from(j),
                    f64::from(k),
                )?;
                if xmin > xw {
                    xmin = xw;
                }
                if ymin > yw {
                    ymin = yw;
                }
                if zmin > zw {
                    zmin = zw;
                }
                if xmax < xw {
                    xmax = xw;
                }
                if ymax < yw {
                    ymax = yw;
                }
                if zmax < zw {
                    zmax = zw;
                }
            }
        }
    }
    if nlabels == 0 {
        g.notices.add(
            &FS_610,
            format!("MRISextractOutsideDistanceMap: no voxel of label {label} (FreeSurfer's message names 0)"),
        );
        let named = if g.notices.reproduce() {
            nlabels
        } else {
            label
        };
        log::error_exit(
            g,
            io.err,
            1,
            &format!("No labels {named} could be found in the volume\n"),
        )?;
    }
    let off = f64::from(offset);
    Ok(MriRegion {
        x: int64((xmin - off).floor()),
        y: int64((ymin - off).floor()),
        z: int64((zmin - off).floor()),
        dx: int64((xmax - xmin + 2.0 * off).ceil()),
        dy: int64((ymax - ymin + 2.0 * off).ceil()),
        dz: int64((zmax - zmin + 2.0 * off).ceil()),
    })
}

/// `MRISextractOutsideDistanceMap(mris, mri_src, label, offset, resolution, max_distance)`: each
/// vertex's `curv` set to its distance outside the surface within the label of `src` (a byte
/// volume), scaled to `[0, 1]` by `max_distance`. `resolution` below 1 is 1, `max_distance` below 1
/// is 1, `offset` below 0 is 0 (a NaN kept). The label's box ([`mri_find_label`]), the surface's
/// signed distances in it ([`mris_binarize_volume`] with 5 mm), a float volume of that extent with
/// its header (`MRIclone`, 0), the fast march to the limit `2 resolution max_distance` from the
/// surface's outer side within the label (FreeSurfer's `FastMarching<+1>` with
/// `InitForOutsideMatch`: `larmorx_freesurfer_filter::fastmarch::outside_match`); the voxels whose
/// distance is below 0 set to -0.1. Then for each vertex, `curv` 1 unless, at `t = 0, 0.1, ...,
/// 0.4` from the white towards the pial position (`(1 - t) white + t pial` per axis in `float`, its
/// voxel `iVOL`), the first voxel inside the volume whose march value over `resolution` is at least
/// 0 gives `MIN(value, max_distance) / max_distance`.
#[allow(clippy::too_many_arguments)]
pub fn mris_extract_outside_distance_map(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    src: &mut Mri,
    label: i32,
    offset: i32,
    resolution: f32,
    max_distance: f32,
) -> Result<(), Fatal> {
    let resolution = if resolution < 1.0 { 1.0 } else { resolution };
    let max_distance = if max_distance < 1.0 {
        1.0
    } else {
        max_distance
    };
    let offset = offset.max(0);
    let region = mri_find_label(g, io, src, label, offset)?;
    let distance = mris_binarize_volume(g, io, mris, &region, resolution, 5.0)?;
    let mut fm = mri_clone(g, io.out, io.err, &distance, None)?.unwrap_new();
    let n = (fm.vg.width.max(0) as usize)
        * (fm.vg.height.max(0) as usize)
        * (fm.vg.depth.max(0) as usize);
    if let Some(c) = fm.chunk.as_mut() {
        c[..4 * n].fill(0);
    }
    let limit = 2.0 * resolution * max_distance;
    outside_match(g, io.out, io.err, &mut fm, &distance, src, label, limit)?;
    let dvals: Vec<f32> = distance.chunk.as_ref().expect("voxels")[..4 * n]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    let fc = fm.chunk.as_mut().expect("voxels");
    for (k, &dv) in dvals.iter().enumerate() {
        if dv < 0.0 {
            fc[4 * k..4 * k + 4].copy_from_slice(&(-0.1f32).to_le_bytes());
        }
    }
    let (w, h, d) = (fm.vg.width, fm.vg.height, fm.vg.depth);
    let (sx, sy, sz) = (fm.vg.xsize, fm.vg.ysize, fm.vg.zsize);
    let (x0, y0, z0) = (fm.xstart, fm.ystart, fm.zstart);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        let (wh, pl) = ([v.whitex, v.whitey, v.whitez], [v.pialx, v.pialy, v.pialz]);
        let mut curv = 1.0f32;
        for step in 0..5 {
            let t = (f64::from(step) * 0.1) as f32;
            let at = |c: usize| (1.0 - t) * wh[c] + t * pl[c];
            let i = ivol(sx, x0, at(0));
            let j = ivol(sy, y0, at(1));
            let k = ivol(sz, z0, at(2));
            if i < 0 || i > w - 1 || j < 0 || j > h - 1 || k < 0 || k > d - 1 {
                continue;
            }
            let o = 4 * (((k as usize * h as usize) + j as usize) * w as usize + i as usize);
            let val = f32::from_le_bytes(fc[o..o + 4].try_into().expect("4 bytes")) / resolution;
            if val >= 0.0 {
                curv = min(val, max_distance) / max_distance;
                break;
            }
        }
        mris.vertex_mut(vno).curv = curv;
    }
    Ok(())
}
