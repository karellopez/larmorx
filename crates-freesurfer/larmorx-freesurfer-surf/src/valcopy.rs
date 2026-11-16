// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The copies between a surface's vertex fields and its vectors: FreeSurfer 8.2.0's
//! `utils/mrisurf_vals.cpp` `MRISsetCroppedToZero` (line 400), `MRISaddToValues` (496),
//! `MRISimportCurvatureVector` (712), `MRISimportValVector` (724), `MRISimportValFromMatrixColumn`
//! (736), `MRISexportValVector` (748), `MRISexportValVectorDouble` (760), the two-operand copies
//! (857-1253: `MRIScopyMarkedToMarked2` ... `MRIScopyValToVal2Bak`), `MRIScopyFromCropped` (1235),
//! `MRIScopyToCropped` (1255), `MRISorigAreaToCurv` (1279), `MRISareaToCurv` (1294),
//! `MRISmarkedToCurv` (1309), `MRIScurvToMarked` (1324), `MRISdToCurv` (1339), `MRIScurvToD`
//! (1354), `MRIScopyFixedValFlagsToMarks` (1369) and `MRISclearFaceMarks` (1386) (relay 38 of
//! track D). FS-908.
//!
//! Each two-operand copy but the marks' and the annotation's skips ripped vertices; a `float` to
//! an integer field converts as x86-64 does (`cvttss2si`: `INT_MIN` for NaN and out of range, then
//! the field's low bits), as [`crate::surfmri`]'s copies.

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use larmorx_core::defects::FS_908;
use larmorx_freesurfer_core::cstd::double_to_int_x86;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::Matrix;

use crate::coords::Io;
use crate::mris::{Mris, Vertex};

/// `VERTEX_VALS` (`include/mrisurf.h` 1610).
pub const VERTEX_VALS: i32 = 1;
/// `VERTEX_MARKS` (`include/mrisurf.h` 1621).
pub const VERTEX_MARKS: i32 = 11;

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// C's conversion of a `float` to `int` (`cvttss2si`).
fn to_int(v: f32) -> i32 {
    double_to_int_x86(f64::from(v))
}

/// Every vertex whose rip flag is 0 changed by `f` (the loops of most of this file).
fn unripped(mris: &mut Mris, f: impl Fn(&mut Vertex)) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            f(v);
        }
    }
    NO_ERROR
}

/// Every vertex changed by `f`.
fn every(mris: &mut Mris, f: impl Fn(&mut Vertex)) -> i32 {
    for vno in 0..nverts(mris) {
        f(mris.vertex_mut(vno));
    }
    NO_ERROR
}

/// `MRISsetCroppedToZero(mris)`: `cropped` 0 (vertices not ripped).
pub fn set_cropped_to_zero(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.cropped = 0)
}

/// `MRISaddToValues(mris, val)`: `val` added to each value (`float`; vertices not ripped).
pub fn add_to_values(mris: &mut Mris, val: f32) -> i32 {
    unripped(mris, |v| {
        v.val = larmorx_freesurfer_core::sse::add(v.val, val)
    })
}

/// A vector of the caller's as long as the surface's vertices, or FS-908 noted (FreeSurfer reads
/// outside it): the vertices beyond it are left alone.
fn vector_len(g: &mut Globals, mris: &Mris, len: usize, what: &str) -> usize {
    let nv = nverts(mris);
    if len < nv {
        g.notices.add(
            &FS_908,
            format!(
                "{what}: a vector of {len} values for a surface of {nv} vertices: the vertices beyond it left alone (FreeSurfer reads outside the vector)"
            ),
        );
    }
    len.min(nv)
}

/// `MRISimportCurvatureVector(mris, curvs)`: each vertex's `curv` from `curvs`. FS-908.
pub fn import_curvature_vector(g: &mut Globals, mris: &mut Mris, curvs: &[f32]) -> i32 {
    for vno in 0..vector_len(g, mris, curvs.len(), "MRISimportCurvatureVector") {
        mris.vertex_mut(vno).curv = curvs[vno];
    }
    NO_ERROR
}

/// `MRISimportValVector(mris, vals)`: each vertex's `val` from `vals`. FS-908.
pub fn import_val_vector(g: &mut Globals, mris: &mut Mris, vals: &[f32]) -> i32 {
    for vno in 0..vector_len(g, mris, vals.len(), "MRISimportValVector") {
        mris.vertex_mut(vno).val = vals[vno];
    }
    NO_ERROR
}

