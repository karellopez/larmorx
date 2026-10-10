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

use larmorx_core::linalg::Mat4;
use larmorx_core::parallel;
use larmorx_interp::ndimage::{Mode, NdimageError, Output, Spline};
use rayon::prelude::*;

use crate::TransformError;
use crate::nitransforms::{self, Step};

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
    /// Multiply by `1 + d(vsm)/d(pe)`.
    pub jacobian: bool,
}

impl Default for ResampleOptions {
    /// `ResampleSeries`' defaults: cubic, `grid-constant`, `cval` 0, prefilter, Jacobian on
    /// (`resample_image`'s default; fMRIPrep sets it per run).
    fn default() -> Self {
        ResampleOptions {
            order: 3,
            mode: Mode::GridConstant,
            cval: 0.0,
            prefilter: true,
            jacobian: true,
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
    /// `1 + np.gradient(vsm, axis=pe)` in float32, when the Jacobian is applied.
    jacobian: Option<Vec<f32>>,
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
        let coords = parallel::with_threads(n_threads, || -> Result<_, ResampleError> {
            let mut coords = nitransforms::ndcoords(target_shape, target_affine);
            for p in &mut coords {
                *p = nitransforms::to_f32(*p);
            }
            nitransforms::map_points(steps, &mut coords)?;
            nitransforms::affine_map(ras2vox, &mut coords);
            Ok(coords)
        })??;
        Ok(SeriesResampler {
            target_shape,
            source_shape,
            coords,
            fmap: fmap_hz,
            options,
        })
    }

    /// A plan from source voxel coordinates computed elsewhere (`coords[n]` for target voxel
    /// `n`, Fortran order).
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
        Ok(SeriesResampler {
            target_shape,
            source_shape,
            coords,
            fmap: fmap_hz,
            options,
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
        let jacobian = if self.options.jacobian {
            Some(jacobian(&vsm, self.target_shape, pe.axis)?)
        } else {
            None
        };
        Ok(Distortion { pe, vsm, jacobian })
    }

    /// Resamples one 3D volume (`data`, Fortran order, the source shape) into `out` (the
    /// target grid, Fortran order): `resample_vol`. `hmc` is the volume's voxel-to-voxel
    /// head-motion affine. `parallel` spreads the voxels over the current rayon pool.
    pub fn resample_volume<O: Jacobian>(
        &self,
        data: &[f32],
        hmc: Option<&Mat4>,
        pe: PeInfo,
        out: &mut [O],
        parallel: bool,
    ) -> Result<(), ResampleError> {
        let d = self.distortion(pe)?;
        self.volume(data, hmc, &d, out, parallel)
    }

    fn volume<O: Output + Jacobian>(
        &self,
        data: &[f32],
        hmc: Option<&Mat4>,
        d: &Distortion,
        out: &mut [O],
        parallel: bool,
    ) -> Result<(), ResampleError> {
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
        )?;
        let axis = d.pe.axis;
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
        Ok(())
    }

    /// Resamples every volume of `source` (Fortran order: the source shape, then `n_volumes`)
    /// into `out` (the target grid, then the volumes): `resample_series`. `hmc` holds one
    /// voxel-to-voxel affine per volume (or none), `pe` one readout vector per volume (or one
    /// for all).
    pub fn resample_series<O: Output + Jacobian>(
        &self,
        source: &[f32],
        n_volumes: usize,
        hmc: Option<&[Mat4]>,
        pe: &[PeInfo],
        out: &mut [O],
        n_threads: usize,
    ) -> Result<(), ResampleError> {
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
            return Ok(());
        }
        let threads = parallel::resolve_threads(n_threads);
        parallel::with_threads(threads, || -> Result<(), ResampleError> {
            if n_volumes >= threads && threads > 1 {
                // Whole volumes per task: at most `threads` prefiltered volumes in memory.
                out.par_chunks_mut(n_out)
                    .enumerate()
                    .with_max_len(1)
                    .try_for_each(|(t, vol_out)| {
                        self.volume(
                            &source[t * n_src..(t + 1) * n_src],
                            hmc.map(|h| &h[t]),
                            &distortions[which[t]],
                            vol_out,
                            false,
                        )
                    })
            } else {
                for (t, vol_out) in out.chunks_mut(n_out).enumerate() {
                    self.volume(
                        &source[t * n_src..(t + 1) * n_src],
                        hmc.map(|h| &h[t]),
                        &distortions[which[t]],
                        vol_out,
                        threads > 1,
                    )?;
                }
                Ok(())
            }
        })?
    }
}

/// nibabel's `apply_affine(h, p)` for one point, as numpy evaluates `pts @ rzs.T + trans`:
/// the 3×3 product with fused multiply-adds (BLAS), then the translation.
#[inline(always)]
pub fn apply_affine(h: &Mat4, p: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|r| {
        let s = p[0] * h[r][0];
        let s = p[1].mul_add(h[r][1], s);
        let s = p[2].mul_add(h[r][2], s);
        s + h[r][3]
    })
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
}
