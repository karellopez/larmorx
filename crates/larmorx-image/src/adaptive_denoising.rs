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
//! smoothed value `s` is above 0 (and, in ITK, the mask is not 0) the bias becomes
//! `2s / CF(mean / √s)`, with `CF` the Rician correction factor built from the modified Bessel
//! functions `I₀` and `I₁` of `GaussianOperator`; elsewhere the bias keeps its unsmoothed
//! value. The output is `√max(0, value − bias)`.
//!
//! **ANTS-44.** Patches about centres in the mask reach voxels outside it, which get an
//! estimate like any other; ITK leaves their bias the raw noise estimate `h` of the last such
//! centre (neither smoothed nor corrected, `UpstreamDefects::Reproduce`). Corrected (the
//! default): every voxel the patch of a centre in the mask reaches gets the smoothed and
//! corrected bias (the voxels outside the mask no such patch reaches are not denoised and keep
//! 0, as in ITK); named where
//! the output changes.
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

use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects};
use larmorx_core::math;
use rayon::prelude::*;

use crate::FilterError;
use crate::discrete_gaussian::{
    DiscreteGaussianOptions, bessel_i0, bessel_i1, discrete_gaussian, scaled_bessel_i0,
    scaled_bessel_i1,
};
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
    /// ANTS-43: at a high signal-to-noise ratio ITK's Rician correction factor overflows
    /// (NaN) and the bias becomes 0 ([`UpstreamDefects::Reproduce`]); the default computes the
    /// factor without the overflow ([`corrected_correction_factor`]).
    pub upstream_defects: UpstreamDefects,
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
            upstream_defects: UpstreamDefects::Fix,
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

/// What [`adaptive_non_local_means`] returns.
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveNlmOutput {
    /// The denoised image.
    pub denoised: Vec<f32>,
    /// The bias image after the correction (Rician model; empty otherwise).
    pub rician_bias: Vec<f32>,
    /// The upstream defects that changed this result (or, reproduced, would have).
    pub notices: Vec<Notice>,
}

/// The largest dimension handled.
const MAXD: usize = 4;
/// Centres computed side by side (the first axis): the lanes of [`Context::group`].
const LANES: usize = 8;
/// Flags of a centre.
const CONTRIBUTES: u8 = 1;
const WRITES_BIAS: u8 = 2;

/// Neighbourhood offsets padded to [`MAXD`] axes, with their steps in memory for one layout.
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

/// Everything the centres read.
///
/// The images are held **padded**: each axis gets the search and patch radii on both sides
/// (and [`LANES`] more voxels at the end of the first axis), where `valid` is 0 and the values
/// are 0. A centre then never needs coordinates: ITK's "is this voxel inside the image" tests
/// become reads of `valid`, and groups of [`LANES`] centres can run past the end of a row.
struct Context<'a> {
    d: usize,
    /// The image's size and strides.
    size: [usize; MAXD],
    /// The padding below each axis.
    pad: [usize; MAXD],
    /// The padded image's size and strides.
    psize: [usize; MAXD],
    pstrides: [usize; MAXD],
    valid: Vec<u8>,
    input: Vec<f32>,
    mean: Vec<f32>,
    variance: Vec<f32>,
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
    /// The search and patch offsets in the padded layout.
    search: Offsets,
    patch: Offsets,
    /// The patch offsets in the image's layout (for summing the outputs).
    image_patch: Offsets,
    /// The search offset of the centre itself (`static_cast<unsigned>(0.5 · size)`).
    search_centre: usize,
}

/// The [`LANES`] values of `data` from `at`.
#[inline(always)]
fn lanes<T>(data: &[T], at: usize) -> &[T; LANES] {
    data[at..at + LANES].try_into().expect("LANES values")
}

/// Per-thread scratch space of a group of centres.
struct Scratch {
    /// Per search offset, the lanes whose neighbour takes part (bit `l`).
    masks: Vec<u8>,
    /// Per patch offset, the lanes' weighted sums.
    weighted: Vec<[f32; LANES]>,
    /// Per patch offset, the lanes' `input − mean`.
    centre: Vec<[f32; LANES]>,
    /// Per patch offset, whether the lanes' patch voxels are inside the image.
    centre_valid: Vec<[bool; LANES]>,
    /// The neighbours with a weight in some lane: their padded index and the lanes' weights.
    weights: Vec<(usize, [f32; LANES])>,
    /// The neighbours that take part in some lane: their padded index and lane mask.
    neighbours: Vec<(usize, u8)>,
}

