// SPDX-License-Identifier: Apache-2.0
//! The registration schedule (`specs/mcflirt.md` §5, §8, §12): every volume registered to the
//! reference by stages of line searches on coarse reference grids.
//!
//! Stage 1 is a chain within each sweep (each volume starts from its predecessor's result), so
//! its volumes run one after another, with each cost evaluation's slices sampled in parallel.
//! Stages 2–4 start each volume from its own previous result, so the volumes run in parallel.
//! Either way every evaluation gives the same bits whatever the thread count.

use larmorx_core::parallel::{self, ThreadPoolError};
use rayon::prelude::*;

use super::cost::{self, CostFunction, CostInterpolation, TestVolume, normcorr, voxel_map};
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

/// When the in-plane mode (§12) is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InPlane {
    /// When a checked image (the reference grids, the test volumes) has fewer than 3 slices
    /// or a z extent below `fov` mm (`-fov`, default 20).
    Auto { fov: i64 },
    /// Always (`-2d`).
    Force,
    /// Never (larmorx's option; mcflirt has none).
    Never,
}

impl Default for InPlane {
    fn default() -> Self {
        InPlane::Auto { fov: 20 }
    }
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
    pub in_plane: InPlane,
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
            in_plane: InPlane::default(),
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
    /// Whether the in-plane mode (§12) was used.
    pub in_plane: bool,
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

/// Whether an image is a thin slab (§12): fewer than 3 slices, or a z extent below `fov` mm.
fn thin(v: &Volume, fov: i64) -> bool {
    v.shape[2] < 3 || (v.shape[2] as f64) * f64::from(v.voxel_size[2]) < fov as f64
}

/// The volume padded in z with `times` copies of its first slice before it and of its last
/// slice after it, its z voxel size set to 8 mm (the in-plane mode, §12).
fn pad_z(v: &Volume, times: usize) -> Volume {
    let [nx, ny, nz] = v.shape;
    let plane = nx * ny;
    let mut data = Vec::with_capacity(plane * (nz + 2 * times));
    for _ in 0..times {
        data.extend_from_slice(&v.data[..plane]);
    }
    data.extend_from_slice(&v.data);
    for _ in 0..times {
        data.extend_from_slice(&v.data[plane * (nz - 1)..]);
    }
    Volume::new(
        [nx, ny, nz + 2 * times],
        [v.voxel_size[0], v.voxel_size[1], 8.0],
        data,
    )
}

/// One stage's settings.
struct Stage<'a> {
    /// The reference grid (bin indices for the histogram costs).
    grid: &'a Volume,
    tolerances: [f32; 12],
    order: Vec<usize>,
    ncompose: usize,
    smooth: f32,
    cost: CostFunction,
    bins: usize,
    sinc: bool,
    in_plane: bool,
    /// Sample each evaluation's slices in parallel (stage 1, whose volumes are sequential).
    parallel: bool,
}

/// The cost of matrix `m` for test volume `tv` in `stage`.
fn evaluate(stage: &Stage<'_>, tv: &TestVolume<'_>, m: &Mat4) -> f32 {
    let grid = stage.grid;
    let b = voxel_map(m, tv.vol.voxel_size, grid.voxel_size);
    let par = stage.parallel;
    match stage.cost {
        CostFunction::NormCorr => normcorr(grid, tv, &b, par),
        CostFunction::LeastSquares => cost::least_squares(grid, tv, &b, par),
        CostFunction::CorrRatio => cost::corr_ratio(grid, tv, &b, stage.bins, par),
        CostFunction::Woods => cost::woods(grid, tv, &b, stage.bins, par),
        CostFunction::MutualInfo => cost::mutual_info(grid, tv, &b, stage.bins, false, par),
        CostFunction::NormMi => cost::mutual_info(grid, tv, &b, stage.bins, true, par),
    }
}

/// Registers `test` (with centre `centre`) to the stage's grid from `start`.
fn register(test: &Volume, centre: [f64; 3], start: &Mat4, stage: &Stage<'_>) -> Mat4 {
    let mut p: Params = rigid::decompose(start, centre);
    if !rigid::is_finite(&p) {
        p = IDENTITY_PARAMS;
    }
    if stage.in_plane {
        // Only rz, tx and ty move; the others stay at the identity's values.
        p[0] = 0.0;
        p[1] = 0.0;
        p[5] = 0.0;
        p[6..].copy_from_slice(&IDENTITY_PARAMS[6..]);
    }
    let padded;
    let vol = if stage.in_plane {
        padded = pad_z(test, 1);
        &padded
    } else {
        test
    };
    let mut tv = TestVolume::new(vol, stage.smooth);
    if stage.sinc {
        tv.interpolation = CostInterpolation::Sinc {
            background: test.background(),
        };
    }
    let mut cost = |q: &Params| evaluate(stage, &tv, &rigid::compose(q, stage.ncompose, centre));
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
            let (matrices, in_plane) = run_stages(series, r, None, params)?;
            Ok(Estimate {
                matrices,
                reference: r.clone(),
                reference_index: None,
                mean: None,
                in_plane,
            })
        }
        &Reference::Index(index) => {
            if index >= n {
                return Err(EstimateError::BadIndex { index, n });
            }
            let (matrices, in_plane) = run_stages(series, &series[index], Some(index), params)?;
            Ok(Estimate {
                matrices,
                reference: series[index].clone(),
                reference_index: Some(index),
                mean: None,
                in_plane,
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
            let (matrices, _) = run_stages(series, &series[index], Some(index), &first)?;
            let mean = super::resample::mean_volume(series, &matrices, params.n_threads)?;
            let (matrices, in_plane) = run_stages(series, &mean, None, params)?;
            Ok(Estimate {
                matrices,
                reference: mean.clone(),
                reference_index: None,
                mean: Some(mean),
                in_plane,
            })
        }
    }
}

