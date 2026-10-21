// SPDX-License-Identifier: Apache-2.0
//! The point-set metrics of `antsRegistration` (ITK v5.4.5, `Modules/Registration/Metricsv4/
//! include/`; ANTs v2.6.5, `Temporary/itkMeanSquaresPointSetToPointSetIntensityMetricv4.hxx`):
//!
//! - `ICP`: `EuclideanDistancePointSetToPointSetMetricv4`, the distance to the nearest moving
//!   point;
//! - `PSE`: `ExpectationBasedPointSetToPointSetMetricv4`, a Gaussian expectation over the `k`
//!   nearest moving points;
//! - `JHCT`: `JensenHavrdaCharvatTsallisPointSetToPointSetMetricv4` with a manifold Parzen
//!   density of the moving points;
//!
//! each wrapped in `LabeledPointSetToPointSetMetricv4` (one metric per label common to both
//! point sets), and `IGDM`: ANTs' `MeanSquaresPointSetToPointSetIntensityMetricv4`, the
//! intensity and distance of the nearest moving point.
//!
//! What they share is `PointSetToPointSetMetricWithIndexv4`: the fixed points are mapped into
//! the virtual domain by the inverse of the fixed transform and on into the moving domain by
//! the moving transform (both stored as `Point<float>`); the points whose virtual position is
//! outside the virtual domain take no part; the value is the mean of the points' values, the
//! derivative the mean of their local derivatives mapped by the moving transform's Jacobian at
//! the virtual point. Sums are ITK's compensated sums over the points in order, as one ITK
//! thread computes them, so the result does not depend on the number of threads.

use std::sync::Arc;

use super::gaussian::{ParzenSettings, ParzenWindows, closest_n};
use super::kdtree::KdTree;
use crate::transform::{Composite, Xfm};
use crate::{CompensatedSum, ItkException, Real, Space};

const BASE_FILE: &str =
    "Modules/Registration/Metricsv4/include/itkPointSetToPointSetMetricWithIndexv4.hxx";
const LABELED_FILE: &str =
    "Modules/Registration/Metricsv4/include/itkLabeledPointSetToPointSetMetricv4.hxx";
const LOCATOR_FILE: &str = "Modules/Registration/Common/include/itkPointsLocator.hxx";
const IGDM_FILE: &str = "Temporary/itkMeanSquaresPointSetToPointSetIntensityMetricv4.hxx";
const VERIFY_FILE: &str = "Modules/Numerics/Optimizersv4/include/itkObjectToObjectMetric.hxx";

/// A labelled point set (`PointSet<unsigned int, D>`, `float` coordinates).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LabeledPointSet<const D: usize> {
    pub points: Vec<[f32; D]>,
    /// One label per point. `None`: no point data at all (ITK's null container, as a reader
    /// leaves an empty set).
    pub labels: Option<Vec<u32>>,
}

/// An intensity point set (`PointSet<Array<R>, D>`): each point's neighbourhood intensities
/// and gradients, `[I, ∂I/∂x…]` per neighbourhood voxel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IntensityPointSet<R, const D: usize> {
    pub points: Vec<[f32; D]>,
    pub data: Vec<Vec<R>>,
}

/// A point-set metric and its settings, as `RegistrationHelper::DoRegistration` configures it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointSetMetricKind<R> {
    Icp,
    /// `SetPointSetSigma` (a `float`), `SetEvaluationKNeighborhood`.
    Pse {
        sigma: f32,
        k: usize,
    },
    /// `SetPointSetSigma`, `SetKernelSigma(10)`, `SetUseAnisotropicCovariances`,
    /// `SetCovarianceKNeighborhood(5)`, `SetEvaluationKNeighborhood`, `SetAlpha`.
    Jhct {
        sigma: R,
        k: usize,
        alpha: R,
        anisotropic: bool,
    },
    /// The intensity and Euclidean distance sigmas, estimated when not above 0.
    Igdm {
        intensity_sigma: R,
        euclidean_sigma: R,
    },
}

impl<R> PointSetMetricKind<R> {
    /// `GetNameOfClass()` of the metric `antsRegistration` hands to the registration method.
    pub fn class(&self) -> &'static str {
        match self {
            PointSetMetricKind::Igdm { .. } => "MeanSquaresPointSetToPointSetIntensityMetricv4",
            _ => "LabeledPointSetToPointSetMetricv4",
        }
    }
}

/// The point sets of a metric.
#[derive(Clone, Debug)]
pub enum PointSetInputs<R, const D: usize> {
    Labeled {
        fixed: Arc<LabeledPointSet<D>>,
        moving: Arc<LabeledPointSet<D>>,
    },
    Intensity {
        fixed: Arc<IntensityPointSet<R, D>>,
        moving: Arc<IntensityPointSet<R, D>>,
    },
}

impl<R, const D: usize> PointSetInputs<R, D> {
    /// The fixed points (the scales estimator's samples when the metric comes first).
    pub fn fixed_points(&self) -> &[[f32; D]] {
        match self {
            PointSetInputs::Labeled { fixed, .. } => &fixed.points,
            PointSetInputs::Intensity { fixed, .. } => &fixed.points,
        }
    }
}

/// larmorx's corrections of ITK's and ANTs' point-set metrics (D13); `Default` is the
/// originals' behaviour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointSetCorrections {
    /// ITK-22: a fixed point whose label the moving set lacks takes no part (ITK throws
    /// "Label not found in common label set").
    pub skip_uncommon_labels: bool,
    /// ITK-23: a Gaussian whose covariance is the identity gets the `D`-dimensional
    /// prefactor (ITK keeps the univariate `1/√(2π)`).
    pub normalise_identity: bool,
    /// ITK-26: each product enters a local covariance's diagonal once (ITK adds it twice).
    pub single_diagonal: bool,
    /// ITK-28: JHCT's prefactor and value test `α = 1` alike (`AlmostEquals`); ITK tests the
    /// prefactor with `!=`.
    pub alpha_test: bool,
    /// ANTS-65: IGDM's estimated intensity sigma is the range of both point sets' centre
    /// intensities.
    pub igdm_intensity_range: bool,
    /// ANTS-66: IGDM's value reads the neighbourhood's centre intensity as its derivative
    /// does (ANTs computes a negative index for neighbourhoods of more than one voxel).
    pub igdm_value_centre: bool,
    /// ITK-32: a SyN evaluation looks its points up in the label metrics of the same
    /// `Initialize` (ITK appends new ones at every `Initialize` and keeps using the first).
    pub current_clones: bool,
}

impl PointSetCorrections {
    /// Every correction on (`fix`) or off.
    pub fn all(fix: bool) -> Self {
        PointSetCorrections {
            skip_uncommon_labels: fix,
            normalise_identity: fix,
            single_diagonal: fix,
            alpha_test: fix,
            igdm_intensity_range: fix,
            igdm_value_centre: fix,
            current_clones: fix,
        }
    }
}

/// Where the originals' defects met this registration (for larmorx's notices).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointSetEvents {
    /// ITK-22: a fixed point inside the virtual domain has a label the moving set lacks.
    pub uncommon_labels: bool,
    /// ITK-23: a Gaussian's covariance was the identity.
    pub identity_gaussians: bool,
    /// ITK-26: a local covariance was accumulated.
    pub covariances: bool,
    /// ITK-28: `α` is within 4 ulps of 1 without being 1.
    pub alpha_near_one: bool,
    /// ITK-25: a label's points are fewer than the covariance neighbourhood.
    pub short_neighbourhood: bool,
    /// ANTS-65: the estimated intensity sigma, ANTs' and the range of the intensities.
    pub igdm_sigma: Option<(f64, f64)>,
    /// ANTS-66: IGDM's value with a neighbourhood of more than one voxel.
    pub igdm_centre: bool,
    /// ITK-32: a SyN evaluation used label metrics whose moving points differ from the
    /// current ones.
    pub stale_clones: bool,
}

