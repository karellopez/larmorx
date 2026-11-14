// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrifilter.cpp`'s white-matter border classification (queue item 3.1-42):
//! `MRImarkBorderVoxels` (4515), `MRIborderClassifyVoxel` (4687), `MRIreclassify` (4824) and
//! `MRIcomputeClassStatistics` (5395), which `mri_segment` and `mris_make_surfaces` use.
//!
//! The labels are `MRI_AMBIGUOUS` 128, `MRI_WHITE` 255 and `MRI_NOT_WHITE` 1. The classifier's
//! likelihoods use `expf` (glibc's with `reproduce`, GLIBC-07). FS-557 to FS-560.

use std::io::Write;

use larmorx_core::defects::{FS_549, FS_557, FS_559, FS_560};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE, diag_show_pct_done};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::{
    histo_alloc, histo_find_highest_peak_in_region, histo_find_median, histo_init,
};
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt64, sub, sub64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MAX_INDEX, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, mri_clone, mri_val_range};

use crate::mem;

/// `MRI_AMBIGUOUS`, `MRI_WHITE`, `MRI_NOT_WHITE`.
pub const MRI_AMBIGUOUS: i32 = 128;
pub const MRI_WHITE: i32 = 255;
pub const MRI_NOT_WHITE: i32 = 1;

/// C's `(int)` of a voxel value (truncated); FS-557 beyond the `int` range (`reproduce`
/// -2147483648, by default the nearest `int`, 0 for NaN).
fn c_int(g: &mut Globals, what: &str, v: f32) -> i32 {
    if (-2147483648.0..2147483648.0).contains(&v) {
        return v as i32;
    }
    g.notices.add(
        &FS_557,
        format!("{what}: the value {v} converted to an int"),
    );
    if g.notices.reproduce() {
        i32::MIN
    } else if v.is_nan() {
        0
    } else if v > 0.0 {
        i32::MAX
    } else {
        i32::MIN
    }
}

/// `v * v` of an `int` in 32 bits as a `float` (FS-557: wrapping beyond 46,340 with `reproduce`).
fn square(g: &mut Globals, v: i32) -> f32 {
    match v.checked_mul(v) {
        Some(s) => s as f32,
        None => {
            g.notices.add(
                &FS_557,
                format!("MRIborderClassifyVoxel: the square of {v} beyond 32 bits"),
            );
            if g.notices.reproduce() {
                v.wrapping_mul(v) as f32
            } else {
                (i64::from(v) * i64::from(v)) as f32
            }
        }
    }
}

/// The boundary index table's entry `j` of an axis of `n` voxels (FS-549 beyond the table).
fn table(g: &mut Globals, j: i32, n: i32) -> i32 {
    if j < -MAX_INDEX || j >= n.saturating_add(MAX_INDEX) {
        g.notices.add(
            &FS_549,
            format!("a neighbourhood index {j} past the boundary index tables of an axis of {n}"),
        );
    }
    if j <= 0 {
        0
    } else if j >= n {
        n - 1
    } else {
        j
    }
}

