// SPDX-License-Identifier: Apache-2.0
//! Linear transforms of a volume through its B-spline (spec §5): voxel to voxel, RAS to RAS and
//! by a transform array.

use super::BSpline;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix, real_point_f};
use crate::mri::{Lta, Mri};
use crate::process::{Process, Stop, codes};
use crate::sse;
use crate::volume::VolOut;

/// The transform array's types (§5.3).
pub mod lta_type {
    pub const LINEAR_VOX_TO_VOX: i32 = 0;
    pub const LINEAR_RAS_TO_RAS: i32 = 1;
    pub const LINEAR_PHYSVOX_TO_PHYSVOX: i32 = 2;
}

/// A matrix a function made, or none.
fn made(r: Returned<Matrix>) -> Option<Matrix> {
    match r {
        Returned::New(m) => Some(m),
        _ => None,
    }
}

impl Process {
    /// `MRIcloneDifferentType(src, type)` (`specs/freesurfer-volume.md`): `MRIclone` for the same
    /// type, else a new volume of `type` (the source's extent and frames) with its header.
    pub fn mri_clone_different_type(&mut self, src: &Mri, kind: i32) -> Result<Mri, Stop> {
        if kind == src.kind {
            return self.mri_clone_new(src);
        }
        let g = &src.geom;
        let mut d = self.mri_alloc_sequence(g.width, g.height, g.depth, kind, src.nframes)?;
        self.mri_copy_header(src, Some(&mut d))?;
        Ok(d)
    }

    /// `MRIlinearTransformBSpline(b, dst, A)` (§5.1): each destination voxel (slices outermost,
    /// columns innermost) sampled at `A^-1` of its indices, every frame of the coefficients; no
    /// destination: a clone of the coefficients of the source's type; a given one cleared.
    pub fn mri_linear_transform_bspline(
        &mut self,
        b: &BSpline,
        dst: Option<&mut Mri>,
        a: Option<&Matrix>,
    ) -> Result<VolOut, Stop> {
        let Some(inv) = made(self.matrix_inverse(a, None)?) else {
            self.error_printf(
                codes::BAD_PARM,
                b"MRIlinearTransformSpline: xform is singular",
            );
            return Ok(VolOut::Nothing);
        };
        match dst {
            Some(d) => {
                self.mri_clear(d)?;
                self.bspline_transform_into(b, d, &inv)?;
                Ok(VolOut::Given)
            }
            None => {
                let c = b.coeff.as_ref().expect("coefficients");
                let mut d = self.mri_clone_different_type(c, b.srctype)?;
                self.bspline_transform_into(b, &mut d, &inv)?;
                Ok(VolOut::New(d))
            }
        }
    }

    /// §5.1 steps 3 and 4 into `d` with the inverse matrix.
    fn bspline_transform_into(
        &mut self,
        b: &BSpline,
        d: &mut Mri,
        inv: &Matrix,
    ) -> Result<(), Stop> {
        let nframes = b.coeff.as_ref().expect("coefficients").nframes;
        let (w, h, dep) = (d.geom.width, d.geom.height, d.geom.depth);
        let fast = !inv.is_complex() && inv.rows == 4 && inv.cols == 4;
        let mut vx = Matrix::zeros(4, 1, MATRIX_REAL);
        let mut vy = Matrix::zeros(4, 1, MATRIX_REAL);
        vx.set(4, 1, 1.0);
        for z in 0..dep {
            for y in 0..h {
                for x in 0..w {
                    let q = if fast {
                        real_point_f(inv, [x as f32, y as f32, z as f32, 1.0])
                    } else {
                        vx.set(1, 1, x as f32);
                        vx.set(2, 1, y as f32);
                        vx.set(3, 1, z as f32);
                        self.matrix_multiply(Some(inv), Some(&vx), Some(&mut vy))?;
                        [vy.at(1, 1), vy.at(2, 1), vy.at(3, 1), vy.at(4, 1)]
                    };
                    let p = [sse::widen(q[0]), sse::widen(q[1]), sse::widen(q[2])];
                    for f in 0..nframes {
                        let (_, v) = self.mri_sample_bspline(b, p[0], p[1], p[2], f)?;
                        let v = sse::narrow(v.unwrap_or(0.0));
                        self.mri_set_vox_val(d, x, y, z, f, v, 0)?;
                    }
                }
            }
        }
        d.geom.ras_good_flag = 1;
        Ok(())
    }

