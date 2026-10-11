// SPDX-License-Identifier: Apache-2.0
//! Adaptive non-local means denoising: `AdaptiveNonLocalMeansDenoisingImageFilter` of ITK's
//! remote module AdaptiveDenoising (commit `24825c8d`, the one ITK v5.4.5 pins; Manjón et
//! al., *Adaptive non-local means denoising of MR images with spatially varying noise
//! levels*, JMRI 31:192-203, 2010), as ANTs' `DenoiseImage` runs it on `float` images.
//!
//! For every voxel `c` (the "centre"), in raster order (the first axis fastest):
//! 1. The centre takes part when its value, its local mean and its local variance (a box of
//!    radius 1, [`crate::neighborhood`]) are above 0, `ε`, `ε`, and the mask (if any) is not 0
//!    there. A neighbour `b` in the search box (radius 2 by default, the centre excluded)
//!    takes part when its value is above 0, its mean and variance above `ε`, the ratio of the
//!    means (or of their distances to the image maximum) is within `(0.95, 1/0.95)` and the
//!    ratio of the variances within `(0.5, 2)`.
//! 2. The noise estimate `h` is the smallest, over those neighbours, of the mean of
//!    `(v − mean)²` over the neighbour's patch (radius 1 by default); 0 becomes 1.
//! 3. Each such neighbour gets the weight `expf(−d / h)` if `d ≤ 3h`, else 0, where `d` is the
//!    mean of `((v − mean)_b+o − (v − mean)_c+o)²` over the patch offsets `o` inside the image
//!    at both ends. The patch around the centre gets the largest weight (1 if there was none).
//! 4. For every patch offset, the weighted mean of the patch values (their **squares** with
//!    the Rician model) is **added** to the output voxel `c + o`, and that voxel's count goes
//!    up by one. With the Rician model, the centre also writes `h` (0 if no neighbour took
//!    part) into a bias image at every `c + o`, overwriting what earlier centres wrote.
//!
//! Afterwards each output voxel is divided by its count. With the Rician model, the bias image
//! is smoothed (`DiscreteGaussianImageFilter`, variance 2 in physical units); where the
//! smoothed value `s` is above 0 (and the mask is not 0) the bias becomes
//! `2s / CF(mean / √s)`, with `CF` the Rician correction factor built from the modified Bessel
//! functions `I₀` and `I₁` of `GaussianOperator`; elsewhere the bias keeps its unsmoothed
//! value. The output is `√max(0, value − bias)`.
//!
//! Every quantity is a `float` computed in ITK's order, with `std::exp(float)` (`expf`) and
//! `exp` (in the Bessel functions) correctly rounded here (`larmorx_core::math`).
//!
//! **Threads.** ITK splits the centres among threads by the slowest axis, and the threads add
//! to output voxels near the borders of their regions concurrently, without locks: ANTs'
//! result depends on its thread count (and can change between runs). larmorx computes ITK's
//! single-thread result on any number of threads: the per-centre work runs in parallel, and
//! each output voxel then sums its contributions in the order the single thread adds them
//! (centres in raster order), see `docs/findings/ants-denoise.md`.

use larmorx_core::math;
use rayon::prelude::*;

use crate::FilterError;
use crate::discrete_gaussian::{DiscreteGaussianOptions, bessel_i0, bessel_i1, discrete_gaussian};
use crate::itk_math::almost_equal_f32;
use crate::neighborhood::{BoxNeighborhood, mean_filter, variance_filter};
use crate::volume::{VolumeRef, strides};

/// The parameters of [`adaptive_non_local_means`].
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveNlmParameters {
    /// The Rician noise model (ITK's default); otherwise Gaussian.
    pub rician: bool,
    /// The patch radius along each axis.
    pub patch_radius: Vec<usize>,
    /// The search radius along each axis.
    pub search_radius: Vec<usize>,
    /// The radius of the local mean and variance (`NeighborhoodRadiusForLocalMeanAndVariance`).
    pub local_radius: Vec<usize>,
    /// The smallest local mean and variance that count (`Epsilon`).
    pub epsilon: f32,
    /// `MeanThreshold`.
    pub mean_threshold: f32,
    /// `VarianceThreshold`.
    pub variance_threshold: f32,
    /// The variance of the bias image's smoothing, physical units² (`SmoothingVariance`).
    pub smoothing_variance: f32,
}

impl AdaptiveNlmParameters {
    /// What `DenoiseImage` sets for a `d`-dimensional image (`DenoiseImage.cxx:197-280`):
    /// patch radius 1, search radius 2, local radius 1, ε = 1e-5, mean threshold 0.95,
    /// variance threshold 0.5, smoothing variance 2.
    pub fn denoise_image(d: usize, rician: bool) -> Self {
        AdaptiveNlmParameters {
            rician,
            patch_radius: vec![1; d],
            search_radius: vec![2; d],
            local_radius: vec![1; d],
            epsilon: 0.00001,
            mean_threshold: 0.95,
            variance_threshold: 0.5,
            smoothing_variance: 2.0,
        }
    }
}

