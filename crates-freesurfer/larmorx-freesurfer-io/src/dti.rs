// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/dti.cpp`'s b-value files and names: `DTIwriteBValues` (848), `DTIwriteBVectors` (931),
//! `DTIparsePulseSeqName` (980) and `DTIbvecChangeSpace` (1083), which `MRIwrite` (the `.bvals`
//! and `.bvecs` files beside a volume that has them) and the DICOM readers call. The rest of
//! `dti.cpp` (the tensor fits, the b-value readers) comes with the items that reach it.
//!
//! Defects (`docs/findings/freesurfer-io.md`, Part 3): FS-217 (no matrix: a crash after the file
//! is made; in both modes the file made empty and 1 returned), FS-218 (fewer columns than written:
//! rule B, refused by default; `reproduce` the next rows' numbers and 0 for memory past the
//! matrix), FS-219 (a name's 100-byte buffer: the whole text in both modes), FS-220 (gradient
//! directions of fewer than 3 columns: rule B; `reproduce` the reads and writes across rows, 0
//! for memory past the matrix, writes outside it dropped).

use std::io::Write;
use std::path::Path;

use larmorx_core::cscan::{sscanf_f64, sscanf_i32};
use larmorx_core::defects::{FS_217, FS_218, FS_219, FS_220};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::fsenv::{BVEC_SPACE_SCANNER, BVEC_SPACE_VOXEL};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply_d};
use larmorx_freesurfer_core::matrix_ops::{matrix_inverse, matrix_print};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::vol_geom::matrix_of_direction_cosines;

use crate::cfile::CFile;

/// The number at (`r`, `c`) as FreeSurfer reads it past a row's end: the array's float there, or
/// `None` past the matrix's last number (memory contents).
fn cell(m: &Matrix, r: i32, c: i32) -> Option<f32> {
    m.relt_index(r, c).and_then(|i| m.raw(i))
}

/// Writes `rows` lines of the first `ncols` columns of `m`, each number with `fmt`, after the
/// FS-218 checks; `what` names the function.
#[allow(clippy::too_many_arguments)]
fn write_columns(
    g: &mut Globals,
    out: &mut dyn Write,
    dir: Option<&Path>,
    m: Option<&Matrix>,
    file: &str,
    what: &str,
    ncols: i32,
    fmt: &dyn Fn(f64) -> String,
    sep: &str,
) -> i32 {
    let reproduce = g.notices.reproduce();
    if let Some(m) = m
        && m.cols < ncols
    {
        g.notices.add(
            &FS_218,
            format!(
                "{what} of a matrix of {} columns: FreeSurfer writes the next rows' numbers in the {ncols} columns it writes, and memory contents past the matrix{}",
                m.cols,
                if reproduce { "" } else { "; refused" }
            ),
        );
        if !reproduce {
            let _ = writeln!(
                out,
                "ERROR: {what}(): the matrix has {} columns, {ncols} needed",
                m.cols
            );
            return 1;
        }
    }
    let Ok(mut fp) = CFile::open(dir, file, "w") else {
        let _ = writeln!(out, "ERROR: {what}(): could not open {file} for writing");
        return 1;
    };
    let Some(m) = m else {
        fp.close();
        g.notices.add(
            &FS_217,
            format!("{what} of no matrix: FreeSurfer crashes after making the file; the file left empty"),
        );
        let _ = writeln!(out, "ERROR: {what}(): no matrix");
        return 1;
    };
    let mut memory = false;
    let mut text = String::new();
    for n in 1..=m.rows {
        for c in 1..=ncols {
            let v = cell(m, n, c).unwrap_or_else(|| {
                memory = true;
                0.0
            });
            text.push_str(&fmt(f64::from(v)));
            text.push_str(if c < ncols { sep } else { "\n" });
        }
    }
    if memory {
        g.notices.add(
            &FS_218,
            format!(
                "{what} of a matrix of {} columns: FreeSurfer writes memory contents past the matrix's last number; written as 0",
                m.cols
            ),
        );
    }
    fp.put_bytes(text.as_bytes());
    fp.close();
    0
}

/// `DTIwriteBValues(bvals, bvalfile)`: the first column, one `%f` a line; 1 after
/// `ERROR: DTIwriteBValues(): could not open <file> for writing` on standard output.
pub fn dti_write_bvalues(
    g: &mut Globals,
    out: &mut dyn Write,
    dir: Option<&Path>,
    bvals: Option<&Matrix>,
    bvalfile: &str,
) -> i32 {
    write_columns(
        g,
        out,
        dir,
        bvals,
        bvalfile,
        "DTIwriteBValues",
        1,
        &|v| fmt_f(v, 0, 6),
        "",
    )
}

/// `DTIwriteBVectors(bvecs, bvecfile)`: the first three columns, each `%16.14f`, a line per row.
pub fn dti_write_bvectors(
    g: &mut Globals,
    out: &mut dyn Write,
    dir: Option<&Path>,
    bvecs: Option<&Matrix>,
    bvecfile: &str,
) -> i32 {
    write_columns(
        g,
        out,
        dir,
        bvecs,
        bvecfile,
        "DTIwriteBVectors",
        3,
        &|v| fmt_f(v, 16, 14),
        " ",
    )
}

/// The length of FreeSurfer's buffer for each part of a pulse-sequence name.
const PSN_BUFFER: usize = 100;

