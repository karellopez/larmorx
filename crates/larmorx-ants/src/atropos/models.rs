// SPDX-License-Identifier: Apache-2.0
//! Atropos' likelihood models (`-k`): ANTs' list-sample functions (ANTs v2.6.5,
//! `ImageSegmentation/ants*ListSampleFunction.hxx`), on ITK v5.4.5's
//! `DiscreteGaussianImageFilter`, `StatisticsImageFilter`, `BSplineInterpolateImageFunction`,
//! `GaussianMembershipFunction`, `KdTree` and vnl's symmetric eigensystem.
//!
//! The models' `RealType` is `float` (Atropos instantiates them with `float` outputs): every
//! quantity ANTs declares `RealType` is an `f32` here, every ITK `double` an `f64`.

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::math::{expf, logf};
use larmorx_core::vnl_eigen::symmetric_eigensystem;
use larmorx_core::vnl_svd::vnl_inverse;
use larmorx_image::discrete_gaussian::{DiscreteGaussianOptions, discrete_gaussian};
use larmorx_image::kdtree::KdTree;
use larmorx_interp::nd::{bspline_at, bspline_coefficients};

use super::gaussian::{EstimateError, GaussianMembership, GaussianModel, PartialVolumeModel};

/// The model chosen with `-k`, with its parameters.
#[derive(Clone, Debug, PartialEq)]
pub enum LikelihoodModel {
    Gaussian,
    /// `HistogramParzenWindows[sigma, bins]` (1, 32).
    HistogramParzenWindows {
        sigma: f32,
        bins: u32,
    },
    /// `ManifoldParzenWindows[pointSetSigma, evaluationK, covarianceK, kernelSigma]`
    /// (1, 50, 0, 1).
    ManifoldParzenWindows {
        regularization_sigma: f32,
        evaluation_k: u32,
        covariance_k: u32,
        kernel_sigma: f32,
    },
    /// `JointShapeAndOrientationProbability[shapeSigma, shapeBins, orientationSigma,
    /// orientationBins]` (2, 64, 1, 32).
    JointShapeAndOrientationProbability {
        shape_sigma: f32,
        shape_bins: u32,
        orientation_sigma: f32,
        orientation_bins: u32,
    },
    LogEuclideanGaussian,
}

/// One class's likelihood function.
#[derive(Clone, Debug)]
pub enum Likelihood {
    Gaussian(GaussianModel),
    PartialVolume(PartialVolumeModel),
    HistogramParzen(HistogramParzenModel),
    ManifoldParzen(ManifoldParzenModel),
    LogEuclidean(LogEuclideanModel),
    JointShape(JointShapeModel),
}

impl Likelihood {
    /// The tissue class model for `model`; `mode` decides ANTs' defects in the models.
    pub fn new(model: &LikelihoodModel, mode: UpstreamDefects) -> Self {
        match *model {
            LikelihoodModel::Gaussian => Likelihood::Gaussian(GaussianModel::default()),
            LikelihoodModel::HistogramParzenWindows { sigma, bins } => {
                Likelihood::HistogramParzen(HistogramParzenModel::new(sigma, bins))
            }
            LikelihoodModel::ManifoldParzenWindows {
                regularization_sigma,
                evaluation_k,
                covariance_k,
                kernel_sigma,
            } => Likelihood::ManifoldParzen(ManifoldParzenModel::new(
                regularization_sigma,
                evaluation_k,
                covariance_k,
                kernel_sigma,
                mode,
            )),
            LikelihoodModel::LogEuclideanGaussian => {
                Likelihood::LogEuclidean(LogEuclideanModel::default())
            }
            LikelihoodModel::JointShapeAndOrientationProbability {
                shape_sigma,
                shape_bins,
                orientation_sigma,
                orientation_bins,
            } => Likelihood::JointShape(JointShapeModel::new(
                shape_sigma,
                shape_bins,
                orientation_sigma,
                orientation_bins,
                mode,
            )),
        }
    }

    /// `SetInputListSample(sample)` with `weights` set before (`n × m` samples, row-major).
    /// `Ok(Some(warning))` for ITK's warning on a sample of at most one element.
    pub fn set_sample(
        &mut self,
        samples: &[f32],
        m: usize,
        weights: &[f64],
    ) -> Result<Option<&'static str>, EstimateError> {
        match self {
            Likelihood::Gaussian(g) => g.set_sample(samples, m, weights),
            Likelihood::PartialVolume(p) => p.set_sample(0, samples, m, weights),
            Likelihood::HistogramParzen(h) => h.set_sample(samples, m, weights),
            Likelihood::ManifoldParzen(h) => h.set_sample(samples, m, weights),
            Likelihood::LogEuclidean(h) => h.set_sample(samples, m, weights),
            Likelihood::JointShape(h) => h.set_sample(samples, m, weights),
        }
    }

    /// `SetIndexedInputListSample(d, sample)` (partial-volume models).
    pub fn set_indexed_sample(
        &mut self,
        d: usize,
        samples: &[f32],
        m: usize,
        weights: &[f64],
    ) -> Result<Option<&'static str>, EstimateError> {
        match self {
            Likelihood::PartialVolume(p) => p.set_sample(d, samples, m, weights),
            other if d == 0 => other.set_sample(samples, m, weights),
            _ => Ok(None),
        }
    }

    /// `Evaluate(measurement)`.
    #[inline]
    pub fn evaluate(&self, x: &[f32]) -> f32 {
        match self {
            Likelihood::Gaussian(g) => g.evaluate(x),
            Likelihood::PartialVolume(p) => p.evaluate(x),
            Likelihood::HistogramParzen(h) => h.evaluate(x),
            Likelihood::ManifoldParzen(h) => h.evaluate(x),
            Likelihood::LogEuclidean(h) => h.evaluate(x),
            Likelihood::JointShape(h) => h.evaluate(x),
        }
    }
}