    /// `MRIapplyRASlinearTransformBSpline(b, dst, M)` (§5.2): §5.1 with the voxel-to-voxel matrix
    /// of the RAS matrix (`MRIrasXformToVoxelXform` of the coefficients and the destination).
    pub fn mri_apply_ras_linear_transform_bspline(
        &mut self,
        b: &BSpline,
        dst: Option<&mut Mri>,
        m: Option<&Matrix>,
    ) -> Result<VolOut, Stop> {
        let c = b.coeff.as_ref().expect("coefficients");
        let v = self.mri_ras_xform_to_voxel_xform(c, dst.as_deref(), m)?;
        self.mri_linear_transform_bspline(b, dst, v.as_ref())
    }

    /// `LTAtransformBSpline(b, dst, lta)` (§5.3): the destination made from the transform's
    /// destination geometry (or the coefficients, with `USE_AVERAGE305`'s centre), then the
    /// transform by its array's type.
    pub fn lta_transform_bspline(
        &mut self,
        b: &BSpline,
        dst: Option<&mut Mri>,
        lta: &mut Lta,
    ) -> Result<VolOut, Stop> {
        if lta.xforms.len() != 1 {
            self.error_exit(
                codes::BAD_PARM,
                b"LTAtransformBSpline: numxforms != 1 not supported\n",
            )?;
            return Ok(VolOut::Nothing);
        }
        let c = b.coeff.as_ref().expect("coefficients");
        let mut own = None;
        if dst.is_none() {
            let vg = lta.xforms[0].dst.clone();
            own = Some(if vg.valid == 1 {
                let mut m =
                    self.mri_alloc_sequence(vg.width, vg.height, vg.depth, b.srctype, c.nframes)?;
                self.mri_copy_header(c, Some(&mut m))?;
                self.use_vol_geom(&vg, &mut m)?;
                m
            } else if self.env.get("USE_AVERAGE305").is_some() {
                self.put_err(b"INFO: Environmental variable USE_AVERAGE305 set\n");
                self.put_err(b"INFO: Modifying dst c_(r,a,s), using average_305 values\n");
                let mut m = self.mri_clone_different_type(c, b.srctype)?;
                m.transform_fname.clear();
                m.geom.c = [
                    sse::narrow(-0.0950),
                    sse::narrow(-16.5100),
                    sse::narrow(9.7500),
                ];
                m.geom.ras_good_flag = 1;
                self.reinit_cache(&mut m.geom)?;
                m
            } else {
                self.put_err(b"INFO: Transform dst volume info is not used (valid flag = 0).\n");
                let mut m = self.mri_clone_different_type(c, b.srctype)?;
                m.transform_fname.clear();
                m
            });
        }
        let d: &mut Mri = match dst {
            Some(d) => d,
            None => own.as_mut().expect("the made destination"),
        };
        let kind = lta.kind;
        let r = match kind {
            lta_type::LINEAR_RAS_TO_RAS => {
                let m = lta.xforms[0].m_l.clone();
                self.mri_apply_ras_linear_transform_bspline(b, Some(d), m.as_ref())?
            }
            lta_type::LINEAR_VOX_TO_VOX => {
                let lt = &lta.xforms[0];
                let i2r = if lt.dst.valid == 1 {
                    self.extract_i_to_r(&lt.dst)?
                } else {
                    self.geometry_missing("dst");
                    self.extract_i_to_r(&d.geom)?
                };
                let lt = &lta.xforms[0];
                let r2i = if lt.src.valid == 1 {
                    self.extract_r_to_i(&lt.src)?
                } else {
                    self.geometry_missing("src");
                    self.extract_r_to_i(&c.geom)?
                };
                let m = lta.xforms[0].m_l.clone();
                let t = made(self.matrix_multiply(Some(&i2r), m.as_ref(), None)?);
                let ras = made(self.matrix_multiply(t.as_ref(), r2i.as_ref(), None)?);
                self.mri_apply_ras_linear_transform_bspline(b, Some(d), ras.as_ref())?
            }
            lta_type::LINEAR_PHYSVOX_TO_PHYSVOX => {
                self.lta_change_type(lta, lta_type::LINEAR_RAS_TO_RAS)?;
                let m = lta.xforms[0].m_l.clone();
                self.mri_apply_ras_linear_transform_bspline(b, Some(d), m.as_ref())?
            }
            _ => {
                self.error_exit(
                    codes::BAD_PARM,
                    b"LTAtransformBSpline: unknown linear transform\n",
                )?;
                VolOut::Nothing
            }
        };
        Ok(match (r, own) {
            (VolOut::Given, Some(m)) => VolOut::New(m),
            (other, _) => other,
        })
    }

    /// The three lines of §5.3 for a geometry the transform does not have.
    fn geometry_missing(&mut self, which: &str) {
        self.put_err(
            format!(
                "INFO: LTA {which} geometry information missing!\n      We assume that the {which} volume passed is the\n      same as the {which} for the transform.\n"
            )
            .as_bytes(),
        );
    }
}
