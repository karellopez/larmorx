// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Linear transforms of a volume (`utils/mri.cpp`, 3.1-36 and 3.1-37; `utils/mriset.cpp`, 3.1-38):
//! `MRIlinearTransformInterp`, `MRIlinearTransform`, `MRIinverseLinearTransform`,
//! `MRIapplyRASlinearTransform`, `MRIapplyRASlinearTransformInterp`,
//! `MRIapplyRASinverseLinearTransform`, `MRIrasXformToVoxelXform`, `MRIgetResampleMatrix` and
//! `MRIsetValues`.
//!
//! A voxel-to-voxel transform `A` (FreeSurfer's single-precision `MATRIX`) is inverted, and each
//! destination voxel `(y1, y2, y3)` samples the source at `A^-1 (y1, y2, y3, 1)` (the product in
//! [`larmorx_freesurfer_core::matrix::matrix_multiply`]'s arithmetic, the point as `float`s) with
//! [`crate::sample::sample_volume_frame_type`], one sample per source frame, stored with
//! `MRIsetVoxVal`.
//!
//! Defect (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §10):
//! - **FS-325** (memory): a destination voxel whose sample fails (sinc sampling at a point that is
//!   not whole, a source type the trilinear sampler refuses) gets the last sample that succeeded;
//!   before the first, a value never written. Both modes take 0 there, noted.
//!
//! Cubic B-spline sampling (`SAMPLE_CUBIC_BSPLINE`) needs `fs.bspline` (wave 3.2): a port error
//! until it is ported.

use std::io::Write;

use larmorx_core::defects::FS_325;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::matrix_ops::{matrix_determinant, matrix_inverse, matrix_print};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::vol_geom::{extract_i_to_r, extract_r_to_i};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;

use crate::arith::{mri_clear, mri_clone};
use crate::coords::get_slice_direction;
use crate::resize::{MRI_CORONAL, MRI_HORIZONTAL, MRI_SAGITTAL};
use crate::sample::{SAMPLE_CUBIC_BSPLINE, SAMPLE_NEAREST, SAMPLE_TRILINEAR, sample_volume_frame_type};

/// An unused bit pattern (a signalling NaN no arithmetic makes) marking a sample not written.
const UNWRITTEN: u64 = 0x7ff4_dead_beef_0002;

/// `MRIsetValues(mri, val)`: every voxel of every frame set to `val` with `MRIsetVoxVal`
/// (frames outermost, then x, y, z).
pub fn mri_set_values(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &mut Mri,
    val: f32,
) -> Result<i32, Fatal> {
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    for f in 0..mri.nframes {
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    mri_set_vox_val(g, err, mri, x, y, z, f, val, 0)?;
                }
            }
        }
    }
    Ok(NO_ERROR)
}

/// `MRIrasXformToVoxelXform(src, dst, m_ras, NULL)`: `ras_to_voxel(dst) * (m_ras *
/// voxel_to_ras(src))` (the destination the source when there is none); `None` when a product
/// fails (a singular destination geometry: `MatrixMultiply`'s error).
pub fn mri_ras_xform_to_voxel_xform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&Mri>,
    m_ras: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    let dst = dst.unwrap_or(src);
    let v2r = extract_i_to_r(g, out, err, &src.vg)?;
    let r2v = extract_r_to_i(g, out, err, &dst.vg)?;
    let tmp = matrix_multiply(g, out, err, m_ras, Some(&v2r), None)?.into_new();
    Ok(matrix_multiply(g, out, err, r2v.as_ref(), tmp.as_ref(), None)?.into_new())
}

