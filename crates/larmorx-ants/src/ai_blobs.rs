// SPDX-License-Identifier: Apache-2.0
//! `antsAI --align-blobs` (ANTs v2.6.5, `Examples/antsAI.cxx:156-604`,
//! `GetTransformFromFeatureMatching`): the initial transform from matched blobs.
//!
//! As `antsAI` computes it, with the values it hard-codes ("taken from Brian in
//! ImageMath.cxx"):
//! 1. **the blobs** of each image: ANTs' `MultiScaleLaplacianBlobDetectorImageFilter` from
//!    `t = 1` to `t = 1024` in 10 steps per octave, at most `numberOfBlobsToExtract`
//!    ([`larmorx_image::blobs`]);
//! 2. **the correspondence matrix**: for each fixed blob and each moving blob whose centre
//!    voxel is above `1e-4`, the correlation of their spherical patches of radius 20 voxels
//!    (`PatchCorrelation`), negative correlations set to 0. A patch is first oriented: the
//!    covariance of each image's gradient (`GradientRecursiveGaussianImageFilter`, σ 1 mm, in
//!    `float`) over the patch, weighted by `expf(−d/400)` normalised, its eigenvectors
//!    (`vnl_symmetric_eigensystem<float>`), Wahba's rotation between them (`vnl_svd<float>`),
//!    and the moving patch's points rotated about their centroid and interpolated linearly
//!    (float coordinates). A patch that leaves either image has correlation 0;
//! 3. **the pairs**: repeatedly the largest entry of the matrix (`arg_max`, the first in row
//!    order), kept when both blobs' radii are above 1 and within 0.25 of each other and both
//!    centres above `1e-4`, its row and column then zeroed, until `numberOfBlobsToMatch` pairs
//!    or as many steps as fixed blobs;
//! 4. **the landmarks**: the pairs with at least 3 others whose squared-distance ratio is
//!    within 0.1 of 1;
//! 5. **the fit**: `LandmarkBasedTransformInitializer` ([`larmorx_registration::landmark`]) with
//!    an `AffineTransform` for `Affine`, otherwise `VersorRigid3DTransform` (3D) or
//!    `Rigid2DTransform` (2D); its matrix and offset become the initial transform, centred
//!    at 0.
//!
//! Every value is computed in ANTs' types and order: `float` for the patches and their sums,
//! double for the points and the fits.
//!
//! **Corrected by default** (D13, `upstream_defects`), each with a notice where the
//! correction changes the result:
//! - ANTS-112: the detector keeps the `N` strongest maxima ([`larmorx_image::blobs`]);
//! - ANTS-111: the pairing stops when no positive correlation is left (ANTs then pairs the
//!   first fixed and the first moving blob again at every step);
//! - ANTS-110: the landmarks are the pairs whose distances to the other pairs are preserved,
//!   `|m_i − m_j|² / |f_i − f_j|²` within 0.1 of 1, as the comment says and `ImageMath
//!   MatchBlobs` computes (ANTs compares `|f_j − m_j|² / |f_i − m_i|²`, the lengths of two
//!   pairs' displacements);
//! - ANTS-114: a rigid fit without a landmark is an error (ANTs writes NaNs);
//! - ANTS-116: two blobs' patches are compared in the orientation that matches best: ANTs'
//!   rotation `V·Uᵀ` is neither checked for a reflection nor independent of the arbitrary
//!   signs of the patches' axes; corrected, each sign combination of the moving axes with
//!   Kabsch's proper rotation, the largest correlation kept (named where the pairs, the
//!   landmarks or the transform change);
//! - ITK-60: the 2D rigid fit's angle near a quarter turn ([`larmorx_registration::landmark`]).

#![allow(clippy::needless_range_loop)]

use larmorx_core::defects::{self, Notices};
use larmorx_core::math;
use larmorx_core::parallel;
use larmorx_core::vnl_eigen::symmetric_eigensystem;
use larmorx_core::vnl_svd_f32::{matmul_f32, vnl_svd_f32};
use larmorx_image::VolumeRef;
pub use larmorx_image::blobs::Blob;
use larmorx_image::blobs::{self, BlobSettings, Thresholds};
use larmorx_registration::interp::Linear;
use larmorx_registration::landmark::{self, LandmarkFit};
use larmorx_registration::method::gradient_image_with;
use larmorx_registration::{Image, ItkException, Matrix};
use rayon::prelude::*;

/// The patch radius in voxels (`radiusValue`).
pub const RADIUS: i64 = 20;
const GRADIENT_SIGMA: f32 = 1.0;
const MAX_RADIUS_DIFFERENCE: f32 = 0.25;
const DISTANCE_PRESERVATION: f32 = 0.1;
const MIN_NEIGHBOURS: f32 = 3.0;

/// Which landmark transform `antsAI` fits (`strcmp(transform, "affine")`, else the rigid one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandmarkKind {
    Affine,
    Rigid,
}

/// Why the blob alignment stops.
#[derive(Clone, Debug, PartialEq)]
pub enum BlobFailure {
    /// An ITK exception `antsAI` does not catch (ANTs aborts): a Laplacian on an axis of fewer
    /// than 4 voxels, too few landmarks for the affine fit.
    Uncaught(ItkException),
    /// ANTs prints "The moving image or fixed image blobs list is empty." and then
    /// dereferences a null transform (it crashes).
    EmptyBlobList,
    /// ANTS-114 corrected: a rigid fit of no landmark.
    NoLandmark,
}

