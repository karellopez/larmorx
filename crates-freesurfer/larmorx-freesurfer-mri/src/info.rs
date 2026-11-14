// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A volume's summaries that only read the structure (`utils/mri.cpp`): `MRIdump` (its header as
//! text), `MRIvolumeDeterminant` (with `stuff_four_by_four`) and `MRIlimits` (its smallest and
//! largest voxel). Ported by fs.volume's replica (larmorx-freesurfer-volume's `coords.rs` and
//! `resize.rs`, which re-export them) and moved down to the structure's crate when fs.io's
//! ANALYZE reader and writers came to call them (waves.md section 4: a callee that only touches
//! the structure moves down with it).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::matrix_ops::{matrix_determinant, matrix_print};

use crate::mri::Mri;
use crate::vol_geom::VolGeom;
use crate::voxel::mri_get_vox_val;

/// `%f`.
fn c_f(v: f64) -> String {
    larmorx_core::cfmt::f(v, 6)
}

/// `stuff_four_by_four(m, ...)` of the direction cosines (`MRIvolumeDeterminant`'s matrix):
/// rows `(x_r, y_r, z_r, 0)`, `(x_a, y_a, z_a, 0)`, `(x_s, y_s, z_s, 0)`, `(0, 0, 0, 1)`.
fn dircos4(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Matrix, Fatal> {
    let mut m = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?.expect("a real matrix");
    let rows = [
        [vg.x_r, vg.y_r, vg.z_r, 0.0],
        [vg.x_a, vg.y_a, vg.z_a, 0.0],
        [vg.x_s, vg.y_s, vg.z_s, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    for (r, row) in rows.iter().enumerate() {
        for (c, &v) in row.iter().enumerate() {
            m.set_relt(r as i32 + 1, c as i32 + 1, v);
        }
    }
    Ok(m)
}

/// `MRIvolumeDeterminant(mri)`: the determinant of the direction cosines' 4 x 4 matrix
/// (`MatrixDeterminant`), as a `float`.
pub fn volume_determinant(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<f32, Fatal> {
    let m = dircos4(g, out, err, vg)?;
    Ok(matrix_determinant(g, err, &m))
}

/// `MRIdump(mri, fp)`: the header on `fp`, one `<name> = <value>` line per field (the names right
/// aligned in 6 columns and cut to 6 characters: `nframe`, `xstart`, ...; floats as `%f`), then
/// `det(xyz_ras)` ([`volume_determinant`]), `ras_good_flag`, `brightness`, `subject_name`,
/// `path_to_t1`, `fname_format`, and `register_mat` (`MatrixPrint`) when there is one;
/// `NO_ERROR`.
pub fn mri_dump(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    fp: &mut dyn Write,
) -> Result<i32, Fatal> {
    let vg = &mri.vg;
    let mut t: Vec<u8> = Vec::new();
    let mut line = |name: &str, v: &[u8]| {
        let n: String = name.chars().take(6).collect();
        t.extend_from_slice(format!("{n:>6} = ").as_bytes());
        t.extend_from_slice(v);
        t.push(b'\n');
    };
    let f = |v: f32| c_f(f64::from(v)).into_bytes();
    let i = |v: i32| v.to_string().into_bytes();
    line("fname", vg.fname.c_bytes());
    line("height", &i(vg.height));
    line("width", &i(vg.width));
    line("depth", &i(vg.depth));
    line("nframes", &i(mri.nframes));
    line("imnr0", &i(mri.imnr0));
    line("imnr1", &i(mri.imnr1));
    line("xnum", &i(vg.width));
    line("ynum", &i(vg.height));
    line("fov", &f(mri.fov));
    line("thick", &f(mri.thick));
    line("xstart", &f(mri.xstart));
    line("xend", &f(mri.xend));
    line("ystart", &f(mri.ystart));
    line("yend", &f(mri.yend));
    line("zstart", &f(mri.zstart));
    line("zend", &f(mri.zend));
    line("type", &i(mri.mtype));
    line("xsize", &f(vg.xsize));
    line("ysize", &f(vg.ysize));
    line("zsize", &f(vg.zsize));
    let three = |a: f32, b: f32, c: f32| {
        format!(
            "{} {} {}",
            c_f(f64::from(a)),
            c_f(f64::from(b)),
            c_f(f64::from(c))
        )
        .into_bytes()
    };
    line("x ras", &three(vg.x_r, vg.x_a, vg.x_s));
    line("y ras", &three(vg.y_r, vg.y_a, vg.y_s));
    line("z ras", &three(vg.z_r, vg.z_a, vg.z_s));
    line("c ras", &three(vg.c_r, vg.c_a, vg.c_s));
    let _ = fp.write_all(&t);
    let det = volume_determinant(g, out, err, vg)?;
    let _ = writeln!(fp, "det(xyz_ras) = {}", c_f(f64::from(det)));
    let _ = writeln!(fp, "ras_good_flag = {}", vg.ras_good_flag);
    let _ = writeln!(fp, "brightness = {}", mri.brightness);
    let _ = writeln!(fp, "subject_name = {}", mri.subject_name);
    let _ = writeln!(fp, "path_to_t1 = {}", mri.path_to_t1);
    let _ = writeln!(fp, "fname_format = {}", mri.fname_format);
    if let Some(m) = &vg.register_mat {
        let _ = fp.write_all(b"register_mat = \n");
        let mut buf = Vec::new();
        matrix_print(
            g,
            Some(&mut buf as &mut dyn Write),
            &mut std::io::sink(),
            err,
            Some(m),
            5,
        );
        let _ = fp.write_all(&buf);
    }
    Ok(NO_ERROR)
}

/// `MRIlimits(mri, &min, &max)`: the smallest and largest voxel (`MRIgetVoxVal`, C's comparisons
/// from voxel (0, 0, 0) of frame 0: a NaN first voxel stays); a volume without voxels leaves them
/// as they were. `NO_ERROR`.
pub fn mri_limits(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    min: &mut f32,
    max: &mut f32,
) -> Result<i32, Fatal> {
    if mri.chunk.is_none() {
        return Ok(NO_ERROR);
    }
    let v0 = mri_get_vox_val(g, err, mri, 0, 0, 0, 0, 0)?;
    *min = v0;
    *max = v0;
    for f in 0..mri.nframes {
        for i in 0..mri.vg.width {
            for j in 0..mri.vg.height {
                for k in 0..mri.vg.depth {
                    let val = mri_get_vox_val(g, err, mri, i, j, k, f, 0)?;
                    if val < *min {
                        *min = val;
                    }
                    if val > *max {
                        *max = val;
                    }
                }
            }
        }
    }
    Ok(NO_ERROR)
}
