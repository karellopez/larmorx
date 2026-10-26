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
use std::sync::atomic::{AtomicUsize, Ordering};

use larmorx_core::IndexFloor;
use rayon::prelude::*;

use crate::interp::{Linear, central_difference};
use crate::mask::Mask;
use crate::transform::Composite;
use crate::{Image, ItkException, Real, Space};

/// Points evaluated in parallel before they are consumed in order (at most).
const CHUNK: usize = 1 << 14;

/// The bytes of results a chunk may hold per thread ([`ImageMetricv4::for_each_sample_of`]).
const CHUNK_BYTES: usize = 1 << 20;

/// The fewest points per thread in a chunk.
const MIN_CHUNK: usize = 64;

/// `f` run in a pool of `n_threads` threads (0: every logical CPU), or in the current pool
/// when `None` (the caller installed the call's pool): never rayon's global pool, whose size
/// is the machine's, not the call's.
pub fn in_pool<T: Send>(
    n_threads: Option<usize>,
    f: impl FnOnce() -> T + Send,
) -> Result<T, ItkException> {
    match n_threads {
        None => Ok(f()),
        Some(n) => larmorx_core::parallel::with_threads(n, f).map_err(|e| {
            ItkException::new(
                "Modules/Core/Common/src/itkMultiThreaderBase.cxx",
                0,
                "MultiThreaderBase",
                e.to_string(),
            )
        }),
    }
}

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
    /// The moving transform being optimised is a displacement field on the virtual domain
    /// (`HasLocalSupport()`): SyN's identity field (`DownsampleImagesForMetricDerivatives`; pass
    /// an empty composite, which maps every point to itself with the identity as its Jacobian),
    /// or the field of a `GaussianDisplacementField` or `BSplineDisplacementField` stage (the
    /// composite's Jacobian, `D` local parameters). Each point's derivative is added to its
    /// voxel's `D` entries of a derivative over the whole virtual domain, which is not averaged
    /// (`StorePointDerivativeResult` for a transform of local support).
    pub local_support: bool,
    /// What becomes of the derivative of a sample point outside the virtual region
    /// (ITK-97, ITK-99), and how many there were.
    pub outside: OutsideDerivatives,
    /// The call's thread count: the metric's parallel loops run in a pool of that many
    /// threads (`larmorx_core::parallel::with_threads`; 0: every logical CPU). `None` when the
    /// caller already runs it inside its own pool (SyN, the velocity fields), which the loops
    /// then use. Never rayon's global pool, whose size is the machine's, not the call's.
    pub n_threads: Option<usize>,
}

/// The derivative of a sample point whose virtual index lies outside the virtual region
/// (a sampled metric with a shrink factor: the samples are drawn on the fixed image, the
/// virtual domain is the shrunk one). ITK's `StorePointDerivativeResult` takes the point's
/// parameters at `ComputeParameterOffsetFromVirtualIndex`, the region's linear offset of the
/// index, without a check: inside the derivative array it is another voxel's (ITK-97, the
/// derivative aliased), outside it a write past the array (ITK-99, memory). By default larmorx
/// drops such a derivative (the point's value still counts); `alias` (`reproduce`) adds it
/// where ITK does when that lies inside the array and drops the others. The counts (one per
/// point and evaluation) serve the notices; the clones of one value share them.
#[derive(Clone, Debug, Default)]
pub struct OutsideDerivatives {
    pub alias: bool,
    aliased: Arc<AtomicUsize>,
    beyond: Arc<AtomicUsize>,
    /// ITK-100: evaluations of a Mattes metric at a later level whose bin size is 0 or less
    /// (`alias`, as everything this value carries in `reproduce`: used as ITK uses it).
    stale_bins: Arc<AtomicUsize>,
}

impl OutsideDerivatives {
    /// ITK's aliasing (`reproduce`) or the derivative dropped (the default).
    pub fn new(alias: bool) -> Self {
        OutsideDerivatives {
            alias,
            ..Default::default()
        }
    }

    /// How many derivatives of points outside the region ITK adds to another voxel (ITK-97).
    pub fn aliased(&self) -> usize {
        self.aliased.load(Ordering::Relaxed)
    }

    /// How many ITK writes past its derivative array (ITK-99).
    pub fn beyond(&self) -> usize {
        self.beyond.load(Ordering::Relaxed)
    }

    /// How many evaluations met a later level's Mattes bin size of 0 or less (ITK-100).
    pub fn stale_bins(&self) -> usize {
        self.stale_bins.load(Ordering::Relaxed)
    }

    pub(crate) fn count_stale_bins(&self) {
        self.stale_bins.fetch_add(1, Ordering::Relaxed);
    }
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

/// A point's Jacobian with respect to the parameters, `D` rows of `n` columns: every local
/// parameter (`cols` `None`), or the parameters `cols` (every other column is zero).
#[derive(Clone, Debug)]
pub struct JacobianColumns<R> {
    pub n: usize,
    pub cols: Option<Vec<usize>>,
    pub values: Vec<R>,
}

impl<R: Copy> JacobianColumns<R> {
    /// The entry of row `dim`, column `c`.
    #[inline]
    pub fn at(&self, dim: usize, c: usize) -> R {
        self.values[dim * self.n + c]
    }

    /// The parameter of column `c`.
    #[inline]
    pub fn parameter(&self, c: usize) -> usize {
        self.cols.as_ref().map_or(c, |cols| cols[c])
    }
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

