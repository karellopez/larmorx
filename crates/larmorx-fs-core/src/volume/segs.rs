// SPDX-License-Identifier: Apache-2.0
//! Segment statistics, borders, concatenation, edits and shifts (spec §56): `MRIsegBoundary`,
//! `MRIsegBorder`, `MRIsegStatsRobust`, `MRIsegFrameAvg`, `MRIindexNo`, `MRIzconcat` (FS-512),
//! `MRIapplyEdits`, `MRIshiftDim` (FS-513), `MRIfixEntoWM` and `LabelAmygalaCortalJunction`
//! (FS-514). `MRIvol2VolTkRegVSM` waits for §41 (part 9).

use larmorx_core::defects::{FS_512, FS_513, FS_514};
use larmorx_core::glibc_qsort::qsort;

use super::sample::cvtt;
use super::{Dst, VolOut, Vols};
use crate::clib::printf::sprintf_f64;
use crate::mri::{ColorTable, ColorTableEntry, Mri, types};
use crate::process::{Process, Stop};
use crate::sse;

/// A volume's extent.
fn extent(m: &Mri) -> (i32, i32, i32) {
    (m.geom.width, m.geom.height, m.geom.depth)
}

/// A label as §56 reads it: `MRIgetVoxVal`'s `float` stored in an `int` (truncated; NaN and
/// values past `int`'s range the `int` minimum).
fn as_label(v: f32) -> i32 {
    cvtt(f64::from(v))
}

/// The robust statistics' comparison of two `float`s (a NaN equals everything).
fn compare_floats(a: &f32, b: &f32) -> i32 {
    if a < b {
        -1
    } else if a > b {
        1
    } else {
        0
    }
}

/// The first and last labels of the junction's colour table.
const ACJ_LEFT: i32 = 7030;
const ACJ_RIGHT: i32 = 7031;

impl Process {
    /// A label of frame 0.
    fn seg_label(&mut self, m: &Mri, c: i32, r: i32, s: i32) -> Result<i32, Stop> {
        Ok(as_label(self.vf(m, (c, r, s), 0)?))
    }

