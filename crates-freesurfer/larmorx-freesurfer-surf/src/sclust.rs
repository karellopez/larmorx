// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A surface's clusters: FreeSurfer 8.2.0's `utils/surfcluster.cpp` whole (fs.surf's piece, taken
//! early for `label.cpp`'s `LabelRemoveIslandsSurf`): `sclustMapSurfClusters` (line 52),
//! `sclustGrowSurfCluster` (118), `sclustSurfaceArea` (147), `sclustWeight` (204),
//! `sclustSurfaceMax` (235), `sclustSurfaceCentroid` (264), `sclustZeroSurfaceClusterNo` (294),
//! `sclustZeroSurfaceNonClusters` (311), `sclustSetSurfaceValToClusterNo` (327),
//! `sclustSetSurfaceValToCWP` (341), `sclustCountClusters` (369), `SurfClusterSummary` (385),
//! `SurfClusterSummaryOld` (534), `DumpSurfClusterSum` (603), `SortSurfClusterSum` (627),
//! `sclustReMap` (648), `sclustReMap0` (677), `sclustCompare` (707), `sclustMaxClusterArea`
//! (730), `sclustMaxClusterCount` (747), `sclustMaxClusterWeightVtx` (764),
//! `sclustPruneByCWPval` (785), `sclustAnnot` (827), `sclustGrowByDist` (863) and
//! `sclustSaveAsPointSet` (937). FS-901 to FS-907.
//!
//! A cluster lives in the vertices' `undefval`: 0 outside every cluster, its number (from 1)
//! inside. The two growths, recursive in FreeSurfer, run here on an explicit stack that visits the
//! vertices in the same order (FS-901: the official build's 8 MiB stack overflows on a deep one).
//! The NaNs of every sum are the official build's (fs.core's `sse`, each operand order read from
//! the object code).

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_901, FS_903, FS_904, FS_905, FS_906, FS_907};
use larmorx_core::glibc_qsort::qsort;
use larmorx_core::math::{acos, Libm};
use larmorx_freesurfer_core::cscan::Bytes;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt, sqrt64, sub};
use larmorx_freesurfer_core::timer::Timer;
use larmorx_freesurfer_filter::volcluster::clust_value_in_range;
use larmorx_freesurfer_io::colortab::{ctab_alloc, ctab_annotation_at_index};
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::mris::Mris;
use crate::scanf::Scanf;

/// `FixSurfClusterArea` (1): its presence says the cluster area's group-average fix is in.
pub const FIX_SURF_CLUSTER_AREA: i32 = 1;

/// `SPHERICAL_COORDS`: [`grow_by_dist`]'s shape for an arc on a sphere (any other: flat).
pub const SPHERICAL_COORDS: i32 = 1;

/// The bytes one `sclustGrowSurfCluster` frame takes on the official build's stack (six saved
/// registers, 24 bytes of locals, the return address: its object code).
pub const GROW_FRAME_BYTES: usize = 80;
/// The bytes one `sclustGrowByDist` frame takes (six saved registers, 40 bytes, the return
/// address).
pub const BY_DIST_FRAME_BYTES: usize = 96;
/// The stack of the official programs' main thread (8 MiB, the usual `ulimit -s`).
pub const OFFICIAL_STACK_BYTES: usize = 8 << 20;
/// What the rest of a program takes of that stack (its other frames, the environment), as a
/// margin: the official probe completes a growth 103,500 levels deep and crashes at 104,000
/// (`validation/probes/fs-surfio/stack.py`), so 68 to 108 KB; FS-901 is noted from 128 KiB less.
pub const STACK_MARGIN_BYTES: usize = 128 << 10;

/// `SURFCLUSTERSUM` (`SCS`): one cluster's summary (104 bytes in C).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfClusterSum {
    pub clusterno: i32,
    /// The number of vertices.
    pub nmembers: i32,
    pub area: f32,
    /// The sum of the values.
    pub weightvtx: f32,
    /// The sum of the values weighted by the vertices' areas.
    pub weightarea: f32,
    /// The value of the largest magnitude.
    pub maxval: f32,
    /// Its vertex.
    pub vtxmaxval: i32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub xxfm: f32,
    pub yxfm: f32,
    pub zxfm: f32,
    /// The centroid.
    pub cx: f32,
    pub cy: f32,
    pub cz: f32,
    pub cxxfm: f32,
    pub cyxfm: f32,
    pub czxfm: f32,
    /// From a cluster simulation.
    pub pval_clusterwise: f64,
    pub pval_clusterwise_low: f64,
    pub pval_clusterwise_hi: f64,
}

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// The vertex's `vnum` neighbours (as many as its list holds).
fn nbr(mris: &Mris, vno: usize, k: usize) -> Option<i32> {
    let vt = mris.topology(vno);
    let v = vt.v.as_deref().unwrap_or(&[]);
    let n = usize::try_from(vt.vnum).unwrap_or(0).min(v.len());
    (k < n).then(|| v[k])
}

/// A vertex number of the surface, or FS-903 noted (FreeSurfer reads or writes outside the
/// vertices there).
fn vertex_in(g: &mut Globals, mris: &Mris, vno: i32, what: &str) -> Option<usize> {
    if (0..mris.nvertices).contains(&vno) {
        return Some(vno as usize);
    }
    g.notices.add(
        &FS_903,
        format!(
            "{what}: vertex {vno} of a surface of {} vertices skipped (FreeSurfer reads or writes outside the vertices)",
            mris.nvertices
        ),
    );
    None
}

/// `(float) x` of a `double` as x86-64 converts it (`cvtsd2ss`): a NaN keeps its sign and the top
/// of its payload, quieted.
fn narrow(x: f64) -> f32 {
    if x.is_nan() {
        let b = x.to_bits();
        f32::from_bits(
            ((b >> 32) as u32 & 0x8000_0000) | 0x7fc0_0000 | ((b >> 29) as u32 & 0x003f_ffff),
        )
    } else {
        x as f32
    }
}

/// `(double) x` of a `float` (`cvtss2sd`): a NaN keeps its sign and payload, quieted.
fn widen(x: f32) -> f64 {
    if x.is_nan() {
        let b = x.to_bits();
        f64::from_bits(
            (u64::from(b & 0x8000_0000) << 32)
                | 0x7ff8_0000_0000_0000
                | (u64::from(b & 0x003f_ffff) << 29),
        )
    } else {
        f64::from(x)
    }
}

