// SPDX-License-Identifier: Apache-2.0
//! `ANTSNeighborhoodCorrelationImageToImageMetricv4` (ITK v5.4.5,
//! `itkANTSNeighborhoodCorrelationImageToImageMetricv4GetValueAndDerivativeThreader.hxx`):
//! ANTs' `CC`.
//!
//! For each virtual voxel, the fixed and moving values of the valid voxels of the window of
//! radius `r` around it give a local correlation `sFM² / (sFF·sMM)`; the value is minus their
//! mean over the voxels whose window holds a valid voxel. ITK keeps one sum per window column
//! (the voxels with the same x offset) in a queue: at the start of each line (and for each
//! sampled point) the columns are computed afresh, `r` columns of zeros standing for those left
//! of the region; moving one voxel along x pushes the new right column and drops the left one.
//! The window's sums are the queue's, added front to back, every sum in `R`. A column adds
//! its voxels inside the virtual region in the window's order (y, then z), each point mapped
//! and interpolated as for any metric.
//!
//! The derivative at a voxel is `2·sFM/(sFF·sMM)·(f − sFM/sMM·m)·∇m` (in double, stored in
//! `R`) projected on the transform's Jacobian, with `f` and `m` the voxel's values less the
//! window's means; zero where `sFF` or `sMM` is at most `ε`.
//!
//! Dense evaluation visits the region line by line; each voxel's mapped values are computed
//! once (in parallel) and the windows built from them, which gives ITK's sums exactly.

use larmorx_core::IndexFloor;
use rayon::prelude::*;

use super::mean_squares::{BaseReduction, Reduced};
use super::{GradientSource, ImageMetricv4};
use crate::transform::Composite;
use crate::{ItkException, Real};

/// The six per-column sums of the queue.
#[derive(Clone, Copy, Debug, Default)]
struct Column<R> {
    fixed2: R,
    moving2: R,
    fixed: R,
    moving: R,
    fixed_moving: R,
    count: R,
}

/// One voxel's mapped values, or `None` where the point is not valid.
type Values<R> = Option<(R, R)>;

/// A voxel's local correlation and derivative, or `None` where it does not count.
/// A valid point's local CC, derivative, voxel, derivative columns, and whether its derivative
/// used the moving transform's Jacobian (`sFixedFixed` and `sMovingMoving` above epsilon).
type PointResult<R> = Option<(R, Vec<R>, Option<usize>, Option<Vec<usize>>, bool)>;

/// The metric's state for a level: its radius.
#[derive(Clone, Copy, Debug)]
pub struct NeighborhoodCorrelation {
    pub radius: usize,
}

impl NeighborhoodCorrelation {
    /// A column of the window centred at `center` at x offset `dx`, from the voxels' values
    /// (`value(index)`; `None` outside the region or not valid).
    fn column<R: Real, const D: usize>(
        &self,
        center: &[i64; D],
        dx: i64,
        value: &impl Fn(&[i64; D]) -> Values<R>,
    ) -> Column<R> {
        let r = self.radius as i64;
        let w = 2 * r + 1;
        let mut c = Column {
            fixed2: R::ZERO,
            moving2: R::ZERO,
            fixed: R::ZERO,
            moving: R::ZERO,
            fixed_moving: R::ZERO,
            count: R::ZERO,
        };
        // The other axes' offsets in the window's order, the first of them fastest.
        let n_rest = (w as usize).pow((D - 1) as u32);
        for k in 0..n_rest {
            let mut index = *center;
            index[0] += dx;
            let mut rest = k;
            for axis in 1..D {
                let o = (rest % w as usize) as i64 - r;
                rest /= w as usize;
                index[axis] += o;
            }
            if let Some((f, m)) = value(&index) {
                c.fixed2 = c.fixed2 + f * f;
                c.moving2 = c.moving2 + m * m;
                c.fixed = c.fixed + f;
                c.moving = c.moving + m;
                c.fixed_moving = c.fixed_moving + f * m;
                c.count = c.count + R::ONE;
            }
        }
        c
    }

