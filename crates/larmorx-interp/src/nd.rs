// SPDX-License-Identifier: Apache-2.0
//! The B-spline, Gaussian, label-Gaussian and windowed-sinc interpolators in any dimension
//! `D` (ITK v5.4.5's classes are templated on the dimension): the same computations as their
//! 3D versions ([`crate::Interpolator`]), with every per-axis step done for `D` axes and the
//! neighbourhood visited with x fastest, the per-axis weights multiplied in axis order.
//!
//! The linear, nearest-neighbour and generic-label interpolators are not here: callers in `D`
//! dimensions have ITK's linear interpolator with their own coordinate type.

use rayon::prelude::*;

use super::{SINC_RADIUS, Window, bspline, erf_weights, sinc, window};
use larmorx_core::{IndexFloor, ItkReal, Precision, RealElement};

/// The Gaussian interpolators' per-axis factors (`ComputeBoundingBox`): the scaling
/// `1/(√2·σ/spacing)` and the cut-off `σ·α/spacing`, in voxels.
struct GaussianWeightsNd<const D: usize> {
    scaling: [f64; D],
    cutoff: [f64; D],
}

impl<const D: usize> GaussianWeightsNd<D> {
    fn new(sigma: [f64; D], alpha: f64, spacing: [f64; D]) -> Result<Self, super::InterpError> {
        if alpha <= 0.0 || sigma.iter().any(|&s| s <= 0.0) {
            return Err(super::InterpError::GaussianParameters);
        }
        Ok(GaussianWeightsNd {
            scaling: std::array::from_fn(|d| {
                1.0 / (std::f64::consts::SQRT_2 * sigma[d] / spacing[d])
            }),
            cutoff: std::array::from_fn(|d| sigma[d] * alpha / spacing[d]),
        })
    }
}

/// An interpolation method in `D` dimensions, with ITK's parameters.
#[derive(Clone, Debug, PartialEq)]
pub enum InterpolationNd<const D: usize> {
    /// Order 0 to 5.
    BSpline { order: u32 },
    /// `BSplineResampleImageFunction`: the B-spline of order 0 to 5 whose coefficients are the
    /// image's voxels (no decomposition).
    BSplineCoefficients { order: u32 },
    /// `sigma` in physical units, `alpha` the cut-off in sigmas.
    Gaussian { sigma: [f64; D], alpha: f64 },
    /// Label-wise Gaussian voting.
    MultiLabel { sigma: [f64; D], alpha: f64 },
    /// Radius 3, zero outside the image.
    WindowedSinc(Window),
    /// Radius 3, the nearest edge voxel outside the image.
    WindowedSincEdge(Window),
}

enum Kind<const D: usize> {
    BSpline { order: u32, coefficients: Vec<f64> },
    Gaussian(GaussianWeightsNd<D>),
    MultiLabel(GaussianWeightsNd<D>),
    WindowedSinc(Window, bool),
}

/// An interpolator prepared for one `D`-dimensional image (x fastest).
pub struct InterpolatorNd<'a, T, const D: usize> {
    data: &'a [T],
    size: [usize; D],
    strides: [usize; D],
    kind: Kind<D>,
    precision: Precision,
}

impl<'a, T: RealElement, const D: usize> InterpolatorNd<'a, T, D> {
    /// Prepares `method` for the image `data` of `size` and `spacing` (for the Gaussian ones),
    /// with ITK's coordinate type `precision` (B-spline coefficients computed here, in parallel
    /// on the current rayon pool, each line independently).
    pub fn with_precision(
        data: &'a [T],
        size: [usize; D],
        method: &InterpolationNd<D>,
        spacing: [f64; D],
        precision: Precision,
    ) -> Result<Self, super::InterpError> {
        assert_eq!(
            data.len(),
            size.iter().product::<usize>(),
            "image size mismatch"
        );
        let mut strides = [1usize; D];
        for d in 1..D {
            strides[d] = strides[d - 1] * size[d - 1];
        }
        let kind = match method {
            InterpolationNd::BSpline { order } => {
                if *order > 5 {
                    return Err(super::InterpError::BSplineOrder(*order));
                }
                Kind::BSpline {
                    order: *order,
                    coefficients: coefficients(data, size, strides, *order),
                }
            }
            InterpolationNd::BSplineCoefficients { order } => {
                if *order > 5 {
                    return Err(super::InterpError::BSplineOrder(*order));
                }
                Kind::BSpline {
                    order: *order,
                    coefficients: data.iter().map(|x| x.to_f64()).collect(),
                }
            }
            InterpolationNd::Gaussian { sigma, alpha } => {
                Kind::Gaussian(GaussianWeightsNd::new(*sigma, *alpha, spacing)?)
            }
            InterpolationNd::MultiLabel { sigma, alpha } => {
                Kind::MultiLabel(GaussianWeightsNd::new(*sigma, *alpha, spacing)?)
            }
            InterpolationNd::WindowedSinc(w) => Kind::WindowedSinc(*w, false),
            InterpolationNd::WindowedSincEdge(w) => Kind::WindowedSinc(*w, true),
        };
        Ok(InterpolatorNd {
            data,
            size,
            strides,
            kind,
            precision,
        })
    }

