// SPDX-License-Identifier: Apache-2.0
//! `3dAutomask`: a brain mask from an EPI series or a skull-stripped anatomical
//! (`specs/3dAutomask.md`), clean-room.
//!
//! - [`average_abs`]: the image the mask is made from (§3);
//! - [`form_mask`]: the mask of that image (§5 for a volume, §7 for a plane), with the
//!   messages AFNI prints while forming it; 3dUnifize uses it too;
//! - [`program`]: the program's own steps after the mask (§6: dilations, erosions, hole
//!   filling, the depth, the empty planes and `-SI`).
//!
//! Built for speed (`docs/porting-guide.md`, "Clean-room originals"): connected clusters by
//! run-based union-find ([`cluster`]), every pass of the mask operations decided voxel by
//! voxel in parallel over z planes on the mask before the pass ([`morph`]), the exterior clip
//! and the depth by breadth-first searches instead of repeated sweeps, the clip levels'
//! histograms in parallel with exact integer sums ([`clip`]). The result never depends on the
//! thread count.

pub mod clip;
pub mod cluster;
pub mod morph;
pub mod program;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::parallel;
use larmorx_io::nifti::NiftiHeader;
use rayon::prelude::*;

use crate::dataset::{self, AfniImage, BrickData, Bricks, OutputTiming};
use crate::geometry;
use crate::msg::Message;
use crate::settings::AfniSettings;
use crate::timing;
use clip::{GradualLevel, clip_level};
use morph::{Neighbourhood, Stencil};

/// What 3dAutomask computes (its options, `specs/3dAutomask.md` §8.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutomaskOptions {
    /// The mask's settings; `peels` is set from [`AutomaskOptions::requested_peels`].
    pub mask: MaskSettings,
    /// `-peels` as given (AFNI-43: below 1 AFNI peels once; the default does not peel).
    pub requested_peels: i64,
    /// `-NN1/2/3`: the neighbourhood of dilations, erosions and the depth.
    pub nn: Neighbourhood,
    pub dilate: usize,
    pub erode: usize,
    /// `-SI` (millimetres).
    pub si: Option<f32>,
    /// `-apply_prefix`: the masked input is wanted.
    pub apply: bool,
    /// `-depth`: the depth is wanted.
    pub depth: bool,
    /// Whether the progress messages are shown (not `-q`): AFNI-43 is named where only the
    /// clustering messages differ only when they are shown (the replica's, observed).
    pub messages_shown: bool,
}

impl Default for AutomaskOptions {
    fn default() -> AutomaskOptions {
        AutomaskOptions {
            mask: MaskSettings::default(),
            requested_peels: 1,
            nn: Neighbourhood::Edges,
            dilate: 0,
            erode: 0,
            si: None,
            apply: false,
            depth: false,
            messages_shown: true,
        }
    }
}

/// An output: its header and voxels.
pub type Output = (NiftiHeader, Bricks);

/// What [`run`] computed.
#[derive(Debug)]
pub struct AutomaskRun {
    /// AFNI's progress messages, from `Forming automask` to the `-SI` lines.
    pub messages: Vec<Message>,
    /// The defects that changed (or, reproduced, would have changed) a result.
    pub notices: Vec<Notice>,
    /// A fatal error after the messages: no mask (a plane), or an empty mask.
    pub failure: Option<String>,
    /// The mask (uint8, one volume) and its header (`None` after a failure).
    pub mask: Option<Output>,
    /// The masked input (with [`AutomaskOptions::apply`]).
    pub masked: Option<Output>,
    /// The depth (with [`AutomaskOptions::depth`]).
    pub depth: Option<Output>,
}

/// The mask of `image` and what 3dAutomask writes with it (spec §3 to §7), on `n_threads`
/// threads (the result does not depend on it). `image` is modified only by AFNI-23's
/// correction (data `AFNI_NIFTI_SCALE=NO` kept as stored are read as without the variable).
pub fn run(
    image: &mut AfniImage,
    options: &AutomaskOptions,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    n_threads: usize,
) -> Result<AutomaskRun, String> {
    parallel::with_threads(n_threads, || run_here(image, options, mode, settings))
        .map_err(|e| e.to_string())?
}

