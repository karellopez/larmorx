// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05: src/mri_3dalign.c (mri_3dalign_params,
// mri_3dalign_method, mri_3dalign_initvals, mri_3dalign_final_regmode, mri_3dalign_edging,
// mri_3dalign_force_edging, mri_3dalign_wproccing, mri_3dalign_scaleinit, mri_3dalign_setup,
// mri_3dalign_one), src/thd_automask.c (MRI_autobbox), src/mri_cut.c (mri_cut_3D), and
// src/mri_max.c (mri_max, mri_min of a float image). Major portions Copyright (C) 1994-2000
// Medical College of Wisconsin (AFNI, Robert W. Cox et al.), released by AFNI 26.3.05 under the
// Creative Commons Attribution 4.0 International licence (CC BY 4.0). thd_automask.c carries no
// such notice: it was written at the NIH and is in the public domain.
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05.
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! AFNI's rigid-body registration of a volume to a base (`mri_3dalign_*`): iterated linearised
//! weighted least squares.
//!
//! **Set-up** ([`Align3d::setup`]): the weight (the base's absolute value blurred with a
//! Gaussian of 3 voxels' width per axis and clipped below 2.5 % of its maximum, unless one is
//! given; a given one processed the same way with `wproc`), its faces zeroed (the edging), and
//! cropped to the bounding box of its main cluster when that saves at least 10 % (`wtrim`,
//! always on in 3dvolreg); then six derivative images by central differences of the base
//! rotated by `±delta` about each axis (`delta = 2·delfac/(nx+ny+nz)` degrees) and shifted by
//! `±delfac·d` along each, scaled by `sinit`, and the least squares on the base and these six
//! ([`crate::lsqfit`]).
//!
//! **One volume** ([`Align3d::one`]): a first fit of the volume itself (unless initial values
//! were set), then, while any parameter moved by more than its threshold (`dxy` voxels, `dph`
//! degrees) and fewer than `max_iter` iterations ran, the volume rotated by the current
//! parameters is fitted for the increments; finally the volume is rotated by the result with
//! the final method and clipped to its own range (for the cubic, quintic, heptic and Fourier
//! methods, when `clipit`).
//!
//! AFNI keeps the parameters in static variables; here they are the fields of [`Align3d`].

use crate::blur::Blur;
use crate::lsqfit::{self, LsqError, Startup};
use crate::mask::{self, Grid, MaskSettings};
use crate::message::Message;
use crate::rot3d::Rotator;
use crate::shift::Method;

/// `DFAC`: degrees to radians.
pub const DFAC: f64 = std::f64::consts::PI / 180.0;

/// A float volume with its voxel sizes (`MRI_IMAGE`'s `nx..nz`, `dx..dz`).
#[derive(Clone, Debug, PartialEq)]
pub struct Vol {
    pub n: [usize; 3],
    pub d: [f32; 3],
    pub data: Vec<f32>,
}

impl Vol {
    pub fn nvox(&self) -> usize {
        self.n[0] * self.n[1] * self.n[2]
    }
}

/// `mri_max` of a float image: from `-1e38`, `MAX(a,b) = a < b ? b : a`, as a double.
pub fn mri_max(v: &[f32]) -> f64 {
    let mut m = -1.0e38f32;
    for &x in v {
        if m < x {
            m = x;
        }
    }
    f64::from(m)
}

/// `mri_min` of a float image: from `1e38`, `MIN(a,b) = a > b ? b : a`.
pub fn mri_min(v: &[f32]) -> f64 {
    let mut m = 1.0e38f32;
    for &x in v {
        if m > x {
            m = x;
        }
    }
    f64::from(m)
}

/// A crop box (`xa..xb`, inclusive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Box3 {
    pub a: [usize; 3],
    pub b: [usize; 3],
}

impl Box3 {
    fn dims(&self) -> [usize; 3] {
        [self.b[0] - self.a[0] + 1, self.b[1] - self.a[1] + 1, self.b[2] - self.a[2] + 1]
    }
}

