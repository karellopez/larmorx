// SPDX-License-Identifier: Apache-2.0
//! The `-stats` images (`specs/mcflirt.md` §10.5): the temporal mean, variance (divided by
//! `N − 1`) and standard deviation of the corrected series, voxel by voxel. The first and last
//! slices are left at 0.

use super::volume::Volume;

/// `[mean, variance, sigma]` of a series of volumes of one shape.
pub fn temporal(series: &[Volume]) -> [Volume; 3] {
    let first = &series[0];
    let [nx, ny, nz] = first.shape;
    let n = series.len();
    let mut mean = vec![0.0f32; first.len()];
    let mut var = vec![0.0f32; first.len()];
    let mut sigma = vec![0.0f32; first.len()];
    for z in 1..nz.saturating_sub(1) {
        for i in nx * ny * z..nx * ny * (z + 1) {
            let mut s = 0.0f64;
            for v in series {
                s += f64::from(v.data[i]);
            }
            let m = s / n as f64;
            let mut ss = 0.0f64;
            for v in series {
                let d = f64::from(v.data[i]) - m;
                ss += d * d;
            }
            let variance = if n > 1 { ss / (n as f64 - 1.0) } else { 0.0 };
            mean[i] = m as f32;
            var[i] = variance as f32;
            sigma[i] = variance.sqrt() as f32;
        }
    }
    let make = |data| Volume::new(first.shape, first.voxel_size, data);
    [make(mean), make(var), make(sigma)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_slices_stay_zero() {
        let a = Volume::new([2, 2, 3], [1.0; 3], vec![1.0; 12]);
        let b = Volume::new([2, 2, 3], [1.0; 3], vec![3.0; 12]);
        let [m, v, s] = temporal(&[a, b]);
        assert_eq!(m.at(0, 0, 0), 0.0);
        assert_eq!(m.at(1, 1, 1), 2.0);
        assert_eq!(v.at(1, 1, 1), 2.0);
        assert_eq!(s.at(0, 1, 1), 2.0f32.sqrt());
        assert_eq!(s.at(0, 1, 2), 0.0);
    }
}