impl PointSetEvents {
    /// Both events together.
    pub fn merge(&mut self, o: &PointSetEvents) {
        self.uncommon_labels |= o.uncommon_labels;
        self.identity_gaussians |= o.identity_gaussians;
        self.covariances |= o.covariances;
        self.alpha_near_one |= o.alpha_near_one;
        self.short_neighbourhood |= o.short_neighbourhood;
        self.igdm_centre |= o.igdm_centre;
        self.stale_clones |= o.stale_clones;
        if self.igdm_sigma.is_none() {
            self.igdm_sigma = o.igdm_sigma;
        }
    }
}

/// `PointsLocator::Initialize()`'s exception for an empty point set.
fn no_points() -> ItkException {
    ItkException::new(
        LOCATOR_FILE,
        41,
        "PointsLocator",
        "The number of points is 0.",
    )
}

fn tree<const D: usize>(points: &[[f32; D]]) -> Result<KdTree<D>, ItkException> {
    if points.is_empty() {
        return Err(no_points());
    }
    KdTree::new(points.to_vec()).map_err(|_| {
        ItkException::new(
            "Modules/Numerics/Statistics/include/itkKdTreeGenerator.hxx",
            150,
            "KdTreeGenerator",
            "a point coordinate is not a number",
        )
    })
}

/// `Point::SquaredEuclideanDistanceTo`: the coordinates' differences in double.
#[inline]
fn squared_distance<const D: usize>(a: &[f32; D], b: &[f32; D]) -> f64 {
    let mut sum = 0.0f64;
    for d in 0..D {
        let diff = f64::from(a[d]) - f64::from(b[d]);
        sum += diff * diff;
    }
    sum
}

/// `Point::EuclideanDistanceTo`.
#[inline]
fn euclidean_distance<const D: usize>(a: &[f32; D], b: &[f32; D]) -> f64 {
    squared_distance(a, b).sqrt()
}

/// One label's metric (a clone of the prototype metric, `LabeledPointSetToPointSetMetricv4`).
#[derive(Clone, Debug)]
enum LabelMetric<R, const D: usize> {
    Icp {
        tree: KdTree<D>,
    },
    Pse {
        tree: KdTree<D>,
        prefactor: R,
        denominator: R,
        k: usize,
    },
    Jhct {
        density: Box<ParzenWindows<D>>,
        total: R,
        prefactor0: R,
        prefactor1: R,
        alpha: R,
        k: usize,
        anisotropic: bool,
    },
}

/// `Math::AlmostEquals` of two values of `R` (`FloatAlmostEqual`: 4 ulps, or an absolute
/// difference of at most `0.1·ε`).
fn almost_equals<R: Real>(a: R, b: R) -> bool {
    match R::PRECISION {
        larmorx_core::Precision::Double => {
            larmorx_image::itk_math::almost_equal_f64(a.to_f64(), b.to_f64())
        }
        larmorx_core::Precision::Float => {
            larmorx_image::itk_math::almost_equal_f32(a.to_f64() as f32, b.to_f64() as f32)
        }
    }
}

impl<R: Real, const D: usize> LabelMetric<R, D> {
    /// The clone's `Initialize()`: its moving points' locator (and its constants).
    fn new(
        kind: &PointSetMetricKind<R>,
        moving: &[[f32; D]],
        corrections: &PointSetCorrections,
        events: &mut PointSetEvents,
        warnings: &mut Vec<String>,
    ) -> Result<Self, ItkException> {
        Ok(match *kind {
            PointSetMetricKind::Icp => LabelMetric::Icp {
                tree: tree(moving)?,
            },
            PointSetMetricKind::Pse { sigma, k } => {
                let tree = tree(moving)?;
                if sigma <= f32::EPSILON {
                    return Err(ItkException::new(
                        "Modules/Registration/Metricsv4/include/itkExpectationBasedPointSetToPointSetMetricv4.hxx",
                        44,
                        "ExpectationBasedPointSetToPointSetMetricv4",
                        "m_PointSetSigma is too small. <= epsilon",
                    ));
                }
                let sqrt_two_pi = (2.0 * std::f64::consts::PI).sqrt();
                let prefactor = R::from_f64(1.0 / (sqrt_two_pi * f64::from(sigma)));
                let denominator = R::from_f64(2.0 * f64::from(sigma * sigma));
                LabelMetric::Pse {
                    tree,
                    prefactor,
                    denominator,
                    k,
                }
            }
            PointSetMetricKind::Jhct {
                sigma,
                k,
                alpha,
                anisotropic,
            } => {
                let settings = ParzenSettings {
                    kernel_sigma: R::from_f64(10.0),
                    regularization_sigma: sigma,
                    covariance_k: 5,
                    evaluation_k: k,
                    anisotropic,
                    normalize: true,
                    normalise_identity: corrections.normalise_identity,
                    single_diagonal: corrections.single_diagonal,
                };
                let mut density_events = super::gaussian::ParzenEvents::default();
                let density = ParzenWindows::new(moving, &settings, warnings, &mut density_events)?;
                events.identity_gaussians |= density_events.identity;
                events.covariances |= density_events.covariances;
                events.short_neighbourhood |= density_events.short_neighbourhood;
                // m_NumberOfValidPoints is 0 here: the clone never runs InitializeForIteration.
                let total = R::from_i64(moving.len() as i64);
                let mut prefactor0 = -R::ONE / total;
                let near_one = almost_equals(alpha, R::ONE);
                if alpha != R::ONE && near_one {
                    events.alpha_near_one = true;
                }
                let one = if corrections.alpha_test {
                    near_one
                } else {
                    alpha == R::ONE
                };
                if !one {
                    prefactor0 = prefactor0 / (alpha - R::ONE);
                }
                let prefactor1 = R::ONE / (total * total);
                LabelMetric::Jhct {
                    density: Box::new(density),
                    total,
                    prefactor0,
                    prefactor1,
                    alpha,
                    k,
                    anisotropic,
                }
            }
            PointSetMetricKind::Igdm { .. } => unreachable!("IGDM is not a labelled metric"),
        })
    }

    /// `GetLocalNeighborhoodValue(point)`.
    fn value(&self, point: &[f32; D], warnings: &mut Vec<String>) -> R {
        match self {
            LabelMetric::Icp { tree } => {
                let closest = &tree.points()[tree.closest(point)];
                // m_DistanceThreshold is -1: every distance counts.
                R::from_f64(euclidean_distance(point, closest))
            }
            LabelMetric::Pse {
                tree,
                prefactor,
                denominator,
                k,
            } => {
                let mut local = CompensatedSum::default();
                for j in closest_n(tree, point, *k, warnings) {
                    let distance = R::from_f64(squared_distance(point, &tree.points()[j]));
                    let term = *prefactor * (-distance / *denominator).exp();
                    local.add(-term.to_f64());
                }
                R::from_f64(local.sum())
            }
            LabelMetric::Jhct { .. } => self.jhct(point, false, warnings).0,
        }
    }

    /// `GetLocalNeighborhoodValueAndDerivative(point)`.
    fn value_and_derivative(&self, point: &[f32; D], warnings: &mut Vec<String>) -> (R, [R; D]) {
        match self {
            LabelMetric::Icp { tree } => {
                let closest = &tree.points()[tree.closest(point)];
                let distance = euclidean_distance(point, closest);
                let local = std::array::from_fn(|d| R::from_f64(f64::from(closest[d] - point[d])));
                (R::from_f64(distance), local)
            }
            LabelMetric::Pse {
                tree,
                prefactor,
                denominator,
                k,
            } => {
                let mut local = [R::ZERO; D];
                let neighbours = closest_n(tree, point, *k, warnings);
                let mut values = vec![R::ZERO; *k];
                let mut sum = CompensatedSum::default();
                for (i, &j) in neighbours.iter().enumerate() {
                    let distance = R::from_f64(squared_distance(point, &tree.points()[j]));
                    values[i] = -*prefactor * (-distance / *denominator).exp();
                    sum.add(values[i].to_f64());
                }
                let measure = R::from_f64(sum.sum());
                if measure.abs() <= R::EPSILON {
                    return (measure, local);
                }
                // Point<float> weightedPoint += neighbour · float(value) / measure.
                let mut weighted = [0.0f32; D];
                for (i, &j) in neighbours.iter().enumerate() {
                    let n = &tree.points()[j];
                    let v = values[i].to_f64() as f32;
                    for d in 0..D {
                        let scaled = n[d] * v;
                        weighted[d] += (R::from_f64(f64::from(scaled)) / measure).to_f64() as f32;
                    }
                }
                let distance = R::from_f64(squared_distance(point, &weighted));
                let weight = *prefactor * (-distance / *denominator).exp() / -measure;
                let w = weight.to_f64() as f32;
                for d in 0..D {
                    local[d] = R::from_f64(f64::from((weighted[d] - point[d]) * w));
                }
                (measure, local)
            }
            LabelMetric::Jhct { .. } => {
                let (v, d) = self.jhct(point, true, warnings);
                (v, d)
            }
        }
    }

