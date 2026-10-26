// SPDX-License-Identifier: Apache-2.0
//! ITK's v4 image registration framework (ITK v5.4.5), as ANTs v2.6.5's `antsRegistration`
//! runs it, ported operation for operation so that a registration gives ANTs' parameters to
//! the bit when ANTs runs on one thread.
//!
//! - [`space`], an image's geometry and ITK's point ↔ index conversions (`ImageBase`).
//! - [`image`], an image of the computation type `R` on that geometry.
//! - [`rng`], ITK's Mersenne twister (`MersenneTwisterRandomVariateGenerator`) and the random
//!   image iterator (`ImageRandomConstIteratorWithIndex`).
//! - [`interp`], `LinearInterpolateImageFunction` and `CentralDifferenceImageFunction`.
//! - [`mask`], `ImageMaskSpatialObject::IsInsideInWorldSpace`.
//! - [`transform`], the transforms a linear stage optimises (`TranslationTransform`,
//!   `Euler2DTransform`, `Euler3DTransform`, `Similarity2DTransform`,
//!   `Similarity3DTransform`, `AffineTransform`, `ANTSAffine3DTransform`,
//!   `ANTSCenteredAffine2DTransform`), displacement fields ([`field`]), the cubic
//!   `BSplineTransform` ([`transform::bspline`]), and `CompositeTransform` with its Jacobian
//!   (sparse for a B-spline transform).
//! - [`field`], [`syn`], [`bspline_field`]: displacement fields (composition, inversion,
//!   Gaussian and B-spline smoothing on update), SyN and B-spline SyN, the B-spline
//!   approximation of fields and scattered points.
//! - [`metric`], the image metrics (`MattesMutualInformationImageToImageMetricv4`,
//!   `MeanSquares`, `Correlation`, `JointHistogramMutualInformation`, `Demons`,
//!   `ANTSNeighborhoodCorrelation`) and the virtual-domain sampling.
//! - [`optimizer`], `ConjugateGradientLineSearchOptimizerv4` with the scales of
//!   `RegistrationParameterScalesFromPhysicalShift` and `WindowConvergenceMonitoringFunction`.
//! - [`method`], `ImageRegistrationMethodv4`: the levels, their shrunk virtual domains and
//!   smoothed images, the metric's sample points, the level adaptors of displacement fields and
//!   B-spline transforms.
//!
//! # Precision and order
//!
//! Everything is generic over the computation type `R` (`f64`, ANTs' default, or `f32`,
//! `--float 1`) and the dimension `D` (2, 3 or 4), as ITK's templates are. Each expression is
//! evaluated in the types and the order ITK's source gives it; the sums ITK accumulates in a
//! loop over points are accumulated here in the order a single ITK thread visits the points,
//! so the result does not depend on the number of threads. What can run in parallel (each
//! point's interpolation, gradient and Jacobian) does, into buffers that are then reduced in
//! that order.

#![forbid(unsafe_code)]
// ITK's loops index several arrays at once; they are kept as written.
#![allow(clippy::needless_range_loop, clippy::manual_memcpy)]

pub mod bspline_field;
pub mod convergence;
pub mod exponential;
pub mod field;
pub mod format;
pub mod image;
pub mod initializer;
pub mod interp;
pub mod landmark;
pub mod mask;
pub mod method;
pub mod metric;
pub mod moments;
pub mod optimizer;
pub mod pointset;
pub mod preprocess;
pub mod resample;
pub mod rng;
pub mod space;
pub mod syn;
pub mod time_varying;
pub mod transform;
pub mod tv_bspline;

use larmorx_core::ItkReal;

pub use image::Image;
pub use space::Space;

/// The computation type of a registration: `f64` (ANTs' default) or `f32` (`--float 1`).
pub trait Real: ItkReal + Default + Send + Sync + 'static {
    /// `NumericTraits<R>::epsilon()`.
    const EPSILON: Self;
    /// `NumericTraits<R>::max()`.
    const MAX: Self;
    /// `std::abs`.
    fn abs(self) -> Self;
    /// `static_cast<R>(i)` of an integer.
    fn from_i64(i: i64) -> Self;
    /// `std::log` (glibc's; correctly rounded here).
    fn ln(self) -> Self;
    /// `std::exp` (glibc's `exp` or `expf`; correctly rounded here).
    fn exp(self) -> Self;
    /// `std::pow(x, y)`: glibc's `pow` or `powf` (correctly rounded here, CORE-MATH's).
    fn powr(self, y: Self) -> Self;
    /// `SmoothingRecursiveGaussianImageFilter<Image<R>, Image<R>>` (sigmas in physical
    /// units).
    fn smooth(
        data: &[Self],
        size: &[usize],
        spacing: &[f64],
        sigma: &[f64],
        n_threads: usize,
    ) -> Result<Vec<Self>, larmorx_image::FilterError>;

    /// `GradientRecursiveGaussianImageFilter<Image<R>>` with one sigma and
    /// `NormalizeAcrossScale` as given: the `D`-vectors (double) before the direction
    /// correction, written axis by axis (never the component images beside the vectors).
    fn gradient<const D: usize>(
        data: &[Self],
        size: &[usize],
        spacing: &[f64],
        sigma: f64,
        normalize_across_scale: bool,
        n_threads: usize,
    ) -> Result<Vec<[f64; D]>, larmorx_image::FilterError>;
}

