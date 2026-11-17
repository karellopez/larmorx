// SPDX-License-Identifier: Apache-2.0
//! Coordinates and geometry (spec §13, §14): tkregister RAS, the points' conversions, the
//! orientation string, the slice direction and the Talairach point.

use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::mri::{Mri, VolGeom};
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};
use crate::sse;

/// The slice directions (`getSliceDirection`).
pub const MRI_CORONAL: i32 = 0;
pub const MRI_SAGITTAL: i32 = 1;
pub const MRI_AXIAL: i32 = 2;
pub const MRI_UNDEFINED: i32 = -1;

/// The point conversions of §13.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfacePoint {
    /// `MRIRASToSurfaceRAS`.
    RasToSurface,
    /// `MRIvoxelToSurfaceRAS`.
    VoxelToSurface,
    /// `MRIsurfaceRASToVoxel`.
    SurfaceToVoxel,
}

impl Process {
    /// The cached matrices computed where missing (§13): the forward one from the scanner
    /// voxel-to-RAS matrix, the inverse from its inverse (possibly none).
    fn fill_cache(&mut self, g: &mut VolGeom) -> Result<(), Stop> {
        if g.i_to_r.is_none() || g.r_to_i.is_none() {
            let m = self.extract_i_to_r(g)?;
            if g.i_to_r.is_none() {
                g.i_to_r = Some(VolGeom::cache_of(&m));
            }
            if g.r_to_i.is_none() {
                g.r_to_i = self.inverse_of(&m)?;
            }
        }
        Ok(())
    }

    /// The diagnostic print of §13 (with `Gdiag_no > 0` and the verbose bit).
    fn sras_diag(&mut self, head: &[u8], m: Option<&Matrix>) {
        if self.gdiag_no > 0 && self.gdiag & DIAG_VERBOSE != 0 {
            self.put_out(head);
            self.matrix_print(Some(false), m, 5);
        }
    }

    /// `surfaceRASFromVoxel_(vg)` (§13): the tkregister voxel-to-RAS matrix (the cache filled).
    pub fn surface_ras_from_voxel(&mut self, g: &mut VolGeom) -> Result<Matrix, Stop> {
        self.fill_cache(g)?;
        let k = self.vox2ras_tkreg(g)?;
        self.sras_diag(
            b"surfaceRASFromVoxel_() vox2ras --------------------\n",
            Some(&k),
        );
        Ok(k)
    }

    /// `voxelFromSurfaceRAS_(vg)` and `GetSurfaceRASToVoxelMatrix(vg)` (§13): the inverse of the
    /// tkregister voxel-to-RAS matrix, or none (the cache filled).
    pub fn voxel_from_surface_ras(&mut self, g: &mut VolGeom) -> Result<Option<Matrix>, Stop> {
        self.fill_cache(g)?;
        let k = self.vox2ras_tkreg(g)?;
        let inv = self.inverse_of(&k)?;
        self.sras_diag(
            b"voxelFromSurfaceRAS_() ras2vox --------------------\n",
            inv.as_ref(),
        );
        Ok(inv)
    }

    /// `surfaceRASFromRAS_(vg)` (§13): tkregister voxel-to-RAS times the scanner one's inverse
    /// (the cache untouched; no inverse ends the program in the product).
    pub fn surface_ras_from_ras(&mut self, g: &VolGeom) -> Result<Option<Matrix>, Stop> {
        self.scanner_to_tkreg(g)
    }

    /// `MRIRASToSurfaceRAS`, `MRIvoxelToSurfaceRAS`, `MRIsurfaceRASToVoxel` (§13): the point
    /// through the matrix of its conversion; returns `NO_ERROR`.
    pub fn surface_point(
        &mut self,
        g: &mut VolGeom,
        which: SurfacePoint,
        x: f64,
        y: f64,
        z: f64,
    ) -> Result<(i32, [f64; 3]), Stop> {
        let m = match which {
            SurfacePoint::RasToSurface => self.surface_ras_from_ras(g)?,
            SurfacePoint::VoxelToSurface => Some(self.surface_ras_from_voxel(g)?),
            SurfacePoint::SurfaceToVoxel => self.voxel_from_surface_ras(g)?,
        };
        let mut p = Matrix::zeros(4, 1, MATRIX_REAL);
        p.set(1, 1, sse::narrow(x));
        p.set(2, 1, sse::narrow(y));
        p.set(3, 1, sse::narrow(z));
        p.set(4, 1, 1.0);
        let out = match self.matrix_multiply(m.as_ref(), Some(&p), None)? {
            Returned::New(v) => [
                sse::widen(v.at(1, 1)),
                sse::widen(v.at(2, 1)),
                sse::widen(v.at(3, 1)),
            ],
            _ => [0.0; 3],
        };
        Ok((codes::NONE, out))
    }

