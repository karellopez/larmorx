// SPDX-License-Identifier: Apache-2.0
//! `ResampleImage` (ANTs v2.6.5, `Examples/ResampleImage.cxx`): ITK's `ResampleImageFilter`
//! with an identity transform onto a grid with the same origin and direction and a new
//! spacing or size, in the image's pixel type, without smoothing.
//!
//! What ANTs computes, and this reproduces:
//! - **by spacing:** `size[d] = int(spacing_old · size_old / spacing[d] + 0.5)`;
//! - **by size:** `spacing[d] = spacing_old · (size_old − 1) / (size[d] − 1)` (the first and
//!   last voxel centres stay in place);
//! - the interpolators: linear, nearest neighbour, Gaussian (sigma defaults to the input
//!   spacing, alpha to 1), windowed sinc of radius 3 **with ITK's default boundary condition**
//!   (the nearest edge voxel outside the image, unlike `antsApplyTransforms`), B-spline of
//!   order 0 to 5;
//! - the output pixel type is the input's (`char` … `double`): each interpolated value is
//!   clamped to the type's range and converted with a C++ `static_cast` (truncation for
//!   integers), as `CastPixelWithBoundsChecking` does; points outside the input get 0.

use larmorx_core::RealElement;
use larmorx_interp::Interpolation;
use larmorx_io::nifti::itk::ItkGeometry;

use crate::image::{AntsImage, Pixel};
use crate::resample::{ResampleError, resample_identity};

/// How [`resample_image`] sets the output grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResampleTarget<'a> {
    /// The new spacing: one value for every axis or one per axis.
    Spacing(&'a [f64]),
    /// The new size: one value for every axis or one per axis.
    Size(&'a [usize]),
}

/// [`resample_image`] failed.
#[derive(Debug, thiserror::Error)]
pub enum ResampleImageError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Resample(#[from] ResampleError),
}

/// `values` for every one of `d` axes: one value repeated, or one per axis.
fn per_axis<T: Copy>(values: &[T], d: usize, what: &str) -> Result<Vec<T>, ResampleImageError> {
    match values.len() {
        1 => Ok(vec![values[0]; d]),
        n if n == d => Ok(values.to_vec()),
        n => Err(ResampleImageError::Invalid(format!(
            "{n} {what} values for a {d}-dimensional image (one or {d})"
        ))),
    }
}

/// The output grid of `ResampleImage` for an input on `input`.
pub fn output_geometry(
    input: &ItkGeometry,
    target: ResampleTarget<'_>,
) -> Result<ItkGeometry, ResampleImageError> {
    let d = input.ndim;
    let mut out = input.clone();
    match target {
        ResampleTarget::Spacing(spacing) => {
            let spacing = per_axis(spacing, d, "spacing")?;
            for (k, &s) in spacing.iter().enumerate() {
                let size = (input.spacing[k] * input.size[k] as f64) / s + 0.5;
                // `static_cast<int>`, then an unsigned size.
                if !(1.0..2_147_483_648.0).contains(&size) {
                    return Err(ResampleImageError::Invalid(format!(
                        "an output size of {size} voxels along axis {k}"
                    )));
                }
                out.size[k] = size as usize;
                out.spacing[k] = s;
            }
        }
        ResampleTarget::Size(size) => {
            let size = per_axis(size, d, "size")?;
            for (k, &n) in size.iter().enumerate() {
                if n < 2 {
                    return Err(ResampleImageError::Invalid(format!(
                        "an output size of {n} along axis {k}: ANTs divides by size − 1"
                    )));
                }
                let ratio = (input.size[k] as f64 - 1.0) / (n as f64 - 1.0);
                out.size[k] = n;
                out.spacing[k] = input.spacing[k] * ratio;
            }
        }
    }
    if out.spacing.iter().any(|s| !(s.is_finite() && *s > 0.0)) {
        return Err(ResampleImageError::Invalid(format!(
            "the output spacing {:?} is not positive",
            out.spacing
        )));
    }
    Ok(out.stored_in_new_image())
}

/// `ResampleImage`: `input` resampled onto the grid of [`output_geometry`] with
/// `interpolation`, in its own pixel type. Linear and nearest-neighbour interpolation work in
/// 2, 3 and 4 dimensions, the others in 3D.
pub fn resample_image<T: Pixel + RealElement>(
    input: &AntsImage<T>,
    target: ResampleTarget<'_>,
    interpolation: &Interpolation,
    n_threads: usize,
) -> Result<AntsImage<T>, ResampleImageError> {
    let geometry = output_geometry(&input.geometry, target)?;
    let data = resample_identity(
        &input.data,
        &input.geometry,
        &geometry,
        interpolation,
        T::from_f64(0.0),
        T::from_f64_bounded,
        n_threads,
    )?;
    Ok(AntsImage {
        data,
        geometry,
        meta: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_io::nifti::itk::GeometrySource;

    fn image<T: Pixel>(size: &[usize], spacing: &[f64], data: Vec<T>) -> AntsImage<T> {
        let d = size.len();
        AntsImage {
            data,
            geometry: ItkGeometry {
                ndim: d,
                size: size.to_vec(),
                spacing: spacing.to_vec(),
                origin: vec![0.0; d],
                direction: (0..d)
                    .map(|i| (0..d).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
                    .collect(),
                source: GeometrySource::Default,
                flipped: vec![false; d],
            },
            meta: Default::default(),
        }
    }

    #[test]
    fn grids_by_spacing_and_by_size() {
        let input = image(&[10, 9, 8], &[1.0, 1.5, 2.0], vec![0.0f32; 720]);
        let g = output_geometry(&input.geometry, ResampleTarget::Spacing(&[2.0])).unwrap();
        assert_eq!(g.size, [5, 7, 8]);
        let g = output_geometry(&input.geometry, ResampleTarget::Size(&[19, 17, 15])).unwrap();
        assert_eq!(g.spacing, [0.5, 0.75, 1.0]);
        assert!(output_geometry(&input.geometry, ResampleTarget::Size(&[1])).is_err());
        assert!(output_geometry(&input.geometry, ResampleTarget::Spacing(&[1.0, 2.0])).is_err());
    }

    #[test]
    fn integer_outputs_are_clamped_and_truncated() {
        // A ramp 0, 100, 200, 255 along x, as unsigned char: linear values between voxels
        // are truncated (227.5 is 227).
        let data: Vec<u8> = vec![0, 100, 200, 255];
        let input = image(&[4, 2], &[1.0, 1.0], [data.clone(), data].concat());
        let out = resample_image(
            &input,
            ResampleTarget::Spacing(&[0.5, 1.0]),
            &Interpolation::Linear,
            1,
        )
        .unwrap();
        // The last output voxel (x = 3.5) lies outside: 0.
        assert_eq!(&out.data[..8], &[0, 50, 100, 150, 200, 227, 255, 0]);
        let wide: Vec<i16> = vec![-300, 0, 300, 600];
        let input = image(&[4, 2], &[1.0, 1.0], [wide.clone(), wide].concat());
        let out = resample_image(
            &input,
            ResampleTarget::Spacing(&[0.5, 1.0]),
            &Interpolation::Linear,
            2,
        )
        .unwrap();
        assert_eq!(&out.data[..4], &[-300, -150, 0, 150]);
    }
}
