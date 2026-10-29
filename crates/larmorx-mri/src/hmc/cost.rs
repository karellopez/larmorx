// SPDX-License-Identifier: Apache-2.0
//! The registration cost (`specs/mcflirt.md` §6): a reference grid compared with a test
//! volume at full resolution, sampled at the positions a candidate matrix maps the grid to.
//!
//! The sampling loop reproduces the float32 arithmetic of §6.1–§6.3: a voxel map rounded to
//! float32, positions stepped along each row by float32 additions, only points inside the test
//! volume, trilinear (or, in stage 4, windowed-sinc) interpolation and a linear edge weight. The
//! normalised correlation (§6.4) accumulates its sums in three float32 levels (row, slice, grid)
//! and uses mcflirt's count `N`. Black-box runs settled what the spec leaves open (all
//! *observed* in `specs/mcflirt.md`): square roots in float32, sums in exactly three levels.

use rayon::prelude::*;

use larmorx_core::math;

use super::interp::trilinear_inside;
use super::kernels::sinc_cost;
use super::rigid::{self, Mat4};
use super::volume::Volume;

/// The cost functions (`-cost`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CostFunction {
    /// Normalised correlation as mcflirt computes it (the default).
    #[default]
    NormCorr,
    /// Least squares: the weighted mean squared difference.
    LeastSquares,
    /// One minus the correlation ratio of the test values given the reference bin.
    CorrRatio,
    /// Woods' criterion (ratio image uniformity).
    Woods,
    /// Minus the mutual information.
    MutualInfo,
    /// Minus the normalised mutual information.
    NormMi,
}

impl CostFunction {
    /// The cost named on the command line.
    pub fn from_name(name: &str) -> Option<CostFunction> {
        Some(match name {
            "normcorr" => CostFunction::NormCorr,
            "leastsquares" => CostFunction::LeastSquares,
            "corratio" => CostFunction::CorrRatio,
            "woods" => CostFunction::Woods,
            "mutualinfo" => CostFunction::MutualInfo,
            "normmi" => CostFunction::NormMi,
            _ => return None,
        })
    }

    /// Whether the cost bins the reference into a histogram (`-bins` applies).
    pub fn is_histogram(self) -> bool {
        matches!(
            self,
            CostFunction::CorrRatio
                | CostFunction::Woods
                | CostFunction::MutualInfo
                | CostFunction::NormMi
        )
    }

    /// The name of the cost.
    pub fn name(self) -> &'static str {
        match self {
            CostFunction::NormCorr => "normcorr",
            CostFunction::LeastSquares => "leastsquares",
            CostFunction::CorrRatio => "corratio",
            CostFunction::Woods => "woods",
            CostFunction::MutualInfo => "mutualinfo",
            CostFunction::NormMi => "normmi",
        }
    }
}

/// The voxel-to-voxel map of a candidate matrix `m` (test FSL-mm → reference FSL-mm):
/// `B = Dv⁻¹ · M⁻¹ · Dg` in double, each entry rounded to float32 (§6.1). Row `k` gives test
/// voxel coordinate `k` of a grid point `(x, y, z, 1)`.
pub fn voxel_map(m: &Mat4, test_voxel: [f32; 3], grid_voxel: [f32; 3]) -> [[f32; 4]; 3] {
    let dv_inv = rigid::diagonal(test_voxel.map(|d| 1.0 / f64::from(d)));
    let dg = rigid::diagonal(grid_voxel.map(f64::from));
    let b = rigid::mul(&rigid::mul(&dv_inv, &rigid::inverse(m)), &dg);
    [0, 1, 2].map(|i| [0, 1, 2, 3].map(|j| b[i][j] as f32))
}

/// How the test volume is interpolated in the cost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CostInterpolation {
    Trilinear,
    /// Stage 4 (§6.5): Hanning-windowed sinc; `background` where the weights vanish.
    Sinc {
        background: f32,
    },
}

/// The count `N` of the weighted normalised correlation (§6.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NormCorrCount {
    /// mcflirt's count: a float32 running sum of running sums of the row weights, many orders
    /// of magnitude larger than the number of points, which makes the correlation effectively
    /// uncentred (FSL-02). The default, pending a decision (D13 item 4).
    #[default]
    Mcflirt,
    /// The sum of the weights: the weighted Pearson correlation (FSL-02 corrected).
    Weights,
}

impl NormCorrCount {
    /// `mcflirt` or `weights`.
    pub fn from_name(name: &str) -> Option<NormCorrCount> {
        match name {
            "mcflirt" => Some(NormCorrCount::Mcflirt),
            "weights" => Some(NormCorrCount::Weights),
            _ => None,
        }
    }
}

