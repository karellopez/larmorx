// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_pretess`'s local functions (`mri_mc/mri_pretess.cpp` 55-1047, 1339-1413): the removal of
//! the edge configurations ([`Tess::remove_edges`], `mriRemoveEdgeConfiguration`), of the corner
//! configurations ([`Tess::remove_corners`], `mriRemoveCornerConfiguration`) and of the
//! background's corner configurations ([`Tess::remove_background_corners`],
//! `mriRemoveBackgroundCornerConfiguration`), and `MRIaddEdgeVoxel` ([`mri_add_edge_voxel`]).
//!
//! The segmentation is a byte volume (`MRIvox`): its frame 0 is read and written in place, in
//! FreeSurfer's order (slices outermost, then rows, then columns), so that a voxel set during a
//! pass is seen by the rest of it. The intensities are `MRIgetVoxVal`'s `float`s of frame 0; two
//! of them are summed in `float` for a corner's paths. Each of the six edge directions, four corner
//! directions and four background directions is a loop of passes until a pass finds nothing; each
//! function counts its own calls (`static int niter`) for its `Iteration Number` line. Every
//! voxel read is inside the volume (the loops stop one voxel before the border they step over).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

/// `WM_MIN_VAL` (`include/mri.h`).
pub const WM_MIN_VAL: i32 = 5;

/// The three functions' call counters (their `static int niter`).
#[derive(Debug, Default)]
pub struct Counters {
    pub edges: i32,
    pub corners: i32,
    pub background: i32,
}

/// A segmentation's frame 0 as bytes, read and written in place, and the intensity volume.
pub struct Tess<'a> {
    seg: &'a mut [u8],
    w: i32,
    h: i32,
    d: i32,
    row: usize,
    orig: &'a Mri,
    label: i32,
}

/// `(x, y, z)` offsets.
type Off = (i32, i32, i32);

fn add(p: Off, q: Off) -> Off {
    (p.0 + q.0, p.1 + q.1, p.2 + q.2)
}

/// The start and end (exclusive) of a loop that reads one voxel further by `step` (-1, 0, 1).
fn span(n: i32, step: i32) -> (i32, i32) {
    (step.min(0).abs(), n - step.max(0))
}

impl<'a> Tess<'a> {
    /// The segmentation `seg` (a byte volume with its voxels) and the intensities `orig` (of the
    /// same width, height and depth) for `label`.
    pub fn new(seg: &'a mut Mri, orig: &'a Mri, label: i32) -> Tess<'a> {
        debug_assert_eq!(seg.mtype, MRI_UCHAR);
        let (w, h, d) = (seg.vg.width, seg.vg.height, seg.vg.depth);
        let row = seg.vox_per_row as usize;
        let n = (d as usize) * (h as usize) * row;
        let chunk = seg.chunk.as_mut().expect("a volume read with its voxels");
        Tess {
            seg: &mut chunk[..n],
            w,
            h,
            d,
            row,
            orig,
            label,
        }
    }

    #[inline]
    fn at(&self, p: Off) -> usize {
        (p.2 as usize * self.h as usize + p.1 as usize) * self.row + p.0 as usize
    }

    /// `MRIvox(mri_seg, x, y, z) == label` (the byte compared as an `int`).
    #[inline]
    fn is(&self, p: Off) -> bool {
        i32::from(self.seg[self.at(p)]) == self.label
    }

    /// `MRIvox(mri_seg, x, y, z) = label` (stored as a byte).
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

    /// The voxels of label `label` (`nvoxels`).
    pub fn count(&self) -> i32 {
        let mut n = 0;
        for z in 0..self.d {
            for y in 0..self.h {
                for x in 0..self.w {
                    if self.is((x, y, z)) {
                        n += 1;
                    }
                }
            }
        }
        n
    }

    /// One pass line: `\npass %3d (<tag>): %3d found - %3d modified     |    TOTAL: %3d`.
    fn pass_line(err: &mut dyn Write, tag: &str, npass: i32, nfound: i32, nmod: i32, total: i32) {
        let _ = write!(
            err,
            "\npass {npass:3} ({tag}): {nfound:3} found - {nmod:3} modified     |    TOTAL: {total:3}"
        );
    }

    /// One edge direction: a voxel of the label whose diagonal neighbour `diag` is of the label
    /// but neither of the two voxels `a` and `b` that join them gets the brighter of `a` and `b`
    /// (`b` when `a` is not brighter); passes until one finds nothing.
    fn edge_direction(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        tag: &str,
        diag: Off,
        a: Off,
        b: Off,
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
                        // select the brighter voxel
                        if self.orig(g, err, pa)? > self.orig(g, err, pb)? {
                            self.set(pa);
                        } else {
                            self.set(pb);
                        }
                        nfound += 1;
                    }
                }
            }
            nmodified += nfound;
            *total += nfound;
            Self::pass_line(err, tag, npass, nfound, nmodified, *total);
        }
        Ok(())
    }

    /// `mriRemoveEdgeConfiguration(mri_seg, mri_orig, label)`: the six edge directions in the xy,
    /// yz and xz planes; the voxels set.
    pub fn remove_edges(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        c: &mut Counters,
    ) -> Result<i32, Fatal> {
        c.edges += 1;
        let _ = write!(err, "\nIteration Number : {}", c.edges);
        let mut total = 0;
        // (tag, the diagonal neighbour, the two voxels between them, the first chosen when brighter)
        let dirs: [(&str, Off, Off, Off); 6] = [
            ("xy+", (1, 1, 0), (0, 1, 0), (1, 0, 0)),
            ("xy-", (-1, 1, 0), (0, 1, 0), (-1, 0, 0)),
            ("yz+", (0, 1, 1), (0, 1, 0), (0, 0, 1)),
            ("yz-", (0, 1, -1), (0, 1, 0), (0, 0, -1)),
            ("xz+", (1, 0, 1), (1, 0, 0), (0, 0, 1)),
            ("xz-", (-1, 0, 1), (-1, 0, 0), (0, 0, 1)),
        ];
        for (tag, diag, a, b) in dirs {
            self.edge_direction(g, err, tag, diag, a, b, &mut total)?;
        }
        Ok(total)
    }

    /// One corner direction `(1, sy, sz)`: a voxel of the label whose corner neighbour is of the
    /// label with no path of two face steps between them inside the label gets the brightest
    /// such path (the sum of its two voxels' intensities in `float`; the first of equal ones):
    /// its voxels not of the label set, each counted.
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
                        // find the problematic configuration
                        if (self.is(px) && (self.is(pxy) || self.is(pxz)))
                            || (self.is(py) && (self.is(pyz) || self.is(pxy)))
                            || (self.is(pz) && (self.is(pxz) || self.is(pyz)))
                        {
                            continue;
                        }
                        // select the brighter path
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
                        let mut refp = 0;
                        let mut maxdist = dist[0];
                        for (q, &dq) in dist.iter().enumerate().skip(1) {
                            if maxdist < dq {
                                maxdist = dq;
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
            Self::pass_line(err, "+++", npass, nfound, nmodified, *total);
        }
        Ok(())
    }

    /// `mriRemoveCornerConfiguration(mri_seg, mri_orig, label)`: the corner directions (1, 1, 1),
    /// (1, 1, -1), (1, -1, -1), (1, -1, 1); the voxels set.
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

    /// One background corner direction `(1, sy, sz)`: a voxel not of the label whose corner
    /// neighbour is not of the label either, with the voxels next to the corner of the label
    /// (the face neighbour along x and those along x and y, y, y and z, x and z; the face
    /// neighbour along z is not looked at): the brighter of the two (the corner neighbour when the
    /// voxel is not brighter) gets the label.
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
                            if self.orig(g, err, p)? > self.orig(g, err, corner)? {
                                self.set(p);
                            } else {
                                self.set(corner);
                            }
                            nfound += 1;
                        }
                    }
                }
            }
            nmodified += nfound;
            *total += nfound;
            Self::pass_line(err, tag, npass, nfound, nmodified, *total);
        }
        Ok(())
    }

    /// `mriRemoveBackgroundCornerConfiguration(mri_seg, mri_orig, label)`: the background corner
    /// directions (1, 1, 1) `++`, (1, 1, -1) `+-`, (1, -1, -1) `--`, (1, -1, 1) `-+`; the voxels
    /// set.
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

