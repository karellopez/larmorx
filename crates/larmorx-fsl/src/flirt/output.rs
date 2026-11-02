// SPDX-License-Identifier: Apache-2.0
//! The resampled image (`specs/flirt.md` §9.2, §9.3): the input, blurred to the reference's
//! smallest voxel size, interpolated at the pre-image of every reference voxel (the shared
//! engine's interpolators), padded outside the input.
//!
//! Output voxels are independent: slices run in parallel and each voxel's position is stepped
//! along its row as the original does, so the image does not depend on the thread count.

use std::f64::consts::PI;

use larmorx_core::math;
use larmorx_mri::hmc::interp::{floor, trilinear_extended};
use larmorx_mri::hmc::kernels::{Spline, nearest};
use larmorx_mri::hmc::volume::Volume;
use rayon::prelude::*;

use super::mat::{self, Mat4};
use super::pyramid;
use super::{Interp, SincWindow};

/// A windowed-sinc kernel of full width `width` voxels (§9.2): its half-width and 1201 samples
/// over `[−h, h]`, read by linear interpolation.
#[derive(Clone, Debug)]
pub struct SincKernel {
    half: i64,
    values: Vec<f32>,
}

impl SincKernel {
    pub fn new(width: f32, window: SincWindow) -> SincKernel {
        let w = (width.round() as i64).max(1);
        let half = ((w - 1) / 2).max(1);
        let h = half as f64;
        let n = 1201usize;
        let mid = (n - 1) / 2;
        let values = (0..n)
            .map(|k| {
                let u = (k as f64 - mid as f64) / mid as f64 * h;
                let s = if u.abs() < 1e-7 {
                    1.0 - u.abs()
                } else {
                    math::sin(PI * u) / (PI * u)
                };
                let win = match window {
                    SincWindow::Rectangular => 1.0,
                    SincWindow::Hanning => 0.5 + 0.5 * math::cos(PI * u / h),
                    SincWindow::Blackman => {
                        0.42 + 0.5 * math::cos(PI * u / h) + 0.08 * math::cos(2.0 * PI * u / h)
                    }
                };
                (s * win) as f32
            })
            .collect();
        SincKernel { half, values }
    }

    #[inline]
    fn at(&self, u: f32) -> f32 {
        let mid = ((self.values.len() - 1) / 2) as f32;
        let x = u / self.half as f32 * mid + mid;
        let n = floor(x);
        if !(n >= 0.0 && (n as usize) + 1 < self.values.len()) {
            return 0.0;
        }
        let i = n as usize;
        let f = x - n;
        self.values[i] + f * (self.values[i + 1] - self.values[i])
    }

    /// The interpolated value at `p` over the voxels inside the volume; `None` where the weights
    /// sum to about 0.
    fn value(&self, vol: &Volume, p: [f32; 3]) -> Option<f32> {
        let h = self.half;
        let base = p.map(|v| floor(v) as i64);
        let w = (2 * h + 1) as usize;
        let mut k = vec![[0.0f32; 3]; w];
        for (d, row) in k.iter_mut().enumerate() {
            for a in 0..3 {
                row[a] = self.at(p[a] - (base[a] + d as i64 - h) as f32);
            }
        }
        let [nx, ny, _] = vol.shape;
        let range = |a: usize| (base[a] - h).max(0)..=(base[a] + h).min(vol.shape[a] as i64 - 1);
        let (mut sum, mut wsum) = (0.0f32, 0.0f32);
        for z in range(2) {
            let wz = k[(z - base[2] + h) as usize][2];
            for y in range(1) {
                let wy = k[(y - base[1] + h) as usize][1];
                let row = &vol.data[nx * (y as usize + ny * z as usize)..][..nx];
                for x in range(0) {
                    let wt = k[(x - base[0] + h) as usize][0] * wy * wz;
                    sum += wt * row[x as usize];
                    wsum += wt;
                }
            }
        }
        (wsum.abs() > 1e-9).then(|| sum / wsum)
    }
}