/// The AFNI-23 notice of 3dAutomask and 3dUnifize, and the correction: data
/// `AFNI_NIFTI_SCALE=NO` kept as stored are read as without the variable by default.
pub fn intercept_correction(
    image: &mut AfniImage,
    mode: UpstreamDefects,
    tool: &str,
) -> Option<Notice> {
    let kept = image.kept_intercept?;
    let name = match kept.stored {
        larmorx_core::element::DataType::U8 => "UINT8",
        _ => "INT16",
    };
    let (slope, inter) = (crate::cfmt::g32(kept.slope), crate::cfmt::g32(kept.inter));
    let detail = match mode {
        UpstreamDefects::Fix => {
            dataset::read_scaled(&mut image.bricks, &kept);
            format!(
                "AFNI_NIFTI_SCALE=NO: the {name} data are read as float32 with scl_slope {slope} \
                 and scl_inter {inter} applied, as without the variable; AFNI keeps them as \
                 {name} and drops the intercept, so {tool} works on values off by {inter}"
            )
        }
        UpstreamDefects::Reproduce => format!(
            "AFNI_NIFTI_SCALE=NO: the {name} data are kept as stored and scl_inter {inter} is \
             dropped, as AFNI does: {tool} works on values off by {inter}"
        ),
    };
    Some(Notice::new(&defects::AFNI_23, mode, detail))
}

