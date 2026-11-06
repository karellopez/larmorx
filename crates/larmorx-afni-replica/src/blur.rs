// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05 src/edt_blur.c (EDIT_blur_volume_3d for float volumes,
// fir_blurx, fir_blury, fir_blurz with the BMIRROR and RMIRROR macros, FIR_blur_volume,
// FIR_blur_volume_3d, fir_gaussian_load). Major portions Copyright (C) 1994-2000 Medical College
// of Wisconsin (AFNI, Robert W. Cox et al.), released by AFNI 26.3.05 under the Creative Commons
// Attribution 4.0 International licence (CC BY 4.0).
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05.
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! AFNI's Gaussian blur of a float volume: along each axis a symmetric FIR filter of `m`
//! points on each side when `m = ceil(sfac·sigma/d) <= 35` (`sfac` 2.5, or
//! `AFNI_BLUR_FIRFAC`), else a multiplication by the Gaussian in Fourier space (AFNI's
//! `csfft_cox`), after which the result is clipped to the input's range.
//!
//! The FIR weights (`fir_gaussian_load`) average the Gaussian over 23 points of each cell
//! (trapezoid, ends weighted 1/2), normalised to sum 1; with `AFNI_BLUR_FIROLD=YES` its value
//! at each cell's centre. The ends of each row are mirrored: along x and y (`BMIRROR`) about
//! the first point but about the half-point after the last (`r[-q] = r[q]`, `r[n-1+q] =
//! r[n-q]`), along z (`RMIRROR`) about the first and the last point (`r[n-1+q] = r[n-1-q]`);
//! along y in volumes 512 or more voxels wide one point at each end only (`r[-1] = r[1]`,
//! `r[n] = r[n-2]`), the rest of the border zero. Both inconsistencies are AFNI's, kept in
//! both modes until measured (`docs/findings/afni-volreg.md`).
//! An axis shorter than `m + 1` is not blurred. The sums keep AFNI's order: for `m` up to 7 the
//! unrolled `w_m·(r[-m]+r[m]) + ... + w_1·(r[-1]+r[1]) + w_0·r[0]`, beyond it
//! `w_0·r[0] + w_1·(r[-1]+r[1]) + ...`.
//!
//! `EDIT_blur_volume_3d` prints `edt_blur: sigx ..., dx ..., fwhm ..., sfac ..., firx ...` the
//! first time a program calls it ([`Blur::note`]).

use crate::csfft::{Complex32, CsfftPlan, csfft_nextup_one35};
use crate::vecmat::Libc;
use larmorx_core::defects::UpstreamDefects;

/// `FIR_MAX`: the longest FIR filter used instead of FFTs.
const FIR_MAX: i32 = 35;

/// `PI` of `mrilib.h`.
const PI: f64 = std::f64::consts::PI;

/// The settings and state of AFNI's blur: its environment variables, the C library choice, and
/// whether the once-per-run message was printed (`bcomment`).
#[derive(Clone, Debug)]
pub struct Blur {
    /// `AFNI_numenv("AFNI_BLUR_FIRFAC")` (0 when unset).
    pub firfac: f64,
    /// `AFNI_yesenv("AFNI_BLUR_FIROLD")`.
    pub firold: bool,
    pub libc: Libc,
    mode: UpstreamDefects,
    /// The message `EDIT_blur_volume_3d` prints once, when it is first called.
    pub note: Option<String>,
    noted: bool,
}

impl Blur {
    pub fn new(firfac: f64, firold: bool, mode: UpstreamDefects) -> Blur {
        Blur {
            firfac,
            firold,
            libc: Libc::new(mode),
            mode,
            note: None,
            noted: false,
        }
    }

    /// Takes the once-per-run message, if it was just produced.
    pub fn take_note(&mut self) -> Option<String> {
        self.note.take()
    }

