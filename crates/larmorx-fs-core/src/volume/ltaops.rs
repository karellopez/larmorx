// SPDX-License-Identifier: Apache-2.0
//! Operations on a transform array (spec §54): `LTAfillInverse` (FS-505) and `LTAchangeType`:
//! its checks, the six changes between the voxel, RAS-to-RAS and physical-voxel types (FS-506 for
//! physical-voxel to RAS-to-RAS), the changes to and from REGISTER_DAT, FSL and coronal RAS
//! (FS-504's diagnostic print, FS-507, FS-508), its unsupported targets and the arrays of another
//! type. The other operations are in `ltachain.rs`.

use larmorx_core::defects::{FS_504, FS_505, FS_506, FS_507, FS_508};

use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::maths::{Output, Returned};
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

    /// FS-504's end (§54): `<function>: <why> (FreeSurfer reads memory it does not own here)`,
    /// status 252, both modes, noted.
    pub(crate) fn lta_fs504(&mut self, function: &str, why: &str) -> Result<(), Stop> {
        self.note(
            &FS_504,
            format!(
                "{function}: {why}: the program ended (FreeSurfer reads memory it does not own)"
            ),
        );
        self.error_exit(
            codes::BAD_PARM,
            format!("{function}: {why} (FreeSurfer reads memory it does not own here)\n")
                .as_bytes(),
        )
    }

    /// `LTAchangeType(lta, type)` (§54): nothing at all when the array has the type; else every
    /// transform's source, then destination, must be valid (the first that is not ends the
    /// program, status 252, before anything changes); then the change (each transform's whole
    /// change in turn): the six between the voxel, RAS-to-RAS and physical-voxel types
    /// (physical-voxel to RAS-to-RAS with FS-506), those to and from REGISTER_DAT, FSL and
    /// coronal RAS (FS-507, FS-508; through RAS or REGISTER_DAT as two whole calls), a target
    /// the type does not support ending the program, an array of another type kept; from
    /// REGISTER_DAT, coronal RAS and FSL the diagnostic print (FS-504 without transforms); the
    /// type and the inverses filled.
    pub fn lta_change_type(&mut self, lta: &mut Lta, kind: i32) -> Result<(), Stop> {
        use lta_type::{
            FSLREG_TYPE, LINEAR_CORONAL_RAS_TO_CORONAL_RAS, LINEAR_PHYSVOX_TO_PHYSVOX,
            LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, REGISTER_DAT,
        };
        const RAS: i32 = LINEAR_RAS_TO_RAS;
        const VOX: i32 = LINEAR_VOX_TO_VOX;
        const PHYS: i32 = LINEAR_PHYSVOX_TO_PHYSVOX;
        const TK: i32 = REGISTER_DAT;
        const FSL: i32 = FSLREG_TYPE;
        const COR: i32 = LINEAR_CORONAL_RAS_TO_CORONAL_RAS;
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
        // the target the type does not support: its name in the error
        let unsupported =
            |name: &str| format!("LTAchangeType: you are requesting {name} to {kind} ");
        let supported = match from {
            RAS => matches!(kind, VOX | PHYS | TK | FSL),
            VOX => matches!(kind, RAS | PHYS | TK | FSL),
            PHYS => matches!(kind, RAS | VOX | TK | FSL),
            TK => matches!(kind, RAS | VOX | FSL),
            FSL => matches!(kind, TK | RAS | VOX),
            COR => matches!(kind, RAS | TK | FSL),
            // an array of another type: its matrices and type kept, its inverses filled
            _ => return self.lta_fill_inverse(lta),
        };
        if !supported {
            let msg = match from {
                RAS => unsupported("ras-to-ras"),
                VOX => unsupported("vox-to-vox"),
                PHYS => unsupported("physvox-to-physvox"),
                TK => {
                    format!("LTAchangeType unsupported: you are requesting REGISTER_DAT to {kind} ")
                }
                FSL => {
                    format!("LTAchangeType unsupported: you are requesting FSLREG_TYPE to {kind} ")
                }
                _ => format!(
                    "LTAchangeType unsupported: you are requesting COR_RAS_TO_COR_RAS to {kind} "
                ),
            };
            self.error_exit(codes::BAD_PARM, msg.as_bytes())?;
            return Ok(());
        }
        // the changes made as two whole calls: to RAS (or REGISTER_DAT), then to the target
        let through = match (from, kind) {
            (VOX | PHYS | COR, TK | FSL) | (TK, VOX) => Some(RAS),
            (FSL, RAS | VOX) => Some(TK),
            _ => None,
        };
        if let Some(mid) = through {
            self.lta_change_type(lta, mid)?;
            self.lta_change_type(lta, kind)?;
        } else {
            for i in 0..lta.xforms.len() {
                match (from, kind) {
                    (RAS, TK) => {
                        let (src, dst) = (lta.xforms[i].src.clone(), lta.xforms[i].dst.clone());
                        let r = self.tkreg_from_native(&dst, &src, lta.xforms[i].m_l.as_ref())?;
                        self.lta_copy_into_m(r, &mut lta.xforms[i].m_l)?;
                    }
                    (RAS, FSL) => {
                        let (src, dst) = (lta.xforms[i].src.clone(), lta.xforms[i].dst.clone());
                        let r = self.tkreg_from_native(&dst, &src, lta.xforms[i].m_l.as_ref())?;
                        let f = self.tkreg_to_fsl(&dst, &src, r.as_ref())?;
                        self.lta_copy_into_m(f, &mut lta.xforms[i].m_l)?;
                    }
                    (RAS, _) => {
                        self.lta_ras_to_vox_one(lta, i)?;
                        if kind == PHYS {
                            self.lta_vox_to_physvox_one(lta, i)?;
                        }
                    }
                    (VOX, RAS) => self.lta_vox_to_ras_one(lta, i)?,
                    (VOX, _) => self.lta_vox_to_physvox_one(lta, i)?,
                    (PHYS, VOX) => {
                        let (src, dst) = (&lta.xforms[i].src, &lta.xforms[i].dst);
                        let (s_src, s1_dst) = (scales(src, false), scales(dst, true));
                        self.lta_sandwich(Some(&s1_dst), &mut lta.xforms[i].m_l, Some(&s_src))?;
                    }
                    (PHYS, _) => self.lta_physvox_to_ras_one(lta, i)?,
                    (TK, RAS) => self.lta_tkreg_to_ras_one(lta, i)?,
                    (TK, _) => {
                        let (src, dst) = (lta.xforms[i].src.clone(), lta.xforms[i].dst.clone());
                        let f = self.tkreg_to_fsl(&dst, &src, lta.xforms[i].m_l.as_ref())?;
                        self.lta_copy_into_m(f, &mut lta.xforms[i].m_l)?;
                    }
                    (FSL, _) => {
                        let (src, dst) = (lta.xforms[i].src.clone(), lta.xforms[i].dst.clone());
                        let r = self.fsl_to_tkreg(&dst, &src, lta.xforms[i].m_l.as_ref())?;
                        self.lta_copy_into_m(r, &mut lta.xforms[i].m_l)?;
                    }
                    _ => {
                        // coronal RAS to RAS: M = RASFromSurfaceRAS_(dst header) · M
                        let h = self.header_of(&lta.xforms[i].dst)?;
                        let c = match self.tkreg_to_scanner(&h, None)? {
                            Returned::New(c) => Some(c),
                            _ => None,
                        };
                        let m = &mut lta.xforms[i].m_l;
                        let b = m.clone();
                        if let Returned::New(x) =
                            self.matrix_multiply(c.as_ref(), b.as_ref(), m.as_mut())?
                        {
                            *m = Some(x);
                        }
                    }
                }
            }
        }
        if matches!(from, TK | FSL | COR) && self.gdiag_no > 0 {
            self.put_out(b"transformed matrix:\n");
            let Some(first) = lta.xforms.first() else {
                self.flush();
                return self.lta_fs504("LTAchangeType", "a transform array without transforms");
            };
            let m = first.m_l.clone();
            self.matrix_print(Some(false), m.as_ref(), 5);
        }
        lta.kind = kind;
        self.lta_fill_inverse(lta)
    }

    /// "Copied into `M`" (§54): `MatrixCopy(new, M)` (`specs/freesurfer-maths.md` §2.4: a given
    /// `M` smaller than the new matrix keeps its numbers, none stays none). A conversion that gives
    /// no matrix is FS-508 (FreeSurfer's copy frees `M` and goes on using it): the end of the
    /// program, status 252, both modes.
    fn lta_copy_into_m(&mut self, new: Option<Matrix>, m: &mut Option<Matrix>) -> Result<(), Stop> {
        let Some(new) = new else {
            self.note(
                &FS_508,
                "LTAchangeType: the FSL conversion gave no matrix: the program ended (FreeSurfer frees the transform's matrix and goes on using it)",
            );
            return self.error_exit(
                codes::BAD_PARM,
                b"LTAchangeType: the FSL conversion gave no matrix (FreeSurfer frees the transform's matrix and goes on using it here)\n",
            );
        };
        if let Some(m) = m.as_mut() {
            self.matrix_copy(Some(&new), Output::Given(m))?;
        }
        Ok(())
    }

    /// One transform's change from REGISTER_DAT to RAS-to-RAS (§54): headers of the source's and
    /// the destination's geometry, then `MRItkReg2Native(destination, source, M)` copied into
    /// `M`. FS-507: with `reproduce` FreeSurfer's change of the FIRST transform each time (from its
    /// previous result), by default transform `i`; noted when the array holds more than one.
    fn lta_tkreg_to_ras_one(&mut self, lta: &mut Lta, i: usize) -> Result<(), Stop> {
        let mut j = i;
        if lta.xforms.len() > 1 {
            if self.mode().reproduce() {
                self.note(
                    &FS_507,
                    "LTAchangeType: REGISTER_DAT to RAS made on the first transform once per transform (FreeSurfer's), the others unchanged",
                );
                j = 0;
            } else {
                self.note(
                    &FS_507,
                    "LTAchangeType: REGISTER_DAT to RAS made on each transform once (FreeSurfer converts the first one once per transform)",
                );
            }
        }
        let src = self.header_of(&lta.xforms[j].src)?;
        let dst = self.header_of(&lta.xforms[j].dst)?;
        let r = self.tkreg_to_native(&dst, &src, lta.xforms[j].m_l.as_ref())?;
        self.lta_copy_into_m(r, &mut lta.xforms[j].m_l)
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
