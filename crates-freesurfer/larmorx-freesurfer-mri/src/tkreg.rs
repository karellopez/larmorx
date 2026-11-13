// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The registration matrices between a geometry's voxels and the coordinates tkregister and FSL
//! use (`utils/mri.cpp` 835-1380, 1437, 3634; `utils/transform.cpp` 567-586): the tkregister
//! vox2ras (`MRIxfmCRS2XYZtkreg`), FSL's (`MRIxfmCRS2XYZfsl`), the conversions between a
//! tkregister registration and the native one (`MRItkReg2Native`, `MRItkRegMtx`), vox2vox and
//! tkregister (`MRItkRegMtxFromVox2Vox`, `MRIvoxToVoxFromTkRegMtx`), the old tkregister's fix
//! (`MRIfixTkReg`), FSL's matrices (`MRIfsl2TkReg`, `MRItkreg2FSL`), `MtxCRS1toCRS0`,
//! `RASFromSurfaceRAS_`, `VGtkreg2RAS`, `VGras2tkreg` and `MRIhfs2Sphinx`.
//!
//! Every product is FreeSurfer's `MatrixMultiply` (in `float`), every inverse its
//! `MatrixInverse`; an input missing where FreeSurfer multiplies it ends the program as
//! `MatrixMultiply` does (`MatrixMultiply(): m1 is null` ...). A product FreeSurfer writes into one
//! of its own inputs is computed from a copy of that input, as `MatrixMultiply` copies it.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_164, FS_165};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_determinant, matrix_inverse, matrix_transpose};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;

use crate::mri::{MRI_INT, MRI_UCHAR, mri_alloc_from_vol_geom, mri_alloc_header};
use crate::vol_geom::{VolGeom, xfm_crs2xyz};

/// A 4 x 4 real matrix of zeros (`MatrixAlloc(4, 4, MATRIX_REAL)`, which cannot fail).
fn zeros44(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write) -> Result<Matrix, Fatal> {
    Ok(matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?.expect("a 4 x 4 matrix"))
}

/// `MatrixMultiply(m1, m2, NULL)`: the new product, or `None` (`NULL`) after its messages.
fn mul(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m1: Option<&Matrix>,
    m2: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    Ok(matrix_multiply(g, out, err, m1, m2, None)?.into_new())
}

/// `MatrixMultiply(m3, m2, m3)` with the result ignored, as FreeSurfer's callers write it: `m3`
/// becomes `m3 * m2` (FreeSurfer copies `m3` first), or keeps its numbers when the sizes do not
/// fit (after `MatrixMultiply`'s messages).
fn mul_left_in_place(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m3: &mut Matrix,
    m2: Option<&Matrix>,
) -> Result<(), Fatal> {
    let m1 = m3.clone();
    matrix_multiply(g, out, err, Some(&m1), m2, Some(m3))?;
    Ok(())
}

/// `m3 = MatrixMultiply(m3, m2, m3)`, the result assigned: `None` when the sizes do not fit; a
/// `None` `m3` ends the program (`m1 is null`).
fn mul_assign(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m3: Option<Matrix>,
    m2: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    let Some(mut m3) = m3 else {
        matrix_multiply(g, out, err, None, m2, None)?;
        return Ok(None);
    };
    let m1 = m3.clone();
    Ok(
        match matrix_multiply(g, out, err, Some(&m1), m2, Some(&mut m3))? {
            Ret::Given => Some(m3),
            _ => None,
        },
    )
}

/// `MatrixInverse(m, NULL)`: the new inverse, or `None` (`NULL`: singular, not square or not
/// finite).
fn inv(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    Ok(matrix_inverse(g, out, err, m, None)?.into_new())
}

