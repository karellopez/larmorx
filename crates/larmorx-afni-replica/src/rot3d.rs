// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05: src/thd_rot3d.c (THD_rota_method, flip_xy, flip_yz,
// flip_xz, apply_xshear, apply_yshear, apply_zshear, apply_3shear, THD_rota_setpad,
// THD_rota_vol, THD_rota3D, THD_rota_vol_matvec, THD_rota3D_matvec) and src/edt_volpad.c
// (EDIT_volpad, mri_zeropad_3D). Major portions Copyright (C) 1994-2000 Medical College of
// Wisconsin (AFNI, Robert W. Cox et al.), released by AFNI 26.3.05 under the Creative Commons
// Attribution 4.0 International licence (CC BY 4.0). edt_volpad.c carries no such notice: it was
// written at the NIH and is in the public domain.
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05.
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! Rotating and shifting a volume as AFNI does: the four shears of [`crate::shear`], each a
//! shift of every row along its axis by the interpolator of [`crate::shift`], after an optional
//! flip of two axes, on the volume zero-padded on every side and cut back afterwards.
//!
//! AFNI keeps the interpolation method and the padding in static variables
//! (`THD_rota_method`, `THD_rota_setpad`); here they are a [`Rotator`]'s fields, passed
//! explicitly. Rows are shifted two at a time (`SHIFT_two_rows`: x rows `(j, j+1)`, y and z rows
//! of columns `(i, i+1)`); with the Fourier method the transform length is
//! `csfft_nextup_one35(n + 0.5·shift)`, the largest shift the shear asks of a row.
//!
//! A shear AFNI cannot compute (an invalid decomposition) leaves the volume as it is, with the
//! message `*** THD_rota_vol: can't compute shear transformation!`, which [`Rotator::messages`]
//! collects.

use crate::csfft::csfft_nextup_one35;
use crate::eispack::SvdError;
use crate::shear::{self, Shear};
use crate::shift::{Method, ShiftWork, shift_two_rows};
use crate::vecmat::{DMat33, DVec3, Libc};
use larmorx_core::defects::UpstreamDefects;

/// The rotation's state: AFNI's static `shifter`/`shift_method` and `rotpx`, `rotpy`, `rotpz`,
/// with the shifters' work space and the C library choice.
pub struct Rotator {
    pub method: Method,
    /// `rotpx, rotpy, rotpz`: the zero padding on each side (3dvolreg's `-zpad`).
    pub pad: [usize; 3],
    pub libc: Libc,
    work: ShiftWork,
    /// Messages AFNI prints on standard error while rotating, in order.
    pub messages: Vec<String>,
}

impl Rotator {
    /// A rotator with `method` and zero padding `pad` on each side, its C library functions and
    /// its Fourier phases chosen by `mode` (glibc's with `reproduce`).
    pub fn new(method: Method, pad: usize, mode: UpstreamDefects) -> Rotator {
        Rotator {
            method,
            pad: [pad; 3],
            libc: Libc::new(mode),
            work: ShiftWork::with_mode(mode),
            messages: Vec::new(),
        }
    }

    /// `THD_rota3D(im, ax1,th1, ax2,th2, ax3,th3, dcode, dx,dy,dz)` on a float volume: the
    /// rotated copy (`th` in radians, `d` in mm, `del` the voxel sizes).
    #[allow(clippy::too_many_arguments)]
    pub fn rota3d(
        &mut self,
        vol: &[f32],
        n: [usize; 3],
        del: [f32; 3],
        ax: [i32; 3],
        th: [f32; 3],
        dcode: i32,
        d: [f32; 3],
    ) -> Vec<f32> {
        let mut out = vol.to_vec();
        self.rota_vol(&mut out, n, del.map(f32::abs), ax, th, dcode, d);
        out
    }

