// SPDX-License-Identifier: Apache-2.0
//! The bias field's smoothing (`specs/fast.md` §4.5, §8.5): Gaussian kernels and the separable
//! convolution along x, then y, then z, the image zero outside. Each output voxel depends only
//! on its line, so lines and slices run in parallel with results independent of the thread
//! count.

use larmorx_core::math::{self, Libm};
use rayon::prelude::*;

/// A normalised Gaussian kernel of `sigma` voxels: radius `2 trunc(σ)`, taps
/// `exp(-j² / (2σ²))` (double, rounded to single) divided by their single-precision sum (as a
/// multiplication by the double inverse); `σ ≤ 1e-6` the unit impulse. The normalised taps stay
/// in double precision (observed: rounding them to single moves 0.05 % of a bias field's voxels
/// by one unit in the last place against FSL with correctly rounded maths; kept in double, the
/// field is bit-identical: `docs/findings/fsl-fast-original.md`).
/// The taps' `exp` is `libm`'s (glibc's with `reproduce`, GLIBC-07; the witness compares the
/// single-precision taps).
pub fn kernel(sigma: f32, libm: &Libm) -> Vec<f64> {
    if sigma <= 1e-6 {
        return vec![1.0];
    }
    let t = sigma.trunc();
    let r = if t >= 0.0 {
        (t as usize).saturating_mul(2)
    } else {
        0
    };
    let s = f64::from(sigma);
    let taps: Vec<f32> = (0..=2 * r)
        .map(|k| {
            let j = k as f64 - r as f64;
            let q = -(j * j) / (2.0 * s * s);
            libm.choose32(|| math::exp(q) as f32, || math::exp_glibc(q) as f32)
        })
        .collect();
    let total = taps.iter().fold(0f32, |a, &b| a + b);
    let inv = 1.0 / f64::from(total);
    taps.iter().map(|&v| f64::from(v) * inv).collect()
}

/// `acc[x] += tap · src[x]` as FSL's convolution adds (the product in double, the sum in single).
#[inline(always)]
fn add_scaled(acc: &mut [f32], src: &[f32], tap: f64) {
    for (a, &s) in acc.iter_mut().zip(src) {
        *a = (f64::from(*a) + tap * f64::from(s)) as f32;
    }
}

/// The range of outputs `[lo, hi)` of a line of `n` that tap `j` (radius `r`) reaches inside
/// the image: output `o` reads input `o + j - r`.
#[inline]
fn reach(n: usize, j: usize, r: usize) -> (usize, usize) {
    let lo = r.saturating_sub(j);
    let hi = (n + r).saturating_sub(j).min(n);
    (lo, hi.max(lo))
}

/// Smooths `v` (an image of `shape`, x fastest) with the kernels `k` along x, y and z.
pub fn smooth(v: Vec<f32>, shape: [usize; 3], k: [&[f64]; 3]) -> Vec<f32> {
    let [nx, ny, nz] = shape;
    if v.is_empty() {
        return v;
    }
    let (tx, ty, tz) = (k[0], k[1], k[2]);
    let (rx, ry, rz) = (tx.len() / 2, ty.len() / 2, tz.len() / 2);
    let slice = nx * ny;
    // x, then y, slice by slice (in place, with one slice of scratch each)
    let mut v = v;
    v.par_chunks_mut(slice).for_each(|s| {
        let mut out = vec![0f32; slice];
        for (line, o) in s.chunks(nx).zip(out.chunks_mut(nx)) {
            for (j, &t) in tx.iter().enumerate() {
                let (lo, hi) = reach(nx, j, rx);
                if lo < hi {
                    add_scaled(&mut o[lo..hi], &line[lo + j - rx..hi + j - rx], t);
                }
            }
        }
        s.fill(0.0);
        for y in 0..ny {
            let acc = &mut s[y * nx..(y + 1) * nx];
            for (j, &t) in ty.iter().enumerate() {
                let yi = y + j;
                if yi < ry || yi - ry >= ny {
                    continue;
                }
                let src = &out[(yi - ry) * nx..(yi - ry + 1) * nx];
                add_scaled(acc, src, t);
            }
        }
    });
    if nz == 1 && rz == 0 {
        // one tap along z: the identity (its value 1 after normalisation)
        let t = tz[0];
        if t == 1.0 {
            return v;
        }
    }
    // z: each output slice from the input slices
    let mut out = vec![0f32; v.len()];
    out.par_chunks_mut(slice).enumerate().for_each(|(z, acc)| {
        for (j, &t) in tz.iter().enumerate() {
            let zi = z + j;
            if zi < rz || zi - rz >= nz {
                continue;
            }
            add_scaled(acc, &v[(zi - rz) * slice..(zi - rz + 1) * slice], t);
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernels_and_smoothing() {
        let l = Libm::default();
        assert_eq!(kernel(0.0, &l), vec![1.0]);
        assert_eq!(kernel(0.5, &l), vec![1.0]);
        let k = kernel(10.2, &l);
        assert_eq!(k.len(), 41);
        let s: f64 = k.iter().sum();
        assert!((s - 1.0).abs() < 1e-6);
        // an impulse is spread into the kernels' outer product
        let shape = [9, 7, 5];
        let mut v = vec![0f32; 9 * 7 * 5];
        v[(2 * 7 + 3) * 9 + 4] = 1.0;
        let kx = kernel(1.0, &l);
        let out = smooth(v, shape, [&kx, &kx, &kx]);
        assert_eq!(kx.len(), 5);
        let at = |x: usize, y: usize, z: usize| out[(z * 7 + y) * 9 + x];
        assert!((f64::from(at(4, 3, 2)) - kx[2] * kx[2] * kx[2]).abs() < 1e-7);
        assert!((f64::from(at(6, 3, 2)) - kx[4] * kx[2] * kx[2]).abs() < 1e-7);
        assert_eq!(at(0, 3, 2), 0.0);
        let total: f32 = out.iter().sum();
        assert!((total - 1.0).abs() < 1e-5);
    }
}
