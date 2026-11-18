// SPDX-License-Identifier: Apache-2.0
//! Linear transforms of a volume (spec §29): `MRIsetValues`, `MRIrasXformToVoxelXform`, the six
//! linear transform functions and `MRIgetResampleMatrix`; FS-325.

use larmorx_core::defects::FS_325;

use super::geometry::{MRI_AXIAL, MRI_CORONAL, MRI_SAGITTAL};
use super::sample::{FLT_EPSILON, sample_type};
use super::{Dst, VolOut, Vols};
use crate::clib::printf::sprintf_f64;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix, real_point_f};
use crate::mri::geom::DIAG_SHOW;
use crate::mri::{Mri, types};
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};
use crate::sse;

/// A matrix the function made, or none.
fn made(r: Returned<Matrix>) -> Option<Matrix> {
    match r {
        Returned::New(m) => Some(m),
        _ => None,
    }
}

impl Process {
    /// `MRIsetValues(mri, val)` (§29): every voxel of every frame `MRIsetVoxVal(val)` (frames
    /// outermost, then x, y, z innermost). Returns 0.
    pub fn mri_set_values(&mut self, m: &mut Mri, val: f32) -> Result<i32, Stop> {
        let (w, h, d, nf) = (m.geom.width, m.geom.height, m.geom.depth, m.nframes);
        if w <= 0 || h <= 0 || d <= 0 || nf <= 0 {
            return Ok(0);
        }
        // one value is the same element everywhere: the first voxel's write, copied (the
        // 64-bit type's second store depends on the place: FS-54, voxel by voxel)
        let fast = matches!(
            m.kind,
            types::UCHAR | types::SHORT | types::USHRT | types::INT | types::FLOAT
        ) && m.has_voxels()
            && m.bytes_per_vox as usize == super::arith::elem_size(m.kind).unwrap_or(0) as usize;
        if fast {
            let n = m.bytes_per_vox as usize;
            let count = (w as usize) * (h as usize) * (d as usize) * (nf as usize);
            if m.voxels.as_ref().is_some_and(|v| v.len() >= count * n) {
                self.mri_set_vox_val(m, 0, 0, 0, 0, val, 0)?;
                if let Some(v) = m.voxels.as_deref_mut() {
                    let (first, rest) = v[..count * n].split_at_mut(n);
                    for c in rest.chunks_exact_mut(n) {
                        c.copy_from_slice(first);
                    }
                }
                return Ok(0);
            }
        }
        for f in 0..nf {
            for x in 0..w {
                for y in 0..h {
                    for z in 0..d {
                        self.mri_set_vox_val(m, x, y, z, f, val, 0)?;
                    }
                }
            }
        }
        Ok(0)
    }

    /// `MRIrasXformToVoxelXform(src, dst, m_ras, NULL)` (§29): `extract_r_to_i(dst) * (m_ras *
    /// extract_i_to_r(src))`, the destination the source when absent; the products' messages and
    /// ends of the program (`specs/freesurfer-maths.md` §8.1).
    pub fn mri_ras_xform_to_voxel_xform(
        &mut self,
        src: &Mri,
        dst: Option<&Mri>,
        m_ras: Option<&Matrix>,
    ) -> Result<Option<Matrix>, Stop> {
        let i2r = self.extract_i_to_r(&src.geom)?;
        let inner = made(self.matrix_multiply(m_ras, Some(&i2r), None)?);
        let r2i = self.extract_r_to_i(&dst.unwrap_or(src).geom)?;
        Ok(made(self.matrix_multiply(
            r2i.as_ref(),
            inner.as_ref(),
            None,
        )?))
    }