/// `mri_cut_3D`: the sub-volume in `bx`.
pub fn cut(v: &[f32], n: [usize; 3], bx: &Box3) -> Vec<f32> {
    let [qx, qy, qz] = bx.dims();
    let mut out = Vec::with_capacity(qx * qy * qz);
    let (nx, nxy) = (n[0], n[0] * n[1]);
    for zz in bx.a[2]..=bx.b[2] {
        for yy in bx.a[1]..=bx.b[1] {
            let s = zz * nxy + yy * nx + bx.a[0];
            out.extend_from_slice(&v[s..s + qx]);
        }
    }
    out
}

/// `MRI_autobbox(qim, &xm,&xp, &ym,&yp, &zm,&zp)`: the first and last planes along each axis
/// that hold a voxel of the image's main cluster (the non-zero voxels clustered, eroded once
/// with `peelcount` 1, and clustered again). Returns `(xm, xp, ym, yp, zm, zp)` as AFNI's ints:
/// all 0 for an image of zeros; an empty cluster gives `n` and -1.
pub fn autobbox(v: &[f32], n: [usize; 3]) -> [i64; 6] {
    let mut mmm: Vec<u8> = v.iter().map(|&x| u8::from(x != 0.0)).collect();
    if !mmm.iter().any(|&m| m != 0) {
        return [0; 6];
    }
    let g = Grid::new(n[0], n[1], n[2]);
    let s = MaskSettings::default();
    let mut log = Vec::new();
    mask::clust(g, &mut mmm, &s, &mut log);
    mask::erodemany(g, &mut mmm, s.peelcount, &s);
    mask::clust(g, &mut mmm, &s, &mut log);
    let [nx, ny, nz] = n;
    let nxy = nx * ny;
    let on = |i: usize, j: usize, k: usize| mmm[i + j * nx + k * nxy] != 0;
    let plane = |axis: usize, idx: usize| -> bool {
        match axis {
            0 => (0..nz).any(|k| (0..ny).any(|j| on(idx, j, k))),
            1 => (0..nz).any(|k| (0..nx).any(|i| on(i, idx, k))),
            _ => (0..ny).any(|j| (0..nx).any(|i| on(i, j, idx))),
        }
    };
    let mut out = [0i64; 6];
    for axis in 0..3 {
        let len = n[axis];
        out[2 * axis] = (0..len).find(|&i| plane(axis, i)).map_or(len as i64, |i| i as i64);
        out[2 * axis + 1] = (0..len).rev().find(|&i| plane(axis, i)).map_or(-1, |i| i as i64);
    }
    out
}

/// The interpolation modes 3dvolreg passes as `regmode` and `final_regmode` (`MRI_NN`,
/// `MRI_LINEAR`, `MRI_CUBIC`, `MRI_FOURIER`, `MRI_QUINTIC`, `MRI_HEPTIC`: 0 to 5, the order of
/// its `modes[]` names).
pub fn clips(m: Method) -> bool {
    matches!(m, Method::Quintic | Method::Cubic | Method::Heptic | Method::Fourier)
}

/// The registration's parameters and state (the static variables of `mri_3dalign.c`).
#[derive(Clone, Debug)]
pub struct Align3d {
    pub max_iter: i32,
    pub dxy_thresh: f32,
    pub phi_thresh: f32,
    pub delfac: f32,
    pub ax: [i32; 3],
    pub dcode: i32,
    pub wproc: bool,
    pub wtrim: bool,
    pub sinit: f32,
    pub init: [f32; 6],
    pub regmode: Method,
    /// `verbose` of `mri_3dalign_method` (3dvolreg: `-verbose -verbose`).
    pub verbose: bool,
    pub noreg: bool,
    pub clipit: bool,
    pub final_regmode: Option<Method>,
    pub edge: [i32; 3],
    pub force_edging: bool,
}

impl Default for Align3d {
    fn default() -> Align3d {
        Align3d {
            max_iter: 5,
            dxy_thresh: 0.07,
            phi_thresh: 0.21,
            delfac: 1.5,
            ax: [0, 1, 2],
            dcode: -1,
            wproc: false,
            wtrim: false,
            sinit: 1.0,
            init: [0.0; 6],
            regmode: Method::Quintic,
            verbose: false,
            noreg: false,
            clipit: false,
            final_regmode: None,
            edge: [-1, -1, -1],
            force_edging: false,
        }
    }
}

