// SPDX-License-Identifier: Apache-2.0
//! Operations on a transform array, the parts of spec §54 made ready for the clean room:
//! `LTAfillInverse` (FS-505) and `LTAchangeType`'s checks with the six changes between the voxel,
//! RAS-to-RAS and physical-voxel types (FS-506 for physical-voxel to RAS-to-RAS), its unsupported
//! targets and the arrays of another type. The changes to and from REGISTER_DAT, FSL and coronal
//! RAS are not yet ready: an interim end.

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
    /// a new matrix, the outer one into `M` itself, their messages and ends of the program
    /// included. An outer product that gives nothing (its operands' sizes do not match) leaves
    /// `M` as it was (observed: `vltapv`, a 3 x 4 matrix changed from physical-voxel to RAS-to-RAS
    /// prints both products' mismatches, then the inverse's "(3 x 4) is not square").
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
            Returned::Output | Returned::Nothing => {}
            Returned::New(x) => *m = Some(x),
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
    /// (the first that is not ends the program, status 252, before anything changes); then the
    /// six changes between the voxel, RAS-to-RAS and physical-voxel types (each transform's whole
    /// change in turn; physical-voxel to RAS-to-RAS with FS-506), a target those types do not
    /// support ending the program, an array of another type kept; the type and the inverses
    /// filled. The changes to and from REGISTER_DAT, FSL and coronal RAS: an interim end (§54's
    /// text for them is not yet ready).
    pub fn lta_change_type(&mut self, lta: &mut Lta, kind: i32) -> Result<(), Stop> {
        use lta_type::{
            FSLREG_TYPE, LINEAR_CORONAL_RAS_TO_CORONAL_RAS, LINEAR_PHYSVOX_TO_PHYSVOX,
            LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, REGISTER_DAT,
        };
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
        let from = lta.kind;
        let name = match from {
            LINEAR_RAS_TO_RAS => "ras-to-ras",
            LINEAR_VOX_TO_VOX => "vox-to-vox",
            LINEAR_PHYSVOX_TO_PHYSVOX => "physvox-to-physvox",
            REGISTER_DAT | FSLREG_TYPE | LINEAR_CORONAL_RAS_TO_CORONAL_RAS => {
                return self.lta_change_not_ready(from, kind);
            }
            // an array of another type: its matrices and type kept, its inverses filled
            _ => return self.lta_fill_inverse(lta),
        };
        if kind == REGISTER_DAT || kind == FSLREG_TYPE {
            return self.lta_change_not_ready(from, kind);
        }
        let known = matches!(
            kind,
            LINEAR_RAS_TO_RAS | LINEAR_VOX_TO_VOX | LINEAR_PHYSVOX_TO_PHYSVOX
        );
        if !known {
            self.error_exit(
                codes::BAD_PARM,
                format!("LTAchangeType: you are requesting {name} to {kind} ").as_bytes(),
            )?;
            return Ok(());
        }
        for i in 0..lta.xforms.len() {
            match (from, kind) {
                (LINEAR_RAS_TO_RAS, _) => {
                    self.lta_ras_to_vox_one(lta, i)?;
                    if kind == LINEAR_PHYSVOX_TO_PHYSVOX {
                        self.lta_vox_to_physvox_one(lta, i)?;
                    }
                }
                (LINEAR_VOX_TO_VOX, LINEAR_RAS_TO_RAS) => self.lta_vox_to_ras_one(lta, i)?,
                (LINEAR_VOX_TO_VOX, _) => self.lta_vox_to_physvox_one(lta, i)?,
                (_, LINEAR_VOX_TO_VOX) => {
                    let (src, dst) = (&lta.xforms[i].src, &lta.xforms[i].dst);
                    let (s_src, s1_dst) = (scales(src, false), scales(dst, true));
                    self.lta_sandwich(Some(&s1_dst), &mut lta.xforms[i].m_l, Some(&s_src))?;
                }
                _ => self.lta_physvox_to_ras_one(lta, i)?,
            }
        }
        lta.kind = kind;
        self.lta_fill_inverse(lta)
    }

    /// The changes to and from REGISTER_DAT, FSL and coronal RAS (§54): not yet ready for the
    /// clean room, an interim end.
    fn lta_change_not_ready(&mut self, from: i32, kind: i32) -> Result<(), Stop> {
        self.error_exit(
            codes::UNSUPPORTED,
            format!("LTAchangeType: the change from type {from} to {kind} is not yet ported\n")
                .as_bytes(),
        )?;
        Ok(())
    }

    /// One transform's change from RAS-to-RAS to voxel (§54): `V(src)`, then `inv(V(dst))` (none
    /// ends the program, status 252); `M = inv(V(dst)) · (M · V(src))`.
    fn lta_ras_to_vox_one(&mut self, lta: &mut Lta, i: usize) -> Result<(), Stop> {
        let (src, dst) = (lta.xforms[i].src.clone(), lta.xforms[i].dst.clone());
        let v_src = self.vg_i_to_r(&src)?;
        let Some(vi_dst) = self.vg_r_to_i(&dst)? else {
            self.error_exit(
                codes::BAD_PARM,
                b"LTAgetV2V: passed volumes did not have the info on i_to_r or r_to_i.",
            )?;
            return Ok(());
        };
        self.lta_sandwich(Some(&vi_dst), &mut lta.xforms[i].m_l, Some(&v_src))
    }

    /// One transform's change from voxel to RAS-to-RAS (§54): `inv(V(src))` (none ends the
    /// program, status 252), then `V(dst)`; `M = V(dst) · (M · inv(V(src)))`.
    fn lta_vox_to_ras_one(&mut self, lta: &mut Lta, i: usize) -> Result<(), Stop> {
        let (src, dst) = (lta.xforms[i].src.clone(), lta.xforms[i].dst.clone());
        let Some(vi_src) = self.vg_r_to_i(&src)? else {
            self.error_exit(
                codes::BAD_PARM,
                b"LTAgetR2R: passed volumes did not have the info on r_to_i or i_to_r",
            )?;
            return Ok(());
        };
        let v_dst = self.vg_i_to_r(&dst)?;
        self.lta_sandwich(Some(&v_dst), &mut lta.xforms[i].m_l, Some(&vi_src))
    }

    /// One transform's change from voxel to physical-voxel (§54): `M = S(dst) · (M · S'(src))`.
    fn lta_vox_to_physvox_one(&mut self, lta: &mut Lta, i: usize) -> Result<(), Stop> {
        let (src, dst) = (&lta.xforms[i].src, &lta.xforms[i].dst);
        let (s1_src, s_dst) = (scales(src, true), scales(dst, false));
        self.lta_sandwich(Some(&s_dst), &mut lta.xforms[i].m_l, Some(&s1_src))
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