/// Everything the alignment computed, for the command line, the Python API and the evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct BlobAlignment<const D: usize> {
    pub fixed_blobs: Vec<Blob>,
    pub moving_blobs: Vec<Blob>,
    /// The correspondence matrix (`fixed_blobs.len() × moving_blobs.len()`, row-major): each
    /// pair's patch correlation, 0 where it is negative or was not computed.
    pub correspondence: Vec<f32>,
    /// The pairs chosen, as (fixed blob, moving blob), in order.
    pub pairs: Vec<(usize, usize)>,
    /// Which pairs became landmarks.
    pub landmarks: Vec<usize>,
    pub fixed_landmarks: Vec<[f64; D]>,
    pub moving_landmarks: Vec<[f64; D]>,
    /// The landmark transform's matrix and offset (the initial transform, centred at 0).
    pub matrix: Matrix<f64, D>,
    pub offset: [f64; D],
    /// What ITK printed on standard error while fitting.
    pub messages: Vec<String>,
}

/// The detector's settings in `antsAI`: `t` from `pow(1.0, 1.0)` to `pow(2.0, 10.0)` (stored
/// in `float`), 10 steps per octave.
pub fn detector_settings(number_of_blobs: u32) -> BlobSettings {
    BlobSettings {
        start_t: f64::from(1.0f32),
        end_t: f64::from(1024.0f32),
        steps_per_octave: 10,
        number_of_blobs: number_of_blobs as usize,
    }
}

/// The detector's blobs of `image` in `notices`' mode, naming ANTS-112 where the two
/// thresholds keep different blobs.
pub fn detect<const D: usize>(
    image: &Image<f32, D>,
    number_of_blobs: u32,
    which: &str,
    notices: &mut Notices,
    n_threads: usize,
) -> Result<Vec<Blob>, BlobFailure> {
    let size: Vec<usize> = image.space.size.to_vec();
    let spacing: Vec<f64> = image.space.spacing.to_vec();
    let det = blobs::detect(
        VolumeRef::new(&image.data, &size, &spacing),
        &detector_settings(number_of_blobs),
        n_threads,
    )
    .map_err(|e| {
        BlobFailure::Uncaught(ItkException::new(
            "Modules/Filtering/ImageFilterBase/include/itkRecursiveSeparableImageFilter.hxx",
            223,
            "RecursiveGaussianImageFilter",
            e.to_string(),
        ))
    })?;
    if det.ants != det.strongest {
        notices.add(
            &defects::ANTS_112,
            format!(
                "the {which} image's blobs: the {} strongest maxima differ from ANTs' {}",
                det.strongest.len(),
                det.ants.len()
            ),
        );
    }
    let mode = if notices.fix() {
        Thresholds::Strongest
    } else {
        Thresholds::Ants
    };
    Ok(det.blobs(mode).to_vec())
}

/// An image's `float` gradient (`GradientRecursiveGaussianImageFilter<Image<float>,
/// Image<CovariantVector<float>>>`, σ 1, not normalised, the direction applied in double and
/// stored in float).
fn gradient_f32<const D: usize>(
    image: &Image<f32, D>,
    n_threads: usize,
) -> Result<Vec<[f32; D]>, ItkException> {
    let g = gradient_image_with::<f32, D>(image, f64::from(GRADIENT_SIGMA), false, n_threads)?;
    Ok(g.data
        .iter()
        .map(|v| std::array::from_fn(|k| v[k] as f32))
        .collect())
}

/// The spherical patch: the offsets of the `(2r + 1)^D` neighbourhood within distance `r`, in
/// ITK's order, with their weights `expf(−d/r²)` normalised by their float sum.
pub fn patch<const D: usize>() -> (Vec<[i64; D]>, Vec<f32>) {
    let side = 2 * RADIUS + 1;
    let count = (side as usize).pow(D as u32);
    let mut offsets = Vec::new();
    let mut weights = Vec::new();
    let mut sum = 0.0f32;
    for i in 0..count {
        let mut off = [0i64; D];
        let mut r = i;
        for k in 0..D {
            off[k] = (r % side as usize) as i64 - RADIUS;
            r /= side as usize;
        }
        // `distance += sqr(index[j] − zeroIndex[j])` in float, then sqrtf.
        let mut distance = 0.0f32;
        for k in 0..D {
            distance += (off[k] * off[k]) as f32;
        }
        let distance = distance.sqrt();
        if distance <= RADIUS as f32 {
            offsets.push(off);
            let w = math::expf(-distance / (RADIUS * RADIUS) as f32);
            weights.push(w);
            sum += w;
        }
    }
    for w in &mut weights {
        *w /= sum;
    }
    (offsets, weights)
}

/// `vnl_c_vector<float>::mean`: the float sum over the float count.
fn mean_f32(v: &[f32]) -> f32 {
    let mut s = 0.0f32;
    for &x in v {
        s += x;
    }
    s / v.len() as f32
}

/// `squared_magnitude` in float.
fn sumsq_f32(v: &[f32]) -> f32 {
    let mut s = 0.0f32;
    for &x in v {
        s += x * x;
    }
    s
}

/// The images and precomputed pieces a patch correlation reads.
struct Patches<'a, const D: usize> {
    fixed: &'a Image<f32, D>,
    moving: &'a Image<f32, D>,
    fixed_gradient: &'a [[f32; D]],
    moving_gradient: &'a [[f32; D]],
    offsets: &'a [[i64; D]],
    weights: &'a [f32],
}

