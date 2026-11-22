// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The geometry changes of `lta_convert`'s `--srcconform*`, `--trgconform*`, `--srcupsample`,
//! `--srcdownsample`, `--trgupsample` and `--trgdownsample`: `upsampleGeom` and `downsampleGeom`
//! (`lta_convert.cpp` 116-193) and `conformGeom` (`utils/transform.cpp` 5665-5794, decided for the
//! program's crate: no other program on the default paths calls it).
//!
//! - [`upsample_geom`]: the first voxel's centre moved to `1/(2N)` of a voxel from the corner (the
//!   voxel-to-RAS matrix times `(-0.5 + 1/(2N))` in each axis, `float`), the sizes times `N`, the
//!   voxel sizes divided by `N` (`float`), the centre from that point (`MRIp0ToCRAS`), the cached
//!   matrices recomputed.
//! - [`downsample_geom`]: a size that `N` does not divide prints `ERROR: MRIdownsampleN: width=<w>,
//!   N=<n>` (height, depth) and leaves the geometry; else the first voxel's centre at `-0.5 + N/2`,
//!   the sizes divided by `N`, the voxel sizes times `N`, the centre and the cache as above.
//! - [`conform_geom`]: as `mri_convert --conform` makes its template: 256 voxels (or the size
//!   `MRIfindMinSize`/`MRIfindRightSize` give) of the conformed size; with `-dc` the direction
//!   cosines kept (the template's voxel-to-RAS matrix the source's times the inverse of the voxel
//!   map `K`, printed), else the coronal ones (the centre kept).
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-lta-convert.md`):
//! - **FS-1166**: a sampling factor below 1, one that makes a size pass 2^31, and a downsampling
//!   factor that does not divide every size: FreeSurfer divides by 0 (status 136), makes sizes of
//!   0 or below, or prints its ERROR and goes on with the geometry unchanged (status 0). By
//!   default the program ends with a message, status 1 (rule B).
//! - **FS-1167**: `-dc` on a geometry whose orientation string lacks an axis (`???` when
//!   `ras_good_flag` is 0, or two axes with the same main direction) reads past FreeSurfer's
//!   three-element arrays (memory contents). By default the program ends with a message, status 1
//!   (rule B); `reproduce` reads those elements as 0.

use larmorx_core::defects::{FS_1166, FS_1167};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{
    MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply, matrix_multiply_d,
};
use larmorx_freesurfer_core::matrix_ops::{matrix_inverse, matrix_print, matrix_zero};
use larmorx_freesurfer_mri::vol_geom::{
    VolGeom, p0_to_cras, reinit_cache, set_vox2ras_from_matrix, xfm_crs2xyz,
};
use larmorx_freesurfer_volume::conformed::{mri_find_min_size, mri_find_right_size};
use larmorx_freesurfer_volume::coords::dircos_to_orientation_string;

use super::main::Run;

/// `MatrixPrint(stdout, m)` on the program's standard output.
pub fn print_matrix(g: &mut Globals, r: &mut Run, m: Option<&Matrix>) {
    let mut buf = Vec::new();
    matrix_print(g, Some(&mut buf), &mut std::io::sink(), r.err, m, 5);
    let _ = r.out.write_all(&buf);
}

/// A sampling factor FS-1166 refuses by default: below 1, or one that makes a size pass 2^31.
fn bad_factor(g: &mut Globals, r: &mut Run, vg: &VolGeom, n: i32, what: &str) -> Result<(), Fatal> {
    let too_big = [vg.width, vg.height, vg.depth]
        .iter()
        .any(|&s| i64::from(s) * i64::from(n) > i64::from(i32::MAX));
    if n >= 1 && !(what == "upsample" && too_big) {
        return Ok(());
    }
    g.notices.add(
        &FS_1166,
        format!("lta_convert: a {what} factor of {n} (FreeSurfer divides by 0 or makes sizes of 0 or below, or past 2^31)"),
    );
    if g.notices.reproduce() {
        if what == "downsample" && n == 0 {
            // `width % 0`: SIGFPE
            return Err(Fatal::Exit(136));
        }
        return Ok(());
    }
    let _ = writeln!(
        r.err,
        "ERROR: lta_convert: the {what} factor must be at least 1 and keep the sizes below 2^31 (got {n})"
    );
    Err(Fatal::Exit(1))
}

/// `upsampleGeom(vg, N)` (FS-1166 for a factor below 1 or one that makes a size pass 2^31).
pub fn upsample_geom(g: &mut Globals, r: &mut Run, vg: &mut VolGeom, n: i32) -> Result<(), Fatal> {
    bad_factor(g, r, vg, n, "upsample")?;
    let vox2ras = xfm_crs2xyz(g, r.out, r.err, vg, 0, false)?;
    let mut crs0 = matrix_zero(g, r.out, r.err, 4, 1, None)?
        .into_new()
        .expect("MatrixZero without an output returns a new matrix");
    let c = (-0.5 + 1.0 / f64::from(n.wrapping_mul(2))) as f32;
    crs0.set_relt(1, 1, c);
    crs0.set_relt(2, 1, c);
    crs0.set_relt(3, 1, c);
    crs0.set_relt(4, 1, 1.0);
    let ras0 = matrix_multiply(g, r.out, r.err, Some(&vox2ras), Some(&crs0), None)?
        .into_new()
        .expect("a 4 x 4 times a 4 x 1 matrix");
    vg.width = vg.width.wrapping_mul(n);
    vg.height = vg.height.wrapping_mul(n);
    vg.depth = vg.depth.wrapping_mul(n);
    vg.xsize /= n as f32;
    vg.ysize /= n as f32;
    vg.zsize /= n as f32;
    p0_to_cras(
        g,
        r.out,
        r.err,
        vg,
        f64::from(ras0.relt(1, 1)),
        f64::from(ras0.relt(2, 1)),
        f64::from(ras0.relt(3, 1)),
    )?;
    reinit_cache(g, r.out, r.err, vg)?;
    Ok(())
}

/// `downsampleGeom(vg, N)` (FS-1166 for a factor below 1 and one that does not divide a size).
pub fn downsample_geom(
    g: &mut Globals,
    r: &mut Run,
    vg: &mut VolGeom,
    n: i32,
) -> Result<(), Fatal> {
    bad_factor(g, r, vg, n, "downsample")?;
    for (what, size) in [
        ("width", vg.width),
        ("height", vg.height),
        ("depth", vg.depth),
    ] {
        if size.wrapping_rem(n) != 0 {
            let _ = writeln!(r.out, "ERROR: MRIdownsampleN: {what}={size}, N={n}");
            g.notices.add(
                &FS_1166,
                format!("lta_convert: a downsample factor of {n} that does not divide the {what} {size} (FreeSurfer goes on with the geometry unchanged, status 0)"),
            );
            if g.notices.reproduce() {
                return Ok(());
            }
            return Err(Fatal::Exit(1));
        }
    }
    let mut crs0 = matrix_zero(g, r.out, r.err, 4, 1, None)?
        .into_new()
        .expect("MatrixZero without an output returns a new matrix");
    let c = (-0.5 + f64::from(n) / 2.0) as f32;
    crs0.set_relt(1, 1, c);
    crs0.set_relt(2, 1, c);
    crs0.set_relt(3, 1, c);
    crs0.set_relt(4, 1, 1.0);
    let vox2ras = xfm_crs2xyz(g, r.out, r.err, vg, 0, false)?;
    let ras0 = matrix_multiply(g, r.out, r.err, Some(&vox2ras), Some(&crs0), None)?
        .into_new()
        .expect("a 4 x 4 times a 4 x 1 matrix");
    vg.width = vg.width.wrapping_div(n);
    vg.height = vg.height.wrapping_div(n);
    vg.depth = vg.depth.wrapping_div(n);
    vg.xsize *= n as f32;
    vg.ysize *= n as f32;
    vg.zsize *= n as f32;
    p0_to_cras(
        g,
        r.out,
        r.err,
        vg,
        f64::from(ras0.relt(1, 1)),
        f64::from(ras0.relt(2, 1)),
        f64::from(ras0.relt(3, 1)),
    )?;
    reinit_cache(g, r.out, r.err, vg)?;
    Ok(())
}

/// `(int) x` of a `double` (or `float`) as x86's `cvttsd2si`: towards 0, `INT_MIN` beyond.
fn trunc_int(x: f64) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x < -2147483648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// `conformGeom(vg, conform_min, conform_size0, confkeepdc)`: 1 (after `ERROR: conformGeom(): all
/// conform options are False`) when none is set, else 0.
pub fn conform_geom(
    g: &mut Globals,
    r: &mut Run,
    vg: &mut VolGeom,
    conform_min: bool,
    conform_size0: f32,
    keepdc: bool,
) -> Result<i32, Fatal> {
    if !conform_min && conform_size0 == 0.0 && !keepdc {
        let _ = writeln!(r.out, "ERROR: conformGeom(): all conform options are False");
        return Ok(1);
    }
    let vg_src = vg.clone();
    let mut conform_size = conform_size0;
    let conform_width = if conform_min {
        let (size, width) = mri_find_min_size(g, vg);
        conform_size = size;
        width
    } else {
        mri_find_right_size(g, r.err, vg, conform_size)
    };
    vg.width = conform_width;
    vg.height = conform_width;
    vg.depth = conform_width;
    vg.xsize = conform_size;
    vg.ysize = conform_size;
    vg.zsize = conform_size;
    if !keepdc {
        // the old method: VOL_GEOM's initial direction cosines (the centre kept)
        vg.x_r = -1.0;
        vg.x_a = 0.0;
        vg.x_s = 0.0;
        vg.y_r = 0.0;
        vg.y_a = 0.0;
        vg.y_s = -1.0;
        vg.z_r = 0.0;
        vg.z_a = 1.0;
        vg.z_s = 0.0;
        return Ok(0);
    }
    let conform_fov = trunc_int(f64::from(conform_width as f32 * conform_size));
    let (_, o) = dircos_to_orientation_string(&vg_src);
    let find = |a: u8, b: u8| (0..3).find(|&i| o[i] == a || o[i] == b).unwrap_or(3);
    let (ilr, iis, iap) = (find(b'L', b'R'), find(b'I', b'S'), find(b'A', b'P'));
    if ilr == 3 || iis == 3 || iap == 3 {
        let ostr = String::from_utf8_lossy(&o).into_owned();
        g.notices.add(
            &FS_1167,
            format!("lta_convert: -dc on a geometry of orientation {ostr} (an axis has no main direction: FreeSurfer reads past its arrays)"),
        );
        if g.notices.fix() {
            let _ = writeln!(
                r.err,
                "ERROR: lta_convert: the geometry's orientation {ostr} does not give each axis its own main direction: its direction cosines cannot be kept"
            );
            return Err(Fatal::Exit(1));
        }
    }
    let _ = writeln!(r.out, "keeping DC {ilr} {iis} {iap}");
    let _ = writeln!(
        r.out,
        "ostr {}, width {conform_width}, size {}",
        String::from_utf8_lossy(&o),
        super::cxx::put(f64::from(conform_size), 6)
    );
    let nvox = [vg_src.width, vg_src.height, vg_src.depth];
    let delta = [
        f64::from(vg_src.xsize),
        f64::from(vg_src.ysize),
        f64::from(vg_src.zsize),
    ];
    let fov: Vec<i32> = (0..3)
        .map(|c| trunc_int(f64::from(nvox[c]) * delta[c]))
        .collect();
    // reproduce (FS-1167): the element past the arrays read as 0
    let d = |i: usize| delta.get(i).copied().unwrap_or(0.0);
    let f = |i: usize| fov.get(i).copied().unwrap_or(0);
    let mut k = matrix_alloc(g, r.out, r.err, 4, 4, MATRIX_REAL)?.expect("a 4 x 4 matrix");
    k.set_relt(4, 4, 1.0);
    let cs = f64::from(conform_size);
    for (row, i, first) in [(1, ilr, b'L'), (2, iis, b'I'), (3, iap, b'A')] {
        let step = d(i) / cs;
        let pad = ((f64::from(conform_fov.wrapping_sub(f(i))) / 2.0) / cs).round();
        let letter = o.get(i).copied().unwrap_or(0);
        if letter == first {
            k.set_relt(row, i as i32 + 1, step as f32);
            k.set_relt(row, 4, pad as f32);
        } else {
            k.set_relt(row, i as i32 + 1, (-step) as f32);
            k.set_relt(row, 4, (f64::from(conform_width) - pad) as f32);
        }
    }
    let inv_k = matrix_inverse(g, r.out, r.err, Some(&k), None)?.into_new();
    let smri = xfm_crs2xyz(g, r.out, r.err, &vg_src, 0, false)?;
    let stemp = matrix_multiply_d(g, r.out, r.err, Some(&smri), inv_k.as_ref(), None)?
        .into_new()
        .expect("MatrixMultiplyD without an output returns a new matrix");
    set_vox2ras_from_matrix(g, r.out, r.err, vg, &stemp)?;
    let _ = r.out.write_all(b"K ---------------\n");
    print_matrix(g, r, Some(&k));
    let _ = r.out.write_all(b"Kinv ---------------\n");
    print_matrix(g, r, inv_k.as_ref());
    let _ = r.out.write_all(b"Smri ---------------\n");
    print_matrix(g, r, Some(&smri));
    let _ = r.out.write_all(b"Stemp ---------------\n");
    print_matrix(g, r, Some(&stemp));
    let _ = r.out.write_all(b"----------------------\n");
    Ok(0)
}
