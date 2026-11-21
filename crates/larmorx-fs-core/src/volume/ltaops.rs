// SPDX-License-Identifier: Apache-2.0
//! Operations on a transform array, the parts of spec §54 made ready for the clean room:
//! `LTAfillInverse` (FS-505) and `LTAchangeType`'s checks with the change from physical-voxel to
//! RAS-to-RAS (FS-506). The other conversions of §54 are not yet ready: an interim end.

use larmorx_core::defects::{FS_505, FS_506};

use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::mri::Lta;
use crate::mri::geom::VolGeom;
use crate::mri::lta::lta_type;
use crate::process::{Process, Stop, codes};
use crate::sse;

/// `S(g)` (§54): a new 4 x 4 identity with the geometry's voxel sizes on the diagonal; `S'(g)`
/// (`reciprocal`) with `1 / size` in `double` stored as `float` (0 gives an infinity, a NaN
/// stays NaN).
fn scales(g: &VolGeom, reciprocal: bool) -> Matrix {
    let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
    for (k, s) in [g.xsize, g.ysize, g.zsize].into_iter().enumerate() {
        let v = if reciprocal {
            sse::narrow(sse::div64(1.0, sse::widen(s)))
        } else {
            s
        };
        m.set(k as i32 + 1, k as i32 + 1, v);
    }
    m.set(4, 4, 1.0);
    m
}

impl Process {
    /// `M = A · (M · B)` (§54's `A · B`, `specs/freesurfer-maths.md` §8.1): the inner product into
    /// a new matrix, the outer one into `M` itself (its result the transform's matrix: none when
    /// the product gives none), their messages and ends of the program included.
    fn lta_sandwich(
        &mut self,
        a: Option<&Matrix>,
        m: &mut Option<Matrix>,
        b: Option<&Matrix>,
    ) -> Result<(), Stop> {
        let t = match self.matrix_multiply(m.as_ref(), b, None)? {
            Returned::New(x) => Some(x),
            _ => None,
        };
        match self.matrix_multiply(a, t.as_ref(), m.as_mut())? {
            Returned::Output => {}
            Returned::New(x) => *m = Some(x),
            Returned::Nothing => *m = None,
        }
        Ok(())
    }

    /// `LTAfillInverse(lta)` (§54): for each transform in order its matrix's inverse into the
    /// inverse transform's matrix (`MatrixInverse`, its messages included); none ends the program
    /// (`TransformInvert: xform noninvertible`, status 252); then the inverse's source is the
    /// transform's destination and its destination the transform's source. FS-505: an inverse
    /// without a matrix keeps the new one by default, stays without one with `reproduce`.
    pub fn lta_fill_inverse(&mut self, lta: &mut Lta) -> Result<(), Stop> {
        let reproduce = self.mode().reproduce();
        for i in 0..lta.xforms.len() {
            let m = lta.xforms[i].m_l.clone();
            let inv = &mut lta.inv_xforms[i];
            match self.matrix_inverse(m.as_ref(), inv.m_l.as_mut())? {
                Returned::Nothing => {
                    self.error_exit(codes::BAD_PARM, b"TransformInvert: xform noninvertible")?;
                    return Ok(());
                }
                Returned::New(x) => {
                    if reproduce {
                        self.note(
                            &FS_505,
                            "LTAfillInverse: the inverse transform without a matrix stayed without one (FreeSurfer drops the inverse it computes)",
                        );
                    } else {
                        self.note(
                            &FS_505,
                            "LTAfillInverse: the inverse transform without a matrix kept the inverse computed for it",
                        );
                        inv.m_l = Some(x);
                    }
                }
                Returned::Output => {}
            }
            let (src, dst) = (lta.xforms[i].src.clone(), lta.xforms[i].dst.clone());
            let inv = &mut lta.inv_xforms[i];
            self.vg_assign(&mut inv.src, &dst);
            self.vg_assign(&mut inv.dst, &src);
        }
        Ok(())
    }

