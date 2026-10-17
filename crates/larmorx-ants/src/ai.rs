// SPDX-License-Identifier: Apache-2.0
//! `antsAI` (ANTs v2.6.5, `Examples/antsAI.cxx`): the linear transform that best aligns two
//! images, from an initial alignment and a multi-start search over rotations and
//! translations, each start point refined by a few iterations of a conjugate-gradient
//! optimizer.
//!
//! The steps of `antsAI<D>` after the command line is read, each as ANTs computes it:
//!
//! 1. [`initial`]: the initial transform (an `AffineTransform<double>`): the images' geometric
//!    centres or centres of mass for `AlignGeometricCenters` / `AlignCentersOfMass`
//!    (`CenteredTransformInitializer`); otherwise the centres of mass
//!    (`ImageMomentsCalculator`, converted to `float` as `antsAI` stores them) and, with
//!    `--align-principal-axes 1`, the rotation of Wahba's problem between the principal axes
//!    (vnl's SVD); or the blob landmarks of `--align-blobs`.
//! 2. With 0 iterations, [`without_search`]: the initial transform as the output class.
//! 3. Otherwise [`search`]: the metric (`Mattes`, `MI`, `GC`; the moving image's gradient from
//!    `GradientRecursiveGaussianImageFilter`, ITK's default), sampled as `antsAI` samples the
//!    fixed image; the list of start points (rotations about the principal axes or the image
//!    axes, translations on a grid); `MultiStartOptimizerv4` with
//!    `ConjugateGradientLineSearchOptimizerv4` (line search over `[0, 2]`, ε 0.1, at most 10
//!    steps, the learning rate estimated from the physical-shift scales at each start point);
//!    the best start point as the output class.
//!
//! The images are `itk::Image<float, D>` (their values are kept as doubles, which represent
//! them exactly); the transforms and every computation are in double.

// ITK's loops index several arrays at once; they are kept as written.
#![allow(clippy::needless_range_loop, clippy::manual_memcpy)]

use std::sync::Arc;

use larmorx_core::IndexFloor;
use larmorx_core::defects::{self, Notices};
use larmorx_core::vnl_fixed;
use larmorx_registration::format::OStream;
use larmorx_registration::initializer::{Feature, initialize};
use larmorx_registration::mask::Mask;
use larmorx_registration::method::{LevelProblem, MetricKind, gradient_image};
use larmorx_registration::metric::ImageMetricv4;
use larmorx_registration::moments::{self, MomentsError};
use larmorx_registration::optimizer::ConjugateGradientLineSearch;
use larmorx_registration::optimizer::multistart::{self, MultiStartObserver};
use larmorx_registration::rng::{MersenneTwister, RandomIterator};
use larmorx_registration::transform::{Composite, Kind, LinearXfm, Xfm};
use larmorx_registration::{Image, ItkException, Matrix, Space};

use crate::registration::helper::Log;

/// The metric of `--metric` (`antsAI` lowercases the name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiMetric {
    /// `Mattes`: `MattesMutualInformationImageToImageMetricv4`.
    Mattes,
    /// `MI`: `JointHistogramMutualInformationImageToImageMetricv4` (joint-PDF smoothing
    /// variance 1).
    Mi,
    /// `GC`: `CorrelationImageToImageMetricv4`.
    Gc,
}

impl AiMetric {
    /// The metric of a lowercased name.
    pub fn from_name(name: &str) -> Option<AiMetric> {
        match name {
            "mattes" => Some(AiMetric::Mattes),
            "mi" => Some(AiMetric::Mi),
            "gc" => Some(AiMetric::Gc),
            _ => None,
        }
    }

    /// The ITK class, as its exceptions name it.
    pub fn class_name(self) -> &'static str {
        match self {
            AiMetric::Mattes => "MattesMutualInformationImageToImageMetricv4",
            AiMetric::Mi => "JointHistogramMutualInformationImageToImageMetricv4",
            AiMetric::Gc => "CorrelationImageToImageMetricv4",
        }
    }
}

/// The fourth parameter of `--metric`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiSampling {
    None,
    Regular,
    Random,
}

/// The transform of `--transform` (lowercased).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiTransform {
    Rigid,
    Affine,
    Similarity,
    AlignGeometricCenters,
    AlignCentersOfMass,
}

impl AiTransform {
    pub fn from_name(name: &str) -> Option<AiTransform> {
        match name {
            "rigid" => Some(AiTransform::Rigid),
            "affine" => Some(AiTransform::Affine),
            "similarity" => Some(AiTransform::Similarity),
            "aligngeometriccenters" => Some(AiTransform::AlignGeometricCenters),
            "aligncentersofmass" => Some(AiTransform::AlignCentersOfMass),
            _ => None,
        }
    }

    /// Whether the transform comes from the images only (no search parameters).
    pub fn from_images(self) -> bool {
        matches!(
            self,
            AiTransform::AlignGeometricCenters | AiTransform::AlignCentersOfMass
        )
    }

