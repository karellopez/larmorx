// SPDX-License-Identifier: Apache-2.0
//! Voxelwise operations (spec §39, part 8): frame powers, binarising, reshaping, squares and
//! roots, maxima, constants, frame binarising, votes, crops, slice reversals, end cuts, label
//! lists and the half-cosine weighting; FS-480 and FS-481 (GLIBC-07 for `powf` and `cos`).

use larmorx_core::defects::{FS_480, FS_481};
use larmorx_core::math;

use super::sample::cvtt;
use super::{Dst, VolOut, Vols};
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix, real_point_f};
use crate::mri::{Mri, types};
use crate::process::{Process, Stop};
use crate::sse;

/// A volume's extent.
fn extent(m: &Mri) -> (i32, i32, i32) {
    (m.geom.width, m.geom.height, m.geom.depth)
}

/// `!(x > t)`: not above (a NaN is not above anything).
fn not_above<T: PartialOrd>(x: T, t: T) -> bool {
    x.partial_cmp(&t) != Some(std::cmp::Ordering::Greater)
}

/// Whether `(c, r, s)` lies in the volume.
fn inside(m: &Mri, c: i32, r: i32, s: i32) -> bool {
    let (w, h, d) = extent(m);
    c >= 0 && c < w && r >= 0 && r < h && s >= 0 && s < d
}

impl Process {
    /// The destination of a function that makes a new `float` volume with the source's header
    /// when none is given.
    fn float_dst(&mut self, v: &mut Vols) -> Result<(), Stop> {
        if !v.has_dst() {
            let a = v.a();
            let m = self.alloc_with_header(a, extent(a), types::FLOAT, a.nframes)?;
            v.set_dst(m);
        }
        Ok(())
    }

    /// `mri_framepower(vol, powers)` (§39): frame `f`'s voxels (as `float`s directly) left for a
    /// power within 0.00001 of 1, `sqrtf` for one within 0.00001 of 0.5, else `powf` (glibc's
    /// with `reproduce`; GLIBC-07). One power per frame: frames past the array are left.
    pub fn mri_framepower(&mut self, m: &mut Mri, powers: &[f32]) -> Result<i32, Stop> {
        let (w, h, d) = extent(m);
        for f in 0..m.nframes {
            let Some(&p) = powers.get(f as usize) else {
                break;
            };
            let pd = sse::widen(p);
            if (pd - 1.0).abs() < 0.00001 {
                continue;
            }
            let root = (pd - 0.5).abs() < 0.00001;
            for r in 0..h {
                for c in 0..w {
                    for s in 0..d {
                        let (x, y, z, ff) =
                            (i64::from(c), i64::from(r), i64::from(s), i64::from(f));
                        let v = self
                            .direct_get(m, types::FLOAT, x, y, z, ff, "mri_framepower")?
                            .to_f32();
                        let o = if root {
                            sse::sqrt32(v)
                        } else {
                            self.libm_powf(v, p)
                        };
                        self.direct_put(m, x, y, z, ff, &o.to_le_bytes(), "mri_framepower")?;
                    }
                }
            }
        }
        Ok(0)
    }