    /// `MRIsegBoundary(seg)` (§56): a clone (zeros) whose inner voxels hold their label where
    /// any of the 27 voxels around them has another label.
    pub fn mri_seg_boundary(&mut self, seg: &Mri) -> Result<Mri, Stop> {
        let mut out = self.mri_clone_new(seg)?;
        let (w, h, d) = extent(seg);
        for c in 1..w - 1 {
            for r in 1..h - 1 {
                for s in 1..d - 1 {
                    let label = self.seg_label(seg, c, r, s)?;
                    let mut other = false;
                    'n: for dc in -1..=1 {
                        for dr in -1..=1 {
                            for ds in -1..=1 {
                                if self.seg_label(seg, c + dc, r + dr, s + ds)? != label {
                                    other = true;
                                    break 'n;
                                }
                            }
                        }
                    }
                    if other {
                        self.mri_set_vox_val(&mut out, c, r, s, 0, label as f32, 0)?;
                    }
                }
            }
        }
        Ok(out)
    }

    /// `MRIsegBorder(seg, &maxlabel, topo)` (§56): an `int` volume of `1 + n` frames (`n` 6, 18,
    /// 26 for the topologies 1 to 3, else 0); frame 0 is 1 where a neighbour within the volume
    /// and at most `topo` steps away has another label, frames 1 on list those labels in the
    /// order met. The largest label (from 0) into `maxlabel`; `nhits` printed.
    pub fn mri_seg_border(
        &mut self,
        seg: &Mri,
        maxlabel: &mut i32,
        topo: i32,
    ) -> Result<Mri, Stop> {
        let n = match topo {
            1 => 6,
            2 => 18,
            3 => 26,
            _ => 0,
        };
        let (w, h, d) = extent(seg);
        let mut out = self.alloc_with_header(seg, (w, h, d), types::INT, 1 + n)?;
        let mut maxl = 0;
        let mut nhits: i64 = 0;
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let label = self.seg_label(seg, c, r, s)?;
                    maxl = maxl.max(label);
                    let mut listed = 0;
                    for dc in -1i32..=1 {
                        for dr in -1i32..=1 {
                            for ds in -1i32..=1 {
                                if (dc, dr, ds) == (0, 0, 0)
                                    || dc.abs() + dr.abs() + ds.abs() > topo
                                {
                                    continue;
                                }
                                let (x, y, z) = (c + dc, r + dr, s + ds);
                                if x < 0 || x >= w || y < 0 || y >= h || z < 0 || z >= d {
                                    continue;
                                }
                                let nl = self.seg_label(seg, x, y, z)?;
                                if nl == label {
                                    continue;
                                }
                                listed += 1;
                                nhits += 1;
                                self.mri_set_vox_val(&mut out, c, r, s, 0, 1.0, 0)?;
                                self.mri_set_vox_val(&mut out, c, r, s, listed, nl as f32, 0)?;
                            }
                        }
                    }
                }
            }
        }
        *maxlabel = maxl;
        self.put_out(format!("nhits = {nhits}, maxlabel={maxl}\n").as_bytes());
        Ok(out)
    }

    /// The values of `frame` where frame 0 of `seg` is `segid` (columns outermost).
    fn seg_values(
        &mut self,
        seg: &Mri,
        segid: i32,
        mri: &Mri,
        frame: i32,
    ) -> Result<Vec<f32>, Stop> {
        let (w, h, d) = extent(seg);
        let mut v = Vec::new();
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    if self.seg_label(seg, c, r, s)? == segid {
                        v.push(self.vf(mri, (c, r, s), frame)?);
                    }
                }
            }
        }
        Ok(v)
    }

    /// `MRIsegStatsRobust(seg, segid, mri, frame, &min, &max, &range, &mean, &std, Pct)` (§56):
    /// the segment's values sorted (glibc's merge order, `float` comparisons), the `k`-th kept
    /// when `k` is within `[Pct n / 100, (100 - Pct) n / 100]`; minimum, maximum, range and
    /// standard deviation as `MRIsegStats` (§48) over the kept values, the mean `sum / m` without
    /// a test (every value trimmed: the default NaN). Returns the kept count; `out` = min, max,
    /// range, mean, std.
    pub fn mri_seg_stats_robust(
        &mut self,
        seg: &Mri,
        segid: i32,
        mri: &Mri,
        frame: i32,
        pct: f32,
        out: &mut [f32; 5],
    ) -> Result<i32, Stop> {
        let mut v = self.seg_values(seg, segid, mri, frame)?;
        let n = v.len();
        if n == 0 {
            *out = [0.0; 5];
            return Ok(0);
        }
        qsort(&mut v, compare_floats);
        let nf = n as f32;
        let lo = sse::div64(sse::widen(sse::mul32(pct, nf)), 100.0);
        let hi = sse::div64(sse::widen(sse::mul32(sse::sub32(100.0, pct), nf)), 100.0);
        let kept: Vec<f32> = v
            .iter()
            .enumerate()
            .filter(|&(k, _)| !((k as f64) < lo) && !((k as f64) > hi))
            .map(|(_, &x)| x)
            .collect();
        let m = kept.len();
        let (mut min, mut max) = (0f32, 0f32);
        let (mut sum, mut sum2) = (0f64, 0f64);
        for (i, &x) in kept.iter().enumerate() {
            if i == 0 {
                (min, max) = (x, x);
            } else {
                if x < min {
                    min = x;
                }
                if x > max {
                    max = x;
                }
            }
            let xd = sse::widen(x);
            sum = sse::add64(sum, xd);
            sum2 = sse::add64(sse::mul64(xd, xd), sum2);
        }
        let range = sse::sub32(max, min);
        let mean = sse::narrow(sse::div64(sum, m as f64));
        let std = if m > 1 {
            let a = sse::mul32(sse::mul32(m as f32, mean), mean);
            let b = sse::mul64(sse::widen(sse::add32(mean, mean)), sum);
            let num = sse::add64(sse::sub64(sse::widen(a), b), sum2);
            sse::narrow(sse::sqrt64(sse::div64(num, (m - 1) as f64)))
        } else {
            0.0
        };
        *out = [min, max, range, mean, std];
        Ok(m as i32)
    }

    /// `MRIsegFrameAvg(seg, segid, mri, favg)` (§56): each frame's mean over the segment (the
    /// running sum in `double`, then divided by the count); all 0 for an empty segment. Returns
    /// the count; `favg` takes one mean per frame of `mri`.
    pub fn mri_seg_frame_avg(
        &mut self,
        seg: &Mri,
        segid: i32,
        mri: &Mri,
        favg: &mut [f64],
    ) -> Result<i32, Stop> {
        let (w, h, d) = extent(seg);
        let mut sums = vec![0f64; mri.nframes.max(0) as usize];
        let mut n = 0i32;
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    if self.seg_label(seg, c, r, s)? != segid {
                        continue;
                    }
                    n += 1;
                    for (f, sum) in sums.iter_mut().enumerate() {
                        let v = self.vf(mri, (c, r, s), f as i32)?;
                        *sum = sse::add64(*sum, sse::widen(v));
                    }
                }
            }
        }
        for (f, a) in favg.iter_mut().enumerate().take(sums.len()) {
            *a = if n == 0 {
                0.0
            } else {
                sse::div64(sums[f], f64::from(n))
            };
        }
        Ok(n)
    }

    /// `MRIindexNo(in, out)` (§56): frame 0 holds each voxel's index `s h w + r w + c` (`out`
    /// new: `float`, the input's header).
    pub fn mri_index_no(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        let (w, h, d) = extent(v.a());
        if !v.has_dst() {
            let m = self.alloc_with_header(v.a(), (w, h, d), types::FLOAT, 1)?;
            v.set_dst(m);
        }
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let i = s
                        .wrapping_mul(h)
                        .wrapping_mul(w)
                        .wrapping_add(r.wrapping_mul(w))
                        .wrapping_add(c);
                    self.mri_set_vox_val(v.d_mut(), c, r, s, 0, i as f32, 0)?;
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIzconcat(mri1, mri2, nskip, out)` (§56): `mri1`'s slices, then `mri2`'s from `nskip`;
    /// `out` new (`mri1`'s width, height, type, frames and header, `d1 + d2 - nskip` slices) or
    /// given; `mri1`'s pulse parameters and scanner vox2ras. FS-512: FreeSurfer copies as many
    /// rows as there are columns.
    pub fn mri_zconcat(
        &mut self,
        mri1: &mut Mri,
        mri2: &mut Mri,
        nskip: i32,
        dst: Dst,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(mri1, Some(mri2), dst);
        let (w1, h1, d1) = extent(v.a());
        let (w2, h2, d2) = extent(v.b());
        let err = |p: &mut Process, m: String| {
            p.put_out(format!("ERROR: MRIzconcat(): {m}\n").as_bytes());
            Ok(VolOut::Nothing)
        };
        if nskip >= d2 {
            return err(self, format!("nskip={nskip} >= nslices in mri2 {d2}"));
        }
        if w1 != w2 {
            return err(self, format!("mri1 and mri2 mismatch width {w1} {w2}"));
        }
        if h1 != h2 {
            return err(self, format!("mri1 and mri2 mismatch height {h1} {h2}"));
        }
        let (nf, kind) = (v.a().nframes, v.a().kind);
        let depth = d1.wrapping_add(d2).wrapping_sub(nskip);
        if v.has_dst() {
            let o = v.d();
            let (ow, oh, od, of) = (o.geom.width, o.geom.height, o.geom.depth, o.nframes);
            if of != nf {
                return err(self, format!("out nframes mismatch {nf} {of}"));
            }
            if ow != w1 {
                return err(self, format!("out width mismatch {w1} {ow}"));
            }
            if oh != h1 {
                return err(self, format!("out height mismatch {h1} {oh}"));
            }
            if od != depth {
                return err(self, format!("out depth mismatch {depth} {od}"));
            }
        } else {
            let m = self.alloc_with_header(v.a(), (w1, h1, depth), kind, nf)?;
            v.set_dst(m);
        }
        let a = v.a().clone();
        self.copy_pulse_parameters(&a, v.d_mut());
        let vox = self.vox2ras_scanner(&a.geom, 0, false)?;
        let mut g = v.d().geom.clone();
        self.set_vox2ras_from_matrix(&mut g, &vox)?;
        v.d_mut().geom = g;
        let rows = if self.mode().reproduce() { w1 } else { h1 };
        if w1 != h1 {
            if self.mode().reproduce() {
                self.note(
                    &FS_512,
                    "MRIzconcat: as many rows copied as there are columns (FreeSurfer's)",
                );
            } else {
                self.note(
                    &FS_512,
                    "MRIzconcat: every row copied (FreeSurfer copies as many rows as there are columns)",
                );
            }
        }
        for f in 0..nf {
            for c in 0..w1 {
                for r in 0..rows {
                    for s in 0..d1 {
                        let x = self.vf(v.a(), (c, r, s), f)?;
                        self.mri_set_vox_val(v.d_mut(), c, r, s, f, x, 0)?;
                    }
                    for s in nskip..d2 {
                        let x = self.vf(v.b(), (c, r, s), f)?;
                        let z = d1.wrapping_add(s).wrapping_sub(nskip);
                        self.mri_set_vox_val(v.d_mut(), c, r, z, f, x, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIapplyEdits(newauto, oldauto, manedit, outvol)` (§56): every voxel of every frame
    /// `newauto`'s where `oldauto` equals `manedit`, else `manedit`'s (counted); `outvol` new
    /// (`oldauto`'s grid, type, frames, header and pulse parameters) or given.
    pub fn mri_apply_edits(
        &mut self,
        newauto: &mut Mri,
        oldauto: &mut Mri,
        manedit: &Mri,
        dst: Dst,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(newauto, Some(oldauto), dst);
        let (w, h, d) = extent(v.b());
        let nf = v.b().nframes;
        if !v.has_dst() {
            let kind = v.b().kind;
            let mut m = self.alloc_with_header(v.b(), (w, h, d), kind, nf)?;
            let b = v.b().clone();
            self.copy_pulse_parameters(&b, &mut m);
            v.set_dst(m);
        }
        let mut n: i64 = 0;
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    for f in 0..nf {
                        let vn = self.vf(v.a(), (c, r, s), f)?;
                        let vo = self.vf(v.b(), (c, r, s), f)?;
                        let vm = self.vf(manedit, (c, r, s), f)?;
                        let x = if vo == vm {
                            vn
                        } else {
                            n += 1;
                            vm
                        };
                        self.mri_set_vox_val(v.d_mut(), c, r, s, f, x, 0)?;
                    }
                }
            }
        }
        self.put_out(format!("MRIapplyEdits(): copying {n} from man to new\n").as_bytes());
        Ok(v.out())
    }

    /// `MRIshiftDim(src, dim, nshift, wrap)` (§56): a new volume (type, frames, header, pulse
    /// parameters, colour table) whose voxel at index + `nshift` along `dim` is the source's,
    /// wrapped once or dropped when it leaves the axis. FS-513: FreeSurfer wraps and drops rows
    /// and slices by the width.
    pub fn mri_shift_dim(
        &mut self,
        src: &Mri,
        dim: i32,
        nshift: i32,
        wrap: i32,
    ) -> Result<Option<Mri>, Stop> {
        self.put_out(
            format!("MRIshiftDim() dim = {dim} nshift = {nshift}  wrap = {wrap}\n").as_bytes(),
        );
        if !(1..=3).contains(&dim) {
            self.put_out(b"MRIshiftDim() dim out of range\n");
            return Ok(None);
        }
        let (w, h, d) = extent(src);
        let mut out = self.alloc_with_header(src, (w, h, d), src.kind, src.nframes)?;
        self.copy_pulse_parameters(src, &mut out);
        out.ct = src.ct.as_ref().and_then(|ct| self.ctab_deep_copy(Some(ct)));
        let own = [w, h, d][dim as usize - 1];
        let by = if self.mode().reproduce() { w } else { own };
        if own != w && dim != 1 {
            if self.mode().reproduce() {
                self.note(
                    &FS_513,
                    "MRIshiftDim: rows or slices wrapped and dropped by the width (FreeSurfer's)",
                );
            } else {
                self.note(
                    &FS_513,
                    "MRIshiftDim: rows or slices wrapped and dropped by their own extent (FreeSurfer uses the width)",
                );
            }
        }
        let shift = |i: i32| -> Option<i32> {
            let mut j = i.wrapping_add(nshift);
            if wrap != 0 {
                if j < 0 {
                    j = j.wrapping_add(by);
                } else if j >= by {
                    j = j.wrapping_sub(by);
                }
                Some(j)
            } else if j < 0 || j >= by {
                None
            } else {
                Some(j)
            }
        };
        for f in 0..src.nframes {
            for c in 0..w {
                for r in 0..h {
                    for s in 0..d {
                        let mut to = [c, r, s];
                        let k = dim as usize - 1;
                        let Some(j) = shift(to[k]) else {
                            continue;
                        };
                        to[k] = j;
                        let x = self.vf(src, (c, r, s), f)?;
                        self.mri_set_vox_val(&mut out, to[0], to[1], to[2], f, x, 0)?;
                    }
                }
            }
        }
        Ok(Some(out))
    }

    /// `MRIfixEntoWM(invol, entowm, Level, lhVal, rhVal, ACJ)` (§56): frame 0 of `invol` set to
    /// `lhVal` where `entowm` is 3006 (levels 1, 3) or 3201 (7030 with `ACJ`; levels 2, 3), to
    /// `rhVal` for 4006 and 4201 (7031). Returns the count.
    pub fn mri_fix_ento_wm(
        &mut self,
        invol: &mut Mri,
        entowm: &Mri,
        level: i32,
        lh: f64,
        rh: f64,
        acj: i32,
    ) -> Result<i32, Stop> {
        let mut b = b"MRIfixEntoWM(): ".to_vec();
        b.extend_from_slice(&sprintf_f64(b"%g", lh));
        b.push(b' ');
        b.extend_from_slice(&sprintf_f64(b"%g", rh));
        b.extend_from_slice(format!(" Level={level}\n").as_bytes());
        self.put_out(&b);
        let (wm_l, wm_r) = if acj != 0 { (7030, 7031) } else { (3201, 4201) };
        let (l1, l2) = (level == 1 || level == 3, level == 2 || level == 3);
        let (w, h, d) = extent(entowm);
        let mut n = 0;
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let e = self.seg_label(entowm, c, r, s)?;
                    let v = if (e == 3006 && l1) || (e == wm_l && l2) {
                        lh
                    } else if (e == 4006 && l1) || (e == wm_r && l2) {
                        rh
                    } else {
                        continue;
                    };
                    self.mri_set_vox_val(invol, c, r, s, 0, sse::narrow(v), 0)?;
                    n += 1;
                }
            }
        }
        self.put_out(format!("MRIfixEntoWM(): nchanged = {n}\n").as_bytes());
        Ok(n)
    }

    /// `CTABalloc(n)` (`specs/freesurfer-io.md` §69): `n` entries of random colours named
    /// `cluster<i>`, then `CTABunique(table, 100)` (entry 0 `unknown`). A negative `n`: the
    /// error and none.
    pub fn ctab_alloc(&mut self, n: i32) -> Result<Option<ColorTable>, Stop> {
        if n < 0 {
            self.error_printf(
                crate::process::codes::BAD_PARM,
                format!("CTABalloc: nentries was {n}").as_bytes(),
            );
            return Ok(None);
        }
        let mut ct = ColorTable {
            entries: Vec::with_capacity(n as usize),
            fname: b"none".to_vec(),
            version: 2,
            idbase: 0,
            schema: Vec::new(),
            tissue: None,
        };
        for i in 0..n {
            let mut e = ColorTableEntry {
                name: format!("cluster{i}").into_bytes(),
                ..Default::default()
            };
            self.ctab_draw(&mut e)?;
            ct.entries.push(Some(e));
        }
        self.ctab_unique(&mut ct, 100)?;
        Ok(Some(ct))
    }

    /// One entry's random colour (`specs/freesurfer-io.md` §69): red, green, blue each
    /// `nint(randomNumber(0, 255))`, alpha 255, the float colours.
    fn ctab_draw(&mut self, e: &mut ColorTableEntry) -> Result<(), Stop> {
        for k in 0..3 {
            let x = self.random_number(0.0, 255.0)?;
            e.rgbai[k] = self.nint(x);
        }
        e.rgbai[3] = 255;
        for k in 0..4 {
            e.rgbaf[k] = sse::narrow(sse::div64(f64::from(e.rgbai[k] as f32), 255.0));
        }
        Ok(())
    }

    /// `CTABunique(table, nmax)` (`specs/freesurfer-io.md` §69): up to `nmax` tries of new
    /// colours until no two entries share one; entry 0 `unknown`; -1 (and the INFO line) when
    /// the tries made equal `nmax`.
    fn ctab_unique(&mut self, ct: &mut ColorTable, nmax: i32) -> Result<i32, Stop> {
        let mut tries = 0;
        while tries < nmax {
            tries += 1;
            for e in ct.entries.iter_mut().flatten() {
                self.ctab_draw(e)?;
            }
            let mut seen = std::collections::HashSet::new();
            let repeats = ct
                .entries
                .iter()
                .flatten()
                .any(|e| !seen.insert([e.rgbai[0], e.rgbai[1], e.rgbai[2]]));
            if !repeats {
                break;
            }
        }
        if let Some(Some(e)) = ct.entries.first_mut() {
            e.name = b"unknown".to_vec();
            e.rgbai = [0; 4];
            e.rgbaf = [0.0; 4];
        }
        if tries == nmax {
            self.put_out(
                format!("INFO: CTABunique() could not find a unique set in {nmax} tries\n")
                    .as_bytes(),
            );
            return Ok(-1);
        }
        Ok(0)
    }

    /// `LabelAmygalaCortalJunction(seg, topo, out)` (§56): cortex voxels (3, 42) within `topo`
    /// steps of an inner amygdala voxel (18 left, 54 right) get 7030 (left) or 7031 (right) in
    /// frame 0 of `out` (new: `int`, the segmentation's header and pulse parameters, a colour
    /// table of 7,032 places of which only the two are kept). FS-514: the later write stays.
    pub fn label_amygala_cortal_junction(
        &mut self,
        seg: &Mri,
        topo: i32,
        out: Option<&mut Mri>,
    ) -> Result<VolOut, Stop> {
        let (w, h, d) = extent(seg);
        let mut own = None;
        let dst: &mut Mri = match out {
            Some(m) => m,
            None => {
                let mut m = self.alloc_with_header(seg, (w, h, d), types::INT, 1)?;
                self.copy_pulse_parameters(seg, &mut m);
                let mut ct = self.ctab_alloc(ACJ_RIGHT + 1)?;
                if let Some(ct) = ct.as_mut() {
                    for (i, e) in ct.entries.iter_mut().enumerate() {
                        let i = i as i32;
                        if i != ACJ_LEFT && i != ACJ_RIGHT {
                            *e = None;
                        } else if let Some(e) = e {
                            let (name, red): (&[u8], i32) = if i == ACJ_LEFT {
                                (b"Left-Amygdala-Cortical-Junction", 255)
                            } else {
                                (b"Right-Amygdala-Cortical-Junction", 254)
                            };
                            e.name = name.to_vec();
                            e.rgbai[0] = red;
                            e.rgbai[1] = 85;
                            e.rgbai[2] = 255;
                        }
                    }
                }
                m.ct = ct;
                own.insert(m)
            }
        };
        let mut nhits = 0;
        let mut written: std::collections::HashMap<(i32, i32, i32), i32> =
            std::collections::HashMap::new();
        let mut both = false;
        for c in 1..w - 1 {
            for r in 1..h - 1 {
                for s in 1..d - 1 {
                    let side = match self.seg_label(seg, c, r, s)? {
                        18 => ACJ_LEFT,
                        54 => ACJ_RIGHT,
                        _ => continue,
                    };
                    for dc in -1i32..=1 {
                        for dr in -1i32..=1 {
                            for ds in -1i32..=1 {
                                if dc.abs() + dr.abs() + ds.abs() > topo {
                                    continue;
                                }
                                let (x, y, z) = (c + dc, r + dr, s + ds);
                                let l = self.seg_label(seg, x, y, z)?;
                                if l != 3 && l != 42 {
                                    continue;
                                }
                                if let Some(prev) = written.insert((x, y, z), side)
                                    && prev != side
                                {
                                    both = true;
                                }
                                self.mri_set_vox_val(dst, x, y, z, 0, side as f32, 0)?;
                                nhits += 1;
                            }
                        }
                    }
                }
            }
        }
        if both {
            self.note(
                &FS_514,
                "LabelAmygalaCortalJunction: a cortex voxel next to both amygdalae takes the later write (one thread's order)",
            );
        }
        self.put_out(format!("LabelAmygalaCortalJunction((): nhits = {nhits}\n").as_bytes());
        Ok(match own {
            Some(m) => VolOut::New(m),
            None => VolOut::Given,
        })
    }
}
