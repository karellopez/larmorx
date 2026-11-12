// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's histograms (`fs.maths`, queue item 3.1-23), translated from FreeSurfer 8.2.0's
//! `include/histo.h` and `utils/histo.cpp`: the functions FreeSurfer's programs reach (static
//! reach, `docs/freesurfer/code.md` §6): `HISTOfree`, `HISTOrealloc`, `HISTOalloc`,
//! `HISTOcopy`, `HISTOclear`, `HISTOclearCounts`, `HISTOadd`, `HISTOsubtract`,
//! `HISTOclearBins`, `HISTOsmooth`, the peak and valley searches (`HISTOfindLastPeak`,
//! `HISTOfindFirstPeak`, `HISTOfindValley`, `HISTOfindLastPeakInRegion`,
//! `HISTOfindHighestPeakInRegion`, `HISTOfindPreviousValley`, `HISTOfindNextValley`,
//! `HISTOfindNextPeak`, `HISTOfindPreviousPeak`, `HISTOfindStartOfPeak`, `HISTOfindEndOfPeak`,
//! `HISTOfindCurrentPeak`), `HISTOplot`, `HISTOaddSample`, `HISTOfillHoles`,
//! `HISTOtotalInRegion`, `HISTOtotal`, `HISTOclearZeroBin`, `HISTOfindBinWithCount`,
//! `HISTOfindBin`, `HISTOgetCount`, `HISTOcorrelate`, `histoComputeLinearFitCorrelation`,
//! `HISTOfindLinearFit`, `HISTOlinearScale`, `HISTOthreshSum`, `HISTOmakePDF`, `HISTObins`,
//! `HISTOcount`, `HISTOvalToBin`, `HISTOinit`, `HISTOfindMedian`, `HISTOmakeReverseCDF`,
//! `HISTOmakeCDF`, `HISTOrobustGaussianFit`, `HISTOabs`, `HISTOgetEntropy`,
//! `HISTOsoapBubbleZeros`, `HISTO2DfindBin1`, `HISTO2DfindBin2`, `HISTO2DgetCount`,
//! `HISTOcomputeFWHM`, `HISTOwriteTxt`, `HISTOsumNorm`, `HISTOcumsum`.
//!
//! Every expression keeps C's types: `float` sums stay `float` (each step rounded), `double`
//! ones `double`; the divisions that can make a NaN from numbers (`0 / 0` of an empty histogram, a
//! histogram of one bin, a zero count) follow SSE2's NaN rules on every platform (`crate::sse`); a `float` compared with an `int` converts the `int`; the C library's `exp`,
//! `log` and `pow` are larmorx's correctly rounded ones by default and glibc 2.35's with
//! `reproduce` (`larmorx_core::math::Libm`).
//!
//! A [`Histogram`] keeps FreeSurfer's two sizes: `max_bins`, what was allocated (the arrays'
//! length), and `nbins`, how many are in use (`HISTOcopy` sets it to the source's). A bin that
//! the C code addresses below 0 or beyond the allocation is memory FreeSurfer does not own.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-maths.md`):
//! - **FS-17**: `(int)` of a `float` or `double` beyond the `int` range or NaN
//!   ([`crate::affine::float_to_int`]); `HISTOfindValley` compares counts as `int`s.
//! - **FS-18** (corrected by default): `HISTOclearCounts` clears only the first count (`memset`
//!   with `sizeof(counts[0])`), and with no output histogram copies only the first bin's value
//!   (`memmove` with `sizeof(*bins)`). The default clears every count and copies every bin.
//! - **FS-19** (corrected by default): `HISTOaddSample` puts a sample outside the range in the
//!   first bin when its bin number is below the range's lower *value* (`bin_no < bmin`, a bin
//!   number compared with an intensity), else in the last bin if its bin number is beyond the
//!   bins, else nowhere: below the range with a negative lower value it is dropped, above it with
//!   a lower value beyond the bin count it goes to the first bin. The default puts a sample below
//!   the range in the first bin and above it in the last; a NaN sample (counted in the first bin
//!   through FS-12's conversion) is not counted.
//! - **FS-20** (corrected by default): `HISTOaddSample` shares a sample between the two bins
//!   around it by its distance to them, `ceil(x) - x` and `x - floor(x)`: a sample exactly at a
//!   bin's value gets both weights 0 and is not counted at all (FreeSurfer's own
//!   `utils/mideface.cpp` says it "appears to be broken when binsize=1"). The default counts it
//!   once in its bin.
//! - **FS-21** (both modes defined): copying, adding or subtracting into an output histogram that
//!   has fewer bins allocated than the result writes past the output's arrays. Both modes grow the
//!   output's arrays to hold the result (what a read of them would then give), with FS-21 noted.
//! - **FS-22** (both modes defined): the searches and a few other functions read (or write) a bin
//!   below 0 or beyond the allocation when their start bin is outside the histogram (a search's
//!   "not found", -1, passed on), when a window reaches past an end (`HISTOfindNextPeak`,
//!   `HISTOfindPreviousPeak`), and for histograms of 0 or 1 bins. Such a read gives 0 and such a
//!   write is dropped, in both modes, with FS-22 noted; reads whose value cannot change the result
//!   (FreeSurfer reads before it returns) are not made.
//! - **FS-23** (corrected by default): `HISTOmakePDF(src, dst)` with an output histogram other
//!   than `src` normalises the output's own counts, not `src`'s (every caller passes the same
//!   histogram twice). The default copies `src` into it first.
//! - **FS-24** (both modes defined): `HISTOrobustGaussianFit` of a histogram of one bin never
//!   returns (its search window cannot grow). Both modes stop widening the window when it covers
//!   the whole histogram, with FS-24 noted.
//!
//! Kept as FreeSurfer has them (rule C, documented in the findings): `HISTOinit` does not set
//! `nbins` on a histogram it is given (its callers set it); `HISTOfindEndOfPeak` returns the last
//! bin whenever no bin up to the next valley falls below the threshold (`if (h->nbins >= b)`);
//! `HISTOfindLinearFit`'s step in `a` is `MAX(MIN(amin, (amax - amin) / 1000), 0.01)`;
//! `HISTOfindBin` with a bin size of exactly 1 returns `(int)((int)val - bins[0])` without
//! looking at the bins.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_18, FS_19, FS_20, FS_21, FS_22, FS_23, FS_24};
use larmorx_core::math::Libm;

use crate::affine::float_to_int;
use crate::cstd::fmt_f;
use crate::diag::{DIAG_WRITE, verbose_on};
use crate::globals::Globals;
use crate::log::{self, ERROR_BADPARM, ERROR_NO_MEMORY, ERROR_NOFILE, ERROR_UNSUPPORTED, Fatal};
use crate::ret::Ret;
use crate::sse;
use crate::utils::{nint, resolve};

/// `MAX_BINS`.
pub const MAX_BINS: i32 = 10000;
/// `HISTOsmooth`'s `MAX_LEN`: the longest kernel.
const MAX_LEN: i32 = 2000;
/// `HISTOfindLastPeakInRegion`'s `MIN_STD`.
const MIN_STD: f64 = 1.9;
/// `HISTOrobustGaussianFit`'s `MIN_BINS`.
const MIN_BINS: i32 = 12;
/// `HISTOfindLinearFit`'s `NSTEPS`.
const NSTEPS: f64 = 1000.0;
/// glibc's `M_E`.
const M_E: f64 = std::f64::consts::E;
/// glibc's `FLT_EPSILON`, as `FZERO` compares with it.
const FLT_EPSILON: f64 = f32::EPSILON as f64;

/// A FreeSurfer `HISTOGRAM` (`HISTO`).
#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    /// `max_bins`: how many bins were allocated.
    pub max_bins: i32,
    /// `nbins`: how many are in use.
    pub nbins: i32,
    /// `bins[b]`: the value of bin `b` (the upper end of its range, as `histo.h` says; in
    /// practice its centre). The arrays are as long as the allocation.
    pub bins: Vec<f32>,
    /// `counts[b]`: how many samples bin `b` holds.
    pub counts: Vec<f32>,
    pub bin_size: f32,
    pub min: f32,
    pub max: f32,
}

impl Histogram {
    /// `HISTOalloc(nbins)` for a size known to be valid.
    fn zeros(nbins: i32) -> Histogram {
        assert!(nbins >= 0, "histogram of {nbins} bins");
        Histogram {
            max_bins: nbins,
            nbins,
            bins: vec![0.0; nbins as usize],
            counts: vec![0.0; nbins as usize],
            bin_size: 0.0,
            min: 0.0,
            max: 0.0,
        }
    }

    /// The bins in use, `0..nbins`.
    fn n(&self) -> usize {
        self.nbins.max(0) as usize
    }

    /// Grows the arrays to `len` (FS-21).
    fn grow(&mut self, g: &mut Globals, len: usize, what: &str) {
        if self.counts.len() < len {
            self.counts.resize(len, 0.0);
            self.bins.resize(len, 0.0);
            g.notices.add(
                &FS_21,
                format!("{what}: the result has more bins than the output histogram allocated; its arrays grow to hold them"),
            );
        }
    }
}

/// A FreeSurfer `HISTOGRAM2D` (`HISTO2D`): `counts[b1][b2]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Histogram2d {
    pub nbins1: i32,
    pub nbins2: i32,
    pub bins1: Vec<f32>,
    pub bins2: Vec<f32>,
    pub counts: Vec<Vec<f32>>,
    pub bin_size1: f32,
    pub bin_size2: f32,
    pub min1: f32,
    pub min2: f32,
    pub max1: f32,
    pub max2: f32,
}

