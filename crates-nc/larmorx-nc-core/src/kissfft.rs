// SPDX-License-Identifier: LicenseRef-FSL AND Apache-2.0 AND BSD-3-Clause
// Translated to Rust from Armadillo 14.2.1 as fsl-armawrap 0.7.0 bundles it (FSL 6.0.7.23):
// fft_engine_kissfft.hpp and op_fft_meat.hpp (Apache-2.0; Copyright 2008-2016 Conrad Sanderson,
// 2008-2016 National ICT Australia (NICTA)), which include portions of Kiss FFT, licensed under
// the following conditions:
//
// Copyright (c) 2003-2010 Mark Borgerding
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:
//
// * Redistributions of source code must retain the above copyright notice,
//   this list of conditions and the following disclaimer.
//
// * Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// * Neither the author nor the names of any contributors may be used to
//   endorse or promote products derived from this software without specific
//   prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO,
// THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
// PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE COPYRIGHT OWNER OR
// CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
// EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
// PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
// WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE
// OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE,
// EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// and from FSL 6.0.7.23's fsl-newimage 2601.0 (imfft.cc: fft,
// ifft; complexvolume.cc: operator*=; newimagefns.h: efficient_convolve's FFT path, insertpart,
// extractpart), Copyright (C) 1999-2018 University of Oxford. Rust translation Copyright 2026
// Karel Lopez Vilaret; modified from the originals. Part of larmorx-nc (FSL Licence terms; see
// LICENSE).
//! The FFT of FSL's NEWMAT layer (armawrap's `FFT`/`FFTI`: Armadillo's `fft`/`ifft` of a complex
//! column, its Kiss FFT engine: FSL's Armadillo is built without FFTW), newimage's 3D transforms
//! of `float` volumes (`fft3`, `ifft3`: each line's transform in `double`, stored back in `float`)
//! and efficient_convolve's FFT convolution.
//!
//! Complex products are libstdc++'s `std::complex<double>` products (`ac - bd`, `ad + bc`); the
//! twiddle factors `exp(i 2 pi n / N)` are glibc's `cexp`, `(cos, sin)` of the angle, computed
//! with correctly rounded `cos` and `sin` (glibc 2.35's double `sin` and `cos` are LGPL code and
//! not always correctly rounded: GLIBC-07).

use larmorx_core::math;

use crate::volops::Arith;
use crate::volume::Volume;

/// A `std::complex<double>`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct C {
    pub re: f64,
    pub im: f64,
}

impl C {
    #[inline]
    fn new(re: f64, im: f64) -> C {
        C { re, im }
    }
    #[inline]
    fn mul(self, b: C) -> C {
        C::new(
            self.re * b.re - self.im * b.im,
            self.re * b.im + self.im * b.re,
        )
    }
    #[inline]
    fn add(self, b: C) -> C {
        C::new(self.re + b.re, self.im + b.im)
    }
    #[inline]
    fn sub(self, b: C) -> C {
        C::new(self.re - b.re, self.im - b.im)
    }
    #[inline]
    fn scale(self, k: f64) -> C {
        C::new(self.re * k, self.im * k)
    }
}

/// Armadillo's `fft_engine_kissfft<cx_double, inverse>` for a length `n`.
pub struct KissFft {
    n: usize,
    inverse: bool,
    coeffs: Vec<C>,
    residue: Vec<usize>,
    radix: Vec<usize>,
    tmp: Vec<C>,
}

impl KissFft {
    /// The engine: the factors of `n` (4 first, then 2, 3, 5, 7, ...; the rest itself once its
    /// square exceeds it) and the twiddle factors `exp(i k n)`, `k = (+-2) pi / N`.
    pub fn new(n: usize, inverse: bool) -> KissFft {
        let (mut residue, mut radix) = (Vec::new(), Vec::new());
        let (mut m, mut r) = (n, 4usize);
        while m >= 2 {
            while m % r > 0 {
                r = match r {
                    2 => 3,
                    4 => 2,
                    _ => r + 2,
                };
                if r * r > m {
                    r = m;
                }
            }
            m /= r;
            residue.push(m);
            radix.push(r);
        }
        let k = (if inverse { 2.0 } else { -2.0 }) * math::acos(-1.0) / n as f64;
        let coeffs = (0..n)
            .map(|i| {
                let y = i as f64 * k;
                if y.abs() > f64::MIN_POSITIVE {
                    C::new(math::cos(y), math::sin(y))
                } else {
                    C::new(1.0, y)
                }
            })
            .collect();
        KissFft {
            n,
            inverse,
            coeffs,
            residue,
            radix,
            tmp: Vec::new(),
        }
    }

