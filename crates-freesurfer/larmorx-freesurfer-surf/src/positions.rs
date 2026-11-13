// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The saved vertex positions and normals, the canonical coordinates (`fs.surfio`, queue item
//! 3.1-50, part a): FreeSurfer 8.2.0's `utils/mrisurf_metricProperties.cpp` lines 3170-3450 and
//! 3668-3720 (`MRISsaveVertexPositions`, `MRISrestoreVertexPositions` (FS-115),
//! `MRISsaveNormals`, `MRISrestoreNormals`, `MRIScomputeFolding`,
//! `MRIScomputeCanonicalCoordinates`), with `MRISaverageCanonicalRadius` (lines 11619-11666, queue
//! item 3.1-51's lines, which the canonical coordinates call).
//!
//! The arithmetic is the C expressions' order, which the official build keeps (read from the unit
//! oracle's object code, `docs/findings/freesurfer-surfio.md` §10; the bounding box's `>` and `<`
//! compiled as `maxsd`/`minsd`, which take the C comparison's choice, NaNs included); `atan2f` is
//! glibc's with `reproduce` (fs.core's [`Libm`]).

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use larmorx_core::defects::FS_115;
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::nint;

use crate::aaa::{self, MrisStatus};
use crate::coords::{self, Io, metric_properties_cpp};
use crate::mris::{Mris, Vertex};
use crate::views::{
    CANONICAL_VERTICES, FLATTENED_VERTICES, INFLATED_VERTICES, LAYERIV_VERTICES, ORIGINAL_VERTICES,
    PIAL_VERTICES, TARGET_VERTICES, TMP_VERTICES, TMP2_VERTICES, WHITE_VERTICES,
};

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// The coordinate set `which` of a vertex, as `MRISsaveVertexPositions` names them (the current
/// coordinates' destinations): `None` for any other `which`.
fn saved_set(v: &mut Vertex, which: i32) -> Option<(&mut f32, &mut f32, &mut f32)> {
    Some(match which {
        LAYERIV_VERTICES => (&mut v.l4x, &mut v.l4y, &mut v.l4z),
        TARGET_VERTICES => (&mut v.targx, &mut v.targy, &mut v.targz),
        WHITE_VERTICES => (&mut v.whitex, &mut v.whitey, &mut v.whitez),
        PIAL_VERTICES => (&mut v.pialx, &mut v.pialy, &mut v.pialz),
        INFLATED_VERTICES => (&mut v.infx, &mut v.infy, &mut v.infz),
        FLATTENED_VERTICES => (&mut v.fx, &mut v.fy, &mut v.fz),
        CANONICAL_VERTICES => (&mut v.cx, &mut v.cy, &mut v.cz),
        TMP2_VERTICES => (&mut v.t2x, &mut v.t2y, &mut v.t2z),
        TMP_VERTICES => (&mut v.tx, &mut v.ty, &mut v.tz),
        _ => return None,
    })
}

/// `MRISsaveVertexPositions(mris, which)`: the current coordinates copied into the set `which`
/// (`ORIGINAL_VERTICES`: `MRISsetOriginalXYZfromXYZ`, which also frees the original distances and
/// sets `origxyz_status`); after `CANONICAL_VERTICES` the canonical coordinates' angles are
/// computed ([`compute_canonical_coordinates`]). Any other `which` fails `cheapAssert(false)`
/// (line 3230) at the first vertex (FS-03; nothing without vertices). Returns `NO_ERROR`.
pub fn save_vertex_positions(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    which: i32,
) -> Result<i32, Fatal> {
    if which == ORIGINAL_VERTICES {
        coords::set_original_xyz_from_xyz(mris);
    } else {
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            let (x, y, z) = (v.x, v.y, v.z);
            match saved_set(v, which) {
                Some((a, b, c)) => (*a, *b, *c) = (x, y, z),
                None => {
                    return Err(diag::assert_failed(
                        g,
                        io.out,
                        io.err,
                        &metric_properties_cpp(),
                        3230,
                        "false",
                    ));
                }
            }
        }
    }
    if which == CANONICAL_VERTICES {
        compute_canonical_coordinates(g, mris);
    }
    Ok(0)
}

