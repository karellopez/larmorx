// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Orientation strings and voxel-to-RAS matrices of a volume (`utils/mri.cpp`, 3.1-37):
//! `MRIcheckOrientationString`, `MRIorientationStringToDircos`, `stuff_four_by_four`,
//! `MRIvoxelXformToRasXform`, `MRIgetVoxelToVoxelXform` and `MRIsetVoxelToRasXform`.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §12):
//! - **FS-463** (memory): an orientation string of fewer than two characters is read past its
//!   end; the bytes there are taken as the string's end in both modes, noted.
//! - **FS-464** (memory): a voxel-to-RAS matrix of fewer than 3 rows or 4 columns is read past its
//!   elements; an error in both modes.

use std::io::Write;

use larmorx_core::defects::{FS_463, FS_464};
use larmorx_freesurfer_core::cscan;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{add, add64, div, mul, sqrt64};
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::vol_geom::{extract_i_to_r, extract_r_to_i, reinit_cache};

/// The first three characters of an orientation string as C reads them: the string's bytes up to
/// its first NUL, then its terminating NUL; FS-463 for a place past the terminator (0 here).
fn three(g: &mut Globals, ostr: &[u8]) -> [u8; 3] {
    let end = ostr.iter().position(|&b| b == 0).unwrap_or(ostr.len());
    let mut c = [0u8; 3];
    for (k, x) in c.iter_mut().enumerate() {
        if k < end {
            *x = ostr[k];
        } else if k > end {
            g.notices.add(
                &FS_463,
                "an orientation string of fewer than two characters is read past its end (taken as its end here)",
            );
        }
    }
    c
}

/// `MRIcheckOrientationString(ostr)`: `None` when the first three characters (any case) name each
/// of the three axes once (L or R, P or A, I or S); otherwise the errors' text: for each other
/// character `Character <c> in position <k> is invalid.` (the character's byte as it is; a NUL
/// there cuts the text built so far just before it), then `  LR axis represented multiple
/// times.`, the same for `PA` and `IS`, then `  LR axis not represented.`, ..., each line ending
/// in a newline.
pub fn check_orientation_string(g: &mut Globals, ostr: &[u8]) -> Option<Vec<u8>> {
    let c3 = three(g, ostr);
    let (mut nsag, mut ncor, mut nax, mut err) = (0, 0, 0, false);
    let mut errstr: Vec<u8> = Vec::new();
    for (c, &ch) in c3.iter().enumerate() {
        match ch.to_ascii_uppercase() {
            b'L' | b'R' => nsag += 1,
            b'P' | b'A' => ncor += 1,
            b'I' | b'S' => nax += 1,
            _ => {
                let mut tmp = errstr.clone();
                tmp.extend_from_slice(b"Character ");
                tmp.push(ch);
                tmp.extend_from_slice(format!(" in position {} is invalid.\n", c + 1).as_bytes());
                // strncpy into the C buffer: up to the first NUL
                let cut = tmp.iter().position(|&b| b == 0).unwrap_or(tmp.len());
                tmp.truncate(cut);
                errstr = tmp;
                err = true;
            }
        }
    }
    for (n, axis) in [(nsag, "LR"), (ncor, "PA"), (nax, "IS")] {
        if n > 1 {
            errstr.extend_from_slice(
                format!("  {axis} axis represented multiple times.\n").as_bytes(),
            );
            err = true;
        }
    }
    for (n, axis) in [(nsag, "LR"), (ncor, "PA"), (nax, "IS")] {
        if n == 0 {
            errstr.extend_from_slice(format!("  {axis} axis not represented.\n").as_bytes());
            err = true;
        }
    }
    err.then_some(errstr)
}