    /// The class `antsAI` writes in dimension `d`: `AffineTransform`, the rigid class
    /// (`Euler2DTransform`, `Euler3DTransform`; also for the two alignments) or the
    /// similarity class.
    pub fn output_kind(self, d: usize) -> Kind {
        match (self, d) {
            (AiTransform::Affine, _) => Kind::Affine,
            (AiTransform::Similarity, 2) => Kind::Similarity2D,
            (AiTransform::Similarity, _) => Kind::Similarity3D,
            (_, 2) => Kind::Euler2D,
            _ => Kind::Euler3D { zyx: false },
        }
    }
}

/// `antsAI`'s settings, read from its command line.
#[derive(Clone, Debug, PartialEq)]
pub struct AiSettings {
    /// `None` for a name `antsAI` does not know (it says so only when the search runs).
    pub metric: Option<AiMetric>,
    pub bins: u32,
    pub sampling: AiSampling,
    pub sampling_percentage: f64,
    pub iterations: u32,
    pub convergence_threshold: f64,
    pub convergence_window: u32,
    pub transform: AiTransform,
    /// The gradient step (`Rigid[step]`), also the optimizer's maximum step in mm.
    pub learning_rate: f64,
    /// The angle step in radians (`--search-factor`, given in degrees).
    pub search_factor: f64,
    /// `--search-factor`'s arc fraction: the angles run over `[−π·f, π·f]`.
    pub arc_fraction: f64,
    /// `--translation-search-grid`: the step in mm and the extent along each axis.
    pub translation_step: f64,
    pub translation_grid: Vec<f64>,
    pub align_principal_axes: bool,
    /// `--align-blobs`: blobs to extract and to match (0: none).
    pub blobs: (u32, u32),
    /// The sampling seed (`--random-seed`, `ANTS_RANDOM_SEED`, 1234 by default; 0: ANTs
    /// seeds from the clock, larmorx uses 121212).
    pub random_seed: i32,
    pub verbose: bool,
}

impl Default for AiSettings {
    /// `antsAI`'s defaults.
    fn default() -> Self {
        AiSettings {
            metric: Some(AiMetric::Mattes),
            bins: 32,
            sampling: AiSampling::None,
            sampling_percentage: 1.0,
            iterations: 0,
            convergence_threshold: 1e-6,
            convergence_window: 5,
            transform: AiTransform::Rigid,
            learning_rate: 0.1,
            search_factor: 20.0 * std::f64::consts::PI / 180.0,
            arc_fraction: 1.0,
            translation_step: 25.0,
            translation_grid: Vec::new(),
            align_principal_axes: false,
            blobs: (0, 0),
            random_seed: 1234,
            verbose: false,
        }
    }
}

/// Why `antsAI` stops.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AiError {
    /// An ITK exception `antsAI` does not catch (the program aborts); larmorx stops with it.
    #[error("{0}")]
    Uncaught(ItkException),
    /// What `antsAI` would run with undefined behaviour, or never finish: larmorx refuses.
    #[error("{0}")]
    Refused(String),
    /// `antsAI` returns this status after its message (already printed).
    #[error("exit {0}")]
    Exit(u8),
    /// A C++ exception other than ITK's reaches `std::terminate` (ANTs aborts): what
    /// libstdc++ prints.
    #[error("{0}")]
    Terminated(String),
}

/// The initial transform and the rotation axes of the search.
#[derive(Clone, Debug, PartialEq)]
pub struct Initial<const D: usize> {
    /// `initialTransform`, an `AffineTransform<double>`.
    pub transform: LinearXfm<f64, D>,
    /// The first and second rotation axes of the 3D search (the principal axes, or x and y).
    pub axis1: [f64; D],
    pub axis2: [f64; D],
}

fn uncaught_moments(e: MomentsError) -> AiError {
    match e {
        MomentsError::ZeroMass(e) => AiError::Uncaught(e),
        MomentsError::NotFinite => AiError::Refused(e.to_string()),
    }
}

/// `vnl_matrix` product (each entry summed from 0).
fn mul_dyn<const D: usize>(a: &Matrix<f64, D>, b: &Matrix<f64, D>) -> Matrix<f64, D> {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            let mut s = 0.0;
            for k in 0..D {
                s += a[i][k] * b[k][j];
            }
            s
        })
    })
}

/// `vnl_determinant` of a `vnl_matrix`.
fn det<const D: usize>(m: &Matrix<f64, D>) -> f64 {
    let flat: Vec<f64> = m.iter().flatten().copied().collect();
    vnl_fixed::determinant(&flat, D)
}