    /// `ComputeValueAndDerivative(samplePoint, value, derivative, true, derivative)`.
    fn jhct(&self, point: &[f32; D], derivative: bool, warnings: &mut Vec<String>) -> (R, [R; D]) {
        let LabelMetric::Jhct {
            density,
            total,
            prefactor0,
            prefactor1,
            alpha,
            k,
            anisotropic,
        } = self
        else {
            unreachable!()
        };
        let mut out = [R::ZERO; D];
        let moving = R::from_i64(density.gaussians.len() as i64);
        let mut p_star = density.evaluate::<R>(point) * moving;
        p_star = p_star / *total;
        if almost_equals(p_star, R::ZERO) {
            return (R::ZERO, out);
        }
        let mut value = if almost_equals(*alpha, R::ONE) {
            p_star.ln()
        } else {
            R::ONE * p_star.powr(*alpha - R::ONE)
        };
        value = value * *prefactor0;
        if derivative {
            let factor_star = p_star.powr(R::from_f64(2.0) - *alpha);
            for j in closest_n(&density.tree, point, *k, warnings) {
                let g = &density.gaussians[j];
                let gaussian = R::from_f64(g.evaluate(point));
                if almost_equals(gaussian, R::ZERO) {
                    continue;
                }
                // Array<float> diffMean: the double mean minus the float point, stored in float.
                let mut diff: [f32; D] =
                    std::array::from_fn(|i| (g.mean[i] - f64::from(point[i])) as f32);
                if *anisotropic {
                    // VariableSizeMatrix<double> × Array<double>(diffMean), stored back in float.
                    let ci = &g.inverse_covariance;
                    let prod: [f32; D] = std::array::from_fn(|r| {
                        let mut sum = 0.0f64;
                        for c in 0..D {
                            sum += ci[r][c] * f64::from(diff[c]);
                        }
                        sum as f32
                    });
                    diff = prod;
                } else {
                    let c00 = g.covariance[0][0] as f32;
                    for v in diff.iter_mut() {
                        *v /= c00;
                    }
                }
                let factor = *prefactor1 * gaussian / factor_star;
                for i in 0..D {
                    out[i] = out[i] + R::from_f64(f64::from(diff[i])) * factor;
                }
            }
        }
        (value, out)
    }
}

/// The fixed transform's inverse (`m_FixedTransform->GetInverseTransform()`): the identity, or
/// the inverted composite.
#[derive(Clone, Debug)]
pub struct FixedInverse<R, const D: usize>(Option<Composite<R, D>>);

impl<R: Real, const D: usize> FixedInverse<R, D> {
    /// The inverse of `fixed` (`None`: `IdentityTransform`); `None` if a transform cannot be
    /// inverted (ITK returns a null transform and crashes on it).
    pub fn of(fixed: Option<&Composite<R, D>>) -> Option<Self> {
        let Some(c) = fixed else {
            return Some(FixedInverse(None));
        };
        let mut out = Vec::with_capacity(c.transforms.len());
        for t in c.transforms.iter().rev() {
            out.push(match t {
                Xfm::Identity => Xfm::Identity,
                Xfm::Translation { offset } => Xfm::Translation {
                    offset: std::array::from_fn(|k| -offset[k]),
                },
                Xfm::Linear(l) => Xfm::Linear(l.inverse()?),
                Xfm::Field(f) => Xfm::Field(f.inverse_transform()?),
                Xfm::BSpline(_) => return None,
            });
        }
        Some(FixedInverse(Some(Composite::new(out))))
    }

    #[inline]
    fn apply(&self, p: &[f32; D]) -> [f32; D] {
        match &self.0 {
            None => *p,
            Some(c) => {
                let q: [R; D] = std::array::from_fn(|k| R::from_f64(f64::from(p[k])));
                let r = c.transform_point(&q);
                std::array::from_fn(|k| r[k].to_f64() as f32)
            }
        }
    }
}

/// ANTs' IGDM state at a level: the moving points' locator and the sigmas.
#[derive(Clone, Debug)]
struct Igdm<R, const D: usize> {
    tree: KdTree<D>,
    intensity_sigma: R,
    euclidean_sigma: R,
}

/// A point-set metric of a stage: its inputs and the state ITK keeps from level to level.
#[derive(Clone, Debug)]
pub struct PointSetMetric<R, const D: usize> {
    pub kind: PointSetMetricKind<R>,
    pub inputs: PointSetInputs<R, D>,
    pub corrections: PointSetCorrections,
    /// `m_HaveWarnedAboutNumberOfValidPoints`: the warning is printed once per stage.
    have_warned: bool,
    /// The level's virtual domain (`SetVirtualDomainFromImage` of the shrunk domain).
    virtual_space: Option<Space<D>>,
    fixed_inverse: FixedInverse<R, D>,
    /// The common labels and one metric per label (labelled metrics).
    labels: Vec<u32>,
    clones: Vec<LabelMetric<R, D>>,
    igdm: Option<Igdm<R, D>>,
    /// The label metrics of the stage's first SyN `Initialize`, with their moving points
    /// (ITK-32: ITK keeps using them).
    syn_clones: Option<SynClones<R, D>>,
    /// ITK-32 corrected: the label metrics of the latest `Initialize`.
    syn_current: Option<Vec<LabelMetric<R, D>>>,
    /// The state SyN's `Initialize()` leaves for the evaluation.
    tangent: Option<TangentState<R, D>>,
    /// The tangent-space derivative's layout (`TangentCall::sparse`).
    sparse: bool,
    /// Where the originals' defects met this metric.
    pub events: PointSetEvents,
}

/// Label metrics and the moving points each was built from.
type SynClones<R, const D: usize> = (Vec<Vec<[f32; D]>>, Vec<LabelMetric<R, D>>);

/// One of SyN's evaluations of a point-set metric (`ComputeMetricGradientField`): in tangent
/// space (both point sets mapped into the virtual domain by the inverses of their transforms),
/// each point's derivative added to the virtual voxel nearest it
/// (`SetCalculateValueAndDerivativeInTangentSpace(true)`,
/// `SetStoreDerivativeAsSparseFieldForLocalSupportTransforms(true)`).
pub struct TangentCall<'a, R, const D: usize> {
    /// The point sets exchanged: the moving set in the fixed role (the moving-to-middle
    /// update).
    pub swapped: bool,
    /// The metric's fixed and moving transforms.
    pub fixed_transform: &'a Composite<R, D>,
    pub moving_transform: &'a Composite<R, D>,
    /// The level's virtual domain.
    pub virtual_space: &'a Space<D>,
    /// `SetStoreDerivativeAsSparseFieldForLocalSupportTransforms`: each point's derivative
    /// added at its nearest virtual voxel (SyN), or stored at the point's index, `D` values per
    /// fixed point (the B-spline SyN method, which fits a B-spline to the points).
    pub sparse: bool,
}

/// What one evaluation gives.
pub struct PointSetValue<R> {
    pub value: R,
    pub derivative: Vec<R>,
}

/// The fixed points in the virtual domain, the same points mapped into the moving space, and
/// the number of valid points.
type TransformedPoints<const D: usize> = (Vec<[f32; D]>, Vec<[f32; D]>, usize);

