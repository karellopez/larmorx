// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_fill`'s topological correction (queue item 3.2-34, `mri_fill/mri_fill.cpp` 199-1846: the
//! option `-topofix <norm>`): `mriRemoveEdgeConfiguration`, `mriRemoveCornerConfiguration`,
//! `mriRemoveBackgroundCornerConfiguration` with a forbidden label (the other hemisphere's), and
//! `mri_topofix`, which runs them for each hemisphere's label and then looks at the background's
//! connected components; with `findMinSize` and `verifyLRSplit` (1847-1946).
//!
//! The removals are `mri_pretess`'s ([`crate::mri_pretess::tess`]: the same directions, loops and
//! order) with two changes: where a choice would set a voxel of the forbidden label, the other
//! voxel is set (a corner's paths through it are skipped, all six forbidden: none is), and each
//! pass reports `(PBM=<n>)`, FreeSurfer's flag of a choice it could not make without the forbidden
//! label (FS-1289: the flag is almost never raised, kept). The segmentation is a byte volume's
//! frame 0, read and written in place in FreeSurfer's order.

use std::io::Write;

use larmorx_core::defects::{FS_1286, FS_1287, FS_1288, FS_1289, FS_1290};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, NO_ERROR};
use larmorx_freesurfer_filter::segment::{mri_segment, mri_segment_free, mri_segment_max};
use larmorx_freesurfer_mri::lta::Lta;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_volume::talairachex::mri_voxel_to_talairach_ex;

/// x86-64's default NaN (sign bit set), the quotient `0.0 / 0.0` of FreeSurfer's build.
const DEFAULT_NAN: u64 = 0xfff8_0000_0000_0000;

/// `(x, y, z)` offsets.
type Off = (i32, i32, i32);

/// An edge direction: its tag, the diagonal neighbour and the two voxels between them (the first
/// chosen when brighter).
type Edge<'t> = (&'t str, Off, Off, Off);

fn add(p: Off, q: Off) -> Off {
    (p.0 + q.0, p.1 + q.1, p.2 + q.2)
}

/// The start and end (exclusive) of a loop that reads one voxel further by `step` (-1, 0, 1).
fn span(n: i32, step: i32) -> (i32, i32) {
    (step.min(0).abs(), n - step.max(0))
}

/// The three removals' call counters (their `static int niter`).
#[derive(Debug, Default)]
pub struct Counters {
    pub edges: i32,
    pub corners: i32,
    pub background: i32,
}

/// A segmentation's frame 0 as bytes, read and written in place; the intensities; the label
/// worked on and the forbidden one; `topofix_pbm`.
pub struct Fix<'a> {
    seg: &'a mut [u8],
    w: i32,
    h: i32,
    d: i32,
    row: usize,
    orig: &'a Mri,
    pub label: i32,
    pub f_label: i32,
    /// `topofix_pbm`: set by a pass whose last flag was raised.
    pub pbm: bool,
}