/// Wahba's rotation between the principal axes (`antsAI.cxx:999-1031`): `A = V·Uᵀ` of vnl's
/// SVD of `B`, inverted (`vnl_inverse`); a negative determinant is turned positive by negating
/// the columns whose diagonal entry is negative. Prints what `antsAI` prints with `-v`.
fn wahba<const D: usize>(b: &Matrix<f64, D>, log: &mut Log<'_>) -> Result<Matrix<f64, D>, AiError> {
    let flat: Vec<f64> = b.iter().flatten().copied().collect();
    let (u, _w, v) = larmorx_core::vnl_svd::vnl_svd(&flat, D, D).ok_or_else(|| {
        AiError::Refused("vnl's SVD of the principal axes did not converge".into())
    })?;
    // Column-major: U(i, j) = u[i + D·j].
    let um: Matrix<f64, D> = std::array::from_fn(|i| std::array::from_fn(|j| u[i + D * j]));
    let vm: Matrix<f64, D> = std::array::from_fn(|i| std::array::from_fn(|j| v[i + D * j]));
    let ut: Matrix<f64, D> = std::array::from_fn(|i| std::array::from_fn(|j| um[j][i]));
    let a = mul_dyn(&vm, &ut);
    let flat_a: Vec<f64> = a.iter().flatten().copied().collect();
    let inv = vnl_fixed::inverse(&flat_a, D);
    let mut a: Matrix<f64, D> = std::array::from_fn(|i| std::array::from_fn(|j| inv[i * D + j]));
    let d = det(&a);
    if d < 0.0 {
        if log.verbose {
            let os = OStream::default();
            log.cout(&format!("Bad determinant = {}\n", os.float(d)));
            log.cout(&format!("  det( V ) = {}\n", os.float(det(&vm))));
            log.cout(&format!("  det( U ) = {}\n", os.float(det(&um))));
        }
        let mut id: Matrix<f64, D> =
            std::array::from_fn(|i| std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 }));
        for i in 0..D {
            if a[i][i] < 0.0 {
                id[i][i] = -1.0;
            }
        }
        let idt: Matrix<f64, D> = std::array::from_fn(|i| std::array::from_fn(|j| id[j][i]));
        a = mul_dyn(&a, &idt);
        let d = det(&a);
        if log.verbose {
            log.cout(&format!(
                "New determinant = {}\n",
                OStream::default().float(d)
            ));
        }
    }
    Ok(a)
}

/// The initial transform (`antsAI.cxx:806-1051`). `fixed` and `moving` hold the float
/// images' values.
pub fn initial<const D: usize>(
    s: &AiSettings,
    fixed: &Image<f64, D>,
    moving: &Image<f64, D>,
    log: &mut Log<'_>,
) -> Result<Initial<D>, AiError> {
    let mut transform = LinearXfm::<f64, D>::new(Kind::Affine);
    let mut axis1 = [0.0f64; D];
    let mut axis2 = [0.0f64; D];
    axis1[0] = 1.0;
    axis2[1] = 1.0;
    if s.transform.from_images() {
        // CenteredTransformInitializer<AffineTransform<double>, Image<float>, Image<float>>.
        let feature = if s.transform == AiTransform::AlignGeometricCenters {
            Feature::Geometry
        } else {
            Feature::Moments
        };
        let (center, translation) =
            initialize::<f64, D>(fixed, moving, feature).map_err(AiError::Uncaught)?;
        transform.set_identity();
        transform.set_center(center);
        transform.set_translation(translation);
        return Ok(Initial {
            transform,
            axis1,
            axis2,
        });
    }
    if s.blobs.0 >= 3 {
        return Err(AiError::Refused(
            "--align-blobs is not ported yet (the blob detector and landmark initializer)".into(),
        ));
    }
    let mf = moments::compute(fixed).map_err(uncaught_moments)?;
    let mm = moments::compute(moving).map_err(uncaught_moments)?;
    // antsAI's VectorType is itk::Vector<float, D>.
    let fcg: [f32; D] = std::array::from_fn(|i| mf.center_of_gravity[i] as f32);
    let mcg: [f32; D] = std::array::from_fn(|i| mm.center_of_gravity[i] as f32);
    let translation: [f64; D] = std::array::from_fn(|i| f64::from(mcg[i] - fcg[i]));
    let center: [f64; D] = std::array::from_fn(|i| f64::from(fcg[i]));
    transform.set_translation(translation);
    let fpa = &mf.principal_axes;
    let mpa = &mm.principal_axes;
    let (fp, fs, mp, ms) = if D == 2 {
        (fpa[1], [0.0; D], mpa[1], [0.0; D])
    } else {
        (fpa[2], fpa[1], mpa[2], mpa[1])
    };
    let b: Matrix<f64, D> = std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            if D == 2 {
                mp[i] * fp[j]
            } else {
                mp[i] * fp[j] + ms[i] * fs[j]
            }
        })
    });
    if s.align_principal_axes {
        let a = wahba(&b, log)?;
        transform.set_matrix(a);
    }
    transform.set_center(center);
    if D == 3 {
        // tertiary = vnl_cross_3d(primary, secondary).
        let t = [
            fp[1] * fs[2] - fp[2] * fs[1],
            fp[2] * fs[0] - fp[0] * fs[2],
            fp[0] * fs[1] - fp[1] * fs[0],
        ];
        for d in 0..D {
            axis1[d] = t[d];
            axis2[d] = fs[d];
        }
    }
    Ok(Initial {
        transform,
        axis1,
        axis2,
    })
}

/// With 0 iterations (`antsAI.cxx:1058-1117`): the output class given the initial
/// transform's centre, matrix and offset, in that order.
pub fn without_search<const D: usize>(
    s: &AiSettings,
    init: &Initial<D>,
    warnings: &mut Vec<String>,
) -> Result<LinearXfm<f64, D>, AiError> {
    let mut out = LinearXfm::<f64, D>::new(s.transform.output_kind(D));
    out.set_center(init.transform.center);
    out.set_matrix_checked(init.transform.matrix, warnings)
        .map_err(AiError::Uncaught)?;
    out.set_offset(init.transform.offset);
    Ok(out)
}

