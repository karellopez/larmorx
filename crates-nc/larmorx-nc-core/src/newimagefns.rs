// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.17: fsl-newimage 2203.12 (newimagefns.{h,cc}:
// isotropic_resample, affine_transform, raw_affine_transform, affine_transform_mask). Copyright
// (C) 1999-2018 University of Oxford (Mark Jenkinson et al.). Rust translation Copyright 2026
// Karel Lopez Vilaret; modified from the originals. Distributed under the FSL Licence (see
// LICENSE).
//! Resampling volumes: to an isotropic grid (`isotropic_resample`) and through an affine
//! transform (`affine_transform`), as FSL's newimage does them.

use crate::arma::{self, Mat};
use crate::interp::{self, SplineCoefs};
use crate::nifti::XFORM_UNKNOWN;
use crate::volume::{Extrapolation, FslError, Interpolation, NEUROLOGICAL, Volume};

/// `isotropic_resample(aniso, scale)`: the volume sampled every `scale` mm along each axis by its
/// own interpolation and extrapolation (positions accumulated in float steps), with the voxel
/// sizes and sform/qform of the new grid. Prints FSL's warning for a negative scale.
pub fn isotropic_resample(aniso: &Volume, scale: f32, warnings: &mut Vec<String>) -> Volume {
    let mut scale = scale;
    if scale < 0.0 {
        warnings.push("WARNING:: Negative scale in isotropic_resample - using abs value".to_owned());
        scale = scale.abs();
    }
    let mut src = aniso.clone();
    if matches!(
        src.extrap,
        Extrapolation::BoundsAssert | Extrapolation::BoundsException
    ) {
        src.extrap = Extrapolation::Constpad;
    }
    let stepx = scale / aniso.xdim();
    let stepy = scale / aniso.ydim();
    let stepz = scale / aniso.zdim();
    // (int) Max(1.0, (float)size / step): FSL's Max returns the second argument's type.
    let size = |n: i64, step: f32| -> i64 {
        let v = n as f32 / step;
        let m = if 1.0 > f64::from(v) { 1.0f32 } else { v };
        i64::from(crate::cnum::f32_to_i32(m))
    };
    let sz = size(aniso.zsize(), stepz);
    let sy = size(aniso.ysize(), stepy);
    let sx = size(aniso.xsize(), stepx);
    let mut iso = Volume::new(sx, sy, sz, 1);
    let spline = if src.interp == Interpolation::Spline {
        Some(SplineCoefs::new(&src))
    } else {
        None
    };
    let mut fz: f32 = 0.0;
    for z in 0..sz {
        let mut fy: f32 = 0.0;
        for y in 0..sy {
            let mut fx: f32 = 0.0;
            for x in 0..sx {
                let i = iso.idx(x, y, z);
                iso.data[i] = interp::interpolate(&src, fx, fy, fz, spline.as_ref());
                fx += stepx;
            }
            fy += stepy;
        }
        fz += stepz;
    }
    iso.copy_properties(aniso);
    iso.setdims(scale, scale, scale);
    let mut iso2aniso = Mat::zeros(4, 4);
    iso2aniso.set(0, 0, f64::from(stepx));
    iso2aniso.set(1, 1, f64::from(stepy));
    iso2aniso.set(2, 2, f64::from(stepz));
    iso2aniso.set(3, 3, 1.0);
    if aniso.sform_code != XFORM_UNKNOWN {
        iso.sform = arma::mul_tinysq(&aniso.sform, &iso2aniso);
    }
    if aniso.qform_code != XFORM_UNKNOWN {
        iso.qform = arma::mul_tinysq(&aniso.qform, &iso2aniso);
    }
    iso
}

/// `affine_transform(vin, vout, aff, paddingsize)` (with the background set): `vout`'s values
/// become `vin` resampled through `aff` (millimetres to millimetres) by `vin`'s interpolation,
/// with `vin`'s pad value set to its background value and extraslice extrapolation, then
/// masked to the background value beyond `paddingsize` voxels outside `vin`. `vout` keeps its
/// grid; its sform/qform are completed as `raw_affine_transform` does.
pub fn affine_transform(
    vin: &Volume,
    vout: &mut Volume,
    aff: &Mat,
    paddingsize: f32,
) -> Result<(), FslError> {
    let mut src = vin.clone();
    src.padvalue = vin.backgroundval();
    src.extrap = Extrapolation::Extraslice;
    raw_affine_transform(&src, vout, aff)?;
    affine_transform_mask(&src, vout, aff, paddingsize, src.padvalue)
}