/// `MRISimportValFromMatrixColumn(mris, m, col)`: each vertex's `val` from row `vno + 1` of
/// column `col` of `m`. FS-908: a matrix with fewer rows than vertices, or a column outside it,
/// leaves those vertices alone (FreeSurfer reads outside the matrix).
pub fn import_val_from_matrix_column(
    g: &mut Globals,
    mris: &mut Mris,
    m: &Matrix,
    col: i32,
) -> i32 {
    let mut outside = 0usize;
    for vno in 0..nverts(mris) {
        let r = vno as i32 + 1;
        if (1..=m.cols).contains(&col) && r <= m.rows {
            mris.vertex_mut(vno).val = m.relt(r, col);
        } else {
            outside += 1;
        }
    }
    if outside > 0 {
        g.notices.add(
            &FS_908,
            format!(
                "MRISimportValFromMatrixColumn: column {col} of a {} x {} matrix for {} vertices: {outside} left alone (FreeSurfer reads outside the matrix)",
                m.rows, m.cols, mris.nvertices
            ),
        );
    }
    NO_ERROR
}

/// `MRISexportValVector(mris, vals)`: each vertex's `val`.
pub fn export_val_vector(mris: &Mris) -> Vec<f32> {
    (0..nverts(mris)).map(|vno| mris.vertex(vno).val).collect()
}

/// `MRISexportValVectorDouble(mris, vals, offset)`: each vertex's `val` widened into
/// `vals[vno + offset]` (the vector grown to hold them).
pub fn export_val_vector_double(mris: &Mris, vals: &mut Vec<f64>, offset: usize) -> i32 {
    let nv = nverts(mris);
    if vals.len() < offset + nv {
        vals.resize(offset + nv, 0.0);
    }
    for vno in 0..nv {
        let x = mris.vertex(vno).val;
        // `cvtss2sd`: a NaN keeps its sign and payload, quieted
        vals[vno + offset] = if x.is_nan() {
            let b = x.to_bits();
            f64::from_bits(
                (u64::from(b & 0x8000_0000) << 32)
                    | 0x7ff8_0000_0000_0000
                    | (u64::from(b & 0x003f_ffff) << 29),
            )
        } else {
            f64::from(x)
        };
    }
    NO_ERROR
}

/// `MRIScopyMarkedToMarked2(mris)`: `marked2 = marked` (every vertex).
pub fn copy_marked_to_marked2(mris: &mut Mris) -> i32 {
    every(mris, |v| v.marked2 = v.marked)
}

/// `MRIScopyMarked2ToMarked(mris)`: `marked = marked2` (every vertex).
pub fn copy_marked2_to_marked(mris: &mut Mris) -> i32 {
    every(mris, |v| v.marked = v.marked2)
}

/// `MRIScopyMarkedToMarked3(mris)`: `marked3 = marked` (every vertex).
pub fn copy_marked_to_marked3(mris: &mut Mris) -> i32 {
    every(mris, |v| v.marked3 = v.marked)
}

/// `MRIScopyMarked3ToMarked(mris)`: `marked = marked3` (every vertex).
pub fn copy_marked3_to_marked(mris: &mut Mris) -> i32 {
    every(mris, |v| v.marked = v.marked3)
}

/// `MRIScopyValsToAnnotations(mris)`: `annotation = (int) val` (every vertex).
pub fn copy_vals_to_annotations(mris: &mut Mris) -> i32 {
    every(mris, |v| v.annotation = to_int(v.val))
}

/// `MRIScopyMeansToValues(mris)`: `val = mean`.
pub fn copy_means_to_values(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.val = v.mean)
}

/// `MRIScopyImaginaryMeansToValues(mris)`: `val = mean_imag`.
pub fn copy_imaginary_means_to_values(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.val = v.mean_imag)
}

/// `MRIScopyStandardErrorsToValues(mris)`: `val = std_error`.
pub fn copy_standard_errors_to_values(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.val = v.std_error)
}

/// `MRIScopyStatsToValues(mris)`: `val = stat`.
pub fn copy_stats_to_values(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.val = v.stat)
}

/// `MRIScopyStatsFromValues(mris)`: `stat = val`.
pub fn copy_stats_from_values(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.stat = v.val)
}

