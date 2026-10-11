// SPDX-License-Identifier: Apache-2.0
//! The `-stats` images (`specs/mcflirt.md` §10.5): the temporal mean, variance (divided by
//! `N − 1`) and standard deviation of the corrected series, voxel by voxel. mcflirt leaves the
//! first and last slices at 0 (FSL-08); larmorx computes every slice unless asked to reproduce
//! mcflirt.

use super::volume::Volume;

/// `[mean, variance, sigma]` of a series of volumes of one shape; with `end_slices_zero`, the
/// first and last slices are left at 0, as mcflirt leaves them (FSL-08).
pub fn temporal(series: &[Volume], end_slices_zero: bool) -> [Volume; 3] {
    let first = &series[0];
    let [nx, ny, nz] = first.shape;
    let n = series.len();
    let mut mean = vec![0.0f32; first.len()];
    let mut var = vec![0.0f32; first.len()];
    let mut sigma = vec![0.0f32; first.len()];
    let slices = if end_slices_zero {
        1..nz.saturating_sub(1)
    } else {
        0..nz
    };
    for z in slices {
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
    fn end_slices_both_modes() {
        let a = Volume::new([2, 2, 3], [1.0; 3], vec![1.0; 12]);
        let b = Volume::new([2, 2, 3], [1.0; 3], vec![3.0; 12]);
        let series = [a, b];
        let [m, v, s] = temporal(&series, true);
        assert_eq!(m.at(0, 0, 0), 0.0);
        assert_eq!(m.at(1, 1, 1), 2.0);
        assert_eq!(v.at(1, 1, 1), 2.0);
        assert_eq!(s.at(0, 1, 1), 2.0f32.sqrt());
        assert_eq!(s.at(0, 1, 2), 0.0);
        let [m, v, s] = temporal(&series, false);
        assert_eq!(m.at(0, 0, 0), 2.0);
        assert_eq!(v.at(1, 1, 2), 2.0);
        assert_eq!(s.at(0, 1, 2), 2.0f32.sqrt());
    }
}
