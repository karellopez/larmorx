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
//!
//! **ANTS-29.** By size, a size of 1 makes the spacing `old · (n − 1) / 0`, infinite: vnl's
//! SVD then fails on the grid's matrix, no output point maps into the input, and ANTs writes
//! an image of zeros with an infinite spacing, exit 0; ITK's writer places the image's first
//! voxel at `origin + D · diag(spacing) · 0`, NaN. With [`UpstreamDefects::Fix`] (the default)
//! a size below 2 is an error; with [`UpstreamDefects::Reproduce`] the output is ANTs': that
//! grid, every voxel 0.

use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects};
use larmorx_core::{IndexFloor, RealElement};
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

/// The output grid of `ResampleImage` for an input on `input`. With
/// [`UpstreamDefects::Reproduce`] a size of 1 gives ANTs' grid (ANTS-29, see the module).
pub fn output_geometry(
    input: &ItkGeometry,
    target: ResampleTarget<'_>,
    mode: UpstreamDefects,
) -> Result<ItkGeometry, ResampleImageError> {
    let d = input.ndim;
    let mut out = input.clone();
    let mut size_one = false;
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
                if n < 2 && !(n == 1 && mode.reproduce()) {
                    return Err(ResampleImageError::Invalid(format!(
                        "an output size of {n} along axis {k}: ANTs divides by size − 1{}",
                        if n == 1 {
                            " and writes an image of zeros with an infinite spacing (ANTS-29; \
                             with the upstream defects reproduced, so does larmorx)"
                                .to_owned()
                        } else {
                            String::new()
                        }
                    )));
                }
                size_one |= n == 1;
                let ratio = (input.size[k] as f64 - 1.0) / (n as f64 - 1.0);
                out.size[k] = n;
                out.spacing[k] = input.spacing[k] * ratio;
            }
        }
    }
    if size_one {
        // ANTS-29 reproduced: ITK's writer places index 0 at origin + D·diag(spacing)·0.
        for i in 0..d {
            let mut p = out.origin[i];
            for j in 0..d {
                p += input.direction[i][j] * out.spacing[j] * 0.0;
            }
            out.origin[i] = p;
        }
        return Ok(out.stored_in_new_image());
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
/// `interpolation`, in its own pixel type; with the notices of the defects it met. Linear and
/// nearest-neighbour interpolation work in 2, 3 and 4 dimensions, the others in 3D.
pub fn resample_image<T: Pixel + RealElement>(
    input: &AntsImage<T>,
    target: ResampleTarget<'_>,
    interpolation: &Interpolation,
    n_threads: usize,
    mode: UpstreamDefects,
) -> Result<(AntsImage<T>, Vec<Notice>), ResampleImageError> {
    let mut notices = Notices::new(mode);
    let geometry = output_geometry(&input.geometry, target, mode)?;
    if geometry.spacing.iter().any(|s| !s.is_finite()) {
        // ANTS-29 reproduced: no output point maps into the input.
        notices.add(
            &defects::ANTS_29,
            "a size of 1 gives an infinite spacing: the image is all 0, its origin NaN, as ANTs \
             writes it",
        );
        let n = geometry.size.iter().product();
        let image = AntsImage {
            data: vec![T::from_f64(0.0); n],
            geometry,
            meta: Default::default(),
        };
        return Ok((image, notices.into_vec()));
    }
    let floor = IndexFloor::new(mode);
    let data = resample_identity(
        &input.data,
        &input.geometry,
        &geometry,
        interpolation,
        T::from_f64(0.0),
        T::from_f64_bounded,
        &floor,
        n_threads,
    )?;
    floor.notice(&mut notices, "output voxels");
    Ok((
        AntsImage {
            data,
            geometry,
            meta: Default::default(),
        },
        notices.into_vec(),
    ))
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
        let fix = UpstreamDefects::Fix;
        let geometry = |t| output_geometry(&input.geometry, t, fix);
        let g = geometry(ResampleTarget::Spacing(&[2.0])).unwrap();
        assert_eq!(g.size, [5, 7, 8]);
        let g = geometry(ResampleTarget::Size(&[19, 17, 15])).unwrap();
        assert_eq!(g.spacing, [0.5, 0.75, 1.0]);
        assert!(geometry(ResampleTarget::Size(&[1])).is_err());
        assert!(geometry(ResampleTarget::Spacing(&[1.0, 2.0])).is_err());
    }

    #[test]
    fn size_one() {
        // ANTS-29: an error, or ANTs' image of zeros with an infinite spacing.
        let input = image(&[10, 9, 8], &[1.0, 1.5, 2.0], vec![5.0f32; 720]);
        let target = ResampleTarget::Size(&[1, 9, 8]);
        let e = resample_image(
            &input,
            target,
            &Interpolation::Linear,
            1,
            UpstreamDefects::Fix,
        );
        assert!(e.unwrap_err().to_string().contains("ANTS-29"));
        let (out, notices) = resample_image(
            &input,
            target,
            &Interpolation::Linear,
            1,
            UpstreamDefects::Reproduce,
        )
        .unwrap();
        assert_eq!(out.size(), [1, 9, 8]);
        assert_eq!(out.geometry.spacing[0], f64::INFINITY);
        assert_eq!(&out.geometry.spacing[1..], [1.5, 2.0]);
        assert!(out.geometry.origin.iter().all(|o| o.is_nan()));
        assert!(out.data.iter().all(|&v| v == 0.0));
        assert_eq!(notices[0].defect.id, "ANTS-29");
    }

    #[test]
    fn integer_outputs_are_clamped_and_truncated() {
        // A ramp 0, 100, 200, 255 along x, as unsigned char: linear values between voxels
        // are truncated (227.5 is 227).
        let data: Vec<u8> = vec![0, 100, 200, 255];
        let input = image(&[4, 2], &[1.0, 1.0], [data.clone(), data].concat());
        let (out, _) = resample_image(
            &input,
            ResampleTarget::Spacing(&[0.5, 1.0]),
            &Interpolation::Linear,
            1,
            UpstreamDefects::Fix,
        )
        .unwrap();
        // The last output voxel (x = 3.5) lies outside: 0.
        assert_eq!(&out.data[..8], &[0, 50, 100, 150, 200, 227, 255, 0]);
        let wide: Vec<i16> = vec![-300, 0, 300, 600];
        let input = image(&[4, 2], &[1.0, 1.0], [wide.clone(), wide].concat());
        let (out, _) = resample_image(
            &input,
            ResampleTarget::Spacing(&[0.5, 1.0]),
            &Interpolation::Linear,
            2,
            UpstreamDefects::Fix,
        )
        .unwrap();
        assert_eq!(&out.data[..4], &[-300, -150, 0, 150]);
    }
}