impl<const D: usize> Patches<'_, D> {
    /// Whether the cube of radius 20 about `c` lies in the image (the sphere's extreme
    /// points along the axes are in it, so this is `IsInside` for every patch index).
    fn inside(space: &larmorx_registration::Space<D>, c: &[i64; D]) -> bool {
        (0..D).all(|k| {
            c[k] - RADIUS >= space.start[k] && c[k] + RADIUS < space.start[k] + space.size[k] as i64
        })
    }

    /// `PatchCorrelation` of the patches about `fc` (fixed) and `mc` (moving): ANTs' value
    /// and the corrected one (ANTS-116), each 0 where it is NaN or infinite.
    ///
    /// ANTs turns the moving patch with Wahba's `A = V·Uᵀ` of `B = Σ outer(moving axis, fixed
    /// axis)`, the patches' principal axes from `vnl_symmetric_eigensystem`, whose signs are
    /// arbitrary, and never checks `det A`: identical patches can be compared reflected, or
    /// turned by 180° about an axis. Corrected: every sign combination of the moving axes (2 in
    /// 2D, 4 in 3D), each with Kabsch's proper rotation (the singular vector of the smallest
    /// singular value negated where `det A < 0`), and the largest correlation kept.
    fn correlation(&self, fc: &[i64; D], mc: &[i64; D]) -> PatchCorrelations {
        let zero = PatchCorrelations {
            ants: 0.0,
            oriented: 0.0,
        };
        if !Self::inside(&self.fixed.space, fc) || !Self::inside(&self.moving.space, mc) {
            return zero;
        }
        let n = self.offsets.len();
        let weight = 1.0f32 / n as f32;
        let mut fixed_samples = vec![0.0f32; n];
        let mut fixed_g = vec![[0.0f32; D]; n];
        let mut moving_g = vec![[0.0f32; D]; n];
        let mut moving_points: Vec<[f64; D]> = Vec::with_capacity(n);
        let mut centroid = [0.0f64; D];
        let fspace = &self.fixed.space;
        let mspace = &self.moving.space;
        for i in 0..n {
            let fi: [i64; D] = std::array::from_fn(|k| fc[k] + self.offsets[i][k]);
            let mi: [i64; D] = std::array::from_fn(|k| mc[k] + self.offsets[i][k]);
            let fo = fspace.offset(&fi);
            let mo = mspace.offset(&mi);
            fixed_samples[i] = self.fixed.data[fo];
            for k in 0..D {
                fixed_g[i][k] = self.fixed_gradient[fo][k] * self.weights[i];
                moving_g[i][k] = self.moving_gradient[mo][k] * self.weights[i];
            }
            let p = mspace.index_to_point(&mi);
            for k in 0..D {
                centroid[k] += p[k] * f64::from(weight);
            }
            moving_points.push(p);
        }
        let fm = mean_f32(&fixed_samples);
        for v in &mut fixed_samples {
            *v -= fm;
        }
        let fixed_sd = sumsq_f32(&fixed_samples).sqrt();
        // The gradient covariances GᵀG in float.
        let cov = |g: &[[f32; D]]| -> Vec<f32> {
            let mut c = vec![0.0f32; D * D];
            for a in 0..D {
                for b in 0..D {
                    let mut s = 0.0f32;
                    for row in g {
                        s += row[a] * row[b];
                    }
                    c[a * D + b] = s;
                }
            }
            c
        };
        let fcov = cov(&fixed_g);
        let mcov = cov(&moving_g);
        // vnl_symmetric_eigensystem<float>: computed in double, stored in float.
        let eig = |c: &[f32]| -> Option<Vec<Vec<f32>>> {
            let cd: Vec<f64> = c.iter().map(|&v| f64::from(v)).collect();
            let e = symmetric_eigensystem(&cd, D);
            if e.ierr != 0 {
                return None;
            }
            // Column j of V is eigenvector j.
            Some(
                (0..D)
                    .map(|j| (0..D).map(|r| e.vectors[r * D + j] as f32).collect())
                    .collect(),
            )
        };
        let (Some(fe), Some(me)) = (eig(&fcov), eig(&mcov)) else {
            return zero;
        };
        // B = Σ outer(sign · moving axis, fixed axis): 2D the eigenvector 1, 3D 2 and 1.
        let outer = |signs: [f32; 2]| -> Vec<f32> {
            let mut b = vec![0.0f32; D * D];
            if D == 2 {
                for i in 0..D {
                    for j in 0..D {
                        b[i * D + j] = (signs[0] * me[1][i]) * fe[1][j];
                    }
                }
            } else {
                for i in 0..D {
                    for j in 0..D {
                        b[i * D + j] =
                            (signs[0] * me[2][i]) * fe[2][j] + (signs[1] * me[1][i]) * fe[1][j];
                    }
                }
            }
            b
        };
        // Wahba's `V·Uᵀ` of `vnl_svd<float>(B)`; `proper`: Kabsch's rotation (det +1).
        let rotation = |b: &[f32], proper: bool| -> Option<(Vec<f32>, bool)> {
            let (u, w, mut v) = vnl_svd_f32(b, D, D)?;
            let product = |v: &[f32]| -> Vec<f32> {
                // A = V · Uᵀ (U and V column-major): V(r, k) = v[r + D·k], Uᵀ(k, c) = u[c + D·k].
                let vr: Vec<f32> = (0..D * D).map(|k| v[(k / D) + D * (k % D)]).collect();
                let utr: Vec<f32> = (0..D * D)
                    .map(|k| {
                        let (r, c) = (k / D, k % D);
                        u[c + D * r]
                    })
                    .collect();
                matmul_f32(&vr, D, D, &utr, D)
            };
            let a = product(&v);
            let reflected = determinant::<D>(&a) < 0.0;
            if !(proper && reflected) {
                return Some((a, reflected));
            }
            // Negate the right singular vector of the smallest singular value.
            let k = (0..D)
                .min_by(|&x, &y| w[x].total_cmp(&w[y]).then(y.cmp(&x)))
                .expect("D > 0");
            for r in 0..D {
                v[r + D * k] = -v[r + D * k];
            }
            Some((product(&v), true))
        };
        let interp = Linear::new(self.moving);
        // The correlation of the fixed patch with the moving patch turned by `a`.
        let correlate = |a: &[f32]| -> f32 {
            let mut moving_samples = vec![0.0f32; n];
            for i in 0..n {
                let mut q = moving_points[i];
                let mut vecf = [0.0f32; D];
                for k in 0..D {
                    q[k] -= centroid[k];
                    vecf[k] = q[k] as f32;
                }
                let mut point = [0.0f64; D];
                for r in 0..D {
                    let mut s = 0.0f32;
                    for c in 0..D {
                        s += a[r * D + c] * vecf[c];
                    }
                    point[r] = f64::from(s) + centroid[r];
                }
                // The interpolator's PointType is Point<float>.
                let pf: [f64; D] = std::array::from_fn(|k| f64::from(point[k] as f32));
                if interp.is_inside(&pf) {
                    moving_samples[i] =
                        interp.evaluate(&pf, &larmorx_core::IndexFloor::default()) as f32;
                } else {
                    return 0.0;
                }
            }
            let mm = mean_f32(&moving_samples);
            for v in &mut moving_samples {
                *v -= mm;
            }
            let moving_sd = sumsq_f32(&moving_samples).sqrt();
            let mut ip = 0.0f32;
            for i in 0..n {
                ip += fixed_samples[i] * moving_samples[i];
            }
            let c = ip / (fixed_sd * moving_sd);
            if c.is_nan() || c.is_infinite() {
                0.0
            } else {
                c
            }
        };
        // ANTs: the axes as the eigensystem gives them, no determinant check.
        let ants = rotation(&outer([1.0, 1.0]), false);
        let ants_value = ants.as_ref().map_or(0.0, |(a, _)| correlate(a));
        let signs: &[[f32; 2]] = if D == 2 {
            &[[1.0, 1.0], [-1.0, 1.0]]
        } else {
            &[[1.0, 1.0], [-1.0, 1.0], [1.0, -1.0], [-1.0, -1.0]]
        };
        let mut oriented = f32::NEG_INFINITY;
        for (c, &s) in signs.iter().enumerate() {
            let value = match (&ants, c) {
                // ANTs' own rotation when it is proper.
                (Some((_, false)), 0) => ants_value,
                _ => match rotation(&outer(s), true) {
                    Some((a, _)) => correlate(&a),
                    None => continue,
                },
            };
            oriented = oriented.max(value);
        }
        PatchCorrelations {
            ants: ants_value,
            oriented: if oriented == f32::NEG_INFINITY {
                0.0
            } else {
                oriented
            },
        }
    }
}