/// The stages of §5.2 for one reference. Returns the matrices and whether the in-plane mode
/// was used.
fn run_stages(
    series: &[Volume],
    reference: &Volume,
    ref_index: Option<usize>,
    params: &EstimateParams,
) -> Result<(Vec<Mat4>, bool), EstimateError> {
    let n = series.len();
    let dof = params.dof.clamp(6, 12);
    let mut matrices = vec![IDENTITY; n];
    let registered: Vec<usize> = (0..n).filter(|&v| Some(v) != ref_index).collect();
    if params.stages == 0 || registered.is_empty() {
        return Ok((matrices, false));
    }
    let out = parallel::with_threads(params.n_threads, || {
        let centres: Vec<[f64; 3]> = series.par_iter().map(Volume::centre_of_mass).collect();
        let grid8 = subsample(reference, 8.0);
        let grid4 = subsample(reference, 4.0);
        let check = |v: &Volume| match params.in_plane {
            InPlane::Force => true,
            InPlane::Never => false,
            InPlane::Auto { fov } => thin(v, fov),
        };
        let mut in_plane = check(&grid8) || check(&series[0]);
        let stage_for =
            |grid: &Volume, factor: f32, bins: usize, sinc: bool, in_plane: bool, par: bool| {
                let grid = if in_plane {
                    // The stage-1 grid is padded twice, the others once.
                    pad_z(grid, if par { 2 } else { 1 })
                } else {
                    grid.clone()
                };
                let grid = if params.cost.is_histogram() {
                    cost::bin_grid(&grid, bins)
                } else {
                    grid
                };
                let (order, ncompose) = if in_plane {
                    (vec![2, 3, 4], 6)
                } else {
                    ((0..dof).collect(), dof)
                };
                (grid, factor, order, ncompose, sinc, in_plane, par, bins)
            };
        // Stage 1: chains of volumes, each started from its predecessor's result.
        let (g1, f1, order1, nc1, _, ip1, _, bins1) =
            stage_for(&grid8, 0.8, params.bins / 8, false, in_plane, true);
        let stage1 = Stage {
            grid: &g1,
            tolerances: tolerances(params.rotation, f1),
            order: order1,
            ncompose: nc1,
            smooth: if ip1 { 0.1 } else { params.smooth },
            cost: params.cost,
            bins: bins1.max(1),
            sinc: false,
            in_plane: ip1,
            parallel: true,
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
        // Stages 2 to 4: every volume from its own previous result, in parallel.
        in_plane = in_plane || check(&grid4);
        for (stage, factor) in [(2usize, 0.8f32), (3, 0.1), (4, 0.1)] {
            if params.stages < stage {
                break;
            }
            let sinc = stage == 4 && params.cost == CostFunction::NormCorr;
            let (g, f, order, nc, sinc, ip, _, bins) =
                stage_for(&grid4, factor, params.bins / 4, sinc, in_plane, false);
            let s = Stage {
                grid: &g,
                tolerances: tolerances(params.rotation, f),
                order,
                ncompose: nc,
                smooth: if ip { 0.1 } else { params.smooth },
                cost: params.cost,
                bins: bins.max(1),
                sinc,
                in_plane: ip,
                parallel: false,
            };
            let out: Vec<Mat4> = registered
                .par_iter()
                .map(|&v| register(&series[v], centres[v], &matrices[v], &s))
                .collect();
            for (&v, m) in registered.iter().zip(out) {
                matrices[v] = m;
            }
        }
        (matrices, in_plane)
    })?;
    Ok(out)
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
        assert_eq!(
            chains(7, Some(2), false),
            vec![vec![3, 4, 5], vec![6], vec![1, 0]]
        );
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

    #[test]
    fn padding_repeats_the_end_slices() {
        let v = Volume::new([1, 1, 3], [2.0, 2.0, 3.0], vec![1.0, 2.0, 3.0]);
        let p = pad_z(&v, 2);
        assert_eq!(p.data, vec![1.0, 1.0, 1.0, 2.0, 3.0, 3.0, 3.0]);
        assert_eq!(p.voxel_size, [2.0, 2.0, 8.0]);
        assert!(thin(&v, 20));
        assert!(!thin(
            &Volume::new([1, 1, 5], [1.0, 1.0, 4.0], vec![0.0; 5]),
            20
        ));
    }
}