/// `MRImarkBorderVoxels(mri_src, mri_dst)`: each voxel `MRI_WHITE` where the source is white with a
/// `MRI_NOT_WHITE` voxel in its 3 x 3 x 3 neighbourhood (the nearest voxel past an edge),
/// `MRI_NOT_WHITE` where it is not white with a white one, `MRI_AMBIGUOUS` elsewhere. A uchar
/// source is read and the destination written through their row pointers; another type by
/// `MRIgetVoxVal` (the voxel rounded by `nint`, its neighbours truncated) and `MRIsetVoxVal`.
/// Without a destination, `MRIclone` of the source. With `Gdiag & DIAG_SHOW`, `border white:` and
/// `border gray` counts on standard error. FS-557, FS-559.
pub fn mri_mark_border_voxels(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut fresh = None;
    let dm: &mut Mri = match dst {
        Some(m) => m,
        None => fresh.insert(mri_clone(g, out, err, src, None)?.unwrap_new()),
    };
    let uchar = src.mtype == MRI_UCHAR;
    if uchar && !mem::fits(dm, MRI_UCHAR, 0, w, h, d) {
        let detail = format!(
            "MRImarkBorderVoxels: a destination of type {} and {}x{}x{} for a uchar {w}x{h}x{d} source",
            dm.mtype, dm.vg.width, dm.vg.height, dm.vg.depth
        );
        g.notices.add(&FS_559, detail.clone());
        if !g.notices.reproduce() {
            log::error_printf(g, err, ERROR_BADPARM, &detail);
            return Ok(Ret::Null);
        }
    }
    let (wu, hu) = (w as usize, h as usize);
    let (mut nw, mut ng) = (0i32, 0i32);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let slabel = if uchar {
                    i32::from(
                        src.chunk.as_ref().expect("voxels")
                            [(z as usize * hu + y as usize) * wu + x as usize],
                    )
                } else {
                    let v = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
                    nint(g, f64::from(v))
                };
                let mut dlabel = MRI_AMBIGUOUS;
                let (want, mark) = match slabel {
                    MRI_WHITE => (MRI_NOT_WHITE, MRI_WHITE),
                    MRI_NOT_WHITE => (MRI_WHITE, MRI_NOT_WHITE),
                    _ => (-1, MRI_AMBIGUOUS),
                };
                if want >= 0 {
                    'search: for zk in -1..=1 {
                        let zi = table(g, z + zk, d);
                        for yk in -1..=1 {
                            let yi = table(g, y + yk, h);
                            for xk in -1..=1 {
                                let xi = table(g, x + xk, w);
                                let v = if uchar {
                                    i32::from(
                                        src.chunk.as_ref().expect("voxels")
                                            [(zi as usize * hu + yi as usize) * wu + xi as usize],
                                    )
                                } else {
                                    let f = mri_get_vox_val(g, err, src, xi, yi, zi, 0, 0)?;
                                    c_int(g, "MRImarkBorderVoxels", f)
                                };
                                if v == want {
                                    dlabel = mark;
                                    break 'search;
                                }
                            }
                        }
                    }
                }
                if dlabel == MRI_WHITE {
                    nw += 1;
                } else if dlabel == MRI_NOT_WHITE {
                    ng += 1;
                }
                if uchar {
                    mem::write(
                        g,
                        (&FS_559, &FS_559),
                        "MRImarkBorderVoxels",
                        dm,
                        i64::from(z),
                        i64::from(y),
                        i64::from(x),
                        &[dlabel as u8],
                    );
                } else {
                    mri_set_vox_val(g, err, dm, x, y, z, 0, dlabel as f32, 0)?;
                }
            }
        }
    }
    if g.gdiag & DIAG_SHOW != 0 {
        let n = w.wrapping_mul(h).wrapping_mul(d) as f32;
        let pct = |k: i32| larmorx_core::cfmt::f(f64::from(div(mul(100.0, k as f32), n)), 2);
        let _ = writeln!(err, "border white:  {:>8} voxels ({}%)", nw, pct(nw));
        let _ = writeln!(err, "border gray    {:>8} voxels ({}%)", ng, pct(ng));
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIborderClassifyVoxel(mri_src, mri_labeled, wsize, x, y, z, &pw, &pg)`: the label
/// (`MRI_NOT_WHITE` when `pg > pw`, else `MRI_WHITE`) and the two probabilities. The means and
/// variances of the voxels labelled white and not white in the `wsize` window (the centre left
/// out; `float` sums of the `int` values and their 32-bit squares; 1 and 1 for a class with none),
/// the likelihoods `expf(-(v - mean)² / (2 var))` of the centre's `int` value, both with the white
/// variance (FS-558, kept), normalised by their sum (`pg` only when the sum is not `FZERO`).
#[allow(clippy::too_many_arguments)]
pub fn mri_border_classify_voxel(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    labeled: &Mri,
    wsize: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Result<(i32, f32, f32), Fatal> {
    const WHAT: &str = "MRIborderClassifyVoxel";
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let whalf = (wsize - 1) / 2;
    let (mut wmean, mut gmean, mut wsq) = (0.0f32, 0.0f32, 0.0f32);
    let (mut nw, mut ng) = (0i32, 0i32);
    for zk in -whalf..=whalf {
        let zi = table(g, z + zk, d);
        for yk in -whalf..=whalf {
            let yi = table(g, y + yk, h);
            for xk in -whalf..=whalf {
                if zk == 0 && yk == 0 && xk == 0 {
                    continue;
                }
                let xi = table(g, x + xk, w);
                let f = mri_get_vox_val(g, err, src, xi, yi, zi, 0, 0)?;
                let val = c_int(g, WHAT, f);
                let l = mri_get_vox_val(g, err, labeled, xi, yi, zi, 0, 0)?;
                if l == MRI_NOT_WHITE as f32 {
                    ng += 1;
                    // the gray squares are computed by the source and never used (the official
                    // build drops them): only their overflow notice is FreeSurfer's behaviour
                    let _ = square(g, val);
                    gmean = add(val as f32, gmean);
                } else {
                    let l = mri_get_vox_val(g, err, labeled, xi, yi, zi, 0, 0)?;
                    if l == MRI_WHITE as f32 {
                        nw += 1;
                        let s = square(g, val);
                        wsq = add(s, wsq);
                        wmean = add(val as f32, wmean);
                    }
                }
            }
        }
    }
    let wvar;
    if nw != 0 {
        wmean = div(wmean, nw as f32);
        wvar = sub(div(wsq, nw as f32), mul(wmean, wmean));
    } else {
        wmean = 1.0;
        wvar = 1.0;
    }
    if ng != 0 {
        gmean = div(gmean, ng as f32);
    } else {
        gmean = 1.0;
    }
    let libm = Libm::new(g.mode());
    let two_var = add(wvar, wvar);
    let mut like = |g: &mut Globals, mean: f32| -> Result<f32, Fatal> {
        let f = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
        let val = c_int(g, WHAT, f);
        let mut dist = sub(val as f32, mean);
        dist = mul(dist, dist);
        Ok(libm.expf(div(-dist, two_var)))
    };
    let mut pw = like(g, wmean)?;
    let mut pg = like(g, gmean)?;
    let ptotal = add(pg, pw);
    // !FZERO(ptotal): a NaN total divides too
    if ptotal.abs().partial_cmp(&f32::EPSILON) != Some(std::cmp::Ordering::Less) {
        pg = div(pg, ptotal);
    }
    pw = div(pw, ptotal);
    note_glibc07(g, &libm, "expf");
    Ok((if pg > pw { MRI_NOT_WHITE } else { MRI_WHITE }, pw, pg))
}

/// `MRIreclassify(mri_src, mri_labeled, mri_dst, wm_low, gray_hi, wsize)`: `MRIreclassify():
/// wm_low=<g>, gray_hi=<g>, wsize=<d>` on standard output; the border of the labels
/// ([`mri_mark_border_voxels`]); each voxel whose (truncated) intensity lies strictly between
/// `wm_low` and `gray_hi` relabelled by [`mri_border_classify_voxel`] over the border labels, and,
/// when the larger probability is below 0.7, by the sums of the probabilities over the windows 5,
/// 9, 13 and 17; every voxel's label written into the destination (the labels' copy when none is
/// given, which FreeSurfer then loses: no output but the counts); the counts of voxels tested,
/// changed and searched at several scales. `NO_ERROR`. The labels as their own destination
/// (`mri_segment`'s call) are allowed.
#[allow(clippy::too_many_arguments)]
pub fn mri_reclassify(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    labeled: Pair,
    wm_low: f32,
    gray_hi: f32,
    wsize: i32,
) -> Result<i32, Fatal> {
    let gf = |v: f32| larmorx_core::cfmt::g(f64::from(v), 6);
    let _ = writeln!(
        out,
        "MRIreclassify(): wm_low={}, gray_hi={}, wsize={wsize}",
        gf(wm_low),
        gf(gray_hi)
    );
    // the new labels from the labels as they are (each voxel's label is read before its own
    // write, so the labels as their own destination give the same), then written
    let (labels, counts) = match &labeled {
        Pair::New(l) | Pair::Given(l, _) => {
            reclassify_labels(g, out, err, src, l, wm_low, gray_hi, wsize)?
        }
        Pair::Same(d) => reclassify_labels(g, out, err, src, d, wm_low, gray_hi, wsize)?,
    };
    let mut lost = None;
    let dm: &mut Mri = match labeled {
        Pair::New(l) => lost.insert(mri_copy(g, out, err, l, None)?.unwrap_new()),
        Pair::Given(_, d) | Pair::Same(d) => d,
    };
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut i = 0;
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                mri_set_vox_val(g, err, dm, x, y, z, 0, labels[i] as f32, 0)?;
                i += 1;
            }
        }
    }
    let (ntested, nchanged, nmulti) = counts;
    let n = w.wrapping_mul(h).wrapping_mul(d) as f32;
    let pct = |k: i32| larmorx_core::cfmt::f(f64::from(div(mul(100.0, k as f32), n)), 2);
    let _ = writeln!(out, "  {ntested:>8} voxels tested ({}%)", pct(ntested));
    let _ = writeln!(out, "  {nchanged:>8} voxels changed ({}%)", pct(nchanged));
    let _ = writeln!(
        out,
        "  {nmulti:>8} multi-scale searches  ({}%)",
        pct(nmulti)
    );
    Ok(NO_ERROR)
}

