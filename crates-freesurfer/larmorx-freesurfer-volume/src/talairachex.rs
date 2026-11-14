// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The Talairach helpers of `utils/talairachex.cpp` (3.1-38, lines 33-315; 3.1-39's
//! `MRItoTalairachEx` and `MRIfromTalairachEx`, lines 330-347, taken with them): the Talairach
//! volume's centre (`ModifyTalairachCRAS`), the voxel/Talairach matrices (`MtalairachFromVoxel`,
//! `MtalVoxelFromVoxel`, `MvoxelFromTalairach`, `MvoxelFromTalVoxel`), a point through one of
//! them (`TransformWithMatrix`, in `float`), the point functions and the volume resamplings.
//! `MRASFromTalVoxel` and `MRItalairachVoxelToWorldEx` are reached by no program (not ported).
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §17):
//! - **FS-487** (crash or memory contents): a point function given a matrix FreeSurfer could not
//!   make (`NULL`: a singular voxel-to-RAS matrix or transform) dereferences it (both modes: an
//!   error); and the Talairach RAS to voxel matrix of a transform array that holds none reads the
//!   first transform past the array (both modes: no matrix, what that read gave on the probe's
//!   runs).
//! - **FS-488** (crash): the resampling from Talairach space without a target volume uses the
//!   target before its own check; both modes give that check's error.

use std::io::Write;

use larmorx_core::defects::{FS_487, FS_488};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_identity, matrix_inverse, matrix_print};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{add, mul};
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, Lta};
use larmorx_freesurfer_mri::mri::{Mri, mri_alloc_header, mri_copy_header};
use larmorx_freesurfer_mri::vol_geom::{extract_i_to_r, extract_r_to_i, fill_cache};

use crate::arith::mri_clone;
use crate::xform::{mri_linear_transform, mri_linear_transform_interp};

/// A `double` stored in a `float` (`cvtsd2ss`): rounded to nearest; a NaN keeps its sign and the
/// top 22 bits of its payload, quieted.
#[inline]
fn narrow(x: f64) -> f32 {
    if x.is_nan() {
        let b = x.to_bits();
        f32::from_bits(
            (((b >> 32) as u32) & 0x8000_0000) | 0x7fc0_0000 | (((b >> 29) as u32) & 0x003f_ffff),
        )
    } else {
        x as f32
    }
}

/// `(double)` of a `float` as `cvtss2sd` makes it (a signalling NaN quieted).
#[inline]
fn widen(x: f32) -> f64 {
    if x.is_nan() {
        f64::from(f32::from_bits(x.to_bits() | 0x0040_0000))
    } else {
        f64::from(x)
    }
}