/// `MRIorientationStringToDircos(mri, ostr)`: a string [`check_orientation_string`] refuses gives
/// `ERROR: in orientation string <ostr>` and the errors' text on standard output, and 1;
/// otherwise each of the three axes' direction cosines becomes the unit vector its letter names
/// (L -x, R +x, P -y, A +y, I -z, S +z, any case), and 0. Nothing else changes.
pub fn orientation_string_to_dircos(
    g: &mut Globals,
    out: &mut dyn Write,
    mri: &mut Mri,
    ostr: &[u8],
) -> i32 {
    if let Some(e) = check_orientation_string(g, ostr) {
        let end = ostr.iter().position(|&b| b == 0).unwrap_or(ostr.len());
        let _ = out.write_all(b"ERROR: in orientation string ");
        let _ = out.write_all(&ostr[..end]);
        let _ = out.write_all(b"\n");
        let _ = out.write_all(&e);
        return 1;
    }
    let c3 = three(g, ostr);
    let mut mdc = [[0.0f64; 3]; 3];
    for (c, &ch) in c3.iter().enumerate() {
        let (r, v) = match ch.to_ascii_uppercase() {
            b'L' => (0, -1.0),
            b'R' => (0, 1.0),
            b'P' => (1, -1.0),
            b'A' => (1, 1.0),
            b'I' => (2, -1.0),
            _ => (2, 1.0),
        };
        mdc[r][c] = v;
    }
    let vg = &mut mri.vg;
    (vg.x_r, vg.x_a, vg.x_s) = (mdc[0][0] as f32, mdc[1][0] as f32, mdc[2][0] as f32);
    (vg.y_r, vg.y_a, vg.y_s) = (mdc[0][1] as f32, mdc[1][1] as f32, mdc[2][1] as f32);
    (vg.z_r, vg.z_a, vg.z_s) = (mdc[0][2] as f32, mdc[1][2] as f32, mdc[2][2] as f32);
    0
}

/// `stuff_four_by_four(m, m11, ..., m44)`: the sixteen values into a 4 x 4 matrix, row by row
/// (`MATRIX_RELT`'s places, whatever the matrix's type), and `NO_ERROR`; no matrix: the error
/// `stuff_four_by_four(): matrix is NULL`, another size: `stuff_four_by_four(): matrix is not
/// four-by-four`, both `ERROR_BADPARM`.
pub fn stuff_four_by_four(
    g: &mut Globals,
    err: &mut dyn Write,
    m: Option<&mut Matrix>,
    v: &[f32; 16],
) -> i32 {
    let Some(m) = m else {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "stuff_four_by_four(): matrix is NULL",
        );
        return ERROR_BADPARM;
    };
    if m.rows != 4 || m.cols != 4 {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "stuff_four_by_four(): matrix is not four-by-four",
        );
        return ERROR_BADPARM;
    }
    for r in 0..4 {
        for c in 0..4 {
            m.set_relt(r as i32 + 1, c as i32 + 1, v[4 * r + c]);
        }
    }
    NO_ERROR
}

/// `MRIvoxelXformToRasXform(src, dst, m_voxel_xform, m_ras_xform)`: the product `vox2ras(dst)`
/// times (`m_voxel_xform` times `ras2vox(src)`) (`MatrixMultiply`; `dst` the source when there is
/// none), into `m_ras_xform` when given.
pub fn mri_voxel_xform_to_ras_xform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&Mri>,
    m_voxel: Option<&Matrix>,
    m_ras: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let dst = dst.unwrap_or(src);
    let r2v = extract_r_to_i(g, out, err, &src.vg)?;
    let v2r = extract_i_to_r(g, out, err, &dst.vg)?;
    let tmp = matrix_multiply(g, out, err, m_voxel, r2v.as_ref(), None)?.into_new();
    matrix_multiply(g, out, err, Some(&v2r), tmp.as_ref(), m_ras)
}

/// `MRIgetVoxelToVoxelXform(src, dst)`: `ras2vox(dst) * vox2ras(src)` (`MatrixMultiply`).
pub fn mri_get_voxel_to_voxel_xform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: &Mri,
) -> Result<Option<Matrix>, Fatal> {
    let v2r = extract_i_to_r(g, out, err, &src.vg)?;
    let r2v = extract_r_to_i(g, out, err, &dst.vg)?;
    Ok(matrix_multiply(g, out, err, r2v.as_ref(), Some(&v2r), None)?.into_new())
}

