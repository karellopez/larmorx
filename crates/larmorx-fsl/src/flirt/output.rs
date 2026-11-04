// SPDX-License-Identifier: Apache-2.0
//! The resampled image (`specs/flirt.md` §9.2, §9.3): the input, blurred to the reference's
//! smallest voxel size, interpolated at the pre-image of every reference voxel (the shared
//! engine's interpolators), padded outside the input.
//!
//! Output voxels are independent: slices run in parallel and each voxel's position is stepped
//! along its row as the original does, so the image does not depend on the thread count.

use std::f64::consts::PI;

use larmorx_core::math::Libm;
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
    /// The table (§9.2): full width `round(width)`, half-width `h = (w − 1)/2`, 1201 samples at
    /// `x = ((k − m − 1)/m)·h` (single, `m = 600`), each the single product of the sinc (`sinf(y)/y`,
    /// `y = π·x` rounded to single; `1 − |x|` below 1e-7) and the window (double, rounded).
    /// The sine and the window's cosines are `libm`'s (glibc's `sinf` and `cos` with
    /// `reproduce`, GLIBC-07).
    pub fn new(width: f32, window: SincWindow, libm: &Libm) -> SincKernel {
        let w = (width.round() as i64).max(1);
        let half = ((w - 1) / 2).max(1);
        let h = half as f32;
        let hd = f64::from(h);
        let m = 600.0f32;
        let values = (1..=1201)
            .map(|k| {
                let x = ((k as f32 - m - 1.0) / m) * h;
                let sinc = if x.abs() < 1e-7 {
                    1.0 - x.abs()
                } else {
                    let y = (PI * f64::from(x)) as f32;
                    libm.sinf(y) / y
                };
                let xd = f64::from(x);
                let win = if x.abs() > h {
                    0.0
                } else {
                    (match window {
                        SincWindow::Rectangular => 1.0,
                        SincWindow::Hanning => 0.5 + 0.5 * libm.cos((PI * xd) / hd),
                        SincWindow::Blackman => {
                            0.42 + 0.5 * libm.cos((PI * xd) / hd)
                                + 0.08 * libm.cos((2.0 * PI * xd) / hd)
                        }
                    }) as f32
                };
                sinc * win
            })
            .collect();
        SincKernel { half, values }
    }

    /// The table read at `t` (§9.2): `q = (t/h)·m + m` (single) plus 1 in double, rounded; the
    /// samples `n` and `n + 1` blended in double.
    #[inline]
    fn at(&self, t: f32) -> f32 {
        let h = self.half as f32;
        if t.abs() > h {
            return 0.0;
        }
        let m = 600.0f32;
        let q = (f64::from((t / h) * m + m) + 1.0) as f32;
        let n = floor(q);
        let f = q - n;
        if !(1.0..=1200.0).contains(&n) {
            return 0.0;
        }
        let i = n as usize - 1;
        let (a, b) = (f64::from(self.values[i]), f64::from(self.values[i + 1]));
        (a * (1.0 - f64::from(f)) + b * f64::from(f)) as f32
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
                // (p − i₀) − e, in single precision
                row[a] = self.at((p[a] - base[a] as f32) - (d as i64 - h) as f32);
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
pub fn blur_for_output(v: &Volume, r: f32, pad: f32, libm: &Libm) -> Volume {
    blur_with_edge(v, r, Edge::Extraslice(pad), libm)
}

/// What a blur reads beyond the image (§9.2, §9.3).
#[derive(Clone, Copy, Debug)]
pub enum Edge {
    /// Zeros (after a registration: the blur comes first).
    Zero,
    /// The extraslice rule: the edge voxel one voxel out, the value further.
    Extraslice(f32),
    /// `-setbackground`'s constant.
    Constant(f32),
}

/// The blur of `v` to `r` mm (§5) reading beyond the image by `edge`.
pub fn blur_with_edge(v: &Volume, r: f32, edge: Edge, libm: &Libm) -> Volume {
    let mut data = v.data.clone();
    let [nx, ny, _] = v.shape;
    for axis in 0..3 {
        let Some(taps) = pyramid::blur_kernel(r, v.voxel_size[axis], libm) else {
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
                        } else {
                            match edge {
                                Edge::Zero => 0.0,
                                Edge::Constant(c) => c,
                                Edge::Extraslice(pad) => {
                                    if j == -1 {
                                        src[base as usize]
                                    } else if j == n {
                                        src[(base + (n - 1) * stride as i64) as usize]
                                    } else {
                                        pad
                                    }
                                }
                            }
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

/// Trilinear interpolation by the extraslice rule (§9.2): a neighbour one voxel beyond an edge
/// reads the edge voxel, a neighbour further out reads `pad`.
pub fn trilinear_extraslice(vol: &Volume, p: [f32; 3], pad: f32) -> f32 {
    let [nx, ny, nz] = vol.shape.map(|n| n as i64);
    let f = p.map(floor);
    let (ix, iy, iz) = (f[0] as i64, f[1] as i64, f[2] as i64);
    let (fx, fy, fz) = (p[0] - f[0], p[1] - f[1], p[2] - f[2]);
    let at = |x: i64, y: i64, z: i64| -> f32 {
        let c = |v: i64, n: i64| -> Option<i64> {
            if v >= 0 && v < n {
                Some(v)
            } else if v == -1 {
                Some(0)
            } else if v == n {
                Some(n - 1)
            } else {
                None
            }
        };
        match (c(x, nx), c(y, ny), c(z, nz)) {
            (Some(a), Some(b), Some(cz)) => vol.data[(a + nx * (b + ny * cz)) as usize],
            _ => pad,
        }
    };
    let v = [0i64, 1].map(|c| [0i64, 1].map(|b| [0i64, 1].map(|a| at(ix + a, iy + b, iz + c))));
    larmorx_mri::hmc::interp::blend(v, fx, fy, fz)
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
    /// The spline's coefficients when the caller computed them (`applyxfm4D`'s, §15.7), else
    /// the shared engine's of the input.
    pub spline: Option<&'a Spline>,
    /// `-setbackground`: neighbours outside the input read `pad` (not the extraslice rule).
    pub constant: bool,
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
    let own = (r.interp == Interp::Spline && r.spline.is_none()).then(|| FlirtSpline::new(src));
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
                    let pad_out =
                        (0..3).any(|k| p[k] < -r.padding || p[k] > n[k] - 1.0 + r.padding);
                    if !pad_out {
                        let extended = (0..3).all(|k| p[k] >= -1.0 && p[k] <= n[k]);
                        let v = if r.interp == Interp::Spline {
                            match (r.spline, own.as_ref()) {
                                (Some(s), _) => {
                                    if extended {
                                        s.value(p, r.pad)
                                    } else {
                                        r.pad
                                    }
                                }
                                (None, Some(s)) => s.value(p, r.pad, r.constant),
                                (None, None) => r.pad,
                            }
                        } else if r.constant && r.interp == Interp::Trilinear {
                            trilinear_constant(src, p, r.pad)
                        } else if r.interp == Interp::NearestNeighbour && !r.constant {
                            // the nearest index first, then the extraslice rule on it: one
                            // voxel beyond an edge reads the edge voxel (observed, 2026-11-02:
                            // positions 1 to 1.5 voxels outside take the edge's value)
                            let beyond = (0..3).any(|k| {
                                let i = p[k].round();
                                i < -1.0 || i > n[k]
                            });
                            if beyond { r.pad } else { nearest(src, p) }
                        } else if !extended {
                            match r.interp {
                                Interp::Trilinear => trilinear_extraslice(src, p, r.pad),
                                _ => r.pad,
                            }
                        } else {
                            match r.interp {
                                Interp::Trilinear => trilinear_extended(src, p[0], p[1], p[2]),
                                Interp::NearestNeighbour => nearest(src, p),
                                Interp::Sinc => {
                                    r.sinc.and_then(|k| k.value(src, p)).unwrap_or(r.pad)
                                }
                                Interp::Spline => r.pad,
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

/// Trilinear interpolation with every neighbour outside the volume reading `c`
/// (`-setbackground`, §9.2).
fn trilinear_constant(vol: &Volume, p: [f32; 3], c: f32) -> f32 {
    let [nx, ny, nz] = vol.shape.map(|n| n as i64);
    let f = p.map(floor);
    let (ix, iy, iz) = (f[0] as i64, f[1] as i64, f[2] as i64);
    let (fx, fy, fz) = (p[0] - f[0], p[1] - f[1], p[2] - f[2]);
    let at = |x: i64, y: i64, z: i64| -> f32 {
        if x >= 0 && x < nx && y >= 0 && y < ny && z >= 0 && z < nz {
            vol.data[(x + nx * (y + ny * z)) as usize]
        } else {
            c
        }
    };
    let v = [0i64, 1].map(|cz| [0i64, 1].map(|b| [0i64, 1].map(|a| at(ix + a, iy + b, iz + cz))));
    larmorx_mri::hmc::interp::blend(v, fx, fy, fz)
}

/// FLIRT's cubic spline (§9.3, exactly): the coefficients along x, then y, then z (axes of
/// one voxel skipped), each column in double, stored in single after each axis; at a position the
/// four weights per axis from `⌈p⌉ − 2`, the value `Σ (c·w_x)·(w_z·w_y)` in double, z outer.
pub struct FlirtSpline {
    shape: [usize; 3],
    coef: Vec<f32>,
}

impl FlirtSpline {
    pub fn new(vol: &Volume) -> FlirtSpline {
        let [nx, ny, nz] = vol.shape;
        let mut c = vol.data.clone();
        let strides = [1, nx, nx * ny];
        for axis in 0..3 {
            let len = vol.shape[axis];
            if len < 2 {
                continue;
            }
            let others: Vec<usize> = (0..nx * ny * nz)
                .filter(|&i| (i / strides[axis]) % len == 0)
                .collect();
            let st = strides[axis];
            let cols: Vec<Vec<f32>> = others
                .par_iter()
                .map(|&start| {
                    let mut col: Vec<f64> =
                        (0..len).map(|k| f64::from(c[start + k * st])).collect();
                    spline_column(&mut col);
                    col.into_iter().map(|v| v as f32).collect()
                })
                .collect();
            for (&start, col) in others.iter().zip(cols) {
                for (k, v) in col.into_iter().enumerate() {
                    c[start + k * st] = v;
                }
            }
        }
        FlirtSpline {
            shape: vol.shape,
            coef: c,
        }
    }

    /// The value at `p`; with `constant` (`-setbackground`) coefficients outside the grid count
    /// 0 and a position outside `[0, n − 1]` gives `pad`; otherwise the edge coefficients repeat
    /// and `pad` where `⌊p⌋ < −1` or `⌊p⌋ ≥ n`.
    pub fn value(&self, p: [f32; 3], pad: f32, constant: bool) -> f32 {
        let n = self.shape.map(|v| v as i64);
        if constant {
            if (0..3).any(|k| !(p[k] >= 0.0 && p[k] <= (n[k] - 1) as f32)) {
                return pad;
            }
        } else if (0..3).any(|k| {
            let b = floor(p[k]) as i64;
            b < -1 || b >= n[k]
        }) {
            return pad;
        }
        let start = p.map(|v| v.ceil() as i64 - 2);
        let w = [0, 1, 2].map(|k| {
            [0, 1, 2, 3].map(|i| {
                let a = (f64::from(p[k]) - (start[k] + i as i64) as f64).abs();
                if a < 1.0 {
                    2.0 / 3.0 + ((0.5 * a) * a) * (a - 2.0)
                } else if a < 2.0 {
                    let b = 2.0 - a;
                    (1.0 / 6.0) * ((b * b) * b)
                } else {
                    0.0
                }
            })
        });
        let at = |x: i64, y: i64, z: i64| -> f64 {
            let inside = |v: i64, k: usize| v >= 0 && v < n[k];
            if constant && !(inside(x, 0) && inside(y, 1) && inside(z, 2)) {
                return 0.0;
            }
            let (x, y, z) = (
                x.clamp(0, n[0] - 1),
                y.clamp(0, n[1] - 1),
                z.clamp(0, n[2] - 1),
            );
            f64::from(self.coef[(x + n[0] * (y + n[1] * z)) as usize])
        };
        let mut sum = 0.0f64;
        for (dz, &wz) in w[2].iter().enumerate() {
            for (dy, &wy) in w[1].iter().enumerate() {
                for (dx, &wx) in w[0].iter().enumerate() {
                    let c = at(
                        start[0] + dx as i64,
                        start[1] + dy as i64,
                        start[2] + dz as i64,
                    );
                    sum += (c * wx) * (wz * wy);
                }
            }
        }
        sum as f32
    }
}

/// One column's cubic B-spline coefficients (§9.3): pole `√3 − 2`, the causal start over
/// `min(15, len)` terms (powers by repeated products), the causal sweep, the anti-causal start
/// from the column's last value before the sweep, the anti-causal sweep, every coefficient
/// times 6.
fn spline_column(c: &mut [f64]) {
    let len = c.len();
    let z = 3.0f64.sqrt() - 2.0;
    let horizon = ((1e-8f64.ln() / z.abs().ln() + 1.5) as usize).min(len);
    let last = c[len - 1];
    let mut zi = z;
    let mut sum = c[0];
    for &v in &c[1..horizon] {
        sum += zi * v;
        zi *= z;
    }
    c[0] = sum;
    for i in 1..len {
        c[i] += z * c[i - 1];
    }
    c[len - 1] = (-z / (1.0 - z * z)) * (2.0 * c[len - 1] - last);
    for i in (0..len - 1).rev() {
        c[i] = z * (c[i + 1] - c[i]);
    }
    for v in c.iter_mut() {
        *v *= 6.0;
    }
}

/// Which output voxels [`resample`] sets to the padding value (their pre-image more than
/// `padding` voxels outside an input of `src_shape`): the same positions, stepped as there.
pub fn padded(
    src_shape: [usize; 3],
    src_voxel: [f32; 3],
    m: &Mat4,
    shape: [usize; 3],
    voxel: [f32; 3],
    padding: f32,
) -> Option<Vec<bool>> {
    let a = output_map(m, src_voxel, voxel)?;
    let [nx, ny, nz] = shape;
    let n = src_shape.map(|v| v as f32);
    let mut out = vec![false; nx * ny * nz];
    let plane = nx * ny;
    out.par_chunks_mut(plane.max(1))
        .enumerate()
        .for_each(|(z, slab)| {
            let zf = z as f32;
            for x in 0..nx {
                let xf = x as f32;
                let mut p = [0, 1, 2].map(|k| xf * a[k][0] + zf * a[k][2] + a[k][3]);
                for y in 0..ny {
                    slab[x + nx * y] =
                        (0..3).any(|k| p[k] < -padding || p[k] > n[k] - 1.0 + padding);
                    p = [p[0] + a[0][1], p[1] + a[1][1], p[2] + a[2][1]];
                }
            }
        });
    Some(out)
}

/// The input `src` resampled through the boundary-based cost's mapping with a field map
/// (§6.5, §9.2): each output voxel `o` reads the input at `T·(D_out·o)` with `T = D_in⁻¹·M⁻¹`,
/// shifted along the phase-encoding axis by `shift(o)`, by the extraslice rule.
pub fn resample_shifted(
    src: &Volume,
    m: &Mat4,
    shape: [usize; 3],
    voxel: [f32; 3],
    pad: f32,
    shift: &(dyn Fn([f64; 3]) -> (usize, f64) + Sync),
) -> Option<Vec<f32>> {
    let inv = mat::inverse(m)?;
    let din = mat::diagonal(src.voxel_size.map(|d| 1.0 / f64::from(d)));
    let t = mat::mul(&din, &inv);
    let [nx, ny, nz] = shape;
    let mut data = vec![pad; nx * ny * nz];
    let plane = nx * ny;
    data.par_chunks_mut(plane.max(1))
        .enumerate()
        .for_each(|(z, slab)| {
            for y in 0..ny {
                for x in 0..nx {
                    let o = [x as f64, y as f64, z as f64];
                    let pmm = [
                        o[0] * f64::from(voxel[0]),
                        o[1] * f64::from(voxel[1]),
                        o[2] * f64::from(voxel[2]),
                        1.0,
                    ];
                    let mut q = mat::apply(&t, pmm);
                    let (axis, d) = shift(o);
                    q[axis] += d;
                    // by the extraslice rule, no padding test (observed)
                    slab[x + nx * y] =
                        trilinear_extraslice(src, [q[0] as f32, q[1] as f32, q[2] as f32], pad);
                }
            }
        });
    Some(data)
}
