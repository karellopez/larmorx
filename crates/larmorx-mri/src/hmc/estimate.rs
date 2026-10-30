// SPDX-License-Identifier: Apache-2.0
//! The registration schedule (`specs/mcflirt.md` §5, §8, §12): every volume registered to the
//! reference by stages of line searches on coarse reference grids.
//!
//! Stage 1 is a chain within each sweep (each volume starts from its predecessor's result), so
//! its volumes run one after another, with each cost evaluation's slices sampled in parallel.
//! Stages 2–4 start each volume from its own previous result, so the volumes run in parallel.
//! Either way every evaluation gives the same bits whatever the thread count.
//!
//! Three of mcflirt's behaviours here are defects that larmorx corrects by default (D13,
//! `docs/api/upstream-defects.md`): the last volume of the series starts stage 1 from the
//! identity instead of its predecessor's result (FSL-01), the in-plane mode pads the first
//! stage's reference grid twice (FSL-03), and one NaN voxel makes a volume's matrix NaN
//! (FSL-14). [`EstimateParams::upstream_defects`] chooses; [`Estimate::notices`] says which
//! changed the result. mcflirt's inflated correlation count (FSL-02) is corrected only on
//! request ([`EstimateParams::normcorr_count`]), pending a decision.
//!
//! Bins no histogram can have (`specs/mcflirt.md` §12.1): with a histogram cost and `-bins`
//! below 8 the first stage has none (FSL-21), and a negative `-bins` crashes mcflirt whatever the
//! cost (FSL-66). Both are refused by default before any work (rule B, [`refused_bins`]); with
//! `reproduce` the histogram costs run with no bins as mcflirt does, and where mcflirt crashes
//! the estimation ends with an error saying how ([`McflirtEnd`]).

use larmorx_core::defects::{self, Defect, Notice, UpstreamDefects};
use larmorx_core::parallel::{self, ThreadPoolError};
use rayon::prelude::*;

use super::cost::{
    self, CostFunction, CostInterpolation, NormCorrCount, TestVolume, normcorr, voxel_map,
};
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
    /// The mean of the series after a first registration to the inner reference (`-meanvol`;
    /// with `-reffile`, the first registration is to that file).
    Mean(Box<Reference>),
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
    /// Histogram bins of the histogram costs (`-bins`, default 256): stage 1 uses `bins / 8`,
    /// stages 2 to 4 `bins / 4` (truncated toward zero). Fewer than 8 with a histogram cost
    /// (FSL-21) and negative values (FSL-66) are refused by default ([`refused_bins`]).
    pub bins: i64,
    /// No previous-volume initialisation in the first stage (`-fudge`).
    pub fudge: bool,
    pub in_plane: InPlane,
    /// Worker threads (0 = all logical CPUs). The result does not depend on it.
    pub n_threads: usize,
    /// D13: correct mcflirt's defects FSL-01, FSL-03 and FSL-14 (the default) or reproduce
    /// them.
    pub upstream_defects: UpstreamDefects,
    /// The count of the weighted normalised correlation: mcflirt's (the default) or the sum of
    /// the weights (FSL-02; opt-in until decided). Must be mcflirt's with
    /// [`UpstreamDefects::Reproduce`].
    pub normcorr_count: NormCorrCount,
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
            upstream_defects: UpstreamDefects::Fix,
            normcorr_count: NormCorrCount::Mcflirt,
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
    #[error("the first reference of a mean registration cannot itself be a mean")]
    NestedMean,
    #[error(
        "the corrected correlation count (FSL-02) cannot be combined with reproducing mcflirt's \
         defects"
    )]
    CountWithReproduce,
    /// FSL-21 or FSL-66 corrected: bins no stage can use, refused before any work.
    #[error("{0}")]
    Bins(BinsRefusal),
    /// FSL-21 or FSL-66 reproduced: where mcflirt crashes, the estimation ends.
    #[error("{}", .0.error())]
    McflirtEnds(McflirtEnd),
    #[error(transparent)]
    Threads(#[from] ThreadPoolError),
}

/// The bins of stage `stage` (1 to 4) for `-bins bins` (`specs/mcflirt.md` §12.1): `bins / 8`
/// in stage 1 (8 mm), `bins / 4` in stages 2 to 4 (4 mm), truncated toward zero.
pub fn stage_bins(bins: i64, stage: usize) -> i64 {
    if stage <= 1 { bins / 8 } else { bins / 4 }
}

/// How mcflirt ends on bins it cannot use (`specs/mcflirt.md` §12.1), at the start of `stage`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McflirtEnd {
    /// −1 bins (FSL-66): an integer division by zero while the stage prepares its tables; the
    /// process is killed by signal 8 (SIGFPE), with no message of its own.
    DivisionByZero { bins: i64, stage: usize },
    /// −2 or fewer bins (FSL-66): `std::bad_array_new_length`, an abort (signal 6).
    BadArrayLength { bins: i64, stage: usize },
    /// `corratio` with no bins (FSL-21): it writes before its arrays and the C library aborts
    /// when they are released (`double free or corruption (out)`, signal 6), at the stage's
    /// first cost evaluation.
    CorrRatioHeap { bins: i64, stage: usize },
}

