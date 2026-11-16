// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The last pieces of the metric properties' files that every caller of `fs.surfio` can reach
//! (queue items 3.1-49 and 3.1-50, group (e) of track D's list): FreeSurfer 8.2.0's
//! `utils/mrisurf_metricProperties.cpp`'s `MRIStalairachTransform` (line 607, FS-629), the
//! vertex-to-voxel conversions (`MRISvertexToVoxel`, `MRISvertexToVoxelNotCached`,
//! `MRISvertexNormalToVoxel`, `MRISvertexNormalToVoxelScaled`, `MRISvertexCoordToVoxel`,
//! `MRISorigVertexToVoxel`, `MRISwhiteVertexToVoxel`, lines 8478-8576), the two distributions
//! `mris_fix_topology` and `mris_topo_fixer` learn their retessellation's likelihoods from
//! (`mrisComputeNormalDotDistribution` 8617, FS-626, FS-627;
//! `mrisComputePrincipalCurvatureDistributions` 8719, FS-626, FS-628), and Ashburner's triangle
//! energy (`mrisSampleAshburnerTriangleEnergy` 9081 with the file-local `get_face_axes` 8838 and
//! `ashburnerTriangleEnergy` 8952, which the official build inlines into it). The integration
//! parameters' `l_ashburner_lambda` (a `float`) is passed by value until fs.deform's
//! `INTEGRATION_PARMS` exists.
//!
//! Every operation that can make or meet a NaN goes through fs.core's [`sse`] helpers in the
//! official build's operand order, read from the unit oracle's `mrisurf_metricProperties.cpp.o`
//! (GCC 8.4.0 commutes some sums and products: `docs/findings/freesurfer-surfio.md` §30). `logf`
//! and `log` are glibc's with `reproduce`, correctly rounded by default (fs.core's [`Libm`]).

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use std::path::Path;

use larmorx_core::defects::{FS_626, FS_627, FS_628, FS_629};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::diag::{DIAG_VERBOSE, DIAG_WRITE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::{Histogram, histo_copy, histo_plot, histo_smooth};
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{self, Matrix};
use larmorx_freesurfer_core::matrix_decomp::matrix_eigen_system;
use larmorx_freesurfer_core::matrix_ops::{
    Region, matrix_copy_region, matrix_determinant, matrix_inverse,
};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt64, sub, sub64};
use larmorx_freesurfer_io::mriio;
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, MRI_FLOAT, Mri};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::talairachex::transform_with_matrix;

use crate::aaa::SurfGlobals;
use crate::base::project_point_onto_sphere;
use crate::coords::{self, Io};
use crate::mris::{Mris, Vertex, VertexTopology};
use crate::views::{ORIGINAL_VERTICES, TMP_VERTICES, vertex_coord2xyz_double};
use crate::{dist, mp, positions, props6, props8, sras};

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// x86-64's `cvttss2si`: a float to `int` toward zero; NaN and values outside the range give
/// `INT_MIN`.
fn cvtt(x: f32) -> i32 {
    if x.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&x) {
        i32::MIN
    } else {
        x as i32
    }
}

/// `FZERO(x)` of a `double`: `fabs(x) < FLT_EPSILON`.
fn fzero64(x: f64) -> bool {
    x.abs() < f64::from(f32::EPSILON)
}

/// `FZERO(x)` of a `float`.
fn fzero(x: f32) -> bool {
    x.abs() < f32::EPSILON
}

// ---- MRIStalairachTransform --------------------------------------------------------------------

/// `MRIStalairachTransform(mris_src, mris)` with two surfaces: `mris` (a clone of the source,
/// `MRISclone`, when none is given) unchanged when the source has no Talairach transform (`lta`);
/// else its distances freed (`MRISfreeDistsButNotOrig`), each of the source's first `nvertices`
/// vertices of `mris` moved through the source's surface-RAS-to-Talairach matrix
/// (`TransformWithMatrix`: the point in `float`, the result stored as `float`), the bounding box of
/// the new positions (from 10000 and -10000) and its centre stored. `New` for a clone, `Given` for
/// the caller's surface.
///
/// **FS-629:** FreeSurfer crashes for a source with a transform but no matrix (and vertices);
/// larmorx gives the error `MRIStalairachTransform: the surface has a Talairach transform but no
/// matrix from surface RAS to Talairach space` and `Null` (`reproduce`: status 139, the crash).
/// A destination with fewer vertices than the source, which FreeSurfer writes past, is the error
/// `MRIStalairachTransform: the destination has <n> vertices, the source <m>` and `Null` in both
/// modes. Every FreeSurfer caller passes one surface as both (see
/// [`talairach_transform_in_place`]).
pub fn talairach_transform(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    src: &Mris,
    dst: Option<&mut Mris>,
) -> Result<Ret<Mris>, Fatal> {
    let m = src.SRASToTalSRAS_.as_ref();
    match dst {
        None => {
            let mut c = props8::clone(g, sg, io, src)?;
            if !apply_talairach(g, io, src.lta.is_some(), m, src.nvertices, &mut c)? {
                return Ok(Ret::Null);
            }
            Ok(Ret::New(c))
        }
        Some(d) => {
            if !apply_talairach(g, io, src.lta.is_some(), m, src.nvertices, d)? {
                return Ok(Ret::Null);
            }
            Ok(Ret::Given)
        }
    }
}