/// `counts[b]` where `b` may be outside the allocation (FS-22: 0).
fn cnt(g: &mut Globals, h: &Histogram, b: i32, what: &str) -> f32 {
    if b >= 0
        && let Some(&v) = h.counts.get(b as usize)
    {
        return v;
    }
    g.notices.add(
        &FS_22,
        format!("{what}: bin {b} read outside the histogram's arrays (0 used)"),
    );
    0.0
}

/// `bins[b]` where `b` may be outside the allocation (FS-22: 0).
fn bin_value(g: &mut Globals, h: &Histogram, b: i32, what: &str) -> f32 {
    if b >= 0
        && let Some(&v) = h.bins.get(b as usize)
    {
        return v;
    }
    g.notices.add(
        &FS_22,
        format!("{what}: bin {b}'s value read outside the histogram's arrays (0 used)"),
    );
    0.0
}

/// `counts[b] = v` where `b` may be outside the allocation (FS-22: dropped).
fn set_cnt(g: &mut Globals, h: &mut Histogram, b: i32, v: f32, what: &str) {
    if b >= 0
        && let Some(p) = h.counts.get_mut(b as usize)
    {
        *p = v;
        return;
    }
    g.notices.add(
        &FS_22,
        format!("{what}: bin {b} written outside the histogram's arrays (dropped)"),
    );
}

/// `HISTOfree(&h)`: `NO_ERROR`.
pub fn histo_free(h: &mut Option<Histogram>) -> i32 {
    *h = None;
    0
}

/// `HISTOalloc(nbins)`: zeros, `max_bins = nbins = nbins`. A negative count is an allocation
/// that fails: `HISTOalloc(<n>): could not allocate histogram` and the program's end.
pub fn histo_alloc(g: &mut Globals, err: &mut dyn Write, nbins: i32) -> Result<Histogram, Fatal> {
    if nbins < 0 {
        log::error_exit(
            g,
            err,
            ERROR_NO_MEMORY,
            &format!("HISTOalloc({nbins}): could not allocate histogram"),
        )?;
        return Ok(Histogram::zeros(0));
    }
    Ok(Histogram::zeros(nbins))
}

/// `HISTOrealloc(histo, nbins)`: a new histogram when there is none; else new arrays of zeros,
/// `max_bins = nbins = nbins` (the other fields kept).
pub fn histo_realloc(
    g: &mut Globals,
    err: &mut dyn Write,
    histo: Option<&mut Histogram>,
    nbins: i32,
) -> Result<Ret<Histogram>, Fatal> {
    let Some(h) = histo else {
        return Ok(Ret::New(histo_alloc(g, err, nbins)?));
    };
    if nbins < 0 {
        log::error_exit(
            g,
            err,
            ERROR_NO_MEMORY,
            &format!("HISTOrealloc({nbins}): could not allocate histogram"),
        )?;
        return Ok(Ret::Given);
    }
    h.bins = vec![0.0; nbins as usize];
    h.counts = vec![0.0; nbins as usize];
    h.max_bins = nbins;
    h.nbins = nbins;
    Ok(Ret::Given)
}

/// `HISTOcopy(src, dst)`: `nbins`, `bin_size`, the first `nbins` counts and bins, `min` and
/// `max` (`max_bins` stays the output's). A copy onto itself changes nothing. FS-21.
pub fn histo_copy(g: &mut Globals, src: &Histogram, dst: Option<&mut Histogram>) -> Ret<Histogram> {
    match dst {
        None => {
            let mut d = Histogram::zeros(src.nbins);
            copy_into(g, src, &mut d);
            Ret::New(d)
        }
        Some(d) => {
            copy_into(g, src, d);
            Ret::Given
        }
    }
}

fn copy_into(g: &mut Globals, src: &Histogram, d: &mut Histogram) {
    let n = src.n();
    d.grow(g, n, "HISTOcopy");
    d.nbins = src.nbins;
    d.bin_size = src.bin_size;
    d.counts[..n].copy_from_slice(&src.counts[..n]);
    d.bins[..n].copy_from_slice(&src.bins[..n]);
    d.min = src.min;
    d.max = src.max;
}

/// `HISTOclear(src, dst)`: the output's first `nbins` counts and bins set to 0 (a new histogram
/// of `src`'s size when there is none). `HISTOclear(h, h)` is `histo_clear(&h, Some(h))`'s
/// in-place form [`histo_clear_in_place`].
pub fn histo_clear(src: &Histogram, dst: Option<&mut Histogram>) -> Ret<Histogram> {
    match dst {
        None => Ret::New(Histogram::zeros(src.nbins)),
        Some(d) => {
            histo_clear_in_place(d);
            Ret::Given
        }
    }
}

/// `HISTOclear(h, h)`.
pub fn histo_clear_in_place(h: &mut Histogram) {
    let n = h.n();
    h.counts[..n].fill(0.0);
    h.bins[..n].fill(0.0);
}

/// `HISTOclearCounts(src, dst)`: FS-18. With an output, `src` is not read.
pub fn histo_clear_counts(
    g: &mut Globals,
    src: &Histogram,
    dst: Option<&mut Histogram>,
) -> Ret<Histogram> {
    match dst {
        None => {
            let mut d = Histogram::zeros(src.nbins);
            let n = src.n().min(d.bins.len());
            if g.notices.reproduce() {
                if n > 0 {
                    d.bins[0] = src.bins[0];
                }
            } else {
                d.bins[..n].copy_from_slice(&src.bins[..n]);
            }
            if src.bins[..n].iter().skip(1).any(|&b| b != 0.0) {
                g.notices.add(
                    &FS_18,
                    if g.notices.reproduce() {
                        "HISTOclearCounts: only the first bin's value copied"
                    } else {
                        "HISTOclearCounts: every bin's value copied (FreeSurfer copies the first only)"
                    },
                );
            }
            Ret::New(d)
        }
        Some(d) => {
            histo_clear_counts_in_place(g, d);
            Ret::Given
        }
    }
}

/// `HISTOclearCounts(h, h)` (FS-18).
pub fn histo_clear_counts_in_place(g: &mut Globals, h: &mut Histogram) {
    let n = h.n();
    let others = h.counts[..n].iter().skip(1).any(|&c| c != 0.0);
    if g.notices.reproduce() {
        if let Some(c) = h.counts.first_mut() {
            *c = 0.0;
        }
    } else {
        h.counts[..n].fill(0.0);
    }
    if others {
        g.notices.add(
            &FS_18,
            if g.notices.reproduce() {
                "HISTOclearCounts: only the first count cleared"
            } else {
                "HISTOclearCounts: every count cleared (FreeSurfer clears the first only)"
            },
        );
    }
}

/// `HISTOadd(h1, h2, dst)`: `counts = h1 + h2` over `h1`'s bins. With no output, a new one with
/// `h1`'s bins, `min`, `max` and `bin_size`, and `h2` may be `None` (it is then the new, zero,
/// output); with one, its `nbins` becomes `h1`'s. To add into `h1` (or `h2`), pass a copy of it
/// as the input. FS-21; a missing `h2` with an output (a crash in FreeSurfer) adds zeros (FS-22).
pub fn histo_add(
    g: &mut Globals,
    h1: &Histogram,
    h2: Option<&Histogram>,
    dst: Option<&mut Histogram>,
) -> Ret<Histogram> {
    let given = dst.is_some();
    let mut fresh = None;
    let d: &mut Histogram = match dst {
        None => {
            let mut d = Histogram::zeros(h1.nbins);
            let n = h1.n();
            d.bins[..n].copy_from_slice(&h1.bins[..n]);
            d.min = h1.min;
            d.max = h1.max;
            d.bin_size = h1.bin_size;
            fresh.insert(d)
        }
        Some(d) => {
            d.grow(g, h1.n(), "HISTOadd");
            d.nbins = h1.nbins;
            d
        }
    };
    let zeros;
    let h2 = match h2 {
        Some(h2) => h2,
        None => {
            if given {
                g.notices.add(
                    &FS_22,
                    "HISTOadd: no second histogram with an output (FreeSurfer crashes): zeros added",
                );
            }
            zeros = Histogram::zeros(h1.nbins);
            &zeros
        }
    };
    for b in 0..h1.nbins {
        let v = h1.counts[b as usize] + cnt(g, h2, b, "HISTOadd");
        d.counts[b as usize] = v;
    }
    match fresh {
        Some(d) => Ret::New(d),
        None => Ret::Given,
    }
}

/// `HISTOsubtract(h1, h2, dst)`: `counts = h1 - h2` over `h1`'s bins (a new output has zero
/// bins and no range). FS-21.
pub fn histo_subtract(
    g: &mut Globals,
    h1: &Histogram,
    h2: &Histogram,
    dst: Option<&mut Histogram>,
) -> Ret<Histogram> {
    let mut fresh = None;
    let d: &mut Histogram = match dst {
        None => fresh.insert(Histogram::zeros(h1.nbins)),
        Some(d) => {
            d.grow(g, h1.n(), "HISTOsubtract");
            d.nbins = h1.nbins;
            d
        }
    };
    for b in 0..h1.nbins {
        let v = h1.counts[b as usize] - cnt(g, h2, b, "HISTOsubtract");
        d.counts[b as usize] = v;
    }
    match fresh {
        Some(d) => Ret::New(d),
        None => Ret::Given,
    }
}

/// `HISTOclearBins(src, dst, min_val, max_val)`: a copy of `src` (unless `dst` is `src`: see
/// [`histo_clear_bins_in_place`]) with the counts of the bins whose values lie in
/// `[min_val, max_val]` set to 0.
pub fn histo_clear_bins(
    g: &mut Globals,
    src: &Histogram,
    dst: Option<&mut Histogram>,
    min_val: i32,
    max_val: i32,
) -> Ret<Histogram> {
    match dst {
        None => {
            let mut d = histo_copy(g, src, None).unwrap_new();
            histo_clear_bins_in_place(&mut d, min_val, max_val);
            Ret::New(d)
        }
        Some(d) => {
            copy_into(g, src, d);
            histo_clear_bins_in_place(d, min_val, max_val);
            Ret::Given
        }
    }
}