/// The set `MRISrestoreVertexPositions` copies from: the named ones, and the temporary one for
/// any other `which` (C's `default:` beside `case TMP_VERTICES:`).
fn restored_set(v: &Vertex, which: i32) -> (f32, f32, f32) {
    match which {
        TARGET_VERTICES => (v.targx, v.targy, v.targz),
        WHITE_VERTICES => (v.whitex, v.whitey, v.whitez),
        LAYERIV_VERTICES => (v.l4x, v.l4y, v.l4z),
        PIAL_VERTICES => (v.pialx, v.pialy, v.pialz),
        INFLATED_VERTICES => (v.infx, v.infy, v.infz),
        FLATTENED_VERTICES => (v.fx, v.fy, v.fz),
        CANONICAL_VERTICES => (v.cx, v.cy, v.cz),
        ORIGINAL_VERTICES => (v.origx, v.origy, v.origz),
        TMP2_VERTICES => (v.t2x, v.t2y, v.t2z),
        _ => (v.tx, v.ty, v.tz),
    }
}

fn same_bits(a: (f32, f32, f32), b: (f32, f32, f32)) -> bool {
    a.0.to_bits() == b.0.to_bits()
        && a.1.to_bits() == b.1.to_bits()
        && a.2.to_bits() == b.2.to_bits()
}

/// `MRISrestoreVertexPositions(mris, which)`: `dist_nsize` 0, then every vertex's current
/// coordinates copied from the set `which` (the temporary set for a `which` not named, as
/// `TMP_VERTICES`); on a spherical patch every unripped vertex's canonical coordinates then
/// become the same set (the canonical ones stay), and the bounding box is recomputed
/// (`mrisComputeSurfaceDimensions`). With `ORIGINAL_VERTICES` it first checks that the surface's
/// kind and the original coordinates' kind are compatible (a debug message when not).
///
/// FS-115 (corrected by default, rule A): on a spherical patch FreeSurfer's second copy has no
/// case for the target and layer-IV sets, so after restoring one of them the canonical coordinates
/// are the temporary set, not the restored one; larmorx copies the restored set (noted where a
/// vertex's canonical coordinates change by it); `reproduce` copies the temporary set. Returns
/// `NO_ERROR`.
pub fn restore_vertex_positions(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    which: i32,
) -> Result<i32, Fatal> {
    if which == ORIGINAL_VERTICES {
        aaa::check_orig_xyz_compatible(g, io.out, io.err, mris.status, mris.origxyz_status)?;
    }
    mris.dist_nsize = 0;
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = restored_set(v, which);
    }
    if mris.status == MrisStatus::SPHERICAL_PATCH {
        let fs115 = which == TARGET_VERTICES || which == LAYERIV_VERTICES;
        let mut changed = false;
        for vno in 0..nverts(mris) {
            let v = mris.vertex(vno);
            if v.ripflag != 0 || which == CANONICAL_VERTICES {
                continue;
            }
            let official = restored_set(v, TMP_VERTICES);
            let set = if fs115 {
                let restored = restored_set(v, which);
                changed |= !same_bits(official, restored);
                if g.notices.reproduce() {
                    official
                } else {
                    restored
                }
            } else {
                restored_set(v, which)
            };
            let v = mris.vertex_mut(vno);
            (v.cx, v.cy, v.cz) = set;
        }
        if changed {
            g.notices.add(
                &FS_115,
                "the canonical coordinates of a spherical patch take the restored target or layer-IV \
                 coordinates, not the temporary ones",
            );
        }
    }
    coords::compute_surface_dimensions(mris);
    Ok(0)
}

/// `MRISsaveNormals(mris, which)`: the normals copied into the temporary, second temporary, pial
/// or white normals (`TMP_VERTICES`, `TMP2_VERTICES`, `PIAL_VERTICES`, `WHITE_VERTICES`); any other
/// `which` is the fatal error `MRISsaveNormals: unsupported which <n>` (`ERROR_BADPARM`) at the
/// first vertex (none without vertices). Returns `NO_ERROR`.
pub fn save_normals(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    which: i32,
) -> Result<i32, Fatal> {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        let n = (v.nx, v.ny, v.nz);
        match which {
            TMP_VERTICES => (v.tx, v.ty, v.tz) = n,
            TMP2_VERTICES => (v.t2x, v.t2y, v.t2z) = n,
            PIAL_VERTICES => (v.pnx, v.pny, v.pnz) = n,
            WHITE_VERTICES => (v.wnx, v.wny, v.wnz) = n,
            _ => log::error_exit(
                g,
                io.err,
                ERROR_BADPARM,
                &format!("MRISsaveNormals: unsupported which {which}"),
            )?,
        }
    }
    Ok(0)
}