impl Real for f64 {
    const EPSILON: f64 = f64::EPSILON;
    const MAX: f64 = f64::MAX;
    #[inline]
    fn abs(self) -> f64 {
        f64::abs(self)
    }
    #[inline]
    fn from_i64(i: i64) -> f64 {
        i as f64
    }
    #[inline]
    fn ln(self) -> f64 {
        larmorx_core::math::log(self)
    }
    #[inline]
    fn exp(self) -> f64 {
        larmorx_core::math::exp(self)
    }
    #[inline]
    fn powr(self, y: f64) -> f64 {
        larmorx_core::math::pow(self, y)
    }
    fn smooth(
        data: &[f64],
        size: &[usize],
        spacing: &[f64],
        sigma: &[f64],
        n_threads: usize,
    ) -> Result<Vec<f64>, larmorx_image::FilterError> {
        larmorx_image::gaussian::smoothing_recursive_gaussian_f64(
            larmorx_image::VolumeRef::new(data, size, spacing),
            sigma,
            false,
            n_threads,
        )
    }
    fn gradient<const D: usize>(
        data: &[f64],
        size: &[usize],
        spacing: &[f64],
        sigma: f64,
        normalize_across_scale: bool,
        n_threads: usize,
    ) -> Result<Vec<[f64; D]>, larmorx_image::FilterError> {
        larmorx_image::gaussian::gradient_recursive_gaussian_vectors_f64::<D>(
            larmorx_image::VolumeRef::new(data, size, spacing),
            sigma,
            normalize_across_scale,
            n_threads,
        )
    }
}

impl Real for f32 {
    const EPSILON: f32 = f32::EPSILON;
    const MAX: f32 = f32::MAX;
    #[inline]
    fn abs(self) -> f32 {
        f32::abs(self)
    }
    #[inline]
    fn from_i64(i: i64) -> f32 {
        i as f32
    }
    #[inline]
    fn ln(self) -> f32 {
        larmorx_core::math::logf(self)
    }
    #[inline]
    fn exp(self) -> f32 {
        larmorx_core::math::expf(self)
    }
    #[inline]
    fn powr(self, y: f32) -> f32 {
        larmorx_core::math::powf(self, y)
    }
    fn smooth(
        data: &[f32],
        size: &[usize],
        spacing: &[f64],
        sigma: &[f64],
        n_threads: usize,
    ) -> Result<Vec<f32>, larmorx_image::FilterError> {
        larmorx_image::gaussian::smoothing_recursive_gaussian(
            larmorx_image::VolumeRef::new(data, size, spacing),
            sigma,
            false,
            n_threads,
        )
    }
    fn gradient<const D: usize>(
        data: &[f32],
        size: &[usize],
        spacing: &[f64],
        sigma: f64,
        normalize_across_scale: bool,
        n_threads: usize,
    ) -> Result<Vec<[f64; D]>, larmorx_image::FilterError> {
        larmorx_image::gaussian::gradient_recursive_gaussian_vectors::<D>(
            larmorx_image::VolumeRef::new(data, size, spacing),
            sigma,
            normalize_across_scale,
            n_threads,
        )
    }
}

/// An exception ITK throws (`itkExceptionMacro`): where, which class, and its description.
/// [`ItkException::print`] gives what `operator<<` of an `itk::ExceptionObject` prints.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{description}")]
pub struct ItkException {
    /// The source file, as it would be in ITK's or ANTs' tree.
    pub file: &'static str,
    pub line: u32,
    /// The class that threw (`GetNameOfClass`), empty for an exception raised without one.
    pub class: &'static str,
    /// The description after ITK's `ITK ERROR: Class(0x..): ` prefix.
    pub description: String,
    /// `GetLocation()`: `"unknown"` (`ITK_LOCATION`) for `itkExceptionMacro`, else what the
    /// thrower set.
    pub location: &'static str,
}

impl ItkException {
    pub fn new(
        file: &'static str,
        line: u32,
        class: &'static str,
        description: impl Into<String>,
    ) -> Self {
        ItkException {
            file,
            line,
            class,
            description: description.into(),
            location: "unknown",
        }
    }

    /// The exception ITK throws on catching this one, `ExceptionObject(__FILE__, __LINE__,
    /// prefix + what())`: no class prefix, the location `Unknown`, and `what()` is the file,
    /// the line and the description.
    pub fn wrapped(&self, file: &'static str, line: u32, prefix: &str) -> ItkException {
        let description = if self.class.is_empty() {
            self.description.clone()
        } else {
            format!("ITK ERROR: {}(0x0): {}", self.class, self.description)
        };
        ItkException {
            file,
            line,
            class: "",
            description: format!("{prefix}{}:{}:\n{description}", self.file, self.line),
            location: "Unknown",
        }
    }

