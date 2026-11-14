// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The volume histograms of `utils/mrihisto.cpp` (3.1-38): `MRIhistogramRegion` (101-187) and its
//! static `mriHistogramRegion` (321-447), `MRIgetEqualizeHisto` (568-601), `MRIhistoEqualize`
//! (643-696), `MRIhistogram` (725-753), `MRIhistogramLabelRegion` (754-830), `MRIhistogramLabel`
//! (831-939), `MRIhistoSegment` (940-1091), `MRIhistogramVoxel` (1194-1230) and
//! `MRIhistogramRegionWithThreshold` (1231-1335). `FZERO`'s constant in this file is
//! `FLT_EPSILON` (the object code's `.rodata`).
//!
//! `MRIhistogramRegion` keeps a static state for an optimisation of overlapping windows that is
//! compiled out (`if (0 && overlap)`): it copies each result into a static histogram and changes
//! no result, so it is not kept here.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §21):
//! - **FS-498** (a, memory): the histograms that do not keep a voxel's bin inside the histogram
//!   (`MRIhistogram`, the region histogram's byte path, `MRIhistogramLabelRegion`) count a voxel
//!   whose bin falls outside it (a NaN voxel or bin size, a rounding past the last bin) in memory
//!   they do not own, or crash. By default the bin is kept inside, as the other paths keep it
//!   (rule A); `reproduce` counts it where FreeSurfer does when that is inside the histogram's
//!   arrays, and nowhere when not.
//! - **FS-499** (a, memory): the equalisation histograms index the histogram by intensity from
//!   `low` to `high`, past its arrays when `low` is below 0 or `high` beyond its bins: reads give
//!   0 and writes go nowhere in both modes.
//! - **FS-500** (c): `MRIgetEqualizeHisto` sets the bin values from `low` to `high` in the volume's
//!   histogram it then frees, not in its result; by default the result's (rule A).
//! - **FS-501** (a, crash): `MRIhistoSegment` smooths the region histogram of a type it does not
//!   support (`int`, `long`: no histogram), or clears a given histogram of a negative bin count (a
//!   range of 2^31 bins or more): an error in both modes.
//! - **FS-502** (a, hang): `MRIhistoSegment` never ends when the gray-matter peak search returns to
//!   a peak it has found (a `gray_hi` below -1 with no peak): an error in both modes.
//! - **FS-503** (b): `MRIhistogramRegionWithThreshold` with several threads sums per-thread
//!   histograms whose fractional counts then depend on the thread count; one thread's order here.

use std::collections::HashSet;
use std::io::Write;

use larmorx_core::defects::{FS_12, FS_498, FS_499, FS_500, FS_501, FS_502};
use larmorx_freesurfer_core::cstd;
use larmorx_freesurfer_core::diag::{self, DIAG_SHOW, DIAG_WRITE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::{
    Histogram, histo_add_into_second, histo_alloc, histo_add_sample, histo_clear_in_place,
    histo_find_last_peak_in_region, histo_find_valley, histo_init, histo_plot, histo_realloc,
    histo_smooth, histo_soap_bubble_zeros_in_place,
};
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, add64, div, mul, sub, sub64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri, MriRegion};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::{Pair, mri_clone, mri_label_val_range, mri_val_range, mri_val_range_region};
use crate::labels::int_of;
use crate::rows::{self, Num};

/// `MRI_NOT_WHITE`, `MRI_AMBIGUOUS`, `MRI_WHITE` (`include/mri.h`).
pub const MRI_NOT_WHITE: i64 = 1;
pub const MRI_AMBIGUOUS: i64 = 128;
pub const MRI_WHITE: i64 = 255;

/// `MRIhistoSegment`'s constants: `PEAK_SEPARATION`, `MIN_VOXELS_PCT`, `VALLEY_WIDTH`, `GRAY_LOW`,
/// `SMOOTH_SIGMA`.
const PEAK_SEPARATION: i32 = 11;
const MIN_VOXELS_PCT: f32 = 0.15;
const VALLEY_WIDTH: i32 = 3;
const GRAY_LOW: i32 = 65;
const SMOOTH_SIGMA: f32 = 4.0;

/// `(double)` of a `float` as `cvtss2sd` makes it (a signalling NaN quieted).
fn widen(x: f32) -> f64 {
    if x.is_nan() {
        f64::from(f32::from_bits(x.to_bits() | 0x0040_0000))
    } else {
        f64::from(x)
    }
}

/// A `double` stored in a `float` (`cvtsd2ss`): rounded to nearest; a NaN keeps its sign and the
/// top 22 bits of its payload, quieted.
fn narrow(x: f64) -> f32 {
    if x.is_nan() {
        let b = x.to_bits();
        f32::from_bits(
            (((b >> 32) as u32) & 0x8000_0000) | 0x7fc0_0000 | (((b >> 29) as u32) & 0x003f_ffff),
        )
    } else {
        x as f32
    }
}

/// `FEQUAL(a, b)` of two `float`s with this file's `FLT_EPSILON`.
fn fequal(a: f32, b: f32) -> bool {
    sub(a, b).abs() < f32::EPSILON
}