/// `MRIsetVoxelToRasXform(mri, m_vox2ras)`: the scales `sqrt(sum of squares)` of the matrix's first
/// three columns (rows 1-3; each square in `float`, summed in `double`); when one differs from the
/// voxel size by more than `thresh` (0.001, or the `double` the environment variable
/// `FS_SetVoxToRasXform_Change_VoxSize` holds, read by `sscanf`'s `%lf`), the lines `WARNING:
/// MRIsetVoxelToRasXform(): voxel size is inconsistent with scale (thresh=<g>)`, `  scale <g> <g>
/// <g>`, `  vs <g> <g> <g>` on standard output and, when the variable is set, `  changing voxel
/// size to scale` and the voxel sizes set to the scales. Then the direction cosines are the columns
/// divided by the voxel sizes (`float`), the centre is column 4 plus the first three columns times
/// half the extent (`float`; `m14 + ((m11 ci + m12 cj) + m13 ck)`), `ras_good_flag` 1, and the
/// cached matrices recomputed (`MRIreInitCache`); `NO_ERROR`. FS-464: a matrix smaller than 3 x 4
/// is an error.
pub fn mri_set_voxel_to_ras_xform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    mri: &mut Mri,
    m: &Matrix,
) -> Result<i32, Fatal> {
    if (1..=3).any(|r| (1..=4).any(|c| m.relt_index(r, c).is_none())) || m.rows < 3 || m.cols < 4 {
        g.notices.add(
            &FS_464,
            "a voxel-to-RAS matrix smaller than 3 x 4 is read past its elements",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "MRIsetVoxelToRasXform(): the matrix is smaller than 3 x 4 (FreeSurfer reads past it)\n",
        )?;
        return Ok(ERROR_BADPARM);
    }
    let e = |r: i32, c: i32| m.relt(r, c);
    let (mut xs, mut ys, mut zs) = (0.0f64, 0.0f64, 0.0f64);
    for k in 1..=3 {
        xs = add64(xs, f64::from(mul(e(k, 1), e(k, 1))));
        ys = add64(ys, f64::from(mul(e(k, 2), e(k, 2))));
        zs = add64(zs, f64::from(mul(e(k, 3), e(k, 3))));
    }
    (xs, ys, zs) = (sqrt64(xs), sqrt64(ys), sqrt64(zs));
    let mut thresh = 0.001f64;
    let a = env.get("FS_SetVoxToRasXform_Change_VoxSize").cloned();
    if let Some(a) = &a {
        let bytes = a.as_encoded_bytes();
        let mut st = cscan::Bytes::new(bytes);
        if let Some(v) = cscan::Call::new().f64(&mut st, &mut g.errno) {
            thresh = v;
        }
    }
    let vg = &mut mri.vg;
    let off = |size: f32, s: f64| (f64::from(size) - s).abs() > thresh;
    if off(vg.xsize, xs) || off(vg.ysize, ys) || off(vg.zsize, zs) {
        let gf = |v: f64| larmorx_core::cfmt::g(v, 6);
        let _ = write!(
            out,
            "WARNING: MRIsetVoxelToRasXform(): voxel size is inconsistent with scale (thresh={})\n  scale {} {} {}\n  vs {} {} {}\n",
            gf(thresh),
            gf(xs),
            gf(ys),
            gf(zs),
            gf(f64::from(vg.xsize)),
            gf(f64::from(vg.ysize)),
            gf(f64::from(vg.zsize))
        );
        if a.is_some() {
            let _ = writeln!(out, "  changing voxel size to scale");
            (vg.xsize, vg.ysize, vg.zsize) = (xs as f32, ys as f32, zs as f32);
        }
    }
    vg.x_r = div(e(1, 1), vg.xsize);
    vg.y_r = div(e(1, 2), vg.ysize);
    vg.z_r = div(e(1, 3), vg.zsize);
    vg.x_a = div(e(2, 1), vg.xsize);
    vg.y_a = div(e(2, 2), vg.ysize);
    vg.z_a = div(e(2, 3), vg.zsize);
    vg.x_s = div(e(3, 1), vg.xsize);
    vg.y_s = div(e(3, 2), vg.ysize);
    vg.z_s = div(e(3, 3), vg.zsize);
    let ci = (f64::from(vg.width) / 2.0) as f32;
    let cj = (f64::from(vg.height) / 2.0) as f32;
    let ck = (f64::from(vg.depth) / 2.0) as f32;
    let centre = |r: i32| {
        add(
            e(r, 4),
            add(add(mul(e(r, 1), ci), mul(e(r, 2), cj)), mul(e(r, 3), ck)),
        )
    };
    (vg.c_r, vg.c_a, vg.c_s) = (centre(1), centre(2), centre(3));
    vg.ras_good_flag = 1;
    reinit_cache(g, out, err, &mut mri.vg)?;
    Ok(NO_ERROR)
}
