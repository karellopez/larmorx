// SPDX-License-Identifier: Apache-2.0
//! ITK's `ResampleImageFilter` with an `IdentityTransform`, in the image's own `D`
//! dimensions, as ANTs' `ResampleImageBySpacing` (and `ResampleImage`) use it: each output
//! voxel's physical point is looked up in the input at the same point.
//!
//! The identity is a linear transform, so ITK takes its scan-line path
//! (`LinearThreadedGenerateData`, ITK v5.4.5 `itkResampleImageFilter.hxx`): the continuous
//! input indices of index 0 and of index `size_x` (one past the end) of each output line are
//! computed, and the voxels in between at `start + (x / size_x)·(end − start)`. Point ↔ index
//! conversions are ITK's (`TransformIndexToPhysicalPoint` summed from the origin;
//! `TransformPhysicalPointToContinuousIndex` with vnl's SVD inverse of
//! `direction·diag(spacing)`, [`larmorx_core::vnl_svd::vnl_inverse`]).
//!
//! Interpolation is ITK's `LinearInterpolateImageFunction` (2D and 3D: its optimised paths,
//! `larmorx_interp`'s linear interpolator; 4D: `EvaluateUnoptimized`, the weighted sum of the
//! 16 neighbours) or `NearestNeighborInterpolateImageFunction` (rounding half up). Points more
//! than half a voxel outside the input get the default value.

use larmorx_core::parallel::{self, ThreadPoolError};
use larmorx_core::vnl_svd::vnl_inverse;
use larmorx_interp::{Interpolation, Interpolator, Volume};
use larmorx_io::nifti::itk::ItkGeometry;
use rayon::prelude::*;

/// The interpolators of [`resample_identity`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResampleInterpolation {
    Linear,
    NearestNeighbor,
}