/// `MRIStalairachTransform(mris, mris)`, as `mris_inflate` and `mris2rgb` call it: the surface
/// moved to Talairach space in place ([`talairach_transform`]). `NO_ERROR`; `ERROR_BADPARM` after
/// FS-629's error.
pub fn talairach_transform_in_place(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    let has = mris.lta.is_some();
    let m = mris.SRASToTalSRAS_.clone();
    let n = mris.nvertices;
    Ok(if apply_talairach(g, io, has, m.as_ref(), n, mris)? {
        NO_ERROR
    } else {
        ERROR_BADPARM
    })
}

fn apply_talairach(
    g: &mut Globals,
    io: &mut Io,
    has_lta: bool,
    m: Option<&Matrix>,
    nsrc: i32,
    mris: &mut Mris,
) -> Result<bool, Fatal> {
    if !has_lta {
        return Ok(true);
    }
    dist::free_dists_but_not_orig(mris);
    let n = usize::try_from(nsrc).unwrap_or(0);
    if n > nverts(mris) {
        g.notices.add(
            &FS_629,
            format!(
                "MRIStalairachTransform into a surface of {} vertices from one of {n}: an error (FreeSurfer writes past the destination's vertices)",
                mris.nvertices
            ),
        );
        log::error_printf(
            g,
            io.err,
            ERROR_BADPARM,
            &format!(
                "MRIStalairachTransform: the destination has {} vertices, the source {n}",
                mris.nvertices
            ),
        );
        return Ok(false);
    }
    let mut lo = [10000f32; 3];
    let mut hi = [-10000f32; 3];
    if n > 0 {
        let Some(m) = m else {
            if g.notices.reproduce() {
                g.notices.add(
                    &FS_629,
                    "MRIStalairachTransform of a surface with a Talairach transform and no surface-RAS-to-Talairach matrix: FreeSurfer crashes (status 139)",
                );
                return Err(Fatal::Exit(139));
            }
            g.notices.add(
                &FS_629,
                "MRIStalairachTransform of a surface with a Talairach transform and no surface-RAS-to-Talairach matrix: an error (FreeSurfer crashes)",
            );
            log::error_printf(
                g,
                io.err,
                ERROR_BADPARM,
                "MRIStalairachTransform: the surface has a Talairach transform but no matrix from surface RAS to Talairach space",
            );
            return Ok(false);
        };
        for vno in 0..n {
            let v = mris.vertex(vno);
            let p = transform_with_matrix(m, f64::from(v.x), f64::from(v.y), f64::from(v.z));
            let v = mris.vertex_mut(vno);
            (v.x, v.y, v.z) = (p[0] as f32, p[1] as f32, p[2] as f32);
            // the official build compares the transformed doubles with the box's floats widened
            for k in 0..3 {
                if p[k] > f64::from(hi[k]) {
                    hi[k] = p[k] as f32;
                }
                if p[k] < f64::from(lo[k]) {
                    lo[k] = p[k] as f32;
                }
            }
        }
    }
    (mris.xlo, mris.ylo, mris.zlo) = (lo[0], lo[1], lo[2]);
    (mris.xhi, mris.yhi, mris.zhi) = (hi[0], hi[1], hi[2]);
    mris.xctr = coords::centre(lo[0], hi[0]);
    mris.yctr = coords::centre(lo[1], hi[1]);
    mris.zctr = coords::centre(lo[2], hi[2]);
    Ok(true)
}

// ---- the vertex-to-voxel conversions ------------------------------------------------------------

fn widen(x: f32, y: f32, z: f32) -> (f64, f64, f64) {
    (f64::from(x), f64::from(y), f64::from(z))
}

/// `MRISvertexToVoxel(mris, v, mri, ...)`: the vertex's current position (`float`, widened) in
/// `mri`'s voxels through the program's cached map (`MRISsurfaceRASToVoxelCached`, FS-604).
pub fn vertex_to_voxel(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &Mris,
    v: &Vertex,
    mri: &Mri,
) -> Result<[f64; 3], Fatal> {
    let (x, y, z) = widen(v.x, v.y, v.z);
    sras::mris_surface_ras_to_voxel_cached(g, sg, io, mris, mri, x, y, z)
}

/// `MRISvertexToVoxelNotCached(mris, v, mri, ...)`: as [`vertex_to_voxel`] through
/// `MRISsurfaceRASToVoxel` (the matrices made for this call).
pub fn vertex_to_voxel_not_cached(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    v: &Vertex,
    mri: &Mri,
) -> Result<[f64; 3], Fatal> {
    let (x, y, z) = widen(v.x, v.y, v.z);
    sras::mris_surface_ras_to_voxel(g, io, mris, mri, x, y, z)
}