/// The time fields of an output that keeps the input's time axis (the masked input, `-EPI`'s
/// output): the input's TR and `toffset`, and the slice fields of its header's slice timing.
///
/// Header slice code 2 changes the written fields in both modes (AFNI-03: AFNI reads every
/// slice time as 0, so it writes slice code 0 and, by AFNI-08, `slice_end` 0), so AFNI-03 is
/// named whenever such an output is written, and AFNI-08 whenever the kept times are all
/// within 3 ms of 0; the notices' words are the replica program's for the same call
/// (`specs/3dAutomask.md` §9, observed by the clean-room implementer).
pub fn copy_timing(
    image: &AfniImage,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> OutputTiming {
    let slices = image.slice_timing.as_ref().map(|h| {
        let (fields, zero) = timing::copy_slice_fields(&h.times, mode);
        if h.code == 2 {
            let detail = match mode {
                UpstreamDefects::Fix => format!(
                    "header slice code 2 (sequential decreasing): the output keeps slices \
                     {}..{}'s times (slice code {}, slice_end {}); AFNI reads every slice time \
                     as 0 and writes slice code 0, slice_end 0",
                    h.end, h.start, fields.code, fields.end
                ),
                UpstreamDefects::Reproduce => "header slice code 2 (sequential decreasing) read \
                     as every slice time 0, as AFNI does"
                    .to_owned(),
            };
            notices.push(Notice::new(&defects::AFNI_03, mode, detail));
        }
        if zero {
            let detail = match mode {
                UpstreamDefects::Fix => format!(
                    "the {} slice times kept in the output are all within 0.003 of 0: no slice \
                     timing, slice_end {}; AFNI writes slice_end 0",
                    h.times.len(),
                    fields.end
                ),
                UpstreamDefects::Reproduce => "the slice times kept in the output are all \
                     within 0.003 of 0: slice_end 0, as AFNI writes it"
                    .to_owned(),
            };
            notices.push(Notice::new(&defects::AFNI_08, mode, detail));
        }
        fields
    });
    OutputTiming {
        tr: image.tr,
        toffset: image.toffset,
        slices,
    }
}

/// A header for `bricks` on `image`'s grid (spec §2), with the geometry notices.
pub fn header_for(
    image: &AfniImage,
    bricks: &Bricks,
    timing: &OutputTiming,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    notices: &mut Vec<Notice>,
    messages: &mut Vec<Message>,
) -> Result<NiftiHeader, String> {
    let (header, n, m) =
        dataset::output_header(&image.geometry, bricks, timing, None, mode, settings)?;
    for x in n {
        defects::push_once(notices, x);
    }
    messages.extend(m);
    Ok(header)
}

/// One volume of `data` on `dims`, without a factor.
fn volume(dims: [usize; 3], data: BrickData) -> Bricks {
    Bricks::new([dims[0], dims[1], dims[2], 1], data, vec![0.0]).expect("the data fit the grid")
}

const NO_TIME: OutputTiming = OutputTiming {
    tr: 0.0,
    toffset: 0.0,
    slices: None,
};

fn run_here(
    image: &mut AfniImage,
    options: &AutomaskOptions,
    mode: UpstreamDefects,
    settings: &AfniSettings,
) -> Result<AutomaskRun, String> {
    let mut messages = vec![Message::Info("Forming automask".into())];
    let mut notices = Vec::new();
    notices.extend(intercept_correction(image, mode, "3dAutomask"));
    let [nx, ny, nz, _] = image.bricks.shape;
    let dims = [nx, ny, nz];
    let (v, negative) = average_abs(&image.bricks, mode);
    let mut settings_mask = options.mask;
    // AFNI-43 concerns `-peels 0` (a value read as 0): the default does not peel; a negative
    // value peels once in both modes, as in AFNI (observed by the clean-room implementer: the
    // replica program's default, without a notice).
    settings_mask.peels = if options.requested_peels >= 1 {
        options.requested_peels as usize
    } else if options.requested_peels == 0 && mode.fix() {
        0
    } else {
        1
    };
    let mut log = Vec::new();
    let formed = form_mask(&v, dims, &settings_mask, &mut log);
    let clustering = log.clone();
    messages.extend(log.into_iter().map(Message::More));
    let mut mask = match formed {
        Ok(m) => m,
        Err(NoMask) => {
            return Ok(failed(
                "Mask creation fails for unknown reasons!",
                messages,
                notices,
            ));
        }
    };
    // AFNI-43: -peels 0 (AFNI peels once). Named where the mask differs from the other mode's,
    // or, with the same mask, where the clustering messages count other voxels; the words are
    // the replica program's for the same call (observed by the clean-room implementer).
    if options.requested_peels == 0 {
        let mut other = settings_mask;
        other.peels = if mode.fix() { 1 } else { 0 };
        let mut other_log = Vec::new();
        let alt = form_mask(&v, dims, &other, &mut other_log).unwrap_or_default();
        let detail = match (mode, differing(&mask, &alt) > 0) {
            (UpstreamDefects::Fix, true) => {
                Some("-peels 0: the mask is not peeled, as the help says; AFNI peels it once")
            }
            (UpstreamDefects::Reproduce, true) => Some(
                "-peels 0: the mask is peeled once, as AFNI does (its help says 0 turns the \
                 peeling off)",
            ),
            (UpstreamDefects::Fix, false) if options.messages_shown && other_log != clustering => {
                Some(
                    "-peels 0: the mask is not peeled, as the help says (AFNI peels it once): the \
                 same mask, but the clustering messages count other voxels",
                )
            }
            (UpstreamDefects::Reproduce, false)
                if options.messages_shown && other_log != clustering =>
            {
                Some(
                    "-peels 0: the mask is peeled once, as AFNI does: the same mask as without \
                     peeling, but the clustering messages count other voxels",
                )
            }
            _ => None,
        };
        if let Some(detail) = detail {
            notices.push(Notice::new(&defects::AFNI_43, mode, detail.to_owned()));
        }
    }
    // AFNI-01: a negative factor in a series.
    if negative {
        let other = if mode.fix() {
            UpstreamDefects::Reproduce
        } else {
            UpstreamDefects::Fix
        };
        let (w, _) = average_abs(&image.bricks, other);
        let alt = form_mask(&w, dims, &settings_mask, &mut Vec::new()).unwrap_or_default();
        let differ = differing(&mask, &alt);
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "a negative scale factor is applied when the sub-bricks are averaged, where AFNI \
                 drops it (and its magnitude): the mask differs from AFNI's in {differ} voxels"
            ),
            UpstreamDefects::Reproduce => format!(
                "a negative scale factor is dropped when the sub-bricks are averaged, as AFNI \
                 does: the mask differs in {differ} voxels from the one with the factor"
            ),
        };
        notices.push(Notice::new(&defects::AFNI_01, mode, detail));
    }
    // §6: the depth, -dilate and -erode, the count, the planes, -SI.
    let steps = program::Steps {
        nn: options.nn,
        dilate: options.dilate,
        erode: options.erode,
        si: options.si,
        depth: options.depth,
    };
    let depth = options
        .depth
        .then(|| program::depth_of(&mask, dims, steps.nn, &mut messages));
    program::dilate_erode(&mut mask, dims, &steps, &mut messages);
    if program::report_count(&mask, &mut messages).is_err() {
        return Ok(failed(
            "No voxels? Quitting without saving mask",
            messages,
            notices,
        ));
    }
    let (out_geometry, _) = geometry::output_geometry(&image.geometry, mode);
    let axes = program::axes_of(&image.geometry, out_geometry.pixdim);
    let planes = program::empty_planes(&mask, dims, &axes, &mut messages);
    if let Some(h) = options.si {
        program::si_clip(&mut mask, dims, &axes, &planes, h, &mut messages);
    }
    // The outputs.
    let mut out_messages = Vec::new();
    let mask_bricks = volume(dims, BrickData::Byte(mask));
    let mask_header = header_for(
        image,
        &mask_bricks,
        &NO_TIME,
        mode,
        settings,
        &mut notices,
        &mut out_messages,
    )?;
    let masked = if options.apply {
        let BrickData::Byte(m) = &mask_bricks.data else {
            unreachable!()
        };
        let bricks = apply_mask(&image.bricks, m);
        let timing = copy_timing(image, mode, &mut notices);
        let header = header_for(
            image,
            &bricks,
            &timing,
            mode,
            settings,
            &mut notices,
            &mut out_messages,
        )?;
        Some((header, bricks))
    } else {
        None
    };
    let depth = match depth {
        Some(d) => {
            let bricks = volume(dims, BrickData::Short(d));
            let header = header_for(
                image,
                &bricks,
                &NO_TIME,
                mode,
                settings,
                &mut notices,
                &mut out_messages,
            )?;
            Some((header, bricks))
        }
        None => None,
    };
    messages.extend(out_messages);
    Ok(AutomaskRun {
        messages,
        notices,
        failure: None,
        mask: Some((mask_header, mask_bricks)),
        masked,
        depth,
    })
}

