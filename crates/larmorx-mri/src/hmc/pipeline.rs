// SPDX-License-Identifier: Apache-2.0
//! The whole correction as one call: estimation, the matrices in every convention, the motion
//! parameters, RMS displacements and (optionally) the corrected series.

use larmorx_core::parallel::ThreadPoolError;

use super::estimate::{Estimate, EstimateError, EstimateParams, Reference, estimate};
use super::image::Series;
use super::report::{motion_parameters, rms_series};
use super::resample::{Interpolation, resample_series};
use super::rigid::Mat4;
use super::volume::Volume;
use super::world::{Geometry, itk_transform, ras_transform};

/// Which reference to register to.
#[derive(Clone, Debug, PartialEq)]
pub enum ReferenceChoice<'a> {
    /// Volume `index` of the series (`None`: mcflirt's default, `N / 2`).
    Index(Option<usize>),
    /// The mean of the series after a first registration (`-meanvol`): to volume `index`
    /// (`None`: `N / 2`), or to a separate volume (`-meanvol` with `-reffile`), whose grid the
    /// mean is then on.
    Mean(Option<usize>),
    MeanFrom(&'a Volume, Geometry),
    /// A separate volume and its geometry (`-reffile`).
    External(&'a Volume, Geometry),
}

/// Options of [`run`].
#[derive(Clone, Debug, PartialEq)]
pub struct HmcOptions {
    pub estimate: EstimateParams,
    /// The final interpolation (`-sinc_final`, `-spline_final`, `-nn_final`).
    pub interpolation: Interpolation,
    /// A matrix applied before every volume's matrix in the final resampling only (`-init`).
    pub init: Option<Mat4>,
    /// Whether to resample the series.
    pub resample: bool,
}

impl Default for HmcOptions {
    fn default() -> Self {
        HmcOptions {
            estimate: EstimateParams::default(),
            interpolation: Interpolation::Trilinear,
            init: None,
            resample: true,
        }
    }
}

/// The correction failed.
#[derive(Debug, thiserror::Error)]
pub enum HmcError {
    #[error(transparent)]
    Estimate(#[from] EstimateError),
    #[error(transparent)]
    Threads(#[from] ThreadPoolError),
    #[error(
        "the reference has {reference} voxels and the volumes of the series {series}: the \
         corrected series would not fit (resample=False estimates only)"
    )]
    ReferenceSize { reference: usize, series: usize },
}

/// Everything [`run`] computes.
#[derive(Clone, Debug)]
pub struct HmcOutput {
    pub estimate: Estimate,
    /// The geometry of the reference (for the world transforms).
    pub reference_geometry: Geometry,
    /// Per volume: `[rx, ry, rz, tx, ty, tz]` (radians, mm) about the reference's
    /// intensity-weighted centre, mcflirt's `.par` convention.
    pub params: Vec<[f64; 6]>,
    /// mcflirt's RMS displacements (radius 80 mm): from the reference and from the previous
    /// volume (one fewer).
    pub rms_abs: Vec<f64>,
    pub rms_rel: Vec<f64>,
    /// Per volume: the transform in RAS world coordinates mapping reference points to the
    /// volume's points (nitransforms' convention), and the same in LPS (ITK/ANTs).
    pub ras: Vec<Mat4>,
    pub itk: Vec<Mat4>,
    /// The corrected volumes on the output grid (the reference's with an external reference),
    /// in memory order, when resampling was asked for.
    pub corrected: Option<Vec<Volume>>,
}

/// The geometry of a series as read.
pub fn series_geometry(series: &Series) -> Geometry {
    Geometry::new(
        &series.affine(),
        series.shape(),
        series.voxel_size(),
        series.flipped,
    )
}

/// Registers every volume of `series` and computes the outputs (§5–§10).
pub fn run(
    series: &Series,
    reference: &ReferenceChoice<'_>,
    options: &HmcOptions,
) -> Result<HmcOutput, HmcError> {
    let n = series.volumes.len();
    let geometry = series_geometry(series);
    let index = |i: Option<usize>| i.unwrap_or(n / 2);
    let check_size = |v: &Volume, options: &HmcOptions| {
        let series_len = series.volumes.first().map_or(0, Volume::len);
        if options.resample && v.len() != series_len {
            return Err(HmcError::ReferenceSize {
                reference: v.len(),
                series: series_len,
            });
        }
        Ok(())
    };
    let (reference, reference_geometry) = match reference {
        ReferenceChoice::Index(i) => (Reference::Index(index(*i)), geometry),
        ReferenceChoice::Mean(i) => (
            Reference::Mean(Box::new(Reference::Index(index(*i)))),
            geometry,
        ),
        ReferenceChoice::MeanFrom(v, g) => {
            check_size(v, options)?;
            (
                Reference::Mean(Box::new(Reference::Volume((*v).clone()))),
                *g,
            )
        }
        ReferenceChoice::External(v, g) => {
            check_size(v, options)?;
            (Reference::Volume((*v).clone()), *g)
        }
    };
    let est = estimate(&series.volumes, &reference, &options.estimate)?;
    let params = motion_parameters(&est.matrices, &est.reference);
    let (rms_abs, rms_rel) = rms_series(&est.matrices, &est.reference);
    let ras = est
        .matrices
        .iter()
        .map(|m| ras_transform(m, &reference_geometry, &geometry))
        .collect();
    let itk = est
        .matrices
        .iter()
        .map(|m| itk_transform(m, &reference_geometry, &geometry))
        .collect();
    let corrected = if options.resample {
        let (shape, voxel) = match &reference {
            Reference::Volume(v) => (v.shape, v.voxel_size),
            Reference::Mean(first) => match first.as_ref() {
                Reference::Volume(v) => (v.shape, v.voxel_size),
                _ => (geometry.shape, geometry.voxel_size),
            },
            Reference::Index(_) => (geometry.shape, geometry.voxel_size),
        };
        Some(resample_series(
            &series.volumes,
            &est.matrices,
            options.init.as_ref(),
            shape,
            voxel,
            options.interpolation,
            options.estimate.n_threads,
        )?)
    } else {
        None
    };
    Ok(HmcOutput {
        estimate: est,
        reference_geometry,
        params,
        rms_abs,
        rms_rel,
        ras,
        itk,
        corrected,
    })
}

/// Power's framewise displacement from motion parameters: the sum of the absolute
/// differences of consecutive volumes' translations and of their rotations as arc lengths on
/// a sphere of `radius` mm (fMRIPrep uses 50). The first volume's is 0.
pub fn framewise_displacement(params: &[[f64; 6]], radius: f64) -> Vec<f64> {
    let mut fd = vec![0.0; params.len()];
    for t in 1..params.len() {
        let (a, b) = (&params[t - 1], &params[t]);
        fd[t] = (0..3).map(|k| (b[k] - a[k]).abs() * radius).sum::<f64>()
            + (3..6).map(|k| (b[k] - a[k]).abs()).sum::<f64>();
    }
    fd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fd_of_known_motion() {
        let p = [
            [0.0; 6],
            [0.01, 0.0, 0.0, 0.5, 0.0, -0.25],
            [0.01, 0.0, 0.0, 0.5, 0.0, -0.25],
        ];
        let fd = framewise_displacement(&p, 50.0);
        assert_eq!(fd[0], 0.0);
        assert!((fd[1] - (0.5 + 0.75)).abs() < 1e-12);
        assert_eq!(fd[2], 0.0);
    }
}