/// `nint(fmax - fmin + 1.0)`: the difference in `float`, the sum in `double`. A count beyond the
/// `int` range (or NaN) is FreeSurfer's -2147483648 in both modes (FS-12 noted): a histogram that
/// cannot be allocated, where a saturated count would ask for 2^31 bins.
fn span_bins(g: &mut Globals, fmin: f32, fmax: f32) -> i32 {
    let v = add64(widen(sub(fmax, fmin)), 1.0);
    if v.is_nan() || v >= 2147483647.5 {
        g.notices.add(
            &FS_12,
            format!("nint of {v}, outside the int range: a histogram of -2147483648 bins"),
        );
        return i32::MIN;
    }
    nint(g, v)
}

/// `HISTOalloc(nbins)`; a negative count's `calloc` fails with `ENOMEM`, which its error message
/// prints first.
fn alloc(g: &mut Globals, err: &mut dyn Write, nbins: i32) -> Result<Histogram, Fatal> {
    if nbins < 0 {
        g.errno = 12;
    }
    histo_alloc(g, err, nbins)
}

/// The histograms' common preparation: a new histogram of `nbins`, or the given one reallocated
/// when it has fewer bins in use (`HISTOrealloc`), else its `nbins` set.
fn prepare<'h>(
    g: &mut Globals,
    err: &mut dyn Write,
    histo: Option<&'h mut Histogram>,
    nbins: i32,
    fresh: &'h mut Option<Histogram>,
) -> Result<&'h mut Histogram, Fatal> {
    Ok(match histo {
        None => fresh.insert(alloc(g, err, nbins)?),
        Some(h) => {
            if h.nbins < nbins {
                histo_realloc(g, err, Some(&mut *h), nbins)?;
            } else {
                h.nbins = nbins;
            }
            h
        }
    })
}

fn ret_of(fresh: Option<Histogram>) -> Ret<Histogram> {
    match fresh {
        Some(h) => Ret::New(h),
        None => Ret::Given,
    }
}

/// `histo->counts[bin]++` where FreeSurfer keeps the bin inside only from above (`clamp_high`) or
/// not at all (FS-498).
fn count(g: &mut Globals, h: &mut Histogram, bin: i32, what: &str) {
    let mut b = bin;
    if b < 0 || b >= h.nbins {
        g.notices.add(
            &FS_498,
            format!(
                "{what}: a voxel's bin {bin} outside the histogram's {} bins",
                h.nbins
            ),
        );
        if !g.notices.reproduce() {
            b = b.clamp(0, (h.nbins - 1).max(0));
        }
    }
    match usize::try_from(b).ok().filter(|&i| i < h.counts.len()) {
        Some(i) => h.counts[i] += 1.0,
        None => g.notices.add(
            &FS_498,
            format!("{what}: a voxel counted outside the histogram's arrays (dropped)"),
        ),
    }
}

/// `counts[i]` of the equalisation histograms (FS-499: 0 outside the arrays).
fn rd(g: &mut Globals, v: &[f32], i: i32, what: &str) -> f32 {
    match usize::try_from(i).ok().and_then(|i| v.get(i)) {
        Some(&x) => x,
        None => {
            g.notices.add(
                &FS_499,
                format!("{what}: bin {i} read outside the histogram's arrays (0 used)"),
            );
            0.0
        }
    }
}

/// `v[i] = x` of the equalisation histograms (FS-499: dropped outside the arrays).
fn wr(g: &mut Globals, v: &mut [f32], i: i32, x: f32, what: &str) {
    match usize::try_from(i).ok().and_then(|i| v.get_mut(i)) {
        Some(p) => *p = x,
        None => g.notices.add(
            &FS_499,
            format!("{what}: bin {i} written outside the histogram's arrays (dropped)"),
        ),
    }
}

/// A region clipped to the volume: `[x0, x1) x [y0, y1) x [z0, z1)`.
fn clip(mri: &Mri, r: &MriRegion) -> (i32, i32, i32, i32, i32, i32) {
    (
        r.x.max(0),
        r.x.wrapping_add(r.dx).min(mri.vg.width),
        r.y.max(0),
        r.y.wrapping_add(r.dy).min(mri.vg.height),
        r.z.max(0),
        r.z.wrapping_add(r.dz).min(mri.vg.depth),
    )
}

