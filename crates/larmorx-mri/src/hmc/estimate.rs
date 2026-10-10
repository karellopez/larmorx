// SPDX-License-Identifier: Apache-2.0
//! The registration schedule (`specs/mcflirt.md` §5, §8): every volume registered to the
//! reference by stages of line searches on coarse reference grids.

use larmorx_core::parallel::{self, ThreadPoolError};
use rayon::prelude::*;

use super::cost::{CostFunction, TestVolume, normcorr, voxel_map};
use super::grid::subsample;
use super::rigid::{self, IDENTITY, IDENTITY_PARAMS, Mat4, Params};
use super::search::sweep;
use super::volume::Volume;

/// What the series is registered to.
#[derive(Clone, Debug, PartialEq)]
pub enum Reference {
    /// A separate volume (`-reffile`): every volume of the series is registered, in order.
    Volume(Volume),
    /// Volume `index` of the series (`-refvol`; mcflirt's default is `N / 2`). That volume
    /// is not registered (its matrix is the identity).
    Index(usize),
    /// The mean of the series after a first registration to volume `index` (`-meanvol`).
    Mean(usize),
}

/// Estimation settings (mcflirt's options, §3).
#[derive(Clone, Debug, PartialEq)]
pub struct EstimateParams {
    /// Stages (`-stages`): 1 (8 mm), 2 (+ 4 mm), 3 (+ 4 mm, tighter tolerance, the default),
    /// 4 (+ sinc interpolation in the cost).
    pub stages: usize,
    /// Degrees of freedom (`-dof`, 6 to 12).
    pub dof: usize,
    pub cost: CostFunction,
    /// The edge-weighting width in mm (`-smooth`, default 1); 0 turns weighting off.
    pub smooth: f32,
    /// The divisor of the rotation tolerances (`-rotation`, default 1).
    pub rotation: f64,
    /// Histogram bins of the histogram costs (`-bins`, default 256).
    pub bins: usize,
    /// No previous-volume initialisation in the first stage (`-fudge`).
    pub fudge: bool,
    /// Worker threads (0 = all logical CPUs). The result does not depend on it.
    pub n_threads: usize,
}

impl Default for EstimateParams {
    fn default() -> Self {
        EstimateParams {
            stages: 3,
            dof: 6,
            cost: CostFunction::NormCorr,
            smooth: 1.0,
            rotation: 1.0,
            bins: 256,
            fudge: false,
            n_threads: 1,
        }
    }
}

/// The estimation failed.
#[derive(Debug, thiserror::Error)]
pub enum EstimateError {
    #[error("the series has no volumes")]
    Empty,
    #[error("reference volume {index} is out of range for {n} volumes")]
    BadIndex { index: usize, n: usize },
    #[error("the volumes of the series do not all have the same shape")]
    Shapes,
    #[error(transparent)]
    Threads(#[from] ThreadPoolError),
}

/// The result of the estimation.
#[derive(Clone, Debug)]
pub struct Estimate {
    /// One matrix per volume (FSL convention, §4.2): test-volume FSL-mm to reference FSL-mm.
    pub matrices: Vec<Mat4>,
    /// The reference the matrices point to: the separate volume, volume `index` of the
    /// series, or the mean (§10.3).
    pub reference: Volume,
    /// The reference index, when the reference is a volume of the series.
    pub reference_index: Option<usize>,
    /// The mean volume of `-meanvol` (after the first registration).
    pub mean: Option<Volume>,
}

/// The base tolerances (§5.2): rotations 0.005 rad / `rotation`, translations 0.2 mm, scales
/// 0.002, skews 0.001, times the stage factor (float32).
fn tolerances(rotation: f64, factor: f32) -> [f32; 12] {
    let f = f64::from(factor);
    let r = (0.005 / rotation * f) as f32;
    let t = (0.2 * f) as f32;
    let s = (0.002 * f) as f32;
    let k = (0.001 * f) as f32;
    [r, r, r, t, t, t, s, s, s, k, k, k]
}

/// One stage's settings.
struct Stage<'a> {
    grid: &'a Volume,
    tolerances: [f32; 12],
    order: Vec<usize>,
    ncompose: usize,
    smooth: f32,
}

/// Registers `test` (with centre `centre`) to the stage's grid from `start`.
fn register(test: &Volume, centre: [f64; 3], start: &Mat4, stage: &Stage<'_>) -> Mat4 {
    let mut p: Params = rigid::decompose(start, centre);
    if !rigid::is_finite(&p) {
        p = IDENTITY_PARAMS;
    }
    let tv = TestVolume::new(test, stage.smooth);
    let grid = stage.grid;
    let trace = std::env::var_os("LARMORX_HMC_TRACE");
    let mut cost = |q: &Params| {
        let m = rigid::compose(q, stage.ncompose, centre);
        let b = voxel_map(&m, test.voxel_size, grid.voxel_size);
        let c = normcorr(grid, &tv, &b);
        if let Some(path) = &trace {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).create(true).open(path).unwrap();
            let _ = write!(f, "Cost::affmat = \n{}\nCOST {c:e} P {:?} C {:?}\n", super::report::mat_text(&m), &q[..6], centre);
        }
        c
    };
    let (p, _) = sweep(&mut cost, &p, &stage.order, &stage.tolerances);
    rigid::compose(&p, stage.ncompose, centre)
}