    /// `EDIT_blur_volume_3d(nx,ny,nz, dx,dy,dz, MRI_float, f, sx,sy,sz)`.
    #[allow(clippy::too_many_arguments)]
    pub fn blur_volume_3d(
        &mut self,
        n: [usize; 3],
        mut d: [f32; 3],
        f: &mut [f32],
        s: [f32; 3],
    ) {
        let mut sfac = self.firfac;
        if sfac <= 0.0 {
            sfac = 2.5;
        }
        if s[0] <= 0.0 && s[1] <= 0.0 && s[2] <= 0.0 {
            return;
        }
        if d[0] <= 0.0 {
            d[0] = 1.0;
        }
        if d[1] <= 0.0 {
            d[1] = d[0];
        }
        if d[2] <= 0.0 {
            d[2] = d[0];
        }
        let [nx, ny, nz] = n;
        let nxyz = nx * ny * nz;
        // allow_fir is always 1.
        let m_of = |sig: f32, dd: f32| (sfac * f64::from(sig) / f64::from(dd)).ceil() as i32;
        let (ii, jj, kk) = (m_of(s[0], d[0]), m_of(s[1], d[1]), m_of(s[2], d[2]));
        let all_fir = ii <= FIR_MAX && jj <= FIR_MAX && kk <= FIR_MAX;
        if !self.noted {
            self.noted = true;
            self.note = Some(format!(
                "edt_blur: sigx {}, dx {}, fwhm {}, sfac {}, firx {}\n",
                crate::cnum::fmt_g(f64::from(s[0])),
                crate::cnum::fmt_g(f64::from(d[0])),
                crate::cnum::fmt_g(f64::from(s[0]) / 0.4246609),
                crate::cnum::fmt_g(sfac),
                ii
            ));
        }
        let (mut fbot, mut ftop) = (0.0f32, 0.0f32);
        if !all_fir && nxyz > 0 {
            fbot = f[0];
            ftop = f[0];
            for &v in &f[1..nxyz] {
                if v < fbot {
                    fbot = v;
                } else if v > ftop {
                    ftop = v;
                }
            }
        }
        let mut fir_num = 0;
        for axis in 0..3 {
            if n[axis] < 2 || s[axis] <= 0.0 {
                fir_num += 1;
                continue;
            }
            let mut m = m_of(s[axis], d[axis]);
            if m <= FIR_MAX {
                if m < 1 {
                    m = 1;
                }
                let wt = self.gaussian_load(m, d[axis] / s[axis]);
                self.fir(axis, m as usize, &wt, n, f);
                fir_num += 1;
            } else {
                self.fft_axis(axis, n, d[axis], s[axis], f);
            }
        }
        if !all_fir && fir_num < 3 {
            for v in &mut f[..nxyz] {
                if *v < fbot {
                    *v = fbot;
                } else if *v > ftop {
                    *v = ftop;
                }
            }
        }
    }

    /// `FIR_blur_volume(nx,ny,nz, dx,dy,dz, f, sigma)`: FIR only, `m` at most half the axis;
    /// `sfac` below 2 gives 2.5.
    pub fn fir_blur_volume(&self, n: [usize; 3], mut d: [f32; 3], f: &mut [f32], sigma: f32) {
        if sigma <= 0.0 {
            return;
        }
        let mut sfac = self.firfac;
        if sfac < 2.0 {
            sfac = 2.5;
        }
        if d[0] <= 0.0 {
            d[0] = 1.0;
        }
        if d[1] <= 0.0 {
            d[1] = d[0];
        }
        if d[2] <= 0.0 {
            d[2] = d[0];
        }
        for axis in 0..3 {
            if n[axis] > 1 {
                let mut m = (sfac * f64::from(sigma) / f64::from(d[axis])).ceil() as i32;
                if m < 1 {
                    m = 1;
                }
                let half = (n[axis] / 2) as i32;
                if m > half {
                    m = half;
                }
                let wt = self.gaussian_load(m, d[axis] / sigma);
                self.fir(axis, m as usize, &wt, n, f);
            }
        }
    }

