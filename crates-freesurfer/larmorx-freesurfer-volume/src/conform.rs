// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Conforming a volume to FreeSurfer's 256^3 1 mm coronal grid (`utils/mri_conform.cpp`, 3.1-38):
//! `MRIgetConformMatrix`, `MRIconform` and `MRIconformedTemplate`.
//!
//! The conformed grid: 256 x 256 x 256 voxels of 1 mm, bytes, coronal direction cosines
//! (`x_r = -1`, `y_s = -1`, `z_a = 1`), the source's centre (`c_r`, `c_a`, `c_s`) and pulse
//! parameters. `MRIconform` copies the volume, gives it the coronal cosines when its own are not
//! good (`ras_good_flag` 0), changes it to bytes when it is of another type (`MRIchangeType` with
//! the robust range 0 to 0.999 and scaling) and resamples it trilinearly onto the grid unless its
//! voxel sizes, extent and direction cosines are already the grid's (compared exactly, as
//! `float`s and `int`s: a NaN always differs).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §19):
//! - **FS-491** (memory contents): `MRIconformedTemplate` keeping the direction cosines of a
//!   volume whose orientation string lacks a left/right, inferior/superior or anterior/posterior
//!   letter (cosines that are not good give `???`; two axes along the same direction) reads the
//!   voxel size and field of view past their three-element arrays and writes that row's step
//!   into the translation column. An error by default (`ERROR_BADPARM`); with `reproduce` the
//!   values read are taken as 0 (FreeSurfer's are memory contents) and the step written as
//!   FreeSurfer writes it. Noted in both modes.
//! - **FS-492** (rule A): the same template's fields of view are `int`s: a product beyond the
//!   32-bit range, or NaN, converts to -2147483648 and their difference wraps around. By default
//!   each conversion is clamped (NaN to 0) and the difference taken exactly; `reproduce` gives
//!   FreeSurfer's. Noted in both modes.

use std::io::Write;

use larmorx_core::defects::{FS_491, FS_492};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply_d};
use larmorx_freesurfer_core::matrix_ops::{matrix_inverse, matrix_print};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{div64, sub64};
use larmorx_freesurfer_io::mriio::set_direction_cosine;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{
    MRI_UCHAR, Mri, mri_alloc_header, mri_copy_header, mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::vol_geom::{set_vox2ras_from_matrix, xfm_crs2xyz};

use crate::coords::dircos_to_orientation_string;
use crate::resample::mri_resample;
use crate::resize::{MRI_CORONAL, mri_change_type};
use crate::sample::SAMPLE_TRILINEAR;

/// The conformed grid's header (`MRIallocHeader(256, 256, 256, MRI_UCHAR, nframes)` and the
/// fields `MRIgetConformMatrix` and `MRIconform` set): the centre `c` and the pulse parameters of
/// `pulse`.
fn conformed_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    nframes: i32,
    c: [f32; 3],
    pulse: &Mri,
) -> Result<Mri, Fatal> {
    let mut t = mri_alloc_header(g, out, err, 256, 256, 256, MRI_UCHAR, nframes)?;
    t.imnr0 = 1;
    t.imnr1 = 256;
    t.thick = 1.0;
    t.ps = 1.0;
    (t.vg.xsize, t.vg.ysize, t.vg.zsize) = (1.0, 1.0, 1.0);
    (t.xstart, t.ystart, t.zstart) = (-128.0, -128.0, -128.0);
    (t.xend, t.yend, t.zend) = (128.0, 128.0, 128.0);
    set_direction_cosine(g, err, &mut t.vg, MRI_CORONAL);
    (t.vg.c_r, t.vg.c_a, t.vg.c_s) = (c[0], c[1], c[2]);
    t.vg.ras_good_flag = 1;
    t.tr = pulse.tr;
    t.te = pulse.te;
    t.flip_angle = pulse.flip_angle;
    t.ti = pulse.ti;
    Ok(t)
}

/// `MRIgetConformMatrix(mri)`: the matrix from the conformed grid's voxels to `mri`'s
/// (`MRIgetResampleMatrix(mri, template)`, `None` where it fails). `mri` gets the coronal
/// direction cosines and centre 0 first when its own are not good.
pub fn mri_get_conform_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &mut Mri,
) -> Result<Option<Matrix>, Fatal> {
    if mri.vg.ras_good_flag == 0 {
        set_direction_cosine(g, err, &mut mri.vg, MRI_CORONAL);
    }
    let c = [mri.vg.c_r, mri.vg.c_a, mri.vg.c_s];
    let templ = conformed_header(g, out, err, mri.nframes, c, mri)?;
    crate::xform::mri_get_resample_matrix(g, out, err, mri, &templ)
}

