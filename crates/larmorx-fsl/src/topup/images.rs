// SPDX-License-Identifier: Apache-2.0
//! The images topup estimates from (`specs/topup.md` §4.1, §4.4, §4.5): the intensity scaling,
//! the regridding, the subsampling and smoothing of each level, and the cubic B-spline
//! interpolation of the result.
//!
//! The interpolation is periodic along every axis (observed: FSL's corrected images at the first
//! cost evaluation, `--debug`, agree with a periodic spline to single precision, and not with a
//! mirrored or clamped one along the axes other than the phase encoding;
//! `docs/findings/fsl-topup-original.md`).

use rayon::prelude::*;

use super::spline::{Op1, apply3, b3, b3d1};

/// A volume of `f64` values, x fastest.
#[derive(Clone, Debug)]
pub struct Grid {
    pub dims: [usize; 3],
    pub data: Vec<f64>,
    /// Samples for trilinear interpolation (`--interp=linear`), else cubic spline coefficients.
    pub linear: bool,
}

impl Grid {
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Solves the periodic system `(c[i-1] + 4 c[i] + c[i+1]) / 6 = s[i]` in place (the cubic
/// B-spline coefficients of periodic samples).
pub fn periodic_prefilter_line(s: &mut [f64], work: &mut Vec<f64>) {
    let n = s.len();
    match n {
        0 | 1 => {}
        2 => {
            // (4 c0 + 2 c1) / 6 = s0, (2 c0 + 4 c1) / 6 = s1
            let (a, b) = (s[0], s[1]);
            s[0] = (4.0 * a - 2.0 * b) * 0.5;
            s[1] = (4.0 * b - 2.0 * a) * 0.5;
        }
        _ => {
            // the cyclic tridiagonal system by Sherman-Morrison on a Thomas solve
            let (a, b, c) = (1.0 / 6.0, 4.0 / 6.0, 1.0 / 6.0);
            let gamma = -b;
            work.clear();
            work.resize(3 * n, 0.0);
            let (cp, rest) = work.split_at_mut(n);
            let (x, z) = rest.split_at_mut(n);
            let diag = |i: usize| {
                if i == 0 {
                    b - gamma
                } else if i == n - 1 {
                    b - a * c / gamma
                } else {
                    b
                }
            };
            let thomas = |rhs: &dyn Fn(usize) -> f64, out: &mut [f64], cp: &mut [f64]| {
                let mut m = diag(0);
                cp[0] = c / m;
                out[0] = rhs(0) / m;
                for i in 1..n {
                    m = diag(i) - a * cp[i - 1];
                    cp[i] = c / m;
                    out[i] = (rhs(i) - a * out[i - 1]) / m;
                }
                for i in (0..n - 1).rev() {
                    out[i] -= cp[i] * out[i + 1];
                }
            };
            let sv: &[f64] = s;
            thomas(&|i| sv[i], x, cp);
            thomas(
                &|i| {
                    if i == 0 {
                        gamma
                    } else if i == n - 1 {
                        c
                    } else {
                        0.0
                    }
                },
                z,
                cp,
            );
            let fact = (x[0] + a * x[n - 1] / gamma) / (1.0 + z[0] + a * z[n - 1] / gamma);
            for i in 0..n {
                s[i] = x[i] - fact * z[i];
            }
        }
    }
}

/// The periodic cubic B-spline coefficients of a volume.
pub fn prefilter(g: &Grid) -> Grid {
    let [nx, ny, nz] = g.dims;
    let mut d = g.data.clone();
    // x lines
    d.par_chunks_mut(nx).for_each(|line| {
        let mut w = Vec::new();
        periodic_prefilter_line(line, &mut w);
    });
    // y lines, per slice
    d.par_chunks_mut(nx * ny).for_each(|slice| {
        let mut w = Vec::new();
        let mut line = vec![0.0; ny];
        for x in 0..nx {
            for y in 0..ny {
                line[y] = slice[y * nx + x];
            }
            periodic_prefilter_line(&mut line, &mut w);
            for y in 0..ny {
                slice[y * nx + x] = line[y];
            }
        }
    });
    // z lines: gather per (x, y) column
    if nz > 1 {
        let plane = nx * ny;
        let cols: Vec<Vec<f64>> = (0..plane)
            .into_par_iter()
            .map(|i| {
                let mut w = Vec::new();
                let mut line: Vec<f64> = (0..nz).map(|z| d[z * plane + i]).collect();
                periodic_prefilter_line(&mut line, &mut w);
                line
            })
            .collect();
        for (i, col) in cols.iter().enumerate() {
            for (z, &v) in col.iter().enumerate() {
                d[z * plane + i] = v;
            }
        }
    }
    Grid {
        dims: g.dims,
        data: d,
        linear: false,
    }
}

/// The periodic cubic B-spline interpolation weights at `x` (voxels of an axis of `n`): the
/// four indices (wrapped) and the values (`deriv`: the first derivative's).
#[inline]
pub fn taps(x: f64, n: usize, deriv: bool) -> ([usize; 4], [f64; 4]) {
    taps_of(x, n, deriv, false)
}

/// [`taps`] for the cubic spline, or (`linear`) for trilinear interpolation: the weights `1 - t`
/// and `t` of the two neighbours in the middle slots (the derivative `-1`, `1`).
#[inline]
pub fn taps_of(x: f64, n: usize, deriv: bool, linear: bool) -> ([usize; 4], [f64; 4]) {
    let (idx, w, d) = taps_both(x, n, linear);
    (idx, if deriv { d } else { w })
}

/// The four indices of [`taps_of`] with both the values' and the first derivative's weights,
/// computed once (the same arithmetic: the indices `(floor(x) - 1 + j) mod n`, the weights of
/// `t - (j - 1)`).
#[inline]
pub fn taps_both(x: f64, n: usize, linear: bool) -> ([usize; 4], [f64; 4], [f64; 4]) {
    let f = x.floor();
    let t = x - f;
    let i0 = (f as i64).wrapping_sub(1);
    let nn = n as i64;
    let idx = if i0 >= 0 && i0 <= nn - 4 {
        let b = i0 as usize;
        [b, b + 1, b + 2, b + 3]
    } else {
        std::array::from_fn(|j| i0.wrapping_add(j as i64).rem_euclid(nn) as usize)
    };
    let mut w = [0.0; 4];
    let mut d = [0.0; 4];
    if linear {
        w[1] = 1.0 - t;
        w[2] = t;
        d[1] = -1.0;
        d[2] = 1.0;
    } else {
        for j in 0..4 {
            let u = t - (j as f64 - 1.0);
            w[j] = b3(u);
            d[j] = b3d1(u);
        }
    }
    (idx, w, d)
}

/// The operator sampling one axis' periodic spline at `xs`.
pub fn sample_op(xs: &[f64], n: usize, deriv: bool) -> Op1 {
    sample_op_of(xs, n, deriv, false)
}

/// [`sample_op`] for the cubic spline or (`linear`) trilinear interpolation.
pub fn sample_op_of(xs: &[f64], n: usize, deriv: bool, linear: bool) -> Op1 {
    let mut op = Op1::new(xs.len(), n, 4);
    for (r, &x) in xs.iter().enumerate() {
        let (idx, w) = taps_of(x, n, deriv, linear);
        // indices may repeat when n < 4: accumulate into distinct slots
        for j in 0..4 {
            op.set(r, j, idx[j], w[j]);
        }
    }
    op
}

/// The Gaussian smoothing of a volume (§4.4): FWHM `fwhm` mm, voxel sizes `voxel`, separable,
/// periodic.
/// The taps' `exp` is glibc's with `glibc` (`reproduce`, GLIBC-07); the second value says
/// whether the other choice gives other taps (single precision, as stored).
pub fn smooth(g: &Grid, fwhm: f64, voxel: [f64; 3], glibc: bool) -> (Grid, bool) {
    // FWHM 0 is no smoothing; so is a negative one (FSL-125 reproduced; observed: FSL's first
    // cost with --fwhm=-8 is that of no smoothing, probe P)
    if !(fwhm > 0.0) || !fwhm.is_finite() {
        return (g.clone(), false);
    }
    let sigma = fwhm / (8.0 * std::f64::consts::LN_2).sqrt();
    let mut ops: [Option<Op1>; 3] = [None, None, None];
    let mut differs = false;
    for a in 0..3 {
        let n = g.dims[a];
        let sv = sigma / voxel[a];
        let r = 2 * ((sv - 0.001).trunc() as i64) + 3;
        let r = r.max(0);
        let tap = |j: i64, glibc: bool| {
            let q = -((j * j) as f64) / (2.0 * sv * sv);
            let v = if glibc {
                larmorx_core::math::exp_glibc(q)
            } else {
                larmorx_core::math::exp(q)
            };
            f64::from(v as f32)
        };
        let mut k: Vec<f64> = (-r..=r).map(|j| tap(j, glibc)).collect();
        differs |= (-r..=r)
            .zip(&k)
            .any(|(j, v)| tap(j, !glibc).to_bits() != v.to_bits());
        let s: f64 = k.iter().sum();
        for v in &mut k {
            *v /= s;
        }
        let width = k.len();
        let mut op = Op1::new(n, n, width);
        for i in 0..n {
            for (j, &kv) in k.iter().enumerate() {
                let src = (i as i64 + j as i64 - r).rem_euclid(n as i64) as usize;
                op.set(i, j, src, kv);
            }
        }
        ops[a] = Some(op);
    }
    let (data, dims) = apply3(
        &g.data,
        g.dims,
        [ops[0].as_ref(), ops[1].as_ref(), ops[2].as_ref()],
    );
    (
        Grid {
            dims,
            data,
            linear: g.linear,
        },
        differs,
    )
}

/// The `s × s × s` block means (sizes divisible by `s`).
pub fn subsample(g: &Grid, s: usize) -> Grid {
    if s <= 1 {
        return g.clone();
    }
    let [nx, ny, nz] = g.dims;
    let o = [nx / s, ny / s, nz / s];
    let inv = 1.0 / (s * s * s) as f64;
    let mut out = vec![0.0; o[0] * o[1] * o[2]];
    out.par_chunks_mut(o[0] * o[1])
        .enumerate()
        .for_each(|(z, slab)| {
            for y in 0..o[1] {
                for x in 0..o[0] {
                    let mut sum = 0.0;
                    for dz in 0..s {
                        for dy in 0..s {
                            let base = ((z * s + dz) * ny + y * s + dy) * nx + x * s;
                            for dx in 0..s {
                                sum += g.data[base + dx];
                            }
                        }
                    }
                    slab[y * o[0] + x] = sum * inv;
                }
            }
        });
    Grid {
        dims: o,
        data: out,
        linear: g.linear,
    }
}

/// The volumes after the intensity scaling and the regridding (§4.1, §4.4), once per run.
#[derive(Clone, Debug)]
pub struct Prepared {
    /// The input grid and voxel sizes (mm).
    pub dims: [usize; 3],
    pub voxel: [f64; 3],
    /// The grid the levels subsample, and its voxel sizes.
    pub reg_dims: [usize; 3],
    pub reg_voxel: [f64; 3],
    pub volumes: Vec<Grid>,
    /// The mean of the volumes' means (the outputs' intensities are multiplied by `g / 100`).
    pub g: f64,
}

/// The mean of a volume (double sums).
pub fn mean(v: &[f32]) -> f64 {
    // fixed blocks summed in order: independent of the thread count
    let partial: Vec<f64> = v
        .par_chunks(1 << 16)
        .map(|c| c.iter().map(|&x| f64::from(x)).sum::<f64>())
        .collect();
    partial.iter().sum::<f64>() / v.len().max(1) as f64
}

/// Scales and regrids the volumes. `extra`: the voxels added per axis (the largest
/// subsampling), 0 without regridding.
pub fn prepare(
    vols: &[Vec<f32>],
    dims: [usize; 3],
    voxel: [f64; 3],
    each: bool,
    extra: Option<usize>,
    linear: bool,
) -> Prepared {
    let means: Vec<f64> = vols.iter().map(|v| mean(v)).collect();
    let g = means.iter().sum::<f64>() / means.len().max(1) as f64;
    let (reg_dims, reg_voxel) = match extra {
        Some(e) => {
            let mut rd = dims;
            let mut rv = voxel;
            for a in 0..3 {
                rd[a] = dims[a] + e;
                rv[a] = ((dims[a] as f64 - 1.0) * voxel[a] - 1e-6) / ((rd[a] - 1).max(1) as f64);
                if dims[a] == 1 {
                    rv[a] = voxel[a];
                }
            }
            (rd, rv)
        }
        None => (dims, voxel),
    };
    let volumes = vols
        .iter()
        .zip(&means)
        .map(|(v, &m)| {
            let f = 100.0 / if each { m } else { g };
            let grid = Grid {
                dims,
                data: v.iter().map(|&x| f64::from(x) * f).collect(),
                linear,
            };
            match extra {
                Some(_) => {
                    // the regridding interpolates as the levels do (§4.4)
                    let c = if linear {
                        grid.clone()
                    } else {
                        prefilter(&grid)
                    };
                    let ops: Vec<Op1> = (0..3)
                        .map(|a| {
                            let xs: Vec<f64> = (0..reg_dims[a])
                                .map(|j| j as f64 * (reg_voxel[a] / voxel[a]))
                                .collect();
                            sample_op_of(&xs, dims[a], false, linear)
                        })
                        .collect();
                    let (data, d) = apply3(
                        &c.data,
                        c.dims,
                        [Some(&ops[0]), Some(&ops[1]), Some(&ops[2])],
                    );
                    Grid {
                        dims: d,
                        data,
                        linear,
                    }
                }
                None => grid,
            }
        })
        .collect();
    Prepared {
        dims,
        voxel,
        reg_dims,
        reg_voxel,
        volumes,
        g,
    }
}

/// One level's interpolated volumes: each volume subsampled, smoothed, as spline coefficients.
#[derive(Clone, Debug)]
pub struct LevelImages {
    pub dims: [usize; 3],
    pub voxel: [f64; 3],
    pub coef: Vec<Grid>,
    /// GLIBC-07: the other choice of `exp` gives other smoothing taps.
    pub glibc_differs: bool,
}

/// The smoothing's `exp` is glibc's with `glibc` (`reproduce`).
pub fn level_images(p: &Prepared, s: usize, fwhm: f64, glibc: bool) -> LevelImages {
    let voxel = [
        p.reg_voxel[0] * s as f64,
        p.reg_voxel[1] * s as f64,
        p.reg_voxel[2] * s as f64,
    ];
    let mut glibc_differs = false;
    let coef: Vec<Grid> = p
        .volumes
        .iter()
        .map(|v| {
            let (g, d) = smooth(&subsample(v, s), fwhm, voxel, glibc);
            glibc_differs |= d;
            if g.linear { g } else { prefilter(&g) }
        })
        .collect();
    LevelImages {
        dims: coef[0].dims,
        voxel,
        coef,
        glibc_differs,
    }
}

/// The value and the first derivatives (per voxel of the coefficient grid) of a periodic cubic
/// spline at `p`; `want[a]`: whether the derivative along axis `a` is needed.
#[inline]
pub fn interp(c: &Grid, p: [f64; 3], want: [bool; 3]) -> (f64, [f64; 3]) {
    let [nx, ny, nz] = c.dims;
    let l = c.linear;
    let (ix, wx, dx) = taps_both(p[0], nx, l);
    let (iy, wy, dy) = taps_both(p[1], ny, l);
    let (iz, wz, dz) = taps_both(p[2], nz, l);
    let dx = if want[0] { dx } else { [0.0; 4] };
    let dy = if want[1] { dy } else { [0.0; 4] };
    let dz = if want[2] { dz } else { [0.0; 4] };
    let mut v = 0.0;
    let mut d = [0.0; 3];
    let (lo, hi) = if l { (1, 3) } else { (0, 4) };
    for c3 in lo..hi {
        let zoff = iz[c3] * nx * ny;
        for b in lo..hi {
            let yoff = zoff + iy[b] * nx;
            let mut sx = 0.0;
            let mut sdx = 0.0;
            for a in lo..hi {
                let val = c.data[yoff + ix[a]];
                sx += wx[a] * val;
                sdx += dx[a] * val;
            }
            v += wz[c3] * wy[b] * sx;
            d[0] += wz[c3] * wy[b] * sdx;
            d[1] += wz[c3] * dy[b] * sx;
            d[2] += dz[c3] * wy[b] * sx;
        }
    }
    (v, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefilter_inverts_the_spline() {
        for n in [1usize, 2, 3, 5, 8] {
            let s: Vec<f64> = (0..n).map(|i| ((i * 7 + 3) % 5) as f64).collect();
            let mut c = s.clone();
            let mut w = Vec::new();
            periodic_prefilter_line(&mut c, &mut w);
            for i in 0..n {
                let v = (c[(i + n - 1) % n] + 4.0 * c[i] + c[(i + 1) % n]) / 6.0;
                assert!((v - s[i]).abs() < 1e-12, "n {n} i {i}");
            }
        }
    }
}