/// `raw_affine_transform(vin, vout, aff)`.
pub fn raw_affine_transform(vin: &Volume, vout: &mut Volume, aff: &Mat) -> Result<(), FslError> {
    if vout.nvoxels() <= 0 {
        return Err(FslError::imthrow(
            "Attempted to use affine transform with no voxels in vout",
            8,
        ));
    }
    let mut src = vin.clone();
    if matches!(
        src.extrap,
        Extrapolation::BoundsAssert | Extrapolation::BoundsException
    ) {
        src.extrap = Extrapolation::Constpad;
    }
    let mut iaffbig = arma::inv(aff).map_err(|e| FslError {
        printed: Vec::new(),
        type_name: "std::runtime_error",
        what: e.to_string(),
    })?;
    if src.left_right_order() == NEUROLOGICAL {
        iaffbig = arma::mul_tinysq(&src.swapmat_lr(), &iaffbig);
    }
    if vout.left_right_order() == NEUROLOGICAL {
        iaffbig = arma::mul_tinysq(&iaffbig, &vout.swapmat_lr());
    }
    // vin.sampling_mat().i() * iaffbig * vout.sampling_mat(): solve(Sin, iaffbig * Sout).
    let b = arma::mul_tinysq(&iaffbig, &vout.sampling_mat());
    let iaffbig = arma::solve(&src.sampling_mat(), &b).map_err(|e| FslError {
        printed: Vec::new(),
        type_name: "std::runtime_error",
        what: e.to_string(),
    })?;
    let a: [f32; 12] = std::array::from_fn(|k| iaffbig.at(k / 4, k % 4) as f32);
    let spline = if src.interp == Interpolation::Spline {
        Some(SplineCoefs::new(&src))
    } else {
        None
    };
    for z in 0..vout.zsize() {
        for x in 0..vout.xsize() {
            let (xf, zf) = (x as f32, z as f32);
            let mut o1 = xf * a[0] + zf * a[2] + a[3];
            let mut o2 = xf * a[4] + zf * a[6] + a[7];
            let mut o3 = xf * a[8] + zf * a[10] + a[11];
            for y in 0..vout.ysize() {
                let i = vout.idx(x, y, z);
                vout.data[i] = interp::interpolate(&src, o1, o2, o3, spline.as_ref());
                o1 += a[1];
                o2 += a[5];
                o3 += a[9];
            }
        }
    }
    // The sform and qform of the output.
    if vout.sform_code == XFORM_UNKNOWN && vout.qform_code != XFORM_UNKNOWN {
        vout.sform_code = vout.qform_code;
        vout.sform = vout.qform.clone();
    }
    if vout.qform_code == XFORM_UNKNOWN && vout.sform_code != XFORM_UNKNOWN {
        vout.qform_code = vout.sform_code;
        vout.qform = vout.sform.clone();
    }
    if vout.qform_code == XFORM_UNKNOWN && vout.sform_code == XFORM_UNKNOWN {
        if src.sform_code != XFORM_UNKNOWN {
            let nmat = arma::mul_tinysq(&src.sform, &iaffbig);
            vout.sform_code = src.sform_code;
            vout.sform = nmat.clone();
            vout.qform_code = src.sform_code;
            vout.qform = nmat;
        } else if src.qform_code != XFORM_UNKNOWN {
            let nmat = arma::mul_tinysq(&src.qform, &iaffbig);
            vout.sform_code = src.qform_code;
            vout.sform = nmat.clone();
            vout.qform_code = src.qform_code;
            vout.qform = nmat;
        }
    }
    Ok(())
}

/// `affine_transform_mask(vin, vout, aff, padding, padval)`: output voxels whose pre-image is
/// more than `padding` voxels outside `vin` get `padval`.
pub fn affine_transform_mask(
    vin: &Volume,
    vout: &mut Volume,
    aff: &Mat,
    padding: f32,
    padval: f32,
) -> Result<(), FslError> {
    if vout.nvoxels() <= 0 {
        return Err(FslError::imthrow(
            "Attempted to use affine transform with no voxels in vout",
            8,
        ));
    }
    let a = crate::costfns::voxel_map(vout, vin, aff);
    let xb = (vin.xsize() - 1) as i32;
    let yb = (vin.ysize() - 1) as i32;
    let zb = (vin.zsize() - 1) as i32;
    let (xb0, yb0, zb0) = (-padding, -padding, -padding);
    let xb1 = xb as f32 + padding;
    let yb1 = yb as f32 + padding;
    let zb1 = zb as f32 + padding;
    for z in 0..vout.zsize() {
        for x in 0..vout.xsize() {
            let (xf, zf) = (x as f32, z as f32);
            let mut o1 = xf * a[0] + zf * a[2] + a[3];
            let mut o2 = xf * a[4] + zf * a[6] + a[7];
            let mut o3 = xf * a[8] + zf * a[10] + a[11];
            for y in 0..vout.ysize() {
                let inside = o1 >= xb0 && o2 >= yb0 && o3 >= zb0 && o1 <= xb1 && o2 <= yb1 && o3 <= zb1;
                if !inside {
                    let i = vout.idx(x, y, z);
                    vout.data[i] = padval;
                }
                o1 += a[1];
                o2 += a[5];
                o3 += a[9];
            }
        }
    }
    Ok(())
}