/// `HISTOclearBins(h, h, min_val, max_val)`.
pub fn histo_clear_bins_in_place(h: &mut Histogram, min_val: i32, max_val: i32) {
    let (lo, hi) = (min_val as f32, max_val as f32);
    for b in 0..h.n() {
        if h.bins[b] >= lo && h.bins[b] <= hi {
            h.counts[b] = 0.0;
        }
    }
}

/// `HISTOsmooth`'s kernel: `len` weights (at most 2000), normalised to sum 1 in `float`.
fn smooth_kernel(g: &mut Globals, sigma: f32) -> Vec<f32> {
    let libm = Libm::quiet(g.mode());
    let mut len = nint(g, f64::from(8.0f32 * sigma)) + 1;
    if (len / 2) * 2 == len {
        len += 1;
    }
    if MAX_LEN < len {
        len = MAX_LEN;
    }
    let half = len / 2;
    let two_sigma = 2.0f32 * sigma;
    let mut kernel = Vec::with_capacity(len.max(0) as usize);
    let mut norm = 0.0f32;
    let e2 = (M_E * M_E) as f32;
    for x in 0..len {
        let fx = (x - half) as f32;
        let k = if fx.abs() <= two_sigma {
            libm.exp(f64::from(-fx * fx / (two_sigma * sigma))) as f32
        } else if two_sigma < fx.abs() && fx.abs() <= 4.0f32 * sigma {
            1.0f32 / (16.0f32 * e2)
                * libm.pow(4.0 - f64::from(fx.abs()) / f64::from(sigma), 4.0) as f32
        } else {
            0.0
        };
        kernel.push(k);
        norm += k;
    }
    for k in kernel.iter_mut() {
        *k = sse::div(*k, norm);
    }
    kernel
}

/// `HISTOsmooth(src, dst, sigma)` into another histogram or a new one: each count the
/// kernel-weighted mean of its neighbours (the weights inside the histogram renormalised).
/// A given output with fewer bins in use than `src` is first reallocated (`HISTOrealloc`); its
/// `nbins` and `bin_size` become `src`'s (its `min` and `max` stay).
pub fn histo_smooth(
    g: &mut Globals,
    src: &Histogram,
    dst: Option<&mut Histogram>,
    sigma: f32,
) -> Ret<Histogram> {
    let nbins = src.nbins;
    let mut fresh = None;
    let d: &mut Histogram = match dst {
        // FreeSurfer's `HISTOclearCounts(histo_dst, histo_dst)` here changes nothing seen: every
        // count is written below.
        None => fresh.insert(histo_copy(g, src, None).unwrap_new()),
        Some(d) => {
            if d.nbins < nbins {
                d.bins = vec![0.0; nbins as usize];
                d.counts = vec![0.0; nbins as usize];
                d.max_bins = nbins;
            }
            d.nbins = nbins;
            d.bin_size = src.bin_size;
            d
        }
    };
    let kernel = smooth_kernel(g, sigma);
    let half = kernel.len() as i32 / 2;
    for b in 0..nbins {
        let (mut norm, mut total) = (0.0f32, 0.0f32);
        for (x, &k) in kernel.iter().enumerate() {
            let b1 = b + x as i32 - half;
            if b1 >= nbins || b1 < 0 {
                continue;
            }
            norm += k;
            total += k * src.counts[b1 as usize];
        }
        d.counts[b as usize] = sse::div(total, norm);
        d.bins[b as usize] = src.bins[b as usize];
    }
    match fresh {
        Some(d) => Ret::New(d),
        None => Ret::Given,
    }
}

/// `HISTOsmooth(h, h, sigma)`: as [`histo_smooth`], each count computed from the counts already
/// smoothed below it (FreeSurfer reads the histogram it writes).
pub fn histo_smooth_in_place(g: &mut Globals, h: &mut Histogram, sigma: f32) {
    let nbins = h.nbins;
    let kernel = smooth_kernel(g, sigma);
    let half = kernel.len() as i32 / 2;
    for b in 0..nbins {
        let (mut norm, mut total) = (0.0f32, 0.0f32);
        for (x, &k) in kernel.iter().enumerate() {
            let b1 = b + x as i32 - half;
            if b1 >= nbins || b1 < 0 {
                continue;
            }
            norm += k;
            total += k * h.counts[b1 as usize];
        }
        h.counts[b as usize] = sse::div(total, norm);
    }
}

/// The largest count of the bins in use (`float`, from 0).
fn max_count(h: &Histogram) -> f32 {
    let mut m = 0.0f32;
    for &c in &h.counts[..h.n()] {
        if c > m {
            m = c;
        }
    }
    m
}

/// Whether bin `b` is a peak: no count in `[b - whalf, b + whalf]` (inside the histogram) above
/// its own.
fn is_window_peak(h: &Histogram, b: i32, whalf: i32) -> bool {
    let center = h.counts[b as usize];
    let mut peak = true;
    for bw in b - whalf..=b + whalf {
        if bw < 0 || bw >= h.nbins {
            continue;
        }
        if h.counts[bw as usize] > center {
            peak = false;
        }
    }
    peak
}

/// `HISTOfindLastPeak(h, wsize, min_pct)`: the last bin above `min_pct` times the largest count
/// that no bin within `(wsize - 1) / 2` exceeds; -1 when none (or every count is 0).
pub fn histo_find_last_peak(h: &Histogram, wsize: i32, min_pct: f32) -> i32 {
    let mx = max_count(h);
    if mx == 0.0 {
        return -1;
    }
    let min_count = min_pct * mx;
    let whalf = (wsize - 1) / 2;
    for b in (0..h.nbins).rev() {
        if h.counts[b as usize] <= min_count {
            continue;
        }
        if is_window_peak(h, b, whalf) {
            return b;
        }
    }
    -1
}

/// `HISTOfindFirstPeak(h, wsize, min_pct)`: as [`histo_find_last_peak`], from the first bin.
pub fn histo_find_first_peak(h: &Histogram, wsize: i32, min_pct: f32) -> i32 {
    let mx = max_count(h);
    if mx == 0.0 {
        return -1;
    }
    let min_count = min_pct * mx;
    let whalf = (wsize - 1) / 2;
    for b in 0..h.nbins {
        if h.counts[b as usize] <= min_count {
            continue;
        }
        if is_window_peak(h, b, whalf) {
            return b;
        }
    }
    -1
}

/// The bins of `I0` and `I1` as the region searches find them: the bin before the first whose
/// value reaches `I0`, the bin after the first whose value reaches `I1` (-1 when none).
fn region_bins(h: &Histogram, i0: i32, i1: i32) -> (i32, i32) {
    let (mut b0, mut b1) = (-1, -1);
    let (f0, f1) = (i0 as f32, i1 as f32);
    for b in 0..h.nbins {
        let v = h.bins[b as usize];
        if v >= f0 && b0 == -1 {
            b0 = if b > 0 { b - 1 } else { b };
        }
        if v >= f1 && b1 == -1 {
            b1 = if b < h.nbins - 1 { b + 1 } else { b };
        }
    }
    (b0, b1)
}

/// `HISTOfindValley(h, wsize, I0, I1)`: the first bin from `I0`'s to `I1`'s whose count (as an
/// `int`, FS-17) no bin within `(wsize - 1) / 2` undercuts; -1 when none or every count is 0.
pub fn histo_find_valley(g: &mut Globals, h: &Histogram, wsize: i32, i0: i32, i1: i32) -> i32 {
    let (mut b0, mut b1) = region_bins(h, i0, i1);
    let mut mx = 0i32;
    for b in 0..h.nbins {
        let c = float_to_int(g, f64::from(h.counts[b as usize]), "HISTOfindValley");
        if c > mx {
            mx = c;
        }
    }
    if mx == 0 {
        return -1;
    }
    let whalf = (wsize - 1) / 2;
    let nbins = h.nbins;
    if b0 < 0 {
        b0 = 0;
    }
    if b1 < 0 || b1 >= nbins {
        b1 = nbins - 1;
    }
    for b in b0..=b1 {
        let center = float_to_int(g, f64::from(h.counts[b as usize]), "HISTOfindValley");
        let mut valley = true;
        for bw in b - whalf..=b + whalf {
            if bw < 0 || bw >= nbins {
                continue;
            }
            let other = float_to_int(g, f64::from(h.counts[bw as usize]), "HISTOfindValley");
            if other < center {
                valley = false;
            }
        }
        if valley {
            return b;
        }
    }
    -1
}

/// `HISTOfindLastPeakInRegion(h, wsize, min_pct, I0, I1)`: from `I1`'s bin down to `I0`'s, the
/// first bin that no neighbour within `(wsize - 1) / 2` exceeds, whose window's mean count (the
/// window's sum over `wsize`) reaches `min_pct` times the largest count and lies at least 1.9
/// below its own count; -1 when none. (When no bin reaches `I0`, FreeSurfer's search ends at bin
/// -1, read outside the histogram; either way the result is -1, so the translation stops at 0.)
pub fn histo_find_last_peak_in_region(
    h: &Histogram,
    wsize: i32,
    min_pct: f32,
    i0: i32,
    i1: i32,
) -> i32 {
    let (b0, mut b1) = region_bins(h, i0, i1);
    if b1 == -1 {
        b1 = h.nbins - 1;
    }
    let mx = max_count(h);
    if mx == 0.0 {
        return -1;
    }
    let min_count = min_pct * mx;
    let whalf = (wsize - 1) / 2;
    let nbins = h.nbins;
    let mut b = b1;
    while b >= b0 && b >= 0 {
        let center = h.counts[b as usize];
        let mut peak = true;
        let mut total = 0.0f32;
        for bw in b - whalf..=b + whalf {
            if bw < 0 || bw >= nbins {
                continue;
            }
            let other = h.counts[bw as usize];
            total += other;
            if other > center {
                peak = false;
                break;
            }
        }
        if peak && total / wsize as f32 >= min_count {
            let mean_count = total / wsize as f32;
            if f64::from(center - mean_count) >= MIN_STD {
                return b;
            }
        }
        b -= 1;
    }
    -1
}

