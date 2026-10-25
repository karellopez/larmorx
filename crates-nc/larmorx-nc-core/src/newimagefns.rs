// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.17: fsl-newimage 2203.12 (newimagefns.{h,cc}:
// isotropic_resample, affine_transform, raw_affine_transform, affine_transform_mask,
// make_grad_masks, gradient; smooth, convolve_separable, gaussian_kernel1D). Copyright
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
        warnings
            .push("WARNING:: Negative scale in isotropic_resample - using abs value".to_owned());
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
                let inside =
                    o1 >= xb0 && o2 >= yb0 && o3 >= zb0 && o1 <= xb1 && o2 <= yb1 && o3 <= zb1;
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

/// `make_grad_masks(maskx, masky, maskz)`: the 3x3x3 gradient masks,
/// `(x-1)*pow(3, 1-|y-1|-|z-1|)` and its permutations (the exponent is 1, 0 or -1, so `pow` is
/// exact or correctly rounded on every libm), stored in single precision; index `x + 3*(y + 3*z)`.
pub fn grad_masks() -> [[f32; 27]; 3] {
    let mut m = [[0.0f32; 27]; 3];
    let p3 = |e: f64| -> f64 {
        if e == 1.0 {
            3.0
        } else if e == 0.0 {
            1.0
        } else {
            1.0 / 3.0
        }
    };
    for z in 0..3 {
        for y in 0..3 {
            for x in 0..3 {
                let (xf, yf, zf) = (f64::from(x), f64::from(y), f64::from(z));
                let i = (x + 3 * (y + 3 * z)) as usize;
                m[0][i] = ((xf - 1.0) * p3(1.0 - (yf - 1.0).abs() - (zf - 1.0).abs())) as f32;
                m[1][i] = ((yf - 1.0) * p3(1.0 - (xf - 1.0).abs() - (zf - 1.0).abs())) as f32;
                m[2][i] = ((zf - 1.0) * p3(1.0 - (xf - 1.0).abs() - (yf - 1.0).abs())) as f32;
            }
        }
    }
    m
}

/// `gradient(source)` (voxel coordinates): the magnitude of the 3x3x3 gradient, the neighbours
/// outside the volume taken by its extrapolation, accumulated in single precision in FSL's
/// order; the result keeps `source`'s properties.
pub fn gradient(source: &Volume) -> Volume {
    let m = grad_masks();
    let mut grad = source.clone();
    for z in 0..source.zsize() {
        for y in 0..source.ysize() {
            for x in 0..source.xsize() {
                let (mut vx, mut vy, mut vz) = (0.0f32, 0.0f32, 0.0f32);
                for mz in -1i64..=1 {
                    for my in -1i64..=1 {
                        for mx in -1i64..=1 {
                            let v = source.get(x + mx, y + my, z + mz);
                            let i = ((mx + 1) + 3 * ((my + 1) + 3 * (mz + 1))) as usize;
                            vx += v * m[0][i];
                            vy += v * m[1][i];
                            vz += v * m[2][i];
                        }
                    }
                }
                let k = grad.idx(x, y, z);
                // sqrt(float) and sqrt(double) rounded to float agree (no double rounding for
                // the square root).
                grad.data[k] = (vx * vx + vy * vy + vz * vz).sqrt();
                if grad.data[k].is_nan() {
                    // NaN or infinite voxels: the same sums with SSE's NaNs.
                    use crate::sse::{add_f32 as add, mul_f32 as mul};
                    let (mut vx, mut vy, mut vz) = (0.0f32, 0.0f32, 0.0f32);
                    for mz in -1i64..=1 {
                        for my in -1i64..=1 {
                            for mx in -1i64..=1 {
                                let v = source.get(x + mx, y + my, z + mz);
                                let i = ((mx + 1) + 3 * ((my + 1) + 3 * (mz + 1))) as usize;
                                vx = add(vx, mul(v, m[0][i]));
                                vy = add(vy, mul(v, m[1][i]));
                                vz = add(vz, mul(v, m[2][i]));
                            }
                        }
                    }
                    let s = add(add(mul(vx, vx), mul(vy, vy)), mul(vz, vz));
                    grad.data[k] = crate::sse::sqrt_f32(s);
                }
            }
        }
    }
    grad
}

// ------------------------------------------------------------------------------------------------
// Smoothing (newimagefns.h: smooth, convolve_separable; newimagefns.cc: gaussian_kernel1D)

