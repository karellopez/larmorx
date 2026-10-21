// SPDX-License-Identifier: Apache-2.0
//! `BSplineTransform<R, D, 3>` (ITK v5.4.5, `Modules/Core/Transform/include/itkBSplineTransform.hxx`,
//! `itkBSplineBaseTransform.hxx`, `Modules/Core/Common/include/itkBSplineInterpolationWeightFunction.hxx`)
//! and its level adaptor (`Modules/Registration/Common/include/itkBSplineTransformParametersAdaptor.hxx`).
//!
//! A cubic B-spline free-form deformation: `D` coefficient images on a control-point grid
//! (the fixed parameters: grid size, origin, spacing, direction), the parameters every
//! coefficient of dimension 0, then of dimension 1, … A point inside the grid's valid region
//! moves by the coefficients of the `4^D` control points around it weighted by the cubic
//! kernel; outside it does not move.

use larmorx_core::IndexFloor;

use crate::space::{Space, mat_vec};
use crate::{Image, ItkException, Matrix, Real};

/// The spline order (`BSplineTransform<…, 3>`, the one ANTs registers).
pub const ORDER: usize = 3;

const FILE: &str = "Modules/Core/Transform/include/itkBSplineBaseTransform.hxx";

/// `BSplineTransform<R, D, 3>`.
#[derive(Clone, Debug, PartialEq)]
pub struct BSplineXfm<R, const D: usize> {
    /// `m_FixedParameters`: grid size, origin, spacing, direction (row by row).
    pub fixed: Vec<f64>,
    /// The coefficients: dimension 0's grid (x fastest), then dimension 1's, …
    pub coefficients: Vec<R>,
    /// The coefficient images' geometry (region index 0).
    grid: Space<D>,
}

/// `FloatAlmostEqual(x, y, 4)` in `R`.
fn almost_equal<R: Real>(x: R, y: R) -> bool {
    match R::PRECISION {
        larmorx_core::Precision::Double => larmorx_image::itk_math::float_almost_equal_f64(
            x.to_f64(),
            y.to_f64(),
            4,
            0.1 * f64::EPSILON,
        ),
        larmorx_core::Precision::Float => larmorx_image::itk_math::float_almost_equal_f32(
            x.to_f64() as f32,
            y.to_f64() as f32,
            4,
            (0.1 * f64::from(f32::EPSILON)) as f32,
        ),
    }
}