    /// An `ExceptionObject` built empty and given a description and a location (no file, line
    /// 0, no class prefix), as `itk::Versor` throws.
    pub fn located(location: &'static str, description: impl Into<String>) -> Self {
        ItkException {
            file: "",
            line: 0,
            class: "",
            description: description.into(),
            location,
        }
    }

    /// `ExceptionObject::Print`: a blank line, the class and address, the location, file, line
    /// and description, and a blank line. Object addresses are not reproducible; they are
    /// printed as `0x0`.
    pub fn print(&self) -> String {
        let description = if self.class.is_empty() {
            self.description.clone()
        } else {
            format!("ITK ERROR: {}(0x0): {}", self.class, self.description)
        };
        format!(
            "\nitk::ExceptionObject (0x0)\nLocation: \"{}\" \nFile: {}\nLine: {}\nDescription: {}\n\n",
            self.location, self.file, self.line, description
        )
    }
}

/// `itk::CompensatedSummation<R>` (Kahan summation, `itkCompensatedSummation.hxx`): the sum
/// and the compensation are held in `NumericTraits<R>::AccumulateType`, which is double for
/// both `float` and `double`; each element is converted to double before it is added.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CompensatedSum {
    sum: f64,
    compensation: f64,
}

impl CompensatedSum {
    /// `AddElement(element)`.
    #[inline]
    pub fn add(&mut self, element: f64) {
        let compensated = element - self.compensation;
        let temp = self.sum + compensated;
        self.compensation = (temp - self.sum) - compensated;
        self.sum = temp;
    }

    /// `AddElement(0)` `k` times. Adding zero is a function of the state that reaches a fixed
    /// point at once in practice (the compensation is below half an ulp of the sum), so the
    /// loop stops there: the state is exactly the one of `k` additions.
    pub fn add_zeros(&mut self, k: u64) {
        for _ in 0..k {
            let before = (self.sum.to_bits(), self.compensation.to_bits());
            self.add(0.0);
            if (self.sum.to_bits(), self.compensation.to_bits()) == before {
                break;
            }
        }
    }

    /// `GetSum()`.
    #[inline]
    pub fn sum(&self) -> f64 {
        self.sum
    }

    /// The compensation (`m_Compensation`), which `operator+=(const Self &)` adds first.
    #[inline]
    pub fn compensation(&self) -> f64 {
        self.compensation
    }
}

/// One compensated sum per parameter, each point adding its local derivative to every one
/// (`StorePointDerivativeResult` of a transform of global support). A B-spline transform's
/// derivative at a point is zero but for the parameters of the control points around it: those
/// are added ([`CompensatedSums::add_sparse`]), and the zeros the other parameters receive are
/// added when the parameter is next touched, or at [`CompensatedSums::flush`], with the same
/// result as adding them in order.
#[derive(Clone, Debug, Default)]
pub struct CompensatedSums {
    pub sums: Vec<CompensatedSum>,
    /// The points added so far, and for each parameter the last one that added to it (sparse
    /// additions only).
    points: u64,
    last: Vec<u64>,
}

impl CompensatedSums {
    pub fn new(n: usize) -> Self {
        CompensatedSums {
            sums: vec![CompensatedSum::default(); n],
            points: 0,
            last: Vec::new(),
        }
    }

    /// One point's derivative for every parameter.
    #[inline]
    pub fn add_dense<R: Real>(&mut self, local: &[R]) {
        self.points += 1;
        for (s, &d) in self.sums.iter_mut().zip(local) {
            s.add(d.to_f64());
        }
    }

    /// One point's derivative, zero but for the parameters `cols` (`local` their values).
    pub fn add_sparse<R: Real>(&mut self, cols: &[usize], local: &[R]) {
        if self.last.is_empty() {
            self.last = vec![self.points; self.sums.len()];
        }
        self.points += 1;
        for (&c, &d) in cols.iter().zip(local) {
            let s = &mut self.sums[c];
            s.add_zeros(self.points - 1 - self.last[c]);
            s.add(d.to_f64());
            self.last[c] = self.points;
        }
    }

    /// The zeros not yet added (after sparse additions).
    pub fn flush(&mut self) {
        if self.last.is_empty() {
            return;
        }
        for (s, &l) in self.sums.iter_mut().zip(&self.last) {
            s.add_zeros(self.points - l);
        }
        self.last = vec![self.points; self.sums.len()];
    }
}

/// A fixed-size vector of the dimension `D`.
pub type Vector<T, const D: usize> = [T; D];

/// A `D × D` matrix, row by row.
pub type Matrix<T, const D: usize> = [[T; D]; D];