/// The visiting order and stage-1 chains (§5.1, §5.4): each chain is a list of volumes whose
/// first starts from the identity and each next from its predecessor's stage-1 result.
fn chains(n: usize, reference: Option<usize>, fudge: bool) -> Vec<Vec<usize>> {
    let mut sweeps: Vec<Vec<usize>> = match reference {
        None => vec![(0..n).collect()],
        Some(r) => vec![(r + 1..n).collect(), (0..r).rev().collect()],
    };
    sweeps.retain(|s| !s.is_empty());
    let mut out = Vec::new();
    for s in sweeps {
        let mut chain = Vec::new();
        for v in s {
            // The last volume of the series starts from the identity; so does every volume
            // with -fudge.
            if (v == n - 1 || fudge) && !chain.is_empty() {
                out.push(std::mem::take(&mut chain));
            }
            chain.push(v);
        }
        out.push(chain);
    }
    out
}

/// Registers every volume of `series` to `reference` (§5): returns one matrix per volume.
pub fn estimate(
    series: &[Volume],
    reference: &Reference,
    params: &EstimateParams,
) -> Result<Estimate, EstimateError> {
    let n = series.len();
    if n == 0 {
        return Err(EstimateError::Empty);
    }
    if series.iter().any(|v| v.shape != series[0].shape) {
        return Err(EstimateError::Shapes);
    }
    match reference {
        Reference::Volume(r) => {
            let matrices = run_stages(series, r, None, params)?;
            Ok(Estimate {
                matrices,
                reference: r.clone(),
                reference_index: None,
                mean: None,
            })
        }
        &Reference::Index(index) => {
            if index >= n {
                return Err(EstimateError::BadIndex { index, n });
            }
            let matrices = run_stages(series, &series[index], Some(index), params)?;
            Ok(Estimate {
                matrices,
                reference: series[index].clone(),
                reference_index: Some(index),
                mean: None,
            })
        }
        &Reference::Mean(index) => {
            if index >= n {
                return Err(EstimateError::BadIndex { index, n });
            }
            let first = EstimateParams {
                stages: params.stages.min(3),
                ..params.clone()
            };
            let matrices = run_stages(series, &series[index], Some(index), &first)?;
            let mean = super::resample::mean_volume(series, &matrices, params.n_threads)?;
            let matrices = run_stages(series, &mean, None, params)?;
            Ok(Estimate {
                matrices,
                reference: mean.clone(),
                reference_index: None,
                mean: Some(mean),
            })
        }
    }
}

/// The stages of §5.2 for one reference.
fn run_stages(
    series: &[Volume],
    reference: &Volume,
    ref_index: Option<usize>,
    params: &EstimateParams,
) -> Result<Vec<Mat4>, EstimateError> {
    let n = series.len();
    let dof = params.dof.clamp(6, 12);
    let ncompose = dof;
    let order: Vec<usize> = (0..dof).collect();
    let mut matrices = vec![IDENTITY; n];
    let registered: Vec<usize> = (0..n).filter(|&v| Some(v) != ref_index).collect();
    if params.stages == 0 || registered.is_empty() {
        return Ok(matrices);
    }
    parallel::with_threads(params.n_threads, || {
        let centres: Vec<[f64; 3]> = series.par_iter().map(Volume::centre_of_mass).collect();
        let grid8 = subsample(reference, 8.0);
        let grid4 = subsample(reference, 4.0);
        // Stage 1: chains of volumes, each started from its predecessor's result.
        let stage1 = Stage {
            grid: &grid8,
            tolerances: tolerances(params.rotation, 0.8),
            order: order.clone(),
            ncompose,
            smooth: params.smooth,
        };
        let results: Vec<Vec<(usize, Mat4)>> = chains(n, ref_index, params.fudge)
            .par_iter()
            .map(|chain| {
                let mut prev = IDENTITY;
                chain
                    .iter()
                    .map(|&v| {
                        prev = register(&series[v], centres[v], &prev, &stage1);
                        (v, prev)
                    })
                    .collect()
            })
            .collect();
        for (v, m) in results.into_iter().flatten() {
            matrices[v] = m;
        }
        // Stages 2 and 3 (and 4): every volume from its own previous result.
        for (stage, factor) in [(2usize, 0.8f32), (3, 0.1)] {
            if params.stages < stage {
                break;
            }
            let s = Stage {
                grid: &grid4,
                tolerances: tolerances(params.rotation, factor),
                order: order.clone(),
                ncompose,
                smooth: params.smooth,
            };
            let out: Vec<Mat4> = registered
                .par_iter()
                .map(|&v| register(&series[v], centres[v], &matrices[v], &s))
                .collect();
            for (&v, m) in registered.iter().zip(out) {
                matrices[v] = m;
            }
        }
        matrices
    })
    .map_err(EstimateError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_one_chains() {
        // With a separate reference: 0 from the identity, 1..N-2 chained, N-1 alone.
        assert_eq!(chains(5, None, false), vec![vec![0, 1, 2, 3], vec![4]]);
        // Without: r+1 .. N-2 chained, N-1 alone, then r-1 down to 0.
        assert_eq!(
            chains(6, Some(3), false),
            vec![vec![4], vec![5], vec![2, 1, 0]]
        );
        assert_eq!(chains(7, Some(2), false), vec![vec![3, 4, 5], vec![6], vec![1, 0]]);
        assert_eq!(chains(1, Some(0), false), Vec::<Vec<usize>>::new());
        assert_eq!(chains(2, None, false), vec![vec![0], vec![1]]);
        // -fudge: every volume alone.
        assert_eq!(chains(3, None, true), vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn tolerances_of_the_stages() {
        let t = tolerances(1.0, 0.8);
        assert_eq!(t[0], 0.004);
        assert_eq!(t[3], 0.16);
        let t = tolerances(2.0, 0.1);
        assert_eq!(t[0], 0.000_25);
        assert_eq!(t[3], 0.02);
    }
}