impl<R: Real, const D: usize> PointSetMetric<R, D> {
    pub fn new(
        kind: PointSetMetricKind<R>,
        inputs: PointSetInputs<R, D>,
        corrections: PointSetCorrections,
    ) -> Self {
        PointSetMetric {
            kind,
            inputs,
            corrections,
            have_warned: false,
            virtual_space: None,
            fixed_inverse: FixedInverse(None),
            labels: Vec::new(),
            clones: Vec::new(),
            igdm: None,
            syn_clones: None,
            syn_current: None,
            tangent: None,
            sparse: true,
            events: PointSetEvents::default(),
        }
    }

    pub fn class(&self) -> &'static str {
        self.kind.class()
    }

    /// The fixed points in the virtual domain and in the moving domain
    /// (`TransformFixedAndCreateVirtualPointSet`).
    fn transform_fixed(&self, moving: &Composite<R, D>) -> (Vec<[f32; D]>, Vec<[f32; D]>) {
        let fixed = self.inputs.fixed_points();
        let mut virt = Vec::with_capacity(fixed.len());
        let mut moved = Vec::with_capacity(fixed.len());
        for p in fixed {
            let v = self.fixed_inverse.apply(p);
            let q: [R; D] = std::array::from_fn(|k| R::from_f64(f64::from(v[k])));
            let m = moving.transform_point(&q);
            virt.push(v);
            moved.push(std::array::from_fn(|k| m[k].to_f64() as f32));
        }
        (virt, moved)
    }

    /// `IsInsideVirtualDomain(point)`.
    fn inside(&self, p: &[f32; D]) -> bool {
        match &self.virtual_space {
            None => true,
            Some(s) => {
                let q: [f64; D] = std::array::from_fn(|k| f64::from(p[k]));
                s.contains(&s.point_to_index(&q))
            }
        }
    }

    /// The moving points.
    fn moving_points(&self) -> &[[f32; D]] {
        match &self.inputs {
            PointSetInputs::Labeled { moving, .. } => &moving.points,
            PointSetInputs::Intensity { moving, .. } => &moving.points,
        }
    }

    /// `Initialize()` at a level: the virtual domain (the level's shrunk domain), the fixed
    /// transform, and the metric's own initialisation with the moving transform as it is.
    pub fn initialize(
        &mut self,
        virtual_space: Option<Space<D>>,
        fixed_initial: Option<&Composite<R, D>>,
        moving: &Composite<R, D>,
        warnings: &mut Vec<String>,
    ) -> Result<(), ItkException> {
        self.virtual_space = virtual_space;
        self.fixed_inverse = FixedInverse::of(fixed_initial).ok_or_else(|| {
            ItkException::new(
                BASE_FILE,
                512,
                self.class(),
                "the fixed transform cannot be inverted (ITK continues with a null transform and crashes)",
            )
        })?;
        match &self.inputs {
            PointSetInputs::Labeled { fixed, moving: m } => {
                let (Some(fl), Some(ml)) = (&fixed.labels, &m.labels) else {
                    return Err(ItkException::new(
                        LABELED_FILE,
                        51,
                        "LabeledPointSetToPointSetMetricv4",
                        "Each point of the point set must be associated with a label.",
                    ));
                };
                // DetermineCommonPointSetLabels.
                let mut fixed_labels: Vec<u32> = Vec::new();
                for &l in fl {
                    if !fixed_labels.contains(&l) {
                        fixed_labels.push(l);
                    }
                }
                fixed_labels.sort_unstable();
                let mut moving_labels: Vec<u32> = Vec::new();
                for &l in ml {
                    if !moving_labels.contains(&l) {
                        moving_labels.push(l);
                    }
                }
                moving_labels.sort_unstable();
                let mut common = Vec::new();
                let mut uncommon = false;
                for &l in &fixed_labels {
                    if moving_labels.contains(&l) {
                        common.push(l);
                    } else {
                        uncommon = true;
                    }
                }
                if uncommon {
                    warnings.push(format!(
                        "WARNING: In {LABELED_FILE}, line 235\nLabeledPointSetToPointSetMetricv4 (0x0): The label sets are not bijective.\n\n"
                    ));
                }
                if common.is_empty() && !fl.is_empty() && self.corrections.skip_uncommon_labels {
                    // ITK-22 corrected: with no label in common nothing can be matched (ITK
                    // throws at the first fixed point).
                    self.events.uncommon_labels = true;
                    return Err(ItkException::new(
                        LABELED_FILE,
                        116,
                        "LabeledPointSetToPointSetMetricv4",
                        "The fixed and moving point sets have no label in common",
                    ));
                }
                let mut clones = Vec::with_capacity(common.len());
                let mut events = self.events;
                for &label in &common {
                    let subset: Vec<[f32; D]> = m
                        .points
                        .iter()
                        .zip(ml)
                        .filter(|(_, l)| **l == label)
                        .map(|(p, _)| *p)
                        .collect();
                    clones.push(LabelMetric::new(
                        &self.kind,
                        &subset,
                        &self.corrections,
                        &mut events,
                        warnings,
                    )?);
                }
                self.events = events;
                self.labels = common;
                self.clones = clones;
            }
            PointSetInputs::Intensity {
                fixed: fixed_set,
                moving: moving_set,
            } => {
                // PointSetToPointSetMetricWithIndexv4::Initialize: the point sets transformed
                // and their locators built (fixed first).
                let (_, fixed_t) = self.transform_fixed(moving);
                if fixed_t.is_empty() {
                    return Err(no_points());
                }
                let tree = tree(&moving_set.points)?;
                let PointSetMetricKind::Igdm {
                    intensity_sigma,
                    euclidean_sigma,
                } = self.kind
                else {
                    unreachable!("an intensity point set is IGDM's")
                };
                let mut is = intensity_sigma;
                let mut es = euclidean_sigma;
                if is <= R::ZERO {
                    let ants = estimate_intensity_sigma(fixed_set, moving_set);
                    let range = intensity_range_sigma(fixed_set, moving_set);
                    if ants.to_f64().to_bits() != range.to_f64().to_bits() {
                        self.events.igdm_sigma = Some((ants.to_f64(), range.to_f64()));
                    }
                    is = if self.corrections.igdm_intensity_range {
                        range
                    } else {
                        ants
                    };
                }
                if es <= R::ZERO {
                    es = estimate_euclidean_sigma(&fixed_t, &tree)?;
                }
                self.igdm = Some(Igdm {
                    tree,
                    intensity_sigma: is,
                    euclidean_sigma: es,
                });
            }
        }
        Ok(())
    }

    /// The wrapper's `InitializeForIteration()`: the transformed points (virtual, then moving),
    /// the locators' checks, the number of valid points and its warning.
    fn initialize_for_iteration(
        &mut self,
        moving: &Composite<R, D>,
        warnings: &mut Vec<String>,
    ) -> Result<TransformedPoints<D>, ItkException> {
        let (virt, fixed_t) = self.transform_fixed(moving);
        // InitializePointsLocators: the fixed locator, then the moving one.
        if fixed_t.is_empty() || self.moving_points().is_empty() {
            return Err(no_points());
        }
        if fixed_t.iter().any(|p| p.iter().any(|c| c.is_nan())) {
            return Err(ItkException::new(
                crate::method::CRASH_FILE,
                0,
                "",
                "a fixed point maps to a coordinate that is not a number (the transform's parameters are not numbers): ITK's nearest-neighbour search finds no point and reads past the end of the moving point set",
            ));
        }
        let mut valid = 0;
        for (i, p) in virt.iter().enumerate() {
            if !self.inside(p) {
                continue;
            }
            if !self.label_common(i) {
                self.events.uncommon_labels = true;
                if self.corrections.skip_uncommon_labels {
                    continue;
                }
            }
            valid += 1;
        }
        if valid < fixed_t.len() && !self.have_warned {
            warnings.push(format!(
                "WARNING: In {BASE_FILE}, line 136\n{} (0x0): Only {valid} of {} points are within the virtual domain, and will be used in the evaluation.\n\n",
                self.class(),
                fixed_t.len()
            ));
            self.have_warned = true;
        }
        Ok((virt, fixed_t, valid))
    }

    /// Whether fixed point `index` has a label of both sets (always, for IGDM).
    fn label_common(&self, index: usize) -> bool {
        match &self.inputs {
            PointSetInputs::Labeled { fixed, .. } => {
                let label = fixed.labels.as_ref().map_or(0, |l| l[index]);
                self.labels.contains(&label)
            }
            PointSetInputs::Intensity { .. } => true,
        }
    }

    /// Whether fixed point `index` takes part (inside the virtual domain, and with ITK-22
    /// corrected, of a common label).
    fn takes_part(&self, index: usize, virt: &[[f32; D]]) -> bool {
        self.inside(&virt[index])
            && (!self.corrections.skip_uncommon_labels || self.label_common(index))
    }

    /// `VerifyNumberOfValidPoints`'s warning.
    fn no_valid_points(&self) -> String {
        crate::metric::no_valid_points_warning(VERIFY_FILE, 544, self.class())
    }

    /// The label metric of fixed point `index` (`std::find` in the common labels), or ITK's
    /// exception.
    fn label_metric(&self, index: usize, line: u32) -> Result<&LabelMetric<R, D>, ItkException> {
        let PointSetInputs::Labeled { fixed, .. } = &self.inputs else {
            unreachable!()
        };
        let label = fixed.labels.as_ref().expect("checked at Initialize")[index];
        match self.labels.iter().position(|&l| l == label) {
            Some(i) => Ok(&self.clones[i]),
            None => Err(ItkException::new(
                LABELED_FILE,
                line,
                "LabeledPointSetToPointSetMetricv4",
                "Label not found in common label set",
            )),
        }
    }

    /// `GetValue()`.
    pub fn value(
        &mut self,
        moving: &Composite<R, D>,
        warnings: &mut Vec<String>,
    ) -> Result<R, ItkException> {
        let (virt, fixed_t, valid) = self.initialize_for_iteration(moving, warnings)?;
        let mut thread = CompensatedSum::default();
        for index in 0..fixed_t.len() {
            if !self.takes_part(index, &virt) {
                continue;
            }
            let v = match &self.inputs {
                PointSetInputs::Labeled { .. } => self
                    .label_metric(index, 95)?
                    .value(&fixed_t[index], warnings),
                PointSetInputs::Intensity { fixed, moving: m } => {
                    let pixel = &fixed.data[index];
                    if pixel.len() / (1 + D) > 1 {
                        self.events.igdm_centre = true;
                    }
                    self.igdm_value(&fixed_t[index], pixel, m)
                }
            };
            thread.add(v.to_f64());
        }
        let total = add_thread(&thread);
        let mut value = R::from_f64(total.sum());
        if valid == 0 {
            warnings.push(self.no_valid_points());
            return Ok(R::MAX);
        }
        value = value / R::from_i64(valid as i64);
        Ok(value)
    }

    /// `GetValueAndDerivative()`: the value and the derivative with respect to the moving
    /// transform's active parameters.
    pub fn value_and_derivative(
        &mut self,
        moving: &Composite<R, D>,
        warnings: &mut Vec<String>,
    ) -> Result<PointSetValue<R>, ItkException> {
        let (virt, fixed_t, valid) = self.initialize_for_iteration(moving, warnings)?;
        if moving.active().has_local_support() {
            return self.local_value_and_derivative(moving, &virt, &fixed_t, valid, warnings);
        }
        let np = moving.number_of_parameters();
        let mut thread = CompensatedSum::default();
        let mut sums = crate::CompensatedSums::new(np);
        let sparse = moving.active().is_bspline();
        let mut jac = vec![R::ZERO; if sparse { 0 } else { D * np }];
        let mut local_transform = vec![R::ZERO; if sparse { 0 } else { np }];
        for index in 0..fixed_t.len() {
            if !self.takes_part(index, &virt) {
                continue;
            }
            let (v, local) = match &self.inputs {
                PointSetInputs::Labeled { .. } => self
                    .label_metric(index, 116)?
                    .value_and_derivative(&fixed_t[index], warnings),
                PointSetInputs::Intensity { fixed, moving: m } => {
                    self.igdm_value_and_derivative(&fixed_t[index], &fixed.data[index], m)?
                }
            };
            thread.add(v.to_f64());
            for x in local_transform.iter_mut() {
                *x = R::ZERO;
            }
            let vp: [R; D] = std::array::from_fn(|k| R::from_f64(f64::from(virt[index][k])));
            if let Some((cols, columns)) = moving.sparse_jacobian(&vp) {
                // A B-spline transform: zero but for the parameters around the point.
                let k = cols.len();
                let values: Vec<R> = (0..k)
                    .map(|par| {
                        let mut v = R::ZERO;
                        for d in 0..D {
                            v = v + columns[d * k + par] * local[d];
                        }
                        v
                    })
                    .collect();
                sums.add_sparse(&cols, &values);
                continue;
            }
            moving.jacobian(&vp, &mut jac);
            for par in 0..np {
                for d in 0..D {
                    local_transform[par] = local_transform[par] + jac[d * np + par] * local[d];
                }
            }
            sums.add_dense(&local_transform);
        }
        sums.flush();
        let total = add_thread(&thread);
        let mut value = R::from_f64(total.sum());
        if valid == 0 {
            warnings.push(self.no_valid_points());
            return Ok(PointSetValue {
                value: R::MAX,
                derivative: vec![R::ZERO; np],
            });
        }
        let n = R::from_i64(valid as i64);
        let derivative = sums
            .sums
            .iter()
            .map(|s| R::from_f64(add_thread(s).sum() / n.to_f64()))
            .collect();
        value = value / n;
        Ok(PointSetValue { value, derivative })
    }

    /// `GetValueAndDerivative()` with a moving transform of local support (a displacement
    /// field a stage optimises): each point's derivative turned by the composite's Jacobian
    /// (`D` local parameters) and added at the virtual voxel nearest its virtual position
    /// (`StorePointDerivative`), not averaged.
    fn local_value_and_derivative(
        &mut self,
        moving: &Composite<R, D>,
        virt: &[[f32; D]],
        fixed_t: &[[f32; D]],
        valid: usize,
        warnings: &mut Vec<String>,
    ) -> Result<PointSetValue<R>, ItkException> {
        let n_all = moving.number_of_parameters();
        let mut derivative = vec![R::ZERO; n_all];
        let mut thread = CompensatedSum::default();
        let mut jac = [R::ZERO; 16];
        let space = self
            .virtual_space
            .clone()
            .expect("a point-set metric has a virtual domain");
        for index in 0..fixed_t.len() {
            if !self.takes_part(index, virt) {
                continue;
            }
            let (v, local) = match &self.inputs {
                PointSetInputs::Labeled { .. } => self
                    .label_metric(index, 116)?
                    .value_and_derivative(&fixed_t[index], warnings),
                PointSetInputs::Intensity { fixed, moving: m } => {
                    self.igdm_value_and_derivative(&fixed_t[index], &fixed.data[index], m)?
                }
            };
            thread.add(v.to_f64());
            let vp: [R; D] = std::array::from_fn(|k| R::from_f64(f64::from(virt[index][k])));
            moving.jacobian(&vp, &mut jac[..D * D]);
            let transformed: [R; D] = std::array::from_fn(|par| {
                let mut s = R::ZERO;
                for d in 0..D {
                    s = s + jac[d * D + par] * local[d];
                }
                s
            });
            let q: [f64; D] = std::array::from_fn(|k| f64::from(virt[index][k]));
            let o = space.offset(&space.point_to_index(&q)) * D;
            for d in 0..D {
                derivative[o + d] = derivative[o + d] + transformed[d];
            }
        }
        let total = add_thread(&thread);
        if valid == 0 {
            warnings.push(self.no_valid_points());
            return Ok(PointSetValue {
                value: R::MAX,
                derivative: vec![R::ZERO; n_all],
            });
        }
        let value = R::from_f64(total.sum()) / R::from_i64(valid as i64);
        Ok(PointSetValue { value, derivative })
    }

    /// The IGDM closest point and its data (`GetPointData(pointId)`).
    fn igdm_closest<'a>(
        &self,
        point: &[f32; D],
        moving: &'a IntensityPointSet<R, D>,
    ) -> (usize, &'a [R]) {
        let igdm = self.igdm.as_ref().expect("initialised");
        let id = igdm.tree.closest(point);
        (id, &moving.data[id])
    }

    /// `GetLocalNeighborhoodValue(point, pixel)` (ANTs' IGDM). For a neighbourhood of more
    /// than one voxel ANTs computes the centre index from `−0.5·voxels` (a negative number
    /// converted to an unsigned index, undefined) and reads memory before both intensity
    /// arrays (ANTS-66): corrected, the centre as the derivative reads it; reproduced, the two
    /// values read as equal (the intensity term 1).
    fn igdm_value(&self, point: &[f32; D], pixel: &[R], moving: &IntensityPointSet<R, D>) -> R {
        let igdm = self.igdm.as_ref().expect("initialised");
        let (id, closest_pixel) = self.igdm_closest(point, moving);
        let closest = &moving.points[id];
        let distance = R::from_f64(euclidean_distance(point, closest));
        let ratio = distance / igdm.euclidean_sigma;
        let distance_probability = (R::from_f64(-0.5) * (ratio * ratio)).exp();
        let voxels = pixel.len() / (1 + D);
        let intensity_distance = if voxels <= 1 {
            // static_cast<SizeValueType>(-0.5): 0.
            pixel[0] - closest_pixel[0]
        } else if self.corrections.igdm_value_centre {
            let centre = ((0.5 * voxels as f64) as usize) * (D + 1);
            pixel[centre] - closest_pixel[centre]
        } else {
            R::ZERO
        };
        let r = intensity_distance / igdm.intensity_sigma;
        let intensity_probability = (R::from_f64(-0.5) * (r * r)).exp();
        -R::ONE * intensity_probability * distance_probability
    }

    /// `GetLocalNeighborhoodValueAndDerivative(point, pixel)` (ANTs' IGDM).
    fn igdm_value_and_derivative(
        &self,
        point: &[f32; D],
        pixel: &[R],
        moving: &IntensityPointSet<R, D>,
    ) -> Result<(R, [R; D]), ItkException> {
        let igdm = self.igdm.as_ref().expect("initialised");
        let (id, closest_pixel) = self.igdm_closest(point, moving);
        let closest = &moving.points[id];
        let distance = R::from_f64(euclidean_distance(point, closest));
        let ratio = distance / igdm.euclidean_sigma;
        let distance_probability = (R::from_f64(-0.5) * (ratio * ratio)).exp();
        let voxels = pixel.len() / (1 + D);
        let centre = ((0.5 * voxels as f64) as usize) * (D + 1);
        let intensity_distance = pixel[centre] - closest_pixel[centre];
        let r = intensity_distance / igdm.intensity_sigma;
        let intensity_probability = (R::from_f64(-0.5) * (r * r)).exp();
        let measure = -R::ONE * intensity_probability * distance_probability;
        // (closestPoint − point) · float(intProb) · float(distProb), in float.
        let ip = intensity_probability.to_f64() as f32;
        let dp = distance_probability.to_f64() as f32;
        let mut local: [R; D] =
            std::array::from_fn(|d| R::from_f64(f64::from((closest[d] - point[d]) * ip * dp)));
        for d in 0..D {
            local[d] = local[d]
                + intensity_probability
                    * distance_probability
                    * intensity_distance
                    * closest_pixel[centre + 1 + d];
        }
        Ok((measure, local))
    }
}