/// What the cost needs about a test volume, computed once per volume and stage.
pub struct TestVolume<'a> {
    pub vol: &'a Volume,
    /// `n_k − 1.0001` in float32: the largest admissible position per axis.
    pub bound: [f32; 3],
    /// The edge-weighting width per axis in voxels (`smooth / d_k`); 0 turns weighting off.
    pub sigma: [f32; 3],
    pub interpolation: CostInterpolation,
    /// Leave out points whose reference or interpolated test value is NaN or infinite
    /// (FSL-14 corrected; set only when the volume or the grid has such values).
    pub skip_non_finite: bool,
    /// The count of the normalised correlation.
    pub count: NormCorrCount,
    /// The test bins of the mutual-information costs ([`TestBins`]), set by the caller for
    /// them; computed from the whole volume once, as they do not change between evaluations.
    pub test_bins: Option<TestBins>,
}

impl<'a> TestVolume<'a> {
    /// A test volume with edge-weighting width `smooth` mm and trilinear interpolation.
    pub fn new(vol: &'a Volume, smooth: f32) -> TestVolume<'a> {
        TestVolume {
            vol,
            bound: vol.shape.map(|n| (n as f64 - 1.0001) as f32),
            sigma: vol.voxel_size.map(|d| smooth / d),
            interpolation: CostInterpolation::Trilinear,
            skip_non_finite: false,
            count: NormCorrCount::Mcflirt,
            test_bins: None,
        }
    }

    /// Whether every point is weighted (`smooth > 0`).
    pub fn weighted(&self) -> bool {
        self.sigma.iter().all(|&s| s > 0.0)
    }

    #[inline(always)]
    fn inside(&self, p: [f32; 3]) -> bool {
        p[0] >= 0.0
            && p[0] <= self.bound[0]
            && p[1] >= 0.0
            && p[1] <= self.bound[1]
            && p[2] >= 0.0
            && p[2] <= self.bound[2]
    }

    /// The edge weight at `p` (§6.3): per axis `p/σ` within `σ` of 0, `(b − p)/σ` within `σ`
    /// of the bound, else 1; the product of the three in float32.
    #[inline(always)]
    fn weight(&self, p: [f32; 3]) -> f32 {
        let axis = |k: usize| {
            let s = self.sigma[k];
            if p[k] < s {
                p[k] / s
            } else if self.bound[k] - p[k] < s {
                (self.bound[k] - p[k]) / s
            } else {
                1.0
            }
        };
        let w = axis(0) * axis(1) * axis(2);
        if w < 0.0 { 0.0 } else { w }
    }
}

/// The admissible grid points of one row (§6.1): the x range where the row's positions
/// `row0 + x·step` lie inside the test volume, found analytically and intersected with the
/// grid. `None` if empty.
#[inline]
fn row_range(row0: [f32; 3], step: [f32; 3], bound: [f32; 3], nx: usize) -> Option<(usize, usize)> {
    let mut lo = 0.0f64;
    let mut hi = nx as f64 - 1.0;
    for k in 0..3 {
        let (r, s, b) = (f64::from(row0[k]), f64::from(step[k]), f64::from(bound[k]));
        if s == 0.0 {
            if !(r >= 0.0 && r <= b) {
                return None;
            }
            continue;
        }
        let (a0, a1) = ((0.0 - r) / s, (b - r) / s);
        let (l, h) = if s > 0.0 { (a0, a1) } else { (a1, a0) };
        lo = lo.max(l);
        hi = hi.min(h);
    }
    // NaN bounds fail these comparisons and give an empty range.
    let (lo, hi) = (lo.ceil(), hi.floor());
    if lo <= hi && lo >= 0.0 {
        Some((lo as usize, hi as usize))
    } else {
        None
    }
}

/// Receives the contributing points of a sampling pass, row by row.
pub trait Accumulator: Send {
    /// One contributing point: grid value `g`, interpolated test value `v`, edge weight `w`.
    fn point(&mut self, g: f32, v: f32, w: f32);
    /// The end of a row (every row of the grid, also rows without points).
    fn end_row(&mut self);
}

/// Visits the contributing points of grid slice `z` (§6.1), interpolating the test volume
/// with `value` and weighting with `weight` (both inlined, so the loop has no dispatch).
#[inline(always)]
fn sample_slice<A: Accumulator, V: Fn([f32; 3]) -> f32, W: Fn([f32; 3]) -> f32>(
    grid: &Volume,
    test: &TestVolume<'_>,
    b: &[[f32; 4]; 3],
    z: usize,
    acc: &mut A,
    value: &V,
    weight: &W,
) {
    let [gx, gy, _] = grid.shape;
    let step = [b[0][0], b[1][0], b[2][0]];
    let zf = z as f32;
    for y in 0..gy {
        let yf = y as f32;
        let row0 = [0, 1, 2].map(|k| yf * b[k][1] + zf * b[k][2] + b[k][3]);
        if let Some((x0, x1)) = row_range(row0, step, test.bound, gx) {
            let x0f = x0 as f32;
            let mut p = [0, 1, 2].map(|k| row0[k] + x0f * step[k]);
            let grow = &grid.data[gx * (y + gy * z)..gx * (y + gy * z + 1)];
            let mut x = x0;
            // Leading points that fail are dropped; the row ends at the first later failure.
            while x <= x1 && !test.inside(p) {
                p = [p[0] + step[0], p[1] + step[1], p[2] + step[2]];
                x += 1;
            }
            if test.skip_non_finite {
                while x <= x1 && test.inside(p) {
                    let (g, v) = (grow[x], value(p));
                    if g.is_finite() && v.is_finite() {
                        acc.point(g, v, weight(p));
                    }
                    p = [p[0] + step[0], p[1] + step[1], p[2] + step[2]];
                    x += 1;
                }
            } else {
                while x <= x1 && test.inside(p) {
                    acc.point(grow[x], value(p), weight(p));
                    p = [p[0] + step[0], p[1] + step[1], p[2] + step[2]];
                    x += 1;
                }
            }
        }
        acc.end_row();
    }
}

/// Samples the whole grid: one accumulator per slice (made by `make`), in slice order. With
/// `parallel`, the slices are sampled in parallel; each has its own accumulator, so the result
/// is the same.
pub fn sample<A: Accumulator>(
    grid: &Volume,
    test: &TestVolume<'_>,
    b: &[[f32; 4]; 3],
    parallel: bool,
    make: impl Fn() -> A + Sync,
) -> Vec<A> {
    let nz = grid.shape[2];
    let one = |z: usize| {
        let mut acc = make();
        sample_into(grid, test, b, z, &mut acc);
        acc
    };
    if parallel {
        (0..nz).into_par_iter().map(one).collect()
    } else {
        (0..nz).map(one).collect()
    }
}

/// Visits the contributing points of grid slice `z` into `acc` (§6.1), with the test volume's
/// interpolation and edge weighting.
fn sample_into<A: Accumulator>(
    grid: &Volume,
    test: &TestVolume<'_>,
    b: &[[f32; 4]; 3],
    z: usize,
    acc: &mut A,
) {
    let vol = test.vol;
    let trilinear = |p: [f32; 3]| trilinear_inside(vol, p[0], p[1], p[2]);
    let weighted = |p: [f32; 3]| test.weight(p);
    let unweighted = |_: [f32; 3]| 1.0f32;
    match (test.interpolation, test.weighted()) {
        (CostInterpolation::Trilinear, true) => {
            sample_slice(grid, test, b, z, acc, &trilinear, &weighted);
        }
        (CostInterpolation::Trilinear, false) => {
            sample_slice(grid, test, b, z, acc, &trilinear, &unweighted);
        }
        (CostInterpolation::Sinc { background }, w) => {
            let sinc = |p: [f32; 3]| sinc_cost(vol, p, background);
            if w {
                sample_slice(grid, test, b, z, acc, &sinc, &weighted);
            } else {
                sample_slice(grid, test, b, z, acc, &sinc, &unweighted);
            }
        }
    }
}

/// The five sums of the correlation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Sums {
    sx: f32,
    sy: f32,
    sxx: f32,
    syy: f32,
    sxy: f32,
}

