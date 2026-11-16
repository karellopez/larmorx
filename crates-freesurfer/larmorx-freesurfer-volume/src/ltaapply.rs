// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A linear transform array applied (`utils/transform.cpp`, 3.1-39 and 3.1-40): `LTAtransform`
//! (907) and `LTAtransformInterp` (912) with `LTAtransformAtPoint` (1388), its inverse
//! `LTAinverseTransformInterp` (1157, for track A's `TransformApplyInverse`), the conversion of a
//! voxel transform to coronal RAS (`LTAvoxelTransformToCoronalRasTransform`, 1946), the MNI to
//! Talairach correction (`FixMNITal`, 1983), a subject's transform made to work with tkregister's
//! coordinates (`DevolveXFM` 2027, `DevolveXFMWithSubjectsDir` 2032) and the arrays' geometries
//! taken from volumes (`LTAmodifySrcDstGeom`, 3582).
//!
//! The products are FreeSurfer's `MatrixMultiply` (`float`), the inverses its `MatrixInverse`; a
//! relative name is opened in `dir`, the program's working directory, when given. The C library's
//! `exp` is glibc's with `reproduce`, correctly rounded by default (GLIBC-07 where they differ).
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-volume.md` §26):
//! - **FS-504** (memory, crash): an array without transforms, or a transform without its matrix,
//!   where FreeSurfer reads one: an error in both modes.
//! - **FS-886** (memory): `LTAtransformAtPoint` keeps each transform's weight in an array of 4,096:
//!   an array of more transforms writes past it; an error in both modes.
//! - **FS-887** (memory): `DevolveXFMWithSubjectsDir` builds its paths in 2,000 bytes: a longer
//!   subject folder writes past them; an error in both modes.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_886, FS_887};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE, diag_heartbeat};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{
    matrix_add, matrix_clear, matrix_inverse, matrix_scalar_mul,
};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::fio::{fio_file_exists_readable, fio_is_directory};
use larmorx_freesurfer_io::identify::{MRI_CORONAL_SLICE_DIRECTORY, MRI_VOLUME_TYPE_UNKNOWN};
use larmorx_freesurfer_io::mriio::mri_read_header;
use larmorx_freesurfer_mri::lta::{
    LINEAR_PHYSVOX_TO_PHYSVOX, LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, Lta, REGISTER_DAT,
};
use larmorx_freesurfer_mri::mri::{Mri, mri_alloc_sequence, mri_copy_header, use_vol_geom_to_mri};
use larmorx_freesurfer_mri::tkreg::xfm_crs2xyz_tkreg;
use larmorx_freesurfer_mri::vol_geom::{
    VolGeom, extract_i_to_r, extract_r_to_i, reinit_cache, vgprint, xfm_crs2xyz,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::mri_clone;
use crate::deriv::{Axis, index};
use crate::ltaops::{lta_change_type, outside};
use crate::ltaread::{LtaIo, lta_read_ex};
use crate::orient::stuff_four_by_four;
use crate::sample::SAMPLE_TRILINEAR;
use crate::vgeom::{get_vol_geom, vg_i_to_r, vg_is_equal, vg_r_to_i};
use crate::xform::mri_apply_ras_linear_transform_interp;

/// `MAX_TRANSFORMS` (`transform.cpp`): the size of `LTAtransformAtPoint`'s weight array.
const MAX_TRANSFORMS: i32 = 1024 * 4;

/// The C library's `exp` of a NaN: the NaN, quieted.
fn exp_nan(libm: &Libm, x: f64) -> f64 {
    if x.is_nan() {
        f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000)
    } else {
        libm.exp(x)
    }
}

/// `LTAtransformAtPoint(lta, x, y, z, m_L)`: the transforms' weighted sum at the point: each
/// transform's weight `exp(-d² / (2 sigma²))` (`d` the distance from its centre, each difference
/// in `float`, the rest in `double`), normalised by their total; a transform whose share is below
/// `0.1 / n` is left out, each other one's matrix times its share (as a `float`) added in turn
/// (`MatrixScalarMul`, `MatrixAdd`, in `float`). A total within 1e-15 of 0 gives the first
/// transform's matrix. Into `m_l` (cleared first) when given, else a new 4 x 4 matrix. FS-504,
/// FS-886.
#[allow(clippy::too_many_arguments)]
pub fn lta_transform_at_point(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta: &Lta,
    x: f32,
    y: f32,
    z: f32,
    m_l: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let mut fresh = None;
    let given = m_l.is_some();
    let m: &mut Matrix = match m_l {
        Some(m) => {
            matrix_clear(g, m);
            m
        }
        None => fresh.insert(Matrix::real(4, 4)),
    };
    let n = lta.num_xforms;
    if n > MAX_TRANSFORMS {
        g.notices.add(
            &FS_886,
            format!(
                "LTAtransformAtPoint of {n} transforms (its weight array holds {MAX_TRANSFORMS})"
            ),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "LTAtransformAtPoint: {n} transforms, more than its {MAX_TRANSFORMS} weights (FreeSurfer writes past its array here)\n"
            ),
        )?;
    }
    let libm = Libm::new(g.mode());
    let nx = n.max(0) as usize;
    let mut w_p = Vec::with_capacity(nx);
    let mut wtotal = 0.0f64;
    for lt in &lta.xforms[..nx] {
        let dx = f64::from(sse::sub(lt.x0, x));
        let dy = f64::from(sse::sub(lt.y0, y));
        let dz = f64::from(sse::sub(lt.z0, z));
        let dsq = sse::add64(
            sse::add64(sse::mul64(dx, dx), sse::mul64(dy, dy)),
            sse::mul64(dz, dz),
        );
        let sigma = f64::from(lt.sigma);
        let s2 = sse::mul64(sse::mul64(2.0, sigma), sigma);
        let w = exp_nan(&libm, sse::div64(-dsq, s2));
        w_p.push(w);
        wtotal = sse::add64(wtotal, w);
    }
    note_glibc07(g, &libm, "exp");
    if wtotal.abs() < 1e-15 {
        let Some(m0) = lta
            .xforms
            .first()
            .filter(|_| n > 0)
            .and_then(|lt| lt.m_l.as_ref())
        else {
            outside(
                g,
                err,
                "LTAtransformAtPoint",
                "no transform's matrix to copy",
            )?;
            return Ok(Ret::Null);
        };
        let m0 = m0.clone();
        matrix_copy(g, out, err, Some(&m0), Some(m))?;
    } else {
        let wmin = 0.1 / f64::from(n);
        let mut m_tmp = Matrix::real(4, 4);
        for (lt, &w) in lta.xforms[..nx].iter().zip(&w_p) {
            let w_k_p = sse::div64(w, wtotal);
            if w_k_p < wmin {
                continue;
            }
            let Some(ml) = lt.m_l.as_ref() else {
                outside(
                    g,
                    err,
                    "LTAtransformAtPoint",
                    "a transform without its matrix",
                )?;
                return Ok(Ret::Null);
            };
            matrix_scalar_mul(g, out, err, ml, w_k_p as f32, Some(&mut m_tmp))?;
            let acc = m.clone();
            matrix_add(g, out, err, &acc, &m_tmp, Some(m))?;
        }
    }
    Ok(match fresh {
        Some(f) => Ret::New(f),
        None if given => Ret::Given,
        None => Ret::Null,
    })
}

/// `LTAtransform(mri_src, mri_dst, lta)`: [`lta_transform_interp`] with `SAMPLE_TRILINEAR`.
#[allow(clippy::too_many_arguments)]
pub fn lta_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    dst: Option<&mut Mri>,
    lta: &mut Lta,
) -> Result<Ret<Mri>, Fatal> {
    lta_transform_interp(g, out, err, env, src, dst, lta, SAMPLE_TRILINEAR)
}

/// The destination `LTAtransformInterp` makes when it is given none (one transform): with the
/// transform's destination geometry valid (1), a volume of that extent and the source's type and
/// frames with the source's header and that geometry; else, with `USE_AVERAGE305` set (any
/// value), the source's clone with no Talairach file name, the centre of `average_305` and its
/// cached matrices recomputed; else the source's clone with no Talairach file name.
fn new_destination(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    tran: &VolGeom,
) -> Result<Mri, Fatal> {
    if tran.valid == 1 {
        if g.gdiag & DIAG_VERBOSE != 0 {
            let _ = err.write_all(b"INFO: Modifying dst geometry, using the transform dst\n");
        }
        let mut d = mri_alloc_sequence(
            g,
            out,
            err,
            tran.width,
            tran.height,
            tran.depth,
            src.mtype,
            src.nframes,
        )?;
        mri_copy_header(g, out, err, src, Some(&mut d))?;
        use_vol_geom_to_mri(g, out, err, tran, &mut d.vg)?;
        return Ok(d);
    }
    if env.get("USE_AVERAGE305").is_some() {
        let _ = err.write_all(b"INFO: Environmental variable USE_AVERAGE305 set\n");
        let _ = err.write_all(b"INFO: Modifying dst c_(r,a,s), using average_305 values\n");
        let mut d = mri_clone(g, out, err, src, None)?.unwrap_new();
        d.transform_fname.clear();
        d.vg.c_r = -0.0950f64 as f32;
        d.vg.c_a = -16.5100f64 as f32;
        d.vg.c_s = 9.7500f64 as f32;
        d.vg.ras_good_flag = 1;
        reinit_cache(g, out, err, &mut d.vg)?;
        return Ok(d);
    }
    let _ = err.write_all(b"INFO: Transform dst volume info is not used (valid flag = 0).\n");
    let mut d = mri_clone(g, out, err, src, None)?.unwrap_new();
    d.transform_fname.clear();
    Ok(d)
}

/// `LTAtransformInterp(mri_src, mri_dst, lta, interp)`. A `REGISTER_DAT` array is first changed
/// to `LINEAR_VOX_TO_VOX` (`warning: changing input transform type from REGISTER_DAT to
/// VOX_TO_VOX` on standard output). With one transform (and no destination given, the one
/// [`new_destination`] makes): `LINEAR_RAS_TO_RAS` is `MRIapplyRASlinearTransformInterp` of its
/// matrix; `LINEAR_VOX_TO_VOX` of the RAS matrix `i2r(dst) M r2i(src)` (the transform's
/// geometries where valid, else, with an `INFO: LTA dst (src) geometry information missing!`
/// message on standard error, the destination's or the source's: `float` products, the left one
/// first); `LINEAR_PHYSVOX_TO_PHYSVOX` changes the array to `LINEAR_RAS_TO_RAS` first; another
/// type ends the program (`LTAtransform: unknown linear transform`). Any other number of
/// transforms (`applying octree transform to image...` on standard error; a clone of the source
/// when no destination is given): every voxel of the source's extent takes the source's voxel at
/// the destination's index tables' entries for the rounded point the inverse of
/// [`lta_transform_at_point`] maps it to (the first frames of the destination; a voxel whose
/// matrix does not invert keeps its value). FS-504, FS-886.
#[allow(clippy::too_many_arguments)]
pub fn lta_transform_interp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    dst: Option<&mut Mri>,
    lta: &mut Lta,
    interp: i32,
) -> Result<Ret<Mri>, Fatal> {
    if lta.ltype == REGISTER_DAT {
        let _ = out
            .write_all(b"warning: changing input transform type from REGISTER_DAT to VOX_TO_VOX\n");
        lta_change_type(g, out, err, env, lta, LINEAR_VOX_TO_VOX)?;
    }
    let mut own = None;
    let given = dst.is_some();
    if lta.num_xforms == 1 {
        let d: &mut Mri = match dst {
            Some(d) => d,
            None => {
                let tran = lta.xforms[0].dst.clone();
                own.insert(new_destination(g, out, err, env, src, &tran)?)
            }
        };
        let r = if lta.ltype == LINEAR_RAS_TO_RAS {
            let m = lta.xforms[0].m_l.clone();
            mri_apply_ras_linear_transform_interp(g, out, err, src, Some(d), m.as_ref(), interp)?
        } else if lta.ltype == LINEAR_VOX_TO_VOX {
            let lt = &lta.xforms[0];
            let i2r = if lt.dst.valid != 0 {
                vg_i_to_r(g, out, err, &lt.dst)?
            } else {
                let _ = err.write_all(
                    b"INFO: LTA dst geometry information missing!\n      We assume that the dst volume passed is the\n      same as the dst for the transform.\n",
                );
                extract_i_to_r(g, out, err, &d.vg)?
            };
            let tmp = matrix_multiply(g, out, err, Some(&i2r), lt.m_l.as_ref(), None)?.into_new();
            let r2i = if lt.src.valid != 0 {
                vg_r_to_i(g, out, err, &lt.src)?
            } else {
                let _ = err.write_all(
                    b"INFO: LTA src geometry information missing!\n      We assume that the src volume passed is the\n      same as the src for the transform.\n",
                );
                extract_r_to_i(g, out, err, &src.vg)?
            };
            // MatrixMultiply(tmp, r2i, tmp): FreeSurfer multiplies a copy of tmp into it
            let tmp = match tmp {
                Some(mut t) => {
                    let t0 = t.clone();
                    match matrix_multiply(g, out, err, Some(&t0), r2i.as_ref(), Some(&mut t))? {
                        Ret::Null => None,
                        _ => Some(t),
                    }
                }
                None => matrix_multiply(g, out, err, None, r2i.as_ref(), None)?.into_new(),
            };
            mri_apply_ras_linear_transform_interp(g, out, err, src, Some(d), tmp.as_ref(), interp)?
        } else if lta.ltype == LINEAR_PHYSVOX_TO_PHYSVOX {
            lta_change_type(g, out, err, env, lta, LINEAR_RAS_TO_RAS)?;
            let m = lta.xforms[0].m_l.clone();
            mri_apply_ras_linear_transform_interp(g, out, err, src, Some(d), m.as_ref(), interp)?
        } else {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "LTAtransform: unknown linear transform\n",
            )?;
            Ret::Null
        };
        return Ok(match (r, own) {
            (Ret::Given, Some(d)) => Ret::New(d),
            (Ret::Given, None) => Ret::Given,
            (Ret::New(m), _) => Ret::New(m),
            (Ret::Null, _) => Ret::Null,
        });
    }
    let _ = err.write_all(b"applying octree transform to image...\n");
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => own.insert(mri_clone(g, out, err, src, None)?.unwrap_new()),
    };
    octree(g, out, err, src, d, lta, "LTAtransformInterp")?;
    Ok(match own {
        Some(d) => Ret::New(d),
        None if given => Ret::Given,
        None => Ret::Null,
    })
}

