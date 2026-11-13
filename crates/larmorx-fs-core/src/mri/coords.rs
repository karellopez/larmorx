// SPDX-License-Identifier: Apache-2.0
//! World and voxel coordinates (spec §10): through the geometry's cached matrices, which are
//! computed when absent and never refreshed.

use super::geom::VolGeom;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::process::{Process, Stop};
use crate::sse;

impl Process {
    /// `MRIvoxelToWorld(mri, x, y, z)` (§10): the cached voxel-to-RAS matrix (computed and kept,
    /// with the RAS-to-voxel one, when absent) applied in float, per row
    /// `((x c1 + y c2) + z c3) + c4`, widened to double.
    pub fn voxel_to_world(
        &mut self,
        g: &mut VolGeom,
        x: f64,
        y: f64,
        z: f64,
    ) -> Result<[f64; 3], Stop> {
        if g.i_to_r.is_none() {
            self.reinit_cache(g)?;
        }
        let a = g.i_to_r.expect("the cache");
        let (xf, yf, zf) = (sse::narrow(x), sse::narrow(y), sse::narrow(z));
        let mut out = [0.0f64; 3];
        for (r, o) in out.iter_mut().enumerate() {
            let s = sse::add32(sse::mul32(xf, a[r]), sse::mul32(yf, a[r + 4]));
            let s = sse::add32(s, sse::mul32(zf, a[r + 8]));
            let s = sse::add32(s, a[r + 12]);
            *o = sse::widen(s);
        }
        Ok(out)
    }

    /// `MRIworldToVoxel(mri, x, y, z)` (§10): the cached RAS-to-voxel matrix (computed and
    /// kept, with the voxel-to-RAS one, when absent) times `(x, y, z, 1)` in float, widened; a
    /// matrix that has no inverse ends the program in the product.
    pub fn world_to_voxel(
        &mut self,
        g: &mut VolGeom,
        x: f64,
        y: f64,
        z: f64,
    ) -> Result<[f64; 3], Stop> {
        if g.r_to_i.is_none() {
            self.reinit_cache(g)?;
        }
        let mut p = Matrix::zeros(4, 1, MATRIX_REAL);
        p.set(1, 1, sse::narrow(x));
        p.set(2, 1, sse::narrow(y));
        p.set(3, 1, sse::narrow(z));
        p.set(4, 1, 1.0);
        let m = g.r_to_i.clone();
        Ok(match self.matrix_multiply(m.as_ref(), Some(&p), None)? {
            Returned::New(v) => [
                sse::widen(v.at(1, 1)),
                sse::widen(v.at(2, 1)),
                sse::widen(v.at(3, 1)),
            ],
            _ => [0.0; 3],
        })
    }

    /// `MRIworldToVoxelIndex(mri, x, y, z)` (§10): the voxel coordinates rounded to the nearest
    /// integers, ties to even (FS-17 beyond the int range).
    pub fn world_to_voxel_index(
        &mut self,
        g: &mut VolGeom,
        x: f64,
        y: f64,
        z: f64,
    ) -> Result<[i32; 3], Stop> {
        let v = self.world_to_voxel(g, x, y, z)?;
        Ok([
            self.to_int(v[0].round_ties_even()),
            self.to_int(v[1].round_ties_even()),
            self.to_int(v[2].round_ties_even()),
        ])
    }
}
