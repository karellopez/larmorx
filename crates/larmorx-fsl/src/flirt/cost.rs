// SPDX-License-Identifier: Apache-2.0
//! FLIRT's costs (`specs/flirt.md` §6) on the shared engine's sampling
//! (`larmorx_mri::hmc::cost`): an image pair (a reference level and the current input), every
//! cost function, their weighted forms, `labeldiff` and the boundary-based cost.
//!
//! A cost evaluation samples the reference grid's rows; with `parallel` its slices run in
//! parallel, each with its own sums, folded in slice order: the same bits at any thread count.

use std::sync::{Arc, Mutex};

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::math::Libm;

use larmorx_mri::hmc::cost::{
    self as hcost, Accumulator, CorrSlice, NormCorrCount, PointWeight, TestBins, TestVolume,
};
use larmorx_mri::hmc::interp::trilinear_inside;
use larmorx_mri::hmc::volume::Volume;

use super::bbr::{BbrPair, BbrSource};
use super::mat::{self, Mat4};

/// FLIRT's cost functions (`-cost`, `-searchcost`, `setoption costfunction`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cost {
    CorrRatio,
    MutualInfo,
    NormCorr,
    NormMi,
    LeastSq,
    LabelDiff,
    Bbr,
    Woods,
}

impl Cost {
    pub fn from_name(name: &str) -> Option<Cost> {
        Some(match name {
            "corratio" => Cost::CorrRatio,
            "mutualinfo" => Cost::MutualInfo,
            "normcorr" => Cost::NormCorr,
            "normmi" => Cost::NormMi,
            "leastsq" => Cost::LeastSq,
            "labeldiff" => Cost::LabelDiff,
            "bbr" => Cost::Bbr,
            "woods" => Cost::Woods,
            _ => return None,
        })
    }

    /// Whether the cost bins the reference (`-bins` applies).
    pub fn histogram(self) -> bool {
        matches!(
            self,
            Cost::CorrRatio | Cost::MutualInfo | Cost::NormMi | Cost::Woods
        )
    }
}

/// Messages a cost prints on standard error as it evaluates (identical lines, so their order
/// across parallel evaluations does not matter; the search's free optimisations, whose lines
/// differ, each write a [`Log::child`] passed on in their order).
#[derive(Debug, Default)]
pub struct Log {
    lines: Mutex<Vec<String>>,
    fatal: Mutex<Option<String>>,
    /// The run's C library functions (GLIBC-07: correctly rounded by default, glibc's with
    /// `reproduce`; quiet, FLIRT names no GLIBC-07), shared with the run's quiet logs.
    libm: Arc<Libm>,
}

impl Log {
    /// The log of a run in `mode` (its functions [`Libm::quiet`]'s).
    pub fn new(mode: UpstreamDefects) -> Log {
        Log {
            libm: Arc::new(Libm::quiet(mode)),
            ..Log::default()
        }
    }
    /// A log whose functions are `libm` (a run's, shared).
    pub fn sharing(libm: Arc<Libm>) -> Log {
        Log {
            libm,
            ..Log::default()
        }
    }
    /// A log whose messages are dropped, with this log's functions.
    pub fn quiet(&self) -> Log {
        Log {
            libm: Arc::clone(&self.libm),
            ..Log::default()
        }
    }
    /// The run's C library functions.
    pub fn libm(&self) -> &Libm {
        &self.libm
    }
    pub fn push(&self, line: impl Into<String>) {
        if let Ok(mut l) = self.lines.lock() {
            l.push(line.into());
        }
    }
    /// `lines` pushed together, so the lines of one evaluation stay together when evaluations
    /// run in parallel (the boundary-based cost's set-up lines with `-debug`, §9.4).
    pub fn push_all<S: Into<String>>(&self, lines: impl IntoIterator<Item = S>) {
        if let Ok(mut l) = self.lines.lock() {
            l.extend(lines.into_iter().map(Into::into));
        }
    }
    pub fn take(&self) -> Vec<String> {
        self.lines
            .lock()
            .map(|mut l| std::mem::take(&mut *l))
            .unwrap_or_default()
    }
    /// A new log with this log's functions, for one part of a run whose lines must keep their
    /// order (a free optimisation of the search, which may run beside others): its lines are
    /// passed on with [`Log::absorb`].
    pub fn child(&self) -> Log {
        self.quiet()
    }
    /// `other`'s lines appended in their order, and its fatal error kept if this log has none.
    pub fn absorb(&self, other: &Log) {
        for line in other.take() {
            self.push(line);
        }
        if let Some(f) = other.fatal_error() {
            self.fatal(f);
        }
    }
    /// An error that ends the run (FSL's exception, e.g. a singular matrix): the first is kept.
    pub fn fatal(&self, message: impl Into<String>) {
        if let Ok(mut f) = self.fatal.lock() {
            f.get_or_insert_with(|| message.into());
        }
    }
    /// The error that ends the run, if one happened.
    pub fn fatal_error(&self) -> Option<String> {
        self.fatal.lock().ok().and_then(|f| f.clone())
    }
}