    /// `fir_gaussian_load(m, dx, ff)`: the `m + 1` weights.
    fn gaussian_load(&self, m: i32, dx: f32) -> Vec<f32> {
        let m = m as usize;
        let mut ff = vec![0.0f32; m + 1];
        if dx <= 0.0 {
            return ff;
        }
        let mut fac: f32;
        if self.firold {
            fac = 1.0;
            ff[0] = 1.0;
            for ii in 1..=m {
                let xx = f64::from(ii as f32 * dx);
                ff[ii] = self.libc.exp(-0.5 * xx * xx) as f32;
                fac += 2.0f32 * ff[ii];
            }
        } else {
            const NG: i32 = 11;
            fac = 0.0;
            let dxj = f64::from(dx) / (2.0 * f64::from(NG));
            for ii in 0..=m {
                let mut sum = 0.0f64;
                for jj in -NG..=NG {
                    // ii*dx is a float product.
                    let xx = f64::from(ii as f32 * dx) + f64::from(jj) * dxj;
                    let ee = self.libc.exp(-0.5 * xx * xx);
                    if jj == -NG || jj == NG {
                        sum += 0.5 * ee;
                    } else {
                        sum += ee;
                    }
                }
                ff[ii] = sum as f32;
                fac = if ii == 0 {
                    (f64::from(fac) + sum) as f32
                } else {
                    (f64::from(fac) + 2.0 * sum) as f32
                };
            }
        }
        fac = 1.0f32 / fac;
        for w in &mut ff {
            *w *= fac;
        }
        ff
    }

    /// `fir_blurx`, `fir_blury`, `fir_blurz`.
    fn fir(&self, axis: usize, m: usize, wt: &[f32], n: [usize; 3], f: &mut [f32]) {
        let [nx, ny, nz] = n;
        if m < 1 || n[axis] < m + 1 || nx == 0 || ny == 0 || nz == 0 {
            return;
        }
        let nxy = nx * ny;
        let len = n[axis];
        let mut r = vec![0.0f32; len + 2 * m];
        // fir_blury's large-volume path (nx >= 512) mirrors one point at each end.
        let one_point = axis == 1 && nx >= 512;
        // (stride, starts of the rows)
        let (stride, starts): (usize, Vec<usize>) = match axis {
            0 => (1, (0..nz).flat_map(|k| (0..ny).map(move |j| j * nx + k * nxy)).collect()),
            1 => (nx, (0..nz).flat_map(|k| (0..nx).map(move |i| i + k * nxy)).collect()),
            _ => (nxy, (0..ny).flat_map(|j| (0..nx).map(move |i| i + j * nx)).collect()),
        };
        for start in starts {
            for q in 0..len {
                r[q + m] = f[start + q * stride];
            }
            if one_point {
                r[..m].fill(0.0);
                r[len + m..].fill(0.0);
                r[m - 1] = r[m + 1];
                r[len + m] = r[len + m - 2];
            } else if axis == 2 {
                // RMIRROR: mirrored about the first and the last point.
                for mm in 1..=m {
                    r[m - mm] = r[m + mm];
                    r[m + len - 1 + mm] = r[m + len - 1 - mm];
                }
            } else {
                // BMIRROR: mirrored about the first point, but about the half-point after the
                // last one (the last value repeated).
                for mm in 1..=m {
                    r[m - mm] = r[m + mm];
                    r[m + len - 1 + mm] = r[m + len - mm];
                }
            }
            for q in 0..len {
                f[start + q * stride] = fir_point(&r[q..q + 2 * m + 1], m, wt);
            }
        }
    }

