// SPDX-License-Identifier: Apache-2.0
//! B-spline transforms (ITK's `BSplineTransform<R, 3, 3>`), as `antsRegistration`'s `BSpline`
//! stage writes them and `antsApplyTransforms` applies them.
//!
//! A cubic B-spline free-form deformation: three coefficient images on a control-point grid
//! (the fixed parameters: grid size, origin, spacing, direction row by row), the parameters
//! every coefficient of dimension 0, then of dimension 1, then of dimension 2. A point inside
//! the grid's valid region moves by the coefficients of the 4 × 4 × 4 control points around it
//! weighted by the cubic kernel; outside it does not move. A B-spline transform has no
//! inverse.
//!
//! Ported from ITK v5.4.5: `Modules/Core/Transform/include/itkBSplineTransform.hxx`
//! (`TransformPoint`, `InsideValidRegion`), `itkBSplineBaseTransform.hxx`,
//! `Modules/Core/Common/include/itkBSplineInterpolationWeightFunction.hxx` and
//! `itkBSplineKernelFunction.h` (`FastEvaluate`), with `ImageBase`'s
//! `TransformPhysicalPointToContinuousIndex`. The arithmetic is the one `larmorx-registration`
//! uses for the same transform (`transform::bspline`), validated against ANTs there.

use larmorx_core::linalg::Mat3;
use larmorx_core::{Grid3, ItkReal, Precision};

use crate::TransformError;
use crate::field::FieldData;

/// The spline order (`BSplineTransform<…, 3>`).
const ORDER: usize = 3;

/// A cubic B-spline transform in three dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct BSplineTransform {
    /// The coefficient grid (`m_FixedParameters`).
    pub grid: Grid3,
    /// The coefficients as stored: dimension 0's grid (x fastest), then dimension 1's, then
    /// dimension 2's.
    pub coefficients: FieldData,
    /// The ITK class name it was read as (`BSplineTransform_double_3_3`, `…_float_3_3`).
    pub itk_name: String,
}

/// `BSplineKernelFunction<3>::FastEvaluate`.
#[inline]
fn cubic(u: f64) -> f64 {
    let a = u.abs();
    if a < 1.0 {
        let s = a * a;
        (4.0 - 6.0 * s + 3.0 * s * a) / 6.0
    } else if a < 2.0 {
        let s = a * a;
        (8.0 - 12.0 * a + 6.0 * s - s * a) / 6.0
    } else {
        0.0
    }
}

/// `FloatIEEE<T>::AsULP`: the bits as a signed integer, negative numbers mapped so that
/// adjacent values differ by one.
fn as_ulp<R: ItkReal>(x: R) -> i64 {
    match R::PRECISION {
        Precision::Double => {
            let u = x.to_f64().to_bits();
            if u >> 63 != 0 {
                (0x8000_0000_0000_0000u64.wrapping_sub(u)) as i64
            } else {
                u as i64
            }
        }
        Precision::Float => {
            let u = (x.to_f64() as f32).to_bits();
            i64::from(if u >> 31 != 0 {
                (0x8000_0000u32.wrapping_sub(u)) as i32
            } else {
                u as i32
            })
        }
    }
}

/// `Math::FloatAlmostEqual(x, y)` in `R`: 4 ulps, or an absolute difference of at most
/// `0.1·ε(R)`.
fn almost_equal<R: ItkReal>(x: R, y: R) -> bool {
    let (max_abs, max_ulps) = match R::PRECISION {
        Precision::Double => (0.1 * f64::EPSILON, 4i64),
        Precision::Float => (f64::from((0.1 * f64::from(f32::EPSILON)) as f32), 4i64),
    };
    let diff = match R::PRECISION {
        Precision::Double => (x.to_f64() - y.to_f64()).abs(),
        Precision::Float => f64::from((x.to_f64() as f32 - y.to_f64() as f32).abs()),
    };
    if diff <= max_abs {
        return true;
    }
    if x.to_f64().is_sign_negative() != y.to_f64().is_sign_negative() {
        return false;
    }
    match R::PRECISION {
        Precision::Double => as_ulp(x).wrapping_sub(as_ulp(y)).wrapping_abs() <= max_ulps,
        Precision::Float => {
            let d = (as_ulp(x) as i32).wrapping_sub(as_ulp(y) as i32);
            i64::from(d.wrapping_abs()) <= max_ulps
        }
    }
}