/// `HISTOfindHighestPeakInRegion(h, b0, b1)`: the bin of the largest count above 0 in
/// `[b0, b1]` (clamped to the histogram), the first of equal ones; -1 when none.
pub fn histo_find_highest_peak_in_region(h: &Histogram, b0: i32, b1: i32) -> i32 {
    let b0 = b0.max(0);
    let b1 = if b1 >= h.nbins { h.nbins - 1 } else { b1 };
    let mut mx = 0.0f32;
    let mut best = -1;
    for b in b0..=b1 {
        let v = h.counts[b as usize];
        if v > mx {
            best = b;
            mx = v;
        }
    }
    best
}

/// `HISTOplot(histo, fname)`: `"%f  %10.10f\n"` of each bin's value and count from the first
/// bin with a count above 0 to the last (`dir`: the run's directory for a relative name);
/// `ERROR_NOFILE` when the file cannot be opened.
pub fn histo_plot(histo: &Histogram, dir: Option<&Path>, fname: &str) -> i32 {
    let Ok(mut fp) = std::fs::File::create(resolve(dir, fname)) else {
        return ERROR_NOFILE;
    };
    let mut bmin = 0;
    while bmin < histo.nbins {
        if histo.counts[bmin as usize] > 0.0 {
            break;
        }
        bmin += 1;
    }
    let mut bmax = histo.nbins - 1;
    while bmax > bmin {
        if histo.counts[bmax as usize] > 0.0 {
            break;
        }
        bmax -= 1;
    }
    let mut text = String::new();
    for b in bmin..=bmax {
        text.push_str(&fmt_f(f64::from(histo.bins[b as usize]), 0, 6));
        text.push_str("  ");
        text.push_str(&fmt_f(f64::from(histo.counts[b as usize]), 10, 10));
        text.push('\n');
    }
    let _ = fp.write_all(text.as_bytes());
    0
}

/// `HISTOaddSample(histo, val, bmin, bmax)`: adds one sample, shared between the two bins
/// around it; `bmin >= bmax` takes the histogram's own range and bin size, else the bin size is
/// `(bmax - bmin) / (nbins - 1)`. Returns the sample's nearest bin number. FS-19, FS-20.
pub fn histo_add_sample(
    g: &mut Globals,
    histo: &mut Histogram,
    val: f32,
    bmin: f32,
    bmax: f32,
) -> i32 {
    let (bmin, bin_size) = if bmin >= bmax {
        (histo.min, histo.bin_size)
    } else {
        (bmin, sse::div(bmax - bmin, histo.nbins as f32 - 1.0))
    };
    let reproduce = g.notices.reproduce();
    let x = sse::div(val - bmin, bin_size);
    let bin_no = nint(g, f64::from(x));
    if !reproduce && x.is_nan() {
        g.notices.add(
            &FS_19,
            "HISTOaddSample: a sample at no position (NaN) not counted",
        );
        return bin_no;
    }
    if bin_no < histo.nbins && bin_no >= 0 {
        let dbin = f64::from(x);
        let dp = dbin - dbin.floor();
        let dn = dbin.ceil() - dbin;
        let mut bp = float_to_int(g, dbin.floor(), "HISTOaddSample");
        let mut bn = float_to_int(g, dbin.ceil(), "HISTOaddSample");
        if bp < 0 {
            bp = 0;
        }
        if bn >= histo.nbins {
            bn = histo.nbins - 1;
        }
        if dp == 0.0 && dn == 0.0 {
            g.notices.add(
                &FS_20,
                if reproduce {
                    "HISTOaddSample: a sample exactly at a bin's value not counted"
                } else {
                    "HISTOaddSample: a sample exactly at a bin's value counted once (FreeSurfer adds 0 twice)"
                },
            );
            if !reproduce {
                let c = &mut histo.counts[bp as usize];
                *c = (f64::from(*c) + 1.0) as f32;
                return bin_no;
            }
        }
        let c = &mut histo.counts[bn as usize];
        *c = (f64::from(*c) + dp) as f32;
        let c = &mut histo.counts[bp as usize];
        *c = (f64::from(*c) + dn) as f32;
        return bin_no;
    }
    let last = histo.nbins - 1;
    if reproduce {
        if (bin_no as f32) < bmin {
            if bin_no >= histo.nbins {
                g.notices.add(
                    &FS_19,
                    "HISTOaddSample: a sample above the range counted in the first bin",
                );
            }
            let v = cnt(g, histo, 0, "HISTOaddSample") + 1.0;
            set_cnt(g, histo, 0, v, "HISTOaddSample");
        } else if bin_no >= histo.nbins {
            let v = cnt(g, histo, last, "HISTOaddSample") + 1.0;
            set_cnt(g, histo, last, v, "HISTOaddSample");
        } else {
            g.notices
                .add(&FS_19, "HISTOaddSample: a sample below the range dropped");
        }
    } else {
        let original = if (bin_no as f32) < bmin {
            0
        } else if bin_no >= histo.nbins {
            last
        } else {
            -1
        };
        let b = if bin_no < 0 { 0 } else { last };
        if original != b {
            g.notices.add(
                &FS_19,
                "HISTOaddSample: a sample outside the range counted in the end bin on its side",
            );
        }
        let v = cnt(g, histo, b, "HISTOaddSample") + 1.0;
        set_cnt(g, histo, b, v, "HISTOaddSample");
    }
    bin_no
}

/// `HISTOfindPreviousValley(h, b0)`: going down from `b0`, the first bin whose count exceeds
/// the one above it; `b0` itself when `b0 <= 0`; -1 when none.
pub fn histo_find_previous_valley(g: &mut Globals, h: &Histogram, b0: i32) -> i32 {
    if b0 <= 0 {
        return b0;
    }
    let mut prev = cnt(g, h, b0, "HISTOfindPreviousValley");
    for b in (0..b0).rev() {
        let v = cnt(g, h, b, "HISTOfindPreviousValley");
        if v > prev {
            return b;
        }
        prev = v;
    }
    -1
}

/// `HISTOfindNextValley(h, b0)`: going up from `b0`, the first bin whose count exceeds the one
/// below it; `b0` itself when `b0 >= nbins`; -1 when none.
pub fn histo_find_next_valley(g: &mut Globals, h: &Histogram, b0: i32) -> i32 {
    if b0 >= h.nbins {
        return b0;
    }
    let mut prev = cnt(g, h, b0, "HISTOfindNextValley");
    for b in b0 + 1..h.nbins {
        let v = h.counts[b as usize];
        if v > prev {
            return b;
        }
        prev = v;
    }
    -1
}

/// `HISTOfindNextPeak(h, b0, whalf)`: going up from `b0 + 1` to `nbins - whalf`, the first bin
/// that no bin within `whalf` exceeds (the window is not clipped: FS-22 past the ends); `b0`
/// when `b0 > nbins - 2`; -1 when none.
pub fn histo_find_next_peak(g: &mut Globals, h: &Histogram, b0: i32, whalf: i32) -> i32 {
    if b0 > h.nbins - 2 {
        return b0;
    }
    let mut peak = false;
    let mut b = b0 + 1;
    while b <= h.nbins - whalf {
        let val = cnt(g, h, b, "HISTOfindNextPeak");
        peak = true;
        for bk in b - whalf..=b + whalf {
            if cnt(g, h, bk, "HISTOfindNextPeak") > val {
                peak = false;
                break;
            }
        }
        if peak {
            break;
        }
        b += 1;
    }
    if peak { b } else { -1 }
}

/// `HISTOfindPreviousPeak(h, b0, whalf)`: going down from `b0 - 1` to `whalf`, the first bin
/// that no bin within `whalf` exceeds (FS-22 past the top); `b0` when `b0 > nbins - 2`; -1 when
/// none.
pub fn histo_find_previous_peak(g: &mut Globals, h: &Histogram, b0: i32, whalf: i32) -> i32 {
    if b0 > h.nbins - 2 {
        return b0;
    }
    let mut peak = false;
    let mut b = b0 - 1;
    while b >= whalf {
        let val = cnt(g, h, b, "HISTOfindPreviousPeak");
        peak = true;
        for bk in b - whalf..=b + whalf {
            if cnt(g, h, bk, "HISTOfindPreviousPeak") > val {
                peak = false;
                break;
            }
        }
        if peak {
            break;
        }
        b -= 1;
    }
    if peak { b } else { -1 }
}

/// `HISTOfindStartOfPeak(h, b0, pct_peak)`: going down from `b0 - 1` to the previous valley,
/// the first bin below `pct_peak` times `b0`'s count; else the bin below the valley.
pub fn histo_find_start_of_peak(g: &mut Globals, h: &Histogram, b0: i32, pct_peak: f32) -> i32 {
    let b1 = histo_find_previous_valley(g, h, b0);
    if b0 <= 0 {
        return b0;
    }
    let thresh = cnt(g, h, b0, "HISTOfindStartOfPeak") * pct_peak;
    let mut b = b0 - 1;
    while b >= b1 {
        if cnt(g, h, b, "HISTOfindStartOfPeak") < thresh {
            return b;
        }
        b -= 1;
    }
    b
}

