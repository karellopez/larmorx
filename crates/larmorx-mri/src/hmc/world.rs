// SPDX-License-Identifier: Apache-2.0
//! FSL matrices in world space (`specs/mcflirt.md` §4.3).
//!
//! A file voxel `v` has FSL-mm position `P·v`, `P = D·F`: `D` the voxel sizes and `F` the x
//! reversal of positive-determinant images. A matrix `M` (volume FSL-mm → reference FSL-mm)
//! moves world points of the volume onto the reference by
//! `W = A_ref · P_ref⁻¹ · M · P_in · A_in⁻¹`; ITK, ANTs and nitransforms store the inverse,
//! `T = W⁻¹`, which maps reference points to moving points.

use larmorx_core::affine::Affine;

use super::rigid::{self, Mat4};

/// An image's geometry as the conversion needs it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    /// Voxel-to-world affine (RAS mm).
    pub affine: Mat4,
    pub shape: [usize; 3],
    /// The voxel sizes the FSL-mm coordinates use (`pixdim`).
    pub voxel_size: [f32; 3],
    /// Whether the x axis is reversed in FSL's memory order.
    pub flipped: bool,
}

impl Geometry {
    /// The geometry of an image with `affine`, `shape` and `voxel_size`; reversed in memory if
    /// `flipped`.
    pub fn new(affine: &Affine, shape: [usize; 3], voxel_size: [f32; 3], flipped: bool) -> Self {
        Geometry {
            affine: *affine.rows(),
            shape,
            voxel_size,
            flipped,
        }
    }

    /// `P = D·F`: file voxel indices to FSL-mm.
    pub fn voxel_to_fsl(&self) -> Mat4 {
        let mut f = rigid::IDENTITY;
        if self.flipped {
            f[0][0] = -1.0;
            f[0][3] = self.shape[0] as f64 - 1.0;
        }
        rigid::mul(&rigid::diagonal(self.voxel_size.map(f64::from)), &f)
    }
}

/// `W`: the world (RAS) transform moving points of the input volume onto the reference.
pub fn world_matrix(m: &Mat4, reference: &Geometry, input: &Geometry) -> Mat4 {
    let ref_side = rigid::mul(&reference.affine, &rigid::inverse(&reference.voxel_to_fsl()));
    let in_side = rigid::mul(&input.voxel_to_fsl(), &rigid::inverse(&input.affine));
    rigid::mul(&rigid::mul(&ref_side, m), &in_side)
}

/// `T = W⁻¹` in RAS: reference points to moving points (nitransforms' convention).
pub fn ras_transform(m: &Mat4, reference: &Geometry, input: &Geometry) -> Mat4 {
    rigid::inverse(&world_matrix(m, reference, input))
}

/// `T` in LPS (ITK's and ANTs' convention): `L·T·L` with `L = diag(−1, −1, 1, 1)`.
pub fn itk_transform(m: &Mat4, reference: &Geometry, input: &Geometry) -> Mat4 {
    let l = rigid::diagonal([-1.0, -1.0, 1.0]);
    rigid::mul(&rigid::mul(&l, &ras_transform(m, reference, input)), &l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_maps_to_identity_on_one_grid() {
        let a = Affine::from_rows([
            [-3.0, 0.0, 0.0, 90.0],
            [0.0, 3.0, 0.0, -100.0],
            [0.0, 0.0, 4.0, -60.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let g = Geometry::new(&a, [64, 64, 30], [3.0, 3.0, 4.0], false);
        let w = world_matrix(&rigid::IDENTITY, &g, &g);
        for i in 0..4 {
            for j in 0..4 {
                assert!((w[i][j] - rigid::IDENTITY[i][j]).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn a_translation_in_flipped_storage() {
        // RAS storage (positive determinant): FSL's x runs against the file's x, so a +1 mm
        // FSL-x shift is a -1 mm world-x shift.
        let a = Affine::from_rows([
            [2.0, 0.0, 0.0, -50.0],
            [0.0, 2.0, 0.0, -50.0],
            [0.0, 0.0, 2.0, -50.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let g = Geometry::new(&a, [50, 50, 50], [2.0; 3], true);
        let mut m = rigid::IDENTITY;
        m[0][3] = 1.0;
        let w = world_matrix(&m, &g, &g);
        assert!((w[0][3] + 1.0).abs() < 1e-12);
        let t = ras_transform(&m, &g, &g);
        assert!((t[0][3] - 1.0).abs() < 1e-12);
        let l = itk_transform(&m, &g, &g);
        assert!((l[0][3] + 1.0).abs() < 1e-12);
    }
}
