// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The rest of `utils/mrisurf_base.cpp`'s structure functions (`fs.surfio`, queue item 3.1-47):
//! the rip flags (lines 777-870), the coordinate, normal and face arrays (871-934), the curvature
//! names and the registration's blurring scales (937-984), the extra gradients and the NaN check
//! (1572-1599), the column printing (`cprints`, `cprintd`, `cprintf`: 1632-1704, FS-119), the
//! vector searches (1706-1752), the progress bar (1755-1790, FS-118), `signum_eval` and the two
//! sort comparisons (1792-1842).
//!
//! Not here: `vlst_alloc`, `vlst_free`, `vlst_enough_data` and `vlst_add_to_list` (they take
//! `VOXEL_LIST`, `utils/voxlist.cpp`: fs.volume's), `mrisDirectionTriangleIntersection` and
//! `mrisAllNormalDirectionCurrentTriangleIntersections` (they take the surface hash, fs.surf's
//! `MRIS_HASH_TABLE`), and `slprints` (no caller in FreeSurfer 8.2.0; it prints the clock and the
//! host's name). The edge intersection test is [`crate::edges`].

use std::io::Write;

use larmorx_core::defects::{FS_118, FS_119};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_NO_MEMORY, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::utils::dev_isnan;

use crate::aaa::SurfGlobals;
use crate::facenorm::get_face_norm;
use crate::mris::Mris;

/// `G_LC` and `G_RC` (`include/mrisurf.h`): the left and right columns' widths.
pub const G_LC: usize = 50;
pub const G_RC: usize = 30;

/// `ENOMEM` (Linux's errno for an allocation that failed).
const ENOMEM: i32 = 12;

fn nonneg(n: i32) -> usize {
    usize::try_from(n).unwrap_or(0)
}

/// `MRISexportFaceRipflags(mris)`: a new array of the faces' rip flags; `NULL` for a negative face
/// count (`malloc` of the count as a size fails), an empty array for none.
pub fn export_face_ripflags(mris: &Mris) -> Option<Vec<i8>> {
    if mris.nfaces < 0 {
        return None;
    }
    Some(
        (0..nonneg(mris.nfaces))
            .map(|f| mris.face(f).ripflag)
            .collect(),
    )
}

/// `MRISimportFaceRipflags(mris, flags)`: each in-use face's rip flag from `flags`.
pub fn import_face_ripflags(mris: &mut Mris, flags: &[i8]) {
    for fno in 0..nonneg(mris.nfaces) {
        mris.face_mut(fno).ripflag = flags[fno];
    }
}

/// `MRISexportVertexRipflags(mris)`: as [`export_face_ripflags`] for the vertices.
pub fn export_vertex_ripflags(mris: &Mris) -> Option<Vec<i8>> {
    if mris.nvertices < 0 {
        return None;
    }
    Some(
        (0..nonneg(mris.nvertices))
            .map(|v| mris.vertex(v).ripflag)
            .collect(),
    )
}

/// `MRISimportVertexRipflags(mris, flags)`.
pub fn import_vertex_ripflags(mris: &mut Mris, flags: &[i8]) {
    for vno in 0..nonneg(mris.nvertices) {
        mris.vertex_mut(vno).ripflag = flags[vno];
    }
}

/// `MRIScountRipped(mris)`: the number of in-use vertices with a rip flag that is not 0.
pub fn count_ripped(mris: &Mris) -> i32 {
    (0..nonneg(mris.nvertices))
        .filter(|&v| mris.vertex(v).ripflag != 0)
        .count() as i32
}

/// `MRISstoreRipFlags(mris)`: each in-use vertex's and face's `oripflag` = its `ripflag`.
pub fn store_rip_flags(mris: &mut Mris) -> i32 {
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex_mut(vno);
        v.oripflag = v.ripflag;
    }
    for fno in 0..nonneg(mris.nfaces) {
        let f = mris.face_mut(fno);
        f.oripflag = f.ripflag;
    }
    NO_ERROR
}

/// `MRISrestoreRipFlags(mris)`: each in-use vertex's and face's `ripflag` = its `oripflag`.
pub fn restore_rip_flags(mris: &mut Mris) -> i32 {
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex_mut(vno);
        v.ripflag = v.oripflag;
    }
    for fno in 0..nonneg(mris.nfaces) {
        let f = mris.face_mut(fno);
        f.ripflag = f.oripflag;
    }
    NO_ERROR
}

/// `MRISunrip(mris)`: every in-use vertex's and face's rip flag 0.
pub fn unrip(mris: &mut Mris) -> i32 {
    for vno in 0..nonneg(mris.nvertices) {
        mris.vertex_mut(vno).ripflag = 0;
    }
    for fno in 0..nonneg(mris.nfaces) {
        mris.face_mut(fno).ripflag = 0;
    }
    NO_ERROR
}

