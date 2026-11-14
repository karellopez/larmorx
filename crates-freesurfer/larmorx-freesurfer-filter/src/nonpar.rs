// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrifilter.cpp`'s non-parametric white matter and cortex classification (queue item
//! 3.1-42): `MRIreclassifyWMCtxNonPar` (4914) and `MRIclassifyAmbiguousNonPar` (5093), as
//! `mri_segment` calls them with a segmentation (`-aseg`). On fs.volume's `MRImatchSegs`,
//! `MRIsegStats`, `HISTOseg`, `MRIdilate` and fs.maths' histograms.
//!
//! The threshold is the bin where the cortex CDF's complement meets the white matter CDF (the
//! first bin of the smallest `|(1 - ctx) - wm|`; FreeSurfer stores the signed difference as its
//! running minimum, which never changes the bin found because the difference only decreases
//! along the bins). FS-572 to FS-575.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_572, FS_573, FS_574, FS_575};
use larmorx_freesurfer_core::cstd::format_double;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::{Histogram, histo_cumsum, histo_sum_norm, histo_write_txt};
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add64, div64, sub64};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc, mri_copy_header};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::Pair;
use larmorx_freesurfer_volume::mriset::mri_dilate;
use larmorx_freesurfer_volume::segs::{histo_seg, mri_match_segs, mri_seg_stats};

use crate::classify::{MRI_AMBIGUOUS, MRI_NOT_WHITE, MRI_WHITE};

/// The white matter labels (`Left/Right-Cerebral-White-Matter` and the corpus callosum's).
const WM_SEGS: [i32; 7] = [2, 41, 251, 252, 253, 254, 255];
/// The cortex labels.
const CTX_SEGS: [i32; 2] = [3, 42];
const LEFT_WM: i32 = 2;
const LEFT_CTX: i32 = 3;
const RIGHT_WM: i32 = 41;
const RIGHT_CTX: i32 = 42;

fn gv(g: &mut Globals, err: &mut dyn Write, m: &Mri, c: i32, r: i32, s: i32) -> Result<f32, Fatal> {
    mri_get_vox_val(g, err, m, c, r, s, 0, 0)
}

/// `(int) v` is not 0 (x86-64: NaN and values outside the range give `INT_MIN`).
fn nonzero(v: f32) -> bool {
    !(v > -1.0 && v < 1.0)
}

fn pg(x: f32) -> String {
    format_double("%g", f64::from(x)).unwrap_or_default()
}

fn pg64(x: f64) -> String {
    format_double("%g", x).unwrap_or_default()
}

/// The white matter and cortex masks of `seg` (1 at the labels, into `wm` and `ctx` when given).
fn masks(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    seg: &Mri,
    wm: Option<Mri>,
    ctx: Option<Mri>,
) -> Result<(Mri, Mri), Fatal> {
    let mut one = |m: Option<Mri>, list: &[i32]| -> Result<Mri, Fatal> {
        Ok(match m {
            None => mri_match_segs(g, out, err, Pair::New(seg), list, 1)?.unwrap_new(),
            Some(mut m) => {
                mri_match_segs(g, out, err, Pair::Given(seg, &mut m), list, 1)?;
                m
            }
        })
    };
    let w = one(wm, &WM_SEGS)?;
    let c = one(ctx, &CTX_SEGS)?;
    Ok((w, c))
}

/// The class statistics both functions print: `<name>stats n=<%6d>, mean=<%g>, std=<%g>,
/// min=<%g>, max=<%g>`; `None` after `ERROR: could not find any <WM|CTX> voxels`.
fn stats(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mask: &Mri,
    norm: &Mri,
    label: &str,
    tag: &str,
) -> Result<Option<(f32, f32)>, Fatal> {
    let (n, st) = mri_seg_stats(g, err, mask, 1, norm, 0)?;
    if n == 0 {
        let _ = writeln!(out, "ERROR: could not find any {tag} voxels");
        return Ok(None);
    }
    let _ = writeln!(
        out,
        "{label} n={n:6}, mean={}, std={}, min={}, max={}",
        pg(st.mean),
        pg(st.std),
        pg(st.min),
        pg(st.max)
    );
    Ok(Some((st.min, st.max)))
}