impl Sums {
    #[inline(always)]
    fn add(&mut self, o: &Sums) {
        self.sx += o.sx;
        self.sy += o.sy;
        self.sxx += o.sxx;
        self.syy += o.syy;
        self.sxy += o.sxy;
    }
}

/// The correlation sums of one slice (§6.4): float32 running sums over each row, the row
/// totals summed over the slice; the total weight of every row; the number of points.
#[derive(Default)]
struct CorrSlice {
    slice: Sums,
    row: Sums,
    row_weight: f32,
    row_weights: Vec<f32>,
    points: u32,
}

impl Accumulator for CorrSlice {
    #[inline(always)]
    fn point(&mut self, g: f32, v: f32, w: f32) {
        let wg = w * g;
        let wv = w * v;
        self.row.sx += wg;
        self.row.sy += wv;
        self.row.sxx += wg * g;
        self.row.syy += wv * v;
        self.row.sxy += wg * v;
        self.row_weight += w;
        self.points += 1;
    }

    fn end_row(&mut self) {
        let row = std::mem::take(&mut self.row);
        self.slice.add(&row);
        self.row_weights.push(std::mem::take(&mut self.row_weight));
    }
}

/// The normalised-correlation cost `1 − |r|` (§6.4). With weighting (`smooth > 0`), the
/// correlation uses mcflirt's count `N` (a float32 running sum of running sums of the row
/// weights), or with [`NormCorrCount::Weights`] the sum of the weights (FSL-02 corrected);
/// without weighting, the number of contributing points (Pearson's correlation).
pub fn normcorr(grid: &Volume, test: &TestVolume<'_>, b: &[[f32; 4]; 3], parallel: bool) -> f32 {
    let slices = sample(grid, test, b, parallel, CorrSlice::default);
    let mut total = Sums::default();
    let (mut c, mut a, mut n) = (0.0f32, 0.0f32, 0.0f32);
    let mut points = 0u32;
    for s in &slices {
        for &w in &s.row_weights {
            c += w;
            a += c;
        }
        n += a;
        total.add(&s.slice);
        points += s.points;
    }
    let r = if test.weighted() {
        match test.count {
            NormCorrCount::Mcflirt => correlation_weighted(&total, n),
            // `c` is the float32 running total of every row's weight: Σ w.
            NormCorrCount::Weights => correlation_weighted(&total, c),
        }
    } else {
        correlation_counted(&total, points as f32)
    };
    1.0 - r.abs()
}