/// `MRISrestoreNormals(mris, which)`: the normals copied back from the temporary, second
/// temporary or pial normals; any other `which`, the white normals included, is the fatal error
/// `MRISsaveNormals: unsupported which <n>` (the message names the saving function) at the first
/// vertex. Returns `NO_ERROR`.
pub fn restore_normals(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    which: i32,
) -> Result<i32, Fatal> {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        match which {
            TMP_VERTICES => (v.nx, v.ny, v.nz) = (v.tx, v.ty, v.tz),
            TMP2_VERTICES => (v.nx, v.ny, v.nz) = (v.t2x, v.t2y, v.t2z),
            PIAL_VERTICES => (v.nx, v.ny, v.nz) = (v.pnx, v.pny, v.pnz),
            _ => log::error_exit(
                g,
                io.err,
                ERROR_BADPARM,
                &format!("MRISsaveNormals: unsupported which {which}"),
            )?,
        }
    }
    Ok(0)
}

/// `MRIScomputeFolding(mris)`: the sum over the unripped vertices of `area * (k1 - k2) * (k1 -
/// k2)`, in double.
pub fn compute_folding(mris: &Mris) -> f64 {
    let mut folding = 0.0f64;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let d = sse::sub64(f64::from(v.k1), f64::from(v.k2));
        folding = sse::add64(folding, sse::mul64(sse::mul64(f64::from(v.area), d), d));
    }
    folding
}

/// `MRISaverageCanonicalRadius(mris)`: the mean distance of the canonical coordinates (in
/// double) from the centre of their bounding box (from 10000 and -10000; a NaN coordinate never
/// moves it); with no vertex `0.0 / 0.0`, SSE's default NaN.
pub fn average_canonical_radius(mris: &Mris) -> f64 {
    let (mut lo, mut hi) = ([10000.0f64; 3], [-10000.0f64; 3]);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        let p = [f64::from(v.cx), f64::from(v.cy), f64::from(v.cz)];
        for k in 0..3 {
            if p[k] > hi[k] {
                hi[k] = p[k];
            }
            if p[k] < lo[k] {
                lo[k] = p[k];
            }
        }
    }
    // (lo + hi) / 2.0f, compiled as a product by 0.5 (the same value, NaNs included)
    let c: [f64; 3] = std::array::from_fn(|k| sse::mul64(sse::add64(lo[k], hi[k]), 0.5));
    let mut radius = 0.0f64;
    let mut n = 0i32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        n += 1;
        let x = sse::sub64(f64::from(v.cx), c[0]);
        let y = sse::sub64(f64::from(v.cy), c[1]);
        let z = sse::sub64(f64::from(v.cz), c[2]);
        let s = sse::add64(
            sse::add64(sse::mul64(x, x), sse::mul64(y, y)),
            sse::mul64(z, z),
        );
        radius = sse::add64(radius, sse::sqrt64(s));
    }
    sse::div64(radius, f64::from(n))
}

/// `atan2f(y, x)`: glibc's with `reproduce`, correctly rounded otherwise ([`Libm`]); of NaN
/// arguments the NaN of `x + y` in both (glibc's, `x` the destination), on every platform.
fn atan2f(libm: &Libm, y: f32, x: f32) -> f32 {
    if x.is_nan() || y.is_nan() {
        return sse::add(x, y);
    }
    libm.atan2f(y, x)
}

/// `MRIScomputeCanonicalCoordinates(mris)`: the surface's radius becomes the rounded
/// ([`nint`], FS-12) mean canonical radius ([`average_canonical_radius`]), and every vertex's
/// `theta` = `atan2f(cy, cx)` (plus `2π`, in double, when negative) and `phi` = `atan2f(√d, cz)`
/// with `d` = `r² - cz²` in float (0 when negative). Returns `NO_ERROR`.
pub fn compute_canonical_coordinates(g: &mut Globals, mris: &mut Mris) -> i32 {
    let libm = Libm::quiet(g.mode());
    mris.radius = average_canonical_radius(mris);
    let r = nint(g, mris.radius) as f32;
    mris.radius = f64::from(r);
    let r2 = sse::mul(r, r);
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        let (x, y, z) = (v.cx, v.cy, v.cz);
        let mut theta = atan2f(&libm, y, x);
        if theta < 0.0 {
            theta = (2.0 * std::f64::consts::PI + f64::from(theta)) as f32;
        }
        let mut d = sse::sub(r2, sse::mul(z, z));
        if d < 0.0 {
            d = 0.0;
        }
        let phi = atan2f(&libm, sse::sqrt(d), z);
        v.theta = theta;
        v.phi = phi;
    }
    0
}
