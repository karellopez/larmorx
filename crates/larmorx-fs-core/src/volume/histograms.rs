// SPDX-License-Identifier: Apache-2.0
//! Volume histograms (spec §52): the region histogram, `MRIhistogramRegion`, `MRIhistogram`,
//! `MRIgetEqualizeHisto`, `MRIhistoEqualize`, `MRIhistogramLabelRegion`, `MRIhistogramLabel`,
//! `MRIhistogramVoxel`, `MRIhistogramRegionWithThreshold` and `MRIhistoSegment`, with FS-498 to
//! FS-503 and FS-965. The histograms themselves are fs.maths' (`specs/freesurfer-maths.md` §5,
//! §21).
// C's comparisons, NaN included: a negated comparison is FreeSurfer's test, not a style choice.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::collections::HashSet;

use larmorx_core::defects::{FS_12, FS_498, FS_499, FS_500, FS_501, FS_502, FS_965};

use super::{Dst, VolOut, Vols};
use crate::maths::histo::{DIAG_WRITE, Histogram};
use crate::maths::{Output, Returned};
use crate::mri::{Mri, Region, types};
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};
use crate::sse;

/// The diagnostic bit `DIAG_SHOW`.
const DIAG_SHOW: u64 = 0x40;
/// The label of a voxel `MRIhistoSegment` decides (`MRI_AMBIGUOUS`).
const AMBIGUOUS: u8 = 128;
/// `MRI_WHITE`.
const WHITE: u8 = 255;
/// `MRI_NOT_WHITE`.
const NOT_WHITE: u8 = 1;
/// The largest bin count taken from a range (FS-965: 2^26).
const MOST_RANGE_BINS: i32 = 67_108_864;

/// A histogram function's result: none, a new histogram, or the caller's given one.
pub type HistOut = Returned<Histogram>;

/// `FEQUAL(a, b)`: `|a - b| < FLT_EPSILON` in `float`.
fn fequal(a: f32, b: f32) -> bool {
    sse::sub32(a, b).abs() < f32::EPSILON
}

/// "The region clipped to the volume" per axis: `[max(s, 0), min(s + d, e))` (the sum in 32-bit
/// wrapping arithmetic).
fn clip(s: i32, d: i32, e: i32) -> (i32, i32) {
    (s.max(0), s.wrapping_add(d).min(e))
}

/// The region's clipped bounds `(x0, x1, y0, y1, z0, z1)`.
fn clipped(m: &Mri, r: &Region) -> (i32, i32, i32, i32, i32, i32) {
    let g = &m.geom;
    let (x0, x1) = clip(r.x, r.dx, g.width);
    let (y0, y1) = clip(r.y, r.dy, g.height);
    let (z0, z1) = clip(r.z, r.dz, g.depth);
    (x0, x1, y0, y1, z0, z1)
}

/// Whether a type is one the region histogram counts.
fn region_type(kind: i32) -> bool {
    matches!(
        kind,
        types::UCHAR | types::SHORT | types::USHRT | types::FLOAT
    )
}

/// Whether a type's bins are made from integer differences (bytes, shorts, unsigned shorts).
fn integer_type(kind: i32) -> bool {
    matches!(kind, types::UCHAR | types::SHORT | types::USHRT)
}

impl Process {
    /// "The bin count of a range": `nint((max - min) + 1.0)`, the difference in `float`, the sum
    /// in `double`; beyond the `int` range or NaN -2147483648 in both modes (FS-12 noted); above
    /// 2^26 the end of the program (FS-965, both modes).
    fn range_bins(&mut self, lo: f32, hi: f32) -> Result<i32, Stop> {
        let s = sse::add64(sse::widen(sse::sub32(hi, lo)), 1.0);
        let y = if s < 0.0 { s - 0.5 } else { s + 0.5 };
        let n = if y.is_nan() || y >= 2_147_483_648.0 || y <= -2_147_483_649.0 {
            self.note(
                &FS_12,
                "a range's bin count beyond the int range or NaN gave -2147483648 (both modes)",
            );
            i32::MIN
        } else {
            y.trunc() as i32
        };
        if n > MOST_RANGE_BINS {
            self.note(
                &FS_965,
                format!("a histogram of {n} bins taken from a range refused"),
            );
            self.error_exit(
                codes::NO_MEMORY,
                format!(
                    "HISTOalloc({n}): a histogram of {n} bins (the values' range, {lo:e} to {hi:e}) is refused (FreeSurfer allocates {} bytes here)",
                    8 * i64::from(n)
                )
                .as_bytes(),
            )?;
        }
        Ok(n)
    }

