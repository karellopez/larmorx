// SPDX-License-Identifier: Apache-2.0
//! ITK's `ResampleImageFilter` with an `IdentityTransform`, in the image's own `D`
//! dimensions, as ANTs' `ResampleImageBySpacing` and `ResampleImage` use it: each output
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
//! Interpolation:
//! - linear: ITK's `LinearInterpolateImageFunction`, in 2D and 3D its optimised paths
//!   ([`larmorx_interp::linear`]), in 4D `EvaluateUnoptimized` (the weighted sum of the 16
//!   neighbours);
//! - nearest neighbour (rounding half up), in any dimension;
//! - the other interpolators of [`Interpolation`] (B-spline, Gaussian, windowed sinc, label
//!   interpolators) in 3D, through [`larmorx_interp::Interpolator`].
//!
//! Points more than half a voxel outside the input get the default value; the others go
//! through the caller's cast (ITK's `CastPixelWithBoundsChecking` for the output pixel type).

use larmorx_core::RealElement;
use larmorx_core::parallel::{self, ThreadPoolError};
use larmorx_core::vnl_svd::vnl_inverse;
use larmorx_interp::{
    InterpError, Interpolation, Interpolator, LinearYz, NearestYz, Volume, linear, nearest,
};
use larmorx_io::nifti::itk::ItkGeometry;
use rayon::prelude::*;