const SMALL: &str =
    "The input list sample has <= 1 element.Function evaluations will be equal to 0.";

// ------------------------------------------------------------------------------------------------
// Histogram images shared by the Parzen-window models

/// A smoothed, normalized histogram image (`Image<float, D>`, origin and spacing per axis,
/// identity direction) and its cubic B-spline coefficients.
#[derive(Clone, Debug, PartialEq)]
pub struct HistogramImage<const D: usize> {
    pub size: [usize; D],
    pub origin: [f64; D],
    pub spacing: [f64; D],
    pub values: Vec<f32>,
    coefficients: Vec<f64>,
    /// `m_PhysicalPointToIndex` (the vnl inverse of `diag(spacing)`), row-major.
    to_index: [[f64; D]; D],
}

impl<const D: usize> HistogramImage<D> {
    fn zeros(size: [usize; D], origin: [f64; D], spacing: [f64; D]) -> Self {
        let n: usize = size.iter().product();
        let flat: Vec<f64> = (0..D * D)
            .map(|k| {
                if k % (D + 1) == 0 {
                    spacing[k / D]
                } else {
                    0.0
                }
            })
            .collect();
        let inv = vnl_inverse(&flat, D).unwrap_or_else(|| vec![f64::NAN; D * D]);
        HistogramImage {
            size,
            origin,
            spacing,
            values: vec![0.0; n],
            coefficients: Vec::new(),
            to_index: std::array::from_fn(|r| std::array::from_fn(|c| inv[r * D + c])),
        }
    }

    /// `TransformPhysicalPointToContinuousIndex<double>`.
    #[inline]
    fn continuous_index(&self, point: [f64; D]) -> [f64; D] {
        let v: [f64; D] = std::array::from_fn(|k| point[k] - self.origin[k]);
        std::array::from_fn(|r| {
            let mut sum = 0.0f64;
            for c in 0..D {
                sum += self.to_index[r][c] * v[c];
            }
            sum
        })
    }

    fn linear(&self, idx: [i64; D]) -> Option<usize> {
        let mut off = 0usize;
        let mut stride = 1usize;
        for d in 0..D {
            if idx[d] < 0 || idx[d] >= self.size[d] as i64 {
                return None;
            }
            off += idx[d] as usize * stride;
            stride *= self.size[d];
        }
        Some(off)
    }

    /// `DiscreteGaussianImageFilter` (variance per axis, maximum error 0.01, no image
    /// spacing), `StatisticsImageFilter`'s sum and `DivideImageFilter` by it (in float), then
    /// the interpolator's coefficients.
    fn smooth_and_normalize(&mut self, variance: [f64; D]) {
        let size_v = self.size.to_vec();
        let spacing_v = self.spacing.to_vec();
        let mut options = DiscreteGaussianOptions::new(D, 0.0);
        options.variance = variance.to_vec();
        options.use_image_spacing = false;
        let smoothed = if self.values.is_empty() {
            Vec::new()
        } else {
            discrete_gaussian(
                larmorx_image::VolumeRef::new(&self.values, &size_v, &spacing_v),
                &options,
                1,
            )
            .unwrap_or_else(|_| self.values.clone())
        };
        // StatisticsImageFilter: a compensated sum over the one region, added to the
        // filter's own compensated total (compensation first, then the sum).
        let mut sum = super::gaussian::Kahan::default();
        for &v in &smoothed {
            sum.add(f64::from(v));
        }
        let mut total = super::gaussian::Kahan::default();
        total.add(sum.compensation());
        total.add(sum.sum());
        let divisor = total.sum() as f32;
        self.values = smoothed
            .iter()
            .map(|&a| {
                if larmorx_image::itk_math::almost_equal_f32(divisor, 0.0) {
                    f32::MAX
                } else {
                    a / divisor
                }
            })
            .collect();
        self.coefficients = bspline_coefficients(&self.values, self.size, 3);
    }

    /// `IsInsideBuffer(point)` then `Evaluate(point)` of the cubic B-spline interpolator.
    #[inline]
    fn evaluate(&self, point: [f64; D]) -> Option<f64> {
        let cidx = self.continuous_index(point);
        for d in 0..D {
            let start = -0.5f64;
            let end = (self.size[d] as i64 - 1) as f64 + 0.5;
            if !(cidx[d] >= start && cidx[d] < end) {
                return None;
            }
        }
        Some(bspline_at(&self.coefficients, self.size, 3, cidx))
    }
}

// ------------------------------------------------------------------------------------------------
// HistogramParzenWindows

/// ANTs' `HistogramParzenWindowsListSampleFunction`: per measurement component, a histogram of
/// the weighted samples (linear splatting into `bins` bins plus 3 sigma on each side),
/// smoothed by a Gaussian of `sigma` bins, normalized, and interpolated with cubic B-splines;
/// the likelihood is the product over the components.
#[derive(Clone, Debug)]
pub struct HistogramParzenModel {
    pub sigma: f32,
    pub bins: u32,
    pub histograms: Vec<HistogramImage<1>>,
}

impl HistogramParzenModel {
    pub fn new(sigma: f32, bins: u32) -> Self {
        HistogramParzenModel {
            sigma,
            bins,
            histograms: Vec::new(),
        }
    }