/// `MIN(a, b)` of two floats (`a < b ? a : b`).
fn min_f(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// The histogram limits (`hbinmin` the smaller minimum, `hbinmax` the smaller maximum: FS-572),
/// printed as `hbminmin = <%g>, hbinmax = <%g>`, and the two class histograms (PDFs) over them
/// with bins of 1.
#[allow(clippy::too_many_arguments)]
fn pdfs(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    wm: &Mri,
    ctx: &Mri,
    norm: &Mri,
    wmr: (f32, f32),
    ctxr: (f32, f32),
) -> Result<(Histogram, Histogram), Fatal> {
    let hbinmin = f64::from(min_f(wmr.0, ctxr.0));
    let hbinmax = f64::from(min_f(wmr.1, ctxr.1));
    if wmr.1 > ctxr.1 {
        g.notices.add(
            &FS_572,
            "the class histograms end at the cortex maximum, below the white matter's (kept: rule D)",
        );
    }
    let _ = writeln!(
        out,
        "hbminmin = {}, hbinmax = {}",
        pg64(hbinmin),
        pg64(hbinmax)
    );
    let mut hwm = histo_seg(g, err, wm, 1, norm, hbinmin, hbinmax, 1.0)?;
    histo_sum_norm(&mut hwm);
    let mut hctx = histo_seg(g, err, ctx, 1, norm, hbinmin, hbinmax, 1.0)?;
    histo_sum_norm(&mut hctx);
    Ok((hwm, hctx))
}

fn nbins(h: &Histogram) -> usize {
    (h.nbins.max(0) as usize).min(h.counts.len())
}

/// The bin of the smallest `|(1 - ctxcdf) - wmcdf|` (the first of equal ones; 0 when there is
/// none).
fn crossing(wmcdf: &Histogram, ctxcdf: &Histogram) -> usize {
    let mut min = 10e10f64;
    let mut nmin = 0;
    for n in 0..nbins(wmcdf) {
        let d = sub64(
            sub64(1.0, f64::from(ctxcdf.counts.get(n).copied().unwrap_or(0.0))),
            f64::from(wmcdf.counts[n]),
        );
        if d.abs() < min {
            // FreeSurfer keeps `d` itself; the bin found is the same (the module's documentation)
            min = d.abs();
            nmin = n;
        }
    }
    nmin
}

/// A histogram's bin value at `n`; the edge bin (FS-573) where FreeSurfer reads before or past
/// the histogram.
fn bin_at(g: &mut Globals, h: &Histogram, n: i64, what: &str) -> f64 {
    let nb = nbins(h) as i64;
    if (0..nb).contains(&n) {
        return f64::from(h.bins[n as usize]);
    }
    g.notices.add(
        &FS_573,
        format!("{what}: no bin satisfies the search; FreeSurfer reads bin {n} of {nb} (memory outside the histogram)"),
    );
    if nb == 0 {
        0.0
    } else {
        f64::from(h.bins[n.clamp(0, nb - 1) as usize])
    }
}

/// `MRIreclassifyWMCtxNonPar(norm, seg, nitersmax, newseg)`: prints `MRIreclassifyWMCtxNonPar():
/// nitersmax = <d> `; the segmentation as its own output fails (`ERROR: cannot be done
/// in-place`, `Null`). The white matter (labels 2, 41, 251-255) and cortex (3, 42) statistics of
/// `norm` (`ERROR: could not find any WM voxels` / `CTX voxels` and `Null` when a class is empty),
/// their histograms' crossing as the threshold (`thresh = <%g>`); then up to `nitersmax` passes:
/// the output becomes a copy of the current segmentation (`MRIcopy`), and every white matter or
/// cortex voxel with a face neighbour of the other class (neighbours read with `MRIgetVoxVal`,
/// FS-52 and FS-53 past the volume) is recoded: cortex (3, 42) above the threshold to white
/// matter (2, 41), white matter at or below it to cortex; `iter <%2d>/<%2d> nborder = <n>, nrecode =
/// <n>` after each pass, which ends the passes when it recoded nothing; the next pass works on
/// the output. A new output when none is given.
pub fn mri_reclassify_wm_ctx_non_par(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    norm: &Mri,
    p: Pair,
    nitersmax: i32,
) -> Result<Ret<Mri>, Fatal> {
    let _ = writeln!(out, "MRIreclassifyWMCtxNonPar(): nitersmax = {nitersmax} ");
    let (seg, given): (&Mri, Option<&mut Mri>) = match p {
        Pair::Same(_) => {
            let _ = writeln!(out, "ERROR: cannot be done in-place");
            return Ok(Ret::Null);
        }
        Pair::New(s) => (s, None),
        Pair::Given(s, d) => (s, Some(d)),
    };
    let (wm, ctx) = masks(g, out, err, seg, None, None)?;
    let Some(wmr) = stats(g, out, err, &wm, norm, "wmstats ", "WM")? else {
        return Ok(Ret::Null);
    };
    let Some(ctxr) = stats(g, out, err, &ctx, norm, "ctxstats", "CTX")? else {
        return Ok(Ret::Null);
    };
    let (mut hwm, mut hctx) = pdfs(g, out, err, &wm, &ctx, norm, wmr, ctxr)?;
    larmorx_freesurfer_core::histo::histo_cumsum_in_place(&mut hwm);
    larmorx_freesurfer_core::histo::histo_cumsum_in_place(&mut hctx);
    let nmin = crossing(&hwm, &hctx);
    let thresh = bin_at(g, &hwm, nmin as i64, "MRIreclassifyWMCtxNonPar");
    let _ = writeln!(out, "thresh = {}", pg64(thresh));

    let mut fresh: Option<Mri> = None;
    let mut given = given;
    let (mut wm, mut ctx) = (Some(wm), Some(ctx));
    for k in 0..nitersmax {
        // the output: a copy of the segmentation in the first pass (MRIcopy(seg, newseg)); later
        // passes work on it in place (MRIcopy of a volume onto itself changes nothing)
        if k == 0 {
            match given.as_deref_mut() {
                Some(d) => {
                    mri_copy(g, out, err, seg, Some(d))?;
                }
                None => fresh = Some(mri_copy(g, out, err, seg, None)?.unwrap_new()),
            }
        }
        let cur: &mut Mri = match given.as_deref_mut() {
            Some(d) => d,
            None => fresh.as_mut().expect("the output"),
        };
        let (w, c) = if k == 0 {
            masks(g, out, err, seg, wm.take(), ctx.take())?
        } else {
            masks(g, out, err, cur, wm.take(), ctx.take())?
        };
        let (mut nborder, mut nrecode) = (0, 0);
        for cc in 0..w.vg.width {
            for r in 0..w.vg.height {
                for s in 0..w.vg.depth {
                    let wmm = nonzero(gv(g, err, &w, cc, r, s)?);
                    let ctxm = nonzero(gv(g, err, &c, cc, r, s)?);
                    if !wmm && !ctxm {
                        continue;
                    }
                    let mut isborder = false;
                    'n: for dc in -1i32..=1 {
                        for dr in -1i32..=1 {
                            for ds in -1i32..=1 {
                                if dc.abs() + dr.abs() + ds.abs() != 1 {
                                    continue;
                                }
                                let (x, y, z) = (cc + dc, r + dr, s + ds);
                                let nbrwmm = nonzero(gv(g, err, &w, x, y, z)?);
                                let nbrctxm = nonzero(gv(g, err, &c, x, y, z)?);
                                if (wmm && nbrctxm) || (ctxm && nbrwmm) {
                                    isborder = true;
                                    break 'n;
                                }
                            }
                        }
                    }
                    if !isborder {
                        continue;
                    }
                    nborder += 1;
                    let val = f64::from(gv(g, err, norm, cc, r, s)?);
                    // the current label: the segmentation's in the first pass, the output's (not
                    // yet written at this voxel) later
                    let segid = if k == 0 {
                        gv(g, err, seg, cc, r, s)?
                    } else {
                        gv(g, err, cur, cc, r, s)?
                    } as i32;
                    let to = if val > thresh {
                        match segid {
                            LEFT_CTX => Some(LEFT_WM),
                            RIGHT_CTX => Some(RIGHT_WM),
                            _ => None,
                        }
                    } else {
                        match segid {
                            LEFT_WM => Some(LEFT_CTX),
                            RIGHT_WM => Some(RIGHT_CTX),
                            _ => None,
                        }
                    };
                    if let Some(v) = to {
                        mri_set_vox_val(g, err, cur, cc, r, s, 0, v as f32, 0)?;
                        nrecode += 1;
                    }
                }
            }
        }
        wm = Some(w);
        ctx = Some(c);
        let _ = write!(out, "iter {k:2}/{nitersmax:2} ");
        let _ = writeln!(out, "nborder = {nborder}, nrecode = {nrecode}");
        if nrecode == 0 {
            break;
        }
    }
    Ok(match (fresh, given) {
        (Some(m), _) => Ret::New(m),
        (None, Some(_)) => Ret::Given,
        // no pass: FreeSurfer returns the newseg it was given (NULL)
        (None, None) => Ret::Null,
    })
}