/// An image pair (§6.1): what every cost evaluation of a schedule's scale compares.
pub struct Pair {
    pub reference: Volume,
    pub ref_weight: Option<Volume>,
    pub input: Volume,
    pub in_weight: Option<Volume>,
    /// The input's centre of mass in scaled mm (the parameters' centre, §3).
    pub centre: [f64; 3],
    /// `trunc(bins / s)` (§6.1).
    pub bins: usize,
    pub cost: Cost,
    pub smoothing: f32,
    pub fuzzy: f32,
    /// FSL-14 corrected: non-finite values left out of the sums.
    pub skip_non_finite: bool,
    /// The initial matrix (the cost sees `T·M_init`).
    pub init: Mat4,
    /// The reference bins of the histogram costs, made when first needed.
    ref_bins: std::sync::OnceLock<Volume>,
    /// The boundary points of the boundary-based cost, made when first needed.
    pub bbr: std::sync::OnceLock<Result<BbrPair, String>>,
    /// What the boundary-based cost reads (`-wmseg`, `-wmcoords`, the field map, its type).
    pub bbr_source: Option<std::sync::Arc<BbrSource>>,
    /// `setoption bbrstep`: every step-th boundary pair is used.
    pub bbr_step: std::sync::atomic::AtomicUsize,
    /// The scale has no bins (`-bins` below the scale, FSL-21): a histogram cost is refused when
    /// it is evaluated here.
    pub no_bins: Option<String>,
    /// The test bins' factors (`b1`, `b0`) of the mutual-information costs.
    test_factors: (f32, f32),
    /// The reference's and input's extremes (the empty-overlap value of `leastsq` and
    /// `labeldiff`).
    extremes: (f32, f32),
}