/// `MRISvertexNormalToVoxel(mris, v, mri, ...)`: the voxel of the vertex's position subtracted
/// from that of its position plus its normal (`v->x + v->nx` in `float`), both through the cached
/// map.
pub fn vertex_normal_to_voxel(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &Mris,
    v: &Vertex,
    mri: &Mri,
) -> Result<[f64; 3], Fatal> {
    let (x, y, z) = widen(v.x, v.y, v.z);
    let p0 = sras::mris_surface_ras_to_voxel_cached(g, sg, io, mris, mri, x, y, z)?;
    let (x, y, z) = widen(add(v.x, v.nx), add(v.y, v.ny), add(v.z, v.nz));
    let p1 = sras::mris_surface_ras_to_voxel_cached(g, sg, io, mris, mri, x, y, z)?;
    Ok([
        sub64(p1[0], p0[0]),
        sub64(p1[1], p0[1]),
        sub64(p1[2], p0[2]),
    ])
}

/// `MRISvertexNormalToVoxelScaled(mris, v, mri, ...)`: [`vertex_normal_to_voxel`] divided by its
/// length; `ERROR_BADPARM` and nothing for a length below `FLT_EPSILON` (`FZERO`), else
/// `NO_ERROR` and the unit vector.
pub fn vertex_normal_to_voxel_scaled(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &Mris,
    v: &Vertex,
    mri: &Mri,
) -> Result<(i32, Option<[f64; 3]>), Fatal> {
    let n = vertex_normal_to_voxel(g, sg, io, mris, v, mri)?;
    let len = sqrt64(add64(
        add64(mul64(n[0], n[0]), mul64(n[1], n[1])),
        mul64(n[2], n[2]),
    ));
    if fzero64(len) {
        return Ok((ERROR_BADPARM, None));
    }
    Ok((
        NO_ERROR,
        Some([div64(n[0], len), div64(n[1], len), div64(n[2], len)]),
    ))
}

/// `MRISvertexCoordToVoxel(mris, v, mri, coords, ...)`: the vertex's coordinate set `coords`
/// (`MRISvertexCoord2XYZ_double`: an unsupported set ends the program) through the cached map.
pub fn vertex_coord_to_voxel(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &Mris,
    v: &Vertex,
    mri: &Mri,
    which: i32,
) -> Result<[f64; 3], Fatal> {
    let (x, y, z) = vertex_coord2xyz_double(g, io.err, v, which)?;
    sras::mris_surface_ras_to_voxel_cached(g, sg, io, mris, mri, x, y, z)
}

/// `MRISorigVertexToVoxel(mris, v, mri, ...)`: the vertex's original position through the cached
/// map.
pub fn orig_vertex_to_voxel(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &Mris,
    v: &Vertex,
    mri: &Mri,
) -> Result<[f64; 3], Fatal> {
    let (x, y, z) = widen(v.origx, v.origy, v.origz);
    sras::mris_surface_ras_to_voxel_cached(g, sg, io, mris, mri, x, y, z)
}

/// `MRISwhiteVertexToVoxel(mris, v, mri, ...)`: the vertex's white position through the cached
/// map.
pub fn white_vertex_to_voxel(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &Mris,
    v: &Vertex,
    mri: &Mri,
) -> Result<[f64; 3], Fatal> {
    let (x, y, z) = widen(v.whitex, v.whitey, v.whitez);
    sras::mris_surface_ras_to_voxel_cached(g, sg, io, mris, mri, x, y, z)
}

// ---- the distributions --------------------------------------------------------------------------

fn neighbours(vt: &VertexTopology) -> &[i32] {
    let v = vt.v.as_deref().unwrap_or(&[]);
    &v[..usize::try_from(vt.vnum).unwrap_or(0).min(v.len())]
}

/// Whether `DIAG_WRITE` and `DIAG_VERBOSE` are both set (FreeSurfer's `Gdiag & DIAG_WRITE &&
/// DIAG_VERBOSE_ON`).
fn diag_write(g: &Globals) -> bool {
    g.gdiag & DIAG_WRITE != 0 && g.gdiag & DIAG_VERBOSE != 0
}

/// The normal-dot product of the official build: `(dy ny) dz nz + dx nx` (FS-627's formula, the
/// products and the sum in this order), and the corrected `dx nx + dy ny + dz nz`.
fn normal_dot(v: &Vertex, vn: &Vertex) -> (f32, f32) {
    let dx = sub(vn.x, v.x);
    let dy = sub(vn.y, v.y);
    let dz = sub(vn.z, v.z);
    let dot = add(mul(mul(mul(dy, v.ny), dz), v.nz), mul(dx, v.nx));
    let right = add(add(mul(dx, v.nx), mul(dy, v.ny)), mul(dz, v.nz));
    (dot, right)
}

