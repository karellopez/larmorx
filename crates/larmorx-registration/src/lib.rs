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
//!   `ANTSCenteredAffine2DTransform`), the transforms they compose with, and
//!   `CompositeTransform` with its Jacobian.
//! - [`metric`], the image metrics (`MattesMutualInformationImageToImageMetricv4`,
//!   `MeanSquares`, `Correlation`, `JointHistogramMutualInformation`, `Demons`,
//!   `ANTSNeighborhoodCorrelation`) and the virtual-domain sampling.
//! - [`optimizer`], `ConjugateGradientLineSearchOptimizerv4` with the scales of
//!   `RegistrationParameterScalesFromPhysicalShift` and `WindowConvergenceMonitoringFunction`.
//! - [`method`], `ImageRegistrationMethodv4`: the levels, their shrunk virtual domains and
//!   smoothed images, the metric's sample points.
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

pub mod convergence;
pub mod format;
pub mod image;
pub mod initializer;
pub mod interp;
pub mod mask;
pub mod method;
pub mod metric;
pub mod optimizer;
pub mod preprocess;
pub mod rng;
pub mod space;
pub mod transform;

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
    /// `SmoothingRecursiveGaussianImageFilter<Image<R>, Image<R>>` (sigmas in physical
    /// units).
    fn smooth(
        data: &[Self],
        size: &[usize],
        spacing: &[f64],
        sigma: &[f64],
        n_threads: usize,
    ) -> Result<Vec<Self>, larmorx_image::FilterError>;
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
            "\nitk::ExceptionObject (0x0)\nLocation: \"unknown\" \nFile: {}\nLine: {}\nDescription: {}\n\n",
            self.file, self.line, description
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

    /// `GetSum()`.
    #[inline]
    pub fn sum(&self) -> f64 {
        self.sum
    }
}

/// A fixed-size vector of the dimension `D`.
pub type Vector<T, const D: usize> = [T; D];

/// A `D × D` matrix, row by row.
pub type Matrix<T, const D: usize> = [[T; D]; D];
