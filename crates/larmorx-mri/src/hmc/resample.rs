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

/// Visits the output voxels whose source position lies inside `src`'s extended grid
/// (`[−1, n]` per axis), in mcflirt's order (z outermost, then x, then y innermost; §9.1), with
/// their linear index and interpolated value; stops early when `f` returns `false`. Returns
/// whether every visit returned `true`. Nothing is visited for a non-finite matrix.
fn visit(
    src: &Volume,
    m: &Mat4,
    shape: [usize; 3],
    voxel_size: [f32; 3],
    interpolation: Interpolation,
    background: f32,
    mut f: impl FnMut(usize, f32) -> bool,
) -> bool {
    let [nx, ny, nz] = shape;
    if m.iter().flatten().any(|v| !v.is_finite()) {
        return true;
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
                    let value = match interpolation {
                        Interpolation::Trilinear => trilinear_extended(src, p[0], p[1], p[2]),
                        Interpolation::Nearest => super::kernels::nearest(src, p),
                        Interpolation::Sinc => super::kernels::sinc_final(src, p),
                        Interpolation::Spline => spline
                            .as_ref()
                            .map_or(background, |s| s.value(p, background)),
                    };
                    if !f(x + nx * (y + ny * z), value) {
                        return false;
                    }
                }
                p = [p[0] + a[0][1], p[1] + a[1][1], p[2] + a[2][1]];
            }
        }
    }
    true
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
    visit(
        src,
        m,
        shape,
        voxel_size,
        interpolation,
        background,
        |i, v| {
            data[i] = v;
            true
        },
    );
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

/// Whether resampling `series` with `interpolation` gives any value (bit for bit) other than
/// `done`, the same series already resampled by [`resample_series`] with another method: each
/// volume is compared voxel by voxel and the search stops at the first difference, so the
/// answer costs little where the two differ (FSL-59: mcflirt 6.0.7.23 resamples `-sinc_final`
/// and `-nn_final` trilinearly, `specs/mcflirt.md` §9.3). The answer does not depend on the
/// thread count.
#[allow(clippy::too_many_arguments)]
pub fn series_differ(
    series: &[Volume],
    matrices: &[Mat4],
    init: Option<&Mat4>,
    shape: [usize; 3],
    voxel_size: [f32; 3],
    interpolation: Interpolation,
    done: &[Volume],
    n_threads: usize,
) -> Result<bool, ThreadPoolError> {
    parallel::with_threads(n_threads, || {
        series
            .par_iter()
            .zip(matrices)
            .zip(done)
            .any(|((v, m), d)| {
                let m = match init {
                    Some(i) => rigid::mul(m, i),
                    None => *m,
                };
                // Only the spline reads the background inside the grid; outside it both series
                // hold the background.
                let background = if interpolation == Interpolation::Spline {
                    v.background()
                } else {
                    0.0
                };
                let same = visit(
                    v,
                    &m,
                    shape,
                    voxel_size,
                    interpolation,
                    background,
                    |i, x| x.to_bits() == d.data[i].to_bits(),
                );
                !same
            })
    })
}

/// `f` of each volume of `series` resampled with `interpolation` (as [`resample_series`] would
/// give it), the volumes in parallel but none kept, summed in volume order: what a series that
/// is not written would hold (FSL-59: the integer conversion of mcflirt 6.0.7.23's trilinear
/// series where larmorx writes another one).
#[allow(clippy::too_many_arguments)]
pub fn fold_resampled<T: Send + Default + std::ops::Add<Output = T>>(
    series: &[Volume],
    matrices: &[Mat4],
    init: Option<&Mat4>,
    shape: [usize; 3],
    voxel_size: [f32; 3],
    interpolation: Interpolation,
    n_threads: usize,
    f: impl Fn(&Volume) -> T + Sync + Send,
) -> Result<T, ThreadPoolError> {
    parallel::with_threads(n_threads, || {
        let parts: Vec<T> = series
            .par_iter()
            .zip(matrices)
            .map(|(v, m)| {
                let m = match init {
                    Some(i) => rigid::mul(m, i),
                    None => *m,
                };
                f(&resample(
                    v,
                    &m,
                    shape,
                    voxel_size,
                    interpolation,
                    v.background(),
                ))
            })
            .collect();
        parts.into_iter().fold(T::default(), |a, b| a + b)
    })
}

/// The mean of the series resampled by `matrices` onto the grid of `reference` (trilinear):
/// a float32 sum in volume order divided by `N` (§8).
pub fn mean_volume(
    series: &[Volume],
    matrices: &[Mat4],
    reference: &Volume,
    n_threads: usize,
) -> Result<Volume, ThreadPoolError> {
    let first = reference;
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
        let out = resample(
            &v,
            &IDENTITY,
            v.shape,
            v.voxel_size,
            Interpolation::Trilinear,
            -1.0,
        );
        assert_eq!(out, v);
    }

    #[test]
    fn shifted_volumes_fill_with_the_background() {
        let v = Volume::new([4, 4, 4], [1.0; 3], vec![5.0; 64]);
        let mut m = IDENTITY;
        m[0][3] = 3.0; // the output at x = 0 samples the input at x = -3
        let out = resample(
            &v,
            &m,
            v.shape,
            v.voxel_size,
            Interpolation::Trilinear,
            -7.0,
        );
        assert_eq!(out.at(0, 0, 0), -7.0);
        assert_eq!(out.at(1, 0, 0), -7.0);
        assert_eq!(out.at(2, 0, 0), 5.0); // x = -1: the extended edge
        assert_eq!(out.at(3, 2, 1), 5.0);
    }
}