impl Pair {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        reference: Volume,
        ref_weight: Option<Volume>,
        input: Volume,
        in_weight: Option<Volume>,
        bins: usize,
        cost: Cost,
        smoothing: f32,
        fuzzy: f32,
        skip_non_finite: bool,
        init: Mat4,
    ) -> Pair {
        let centre = centre_of_mass(&input, skip_non_finite);
        let tb = TestBins::new(&input, bins);
        let (lo, hi) = [&reference, &input]
            .iter()
            .flat_map(|v| v.data.iter())
            .filter(|v| v.is_finite())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &x| {
                (l.min(x), h.max(x))
            });
        Pair {
            reference,
            ref_weight,
            input,
            in_weight,
            centre,
            bins,
            cost,
            smoothing,
            fuzzy,
            skip_non_finite,
            init,
            ref_bins: std::sync::OnceLock::new(),
            bbr: std::sync::OnceLock::new(),
            bbr_source: None,
            bbr_step: std::sync::atomic::AtomicUsize::new(1),
            no_bins: None,
            test_factors: (tb.b1, tb.b0),
            extremes: (lo, hi),
        }
    }

    /// The boundary-pair step (`setoption bbrstep`).
    pub fn bbr_step(&self) -> usize {
        self.bbr_step.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn reference_bins(&self) -> &Volume {
        self.ref_bins
            .get_or_init(|| hcost::bin_grid_mi(&self.reference, self.bins, self.skip_non_finite))
    }

    /// Whether weighting is on (`-refweight` or `-inweight`).
    pub fn weighted(&self) -> bool {
        self.ref_weight.is_some() || self.in_weight.is_some()
    }

    fn test_volume(&self) -> TestVolume<'_> {
        let mut tv = TestVolume::new(&self.input, self.smoothing);
        tv.skip_non_finite = self.skip_non_finite;
        tv.count = NormCorrCount::Mcflirt;
        tv.test_bins = Some(TestBins {
            b1: self.test_factors.0,
            b0: self.test_factors.1,
            fuzzy: self.fuzzy,
        });
        tv
    }

    /// The cost `cost` of the trial matrix `t` (the matrix compared is `t·M_init`).
    pub fn cost_of(&self, t: &Mat4, cost: Cost, parallel: bool, log: &Log) -> f32 {
        let a = mat::mul(t, &self.init);
        self.evaluate(&a, cost, parallel, log)
    }

    /// The cost of the matrix `a` (input mm → reference mm).
    pub fn evaluate(&self, a: &Mat4, cost: Cost, parallel: bool, log: &Log) -> f32 {
        if let Some(m) = &self.no_bins
            && cost.histogram()
        {
            log.fatal(m.clone());
            return f32::NAN;
        }
        if cost == Cost::Bbr {
            return super::bbr::cost(self, a, log);
        }
        if mat::inverse(a).is_none() {
            log.fatal(mat::SINGULAR);
            return f32::NAN;
        }
        let b = hcost::voxel_map(a, self.input.voxel_size, self.reference.voxel_size);
        let tv = self.test_volume();
        let ref_w = self.ref_weight.as_ref();
        let in_w = self.in_weight.as_ref();
        let edges = tv.weighted();
        // wt·wr, then the edge factors multiply it one after the other (§6.4)
        let weight_fn = |index: usize, p: [f32; 3], w: f32| -> f32 {
            let wt = in_w.map_or(1.0, |w| trilinear_inside(w, p[0], p[1], p[2]));
            let wr = ref_w.map_or(1.0, |w| w.data[index]);
            if !edges {
                return (wt * wr) * w;
            }
            let [fx, fy, fz] = tv.edge_factors(p);
            let v = (((wt * wr) * fx) * fy) * fz;
            if v < 0.0 { 0.0 } else { v }
        };
        let weight: Option<PointWeight<'_>> = if self.weighted() {
            Some(&weight_fn)
        } else {
            None
        };
        match cost {
            Cost::NormCorr => {
                let slices = hcost::sample_weighted(
                    &self.reference,
                    &tv,
                    &b,
                    parallel,
                    weight,
                    CorrSlice::default,
                );
                if weight.is_some() {
                    1.0 - hcost::correlation(&slices, true, NormCorrCount::Mcflirt)
                } else {
                    1.0 - hcost::correlation(&slices, tv.weighted(), NormCorrCount::Mcflirt).abs()
                }
            }
            Cost::LeastSq => self.least_squares(&tv, &b, parallel, weight),
            Cost::CorrRatio => hcost::corr_ratio_of(&hcost::bin_moments(
                self.reference_bins(),
                &tv,
                &b,
                self.bins,
                parallel,
                weight,
                false,
            )),
            Cost::Woods => {
                if weight.is_some() {
                    log.push("WARNING: Woods is not implemented with cost function weighting");
                }
                hcost::woods_of(&hcost::bin_moments(
                    self.reference_bins(),
                    &tv,
                    &b,
                    self.bins,
                    parallel,
                    None,
                    true,
                ))
            }
            Cost::MutualInfo | Cost::NormMi => hcost::mutual_info_weighted(
                self.reference_bins(),
                &tv,
                &b,
                self.bins,
                cost == Cost::NormMi,
                parallel,
                weight,
                log.libm(),
            ),
            Cost::LabelDiff => self.labeldiff(&tv, &b, parallel, weight),
            Cost::Bbr => unreachable!(),
        }
    }

    /// `leastsq` (§6.2, spec writer 2026-11-03): over the contributing points in the sampling
    /// order, `δ = r − y` in single precision; without smoothing or weights `δ·δ` and the count,
    /// otherwise `(w·δ)·δ` and `w`; three levels of single-precision sums (row, slice, total);
    /// the total over the weight (the count) when that is above 1, else `(max − min)²` of both
    /// images.
    fn least_squares(
        &self,
        tv: &TestVolume<'_>,
        b: &[[f32; 4]; 3],
        parallel: bool,
        weight: Option<PointWeight<'_>>,
    ) -> f32 {
        let smoothed = tv.weighted() || weight.is_some();
        let slices =
            hcost::sample_weighted(&self.reference, tv, b, parallel, weight, || SquaresSlice {
                smoothed,
                row: (0.0, 0.0),
                slice: (0.0, 0.0),
                points: 0,
            });
        let (mut num, mut den) = (0.0f32, 0.0f32);
        let mut points = 0u64;
        for s in &slices {
            num += s.slice.0;
            den += s.slice.1;
            points += s.points;
        }
        let empty = |(lo, hi): (f32, f32)| (hi - lo) * (hi - lo);
        if smoothed {
            if den <= 1.0 {
                return empty(self.extremes);
            }
            num / den
        } else {
            if points <= 1 {
                return empty(self.extremes);
            }
            num / points as f32
        }
    }

    /// `labeldiff` (§6.3).
    fn labeldiff(
        &self,
        tv: &TestVolume<'_>,
        b: &[[f32; 4]; 3],
        parallel: bool,
        weight: Option<PointWeight<'_>>,
    ) -> f32 {
        let input = &self.input;
        let smoothed = tv.weighted() || weight.is_some();
        let slices =
            hcost::sample_weighted(&self.reference, tv, b, parallel, weight, || LabelSlice {
                input,
                row: (0.0, 0.0),
                slice: (0.0, 0.0),
                points: 0,
            });
        let (mut num, mut den) = (0.0f32, 0.0f32);
        let mut points = 0u64;
        for s in &slices {
            num += s.slice.0;
            den += s.slice.1;
            points += s.points;
        }
        let empty = |(lo, hi): (f32, f32)| (hi - lo) * (hi - lo);
        if smoothed {
            if den <= 1.0 {
                return empty(self.extremes);
            }
            num / den
        } else {
            if points <= 1 {
                return empty(self.extremes);
            }
            num / points as f32
        }
    }
}