/// `MRIhistogramRegion(mri, nbins, histo, region)`: frame 0's histogram over the region clipped to
/// the volume. With `nbins` of 0 or below: `MRIhistogramRegion: input mri <fname> is blank`
/// (`ERROR_UNSUPPORTED`) and `NULL` when the region's range is within `FLT_EPSILON`, else
/// `nint(max - min + 1)` bins (255 when that is not above 0). The histogram (a new one, or the
/// given one reallocated when it has fewer bins in use, else its bin count set) is filled as the
/// region histogram ([`region_histogram`]) fills it; its result is not checked (a volume type it
/// does not support leaves the histogram's bins set and its counts 0, after its message).
pub fn mri_histogram_region(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    nbins: i32,
    histo: Option<&mut Histogram>,
    region: &MriRegion,
) -> Result<Ret<Histogram>, Fatal> {
    let (_, fmin, fmax) = mri_val_range_region(g, err, mri, region)?;
    let mut nbins = nbins;
    if nbins <= 0 && fequal(fmin, fmax) {
        let mut msg = b"MRIhistogramRegion: input mri ".to_vec();
        msg.extend_from_slice(mri.vg.fname.c_bytes());
        msg.extend_from_slice(b" is blank");
        log::error_printf_bytes(g, err, ERROR_UNSUPPORTED, &msg);
        return Ok(Ret::Null);
    }
    if nbins <= 0 {
        nbins = span_bins(g, fmin, fmax);
    }
    if nbins <= 0 {
        nbins = 255;
    }
    let mut fresh = None;
    let h = prepare(g, err, histo, nbins, &mut fresh)?;
    histo_clear_in_place(h);
    h.bin_size = div(add(sub(fmax, fmin), 1.0), nbins as f32);
    region_histogram(g, err, mri, nbins, Some(&mut *h), region)?;
    Ok(ret_of(fresh))
}

/// FreeSurfer's static `mriHistogramRegion(mri, nbins, histo, region)`: frame 0's histogram over
/// the region clipped to the volume, of `nint(max - min + 1)` bins when `nbins` is 0, between the
/// region's minimum and maximum (`HISTOinit`), bin `b`'s value `b bin_size + min` (in `float`);
/// each voxel's bin `nint((v - min) / bin_size)` (in `float`), kept below the bin count, and above
/// 0 but for bytes (FS-498). Volume types other than bytes, shorts, unsigned shorts and floats:
/// `mriHistogramRegion: unsupported mri type <type>` (`ERROR_UNSUPPORTED`) and `NULL` (a new
/// histogram is lost; a given one keeps its bins and zero counts). A given histogram with a
/// negative bin count crashes FreeSurfer (FS-501).
pub fn region_histogram(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    nbins: i32,
    histo: Option<&mut Histogram>,
    region: &MriRegion,
) -> Result<Ret<Histogram>, Fatal> {
    const WHAT: &str = "mriHistogramRegion";
    let (_, fmin, fmax) = mri_val_range_region(g, err, mri, region)?;
    let mut nbins = nbins;
    if nbins == 0 {
        nbins = span_bins(g, fmin, fmax);
    }
    let given = histo.is_some();
    let mut fresh = None;
    let h = prepare(g, err, histo, nbins, &mut fresh)?;
    if given && h.nbins < 0 {
        g.notices.add(
            &FS_501,
            "a region histogram of a negative bin count cleared (FreeSurfer crashes)",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "{WHAT}: a histogram of {} bins cannot be cleared (FreeSurfer crashes here)\n",
                h.nbins
            ),
        )?;
        return Ok(Ret::Null);
    }
    histo_clear_in_place(h);
    histo_init(g, err, Some(&mut *h), nbins, f64::from(fmin), f64::from(fmax))?;
    let (x0, x1, y0, y1, z0, z1) = clip(mri, region);
    for b in 0..nbins.max(0) {
        let v = add(mul(b as f32, h.bin_size), fmin);
        if let Some(p) = h.bins.get_mut(b as usize) {
            *p = v;
        }
    }
    let ty = mri.mtype;
    if !matches!(ty, MRI_UCHAR | MRI_SHORT | MRI_USHRT | MRI_FLOAT) {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("{WHAT}: unsupported mri type {ty}"),
        );
        return Ok(Ret::Null);
    }
    for z in z0..z1 {
        for y in y0..y1 {
            rows::touch(g, WHAT, mri, i64::from(z), i64::from(y));
            for x in x0..x1 {
                let v = rows::read(g, WHAT, mri, i64::from(z), i64::from(y), i64::from(x), ty);
                let q = div(sub(v.f(), fmin), h.bin_size);
                let mut bin = nint(g, widen(q));
                if ty == MRI_UCHAR {
                    if bin >= nbins {
                        bin = nbins - 1;
                    }
                } else {
                    if bin < 0 {
                        bin = 0;
                    }
                    if bin >= h.nbins {
                        bin = h.nbins - 1;
                    }
                }
                count(g, h, bin, WHAT);
            }
        }
    }
    Ok(ret_of(fresh))
}

