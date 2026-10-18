// SPDX-License-Identifier: Apache-2.0
//! `GaussianMembershipFunction<Point<float, D>>` (ITK v5.4.5,
//! `Modules/Numerics/Statistics/include/itkGaussianMembershipFunction.hxx`) and
//! `ManifoldParzenWindowsPointSetFunction` (`Modules/Registration/Metricsv4/include/
//! itkManifoldParzenWindowsPointSetFunction.hxx`), the moving density of the
//! Jensen-Havrda-Charvát-Tsallis metric.
//!
//! A Gaussian's covariance is inverted by vnl's SVD (`vnl_matrix_inverse<double>`), its
//! determinant the product of the singular values. Setting a covariance equal to the one the
//! Gaussian has keeps everything as it is: a new Gaussian starts with the identity covariance
//! and the univariate prefactor `1/√(2π)`, so ITK keeps that prefactor for an identity
//! covariance in any dimension (ITK-23, corrected on request of the caller).

use larmorx_core::vnl_svd::vnl_svd;

use super::kdtree::KdTree;
use crate::{CompensatedSum, ItkException, Real};

/// `std::sqrt(2.0 * itk::Math::pi)`.
fn sqrt_two_pi() -> f64 {
    (2.0 * std::f64::consts::PI).sqrt()
}

/// A Gaussian of mean `mean` (`Point<double>`) and covariance `covariance` (row-major,
/// `VariableSizeMatrix<double>`).
#[derive(Clone, Debug)]
pub struct Gaussian<const D: usize> {
    pub mean: [f64; D],
    pub covariance: [[f64; D]; D],
    pub inverse_covariance: [[f64; D]; D],
    pub prefactor: f64,
}

fn identity<const D: usize>() -> [[f64; D]; D] {
    std::array::from_fn(|i| std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 }))
}

impl<const D: usize> Gaussian<D> {
    /// A new `GaussianMembershipFunction` with its mean set.
    pub fn new(mean: [f64; D]) -> Self {
        Gaussian {
            mean,
            covariance: identity(),
            inverse_covariance: identity(),
            prefactor: 1.0 / sqrt_two_pi(),
        }
    }

    /// `SetCovariance(cov)`; `true` when `cov` is the covariance the Gaussian has (a new
    /// Gaussian's identity). `ITK-23` corrected (`normalise_identity`): it is set like any
    /// other, and gets the `D`-dimensional prefactor; ITK returns early and keeps the
    /// univariate one.
    pub fn set_covariance(
        &mut self,
        cov: [[f64; D]; D],
        normalise_identity: bool,
    ) -> Result<bool, ItkException> {
        let same = self.covariance == cov;
        if same && !normalise_identity {
            return Ok(true);
        }
        self.covariance = cov;
        let flat: Vec<f64> = cov.iter().flatten().copied().collect();
        let Some((u, w, v)) = vnl_svd(&flat, D, D) else {
            return Err(ItkException::new(
                "Modules/Numerics/Statistics/include/itkGaussianMembershipFunction.hxx",
                108,
                "GaussianMembershipFunction",
                "vnl's SVD of the covariance did not converge",
            ));
        };
        // vnl_svd's zero_out_absolute(0): singular values of magnitude 0 are zero and have no
        // inverse.
        let mut det = w[0];
        for &wk in &w[1..] {
            det *= wk;
        }
        if det < 0.0 {
            return Err(ItkException::new(
                "Modules/Numerics/Statistics/include/itkGaussianMembershipFunction.hxx",
                113,
                "GaussianMembershipFunction",
                "det( m_Covariance ) < 0",
            ));
        }
        if det > 1.0e-6 {
            // pinverse: (V · W⁻¹) · Uᵀ, each product summed in order.
            let winv: Vec<f64> = w
                .iter()
                .map(|&wk| if wk.abs() <= 0.0 { 0.0 } else { 1.0 / wk })
                .collect();
            let mut vw = [[0.0f64; D]; D];
            for i in 0..D {
                for k in 0..D {
                    let mut sum = 0.0;
                    for (j, &wj) in winv.iter().enumerate() {
                        let d = if j == k { wj } else { 0.0 };
                        sum += v[i + D * j] * d;
                    }
                    vw[i][k] = sum;
                }
            }
            let mut inv = [[0.0f64; D]; D];
            for i in 0..D {
                for k in 0..D {
                    let mut sum = 0.0;
                    for j in 0..D {
                        sum += vw[i][j] * u[k + D * j];
                    }
                    inv[i][k] = sum;
                }
            }
            self.inverse_covariance = inv;
            self.prefactor = 1.0 / (det.sqrt() * larmorx_core::math::powi(sqrt_two_pi(), D as u32));
        } else {
            // std::pow(DBL_MAX, 1.0 / 3.0) / D.
            let large = libm::pow(f64::MAX, 1.0 / 3.0) / D as f64;
            let mut inv = identity::<D>();
            for (r, row) in inv.iter_mut().enumerate() {
                row[r] *= large;
            }
            self.inverse_covariance = inv;
            self.prefactor = 1.0;
        }
        Ok(same)
    }