    /// `THD_rota_vol`: rotates `vol` in place.
    #[allow(clippy::too_many_arguments)]
    pub fn rota_vol(
        &mut self,
        vol: &mut [f32],
        n: [usize; 3],
        mut del: [f32; 3],
        ax: [i32; 3],
        mut th: [f32; 3],
        dcode: i32,
        d: [f32; 3],
    ) {
        let [nx, ny, nz] = n;
        if nx < 2 || ny < 2 || nz < 2 {
            return;
        }
        for x in &mut del {
            if *x == 0.0 {
                *x = 1.0;
            }
        }
        if th == [0.0, 0.0, 0.0] {
            // Nudge the rotation.
            th = [1.0e-6, 1.1e-6, 0.9e-6];
        }
        let shr = shear::rot_to_shear(
            ax,
            [-f64::from(th[0]), -f64::from(th[1]), -f64::from(th[2])],
            dcode,
            d.map(f64::from),
            del.map(f64::from),
            self.libc,
        );
        if !shr.is_valid() {
            self.messages
                .push("*** THD_rota_vol: can't compute shear transformation!\n".to_owned());
            return;
        }
        self.padded(vol, n, &shr);
    }

    /// `THD_rota3D_matvec(im, rmat, tvec)`: the copy of `vol` moved by `x -> rmat·x + tvec`.
    pub fn rota3d_matvec(
        &mut self,
        vol: &[f32],
        n: [usize; 3],
        del: [f32; 3],
        rmat: &DMat33,
        tvec: &DVec3,
    ) -> Result<Vec<f32>, SvdError> {
        let mut out = vol.to_vec();
        self.rota_vol_matvec(&mut out, n, del.map(f32::abs), rmat, tvec)?;
        Ok(out)
    }

    /// `THD_rota_vol_matvec`: rotates `vol` in place by `x -> rmat·x + tvec`.
    pub fn rota_vol_matvec(
        &mut self,
        vol: &mut [f32],
        n: [usize; 3],
        mut del: [f32; 3],
        rmat: &DMat33,
        tvec: &DVec3,
    ) -> Result<(), SvdError> {
        let [nx, ny, nz] = n;
        if nx < 2 || ny < 2 || nz < 2 {
            return Ok(());
        }
        for x in &mut del {
            if *x == 0.0 {
                *x = 1.0;
            }
        }
        let shr = shear::rot_to_shear_matvec(rmat, tvec, del.map(f64::from), self.libc)?;
        if !shr.is_valid() {
            self.messages
                .push("*** THD_rota_vol: can't compute shear transformation!\n".to_owned());
            return Ok(());
        }
        self.padded(vol, n, &shr);
        Ok(())
    }

    /// The padding of `THD_rota_vol`: only when all three pads are positive, the shears on the
    /// padded volume, then the middle copied back.
    fn padded(&mut self, vol: &mut [f32], n: [usize; 3], shr: &Shear) {
        let [px, py, pz] = self.pad;
        let np = [n[0] + 2 * px, n[1] + 2 * py, n[2] + 2 * pz];
        if px > 0 && py > 0 && pz > 0 {
            let mut vvv = volpad(vol, n, [px, py, pz], [px, py, pz]);
            self.apply_3shear(shr, np, &mut vvv);
            let www = volcut(&vvv, np, [px, py, pz], n);
            vol.copy_from_slice(&www);
        } else {
            self.apply_3shear(shr, np, vol);
        }
    }

    /// `apply_3shear`: the preliminary flip, then each of the four shears.
    fn apply_3shear(&mut self, shr: &Shear, n: [usize; 3], vol: &mut [f32]) {
        if !shr.is_valid() {
            return;
        }
        if shr.flip0 >= 0 {
            match shr.flip0 + shr.flip1 {
                1 => flip_xy(n, vol),
                2 => flip_xz(n, vol),
                3 => flip_yz(n, vol),
                _ => return,
            }
        }
        for qq in 0..4 {
            match shr.ax[qq] {
                0 => {
                    let (a, b, s) = (
                        shr.scl[qq][1] as f32,
                        shr.scl[qq][2] as f32,
                        shr.sft[qq] as f32,
                    );
                    self.xshear(a, b, s, n, vol);
                }
                1 => {
                    let (a, b, s) = (
                        shr.scl[qq][0] as f32,
                        shr.scl[qq][2] as f32,
                        shr.sft[qq] as f32,
                    );
                    self.yshear(a, b, s, n, vol);
                }
                2 => {
                    let (a, b, s) = (
                        shr.scl[qq][0] as f32,
                        shr.scl[qq][1] as f32,
                        shr.sft[qq] as f32,
                    );
                    self.zshear(a, b, s, n, vol);
                }
                _ => {}
            }
        }
    }

