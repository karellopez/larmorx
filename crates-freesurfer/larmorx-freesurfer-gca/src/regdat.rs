// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/transform.cpp`'s `TransformLTA2RegDat` (4460-4537): the tkregister matrix of a linear
//! transform array's first transform (track D's relay 49 of FreeSurfer's foundations, for
//! `regio_read_registermat` of an `.lta`: `mri_segstats --reg`).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_inverse, matrix_print};
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, Lta, REGISTER_DAT};
use larmorx_freesurfer_mri::vol_geom::vgprint;
use larmorx_freesurfer_volume::vgeom::{tkr_vox2ras_from_vol_geom, vg_i_to_r};

/// `MatrixMultiply(a, b, NULL)`, `None` for a `NULL` operand or result.
fn mul(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    a: Option<&Matrix>,
    b: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    Ok(matrix_multiply(g, out, err, a, b, None)?.into_new())
}

/// `MatrixInverse(m, NULL)`.
fn inv(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    Ok(matrix_inverse(g, out, err, m, None)?.into_new())
}

/// `TransformLTA2RegDat(lta)`: for type `REGISTER_DAT` a copy of the first matrix; for
/// `LINEAR_VOX_TO_VOX` and `LINEAR_RAS_TO_RAS` (the latter made voxel-to-voxel with the
/// geometries' scanner matrices, `inv(Starg) M Smov`) `R = Tmov inv(Vox2Vox) inv(Ttarg)` with
/// the tkregister matrices of the source and destination geometries; with `Gdiag_no > 0` the
/// steps printed. Another type: `ERROR: TransformLTA2RegDat(): type = <t>, must be 1 or 0 or 14`
/// and `None`.
pub fn transform_lta2_reg_dat(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lta: &mut Lta,
) -> Result<Option<Matrix>, Fatal> {
    if lta.ltype != LINEAR_RAS_TO_RAS && lta.ltype != LINEAR_VOX_TO_VOX && lta.ltype != REGISTER_DAT
    {
        let _ = writeln!(
            out,
            "ERROR: TransformLTA2RegDat(): type = {}, must be {LINEAR_RAS_TO_RAS} or {LINEAR_VOX_TO_VOX} or {REGISTER_DAT}",
            lta.ltype
        );
        return Ok(None);
    }
    let x = &lta.xforms[0];
    if lta.ltype == REGISTER_DAT {
        return Ok(matrix_copy(g, out, err, x.m_l.as_ref(), None)?.into_new());
    }
    let vox2vox = if lta.ltype == LINEAR_VOX_TO_VOX {
        matrix_copy(g, out, err, x.m_l.as_ref(), None)?.into_new()
    } else {
        let m = matrix_copy(g, out, err, x.m_l.as_ref(), None)?.into_new();
        let starg = vg_i_to_r(g, out, err, &x.dst)?;
        let smov = vg_i_to_r(g, out, err, &x.src)?;
        let inv_starg = inv(g, out, err, Some(&starg))?;
        let v = mul(g, out, err, inv_starg.as_ref(), m.as_ref())?;
        mul(g, out, err, v.as_ref(), Some(&smov))?
    };
    let inv_vox2vox = inv(g, out, err, vox2vox.as_ref())?;
    let ttarg = tkr_vox2ras_from_vol_geom(g, out, err, &x.dst)?;
    let tmov = tkr_vox2ras_from_vol_geom(g, out, err, &x.src)?;
    let inv_ttarg = inv(g, out, err, Some(&ttarg))?;
    let r1 = mul(g, out, err, Some(&tmov), inv_vox2vox.as_ref())?;
    let r = mul(g, out, err, r1.as_ref(), inv_ttarg.as_ref())?;
    if g.gdiag_no > 0 {
        let _ = write!(out, "TransformLTA2RegDat() -----------");
        let _ = write!(out, "src/targ Vol Geom");
        vgprint(out, &x.src, false);
        let _ = write!(out, "dst/mov  Vol Geom");
        vgprint(out, &x.dst, false);
        let _ = writeln!(out, "Vox2Vox---------------------------");
        print(g, out, err, vox2vox.as_ref());
        let _ = writeln!(out, "Tmov ---------------------------");
        print(g, out, err, Some(&tmov));
        let _ = writeln!(out, "invTtarg ---------------------------");
        print(g, out, err, inv_ttarg.as_ref());
        let _ = writeln!(out, "---------------------------");
        print(g, out, err, r.as_ref());
        let _ = writeln!(out, "---------------------------");
    }
    Ok(r)
}

/// `MatrixPrint(stdout, m)`.
fn print(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, m: Option<&Matrix>) {
    let mut text = Vec::new();
    matrix_print(g, Some(&mut text), out, err, m, 5);
    let _ = out.write_all(&text);
}
