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

use larmorx_core::parallel::{self, ThreadPoolError};
use larmorx_core::vnl_svd::vnl_inverse;
use larmorx_core::{IndexFloor, RealElement, same_value};
use larmorx_interp::{
    BSplineMirror, InterpError, Interpolation, Interpolator, LinearYz, NearestYz, Volume, linear,
    nearest,
};
use larmorx_io::nifti::itk::ItkGeometry;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    /// The output grid needs more memory than the machine has (ITK throws
    /// `MemoryAllocationError`; the ANTs programs abort).
    #[error(transparent)]
    Memory(#[from] larmorx_core::alloc::AllocError),
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
    floor: &IndexFloor,
) -> f64 {
    let d = size.len();
    let mut base = [0i64; 8];
    let mut distance = [0.0f64; 8];
    for k in 0..d {
        // `Math::Floor<long>(double)` (0, not −1, for negative values closer to 0 than 2⁻⁵⁵;
        // ITK-01, the exact floor unless reproduced).
        base[k] = floor.pick(larmorx_image::grid::itk_floor_f64(c[k]), c[k]);
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

/// [`linear_unoptimized`] for an interpolator with `float` coordinates
/// (`LinearInterpolateImageFunction<I, float>`): its `InternalComputationType` is `float`, so
/// each distance is a float, `overlap *= distance` is a float product and
/// `overlap *= 1.0 - distance` a double product stored as float; the sum stays in double.
fn linear_unoptimized_f32<T: RealElement>(
    data: &[T],
    size: &[usize],
    strides: &[usize],
    c: &[f64],
    floor: &IndexFloor,
) -> f64 {
    let d = size.len();
    let mut base = [0i64; 8];
    let mut distance = [0.0f32; 8];
    for k in 0..d {
        // `Math::Floor<long>(float)`: vnl's SSE2 floor, `(2x − 0.5f)` rounded to an integer
        // and halved, which is 0 (not −1) for negative values closer to 0 than 2⁻²⁵ or so.
        let x = c[k] as f32;
        base[k] = floor.pick(
            i64::from(larmorx_image::grid::vnl_floor_f32(x)),
            f64::from(x),
        );
        distance[k] = x - base[k] as f32;
    }
    let mut value = 0.0f64;
    for counter in 0..(1usize << d) {
        let mut overlap = 1.0f32;
        let mut upper = counter;
        let mut offset = 0usize;
        for k in 0..d {
            let index = if upper & 1 == 1 {
                overlap *= distance[k];
                (base[k] + 1).min(size[k] as i64 - 1)
            } else {
                overlap = (f64::from(overlap) * (1.0 - f64::from(distance[k]))) as f32;
                base[k].max(0)
            };
            offset += index as usize * strides[k];
            upper >>= 1;
        }
        value += data[offset].to_f64() * f64::from(overlap);
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

/// The coordinate type of a `ResampleImageFilter`: its `TInterpolatorPrecisionType` (and so
/// the identity transform's and the interpolator's coordinate type).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coordinates {
    /// `double`, ITK's default (antsApplyTransforms, ResampleImage, ResampleImageBySpacing).
    Double,
    /// `float` (`ResampleImageFilter<I, O, float>`, as DenoiseImage declares it): each
    /// output point is rounded to `float` before the `IdentityTransform<float>`, the
    /// continuous indices of a scan line's ends are rounded to `float`, their difference is
    /// taken in float, and each voxel's continuous index `start + alpha · vector` (computed in
    /// double) is rounded to `float` before the interpolator sees it.
    Float,
}

/// larmorx's working memory per output voxel of `ResampleImage` and `ResampleImageBySpacing`,
/// measured with `validation/investigations/difftest/scaled_memory.py` (12.5 to 16 bytes; ANTs
/// 7.3) when ResampleImageBySpacing still resampled a second time for its ANTS-32 notice (the
/// voxels beyond the input are now counted in the pass itself): kept as the guard's bound
/// until measured again.
pub const GRID_BYTES: f64 = 16.0;

/// ANTS-162: the memory an output grid of `output` needs ([`GRID_BYTES`] per voxel), checked
/// before anything is allocated; the error names the bytes and their likely cause (the
/// input's extent over the output's spacing; a header in metres or micrometres) when they
/// exceed [`larmorx_core::alloc::memory_limit`].
pub fn check_output_grid(input: &ItkGeometry, output: &ItkGeometry) -> Result<(), String> {
    let voxels: f64 = output.size.iter().map(|&n| n as f64).product();
    let needed = GRID_BYTES * voxels;
    let Err(limit) = larmorx_core::alloc::check_plan(needed) else {
        return Ok(());
    };
    let short = |x: f64| {
        let s = format!("{x:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let join = |v: &[f64]| v.iter().map(|&x| short(x)).collect::<Vec<_>>().join(" x ");
    let extent: Vec<f64> = (0..input.ndim)
        .map(|i| input.size[i] as f64 * input.spacing[i])
        .collect();
    let unusual = input.spacing[..input.ndim.min(3)]
        .iter()
        .any(|&s| !(0.01..100.0).contains(&s.abs()));
    Err(format!(
        "the output would need about {} (a grid of {} voxels), more than {limit}. The \
         input's voxels are {} mm: it spans {} mm, resampled to voxels of {} mm{}. An error \
         (ANTS-162); LARMORX_UPSTREAM_DEFECTS=reproduce attempts it as ANTs does",
        larmorx_core::alloc::gib(needed),
        output
            .size
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" x "),
        join(&input.spacing),
        join(&extent),
        join(&output.spacing),
        if unusual {
            " (a header whose units are metres or micrometres makes an image 1000 times larger \
             or smaller than it looks)"
        } else {
            ""
        },
    ))
}

/// Resamples `input` (on `input_geometry`) onto `output` through the identity transform, as
/// `ResampleImageFilter` does: points outside the input get `default_value`, the others the
/// interpolated value (in double) passed through `cast`.
///
/// Linear and nearest-neighbour interpolation work in 1 to 4 dimensions; the others in 3D.
/// `floor` chooses the floor of the base index of linear interpolation in 4D and of the
/// windowed sinc (ITK's, or the exact one: defect ITK-01) and counts the output values the
/// choice changed ([`IndexFloor::checked`]).
#[allow(clippy::too_many_arguments)]
pub fn resample_identity<T, O>(
    input: &[T],
    input_geometry: &ItkGeometry,
    output: &ItkGeometry,
    interpolation: &Interpolation,
    default_value: O,
    cast: impl Fn(f64) -> O + Sync,
    floor: &IndexFloor,
    n_threads: usize,
) -> Result<Vec<O>, ResampleError>
where
    T: RealElement,
    O: Copy + PartialEq + Send + Sync,
{
    resample_identity_with(
        input,
        input_geometry,
        output,
        interpolation,
        default_value,
        cast,
        Coordinates::Double,
        floor,
        n_threads,
    )
}

/// [`resample_identity`] with the filter's coordinate type.
#[allow(clippy::too_many_arguments)]
pub fn resample_identity_with<T, O>(
    input: &[T],
    input_geometry: &ItkGeometry,
    output: &ItkGeometry,
    interpolation: &Interpolation,
    default_value: O,
    cast: impl Fn(f64) -> O + Sync,
    coordinates: Coordinates,
    floor: &IndexFloor,
    n_threads: usize,
) -> Result<Vec<O>, ResampleError>
where
    T: RealElement,
    O: Copy + PartialEq + Send + Sync,
{
    resample_identity_mirrored(
        input,
        input_geometry,
        output,
        interpolation,
        default_value,
        cast,
        coordinates,
        floor,
        &BSplineMirror::full(),
        n_threads,
    )
}

/// [`resample_identity_with`] with the B-spline's mirror boundary `mirror` (the repeated one,
/// or ITK's: defect ITK-98), which counts the output values that needed the second mirror.
#[allow(clippy::too_many_arguments)]
pub fn resample_identity_mirrored<T, O>(
    input: &[T],
    input_geometry: &ItkGeometry,
    output: &ItkGeometry,
    interpolation: &Interpolation,
    default_value: O,
    cast: impl Fn(f64) -> O + Sync,
    coordinates: Coordinates,
    floor: &IndexFloor,
    mirror: &BSplineMirror,
    n_threads: usize,
) -> Result<Vec<O>, ResampleError>
where
    T: RealElement,
    O: Copy + PartialEq + Send + Sync,
{
    resample_identity_counted(
        input,
        input_geometry,
        output,
        interpolation,
        default_value,
        cast,
        coordinates,
        floor,
        mirror,
        n_threads,
    )
    .map(|(out, _)| out)
}

/// [`resample_identity`] and the number of output voxels beyond the input (those given
/// `default_value`): what a second pass with another default would change, counted without one.
#[allow(clippy::too_many_arguments)]
pub fn resample_identity_outside<T, O>(
    input: &[T],
    input_geometry: &ItkGeometry,
    output: &ItkGeometry,
    interpolation: &Interpolation,
    default_value: O,
    cast: impl Fn(f64) -> O + Sync,
    floor: &IndexFloor,
    n_threads: usize,
) -> Result<(Vec<O>, usize), ResampleError>
where
    T: RealElement,
    O: Copy + PartialEq + Send + Sync,
{
    resample_identity_counted(
        input,
        input_geometry,
        output,
        interpolation,
        default_value,
        cast,
        Coordinates::Double,
        floor,
        &BSplineMirror::full(),
        n_threads,
    )
}

/// The core of the resamplers: the output and its count of voxels beyond the input.
#[allow(clippy::too_many_arguments)]
fn resample_identity_counted<T, O>(
    input: &[T],
    input_geometry: &ItkGeometry,
    output: &ItkGeometry,
    interpolation: &Interpolation,
    default_value: O,
    cast: impl Fn(f64) -> O + Sync,
    coordinates: Coordinates,
    floor: &IndexFloor,
    mirror: &BSplineMirror,
    n_threads: usize,
) -> Result<(Vec<O>, usize), ResampleError>
where
    T: RealElement,
    O: Copy + PartialEq + Send + Sync,
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
        return Ok((Vec::new(), 0));
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
                Interpolator::new(volume3(), other, spacing)
                    .map(|i| Evaluator::Other(i.with_bspline_mirror(mirror.local())))
            })??
        }
        other => return Err(ResampleError::Interpolation(name(other), d)),
    };
    let mut out = larmorx_core::alloc::filled(n_out, default_value)?;
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
        float: coordinates == Coordinates::Float,
        floor,
        outside: AtomicUsize::new(0),
    };
    parallel::with_threads(n_threads, || match d {
        1 => job.run::<1>(&mut out),
        2 => job.run::<2>(&mut out),
        3 => job.run::<3>(&mut out),
        _ => job.run::<4>(&mut out),
    })?;
    if let Evaluator::Other(interpolator) = &job.evaluator {
        mirror.absorb(interpolator.bspline_mirror());
    }
    Ok((out, job.outside.load(Ordering::Relaxed)))
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
    /// [`Coordinates::Float`].
    float: bool,
    /// The floor of base indices (ITK-01).
    floor: &'a IndexFloor,
    /// The output voxels given `default_value` (beyond the input).
    outside: AtomicUsize,
}

