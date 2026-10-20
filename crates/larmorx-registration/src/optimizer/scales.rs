// SPDX-License-Identifier: Apache-2.0
//! `RegistrationParameterScalesFromPhysicalShift` for transforms of global support (ITK
//! v5.4.5, `itkRegistrationParameterScalesEstimator.hxx`,
//! `itkRegistrationParameterScalesFromShiftBase.hxx`,
//! `itkRegistrationParameterScalesFromPhysicalShift.hxx`).
//!
//! For a linear moving transform the virtual domain is sampled at its `2^D` corners
//! (`CornerSampling`). A parameter's scale is the squared largest shift of a corner when the
//! parameter changes by 0.01, divided by 0.01²; the step scale of a gradient is the largest
//! corner shift of the gradient scaled to a largest component of 0.01, divided back.

use crate::transform::Composite;
use crate::{ItkException, Real, Space};

/// `SmallParameterVariation`.
const SMALL: f64 = 0.01;

/// `SampleVirtualDomainWithCorners`: the corners of the region, corner `i` taking the upper
/// end along axis `d` when bit `d` of `i` is set.
pub fn corners<const D: usize>(space: &Space<D>) -> Vec<[f64; D]> {
    (0..(1usize << D))
        .map(|i| {
            let idx: [i64; D] = std::array::from_fn(|d| {
                let bit = ((i >> d) & 1) as i64;
                space.start[d] + bit * (space.size[d] as i64 - 1)
            });
            space.index_to_point(&idx)
        })
        .collect()
}

/// `ComputeSampleShifts(delta)`: each sample mapped by the transform before and after
/// `UpdateTransformParameters(delta)`, the distances (in double, stored in `R`); the
/// parameters are then restored.
fn sample_shifts<R: Real, const D: usize>(
    transform: &mut Composite<R, D>,
    samples: &[[f64; D]],
    delta: &[R],
) -> Result<Vec<R>, ItkException> {
    let old_parameters = transform.active().parameters();
    let to_r = |p: &[f64; D]| -> [R; D] { std::array::from_fn(|k| R::from_f64(p[k])) };
    let old: Vec<[R; D]> = samples
        .iter()
        .map(|p| transform.transform_point(&to_r(p)))
        .collect();
    transform.active_mut().update(delta, R::ONE)?;
    let shifts = samples
        .iter()
        .zip(&old)
        .map(|(p, o)| {
            let n = transform.transform_point(&to_r(p));
            let mut sum = 0.0f64;
            for k in 0..D {
                let d = n[k].to_f64() - o[k].to_f64();
                sum += d * d;
            }
            R::from_f64(sum.sqrt())
        })
        .collect();
    transform.active_mut().set_parameters(&old_parameters)?;
    Ok(shifts)
}

/// `ComputeMaximumVoxelShift(delta)`.
fn maximum_shift<R: Real, const D: usize>(
    transform: &mut Composite<R, D>,
    samples: &[[f64; D]],
    delta: &[R],
) -> Result<R, ItkException> {
    let shifts = sample_shifts(transform, samples, delta)?;
    let mut max = R::ZERO;
    for s in shifts {
        // std::max(maxShift, shift): the first argument unless the second is larger.
        if max < s {
            max = s;
        }
    }
    Ok(max)
}

/// `EstimateScales` (`RegistrationParameterScalesFromShiftBase`) with corner sampling.
pub fn estimate_scales<R: Real, const D: usize>(
    transform: &mut Composite<R, D>,
    samples: &[[f64; D]],
) -> Result<Vec<R>, ItkException> {
    let n = transform.number_of_parameters();
    let small = R::from_f64(SMALL);
    let mut scales = vec![R::ZERO; n];
    let mut min_nonzero = R::MAX;
    for i in 0..n {
        let mut delta = vec![R::ZERO; n];
        delta[i] = small;
        let shift = maximum_shift(transform, samples, &delta)?;
        scales[i] = shift;
        if shift > R::EPSILON && shift < min_nonzero {
            min_nonzero = shift;
        }
    }
    if min_nonzero == R::MAX {
        // itkWarningMacro("Variation in any parameter won't change a voxel position. ...")
        return Ok(vec![R::ONE; n]);
    }
    let factor = R::ONE / (small * small);
    for s in scales.iter_mut() {
        if *s <= R::EPSILON {
            *s = min_nonzero * min_nonzero;
        } else {
            *s = *s * *s;
        }
        *s = *s * factor;
    }
    Ok(scales)
}

/// `EstimateStepScale(step)` with corner sampling.
pub fn estimate_step_scale<R: Real, const D: usize>(
    transform: &mut Composite<R, D>,
    samples: &[[f64; D]],
    step: &mut [R],
) -> Result<R, ItkException> {
    let mut max_step = R::ZERO;
    for &s in step.iter() {
        let a = s.abs();
        if max_step < a {
            max_step = a;
        }
    }
    if max_step <= R::EPSILON {
        return Ok(R::ZERO);
    }
    let factor = R::from_f64(SMALL) / max_step;
    let small: Vec<R> = step.iter().map(|&s| s * factor).collect();
    Ok(maximum_shift(transform, samples, &small)? / factor)
}