/// `ModifyTalairachCRAS(mri_tal, lta)`: the Talairach volume's centre. With a transform array of
/// one `LINEAR_RAS_TO_RAS` transform: its destination's centre when that geometry is valid (with
/// `DIAG_SHOW` and `DIAG_VERBOSE`, `INFO: Modifying dst c_(r,a,s), using the transform dst` on
/// standard output); otherwise, and without a transform array, `INFO: tal c_(r,a,s) not
/// modified` on standard error when the program's environment has `NO_AVERAGE305` (whatever its
/// value), else average_305's centre (-0.095, -16.51, 9.75 as `float`s; with `DIAG_VERBOSE` its
/// `INFO:` lines on standard error). Another type: `<Progname>: xfm passed is not of RAS-to-RAS
/// type`; another count (none included): `<Progname>: xfm has more than one xfrm` (both
/// `ERROR_BADPARM`, the program ends). Then the cached matrices are recomputed; `NO_ERROR`.
pub fn modify_talairach_cras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    mri_tal: &mut Mri,
    lta: Option<&Lta>,
) -> Result<i32, Fatal> {
    let verbose = g.gdiag & DIAG_VERBOSE != 0;
    let average_305 = |vg: &mut larmorx_freesurfer_mri::vol_geom::VolGeom| {
        vg.c_r = -0.095f64 as f32;
        vg.c_a = -16.51f64 as f32;
        vg.c_s = 9.75f64 as f32;
    };
    match lta {
        Some(lta) => {
            if lta.num_xforms != 1 {
                let msg = format!("{}: xfm has more than one xfrm", g.progname);
                log::error_exit(g, err, ERROR_BADPARM, &msg)?;
                return Ok(ERROR_BADPARM);
            }
            if lta.ltype != LINEAR_RAS_TO_RAS {
                let msg = format!("{}: xfm passed is not of RAS-to-RAS type", g.progname);
                log::error_exit(g, err, ERROR_BADPARM, &msg)?;
                return Ok(ERROR_BADPARM);
            }
            let dst = &lta.xforms[0].dst;
            if dst.valid == 1 {
                if g.gdiag & DIAG_SHOW != 0 && verbose {
                    let _ = writeln!(
                        out,
                        "INFO: Modifying dst c_(r,a,s), using the transform dst"
                    );
                }
                mri_tal.vg.c_r = dst.c_r;
                mri_tal.vg.c_a = dst.c_a;
                mri_tal.vg.c_s = dst.c_s;
            } else if env.get("NO_AVERAGE305").is_some() {
                let _ = writeln!(err, "INFO: tal c_(r,a,s) not modified");
            } else {
                if verbose {
                    let _ = writeln!(
                        err,
                        "INFO: Modifying talairach volume c_(r,a,s) based on average_305."
                    );
                    let _ = writeln!(
                        err,
                        "INFO: If not preferred, set environmental varible NO_AVERAGE305 true."
                    );
                }
                average_305(&mut mri_tal.vg);
            }
        }
        None => {
            if env.get("NO_AVERAGE305").is_some() {
                let _ = writeln!(err, "INFO: tal c_(r,a,s) not modified");
            } else {
                if verbose {
                    let _ = writeln!(
                        err,
                        "INFO: Modifying talairach volume c_(r,a,s) based on average_305"
                    );
                }
                average_305(&mut mri_tal.vg);
            }
        }
    }
    // when you modify c_(ras), you must recalculate i_to_r__ and r_to_i__
    mri_tal.vg.i_to_r = None;
    mri_tal.vg.r_to_i = None;
    fill_cache(g, out, err, &mut mri_tal.vg)?;
    Ok(NO_ERROR)
}

/// The first transform's matrix of an array (`lta->xforms[0].m_L`); `None` when it has none.
fn first_matrix(lta: &Lta) -> Option<&Matrix> {
    lta.xforms.first().and_then(|x| x.m_l.as_ref())
}

/// `MtalairachFromVoxel(mri_src, lta)`: the source's voxel-to-RAS matrix (`extract_i_to_r`),
/// times the first transform's matrix on the left (`MatrixMultiply`) when a transform array is
/// given, else a copy. A transform array without transforms (`lta does not have xform`) or of
/// another type than `LINEAR_RAS_TO_RAS` (`lta must be RAS_TO_RAS transform`) ends the program
/// (`ERROR_BADPARM`). `None` where `MatrixMultiply` gives `NULL`.
pub fn mtalairach_from_voxel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    lta: Option<&Lta>,
) -> Result<Option<Matrix>, Fatal> {
    let ras_from_voxel = extract_i_to_r(g, out, err, &src.vg)?;
    Ok(match lta {
        Some(lta) => {
            if lta.num_xforms == 0 {
                log::error_exit(g, err, ERROR_BADPARM, "lta does not have xform")?;
                return Ok(None);
            }
            if lta.ltype != LINEAR_RAS_TO_RAS {
                log::error_exit(g, err, ERROR_BADPARM, "lta must be RAS_TO_RAS transform")?;
                return Ok(None);
            }
            matrix_multiply(g, out, err, first_matrix(lta), Some(&ras_from_voxel), None)?.into_new()
        }
        None => matrix_copy(g, out, err, Some(&ras_from_voxel), None)?.into_new(),
    })
}

/// The Talairach volume of `mri` (`MRIallocHeader` of its extent and type, one frame, then
/// `MRIcopyHeader` and [`modify_talairach_cras`]).
fn talairach_volume(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    mri: &Mri,
    lta: Option<&Lta>,
) -> Result<Mri, Fatal> {
    let mut tal = mri_alloc_header(
        g,
        out,
        err,
        mri.vg.width,
        mri.vg.height,
        mri.vg.depth,
        mri.mtype,
        1,
    )?;
    mri_copy_header(g, out, err, mri, Some(&mut tal))?;
    modify_talairach_cras(g, out, err, env, &mut tal, lta)?;
    Ok(tal)
}