/// `MRIreclassify`'s labels (in the source's voxel order) and its counts (tested, changed, searched
/// at several scales).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn reclassify_labels(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    lab: &Mri,
    wm_low: f32,
    gray_hi: f32,
    wsize: i32,
) -> Result<(Vec<i32>, (i32, i32, i32)), Fatal> {
    let border = mri_mark_border_voxels(g, out, err, lab, None)?.unwrap_new();
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let (mut nmulti, mut ntested, mut nchanged) = (0i32, 0i32, 0i32);
    let mut labels = Vec::with_capacity((w.max(0) * h.max(0) * d.max(0)) as usize);
    for z in 0..d {
        diag_show_pct_done(g, err, div(z as f32, (d - 1) as f32), 5);
        for y in 0..h {
            for x in 0..w {
                let sv = mri_get_vox_val(g, err, src, x, y, z, 0, 0)?;
                let s = c_int(g, "MRIreclassify", sv);
                let lv = mri_get_vox_val(g, err, lab, x, y, z, 0, 0)?;
                let mut label = c_int(g, "MRIreclassify", lv);
                if (s as f32) > wm_low && (s as f32) < gray_hi {
                    ntested += 1;
                    let (mut new_label, pw, pg) =
                        mri_border_classify_voxel(g, err, src, &border, wsize, x, y, z)?;
                    let mx = if pw > pg { pw } else { pg };
                    if f64::from(mx) < 0.7 {
                        nmulti += 1;
                        let (mut gtotal, mut wtotal) = (0.0f32, 0.0f32);
                        let mut ws = 5;
                        while ws < 21 {
                            let (_, pw, pg) =
                                mri_border_classify_voxel(g, err, src, &border, ws, x, y, z)?;
                            gtotal = add(gtotal, pg);
                            wtotal = add(wtotal, pw);
                            ws += 4;
                        }
                        new_label = if gtotal > wtotal {
                            MRI_NOT_WHITE
                        } else {
                            MRI_WHITE
                        };
                    }
                    if new_label != label {
                        nchanged += 1;
                    }
                    label = new_label;
                }
                labels.push(label);
            }
        }
    }
    Ok((labels, (ntested, nchanged, nmulti)))
}