/// The destination `LTAinverseTransformInterp` makes when it is given none (one transform): the
/// source's clone; with the inverse transform's destination geometry valid (1) its centre (`INFO:
/// Modifying dst c_(r,a,s), using the transform dst` on standard error with `DIAG_VERBOSE`), else
/// with `USE_AVERAGE305` set (any value) the centre of `average_305`, either way with
/// `ras_good_flag` 1 and the cached matrices recomputed; else the clone as it is (`INFO: Transform
/// dst volume info is not used (valid flag = 0).`). The clone keeps the source's Talairach file
/// name.
fn new_inverse_destination(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    tran: &VolGeom,
) -> Result<Mri, Fatal> {
    let mut d = mri_clone(g, out, err, src, None)?.unwrap_new();
    if tran.valid == 1 {
        if g.gdiag & DIAG_VERBOSE != 0 {
            let _ = err.write_all(b"INFO: Modifying dst c_(r,a,s), using the transform dst\n");
        }
        d.vg.c_r = tran.c_r;
        d.vg.c_a = tran.c_a;
        d.vg.c_s = tran.c_s;
        d.vg.ras_good_flag = 1;
        reinit_cache(g, out, err, &mut d.vg)?;
    } else if env.get("USE_AVERAGE305").is_some() {
        let _ = err.write_all(b"INFO: Environmental variable USE_AVERAGE305 set\n");
        let _ = err.write_all(b"INFO: Modifying dst c_(r,a,s), using average_305 values\n");
        d.vg.c_r = -0.0950f64 as f32;
        d.vg.c_a = -16.5100f64 as f32;
        d.vg.c_s = 9.7500f64 as f32;
        d.vg.ras_good_flag = 1;
        reinit_cache(g, out, err, &mut d.vg)?;
    } else {
        let _ = err.write_all(b"INFO: Transform dst volume info is not used (valid flag = 0).\n");
    }
    Ok(d)
}

