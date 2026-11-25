// SPDX-License-Identifier: Apache-2.0
//! A volume moved by a transform array (spec §63): `LTAtransformInterp` and `LTAtransform`: the
//! REGISTER_DAT change, one transform's destination (the transform's geometry, `USE_AVERAGE305`,
//! the source's clone) and its RAS resampling by type, the octree path of no or several
//! transforms (the heartbeat, the weighted matrices' inverses, the destination's index tables:
//! FS-323).

use super::{Dst, VolOut};
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::mri::lta::lta_type;
use crate::mri::{Lta, Mri, VolGeom};
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};
use crate::sse;

impl Process {
    /// `LTAtransform(src, dst, lta)` (§63): trilinear.
    pub fn lta_transform(
        &mut self,
        src: &mut Mri,
        dst: Option<&mut Mri>,
        lta: &mut Lta,
    ) -> Result<VolOut, Stop> {
        self.lta_transform_interp(src, dst, lta, 1)
    }

    /// `LTAtransformInterp(src, dst, lta, interp)` (§63). Returns the destination (the given
    /// one, or a new volume) or none where the resampling gives none; `lta` changed where its
    /// type is changed.
    pub fn lta_transform_interp(
        &mut self,
        src: &mut Mri,
        dst: Option<&mut Mri>,
        lta: &mut Lta,
        interp: i32,
    ) -> Result<VolOut, Stop> {
        if lta.kind == lta_type::REGISTER_DAT {
            self.put_out(
                b"warning: changing input transform type from REGISTER_DAT to VOX_TO_VOX\n",
            );
            self.lta_change_type(lta, lta_type::LINEAR_VOX_TO_VOX)?;
        }
        let first =
            (lta.xforms.len() == 1).then(|| (lta.xforms[0].src.clone(), lta.xforms[0].dst.clone()));
        if let Some((tsrc, tdst)) = first {
            return self.lta_move_one(src, dst, lta, interp, &tsrc, &tdst, true);
        }
        self.lta_move_octree(src, dst, lta, false)
    }

    /// `LTAinverseTransformInterp(src, dst, lta, interp)` (§67): [`Process::lta_transform_interp`]
    /// with the array's inverse transforms as it holds them (no REGISTER_DAT change; one
    /// transform's destination takes the inverse's destination centre only, its Talairach name
    /// kept); no transform or several: the octree path of the forward transforms, unchanged.
    pub fn lta_inverse_transform_interp(
        &mut self,
        src: &mut Mri,
        dst: Option<&mut Mri>,
        lta: &mut Lta,
        interp: i32,
    ) -> Result<VolOut, Stop> {
        if lta.xforms.len() == 1 {
            let (tsrc, tdst) = (lta.inv_xforms[0].src.clone(), lta.inv_xforms[0].dst.clone());
            return self.lta_move_one(src, dst, lta, interp, &tsrc, &tdst, false);
        }
        self.lta_move_octree(src, dst, lta, true)
    }

