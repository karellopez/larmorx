// SPDX-License-Identifier: Apache-2.0
//! `LinearInterpolateImageFunction<Image<R, D>, R>` and
//! `CentralDifferenceImageFunction<Image<R, D>, R>` (ITK v5.4.5,
//! `Modules/Core/ImageFunction/include/itkLinearInterpolateImageFunction.h/.hxx`,
//! `itkCentralDifferenceImageFunction.hxx`), the metrics' defaults.
//!
//! The interpolator's coordinate type is the computation type `R`: a physical point becomes a
//! continuous index of `R` ([`Space::point_to_cindex`]); each axis's distance to the voxel
//! below is computed in `R`; the values are combined in double (`RealType`). In 1, 2 and 3
//! dimensions ITK uses its optimised paths (successive linear interpolations along x, y, z,
//! skipping an axis whose distance is not positive or whose upper neighbour is outside), in
//! 4 dimensions the general sum over the 16 neighbours (`EvaluateUnoptimized`).

use larmorx_core::IndexFloor;

use crate::{Image, Real};

/// The linear interpolator of an image.
#[derive(Clone, Copy, Debug)]
pub struct Linear<'a, R, const D: usize> {
    pub image: &'a Image<R, D>,
}

impl<'a, R: Real, const D: usize> Linear<'a, R, D> {
    pub fn new(image: &'a Image<R, D>) -> Self {
        Linear { image }
    }

    /// `IsInsideBuffer(point)`.
    #[inline]
    pub fn is_inside(&self, p: &[f64; D]) -> bool {
        let c: [R; D] = self.image.space.point_to_cindex(p);
        self.image.space.is_inside_buffer(&c)
    }

    /// `Evaluate(point)`: the continuous index, then [`Linear::at_cindex`].
    #[inline]
    pub fn evaluate(&self, p: &[f64; D], floor: &IndexFloor) -> f64 {
        let c: [R; D] = self.image.space.point_to_cindex(p);
        self.at_cindex(&c, floor)
    }

    /// `EvaluateAtContinuousIndex`.
    #[inline]
    pub fn at_cindex(&self, c: &[R; D], floor: &IndexFloor) -> f64 {
        let data = &self.image.data;
        at_cindex_with(&self.image.space, c, floor, |o| data[o].to_f64())
    }
}

/// `LinearInterpolateImageFunction::EvaluateAtContinuousIndex` on a grid, the values read by
/// `value(offset)` (in double), the coordinates of type `R`: in 1 to 3 dimensions ITK's
/// optimised paths, in 4 the general sum.
#[inline]
pub fn at_cindex_with<R: Real, const D: usize>(
    space: &crate::Space<D>,
    c: &[R; D],
    floor: &IndexFloor,
    value: impl Fn(usize) -> f64,
) -> f64 {
    match D {
        1..=3 => optimized(space, c, value),
        _ => unoptimized(space, c, floor, value),
    }
}

/// `EvaluateOptimized` (1 to 3 dimensions): the base index `max(floor(c), start)`, the
/// distance `c − base` in `R`; then linear interpolation along x, then y, then z, each
/// `a + (b − a)·d` in double, an axis skipped when its distance is ≤ 0 or its upper
/// neighbour is past the end.
#[inline]
fn optimized<R: Real, const D: usize>(
    space: &crate::Space<D>,
    c: &[R; D],
    v: impl Fn(usize) -> f64,
) -> f64 {
    Cell::new(space, c).eval(v)
}

/// The cell of a continuous index that `EvaluateOptimized` (1 to 3 dimensions) interpolates
/// in: its first voxel's offset, the step to the upper neighbour along each axis (0 when the
/// axis is skipped) and each axis's distance. One cell serves every image on the same grid
/// (a value and its gradient's components), each [`Cell::eval`] the interpolation
/// [`at_cindex_with`] computes, operation for operation.
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    origin: usize,
    step: [usize; 3],
    dist: [f64; 3],
    dims: usize,
}

impl Cell {
    /// The cell of `c` (1 to 3 dimensions).
    #[inline]
    pub fn new<R: Real, const D: usize>(space: &crate::Space<D>, c: &[R; D]) -> Cell {
        debug_assert!((1..=3).contains(&D));
        let mut base = [0usize; 3];
        let mut dist = [0.0f64; 3];
        let mut step = [0usize; 3];
        let mut stride = 1usize;
        for k in 0..D {
            let start = space.start[k];
            let b = c[k].itk_floor().max(start);
            dist[k] = (c[k] - R::from_i64(b)).to_f64();
            let rel = (b - start) as usize;
            base[k] = rel;
            // The upper neighbour along the axis, or none.
            step[k] = if dist[k] > 0.0 && rel + 1 < space.size[k] {
                stride
            } else {
                0
            };
            stride *= space.size[k];
        }
        let mut origin = 0usize;
        let mut s = 1usize;
        for k in 0..D {
            origin += base[k] * s;
            s *= space.size[k];
        }
        Cell {
            origin,
            step,
            dist,
            dims: D,
        }
    }