/// `LTAinverseTransformInterp(mri_src, mri_dst, lta, interp)`: [`lta_transform_interp`] with the
/// array's inverse transforms (`inv_xforms`, as `LTAfillInverse` fills them): with one transform
/// (and no destination given, the one [`new_inverse_destination`] makes), `LINEAR_RAS_TO_RAS` is
/// `MRIapplyRASlinearTransformInterp` of the inverse's matrix; `LINEAR_VOX_TO_VOX` of the RAS
/// matrix `i2r(dst) M r2i(src)` of the inverse (its geometries where valid, else, with `INFO: LTA
/// inv dst (src) geometry information missing!` on standard error, the destination's or the
/// source's); `LINEAR_PHYSVOX_TO_PHYSVOX` changes the array to `LINEAR_RAS_TO_RAS` first (which
/// fills its inverses again); another type (`REGISTER_DAT` included: no change of type here) ends
/// the program (`LTAtransform: unknown linear transform`). Any other number of transforms: the
/// octree part of [`lta_transform_interp`], unchanged (the forward transforms, inverted at each
/// voxel). FS-504, FS-886.
#[allow(clippy::too_many_arguments)]
pub fn lta_inverse_transform_interp(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &Mri,
    dst: Option<&mut Mri>,
    lta: &mut Lta,
    interp: i32,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "LTAinverseTransformInterp";
    let mut own = None;
    let given = dst.is_some();
    if lta.num_xforms == 1 {
        if lta.inv_xforms.is_empty() {
            outside(g, err, WHAT, "an array without its inverse transforms")?;
            return Ok(Ret::Null);
        }
        let d: &mut Mri = match dst {
            Some(d) => d,
            None => {
                let tran = lta.inv_xforms[0].dst.clone();
                own.insert(new_inverse_destination(g, out, err, env, src, &tran)?)
            }
        };
        let r = if lta.ltype == LINEAR_RAS_TO_RAS {
            let m = lta.inv_xforms[0].m_l.clone();
            mri_apply_ras_linear_transform_interp(g, out, err, src, Some(d), m.as_ref(), interp)?
        } else if lta.ltype == LINEAR_VOX_TO_VOX {
            let lt = &lta.inv_xforms[0];
            let i2r = if lt.dst.valid != 0 {
                vg_i_to_r(g, out, err, &lt.dst)?
            } else {
                let _ = err.write_all(
                    b"INFO: LTA inv dst geometry information missing!\n      We assume that the dst volume passed is the\n      same as the dst for the transform.\n",
                );
                extract_i_to_r(g, out, err, &d.vg)?
            };
            let tmp = matrix_multiply(g, out, err, Some(&i2r), lt.m_l.as_ref(), None)?.into_new();
            let r2i = if lt.src.valid != 0 {
                vg_r_to_i(g, out, err, &lt.src)?
            } else {
                let _ = err.write_all(
                    b"INFO: LTA inv src geometry information missing!\n      We assume that the src volume passed is the\n      same as the src for the transform.\n",
                );
                extract_r_to_i(g, out, err, &src.vg)?
            };
            // MatrixMultiply(tmp, r2i, tmp): FreeSurfer multiplies a copy of tmp into it
            let tmp = match tmp {
                Some(mut t) => {
                    let t0 = t.clone();
                    match matrix_multiply(g, out, err, Some(&t0), r2i.as_ref(), Some(&mut t))? {
                        Ret::Null => None,
                        _ => Some(t),
                    }
                }
                None => matrix_multiply(g, out, err, None, r2i.as_ref(), None)?.into_new(),
            };
            mri_apply_ras_linear_transform_interp(g, out, err, src, Some(d), tmp.as_ref(), interp)?
        } else if lta.ltype == LINEAR_PHYSVOX_TO_PHYSVOX {
            lta_change_type(g, out, err, env, lta, LINEAR_RAS_TO_RAS)?;
            let m = lta.inv_xforms[0].m_l.clone();
            mri_apply_ras_linear_transform_interp(g, out, err, src, Some(d), m.as_ref(), interp)?
        } else {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "LTAtransform: unknown linear transform\n",
            )?;
            Ret::Null
        };
        return Ok(match (r, own) {
            (Ret::Given, Some(d)) => Ret::New(d),
            (Ret::Given, None) => Ret::Given,
            (Ret::New(m), _) => Ret::New(m),
            (Ret::Null, _) => Ret::Null,
        });
    }
    let _ = err.write_all(b"applying octree transform to image...\n");
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => own.insert(mri_clone(g, out, err, src, None)?.unwrap_new()),
    };
    octree(g, out, err, src, d, lta, WHAT)?;
    Ok(match own {
        Some(d) => Ret::New(d),
        None if given => Ret::Given,
        None => Ret::Null,
    })
}