/// A count where FreeSurfer's index may fall outside the histogram's counts (FS-626 reproduced):
/// added where the allocation (`max_bins`) holds it, dropped and noted elsewhere.
fn count_at(g: &mut Globals, h: &mut Histogram, bin: i32, what: &str) {
    match usize::try_from(bin).ok().filter(|&b| b < h.counts.len()) {
        Some(b) => h.counts[b] = add(h.counts[b], 1.0),
        None => g.notices.add(
            &FS_626,
            format!(
                "{what}: a count at bin {bin} of a histogram of {} bins, outside its counts, dropped (FreeSurfer writes outside them)",
                h.nbins
            ),
        ),
    }
}

/// Each count in use set to 0.01 where it is 0, then divided by `n` (`float`).
fn normalise_counts(h: &mut Histogram, n: i32) {
    let nf = n as f32;
    for b in 0..usize::try_from(h.nbins).unwrap_or(0) {
        if h.counts[b] == 0.0 {
            h.counts[b] = 0.01;
        }
        h.counts[b] = div(h.counts[b], nf);
    }
}

/// `mrisComputeNormalDotDistribution(mris, h_dot)`: on the original positions (the current ones
/// saved in the temporary set, the original ones restored and the metric properties computed),
/// each unripped vertex's dot of every neighbour's offset with its normal (FS-627's formula); the
/// histogram's bin size `(max - min) / (nbins - 2)`, its bins from `min - bin_size` in steps of
/// it, each dot counted in bin `(dot - bins[0]) / bin_size` (truncated; NaN 0) kept within 0 and
/// `nbins`; every count in use 0 made 0.01 and all divided by the number of offsets; then the
/// counts smoothed (`HISTOsmooth`, sigma 2) from a copy of the histogram. With `DIAG_WRITE` and
/// `DIAG_VERBOSE` the smoothed and the raw histograms are plotted to `ndot.plt` and `rdot.plt` (in
/// `dir`). The current positions are restored and the metric properties computed again.
/// `NO_ERROR`.
///
/// **FS-627** (kept, rule D: measured at `mris_fix_topology`'s port): the dot is `dx nx + dy ny dz
/// nz`, a product where the sum `dx nx + dy ny + dz nz` is meant; noted when the two differ.
///
/// **FS-626:** with fewer than 3 bins the bin size divides by 0 or a negative count (with one bin
/// the largest dots fall one past the last bin, outside the counts). By default the error
/// `mrisComputeNormalDotDistribution: a histogram of <n> bins (3 or more are needed)`,
/// `ERROR_BADPARM` and nothing changed; `reproduce` computes, a count past the histogram's
/// allocation dropped. With 3 bins or more a dot counted at bin `nbins` (FreeSurfer's index allows
/// it; float rounding alone could reach it) goes to the last bin by default.
pub fn compute_normal_dot_distribution(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    h: &mut Histogram,
) -> Result<i32, Fatal> {
    let nbins = h.nbins;
    let rep = g.notices.reproduce();
    if nbins < 3 {
        if !rep {
            g.notices.add(
                &FS_626,
                format!(
                    "mrisComputeNormalDotDistribution with a histogram of {nbins} bins: an error (FreeSurfer's bin size divides by {})",
                    nbins - 2
                ),
            );
            log::error_printf(
                g,
                io.err,
                ERROR_BADPARM,
                &format!(
                    "mrisComputeNormalDotDistribution: a histogram of {nbins} bins (3 or more are needed)"
                ),
            );
            return Ok(ERROR_BADPARM);
        }
        g.notices.add(
            &FS_626,
            format!(
                "mrisComputeNormalDotDistribution with a histogram of {nbins} bins: computed (its bin size divides by {})",
                nbins - 2
            ),
        );
    }
    positions::save_vertex_positions(g, io, mris, TMP_VERTICES)?;
    positions::restore_vertex_positions(g, io, mris, ORIGINAL_VERTICES)?;
    mp::compute_metric_properties(g, sg, io, mris)?;
    let mut min_dot = 100000f32;
    let mut max_dot = -100000f32;
    let mut misnamed = 0usize;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        for &n in neighbours(mris.topology(vno)) {
            let (dot, right) = normal_dot(v, mris.vertex(n as usize));
            misnamed += usize::from(dot.to_bits() != right.to_bits());
            if dot < min_dot {
                min_dot = dot;
            }
            if dot > max_dot {
                max_dot = dot;
            }
        }
    }
    if misnamed > 0 {
        g.notices.add(
            &FS_627,
            format!(
                "mrisComputeNormalDotDistribution: {misnamed} offsets whose dot dx nx + dy ny dz nz differs from dx nx + dy ny + dz nz (kept)"
            ),
        );
    }
    let bin_size = div(sub(max_dot, min_dot), (nbins - 2) as f32);
    h.bin_size = bin_size;
    let mut bin_val = sub(min_dot, bin_size);
    for b in 0..usize::try_from(nbins).unwrap_or(0) {
        h.bins[b] = bin_val;
        bin_val = add(bin_val, bin_size);
    }
    // FreeSurfer reads bins[0] also for a histogram without bins (memory it does not own); every
    // count then lands outside the histogram, so the value read changes nothing
    let first = h.bins.first().copied().unwrap_or(0.0);
    let mut num = 0i32;
    let mut past = 0usize;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        for &n in neighbours(mris.topology(vno)) {
            num = num.wrapping_add(1);
            let (dot, _) = normal_dot(v, mris.vertex(n as usize));
            let mut bin = cvtt(div(sub(dot, first), bin_size));
            if bin < 0 {
                bin = 0;
            }
            if nbins <= bin {
                bin = nbins;
            }
            if bin == nbins && nbins >= 3 && !rep {
                past += 1;
                bin = nbins - 1;
            }
            count_at(g, h, bin, "mrisComputeNormalDotDistribution");
        }
    }
    if past > 0 {
        g.notices.add(
            &FS_626,
            format!(
                "mrisComputeNormalDotDistribution: {past} dots counted in the last bin, where FreeSurfer counts them one past it"
            ),
        );
    }
    normalise_counts(h, num);
    let mut raw = histo_copy(g, h, None).unwrap_new();
    // FreeSurfer's own correction of HISTOcopy (FS-21)
    raw.bin_size = h.bin_size;
    histo_smooth(g, &raw, Some(h), 2.0);
    if diag_write(g) {
        histo_plot(h, dir, "ndot.plt");
        histo_plot(&raw, dir, "rdot.plt");
    }
    positions::restore_vertex_positions(g, io, mris, TMP_VERTICES)?;
    mp::compute_metric_properties(g, sg, io, mris)?;
    Ok(NO_ERROR)
}