/// `HISTOfindEndOfPeak(h, b0, pct_peak)`: going up from `b0 + 1` to the next valley, the first
/// bin below `pct_peak` times `b0`'s count; else the last bin (FreeSurfer's
/// `if (h->nbins >= b)`, kept).
pub fn histo_find_end_of_peak(g: &mut Globals, h: &Histogram, b0: i32, pct_peak: f32) -> i32 {
    let b1 = histo_find_next_valley(g, h, b0);
    if b0 >= h.nbins {
        return b0;
    }
    let thresh = cnt(g, h, b0, "HISTOfindEndOfPeak") * pct_peak;
    let mut b = b0 + 1;
    while b <= b1 {
        if cnt(g, h, b, "HISTOfindEndOfPeak") < thresh {
            return b;
        }
        b += 1;
    }
    if h.nbins >= b {
        b = h.nbins - 1;
    }
    b
}

/// `HISTOfindCurrentPeak(histo, b0, wsize, min_pct)`: the peak `b0` belongs to, searched
/// forwards when the counts after `b0` (within the window) outweigh those before it, else
/// backwards; -1 when none.
pub fn histo_find_current_peak(h: &Histogram, b0: i32, wsize: i32, min_pct: f32) -> i32 {
    let nbins = h.nbins;
    let peak = histo_find_highest_peak_in_region(h, 0, nbins);
    if peak < 0 {
        return -1;
    }
    let min_count = min_pct * h.counts[peak as usize];
    let whalf = (wsize - 1) / 2;
    let (mut next, mut prev) = (0.0f32, 0.0f32);
    for bw in b0 - whalf..=b0 + whalf {
        if bw < 0 || bw >= nbins {
            continue;
        }
        if bw < b0 {
            prev += h.counts[bw as usize];
        } else if bw > b0 {
            next += h.counts[bw as usize];
        }
    }
    if next > prev {
        for b in b0 - whalf..nbins {
            if b < 0 {
                continue;
            }
            let center = h.counts[b as usize];
            if center < min_count {
                continue;
            }
            let mut is_peak = true;
            for bw in b..=b + whalf {
                if bw < 0 || bw >= nbins {
                    continue;
                }
                if h.counts[bw as usize] > center {
                    is_peak = false;
                    break;
                }
            }
            if is_peak {
                return b;
            }
        }
    } else {
        let mut b = b0;
        while b >= 0 {
            if b < nbins {
                let center = h.counts[b as usize];
                if center >= min_count {
                    let mut is_peak = true;
                    for bw in b - whalf..=b + whalf {
                        if bw < 0 || bw >= nbins {
                            continue;
                        }
                        if h.counts[bw as usize] > center {
                            is_peak = false;
                            break;
                        }
                    }
                    if is_peak {
                        return b;
                    }
                }
            }
            b -= 1;
        }
    }
    -1
}

/// `HISTOfillHoles(h)`: the empty bins between the first and last filled one (bins 1 to
/// `nbins - 2`) repeatedly set to their neighbours' mean until no change exceeds 1 % (at most
/// 101 sweeps); an empty end bin takes its neighbour's count. `NO_ERROR`.
pub fn histo_fill_holes(g: &mut Globals, h: &mut Histogram) -> i32 {
    let nbins = h.nbins;
    let mut filled = vec![false; h.n()];
    let (mut min_b, mut max_b) = (nbins, 0);
    for b in 1..nbins - 1 {
        if h.counts[b as usize] > 0.0 {
            filled[b as usize] = true;
            max_b = max_b.max(b);
            min_b = min_b.min(b);
        }
    }
    let mut niter = 0;
    loop {
        let mut max_change = 0.0f64;
        for b in min_b..=max_b {
            if !filled[b as usize] {
                let u = b as usize;
                let new_val = f64::from((h.counts[u - 1] + h.counts[u + 1]) / 2.0f32);
                let change = if h.counts[u] == 0.0 {
                    1.0
                } else {
                    sse::div64(
                        (new_val - f64::from(h.counts[u])).abs(),
                        f64::from(h.counts[u]).abs(),
                    )
                };
                h.counts[u] = new_val as f32;
                if change > max_change {
                    max_change = change;
                }
            }
        }
        let go_on = max_change > 0.01 && niter < 100;
        niter += 1;
        if !go_on {
            break;
        }
    }
    if cnt(g, h, 0, "HISTOfillHoles") == 0.0 {
        let v = cnt(g, h, 1, "HISTOfillHoles");
        set_cnt(g, h, 0, v, "HISTOfillHoles");
    }
    if cnt(g, h, nbins - 1, "HISTOfillHoles") == 0.0 {
        let v = cnt(g, h, nbins - 2, "HISTOfillHoles");
        set_cnt(g, h, nbins - 1, v, "HISTOfillHoles");
    }
    0
}

/// `HISTOtotalInRegion(h, b0, b1)`: the `float` sum of the counts of `[b0, b1]` (FS-22 outside).
pub fn histo_total_in_region(g: &mut Globals, h: &Histogram, b0: i32, b1: i32) -> f32 {
    let mut total = 0.0f32;
    let mut b = b0;
    while b <= b1 {
        total += cnt(g, h, b, "HISTOtotalInRegion");
        b += 1;
    }
    total
}

/// `HISTOtotal(h)`: [`histo_total_in_region`] over every bin in use.
pub fn histo_total(g: &mut Globals, h: &Histogram) -> f32 {
    histo_total_in_region(g, h, 0, h.nbins - 1)
}

/// `HISTOclearZeroBin(h)`: unless the first bin's value exceeds the bin size, the count of the
/// first bin whose value is at least 0 set to 0 (none: FreeSurfer writes the bin after the last,
/// FS-22). `NO_ERROR`.
pub fn histo_clear_zero_bin(g: &mut Globals, h: &mut Histogram) -> i32 {
    if bin_value(g, h, 0, "HISTOclearZeroBin") > h.bin_size {
        return 0;
    }
    let mut b = 0;
    while b < h.nbins {
        if h.bins[b as usize] >= 0.0 {
            break;
        }
        b += 1;
    }
    set_cnt(g, h, b, 0.0, "HISTOclearZeroBin");
    0
}

/// `HISTOfindBinWithCount(h, val)`: the first bin whose count is nearest `val` (distances in
/// `float`).
pub fn histo_find_bin_with_count(g: &mut Globals, h: &Histogram, val: f32) -> i32 {
    let mut min_b = 0;
    let mut min_dist = f64::from((cnt(g, h, 0, "HISTOfindBinWithCount") - val).abs());
    for b in 1..h.nbins {
        let dist = f64::from((h.counts[b as usize] - val).abs());
        if dist < min_dist {
            min_dist = dist;
            min_b = b;
        }
    }
    min_b
}

/// `HISTOfindBin(h, val)`: with a bin size of exactly 1, `(int)val - bins[0]` (in `float`,
/// converted to `int`: FS-17); else the last bin whose predecessor's value is below `val`, or 0.
pub fn histo_find_bin(g: &mut Globals, h: &Histogram, val: f32) -> i32 {
    if h.bin_size == 1.0 {
        let iv = float_to_int(g, f64::from(val), "HISTOfindBin");
        let d = iv as f32 - bin_value(g, h, 0, "HISTOfindBin");
        return float_to_int(g, f64::from(d), "HISTOfindBin");
    }
    let mut b = h.nbins - 1;
    while b > 0 {
        if h.bins[(b - 1) as usize] < val {
            return b;
        }
        b -= 1;
    }
    0
}

/// `HISTOgetCount(h, bin_val)`: the count of [`histo_find_bin`]'s bin, 0 outside the histogram.
pub fn histo_get_count(g: &mut Globals, h: &Histogram, bin_val: f32) -> f64 {
    let b = histo_find_bin(g, h, bin_val);
    if b < 0 || b >= h.nbins {
        return 0.0;
    }
    f64::from(h.counts[b as usize])
}

/// `HISTOcorrelate(h1, h2)`: the normalised correlation of `h1`'s counts with `h2`'s counts at
/// the same values ([`histo_find_bin`]); more than 256 bins in `h2` end the program
/// (`ERROR_UNSUPPORTED`, with `histoComputeLinearFitCorrelation`'s name in the message).
pub fn histo_correlate(
    g: &mut Globals,
    err: &mut dyn Write,
    h1: &Histogram,
    h2: &Histogram,
) -> Result<f64, Fatal> {
    if h2.nbins > 256 {
        log::error_exit(
            g,
            err,
            ERROR_UNSUPPORTED,
            "histoComputeLinearFitCorrelation: only 256 bins allowed",
        )?;
    }
    let (mut corr, mut norm1, mut norm2) = (0.0f64, 0.0f64, 0.0f64);
    for b1 in 0..h1.nbins {
        let b2 = histo_find_bin(g, h2, h1.bins[b1 as usize]);
        let c2 = if b2 < 0 || b2 > h2.nbins - 1 {
            0.0
        } else {
            f64::from(h2.counts[b2 as usize])
        };
        let c1 = f64::from(h1.counts[b1 as usize]);
        corr += c1 * c2;
        norm1 += c1 * c1;
        norm2 += c2 * c2;
    }
    if norm1.abs() < FLT_EPSILON {
        norm1 = 1.0;
    }
    if norm2.abs() < FLT_EPSILON {
        norm2 = 1.0;
    }
    corr /= norm1.sqrt() * norm2.sqrt();
    Ok(corr)
}

