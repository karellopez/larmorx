// SPDX-License-Identifier: Apache-2.0
//! `antsAffineInitializer` (ANTs v2.6.5, `Examples/antsAffineInitializer.cxx`): an affine
//! transform that aligns two images' centres of mass (and, optionally, their principal axes),
//! then refined by a multi-start search over rotations about the fixed image's principal
//! axes.
//!
//! The steps of `antsAffineInitializerImp<D>`, each as ANTs computes it:
//!
//! 1. [`initial`]: both images' moments (`ImageMomentsCalculator`, centres in double); the
//!    translation between the centres of mass; with `UsePrincipalAxes`, the rotation of
//!    Wahba's problem between the principal axes (`vnl_svd_fixed`, `vnl_inverse`; a negative
//!    determinant turned positive by negating the columns whose diagonal entry is negative,
//!    with ANTs' two ` bad det` lines on standard error); the transform centred at the fixed
//!    image's centre of mass. ANTs writes it first; in 4D it is the result.
//! 2. [`search`] (2D and 3D): Mattes mutual information (32 bins) on every 10th voxel of the
//!    fixed image (no perturbation), with the fixed mask; the physical-shift scales estimated
//!    once, for the search transform centred at 0; one start point per rotation (2D: about
//!    the image plane; 3D: about the tertiary axis, then the secondary one) on
//!    `[−π·f, π·f + step]`; `MultiStartOptimizerv4` with the scales and
//!    `ConjugateGradientLineSearchOptimizerv4` (20 iterations, learning rate 0.1, line search
//!    over `[0, 2]`, ε 0.1, at most 10 steps; no scales estimator, so the learning rate the
//!    line search leaves carries over to the next start point, ITK-21).
//!
//! The images are `itk::Image<float, D>` (their values are kept as doubles, which represent
//! them exactly); the transforms and every computation are in double.

// ITK's loops index several arrays at once; they are kept as written.
#![allow(clippy::needless_range_loop, clippy::manual_memcpy)]

use std::sync::Arc;

use larmorx_core::IndexFloor;
use larmorx_core::defects::Notices;
use larmorx_core::vnl_fixed;
use larmorx_registration::format::OStream;
use larmorx_registration::mask::Mask;
use larmorx_registration::method::{LevelProblem, MetricKind, gradient_image};
use larmorx_registration::metric::ImageMetricv4;
use larmorx_registration::moments::{self, Moments};
use larmorx_registration::optimizer::ConjugateGradientLineSearch;
use larmorx_registration::optimizer::multistart;
use larmorx_registration::optimizer::scales;
use larmorx_registration::transform::{Composite, Kind, LinearXfm, Xfm};
use larmorx_registration::{Image, ItkException, Matrix};

use crate::ai::{AiError, MAX_START_POINTS, best_start_point, crash_after_failure, point_f32};
use crate::registration::helper::Log;

/// `antsAffineInitializer`'s settings, read from its positional arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct AffineInitializerSettings {
    /// `Optional-SearchFactor`: the angle step in degrees (default 10).
    pub search_factor: f64,
    /// `Optional-Radian-Fraction`: the start points' angles run over `[−π·f, π·f + step]`,
    /// `f` clamped to `[0.01, 1]` (default 0.25, ANTs' `pi / 4`).
    pub radian_fraction: f64,
    /// `Optional-bool-UsePrincipalAxes` (default false).
    pub use_principal_axes: bool,
}

impl Default for AffineInitializerSettings {
    fn default() -> Self {
        AffineInitializerSettings {
            search_factor: 10.0,
            radian_fraction: 0.25,
            use_principal_axes: false,
        }
    }
}

/// `degtorad`, as ANTs writes it.
pub const DEGREES_TO_RADIANS: f64 = 0.0174532925;

/// The local optimizer's iterations: always 20 (the command line's value is ignored,
/// ANTS-54).
pub const LOCAL_SEARCH_ITERATIONS: usize = 20;

