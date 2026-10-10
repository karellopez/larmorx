// SPDX-License-Identifier: Apache-2.0
//! The final resampling (`specs/mcflirt.md` §9): every volume moved onto the output grid by
//! its matrix, positions stepped in float32 along y, values outside the input's extended grid
//! set to the volume's background value.

use larmorx_core::parallel::{self, ThreadPoolError};
use rayon::prelude::*;

use super::interp::trilinear_extended;
use super::rigid::{self, Mat4};
use super::volume::Volume;

/// The final interpolation (`-sinc_final`, `-spline_final`, `-nn_final`; trilinear by
/// default).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Interpolation {
    #[default]
    Trilinear,
    Sinc,
    Spline,
    Nearest,
}

impl Interpolation {
    pub fn from_name(name: &str) -> Option<Interpolation> {
        Some(match name {
            "trilinear" => Interpolation::Trilinear,
            "sinc" => Interpolation::Sinc,
            "spline" => Interpolation::Spline,
            "nearest" | "nn" => Interpolation::Nearest,
            _ => return None,
        })
    }
}

/// The voxel-to-voxel map of the output grid: `Dsrc⁻¹ · M⁻¹ · Dout`, rounded to float32.
fn output_map(m: &Mat4, src_voxel: [f32; 3], out_voxel: [f32; 3]) -> [[f32; 4]; 3] {
    let dsrc_inv = rigid::diagonal(src_voxel.map(|d| 1.0 / f64::from(d)));
    let dout = rigid::diagonal(out_voxel.map(f64::from));
    let a = rigid::mul(&rigid::mul(&dsrc_inv, &rigid::inverse(m)), &dout);
    [0, 1, 2].map(|i| [0, 1, 2, 3].map(|j| a[i][j] as f32))
}

/// Resamples `src` by `m` (source FSL-mm → output FSL-mm) onto a grid of `shape` and
/// `voxel_size`, filling positions outside `[−1, n]` with `background`.
pub fn resample(
    src: &Volume,
    m: &Mat4,
    shape: [usize; 3],
    voxel_size: [f32; 3],
    interpolation: Interpolation,
    background: f32,
) -> Volume {
    let [nx, ny, nz] = shape;
    let mut data = vec![background; nx * ny * nz];
    if m.iter().flatten().any(|v| !v.is_finite()) {
        return Volume::new(shape, voxel_size, data);
    }
    let a = output_map(m, src.voxel_size, voxel_size);
    let n = src.shape.map(|n| n as f32);
    let spline = (interpolation == Interpolation::Spline).then(|| super::kernels::Spline::new(src));
    for z in 0..nz {
        let zf = z as f32;
        for x in 0..nx {
            let xf = x as f32;
            let mut p = [0, 1, 2].map(|k| xf * a[k][0] + zf * a[k][2] + a[k][3]);
            for y in 0..ny {
                let inside = (0..3).all(|k| p[k] >= -1.0 && p[k] <= n[k]);
                if inside {
                    data[x + nx * (y + ny * z)] = match interpolation {
                        Interpolation::Trilinear => trilinear_extended(src, p[0], p[1], p[2]),
                        Interpolation::Nearest => super::kernels::nearest(src, p),
                        Interpolation::Sinc => super::kernels::sinc_final(src, p),
                        Interpolation::Spline => spline
                            .as_ref()
                            .map_or(background, |s| s.value(p, background)),
                    };
                }
                p = [p[0] + a[0][1], p[1] + a[1][1], p[2] + a[2][1]];
            }
        }
    }
    Volume::new(shape, voxel_size, data)
}

/// Resamples every volume of `series` by its matrix onto `shape`/`voxel_size` (§9), in
/// parallel over volumes. `init` (`-init`) is applied before each matrix.
pub fn resample_series(
    series: &[Volume],
    matrices: &[Mat4],
    init: Option<&Mat4>,
    shape: [usize; 3],
    voxel_size: [f32; 3],
    interpolation: Interpolation,
    n_threads: usize,
) -> Result<Vec<Volume>, ThreadPoolError> {
    parallel::with_threads(n_threads, || {
        series
            .par_iter()
            .zip(matrices)
            .map(|(v, m)| {
                let m = match init {
                    Some(i) => rigid::mul(m, i),
                    None => *m,
                };
                resample(v, &m, shape, voxel_size, interpolation, v.background())
            })
            .collect()
    })
}

/// The mean of the series resampled by `matrices` onto its own grid (trilinear): a float32
/// sum in volume order divided by `N` (§8).
pub fn mean_volume(
    series: &[Volume],
    matrices: &[Mat4],
    n_threads: usize,
) -> Result<Volume, ThreadPoolError> {
    let first = &series[0];
    let moved = resample_series(
        series,
        matrices,
        None,
        first.shape,
        first.voxel_size,
        Interpolation::Trilinear,
        n_threads,
    )?;
    let mut sum = vec![0.0f32; first.len()];
    for v in &moved {
        for (s, &x) in sum.iter_mut().zip(&v.data) {
            *s += x;
        }
    }
    let count = series.len() as f32;
    Ok(Volume::new(
        first.shape,
        first.voxel_size,
        sum.into_iter().map(|s| s / count).collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hmc::rigid::IDENTITY;

    #[test]
    fn identity_resampling_copies_the_volume() {
        let data: Vec<f32> = (0..60).map(|i| (i * 7 % 13) as f32).collect();
        let v = Volume::new([5, 4, 3], [2.0, 2.5, 3.0], data);
        let out = resample(&v, &IDENTITY, v.shape, v.voxel_size, Interpolation::Trilinear, -1.0);
        assert_eq!(out, v);
    }

    #[test]
    fn shifted_volumes_fill_with_the_background() {
        let v = Volume::new([4, 4, 4], [1.0; 3], vec![5.0; 64]);
        let mut m = IDENTITY;
        m[0][3] = 3.0; // the output at x = 0 samples the input at x = -3
        let out = resample(&v, &m, v.shape, v.voxel_size, Interpolation::Trilinear, -7.0);
        assert_eq!(out.at(0, 0, 0), -7.0);
        assert_eq!(out.at(1, 0, 0), -7.0);
        assert_eq!(out.at(2, 0, 0), 5.0); // x = -1: the extended edge
        assert_eq!(out.at(3, 2, 1), 5.0);
    }
}