/// `MRIhistogram(mri, nbins)`: the histogram of every frame's voxels (`MRIgetVoxVal`), of
/// `nint(max - min + 1)` bins when `nbins` is 0, between the volume's minimum and maximum
/// (`MRIvalRange`; a maximum equal to the minimum becomes 255), each voxel's bin
/// `nint((float)(v - min) / bin_size)`, not kept inside the histogram (FS-498).
pub fn mri_histogram(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    nbins: i32,
) -> Result<Histogram, Fatal> {
    const WHAT: &str = "MRIhistogram";
    let (_, fmin, mut fmax) = mri_val_range(g, err, mri)?;
    let mut nbins = nbins;
    if nbins == 0 {
        nbins = span_bins(g, fmin, fmax);
    }
    if fmin == fmax {
        fmax = 255.0;
    }
    let mut h = alloc(g, err, nbins)?;
    histo_init(g, err, Some(&mut h), nbins, f64::from(fmin), f64::from(fmax))?;
    for frame in 0..mri.nframes {
        for z in 0..mri.vg.depth {
            for y in 0..mri.vg.height {
                for x in 0..mri.vg.width {
                    let val = widen(mri_get_vox_val(g, err, mri, x, y, z, frame, 0)?);
                    let q = div(narrow(sub64(val, f64::from(fmin))), h.bin_size);
                    let bin = nint(g, widen(q));
                    count(g, &mut h, bin, WHAT);
                }
            }
        }
    }
    Ok(h)
}

/// `MRIgetEqualizeHisto(mri, histo_eq, low, high, norm)`: the volume's histogram
/// ([`mri_histogram`] of `nint(max - min + 1)` bins), `high` lowered to its maximum (`(int)`);
/// the result (a new histogram of as many bins, or the given one) holds 0 and the bin's number
/// below `low`; from `low` to `high` the running count of the histogram's bins (`int` sums of the
/// `float` counts), as `nint(255 total / total_pix)` (in `float`) with `norm`; above `high` the
/// count at `high` plus the bin's number, and the bin's number as its value; its bin count the
/// histogram's. The histogram is indexed by intensity (FS-499 past its arrays); the values of the
/// bins from `low` to `high` are set in the volume's histogram, not the result (FS-500).
pub fn mri_get_equalize_histo(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    histo_eq: Option<&mut Histogram>,
    low: i32,
    high: i32,
    norm: i32,
) -> Result<Ret<Histogram>, Fatal> {
    const WHAT: &str = "MRIgetEqualizeHisto";
    let mut histo = mri_histogram(g, err, mri, 0)?;
    let mut fresh = None;
    let eq: &mut Histogram = match histo_eq {
        Some(h) => h,
        None => fresh.insert(alloc(g, err, histo.nbins)?),
    };
    let mut high = high;
    if high as f32 > histo.max {
        high = int_of(g, WHAT, f64::from(histo.max));
    }
    let mut total_pix: i32 = 0;
    for i in low..=high {
        let c = rd(g, &histo.counts, i, WHAT);
        total_pix = int_of(g, WHAT, f64::from(total_pix as f32 + c));
    }
    for i in 0..low {
        wr(g, &mut eq.counts, i, 0.0, WHAT);
        wr(g, &mut eq.bins, i, i as f32, WHAT);
    }
    let mut total: i32 = 0;
    for i in low..=high {
        let c = rd(g, &histo.counts, i, WHAT);
        total = int_of(g, WHAT, f64::from(total as f32 + c));
        let v = if norm != 0 {
            let q = div(mul(255.0, total as f32), total_pix as f32);
            nint(g, widen(q)) as f32
        } else {
            total as f32
        };
        wr(g, &mut eq.counts, i, v, WHAT);
        let have = usize::try_from(i).ok().and_then(|k| eq.bins.get(k)).copied();
        if have.is_some_and(|b| b != i as f32) {
            g.notices.add(
                &FS_500,
                "MRIgetEqualizeHisto: a bin's value set in the volume's histogram, not the result",
            );
        }
        if g.notices.reproduce() {
            wr(g, &mut histo.bins, i, i as f32, WHAT);
        } else {
            wr(g, &mut eq.bins, i, i as f32, WHAT);
        }
    }
    for i in high.saturating_add(1)..histo.nbins {
        let v = add(rd(g, &eq.counts, high, WHAT), i as f32);
        wr(g, &mut eq.counts, i, v, WHAT);
        wr(g, &mut eq.bins, i, i as f32, WHAT);
    }
    eq.nbins = histo.nbins;
    Ok(ret_of(fresh))
}