/// A point-set metric's transform that ITK cannot invert in tangent space: its
/// `GetInverseTransform()` is null and ITK crashes on it.
fn no_inverse(which: &str) -> ItkException {
    ItkException::new(
        crate::method::CRASH_FILE,
        0,
        "",
        format!(
            "the {which} transform of a point-set metric in a SyN stage cannot be inverted (a linear transform with a singular matrix, or a displacement field without an inverse): ITK maps the points with a null transform and crashes"
        ),
    )
}

/// `DetermineCommonPointSetLabels()`: the sorted labels of `fixed` that `moving` has too, and
/// ITK's warning when `fixed` has others.
fn common_labels(fixed: &[u32], moving: &[u32], warnings: &mut Vec<String>) -> Vec<u32> {
    let sorted_unique = |labels: &[u32]| {
        let mut out: Vec<u32> = Vec::new();
        for &l in labels {
            if !out.contains(&l) {
                out.push(l);
            }
        }
        out.sort_unstable();
        out
    };
    let fixed_labels = sorted_unique(fixed);
    let moving_labels = sorted_unique(moving);
    let mut common = Vec::new();
    let mut uncommon = false;
    for &l in &fixed_labels {
        if moving_labels.contains(&l) {
            common.push(l);
        } else {
            uncommon = true;
        }
    }
    if uncommon {
        warnings.push(format!(
            "WARNING: In {LABELED_FILE}, line 235\nLabeledPointSetToPointSetMetricv4 (0x0): The label sets are not bijective.\n\n"
        ));
    }
    common
}