/// A run that stops with `why` after its messages.
fn failed(why: &str, messages: Vec<Message>, notices: Vec<Notice>) -> AutomaskRun {
    AutomaskRun {
        messages,
        notices,
        failure: Some(why.into()),
        mask: None,
        masked: None,
        depth: None,
    }
}

/// The number of voxels where two masks differ (all of the first when the second is empty).
fn differing(a: &[u8], b: &[u8]) -> usize {
    if b.len() != a.len() {
        return a.len();
    }
    a.par_iter()
        .zip(b.par_iter())
        .filter(|(x, y)| x != y)
        .count()
}

/// The input with every voxel outside `mask` 0, in its storage type and with its factors.
pub fn apply_mask(bricks: &Bricks, mask: &[u8]) -> Bricks {
    let n = mask.len();
    fn zero<T: Copy + Default + Send + Sync>(v: &[T], mask: &[u8], n: usize) -> Vec<T> {
        let mut out = v.to_vec();
        out.par_chunks_mut(n).for_each(|vol| {
            for (x, &m) in vol.iter_mut().zip(mask) {
                if m == 0 {
                    *x = T::default();
                }
            }
        });
        out
    }
    let data = match &bricks.data {
        BrickData::Byte(v) => BrickData::Byte(zero(v, mask, n)),
        BrickData::Short(v) => BrickData::Short(zero(v, mask, n)),
        BrickData::Float(v) => BrickData::Float(zero(v, mask, n)),
    };
    Bricks {
        shape: bricks.shape,
        data,
        factors: bricks.factors.clone(),
    }
}