    /// The derivative values one sample keeps until its chunk is consumed
    /// ([`ImageMetricv4::jacobian_columns`]' columns): a B-spline transform's support, else
    /// [`ImageMetricv4::local_parameters`].
    #[inline]
    pub fn sample_width(&self, transform: &Composite<R, D>) -> usize {
        if self.local_support {
            D
        } else {
            transform
                .sparse_jacobian_width()
                .unwrap_or_else(|| transform.number_of_parameters())
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
        if self.local_support && transform.transforms.is_empty() {
            out.fill(R::ZERO);
            for i in 0..D {
                out[i * D + i] = R::ONE;
            }
        } else {
            transform.jacobian(vp, out);
        }
    }

    /// The Jacobian's columns at `vp` ([`ImageMetricv4::moving_jacobian`]): every local
    /// parameter's, or for a B-spline transform being optimised only those of the parameters
    /// around the point ([`Composite::sparse_jacobian`]; the others are zero).
    pub fn jacobian_columns(&self, transform: &Composite<R, D>, vp: &[R; D]) -> JacobianColumns<R> {
        if !self.local_support
            && let Some((cols, values)) = transform.sparse_jacobian(vp)
        {
            return JacobianColumns {
                n: cols.len(),
                cols: Some(cols),
                values,
            };
        }
        let n = self.local_parameters(transform);
        let mut values = vec![R::ZERO; D * n];
        self.moving_jacobian(transform, vp, &mut values);
        JacobianColumns {
            n,
            cols: None,
            values,
        }
    }

    /// `ComputeParameterOffsetFromVirtualIndex(index, D) / D`: the virtual voxel of `index`
    /// (`None` outside the virtual region, where ITK would write outside its derivative).
    #[inline]
    pub fn voxel_of(&self, index: &[i64; D]) -> Option<usize> {
        let space = &self.virtual_space;
        if space.contains(index) {
            return Some(space.offset(index));
        }
        // ComputeOffset of an index outside the region: the signed linear offset.
        let mut offset = 0i64;
        let mut stride = 1i64;
        for k in 0..D {
            offset += (index[k] - space.start[k]) * stride;
            stride *= space.size[k] as i64;
        }
        if (0..stride).contains(&offset) {
            self.outside.aliased.fetch_add(1, Ordering::Relaxed);
            self.outside.alias.then_some(offset as usize)
        } else {
            self.outside.beyond.fetch_add(1, Ordering::Relaxed);
            None
        }
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

    /// Runs `f` in the call's pool (`n_threads`), or in the current one when the caller
    /// installed it (`None`).
    pub fn in_pool<T: Send>(&self, f: impl FnOnce() -> T + Send) -> Result<T, ItkException> {
        in_pool(self.n_threads, f)
    }

    /// Evaluates `f` on every domain point in parallel, chunk by chunk, and passes the
    /// results that are `Some` to `consume` in domain order.
    pub fn for_each_sample<T: Send>(
        &self,
        f: impl Fn(usize) -> Option<T> + Sync,
        consume: impl FnMut(T) -> Result<(), ItkException> + Send,
    ) -> Result<(), ItkException> {
        self.for_each_sample_of(0, f, consume)
    }

    /// The points evaluated together when each keeps `width` derivative values and their
    /// columns: about `CHUNK_BYTES` of results per thread of the call, between `MIN_CHUNK`
    /// points per thread and `CHUNK` points.
    pub fn sample_chunk(&self, width: usize) -> usize {
        let threads = match self.n_threads {
            Some(t) => larmorx_core::parallel::resolve_threads(t),
            None => rayon::current_num_threads(),
        }
        .max(1);
        // A value, a column index and the tuple's own words per kept value.
        let per_sample = 64 + 16 * width;
        (CHUNK_BYTES * threads / per_sample)
            .clamp(MIN_CHUNK * threads, CHUNK.max(MIN_CHUNK * threads))
    }

    /// [`ImageMetricv4::for_each_sample`] for samples that each keep `width` derivative values
    /// (and their columns) until consumed: the chunks hold at most about `CHUNK_BYTES` per
    /// thread of results (a B-spline transform's samples keep its whole support: a fixed chunk
    /// of `CHUNK` of them held hundreds of MiB where ITK holds one sample per thread; the
    /// user's rule, never more memory than the original). The chunk size never changes a
    /// result: every sample is consumed in domain order.
    pub fn for_each_sample_of<T: Send>(
        &self,
        width: usize,
        f: impl Fn(usize) -> Option<T> + Sync,
        mut consume: impl FnMut(T) -> Result<(), ItkException> + Send,
    ) -> Result<(), ItkException> {
        let n = self.number_of_domain_points();
        let chunk = self.sample_chunk(width);
        let f = &f;
        // One entry into the call's pool for every chunk (the chunks consumed in order there).
        self.in_pool(move || {
            let mut start = 0;
            let mut buffer: Vec<Option<T>> = Vec::new();
            while start < n {
                let end = (start + chunk).min(n);
                buffer.clear();
                (start..end)
                    .into_par_iter()
                    .map(f)
                    .collect_into_vec(&mut buffer);
                for r in buffer.drain(..).flatten() {
                    consume(r)?;
                }
                start = end;
            }
            Ok(())
        })?
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
