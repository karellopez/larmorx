// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Three functions of FreeSurfer 8.2.0's `utils/transform.cpp` that `mri_vol2vol` reaches (queue
//! item 3.2-38, part 2; track A relay 54): `LTA::get_matrix` (66-148, with the `VOL_GEOM`
//! matrices of `include/mri.h` 213-218), `SegRegCost` (4664-4742) and `MRIaffineDisplacment`
//! (4750-4795).

use std::io::Write;

use larmorx_core::defects::FS_1185;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{
    MATRIX_REAL, Matrix, matrix_alloc, matrix_copy, matrix_multiply, matrix_multiply_d,
};
use larmorx_freesurfer_core::matrix_ops::{matrix_inverse, matrix_print};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sqrt64, sub, sub64};
use larmorx_freesurfer_core::utils::sum2stddev;
use larmorx_freesurfer_mri::lta::{
    LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, Lta, REGISTER_DAT, lta_copy,
};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_UCHAR, Mri, mri_alloc_sequence, mri_copy_header,
};
use larmorx_freesurfer_mri::tkreg::{vg_ras2tkreg, vg_tkreg2ras, xfm_crs2xyz_tkreg};
use larmorx_freesurfer_mri::vol_geom::{VolGeom, xfm_crs2xyz};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::ltaops::lta_change_type;

/// `FS_COORDS_TKREG_RAS`, `FS_COORDS_SCANNER_RAS`, `FS_COORDS_VOXEL` (`include/transform.h`).
pub const FS_COORDS_TKREG_RAS: i32 = 1;
pub const FS_COORDS_SCANNER_RAS: i32 = 2;
pub const FS_COORDS_VOXEL: i32 = 3;

fn inverse(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    Ok(matrix_inverse(g, out, err, m, None)?.into_new())
}

fn product_d(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    a: Option<&Matrix>,
    b: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    Ok(matrix_multiply_d(g, out, err, a, b, None)?.into_new())
}

/// `MatrixPrint(stdout, m)` (`% 2.5f`).
fn print(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, m: Option<&Matrix>) {
    let mut buf = Vec::new();
    matrix_print(g, Some(&mut buf), &mut std::io::sink(), err, m, 5);
    let _ = out.write_all(&buf);
}

/// `vg.get_Vox2RAS()`: `MRIxfmCRS2XYZ(vg, 0)` (no shear).
fn vox2ras(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
) -> Result<Matrix, Fatal> {
    xfm_crs2xyz(g, out, err, vg, 0, false)
}