/// The settings of forming a mask (spec §4 and §5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaskSettings {
    /// The clip level fraction (`-clfrac`).
    pub clfrac: f32,
    /// The gradual clip level (on unless `-nograd`).
    pub gradual: bool,
    /// The peels of §5.3 (0: none).
    pub peels: usize,
    /// The peel threshold: a voxel with fewer set neighbours (of 18) is peeled; at most 18.
    pub threshold: u32,
    /// `-eclip`.
    pub exterior_clip: bool,
}

impl Default for MaskSettings {
    fn default() -> MaskSettings {
        MaskSettings {
            clfrac: 0.5,
            gradual: true,
            peels: 1,
            threshold: 17,
            exterior_clip: false,
        }
    }
}

/// The image the mask is made from (spec §3): the average of the absolute values of the
/// sub-bricks, in float.
///
/// One sub-brick: `|f * x|` with `f` its brick factor (none when 0 or 1), non-finite results
/// 0. Several: each value times its sub-brick's factor (only when some factor is neither 0
/// nor 1), the absolute values summed in float in sub-brick order, times `(float)(1.0 / n)`.
/// AFNI-01: AFNI applies only positive factors there; [`UpstreamDefects::Fix`] applies negative
/// ones too. Returns the image and whether a negative factor was met in the series case.
pub fn average_abs(bricks: &Bricks, mode: UpstreamDefects) -> (Vec<f32>, bool) {
    let [nx, ny, nz, nt] = bricks.shape;
    let n = nx * ny * nz;
    let value = |t: usize, p: usize| -> f32 {
        match &bricks.data {
            BrickData::Byte(v) => f32::from(v[t * n + p]),
            BrickData::Short(v) => f32::from(v[t * n + p]),
            BrickData::Float(v) => {
                let x = v[t * n + p];
                if x.is_finite() { x } else { 0.0 }
            }
        }
    };
    let chunk = 1 << 14;
    if nt <= 1 {
        let f = bricks.factors.first().copied().unwrap_or(0.0);
        let scale = f != 0.0 && f != 1.0;
        let mut out = vec![0f32; n];
        out.par_chunks_mut(chunk).enumerate().for_each(|(c, o)| {
            for (q, slot) in o.iter_mut().enumerate() {
                let x = value(0, c * chunk + q);
                let y = if scale { (f * x).abs() } else { x.abs() };
                *slot = if y.is_finite() { y } else { 0.0 };
            }
        });
        return (out, false);
    }
    let any_scale = bricks.factors.iter().any(|&f| f != 0.0 && f != 1.0);
    let negative = any_scale && bricks.factors.iter().any(|&f| f < 0.0);
    let factor = |t: usize| -> Option<f32> {
        let f = bricks.factors[t];
        (any_scale && (f > 0.0 || (f != 0.0 && mode.fix()))).then_some(f)
    };
    let factors: Vec<Option<f32>> = (0..nt).map(factor).collect();
    let c = (1.0 / nt as f64) as f32;
    let mut out = vec![0f32; n];
    out.par_chunks_mut(chunk).enumerate().for_each(|(ci, o)| {
        let base = ci * chunk;
        for (t, f) in factors.iter().enumerate() {
            for (q, slot) in o.iter_mut().enumerate() {
                let x = value(t, base + q);
                let y = match f {
                    Some(f) => f * x,
                    None => x,
                };
                *slot += y.abs();
            }
        }
        for slot in o.iter_mut() {
            *slot *= c;
        }
    });
    (out, negative)
}

/// The mask of `v` could not be made (spec §7: a plane smaller than 16 × 16, or nothing above
/// its clip level): AFNI's "Mask creation fails for unknown reasons!".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoMask;

fn count(mask: &[u8]) -> usize {
    mask.par_chunks(1 << 16)
        .map(|c| c.iter().filter(|&&b| b != 0).count())
        .sum()
}

fn invert(mask: &mut [u8]) {
    mask.par_chunks_mut(1 << 16)
        .for_each(|c| c.iter_mut().for_each(|b| *b ^= 1));
}