/// The set-up base (`MRI_3dalign_basis`): the least squares on the base and its six derivative
/// images (possibly cropped), or why it failed (AFNI keeps a basis without a factorisation and
/// fails when it fits a volume).
#[derive(Debug, Clone)]
pub struct Basis {
    pub fit: Result<Startup, LsqError>,
    pub trim: Option<Box3>,
    pub n: [usize; 3],
}

/// `"%g"` of a double.
fn g(x: f64) -> String {
    crate::cnum::fmt_g(x)
}

/// `"%13.6g"`.
fn g13(x: f64) -> String {
    format!("{:>13}", g(x))
}

impl Align3d {
    /// `mri_3dalign_params(maxite, dxy, dph, dfac, bx1, bx2, bx3, dc)`.
    #[allow(clippy::too_many_arguments)]
    pub fn params(&mut self, maxite: i32, dxy: f32, dph: f32, dfac: f32, bx: [i32; 3], dc: i32) {
        self.max_iter = if maxite > 0 { maxite } else { 5 };
        self.dxy_thresh = if dxy > 0.0 { dxy } else { 0.07 };
        self.phi_thresh = if dph > 0.0 { dph } else { 0.21 };
        if dfac > 0.0 {
            self.delfac = dfac;
        }
        for i in 0..3 {
            if (0..=2).contains(&bx[i]) {
                self.ax[i] = bx[i];
            }
        }
        self.dcode = dc;
    }

    /// `mri_3dalign_method(rmode, verb, norgg, clip)`.
    pub fn method(&mut self, rmode: Method, verb: bool, noreg: bool, clip: bool) {
        self.regmode = rmode;
        self.verbose = verb;
        self.noreg = noreg;
        self.clipit = clip;
    }

    /// `mri_3dalign_scaleinit`: a positive value replaces `sinit`.
    pub fn scaleinit(&mut self, s: f32) {
        if s > 0.0 {
            self.sinit = s;
        }
    }

    fn nonzero_init(&self) -> bool {
        self.init.iter().any(|&v| v != 0.0)
    }