/// The k1-k2 volume's first slice: its columns and rows the floor and normalisation cover
/// (FS-628: FreeSurfer's 100 x 100 with `reproduce`, the slice's own size by default).
fn k1k2_extent(g: &mut Globals, mri: &Mri) -> (i32, i32) {
    let (w, h) = (mri.width, mri.height);
    if w == 100 && h == 100 {
        return (100, 100);
    }
    if g.notices.reproduce() {
        g.notices.add(
            &FS_628,
            format!(
                "mrisComputePrincipalCurvatureDistributions with a {w} x {h} k1-k2 volume: its first 100 x 100 voxels floored and normalised (FreeSurfer's)"
            ),
        );
        (100, 100)
    } else {
        g.notices.add(
            &FS_628,
            format!(
                "mrisComputePrincipalCurvatureDistributions with a {w} x {h} k1-k2 volume: its whole first slice floored and normalised (FreeSurfer: its first 100 x 100 voxels)"
            ),
        );
        (w, h)
    }
}

/// `mrisComputePrincipalCurvatureDistributions(mris, h_k1, h_k2, mri_k1_k2)`: the second
/// fundamental form computed (`MRIScomputeSecondFundamentalForm`); the unripped vertices' range
/// of `k1` and of `k2` (from 100000 and -100000), each kept within -3 and 3; each histogram's bin
/// size its range over its bins, its bins from the low end in steps of it; the volume's `xsize`,
/// `ysize` the ranges over its width and height, `xstart`, `ystart` the low ends; each unripped
/// vertex counted in its `k1` and `k2` bins (truncated, NaN 0, kept within the bins) and at its
/// voxel `(k1, k2)` of the volume's first slice (`+= 1`, kept within its columns and rows); every
/// count in use 0 made 0.01 and all divided by the number of unripped vertices; the slice's
/// voxels below `FLT_EPSILON` in magnitude made 0.1, their sum (column by column, each column's
/// rows in order) dividing each. With `DIAG_WRITE` and `DIAG_VERBOSE` the histograms are plotted
/// to `k1.plt`, `k2.plt` and the volume written to `mri_k1_k2.mgh` (in `dir`). `NO_ERROR`.
///
/// **FS-628:** FreeSurfer floors and normalises the first 100 x 100 voxels whatever the volume's
/// size (every caller passes 100 x 100 floats). A volume narrower or lower than that, or not of
/// floats, is read and written outside its voxels: both modes give the error
/// `mrisComputePrincipalCurvatureDistributions: the k1-k2 volume must hold floats and be 100 x 100
/// or larger (<w> x <h>, type <t>)` and `ERROR_BADPARM`, nothing changed. A larger one: by
/// default its whole first slice, with `reproduce` the corner (its other voxels raw counts).
///
/// **FS-626:** a histogram without bins counts every vertex one before its first bin (memory
/// corrupted): by default the error `mrisComputePrincipalCurvatureDistributions: a histogram of 0
/// bins` and `ERROR_BADPARM`, nothing changed; `reproduce` computes, those counts dropped.
#[allow(clippy::too_many_arguments)]
pub fn compute_principal_curvature_distributions(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    h_k1: &mut Histogram,
    h_k2: &mut Histogram,
    mri: &mut Mri,
) -> Result<i32, Fatal> {
    const NAME: &str = "mrisComputePrincipalCurvatureDistributions";
    if mri.mtype != MRI_FLOAT || mri.width < 100 || mri.height < 100 || mri.chunk.is_none() {
        g.notices.add(
            &FS_628,
            format!(
                "{NAME} with a {} x {} k1-k2 volume of type {}: an error (FreeSurfer reads and writes outside its voxels)",
                mri.width, mri.height, mri.mtype
            ),
        );
        log::error_printf(
            g,
            io.err,
            ERROR_BADPARM,
            &format!(
                "{NAME}: the k1-k2 volume must hold floats and be 100 x 100 or larger ({} x {}, type {})",
                mri.width, mri.height, mri.mtype
            ),
        );
        return Ok(ERROR_BADPARM);
    }
    if h_k1.nbins <= 0 || h_k2.nbins <= 0 {
        let n = if h_k1.nbins <= 0 {
            h_k1.nbins
        } else {
            h_k2.nbins
        };
        if !g.notices.reproduce() {
            g.notices.add(
                &FS_626,
                format!(
                    "{NAME} with a histogram of {n} bins: an error (FreeSurfer counts every vertex before its first bin)"
                ),
            );
            log::error_printf(
                g,
                io.err,
                ERROR_BADPARM,
                &format!("{NAME}: a histogram of {n} bins"),
            );
            return Ok(ERROR_BADPARM);
        }
    }
    props6::compute_second_fundamental_form(g, sg, io, dir, mris)?;
    let (mut min_k1, mut min_k2) = (100000f32, 100000f32);
    let (mut max_k1, mut max_k2) = (-100000f32, -100000f32);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        if v.k1 < min_k1 {
            min_k1 = v.k1;
        }
        if v.k2 < min_k2 {
            min_k2 = v.k2;
        }
        if v.k1 > max_k1 {
            max_k1 = v.k1;
        }
        if v.k2 > max_k2 {
            max_k2 = v.k2;
        }
    }
    // MAX(-3, min) and MIN(3, max)
    let lo = |x: f32| if -3.0 > x { -3.0 } else { x };
    let hi = |x: f32| if 3.0 < x { 3.0 } else { x };
    (min_k1, max_k1) = (lo(min_k1), hi(max_k1));
    (min_k2, max_k2) = (lo(min_k2), hi(max_k2));
    let (r1, r2) = (sub(max_k1, min_k1), sub(max_k2, min_k2));
    let k1_bin_size = div(r1, h_k1.nbins as f32);
    let k2_bin_size = div(r2, h_k2.nbins as f32);
    mri.vg.xsize = div(r1, mri.width as f32);
    mri.vg.ysize = div(r2, mri.height as f32);
    mri.xstart = min_k1;
    mri.ystart = min_k2;
    h_k1.bin_size = k1_bin_size;
    h_k2.bin_size = k2_bin_size;
    for (h, start, size) in [
        (&mut *h_k1, min_k1, k1_bin_size),
        (&mut *h_k2, min_k2, k2_bin_size),
    ] {
        let mut bin_val = start;
        for b in 0..usize::try_from(h.nbins).unwrap_or(0) {
            h.bins[b] = bin_val;
            bin_val = add(bin_val, size);
        }
    }
    let (w, ht) = (mri.width, mri.height);
    let (xstart, ystart, xsize, ysize) = (mri.xstart, mri.ystart, mri.vg.xsize, mri.vg.ysize);
    let mut nvertices = 0i32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let (k1, k2) = (v.k1, v.k2);
        nvertices += 1;
        let clamp = |q: i32, n: i32| {
            let q = if q < 0 { 0 } else { q };
            if q >= n { n - 1 } else { q }
        };
        let b1 = clamp(cvtt(div(sub(k1, min_k1), k1_bin_size)), h_k1.nbins);
        count_at(g, h_k1, b1, NAME);
        let b2 = clamp(cvtt(div(sub(k2, min_k2), k2_bin_size)), h_k2.nbins);
        count_at(g, h_k2, b2, NAME);
        let x = clamp(cvtt(div(sub(k1, xstart), xsize)), w);
        let y = clamp(cvtt(div(sub(k2, ystart), ysize)), ht);
        let c = mri_get_vox_val(g, io.err, mri, x, y, 0, 0, 0)?;
        mri_set_vox_val(g, io.err, mri, x, y, 0, 0, add(c, 1.0), 0)?;
    }
    normalise_counts(h_k1, nvertices);
    normalise_counts(h_k2, nvertices);
    let (ex, ey) = k1k2_extent(g, mri);
    for x in 0..ex {
        for y in 0..ey {
            let c = mri_get_vox_val(g, io.err, mri, x, y, 0, 0, 0)?;
            if fzero(c) {
                mri_set_vox_val(g, io.err, mri, x, y, 0, 0, 0.1, 0)?;
            }
        }
    }
    let mut norm = 0f32;
    for x in 0..ex {
        for y in 0..ey {
            norm = add(norm, mri_get_vox_val(g, io.err, mri, x, y, 0, 0, 0)?);
        }
    }
    for x in 0..ex {
        for y in 0..ey {
            let c = mri_get_vox_val(g, io.err, mri, x, y, 0, 0, 0)?;
            mri_set_vox_val(g, io.err, mri, x, y, 0, 0, div(c, norm), 0)?;
        }
    }
    if diag_write(g) {
        histo_plot(h_k1, dir, "k1.plt");
        histo_plot(h_k2, dir, "k2.plt");
        mriio::mri_write(
            g,
            io.out,
            io.err,
            io.env,
            dir,
            mri,
            "mri_k1_k2.mgh",
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
    }
    Ok(NO_ERROR)
}