/// `MRIcomputeClassStatistics`'s results: the white and gray means and standard deviations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassStats {
    pub mean_wm: f32,
    pub sigma_wm: f32,
    pub mean_gm: f32,
    pub sigma_gm: f32,
}

/// `MRIcomputeClassStatistics(mri_T1, mri_labeled, gray_low, gray_hi, ...)`: over the border of the
/// labels ([`mri_mark_border_voxels`]) and the voxels not labelled `MRI_AMBIGUOUS`: the white
/// border voxels at or above `(gray_low + gray_hi) / 2` and the gray ones within `[gray_low,
/// gray_hi]` (all of them for a negative `gray_low`), their means, standard deviations, extremes and
/// histograms (bins of 1 from 0 to the volume's maximum), whose medians are printed as the modes:
/// `CCS WM (<mode>): <mean> +- <std> [<min> --> <max>]` and `CCS GM (...) : ...` on standard
/// output. The means and deviations are returned (in `double` until then). FS-560.
pub fn mri_compute_class_statistics(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t1: &Mri,
    labeled: &Mri,
    gray_low: f32,
    gray_hi: f32,
) -> Result<ClassStats, Fatal> {
    let (_, _, max_val) = mri_val_range(g, err, t1)?;
    let min_val = 0.0f32;
    let nbins = nint_ceil(g, sub(max_val, min_val)).wrapping_add(1);
    let mut hw = histo_alloc(g, err, nbins)?;
    let mut hg = histo_alloc(g, err, nbins)?;
    for bin in 0..nbins.max(0) {
        let v = add(min_val, bin as f32);
        hw.bins[bin as usize] = v;
        hg.bins[bin as usize] = v;
    }
    let ngb = hg.nbins;
    histo_init(g, err, Some(&mut hg), ngb, 0.0, 0.0)?;
    let nwb = hw.nbins;
    histo_init(g, err, Some(&mut hw), nwb, 0.0, 0.0)?;
    let border = mri_mark_border_voxels(g, out, err, labeled, None)?.unwrap_new();
    let (w, h, d) = (t1.vg.width, t1.vg.height, t1.vg.depth);
    let (mut white_mean, mut gray_mean, mut white_std, mut gray_std) =
        (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut white_min, mut gray_min) = (100000.0f64, 100000.0f64);
    let (mut white_max, mut gray_max) = (-100000.0f64, -100000.0f64);
    let (mut nwhite, mut ngray) = (0i32, 0i32);
    let mid = div(add(gray_low, gray_hi), 2.0);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let lf = mri_get_vox_val(g, err, labeled, x, y, z, 0, 0)?;
                let label = c_int(g, "MRIcomputeClassStatistics", lf);
                if label == MRI_AMBIGUOUS {
                    continue;
                }
                let val = f64::from(mri_get_vox_val(g, err, t1, x, y, z, 0, 0)?);
                let bf = mri_get_vox_val(g, err, &border, x, y, z, 0, 0)?;
                let border_label = c_int(g, "MRIcomputeClassStatistics", bf);
                let class = if border_label == MRI_WHITE {
                    (val >= f64::from(mid) || gray_low < 0.0).then_some(true)
                } else if border_label == MRI_NOT_WHITE {
                    ((val >= f64::from(gray_low) && val <= f64::from(gray_hi)) || gray_low < 0.0)
                        .then_some(false)
                } else {
                    None
                };
                let Some(white) = class else {
                    continue;
                };
                let (n, mean, std, mn, mx, hist) = if white {
                    (
                        &mut nwhite,
                        &mut white_mean,
                        &mut white_std,
                        &mut white_min,
                        &mut white_max,
                        &mut hw,
                    )
                } else {
                    (
                        &mut ngray,
                        &mut gray_mean,
                        &mut gray_std,
                        &mut gray_min,
                        &mut gray_max,
                        &mut hg,
                    )
                };
                *n += 1;
                *mean = add64(*mean, val);
                *std = add64(*std, mul64(val, val));
                if val < *mn {
                    *mn = val;
                }
                if val > *mx {
                    *mx = val;
                }
                let bin = nint(g, sub64(val, f64::from(min_val)));
                if bin < 0 || bin >= hist.nbins {
                    g.notices.add(
                        &FS_560,
                        format!(
                            "MRIcomputeClassStatistics: the value {val} counted in bin {bin} of a histogram of {}{}",
                            hist.nbins,
                            if g.notices.reproduce() {
                                " (memory outside it: left out)"
                            } else {
                                "; left out"
                            }
                        ),
                    );
                    continue;
                }
                hist.counts[bin as usize] += 1.0;
            }
        }
    }
    let _ = histo_find_highest_peak_in_region(&hw, 0, hw.nbins);
    let _ = histo_find_highest_peak_in_region(&hg, 0, hg.nbins);
    let white_mode = histo_find_median(&hw) as f32;
    let gray_mode = histo_find_median(&hg) as f32;
    gray_mean = div64(gray_mean, f64::from(ngray));
    white_mean = div64(white_mean, f64::from(nwhite));
    white_std = sqrt64(sub64(
        div64(white_std, f64::from(nwhite)),
        mul64(white_mean, white_mean),
    ));
    gray_std = sqrt64(sub64(
        div64(gray_std, f64::from(ngray)),
        mul64(gray_mean, gray_mean),
    ));
    let f1 = |v: f64| larmorx_core::cfmt::f(v, 1);
    let _ = writeln!(
        out,
        "CCS WM ({}): {} +- {} [{} --> {}]",
        f1(f64::from(white_mode)),
        f1(white_mean),
        f1(white_std),
        f1(white_min),
        f1(white_max)
    );
    let _ = writeln!(
        out,
        "CCS GM ({}) : {} +- {} [{} --> {}]",
        f1(f64::from(gray_mode)),
        f1(gray_mean),
        f1(gray_std),
        f1(gray_min),
        f1(gray_max)
    );
    if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        // FreeSurfer writes border.mgz and fits the gray histogram here (diagnostics only)
    }
    Ok(ClassStats {
        mean_wm: white_mean as f32,
        sigma_wm: white_std as f32,
        mean_gm: gray_mean as f32,
        sigma_gm: gray_std as f32,
    })
}

/// `(int)ceil(v)` of a `float` (C++'s `ceilf`), FS-557 beyond the `int` range.
fn nint_ceil(g: &mut Globals, v: f32) -> i32 {
    c_int(g, "MRIcomputeClassStatistics", v.ceil())
}
