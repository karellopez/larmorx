// SPDX-License-Identifier: Apache-2.0
//! FLIRT's costs (`specs/flirt.md` §6) on the shared engine's sampling
//! (`larmorx_mri::hmc::cost`): an image pair (a reference level and the current input), every
//! cost function, their weighted forms, `labeldiff` and the boundary-based cost.
//!
//! A cost evaluation samples the reference grid's rows; with `parallel` its slices run in
//! parallel, each with its own sums, folded in slice order: the same bits at any thread count.

use std::sync::Mutex;

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
/// across parallel evaluations does not matter).
#[derive(Default)]
pub struct Log {
    lines: Mutex<Vec<String>>,
    fatal: Mutex<Option<String>>,
}

impl Log {
    pub fn push(&self, line: impl Into<String>) {
        if let Ok(mut l) = self.lines.lock() {
            l.push(line.into());
        }
    }
    pub fn take(&self) -> Vec<String> {
        self.lines.lock().map(|mut l| std::mem::take(&mut *l)).unwrap_or_default()
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
        let centre = if skip_non_finite {
            input.centre_of_mass_finite()
        } else {
            input.centre_of_mass()
        };
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
            test_factors: (tb.b1, tb.b0),
            extremes: (lo, hi),
        }
    }

    /// The boundary-pair step (`setoption bbrstep`).
    pub fn bbr_step(&self) -> usize {
        self.bbr_step.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn reference_bins(&self) -> &Volume {
        self.ref_bins.get_or_init(|| {
            hcost::bin_grid_mi(&self.reference, self.bins, self.skip_non_finite)
        })
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
        if cost == Cost::Bbr {
            return super::bbr::cost(self, a, log);
        }
        if mat::inverse(a).is_none() {
            log.fatal(mat::SINGULAR);
            return f32::NAN;
        }
        let b = hcost::voxel_map(a, self.input.voxel_size, self.reference.voxel_size);
        let tv = self.test_volume();
        let ones = |_: usize, _: [f32; 3]| 1.0f32;
        let ref_w = self.ref_weight.as_ref();
        let in_w = self.in_weight.as_ref();
        let weight_fn = |index: usize, p: [f32; 3]| -> f32 {
            let wt = in_w.map_or(1.0, |w| trilinear_inside(w, p[0], p[1], p[2]));
            let wr = ref_w.map_or(1.0, |w| w.data[index]);
            wt * wr
        };
        let weight: Option<PointWeight<'_>> = if self.weighted() {
            Some(&weight_fn)
        } else {
            None
        };
        let _ = ones;
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
            Cost::LeastSq => {
                hcost::least_squares_weighted(&self.reference, &tv, &b, parallel, weight)
            }
            Cost::CorrRatio => hcost::corr_ratio_weighted(
                self.reference_bins(),
                &tv,
                &b,
                self.bins,
                parallel,
                weight,
            ),
            Cost::Woods => {
                if weight.is_some() {
                    log.push("WARNING: Woods is not implemented with cost function weighting");
                }
                hcost::woods(self.reference_bins(), &tv, &b, self.bins, parallel)
            }
            Cost::MutualInfo | Cost::NormMi => hcost::mutual_info_weighted(
                self.reference_bins(),
                &tv,
                &b,
                self.bins,
                cost == Cost::NormMi,
                parallel,
                weight,
            ),
            Cost::LabelDiff => self.labeldiff(&tv, &b, parallel, weight),
            Cost::Bbr => unreachable!(),
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
        let slices = hcost::sample_weighted(&self.reference, tv, b, parallel, weight, || {
            LabelSlice {
                input,
                row: (0.0, 0.0),
                slice: (0.0, 0.0),
                points: 0,
            }
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