/// FS-901 noted when a growth's deepest path comes within [`STACK_MARGIN_BYTES`] of what the
/// official build's 8 MiB stack holds (its frames of `frame` bytes): FreeSurfer crashes there
/// (signal 11), or is about to.
fn note_depth(g: &mut Globals, what: &str, deepest: usize, frame: usize) {
    if deepest.saturating_mul(frame) + STACK_MARGIN_BYTES >= OFFICIAL_STACK_BYTES {
        g.notices.add(
            &FS_901,
            format!(
                "{what}: a path of {deepest} vertices ({} bytes of stack in FreeSurfer's recursion, its 8 MiB less 128 KiB or more): FreeSurfer crashes (stack overflow); grown in full",
                deepest * frame
            ),
        );
    }
}

/// `sclustMapSurfClusters(Surf, thmin, thmax, thsign, minarea, &nClusters, XFM, fwhmmap)`: every
/// vertex's `undefval` set to 0; then each vertex in order whose `undefval` is still 0 and whose
/// value is in range (`clustValueInRange`) seeds a cluster numbered from 1 ([`grow_surf_cluster`]);
/// with `minarea` above 0 a cluster whose area ([`surface_area`]) is below it is zeroed again and
/// its number reused. No cluster: `(None, 0)`. Otherwise the summary ([`surf_cluster_summary`]),
/// sorted ([`sort_surf_cluster_sum`]; with `Gdiag_no` above 1 both dumped on standard output
/// between `--- Surface Cluster Summary (unsorted) ---------------` and `---------- sorted
/// ---------------`), the vertices renumbered to the sorted order ([`remap`]); the sorted list and
/// the count.
#[allow(clippy::too_many_arguments)]
pub fn map_surf_clusters(
    g: &mut Globals,
    io: &mut Io,
    surf: &mut Mris,
    thmin: f32,
    thmax: f32,
    thsign: i32,
    minarea: f32,
    xfm: Option<&Matrix>,
    fwhmmap: Option<&Mri>,
) -> Result<(Option<Vec<SurfClusterSum>>, i32), Fatal> {
    let nv = nverts(surf);
    for vtx in 0..nv {
        surf.vertex_mut(vtx).undefval = 0;
    }
    let mut current = 1i32;
    for vtx in 0..nv {
        let v = surf.vertex(vtx);
        let (val, clustno) = (v.val, v.undefval);
        let inrange = clust_value_in_range(val, thmin, thmax, thsign);
        if clustno == 0 && inrange {
            grow_surf_cluster(g, io, current, vtx as i32, surf, thmin, thmax, thsign);
            if minarea > 0.0 {
                let (area, _) = surface_area(current, surf);
                if area < minarea {
                    zero_surface_cluster_no(current, surf);
                    continue;
                }
            }
            current = current.wrapping_add(1);
        }
    }
    let n = current.wrapping_sub(1);
    if n == 0 {
        return Ok((None, 0));
    }
    let (scs, n) = surf_cluster_summary(g, io, surf, xfm, fwhmmap)?;
    let scs = scs.unwrap_or_default();
    let sorted = sort_surf_cluster_sum(&scs, n);
    if g.gdiag_no > 1 {
        let _ = writeln!(
            io.out,
            "--- Surface Cluster Summary (unsorted) ---------------"
        );
        dump_surf_cluster_sum(io.out, &scs, n);
        let _ = writeln!(io.out, "---------- sorted ---------------");
        dump_surf_cluster_sum(io.out, &sorted, n);
    }
    let mut sorted = sorted;
    remap(g, surf, n, &mut sorted);
    Ok((Some(sorted), n))
}

/// `sclustGrowSurfCluster(ClusterNo, SeedVtx, Surf, thmin, thmax, thsign)`: cluster number 0
/// prints `ERROR: clustGrowSurfCluster(): ClusterNo is 0` (standard output) and returns 1.
/// Otherwise the seed's `undefval` becomes the number, then each of its `vnum` neighbours in order
/// whose `undefval` is (still) 0, whose value's magnitude is not below `thmin` and whose value is
/// in range is grown in turn, depth first (each neighbour's own neighbours before the next
/// neighbour). Returns 0. FS-901: noted when the deepest path comes near what FreeSurfer's
/// recursion can hold (it crashes from about 104,000 vertices); the cluster is grown in full in
/// both modes. FS-903: a seed that is not
/// one of the surface's vertices grows nothing (FreeSurfer writes outside the vertices), 0.
#[allow(clippy::too_many_arguments)]
pub fn grow_surf_cluster(
    g: &mut Globals,
    io: &mut Io,
    cluster_no: i32,
    seed: i32,
    surf: &mut Mris,
    thmin: f32,
    thmax: f32,
    thsign: i32,
) -> i32 {
    grow_surf_cluster_depth(g, io, cluster_no, seed, surf, thmin, thmax, thsign).0
}

/// [`grow_surf_cluster`] and the deepest path of its growth (the nested calls FreeSurfer's
/// recursion makes, the seed's included; 0 when nothing is grown): what FS-901 measures.
#[allow(clippy::too_many_arguments)]
pub fn grow_surf_cluster_depth(
    g: &mut Globals,
    io: &mut Io,
    cluster_no: i32,
    seed: i32,
    surf: &mut Mris,
    thmin: f32,
    thmax: f32,
    thsign: i32,
) -> (i32, usize) {
    if cluster_no == 0 {
        let _ = writeln!(io.out, "ERROR: clustGrowSurfCluster(): ClusterNo is 0");
        return (1, 0);
    }
    let Some(seed) = vertex_in(g, surf, seed, "sclustGrowSurfCluster") else {
        return (0, 0);
    };
    surf.vertex_mut(seed).undefval = cluster_no;
    // each frame: the vertex and the next of its neighbours to look at (the recursion's loop)
    let mut stack: Vec<(usize, usize)> = vec![(seed, 0)];
    let mut deepest = 1usize;
    while let Some(top) = stack.last_mut() {
        let (vno, k) = *top;
        let Some(nb) = nbr(surf, vno, k) else {
            stack.pop();
            continue;
        };
        top.1 += 1;
        let Some(nb) = vertex_in(g, surf, nb, "sclustGrowSurfCluster's neighbour") else {
            continue;
        };
        let v = surf.vertex(nb);
        if v.undefval != 0 {
            continue;
        }
        let val = v.val;
        if val.abs() < thmin {
            continue;
        }
        if !clust_value_in_range(val, thmin, thmax, thsign) {
            continue;
        }
        surf.vertex_mut(nb).undefval = cluster_no;
        stack.push((nb, 0));
        deepest = deepest.max(stack.len());
    }
    note_depth(g, "sclustGrowSurfCluster", deepest, GROW_FRAME_BYTES);
    (0, deepest)
}