/// `histoComputeLinearFitCorrelation(h1, h2, a, b)`: the sum of `h1`'s counts times `h2`'s at
/// bin `nint(b1 a + b)`, plus the same through the inverse map `nint((b2 - b) / a)`.
pub fn histo_compute_linear_fit_correlation(
    g: &mut Globals,
    h1: &Histogram,
    h2: &Histogram,
    a: f64,
    b: f64,
) -> f64 {
    let mut corr = 0.0f64;
    for b1 in 0..h1.nbins {
        let b2 = nint(g, f64::from(b1) * a + b);
        let c2 = if b2 < 0 || b2 > h2.nbins - 1 {
            0.0
        } else {
            f64::from(h2.counts[b2 as usize])
        };
        corr += f64::from(h1.counts[b1 as usize]) * c2;
    }
    for b2 in 0..h2.nbins {
        let b1 = nint(g, sse::div64(f64::from(b2) - b, a));
        let c1 = if b1 < 0 || b1 > h1.nbins - 1 {
            0.0
        } else {
            f64::from(h1.counts[b1 as usize])
        };
        corr += c1 * f64::from(h2.counts[b2 as usize]);
    }
    corr
}

/// `HISTOfindLinearFit(h1, h2, amin, amax, bmin, bmax, &a, &b)`: the `a`, `b` (as `float`s)
/// of the best [`histo_compute_linear_fit_correlation`] on a grid (`a` from `amin` by
/// `MAX(MIN(amin, (amax - amin) / 1000), 0.01)`, `b` from `bmin` by `MAX((bmax - bmin) / 1000,
/// 0.01)`), starting from `a = 1`, `b = 0`; and that correlation.
#[allow(clippy::too_many_arguments)]
pub fn histo_find_linear_fit(
    g: &mut Globals,
    h1: &Histogram,
    h2: &Histogram,
    amin: f64,
    amax: f64,
    bmin: f64,
    bmax: f64,
) -> (f64, f32, f32) {
    let mut best = histo_compute_linear_fit_correlation(g, h1, h2, 1.0, 0.0);
    let (mut best_a, mut best_b) = (1.0f64, 0.0f64);
    let t = (amax - amin) / NSTEPS;
    let astep = if amin < t { amin } else { t };
    let astep = if astep >= 0.01 { astep } else { 0.01 };
    let bstep = (bmax - bmin) / NSTEPS;
    let bstep = if bstep >= 0.01 { bstep } else { 0.01 };
    let mut a = amin;
    while a <= amax {
        let mut b = bmin;
        while b <= bmax {
            let c = histo_compute_linear_fit_correlation(g, h1, h2, a, b);
            if c > best {
                best = c;
                best_a = a;
                best_b = b;
            }
            b += bstep;
        }
        a += astep;
    }
    (best, best_a as f32, best_b as f32)
}

/// `HISTOlinearScale(hsrc, hdst, scale, offset)`: a copy of `hsrc` with every bin's value,
/// `min` and `max` mapped by `v scale + offset` and the bin size scaled (in `float`).
pub fn histo_linear_scale(
    g: &mut Globals,
    hsrc: &Histogram,
    hdst: Option<&mut Histogram>,
    scale: f32,
    offset: f32,
) -> Ret<Histogram> {
    match hdst {
        None => {
            let mut d = histo_copy(g, hsrc, None).unwrap_new();
            scale_into(hsrc, &mut d, scale, offset);
            Ret::New(d)
        }
        Some(d) => {
            copy_into(g, hsrc, d);
            scale_into(hsrc, d, scale, offset);
            Ret::Given
        }
    }
}

/// `HISTOlinearScale(h, h, scale, offset)`.
pub fn histo_linear_scale_in_place(h: &mut Histogram, scale: f32, offset: f32) {
    let src = h.clone();
    scale_into(&src, h, scale, offset);
}

fn scale_into(src: &Histogram, d: &mut Histogram, scale: f32, offset: f32) {
    for b in 0..d.n() {
        d.bins[b] = d.bins[b] * scale + offset;
    }
    d.min = src.min * scale + offset;
    d.max = src.max * scale + offset;
    d.bin_size = src.bin_size * scale;
}

/// `HISTOthreshSum(h_mask, h_src, m_thresh)`: the `float` sum of `h_src`'s counts at the bins
/// where `h_mask`'s count exceeds `m_thresh`.
pub fn histo_thresh_sum(
    g: &mut Globals,
    h_mask: &Histogram,
    h_src: &Histogram,
    m_thresh: f32,
) -> f32 {
    let mut total = 0.0f32;
    for b in 0..h_mask.nbins {
        if h_mask.counts[b as usize] > m_thresh {
            total += cnt(g, h_src, b, "HISTOthreshSum");
        }
    }
    total
}

/// `HISTOmakePDF(h_src, h_dst)` with no output or another one (FS-23): the counts divided by
/// their `float` sum when it is above 0, a count that becomes (nearly) 0 set to
/// `1 / (10 total)`.
pub fn histo_make_pdf(
    g: &mut Globals,
    h_src: &Histogram,
    h_dst: Option<&mut Histogram>,
) -> Ret<Histogram> {
    match h_dst {
        None => {
            let mut d = histo_copy(g, h_src, None).unwrap_new();
            histo_make_pdf_in_place(&mut d);
            Ret::New(d)
        }
        Some(d) => {
            if g.notices.reproduce() {
                g.notices.add(
                    &FS_23,
                    "HISTOmakePDF: the output histogram's own counts normalised, not the source's",
                );
            } else {
                copy_into(g, h_src, d);
                g.notices.add(
                    &FS_23,
                    "HISTOmakePDF: the source copied into the output first (FreeSurfer normalises the output's own counts)",
                );
            }
            histo_make_pdf_in_place(d);
            Ret::Given
        }
    }
}

/// `HISTOmakePDF(h, h)`.
pub fn histo_make_pdf_in_place(h: &mut Histogram) {
    let n = h.n();
    let mut total = 0.0f32;
    for &c in &h.counts[..n] {
        total += c;
    }
    if total > 0.0 {
        for c in &mut h.counts[..n] {
            *c = sse::div(*c, total);
            if f64::from(*c).abs() < 1e-15 {
                *c = (1.0f64 / f64::from(10.0f32 * total)) as f32;
            }
        }
    }
}

/// `HISTObins(nbins, min, max)`: a new histogram with `bin_size = (max - min) / (nbins - 1)`
/// and bin `n`'s value `min + bin_size n` (the product in `float`).
pub fn histo_bins(
    g: &mut Globals,
    err: &mut dyn Write,
    nbins: i32,
    min: f64,
    max: f64,
) -> Result<Histogram, Fatal> {
    let mut h = histo_alloc(g, err, nbins)?;
    h.bin_size = sse::div64(max - min, f64::from(nbins - 1)) as f32;
    for n in 0..h.n() {
        h.bins[n] = (min + f64::from(h.bin_size * n as f32)) as f32;
    }
    h.min = min as f32;
    h.max = max as f32;
    Ok(h)
}

/// `HISTOcount(h, samples, nsamples)`: each sample adds 1 to its nearest bin
/// ([`histo_val_to_bin`]). 0.
pub fn histo_count(g: &mut Globals, h: &mut Histogram, samples: &[f64]) -> i32 {
    for &s in samples {
        let b = histo_val_to_bin(h, s);
        let v = cnt(g, h, b, "HISTOcount") + 1.0;
        set_cnt(g, h, b, v, "HISTOcount");
    }
    0
}

/// `HISTOvalToBin(h, val)`: the first bin whose value is nearest `val` (distances in `double`).
pub fn histo_val_to_bin(h: &Histogram, val: f64) -> i32 {
    let mut bin = 0;
    let Some(&b0) = h.bins.first() else {
        return 0;
    };
    let mut dmin = (f64::from(b0) - val).abs();
    for n in 0..h.nbins {
        let d = (f64::from(h.bins[n as usize]) - val).abs();
        if dmin > d {
            dmin = d;
            bin = n;
        }
    }
    bin
}

/// `HISTOinit(h, nbins, mn, mx)`: a new histogram of `nbins` (or `h` cleared, its `nbins` left
/// as it is), `min = mn`, `max = mx` (0 and `max_bins - 1` when `mx <= mn`),
/// `bin_size = (mx - mn) / (nbins - 1)` (1 when not above 0), bin `b`'s value
/// `mn + bin_size b` (the product in `float`). `nbins` beyond `max_bins` ends the program.
pub fn histo_init(
    g: &mut Globals,
    err: &mut dyn Write,
    h: Option<&mut Histogram>,
    nbins: i32,
    mn: f64,
    mx: f64,
) -> Result<Ret<Histogram>, Fatal> {
    let mut fresh = None;
    let h: &mut Histogram = match h {
        None => fresh.insert(histo_alloc(g, err, nbins)?),
        Some(h) => {
            histo_clear_in_place(h);
            h
        }
    };
    if nbins > h.max_bins {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "HISTOinit: nbins {nbins} bigger than max bins {}\n",
                h.max_bins
            ),
        )?;
    }
    let (mut mn, mut mx) = (mn, mx);
    if mx <= mn {
        mx = f64::from(h.max_bins - 1);
        mn = 0.0;
    }
    h.min = mn as f32;
    h.max = mx as f32;
    h.bin_size = sse::div64(mx - mn, f64::from(nbins - 1)) as f32;
    if h.bin_size <= 0.0 {
        h.bin_size = 1.0;
    }
    for b in 0..nbins.max(0) {
        let v = (mn + f64::from(h.bin_size * b as f32)) as f32;
        if let Some(p) = h.bins.get_mut(b as usize) {
            *p = v;
        }
    }
    Ok(match fresh {
        Some(h) => Ret::New(h),
        None => Ret::Given,
    })
}