/// `MRIaddEdgeVoxel(mri, SetVal)`: the first voxel (columns outermost, then rows, then slices; the
/// volume's border excluded) below `WM_MIN_VAL` whose six face neighbours are all below it and one
/// of whose four in-slice diagonal neighbours (x - 1 and y - 1, x + 1 and y - 1, x + 1 and y + 1,
/// x - 1 and y + 1) is not, set to `set_val` (`MRIsetVoxVal`, any type; frame 0) after
/// `  Adding test edge at <c> <r> <s>, val=<set_val> ` on standard output: 1; none: `Could not find
/// a test voxel to set` and 0.
pub fn mri_add_edge_voxel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &mut Mri,
    set_val: i32,
) -> Result<i32, Fatal> {
    let wm = WM_MIN_VAL as f32;
    for c in 1..mri.vg.width - 1 {
        for r in 1..mri.vg.height - 1 {
            for s in 1..mri.vg.depth - 1 {
                let mut val = |g: &mut Globals, x: i32, y: i32, z: i32| {
                    mri_get_vox_val(g, err, mri, x, y, z, 0, 0)
                };
                // skip a centre or face neighbour of the white matter
                if f64::from(val(g, c, r, s)?) >= f64::from(WM_MIN_VAL) {
                    continue;
                }
                let faces = [
                    (c - 1, r, s),
                    (c + 1, r, s),
                    (c, r - 1, s),
                    (c, r + 1, s),
                    (c, r, s - 1),
                    (c, r, s + 1),
                ];
                let mut face = false;
                for (x, y, z) in faces {
                    if val(g, x, y, z)? >= wm {
                        face = true;
                        break;
                    }
                }
                if face {
                    continue;
                }
                // the centre set when an edge neighbour is of the white matter
                let mut set = false;
                for (x, y) in [
                    (c - 1, r - 1),
                    (c + 1, r - 1),
                    (c + 1, r + 1),
                    (c - 1, r + 1),
                ] {
                    if val(g, x, y, s)? >= wm {
                        set = true;
                    }
                }
                if set {
                    let _ = writeln!(out, "  Adding test edge at {c} {r} {s}, val={set_val} ");
                    mri_set_vox_val(g, err, mri, c, r, s, 0, set_val as f32, 0)?;
                    return Ok(1);
                }
            }
        }
    }
    let _ = writeln!(out, "Could not find a test voxel to set");
    Ok(0)
}