/// A patch correlation as ANTs computes it and as corrected (ANTS-116).
#[derive(Clone, Copy, Debug, PartialEq)]
struct PatchCorrelations {
    ants: f32,
    oriented: f32,
}

/// The determinant of a `D × D` float matrix (row-major), in double: its sign tells a rotation
/// from a reflection.
fn determinant<const D: usize>(a: &[f32]) -> f64 {
    let m = |r: usize, c: usize| f64::from(a[r * D + c]);
    match D {
        1 => m(0, 0),
        2 => m(0, 0) * m(1, 1) - m(0, 1) * m(1, 0),
        _ => {
            m(0, 0) * (m(1, 1) * m(2, 2) - m(1, 2) * m(2, 1))
                - m(0, 1) * (m(1, 0) * m(2, 2) - m(1, 2) * m(2, 0))
                + m(0, 2) * (m(1, 0) * m(2, 1) - m(1, 1) * m(2, 0))
        }
    }
}

/// The correspondence matrices of [`correspondence`]: ANTs' and the corrected one.
#[derive(Clone, Debug, PartialEq)]
pub struct Correspondence {
    /// `GetBlobCorrespondenceMatrix` as ANTs computes it.
    pub ants: Vec<f32>,
    /// With the patches compared in the orientation that matches best (ANTS-116).
    pub oriented: Vec<f32>,
}

/// `GetBlobCorrespondenceMatrix`: the `fixed × moving` matrices (row-major, float), ANTs' and
/// the corrected one (ANTS-116).
#[allow(clippy::too_many_arguments)]
pub fn correspondence<const D: usize>(
    fixed: &Image<f32, D>,
    moving: &Image<f32, D>,
    fixed_blobs: &[Blob],
    moving_blobs: &[Blob],
    n_threads: usize,
) -> Result<Correspondence, ItkException> {
    let fg = gradient_f32(fixed, n_threads)?;
    let mg = gradient_f32(moving, n_threads)?;
    let (offsets, weights) = patch::<D>();
    let p = Patches {
        fixed,
        moving,
        fixed_gradient: &fg,
        moving_gradient: &mg,
        offsets: &offsets,
        weights: &weights,
    };
    let (nf, nm) = (fixed_blobs.len(), moving_blobs.len());
    let center = |b: &Blob| -> [i64; D] { std::array::from_fn(|k| b.center[k]) };
    let bright = |img: &Image<f32, D>, c: &[i64; D]| img.at(c) > 1.0e-4f32;
    let out = parallel::with_threads(n_threads, || {
        (0..nf * nm)
            .into_par_iter()
            .map(|k| {
                let (i, j) = (k / nm, k % nm);
                let fc = center(&fixed_blobs[i]);
                let mc = center(&moving_blobs[j]);
                if !bright(fixed, &fc) || !bright(moving, &mc) {
                    return (0.0f32, 0.0f32);
                }
                let c = p.correlation(&fc, &mc);
                let clip = |v: f32| if v < 0.0 { 0.0 } else { v };
                (clip(c.ants), clip(c.oriented))
            })
            .collect::<Vec<(f32, f32)>>()
    })
    .map_err(|e| ItkException::new("", 0, "", e.to_string()))?;
    let (ants, oriented) = out.into_iter().unzip();
    Ok(Correspondence { ants, oriented })
}