/// The sums of one slice of `leastsq` (§6.2): `Σ δ·δ` (or `Σ (w·δ)·δ` and `Σ w`) per row, per
/// slice, in single precision.
struct SquaresSlice {
    smoothed: bool,
    row: (f32, f32),
    slice: (f32, f32),
    points: u64,
}

impl Accumulator for SquaresSlice {
    #[inline(always)]
    fn point(&mut self, g: f32, v: f32, w: f32) {
        let d = g - v;
        if self.smoothed {
            self.row.0 += (w * d) * d;
            self.row.1 += w;
        } else {
            self.row.0 += d * d;
        }
        self.points += 1;
    }

    fn end_row(&mut self) {
        self.slice.0 += self.row.0;
        self.slice.1 += self.row.1;
        self.row = (0.0, 0.0);
    }
}

/// The mismatch sums of one slice of `labeldiff` (§6.3): `Σ w·mismatch` and `Σ w` per row,
/// per slice.
struct LabelSlice<'a> {
    input: &'a Volume,
    row: (f32, f32),
    slice: (f32, f32),
    points: u64,
}

impl Accumulator for LabelSlice<'_> {
    fn point(&mut self, _g: f32, _v: f32, _w: f32) {}

    #[inline(always)]
    fn point_at(&mut self, g: f32, _v: f32, w: f32, _index: usize, p: [f32; 3]) {
        let m = mismatch(self.input, p, g);
        self.row.0 += w * m;
        self.row.1 += w;
        self.points += 1;
    }

    fn end_row(&mut self) {
        self.slice.0 += self.row.0;
        self.slice.1 += self.row.1;
        self.row = (0.0, 0.0);
    }
}

/// The trilinear weight of the cell corners whose labels differ from `x` by more than 0.5, in
/// the order 000, 001, 011, 010, 110, 100, 101, 111 (§6.3).
fn mismatch(v: &Volume, p: [f32; 3], x: f32) -> f32 {
    let (ix, iy, iz) = (p[0] as usize, p[1] as usize, p[2] as usize);
    let dx = f64::from(p[0] - ix as f32);
    let dy = f64::from(p[1] - iy as f32);
    let dz = f64::from(p[2] - iz as f32);
    let at = |a: usize, b: usize, c: usize| v.at(ix + a, iy + b, iz + c);
    let w = |a: usize, b: usize, c: usize| {
        let fx = if a == 1 { dx } else { 1.0 - dx };
        let fy = if b == 1 { dy } else { 1.0 - dy };
        let fz = if c == 1 { dz } else { 1.0 - dz };
        fx * fy * fz
    };
    let mut sum = 0.0f32;
    for (a, b, c) in [
        (0, 0, 0),
        (0, 0, 1),
        (0, 1, 1),
        (0, 1, 0),
        (1, 1, 0),
        (1, 0, 0),
        (1, 0, 1),
        (1, 1, 1),
    ] {
        if (at(a, b, c) - x).abs() > 0.5 {
            sum = (f64::from(sum) + w(a, b, c)) as f32;
        }
    }
    sum
}

