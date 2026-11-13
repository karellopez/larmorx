// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The face-normal cache (`fs.surfio`, queue item 3.1-50, part a): FreeSurfer 8.2.0's
//! `utils/mrisurf_metricProperties.cpp` lines 7633-7875 for an `MRIS`
//! (`computeDefectFaceNormal_calculate_template`, `computeDefectFaceNormal_calculate`,
//! `computeDefectFaceNormal`, `setFaceNorm`, `setFaceOrigArea`, `getFaceOrigArea` (FS-116),
//! `getFaceNorm`, `mrisurf_deferSetFaceNorms`, `recomputeOrUndeferFaceNorms`,
//! `mrisurf_recomputeFaceNorms`, `mrisurf_undeferSetFaceNorms`).
//!
//! Each face has a cached normal and original area (`MRIS::faceNormCacheEntries`) and a deferral
//! byte (`MRIS::faceNormDeferredEntries`: bit 1 the normal, bit 2 the area still to compute). The
//! normal is the original coordinates' (`origx`, `origy`, `origz`): the cross product of the edges
//! from the face's first vertex, normalised, and half its length the area; a cross product
//! shorter than `FLT_EPSILON` is taken again from the second vertex with the edges scaled by 100,
//! and a second short one keeps the unnormalised vector with length 1. Every operation is the C
//! expression's, in the official build's order (read from the unit oracle's object code:
//! `docs/findings/freesurfer-surfio.md` §10).
//!
//! The `MRIS_MP` overloads read and write an `MRIS_MP`'s face-normal arrays, which FreeSurfer
//! never fills (`crate::mris_mp`); their callers are queue items 3.1-51 and 3.1-52's.
//! `mrisurf_deferSetFaceNorms` reads `FREESURFER_deferSetFaceNorms_parallel` once per process to
//! choose a parallel loop; both loops store the same bytes, so the variable changes nothing seen.

use larmorx_core::defects::FS_116;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::sse::{add, div, mul, sqrt, sub};

use crate::aaa::FaceNormCacheEntry;
use crate::coords::{Io, metric_properties_cpp};
use crate::mris::Mris;

/// `FLT_EPSILON` (`FZERO`).
const FLT_EPSILON: f32 = f32::EPSILON;

fn fzero(x: f32) -> bool {
    x.abs() < FLT_EPSILON
}

/// `F_CROSS(a, b, d)` (`include/mrisurf_base.h`), each product and difference in the macro's
/// order.
fn f_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        sub(mul(a[1], b[2]), mul(b[1], a[2])),
        sub(mul(a[2], b[0]), mul(b[2], a[0])),
        sub(mul(a[0], b[1]), mul(b[0], a[1])),
    ]
}

fn len3(n: [f32; 3]) -> f32 {
    sqrt(add(add(mul(n[0], n[0]), mul(n[1], n[1])), mul(n[2], n[2])))
}

/// `computeDefectFaceNormal_calculate(mris, fno, ...)`: the face's normal and original area from
/// its vertices' original coordinates, `(nx, ny, nz, orig_area)`. A vertex number outside the
/// surface is FreeSurfer's invalid read: a panic.
pub fn compute_defect_face_normal_calculate(mris: &Mris, fno: usize) -> (f32, f32, f32, f32) {
    let f = mris.face(fno);
    let o = |k: usize| {
        let v = mris.vertex(f.v[k] as usize);
        [v.origx, v.origy, v.origz]
    };
    let (v1, v2, v3) = (o(0), o(1), o(2));
    let a: [f32; 3] = std::array::from_fn(|k| sub(v2[k], v1[k]));
    let b: [f32; 3] = std::array::from_fn(|k| sub(v3[k], v1[k]));
    let mut n = f_cross(a, b);
    let mut len = len3(n);
    if fzero(len) {
        // `100.0 * (v3 - v2)` in double, rounded to float: the same value as the float product
        // the official build computes
        let a: [f32; 3] = std::array::from_fn(|k| mul(sub(v3[k], v2[k]), 100.0));
        let b: [f32; 3] = std::array::from_fn(|k| mul(sub(v1[k], v2[k]), 100.0));
        n = f_cross(a, b);
        len = len3(n);
        if fzero(len) {
            len = 1.0;
        }
    }
    (
        div(n[0], len),
        div(n[1], len),
        div(n[2], len),
        div(len, 2.0),
    )
}

fn entries(mris: &mut Mris) -> &mut Vec<FaceNormCacheEntry> {
    mris.faceNormCacheEntries
        .as_mut()
        .expect("MRIS::faceNormCacheEntries is NULL")
}

fn deferred(mris: &mut Mris) -> &mut Vec<crate::aaa::FaceNormDeferredEntry> {
    mris.faceNormDeferredEntries
        .as_mut()
        .expect("MRIS::faceNormDeferredEntries is NULL")
}