/// The correlation with mcflirt's count `N`: the divisions by `N − 1` in double, the
/// products and the second divisions in float32, `cov`, `varx`, `vary` and the square roots
/// in float32.
fn correlation_weighted(s: &Sums, n: f32) -> f32 {
    if n.partial_cmp(&2.0) != Some(std::cmp::Ordering::Greater) {
        return 0.0;
    }
    let n1 = f64::from(n) - 1.0;
    let nn = n * n;
    let cov = (f64::from(s.sxy) / n1 - f64::from((s.sx * s.sy) / nn)) as f32;
    let varx = (f64::from(s.sxx) / n1 - f64::from((s.sx * s.sx) / nn)) as f32;
    let vary = (f64::from(s.syy) / n1 - f64::from((s.sy * s.sy) / nn)) as f32;
    if varx > 0.0 && vary > 0.0 {
        cov / varx.sqrt() / vary.sqrt()
    } else {
        0.0
    }
}

/// The unweighted correlation (`-smooth 0`): Pearson's, in float32.
fn correlation_counted(s: &Sums, n: f32) -> f32 {
    if n.partial_cmp(&2.0) != Some(std::cmp::Ordering::Greater) {
        return 0.0;
    }
    let n1 = n - 1.0;
    let nn = n * n;
    let cov = s.sxy / n1 - (s.sx * s.sy) / nn;
    let varx = s.sxx / n1 - (s.sx * s.sx) / nn;
    let vary = s.syy / n1 - (s.sy * s.sy) / nn;
    if varx > 0.0 && vary > 0.0 {
        cov / varx.sqrt() / vary.sqrt()
    } else {
        0.0
    }
}

/// Least squares (appendix A): the weighted mean of `(g − v)²`.
#[derive(Default)]
struct SquaresSlice {
    sum: f64,
    weight: f64,
}

impl Accumulator for SquaresSlice {
    #[inline(always)]
    fn point(&mut self, g: f32, v: f32, w: f32) {
        let d = f64::from(g) - f64::from(v);
        self.sum += f64::from(w) * d * d;
        self.weight += f64::from(w);
    }
    fn end_row(&mut self) {}
}

/// The least-squares cost; an empty overlap gives `(max − min)²` over both images.
pub fn least_squares(
    grid: &Volume,
    test: &TestVolume<'_>,
    b: &[[f32; 4]; 3],
    parallel: bool,
) -> f32 {
    let slices = sample(grid, test, b, parallel, SquaresSlice::default);
    let (sum, weight) = slices
        .iter()
        .fold((0.0, 0.0), |(s, w), x| (s + x.sum, w + x.weight));
    if weight > 0.0 {
        (sum / weight) as f32
    } else {
        let (lo, hi) = [grid, test.vol]
            .iter()
            .flat_map(|v| v.data.iter())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &x| {
                (l.min(x), h.max(x))
            });
        (hi - lo) * (hi - lo)
    }
}

/// Reference bins (appendix A): `⌊(g − g_min)·B/(g_max − g_min)⌋` clamped to `0 … B−1`,
/// returned as a grid of bin indices (the histogram costs sample it in place of the values).
pub fn bin_grid(grid: &Volume, bins: usize) -> Volume {
    let (lo, hi) = min_max(&grid.data);
    let range = if hi - lo == 0.0 { 1.0 } else { hi - lo };
    let b = bins.max(1);
    let data = grid
        .data
        .iter()
        .map(|&g| {
            let k = ((g - lo) * b as f32 / range).floor();
            (k.max(0.0) as usize).min(b - 1) as f32
        })
        .collect();
    Volume::new(grid.shape, grid.voxel_size, data)
}

/// The smallest and largest finite values.
pub fn min_max(data: &[f32]) -> (f32, f32) {
    data.iter()
        .filter(|v| v.is_finite())
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &x| {
            (l.min(x), h.max(x))
        })
}

/// Per-bin moments of the test values: weighted `[Σw, Σwv, Σwv²]` and unweighted
/// `[n, Σv, Σv²]`.
struct BinSlice {
    weighted: Vec<[f64; 3]>,
    plain: Vec<[f64; 3]>,
}

impl BinSlice {
    fn new(bins: usize) -> BinSlice {
        BinSlice {
            weighted: vec![[0.0; 3]; bins],
            plain: vec![[0.0; 3]; bins],
        }
    }
}

impl Accumulator for BinSlice {
    #[inline(always)]
    fn point(&mut self, g: f32, v: f32, w: f32) {
        let k = (g as usize).min(self.weighted.len() - 1);
        let (w, v) = (f64::from(w), f64::from(v));
        let s = &mut self.weighted[k];
        s[0] += w;
        s[1] += w * v;
        s[2] += w * v * v;
        let s = &mut self.plain[k];
        s[0] += 1.0;
        s[1] += v;
        s[2] += v * v;
    }
    fn end_row(&mut self) {}
}