/// The octree part of `LTAtransformInterp` and `LTAinverseTransformInterp` (the same in both):
/// every voxel of the source's extent takes the source's voxel at the destination's index tables'
/// entries for the rounded point the inverse of [`lta_transform_at_point`] maps it to (the first
/// frames of the destination; a voxel whose matrix does not invert keeps its value); an array of
/// no transforms maps every voxel to the point 0. `what` names the function in the index tables'
/// messages.
#[allow(clippy::too_many_arguments)]
fn octree(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    d: &mut Mri,
    lta: &Lta,
    what: &str,
) -> Result<(), Fatal> {
    let (width, height, depth) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut v_x = Matrix::real(4, 1);
    let mut v_y = Matrix::real(4, 1);
    let mut m_l = Matrix::real(4, 4);
    let mut m_l_inv = Matrix::real(4, 4);
    v_y.set_relt(4, 1, 1.0);
    // one transform cannot reach here (it returned above): the zero matrix stays the inverse
    // for an array of none
    for y3 in 0..depth {
        diag_heartbeat(g, err, sse::div(y3 as f32, (depth - 1) as f32));
        v_y.set_relt(3, 1, y3 as f32);
        for y2 in 0..height {
            v_y.set_relt(2, 1, y2 as f32);
            for y1 in 0..width {
                v_y.set_relt(1, 1, y1 as f32);
                if lta.num_xforms > 1 {
                    lta_transform_at_point(
                        g,
                        out,
                        err,
                        lta,
                        y1 as f32,
                        y2 as f32,
                        y3 as f32,
                        Some(&mut m_l),
                    )?;
                    let ml = m_l.clone();
                    if matrix_inverse(g, out, err, Some(&ml), Some(&mut m_l_inv))?.is_null() {
                        continue;
                    }
                }
                matrix_multiply(g, out, err, Some(&m_l_inv), Some(&v_y), Some(&mut v_x))?;
                let x1 = f64::from(v_x.relt(1, 1));
                let x2 = f64::from(v_x.relt(2, 1));
                let x3 = f64::from(v_x.relt(3, 1));
                let ix = nint(g, x1);
                let xi = index(g, err, what, d, Axis::X, ix)?;
                let iy = nint(g, x2);
                let yi = index(g, err, what, d, Axis::Y, iy)?;
                let iz = nint(g, x3);
                let zi = index(g, err, what, d, Axis::Z, iz)?;
                for f in 0..d.nframes {
                    let v = mri_get_vox_val(g, err, src, xi, yi, zi, f, 0)?;
                    mri_set_vox_val(g, err, d, y1, y2, y3, f, v, 0)?;
                }
            }
        }
    }
    Ok(())
}