/// `GetVirtualDomainCentralIndex`: per axis `(IndexValueType)((lower + upper) / 2.0)`.
pub fn central_index<const D: usize>(space: &Space<D>) -> [i64; D] {
    std::array::from_fn(|d| {
        let lower = space.start[d];
        let upper = space.start[d] + space.size[d] as i64 - 1;
        ((lower + upper) as f64 / 2.0) as i64
    })
}

/// `SampleVirtualDomainWithCentralRegion`: the physical points of the voxels within 5 of the
/// central index (`m_CentralRegionRadius`), x fastest.
pub fn central_region<const D: usize>(space: &Space<D>) -> Vec<[f64; D]> {
    let central = central_index(space);
    let mut lower = [0i64; D];
    let mut upper = [0i64; D];
    for d in 0..D {
        lower[d] = space.start[d];
        upper[d] = space.start[d] + space.size[d] as i64 - 1;
        if lower[d] < central[d] - 5 {
            lower[d] = central[d] - 5;
        }
        if upper[d] > central[d] + 5 {
            upper[d] = central[d] + 5;
        }
    }
    let mut points = Vec::new();
    let size: [usize; D] = std::array::from_fn(|d| (upper[d] - lower[d] + 1).max(0) as usize);
    let total: usize = size.iter().product();
    for o in 0..total {
        let mut rest = o;
        let idx: [i64; D] = std::array::from_fn(|d| {
            let i = (rest % size[d]) as i64;
            rest /= size[d];
            lower[d] + i
        });
        points.push(space.index_to_point(&idx));
    }
    points
}

/// `SampleVirtualDomainFully`: every voxel's physical point, x fastest.
pub fn full_domain<const D: usize>(space: &Space<D>) -> Vec<[f64; D]> {
    (0..space.len())
        .map(|o| space.index_to_point(&space.index_of(o)))
        .collect()
}

/// `ComputeSampleShifts` for a transform with local support: the samples mapped before and
/// after `UpdateTransformParameters(delta)` (which may smooth `delta` in place, as ITK's
/// Gaussian-smoothing field does to the caller's array), then the parameters restored.
fn local_shifts<R: Real, const D: usize>(
    transform: &mut Composite<R, D>,
    samples: &[[f64; D]],
    delta: &mut [R],
) -> Result<R, ItkException> {
    let old_parameters = transform.active().parameters();
    let to_r = |p: &[f64; D]| -> [R; D] { std::array::from_fn(|k| R::from_f64(p[k])) };
    let old: Vec<[R; D]> = samples
        .iter()
        .map(|p| transform.transform_point(&to_r(p)))
        .collect();
    transform.active_mut().update_in_place(delta, R::ONE)?;
    let mut max = R::ZERO;
    for (p, o) in samples.iter().zip(&old) {
        let n = transform.transform_point(&to_r(p));
        let mut sum = 0.0f64;
        for k in 0..D {
            let d = n[k].to_f64() - o[k].to_f64();
            sum += d * d;
        }
        let s = R::from_f64(sum.sqrt());
        if max < s {
            max = s;
        }
    }
    transform.active_mut().set_parameters(&old_parameters)?;
    Ok(max)
}

/// `EstimateScales` for a displacement field (`IsDisplacementFieldTransform`): for each of the
/// `D` local parameters, the largest shift of the `samples` when the parameter at `offset` (the
/// central voxel's, or the first point's) changes by 0.01 through
/// `UpdateTransformParameters`, squared and divided by 0.01²; zero shifts get the smallest
/// non-zero shift squared; all 1 when every shift is zero.
pub fn estimate_local_scales<R: Real, const D: usize>(
    transform: &mut Composite<R, D>,
    samples: &[[f64; D]],
    offset: usize,
) -> Result<Vec<R>, ItkException> {
    let n_all = transform.number_of_parameters();
    let small = R::from_f64(SMALL);
    let mut scales = vec![R::ZERO; D];
    let mut min_nonzero = R::MAX;
    for i in 0..D {
        let mut delta = vec![R::ZERO; n_all];
        delta[offset + i] = small;
        let shift = local_shifts(transform, samples, &mut delta)?;
        scales[i] = shift;
        if shift > R::EPSILON && shift < min_nonzero {
            min_nonzero = shift;
        }
    }
    if min_nonzero == R::MAX {
        return Ok(vec![R::ONE; D]);
    }
    let factor = R::ONE / (small * small);
    for s in scales.iter_mut() {
        if *s <= R::EPSILON {
            *s = min_nonzero * min_nonzero;
        } else {
            *s = *s * *s;
        }
        *s = *s * factor;
    }
    Ok(scales)
}

/// `EstimateStepScale(step)` for a transform with local support: the largest shift of every
/// sample (no scaling of the step), `step` possibly smoothed in place.
pub fn estimate_local_step_scale<R: Real, const D: usize>(
    transform: &mut Composite<R, D>,
    samples: &[[f64; D]],
    step: &mut [R],
) -> Result<R, ItkException> {
    local_shifts(transform, samples, step)
}

/// `EstimateMaximumStepSize`: the smallest virtual spacing.
pub fn maximum_step<R: Real, const D: usize>(space: &Space<D>) -> R {
    let mut m = R::MAX;
    for &s in &space.spacing {
        let s = R::from_f64(s);
        if s < m {
            m = s;
        }
    }
    m
}
