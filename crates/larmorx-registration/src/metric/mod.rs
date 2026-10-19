// SPDX-License-Identifier: Apache-2.0
//! The image metrics (ITK v5.4.5 `Modules/Registration/Metricsv4`).
//!
//! [`ImageMetricv4`] holds what `ImageToImageMetricv4` holds for one evaluation: the fixed and
//! moving images (smoothed for the current level), their masks, the fixed transform, the
//! virtual domain and the sampled points, and evaluates one virtual point as
//! `ProcessVirtualPoint` does ([`ImageMetricv4::sample`]): the point mapped to the fixed
//! image and interpolated there, mapped by the moving transform and interpolated, with the
//! moving image's gradient (central differences, or the gradient image of
//! `GradientRecursiveGaussianImageFilter` with `useGradientFilter`).
//!
//! Each metric module turns the samples into its value and derivative. The samples are
//! computed in parallel, chunk by chunk, and consumed in the order a single ITK thread visits
//! the points ([`ImageMetricv4::for_each_sample`]): the dense domain in index order, the
//! sampled points in their order.

pub mod correlation;
pub mod demons;
pub mod joint_histogram;
pub mod mattes;
pub mod mean_squares;
pub mod neighborhood_correlation;

use std::sync::Arc;

use larmorx_core::IndexFloor;
use rayon::prelude::*;

use crate::interp::{Linear, central_difference};
use crate::mask::Mask;
use crate::transform::Composite;
use crate::{Image, ItkException, Real, Space};

/// Points evaluated in parallel before they are consumed in order.
const CHUNK: usize = 1 << 14;

/// The inputs of an image metric for one level.
#[derive(Clone, Debug)]
pub struct ImageMetricv4<R, const D: usize> {
    pub fixed: Arc<Image<R, D>>,
    pub moving: Arc<Image<R, D>>,
    pub fixed_mask: Option<Mask<D>>,
    pub moving_mask: Option<Mask<D>>,
    /// The fixed transform (`IdentityTransform` when `None`).
    pub fixed_transform: Option<Composite<R, D>>,
    /// The level's virtual domain.
    pub virtual_space: Space<D>,
    /// The sampled virtual points (`UseSampledPointSet`), or the whole domain when `None`.
    pub samples: Option<Arc<Vec<[f64; D]>>>,
    /// Gradient images of the fixed and moving images (`useGradientFilter`):
    /// `CovariantVector<double, D>` pixels.
    pub fixed_gradient_image: Option<Arc<Image<[f64; D], D>>>,
    pub moving_gradient_image: Option<Arc<Image<[f64; D], D>>>,
    /// The images' pixel type is `float` while the metric computes in `R` = double
    /// (`antsAI`, `antsAffineInitializer`): the interpolated values are stored in a
    /// `FixedImagePixelType` / `MovingImagePixelType` (`TransformAndEvaluateFixedPoint`), so
    /// they are rounded to float. In `antsRegistration` the pixel type is `R`.
    pub pixel_f32: bool,
    /// The moving transform is a displacement field transform on the virtual domain (SyN's
    /// identity field, `DownsampleImagesForMetricDerivatives`): it maps every point to itself
    /// (pass an empty composite), has `D` local parameters with the identity as their Jacobian,
    /// and each point's derivative is added to its voxel's `D` entries of a derivative over the
    /// whole virtual domain, which is not averaged (`StorePointDerivativeResult` for a
    /// transform of local support).
    pub local_support: bool,
}

/// One virtual point as the metric sees it (`ProcessVirtualPoint`'s locals).
#[derive(Clone, Copy, Debug)]
pub struct Sample<R, const D: usize> {
    pub virtual_index: [i64; D],
    pub virtual_point: [f64; D],
    pub fixed_point: [f64; D],
    pub fixed_value: R,
    pub fixed_gradient: [R; D],
    pub moving_point: [f64; D],
    pub moving_value: R,
    pub moving_gradient: [R; D],
}

/// Which gradients a metric uses (`GetGradientSourceIncludesFixed/Moving`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GradientSource {
    pub fixed: bool,
    pub moving: bool,
}