/// `static_cast<unsigned long>(x)` of a double: in range it truncates; otherwise it is
/// undefined (ANTS-06): clamped by default, x86-64's answer (`cvttsd2si`, then the unsigned
/// fix-up GCC emits) with `reproduce`.
fn ulong_of(x: f64, notices: &mut Notices, what: &str) -> u64 {
    if (0.0..18_446_744_073_709_551_616.0).contains(&x) {
        return x as u64;
    }
    let clamped = if x.is_nan() || x < 0.0 { 0 } else { u64::MAX };
    let x86 = if x.is_finite() && x < 0.0 && x > -9_223_372_036_854_775_808.0 {
        (x as i64) as u64
    } else {
        0x8000_0000_0000_0000
    };
    let value = if notices.fix() { clamped } else { x86 };
    notices.add(
        &defects::ANTS_06,
        format!(
            "{what}: unsigned long of {} is undefined; {}",
            OStream::default().float(x),
            if notices.fix() {
                format!("clamped to {clamped}")
            } else {
                format!("{x86}, as x86-64 converts it")
            }
        ),
    );
    value
}

/// The fixed image's sample points (`antsAI.cxx:1234-1334`): every `ceil(1/p)`-th voxel
/// (Regular) or `float(N)·float(p)` random voxels (Random), each point a `Point<float>`
/// (`TransformIndexToPhysicalPoint` summed in float) moved by a normal variate times a third
/// of the spacing (Regular: `float(normal)·float(third)`; Random: `float(normal·third)`),
/// kept inside the fixed mask; then, as the metric maps them to its virtual domain, those
/// whose nearest index lies outside it are dropped.
///
/// The normal variates come from a generator seeded with `seed`. ANTs draws the random
/// voxels from the iterator's own generator, seeded from the clock (it is never reseeded):
/// larmorx seeds it with `seed + 1` (`seed` 0: 121212 and 121213).
///
/// Returns the points kept and how many were generated before the mapping (ITK's two
/// exceptions for an empty set tell them apart).
pub fn sample_points<const D: usize>(
    space: &Space<D>,
    sampling: AiSampling,
    percentage: f64,
    fixed_mask: Option<&Mask<D>>,
    seed: i32,
    notices: &mut Notices,
) -> Result<(Vec<[f64; D]>, usize), AiError> {
    let third: [f64; D] = std::array::from_fn(|k| space.spacing[k] / 3.0);
    let base = if seed == 0 { 121_212 } else { seed };
    let mut randomizer = MersenneTwister::from_int(base);
    let point_f32 = |idx: &[i64; D]| -> [f32; D] {
        std::array::from_fn(|i| {
            let mut v = space.origin[i] as f32;
            for j in 0..D {
                v = (f64::from(v) + space.index_to_point[i][j] * idx[j] as f64) as f32;
            }
            v
        })
    };
    let mut points = Vec::new();
    let keep = |p: [f32; D], points: &mut Vec<[f64; D]>| {
        let q: [f64; D] = std::array::from_fn(|k| f64::from(p[k]));
        if fixed_mask.is_none_or(|m| m.is_inside(&q)) {
            points.push(q);
        }
    };
    match sampling {
        AiSampling::None => {}
        AiSampling::Regular => {
            let step = ulong_of(
                (1.0 / percentage).ceil(),
                notices,
                "the Regular sampling step",
            );
            let mut count = step;
            for o in 0..space.len() {
                if count == step {
                    count = 0;
                    let mut p = point_f32(&space.index_of(o));
                    for d in 0..D {
                        let prod = (randomizer.normal() as f32) * (third[d] as f32);
                        p[d] += prod;
                    }
                    keep(p, &mut points);
                }
                count = count.wrapping_add(1);
            }
        }
        AiSampling::Random => {
            let total = space.len() as u64;
            let product = (total as f32) * (percentage as f32);
            if !(0.0..18_446_744_073_709_551_616.0).contains(&f64::from(product)) {
                return Err(AiError::Refused(format!(
                    "a sampling percentage of {} asks for {} random samples: ANTs' conversion to unsigned long is undefined and its loop never ends",
                    OStream::default().float(percentage),
                    OStream::default().float(f64::from(product))
                )));
            }
            let n = product as u64;
            let iterator_seed = if seed == 0 {
                121_213
            } else {
                seed.wrapping_add(1)
            };
            let mut it = RandomIterator::new(space.size, space.start, iterator_seed);
            for _ in 0..n {
                let idx = it.jump();
                let mut p = point_f32(&idx);
                for d in 0..D {
                    p[d] += (randomizer.normal() * third[d]) as f32;
                }
                keep(p, &mut points);
            }
        }
    }
    // MapFixedSampledPointSetToVirtual: points whose nearest index is outside the virtual
    // domain (the fixed image's) are skipped.
    let generated = points.len();
    points.retain(|p| space.contains(&space.point_to_index(p)));
    Ok((points, generated))
}