/// `FloatAddULP(x, ulps)` in `R`: the integer representation moved by `ulps`.
fn add_ulp<R: Real>(x: R, ulps: i64) -> R {
    match R::PRECISION {
        larmorx_core::Precision::Double => {
            let bits = x.to_f64().to_bits() as i64;
            R::from_f64(f64::from_bits((bits + ulps) as u64))
        }
        larmorx_core::Precision::Float => {
            let bits = (x.to_f64() as f32).to_bits() as i32;
            R::from_f64(f64::from(f32::from_bits((bits + ulps as i32) as u32)))
        }
    }
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

/// A point's support: the first control point per axis and the `4^D` weights (x fastest).
pub struct Support<const D: usize> {
    pub start: [usize; D],
    pub weights: Vec<f64>,
}

impl<R: Real, const D: usize> BSplineXfm<R, D> {
    /// The grid geometry of fixed parameters.
    fn grid_of(fixed: &[f64]) -> Result<Space<D>, ItkException> {
        let size: [usize; D] = std::array::from_fn(|i| fixed[i] as usize);
        let origin: [f64; D] = std::array::from_fn(|i| fixed[D + i]);
        let spacing: [f64; D] = std::array::from_fn(|i| fixed[2 * D + i]);
        let direction: Matrix<f64, D> =
            std::array::from_fn(|i| std::array::from_fn(|j| fixed[3 * D + i * D + j]));
        Space::new(size, [0; D], spacing, origin, direction)
    }

    /// The transform with fixed parameters `fixed` and identity coefficients
    /// (`SetFixedParameters` then `SetIdentity`).
    pub fn with_fixed(fixed: Vec<f64>) -> Result<Self, ItkException> {
        let grid = Self::grid_of(&fixed)?;
        let n = D * grid.len();
        Ok(BSplineXfm {
            fixed,
            coefficients: vec![R::ZERO; n],
            grid,
        })
    }

    /// The default transform (`BSplineTransform::New()`): origin 0, physical dimensions 1, the
    /// identity direction, a mesh of 1.
    pub fn new_default() -> Self {
        let identity: Matrix<f64, D> =
            std::array::from_fn(|i| std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 }));
        let fixed = Self::fixed_from_domain(&[0.0; D], &[1.0; D], &identity, &[1; D]);
        Self::with_fixed(fixed).expect("a regular default grid")
    }

    /// `SetFixedParametersFromTransformDomainInformation(origin, physical, direction, mesh)`.
    pub fn fixed_from_domain(
        mesh_origin: &[f64; D],
        physical: &[f64; D],
        direction: &Matrix<f64, D>,
        mesh: &[usize; D],
    ) -> Vec<f64> {
        let mut fixed = vec![0.0; D * (D + 3)];
        for i in 0..D {
            fixed[i] = (mesh[i] + ORDER) as f64;
        }
        // gridSpacing (ScalarType) = physical / mesh; origin = -0.5·gridSpacing·(order − 1).
        let grid_spacing: [R; D] = std::array::from_fn(|i| {
            R::from_f64(physical[i] / R::from_i64(mesh[i] as i64).to_f64())
        });
        let origin: [f64; D] =
            std::array::from_fn(|i| -0.5 * grid_spacing[i].to_f64() * (ORDER - 1) as f64);
        let origin = mat_vec(direction, &origin);
        for i in 0..D {
            fixed[D + i] = origin[i] + mesh_origin[i];
            fixed[2 * D + i] = grid_spacing[i].to_f64();
        }
        for i in 0..D {
            for j in 0..D {
                fixed[3 * D + i * D + j] = direction[i][j];
            }
        }
        fixed
    }

    /// `GetTransformDomainOrigin()`.
    pub fn domain_origin(&self) -> [f64; D] {
        let origin: [f64; D] = std::array::from_fn(|i| {
            let spacing = R::from_f64(self.fixed[2 * D + i]);
            spacing.to_f64() * 0.5 * (ORDER - 1) as f64
        });
        let mut origin = mat_vec(&self.domain_direction(), &origin);
        for i in 0..D {
            let grid_origin = R::from_f64(self.fixed[D + i]);
            origin[i] += grid_origin.to_f64();
        }
        origin
    }

    /// `GetTransformDomainPhysicalDimensions()`: mesh size × grid spacing (in `R`).
    pub fn domain_physical(&self) -> [f64; D] {
        let mesh = self.domain_mesh();
        std::array::from_fn(|i| {
            let spacing = R::from_f64(self.fixed[2 * D + i]);
            (R::from_i64(mesh[i] as i64) * spacing).to_f64()
        })
    }

    /// `GetTransformDomainDirection()`.
    pub fn domain_direction(&self) -> Matrix<f64, D> {
        std::array::from_fn(|i| std::array::from_fn(|j| self.fixed[3 * D + i * D + j]))
    }

    /// `GetTransformDomainMeshSize()`.
    pub fn domain_mesh(&self) -> [usize; D] {
        std::array::from_fn(|i| (self.fixed[i] as usize).wrapping_sub(ORDER))
    }

    /// Applies new fixed parameters (`SetCoefficientImageInformationFromFixedParameters`):
    /// the grid, and identity coefficients when their number changes.
    fn set_fixed_internal(&mut self, fixed: Vec<f64>) -> Result<(), ItkException> {
        self.grid = Self::grid_of(&fixed)?;
        self.fixed = fixed;
        let n = D * self.grid.len();
        if self.coefficients.len() != n {
            self.coefficients = vec![R::ZERO; n];
        }
        Ok(())
    }

    /// `SetTransformDomainOrigin`, `…PhysicalDimensions`, `…MeshSize`, `…Direction`: each
    /// recomputes the fixed parameters from the domain's other properties as the getters give
    /// them, when the value differs from the current one.
    pub fn set_domain_origin(&mut self, origin: [f64; D]) -> Result<(), ItkException> {
        if self.domain_origin() != origin {
            let fixed = Self::fixed_from_domain(
                &origin,
                &self.domain_physical(),
                &self.domain_direction(),
                &self.domain_mesh(),
            );
            self.set_fixed_internal(fixed)?;
        }
        Ok(())
    }

    pub fn set_domain_physical(&mut self, physical: [f64; D]) -> Result<(), ItkException> {
        if self.domain_physical() != physical {
            let fixed = Self::fixed_from_domain(
                &self.domain_origin(),
                &physical,
                &self.domain_direction(),
                &self.domain_mesh(),
            );
            self.set_fixed_internal(fixed)?;
        }
        Ok(())
    }

    pub fn set_domain_mesh(&mut self, mesh: [usize; D]) -> Result<(), ItkException> {
        if self.domain_mesh() != mesh {
            let fixed = Self::fixed_from_domain(
                &self.domain_origin(),
                &self.domain_physical(),
                &self.domain_direction(),
                &mesh,
            );
            self.set_fixed_internal(fixed)?;
        }
        Ok(())
    }

    pub fn set_domain_direction(&mut self, direction: Matrix<f64, D>) -> Result<(), ItkException> {
        if self.domain_direction() != direction {
            let fixed = Self::fixed_from_domain(
                &self.domain_origin(),
                &self.domain_physical(),
                &direction,
                &self.domain_mesh(),
            );
            self.set_fixed_internal(fixed)?;
        }
        Ok(())
    }

    /// `SetIdentity()`.
    pub fn set_identity(&mut self) {
        self.coefficients.fill(R::ZERO);
    }

    /// `SetFixedParameters(p)`: ITK's exception for a wrong count; the coefficients reset to
    /// zero when their number changes.
    pub fn set_fixed_parameters(&mut self, p: &[f64]) -> Result<(), ItkException> {
        if p.len() != D * (D + 3) {
            return Err(ItkException::new(
                "Modules/Core/Transform/include/itkBSplineTransform.hxx",
                424,
                "BSplineTransform",
                format!(
                    "Mismatched between parameters size {} and the required number of fixed parameters {}",
                    p.len(),
                    D * (D + 3)
                ),
            ));
        }
        self.set_fixed_internal(p.to_vec())
    }

    /// The coefficient grid.
    pub fn grid(&self) -> &Space<D> {
        &self.grid
    }

    /// `GetNumberOfParametersPerDimension()`.
    pub fn parameters_per_dimension(&self) -> usize {
        self.grid.len()
    }

    /// `GetNumberOfParameters()`.
    pub fn number_of_parameters(&self) -> usize {
        D * self.grid.len()
    }

    /// `SetParameters(p)`.
    pub fn set_parameters(&mut self, p: &[R]) -> Result<(), ItkException> {
        let n = self.number_of_parameters();
        if p.len() != n {
            return Err(ItkException::new(
                FILE,
                50,
                "BSplineTransform",
                format!(
                    "Mismatch between parameters size {} and expected number of parameters {n}{}",
                    p.len(),
                    if self.grid.is_empty() {
                        ". \nSince the size of the grid region is 0, perhaps you forgot to SetGridRegion or SetFixedParameters before setting the Parameters."
                    } else {
                        ""
                    }
                ),
            ));
        }
        self.coefficients.copy_from_slice(p);
        Ok(())
    }

    /// `SetParametersByValue(p)` (the transform readers, after `SetFixedParameters`).
    pub fn set_parameters_by_value(&mut self, p: &[R]) -> Result<(), ItkException> {
        let n = self.number_of_parameters();
        if p.len() != n {
            return Err(ItkException::new(
                FILE,
                83,
                "BSplineTransform",
                format!(
                    "Mismatched between parameters size {} and region size {n}",
                    p.len()
                ),
            ));
        }
        self.coefficients.copy_from_slice(p);
        Ok(())
    }

    /// `UpdateTransformParameters(update, factor)`.
    pub fn update(&mut self, update: &[R], factor: R) -> Result<(), ItkException> {
        let n = self.number_of_parameters();
        if update.len() != n {
            return Err(ItkException::new(
                FILE,
                128,
                "BSplineTransform",
                format!(
                    "Parameter update size, {}, must  be same as transform parameter size, {n}\n",
                    update.len()
                ),
            ));
        }
        for (c, &u) in self.coefficients.iter_mut().zip(update) {
            *c = if factor == R::ONE {
                *c + u
            } else {
                *c + u * factor
            };
        }
        Ok(())
    }

    /// The continuous index of `p` on the grid, moved inside the valid region as
    /// `InsideValidRegion` moves it, or `None` outside.
    fn valid_index(&self, p: &[R; D]) -> Option<[R; D]> {
        let pf: [f64; D] = std::array::from_fn(|k| p[k].to_f64());
        let mut c: [R; D] = self.grid.point_to_cindex(&pf);
        let min_limit = R::from_f64(0.5 * (ORDER - 1) as f64);
        for j in 0..D {
            let max_limit = R::from_f64(
                R::from_i64(self.grid.size[j] as i64).to_f64() - 0.5 * (ORDER - 1) as f64 - 1.0,
            );
            if almost_equal(c[j], max_limit) {
                c[j] = add_ulp(max_limit, -6);
            } else if c[j] >= max_limit || c[j] < min_limit {
                return None;
            }
        }
        Some(c)
    }

    /// `BSplineInterpolationWeightFunction::Evaluate`: the support's first index
    /// `floor(c + 0.5 − order/2)` and the weights, each the product (from 1, axis 0 first) of
    /// the axes' cubic kernel values at `c − start`, `c − start − 1`, …
    pub fn support(&self, p: &[R; D]) -> Option<Support<D>> {
        let c = self.valid_index(p)?;
        let mut start = [0usize; D];
        let mut w1 = [[0.0f64; ORDER + 1]; D];
        for j in 0..D {
            let s = (c[j].to_f64() + 0.5 - ORDER as f64 / 2.0).floor();
            start[j] = s as usize;
            let mut x = c[j].to_f64() - s;
            for k in 0..=ORDER {
                w1[j][k] = cubic(x);
                x -= 1.0;
            }
        }
        let n = (ORDER + 1).pow(D as u32);
        let mut weights = Vec::with_capacity(n);
        for k in 0..n {
            let mut rest = k;
            let mut w = 1.0f64;
            for wj in w1.iter() {
                w *= wj[rest % (ORDER + 1)];
                rest /= ORDER + 1;
            }
            weights.push(w);
        }
        Some(Support { start, weights })
    }

    /// The grid offsets of a support's control points (x fastest).
    pub fn support_offsets(&self, s: &Support<D>) -> Vec<usize> {
        let n = (ORDER + 1).pow(D as u32);
        let strides = self.grid.strides();
        (0..n)
            .map(|k| {
                let mut rest = k;
                let mut o = 0usize;
                for j in 0..D {
                    o += (s.start[j] + rest % (ORDER + 1)) * strides[j];
                    rest /= ORDER + 1;
                }
                o
            })
            .collect()
    }

    /// `TransformPoint(p)`: inside the valid region, each coordinate the sum (from 0, in `R`)
    /// of the weighted coefficients (each product in double, stored in `R`) plus the point's
    /// coordinate; outside, `p`.
    pub fn transform_point(&self, p: &[R; D]) -> [R; D] {
        let Some(s) = self.support(p) else {
            return *p;
        };
        let offsets = self.support_offsets(&s);
        let per = self.parameters_per_dimension();
        let mut out = [R::ZERO; D];
        for (w, &o) in s.weights.iter().zip(&offsets) {
            for j in 0..D {
                let c = self.coefficients[j * per + o];
                out[j] = out[j] + R::from_f64(w * c.to_f64());
            }
        }
        std::array::from_fn(|j| out[j] + p[j])
    }

    /// `ComputeJacobianWithRespectToParameters(p, J)`: `D` rows of all the parameters, zero
    /// but at the support's control points (row `d`, column `d·perDimension + offset`), where
    /// the weight is.
    pub fn jacobian_parameters(&self, p: &[R; D], out: &mut [R]) {
        out.fill(R::ZERO);
        let Some(s) = self.support(p) else {
            return;
        };
        let np = self.number_of_parameters();
        let per = self.parameters_per_dimension();
        for (w, o) in s.weights.iter().zip(self.support_offsets(&s)) {
            for d in 0..D {
                out[d * np + o + d * per] = R::from_f64(*w);
            }
        }
    }

    /// ITK's exception of `ComputeJacobianWithRespectToPosition`, which B-spline transforms do
    /// not implement.
    pub fn no_position_jacobian() -> ItkException {
        ItkException::new(
            "Modules/Core/Transform/include/itkBSplineBaseTransform.h",
            301,
            "BSplineTransform",
            "ComputeJacobianWithRespectToPosition not yet implemented for BSplineTransform",
        )
    }
}