// ---- Ashburner's triangle energy ----------------------------------------------------------------

/// `get_face_axes(mris, face, ...)`: in `double` from the vertices' current positions, the unit
/// vector `e1` along the face's first edge and `e2` the unit vector of the second edge
/// orthogonalised to it (Gram-Schmidt); either length below `FLT_EPSILON` (`FZERO`) gives the
/// first vertex's own tangent axes (`e1x` .. `e2z`) instead. As `float`.
fn face_axes(mris: &Mris, fno: usize) -> ([f32; 3], [f32; 3]) {
    let f = mris.face(fno).v;
    let p = |k: usize| {
        let v = mris.vertex(f[k] as usize);
        [f64::from(v.x), f64::from(v.y), f64::from(v.z)]
    };
    let (v0, v1, v2) = (p(0), p(1), p(2));
    let fallback = || {
        let v = mris.vertex(f[0] as usize);
        ([v.e1x, v.e1y, v.e1z], [v.e2x, v.e2y, v.e2z])
    };
    let len = |e: [f64; 3]| {
        sqrt64(add64(
            add64(mul64(e[0], e[0]), mul64(e[1], e[1])),
            mul64(e[2], e[2]),
        ))
    };
    let mut e1 = [0f64; 3];
    for k in 0..3 {
        e1[k] = sub64(v1[k], v0[k]);
    }
    let n1 = len(e1);
    if fzero64(n1) {
        return fallback();
    }
    let mut e2 = [0f64; 3];
    for k in 0..3 {
        e2[k] = sub64(v2[k], v0[k]);
    }
    let n2 = len(e2);
    if fzero64(n2) {
        return fallback();
    }
    for k in 0..3 {
        e1[k] = div64(e1[k], n1);
        e2[k] = div64(e2[k], n2);
    }
    let dot = add64(
        add64(mul64(e1[0], e2[0]), mul64(e1[1], e2[1])),
        mul64(e1[2], e2[2]),
    );
    // the official build: e1 x dot, e1 y dot, dot e1 z
    let tmp = [mul64(e1[0], dot), mul64(e1[1], dot), mul64(dot, e1[2])];
    for k in 0..3 {
        e2[k] = sub64(e2[k], tmp[k]);
    }
    let n3 = len(e2);
    if fzero64(n3) {
        return fallback();
    }
    for k in 0..3 {
        e2[k] = div64(e2[k], n3);
    }
    (e1.map(|x| x as f32), e2.map(|x| x as f32))
}

