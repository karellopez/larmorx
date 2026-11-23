// SPDX-License-Identifier: Apache-2.0
//! Morphology (spec §50): `MRIerodeNN`, `MRIerode`, `MRIerode2D`, `MRIerodeSegmentation` and
//! `MRIdilateSegmentation` (FS-493, FS-497), the threshold dilations of a label
//! (`MRIdilateThreshLabel`, `MRIdilateInvThreshLabel`), `MRIdilateLabel` and
//! `MRIdilateLabelUchar`, `MRIopenN`, `MRIopen`, `MRIclose`, `MRIdilate6`, `MRIgrowLabel`,
//! `MRIturnOnFG` and `MRIturnOffBG`; §50's common rules as in `setops`.

use std::cmp::Ordering;

use larmorx_core::defects::{FS_493, FS_497};

use super::setops::ext;
use super::{Dst, VolOut, Vols};
use crate::maths::Returned;
use crate::mri::{Mri, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// The search box's start per axis (`[e, -1]`, so an empty search keeps `[e - 1, 0]` after the
/// growth: a voxel only for an extent of 1, as the dilations of §48).
const fn box_start(e: i32) -> (i32, i32) {
    (e, -1)
}

/// The inclusive box grown by one voxel and clipped to `[0, e - 1]`.
fn grow(lo: i32, hi: i32, e: i32) -> (i32, i32) {
    (lo.saturating_sub(1).max(0), hi.saturating_add(1).min(e - 1))
}

/// Whether a coordinate lies in `[0, e)`.
fn inside(i: i32, e: i32) -> bool {
    (0..e).contains(&i)
}

/// The most frequent of the labels (the smallest of a tie), none for none.
fn most_frequent(labels: &[i32]) -> Option<i32> {
    let mut best: Option<(usize, i32)> = None;
    for &l in labels {
        let n = labels.iter().filter(|&&k| k == l).count();
        best = match best {
            Some((bn, bl)) if bn > n || (bn == n && bl <= l) => Some((bn, bl)),
            _ => Some((n, l)),
        };
    }
    best.map(|(_, l)| l)
}

/// The kind of a segmentation's steps.
#[derive(Clone, Copy)]
enum SegStep<'m> {
    Erode {
        thresh: i32,
    },
    Dilate {
        mask: Option<&'m Mri>,
        frame: i32,
        mthresh: f64,
    },
}

