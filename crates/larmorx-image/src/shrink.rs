// SPDX-License-Identifier: Apache-2.0
//! ITK's `ShrinkImageFilter` (ITK v5.4.5, `itkShrinkImageFilter.hxx`): subsampling by an
//! integer factor per axis, without smoothing. N4 shrinks its input with it, and the
//! registration pyramids of antsRegistration (milestone A4) build their levels with it.
//!
//! The output grid (`GenerateOutputInformation`):
//! - spacing `spacing · factor`;
//! - size `floor(size / factor)`, at least 1;
//! - start index `ceil(start / factor)`;
//! - origin chosen so that the physical **centres** of the input and output regions coincide:
//!   `origin + (centre_in − centre_out)`, the centres being the continuous indices
//!   `start + (size − 1)/2` mapped to physical points.
//!
//! Each output voxel copies one input voxel (`DynamicThreadedGenerateData`):
//! `input index = output index · factor + offset`, where the offset comes from mapping the
//! output's first voxel to the input (`TransformPhysicalPointToIndex`, which rounds half up)
//! and is clamped at 0 per axis. Nothing is interpolated or averaged, so any pixel type works
//! and the result does not depend on the number of threads.

#![allow(clippy::needless_range_loop)]

use rayon::prelude::*;

use crate::FilterError;
use crate::grid::ItkGrid;
use crate::volume::strides;

/// What [`shrink`] does: the output grid and the index mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct ShrinkPlan {
    /// The output grid.
    pub output: ItkGrid,
    /// The shrink factor of each axis (at least 1).
    pub factors: Vec<usize>,
    /// `input index − output index · factor`, per axis (`offsetIndex`).
    pub offset: Vec<i64>,
}

/// The output grid and index mapping of `ShrinkImageFilter` with `factors` (one per axis) on
/// an image with grid `input`.
///
/// Factors below 1 become 1, as `SetShrinkFactors` does. Fails if the grid's direction or
/// spacing is singular.
pub fn shrink_plan(input: &ItkGrid, factors: &[usize]) -> Result<ShrinkPlan, FilterError> {
    let d = input.dim();
    if factors.len() != d {
        return Err(FilterError::invalid(format!(
            "{} shrink factors for a {d}-dimensional image",
            factors.len()
        )));
    }
    if input.size.contains(&0) {
        return Err(FilterError::invalid("the image is empty"));
    }
    let factors: Vec<usize> = factors.iter().map(|&f| f.max(1)).collect();
    let mut output = ItkGrid {
        size: vec![0; d],
        start: vec![0; d],
        spacing: vec![0.0; d],
        origin: input.origin.clone(),
        direction: input.direction.clone(),
    };
    for i in 0..d {
        let f = factors[i] as f64;
        output.spacing[i] = input.spacing[i] * f;
        output.size[i] = ((input.size[i] as f64 / f).floor() as usize).max(1);
        output.start[i] = (input.start[i] as f64 / f).ceil() as i64;
    }
    // Centres of the input and output regions, as continuous indices.
    let in_centre: Vec<f64> = (0..d)
        .map(|i| input.start[i] as f64 + (input.size[i] - 1) as f64 / 2.0)
        .collect();
    let out_centre: Vec<f64> = (0..d)
        .map(|i| output.start[i] as f64 + (output.size[i] - 1) as f64 / 2.0)
        .collect();
    let in_point = input.continuous_index_to_point(&in_centre);
    // The output grid's own index-to-point map: its spacing, the input's origin (not yet
    // shifted) and direction, as when ITK computes it.
    let out_point = output.continuous_index_to_point(&out_centre);
    for i in 0..d {
        output.origin[i] = input.origin[i] + (in_point[i] - out_point[i]);
    }
    // The offset between output and input indices, from the output's first voxel.
    let first = output.index_to_point(&output.start);
    let input_index = input.point_to_index(&first)?;
    let offset = (0..d)
        .map(|i| (input_index[i] - output.start[i] * factors[i] as i64).max(0))
        .collect();
    Ok(ShrinkPlan {
        output,
        factors,
        offset,
    })
}