/// A mask read as ITK's `Image<unsigned char, D>`: a voxel counts where the mask is not 0. It
/// is indexed by the image's voxel indices, so it may be larger than the image (never
/// smaller).
#[derive(Clone, Copy, Debug)]
pub struct MaskRef<'a> {
    pub data: &'a [u8],
    pub size: &'a [usize],
}

/// What [`adaptive_non_local_means`] returns besides the image (for inspection and tests).
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveNlmOutput {
    /// The denoised image.
    pub denoised: Vec<f32>,
    /// The local mean and variance.
    pub mean: Vec<f32>,
    pub variance: Vec<f32>,
    /// The bias image after the correction (Rician model; empty otherwise).
    pub rician_bias: Vec<f32>,
}

/// The largest dimension handled.
const MAXD: usize = 4;
/// Flags of a centre.
const CONTRIBUTES: u8 = 1;
const WRITES_BIAS: u8 = 2;

/// Everything the centres read.
struct Context<'a> {
    d: usize,
    size: [usize; MAXD],
    strides: [usize; MAXD],
    input: &'a [f32],
    mean: &'a [f32],
    variance: &'a [f32],
    /// `input − mean`.
    diff: Vec<f32>,
    /// The values averaged: `input²` (Rician) or `input`.
    values: Vec<f32>,
    /// Per voxel, the mean of `(input − mean)²` over its patch (step 2's distance).
    patch_distance: Vec<f32>,
    mask: Option<MaskRef<'a>>,
    mask_strides: [usize; MAXD],
    maximum: f32,
    p: &'a AdaptiveNlmParameters,
    search: Offsets,
    patch: Offsets,
    /// The search offset of the centre itself (`static_cast<unsigned>(0.5 · size)`).
    search_centre: usize,
}

/// Neighbourhood offsets padded to [`MAXD`] axes.
struct Offsets {
    steps: Vec<[isize; MAXD]>,
    linear: Vec<isize>,
    radius: [usize; MAXD],
}

impl Offsets {
    fn new(radius: &[usize], size: &[usize]) -> Self {
        let nb = BoxNeighborhood::new(radius, size);
        let steps = nb
            .offsets
            .iter()
            .map(|o| std::array::from_fn(|k| o.get(k).copied().unwrap_or(0)))
            .collect();
        Offsets {
            steps,
            linear: nb.linear,
            radius: std::array::from_fn(|k| radius.get(k).copied().unwrap_or(0)),
        }
    }

    fn len(&self) -> usize {
        self.linear.len()
    }
}

/// Per-thread scratch space of a centre.
struct Scratch {
    weighted: Vec<f32>,
    neighbours: Vec<usize>,
}