/// The world-to-voxel and voxel-to-world matrices of a 256^3 coronal volume
/// (`LTAvoxelTransformToCoronalRasTransform`'s `V` and `W`).
fn coronal_v_w() -> (Matrix, Matrix) {
    let mut v = Matrix::real(4, 4);
    let mut w = Matrix::real(4, 4);
    for (r, c, x) in [
        (1, 1, -1.0),
        (1, 4, 128.0),
        (2, 3, -1.0),
        (2, 4, 128.0),
        (3, 2, 1.0),
        (3, 4, 128.0),
        (4, 4, 1.0),
    ] {
        v.set_relt(r, c, x);
    }
    for (r, c, x) in [
        (1, 1, -1.0),
        (1, 4, 128.0),
        (2, 3, 1.0),
        (2, 4, -128.0),
        (3, 2, -1.0),
        (3, 4, 128.0),
        (4, 4, 1.0),
    ] {
        w.set_relt(r, c, x);
    }
    (v, w)
}

/// `LTAvoxelTransformToCoronalRasTransform(lta)`: the first transform's matrix `M` becomes `W (M
/// V)` (`V` a 256^3 coronal volume's world-to-voxel matrix, `W` its voxel-to-world one; `float`
/// products), the array's type `LINEAR_RAS_TO_RAS` (the transform's own type unchanged).
/// `NO_ERROR`. FS-504.
pub fn lta_voxel_transform_to_coronal_ras_transform(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta: &mut Lta,
) -> Result<i32, Fatal> {
    let (v, w) = coronal_v_w();
    if lta.num_xforms <= 0 || lta.xforms.is_empty() {
        outside(
            g,
            err,
            "LTAvoxelTransformToCoronalRasTransform",
            "an array without transforms",
        )?;
        return Ok(ERROR_BADPARM);
    }
    let m_tmp =
        matrix_multiply(g, out, err, lta.xforms[0].m_l.as_ref(), Some(&v), None)?.into_new();
    match lta.xforms[0].m_l.as_mut() {
        Some(ml) => {
            matrix_multiply(g, out, err, Some(&w), m_tmp.as_ref(), Some(ml))?;
        }
        None => {
            // MatrixMultiply(W, m_tmp, NULL): a new product FreeSurfer drops
            matrix_multiply(g, out, err, Some(&w), m_tmp.as_ref(), None)?;
        }
    }
    lta.ltype = LINEAR_RAS_TO_RAS;
    Ok(NO_ERROR)
}