/// `nvertices * 3` places (`new float[n * 3]`): a negative count is C++'s
/// `std::bad_array_new_length`, out of range here (a panic).
fn three(n: i32) -> usize {
    3 * usize::try_from(n).expect("a negative count: new[] throws std::bad_array_new_length")
}

/// `MRISgetVertexArray(mris)`: the in-use vertices' `x`, `y`, `z`, one after the other.
pub fn get_vertex_array(mris: &Mris) -> Vec<f32> {
    let mut b = Vec::with_capacity(three(mris.nvertices));
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex(vno);
        b.extend([v.x, v.y, v.z]);
    }
    b
}

/// `MRISgetVertexNormalArray(mris)`: the in-use vertices' `nx`, `ny`, `nz`.
pub fn get_vertex_normal_array(mris: &Mris) -> Vec<f32> {
    let mut b = Vec::with_capacity(three(mris.nvertices));
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex(vno);
        b.extend([v.nx, v.ny, v.nz]);
    }
    b
}

/// `MRISgetFaceArray(mris)`: the in-use faces' three vertex numbers.
pub fn get_face_array(mris: &Mris) -> Vec<i32> {
    let mut b = Vec::with_capacity(three(mris.nfaces));
    for fno in 0..nonneg(mris.nfaces) {
        b.extend(mris.face(fno).v);
    }
    b
}

/// `MRISgetFaceNormalArray(mris)`: the in-use faces' normals from the face-normal cache
/// (`getFaceNorm`: a deferred face's normal is computed first).
pub fn get_face_normal_array(mris: &mut Mris) -> Vec<f32> {
    let mut b = Vec::with_capacity(three(mris.nfaces));
    for fno in 0..nonneg(mris.nfaces) {
        let e = get_face_norm(mris, fno);
        b.extend([e.nx, e.ny, e.nz]);
    }
    b
}

/// `MRISsetCurvatureName(nth, name)`: the `nth` curvature name (0 to 2) becomes a copy of `name`
/// (its bytes before a NUL); returns 0. For `nth` above 2: `ERROR: MRISsetCurvatureName() nth=<n>
/// > 2` on standard output, 1. A negative `nth` writes before FreeSurfer's array (FreeSurfer's
/// callers give 0, 1 and 2): out of range here (a panic).
pub fn set_curvature_name(
    sg: &mut SurfGlobals,
    out: &mut dyn Write,
    nth: i32,
    name: &[u8],
) -> i32 {
    if nth > 2 {
        let _ = writeln!(out, "ERROR: MRISsetCurvatureName() nth={nth} > 2");
        return 1;
    }
    let k = usize::try_from(nth).expect("MRISsetCurvatureName with a negative nth");
    let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    sg.curvature_names[k] = Some(name[..end].to_vec());
    0
}

/// `MRISprintCurvatureNames(fp)`: on standard output (FreeSurfer ignores `fp`), for each of the
/// three: `<k> <curvature name>`, or `<k> <surface name> (computed)` where the curvature name is
/// `NULL` and the surface name is not. Returns 0.
pub fn print_curvature_names(sg: &SurfGlobals, out: &mut dyn Write) -> i32 {
    for k in 0..3 {
        if let Some(c) = &sg.curvature_names[k] {
            let _ = write!(out, "{k} ");
            let _ = out.write_all(c);
            let _ = writeln!(out);
        } else if let Some(s) = &sg.surface_names[k] {
            let _ = write!(out, "{k} ");
            let _ = out.write_all(s);
            let _ = writeln!(out, " (computed)");
        }
    }
    0
}

/// `MRISsetRegistrationSigmas(sigmas, nsigmas)`: the scales the spherical registration blurs with
/// and their count (a `double`); `None` restores the default four (4, 2, 1, 0.5). FreeSurfer keeps
/// the caller's array; here its values are copied (the same while the caller leaves the array as
/// it was, as FreeSurfer's callers do). Returns `NO_ERROR`.
pub fn set_registration_sigmas(sg: &mut SurfGlobals, sigmas: Option<&[f32]>, nsigmas: i32) -> i32 {
    match sigmas {
        None => {
            sg.sigmas = crate::aaa::SIGMAS_DEFAULT.to_vec();
            sg.nsigmas = crate::aaa::SIGMAS_DEFAULT.len() as f64;
        }
        Some(s) => {
            sg.sigmas = s.to_vec();
            sg.nsigmas = f64::from(nsigmas);
        }
    }
    NO_ERROR
}