/// `HISTOfindMedian(h)`: the value of the bin where the running count passes half the total
/// (of it or the bin before, whichever the half lies nearer); 0 when none.
pub fn histo_find_median(h: &Histogram) -> f64 {
    let n = h.n();
    let mut total = 0.0f64;
    for &c in &h.counts[..n] {
        total += f64::from(c);
    }
    let mut median = 0.0f64;
    let mut total2 = 0.0f64;
    for b in 0..n {
        let c = f64::from(h.counts[b]);
        if total2 < total / 2.0 && total2 + c > total / 2.0 {
            let d1 = total / 2.0 - total2;
            let d2 = total2 + c - total / 2.0;
            median = if d1 > d2 || b == 0 {
                f64::from(h.bins[b])
            } else {
                f64::from(h.bins[b - 1])
            };
            break;
        }
        total2 += c;
    }
    median
}

/// `HISTOmakeReverseCDF(hsrc, hdst)` and `HISTOmakeCDF(hsrc, hdst)`: for each output bin of a
/// value at least 0, the `double` sum of [`histo_abs`]'s counts at absolute values at or above
/// (reverse) or at or below it; 0 for negative values; then every count divided by the last
/// sum computed.
fn make_cdf(
    g: &mut Globals,
    err: &mut dyn Write,
    hsrc: &Histogram,
    hdst: Option<&mut Histogram>,
    reverse: bool,
) -> Result<Ret<Histogram>, Fatal> {
    let habs = histo_abs(g, err, hsrc, None)?.unwrap_new();
    let mut fresh = None;
    let d: &mut Histogram = match hdst {
        None => fresh.insert(histo_copy(g, hsrc, None).unwrap_new()),
        Some(d) => d,
    };
    let mut total = 0.0f64;
    let order: Vec<i32> = if reverse {
        (0..d.nbins).rev().collect()
    } else {
        (0..d.nbins).collect()
    };
    for b in order {
        let bv = d.bins[b as usize];
        if bv >= 0.0 {
            total = 0.0;
            let inner: Vec<i32> = if reverse {
                (0..habs.nbins).rev().collect()
            } else {
                (0..habs.nbins).collect()
            };
            for b2 in inner {
                let a = habs.bins[b2 as usize].abs();
                if (reverse && a >= bv) || (!reverse && a <= bv) {
                    total += f64::from(habs.counts[b2 as usize]);
                }
            }
            d.counts[b as usize] = total as f32;
        } else {
            d.counts[b as usize] = 0.0;
        }
    }
    for b in 0..d.n() {
        d.counts[b] = sse::div64(f64::from(d.counts[b]), total) as f32;
    }
    Ok(match fresh {
        Some(d) => Ret::New(d),
        None => Ret::Given,
    })
}

/// `HISTOmakeReverseCDF(hsrc, hdst)` (see `make_cdf`).
pub fn histo_make_reverse_cdf(
    g: &mut Globals,
    err: &mut dyn Write,
    hsrc: &Histogram,
    hdst: Option<&mut Histogram>,
) -> Result<Ret<Histogram>, Fatal> {
    make_cdf(g, err, hsrc, hdst, true)
}

/// `HISTOmakeCDF(hsrc, hdst)` (see `make_cdf`).
pub fn histo_make_cdf(
    g: &mut Globals,
    err: &mut dyn Write,
    hsrc: &Histogram,
    hdst: Option<&mut Histogram>,
) -> Result<Ret<Histogram>, Fatal> {
    make_cdf(g, err, hsrc, hdst, false)
}

/// `HISTOrobustGaussianFit(h, max_percentile, &offset, &sigma)`: the mode of `h` (its smoothed
/// highest peak) as the offset, and the standard deviation of the Gaussian that best fits the
/// counts around it (a grid of sigmas in steps of a tenth of the bin size). A bin size not above
/// 0 is set to 1 in `h`. `ERROR_BADPARM` (after its message) for a histogram of 0 bins, else
/// `NO_ERROR`; `None` for the outputs FreeSurfer leaves unset. FS-22, FS-24.
pub fn histo_robust_gaussian_fit(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    h: &mut Histogram,
    max_percentile: f64,
) -> Result<(i32, Option<f64>, Option<f64>), Fatal> {
    let libm = Libm::quiet(g.mode());
    if h.bin_size <= 0.0 {
        h.bin_size = 1.0;
    }
    let sqrt_2pi = (2.0 * std::f64::consts::PI).sqrt();
    let hs = histo_smooth(g, h, None, (2.0 / f64::from(h.bin_size)) as f32).unwrap_new();
    let peak = histo_find_highest_peak_in_region(&hs, 0, hs.nbins);
    let mean = f64::from(bin_value(g, &hs, peak, "HISTOrobustGaussianFit"));
    let offset = mean;
    let hz = histo_linear_scale(g, h, None, 1.0, (-mean) as f32).unwrap_new();
    let habs = histo_abs(g, err, &hz, None)?.unwrap_new();
    let hcdf = histo_make_cdf(g, err, &hz, None)?.unwrap_new();
    let diag = g.gdiag & DIAG_WRITE != 0 && verbose_on(g);
    if diag {
        histo_plot(&habs, dir, "ha.plt");
        histo_plot(&hcdf, dir, "hc.plt");
        histo_plot(h, dir, "h.plt");
        histo_plot(&hs, dir, "hs.plt");
        histo_plot(&hz, dir, "hz.plt");
    }
    let start = float_to_int(
        g,
        f64::from(peak as f32 + 10.0f32 / hs.bin_size),
        "HISTOrobustGaussianFit",
    );
    let mut bin = histo_find_next_valley(g, &hs, start);
    let thresh: f64 = if bin < 0 {
        bin = histo_find_bin_with_count(g, &hcdf, max_percentile as f32);
        f64::from(bin_value(g, &hcdf, bin, "HISTOrobustGaussianFit"))
    } else {
        let peak_count = cnt(g, h, peak, "HISTOrobustGaussianFit");
        while f64::from(cnt(g, h, bin, "HISTOrobustGaussianFit"))
            < f64::from(peak_count) * max_percentile
            && bin > peak + 1
        {
            bin -= 1;
        }
        f64::from(
            bin_value(g, h, bin, "HISTOrobustGaussianFit")
                - bin_value(g, h, peak, "HISTOrobustGaussianFit"),
        )
    };
    let mut min_bin = histo_find_bin(g, &hz, (-thresh) as f32);
    let mut max_bin = histo_find_bin(g, &hz, thresh as f32);
    if hz.nbins == 0 {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "HISTOrobustGaussianFit: 0 length cdf",
        );
        return Ok((ERROR_BADPARM, Some(offset), None));
    }
    while min_bin >= max_bin {
        let (m0, m1) = ((min_bin - 1).max(0), (max_bin + 1).min(hz.nbins - 1));
        if (m0, m1) == (min_bin, max_bin) {
            g.notices.add(
                &FS_24,
                "HISTOrobustGaussianFit: its window cannot grow (one bin): FreeSurfer loops for ever",
            );
            break;
        }
        min_bin = m0;
        max_bin = m1;
    }
    if max_bin - min_bin < MIN_BINS {
        max_bin += (MIN_BINS - (max_bin - min_bin)) / 2;
        min_bin -= (MIN_BINS - (max_bin - min_bin)) / 2;
        min_bin = min_bin.max(0);
        max_bin = max_bin.min(hz.nbins - 1);
    }
    let zbin = histo_find_bin(g, &hz, 0.0);
    let _ = zbin;
    let delta_sigma = f64::from(h.bin_size / 10.0f32);
    let max_sigma = f64::from(habs.nbins as f32 * habs.bin_size / 2.0f32);
    let mut best_sse = -1.0f64;
    let mut best_sigma = 0.0f64;
    let mut sigma = delta_sigma;
    while sigma <= max_sigma {
        let mut scale = 0.0f64;
        let mut n = 0i32;
        for b in min_bin..=max_bin {
            let val = f64::from(bin_value(g, &hz, b, "HISTOrobustGaussianFit"));
            let predicted = 1.0 / (sigma * sqrt_2pi) * libm.exp(-val * val / (2.0 * sigma * sigma));
            if predicted > 1e-3 {
                scale += f64::from(cnt(g, &hz, b, "HISTOrobustGaussianFit")) / predicted;
                n += 1;
            }
        }
        scale = sse::div64(scale, f64::from(n));
        let mut sse = 0.0f64;
        for b in min_bin..=max_bin {
            let val = f64::from(bin_value(g, &hz, b, "HISTOrobustGaussianFit"));
            let predicted =
                scale / (sigma * sqrt_2pi) * libm.exp(-val * val / (2.0 * sigma * sigma));
            let error = predicted - f64::from(cnt(g, &hz, b, "HISTOrobustGaussianFit"));
            sse += error * error;
        }
        if sse < best_sse || best_sse < 0.0 {
            best_sse = sse;
            best_sigma = sigma;
        }
        sigma += delta_sigma;
    }
    if diag {
        let mut hp = hz.clone();
        let sigma = best_sigma;
        let mut scale = 0.0f64;
        let mut n = 0i32;
        for b in min_bin..=max_bin {
            let val = f64::from(bin_value(g, &hz, b, "HISTOrobustGaussianFit")) - mean;
            let predicted = 1.0 / (sigma * sqrt_2pi) * libm.exp(-val * val / (2.0 * sigma * sigma));
            if predicted > 1e-3 {
                scale += f64::from(cnt(g, &hz, b, "HISTOrobustGaussianFit")) / predicted;
                n += 1;
            }
        }
        scale = sse::div64(scale, f64::from(n));
        for b in 0..hp.n() {
            let val = f64::from(hp.bins[b]) - mean;
            let predicted = scale / (best_sigma * sqrt_2pi)
                * libm.exp(-val * val / (2.0 * best_sigma * best_sigma));
            hp.counts[b] = predicted as f32;
        }
        histo_plot(&hp, dir, "hp.plt");
    }
    Ok((0, Some(offset), Some(best_sigma)))
}