/// `log(SMALL) * log(SMALL)` (`SMALL` 1e-8): the energy of a triangle whose map cannot be used.
fn unusable(libm: &Libm) -> f64 {
    let a = libm.log(1e-8);
    mul64(libm.log(1e-8), a)
}

/// `ashburnerTriangleEnergy(mris, fno, lambda)`: the face's canonical and current positions in
/// its tangent plane ([`face_axes`]), the affine map between them (`MatrixInverse` of the
/// canonical positions' 3 x 3 matrix, `MatrixMultiply`), its 2 x 2 Jacobian `J`
/// (`MatrixCopyRegion`) and determinant (`MatrixDeterminant`); `log(SMALL)²` when the inverse
/// fails, the determinant is negative, or an eigenvalue of `J` (`MatrixEigenSystem`) is not
/// finite; else each eigenvalue (at most 0 made 1e-8) as `logf(s)²` and the energy `lambda (1 +
/// det) (s11 + s22) / 2`.
fn ashburner_triangle_energy(
    g: &mut Globals,
    io: &mut Io,
    libm: &Libm,
    mris: &Mris,
    fno: usize,
    lambda: f32,
) -> Result<f64, Fatal> {
    let (e1, e2) = face_axes(mris, fno);
    let [e1x, e1y, e1z] = e1;
    let [e2x, e2y, e2z] = e2;
    let f = mris.face(fno).v;
    let (v1, v2, v3) = (
        mris.vertex(f[0] as usize),
        mris.vertex(f[1] as usize),
        mris.vertex(f[2] as usize),
    );
    // the official build's operand orders (`mrisurf_metricProperties.cpp.o`): the e1 rows' sums
    // commuted, y13 and y23 commuted throughout
    let x1 = |v: &Vertex| add(add(mul(v.cy, e1y), mul(v.cx, e1x)), mul(v.cz, e1z));
    let x2 = |v: &Vertex| add(add(mul(v.cx, e2x), mul(v.cy, e2y)), mul(v.cz, e2z));
    let y1 = |v: &Vertex| add(add(mul(v.y, e1y), mul(v.x, e1x)), mul(v.z, e1z));
    let y2 = |v: &Vertex| add(add(mul(v.x, e2x), mul(v.y, e2y)), mul(v.z, e2z));
    let y13 = add(mul(e1z, v3.z), add(mul(e1y, v3.y), mul(e1x, v3.x)));
    let y23 = add(mul(e2z, v3.z), add(mul(e2y, v3.y), mul(v3.x, e2x)));
    let mut m_x = Matrix::real(3, 3);
    let mut m_y = Matrix::real(3, 3);
    let xs = [[x1(v1), x1(v2), x1(v3)], [x2(v1), x2(v2), x2(v3)]];
    let ys = [[y1(v1), y1(v2), y13], [y2(v1), y2(v2), y23]];
    for r in 0..2 {
        for c in 0..3 {
            m_x.set_relt(r as i32 + 1, c as i32 + 1, xs[r][c]);
            m_y.set_relt(r as i32 + 1, c as i32 + 1, ys[r][c]);
        }
    }
    for c in 1..=3 {
        m_x.set_relt(3, c, 1.0);
        m_y.set_relt(3, c, 1.0);
    }
    let mut m_x_inv = Matrix::real(3, 3);
    if matrix_inverse(g, io.out, io.err, Some(&m_x), Some(&mut m_x_inv))?.is_null() {
        return Ok(unusable(libm));
    }
    let mut m_m = Matrix::real(3, 3);
    matrix::matrix_multiply(
        g,
        io.out,
        io.err,
        Some(&m_y),
        Some(&m_x_inv),
        Some(&mut m_m),
    )?;
    let mut m_j = Matrix::real(2, 2);
    matrix_copy_region(
        g,
        io.out,
        io.err,
        Region::All,
        &m_m,
        Some(&mut m_j),
        1,
        1,
        2,
        2,
        1,
        1,
    )?;
    let det = matrix_determinant(g, io.err, &m_j);
    if det < 0.0 {
        return Ok(unusable(libm));
    }
    let mut evalues = [0f32; 2];
    // FreeSurfer keeps each thread's eigenvector matrix between calls; every element is written
    // again, so a new one each call gives the same values. A matrix VNL never returns on
    // (ITK-112: FreeSurfer hangs) is refused: the triangle counts as unusable.
    if matrix_eigen_system(g, io.out, io.err, &m_j, &mut evalues, None)?.is_null() {
        return Ok(unusable(libm));
    }
    let (mut s11, mut s22) = (evalues[0], evalues[1]);
    if !s11.is_finite() || !s22.is_finite() {
        return Ok(unusable(libm));
    }
    if s11 <= 0.0 {
        s11 = 1e-8;
    }
    s11 = libm.logf(s11);
    s11 = mul(s11, s11);
    if s22 <= 0.0 {
        s22 = 1e-8;
    }
    s22 = libm.logf(s22);
    s22 = mul(s22, s22);
    // lambda * (1 + det) * (s11 + s22), as the official build orders it
    let energy = mul64(
        f64::from(add(s22, s11)),
        mul64(f64::from(add(det, 1.0)), f64::from(lambda)),
    );
    Ok(mul64(energy, 0.5))
}