impl Process {
    /// `MRIerodeNN(in, out, NNDef, ErodeEdges)` (§50): not in place; without `out` one frame of
    /// the input's extent and type with its header, pulse parameters and colour table; another
    /// `out` of other dimensions refused; `out` set to 0, then (columns outermost) frame 0 the
    /// smallest of the voxel's value and its neighbourhood's (`NNDef` 1: the 6 faces, 2: the 18,
    /// else 27), an offset outside setting it to 0 with `ErodeEdges`.
    pub fn mri_erode_nn(
        &mut self,
        src: &mut Mri,
        out: Dst,
        nndef: i32,
        erode_edges: i32,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, out);
        if v.d_is_a() {
            self.put_out(b"ERROR: MRIerodeNN(): cannot be done in-place\n");
            return Ok(VolOut::Nothing);
        }
        if v.has_dst() {
            if Process::mri_dim_mismatch(v.a(), v.d(), false) != 0 {
                self.put_out(b"ERROR: MRIerodeNN(): seg/out dim mismatch\n");
                return Ok(VolOut::Nothing);
            }
        } else {
            let k = v.a().kind;
            let mut m = self.new_with_header(v.a(), k, 1)?;
            let s = v.a().clone();
            self.copy_pulse_parameters(&s, &mut m);
            if let Some(ct) = &s.ct {
                m.ct = self.ctab_deep_copy(Some(ct));
            }
            v.set_dst(m);
        }
        self.mri_set_values(v.d_mut(), 0.0)?;
        let (w, h, d) = ext(v.a());
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let mut min = sse::widen(self.mri_get_vox_val(v.a(), c, r, s, 0, 0)?);
                    for dc in -1..=1i32 {
                        for dr in -1..=1i32 {
                            for ds in -1..=1i32 {
                                let (cc, rr, ss) = (c + dc, r + dr, s + ds);
                                if !inside(cc, w) || !inside(rr, h) || !inside(ss, d) {
                                    if erode_edges != 0 {
                                        min = 0.0;
                                    }
                                    continue;
                                }
                                let n = dc.abs() + dr.abs() + ds.abs();
                                if (nndef == 1 && n > 1) || (nndef == 2 && n > 2) {
                                    continue;
                                }
                                let x =
                                    sse::widen(self.mri_get_vox_val(v.a(), cc, rr, ss, 0, 0)?);
                                if x < min {
                                    min = x;
                                }
                            }
                        }
                    }
                    self.mri_set_vox_val(v.d_mut(), c, r, s, 0, sse::narrow(min), 0)?;
                }
            }
        }
        Ok(v.out())
    }

    /// The destinations of `MRIerode` and `MRIerode2D`: a clone without one; a given one of
    /// another extent ends the program (`MRIcheckVolDims`, observed: `vmorph`); the source itself
    /// into a clone copied back with `MRIcopy` at the end.
    fn erode_dst(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        f: fn(&mut Process, &Mri, &mut Mri) -> Result<(), Stop>,
    ) -> Result<VolOut, Stop> {
        match dst {
            Dst::New | Dst::Other => {
                let mut d = self.mri_clone_new(src)?;
                f(self, src, &mut d)?;
                Ok(VolOut::New(d))
            }
            Dst::Given(d) => {
                self.check_vol_dims(src, d)?;
                f(self, src, d)?;
                Ok(VolOut::Given)
            }
            Dst::Src => {
                let mut w = self.mri_clone_new(src)?;
                f(self, src, &mut w)?;
                self.mri_copy(&w, Some(src))?;
                Ok(VolOut::Src)
            }
        }
    }

    /// `MRIerode(src, dst)` (§50): two byte volumes: bytes, the smallest of the 27 neighbourhood
    /// bytes (coordinates clamped) from 255; else for each frame the smallest of the voxel's
    /// value and its 27 neighbours' (`float`, `<`), as values.
    pub fn mri_erode(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        self.erode_dst(src, dst, Process::erode_into)
    }

    fn erode_into(&mut self, src: &Mri, dst: &mut Mri) -> Result<(), Stop> {
        let name = "MRIerode";
        let e = ext(src);
        let (w, h, d) = e;
        if src.kind == types::UCHAR && dst.kind == types::UCHAR {
            self.walk_needs(e, &[src, dst], name)?;
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let mut m = 255u8;
                        for dz in -1..=1 {
                            let zz = (z + dz).clamp(0, d - 1);
                            for dy in -1..=1 {
                                let yy = (y + dy).clamp(0, h - 1);
                                for dx in -1..=1 {
                                    let xx = (x + dx).clamp(0, w - 1);
                                    m = m.min(self.byte_at(src, xx, yy, zz, name));
                                }
                            }
                        }
                        self.byte_put(dst, x, y, z, m, name);
                    }
                }
            }
            return Ok(());
        }
        for f in 0..src.nframes {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let mut m = self.mri_get_vox_val(src, x, y, z, f, 0)?;
                        for dz in -1..=1 {
                            let zz = (z + dz).clamp(0, d - 1);
                            for dy in -1..=1 {
                                let yy = (y + dy).clamp(0, h - 1);
                                for dx in -1..=1 {
                                    let xx = (x + dx).clamp(0, w - 1);
                                    let v = self.mri_get_vox_val(src, xx, yy, zz, f, 0)?;
                                    if v < m {
                                        m = v;
                                    }
                                }
                            }
                        }
                        self.mri_set_vox_val(dst, x, y, z, f, m, 0)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// `MRIerode2D(src, dst)` (§50): `MRIerode`'s destinations; frame 0, the smallest of the
    /// 3 x 3 in-slice neighbourhood (`double`, from the voxel), as values.
    pub fn mri_erode_2d(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        self.erode_dst(src, dst, Process::erode_2d_into)
    }

    fn erode_2d_into(&mut self, src: &Mri, dst: &mut Mri) -> Result<(), Stop> {
        let (w, h, d) = ext(src);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let mut m = sse::widen(self.mri_get_vox_val(src, x, y, z, 0, 0)?);
                    for dy in -1..=1 {
                        let yy = (y + dy).clamp(0, h - 1);
                        for dx in -1..=1 {
                            let xx = (x + dx).clamp(0, w - 1);
                            let v = sse::widen(self.mri_get_vox_val(src, xx, yy, z, 0, 0)?);
                            if v < m {
                                m = v;
                            }
                        }
                    }
                    self.mri_set_vox_val(dst, x, y, z, 0, sse::narrow(m), 0)?;
                }
            }
        }
        Ok(())
    }

    /// `MRIerodeSegmentation(seg, out, n, thresh)` (§50): FS-493 (a count below 1 never ends in
    /// FreeSurfer): an error in both modes.
    pub fn mri_erode_segmentation(
        &mut self,
        seg: &mut Mri,
        out: Dst,
        n: i32,
        thresh: i32,
    ) -> Result<VolOut, Stop> {
        let (r, _) = self.segmentation_steps(
            seg,
            out,
            n,
            "MRIerodeSegmentation",
            SegStep::Erode { thresh },
        )?;
        Ok(r)
    }

    /// `MRIdilateSegmentation(seg, out, n, mask, frame, mthresh, &nchanges)` (§50): the result and
    /// `nchanges` (`None` where it is not set). FS-493 (a count of 0 or below -1 never ends) and
    /// FS-497 (with -1, a step whose changes never reach the working copy repeats forever): errors
    /// in both modes.
    pub fn mri_dilate_segmentation(
        &mut self,
        seg: &mut Mri,
        out: Dst,
        n: i32,
        mask: Option<&Mri>,
        frame: i32,
        mthresh: f64,
    ) -> Result<(VolOut, Option<i32>), Stop> {
        self.segmentation_steps(
            seg,
            out,
            n,
            "MRIdilateSegmentation",
            SegStep::Dilate {
                mask,
                frame,
                mthresh,
            },
        )
    }

    fn segmentation_steps(
        &mut self,
        seg: &mut Mri,
        out: Dst,
        n: i32,
        name: &str,
        kind: SegStep,
    ) -> Result<(VolOut, Option<i32>), Stop> {
        let mut v = Vols::new(seg, None, out);
        if v.d_is_a() {
            self.put_out(format!("ERROR: {name}(): cannot be done in-place\n").as_bytes());
            return Ok((VolOut::Nothing, None));
        }
        if v.has_dst() {
            if Process::mri_dim_mismatch(v.a(), v.d(), false) != 0 {
                self.put_out(format!("ERROR: {name}(): seg/out dim mismatch\n").as_bytes());
                return Ok((VolOut::Nothing, None));
            }
        } else {
            let g = &v.a().geom;
            let m = self.mri_alloc_sequence(g.width, g.height, g.depth, v.a().kind, 1)?;
            v.set_dst(m);
        }
        if !self.copy_to_dst(&mut v)? {
            return Ok((VolOut::Nothing, None));
        }
        let dilate = matches!(kind, SegStep::Dilate { .. });
        let never = if dilate { n == 0 || n < -1 } else { n < 1 };
        if never {
            self.note(
                &FS_493,
                format!("{name}: a count of {n} refused (FreeSurfer never ends)"),
            );
            self.error_exit(
                codes::BAD_PARM,
                format!("{name}: a count of {n} never ends\n").as_bytes(),
            )?;
            return Ok((VolOut::Nothing, None));
        }
        let mut seg2 = match self.mri_copy(v.a(), None)? {
            Returned::New(m) => m,
            _ => return Ok((VolOut::Nothing, None)),
        };
        let mut nchanges = None;
        if n == -1 {
            let mut k = 0;
            loop {
                let c = self.seg_step(&seg2, &mut v, kind)?;
                k += 1;
                if c == 0 {
                    break;
                }
                let next = match self.mri_copy(v.d(), None)? {
                    Returned::New(m) => m,
                    _ => return Ok((VolOut::Nothing, None)),
                };
                if next.voxels == seg2.voxels {
                    self.note(
                        &FS_497,
                        format!("{name}: a step whose changes never reach the working copy refused (FreeSurfer repeats it forever)"),
                    );
                    self.error_exit(
                        codes::BAD_PARM,
                        format!(
                            "{name}: step {k} changes {c} voxels that the next step does not see\n"
                        )
                        .as_bytes(),
                    )?;
                    return Ok((VolOut::Nothing, None));
                }
                seg2 = next;
            }
            nchanges = Some(k);
        } else {
            for _ in 0..n - 1 {
                self.seg_step(&seg2, &mut v, kind)?;
                seg2 = match self.mri_copy(v.d(), None)? {
                    Returned::New(m) => m,
                    _ => return Ok((VolOut::Nothing, None)),
                };
            }
            let c = self.seg_step(&seg2, &mut v, kind)?;
            if dilate {
                nchanges = Some(c);
            }
        }
        Ok((v.out(), nchanges))
    }

    /// One step of the segmentation's erosion or dilation from the labels of `seg2` (frame 0,
    /// columns outermost) into the destination; the number changed.
    fn seg_step(&mut self, seg2: &Mri, v: &mut Vols, kind: SegStep) -> Result<i32, Stop> {
        let (w, h, d) = ext(seg2);
        let mut changed = 0;
        let label = |p: &mut Process, c: i32, r: i32, s: i32| -> Result<i32, Stop> {
            let x = p.mri_get_vox_val(seg2, c, r, s, 0, 0)?;
            Ok(p.int_fs12(sse::widen(x)))
        };
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let own = label(self, c, r, s)?;
                    let nb = [
                        (c - 1, r, s),
                        (c + 1, r, s),
                        (c, r - 1, s),
                        (c, r + 1, s),
                        (c, r, s - 1),
                        (c, r, s + 1),
                    ];
                    let o: Option<i32> = match kind {
                        SegStep::Erode { thresh } => Some({
                            let face = c == 0
                                || r == 0
                                || s == 0
                                || c == w - 1
                                || r == h - 1
                                || s == d - 1;
                            if face {
                                0
                            } else {
                                let mut k = 0;
                                for (x, y, z) in nb {
                                    if label(self, x, y, z)? != own {
                                        k += 1;
                                    }
                                }
                                if k > thresh { 0 } else { own }
                            }
                        }),
                        // only the voxels it changes are written
                        SegStep::Dilate {
                            mask,
                            frame,
                            mthresh,
                        } => {
                            let mut o = None;
                            let in_mask = match mask {
                                // not below the threshold (a NaN on either side is not below)
                                Some(m) => {
                                    sse::widen(self.mri_get_vox_val(m, c, r, s, frame, 0)?)
                                        .partial_cmp(&mthresh)
                                        != Some(Ordering::Less)
                                }
                                None => true,
                            };
                            if own == 0 && in_mask {
                                let mut labs = Vec::with_capacity(6);
                                for (x, y, z) in nb {
                                    if inside(x, w) && inside(y, h) && inside(z, d) {
                                        let l = label(self, x, y, z)?;
                                        if l != 0 {
                                            labs.push(l);
                                        }
                                    }
                                }
                                if let Some(l) = most_frequent(&labs) {
                                    o = Some(l);
                                    changed += 1;
                                }
                            }
                            o
                        }
                    };
                    if let Some(o) = o {
                        self.mri_set_vox_val(v.d_mut(), c, r, s, 0, o as f32, 0)?;
                    }
                }
            }
        }
        if let SegStep::Erode { .. } = kind
            && v.d().ct.is_some()
            && let Some(ct) = v.a().ct.clone()
        {
            v.d_mut().ct = self.ctab_deep_copy(Some(&ct));
        }
        Ok(changed)
    }

    /// `MRIdilateThreshLabel(src, val, dst, label, niter, thresh)` (§50).
    pub fn mri_dilate_thresh_label(
        &mut self,
        src: &mut Mri,
        val: &mut Mri,
        dst: Dst,
        label: i32,
        niter: i32,
        thresh: i32,
    ) -> Result<VolOut, Stop> {
        self.thresh_label(src, val, dst, label, niter, thresh, false)
    }

    /// `MRIdilateInvThreshLabel(src, val, dst, label, niter, thresh)` (§50).
    pub fn mri_dilate_inv_thresh_label(
        &mut self,
        src: &mut Mri,
        val: &mut Mri,
        dst: Dst,
        label: i32,
        niter: i32,
        thresh: i32,
    ) -> Result<VolOut, Stop> {
        self.thresh_label(src, val, dst, label, niter, thresh, true)
    }

    /// The threshold dilations (§50): up to `niter` steps on bytes, each from a copy of the
    /// destination; over the box of the source's `label` bytes grown by one (its upper bounds
    /// raised during the pass), a voxel whose value byte passes the threshold becomes `label`
    /// where its 27 neighbourhood holds it.
    #[allow(clippy::too_many_arguments)]
    fn thresh_label(
        &mut self,
        src: &mut Mri,
        val: &mut Mri,
        dst: Dst,
        label: i32,
        niter: i32,
        thresh: i32,
        inverse: bool,
    ) -> Result<VolOut, Stop> {
        let name = if inverse {
            "MRIdilateInvThreshLabel"
        } else {
            "MRIdilateThreshLabel"
        };
        self.check_vol_dims(src, val)?;
        let mut v = Vols::new(src, Some(val), dst);
        if !self.copy_to_dst(&mut v)? {
            return Ok(VolOut::Nothing);
        }
        let e = ext(v.a());
        let (w, h, d) = e;
        let mut tmp: Option<Mri> = None;
        for _ in 0..niter.max(0) {
            let t = match self.mri_copy(v.d(), None)? {
                Returned::New(m) => m,
                _ => break,
            };
            self.walk_needs(e, &[v.a(), v.b(), &t, v.d()], name)?;
            let ((mut x0, mut x1), (mut y0, mut y1), (mut z0, mut z1)) =
                (box_start(w), box_start(h), box_start(d));
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        if i32::from(self.byte_at(v.a(), x, y, z, name)) == label {
                            (x0, x1) = (x0.min(x), x1.max(x));
                            (y0, y1) = (y0.min(y), y1.max(y));
                            (z0, z1) = (z0.min(z), z1.max(z));
                        }
                    }
                }
            }
            (x0, x1) = grow(x0, x1, w);
            (y0, y1) = grow(y0, y1, h);
            (z0, z1) = grow(z0, z1, d);
            let mut added = 0;
            let mut z = z0;
            while z <= z1 {
                let mut y = y0;
                while y <= y1 {
                    let mut x = x0;
                    while x <= x1 {
                        let mut o = self.byte_at(&t, x, y, z, name);
                        let vb = i32::from(self.byte_at(v.b(), x, y, z, name));
                        let pass = if inverse { vb <= thresh } else { vb >= thresh };
                        if pass {
                            let mut found = false;
                            for dz in -1..=1 {
                                let zz = (z + dz).clamp(0, d - 1);
                                for dy in -1..=1 {
                                    let yy = (y + dy).clamp(0, h - 1);
                                    for dx in -1..=1 {
                                        let xx = (x + dx).clamp(0, w - 1);
                                        if i32::from(self.byte_at(&t, xx, yy, zz, name)) == label {
                                            found = true;
                                        }
                                    }
                                }
                            }
                            if found {
                                if i32::from(self.byte_at(v.d(), x, y, z, name)) != label {
                                    added += 1;
                                }
                                o = label as u8;
                                x1 = x1.max((x + 1).min(w - 1));
                                y1 = y1.max((y + 1).min(h - 1));
                                z1 = z1.max((z + 1).min(d - 1));
                            }
                        }
                        self.byte_put(v.d_mut(), x, y, z, o, name);
                        x += 1;
                    }
                    y += 1;
                }
                z += 1;
            }
            tmp = Some(t);
            if added == 0 {
                break;
            }
        }
        self.mri_free(&mut tmp);
        Ok(v.out())
    }

    /// `MRIdilateLabelUchar(src, dst, label, niter)` (§50): checked; `dst = MRIcopy(src, dst)`;
    /// `niter` steps on bytes over the box of the source's `label` grown by one: a voxel becomes
    /// `label` when it or a neighbourhood byte of the step's copy is `label`.
    pub fn mri_dilate_label_uchar(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        label: i32,
        niter: i32,
    ) -> Result<VolOut, Stop> {
        let name = "MRIdilateLabelUchar";
        let mut v = Vols::new(src, None, dst);
        self.check_a_d(&v)?;
        if !self.copy_to_dst(&mut v)? {
            return Ok(VolOut::Nothing);
        }
        let e = ext(v.a());
        let (w, h, d) = e;
        let mut tmp: Option<Mri> = None;
        for _ in 0..niter.max(0) {
            let t = match self.mri_copy(v.d(), None)? {
                Returned::New(m) => m,
                _ => break,
            };
            self.walk_needs(e, &[v.a(), &t, v.d()], name)?;
            // the box of the source's label at each step (the source may be the destination);
            // its bounds fixed during the pass
            let ((mut x0, mut x1), (mut y0, mut y1), (mut z0, mut z1)) =
                (box_start(w), box_start(h), box_start(d));
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        if i32::from(self.byte_at(v.a(), x, y, z, name)) == label {
                            (x0, x1) = (x0.min(x), x1.max(x));
                            (y0, y1) = (y0.min(y), y1.max(y));
                            (z0, z1) = (z0.min(z), z1.max(z));
                        }
                    }
                }
            }
            let (x0, x1) = grow(x0, x1, w);
            let (y0, y1) = grow(y0, y1, h);
            let (z0, z1) = grow(z0, z1, d);
            for z in z0..=z1 {
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        let mut found = false;
                        'search: for dz in -1..=1 {
                            let zz = (z + dz).clamp(0, d - 1);
                            for dy in -1..=1 {
                                let yy = (y + dy).clamp(0, h - 1);
                                for dx in -1..=1 {
                                    let xx = (x + dx).clamp(0, w - 1);
                                    if i32::from(self.byte_at(&t, xx, yy, zz, name)) == label {
                                        found = true;
                                        break 'search;
                                    }
                                }
                            }
                        }
                        if found {
                            self.byte_put(v.d_mut(), x, y, z, label as u8, name);
                        }
                    }
                }
            }
            tmp = Some(t);
        }
        self.mri_free(&mut tmp);
        Ok(v.out())
    }

    /// `MRIdilateLabel(src, dst, label, niter)` (§50): checked; byte sources go to
    /// [`Process::mri_dilate_label_uchar`]; else the same on values for every frame, the box from
    /// the step copy's values of any frame equal to `label`.
    pub fn mri_dilate_label(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        label: i32,
        niter: i32,
    ) -> Result<VolOut, Stop> {
        if src.kind == types::UCHAR {
            return self.mri_dilate_label_uchar(src, dst, label, niter);
        }
        let mut v = Vols::new(src, None, dst);
        self.check_a_d(&v)?;
        if !self.copy_to_dst(&mut v)? {
            return Ok(VolOut::Nothing);
        }
        let (w, h, d) = ext(v.a());
        let nf = v.a().nframes;
        let lab = f64::from(label);
        let mut tmp: Option<Mri> = None;
        for _ in 0..niter.max(0) {
            let t = match self.mri_copy(v.d(), None)? {
                Returned::New(m) => m,
                _ => break,
            };
            let ((mut x0, mut x1), (mut y0, mut y1), (mut z0, mut z1)) =
                (box_start(w), box_start(h), box_start(d));
            for f in 0..nf {
                for z in 0..d {
                    for y in 0..h {
                        for x in 0..w {
                            if sse::widen(self.mri_get_vox_val(&t, x, y, z, f, 0)?) == lab {
                                (x0, x1) = (x0.min(x), x1.max(x));
                                (y0, y1) = (y0.min(y), y1.max(y));
                                (z0, z1) = (z0.min(z), z1.max(z));
                            }
                        }
                    }
                }
            }
            let (x0, x1) = grow(x0, x1, w);
            let (y0, y1) = grow(y0, y1, h);
            let (z0, z1) = grow(z0, z1, d);
            for f in 0..nf {
                for z in z0..=z1 {
                    for y in y0..=y1 {
                        for x in x0..=x1 {
                            let mut found = false;
                            'search: for dz in -1..=1 {
                                let zz = (z + dz).clamp(0, d - 1);
                                for dy in -1..=1 {
                                    let yy = (y + dy).clamp(0, h - 1);
                                    for dx in -1..=1 {
                                        let xx = (x + dx).clamp(0, w - 1);
                                        let q = self.mri_get_vox_val(&t, xx, yy, zz, f, 0)?;
                                        if sse::widen(q) == lab {
                                            found = true;
                                            break 'search;
                                        }
                                    }
                                }
                            }
                            if found {
                                self.mri_set_vox_val(v.d_mut(), x, y, z, f, label as f32, 0)?;
                            }
                        }
                    }
                }
            }
            tmp = Some(t);
        }
        self.mri_free(&mut tmp);
        Ok(v.out())
    }

    /// `MRIopenN(src, dst, order)` (§50): `order` erosions (the first into a new volume), then
    /// `order` dilations (`MRIdilate`, the first into `dst`); below 1 one of each.
    pub fn mri_open_n(&mut self, src: &mut Mri, dst: Dst, order: i32) -> Result<VolOut, Stop> {
        let k = order.max(1);
        let mut e = match self.mri_erode(src, Dst::New)? {
            VolOut::New(m) => m,
            _ => return Ok(VolOut::Nothing),
        };
        for _ in 1..k {
            self.mri_erode(&mut e, Dst::Src)?;
        }
        self.dilate_steps(src, &mut e, dst, k)
    }

    /// `k` dilations of `e`, the first into `dst` (`src` the open's source, when `dst` is it).
    fn dilate_steps(
        &mut self,
        src: &mut Mri,
        e: &mut Mri,
        dst: Dst,
        k: i32,
    ) -> Result<VolOut, Stop> {
        match dst {
            Dst::New | Dst::Other => {
                let mut m = match self.mri_dilate(e, Dst::New)? {
                    VolOut::New(m) => m,
                    _ => return Ok(VolOut::Nothing),
                };
                for _ in 1..k {
                    self.mri_dilate(&mut m, Dst::Src)?;
                }
                Ok(VolOut::New(m))
            }
            Dst::Given(m) => {
                self.mri_dilate(e, Dst::Given(m))?;
                for _ in 1..k {
                    self.mri_dilate(m, Dst::Src)?;
                }
                Ok(VolOut::Given)
            }
            Dst::Src => {
                self.mri_dilate(e, Dst::Given(src))?;
                for _ in 1..k {
                    self.mri_dilate(src, Dst::Src)?;
                }
                Ok(VolOut::Src)
            }
        }
    }

    /// `MRIopen(src, dst)` (§50): one erosion, one dilation.
    pub fn mri_open(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let mut e = match self.mri_erode(src, Dst::New)? {
            VolOut::New(m) => m,
            _ => return Ok(VolOut::Nothing),
        };
        self.dilate_steps(src, &mut e, dst, 1)
    }

    /// `MRIclose(src, dst)` (§50): one dilation (into a new volume), one erosion into `dst`.
    pub fn mri_close(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let mut t = match self.mri_dilate(src, Dst::New)? {
            VolOut::New(m) => m,
            _ => return Ok(VolOut::Nothing),
        };
        match dst {
            Dst::New | Dst::Other => self.mri_erode(&mut t, Dst::New),
            Dst::Given(m) => {
                self.mri_erode(&mut t, Dst::Given(m))?;
                Ok(VolOut::Given)
            }
            Dst::Src => {
                self.mri_erode(&mut t, Dst::Given(src))?;
                Ok(VolOut::Src)
            }
        }
    }

    /// `MRIdilate6(src, dst)` (§50): a clone without `dst`; checked; frame 0 in one pass (in place
    /// the voxels already written are read): from -1e10 the largest of the voxel's z neighbours
    /// (z - 1, z, z + 1), y neighbours (y - 1, y + 1) and x neighbours (x - 1, x, x + 1), as
    /// values; a NaN never wins.
    pub fn mri_dilate6(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        self.clone_dst(&mut v)?;
        self.check_vol_dims(v.a(), v.d())?;
        let (w, h, d) = ext(v.a());
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let pts = [
                        (x, y, (z - 1).clamp(0, d - 1)),
                        (x, y, z),
                        (x, y, (z + 1).clamp(0, d - 1)),
                        (x, (y - 1).clamp(0, h - 1), z),
                        (x, (y + 1).clamp(0, h - 1), z),
                        ((x - 1).clamp(0, w - 1), y, z),
                        (x, y, z),
                        ((x + 1).clamp(0, w - 1), y, z),
                    ];
                    let mut m = -1e10f32;
                    for (a, b, c) in pts {
                        let q = self.mri_get_vox_val(v.a(), a, b, c, 0, 0)?;
                        if q > m {
                            m = q;
                        }
                    }
                    self.mri_set_vox_val(v.d_mut(), x, y, z, 0, m, 0)?;
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIgrowLabel(mri, filled, in, out)` (§50): checked; bytes; passes over the box of the
    /// `in` voxels (for `in == 0`, of the nonzero voxels, every voxel on or outside its faces set
    /// to `out` first) until none fills, each pass's count on standard error. Returns 0.
    pub fn mri_grow_label(
        &mut self,
        m: &Mri,
        filled: &mut Mri,
        vin: i32,
        vout: i32,
    ) -> Result<i32, Stop> {
        let name = "MRIgrowLabel";
        self.check_vol_dims(m, filled)?;
        let e = ext(m);
        let (w, h, d) = e;
        self.walk_needs(e, &[m, filled], name)?;
        let (mut x0, mut x1, mut y0, mut y1, mut z0, mut z1) = (w, 0, h, 0, d, 0);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let b = i32::from(self.byte_at(m, x, y, z, name));
                    let hit = if vin == 0 { b != 0 } else { b == vin };
                    if hit {
                        (x0, x1) = (x0.min(x), x1.max(x));
                        (y0, y1) = (y0.min(y), y1.max(y));
                        (z0, z1) = (z0.min(z), z1.max(z));
                    }
                }
            }
        }
        let ob = vout as u8;
        let mut total = 0i64;
        if vin == 0 {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let outside =
                            x <= x0 || x >= x1 || y <= y0 || y >= y1 || z <= z0 || z >= z1;
                        if outside {
                            self.byte_put(filled, x, y, z, ob, name);
                            total += 1;
                        }
                    }
                }
            }
        }
        loop {
            let mut n = 0i64;
            for z in z0..=z1.min(d - 1) {
                for y in y0..=y1.min(h - 1) {
                    for x in x0..=x1.min(w - 1) {
                        if i32::from(self.byte_at(filled, x, y, z, name)) != vout {
                            continue;
                        }
                        for dz in -1..=1 {
                            for dy in -1..=1 {
                                for dx in -1..=1 {
                                    let (xx, yy, zz) = (x + dx, y + dy, z + dz);
                                    if !inside(xx, w) || !inside(yy, h) || !inside(zz, d) {
                                        continue;
                                    }
                                    if i32::from(self.byte_at(m, xx, yy, zz, name)) == vin
                                        && i32::from(self.byte_at(filled, xx, yy, zz, name)) != vout
                                    {
                                        self.byte_put(filled, xx, yy, zz, ob, name);
                                        n += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            total += n;
            self.put_err(format!("{n} voxels filled, total = {total}.\n").as_bytes());
            if n == 0 {
                break;
            }
        }
        Ok(0)
    }

    /// `MRIturnOnFG(mri, fg, bg)` (§50): checked (the volume and the foreground); bytes: the
    /// volume takes the foreground where it is above 0; then where the foreground is 0 and the
    /// volume above 0, the volume 0 and the background 1. Returns 0.
    pub fn mri_turn_on_fg(&mut self, m: &mut Mri, fg: &Mri, bg: &mut Mri) -> Result<i32, Stop> {
        let name = "MRIturnOnFG";
        self.check_vol_dims(m, fg)?;
        let e = ext(m);
        let (w, h, d) = e;
        self.walk_needs(e, &[m, fg], name)?;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let f = self.byte_at(fg, x, y, z, name);
                    if f > 0 {
                        self.byte_put(m, x, y, z, f, name);
                    }
                }
            }
        }
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if self.byte_at(fg, x, y, z, name) == 0 && self.byte_at(m, x, y, z, name) > 0 {
                        self.byte_put(m, x, y, z, 0, name);
                        self.direct_needs_voxels(bg, name)?;
                        self.byte_put(bg, x, y, z, 1, name);
                    }
                }
            }
        }
        Ok(0)
    }

    /// `MRIturnOffBG(mri, bg)` (§50): checked; bytes: the volume 0 where the background is above
    /// 0; then each voxel 0 in both takes the most frequent byte above 5 in the cube of half-width
    /// 1 around it (the lowest of a tie), the half-width grown up to 4 while none is found, else 0.
    /// Returns 0.
    pub fn mri_turn_off_bg(&mut self, m: &mut Mri, bg: &Mri) -> Result<i32, Stop> {
        let name = "MRIturnOffBG";
        self.check_vol_dims(m, bg)?;
        let e = ext(m);
        let (w, h, d) = e;
        self.walk_needs(e, &[m, bg], name)?;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if self.byte_at(bg, x, y, z, name) > 0 {
                        self.byte_put(m, x, y, z, 0, name);
                    }
                }
            }
        }
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if self.byte_at(m, x, y, z, name) != 0 || self.byte_at(bg, x, y, z, name) != 0 {
                        continue;
                    }
                    let mut o = 0u8;
                    for hw in 1..=4 {
                        let mut counts = [0u32; 256];
                        let mut any = false;
                        for zz in z - hw..=z + hw {
                            for yy in y - hw..=y + hw {
                                for xx in x - hw..=x + hw {
                                    if !inside(xx, w) || !inside(yy, h) || !inside(zz, d) {
                                        continue;
                                    }
                                    let b = self.byte_at(m, xx, yy, zz, name);
                                    if b > 5 {
                                        counts[b as usize] += 1;
                                        any = true;
                                    }
                                }
                            }
                        }
                        if any {
                            let mut best = 0usize;
                            for (k, &c) in counts.iter().enumerate() {
                                if c > counts[best] {
                                    best = k;
                                }
                            }
                            o = best as u8;
                            break;
                        }
                    }
                    self.byte_put(m, x, y, z, o, name);
                }
            }
        }
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn most_frequent_smallest_of_a_tie() {
        assert_eq!(most_frequent(&[]), None);
        assert_eq!(most_frequent(&[3, 2, 3, 2]), Some(2));
        assert_eq!(most_frequent(&[5, 4, 5]), Some(5));
        assert_eq!(most_frequent(&[-1, 7]), Some(-1));
    }

    #[test]
    fn empty_box_grows_to_extent_one_only() {
        let (lo, hi) = box_start(1);
        assert_eq!(grow(lo, hi, 1), (0, 0));
        let (lo, hi) = box_start(2);
        let (a, b) = grow(lo, hi, 2);
        assert!(a > b);
        assert!(super::super::setops::any_voxel((1, 1, 1)));
    }
}