/// `lta->get_matrix(InputCoords, OutputCoords, NULL)`: the matrix that takes a point of the given
/// kind of input coordinates (`FS_COORDS_TKREG_RAS` 1, `FS_COORDS_SCANNER_RAS` 2, `FS_COORDS_VOXEL`
/// 3; of the transform's source) to the given kind of output coordinates (of its destination),
/// from a copy of the array (the array itself is not changed): the copy changed to
/// `LINEAR_RAS_TO_RAS` (scanner to anything but voxels of the destination, voxels to scanner,
/// tkregister to scanner), `LINEAR_VOX_TO_VOX` (voxels to voxels or tkregister, tkregister to
/// voxels) or `REGISTER_DAT` (tkregister to tkregister: its matrix inverted), and its matrix
/// multiplied (`MatrixMultiplyD`) by the destination's RAS-to-voxel (scanner to voxels: both
/// matrices then printed, the RAS-to-voxel first), RAS-to-tkregister or voxel-to-tkregister
/// matrix on the left, or the source's voxel-to-RAS, tkregister-to-RAS or tkregister-to-voxel
/// matrix on the right. More than one transform: `ERROR: LTA::GetMatrix: num_xforms > 1` and
/// `None`; another kind: `ERROR: LTA::GetMatrix: intype=<i>, outtype=<o> not recognized` and
/// `None`.
#[allow(clippy::too_many_arguments)]
pub fn lta_get_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    lta: &Lta,
    incoords: i32,
    outcoords: i32,
) -> Result<Option<Matrix>, Fatal> {
    let Some(mut l) = lta_copy(g, out, err, lta, None)?.into_new() else {
        let _ = writeln!(out, "ERROR: LTA::GetMatrix: LTA is NULL");
        return Ok(None);
    };
    if l.num_xforms > 1 {
        let _ = writeln!(out, "ERROR: LTA::GetMatrix: num_xforms > 1");
        return Ok(None);
    }
    use FS_COORDS_SCANNER_RAS as SCANNER;
    use FS_COORDS_TKREG_RAS as TKREG;
    use FS_COORDS_VOXEL as VOXEL;
    let ty = match (incoords, outcoords) {
        (SCANNER, SCANNER) | (SCANNER, VOXEL) | (SCANNER, TKREG) => LINEAR_RAS_TO_RAS,
        (VOXEL, SCANNER) | (TKREG, SCANNER) => LINEAR_RAS_TO_RAS,
        (VOXEL, VOXEL) | (VOXEL, TKREG) | (TKREG, VOXEL) => LINEAR_VOX_TO_VOX,
        (TKREG, TKREG) => REGISTER_DAT,
        _ => {
            let _ = writeln!(
                out,
                "ERROR: LTA::GetMatrix: intype={incoords}, outtype={outcoords} not recognized"
            );
            return Ok(None);
        }
    };
    lta_change_type(g, out, err, env, &mut l, ty)?;
    let lt = &l.xforms[0];
    let m_l = lt.m_l.as_ref();
    let m = match (incoords, outcoords) {
        (SCANNER, VOXEL) => {
            let v2r = vox2ras(g, out, err, &lt.dst)?;
            let r2v = inverse(g, out, err, Some(&v2r))?;
            let m = product_d(g, out, err, r2v.as_ref(), m_l)?;
            print(g, out, err, r2v.as_ref());
            print(g, out, err, m_l);
            m
        }
        (SCANNER, TKREG) => {
            let r2t = vg_ras2tkreg(g, out, err, &lt.dst)?;
            product_d(g, out, err, r2t.as_ref(), m_l)?
        }
        (VOXEL, SCANNER) => {
            let v2r = vox2ras(g, out, err, &lt.src)?;
            product_d(g, out, err, m_l, Some(&v2r))?
        }
        (VOXEL, TKREG) => {
            let v2t = xfm_crs2xyz_tkreg(g, out, err, &lt.dst)?;
            product_d(g, out, err, Some(&v2t), m_l)?
        }
        (TKREG, SCANNER) => {
            let t2r = vg_tkreg2ras(g, out, err, &lt.src, None)?.into_new();
            product_d(g, out, err, m_l, t2r.as_ref())?
        }
        (TKREG, VOXEL) => {
            let v2t = xfm_crs2xyz_tkreg(g, out, err, &lt.src)?;
            let t2v = inverse(g, out, err, Some(&v2t))?;
            product_d(g, out, err, m_l, t2v.as_ref())?
        }
        (TKREG, TKREG) => inverse(g, out, err, m_l)?,
        // scanner to scanner, voxels to voxels: the matrix itself
        _ => matrix_copy(g, out, err, m_l, None)?.into_new(),
    };
    Ok(m)
}

/// The eight numbers of `SegRegCost`: the white matter's count, mean and standard deviation, the
/// cortex's, `t` and the cost `1 / t`.
pub type SegRegCosts = [f64; 8];