/// `MRISallocExtraGradients(mris)`: three zeroed arrays of one float per in-use vertex (`dx2`,
/// `dy2`, `dz2`), unless `dx2` exists already. A negative vertex count makes the allocations fail:
/// `Cannot allocate memory` (the failed allocation's `errno`), `MRISallocExtraGradients: could
/// allocate gradient vectors` and the end of the program (`ErrorExit`, `ERROR_NO_MEMORY`). Returns `NO_ERROR`.
pub fn alloc_extra_gradients(
    g: &mut Globals,
    err: &mut dyn Write,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    if mris.dx2.is_some() {
        return Ok(NO_ERROR);
    }
    if mris.nvertices < 0 {
        // calloc's failure sets errno, which ErrorExit reports first
        g.errno = ENOMEM;
        log::error_exit(
            g,
            err,
            ERROR_NO_MEMORY,
            "MRISallocExtraGradients: could allocate gradient vectors",
        )?;
        return Ok(NO_ERROR);
    }
    let n = nonneg(mris.nvertices);
    mris.dx2 = Some(vec![0.0; n]);
    mris.dy2 = Some(vec![0.0; n]);
    mris.dz2 = Some(vec![0.0; n]);
    Ok(NO_ERROR)
}

/// `MRIScheckForNans(mris)`: for each in-use vertex, `odx`, `ody`, `odz`, `dx`, `dy`, `dz` in turn;
/// the first NaN fails `cheapAssert(!devIsnan(f))` (`mrisurf_base.cpp` line 1587) (FS-03).
pub fn check_for_nans(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mris: &Mris,
) -> Result<(), Fatal> {
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex(vno);
        for f in [v.odx, v.ody, v.odz, v.dx, v.dy, v.dz] {
            if dev_isnan(f) != 0 {
                return Err(diag::assert_failed(
                    g,
                    out,
                    err,
                    &format!("{}/utils/mrisurf_base.cpp", diag::BUILD_ROOT),
                    1587,
                    "!devIsnan(f)",
                ));
            }
        }
    }
    Ok(())
}

/// `printf("%*s", width, s)` on bytes: right-aligned in at least `width` bytes.
fn pad_bytes(err: &mut dyn Write, width: usize, s: &[u8]) {
    for _ in s.len()..width {
        let _ = err.write_all(b" ");
    }
    let _ = err.write_all(s);
}

fn cstr(s: &[u8]) -> &[u8] {
    &s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())]
}

/// `cprints(left, right)`: on standard error, `left` right-aligned in 50 bytes (when not empty),
/// then ` [ <right> ]` and a newline right-aligned in 30 (when `right` is not empty).
///
/// FreeSurfer first writes ` [ <right> ]\n` into a 16,384-byte buffer the official build checks
/// (`_FORTIFY_SOURCE`): a `right` of 16,378 bytes or more ends the program before anything is
/// printed, with `*** buffer overflow detected ***: terminated` and signal 6 (FS-119). larmorx
/// prints it whole, with FS-119's notice; `reproduce` gives the original's end (exit status 134).
pub fn cprints(
    g: &mut Globals,
    err: &mut dyn Write,
    left: &[u8],
    right: &[u8],
) -> Result<(), Fatal> {
    let (left, right) = (cstr(left), cstr(right));
    if right.len() + 7 > 16384 {
        g.notices.add(
            &FS_119,
            "a column text of 16,378 bytes or more is printed whole (the original aborts)",
        );
        if g.notices.reproduce() {
            let _ = err.write_all(b"*** buffer overflow detected ***: terminated\n");
            return Err(Fatal::Exit(134));
        }
    }
    let mut pch_right = b" [ ".to_vec();
    pch_right.extend_from_slice(right);
    pch_right.extend_from_slice(b" ]\n");
    if !left.is_empty() {
        pad_bytes(err, G_LC, left);
    }
    if !right.is_empty() {
        pad_bytes(err, G_RC, &pch_right);
    }
    let _ = err.flush();
    Ok(())
}

fn cprint_value(err: &mut dyn Write, left: &[u8], right: String) {
    let left = cstr(left);
    if !left.is_empty() {
        pad_bytes(err, G_LC, left);
    } else {
        pad_bytes(err, G_LC, b" ");
    }
    pad_bytes(err, G_RC, format!(" [ {right} ]\n").as_bytes());
    let _ = err.flush();
}

/// `cprintd(left, right)`: on standard error, `left` (or a blank) right-aligned in 50 bytes, then
/// ` [ <right> ]` and a newline right-aligned in 30.
pub fn cprintd(err: &mut dyn Write, left: &[u8], right: i32) {
    cprint_value(err, left, right.to_string());
}

