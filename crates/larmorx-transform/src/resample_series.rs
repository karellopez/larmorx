// SPDX-License-Identifier: Apache-2.0 AND MIT
//! fMRIPrep's one-shot resampler (`fmriprep/interfaces/resampling.py`: `resample_image`,
//! `resample_series`, `resample_vol`, fMRIPrep 26.0.0.dev at `21a490fb`, Apache-2.0; NOTICE),
//! fused and parallel.
//!
//! For every volume of a BOLD series, fMRIPrep maps the target grid to source voxel indices
//! once (`ref2vox`, a nitransforms chain ending in the source's RAS-to-voxel affine), then per
//! volume:
//!
//! 1. moves the coordinates with the head-motion affine (voxel to voxel), with nibabel's
//!    `apply_affine` (MIT): `pts @ R.T + t`, the product fused as numpy's BLAS does;
//! 2. adds the voxel-shift map `vsm = fmap_hz · readout` (float32) along the phase-encoding
//!    axis;
//! 3. interpolates with SciPy's `map_coordinates` (cubic, `grid-constant` by default, the
//!    volume prefiltered in float64);
//! 4. multiplies by the Jacobian `1 + np.gradient(vsm, axis=pe)` (float32) when asked.
//!
//! Here the coordinates are mapped once per run, the voxel-shift map and Jacobian once per
//! distinct phase-encoding setting, and each volume is prefiltered once and interpolated in
//! one pass, without temporary coordinate arrays. Every value is bit-identical to fMRIPrep's.
//! Volumes run in parallel (or, with fewer volumes than threads, the voxels of each volume),
//! and the result does not depend on the thread count.
//!
//! **PIPE-03, corrected by default** (D13, `docs/api/upstream-defects.md`). fMRIPrep takes
//! the gradient of the voxel-shift map along the *target* array axis whose index is the
//! source's phase-encoding axis, in target voxels. That is the derivative along the
//! phase-encoding direction only when the target grid's axes are the source's (the native
//! boldref); in T1w or standard space it is taken along another direction, or at another
//! scale. The Jacobian determinant of the corrected mapping `t ↦ s(t) + vsm(t)·e_pe`,
//! relative to the uncorrected one, is `1 + ∇_t vsm · ∂t/∂s_pe` (the matrix determinant
//! lemma), where `∂t/∂s_pe` is column `pe` of the inverse of `J = ∂s/∂t`, the Jacobian of the
//! target-to-source voxel map. larmorx multiplies by that. Where the target's axes are the
//! source's (`∂t/∂s_pe = e_pe`), it is fMRIPrep's float32 `1 + gradient` bit for bit.
//! [`UpstreamDefects::Reproduce`] gives fMRIPrep's everywhere. Head motion is left out of `J`,
//! as fMRIPrep leaves it out of its Jacobian.

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::linalg::{Mat3, Mat4, inverse3};
use larmorx_core::parallel;
use larmorx_interp::ndimage::{Mode, NdimageError, Output, Spline};
use rayon::prelude::*;

use crate::TransformError;
use crate::nitransforms::{self, Step};
use crate::openblas;

/// Output voxels per parallel task within one volume.
const CHUNK: usize = 4096;

/// The readout vector of one volume (`pe_info`): the source voxel axis the field shifts along
/// and the signed total readout time in seconds (`(0, 0.0)` without distortion correction).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeInfo {
    pub axis: usize,
    pub readout: f64,
}

impl Default for PeInfo {
    fn default() -> Self {
        PeInfo {
            axis: 0,
            readout: 0.0,
        }
    }
}

/// Interpolation settings (`ResampleSeries`' `order`, `mode`, `cval`, `prefilter`, `jacobian`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResampleOptions {
    pub order: u32,
    pub mode: Mode,
    pub cval: f64,
    pub prefilter: bool,
    /// Multiply by the Jacobian of the voxel-shift map, `1 + d(vsm)/d(pe)`.
    pub jacobian: bool,
    /// D13: correct (default) or reproduce fMRIPrep's Jacobian axis (PIPE-03) and SciPy's
    /// reflect boundary (SCI-03).
    pub upstream_defects: UpstreamDefects,
}

impl Default for ResampleOptions {
    /// `ResampleSeries`' defaults: cubic, `grid-constant`, `cval` 0, prefilter, Jacobian on
    /// (`resample_image`'s default; fMRIPrep sets it per run); defects corrected.
    fn default() -> Self {
        ResampleOptions {
            order: 3,
            mode: Mode::GridConstant,
            cval: 0.0,
            prefilter: true,
            jacobian: true,
            upstream_defects: UpstreamDefects::Fix,
        }
    }
}