/// `DTIparsePulseSeqName(pulseseq, &bValue, &nthDirection)` for a name `ep_b<b>#<n>`: 1 (the
/// outputs unchanged) for a name of fewer than 7 bytes or without `#` after its fifth; else 0,
/// `bvalue` set when the text between the fifth byte and `#` starts with a number (`%lf`) and
/// `nth` when the rest does (`%d`). The first four bytes are not looked at. A name with a NUL
/// ends there, as a C string.
pub fn dti_parse_pulse_seq_name(
    g: &mut Globals,
    pulseseq: &[u8],
    bvalue: &mut f64,
    nth: &mut i32,
) -> i32 {
    let s = &pulseseq[..pulseseq
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(pulseseq.len())];
    if s.len() < 7 {
        return 1;
    }
    let rest = &s[4..];
    let Some(hash) = rest.iter().position(|&c| c == b'#') else {
        if rest.len() > PSN_BUFFER {
            g.notices.add(
                &FS_219,
                format!(
                    "a pulse-sequence name with {} bytes and no #: FreeSurfer copies them into a {PSN_BUFFER}-byte buffer (a crash or wrong values)",
                    rest.len()
                ),
            );
        }
        return 1;
    };
    let (b, n) = (&rest[..hash], &rest[hash + 1..]);
    if b.len() >= PSN_BUFFER || n.len() >= PSN_BUFFER {
        g.notices.add(
            &FS_219,
            format!(
                "a pulse-sequence name with parts of {} and {} bytes: FreeSurfer copies them into a {PSN_BUFFER}-byte buffer (a crash or wrong values); read whole",
                b.len(),
                n.len()
            ),
        );
    }
    if let Some(v) = sscanf_f64(b) {
        *bvalue = v;
    }
    if let Some(v) = sscanf_i32(n) {
        *nth = v;
    }
    0
}

/// `DTIbvecChangeSpace(vol, desired_bvec_space)`: the gradient directions (`vol.bvecs`, the first
/// three columns of each row) moved from scanner to voxel space (by the inverse of the direction
/// cosines) or back, as 4-vectors with 1 multiplied in double and stored as floats; then
/// `vol.bvec_space` is the desired space. Prints `DTIbvecChangeSpace(): <space> <desired>` first;
/// 1 after `ERROR: DTIbvecChangeSpace(): bvec is NULL` (no directions) or `... desired_bvec_space =
/// <b>, must be 1 or 2`; 0 at once when the space is already the desired one. With `Gdiag_no` above
/// 0 the matrix is printed. Direction cosines without an inverse end the program (the product's
/// missing matrix, after MatrixInverse's message).
pub fn dti_bvec_change_space(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vol: &mut Mri,
    desired: i32,
) -> Result<i32, Fatal> {
    let _ = writeln!(out, "DTIbvecChangeSpace(): {} {desired}", vol.bvec_space);
    let b = desired;
    if vol.bvecs.is_none() {
        let _ = writeln!(out, "ERROR: DTIbvecChangeSpace(): bvec is NULL");
        return Ok(1);
    }
    if vol.bvec_space == desired {
        return Ok(0);
    }
    if b != BVEC_SPACE_SCANNER && b != BVEC_SPACE_VOXEL {
        let _ = writeln!(
            out,
            "ERROR: DTIbvecChangeSpace(): desired_bvec_space = {b}, must be {BVEC_SPACE_SCANNER} or {BVEC_SPACE_VOXEL}"
        );
        return Ok(1);
    }
    let reproduce = g.notices.reproduce();
    let cols = vol.bvecs.as_ref().map_or(3, |m| m.cols);
    if cols < 3 {
        g.notices.add(
            &FS_220,
            format!(
                "gradient directions of {cols} columns: FreeSurfer reads and writes three numbers a row, across the rows and past the matrix{}",
                if reproduce { "" } else { "; refused" }
            ),
        );
        if !reproduce {
            let _ = writeln!(
                out,
                "ERROR: DTIbvecChangeSpace(): bvecs have {cols} columns, 3 needed"
            );
            return Ok(1);
        }
    }
    let mdc = match matrix_of_direction_cosines(g, out, err, &vol.vg, None)? {
        Ret::New(m) => m,
        _ => unreachable!("MRImatrixOfDirectionCosines makes a matrix"),
    };
    let m: Option<Matrix> = if b == BVEC_SPACE_SCANNER {
        Some(mdc)
    } else {
        match matrix_inverse(g, out, err, Some(&mdc), None)? {
            Ret::New(inv) => Some(inv),
            _ => None,
        }
    };
    if g.gdiag_no > 0 {
        let _ = writeln!(
            out,
            "DTIbvecChangeSpace(): transforming gradient directions from {} to {desired}",
            vol.bvec_space
        );
        let _ = writeln!(out, "M------------------------------");
        let mut text: Vec<u8> = Vec::new();
        matrix_print(g, Some(&mut text as &mut dyn Write), out, err, m.as_ref(), 5);
        let _ = out.write_all(&text);
        let _ = writeln!(out, "------------------------------");
    }
    let mut bvec = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 matrix");
    bvec.set_relt(4, 1, 1.0);
    let bvecs = vol.bvecs.as_mut().expect("the gradient directions");
    let mut memory = false;
    for f in 1..=bvecs.rows {
        for i in 1..=3 {
            let v = cell(bvecs, f, i).unwrap_or_else(|| {
                memory = true;
                0.0
            });
            bvec.set_relt(i, 1, v);
        }
        let input = bvec.clone();
        matrix_multiply_d(g, out, err, m.as_ref(), Some(&input), Some(&mut bvec))?;
        for i in 1..=3 {
            let v = bvec.relt(i, 1);
            match bvecs.relt_index(f, i) {
                Some(k) if bvecs.set_raw(k, v) => {}
                _ => memory = true,
            }
        }
    }
    if memory {
        g.notices.add(
            &FS_220,
            format!(
                "gradient directions of {cols} columns: FreeSurfer reads memory contents past the matrix and writes outside it; read as 0, the writes dropped"
            ),
        );
    }
    vol.bvec_space = desired;
    Ok(0)
}