    fn set_sample(
        &mut self,
        samples: &[f32],
        m: usize,
        weights: &[f64],
    ) -> Result<Option<&'static str>, EstimateError> {
        let n = samples.len() / m.max(1);
        if n <= 1 {
            return Ok(Some(SMALL));
        }
        let mut min = vec![f32::MAX; m];
        let mut max = vec![-f32::MAX; m];
        for i in 0..n {
            for d in 0..m {
                let x = samples[i * m + d];
                if x < min[d] {
                    min[d] = x;
                }
                if x > max[d] {
                    max[d] = x;
                }
            }
        }
        let sigma = self.sigma;
        let mut histograms = Vec::with_capacity(m);
        for d in 0..m {
            let spacing = f64::from((max[d] - min[d]) / (self.bins.wrapping_sub(1)) as f32);
            let origin = f64::from(min[d]) - 3.0 * (f64::from(sigma) * spacing);
            let three = 3.0f32 * (sigma * spacing as f32);
            let extent = ((max[d] + three) - (min[d] - three)) / spacing as f32;
            let size = cast_unsigned(extent.ceil());
            histograms.push(HistogramImage::<1>::zeros([size], [origin], [spacing]));
        }
        let weighted = weights.len() == n;
        for i in 0..n {
            let w = if weighted { weights[i] as f32 } else { 1.0 };
            for d in 0..m {
                let h = &mut histograms[d];
                let cidx = h.continuous_index([f64::from(samples[i * m + d])])[0];
                let idx = cidx.floor() as i64;
                if let Some(k) = h.linear([idx]) {
                    let old = h.values[k];
                    h.values[k] = (1.0 - (cidx - idx as f64) as f32) * w + old;
                }
                let idx = idx + 1;
                if let Some(k) = h.linear([idx]) {
                    let old = h.values[k];
                    h.values[k] = (1.0 - (idx as f64 - cidx) as f32) * w + old;
                }
            }
        }
        let variance = f64::from(sigma * sigma);
        for h in histograms.iter_mut() {
            h.smooth_and_normalize([variance]);
        }
        self.histograms = histograms;
        Ok(None)
    }

    #[inline]
    fn evaluate(&self, x: &[f32]) -> f32 {
        let mut probability = 1.0f32;
        for (d, h) in self.histograms.iter().enumerate() {
            match h.evaluate([f64::from(x[d])]) {
                Some(v) => probability *= v as f32,
                None => return 0.0,
            }
        }
        probability
    }
}

/// `static_cast<unsigned int>(float)` as x86-64 converts it (through a 64-bit integer, NaN and
/// out-of-range values to `0x8000000000000000`, then truncated).
fn cast_unsigned(v: f32) -> usize {
    let wide: i64 = if v.is_nan() || v >= 9.223_372e18 || v < -9.223_372e18 {
        i64::MIN
    } else {
        v as i64
    };
    wide as u32 as usize
}

// ------------------------------------------------------------------------------------------------
// ManifoldParzenWindows

/// ANTs' `ManifoldParzenWindowsListSampleFunction`: a Gaussian at every sample, with a
/// covariance `regularizationSigma · I`, or (with a covariance neighbourhood) estimated from the
/// sample's nearest neighbours weighted by a Gaussian kernel; the likelihood is the sum of the
/// Gaussians of the `evaluationK` nearest samples over the sum of the weights.
///
/// Two defects of ANTs: the local covariance adds each product to the diagonal twice and the
/// running upper value to the lower triangle (ANTS-104), and the evaluation ignores the weights
/// (ANTS-95), so every class gets the same likelihood up to a constant. Both are corrected
/// unless reproduced.
#[derive(Clone, Debug)]
pub struct ManifoldParzenModel {
    pub regularization_sigma: f32,
    pub evaluation_k: u32,
    pub covariance_k: u32,
    pub kernel_sigma: f32,
    mode: UpstreamDefects,
    samples: Vec<f32>,
    m: usize,
    weights: Vec<f32>,
    tree: Option<KdTree>,
    gaussians: Vec<GaussianMembership>,
    /// All Gaussians share one covariance (no covariance neighbourhood).
    shared: bool,
    normalization: f32,
    /// Whether the covariances met ANTS-104 (more than one dimension or a neighbour).
    pub met_covariance_defect: bool,
}

impl ManifoldParzenModel {
    pub fn new(
        regularization_sigma: f32,
        evaluation_k: u32,
        covariance_k: u32,
        kernel_sigma: f32,
        mode: UpstreamDefects,
    ) -> Self {
        ManifoldParzenModel {
            regularization_sigma,
            evaluation_k,
            covariance_k,
            kernel_sigma,
            mode,
            samples: Vec::new(),
            m: 0,
            weights: Vec::new(),
            tree: None,
            gaussians: Vec::new(),
            shared: true,
            normalization: 0.0,
            met_covariance_defect: false,
        }
    }