/// `cprintf(left, right)`: as [`cprintd`] with the float printed `%f`.
pub fn cprintf(err: &mut dyn Write, left: &[u8], right: f32) {
    cprint_value(err, left, fmt_f(f64::from(right), 0, 6));
}

/// `VECTOR_elementIndex_findNotEqual(v, term)`: for the first row `i` (from 0) whose element
/// `(i + 1, 1)` is not equal to `term` (`!=`: a NaN never equals), `i` as a `short`; 0 when every
/// element equals it. (FreeSurfer's comment promises the 1-based index; the code returns the
/// 0-based one, so a difference in the first row and none at all both give 0. No caller in
/// FreeSurfer 8.2.0.)
pub fn vector_element_index_find_not_equal(v: &Matrix, term: f32) -> i16 {
    for i in 0..v.rows.max(0) {
        if v.relt(i + 1, 1) != term {
            return i as i16;
        }
    }
    0
}

/// `VECTOR_elementIndex_find(v, term)`: the 1-based row of the first element `(i, 1)` equal to
/// `term`, as a `short`; 0 when none is.
pub fn vector_element_index_find(v: &Matrix, term: f32) -> i16 {
    for i in 1..=v.rows.max(0) {
        if v.relt(i, 1) == term {
            return i as i16;
        }
    }
    0
}

/// `MRIS_vertexProgress_print(mris, vertex, message)`: the progress bar of a loop over the
/// surface's vertices, on standard error: at vertex 0 the message right-aligned in 50 bytes (when
/// there is one) and ` [`; a `#` at each vertex whose number modulo `fivePerc` (the integer part of
/// 0.05 times the vertex count) is `fivePerc - 1`; at the last vertex `] ` and, with a message,
/// `[ ok ]` and a newline. Returns 1.
///
/// A surface of 1 to 19 vertices has `fivePerc` 0: FreeSurfer's modulo by 0 ends the program with
/// an arithmetic exception (signal 8) at the first call, after the message and ` [` (FS-118).
/// larmorx prints no `#` there, with FS-118's notice; `reproduce` gives the original's end (exit
/// status 136).
pub fn vertex_progress_print(
    g: &mut Globals,
    err: &mut dyn Write,
    mris: &Mris,
    avertex: i32,
    message: Option<&[u8]>,
) -> Result<i16, Fatal> {
    let five_perc = (0.05 * f64::from(mris.nvertices)) as i32;
    if avertex == 0 {
        if let Some(m) = message {
            pad_bytes(err, G_LC, cstr(m));
        }
        let _ = err.write_all(b" [");
    }
    if five_perc == 0 {
        g.notices.add(
            &FS_118,
            "a progress bar over fewer than 20 vertices prints no # (the original divides by 0)",
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(136));
        }
    } else if avertex.wrapping_rem(five_perc) == five_perc - 1 {
        let _ = err.write_all(b"#");
    }
    if avertex == mris.nvertices.wrapping_sub(1) {
        let _ = err.write_all(b"] ");
        if message.is_some() {
            let _ = err.write_all(b"[ ok ]\n");
        }
    }
    Ok(1)
}

/// `signum_eval(af)`: -1 when `af < 0`, else 1 (a NaN gives 1).
pub fn signum_eval(af: f32) -> i32 {
    if af < 0.0 { -1 } else { 1 }
}

/// `VERTEX_SORT` (`include/mrisurf.h`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexSort {
    pub vtxno: i32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// `FACE_SORT` (`include/mrisurf.h`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FaceSort {
    pub faceno: i32,
    pub v0: i32,
    pub v1: i32,
    pub v2: i32,
}

/// `CompareVertexCoords(v1, v2)` (a `qsort` comparison): by `y`, then `x`, then `z` (`<` gives -1,
/// `>` 1), and 1 where all three are neither below nor above (it never gives 0; a NaN compares
/// neither way).
pub fn compare_vertex_coords(a: &VertexSort, b: &VertexSort) -> i32 {
    if a.y < b.y {
        return -1;
    }
    if a.y > b.y {
        return 1;
    }
    if a.x < b.x {
        return -1;
    }
    if a.x > b.x {
        return 1;
    }
    if a.z < b.z {
        return -1;
    }
    1
}

/// `CompareFaceVertices(f1, f2)` (a `qsort` comparison): by `v0`, `v1`, `v2`; 1 for equal faces
/// (never 0).
pub fn compare_face_vertices(a: &FaceSort, b: &FaceSort) -> i32 {
    if a.v0 < b.v0 {
        return -1;
    }
    if a.v0 > b.v0 {
        return 1;
    }
    if a.v1 < b.v1 {
        return -1;
    }
    if a.v1 > b.v1 {
        return 1;
    }
    if a.v2 < b.v2 {
        return -1;
    }
    1
}