/// The initial alignment and what the search needs of it.
#[derive(Clone, Debug, PartialEq)]
pub struct InitialAffine<const D: usize> {
    /// `affine1`: the alignment of the centres of mass (and principal axes), centred at the
    /// fixed image's centre of mass. ANTs writes it first.
    pub transform: LinearXfm<f64, D>,
    /// `trans`: the moving centre of mass minus the fixed one.
    pub translation: [f64; D],
    /// `trans2`: the fixed image's centre of mass.
    pub center: [f64; D],
    /// The rotation of Wahba's problem, applied when `use_principal_axes`.
    pub rotation: Matrix<f64, D>,
    /// The search's rotation axes: the tertiary and secondary axes of the fixed image (3D).
    pub axis1: [f64; D],
    pub axis2: [f64; D],
}

/// `vnl_matrix_fixed` product (each entry starts from its first product).
fn mul_fixed<const D: usize>(a: &Matrix<f64, D>, b: &Matrix<f64, D>) -> Matrix<f64, D> {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            let mut s = a[i][0] * b[0][j];
            for k in 1..D {
                s += a[i][k] * b[k][j];
            }
            s
        })
    })
}

fn transpose<const D: usize>(m: &Matrix<f64, D>) -> Matrix<f64, D> {
    std::array::from_fn(|i| std::array::from_fn(|j| m[j][i]))
}

fn flat<const D: usize>(m: &Matrix<f64, D>) -> Vec<f64> {
    m.iter().flatten().copied().collect()
}

fn det<const D: usize>(m: &Matrix<f64, D>) -> f64 {
    vnl_fixed::determinant(&flat(m), D)
}

/// Wahba's rotation (`antsAffineInitializer.cxx:319-339`): `vnl_inverse(V·Uᵀ)` of
/// `vnl_svd_fixed(B)`; a negative determinant is reported on standard error and turned
/// positive by negating the columns whose diagonal entry is negative.
fn wahba<const D: usize>(b: &Matrix<f64, D>, log: &mut Log<'_>) -> Result<Matrix<f64, D>, AiError> {
    let (u, _w, v) = larmorx_core::vnl_svd::vnl_svd(&flat(b), D, D).ok_or_else(|| {
        AiError::Refused("vnl's SVD of the principal axes did not converge".into())
    })?;
    // Column-major: U(i, j) = u[i + D·j].
    let um: Matrix<f64, D> = std::array::from_fn(|i| std::array::from_fn(|j| u[i + D * j]));
    let vm: Matrix<f64, D> = std::array::from_fn(|i| std::array::from_fn(|j| v[i + D * j]));
    let product = mul_fixed(&vm, &transpose(&um));
    let inv = vnl_fixed::inverse(&flat(&product), D);
    let mut a: Matrix<f64, D> = std::array::from_fn(|i| std::array::from_fn(|j| inv[i * D + j]));
    let d = det(&a);
    if d < 0.0 {
        let os = OStream::default();
        let head = format!(
            " bad det {} v {} u {}",
            os.float(d),
            os.float(det(&vm)),
            os.float(det(&um))
        );
        log.cerr(&format!("{head}\n"));
        let mut id: Matrix<f64, D> =
            std::array::from_fn(|i| std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 }));
        for i in 0..D {
            if a[i][i] < 0.0 {
                id[i][i] = -1.0;
            }
        }
        a = mul_fixed(&a, &transpose(&id));
        log.cerr(&format!("{head} new {}\n", os.float(det(&a))));
    }
    Ok(a)
}

/// The two images' moments, as `antsAffineInitializerImp` computes them
/// (`antsAffineInitializer.cxx:263-303`): an image that is missing or whose total mass is 0
/// makes ANTs print ` zero image1 error ` (or `2`) and then abort on the second calculator's
/// `GetTotalMass()`.
pub fn image_moments<const D: usize>(
    fixed: Option<&Image<f64, D>>,
    moving: Option<&Image<f64, D>>,
    log: &mut Log<'_>,
) -> Result<(Moments<D>, Moments<D>), AiError> {
    let get_total_mass = || {
        AiError::Uncaught(ItkException::new(
            "Modules/Filtering/ImageStatistics/include/itkImageMomentsCalculator.hxx",
            186,
            "ImageMomentsCalculator",
            "GetTotalMass() invoked, but the moments have not been computed. Call Compute() first.",
        ))
    };
    let compute = |image: Option<&Image<f64, D>>| -> Result<Option<Moments<D>>, AiError> {
        match image {
            None => Ok(None),
            Some(img) => match moments::compute(img) {
                Ok(m) => Ok(Some(m)),
                Err(moments::MomentsError::ZeroMass(_)) => Ok(None),
                Err(e) => Err(AiError::Refused(e.to_string())),
            },
        }
    };
    let Some(mf) = compute(fixed)? else {
        log.cerr(" zero image1 error ");
        return Err(get_total_mass());
    };
    let Some(mm) = compute(moving)? else {
        log.cerr(" zero image2 error ");
        return Err(get_total_mass());
    };
    Ok((mf, mm))
}