/// The largest cluster, with its two messages (spec §5.2).
fn largest(mask: &mut [u8], dims: [usize; 3], log: &mut Vec<String>) {
    log.push("Clustering voxels ...".into());
    let n = cluster::keep_largest(mask, dims);
    log.push(format!("Largest cluster has {n} voxels"));
}

/// The mask of the float image `v` on a grid of `dims` (spec §5 for `nz >= 2`, §7 for a
/// plane), and AFNI's ` + ` messages of forming it in `log` (without the label). Runs on the
/// current thread pool.
pub fn form_mask(
    v: &[f32],
    dims: [usize; 3],
    settings: &MaskSettings,
    log: &mut Vec<String>,
) -> Result<Vec<u8>, NoMask> {
    if dims[2] == 1 {
        return form_mask_2d(v, dims, settings.clfrac);
    }
    let [nx, ny, nz] = dims;
    let fixed = clip_level(v, dims, settings.clfrac);
    log.push(format!(
        "Fixed clip level = {}",
        crate::cfmt::f(f64::from(fixed))
    ));
    let mut mask = vec![0u8; v.len()];
    if settings.gradual {
        let g = GradualLevel::new(v, dims, settings.clfrac);
        let wy: Vec<f32> = (0..ny).map(|j| g.weight(1, j)).collect();
        let wx: Vec<f32> = (0..nx).map(|i| g.weight(0, i)).collect();
        let plane = nx * ny;
        let (lo, hi) = mask
            .par_chunks_mut(plane)
            .enumerate()
            .map(|(k, m)| {
                let w = g.weight(2, k);
                let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                for (j, &vj) in wy.iter().enumerate() {
                    for (i, &u) in wx.iter().enumerate() {
                        let level = g.at_weights(u, vj, w);
                        lo = lo.min(level);
                        hi = hi.max(level);
                        let p = i + nx * j;
                        m[p] = u8::from(v[k * plane + p] >= level);
                    }
                }
                (lo, hi)
            })
            .reduce(
                || (f32::INFINITY, f32::NEG_INFINITY),
                |a, b| (a.0.min(b.0), a.1.max(b.1)),
            );
        log.push(format!(
            "Used gradual clip level = {} .. {}",
            crate::cfmt::f(f64::from(lo)),
            crate::cfmt::f(f64::from(hi))
        ));
    } else {
        mask.par_iter_mut()
            .zip(v.par_iter())
            .for_each(|(m, &x)| *m = u8::from(x >= fixed));
    }
    let above = count(&mask);
    log.push(format!("Number voxels above clip level = {above}"));
    if above == 0 || nx < 2 || ny < 2 {
        return Ok(mask);
    }
    // 5.2 and 5.3: the largest cluster, peeled and restored, then the largest again.
    largest(&mut mask, dims, log);
    let stencil = Stencil::new(dims, Neighbourhood::Edges.offsets());
    let threshold = settings.threshold.min(18);
    morph::peel_and_restore(&mut mask, &stencil, settings.peels, threshold);
    largest(&mut mask, dims, log);
    // 5.4: small holes, up to three passes of side 1.
    let mut filled = 0;
    for _ in 0..3 {
        let n = morph::fill_once(&mut mask, dims, 1);
        filled += n;
        if n == 0 {
            break;
        }
    }
    if filled > 0 {
        log.push(format!(
            "Filled {filled:5} voxels in small holes; now have {} voxels",
            count(&mask)
        ));
    }
    // 5.5: large holes (AFNI-44: run when the z term alone says so, kept as AFNI's).
    let r = |n: usize| (0.016 * n as f64).round_ties_even() as usize;
    let m = 1.max(r(nx)).max(r(ny)).max(r(nz));
    if m > 1 || r(nz) > 0 {
        let mut filled = 0;
        for side in 2..m {
            filled += morph::fill_once(&mut mask, dims, side);
        }
        filled += morph::fill_until_done(&mut mask, dims, m);
        if filled > 0 {
            log.push(format!(
                "Filled {filled:5} voxels in large holes; now have {} voxels",
                count(&mask)
            ));
        }
    }
    // 5.6: the final peel, the outside, the exterior clip.
    morph::peel_and_restore(&mut mask, &stencil, 1, threshold);
    largest(&mut mask, dims, log);
    invert(&mut mask);
    log.push("Clustering non-brain voxels ...".into());
    largest(&mut mask, dims, log);
    let mut clipped = 0;
    if settings.exterior_clip {
        clipped = morph::exterior_clip(&mut mask, v, dims, fixed);
        if clipped > 0 {
            log.push(format!(
                "Removed  {clipped} exterior voxels below clip level"
            ));
        }
    }
    invert(&mut mask);
    // The count comes before the exterior clip's last peel and cluster (observed).
    log.push(format!("Mask now has {} voxels", count(&mask)));
    if clipped > 0 {
        morph::peel_and_restore(&mut mask, &stencil, 1, threshold);
        largest(&mut mask, dims, log);
    }
    Ok(mask)
}