    /// One transform (§63 step 2; §67 step 2 with `forward` false): the destination when none is
    /// given, then the RAS resampling by the array's type.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lta_move_one(
        &mut self,
        src: &mut Mri,
        dst: Option<&mut Mri>,
        lta: &mut Lta,
        interp: i32,
        tsrc: &VolGeom,
        tdst: &VolGeom,
        forward: bool,
    ) -> Result<VolOut, Stop> {
        let mut made: Option<Mri> = None;
        if dst.is_none() {
            made = Some(self.lta_new_destination(src, tdst, forward)?);
        }
        let ras = match lta.kind {
            lta_type::LINEAR_RAS_TO_RAS => {
                let m = if forward {
                    &lta.xforms[0]
                } else {
                    &lta.inv_xforms[0]
                };
                m.m_l.clone()
            }
            lta_type::LINEAR_VOX_TO_VOX => {
                let d: &Mri = match (&made, &dst) {
                    (Some(m), _) => m,
                    (None, Some(d)) => d,
                    (None, None) => unreachable!("a destination"),
                };
                let dgeom = d.geom.clone();
                let m = if forward {
                    &lta.xforms[0]
                } else {
                    &lta.inv_xforms[0]
                };
                let m = m.m_l.clone();
                self.lta_vox_to_ras(m.as_ref(), tsrc, tdst, &src.geom, &dgeom, !forward)?
            }
            lta_type::LINEAR_PHYSVOX_TO_PHYSVOX => {
                self.lta_change_type(lta, lta_type::LINEAR_RAS_TO_RAS)?;
                let m = if forward {
                    &lta.xforms[0]
                } else {
                    &lta.inv_xforms[0]
                };
                m.m_l.clone()
            }
            _ => {
                self.error_exit(codes::BAD_PARM, b"LTAtransform: unknown linear transform\n")?;
                return Ok(VolOut::Nothing);
            }
        };
        match (made, dst) {
            (Some(mut m), _) => {
                match self.mri_apply_ras_linear_transform_interp(
                    src,
                    Dst::Given(&mut m),
                    ras.as_ref(),
                    interp,
                )? {
                    VolOut::Nothing => Ok(VolOut::Nothing),
                    VolOut::Given => Ok(VolOut::New(m)),
                    other => Ok(other),
                }
            }
            (None, Some(d)) => {
                self.mri_apply_ras_linear_transform_interp(src, Dst::Given(d), ras.as_ref(), interp)
            }
            (None, None) => unreachable!("a destination"),
        }
    }

    /// The RAS matrix of a voxel-to-voxel transform (§63; §67's inverse with `inv`): `(i2r M)
    /// r2i` with the transform's destination geometry's voxel-to-RAS (else the destination
    /// volume's, after its three lines) and its source geometry's RAS-to-voxel (else the source
    /// volume's).
    pub(crate) fn lta_vox_to_ras(
        &mut self,
        m: Option<&Matrix>,
        tsrc: &VolGeom,
        tdst: &VolGeom,
        src: &VolGeom,
        dst: &VolGeom,
        inv: bool,
    ) -> Result<Option<Matrix>, Stop> {
        let lta = if inv { "LTA inv" } else { "LTA" };
        let i2r = if tdst.valid != 0 {
            self.vg_i_to_r(tdst)?
        } else {
            self.put_err(format!("INFO: {lta} dst geometry information missing!\n      We assume that the dst volume passed is the\n      same as the dst for the transform.\n").as_bytes());
            self.extract_i_to_r(dst)?
        };
        let r2i = if tsrc.valid != 0 {
            self.vg_r_to_i(tsrc)?
        } else {
            self.put_err(format!("INFO: {lta} src geometry information missing!\n      We assume that the src volume passed is the\n      same as the src for the transform.\n").as_bytes());
            self.extract_r_to_i(src)?
        };
        let a = match self.matrix_multiply(Some(&i2r), m, None)? {
            Returned::New(x) => Some(x),
            _ => None,
        };
        Ok(
            match self.matrix_multiply(a.as_ref(), r2i.as_ref(), None)? {
                Returned::New(x) => Some(x),
                _ => None,
            },
        )
    }

    /// The destination of one transform when none is given (§63 step 2; §67's differences with
    /// `forward` false: the clone first, its Talairach name kept).
    fn lta_new_destination(
        &mut self,
        src: &Mri,
        tdst: &VolGeom,
        forward: bool,
    ) -> Result<Mri, Stop> {
        if forward {
            if tdst.valid == 1 {
                let g = &src.geom;
                let _ = g;
                let mut d = self.mri_alloc_sequence(
                    tdst.width,
                    tdst.height,
                    tdst.depth,
                    src.kind,
                    src.nframes,
                )?;
                self.mri_copy_header(src, Some(&mut d))?;
                self.use_vol_geom(tdst, &mut d)?;
                if self.gdiag & DIAG_VERBOSE != 0 {
                    self.put_err(b"INFO: Modifying dst geometry, using the transform dst\n");
                }
                return Ok(d);
            }
            if self.env.is_set("USE_AVERAGE305") {
                self.put_err(b"INFO: Environmental variable USE_AVERAGE305 set\nINFO: Modifying dst c_(r,a,s), using average_305 values\n");
                let mut d = self.mri_clone_new(src)?;
                d.transform_fname.clear();
                d.geom.c = [-0.0950, -16.5100, 9.7500];
                d.geom.ras_good_flag = 1;
                self.reinit_cache(&mut d.geom)?;
                return Ok(d);
            }
            self.put_err(b"INFO: Transform dst volume info is not used (valid flag = 0).\n");
            let mut d = self.mri_clone_new(src)?;
            d.transform_fname.clear();
            return Ok(d);
        }
        let mut d = self.mri_clone_new(src)?;
        if tdst.valid == 1 {
            // the centre only: the extent, sizes and cosines stay the source's (§67)
            d.geom.c = tdst.c;
            d.geom.ras_good_flag = 1;
            self.reinit_cache(&mut d.geom)?;
            if self.gdiag & DIAG_VERBOSE != 0 {
                self.put_err(b"INFO: Modifying dst c_(r,a,s), using the transform dst\n");
            }
        } else if self.env.is_set("USE_AVERAGE305") {
            self.put_err(b"INFO: Environmental variable USE_AVERAGE305 set\nINFO: Modifying dst c_(r,a,s), using average_305 values\n");
            d.geom.c = [-0.0950, -16.5100, 9.7500];
            d.geom.ras_good_flag = 1;
            self.reinit_cache(&mut d.geom)?;
        } else {
            self.put_err(b"INFO: Transform dst volume info is not used (valid flag = 0).\n");
        }
        Ok(d)
    }

    /// The octree path (§63 step 3; §67's with `inverse` true): no transform or several; each
    /// voxel of the source's extent takes, in every frame of the destination, the source's value
    /// at the voxel the weighted matrix's inverse maps it to (rounded, through the destination's
    /// index tables).
    pub(crate) fn lta_move_octree(
        &mut self,
        src: &mut Mri,
        dst: Option<&mut Mri>,
        lta: &Lta,
        inverse: bool,
    ) -> Result<VolOut, Stop> {
        let _ = inverse;
        self.put_err(b"applying octree transform to image...\n");
        let mut made: Option<Mri> = None;
        if dst.is_none() {
            made = Some(self.mri_clone_new(src)?);
        }
        let d: &mut Mri = match (made.as_mut(), dst) {
            (Some(m), _) => m,
            (None, Some(d)) => d,
            (None, None) => unreachable!("a destination"),
        };
        let (w, h, depth) = (src.geom.width, src.geom.height, src.geom.depth);
        let n = lta.xforms.len();
        let zero = Matrix::zeros(4, 4, MATRIX_REAL);
        let mut point = Matrix::zeros(4, 4, MATRIX_REAL);
        let name = "LTAtransformInterp";
        for z in 0..depth {
            self.diag_heartbeat(sse::div32(z as f32, (depth - 1) as f32));
            for y in 0..h {
                for x in 0..w {
                    let inv = if n == 0 {
                        Some(zero.clone())
                    } else {
                        self.lta_transform_at_point(
                            lta,
                            x as f32,
                            y as f32,
                            z as f32,
                            Some(&mut point),
                        )?;
                        match self.matrix_inverse(Some(&point), None)? {
                            Returned::New(m) => Some(m),
                            _ => None,
                        }
                    };
                    let Some(inv) = inv else {
                        continue;
                    };
                    let mut v = Matrix::zeros(4, 1, MATRIX_REAL);
                    for (k, c) in [x as f32, y as f32, z as f32, 1.0].into_iter().enumerate() {
                        v.set(k as i32 + 1, 1, c);
                    }
                    let q = match self.matrix_multiply(Some(&inv), Some(&v), None)? {
                        Returned::New(m) => m,
                        _ => continue,
                    };
                    let e = [d.geom.width, d.geom.height, d.geom.depth];
                    let mut idx = [0i32; 3];
                    for k in 0..3 {
                        let r = self.nint(sse::widen(q.at(k as i32 + 1, 1)));
                        idx[k] = self.table_index(e[k], r, name)?;
                    }
                    for f in 0..d.nframes {
                        let val = self.mri_get_vox_val(src, idx[0], idx[1], idx[2], f, 0)?;
                        self.mri_set_vox_val(d, x, y, z, f, val, 0)?;
                    }
                }
            }
        }
        Ok(match made {
            Some(m) => VolOut::New(m),
            None => VolOut::Given,
        })
    }
}