    fn set_sample(
        &mut self,
        samples: &[f32],
        m: usize,
        weights: &[f64],
    ) -> Result<Option<&'static str>, EstimateError> {
        let n = samples.len() / m.max(1);
        if n <= 1 {
            return Ok(Some(SMALL));
        }
        self.samples = samples.to_vec();
        self.m = m;
        let weighted = weights.len() == n;
        self.weights = if weighted {
            weights.iter().map(|&w| w as f32).collect()
        } else {
            vec![1.0; n]
        };
        let identity = |s: f64| -> Vec<f64> {
            (0..m * m)
                .map(|k| if k % (m + 1) == 0 { s } else { 0.0 })
                .collect()
        };
        let fix = self.mode.fix();
        self.gaussians.clear();
        let tree = KdTree::new(&self.samples, m, 16).map_err(|e| EstimateError {
            file: "Modules/Numerics/Statistics/include/itkKdTree.hxx",
            line: 0,
            class: "KdTree",
            description: e.to_string(),
        })?;
        if self.covariance_k > 0 {
            self.shared = false;
            let k = (self.covariance_k as usize).min(n);
            let mut ids = Vec::new();
            let mut dist = Vec::new();
            for count in 0..n {
                let x = &self.samples[count * m..(count + 1) * m];
                let mut g = GaussianMembership::default();
                let mean: Vec<f64> = x.iter().map(|&v| f64::from(v)).collect();
                g.set(
                    mean.clone(),
                    identity(1.0)
                        .iter()
                        .map(|v| v * f64::from(self.kernel_sigma))
                        .collect(),
                )?;
                let mut cov = vec![0.0f64; m * m];
                tree.search_into(x, k, &mut ids, &mut dist)
                    .map_err(|e| EstimateError {
                        file: "Modules/Numerics/Statistics/include/itkKdTree.hxx",
                        line: 167,
                        class: "KdTree",
                        description: e.to_string(),
                    })?;
                let mut denominator = 0.0f32;
                for &id in ids.iter() {
                    let id = id as usize;
                    if id == count || id >= n {
                        continue;
                    }
                    let nb = &self.samples[id * m..(id + 1) * m];
                    let mut kernel = g.evaluate(nb) as f32;
                    if weighted {
                        kernel *= self.weights[count];
                    }
                    denominator += kernel;
                    if kernel > 0.0 {
                        for a in 0..m {
                            for b in a..m {
                                let c = kernel * (nb[a] - x[a]) * (nb[b] - x[b]);
                                cov[a * m + b] += f64::from(c);
                                if fix {
                                    if a != b {
                                        cov[b * m + a] += f64::from(c);
                                    }
                                } else {
                                    let upper = cov[a * m + b];
                                    cov[b * m + a] += upper;
                                }
                                self.met_covariance_defect = true;
                            }
                        }
                    }
                }
                if denominator > 0.0 {
                    let d = f64::from(denominator);
                    for v in cov.iter_mut() {
                        *v /= d;
                    }
                }
                let reg = f64::from(self.regularization_sigma * self.regularization_sigma);
                for a in 0..m {
                    cov[a * m + a] += reg;
                }
                g.set(mean, cov)?;
                self.gaussians.push(g);
            }
        } else {
            self.shared = true;
            let mut g = GaussianMembership::default();
            let cov = identity(f64::from(self.regularization_sigma));
            g.set(
                self.samples[..m].iter().map(|&v| f64::from(v)).collect(),
                cov,
            )?;
            self.gaussians.push(g);
        }
        let mut norm = 0.0f32;
        for i in 0..n {
            norm += if weighted { self.weights[i] } else { 1.0 };
        }
        self.normalization = norm;
        self.tree = Some(tree);
        Ok(None)
    }

    /// The Gaussian of sample `j` at `x`.
    #[inline]
    fn gaussian_at(&self, j: usize, x: &[f32]) -> f64 {
        let m = self.m;
        if self.shared {
            // One covariance for all: the shared Gaussian with sample j's mean.
            let g = &self.gaussians[0];
            let mean = &self.samples[j * m..(j + 1) * m];
            let mut temp = 0.0f64;
            for r in 0..m {
                let mut rowdot = 0.0f64;
                for c in 0..m {
                    rowdot += g.inverse[r * m + c] * (f64::from(x[c]) - f64::from(mean[c]));
                }
                temp += rowdot * (f64::from(x[r]) - f64::from(mean[r]));
            }
            g.prefactor * larmorx_core::math::exp(-0.5 * temp)
        } else {
            self.gaussians[j].evaluate(x)
        }
    }

    fn evaluate(&self, x: &[f32]) -> f32 {
        let n = self.samples.len() / self.m.max(1);
        if n == 0 {
            return 0.0;
        }
        let k = (self.evaluation_k as usize).min(n);
        let fix = self.mode.fix();
        let mut sum = 0.0f32;
        let term = |j: usize| -> f32 {
            let g = self.gaussian_at(j, x) as f32;
            if fix { g * self.weights[j] } else { g }
        };
        if k == n {
            for j in 0..n {
                sum += term(j);
            }
        } else {
            match self.tree.as_ref().and_then(|t| t.search(x, k).ok()) {
                Some(ids) => {
                    for id in ids {
                        sum += term(id as usize);
                    }
                }
                None => return 0.0,
            }
        }
        sum / self.normalization
    }
}

// ------------------------------------------------------------------------------------------------
// Tensors

/// `D` of `L = D(D+1)/2` tensor components: `(unsigned)(0.5f · (−1 + sqrtf(1 + 8L)))`.
fn tensor_dimension(l: usize) -> usize {
    let lf = l as f32;
    (0.5f32 * (-1.0 + (1.0 + 8.0 * lf).sqrt())) as usize
}

/// A `D × D` float matrix (row-major), as `VariableSizeMatrix<float>`.
#[derive(Clone, Debug, PartialEq)]
struct Mat {
    d: usize,
    v: Vec<f32>,
}

impl Mat {
    fn zeros(d: usize) -> Self {
        Mat {
            d,
            v: vec![0.0; d * d],
        }
    }
    fn at(&self, r: usize, c: usize) -> f32 {
        self.v[r * self.d + c]
    }
    /// The symmetric tensor of the upper-triangular components `x`.
    fn tensor(x: &[f32], d: usize) -> Self {
        let mut t = Mat::zeros(d);
        let mut index = 0;
        for i in 0..d {
            for j in i..d {
                t.v[i * d + j] = x[index];
                t.v[j * d + i] = t.v[i * d + j];
                index += 1;
            }
        }
        t
    }
    /// `vnl_matrix<float>::operator*`: sums in float, from 0.
    fn mul(&self, b: &Mat) -> Mat {
        let d = self.d;
        let mut out = Mat::zeros(d);
        for i in 0..d {
            for k in 0..d {
                let mut s = 0.0f32;
                for j in 0..d {
                    s += self.at(i, j) * b.at(j, k);
                }
                out.v[i * d + k] = s;
            }
        }
        out
    }
    fn transpose(&self) -> Mat {
        let d = self.d;
        let mut out = Mat::zeros(d);
        for i in 0..d {
            for j in 0..d {
                out.v[j * d + i] = self.at(i, j);
            }
        }
        out
    }
    /// `DecomposeTensorFunction::EvaluateSymmetricEigenDecomposition`: vnl's eigensystem of the
    /// float matrix, computed in double; the eigenvalues ascending on `W`'s diagonal and the
    /// eigenvectors in `V`'s columns, as float.
    fn eigen(&self) -> (Mat, Mat) {
        let d = self.d;
        let a: Vec<f64> = self.v.iter().map(|&x| f64::from(x)).collect();
        let e = symmetric_eigensystem(&a, d);
        let mut w = Mat::zeros(d);
        let mut v = Mat::zeros(d);
        for j in 0..d {
            for i in 0..d {
                v.v[i * d + j] = e.vectors[i * d + j] as f32;
                if i == j {
                    w.v[i * d + j] = e.values[j] as f32;
                }
            }
        }
        (w, v)
    }
}