    /// `ComputeInformationFromQueues` and `ComputeMovingTransformDerivative` for the voxel
    /// `center` whose window's columns are `queue`: `None` if the window holds no valid voxel
    /// or the voxel itself is not valid, else `(localCC, local derivative)`.
    #[allow(clippy::too_many_arguments)]
    fn point<R: Real, const D: usize>(
        &self,
        m: &ImageMetricv4<R, D>,
        transform: &Composite<R, D>,
        queue: &[Column<R>],
        center: &[i64; D],
        compute_derivative: bool,
        floor: &IndexFloor,
        np: usize,
    ) -> PointResult<R> {
        let mut count = R::ZERO;
        for c in queue {
            count = count + c.count;
        }
        if count <= R::ZERO {
            return None;
        }
        let (mut sf2, mut sm2, mut sf, mut sm, mut sfm) =
            (R::ZERO, R::ZERO, R::ZERO, R::ZERO, R::ZERO);
        for c in queue {
            sf2 = sf2 + c.fixed2;
            sm2 = sm2 + c.moving2;
            sf = sf + c.fixed;
            sm = sm + c.moving;
            sfm = sfm + c.fixed_moving;
        }
        let fixed_mean = sf / count;
        let moving_mean = sm / count;
        let s_ff = sf2 - fixed_mean * sf - fixed_mean * sf + count * fixed_mean * fixed_mean;
        let s_mm = sm2 - moving_mean * sm - moving_mean * sm + count * moving_mean * moving_mean;
        let s_fm = sfm - moving_mean * sf - fixed_mean * sm + count * moving_mean * fixed_mean;
        let p = m.virtual_space.index_to_point(center);
        let gradients = GradientSource {
            fixed: false,
            moving: true,
        };
        let s = m.sample_at(center, &p, transform, gradients, compute_derivative, floor)?;
        let fixed_a = s.fixed_value - fixed_mean;
        let moving_a = s.moving_value - moving_mean;
        let eps = R::EPSILON;
        let ff_mm = s_ff * s_mm;
        let local_cc = if ff_mm.abs() > eps {
            s_fm * s_fm / ff_mm
        } else {
            R::ONE
        };
        let mut derivative = Vec::new();
        let mut cols = None;
        let mut jacobian_used = false;
        if compute_derivative {
            // A B-spline transform's derivative is zero away from the point's control points.
            let sparse = !m.local_support && transform.active().is_bspline();
            if sparse {
                cols = Some(Vec::new());
            } else {
                derivative = vec![R::ZERO; np];
            }
            if s_ff > eps && s_mm > eps {
                let factor = 2.0 * s_fm.to_f64() / ff_mm.to_f64()
                    * (fixed_a - s_fm / s_mm * moving_a).to_f64();
                let wrt_image: [R; D] =
                    std::array::from_fn(|q| R::from_f64(factor * s.moving_gradient[q].to_f64()));
                let vp: [R; D] = std::array::from_fn(|k| R::from_f64(p[k]));
                let jac = m.jacobian_columns(transform, &vp);
                jacobian_used = true;
                derivative = (0..jac.n)
                    .map(|par| {
                        let mut v = R::ZERO;
                        for dim in 0..D {
                            v = v + wrt_image[dim] * jac.at(dim, par);
                        }
                        v
                    })
                    .collect();
                cols = jac.cols;
            }
        }
        let voxel = if m.local_support {
            m.voxel_of(center)
        } else {
            None
        };
        Some((local_cc, derivative, voxel, cols, jacobian_used))
    }

