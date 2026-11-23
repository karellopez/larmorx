// SPDX-License-Identifier: Apache-2.0
//! Set operations, counts and labels (spec §50): `MRIunion`, `MRIintersect`, `MRIcomplement`,
//! `MRIand`, `MRIxor`, `MRIsetEdges`, `MRInot`, `MRIbinMaskToCol`, the counts
//! (`MRItotalVoxelsOn`, `MRIvoxelsInLabel`, `MRIlabelInVolume`, `MRInMask`,
//! `MRIcountValInNbhd`), `MRIcomputeCentroid`, `MRIpercentThresh` (FS-495, FS-496),
//! `MRIreplaceList`, `MRIreplaceValuesOnly`, `MRIcopyLabel`, `MRIcopyLabeledVoxels` and
//! `MRIsetLabelValues`.
//!
//! §50's common rules: "bytes" are frame 0 read and written one byte per voxel at the voxel's
//! index of its row (the direct walk of §6); "values" the voxel functions (`float`); loops z, y,
//! x unless columns outermost; `(int)` under FS-12; a volume without voxels where it is read or
//! written, FS-58.

use std::cmp::Ordering;

use larmorx_core::defects::{FS_495, FS_496};
use larmorx_core::glibc_qsort::qsort;

use super::sample::FLT_EPSILON;
use super::seqops::compare_doubles;
use super::{Dst, VolOut, Vols};
use crate::maths::Returned;
use crate::mri::{Mri, types};
use crate::process::{Process, Stop};
use crate::sse;

/// A volume's extent.
pub(crate) fn ext(m: &Mri) -> (i32, i32, i32) {
    (m.geom.width, m.geom.height, m.geom.depth)
}

/// Whether an extent holds a voxel.
pub(crate) fn any_voxel((w, h, d): (i32, i32, i32)) -> bool {
    w > 0 && h > 0 && d > 0
}

impl Process {
    /// Frame 0's byte of voxel `(x, y, z)` at its index of the row (§50's "bytes").
    pub(crate) fn byte_at(&mut self, m: &Mri, x: i32, y: i32, z: i32, name: &str) -> u8 {
        self.row_byte(m, (x, y, z), name)
    }

    /// Frame 0's byte of voxel `(x, y, z)` written at its index of the row.
    pub(crate) fn byte_put(&mut self, m: &mut Mri, x: i32, y: i32, z: i32, b: u8, name: &str) {
        let place = self.row_place(m, i64::from(x), i64::from(y), i64::from(z), 0, 1);
        self.direct_write(m, place, &[b], name);
    }

    /// FS-58 for each volume a direct walk over a nonempty extent reads or writes.
    pub(crate) fn walk_needs(
        &mut self,
        e: (i32, i32, i32),
        ms: &[&Mri],
        name: &str,
    ) -> Result<(), Stop> {
        if any_voxel(e) {
            for m in ms {
                self.direct_needs_voxels(m, name)?;
            }
        }
        Ok(())
    }

    /// `MRIcheckVolDims(a, dst)` when the destination exists.
    pub(crate) fn check_a_d(&mut self, v: &Vols) -> Result<(), Stop> {
        if v.has_dst() {
            self.check_vol_dims(v.a(), v.d())?;
        }
        Ok(())
    }

