// SPDX-License-Identifier: Apache-2.0
//! Histograms (spec §5, §21): the record, making and copying, arithmetic, smoothing, the
//! searches, samples, totals and statistics, the robust Gaussian fit and the files. The
//! operand orders of §21 (which NaN a sum keeps) are followed explicitly.
// C's comparisons, NaN included: a negated comparison is FreeSurfer's test, not a style choice.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use larmorx_core::defects::{FS_18, FS_19, FS_20, FS_21, FS_22, FS_23, FS_24, FS_50, FS_51};

use super::Output;
use super::Returned;
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};
use crate::sse;

/// The diagnostic bit that lets functions write their files (`DIAG_WRITE`).
pub const DIAG_WRITE: u64 = 0x8;

/// A histogram (§5.1): `max_bins` allocated, `nbins` in use, a value and a count per bin, the
/// bin size and the range.
#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    pub max_bins: i32,
    pub nbins: i32,
    pub bin_size: f32,
    pub min: f32,
    pub max: f32,
    /// The bins' values (the allocated array).
    pub bins: Vec<f32>,
    /// The bins' counts (the allocated array).
    pub counts: Vec<f32>,
}

impl Histogram {
    /// A new histogram of `n` bins (n ≥ 0), zeros everywhere.
    pub fn new(n: i32) -> Histogram {
        let k = n.max(0) as usize;
        Histogram {
            max_bins: n,
            nbins: n,
            bin_size: 0.0,
            min: 0.0,
            max: 0.0,
            bins: vec![0.0; k],
            counts: vec![0.0; k],
        }
    }

    /// Whether bin `b` lies in the allocated arrays.
    pub fn inside(&self, b: i32) -> bool {
        b >= 0 && (b as usize) < self.counts.len()
    }

    /// Bin `b`'s value, 0 outside the arrays (§1).
    pub fn bin(&self, b: i32) -> f32 {
        if b >= 0 && (b as usize) < self.bins.len() {
            self.bins[b as usize]
        } else {
            0.0
        }
    }

    /// Bin `b`'s count, 0 outside the arrays.
    pub fn count(&self, b: i32) -> f32 {
        if self.inside(b) {
            self.counts[b as usize]
        } else {
            0.0
        }
    }

    /// Sets bin `b`'s value (dropped outside).
    pub fn set_bin(&mut self, b: i32, v: f32) {
        if b >= 0 && (b as usize) < self.bins.len() {
            self.bins[b as usize] = v;
        }
    }

    /// Sets bin `b`'s count (dropped outside).
    pub fn set_count(&mut self, b: i32, v: f32) {
        if self.inside(b) {
            self.counts[b as usize] = v;
        }
    }

    /// FS-21: the arrays grown to hold `n` bins (`max_bins` unchanged).
    fn grow(&mut self, n: i32) -> bool {
        let n = n.max(0) as usize;
        if self.bins.len() < n {
            self.bins.resize(n, 0.0);
            self.counts.resize(n, 0.0);
            true
        } else {
            false
        }
    }

    /// The bins in use, as a range of indices.
    fn used(&self) -> std::ops::Range<i32> {
        0..self.nbins.max(0)
    }
}

/// The float sum of `terms` in the totals' order (§21): with 4 or more terms, blocks of four
/// `T + c0`, `T + c1`, `c2 + T`, `c3 + T`, then the rest `T + c`.
fn total_sum(terms: &[f32]) -> f32 {
    let mut t = 0.0f32;
    let mut i = 0;
    if terms.len() >= 4 {
        while i + 4 <= terms.len() {
            t = sse::add32(t, terms[i]);
            t = sse::add32(t, terms[i + 1]);
            t = sse::add32(terms[i + 2], t);
            t = sse::add32(terms[i + 3], t);
            i += 4;
        }
    }
    for &c in &terms[i..] {
        t = sse::add32(t, c);
    }
    t
}

/// The density's float sum (§21): blocks of four `c0 + T`, `T + c1`, `c2 + T`, `T + c3`, then
/// `T + c`.
fn pdf_sum(terms: &[f32]) -> f32 {
    let mut t = 0.0f32;
    let mut i = 0;
    if terms.len() >= 4 {
        while i + 4 <= terms.len() {
            t = sse::add32(terms[i], t);
            t = sse::add32(t, terms[i + 1]);
            t = sse::add32(terms[i + 2], t);
            t = sse::add32(t, terms[i + 3]);
            i += 4;
        }
    }
    for &c in &terms[i..] {
        t = sse::add32(t, c);
    }
    t
}

/// The window's half width of the searches, `(wsize - 1) / 2` (C's division).
fn half(wsize: i32) -> i32 {
    (wsize - 1) / 2
}

impl Process {
    /// A count read with FS-22 noted outside the arrays (§1: 0).
    fn hc(&mut self, h: &Histogram, b: i32) -> f32 {
        if !h.inside(b) {
            self.note(&FS_22, "a histogram's bin outside its arrays read as 0");
        }
        h.count(b)
    }

    /// A bin value read with FS-22 noted outside.
    fn hb(&mut self, h: &Histogram, b: i32) -> f32 {
        if b < 0 || b as usize >= h.bins.len() {
            self.note(&FS_22, "a histogram's bin outside its arrays read as 0");
        }
        h.bin(b)
    }

    /// A count written with FS-22 noted outside (dropped).
    fn hset(&mut self, h: &mut Histogram, b: i32, v: f32) {
        if !h.inside(b) {
            self.note(
                &FS_22,
                "a write to a histogram's bin outside its arrays dropped",
            );
        }
        h.set_count(b, v);
    }

    // ---- §5.1, §5.2: making and copying

    /// `HISTOalloc(n)`: n negative ends the program (code -2).
    pub fn histo_alloc(&mut self, n: i32) -> Result<Histogram, Stop> {
        self.histo_alloc_named(n, "HISTOalloc")
    }

    fn histo_alloc_named(&mut self, n: i32, name: &str) -> Result<Histogram, Stop> {
        if n < 0 {
            self.errno = 12;
            self.error_exit(
                codes::NO_MEMORY,
                format!("{name}({n}): could not allocate histogram").as_bytes(),
            )?;
        }
        Ok(Histogram::new(n))
    }

    /// `HISTOrealloc(h, n)`: absent, a new one; else new zeroed arrays of n bins, `max_bins =
    /// nbins = n`, the other fields kept.
    pub fn histo_realloc(
        &mut self,
        h: Option<&mut Histogram>,
        n: i32,
    ) -> Result<Returned<Histogram>, Stop> {
        match h {
            None => Ok(Returned::New(self.histo_alloc(n)?)),
            Some(h) => {
                self.histo_realloc_in(h, n)?;
                Ok(Returned::Output)
            }
        }
    }

    fn histo_realloc_in(&mut self, h: &mut Histogram, n: i32) -> Result<(), Stop> {
        let fresh = self.histo_alloc_named(n, "HISTOrealloc")?;
        h.bins = fresh.bins;
        h.counts = fresh.counts;
        h.max_bins = n;
        h.nbins = n;
        Ok(())
    }

    /// `HISTOfree(&h)`: the caller's histogram becomes none; returns 0.
    pub fn histo_free(&mut self, h: &mut Option<Histogram>) -> i32 {
        *h = None;
        0
    }

    /// The copy's fields into `dst` (FS-21: a dst too small is grown, in both modes).
    fn copy_fields(&mut self, src: &Histogram, dst: &mut Histogram) {
        if dst.grow(src.nbins) {
            self.note(
                &FS_21,
                "a histogram output with fewer bins allocated than the copy grown to hold it",
            );
        }
        dst.nbins = src.nbins;
        dst.bin_size = src.bin_size;
        for b in src.used() {
            dst.set_bin(b, src.bin(b));
            dst.set_count(b, src.count(b));
        }
        dst.min = src.min;
        dst.max = src.max;
    }

    /// `HISTOcopy(src, dst)` (§5.2).
    pub fn histo_copy(
        &mut self,
        src: &mut Histogram,
        dst: Output<'_, Histogram>,
    ) -> Result<Returned<Histogram>, Stop> {
        match dst {
            Output::New => {
                let mut d = self.histo_alloc(src.nbins)?;
                self.copy_fields(src, &mut d);
                Ok(Returned::New(d))
            }
            Output::Given(d) => {
                self.copy_fields(src, d);
                Ok(Returned::Output)
            }
            Output::First | Output::Second => Ok(Returned::Output),
        }
    }