/// `BSplineDecompositionImageFilter<Image<R>, Image<R>>`: the 1D filter along each axis in
/// turn (lines in raster order of their start), each line read into double, filtered and
/// stored back in `R`.
pub fn decompose<R: Real, const D: usize>(data: &[R], size: &[usize; D], order: u32) -> Vec<R> {
    let mut c = data.to_vec();
    let strides: [usize; D] = {
        let mut s = [1usize; D];
        for k in 1..D {
            s[k] = s[k - 1] * size[k - 1];
        }
        s
    };
    for d in 0..D {
        let n = size[d];
        let stride = strides[d];
        let starts: Vec<usize> = (0..c.len())
            .filter(|&o| (o / stride).is_multiple_of(n))
            .collect();
        for &s in &starts {
            let mut line: Vec<f64> = (0..n).map(|i| c[s + i * stride].to_f64()).collect();
            larmorx_interp::bspline_line_to_coefficients(&mut line, order);
            for (i, v) in line.into_iter().enumerate() {
                c[s + i * stride] = R::from_f64(v);
            }
        }
    }
    c
}

/// `BSplineTransformParametersAdaptor`'s required fixed parameters
/// (`UpdateRequiredFixedParameters`) for a mesh, a domain origin, physical dimensions and a
/// direction (all in double).
pub fn required_fixed<const D: usize>(
    mesh: &[usize; D],
    origin: &[f64; D],
    physical: &[f64; D],
    direction: &Matrix<f64, D>,
) -> Vec<f64> {
    let mut fixed = vec![0.0; D * (D + 3)];
    for i in 0..D {
        fixed[i] = (mesh[i] + ORDER) as f64;
    }
    let grid_origin: [f64; D] = std::array::from_fn(|i| {
        let grid_spacing = physical[i] / mesh[i] as f64;
        -0.5 * grid_spacing * (ORDER - 1) as f64
    });
    let grid_origin = mat_vec(direction, &grid_origin);
    for i in 0..D {
        fixed[D + i] = grid_origin[i] + origin[i];
        fixed[2 * D + i] = physical[i] / mesh[i] as f64;
    }
    for i in 0..D {
        for j in 0..D {
            fixed[3 * D + i * D + j] = direction[i][j];
        }
    }
    fixed
}