impl Context<'_> {
    fn inside(&self, c: &[isize; MAXD]) -> bool {
        (0..self.d).all(|k| c[k] >= 0 && (c[k] as usize) < self.size[k])
    }

    fn linear(&self, c: &[isize; MAXD]) -> usize {
        (0..self.d).map(|k| c[k] as usize * self.strides[k]).sum()
    }

    fn mask_allows(&self, c: &[usize; MAXD]) -> bool {
        match self.mask {
            None => true,
            Some(m) => {
                let i: usize = (0..self.d).map(|k| c[k] * self.mask_strides[k]).sum();
                m.data[i] != 0
            }
        }
    }

    /// Step 1's test of a neighbour against the centre's mean `mc` and variance `vc`.
    #[inline(always)]
    fn similar(&self, b: usize, mc: f32, vc: f32) -> bool {
        let p = self.p;
        if self.input[b] <= 0.0 {
            return false;
        }
        let mb = self.mean[b];
        let vb = self.variance[b];
        if mb <= p.epsilon || vb <= p.epsilon {
            return false;
        }
        let mean_ratio = mc / mb;
        let mean_ratio_inverse = (self.maximum - mc) / (self.maximum - mb);
        let variance_ratio = vc / vb;
        let mt = p.mean_threshold;
        let vt = p.variance_threshold;
        ((mean_ratio > mt && mean_ratio < 1.0 / mt)
            || (mean_ratio_inverse > mt && mean_ratio_inverse < 1.0 / mt))
            && variance_ratio > vt
            && variance_ratio < 1.0 / vt
    }

    /// Is the whole search box, with every patch around it, inside the image?
    fn interior(&self, c: &[usize; MAXD]) -> bool {
        (0..self.d).all(|k| {
            let r = self.search.radius[k] + self.patch.radius[k];
            c[k] >= r && c[k] + r < self.size[k]
        })
    }

    /// One centre: its contribution `weighted[o] / Σ weights` to each patch voxel (written
    /// to `contribution`), its flags and the bias it writes.
    fn centre(
        &self,
        c: &[usize; MAXD],
        lin: usize,
        contribution: &mut [f32],
        scratch: &mut Scratch,
    ) -> (u8, f32) {
        let p = self.p;
        let np = self.patch.len();
        let interior = self.interior(c);
        let ci: [isize; MAXD] = std::array::from_fn(|k| c[k] as isize);
        let weighted = &mut scratch.weighted;
        weighted.clear();
        weighted.resize(np, 0.0);
        let mc = self.mean[lin];
        let vc = self.variance[lin];
        let mut maximum_weight = 0.0f32;
        let mut sum_of_weights = 0.0f32;
        let mut flags = 0u8;
        let mut bias = 0.0f32;
        let takes_part =
            self.input[lin] > 0.0 && mc > p.epsilon && vc > p.epsilon && self.mask_allows(c);
        if takes_part {
            // The neighbours that take part, and the smallest patch distance among them.
            let neighbours = &mut scratch.neighbours;
            neighbours.clear();
            let mut minimum_distance = f32::MAX;
            for m in 0..self.search.len() {
                if m == self.search_centre {
                    continue;
                }
                let b = if interior {
                    (lin as isize + self.search.linear[m]) as usize
                } else {
                    let bc: [isize; MAXD] =
                        std::array::from_fn(|k| ci[k] + self.search.steps[m][k]);
                    if !self.inside(&bc) {
                        continue;
                    }
                    self.linear(&bc)
                };
                if !self.similar(b, mc, vc) {
                    continue;
                }
                neighbours.push(m);
                // std::min(averageDistance, minimumDistance)
                let a = self.patch_distance[b];
                minimum_distance = if minimum_distance < a {
                    minimum_distance
                } else {
                    a
                };
            }
            if almost_equal_f32(minimum_distance, 0.0) {
                minimum_distance = 1.0;
            }
            if p.rician {
                flags |= WRITES_BIAS;
                bias = if almost_equal_f32(minimum_distance, f32::MAX) {
                    0.0
                } else {
                    minimum_distance
                };
            }
            let three_h = 3.0f32 * minimum_distance;
            for &m in neighbours.iter() {
                let (b, average_distance) = if interior {
                    let b = (lin as isize + self.search.linear[m]) as usize;
                    let mut sum = 0.0f32;
                    let mut count = 0.0f32;
                    for &o in &self.patch.linear {
                        let t = self.diff[(b as isize + o) as usize]
                            - self.diff[(lin as isize + o) as usize];
                        sum += t * t;
                        count += 1.0;
                    }
                    (b, sum / count)
                } else {
                    let bc: [isize; MAXD] =
                        std::array::from_fn(|k| ci[k] + self.search.steps[m][k]);
                    let b = self.linear(&bc);
                    let mut sum = 0.0f32;
                    let mut count = 0.0f32;
                    for (n, s) in self.patch.steps.iter().enumerate() {
                        let x: [isize; MAXD] = std::array::from_fn(|k| bc[k] + s[k]);
                        let y: [isize; MAXD] = std::array::from_fn(|k| ci[k] + s[k]);
                        if !self.inside(&x) || !self.inside(&y) {
                            continue;
                        }
                        let o = self.patch.linear[n];
                        let t = self.diff[(b as isize + o) as usize]
                            - self.diff[(lin as isize + o) as usize];
                        sum += t * t;
                        count += 1.0;
                    }
                    (b, sum / count)
                };
                let weight = if average_distance <= three_h {
                    math::expf(-average_distance / minimum_distance)
                } else {
                    0.0
                };
                if weight > maximum_weight {
                    maximum_weight = weight;
                }
                if weight > 0.0 {
                    if interior {
                        for (w, &o) in weighted.iter_mut().zip(&self.patch.linear) {
                            *w += weight * self.values[(b as isize + o) as usize];
                        }
                    } else {
                        let bc: [isize; MAXD] =
                            std::array::from_fn(|k| ci[k] + self.search.steps[m][k]);
                        for (n, s) in self.patch.steps.iter().enumerate() {
                            let x: [isize; MAXD] = std::array::from_fn(|k| bc[k] + s[k]);
                            if self.inside(&x) {
                                weighted[n] += weight
                                    * self.values[(b as isize + self.patch.linear[n]) as usize];
                            }
                        }
                    }
                    sum_of_weights += weight;
                }
            }
            if almost_equal_f32(maximum_weight, 0.0) {
                maximum_weight = 1.0;
            }
        } else {
            maximum_weight = 1.0;
        }
        // The centre's own patch, with the largest weight.
        for (n, s) in self.patch.steps.iter().enumerate() {
            if !interior {
                let x: [isize; MAXD] = std::array::from_fn(|k| ci[k] + s[k]);
                if !self.inside(&x) {
                    continue;
                }
            }
            weighted[n] +=
                maximum_weight * self.values[(lin as isize + self.patch.linear[n]) as usize];
        }
        sum_of_weights += maximum_weight;
        if sum_of_weights > 0.0 {
            flags |= CONTRIBUTES;
            for (out, &w) in contribution.iter_mut().zip(weighted.iter()) {
                *out = w / sum_of_weights;
            }
        }
        (flags, bias)
    }
}