/// ANTs' `LogEuclideanGaussianListSampleFunction`: the weighted mean of the tensors' matrix
/// logarithms, mapped back with the exponential; the dispersion is the weighted mean squared
/// log-Euclidean distance to it; the likelihood a univariate Gaussian of that distance.
#[derive(Clone, Debug, Default)]
pub struct LogEuclideanModel {
    mean: Option<Mat>,
    dispersion: f32,
}

impl LogEuclideanModel {
    fn log_tensor(t: &Mat) -> Mat {
        let (mut w, v) = t.eigen();
        for i in 0..w.d {
            let x = w.at(i, i);
            w.v[i * w.d + i] = if x > 0.0 { logf(x) } else { 0.0 };
        }
        let w = w.mul(&v.transpose());
        v.mul(&w)
    }

    fn exp_tensor(t: &Mat) -> Mat {
        let (mut w, v) = t.eigen();
        for i in 0..w.d {
            w.v[i * w.d + i] = expf(w.at(i, i));
        }
        let w = w.mul(&v.transpose());
        v.mul(&w)
    }

    fn distance(s: &Mat, t: &Mat) -> f32 {
        let ls = Self::log_tensor(s);
        let lt = Self::log_tensor(t);
        let mut diff = Mat::zeros(s.d);
        for k in 0..diff.v.len() {
            diff.v[k] = ls.v[k] - lt.v[k];
        }
        let sq = diff.mul(&diff);
        let mut trace = 0.0f32;
        for i in 0..sq.d {
            trace += sq.at(i, i);
        }
        trace.sqrt()
    }

    fn set_sample(
        &mut self,
        samples: &[f32],
        m: usize,
        weights: &[f64],
    ) -> Result<Option<&'static str>, EstimateError> {
        let n = samples.len() / m.max(1);
        if n <= 1 {
            return Ok(Some(SMALL));
        }
        let d = tensor_dimension(m);
        let weighted = weights.len() == n;
        let mut mean = Mat::zeros(d);
        let mut total = 0.0f32;
        let mut count = 0usize;
        for i in 0..n {
            let t = Self::log_tensor(&Mat::tensor(&samples[i * m..(i + 1) * m], d));
            let w = if weighted {
                let w = weights[count] as f32;
                count += 1;
                w
            } else {
                1.0
            };
            total += w;
            for k in 0..mean.v.len() {
                mean.v[k] += t.v[k] * w;
            }
        }
        if total > 0.0 {
            for v in mean.v.iter_mut() {
                *v /= total;
            }
        }
        let mean = Self::exp_tensor(&mean);
        let mut dispersion = 0.0f32;
        let mut count = 0usize;
        for i in 0..n {
            let t = Mat::tensor(&samples[i * m..(i + 1) * m], d);
            let dist = Self::distance(&t, &mean);
            let w = if weighted {
                let w = weights[count] as f32;
                count += 1;
                w
            } else {
                1.0
            };
            dispersion += w * (dist * dist);
        }
        dispersion /= count as f32;
        self.mean = Some(mean);
        self.dispersion = dispersion;
        Ok(None)
    }

    fn evaluate(&self, x: &[f32]) -> f32 {
        let Some(mean) = &self.mean else {
            return 0.0;
        };
        let t = Mat::tensor(x, mean.d);
        let distance = Self::distance(&t, mean);
        let pre = 1.0f32 / (2.0f32 * std::f32::consts::PI * self.dispersion).sqrt();
        pre * expf(-0.5f32 * (distance * distance) / self.dispersion)
    }
}

/// ANTs' `JointHistogramParzenShapeAndOrientationListSampleFunction`: joint histograms of the
/// tensors' two largest eigenvalues (shape) and of the angles of their first and second
/// eigenvectors (orientation), smoothed and normalized.
///
/// ANTs evaluates the histograms of shape and first orientation at the measurement's first two
/// components (raw tensor values, not eigenvalues or angles), each at a point whose second
/// coordinate it never sets (ANTS-103). With `reproduce` larmorx evaluates them there, the
/// unset coordinate 0; with `fix` it evaluates them where the histograms were filled: at the
/// measurement's normalized two largest eigenvalues and its principal eigenvector's angles.
#[derive(Clone, Debug)]
pub struct JointShapeModel {
    pub shape_sigma: f32,
    pub shape_bins: u32,
    pub orientation_sigma: f32,
    pub orientation_bins: u32,
    mode: UpstreamDefects,
    max_ev1: f32,
    max_ev2: f32,
    min_ev1: f32,
    min_ev2: f32,
    histograms: [Option<HistogramImage<2>>; 3],
}