/// The rigid start point's class in dimension `D`.
fn rigid_kind<const D: usize>() -> Kind {
    if D == 2 {
        Kind::Euler2D
    } else {
        Kind::Euler3D { zyx: false }
    }
}

/// The values `for (v = from; v <= to; v += step)` visits, as doubles accumulate them;
/// `None` if the loop never ends (a step that does not move `v`) or visits more than
/// `limit` values.
fn loop_values(from: f64, to: f64, step: f64, limit: usize) -> Option<Vec<f64>> {
    let mut out = Vec::new();
    let mut v = from;
    while v <= to {
        if out.len() >= limit {
            return None;
        }
        out.push(v);
        let next = v + step;
        if next <= v {
            return None;
        }
        v = next;
    }
    Some(out)
}

/// The most start points larmorx lists before it refuses (ANTs would run for years).
pub const MAX_START_POINTS: usize = 10_000_000;

/// The multi-start optimizer's start points (`antsAI.cxx:1390-1509`), with what ANTs reads
/// of the translation grid: missing entries are read past the end of ANTs' vector
/// (undefined; refused by default, read as 0 with `reproduce`). Returns the list and ANTs'
/// `trialCounter` (the number of start points visited; for the two alignments the list
/// stays empty).
pub fn start_points<const D: usize>(
    s: &AiSettings,
    init: &Initial<D>,
    notices: &mut Notices,
    warnings: &mut Vec<String>,
) -> Result<(Vec<Vec<f64>>, usize), AiError> {
    let pi = std::f64::consts::PI;
    let grid: Vec<f64> = (0..D)
        .map(|d| s.translation_grid.get(d).copied())
        .collect::<Option<Vec<f64>>>()
        .map_or_else(
            || {
                if notices.fix() {
                    Err(AiError::Refused(format!(
                        "--translation-search-grid has {} extents for {D} dimensions: ANTs reads the missing ones past the end of its vector (ANTS-55)",
                        s.translation_grid.len()
                    )))
                } else {
                    notices.add(
                        &defects::ANTS_55,
                        format!(
                            "--translation-search-grid has {} extents for {D} dimensions; ANTs reads the missing ones past the end of its vector, read here as 0",
                            s.translation_grid.len()
                        ),
                    );
                    Ok((0..D)
                        .map(|d| s.translation_grid.get(d).copied().unwrap_or(0.0))
                        .collect())
                }
            },
            Ok,
        )?;
    let never = |what: &str| {
        AiError::Refused(format!(
            "{what}: ANTs' loop over the start points never ends (or lists more than {MAX_START_POINTS})"
        ))
    };
    let angles = loop_values(
        pi * -s.arc_fraction,
        pi * s.arc_fraction + 0.000001,
        s.search_factor,
        MAX_START_POINTS,
    )
    .ok_or_else(|| never("the search factor"))?;
    let translations: Vec<Vec<f64>> = (0..D)
        .map(|d| {
            loop_values(
                -grid[d],
                grid[d] + 0.000001,
                s.translation_step,
                MAX_START_POINTS,
            )
            .ok_or_else(|| never("the translation step"))
        })
        .collect::<Result<_, _>>()?;
    let mut total: usize = 1;
    for count in std::iter::once(angles.len().pow(if D == 2 { 1 } else { 3 }))
        .chain(translations.iter().map(Vec::len))
    {
        total = total.saturating_mul(count);
    }
    if total > MAX_START_POINTS {
        return Err(never("the search factor and the translation grid"));
    }
    if s.transform.from_images() {
        // The loops only move the affine search transform: no start point is listed.
        return Ok((Vec::new(), total));
    }
    let c = init.transform.center;
    let m0 = init.transform.matrix;
    let o0 = init.transform.offset;
    let fix_grid = notices.fix();
    let mut list = Vec::with_capacity(total);
    let mut grid_ignored = false;
    let mut affine = LinearXfm::<f64, D>::new(Kind::Affine);
    let mut rigid = LinearXfm::<f64, D>::new(rigid_kind::<D>());
    let mut similarity = LinearXfm::<f64, D>::new(s.transform.output_kind(D));
    if s.transform != AiTransform::Similarity {
        similarity = LinearXfm::<f64, D>::new(if D == 2 {
            Kind::Similarity2D
        } else {
            Kind::Similarity3D
        });
    }
    let push = |list: &mut Vec<Vec<f64>>, p: Vec<f64>| list.push(p);
    for &angle1 in &angles {
        if D == 2 {
            for &t1 in &translations[0] {
                for &t2 in &translations[1] {
                    let mut st = [0.0f64; D];
                    st[0] = t1;
                    st[1] = t2;
                    affine.set_identity();
                    affine.set_center(c);
                    affine.set_matrix(m0);
                    affine.set_offset(o0);
                    affine.translate(&st, true);
                    affine.rotate_2d(angle1, true);
                    match s.transform {
                        AiTransform::Affine => {
                            affine.scale_uniform(1.0, false);
                            push(&mut list, affine.parameters());
                        }
                        AiTransform::Rigid | AiTransform::Similarity => {
                            let t = if s.transform == AiTransform::Rigid {
                                &mut rigid
                            } else {
                                &mut similarity
                            };
                            // ANTS-52: ANTs takes the initial offset, which drops the search
                            // translation; corrected, the affine start point's offset.
                            let offset = if fix_grid { affine.offset } else { o0 };
                            if st.iter().any(|&v| v != 0.0) {
                                grid_ignored = true;
                            }
                            t.set_identity();
                            t.set_center(c);
                            t.set_matrix_checked(affine.matrix, warnings)
                                .map_err(AiError::Uncaught)?;
                            t.set_offset(offset);
                            if s.transform == AiTransform::Similarity {
                                t.set_scale(1.0);
                                t.set_scale(1.0);
                            }
                            push(&mut list, t.parameters());
                        }
                        _ => {}
                    }
                }
            }
        } else {
            for &angle2 in &angles {
                for &angle3 in &angles {
                    for &t1 in &translations[0] {
                        for &t2 in &translations[1] {
                            for &t3 in &translations[2] {
                                let mut st = [0.0f64; D];
                                st[0] = t1;
                                st[1] = t2;
                                st[2] = t3;
                                affine.set_identity();
                                affine.set_center(c);
                                affine.set_offset(o0);
                                affine.set_matrix(m0);
                                affine.translate(&st, false);
                                affine.rotate_3d(&init.axis1, angle1, true);
                                affine.rotate_3d(&init.axis2, angle2, true);
                                affine.rotate_3d(&init.axis1, angle3, true);
                                match s.transform {
                                    AiTransform::Affine => {
                                        affine.scale_uniform(1.0, false);
                                        push(&mut list, affine.parameters());
                                    }
                                    AiTransform::Rigid => {
                                        rigid.set_identity();
                                        rigid.set_center(c);
                                        rigid.set_offset(o0);
                                        rigid.translate_offset(&st);
                                        rigid
                                            .set_matrix_checked(affine.matrix, warnings)
                                            .map_err(AiError::Uncaught)?;
                                        push(&mut list, rigid.parameters());
                                    }
                                    AiTransform::Similarity => {
                                        similarity.set_identity();
                                        similarity.set_center(c);
                                        similarity.set_offset(o0);
                                        // ANTS-52: ANTs does not translate the similarity
                                        // start point; corrected, as its rigid one.
                                        if fix_grid {
                                            similarity.translate_offset(&st);
                                        }
                                        if st.iter().any(|&v| v != 0.0) {
                                            grid_ignored = true;
                                        }
                                        similarity
                                            .set_matrix_checked(affine.matrix, warnings)
                                            .map_err(AiError::Uncaught)?;
                                        similarity.set_scale(1.0);
                                        push(&mut list, similarity.parameters());
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if grid_ignored && (D == 2 || s.transform == AiTransform::Similarity) {
        notices.add(
            &defects::ANTS_52,
            if fix_grid {
                "the translation search grid moves each start point, as for Affine; ANTs ignores it for this transform"
            } else {
                "the translation search grid is ignored for this transform, as in ANTs"
            },
        );
    }
    Ok((list, total))
}

/// ANTs' `antsCommandIterationUpdate<MultiStartOptimizerv4>` (verbose): the header, every
/// `N/10`-th start point's value, the best value.
struct Progress<'l, 'a> {
    log: &'l mut Log<'a>,
    interval: usize,
    total: usize,
}

impl MultiStartObserver<f64> for Progress<'_, '_> {
    fn start(&mut self, number_of_iterations: usize) {
        self.total = number_of_iterations;
        self.interval = if number_of_iterations > 10 {
            number_of_iterations / 10
        } else {
            1
        };
        self.log.cout("\nIteration/Total,MetricValue\n");
    }

    fn iteration(&mut self, current_iteration: usize, value: f64) {
        let print_iter = current_iteration + 1;
        if print_iter.is_multiple_of(self.interval) {
            self.log.cout(&format!(
                "{print_iter}/{},{}\n",
                self.total,
                OStream::default().float(value)
            ));
        }
    }

    fn end(&mut self, _best_index: usize, _metric_values: &[f64]) {}
}

/// What [`search`] ends with.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum SearchOutcome<const D: usize> {
    /// The best start point in the output class, centred at the initial centre.
    Best(LinearXfm<f64, D>),
    /// No start point was listed: ANTs' multi-start optimizer does nothing. For the two
    /// alignments (`AlignGeometricCenters`, `AlignCentersOfMass` with iterations) its loops
    /// list none; otherwise the angle or translation loops are empty (a negative arc
    /// fraction or grid extent).
    NoStartPoint,
}

const METRIC_FILE: &str = "Modules/Registration/Metricsv4/include/itkImageToImageMetricv4.hxx";

/// The warning `RegistrationParameterScalesFromPhysicalShift::EstimateScales` gives when no
/// parameter moves a voxel (the two alignments: the metric keeps its default
/// `IdentityTransform`, which has no parameters).
const NO_SHIFT_WARNING: &str = "WARNING: In Modules/Numerics/Optimizersv4/include/itkRegistrationParameterScalesFromShiftBase.hxx, line 91\nRegistrationParameterScalesFromPhysicalShift (0x0): Variation in any parameter won't change a voxel position. The default scales (1.0) are used to avoid division-by-zero.\n\n";

/// The search (`antsAI.cxx:1125-1577`): the metric, the start points and the multi-start
/// optimization. `moving_f32` is the moving image as read (for its gradient image).
/// `seed_failure`: why the sampling seed could not be read (returned where ANTs reads it,
/// only when the metric is sampled).
///
/// As in ANTs, the two alignments run this too when they are given iterations (ANTS-53): the
/// metric is set up, the loops count their start points without listing any, and the
/// optimizer does nothing ([`SearchOutcome::NoStartPoint`]).
#[allow(clippy::too_many_arguments)]
pub fn search<const D: usize>(
    s: &AiSettings,
    init: &Initial<D>,
    fixed: &Arc<Image<f64, D>>,
    moving: &Arc<Image<f64, D>>,
    moving_f32: &Image<f32, D>,
    fixed_mask: Option<Mask<D>>,
    moving_mask: Option<Mask<D>>,
    log: &mut Log<'_>,
    notices: &mut Notices,
    warnings: &mut Vec<String>,
    n_threads: usize,
    seed_failure: Option<AiError>,
) -> Result<SearchOutcome<D>, AiError> {
    let (kind, metric_kind) = match s.metric {
        Some(AiMetric::Mattes) => {
            if log.verbose {
                log.cout(&format!(
                    "Using the Mattes MI metric (number of bins = {})\n",
                    s.bins
                ));
            }
            (
                AiMetric::Mattes,
                MetricKind::Mattes {
                    bins: s.bins as usize,
                },
            )
        }
        Some(AiMetric::Mi) => {
            if log.verbose {
                log.cout(&format!(
                    "Using the joint histogram MI metric (number of bins = {})\n",
                    s.bins
                ));
            }
            let correct_marginals = notices.fix();
            (
                AiMetric::Mi,
                MetricKind::Mi {
                    bins: s.bins as usize,
                    correct_marginals,
                },
            )
        }
        Some(AiMetric::Gc) => {
            if log.verbose {
                log.cout("Using the global correlation metric \n");
            }
            (AiMetric::Gc, MetricKind::Gc)
        }
        None => {
            // ANTS-45: larmorx explains it without -v too.
            log.cerr("ERROR: Unrecognized metric. \n");
            return Err(AiError::Exit(1));
        }
    };
    let samples = if s.sampling == AiSampling::None {
        None
    } else {
        // The seed is read here (--random-seed, else ANTS_RANDOM_SEED).
        if let Some(e) = seed_failure {
            return Err(e);
        }
        let (points, generated) = sample_points(
            &fixed.space,
            s.sampling,
            s.sampling_percentage,
            fixed_mask.as_ref(),
            s.random_seed,
            notices,
        )?;
        // The metric's Initialize (MapFixedSampledPointSetToVirtual).
        if generated == 0 {
            return Err(AiError::Uncaught(ItkException::new(
                METRIC_FILE,
                595,
                kind.class_name(),
                "Fixed Sample point set is empty.",
            )));
        }
        if points.is_empty() {
            return Err(AiError::Uncaught(ItkException::new(
                METRIC_FILE,
                628,
                kind.class_name(),
                "The virtual sampled point set has zero points because no fixed sampled points were within the virtual domain after mapping. There are no points to evaulate.",
            )));
        }
        Some(Arc::new(points))
    };
    if s.transform.from_images() {
        // The metric keeps its IdentityTransform: the scales estimator finds no parameter,
        // the loops list nothing, and the multi-start optimizer does nothing.
        warnings.push(NO_SHIFT_WARNING.to_owned());
        let (_, trials) = start_points(s, init, notices, warnings)?;
        if log.verbose {
            log.cout(&format!(
                "Starting optimizer with {trials} starting points\n"
            ));
        }
        return Ok(SearchOutcome::NoStartPoint);
    }
    let moving_gradient =
        Arc::new(gradient_image::<f32, D>(moving_f32, n_threads).map_err(AiError::Uncaught)?);
    let metric = ImageMetricv4 {
        fixed: Arc::clone(fixed),
        moving: Arc::clone(moving),
        fixed_mask,
        moving_mask,
        fixed_transform: None,
        virtual_space: fixed.space.clone(),
        samples,
        fixed_gradient_image: None,
        moving_gradient_image: Some(moving_gradient),
        pixel_f32: true,
    };
    let (mut list, trials) = start_points(s, init, notices, warnings)?;
    if list.is_empty() {
        if log.verbose {
            log.cout("Starting optimizer with 0 starting points\n");
        }
        return Ok(SearchOutcome::NoStartPoint);
    }
    if let MetricKind::Mi {
        correct_marginals, ..
    } = metric_kind
    {
        notices.add(
            &defects::ITK_18,
            if correct_marginals {
                "each image's marginal density is its own; ANTs swaps them"
            } else {
                "the marginal densities are swapped, as in ANTs"
            },
        );
    }
    let output_kind = s.transform.output_kind(D);
    let mut search_transform = LinearXfm::<f64, D>::new(output_kind.clone());
    search_transform.set_center(init.transform.center);
    search_transform
        .set_parameters(&list[0])
        .map_err(AiError::Uncaught)?;
    let floor = IndexFloor::new(notices.mode());
    let mut problem = LevelProblem::single(
        metric,
        metric_kind,
        Composite::new(vec![Xfm::Linear(search_transform)]),
        &floor,
    )
    .map_err(AiError::Uncaught)?;
    let mut local = ConjugateGradientLineSearch {
        learning_rate: s.learning_rate,
        maximum_step_size: s.learning_rate,
        number_of_iterations: s.iterations as usize,
        minimum_convergence_value: s.convergence_threshold,
        convergence_window_size: s.convergence_window as usize,
        lower_limit: 0.0,
        upper_limit: 2.0,
        epsilon: 0.1,
        maximum_line_search_iterations: 10,
        weights: Vec::new(),
        estimate_learning_rate_at_each_iteration: false,
        estimate_learning_rate_once: true,
        scales: None,
    };
    let skip_failed = notices.fix();
    let outcome = if log.verbose {
        log.cout(&format!(
            "Starting optimizer with {trials} starting points\n"
        ));
        let mut progress = Progress {
            log,
            interval: 1,
            total: 0,
        };
        let outcome = multistart::optimize(
            std::mem::take(&mut list),
            Some(&mut local),
            &[],
            &mut problem,
            &mut progress,
            warnings,
            skip_failed,
        )
        .map_err(AiError::Uncaught)?;
        warnings.append(&mut problem.warnings);
        crash_after_failure(&outcome)?;
        if outcome.metric_values.is_empty() {
            // `std::endl` is printed before the value that cannot be read.
            progress.log.cout("\n");
        }
        let best = best_value_printed(&outcome, notices)?;
        progress.log.cout(&format!(
            "\nOptimization complete. Best metric value: {}\n",
            OStream::default().float(best)
        ));
        outcome
    } else {
        let outcome = multistart::optimize(
            std::mem::take(&mut list),
            Some(&mut local),
            &[],
            &mut problem,
            &mut multistart::Silent,
            warnings,
            skip_failed,
        )
        .map_err(AiError::Uncaught)?;
        warnings.append(&mut problem.warnings);
        crash_after_failure(&outcome)?;
        outcome
    };
    // ITK-19: a first start point that throws counts with the value 0 in ITK, so ITK keeps it
    // as the best unless another start point's value is below 0.
    if outcome.failed.first() == Some(&true) {
        let itk_keeps_first = outcome
            .metric_values
            .iter()
            .all(|&v| v >= 0.0 || v.is_nan());
        if itk_keeps_first {
            notices.add(
                &defects::ITK_19,
                if skip_failed {
                    "the first start point threw; larmorx does not count it, ITK would have kept it (with the value 0) as the best"
                } else {
                    "the first start point threw and is kept as the best with the value 0, as in ITK"
                },
            );
        }
    }
    if skip_failed && outcome.failed.iter().all(|&f| f) {
        let why = outcome
            .errors
            .first()
            .map(|(_, e)| format!(" (the first: {})", e.description.trim_end()))
            .unwrap_or_default();
        return Err(AiError::Refused(format!(
            "every start point threw{why}: there is no best one (ITK keeps the first, unoptimised)"
        )));
    }
    let best = outcome
        .best_parameters()
        .ok_or_else(|| AiError::Refused("no start point".into()))?;
    let mut out = LinearXfm::<f64, D>::new(output_kind);
    out.set_center(init.transform.center);
    out.set_parameters(best).map_err(AiError::Uncaught)?;
    Ok(SearchOutcome::Best(out))
}

/// ITK's Mattes metric leaves its joint histogram without a buffer when the histogram's
/// spacing is refused (an analysis region of one intensity or none): ANTs then crashes
/// (`Image::FillBuffer`) in the start point after the one that threw. larmorx refuses there,
/// in both modes (class (a), ITK-20).
fn crash_after_failure(outcome: &multistart::MultiStartOutcome<f64>) -> Result<(), AiError> {
    if let Some(k) = outcome.crashed_at
        && let Some((current, e)) = outcome.errors.last()
    {
        return Err(AiError::Refused(format!(
            "start point {current} threw ({}); ITK's Mattes metric is then left without its joint histogram, and ANTs crashes in start point {k} (ITK-20)",
            e.description.replace('\n', " ")
        )));
    }
    Ok(())
}

/// The value the observer prints at the end: `metricValues[bestIndex]` (ITK). When start
/// points threw, that index points at another start point's value, or past the end of the
/// list: larmorx prints the best start point's value, with `reproduce` the list's entry where
/// it exists. When every start point threw the list is empty and ANTs crashes reading it:
/// refused in both modes.
fn best_value_printed(
    outcome: &multistart::MultiStartOutcome<f64>,
    notices: &mut Notices,
) -> Result<f64, AiError> {
    if outcome.metric_values.is_empty() {
        return Err(AiError::Refused(
            "every start point threw: ANTs' observer reads the best metric value from an empty list and crashes".into(),
        ));
    }
    let failed = outcome.failed.iter().filter(|&&f| f).count();
    if failed == 0 {
        return Ok(outcome.metric_values[outcome.best_index]);
    }
    if notices.reproduce()
        && let Some(&v) = outcome.metric_values.get(outcome.best_index)
    {
        return Ok(v);
    }
    Ok(outcome.minimum)
}