/// The pairing loop (`antsAI.cxx:494-527`) on the correspondence matrix `m` (row-major,
/// `nf × nm`): the pairs, in order. `stop_when_exhausted`: ANTS-111 corrected, the loop stops
/// once the largest entry is not positive (ANTs goes on taking entry 0, the first fixed and
/// moving blob, at every step).
#[allow(clippy::too_many_arguments)]
pub fn pair_blobs(
    m: &[f32],
    nf: usize,
    nm: usize,
    fixed: &[Blob],
    moving: &[Blob],
    fixed_bright: &[bool],
    moving_bright: &[bool],
    number_to_match: u32,
    stop_when_exhausted: bool,
) -> Vec<(usize, usize)> {
    let mut m = m.to_vec();
    let mut pairs = Vec::new();
    let mut match_point = 1u32;
    let mut fixed_count = 0usize;
    while match_point <= number_to_match && fixed_count < nf {
        // arg_max: the first largest entry in row order.
        let mut best = 0usize;
        let mut tmp = m[0];
        for k in 1..m.len() {
            if m[k] > tmp {
                tmp = m[k];
                best = k;
            }
        }
        // No positive correlation left (the matrix holds no NaN: ANTs' 0 for those).
        if stop_when_exhausted && tmp.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            break;
        }
        let (row, col) = (best / nm, best % nm);
        let fb = &fixed[row];
        let mb = &moving[col];
        if mb.object_radius() > 1.0
            && ((mb.object_radius() - fb.object_radius()).abs() as f32) < MAX_RADIUS_DIFFERENCE
            && fixed_bright[row]
            && moving_bright[col]
        {
            pairs.push((row, col));
            match_point += 1;
        }
        for c in 0..nm {
            m[row * nm + c] = 0.0;
        }
        for r in 0..nf {
            m[r * nm + col] = 0.0;
        }
        fixed_count += 1;
    }
    pairs
}

/// The pairs that become landmarks (`antsAI.cxx:534-596`): those with at least 3 others
/// whose squared-distance ratio, stored in float, is within 0.1 of 1. `preserved`: ANTS-110
/// corrected, the ratio of the distances between the two pairs' blobs in the moving and the
/// fixed image; otherwise ANTs' ratio of the two pairs' displacement lengths (with its
/// symmetric assignment, which leaves `ratio(i, j)` = `d_min(i,j) / d_max(i,j)` by index).
pub fn landmark_pairs<const D: usize>(
    fixed_points: &[[f64; D]],
    moving_points: &[[f64; D]],
    preserved: bool,
) -> Vec<usize> {
    let n = fixed_points.len();
    let sq = |a: &[f64; D], b: &[f64; D]| -> f64 {
        // Point::SquaredEuclideanDistanceTo: Σ (a − b)² from 0 in order.
        let mut s = 0.0f64;
        for k in 0..D {
            let d = b[k] - a[k];
            s += d * d;
        }
        s
    };
    let mut ratio = vec![0.0f32; n * n];
    for i in 0..n {
        for j in 0..n {
            let r = if preserved {
                sq(&moving_points[i], &moving_points[j]) / sq(&fixed_points[i], &fixed_points[j])
            } else {
                sq(&fixed_points[j], &moving_points[j]) / sq(&fixed_points[i], &moving_points[i])
            } as f32;
            ratio[i * n + j] = r;
            ratio[j * n + i] = r;
        }
    }
    let mut out = Vec::new();
    for i in 0..n {
        let mut count = 0u32;
        for j in 0..n {
            if j != i && (ratio[i * n + j] - 1.0f32).abs() < DISTANCE_PRESERVATION {
                count += 1;
            }
        }
        if count as f32 >= MIN_NEIGHBOURS {
            out.push(i);
        }
    }
    out
}