/// `MRIclassifyAmbiguousNonPar(norm, seg, statseg, Qwm, Qctx, NdilWM, NdilCtx)`: prints
/// `MRIclassifyAmbiguousNonPar(): Qwm=<%g>, Qctx=<%g>, NdilWM=<d>, NdilCtx=<d> `; the white matter and
/// cortex statistics and histograms of `norm` over `statseg`'s classes (as
/// [`mri_reclassify_wm_ctx_non_par`]), written as text to `wm.pdf.dat` and `ctx.pdf.dat` in `dir`
/// (the current directory when `None`; `HISTOwriteTxt`); the crossing `thresh`, `thresh_low` the
/// first bin at or below it (going down) where the cortex PDF is at least `Qctx` times the white
/// matter's plus 1e-6, `thresh_hi` the first bin from the bottom (FS-572) where the white matter
/// PDF is at least `Qwm` times the cortex's plus 1e-6 (FS-573 where none is), printed `thresh =
/// (<%g>,<%g>,<%g>)`. Then a new uchar volume of `seg`'s extent and header: 255 (white) where `seg`
/// is white matter, else 1, and 128 (ambiguous) where `norm` lies strictly between the two
/// thresholds and the voxel is in the white matter or cortex mask of `seg` dilated once (FS-574:
/// `NdilWM` and `NdilCtx` dilations each start from the undilated mask; FS-575: 0 or fewer ends
/// the program); `nhits = <n>, nrecode = <n>`.
#[allow(clippy::too_many_arguments)]
pub fn mri_classify_ambiguous_non_par(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    norm: &Mri,
    seg: &Mri,
    statseg: &Mri,
    qwm: f64,
    qctx: f64,
    ndil_wm: i32,
    ndil_ctx: i32,
    dir: Option<&Path>,
) -> Result<Option<Mri>, Fatal> {
    const WHAT: &str = "MRIclassifyAmbiguousNonPar";
    let _ = writeln!(
        out,
        "MRIclassifyAmbiguousNonPar(): Qwm={}, Qctx={}, NdilWM={ndil_wm}, NdilCtx={ndil_ctx} ",
        pg64(qwm),
        pg64(qctx)
    );
    let (wm, ctx) = masks(g, out, err, statseg, None, None)?;
    let Some(wmr) = stats(g, out, err, &wm, norm, "wmstats ", "WM")? else {
        return Ok(None);
    };
    let Some(ctxr) = stats(g, out, err, &ctx, norm, "ctxstats", "CTX")? else {
        return Ok(None);
    };
    let (hwm, hctx) = pdfs(g, out, err, &wm, &ctx, norm, wmr, ctxr)?;
    let wmcdf = histo_cumsum(g, &hwm, None).unwrap_new();
    let ctxcdf = histo_cumsum(g, &hctx, None).unwrap_new();
    histo_write_txt(&hwm, out, dir, "wm.pdf.dat");
    histo_write_txt(&hctx, out, dir, "ctx.pdf.dat");
    let nmin = crossing(&wmcdf, &ctxcdf);
    let thresh = bin_at(g, &hwm, nmin as i64, WHAT);
    let ratio = |a: &Histogram, b: &Histogram, n: usize| {
        div64(
            f64::from(a.counts[n]),
            add64(f64::from(b.counts.get(n).copied().unwrap_or(0.0)), 1e-6),
        )
    };
    let mut n = nmin as i64;
    while n >= 0 {
        if (n as usize) < nbins(&hctx) && ratio(&hctx, &hwm, n as usize) >= qctx {
            break;
        }
        n -= 1;
    }
    let thresh_low = bin_at(g, &hctx, n, WHAT);
    let mut n = 0i64;
    while (n as usize) < nbins(&hwm) {
        if ratio(&hwm, &hctx, n as usize) >= qwm {
            break;
        }
        n += 1;
    }
    let thresh_hi = bin_at(g, &hwm, n, WHAT);
    let _ = writeln!(
        out,
        "thresh = ({},{},{})",
        pg64(thresh),
        pg64(thresh_low),
        pg64(thresh_hi)
    );
    let (wm, ctx) = masks(g, out, err, seg, Some(wm), Some(ctx))?;
    if ndil_wm <= 0 || ndil_ctx <= 0 {
        g.notices.add(
            &FS_575,
            format!("{WHAT}: NdilWM {ndil_wm}, NdilCtx {ndil_ctx}: no dilated mask (FreeSurfer reads a NULL volume)"),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "MRIclassifyAmbiguousNonPar(): a dilation count of 0 or less (FreeSurfer crashes here)\n",
        )?;
        return Ok(None);
    }
    if ndil_wm > 1 || ndil_ctx > 1 {
        g.notices.add(
            &FS_574,
            format!("{WHAT}: {ndil_wm} and {ndil_ctx} dilations each restart from the undilated mask: one voxel (kept: rule D)"),
        );
    }
    let mut dilate = |m: &Mri, times: i32| -> Result<Mri, Fatal> {
        let mut d = mri_dilate(g, out, err, Pair::New(m))?.unwrap_new();
        for _ in 1..times {
            mri_dilate(g, out, err, Pair::Given(m, &mut d))?;
        }
        Ok(d)
    };
    let wmdil = dilate(&wm, ndil_wm)?;
    let ctxdil = dilate(&ctx, ndil_ctx)?;
    let (w, h, d) = (seg.vg.width, seg.vg.height, seg.vg.depth);
    let mut ambi = mri_alloc(g, out, err, w, h, d, MRI_UCHAR)?;
    mri_copy_header(g, out, err, seg, Some(&mut ambi))?;
    let (mut nhits, mut nrecode) = (0, 0);
    for c in 0..wm.vg.width {
        for r in 0..wm.vg.height {
            for s in 0..wm.vg.depth {
                let wmm = nonzero(gv(g, err, &wm, c, r, s)?);
                let v = if wmm { MRI_WHITE } else { MRI_NOT_WHITE };
                mri_set_vox_val(g, err, &mut ambi, c, r, s, 0, v as f32, 0)?;
                let wmm = nonzero(gv(g, err, &wmdil, c, r, s)?);
                let ctxm = nonzero(gv(g, err, &ctxdil, c, r, s)?);
                if !wmm && !ctxm {
                    continue;
                }
                nhits += 1;
                let val = f64::from(gv(g, err, norm, c, r, s)?);
                if val > thresh_low && val < thresh_hi {
                    mri_set_vox_val(g, err, &mut ambi, c, r, s, 0, MRI_AMBIGUOUS as f32, 0)?;
                    nrecode += 1;
                }
            }
        }
    }
    let _ = writeln!(out, "nhits = {nhits}, nrecode = {nrecode}");
    Ok(Some(ambi))
}
