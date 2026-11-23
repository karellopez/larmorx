// SPDX-License-Identifier: Apache-2.0
//! Dilation and the segment helpers (spec §48): `MRIdilateUchar`, `MRIdilate`, `MRImatchSegs`
//! (FS-489), `MRIsegStats` and `HISTOseg` (FS-490); `(int)` conversions under FS-12.

use larmorx_core::defects::{FS_489, FS_490};

use super::sample::sample_type;
use super::{Dst, VolOut, Vols};
use crate::maths::Returned;
use crate::maths::histo::Histogram;
use crate::mri::{Mri, types};
use crate::process::{Process, Stop};
use crate::sse;

/// `MRIsegStats`' five values: `min max range mean std`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegStats {
    pub min: f32,
    pub max: f32,
    pub range: f32,
    pub mean: f32,
    pub std: f32,
}

/// The inclusive box `[lo, hi]` per axis grown by one voxel and clipped to `[0, e - 1]`.
fn grow(lo: i32, hi: i32, e: i32) -> (i32, i32) {
    (lo.saturating_sub(1).max(0), hi.saturating_add(1).min(e - 1))
}

impl Process {
    /// The destination rules of the dilations (§48): none, a clone of the source (voxels
    /// zero); one of another extent ends the program (`MRIcheckVolDims`); the source itself, the
    /// dilation into a clone copied back with `MRIcopy`.
    fn dilation_dst(
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

    /// `MRIdilateUchar(src, dst)` (§48): in the box of the source's nonzero bytes (frame 0,
    /// each row read as bytes) grown by one voxel, each voxel of the destination the largest
    /// byte of its 3 x 3 x 3 neighbourhood (clamped), written as a byte at its index of the row.
    pub fn mri_dilate_uchar(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        self.dilation_dst(src, dst, Process::dilate_uchar_into)
    }

    fn dilate_uchar_into(&mut self, src: &Mri, dst: &mut Mri) -> Result<(), Stop> {
        let name = "MRIdilateUchar";
        let (w, h, d) = (src.geom.width, src.geom.height, src.geom.depth);
        if w <= 0 || h <= 0 || d <= 0 {
            return Ok(());
        }
        self.direct_needs_voxels(src, name)?;
        self.direct_needs_voxels(dst, name)?;
        // the source's bytes of frame 0, row by row (each row read as bytes at its own place)
        let (wu, hu, du) = (w as usize, h as usize, d as usize);
        let mut b = vec![0u8; wu * hu * du];
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    b[(z as usize * hu + y as usize) * wu + x as usize] =
                        self.row_byte(src, (x, y, z), name);
                }
            }
        }
        let (mut x0, mut x1, mut y0, mut y1, mut z0, mut z1) = (w - 1, 0, h - 1, 0, d - 1, 0);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if b[(z as usize * hu + y as usize) * wu + x as usize] != 0 {
                        x0 = x0.min(x);
                        x1 = x1.max(x);
                        y0 = y0.min(y);
                        y1 = y1.max(y);
                        z0 = z0.min(z);
                        z1 = z1.max(z);
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
                    let mut m = 0u8;
                    for zz in (z - 1).max(0)..=(z + 1).min(d - 1) {
                        for yy in (y - 1).max(0)..=(y + 1).min(h - 1) {
                            let row = (zz as usize * hu + yy as usize) * wu;
                            for xx in (x - 1).max(0)..=(x + 1).min(w - 1) {
                                m = m.max(b[row + xx as usize]);
                            }
                        }
                    }
                    let place =
                        self.row_place(dst, i64::from(x), i64::from(y), i64::from(z), 0, 1);
                    self.direct_write(dst, place, &[m], name);
                }
            }
        }
        Ok(())
    }

    /// `MRIdilate(src, dst)` (§48): a byte source is [`Process::mri_dilate_uchar`]; else, in
    /// the box of the voxels above 0 in any frame (nearest samples) grown by one voxel, each
    /// voxel of each frame the largest of 0 and its 27 neighbourhood samples (a NaN never wins),
    /// written with `MRIsetVoxVal`.
    pub fn mri_dilate(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        if src.kind == types::UCHAR {
            return self.mri_dilate_uchar(src, dst);
        }
        self.dilation_dst(src, dst, Process::dilate_into)
    }

    /// The nearest sample of frame `f` at whole coordinates (`MRIsampleVolumeFrameType`; a
    /// type the sampler refuses samples 0 with its error).
    fn whole_sample(&mut self, m: &Mri, x: i32, y: i32, z: i32, f: i32) -> Result<f64, Stop> {
        let mut v = 0.0;
        self.mri_sample_volume_frame_type(
            m,
            f64::from(x),
            f64::from(y),
            f64::from(z),
            f,
            sample_type::NEAREST,
            &mut v,
        )?;
        Ok(v)
    }

    fn dilate_into(&mut self, src: &Mri, dst: &mut Mri) -> Result<(), Stop> {
        let (w, h, d) = (src.geom.width, src.geom.height, src.geom.depth);
        if w <= 0 || h <= 0 || d <= 0 {
            return Ok(());
        }
        let nf = src.nframes;
        let (mut x0, mut x1, mut y0, mut y1, mut z0, mut z1) = (w - 1, 0, h - 1, 0, d - 1, 0);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    for f in 0..nf {
                        if self.whole_sample(src, x, y, z, f)? > 0.0 {
                            x0 = x0.min(x);
                            x1 = x1.max(x);
                            y0 = y0.min(y);
                            y1 = y1.max(y);
                            z0 = z0.min(z);
                            z1 = z1.max(z);
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
                        let mut m = 0.0f64;
                        for dz in -1..=1 {
                            for dy in -1..=1 {
                                for dx in -1..=1 {
                                    let v = self.whole_sample(src, x + dx, y + dy, z + dz, f)?;
                                    if v > m {
                                        m = v;
                                    }
                                }
                            }
                        }
                        self.mri_set_vox_val(dst, x, y, z, f, sse::narrow(m), 0)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// `MRImatchSegs(seg, list, n, MaskId, mask)` (§48): over the segmentation's extent (columns
    /// outermost) frame 0 of the mask 0, then `MaskId` where `(int)` of the label is in the
    /// list; without a mask a copy of the segmentation. FS-489: the segmentation as its own
    /// mask: each label read before its voxel is cleared (`reproduce`: after, every label 0).
    pub fn mri_match_segs(
        &mut self,
        seg: &mut Mri,
        list: &[i32],
        mask_id: i32,
        mask: Dst,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(seg, None, mask);
        if !v.has_dst() {
            let c = match self.mri_copy(v.a(), None)? {
                Returned::New(c) => c,
                _ => return Ok(VolOut::Nothing),
            };
            v.set_dst(c);
        }
        let own = v.d_is_a();
        let after = own && self.mode().reproduce();
        if own {
            if after {
                self.note(
                    &FS_489,
                    "MRImatchSegs: the segmentation as its own mask: each voxel cleared before its label is read (every label 0), as FreeSurfer's",
                );
            } else {
                self.note(
                    &FS_489,
                    "MRImatchSegs: the segmentation as its own mask: each label read before its voxel is cleared (FreeSurfer reads it after: every label 0)",
                );
            }
        }
        let id = mask_id as f32;
        let (w, h, d) = (v.a().geom.width, v.a().geom.height, v.a().geom.depth);
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let lab = if after {
                        self.mri_set_vox_val(v.d_mut(), c, r, s, 0, 0.0, 0)?;
                        self.mri_get_vox_val(v.a(), c, r, s, 0, 0)?
                    } else {
                        let l = self.mri_get_vox_val(v.a(), c, r, s, 0, 0)?;
                        self.mri_set_vox_val(v.d_mut(), c, r, s, 0, 0.0, 0)?;
                        l
                    };
                    let lab = self.int_fs12(sse::widen(lab));
                    if list.contains(&lab) {
                        self.mri_set_vox_val(v.d_mut(), c, r, s, 0, id, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIsegStats(seg, segid, mri, frame, ...)` (§48): the count of the voxels (columns
    /// outermost) whose `(int)` label is `segid`, and the statistics of `mri`'s frame there.
    pub fn mri_seg_stats(
        &mut self,
        seg: &Mri,
        segid: i32,
        mri: &Mri,
        frame: i32,
    ) -> Result<(i32, SegStats), Stop> {
        use sse::{add32, add64, mul32, mul64, sub32, sub64, widen};
        let (mut n, mut min, mut max) = (0i32, 0f32, 0f32);
        let (mut sum, mut sum2) = (0f64, 0f64);
        let (w, h, d) = (seg.geom.width, seg.geom.height, seg.geom.depth);
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let l = self.mri_get_vox_val(seg, c, r, s, 0, 0)?;
                    if self.int_fs12(widen(l)) != segid {
                        continue;
                    }
                    let v = self.mri_get_vox_val(mri, c, r, s, frame, 0)?;
                    if n == 0 {
                        min = v;
                        max = v;
                    }
                    if widen(min) > widen(v) {
                        min = v;
                    }
                    if widen(max) < widen(v) {
                        max = v;
                    }
                    let x = widen(v);
                    sum = add64(sum, x);
                    sum2 = add64(mul64(x, x), sum2);
                    n += 1;
                }
            }
        }
        let range = sub32(max, min);
        let mean = if n > 0 {
            sse::narrow(sse::div64(sum, f64::from(n)))
        } else {
            0.0
        };
        let std = if n > 1 {
            let a = widen(mul32(mul32(n as f32, mean), mean));
            let b = mul64(widen(add32(mean, mean)), sum);
            let num = add64(sub64(a, b), sum2);
            sse::narrow(sse::sqrt64(sse::div64(num, f64::from(n - 1))))
        } else {
            0.0
        };
        Ok((
            n,
            SegStats {
                min,
                max,
                range,
                mean,
                std,
            },
        ))
    }

    /// `HISTOseg(seg, segid, vol, bmin, bmax, bdelta)` (§48): a histogram of
    /// `(int)(round((bmax - bmin) / bdelta) + 1)` bins over `[bmin, bmax]` (`HISTOinit`), each
    /// voxel whose label equals `segid` counted once in bin `(int)(round(v - bmin) / bdelta)`,
    /// clamped. FS-490 (both modes): a histogram of 0 bins counts nowhere.
    pub fn histo_seg(
        &mut self,
        seg: &Mri,
        segid: i32,
        vol: &Mri,
        bmin: f64,
        bmax: f64,
        bdelta: f64,
    ) -> Result<Histogram, Stop> {
        use sse::{add64, div64, sub64, widen};
        let nb = add64(div64(sub64(bmax, bmin), bdelta).round(), 1.0);
        let nbins = self.int_fs12(nb);
        let mut hist = match self.histo_init(None, nbins, bmin, bmax)? {
            Returned::New(h) => h,
            _ => Histogram::new(nbins.max(0)),
        };
        let id = f64::from(segid);
        let (w, h, d) = (seg.geom.width, seg.geom.height, seg.geom.depth);
        let mut noted = false;
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let l = widen(self.mri_get_vox_val(seg, c, r, s, 0, 0)?);
                    if l != id {
                        continue;
                    }
                    let v = widen(self.mri_get_vox_val(vol, c, r, s, 0, 0)?);
                    let b = self.int_fs12(div64(sub64(v, bmin).round(), bdelta));
                    let b = b.max(0).min(nbins.wrapping_sub(1));
                    if !hist.inside(b) {
                        if !noted {
                            self.note(
                                &FS_490,
                                "HISTOseg: a histogram of no bins counts nowhere (FreeSurfer counts before its array)",
                            );
                            noted = true;
                        }
                        continue;
                    }
                    let k = hist.count(b);
                    hist.set_count(b, sse::add32(k, 1.0));
                }
            }
        }
        Ok(hist)
    }
}