/// `HISTOabs(h, habs)`: the histogram of the absolute values of `h`'s bins: bins from the
/// smallest absolute value to the largest in `h`'s bin size (`nint(ceil((mx - mn) / bin_size)
/// + 1)` of them, reallocated), each of `h`'s counts added at its absolute value's bin
/// ([`histo_find_bin`]; FS-22 outside).
pub fn histo_abs(
    g: &mut Globals,
    err: &mut dyn Write,
    h: &Histogram,
    habs: Option<&mut Histogram>,
) -> Result<Ret<Histogram>, Fatal> {
    let (mut mx, mut mn) = (0.0f64, 1e10f64);
    for &b in &h.bins[..h.n()] {
        let a = f64::from(b.abs());
        if a > mx {
            mx = a;
        }
        if a < mn {
            mn = a;
        }
    }
    if mn >= mx {
        mn = mx * 0.9;
    }
    let nb = nint(g, ((mx - mn) / f64::from(h.bin_size)).ceil() + 1.0);
    let mut fresh = None;
    let a: &mut Histogram = match habs {
        None => fresh.insert(histo_alloc(g, err, nb)?),
        Some(a) => {
            histo_realloc(g, err, Some(&mut *a), nb)?;
            a
        }
    };
    a.bin_size = h.bin_size;
    a.min = mn as f32;
    a.max = mx as f32;
    for b in 0..a.n() {
        a.bins[b] = (f64::from(b as f32 * a.bin_size) + mn) as f32;
    }
    for b in 0..h.n() {
        let pbin = histo_find_bin(g, a, h.bins[b].abs());
        let v = cnt(g, a, pbin, "HISTOabs") + h.counts[b];
        set_cnt(g, a, pbin, v, "HISTOabs");
    }
    Ok(match fresh {
        Some(a) => Ret::New(a),
        None => Ret::Given,
    })
}

/// `HISTOgetEntropy(h)`: `-sum p log p` over the bins with a count above 0, `p` the count over
/// [`histo_total`] (in `double`).
pub fn histo_get_entropy(g: &mut Globals, h: &Histogram) -> f64 {
    let libm = Libm::quiet(g.mode());
    let total = f64::from(histo_total(g, h));
    let mut entropy = 0.0f64;
    for &c in &h.counts[..h.n()] {
        if c > 0.0 {
            let p = f64::from(c) / total;
            entropy -= p * libm.log(p);
        }
    }
    entropy
}

/// `HISTOsoapBubbleZeros(hsrc, hdst, niters)`: a copy of `hsrc` whose empty bins are replaced
/// `niters` times by the mean of themselves and their neighbours (the filled bins kept). FS-22
/// for a histogram of one bin.
pub fn histo_soap_bubble_zeros(
    g: &mut Globals,
    hsrc: &Histogram,
    hdst: Option<&mut Histogram>,
    niters: i32,
) -> Ret<Histogram> {
    let mut fresh = None;
    let d: &mut Histogram = match hdst {
        None => fresh.insert(histo_copy(g, hsrc, None).unwrap_new()),
        Some(d) => {
            copy_into(g, hsrc, d);
            d
        }
    };
    let n = hsrc.n();
    let control: Vec<bool> = hsrc.counts[..n].iter().map(|&c| c > 0.0).collect();
    let mut tmp = vec![0.0f64; n];
    let last = d.nbins - 1;
    for _ in 0..niters {
        for (b, t) in tmp.iter_mut().enumerate() {
            *t = if control[b] {
                f64::from(hsrc.counts[b])
            } else {
                let bi = b as i32;
                let c = |g: &mut Globals, k: i32| cnt(g, d, k, "HISTOsoapBubbleZeros");
                f64::from(if bi == 0 {
                    (c(g, bi) + c(g, bi + 1)) / 2.0f32
                } else if bi == last {
                    (c(g, bi) + c(g, bi - 1)) / 2.0f32
                } else {
                    (c(g, bi + 1) + c(g, bi) + c(g, bi - 1)) / 3.0f32
                })
            };
        }
        for (c, &t) in d.counts[..n].iter_mut().zip(&tmp) {
            *c = t as f32;
        }
    }
    match fresh {
        Some(d) => Ret::New(d),
        None => Ret::Given,
    }
}

/// `HISTOsoapBubbleZeros(h, h, niters)` (the same result: FreeSurfer reads the filled bins'
/// counts back unchanged).
pub fn histo_soap_bubble_zeros_in_place(g: &mut Globals, h: &mut Histogram, niters: i32) {
    let src = h.clone();
    histo_soap_bubble_zeros(g, &src, Some(h), niters);
}

/// `HISTO2DfindBin1(h, val)`: as [`histo_find_bin`] on the first axis (with a bin size of 1,
/// `(int)val - min1`).
pub fn histo2d_find_bin1(g: &mut Globals, h: &Histogram2d, val: f32) -> i32 {
    find_bin_2d(g, &h.bins1, h.nbins1, h.bin_size1, h.min1, val)
}

/// `HISTO2DfindBin2(h, val)`: on the second axis.
pub fn histo2d_find_bin2(g: &mut Globals, h: &Histogram2d, val: f32) -> i32 {
    find_bin_2d(g, &h.bins2, h.nbins2, h.bin_size2, h.min2, val)
}

fn find_bin_2d(
    g: &mut Globals,
    bins: &[f32],
    nbins: i32,
    bin_size: f32,
    min: f32,
    val: f32,
) -> i32 {
    if bin_size == 1.0 {
        let iv = float_to_int(g, f64::from(val), "HISTO2DfindBin");
        return float_to_int(g, f64::from(iv as f32 - min), "HISTO2DfindBin");
    }
    let mut b = nbins - 1;
    while b > 0 {
        if bins[(b - 1) as usize] < val {
            return b;
        }
        b -= 1;
    }
    0
}

/// `HISTO2DgetCount(h, v1, v2)`: the count at both values' bins, 0 outside.
pub fn histo2d_get_count(g: &mut Globals, h: &Histogram2d, v1: f32, v2: f32) -> f64 {
    let b1 = histo2d_find_bin1(g, h, v1);
    if b1 < 0 || b1 >= h.nbins1 {
        return 0.0;
    }
    let b2 = histo2d_find_bin2(g, h, v2);
    if b2 < 0 || b2 >= h.nbins2 {
        return 0.0;
    }
    f64::from(h.counts[b1 as usize][b2 as usize])
}

/// `HISTOcomputeFWHM(h, peak)`: twice the first half-width at which both sides fall below half
/// the peak's count (at most `min(peak, nbins - peak - 1) - 1`; 1 when that is not above 1),
/// times the bin size.
pub fn histo_compute_fwhm(g: &mut Globals, h: &Histogram, peak: i32) -> f32 {
    let thresh = cnt(g, h, peak, "HISTOcomputeFWHM") / 2.0f32;
    let max_width = peak.min(h.nbins - (peak + 1));
    let mut width = 1;
    while width < max_width {
        if cnt(g, h, peak - width, "HISTOcomputeFWHM") < thresh
            && cnt(g, h, peak + width, "HISTOcomputeFWHM") < thresh
        {
            break;
        }
        width += 1;
    }
    (2 * width) as f32 * h.bin_size
}

/// `HISTOwriteTxt(histo, fname)`: `"%4d %10.5f %10.5f\n"` of each bin's number, value and
/// count; 1 after `ERROR: HISTOwrite(): opening <fname>` on standard output when the file
/// cannot be opened, else 0.
pub fn histo_write_txt(
    histo: &Histogram,
    out: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
) -> i32 {
    let Ok(mut fp) = std::fs::File::create(resolve(dir, fname)) else {
        let _ = writeln!(out, "ERROR: HISTOwrite(): opening {fname}");
        return 1;
    };
    let mut text = String::new();
    for n in 0..histo.n() {
        text.push_str(&format!(
            "{n:>4} {} {}\n",
            fmt_f(f64::from(histo.bins[n]), 10, 5),
            fmt_f(f64::from(histo.counts[n]), 10, 5)
        ));
    }
    let _ = fp.write_all(text.as_bytes());
    0
}

/// `HISTOsumNorm(h)`: the counts divided by their `double` sum when it is not 0. 0.
pub fn histo_sum_norm(h: &mut Histogram) -> i32 {
    let n = h.n();
    let mut sum = 0.0f64;
    for &c in &h.counts[..n] {
        sum += f64::from(c);
    }
    if sum != 0.0 {
        for c in &mut h.counts[..n] {
            *c = sse::div64(f64::from(*c), sum) as f32;
        }
    }
    0
}

/// `HISTOcumsum(h, hout)`: the running `double` sum of `h`'s counts into `hout` (a copy of `h`
/// when there is none; a given one keeps its bins). In place: [`histo_cumsum_in_place`]. FS-21.
pub fn histo_cumsum(
    g: &mut Globals,
    h: &Histogram,
    hout: Option<&mut Histogram>,
) -> Ret<Histogram> {
    let mut fresh = None;
    let d: &mut Histogram = match hout {
        None => fresh.insert(histo_copy(g, h, None).unwrap_new()),
        Some(d) => {
            d.grow(g, h.n(), "HISTOcumsum");
            d
        }
    };
    let mut sum = 0.0f64;
    for n in 0..h.n() {
        sum += f64::from(h.counts[n]);
        d.counts[n] = sum as f32;
    }
    match fresh {
        Some(d) => Ret::New(d),
        None => Ret::Given,
    }
}

/// `HISTOcumsum(h, h)`.
pub fn histo_cumsum_in_place(h: &mut Histogram) {
    let mut sum = 0.0f64;
    for n in 0..h.n() {
        sum += f64::from(h.counts[n]);
        h.counts[n] = sum as f32;
    }
}

#[cfg(test)]
mod tests;