    /// The transform length of a Fourier shear whose largest row shift is `st` on rows of
    /// length `n`: `csfft_nextup_one35(n + 0.5·st)` (`nst` an `int`); unused by the other
    /// methods.
    fn nup(&self, n: usize, st: f32) -> usize {
        if self.method == Method::Fourier {
            let nst = (n as f64 + 0.5 * f64::from(st)) as usize;
            csfft_nextup_one35(nst)
        } else {
            0
        }
    }

    /// `apply_xshear`: `x -> x + a·y + b·z + s`, rows `(j, j+1)` shifted together.
    fn xshear(&mut self, a: f32, b: f32, s: f32, n: [usize; 3], v: &mut [f32]) {
        let [nx, ny, nz] = n;
        let (ny1, nz1) = (ny - 1, nz - 1);
        let ny2 = (0.5 * ny1 as f64) as f32;
        let nz2 = (0.5 * nz1 as f64) as f32;
        let st = shift_extent(a, ny2, b, nz2, s);
        if st < 1.0e-3 {
            return;
        }
        let nup = self.nup(nx, st);
        let nxy = nx * ny;
        for kk in 0..nz {
            let mut jj = 0;
            while jj < ny {
                let off = jj * nx + kk * nxy;
                let a0 = a * (jj as f32 - ny2) + b * (kk as f32 - nz2) + s;
                let a1 = a0 + a;
                if jj < ny1 {
                    let (f, g) = v[off..off + 2 * nx].split_at_mut(nx);
                    shift_two_rows(self.method, nup, a0, f, a1, Some(g), &mut self.work);
                } else {
                    let f = &mut v[off..off + nx];
                    shift_two_rows(self.method, nup, a0, f, a1, None, &mut self.work);
                }
                jj += 2;
            }
        }
    }

    /// `apply_yshear`: `y -> y + a·x + b·z + s`, the y rows of columns `(i, i+1)` together.
    /// With odd `nx` the last column is shifted alone, AFNI passing the previous pair's `a1`
    /// as the unused second shift (which only the Fourier method's both-rows-out-of-range test
    /// reads).
    fn yshear(&mut self, a: f32, b: f32, s: f32, n: [usize; 3], v: &mut [f32]) {
        let [nx, ny, nz] = n;
        let (nx1, nz1) = (nx - 1, nz - 1);
        let nx2 = (0.5 * nx1 as f64) as f32;
        let nz2 = (0.5 * nz1 as f64) as f32;
        let st = shift_extent(a, nx2, b, nz2, s);
        if st < 1.0e-3 {
            return;
        }
        let nup = self.nup(ny, st);
        let nxy = nx * ny;
        let mut fj0 = vec![0.0f32; ny];
        let mut fj1 = vec![0.0f32; ny];
        let at = |i: usize, j: usize, k: usize| i + j * nx + k * nxy;
        for kk in 0..nz {
            let mut a1 = 0.0f32;
            let mut ii = 0;
            while ii < nx1 {
                for jj in 0..ny {
                    fj0[jj] = v[at(ii, jj, kk)];
                    fj1[jj] = v[at(ii + 1, jj, kk)];
                }
                let a0 = a * (ii as f32 - nx2) + b * (kk as f32 - nz2) + s;
                a1 = a0 + a;
                shift_two_rows(
                    self.method,
                    nup,
                    a0,
                    &mut fj0,
                    a1,
                    Some(&mut fj1),
                    &mut self.work,
                );
                for jj in 0..ny {
                    v[at(ii, jj, kk)] = fj0[jj];
                    v[at(ii + 1, jj, kk)] = fj1[jj];
                }
                ii += 2;
            }
            if ii == nx1 {
                for jj in 0..ny {
                    fj0[jj] = v[at(ii, jj, kk)];
                }
                let a0 = a * (ii as f32 - nx2) + b * (kk as f32 - nz2) + s;
                shift_two_rows(self.method, nup, a0, &mut fj0, a1, None, &mut self.work);
                for jj in 0..ny {
                    v[at(ii, jj, kk)] = fj0[jj];
                }
            }
        }
    }