impl Scratch {
    fn new(patch: usize, search: usize) -> Self {
        Scratch {
            masks: Vec::with_capacity(search),
            weighted: Vec::with_capacity(patch),
            centre: Vec::with_capacity(patch),
            centre_valid: Vec::with_capacity(patch),
            weights: Vec::with_capacity(search),
            neighbours: Vec::with_capacity(search),
        }
    }
}

impl Context<'_> {
    /// The padded index of the first voxel of image row `row` (every axis but the first).
    fn padded_row(&self, row: usize) -> usize {
        let mut rest = row;
        let mut at = self.pad[0];
        for k in 1..self.d {
            let c = rest % self.size[k];
            rest /= self.size[k];
            at += (c + self.pad[k]) * self.pstrides[k];
        }
        at
    }

    /// `src` (on the image's grid) in the padded layout, 0 in the padding.
    fn padded<T: Copy + Default>(&self, src: &[T]) -> Vec<T> {
        let mut out = vec![T::default(); self.psize.iter().product()];
        let nx = self.size[0];
        for (row, line) in src.chunks(nx).enumerate() {
            let at = self.padded_row(row);
            out[at..at + nx].copy_from_slice(line);
        }
        out
    }

    fn mask_allows(&self, row_coords: &[usize; MAXD], x: usize) -> bool {
        match self.mask {
            None => true,
            Some(m) => {
                let i: usize = x * self.mask_strides[0]
                    + (1..self.d)
                        .map(|k| row_coords[k] * self.mask_strides[k])
                        .sum::<usize>();
                m.data[i] != 0
            }
        }
    }

    /// A group of [`LANES`] centres of one row: image columns `x0 ..` (those past the end of
    /// the row are left out), padded index `at` for the first. For each centre, the
    /// operations of ITK's loop body in its order; the lanes run side by side. `MASKED` is
    /// false when every search box and patch of the group is inside the image (no `valid`
    /// test is needed).
    #[allow(clippy::too_many_arguments)]
    fn group<const MASKED: bool>(
        &self,
        row_coords: &[usize; MAXD],
        x0: usize,
        at: usize,
        contribution: &mut [f32],
        flags: &mut [u8],
        bias: &mut [f32],
        scratch: &mut Scratch,
    ) {
        let p = self.p;
        let np = self.patch.len();
        let nx = self.size[0];
        let active = (nx - x0).min(LANES);
        let mc = *lanes(&self.mean, at);
        let vc = *lanes(&self.variance, at);
        let xc = lanes(&self.input, at);
        let mut takes_part = [false; LANES];
        for l in 0..active {
            takes_part[l] = xc[l] > 0.0
                && mc[l] > p.epsilon
                && vc[l] > p.epsilon
                && self.mask_allows(row_coords, x0 + l);
        }
        // Step 1: the neighbours that take part, per lane, and the smallest patch distance.
        // Every lane computes every test (they have no side effects), so the lanes run side by
        // side; the outcome is what ITK's early `continue`s give.
        let masks = &mut scratch.masks;
        masks.clear();
        masks.resize(self.search.len(), 0);
        let mut minimum_distance = [f32::MAX; LANES];
        let (eps, mt, vt, maximum) = (
            p.epsilon,
            p.mean_threshold,
            p.variance_threshold,
            self.maximum,
        );
        let (inv_mt, inv_vt) = (1.0f32 / mt, 1.0f32 / vt);
        if takes_part.iter().any(|&t| t) {
            for (m, mask) in masks.iter_mut().enumerate() {
                if m == self.search_centre {
                    continue;
                }
                let b0 = (at as isize + self.search.linear[m]) as usize;
                let xb = lanes(&self.input, b0);
                let mb = lanes(&self.mean, b0);
                let vb = lanes(&self.variance, b0);
                let pd = lanes(&self.patch_distance, b0);
                let inside = lanes(&self.valid, b0);
                let mut bits = 0u8;
                for l in 0..LANES {
                    let r = mc[l] / mb[l];
                    let ri = (maximum - mc[l]) / (maximum - mb[l]);
                    let rv = vc[l] / vb[l];
                    // `input <= 0` and `mean <= ε || variance <= ε` skip the neighbour (NaN
                    // does not).
                    let skip_value = xb[l] <= 0.0;
                    let skip_statistics = mb[l] <= eps || vb[l] <= eps;
                    let pass = takes_part[l]
                        & (!MASKED || inside[l] != 0)
                        & !skip_value
                        & !skip_statistics
                        & (((r > mt) & (r < inv_mt)) | ((ri > mt) & (ri < inv_mt)))
                        & (rv > vt)
                        & (rv < inv_vt);
                    // std::min(averageDistance, minimumDistance)
                    let a = pd[l];
                    let smaller = if minimum_distance[l] < a {
                        minimum_distance[l]
                    } else {
                        a
                    };
                    minimum_distance[l] = if pass { smaller } else { minimum_distance[l] };
                    bits |= u8::from(pass) << l;
                }
                *mask = bits;
            }
        }
        let mut three_h = [0.0f32; LANES];
        for l in 0..LANES {
            if l < active {
                flags[l] = 0;
                bias[l] = 0.0;
            }
            if !takes_part[l] {
                continue;
            }
            if almost_equal_f32(minimum_distance[l], 0.0) {
                minimum_distance[l] = 1.0;
            }
            if p.rician {
                flags[l] |= WRITES_BIAS;
                bias[l] = if almost_equal_f32(minimum_distance[l], f32::MAX) {
                    0.0
                } else {
                    minimum_distance[l]
                };
            }
            three_h[l] = 3.0f32 * minimum_distance[l];
        }
        // The centres' patches, once.
        let centre = &mut scratch.centre;
        centre.clear();
        centre.extend(
            self.patch
                .linear
                .iter()
                .map(|&o| *lanes(&self.diff, (at as isize + o) as usize)),
        );
        let centre_valid = &mut scratch.centre_valid;
        centre_valid.clear();
        if MASKED {
            centre_valid.extend(
                self.patch
                    .linear
                    .iter()
                    .map(|&o| lanes(&self.valid, (at as isize + o) as usize).map(|v| v != 0)),
            );
        }
        // Step 3: the weights, in search order. The patch distances of two neighbours are
        // computed together (two independent chains of additions); each is still the sum over
        // the patch offsets in order.
        let weights = &mut scratch.weights;
        weights.clear();
        let mut maximum_weight = [0.0f32; LANES];
        let mut sum_of_weights = [0.0f32; LANES];
        let neighbours = &mut scratch.neighbours;
        neighbours.clear();
        neighbours.extend(
            masks
                .iter()
                .enumerate()
                .filter(|&(_, &mask)| mask != 0)
                .map(|(m, &mask)| ((at as isize + self.search.linear[m]) as usize, mask)),
        );
        for pair in neighbours.chunks(2) {
            let (ba, bb) = (pair[0].0, pair.get(1).map_or(pair[0].0, |q| q.0));
            let distances = self.distance_sums::<MASKED>(ba, bb, centre, centre_valid);
            for (&(b0, mask), &(sum, count)) in pair.iter().zip(&distances) {
                let count = if MASKED { count } else { [np as f32; LANES] };
                let mut argument = [0.0f32; LANES];
                let mut candidate = [false; LANES];
                for l in 0..LANES {
                    let average_distance = sum[l] / count[l];
                    argument[l] = -average_distance / minimum_distance[l];
                    candidate[l] = (mask >> l) & 1 == 1 && average_distance <= three_h[l];
                }
                let mut weight = [0.0f32; LANES];
                let mut any = false;
                for l in 0..LANES {
                    if candidate[l] {
                        weight[l] = math::expf(argument[l]);
                        any |= weight[l] > 0.0;
                    }
                }
                for l in 0..LANES {
                    // Lanes without a weight have 0 here, and `0 > maximum` is false.
                    if weight[l] > maximum_weight[l] {
                        maximum_weight[l] = weight[l];
                    }
                    let added = sum_of_weights[l] + weight[l];
                    sum_of_weights[l] = if weight[l] > 0.0 {
                        added
                    } else {
                        sum_of_weights[l]
                    };
                }
                if any {
                    weights.push((b0, weight));
                }
            }
        }
        // The weighted sums of the neighbours' patches: per patch offset, the neighbours in
        // search order (as ITK adds them), [`SUMS`] offsets at a time so that the sums stay in
        // registers.
        let weighted = &mut scratch.weighted;
        weighted.clear();
        weighted.resize(np, [0.0; LANES]);
        const SUMS: usize = 4;
        for n0 in (0..np).step_by(SUMS) {
            // Offsets past the end repeat the last one; their sums are not kept.
            let offsets: [isize; SUMS] =
                std::array::from_fn(|j| self.patch.linear[(n0 + j).min(np - 1)]);
            let mut acc = [[0.0f32; LANES]; SUMS];
            for &(b0, weight) in weights.iter() {
                for (a, &o) in acc.iter_mut().zip(&offsets) {
                    let at = (b0 as isize + o) as usize;
                    let v = lanes(&self.values, at);
                    if MASKED {
                        let xv = lanes(&self.valid, at);
                        for l in 0..LANES {
                            let added = a[l] + weight[l] * v[l];
                            a[l] = if (weight[l] > 0.0) & (xv[l] != 0) {
                                added
                            } else {
                                a[l]
                            };
                        }
                    } else {
                        for l in 0..LANES {
                            let added = a[l] + weight[l] * v[l];
                            a[l] = if weight[l] > 0.0 { added } else { a[l] };
                        }
                    }
                }
            }
            for (j, a) in acc.iter().enumerate() {
                if n0 + j < np {
                    weighted[n0 + j] = *a;
                }
            }
        }
        // The centres' own patches, with their largest weights.
        for l in 0..LANES {
            if !takes_part[l] || almost_equal_f32(maximum_weight[l], 0.0) {
                maximum_weight[l] = 1.0;
            }
        }
        for (n, (acc, &o)) in weighted.iter_mut().zip(&self.patch.linear).enumerate() {
            let v = lanes(&self.values, (at as isize + o) as usize);
            for l in 0..LANES {
                let added = acc[l] + maximum_weight[l] * v[l];
                acc[l] = if !MASKED || centre_valid[n][l] {
                    added
                } else {
                    acc[l]
                };
            }
        }
        for l in 0..active {
            sum_of_weights[l] += maximum_weight[l];
            if sum_of_weights[l] > 0.0 {
                flags[l] |= CONTRIBUTES;
                for (n, acc) in weighted.iter().enumerate() {
                    contribution[l * np + n] = acc[l] / sum_of_weights[l];
                }
            }
        }
    }

    /// The sums of `((input − mean)_b+o − (input − mean)_c+o)²` over the patch offsets `o`
    /// (those inside the image at both ends when `MASKED`) and their counts, for the
    /// neighbours `ba` and `bb` of the lanes' centres (whose values are `centre`).
    #[inline(always)]
    fn distance_sums<const MASKED: bool>(
        &self,
        ba: usize,
        bb: usize,
        centre: &[[f32; LANES]],
        centre_valid: &[[bool; LANES]],
    ) -> [([f32; LANES], [f32; LANES]); 2] {
        let mut sa = [0.0f32; LANES];
        let mut sb = [0.0f32; LANES];
        let mut ca = [0.0f32; LANES];
        let mut cb = [0.0f32; LANES];
        for (n, (&o, y)) in self.patch.linear.iter().zip(centre).enumerate() {
            let ia = (ba as isize + o) as usize;
            let ib = (bb as isize + o) as usize;
            let xa = lanes(&self.diff, ia);
            let xb = lanes(&self.diff, ib);
            if MASKED {
                let va = lanes(&self.valid, ia);
                let vb = lanes(&self.valid, ib);
                let yv = &centre_valid[n];
                for l in 0..LANES {
                    let ta = xa[l] - y[l];
                    let tb = xb[l] - y[l];
                    let both_a = (va[l] != 0) & yv[l];
                    let both_b = (vb[l] != 0) & yv[l];
                    let s = sa[l] + ta * ta;
                    sa[l] = if both_a { s } else { sa[l] };
                    let s = sb[l] + tb * tb;
                    sb[l] = if both_b { s } else { sb[l] };
                    let c = ca[l] + 1.0;
                    ca[l] = if both_a { c } else { ca[l] };
                    let c = cb[l] + 1.0;
                    cb[l] = if both_b { c } else { cb[l] };
                }
            } else {
                for l in 0..LANES {
                    let ta = xa[l] - y[l];
                    let tb = xb[l] - y[l];
                    sa[l] += ta * ta;
                    sb[l] += tb * tb;
                }
            }
        }
        [(sa, ca), (sb, cb)]
    }

    /// Computes the centres of image row `row`, [`LANES`] at a time.
    fn row(
        &self,
        row: usize,
        contribution: &mut [f32],
        flags: &mut [u8],
        bias: &mut [f32],
        scratch: &mut Scratch,
    ) {
        let np = self.patch.len();
        let nx = self.size[0];
        let mut row_coords = [0usize; MAXD];
        let mut rest = row;
        for (c, &n) in row_coords.iter_mut().zip(&self.size).take(self.d).skip(1) {
            *c = rest % n;
            rest /= n;
        }
        let at = self.padded_row(row);
        // The groups whose search boxes and patches are all inside the image.
        let r0 = self.search.radius[0] + self.patch.radius[0];
        let row_interior = (1..self.d).all(|k| {
            let r = self.search.radius[k] + self.patch.radius[k];
            row_coords[k] >= r && row_coords[k] + r < self.size[k]
        });
        for x0 in (0..nx).step_by(LANES) {
            let end = (x0 + LANES).min(nx);
            let contribution = &mut contribution[x0 * np..end * np];
            let flags = &mut flags[x0..end];
            let bias = &mut bias[x0..end];
            if row_interior && x0 >= r0 && x0 + LANES - 1 + r0 < nx {
                self.group::<false>(&row_coords, x0, at + x0, contribution, flags, bias, scratch);
            } else {
                self.group::<true>(&row_coords, x0, at + x0, contribution, flags, bias, scratch);
            }
        }
    }
}