impl<R: Real, const D: usize> FixedInverse<R, D> {
    /// `TransformCovariantVector(vector, point)` of the inverse at `p` (a `Point<float>`).
    fn covariant(&self, v: &[R; D], p: &[f32; D]) -> [R; D] {
        match &self.0 {
            None => *v,
            Some(c) => {
                let q: [R; D] = std::array::from_fn(|k| R::from_f64(f64::from(p[k])));
                c.transform_covariant(v, &q)
            }
        }
    }
}

/// What SyN's `Initialize()` of a point-set metric leaves for its evaluation.
#[derive(Clone, Debug)]
struct TangentState<R, const D: usize> {
    swapped: bool,
    fixed_inverse: FixedInverse<R, D>,
    /// IGDM: the moving points and their gradients in the virtual domain.
    moved: Option<IntensityPointSet<R, D>>,
}

impl<R: Real, const D: usize> PointSetMetric<R, D> {
    /// SyN's `Initialize()` of this metric for one call (`ComputeMetricGradientField`): in
    /// tangent space with the call's transforms. The labelled metrics build one metric per
    /// common label on that label's moving points mapped into the virtual domain; IGDM maps
    /// the moving points and their gradients and estimates its sigmas.
    pub fn tangent_initialize(
        &mut self,
        call: &TangentCall<'_, R, D>,
        warnings: &mut Vec<String>,
    ) -> Result<(), ItkException> {
        self.virtual_space = Some(call.virtual_space.clone());
        // InitializePointSets maps the moving points first, then the fixed ones.
        let moving_inverse =
            FixedInverse::of(Some(call.moving_transform)).ok_or_else(|| no_inverse("moving"))?;
        let fixed_inverse =
            FixedInverse::of(Some(call.fixed_transform)).ok_or_else(|| no_inverse("fixed"))?;
        let swapped = call.swapped;
        let moved = match self.inputs.clone() {
            PointSetInputs::Labeled { fixed, moving } => {
                let (wf, wm) = if swapped {
                    (moving, fixed)
                } else {
                    (fixed, moving)
                };
                self.labeled_initialize(&wf, &wm, &moving_inverse, warnings)?;
                None
            }
            PointSetInputs::Intensity { fixed, moving } => {
                let (wf, wm) = if swapped {
                    (moving, fixed)
                } else {
                    (fixed, moving)
                };
                Some(self.igdm_initialize(&wf, &wm, &fixed_inverse, &moving_inverse)?)
            }
        };
        self.sparse = call.sparse;
        self.tangent = Some(TangentState {
            swapped,
            fixed_inverse,
            moved,
        });
        Ok(())
    }