    /// `Evaluate(measurement)`: `prefactor · exp(−½ (y − μ)ᵀ Σ⁻¹ (y − μ))`, the differences of
    /// a `float` coordinate and a double mean in double.
    #[inline]
    pub fn evaluate(&self, y: &[f32; D]) -> f64 {
        let mut temp = 0.0f64;
        for r in 0..D {
            let mut rowdot = 0.0f64;
            for c in 0..D {
                rowdot += self.inverse_covariance[r][c] * (f64::from(y[c]) - self.mean[c]);
            }
            temp += rowdot * (f64::from(y[r]) - self.mean[r]);
        }
        self.prefactor * larmorx_core::math::exp(-0.5 * temp)
    }
}

/// `ManifoldParzenWindowsPointSetFunction<PointSet, R>` over a point set's points: one Gaussian
/// per point, its covariance the regularisation `σ²·I` or, anisotropic, the kernel-weighted
/// covariance of its nearest points plus `σ²·I`.
#[derive(Clone, Debug)]
pub struct ParzenWindows<const D: usize> {
    pub tree: KdTree<D>,
    pub gaussians: Vec<Gaussian<D>>,
    pub evaluation_k: usize,
}

/// The settings of the density (`antsRegistration`'s JHCT: kernel σ 10, covariance
/// neighbourhood 5, normalised).
#[derive(Clone, Copy, Debug)]
pub struct ParzenSettings<R> {
    pub kernel_sigma: R,
    pub regularization_sigma: R,
    pub covariance_k: usize,
    pub evaluation_k: usize,
    pub anisotropic: bool,
    pub normalize: bool,
    /// ITK-23 corrected: an identity covariance gets the `D`-dimensional prefactor.
    pub normalise_identity: bool,
    /// ITK-26 corrected: each product enters the covariance's diagonal once.
    pub single_diagonal: bool,
}

/// What building the density met (for larmorx's notices).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParzenEvents {
    /// A Gaussian's covariance was the identity (ITK-23).
    pub identity: bool,
    /// A local covariance was accumulated (ITK-26).
    pub covariances: bool,
    /// The covariance neighbourhood is larger than the point set: ITK reads past the end of
    /// the list of neighbours (ITK-25); the missing neighbours are left out.
    pub short_neighbourhood: bool,
}

/// `PointsLocator::FindClosestNPoints`' warning (one per call that asks for more neighbours
/// than there are points).
pub fn too_many_neighbours_warning(n: usize) -> String {
    format!(
        "WARNING: In Modules/Registration/Common/include/itkPointsLocator.hxx, line 82\nPointsLocator (0x0): The number of requested neighbors is greater than the total number of points.  Only returning {n} points.\n\n"
    )
}

/// `FindClosestNPoints(query, k)`: at most the number of points, with ITK's warning when `k`
/// is larger.
pub fn closest_n<const D: usize>(
    tree: &KdTree<D>,
    query: &[f32; D],
    k: usize,
    warnings: &mut Vec<String>,
) -> Vec<usize> {
    let mut n = k;
    if n > tree.len() {
        n = tree.len();
        warnings.push(too_many_neighbours_warning(n));
    }
    tree.search(query, n)
}