impl<R: Real, const D: usize> ImageMetricv4<R, D> {
    /// `GetNumberOfLocalParameters()`: `D` with local support, else the transform's.
    #[inline]
    pub fn local_parameters(&self, transform: &Composite<R, D>) -> usize {
        if self.local_support {
            D
        } else {
            transform.number_of_parameters()
        }
    }

    /// `GetNumberOfParameters()`: the size of the derivative (`D` per virtual voxel with local
    /// support).
    #[inline]
    pub fn number_of_parameters(&self, transform: &Composite<R, D>) -> usize {
        if self.local_support {
            D * self.virtual_space.len()
        } else {
            transform.number_of_parameters()
        }
    }

    /// `ComputeJacobianWithRespectToParametersCachedTemporaries(virtualPoint)`: `D` rows of
    /// [`ImageMetricv4::local_parameters`] values (the identity with local support).
    #[inline]
    pub fn moving_jacobian(&self, transform: &Composite<R, D>, vp: &[R; D], out: &mut [R]) {
        if self.local_support {
            out.fill(R::ZERO);
            for i in 0..D {
                out[i * D + i] = R::ONE;
            }
        } else {
            transform.jacobian(vp, out);
        }
    }

    /// `ComputeParameterOffsetFromVirtualIndex(index, D) / D`: the virtual voxel of `index`
    /// (`None` outside the virtual region, where ITK would write outside its derivative).
    #[inline]
    pub fn voxel_of(&self, index: &[i64; D]) -> Option<usize> {
        self.virtual_space
            .contains(index)
            .then(|| self.virtual_space.offset(index))
    }

    /// The number of domain points (`GetNumberOfDomainPoints`).
    pub fn number_of_domain_points(&self) -> usize {
        match &self.samples {
            Some(s) => s.len(),
            None => self.virtual_space.len(),
        }
    }

    /// The `i`-th domain point: its index and physical point.
    #[inline]
    pub fn domain_point(&self, i: usize) -> ([i64; D], [f64; D]) {
        match &self.samples {
            Some(s) => {
                let p = s[i];
                (self.virtual_space.point_to_index(&p), p)
            }
            None => {
                let idx = self.virtual_space.index_of(i);
                (idx, self.virtual_space.index_to_point(&idx))
            }
        }
    }

    /// `ProcessVirtualPoint` up to the metric's own `ProcessPoint`: `None` where the point
    /// falls outside a mask or an image. `compute_derivative` asks for the gradients of
    /// `gradients`.
    pub fn sample(
        &self,
        i: usize,
        moving: &Composite<R, D>,
        gradients: GradientSource,
        compute_derivative: bool,
        floor: &IndexFloor,
    ) -> Option<Sample<R, D>> {
        let (virtual_index, virtual_point) = self.domain_point(i);
        self.sample_at(
            &virtual_index,
            &virtual_point,
            moving,
            gradients,
            compute_derivative,
            floor,
        )
    }