/// [`resample_identity`] failed.
#[derive(Debug, thiserror::Error)]
pub enum ResampleError {
    #[error("the {0} image geometry is singular")]
    Singular(&'static str),
    #[error("the input and output have {input} and {output} dimensions")]
    Dimensions { input: usize, output: usize },
    #[error("{0}-dimensional images are not supported")]
    Unsupported(usize),
    #[error("the input has {values} values for {voxels} voxels")]
    InputSize { values: usize, voxels: usize },
    #[error(transparent)]
    Threads(#[from] ThreadPoolError),
}

/// An image geometry's index ↔ point mappings, as `itk::ImageBase` computes them.
struct Maps {
    d: usize,
    origin: Vec<f64>,
    /// `direction · diag(spacing)`, row-major.
    index_to_point: Vec<f64>,
    /// Its vnl SVD inverse, row-major.
    point_to_index: Vec<f64>,
}

impl Maps {
    fn new(g: &ItkGeometry) -> Option<Self> {
        let d = g.ndim;
        let mut m = vec![0.0f64; d * d];
        for i in 0..d {
            for j in 0..d {
                m[i * d + j] = g.direction[i][j] * g.spacing[j];
            }
        }
        if !m.iter().all(|v| v.is_finite()) || determinant(&m, d) == 0.0 {
            return None;
        }
        let point_to_index = vnl_inverse(&m, d)?;
        Some(Maps {
            d,
            origin: g.origin.clone(),
            index_to_point: m,
            point_to_index,
        })
    }

    /// `TransformIndexToPhysicalPoint` for an integer index: from the origin, adding
    /// `M[i][j]·index[j]` term by term.
    fn voxel_to_point(&self, index: &[f64], point: &mut [f64]) {
        let d = self.d;
        for (i, p) in point.iter_mut().enumerate().take(d) {
            let row = &self.index_to_point[i * d..(i + 1) * d];
            let mut sum = self.origin[i];
            for (&m, &x) in row.iter().zip(index) {
                sum += m * x;
            }
            *p = sum;
        }
    }

    /// `TransformPhysicalPointToContinuousIndex`: `M⁻¹ · (point − origin)`, each row summed
    /// from 0 in index order.
    fn point_to_index(&self, point: &[f64], index: &mut [f64]) {
        let d = self.d;
        let v: Vec<f64> = point.iter().zip(&self.origin).map(|(p, o)| p - o).collect();
        for (i, c) in index.iter_mut().enumerate().take(d) {
            let row = &self.point_to_index[i * d..(i + 1) * d];
            let mut sum = 0.0;
            for (&m, &x) in row.iter().zip(&v) {
                sum += m * x;
            }
            *c = sum;
        }
    }
}

/// The determinant (only compared with 0, as ITK's `GetInverse` does before inverting).
fn determinant(m: &[f64], d: usize) -> f64 {
    let mut a = m.to_vec();
    let mut det = 1.0;
    for c in 0..d {
        let p = (c..d)
            .max_by(|&x, &y| a[x * d + c].abs().total_cmp(&a[y * d + c].abs()))
            .unwrap_or(c);
        if a[p * d + c] == 0.0 {
            return 0.0;
        }
        if p != c {
            for k in 0..d {
                a.swap(p * d + k, c * d + k);
            }
            det = -det;
        }
        det *= a[c * d + c];
        for r in c + 1..d {
            let f = a[r * d + c] / a[c * d + c];
            for k in c..d {
                a[r * d + k] -= f * a[c * d + k];
            }
        }
    }
    det
}

/// `LinearInterpolateImageFunction::EvaluateUnoptimized`: the sum over the `2^D` neighbours
/// (bit `d` of the counter chooses the upper neighbour along axis `d`) of
/// `value · ∏ overlap`, the neighbours clamped to the image.
fn linear_unoptimized(data: &[f32], size: &[usize], strides: &[usize], c: &[f64]) -> f64 {
    let d = size.len();
    let mut base = [0i64; 8];
    let mut distance = [0.0f64; 8];
    for k in 0..d {
        base[k] = c[k].floor() as i64;
        distance[k] = c[k] - base[k] as f64;
    }
    let mut value = 0.0f64;
    for counter in 0..(1usize << d) {
        let mut overlap = 1.0f64;
        let mut upper = counter;
        let mut offset = 0usize;
        for k in 0..d {
            let index = if upper & 1 == 1 {
                overlap *= distance[k];
                (base[k] + 1).min(size[k] as i64 - 1)
            } else {
                overlap *= 1.0 - distance[k];
                base[k].max(0)
            };
            offset += index as usize * strides[k];
            upper >>= 1;
        }
        value += f64::from(data[offset]) * overlap;
    }
    value
}

/// Resamples `input` (on `input_geometry`) onto `output` through the identity transform, as
/// `ResampleImageFilter<float, float>` does: the output is `float`, points outside the input
/// get `default_value`.
pub fn resample_identity(
    input: &[f32],
    input_geometry: &ItkGeometry,
    output: &ItkGeometry,
    interpolation: ResampleInterpolation,
    default_value: f32,
    n_threads: usize,
) -> Result<Vec<f32>, ResampleError> {
    let d = input_geometry.ndim;
    if output.ndim != d {
        return Err(ResampleError::Dimensions {
            input: d,
            output: output.ndim,
        });
    }
    if !(1..=4).contains(&d) {
        return Err(ResampleError::Unsupported(d));
    }
    let in_size = &input_geometry.size;
    let voxels: usize = in_size.iter().product();
    if input.len() != voxels {
        return Err(ResampleError::InputSize {
            values: input.len(),
            voxels,
        });
    }
    let n_out: usize = output.size.iter().product();
    if n_out == 0 {
        return Ok(Vec::new());
    }
    let in_maps = Maps::new(input_geometry).ok_or(ResampleError::Singular("input"))?;
    let out_maps = Maps::new(output).ok_or(ResampleError::Singular("output"))?;
    let strides = larmorx_image::strides(in_size);
    // 2D and 3D linear interpolation: ITK's optimised paths (a 2D image is a 3D volume with
    // one slice and a zero z index, which never interpolates along z).
    let size3: [usize; 3] = std::array::from_fn(|k| if k < d { in_size[k] } else { 1 });
    let interpolator = if d <= 3 {
        let method = match interpolation {
            ResampleInterpolation::Linear => Interpolation::Linear,
            ResampleInterpolation::NearestNeighbor => Interpolation::NearestNeighbor,
        };
        Some(
            Interpolator::new(Volume::new(input, size3), &method, [1.0; 3])
                .expect("linear and nearest-neighbour interpolators always build"),
        )
    } else {
        None
    };
    let nx = output.size[0];
    let mut out = vec![0.0f32; n_out];
    parallel::with_threads(n_threads, || {
        out.par_chunks_mut(nx).enumerate().for_each(|(row, line)| {
            let mut index = vec![0.0f64; d];
            let mut rest = row;
            for (c, &n) in index.iter_mut().zip(&output.size).skip(1) {
                *c = (rest % n) as f64;
                rest /= n;
            }
            let mut point = vec![0.0f64; d];
            let mut start = vec![0.0f64; d];
            let mut end = vec![0.0f64; d];
            index[0] = 0.0;
            out_maps.voxel_to_point(&index, &mut point);
            in_maps.point_to_index(&point, &mut start);
            index[0] = nx as f64;
            out_maps.voxel_to_point(&index, &mut point);
            in_maps.point_to_index(&point, &mut end);
            let vector: Vec<f64> = (0..d).map(|k| end[k] - start[k]).collect();
            let mut c = vec![0.0f64; d];
            for (x, o) in line.iter_mut().enumerate() {
                let alpha = x as f64 / nx as f64;
                for k in 0..d {
                    c[k] = start[k] + alpha * vector[k];
                }
                let inside = (0..d).all(|k| c[k] >= -0.5 && c[k] < in_size[k] as f64 - 0.5);
                *o = if !inside {
                    default_value
                } else if let Some(interp) = &interpolator {
                    let c3: [f64; 3] = std::array::from_fn(|k| if k < d { c[k] } else { 0.0 });
                    interp.evaluate(c3) as f32
                } else {
                    match interpolation {
                        ResampleInterpolation::Linear => {
                            linear_unoptimized(input, in_size, &strides, &c) as f32
                        }
                        ResampleInterpolation::NearestNeighbor => {
                            let offset: usize = (0..d)
                                .map(|k| (c[k] + 0.5).floor() as usize * strides[k])
                                .sum();
                            input[offset]
                        }
                    }
                };
            }
        });
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_io::nifti::itk::GeometrySource;

    fn geometry(size: &[usize], spacing: &[f64]) -> ItkGeometry {
        let d = size.len();
        ItkGeometry {
            ndim: d,
            size: size.to_vec(),
            spacing: spacing.to_vec(),
            origin: vec![0.0; d],
            direction: (0..d)
                .map(|i| (0..d).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
                .collect(),
            source: GeometrySource::Default,
            flipped: vec![false; d],
        }
    }

    #[test]
    fn same_grid_is_identity_and_4d_matches_3d_per_volume() {
        let size = [5usize, 4, 3, 2];
        let n: usize = size.iter().product();
        let data: Vec<f32> = (0..n).map(|i| ((i * 7) % 11) as f32).collect();
        let g = geometry(&size, &[1.0, 1.0, 1.0, 1.0]);
        for interp in [
            ResampleInterpolation::Linear,
            ResampleInterpolation::NearestNeighbor,
        ] {
            assert_eq!(
                resample_identity(&data, &g, &g, interp, -1.0, 2).unwrap(),
                data
            );
        }
        // Halving the spacing along x only: linear interpolation along x in 4D is close to the
        // 3D result on each volume (the 4D weighted sum rounds differently from the 3D
        // `a + (b − a)·d`).
        let mut g4 = g.clone();
        g4.spacing[0] = 0.5;
        g4.size[0] = 10;
        let out4 =
            resample_identity(&data, &g, &g4, ResampleInterpolation::Linear, 0.0, 1).unwrap();
        let g3 = geometry(&size[..3], &[1.0, 1.0, 1.0]);
        let mut g3o = g3.clone();
        g3o.spacing[0] = 0.5;
        g3o.size[0] = 10;
        let half = n / 2;
        for t in 0..2 {
            let out3 = resample_identity(
                &data[t * half..(t + 1) * half],
                &g3,
                &g3o,
                ResampleInterpolation::Linear,
                0.0,
                3,
            )
            .unwrap();
            for (a, b) in out4[t * out3.len()..(t + 1) * out3.len()].iter().zip(&out3) {
                assert!((a - b).abs() < 1e-5, "{a} {b}");
            }
        }
    }

    #[test]
    fn points_outside_get_the_default() {
        let data = vec![1.0f32; 16];
        let g = geometry(&[4, 4], &[1.0, 1.0]);
        let mut o = g.clone();
        o.size = vec![8, 4];
        let out = resample_identity(&data, &g, &o, ResampleInterpolation::Linear, 9.0, 1).unwrap();
        assert_eq!(&out[..8], &[1.0, 1.0, 1.0, 1.0, 9.0, 9.0, 9.0, 9.0]);
    }
}