/// `MRIconform(mri)`: a new volume on the conformed grid (module documentation), or `None` where
/// the change of type or the resampling fails. `mri` itself is not changed. `env` is the
/// program's environment (`MRIchangeType` reads it).
pub fn mri_conform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    mri: &Mri,
) -> Result<Option<Mri>, Fatal> {
    let Some(mut res) = mri_copy(g, out, err, mri, None)?.into_new() else {
        unreachable!("MRIcopy into no destination makes a new volume");
    };
    if res.vg.ras_good_flag == 0 {
        set_direction_cosine(g, err, &mut res.vg, MRI_CORONAL);
    }
    let c = [res.vg.c_r, res.vg.c_a, res.vg.c_s];
    let templ = conformed_header(g, out, err, mri.nframes, c, mri)?;
    if res.mtype != templ.mtype {
        // FALSE: rescale (the comment in FreeSurfer's source says the opposite)
        match mri_change_type(g, out, err, env, &res, templ.mtype, 0.0, 0.999, 0)? {
            Some(m) => res = m,
            None => return Ok(None),
        }
    }
    let (r, t) = (&res.vg, &templ.vg);
    #[allow(clippy::float_cmp)]
    let differs = r.xsize != t.xsize
        || r.ysize != t.ysize
        || r.zsize != t.zsize
        || r.width != t.width
        || r.height != t.height
        || r.depth != t.depth
        || r.x_r != t.x_r
        || r.x_a != t.x_a
        || r.x_s != t.x_s
        || r.y_r != t.y_r
        || r.y_a != t.y_a
        || r.y_s != t.y_s
        || r.z_r != t.z_r
        || r.z_a != t.z_a
        || r.z_s != t.z_s;
    if differs {
        match mri_resample(g, out, err, &res, &templ, SAMPLE_TRILINEAR)? {
            Some(m) => res = m,
            None => return Ok(None),
        }
    }
    Ok(Some(res))
}

/// x86-64's `(int) x` with FS-492: clamped by default (NaN to 0), `INT_MIN` with `reproduce`.
fn to_int(g: &mut Globals, x: f64, what: &str) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        g.notices.add(
            &FS_492,
            format!(
                "MRIconformedTemplate: the {what} {x} converted to an int, outside the int range"
            ),
        );
        if g.notices.reproduce() {
            return i32::MIN;
        }
        if x.is_nan() {
            return 0;
        }
        return if x > 0.0 { i32::MAX } else { i32::MIN };
    }
    x.trunc() as i32
}

/// C's `round(x)` (glibc: halves away from zero; a NaN passed on quieted).
fn c_round(x: f64) -> f64 {
    if x.is_nan() {
        f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000)
    } else {
        x.round()
    }
}