    /// [`ImageMetricv4::sample`] for the virtual point `virtual_point` (of index
    /// `virtual_index`).
    pub fn sample_at(
        &self,
        virtual_index: &[i64; D],
        virtual_point: &[f64; D],
        moving: &Composite<R, D>,
        gradients: GradientSource,
        compute_derivative: bool,
        floor: &IndexFloor,
    ) -> Option<Sample<R, D>> {
        let (virtual_index, virtual_point) = (*virtual_index, *virtual_point);
        let as_r = |p: &[f64; D]| -> [R; D] { std::array::from_fn(|k| R::from_f64(p[k])) };
        let as_f64 = |p: &[R; D]| -> [f64; D] { std::array::from_fn(|k| p[k].to_f64()) };
        let local = as_r(&virtual_point);
        // TransformAndEvaluateFixedPoint.
        let fixed_point = match &self.fixed_transform {
            Some(t) => as_f64(&t.transform_point(&local)),
            None => as_f64(&local),
        };
        if let Some(m) = &self.fixed_mask
            && !m.is_inside(&fixed_point)
        {
            return None;
        }
        let fixed_interp = Linear::new(&*self.fixed);
        let fixed_r = as_r(&fixed_point);
        let fixed_c: [R; D] = self.fixed.space.point_to_cindex(&fixed_point);
        if !self.fixed.space.is_inside_buffer(&fixed_c) {
            return None;
        }
        let fixed_value = R::from_f64(self.pixel(fixed_interp.at_cindex(&fixed_c, floor)));
        let mut fixed_gradient = [R::ZERO; D];
        if compute_derivative && gradients.fixed {
            fixed_gradient =
                self.gradient(&fixed_interp, &self.fixed_gradient_image, &fixed_r, floor);
        }
        // TransformAndEvaluateMovingPoint.
        let moving_r = moving.transform_point(&local);
        let moving_point = as_f64(&moving_r);
        if let Some(m) = &self.moving_mask
            && !m.is_inside(&moving_point)
        {
            return None;
        }
        let moving_interp = Linear::new(&*self.moving);
        let moving_c: [R; D] = self.moving.space.point_to_cindex(&moving_point);
        if !self.moving.space.is_inside_buffer(&moving_c) {
            return None;
        }
        let moving_value = R::from_f64(self.pixel(moving_interp.at_cindex(&moving_c, floor)));
        let mut moving_gradient = [R::ZERO; D];
        if compute_derivative && gradients.moving {
            moving_gradient = self.gradient(
                &moving_interp,
                &self.moving_gradient_image,
                &moving_r,
                floor,
            );
        }
        Some(Sample {
            virtual_index,
            virtual_point,
            fixed_point,
            fixed_value,
            fixed_gradient,
            moving_point,
            moving_value,
            moving_gradient,
        })
    }

    /// An interpolated value stored in the pixel type.
    #[inline]
    fn pixel(&self, v: f64) -> f64 {
        if self.pixel_f32 {
            f64::from(v as f32)
        } else {
            v
        }
    }

    /// `ComputeFixed/MovingImageGradientAtPoint`: the gradient image interpolated linearly
    /// (`useGradientFilter`), else central differences, stored in `R`.
    fn gradient(
        &self,
        interp: &Linear<'_, R, D>,
        image: &Option<Arc<Image<[f64; D], D>>>,
        p: &[R; D],
        floor: &IndexFloor,
    ) -> [R; D] {
        match image {
            Some(g) => {
                let pf: [f64; D] = std::array::from_fn(|k| p[k].to_f64());
                crate::interp::vector_linear(g, &pf, floor)
            }
            None => {
                let g = central_difference(interp, p, floor);
                std::array::from_fn(|k| R::from_f64(g[k]))
            }
        }
    }

    /// Evaluates `f` on every domain point in parallel, chunk by chunk, and passes the
    /// results that are `Some` to `consume` in domain order.
    pub fn for_each_sample<T: Send>(
        &self,
        f: impl Fn(usize) -> Option<T> + Sync,
        mut consume: impl FnMut(T) -> Result<(), ItkException>,
    ) -> Result<(), ItkException> {
        let n = self.number_of_domain_points();
        let mut start = 0;
        let mut buffer: Vec<Option<T>> = Vec::new();
        while start < n {
            let end = (start + CHUNK).min(n);
            buffer.clear();
            (start..end)
                .into_par_iter()
                .map(&f)
                .collect_into_vec(&mut buffer);
            for r in buffer.drain(..).flatten() {
                consume(r)?;
            }
            start = end;
        }
        Ok(())
    }
}

/// `ObjectToObjectMetric::VerifyNumberOfValidPoints`'s warning, as ITK's output window
/// prints it (the object address is not reproducible; `0x0`).
pub fn no_valid_points_warning(file: &str, line: u32, class: &str) -> String {
    format!(
        "WARNING: In {file}, line {line}\n{class} (0x0): No valid points were found during metric evaluation. \
         For image metrics, verify that the images overlap appropriately. For instance, you can align the \
         image centers by translation. For point-set metrics, verify that the fixed points, once transformed \
         into the virtual domain space, actually lie within the virtual domain.\n\n"
    )
}