/// The blur of the input before its resampling (§9.2): §5's blur to `r` mm, the values beyond
/// the image's edges read by the extraslice rule (one voxel beyond: the edge voxel; further:
/// `pad`).
pub fn blur_for_output(v: &Volume, r: f32, pad: f32) -> Volume {
    let mut data = v.data.clone();
    let [nx, ny, _] = v.shape;
    for axis in 0..3 {
        let Some(taps) = pyramid::blur_kernel(r, v.voxel_size[axis]) else {
            continue;
        };
        let n = v.shape[axis] as i64;
        let stride = [1, nx, nx * ny][axis];
        let half = ((taps.len() - 1) / 2) as i64;
        let src = data;
        let plane = nx * ny;
        let mut out = vec![0.0f32; src.len()];
        out.par_chunks_mut(plane.max(1))
            .enumerate()
            .for_each(|(z, slab)| {
                for (k, o) in slab.iter_mut().enumerate() {
                    let idx = z * plane + k;
                    let pos = [k % nx, k / nx, z][axis] as i64;
                    let base = idx as i64 - pos * stride as i64;
                    let mut acc = 0.0f32;
                    for (t, &w) in taps.iter().enumerate() {
                        let j = pos + t as i64 - half;
                        let v = if j >= 0 && j < n {
                            src[(base + j * stride as i64) as usize]
                        } else if j == -1 {
                            src[base as usize]
                        } else if j == n {
                            src[(base + (n - 1) * stride as i64) as usize]
                        } else {
                            pad
                        };
                        acc = (f64::from(acc) + f64::from(v) * w) as f32;
                    }
                    *o = acc;
                }
            });
        data = out;
    }
    Volume::new(v.shape, v.voxel_size, data)
}

/// The voxel-to-voxel map of the output grid (§9.2): `D_in⁻¹ · M⁻¹ · D_out`, rounded to
/// float32; `None` for a singular matrix.
pub fn output_map(m: &Mat4, in_voxel: [f32; 3], out_voxel: [f32; 3]) -> Option<[[f32; 4]; 3]> {
    let inv = mat::inverse(m)?;
    let din = mat::diagonal(in_voxel.map(|d| 1.0 / f64::from(d)));
    let dout = mat::diagonal(out_voxel.map(f64::from));
    let a = mat::mul(&mat::mul(&din, &inv), &dout);
    Some([0, 1, 2].map(|i| [0, 1, 2, 3].map(|j| a[i][j] as f32)))
}

/// How the output is interpolated and padded.
pub struct Resampling<'a> {
    pub interp: Interp,
    pub sinc: Option<&'a SincKernel>,
    /// The value outside the input (the background value or `-setbackground`).
    pub pad: f32,
    /// `-paddingsize` (voxels).
    pub padding: f32,
}

/// The input `src` resampled by `m` (input mm → output mm) onto a grid of `shape` and
/// `voxel` (§9.2). `None` for a singular matrix.
pub fn resample(
    src: &Volume,
    m: &Mat4,
    shape: [usize; 3],
    voxel: [f32; 3],
    r: &Resampling<'_>,
) -> Option<Vec<f32>> {
    let a = output_map(m, src.voxel_size, voxel)?;
    let [nx, ny, nz] = shape;
    let n = src.shape.map(|v| v as f32);
    let spline = (r.interp == Interp::Spline).then(|| Spline::new(src));
    let mut data = vec![r.pad; nx * ny * nz];
    let plane = nx * ny;
    data.par_chunks_mut(plane.max(1))
        .enumerate()
        .for_each(|(z, slab)| {
            let zf = z as f32;
            for x in 0..nx {
                let xf = x as f32;
                let mut p = [0, 1, 2].map(|k| xf * a[k][0] + zf * a[k][2] + a[k][3]);
                for y in 0..ny {
                    let pad_out = (0..3).any(|k| p[k] < -r.padding || p[k] > n[k] - 1.0 + r.padding);
                    if !pad_out {
                        let extended = (0..3).all(|k| p[k] >= -1.0 && p[k] <= n[k]);
                        let v = if !extended {
                            r.pad
                        } else {
                            match r.interp {
                                Interp::Trilinear => trilinear_extended(src, p[0], p[1], p[2]),
                                Interp::NearestNeighbour => nearest(src, p),
                                Interp::Sinc => r
                                    .sinc
                                    .and_then(|k| k.value(src, p))
                                    .unwrap_or(r.pad),
                                Interp::Spline => spline
                                    .as_ref()
                                    .map_or(r.pad, |s| s.value(p, r.pad)),
                            }
                        };
                        slab[x + nx * y] = v;
                    }
                    p = [p[0] + a[0][1], p[1] + a[1][1], p[2] + a[2][1]];
                }
            }
        });
    Some(data)
}
