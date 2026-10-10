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

/// What the cost needs about a test volume, computed once per volume and stage.
pub struct TestVolume<'a> {
    pub vol: &'a Volume,
    /// `n_k − 1.0001` in float32: the largest admissible position per axis.
    pub bound: [f32; 3],
    /// The edge-weighting width per axis in voxels (`smooth / d_k`); 0 turns weighting off.
    pub sigma: [f32; 3],
    pub interpolation: CostInterpolation,
}

impl<'a> TestVolume<'a> {
    /// A test volume with edge-weighting width `smooth` mm and trilinear interpolation.
    pub fn new(vol: &'a Volume, smooth: f32) -> TestVolume<'a> {
        TestVolume {
            vol,
            bound: vol.shape.map(|n| (n as f64 - 1.0001) as f32),
            sigma: vol.voxel_size.map(|d| smooth / d),
            interpolation: CostInterpolation::Trilinear,
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

    #[inline(always)]
    fn value(&self, p: [f32; 3]) -> f32 {
        match self.interpolation {
            CostInterpolation::Trilinear => trilinear_inside(self.vol, p[0], p[1], p[2]),
            CostInterpolation::Sinc { background } => sinc_cost(self.vol, p, background),
        }
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

/// Visits the contributing points of grid slice `z` (§6.1).
#[inline(always)]
fn sample_slice<A: Accumulator>(
    grid: &Volume,
    test: &TestVolume<'_>,
    b: &[[f32; 4]; 3],
    z: usize,
    acc: &mut A,
) {
    let [gx, gy, _] = grid.shape;
    let step = [b[0][0], b[1][0], b[2][0]];
    let zf = z as f32;
    let weighted = test.weighted();
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
            while x <= x1 && test.inside(p) {
                let w = if weighted { test.weight(p) } else { 1.0 };
                acc.point(grow[x], test.value(p), w);
                p = [p[0] + step[0], p[1] + step[1], p[2] + step[2]];
                x += 1;
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
        sample_slice(grid, test, b, z, &mut acc);
        acc
    };
    if parallel {
        (0..nz).into_par_iter().map(one).collect()
    } else {
        (0..nz).map(one).collect()
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
/// weights); without, the number of contributing points (Pearson's correlation).
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
        correlation_weighted(&total, n)
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

/// A joint histogram of reference bins and test bins with fuzzy binning (appendix A).
struct JointSlice {
    hist: Vec<f64>,
    bins: usize,
    lo: f32,
    range: f32,
    overlap: f64,
}

impl Accumulator for JointSlice {
    #[inline(always)]
    fn point(&mut self, g: f32, v: f32, w: f32) {
        let bins = self.bins;
        let r = (g as usize).min(bins - 1);
        // The sample is spread over its bin and the nearer neighbour, linearly within half a
        // bin of the bin's centre.
        let t = f64::from((v - self.lo) * bins as f32 / self.range);
        let k = (t.floor().max(0.0) as usize).min(bins - 1);
        let frac = t - k as f64 - 0.5;
        let (other, share) = if frac >= 0.0 {
            ((k + 1).min(bins - 1), frac.min(0.5))
        } else {
            (k.saturating_sub(1), (-frac).min(0.5))
        };
        let w = f64::from(w);
        self.hist[r * bins + k] += w * (1.0 - share);
        self.hist[r * bins + other] += w * share;
        self.overlap += 1.0;
    }
    fn end_row(&mut self) {}
}

/// The mutual-information costs (appendix A): `−MI`, or `−(H1 + H2)/H12` when `normalised`.
/// Entropies use probabilities `count / grid points`, corrected to the overlap size by
/// `H' = (n_grid/n_overlap)·H − log(n_grid/n_overlap)`.
pub fn mutual_info(
    bin_grid: &Volume,
    test: &TestVolume<'_>,
    b: &[[f32; 4]; 3],
    bins: usize,
    normalised: bool,
    parallel: bool,
) -> f32 {
    let (lo, hi) = min_max(&test.vol.data);
    let range = if hi - lo == 0.0 { 1.0 } else { hi - lo };
    let make = || JointSlice {
        hist: vec![0.0; bins * bins],
        bins,
        lo,
        range,
        overlap: 0.0,
    };
    let mut hist = vec![0.0f64; bins * bins];
    let mut overlap = 0.0;
    for s in sample(bin_grid, test, b, parallel, make) {
        for (h, x) in hist.iter_mut().zip(&s.hist) {
            *h += x;
        }
        overlap += s.overlap;
    }
    if overlap <= 0.0 {
        return 0.0;
    }
    let n_grid = bin_grid.len() as f64;
    let entropy = |counts: &mut dyn Iterator<Item = f64>| -> f64 {
        counts
            .filter(|&c| c > 0.0)
            .map(|c| {
                let q = c / n_grid;
                -q * math::log(q)
            })
            .sum()
    };
    let ratio = n_grid / overlap;
    let correct = |h: f64| ratio * h - math::log(ratio);
    let h12 = correct(entropy(&mut hist.iter().copied()));
    let h1 = correct(entropy(
        &mut (0..bins).map(|r| hist[r * bins..(r + 1) * bins].iter().sum()),
    ));
    let h2 = correct(entropy(
        &mut (0..bins).map(|t| (0..bins).map(|r| hist[r * bins + t]).sum()),
    ));
    if normalised {
        if h12 > 0.0 {
            (-(h1 + h2) / h12) as f32
        } else {
            0.0
        }
    } else {
        (-(h1 + h2 - h12)) as f32
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
        for normalised in [false, true] {
            let m0 = mutual_info(&bg, &test, &b0, 64, normalised, false);
            let m1 = mutual_info(&bg, &test, &b1, 64, normalised, false);
            assert!(m0 < m1, "{normalised}: {m0} {m1}");
        }
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