    fn butterfly_2(&self, y: &mut [C], y0: usize, stride: usize, m: usize) {
        for i in 0..m {
            let t = y[y0 + i + m].mul(self.coeffs[i * stride]);
            y[y0 + i + m] = y[y0 + i].sub(t);
            y[y0 + i] = y[y0 + i].add(t);
        }
    }

    fn butterfly_3(&self, y: &mut [C], y0: usize, stride: usize, m: usize) {
        let c = &self.coeffs;
        let sm_imag = c[stride * m].im;
        let n = m * 2;
        for j in 0..m {
            let b = y0 + j;
            let t1 = y[b + m].mul(c[j * stride]);
            let t2 = y[b + n].mul(c[j * stride * 2]);
            let t0 = t1.sub(t2).scale(sm_imag);
            let t3 = t1.add(t2);
            y[b + m] = C::new(y[b].re - 0.5 * t3.re, y[b].im - 0.5 * t3.im);
            y[b] = y[b].add(t3);
            y[b + n] = C::new(y[b + m].re + t0.im, y[b + m].im - t0.re);
            y[b + m] = y[b + m].add(C::new(-t0.im, t0.re));
        }
    }

    fn butterfly_4(&self, y: &mut [C], y0: usize, stride: usize, m: usize) {
        let c = &self.coeffs;
        let (m2, m3) = (m * 2, m * 3);
        for i in 0..m {
            let b = y0 + i;
            let t0 = y[b + m].mul(c[i * stride]);
            let t2 = y[b + m3].mul(c[i * stride * 3]);
            let t3 = t0.add(t2);
            let t4 = if self.inverse {
                C::new(t2.im - t0.im, t0.re - t2.re)
            } else {
                C::new(t0.im - t2.im, t2.re - t0.re)
            };
            let t1 = y[b + m2].mul(c[i * stride * 2]);
            let t5 = y[b].sub(t1);
            y[b] = y[b].add(t1);
            y[b + m2] = y[b].sub(t3);
            y[b] = y[b].add(t3);
            y[b + m] = t5.add(t4);
            y[b + m3] = t5.sub(t4);
        }
    }

    fn butterfly_5(&self, y: &mut [C], y0: usize, stride: usize, m: usize) {
        let c = &self.coeffs;
        let (ar, ai) = (c[stride * m].re, c[stride * m].im);
        let (br, bi) = (c[stride * 2 * m].re, c[stride * 2 * m].im);
        for i in 0..m {
            let (i0, i1, i2, i3, i4) = (
                y0 + i,
                y0 + m + i,
                y0 + 2 * m + i,
                y0 + 3 * m + i,
                y0 + 4 * m + i,
            );
            let t0 = y[i0];
            let t1 = y[i1].mul(c[stride * i]);
            let t2 = y[i2].mul(c[stride * 2 * i]);
            let t3 = y[i3].mul(c[stride * 3 * i]);
            let t4 = y[i4].mul(c[stride * 4 * i]);
            let t7 = t1.add(t4);
            let t8 = t2.add(t3);
            let t9 = t2.sub(t3);
            let t10 = t1.sub(t4);
            y[i0] = y[i0].add(t7);
            y[i0] = y[i0].add(t8);
            let t5 = t0.add(C::new(t7.re * ar + t8.re * br, t7.im * ar + t8.im * br));
            let t6 = C::new(t10.im * ai + t9.im * bi, -(t10.re * ai) - t9.re * bi);
            y[i1] = t5.sub(t6);
            y[i4] = t5.add(t6);
            let t11 = t0.add(C::new(t7.re * br + t8.re * ar, t7.im * br + t8.im * ar));
            let t12 = C::new(-(t10.im * bi) + t9.im * ai, t10.re * bi - t9.re * ai);
            y[i2] = t11.add(t12);
            y[i3] = t11.sub(t12);
        }
    }