    /// `apply_zshear`: `z -> z + a·x + b·y + s`, the z rows of columns `(i, i+1)` together.
    fn zshear(&mut self, a: f32, b: f32, s: f32, n: [usize; 3], v: &mut [f32]) {
        let [nx, ny, nz] = n;
        let (nx1, ny1) = (nx - 1, ny - 1);
        let nx2 = (0.5 * nx1 as f64) as f32;
        let ny2 = (0.5 * ny1 as f64) as f32;
        let st = shift_extent(a, nx2, b, ny2, s);
        if st < 1.0e-3 {
            return;
        }
        let nup = self.nup(nz, st);
        let nxy = nx * ny;
        let mut fj0 = vec![0.0f32; nz];
        let mut fj1 = vec![0.0f32; nz];
        let at = |i: usize, j: usize, k: usize| i + j * nx + k * nxy;
        for jj in 0..ny {
            // `a1` is a variable of the whole function in AFNI, kept from the last pair.
            let mut a1 = 0.0f32;
            let mut ii = 0;
            while ii < nx1 {
                for kk in 0..nz {
                    fj0[kk] = v[at(ii, jj, kk)];
                    fj1[kk] = v[at(ii + 1, jj, kk)];
                }
                let a0 = a * (ii as f32 - nx2) + b * (jj as f32 - ny2) + s;
                a1 = a0 + a;
                shift_two_rows(
                    self.method,
                    nup,
                    a0,
                    &mut fj0,
                    a1,
                    Some(&mut fj1),
                    &mut self.work,
                );
                for kk in 0..nz {
                    v[at(ii, jj, kk)] = fj0[kk];
                    v[at(ii + 1, jj, kk)] = fj1[kk];
                }
                ii += 2;
            }
            if ii == nx1 {
                for kk in 0..nz {
                    fj0[kk] = v[at(ii, jj, kk)];
                }
                let a0 = a * (ii as f32 - nx2) + b * (jj as f32 - ny2) + s;
                shift_two_rows(self.method, nup, a0, &mut fj0, a1, None, &mut self.work);
                for kk in 0..nz {
                    v[at(ii, jj, kk)] = fj0[kk];
                }
            }
        }
    }
}

/// The largest shift a shear asks of a row: `st = fabs(a)*n2 + fabs(b)*m2 + fabs(s)`, in double
/// (`fabs` of a float is a double), stored as float.
fn shift_extent(a: f32, n2: f32, b: f32, m2: f32, s: f32) -> f32 {
    (f64::from(a).abs() * f64::from(n2) + f64::from(b).abs() * f64::from(m2) + f64::from(s).abs())
        as f32
}

/// `flip_xy`: `i -> nx-1-i`, `j -> ny-1-j`.
fn flip_xy(n: [usize; 3], v: &mut [f32]) {
    let [nx, ny, nz] = n;
    let nxy = nx * ny;
    for kk in 0..nz {
        let slab = &mut v[kk * nxy..(kk + 1) * nxy];
        slab.reverse();
    }
}

/// `flip_yz`: `j -> ny-1-j`, `k -> nz-1-k`.
fn flip_yz(n: [usize; 3], v: &mut [f32]) {
    let [nx, ny, nz] = n;
    let nxy = nx * ny;
    let src = v.to_vec();
    for kk in 0..nz {
        for jj in 0..ny {
            for ii in 0..nx {
                v[ii + jj * nx + kk * nxy] = src[ii + (ny - 1 - jj) * nx + (nz - 1 - kk) * nxy];
            }
        }
    }
}

