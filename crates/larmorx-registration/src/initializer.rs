// SPDX-License-Identifier: Apache-2.0
//! `CenteredTransformInitializer` (ITK v5.4.5,
//! `Modules/Registration/Common/include/itkCenteredTransformInitializer.hxx`) with
//! `ImageMomentsCalculator` (`Modules/Filtering/ImageStatistics/include/itkImageMomentsCalculator.hxx`),
//! as ANTs' `[fixedImage,movingImage,feature]` initial transforms use them: the rotation
//! centre is the fixed image's centre (of its grid, or of mass), the translation the moving
//! image's centre minus the fixed image's; with feature 2, the origins.

use crate::{Image, ItkException, Real, Space};

/// `GetCenterOfGravity()`: `Σ point·value / Σ value` over the voxels in index order, in
/// double.
pub fn center_of_gravity<R: Real, const D: usize>(
    image: &Image<R, D>,
) -> Result<[f64; D], ItkException> {
    let mut m0 = 0.0f64;
    let mut cg = [0.0f64; D];
    for (o, &v) in image.data.iter().enumerate() {
        let value = v.to_f64();
        let p = image.space.index_to_point(&image.space.index_of(o));
        m0 += value;
        for i in 0..D {
            cg[i] += p[i] * value;
        }
    }
    if m0 == 0.0 {
        return Err(ItkException::new(
            "Modules/Filtering/ImageStatistics/include/itkImageMomentsCalculator.hxx",
            123,
            "ImageMomentsCalculator",
            "Compute(): Total Mass of the image was zero. Aborting here to prevent division by zero later on.",
        ));
    }
    for c in cg.iter_mut() {
        *c /= m0;
    }
    Ok(cg)
}

/// The physical point of the region's centre (`(start + (size − 1) / 2.0)` as a continuous
/// index of `R`, then `TransformContinuousIndexToPhysicalPoint<R, R>`: each row summed in
/// `R`, plus the origin).
pub fn geometric_center<R: Real, const D: usize>(space: &Space<D>) -> [R; D] {
    let c: [R; D] = std::array::from_fn(|k| {
        R::from_f64(
            R::from_i64(space.start[k]).to_f64()
                + R::from_i64(space.size[k] as i64 - 1).to_f64() / 2.0,
        )
    });
    std::array::from_fn(|r| {
        let mut sum = R::ZERO;
        for k in 0..D {
            sum = R::from_f64(sum.to_f64() + space.index_to_point[r][k] * c[k].to_f64());
        }
        R::from_f64(sum.to_f64() + space.origin[r])
    })
}

/// How the initial transform is derived from the two images (ANTs' third parameter).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    /// 0: the images' centres (`GeometryOn`).
    Geometry,
    /// 1: the centres of mass (`MomentsOn`).
    Moments,
    /// 2 (or more): the origins.
    Origins,
}

/// The rotation centre and translation of the initial transform.
pub fn initialize<R: Real, const D: usize>(
    fixed: &Image<R, D>,
    moving: &Image<R, D>,
    feature: Feature,
) -> Result<([R; D], [R; D]), ItkException> {
    initialize_pixels(fixed, moving, feature)
}

/// [`initialize`] of images of pixel type `P` for a transform of type `R` (`antsAI`'s
/// `CenteredTransformInitializer<AffineTransform<double>, Image<float>, Image<float>>`): the
/// moments are summed in double whatever the pixel type.
pub fn initialize_pixels<R: Real, P: Real, const D: usize>(
    fixed: &Image<P, D>,
    moving: &Image<P, D>,
    feature: Feature,
) -> Result<([R; D], [R; D]), ItkException> {
    Ok(match feature {
        Feature::Moments => {
            let f = center_of_gravity(fixed)?;
            let m = center_of_gravity(moving)?;
            (
                std::array::from_fn(|i| R::from_f64(f[i])),
                std::array::from_fn(|i| R::from_f64(m[i] - f[i])),
            )
        }
        Feature::Geometry => {
            let f: [R; D] = geometric_center(&fixed.space);
            let m: [R; D] = geometric_center(&moving.space);
            (f, std::array::from_fn(|i| m[i] - f[i]))
        }
        Feature::Origins => (
            std::array::from_fn(|i| R::from_f64(fixed.space.origin[i])),
            std::array::from_fn(|i| R::from_f64(moving.space.origin[i] - fixed.space.origin[i])),
        ),
    })
}