    /// `mri_3dalign_setup(imbase, imwt)`. AFNI's messages go to `log`.
    pub fn setup(
        &self,
        base: &Vol,
        wt: Option<&Vol>,
        rot: &mut Rotator,
        blur: &mut Blur,
        log: &mut Vec<Message>,
    ) -> Basis {
        let n = base.n;
        let [nx, ny, nz] = n;
        let nxyz = nx * ny * nz;
        let mut cim = base.data.clone();
        crate::dataset::floatscan(&mut cim);
        let fixd = |x: f32| if x.abs() == 0.0 { 1.0 } else { x.abs() };
        let d = [fixd(base.d[0]), fixd(base.d[1]), fixd(base.d[2])];
        let raw = |s: String, log: &mut Vec<Message>| log.push(Message::Raw(s));

        let mut wt = wt;
        if let Some(w) = wt
            && w.n != n
        {
            raw(
                "*** WARNING: in mri_3dalign_setup, weight image mismatch!\n".to_owned(),
                log,
            );
            wt = None;
        }
        let mut imww: Vec<f32>;
        let process = |f: &mut Vec<f32>, blur: &mut Blur, log: &mut Vec<Message>| {
            for x in f.iter_mut() {
                *x = x.abs();
            }
            blur.blur_volume_3d(n, d, f, [3.0 * d[0], 3.0 * d[1], 3.0 * d[2]].map(|v| v as f32));
            if let Some(m) = blur.take_note() {
                log.push(Message::Info(m));
            }
            if self.verbose {
                raw(":".to_owned(), log);
            }
            let clip = (0.025 * mri_max(f)) as f32;
            for x in f.iter_mut() {
                if *x < clip {
                    *x = 0.0;
                }
            }
        };
        match wt {
            None => {
                imww = cim.clone();
                if self.verbose {
                    raw("  initializing weight".to_owned(), log);
                }
                process(&mut imww, blur, log);
            }
            Some(w) => {
                imww = w.data.clone();
                crate::dataset::floatscan(&mut imww);
                if self.wproc {
                    if self.verbose {
                        raw("  processing weight".to_owned(), log);
                    }
                    process(&mut imww, blur, log);
                }
            }
        }

        // The fade out around the edges.
        if wt.is_none() || self.force_edging {
            let [xf, yf, zf] = self.edge.map(|e| e.max(0) as usize);
            let nxy = nx * ny;
            let mut zero = |i: usize, j: usize, k: usize| imww[i + j * nx + k * nxy] = 0.0;
            for jj in 0..ny {
                for ii in 0..nx {
                    for ff in 0..zf {
                        zero(ii, jj, ff);
                        zero(ii, jj, nz - 1 - ff);
                    }
                }
            }
            for kk in 0..nz {
                for jj in 0..ny {
                    for ff in 0..xf {
                        zero(ff, jj, kk);
                        zero(nx - 1 - ff, jj, kk);
                    }
                }
            }
            for kk in 0..nz {
                for ii in 0..nx {
                    for ff in 0..yf {
                        zero(ii, ff, kk);
                        zero(ii, ny - 1 - ff, kk);
                    }
                }
            }
        }

        // wtrim: tighten the weight box when that saves at least 10 %.
        let mut trim = None;
        if self.wtrim {
            let [xa, xb, ya, yb, za, zb] = autobbox(&imww, n);
            if xa >= 0 && (xb > 0 || yb > 0 || zb > 0) {
                let fnxyz = (nx * ny * nz) as f32;
                let nttt = ((xb - xa + 1) * (yb - ya + 1) * (zb - za + 1)) as f32;
                let trat = (100.0 * f64::from(nttt) / f64::from(fnxyz)) as f32;
                if self.verbose {
                    raw(
                        format!(
                            " + wtrim: [{xa}..{xb}]x[{ya}..{yb}]x[{za}..{zb}] = {} voxels kept, \
                             out of {} ({:.1}%)\n",
                            nttt as i64, fnxyz as i64, trat
                        ),
                        log,
                    );
                }
                if trat < 90.0 {
                    let bx = Box3 {
                        a: [xa as usize, ya as usize, za as usize],
                        b: [xb as usize, yb as usize, zb as usize],
                    };
                    imww = cut(&imww, n, &bx);
                    trim = Some(bx);
                } else {
                    raw(" + skipping use of trim - too little savings\n".to_owned(), log);
                }
            } else {
                raw(
                    format!(
                        " + skipping empty trim, {{xyz}}{{a,b}} = {xa},{xb}, {ya},{yb}, {za},{zb}\n"
                    ),
                    log,
                );
            }
        }
        if self.verbose {
            raw(
                format!("  weight range: min={}  max={}\n", g(mri_min(&imww)), g(mri_max(&imww))),
                log,
            );
        }
        {
            let nzer = imww.iter().filter(|&&x| x == 0.0).count();
            if nzer == imww.len() {
                raw("** error: weights are all zero\n".to_owned(), log);
            }
            if self.verbose {
                let perc = (100.0 * nzer as f64 / imww.len() as f64) as f32;
                raw(
                    format!(" # zero weights={nzer} out of {} ({perc:.1}%)\n", imww.len()),
                    log,
                );
            }
        }

        let tr = |v: Vec<f32>| match &trim {
            Some(bx) => cut(&v, n, bx),
            None => v,
        };
        let mut fitim: Vec<Vec<f32>> = Vec::with_capacity(7);
        fitim.push(tr(cim.clone()));
        rot.method = self.regmode;
        if self.verbose {
            let nvox = fitim[0].len();
            raw(
                format!(
                    "  mri_3dalign: using {} Mbytes of workspace\n",
                    (10 * nvox * 4) / (1024 * 1024)
                ),
                log,
            );
        }
        let names = ["d/d(th1)", "d/d(th2)", "d/d(th3)", "d/dx", "d/dy", "d/dz"];
        for (q, name) in names.iter().enumerate() {
            if self.verbose {
                raw(format!("  initializing {name}"), log);
            }
            let (delta, scale) = if q < 3 {
                let delta = (2.0 * f64::from(self.delfac) / (nx + ny + nz) as f64) as f32;
                (delta, (f64::from(self.sinit) * 0.5 * DFAC / f64::from(delta)) as f32)
            } else {
                let delta = self.delfac * d[q - 3];
                (delta, (f64::from(self.sinit) * 0.5 / f64::from(delta)) as f32)
            };
            if self.verbose {
                raw(format!("[delta={}]", g(f64::from(delta))), log);
            }
            let mut th = [0.0f32; 3];
            let mut sh = [0.0f32; 3];
            if q < 3 {
                th[q] = delta;
            } else {
                sh[q - 3] = delta;
            }
            let pim = rot.rota3d(&cim, n, d, self.ax, th, self.dcode, sh);
            if self.verbose {
                raw(":".to_owned(), log);
            }
            let th_m = th.map(|x| -x);
            let sh_m = sh.map(|x| -x);
            let mim = rot.rota3d(&cim, n, d, self.ax, th_m, self.dcode, sh_m);
            if self.verbose {
                raw(":".to_owned(), log);
            }
            for m in rot.messages.drain(..) {
                log.push(Message::Raw(m));
            }
            let dim: Vec<f32> = mim.iter().zip(&pim).map(|(&m, &p)| scale * (m - p)).collect();
            let dim = tr(dim);
            if self.verbose {
                raw(
                    format!("  {name} range: min={}  max={}\n", g(mri_min(&dim)), g(mri_max(&dim))),
                    log,
                );
            }
            fitim.push(dim);
        }
        let _ = nxyz;
        if self.verbose {
            raw("  initializing least squares\n".to_owned(), log);
        }
        let fit = lsqfit::startup(fitim, Some(&imww));
        if let Err(e) = &fit {
            for m in &e.messages {
                log.push(if m.starts_with("mri_startup_lsqfit:") {
                    Message::Error(m.clone())
                } else {
                    Message::Error(m.clone())
                });
            }
        }
        Basis { fit, trim, n }
    }