    /// The interpolated value of the image whose voxel at offset `o` is `v(o)`.
    #[inline]
    pub fn eval(&self, v: impl Fn(usize) -> f64) -> f64 {
        let (step, dist, origin) = (&self.step, &self.dist, self.origin);
        let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
        let along_x = |o: usize| {
            let a = v(o);
            if step[0] != 0 {
                lerp(a, v(o + step[0]), dist[0])
            } else {
                a
            }
        };
        if self.dims == 1 {
            return along_x(origin);
        }
        let along_y = |o: usize| {
            let a = along_x(o);
            if step[1] != 0 {
                lerp(a, along_x(o + step[1]), dist[1])
            } else {
                a
            }
        };
        if self.dims == 2 {
            return along_y(origin);
        }
        let a = along_y(origin);
        if step[2] != 0 {
            lerp(a, along_y(origin + step[2]), dist[2])
        } else {
            a
        }
    }
}

/// `EvaluateUnoptimized`: the sum over the `2^D` neighbours of `value · overlap`, each
/// overlap a product in `R` of `distance` or `1.0 − distance` (the latter computed in
/// double and stored in `R`), the neighbours clamped to the region. The floor is ITK's
/// or the exact one (ITK-01, chosen by `floor`).
fn unoptimized<R: Real, const D: usize>(
    space: &crate::Space<D>,
    c: &[R; D],
    floor: &IndexFloor,
    v: impl Fn(usize) -> f64,
) -> f64 {
    let mut base = [0i64; D];
    let mut dist = [R::ZERO; D];
    for k in 0..D {
        base[k] = floor.pick(c[k].itk_floor(), c[k].to_f64());
        dist[k] = c[k] - R::from_i64(base[k]);
    }
    let strides = space.strides();
    let mut value = 0.0f64;
    for counter in 0..(1usize << D) {
        let mut overlap = R::ONE;
        let mut upper = counter;
        let mut offset = 0usize;
        for k in 0..D {
            let end = space.start[k] + space.size[k] as i64 - 1;
            let index = if upper & 1 == 1 {
                overlap = overlap * dist[k];
                (base[k] + 1).min(end)
            } else {
                overlap = R::from_f64(overlap.to_f64() * (1.0 - dist[k].to_f64()));
                base[k].max(space.start[k])
            };
            offset += (index - space.start[k]) as usize * strides[k];
            upper >>= 1;
        }
        value += v(offset) * overlap.to_f64();
    }
    value
}

/// `LinearInterpolateImageFunction<Image<CovariantVector<double, D>>, R>::Evaluate(point)` on
/// a gradient image (the metrics' gradient-filter path): the continuous index in `R`, each
/// component interpolated as a scalar, the result stored in `R`.
pub fn vector_linear<R: Real, const D: usize>(
    image: &Image<[f64; D], D>,
    p: &[f64; D],
    floor: &IndexFloor,
) -> [R; D] {
    let c: [R; D] = image.space.point_to_cindex(p);
    std::array::from_fn(|k| {
        R::from_f64(at_cindex_with(&image.space, &c, floor, |o| {
            image.data[o][k]
        }))
    })
}

/// `CentralDifferenceImageFunction::Evaluate(point)` with `UseImageDirection` on (the
/// metrics' gradient calculator): along each axis `d`, the points `p ∓ 0.5·spacing[d]` (the
/// half spacing and the points in `R`); if both are inside, the difference of the linearly
/// interpolated values divided by their distance (0 if the distance is not above
/// `10·ε(R)`), else 0. The result is in double; the metric stores it in `R`.
pub fn central_difference<R: Real, const D: usize>(
    interp: &Linear<'_, R, D>,
    p: &[R; D],
    floor: &IndexFloor,
) -> [f64; D] {
    central_difference_on(interp.image, p, floor)
}

/// [`central_difference`] on an image of any pixel type `P` (its values read as doubles), the
/// coordinates of type `R`: a `float` image's (`antsAI`) as the interpolator of
/// `itk::Image<float, D>` with double coordinates evaluates it.
pub fn central_difference_on<P: Real, R: Real, const D: usize>(
    image: &Image<P, D>,
    p: &[R; D],
    floor: &IndexFloor,
) -> [f64; D] {
    let space = &image.space;
    let is_inside = |q: &[f64; D]| {
        let c: [R; D] = space.point_to_cindex(q);
        space.is_inside_buffer(&c)
    };
    let evaluate = |q: &[f64; D]| {
        let c: [R; D] = space.point_to_cindex(q);
        let data = &image.data;
        at_cindex_with(space, &c, floor, |o| data[o].to_f64())
    };
    let spacing = space.spacing;
    let mut out = [0.0f64; D];
    let mut n1 = *p;
    let mut n2 = *p;
    let to64 = |q: &[R; D]| -> [f64; D] { std::array::from_fn(|k| q[k].to_f64()) };
    for d in 0..D {
        let offset = R::from_f64(0.5 * spacing[d]);
        n1[d] = p[d] - offset;
        if !is_inside(&to64(&n1)) {
            out[d] = 0.0;
            n1[d] = p[d];
            n2[d] = p[d];
            continue;
        }
        n2[d] = p[d] + offset;
        if !is_inside(&to64(&n2)) {
            out[d] = 0.0;
            n1[d] = p[d];
            n2[d] = p[d];
            continue;
        }
        let delta = n2[d] - n1[d];
        out[d] = if delta > R::from_f64(10.0) * R::EPSILON {
            (evaluate(&to64(&n2)) - evaluate(&to64(&n1))) / delta.to_f64()
        } else {
            0.0
        };
        n1[d] = p[d];
        n2[d] = p[d];
    }
    out
}