/// `MRIScopyValuesToImagValues(mris)`: `imag_val = val`.
pub fn copy_values_to_imag_values(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.imag_val = v.val)
}

/// `MRIScopyImagValuesToValues(mris)`: `val = imag_val`.
pub fn copy_imag_values_to_values(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.val = v.imag_val)
}

/// `MRIScopyVal2ToVal(mris)`: `val = val2`.
pub fn copy_val2_to_val(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.val = v.val2)
}

/// `MRIScopyCurvatureToValues(mris)`: `val = curv`.
pub fn copy_curvature_to_values(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.val = v.curv)
}

/// `MRIScopyVal2BakToVal(mris)`: `val = val2bak`.
pub fn copy_val2bak_to_val(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.val = v.val2bak)
}

/// `MRIScopyCurvatureToImagValues(mris)`: `imag_val = curv`.
pub fn copy_curvature_to_imag_values(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.imag_val = v.curv)
}

/// `MRIScopyValuesToCurvature(mris)` and `MRIScopyCurvatureFromValues(mris)`: `curv = val`.
pub fn copy_values_to_curvature(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.curv = v.val)
}

/// `MRIScopyImagValuesToCurvature(mris)` and `MRIScopyCurvatureFromImagValues(mris)`: `curv =
/// imag_val`.
pub fn copy_imag_values_to_curvature(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.curv = v.imag_val)
}

/// `MRIScopyValToVal2(mris)`: `val2 = val`.
pub fn copy_val_to_val2(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.val2 = v.val)
}

/// `MRIScopyValuesToVal2Bak(mris)` and `MRIScopyValToVal2Bak(mris)`: `val2bak = val`.
pub fn copy_values_to_val2bak(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.val2bak = v.val)
}

/// `MRIScopyValToValBak(mris)`: `valbak = val`.
pub fn copy_val_to_valbak(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.valbak = v.val)
}

/// `MRIScopyFromCropped(mris, which)` (`to` false) and `MRIScopyToCropped(mris, which)` (`to`
/// true): `val` or `marked` from or to `cropped` ([`VERTEX_VALS`], [`VERTEX_MARKS`]; vertices not
/// ripped); any other `which` ends the program at the first vertex not ripped
/// (`MRIScopyFromCropped: <which> unsupported target`, both functions' wording; `ERROR_UNSUPPORTED`).
pub fn copy_cropped(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    which: i32,
    to: bool,
) -> Result<i32, Fatal> {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        match (which, to) {
            (VERTEX_VALS, false) => v.val = v.cropped as f32,
            (VERTEX_MARKS, false) => v.marked = v.cropped as i16,
            (VERTEX_VALS, true) => v.cropped = to_int(v.val),
            (VERTEX_MARKS, true) => v.cropped = i32::from(v.marked),
            _ => {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_UNSUPPORTED,
                    &format!("MRIScopyFromCropped: {which} unsupported target"),
                )?;
            }
        }
    }
    Ok(NO_ERROR)
}

/// `MRISorigAreaToCurv(mris)`: `curv = origarea`.
pub fn orig_area_to_curv(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.curv = v.origarea)
}

/// `MRISareaToCurv(mris)`: `curv = area`.
pub fn area_to_curv(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.curv = v.area)
}

/// `MRISmarkedToCurv(mris)`: `curv = (float) marked`.
pub fn marked_to_curv(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.curv = f32::from(v.marked))
}

/// `MRIScurvToMarked(mris)`: `marked = (int) curv` (its low 16 bits).
pub fn curv_to_marked(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.marked = to_int(v.curv) as i16)
}

/// `MRISdToCurv(mris)`: `curv = d`.
pub fn d_to_curv(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.curv = v.d)
}

/// `MRIScurvToD(mris)`: `d = curv`.
pub fn curv_to_d(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.d = v.curv)
}

/// `MRIScopyFixedValFlagsToMarks(mris)`: `marked = fixedval` (its low 16 bits).
pub fn copy_fixed_val_flags_to_marks(mris: &mut Mris) -> i32 {
    unripped(mris, |v| v.marked = v.fixedval as i16)
}

/// `MRISclearFaceMarks(mris)`: every face's `marked` 0.
pub fn clear_face_marks(mris: &mut Mris) -> i32 {
    for fno in 0..usize::try_from(mris.nfaces).unwrap_or(0) {
        mris.face_mut(fno).marked = 0;
    }
    NO_ERROR
}