    fn butterfly_n(&mut self, y: &mut [C], y0: usize, stride: usize, m: usize, r: usize) {
        if self.tmp.len() < r {
            self.tmp.resize(r, C::default());
        }
        for u in 0..m {
            let mut k = u;
            for v in 0..r {
                self.tmp[v] = y[y0 + k];
                k += m;
            }
            k = u;
            for _ in 0..r {
                y[y0 + k] = self.tmp[0];
                let mut j = 0usize;
                for w in 1..r {
                    j += stride * k;
                    if j >= self.n {
                        j -= self.n;
                    }
                    y[y0 + k] = y[y0 + k].add(self.tmp[w].mul(self.coeffs[j]));
                }
                k += m;
            }
        }
    }

    fn run(&mut self, y: &mut [C], y0: usize, x: &[C], x0: usize, stage: usize, stride: usize) {
        let m = self.residue[stage];
        let r = self.radix[stage];
        if m == 1 {
            for j in 0..r {
                y[y0 + j] = x[x0 + j * stride];
            }
        } else {
            for j in 0..r {
                self.run(y, y0 + j * m, x, x0 + j * stride, stage + 1, stride * r);
            }
        }
        match r {
            2 => self.butterfly_2(y, y0, stride, m),
            3 => self.butterfly_3(y, y0, stride, m),
            4 => self.butterfly_4(y, y0, stride, m),
            5 => self.butterfly_5(y, y0, stride, m),
            _ => self.butterfly_n(y, y0, stride, m, r),
        }
    }

    /// `fft(X)` (or `ifft(X)`: then each value times `1 / N`) of a column of `n` values.
    pub fn transform(&mut self, x: &[C], out: &mut [C]) {
        if self.n == 0 {
            return;
        }
        if self.n == 1 {
            out[0] = x[0];
            return;
        }
        self.run(out, 0, x, 0, 0, 1);
        if self.inverse {
            let k = 1.0 / self.n as f64;
            for v in out.iter_mut() {
                *v = v.scale(k);
            }
        }
    }
}

/// newimage's `fft` / `ifft` of a complex `float` volume (sizes `dims`): each line along x, then
/// y, then z (when there is more than one slice) transformed in `double` and stored back in
/// `float`.
pub fn fft3(re: &mut [f32], im: &mut [f32], dims: [usize; 3], inverse: bool) {
    let [nx, ny, nz] = dims;
    let idx = |x: usize, y: usize, z: usize| (z * ny + y) * nx + x;
    let mut pass = |n: usize, lines: Vec<Vec<usize>>| {
        let mut e = KissFft::new(n, inverse);
        let mut xin = vec![C::default(); n];
        let mut out = vec![C::default(); n];
        for line in lines {
            for (j, &k) in line.iter().enumerate() {
                xin[j] = C::new(f64::from(re[k]), f64::from(im[k]));
            }
            e.transform(&xin, &mut out);
            for (j, &k) in line.iter().enumerate() {
                re[k] = out[j].re as f32;
                im[k] = out[j].im as f32;
            }
        }
    };
    let mut lx = Vec::with_capacity(ny * nz);
    for z in 0..nz {
        for y in 0..ny {
            lx.push((0..nx).map(|x| idx(x, y, z)).collect());
        }
    }
    pass(nx, lx);
    let mut ly = Vec::with_capacity(nx * nz);
    for z in 0..nz {
        for x in 0..nx {
            ly.push((0..ny).map(|y| idx(x, y, z)).collect());
        }
    }
    pass(ny, ly);
    if nz > 1 {
        let mut lz = Vec::with_capacity(nx * ny);
        for x in 0..nx {
            for y in 0..ny {
                lz.push((0..nz).map(|z| idx(x, y, z)).collect());
            }
        }
        pass(nz, lz);
    }
}