/// `FixMNITal(xmni, ymni, zmni, &xtal, &ytal, &ztal)`: Matthew Brett's MNI-to-Talairach matrix
/// (the one for `z >= 0`, or below 0 the other; its numbers as `float`) times the point (`float`
/// products). Returns the Talairach point (the C function's 0).
pub fn fix_mni_tal(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    xmni: f32,
    ymni: f32,
    zmni: f32,
) -> Result<(f32, f32, f32), Fatal> {
    let mut t = Matrix::real(4, 4);
    let v: [f64; 16] = if zmni >= 0.0 {
        [
            0.9900, 0.0000, 0.0000, 0.0, 0.0000, 0.9688, 0.0460, 0.0, 0.0000, -0.0485, 0.9189, 0.0,
            0.0000, 0.0000, 0.0000, 1.0,
        ]
    } else {
        [
            0.9900, 0.0000, 0.0000, 0.0, 0.0000, 0.9688, 0.0420, 0.0, 0.0000, -0.0485, 0.8390, 0.0,
            0.0000, 0.0000, 0.0000, 1.0,
        ]
    };
    stuff_four_by_four(g, err, Some(&mut t), &v.map(|x| x as f32));
    let mut p = Matrix::real(4, 1);
    p.set_relt(1, 1, xmni);
    p.set_relt(2, 1, ymni);
    p.set_relt(3, 1, zmni);
    p.set_relt(4, 1, 1.0);
    let r = matrix_multiply(g, out, err, Some(&t), Some(&p), None)?.unwrap_new();
    Ok((r.relt(1, 1), r.relt(2, 1), r.relt(3, 1)))
}