    /// The Fourier blur along `axis`: rows (pairs of rows as one complex row) padded with zeros
    /// to `csfft_nextup_one35(n + (int)(3·sigma/d))`, multiplied by `exp(-sigma²k²/2)/nup`.
    fn fft_axis(&self, axis: usize, n: [usize; 3], d: f32, sigma: f32, f: &mut [f32]) {
        let [nx, ny, nz] = n;
        let nxy = nx * ny;
        let len = n[axis];
        let aa = (f64::from(sigma * sigma) * 0.5) as f32;
        let nup = csfft_nextup_one35(len + (3.0 * f64::from(sigma) / f64::from(d)) as usize);
        let nby2 = nup / 2;
        let dk = ((2.0 * PI) / f64::from(nup as f32 * d)) as f32;
        let fac = (1.0 / nup as f64) as f32;
        let mut gg = vec![0.0f32; nup];
        gg[0] = fac;
        for ii in 1..=nby2 {
            let k = ii as f32 * dk;
            let g = (f64::from(fac) * self.libc.exp(f64::from(-aa * k * k))) as f32;
            gg[ii] = g;
            gg[nup - ii] = g;
        }
        let plan = CsfftPlan::with_mode(nup, self.mode).unwrap_or_else(|e| panic!("{e}"));
        let mut scratch = vec![Complex32::default(); plan.scratch_len()];
        let mut cx = vec![Complex32::default(); nup];
        // (stride, row starts, the other row's offset)
        let (stride, pairs, next, count) = match axis {
            0 => (1, (0..nz).flat_map(|k| (0..ny).step_by(2).map(move |j| (j, j * nx + k * nxy))).collect::<Vec<_>>(), nx, ny),
            1 => (nx, (0..nz).flat_map(|k| (0..nx).step_by(2).map(move |i| (i, i + k * nxy))).collect(), 1, nx),
            _ => (nxy, (0..ny).flat_map(|j| (0..nx).step_by(2).map(move |i| (i, i + j * nx))).collect(), 1, nx),
        };
        for (idx, base) in pairs {
            let single = idx == count - 1;
            for q in 0..len {
                let re = f[base + q * stride];
                let im = if single { 0.0 } else { f[base + next + q * stride] };
                cx[q] = Complex32::new(re, im);
            }
            for c in &mut cx[len..] {
                *c = Complex32::new(0.0, 0.0);
            }
            plan.execute_with_scratch(-1, &mut cx, &mut scratch);
            for (c, &g) in cx.iter_mut().zip(&gg) {
                c.r *= g;
                c.i *= g;
            }
            plan.execute_with_scratch(1, &mut cx, &mut scratch);
            for q in 0..len {
                f[base + q * stride] = cx[q].r;
                if !single {
                    f[base + next + q * stride] = cx[q].i;
                }
            }
        }
    }
}

/// One output point of a FIR filter: `r` is the window `[-m, m]` around it.
#[inline]
fn fir_point(r: &[f32], m: usize, wt: &[f32]) -> f32 {
    if m <= 7 {
        // The unrolled cases: from the outermost pair in, the centre last.
        let mut sum = wt[m] * (r[0] + r[2 * m]);
        for q in (1..m).rev() {
            sum += wt[q] * (r[m - q] + r[m + q]);
        }
        sum + wt[0] * r[m]
    } else {
        let mut sum = wt[0] * r[m];
        for q in 1..=m {
            sum += wt[q] * (r[m - q] + r[m + q]);
        }
        sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_sum_to_one() {
        let b = Blur::new(0.0, false, UpstreamDefects::Fix);
        for (m, dx) in [(5, 0.5f32), (8, 1.0 / 3.0), (1, 2.0), (12, 0.2)] {
            let w = b.gaussian_load(m, dx);
            let s: f32 = w[0] + 2.0 * w[1..].iter().sum::<f32>();
            assert!((s - 1.0).abs() < 1e-5, "{m} {dx}: {s}");
        }
    }

    #[test]
    fn a_constant_stays_constant() {
        let mut b = Blur::new(0.0, false, UpstreamDefects::Fix);
        let n = [12, 10, 9];
        let mut f = vec![3.0f32; 12 * 10 * 9];
        b.blur_volume_3d(n, [1.0; 3], &mut f, [2.0; 3]);
        assert!(f.iter().all(|&x| (x - 3.0).abs() < 1e-5));
        assert!(b.take_note().unwrap().starts_with("edt_blur: sigx 2, dx 1, fwhm 4.70964"));
        assert!(b.take_note().is_none());
        // The FFT path (m > 35).
        let mut f = vec![3.0f32; 12 * 10 * 9];
        b.blur_volume_3d(n, [0.1; 3], &mut f, [2.0; 3]);
        assert!(f.iter().all(|&x| (x - 3.0).abs() < 1e-3));
    }
}