    /// SyN's `GetValueAndDerivative()` after [`PointSetMetric::tangent_initialize`]: the
    /// value and the derivative, `D` values per voxel of the virtual domain (each point's
    /// local derivative added at the voxel nearest its virtual position, in point order).
    pub fn tangent_evaluate(
        &mut self,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Vec<R>), ItkException> {
        let state = self.tangent.take().expect("initialised");
        let result = match self.inputs.clone() {
            PointSetInputs::Labeled { fixed, moving } => {
                let (wf, wm) = if state.swapped {
                    (moving, fixed)
                } else {
                    (fixed, moving)
                };
                self.labeled_evaluate(&wf, &wm, &state.fixed_inverse, warnings)
            }
            PointSetInputs::Intensity { fixed, moving } => {
                let wf = if state.swapped { moving } else { fixed };
                let moved = state.moved.as_ref().expect("IGDM's moving set");
                self.igdm_evaluate(&wf, moved, &state.fixed_inverse, warnings)
            }
        };
        self.tangent = Some(state);
        result
    }

    /// The fixed points' virtual positions checked, the "Only N of M points" warning (once
    /// per stage) and the number of valid points; `label_ok(i)` tells whether point `i` has a
    /// label of both sets.
    fn tangent_valid(
        &mut self,
        virt: &[[f32; D]],
        label_ok: impl Fn(usize) -> bool,
        warnings: &mut Vec<String>,
    ) -> Result<usize, ItkException> {
        if virt.iter().any(|p| p.iter().any(|c| c.is_nan())) {
            return Err(ItkException::new(
                crate::method::CRASH_FILE,
                0,
                "",
                "a point maps to a coordinate that is not a number (the transform's parameters are not numbers): ITK's nearest-neighbour search finds no point and reads past the end of the point set",
            ));
        }
        let mut valid = 0;
        for (i, p) in virt.iter().enumerate() {
            if !self.inside(p) {
                continue;
            }
            if !label_ok(i) {
                self.events.uncommon_labels = true;
                if self.corrections.skip_uncommon_labels {
                    continue;
                }
            }
            valid += 1;
        }
        if valid < virt.len() && !self.have_warned {
            warnings.push(format!(
                "WARNING: In {BASE_FILE}, line 136\n{} (0x0): Only {valid} of {} points are within the virtual domain, and will be used in the evaluation.\n\n",
                self.class(),
                virt.len()
            ));
            self.have_warned = true;
        }
        Ok(valid)
    }

    /// The size of a tangent-space derivative: `D` per virtual voxel (sparse) or per fixed
    /// point.
    fn tangent_len(&self, points: usize) -> usize {
        if self.sparse {
            D * self.virtual_space.as_ref().map_or(0, Space::len)
        } else {
            D * points
        }
    }

    /// The fixed points' virtual positions of the latest tangent-space call
    /// (`GetModifiableFixedTransformedPointSet`, in tangent space).
    pub fn tangent_fixed_points(&self) -> Vec<[f32; D]> {
        let Some(state) = &self.tangent else {
            return Vec::new();
        };
        let points: &[[f32; D]] = match &self.inputs {
            PointSetInputs::Labeled { fixed, moving } => {
                if state.swapped {
                    &moving.points
                } else {
                    &fixed.points
                }
            }
            PointSetInputs::Intensity { fixed, moving } => {
                if state.swapped {
                    &moving.points
                } else {
                    &fixed.points
                }
            }
        };
        points
            .iter()
            .map(|p| state.fixed_inverse.apply(p))
            .collect()
    }

    /// `StorePointDerivative`: `local` added at the virtual voxel nearest `p`; without the
    /// sparse field (`StoreDerivativeAsSparseFieldForLocalSupportTransforms` off), `local`
    /// stored at the point's index.
    fn store(&self, derivative: &mut [R], index: usize, p: &[f32; D], local: &[R; D]) {
        if !self.sparse {
            derivative[index * D..(index + 1) * D].copy_from_slice(local);
            return;
        }
        let space = self.virtual_space.as_ref().expect("set by the call");
        let q: [f64; D] = std::array::from_fn(|k| f64::from(p[k]));
        let o = space.offset(&space.point_to_index(&q)) * D;
        for d in 0..D {
            derivative[o + d] = derivative[o + d] + local[d];
        }
    }

    /// `LabeledPointSetToPointSetMetricv4::Initialize()` in tangent space.
    fn labeled_initialize(
        &mut self,
        wf: &LabeledPointSet<D>,
        wm: &LabeledPointSet<D>,
        moving_inverse: &FixedInverse<R, D>,
        warnings: &mut Vec<String>,
    ) -> Result<(), ItkException> {
        let (Some(fl), Some(ml)) = (&wf.labels, &wm.labels) else {
            return Err(ItkException::new(
                LABELED_FILE,
                51,
                "LabeledPointSetToPointSetMetricv4",
                "Each point of the point set must be associated with a label.",
            ));
        };
        let common = common_labels(fl, ml, warnings);
        if common.is_empty() && !fl.is_empty() && self.corrections.skip_uncommon_labels {
            self.events.uncommon_labels = true;
            return Err(ItkException::new(
                LABELED_FILE,
                116,
                "LabeledPointSetToPointSetMetricv4",
                "The fixed and moving point sets have no label in common",
            ));
        }
        let mut subsets = Vec::with_capacity(common.len());
        let mut clones = Vec::with_capacity(common.len());
        let mut events = self.events;
        for &label in &common {
            let subset: Vec<[f32; D]> = wm
                .points
                .iter()
                .zip(ml)
                .filter(|(_, l)| **l == label)
                .map(|(p, _)| moving_inverse.apply(p))
                .collect();
            clones.push(LabelMetric::new(
                &self.kind,
                &subset,
                &self.corrections,
                &mut events,
                warnings,
            )?);
            subsets.push(subset);
        }
        self.events = events;
        self.labels = common;
        // ITK-32: ITK keeps the first Initialize's metrics; the notice whenever their moving
        // points differ from the current ones.
        match &self.syn_clones {
            None => self.syn_clones = Some((subsets, clones)),
            Some((first, _)) => {
                if *first != subsets {
                    self.events.stale_clones = true;
                }
                if self.corrections.current_clones {
                    self.syn_current = Some(clones);
                }
            }
        }
        Ok(())
    }

    /// The labelled metrics' `GetValueAndDerivative()` in tangent space.
    fn labeled_evaluate(
        &mut self,
        wf: &LabeledPointSet<D>,
        wm: &LabeledPointSet<D>,
        fixed_inverse: &FixedInverse<R, D>,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Vec<R>), ItkException> {
        // InitializeForIteration maps both sets again and builds the locators (the fixed
        // points', then the moving points').
        let virt: Vec<[f32; D]> = wf.points.iter().map(|p| fixed_inverse.apply(p)).collect();
        if virt.is_empty() || wm.points.is_empty() {
            return Err(no_points());
        }
        let fl = wf.labels.as_ref().expect("checked at Initialize");
        let labels = self.labels.clone();
        let valid = self.tangent_valid(&virt, |i| labels.contains(&fl[i]), warnings)?;
        let total_len = self.tangent_len(virt.len());
        let mut derivative = vec![R::ZERO; total_len];
        let mut thread = CompensatedSum::default();
        let clones = match &self.syn_current {
            Some(c) if self.corrections.current_clones => c,
            _ => &self.syn_clones.as_ref().expect("set at Initialize").1,
        };
        for (index, p) in virt.iter().enumerate() {
            if !self.inside(p) {
                continue;
            }
            let Some(position) = labels.iter().position(|&l| l == fl[index]) else {
                if self.corrections.skip_uncommon_labels {
                    continue;
                }
                return Err(ItkException::new(
                    LABELED_FILE,
                    116,
                    "LabeledPointSetToPointSetMetricv4",
                    "Label not found in common label set",
                ));
            };
            let (v, local) = clones[position].value_and_derivative(p, warnings);
            thread.add(v.to_f64());
            self.store(&mut derivative, index, p, &local);
        }
        let total = add_thread(&thread);
        if valid == 0 {
            warnings.push(self.no_valid_points());
            return Ok((R::MAX, vec![R::ZERO; total_len]));
        }
        Ok((
            R::from_f64(total.sum()) / R::from_i64(valid as i64),
            derivative,
        ))
    }