/// `LTAmodifySrcDstGeom(lta, src, dst)`: for each transform, with `src` given, the source
/// geometry (`getVolGeom` of `src`) replaces the transform's when that one is not valid (0) or
/// differs (`vg_isEqual`; then `INFO: src volume info differs from the one stored in lta. gets
/// modified now.` on standard error and both geometries printed by `vgprint`, the volume's
/// first); the same for the destination (`INFO: dst volume info differs from the one stored in
/// lta.  gets modified now.`). 0.
pub fn lta_modify_src_dst_geom(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta: &mut Lta,
    src: Option<&Mri>,
    dst: Option<&Mri>,
) -> i32 {
    let mut svg = VolGeom::default();
    let mut dvg = VolGeom::default();
    let n = lta.num_xforms.max(0) as usize;
    for lt in &mut lta.xforms[..n] {
        for (vol, vg, mine, what) in [
            (
                src,
                &mut svg,
                &mut lt.src,
                "INFO: src volume info differs from the one stored in lta. gets modified now.\n",
            ),
            (
                dst,
                &mut dvg,
                &mut lt.dst,
                "INFO: dst volume info differs from the one stored in lta.  gets modified now.\n",
            ),
        ] {
            let Some(vol) = vol else { continue };
            if mine.valid != 0 {
                get_vol_geom(&vol.vg, vg);
                if vg_is_equal(g, mine, vg) == 0 {
                    let _ = err.write_all(what.as_bytes());
                    vgprint(out, vg, false);
                    vgprint(out, mine, false);
                    get_vol_geom(&vol.vg, mine);
                }
            } else {
                get_vol_geom(&vol.vg, mine);
            }
        }
    }
    0
}

/// `sprintf` into `DevolveXFMWithSubjectsDir`'s 2,000-byte buffers (FS-887 past them).
fn path2000(g: &mut Globals, err: &mut dyn Write, s: String) -> Result<String, Fatal> {
    if s.len() >= 2000 {
        g.notices.add(
            &FS_887,
            format!(
                "DevolveXFMWithSubjectsDir: a path of {} bytes (its buffer holds 2,000)",
                s.len()
            ),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "DevolveXFMWithSubjectsDir: a path longer than its 2,000-byte buffer (FreeSurfer writes past it here)\n",
        )?;
    }
    Ok(s)
}

/// `MRIreadHeader` of the subject's original volume when the name opens for reading (or, for the
/// slice folder, is a folder), with `Trying <name>` on standard output under `DIAG_SHOW` and
/// `DIAG_VERBOSE`.
#[allow(clippy::too_many_arguments)]
fn try_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    name: &str,
    folder: bool,
) -> Result<Option<Mri>, Fatal> {
    if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let _ = writeln!(out, "Trying {name}");
    }
    if folder {
        if fio_is_directory(g, dir, name) != 0 {
            return mri_read_header(g, out, err, env, dir, name, MRI_CORONAL_SLICE_DIRECTORY);
        }
    } else if fio_file_exists_readable(g, dir, name) != 0 {
        return mri_read_header(g, out, err, env, dir, name, MRI_VOLUME_TYPE_UNKNOWN);
    }
    Ok(None)
}

/// `DevolveXFM(subjid, XFM, xfmname)`: [`devolve_xfm_with_subjects_dir`] with `SUBJECTS_DIR`.
#[allow(clippy::too_many_arguments)]
pub fn devolve_xfm(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    subjid: &str,
    xfm: Option<&mut Matrix>,
    xfmname: Option<&str>,
    io: &mut dyn LtaIo,
) -> Result<Ret<Matrix>, Fatal> {
    devolve_xfm_with_subjects_dir(g, out, err, dir, subjid, xfm, xfmname, None, io)
}