    /// `mri_3dalign_one(basis, im, ...)`: the parameters `[th1, th2, th3]` (radians) and
    /// `[dx, dy, dz]` (mm), and the registered volume (`None` with `noreg`). An error where AFNI
    /// dereferences the failed least squares (AFNI-89).
    pub fn one(
        &self,
        basis: &Basis,
        im: &Vol,
        rot: &mut Rotator,
        log: &mut Vec<Message>,
    ) -> Result<([f32; 3], [f32; 3], Option<Vec<f32>>), LsqError> {
        let fitter = match &basis.fit {
            Ok(s) => s,
            Err(e) => return Err(e.clone()),
        };
        let fim = &im.data;
        let n = im.n;
        let trim = |v: &[f32]| match &basis.trim {
            Some(bx) => cut(v, n, bx),
            None => v.to_vec(),
        };
        rot.method = self.regmode;
        let thr = |d: f32| {
            if d != 0.0 {
                (f64::from(d).abs() * f64::from(self.dxy_thresh)) as f32
            } else {
                self.dxy_thresh
            }
        };
        let (dxt, dyt, dzt) = (thr(im.d[0]), thr(im.d[1]), thr(im.d[2]));
        let phi = f64::from(self.phi_thresh);
        let mut fit: Vec<f32>;
        let mut good;
        if self.nonzero_init() {
            fit = vec![1.0];
            fit.extend_from_slice(&self.init);
            good = true;
        } else {
            fit = fitter.solve(&trim(fim))?;
            let a = |i: usize| 10.0 * f64::from(fit[i]).abs();
            good = a(4) > f64::from(dxt)
                || a(5) > f64::from(dyt)
                || a(6) > f64::from(dzt)
                || a(1) > phi
                || a(2) > phi
                || a(3) > phi;
        }
        if self.verbose {
            log.push(Message::Raw(format!(
                "\nFirst fit: {} {} {} {} {} {} {}\n",
                g13(f64::from(fit[0])),
                g13(f64::from(fit[1])),
                g13(f64::from(fit[2])),
                g13(f64::from(fit[3])),
                g13(f64::from(fit[4])),
                g13(f64::from(fit[5])),
                g13(f64::from(fit[6]))
            )));
        }
        let angles = |fit: &[f32]| {
            [
                (f64::from(fit[1]) * DFAC) as f32,
                (f64::from(fit[2]) * DFAC) as f32,
                (f64::from(fit[3]) * DFAC) as f32,
            ]
        };
        let mut iter = 0;
        while good {
            let tim = rot.rota3d(fim, n, im.d, self.ax, angles(&fit), self.dcode, [fit[4], fit[5], fit[6]]);
            for m in rot.messages.drain(..) {
                log.push(Message::Raw(m));
            }
            let dfit = fitter.solve(&trim(&tim))?;
            for i in 1..7 {
                fit[i] += dfit[i];
            }
            if self.verbose {
                let row = |label: &str, v0: f32, v: &[f32]| {
                    format!(
                        "{label} fit: {} {} {} {} {} {} {}\n",
                        g13(f64::from(v0)),
                        g13(f64::from(v[1])),
                        g13(f64::from(v[2])),
                        g13(f64::from(v[3])),
                        g13(f64::from(v[4])),
                        g13(f64::from(v[5])),
                        g13(f64::from(v[6]))
                    )
                };
                log.push(Message::Raw(row("Delta", dfit[0], &dfit)));
                log.push(Message::Raw(row("Total", dfit[0], &fit)));
            }
            iter += 1;
            let a = |i: usize| f64::from(dfit[i]).abs();
            good = iter < self.max_iter
                && (a(4) > f64::from(dxt)
                    || a(5) > f64::from(dyt)
                    || a(6) > f64::from(dzt)
                    || a(1) > phi
                    || a(2) > phi
                    || a(3) > phi);
        }
        if self.verbose {
            log.push(Message::Raw(format!("Iteration complete at {iter} steps\n")));
        }
        let th = angles(&fit);
        let sh = [fit[4], fit[5], fit[6]];
        let mut tim = None;
        let final_mode = self.final_regmode.unwrap_or(self.regmode);
        if !self.noreg {
            rot.method = final_mode;
            let mut out = rot.rota3d(fim, n, im.d, self.ax, th, self.dcode, sh);
            for m in rot.messages.drain(..) {
                log.push(Message::Raw(m));
            }
            if self.clipit && clips(final_mode) {
                let ftop = mri_max(fim) as f32;
                let fbot = mri_min(fim) as f32;
                for x in &mut out {
                    if *x < fbot {
                        *x = fbot;
                    } else if *x > ftop {
                        *x = ftop;
                    }
                }
            }
            tim = Some(out);
        }
        Ok((th, sh, tim))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounding_box_and_cut() {
        let n = [10, 9, 8];
        let at = |i: usize, j: usize, k: usize| i + j * 10 + k * 90;
        let mut v = vec![0.0f32; 720];
        for k in 1..7 {
            for j in 2..8 {
                for i in 3..9 {
                    v[at(i, j, k)] = 1.0;
                }
            }
        }
        // A stray voxel outside the main cluster is ignored.
        v[at(0, 0, 0)] = 5.0;
        assert_eq!(autobbox(&v, n), [3, 8, 2, 7, 1, 6]);
        let bx = Box3 {
            a: [3, 2, 1],
            b: [8, 7, 6],
        };
        let c = cut(&v, n, &bx);
        assert_eq!(c.len(), 216);
        assert!(c.iter().all(|&x| x == 1.0));
        // A cluster the erosion removes entirely: an empty box (n and -1).
        let mut w = vec![0.0f32; 720];
        w[at(4, 4, 4)] = 1.0;
        assert_eq!(autobbox(&w, n), [10, -1, 9, -1, 8, -1]);
        assert_eq!(autobbox(&[0.0; 8], [2, 2, 2]), [0; 6]);
    }
}