/// [`resample_identity`] failed.
#[derive(Debug, thiserror::Error)]
pub enum ResampleError {
    #[error("the {0} image geometry is singular")]
    Singular(&'static str),
    #[error("the input and output have {input} and {output} dimensions")]
    Dimensions { input: usize, output: usize },
    #[error("{0}-dimensional images are not supported")]
    Unsupported(usize),
    #[error("{0} interpolation is supported for 3D images only, not yet in {1}D")]
    Interpolation(&'static str, usize),
    #[error("the input has {values} values for {voxels} voxels")]
    InputSize { values: usize, voxels: usize },
    #[error(transparent)]
    Interpolator(#[from] InterpError),
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
fn linear_unoptimized<T: RealElement>(
    data: &[T],
    size: &[usize],
    strides: &[usize],
    c: &[f64],
) -> f64 {
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
        value += data[offset].to_f64() * overlap;
    }
    value
}

/// How the voxels of one resampling are interpolated.
enum Evaluator<'a, T> {
    /// 2D (one slice) and 3D linear, ITK's optimised paths.
    Linear3(Volume<'a, T>),
    /// 2D and 3D nearest neighbour.
    Nearest3(Volume<'a, T>),
    /// 1D and 4D linear (`EvaluateUnoptimized`).
    LinearN,
    /// 1D and 4D nearest neighbour.
    NearestN,
    /// Any other interpolator, 3D.
    Other(Interpolator<'a, T>),
}

/// Resamples `input` (on `input_geometry`) onto `output` through the identity transform, as
/// `ResampleImageFilter` does: points outside the input get `default_value`, the others the
/// interpolated value (in double) passed through `cast`.
///
/// Linear and nearest-neighbour interpolation work in 1 to 4 dimensions; the others in 3D.
pub fn resample_identity<T, O>(
    input: &[T],
    input_geometry: &ItkGeometry,
    output: &ItkGeometry,
    interpolation: &Interpolation,
    default_value: O,
    cast: impl Fn(f64) -> O + Sync,
    n_threads: usize,
) -> Result<Vec<O>, ResampleError>
where
    T: RealElement,
    O: Copy + Send + Sync,
{
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
    // A 2D image is a 3D volume with one slice and a zero z index, which never interpolates
    // along z: ITK's 2D optimised linear path.
    let size3: [usize; 3] = std::array::from_fn(|k| if k < d { in_size[k] } else { 1 });
    let volume3 = || Volume::new(input, size3);
    let evaluator = match interpolation {
        Interpolation::Linear if (2..=3).contains(&d) => Evaluator::Linear3(volume3()),
        Interpolation::NearestNeighbor if (2..=3).contains(&d) => Evaluator::Nearest3(volume3()),
        Interpolation::Linear => Evaluator::LinearN,
        Interpolation::NearestNeighbor => Evaluator::NearestN,
        other if d == 3 => {
            let spacing: [f64; 3] = std::array::from_fn(|k| input_geometry.spacing[k]);
            parallel::with_threads(n_threads, || {
                Interpolator::new(volume3(), other, spacing).map(Evaluator::Other)
            })??
        }
        other => return Err(ResampleError::Interpolation(name(other), d)),
    };
    let mut out = vec![default_value; n_out];
    let job = Rows {
        input,
        in_size,
        strides: &strides,
        output,
        in_maps: &in_maps,
        out_maps: &out_maps,
        evaluator,
        default_value,
        cast: &cast,
    };
    parallel::with_threads(n_threads, || match d {
        1 => job.run::<1>(&mut out),
        2 => job.run::<2>(&mut out),
        3 => job.run::<3>(&mut out),
        _ => job.run::<4>(&mut out),
    })?;
    Ok(out)
}

fn name(i: &Interpolation) -> &'static str {
    match i {
        Interpolation::Linear => "linear",
        Interpolation::NearestNeighbor => "nearest-neighbour",
        Interpolation::BSpline { .. } => "B-spline",
        Interpolation::Gaussian { .. } => "Gaussian",
        Interpolation::MultiLabel { .. } => "MultiLabel",
        Interpolation::GenericLabel => "GenericLabel",
        Interpolation::WindowedSinc(_) | Interpolation::WindowedSincEdge(_) => "windowed-sinc",
    }
}

/// What every output row of [`resample_identity`] needs.
struct Rows<'a, T, O, C> {
    input: &'a [T],
    in_size: &'a [usize],
    strides: &'a [usize],
    output: &'a ItkGeometry,
    in_maps: &'a Maps,
    out_maps: &'a Maps,
    evaluator: Evaluator<'a, T>,
    default_value: O,
    cast: &'a C,
}

impl<T, O, C> Rows<'_, T, O, C>
where
    T: RealElement,
    O: Copy + Send + Sync,
    C: Fn(f64) -> O + Sync,
{
    /// Fills `out` row by row (in parallel on the current pool), for `D` dimensions.
    fn run<const D: usize>(&self, out: &mut [O]) {
        let nx = self.output.size[0];
        let limit: [f64; D] = std::array::from_fn(|k| self.in_size[k] as f64 - 0.5);
        out.par_chunks_mut(nx).enumerate().for_each(|(row, line)| {
            let mut index = [0.0f64; D];
            let mut rest = row;
            for (c, &n) in index.iter_mut().zip(&self.output.size).skip(1) {
                *c = (rest % n) as f64;
                rest /= n;
            }
            let (mut point, mut start, mut end) = ([0.0f64; D], [0.0f64; D], [0.0f64; D]);
            self.out_maps.voxel_to_point(&index, &mut point);
            self.in_maps.point_to_index(&point, &mut start);
            index[0] = nx as f64;
            self.out_maps.voxel_to_point(&index, &mut point);
            self.in_maps.point_to_index(&point, &mut end);
            let vector: [f64; D] = std::array::from_fn(|k| end[k] - start[k]);
            if matches!(
                self.evaluator,
                Evaluator::Linear3(_) | Evaluator::Nearest3(_)
            ) && vector[1..].iter().all(|&v| v == 0.0)
            {
                // The row runs along the input's first axis (axis-aligned images): the other
                // indices are the same for every voxel (`start + alpha·0`), so their part of
                // the interpolation is computed once.
                self.aligned_row(&start, &vector, &limit, line);
                return;
            }
            for (x, o) in line.iter_mut().enumerate() {
                let alpha = x as f64 / nx as f64;
                let c: [f64; D] = std::array::from_fn(|k| start[k] + alpha * vector[k]);
                let inside = (0..D).all(|k| c[k] >= -0.5 && c[k] < limit[k]);
                if !inside {
                    *o = self.default_value;
                    continue;
                }
                let c3: [f64; 3] = std::array::from_fn(|k| if k < D { c[k] } else { 0.0 });
                let value = match &self.evaluator {
                    Evaluator::Linear3(volume) => linear(volume, c3),
                    Evaluator::Nearest3(volume) => nearest(volume, c3),
                    Evaluator::LinearN => {
                        linear_unoptimized(self.input, self.in_size, self.strides, &c)
                    }
                    Evaluator::NearestN => {
                        let offset: usize = (0..D)
                            .map(|k| (c[k] + 0.5) as i64 as usize * self.strides[k])
                            .sum();
                        self.input[offset].to_f64()
                    }
                    Evaluator::Other(interpolator) => interpolator.evaluate(c3),
                };
                *o = (self.cast)(value);
            }
        });
    }

    /// One output row whose points share every index but the first (`start[1..]`), in 2D or
    /// 3D with linear or nearest-neighbour interpolation: the same values as the general loop
    /// of [`Rows::run`].
    fn aligned_row<const D: usize>(
        &self,
        start: &[f64; D],
        vector: &[f64; D],
        limit: &[f64; D],
        line: &mut [O],
    ) {
        let nx = line.len();
        // `start[k] + alpha · (±0)` for the other axes, at alpha = 0 (the same for every
        // alpha ≥ 0).
        let c: [f64; 3] = std::array::from_fn(|k| {
            if k == 0 || k >= D {
                0.0
            } else {
                start[k] + 0.0 * vector[k]
            }
        });
        let inside = (1..D).all(|k| c[k] >= -0.5 && c[k] < limit[k]);
        if !inside {
            line.fill(self.default_value);
            return;
        }
        let x_value = |x: usize| start[0] + (x as f64 / nx as f64) * vector[0];
        let x_inside = |cx: f64| cx >= -0.5 && cx < limit[0];
        match &self.evaluator {
            Evaluator::Linear3(volume) => {
                let yz = LinearYz::new(volume, c[1], c[2]);
                for (x, o) in line.iter_mut().enumerate() {
                    let cx = x_value(x);
                    *o = if x_inside(cx) {
                        (self.cast)(yz.evaluate(volume, cx))
                    } else {
                        self.default_value
                    };
                }
            }
            Evaluator::Nearest3(volume) => {
                let yz = NearestYz::new(volume, c[1], c[2]);
                for (x, o) in line.iter_mut().enumerate() {
                    let cx = x_value(x);
                    *o = if x_inside(cx) {
                        (self.cast)(yz.evaluate(volume, cx))
                    } else {
                        self.default_value
                    };
                }
            }
            _ => unreachable!("aligned rows are for the 2D and 3D linear and nearest paths"),
        }
    }
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

    fn f32s(
        data: &[f32],
        g: &ItkGeometry,
        o: &ItkGeometry,
        interpolation: &Interpolation,
        default: f32,
        threads: usize,
    ) -> Vec<f32> {
        resample_identity(data, g, o, interpolation, default, |v| v as f32, threads).unwrap()
    }

    #[test]
    fn same_grid_is_identity_and_4d_matches_3d_per_volume() {
        let size = [5usize, 4, 3, 2];
        let n: usize = size.iter().product();
        let data: Vec<f32> = (0..n).map(|i| ((i * 7) % 11) as f32).collect();
        let g = geometry(&size, &[1.0, 1.0, 1.0, 1.0]);
        for interp in [Interpolation::Linear, Interpolation::NearestNeighbor] {
            assert_eq!(f32s(&data, &g, &g, &interp, -1.0, 2), data);
        }
        // Halving the spacing along x only: linear interpolation along x in 4D is close to the
        // 3D result on each volume (the 4D weighted sum rounds differently from the 3D
        // `a + (b − a)·d`).
        let mut g4 = g.clone();
        g4.spacing[0] = 0.5;
        g4.size[0] = 10;
        let out4 = f32s(&data, &g, &g4, &Interpolation::Linear, 0.0, 1);
        let g3 = geometry(&size[..3], &[1.0, 1.0, 1.0]);
        let mut g3o = g3.clone();
        g3o.spacing[0] = 0.5;
        g3o.size[0] = 10;
        let half = n / 2;
        for t in 0..2 {
            let out3 = f32s(
                &data[t * half..(t + 1) * half],
                &g3,
                &g3o,
                &Interpolation::Linear,
                0.0,
                3,
            );
            for (a, b) in out4[t * out3.len()..(t + 1) * out3.len()].iter().zip(&out3) {
                assert!((a - b).abs() < 1e-5, "{a} {b}");
            }
        }
    }

    #[test]
    fn aligned_rows_equal_the_general_path() {
        // An oblique copy of the same grid takes the general path; the axis-aligned one the
        // row path. Both must give exactly the interpolator's values at the same indices.
        let size = [9usize, 7, 5];
        let n: usize = size.iter().product();
        let data: Vec<f32> = (0..n).map(|i| ((i * 37) % 23) as f32 * 0.7).collect();
        let g = geometry(&size, &[1.0, 1.5, 2.0]);
        let mut o = g.clone();
        o.spacing = vec![0.7, 1.1, 1.3];
        o.size = vec![13, 10, 8];
        let (in_maps, out_maps) = (Maps::new(&g).unwrap(), Maps::new(&o).unwrap());
        for interp in [Interpolation::Linear, Interpolation::NearestNeighbor] {
            let fast = f32s(&data, &g, &o, &interp, -5.0, 2);
            let volume = Volume::new(&data, [9, 7, 5]);
            let expected: Vec<f32> = (0..o.size.iter().product::<usize>())
                .map(|i| {
                    let idx = [i % 13, (i / 13) % 10, i / 130];
                    // The general path: start + (x / nx) · (end − start) for every axis.
                    let (mut p, mut start, mut end) = ([0.0; 3], [0.0; 3], [0.0; 3]);
                    out_maps.voxel_to_point(&[0.0, idx[1] as f64, idx[2] as f64], &mut p);
                    in_maps.point_to_index(&p, &mut start);
                    out_maps.voxel_to_point(&[13.0, idx[1] as f64, idx[2] as f64], &mut p);
                    in_maps.point_to_index(&p, &mut end);
                    let alpha = idx[0] as f64 / 13.0;
                    let c: [f64; 3] =
                        std::array::from_fn(|k| start[k] + alpha * (end[k] - start[k]));
                    if (0..3).all(|k| c[k] >= -0.5 && c[k] < size[k] as f64 - 0.5) {
                        match interp {
                            Interpolation::Linear => linear(&volume, c) as f32,
                            _ => nearest(&volume, c) as f32,
                        }
                    } else {
                        -5.0
                    }
                })
                .collect();
            assert_eq!(fast, expected, "{interp:?}");
        }
    }

    #[test]
    fn points_outside_get_the_default_and_others_need_3d() {
        let data = vec![1.0f32; 16];
        let g = geometry(&[4, 4], &[1.0, 1.0]);
        let mut o = g.clone();
        o.size = vec![8, 4];
        let out = f32s(&data, &g, &o, &Interpolation::Linear, 9.0, 1);
        assert_eq!(&out[..8], &[1.0, 1.0, 1.0, 1.0, 9.0, 9.0, 9.0, 9.0]);
        let bspline = Interpolation::BSpline { order: 3 };
        assert!(resample_identity(&data, &g, &o, &bspline, 0.0, |v| v, 1).is_err());
        let g3 = geometry(&[4, 4, 4], &[1.0, 1.0, 1.0]);
        let data3 = vec![2.0f64; 64];
        let out = resample_identity(&data3, &g3, &g3, &bspline, 0.0, |v| v, 1).unwrap();
        assert!(out.iter().all(|&v| (v - 2.0).abs() < 1e-12));
    }
}