/// `Math::FloatAddULP(x, ulps)` in `R`: the integer representation moved by `ulps`.
fn add_ulp<R: ItkReal>(x: R, ulps: i64) -> R {
    match R::PRECISION {
        Precision::Double => {
            let bits = x.to_f64().to_bits() as i64;
            R::from_f64(f64::from_bits((bits + ulps) as u64))
        }
        Precision::Float => {
            let bits = (x.to_f64() as f32).to_bits() as i32;
            R::from_f64(f64::from(f32::from_bits((bits + ulps as i32) as u32)))
        }
    }
}

impl BSplineTransform {
    /// A B-spline transform from ITK's representation: the coefficients, and the 18 fixed
    /// parameters (grid size, origin, spacing, direction row by row).
    pub fn from_itk(
        itk_name: &str,
        coefficients: FieldData,
        fixed: &[f64],
    ) -> Result<Self, TransformError> {
        if !itk_name.ends_with("_3_3") {
            return Err(TransformError::Unsupported(itk_name.to_owned()));
        }
        if fixed.len() != 18 {
            return Err(TransformError::Invalid(format!(
                "{itk_name} needs 18 fixed parameters, got {}",
                fixed.len()
            )));
        }
        let size = [fixed[0], fixed[1], fixed[2]].map(|v| v as usize);
        let direction: Mat3 =
            std::array::from_fn(|i| std::array::from_fn(|j| fixed[9 + 3 * i + j]));
        let grid = Grid3::new(
            size,
            [fixed[6], fixed[7], fixed[8]],
            [fixed[3], fixed[4], fixed[5]],
            direction,
        )
        .ok_or_else(|| TransformError::Invalid("singular B-spline grid".into()))?;
        let n = 3 * grid.len();
        let len = match &coefficients {
            FieldData::F32(v) => v.len(),
            FieldData::F64(v) => v.len(),
        };
        if len != n {
            return Err(TransformError::Invalid(format!(
                "Mismatch between parameters size {len} and expected number of parameters {n}"
            )));
        }
        Ok(BSplineTransform {
            grid,
            coefficients,
            itk_name: itk_name.to_owned(),
        })
    }

    /// ITK's fixed parameters of this transform.
    pub fn fixed_parameters(&self) -> Vec<f64> {
        let g = &self.grid;
        let mut out: Vec<f64> = g.size.iter().map(|&n| n as f64).collect();
        out.extend_from_slice(&g.origin);
        out.extend_from_slice(&g.spacing);
        out.extend(g.direction.iter().flatten());
        out
    }

    /// The number of parameters (three coefficients per control point).
    pub fn number_of_parameters(&self) -> usize {
        3 * self.grid.len()
    }

    /// The parameters as `BSplineTransform<R>` holds them (each stored value cast to `R`).
    pub fn parameters_as<R: ItkReal>(&self) -> Vec<f64> {
        (0..self.number_of_parameters())
            .map(|i| self.coefficient::<R>(i).to_f64())
            .collect()
    }

    /// Coefficient `i` as `BSplineTransform<R>` holds it (`static_cast<R>` of the stored
    /// value).
    #[inline]
    fn coefficient<R: ItkReal>(&self, i: usize) -> R {
        match &self.coefficients {
            FieldData::F32(v) => R::from_f64(f64::from(v[i])),
            FieldData::F64(v) => R::from_f64(v[i]),
        }
    }

    /// Maps a point (LPS) in double.
    pub fn transform_point(&self, p: [f64; 3]) -> [f64; 3] {
        self.transform_point_as(p)
    }

