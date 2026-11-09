// SPDX-License-Identifier: Apache-2.0
//! sdcflows' `brain_masker` (`sdcflows/utils/tools.py`, sdcflows 2.17.1; the same in 2.15.1):
//! a quick brain mask of an EPI or magnitude image by grey-level morphology and scikit-image's
//! random walker, on the ports of SciPy's `ndimage` and scikit-image (`larmorx-ndimage`) and
//! numpy's statistics (`larmorx_core::numpy`).
//!
//! The steps, on the image padded by `padding` zeros on every side (`np.pad`): the values below
//! the 15th percentile of the positive ones set to 0 (numpy's `percentile` with a Python-number
//! `q`: the release's arithmetic, [`NumpyRelease`]); the positive ones shifted down by their
//! minimum; the image inverted (`-data`, minus its minimum, divided by its maximum);
//! `grey_closing` with the non-flat `structure=ball(1)` (its values added: a flat 3³ footprint);
//! `median_filter` with `footprint=ball(3)`; scikit-image's Otsu threshold of that; the rough mask
//! `closed < threshold` (a `float32` 0/1 array), `binary_opening` with `ball(3)`; its largest
//! connected component (6-connectivity, every tie kept; with no component the background's label
//! is the largest: SDC-37); `binary_closing` with `ball(5)`; the markers (2 on the padded array's
//! outer shell, 0 inside and on the rough mask's `binary_dilation` with `ball(5)`, 1 on its
//! `binary_erosion` with `ball(5)`); the random walker of `2 * closed / closed.max() - 1` (with
//! the positive values of `closed` shifted down by their minimum first) with these markers, the
//! image's zooms as spacing (`float32`) and `return_full_prob=True`; the first label's
//! probability without the padding is the result (`>= 0.5`: the mask).
//!
//! Everything after the inversion runs on C-ordered arrays, as numpy's `copy()` and SciPy's
//! outputs are; the steps before it are elementwise or order-free.

use larmorx_core::numpy::histograms::min_max;
use larmorx_core::numpy::reduce::{SumOrder, std_ordered};
use larmorx_core::numpy::{NpFloat, NumpyRelease, quantile};
use larmorx_ndimage::skimage::{self, Mode, Numbering, Preprocessed, RandomWalker, SparseOrder};
use larmorx_ndimage::{
    BinaryErosion, Extend, Footprint, Labels, Layout, binary_erosion, dilation_arguments,
    generate_binary_structure, label, min_or_max_filter, rank_filter,
};

use crate::SdcError;

/// The options of [`brain_masker`].
#[derive(Clone, Copy, Debug)]
pub struct MaskerOptions {
    /// `padding` (sdcflows' default 5).
    pub padding: usize,
    /// numpy's release for the percentile of a Python-number `q`.
    pub numpy: NumpyRelease,
    /// numpy's order of a whole-array sum (the random walker's `data.std()`: numpy 2.2 sums in
    /// buffered chunks).
    pub sum_order: SumOrder,
    /// SciPy's order of the random walker's Laplacian rows.
    pub scipy: SparseOrder,
    /// Keep sdcflows' results where its inputs make no sense (`reproduce`): with no component in
    /// the rough mask, the whole image as the rough mask (SDC-37); with no brain seed left after
    /// the erosion, the background's probability as the brain's (SDC-38). By default both are
    /// errors.
    pub reproduce: bool,
}

impl Default for MaskerOptions {
    fn default() -> Self {
        MaskerOptions {
            padding: 5,
            numpy: NumpyRelease::V2_5,
            sum_order: SumOrder::Pairwise,
            scipy: SparseOrder::Scipy118,
            reproduce: false,
        }
    }
}

/// The result of [`brain_masker`].
#[derive(Clone, Debug, PartialEq)]
pub struct MaskerOutput {
    /// The probability of the brain (the random walker's first label), C-ordered over the input's
    /// shape, `float64` (sdcflows writes it as `float32`).
    pub probseg: Vec<f64>,
    /// `probseg >= 0.5` as 0/1.
    pub mask: Vec<u8>,
    /// The Otsu threshold of the denoised inverted image.
    pub threshold: f32,
    /// The random walker's conjugate-gradient iterations per label.
    pub iterations: Vec<usize>,
    /// scikit-image's warnings, as their messages.
    pub warnings: Vec<String>,
}

fn nd(e: larmorx_ndimage::NdimageError) -> SdcError {
    SdcError::Invalid(e.to_string())
}

/// A binary erosion (or, `dilate`, a dilation) of 0/1 values with a structure, one iteration,
/// border 0 (`scipy.ndimage.binary_erosion(input, structure)` / `binary_dilation`).
fn binary<T: larmorx_ndimage::NdElement>(
    input: &[T],
    layout: &Layout,
    structure: &Footprint,
    dilate: bool,
) -> Result<Vec<u8>, SdcError> {
    let origins = vec![0isize; layout.ndim()];
    let (s, o) = if dilate {
        dilation_arguments(structure, &origins)
    } else {
        (structure.clone(), origins)
    };
    binary_erosion::<T, u8>(
        input,
        layout,
        BinaryErosion {
            structure: &s,
            origins: &o,
            iterations: 1,
            border_value: false,
            invert: dilate,
            brute_force: false,
        },
        None,
    )
    .map_err(nd)
}