/// `DevolveXFMWithSubjectsDir(subjid, XFM, xfmname, sdir)`: `XFM` times `Torig_native
/// inv(Torig_tkreg)` (the subject's original volume's scanner and tkregister voxel-to-RAS
/// matrices; `float` products, `XFM`'s copy on the left), so that the transform works with
/// tkregister's coordinates. The subjects' folder is `sdir`, else `SUBJECTS_DIR` (from `io`'s
/// environment); the original volume's header is read from `<sd>/<subject>/mri/orig.mgz`, else
/// `orig.mgh`, else the slice folder `orig`. Without `XFM`, `<sd>/<subject>/mri/transforms/<xfmname>`
/// (`talairach.xfm` without a name) is read (`LTAreadEx`; a `LINEAR_VOX_TO_VOX` one made coronal
/// RAS first) and its first matrix copied. Returns `XFM` (given) or the new matrix; `Null` after
/// `ERROR: SUBJECTS_DIR not defined`, `ERROR: cannot find subject <s> in <sd>`, `ERROR: could not
/// read header for <name>` (the slice folder's), `ERROR: could not open <path> for reading ` or
/// `ERROR: reading <path>` on standard output. FS-504, FS-887.
#[allow(clippy::too_many_arguments)]
pub fn devolve_xfm_with_subjects_dir(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    subjid: &str,
    xfm: Option<&mut Matrix>,
    xfmname: Option<&str>,
    sdir: Option<&str>,
    io: &mut dyn LtaIo,
) -> Result<Ret<Matrix>, Fatal> {
    let env = io.env().clone();
    let sd: String = match sdir {
        Some(s) => s.to_owned(),
        None => match env.get("SUBJECTS_DIR") {
            Some(v) => v.to_string_lossy().into_owned(),
            None => {
                let _ = out.write_all(b"ERROR: SUBJECTS_DIR not defined\n");
                return Ok(Ret::Null);
            }
        },
    };
    let subject = path2000(g, err, format!("{sd}/{subjid}"))?;
    if fio_is_directory(g, dir, &subject) == 0 {
        let _ = writeln!(out, "ERROR: cannot find subject {subjid} in {sd}");
        return Ok(Ret::Null);
    }
    let mut name = path2000(g, err, format!("{sd}/{subjid}/mri/orig.mgz"))?;
    let mut orig = try_header(g, out, err, &env, dir, &name, false)?;
    if orig.is_none() {
        name = path2000(g, err, format!("{sd}/{subjid}/mri/orig.mgh"))?;
        orig = try_header(g, out, err, &env, dir, &name, false)?;
        if orig.is_none() {
            name = path2000(g, err, format!("{sd}/{subjid}/mri/orig"))?;
            orig = try_header(g, out, err, &env, dir, &name, true)?;
            if orig.is_none() {
                let _ = writeln!(out, "ERROR: could not read header for {name}");
                return Ok(Ret::Null);
            }
        }
    }
    let orig = orig.expect("the original volume's header");
    let mut fresh = None;
    let given = xfm.is_some();
    let x: &mut Matrix = match xfm {
        Some(x) => x,
        None => {
            let xfmname = xfmname.unwrap_or("talairach.xfm");
            let xfmpath = path2000(g, err, format!("{sd}/{subjid}/mri/transforms/{xfmname}"))?;
            // the file is opened (and kept open) before LTAreadEx reads it
            let fp = match CFile::open(dir, &xfmpath, "r") {
                Ok(f) => f,
                Err(e) => {
                    g.errno = e;
                    let _ = writeln!(out, "ERROR: could not open {xfmpath} for reading ");
                    return Ok(Ret::Null);
                }
            };
            let Some(mut lta) = lta_read_ex(g, out, err, dir, &xfmpath, io)? else {
                let _ = writeln!(out, "ERROR: reading {xfmpath}");
                return Ok(Ret::Null);
            };
            if lta.ltype == LINEAR_VOX_TO_VOX {
                lta_voxel_transform_to_coronal_ras_transform(g, out, err, &mut lta)?;
            }
            if lta.num_xforms <= 0 || lta.xforms.is_empty() {
                outside(
                    g,
                    err,
                    "DevolveXFMWithSubjectsDir",
                    "an array without transforms",
                )?;
                return Ok(Ret::Null);
            }
            let m = match matrix_copy(g, out, err, lta.xforms[0].m_l.as_ref(), None)? {
                Ret::New(m) => m,
                _ => {
                    outside(
                        g,
                        err,
                        "DevolveXFMWithSubjectsDir",
                        "a transform without its matrix",
                    )?;
                    return Ok(Ret::Null);
                }
            };
            fp.close();
            fresh.insert(m)
        }
    };
    let torig_tkreg = xfm_crs2xyz_tkreg(g, out, err, &orig.vg)?;
    let torig_native = xfm_crs2xyz(g, out, err, &orig.vg, 0, false)?;
    let inv = matrix_inverse(g, out, err, Some(&torig_tkreg), None)?.into_new();
    let mfix = matrix_multiply(g, out, err, Some(&torig_native), inv.as_ref(), None)?.into_new();
    let x0 = x.clone();
    // a product of other sizes leaves XFM as it was; FreeSurfer returns it all the same
    matrix_multiply(g, out, err, Some(&x0), mfix.as_ref(), Some(x))?;
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None if given => Ret::Given,
        None => Ret::Null,
    })
}