/// The mean of `(input − mean)²` over each voxel's patch, the patch clipped to the image (in
/// the padded layout; 0 in the padding).
fn patch_distances(ctx: &Context<'_>, n_threads: usize) -> Result<Vec<f32>, FilterError> {
    let nx = ctx.size[0];
    let rows: usize = ctx.size[1..].iter().product();
    let lines: Vec<Vec<f32>> = larmorx_core::parallel::with_threads(n_threads, || {
        (0..rows)
            .into_par_iter()
            .map(|row| {
                let at = ctx.padded_row(row);
                (0..nx)
                    .map(|x| {
                        let v = at + x;
                        let mut sum = 0.0f32;
                        let mut count = 0.0f32;
                        for &o in &ctx.patch.linear {
                            let u = (v as isize + o) as usize;
                            if ctx.valid[u] != 0 {
                                let t = ctx.diff[u];
                                sum += t * t;
                                count += 1.0;
                            }
                        }
                        sum / count
                    })
                    .collect()
            })
            .collect()
    })?;
    let mut out = vec![0.0f32; ctx.valid.len()];
    for (row, line) in lines.iter().enumerate() {
        let at = ctx.padded_row(row);
        out[at..at + nx].copy_from_slice(line);
    }
    Ok(out)
}

/// The coordinates of image voxel `lin`.
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
            rician_bias: Vec::new(),
            notices: Vec::new(),
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

    // The padded layout.
    let size: [usize; MAXD] = std::array::from_fn(|k| input.size.get(k).copied().unwrap_or(1));
    let pad: [usize; MAXD] = std::array::from_fn(|k| {
        if k < d {
            parameters.search_radius[k] + parameters.patch_radius[k]
        } else {
            0
        }
    });
    let psize: [usize; MAXD] =
        std::array::from_fn(|k| size[k] + 2 * pad[k] + if k == 0 { LANES } else { 0 });
    let ps = strides(&psize);
    let pstrides: [usize; MAXD] = std::array::from_fn(|k| ps[k]);
    let search = Offsets::new(&parameters.search_radius, &psize[..d]);
    let patch = Offsets::new(&parameters.patch_radius, &psize[..d]);
    let search_centre = (0.5 * search.len() as f64) as usize;
    let mut ctx = Context {
        d,
        size,
        pad,
        psize,
        pstrides,
        valid: Vec::new(),
        input: Vec::new(),
        mean: Vec::new(),
        variance: Vec::new(),
        diff: Vec::new(),
        values: Vec::new(),
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
        image_patch: Offsets::new(&parameters.patch_radius, input.size),
        search_centre,
    };
    ctx.valid = ctx.padded(&vec![1u8; len]);
    ctx.input = ctx.padded(input.data);
    ctx.mean = ctx.padded(&mean);
    ctx.variance = ctx.padded(&variance);
    drop(variance);
    ctx.diff = ctx.padded(&diff);
    drop(diff);
    ctx.values = ctx.padded(&values);
    drop(values);
    ctx.patch_distance = patch_distances(&ctx, n_threads)?;
    let ctx = ctx;

    // The centres, block by block in raster order; each output voxel is summed once every
    // centre whose patch reaches it is known.
    let np = ctx.patch.len();
    let nx = ctx.size[0];
    let rows = len / nx;
    let reach = ctx
        .image_patch
        .linear
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .max(0) as usize;
    let block_rows = (block_voxels / nx).clamp(1, rows);
    let mut sums = vec![0.0f32; len];
    let mut counts = vec![0.0f32; len];
    let mut raw_bias = vec![0.0f32; len];
    // Whether a centre that took part wrote a noise estimate at the voxel (its patch reached
    // it): ANTS-44's voxels outside the mask are those.
    let mut bias_written = vec![false; len];
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
                    || Scratch::new(np, ctx.search.len()),
                    |scratch, (i, ((contribution, flags), bias))| {
                        ctx.row(row + i, contribution, flags, bias, scratch);
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
                    &mut bias_written[next_output..complete],
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
    drop(window);

    // AfterThreadedGenerateData.
    let mut bias_image = Vec::new();
    let overflowed = std::sync::atomic::AtomicUsize::new(0);
    let fix = parameters.upstream_defects.fix();
    // ANTS-44: the voxels outside the mask whose bias ITK leaves unsmoothed, with the bias
    // the other mode gives them.
    let mut other_bias: Vec<(usize, f32)> = Vec::new();
    if rician {
        let options = DiscreteGaussianOptions::new(d, f64::from(parameters.smoothing_variance));
        let smoothed = discrete_gaussian(
            VolumeRef::new(&raw_bias, input.size, input.spacing),
            &options,
            n_threads,
        )?;
        // `ItS.Get() > 0`: false for NaN too.
        let positive = |s: f32| s.partial_cmp(&0.0) == Some(std::cmp::Ordering::Greater);
        let outside_mask = |lin: usize| -> bool {
            ctx.mask.is_some() && {
                let c = coordinates(&ctx, lin);
                let rc: [usize; MAXD] = std::array::from_fn(|k| c[k] as usize);
                !ctx.mask_allows(&rc, rc[0])
            }
        };
        // The smoothed and corrected bias, and whether ITK's factor overflowed (ANTS-43).
        let corrected = |s: f32, m: f32| -> (f32, bool) {
            let snr = m / s.sqrt();
            let factor = correction_factor(snr);
            let mut v = 2.0f32 * s / factor;
            let overflow = factor.is_nan() && (v.is_nan() || v.is_infinite());
            if v.is_nan() || v.is_infinite() {
                v = if fix && factor.is_nan() {
                    2.0f32 * s / corrected_correction_factor(snr)
                } else {
                    0.0
                };
                if v.is_nan() || v.is_infinite() {
                    v = 0.0;
                }
            }
            (v, overflow)
        };
        if ctx.mask.is_some() {
            other_bias = larmorx_core::parallel::with_threads(n_threads, || {
                (0..len)
                    .into_par_iter()
                    .with_min_len(1 << 12)
                    .filter(|&lin| {
                        bias_written[lin]
                            && !almost_equal_f32(counts[lin], 0.0)
                            && positive(smoothed[lin])
                            && outside_mask(lin)
                    })
                    .map(|lin| {
                        let other = if fix {
                            raw_bias[lin]
                        } else {
                            corrected(smoothed[lin], mean[lin]).0
                        };
                        (lin, other)
                    })
                    .collect()
            })?;
        }
        bias_image = raw_bias;
        larmorx_core::parallel::with_threads(n_threads, || {
            bias_image
                .par_iter_mut()
                .zip(smoothed.par_iter())
                .zip(mean.par_iter())
                .enumerate()
                .with_min_len(1 << 12)
                .for_each(|(lin, ((b, &s), &m))| {
                    if !positive(s) {
                        return;
                    }
                    // ITK keeps the unsmoothed bias outside the mask (ANTS-44); corrected,
                    // the voxels the patches of centres in the mask reach get the smoothed and
                    // corrected bias too (the others outside the mask, which no centre that
                    // took part reached, are not denoised and keep 0).
                    let outside = outside_mask(lin);
                    if outside && !(fix && bias_written[lin]) {
                        return;
                    }
                    let (v, overflow) = corrected(s, m);
                    // ANTS-43 is counted where both modes compute the factor.
                    if overflow && !outside {
                        overflowed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    *b = v;
                });
        })?;
    }
    let mut notices = Notices::new(parameters.upstream_defects);
    let overflowed = overflowed.into_inner();
    if overflowed > 0 {
        notices.add(
            &defects::ANTS_43,
            if fix {
                format!(
                    "the Rician correction factor of {overflowed} voxel(s) (a high SNR) is \
                     computed without overflow; ITK's is NaN and their bias 0"
                )
            } else {
                format!(
                    "the Rician correction factor of {overflowed} voxel(s) (a high SNR) \
                     overflows and their bias is 0, as in ITK"
                )
            },
        );
    }
    let estimate = |lin: usize, bias: f32| -> f32 {
        let (s, n) = (sums[lin], counts[lin]);
        if almost_equal_f32(n, 0.0) {
            return s;
        }
        let mut estimate = s / n;
        if rician {
            estimate -= bias;
            if estimate < 0.0 {
                estimate = 0.0;
            }
            estimate = estimate.sqrt();
        }
        estimate
    };
    let denoised: Vec<f32> = larmorx_core::parallel::with_threads(n_threads, || {
        (0..len)
            .into_par_iter()
            .with_min_len(1 << 14)
            .map(|lin| estimate(lin, if rician { bias_image[lin] } else { 0.0 }))
            .collect()
    })?;
    let changed = other_bias
        .iter()
        .filter(|&&(lin, other)| estimate(lin, other).to_bits() != denoised[lin].to_bits())
        .count();
    if changed > 0 {
        let reached = other_bias.len();
        notices.add(
            &defects::ANTS_44,
            if fix {
                format!(
                    "{reached} voxel(s) outside the mask that patches reach get the smoothed and \
                     corrected Rician bias, which changes {changed} of them; ITK leaves them the \
                     raw noise estimate of a centre in the mask"
                )
            } else {
                format!(
                    "{reached} voxel(s) outside the mask that patches reach keep the raw noise \
                     estimate of a centre in the mask, as in ITK ({changed} differ from the \
                     smoothed and corrected bias)"
                )
            },
        );
    }
    Ok(AdaptiveNlmOutput {
        denoised,
        rician_bias: bias_image,
        notices: notices.into_vec(),
    })
}

/// Sums the contributions to output voxels `[from, from + sums.len())` in the single
/// thread's order: the centres in raster order, which for a fixed output voxel is the patch
/// offsets in reverse. The bias is the one written last (the first offset whose centre wrote
/// one).
fn gather(
    ctx: &Context<'_>,
    window: &Window,
    from: usize,
    sums: &mut [f32],
    counts: &mut [f32],
    bias: &mut [f32],
    written: &mut [bool],
) {
    const CHUNK: usize = 1 << 12;
    let patch = &ctx.image_patch;
    let np = patch.len();
    sums.par_chunks_mut(CHUNK)
        .zip(counts.par_chunks_mut(CHUNK))
        .zip(bias.par_chunks_mut(CHUNK))
        .zip(written.par_chunks_mut(CHUNK))
        .enumerate()
        .for_each(|(chunk, (((sums, counts), bias), written))| {
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
                    let r = patch.radius[k] as isize;
                    c[k] >= r && c[k] + r < ctx.size[k] as isize
                });
                let mut sum = 0.0f32;
                let mut count = 0.0f32;
                let mut last_bias: Option<f32> = None;
                for n in (0..np).rev() {
                    if !interior {
                        let inside = (0..ctx.d).all(|k| {
                            let x = c[k] - patch.steps[n][k];
                            x >= 0 && (x as usize) < ctx.size[k]
                        });
                        if !inside {
                            continue;
                        }
                    }
                    let centre = (p as isize - patch.linear[n]) as usize;
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
                written[i] = last_bias.is_some();
            }
        });
}