/// `gaussian_kernel1D(sigma, radius)`: `2 radius + 1` values `exp(-j^2 / (2 sigma^2))` (glibc's
/// `exp`) rounded to `float` and summed in `float`, then scaled in `double` by the inverse of the
/// sum; a sigma of at most 1e-6 gives the unit impulse.
pub fn gaussian_kernel_1d(sigma: f32, radius: i32) -> Vec<f64> {
    let mut kern = Vec::with_capacity((2 * i64::from(radius) + 1).max(0) as usize);
    let mut sum = 0f32;
    for j in -radius..=radius {
        let val: f32 = if f64::from(sigma) > 1e-6 {
            let s = f64::from(sigma);
            crate::glibc_double::exp(f64::from(-(j.wrapping_mul(j))) / (2.0 * s * s)) as f32
        } else if j == 0 {
            1.0
        } else {
            0.0
        };
        kern.push(f64::from(val));
        sum += val;
    }
    let f = 1.0 / f64::from(sum);
    for k in &mut kern {
        *k *= f;
    }
    kern
}

/// One pass of `convolve_separable`: `convolve(source, kernel)` with a kernel along `axis`
/// (centred on `(n - 1) / 2`), the image zero outside; each output voxel the `float` sum, in
/// kernel order, of the `double` products (`val += source(...) * kernel(...)`). A sum that
/// comes out NaN (NaN or infinite voxels) is computed again with SSE's NaNs ([`crate::sse`]:
/// the products' and the running sum's NaNs as FSL's binary passes them on). The z slices are
/// shared between `threads` threads; the result does not depend on their number.
fn convolve_axis(src: &Volume, ker: &[f64], axis: usize, threads: usize) -> Volume {
    use crate::sse::{add_f64, mul_f64};
    let mut result = src.clone();
    let (nx, ny, nz) = (src.dims[0], src.dims[1], src.dims[2]);
    let mid = (ker.len() as i64 - 1) / 2;
    let plane = (nx * ny) as usize;
    let n = (nx * ny * nz) as usize;
    if n == 0 {
        return result;
    }
    let data = &src.data[..n];
    let at = |x: i64, y: i64, z: i64, d: i64| -> Option<f32> {
        let (cx, cy, cz) = match axis {
            0 => (x + d, y, z),
            1 => (x, y + d, z),
            _ => (x, y, z + d),
        };
        if cx < 0 || cy < 0 || cz < 0 || cx >= nx || cy >= ny || cz >= nz {
            return None; // + 0 * k: no change
        }
        Some(data[((cz * ny + cy) * nx + cx) as usize])
    };
    let slice = |z: i64, out: &mut [f32]| {
        for y in 0..ny {
            for x in 0..nx {
                let mut val = 0f32;
                for (m, &k) in ker.iter().enumerate() {
                    if let Some(v) = at(x, y, z, m as i64 - mid) {
                        val = (f64::from(val) + f64::from(v) * k) as f32;
                    }
                }
                if val.is_nan() {
                    val = 0.0;
                    for (m, &k) in ker.iter().enumerate() {
                        if let Some(v) = at(x, y, z, m as i64 - mid) {
                            val = add_f64(f64::from(val), mul_f64(f64::from(v), k)) as f32;
                        }
                    }
                }
                out[(y * nx + x) as usize] = val;
            }
        }
    };
    let out = &mut result.data[..n];
    let threads = threads.max(1).min(nz as usize);
    if threads == 1 {
        for (z, chunk) in out.chunks_mut(plane).enumerate() {
            slice(z as i64, chunk);
        }
    } else {
        let per = (nz as usize).div_ceil(threads);
        std::thread::scope(|scope| {
            for (t, block) in out.chunks_mut(plane * per).enumerate() {
                let slice = &slice;
                scope.spawn(move || {
                    for (k, chunk) in block.chunks_mut(plane).enumerate() {
                        slice((t * per + k) as i64, chunk);
                    }
                });
            }
        });
    }
    result
}

/// `smooth(source, sigma_mm)`: a separable Gaussian of `sigma_mm` millimetres, along x, then y,
/// then z, each kernel of radius `2 (int)(sigma - 0.001) + 3` voxels, on `threads` threads (the
/// result does not depend on their number).
pub fn smooth(src: &Volume, sigma_mm: f32, threads: usize) -> Volume {
    let sigma = [
        sigma_mm / src.xdim(),
        sigma_mm / src.ydim(),
        sigma_mm / src.zdim(),
    ];
    let mut result = src.clone();
    for (axis, &s) in sigma.iter().enumerate() {
        let radius = crate::cnum::f64_to_i32(f64::from(s) - 0.001)
            .wrapping_mul(2)
            .wrapping_add(3);
        let ker = gaussian_kernel_1d(s, radius);
        result = convolve_axis(&result, &ker, axis, threads);
    }
    result
}