/// `GetTransformFromFeatureMatching` and the initial transform `antsAI` makes of it, in
/// `notices`' mode.
pub fn align<const D: usize>(
    fixed: &Image<f32, D>,
    moving: &Image<f32, D>,
    blobs_to_extract: u32,
    blobs_to_match: u32,
    kind: LandmarkKind,
    notices: &mut Notices,
    n_threads: usize,
) -> Result<BlobAlignment<D>, BlobFailure> {
    let fixed_blobs = detect(fixed, blobs_to_extract, "fixed", notices, n_threads)?;
    let moving_blobs = detect(moving, blobs_to_extract, "moving", notices, n_threads)?;
    if fixed_blobs.is_empty() || moving_blobs.is_empty() {
        return Err(BlobFailure::EmptyBlobList);
    }
    let both = correspondence(fixed, moving, &fixed_blobs, &moving_blobs, n_threads)
        .map_err(BlobFailure::Uncaught)?;
    let (m, other) = if notices.fix() {
        (both.oriented, both.ants)
    } else {
        (both.ants, both.oriented)
    };
    let changed = m.iter().zip(&other).filter(|(a, b)| a != b).count();
    let total = m.len();
    let result = match_and_fit(
        fixed,
        moving,
        &fixed_blobs,
        &moving_blobs,
        m,
        blobs_to_match,
        kind,
        notices,
    );
    if changed > 0 {
        // ANTS-116: named where the other matrix changes the outcome.
        let mut scratch = Notices::new(notices.mode());
        let alternative = match_and_fit(
            fixed,
            moving,
            &fixed_blobs,
            &moving_blobs,
            other,
            blobs_to_match,
            kind,
            &mut scratch,
        );
        let outcome = |r: &Result<BlobAlignment<D>, BlobFailure>| -> String {
            match r {
                Ok(a) => format!(
                    "{} landmark(s) of {} pair(s)",
                    a.landmarks.len(),
                    a.pairs.len()
                ),
                Err(BlobFailure::NoLandmark) => "no landmark".into(),
                Err(e) => format!("{e:?}"),
            }
        };
        let same = match (&result, &alternative) {
            (Ok(a), Ok(b)) => {
                let bits = |x: &BlobAlignment<D>| -> Vec<u64> {
                    x.matrix
                        .iter()
                        .flatten()
                        .chain(&x.offset)
                        .map(|v| v.to_bits())
                        .collect()
                };
                a.pairs == b.pairs && a.landmarks == b.landmarks && bits(a) == bits(b)
            }
            (Err(a), Err(b)) => a == b,
            _ => false,
        };
        if !same {
            let (ants, fixed_outcome) = if notices.fix() {
                (outcome(&alternative), outcome(&result))
            } else {
                (outcome(&result), outcome(&alternative))
            };
            notices.add(
                &defects::ANTS_116,
                if notices.fix() {
                    format!(
                        "{changed} of {total} patch correlations change when the patches are                          compared in the orientation that matches best (the rotation's                          determinant checked, the axes' signs tried): {fixed_outcome}; ANTs                          compares them as its rotation leaves them: {ants}"
                    )
                } else {
                    format!(
                        "the patches are compared as ANTs' rotation leaves them (possibly                          reflected or turned): {ants}; in the orientation that matches best                          {changed} of {total} patch correlations change: {fixed_outcome}"
                    )
                },
            );
        }
    }
    result
}

/// The pairing, the landmarks and the fit of [`align`] on the correspondence matrix `m`.
#[allow(clippy::too_many_arguments)]
fn match_and_fit<const D: usize>(
    fixed: &Image<f32, D>,
    moving: &Image<f32, D>,
    fixed_blobs: &[Blob],
    moving_blobs: &[Blob],
    m: Vec<f32>,
    blobs_to_match: u32,
    kind: LandmarkKind,
    notices: &mut Notices,
) -> Result<BlobAlignment<D>, BlobFailure> {
    let (nf, nm) = (fixed_blobs.len(), moving_blobs.len());
    let center = |b: &Blob| -> [i64; D] { std::array::from_fn(|k| b.center[k]) };
    let fb: Vec<bool> = fixed_blobs
        .iter()
        .map(|b| fixed.at(&center(b)) > 1.0e-4f32)
        .collect();
    let mb: Vec<bool> = moving_blobs
        .iter()
        .map(|b| moving.at(&center(b)) > 1.0e-4f32)
        .collect();
    let ants_pairs = pair_blobs(
        &m,
        nf,
        nm,
        fixed_blobs,
        moving_blobs,
        &fb,
        &mb,
        blobs_to_match,
        false,
    );
    let stopped = pair_blobs(
        &m,
        nf,
        nm,
        fixed_blobs,
        moving_blobs,
        &fb,
        &mb,
        blobs_to_match,
        true,
    );
    if ants_pairs != stopped {
        notices.add(
            &defects::ANTS_111,
            format!(
                "ANTs takes {} more blob pair(s) after the positive correlations run out (the first fixed and moving blob, again): {} pairs instead of {}",
                ants_pairs.len() - stopped.len(),
                ants_pairs.len(),
                stopped.len()
            ),
        );
    }
    let pairs = if notices.fix() { stopped } else { ants_pairs };
    let point = |img: &Image<f32, D>, b: &Blob| img.space.index_to_point(&center(b));
    let fixed_points: Vec<[f64; D]> = pairs
        .iter()
        .map(|&(i, _)| point(fixed, &fixed_blobs[i]))
        .collect();
    let moving_points: Vec<[f64; D]> = pairs
        .iter()
        .map(|&(_, j)| point(moving, &moving_blobs[j]))
        .collect();
    let preserved = notices.fix();
    let landmarks = landmark_pairs(&fixed_points, &moving_points, preserved);
    let other = landmark_pairs(&fixed_points, &moving_points, !preserved);
    if other != landmarks {
        let (ants, fixed_ones) = if preserved {
            (&other, &landmarks)
        } else {
            (&landmarks, &other)
        };
        notices.add(
            &defects::ANTS_110,
            format!(
                "{} of {} blob pairs preserve their distances to at least 3 others; ANTs' test of the displacement lengths keeps {}",
                fixed_ones.len(),
                pairs.len(),
                ants.len()
            ),
        );
    }
    let fl: Vec<[f64; D]> = landmarks.iter().map(|&i| fixed_points[i]).collect();
    let ml: Vec<[f64; D]> = landmarks.iter().map(|&i| moving_points[i]).collect();
    let fit: LandmarkFit<D> = match kind {
        LandmarkKind::Affine => landmark::affine(&fl, &ml).map_err(BlobFailure::Uncaught)?,
        LandmarkKind::Rigid => {
            if fl.is_empty() {
                notices.add(
                    &defects::ANTS_114,
                    if notices.fix() {
                        "no blob pair is a landmark: the rigid fit has nothing to fit (ANTs writes NaNs)"
                    } else {
                        "no blob pair is a landmark: the rigid fit is NaN, as in ANTs"
                    },
                );
                if notices.fix() {
                    return Err(BlobFailure::NoLandmark);
                }
            }
            rigid_fit::<D>(&fl, &ml, notices).map_err(BlobFailure::Uncaught)?
        }
    };
    Ok(BlobAlignment {
        fixed_blobs: fixed_blobs.to_vec(),
        moving_blobs: moving_blobs.to_vec(),
        correspondence: m,
        pairs,
        landmarks,
        fixed_landmarks: fl,
        moving_landmarks: ml,
        matrix: fit.matrix,
        offset: fit.offset,
        messages: fit.messages,
    })
}