/// FLIRT's centre of mass (`specs/flirt.md` §8.1 step 3), in mm: `v − v_min` in single
/// precision, converted to double, the sums `Σ(v − v_min)·i` per axis and `Σ(v − v_min)` over the
/// voxels in z, y, x order in blocks of `max(1000, trunc(√N)) + 1` voxels (each block's double
/// sums restart at 0 and are added to the totals, the last block's at the end); each coordinate
/// its total over the total (a total below 1e-5 in magnitude counts 1) times the voxel size.
/// With `finite_only` (FSL-14 corrected) NaN and infinite voxels are left out of the minimum and
/// the sums (they still count in the blocks' lengths).
pub fn centre_of_mass(v: &Volume, finite_only: bool) -> [f64; 3] {
    centre_of_mass_counted(v, finite_only).0
}

/// [`centre_of_mass`], and whether its total was below 1e-5 in magnitude (counted as 1: FSL
/// prints `WARNING::in calculating COG, total = 0.0` each time it measures such a centre; a NaN
/// total is not below).
pub fn centre_of_mass_counted(v: &Volume, finite_only: bool) -> ([f64; 3], bool) {
    let [nx, ny, nz] = v.shape;
    let vmin = if finite_only {
        let mut it = v.data.iter().copied().filter(|x| x.is_finite());
        it.next().map_or(0.0, |first| {
            it.fold(first, |m, x| if x < m { x } else { m })
        })
    } else {
        v.min()
    };
    let n = v.data.len();
    let block = ((n as f64).sqrt() as usize).max(1000) + 1;
    let mut tot = [0.0f64; 4];
    let mut part = [0.0f64; 4];
    let mut count = 0usize;
    for z in 0..nz {
        for y in 0..ny {
            let row = &v.data[nx * (y + ny * z)..nx * (y + ny * z + 1)];
            for (x, &val) in row.iter().enumerate() {
                if !(finite_only && !val.is_finite()) {
                    let w = f64::from(val - vmin);
                    part[0] += w * x as f64;
                    part[1] += w * y as f64;
                    part[2] += w * z as f64;
                    part[3] += w;
                }
                count += 1;
                if count == block {
                    for k in 0..4 {
                        tot[k] += part[k];
                        part[k] = 0.0;
                    }
                    count = 0;
                }
            }
        }
    }
    for k in 0..4 {
        tot[k] += part[k];
    }
    let mut total = tot[3];
    let zero = total.abs() < 1e-5;
    if zero {
        total = 1.0;
    }
    let d = v.voxel_size.map(f64::from);
    (
        [
            d[0] * (tot[0] / total),
            d[1] * (tot[1] / total),
            d[2] * (tot[2] / total),
        ],
        zero,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §6.2's `leastsq` sums: without smoothing `δ·δ` and the count; with smoothing `(w·δ)·δ`
    /// and `w`; each row's sums added to the slice's at the row's end, in single precision.
    #[test]
    fn leastsq_sums_by_rows() {
        let mut plain = SquaresSlice {
            smoothed: false,
            row: (0.0, 0.0),
            slice: (0.0, 0.0),
            points: 0,
        };
        let mut weighted = SquaresSlice {
            smoothed: true,
            row: (0.0, 0.0),
            slice: (0.0, 0.0),
            points: 0,
        };
        let rows: [&[(f32, f32, f32)]; 2] = [
            &[(1e8, 0.5, 0.25), (3.0, 1.0, 0.5)],
            &[(2.0, 0.1, 1.0), (1e-3, 0.0, 0.75)],
        ];
        let (mut want_p, mut want_w) = (0.0f32, (0.0f32, 0.0f32));
        for row in rows {
            let (mut rp, mut rw) = (0.0f32, (0.0f32, 0.0f32));
            for &(g, v, w) in row {
                plain.point(g, v, w);
                weighted.point(g, v, w);
                let d = g - v;
                rp += d * d;
                rw.0 += (w * d) * d;
                rw.1 += w;
            }
            plain.end_row();
            weighted.end_row();
            want_p += rp;
            want_w.0 += rw.0;
            want_w.1 += rw.1;
        }
        assert_eq!(plain.slice.0.to_bits(), want_p.to_bits());
        assert_eq!(plain.points, 4);
        assert_eq!(weighted.slice.0.to_bits(), want_w.0.to_bits());
        assert_eq!(weighted.slice.1.to_bits(), want_w.1.to_bits());
    }
}