    /// `MRIdircosToOrientationString(vg, ostr)` (§14): three letters and a NUL (`???` and a NUL,
    /// returning 1, without `ras_good_flag`); returns 0.
    pub fn dircos_to_orientation_string(&mut self, g: &VolGeom, ostr: &mut [u8]) -> i32 {
        if g.ras_good_flag == 0 {
            ostr[..4].copy_from_slice(b"???\0");
            return 1;
        }
        for (j, o) in ostr.iter_mut().take(3).enumerate() {
            let [r, a, s] = g.cosine(j);
            let (ar, aa, as_) = (r.abs(), a.abs(), s.abs());
            *o = if ar > aa && ar > as_ {
                if r > 0.0 { b'R' } else { b'L' }
            } else if aa > as_ {
                if a > 0.0 { b'A' } else { b'P' }
            } else if s > 0.0 {
                b'S'
            } else {
                b'I'
            };
        }
        ostr[3] = 0;
        0
    }

    /// `MRIsliceDirectionName(vg)` (§14).
    pub fn slice_direction_name(&mut self, g: &VolGeom) -> &'static str {
        if g.ras_good_flag == 0 {
            return "unknown";
        }
        let mut o = [0u8; 4];
        self.dircos_to_orientation_string(g, &mut o);
        match o[2] {
            b'L' | b'R' => "sagittal",
            b'A' | b'P' => "coronal",
            b'S' | b'I' => "axial",
            _ => "unknown",
        }
    }

    /// `getSliceDirection(vg)` (§14).
    pub fn get_slice_direction(&mut self, g: &VolGeom) -> i32 {
        match self.slice_direction_name(g) {
            "coronal" => MRI_CORONAL,
            "sagittal" => MRI_SAGITTAL,
            "axial" => MRI_AXIAL,
            _ => MRI_UNDEFINED,
        }
    }

    /// `MRIvoxelToTalairach(mri, x, y, z, &xt, &yt, &zt)` (§14): the voxel's scanner RAS through
    /// the volume's linear Talairach transform; `None` (the outputs untouched) after the error
    /// for a slice direction that is not coronal.
    pub fn voxel_to_talairach(
        &mut self,
        m: &mut Mri,
        x: f64,
        y: f64,
        z: f64,
    ) -> Result<(i32, Option<[f64; 3]>), Stop> {
        let d = self.get_slice_direction(&m.geom);
        if d != MRI_CORONAL {
            self.error_printf(
                codes::UNSUPPORTED,
                format!("MRIvoxelToTalairachVoxel: unsupported slice direction {d}").as_bytes(),
            );
            return Ok((codes::UNSUPPORTED, None));
        }
        let p = self.voxel_to_world(&mut m.geom, x, y, z)?;
        let Some(t) = &m.transform else {
            return Ok((codes::NONE, Some(p)));
        };
        let t = *t.effective();
        let e = |i: usize, j: usize| t[4 * i + j];
        let mut q = [0.0f64; 3];
        for (i, o) in q.iter_mut().enumerate() {
            let s = sse::add64(sse::mul64(e(i, 0), p[0]), sse::mul64(e(i, 1), p[1]));
            let s = sse::add64(s, sse::mul64(e(i, 2), p[2]));
            *o = sse::add64(s, e(i, 3));
        }
        let w = sse::add64(sse::mul64(p[0], e(3, 0)), sse::mul64(p[1], e(3, 1)));
        let w = sse::add64(w, sse::mul64(p[2], e(3, 2)));
        let w = sse::add64(e(3, 3), w);
        if w != 0.0 && w != 1.0 {
            for o in q.iter_mut() {
                *o = sse::div64(*o, w);
            }
        }
        Ok((codes::NONE, Some(q)))
    }
}