    /// The value at a continuous index inside the image (within half a voxel of the grid),
    /// the windowed sinc's base index floored by `floor` (ITK-01).
    pub fn evaluate(&self, cidx: [f64; D], floor: &IndexFloor) -> f64 {
        match self.precision {
            Precision::Double => self.evaluate_as::<f64>(cidx, floor),
            Precision::Float => self.evaluate_as::<f32>(cidx, floor),
        }
    }

    fn evaluate_as<R: ItkReal>(&self, cidx: [f64; D], floor: &IndexFloor) -> f64 {
        match &self.kind {
            Kind::BSpline {
                order,
                coefficients,
            } => bspline_evaluate(coefficients, self.size, self.strides, *order, cidx),
            Kind::Gaussian(p) => self.gaussian::<R>(p, cidx),
            Kind::MultiLabel(p) => self.multi_label::<R>(p, cidx),
            Kind::WindowedSinc(w, edge) => self.windowed_sinc::<R>(*w, *edge, cidx, floor),
        }
    }

    /// The Gaussian region's weights per axis, and the visit of its voxels with x fastest:
    /// `f(value, weight)` with the weight `w0·w1·…` in axis order.
    fn gaussian_visit<R: ItkReal>(
        &self,
        p: &GaussianWeightsNd<D>,
        cidx: [f64; D],
        mut f: impl FnMut(f64, f64),
    ) {
        let weights: [_; D] = std::array::from_fn(|d| {
            erf_weights::<R>(cidx[d], self.size[d], p.scaling[d], p.cutoff[d])
        });
        let lens: [usize; D] = std::array::from_fn(|d| weights[d].as_slice().len());
        if lens.contains(&0) {
            return;
        }
        let mut k = [0usize; D];
        loop {
            let mut offset = 0usize;
            let mut w = 1.0f64;
            for d in 0..D {
                offset += (weights[d].begin + k[d]) * self.strides[d];
                let wd = weights[d].as_slice()[k[d]];
                w = if d == 0 { wd } else { w * wd };
            }
            f(self.data[offset].to_f64(), w);
            if !advance(&mut k, &lens) {
                break;
            }
        }
    }

    fn gaussian<R: ItkReal>(&self, p: &GaussianWeightsNd<D>, cidx: [f64; D]) -> f64 {
        let (mut sum_me, mut sum_m) = (0.0f64, 0.0f64);
        self.gaussian_visit::<R>(p, cidx, |value, w| {
            sum_me += value * w;
            sum_m += w;
        });
        sum_me / sum_m
    }

    /// Label voting: the label whose summed weight first exceeds the running maximum, in
    /// scan order, wins (0 if none does).
    fn multi_label<R: ItkReal>(&self, p: &GaussianWeightsNd<D>, cidx: [f64; D]) -> f64 {
        let mut labels: Vec<(f64, f64)> = Vec::new();
        let (mut wmax, mut vmax) = (0.0f64, 0.0f64);
        self.gaussian_visit::<R>(p, cidx, |value, w| {
            let total = match labels.iter_mut().find(|(l, _)| *l == value) {
                Some((_, acc)) => {
                    *acc += w;
                    *acc
                }
                None => {
                    labels.push((value, w));
                    w
                }
            };
            if total > wmax {
                wmax = total;
                vmax = value;
            }
        });
        vmax
    }