/// `sclustSurfaceArea(ClusterNo, Surf, &nvtxs)`: the `float` sum, in vertex order, of the areas
/// (each vertex's `area`, or its `group_avg_area` when the group-average areas are loaded) of the
/// vertices of the cluster, scaled by `group_avg_surface_area / total_area` when the group-average
/// surface area is above 0 and the vertex areas are not loaded; and the vertex count.
pub fn surface_area(cluster_no: i32, surf: &Mris) -> (f32, i32) {
    let mut n = 0i32;
    let mut area = 0.0f32;
    for vtx in 0..nverts(surf) {
        let v = surf.vertex(vtx);
        if v.undefval != cluster_no {
            continue;
        }
        let a = if surf.group_avg_vtxarea_loaded == 0 {
            v.area
        } else {
            v.group_avg_area
        };
        area = add(area, a);
        n += 1;
    }
    if surf.group_avg_surface_area > 0.0 && surf.group_avg_vtxarea_loaded == 0 {
        area = mul(area, div(surf.group_avg_surface_area, surf.total_area));
    }
    (area, n)
}

/// `sclustWeight(ClusterNo, Surf, mri, UseArea)`: the `double` sum, in vertex order, over the
/// cluster's vertices of each value (the vertex's `val`, or `MRIgetVoxVal(mri, vtx, 0, 0, 0)`),
/// with `UseArea` times the vertex's area (as [`surface_area`] takes it, each scaled alike),
/// returned as a `float`.
pub fn weight(
    g: &mut Globals,
    err: &mut dyn Write,
    cluster_no: i32,
    surf: &Mris,
    mri: Option<&Mri>,
    use_area: bool,
) -> Result<f32, Fatal> {
    let mut w = 0.0f64;
    for vtx in 0..nverts(surf) {
        let v = surf.vertex(vtx);
        if v.undefval != cluster_no {
            continue;
        }
        let mut h = match mri {
            None => widen(v.val),
            Some(m) => widen(mri_get_vox_val(g, err, m, vtx as i32, 0, 0, 0, 0)?),
        };
        if use_area {
            let mut vtxarea = if surf.group_avg_vtxarea_loaded == 0 {
                widen(v.area)
            } else {
                widen(v.group_avg_area)
            };
            if surf.group_avg_surface_area > 0.0 && surf.group_avg_vtxarea_loaded == 0 {
                vtxarea = mul64(
                    vtxarea,
                    widen(div(surf.group_avg_surface_area, surf.total_area)),
                );
            }
            h = mul64(h, vtxarea);
        }
        w = add64(w, h);
    }
    Ok(narrow(w))
}

/// `sclustSurfaceMax(ClusterNo, Surf, &vtxmax)`: the value of the largest magnitude in the cluster
/// (the first vertex's, then any larger magnitude in vertex order), with its vertex written to
/// `vtxmax`; 0 and `vtxmax` unchanged for a cluster without vertices.
pub fn surface_max(cluster_no: i32, surf: &Mris, vtxmax: &mut i32) -> f32 {
    let mut first = true;
    let mut max = 0.0f32;
    for vtx in 0..nverts(surf) {
        let v = surf.vertex(vtx);
        if v.undefval != cluster_no {
            continue;
        }
        let val = v.val;
        if first {
            max = val;
            *vtxmax = vtx as i32;
            first = false;
            continue;
        }
        if val.abs() > max.abs() {
            max = val;
            *vtxmax = vtx as i32;
        }
    }
    max
}

/// `sclustSurfaceCentroid(ClusterNo, Surf, xyz)`: the mean position of the cluster's vertices
/// (`float` sums in vertex order divided by the count; NaN for a cluster without vertices).
pub fn surface_centroid(cluster_no: i32, surf: &Mris) -> [f64; 3] {
    let (mut xs, mut ys, mut zs) = (0.0f32, 0.0f32, 0.0f32);
    let mut n = 0i32;
    for vtx in 0..nverts(surf) {
        let v = surf.vertex(vtx);
        if v.undefval != cluster_no {
            continue;
        }
        xs = add(xs, v.x);
        ys = add(ys, v.y);
        zs = add(zs, v.z);
        n += 1;
    }
    let nf = n as f32;
    [widen(div(xs, nf)), widen(div(ys, nf)), widen(div(zs, nf))]
}

/// `sclustZeroSurfaceClusterNo(ClusterNo, Surf)`: the cluster's vertices get `undefval` 0. 0.
pub fn zero_surface_cluster_no(cluster_no: i32, surf: &mut Mris) -> f32 {
    for vtx in 0..nverts(surf) {
        let v = surf.vertex_mut(vtx);
        if v.undefval == cluster_no {
            v.undefval = 0;
        }
    }
    0.0
}

/// `sclustZeroSurfaceNonClusters(Surf)`: the vertices outside every cluster get `val` 0. 0.
pub fn zero_surface_non_clusters(surf: &mut Mris) -> f32 {
    for vtx in 0..nverts(surf) {
        let v = surf.vertex_mut(vtx);
        if v.undefval == 0 {
            v.val = 0.0;
        }
    }
    0.0
}

/// `sclustSetSurfaceValToClusterNo(Surf)`: each vertex's `val` becomes its cluster number. 0.
pub fn set_surface_val_to_cluster_no(surf: &mut Mris) -> f32 {
    for vtx in 0..nverts(surf) {
        let v = surf.vertex_mut(vtx);
        v.val = v.undefval as f32;
    }
    0.0
}

/// `sclustSetSurfaceValToCWP(Surf, scs)`: each vertex's `val` becomes 0 outside every cluster,
/// else its cluster's clusterwise p-value `p` as a `float`: 50 when it is 0 (either sign), else
/// `-log10f(p)`; negated unless the cluster's `maxval` is above 0. The base-10 logarithm is
/// correctly rounded by default and glibc 2.35's `log10f` with `reproduce` (fs.core's [`Libm`];
/// they differ in the last bit on some arguments). FS-903: a cluster number beyond the list (or negative) is left
/// alone (FreeSurfer reads outside the list). 0.
pub fn set_surface_val_to_cwp(g: &mut Globals, surf: &mut Mris, scs: &[SurfClusterSum]) -> f32 {
    let libm = Libm::quiet(g.mode());
    for vtx in 0..nverts(surf) {
        let c = surf.vertex(vtx).undefval;
        let val = if c == 0 {
            0.0
        } else {
            let Some(s) = usize::try_from(c - 1).ok().and_then(|k| scs.get(k)) else {
                g.notices.add(
                    &FS_903,
                    format!(
                        "sclustSetSurfaceValToCWP: vertex {vtx}'s cluster {c} is not one of the {} of the list: left alone (FreeSurfer reads outside the list)",
                        scs.len()
                    ),
                );
                continue;
            };
            let p = narrow(s.pval_clusterwise);
            let val = if p == 0.0 {
                50.0
            } else {
                if p < 0.0 {
                    // glibc's log10f of a negative number: EDOM
                    g.errno = 33;
                }
                -libm.log10f(p)
            };
            if s.maxval > 0.0 { val } else { -val }
        };
        surf.vertex_mut(vtx).val = val;
    }
    0.0
}