/// `flip_xz`: `i -> nx-1-i`, `k -> nz-1-k`.
fn flip_xz(n: [usize; 3], v: &mut [f32]) {
    let [nx, ny, nz] = n;
    let nxy = nx * ny;
    let src = v.to_vec();
    for kk in 0..nz {
        for jj in 0..ny {
            for ii in 0..nx {
                v[ii + jj * nx + kk * nxy] = src[(nx - 1 - ii) + jj * nx + (nz - 1 - kk) * nxy];
            }
        }
    }
}

/// `EDIT_volpad(nxbot,nxtop, nybot,nytop, nzbot,nztop, ...)` with non-negative pads: the volume
/// in the middle of a larger one of zeros.
pub fn volpad(v: &[f32], n: [usize; 3], bot: [usize; 3], top: [usize; 3]) -> Vec<f32> {
    let [nx, ny, nz] = n;
    let nn = [
        nx + bot[0] + top[0],
        ny + bot[1] + top[1],
        nz + bot[2] + top[2],
    ];
    let mut out = vec![0.0f32; nn[0] * nn[1] * nn[2]];
    for kk in 0..nz {
        for jj in 0..ny {
            let src = jj * nx + kk * nx * ny;
            let dst = bot[0] + (jj + bot[1]) * nn[0] + (kk + bot[2]) * nn[0] * nn[1];
            out[dst..dst + nx].copy_from_slice(&v[src..src + nx]);
        }
    }
    out
}

/// `EDIT_volpad` with negative pads `-cut` on each side: the middle `n` of a volume of size
/// `np`.
pub fn volcut(v: &[f32], np: [usize; 3], cut: [usize; 3], n: [usize; 3]) -> Vec<f32> {
    let [nx, ny, nz] = n;
    let mut out = vec![0.0f32; nx * ny * nz];
    for kk in 0..nz {
        for jj in 0..ny {
            let src = cut[0] + (jj + cut[1]) * np[0] + (kk + cut[2]) * np[0] * np[1];
            let dst = jj * nx + kk * nx * ny;
            out[dst..dst + nx].copy_from_slice(&v[src..src + nx]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flips_are_their_own_inverse() {
        let n = [3, 4, 5];
        let v: Vec<f32> = (0..60).map(|i| i as f32).collect();
        for f in [flip_xy, flip_yz, flip_xz] {
            let mut w = v.clone();
            f(n, &mut w);
            assert_ne!(w, v);
            f(n, &mut w);
            assert_eq!(w, v);
        }
        let mut w = v.clone();
        flip_xy(n, &mut w);
        assert_eq!(w[0], v[2 + 3 * 3]);
    }

    #[test]
    fn padding_round_trip() {
        let n = [3, 2, 2];
        let v: Vec<f32> = (1..=12).map(|i| i as f32).collect();
        let p = volpad(&v, n, [2, 1, 3], [2, 1, 3]);
        assert_eq!(p.len(), 7 * 4 * 8);
        assert_eq!(p.iter().filter(|&&x| x != 0.0).count(), 12);
        assert_eq!(volcut(&p, [7, 4, 8], [2, 1, 3], n), v);
    }

    #[test]
    fn a_shift_moves_a_volume() {
        // A whole-voxel shift along x with linear interpolation moves the data by one voxel.
        let n = [8, 6, 5];
        let v: Vec<f32> = (0..240).map(|i| ((i * 7) % 13) as f32).collect();
        let mut r = Rotator::new(Method::Linear, 0, UpstreamDefects::Fix);
        let out = r.rota3d(&v, n, [1.0; 3], [2, 0, 1], [0.0; 3], -1, [1.0, 0.0, 0.0]);
        // Interior voxels away from the edges: the value of the voxel one step back.
        let at = |i: usize, j: usize, k: usize| i + j * 8 + k * 48;
        assert!(
            (out[at(4, 3, 2)] - v[at(3, 3, 2)]).abs() < 1e-3,
            "{} {}",
            out[at(4, 3, 2)],
            v[at(3, 3, 2)]
        );
    }
}