/// The mean of `(input − mean)²` over each voxel's patch, the patch clipped to the image.
fn patch_distances(ctx: &Context<'_>, n_threads: usize) -> Result<Vec<f32>, FilterError> {
    let len = ctx.input.len();
    let nx = ctx.size[0];
    let mut out = vec![0.0f32; len];
    larmorx_core::parallel::with_threads(n_threads, || {
        out.par_chunks_mut(nx).enumerate().for_each(|(row, line)| {
            let mut c = row_coordinates(ctx, row);
            for (x, o) in line.iter_mut().enumerate() {
                c[0] = x as isize;
                let lin = row * nx + x;
                let mut sum = 0.0f32;
                let mut count = 0.0f32;
                for (n, s) in ctx.patch.steps.iter().enumerate() {
                    let y: [isize; MAXD] = std::array::from_fn(|k| c[k] + s[k]);
                    if !ctx.inside(&y) {
                        continue;
                    }
                    let t = ctx.diff[(lin as isize + ctx.patch.linear[n]) as usize];
                    sum += t * t;
                    count += 1.0;
                }
                *o = sum / count;
            }
        });
    })?;
    Ok(out)
}

/// The coordinates of the first voxel of row `row` (every axis but the first).
fn row_coordinates(ctx: &Context<'_>, row: usize) -> [isize; MAXD] {
    let mut c = [0isize; MAXD];
    let mut rest = row;
    for (ck, &n) in c.iter_mut().zip(&ctx.size).take(ctx.d).skip(1) {
        *ck = (rest % n) as isize;
        rest /= n;
    }
    c
}

/// The coordinates of voxel `lin`.
fn coordinates(ctx: &Context<'_>, lin: usize) -> [isize; MAXD] {
    let mut c = [0isize; MAXD];
    let mut rest = lin;
    for (ck, &n) in c.iter_mut().zip(&ctx.size).take(ctx.d) {
        *ck = (rest % n) as isize;
        rest /= n;
    }
    c
}

/// The centres of a window of the image: `[start, start + len)` in raster order.
struct Window {
    start: usize,
    contribution: Vec<f32>,
    flags: Vec<u8>,
    bias: Vec<f32>,
}

/// `AdaptiveNonLocalMeansDenoisingImageFilter` on `input` (ITK's single-thread result, with
/// any `n_threads`).
pub fn adaptive_non_local_means(
    input: VolumeRef<'_, f32>,
    mask: Option<MaskRef<'_>>,
    parameters: &AdaptiveNlmParameters,
    n_threads: usize,
) -> Result<AdaptiveNlmOutput, FilterError> {
    run(input, mask, parameters, n_threads, BLOCK_VOXELS)
}

/// About how many centres are computed between two summing passes.
const BLOCK_VOXELS: usize = 1 << 18;