/// `MRIhistoEqualize(mri_src, mri_template, mri_dst, low, high)`: frame 0 of the destination (a
/// clone of the source when none is given) over the source's extent: each voxel's value `(int)`;
/// one from `low` to `high` becomes the first bin from `low` up whose count in the template's
/// cumulative histogram ([`mri_get_equalize_histo`], normalised) reaches the source's at its
/// value, any other keeps its value. The template must be another volume than the destination.
pub fn mri_histo_equalize(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    template: &Mri,
    low: i32,
    high: i32,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIhistoEqualize";
    let mut fresh = None;
    let (src, dst): (Option<&Mri>, &mut Mri) = match p {
        Pair::New(src) => (
            Some(src),
            fresh.insert(
                mri_clone(g, out, err, src, None)?
                    .into_new()
                    .expect("MRIclone without a destination"),
            ),
        ),
        Pair::Given(src, dst) => (Some(src), dst),
        Pair::Same(m) => (None, m),
    };
    let hsrc = mri_get_equalize_histo(g, err, src.unwrap_or(dst), None, low, high, 1)?
        .into_new()
        .expect("a new histogram");
    let htpl = mri_get_equalize_histo(g, err, template, None, low, high, 1)?
        .into_new()
        .expect("a new histogram");
    if g.gdiag & DIAG_WRITE != 0 && diag::verbose_on(g) {
        histo_plot(&hsrc, None, "cum_src.plt");
        histo_plot(&htpl, None, "cum_template.plt");
    }
    let (w, h, d) = {
        let s = src.unwrap_or(dst);
        (s.vg.width, s.vg.height, s.vg.depth)
    };
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let v = mri_get_vox_val(g, err, src.unwrap_or(dst), x, y, z, 0, 0)?;
                let sval = int_of(g, WHAT, f64::from(v));
                let mut dval = sval;
                if sval >= low && sval <= high {
                    let pct = rd(g, &hsrc.counts, sval, WHAT);
                    for index in low..htpl.nbins {
                        if rd(g, &htpl.counts, index, WHAT) >= pct {
                            dval = index;
                            break;
                        }
                    }
                }
                mri_set_vox_val(g, err, dst, x, y, z, 0, dval as f32, 0)?;
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIhistogramLabelRegion(mri, mri_labeled, region, label, nbins)`: frame 0's histogram, over
/// the region clipped to the volume, of the voxels whose label (`nint` of the labelled volume's
/// frame 0) is `label`; `nint(max - min + 1)` bins when `nbins` is not above 0, between the
/// region's minimum and maximum. A constant region: bin size 1 and the histogram returned after
/// `MRIhistogramLabelRegion: constant image` (`ERROR_BADPARM`). Each voxel's bin
/// `nint((v - (int)min) / bin_size)` for the integer types (the difference in `int`), `nint((v -
/// min) / bin_size)` for floats, not kept inside the histogram (FS-498); another type, at the first
/// voxel with the label: `MRIhistogramLabelRegion: unsupported type <type>` (`ERROR_UNSUPPORTED`)
/// and `NULL` (`None`).
pub fn mri_histogram_label_region(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    labeled: &Mri,
    region: &MriRegion,
    label: i32,
    nbins: i32,
) -> Result<Option<Histogram>, Fatal> {
    const WHAT: &str = "MRIhistogramLabelRegion";
    let (_, fmin, fmax) = mri_val_range_region(g, err, mri, region)?;
    let bmin = int_of(g, WHAT, f64::from(fmin));
    let mut nbins = nbins;
    if nbins <= 0 {
        nbins = span_bins(g, fmin, fmax);
    }
    let mut h = alloc(g, err, nbins)?;
    histo_init(g, err, Some(&mut h), nbins, f64::from(fmin), f64::from(fmax))?;
    if fequal(fmin, fmax) {
        h.bin_size = 1.0;
        log::error_printf(g, err, ERROR_BADPARM, &format!("{WHAT}: constant image"));
        return Ok(Some(h));
    }
    let (x0, x1, y0, y1, z0, z1) = clip(mri, region);
    let ty = mri.mtype;
    for z in z0..z1 {
        for y in y0..y1 {
            for x in x0..x1 {
                let lv = mri_get_vox_val(g, err, labeled, x, y, z, 0, 0)?;
                if nint(g, widen(lv)) != label {
                    continue;
                }
                let q = match ty {
                    MRI_UCHAR | MRI_SHORT | MRI_USHRT => {
                        let v = match rows::read(
                            g,
                            WHAT,
                            mri,
                            i64::from(z),
                            i64::from(y),
                            i64::from(x),
                            ty,
                        ) {
                            Num::I(v) => v as i32,
                            Num::F(_) => unreachable!("an integer type"),
                        };
                        div(v.wrapping_sub(bmin) as f32, h.bin_size)
                    }
                    MRI_FLOAT => {
                        let v = rows::read(
                            g,
                            WHAT,
                            mri,
                            i64::from(z),
                            i64::from(y),
                            i64::from(x),
                            ty,
                        );
                        div(sub(v.f(), fmin), h.bin_size)
                    }
                    _ => {
                        log::error_printf(
                            g,
                            err,
                            ERROR_UNSUPPORTED,
                            &format!("{WHAT}: unsupported type {ty}"),
                        );
                        return Ok(None);
                    }
                };
                let bin = nint(g, widen(q));
                count(g, &mut h, bin, WHAT);
            }
        }
    }
    Ok(Some(h))
}

/// `MRIhistogramLabel(mri, mri_labeled, label, nbins)`: frame 0's histogram of the voxels whose
/// labelled value (frame 0, as a `float`) equals `label`, over the volume's extent; between the
/// range of the labelled voxels (`MRIlabelValRange`, every frame, `nint` labels), of `nint(max -
/// min + 1)` bins when `nbins` is not above 0 (no labelled voxel: a negative count, the program's
/// end); each bin `nint((v - (int)min) / bin_size)` for bytes, shorts and unsigned shorts (the
/// difference in `int`), `nint((v - min) / bin_size)` for the other types (`MRIgetVoxVal`), kept
/// inside the histogram.
pub fn mri_histogram_label(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    labeled: &Mri,
    label: i32,
    nbins: i32,
) -> Result<Histogram, Fatal> {
    const WHAT: &str = "MRIhistogramLabel";
    let (_, fmin, fmax) = mri_label_val_range(g, err, mri, labeled, label)?;
    let bmin = int_of(g, WHAT, f64::from(fmin));
    let mut nbins = nbins;
    if nbins <= 0 {
        nbins = span_bins(g, fmin, fmax);
    }
    let mut h = alloc(g, err, nbins)?;
    histo_init(g, err, Some(&mut h), nbins, f64::from(fmin), f64::from(fmax))?;
    let ty = mri.mtype;
    let lab = label as f32;
    for z in 0..mri.vg.depth {
        for y in 0..mri.vg.height {
            for x in 0..mri.vg.width {
                if mri_get_vox_val(g, err, labeled, x, y, z, 0, 0)? != lab {
                    continue;
                }
                let q = match ty {
                    MRI_UCHAR | MRI_SHORT | MRI_USHRT => {
                        let v = match rows::read(
                            g,
                            WHAT,
                            mri,
                            i64::from(z),
                            i64::from(y),
                            i64::from(x),
                            ty,
                        ) {
                            Num::I(v) => v as i32,
                            Num::F(_) => unreachable!("an integer type"),
                        };
                        div(v.wrapping_sub(bmin) as f32, h.bin_size)
                    }
                    MRI_FLOAT => {
                        let v = rows::read(
                            g,
                            WHAT,
                            mri,
                            i64::from(z),
                            i64::from(y),
                            i64::from(x),
                            ty,
                        );
                        div(sub(v.f(), fmin), h.bin_size)
                    }
                    _ => {
                        let v = mri_get_vox_val(g, err, mri, x, y, z, 0, 0)?;
                        div(sub(v, fmin), h.bin_size)
                    }
                };
                let mut bin = nint(g, widen(q));
                if bin < 0 {
                    bin = 0;
                } else if bin >= h.nbins {
                    bin = h.nbins - 1;
                }
                count(g, &mut h, bin, WHAT);
            }
        }
    }
    Ok(h)
}

/// `MRIhistogramVoxel(mri, nbins, histo, x0, y0, z0, wsize, mri_thresh, thresh)`: the histogram of
/// the `wsize`-wide window whose corner is `(wsize - 1) / 2` before the voxel: of
/// `nint(max - min + 1)` bins of the window's range when `nbins` is not above 0 (255 when that is
/// not above 0); the region histogram ([`region_histogram`]) without a threshold volume, else
/// [`mri_histogram_region_with_threshold`] (frame 0); then its empty bins filled
/// (`HISTOsoapBubbleZeros`, 100 times).
#[allow(clippy::too_many_arguments)]
pub fn mri_histogram_voxel(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    nbins: i32,
    histo: Option<&mut Histogram>,
    x0: i32,
    y0: i32,
    z0: i32,
    wsize: i32,
    mri_thresh: Option<&Mri>,
    thresh: f32,
) -> Result<Ret<Histogram>, Fatal> {
    let whalf = wsize.wrapping_sub(1) / 2;
    let region = MriRegion {
        x: x0.wrapping_sub(whalf),
        y: y0.wrapping_sub(whalf),
        z: z0.wrapping_sub(whalf),
        dx: wsize,
        dy: wsize,
        dz: wsize,
    };
    let (_, fmin, fmax) = mri_val_range_region(g, err, mri, &region)?;
    let mut nbins = nbins;
    if nbins <= 0 {
        nbins = span_bins(g, fmin, fmax);
    }
    if nbins <= 0 {
        nbins = 255;
    }
    let mut fresh = None;
    let h = prepare(g, err, histo, nbins, &mut fresh)?;
    histo_clear_in_place(h);
    histo_init(g, err, Some(&mut *h), nbins, f64::from(fmin), f64::from(fmax))?;
    match mri_thresh {
        None => {
            region_histogram(g, err, mri, nbins, Some(&mut *h), &region)?;
        }
        Some(t) => {
            mri_histogram_region_with_threshold(
                g,
                err,
                mri,
                nbins,
                Some(&mut *h),
                &region,
                t,
                thresh,
                0,
            )?;
        }
    }
    histo_soap_bubble_zeros_in_place(g, h, 100);
    Ok(ret_of(fresh))
}

/// `MRIhistogramRegionWithThreshold(mri, nbins, histo, region, mri_thresh, thresh, frame)`: over
/// the region clipped to the volume, the voxels of `frame` whose threshold value is not below
/// `thresh`: their range (from 1e10 and -1e10), `nint(max - min + 1)` bins when `nbins` is 0, and
/// each added with `HISTOaddSample` between that range; the histogram (new, or the given one
/// reallocated or its bin count set) set over the range (`HISTOinit`) with the samples' counts
/// (`HISTOadd` of the per-thread histograms: one thread's here, FS-503).
#[allow(clippy::too_many_arguments)]
pub fn mri_histogram_region_with_threshold(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    nbins: i32,
    histo: Option<&mut Histogram>,
    region: &MriRegion,
    mri_thresh: &Mri,
    thresh: f32,
    frame: i32,
) -> Result<Ret<Histogram>, Fatal> {
    let (x0, x1, y0, y1, z0, z1) = clip(mri, region);
    let mut fmin: f32 = 1e10;
    let mut fmax: f32 = -fmin;
    for z in z0..z1 {
        for y in y0..y1 {
            for x in x0..x1 {
                if mri_get_vox_val(g, err, mri_thresh, x, y, z, frame, 0)? < thresh {
                    continue;
                }
                let val = mri_get_vox_val(g, err, mri, x, y, z, frame, 0)?;
                if val < fmin {
                    fmin = val;
                }
                if val > fmax {
                    fmax = val;
                }
            }
        }
    }
    let mut nbins = nbins;
    if nbins == 0 {
        nbins = span_bins(g, fmin, fmax);
    }
    let mut fresh = None;
    let h = prepare(g, err, histo, nbins, &mut fresh)?;
    // the per-thread histograms: allocated and set over the range, the first one filled
    let mut t0 = alloc(g, err, nbins)?;
    histo_clear_in_place(&mut t0);
    histo_init(g, err, Some(&mut t0), nbins, f64::from(fmin), f64::from(fmax))?;
    histo_clear_in_place(h);
    histo_init(g, err, Some(&mut *h), nbins, f64::from(fmin), f64::from(fmax))?;
    for z in z0..z1 {
        for y in y0..y1 {
            for x in x0..x1 {
                if mri_get_vox_val(g, err, mri_thresh, x, y, z, frame, 0)? < thresh {
                    continue;
                }
                let val = mri_get_vox_val(g, err, mri, x, y, z, frame, 0)?;
                histo_add_sample(g, &mut t0, val, fmin, fmax);
            }
        }
    }
    // the other threads' histograms are zeros: adding them changes no count
    histo_add_into_second(g, &t0, h);
    Ok(ret_of(fresh))
}

/// `MRIhistoSegment(mri_src, mri_labeled, wm_low, wm_hi, gray_hi, wsize, sigma)`: the labelled
/// volume (a clone of the source when none is given) relabelled where it holds `MRI_AMBIGUOUS`
/// (128), reading both volumes' rows as bytes whatever their types: the source's region histogram
/// of the `wsize`-wide window starting `wsize / 2` before the voxel ([`region_histogram`], bin
/// count from the window's range), smoothed (`HISTOsmooth`, `sigma`, 4 when negative); its last
/// white-matter peak between `wm_low` and `wm_hi - 7` and the last gray-matter peak between 72 and
/// 10 below it (`HISTOfindLastPeakInRegion`, 11 bins, 15 %), as intensities (`(int)` of the bins'
/// values), the white-matter peak moved to the gray-matter one while that is above `gray_hi`; the
/// valley between them (`HISTOfindValley`, 3 bins) as an intensity, -1 when either peak is
/// missing or it is above `gray_hi`; the voxel stays ambiguous when there is no valley, its value
/// is within 2 of it, or the valley is at or above `gray_hi`, else becomes `MRI_WHITE` (255) at or
/// above the valley and `MRI_NOT_WHITE` (1) below. With `DIAG_SHOW` in `Gdiag`, the counts of
/// voxels processed and labelled on standard error. FS-501, FS-502.
#[allow(clippy::too_many_arguments)]
pub fn mri_histo_segment(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    wm_low: i32,
    wm_hi: i32,
    gray_hi: i32,
    wsize: i32,
    sigma: f32,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIhistoSegment";
    let sigma = if sigma < 0.0 { SMOOTH_SIGMA } else { sigma };
    let mut fresh = None;
    let (src, lab): (Option<&Mri>, &mut Mri) = match p {
        Pair::New(src) => (
            Some(src),
            fresh.insert(
                mri_clone(g, out, err, src, None)?
                    .into_new()
                    .expect("MRIclone without a destination"),
            ),
        ),
        Pair::Given(src, dst) => (Some(src), dst),
        Pair::Same(m) => (None, m),
    };
    let (width, height, depth) = {
        let s = src.unwrap_or(lab);
        (s.vg.width, s.vg.height, s.vg.depth)
    };
    let whalf = wsize / 2;
    let mut region = MriRegion {
        dx: wsize,
        dy: wsize,
        dz: wsize,
        ..MriRegion::default()
    };
    let mut histo: Option<Histogram> = None;
    let mut hsmooth: Option<Histogram> = None;
    let (mut nvox, mut nlabeled) = (0i32, 0i32);
    for z in 0..depth {
        diag::diag_show_pct_done(g, err, z as f32 / (depth - 1) as f32, 5);
        for y in 0..height {
            let (zl, yl) = (i64::from(z), i64::from(y));
            rows::touch(g, WHAT, lab, zl, yl);
            rows::touch(g, WHAT, src.unwrap_or(lab), zl, yl);
            for x in 0..width {
                let xl = i64::from(x);
                let label = byte(rows::read(g, WHAT, lab, zl, yl, xl, MRI_UCHAR));
                let in_val = byte(rows::read(g, WHAT, src.unwrap_or(lab), zl, yl, xl, MRI_UCHAR));
                if label != MRI_AMBIGUOUS {
                    continue;
                }
                nvox += 1;
                region.x = x.wrapping_sub(whalf);
                region.y = y.wrapping_sub(whalf);
                region.z = z.wrapping_sub(whalf);
                let s = src.unwrap_or(lab);
                match region_histogram(g, err, s, 0, histo.as_mut(), &region)? {
                    Ret::New(h) => histo = Some(h),
                    Ret::Given => {}
                    Ret::Null => histo = None,
                }
                let Some(hh) = histo.as_ref() else {
                    g.notices.add(
                        &FS_501,
                        "a missing region histogram smoothed (FreeSurfer crashes)",
                    );
                    log::error_exit(
                        g,
                        err,
                        ERROR_UNSUPPORTED,
                        &format!("{WHAT}: no region histogram to smooth (FreeSurfer crashes here)\n"),
                    )?;
                    return Ok(Ret::Null);
                };
                if let Ret::New(h) = histo_smooth(g, hh, hsmooth.as_mut(), sigma) {
                    hsmooth = Some(h);
                }
                let hs = hsmooth.as_ref().expect("a smoothed histogram");
                let peak_value = |g: &mut Globals, b: i32| -> i32 {
                    let v = usize::try_from(b)
                        .ok()
                        .and_then(|i| hs.bins.get(i))
                        .copied()
                        .unwrap_or(0.0);
                    int_of(g, WHAT, f64::from(v))
                };
                let mut wm_peak = histo_find_last_peak_in_region(
                    hs,
                    PEAK_SEPARATION,
                    MIN_VOXELS_PCT,
                    wm_low,
                    wm_hi.wrapping_sub(PEAK_SEPARATION / 2 + 2),
                );
                if wm_peak >= 0 {
                    wm_peak = peak_value(g, wm_peak);
                }
                let gray_lo = GRAY_LOW + PEAK_SEPARATION / 2 + 2;
                let mut gray_peak = histo_find_last_peak_in_region(
                    hs,
                    PEAK_SEPARATION,
                    MIN_VOXELS_PCT,
                    gray_lo,
                    wm_peak.wrapping_sub(PEAK_SEPARATION - 1),
                );
                if gray_peak >= 0 {
                    gray_peak = peak_value(g, gray_peak);
                }
                if gray_peak >= 0 && wm_peak >= 0 {
                    let mut seen = HashSet::new();
                    while gray_peak > gray_hi {
                        // white matter is bimodal
                        wm_peak = gray_peak;
                        if !seen.insert(wm_peak) {
                            g.notices.add(
                                &FS_502,
                                "the gray-matter peak search came back to a peak it had found (FreeSurfer never ends)",
                            );
                            log::error_exit(
                                g,
                                err,
                                ERROR_BADPARM,
                                &format!(
                                    "{WHAT}: the gray-matter peak {gray_peak} stays above {gray_hi} (FreeSurfer never ends here)\n"
                                ),
                            )?;
                            return Ok(Ret::Null);
                        }
                        gray_peak = histo_find_last_peak_in_region(
                            hs,
                            PEAK_SEPARATION,
                            MIN_VOXELS_PCT,
                            gray_lo,
                            wm_peak.wrapping_sub(PEAK_SEPARATION - 1),
                        );
                        if gray_peak >= 0 {
                            gray_peak = peak_value(g, gray_peak);
                        }
                    }
                }
                let mut valley = if wm_peak < 0 || gray_peak < 0 {
                    -1
                } else {
                    histo_find_valley(
                        g,
                        hs,
                        VALLEY_WIDTH,
                        gray_peak.wrapping_add(VALLEY_WIDTH - 1),
                        wm_peak.wrapping_sub(VALLEY_WIDTH - 1),
                    )
                };
                if valley >= 0 {
                    valley = peak_value(g, valley);
                }
                if valley > gray_hi {
                    valley = -1;
                }
                let iv = in_val as i32;
                let new = if valley < 0 || (iv - valley).abs() <= 2 || valley >= gray_hi {
                    MRI_AMBIGUOUS
                } else if iv >= valley {
                    nlabeled += 1;
                    MRI_WHITE
                } else {
                    nlabeled += 1;
                    MRI_NOT_WHITE
                };
                rows::write(g, WHAT, lab, zl, yl, xl, MRI_UCHAR, Num::I(new));
            }
        }
    }
    if g.gdiag & DIAG_SHOW != 0 {
        let n = width.wrapping_mul(height).wrapping_mul(depth) as f32;
        for (k, what) in [(nvox, "processed"), (nlabeled, "labeled")] {
            let pct = div(mul(100.0, k as f32), n);
            let _ = writeln!(
                err,
                "              {k:8} voxels {what} ({}%)",
                cstd::fmt_f(f64::from(pct), 2, 2)
            );
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// A byte read through a `BUFTYPE` pointer.
fn byte(v: Num) -> i64 {
    match v {
        Num::I(v) => v,
        Num::F(_) => unreachable!("a byte"),
    }
}