    /// `dst = MRIcopy(src, dst)` on a pair: a given destination copied into (the source itself:
    /// nothing changes), else a new copy; `false` when the copy gives none.
    pub(crate) fn copy_to_dst(&mut self, v: &mut Vols) -> Result<bool, Stop> {
        if v.d_is_a() {
            return Ok(true);
        }
        if v.has_dst() {
            let s = v.a().clone();
            return Ok(!self.mri_copy(&s, Some(v.d_mut()))?.is_nothing());
        }
        match self.mri_copy(v.a(), None)? {
            Returned::New(m) => {
                v.set_dst(m);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// `MRIunion(a, b, dst)` (§50): checked; a clone of `a` without `dst`; bytes, the larger.
    pub fn mri_union(&mut self, a: &mut Mri, b: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        self.byte_pair(a, b, dst, "MRIunion", u8::max)
    }

    fn byte_pair(
        &mut self,
        a: &mut Mri,
        b: &mut Mri,
        dst: Dst,
        name: &str,
        f: fn(u8, u8) -> u8,
    ) -> Result<VolOut, Stop> {
        self.check_vol_dims(a, b)?;
        let mut v = Vols::new(a, Some(b), dst);
        self.clone_dst(&mut v)?;
        let e = ext(v.a());
        self.walk_needs(e, &[v.a(), v.b(), v.d()], name)?;
        let (w, h, d) = e;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let va = self.byte_at(v.a(), x, y, z, name);
                    let vb = self.byte_at(v.b(), x, y, z, name);
                    self.byte_put(v.d_mut(), x, y, z, f(va, vb), name);
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIintersect(a, b, dst)` (§50): checked; a clone; for each frame of `dst` the smaller of
    /// the two `(int)` values, as a value.
    pub fn mri_intersect(&mut self, a: &mut Mri, b: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        self.check_vol_dims(a, b)?;
        let mut v = Vols::new(a, Some(b), dst);
        self.clone_dst(&mut v)?;
        let (w, h, d) = ext(v.a());
        for f in 0..v.d().nframes {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let va = self.mri_get_vox_val(v.a(), x, y, z, f, 0)?;
                        let ia = self.int_fs12(sse::widen(va));
                        let vb = self.mri_get_vox_val(v.b(), x, y, z, f, 0)?;
                        let ib = self.int_fs12(sse::widen(vb));
                        self.mri_set_vox_val(v.d_mut(), x, y, z, f, ia.min(ib) as f32, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIcomplement(src, dst)` (§50): checked; without `dst` one frame of the source's extent and
    /// type with its header; for each frame of `src`, 1 where `(int) v` is 0, else 0.
    pub fn mri_complement(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        if v.has_dst() {
            self.check_vol_dims(v.a(), v.d())?;
        } else {
            let k = v.a().kind;
            let m = self.new_with_header(v.a(), k, 1)?;
            v.set_dst(m);
        }
        let (w, h, d) = ext(v.a());
        for f in 0..v.a().nframes {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let s = self.mri_get_vox_val(v.a(), x, y, z, f, 0)?;
                        let o = if self.int_fs12(sse::widen(s)) == 0 {
                            1.0
                        } else {
                            0.0
                        };
                        self.mri_set_vox_val(v.d_mut(), x, y, z, f, o, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIand(a, b, dst, thresh)` (§50): checked; a clone; for each frame of `a` each voxel's
    /// nearest samples (`MRIsampleVolumeFrame` at whole coordinates) taken as 0 below `thresh`;
    /// 1 where both are not 0 (a NaN is not 0), else 0.
    pub fn mri_and(
        &mut self,
        a: &mut Mri,
        b: &mut Mri,
        dst: Dst,
        thresh: i32,
    ) -> Result<VolOut, Stop> {
        self.check_vol_dims(a, b)?;
        let mut v = Vols::new(a, Some(b), dst);
        self.clone_dst(&mut v)?;
        let t = f64::from(thresh);
        let (w, h, d) = ext(v.a());
        for f in 0..v.a().nframes {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let (fx, fy, fz) = (f64::from(x), f64::from(y), f64::from(z));
                        let mut va = 0.0;
                        self.mri_sample_volume_frame(v.a(), fx, fy, fz, f, &mut va)?;
                        let mut vb = 0.0;
                        self.mri_sample_volume_frame(v.b(), fx, fy, fz, f, &mut vb)?;
                        if va < t {
                            va = 0.0;
                        }
                        if vb < t {
                            vb = 0.0;
                        }
                        let o = if va != 0.0 && vb != 0.0 { 1.0 } else { 0.0 };
                        self.mri_set_vox_val(v.d_mut(), x, y, z, f, o, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIxor(a, b, dst, t1, t2)` (§50): checked; a clone; frame 0: a voxel is on when its `(int)`
    /// value lies in `[t1, t2]`; 1 where exactly one is.
    pub fn mri_xor(
        &mut self,
        a: &mut Mri,
        b: &mut Mri,
        dst: Dst,
        t1: i32,
        t2: i32,
    ) -> Result<VolOut, Stop> {
        self.check_vol_dims(a, b)?;
        let mut v = Vols::new(a, Some(b), dst);
        self.clone_dst(&mut v)?;
        let (w, h, d) = ext(v.a());
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let va = self.mri_get_vox_val(v.a(), x, y, z, 0, 0)?;
                    let ia = self.int_fs12(sse::widen(va));
                    let vb = self.mri_get_vox_val(v.b(), x, y, z, 0, 0)?;
                    let ib = self.int_fs12(sse::widen(vb));
                    let on_a = t1 <= ia && ia <= t2;
                    let on_b = t1 <= ib && ib <= t2;
                    let o = if on_a != on_b { 1.0 } else { 0.0 };
                    self.mri_set_vox_val(v.d_mut(), x, y, z, 0, o, 0)?;
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIsetEdges(in, val, out)` (§50): `MRIcopy(in, out)` (none: none), the pulse parameters and
    /// colour table copied; every voxel of every frame on the six faces `(float) val`.
    pub fn mri_set_edges(&mut self, src: &mut Mri, val: f64, dst: Dst) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        if !self.copy_to_dst(&mut v)? {
            return Ok(VolOut::Nothing);
        }
        let s = v.a().clone();
        self.copy_pulse_parameters(&s, v.d_mut());
        if let Some(ct) = &s.ct {
            let c = self.ctab_deep_copy(Some(ct));
            v.d_mut().ct = c;
        }
        let fv = sse::narrow(val);
        let (w, h, d) = ext(v.d());
        for f in 0..v.d().nframes {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let face =
                            x == 0 || y == 0 || z == 0 || x == w - 1 || y == h - 1 || z == d - 1;
                        if face {
                            self.mri_set_vox_val(v.d_mut(), x, y, z, f, fv, 0)?;
                        }
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRInot(src, dst)` (§50): checked; a clone; columns outermost, 1 where `nint(v)` is 0,
    /// else 0.
    pub fn mri_not(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        self.check_a_d(&v)?;
        self.clone_dst(&mut v)?;
        let (w, h, d) = ext(v.a());
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    let s = self.mri_get_vox_val(v.a(), x, y, z, 0, 0)?;
                    let o = if self.nint(sse::widen(s)) == 0 {
                        1.0
                    } else {
                        0.0
                    };
                    self.mri_set_vox_val(v.d_mut(), x, y, z, 0, o, 0)?;
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIbinMaskToCol(mask, col)` (§50): without `col` an int volume of the mask's extent and
    /// frames with its header; another `col` of other dimensions (frames compared) refused; frame
    /// 0, the column where the mask is not below 0.5, else 0.
    pub fn mri_bin_mask_to_col(&mut self, mask: &mut Mri, col: Dst) -> Result<VolOut, Stop> {
        let mut v = Vols::new(mask, None, col);
        if v.has_dst() {
            if Process::mri_dim_mismatch(v.a(), v.d(), true) != 0 {
                self.put_out(b"ERROR: MRIbinMaskToCol: input/output dim mismatch\n");
                return Ok(VolOut::Nothing);
            }
        } else {
            let nf = v.a().nframes;
            let mut m = self.new_with_header(v.a(), types::INT, nf)?;
            let s = v.a().clone();
            self.copy_pulse_parameters(&s, &mut m);
            v.set_dst(m);
        }
        let (w, h, d) = ext(v.a());
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let m = self.mri_get_vox_val(v.a(), x, y, z, 0, 0)?;
                    let o = if m < 0.5 { 0.0 } else { x as f32 };
                    self.mri_set_vox_val(v.d_mut(), x, y, z, 0, o, 0)?;
                }
            }
        }
        Ok(v.out())
    }

    /// `MRItotalVoxelsOn(mri, t)` (§50): the values of every frame at or above `(float) t`.
    pub fn mri_total_voxels_on(&mut self, m: &Mri, t: i32) -> Result<i32, Stop> {
        let t = t as f32;
        let (w, h, d) = ext(m);
        let mut n = 0;
        for f in 0..m.nframes {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        if self.mri_get_vox_val(m, x, y, z, f, 0)? >= t {
                            n += 1;
                        }
                    }
                }
            }
        }
        Ok(n)
    }

    /// `MRIvoxelsInLabel(mri, l)` (§50): the frame-0 values with `nint(v) == l`.
    pub fn mri_voxels_in_label(&mut self, m: &Mri, l: i32) -> Result<i32, Stop> {
        let (w, h, d) = ext(m);
        let mut n = 0;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let v = self.mri_get_vox_val(m, x, y, z, 0, 0)?;
                    if self.nint(sse::widen(v)) == l {
                        n += 1;
                    }
                }
            }
        }
        Ok(n)
    }

    /// `MRIlabelInVolume(src, l)` (§50): 1 at the first frame-0 `(int)` value equal to `l`
    /// (columns outermost), else 0.
    pub fn mri_label_in_volume(&mut self, m: &Mri, l: i32) -> Result<i32, Stop> {
        let (w, h, d) = ext(m);
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    let v = self.mri_get_vox_val(m, x, y, z, 0, 0)?;
                    if self.int_fs12(sse::widen(v)) == l {
                        return Ok(1);
                    }
                }
            }
        }
        Ok(0)
    }

    /// `MRInMask(mask)` (§50): the values of every frame above 0.5.
    pub fn mri_n_mask(&mut self, m: &Mri) -> Result<i32, Stop> {
        let (w, h, d) = ext(m);
        let mut n = 0;
        for f in 0..m.nframes {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        if self.mri_get_vox_val(m, x, y, z, f, 0)? > 0.5 {
                            n += 1;
                        }
                    }
                }
            }
        }
        Ok(n)
    }

    /// `MRIcountValInNbhd(mri, wsize, x, y, z, val)` (§50): over the cube of offsets
    /// `-whalf..whalf` (z, y, x; coordinates through the index tables, FS-323 past their margin),
    /// the frame-0 values whose `(int)` is `val`.
    pub fn mri_count_val_in_nbhd(
        &mut self,
        m: &Mri,
        wsize: i32,
        (x, y, z): (i32, i32, i32),
        val: i32,
    ) -> Result<i32, Stop> {
        let name = "MRIcountValInNbhd";
        let whalf = (wsize - 1) / 2;
        let (w, h, d) = ext(m);
        let mut n = 0;
        for zk in -whalf..=whalf {
            let zi = self.table_index(d, z.wrapping_add(zk), name)?;
            for yk in -whalf..=whalf {
                let yi = self.table_index(h, y.wrapping_add(yk), name)?;
                for xk in -whalf..=whalf {
                    let xi = self.table_index(w, x.wrapping_add(xk), name)?;
                    let v = self.mri_get_vox_val(m, xi, yi, zi, 0, 0)?;
                    if self.int_fs12(sse::widen(v)) == val {
                        n += 1;
                    }
                }
            }
        }
        Ok(n)
    }

    /// `MRIcomputeCentroid(mri)` (§50): columns outermost, the frame-0 values whose magnitude is
    /// not below `FLT_EPSILON` weigh their coordinates; divided by their sum when it is above 0.
    pub fn mri_compute_centroid(&mut self, m: &Mri) -> Result<(i32, [f64; 3]), Stop> {
        use sse::{add64, div64, mul64};
        let (w, h, d) = ext(m);
        let (mut norm, mut c) = (0.0f64, [0.0f64; 3]);
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    let v = sse::widen(self.mri_get_vox_val(m, x, y, z, 0, 0)?);
                    if v.abs() < FLT_EPSILON {
                        continue;
                    }
                    norm = add64(norm, v);
                    c[0] = add64(mul64(f64::from(x), v), c[0]);
                    c[1] = add64(mul64(f64::from(y), v), c[1]);
                    c[2] = add64(mul64(f64::from(z), v), c[2]);
                }
            }
        }
        if norm > 0.0 {
            for k in &mut c {
                *k = div64(*k, norm);
            }
        }
        Ok((0, c))
    }

    /// `MRIpercentThresh(mri, mask, frame, pct)` (§50): the `frame` values where the mask is not
    /// below 0.5 (columns outermost) in glibc's `qsort` order, the value at
    /// `npct = (int) round(n (100 - pct) / 100)`. FS-495 (`npct` from `n` to the voxel count
    /// minus 1) and FS-496 (below 0, at or past the voxel count, NaN): the largest value by default
    /// (0 for none), 0 with `reproduce`.
    pub fn mri_percent_thresh(
        &mut self,
        m: &Mri,
        mask: Option<&Mri>,
        frame: i32,
        pct: f64,
    ) -> Result<f64, Stop> {
        let (w, h, d) = ext(m);
        let mut list = Vec::new();
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    if let Some(k) = mask
                        && self.mri_get_vox_val(k, x, y, z, 0, 0)? < 0.5
                    {
                        continue;
                    }
                    list.push(sse::widen(self.mri_get_vox_val(m, x, y, z, frame, 0)?));
                }
            }
        }
        qsort(&mut list, compare_doubles);
        let n = list.len();
        let nvox = i64::from(w.max(0)) * i64::from(h.max(0)) * i64::from(d.max(0));
        let r = sse::div64(sse::mul64(n as f64, sse::sub64(100.0, pct)), 100.0).round();
        let reproduce = self.mode().reproduce();
        let largest = list.last().copied().unwrap_or(0.0);
        let at = if r.is_nan() || r < 0.0 || r >= nvox as f64 {
            None
        } else {
            Some(r as usize)
        };
        Ok(match at {
            Some(k) if k < n => list[k],
            Some(_) => {
                if reproduce {
                    self.note(
                        &FS_495,
                        "MRIpercentThresh: the percentage reaches the unused end of the list: 0, as FreeSurfer's",
                    );
                    0.0
                } else {
                    self.note(
                        &FS_495,
                        "MRIpercentThresh: the percentage reaches past the sorted values: the largest value (FreeSurfer gives 0)",
                    );
                    largest
                }
            }
            None => {
                if reproduce {
                    self.note(
                        &FS_496,
                        "MRIpercentThresh: the percentage reaches outside the list (FreeSurfer reads memory it does not own): 0",
                    );
                    0.0
                } else {
                    self.note(
                        &FS_496,
                        "MRIpercentThresh: the percentage reaches outside the list: the largest value (FreeSurfer reads memory it does not own)",
                    );
                    largest
                }
            }
        })
    }

    /// `MRIreplaceList(seg, src, targ, n, mask, out)` (§50): without `out` one int frame with the
    /// segmentation's header; another `out` of other dimensions or a byte `out` with a target
    /// above 255 refused; columns outermost, frame 0: the `(int)` label, or (inside the mask) the
    /// target of its first equal source entry, as a value.
    pub fn mri_replace_list(
        &mut self,
        seg: &mut Mri,
        pairs: &[(i32, i32)],
        mask: Option<&mut Mri>,
        out: Dst,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(seg, mask, out);
        if v.has_dst() {
            if Process::mri_dim_mismatch(v.a(), v.d(), false) != 0 {
                self.put_out(b"ERROR: MRIreplaceList(): seg/out dim mismatch\n");
                return Ok(VolOut::Nothing);
            }
        } else {
            let mut m = self.new_with_header(v.a(), types::INT, 1)?;
            let s = v.a().clone();
            self.copy_pulse_parameters(&s, &mut m);
            v.set_dst(m);
        }
        if v.d().kind == types::UCHAR && pairs.iter().any(|&(_, t)| t > 255) {
            self.put_out(b"ERROR: MRIreplaceList(): output is UCHAR, but target is > 255\n");
            return Ok(VolOut::Nothing);
        }
        let has_mask = v.has_b();
        let (w, h, d) = ext(v.a());
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    let l = self.mri_get_vox_val(v.a(), x, y, z, 0, 0)?;
                    let l = self.int_fs12(sse::widen(l));
                    let mut o = l;
                    // not below 0.5 (a NaN mask value is not below)
                    let inside = if has_mask {
                        self.mri_get_vox_val(v.b(), x, y, z, 0, 0)?
                            .partial_cmp(&0.5)
                            != Some(Ordering::Less)
                    } else {
                        true
                    };
                    if inside && let Some(&(_, t)) = pairs.iter().find(|&&(s, _)| s == l) {
                        o = t;
                    }
                    self.mri_set_vox_val(v.d_mut(), x, y, z, 0, o as f32, 0)?;
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIreplaceValuesOnly(src, dst, in, out)` (§50): checked; a clone; two byte volumes go to
    /// `MRIreplaceValuesUchar` with `(byte) nint` of both; else each value of each frame within
    /// `FLT_EPSILON` of `in` becomes `out`, the others not written.
    pub fn mri_replace_values_only(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        vin: f32,
        vout: f32,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        self.check_a_d(&v)?;
        self.clone_dst(&mut v)?;
        if v.a().kind == types::UCHAR && v.d().kind == types::UCHAR {
            let a = self.nint(sse::widen(vin)) as u8;
            let b = self.nint(sse::widen(vout)) as u8;
            self.check_vol_dims(v.a(), v.d())?;
            self.replace_bytes(&mut v, a, b)?;
            return Ok(v.out());
        }
        let eps = FLT_EPSILON as f32;
        let (w, h, d) = ext(v.a());
        let nf = v.a().nframes;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    for f in 0..nf {
                        let s = self.mri_get_vox_val(v.a(), x, y, z, f, 0)?;
                        if sse::sub32(s, vin).abs() < eps {
                            self.mri_set_vox_val(v.d_mut(), x, y, z, f, vout, 0)?;
                        }
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIcopyLabel(src, dst, label)` (§50): checked; frame 0; two byte volumes: the bytes equal
    /// to `label` copied as `(byte) label`; else the values equal to `(float) label`. The count.
    pub fn mri_copy_label(&mut self, src: &mut Mri, dst: Dst, label: i32) -> Result<i32, Stop> {
        let name = "MRIcopyLabel";
        let mut v = Vols::new(src, None, dst);
        self.check_a_d(&v)?;
        self.clone_dst(&mut v)?;
        let e = ext(v.a());
        let (w, h, d) = e;
        let mut n = 0;
        if v.a().kind == types::UCHAR && v.d().kind == types::UCHAR {
            self.walk_needs(e, &[v.a(), v.d()], name)?;
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        if i32::from(self.byte_at(v.a(), x, y, z, name)) == label {
                            self.byte_put(v.d_mut(), x, y, z, label as u8, name);
                            n += 1;
                        }
                    }
                }
            }
            return Ok(n);
        }
        let l = label as f32;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if self.mri_get_vox_val(v.a(), x, y, z, 0, 0)? == l {
                        self.mri_set_vox_val(v.d_mut(), x, y, z, 0, l, 0)?;
                        n += 1;
                    }
                }
            }
        }
        Ok(n)
    }

    /// `MRIcopyLabeledVoxels(src, lab, dst, l)` (§50): checked (the source and the labels); where
    /// the label value is `(float) l` the source's frame-0 value written; the count.
    pub fn mri_copy_labeled_voxels(
        &mut self,
        src: &mut Mri,
        lab: &mut Mri,
        dst: Dst,
        l: i32,
    ) -> Result<i32, Stop> {
        self.check_vol_dims(src, lab)?;
        let mut v = Vols::new(src, Some(lab), dst);
        self.clone_dst(&mut v)?;
        let lv = l as f32;
        let (w, h, d) = ext(v.a());
        let mut n = 0;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if self.mri_get_vox_val(v.b(), x, y, z, 0, 0)? == lv {
                        let s = self.mri_get_vox_val(v.a(), x, y, z, 0, 0)?;
                        self.mri_set_vox_val(v.d_mut(), x, y, z, 0, s, 0)?;
                        n += 1;
                    }
                }
            }
        }
        Ok(n)
    }

    /// `MRIsetLabelValues(src, lab, dst, l, v)` (§50): checked (the source and the labels);
    /// `dst = MRIcopy(src, dst)`; columns outermost, frame 0 where `(int)` of the label is `l` set
    /// to `val`.
    pub fn mri_set_label_values(
        &mut self,
        src: &mut Mri,
        lab: &mut Mri,
        dst: Dst,
        l: i32,
        val: f32,
    ) -> Result<VolOut, Stop> {
        self.check_vol_dims(src, lab)?;
        let mut v = Vols::new(src, Some(lab), dst);
        if !self.copy_to_dst(&mut v)? {
            return Ok(VolOut::Nothing);
        }
        let (w, h, d) = ext(v.a());
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    let lb = self.mri_get_vox_val(v.b(), x, y, z, 0, 0)?;
                    if self.int_fs12(sse::widen(lb)) == l {
                        self.mri_set_vox_val(v.d_mut(), x, y, z, 0, val, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }
}