fn run(
    input: VolumeRef<'_, f32>,
    mask: Option<MaskRef<'_>>,
    parameters: &AdaptiveNlmParameters,
    n_threads: usize,
    block_voxels: usize,
) -> Result<AdaptiveNlmOutput, FilterError> {
    let d = input.dim();
    if !(1..=MAXD).contains(&d) {
        return Err(FilterError::invalid(format!(
            "{d}-dimensional images are not supported"
        )));
    }
    for (what, r) in [
        ("patch", &parameters.patch_radius),
        ("search", &parameters.search_radius),
        ("local mean and variance", &parameters.local_radius),
    ] {
        if r.len() != d {
            return Err(FilterError::invalid(format!(
                "the {what} radius has {} values for a {d}-dimensional image",
                r.len()
            )));
        }
    }
    if let Some(m) = mask {
        if m.size.len() != d || (0..d).any(|k| m.size[k] < input.size[k]) {
            return Err(FilterError::invalid(
                "the mask's region does not contain the image's (ITK: requested region is \
                 (at least partially) outside the largest possible region)",
            ));
        }
        if m.data.len() != m.size.iter().product::<usize>() {
            return Err(FilterError::SizeMismatch {
                what: "the mask",
                actual: m.data.len(),
                expected: m.size.iter().product(),
            });
        }
    }
    let len = input.len();
    if len == 0 {
        return Ok(AdaptiveNlmOutput {
            denoised: Vec::new(),
            mean: Vec::new(),
            variance: Vec::new(),
            rician_bias: Vec::new(),
        });
    }
    let mean = mean_filter(input, &parameters.local_radius, n_threads)?;
    let variance = variance_filter(input, &parameters.local_radius, n_threads)?;
    // StatisticsImageFilter's maximum: std::max from -FLT_MAX (NaN never wins).
    let mut maximum = -f32::MAX;
    for &v in input.data {
        if maximum < v {
            maximum = v;
        }
    }
    let rician = parameters.rician;
    let (diff, values): (Vec<f32>, Vec<f32>) =
        larmorx_core::parallel::with_threads(n_threads, || {
            input
                .data
                .par_iter()
                .zip(mean.par_iter())
                .with_min_len(1 << 14)
                .map(|(&v, &m)| (v - m, if rician { v * v } else { v }))
                .unzip()
        })?;
    let st = strides(input.size);
    let search = Offsets::new(&parameters.search_radius, input.size);
    let patch = Offsets::new(&parameters.patch_radius, input.size);
    let search_centre = (0.5 * search.len() as f64) as usize;
    let mut ctx = Context {
        d,
        size: std::array::from_fn(|k| input.size.get(k).copied().unwrap_or(1)),
        strides: std::array::from_fn(|k| st.get(k).copied().unwrap_or(0)),
        input: input.data,
        mean: &mean,
        variance: &variance,
        diff,
        values,
        patch_distance: Vec::new(),
        mask,
        mask_strides: match mask {
            Some(m) => {
                let s = strides(m.size);
                std::array::from_fn(|k| s.get(k).copied().unwrap_or(0))
            }
            None => [0; MAXD],
        },
        maximum,
        p: parameters,
        search,
        patch,
        search_centre,
    };
    ctx.patch_distance = patch_distances(&ctx, n_threads)?;
    let ctx = ctx;

    // The centres, block by block in raster order; each output voxel is summed once every
    // centre whose patch reaches it is known.
    let np = ctx.patch.len();
    let nx = ctx.size[0];
    let rows = len / nx;
    let reach = ctx.patch.linear.iter().copied().max().unwrap_or(0).max(0) as usize;
    let block_rows = (block_voxels / nx).clamp(1, rows);
    let mut sums = vec![0.0f32; len];
    let mut counts = vec![0.0f32; len];
    let mut raw_bias = vec![0.0f32; len];
    let mut window = Window {
        start: 0,
        contribution: Vec::new(),
        flags: Vec::new(),
        bias: Vec::new(),
    };
    let mut next_output = 0usize;
    let mut row = 0usize;
    larmorx_core::parallel::with_threads(n_threads, || {
        while row < rows {
            let block_end_row = (row + block_rows).min(rows);
            let block_start = row * nx;
            let block_len = (block_end_row - row) * nx;
            let old = window.flags.len();
            window.contribution.resize((old + block_len) * np, 0.0);
            window.flags.resize(old + block_len, 0);
            window.bias.resize(old + block_len, 0.0);
            let contribution = &mut window.contribution[old * np..];
            let flags = &mut window.flags[old..];
            let bias = &mut window.bias[old..];
            contribution
                .par_chunks_mut(nx * np)
                .zip(flags.par_chunks_mut(nx))
                .zip(bias.par_chunks_mut(nx))
                .enumerate()
                .for_each_init(
                    || Scratch {
                        weighted: Vec::with_capacity(np),
                        neighbours: Vec::with_capacity(ctx.search.len()),
                    },
                    |scratch, (i, ((contribution, flags), bias))| {
                        let r = row + i;
                        let rc = row_coordinates(&ctx, r);
                        let mut c: [usize; MAXD] = std::array::from_fn(|k| rc[k] as usize);
                        for x in 0..nx {
                            c[0] = x;
                            let (f, b) = ctx.centre(
                                &c,
                                r * nx + x,
                                &mut contribution[x * np..(x + 1) * np],
                                scratch,
                            );
                            flags[x] = f;
                            bias[x] = b;
                        }
                    },
                );
            row = block_end_row;
            let known = block_start + block_len;
            let complete = if known == len {
                len
            } else {
                known.saturating_sub(reach)
            };
            if complete > next_output {
                gather(
                    &ctx,
                    &window,
                    next_output,
                    &mut sums[next_output..complete],
                    &mut counts[next_output..complete],
                    &mut raw_bias[next_output..complete],
                );
                next_output = complete;
            }
            // Keep the centres that outputs still to be summed need.
            let keep_from = next_output.saturating_sub(reach).max(window.start);
            let drop = keep_from - window.start;
            if drop > 0 {
                window.contribution.drain(..drop * np);
                window.flags.drain(..drop);
                window.bias.drain(..drop);
                window.start = keep_from;
            }
        }
    })?;

    // AfterThreadedGenerateData.
    let mut bias_image = Vec::new();
    if rician {
        let options = DiscreteGaussianOptions::new(d, f64::from(parameters.smoothing_variance));
        let smoothed = discrete_gaussian(
            VolumeRef::new(&raw_bias, input.size, input.spacing),
            &options,
            n_threads,
        )?;
        bias_image = raw_bias;
        larmorx_core::parallel::with_threads(n_threads, || {
            bias_image
                .par_iter_mut()
                .zip(smoothed.par_iter())
                .zip(mean.par_iter())
                .enumerate()
                .with_min_len(1 << 12)
                .for_each(|(lin, ((b, &s), &m))| {
                    // `ItS.Get() > 0`: false for NaN too.
                    if s.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
                        return;
                    }
                    if ctx.mask.is_some() {
                        let c = coordinates(&ctx, lin);
                        let c: [usize; MAXD] = std::array::from_fn(|k| c[k] as usize);
                        if !ctx.mask_allows(&c) {
                            return;
                        }
                    }
                    let snr = m / s.sqrt();
                    let mut v = 2.0f32 * s / correction_factor(snr);
                    if v.is_nan() || v.is_infinite() {
                        v = 0.0;
                    }
                    *b = v;
                });
        })?;
    }
    let denoised: Vec<f32> = larmorx_core::parallel::with_threads(n_threads, || {
        sums.par_iter()
            .zip(counts.par_iter())
            .enumerate()
            .with_min_len(1 << 14)
            .map(|(lin, (&s, &n))| {
                if almost_equal_f32(n, 0.0) {
                    return s;
                }
                let mut estimate = s / n;
                if rician {
                    estimate -= bias_image[lin];
                    if estimate < 0.0 {
                        estimate = 0.0;
                    }
                    estimate = estimate.sqrt();
                }
                estimate
            })
            .collect()
    })?;
    Ok(AdaptiveNlmOutput {
        denoised,
        mean,
        variance,
        rician_bias: bias_image,
    })
}