/// The initial alignment (`antsAffineInitializer.cxx:303-372`) from the two images' moments.
pub fn initial<const D: usize>(
    s: &AffineInitializerSettings,
    mf: &Moments<D>,
    mm: &Moments<D>,
    log: &mut Log<'_>,
) -> Result<InitialAffine<D>, AiError> {
    let eigind1 = if D == 3 { 2 } else { 1 };
    let eigind2 = 1;
    let e1p = mf.principal_axes[eigind1];
    let e2p = mm.principal_axes[eigind1];
    let e1s = mf.principal_axes[eigind2];
    let e2s = mm.principal_axes[eigind2];
    // B = outer(evec2_primary, evec1_primary), in 3D after outer(evec2_2ndary, evec1_2ndary).
    let b: Matrix<f64, D> = std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            if D == 3 {
                e2s[i] * e1s[j] + e2p[i] * e1p[j]
            } else {
                e2p[i] * e1p[j]
            }
        })
    });
    let rotation = wahba(&b, log)?;
    let translation: [f64; D] =
        std::array::from_fn(|i| mm.center_of_gravity[i] - mf.center_of_gravity[i]);
    let center = mf.center_of_gravity;
    let mut transform = LinearXfm::<f64, D>::new(Kind::Affine);
    transform.set_identity();
    transform.set_offset(translation);
    if s.use_principal_axes {
        transform.set_matrix(rotation);
    }
    transform.set_center(center);
    // l_rotate_around_tertiatry_and_secondary_axis.
    let mut axis1 = [0.0f64; D];
    let mut axis2 = [0.0f64; D];
    match D {
        3 => {
            let t = [
                e1p[1] * e1s[2] - e1p[2] * e1s[1],
                e1p[2] * e1s[0] - e1p[0] * e1s[2],
                e1p[0] * e1s[1] - e1p[1] * e1s[0],
            ];
            for d in 0..D {
                axis1[d] = t[d];
                axis2[d] = e1s[d];
            }
        }
        2 => {
            axis1 = e1s;
            axis2 = e1p;
        }
        _ => {}
    }
    Ok(InitialAffine {
        transform,
        translation,
        center,
        rotation,
        axis1,
        axis2,
    })
}

/// The angles `for (a = −p; a <= p + step; a += step)` visits; `None` if the loop never ends
/// (a step that does not move `a`) or visits more than `limit` values.
fn angles(p: f64, step: f64, limit: usize) -> Option<Vec<f64>> {
    let mut out = Vec::new();
    let mut a = -p;
    while a <= p + step {
        if out.len() >= limit {
            return None;
        }
        out.push(a);
        let next = a + step;
        if next <= a {
            return None;
        }
        a = next;
    }
    Some(out)
}