    /// A given histogram prepared for `n` bins: reallocated (zeros, `max_bins = nbins = n`) when
    /// fewer are in use, else its `nbins` set to `n`.
    fn prepare_given(&mut self, h: &mut Histogram, n: i32) -> Result<(), Stop> {
        if h.nbins < n {
            self.histo_realloc_in(h, n)
        } else {
            h.nbins = n;
            Ok(())
        }
    }

    /// A count in bin `b` of a histogram "with no bound" (FS-498): inside `[0, nbins - 1]` there;
    /// else by default the nearer end (0 when `nbins` is 0 or below), with `reproduce` where
    /// FreeSurfer puts it when that bin lies in the allocation, nowhere outside it.
    fn count_unbounded(&mut self, h: &mut Histogram, b: i32, what: &str) {
        let target = if b >= 0 && b < h.nbins {
            Some(b)
        } else if self.mode().reproduce() {
            if b >= 0 && b < h.max_bins && h.inside(b) {
                self.note(
                    &FS_498,
                    format!(
                        "{what}: a voxel counted in bin {b}, past the {} bins in use",
                        h.nbins
                    ),
                );
                Some(b)
            } else {
                self.note(
                    &FS_498,
                    format!("{what}: a voxel of bin {b}, outside the histogram's allocation, not counted"),
                );
                None
            }
        } else {
            let t = if b < 0 || h.nbins <= 0 {
                0
            } else {
                h.nbins - 1
            };
            self.note(
                &FS_498,
                format!("{what}: a voxel of bin {b} counted in the nearer end bin {t}"),
            );
            Some(t)
        };
        if let Some(t) = target {
            let c = sse::add32(1.0, h.count(t));
            h.set_count(t, c);
        }
    }

    /// The region histogram (§52; used by `MRIhistogramRegion`, `MRIhistogramVoxel` and
    /// `MRIhistoSegment`): frame 0's range over the clipped region, the histogram (new, or the
    /// given one prepared; FS-501 for a negative count) cleared and initialised, its bins'
    /// values `b bin_size + min`, then each voxel counted (bytes' bins below 0: FS-498). Other
    /// types: the error and no histogram (a given one keeps its values, zero counts).
    pub(crate) fn region_histogram(
        &mut self,
        m: &Mri,
        nbins: i32,
        given: Option<&mut Histogram>,
        r: &Region,
    ) -> Result<HistOut, Stop> {
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        self.mri_val_range_region(m, &mut lo, &mut hi, Some(r))?;
        let mut nbins = nbins;
        if nbins == 0 {
            nbins = self.range_bins(lo, hi)?;
        }
        let mut fresh = None;
        let is_given = given.is_some();
        let h: &mut Histogram = match given {
            None => fresh.insert(self.histo_alloc(nbins)?),
            Some(g) => {
                self.prepare_given(g, nbins)?;
                if g.nbins < 0 {
                    self.note(
                        &FS_501,
                        "a given histogram of a negative bin count refused (FreeSurfer crashes)",
                    );
                    self.error_exit(
                        codes::BAD_PARM,
                        format!(
                            "mriHistogramRegion: a histogram of {} bins cannot be cleared (FreeSurfer crashes here)",
                            g.nbins
                        )
                        .as_bytes(),
                    )?;
                }
                g
            }
        };
        for b in 0..h.nbins.max(0) {
            h.set_bin(b, 0.0);
            h.set_count(b, 0.0);
        }
        self.histo_init(Some(h), nbins, f64::from(lo), f64::from(hi))?;
        for b in 0..nbins.max(0) {
            let v = sse::add32(sse::mul32(b as f32, h.bin_size), lo);
            h.set_bin(b, v);
        }
        if !region_type(m.kind) {
            self.error_printf(
                codes::UNSUPPORTED,
                format!("mriHistogramRegion: unsupported mri type {}", m.kind).as_bytes(),
            );
            return Ok(Returned::Nothing);
        }
        let bytes = m.kind == types::UCHAR;
        let (x0, x1, y0, y1, z0, z1) = clipped(m, r);
        let top = nbins.wrapping_sub(1);
        for z in z0..z1 {
            for y in y0..y1 {
                for x in x0..x1 {
                    let v = self.mri_get_vox_val(m, x, y, z, 0, 0)?;
                    let q = sse::div32(sse::sub32(v, lo), h.bin_size);
                    let mut b = self.nint(sse::widen(q));
                    if b > top {
                        b = top;
                    }
                    if b < 0 {
                        if bytes {
                            self.count_unbounded(h, b, "the region histogram of bytes");
                            continue;
                        }
                        b = 0;
                    }
                    let c = sse::add32(1.0, h.count(b));
                    h.set_count(b, c);
                }
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None if is_given => Returned::Output,
            None => Returned::Nothing,
        })
    }