impl JointShapeModel {
    pub fn new(
        shape_sigma: f32,
        shape_bins: u32,
        orientation_sigma: f32,
        orientation_bins: u32,
        mode: UpstreamDefects,
    ) -> Self {
        JointShapeModel {
            shape_sigma,
            shape_bins,
            orientation_sigma,
            orientation_bins,
            mode,
            max_ev1: 0.0,
            max_ev2: 0.0,
            min_ev1: 1.0,
            min_ev2: 1.0,
            histograms: [None, None, None],
        }
    }

    fn increment(h: &mut HistogramImage<2>, point: [f64; 2]) {
        let cidx = h.continuous_index(point);
        let idx = [
            (cidx[0] + 0.5).floor() as i64,
            (cidx[1] + 0.5).floor() as i64,
        ];
        if let Some(k) = h.linear(idx) {
            let old = h.values[k];
            h.values[k] = 1.0 + old;
        }
    }

    /// The shape histogram's point of the normalized eigenvalues `ev1`, `ev2`.
    fn shape_point(&self, mut ev1: f32, mut ev2: f32) -> [f64; 2] {
        ev1 = ev1.clamp_upper(1.0);
        ev2 = ev2.clamp_upper(1.0);
        if ev1 < 0.0 {
            ev1 = 0.0;
        }
        if ev2 < 0.0 {
            ev2 = 0.0;
        }
        let scale = self.shape_bins.wrapping_sub(1) as f32;
        [f64::from(ev1 * scale), f64::from(ev2 * scale)]
    }

    fn increment_shape(&mut self, ev1: f32, ev2: f32) {
        let bins = self.shape_bins as usize;
        let p = self.shape_point(ev1, ev2);
        let h = self.histograms[0]
            .get_or_insert_with(|| HistogramImage::zeros([bins, bins], [0.0; 2], [1.0; 2]));
        Self::increment(h, p);
    }

    /// The orientation histograms' point of the unit vector (`x`, `y`, `z`).
    fn orientation_point(&self, mut x: f32, mut y: f32, mut z: f32) -> [f64; 2] {
        if x < 0.0 {
            x *= -1.0;
            y *= -1.0;
            z *= -1.0;
        }
        let psi = acosf(z);
        let pi = std::f64::consts::PI;
        let theta: f32 = if x.abs() + y.abs() < 1e-9f32 {
            0.0
        } else if larmorx_image::itk_math::almost_equal_f32(y, 0.0) {
            if x > 0.0 { 0.0 } else { pi as f32 }
        } else if larmorx_image::itk_math::almost_equal_f32(x, 0.0) {
            if y > 0.0 {
                std::f64::consts::FRAC_PI_2 as f32
            } else {
                -std::f64::consts::FRAC_PI_2 as f32
            }
        } else if x > 0.0 && y > 0.0 {
            atanf(y / x)
        } else if x < 0.0 && y > 0.0 || x < 0.0 && y < 0.0 {
            pi as f32 + atanf(y / x)
        } else {
            atanf(y / x)
        };
        let nb = self.orientation_bins.wrapping_sub(1) as f32;
        let p0 = f64::from(psi / pi as f32 * nb + 1.0);
        let p1 = f64::from((theta + std::f64::consts::FRAC_PI_2 as f32) / pi as f32 * nb);
        [p0, p1]
    }

    fn increment_orientation(&mut self, x: f32, y: f32, z: f32, which: usize) {
        let bins = self.orientation_bins as usize;
        let p = self.orientation_point(x, y, z);
        let h = self.histograms[which]
            .get_or_insert_with(|| HistogramImage::zeros([bins + 2, bins], [0.0; 2], [1.0; 2]));
        Self::increment(h, p);
        // Circular boundary: column 0 copies column `bins`, column `bins + 1` copies column 1.
        let (w, rows) = (bins + 2, bins);
        for r in 0..rows {
            h.values[r * w] = h.values[r * w + bins];
            h.values[r * w + bins + 1] = h.values[r * w + 1];
        }
    }

    fn set_sample(
        &mut self,
        samples: &[f32],
        m: usize,
        _weights: &[f64],
    ) -> Result<Option<&'static str>, EstimateError> {
        let n = samples.len() / m.max(1);
        if n <= 1 {
            return Ok(Some(SMALL));
        }
        let d = tensor_dimension(m);
        if d < 3 {
            return Err(EstimateError {
                file: "ImageSegmentation/antsJointHistogramParzenShapeAndOrientationListSampleFunction.hxx",
                line: 0,
                class: "JointHistogramParzenShapeAndOrientationListSampleFunction",
                description: format!(
                    "{d}x{d} tensors: ANTs reads the third eigenvalue and eigenvector past the \
                     decomposition's matrices (larmorx refuses)"
                ),
            });
        }
        self.histograms = [None, None, None];
        let decompose = |i: usize| Mat::tensor(&samples[i * m..(i + 1) * m], d).eigen();
        for i in 0..n {
            let (w, _) = decompose(i);
            let (ev1, ev2) = (w.at(2, 2), w.at(1, 1));
            if ev1 > self.max_ev1 {
                self.max_ev1 = ev1;
            }
            if ev2 > self.max_ev2 {
                self.max_ev2 = ev2;
            }
            if ev1 < self.min_ev1 {
                self.min_ev1 = ev1;
            }
            if ev2 < self.min_ev2 {
                self.min_ev2 = ev2;
            }
        }
        for i in 0..n {
            let (w, v) = decompose(i);
            let ev1 = w.at(2, 2) / (self.max_ev1 - self.min_ev1);
            let ev2 = w.at(1, 1) / (self.max_ev2 - self.min_ev2);
            self.increment_shape(ev1, ev2);
            self.increment_orientation(v.at(0, 2), v.at(1, 2), v.at(2, 2), 1);
            self.increment_orientation(v.at(0, 1), v.at(1, 1), v.at(2, 1), 2);
        }
        let variances = [
            f64::from(self.shape_sigma * self.shape_sigma),
            f64::from(self.orientation_sigma * self.orientation_sigma),
            f64::from(self.shape_sigma * self.orientation_sigma),
        ];
        for (h, &v) in self.histograms.iter_mut().zip(&variances) {
            if let Some(h) = h {
                h.smooth_and_normalize([v, v]);
            }
        }
        Ok(None)
    }

    fn evaluate(&self, x: &[f32]) -> f32 {
        let points = if self.mode.fix() {
            // Where the histograms were filled (ANTS-103 corrected).
            let d = tensor_dimension(x.len());
            if d < 3 {
                return 0.0;
            }
            let (w, v) = Mat::tensor(x, d).eigen();
            let ev1 = w.at(2, 2) / (self.max_ev1 - self.min_ev1);
            let ev2 = w.at(1, 1) / (self.max_ev2 - self.min_ev2);
            [
                self.shape_point(ev1, ev2),
                self.orientation_point(v.at(0, 2), v.at(1, 2), v.at(2, 2)),
            ]
        } else {
            // ANTs: the raw components, point[1] never set (0 here).
            [[f64::from(x[0]), 0.0], [f64::from(x[1]), 0.0]]
        };
        let mut probability = 1.0f32;
        for (h, point) in self.histograms[..2].iter().zip(points) {
            let Some(h) = h else {
                return 0.0;
            };
            match h.evaluate(point) {
                Some(v) => probability *= v as f32,
                None => return 0.0,
            }
        }
        probability
    }
}