    /// `mri_binarize(vol, thresh, tail, invert, bin, &nover)` (§39): each voxel (negated for the
    /// tail `neg`, its magnitude for `abs`) above `thresh` 1 (0 inverted), else the other, the 1s
    /// counted; into a new `float` volume (the header and pulse parameters) or the given one.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_binarize_tail(
        &mut self,
        src: &mut Mri,
        thresh: f32,
        tail: Option<&[u8]>,
        invert: bool,
        dst: Dst,
        nover: &mut i32,
    ) -> Result<VolOut, Stop> {
        let which = match tail {
            None => 0,
            Some(t) => {
                let head = t.get(..3).map(<[u8]>::to_ascii_lowercase);
                match head.as_deref() {
                    Some(b"pos") => 0,
                    Some(b"neg") => 1,
                    Some(b"abs") => 2,
                    _ => {
                        let mut msg = b"mri_binarize: tail = ".to_vec();
                        msg.extend_from_slice(t);
                        msg.extend_from_slice(b" unrecoginzed\n");
                        self.put_err(&msg);
                        return Ok(VolOut::Nothing);
                    }
                }
            }
        };
        let mut v = Vols::new(src, None, dst);
        if !v.has_dst() {
            let a = v.a().clone();
            let mut m = self.alloc_with_header(&a, extent(&a), types::FLOAT, a.nframes)?;
            self.copy_pulse_parameters(&a, &mut m);
            v.set_dst(m);
        }
        self.put_out(if invert {
            b"INVERTING\n"
        } else {
            b"NOT INVERTING\n"
        });
        *nover = 0;
        let (w, h, d) = extent(v.a());
        let nf = v.a().nframes;
        for r in 0..h {
            for c in 0..w {
                for s in 0..d {
                    for f in 0..nf {
                        let mut x = self.vf(v.a(), (c, r, s), f)?;
                        match which {
                            1 => x = -x,
                            2 => x = x.abs(),
                            _ => {}
                        }
                        let one = (x > thresh) != invert;
                        let b = if one { 1.0 } else { 0.0 };
                        self.mri_set_vox_val(v.d_mut(), c, r, s, f, b, 0)?;
                        if one {
                            *nover += 1;
                        }
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `mri_reshape(vol, ncols, nrows, nslices, nframes)` (§39): a new volume of the new shape
    /// holding the voxels in memory order (the six plain types; FS-481: `long` voxels copied as
    /// 32-bit integers with `reproduce`); a volume without frames is given one; element counts
    /// that differ: two lines and none.
    pub fn mri_reshape(
        &mut self,
        m: &mut Mri,
        nc: i32,
        nr: i32,
        ns: i32,
        nf: i32,
    ) -> Result<Option<Mri>, Stop> {
        if m.nframes == 0 {
            m.nframes = 1;
        }
        let (w, h, d) = extent(m);
        let nv1 = w.wrapping_mul(h).wrapping_mul(d).wrapping_mul(m.nframes);
        let nv2 = nc.wrapping_mul(nr).wrapping_mul(ns).wrapping_mul(nf);
        if nv1 != nv2 {
            self.put_out(
                format!(
                    "ERROR: mri_reshape: number of elements cannot change\n  nv1 = {nv1}, nv1 = {nv2}\n"
                )
                .as_bytes(),
            );
            return Ok(None);
        }
        let mut out = self.alloc_with_header(m, (nc, nr, ns), m.kind, nf)?;
        self.copy_pulse_parameters(m, &mut out);
        let plain = matches!(
            m.kind,
            types::UCHAR | types::SHORT | types::USHRT | types::FLOAT | types::INT | types::LONG
        );
        let n = nv1.max(0) as usize;
        if plain && n > 0 {
            if m.voxels.is_none() {
                self.no_voxels("mri_reshape")?;
            }
            let size = m.bytes_per_vox as usize;
            let bytes = if m.kind == types::LONG {
                if self.mode().reproduce() {
                    self.note(
                        &FS_481,
                        "mri_reshape: 64-bit voxels copied as 32-bit integers at their indices (the rest 0)",
                    );
                    4 * n
                } else {
                    self.note(
                        &FS_481,
                        "mri_reshape: 64-bit voxels copied whole (FreeSurfer copies 32-bit halves)",
                    );
                    size * n
                }
            } else {
                size * n
            };
            if let (Some(a), Some(b)) = (m.voxels.as_ref(), out.voxels.as_mut()) {
                let k = bytes.min(a.len()).min(b.len());
                b[..k].copy_from_slice(&a[..k]);
            }
        }
        Ok(Some(out))
    }

    /// `MRIsquareRoot(in, mask, out)` (§39): `sqrt(fabs(v))` (`double`), `v` taken as 0 where the
    /// mask voxel is not above 0.5; into a new `float` volume with `in`'s header or the given one.
    pub fn mri_square_root(
        &mut self,
        src: &mut Mri,
        mask: Option<&mut Mri>,
        dst: Dst,
    ) -> Result<VolOut, Stop> {
        let has_mask = mask.is_some();
        let mut v = Vols::new(src, mask, dst);
        self.float_dst(&mut v)?;
        let (w, h, d) = extent(v.a());
        let nf = v.a().nframes;
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    for f in 0..nf {
                        let zero = has_mask && not_above(self.v0(v.b(), (c, r, s))?, 0.5);
                        let x = if zero {
                            0.0
                        } else {
                            sse::widen(self.vf(v.a(), (c, r, s), f)?)
                        };
                        let o = sse::narrow(sse::sqrt64(x.abs()));
                        self.mri_set_vox_val(v.d_mut(), c, r, s, f, o, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIsqrt(in, out)` (§39) = `MRIsquareRoot(in, NULL, out)`.
    pub fn mri_sqrt(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        self.mri_square_root(src, None, dst)
    }

    /// `MRIsquare(in, mask, out)` (§39): a mask voxel below 0.5 skips the voxel; else each frame
    /// `v v` (`double`), `v` taken as 0 where the mask voxel is not above 0.5; into a clone or the
    /// given volume.
    pub fn mri_square(
        &mut self,
        src: &mut Mri,
        mask: Option<&mut Mri>,
        dst: Dst,
    ) -> Result<VolOut, Stop> {
        let has_mask = mask.is_some();
        let mut v = Vols::new(src, mask, dst);
        self.clone_dst(&mut v)?;
        if !v.has_dst() {
            return Ok(VolOut::Nothing);
        }
        let (w, h, d) = extent(v.a());
        let nf = v.a().nframes;
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let mv = if has_mask {
                        Some(self.v0(v.b(), (c, r, s))?)
                    } else {
                        None
                    };
                    if mv.is_some_and(|m| m < 0.5) {
                        continue;
                    }
                    let zero = mv.is_some_and(|m| not_above(m, 0.5));
                    for f in 0..nf {
                        let x = if zero {
                            0.0
                        } else {
                            sse::widen(self.vf(v.a(), (c, r, s), f)?)
                        };
                        let o = sse::narrow(sse::mul64(x, x));
                        self.mri_set_vox_val(v.d_mut(), c, r, s, f, o, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRImax(a, b, out)` (§39): `a` unless `a < b` (`double`s), into a new volume of `a`'s type,
    /// extent, frames and header or the given one (its four dimensions checked; `b` is not).
    pub fn mri_max(&mut self, a: &mut Mri, b: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let mut v = Vols::new(a, Some(b), dst);
        if !v.has_dst() {
            let a = v.a();
            let m = self.alloc_with_header(a, extent(a), a.kind, a.nframes)?;
            v.set_dst(m);
        }
        let (a, d) = (v.a(), v.d());
        if extent(a) != extent(d) || a.nframes != d.nframes {
            self.put_out(b"ERROR: MRImax: dimension mismatch\n");
            return Ok(VolOut::Nothing);
        }
        let (w, h, dd) = extent(v.a());
        for f in 0..v.a().nframes {
            for c in 0..w {
                for r in 0..h {
                    for s in 0..dd {
                        let x = sse::widen(self.vf(v.a(), (c, r, s), f)?);
                        let y = sse::widen(self.vf(v.b(), (c, r, s), f)?);
                        let o = if x < y { y } else { x };
                        self.mri_set_vox_val(v.d_mut(), c, r, s, f, sse::narrow(o), 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRImultiplyConst(src, k, out)` (`multiply`) and `MRIaddConst(src, k, out)` (§39): `v k` or
    /// `v + k` (`double`), into a new `float` volume with the source's header or the given one
    /// (any type and extent, not checked).
    pub fn mri_const_op(
        &mut self,
        src: &mut Mri,
        k: f64,
        multiply: bool,
        dst: Dst,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        self.float_dst(&mut v)?;
        let (w, h, d) = extent(v.a());
        let nf = v.a().nframes;
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    for f in 0..nf {
                        let x = sse::widen(self.vf(v.a(), (c, r, s), f)?);
                        let o = if multiply {
                            sse::mul64(x, k)
                        } else {
                            sse::add64(x, k)
                        };
                        self.mri_set_vox_val(v.d_mut(), c, r, s, f, sse::narrow(o), 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIframeBinarize(mri, thresh, mask)` (§39): 1 where every frame's magnitude is above
    /// `thresh`, else 0, into frame 0 of the given mask (its voxels below 0.5 kept) or of a new
    /// cleared `float` volume of one frame.
    pub fn mri_frame_binarize(
        &mut self,
        m: &Mri,
        thresh: f64,
        mask: Option<&mut Mri>,
    ) -> Result<VolOut, Stop> {
        let given = mask.is_some();
        let mut own = None;
        let out: &mut Mri = match mask {
            Some(k) => k,
            None => {
                let mut n = self.mri_clone_by_space(m, types::FLOAT, 1)?;
                self.mri_clear(&mut n)?;
                own.insert(n)
            }
        };
        let (w, h, d) = extent(m);
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    if given && self.v0(out, (c, r, s))? < 0.5 {
                        continue;
                    }
                    let mut all = true;
                    for f in 0..m.nframes {
                        if not_above(sse::widen(self.vf(m, (c, r, s), f)?).abs(), thresh) {
                            all = false;
                            break;
                        }
                    }
                    let b = if all { 1.0 } else { 0.0 };
                    self.mri_set_vox_val(out, c, r, s, 0, b, 0)?;
                }
            }
        }
        Ok(match own {
            Some(n) => VolOut::New(n),
            None => VolOut::Given,
        })
    }

    /// `MRIvote(in, mask, vote, exclude0)` (§39): `in` sorted in place within the mask, then per
    /// voxel the first longest run of equal sorted frames (zeros left out with `exclude0`; FS-480:
    /// the last run taken even of zeros with `reproduce`): its value into frame 0 and its length
    /// over the frame count into frame 1 of a new two-frame volume of `in`'s type or the given one.
    pub fn mri_vote(
        &mut self,
        src: &mut Mri,
        mut mask: Option<&mut Mri>,
        dst: Dst,
        exclude0: bool,
    ) -> Result<VolOut, Stop> {
        self.put_out(b"MRIvote: sorting\n");
        if matches!(
            self.mri_sort(&mut *src, mask.as_deref_mut(), Dst::Src)?,
            VolOut::Nothing
        ) {
            return Ok(VolOut::Nothing);
        }
        self.put_out(b"MRIvote: done sorting\n");
        let has_mask = mask.is_some();
        let mut v = Vols::new(src, mask, dst);
        if !v.has_dst() {
            let a = v.a();
            let m = self.alloc_with_header(a, extent(a), a.kind, 2)?;
            v.set_dst(m);
        }
        if v.d().kind != v.a().kind {
            self.put_out(b"ERROR: MRIvote: type mismatch\n");
            return Ok(VolOut::Nothing);
        }
        if extent(v.d()) != extent(v.a()) || v.d().nframes != 2 {
            self.put_out(b"ERROR: MRIvote: dimension mismatch\n");
            return Ok(VolOut::Nothing);
        }
        let reproduce = self.mode().reproduce();
        let (w, h, d) = extent(v.a());
        let nf = v.a().nframes;
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    if has_mask && self.v0(v.b(), (c, r, s))? < 0.5 {
                        continue;
                    }
                    let mut x = Vec::with_capacity(nf.max(0) as usize);
                    for f in 0..nf {
                        x.push(sse::widen(self.vf(v.a(), (c, r, s), f)?));
                    }
                    let (mut best, mut most) = (0.0f64, 0usize);
                    let mut i = 0;
                    while i < x.len() {
                        let mut j = i + 1;
                        while j < x.len() && x[j] == x[j - 1] {
                            j += 1;
                        }
                        let (len, val, last) = (j - i, x[i], j == x.len());
                        if len > most {
                            let zero = exclude0 && val == 0.0;
                            if zero && last {
                                if reproduce {
                                    self.note(
                                        &FS_480,
                                        "MRIvote: the last run taken although it is of zeros",
                                    );
                                    best = val;
                                    most = len;
                                } else {
                                    self.note(
                                        &FS_480,
                                        "MRIvote: the last run of zeros left out (FreeSurfer takes it)",
                                    );
                                }
                            } else if !zero {
                                best = val;
                                most = len;
                            }
                        }
                        i = j;
                    }
                    let frac = sse::div64(most as f64, f64::from(nf));
                    self.mri_set_vox_val(v.d_mut(), c, r, s, 0, sse::narrow(best), 0)?;
                    self.mri_set_vox_val(v.d_mut(), c, r, s, 1, sse::narrow(frac), 0)?;
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIcrop(mri, c1, r1, s1, c2, r2, s2)` (§39): a new volume of the inclusive box, its first
    /// voxel's RAS as its `P0`; a start or end outside the volume: its line and none.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_crop(
        &mut self,
        m: &Mri,
        c1: i32,
        r1: i32,
        s1: i32,
        c2: i32,
        r2: i32,
        s2: i32,
    ) -> Result<Option<Mri>, Stop> {
        for (what, (c, r, s)) in [("start", (c1, r1, s1)), ("end", (c2, r2, s2))] {
            if !inside(m, c, r, s) {
                self.put_out(
                    format!("MRIcrop(): {what} point {c} {r} {s} out of range\n").as_bytes(),
                );
                return Ok(None);
            }
        }
        let (nw, nh, nd) = (
            c2.wrapping_sub(c1).wrapping_add(1),
            r2.wrapping_sub(r1).wrapping_add(1),
            s2.wrapping_sub(s1).wrapping_add(1),
        );
        let mut out = self.alloc_with_header(m, (nw, nh, nd), m.kind, m.nframes)?;
        let v = self.vox2ras_scanner(&m.geom, 0, false)?;
        let p = real_point_f(&v, [c1 as f32, r1 as f32, s1 as f32, 1.0]);
        self.p0_to_cras(
            &mut out.geom,
            sse::widen(p[0]),
            sse::widen(p[1]),
            sse::widen(p[2]),
        )?;
        for c in 0..nw {
            for r in 0..nh {
                for s in 0..nd {
                    for f in 0..m.nframes {
                        let x = self.vf(m, (c1 + c, r1 + r, s1 + s), f)?;
                        self.mri_set_vox_val(&mut out, c, r, s, f, x, 0)?;
                    }
                }
            }
        }
        Ok(Some(out))
    }

    /// `MRIreverseSlices(in, out)` (§39): a copy with its slices reversed, its vox2ras the
    /// input's times the inverse of the slice flip; in place refused.
    pub fn mri_reverse_slices(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        if v.d_is_a() {
            self.put_out(b"ERROR: MRIreverseSlices(): cannot be done in-place\n");
            return Ok(VolOut::Nothing);
        }
        if v.has_dst() {
            let s = v.a().clone();
            if self.mri_copy(&s, Some(v.d_mut()))?.is_nothing() {
                return Ok(VolOut::Nothing);
            }
        } else {
            match self.mri_copy(v.a(), None)? {
                Returned::New(m) => v.set_dst(m),
                _ => return Ok(VolOut::Nothing),
            }
        }
        let (w, h, d) = extent(v.a());
        let vox = self.vox2ras_scanner(&v.a().geom, 0, false)?;
        let mut flip = Matrix::zeros(4, 4, MATRIX_REAL);
        for k in 1..=4 {
            flip.set(k, k, 1.0);
        }
        flip.set(3, 3, -1.0);
        flip.set(3, 4, (d - 1) as f32);
        let inv = match self.matrix_inverse(Some(&flip), None)? {
            Returned::New(m) => Some(m),
            _ => None,
        };
        let m = match self.matrix_multiply(Some(&vox), inv.as_ref(), None)? {
            Returned::New(m) => Some(m),
            _ => None,
        };
        self.mri_set_voxel_to_ras_xform(v.d_mut(), m.as_ref())?;
        let nf = v.a().nframes;
        for s in 0..d {
            let t = cvtt(sse::sub64(sse::sub64(f64::from(d), 1.0), f64::from(s)));
            for c in 0..w {
                for r in 0..h {
                    for f in 0..nf {
                        let x = self.vf(v.a(), (c, r, s), f)?;
                        self.mri_set_vox_val(v.d_mut(), c, r, t, f, x, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIcutEndSlices(mri, n)` (§39): a new volume without `n` slices at each end (the header
    /// as it is); none left: its line and none.
    pub fn mri_cut_end_slices(&mut self, m: &Mri, n: i32) -> Result<Option<Mri>, Stop> {
        let (w, h, d) = extent(m);
        let nd = d.wrapping_sub(n.wrapping_mul(2));
        if nd <= 0 {
            self.put_out(
                format!("ERROR: MRIcutEndSlices(): ncut = {n}, input only has {d} \n").as_bytes(),
            );
            return Ok(None);
        }
        let mut out = self.alloc_with_header(m, (w, h, nd), m.kind, m.nframes)?;
        for c in 0..w {
            for r in 0..h {
                for s in 0..nd {
                    for f in 0..m.nframes {
                        let x = self.vf(m, (c, r, s.wrapping_add(n)), f)?;
                        self.mri_set_vox_val(&mut out, c, r, s, f, x, 0)?;
                    }
                }
            }
        }
        Ok(Some(out))
    }

    /// `MRIsegIdList(seg, &n, frame)` and, `not0`, `MRIsegIdListNot0` (§39): the voxels' `(int)`
    /// values of the frame (FS-12), ascending, each once (0 left out with `not0`).
    pub fn mri_seg_id_list(&mut self, seg: &Mri, frame: i32, not0: bool) -> Result<Vec<i32>, Stop> {
        let (w, h, d) = extent(seg);
        let mut ids = Vec::new();
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let x = self.vf(seg, (c, r, s), frame)?;
                    ids.push(self.int_fs12(sse::widen(x)));
                }
            }
        }
        ids.sort_unstable();
        ids.dedup();
        if not0 {
            ids.retain(|&x| x != 0);
        }
        Ok(ids)
    }

    /// `MRIhalfCosBias(in, alpha, out)` (§39): a copy whose slice `s` is weighted by `(alpha
    /// cos(pi s / (depth - 1)) + (2 - alpha)) / 2` (`double`; glibc's `cos` with `reproduce`); the
    /// product's NaN, where both are NaN, is the voxel's (observed: `vmri2`, one slice).
    pub fn mri_half_cos_bias(
        &mut self,
        src: &mut Mri,
        alpha: f64,
        dst: Dst,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        if v.has_dst() {
            if !v.d_is_a() {
                let s = v.a().clone();
                if self.mri_copy(&s, Some(v.d_mut()))?.is_nothing() {
                    return Ok(VolOut::Nothing);
                }
            }
        } else {
            match self.mri_copy(v.a(), None)? {
                Returned::New(m) => v.set_dst(m),
                _ => return Ok(VolOut::Nothing),
            }
        }
        let (w, h, d) = extent(v.d());
        let nf = v.d().nframes;
        for s in 0..d {
            let a = sse::div64(
                sse::mul64(std::f64::consts::PI, f64::from(s)),
                sse::sub64(f64::from(d), 1.0),
            );
            let cos = self.libm("cos", a, math::cos, math::cos_glibc);
            let wgt = sse::div64(
                sse::add64(sse::mul64(alpha, cos), sse::sub64(2.0, alpha)),
                2.0,
            );
            for c in 0..w {
                for r in 0..h {
                    for f in 0..nf {
                        let x = sse::widen(self.vf(v.d(), (c, r, s), f)?);
                        // observed: of two NaNs the voxel's (the product as `v w`)
                        let o = sse::narrow(sse::mul64(x, wgt));
                        self.mri_set_vox_val(v.d_mut(), c, r, s, f, o, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }
}