/// `BSplineTransformParametersAdaptor::AdaptTransformParameters`: nothing when the transform's
/// fixed parameters are `required`; else each dimension's coefficient image resampled onto the
/// new grid (`ResampleImageFilter` with the identity transform and
/// `BSplineResampleImageFunction`, which evaluates the cubic B-spline of the coefficients as
/// they are) and turned back into coefficients (`BSplineDecompositionImageFilter`, order 3).
pub fn adapt<R: Real, const D: usize>(
    t: &mut BSplineXfm<R, D>,
    required: &[f64],
    floor: &IndexFloor,
) -> Result<(), ItkException> {
    if t.fixed.as_slice() == required {
        return Ok(());
    }
    let new_grid = BSplineXfm::<R, D>::grid_of(required)?;
    let per = t.parameters_per_dimension();
    let mut coefficients = Vec::with_capacity(D * new_grid.len());
    let identity = crate::transform::Composite::new(Vec::new());
    for j in 0..D {
        let image = Image::new(
            t.grid.clone(),
            t.coefficients[j * per..(j + 1) * per].to_vec(),
        );
        let upsampled = crate::resample::resample_linear(
            &image,
            &new_grid,
            &identity,
            &crate::resample::Interpolator::Other(
                larmorx_interp::nd::InterpolationNd::BSplineCoefficients { order: 3 },
            ),
            R::ZERO,
            floor,
        )
        .map_err(|e| ItkException::new(FILE, 0, "BSplineTransformParametersAdaptor", e))?;
        coefficients.extend(decompose(&upsampled, &new_grid.size, 3));
    }
    t.grid = new_grid;
    t.fixed = required.to_vec();
    t.coefficients = coefficients;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CompensatedSums;
    use crate::rng::MersenneTwister;
    use crate::transform::{Composite, Kind, LinearXfm, Xfm};

    fn transform(rng: &mut MersenneTwister) -> BSplineXfm<f64, 3> {
        let mut b = BSplineXfm::<f64, 3>::new_default();
        let direction = [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        b.set_domain_origin([-10.0, 4.0, 2.5]).unwrap();
        b.set_domain_physical([30.0, 22.0, 17.0]).unwrap();
        b.set_domain_mesh([3, 2, 2]).unwrap();
        b.set_domain_direction(direction).unwrap();
        for c in &mut b.coefficients {
            *c = 4.0 * (rng.closed() - 0.5);
        }
        b
    }

    /// The sparse Jacobian's columns are the dense Jacobian's non-zero ones, through a
    /// composite with a linear transform after the B-spline one too.
    #[test]
    fn sparse_jacobian_matches_the_dense_one() {
        let mut rng = MersenneTwister::from_int(7);
        let b = transform(&mut rng);
        let mut affine = LinearXfm::<f64, 3>::new(Kind::Affine);
        affine.set_matrix([[1.1, 0.1, 0.0], [-0.05, 0.95, 0.02], [0.0, 0.03, 1.02]]);
        affine.set_offset([1.0, -2.0, 0.5]);
        let mut inside = 0;
        for queue in [
            vec![Xfm::BSpline(b.clone())],
            vec![Xfm::Linear(affine), Xfm::BSpline(b.clone())],
        ] {
            let c = Composite::new(queue);
            let np = c.number_of_parameters();
            for _ in 0..200 {
                let p = [
                    -42.0 + 40.0 * rng.closed(),
                    -6.0 + 40.0 * rng.closed(),
                    1.0 + 19.0 * rng.closed(),
                ];
                let mut dense = vec![0.0; 3 * np];
                c.jacobian(&p, &mut dense);
                let (cols, values) = c.sparse_jacobian(&p).unwrap();
                let k = cols.len();
                if k > 0 {
                    inside += 1;
                }
                let mut rebuilt = vec![0.0; 3 * np];
                for (j, &col) in cols.iter().enumerate() {
                    for r in 0..3 {
                        rebuilt[r * np + col] = values[r * k + j];
                    }
                }
                assert_eq!(
                    dense.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    rebuilt.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                );
            }
        }
        assert!(inside > 50, "{inside} points inside the grid");
    }

    /// Sparse additions with the zeros deferred give the dense sums bit for bit.
    #[test]
    fn deferred_zeros_give_the_dense_sums() {
        let mut rng = MersenneTwister::from_int(3);
        let n = 40;
        let mut dense = CompensatedSums::new(n);
        let mut sparse = CompensatedSums::new(n);
        for _ in 0..3000 {
            let mut local = vec![0.0f64; n];
            let mut cols = Vec::new();
            for (c, v) in local.iter_mut().enumerate() {
                if rng.closed() < 0.2 {
                    // Values over many magnitudes, so the compensations matter.
                    *v = (rng.closed() - 0.5) * 10f64.powi((rng.closed() * 12.0) as i32 - 6);
                    cols.push(c);
                }
            }
            let values: Vec<f64> = cols.iter().map(|&c| local[c]).collect();
            dense.add_dense(&local);
            sparse.add_sparse(&cols, &values);
        }
        sparse.flush();
        for (a, b) in dense.sums.iter().zip(&sparse.sums) {
            assert_eq!(a.sum().to_bits(), b.sum().to_bits());
            assert_eq!(a.compensation().to_bits(), b.compensation().to_bits());
        }
    }

    /// The decomposition of a constant is the constant (order 3), axis by axis.
    #[test]
    fn decomposition_of_a_constant() {
        let data = vec![2.5f64; 5 * 4 * 3];
        let c = decompose(&data, &[5, 4, 3], 3);
        for v in c {
            assert!((v - 2.5).abs() < 1e-12);
        }
    }
}