/// `sclustCountClusters(Surf)`: the largest cluster number (0 at least), as a `float`.
pub fn count_clusters(surf: &Mris) -> f32 {
    let mut max = 0i32;
    for vtx in 0..nverts(surf) {
        max = max.max(surf.vertex(vtx).undefval);
    }
    max as f32
}

/// `(int) f` of [`count_clusters`]'s `float`, as x86-64 truncates it.
fn count_as_int(f: f32) -> i32 {
    f as i32
}

/// The 4 x 1 product `T * [x y z 1]'` into `out` (`MatrixMultiply`; a transform of another size
/// leaves `out` as it was, after `MatrixMultiply`'s messages).
#[allow(clippy::too_many_arguments)]
fn xfm_point(
    g: &mut Globals,
    io: &mut Io,
    t: &Matrix,
    xyz: &mut Matrix,
    out: &mut Matrix,
    p: [f32; 3],
) -> Result<[f32; 3], Fatal> {
    xyz.set_relt(1, 1, p[0]);
    xyz.set_relt(2, 1, p[1]);
    xyz.set_relt(3, 1, p[2]);
    let _ = matrix_multiply(g, io.out, io.err, Some(t), Some(xyz), Some(out))?;
    Ok([out.relt(1, 1), out.relt(2, 1), out.relt(3, 1)])
}