/// `VersorRigid3DTransform` (3D) or `Rigid2DTransform` (2D) fit.
fn rigid_fit<const D: usize>(
    fixed: &[[f64; D]],
    moving: &[[f64; D]],
    notices: &mut Notices,
) -> Result<LandmarkFit<D>, ItkException> {
    match D {
        3 => {
            let f: Vec<[f64; 3]> = fixed
                .iter()
                .map(|p| std::array::from_fn(|k| p[k]))
                .collect();
            let m: Vec<[f64; 3]> = moving
                .iter()
                .map(|p| std::array::from_fn(|k| p[k]))
                .collect();
            let fit = landmark::versor_rigid_3d(&f, &m)?;
            Ok(LandmarkFit {
                matrix: std::array::from_fn(|r| std::array::from_fn(|c| fit.matrix[r][c])),
                offset: std::array::from_fn(|k| fit.offset[k]),
                messages: fit.messages,
            })
        }
        _ => {
            let f: Vec<[f64; 2]> = fixed
                .iter()
                .map(|p| std::array::from_fn(|k| p[k]))
                .collect();
            let m: Vec<[f64; 2]> = moving
                .iter()
                .map(|p| std::array::from_fn(|k| p[k]))
                .collect();
            let fit = landmark::rigid_2d(&f, &m, notices);
            Ok(LandmarkFit {
                matrix: std::array::from_fn(|r| std::array::from_fn(|c| fit.matrix[r][c])),
                offset: std::array::from_fn(|k| fit.offset[k]),
                messages: fit.messages,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;
    use larmorx_registration::Space;

    /// The 3D patch as ANTs builds it (from the instrumented copy of `antsAI`: 33,401 offsets,
    /// the float sum of the weights `0x1.f6b81ep+14`, the first weight `0x1.f00548p-16`).
    #[test]
    fn patch_as_ants_builds_it() {
        let (offsets, weights) = patch::<3>();
        assert_eq!(offsets.len(), 33_401);
        assert_eq!(offsets[0], [0, 0, -20]);
        assert_eq!(weights[0].to_bits(), 0x37f8_02a4);
        let (o2, w2) = patch::<2>();
        assert_eq!(o2.len(), w2.len());
        assert_eq!(o2.len(), 1_257);
    }

    fn blob(r: f64) -> Blob {
        // object_radius = √(D/2)·σ·√2 = √3·σ in 3D.
        Blob {
            center: vec![30, 30, 30],
            sigma: r / 3f64.sqrt(),
            value: 1.0,
        }
    }

    /// ANTS-111: once the positive correlations are used up, ANTs pairs blob 0 with blob 0
    /// again at every step; corrected, the pairing stops.
    #[test]
    fn pairing_after_the_correlations_run_out() {
        let fixed = vec![blob(3.0), blob(3.0), blob(3.0)];
        let moving = vec![blob(3.0), blob(3.0), blob(3.0)];
        let bright = [true, true, true];
        // Only one positive correlation, at (1, 2).
        let m = [0.0, 0.0, 0.0, 0.0, 0.0, 0.9, 0.0, 0.0, 0.0f32];
        let ants = pair_blobs(&m, 3, 3, &fixed, &moving, &bright, &bright, 3, false);
        assert_eq!(ants, vec![(1, 2), (0, 0), (0, 0)]);
        let fixed_pairs = pair_blobs(&m, 3, 3, &fixed, &moving, &bright, &bright, 3, true);
        assert_eq!(fixed_pairs, vec![(1, 2)]);
        // Radii further apart than 0.25 are not paired.
        let far = vec![blob(3.0), blob(3.3), blob(3.0)];
        let p = pair_blobs(&m, 3, 3, &far, &moving, &bright, &bright, 3, true);
        assert_eq!(p, vec![]);
    }

    /// ANTS-110: ANTs keeps pairs whose displacements have similar lengths; corrected, the
    /// pairs whose distances to the others are preserved.
    #[test]
    fn landmark_tests() {
        // Five pairs related by a rotation of 90° about z: every distance is preserved, but
        // the displacements have different lengths.
        let fixed: Vec<[f64; 3]> = vec![
            [10.0, 0.0, 0.0],
            [0.0, 20.0, 0.0],
            [-30.0, 0.0, 5.0],
            [0.0, -40.0, 0.0],
            [15.0, 15.0, 15.0],
        ];
        let moving: Vec<[f64; 3]> = fixed.iter().map(|p| [-p[1], p[0], p[2]]).collect();
        assert_eq!(landmark_pairs(&fixed, &moving, true), vec![0, 1, 2, 3, 4]);
        assert_eq!(landmark_pairs(&fixed, &moving, false), Vec::<usize>::new());
        // A pure translation: both keep every pair.
        let shifted: Vec<[f64; 3]> = fixed
            .iter()
            .map(|p| [p[0] + 3.0, p[1] - 1.0, p[2]])
            .collect();
        assert_eq!(landmark_pairs(&fixed, &shifted, true), vec![0, 1, 2, 3, 4]);
        assert_eq!(landmark_pairs(&fixed, &shifted, false), vec![0, 1, 2, 3, 4]);
    }

    fn phantom_2d(shift: (f64, f64)) -> Image<f32, 2> {
        let (nx, ny) = (128usize, 124usize);
        // Twelve blobs of distinct sizes and brightnesses, 25 voxels or more from the borders.
        let centres = [
            (30.0, 32.0, 2.0, 120.0),
            (52.0, 30.0, 3.0, 160.0),
            (78.0, 34.0, 2.5, 140.0),
            (98.0, 30.0, 3.5, 180.0),
            (34.0, 58.0, 3.0, 130.0),
            (60.0, 56.0, 2.0, 170.0),
            (84.0, 60.0, 3.5, 150.0),
            (100.0, 62.0, 2.5, 110.0),
            (30.0, 86.0, 3.5, 175.0),
            (56.0, 90.0, 2.5, 125.0),
            (80.0, 88.0, 2.0, 145.0),
            (98.0, 92.0, 3.0, 165.0),
        ];
        let mut data = vec![0.0f32; nx * ny];
        for y in 0..ny {
            for x in 0..nx {
                let mut v = 5.0f64;
                for &(cx, cy, s, a) in &centres {
                    let (dx, dy) = (x as f64 - cx - shift.0, y as f64 - cy - shift.1);
                    let r2 = (dx * dx + dy * dy) / (2.0 * s * s);
                    v += a / (1.0 + r2 + r2 * r2 / 2.0 + r2 * r2 * r2 / 6.0);
                }
                data[x + nx * y] = v as f32;
            }
        }
        let space = Space::new(
            [nx, ny],
            [0, 0],
            [1.0, 1.0],
            [0.0, 0.0],
            [[1.0, 0.0], [0.0, 1.0]],
        )
        .unwrap();
        Image::new(space, data)
    }

    /// A 2D phantom against its copy shifted by whole voxels: the pairs and landmarks recover
    /// the shift; the result does not depend on the thread count.
    #[test]
    fn aligns_a_shifted_phantom() {
        let fixed = phantom_2d((0.0, 0.0));
        let moving = phantom_2d((3.0, -2.0));
        for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
            let mut n1 = Notices::new(mode);
            let a = align(&fixed, &moving, 12, 12, LandmarkKind::Rigid, &mut n1, 1).unwrap();
            let mut n4 = Notices::new(mode);
            let b = align(&fixed, &moving, 12, 12, LandmarkKind::Rigid, &mut n4, 4).unwrap();
            assert_eq!(format!("{a:?}"), format!("{b:?}"));
            assert!(
                a.landmarks.len() >= 4,
                "{mode:?}: {:?} {:?}",
                a.pairs,
                a.fixed_blobs
            );
            assert!(
                (a.offset[0] - 3.0).abs() < 1e-9 && (a.offset[1] + 2.0).abs() < 1e-9,
                "{:?}",
                a.offset
            );
            assert!((a.matrix[0][0] - 1.0).abs() < 1e-12);
        }
    }

    /// ANTS-116: identical patches (the phantom against its copy shifted by whole voxels)
    /// correlate at 1 once compared in the orientation that matches best; ANTs' rotation can
    /// reflect or turn them. (Between different patches ANTs' reflection may correlate more
    /// than any rotation: a reflected match is no match for a rigid transform.)
    #[test]
    fn identical_patches_correlate_at_one() {
        let fixed = phantom_2d((0.0, 0.0));
        let moving = phantom_2d((3.0, -2.0));
        let mut notices = Notices::new(UpstreamDefects::Fix);
        let fb = detect(&fixed, 12, "fixed", &mut notices, 1).unwrap();
        let mb = detect(&moving, 12, "moving", &mut notices, 1).unwrap();
        let c = correspondence(&fixed, &moving, &fb, &mb, 2).unwrap();
        let mut identical = 0;
        for (i, f) in fb.iter().enumerate() {
            for (j, m) in mb.iter().enumerate() {
                let k = i * mb.len() + j;
                if m.center[0] == f.center[0] + 3 && m.center[1] == f.center[1] - 2 {
                    assert!(c.oriented[k] > 0.999, "{k}: {}", c.oriented[k]);
                    assert!(c.oriented[k] >= c.ants[k]);
                    identical += 1;
                }
            }
        }
        assert!(identical >= 4, "{identical}");
    }

    /// An image whose axis is shorter than 4 voxels: ITK's recursive Gaussian refuses it.
    #[test]
    fn thin_images_are_refused() {
        let space = Space::new(
            [50, 3],
            [0, 0],
            [1.0, 1.0],
            [0.0, 0.0],
            [[1.0, 0.0], [0.0, 1.0]],
        )
        .unwrap();
        let img = Image::new(space, vec![1.0f32; 150]);
        let mut n = Notices::new(UpstreamDefects::Fix);
        match align(&img, &img, 5, 5, LandmarkKind::Rigid, &mut n, 1) {
            Err(BlobFailure::Uncaught(e)) => {
                assert!(e.description.contains("less than 4"), "{e:?}")
            }
            other => panic!("{other:?}"),
        }
    }
}