/// `MtalVoxelFromVoxel(mri_src, lta)`: the Talairach volume's RAS-to-voxel matrix
/// (`extract_r_to_i` of [`talairach_volume`]) times [`mtalairach_from_voxel`] (`MatrixMultiply`;
/// `None` for `NULL`).
pub fn mtal_voxel_from_voxel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    lta: Option<&Lta>,
) -> Result<Option<Matrix>, Fatal> {
    let tal_ras_from_voxel = mtalairach_from_voxel(g, out, err, src, lta)?;
    let tal = talairach_volume(g, out, err, env, src, lta)?;
    let tal_voxel_from_tal_ras = extract_r_to_i(g, out, err, &tal.vg)?;
    Ok(matrix_multiply(
        g,
        out,
        err,
        tal_voxel_from_tal_ras.as_ref(),
        tal_ras_from_voxel.as_ref(),
        None,
    )?
    .into_new())
}

/// `MvoxelFromTalairach(mri_dst, lta)`: the destination's RAS-to-voxel matrix times the inverse
/// of the first transform's matrix (`MatrixInverse`), or times the identity without a transform
/// array (`None` for `NULL`). FS-487: of an array that holds no transform FreeSurfer inverts
/// whatever lies past it (on the probe's runs a `NULL` matrix: `MatrixInverse: NULL input
/// matrix!`, the program ends); both modes take no matrix, as those runs did.
pub fn mvoxel_from_talairach(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dst: &Mri,
    lta: Option<&Lta>,
) -> Result<Option<Matrix>, Fatal> {
    let ras_from_talairach = match lta {
        Some(lta) => {
            if lta.xforms.is_empty() {
                // what FreeSurfer's read past the array gave on the probe's runs: no matrix
                g.notices.add(
                    &FS_487,
                    "the Talairach-to-voxel matrix of a transform array that holds no transform (taken as none)",
                );
            }
            matrix_inverse(g, out, err, first_matrix(lta), None)?.into_new()
        }
        None => matrix_identity(g, out, err, 4, None)?.into_new(),
    };
    let voxel_from_ras = extract_r_to_i(g, out, err, &dst.vg)?;
    Ok(matrix_multiply(
        g,
        out,
        err,
        voxel_from_ras.as_ref(),
        ras_from_talairach.as_ref(),
        None,
    )?
    .into_new())
}

/// `MvoxelFromTalVoxel(mri_dst, lta)`: [`mvoxel_from_talairach`] times the Talairach volume's
/// voxel-to-RAS matrix (`extract_i_to_r` of [`talairach_volume`]; `None` for `NULL`).
pub fn mvoxel_from_tal_voxel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dst: &Mri,
    lta: Option<&Lta>,
) -> Result<Option<Matrix>, Fatal> {
    let tal = talairach_volume(g, out, err, env, dst, lta)?;
    let talairach_from_tal_vol = extract_i_to_r(g, out, err, &tal.vg)?;
    let voxel_from_tal_ras = mvoxel_from_talairach(g, out, err, dst, lta)?;
    Ok(matrix_multiply(
        g,
        out,
        err,
        voxel_from_tal_ras.as_ref(),
        Some(&talairach_from_tal_vol),
        None,
    )?
    .into_new())
}

/// `TransformWithMatrix(mat, x, y, z, &px, &py, &pz)`: the point (each coordinate stored in a
/// `float` first) through the first three rows of the 4 x 4 matrix, in `float` arithmetic
/// (`m11 x + m12 y + m13 z + m14`, left to right; the official build's operand orders, which
/// decide which of two NaNs is kept: the matrix element first in the first two rows, the
/// coordinate first in the third), each result widened to `double`.
pub fn transform_with_matrix(mat: &Matrix, x: f64, y: f64, z: f64) -> [f64; 3] {
    let d = |k: usize| mat.data()[k];
    let (xf, yf, zf) = (narrow(x), narrow(y), narrow(z));
    let px = add(add(add(mul(d(0), xf), mul(d(1), yf)), mul(d(2), zf)), d(3));
    let py = add(add(add(mul(d(4), xf), mul(d(5), yf)), mul(d(6), zf)), d(7));
    let pz = add(
        add(add(mul(xf, d(8)), mul(yf, d(9))), mul(zf, d(10))),
        d(11),
    );
    [widen(px), widen(py), widen(pz)]
}