    /// `GetValue` (`compute_derivative` false) or `GetValueAndDerivative`.
    pub fn evaluate<R: Real, const D: usize>(
        &self,
        m: &ImageMetricv4<R, D>,
        transform: &Composite<R, D>,
        compute_derivative: bool,
        floor: &IndexFloor,
    ) -> Result<Reduced<R>, ItkException> {
        let np = m.local_parameters(transform);
        let space = &m.virtual_space;
        let none = GradientSource {
            fixed: false,
            moving: false,
        };
        let r = self.radius as i64;
        let mut reduction = BaseReduction::for_metric(m, transform, compute_derivative);
        let mut jacobian_used = false;
        match &m.samples {
            None => {
                // Every voxel's values once.
                let values: Vec<Values<R>> = (0..space.len())
                    .into_par_iter()
                    .map(|o| {
                        let idx = space.index_of(o);
                        let p = space.index_to_point(&idx);
                        m.sample_at(&idx, &p, transform, none, false, floor)
                            .map(|s| (s.fixed_value, s.moving_value))
                    })
                    .collect();
                let value = |idx: &[i64; D]| -> Values<R> {
                    if space.contains(idx) {
                        values[space.offset(idx)]
                    } else {
                        None
                    }
                };
                let line_len = space.size[0];
                let n_lines = space.len() / line_len.max(1);
                let lines: Vec<Vec<PointResult<R>>> = (0..n_lines)
                    .into_par_iter()
                    .map(|line| {
                        let first = space.index_of(line * line_len);
                        let mut queue: std::collections::VecDeque<Column<R>> =
                            std::collections::VecDeque::with_capacity(2 * self.radius + 1);
                        let mut out = Vec::with_capacity(line_len);
                        for x in 0..line_len {
                            let mut center = first;
                            center[0] += x as i64;
                            if x == 0 {
                                queue.clear();
                                for _ in 0..self.radius {
                                    queue.push_back(Column::default_zero());
                                }
                                for dx in 0..=r {
                                    queue.push_back(self.column(&center, dx, &value));
                                }
                            } else {
                                queue.push_back(self.column(&center, r, &value));
                                queue.pop_front();
                            }
                            let q: Vec<Column<R>> = queue.iter().copied().collect();
                            out.push(self.point(
                                m,
                                transform,
                                &q,
                                &center,
                                compute_derivative,
                                floor,
                                np,
                            ));
                        }
                        out
                    })
                    .collect();
                for line in lines {
                    for (cc, d, voxel, cols, used) in line.into_iter().flatten() {
                        jacobian_used |= used;
                        reduction.add_point_cols(-cc, &d, cols.as_deref(), voxel);
                    }
                }
            }
            Some(samples) => {
                let start0 = space.start[0];
                let results: Vec<PointResult<R>> = samples
                    .par_iter()
                    .map(|p| {
                        let center = space.point_to_index(p);
                        let value = |idx: &[i64; D]| -> Values<R> {
                            if !space.contains(idx) {
                                return None;
                            }
                            let q = space.index_to_point(idx);
                            m.sample_at(idx, &q, transform, none, false, floor)
                                .map(|s| (s.fixed_value, s.moving_value))
                        };
                        let fill = (start0 - (center[0] - r)).max(0);
                        let mut queue = Vec::with_capacity(2 * self.radius + 1);
                        for _ in 0..fill {
                            queue.push(Column::default_zero());
                        }
                        for i in fill..=2 * r {
                            queue.push(self.column(&center, i - r, &value));
                        }
                        self.point(m, transform, &queue, &center, compute_derivative, floor, np)
                    })
                    .collect();
                for (cc, d, voxel, cols, used) in results.into_iter().flatten() {
                    jacobian_used |= used;
                    reduction.add_point_cols(-cc, &d, cols.as_deref(), voxel);
                }
            }
        }
        let mut r = reduction.finish(compute_derivative, np);
        // ComputeMovingTransformDerivative returns before the transform's Jacobian when a
        // neighbourhood is flat: a transform without a Jacobian with respect to position (a
        // B-spline after the active one, ITK-35) throws only if some point got that far
        // (found by differential random testing, 2026-10-25).
        r.jacobian_used = jacobian_used;
        Ok(r)
    }
}

impl<R: Real> Column<R> {
    fn default_zero() -> Self {
        Column {
            fixed2: R::ZERO,
            moving2: R::ZERO,
            fixed: R::ZERO,
            moving: R::ZERO,
            fixed_moving: R::ZERO,
            count: R::ZERO,
        }
    }
}