/// Sums the contributions to output voxels `[from, to)` in the single thread's order: the
/// centres in raster order, which for a fixed output voxel is the patch offsets in reverse.
/// The bias is the one written last (the first offset whose centre wrote one).
fn gather(
    ctx: &Context<'_>,
    window: &Window,
    from: usize,
    sums: &mut [f32],
    counts: &mut [f32],
    bias: &mut [f32],
) {
    const CHUNK: usize = 1 << 12;
    let np = ctx.patch.len();
    sums.par_chunks_mut(CHUNK)
        .zip(counts.par_chunks_mut(CHUNK))
        .zip(bias.par_chunks_mut(CHUNK))
        .enumerate()
        .for_each(|(chunk, ((sums, counts), bias))| {
            let first = from + chunk * CHUNK;
            let mut c = coordinates(ctx, first);
            for i in 0..sums.len() {
                let p = first + i;
                if i > 0 {
                    // Advance the coordinates by one voxel.
                    let mut k = 0;
                    c[0] += 1;
                    while k + 1 < ctx.d && c[k] as usize == ctx.size[k] {
                        c[k] = 0;
                        c[k + 1] += 1;
                        k += 1;
                    }
                }
                let interior = (0..ctx.d).all(|k| {
                    let r = ctx.patch.radius[k] as isize;
                    c[k] >= r && c[k] + r < ctx.size[k] as isize
                });
                let mut sum = 0.0f32;
                let mut count = 0.0f32;
                let mut last_bias: Option<f32> = None;
                for n in (0..np).rev() {
                    if !interior {
                        let centre: [isize; MAXD] =
                            std::array::from_fn(|k| c[k] - ctx.patch.steps[n][k]);
                        if !ctx.inside(&centre) {
                            continue;
                        }
                    }
                    let centre = (p as isize - ctx.patch.linear[n]) as usize;
                    let w = centre - window.start;
                    let f = window.flags[w];
                    if f & CONTRIBUTES != 0 {
                        sum += window.contribution[w * np + n];
                        count += 1.0;
                    }
                    if f & WRITES_BIAS != 0 {
                        // Later centres (smaller n) overwrite earlier ones.
                        last_bias = Some(window.bias[w]);
                    }
                }
                sums[i] = sum;
                counts[i] = count;
                bias[i] = last_bias.unwrap_or(0.0);
            }
        });
}