    /// `MRIhistogramRegion(mri, nbins, histo, region)` (§52): a blank region with `nbins` 0 or
    /// below gives the error and no histogram; else the bin count (255 when not above 0), the
    /// histogram (new or the given one prepared) cleared, its bin size `(max - min + 1) / nbins`,
    /// then the region histogram; the histogram is returned whatever the region histogram did.
    pub fn mri_histogram_region(
        &mut self,
        m: &Mri,
        nbins: i32,
        given: Option<&mut Histogram>,
        r: &Region,
    ) -> Result<HistOut, Stop> {
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        self.mri_val_range_region(m, &mut lo, &mut hi, Some(r))?;
        let mut nbins = nbins;
        if nbins <= 0 && fequal(lo, hi) {
            let mut msg = b"MRIhistogramRegion: input mri ".to_vec();
            msg.extend_from_slice(&m.geom.fname);
            msg.extend_from_slice(b" is blank");
            self.error_printf(codes::UNSUPPORTED, &msg);
            return Ok(Returned::Nothing);
        }
        if nbins <= 0 {
            nbins = self.range_bins(lo, hi)?;
            if !(nbins > 0) {
                nbins = 255;
            }
        }
        let mut fresh = None;
        let is_given = given.is_some();
        let h: &mut Histogram = match given {
            None => fresh.insert(self.histo_alloc(nbins)?),
            Some(g) => {
                self.prepare_given(g, nbins)?;
                g
            }
        };
        for b in 0..h.nbins.max(0) {
            h.set_bin(b, 0.0);
            h.set_count(b, 0.0);
        }
        h.bin_size = sse::div32(sse::add32(sse::sub32(hi, lo), 1.0), nbins as f32);
        self.region_histogram(m, nbins, Some(h), r)?;
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None if is_given => Returned::Output,
            None => Returned::Nothing,
        })
    }

    /// `MRIhistogram(mri, nbins)` (§52): the range of every frame, `nbins` 0 the range's bin
    /// count, a maximum equal to the minimum 255; every voxel of every frame counted with no
    /// bound (FS-498).
    pub fn mri_histogram(&mut self, m: &Mri, nbins: i32) -> Result<Histogram, Stop> {
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        self.mri_val_range(m, &mut lo, &mut hi)?;
        let mut nbins = nbins;
        if nbins == 0 {
            nbins = self.range_bins(lo, hi)?;
        }
        if hi == lo {
            hi = 255.0;
        }
        let mut h = match self.histo_init(None, nbins, f64::from(lo), f64::from(hi))? {
            Returned::New(h) => h,
            _ => unreachable!("HISTOinit without a histogram makes one"),
        };
        let g = m.geom.clone();
        for f in 0..m.nframes {
            for z in 0..g.depth {
                for y in 0..g.height {
                    for x in 0..g.width {
                        let v = self.mri_get_vox_val(m, x, y, z, f, 0)?;
                        let d = sse::narrow(sse::sub64(sse::widen(v), sse::widen(lo)));
                        let q = sse::div32(d, h.bin_size);
                        let b = self.nint(sse::widen(q));
                        self.count_unbounded(&mut h, b, "MRIhistogram");
                    }
                }
            }
        }
        Ok(h)
    }

    /// A bin's count read for the equalisation (FS-499: outside the allocation 0, noted).
    fn eq_count(&mut self, h: &Histogram, b: i64) -> f32 {
        match i32::try_from(b) {
            Ok(b) if h.inside(b) => h.count(b),
            _ => {
                self.note(
                    &FS_499,
                    format!("a histogram's bin {b} outside its allocation read as 0"),
                );
                0.0
            }
        }
    }

    /// A bin's count written for the equalisation (FS-499: outside the allocation dropped).
    fn eq_set_count(&mut self, h: &mut Histogram, b: i64, v: f32) {
        match i32::try_from(b) {
            Ok(b) if h.inside(b) => h.set_count(b, v),
            _ => self.note(
                &FS_499,
                format!("a write to a histogram's bin {b} outside its allocation dropped"),
            ),
        }
    }

    /// A bin's value written for the equalisation (FS-499 as for counts).
    fn eq_set_bin(&mut self, h: &mut Histogram, b: i64, v: f32) {
        match i32::try_from(b) {
            Ok(b) if h.inside(b) => h.set_bin(b, v),
            _ => self.note(
                &FS_499,
                format!("a write to a histogram's bin {b} outside its allocation dropped"),
            ),
        }
    }

    /// `MRIgetEqualizeHisto(mri, histo_eq, low, high, norm)` (§52): the cumulative histogram of
    /// the volume's histogram from `low` to `high` (normalised to 255 with `norm`), indexed by
    /// intensity (FS-499); FS-500 for the values of the bins `low` to `high`.
    pub fn mri_get_equalize_histo(
        &mut self,
        m: &Mri,
        given: Option<&mut Histogram>,
        low: i32,
        high: i32,
        norm: bool,
    ) -> Result<HistOut, Stop> {
        let h = self.mri_histogram(m, 0)?;
        let mut fresh = None;
        let is_given = given.is_some();
        let res: &mut Histogram = match given {
            None => fresh.insert(self.histo_alloc(h.nbins)?),
            Some(g) => g,
        };
        let mut high = high;
        if (high as f32) > h.max {
            high = self.int_fs12(sse::widen(h.max));
        }
        let (lo64, hi64) = (i64::from(low), i64::from(high));
        let mut total = 0i32;
        for i in lo64..=hi64 {
            let c = self.eq_count(&h, i);
            total = self.int_fs12(sse::widen(sse::add32(total as f32, c)));
        }
        let total_pix = total;
        for i in 0..lo64 {
            self.eq_set_count(res, i, 0.0);
            self.eq_set_bin(res, i, i as f32);
        }
        let reproduce = self.mode().reproduce();
        total = 0;
        for i in lo64..=hi64 {
            let c = self.eq_count(&h, i);
            total = self.int_fs12(sse::widen(sse::add32(total as f32, c)));
            let v = if norm {
                let q = sse::div32(sse::mul32(255.0, total as f32), total_pix as f32);
                self.nint(sse::widen(q)) as f32
            } else {
                total as f32
            };
            self.eq_set_count(res, i, v);
            let inside = i32::try_from(i).is_ok_and(|b| res.inside(b));
            if inside {
                let b = i as i32;
                let old = res.bin(b);
                if old != i as f32 {
                    if reproduce {
                        self.note(
                            &FS_500,
                            format!("the cumulative histogram's bin {b} keeps its value {old}"),
                        );
                    } else {
                        self.note(
                            &FS_500,
                            format!("the cumulative histogram's bin {b} given its number as value"),
                        );
                    }
                }
                if !reproduce {
                    res.set_bin(b, i as f32);
                }
            } else {
                self.note(
                    &FS_499,
                    format!("a write to a histogram's bin {i} outside its allocation dropped"),
                );
            }
        }
        let at_high = self.eq_count(res, hi64);
        for i in hi64 + 1..i64::from(h.nbins) {
            let v = sse::add32(at_high, i as f32);
            self.eq_set_count(res, i, v);
            self.eq_set_bin(res, i, i as f32);
        }
        res.nbins = h.nbins;
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None if is_given => Returned::Output,
            None => Returned::Nothing,
        })
    }

    /// `MRIhistoEqualize(src, template, dst, low, high)` (§52): both normalised cumulative
    /// histograms first; then each voxel of frame 0 whose `(int)` value lies in `[low, high]`
    /// becomes the first index from `low` whose template count reaches the source's count at that
    /// value, others their `(int)` value.
    pub fn mri_histo_equalize(
        &mut self,
        src: &mut Mri,
        template: &mut Mri,
        dst: Dst,
        low: i32,
        high: i32,
    ) -> Result<VolOut, Stop> {
        let hs = match self.mri_get_equalize_histo(src, None, low, high, true)? {
            Returned::New(h) => h,
            _ => unreachable!("a new histogram"),
        };
        let ht = match self.mri_get_equalize_histo(template, None, low, high, true)? {
            Returned::New(h) => h,
            _ => unreachable!("a new histogram"),
        };
        if self.gdiag & DIAG_WRITE != 0 && self.gdiag & DIAG_VERBOSE != 0 {
            self.histo_plot(&hs, b"cum_src.plt");
            self.histo_plot(&ht, b"cum_template.plt");
        }
        let mut v = Vols::new(src, Some(template), dst);
        self.clone_dst(&mut v)?;
        let g = v.a().geom.clone();
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    let val = self.mri_get_vox_val(v.a(), x, y, z, 0, 0)?;
                    let sval = self.int_fs12(sse::widen(val));
                    // §52 (item 129 b): the first index from `low` while below the template's
                    // bin count whose count reaches the source's; none (no index there, or no
                    // count high enough): the voxel's own `sval`, as outside the limits.
                    let out = if low <= sval && sval <= high {
                        let sc = self.eq_count(&hs, i64::from(sval));
                        let mut j = i64::from(low);
                        let mut found = None;
                        while j < i64::from(ht.nbins) {
                            if self.eq_count(&ht, j) >= sc {
                                found = Some(j);
                                break;
                            }
                            j += 1;
                        }
                        found.map_or(sval as f32, |j| j as f32)
                    } else {
                        sval as f32
                    };
                    self.mri_set_vox_val(v.d_mut(), x, y, z, 0, out, 0)?;
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIhistogramLabelRegion(mri, labeled, region, label, nbins)` (§52): the histogram of
    /// the region's voxels of `label` (`nint` of the labelled volume's frame-0 value), with no
    /// bound (FS-498); a constant region gives the error and a histogram of bin size 1; a type
    /// other than bytes, shorts, unsigned shorts and floats the error and none at its first
    /// labelled voxel.
    pub fn mri_histogram_label_region(
        &mut self,
        m: &Mri,
        labeled: &Mri,
        r: &Region,
        label: i32,
        nbins: i32,
    ) -> Result<Option<Histogram>, Stop> {
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        self.mri_val_range_region(m, &mut lo, &mut hi, Some(r))?;
        let bmin = self.int_fs12(sse::widen(lo));
        let mut nbins = nbins;
        if nbins <= 0 {
            nbins = self.range_bins(lo, hi)?;
        }
        let mut h = match self.histo_init(None, nbins, f64::from(lo), f64::from(hi))? {
            Returned::New(h) => h,
            _ => unreachable!("HISTOinit without a histogram makes one"),
        };
        if fequal(lo, hi) {
            h.bin_size = 1.0;
            self.error_printf(codes::BAD_PARM, b"MRIhistogramLabelRegion: constant image");
            return Ok(Some(h));
        }
        let (x0, x1, y0, y1, z0, z1) = clipped(m, r);
        for z in z0..z1 {
            for y in y0..y1 {
                for x in x0..x1 {
                    let lv = self.mri_get_vox_val(labeled, x, y, z, 0, 0)?;
                    if self.nint(sse::widen(lv)) != label {
                        continue;
                    }
                    let v = self.mri_get_vox_val(m, x, y, z, 0, 0)?;
                    let q = if integer_type(m.kind) {
                        sse::div32((v as i32).wrapping_sub(bmin) as f32, h.bin_size)
                    } else if m.kind == types::FLOAT {
                        sse::div32(sse::sub32(v, lo), h.bin_size)
                    } else {
                        self.error_printf(
                            codes::UNSUPPORTED,
                            format!("MRIhistogramLabelRegion: unsupported type {}", m.kind)
                                .as_bytes(),
                        );
                        return Ok(None);
                    };
                    let b = self.nint(sse::widen(q));
                    self.count_unbounded(&mut h, b, "MRIhistogramLabelRegion");
                }
            }
        }
        Ok(Some(h))
    }

    /// `MRIhistogramLabel(mri, labeled, label, nbins)` (§52): the histogram of the volume's
    /// voxels whose labelled frame-0 value equals `label` (as `float`s), bins kept inside.
    pub fn mri_histogram_label(
        &mut self,
        m: &Mri,
        labeled: &Mri,
        label: i32,
        nbins: i32,
    ) -> Result<Histogram, Stop> {
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        self.mri_label_val_range(m, labeled, label, &mut lo, &mut hi)?;
        let bmin = self.int_fs12(sse::widen(lo));
        let mut nbins = nbins;
        if nbins <= 0 {
            nbins = self.range_bins(lo, hi)?;
        }
        let mut h = match self.histo_init(None, nbins, f64::from(lo), f64::from(hi))? {
            Returned::New(h) => h,
            _ => unreachable!("HISTOinit without a histogram makes one"),
        };
        let g = m.geom.clone();
        let lab = label as f32;
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    let lv = self.mri_get_vox_val(labeled, x, y, z, 0, 0)?;
                    if lv != lab {
                        continue;
                    }
                    let v = self.mri_get_vox_val(m, x, y, z, 0, 0)?;
                    let q = if integer_type(m.kind) {
                        sse::div32((v as i32).wrapping_sub(bmin) as f32, h.bin_size)
                    } else {
                        sse::div32(sse::sub32(v, lo), h.bin_size)
                    };
                    let mut b = self.nint(sse::widen(q));
                    if b < 0 {
                        b = 0;
                    }
                    if b >= nbins {
                        b = nbins - 1;
                    }
                    let c = sse::add32(1.0, h.count(b));
                    h.set_count(b, c);
                }
            }
        }
        Ok(h)
    }

    /// `MRIhistogramVoxel(mri, nbins, histo, x0, y0, z0, wsize, mri_thresh, thresh)` (§52): the
    /// window of `wsize` voxels per axis around the voxel, its range and bin count (255 when not
    /// above 0), the histogram (new or given, prepared) cleared and initialised, the region
    /// histogram or the thresholded one, then `HISTOsoapBubbleZeros(h, h, 100)`.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_histogram_voxel(
        &mut self,
        m: &Mri,
        nbins: i32,
        given: Option<&mut Histogram>,
        (x0, y0, z0): (i32, i32, i32),
        wsize: i32,
        thresh_vol: Option<&Mri>,
        thresh: f32,
    ) -> Result<HistOut, Stop> {
        let w2 = wsize.wrapping_sub(1) / 2;
        let r = Region {
            x: x0.wrapping_sub(w2),
            y: y0.wrapping_sub(w2),
            z: z0.wrapping_sub(w2),
            dx: wsize,
            dy: wsize,
            dz: wsize,
        };
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        self.mri_val_range_region(m, &mut lo, &mut hi, Some(&r))?;
        let mut nbins = nbins;
        if nbins <= 0 {
            nbins = self.range_bins(lo, hi)?;
            if !(nbins > 0) {
                nbins = 255;
            }
        }
        let mut fresh = None;
        let is_given = given.is_some();
        let h: &mut Histogram = match given {
            None => fresh.insert(self.histo_alloc(nbins)?),
            Some(g) => {
                self.prepare_given(g, nbins)?;
                g
            }
        };
        for b in 0..h.nbins.max(0) {
            h.set_bin(b, 0.0);
            h.set_count(b, 0.0);
        }
        self.histo_init(Some(h), nbins, f64::from(lo), f64::from(hi))?;
        match thresh_vol {
            None => {
                self.region_histogram(m, nbins, Some(h), &r)?;
            }
            Some(t) => {
                self.mri_histogram_region_with_threshold(m, nbins, Some(h), &r, t, thresh, 0)?;
            }
        }
        self.histo_soap_bubble_zeros(h, Output::First, 100)?;
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None if is_given => Returned::Output,
            None => Returned::Nothing,
        })
    }

    /// `MRIhistogramRegionWithThreshold(mri, nbins, histo, region, mri_thresh, thresh, frame)`
    /// (§52): the region's voxels of `frame` whose threshold value is not below `thresh`, their
    /// range (from 1e10 and -1e10), the histogram (new or given, prepared) cleared and
    /// initialised, each voxel added with `HISTOaddSample` (FS-503: one thread's sums at any
    /// thread count).
    #[allow(clippy::too_many_arguments)]
    pub fn mri_histogram_region_with_threshold(
        &mut self,
        m: &Mri,
        nbins: i32,
        given: Option<&mut Histogram>,
        r: &Region,
        thresh_vol: &Mri,
        thresh: f32,
        frame: i32,
    ) -> Result<HistOut, Stop> {
        let (x0, x1, y0, y1, z0, z1) = clipped(m, r);
        let (mut lo, mut hi) = (1e10f32, -1e10f32);
        let mut taken = Vec::new();
        for z in z0..z1 {
            for y in y0..y1 {
                for x in x0..x1 {
                    let t = self.mri_get_vox_val(thresh_vol, x, y, z, frame, 0)?;
                    if t < thresh {
                        continue;
                    }
                    let v = self.mri_get_vox_val(m, x, y, z, frame, 0)?;
                    if v < lo {
                        lo = v;
                    }
                    if v > hi {
                        hi = v;
                    }
                    taken.push(v);
                }
            }
        }
        let mut nbins = nbins;
        if nbins == 0 {
            nbins = self.range_bins(lo, hi)?;
        }
        let mut fresh = None;
        let is_given = given.is_some();
        let h: &mut Histogram = match given {
            None => fresh.insert(self.histo_alloc(nbins)?),
            Some(g) => {
                self.prepare_given(g, nbins)?;
                g
            }
        };
        for b in 0..h.nbins.max(0) {
            h.set_bin(b, 0.0);
            h.set_count(b, 0.0);
        }
        self.histo_init(Some(h), nbins, f64::from(lo), f64::from(hi))?;
        // FreeSurfer's per-thread histogram (observed: `vhist`, a given histogram with a negative
        // count ends at its `HISTOalloc`; a range of one value places the samples by the new
        // histogram's own `HISTOinit` over that range, whose `max_bins` is `nbins`, not by the
        // given one's): `HISTOinit(NULL, nbins, min, max)`, the samples added into it with
        // `HISTOaddSample` and its counts summed into `h`. With a range of more than one value,
        // or a new `h`, this is `HISTOaddSample(h, v, min, max)` itself.
        let mut t = match self.histo_init(None, nbins, f64::from(lo), f64::from(hi))? {
            Returned::New(t) => t,
            _ => unreachable!("HISTOinit without a histogram makes one"),
        };
        for v in taken {
            self.histo_add_sample(&mut t, v, lo, hi);
        }
        for b in 0..nbins.max(0) {
            let c = sse::add32(h.count(b), t.count(b));
            h.set_count(b, c);
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None if is_given => Returned::Output,
            None => Returned::Nothing,
        })
    }

    /// `MRIhistoSegment(src, labeled, wm_low, wm_hi, gray_hi, wsize, sigma)` (§52): each voxel
    /// whose label byte is 128 relabelled from the smoothed region histogram of its window (the
    /// white- and gray-matter peaks and the valley between them); FS-501 and FS-502 end the
    /// program in both modes.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_histo_segment(
        &mut self,
        src: &mut Mri,
        labeled: Dst,
        wm_low: i32,
        wm_hi: i32,
        gray_hi: i32,
        wsize: i32,
        sigma: f32,
    ) -> Result<VolOut, Stop> {
        let name = "MRIhistoSegment";
        let sigma = if sigma < 0.0 { 4.0 } else { sigma };
        let mut v = Vols::new(src, None, labeled);
        self.clone_dst(&mut v)?;
        let g = v.a().geom.clone();
        let (w, hgt, d) = (g.width, g.height, g.depth);
        let mut hist: Option<Histogram> = None;
        let mut smooth: Option<Histogram> = None;
        let (mut processed, mut changed) = (0i32, 0i32);
        let half = wsize / 2;
        for z in 0..d {
            self.diag_show_pct_done(sse::div32(z as f32, (d.wrapping_sub(1)) as f32), 5);
            for y in 0..hgt {
                for x in 0..w {
                    let lb = self.row_byte(v.d(), (x, y, z), name);
                    if lb != AMBIGUOUS {
                        continue;
                    }
                    processed += 1;
                    let in_val = i32::from(self.row_byte(v.a(), (x, y, z), name));
                    let r = Region {
                        x: x.wrapping_sub(half),
                        y: y.wrapping_sub(half),
                        z: z.wrapping_sub(half),
                        dx: wsize,
                        dy: wsize,
                        dz: wsize,
                    };
                    match hist.as_mut() {
                        None => {
                            if let Returned::New(h) = self.region_histogram(v.a(), 0, None, &r)? {
                                hist = Some(h);
                            }
                        }
                        Some(h) => {
                            self.region_histogram(v.a(), 0, Some(h), &r)?;
                        }
                    }
                    let Some(h) = hist.as_mut() else {
                        self.note(
                            &FS_501,
                            "no region histogram to smooth (a type the region histogram refuses)",
                        );
                        self.error_exit(
                            codes::UNSUPPORTED,
                            b"MRIhistoSegment: no region histogram to smooth (FreeSurfer crashes here)",
                        )?;
                        return Ok(VolOut::Nothing);
                    };
                    match smooth.as_mut() {
                        None => {
                            if let Returned::New(s) = self.histo_smooth(h, Output::New, sigma)? {
                                smooth = Some(s);
                            }
                        }
                        Some(s) => {
                            self.histo_smooth(h, Output::Given(s), sigma)?;
                        }
                    }
                    let hs = smooth.as_ref().expect("the smoothed histogram");
                    let peak_value = |p: &mut Process, i: i32| -> i32 {
                        if i >= 0 {
                            p.int_fs12(sse::widen(hs.bin(i)))
                        } else {
                            i
                        }
                    };
                    let i = self.histo_find_last_peak_in_region(
                        hs,
                        11,
                        0.15,
                        wm_low,
                        wm_hi.wrapping_sub(7),
                    );
                    let mut wm_peak = peak_value(self, i);
                    let i = self.histo_find_last_peak_in_region(
                        hs,
                        11,
                        0.15,
                        72,
                        wm_peak.wrapping_sub(10),
                    );
                    let mut gray_peak = peak_value(self, i);
                    if wm_peak >= 0 && gray_peak >= 0 {
                        let mut seen = HashSet::new();
                        seen.insert(wm_peak);
                        while gray_peak > gray_hi {
                            wm_peak = gray_peak;
                            if !seen.insert(wm_peak) {
                                self.note(
                                    &FS_502,
                                    "the gray-matter peak's search came back to a peak found before",
                                );
                                self.error_exit(
                                    codes::BAD_PARM,
                                    format!(
                                        "MRIhistoSegment: the gray-matter peak {gray_peak} stays above {gray_hi} (FreeSurfer never ends here)"
                                    )
                                    .as_bytes(),
                                )?;
                            }
                            let i = self.histo_find_last_peak_in_region(
                                hs,
                                11,
                                0.15,
                                72,
                                wm_peak.wrapping_sub(10),
                            );
                            gray_peak = peak_value(self, i);
                        }
                    }
                    let mut valley = if wm_peak < 0 || gray_peak < 0 {
                        -1
                    } else {
                        let i = self.histo_find_valley(
                            hs,
                            3,
                            gray_peak.wrapping_add(2),
                            wm_peak.wrapping_sub(2),
                        );
                        peak_value(self, i)
                    };
                    if valley > gray_hi {
                        valley = -1;
                    }
                    let nb =
                        if valley < 0 || (in_val - valley).wrapping_abs() <= 2 || valley >= gray_hi
                        {
                            AMBIGUOUS
                        } else {
                            changed += 1;
                            if in_val >= valley { WHITE } else { NOT_WHITE }
                        };
                    self.byte_put(v.d_mut(), x, y, z, nb, name);
                }
            }
        }
        if self.gdiag & DIAG_SHOW != 0 {
            let total = w.wrapping_mul(hgt).wrapping_mul(d) as f32;
            for (n, what) in [(processed, "processed"), (changed, "labeled")] {
                let pct = sse::div32(sse::mul32(100.0, n as f32), total);
                let s = larmorx_core::cfmt::pad(&larmorx_core::cfmt::f(sse::widen(pct), 2), 2);
                self.put_err(format!("              {n:>8} voxels {what} ({s}%)\n").as_bytes());
            }
        }
        Ok(v.out())
    }
}
