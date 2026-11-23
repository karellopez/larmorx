// SPDX-License-Identifier: Apache-2.0
//! Masks (spec §50): `MRImask`, `MRImaskDifferentGeometry` and `MRIthresholdMask` (FS-494).

use larmorx_core::defects::FS_494;

use super::setops::ext;
use super::{Dst, VolOut, Vols};
use crate::clib::scan_d;
use crate::mri::{Mri, VolGeom, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// The environment variable that lets `MRImask` take a mask of another geometry.
pub const FS_MRIMASK_ALLOW_DIFF_GEOM: &str = "FS_MRIMASK_ALLOW_DIFF_GEOM";

impl Process {
    /// `MRImaskDifferentGeometry(src, mask, dst, m, out)` (§50): each voxel of the destination
    /// (columns outermost) moved into the mask by the voxel-to-voxel matrix (`float`, rounded); outside the mask's
    /// extent the mask value is `m`, else `nint` of its frame-0 value; every frame of the
    /// destination `out` where the mask value is `m`, else the source's value.
    pub fn mri_mask_different_geometry(
        &mut self,
        src: &mut Mri,
        mask: &mut Mri,
        dst: Dst,
        mval: i32,
        out: f32,
    ) -> Result<VolOut, Stop> {
        let xf = self.mri_get_voxel_to_voxel_xform(src, mask)?;
        let mut v = Vols::new(src, Some(mask), dst);
        self.clone_dst(&mut v)?;
        // the destination's voxels (observed: the mask as the destination is written over its
        // whole extent, `vmask`)
        let (w, h, d) = ext(v.d());
        let (mw, mh, md) = ext(v.b());
        let nf = v.d().nframes;
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let mv = match &xf {
                        Some(m) => {
                            let p = [c as f32, r as f32, s as f32, 1.0f32];
                            let mut q = [0.0f32; 3];
                            for (k, qk) in q.iter_mut().enumerate() {
                                let row = k as i32 + 1;
                                let mut acc = 0.0f32;
                                for (j, pj) in p.iter().enumerate() {
                                    acc = sse::add32(acc, sse::mul32(m.at(row, j as i32 + 1), *pj));
                                }
                                *qk = acc;
                            }
                            let mc = self.nint(sse::widen(q[0]));
                            let mr = self.nint(sse::widen(q[1]));
                            let ms = self.nint(sse::widen(q[2]));
                            if mc < 0 || mc >= mw || mr < 0 || mr >= mh || ms < 0 || ms >= md {
                                mval
                            } else {
                                let x = self.mri_get_vox_val(v.b(), mc, mr, ms, 0, 0)?;
                                self.nint(sse::widen(x))
                            }
                        }
                        None => mval,
                    };
                    for f in 0..nf {
                        let o = if mv == mval {
                            out
                        } else {
                            self.mri_get_vox_val(v.a(), c, r, s, f, 0)?
                        };
                        self.mri_set_vox_val(v.d_mut(), c, r, s, f, o, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRImask(src, mask, dst, m, out)` (§50): a mask of another geometry goes to
    /// [`Process::mri_mask_different_geometry`] when `FS_MRIMASK_ALLOW_DIFF_GEOM` allows it;
    /// else checked; a clone; a destination of another type refused; each voxel whose `(int)`
    /// frame-0 mask value is `m` gets `out` in every frame, the others the source's values.
    pub fn mri_mask(
        &mut self,
        src: &mut Mri,
        mask: &mut Mri,
        dst: Dst,
        mval: i32,
        out: f32,
    ) -> Result<VolOut, Stop> {
        let allow = match self.env.get(FS_MRIMASK_ALLOW_DIFF_GEOM) {
            Some(s) => match scan_d(s) {
                (1, Some(n)) => n,
                _ => 1,
            },
            None => 1,
        };
        if self.gdiag != 0 {
            self.put_out(format!("MRImask(): AllowDiffGeom = {allow}\n").as_bytes());
        }
        let (mut va, mut vb) = (VolGeom::default(), VolGeom::default());
        self.get_vol_geom(&src.geom, &mut va);
        self.get_vol_geom(&mask.geom, &mut vb);
        if self.vg_is_not_equal_thresh(&va, &vb, 1e-4) != 0 {
            if allow != 0 {
                self.put_out(b"INFO: MRImask() using MRImaskDifferentGeometry()\n");
                return self.mri_mask_different_geometry(src, mask, dst, mval, out);
            }
            self.put_out(
                b"INFO: MRImask(): geometries differ but not using MRImaskDifferentGeometry() \n",
            );
        }
        self.check_vol_dims(src, mask)?;
        let mut v = Vols::new(src, Some(mask), dst);
        self.clone_dst(&mut v)?;
        if v.d().kind != v.a().kind {
            self.error_printf(codes::UNSUPPORTED, b"MRImask: src and dst must be same type");
            return Ok(VolOut::Nothing);
        }
        let (w, h, d) = ext(v.a());
        let nf = v.a().nframes;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let mv = self.mri_get_vox_val(v.b(), x, y, z, 0, 0)?;
                    let on = self.int_fs12(sse::widen(mv)) == mval;
                    for f in 0..nf {
                        let o = if on {
                            out
                        } else {
                            self.mri_get_vox_val(v.a(), x, y, z, f, 0)?
                        };
                        self.mri_set_vox_val(v.d_mut(), x, y, z, f, o, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIthresholdMask(src, mask, dst, thresh, out)` (§50): checked; a clone; a byte source
    /// with a byte mask: bytes, the source's where the mask byte is at or above `thresh`, else
    /// `out` as a byte (FS-494 beyond 0 to 255 or NaN); else frame 0, `out` where the mask is
    /// below `thresh`, the source's value elsewhere.
    pub fn mri_threshold_mask(
        &mut self,
        src: &mut Mri,
        mask: &mut Mri,
        dst: Dst,
        thresh: f32,
        out: f32,
    ) -> Result<VolOut, Stop> {
        let name = "MRIthresholdMask";
        self.check_vol_dims(src, mask)?;
        let mut v = Vols::new(src, Some(mask), dst);
        self.clone_dst(&mut v)?;
        let e = ext(v.a());
        let (w, h, d) = e;
        if v.a().kind == types::UCHAR && v.b().kind == types::UCHAR {
            let ob = self.out_byte(out);
            self.walk_needs(e, &[v.a(), v.b(), v.d()], name)?;
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let mb = self.byte_at(v.b(), x, y, z, name);
                        let o = if f32::from(mb) >= thresh {
                            self.byte_at(v.a(), x, y, z, name)
                        } else {
                            ob
                        };
                        self.byte_put(v.d_mut(), x, y, z, o, name);
                    }
                }
            }
            return Ok(v.out());
        }
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let mv = self.mri_get_vox_val(v.b(), x, y, z, 0, 0)?;
                    let o = if mv < thresh {
                        out
                    } else {
                        self.mri_get_vox_val(v.a(), x, y, z, 0, 0)?
                    };
                    self.mri_set_vox_val(v.d_mut(), x, y, z, 0, o, 0)?;
                }
            }
        }
        Ok(v.out())
    }

    /// The replacement byte of `MRIthresholdMask` (FS-494): within 0 to 255 its truncation;
    /// beyond or NaN, clamped (NaN 0) by default, the low byte of its truncation to a 32-bit
    /// integer (-2147483648 for NaN and beyond) with `reproduce`.
    fn out_byte(&mut self, out: f32) -> u8 {
        if (0.0..=255.0).contains(&out) {
            return out as u8;
        }
        let x = sse::widen(out);
        if self.mode().reproduce() {
            self.note(
                &FS_494,
                "MRIthresholdMask: a replacement beyond 0 to 255 (or NaN) stored as the low byte of its 32-bit truncation, as FreeSurfer's",
            );
            let i = if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
                i32::MIN
            } else {
                x as i32
            };
            i as u8
        } else {
            self.note(
                &FS_494,
                "MRIthresholdMask: a replacement beyond 0 to 255 clamped (NaN 0)",
            );
            if x.is_nan() {
                0
            } else {
                x.clamp(0.0, 255.0) as u8
            }
        }
    }
}
