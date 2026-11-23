// SPDX-License-Identifier: Apache-2.0
//! The Talairach helpers (spec §46): `ModifyTalairachCRAS`, the Talairach volume of a volume,
//! the four matrices (`MtalairachFromVoxel`, `MtalVoxelFromVoxel`, `MvoxelFromTalairach`,
//! `MvoxelFromTalVoxel`), `TransformWithMatrix`, the four point conversions (FS-487) and the
//! three resamplings (`MRItoTalairachExInterp`, `MRItoTalairachEx`, `MRIfromTalairachEx`:
//! FS-488).

use larmorx_core::defects::{FS_487, FS_488};

use super::sample::sample_type;
use super::{Dst, VolOut};
use crate::maths::Returned;
use crate::maths::matrix::Matrix;
use crate::mri::geom::DIAG_SHOW;
use crate::mri::lta::lta_type;
use crate::mri::{Lta, Mri};
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};
use crate::sse;

/// average_305's centre (`c_r`, `c_a`, `c_s`): the `double` constants stored as `float`s.
const AVERAGE_305: [f64; 3] = [-0.095, -16.51, 9.75];

/// The value of a `Returned` matrix, or none.
fn made(r: Returned<Matrix>) -> Option<Matrix> {
    match r {
        Returned::New(m) => Some(m),
        _ => None,
    }
}

/// One of the four point conversions (§46).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TalPoint {
    /// `MRIvoxelToTalairachEx` (through `MtalairachFromVoxel`).
    VoxelToTalairach,
    /// `MRIvoxelToTalairachVoxelEx` (through `MtalVoxelFromVoxel`).
    VoxelToTalairachVoxel,
    /// `MRItalairachToVoxelEx` (through `MvoxelFromTalairach`).
    TalairachToVoxel,
    /// `MRItalairachVoxelToVoxelEx` (through `MvoxelFromTalVoxel`).
    TalairachVoxelToVoxel,
}

impl TalPoint {
    fn name(self) -> &'static str {
        match self {
            TalPoint::VoxelToTalairach => "MRIvoxelToTalairachEx",
            TalPoint::VoxelToTalairachVoxel => "MRIvoxelToTalairachVoxelEx",
            TalPoint::TalairachToVoxel => "MRItalairachToVoxelEx",
            TalPoint::TalairachVoxelToVoxel => "MRItalairachVoxelToVoxelEx",
        }
    }
}

/// `TransformWithMatrix(M, x, y, z)` (§46): each coordinate rounded to `float`, then rows 1 to 3
/// as `((M[r][1] x + M[r][2] y) + M[r][3] z) + M[r][4]` in `float` (one rounding per
/// operation), widened. Of two NaN operands: rows 1 and 2 keep the matrix element's of each
/// product, row 3 the coordinate's; every sum the left partial sum's. A matrix smaller than
/// 3 x 4 (outside the spec: FreeSurfer's callers pass 4 x 4 ones) gives none.
pub fn transform_with_matrix(m: &Matrix, x: f64, y: f64, z: f64) -> Option<[f64; 3]> {
    use sse::{add32, mul32};
    if m.rows < 3 || m.cols < 4 || m.is_complex() {
        return None;
    }
    let p = [sse::narrow(x), sse::narrow(y), sse::narrow(z)];
    let mut out = [0f64; 3];
    for (r, o) in out.iter_mut().enumerate() {
        let row = r as i32 + 1;
        let prod = |k: usize| {
            let e = m.at(row, k as i32 + 1);
            if r < 2 { mul32(e, p[k]) } else { mul32(p[k], e) }
        };
        let t = add32(add32(prod(0), prod(1)), prod(2));
        *o = sse::widen(add32(t, m.at(row, 4)));
    }
    Some(out)
}

impl Process {
    /// `ModifyTalairachCRAS(mri_tal, lta)` (§46): the centre from the array's one RAS-to-RAS
    /// transform's destination when that geometry is valid (exactly 1); else, without
    /// `NO_AVERAGE305` in the environment, average_305's centre; then the cached matrices
    /// recomputed. An array of another count or type ends the program (`ERROR_BADPARM`).
    pub fn modify_talairach_cras(&mut self, tal: &mut Mri, lta: Option<&Lta>) -> Result<i32, Stop> {
        let mut done = false;
        if let Some(l) = lta {
            if l.num_xforms() != 1 {
                let mut msg = self.progname.clone();
                msg.extend_from_slice(b": xfm has more than one xfrm");
                self.error_exit(codes::BAD_PARM, &msg)?;
                return Ok(codes::BAD_PARM);
            }
            if l.kind != lta_type::LINEAR_RAS_TO_RAS {
                let mut msg = self.progname.clone();
                msg.extend_from_slice(b": xfm passed is not of RAS-to-RAS type");
                self.error_exit(codes::BAD_PARM, &msg)?;
                return Ok(codes::BAD_PARM);
            }
            let dst = &l.xforms[0].dst;
            if dst.valid == 1 {
                if self.gdiag & DIAG_SHOW != 0 && self.gdiag & DIAG_VERBOSE != 0 {
                    self.put_out(b"INFO: Modifying dst c_(r,a,s), using the transform dst\n");
                }
                tal.geom.c = dst.c;
                done = true;
            }
        }
        if !done {
            if self.env.is_set("NO_AVERAGE305") {
                self.put_err(b"INFO: tal c_(r,a,s) not modified\n");
            } else {
                if self.gdiag & DIAG_VERBOSE != 0 {
                    if lta.is_some() {
                        self.put_err(
                            b"INFO: Modifying talairach volume c_(r,a,s) based on average_305.\n\
                              INFO: If not preferred, set environmental varible NO_AVERAGE305 true.\n",
                        );
                    } else {
                        self.put_err(
                            b"INFO: Modifying talairach volume c_(r,a,s) based on average_305\n",
                        );
                    }
                }
                tal.geom.c = AVERAGE_305.map(sse::narrow);
            }
        }
        self.reinit_cache(&mut tal.geom)?;
        Ok(0)
    }