/// `MRIxfmCRS2XYZtkreg(mri)`: the tkregister vox2ras of the geometry: [`xfm_crs2xyz`] (base 0) of a
/// volume header of its extent (`MRIallocHeader(width, height, depth, MRI_UCHAR, 1)`, whose
/// messages for an extent that is not positive it prints) with the tkregister direction cosines
/// (columns along -x, rows along -z, slices along +y), the centre at 0 and the geometry's voxel
/// sizes.
pub fn xfm_crs2xyz_tkreg(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Matrix, Fatal> {
    let mut tmp = mri_alloc_header(g, out, err, vg.width, vg.height, vg.depth, MRI_UCHAR, 1)?;
    let t = &mut tmp.vg;
    t.x_r = -1.0;
    t.y_r = 0.0;
    t.z_r = 0.0;
    t.c_r = 0.0;
    t.x_a = 0.0;
    t.y_a = 0.0;
    t.z_a = 1.0;
    t.c_a = 0.0;
    t.x_s = 0.0;
    t.y_s = -1.0;
    t.z_s = 0.0;
    t.c_s = 0.0;
    t.xsize = vg.xsize;
    t.ysize = vg.ysize;
    t.zsize = vg.zsize;
    xfm_crs2xyz(g, out, err, &tmp.vg, 0, false)
}

/// `MRIxfmCRS2XYZfsl(mri)`: FSL's vox2ras, the voxel sizes on the diagonal and 1 at (4, 4).
pub fn xfm_crs2xyz_fsl(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Matrix, Fatal> {
    let mut m = zeros44(g, out, err)?;
    m.set_relt(1, 1, vg.xsize);
    m.set_relt(2, 2, vg.ysize);
    m.set_relt(3, 3, vg.zsize);
    m.set_relt(4, 4, 1.0);
    Ok(m)
}

/// `MRItkReg2Native(ref, mov, R)`: the native (scanner) registration `D = inv(Tmov inv(Kmov) R Kref
/// inv(Tref))` of the tkregister registration `R` (`None`: the identity), `T` the vox2ras and `K`
/// the tkregister vox2ras of each geometry. The products and the final inverse in place are
/// FreeSurfer's; where that inverse fails (the product is singular or its inverse not finite)
/// FreeSurfer returns the product itself, or the inverse's infinities, with no message (FS-164): by
/// default an error that ends the program (`MRItkReg2Native: the registration cannot be inverted`),
/// with `reproduce` FreeSurfer's matrix.
pub fn tkreg2native(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rf: &VolGeom,
    mv: &VolGeom,
    r: Option<&Matrix>,
) -> Result<Matrix, Fatal> {
    let tref = xfm_crs2xyz(g, out, err, rf, 0, false)?;
    let tmov = xfm_crs2xyz(g, out, err, mv, 0, false)?;
    let kref = xfm_crs2xyz_tkreg(g, out, err, rf)?;
    let kmov = xfm_crs2xyz_tkreg(g, out, err, mv)?;
    let inv_kmov = inv(g, out, err, Some(&kmov))?;
    let inv_tref = inv(g, out, err, Some(&tref))?;
    let Some(mut d) = mul(g, out, err, Some(&tmov), inv_kmov.as_ref())? else {
        unreachable!("4 x 4 matrices")
    };
    if let Some(r) = r {
        mul_left_in_place(g, out, err, &mut d, Some(r))?;
    }
    mul_left_in_place(g, out, err, &mut d, Some(&kref))?;
    mul_left_in_place(g, out, err, &mut d, inv_tref.as_ref())?;
    let p = d.clone();
    if matrix_inverse(g, out, err, Some(&p), Some(&mut d))?.is_null() {
        g.notices.add(
            &FS_164,
            "MRItkReg2Native: the native registration's product cannot be inverted (FreeSurfer returns it uninverted, or infinities)",
        );
        if !g.notices.reproduce() {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "MRItkReg2Native: the registration cannot be inverted\n",
            )?;
        }
    }
    Ok(d)
}

/// `MRItkRegMtx(ref, mov, D)`: the tkregister registration `R = Kmov inv(Tmov) inv(D) Tref
/// inv(Kref)` of the native registration `D` (`None`: the identity).
pub fn tkreg_mtx(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rf: &VolGeom,
    mv: &VolGeom,
    d: Option<&Matrix>,
) -> Result<Matrix, Fatal> {
    let tref = xfm_crs2xyz(g, out, err, rf, 0, false)?;
    let tmov = xfm_crs2xyz(g, out, err, mv, 0, false)?;
    let kref = xfm_crs2xyz_tkreg(g, out, err, rf)?;
    let kmov = xfm_crs2xyz_tkreg(g, out, err, mv)?;
    let inv_tmov = inv(g, out, err, Some(&tmov))?;
    let inv_kref = inv(g, out, err, Some(&kref))?;
    let Some(mut r) = mul(g, out, err, Some(&kmov), inv_tmov.as_ref())? else {
        unreachable!("4 x 4 matrices")
    };
    if let Some(d) = d {
        let inv_d = inv(g, out, err, Some(d))?;
        mul_left_in_place(g, out, err, &mut r, inv_d.as_ref())?;
    }
    mul_left_in_place(g, out, err, &mut r, Some(&tref))?;
    mul_left_in_place(g, out, err, &mut r, inv_kref.as_ref())?;
    Ok(r)
}

/// `MRItkRegMtxFromVox2Vox(ref, mov, vox2vox)`: the tkregister registration `Kmov vox2vox inv(Kref)`
/// of a vox2vox from the reference's voxels to the moving volume's; [`tkreg_mtx`] with no
/// registration when `vox2vox` is `None`. `None` when `vox2vox` has another size (after
/// `MatrixMultiply`'s messages; a product of the wrong size is returned as it is).
pub fn tkreg_mtx_from_vox2vox(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rf: &VolGeom,
    mv: &VolGeom,
    vox2vox: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    let Some(v2v) = vox2vox else {
        return tkreg_mtx(g, out, err, rf, mv, None).map(Some);
    };
    let kref = xfm_crs2xyz_tkreg(g, out, err, rf)?;
    let kmov = xfm_crs2xyz_tkreg(g, out, err, mv)?;
    let inv_kref = inv(g, out, err, Some(&kref))?;
    let r = mul(g, out, err, Some(&kmov), Some(v2v))?;
    match r {
        Some(mut r) => {
            mul_left_in_place(g, out, err, &mut r, inv_kref.as_ref())?;
            Ok(Some(r))
        }
        None => {
            // MatrixMultiply(NULL, invKref, NULL): the program ends
            matrix_multiply(g, out, err, None, inv_kref.as_ref(), None)?;
            Ok(None)
        }
    }
}

/// `MRIvoxToVoxFromTkRegMtx(mov, targ, tkR)`: the vox2vox `inv(Kmov) tkR Ktarg` from the target's
/// voxels to the moving volume's (`tkR` `None`: the identity).
pub fn vox_to_vox_from_tkreg_mtx(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mv: &VolGeom,
    targ: &VolGeom,
    tk_r: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    let vox2ras_mov = xfm_crs2xyz_tkreg(g, out, err, mv)?;
    let ras2vox_mov = inv(g, out, err, Some(&vox2ras_mov))?;
    let vox2ras_targ = xfm_crs2xyz_tkreg(g, out, err, targ)?;
    match tk_r {
        Some(t) => {
            let v = mul(g, out, err, ras2vox_mov.as_ref(), Some(t))?;
            match v {
                Some(mut v) => {
                    mul_left_in_place(g, out, err, &mut v, Some(&vox2ras_targ))?;
                    Ok(Some(v))
                }
                None => {
                    matrix_multiply(g, out, err, None, Some(&vox2ras_targ), None)?;
                    Ok(None)
                }
            }
        }
        None => mul(g, out, err, ras2vox_mov.as_ref(), Some(&vox2ras_targ)),
    }
}

/// `MRIfixTkReg(mov, R)`: the registration that, applied with rounding to the nearest voxel, gives
/// the voxels the old tkregister chose (`floor`, `ceil`, `floor` of the column, row and slice):
/// 1000 points drawn in a 256 mm cube with the C library's `drand48` (`g.drand48`, the process's
/// state: x, y and z of each point in turn, `256 * (drand48() - 0.5)` stored as `float`), mapped
/// through `inv(Tmov) R` (`Tmov` the tkregister vox2ras), truncated the old way, and the least
/// squares `Tmov (Pcrs Pxyz') inv(Pxyz Pxyz')`. A registration of another size than 4 x 4 ends the
/// program in a product (`MatrixMultiply`'s messages), or, with four columns and another row count,
/// where FreeSurfer crashes, after `MRIfixTkReg: the registration matrix is not 4 x 4` (FS-165, both
/// modes).
pub fn fix_tkreg(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mv: &VolGeom,
    r: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    const NTEST: i32 = 1000;
    let range = 256.0f32;
    let tmov = xfm_crs2xyz_tkreg(g, out, err, mv)?;
    let inv_tmov = inv(g, out, err, Some(&tmov))?;
    let mut pxyz = matrix_alloc(g, out, err, 4, NTEST, MATRIX_REAL)?.expect("a 4 x 1000 matrix");
    let mut pcrs_tk = matrix_alloc(g, out, err, 4, NTEST, MATRIX_REAL)?.expect("a 4 x 1000 matrix");
    for n in 1..=NTEST {
        for row in 1..=3 {
            let v = f64::from(range) * (g.drand48.drand48() - 0.5);
            pxyz.set_relt(row, n, v as f32);
        }
        pxyz.set_relt(4, n, 1.0);
    }
    let tmp = mul(g, out, err, r, Some(&pxyz))?;
    let Some(pcrs) = mul(g, out, err, inv_tmov.as_ref(), tmp.as_ref())? else {
        // a registration of 4 columns and another row count: FreeSurfer reads the missing
        // product (NULL) and crashes (FS-165, never reproduced)
        g.notices.add(
            &FS_165,
            "MRIfixTkReg of a registration matrix that is not 4 x 4 (FreeSurfer crashes)",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "MRIfixTkReg: the registration matrix is not 4 x 4\n",
        )?;
        return Ok(None);
    };
    for n in 1..=NTEST {
        pcrs_tk.set_relt(1, n, pcrs.relt(1, n).floor());
        pcrs_tk.set_relt(2, n, pcrs.relt(2, n).ceil());
        pcrs_tk.set_relt(3, n, pcrs.relt(3, n).floor());
        pcrs_tk.set_relt(4, n, 1.0);
    }
    let pxyz_t = matrix_transpose(g, out, err, &pxyz, None)?
        .into_new()
        .expect("a transpose");
    let pp = mul(g, out, err, Some(&pxyz), Some(&pxyz_t))?;
    let inv_pp = inv(g, out, err, pp.as_ref())?;
    let Some(tmp) = mul(g, out, err, Some(&pcrs_tk), Some(&pxyz_t))? else {
        unreachable!("4 x 1000 times 1000 x 4")
    };
    // MatrixMultiply(Tmov, tmp, tmp): FreeSurfer copies tmp first
    let mut t2 = tmp.clone();
    matrix_multiply(g, out, err, Some(&tmov), Some(&tmp), Some(&mut t2))?;
    mul(g, out, err, Some(&t2), inv_pp.as_ref())
}

/// The value of `FSLOUTPUTTYPE` for FSL's conversions, or the end of the program after their two
/// lines on standard output (`exit(1)`).
fn fsl_output_type(out: &mut dyn Write, env: &Environment, from_to: &str) -> Result<String, Fatal> {
    match env.get("FSLOUTPUTTYPE") {
        Some(v) => Ok(v.to_string_lossy().into_owned()),
        None => {
            let _ = write!(
                out,
                "ERROR: trying to convert {from_to},\n       but FSLOUTPUTTYPE variable is not set.\n"
            );
            Err(Fatal::Exit(1))
        }
    }
}

fn is_nifti(t: &str) -> bool {
    t == "NIFTI" || t == "NIFTI_PAIR" || t == "NIFTI_GZ"
}

/// The NIFTI part of FSL's conversions: the two vox2ras' determinants printed (`<tag>: mov det =
/// %g, ref det = %g`), and `true` for each one that is positive (after FreeSurfer's two lines).
fn nifti_flips(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rf: &VolGeom,
    mv: &VolGeom,
    tag: &str,
) -> Result<(bool, bool), Fatal> {
    let qref = xfm_crs2xyz(g, out, err, rf, 0, false)?;
    let qmov = xfm_crs2xyz(g, out, err, mv, 0, false)?;
    let dmov = matrix_determinant(g, err, &qmov);
    let dref = matrix_determinant(g, err, &qref);
    let _ = writeln!(
        out,
        "{tag}: mov det = {}, ref det = {}",
        cfmt::g(f64::from(dmov), 6),
        cfmt::g(f64::from(dref), 6)
    );
    let flip_mov = dmov > 0.0;
    if flip_mov {
        let _ = out.write_all(
            b"INFO: FSL2FreeSurfer: Mov volume is NIFTI with positive det,\n      applying LR flip to registration matrix.\n",
        );
    }
    let flip_ref = dref > 0.0;
    if flip_ref {
        let _ = out.write_all(
            b"INFO: FSL2FreeSurfer: Ref volume is NIFTI with positive det,\n      applying LR flip to registration matrix.\n",
        );
    }
    Ok((flip_mov, flip_ref))
}

/// `MRIfsl2TkReg(ref, mov, FSLRegMat)`: the tkregister registration `Tmov inv(Dmov) inv(Mfsl) Dref
/// inv(Tref)` of an FSL registration (`T` the tkregister vox2ras, `D` the voxel sizes' diagonal).
/// `FSLOUTPUTTYPE` (`env`) must be set (else FreeSurfer's two lines and `exit(1)`); with a NIFTI
/// type the determinants of both vox2ras are printed and a positive one flips that volume left
/// to right (FreeSurfer's lines; `inv(Dmov)`'s first element negated and `width - 1` in its
/// translation, `Dref`'s first element negated and `xsize (width - 1)`). `None` where a product's
/// sizes do not fit.
pub fn fsl2tkreg(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    rf: &VolGeom,
    mv: &VolGeom,
    fsl: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    let mut inv_dmov = zeros44(g, out, err)?;
    // 1.0 / size in double, stored as float: the same number as float's division (GCC's divss)
    inv_dmov.set_relt(1, 1, sse::div(1.0, mv.xsize));
    inv_dmov.set_relt(2, 2, sse::div(1.0, mv.ysize));
    inv_dmov.set_relt(3, 3, sse::div(1.0, mv.zsize));
    inv_dmov.set_relt(4, 4, 1.0);
    let mut dref = zeros44(g, out, err)?;
    dref.set_relt(1, 1, rf.xsize);
    dref.set_relt(2, 2, rf.ysize);
    dref.set_relt(3, 3, rf.zsize);
    dref.set_relt(4, 4, 1.0);
    let t = fsl_output_type(out, env, "FSL registration matrix to FreeSurfer")?;
    let _ = writeln!(out, "FSLOUTPUTTYPE {t} ");
    if is_nifti(&t) {
        let (flip_mov, flip_ref) = nifti_flips(g, out, err, rf, mv, "fsl2TkReg")?;
        if flip_mov {
            let v = -inv_dmov.relt(1, 1);
            inv_dmov.set_relt(1, 1, v);
            inv_dmov.set_relt(1, 4, mv.width.wrapping_sub(1) as f32);
        }
        if flip_ref {
            let v = -dref.relt(1, 1);
            dref.set_relt(1, 1, v);
            dref.set_relt(1, 4, sse::mul(rf.width.wrapping_sub(1) as f32, rf.xsize));
        }
    }
    let tmov = xfm_crs2xyz_tkreg(g, out, err, mv)?;
    let tref = xfm_crs2xyz_tkreg(g, out, err, rf)?;
    let inv_tref = inv(g, out, err, Some(&tref))?;
    let inv_fsl = inv(g, out, err, fsl)?;
    let reg = mul(g, out, err, Some(&tmov), Some(&inv_dmov))?;
    let reg = mul_assign(g, out, err, reg, inv_fsl.as_ref())?;
    let reg = mul_assign(g, out, err, reg, Some(&dref))?;
    mul_assign(g, out, err, reg, inv_tref.as_ref())
}

/// `MRItkreg2FSL(ref, mov, tkRegMat)`: the FSL registration `inv(Dmov inv(Tmov) R Tref inv(Dref))`
/// of a tkregister registration, with [`fsl2tkreg`]'s `FSLOUTPUTTYPE` rule and flips (`Dmov`'s and
/// `Dref`'s first element negated and `xsize (width - 1)` in their translation). `None` when the
/// product cannot be inverted (FreeSurfer's `NULL`) or a product's sizes do not fit.
pub fn tkreg2fsl(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    rf: &VolGeom,
    mv: &VolGeom,
    tk: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    let mut dmov = zeros44(g, out, err)?;
    dmov.set_relt(1, 1, mv.xsize);
    dmov.set_relt(2, 2, mv.ysize);
    dmov.set_relt(3, 3, mv.zsize);
    dmov.set_relt(4, 4, 1.0);
    let mut dref = zeros44(g, out, err)?;
    dref.set_relt(1, 1, rf.xsize);
    dref.set_relt(2, 2, rf.ysize);
    dref.set_relt(3, 3, rf.zsize);
    dref.set_relt(4, 4, 1.0);
    let t = fsl_output_type(out, env, "FreeSurfer registration matrix to FSL")?;
    let _ = writeln!(out, "FSLOUTPUTTYPE {t} ");
    if is_nifti(&t) {
        let (flip_mov, flip_ref) = nifti_flips(g, out, err, rf, mv, "tkreg2FSL")?;
        if flip_mov {
            let v = -dmov.relt(1, 1);
            dmov.set_relt(1, 1, v);
            dmov.set_relt(1, 4, sse::mul(mv.width.wrapping_sub(1) as f32, mv.xsize));
        }
        if flip_ref {
            let v = -dref.relt(1, 1);
            dref.set_relt(1, 1, v);
            dref.set_relt(1, 4, sse::mul(rf.width.wrapping_sub(1) as f32, rf.xsize));
        }
    }
    let inv_dref = inv(g, out, err, Some(&dref))?;
    let tmov = xfm_crs2xyz_tkreg(g, out, err, mv)?;
    let inv_tmov = inv(g, out, err, Some(&tmov))?;
    let tref = xfm_crs2xyz_tkreg(g, out, err, rf)?;
    let m = mul(g, out, err, Some(&dmov), inv_tmov.as_ref())?;
    let m = mul_assign(g, out, err, m, tk)?;
    let m = mul_assign(g, out, err, m, Some(&tref))?;
    let m = mul_assign(g, out, err, m, inv_dref.as_ref())?;
    // FSLRegMat = MatrixInverse(FSLRegMat, FSLRegMat)
    let Some(mut m) = m else {
        matrix_inverse(g, out, err, None, None)?;
        return Ok(None);
    };
    let p = m.clone();
    Ok(match matrix_inverse(g, out, err, Some(&p), Some(&mut m))? {
        Ret::Given => Some(m),
        _ => None,
    })
}

/// `MtxCRS1toCRS0(Q)`: the matrix that turns one-based voxel indices into zero-based ones (the
/// identity with 1 in the last column's first three rows), into `q` (a new matrix when `None`);
/// `q` not 4 x 4: `ERROR: MtxCRS1toCRS0(): input matrix is not 4x4` on standard output and `Null`.
pub fn mtx_crs1_to_crs0(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    q: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let mut fresh = None;
    let q = match q {
        None => fresh.insert(zeros44(g, out, err)?),
        Some(q) => {
            if q.rows != 4 || q.cols != 4 {
                let _ = out.write_all(b"ERROR: MtxCRS1toCRS0(): input matrix is not 4x4\n");
                return Ok(Ret::Null);
            }
            q
        }
    };
    for r in 1..=4 {
        for c in 1..=4 {
            q.set_relt(r, c, if r == c || c == 4 { 1.0 } else { 0.0 });
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `RASFromSurfaceRAS_(mri, RASFromSRAS)`: the matrix from tkregister ("surface") RAS to scanner
/// RAS, `vox2ras inv(tkregister vox2ras)` (into `m` when given). Where the tkregister vox2ras
/// cannot be inverted (a voxel size of 0) the program ends in the product (`MatrixMultiply(): m2
/// is null`).
pub fn ras_from_surface_ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
    m: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let vox2ras = xfm_crs2xyz(g, out, err, vg, 0, false)?;
    let vox2tk = xfm_crs2xyz_tkreg(g, out, err, vg)?;
    match m {
        Some(m) => {
            if matrix_inverse(g, out, err, Some(&vox2tk), Some(m))?.is_null() {
                matrix_multiply(g, out, err, Some(&vox2ras), None, None)?;
                return Ok(Ret::Null);
            }
            let c = m.clone();
            matrix_multiply(g, out, err, Some(&vox2ras), Some(&c), Some(m))
        }
        None => {
            let Some(mut i) = inv(g, out, err, Some(&vox2tk))? else {
                matrix_multiply(g, out, err, Some(&vox2ras), None, None)?;
                return Ok(Ret::Null);
            };
            let c = i.clone();
            Ok(
                match matrix_multiply(g, out, err, Some(&vox2ras), Some(&c), Some(&mut i))? {
                    Ret::Given => Ret::New(i),
                    _ => Ret::Null,
                },
            )
        }
    }
}

/// `VGtkreg2RAS(vg, tkreg2ras)`: [`ras_from_surface_ras`] of a volume header made from the geometry
/// (`MRIallocFromVolGeom(vg, MRI_INT, 1, 1)`: its extent, voxel sizes, direction cosines, centre;
/// not its shears).
pub fn vg_tkreg2ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
    m: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let mri = mri_alloc_from_vol_geom(g, out, err, vg, MRI_INT, 1, true)?;
    ras_from_surface_ras(g, out, err, &mri.vg, m)
}

/// `VGras2tkreg(vg, ras2tkreg)`: the inverse of [`vg_tkreg2ras`], a new matrix (`None` when it
/// cannot be inverted). FreeSurfer ignores its second argument (no caller passes one), so it has
/// none here.
pub fn vg_ras2tkreg(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Option<Matrix>, Fatal> {
    let m = vg_tkreg2ras(g, out, err, vg, None)?.into_new();
    inv(g, out, err, m.as_ref())
}

/// `MRIhfs2Sphinx(mri)`: the geometry of a subject scanned in the sphinx position: the R
/// components of the direction cosines and centre negated, the A and S components swapped; 0.
pub fn hfs2sphinx(vg: &mut VolGeom) -> i32 {
    vg.x_r = -vg.x_r;
    vg.y_r = -vg.y_r;
    vg.z_r = -vg.z_r;
    vg.c_r = -vg.c_r;
    std::mem::swap(&mut vg.x_a, &mut vg.x_s);
    std::mem::swap(&mut vg.y_a, &mut vg.y_s);
    std::mem::swap(&mut vg.z_a, &mut vg.z_s);
    std::mem::swap(&mut vg.c_a, &mut vg.c_s);
    0
}

/// `VOL_GEOM::get_Vox2RAS(base, useshear)`: [`xfm_crs2xyz`].
pub fn get_vox2ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
    base: i32,
    useshear: bool,
) -> Result<Matrix, Fatal> {
    xfm_crs2xyz(g, out, err, vg, base, useshear)
}

/// `VOL_GEOM::get_RAS2Vox(base, useshear)`: the inverse of [`get_vox2ras`] (`None` when it cannot
/// be inverted).
pub fn get_ras2vox(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
    base: i32,
    useshear: bool,
) -> Result<Option<Matrix>, Fatal> {
    let m = xfm_crs2xyz(g, out, err, vg, base, useshear)?;
    inv(g, out, err, Some(&m))
}

/// `VOL_GEOM::get_Vox2TkregRAS()`: [`xfm_crs2xyz_tkreg`].
pub fn get_vox2tkreg_ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Matrix, Fatal> {
    xfm_crs2xyz_tkreg(g, out, err, vg)
}

/// `VOL_GEOM::get_TkregRAS2Vox()`: the inverse of [`xfm_crs2xyz_tkreg`].
pub fn get_tkreg_ras2vox(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Option<Matrix>, Fatal> {
    let m = xfm_crs2xyz_tkreg(g, out, err, vg)?;
    inv(g, out, err, Some(&m))
}

/// `VOL_GEOM::get_RAS2TkregRAS()`: [`vg_ras2tkreg`].
pub fn get_ras2tkreg_ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Option<Matrix>, Fatal> {
    vg_ras2tkreg(g, out, err, vg)
}

/// `VOL_GEOM::get_TkregRAS2RAS()`: [`vg_tkreg2ras`], a new matrix.
pub fn get_tkreg_ras2ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Option<Matrix>, Fatal> {
    Ok(vg_tkreg2ras(g, out, err, vg, None)?.into_new())
}