    /// `LTAchangeType(lta, type)` (§54), the parts ready for the clean room: nothing at all when
    /// the array has the type; else every transform's source, then destination, must be valid
    /// (the first that is not ends the program, status 252, before anything changes); the change
    /// from physical-voxel to RAS-to-RAS (FS-506), then the type and the inverses filled. Other
    /// changes: an interim end (§54's text for them is not yet ready).
    pub fn lta_change_type(&mut self, lta: &mut Lta, kind: i32) -> Result<(), Stop> {
        if lta.kind == kind {
            return Ok(());
        }
        for lt in &lta.xforms {
            for (g, which) in [(&lt.src, "src"), (&lt.dst, "dst")] {
                if g.valid != 1 {
                    self.error_exit(
                        codes::BAD_PARM,
                        format!("LTAchangeType: {which} geometry must be valid\n").as_bytes(),
                    )?;
                    return Ok(());
                }
            }
        }
        if lta.kind == lta_type::LINEAR_PHYSVOX_TO_PHYSVOX && kind == lta_type::LINEAR_RAS_TO_RAS {
            for i in 0..lta.xforms.len() {
                self.lta_physvox_to_ras_one(lta, i)?;
            }
        } else {
            self.error_exit(
                codes::UNSUPPORTED,
                format!(
                    "LTAchangeType: the change from type {} to {kind} is not yet ported\n",
                    lta.kind
                )
                .as_bytes(),
            )?;
            return Ok(());
        }
        lta.kind = kind;
        self.lta_fill_inverse(lta)
    }

    /// One transform's change from physical-voxel to RAS-to-RAS (§54, FS-506). `reproduce`:
    /// `M = inv(V(dst)) · (M · V(src))`, then `M = S'(dst) · (M · S(src))`; by default (rule A, the
    /// RAS transform): `M = S'(dst) · (M · S(src))`, then `M = V(dst) · (M · inv(V(src)))`.
    fn lta_physvox_to_ras_one(&mut self, lta: &mut Lta, i: usize) -> Result<(), Stop> {
        let (src, dst) = (lta.xforms[i].src.clone(), lta.xforms[i].dst.clone());
        let (s_src, s1_dst) = (scales(&src, false), scales(&dst, true));
        if self.mode().reproduce() {
            self.note(
                &FS_506,
                "LTAchangeType: the physical-voxel matrix converted as if it were RAS-to-RAS before its scaling (FreeSurfer's products)",
            );
            let v_src = self.vg_i_to_r(&src)?;
            let Some(vi_dst) = self.vg_r_to_i(&dst)? else {
                self.error_exit(
                    codes::BAD_PARM,
                    b"LTAgetV2V: passed volumes did not have the info on i_to_r or r_to_i.",
                )?;
                return Ok(());
            };
            let m = &mut lta.xforms[i].m_l;
            self.lta_sandwich(Some(&vi_dst), m, Some(&v_src))?;
            self.lta_sandwich(Some(&s1_dst), m, Some(&s_src))?;
        } else {
            self.note(
                &FS_506,
                "LTAchangeType: the physical-voxel matrix scaled to voxels, then made RAS-to-RAS (FreeSurfer converts it as if it were RAS-to-RAS first)",
            );
            let m = &mut lta.xforms[i].m_l;
            self.lta_sandwich(Some(&s1_dst), m, Some(&s_src))?;
            let Some(vi_src) = self.vg_r_to_i(&src)? else {
                self.error_exit(
                    codes::BAD_PARM,
                    b"LTAgetR2R: passed volumes did not have the info on r_to_i or i_to_r",
                )?;
                return Ok(());
            };
            let v_dst = self.vg_i_to_r(&dst)?;
            let m = &mut lta.xforms[i].m_l;
            self.lta_sandwich(Some(&v_dst), m, Some(&vi_src))?;
        }
        Ok(())
    }
}