    /// The working histogram of a function whose output starts as a copy of `src` (none for
    /// `src` itself): a new one, the given one with `src` copied in, or `src`.
    fn work_copy(
        &mut self,
        src: &Histogram,
        dst: &Output<'_, Histogram>,
    ) -> Result<Histogram, Stop> {
        Ok(match dst {
            Output::New => {
                let mut d = self.histo_alloc(src.nbins)?;
                self.copy_fields(src, &mut d);
                d
            }
            Output::Given(d) => {
                let mut d = (**d).clone();
                self.copy_fields(src, &mut d);
                d
            }
            Output::First | Output::Second => src.clone(),
        })
    }

    /// `HISTOclear(src, dst)`.
    pub fn histo_clear(
        &mut self,
        src: &mut Histogram,
        dst: Output<'_, Histogram>,
    ) -> Result<Returned<Histogram>, Stop> {
        let clear = |d: &mut Histogram| {
            for b in d.used() {
                d.set_bin(b, 0.0);
                d.set_count(b, 0.0);
            }
        };
        match dst {
            Output::New => Ok(Returned::New(self.histo_alloc(src.nbins)?)),
            Output::Given(d) => {
                clear(d);
                Ok(Returned::Output)
            }
            Output::First | Output::Second => {
                clear(src);
                Ok(Returned::Output)
            }
        }
    }

    /// `HISTOclearCounts(src, dst)` (FS-18).
    pub fn histo_clear_counts(
        &mut self,
        src: &mut Histogram,
        dst: Output<'_, Histogram>,
    ) -> Result<Returned<Histogram>, Stop> {
        let reproduce = self.mode().reproduce();
        let clear = |p: &mut Process, d: &mut Histogram| {
            let others = d.used().skip(1).any(|b| d.count(b) != 0.0);
            let upto = if reproduce { d.nbins.min(1) } else { d.nbins };
            for b in 0..upto.max(0) {
                d.set_count(b, 0.0);
            }
            if others {
                p.note_fs18(reproduce, "counts");
            }
        };
        match dst {
            Output::New => {
                let mut d = self.histo_alloc(src.nbins)?;
                let others = src.used().skip(1).any(|b| src.bin(b) != 0.0);
                let upto = if reproduce {
                    src.nbins.min(1)
                } else {
                    src.nbins
                };
                for b in 0..upto.max(0) {
                    d.set_bin(b, src.bin(b));
                }
                if others {
                    self.note_fs18(reproduce, "bins");
                }
                Ok(Returned::New(d))
            }
            Output::Given(d) => {
                clear(self, d);
                Ok(Returned::Output)
            }
            Output::First | Output::Second => {
                clear(self, src);
                Ok(Returned::Output)
            }
        }
    }

    fn note_fs18(&mut self, reproduce: bool, what: &str) {
        if reproduce {
            self.note(
                &FS_18,
                format!("only the first of the histogram's {what} handled"),
            );
        } else {
            self.note(
                &FS_18,
                format!("every one of the histogram's {what} handled, not only the first"),
            );
        }
    }