    /// `BSplineTransform<R>::TransformPoint`: the continuous index of the point on the grid
    /// (computed in double, stored as `R`), moved inside the valid region as
    /// `InsideValidRegion` moves it (an index within 4 ulps of the upper limit moved 6 ulps
    /// below it) or the point returned unchanged outside it; inside, the support's first
    /// index `floor(c + 0.5 − order/2)` and the weights (the products, axis 0 first, of the
    /// cubic kernel at `c − start`, `c − start − 1`, …, in double); each coordinate is the
    /// sum from 0 in `R` of the weighted coefficients (each product in double, stored in `R`)
    /// plus the point's coordinate, in `R`.
    pub fn transform_point_as<R: ItkReal>(&self, p: [R; 3]) -> [R; 3] {
        let c: [R; 3] = self
            .grid
            .point_to_index(p.map(|x| x.to_f64()))
            .map(R::from_f64);
        let mut c = c;
        let min_limit = R::from_f64(0.5 * (ORDER - 1) as f64);
        for j in 0..3 {
            let max_limit = R::from_f64(
                R::from_f64(self.grid.size[j] as f64).to_f64() - 0.5 * (ORDER - 1) as f64 - 1.0,
            );
            if almost_equal(c[j], max_limit) {
                c[j] = add_ulp(max_limit, -6);
            } else if c[j] >= max_limit || c[j] < min_limit {
                return p;
            }
        }
        let mut start = [0usize; 3];
        let mut w1 = [[0.0f64; ORDER + 1]; 3];
        for j in 0..3 {
            let s = (c[j].to_f64() + 0.5 - ORDER as f64 / 2.0).floor();
            start[j] = s as usize;
            let mut x = c[j].to_f64() - s;
            for k in 0..=ORDER {
                w1[j][k] = cubic(x);
                x -= 1.0;
            }
        }
        let size = self.grid.size;
        let per = self.grid.len();
        let mut out = [R::ZERO; 3];
        for k in 0..(ORDER + 1).pow(3) {
            let mut rest = k;
            let mut w = 1.0f64;
            let mut offset = 0usize;
            let mut stride = 1usize;
            for j in 0..3 {
                let t = rest % (ORDER + 1);
                rest /= ORDER + 1;
                w *= w1[j][t];
                offset += (start[j] + t) * stride;
                stride *= size[j];
            }
            for (j, o) in out.iter_mut().enumerate() {
                let coefficient: R = self.coefficient(j * per + offset);
                *o = *o + R::from_f64(w * coefficient.to_f64());
            }
        }
        std::array::from_fn(|j| out[j] + p[j])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A transform of mesh 2 × 2 × 2 over [0, 20] × [0, 30] × [0, 10] (identity direction).
    fn transform(coefficient: impl Fn(usize, usize) -> f64) -> BSplineTransform {
        let n = 5usize;
        let spacing = [10.0, 15.0, 5.0];
        let origin = spacing.map(|s| -0.5 * s * (ORDER - 1) as f64);
        let mut fixed = vec![n as f64; 3];
        fixed.extend(origin);
        fixed.extend(spacing);
        fixed.extend([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        let per = n * n * n;
        let c: Vec<f64> = (0..3 * per).map(|i| coefficient(i / per, i % per)).collect();
        BSplineTransform::from_itk("BSplineTransform_double_3_3", FieldData::F64(c), &fixed)
            .unwrap()
    }

    #[test]
    fn constant_coefficients_translate_the_valid_region_only() {
        // The cubic B-spline's weights sum to 1: equal coefficients translate.
        let t = transform(|d, _| [1.0, -2.0, 0.5][d]);
        let q = t.transform_point([7.0, 11.0, 3.0]);
        for (a, b) in q.iter().zip([8.0, 9.0, 3.5]) {
            assert!((a - b).abs() < 1e-12, "{q:?}");
        }
        // Outside the valid region (the mesh's physical domain): unchanged.
        assert_eq!(t.transform_point([-1.0, 11.0, 3.0]), [-1.0, 11.0, 3.0]);
        assert_eq!(t.transform_point([7.0, 31.0, 3.0]), [7.0, 31.0, 3.0]);
        // The upper edge of the domain is inside (moved 6 ulps below the limit).
        let edge = t.transform_point([20.0, 30.0, 10.0]);
        assert!((edge[0] - 21.0).abs() < 1e-9, "{edge:?}");
    }

    #[test]
    fn float_points_map_in_float() {
        let t = transform(|d, i| 0.01 * (i as f64) * (d as f64 + 1.0));
        let p = [7.25f32, 11.5, 3.125];
        let q = t.transform_point_as(p);
        let d = t.transform_point([7.25, 11.5, 3.125]);
        for k in 0..3 {
            assert!((f64::from(q[k]) - d[k]).abs() < 1e-4);
        }
    }

    #[test]
    fn wrong_counts_are_refused() {
        let fixed = vec![5.0; 18];
        assert!(
            BSplineTransform::from_itk("BSplineTransform_double_3_3", FieldData::F64(vec![0.0; 3]), &fixed)
                .is_err()
        );
        assert!(
            BSplineTransform::from_itk("BSplineTransform_double_2_2", FieldData::F64(vec![]), &fixed)
                .is_err()
        );
    }
}