trait ClampUpper {
    fn clamp_upper(self, max: f32) -> f32;
}

impl ClampUpper for f32 {
    fn clamp_upper(self, max: f32) -> f32 {
        if self > max { max } else { self }
    }
}

/// `std::acos(float)`: `acosf`, correctly rounded through the correctly rounded double `acos`
/// (the float result is exact unless the double one lies on a float midpoint, which is not
/// checked).
fn acosf(x: f32) -> f32 {
    larmorx_core::math::acos(f64::from(x)) as f32
}

/// `std::atan(float)`: `atanf` from the correctly rounded double `atan2(y, 1)` (as above).
fn atanf(x: f32) -> f32 {
    larmorx_core::math::atan2(f64::from(x), 1.0) as f32
}

// ------------------------------------------------------------------------------------------------
// Printing

/// `Print(os, indent)` of a model: the header (`Name (0x0)`, the address as `0x0`) and its
/// `PrintSelf` one indent further.
pub fn print(model: &Likelihood, indent: &str) -> String {
    let next = format!("{indent}  ");
    let g = |v: f32| crate::cli::cstd::format_g(f64::from(v));
    match model {
        Likelihood::Gaussian(gm) => format!(
            "{indent}GaussianListSampleFunction (0x0)\n{next}{}",
            gaussian_text(&gm.gaussian)
        ),
        Likelihood::PartialVolume(p) => format!(
            "{indent}PartialVolumeGaussianListSampleFunction (0x0)\n{next}{}",
            gaussian_text(&p.gaussian)
        ),
        Likelihood::HistogramParzen(h) => format!(
            "{indent}HistogramParzenWindowsListSampleFunction (0x0)\n{next}Sigma: {}\n{next}Number of histogram bins: {}\n",
            g(h.sigma),
            h.bins
        ),
        Likelihood::ManifoldParzen(h) => {
            let mut s = format!(
                "{indent}ManifoldParzenWindowsListSampleFunction (0x0)\n{next}Regularization sigma: {}\n{next}Evaluation K neighborhood: {}\n",
                g(h.regularization_sigma),
                h.evaluation_k
            );
            if h.covariance_k > 0 {
                s += &format!(
                    "{next}Covariance K neighborhood: {}\n{next}Kernel sigma: {}\n",
                    h.covariance_k,
                    g(h.kernel_sigma)
                );
            }
            s
        }
        Likelihood::LogEuclidean(h) => {
            let mut s = format!(
                "{indent}LogEuclideanGaussianListSampleFunction (0x0)\n{next}Mean tensor = ["
            );
            if let Some(m) = &h.mean {
                for r in 0..m.d {
                    for c in 0..m.d - 1 {
                        s += &format!("{}, ", g(m.at(r, c)));
                    }
                    s += &g(m.at(r, m.d - 1));
                    s += if r == m.d - 1 { "], " } else { "; " };
                }
            }
            s += &format!("Dispersion (variance) = {}\n", g(h.dispersion));
            s
        }
        Likelihood::JointShape(h) => format!(
            "{indent}JointHistogramParzenShapeAndOrientationListSampleFunction (0x0)\n{next}Shape Sigma: {}\n{next}Number of shape histogram bins: {}\n{next}Orientation Sigma: {}\n{next}Number of orientation histogram bins: {}\n{next}Minimum eigenvalue 1: {}{next}Minimum eigenvalue 2: {}{next}Maximum eigenvalue 1: {}{next}Maximum eigenvalue 2: {}{next}Use nearest neighbor increments.\n",
            g(h.shape_sigma),
            h.shape_bins,
            g(h.orientation_sigma),
            h.orientation_bins,
            g(h.min_ev1),
            g(h.min_ev2),
            g(h.max_ev1),
            g(h.max_ev2)
        ),
    }
}