fn merge_bins(slices: Vec<BinSlice>, bins: usize) -> BinSlice {
    let mut out = BinSlice::new(bins);
    for s in slices {
        for k in 0..bins {
            for j in 0..3 {
                out.weighted[k][j] += s.weighted[k][j];
                out.plain[k][j] += s.plain[k][j];
            }
        }
    }
    out
}

/// One minus the correlation ratio (appendix A): `Σ_b n_b·var_b / (n·var)` over bins with
/// `n_b > 2`, weighted; 0 if the total variance is not positive. `bin_grid` holds bin indices.
pub fn corr_ratio(
    bin_grid: &Volume,
    test: &TestVolume<'_>,
    b: &[[f32; 4]; 3],
    bins: usize,
    parallel: bool,
) -> f32 {
    let m = merge_bins(
        sample(bin_grid, test, b, parallel, || BinSlice::new(bins)),
        bins,
    );
    let (mut n, mut s, mut ss) = (0.0, 0.0, 0.0);
    for st in &m.weighted {
        n += st[0];
        s += st[1];
        ss += st[2];
    }
    if n <= 1.0 {
        return 1.0;
    }
    let var = (ss - s * s / n) / (n - 1.0);
    if var <= 0.0 {
        return 0.0;
    }
    let within: f64 = m
        .weighted
        .iter()
        .filter(|st| st[0] > 2.0)
        .map(|st| st[0] * (st[2] - st[1] * st[1] / st[0]) / (st[0] - 1.0))
        .sum();
    (within / (n * var)) as f32
}

/// Woods' criterion (appendix A), unweighted: `Σ_b n_b²·σ_b/μ_b / n` (`σ_b` alone where the bin
/// mean is not positive); `1e10` for an empty overlap.
pub fn woods(
    bin_grid: &Volume,
    test: &TestVolume<'_>,
    b: &[[f32; 4]; 3],
    bins: usize,
    parallel: bool,
) -> f32 {
    let m = merge_bins(
        sample(bin_grid, test, b, parallel, || BinSlice::new(bins)),
        bins,
    );
    let n: f64 = m.plain.iter().map(|st| st[0]).sum();
    if n <= 0.0 {
        return 1e10;
    }
    let mut cost = 0.0;
    for st in &m.plain {
        let nb = st[0];
        if nb > 1.0 {
            let mean = st[1] / nb;
            let sd = ((st[2] - st[1] * st[1] / nb) / (nb - 1.0)).max(0.0).sqrt();
            cost += nb * nb * if mean > 0.0 { sd / mean } else { sd };
        }
    }
    (cost / n) as f32
}

/// x86-64's conversion of a float32 to a 64-bit integer (`cvttss2si`): truncation toward zero,
/// and `i64::MIN` for NaN and values outside the 64-bit range (appendix B: the test bin).
#[inline(always)]
fn x86_i64(v: f32) -> i64 {
    if v.is_nan() || !(-9.223_372e18..9.223_372e18).contains(&v) {
        i64::MIN
    } else {
        v as i64
    }
}

/// x86-64's conversion of a float32 to a 32-bit integer: truncation toward zero, `i32::MIN` for
/// NaN and values outside the 32-bit range (appendix B: the reference bins, the fraction).
#[inline(always)]
fn x86_i32(v: f32) -> i32 {
    if v.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&v) {
        i32::MIN
    } else {
        v as i32
    }
}

/// The reference bins of the mutual-information costs (appendix B), once per stage: with
/// `r_min`, `r_max` the grid's smallest and largest finite values (`r_max` increased by 1 when
/// they are equal), `a1 = B/(r_max − r_min)`, `a0 = −r_min·B/(r_max − r_min)` in float32, each
/// point's bin `trunc(v·a1 + a0)` clamped to `0 … B−1`. Returned as a grid of bin indices,
/// which the costs sample in place of the values. With `skip_non_finite` (FSL-14 corrected), a
/// point whose value is NaN or infinite gets a NaN bin, so the sampling leaves it out.
pub fn bin_grid_mi(grid: &Volume, bins: usize, skip_non_finite: bool) -> Volume {
    let (lo, mut hi) = min_max(&grid.data);
    if hi - lo == 0.0 {
        hi += 1.0;
    }
    let b = bins.max(1) as f32;
    let range = hi - lo;
    let a1 = b / range;
    let a0 = (-lo * b) / range;
    let top = bins.max(1) as i32 - 1;
    let data = grid
        .data
        .par_iter()
        .with_min_len(1 << 14)
        .map(|&v| {
            if skip_non_finite && !v.is_finite() {
                f32::NAN
            } else {
                x86_i32(v * a1 + a0).clamp(0, top) as f32
            }
        })
        .collect();
    Volume::new(grid.shape, grid.voxel_size, data)
}

/// The test bins of the mutual-information costs (appendix B): with `t_min`, `t_max` the
/// smallest and largest finite values of the whole test volume (not only the sampled points),
/// `b1 = B/(t_max − t_min)` and `b0 = −t_min·B/(t_max − t_min)` in float32. Not guarded: a
/// constant volume gives infinite or NaN factors, as in mcflirt, and every sample then falls
/// into bin 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestBins {
    pub b1: f32,
    pub b0: f32,
}