/// `setFaceNorm(mris, fno, nx, ny, nz)`: the face's cached normal; its normal no longer deferred
/// (bit 1 cleared).
pub fn set_face_norm(mris: &mut Mris, fno: usize, nx: f32, ny: f32, nz: f32) {
    let e = &mut entries(mris)[fno];
    (e.nx, e.ny, e.nz) = (nx, ny, nz);
    deferred(mris)[fno].deferred &= !1;
}

/// `setFaceOrigArea(mris, fno, orig_area)`: the face's cached original area; no longer deferred
/// (bit 2 cleared).
pub fn set_face_orig_area(mris: &mut Mris, fno: usize, orig_area: f32) {
    entries(mris)[fno].orig_area = orig_area;
    deferred(mris)[fno].deferred &= !2;
}

/// `getFaceOrigArea(mris, fno)`: the face's cached original area.
///
/// FS-116 (corrected by default, rule A): FreeSurfer never wrote the case of an area still
/// deferred: it prints `<file>:7747 NYI` and writes to address -1 on purpose (a segmentation
/// fault). larmorx computes the area as the deferred cache would ([`get_face_norm`]'s), stores
/// it and clears bit 2 (the normal's deferral stays); `reproduce` prints the message and ends the
/// program with exit status 139 (the status a shell reports for the signal).
pub fn get_face_orig_area(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    fno: usize,
) -> Result<f32, Fatal> {
    if deferred(mris)[fno].deferred & 2 != 0 {
        g.notices.add(
            &FS_116,
            "a face's original area still deferred is computed, not a deliberate crash",
        );
        if g.notices.reproduce() {
            let _ = writeln!(io.err, "{}:7747 NYI", metric_properties_cpp());
            return Err(Fatal::Exit(139));
        }
        let (_, _, _, orig_area) = compute_defect_face_normal_calculate(mris, fno);
        set_face_orig_area(mris, fno, orig_area);
    }
    Ok(entries(mris)[fno].orig_area)
}

/// `getFaceNorm(mris, fno)`: the face's cache entry; when anything of it is deferred (a nonzero
/// byte), the normal is computed and stored, the original area too when bit 2 is set, and the
/// byte cleared.
pub fn get_face_norm(mris: &mut Mris, fno: usize) -> FaceNormCacheEntry {
    let d = deferred(mris)[fno].deferred;
    if d != 0 {
        let (nx, ny, nz, orig_area) = compute_defect_face_normal_calculate(mris, fno);
        let e = &mut entries(mris)[fno];
        (e.nx, e.ny, e.nz) = (nx, ny, nz);
        if d & 2 != 0 {
            e.orig_area = orig_area;
        }
        deferred(mris)[fno].deferred = 0;
    }
    entries(mris)[fno]
}

/// `computeDefectFaceNormal(mris, fno)`: the face's normal and original area computed and set
/// ([`set_face_norm`], [`set_face_orig_area`]).
pub fn compute_defect_face_normal(mris: &mut Mris, fno: usize) {
    let (nx, ny, nz, orig_area) = compute_defect_face_normal_calculate(mris, fno);
    set_face_norm(mris, fno, nx, ny, nz);
    set_face_orig_area(mris, fno, orig_area);
}

fn nfaces(mris: &Mris) -> usize {
    usize::try_from(mris.nfaces).unwrap_or(0)
}

/// `mrisurf_deferSetFaceNorms(mris)`: every face's normal and area deferred (byte 3).
pub fn defer_set_face_norms(mris: &mut Mris) {
    let n = nfaces(mris);
    if n == 0 {
        return;
    }
    for e in &mut deferred(mris)[..n] {
        e.deferred = 3;
    }
}

fn recompute_or_undefer(mris: &mut Mris, is_recompute: bool) {
    for fno in 0..nfaces(mris) {
        if !is_recompute && deferred(mris)[fno].deferred == 0 {
            continue;
        }
        let (nx, ny, nz, orig_area) = compute_defect_face_normal_calculate(mris, fno);
        let e = &mut entries(mris)[fno];
        (e.nx, e.ny, e.nz, e.orig_area) = (nx, ny, nz, orig_area);
        deferred(mris)[fno].deferred = 0;
    }
}

/// `mrisurf_recomputeFaceNorms(mris)`: every face's normal and area computed and stored, nothing
/// deferred.
pub fn recompute_face_norms(mris: &mut Mris) {
    recompute_or_undefer(mris, true);
}

/// `mrisurf_undeferSetFaceNorms(mris)`: every face with a nonzero deferral byte gets its normal
/// and area computed and stored (both, whatever the byte's bits), the byte cleared.
pub fn undefer_set_face_norms(mris: &mut Mris) {
    recompute_or_undefer(mris, false);
}