impl<'a> Fix<'a> {
    /// The segmentation `seg` (a byte volume with its voxels) and the intensities `orig` (of the
    /// same width, height and depth).
    pub fn new(seg: &'a mut Mri, orig: &'a Mri) -> Fix<'a> {
        debug_assert_eq!(seg.mtype, MRI_UCHAR);
        let (w, h, d) = (seg.vg.width, seg.vg.height, seg.vg.depth);
        let row = seg.vox_per_row as usize;
        let n = (d as usize) * (h as usize) * row;
        let chunk = seg.chunk.as_mut().expect("a volume with its voxels");
        Fix {
            seg: &mut chunk[..n],
            w,
            h,
            d,
            row,
            orig,
            label: 0,
            f_label: 0,
            pbm: false,
        }
    }

    #[inline]
    fn at(&self, p: Off) -> usize {
        (p.2 as usize * self.h as usize + p.1 as usize) * self.row + p.0 as usize
    }

    /// `MRIvox(mri_seg, ...)` as an `int`.
    #[inline]
    fn v(&self, p: Off) -> i32 {
        i32::from(self.seg[self.at(p)])
    }

    #[inline]
    fn is(&self, p: Off) -> bool {
        self.v(p) == self.label
    }

    #[inline]
    fn forbidden(&self, p: Off) -> bool {
        self.v(p) == self.f_label
    }

    #[inline]
    fn set(&mut self, p: Off) {
        let i = self.at(p);
        self.seg[i] = self.label as u8;
    }

    /// `MRIgetVoxVal(mri_orig, x, y, z, 0)`.
    fn orig(&self, g: &mut Globals, err: &mut dyn Write, p: Off) -> Result<f32, Fatal> {
        let o = self.orig;
        if o.mtype == MRI_UCHAR
            && let Some(c) = o.chunk.as_ref()
        {
            let i = (p.2 as usize * o.vg.height as usize + p.1 as usize) * o.vox_per_row as usize
                + p.0 as usize;
            if let Some(b) = c.get(i) {
                return Ok(f32::from(*b));
            }
        }
        mri_get_vox_val(g, err, o, p.0, p.1, p.2, 0, 0)
    }

    /// The voxels of `label`.
    pub fn count(&self, label: i32) -> i32 {
        self.seg.chunks(self.row).fold(0, |n, r| {
            n + r[..self.w as usize]
                .iter()
                .filter(|&&b| i32::from(b) == label)
                .count() as i32
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn pass_line(
        err: &mut dyn Write,
        tag: &str,
        npass: i32,
        nfound: i32,
        nmod: i32,
        total: i32,
        pbm: i32,
    ) {
        let _ = write!(
            err,
            "\npass {npass:3} ({tag}): {nfound:3} found - {nmod:3} modified     |    TOTAL: {total:3}  (PBM={pbm})"
        );
    }

    /// Sets `a` or `b` (of which neither is the label): the one that is not forbidden when the
    /// other is; otherwise the brighter (`b` unless `a` is brighter), the flag counted when both
    /// are then forbidden (only when the label is the forbidden one: FS-1289).
    fn choose(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        a: Off,
        b: Off,
        detect: &mut i32,
    ) -> Result<(), Fatal> {
        if self.forbidden(a) && !self.forbidden(b) {
            self.set(b);
        } else if !self.forbidden(a) && self.forbidden(b) {
            self.set(a);
        } else {
            if self.orig(g, err, a)? > self.orig(g, err, b)? {
                self.set(a);
            } else {
                self.set(b);
            }
            if self.forbidden(a) && self.forbidden(b) {
                *detect += 1;
            }
        }
        Ok(())
    }

    fn edge_direction(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        (tag, diag, a, b): Edge,
        total: &mut i32,
    ) -> Result<(), Fatal> {
        let (x0, x1) = span(self.w, diag.0);
        let (y0, y1) = span(self.h, diag.1);
        let (z0, z1) = span(self.d, diag.2);
        let mut nfound = 1;
        let mut nmodified = 0;
        let mut npass = 0;
        while nfound != 0 {
            nfound = 0;
            npass += 1;
            let mut detect = 0;
            for k in z0..z1 {
                for j in y0..y1 {
                    for i in x0..x1 {
                        let p = (i, j, k);
                        if !self.is(p) || !self.is(add(p, diag)) {
                            continue;
                        }
                        let (pa, pb) = (add(p, a), add(p, b));
                        if self.is(pa) || self.is(pb) {
                            continue;
                        }
                        self.choose(g, err, pa, pb, &mut detect)?;
                        nfound += 1;
                    }
                }
            }
            nmodified += nfound;
            *total += nfound;
            if detect != 0 {
                self.pbm = true;
            }
            Self::pass_line(err, tag, npass, nfound, nmodified, *total, detect);
        }
        Ok(())
    }

    /// `mriRemoveEdgeConfiguration(mri_seg, mri_orig, label, f_label)`.
    pub fn remove_edges(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        c: &mut Counters,
    ) -> Result<i32, Fatal> {
        c.edges += 1;
        let _ = write!(err, "\nIteration Number : {}", c.edges);
        let mut total = 0;
        let dirs: [Edge; 6] = [
            ("xy+", (1, 1, 0), (0, 1, 0), (1, 0, 0)),
            ("xy-", (-1, 1, 0), (0, 1, 0), (-1, 0, 0)),
            ("yz+", (0, 1, 1), (0, 1, 0), (0, 0, 1)),
            ("yz-", (0, 1, -1), (0, 1, 0), (0, 0, -1)),
            ("xz+", (1, 0, 1), (1, 0, 0), (0, 0, 1)),
            ("xz-", (-1, 0, 1), (-1, 0, 0), (0, 0, 1)),
        ];
        for e in dirs {
            self.edge_direction(g, err, e, &mut total)?;
        }
        Ok(total)
    }

    /// One corner direction `(1, sy, sz)`: as `mri_pretess`'s, the paths with a forbidden voxel
    /// skipped (all six forbidden: none skipped, the flag 1; else 0). The flag is the last
    /// configuration's (kept from the previous pass when a pass finds none: FS-1289).
    fn corner_direction(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        sy: i32,
        sz: i32,
        total: &mut i32,
    ) -> Result<(), Fatal> {
        let (ex, ey, ez) = ((1, 0, 0), (0, sy, 0), (0, 0, sz));
        let (x0, x1) = span(self.w, 1);
        let (y0, y1) = span(self.h, sy);
        let (z0, z1) = span(self.d, sz);
        let mut nfound = 1;
        let mut nmodified = 0;
        let mut npass = 0;
        let mut detect = 0;
        while nfound != 0 {
            nfound = 0;
            npass += 1;
            for k in z0..z1 {
                for j in y0..y1 {
                    for i in x0..x1 {
                        let p = (i, j, k);
                        let (px, py, pz) = (add(p, ex), add(p, ey), add(p, ez));
                        let (pxy, pxz, pyz) = (add(px, ey), add(px, ez), add(py, ez));
                        if !self.is(p) || !self.is(add(pxy, ez)) {
                            continue;
                        }
                        if (self.is(px) && (self.is(pxy) || self.is(pxz)))
                            || (self.is(py) && (self.is(pyz) || self.is(pxy)))
                            || (self.is(pz) && (self.is(pxz) || self.is(pyz)))
                        {
                            continue;
                        }
                        let paths = [
                            (px, pxy),
                            (px, pxz),
                            (py, pyz),
                            (py, pxy),
                            (pz, pxz),
                            (pz, pyz),
                        ];
                        let mut dist = [0f32; 6];
                        for (d, &(u, v)) in dist.iter_mut().zip(&paths) {
                            *d = self.orig(g, err, u)? + self.orig(g, err, v)?;
                        }
                        let mut off = [false; 6];
                        for (f, &(u, v)) in off.iter_mut().zip(&paths) {
                            *f = self.forbidden(u) || self.forbidden(v);
                        }
                        if off.iter().all(|&f| f) {
                            detect = 1;
                            off = [false; 6];
                        } else {
                            detect = 0;
                        }
                        let mut refp = off.iter().position(|&f| !f).unwrap_or(0);
                        let mut maxdist = dist[refp];
                        for q in refp + 1..6 {
                            if off[q] {
                                continue;
                            }
                            if maxdist < dist[q] {
                                maxdist = dist[q];
                                refp = q;
                            }
                        }
                        let (u, v) = paths[refp];
                        if !self.is(u) {
                            self.set(u);
                            nfound += 1;
                        }
                        if !self.is(v) {
                            self.set(v);
                            nfound += 1;
                        }
                    }
                }
            }
            nmodified += nfound;
            *total += nfound;
            if detect != 0 {
                self.pbm = true;
            }
            Self::pass_line(err, "+++", npass, nfound, nmodified, *total, detect);
        }
        Ok(())
    }

    /// `mriRemoveCornerConfiguration(mri_seg, mri_orig, label, f_label)`.
    pub fn remove_corners(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        c: &mut Counters,
    ) -> Result<i32, Fatal> {
        c.corners += 1;
        let _ = write!(err, "\nIteration Number : {}", c.corners);
        let mut total = 0;
        for (sy, sz) in [(1, 1), (1, -1), (-1, -1), (-1, 1)] {
            self.corner_direction(g, err, sy, sz, &mut total)?;
        }
        Ok(total)
    }

    /// One background corner direction `(1, sy, sz)`: as `mri_pretess`'s (the voxel along z not
    /// looked at: FS-1286), the voxel or its corner neighbour set as [`Fix::choose`] sets them.
    fn background_direction(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        tag: &str,
        sy: i32,
        sz: i32,
        total: &mut i32,
    ) -> Result<(), Fatal> {
        let (ex, ey, ez) = ((1, 0, 0), (0, sy, 0), (0, 0, sz));
        let (x0, x1) = span(self.w, 1);
        let (y0, y1) = span(self.h, sy);
        let (z0, z1) = span(self.d, sz);
        let mut nfound = 1;
        let mut nmodified = 0;
        let mut npass = 0;
        while nfound != 0 {
            nfound = 0;
            npass += 1;
            let mut detect = 0;
            for k in z0..z1 {
                for j in y0..y1 {
                    for i in x0..x1 {
                        let p = (i, j, k);
                        let (px, py) = (add(p, ex), add(p, ey));
                        let (pxy, pxz, pyz) = (add(px, ey), add(px, ez), add(py, ez));
                        let corner = add(pxy, ez);
                        if self.is(p) || self.is(corner) {
                            continue;
                        }
                        if self.is(px)
                            && self.is(pxy)
                            && self.is(py)
                            && self.is(pyz)
                            && self.is(px)
                            && self.is(pxz)
                        {
                            if !self.is(add(p, ez)) {
                                g.notices.add(
                                    &FS_1286,
                                    "mri_fill -topofix: a background corner configuration filled whose voxel along z is not of the label (FreeSurfer does not look at it): kept",
                                );
                            }
                            // the voxel first, its corner neighbour second, as `choose` takes
                            // `a` and `b`
                            self.choose(g, err, p, corner, &mut detect)?;
                            nfound += 1;
                        }
                    }
                }
            }
            nmodified += nfound;
            *total += nfound;
            if detect != 0 {
                self.pbm = true;
            }
            Self::pass_line(err, tag, npass, nfound, nmodified, *total, detect);
        }
        Ok(())
    }

    /// `mriRemoveBackgroundCornerConfiguration(mri_seg, mri_orig, label, f_label)`.
    pub fn remove_background_corners(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        c: &mut Counters,
    ) -> Result<i32, Fatal> {
        c.background += 1;
        let _ = write!(err, "\nIteration Number : {}", c.background);
        let mut total = 0;
        for (tag, sy, sz) in [("++", 1, 1), ("+-", 1, -1), ("--", -1, -1), ("-+", -1, 1)] {
            self.background_direction(g, err, tag, sy, sz, &mut total)?;
        }
        Ok(total)
    }
}

/// `100.0 * ntotal / nvoxels` as FreeSurfer's build computes it: with no voxel of the label
/// x86-64's default NaN, printed `-nan` (FS-1290, kept).
fn percent(g: &mut Globals, ntotal: i32, nvoxels: i32, side: &str) -> f64 {
    if nvoxels == 0 {
        g.notices.add(
            &FS_1290,
            format!(
                "mri_fill -topofix: no voxel of the {side} hemisphere's label in the volume corrected: its share of modified voxels is printed -nan or inf, as in FreeSurfer (kept)"
            ),
        );
        if ntotal == 0 {
            return f64::from_bits(DEFAULT_NAN);
        }
        return if ntotal > 0 {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
    }
    100.0 * f64::from(ntotal) / f64::from(nvoxels)
}

/// One hemisphere's correction: up to 100 iterations of the three removals for `label` with
/// `f_label` forbidden, until one sets nothing; the problem counts of the three.
fn hemisphere(
    g: &mut Globals,
    err: &mut dyn Write,
    fix: &mut Fix,
    counters: &mut Counters,
    ntotal: &mut i32,
) -> Result<(i32, i32, i32), Fatal> {
    let (mut edge, mut fcorner, mut bcorner) = (0, 0, 0);
    for _ in 0..100 {
        let mut nmodified = 0;
        fix.pbm = false;
        nmodified += fix.remove_edges(g, err, counters)?;
        edge += i32::from(fix.pbm);
        fix.pbm = false;
        nmodified += fix.remove_corners(g, err, counters)?;
        fcorner += i32::from(fix.pbm);
        fix.pbm = false;
        nmodified += fix.remove_background_corners(g, err, counters)?;
        bcorner += i32::from(fix.pbm);
        if nmodified == 0 {
            break;
        }
        *ntotal += nmodified;
    }
    Ok((edge, fcorner, bcorner))
}

/// `mri_topofix(mri_seg, mri_orig)` with FreeSurfer's fill values `rh_fill_val` and `lh_fill_val`
/// (and the call counters, the functions' statics, shared by the program's two calls): the right
/// hemisphere's label corrected with the left's forbidden, then the left's with the right's
/// forbidden (each with its summary on standard error; the left's total also counts the right's in
/// FreeSurfer: FS-1288), then each connected component of the background (`MRIsegment(seg, 0, 0)`)
/// but the largest that touches only one hemisphere's label (6-connected) filled with that label
/// (FreeSurfer prints `removing component` but fills nothing, and counts the kept components among
/// the removed: FS-1287). `NO_ERROR`.
#[allow(clippy::too_many_arguments)]
pub fn mri_topofix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    seg: &mut Mri,
    orig: &Mri,
    rh_fill_val: i32,
    lh_fill_val: i32,
    counters: &mut Counters,
) -> Result<i32, Fatal> {
    let reproduce = g.notices.reproduce();
    g.notices.add(
        &FS_1289,
        "mri_fill -topofix: the counts of configurations not resolved without the other hemisphere's voxels (pbm) are FreeSurfer's flags, which are almost never raised: kept",
    );
    let mut ntotal = 0;
    let (rpbm, lpbm);
    {
        let mut fix = Fix::new(seg, orig);
        let _ = writeln!(
            err,
            "correcting the topological correctness of the right hemisphere"
        );
        fix.label = rh_fill_val;
        fix.f_label = lh_fill_val;
        let (e, f, b) = hemisphere(g, err, &mut fix, counters, &mut ntotal)?;
        let nvoxels = fix.count(rh_fill_val);
        let pct = percent(g, ntotal, nvoxels, "right");
        let _ = write!(
            err,
            "\nRight hemisphere: total Number of Modified Voxels = {ntotal} (out of {nvoxels}: {})\n",
            fmt_f(pct, 0, 6)
        );
        rpbm = e + f + b;
        let _ = writeln!(
            err,
            "                  pbm: edges = {e} - corner = {f} - background = {b}"
        );
        let _ = writeln!(err);
        let _ = writeln!(
            err,
            "correcting the topological correctness of the left hemisphere"
        );
        fix.label = lh_fill_val;
        fix.f_label = rh_fill_val;
        let right_total = ntotal;
        if !reproduce && right_total != 0 {
            g.notices.add(
                &FS_1288,
                "mri_fill -topofix: the left hemisphere's total of modified voxels counts only its own (FreeSurfer adds the right's)",
            );
            ntotal = 0;
        }
        let (e, f, b) = hemisphere(g, err, &mut fix, counters, &mut ntotal)?;
        if reproduce && right_total != 0 {
            g.notices.add(
                &FS_1288,
                "mri_fill -topofix: the left hemisphere's total of modified voxels also counts the right's, as in FreeSurfer",
            );
        }
        let nvoxels = fix.count(lh_fill_val);
        let pct = percent(g, ntotal, nvoxels, "left");
        let _ = write!(
            err,
            "\nLeft hemisphere: total Number of Modified Voxels = {ntotal} (out of {nvoxels}: {})\n",
            fmt_f(pct, 0, 6)
        );
        lpbm = e + f + b;
        let _ = writeln!(
            err,
            "                 pbm: edges = {e} - corner = {f} - background = {b}"
        );
        let _ = writeln!(out);
    }

    // finally, the small connected components
    let _ = writeln!(err, "removing connected components...");
    let mut ncpts = 0;
    let mut removed = 0;
    let mut segmentation = Some(mri_segment(g, out, err, seg, 0.0, 0.0)?);
    let sg = segmentation.as_ref().expect("made above");
    let max_segment = mri_segment_max(sg);
    let (w, h, d) = (seg.vg.width, seg.vg.height, seg.vg.depth);
    let row = seg.vox_per_row as usize;
    let at = |x: i32, y: i32, z: i32| (z as usize * h as usize + y as usize) * row + x as usize;
    let mut fills: Vec<(usize, i32)> = Vec::new();
    for (k, segment) in sg.segments.iter().enumerate() {
        if k as i32 == max_segment || segment.nvoxels() == 0 {
            continue;
        }
        let chunk = seg.chunk.as_ref().expect("voxels");
        // the neighbouring label of the segment
        let mut label = 0;
        let mut nlabels = 0;
        for v in &segment.voxels {
            let (x, y, z) = (i32::from(v.x), i32::from(v.y), i32::from(v.z));
            for a in -1..2 {
                if x + a < 0 || x + a >= w {
                    continue;
                }
                for b in -1i32..2 {
                    if y + b < 0 || y + b >= h {
                        continue;
                    }
                    for c in -1i32..2 {
                        if z + c < 0 || z + c >= d {
                            continue;
                        }
                        if a.abs() + b.abs() + c.abs() > 1 {
                            continue;
                        }
                        let val = i32::from(chunk[at(x + a, y + b, z + c)]);
                        if val == rh_fill_val || val == lh_fill_val {
                            if label > 0 && val != label {
                                nlabels = 2;
                            }
                            if label == 0 {
                                label = val;
                                nlabels = 1;
                            }
                        }
                    }
                }
            }
            if nlabels == 2 {
                break;
            }
        }
        if label == 0 {
            let _ = write!(err, "\ncould not find neighboring label!!\n");
            continue;
        }
        ncpts += 1;
        let what = if nlabels == 2 { "keeping" } else { "removing" };
        let _ = writeln!(
            err,
            "   {what} component {k}, located in ({},{},{}) with {} voxels",
            fmt_f(f64::from(segment.cx), 3, 1),
            fmt_f(f64::from(segment.cy), 3, 1),
            fmt_f(f64::from(segment.cz), 3, 1),
            segment.nvoxels()
        );
        if nlabels != 2 {
            removed += 1;
            g.notices.add(
                &FS_1287,
                if reproduce {
                    "mri_fill -topofix: a background component it says it removes is left as it is, and the kept components are counted among the removed, as in FreeSurfer"
                } else {
                    "mri_fill -topofix: a background component touching one hemisphere filled with its label, and only the filled ones counted as removed (FreeSurfer fills none and counts the kept ones too)"
                },
            );
            if !reproduce {
                for v in &segment.voxels {
                    let (x, y, z) = (i32::from(v.x), i32::from(v.y), i32::from(v.z));
                    fills.push((at(x, y, z), label));
                }
            }
        }
    }
    if let Some(chunk) = seg.chunk.as_mut() {
        for (i, label) in fills {
            chunk[i] = label as u8;
        }
    }
    let _ = writeln!(
        err,
        "{} connected components were removed",
        if reproduce { ncpts } else { removed }
    );
    mri_segment_free(&mut segmentation);
    let _ = write!(
        err,
        "\nTopological correction completed with {} problems (r = {rpbm}  l = {lpbm})\n\n",
        lpbm + rpbm
    );
    Ok(NO_ERROR)
}

/// `findMinSize(mri)`: the smallest voxel size, as FreeSurfer's comparisons pick it (`x > y`: the
/// smaller of `y` and `z`, `y` on a tie; else the smaller of `x` and `z`, `x` on a tie).
pub fn find_min_size(mri: &Mri) -> f64 {
    let (x, y, z) = (
        f64::from(mri.vg.xsize),
        f64::from(mri.vg.ysize),
        f64::from(mri.vg.zsize),
    );
    if x > y {
        if y > z { z } else { y }
    } else if z > x {
        x
    } else {
        z
    }
}

/// `verifyLRSplit(mri_fill, lta, cc_tal_x, ...)`'s counts: `(badRH, badLH, RH, LH)`: the voxels
/// of the right's and the left's fill value, and those of the right's whose Talairach x is below
/// `cc_tal_x` (of the left's: above it), by `MRIvoxelToTalairachEx` (slices outermost, then rows,
/// then columns; voxels of other nonzero values are transformed too).
#[allow(clippy::too_many_arguments)]
pub fn verify_lr_split(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri_fill: &Mri,
    lta: Option<&Lta>,
    cc_tal_x: f64,
    rh_fill_val: i32,
    lh_fill_val: i32,
) -> Result<(i32, i32, i32, i32), Fatal> {
    let (mut bad_rh, mut bad_lh, mut rh, mut lh) = (0, 0, 0, 0);
    let row = mri_fill.vox_per_row as usize;
    let hgt = mri_fill.vg.height as usize;
    for z in 0..mri_fill.vg.depth {
        for y in 0..mri_fill.vg.height {
            for x in 0..mri_fill.vg.width {
                let i = (z as usize * hgt + y as usize) * row + x as usize;
                let val = i32::from(mri_fill.chunk.as_ref().expect("voxels")[i]);
                if val == 0 {
                    continue;
                }
                if val == rh_fill_val {
                    rh += 1;
                } else if val == lh_fill_val {
                    lh += 1;
                }
                let (_, t) = mri_voxel_to_talairach_ex(
                    g,
                    out,
                    err,
                    mri_fill,
                    f64::from(x),
                    f64::from(y),
                    f64::from(z),
                    lta,
                )?;
                // a point that cannot be transformed: FreeSurfer crashes (FS-487); 0 here
                let tal_x = t.map_or(0.0, |p| p[0]);
                if tal_x < cc_tal_x {
                    if val == rh_fill_val {
                        bad_rh += 1;
                    }
                } else if tal_x > cc_tal_x && val == lh_fill_val {
                    bad_lh += 1;
                }
            }
        }
    }
    Ok((bad_rh, bad_lh, rh, lh))
}