impl TestBins {
    pub fn new(vol: &Volume, bins: usize) -> TestBins {
        let (lo, hi) = min_max(&vol.data);
        let b = bins.max(1) as f32;
        let range = hi - lo;
        TestBins {
            b1: b / range,
            b0: (-lo * b) / range,
        }
    }
}

/// One sample's part of the histograms (appendix B): its reference bin `a`, its test bin `c`
/// and the neighbours `cp`, `cm` (an index outside `0 … B−1` only with a zero weight), the
/// products of the edge weight `g` with the fuzzy weights, and `g`.
#[derive(Clone, Copy, Debug, Default)]
struct Contribution {
    a: u32,
    c: i64,
    cp: i64,
    cm: i64,
    gwc: f32,
    gwp: f32,
    gwm: f32,
    g: f32,
}

/// The fuzzy fraction φ (fixed; mcflirt has no option for it).
const FUZZY: f32 = 0.5;

/// The contribution of a sample with reference bin `a`, interpolated test value `y` and edge
/// weight `g` (appendix B, "Bins" and "Fuzzy binning"), every step in float32 as described.
#[inline(always)]
fn contribution(a: f32, y: f32, g: f32, bins: i64, t: TestBins) -> Contribution {
    let u = y * t.b1 + t.b0;
    let mut c = x86_i64(u);
    let mut cp = c.wrapping_add(1);
    let mut cm = c.wrapping_sub(1);
    if c >= bins {
        c = bins - 1;
        cp = c;
    }
    if c < 0 {
        c = 0;
        cm = c;
    }
    if cp >= bins {
        cp = bins - 1;
    }
    if cm < 0 {
        cm = 0;
    }
    let f = (u - x86_i32(u) as f32).abs();
    let (wc, wp, wm) = if f < FUZZY {
        let wc = 0.5 + 0.5 * (f / FUZZY);
        (wc, 0.0, 1.0 - wc)
    } else if f > 1.0 - FUZZY {
        let wc = 0.5 + 0.5 * ((1.0 - f) / FUZZY);
        (wc, 1.0 - wc, 0.0)
    } else {
        (1.0, 0.0, 0.0)
    };
    Contribution {
        a: a as u32,
        c,
        cp,
        cm,
        gwc: g * wc,
        gwp: g * wp,
        gwm: g * wm,
        g,
    }
}

/// The joint histogram and the two marginals (appendix B, "Histograms"): float32 cells, each
/// contribution added in the sampling order.
struct MiHistograms {
    joint: Vec<f32>,
    test: Vec<f32>,
    reference: Vec<f32>,
    bins: usize,
    test_bins: TestBins,
}

impl MiHistograms {
    fn new(bins: usize, test_bins: TestBins) -> MiHistograms {
        MiHistograms {
            joint: vec![0.0; bins * bins],
            test: vec![0.0; bins],
            reference: vec![0.0; bins],
            bins,
            test_bins,
        }
    }

    /// Adds one contribution: the joint cell and the test marginal for `c`, `cp` and `cm` in
    /// that order (each a separate float32 addition, also where two indices coincide), then
    /// `g` to the reference marginal. A zero product is skipped: adding +0 to a cell, which is
    /// never −0, leaves it as it is.
    #[inline(always)]
    fn add(&mut self, k: &Contribution) {
        let row = k.a as usize * self.bins;
        for (i, x) in [(k.c, k.gwc), (k.cp, k.gwp), (k.cm, k.gwm)] {
            // An index outside the bins comes only with a zero weight (a NaN or an out-of-range
            // test value, which mcflirt's conversion maps to the most negative integer).
            if x != 0.0 && (0..self.bins as i64).contains(&i) {
                self.joint[row + i as usize] += x;
                self.test[i as usize] += x;
            }
        }
        self.reference[k.a as usize] += k.g;
    }
}

impl Accumulator for MiHistograms {
    #[inline(always)]
    fn point(&mut self, g: f32, v: f32, w: f32) {
        let k = contribution(g, v, w, self.bins as i64, self.test_bins);
        self.add(&k);
    }
    fn end_row(&mut self) {}
}

/// The contributions of one slice, in sampling order (the parallel sampling of stage 1).
struct MiRecord {
    points: Vec<Contribution>,
    bins: i64,
    test_bins: TestBins,
}

impl Accumulator for MiRecord {
    #[inline(always)]
    fn point(&mut self, g: f32, v: f32, w: f32) {
        self.points
            .push(contribution(g, v, w, self.bins, self.test_bins));
    }
    fn end_row(&mut self) {}
}

/// `Σ −p·ln p` over the cells with a positive count, `p = count/n`, in index order, each term
/// rounded to float32 (single-precision logarithm), and the sum of those counts.
fn entropy(cells: &[f32], n: f32) -> (f32, f32) {
    let (mut h, mut total) = (0.0f32, 0.0f32);
    for &c in cells {
        if c > 0.0 {
            let p = c / n;
            h += -p * math::logf(p);
            total += c;
        }
    }
    (h, total)
}