    /// ITK's windowed sinc of radius 3 (see the 3D version): 6 taps per axis around
    /// `Math::Floor(index)`, a delta on a voxel, each neighbour's value multiplied by the
    /// axes' weights in order and summed with x fastest.
    fn windowed_sinc<R: ItkReal>(
        &self,
        w: Window,
        edge: bool,
        cidx: [f64; D],
        floor: &IndexFloor,
    ) -> f64 {
        let taps = (2 * SINC_RADIUS) as usize;
        let mut base = [0i64; D];
        let mut weights = [[0.0f64; 6]; D];
        for d in 0..D {
            let c = R::from_f64(cidx[d]);
            base[d] = floor.pick(c.itk_floor(), c.to_f64());
            let dist = cidx[d] - base[d] as f64;
            if dist == 0.0 {
                for (i, wt) in weights[d].iter_mut().enumerate() {
                    *wt = if i as i64 == SINC_RADIUS - 1 {
                        1.0
                    } else {
                        0.0
                    };
                }
            } else {
                let mut x = dist + SINC_RADIUS as f64;
                for wt in weights[d].iter_mut().take(taps) {
                    x -= 1.0;
                    let windowed = R::from_f64(window(w, R::from_f64(x).to_f64())).to_f64();
                    *wt = windowed * sinc(x);
                }
            }
        }
        let lens = [taps; D];
        let mut k = [0usize; D];
        let mut sum = 0.0f64;
        loop {
            let mut inside = true;
            let mut offset = 0usize;
            for d in 0..D {
                let mut i = base[d] + k[d] as i64 - (SINC_RADIUS - 1);
                if edge {
                    i = i.clamp(0, self.size[d] as i64 - 1);
                }
                if i < 0 || i >= self.size[d] as i64 {
                    inside = false;
                } else {
                    offset += i as usize * self.strides[d];
                }
            }
            let mut value = if inside {
                self.data[offset].to_f64()
            } else {
                0.0
            };
            for d in 0..D {
                value *= weights[d][k[d]];
            }
            sum += value;
            if !advance(&mut k, &lens) {
                break;
            }
        }
        sum
    }
}

/// The next multi-index in scan order (x fastest); `false` after the last.
fn advance<const D: usize>(k: &mut [usize; D], lens: &[usize; D]) -> bool {
    for d in 0..D {
        k[d] += 1;
        if k[d] < lens[d] {
            return true;
        }
        k[d] = 0;
    }
    false
}

/// `BSplineDecompositionImageFilter`: the 1D filter along each axis in turn, each line
/// independently.
fn coefficients<T: RealElement, const D: usize>(
    data: &[T],
    size: [usize; D],
    strides: [usize; D],
    order: u32,
) -> Vec<f64> {
    let mut c: Vec<f64> = data.iter().map(|x| x.to_f64()).collect();
    let poles = bspline::poles(order);
    if poles.is_empty() {
        return c;
    }
    for d in 0..D {
        let n = size[d];
        let stride = strides[d];
        // The lines along axis d start at every index whose d-th component is 0.
        let starts: Vec<usize> = (0..c.len())
            .filter(|&o| (o / stride).is_multiple_of(n))
            .collect();
        let lines: Vec<Vec<f64>> = starts
            .par_iter()
            .map(|&s| {
                let mut line: Vec<f64> = (0..n).map(|i| c[s + i * stride]).collect();
                bspline::line_to_coefficients(&mut line, &poles);
                line
            })
            .collect();
        for (&s, line) in starts.iter().zip(lines) {
            for (i, v) in line.into_iter().enumerate() {
                c[s + i * stride] = v;
            }
        }
    }
    c
}

/// ITK's B-spline evaluation (see the 3D version): the support from
/// `floor(float(x) + ½·even)`, the weights per axis, mirror boundaries, the sum over the
/// support with x fastest, each weight the axes' weights multiplied in order.
fn bspline_evaluate<const D: usize>(
    coefficients: &[f64],
    size: [usize; D],
    strides: [usize; D],
    order: u32,
    cidx: [f64; D],
) -> f64 {
    let n = order as usize + 1;
    let half: f32 = if order & 1 == 1 { 0.0 } else { 0.5 };
    let mut index = [[0usize; 6]; D];
    let mut w = [[0.0f64; 6]; D];
    for d in 0..D {
        let first = ((cidx[d] as f32) + half).floor() as i64 - i64::from(order / 2);
        w[d] = bspline::weights(order, cidx[d], first);
        let end = size[d] as i64 - 1;
        for (k, slot) in index[d].iter_mut().take(n).enumerate() {
            let mut i = first + k as i64;
            if size[d] == 1 {
                i = 0;
            } else {
                if i < 0 {
                    i = -i;
                }
                if i >= end {
                    i = end - (i - end);
                }
            }
            *slot = i as usize;
        }
    }
    let lens = [n; D];
    let mut k = [0usize; D];
    let mut sum = 0.0f64;
    loop {
        let mut weight = w[0][k[0]];
        let mut offset = index[0][k[0]] * strides[0];
        for d in 1..D {
            weight *= w[d][k[d]];
            offset += index[d][k[d]] * strides[d];
        }
        sum += weight * coefficients[offset];
        if !advance(&mut k, &lens) {
            break;
        }
    }
    sum
}