    /// ANTs' IGDM `Initialize()` in tangent space: the moving points and their gradients
    /// mapped into the virtual domain (`TransformMovingPointSetGradients`, each gradient a
    /// covariant vector at its point), then the sigmas.
    fn igdm_initialize(
        &mut self,
        wf: &IntensityPointSet<R, D>,
        wm: &IntensityPointSet<R, D>,
        fixed_inverse: &FixedInverse<R, D>,
        moving_inverse: &FixedInverse<R, D>,
    ) -> Result<IntensityPointSet<R, D>, ItkException> {
        let moved_points: Vec<[f32; D]> =
            wm.points.iter().map(|p| moving_inverse.apply(p)).collect();
        let virt: Vec<[f32; D]> = wf.points.iter().map(|p| fixed_inverse.apply(p)).collect();
        if virt.is_empty() {
            return Err(no_points());
        }
        let tree = tree(&moved_points)?;
        let data: Vec<Vec<R>> = wm
            .points
            .iter()
            .zip(&wm.data)
            .map(|(p, pixel)| {
                let mut out = pixel.clone();
                let voxels = pixel.len() / (1 + D);
                for n in 0..voxels {
                    let base = n * (D + 1) + 1;
                    let v: [R; D] = std::array::from_fn(|d| pixel[base + d]);
                    let t = moving_inverse.covariant(&v, p);
                    out[base..base + D].copy_from_slice(&t);
                }
                out
            })
            .collect();
        let PointSetMetricKind::Igdm {
            intensity_sigma,
            euclidean_sigma,
        } = self.kind
        else {
            unreachable!("an intensity point set is IGDM's")
        };
        let mut is = intensity_sigma;
        let mut es = euclidean_sigma;
        if is <= R::ZERO {
            let ants = estimate_intensity_sigma(wf, wm);
            let range = intensity_range_sigma(wf, wm);
            if ants.to_f64().to_bits() != range.to_f64().to_bits() {
                self.events.igdm_sigma = Some((ants.to_f64(), range.to_f64()));
            }
            is = if self.corrections.igdm_intensity_range {
                range
            } else {
                ants
            };
        }
        if es <= R::ZERO {
            es = estimate_euclidean_sigma(&virt, &tree)?;
        }
        self.igdm = Some(Igdm {
            tree,
            intensity_sigma: is,
            euclidean_sigma: es,
        });
        Ok(IntensityPointSet {
            points: moved_points,
            data,
        })
    }

    /// ANTs' IGDM `GetValueAndDerivative()` in tangent space (the fixed points' data as
    /// given).
    fn igdm_evaluate(
        &mut self,
        wf: &IntensityPointSet<R, D>,
        moved: &IntensityPointSet<R, D>,
        fixed_inverse: &FixedInverse<R, D>,
        warnings: &mut Vec<String>,
    ) -> Result<(R, Vec<R>), ItkException> {
        let virt: Vec<[f32; D]> = wf.points.iter().map(|p| fixed_inverse.apply(p)).collect();
        if virt.is_empty() || moved.points.is_empty() {
            return Err(no_points());
        }
        let valid = self.tangent_valid(&virt, |_| true, warnings)?;
        let total_len = self.tangent_len(virt.len());
        let mut derivative = vec![R::ZERO; total_len];
        let mut thread = CompensatedSum::default();
        for (index, p) in virt.iter().enumerate() {
            if !self.inside(p) {
                continue;
            }
            if wf.data[index].len() / (1 + D) > 1 {
                self.events.igdm_centre = true;
            }
            let (v, local) = self.igdm_value_and_derivative(p, &wf.data[index], moved)?;
            thread.add(v.to_f64());
            self.store(&mut derivative, index, p, &local);
        }
        let total = add_thread(&thread);
        if valid == 0 {
            warnings.push(self.no_valid_points());
            return Ok((R::MAX, vec![R::ZERO; total_len]));
        }
        Ok((
            R::from_f64(total.sum()) / R::from_i64(valid as i64),
            derivative,
        ))
    }
}

/// `CompensatedSummation value = 0; value += threadValue`: the thread's compensation added,
/// then its sum.
fn add_thread(thread: &CompensatedSum) -> CompensatedSum {
    let mut total = CompensatedSum::default();
    total.add(thread.compensation());
    total.add(thread.sum());
    total
}

/// `EstimateIntensityDistanceSigma()` as ANTs writes it: the maxima of both sets' centre
/// intensities, and minima that are never updated (`else if (value > min)` from the largest
/// value), so the sigma is `max(maxMoving, maxFixed) − maxMoving`, or the larger maximum when
/// that is 0.
fn estimate_intensity_sigma<R: Real, const D: usize>(
    fixed: &IntensityPointSet<R, D>,
    moving: &IntensityPointSet<R, D>,
) -> R {
    let extremes = |set: &IntensityPointSet<R, D>| {
        let mut max = -R::MAX;
        let mut min = R::MAX;
        for pixel in &set.data {
            let voxels = pixel.len() / (1 + D);
            let centre = ((0.5 * voxels as f64) as usize) * (D + 1);
            let v = pixel[centre];
            if v > max {
                max = v;
            } else if v > min {
                min = v;
            }
        }
        (min, max)
    };
    let (_, max_fixed) = extremes(fixed);
    let (min_moving, max_moving) = extremes(moving);
    let hi = if max_moving < max_fixed {
        max_fixed
    } else {
        max_moving
    };
    let lo = if max_moving < min_moving {
        max_moving
    } else {
        min_moving
    };
    let mut sigma = hi - lo;
    if almost_equals(sigma, R::ZERO) {
        sigma = hi;
    }
    sigma
}

/// ANTS-65 corrected: the range of both point sets' centre intensities,
/// `max(maxFixed, maxMoving) − min(minFixed, minMoving)` (the larger maximum when that is 0,
/// as ANTs falls back).
fn intensity_range_sigma<R: Real, const D: usize>(
    fixed: &IntensityPointSet<R, D>,
    moving: &IntensityPointSet<R, D>,
) -> R {
    let mut max = -R::MAX;
    let mut min = R::MAX;
    for set in [fixed, moving] {
        for pixel in &set.data {
            let voxels = pixel.len() / (1 + D);
            let v = pixel[((0.5 * voxels as f64) as usize) * (D + 1)];
            if v > max {
                max = v;
            }
            if v < min {
                min = v;
            }
        }
    }
    let mut sigma = max - min;
    if almost_equals(sigma, R::ZERO) {
        sigma = max;
    }
    sigma
}

/// `EstimateEuclideanDistanceSigma()`: the standard deviation of the fixed points' distances
/// to their nearest moving points (Welford's running mean and variance).
fn estimate_euclidean_sigma<R: Real, const D: usize>(
    fixed_t: &[[f32; D]],
    tree: &KdTree<D>,
) -> Result<R, ItkException> {
    if fixed_t.len() <= 1 {
        return Err(ItkException::new(
            IGDM_FILE,
            185,
            "MeanSquaresPointSetToPointSetIntensityMetricv4",
            "Need more than 1 point to estimate the distance sigma.",
        ));
    }
    let mut mean = R::ZERO;
    let mut sigma = R::ZERO;
    for (count, p) in fixed_t.iter().enumerate() {
        let closest = &tree.points()[tree.closest(p)];
        // closestPoint.EuclideanDistanceTo(ItF.Value()).
        let distance = R::from_f64(euclidean_distance(closest, p));
        if count == 0 {
            mean = distance;
            sigma = R::ZERO;
        } else {
            let previous = mean;
            mean = previous + (distance - previous) / R::from_i64(count as i64 + 1);
            sigma = sigma + (distance - previous) * (distance - mean);
        }
    }
    Ok((sigma / R::from_i64(fixed_t.len() as i64)).sqrt())
}