/// A point through a matrix the caller may not have (FS-487: FreeSurfer dereferences `NULL`;
/// both modes give the error `<what>: no matrix to transform the point with` and `None`).
fn transform_or_error(
    g: &mut Globals,
    err: &mut dyn Write,
    mat: Option<&Matrix>,
    what: &str,
    p: [f64; 3],
) -> Option<[f64; 3]> {
    match mat {
        Some(m) => Some(transform_with_matrix(m, p[0], p[1], p[2])),
        None => {
            g.notices.add(
                &FS_487,
                format!("{what} transforms a point with a matrix it could not make"),
            );
            log::error(
                g,
                err,
                &format!("{what}: no matrix to transform the point with"),
            );
            None
        }
    }
}

/// `MRIvoxelToTalairachEx(mri_src, xv, yv, zv, ..., lta)`: the voxel through
/// [`mtalairach_from_voxel`] ([`transform_with_matrix`]); `NO_ERROR`. `None`: FS-487.
#[allow(clippy::too_many_arguments)]
pub fn mri_voxel_to_talairach_ex(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    xv: f64,
    yv: f64,
    zv: f64,
    lta: Option<&Lta>,
) -> Result<(i32, Option<[f64; 3]>), Fatal> {
    let m = mtalairach_from_voxel(g, out, err, src, lta)?;
    let p = transform_or_error(g, err, m.as_ref(), "MRIvoxelToTalairachEx", [xv, yv, zv]);
    Ok((NO_ERROR, p))
}

/// `MRIvoxelToTalairachVoxelEx(mri_src, xv, yv, zv, ..., lta)`: the voxel through
/// [`mtal_voxel_from_voxel`]; `NO_ERROR`. `None`: FS-487.
#[allow(clippy::too_many_arguments)]
pub fn mri_voxel_to_talairach_voxel_ex(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    xv: f64,
    yv: f64,
    zv: f64,
    lta: Option<&Lta>,
) -> Result<(i32, Option<[f64; 3]>), Fatal> {
    let m = mtal_voxel_from_voxel(g, out, err, env, src, lta)?;
    let p = transform_or_error(
        g,
        err,
        m.as_ref(),
        "MRIvoxelToTalairachVoxelEx",
        [xv, yv, zv],
    );
    Ok((NO_ERROR, p))
}

/// `MRItalairachToVoxelEx(mri_dst, xt, yt, zt, ..., lta)`: the Talairach point through
/// [`mvoxel_from_talairach`]; `NO_ERROR`. `None`: FS-487.
#[allow(clippy::too_many_arguments)]
pub fn mri_talairach_to_voxel_ex(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dst: &Mri,
    xt: f64,
    yt: f64,
    zt: f64,
    lta: Option<&Lta>,
) -> Result<(i32, Option<[f64; 3]>), Fatal> {
    let m = mvoxel_from_talairach(g, out, err, dst, lta)?;
    let p = transform_or_error(g, err, m.as_ref(), "MRItalairachToVoxelEx", [xt, yt, zt]);
    Ok((NO_ERROR, p))
}

/// `MRItalairachVoxelToVoxelEx(mri_dst, xtv, ytv, ztv, ..., lta)`: the Talairach volume's voxel
/// through [`mvoxel_from_tal_voxel`]; `NO_ERROR`. `None`: FS-487.
#[allow(clippy::too_many_arguments)]
pub fn mri_talairach_voxel_to_voxel_ex(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dst: &Mri,
    xtv: f64,
    ytv: f64,
    ztv: f64,
    lta: Option<&Lta>,
) -> Result<(i32, Option<[f64; 3]>), Fatal> {
    let m = mvoxel_from_tal_voxel(g, out, err, env, dst, lta)?;
    let p = transform_or_error(
        g,
        err,
        m.as_ref(),
        "MRItalairachVoxelToVoxelEx",
        [xtv, ytv, ztv],
    );
    Ok((NO_ERROR, p))
}

