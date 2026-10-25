// SPDX-License-Identifier: Apache-2.0
//! `antsApplyTransforms`: resampling an image through a chain of transforms.
//!
//! Ported from ANTs v2.6.5 (`Examples/antsApplyTransforms.cxx`) and ITK v5.4.5
//! (`itkResampleImageFilter.hxx`). For every voxel of the reference grid, its physical point is
//! mapped through the transform chain into the input image, converted to a continuous index
//! there and interpolated; points that fall outside the input (more than half a voxel beyond
//! its grid) get the default value. As in ITK, when every transform is linear each output
//! scan line is mapped by its two ends and the indices in between are interpolated along the
//! line, which rounds slightly differently from mapping every voxel.
//!
//! **Precision.** ANTs instantiates the whole pipeline with `double`, or with `float` under
//! `--float 1` ([`Precision`]). In float, as ITK's types make it
//! (`ResampleImageFilter<Image<float>, Image<float>, float>`, `CompositeTransform<float>`):
//! - each output voxel's physical point is computed in double and stored as `Point<float>`;
//! - every transform maps float points in float arithmetic, from its parameters cast to float
//!   ([`TransformChain::with_precision`]);
//! - the continuous index is computed in double from the float point and stored as float, and
//!   so are the ends of a scan line; along the line, `vector = end − start` is a float
//!   difference and each index `start + α·vector` is computed in double and stored as float;
//! - the interpolators take float indices ([`Interpolator::with_precision`]) and return double;
//! - the output pixel is float: the interpolated value is clamped to ±`FLT_MAX`
//!   (`CastComponentWithBoundsChecking`) and rounded, the default value is a float.
//!
//! **Upstream defects (D13),** corrected unless [`ApplyTransformsOptions::upstream_defects`]
//! is [`UpstreamDefects::Reproduce`]: ITK's floor of the windowed sinc's and the displacement
//! fields' base indices (ITK-01) and a versor that fails to load in float (ITK-10).

use larmorx_core::defects::{Notice, Notices, UpstreamDefects};
use larmorx_core::element::RealElement;
use larmorx_core::parallel::{self, ThreadPoolError};
use larmorx_core::{Grid3, IndexFloor, ItkReal, Precision, same_value};
use larmorx_interp::{BSplineMirror, InterpError, Interpolation, Interpolator, Volume, is_inside};
use larmorx_transform::{PrecisionChain, TransformChain, TransformError};
use rayon::prelude::*;

/// Options of [`apply_transforms`].
#[derive(Clone, Debug, PartialEq)]
pub struct ApplyTransformsOptions {
    pub interpolation: Interpolation,
    /// The value of output voxels that map outside the input (`-f`; default 0).
    pub default_value: f64,
    /// Worker threads (0 = all logical CPUs). The result does not depend on it.
    pub n_threads: usize,
    /// The computation type: [`Precision::Double`] (ANTs' default) or [`Precision::Float`]
    /// (`--float 1`).
    pub precision: Precision,
    /// Correct ITK's defects (ITK-01, ITK-10; the default) or reproduce them.
    pub upstream_defects: UpstreamDefects,
}

impl Default for ApplyTransformsOptions {
    fn default() -> Self {
        ApplyTransformsOptions {
            interpolation: Interpolation::Linear,
            default_value: 0.0,
            n_threads: 1,
            precision: Precision::Double,
            upstream_defects: UpstreamDefects::Fix,
        }
    }
}