/// `MRIlinearTransformInterp(src, dst, A, interp)`: a method other than `SAMPLE_NEAREST`,
/// `SAMPLE_TRILINEAR` and `SAMPLE_CUBIC_BSPLINE` prints `ERROR: MRIlinearTransformInterp:
/// unrecognized or unsupported interpolation method <m>` on standard output and goes on; a
/// singular `A` gives `NULL` and the error `MRIlinearTransform: xform is singular`. The
/// destination is a clone of the source (or the given one, cleared); a source whose `outside_val`
/// is not `FZERO` fills it with that value first (and passes it on). Then each destination voxel
/// (z outermost) of each source frame is the source sampled at `A^-1` of the voxel ([module
/// documentation](self)); the destination's `ras_good_flag` becomes 1.
#[allow(clippy::too_many_arguments)]
pub fn mri_linear_transform_interp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    ma: Option<&Matrix>,
    interp: i32,
) -> Result<Ret<Mri>, Fatal> {
    if interp != SAMPLE_NEAREST && interp != SAMPLE_TRILINEAR && interp != SAMPLE_CUBIC_BSPLINE {
        let _ = writeln!(
            out,
            "ERROR: MRIlinearTransformInterp: unrecognized or unsupported interpolation method {interp}"
        );
    }
    let Some(ainv) = matrix_inverse(g, out, err, ma, None)?.into_new() else {
        log::error_printf(g, err, ERROR_BADPARM, "MRIlinearTransform: xform is singular");
        return Ok(Ret::Null);
    };
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => {
            mri_clear(g, err, d)?;
            d
        }
        None => match mri_clone(g, out, err, src, None)?.into_new() {
            Some(m) => fresh.insert(m),
            None => return Ok(Ret::Null),
        },
    };
    if src.outside_val.abs() >= f64::from(f32::EPSILON) || src.outside_val.is_nan() {
        mri_set_values(g, err, d, src.outside_val as f32)?;
        d.outside_val = src.outside_val;
    }
    if interp == SAMPLE_CUBIC_BSPLINE {
        log::error_exit(
            g,
            err,
            ERROR_UNSUPPORTED,
            "MRIlinearTransformInterp: cubic B-spline sampling needs fs.bspline (not yet ported)\n",
        )?;
    }
    let (w, h, dd) = (d.vg.width, d.vg.height, d.vg.depth);
    let mut vx = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    let mut vy = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    vy.set_relt(4, 1, 1.0);
    let (mut val, mut written) = (0.0f64, false);
    for y3 in 0..dd {
        vy.set_relt(3, 1, y3 as f32);
        for y2 in 0..h {
            vy.set_relt(2, 1, y2 as f32);
            for y1 in 0..w {
                vy.set_relt(1, 1, y1 as f32);
                matrix_multiply(g, out, err, Some(&ainv), Some(&vy), Some(&mut vx))?;
                let x1 = f64::from(vx.relt(1, 1));
                let x2 = f64::from(vx.relt(2, 1));
                let x3 = f64::from(vx.relt(3, 1));
                // the diagnostic tests' nint calls (their conversions' notices)
                if nint(g, x1) == g.gx && nint(g, x2) == g.gy {
                    nint(g, x3);
                }
                for frame in 0..src.nframes {
                    let mut v = f64::from_bits(UNWRITTEN);
                    sample_volume_frame_type(g, err, src, x1, x2, x3, frame, interp, &mut v);
                    if v.to_bits() != UNWRITTEN {
                        val = v;
                        written = true;
                    } else if !written {
                        g.notices.add(
                            &FS_325,
                            "MRIlinearTransformInterp stores a sample value never written",
                        );
                    }
                    mri_set_vox_val(g, err, d, y1, y2, y3, frame, val as f32, 0)?;
                }
            }
        }
    }
    d.vg.ras_good_flag = 1;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIlinearTransform(src, dst, A)`: [`mri_linear_transform_interp`] with `SAMPLE_TRILINEAR`.
pub fn mri_linear_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    ma: Option<&Matrix>,
) -> Result<Ret<Mri>, Fatal> {
    mri_linear_transform_interp(g, out, err, src, dst, ma, SAMPLE_TRILINEAR)
}

/// `MRIinverseLinearTransform(src, dst, A)`: [`mri_linear_transform`] with `A^-1` (a singular
/// `A`: `NULL` and the error `MRIinverseLinearTransform: xform is singular!`).
pub fn mri_inverse_linear_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    ma: Option<&Matrix>,
) -> Result<Ret<Mri>, Fatal> {
    let Some(inv) = matrix_inverse(g, out, err, ma, None)?.into_new() else {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "MRIinverseLinearTransform: xform is singular!",
        );
        return Ok(Ret::Null);
    };
    mri_linear_transform(g, out, err, src, dst, Some(&inv))
}

/// `MRIapplyRASlinearTransformInterp(src, dst, m_ras, interp)`: the RAS transform made a
/// voxel-to-voxel one ([`mri_ras_xform_to_voxel_xform`] with the destination's geometry, the
/// source's when there is none), then [`mri_linear_transform_interp`].
#[allow(clippy::too_many_arguments)]
pub fn mri_apply_ras_linear_transform_interp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    m_ras: Option<&Matrix>,
    interp: i32,
) -> Result<Ret<Mri>, Fatal> {
    let mv = mri_ras_xform_to_voxel_xform(g, out, err, src, dst.as_deref(), m_ras)?;
    mri_linear_transform_interp(g, out, err, src, dst, mv.as_ref(), interp)
}

/// `MRIapplyRASlinearTransform(src, dst, m_ras)`: [`mri_apply_ras_linear_transform_interp`] with
/// `SAMPLE_TRILINEAR`.
pub fn mri_apply_ras_linear_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    m_ras: Option<&Matrix>,
) -> Result<Ret<Mri>, Fatal> {
    mri_apply_ras_linear_transform_interp(g, out, err, src, dst, m_ras, SAMPLE_TRILINEAR)
}

/// `MRIapplyRASinverseLinearTransform(src, dst, m_ras)`: the voxel-to-voxel transform of
/// [`mri_ras_xform_to_voxel_xform`], then [`mri_inverse_linear_transform`].
pub fn mri_apply_ras_inverse_linear_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    m_ras: Option<&Matrix>,
) -> Result<Ret<Mri>, Fatal> {
    let mv = mri_ras_xform_to_voxel_xform(g, out, err, src, dst.as_deref(), m_ras)?;
    mri_inverse_linear_transform(g, out, err, src, dst, mv.as_ref())
}

/// `MatrixPrint(stderr, m)` (`err` is FreeSurfer's standard error).
fn print_matrix(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, m: &Matrix) {
    let mut buf: Vec<u8> = Vec::new();
    matrix_print(g, Some(&mut buf), out, err, Some(m), 5);
    let _ = err.write_all(&buf);
}

/// `MRIgetResampleMatrix(src, template)`: `src_i_to_r^-1 * template_i_to_r`, the template's
/// voxels to the source's. `NULL` with an error: a source without a good RAS whose slice
/// direction is none of coronal, sagittal, horizontal (`MRIresample(): source volume orientation
/// is unknown`), a source or template matrix of determinant 0 (as a `float`; `MRIresample():
/// source matrix has zero determinant; matrix is:` or `destination ...`, then the matrix on
/// standard error), a source matrix that does not invert (`MRIresample(): error inverting matrix;
/// determinant is <%g>, matrix is:` and the matrix). With `Gdiag`'s show and verbose bits, the
/// matrix is printed on standard output after `MRIresample() matrix is:`.
pub fn mri_get_resample_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    template: &Mri,
) -> Result<Option<Matrix>, Fatal> {
    let dir = get_slice_direction(&src.vg);
    if src.vg.ras_good_flag == 0
        && dir != MRI_CORONAL
        && dir != MRI_SAGITTAL
        && dir != MRI_HORIZONTAL
    {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "MRIresample(): source volume orientation is unknown",
        );
        return Ok(None);
    }
    let src_mat = extract_i_to_r(g, out, err, &src.vg)?;
    let dest_mat = extract_i_to_r(g, out, err, &template.vg)?;
    let src_det = matrix_determinant(g, err, &src_mat);
    let dest_det = matrix_determinant(g, err, &dest_mat);
    for (det, m, which) in [(src_det, &src_mat, "source"), (dest_det, &dest_mat, "destination")] {
        if det == 0.0 {
            g.errno = 0;
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!("MRIresample(): {which} matrix has zero determinant; matrix is:"),
            );
            print_matrix(g, out, err, m);
            return Ok(None);
        }
    }
    let Some(src_inv) = matrix_inverse(g, out, err, Some(&src_mat), None)?.into_new() else {
        g.errno = 0;
        let det = larmorx_core::cfmt::g(f64::from(src_det), 6);
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIresample(): error inverting matrix; determinant is {det}, matrix is:"),
        );
        print_matrix(g, out, err, &src_mat);
        return Ok(None);
    };
    let m = matrix_multiply(g, out, err, Some(&src_inv), Some(&dest_mat), None)?.into_new();
    if let Some(m) = m.as_ref()
        && g.gdiag & DIAG_SHOW != 0
        && g.gdiag & DIAG_VERBOSE != 0
    {
        let _ = writeln!(out, "MRIresample() matrix is:");
        let mut buf: Vec<u8> = Vec::new();
        matrix_print(g, Some(&mut buf), out, err, Some(m), 5);
        let _ = out.write_all(&buf);
    }
    Ok(m)
}
