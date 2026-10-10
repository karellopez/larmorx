// SPDX-License-Identifier: Apache-2.0
//! The coarse reference grids of the 8 mm and 4 mm stages (`specs/mcflirt.md` §5.3).
//!
//! The reference is sampled on an isotropic grid of spacing `s` without smoothing: grid point
//! `q` lies at reference voxel position `q·(s/d)` per axis (the step added up in float32 from
//! 0), its value interpolated trilinearly with neighbours outside the reference counting as 0.

use super::interp::trilinear_zero;
use super::volume::Volume;

/// The per-axis voxel positions of a grid of spacing `s` over `n` voxels of size `d`:
/// `max(1, ⌊n / step⌋)` positions `0, step, 2·step, …` (float32 sums), `step = s / d`.
fn positions(n: usize, d: f32, s: f32) -> Vec<f32> {
    let step = s / d;
    let count = ((n as f32 / step).floor() as usize).max(1);
    let mut out = Vec::with_capacity(count);
    let mut p = 0.0f32;
    for _ in 0..count {
        out.push(p);
        p += step;
    }
    out
}

/// The reference resampled to an isotropic grid of spacing `s` mm (a [`Volume`] whose voxel
/// size is `s` in every axis).
pub fn subsample(reference: &Volume, s: f32) -> Volume {
    let px = positions(reference.shape[0], reference.voxel_size[0], s);
    let py = positions(reference.shape[1], reference.voxel_size[1], s);
    let pz = positions(reference.shape[2], reference.voxel_size[2], s);
    let mut data = Vec::with_capacity(px.len() * py.len() * pz.len());
    for &z in &pz {
        for &y in &py {
            for &x in &px {
                data.push(trilinear_zero(reference, x, y, z));
            }
        }
    }
    Volume::new([px.len(), py.len(), pz.len()], [s; 3], data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_sizes_use_float32_division() {
        // 64 voxels of 3.125 mm: 25 points at 8 mm and 50 at 4 mm (spec §5.3).
        assert_eq!(positions(64, 3.125, 8.0).len(), 25);
        assert_eq!(positions(64, 3.125, 4.0).len(), 50);
        // Voxels larger than the spacing: at least one point.
        assert_eq!(positions(1, 10.0, 8.0).len(), 1);
        assert_eq!(positions(3, 10.0, 8.0).len(), 3);
    }

    #[test]
    fn grid_values_interpolate_the_reference() {
        let data: Vec<f32> = (0..4 * 4 * 4).map(|i| (i % 4) as f32).collect();
        let v = Volume::new([4, 4, 4], [2.0; 3], data);
        let g = subsample(&v, 4.0);
        assert_eq!(g.shape, [2, 2, 2]);
        assert_eq!(g.voxel_size, [4.0; 3]);
        // Grid x positions 0 and 2 (reference voxels): values 0 and 2.
        assert_eq!(g.at(0, 0, 0), 0.0);
        assert_eq!(g.at(1, 1, 1), 2.0);
    }
}