/// [`apply_transforms`] failed.
#[derive(Debug, thiserror::Error)]
pub enum ApplyTransformsError {
    #[error("the input has {values} values, which is not a whole number of {voxels}-voxel volumes")]
    InputSize { values: usize, voxels: usize },
    #[error(transparent)]
    Interpolation(#[from] InterpError),
    #[error(transparent)]
    Transform(#[from] TransformError),
    #[error(transparent)]
    Threads(#[from] ThreadPoolError),
}

/// Resamples `input` onto the `reference` grid through `chain` (applied last first, as
/// `antsApplyTransforms` applies its `-t` options).
///
/// `input` holds one or more volumes on `input_grid` (Fortran order, `x` fastest, volumes one
/// after the other, as a 4D time series). Each volume is resampled the same way (`-e 3`) and
/// the result has as many volumes, on `reference`, in the same layout. With
/// [`Precision::Float`] the values are `f32` values (the output image's pixels), returned as
/// `f64`. The notices name the upstream defects that changed the result (or, reproduced,
/// would have).
pub fn apply_transforms<T: RealElement>(
    input: &[T],
    input_grid: &Grid3,
    reference: &Grid3,
    chain: &TransformChain,
    options: &ApplyTransformsOptions,
) -> Result<(Vec<f64>, Vec<Notice>), ApplyTransformsError> {
    match options.precision {
        Precision::Double => apply_as::<T, f64>(input, input_grid, reference, chain, options),
        Precision::Float => apply_as::<T, f32>(input, input_grid, reference, chain, options),
    }
}

/// `ResampleImageFilter`'s output pixel from the interpolated value
/// (`CastPixelWithBoundsChecking`): in double the value itself; in float clamped to
/// `[−FLT_MAX, FLT_MAX]` (so ±∞ become ±`FLT_MAX`; NaN passes) and rounded.
#[inline]
fn output_pixel<R: ItkReal>(value: f64) -> f64 {
    match R::PRECISION {
        Precision::Double => value,
        Precision::Float => {
            let max = f64::from(f32::MAX);
            if value <= -max {
                -max
            } else if value >= max {
                max
            } else {
                f64::from(value as f32)
            }
        }
    }
}

fn apply_as<T: RealElement, R: ItkReal>(
    input: &[T],
    input_grid: &Grid3,
    reference: &Grid3,
    chain: &TransformChain,
    options: &ApplyTransformsOptions,
) -> Result<(Vec<f64>, Vec<Notice>), ApplyTransformsError> {
    let n_in = input_grid.len();
    if n_in == 0 || !input.len().is_multiple_of(n_in) {
        return Err(ApplyTransformsError::InputSize {
            values: input.len(),
            voxels: n_in,
        });
    }
    let n_volumes = input.len() / n_in;
    let n_out = reference.len();
    if n_out == 0 || n_volumes == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    let nx = reference.size[0];
    let mode = options.upstream_defects;
    let mut notices = Notices::new(mode);
    let (points, chain_notices) = chain.prepare::<R>(mode)?;
    notices.extend(chain_notices);
    let mapper = IndexMapper {
        linear: points.is_linear(),
        chain: points,
        input: input_grid,
        reference,
    };
    let default_value = R::from_f64(options.default_value).to_f64();
    // The floor of the displacement-field lookups and of the windowed sinc (ITK-01). A voxel
    // whose mapping or interpolation met a differing floor is computed again with the other
    // floor, and counted if its value changed.
    let floor = IndexFloor::new(mode);
    let floor = &floor;
    // ITK-98: the B-spline's mirror on an axis too short for its support.
    let mirror = BSplineMirror::new(mode);
    let mirror = &mirror;
    let output = parallel::with_threads(options.n_threads, || {
        // With several volumes, map every voxel once and reuse the indices (ITK recomputes
        // them per volume, with the same result).
        let cached: Option<Vec<Mapped>> = (n_volumes > 1).then(|| {
            let mut all = vec![Mapped::default(); n_out];
            all.par_chunks_mut(nx)
                .enumerate()
                .for_each(|(row, out)| mapper.row(row, out, floor));
            all
        });
        let mut output = vec![0.0f64; n_out * n_volumes];
        for (volume, out) in input.chunks_exact(n_in).zip(output.chunks_mut(n_out)) {
            let interpolator = Interpolator::with_precision(
                Volume::new(volume, input_grid.size),
                &options.interpolation,
                input_grid.spacing,
                R::PRECISION,
            )?
            .with_bspline_mirror(mirror.local());
            let value = |c: [f64; 3], floor: &IndexFloor| {
                if is_inside(c, input_grid.size) {
                    output_pixel::<R>(interpolator.evaluate_with_floor(c, floor))
                } else {
                    default_value
                }
            };
            out.par_chunks_mut(nx).enumerate().for_each_init(
                || vec![Mapped::default(); nx],
                |buffer, (row, out_row)| {
                    let mapped: &[Mapped] = match &cached {
                        Some(all) => &all[row * nx..(row + 1) * nx],
                        None => {
                            mapper.row(row, buffer, floor);
                            buffer
                        }
                    };
                    for (x, (o, m)) in out_row.iter_mut().zip(mapped).enumerate() {
                        let local = floor.local();
                        *o = value(m.index, &local);
                        let differing = m.differing + local.differing();
                        if differing > 0 {
                            let other = floor.alternative();
                            let index = if m.differing > 0 {
                                mapper.voxel_with(mapper.voxel_index(row, x), &other)
                            } else {
                                m.index
                            };
                            let changed = !same_value(o, &value(index, &other));
                            floor.record(differing, changed);
                        }
                    }
                },
            );
            mirror.absorb(interpolator.bspline_mirror());
        }
        Ok::<_, ApplyTransformsError>(output)
    })??;
    floor.notice(&mut notices, "output voxels");
    mirror.notice(&mut notices, "output voxels");
    Ok((output, notices.into_vec()))
}

/// A reference voxel mapped to the input: its continuous index, and how many floors of
/// displacement-field lookups differed from ITK's on the way (ITK-01).
#[derive(Clone, Copy, Debug, Default)]
struct Mapped {
    index: [f64; 3],
    differing: usize,
}

/// Maps reference voxels to continuous indices of the input, in the computation type `R`
/// (the indices are `R` values, returned as `f64`).
struct IndexMapper<'a, R> {
    chain: PrecisionChain<'a, R>,
    input: &'a Grid3,
    reference: &'a Grid3,
    linear: bool,
}

impl<R: ItkReal> IndexMapper<'_, R> {
    /// The continuous index of output voxel `index`, as `R` values: the voxel's point in
    /// double, stored as `Point<R>`, mapped by the chain in `R`, then
    /// `TransformPhysicalPointToContinuousIndex<R>` (double arithmetic, stored as `R`).
    #[inline]
    fn voxel(&self, index: [usize; 3], floor: &IndexFloor) -> [R; 3] {
        let point = self.reference.voxel_to_point(index).map(R::from_f64);
        let mapped = self.chain.transform_point_with(point, floor);
        self.input
            .point_to_index(mapped.map(|x| x.to_f64()))
            .map(R::from_f64)
    }

    /// [`IndexMapper::voxel`] as `f64` values.
    fn voxel_with(&self, index: [usize; 3], floor: &IndexFloor) -> [f64; 3] {
        self.voxel(index, floor).map(|v| v.to_f64())
    }

    /// The index of voxel `x` of output scan line `row`.
    fn voxel_index(&self, row: usize, x: usize) -> [usize; 3] {
        let ny = self.reference.size[1];
        [x, row % ny, row / ny]
    }

    /// The continuous indices of output scan line `row` (`j + ny·k`), the displacement-field
    /// lookups taking the floor of `floor`'s kind.
    fn row(&self, row: usize, out: &mut [Mapped], floor: &IndexFloor) {
        let [nx, ny, _] = self.reference.size;
        let (j, k) = (row % ny, row / ny);
        if self.linear {
            // ITK's LinearThreadedGenerateData: the line from index 0 to index nx (one past the
            // end), sampled at x / nx; the difference of the ends in R, each index computed in
            // double and stored as R.
            // A linear chain looks nothing up, so takes no floor.
            let start = self.voxel([0, j, k], floor);
            let end = self.voxel([nx, j, k], floor);
            let vector: [f64; 3] = std::array::from_fn(|d| (end[d] - start[d]).to_f64());
            let start = start.map(|s| s.to_f64());
            for (x, c) in out.iter_mut().enumerate() {
                let alpha = x as f64 / nx as f64;
                c.index =
                    std::array::from_fn(|d| R::from_f64(start[d] + alpha * vector[d]).to_f64());
                c.differing = 0;
            }
        } else {
            for (x, c) in out.iter_mut().enumerate() {
                let local = floor.local();
                c.index = self.voxel_with([x, j, k], &local);
                c.differing = local.differing();
            }
        }
    }
}

/// `antsApplyTransformsToPoints`: maps physical points (LPS) through `chain`.
///
/// The chain maps points the way it maps output voxels to input positions in
/// [`apply_transforms`], so to move points from the moving to the fixed image, give the
/// transforms (or their inverses) of the opposite direction, as with ANTs.
pub fn transform_points(points: &[[f64; 3]], chain: &TransformChain) -> Vec<[f64; 3]> {
    points.iter().map(|&p| chain.transform_point(p)).collect()
}

/// `antsApplyTransforms`' output pixel types (`-u`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputType {
    Char,
    UChar,
    Short,
    Int,
    Float,
    Double,
}

impl OutputType {
    /// The `-u` name.
    pub fn name(self) -> &'static str {
        match self {
            OutputType::Char => "char",
            OutputType::UChar => "uchar",
            OutputType::Short => "short",
            OutputType::Int => "int",
            OutputType::Float => "float",
            OutputType::Double => "double",
        }
    }

    /// The `-u` names (`default` is the computation type and is handled by the caller).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "char" => OutputType::Char,
            "uchar" => OutputType::UChar,
            "short" => OutputType::Short,
            "int" => OutputType::Int,
            "float" => OutputType::Float,
            "double" => OutputType::Double,
            _ => return None,
        })
    }
}