/// `mean = [..], covariance = [a, b; c, d]` as the Gaussian functions print it: the mean
/// with ITK's `ConvertNumberToString`, the covariance with the stream's default format.
fn gaussian_text(g: &GaussianMembership) -> String {
    let mean: Vec<String> = g
        .mean
        .iter()
        .map(|&x| larmorx_io::itk_transform::itk_number(x))
        .collect();
    let k = g.mean.len();
    let mut s = format!("mean = [{}], covariance = [", mean.join(", "));
    if k == 0 || g.covariance.len() != k * k {
        s.push_str("]\n");
        return s;
    }
    for r in 0..k {
        for c in 0..k - 1 {
            s.push_str(&crate::cli::cstd::format_g(g.covariance[r * k + c]));
            s.push_str(", ");
        }
        s.push_str(&crate::cli::cstd::format_g(g.covariance[r * k + k - 1]));
        s.push_str(if r == k - 1 { "]\n" } else { "; " });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tensor_dimensions() {
        assert_eq!(tensor_dimension(3), 2);
        assert_eq!(tensor_dimension(6), 3);
        assert_eq!(tensor_dimension(10), 4);
        assert_eq!(tensor_dimension(1), 1);
    }

    #[test]
    fn float_to_unsigned_as_x86() {
        assert_eq!(cast_unsigned(3.0), 3);
        assert_eq!(cast_unsigned(f32::NAN), 0);
        assert_eq!(cast_unsigned(-1.0), u32::MAX as usize);
    }

    #[test]
    fn histogram_parzen_peaks_at_the_samples() {
        let mut h = HistogramParzenModel::new(1.0, 32);
        let samples: Vec<f32> = (0..400).map(|i| 10.0 + (i % 20) as f32 * 0.1).collect();
        let weights = vec![1.0f64; samples.len()];
        h.set_sample(&samples, 1, &weights).unwrap();
        let inside = h.evaluate(&[11.0]);
        let outside = h.evaluate(&[100.0]);
        assert!(inside > 0.0);
        assert_eq!(outside, 0.0);
    }

    fn two_clusters() -> Vec<f32> {
        (0..120)
            .map(|i| if i % 2 == 0 { 10.0 } else { 30.0 } + (i % 7) as f32 * 0.3)
            .collect()
    }

    #[test]
    fn manifold_parzen_follows_the_clusters_and_the_weights_when_fixed() {
        let samples = two_clusters();
        let weights: Vec<f64> = (0..samples.len())
            .map(|i| if i % 2 == 0 { 1.0 } else { 0.05 })
            .collect();
        let model = LikelihoodModel::ManifoldParzenWindows {
            regularization_sigma: 1.0,
            evaluation_k: 10,
            covariance_k: 0,
            kernel_sigma: 1.0,
        };
        let mut fixed = Likelihood::new(&model, UpstreamDefects::Fix);
        let mut ants = Likelihood::new(&model, UpstreamDefects::Reproduce);
        fixed.set_sample(&samples, 1, &weights).unwrap();
        ants.set_sample(&samples, 1, &weights).unwrap();
        assert!(ants.evaluate(&[11.0]) > ants.evaluate(&[20.0]));
        assert!(ants.evaluate(&[31.0]) > ants.evaluate(&[20.0]));
        // ANTs ignores the weights (ANTS-95): both clusters count alike.
        let ratio_ants = ants.evaluate(&[11.0]) / ants.evaluate(&[31.0]);
        let ratio_fixed = fixed.evaluate(&[11.0]) / fixed.evaluate(&[31.0]);
        assert!(ratio_fixed > 2.0 * ratio_ants, "{ratio_fixed} {ratio_ants}");
    }

    #[test]
    fn log_euclidean_tensors_prefer_their_class_mean() {
        // 2-D tensors (xx, xy, yy): an isotropic class.
        let mut samples = Vec::new();
        for i in 0..40 {
            let e = 1.0 + (i % 5) as f32 * 0.01;
            samples.extend_from_slice(&[e, 0.0, e]);
        }
        let weights = vec![1.0f64; 40];
        let mut m = Likelihood::new(&LikelihoodModel::LogEuclideanGaussian, UpstreamDefects::Fix);
        m.set_sample(&samples, 3, &weights).unwrap();
        assert!(m.evaluate(&[1.02, 0.0, 1.02]) > m.evaluate(&[3.0, 0.5, 0.4]));
    }

    #[test]
    fn joint_shape_is_evaluated_where_its_histograms_were_filled_when_fixed() {
        // 3-D tensors with a principal direction along x.
        let mut samples = Vec::new();
        for i in 0..60 {
            let a = 2.0 + (i % 6) as f32 * 0.05;
            samples.extend_from_slice(&[a, 0.0, 0.0, 1.0, 0.0, 0.5]);
        }
        let weights = vec![1.0f64; 60];
        let model = LikelihoodModel::JointShapeAndOrientationProbability {
            shape_sigma: 1.0,
            shape_bins: 16,
            orientation_sigma: 1.0,
            orientation_bins: 8,
        };
        let mut fixed = Likelihood::new(&model, UpstreamDefects::Fix);
        let mut ants = Likelihood::new(&model, UpstreamDefects::Reproduce);
        fixed.set_sample(&samples, 6, &weights).unwrap();
        ants.set_sample(&samples, 6, &weights).unwrap();
        let x = [2.1f32, 0.0, 0.0, 1.0, 0.0, 0.5];
        assert!(fixed.evaluate(&x) > 0.0);
        assert_ne!(fixed.evaluate(&x), ants.evaluate(&x));
        // A tensor along y has another orientation: less likely when fixed.
        let y = [1.0f32, 0.0, 0.0, 2.1, 0.0, 0.5];
        assert!(fixed.evaluate(&y) < fixed.evaluate(&x));
    }

    #[test]
    fn print_follows_ants_print_self() {
        let mut g = Likelihood::new(&LikelihoodModel::Gaussian, UpstreamDefects::Fix);
        g.set_sample(&[1.0, 2.0, 3.0, 4.0], 1, &[1.0; 4]).unwrap();
        let text = print(&g, "  ");
        assert!(
            text.starts_with("  GaussianListSampleFunction (0x0)\n"),
            "{text}"
        );
        assert!(
            text.contains("mean = [2.5], covariance = [1.66667]"),
            "{text}"
        );
    }
}