/// `BSplineDecompositionImageFilter`: the B-spline coefficients of order `order` of an image of
/// `size` (x fastest), as [`InterpolationNd::BSpline`] computes them, for callers that keep
/// the coefficients and evaluate them with [`bspline_at`].
pub fn bspline_coefficients<T: RealElement, const D: usize>(
    data: &[T],
    size: [usize; D],
    order: u32,
) -> Vec<f64> {
    let mut strides = [1usize; D];
    for d in 1..D {
        strides[d] = strides[d - 1] * size[d - 1];
    }
    coefficients(data, size, strides, order)
}

/// `BSplineInterpolateImageFunction::EvaluateAtContinuousIndex` on coefficients from
/// [`bspline_coefficients`].
pub fn bspline_at<const D: usize>(
    coefficients: &[f64],
    size: [usize; D],
    order: u32,
    cidx: [f64; D],
) -> f64 {
    let mut strides = [1usize; D];
    for d in 1..D {
        strides[d] = strides[d - 1] * size[d - 1];
    }
    bspline_evaluate(coefficients, size, strides, order, cidx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Interpolation, Interpolator, Volume};

    /// In 3D the generic code computes what the 3D interpolators compute, bit for bit.
    #[test]
    fn three_dimensions_match_the_3d_interpolators() {
        let size = [7usize, 6, 5];
        let data: Vec<f64> = (0..size.iter().product::<usize>())
            .map(|i| ((i * 37) % 101) as f64 * 0.25 - 3.0)
            .collect();
        let spacing = [1.0, 1.5, 0.75];
        let points = [
            [1.25, 2.5, 3.75],
            [0.0, 0.0, 0.0],
            [5.9, 4.6, 3.6],
            [-0.4, 2.0, 1.1],
        ];
        let cases: Vec<(Interpolation, InterpolationNd<3>)> = vec![
            (
                Interpolation::BSpline { order: 3 },
                InterpolationNd::BSpline { order: 3 },
            ),
            (
                Interpolation::BSpline { order: 4 },
                InterpolationNd::BSpline { order: 4 },
            ),
            (
                Interpolation::Gaussian {
                    sigma: [1.0, 1.2, 0.9],
                    alpha: 1.5,
                },
                InterpolationNd::Gaussian {
                    sigma: [1.0, 1.2, 0.9],
                    alpha: 1.5,
                },
            ),
            (
                Interpolation::MultiLabel {
                    sigma: [1.0, 1.2, 0.9],
                    alpha: 4.0,
                },
                InterpolationNd::MultiLabel {
                    sigma: [1.0, 1.2, 0.9],
                    alpha: 4.0,
                },
            ),
            (
                Interpolation::WindowedSinc(Window::Lanczos),
                InterpolationNd::WindowedSinc(Window::Lanczos),
            ),
            (
                Interpolation::WindowedSincEdge(Window::Hamming),
                InterpolationNd::WindowedSincEdge(Window::Hamming),
            ),
        ];
        let floor = IndexFloor::itk();
        for precision in [Precision::Double, Precision::Float] {
            for (a, b) in &cases {
                let i3 =
                    Interpolator::with_precision(Volume::new(&data, size), a, spacing, precision)
                        .unwrap();
                let nd =
                    InterpolatorNd::with_precision(&data, size, b, spacing, precision).unwrap();
                for p in points {
                    let p = match precision {
                        Precision::Double => p,
                        Precision::Float => p.map(|v| f64::from(v as f32)),
                    };
                    assert_eq!(
                        i3.evaluate(p).to_bits(),
                        nd.evaluate(p, &floor).to_bits(),
                        "{a:?} {precision:?} {p:?}"
                    );
                }
            }
        }
    }
}