impl<T, O, C> Rows<'_, T, O, C>
where
    T: RealElement,
    O: Copy + PartialEq + Send + Sync,
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
            self.round(&mut point);
            self.in_maps.point_to_index(&point, &mut start);
            self.round(&mut start);
            index[0] = nx as f64;
            self.out_maps.voxel_to_point(&index, &mut point);
            self.round(&mut point);
            self.in_maps.point_to_index(&point, &mut end);
            self.round(&mut end);
            let vector: [f64; D] = if self.float {
                std::array::from_fn(|k| f64::from(end[k] as f32 - start[k] as f32))
            } else {
                std::array::from_fn(|k| end[k] - start[k])
            };
            if matches!(
                self.evaluator,
                Evaluator::Linear3(_) | Evaluator::Nearest3(_)
            ) && vector[1..].iter().all(|&v| v == 0.0)
            {
                // The row runs along the input's first axis (axis-aligned images): the other
                // indices are the same for every voxel (`start + alpha·0`), so their part of
                // the interpolation is computed once.
                let beyond = self.aligned_row(&start, &vector, &limit, line);
                if beyond > 0 {
                    self.outside.fetch_add(beyond, Ordering::Relaxed);
                }
                return;
            }
            let mut beyond = 0usize;
            for (x, o) in line.iter_mut().enumerate() {
                let alpha = x as f64 / nx as f64;
                let mut c: [f64; D] = std::array::from_fn(|k| start[k] + alpha * vector[k]);
                self.round(&mut c);
                let inside = (0..D).all(|k| c[k] >= -0.5 && c[k] < limit[k]);
                if !inside {
                    *o = self.default_value;
                    beyond += 1;
                    continue;
                }
                let c3: [f64; 3] = std::array::from_fn(|k| if k < D { c[k] } else { 0.0 });
                let evaluate = |floor: &IndexFloor| {
                    (self.cast)(match &self.evaluator {
                        Evaluator::Linear3(volume) => linear(volume, c3),
                        Evaluator::Nearest3(volume) => nearest(volume, c3),
                        Evaluator::LinearN if self.float => linear_unoptimized_f32(
                            self.input,
                            self.in_size,
                            self.strides,
                            &c,
                            floor,
                        ),
                        Evaluator::LinearN => {
                            linear_unoptimized(self.input, self.in_size, self.strides, &c, floor)
                        }
                        Evaluator::NearestN => {
                            let offset: usize = (0..D)
                                .map(|k| (c[k] + 0.5) as i64 as usize * self.strides[k])
                                .sum();
                            self.input[offset].to_f64()
                        }
                        Evaluator::Other(interpolator) => {
                            interpolator.evaluate_with_floor(c3, floor)
                        }
                    })
                };
                // Only these take ITK's floor (ITK-01).
                *o = match &self.evaluator {
                    Evaluator::LinearN | Evaluator::Other(_) => {
                        self.floor.checked(evaluate, same_value)
                    }
                    _ => evaluate(self.floor),
                };
            }
            if beyond > 0 {
                self.outside.fetch_add(beyond, Ordering::Relaxed);
            }
        });
    }

    /// Rounds coordinates to `float` in [`Coordinates::Float`] mode.
    fn round<const D: usize>(&self, v: &mut [f64; D]) {
        if self.float {
            for x in v.iter_mut() {
                *x = f64::from(*x as f32);
            }
        }
    }

    /// One output row whose points share every index but the first (`start[1..]`), in 2D or
    /// 3D with linear or nearest-neighbour interpolation: the same values as the general loop
    /// of [`Rows::run`]. Returns the number of its voxels beyond the input.
    fn aligned_row<const D: usize>(
        &self,
        start: &[f64; D],
        vector: &[f64; D],
        limit: &[f64; D],
        line: &mut [O],
    ) -> usize {
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
            return nx;
        }
        let float = self.float;
        let x_value = |x: usize| {
            let v = start[0] + (x as f64 / nx as f64) * vector[0];
            if float { f64::from(v as f32) } else { v }
        };
        let x_inside = |cx: f64| cx >= -0.5 && cx < limit[0];
        let mut beyond = 0usize;
        match &self.evaluator {
            Evaluator::Linear3(volume) => {
                let yz = LinearYz::new(volume, c[1], c[2]);
                for (x, o) in line.iter_mut().enumerate() {
                    let cx = x_value(x);
                    *o = if x_inside(cx) {
                        (self.cast)(yz.evaluate(volume, cx))
                    } else {
                        beyond += 1;
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
                        beyond += 1;
                        self.default_value
                    };
                }
            }
            _ => unreachable!("aligned rows are for the 2D and 3D linear and nearest paths"),
        }
        beyond
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;
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
        resample_identity(
            data,
            g,
            o,
            interpolation,
            default,
            |v| v as f32,
            &IndexFloor::itk(),
            threads,
        )
        .unwrap()
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

    /// The voxels beyond the input are counted in the pass itself: exactly those a second pass
    /// with another default value changes (ResampleImageBySpacing's ANTS-32 notice), on the
    /// aligned rows and the general path (an oblique output), in 2D, 3D and 4D.
    #[test]
    fn the_voxels_beyond_the_input_are_counted() {
        let cases: Vec<(Vec<usize>, Vec<usize>, Option<f64>)> = vec![
            (vec![4, 4], vec![8, 5], None),
            (vec![3, 4, 2], vec![5, 6, 3], None),
            (vec![3, 4, 2], vec![5, 6, 3], Some(0.3)),
            (vec![2, 3, 2, 2], vec![3, 4, 3, 3], None),
        ];
        for (size, out_size, rotate) in cases {
            let n: usize = size.iter().product();
            let data: Vec<f32> = (0..n).map(|i| (i % 7) as f32 + 0.5).collect();
            let g = geometry(&size, &vec![1.0; size.len()]);
            let mut o = g.clone();
            o.size = out_size;
            o.origin = vec![-0.75; size.len()];
            if let Some(t) = rotate {
                let (s, c) = (f64::sin(t), f64::cos(t));
                o.direction = vec![vec![c, -s, 0.0], vec![s, c, 0.0], vec![0.0, 0.0, 1.0]];
            }
            let floor = IndexFloor::itk();
            let (a, beyond) = resample_identity_outside(
                &data,
                &g,
                &o,
                &Interpolation::Linear,
                0.0f32,
                |v| v as f32,
                &floor,
                2,
            )
            .unwrap();
            let b = resample_identity(
                &data,
                &g,
                &o,
                &Interpolation::Linear,
                -7.0f32,
                |v| v as f32,
                &floor,
                2,
            )
            .unwrap();
            let differ = a
                .iter()
                .zip(&b)
                .filter(|(x, y)| x.to_bits() != y.to_bits())
                .count();
            assert_eq!(beyond, differ, "{size:?} {rotate:?}");
            assert!(beyond > 0 && beyond < a.len(), "{size:?}");
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
        assert!(
            resample_identity(&data, &g, &o, &bspline, 0.0, |v| v, &IndexFloor::itk(), 1).is_err()
        );
        let g3 = geometry(&[4, 4, 4], &[1.0, 1.0, 1.0]);
        let data3 = vec![2.0f64; 64];
        let out = resample_identity(
            &data3,
            &g3,
            &g3,
            &bspline,
            0.0,
            |v| v,
            &IndexFloor::itk(),
            1,
        )
        .unwrap();
        assert!(out.iter().all(|&v| (v - 2.0).abs() < 1e-12));
    }

    #[test]
    fn a_floor_just_below_zero_in_4d() {
        // ITK-01: the output grid starts 1e-20 voxels before the input, so the first x index
        // is −1e-20, whose floor ITK takes as 0: linear interpolation in 4D then mixes in the
        // next voxel with weight −1e-20. With a 0 there and 100 next to it, ITK gives −1e-18.
        let size = [3usize, 1, 1, 2];
        let data = [0.0f32, 100.0, 100.0, 0.0, 100.0, 100.0];
        let g = geometry(&size, &[1.0; 4]);
        let mut o = g.clone();
        o.origin[0] = -1e-20;
        let run = |mode| {
            let floor = IndexFloor::new(mode);
            let out = resample_identity(
                &data,
                &g,
                &o,
                &Interpolation::Linear,
                0.0,
                |v| v as f32,
                &floor,
                1,
            )
            .unwrap();
            (out, floor.differing(), floor.changed())
        };
        let (fixed, d, c) = run(UpstreamDefects::Fix);
        assert_eq!(fixed[0], 0.0);
        assert_eq!((d, c), (2, 2));
        let (ants, d, c) = run(UpstreamDefects::Reproduce);
        assert_eq!(ants[0], -1e-18);
        assert_eq!((d, c), (2, 2));
        assert_eq!(fixed[1..3], ants[1..3]);
        // Without a 0 beside the 100, the tiny weight vanishes in the sum: no change counted.
        let data = [50.0f32, 100.0, 100.0, 50.0, 100.0, 100.0];
        let floor = IndexFloor::new(UpstreamDefects::Fix);
        resample_identity(
            &data,
            &g,
            &o,
            &Interpolation::Linear,
            0.0,
            |v| v as f32,
            &floor,
            1,
        )
        .unwrap();
        assert_eq!((floor.differing(), floor.changed()), (2, 0));
    }
}