/// Errors of the resampler.
#[derive(Debug, thiserror::Error)]
pub enum ResampleError {
    #[error(transparent)]
    Transform(#[from] TransformError),
    #[error(transparent)]
    Interpolation(#[from] NdimageError),
    #[error(transparent)]
    Threads(#[from] parallel::ThreadPoolError),
    #[error("{0}")]
    Invalid(String),
}

fn invalid(msg: impl Into<String>) -> ResampleError {
    ResampleError::Invalid(msg.into())
}

/// The voxel-shift map and Jacobian for one phase-encoding setting.
struct Distortion {
    pe: PeInfo,
    /// `fmap_hz * readout` in float32, per target voxel (Fortran order).
    vsm: Vec<f32>,
    /// The Jacobian factor per target voxel (float32), when the Jacobian is applied: the
    /// corrected one, or fMRIPrep's `1 + np.gradient(vsm, axis=pe)` (PIPE-03).
    jacobian: Option<Vec<f32>>,
    /// PIPE-03: how many factors differ from fMRIPrep's, the largest difference, and the
    /// number of voxels.
    pipe03: Option<(usize, f64, usize)>,
}

/// `∂t/∂s`, the inverse of the Jacobian `J = ∂s/∂t` of the map from target voxels to source
/// voxels (before head motion), for the corrected Jacobian of PIPE-03. Entries within 1e-9
/// of an integer are snapped to it, so that grids whose axes are the source's give exactly
/// `±1` and `0`.
enum InverseJacobian {
    /// The same everywhere: the steps are affines.
    Constant(Mat3),
    /// Per target voxel (Fortran order): a step is a displacement field.
    PerVoxel(Vec<[[f32; 3]; 3]>),
}

impl InverseJacobian {
    /// `∂t/∂s_pe` at target voxel `n`: column `pe` of the inverse.
    fn column(&self, n: usize, pe: usize) -> [f64; 3] {
        match self {
            InverseJacobian::Constant(m) => [m[0][pe], m[1][pe], m[2][pe]],
            InverseJacobian::PerVoxel(v) => {
                let m = &v[n];
                [m[0][pe], m[1][pe], m[2][pe]].map(f64::from)
            }
        }
    }
}

/// `v` rounded to the nearest integer when it lies within 1e-9 of it.
fn snap(v: f64) -> f64 {
    let r = v.round();
    if (v - r).abs() < 1e-9 { r } else { v }
}

/// The inverse of `j` with its entries snapped ([`snap`]); `None` if `j` is singular.
fn snapped_inverse(j: &Mat3) -> Option<Mat3> {
    let j = j.map(|row| row.map(snap));
    inverse3(&j).map(|m| m.map(|row| row.map(snap)))
}

/// A run's resampling plan: the source voxel coordinates of every target voxel, and the
/// field map. Build it once, then resample any number of volumes.
pub struct SeriesResampler {
    target_shape: [usize; 3],
    source_shape: [usize; 3],
    /// `ref2vox.map(ndcoords(target).astype('f4'))`, per target voxel (Fortran order).
    coords: Vec<[f64; 3]>,
    /// The field map in Hz on the target grid (Fortran order); zeros when absent.
    fmap: Option<Vec<f32>>,
    options: ResampleOptions,
    /// For the corrected Jacobian (PIPE-03), when a field map is applied with the Jacobian.
    inverse_jacobian: Option<InverseJacobian>,
}

impl SeriesResampler {
    /// Maps the target grid into the source: `ndcoords(target)` rounded to float32, then
    /// `steps` (the transforms from target to source world space, in nitransforms' order)
    /// followed by `ras2vox` (the inverse of the source affine).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target_shape: [usize; 3],
        target_affine: &Mat4,
        steps: &[Step],
        ras2vox: &Mat4,
        source_shape: [usize; 3],
        fmap_hz: Option<Vec<f32>>,
        options: ResampleOptions,
        n_threads: usize,
    ) -> Result<Self, ResampleError> {
        if options.order > 5 {
            return Err(NdimageError::Order(options.order).into());
        }
        let n: usize = target_shape.iter().product();
        if let Some(f) = &fmap_hz
            && f.len() != n
        {
            return Err(invalid(format!(
                "the field map has {} values, the target grid {n} ({target_shape:?})",
                f.len()
            )));
        }
        let needs_jacobian = options.jacobian && fmap_hz.is_some();
        let (coords, inverse_jacobian) =
            parallel::with_threads(n_threads, || -> Result<_, ResampleError> {
                let mut coords = nitransforms::ndcoords(target_shape, target_affine);
                for p in &mut coords {
                    *p = nitransforms::to_f32(*p);
                }
                nitransforms::map_points(steps, &mut coords)?;
                nitransforms::affine_map(ras2vox, &mut coords);
                let inverse = if needs_jacobian {
                    Some(chain_inverse_jacobian(
                        target_shape,
                        target_affine,
                        steps,
                        ras2vox,
                    )?)
                } else {
                    None
                };
                Ok((coords, inverse))
            })??;
        Ok(SeriesResampler {
            target_shape,
            source_shape,
            coords,
            fmap: fmap_hz,
            options,
            inverse_jacobian,
        })
    }

    /// A plan from source voxel coordinates computed elsewhere (`coords[n]` for target voxel
    /// `n`, Fortran order). The corrected Jacobian (PIPE-03) then takes `∂s/∂t` from the
    /// coordinates' central differences (numpy's `gradient` of each coordinate along each
    /// target axis).
    pub fn from_coordinates(
        target_shape: [usize; 3],
        coords: Vec<[f64; 3]>,
        source_shape: [usize; 3],
        fmap_hz: Option<Vec<f32>>,
        options: ResampleOptions,
    ) -> Result<Self, ResampleError> {
        let n: usize = target_shape.iter().product();
        if coords.len() != n || fmap_hz.as_ref().is_some_and(|f| f.len() != n) {
            return Err(invalid(
                "coordinates and field map must cover the target grid",
            ));
        }
        let inverse_jacobian = (options.jacobian && fmap_hz.is_some())
            .then(|| coordinates_inverse_jacobian(&coords, target_shape));
        Ok(SeriesResampler {
            target_shape,
            source_shape,
            coords,
            fmap: fmap_hz,
            options,
            inverse_jacobian,
        })
    }

    pub fn target_shape(&self) -> [usize; 3] {
        self.target_shape
    }

    /// The source voxel coordinates of every target voxel, before head motion and
    /// distortion (Fortran order).
    pub fn coordinates(&self) -> &[[f64; 3]] {
        &self.coords
    }

    fn distortion(&self, pe: PeInfo) -> Result<Distortion, ResampleError> {
        if pe.axis > 2 {
            return Err(invalid(format!(
                "phase-encoding axis {} is not 0, 1 or 2",
                pe.axis
            )));
        }
        let n = self.coords.len();
        // vsm = fmap_hz * pe_info[1]: float32 times a Python float, computed in float32.
        let ro = pe.readout as f32;
        let vsm: Vec<f32> = match &self.fmap {
            Some(f) => f.iter().map(|&v| v * ro).collect(),
            None => vec![0.0f32 * ro; n],
        };
        if !self.options.jacobian {
            return Ok(Distortion {
                pe,
                vsm,
                jacobian: None,
                pipe03: None,
            });
        }
        let fix = self.options.upstream_defects.fix();
        let Some(inverse) = &self.inverse_jacobian else {
            // No field map: vsm is 0, and both Jacobians are 1 (fMRIPrep's still needs two
            // voxels along the axis).
            return Ok(Distortion {
                pe,
                jacobian: Some(jacobian(&vsm, self.target_shape, pe.axis)?),
                vsm,
                pipe03: None,
            });
        };
        // fMRIPrep's gradient fails with fewer than two voxels along the axis; the corrected
        // one leaves such an axis out.
        let fmriprep = jacobian(&vsm, self.target_shape, pe.axis);
        if !fix {
            let fmriprep = fmriprep?;
            let (changed, max) =
                compare_jacobians(&vsm, self.target_shape, pe.axis, inverse, &fmriprep);
            return Ok(Distortion {
                pe,
                vsm,
                jacobian: Some(fmriprep),
                pipe03: Some((changed, max, n)),
            });
        }
        let corrected = corrected_jacobian(&vsm, self.target_shape, pe.axis, inverse);
        let (changed, max) = match &fmriprep {
            Ok(f) => {
                let mut changed = 0usize;
                let mut max = 0f64;
                for (a, b) in corrected.iter().zip(f) {
                    if a.to_bits() != b.to_bits() {
                        changed += 1;
                        max = max.max((f64::from(*a) - f64::from(*b)).abs());
                    }
                }
                (changed, max)
            }
            Err(_) => (n, f64::INFINITY),
        };
        Ok(Distortion {
            pe,
            vsm,
            jacobian: Some(corrected),
            pipe03: Some((changed, max, n)),
        })
    }

    /// Resamples one 3D volume (`data`, Fortran order, the source shape) into `out` (the
    /// target grid, Fortran order): `resample_vol`. `hmc` is the volume's voxel-to-voxel
    /// head-motion affine. `parallel` spreads the voxels over the current rayon pool. Returns
    /// the upstream defects that changed (or would have changed) the result.
    pub fn resample_volume<O: Jacobian>(
        &self,
        data: &[f32],
        hmc: Option<&Mat4>,
        pe: PeInfo,
        out: &mut [O],
        parallel: bool,
    ) -> Result<Vec<Notice>, ResampleError> {
        let d = self.distortion(pe)?;
        let sci03 = self.volume(data, hmc, &d, out, parallel)?;
        Ok(self.notices(std::slice::from_ref(&d), sci03))
    }

    /// The notices of a resampling: PIPE-03 for the Jacobians, SCI-03 for the prefilter.
    fn notices(&self, distortions: &[Distortion], sci03: Option<Notice>) -> Vec<Notice> {
        let mode = self.options.upstream_defects;
        let (mut changed, mut max, mut n) = (0usize, 0f64, 0usize);
        for d in distortions {
            if let Some((c, m, len)) = d.pipe03 {
                changed += c;
                max = max.max(m);
                n += len;
            }
        }
        let mut notices = Vec::new();
        if changed > 0 {
            let axes = distortions
                .iter()
                .map(|d| d.pe.axis.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let size = if max.is_finite() {
                format!("by up to {max:.3e}")
            } else {
                "where fMRIPrep's gradient fails (fewer than 2 voxels along the axis)".to_owned()
            };
            let detail = match mode {
                UpstreamDefects::Fix => format!(
                    "the Jacobian is taken along the phase-encoding direction mapped into the \
                     target grid; fMRIPrep's (along target axis {axes}, in target voxels) \
                     differs in {changed} of {n} voxels, {size}"
                ),
                UpstreamDefects::Reproduce => format!(
                    "the Jacobian is taken along target axis {axes} in target voxels, as \
                     fMRIPrep does; along the phase-encoding direction it differs in {changed} \
                     of {n} voxels, {size}"
                ),
            };
            notices.push(Notice::new(&defects::PIPE_03, mode, detail));
        }
        notices.extend(sci03);
        notices
    }

    /// Resamples one volume; returns the SCI-03 notice of its prefilter, if any.
    fn volume<O: Output + Jacobian>(
        &self,
        data: &[f32],
        hmc: Option<&Mat4>,
        d: &Distortion,
        out: &mut [O],
        parallel: bool,
    ) -> Result<Option<Notice>, ResampleError> {
        let n = self.coords.len();
        if out.len() != n {
            return Err(invalid("the output does not match the target grid"));
        }
        let o = &self.options;
        let spline = Spline::new(
            data,
            self.source_shape,
            o.order,
            o.mode,
            o.cval,
            o.prefilter,
            parallel,
            o.upstream_defects,
        )?;
        let axis = d.pe.axis;
        let single = n == 1;
        let run = |start: usize, chunk: &mut [O]| {
            // Small blocks: the moved coordinates and the interpolated doubles stay in cache.
            const BLOCK: usize = 256;
            let mut pts = [[0.0f64; 3]; BLOCK];
            let mut vals = [0.0f64; BLOCK];
            for (b, block) in chunk.chunks_mut(BLOCK).enumerate() {
                let s = start + b * BLOCK;
                let e = s + block.len();
                let pts = &mut pts[..block.len()];
                let vals = &mut vals[..block.len()];
                for ((p, c), v) in pts.iter_mut().zip(&self.coords[s..e]).zip(&d.vsm[s..e]) {
                    *p = match hmc {
                        Some(h) if single => apply_affine_single(h, *c),
                        Some(h) => apply_affine(h, *c),
                        None => *c,
                    };
                    p[axis] += f64::from(*v);
                }
                spline.sample_batch(pts, vals);
                for (o, &t) in block.iter_mut().zip(vals.iter()) {
                    *o = O::from_interp(t);
                }
                if let Some(j) = &d.jacobian {
                    for (o, &j) in block.iter_mut().zip(&j[s..e]) {
                        *o = o.times(j);
                    }
                }
            }
        };
        if parallel {
            out.par_chunks_mut(CHUNK)
                .enumerate()
                .for_each(|(c, chunk)| run(c * CHUNK, chunk));
        } else {
            run(0, out);
        }
        Ok(spline.notices().into_iter().next())
    }

    /// Resamples every volume of `source` (Fortran order: the source shape, then `n_volumes`)
    /// into `out` (the target grid, then the volumes): `resample_series`. `hmc` holds one
    /// voxel-to-voxel affine per volume (or none), `pe` one readout vector per volume (or one
    /// for all). Returns the upstream defects that changed (or would have changed) the result.
    pub fn resample_series<O: Output + Jacobian>(
        &self,
        source: &[f32],
        n_volumes: usize,
        hmc: Option<&[Mat4]>,
        pe: &[PeInfo],
        out: &mut [O],
        n_threads: usize,
    ) -> Result<Vec<Notice>, ResampleError> {
        let n_src: usize = self.source_shape.iter().product();
        let n_out = self.coords.len();
        if source.len() != n_src * n_volumes {
            return Err(invalid(format!(
                "the source has {} values, not {n_volumes} volumes of {:?}",
                source.len(),
                self.source_shape
            )));
        }
        if out.len() != n_out * n_volumes {
            return Err(invalid(
                "the output does not match the target grid and volumes",
            ));
        }
        if let Some(h) = hmc
            && h.len() < n_volumes
        {
            return Err(invalid(format!(
                "{} head-motion transforms for {n_volumes} volumes",
                h.len()
            )));
        }
        if pe.is_empty() || (pe.len() != 1 && pe.len() < n_volumes) {
            return Err(invalid(format!(
                "{} readout vectors for {n_volumes} volumes",
                pe.len()
            )));
        }
        let pe_of = |t: usize| if pe.len() == 1 { pe[0] } else { pe[t] };
        // One voxel-shift map and Jacobian per distinct readout vector.
        let mut distortions: Vec<Distortion> = Vec::new();
        let mut which = Vec::with_capacity(n_volumes);
        for t in 0..n_volumes {
            let p = pe_of(t);
            let i = match distortions
                .iter()
                .position(|d| d.pe.axis == p.axis && d.pe.readout.to_bits() == p.readout.to_bits())
            {
                Some(i) => i,
                None => {
                    distortions.push(self.distortion(p)?);
                    distortions.len() - 1
                }
            };
            which.push(i);
        }
        if n_volumes == 0 {
            return Ok(self.notices(&distortions, None));
        }
        let threads = parallel::resolve_threads(n_threads);
        let sci03 =
            parallel::with_threads(threads, || -> Result<Option<Notice>, ResampleError> {
                if n_volumes >= threads && threads > 1 {
                    // Whole volumes per task: at most `threads` prefiltered volumes in memory.
                    let notices: Vec<Option<Notice>> = out
                        .par_chunks_mut(n_out)
                        .enumerate()
                        .with_max_len(1)
                        .map(|(t, vol_out)| {
                            self.volume(
                                &source[t * n_src..(t + 1) * n_src],
                                hmc.map(|h| &h[t]),
                                &distortions[which[t]],
                                vol_out,
                                false,
                            )
                        })
                        .collect::<Result<_, _>>()?;
                    Ok(notices.into_iter().flatten().next())
                } else {
                    let mut first = None;
                    for (t, vol_out) in out.chunks_mut(n_out).enumerate() {
                        let notice = self.volume(
                            &source[t * n_src..(t + 1) * n_src],
                            hmc.map(|h| &h[t]),
                            &distortions[which[t]],
                            vol_out,
                            threads > 1,
                        )?;
                        first = first.or(notice);
                    }
                    Ok(first)
                }
            })??;
        Ok(self.notices(&distortions, sci03))
    }
}

/// [`InverseJacobian`] of the chain `ras2vox ∘ steps ∘ target_affine` (target voxels to source
/// voxels, in double precision): constant for a chain of affines, else per target voxel, by
/// central differences (±0.5 voxel) of the chain without nitransforms' float32 roundings
/// ([`nitransforms::ExactChain`]).
fn chain_inverse_jacobian(
    shape: [usize; 3],
    target_affine: &Mat4,
    steps: &[Step],
    ras2vox: &Mat4,
) -> Result<InverseJacobian, ResampleError> {
    let lin = |m: &Mat4| -> Mat3 { std::array::from_fn(|i| std::array::from_fn(|j| m[i][j])) };
    let identity: Mat3 =
        std::array::from_fn(|i| std::array::from_fn(|j| f64::from(u8::from(i == j))));
    if steps.iter().all(|s| matches!(s, Step::Affine(_))) {
        // J = ras2vox · M_n ⋯ M_1 · target_affine (linear parts; the steps apply in list order).
        let mut j = lin(target_affine);
        for s in steps {
            if let Step::Affine(m) = s {
                j = larmorx_core::linalg::mul3(&lin(m), &j);
            }
        }
        j = larmorx_core::linalg::mul3(&lin(ras2vox), &j);
        return Ok(InverseJacobian::Constant(
            snapped_inverse(&j).unwrap_or(identity),
        ));
    }
    let chain = nitransforms::ExactChain::new(steps)?;
    let [nx, ny, nz] = shape;
    let n = nx * ny * nz;
    let map = |t: [f64; 3]| -> [f64; 3] {
        let world = nitransforms::exact_affine(target_affine, t);
        nitransforms::exact_affine(ras2vox, chain.map(world))
    };
    let mut out = vec![[[0f32; 3]; 3]; n];
    out.par_chunks_mut(nx.max(1))
        .enumerate()
        .for_each(|(row, chunk)| {
            let (y, z) = (row % ny, row / ny);
            for (x, o) in chunk.iter_mut().enumerate() {
                let t = [x as f64, y as f64, z as f64];
                let mut j = [[0f64; 3]; 3];
                for k in 0..3 {
                    let (mut a, mut b) = (t, t);
                    a[k] += 0.5;
                    b[k] -= 0.5;
                    let (sa, sb) = (map(a), map(b));
                    for i in 0..3 {
                        j[i][k] = sa[i] - sb[i];
                    }
                }
                let inv = snapped_inverse(&j).unwrap_or(identity);
                *o = inv.map(|r| r.map(|v| v as f32));
            }
        });
    Ok(InverseJacobian::PerVoxel(out))
}

/// [`InverseJacobian`] from coordinates computed elsewhere: `J` from numpy-style gradients of
/// each coordinate along each target axis (unit spacing, first-order edges; 0 along an axis
/// of one voxel).
fn coordinates_inverse_jacobian(coords: &[[f64; 3]], shape: [usize; 3]) -> InverseJacobian {
    let identity: Mat3 =
        std::array::from_fn(|i| std::array::from_fn(|j| f64::from(u8::from(i == j))));
    let strides = [1, shape[0], shape[0] * shape[1]];
    let out = (0..coords.len())
        .into_par_iter()
        .map(|n| {
            let idx = [n % shape[0], (n / shape[0]) % shape[1], n / strides[2]];
            let mut j = [[0f64; 3]; 3];
            for k in 0..3 {
                let (len, s) = (shape[k], strides[k]);
                if len < 2 {
                    continue;
                }
                let (lo, hi, h) = if idx[k] == 0 {
                    (n, n + s, 1.0)
                } else if idx[k] == len - 1 {
                    (n - s, n, 1.0)
                } else {
                    (n - s, n + s, 2.0)
                };
                for i in 0..3 {
                    j[i][k] = (coords[hi][i] - coords[lo][i]) / h;
                }
            }
            snapped_inverse(&j)
                .unwrap_or(identity)
                .map(|r| r.map(|v| v as f32))
        })
        .collect();
    InverseJacobian::PerVoxel(out)
}

/// numpy's `gradient(vsm, axis=k)` at voxel `n` (float32, unit spacing, first-order edges), or
/// 0 along an axis of one voxel.
#[inline]
fn gradient_at(vsm: &[f32], shape: [usize; 3], k: usize, n: usize) -> f32 {
    let len = shape[k];
    if len < 2 {
        return 0.0;
    }
    let stride: usize = shape[..k].iter().product();
    let t = (n / stride) % len;
    if t == 0 {
        (vsm[n + stride] - vsm[n]) / 1.0
    } else if t == len - 1 {
        (vsm[n] - vsm[n - stride]) / 1.0
    } else {
        (vsm[n + stride] - vsm[n - stride]) / 2.0
    }
}

/// The corrected Jacobian factor at voxel `n` for direction `d = ∂t/∂s_pe`: `1 + Σ d_k·g_k`.
/// When `d` is a signed unit vector `±e_k`, it is the float32 `1 ± g_k`, numpy's arithmetic
/// (so a target whose axes are the source's gives fMRIPrep's value); else the sum is taken
/// in double and rounded to float32.
#[inline]
fn corrected_factor(vsm: &[f32], shape: [usize; 3], n: usize, d: [f64; 3]) -> f32 {
    let nonzero = d.iter().filter(|&&v| v != 0.0).count();
    if nonzero == 1 {
        let k = d.iter().position(|&v| v != 0.0).unwrap_or(0);
        if d[k] == 1.0 {
            return 1.0 + gradient_at(vsm, shape, k, n);
        }
        if d[k] == -1.0 {
            return 1.0 - gradient_at(vsm, shape, k, n);
        }
    }
    let mut sum = 1.0f64;
    for (k, &dk) in d.iter().enumerate() {
        if dk != 0.0 {
            sum += dk * f64::from(gradient_at(vsm, shape, k, n));
        }
    }
    sum as f32
}

/// The corrected Jacobian factors (PIPE-03), per target voxel.
fn corrected_jacobian(
    vsm: &[f32],
    shape: [usize; 3],
    pe: usize,
    inverse: &InverseJacobian,
) -> Vec<f32> {
    (0..vsm.len())
        .into_par_iter()
        .with_min_len(4096)
        .map(|n| corrected_factor(vsm, shape, n, inverse.column(n, pe)))
        .collect()
}

/// How many of fMRIPrep's factors the corrected Jacobian would change, and by how much at
/// most (the reproduced mode's PIPE-03 notice).
fn compare_jacobians(
    vsm: &[f32],
    shape: [usize; 3],
    pe: usize,
    inverse: &InverseJacobian,
    fmriprep: &[f32],
) -> (usize, f64) {
    (0..vsm.len())
        .into_par_iter()
        .with_min_len(4096)
        .map(|n| {
            let c = corrected_factor(vsm, shape, n, inverse.column(n, pe));
            if c.to_bits() == fmriprep[n].to_bits() {
                (0usize, 0f64)
            } else {
                (1, (f64::from(c) - f64::from(fmriprep[n])).abs())
            }
        })
        .reduce(|| (0, 0.0), |a, b| (a.0 + b.0, a.1.max(b.1)))
}

/// nibabel's `apply_affine(h, p)` for one point, as numpy evaluates `pts @ rzs.T + trans`:
/// the 3×3 product as OpenBLAS's Haswell `dgemm` computes it ([`openblas::rows3`]), then the
/// translation.
#[inline(always)]
pub fn apply_affine(h: &Mat4, p: [f64; 3]) -> [f64; 3] {
    let q = openblas::rows3(h, p);
    std::array::from_fn(|r| q[r] + h[r][3])
}

/// [`apply_affine`] when the target grid has a single voxel: numpy's `matmul` then uses
/// `dgemv` ([`openblas::rows3_single`]).
#[inline(always)]
pub fn apply_affine_single(h: &Mat4, p: [f64; 3]) -> [f64; 3] {
    let q = openblas::rows3_single(h, p);
    std::array::from_fn(|r| q[r] + h[r][3])
}

/// `1 + np.gradient(vsm, axis=axis)` in float32 (unit spacing, first-order edges).
fn jacobian(vsm: &[f32], shape: [usize; 3], axis: usize) -> Result<Vec<f32>, ResampleError> {
    let len = shape[axis];
    if len < 2 {
        return Err(invalid(
            "Shape of array too small to calculate a numerical gradient, at least \
             (edge_order + 1) elements are required.",
        ));
    }
    let stride: usize = shape[..axis].iter().product();
    let mut out = vec![0.0f32; vsm.len()];
    for (n, o) in out.iter_mut().enumerate() {
        let t = (n / stride) % len;
        let g = if t == 0 {
            (vsm[n + stride] - vsm[n]) / 1.0
        } else if t == len - 1 {
            (vsm[n] - vsm[n - stride]) / 1.0
        } else {
            (vsm[n + stride] - vsm[n - stride]) / 2.0
        };
        *o = 1.0 + g;
    }
    Ok(out)
}

/// Output types: `result *= jacobian` in numpy's arithmetic for the output dtype.
pub trait Jacobian: Output {
    fn times(self, j: f32) -> Self;
}

impl Jacobian for f32 {
    #[inline(always)]
    fn times(self, j: f32) -> Self {
        self * j
    }
}

impl Jacobian for f64 {
    #[inline(always)]
    fn times(self, j: f32) -> Self {
        self * f64::from(j)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eye() -> Mat4 {
        let mut m = [[0.0; 4]; 4];
        for (i, row) in m.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        m
    }

    fn series(shape: [usize; 3], n: usize) -> Vec<f32> {
        let len: usize = shape.iter().product();
        (0..len * n)
            .map(|i| ((i * 37 % 101) as f32) * 0.5 + (i / len) as f32)
            .collect()
    }

    /// The identity resampling of a grid onto itself returns the data (cubic interpolation at
    /// the samples, to rounding) and the result does not depend on the thread count.
    #[test]
    fn identity_and_thread_invariance() {
        let shape = [7, 6, 5];
        let n_vol = 5;
        let src = series(shape, n_vol);
        let fmap: Vec<f32> = (0..210).map(|i| (i % 13) as f32 - 6.0).collect();
        let plan = SeriesResampler::new(
            shape,
            &eye(),
            &[],
            &eye(),
            shape,
            Some(fmap),
            ResampleOptions::default(),
            1,
        )
        .unwrap();
        let mut hmc = vec![eye(); n_vol];
        hmc[2][0][3] = 0.3;
        hmc[3][1][3] = -0.7;
        let pe = [PeInfo {
            axis: 1,
            readout: -0.031,
        }];
        let mut results = Vec::new();
        for threads in [1, 2, 3, 8] {
            let mut out = vec![0.0f32; src.len()];
            plan.resample_series(&src, n_vol, Some(&hmc), &pe, &mut out, threads)
                .unwrap();
            results.push(out);
        }
        for r in &results[1..] {
            assert!(
                r.iter()
                    .zip(&results[0])
                    .all(|(a, b)| a.to_bits() == b.to_bits())
            );
        }
        // Without distortion or motion, the identity returns the samples.
        let plan = SeriesResampler::new(
            shape,
            &eye(),
            &[],
            &eye(),
            shape,
            None,
            ResampleOptions::default(),
            1,
        )
        .unwrap();
        let mut out = vec![0.0f32; src.len()];
        plan.resample_series(&src, n_vol, None, &[PeInfo::default()], &mut out, 4)
            .unwrap();
        for (a, b) in out.iter().zip(&src) {
            assert!((a - b).abs() <= 1e-4 * b.abs().max(1.0), "{a} vs {b}");
        }
    }

    #[test]
    fn jacobian_is_numpy_gradient() {
        let shape = [3, 2, 1];
        let vsm = [1.0f32, 4.0, 9.0, 2.0, 2.5, -1.0];
        let j = jacobian(&vsm, shape, 0).unwrap();
        assert_eq!(j, vec![4.0, 5.0, 6.0, 1.5, -0.5, -2.5]);
        let j = jacobian(&vsm, shape, 1).unwrap();
        assert_eq!(j, vec![2.0, -0.5, -9.0, 2.0, -0.5, -9.0]);
        assert!(jacobian(&vsm, shape, 2).is_err());
    }

    fn options(defects: UpstreamDefects) -> ResampleOptions {
        ResampleOptions {
            order: 1,
            mode: Mode::Nearest,
            upstream_defects: defects,
            ..ResampleOptions::default()
        }
    }

    /// A field map in Hz on a target grid, varying along every axis.
    fn field(shape: [usize; 3]) -> Vec<f32> {
        let n: usize = shape.iter().product();
        (0..n)
            .map(|i| {
                let (x, y, z) = (
                    i % shape[0],
                    (i / shape[0]) % shape[1],
                    i / (shape[0] * shape[1]),
                );
                (x * x) as f32 * 0.5 + (y * 3) as f32 - (z * y) as f32 * 0.25 + (i % 5) as f32
            })
            .collect()
    }

    /// Resamples with the Jacobian only (a constant source, so the output is the Jacobian
    /// factor): returns the output and the notices.
    fn jacobian_of(
        target_shape: [usize; 3],
        target_affine: &Mat4,
        steps: &[Step],
        defects: UpstreamDefects,
    ) -> (Vec<f32>, Vec<Notice>) {
        let source_shape = [6, 5, 4];
        let src = vec![1.0f32; 120];
        let fmap = field(target_shape);
        let plan = SeriesResampler::new(
            target_shape,
            target_affine,
            steps,
            &eye(),
            source_shape,
            Some(fmap),
            options(defects),
            2,
        )
        .unwrap();
        let mut out = vec![0f32; target_shape.iter().product()];
        let pe = [PeInfo {
            axis: 1,
            readout: 0.01,
        }];
        let notices = plan
            .resample_series(&src, 1, None, &pe, &mut out, 2)
            .unwrap();
        (out, notices)
    }

    /// PIPE-03: on the source's own grid the corrected Jacobian is fMRIPrep's, bit for bit, and
    /// nothing is said.
    #[test]
    fn native_grid_jacobian_is_fmriprep_s() {
        let fixed = jacobian_of([6, 5, 4], &eye(), &[], UpstreamDefects::Fix);
        let reproduced = jacobian_of([6, 5, 4], &eye(), &[], UpstreamDefects::Reproduce);
        assert_eq!(fixed, reproduced);
        assert!(fixed.1.is_empty());
        // An affine step and its inverse in the chain change nothing either.
        let mut m = eye();
        m[0][1] = 0.3;
        m[2][3] = 4.0;
        let inv = larmorx_core::linalg::inverse4(&m).unwrap();
        let steps = [Step::Affine(m), Step::Affine(inv)];
        let fixed = jacobian_of([6, 5, 4], &eye(), &steps, UpstreamDefects::Fix);
        assert!(fixed.1.is_empty());
    }

    /// PIPE-03: a target whose axes 0 and 1 are the source's 1 and 0. fMRIPrep differentiates
    /// along target axis 1 (the source's x); the corrected Jacobian along target axis 0 (the
    /// source's phase-encoding axis y).
    #[test]
    fn permuted_target_jacobian() {
        let shape = [5, 6, 4];
        let mut swap = [[0.0; 4]; 4];
        (swap[0][1], swap[1][0], swap[2][2], swap[3][3]) = (1.0, 1.0, 1.0, 1.0);
        let (fixed, nf) = jacobian_of(shape, &swap, &[], UpstreamDefects::Fix);
        let (fmriprep, nr) = jacobian_of(shape, &swap, &[], UpstreamDefects::Reproduce);
        let vsm: Vec<f32> = field(shape).iter().map(|v| v * 0.01f32).collect();
        assert_eq!(fmriprep, jacobian(&vsm, shape, 1).unwrap());
        assert_eq!(fixed, jacobian(&vsm, shape, 0).unwrap());
        for notices in [&nf, &nr] {
            assert_eq!(notices.len(), 1);
            assert_eq!(notices[0].defect.id, "PIPE-03");
        }
        assert_eq!(nf[0].mode, UpstreamDefects::Fix);
        assert_eq!(nr[0].mode, UpstreamDefects::Reproduce);
        // A reversed axis takes the derivative with the other sign.
        let mut flip = eye();
        (flip[1][1], flip[1][3]) = (-1.0, 4.0);
        let (fixed, _) = jacobian_of([6, 5, 4], &flip, &[], UpstreamDefects::Fix);
        let vsm: Vec<f32> = field([6, 5, 4]).iter().map(|v| v * 0.01f32).collect();
        let expected: Vec<f32> = (0..120)
            .map(|n| 1.0 - gradient_at(&vsm, [6, 5, 4], 1, n))
            .collect();
        assert_eq!(fixed, expected);
    }

    /// PIPE-03: a target with half the voxel size along the phase-encoding axis: the
    /// derivative per source voxel is twice the derivative per target voxel.
    #[test]
    fn finer_target_jacobian() {
        let shape = [6, 9, 4];
        let mut half = eye();
        half[1][1] = 0.5;
        let (fixed, notices) = jacobian_of(shape, &half, &[], UpstreamDefects::Fix);
        let vsm: Vec<f32> = field(shape).iter().map(|v| v * 0.01f32).collect();
        for (n, &f) in fixed.iter().enumerate() {
            let g = f64::from(gradient_at(&vsm, shape, 1, n));
            assert_eq!(f, (1.0 + 2.0 * g) as f32);
        }
        assert_eq!(notices.len(), 1);
        // The per-voxel path (a displacement field that is zero) gives the same factors. The
        // field's splines bend near its edges (mirror boundary), so it extends 25 nodes beyond
        // the target, where the bend is below 1e-12.
        let n = 56;
        let mut corner = eye();
        for (k, row) in corner.iter_mut().take(3).enumerate() {
            row[3] = -25.0;
            row[k] = 1.0;
        }
        let inverse = larmorx_core::linalg::inverse4(&corner).unwrap();
        let zero = nitransforms::DenseField::from_deltas(
            [n, n, n],
            &corner,
            &inverse,
            [
                vec![0.0; n * n * n],
                vec![0.0; n * n * n],
                vec![0.0; n * n * n],
            ],
        )
        .unwrap();
        let (per_voxel, _) = jacobian_of(
            shape,
            &half,
            &[Step::DenseField(zero)],
            UpstreamDefects::Fix,
        );
        for (a, b) in per_voxel.iter().zip(&fixed) {
            assert!((a - b).abs() <= 1e-6, "{a} {b}");
        }
    }

    /// PIPE-03 from coordinates computed elsewhere: `J` from their differences.
    #[test]
    fn jacobian_from_coordinates() {
        let shape = [5, 6, 4];
        let coords: Vec<[f64; 3]> = (0..120)
            .map(|i| {
                let (x, y, z) = (i % 5, (i / 5) % 6, i / 30);
                [y as f64, x as f64, z as f64]
            })
            .collect();
        let fmap = field(shape);
        let plan = SeriesResampler::from_coordinates(
            shape,
            coords,
            [6, 5, 4],
            Some(fmap.clone()),
            options(UpstreamDefects::Fix),
        )
        .unwrap();
        let mut out = vec![0f32; 120];
        let pe = PeInfo {
            axis: 1,
            readout: 0.01,
        };
        let notices = plan
            .resample_volume(&[1.0f32; 120], None, pe, &mut out, false)
            .unwrap();
        let vsm: Vec<f32> = fmap.iter().map(|v| v * 0.01f32).collect();
        assert_eq!(out, jacobian(&vsm, shape, 0).unwrap());
        assert_eq!(notices[0].defect.id, "PIPE-03");
    }
}