    /// `MRIlinearTransformInterp(src, dst, A, interp)` (§29): each voxel of the destination (a
    /// clone of the source, or the given one cleared) sampled in the source at `A^-1` of its
    /// indices, every source frame, with §18's `MRIsampleVolumeFrameType`; the source's outside
    /// value first where it is not within `FLT_EPSILON` of 0. FS-325: a failed sample stores the
    /// last one that succeeded (0 before the first, noted).
    pub fn mri_linear_transform_interp(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        a: Option<&Matrix>,
        interp: i32,
    ) -> Result<VolOut, Stop> {
        if !matches!(
            interp,
            sample_type::NEAREST | sample_type::TRILINEAR | sample_type::CUBIC_BSPLINE
        ) {
            self.put_out(
                format!("ERROR: MRIlinearTransformInterp: unrecognized or unsupported interpolation method {interp}\n").as_bytes(),
            );
        }
        let Some(inv) = made(self.matrix_inverse(a, None)?) else {
            self.error_printf(codes::BAD_PARM, b"MRIlinearTransform: xform is singular");
            return Ok(VolOut::Nothing);
        };
        let mut v = Vols::new(src, None, dst);
        if v.has_dst() {
            self.mri_clear(v.d_mut())?;
        } else {
            let c = self.mri_clone_new(v.a())?;
            v.set_dst(c);
        }
        let ov = v.a().outside_val;
        if !(ov.abs() < FLT_EPSILON) {
            self.mri_set_values(v.d_mut(), sse::narrow(ov))?;
            v.d_mut().outside_val = ov;
        }
        if interp == sample_type::CUBIC_BSPLINE {
            self.error_exit(
                codes::UNSUPPORTED,
                b"MRIlinearTransformInterp: cubic B-spline sampling needs fs.bspline (not yet ported)\n",
            )?;
            return Ok(VolOut::Nothing);
        }
        let (w, h, d) = (v.d().geom.width, v.d().geom.height, v.d().geom.depth);
        let nframes = v.a().nframes;
        let fast = !inv.is_complex() && inv.rows == 4 && inv.cols == 4;
        let mut vx = Matrix::zeros(4, 1, MATRIX_REAL);
        let mut vy = Matrix::zeros(4, 1, MATRIX_REAL);
        vx.set(4, 1, 1.0);
        let (mut val, mut written) = (0.0f64, false);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let q = if fast {
                        real_point_f(&inv, [x as f32, y as f32, z as f32, 1.0])
                    } else {
                        vx.set(1, 1, x as f32);
                        vx.set(2, 1, y as f32);
                        vx.set(3, 1, z as f32);
                        self.matrix_multiply(Some(&inv), Some(&vx), Some(&mut vy))?;
                        [vy.at(1, 1), vy.at(2, 1), vy.at(3, 1), vy.at(4, 1)]
                    };
                    let p = [sse::widen(q[0]), sse::widen(q[1]), sse::widen(q[2])];
                    // the diagnostic voxel's test (its only trace: FS-12's notices)
                    if self.nint(p[0]) == self.gvoxel[0] && self.nint(p[1]) == self.gvoxel[1] {
                        self.nint(p[2]);
                    }
                    for f in 0..nframes {
                        let ok =
                            self.frame_type_kept(v.a(), p[0], p[1], p[2], f, interp, &mut val)?;
                        if !ok && !written {
                            self.note(
                                &FS_325,
                                "MRIlinearTransformInterp: a failed sample before any succeeded stored as 0 (FreeSurfer's is memory contents)",
                            );
                        }
                        written |= ok;
                        self.mri_set_vox_val(v.d_mut(), x, y, z, f, sse::narrow(val), 0)?;
                    }
                }
            }
        }
        v.d_mut().geom.ras_good_flag = 1;
        Ok(v.out())
    }

    /// `MRIlinearTransform(src, dst, A)` (§29): trilinear.
    pub fn mri_linear_transform(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        a: Option<&Matrix>,
    ) -> Result<VolOut, Stop> {
        self.mri_linear_transform_interp(src, dst, a, sample_type::TRILINEAR)
    }

    /// `MRIinverseLinearTransform(src, dst, A)` (§29): `MRIlinearTransform` with `A^-1`.
    pub fn mri_inverse_linear_transform(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        a: Option<&Matrix>,
    ) -> Result<VolOut, Stop> {
        let Some(inv) = made(self.matrix_inverse(a, None)?) else {
            self.error_printf(
                codes::BAD_PARM,
                b"MRIinverseLinearTransform: xform is singular!",
            );
            return Ok(VolOut::Nothing);
        };
        self.mri_linear_transform(src, dst, Some(&inv))
    }

    /// The voxel-to-voxel transform of a RAS transform (§29's `MRIapplyRAS...` functions).
    fn ras_to_voxel_of(
        &mut self,
        src: &Mri,
        dst: &Dst,
        m_ras: Option<&Matrix>,
    ) -> Result<Option<Matrix>, Stop> {
        let d = match dst {
            Dst::Given(m) => Some(&**m),
            _ => None,
        };
        self.mri_ras_xform_to_voxel_xform(src, d, m_ras)
    }

    /// `MRIapplyRASlinearTransformInterp(src, dst, m_ras, interp)` (§29).
    pub fn mri_apply_ras_linear_transform_interp(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        m_ras: Option<&Matrix>,
        interp: i32,
    ) -> Result<VolOut, Stop> {
        let m = self.ras_to_voxel_of(src, &dst, m_ras)?;
        self.mri_linear_transform_interp(src, dst, m.as_ref(), interp)
    }

    /// `MRIapplyRASlinearTransform(src, dst, m_ras)` (§29): trilinear.
    pub fn mri_apply_ras_linear_transform(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        m_ras: Option<&Matrix>,
    ) -> Result<VolOut, Stop> {
        self.mri_apply_ras_linear_transform_interp(src, dst, m_ras, sample_type::TRILINEAR)
    }

    /// `MRIapplyRASinverseLinearTransform(src, dst, m_ras)` (§29).
    pub fn mri_apply_ras_inverse_linear_transform(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        m_ras: Option<&Matrix>,
    ) -> Result<VolOut, Stop> {
        let m = self.ras_to_voxel_of(src, &dst, m_ras)?;
        self.mri_inverse_linear_transform(src, dst, m.as_ref())
    }

    /// `MRIgetResampleMatrix(src, template)` (§29): `inverse(extract_i_to_r(src)) *
    /// extract_i_to_r(template)`; NULL after the error for a source of unknown orientation, a
    /// matrix of determinant 0 or a source matrix that does not invert.
    pub fn mri_get_resample_matrix(
        &mut self,
        src: &Mri,
        template: &Mri,
    ) -> Result<Option<Matrix>, Stop> {
        if src.geom.ras_good_flag == 0 {
            let dir = self.get_slice_direction(&src.geom);
            if !matches!(dir, MRI_CORONAL | MRI_SAGITTAL | MRI_AXIAL) {
                self.error_printf(
                    codes::BAD_PARM,
                    b"MRIresample(): source volume orientation is unknown",
                );
                return Ok(None);
            }
        }
        let a = self.extract_i_to_r(&src.geom)?;
        let b = self.extract_i_to_r(&template.geom)?;
        let det = self.matrix_determinant(&a);
        for (m, which, det) in [
            (&a, "source", det),
            (&b, "destination", self.matrix_determinant(&b)),
        ] {
            if det == 0.0 {
                self.errno = 0;
                self.error_printf(
                    codes::BAD_PARM,
                    format!("MRIresample(): {which} matrix has zero determinant; matrix is:")
                        .as_bytes(),
                );
                self.matrix_print(Some(true), Some(m), 5);
                return Ok(None);
            }
        }
        let Some(inv) = self.inverse_of(&a)? else {
            self.errno = 0;
            let mut msg = b"MRIresample(): error inverting matrix; determinant is ".to_vec();
            msg.extend_from_slice(&sprintf_f64(b"%g", sse::widen(det)));
            msg.extend_from_slice(b", matrix is:");
            self.error_printf(codes::BAD_PARM, &msg);
            self.matrix_print(Some(true), Some(&a), 5);
            return Ok(None);
        };
        let m = made(self.matrix_multiply(Some(&inv), Some(&b), None)?);
        if self.gdiag & DIAG_SHOW != 0 && self.gdiag & DIAG_VERBOSE != 0 {
            self.put_out(b"MRIresample() matrix is:\n");
            self.matrix_print(Some(false), m.as_ref(), 5);
        }
        Ok(m)
    }
}