impl<const D: usize> ParzenWindows<D> {
    /// `SetInputPointSet(points)`. Where the covariance neighbourhood is larger than the
    /// point set, ITK reads past the end of the list of neighbours (class (a), ITK-25): the
    /// missing neighbours are left out here.
    pub fn new<R: Real>(
        points: &[[f32; D]],
        s: &ParzenSettings<R>,
        warnings: &mut Vec<String>,
        events: &mut ParzenEvents,
    ) -> Result<Self, ItkException> {
        let n = points.len();
        let mut kernel_cov = identity::<D>();
        for (r, row) in kernel_cov.iter_mut().enumerate() {
            row[r] *= s.kernel_sigma.to_f64();
        }
        let mut inputs = Vec::with_capacity(n);
        for p in points {
            let mut g = Gaussian::new(std::array::from_fn(|k| f64::from(p[k])));
            events.identity |= g.set_covariance(kernel_cov, s.normalise_identity)?;
            inputs.push(g);
        }
        if n == 0 {
            return Err(ItkException::new(
                "Modules/Registration/Common/include/itkPointsLocator.hxx",
                41,
                "PointsLocator",
                "The number of points is 0.",
            ));
        }
        let tree = KdTree::new(points.to_vec()).map_err(|_| {
            ItkException::new(
                "Modules/Numerics/Statistics/include/itkKdTreeGenerator.hxx",
                150,
                "KdTreeGenerator",
                "a point coordinate is not a number",
            )
        })?;
        let sigma2 = s.regularization_sigma * s.regularization_sigma;
        let mut gaussians = Vec::with_capacity(n);
        for (index, point) in points.iter().enumerate() {
            let mut g = Gaussian::new(inputs[index].mean);
            if s.covariance_k > 0 && s.anisotropic {
                let mut cout = [[0.0f64; D]; D];
                let neighbours = closest_n(&tree, point, s.covariance_k, warnings);
                if neighbours.len() < s.covariance_k {
                    events.short_neighbourhood = true;
                }
                let mut denominator = CompensatedSum::default();
                for &j in &neighbours {
                    if j != index && j < n {
                        let neighbour = &points[j];
                        let kernel = R::from_f64(inputs[index].evaluate(neighbour));
                        denominator.add(kernel.to_f64());
                        if kernel > R::ZERO {
                            events.covariances = true;
                            for m in 0..D {
                                for q in m..D {
                                    let dm = R::from_f64(f64::from(neighbour[m] - point[m]));
                                    let dq = R::from_f64(f64::from(neighbour[q] - point[q]));
                                    let c = kernel * dm * dq;
                                    cout[m][q] += c.to_f64();
                                    if q != m || !s.single_diagonal {
                                        cout[q][m] += c.to_f64();
                                    }
                                }
                            }
                        }
                    }
                }
                // GetSum() is the double accumulator.
                let den = denominator.sum();
                if s.normalize && den > 0.0 {
                    for row in cout.iter_mut() {
                        for c in row.iter_mut() {
                            *c /= den;
                        }
                    }
                } else {
                    let k = R::from_i64(s.covariance_k as i64).to_f64();
                    for row in cout.iter_mut() {
                        for c in row.iter_mut() {
                            *c /= k;
                        }
                    }
                }
                for m in 0..D {
                    cout[m][m] += sigma2.to_f64();
                }
                events.identity |= g.set_covariance(cout, s.normalise_identity)?;
            } else {
                let mut cov = identity::<D>();
                for (r, row) in cov.iter_mut().enumerate() {
                    row[r] *= sigma2.to_f64();
                }
                events.identity |= g.set_covariance(cov, s.normalise_identity)?;
            }
            gaussians.push(g);
        }
        Ok(ParzenWindows {
            tree,
            gaussians,
            evaluation_k: s.evaluation_k,
        })
    }

    /// `Evaluate(point)`: the mean of the nearest Gaussians' values over all Gaussians (every
    /// Gaussian when the neighbourhood holds them all), in `R`.
    pub fn evaluate<R: Real>(&self, point: &[f32; D]) -> R {
        let size = self.gaussians.len();
        let k = self.evaluation_k.min(size);
        // CompensatedSummation<R>: each value converted to R, then added in double.
        let mut sum = CompensatedSum::default();
        if k == size {
            for g in &self.gaussians {
                sum.add(R::from_f64(g.evaluate(point)).to_f64());
            }
        } else {
            for j in self.tree.search(point, k) {
                sum.add(R::from_f64(self.gaussians[j].evaluate(point)).to_f64());
            }
        }
        R::from_f64(sum.sum() / R::from_i64(size as i64).to_f64())
    }
}