    /// The Talairach volume of `v` (§46): a header of its extent and type with one frame, its
    /// header copied in, then [`Process::modify_talairach_cras`] with the array.
    pub fn talairach_volume(&mut self, v: &Mri, lta: Option<&Lta>) -> Result<Mri, Stop> {
        let g = &v.geom;
        let mut t = self.mri_alloc_header(g.width, g.height, g.depth, v.kind, 1)?;
        self.mri_copy_header(v, Some(&mut t))?;
        self.modify_talairach_cras(&mut t, lta)?;
        Ok(t)
    }

    /// `MtalairachFromVoxel(src, lta)` (§46): the source's voxel-to-RAS matrix, times the
    /// array's first transform on the left (`MatrixMultiply(M, A)`); without an array, the
    /// matrix itself. An array of no transform, or of another type than RAS-to-RAS, ends the
    /// program (after the voxel-to-RAS matrix is made).
    pub fn mtalairach_from_voxel(
        &mut self,
        src: &Mri,
        lta: Option<&Lta>,
    ) -> Result<Option<Matrix>, Stop> {
        let a = self.extract_i_to_r(&src.geom)?;
        let Some(l) = lta else {
            return Ok(Some(a));
        };
        if l.num_xforms() == 0 {
            self.error_exit(codes::BAD_PARM, b"lta does not have xform")?;
            return Ok(None);
        }
        if l.kind != lta_type::LINEAR_RAS_TO_RAS {
            self.error_exit(codes::BAD_PARM, b"lta must be RAS_TO_RAS transform")?;
            return Ok(None);
        }
        Ok(made(self.matrix_multiply(
            l.xforms[0].m_l.as_ref(),
            Some(&a),
            None,
        )?))
    }

    /// `MtalVoxelFromVoxel(src, lta)` (§46): [`Process::mtalairach_from_voxel`], then the
    /// Talairach volume of the source; the latter's RAS-to-voxel matrix times the former.
    pub fn mtal_voxel_from_voxel(
        &mut self,
        src: &Mri,
        lta: Option<&Lta>,
    ) -> Result<Option<Matrix>, Stop> {
        let a = self.mtalairach_from_voxel(src, lta)?;
        let tal = self.talairach_volume(src, lta)?;
        let r2i = self.extract_r_to_i(&tal.geom)?;
        Ok(made(self.matrix_multiply(r2i.as_ref(), a.as_ref(), None)?))
    }

    /// `MvoxelFromTalairach(dst, lta)` (§46): the inverse of the array's first transform (the
    /// identity without an array; the type not checked), then the destination's RAS-to-voxel
    /// matrix times it. FS-487 (both modes): an array of no transform has no matrix (FreeSurfer
    /// reads past the array): `MatrixInverse`'s end.
    pub fn mvoxel_from_talairach(
        &mut self,
        dst: &Mri,
        lta: Option<&Lta>,
    ) -> Result<Option<Matrix>, Stop> {
        let inv = match lta {
            Some(l) => {
                let first = match l.xforms.first() {
                    Some(x) => x.m_l.as_ref(),
                    None => {
                        self.note(
                            &FS_487,
                            "MvoxelFromTalairach: an array of no transform taken as having no matrix (FreeSurfer reads past the array)",
                        );
                        None
                    }
                };
                made(self.matrix_inverse(first, None)?)
            }
            None => made(self.matrix_identity(4, None)?),
        };
        let r2i = self.extract_r_to_i(&dst.geom)?;
        Ok(made(self.matrix_multiply(r2i.as_ref(), inv.as_ref(), None)?))
    }