/// sdcflows' `brain_masker` of a 3-D image (`data`, C-ordered `shape`; `zooms`, the header's
/// first three). Runs on the threads of the current rayon pool.
pub fn brain_masker(
    data: &[f32],
    shape: [usize; 3],
    zooms: [f32; 3],
    opts: MaskerOptions,
) -> Result<MaskerOutput, SdcError> {
    let n: usize = shape.iter().product();
    if data.len() != n {
        return Err(SdcError::Invalid("data and shape disagree".into()));
    }
    let p = opts.padding;
    let ps = [shape[0] + 2 * p, shape[1] + 2 * p, shape[2] + 2 * p];
    let np_: usize = ps.iter().product();
    let layout = Layout::c(&ps);
    let mut d = vec![0.0f32; np_];
    for i in 0..shape[0] {
        for j in 0..shape[1] {
            let src = (i * shape[1] + j) * shape[2];
            let dst = ((i + p) * ps[1] + j + p) * ps[2] + p;
            d[dst..dst + shape[2]].copy_from_slice(&data[src..src + shape[2]]);
        }
    }
    // data[data < percentile(data[data > 0], 15)] = 0
    let pos: Vec<f32> = d.iter().copied().filter(|&x| x > 0.0).collect();
    let p15 = quantile::percentile_linear_weak(&pos, 15.0, opts.numpy)
        .map_err(|e| SdcError::Invalid(e.to_string()))?;
    drop(pos);
    for x in &mut d {
        if *x < p15 {
            *x = 0.0;
        }
    }
    // data[data > 0] -= data[data > 0].min()
    if let Some(m) = d
        .iter()
        .copied()
        .filter(|&x| x > 0.0)
        .reduce(|a, b| if b < a { b } else { a })
    {
        for x in &mut d {
            if *x > 0.0 {
                *x -= m;
            }
        }
    }
    // datainv = -data; datainv -= datainv.min(); datainv /= datainv.max() (numpy's min and max
    // propagate a NaN; a constant image divides 0 by 0: SDC-37)
    for x in &mut d {
        *x = -*x;
    }
    let (mn, _) = min_max(&d);
    for x in &mut d {
        *x = NpFloat::sub(*x, mn);
    }
    let (_, mx) = min_max(&d);
    for x in &mut d {
        *x = NpFloat::div(*x, mx);
    }
    // closed = grey_closing(datainv, structure=ball(1)): a flat 3x3x3 footprint and the ball's
    // values added (the dilation's structure reversed: the ball is symmetric)
    let b1 = skimage::ball(1.0, true);
    let s1: Vec<f64> = b1.mask.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();
    let cube = Footprint::full(&b1.shape);
    let o3 = [0isize; 3];
    let dil: Vec<f32> = min_or_max_filter(
        &d,
        &layout,
        &cube,
        Some(&s1),
        Extend::Reflect,
        0.0,
        &o3,
        false,
    )
    .map_err(nd)?;
    drop(d);
    let mut closed: Vec<f32> = min_or_max_filter(
        &dil,
        &layout,
        &cube,
        Some(&s1),
        Extend::Reflect,
        0.0,
        &o3,
        true,
    )
    .map_err(nd)?;
    drop(dil);
    // denoised = median_filter(closed, footprint=ball(3)); th = threshold_otsu(denoised)
    let b3 = skimage::ball(3.0, true);
    let denoised: Vec<f32> = rank_filter(
        &closed,
        &layout,
        b3.count() / 2,
        &b3,
        Extend::Reflect,
        0.0,
        &o3,
    )
    .map_err(nd)?;
    let th =
        skimage::threshold_otsu(&denoised, 256).map_err(|e| SdcError::Invalid(e.to_string()))?;
    drop(denoised);
    // closedbin[closed < th] = 1 (float32); binary_opening(.., ball(3))
    let rough: Vec<f32> = closed
        .iter()
        .map(|&x| if x < th { 1.0 } else { 0.0 })
        .collect();
    let eroded = binary(&rough, &layout, &b3, false)?;
    drop(rough);
    let opened = binary(&eroded, &layout, &b3, true)?;
    drop(eroded);
    // the largest component: label, sizes = sum(closedbin, label_im, range(n + 1))
    let (labels, nb) = label(&opened, &layout, &generate_binary_structure(3, 1)).map_err(nd)?;
    let lab: Vec<i64> = match labels {
        Labels::I32(v) => v.into_iter().map(i64::from).collect(),
        Labels::I64(v) => v,
    };
    let mut sizes = vec![0u64; nb + 1];
    for (&l, &o) in lab.iter().zip(&opened) {
        if o != 0 {
            sizes[l as usize] += 1;
        }
    }
    let max = *sizes.iter().max().expect("label 0");
    if max == 0 && !opts.reproduce {
        return Err(SdcError::Invalid(
            "brain_masker: the rough mask has no connected component (sdcflows would take the \
             whole image as the brain: SDC-37; upstream_defects='reproduce' gives its result)"
                .into(),
        ));
    }
    let keep: Vec<bool> = sizes.iter().map(|&s| s == max).collect();
    let largest: Vec<u8> = lab.iter().map(|&l| u8::from(keep[l as usize])).collect();
    drop(lab);
    drop(opened);
    // binary_closing(.., ball(5)): a dilation, then an erosion
    let b5 = skimage::ball(5.0, true);
    let tmp = binary(&largest, &layout, &b5, true)?;
    let closedbin = binary(&tmp, &layout, &b5, false)?;
    drop(tmp);
    drop(largest);
    // markers: 2 on the outer shell, 0 inside, 0 on the dilation, 1 on the erosion (ball(5))
    let dil5 = binary(&closedbin, &layout, &b5, true)?;
    let ero5 = binary(&closedbin, &layout, &b5, false)?;
    drop(closedbin);
    let mut markers = vec![0i64; np_];
    for i in 0..ps[0] {
        for j in 0..ps[1] {
            for k in 0..ps[2] {
                let g = (i * ps[1] + j) * ps[2] + k;
                let shell = i == 0
                    || j == 0
                    || k == 0
                    || i + 1 == ps[0]
                    || j + 1 == ps[1]
                    || k + 1 == ps[2];
                let mut m = if shell { 2 } else { 0 };
                if dil5[g] != 0 {
                    m = 0;
                }
                if ero5[g] != 0 {
                    m = 1;
                }
                markers[g] = m;
            }
        }
    }
    drop(dil5);
    drop(ero5);
    let brain_seeds = markers.contains(&1);
    // closed[closed > 0] -= closed[closed > 0].min(); segtarget = 2 * closed / closed.max() - 1
    let positive: Vec<f32> = closed.iter().copied().filter(|&x| x > 0.0).collect();
    if positive.is_empty() {
        return Err(SdcError::Invalid(
            "zero-size array to reduction operation minimum which has no identity".into(),
        ));
    }
    let (m, _) = min_max(&positive);
    drop(positive);
    for x in &mut closed {
        if *x > 0.0 {
            *x = NpFloat::sub(*x, m);
        }
    }
    let (_, cmax) = min_max(&closed);
    let seg: Vec<f32> = closed
        .iter()
        .map(|&x| NpFloat::sub(NpFloat::div(NpFloat::mul(2.0f32, x), cmax), 1.0))
        .collect();
    drop(closed);
    let prep =
        match skimage::preprocess(&markers, ps).map_err(|e| SdcError::Invalid(e.to_string()))? {
            Preprocessed::Ready(p) => p,
            Preprocessed::NoZero | Preprocessed::AllIsolated(_) => {
                return Err(SdcError::Invalid(
                    "brain_masker: the random walker has no pixel to segment".into(),
                ));
            }
        };
    drop(markers);
    // label 1 is the brain unless the erosion left no seed of it
    if !brain_seeds && !opts.reproduce {
        return Err(SdcError::Invalid(
            "brain_masker: no brain seed is left after the rough mask's erosion by ball(5) \
             (sdcflows would take the background's probability as the brain's: SDC-38; \
             upstream_defects='reproduce' gives its result)"
                .into(),
        ));
    }
    let rw = RandomWalker {
        data: &seg,
        shape: ps,
        channels: 1,
        std: Some(std_ordered(&seg, 0, opts.sum_order)),
        multichannel: false,
        spacing: skimage::random_walker::Spacing::F32(zooms),
        beta: 130.0,
        mode: Mode::CgJ,
        tol: 1e-3,
        prob_tol: 1e-3,
        order: opts.scipy,
        numbering: Numbering::Mask,
    };
    let out = skimage::random_walker(&rw, &prep).map_err(|e| SdcError::Invalid(e.to_string()))?;
    let full = out.full_prob(&prep);
    drop(seg);
    // labels[0, p:-p, p:-p, p:-p]
    let mut probseg = Vec::with_capacity(n);
    for i in 0..shape[0] {
        for j in 0..shape[1] {
            let g = ((i + p) * ps[1] + j + p) * ps[2] + p;
            probseg.extend_from_slice(&full[g..g + shape[2]]);
        }
    }
    let mask = probseg.iter().map(|&v| u8::from(v >= 0.5)).collect();
    let warnings = out
        .warnings
        .iter()
        .map(|w| match w {
            skimage::RwWarning::NotConverged => "Conjugate gradient convergence to tolerance not \
                achieved. Consider decreasing beta to improve system conditionning."
                .to_string(),
            skimage::RwWarning::ProbabilityRange => "The probability range is outside [0, 1] \
                given the tolerance `prob_tol`. Consider decreasing `beta` and/or decreasing \
                `tol`."
                .to_string(),
        })
        .collect();
    Ok(MaskerOutput {
        probseg,
        mask,
        threshold: th,
        iterations: out.iterations,
        warnings,
    })
}