impl McflirtEnd {
    /// The defect: FSL-66 for a negative number of bins, FSL-21 for `corratio` with none.
    pub fn defect(&self) -> &'static Defect {
        match self {
            McflirtEnd::CorrRatioHeap { .. } => &defects::FSL_21,
            _ => &defects::FSL_66,
        }
    }

    /// The lines mcflirt prints on standard error as it ends.
    pub fn mcflirt_lines(&self) -> &'static [&'static str] {
        match self {
            McflirtEnd::DivisionByZero { .. } => &[],
            McflirtEnd::BadArrayLength { .. } => &[
                "terminate called after throwing an instance of 'std::bad_array_new_length'",
                "  what():  std::bad_array_new_length",
            ],
            McflirtEnd::CorrRatioHeap { .. } => &["double free or corruption (out)"],
        }
    }

    /// What mcflirt does, in a few words.
    pub fn what(&self) -> String {
        match *self {
            McflirtEnd::DivisionByZero { bins, stage } => format!(
                "-bins {bins} gives stage {stage} -1 bins: mcflirt is killed by signal 8 (an \
                 integer division by zero) when the stage starts, writing nothing"
            ),
            McflirtEnd::BadArrayLength { bins, stage } => format!(
                "-bins {bins} gives stage {stage} {} bins: mcflirt aborts (signal 6, \
                 std::bad_array_new_length) when the stage starts, writing nothing",
                stage_bins(bins, stage)
            ),
            McflirtEnd::CorrRatioHeap { bins, stage } => format!(
                "-bins {bins} gives stage {stage} no bins: mcflirt's corratio writes before its \
                 arrays and aborts (signal 6, double free or corruption) at the stage's first \
                 cost evaluation, writing nothing"
            ),
        }
    }

    /// The error that ends a reproduced run here.
    pub fn error(&self) -> String {
        format!(
            "{} ({}; larmorx stops where mcflirt crashes, and refuses these bins by default)",
            self.what(),
            self.defect().id
        )
    }

    /// The notice of the reproduced run.
    pub fn notice(&self) -> Notice {
        Notice::new(
            self.defect(),
            UpstreamDefects::Reproduce,
            format!("{}; larmorx stops there with an error", self.what()),
        )
    }
}

/// How mcflirt ends with `-bins bins`, `stages` stages and `cost` when registration runs: the
/// first stage whose bins it cannot use (§12.1), if any.
pub fn mcflirt_end(bins: i64, stages: usize, cost: CostFunction) -> Option<McflirtEnd> {
    (1..=stages.min(4)).find_map(|stage| match stage_bins(bins, stage) {
        -1 => Some(McflirtEnd::DivisionByZero { bins, stage }),
        b if b <= -2 => Some(McflirtEnd::BadArrayLength { bins, stage }),
        0 if cost == CostFunction::CorrRatio => Some(McflirtEnd::CorrRatioHeap { bins, stage }),
        _ => None,
    })
}

/// Bins larmorx refuses by default (FSL-21, FSL-66; rule B): the defect, the error and the
/// notice's detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinsRefusal {
    pub defect: &'static Defect,
    pub message: String,
    pub detail: String,
}

impl std::fmt::Display for BinsRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl BinsRefusal {
    /// The notice of the refusal (larmorx's default).
    pub fn notice(&self) -> Notice {
        Notice::new(self.defect, UpstreamDefects::Fix, self.detail.clone())
    }
}

/// The default's refusal of `-bins bins` with `stages` stages and `cost` (`specs/mcflirt.md`
/// §12.1), whatever the series: with a histogram cost and fewer than 8 bins the first stage
/// would have none (FSL-21); a negative number of bins (FSL-66). `None` where the bins are
/// usable or no stage runs (`-stages 0`).
pub fn refused_bins(bins: i64, stages: usize, cost: CostFunction) -> Option<BinsRefusal> {
    if stages == 0 {
        return None;
    }
    if cost.is_histogram() && stage_bins(bins, 1) == 0 {
        return Some(BinsRefusal {
            defect: &defects::FSL_21,
            message: format!(
                "-bins {bins}: the histogram costs need at least 8 bins (the first stage uses \
                 -bins / 8 of them; mcflirt computes them with none, FSL-21)"
            ),
            detail: format!(
                "-bins {bins} is refused with this cost: the first stage's histogram would have \
                 no bins"
            ),
        });
    }
    if bins < 0 {
        let how = match mcflirt_end(bins, stages, cost) {
            Some(end) => end.what(),
            None => format!(
                "-bins {bins} gives every stage no bins, which this cost does not use, in mcflirt"
            ),
        };
        return Some(BinsRefusal {
            defect: &defects::FSL_66,
            message: format!(
                "-bins {bins}: a negative number of bins (mcflirt prepares -bins / 8 and -bins / \
                 4 bins for every cost and crashes on a negative count, FSL-66)"
            ),
            detail: format!("-bins {bins} (a negative number of bins) is refused: {how}"),
        });
    }
    None
}