/// `MRIconformedTemplate(mri, conform_width, conform_size, KeepDC)`: a header-only byte volume of
/// `conform_width`^3 voxels of `conform_size` with `mri`'s header (`MRIcopyHeader`) and pulse
/// parameters, `imnr0` 1, `imnr1` `conform_width`, `thick` and `ps` `conform_size`, the starts
/// `-conform_width / 2` and ends `conform_width / 2` (integer division). Without `keep_dc` the
/// coronal direction cosines (FreeSurfer's "old method"; the centre stays `mri`'s). With
/// `keep_dc`: on standard output `keeping DC <iLR> <iIS> <iAP>` (the positions of the
/// orientation string's left/right, inferior/superior and anterior/posterior letters) and
/// `ostr <ostr>, width <w>, size <%g>`; then the voxel-to-voxel matrix `K` (each row the step
/// `delta / conform_size` at the letter's column, negative for `R`, `S` and `P`, and a padding
/// `round(((conform_width * conform_size - n * delta) / 2) / conform_size)` voxels, from the far
/// side for those letters, the two fields of view truncated to `int`s), the template's
/// voxel-to-RAS matrix `MRIxfmCRS2XYZ(mri, 0) * inverse(K)` set with `MRIsetVox2RASFromMatrix`,
/// and the four matrices printed (`K`, `Kinv`, `Smri`, `Stemp`, each after its title line).
/// FS-491, FS-492.
#[allow(clippy::too_many_arguments)]
pub fn mri_conformed_template(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    conform_width: i32,
    conform_size: f64,
    keep_dc: bool,
) -> Result<Mri, Fatal> {
    let cw = conform_width;
    let mut t = mri_alloc_header(g, out, err, cw, cw, cw, MRI_UCHAR, mri.nframes)?;
    mri_copy_header(g, out, err, mri, Some(&mut t))?;
    mri_copy_pulse_parameters(mri, &mut t);
    t.imnr0 = 1;
    t.imnr1 = cw;
    t.thick = conform_size as f32;
    t.ps = conform_size as f32;
    let s = conform_size as f32;
    (t.vg.xsize, t.vg.ysize, t.vg.zsize) = (s, s, s);
    let lo = (cw.wrapping_neg() / 2) as f32;
    let hi = (cw / 2) as f32;
    (t.xstart, t.ystart, t.zstart) = (lo, lo, lo);
    (t.xend, t.yend, t.zend) = (hi, hi, hi);
    if !keep_dc {
        (t.vg.x_r, t.vg.x_a, t.vg.x_s) = (-1.0, 0.0, 0.0);
        (t.vg.y_r, t.vg.y_a, t.vg.y_s) = (0.0, 0.0, -1.0);
        (t.vg.z_r, t.vg.z_a, t.vg.z_s) = (0.0, 1.0, 0.0);
        return Ok(t);
    }
    let conform_fov = to_int(g, f64::from(cw) * conform_size, "field of view");
    let (_, ostr) = dircos_to_orientation_string(&mri.vg);
    let find = |a: u8, b: u8| ostr.iter().position(|&c| c == a || c == b).unwrap_or(3);
    let (i_lr, i_is, i_ap) = (find(b'L', b'R'), find(b'I', b'S'), find(b'A', b'P'));
    let _ = writeln!(out, "keeping DC {i_lr} {i_is} {i_ap}");
    let size_g = larmorx_freesurfer_core::cstd::format_double("%g", conform_size)
        .expect("%g is a double format");
    let _ = writeln!(
        out,
        "ostr {}, width {cw}, size {size_g}",
        String::from_utf8_lossy(&ostr)
    );
    let missing = i_lr == 3 || i_is == 3 || i_ap == 3;
    if missing {
        g.notices.add(
            &FS_491,
            format!(
                "MRIconformedTemplate: the orientation string {} lacks a letter (FreeSurfer reads \
                 past its arrays)",
                String::from_utf8_lossy(&ostr)
            ),
        );
        if !g.notices.reproduce() {
            let msg = format!(
                "MRIconformedTemplate: orientation string {} lacks a left/right, \
                 inferior/superior or anterior/posterior axis",
                String::from_utf8_lossy(&ostr)
            );
            log::error_exit(g, err, ERROR_BADPARM, &msg)?;
        }
    }
    let nvox = [mri.vg.width, mri.vg.height, mri.vg.depth];
    // delta[3] and FoV[3] (FS-491): memory contents in FreeSurfer, 0 here
    let mut delta = [0.0f64; 4];
    let mut fov = [0i32; 4];
    delta[..3].copy_from_slice(&[
        f64::from(mri.vg.xsize),
        f64::from(mri.vg.ysize),
        f64::from(mri.vg.zsize),
    ]);
    for c in 0..3 {
        fov[c] = to_int(g, f64::from(nvox[c]) * delta[c], "field of view");
    }
    let mut k = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?.expect("a 4 x 4 matrix");
    k.set_relt(4, 4, 1.0);
    for (row, i, low) in [(1, i_lr, b'L'), (2, i_is, b'I'), (3, i_ap, b'A')] {
        let step = div64(delta[i], conform_size);
        let diff = fov_diff(g, conform_fov, fov[i]);
        let pad = c_round(div64(diff / 2.0, conform_size));
        // ostr[3] is the string's end: never the low letter
        if i < 3 && ostr[i] == low {
            k.set_relt(row, i as i32 + 1, step as f32);
            k.set_relt(row, 4, pad as f32);
        } else {
            k.set_relt(row, i as i32 + 1, (-step) as f32);
            k.set_relt(row, 4, sub64(f64::from(cw), pad) as f32);
        }
    }
    let inv_k = matrix_inverse(g, out, err, Some(&k), None)?.into_new();
    let smri = xfm_crs2xyz(g, out, err, &mri.vg, 0, false)?;
    let stemp = matrix_multiply_d(g, out, err, Some(&smri), inv_k.as_ref(), None)?.into_new();
    if let Some(st) = stemp.as_ref() {
        set_vox2ras_from_matrix(g, out, err, &mut t.vg, st)?;
    }
    for (title, m) in [
        ("K", Some(&k)),
        ("Kinv", inv_k.as_ref()),
        ("Smri", Some(&smri)),
        ("Stemp", stemp.as_ref()),
    ] {
        let _ = writeln!(out, "{title} ---------------");
        let mut buf: Vec<u8> = Vec::new();
        matrix_print(g, Some(&mut buf), out, err, m, 5);
        let _ = out.write_all(&buf);
    }
    let _ = writeln!(out, "----------------------");
    Ok(t)
}

/// `conform_FoV - FoV[i]` (two `int`s, in `double` after): wraps around with `reproduce`, exact by
/// default (FS-492).
fn fov_diff(g: &mut Globals, a: i32, b: i32) -> f64 {
    let exact = i64::from(a) - i64::from(b);
    let wrapped = a.wrapping_sub(b);
    if i64::from(wrapped) != exact {
        g.notices.add(
            &FS_492,
            format!("MRIconformedTemplate: the fields of view's difference {exact} wraps around"),
        );
        if g.notices.reproduce() {
            return f64::from(wrapped);
        }
    }
    exact as f64
}