/// ITK's Rician correction factor without the overflow (ANTS-43): the same formula with
/// `e^{-θ²/2} [(2 + θ²) I₀(θ²/4) + θ² I₁(θ²/4)]²` computed as
/// `[(2 + θ²) e^{-θ²/4} I₀(θ²/4) + θ² e^{-θ²/4} I₁(θ²/4)]²`, in double, then ITK's clamp
/// (outside `[0.001, 10]`: 1). It tends to 1 at a high SNR, as the factor does.
fn corrected_correction_factor(snr: f32) -> f32 {
    let s2 = f64::from(snr) * f64::from(snr);
    let quarter = 0.25 * s2;
    let t = (2.0 + s2) * scaled_bessel_i0(quarter) + s2 * scaled_bessel_i1(quarter);
    let mut value = 2.0 + s2 - 0.125 * std::f64::consts::PI * (t * t);
    if !(0.001..=10.0).contains(&value) {
        value = 1.0;
    }
    value as f32
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
        let mut written = vec![false; n];
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
                                    written[j] = true;
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
            let fix = p.upstream_defects.fix();
            for i in 0..n {
                // ANTS-44: ITK corrects the bias only in the mask; fixed, also where a centre
                // that took part wrote one.
                if s[i] > 0.0 && ((fix && written[i]) || mask.is_none_or(|m| m[i] != 0)) {
                    let snr = mean[i] / s[i].sqrt();
                    let factor = correction_factor(snr);
                    let mut b = 2.0 * s[i] / factor;
                    if b.is_nan() || b.is_infinite() {
                        b = if fix && factor.is_nan() {
                            2.0 * s[i] / corrected_correction_factor(snr)
                        } else {
                            0.0
                        };
                        if b.is_nan() || b.is_infinite() {
                            b = 0.0;
                        }
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
        for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
            for rician in [true, false] {
                for (patch, search) in [(1usize, 2usize), (1, 1), (2, 1), (0, 3)] {
                    let mut p = AdaptiveNlmParameters::denoise_image(3, rician);
                    p.upstream_defects = mode;
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
                                "{mode}, rician {rician}, patch {patch}, search {search}, \
                                 mask {}, {threads} threads",
                                m.is_some()
                            );
                            // ANTS-44 is met only with a mask and the Rician model (the
                            // centres' patches reach the voxels the mask leaves out).
                            let named = got.notices.iter().any(|n| n.defect.id == "ANTS-44");
                            if !(rician && m.is_some()) {
                                assert!(!named);
                            } else if patch > 0 {
                                assert!(named, "{mode}, patch {patch}, search {search}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn lanes_match_the_single_thread_loops() {
        // Rows long enough for the lanes (interior runs of 8 centres and more), with a mask
        // that switches centres off inside a lane group, NaN, infinity and zeros.
        let size = [30usize, 9, 8];
        let spacing = [1.0, 1.1, 0.9];
        let mut data = phantom(size, 11);
        data[30 * 4 + 13] = f32::NAN;
        data[30 * 9 * 3 + 30 * 5 + 17] = f32::INFINITY;
        data[30 * 9 * 4 + 30 * 4 + 12] = 0.0;
        let mask: Vec<u8> = (0..data.len()).map(|i| u8::from(i % 5 != 2)).collect();
        for rician in [true, false] {
            for (patch, search) in [(1usize, 2usize), (2, 1), (1, 3)] {
                let mut p = AdaptiveNlmParameters::denoise_image(3, rician);
                p.upstream_defects = UpstreamDefects::Reproduce;
                p.patch_radius = vec![patch; 3];
                p.search_radius = vec![search; 3];
                for m in [None, Some(&mask[..])] {
                    let expected = reference(&data, size, spacing, m, &p);
                    let v = VolumeRef::new(&data, &size, &spacing);
                    let mref = m.map(|d| MaskRef {
                        data: d,
                        size: &size,
                    });
                    let got = adaptive_non_local_means(v, mref, &p, 2).unwrap();
                    assert!(
                        got.denoised
                            .iter()
                            .zip(&expected)
                            .all(|(a, b)| a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())),
                        "rician {rician}, patch {patch}, search {search}, mask {}",
                        m.is_some()
                    );
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
            p.upstream_defects = UpstreamDefects::Reproduce;
            p.patch_radius = vec![patch; 3];
            let expected = reference(&data, size, [1.0; 3], None, &p);
            for block in [1usize, 9, 20, 64, 1000] {
                let got = run(VolumeRef::new(&data, &size, &[1.0; 3]), None, &p, 2, block).unwrap();
                assert_eq!(got.denoised, expected, "block {block}");
            }
        }
    }

    #[test]
    fn rician_bias_at_a_high_snr() {
        // ANTS-43: the phantom's SNR is high enough for ITK's correction factor to overflow.
        let size = [9usize, 7, 6];
        let data = phantom(size, 3);
        let v = VolumeRef::new(&data, &size, &[1.0; 3]);
        let mut p = AdaptiveNlmParameters::denoise_image(3, true);
        let fixed = adaptive_non_local_means(v, None, &p, 2).unwrap();
        p.upstream_defects = UpstreamDefects::Reproduce;
        let ants = adaptive_non_local_means(v, None, &p, 2).unwrap();
        assert_eq!(fixed.notices[0].defect.id, "ANTS-43");
        assert_eq!(ants.notices[0].defect.id, "ANTS-43");
        assert_ne!(fixed.denoised, ants.denoised);
        // The corrected bias is never 0 where ITK's became 0, so the corrected values are lower.
        assert!(
            fixed
                .denoised
                .iter()
                .zip(&ants.denoised)
                .all(|(f, a)| f <= a)
        );
        // The Gaussian model has no such factor.
        let mut g = AdaptiveNlmParameters::denoise_image(3, false);
        let a = adaptive_non_local_means(v, None, &g, 1).unwrap();
        g.upstream_defects = UpstreamDefects::Reproduce;
        assert_eq!(a, adaptive_non_local_means(v, None, &g, 1).unwrap());
        assert!(a.notices.is_empty());
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
        // (the caller then sets the bias to 0; ANTS-43).
        assert!(correction_factor(20.0).is_nan());
        // Corrected: the same values where ITK's are finite (to float rounding), and close to
        // 1 above.
        for snr in [0.5f32, 1.0, 2.0, 5.0, 10.0, 12.0, 18.0] {
            let (a, b) = (correction_factor(snr), corrected_correction_factor(snr));
            assert!(
                a.is_nan() || (a - b).abs() <= 2e-4 * a.abs().max(1.0),
                "{snr}: {a} {b}"
            );
        }
        for snr in [20.0f32, 50.0, 100.0] {
            let c = corrected_correction_factor(snr);
            assert!((c - 1.0).abs() < 1e-2, "{snr}: {c}");
        }
    }
}