/// efficient_convolve's FFT path of one volume with a kernel of sizes `kdims` (values `kdata`):
/// both inserted at the origin of `float` volumes twice the larger size along each axis,
/// transformed (`fft3`), multiplied (complexvolume's `*=` in `float`), transformed back
/// (`ifft3`), and the real part read from the kernel's centre offset (`extractpart`); `conv` is
/// `(T)` of the `float`.
pub fn fft_convolve<T: Arith>(
    vin: &Volume<T>,
    kdims: [i64; 3],
    kdata: &[f32],
    conv: &mut impl FnMut(f64) -> T,
) -> Volume<T> {
    let (vx, vy, vz) = (vin.xsize(), vin.ysize(), vin.zsize());
    let sx = (vx.max(kdims[0]) * 2) as usize;
    let sy = (vy.max(kdims[1]) * 2) as usize;
    let sz = (vz.max(kdims[2]) * 2) as usize;
    let n = sx * sy * sz;
    let idx = |x: usize, y: usize, z: usize| (z * sy + y) * sx + x;
    let (mut ir, mut ii) = (vec![0f32; n], vec![0f32; n]);
    let (mut kr, mut ki) = (vec![0f32; n], vec![0f32; n]);
    for z in 0..vz as usize {
        for y in 0..vy as usize {
            for x in 0..vx as usize {
                ir[idx(x, y, z)] = vin.value(x as i64, y as i64, z as i64).to_f32();
            }
        }
    }
    let [kx, ky, kz] = kdims.map(|d| d.max(0) as usize);
    for z in 0..kz {
        for y in 0..ky {
            for x in 0..kx {
                kr[idx(x, y, z)] = kdata[(z * ky + y) * kx + x];
            }
        }
    }
    fft3(&mut ir, &mut ii, [sx, sy, sz], false);
    fft3(&mut kr, &mut ki, [sx, sy, sz], false);
    for k in 0..n {
        let r2 = ir[k] * kr[k] - ii[k] * ki[k];
        let i2 = ir[k] * ki[k] + ii[k] * kr[k];
        ir[k] = r2;
        ii[k] = i2;
    }
    fft3(&mut ir, &mut ii, [sx, sy, sz], true);
    let (ox, oy, oz) = (
        ((kdims[0] - 1) / 2) as usize,
        ((kdims[1] - 1) / 2) as usize,
        ((kdims[2] - 1) / 2) as usize,
    );
    let mut out = vin.clone();
    for z in 0..vz as usize {
        for y in 0..vy as usize {
            for x in 0..vx as usize {
                let v = ir[idx(x + ox, y + oy, z + oz)];
                let i = out.idx(x as i64, y as i64, z as i64);
                out.data[i] = conv(f64::from(v));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The DFT by its definition, for comparison.
    fn dft(x: &[C], inverse: bool) -> Vec<C> {
        let n = x.len();
        let s = if inverse { 1.0 } else { -1.0 };
        (0..n)
            .map(|k| {
                let mut acc = C::default();
                for (j, v) in x.iter().enumerate() {
                    let a = s * 2.0 * std::f64::consts::PI * (j * k) as f64 / n as f64;
                    acc = acc.add(v.mul(C::new(a.cos(), a.sin())));
                }
                if inverse {
                    acc.scale(1.0 / n as f64)
                } else {
                    acc
                }
            })
            .collect()
    }

    #[test]
    fn every_radix_matches_the_dft() {
        for n in [2usize, 3, 4, 5, 6, 7, 8, 10, 12, 14, 18, 22, 26, 30, 49] {
            for inverse in [false, true] {
                let x: Vec<C> = (0..n)
                    .map(|j| C::new(((j * 7) % 11) as f64 - 3.0, ((j * 5) % 3) as f64))
                    .collect();
                let mut out = vec![C::default(); n];
                KissFft::new(n, inverse).transform(&x, &mut out);
                for (a, b) in out.iter().zip(dft(&x, inverse)) {
                    assert!(
                        (a.re - b.re).abs() < 1e-9 && (a.im - b.im).abs() < 1e-9,
                        "n={n}"
                    );
                }
            }
        }
    }
}