/// `mrisSampleAshburnerTriangleEnergy(mris, vno, parms, cx, cy, cz)` with `parms`'
/// `l_ashburner_lambda` as `lambda`: the point projected onto the sphere of the surface's radius
/// (`project_point_onto_sphere`, the radius as `float`), the vertex moved there, the mean of
/// Ashburner's energy ([`ashburner_triangle_energy`]) over its faces (`double`; 0 faces give the
/// NaN of 0 / 0), the vertex's position restored. As `float`. `vno` must be one of the surface's
/// vertices.
#[allow(clippy::too_many_arguments)]
pub fn sample_ashburner_triangle_energy(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    vno: usize,
    lambda: f32,
    cx: f32,
    cy: f32,
    cz: f32,
) -> Result<f32, Fatal> {
    let libm = Libm::quiet(g.mode());
    let (cx, cy, cz) = project_point_onto_sphere(cx, cy, cz, mris.radius as f32);
    let saved = {
        let v = mris.vertex(vno);
        (v.x, v.y, v.z)
    };
    {
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (cx, cy, cz);
    }
    let faces: Vec<i32> = {
        let vt = mris.topology(vno);
        let f = vt.f.as_deref().unwrap_or(&[]);
        f[..usize::from(vt.num).min(f.len())].to_vec()
    };
    let mut total = 0f64;
    let mut res = Ok(());
    for &fno in &faces {
        match ashburner_triangle_energy(g, io, &libm, mris, fno as usize, lambda) {
            // the official build: sse + total
            Ok(sse) => total = add64(sse, total),
            Err(e) => {
                res = Err(e);
                break;
            }
        }
    }
    {
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = saved;
    }
    res?;
    Ok(div64(total, f64::from(mris.topology(vno).num)) as f32)
}