/// Shrinks `data` (the voxels of an image with grid `input`, Fortran order) as planned by
/// [`shrink_plan`]; returns the output voxels in Fortran order.
///
/// Fails if a mapped index falls outside the input (ITK reads outside its buffer there; it
/// cannot happen for grids that [`shrink_plan`] made from `input`).
pub fn shrink<T: Copy + Send + Sync>(
    data: &[T],
    input: &ItkGrid,
    plan: &ShrinkPlan,
    n_threads: usize,
) -> Result<Vec<T>, FilterError> {
    let d = input.dim();
    if data.len() != input.len() {
        return Err(FilterError::SizeMismatch {
            what: "the image",
            actual: data.len(),
            expected: input.len(),
        });
    }
    let out = &plan.output;
    // The input coordinate (from the input's first voxel) of each output coordinate, per axis.
    let mut maps: Vec<Vec<usize>> = Vec::with_capacity(d);
    for i in 0..d {
        let mut axis = Vec::with_capacity(out.size[i]);
        for k in 0..out.size[i] {
            let index = (out.start[i] + k as i64) * plan.factors[i] as i64 + plan.offset[i];
            let rel = index - input.start[i];
            if rel < 0 || rel >= input.size[i] as i64 {
                return Err(FilterError::invalid(format!(
                    "the shrunk grid reads index {index} of axis {i}, outside the input"
                )));
            }
            axis.push(rel as usize);
        }
        maps.push(axis);
    }
    let in_strides = strides(&input.size);
    let n0 = out.size[0];
    let rows = out.len() / n0;
    let row = |r: usize, dst: &mut [T]| {
        // The input offset of this output row (all axes but the first).
        let mut rest = r;
        let mut base = 0;
        for i in 1..d {
            let k = rest % out.size[i];
            rest /= out.size[i];
            base += maps[i][k] * in_strides[i];
        }
        for (x, v) in dst.iter_mut().enumerate() {
            *v = data[base + maps[0][x]];
        }
    };
    let mut result = vec![data[0]; out.len()];
    larmorx_core::parallel::with_threads(n_threads, || {
        result
            .par_chunks_mut(n0)
            .enumerate()
            .with_min_len((1 << 14) / n0.max(1) + 1)
            .for_each(|(r, dst)| row(r, dst));
    })?;
    debug_assert_eq!(rows * n0, result.len());
    Ok(result)
}

/// Whether `plan` maps every voxel of `input` to itself (every factor 1, no offset): then
/// [`shrink`] copies the image unchanged.
pub fn is_identity(input: &ItkGrid, plan: &ShrinkPlan) -> bool {
    plan.factors.iter().all(|&f| f == 1)
        && plan.offset.iter().all(|&o| o == 0)
        && plan.output.size == input.size
        && plan.output.start == input.start
}

/// [`shrink`], or `data` itself where the plan is the identity ([`is_identity`]): the same
/// voxels without a copy (random testing, 2026-10-25: larmorx's peak memory against ANTs').
pub fn shrink_or_borrow<'a, T: Copy + Send + Sync>(
    data: &'a [T],
    input: &ItkGrid,
    plan: &ShrinkPlan,
    n_threads: usize,
) -> Result<std::borrow::Cow<'a, [T]>, FilterError> {
    if is_identity(input, plan) && data.len() == input.len() {
        Ok(std::borrow::Cow::Borrowed(data))
    } else {
        Ok(std::borrow::Cow::Owned(shrink(
            data, input, plan, n_threads,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(d: usize) -> Vec<Vec<f64>> {
        (0..d)
            .map(|i| (0..d).map(|j| f64::from(u8::from(i == j))).collect())
            .collect()
    }

    #[test]
    fn grid_and_values() {
        let input = ItkGrid::new(&[7, 4], &[1.0, 2.0], &[0.0, 0.0], &identity(2));
        let plan = shrink_plan(&input, &[2, 2]).unwrap();
        assert_eq!(plan.output.size, [3, 2]);
        assert_eq!(plan.output.spacing, [2.0, 4.0]);
        // Centres: input (3, 1.5)·spacing = (3, 3); output (1, 0.5)·(2, 4) = (2, 2).
        assert_eq!(plan.output.origin, [1.0, 1.0]);
        assert_eq!(plan.offset, [1, 1]);
        let data: Vec<u16> = (0..28).collect();
        let out = shrink(&data, &input, &plan, 2).unwrap();
        // Input x = 1, 3, 5 on rows y = 1, 3.
        assert_eq!(out, [8, 10, 12, 22, 24, 26]);
        // Thread count does not matter.
        assert_eq!(shrink(&data, &input, &plan, 1).unwrap(), out);
    }

    #[test]
    fn padded_regions_and_factor_one() {
        let mut input = ItkGrid::new(&[10, 3, 3], &[1.0; 3], &[5.0, 0.0, 0.0], &identity(3));
        input.start = vec![-3, 0, 0];
        let plan = shrink_plan(&input, &[4, 1, 0]).unwrap();
        assert_eq!(plan.factors, [4, 1, 1]);
        assert_eq!(plan.output.start, [0, 0, 0]);
        assert_eq!(plan.output.size, [2, 3, 3]);
        let data: Vec<f32> = (0..90).map(|v| v as f32).collect();
        let out = shrink(&data, &input, &plan, 1).unwrap();
        assert_eq!(out.len(), 18);
        let same = shrink_plan(&input, &[1, 1, 1]).unwrap();
        assert_eq!(same.output.origin, input.origin);
        assert_eq!(shrink(&data, &input, &same, 3).unwrap(), data);
        // The identity is not copied.
        assert!(is_identity(&input, &same) && !is_identity(&input, &plan));
        let borrowed = shrink_or_borrow(&data, &input, &same, 1).unwrap();
        assert!(matches!(borrowed, std::borrow::Cow::Borrowed(_)));
        assert_eq!(*borrowed, *data);
        assert_eq!(*shrink_or_borrow(&data, &input, &plan, 1).unwrap(), *out);
    }
}