/// The search (`antsAffineInitializer.cxx:373-565`), in 2D and 3D: the best affine transform,
/// centred at the fixed image's centre of mass. `moving_f32` is the moving image as read (for
/// its gradient image).
#[allow(clippy::too_many_arguments)]
pub fn search<const D: usize>(
    s: &AffineInitializerSettings,
    init: &InitialAffine<D>,
    fixed: &Arc<Image<f64, D>>,
    moving: &Arc<Image<f64, D>>,
    moving_f32: &Image<f32, D>,
    mask: Option<Mask<D>>,
    notices: &mut Notices,
    warnings: &mut Vec<String>,
    n_threads: usize,
) -> Result<LinearXfm<f64, D>, AiError> {
    let pi = std::f64::consts::PI;
    let step = s.search_factor * DEGREES_TO_RADIANS;
    // `if (temp > 1) temp = 1; if (temp < 0.01) temp = 0.01;` (NaN stays NaN).
    let fraction = s.radian_fraction.clamp(0.01, 1.0);
    let piover4 = pi * fraction;
    // Every 10th voxel of the fixed image, as Point<float>.
    let space = &fixed.space;
    let mut points = Vec::with_capacity(space.len() / 10 + 1);
    for o in (0..space.len()).step_by(10) {
        let p = point_f32(space, &space.index_of(o));
        points.push(std::array::from_fn(|k| f64::from(p[k])));
    }
    // MapFixedSampledPointSetToVirtual (points whose nearest index is outside are skipped).
    points.retain(|p| space.contains(&space.point_to_index(p)));
    let moving_gradient =
        Arc::new(gradient_image::<f32, D>(moving_f32, n_threads).map_err(AiError::Uncaught)?);
    let metric = ImageMetricv4 {
        fixed: Arc::clone(fixed),
        moving: Arc::clone(moving),
        fixed_mask: mask,
        moving_mask: None,
        fixed_transform: None,
        virtual_space: fixed.space.clone(),
        samples: Some(Arc::new(points)),
        fixed_gradient_image: None,
        moving_gradient_image: Some(moving_gradient),
        pixel_f32: true,
    };
    // The scales, estimated once for the search transform as it is then: centred at 0, with
    // the initial transform's parameters.
    let mut scales_transform = LinearXfm::<f64, D>::new(Kind::Affine);
    scales_transform
        .set_parameters(&init.transform.parameters())
        .map_err(AiError::Uncaught)?;
    let mut scales_composite = Composite::new(vec![Xfm::Linear(scales_transform)]);
    let moving_scales =
        scales::estimate_scales(&mut scales_composite, &scales::corners(&fixed.space))
            .map_err(AiError::Uncaught)?;
    // The start points.
    let never = || {
        AiError::Refused(format!(
            "the search factor: ANTs' loop over the start points never ends (or lists more than {MAX_START_POINTS})"
        ))
    };
    let a1 = angles(piover4, step, MAX_START_POINTS).ok_or_else(never)?;
    let a2 = if D == 3 { a1.clone() } else { vec![0.0] };
    if a1.len().saturating_mul(a2.len()) > MAX_START_POINTS {
        return Err(never());
    }
    let mut list = Vec::with_capacity(a1.len() * a2.len());
    let mut affine = LinearXfm::<f64, D>::new(Kind::Affine);
    for &ang1 in &a1 {
        for &ang2 in &a2 {
            affine.set_identity();
            affine.set_center(init.center);
            affine.set_offset(init.translation);
            if s.use_principal_axes {
                affine.set_matrix(init.rotation);
            }
            if D == 3 {
                affine.rotate_3d(&init.axis1, ang1, true);
                affine.rotate_3d(&init.axis2, ang2, true);
            } else {
                affine.rotate_2d(ang1, true);
            }
            affine.scale_uniform(1.0, false);
            list.push(affine.parameters());
        }
    }
    if list.is_empty() {
        return Err(AiError::Refused(
            "no start point (a search factor or radian fraction that is not a number): ANTs reads the best parameters from an empty list and crashes".into(),
        ));
    }
    let floor = IndexFloor::new(notices.mode());
    let mut problem = LevelProblem::single(
        metric,
        MetricKind::Mattes { bins: 32 },
        Composite::new(vec![Xfm::Linear(affine)]),
        &floor,
    )
    .map_err(AiError::Uncaught)?;
    let mut local = ConjugateGradientLineSearch {
        learning_rate: 0.1,
        maximum_step_size: 0.1,
        number_of_iterations: LOCAL_SEARCH_ITERATIONS,
        minimum_convergence_value: 1e-6,
        convergence_window_size: 5,
        lower_limit: 0.0,
        upper_limit: 2.0,
        epsilon: 0.1,
        maximum_line_search_iterations: 10,
        weights: Vec::new(),
        estimate_learning_rate_at_each_iteration: false,
        estimate_learning_rate_once: true,
        scales: Some(moving_scales.clone()),
    };
    let skip_failed = notices.fix();
    let outcome = multistart::optimize(
        list,
        Some(&mut local),
        &moving_scales,
        &mut problem,
        &mut multistart::Silent,
        warnings,
        skip_failed,
    )
    .map_err(AiError::Uncaught)?;
    warnings.append(&mut problem.warnings);
    crash_after_failure(&outcome)?;
    let best = best_start_point(&outcome, notices, skip_failed)?;
    let mut out = LinearXfm::<f64, D>::new(Kind::Affine);
    out.set_center(init.center);
    out.set_parameters(&best).map_err(AiError::Uncaught)?;
    Ok(out)
}