    /// `HISTOinit(h, n, lo, hi)` (§5.2, §21).
    pub fn histo_init(
        &mut self,
        h: Option<&mut Histogram>,
        n: i32,
        lo: f64,
        hi: f64,
    ) -> Result<Returned<Histogram>, Stop> {
        let mut fresh = None;
        let h: &mut Histogram = match h {
            None => fresh.insert(self.histo_alloc(n)?),
            Some(h) => {
                for b in h.used() {
                    h.set_bin(b, 0.0);
                    h.set_count(b, 0.0);
                }
                h
            }
        };
        if n > h.max_bins {
            self.error_exit(
                codes::BAD_PARM,
                format!("HISTOinit: nbins {n} bigger than max bins {}\n", h.max_bins).as_bytes(),
            )?;
        }
        let (mut lo, mut hi) = (lo, hi);
        if hi <= lo {
            lo = 0.0;
            hi = f64::from(h.max_bins - 1);
        }
        h.min = sse::narrow(lo);
        h.max = sse::narrow(hi);
        let mut size = sse::narrow(sse::div64(sse::sub64(hi, lo), f64::from(n - 1)));
        // `<= 0` (a NaN size stays)
        if size <= 0.0 {
            size = 1.0;
        }
        h.bin_size = size;
        for b in 0..n.max(0) {
            let v = sse::narrow(sse::add64(sse::widen(sse::mul32(size, b as f32)), lo));
            h.set_bin(b, v);
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// `HISTObins(n, lo, hi)` (§5.2): a new histogram of uniform bins.
    pub fn histo_bins(&mut self, n: i32, lo: f64, hi: f64) -> Result<Histogram, Stop> {
        let mut h = self.histo_alloc(n)?;
        let size = sse::narrow(sse::div64(sse::sub64(hi, lo), f64::from(n - 1)));
        h.bin_size = size;
        for b in 0..n.max(0) {
            let v = sse::narrow(sse::add64(sse::widen(sse::mul32(size, b as f32)), lo));
            h.set_bin(b, v);
        }
        h.min = sse::narrow(lo);
        h.max = sse::narrow(hi);
        Ok(h)
    }

    // ---- §5.3: arithmetic

    /// `HISTOadd(h1, h2, dst)` (§5.3, §21).
    pub fn histo_add(
        &mut self,
        h1: &mut Histogram,
        h2: Option<&mut Histogram>,
        dst: Output<'_, Histogram>,
    ) -> Result<Returned<Histogram>, Stop> {
        self.histo_add_sub(h1, h2, dst, false)
    }

    /// `HISTOsubtract(h1, h2, dst)`.
    pub fn histo_subtract(
        &mut self,
        h1: &mut Histogram,
        h2: Option<&mut Histogram>,
        dst: Output<'_, Histogram>,
    ) -> Result<Returned<Histogram>, Stop> {
        self.histo_add_sub(h1, h2, dst, true)
    }

    fn histo_add_sub(
        &mut self,
        h1: &mut Histogram,
        h2: Option<&mut Histogram>,
        dst: Output<'_, Histogram>,
        subtract: bool,
    ) -> Result<Returned<Histogram>, Stop> {
        let a = h1.clone();
        let b = h2.as_ref().map(|h| (**h).clone());
        let n = a.nbins;
        // the counts of the result, in §21's operand order
        let alias = matches!(dst, Output::First | Output::Second);
        let new_h2 = b.is_none() && matches!(dst, Output::New);
        if b.is_none() && !matches!(dst, Output::New) {
            self.note(
                &FS_22,
                "no second histogram with a given output: zeros added",
            );
        }
        let blocks = !alias && !new_h2 && n > 3 && !subtract;
        let mut out = Vec::with_capacity(n.max(0) as usize);
        let full = (n.max(0) / 4) * 4;
        for i in 0..n.max(0) {
            let x = a.count(i);
            let y = match &b {
                Some(b) => self.hc(b, i),
                None => 0.0,
            };
            let v = if subtract {
                sse::sub32(x, y)
            } else if blocks && i >= full && (i - full) < 2 {
                sse::add32(y, x)
            } else {
                sse::add32(x, y)
            };
            out.push(v);
        }
        let write = |p: &mut Process, d: &mut Histogram| {
            if d.grow(n) {
                p.note(
                    &FS_21,
                    "a histogram output with fewer bins allocated than the result grown to hold it",
                );
            }
            d.nbins = n;
            for (i, &v) in out.iter().enumerate() {
                d.set_count(i as i32, v);
            }
        };
        match dst {
            Output::New => {
                let mut d = self.histo_alloc(n)?;
                if !subtract {
                    for i in a.used() {
                        d.set_bin(i, a.bin(i));
                    }
                    d.min = a.min;
                    d.max = a.max;
                    d.bin_size = a.bin_size;
                }
                write(self, &mut d);
                Ok(Returned::New(d))
            }
            Output::Given(d) => {
                write(self, d);
                Ok(Returned::Output)
            }
            Output::First => {
                write(self, h1);
                Ok(Returned::Output)
            }
            Output::Second => {
                if let Some(h2) = h2 {
                    write(self, h2);
                }
                Ok(Returned::Output)
            }
        }
    }

    /// `HISTOlinearScale(src, dst, scale, offset)` (§5.3, §21).
    pub fn histo_linear_scale(
        &mut self,
        src: &mut Histogram,
        dst: Output<'_, Histogram>,
        s: f32,
        o: f32,
    ) -> Result<Returned<Histogram>, Stop> {
        let s0 = src.clone();
        let mut w = self.work_copy(src, &dst)?;
        let d = &mut w;
        for b in d.used() {
            let v = sse::add32(sse::mul32(d.bin(b), s), o);
            d.set_bin(b, v);
        }
        d.min = sse::add32(sse::mul32(s0.min, s), o);
        d.max = sse::add32(o, sse::mul32(s0.max, s));
        d.bin_size = sse::mul32(s, s0.bin_size);
        Ok(put_back(w, dst, src))
    }

    /// `HISTOmakePDF(src, dst)` (§5.3, §21; FS-23).
    pub fn histo_make_pdf(
        &mut self,
        src: &mut Histogram,
        dst: Output<'_, Histogram>,
    ) -> Result<Returned<Histogram>, Stop> {
        let mut fresh = None;
        let d: &mut Histogram = match dst {
            Output::New => {
                let mut f = self.histo_alloc(src.nbins)?;
                self.copy_fields(src, &mut f);
                fresh = Some(f);
                fresh.as_mut().expect("new")
            }
            Output::Given(d) => {
                if self.mode().reproduce() {
                    self.note(
                        &FS_23,
                        "the output histogram's own counts normalised, not the source's",
                    );
                } else {
                    self.note(
                        &FS_23,
                        "the source copied into the output histogram before normalising",
                    );
                    self.copy_fields(src, d);
                }
                d
            }
            Output::First | Output::Second => src,
        };
        let counts: Vec<f32> = d.used().map(|b| d.count(b)).collect();
        let t = pdf_sum(&counts);
        if t > 0.0 {
            let floor = sse::narrow(sse::div64(1.0, sse::widen(sse::mul32(10.0, t))));
            for b in d.used() {
                let mut c = sse::div32(d.count(b), t);
                if f64::from(c).abs() < 1e-15 {
                    c = floor;
                }
                d.set_count(b, c);
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// `HISTOsumNorm(h)`: each count over the double sum of the counts; returns 0.
    pub fn histo_sum_norm(&mut self, h: &mut Histogram) -> i32 {
        let mut s = 0.0f64;
        for b in h.used() {
            s = sse::add64(s, sse::widen(h.count(b)));
        }
        if s != 0.0 {
            for b in h.used() {
                let v = sse::narrow(sse::div64(sse::widen(h.count(b)), s));
                h.set_count(b, v);
            }
        }
        0
    }

    /// `HISTOcumsum(h, out)`: the double running sum of the counts.
    pub fn histo_cumsum(
        &mut self,
        h: &mut Histogram,
        out: Output<'_, Histogram>,
    ) -> Result<Returned<Histogram>, Stop> {
        let counts: Vec<f32> = h.used().map(|b| h.count(b)).collect();
        let mut w = self.work_copy(h, &out)?;
        let mut s = 0.0f64;
        for (b, &c) in counts.iter().enumerate() {
            s = sse::add64(s, sse::widen(c));
            w.set_count(b as i32, sse::narrow(s));
        }
        Ok(put_back(w, out, h))
    }

    /// `HISTOclearBins(src, dst, lo, hi)`: the counts of the bins in `[lo, hi]` set to 0.
    pub fn histo_clear_bins(
        &mut self,
        src: &mut Histogram,
        dst: Output<'_, Histogram>,
        lo: i32,
        hi: i32,
    ) -> Result<Returned<Histogram>, Stop> {
        let mut w = self.work_copy(src, &dst)?;
        let (l, h) = (lo as f32, hi as f32);
        for b in w.used() {
            let v = w.bin(b);
            if v >= l && v <= h {
                w.set_count(b, 0.0);
            }
        }
        Ok(put_back(w, dst, src))
    }

    /// `HISTOabs(h, out)` (§5.3, §21; FS-50).
    pub fn histo_abs(
        &mut self,
        h: &mut Histogram,
        out: Output<'_, Histogram>,
    ) -> Result<Returned<Histogram>, Stop> {
        let src = h.clone();
        let (mut mx, mut mn) = (0.0f64, 1e10f64);
        for b in src.used() {
            let v = sse::widen(src.bin(b)).abs();
            if v > mx {
                mx = v;
            }
            if v < mn {
                mn = v;
            }
        }
        if mn >= mx {
            mn = sse::mul64(mx, 0.9);
        }
        let x = sse::add64(
            sse::div64(sse::sub64(mx, mn), sse::widen(src.bin_size)).ceil(),
            1.0,
        );
        let outside = !(x > -2147483649.0 && x < 2147483648.0);
        // nint's own rounding (x is an integer or not finite here)
        if outside && self.mode().fix() {
            self.note(
                &FS_50,
                "a bin count beyond the int range refused (no 2^31-bin allocation)",
            );
            let msg = format!(
                "HISTOabs: absolute values from {} to {} in bins of {} need {} bins: outside the int range (an infinite range, or a bin size of 0 or NaN)",
                larmorx_core::cfmt::f(mn, 6),
                larmorx_core::cfmt::f(mx, 6),
                larmorx_core::cfmt::f(f64::from(src.bin_size), 6),
                shortest(x)
            );
            self.error_exit(codes::BAD_PARM, msg.as_bytes())?;
            return Ok(Returned::Nothing);
        }
        if outside {
            self.note(
                &FS_50,
                "a bin count beyond the int range: the allocation of -2147483648 bins failed",
            );
        }
        let n = self.nint(x);
        let mut a = match &out {
            Output::New => self.histo_alloc(n)?,
            Output::Given(d) => {
                let mut d = (**d).clone();
                self.histo_realloc_in(&mut d, n)?;
                d
            }
            Output::First | Output::Second => {
                let mut d = src.clone();
                self.histo_realloc_in(&mut d, n)?;
                d
            }
        };
        a.bin_size = src.bin_size;
        a.min = sse::narrow(mn);
        a.max = sse::narrow(mx);
        for b in 0..n.max(0) {
            let v = sse::narrow(sse::add64(
                sse::widen(sse::mul32(b as f32, src.bin_size)),
                mn,
            ));
            a.set_bin(b, v);
        }
        // the counts of `h` as they are now: zeros when the output is `h` itself, reallocated
        let in_place = matches!(out, Output::First | Output::Second);
        for b in src.used() {
            let c = if in_place { a.count(b) } else { src.count(b) };
            let k = self.histo_find_bin(&a, src.bin(b).abs());
            let v = sse::add32(self.hc(&a, k), c);
            self.hset(&mut a, k, v);
        }
        Ok(put_back(a, out, h))
    }
}

/// The working histogram written back where the output belongs.
fn put_back(
    w: Histogram,
    dst: Output<'_, Histogram>,
    first: &mut Histogram,
) -> Returned<Histogram> {
    match dst {
        Output::New => Returned::New(w),
        Output::Given(d) => {
            *d = w;
            Returned::Output
        }
        Output::First | Output::Second => {
            *first = w;
            Returned::Output
        }
    }
}

/// The shortest decimal of a double as C's `%g` would not: Rust's shortest round-trip form.
fn shortest(x: f64) -> String {
    if x.is_nan() {
        "nan".into()
    } else if x.is_infinite() {
        if x > 0.0 { "inf".into() } else { "-inf".into() }
    } else {
        format!("{x}")
    }
}

/// Which cumulative distribution (§5.3).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cdf {
    Forward,
    Reverse,
}

impl Process {
    /// `HISTOmakeCDF(src, dst)` (§5.3): from the absolute-value histogram of `src`.
    pub fn histo_make_cdf(
        &mut self,
        src: &mut Histogram,
        dst: Output<'_, Histogram>,
    ) -> Result<Returned<Histogram>, Stop> {
        self.histo_cdf(src, dst, Cdf::Forward)
    }

    /// `HISTOmakeReverseCDF(src, dst)`.
    pub fn histo_make_reverse_cdf(
        &mut self,
        src: &mut Histogram,
        dst: Output<'_, Histogram>,
    ) -> Result<Returned<Histogram>, Stop> {
        self.histo_cdf(src, dst, Cdf::Reverse)
    }

    fn histo_cdf(
        &mut self,
        src: &mut Histogram,
        dst: Output<'_, Histogram>,
        kind: Cdf,
    ) -> Result<Returned<Histogram>, Stop> {
        let mut s0 = src.clone();
        let a = match self.histo_abs(&mut s0, Output::New)? {
            Returned::New(a) => a,
            _ => return Ok(Returned::Nothing),
        };
        let mut w = match &dst {
            Output::New => {
                let mut d = self.histo_alloc(src.nbins)?;
                self.copy_fields(src, &mut d);
                d
            }
            Output::Given(d) => (**d).clone(),
            Output::First | Output::Second => src.clone(),
        };
        let mut t = 0.0f64;
        let order: Vec<i32> = match kind {
            Cdf::Forward => w.used().collect(),
            Cdf::Reverse => w.used().rev().collect(),
        };
        let aorder: Vec<i32> = match kind {
            Cdf::Forward => a.used().collect(),
            Cdf::Reverse => a.used().rev().collect(),
        };
        for &b in &order {
            let v = w.bin(b);
            if v < 0.0 {
                w.set_count(b, 0.0);
                continue;
            }
            let mut s = 0.0f64;
            for &i in &aorder {
                let av = a.bin(i).abs();
                let take = match kind {
                    Cdf::Forward => av <= v,
                    Cdf::Reverse => av >= v,
                };
                if take {
                    s = sse::add64(s, sse::widen(a.count(i)));
                }
            }
            t = s;
            w.set_count(b, sse::narrow(s));
        }
        for b in w.used() {
            let v = sse::narrow(sse::div64(sse::widen(w.count(b)), t));
            w.set_count(b, v);
        }
        Ok(put_back(w, dst, src))
    }

    /// `HISTOsoapBubbleZeros(src, dst, n)` (§5.3, §21).
    pub fn histo_soap_bubble_zeros(
        &mut self,
        src: &mut Histogram,
        dst: Output<'_, Histogram>,
        n: i32,
    ) -> Result<Returned<Histogram>, Stop> {
        let s0 = src.clone();
        let mut w = self.work_copy(src, &dst)?;
        let nb = s0.nbins.max(0);
        let fixed: Vec<bool> = (0..nb).map(|b| s0.count(b) > 0.0).collect();
        if nb == 1 && !fixed[0] {
            self.note(&FS_22, "a one-bin histogram's missing neighbour read as 0");
        }
        for _ in 0..n.max(0) {
            let c: Vec<f32> = (0..nb + 1).map(|b| w.count(b)).collect();
            let mut next = c.clone();
            for b in 0..nb as usize {
                next[b] = if fixed[b] {
                    s0.count(b as i32)
                } else if b == 0 {
                    sse::mul32(sse::add32(c[0], c[1]), 0.5)
                } else if b == nb as usize - 1 {
                    sse::mul32(sse::add32(c[b], c[b - 1]), 0.5)
                } else {
                    sse::div32(sse::add32(sse::add32(c[b], c[b + 1]), c[b - 1]), 3.0)
                };
            }
            for b in 0..nb {
                w.set_count(b, next[b as usize]);
            }
        }
        Ok(put_back(w, dst, src))
    }

    /// `HISTOfillHoles(h)` (§5.3, §21): returns 0.
    pub fn histo_fill_holes(&mut self, h: &mut Histogram) -> i32 {
        let n = h.nbins;
        let filled: Vec<bool> = (0..n.max(0))
            .map(|b| b >= 1 && b <= n - 2 && h.count(b) > 0.0)
            .collect();
        let first = filled.iter().position(|&f| f);
        let last = filled.iter().rposition(|&f| f);
        if let (Some(f0), Some(l0)) = (first, last) {
            for _ in 0..101 {
                let mut most = 0.0f64;
                for (b, &f) in filled.iter().enumerate().take(l0).skip(f0 + 1) {
                    if f {
                        continue;
                    }
                    let b = b as i32;
                    let old = h.count(b);
                    let new = sse::mul32(sse::add32(h.count(b - 1), h.count(b + 1)), 0.5);
                    h.set_count(b, new);
                    let change = if old == 0.0 {
                        1.0
                    } else {
                        sse::div64(
                            sse::sub64(f64::from(new), f64::from(old)).abs(),
                            f64::from(old).abs(),
                        )
                    };
                    if change > most {
                        most = change;
                    }
                }
                if !(most > 0.01) {
                    break;
                }
            }
        }
        if n < 2 {
            self.note(
                &FS_22,
                "a histogram of fewer than 2 bins: the end bins' neighbours read as 0",
            );
        }
        if n >= 1 {
            if h.count(0) == 0.0 {
                let v = self.hc(h, 1);
                h.set_count(0, v);
            }
            if h.count(n - 1) == 0.0 {
                let v = self.hc(h, n - 2);
                self.hset(h, n - 1, v);
            }
        }
        0
    }

    // ---- §5.4: smoothing

    /// `HISTOsmooth(src, dst, sigma)` (§5.4, §21).
    pub fn histo_smooth(
        &mut self,
        src: &mut Histogram,
        dst: Output<'_, Histogram>,
        sigma: f32,
    ) -> Result<Returned<Histogram>, Stop> {
        let in_place = matches!(dst, Output::First | Output::Second);
        let mut w = match &dst {
            Output::New => {
                let mut d = self.histo_alloc(src.nbins)?;
                self.copy_fields(src, &mut d);
                d
            }
            Output::Given(d) => {
                let mut d = (**d).clone();
                if d.nbins < src.nbins {
                    self.histo_realloc_in(&mut d, src.nbins)?;
                }
                d
            }
            Output::First | Output::Second => src.clone(),
        };
        w.nbins = src.nbins;
        w.bin_size = src.bin_size;
        let k = self.smooth_kernel(sigma);
        let half = (k.len() as i32) / 2;
        let n = src.nbins;
        let s0 = src.clone();
        for b in 0..n.max(0) {
            let (mut norm, mut total) = (0.0f32, 0.0f32);
            for (x, &kx) in k.iter().enumerate() {
                let j = b + x as i32 - half;
                if j < 0 || j >= n {
                    continue;
                }
                let c = if in_place { w.count(j) } else { s0.count(j) };
                norm = sse::add32(norm, kx);
                total = sse::add32(total, sse::mul32(kx, c));
            }
            w.set_count(b, sse::div32(total, norm));
            w.set_bin(b, s0.bin(b));
        }
        if in_place {
            *src = w;
            return Ok(Returned::Output);
        }
        Ok(put_back(w, dst, src))
    }

    /// The smoothing kernel of §5.4, normalised to a float sum of 1 (empty for a negative
    /// length).
    fn smooth_kernel(&mut self, sigma: f32) -> Vec<f32> {
        let mut len = self.nint(f64::from(sse::mul32(8.0, sigma))) + 1;
        if len % 2 == 0 {
            len += 1;
        }
        if len > 2000 {
            len = 2000;
        }
        if len <= 0 {
            return Vec::new();
        }
        let half = len / 2;
        let two = sse::mul32(2.0, sigma);
        let four = sse::mul32(4.0, sigma);
        let e2 = sse::narrow(std::f64::consts::E * std::f64::consts::E);
        let c = sse::div32(1.0, sse::mul32(16.0, e2));
        let mut k = Vec::with_capacity(len as usize);
        for x in 0..len {
            let f = (x - half) as f32;
            let af = f.abs();
            let v = if af <= two {
                let q = sse::div32(sse::mul32(-f, f), sse::mul32(two, sigma));
                sse::narrow(self.libm(
                    "exp",
                    f64::from(q),
                    larmorx_core::math::exp,
                    larmorx_core::math::exp_glibc,
                ))
            } else if af <= four {
                let base = sse::sub64(4.0, sse::div64(f64::from(af), sse::widen(sigma)));
                sse::mul32(c, sse::narrow(self.libm_pow(base, 4.0)))
            } else {
                0.0
            };
            k.push(v);
        }
        let mut s = 0.0f32;
        for &v in &k {
            s = sse::add32(s, v);
        }
        for v in &mut k {
            *v = sse::div32(*v, s);
        }
        k
    }

    // ---- §5.5: searches

    /// The float maximum of the counts from 0.
    fn largest(&mut self, h: &Histogram) -> f32 {
        let mut m = 0.0f32;
        for b in h.used() {
            let c = h.count(b);
            if c > m {
                m = c;
            }
        }
        m
    }

    /// Whether no bin of `[b - whalf, b + whalf]` inside the histogram exceeds bin `b`.
    fn is_peak_clipped(&mut self, h: &Histogram, b: i32, whalf: i32) -> bool {
        let c = h.count(b);
        for i in (b - whalf)..=(b + whalf) {
            if i < 0 || i >= h.nbins {
                continue;
            }
            if h.count(i) > c {
                return false;
            }
        }
        true
    }

    /// `HISTOfindLastPeak(h, wsize, pct)`.
    pub fn histo_find_last_peak(&mut self, h: &Histogram, wsize: i32, pct: f32) -> i32 {
        let m = self.largest(h);
        if m == 0.0 {
            return -1;
        }
        let t = sse::mul32(pct, m);
        let w = half(wsize);
        for b in h.used().rev() {
            if !(h.count(b) <= t) && self.is_peak_clipped(h, b, w) {
                return b;
            }
        }
        -1
    }

    /// `HISTOfindFirstPeak(h, wsize, pct)`.
    pub fn histo_find_first_peak(&mut self, h: &Histogram, wsize: i32, pct: f32) -> i32 {
        let m = self.largest(h);
        if m == 0.0 {
            return -1;
        }
        let t = sse::mul32(pct, m);
        let w = half(wsize);
        for b in h.used() {
            if !(h.count(b) <= t) && self.is_peak_clipped(h, b, w) {
                return b;
            }
        }
        -1
    }

    /// The region's bins of intensities `i0`, `i1` (§5.5).
    fn region_bins(&mut self, h: &Histogram, i0: i32, i1: i32) -> (i32, i32) {
        let mut b0 = -1;
        let mut b1 = -1;
        for b in h.used() {
            if h.bin(b) >= i0 as f32 {
                b0 = if b == 0 { 0 } else { b - 1 };
                break;
            }
        }
        for b in h.used() {
            if h.bin(b) >= i1 as f32 {
                b1 = if b == h.nbins - 1 { b } else { b + 1 };
                break;
            }
        }
        (b0, b1)
    }

    /// `HISTOfindValley(h, wsize, i0, i1)`: counts compared as integers (FS-17).
    pub fn histo_find_valley(&mut self, h: &Histogram, wsize: i32, i0: i32, i1: i32) -> i32 {
        let ints: Vec<i32> = h
            .used()
            .map(|b| h.count(b))
            .collect::<Vec<f32>>()
            .into_iter()
            .map(|c| self.to_int(f64::from(c)))
            .collect();
        let mut m = 0;
        for &c in &ints {
            if c > m {
                m = c;
            }
        }
        if m == 0 {
            return -1;
        }
        let (mut b0, mut b1) = self.region_bins(h, i0, i1);
        if b0 < 0 {
            b0 = 0;
        }
        if b1 == -1 || b1 >= h.nbins {
            b1 = h.nbins - 1;
        }
        let w = half(wsize);
        for b in b0..=b1 {
            let c = ints[b as usize];
            let mut valley = true;
            for i in (b - w)..=(b + w) {
                if i < 0 || i >= h.nbins {
                    continue;
                }
                if ints[i as usize] < c {
                    valley = false;
                    break;
                }
            }
            if valley {
                return b;
            }
        }
        -1
    }

    /// `HISTOfindLastPeakInRegion(h, wsize, pct, i0, i1)`.
    pub fn histo_find_last_peak_in_region(
        &mut self,
        h: &Histogram,
        wsize: i32,
        pct: f32,
        i0: i32,
        i1: i32,
    ) -> i32 {
        let m = self.largest(h);
        let (b0, mut b1) = self.region_bins(h, i0, i1);
        if b1 == -1 {
            b1 = h.nbins - 1;
        }
        let w = half(wsize);
        let t = sse::mul32(pct, m);
        let mut b = b1;
        while b >= b0 {
            let c = self.hc(h, b);
            let mut sum = 0.0f32;
            let mut peak = true;
            for i in (b - w)..=(b + w) {
                if i < 0 || i >= h.nbins {
                    continue;
                }
                let ci = h.count(i);
                if ci > c {
                    peak = false;
                    break;
                }
                sum = sse::add32(sum, ci);
            }
            if peak {
                let avg = sse::div32(sum, wsize as f32);
                if avg >= t && f64::from(sse::sub32(c, avg)) >= 1.9 {
                    return b;
                }
            }
            b -= 1;
        }
        -1
    }

    /// `HISTOfindHighestPeakInRegion(h, b0, b1)`.
    pub fn histo_find_highest_peak_in_region(&mut self, h: &Histogram, b0: i32, b1: i32) -> i32 {
        let b0 = b0.max(0);
        let b1 = if b1 >= h.nbins { h.nbins - 1 } else { b1 };
        let mut m = 0.0f32;
        let mut peak = -1;
        for b in b0..=b1 {
            let c = self.hc(h, b);
            if c > m {
                m = c;
                peak = b;
            }
        }
        peak
    }

    /// `HISTOfindPreviousValley(h, b0)`.
    pub fn histo_find_previous_valley(&mut self, h: &Histogram, b0: i32) -> i32 {
        if b0 <= 0 {
            return b0;
        }
        let mut prev = self.hc(h, b0);
        let mut b = b0 - 1;
        while b >= 0 {
            let c = self.hc(h, b);
            if c > prev {
                return b;
            }
            prev = c;
            b -= 1;
        }
        -1
    }

    /// `HISTOfindNextValley(h, b0)`.
    pub fn histo_find_next_valley(&mut self, h: &Histogram, b0: i32) -> i32 {
        if b0 >= h.nbins {
            return b0;
        }
        let mut prev = self.hc(h, b0);
        for b in b0 + 1..h.nbins {
            let c = self.hc(h, b);
            if c > prev {
                return b;
            }
            prev = c;
        }
        -1
    }

    /// Whether no bin of the unclipped window `[b - whalf, b + whalf]` exceeds bin `b` (FS-22
    /// outside).
    fn is_peak_unclipped(&mut self, h: &Histogram, b: i32, whalf: i32) -> bool {
        let c = self.hc(h, b);
        for i in (b - whalf)..=(b + whalf) {
            if self.hc(h, i) > c {
                return false;
            }
        }
        true
    }

    /// `HISTOfindNextPeak(h, b0, whalf)`.
    pub fn histo_find_next_peak(&mut self, h: &Histogram, b0: i32, whalf: i32) -> i32 {
        if b0 > h.nbins - 2 {
            return b0;
        }
        let mut b = b0 + 1;
        while b <= h.nbins - whalf {
            if self.is_peak_unclipped(h, b, whalf) {
                return b;
            }
            b += 1;
        }
        -1
    }

    /// `HISTOfindPreviousPeak(h, b0, whalf)`.
    pub fn histo_find_previous_peak(&mut self, h: &Histogram, b0: i32, whalf: i32) -> i32 {
        if b0 > h.nbins - 2 {
            return b0;
        }
        let mut b = b0 - 1;
        while b >= whalf {
            if self.is_peak_unclipped(h, b, whalf) {
                return b;
            }
            b -= 1;
        }
        -1
    }

    /// `HISTOfindStartOfPeak(h, b0, pct)`.
    pub fn histo_find_start_of_peak(&mut self, h: &Histogram, b0: i32, pct: f32) -> i32 {
        let v = self.histo_find_previous_valley(h, b0);
        if b0 <= 0 {
            return b0;
        }
        let t = sse::mul32(self.hc(h, b0), pct);
        let mut b = b0 - 1;
        while b >= v {
            if self.hc(h, b) < t {
                return b;
            }
            b -= 1;
        }
        v - 1
    }

    /// `HISTOfindEndOfPeak(h, b0, pct)`.
    pub fn histo_find_end_of_peak(&mut self, h: &Histogram, b0: i32, pct: f32) -> i32 {
        let v = self.histo_find_next_valley(h, b0);
        if b0 >= h.nbins {
            return b0;
        }
        let t = sse::mul32(self.hc(h, b0), pct);
        for b in b0 + 1..=v {
            if self.hc(h, b) < t {
                return b;
            }
        }
        h.nbins - 1
    }

    /// `HISTOfindCurrentPeak(h, b0, wsize, pct)`.
    pub fn histo_find_current_peak(&mut self, h: &Histogram, b0: i32, wsize: i32, pct: f32) -> i32 {
        let top = self.histo_find_highest_peak_in_region(h, 0, h.nbins);
        if top < 0 {
            return -1;
        }
        let minc = sse::mul32(pct, h.count(top));
        let w = half(wsize);
        // a start beyond the last bin starts at the last bin (measured: `hsearch`)
        let b0 = b0.min(h.nbins - 1);
        let (mut before, mut after) = (0.0f32, 0.0f32);
        for i in b0 - w..b0 {
            if i >= 0 && i < h.nbins {
                before = sse::add32(before, h.count(i));
            }
        }
        for i in b0 + 1..=b0 + w {
            if i >= 0 && i < h.nbins {
                after = sse::add32(after, h.count(i));
            }
        }
        if after > before {
            let mut b = (b0 - w).max(0);
            while b < h.nbins {
                let c = h.count(b);
                if !(c < minc) {
                    let mut peak = true;
                    for i in b..=b + w {
                        if i < h.nbins && h.count(i) > c {
                            peak = false;
                            break;
                        }
                    }
                    if peak {
                        return b;
                    }
                }
                b += 1;
            }
        } else {
            let mut b = b0;
            while b >= 0 {
                let c = self.hc(h, b);
                if !(c < minc) && self.is_peak_clipped(h, b, w) {
                    return b;
                }
                b -= 1;
            }
        }
        -1
    }

    /// `HISTOfindBin(h, v)` (§5.5): FS-17 for a bin size of exactly 1.
    pub fn histo_find_bin(&mut self, h: &Histogram, v: f32) -> i32 {
        if h.bin_size == 1.0 {
            let iv = self.to_int(f64::from(v));
            let d = sse::sub32(iv as f32, self.hb(h, 0));
            return self.to_int(f64::from(d));
        }
        let mut b = h.nbins - 1;
        while b >= 1 {
            if h.bin(b - 1) < v {
                return b;
            }
            b -= 1;
        }
        0
    }

    /// `HISTOgetCount(h, v)`: the count of `v`'s bin, 0 outside the bins in use.
    pub fn histo_get_count(&mut self, h: &Histogram, v: f32) -> f64 {
        let b = self.histo_find_bin(h, v);
        if b < 0 || b >= h.nbins {
            return 0.0;
        }
        sse::widen(h.count(b))
    }

    /// `HISTOfindBinWithCount(h, v)`: the first bin whose `|count - v|` is smallest.
    pub fn histo_find_bin_with_count(&mut self, h: &Histogram, v: f32) -> i32 {
        let mut best = 0;
        let mut d = f32::INFINITY;
        let mut first = true;
        for b in h.used() {
            let x = sse::sub32(h.count(b), v).abs();
            if first || x < d {
                d = x;
                best = b;
                first = false;
            }
        }
        best
    }

    /// `HISTOvalToBin(h, v)`: the first bin whose `|value - v|` (double) is smallest.
    pub fn histo_val_to_bin(&mut self, h: &Histogram, v: f64) -> i32 {
        let mut best = 0;
        let mut d = 0.0f64;
        let mut first = true;
        for b in h.used() {
            let x = sse::sub64(sse::widen(h.bin(b)), v).abs();
            if first || x < d {
                d = x;
                best = b;
                first = false;
            }
        }
        best
    }

    /// `HISTOcount(h, samples)`: each sample adds 1 at its value's bin; returns 0.
    pub fn histo_count(&mut self, h: &mut Histogram, samples: &[f64]) -> i32 {
        for &s in samples {
            let b = self.histo_val_to_bin(h, s);
            let v = sse::add32(self.hc(h, b), 1.0);
            self.hset(h, b, v);
        }
        0
    }
}

/// The fit's scale (§5.7 step 8): `(Σ counts[b] / g_b) / n` over the bins whose
/// `g_b = (1 / r) e_b` is above 1e-3 (`e` from bin `lo`).
fn gauss_scale(z: &Histogram, e: &[f64], r: f64, lo: i32) -> f64 {
    let (mut sum, mut cnt) = (0.0f64, 0i32);
    for (k, &eb) in e.iter().enumerate() {
        let gb = sse::mul64(sse::div64(1.0, r), eb);
        if gb > 1e-3 {
            sum = sse::add64(sum, sse::div64(sse::widen(z.count(lo + k as i32)), gb));
            cnt += 1;
        }
    }
    sse::div64(sum, f64::from(cnt))
}

/// A two-dimensional histogram, for `HISTO2DfindBin1`, `HISTO2DfindBin2` and `HISTO2DgetCount`
/// (§5.5): its bin counts, bin sizes, minima, bin values and counts (`counts[b1][b2]`).
#[derive(Clone, Debug, PartialEq)]
pub struct Histogram2d {
    pub nbins1: i32,
    pub nbins2: i32,
    pub bin_size1: f32,
    pub bin_size2: f32,
    pub min1: f32,
    pub min2: f32,
    pub bins1: Vec<f32>,
    pub bins2: Vec<f32>,
    pub counts: Vec<Vec<f32>>,
}

impl Histogram2d {
    /// A histogram of zeros (`HISTO2Dalloc`).
    pub fn new(n1: i32, n2: i32) -> Histogram2d {
        let (a, b) = (n1.max(0) as usize, n2.max(0) as usize);
        Histogram2d {
            nbins1: n1,
            nbins2: n2,
            bin_size1: 0.0,
            bin_size2: 0.0,
            min1: 0.0,
            min2: 0.0,
            bins1: vec![0.0; a],
            bins2: vec![0.0; b],
            counts: vec![vec![0.0; b]; a],
        }
    }
}

impl Process {
    /// Find bin on one axis of a two-dimensional histogram (§5.5).
    fn find_bin_axis(&mut self, bins: &[f32], n: i32, size: f32, min: f32, v: f32) -> i32 {
        if size == 1.0 {
            let iv = self.to_int(f64::from(v));
            let d = sse::sub32(iv as f32, min);
            return self.to_int(f64::from(d));
        }
        let mut b = n - 1;
        while b >= 1 {
            if bins[b as usize - 1] < v {
                return b;
            }
            b -= 1;
        }
        0
    }

    /// `HISTO2DfindBin1(h, v)`.
    pub fn histo2d_find_bin1(&mut self, h: &Histogram2d, v: f32) -> i32 {
        self.find_bin_axis(&h.bins1, h.nbins1, h.bin_size1, h.min1, v)
    }

    /// `HISTO2DfindBin2(h, v)`.
    pub fn histo2d_find_bin2(&mut self, h: &Histogram2d, v: f32) -> i32 {
        self.find_bin_axis(&h.bins2, h.nbins2, h.bin_size2, h.min2, v)
    }

    /// `HISTO2DgetCount(h, v1, v2)`: the count of the bins (double), 0 when either is outside.
    pub fn histo2d_get_count(&mut self, h: &Histogram2d, v1: f32, v2: f32) -> f64 {
        let b1 = self.histo2d_find_bin1(h, v1);
        let b2 = self.histo2d_find_bin2(h, v2);
        if b1 < 0 || b1 >= h.nbins1 || b2 < 0 || b2 >= h.nbins2 {
            return 0.0;
        }
        sse::widen(h.counts[b1 as usize][b2 as usize])
    }

    // ---- §5.6: samples, totals, statistics

    /// `HISTOaddSample(h, v, lo, hi)` (§5.6; FS-12, FS-19, FS-20): returns the bin number.
    pub fn histo_add_sample(&mut self, h: &mut Histogram, v: f32, lo: f32, hi: f32) -> i32 {
        let (lo, size) = if lo >= hi {
            (h.min, h.bin_size)
        } else {
            (
                lo,
                sse::div32(sse::sub32(hi, lo), sse::sub32(h.nbins as f32, 1.0)),
            )
        };
        let x = sse::div32(sse::sub32(v, lo), size);
        let bin = self.nint(f64::from(x));
        let reproduce = self.mode().reproduce();
        if x.is_nan() && !reproduce {
            self.note(&FS_19, "a sample at a NaN position not counted");
            return bin;
        }
        if bin >= 0 && bin < h.nbins {
            let xd = f64::from(x);
            let dp = sse::sub64(xd, xd.floor());
            let dn = sse::sub64(xd.ceil(), xd);
            let mut bp = self.to_int(xd.floor());
            if bp < 0 {
                bp = 0;
            }
            let mut bn = self.to_int(xd.ceil());
            if bn >= h.nbins {
                bn = h.nbins - 1;
            }
            if dp == 0.0 && dn == 0.0 {
                if reproduce {
                    self.note(&FS_20, "a sample exactly at a bin's position not counted");
                } else {
                    self.note(
                        &FS_20,
                        "a sample exactly at a bin's position counted with weight 1",
                    );
                    let c = sse::add32(h.count(bn), 1.0);
                    h.set_count(bn, c);
                    return bin;
                }
            }
            let c = sse::narrow(sse::add64(sse::widen(h.count(bn)), dp));
            h.set_count(bn, c);
            let c = sse::narrow(sse::add64(sse::widen(h.count(bp)), dn));
            h.set_count(bp, c);
            return bin;
        }
        // outside the bins: FreeSurfer compares the bin number with the lower intensity
        let fs_target = if (bin as f32) < lo {
            Some(0)
        } else if bin >= h.nbins {
            Some(h.nbins - 1)
        } else {
            None
        };
        let fix_target = if bin < 0 { Some(0) } else { Some(h.nbins - 1) };
        let target = if reproduce { fs_target } else { fix_target };
        if fs_target != fix_target {
            if reproduce {
                self.note(&FS_19, "a sample outside the histogram placed by its bin number against the lower intensity");
            } else {
                self.note(
                    &FS_19,
                    "a sample outside the histogram counted in the nearest end bin",
                );
            }
        }
        if let Some(t) = target {
            let c = sse::add32(self.hc(h, t), 1.0);
            self.hset(h, t, c);
        }
        bin
    }

    /// `HISTOtotalInRegion(h, b0, b1)`: the float sum of counts `b0..b1` (§21's order).
    pub fn histo_total_in_region(&mut self, h: &Histogram, b0: i32, b1: i32) -> f32 {
        let mut terms = Vec::new();
        let mut b = b0;
        while b <= b1 {
            terms.push(self.hc(h, b));
            b += 1;
        }
        total_sum(&terms)
    }

    /// `HISTOtotal(h)`: the region `[0, nbins - 1]`.
    pub fn histo_total(&mut self, h: &Histogram) -> f32 {
        self.histo_total_in_region(h, 0, h.nbins - 1)
    }

    /// `HISTOclearZeroBin(h)`: returns 0.
    pub fn histo_clear_zero_bin(&mut self, h: &mut Histogram) -> i32 {
        if self.hb(h, 0) > h.bin_size {
            return 0;
        }
        let mut b = 0;
        while b < h.nbins {
            if h.bin(b) >= 0.0 {
                break;
            }
            b += 1;
        }
        self.hset(h, b, 0.0);
        0
    }

    /// `HISTOthreshSum(mask, src, t)`: the float sum of src's counts at the mask's bins whose
    /// count is above `t`.
    pub fn histo_thresh_sum(&mut self, mask: &Histogram, src: &Histogram, t: f32) -> f32 {
        let mut s = 0.0f32;
        for b in mask.used() {
            if mask.count(b) > t {
                let c = self.hc(src, b);
                s = sse::add32(s, c);
            }
        }
        s
    }

    /// `HISTOfindMedian(h)` (§5.6).
    pub fn histo_find_median(&mut self, h: &Histogram) -> f64 {
        let mut t = 0.0f64;
        for b in h.used() {
            t = sse::add64(t, sse::widen(h.count(b)));
        }
        let half = sse::div64(t, 2.0);
        let mut r = 0.0f64;
        for b in h.used() {
            let c = sse::widen(h.count(b));
            let next = sse::add64(r, c);
            if r < half && next > half {
                if sse::sub64(half, r) > sse::sub64(next, half) || b == 0 {
                    return sse::widen(h.bin(b));
                }
                return sse::widen(h.bin(b - 1));
            }
            r = next;
        }
        0.0
    }

    /// `HISTOgetEntropy(h)` (§5.6, §21).
    pub fn histo_get_entropy(&mut self, h: &Histogram) -> f64 {
        let t = sse::widen(self.histo_total(h));
        let mut e = 0.0f64;
        for b in h.used() {
            let c = h.count(b);
            if c > 0.0 {
                let p = sse::div64(sse::widen(c), t);
                let lp = self.libm(
                    "log",
                    p,
                    larmorx_core::math::log,
                    larmorx_core::math::log_glibc,
                );
                e = sse::sub64(e, sse::mul64(p, lp));
            }
        }
        e
    }

    /// `HISTOcorrelate(h1, h2)` (§5.6, §21).
    pub fn histo_correlate(&mut self, h1: &Histogram, h2: &Histogram) -> Result<f64, Stop> {
        if h2.nbins > 256 {
            self.error_exit(
                codes::UNSUPPORTED,
                b"histoComputeLinearFitCorrelation: only 256 bins allowed",
            )?;
        }
        let (mut s, mut n1, mut n2) = (0.0f64, 0.0f64, 0.0f64);
        for b in h1.used() {
            let c1 = sse::widen(h1.count(b));
            let k = self.histo_find_bin(h2, h1.bin(b));
            let c2 = if k >= 0 && k < h2.nbins {
                sse::widen(h2.count(k))
            } else {
                0.0
            };
            s = sse::add64(sse::mul64(c2, c1), s);
            n1 = sse::add64(sse::mul64(c1, c1), n1);
            n2 = sse::add64(sse::mul64(c2, c2), n2);
        }
        let eps = f64::from(f32::EPSILON);
        if n1.abs() < eps {
            n1 = 1.0;
        }
        if n2.abs() < eps {
            n2 = 1.0;
        }
        Ok(sse::div64(s, sse::mul64(sse::sqrt64(n2), sse::sqrt64(n1))))
    }

    /// The linear-fit correlation of `h1` and `h2` at (a, b) (§5.6, §21).
    fn linear_fit_correlation(&mut self, h1: &Histogram, h2: &Histogram, a: f64, b: f64) -> f64 {
        let mut s = 0.0f64;
        for b1 in h1.used() {
            let c1 = sse::widen(h1.count(b1));
            let k = self.nint(sse::add64(sse::mul64(f64::from(b1), a), b));
            let c2 = if k >= 0 && k < h2.nbins {
                sse::widen(h2.count(k))
            } else {
                0.0
            };
            s = sse::add64(sse::mul64(c1, c2), s);
        }
        for b2 in h2.used() {
            let c2 = sse::widen(h2.count(b2));
            let k = self.nint(sse::div64(sse::sub64(f64::from(b2), b), a));
            let c1 = if k >= 0 && k < h1.nbins {
                sse::widen(h1.count(k))
            } else {
                0.0
            };
            s = sse::add64(sse::mul64(c2, c1), s);
        }
        s
    }

    /// `HISTOfindLinearFit(h1, h2, a0, a1, b0, b1)`: the best correlation and its (a, b), `None`
    /// when no step improved on (1, 0).
    pub fn histo_find_linear_fit(
        &mut self,
        h1: &Histogram,
        h2: &Histogram,
        a0: f64,
        a1: f64,
        b0: f64,
        b1: f64,
    ) -> (f64, Option<(f32, f32)>) {
        let mut best = self.linear_fit_correlation(h1, h2, 1.0, 0.0);
        let mut at = Some((1.0f32, 0.0f32));
        // C's MAX(MIN(a0, (a1 - a0) / 1000), 0.01)
        let astep = {
            let d = sse::div64(sse::sub64(a1, a0), 1000.0);
            let m = if a0 < d { a0 } else { d };
            if m > 0.01 { m } else { 0.01 }
        };
        let bstep = {
            let m = sse::div64(sse::sub64(b1, b0), 1000.0);
            if m > 0.01 { m } else { 0.01 }
        };
        let mut a = a0;
        while a <= a1 {
            let mut b = b0;
            while b <= b1 {
                let c = self.linear_fit_correlation(h1, h2, a, b);
                if c > best {
                    best = c;
                    at = Some((sse::narrow(a), sse::narrow(b)));
                }
                b = sse::add64(b, bstep);
            }
            a = sse::add64(a, astep);
        }
        (best, at)
    }

    /// `HISTOcomputeFWHM(h, peak)` (§5.6).
    pub fn histo_compute_fwhm(&mut self, h: &Histogram, peak: i32) -> f32 {
        let t = sse::div32(self.hc(h, peak), 2.0);
        let w_max = peak.min(h.nbins - peak - 1);
        let mut w = 1;
        while w < w_max {
            if self.hc(h, peak - w) < t && self.hc(h, peak + w) < t {
                break;
            }
            w += 1;
        }
        sse::mul32((2 * w) as f32, h.bin_size)
    }
}

/// The result of `HISTOrobustGaussianFit` (§5.7): its status, and the offset and σ when set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RobustFit {
    pub status: i32,
    pub offset: Option<f64>,
    pub sigma: Option<f64>,
}

impl Process {
    /// `HISTOrobustGaussianFit(h, q)` (§5.7; FS-24, FS-51).
    pub fn histo_robust_gaussian_fit(
        &mut self,
        h: &mut Histogram,
        q: f64,
    ) -> Result<RobustFit, Stop> {
        if !(h.bin_size > 0.0) {
            h.bin_size = 1.0;
        }
        let sigma_s = sse::narrow(sse::div64(2.0, sse::widen(h.bin_size)));
        let mut h0 = h.clone();
        let s = match self.histo_smooth(&mut h0, Output::New, sigma_s)? {
            Returned::New(s) => s,
            _ => {
                return Ok(RobustFit {
                    status: codes::BAD_PARM,
                    offset: None,
                    sigma: None,
                });
            }
        };
        let p = self.histo_find_highest_peak_in_region(&s, 0, s.nbins);
        let m = sse::widen(self.hb(&s, p));
        let mut h1 = h.clone();
        let Returned::New(z) =
            self.histo_linear_scale(&mut h1, Output::New, 1.0, sse::narrow(-m))?
        else {
            unreachable!("a new histogram")
        };
        let mut z1 = z.clone();
        let Returned::New(a) = self.histo_abs(&mut z1, Output::New)? else {
            return Ok(RobustFit {
                status: codes::BAD_PARM,
                offset: None,
                sigma: None,
            });
        };
        let mut z2 = z.clone();
        let Returned::New(c) = self.histo_make_cdf(&mut z2, Output::New)? else {
            return Ok(RobustFit {
                status: codes::BAD_PARM,
                offset: None,
                sigma: None,
            });
        };
        let plots = self.gdiag & DIAG_WRITE != 0 && self.gdiag & DIAG_VERBOSE != 0;
        if plots {
            self.histo_plot(&a, b"ha.plt");
            self.histo_plot(&c, b"hc.plt");
            self.histo_plot(h, b"h.plt");
            self.histo_plot(&s, b"hs.plt");
            self.histo_plot(&z, b"hz.plt");
        }
        let start = sse::add32(p as f32, sse::div32(10.0, s.bin_size));
        let start = self.to_int(f64::from(start));
        let mut v = self.histo_find_next_valley(&s, start);
        let theta = if v < 0 {
            v = self.histo_find_bin_with_count(&c, sse::narrow(q));
            c.bin(v)
        } else {
            let cp = sse::widen(self.hc(h, p));
            while sse::widen(self.hc(h, v)) < sse::mul64(cp, q) && v > p + 1 {
                v -= 1;
            }
            sse::sub32(self.hb(h, v), self.hb(h, p))
        };
        if z.nbins <= 0 {
            self.error_printf(codes::BAD_PARM, b"HISTOrobustGaussianFit: 0 length cdf");
            return Ok(RobustFit {
                status: codes::BAD_PARM,
                offset: Some(m),
                sigma: None,
            });
        }
        let mut lo = self.histo_find_bin(&z, -theta);
        let mut hi = self.histo_find_bin(&z, theta);
        let n = z.nbins;
        while lo >= hi {
            let (l, u) = ((lo - 1).max(0), (hi + 1).min(n - 1));
            if l == lo && u == hi {
                self.note(
                    &FS_24,
                    "the fit's window could not grow (one bin): the loop stopped",
                );
                break;
            }
            lo = l;
            hi = u;
        }
        if hi - lo < 12 {
            hi += (12 - (hi - lo)) / 2;
            lo -= (12 - (hi - lo)) / 2;
            lo = lo.max(0);
            hi = hi.min(n - 1);
        }
        let delta = sse::widen(sse::div32(h.bin_size, 10.0));
        let sigma_max = sse::widen(sse::div32(sse::mul32(a.nbins as f32, a.bin_size), 2.0));
        let root = sse::sqrt64(2.0 * std::f64::consts::PI);
        let mut best: Option<(f64, f64)> = None;
        let mut sigma = delta;
        if sigma <= sigma_max && !(sse::add64(sigma, delta) > sigma) {
            self.note(&FS_51, "a bin size that gives no search over sigma: an error instead of a run that never ends");
            let msg = format!(
                "HISTOrobustGaussianFit: the bin size {} gives no search over sigma (steps of {})",
                larmorx_core::cfmt::f(sse::widen(h.bin_size), 6),
                larmorx_core::cfmt::f(delta, 6)
            );
            self.error_printf(codes::BAD_PARM, msg.as_bytes());
            return Ok(RobustFit {
                status: codes::BAD_PARM,
                offset: Some(m),
                sigma: None,
            });
        }
        while sigma <= sigma_max {
            let r = sse::mul64(sigma, root);
            let two_ss = sse::mul64(sse::mul64(2.0, sigma), sigma);
            let g = self.gauss_exps(&z, 0.0, two_ss, lo, hi);
            let scale = gauss_scale(&z, &g, r, lo);
            let mut err = 0.0f64;
            for b in lo..=hi {
                let gb = g[(b - lo) as usize];
                let d = sse::sub64(sse::mul64(sse::div64(scale, r), gb), sse::widen(z.count(b)));
                err = sse::add64(err, sse::mul64(d, d));
            }
            if best.is_none_or(|(e, _)| err < e) {
                best = Some((err, sigma));
            }
            sigma = sse::add64(sigma, delta);
        }
        if plots && let Some((_, sg)) = best {
            // the fitted curve, its scale recomputed with z - m, over Z's bins
            let r = sse::mul64(sg, root);
            let two_ss = sse::mul64(sse::mul64(2.0, sg), sg);
            let g = self.gauss_exps(&z, m, two_ss, lo, hi);
            let scale = gauss_scale(&z, &g, r, lo);
            let all = self.gauss_exps(&z, m, two_ss, 0, z.nbins - 1);
            let mut fit = z.clone();
            for b in z.used() {
                let v = sse::narrow(sse::mul64(sse::div64(scale, r), all[b as usize]));
                fit.set_count(b, v);
            }
            self.histo_plot(&fit, b"hp.plt");
        }
        Ok(RobustFit {
            status: 0,
            offset: Some(m),
            sigma: best.map(|(_, s)| s),
        })
    }

    /// `exp(-z z / two_ss)` for Z's bins `lo..=hi`, `z` the bin value less `shift`.
    fn gauss_exps(&mut self, z: &Histogram, shift: f64, two_ss: f64, lo: i32, hi: i32) -> Vec<f64> {
        let mut g = Vec::new();
        for b in lo..=hi {
            let zv = sse::sub64(sse::widen(z.bin(b)), shift);
            let x = sse::div64(sse::mul64(-zv, zv), two_ss);
            g.push(self.libm(
                "exp",
                x,
                larmorx_core::math::exp,
                larmorx_core::math::exp_glibc,
            ));
        }
        g
    }

    // ---- §5.8: files

    /// `HISTOplot(h, name)`: returns 0, or -1 when the file cannot be opened.
    pub fn histo_plot(&mut self, h: &Histogram, name: &[u8]) -> i32 {
        let Ok(mut f) = std::fs::File::create(crate::files::path_of(name)) else {
            return -1;
        };
        let first = h.used().find(|&b| h.count(b) > 0.0);
        let last = h.used().rev().find(|&b| h.count(b) > 0.0);
        let mut text = Vec::new();
        if let (Some(b0), Some(b1)) = (first, last) {
            for b in b0..=b1.max(b0) {
                text.extend_from_slice(
                    format!(
                        "{}  {}\n",
                        larmorx_core::cfmt::f(sse::widen(h.bin(b)), 6),
                        larmorx_core::cfmt::pad(
                            &larmorx_core::cfmt::f(sse::widen(h.count(b)), 10),
                            10
                        )
                    )
                    .as_bytes(),
                );
            }
        }
        use std::io::Write;
        let _ = f.write_all(&text);
        0
    }

    /// `HISTOwriteTxt(h, name)`: returns 0, or 1 after its line when the file cannot be opened.
    pub fn histo_write_txt(&mut self, h: &Histogram, name: &[u8]) -> i32 {
        let Ok(mut f) = std::fs::File::create(crate::files::path_of(name)) else {
            let mut b = b"ERROR: HISTOwrite(): opening ".to_vec();
            b.extend_from_slice(name);
            b.push(b'\n');
            self.put_out(&b);
            return 1;
        };
        let mut text = Vec::new();
        for b in h.used() {
            text.extend_from_slice(
                format!(
                    "{:>4} {} {}\n",
                    b,
                    larmorx_core::cfmt::pad(&larmorx_core::cfmt::f(sse::widen(h.bin(b)), 5), 10),
                    larmorx_core::cfmt::pad(&larmorx_core::cfmt::f(sse::widen(h.count(b)), 5), 10)
                )
                .as_bytes(),
            );
        }
        use std::io::Write;
        let _ = f.write_all(&text);
        0
    }
}