/// The notice of a reproduced run on bins larmorx refuses by default, where mcflirt does not
/// crash (§12.1): a histogram cost with no bins in a stage (FSL-21), or a negative `-bins`
/// with a cost that uses none (FSL-66).
fn reproduced_bins_notice(bins: i64, stages: usize, cost: CostFunction) -> Option<Notice> {
    let mode = UpstreamDefects::Reproduce;
    if cost.is_histogram() && (1..=stages.min(4)).any(|s| stage_bins(bins, s) == 0) {
        return Some(Notice::new(
            &defects::FSL_21,
            mode,
            format!(
                "-bins {bins}: the first stage's histogram has no bins, as in mcflirt; its cost \
                 is degenerate and the matrices arbitrary"
            ),
        ));
    }
    (bins < 0).then(|| {
        Notice::new(
            &defects::FSL_66,
            mode,
            format!(
                "-bins {bins} (a negative number of bins) gives every stage none, as in \
                 mcflirt; this cost uses none"
            ),
        )
    })
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
    /// The upstream defects that changed the matrices (or, reproduced, would have).
    pub notices: Vec<Notice>,
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
    /// Leave non-finite values out of the cost (FSL-14 corrected).
    skip_non_finite: bool,
    count: NormCorrCount,
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

/// Registers `test` (with centre `centre`) to the stage's grid from `start`. In the in-plane
/// mode the test volume is padded (§12) and its centre is the padded volume's (observed by the
/// clean-room implementer, 2026-10-29: with the unpadded volume's centre the evaluated matrices
/// of mcflirt 6.0.7.23's `-verbose 20` trace differ from the first rotation step on;
/// `validation/investigations/cleanroom-align/mi_trace.py`).
fn register(test: &Volume, centre: [f64; 3], start: &Mat4, stage: &Stage<'_>) -> Mat4 {
    let padded = stage.in_plane.then(|| pad_z(test, 1));
    let vol = padded.as_ref().unwrap_or(test);
    let centre = match &padded {
        Some(v) if stage.skip_non_finite => v.centre_of_mass_finite(),
        Some(v) => v.centre_of_mass(),
        None => centre,
    };
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
    let mut tv = TestVolume::new(vol, stage.smooth);
    tv.skip_non_finite = stage.skip_non_finite;
    tv.count = stage.count;
    if matches!(stage.cost, CostFunction::MutualInfo | CostFunction::NormMi) {
        // The test bins come from the whole volume, padded as the cost sees it (appendix B).
        tv.test_bins = Some(cost::TestBins::new(vol, stage.bins));
    }
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
/// first starts from the identity and each next from its predecessor's stage-1 result. With
/// `-fudge` every volume starts from the identity. mcflirt also starts the last volume of the
/// series from the identity (FSL-01), which `reproduce` keeps; corrected, it continues its
/// chain.
fn chains(n: usize, reference: Option<usize>, fudge: bool, reproduce: bool) -> Vec<Vec<usize>> {
    let mut sweeps: Vec<Vec<usize>> = match reference {
        None => vec![(0..n).collect()],
        Some(r) => vec![(r + 1..n).collect(), (0..r).rev().collect()],
    };
    sweeps.retain(|s| !s.is_empty());
    let mut out = Vec::new();
    for s in sweeps {
        let mut chain = Vec::new();
        for v in s {
            if (fudge || (reproduce && v == n - 1)) && !chain.is_empty() {
                out.push(std::mem::take(&mut chain));
            }
            chain.push(v);
        }
        out.push(chain);
    }
    out
}

/// The volume visited just before the last volume of the series in its stage-1 sweep, if
/// any: the one the last volume's chain continues from (FSL-01).
fn last_predecessor(n: usize, reference: Option<usize>) -> Option<usize> {
    let last = n.checked_sub(1)?;
    match reference {
        None => last.checked_sub(1),
        Some(r) => (last > r + 1).then(|| last - 1),
    }
}

/// Registers every volume of `series` to `reference` (§5): returns one matrix per volume.
pub fn estimate(
    series: &[Volume],
    reference: &Reference,
    params: &EstimateParams,
) -> Result<Estimate, EstimateError> {
    if params.upstream_defects.fix()
        && let Some(r) = refused_bins(params.bins, params.stages, params.cost)
    {
        return Err(EstimateError::Bins(r));
    }
    let n = series.len();
    if n == 0 {
        return Err(EstimateError::Empty);
    }
    if series.iter().any(|v| v.shape != series[0].shape) {
        return Err(EstimateError::Shapes);
    }
    if params.upstream_defects.reproduce() && params.normcorr_count != NormCorrCount::Mcflirt {
        return Err(EstimateError::CountWithReproduce);
    }
    match reference {
        Reference::Volume(r) => {
            let (matrices, in_plane, notices) = run_stages(series, r, None, params)?;
            Ok(Estimate {
                matrices,
                reference: r.clone(),
                reference_index: None,
                mean: None,
                in_plane,
                notices,
            })
        }
        &Reference::Index(index) => {
            if index >= n {
                return Err(EstimateError::BadIndex { index, n });
            }
            let (matrices, in_plane, notices) =
                run_stages(series, &series[index], Some(index), params)?;
            Ok(Estimate {
                matrices,
                reference: series[index].clone(),
                reference_index: Some(index),
                mean: None,
                in_plane,
                notices,
            })
        }
        Reference::Mean(first_ref) => {
            let (first_volume, first_index) = match first_ref.as_ref() {
                &Reference::Index(index) => {
                    if index >= n {
                        return Err(EstimateError::BadIndex { index, n });
                    }
                    (&series[index], Some(index))
                }
                Reference::Volume(v) => (v, None),
                Reference::Mean(_) => return Err(EstimateError::NestedMean),
            };
            let first = EstimateParams {
                stages: params.stages.min(3),
                ..params.clone()
            };
            let (matrices, _, mut notices) = run_stages(series, first_volume, first_index, &first)?;
            let mean =
                super::resample::mean_volume(series, &matrices, first_volume, params.n_threads)?;
            let (matrices, in_plane, second) = run_stages(series, &mean, None, params)?;
            for n in second {
                defects::push_once(&mut notices, n);
            }
            Ok(Estimate {
                matrices,
                reference: mean.clone(),
                reference_index: None,
                mean: Some(mean),
                in_plane,
                notices,
            })
        }
    }
}

/// The stages of §5.2 for one reference. Returns the matrices, whether the in-plane mode was
/// used, and the upstream defects that changed the matrices (or would have).
fn run_stages(
    series: &[Volume],
    reference: &Volume,
    ref_index: Option<usize>,
    params: &EstimateParams,
) -> Result<(Vec<Mat4>, bool, Vec<Notice>), EstimateError> {
    let n = series.len();
    let dof = params.dof.clamp(6, 12);
    let mode = params.upstream_defects;
    let fix = mode.fix();
    let mut matrices = vec![IDENTITY; n];
    let mut notices = Vec::new();
    let registered: Vec<usize> = (0..n).filter(|&v| Some(v) != ref_index).collect();
    if params.stages == 0 || registered.is_empty() {
        return Ok((matrices, false, notices));
    }
    // FSL-21 and FSL-66 reproduced: mcflirt crashes when a stage starts on bins it cannot use;
    // a histogram cost with no bins runs as mcflirt's does.
    if mode.reproduce() {
        if let Some(end) = mcflirt_end(params.bins, params.stages, params.cost) {
            return Err(EstimateError::McflirtEnds(end));
        }
        notices.extend(reproduced_bins_notice(
            params.bins,
            params.stages,
            params.cost,
        ));
    }
    // FSL-14: volumes (and the reference) with NaN or infinite voxels.
    let non_finite: Vec<usize> = registered
        .iter()
        .copied()
        .filter(|&v| series[v].has_non_finite())
        .collect();
    let reference_non_finite = reference.has_non_finite();
    if !non_finite.is_empty() || reference_non_finite {
        let which = describe_volumes(&non_finite, reference_non_finite);
        let detail = if fix {
            format!(
                "{which} NaN or infinite voxels: they are left out of the centre of mass and the \
                 cost; mcflirt's matrices for such volumes are NaN"
            )
        } else {
            format!(
                "{which} NaN or infinite voxels: as in mcflirt, they make the centre of mass, and \
                 so the matrix, of such volumes NaN"
            )
        };
        notices.push(Notice::new(&defects::FSL_14, mode, detail));
    }
    let skip_non_finite = fix && (!non_finite.is_empty() || reference_non_finite);
    let out = parallel::with_threads(params.n_threads, || {
        let centres: Vec<[f64; 3]> = series
            .par_iter()
            .map(|v| {
                if fix {
                    v.centre_of_mass_finite()
                } else {
                    v.centre_of_mass()
                }
            })
            .collect();
        let grid8 = subsample(reference, 8.0);
        let grid4 = subsample(reference, 4.0);
        let check = |v: &Volume| match params.in_plane {
            InPlane::Force => true,
            InPlane::Never => false,
            InPlane::Auto { fov } => thin(v, fov),
        };
        let mut in_plane = check(&grid8) || check(&series[0]);
        let stage_for =
            |grid: &Volume, factor: f32, bins: i64, sinc: bool, in_plane: bool, par: bool| {
                // A stage's bins: none where mcflirt has none (FSL-21 reproduced); never
                // negative here (refused by default, the end of a reproduced run).
                let bins = bins.max(0) as usize;
                let grid = if in_plane {
                    // mcflirt pads the stage-1 grid twice, the others once (FSL-03).
                    pad_z(grid, if par && !fix { 2 } else { 1 })
                } else {
                    grid.clone()
                };
                let grid = match params.cost {
                    // The mutual-information costs bin as appendix B describes.
                    CostFunction::MutualInfo | CostFunction::NormMi => {
                        cost::bin_grid_mi(&grid, bins, skip_non_finite)
                    }
                    c if c.is_histogram() => cost::bin_grid(&grid, bins),
                    _ => grid,
                };
                let (order, ncompose) = if in_plane {
                    (vec![2, 3, 4], 6)
                } else {
                    ((0..dof).collect(), dof)
                };
                (grid, factor, order, ncompose, sinc, in_plane, par, bins)
            };
        // Stage 1: chains of volumes, each started from its predecessor's result.
        let (g1, f1, order1, nc1, _, ip1, _, bins1) = stage_for(
            &grid8,
            0.8,
            stage_bins(params.bins, 1),
            false,
            in_plane,
            true,
        );
        let stage1 = Stage {
            grid: &g1,
            tolerances: tolerances(params.rotation, f1),
            order: order1,
            ncompose: nc1,
            smooth: if ip1 { 0.1 } else { params.smooth },
            cost: params.cost,
            bins: bins1,
            sinc: false,
            in_plane: ip1,
            parallel: true,
            skip_non_finite,
            count: params.normcorr_count,
        };
        let mut stage_notices = Vec::new();
        if ip1 {
            let detail = if fix {
                "in-plane mode: the first stage's reference grid is padded once, like the other \
                 stages'; mcflirt pads it twice"
            } else {
                "in-plane mode: the first stage's reference grid is padded twice, as in mcflirt"
            };
            stage_notices.push(Notice::new(&defects::FSL_03, mode, detail));
        }
        let results: Vec<Vec<(usize, Mat4)>> = chains(n, ref_index, params.fudge, !fix)
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
        // FSL-01: the last volume's start, when its predecessor's result is not the identity.
        if !params.fudge
            && let Some(p) = last_predecessor(n, ref_index)
            && matrices[p] != IDENTITY
        {
            let detail = if fix {
                format!(
                    "the last volume ({}) starts the first stage from volume {p}'s result, like \
                     the others; mcflirt starts it from the identity",
                    n - 1
                )
            } else {
                format!(
                    "the last volume ({}) starts the first stage from the identity, as in \
                     mcflirt, not from volume {p}'s result",
                    n - 1
                )
            };
            stage_notices.push(Notice::new(&defects::FSL_01, mode, detail));
        }
        // Stages 2 to 4: every volume from its own previous result, in parallel.
        in_plane = in_plane || check(&grid4);
        for (stage, factor) in [(2usize, 0.8f32), (3, 0.1), (4, 0.1)] {
            if params.stages < stage {
                break;
            }
            let sinc = stage == 4 && params.cost == CostFunction::NormCorr;
            let (g, f, order, nc, sinc, ip, _, bins) = stage_for(
                &grid4,
                factor,
                stage_bins(params.bins, stage),
                sinc,
                in_plane,
                false,
            );
            let s = Stage {
                grid: &g,
                tolerances: tolerances(params.rotation, f),
                order,
                ncompose: nc,
                smooth: if ip { 0.1 } else { params.smooth },
                cost: params.cost,
                bins,
                sinc,
                in_plane: ip,
                parallel: false,
                skip_non_finite,
                count: params.normcorr_count,
            };
            let out: Vec<Mat4> = registered
                .par_iter()
                .map(|&v| register(&series[v], centres[v], &matrices[v], &s))
                .collect();
            for (&v, m) in registered.iter().zip(out) {
                matrices[v] = m;
            }
        }
        (matrices, in_plane, stage_notices)
    })?;
    let (matrices, in_plane, stage_notices) = out;
    notices.extend(stage_notices);
    Ok((matrices, in_plane, notices))
}

/// "volumes 3, 7 and the reference have", for the FSL-14 notice (at most 10 indices listed).
fn describe_volumes(volumes: &[usize], reference: bool) -> String {
    let mut parts = Vec::new();
    if !volumes.is_empty() {
        let mut list: Vec<String> = volumes.iter().take(10).map(|v| v.to_string()).collect();
        if volumes.len() > 10 {
            list.push(format!("... ({} volumes)", volumes.len()));
        }
        parts.push(format!(
            "volume{} {}",
            if volumes.len() > 1 { "s" } else { "" },
            list.join(", ")
        ));
    }
    if reference {
        parts.push("the reference".to_owned());
    }
    let plural = volumes.len() + usize::from(reference) > 1;
    format!(
        "{} {}",
        parts.join(" and "),
        if plural { "have" } else { "has" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FSL-01 reproduced: mcflirt's chains.
    #[test]
    fn stage_one_chains_as_mcflirt() {
        // With a separate reference: 0 from the identity, 1..N-2 chained, N-1 alone.
        assert_eq!(
            chains(5, None, false, true),
            vec![vec![0, 1, 2, 3], vec![4]]
        );
        // Without: r+1 .. N-2 chained, N-1 alone, then r-1 down to 0.
        assert_eq!(
            chains(6, Some(3), false, true),
            vec![vec![4], vec![5], vec![2, 1, 0]]
        );
        assert_eq!(
            chains(7, Some(2), false, true),
            vec![vec![3, 4, 5], vec![6], vec![1, 0]]
        );
        assert_eq!(chains(1, Some(0), false, true), Vec::<Vec<usize>>::new());
        assert_eq!(chains(2, None, false, true), vec![vec![0], vec![1]]);
        // -fudge: every volume alone.
        assert_eq!(chains(3, None, true, true), vec![vec![0], vec![1], vec![2]]);
    }

    /// FSL-01 corrected: the last volume continues its chain.
    #[test]
    fn stage_one_chains_corrected() {
        assert_eq!(chains(5, None, false, false), vec![vec![0, 1, 2, 3, 4]]);
        assert_eq!(
            chains(6, Some(3), false, false),
            vec![vec![4, 5], vec![2, 1, 0]]
        );
        assert_eq!(chains(2, None, false, false), vec![vec![0, 1]]);
        assert_eq!(
            chains(3, None, true, false),
            vec![vec![0], vec![1], vec![2]]
        );
        // The volume the last one continues from.
        assert_eq!(last_predecessor(5, None), Some(3));
        assert_eq!(last_predecessor(6, Some(3)), Some(4));
        assert_eq!(last_predecessor(6, Some(4)), None);
        assert_eq!(last_predecessor(6, Some(5)), None);
        assert_eq!(last_predecessor(1, None), None);
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

    /// A smooth asymmetric blob of `shape` voxels of `voxel` mm, moved by `shift` voxels.
    fn blob(shape: [usize; 3], voxel: [f32; 3], shift: [f32; 3]) -> Volume {
        let mut data = Vec::new();
        for z in 0..shape[2] {
            for y in 0..shape[1] {
                for x in 0..shape[0] {
                    let idx = [x, y, z];
                    let r2: f32 = (0..3)
                        .map(|k| {
                            let c = (shape[k] as f32 - 1.0) / 2.0 + shift[k];
                            let u = (idx[k] as f32 - c) * voxel[k] / [28.0, 34.0, 22.0][k];
                            u * u
                        })
                        .sum();
                    let lump = ((idx[0] as f32 - 6.0 - shift[0]).powi(2)
                        + (idx[1] as f32 - 9.0 - shift[1]).powi(2))
                        / 8.0;
                    data.push(800.0 * (-r2).exp() + 300.0 * (-lump).exp() + 20.0);
                }
            }
        }
        Volume::new(shape, voxel, data)
    }

    fn series() -> Vec<Volume> {
        let (shape, voxel) = ([20, 22, 14], [3.5, 3.5, 4.0]);
        [
            [0.0, 0.0, 0.0],
            [0.4, 0.0, 0.0],
            [0.0, -0.3, 0.2],
            [-0.2, 0.3, 0.0],
        ]
        .iter()
        .map(|&s| blob(shape, voxel, s))
        .collect()
    }

    #[test]
    fn results_do_not_depend_on_the_thread_count() {
        let s = series();
        for reference in [Reference::Index(1), Reference::Volume(s[0].clone())] {
            let run = |n_threads| {
                let params = EstimateParams {
                    n_threads,
                    ..EstimateParams::default()
                };
                estimate(&s, &reference, &params).unwrap().matrices
            };
            assert_eq!(run(1), run(3));
        }
        // The mean pass and the in-plane mode too.
        let run = |n_threads, reference: &Reference, in_plane| {
            let params = EstimateParams {
                n_threads,
                in_plane,
                ..EstimateParams::default()
            };
            estimate(&s, reference, &params).unwrap()
        };
        let mean = Reference::Mean(Box::new(Reference::Index(1)));
        let a = run(1, &mean, InPlane::default());
        assert_eq!(a.matrices, run(4, &mean, InPlane::default()).matrices);
        assert!(a.mean.is_some() && a.reference_index.is_none());
        let a = run(1, &Reference::Index(0), InPlane::Force);
        assert!(a.in_plane);
        assert_eq!(
            a.matrices,
            run(2, &Reference::Index(0), InPlane::Force).matrices
        );
        // In-plane mode moves only rz, tx and ty: the z row and column stay the identity's.
        for m in &a.matrices {
            assert_eq!(
                [m[2][0], m[2][1], m[2][2], m[0][2], m[1][2]],
                [0.0, 0.0, 1.0, 0.0, 0.0]
            );
        }
    }

    /// `specs/mcflirt.md` §12.1: each stage's bins, how mcflirt ends, what the default refuses.
    #[test]
    fn bins_no_stage_can_use() {
        use CostFunction::*;
        use McflirtEnd::*;
        // Truncated toward zero: -bins 7 gives 0 and 1, -bins -9 gives -1 and -2.
        assert_eq!(
            (stage_bins(7, 1), stage_bins(7, 2), stage_bins(7, 4)),
            (0, 1, 1)
        );
        assert_eq!((stage_bins(-9, 1), stage_bins(-9, 3)), (-1, -2));
        assert_eq!(
            mcflirt_end(-8, 3, NormCorr),
            Some(DivisionByZero { bins: -8, stage: 1 })
        );
        assert_eq!(
            mcflirt_end(-4, 2, LeastSquares),
            Some(DivisionByZero { bins: -4, stage: 2 })
        );
        assert_eq!(mcflirt_end(-4, 1, NormCorr), None);
        assert_eq!(mcflirt_end(-3, 4, NormCorr), None);
        assert_eq!(
            mcflirt_end(-16, 3, MutualInfo),
            Some(BadArrayLength {
                bins: -16,
                stage: 1
            })
        );
        assert_eq!(
            mcflirt_end(2, 3, CorrRatio),
            Some(CorrRatioHeap { bins: 2, stage: 1 })
        );
        // corratio meets its empty first stage before the second stage's division by zero.
        assert_eq!(
            mcflirt_end(-4, 3, CorrRatio),
            Some(CorrRatioHeap { bins: -4, stage: 1 })
        );
        assert_eq!(mcflirt_end(8, 3, CorrRatio), None);
        assert_eq!(mcflirt_end(-16, 0, NormCorr), None);
        let bad = BadArrayLength {
            bins: -16,
            stage: 1,
        };
        assert_eq!(bad.mcflirt_lines().len(), 2);
        assert_eq!(bad.defect().id, "FSL-66");
        assert!(bad.error().contains("(FSL-66;"), "{}", bad.error());
        assert_eq!(CorrRatioHeap { bins: 2, stage: 1 }.defect().id, "FSL-21");
        // The default's refusals.
        let id = |b, s, c| refused_bins(b, s, c).map(|r| r.defect.id);
        assert_eq!(id(7, 3, MutualInfo), Some("FSL-21"));
        assert_eq!(id(0, 1, NormMi), Some("FSL-21"));
        assert_eq!(id(-7, 3, Woods), Some("FSL-21"));
        assert_eq!(id(-8, 3, Woods), Some("FSL-66"));
        assert_eq!(id(-1, 3, NormCorr), Some("FSL-66"));
        assert_eq!(id(2, 3, NormCorr), None);
        assert_eq!(id(8, 3, CorrRatio), None);
        assert_eq!(id(4, 0, MutualInfo), None);
        assert_eq!(id(-16, 0, NormCorr), None);
        assert_eq!(
            refused_bins(4, 3, MutualInfo).unwrap().message,
            "-bins 4: the histogram costs need at least 8 bins (the first stage uses -bins / 8 of \
             them; mcflirt computes them with none, FSL-21)"
        );
    }

    /// FSL-21 and FSL-66 through the estimation: refused before any work by default (one
    /// volume included), mcflirt's end or its zero-bin costs reproduced.
    #[test]
    fn bins_in_both_modes() {
        let s = series();
        let reproduce = |bins, cost| EstimateParams {
            bins,
            cost,
            stages: 1,
            upstream_defects: UpstreamDefects::Reproduce,
            ..EstimateParams::default()
        };
        let fix = |bins, cost| EstimateParams {
            upstream_defects: UpstreamDefects::Fix,
            ..reproduce(bins, cost)
        };
        let one = &s[..1];
        for series in [&s[..], one] {
            let e = estimate(
                series,
                &Reference::Index(0),
                &fix(4, CostFunction::MutualInfo),
            );
            assert!(matches!(e, Err(EstimateError::Bins(_))), "{e:?}");
        }
        let e = estimate(&s, &Reference::Index(0), &fix(-16, CostFunction::NormCorr));
        assert!(matches!(e, Err(EstimateError::Bins(r)) if r.defect.id == "FSL-66"));
        let e = estimate(
            &s,
            &Reference::Index(0),
            &reproduce(-8, CostFunction::NormCorr),
        );
        assert!(matches!(
            e,
            Err(EstimateError::McflirtEnds(
                McflirtEnd::DivisionByZero { .. }
            ))
        ));
        // Nothing registered: no stage starts, so nothing ends.
        let est = estimate(
            one,
            &Reference::Index(0),
            &reproduce(-8, CostFunction::NormCorr),
        );
        assert_eq!(est.unwrap().matrices, vec![IDENTITY]);
        // The histogram costs with no bins run, as mcflirt's do, and name FSL-21.
        for cost in [
            CostFunction::MutualInfo,
            CostFunction::NormMi,
            CostFunction::Woods,
        ] {
            let est = estimate(&s, &Reference::Index(0), &reproduce(7, cost)).unwrap();
            assert!(
                est.matrices
                    .iter()
                    .flatten()
                    .flatten()
                    .all(|v| v.is_finite())
            );
            assert!(
                est.notices.iter().any(|n| n.defect.id == "FSL-21"),
                "{cost:?}"
            );
        }
        // -bins -1 with normcorr: no bins anywhere, as mcflirt; FSL-66 named.
        let est = estimate(
            &s,
            &Reference::Index(0),
            &reproduce(-1, CostFunction::NormCorr),
        );
        let est = est.unwrap();
        let plain = estimate(
            &s,
            &Reference::Index(0),
            &reproduce(256, CostFunction::NormCorr),
        );
        assert_eq!(est.matrices, plain.unwrap().matrices);
        assert!(est.notices.iter().any(|n| n.defect.id == "FSL-66"));
    }

    #[test]
    fn shifts_are_recovered_and_the_reference_is_the_identity() {
        let s = series();
        let est = estimate(&s, &Reference::Index(0), &EstimateParams::default()).unwrap();
        assert_eq!(est.matrices[0], IDENTITY);
        // Volume 1's blob moved by +0.4 voxels in x: mapped back by -1.4 mm.
        assert!(
            (est.matrices[1][0][3] + 1.4).abs() < 0.2,
            "{:?}",
            est.matrices[1]
        );
        let err = estimate(&s, &Reference::Index(9), &EstimateParams::default());
        assert!(matches!(
            err,
            Err(EstimateError::BadIndex { index: 9, n: 4 })
        ));
    }

    fn with_mode(mode: UpstreamDefects) -> EstimateParams {
        EstimateParams {
            upstream_defects: mode,
            ..EstimateParams::default()
        }
    }

    fn ids(notices: &[Notice]) -> Vec<(&'static str, UpstreamDefects)> {
        notices.iter().map(|n| (n.defect.id, n.mode)).collect()
    }

    /// FSL-01: the last volume of a chain starts from its predecessor's result when corrected,
    /// from the identity as mcflirt does when reproduced; the other volumes are the same bits.
    #[test]
    fn last_volume_both_modes() {
        let s = series();
        let reference = Reference::Volume(s[0].clone());
        let fixed = estimate(&s, &reference, &with_mode(UpstreamDefects::Fix)).unwrap();
        let mcflirt = estimate(&s, &reference, &with_mode(UpstreamDefects::Reproduce)).unwrap();
        assert_eq!(fixed.matrices[..3], mcflirt.matrices[..3]);
        assert_ne!(fixed.matrices[3], mcflirt.matrices[3]);
        assert_eq!(ids(&fixed.notices), [("FSL-01", UpstreamDefects::Fix)]);
        assert_eq!(
            ids(&mcflirt.notices),
            [("FSL-01", UpstreamDefects::Reproduce)]
        );
        // -fudge: no chain, nothing to correct.
        let fudge = EstimateParams {
            fudge: true,
            ..with_mode(UpstreamDefects::Fix)
        };
        assert!(estimate(&s, &reference, &fudge).unwrap().notices.is_empty());
        // The default reference (volume 2 of 4): the last volume starts its own sweep.
        let r = Reference::Index(2);
        let a = estimate(&s, &r, &with_mode(UpstreamDefects::Fix)).unwrap();
        let b = estimate(&s, &r, &with_mode(UpstreamDefects::Reproduce)).unwrap();
        assert_eq!(a.matrices, b.matrices);
        assert!(a.notices.is_empty() && b.notices.is_empty());
    }

    /// FSL-03: the in-plane mode pads the first stage's grid once when corrected.
    #[test]
    fn in_plane_padding_both_modes() {
        let s = series();
        let run = |mode| {
            let params = EstimateParams {
                in_plane: InPlane::Force,
                fudge: true,
                ..with_mode(mode)
            };
            estimate(&s, &Reference::Index(0), &params).unwrap()
        };
        let (fixed, mcflirt) = (run(UpstreamDefects::Fix), run(UpstreamDefects::Reproduce));
        assert_ne!(fixed.matrices, mcflirt.matrices);
        assert_eq!(ids(&fixed.notices), [("FSL-03", UpstreamDefects::Fix)]);
        assert_eq!(
            ids(&mcflirt.notices),
            [("FSL-03", UpstreamDefects::Reproduce)]
        );
    }

    /// FSL-14: one NaN voxel. mcflirt's matrix for that volume is NaN; corrected, the voxel is
    /// left out and the motion is still found. The other volumes are the same bits.
    #[test]
    fn nan_voxel_both_modes() {
        let mut s = series();
        s[1].data[100] = f32::NAN;
        let r = Reference::Index(0);
        let params = |mode| EstimateParams {
            fudge: true,
            ..with_mode(mode)
        };
        let fixed = estimate(&s, &r, &params(UpstreamDefects::Fix)).unwrap();
        let mcflirt = estimate(&s, &r, &params(UpstreamDefects::Reproduce)).unwrap();
        assert!(
            mcflirt.matrices[1][..3]
                .iter()
                .flatten()
                .all(|v| v.is_nan())
        );
        assert!(fixed.matrices[1].iter().flatten().all(|v| v.is_finite()));
        assert!(
            (fixed.matrices[1][0][3] + 1.4).abs() < 0.2,
            "{:?}",
            fixed.matrices[1]
        );
        assert_eq!(fixed.matrices[2..], mcflirt.matrices[2..]);
        assert_eq!(ids(&fixed.notices), [("FSL-14", UpstreamDefects::Fix)]);
        assert_eq!(
            ids(&mcflirt.notices),
            [("FSL-14", UpstreamDefects::Reproduce)]
        );
    }

    /// FSL-02: the corrected count changes the cost and so the matrices; it cannot be combined
    /// with reproducing mcflirt.
    #[test]
    fn correlation_count() {
        let s = series();
        let r = Reference::Index(0);
        let weights = EstimateParams {
            normcorr_count: NormCorrCount::Weights,
            fudge: true,
            ..EstimateParams::default()
        };
        let mcflirt = EstimateParams {
            fudge: true,
            ..EstimateParams::default()
        };
        let a = estimate(&s, &r, &weights).unwrap();
        let b = estimate(&s, &r, &mcflirt).unwrap();
        assert_ne!(a.matrices, b.matrices);
        assert!(
            (a.matrices[1][0][3] + 1.4).abs() < 0.2,
            "{:?}",
            a.matrices[1]
        );
        let bad = EstimateParams {
            upstream_defects: UpstreamDefects::Reproduce,
            ..weights
        };
        assert!(matches!(
            estimate(&s, &r, &bad),
            Err(EstimateError::CountWithReproduce)
        ));
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