    /// `MvoxelFromTalVoxel(dst, lta)` (§46): the Talairach volume of the destination, its
    /// voxel-to-RAS matrix, then [`Process::mvoxel_from_talairach`]; the latter times the
    /// former.
    pub fn mvoxel_from_tal_voxel(
        &mut self,
        dst: &Mri,
        lta: Option<&Lta>,
    ) -> Result<Option<Matrix>, Stop> {
        let tal = self.talairach_volume(dst, lta)?;
        let i2r = self.extract_i_to_r(&tal.geom)?;
        let b = self.mvoxel_from_talairach(dst, lta)?;
        Ok(made(self.matrix_multiply(b.as_ref(), Some(&i2r), None)?))
    }

    /// One of the four point conversions (§46) of `(x, y, z)` through its matrix, computed
    /// again for this point (its messages and ends): the point, or none. Returns 0 as
    /// FreeSurfer's do. FS-487 (both modes): where the matrix is none without an end, the
    /// point is left unset with the error `<function>: no matrix to transform the point with`
    /// (FreeSurfer crashes).
    pub fn talairach_point(
        &mut self,
        kind: TalPoint,
        v: &Mri,
        (x, y, z): (f64, f64, f64),
        lta: Option<&Lta>,
    ) -> Result<(i32, Option<[f64; 3]>), Stop> {
        let m = match kind {
            TalPoint::VoxelToTalairach => self.mtalairach_from_voxel(v, lta)?,
            TalPoint::VoxelToTalairachVoxel => self.mtal_voxel_from_voxel(v, lta)?,
            TalPoint::TalairachToVoxel => self.mvoxel_from_talairach(v, lta)?,
            TalPoint::TalairachVoxelToVoxel => self.mvoxel_from_tal_voxel(v, lta)?,
        };
        let p = m.as_ref().and_then(|m| transform_with_matrix(m, x, y, z));
        if p.is_none() {
            let name = kind.name();
            self.note(
                &FS_487,
                format!("{name}: no matrix to transform the point with: the point left unset (FreeSurfer crashes)"),
            );
            self.error(format!("{name}: no matrix to transform the point with").as_bytes());
        }
        Ok((0, p))
    }

    /// `MRItoTalairachExInterp(src, tal, lta, interp)` (§46): [`Process::mtal_voxel_from_voxel`]
    /// printed on standard error; without `tal`, a clone of the source with
    /// [`Process::modify_talairach_cras`]; then `MRIlinearTransformInterp` into it (its result
    /// ignored); returns `tal`.
    pub fn mri_to_talairach_ex_interp(
        &mut self,
        src: &mut Mri,
        tal: Dst,
        lta: Option<&Lta>,
        interp: i32,
    ) -> Result<VolOut, Stop> {
        let m = self.mtal_voxel_from_voxel(src, lta)?;
        self.put_err(b"voxel to talairach voxel transform\n");
        self.matrix_print(Some(true), m.as_ref(), 5);
        match tal {
            Dst::Given(t) => {
                self.mri_linear_transform_interp(src, Dst::Given(t), m.as_ref(), interp)?;
                Ok(VolOut::Given)
            }
            _ => {
                let mut t = self.mri_clone_new(src)?;
                self.modify_talairach_cras(&mut t, lta)?;
                self.mri_linear_transform_interp(src, Dst::Given(&mut t), m.as_ref(), interp)?;
                Ok(VolOut::New(t))
            }
        }
    }

    /// `MRItoTalairachEx(src, tal, lta)` (§46): [`Process::mri_to_talairach_ex_interp`],
    /// trilinear.
    pub fn mri_to_talairach_ex(
        &mut self,
        src: &mut Mri,
        tal: Dst,
        lta: Option<&Lta>,
    ) -> Result<VolOut, Stop> {
        self.mri_to_talairach_ex_interp(src, tal, lta, sample_type::TRILINEAR)
    }

    /// `MRIfromTalairachEx(tal, dst, lta)` (§46): [`Process::mvoxel_from_tal_voxel`] printed on
    /// standard error, then `MRIlinearTransform(tal, dst, M)`; returns `dst`. FS-488 (both
    /// modes): without `dst` the program ends with the check's error first (FreeSurfer uses the
    /// missing target before its check and crashes).
    pub fn mri_from_talairach_ex(
        &mut self,
        tal: &mut Mri,
        dst: Option<&mut Mri>,
        lta: Option<&Lta>,
    ) -> Result<VolOut, Stop> {
        let Some(d) = dst else {
            self.note(
                &FS_488,
                "MRIfromTalairachEx: no target volume: the check's end first (FreeSurfer uses the target before its check and crashes)",
            );
            let mut msg = self.progname.clone();
            msg.extend_from_slice(b": Needs target volume to recover c_(ras)");
            self.error_exit(codes::BAD_PARM, &msg)?;
            return Ok(VolOut::Nothing);
        };
        let m = self.mvoxel_from_tal_voxel(d, lta)?;
        self.put_err(b"talairach voxel to voxel transform\n");
        self.matrix_print(Some(true), m.as_ref(), 5);
        self.mri_linear_transform(tal, Dst::Given(d), m.as_ref())?;
        Ok(VolOut::Given)
    }
}