/// `SegRegCost(regseg, f, costs)`: the contrast of white matter and cortex in the intensities `f`
/// (frame 0) over the segmentation `regseg` (frame 0): each voxel of `f` that is not 0 (columns
/// slowest, then rows, then slices) whose label is 2 or 41 adds its value to the white matter's
/// sums (value and square, in `double`), 3 or 42 to the cortex's; the means (sum / count), the
/// standard deviations (`sum2stddev`), `t = |ctxmean - wmmean| / sqrt(ctxstd^2 + wmstd^2)` and
/// `1 / t` (a class without voxels gives `0 / 0`'s NaN, SSE2's rules). `regseg` must be `int`,
/// `uchar` or `float` (`ERROR: SegRegCost(): regseg type must be int, uchar, or float`), `f`
/// `float` (`ERROR: SegRegCost(): f type must be float`): `false` and `costs` unchanged.
/// FS-1185: FreeSurfer reads `regseg` at each voxel of `f` without checking the two have the
/// same size: a smaller segmentation is read past its rows and slices (memory it does not own);
/// by default an error (`ERROR: SegRegCost(): regseg and f differ in size`, `false`), with
/// `reproduce` the voxels past it read as 0 (a notice).
pub fn seg_reg_cost(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    regseg: &Mri,
    f: &Mri,
    costs: &mut SegRegCosts,
) -> Result<bool, Fatal> {
    if regseg.mtype != MRI_INT && regseg.mtype != MRI_UCHAR && regseg.mtype != MRI_FLOAT {
        let _ = writeln!(
            out,
            "ERROR: SegRegCost(): regseg type must be int, uchar, or float"
        );
        return Ok(false);
    }
    if f.mtype != MRI_FLOAT {
        let _ = writeln!(out, "ERROR: SegRegCost(): f type must be float");
        return Ok(false);
    }
    let (w, h, d) = (f.vg.width, f.vg.height, f.vg.depth);
    let (sw, sh, sd) = (regseg.vg.width, regseg.vg.height, regseg.vg.depth);
    let smaller = sw < w || sh < h || sd < d;
    if smaller {
        g.notices.add(
            &FS_1185,
            format!(
                "SegRegCost: the segmentation ({sw} x {sh} x {sd}) is smaller than the volume ({w} x {h} x {d}) (FreeSurfer reads memory it does not own)"
            ),
        );
        if !g.notices.reproduce() {
            let _ = writeln!(out, "ERROR: SegRegCost(): regseg and f differ in size");
            return Ok(false);
        }
    }
    let (mut nwm, mut nctx) = (0i32, 0i32);
    let (mut wmsum, mut wmsum2, mut ctxsum, mut ctxsum2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                let vf = f64::from(mri_get_vox_val(g, err, f, c, r, s, 0, 0)?);
                if vf == 0.0 {
                    continue;
                }
                let vseg = if c < sw && r < sh && s < sd {
                    f64::from(mri_get_vox_val(g, err, regseg, c, r, s, 0, 0)?)
                } else {
                    0.0
                };
                if vseg == 2.0 || vseg == 41.0 {
                    wmsum = add64(wmsum, vf);
                    wmsum2 = add64(wmsum2, mul64(vf, vf));
                    nwm += 1;
                }
                if vseg == 3.0 || vseg == 42.0 {
                    ctxsum = add64(ctxsum, vf);
                    ctxsum2 = add64(ctxsum2, mul64(vf, vf));
                    nctx += 1;
                }
            }
        }
    }
    let wmmean = div64(wmsum, f64::from(nwm));
    let wmstd = sum2stddev(wmsum, wmsum2, nwm);
    let ctxmean = div64(ctxsum, f64::from(nctx));
    let ctxstd = sum2stddev(ctxsum, ctxsum2, nctx);
    let t = div64(
        sub64(ctxmean, wmmean).abs(),
        sqrt64(add64(mul64(ctxstd, ctxstd), mul64(wmstd, wmstd))),
    );
    let cost = div64(1.0, t);
    *costs = [
        f64::from(nwm),
        wmmean,
        wmstd,
        f64::from(nctx),
        ctxmean,
        ctxstd,
        t,
        cost,
    ];
    Ok(true)
}

/// `MRIaffineDisplacment(mri, R)`: a one-frame `float` volume of `mri`'s extent and header
/// (`MRIcopyHeader`) holding at each voxel the distance (`sqrt` of the squared differences, in
/// `double`) between its tkregister RAS point and that point moved by the tkregister matrix `R`
/// (`MatrixMultiply` of `R` and the voxel-to-tkregister matrix, each point `MatrixMultiply`'s,
/// in `float`; each coordinate's difference in `float`, then squared and summed in `double`): at
/// voxel `v`, `|T v - R T v|` with `T` the voxel-to-tkregister matrix.
pub fn mri_affine_displacement(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    r: &Matrix,
) -> Result<Mri, Fatal> {
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let mut disp = mri_alloc_sequence(g, out, err, w, h, d, MRI_FLOAT, 1)?;
    mri_copy_header(g, out, err, mri, Some(&mut disp))?;
    let mut pcrs = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    pcrs.set_relt(4, 1, 1.0);
    let mut pras = Matrix::real(4, 1);
    let mut pras2 = Matrix::real(4, 1);
    let v2r = xfm_crs2xyz_tkreg(g, out, err, &disp.vg)?;
    let v2r2 = matrix_multiply(g, out, err, Some(r), Some(&v2r), None)?
        .into_new()
        .expect("a 4 x 4 product");
    for c in 0..w {
        for rr in 0..h {
            for s in 0..d {
                pcrs.set_relt(1, 1, c as f32);
                pcrs.set_relt(2, 1, rr as f32);
                pcrs.set_relt(3, 1, s as f32);
                matrix_multiply(g, out, err, Some(&v2r), Some(&pcrs), Some(&mut pras))?;
                matrix_multiply(g, out, err, Some(&v2r2), Some(&pcrs), Some(&mut pras2))?;
                // float - float, then widened
                let dx = f64::from(sub(pras.relt(1, 1), pras2.relt(1, 1)));
                let dy = f64::from(sub(pras.relt(2, 1), pras2.relt(2, 1)));
                let dz = f64::from(sub(pras.relt(3, 1), pras2.relt(3, 1)));
                let dd = sqrt64(add64(add64(mul64(dx, dx), mul64(dy, dy)), mul64(dz, dz)));
                mri_set_vox_val(g, err, &mut disp, c, rr, s, 0, dd as f32, 0)?;
            }
        }
    }
    Ok(disp)
}