/// `MatrixPrint(stderr, m)` (5 decimals; `NULL` gives its error).
fn print_on(g: &mut Globals, err: &mut dyn Write, m: Option<&Matrix>) {
    let mut buf = Vec::new();
    matrix_print(
        g,
        Some(&mut buf as &mut dyn Write),
        &mut std::io::sink(),
        err,
        m,
        5,
    );
    let _ = err.write_all(&buf);
}

/// The resampling of `MRItoTalairachExInterp` and `MRItoTalairachEx`: the matrix
/// ([`mtal_voxel_from_voxel`]) after `voxel to talairach voxel transform` on standard error
/// (`MatrixPrint`), the destination a clone of the source (its voxels not copied) with
/// [`modify_talairach_cras`] when none is given, then `MRIlinearTransformInterp` (`interp`
/// `None`: `MRIlinearTransform`, trilinear) into it, whose result is not looked at.
#[allow(clippy::too_many_arguments)]
fn to_talairach(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    tal: Option<&mut Mri>,
    lta: Option<&Lta>,
    interp: Option<i32>,
) -> Result<Ret<Mri>, Fatal> {
    let m = mtal_voxel_from_voxel(g, out, err, env, src, lta)?;
    let _ = writeln!(err, "voxel to talairach voxel transform");
    print_on(g, err, m.as_ref());
    let mut fresh = None;
    let given = tal.is_some();
    let t: &mut Mri = match tal {
        Some(t) => t,
        None => {
            let mut c = mri_clone(g, out, err, src, None)?
                .into_new()
                .expect("MRIclone without a destination");
            modify_talairach_cras(g, out, err, env, &mut c, lta)?;
            fresh.insert(c)
        }
    };
    match interp {
        Some(i) => {
            mri_linear_transform_interp(g, out, err, src, Some(t), m.as_ref(), i)?;
        }
        None => {
            mri_linear_transform(g, out, err, src, Some(t), m.as_ref())?;
        }
    }
    Ok(match fresh {
        Some(c) if !given => Ret::New(c),
        _ => Ret::Given,
    })
}

/// `MRItoTalairachExInterp(mri_src, mri_tal, lta, interp)`: the source resampled into its
/// Talairach volume with `interp` (`to_talairach`, above).
#[allow(clippy::too_many_arguments)]
pub fn mri_to_talairach_ex_interp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    tal: Option<&mut Mri>,
    lta: Option<&Lta>,
    interp: i32,
) -> Result<Ret<Mri>, Fatal> {
    to_talairach(g, out, err, env, src, tal, lta, Some(interp))
}

/// `MRItoTalairachEx(mri_src, mri_tal, lta)`: as [`mri_to_talairach_ex_interp`] with
/// `MRIlinearTransform` (trilinear).
pub fn mri_to_talairach_ex(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    tal: Option<&mut Mri>,
    lta: Option<&Lta>,
) -> Result<Ret<Mri>, Fatal> {
    to_talairach(g, out, err, env, src, tal, lta, None)
}

/// `MRIfromTalairachEx(mri_tal, mri_dst, lta)`: the matrix ([`mvoxel_from_tal_voxel`] of the
/// destination) after `talairach voxel to voxel transform` on standard error (`MatrixPrint`), then
/// `MRIlinearTransform` of the Talairach volume into the destination, whose result is not looked
/// at; `Given`. FS-488: without a destination FreeSurfer uses it before its own check; both modes
/// end with that check's `<Progname>: Needs target volume to recover c_(ras)` (`ERROR_BADPARM`).
pub fn mri_from_talairach_ex(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    tal: &Mri,
    dst: Option<&mut Mri>,
    lta: Option<&Lta>,
) -> Result<Ret<Mri>, Fatal> {
    let Some(dst) = dst else {
        g.notices.add(
            &FS_488,
            "the resampling from Talairach space was given no target volume",
        );
        let msg = format!("{}: Needs target volume to recover c_(ras)", g.progname);
        log::error_exit(g, err, ERROR_BADPARM, &msg)?;
        return Ok(Ret::Null);
    };
    let m = mvoxel_from_tal_voxel(g, out, err, env, dst, lta)?;
    let _ = writeln!(err, "talairach voxel to voxel transform");
    print_on(g, err, m.as_ref());
    mri_linear_transform(g, out, err, tal, Some(dst), m.as_ref())?;
    Ok(Ret::Given)
}