/// `SurfClusterSummary(Surf, T, &nClusters, fwhmmap)`: the summary of the clusters numbered 1 to
/// the largest cluster number ([`count_clusters`]; none: `(None, 0)`), in one pass over the
/// vertices (see `specs/freesurfer-surfio.md` §132.3 for the arithmetic): the count, area, value
/// sums, largest magnitude and its vertex, and centroid of each; with `T` the largest value's
/// position and the centroid through `T`. `FS_CLUSTER_USE_AVG_VERTEX_AREA` (`sscanf %d`, not 0)
/// takes every vertex's area as the mean vertex area; a FWHM map rescales each area by the map's
/// mean FWHM (over its values above 0) squared over the vertex's (0 taken as 1). With `Gdiag_no`
/// above 0: `ClusterUseAvgVertexArea = <n>, avgvertexarea = <a>`, `fwhm mean = <m>` (with a map)
/// and `SurfClusterSumFast: n=<n>, t = <seconds>` on standard output. FS-903: a vertex whose
/// cluster number is negative is left out (FreeSurfer writes before the list).
pub fn surf_cluster_summary(
    g: &mut Globals,
    io: &mut Io,
    surf: &Mris,
    t: Option<&Matrix>,
    fwhmmap: Option<&Mri>,
) -> Result<(Option<Vec<SurfClusterSum>>, i32), Fatal> {
    let nv = nverts(surf);
    let avgvertexarea = if surf.group_avg_vtxarea_loaded != 0 {
        widen(div(surf.group_avg_surface_area, surf.nvertices as f32))
    } else {
        widen(div(surf.total_area, surf.nvertices as f32))
    };
    let mut use_avg = 0i32;
    if let Some(v) = io.env.get("FS_CLUSTER_USE_AVG_VERTEX_AREA") {
        let s = v.as_encoded_bytes().to_vec();
        let mut b = Bytes::new(&s);
        let mut sc = Scanf::new(&mut b);
        if let Some(x) = sc.d(&mut g.errno, true) {
            use_avg = x;
        }
        sc.finish(&mut g.errno);
    }
    if g.gdiag_no > 0 {
        let _ = writeln!(
            io.out,
            "ClusterUseAvgVertexArea = {use_avg}, avgvertexarea = {}",
            cfmt::g(avgvertexarea, 6)
        );
        let _ = io.out.flush();
    }
    let timer = Timer::new();
    let n = count_as_int(count_clusters(surf));
    if n == 0 {
        return Ok((None, 0));
    }
    let nc = usize::try_from(n).unwrap_or(0);
    let mut xyz = matrix_alloc(g, io.out, io.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 matrix");
    xyz.set_relt(4, 1, 1.0);
    let mut xyzxfm = matrix_alloc(g, io.out, io.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 matrix");
    let mut scs = vec![SurfClusterSum::default(); nc];
    let mut weightvtx = vec![0.0f64; nc];
    let mut weightarea = vec![0.0f64; nc];
    let mut fwhmmean2 = 0.0f64;
    if let Some(map) = fwhmmap {
        let (mut sum, mut nhits) = (0.0f64, 0i32);
        for vtx in 0..nv {
            let f = mri_get_vox_val(g, io.err, map, vtx as i32, 0, 0, 0, 0)?;
            if f > 0.0 {
                sum = add64(widen(f), sum);
                nhits += 1;
            }
        }
        let mean = div64(sum, f64::from(nhits));
        if g.gdiag_no > 0 {
            let _ = writeln!(io.out, "fwhm mean = {}", cfmt::g(mean, 6));
        }
        fwhmmean2 = mul64(mean, mean);
    }
    for vtx in 0..nv {
        let v = surf.vertex(vtx);
        let clusterno = v.undefval;
        if clusterno == 0 {
            continue;
        }
        let Some(k) = usize::try_from(clusterno - 1).ok().filter(|&k| k < nc) else {
            g.notices.add(
                &FS_903,
                format!(
                    "SurfClusterSummary: vertex {vtx}'s cluster {clusterno} left out (FreeSurfer writes before its list)"
                ),
            );
            continue;
        };
        let s = &mut scs[k];
        s.nmembers += 1;
        let vtxval = v.val;
        if s.nmembers == 1 {
            s.maxval = vtxval;
            s.vtxmaxval = vtx as i32;
            weightvtx[k] = 0.0;
            weightarea[k] = 0.0;
            s.cx = 0.0;
            s.cy = 0.0;
            s.cz = 0.0;
        }
        let mut vtxarea = if use_avg == 0 {
            if surf.group_avg_vtxarea_loaded == 0 {
                v.area
            } else {
                v.group_avg_area
            }
        } else {
            narrow(avgvertexarea)
        };
        if let Some(map) = fwhmmap {
            let f = mri_get_vox_val(g, io.err, map, vtx as i32, 0, 0, 0, 0)?;
            let q = if f == 0.0 {
                fwhmmean2
            } else {
                let fd = widen(f);
                div64(fwhmmean2, mul64(fd, fd))
            };
            vtxarea = narrow(mul64(widen(vtxarea), q));
        }
        let s = &mut scs[k];
        s.area = add(s.area, vtxarea);
        if vtxval.abs() > s.maxval.abs() {
            s.maxval = vtxval;
            s.vtxmaxval = vtx as i32;
        }
        weightvtx[k] = add64(widen(vtxval), weightvtx[k]);
        weightarea[k] = add64(widen(mul(vtxarea, vtxval)), weightarea[k]);
        s.cx = add(s.cx, v.x);
        s.cy = add(s.cy, v.y);
        s.cz = add(s.cz, v.z);
    }
    for k in 0..nc {
        let p = {
            let s = &mut scs[k];
            s.clusterno = k as i32 + 1;
            let mv = surf.vertex(usize::try_from(s.vtxmaxval).unwrap_or(0));
            (s.x, s.y, s.z) = (mv.x, mv.y, mv.z);
            s.weightvtx = narrow(weightvtx[k]);
            s.weightarea = narrow(weightarea[k]);
            let nm = s.nmembers as f32;
            s.cx = div(s.cx, nm);
            s.cy = div(s.cy, nm);
            s.cz = div(s.cz, nm);
            [s.x, s.y, s.z, s.cx, s.cy, s.cz]
        };
        if let Some(t) = t {
            let a = xfm_point(g, io, t, &mut xyz, &mut xyzxfm, [p[0], p[1], p[2]])?;
            let b = xfm_point(g, io, t, &mut xyz, &mut xyzxfm, [p[3], p[4], p[5]])?;
            let s = &mut scs[k];
            (s.xxfm, s.yxfm, s.zxfm) = (a[0], a[1], a[2]);
            (s.cxxfm, s.cyxfm, s.czxfm) = (b[0], b[1], b[2]);
        }
    }
    if g.gdiag_no > 0 {
        let msec = timer.milliseconds();
        let _ = writeln!(
            io.out,
            "SurfClusterSumFast: n={n}, t = {}",
            cfmt::g(msec as f64 / 1000.0, 6)
        );
    }
    Ok((Some(scs), n))
}

/// `SurfClusterSummaryOld(Surf, T, &nClusters)`: [`surf_cluster_summary`] without a FWHM map
/// unless `USE_FAST_SURF_SMOOTHER` is set to `0`; then (with `Gdiag_no` above 0
/// `SurfClusterSummary()` first) each cluster from 1 to the largest number in turn by
/// [`surface_area`], [`weight`] without and with the areas, [`surface_max`] (an empty cluster's
/// vertex 0) and [`surface_centroid`], each position through `T` as [`surf_cluster_summary`]; with
/// `Gdiag_no` above 0 `SurfClusterSum: n=<n>, t = <seconds>` at the end.
pub fn surf_cluster_summary_old(
    g: &mut Globals,
    io: &mut Io,
    surf: &Mris,
    t: Option<&Matrix>,
) -> Result<(Option<Vec<SurfClusterSum>>, i32), Fatal> {
    let ufss = io
        .env
        .get("USE_FAST_SURF_SMOOTHER")
        .map(|v| v.as_encoded_bytes().to_vec())
        .unwrap_or_else(|| b"1".to_vec());
    if ufss != b"0" {
        return surf_cluster_summary(g, io, surf, t, None);
    }
    if g.gdiag_no > 0 {
        let _ = writeln!(io.out, "SurfClusterSummary()");
    }
    let timer = Timer::new();
    let n = count_as_int(count_clusters(surf));
    if n == 0 {
        return Ok((None, 0));
    }
    let nc = usize::try_from(n).unwrap_or(0);
    let mut xyz = matrix_alloc(g, io.out, io.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 matrix");
    xyz.set_relt(4, 1, 1.0);
    let mut xyzxfm = matrix_alloc(g, io.out, io.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 matrix");
    let mut scs = vec![SurfClusterSum::default(); nc];
    for k in 0..nc {
        let c = k as i32 + 1;
        let (area, nm) = surface_area(c, surf);
        let wv = weight(g, io.err, c, surf, None, false)?;
        let wa = weight(g, io.err, c, surf, None, true)?;
        let mut vm = scs[k].vtxmaxval;
        let maxval = surface_max(c, surf, &mut vm);
        let cen = surface_centroid(c, surf);
        let s = &mut scs[k];
        s.clusterno = c;
        s.area = area;
        s.nmembers = nm;
        s.weightvtx = wv;
        s.weightarea = wa;
        s.maxval = maxval;
        s.vtxmaxval = vm;
        let mv = surf.vertex(usize::try_from(vm).unwrap_or(0));
        (s.x, s.y, s.z) = (mv.x, mv.y, mv.z);
        (s.cx, s.cy, s.cz) = (narrow(cen[0]), narrow(cen[1]), narrow(cen[2]));
        let p = [s.x, s.y, s.z, s.cx, s.cy, s.cz];
        if let Some(t) = t {
            let a = xfm_point(g, io, t, &mut xyz, &mut xyzxfm, [p[0], p[1], p[2]])?;
            let b = xfm_point(g, io, t, &mut xyz, &mut xyzxfm, [p[3], p[4], p[5]])?;
            let s = &mut scs[k];
            (s.xxfm, s.yxfm, s.zxfm) = (a[0], a[1], a[2]);
            (s.cxxfm, s.cyxfm, s.czxfm) = (b[0], b[1], b[2]);
        }
    }
    if g.gdiag_no > 0 {
        let msec = timer.milliseconds();
        let _ = writeln!(
            io.out,
            "SurfClusterSum: n={n}, t = {}",
            cfmt::g(msec as f64 / 1000.0, 6)
        );
    }
    Ok((Some(scs), n))
}

/// `%W.Pf`.
fn f(x: f32, w: usize, p: usize) -> String {
    larmorx_freesurfer_core::cstd::fmt_f(f64::from(x), w, p)
}

/// `DumpSurfClusterSum(fp, scs, nClusters)`: one line per cluster, `%4d  %4d  %8.4f  %6d
/// %6.2f  %4d   %5.1f %5.1f %5.1f   %5.1f %5.1f %5.1f` (its index, number, `maxval`, its vertex,
/// area, count, position, transformed position). 0.
pub fn dump_surf_cluster_sum(fp: &mut dyn Write, scs: &[SurfClusterSum], n: i32) -> i32 {
    for k in 0..usize::try_from(n).unwrap_or(0).min(scs.len()) {
        let s = &scs[k];
        let _ = writeln!(
            fp,
            "{:4}  {:4}  {}  {:6}    {}  {:4}   {} {} {}   {} {} {}",
            k,
            s.clusterno,
            f(s.maxval, 8, 4),
            s.vtxmaxval,
            f(s.area, 6, 2),
            s.nmembers,
            f(s.x, 5, 1),
            f(s.y, 5, 1),
            f(s.z, 5, 1),
            f(s.xxfm, 5, 1),
            f(s.yxfm, 5, 1),
            f(s.zxfm, 5, 1)
        );
    }
    0
}

/// `sclustCompare(a, b)`: the smaller clusterwise p-value first, then the larger area, then the
/// larger `|maxval|`; 0 when none decides (NaNs decide nothing).
pub fn compare(a: &SurfClusterSum, b: &SurfClusterSum) -> i32 {
    if a.pval_clusterwise < b.pval_clusterwise {
        return -1;
    }
    if a.pval_clusterwise > b.pval_clusterwise {
        return 1;
    }
    if a.area > b.area {
        return -1;
    }
    if a.area < b.area {
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

/// `SortSurfClusterSum(scs, nClusters)`: a copy of the first `nClusters` summaries sorted by
/// [`compare`] with C's `qsort` (glibc 2.35's merge order: ties keep their order); the cluster
/// numbers are not changed.
pub fn sort_surf_cluster_sum(scs: &[SurfClusterSum], n: i32) -> Vec<SurfClusterSum> {
    let n = usize::try_from(n).unwrap_or(0).min(scs.len());
    let mut v = scs[..n].to_vec();
    qsort(&mut v, compare);
    v
}

/// `sclustReMap(Surf, nClusters, scs_sorted)`: each vertex's cluster number replaced by the place
/// (from 1) of its cluster in the sorted list (the list's `clusterno` naming each place's old
/// number), 0 kept for the vertices outside; then the list renumbered 1 to `nClusters`. FS-903: an
/// old number outside 1 to `nClusters` in the list, or a vertex's number beyond it, is skipped (the
/// vertex keeps its number; FreeSurfer reads or writes outside its table). 0.
pub fn remap(g: &mut Globals, surf: &mut Mris, n: i32, sorted: &mut [SurfClusterSum]) -> i32 {
    let nc = usize::try_from(n).unwrap_or(0);
    let mut orig2sorted = vec![0i32; nc];
    for c in 1..=nc.min(sorted.len()) {
        let old = sorted[c - 1].clusterno;
        match usize::try_from(old - 1).ok().filter(|&k| k < nc) {
            Some(k) => orig2sorted[k] = c as i32,
            None => g.notices.add(
                &FS_903,
                format!(
                    "sclustReMap: the sorted list's cluster {old} is not one of 1 to {n}: skipped (FreeSurfer writes outside its table)"
                ),
            ),
        }
    }
    for vtx in 0..nverts(surf) {
        let c = surf.vertex(vtx).undefval.wrapping_sub(1);
        if c < 0 {
            surf.vertex_mut(vtx).undefval = 0;
        } else if let Some(&s) = orig2sorted.get(c as usize) {
            surf.vertex_mut(vtx).undefval = s;
        } else {
            g.notices.add(
                &FS_903,
                format!(
                    "sclustReMap: vertex {vtx}'s cluster {} is beyond the {n} clusters: kept (FreeSurfer reads outside its table)",
                    c + 1
                ),
            );
        }
    }
    for c in 1..=nc.min(sorted.len()) {
        sorted[c - 1].clusterno = c as i32;
    }
    0
}

/// `sclustReMap0(Surf, nClusters, scs_sorted)`, the older form: each vertex whose cluster number is
/// the `clusterno` of the list's place `c` (the first such, from 1) gets `c`; other vertices keep
/// theirs. With `Gdiag_no` above 1 first `sclustReMap:` and `new = %3d old = %3d` per place on
/// standard output. 0.
pub fn remap0(
    g: &mut Globals,
    io: &mut Io,
    surf: &mut Mris,
    n: i32,
    sorted: &[SurfClusterSum],
) -> i32 {
    let nc = usize::try_from(n).unwrap_or(0).min(sorted.len());
    if g.gdiag_no > 1 {
        let _ = writeln!(io.out, "sclustReMap:");
        for c in 1..=nc {
            let _ = writeln!(io.out, "new = {c:3} old = {:3}", sorted[c - 1].clusterno);
        }
    }
    for vtx in 0..nverts(surf) {
        let old = surf.vertex(vtx).undefval;
        if let Some(c) = (1..=nc).find(|&c| old == sorted[c - 1].clusterno) {
            surf.vertex_mut(vtx).undefval = c as i32;
        }
    }
    0
}

/// `sclustMaxClusterArea(scs, nClusters)`: the largest area of the first `nClusters` (0 for none).
pub fn max_cluster_area(scs: &[SurfClusterSum], n: i32) -> f64 {
    let n = usize::try_from(n).unwrap_or(0).min(scs.len());
    if n == 0 {
        return 0.0;
    }
    let mut max = widen(scs[0].area);
    for s in &scs[..n] {
        if max < widen(s.area) {
            max = widen(s.area);
        }
    }
    max
}

/// `sclustMaxClusterCount(scs, nClusters)`: the largest vertex count (0 for none).
pub fn max_cluster_count(scs: &[SurfClusterSum], n: i32) -> i32 {
    let n = usize::try_from(n).unwrap_or(0).min(scs.len());
    if n == 0 {
        return 0;
    }
    let mut max = scs[0].nmembers;
    for s in &scs[..n] {
        if max < s.nmembers {
            max = s.nmembers;
        }
    }
    max
}

/// `sclustMaxClusterWeightVtx(scs, nClusters, thsign)`: from `-thsign x 10e10` (0 for `thsign` 0)
/// as a `float`, the `weightvtx` of the largest magnitude (`thsign` 0), the largest (+1) or the
/// smallest (-1); any other `thsign` keeps the start. 0 for no cluster.
pub fn max_cluster_weight_vtx(scs: &[SurfClusterSum], n: i32, thsign: i32) -> f32 {
    let n = usize::try_from(n).unwrap_or(0).min(scs.len());
    if n == 0 {
        return 0.0;
    }
    let mut maxw = if thsign == 0 {
        0.0
    } else {
        narrow(f64::from(thsign.wrapping_neg()) * 10e10)
    };
    for s in &scs[..n] {
        let w = s.weightvtx;
        if thsign == 0 && maxw.abs() < w.abs() {
            maxw = w;
        }
        if thsign == 1 && maxw < w {
            maxw = w;
        }
        if thsign == -1 && maxw > w {
            maxw = w;
        }
    }
    maxw
}

/// `sclustPruneByCWPval(ClusterList, nclusters, cwpvalthresh, &nPruned, surf)`: the clusters whose
/// clusterwise p-value is at most the threshold, in order, and their count; each vertex of a
/// cluster above it gets `undefval` and `val` 0, each vertex of a kept cluster its place (from 1)
/// in the new list. FS-904: FreeSurfer's renumbering table holds 10,000 clusters (more are written
/// past it); larmorx takes any number, noted. FS-905: a cluster whose p-value is NaN is neither kept
/// nor removed in FreeSurfer (its vertices keep their value and get a number from memory never
/// written); its vertices are removed by default (rule A), with `reproduce` they keep their value
/// and get 0. FS-903: a vertex whose cluster number is beyond the list or negative is left alone
/// (FreeSurfer reads outside the list).
pub fn prune_by_cw_pval(
    g: &mut Globals,
    list: &[SurfClusterSum],
    n: i32,
    thresh: f64,
    surf: &mut Mris,
) -> (Vec<SurfClusterSum>, i32) {
    let nc = usize::try_from(n).unwrap_or(0).min(list.len());
    if nc > 10_000 {
        g.notices.add(
            &FS_904,
            format!(
                "sclustPruneByCWPval of {nc} clusters: FreeSurfer's table of 10,000 is written past; all renumbered"
            ),
        );
    }
    let mut pruned = Vec::new();
    let mut map = vec![None; nc];
    for k in 0..nc {
        if list[k].pval_clusterwise <= thresh {
            map[k] = Some(pruned.len() as i32);
            pruned.push(list[k]);
        }
    }
    let reproduce = g.notices.reproduce();
    for vtx in 0..nverts(surf) {
        let c = surf.vertex(vtx).undefval;
        if c == 0 {
            continue;
        }
        let Some(k) = usize::try_from(c - 1).ok().filter(|&k| k < nc) else {
            g.notices.add(
                &FS_903,
                format!(
                    "sclustPruneByCWPval: vertex {vtx}'s cluster {c} is not one of the {nc}: left alone (FreeSurfer reads outside the list)"
                ),
            );
            continue;
        };
        let p = list[k].pval_clusterwise;
        if p > thresh {
            let v = surf.vertex_mut(vtx);
            v.undefval = 0;
            v.val = 0.0;
        } else if let Some(m) = map[k] {
            surf.vertex_mut(vtx).undefval = m + 1;
        } else {
            g.notices.add(
                &FS_905,
                format!(
                    "sclustPruneByCWPval: cluster {c} has a NaN clusterwise p-value: FreeSurfer gives its vertices a number from memory never written"
                ),
            );
            let v = surf.vertex_mut(vtx);
            v.undefval = 0;
            if !reproduce {
                v.val = 0.0;
            }
        }
    }
    let np = pruned.len() as i32;
    (pruned, np)
}

/// `sclustAnnot(surf, NClusters)`: the surface's colour table replaced by `CTABalloc(NClusters +
/// 1)` with entries 1 to `NClusters` named `cluster-%03d`; each vertex's annotation 0 outside the
/// clusters 1 to `NClusters`, else its cluster's entry's. FS-903: a negative cluster number gives
/// annotation 0 (FreeSurfer's lookup refuses it, `CTABannotationAtIndex: index <i> was OOB`, and
/// stores a value it did not set). 0.
pub fn annot(g: &mut Globals, io: &mut Io, surf: &mut Mris, nclusters: i32) -> Result<i32, Fatal> {
    let ct = ctab_alloc(g, io.env, io.out, io.err, nclusters.wrapping_add(1))?;
    surf.ct = ct;
    if let Some(ct) = surf.ct.as_mut() {
        for k in 1..usize::try_from(nclusters.wrapping_add(1)).unwrap_or(0) {
            if let Some(Some(e)) = ct.entries.get_mut(k) {
                e.name = format!("cluster-{k:03}").into_bytes();
            }
        }
    }
    for vtx in 0..nverts(surf) {
        let c = surf.vertex(vtx).undefval;
        if c == 0 || c > nclusters {
            surf.vertex_mut(vtx).annotation = 0;
            continue;
        }
        let a = match surf.ct.as_ref() {
            Some(ct) => ctab_annotation_at_index(g, io.err, ct, c),
            None => Err(0),
        };
        let a = match a {
            Ok(a) => a,
            Err(_) => {
                g.notices.add(
                    &FS_903,
                    format!(
                        "sclustAnnot: vertex {vtx}'s cluster {c} has no entry: annotation 0 (FreeSurfer stores a value it did not set)"
                    ),
                );
                0
            }
        };
        surf.vertex_mut(vtx).annotation = a;
    }
    Ok(0)
}

/// `sclustGrowByDist(surf, seedvtxno, dthresh, shape, vtxno, vtxlist)`: the vertices within
/// `dthresh` of the seed reachable through such vertices, listed in `vtxlist` in the order the
/// growth reaches them (depth first, each vertex's `vnum` neighbours in order), their `undefval`
/// 1; returns the count. A first call (`vtxno` negative) clears every `undefval`, takes the seed
/// (and, for [`SPHERICAL_COORDS`], its radius); the distance is the arc on that sphere
/// (`radius x acos(dot / radius²)`, the dot in `float`) or the flat distance in x and y
/// (`sqrtf`). The state (seed, radius, count) stays for later calls with `vtxno` not negative,
/// which continue from `vtxno`; such a call before any first call prints `ERROR: set vtxno to a
/// negative value when calling this function!` and returns -1, a call without a list `ERROR:
/// vtxlist is NULL` and -1 (standard output). FS-901: noted when the deepest path passes what
/// FreeSurfer's recursion can hold (it crashes); grown in full in both modes. FS-907: a call that
/// continues on a surface without the earlier seed's vertex returns -1 (FreeSurfer reads through a
/// pointer into the earlier surface).
#[allow(clippy::too_many_arguments)]
pub fn grow_by_dist(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    surf: &mut Mris,
    seedvtxno: i32,
    dthresh: f64,
    shape: i32,
    vtxno: i32,
    vtxlist: Option<&mut Vec<i32>>,
) -> i32 {
    let Some(list) = vtxlist else {
        let _ = writeln!(io.out, "ERROR: vtxlist is NULL");
        return -1;
    };
    let st = &mut sg.sclust_by_dist;
    if st.ncalls == 0 && vtxno >= 0 {
        let _ = writeln!(
            io.out,
            "ERROR: set vtxno to a negative value when calling this function!"
        );
        return -1;
    }
    let mut start = vtxno;
    if vtxno < 0 {
        let Some(seed) = vertex_in(g, surf, seedvtxno, "sclustGrowByDist") else {
            return -1;
        };
        st.v1 = seed;
        st.ncalls = 0;
        st.nhits = 0;
        if shape == SPHERICAL_COORDS {
            let v1 = surf.vertex(seed);
            let r2 = add(add(mul(v1.y, v1.y), mul(v1.x, v1.x)), mul(v1.z, v1.z));
            st.radius2 = widen(r2);
            st.radius = sqrt64(st.radius2);
        }
        for k in 0..nverts(surf) {
            surf.vertex_mut(k).undefval = 0;
        }
        start = seedvtxno;
    }
    if st.v1 >= nverts(surf) {
        g.notices.add(
            &FS_907,
            format!(
                "sclustGrowByDist: the earlier seed {} is not one of this surface's {} vertices: -1 (FreeSurfer reads through a pointer into the earlier surface)",
                st.v1, surf.nvertices
            ),
        );
        return -1;
    }
    let (v1x, v1y, v1z) = {
        let v1 = surf.vertex(st.v1);
        (v1.x, v1.y, v1.z)
    };
    let (radius, radius2) = (st.radius, st.radius2);
    // one visit of the recursion: Some(true) when the vertex was added (its neighbours follow)
    let mut visit =
        |g: &mut Globals, st: &mut ByDistState, surf: &mut Mris, vno: i32| -> Option<usize> {
            st.ncalls = st.ncalls.wrapping_add(1);
            let vno = vertex_in(g, surf, vno, "sclustGrowByDist's neighbour")?;
            let v2 = surf.vertex(vno);
            if v2.undefval != 0 {
                return None;
            }
            let d = if shape == SPHERICAL_COORDS {
                let dot = add(add(mul(v1x, v2.x), mul(v1y, v2.y)), mul(v1z, v2.z));
                let costheta = div64(widen(dot), radius2);
                if costheta.abs() > 1.0 {
                    // glibc's acos outside [-1, 1]: EDOM (a vertex farther from the centre than the seed)
                    g.errno = 33;
                }
                mul64(acos(costheta), radius)
            } else {
                let dx = sub(v1x, v2.x);
                let dy = sub(v1y, v2.y);
                widen(sqrt(add(mul(dx, dx), mul(dy, dy))))
            };
            if d > dthresh {
                return None;
            }
            let at = usize::try_from(st.nhits).unwrap_or(0);
            if at < list.len() {
                list[at] = vno as i32;
            } else {
                list.resize(at, 0);
                list.push(vno as i32);
            }
            st.nhits += 1;
            surf.vertex_mut(vno).undefval = 1;
            Some(vno)
        };
    let st = &mut sg.sclust_by_dist;
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut deepest = 0usize;
    match visit(g, st, surf, start) {
        None => return 0,
        Some(v) => stack.push((v, 0)),
    }
    deepest = deepest.max(stack.len());
    while let Some(top) = stack.last_mut() {
        let (vno, k) = *top;
        let Some(nb) = nbr(surf, vno, k) else {
            stack.pop();
            continue;
        };
        top.1 += 1;
        if let Some(v) = visit(g, st, surf, nb) {
            stack.push((v, 0));
            deepest = deepest.max(stack.len());
        } else {
            // a visit that returns at once still took a frame
            deepest = deepest.max(stack.len() + 1);
        }
    }
    note_depth(g, "sclustGrowByDist", deepest, BY_DIST_FRAME_BYTES);
    st.nhits
}

/// The state `sclustGrowByDist` keeps between calls (its `static` variables): the seed (a vertex
/// number of the surface it was taken from), the sphere's radius and its square, the count of
/// vertices listed and of visits.
#[derive(Clone, Debug, Default)]
pub struct ByDistState {
    pub v1: usize,
    pub radius: f64,
    pub radius2: f64,
    pub nhits: i32,
    pub ncalls: i32,
}

/// `sclustSaveAsPointSet(fname, scslist, NClusters, surf)`: a text file for freeview: for each of
/// the clusters the position of its largest value's vertex (`%g %g %g`), then `info`,
/// `numpoints <n>`, `useRealRAS 0`. 0. FS-906: a file that cannot be created crashes FreeSurfer;
/// larmorx prints `ERROR: sclustSaveAsPointSet(): could not open <fname>` and returns 1 by
/// default (rule A), the crash (status 139) with `reproduce`. FS-903: a cluster whose vertex is not
/// one of the surface's is written as `0 0 0` (FreeSurfer reads outside the vertices).
pub fn save_as_point_set(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&std::path::Path>,
    fname: &str,
    scs: &[SurfClusterSum],
    n: i32,
    surf: &Mris,
) -> Result<i32, Fatal> {
    let mut fp = match crate::sfile::open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            if g.notices.reproduce() {
                g.notices.add(
                    &FS_906,
                    format!("sclustSaveAsPointSet: {fname} cannot be created: FreeSurfer crashes (status 139)"),
                );
                return Err(Fatal::Exit(139));
            }
            g.notices.add(
                &FS_906,
                format!(
                    "sclustSaveAsPointSet: {fname} cannot be created: an error (FreeSurfer crashes)"
                ),
            );
            let _ = writeln!(
                io.out,
                "ERROR: sclustSaveAsPointSet(): could not open {fname}"
            );
            return Ok(1);
        }
    };
    for s in &scs[..usize::try_from(n).unwrap_or(0).min(scs.len())] {
        let p = match vertex_in(g, surf, s.vtxmaxval, "sclustSaveAsPointSet") {
            Some(v) => {
                let v = surf.vertex(v);
                [v.x, v.y, v.z]
            }
            None => [0.0; 3],
        };
        let _ = writeln!(
            fp,
            "{} {} {}",
            cfmt::g(f64::from(p[0]), 6),
            cfmt::g(f64::from(p[1]), 6),
            cfmt::g(f64::from(p[2]), 6)
        );
    }
    let _ = write!(fp, "info\nnumpoints {n}\nuseRealRAS 0\n");
    drop(fp);
    Ok(0)
}