/// `CalculateCorrectionFactor`: the Rician bias correction factor for a signal-to-noise
/// ratio `snr`, in float, with `GaussianOperator`'s modified Bessel functions (double).
fn correction_factor(snr: f32) -> f32 {
    let snr_squared = snr * snr;
    let quarter = 0.25f32 * snr_squared;
    let i0 = bessel_i0(f64::from(quarter)) as f32;
    let i1 = bessel_i1(f64::from(quarter)) as f32;
    let e = math::expf(-0.5f32 * snr_squared);
    let t = (2.0f32 + snr_squared) * i0 + snr_squared * i1;
    let mut value = 2.0f32 + snr_squared - 0.125f32 * (std::f64::consts::PI as f32) * e * (t * t);
    if value < 0.001f32 || value > 10.0f32 {
        value = 1.0;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A literal transcription of ITK's single-thread loops (`ThreadedGenerateData` and
    /// `AfterThreadedGenerateData`) on 3D images, with coordinates and in-place updates.
    fn reference(
        data: &[f32],
        size: [usize; 3],
        spacing: [f64; 3],
        mask: Option<&[u8]>,
        p: &AdaptiveNlmParameters,
    ) -> Vec<f32> {
        let v = VolumeRef::new(data, &size, &spacing);
        let mean = mean_filter(v, &p.local_radius, 1).unwrap();
        let var = variance_filter(v, &p.local_radius, 1).unwrap();
        let maximum = data
            .iter()
            .fold(-f32::MAX, |m, &x| if m < x { x } else { m });
        let n = size[0] * size[1] * size[2];
        let idx = |c: [isize; 3]| -> Option<usize> {
            if (0..3).all(|k| c[k] >= 0 && (c[k] as usize) < size[k]) {
                Some(c[0] as usize + size[0] * (c[1] as usize + size[1] * c[2] as usize))
            } else {
                None
            }
        };
        let search = BoxNeighborhood::new(&p.search_radius, &size);
        let patch = BoxNeighborhood::new(&p.patch_radius, &size);
        let mut out = vec![0.0f32; n];
        let mut count = vec![0.0f32; n];
        let mut bias = vec![0.0f32; n];
        let mt = p.mean_threshold;
        let vt = p.variance_threshold;
        for z in 0..size[2] as isize {
            for y in 0..size[1] as isize {
                for x in 0..size[0] as isize {
                    let c = [x, y, z];
                    let ci = idx(c).unwrap();
                    let (xc, mc, vc) = (data[ci], mean[ci], var[ci]);
                    let mut max_w = 0.0f32;
                    let mut sum_w = 0.0f32;
                    let mut wai = vec![0.0f32; patch.len()];
                    let ok = |b: usize| -> bool {
                        if data[b] <= 0.0 {
                            return false;
                        }
                        let (mb, vb) = (mean[b], var[b]);
                        if mb <= p.epsilon || vb <= p.epsilon {
                            return false;
                        }
                        let r = mc / mb;
                        let ri = (maximum - mc) / (maximum - mb);
                        let rv = vc / vb;
                        ((r > mt && r < 1.0 / mt) || (ri > mt && ri < 1.0 / mt))
                            && rv > vt
                            && rv < 1.0 / vt
                    };
                    if xc > 0.0
                        && mc > p.epsilon
                        && vc > p.epsilon
                        && mask.is_none_or(|m| m[ci] != 0)
                    {
                        let mut min_d = f32::MAX;
                        for m in 0..search.len() {
                            let o = &search.offsets[m];
                            let bc = [c[0] + o[0], c[1] + o[1], c[2] + o[2]];
                            let Some(b) = idx(bc) else { continue };
                            if m == search.len() / 2 || !ok(b) {
                                continue;
                            }
                            let mut a = 0.0f32;
                            let mut k = 0.0f32;
                            for q in &patch.offsets {
                                if let Some(j) = idx([bc[0] + q[0], bc[1] + q[1], bc[2] + q[2]]) {
                                    let t = data[j] - mean[j];
                                    a += t * t;
                                    k += 1.0;
                                }
                            }
                            a /= k;
                            min_d = if min_d < a { min_d } else { a };
                        }
                        if almost_equal_f32(min_d, 0.0) {
                            min_d = 1.0;
                        }
                        if p.rician {
                            for q in &patch.offsets {
                                if let Some(j) = idx([c[0] + q[0], c[1] + q[1], c[2] + q[2]]) {
                                    bias[j] = if almost_equal_f32(min_d, f32::MAX) {
                                        0.0
                                    } else {
                                        min_d
                                    };
                                }
                            }
                        }
                        for m in 0..search.len() {
                            let o = &search.offsets[m];
                            let bc = [c[0] + o[0], c[1] + o[1], c[2] + o[2]];
                            let Some(b) = idx(bc) else { continue };
                            if m == search.len() / 2 || !ok(b) {
                                continue;
                            }
                            let mut a = 0.0f32;
                            let mut k = 0.0f32;
                            for q in &patch.offsets {
                                let s = idx([bc[0] + q[0], bc[1] + q[1], bc[2] + q[2]]);
                                let t = idx([c[0] + q[0], c[1] + q[1], c[2] + q[2]]);
                                if let (Some(s), Some(t)) = (s, t) {
                                    let d = (data[s] - mean[s]) - (data[t] - mean[t]);
                                    a += d * d;
                                    k += 1.0;
                                }
                            }
                            a /= k;
                            let w = if a <= 3.0 * min_d {
                                math::expf(-a / min_d)
                            } else {
                                0.0
                            };
                            if w > max_w {
                                max_w = w;
                            }
                            if w > 0.0 {
                                for (n, q) in patch.offsets.iter().enumerate() {
                                    if let Some(j) = idx([bc[0] + q[0], bc[1] + q[1], bc[2] + q[2]])
                                    {
                                        wai[n] +=
                                            w * if p.rician { data[j] * data[j] } else { data[j] };
                                    }
                                }
                                sum_w += w;
                            }
                        }
                        if almost_equal_f32(max_w, 0.0) {
                            max_w = 1.0;
                        }
                    } else {
                        max_w = 1.0;
                    }
                    for (n, q) in patch.offsets.iter().enumerate() {
                        if let Some(j) = idx([c[0] + q[0], c[1] + q[1], c[2] + q[2]]) {
                            wai[n] += max_w * if p.rician { data[j] * data[j] } else { data[j] };
                        }
                    }
                    sum_w += max_w;
                    if sum_w > 0.0 {
                        for (n, q) in patch.offsets.iter().enumerate() {
                            if let Some(j) = idx([c[0] + q[0], c[1] + q[1], c[2] + q[2]]) {
                                out[j] += wai[n] / sum_w;
                                count[j] += 1.0;
                            }
                        }
                    }
                }
            }
        }
        if p.rician {
            let options = DiscreteGaussianOptions::new(3, f64::from(p.smoothing_variance));
            let s = discrete_gaussian(VolumeRef::new(&bias, &size, &spacing), &options, 1).unwrap();
            for i in 0..n {
                if s[i] > 0.0 && mask.is_none_or(|m| m[i] != 0) {
                    let snr = mean[i] / s[i].sqrt();
                    let mut b = 2.0 * s[i] / correction_factor(snr);
                    if b.is_nan() || b.is_infinite() {
                        b = 0.0;
                    }
                    bias[i] = b;
                }
            }
        }
        for i in 0..n {
            if almost_equal_f32(count[i], 0.0) {
                continue;
            }
            let mut e = out[i] / count[i];
            if p.rician {
                e -= bias[i];
                if e < 0.0 {
                    e = 0.0;
                }
                e = e.sqrt();
            }
            out[i] = e;
        }
        out
    }

    /// A small head-like image: noise on a background of 0, a bright blob, a gradient.
    fn phantom(size: [usize; 3], seed: u64) -> Vec<f32> {
        let mut s = seed;
        let mut next = move || {
            s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
        };
        let mut out = Vec::new();
        for z in 0..size[2] {
            for y in 0..size[1] {
                for x in 0..size[0] {
                    let dx = x as f32 - size[0] as f32 / 2.0;
                    let dy = y as f32 - size[1] as f32 / 2.0;
                    let dz = z as f32 - size[2] as f32 / 2.0;
                    let r2 = dx * dx + dy * dy + 2.0 * dz * dz;
                    let base = if r2 < 30.0 {
                        300.0 + 4.0 * x as f32
                    } else {
                        20.0
                    };
                    let noise = 40.0 * (next() - 0.3);
                    let v = base + noise;
                    out.push(if x == 0 && y % 3 == 0 {
                        0.0
                    } else {
                        v.max(0.0)
                    });
                }
            }
        }
        out
    }

    #[test]
    fn matches_the_single_thread_loops() {
        let size = [13usize, 11, 9];
        let spacing = [1.0, 1.2, 1.5];
        let data = phantom(size, 7);
        let mask: Vec<u8> = (0..data.len()).map(|i| u8::from(i % 7 != 3)).collect();
        for rician in [true, false] {
            for (patch, search) in [(1usize, 2usize), (1, 1), (2, 1), (0, 3)] {
                let mut p = AdaptiveNlmParameters::denoise_image(3, rician);
                p.patch_radius = vec![patch; 3];
                p.search_radius = vec![search; 3];
                for m in [None, Some(&mask[..])] {
                    let expected = reference(&data, size, spacing, m, &p);
                    let v = VolumeRef::new(&data, &size, &spacing);
                    let mref = m.map(|d| MaskRef {
                        data: d,
                        size: &size,
                    });
                    for threads in [1, 3] {
                        let got = adaptive_non_local_means(v, mref, &p, threads).unwrap();
                        assert!(
                            got.denoised
                                .iter()
                                .zip(&expected)
                                .all(|(a, b)| a.to_bits() == b.to_bits()),
                            "rician {rician}, patch {patch}, search {search}, mask {}, \
                             {threads} threads",
                            m.is_some()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn small_blocks_give_the_same_result() {
        // Blocks of one to a few rows: the window of centres kept between blocks.
        let size = [9usize, 7, 6];
        let data = phantom(size, 3);
        for (rician, patch) in [(true, 1usize), (false, 2)] {
            let mut p = AdaptiveNlmParameters::denoise_image(3, rician);
            p.patch_radius = vec![patch; 3];
            let expected = reference(&data, size, [1.0; 3], None, &p);
            for block in [1usize, 9, 20, 64, 1000] {
                let got = run(VolumeRef::new(&data, &size, &[1.0; 3]), None, &p, 2, block).unwrap();
                assert_eq!(got.denoised, expected, "block {block}");
            }
        }
    }

    #[test]
    fn correction_factor_is_bounded() {
        // ξ(0) = 2 − π/2.
        assert!((correction_factor(0.0) - (2.0 - std::f32::consts::FRAC_PI_2)).abs() < 1e-6);
        for snr in [0.5f32, 1.0, 2.0, 5.0, 10.0] {
            let cf = correction_factor(snr);
            assert!((0.001..=10.0).contains(&cf), "{snr}: {cf}");
        }
        // At a high SNR, float(I₀) overflows and expf underflows: 0 · ∞ is NaN, as in ITK
        // (the caller then sets the bias to 0).
        assert!(correction_factor(20.0).is_nan());
    }
}