/// The mask of a plane (spec §7).
fn form_mask_2d(v: &[f32], dims: [usize; 3], clfrac: f32) -> Result<Vec<u8>, NoMask> {
    let [nx, ny, _] = dims;
    if nx < 16 || ny < 16 {
        return Err(NoMask);
    }
    let w: Vec<f32> = v
        .iter()
        .map(|&x| if x.is_finite() { x.abs() } else { 0.0 })
        .collect();
    let level = clip_level(&w, dims, clfrac);
    let mut mask: Vec<u8> = w.iter().map(|&x| u8::from(x >= level)).collect();
    let n = count(&mask);
    if n == 0 {
        return Err(NoMask);
    }
    if n <= 2 || n == mask.len() {
        return Ok(mask);
    }
    let stencil = Stencil::new(dims, morph::plane_offsets());
    morph::peel_and_restore(&mut mask, &stencil, 3, 6);
    let keep = |mask: &mut [u8], f: f32| {
        let big = cluster::largest_size(mask, dims);
        let t = (f * big as f32) as usize;
        let t = if t == 0 || t > big { big } else { t };
        cluster::keep_at_least(mask, dims, t);
    };
    keep(&mut mask, 0.5);
    invert(&mut mask);
    keep(&mut mask, 0.9);
    invert(&mut mask);
    Ok(mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_of_one_and_several_bricks() {
        // One sub-brick: |f x|, the factor applied even when negative.
        let b = Bricks::new([2, 1, 1, 1], BrickData::Short(vec![3, -4]), vec![-0.5]).unwrap();
        assert_eq!(average_abs(&b, UpstreamDefects::Reproduce).0, [1.5, 2.0]);
        // Several: the sum of |values| times (float)(1/n); AFNI drops a negative factor.
        let b = Bricks::new(
            [1, 1, 1, 3],
            BrickData::Short(vec![2, -4, 6]),
            vec![-2.0, -2.0, -2.0],
        )
        .unwrap();
        let c = (1.0f64 / 3.0) as f32;
        let (reproduced, negative) = average_abs(&b, UpstreamDefects::Reproduce);
        assert!(negative);
        assert_eq!(reproduced, [12.0 * c]);
        assert_eq!(average_abs(&b, UpstreamDefects::Fix).0, [24.0 * c]);
        // Factors all 0 or 1: values as stored.
        let b = Bricks::new(
            [1, 1, 1, 2],
            BrickData::Float(vec![f32::NAN, -3.0]),
            vec![1.0, 0.0],
        )
        .unwrap();
        assert_eq!(average_abs(&b, UpstreamDefects::Fix).0, [3.0 * 0.5]);
    }

    #[test]
    fn small_planes_have_no_mask() {
        let v = vec![1.0f32; 12 * 12];
        assert_eq!(form_mask_2d(&v, [12, 12, 1], 0.5), Err(NoMask));
        assert_eq!(form_mask_2d(&[0.0; 64], [64, 1, 1], 0.5), Err(NoMask));
    }
}