/// The mutual-information costs exactly as `specs/mcflirt.md` appendix B describes them:
/// `−MI` (`mutualinfo`), or `−NMI` with `NMI = (H_ref + H_test)/H_joint` (`normmi`; 0 when
/// `|H_joint| < 1e−9`). `bin_grid` holds the reference bins ([`bin_grid_mi`]) and `test` its
/// [`TestBins`]. The float32 histograms are filled in the sampling order (z, y, x): directly
/// when `parallel` is false (stages 2–4, whose volumes run in parallel), and with `parallel`
/// (stage 1) from each slice's contributions computed in parallel and then added slice by slice,
/// so the result is the same bits at any thread count.
pub fn mutual_info(
    bin_grid: &Volume,
    test: &TestVolume<'_>,
    b: &[[f32; 4]; 3],
    bins: usize,
    normalised: bool,
    parallel: bool,
) -> f32 {
    let bins = bins.max(1);
    let test_bins = test
        .test_bins
        .unwrap_or_else(|| TestBins::new(test.vol, bins));
    let mut h = MiHistograms::new(bins, test_bins);
    if parallel {
        let make = || MiRecord {
            points: Vec::new(),
            bins: bins as i64,
            test_bins,
        };
        for slice in sample(bin_grid, test, b, true, make) {
            for k in &slice.points {
                h.add(k);
            }
        }
    } else {
        for z in 0..bin_grid.shape[2] {
            sample_into(bin_grid, test, b, z, &mut h);
        }
    }
    // N: every point of the reference grid, sampled or not, as float32.
    let n = bin_grid.len() as f32;
    let (mut h_joint, _) = entropy(&h.joint, n);
    let (mut h_ref, _) = entropy(&h.reference, n);
    let (mut h_test, overlap) = entropy(&h.test, n);
    if overlap > 0.0 {
        // The entropies of N points corrected to the `overlap` points that overlap.
        let rho = n / overlap;
        let ln_rho = math::logf(rho);
        h_joint = rho * h_joint - ln_rho;
        h_ref = rho * h_ref - ln_rho;
        h_test = rho * h_test - ln_rho;
    } else {
        // No overlap: the values of a bad registration.
        let ln_b = math::log(bins as f64);
        h_joint = (2.0 * ln_b) as f32;
        h_ref = ln_b as f32;
        h_test = ln_b as f32;
    }
    if normalised {
        let nmi = if f64::from(h_joint).abs() < 1e-9 {
            0.0
        } else {
            (h_ref + h_test) / h_joint
        };
        -nmi
    } else {
        -((h_ref + h_test) - h_joint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hmc::rigid::IDENTITY;

    fn blob(shape: [usize; 3], voxel: [f32; 3]) -> Volume {
        let mut data = Vec::new();
        for z in 0..shape[2] {
            for y in 0..shape[1] {
                for x in 0..shape[0] {
                    let idx = [x, y, z];
                    let d: f32 = (0..3)
                        .map(|k| {
                            let u = idx[k] as f32 - (shape[k] as f32 - 1.0) / 2.0;
                            u * u
                        })
                        .sum();
                    data.push(1000.0 * (-d / 30.0).exp() + 10.0);
                }
            }
        }
        Volume::new(shape, voxel, data)
    }

    #[test]
    fn identical_images_correlate_best() {
        let v = blob([20, 22, 16], [3.0, 3.0, 4.0]);
        let grid = crate::hmc::grid::subsample(&v, 4.0);
        let test = TestVolume::new(&v, 1.0);
        let at = |m: &Mat4, par| {
            normcorr(
                &grid,
                &test,
                &voxel_map(m, v.voxel_size, grid.voxel_size),
                par,
            )
        };
        let c0 = at(&IDENTITY, false);
        let mut shifted = IDENTITY;
        shifted[0][3] = 2.0;
        assert!(c0 < at(&shifted, false), "{c0} {}", at(&shifted, false));
        assert!((0.0..0.01).contains(&c0));
        // Parallel slices give the same bits.
        assert_eq!(at(&shifted, true).to_bits(), at(&shifted, false).to_bits());
        // The other costs prefer the identity too.
        let b0 = voxel_map(&IDENTITY, v.voxel_size, grid.voxel_size);
        let b1 = voxel_map(&shifted, v.voxel_size, grid.voxel_size);
        assert!(least_squares(&grid, &test, &b0, false) < least_squares(&grid, &test, &b1, false));
        let bg = bin_grid(&grid, 64);
        assert!(corr_ratio(&bg, &test, &b0, 64, false) < corr_ratio(&bg, &test, &b1, 64, false));
        assert!(woods(&bg, &test, &b0, 64, false) < woods(&bg, &test, &b1, 64, false));
        let bg = bin_grid_mi(&grid, 64, false);
        for normalised in [false, true] {
            let m0 = mutual_info(&bg, &test, &b0, 64, normalised, false);
            let m1 = mutual_info(&bg, &test, &b1, 64, normalised, false);
            assert!(m0 < m1, "{normalised}: {m0} {m1}");
            // Stage 1's parallel path gives the same bits, at any thread count.
            for threads in [1, 3, 8] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                let par = pool.install(|| mutual_info(&bg, &test, &b1, 64, normalised, true));
                assert_eq!(par.to_bits(), m1.to_bits(), "{normalised} {threads}");
            }
        }
    }

    /// Appendix B's bins and fuzzy weights, by hand: four bins over the test range [0, 4], so
    /// `u = y`.
    #[test]
    fn mi_contributions_by_hand() {
        let t = TestBins { b1: 1.0, b0: -0.0 };
        let k = |y: f32| {
            let k = contribution(2.0, y, 0.5, 4, t);
            (k.c, k.cp, k.cm, k.gwc, k.gwp, k.gwm)
        };
        // Near the lower edge: shared with the bin below.
        assert_eq!(k(1.25), (1, 2, 0, 0.375, 0.0, 0.125));
        // Near the upper edge: shared with the bin above.
        assert_eq!(k(2.75), (2, 3, 1, 0.375, 0.125, 0.0));
        // At the centre: all in the bin.
        assert_eq!(k(2.5), (2, 3, 1, 0.5, 0.0, 0.0));
        // The top of the range: bin B−1 twice (the neighbour below computed before the clamp).
        assert_eq!(k(4.0), (3, 3, 3, 0.25, 0.0, 0.25));
        // The bottom: bin 0 twice.
        assert_eq!(k(0.0), (0, 1, 0, 0.25, 0.0, 0.25));
        // NaN (a constant test volume's u): all in bin 0, the neighbour above unused.
        let n = contribution(1.0, f32::NAN, 0.5, 4, t);
        assert_eq!((n.c, n.cm, n.gwc, n.gwp, n.gwm), (0, 0, 0.5, 0.0, 0.0));
        assert_eq!(n.cp, i64::MIN + 1);
        let mut h = MiHistograms::new(4, t);
        h.add(&n);
        assert_eq!((h.joint[4], h.test[0], h.reference[1]), (0.5, 0.5, 0.5));
        // Reference bins: trunc(v·a1 + a0), clamped; equal extremes widened by 1.
        let g = Volume::new([4, 1, 1], [1.0; 3], vec![10.0, 12.0, 14.0, f32::NAN]);
        assert_eq!(bin_grid_mi(&g, 4, false).data, [0.0, 2.0, 3.0, 0.0]);
        assert!(bin_grid_mi(&g, 4, true).data[3].is_nan());
        let flat = Volume::new([2, 1, 1], [1.0; 3], vec![5.0, 5.0]);
        assert_eq!(bin_grid_mi(&flat, 4, false).data, [0.0, 0.0]);
    }

    /// The entropies and the costs (appendix B): no overlap gives the values of a bad
    /// registration; a constant test volume puts every sample in bin 0.
    #[test]
    fn mi_costs_at_the_extremes() {
        let v = blob([12, 12, 8], [3.0, 3.0, 3.0]);
        let grid = crate::hmc::grid::subsample(&v, 4.0);
        let bg = bin_grid_mi(&grid, 16, false);
        let test = TestVolume::new(&v, 1.0);
        let mut far = IDENTITY;
        far[0][3] = 1000.0;
        let b = voxel_map(&far, v.voxel_size, grid.voxel_size);
        let ln16 = 16f64.ln();
        let mi = mutual_info(&bg, &test, &b, 16, false, false);
        assert_eq!(mi, -((ln16 as f32 + ln16 as f32) - (2.0 * ln16) as f32));
        let nmi = mutual_info(&bg, &test, &b, 16, true, false);
        assert_eq!(nmi, -((ln16 as f32 + ln16 as f32) / (2.0 * ln16) as f32));
        let flat = Volume::new(v.shape, v.voxel_size, vec![7.0; v.len()]);
        let test = TestVolume::new(&flat, 1.0);
        let b = voxel_map(&IDENTITY, v.voxel_size, grid.voxel_size);
        let c = mutual_info(&bg, &test, &b, 16, true, false);
        assert!(c.is_finite() && c < 0.0, "{c}");
    }

    #[test]
    fn row_ranges() {
        // A row stepping by 0.5 voxels from -1: positions inside [0, 8.9999] for x in 2..=19.
        let r = row_range([-1.0, 1.0, 1.0], [0.5, 0.0, 0.0], [8.9999, 5.0, 5.0], 40);
        assert_eq!(r, Some((2, 19)));
        // Backwards.
        let r = row_range([9.5, 1.0, 1.0], [-0.5, 0.0, 0.0], [8.9999, 5.0, 5.0], 40);
        assert_eq!(r, Some((2, 19)));
        // A fixed coordinate outside: empty.
        assert_eq!(
            row_range([1.0, -1.0, 1.0], [0.5, 0.0, 0.0], [8.9999, 5.0, 5.0], 40),
            None
        );
        assert_eq!(
            row_range([f32::NAN; 3], [0.5, 0.0, 0.0], [8.9999; 3], 40),
            None
        );
    }
}
