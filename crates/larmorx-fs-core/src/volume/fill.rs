// SPDX-License-Identifier: Apache-2.0
//! Filling a coarser grid (spec §59): `MRIvol2VolFill`.

use super::VolOut;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::mri::lta::lta_type;
use crate::mri::{Lta, Mri, types};
use crate::process::{Process, Stop};
use crate::sse;

/// `MRIvol2VolFill`'s output: none, a new volume or the caller's.
pub enum FillOut<'a> {
    New,
    Given(&'a mut Mri),
}

impl Process {
    /// `MRIvol2VolFill(src, mask, lta, UpsampleFactor, DoConserve, outfill)` (§59): each source
    /// voxel (of the upsampled source with a factor above 0) sent to its nearest voxel of the
    /// grid at the transform's other end, every frame added there; without `DoConserve` each
    /// target voxel divided by its hits. The array is not changed.
    pub fn mri_vol2vol_fill(
        &mut self,
        src: &mut Mri,
        mask: Option<&Mri>,
        lta: &Lta,
        usf: i32,
        conserve: i32,
        out: FillOut,
    ) -> Result<VolOut, Stop> {
        if lta.num_xforms() > 1 {
            self.put_out(b"ERROR: MRIvol2VolFill(): LTA can only have one xform\n");
            return Ok(VolOut::Nothing);
        }
        let is_src = self.lta_mri_is_source(lta, src)? != 0;
        if !is_src && self.lta_mri_is_target(lta, src)? == 0 {
            self.put_out(b"ERROR: MRIvol2VolFill(): src MRI is neither source nor target in LTA\n");
            return Ok(VolOut::Nothing);
        }
        if self.gdiag_no > 0 {
            self.put_out(
                format!("MRIvol2VolFill(): USF={usf}, DoConserve={conserve}\n").as_bytes(),
            );
        }
        let mut l = match self.lta_copy(lta, None)? {
            Returned::New(c) => c,
            _ => return Ok(VolOut::Nothing),
        };
        self.lta_change_type(&mut l, lta_type::LINEAR_VOX_TO_VOX)?;
        let (mut m, vg) = if is_src {
            if self.gdiag_no > 0 {
                self.put_out(b"MRIvol2VolFill(): not using inverse\n");
            }
            let x = &l.xforms[0];
            (x.m_l.clone(), x.dst.clone())
        } else {
            if self.gdiag_no > 0 {
                self.put_out(b"MRIvol2VolFill(): using inverse\n");
            }
            self.lta_fill_inverse(&mut l)?;
            (l.inv_xforms[0].m_l.clone(), l.xforms[0].src.clone())
        };
        let mut up = None;
        if usf > 0 {
            let Some((u, step)) = self.mri_mask_and_upsample(src, mask, usf, 2, conserve != 0)?
            else {
                self.lta_fs504("MRIvol2VolFill", "no transform arrays")?;
                return Ok(VolOut::Nothing);
            };
            let v = step.xforms.first().and_then(|x| x.m_l.clone());
            let inv = match self.matrix_inverse(v.as_ref(), None)? {
                Returned::New(i) => Some(i),
                _ => None,
            };
            m = match self.matrix_multiply(m.as_ref(), inv.as_ref(), None)? {
                Returned::New(p) => Some(p),
                _ => None,
            };
            up = Some(u);
        }
        let s: &Mri = up.as_ref().unwrap_or(&*src);
        let mut own = None;
        let o: &mut Mri = match out {
            FillOut::Given(o) => o,
            FillOut::New => {
                let mut n = self.mri_from_vol_geom(&vg, types::FLOAT, s.nframes, false)?;
                n.tr = src.tr;
                n.te = src.te;
                n.ti = src.ti;
                n.flip_angle = src.flip_angle;
                own.insert(n)
            }
        };
        self.mri_clear(o)?;
        let (tw, th, td) = (vg.width, vg.height, vg.depth);
        let nt = (tw.max(0) as usize) * (th.max(0) as usize) * (td.max(0) as usize);
        let mut hits = if conserve == 0 {
            vec![0.0f32; nt]
        } else {
            Vec::new()
        };
        let (sw, sh, sd) = (s.geom.width, s.geom.height, s.geom.depth);
        let mut p = Matrix::zeros(4, 1, MATRIX_REAL);
        p.set(4, 1, 1.0);
        let mut q = Matrix::zeros(4, 1, MATRIX_REAL);
        for c in 0..sw {
            for r in 0..sh {
                for sl in 0..sd {
                    p.set(1, 1, c as f32);
                    p.set(2, 1, r as f32);
                    p.set(3, 1, sl as f32);
                    self.matrix_multiply(m.as_ref(), Some(&p), Some(&mut q))?;
                    let tc = self.nint(sse::widen(q.at(1, 1)));
                    let tr = self.nint(sse::widen(q.at(2, 1)));
                    let ts = self.nint(sse::widen(q.at(3, 1)));
                    if tc < 0 || tc >= tw || tr < 0 || tr >= th || ts < 0 || ts >= td {
                        continue;
                    }
                    for f in 0..s.nframes {
                        let a = self.mri_get_vox_val(o, tc, tr, ts, f, 0)?;
                        let b = self.mri_get_vox_val(s, c, r, sl, f, 0)?;
                        let sum = sse::add64(sse::widen(a), sse::widen(b));
                        self.mri_set_vox_val(o, tc, tr, ts, f, sse::narrow(sum), 0)?;
                    }
                    if conserve == 0 {
                        let k =
                            ((ts as usize * th as usize) + tr as usize) * tw as usize + tc as usize;
                        hits[k] = sse::add32(hits[k], 1.0);
                    }
                }
            }
        }
        if conserve == 0 {
            for c in 0..tw {
                for r in 0..th {
                    for sl in 0..td {
                        let k =
                            ((sl as usize * th as usize) + r as usize) * tw as usize + c as usize;
                        let n = hits[k];
                        if n == 0.0 {
                            continue;
                        }
                        for f in 0..s.nframes {
                            let a = self.mri_get_vox_val(o, c, r, sl, f, 0)?;
                            let v = sse::div64(sse::widen(a), sse::widen(n));
                            self.mri_set_vox_val(o, c, r, sl, f, sse::narrow(v), 0)?;
                        }
                    }
                }
            }
        }
        Ok(match own {
            Some(n) => VolOut::New(n),
            None => VolOut::Given,
        })
    }
}
