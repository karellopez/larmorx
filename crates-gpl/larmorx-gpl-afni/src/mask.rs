// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09: src/thd_automask.c (THD_automask_* settings,
// mask_count, THD_automask, mri_automask_image, mri_automask_image2D, THD_mask_clip_neighbors,
// THD_mask_fillin_once, THD_mask_fillin_completely, THD_mask_clust, THD_mask_clust2D,
// THD_mask_erode, THD_mask_erodemany, THD_mask_erodemany2D, THD_mask_dilate, THD_mask_depth,
// thd_apply_mask), src/thd_cliplevel.c (THD_cliplevel, THD_cliplevel_partial,
// get_octant_clips, pointclip, THD_cliplevel_gradual), src/thd_median.c (THD_aveabs_brick),
// src/mri_get_cmass.c (mri_get_cmass_3D), src/mri_cut.c (mri_cut_3D), src/mri_max.c (mri_max),
// src/thd_makemask.c (THD_countmask), src/mri_to_float.c (mri_to_float, mri_scale_to_float)
// and src/thd_dsetto1D.c (THD_extract_array).
// thd_automask.c, thd_cliplevel.c, thd_median.c and mri_get_cmass.c carry no Medical College
// of Wisconsin notice: they were written at the NIH and are in the public domain. mri_cut.c,
// mri_max.c, thd_makemask.c, mri_to_float.c and thd_dsetto1D.c: Major portions Copyright (C)
// 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.), released under the GNU
// GPL version 2 or any later version.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! AFNI's automask: the clip level, the brain mask of `3dAutomask` and the mask steps
//! `3dUnifize` uses, step by step as AFNI 25.2.09 computes them.
//!
//! - [`cliplevel`] (`THD_cliplevel`): a histogram of 10,000 bins up to the image's maximum, and
//!   the cut where `clfrac` times the median of the values above it equals the cut;
//!   [`cliplevel_gradual`]: the same in each octant around the centre of mass, interpolated
//!   trilinearly between the octants' centres.
//! - [`automask_image`] (`mri_automask_image`): values at or above the (gradual) clip level,
//!   the largest 6-connected cluster, peeled and restored ([`erodemany`]), small and large
//!   holes filled ([`fillin_once`]), and holes inside the brain filled by clustering the
//!   outside; with the exterior clip, voxels below the clip level next to the outside are
//!   removed. A 2D image (`nz = 1`) has its own path.
//! - The morphology of `3dAutomask`'s options: [`dilate`], [`erode`] (`-NN1/2/3`), [`depth`].
//!
//! AFNI keeps the settings in static variables of `thd_automask.c` (`THD_automask_set_*`);
//! here they are [`MaskSettings`], passed explicitly. The progress messages AFNI prints when its
//! `verb` is set (`ININFO_message`) are appended to a list of [`Message`]s.
//!
//! The results do not depend on the order in which AFNI's cluster search visits voxels, only on
//! the components it finds: [`clust`] labels 6-connected components and keeps the first
//! largest one in raster order of its first voxel, as `THD_mask_clust` does.

use crate::message::Message;

/// The settings `thd_automask.c` keeps in static variables.
#[derive(Clone, Debug, PartialEq)]
pub struct MaskSettings {
    /// `verb` (`THD_automask_verbose`): print the progress messages.
    pub verbose: bool,
    /// `exterior_clip` (`THD_automask_extclip`, 3dAutomask's `-eclip`).
    pub exterior_clip: bool,
    /// `clfrac` (`THD_automask_set_clipfrac`).
    pub clfrac: f32,
    /// `peelcount` (`THD_automask_set_peelcounts`).
    pub peelcount: i32,
    /// `peelthr` (`THD_automask_set_peelcounts`).
    pub peelthr: i32,
    /// `gradualize` (`THD_automask_set_gradualize`; `-nograd` turns it off).
    pub gradualize: bool,
}

impl Default for MaskSettings {
    fn default() -> MaskSettings {
        MaskSettings {
            verbose: false,
            exterior_clip: false,
            clfrac: 0.5,
            peelcount: 1,
            peelthr: 17,
            gradualize: true,
        }
    }
}

impl MaskSettings {
    /// `THD_automask_set_clipfrac`: values outside [0.1, 0.9] give 0.5.
    pub fn set_clipfrac(&mut self, f: f32) {
        self.clfrac = if (0.1..=0.9).contains(&f) { f } else { 0.5 };
    }

    /// `THD_automask_set_peelcounts`: a non-positive count gives 1, a threshold outside
    /// [6, 26] gives 17.
    pub fn set_peelcounts(&mut self, p: i32, t: i32) {
        self.peelcount = if p > 0 { p } else { 1 };
        self.peelthr = if (6..=26).contains(&t) { t } else { 17 };
    }
}

/// A 3D grid: `nx`, `ny`, `nz` (x fastest).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
}

impl Grid {
    pub fn new(nx: usize, ny: usize, nz: usize) -> Grid {
        Grid { nx, ny, nz }
    }

    pub fn nvox(&self) -> usize {
        self.nx * self.ny * self.nz
    }

    fn nxy(&self) -> usize {
        self.nx * self.ny
    }
}

/// `mask_count`, `THD_countmask`: the number of non-zero voxels.
pub fn count(mask: &[u8]) -> i32 {
    mask.iter().filter(|&&m| m != 0).count() as i32
}

/// `mri_max` of a float image: the largest value, starting from -1e38 (`MAX(a,b)` is
/// `a < b ? b : a`), as a double.
pub fn mri_max(im: &[f32]) -> f64 {
    let mut m = -1.0e38f32;
    for &v in im {
        if m < v {
            m = v;
        }
    }
    f64::from(m)
}

/// `THD_cliplevel` of a float image: the clip level for `mfrac` (0.5 when it is not in
/// (0, 0.99)). 0 when the image has no positive maximum or at most 222 positive values.
pub fn cliplevel(im: &[f32], mfrac: f32) -> f32 {
    let mfrac = if mfrac <= 0.0 || mfrac >= 0.99 {
        0.50f32
    } else {
        mfrac
    };
    const NHIST: i32 = 10000;
    let mut hist = vec![0i32; NHIST as usize + 1];
    // The float->int conversion inline (20 Dec 2006).
    let fac = mri_max(im);
    if fac < 1.0e-100 {
        return 0.0;
    }
    let sfac = f64::from(NHIST) / fac;
    let mut dsum = 0.0f64;
    let mut npos: i32 = 0;
    for &v in im {
        if v > 0.0 {
            let kk = (sfac * f64::from(v) + 0.499) as i32;
            if kk <= NHIST {
                hist[kk as usize] += 1;
                dsum += f64::from(kk) * f64::from(kk);
                npos += 1;
            }
        }
    }
    if npos <= 222 {
        return 0.0;
    }
    // Start the cut where the upper 65% of the positive values begin.
    let qq = (0.65f32 * npos as f32) as i32;
    let ib = (0.5 * (dsum / f64::from(npos)).sqrt()).round_ties_even() as i32;
    let h = |i: i32| -> i32 {
        // An index below 0 (only with more than 100 million positive voxels, AFNI reads before
        // the histogram) counts as an empty bin.
        if i >= 0 { hist[i as usize] } else { 0 }
    };
    let mut kk = 0i32;
    let mut ii = NHIST - 1;
    while ii >= ib && kk < qq {
        kk += h(ii);
        ii -= 1;
    }
    // The cut that is mfrac times the median of the values above it.
    let mut ncut = ii;
    let mut iterations = 0;
    loop {
        let mut npos = 0i32;
        for i in ncut..NHIST {
            npos += h(i);
        }
        let nhalf = npos / 2;
        let mut kk = 0i32;
        let mut ii = ncut;
        while ii < NHIST && kk < nhalf {
            kk += h(ii);
            ii += 1;
        }
        let nold = ncut;
        ncut = (mfrac * ii as f32) as i32;
        iterations += 1;
        if !(iterations < 66 && ncut != nold) {
            break;
        }
    }
    let mut fac = f64::from(ncut) / sfac;
    if fac > 1.0e38 {
        fac = 1.0e38;
    }
    fac as f32
}

/// `mri_get_cmass_3D`: the centre of mass of |values|, in voxel indices (the grid's centre
/// when every value is 0).
fn cmass(im: &[f32], g: Grid) -> [f32; 3] {
    let (mut sum, mut xx, mut yy, mut zz) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut v = 0;
    for kk in 0..g.nz {
        for jj in 0..g.ny {
            for ii in 0..g.nx {
                let val = f64::from(im[v]).abs();
                sum += val;
                xx += val * ii as f64;
                yy += val * jj as f64;
                zz += val * kk as f64;
                v += 1;
            }
        }
    }
    if sum > 0.0 {
        xx /= sum;
        yy /= sum;
        zz /= sum;
    } else {
        xx = 0.5 * (g.nx as f64 - 1.0);
        yy = 0.5 * (g.ny as f64 - 1.0);
        zz = 0.5 * (g.nz as f64 - 1.0);
    }
    [xx as f32, yy as f32, zz as f32]
}

/// `THD_cliplevel_partial` (`mri_cut_3D`, then `THD_cliplevel`) on the box `[a, b]` of each
/// axis (clipped to the grid).
fn cliplevel_partial(im: &[f32], g: Grid, mfrac: f32, a: [i64; 3], b: [i64; 3]) -> f32 {
    let n = [g.nx as i64, g.ny as i64, g.nz as i64];
    let lo: Vec<i64> = a.iter().map(|&x| x.max(0)).collect();
    let hi: Vec<i64> = (0..3).map(|d| b[d].min(n[d] - 1)).collect();
    if (0..3).any(|d| hi[d] < lo[d]) {
        // mri_cut_3D returns NULL, and THD_cliplevel of NULL is 0.
        return 0.0;
    }
    let mut cut = Vec::with_capacity(
        ((hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1) * (hi[2] - lo[2] + 1)) as usize,
    );
    for zz in lo[2]..=hi[2] {
        for yy in lo[1]..=hi[1] {
            let row = (zz as usize * g.ny + yy as usize) * g.nx;
            cut.extend_from_slice(&im[row + lo[0] as usize..=row + hi[0] as usize]);
        }
    }
    cliplevel(&cut, mfrac)
}

/// `clipvec`: the octants' clip levels and the box between the octants' centres.
struct ClipVec {
    clip: [f32; 8], // 000, 100, 010, 110, 001, 101, 011, 111
    x0: f32,
    y0: f32,
    z0: f32,
    dxi: f32,
    dyi: f32,
    dzi: f32,
}

/// `get_octant_clips`.
fn octant_clips(im: &[f32], g: Grid, mfrac: f32) -> ClipVec {
    let (it, jt, kt) = (g.nx as i64 - 1, g.ny as i64 - 1, g.nz as i64 - 1);
    let [xcm, ycm, zcm] = cmass(im, g);
    let ic = f64::from(xcm).round_ties_even() as i64;
    let jc = f64::from(ycm).round_ties_even() as i64;
    let kc = f64::from(zcm).round_ties_even() as i64;
    let val = 0.333f32 * cliplevel(im, mfrac);
    let overlap = |n: usize| ((0.01 * n as f64).round_ties_even() as i64).max(1);
    let (ii, jj, kk) = (overlap(g.nx), overlap(g.ny), overlap(g.nz));
    let (icm, icp) = ((ic - ii).max(0), (ic + ii).min(it));
    let (jcm, jcp) = ((jc - jj).max(0), (jc + jj).min(jt));
    let (kcm, kcp) = ((kc - kk).max(0), (kc + kk).min(kt));
    let part = |a: [i64; 3], b: [i64; 3]| cliplevel_partial(im, g, mfrac, a, b);
    let mut clip = [
        part([0, 0, 0], [icp, jcp, kcp]),
        part([icm, 0, 0], [it, jcp, kcp]),
        part([0, jcm, 0], [icp, jt, kcp]),
        part([icm, jcm, 0], [it, jt, kcp]),
        part([0, 0, kcm], [icp, jcp, kt]),
        part([icm, 0, kcm], [it, jcp, kt]),
        part([0, jcm, kcm], [icp, jt, kt]),
        part([icm, jcm, kcm], [it, jt, kt]),
    ];
    for c in &mut clip {
        if *c < val {
            *c = val;
        }
    }
    // (x0,y0,z0): the centre of the lowest octant; (x1,y1,z1): of the highest.
    let x0 = (0.5 * ic as f64) as f32;
    let x1 = (0.5 * (ic + it) as f64) as f32;
    let y0 = (0.5 * jc as f64) as f32;
    let y1 = (0.5 * (jc + jt) as f64) as f32;
    let z0 = (0.5 * kc as f64) as f32;
    let z1 = (0.5 * (kc + kt) as f64) as f32;
    let inv = |a: f32, b: f32| {
        if b > a {
            (1.0 / f64::from(b - a)) as f32
        } else {
            0.0
        }
    };
    ClipVec {
        clip,
        x0,
        y0,
        z0,
        dxi: inv(x0, x1),
        dyi: inv(y0, y1),
        dzi: inv(z0, z1),
    }
}

/// `pointclip`: the clip level at voxel `(ii, jj, kk)`, trilinear between the octants' centres.
fn pointclip(ii: usize, jj: usize, kk: usize, cv: &ClipVec) -> f32 {
    // AFNI's `if( x < 0 ) x = 0 ; else if( x > 1 ) x = 1`: the same as `clamp` for every
    // value, NaN and -0 included.
    let clamp = |v: f32| v.clamp(0.0, 1.0);
    let x1 = clamp((ii as f32 - cv.x0) * cv.dxi);
    let y1 = clamp((jj as f32 - cv.y0) * cv.dyi);
    let z1 = clamp((kk as f32 - cv.z0) * cv.dzi);
    let (x0, y0, z0) = (1.0f32 - x1, 1.0f32 - y1, 1.0f32 - z1);
    let c = &cv.clip;
    c[0] * x0 * y0 * z0
        + c[1] * x1 * y0 * z0
        + c[2] * x0 * y1 * z0
        + c[3] * x1 * y1 * z0
        + c[4] * x0 * y0 * z1
        + c[5] * x1 * y0 * z1
        + c[6] * x0 * y1 * z1
        + c[7] * x1 * y1 * z1
}

/// `THD_cliplevel_gradual`: the clip level of every voxel.
pub fn cliplevel_gradual(im: &[f32], g: Grid, mfrac: f32) -> Vec<f32> {
    let cv = octant_clips(im, g, mfrac);
    let mut out = Vec::with_capacity(g.nvox());
    for kk in 0..g.nz {
        for jj in 0..g.ny {
            for ii in 0..g.nx {
                out.push(pointclip(ii, jj, kk, &cv));
            }
        }
    }
    out
}

/// `ININFO_message` when the settings say so.
fn say(s: &MaskSettings, log: &mut Vec<Message>, text: impl FnOnce() -> String) {
    if s.verbose {
        log.push(Message::InInfo(text()));
    }
}

/// `%f` of a float.
fn pf(x: f32) -> String {
    format!("{:.6}", f64::from(x))
}

/// `mri_automask_image`: the brain mask of a float image. `None` where AFNI returns `NULL`
/// (a 2D image smaller than 16 x 16, or with no voxel above its clip level).
pub fn automask_image(
    im: &[f32],
    g: Grid,
    s: &MaskSettings,
    log: &mut Vec<Message>,
) -> Option<Vec<u8>> {
    if g.nz == 1 {
        return automask_image_2d(im, g.nx, g.ny, s);
    }
    let mar = im;
    let nvox = g.nvox();
    let clip_val = cliplevel(mar, s.clfrac);
    say(s, log, || format!("Fixed clip level = {}\n", pf(clip_val)));
    let mut mmm = vec![0u8; nvox];
    let mut nmm = 0i32;
    if !s.gradualize {
        for (m, &v) in mmm.iter_mut().zip(mar) {
            if v >= clip_val {
                *m = 1;
                nmm += 1;
            }
        }
    } else {
        let car = cliplevel_gradual(mar, g, s.clfrac);
        let (mut cbot, mut ctop) = (1.0e38f32, -1.0e38f32);
        for ii in 0..nvox {
            if mar[ii] >= car[ii] {
                mmm[ii] = 1;
                nmm += 1;
            }
            if car[ii] < cbot {
                cbot = car[ii];
            }
            if car[ii] > ctop {
                ctop = car[ii];
            }
        }
        say(s, log, || {
            format!("Used gradual clip level = {} .. {}", pf(cbot), pf(ctop))
        });
    }
    say(s, log, || {
        format!("Number voxels above clip level = {nmm}\n")
    });
    if nmm == 0 {
        return Some(mmm); // should not happen
    }
    // No volume data: stop here.
    if g.nx < 2 || g.ny < 2 || g.nz < 2 {
        return Some(mmm);
    }

    // Only keep the largest connected component.
    clust(g, &mut mmm, s, log);
    // Erode the result (to break off thinly attached pieces), then recluster.
    erodemany(g, &mut mmm, s.peelcount, s);
    clust(g, &mut mmm, s, log);

    // Fill in small holes.
    let mut ii = fillin_once(g, &mut mmm, 1);
    let mut jj = ii;
    if ii > 0 {
        ii = fillin_once(g, &mut mmm, 1);
        jj += ii;
        if ii > 0 {
            ii = fillin_once(g, &mut mmm, 1);
            jj += ii;
        }
    }
    if jj > 0 {
        say(s, log, || {
            format!(
                "Filled {jj:5} voxels in small holes; now have {} voxels\n",
                count(&mmm)
            )
        });
    }

    let rint = |x: f64| x.round_ties_even() as i32;
    let mut nmm = 1;
    let mut jj = rint(0.016 * g.nx as f64);
    nmm = nmm.max(jj);
    jj = rint(0.016 * g.ny as f64);
    nmm = nmm.max(jj);
    jj = rint(0.016 * g.nz as f64);
    nmm = nmm.max(jj);
    // `jj` is now the z term: AFNI tests it, not the largest.
    if nmm > 1 || jj > 0 {
        let mut jj = 0;
        for side in 2..nmm {
            jj += fillin_once(g, &mut mmm, side);
        }
        jj += fillin_completely(g, &mut mmm, nmm);
        if jj > 0 {
            say(s, log, || {
                format!(
                    "Filled {jj:5} voxels in large holes; now have {} voxels\n",
                    count(&mmm)
                )
            });
        }
    }

    erodemany(g, &mut mmm, 1, s);
    clust(g, &mut mmm, s, log);

    // Invert, keep the largest cluster (the outside of the brain), invert again: this fills
    // any holes left inside the brain.
    for m in mmm.iter_mut() {
        *m = u8::from(*m == 0);
    }
    say(s, log, || "Clustering non-brain voxels ...\n".to_owned());
    clust(g, &mut mmm, s, log);

    // The mask is now 1 outside the brain: add the voxels next to it that are below the clip
    // level.
    let jj = if s.exterior_clip {
        let tclip = (9999.9 * f64::from(clip_val)) as f32;
        let jj = clip_neighbors(g, &mut mmm, clip_val, tclip, mar);
        if jj > 0 {
            say(s, log, || {
                format!("Removed  {jj} exterior voxels below clip level\n")
            });
        }
        jj
    } else {
        0
    };

    for m in mmm.iter_mut() {
        *m = u8::from(*m == 0);
    }
    say(s, log, || format!("Mask now has {} voxels\n", count(&mmm)));

    if s.exterior_clip && jj > 0 {
        erodemany(g, &mut mmm, 1, s);
        clust(g, &mut mmm, s, log);
    }
    Some(mmm)
}

/// `THD_mask_clip_neighbors`: interior voxels not in the mask whose value is below `clip_val`
/// (or above `tclip`) and that have a face neighbour in the mask are added, in raster order and
/// in place (a voxel added counts for the next ones), until a sweep adds none. Returns the
/// number added.
fn clip_neighbors(g: Grid, mmm: &mut [u8], clip_val: f32, tclip: f32, mar: &[f32]) -> i32 {
    let (nx, ny, nz, nxy) = (g.nx, g.ny, g.nz, g.nxy());
    let mut ntot = 0;
    loop {
        let mut nnew = 0;
        for kk in 1..nz.saturating_sub(1) {
            let k3 = kk * nxy;
            for jj in 1..ny.saturating_sub(1) {
                let j3 = k3 + jj * nx;
                for ii in 1..nx.saturating_sub(1) {
                    let i3 = ii + j3;
                    if mmm[i3] != 0 || (mar[i3] >= clip_val && mar[i3] <= tclip) {
                        continue;
                    }
                    if mmm[i3 - 1] != 0
                        || mmm[i3 + 1] != 0
                        || mmm[i3 - nx] != 0
                        || mmm[i3 + nx] != 0
                        || mmm[i3 - nxy] != 0
                        || mmm[i3 + nxy] != 0
                    {
                        mmm[i3] = 1;
                        nnew += 1;
                    }
                }
            }
        }
        ntot += nnew;
        if nnew == 0 {
            return ntot;
        }
    }
}

/// `THD_mask_fillin_once`: a voxel not in the mask, at least `nside` voxels from the edges
/// along each axis that has room (`(n-1)/2`, at most `nside`), is filled when along x, y or z
/// there is a mask voxel within that distance on both sides. Returns the number filled.
pub fn fillin_once(g: Grid, mmm: &mut [u8], nside: i32) -> i32 {
    if nside <= 0 {
        return 0;
    }
    let side = |n: usize| (((n as i64 - 1) / 2).min(i64::from(nside))).max(0) as usize;
    let (nsx, nsy, nsz) = (side(g.nx), side(g.ny), side(g.nz));
    if nsx == 0 && nsy == 0 && nsz == 0 {
        return 0;
    }
    let (nx, ny, nz, nxy) = (g.nx, g.ny, g.nz, g.nxy());
    let mut nnn = vec![0u8; g.nvox()];
    let mut nfill = 0;
    // Whether a mask voxel lies within `ns` steps of `stride` on both sides of `iv`.
    let both = |mmm: &[u8], iv: usize, stride: usize, ns: usize| {
        ns > 0
            && (1..=ns).any(|l| mmm[iv + l * stride] != 0)
            && (1..=ns).any(|l| mmm[iv - l * stride] != 0)
    };
    for kk in nsz..nz.saturating_sub(nsz) {
        for jj in nsy..ny.saturating_sub(nsy) {
            for ii in nsx..nx.saturating_sub(nsx) {
                let iv = ii + jj * nx + kk * nxy;
                if mmm[iv] != 0 {
                    continue;
                }
                if both(mmm, iv, 1, nsx) || both(mmm, iv, nx, nsy) || both(mmm, iv, nxy, nsz) {
                    nnn[iv] = 1;
                    nfill += 1;
                }
            }
        }
    }
    if nfill > 0 {
        for (m, &n) in mmm.iter_mut().zip(&nnn) {
            if n != 0 {
                *m = 1;
            }
        }
    }
    nfill
}

/// `THD_mask_fillin_completely`: [`fillin_once`] until it fills nothing. Returns the number
/// filled.
pub fn fillin_completely(g: Grid, mmm: &mut [u8], nside: i32) -> i32 {
    let mut nfill = 0;
    loop {
        let k = fillin_once(g, mmm, nside);
        nfill += k;
        if k == 0 {
            return nfill;
        }
    }
}

/// `THD_mask_clust`: keeps the largest 6-connected cluster of non-zero voxels (the first of
/// the largest in raster order of their first voxel), as 1s; everything else becomes 0.
pub fn clust(g: Grid, mmm: &mut [u8], s: &MaskSettings, log: &mut Vec<Message>) {
    say(s, log, || "Clustering voxels ...".to_owned());
    let (nx, ny, nz, nxy) = (g.nx, g.ny, g.nz, g.nxy());
    let nxyz = g.nvox();
    let mut nbest = 0usize;
    let mut best: Vec<usize> = Vec::new();
    let mut now: Vec<usize> = Vec::new();
    for start in 0..nxyz {
        if mmm[start] == 0 {
            continue;
        }
        mmm[start] = 0;
        now.clear();
        now.push(start);
        let mut icl = 0;
        while icl < now.len() {
            let ijk = now[icl];
            let ii = ijk % nx;
            let jj = (ijk / nx) % ny;
            let kk = ijk / nxy;
            let mut put = |q: usize, mmm: &mut [u8]| {
                if mmm[q] != 0 {
                    mmm[q] = 0;
                    now.push(q);
                }
            };
            if ii > 0 {
                put(ijk - 1, mmm);
            }
            if ii + 1 < nx {
                put(ijk + 1, mmm);
            }
            if jj > 0 {
                put(ijk - nx, mmm);
            }
            if jj + 1 < ny {
                put(ijk + nx, mmm);
            }
            if kk > 0 {
                put(ijk - nxy, mmm);
            }
            if kk + 1 < nz {
                put(ijk + nxy, mmm);
            }
            icl += 1;
        }
        if now.len() > nbest {
            nbest = now.len();
            std::mem::swap(&mut best, &mut now);
        }
    }
    for &v in &best {
        mmm[v] = 1;
    }
    say(s, log, || format!("Largest cluster has {nbest} voxels\n"));
}

/// The 18 neighbours `THD_mask_erodemany` counts, with each axis's minus and plus offsets
/// replaced by the voxel itself at the grid's edges (planar duplication).
#[inline]
#[allow(clippy::too_many_arguments)]
fn count18(
    mmm: &[u8],
    ii: usize,
    im: usize,
    ip: usize,
    jy: usize,
    jm: usize,
    jp: usize,
    kz: usize,
    km: usize,
    kp: usize,
) -> i32 {
    let m = |i: usize| i32::from(mmm[i]);
    m(im + jy + km)
        + m(ii + jm + km)
        + m(ii + jy + km)
        + m(ii + jp + km)
        + m(ip + jy + km)
        + m(im + jm + kz)
        + m(im + jy + kz)
        + m(im + jp + kz)
        + m(ii + jm + kz)
        + m(ii + jp + kz)
        + m(ip + jm + kz)
        + m(ip + jy + kz)
        + m(ip + jp + kz)
        + m(im + jy + kp)
        + m(ii + jm + kp)
        + m(ii + jy + kp)
        + m(ii + jp + kp)
        + m(ip + jy + kp)
}

/// The edge-duplicated offsets of a row, slice or voxel: `(own, minus, plus)`.
#[inline]
fn offsets(i: usize, n: usize, step: usize) -> (usize, usize, usize) {
    let own = i * step;
    let minus = if i == 0 { own } else { own - step };
    let plus = if i + 1 == n { own } else { own + step };
    (own, minus, plus)
}

/// `THD_mask_erodemany`: peels `npeel` layers (a voxel with fewer than `min(18, peelthr)` of
/// its 18 neighbours in the mask is removed, layer by layer), then restores, from the innermost
/// layer out, the peeled voxels next to survivors (the innermost layer: any survivor; the
/// others: at least two). Nothing for fewer than 27 voxels.
pub fn erodemany(g: Grid, mmm: &mut [u8], npeel: i32, s: &MaskSettings) {
    let nxyz = g.nvox();
    if npeel < 1 || nxyz < 27 {
        return;
    }
    let (nx, ny, nz, nxy) = (g.nx, g.ny, g.nz, g.nxy());
    // Until NN1 and NN3 are allowed here, NN2 at most.
    let realpeelthr = 18.min(s.peelthr);
    let mut nnn = vec![0u8; nxyz];
    let mut qqq = vec![0u8; nxyz];
    for pp in 1..=npeel {
        let bpp = pp as u8;
        for kk in 0..nz {
            let (kz, km, kp) = offsets(kk, nz, nxy);
            for jj in 0..ny {
                let (jy, jm, jp) = offsets(jj, ny, nx);
                for ii in 0..nx {
                    if mmm[ii + jy + kz] != 0 {
                        let (_, im, ip) = offsets(ii, nx, 1);
                        let num = count18(mmm, ii, im, ip, jy, jm, jp, kz, km, kp);
                        if num < realpeelthr {
                            nnn[ii + jy + kz] = bpp;
                        }
                    }
                }
            }
        }
        for (m, &n) in mmm.iter_mut().zip(&nnn) {
            if n != 0 {
                *m = 0;
            }
        }
    }
    // Re-dilate eroded voxels that are next to survivors, from the innermost peel out.
    for pp in (1..=npeel).rev() {
        let bpp = pp as u8;
        qqq.iter_mut().for_each(|q| *q = 0);
        let bth: u8 = if pp == npeel { 0 } else { 1 };
        for kk in 0..nz {
            let (kz, km, kp) = offsets(kk, nz, nxy);
            for jj in 0..ny {
                let (jy, jm, jp) = offsets(jj, ny, nx);
                for ii in 0..nx {
                    let v = ii + jy + kz;
                    if nnn[v] >= bpp && mmm[v] == 0 {
                        let (_, im, ip) = offsets(ii, nx, 1);
                        qqq[v] = count18(mmm, ii, im, ip, jy, jm, jp, kz, km, kp) as u8;
                    }
                }
            }
        }
        for (m, &q) in mmm.iter_mut().zip(&qqq) {
            if q > bth {
                *m = 1;
            }
        }
    }
}

/// The neighbour counts of `THD_mask_erode` and `THD_mask_dilate` at one voxel, by level:
/// faces (6), edges (12 more), corners (8 more), from the edge-duplicated offsets.
#[derive(Clone, Copy)]
struct Nbhd {
    ii: usize,
    im: usize,
    ip: usize,
    jy: usize,
    jm: usize,
    jp: usize,
    kz: usize,
    km: usize,
    kp: usize,
}

impl Nbhd {
    fn faces(&self, m: &[u8]) -> i32 {
        let f = |i: usize| i32::from(m[i]);
        f(self.ii + self.jy + self.km)
            + f(self.im + self.jy + self.kz)
            + f(self.ii + self.jm + self.kz)
            + f(self.ii + self.jp + self.kz)
            + f(self.ip + self.jy + self.kz)
            + f(self.ii + self.jy + self.kp)
    }

    fn edges(&self, m: &[u8]) -> i32 {
        let f = |i: usize| i32::from(m[i]);
        let Nbhd {
            ii,
            im,
            ip,
            jy,
            jm,
            jp,
            kz,
            km,
            kp,
        } = *self;
        f(im + jy + km)
            + f(ii + jm + km)
            + f(ii + jp + km)
            + f(ip + jy + km)
            + f(im + jm + kz)
            + f(im + jp + kz)
            + f(ip + jm + kz)
            + f(ip + jp + kz)
            + f(im + jy + kp)
            + f(ii + jm + kp)
            + f(ii + jp + kp)
            + f(ip + jy + kp)
    }

    fn corners(&self, m: &[u8]) -> i32 {
        let f = |i: usize| i32::from(m[i]);
        let Nbhd {
            im,
            ip,
            jm,
            jp,
            km,
            kp,
            ..
        } = *self;
        f(im + jm + km)
            + f(im + jp + km)
            + f(ip + jm + km)
            + f(ip + jp + km)
            + f(im + jm + kp)
            + f(im + jp + kp)
            + f(ip + jm + kp)
            + f(ip + jp + kp)
    }
}

/// `THD_mask_erode` with `redilate` 0 (the only way 3dAutomask calls it): removes every mask
/// voxel on the grid's border, and every other mask voxel that lacks one of its neighbours of
/// level `nn` (1: the 6 faces; 2: also the 12 edges; 3: also the 8 corners).
pub fn erode(g: Grid, mmm: &mut [u8], nn: u8, s: &MaskSettings, log: &mut Vec<Message>) {
    let (nx, ny, nz, nxy) = (g.nx, g.ny, g.nz, g.nxy());
    let mut nnn = vec![0u8; g.nvox()];
    for kk in 0..nz {
        let (kz, km, kp) = offsets(kk, nz, nxy);
        for jj in 0..ny {
            let (jy, jm, jp) = offsets(jj, ny, nx);
            for ii in 0..nx {
                if mmm[ii + jy + kz] == 0 {
                    continue;
                }
                if ii == 0 || jj == 0 || kk == 0 || ii == nx - 1 || jj == ny - 1 || kk == nz - 1 {
                    nnn[ii + jy + kz] = 1;
                    continue;
                }
                let n = Nbhd {
                    ii,
                    im: ii - 1,
                    ip: ii + 1,
                    jy,
                    jm,
                    jp,
                    kz,
                    km,
                    kp,
                };
                let mut num = n.faces(mmm);
                let mut victim = num < 6;
                if nn >= 2 && !victim {
                    num += n.edges(mmm);
                    victim = num < 18;
                }
                if nn == 3 && !victim {
                    num += n.corners(mmm);
                    victim = num < 26;
                }
                nnn[ii + jy + kz] = u8::from(victim);
            }
        }
    }
    let mut jj = 0;
    for (m, &n) in mmm.iter_mut().zip(&nnn) {
        if n != 0 {
            *m = 0;
            jj += 1;
        }
    }
    if jj > 0 {
        say(s, log, || format!("Eroded   {jj} voxels\n"));
    }
}

/// `THD_mask_dilate`: adds every voxel outside the mask with at least `ndil` (at least 1)
/// neighbours of level `nn` in it (at the grid's edges the missing neighbours count as the
/// voxel itself). Returns the number added.
pub fn dilate(g: Grid, mmm: &mut [u8], ndil: i32, nn: u8) -> i32 {
    let ndil = ndil.max(1);
    let (nx, ny, nz, nxy) = (g.nx, g.ny, g.nz, g.nxy());
    let mut nnn = vec![0u8; g.nvox()];
    for kk in 0..nz {
        let (kz, km, kp) = offsets(kk, nz, nxy);
        for jj in 0..ny {
            let (jy, jm, jp) = offsets(jj, ny, nx);
            for ii in 0..nx {
                if mmm[ii + jy + kz] != 0 {
                    continue;
                }
                let (_, im, ip) = offsets(ii, nx, 1);
                let n = Nbhd {
                    ii,
                    im,
                    ip,
                    jy,
                    jm,
                    jp,
                    kz,
                    km,
                    kp,
                };
                let mut num = n.faces(mmm);
                if nn >= 2 {
                    num += n.edges(mmm);
                }
                if nn == 3 {
                    num += n.corners(mmm);
                }
                if num >= ndil {
                    nnn[ii + jy + kz] = 1;
                }
            }
        }
    }
    let mut nadd = 0;
    for (m, &n) in mmm.iter_mut().zip(&nnn) {
        if n != 0 && *m == 0 {
            *m = 1;
            nadd += 1;
        }
    }
    nadd
}

/// What [`depth`] says when it stops early.
pub enum DepthNote {
    /// "Nothing left to peel": the erosion removed nothing while voxels remain.
    Stuck { iterations: i32, remaining: i32 },
}

/// `THD_mask_depth` (on a copy of the mask): how many erosions ([`erode`] with `nn`) it takes
/// to remove each voxel; 0 outside the mask.
pub fn depth(
    g: Grid,
    mask: &[u8],
    nn: u8,
    s: &MaskSettings,
    log: &mut Vec<Message>,
) -> (Vec<i16>, Option<DepthNote>) {
    let nxyz = g.nvox();
    let mut cp = mask.to_vec();
    let mut d = vec![0i16; nxyz];
    let mut ncp = count(&cp);
    let mut niter = 0;
    while ncp > 0 {
        for (dv, &m) in d.iter_mut().zip(&cp) {
            if m != 0 {
                *dv = dv.wrapping_add(1);
            }
        }
        erode(g, &mut cp, nn, s, log);
        let np = ncp - count(&cp);
        if s.verbose {
            log.push(Message::Info(format!(
                "Peeled {np} voxels from mask of {ncp} (now {})\n",
                ncp - np
            )));
        }
        ncp -= np;
        niter += 1;
        if np == 0 && ncp != 0 {
            return (
                d,
                Some(DepthNote::Stuck {
                    iterations: niter,
                    remaining: ncp,
                }),
            );
        }
    }
    (d, None)
}

/// `THD_mask_erodemany2D` with AFNI's fixed threshold of 6 of the 8 in-plane neighbours.
fn erodemany_2d(nx: usize, ny: usize, mmm: &mut [u8], npeel: i32) {
    let nxy = nx * ny;
    if npeel < 1 || nxy < 16 {
        return;
    }
    const REALPEELTHR: i32 = 6;
    let mut nnn = vec![0u8; nxy];
    let mut qqq = vec![0u8; nxy];
    let count8 = |m: &[u8], ii: usize, im: usize, ip: usize, jy: usize, jm: usize, jp: usize| {
        let f = |i: usize| i32::from(m[i]);
        f(im + jm)
            + f(ii + jm)
            + f(ip + jm)
            + f(im + jy)
            + f(ip + jy)
            + f(im + jp)
            + f(ii + jp)
            + f(ip + jp)
    };
    for pp in 1..=npeel {
        let bpp = pp as u8;
        for jj in 0..ny {
            let (jy, jm, jp) = offsets(jj, ny, nx);
            for ii in 0..nx {
                if mmm[ii + jy] != 0 {
                    let (_, im, ip) = offsets(ii, nx, 1);
                    if count8(mmm, ii, im, ip, jy, jm, jp) < REALPEELTHR {
                        nnn[ii + jy] = bpp;
                    }
                }
            }
        }
        for (m, &n) in mmm.iter_mut().zip(&nnn) {
            if n != 0 {
                *m = 0;
            }
        }
    }
    for pp in (1..=npeel).rev() {
        let bpp = pp as u8;
        qqq.iter_mut().for_each(|q| *q = 0);
        let bth: u8 = if pp == npeel { 0 } else { 1 };
        for jj in 0..ny {
            let (jy, jm, jp) = offsets(jj, ny, nx);
            for ii in 0..nx {
                if nnn[ii + jy] >= bpp && mmm[ii + jy] == 0 {
                    let (_, im, ip) = offsets(ii, nx, 1);
                    qqq[ii + jy] = count8(mmm, ii, im, ip, jy, jm, jp) as u8;
                }
            }
        }
        for (m, &q) in mmm.iter_mut().zip(&qqq) {
            if q > bth {
                *m = 1;
            }
        }
    }
}

/// `THD_mask_clust2D`: keeps every 4-connected cluster at least `kfrac` times the size of the
/// largest (`(int)(kfrac * nbest)` voxels; the largest alone when that is not positive).
fn clust_2d(nx: usize, ny: usize, kfrac: f32, mmm: &mut [u8]) {
    let nxy = nx * ny;
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut nbest = 0usize;
    for start in 0..nxy {
        if mmm[start] == 0 {
            continue;
        }
        mmm[start] = 0;
        let mut now = vec![start];
        let mut icl = 0;
        while icl < now.len() {
            let ijk = now[icl];
            let (ii, jj) = (ijk % nx, ijk / nx);
            let mut put = |q: usize, mmm: &mut [u8]| {
                if mmm[q] != 0 {
                    mmm[q] = 0;
                    now.push(q);
                }
            };
            if ii > 0 {
                put(ijk - 1, mmm);
            }
            if ii + 1 < nx {
                put(ijk + 1, mmm);
            }
            if jj > 0 {
                put(ijk - nx, mmm);
            }
            if jj + 1 < ny {
                put(ijk + nx, mmm);
            }
            icl += 1;
        }
        nbest = nbest.max(now.len());
        clusters.push(now);
    }
    let mut nkeep = (kfrac * nbest as f32) as i64;
    if nkeep <= 0 || nkeep > nbest as i64 {
        nkeep = nbest as i64;
    }
    for c in &clusters {
        if c.len() as i64 >= nkeep {
            for &v in c {
                mmm[v] = 1;
            }
        }
    }
}

/// `mri_automask_image2D`: the mask of a 2D image (at least 16 x 16): |values| at or above the
/// clip level, peeled 3 times and restored, clusters of at least half the largest kept, and
/// holes (clusters of the outside smaller than 0.9 times the largest) filled. `None` when the
/// image is too small or nothing is above the clip level.
fn automask_image_2d(im: &[f32], nx: usize, ny: usize, s: &MaskSettings) -> Option<Vec<u8>> {
    if nx < 16 || ny < 16 {
        return None;
    }
    let mut mar: Vec<f32> = im.to_vec();
    crate::dataset::floatscan(&mut mar);
    for v in &mut mar {
        *v = v.abs();
    }
    let clip_val = cliplevel(&mar, s.clfrac);
    let nvox = mar.len();
    let mut mmm = vec![0u8; nvox];
    let mut nmm = 0;
    for (m, &v) in mmm.iter_mut().zip(&mar) {
        if v >= clip_val {
            *m = 1;
            nmm += 1;
        }
    }
    if nmm == 0 {
        return None;
    }
    if nmm <= 2 || nmm == nvox {
        return Some(mmm);
    }
    erodemany_2d(nx, ny, &mut mmm, 3);
    clust_2d(nx, ny, 0.5, &mut mmm);
    for m in mmm.iter_mut() {
        *m = u8::from(*m == 0);
    }
    clust_2d(nx, ny, 0.9, &mut mmm);
    for m in mmm.iter_mut() {
        *m = u8::from(*m == 0);
    }
    Some(mmm)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ball(g: Grid, c: [f64; 3], r: f64, inside: f32, outside: f32) -> Vec<f32> {
        let mut v = Vec::with_capacity(g.nvox());
        for k in 0..g.nz {
            for j in 0..g.ny {
                for i in 0..g.nx {
                    let d = ((i as f64 - c[0]).powi(2)
                        + (j as f64 - c[1]).powi(2)
                        + (k as f64 - c[2]).powi(2))
                    .sqrt();
                    // A texture of 10 % (AFNI's histogram leaves out the bin of the maximum).
                    let t = ((i * 7 + j * 13 + k * 17) % 10) as f32 / 100.0;
                    v.push(if d <= r { inside } else { outside } * (0.95 + t));
                }
            }
        }
        v
    }

    #[test]
    fn settings_ranges() {
        let mut s = MaskSettings::default();
        s.set_clipfrac(0.95);
        assert_eq!(s.clfrac, 0.5);
        s.set_clipfrac(0.2);
        assert_eq!(s.clfrac, 0.2);
        s.set_peelcounts(0, 30);
        assert_eq!((s.peelcount, s.peelthr), (1, 17));
        s.set_peelcounts(3, 6);
        assert_eq!((s.peelcount, s.peelthr), (3, 6));
    }

    #[test]
    fn cliplevel_of_two_levels() {
        // 1000 voxels of 100 and 1000 of 10. The maximum's bin (10000) is outside AFNI's
        // loops, so only the 10s count: the cut is 0.5 times their bin, 1001 (the loop ends one
        // past the median bin), truncated: 500 bins of 100/10000.
        let mut im = vec![100.0f32; 1000];
        im.extend(std::iter::repeat_n(10.0f32, 1000));
        assert_eq!(cliplevel(&im, 0.5), 5.0);
        // A spread of values: the cut lies between the groups.
        let im: Vec<f32> = (0..2000)
            .map(|i| {
                if i < 1000 {
                    90.0 + (i % 20) as f32
                } else {
                    5.0 + (i % 10) as f32
                }
            })
            .collect();
        let c = cliplevel(&im, 0.5);
        assert!(c > 14.0 && c < 90.0, "{c}");
        // At most 222 positive voxels: 0.
        assert_eq!(cliplevel(&[5.0; 222], 0.5), 0.0);
        assert_eq!(cliplevel(&[0.0; 1000], 0.5), 0.0);
    }

    #[test]
    fn clust_keeps_the_first_largest() {
        let g = Grid::new(10, 1, 1);
        let mut m = vec![1, 1, 0, 1, 1, 0, 1, 1, 1, 0];
        clust(g, &mut m, &MaskSettings::default(), &mut Vec::new());
        assert_eq!(m, vec![0, 0, 0, 0, 0, 0, 1, 1, 1, 0]);
        let mut tie = vec![1, 1, 0, 1, 1, 0, 0, 0, 0, 0];
        clust(g, &mut tie, &MaskSettings::default(), &mut Vec::new());
        assert_eq!(tie, vec![1, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn fillin_fills_a_one_voxel_gap() {
        let g = Grid::new(5, 5, 5);
        let mut m = vec![0u8; 125];
        let at = |i: usize, j: usize, k: usize| i + 5 * j + 25 * k;
        m[at(1, 2, 2)] = 1;
        m[at(3, 2, 2)] = 1;
        assert_eq!(fillin_once(g, &mut m, 1), 1);
        assert_eq!(m[at(2, 2, 2)], 1);
    }

    #[test]
    fn dilate_and_erode_by_level() {
        let g = Grid::new(7, 7, 7);
        let at = |i: usize, j: usize, k: usize| i + 7 * j + 49 * k;
        let mut one = vec![0u8; 343];
        one[at(3, 3, 3)] = 1;
        let mut m = one.clone();
        assert_eq!(dilate(g, &mut m, 1, 1), 6);
        let mut m = one.clone();
        assert_eq!(dilate(g, &mut m, 1, 2), 18);
        let mut m = one.clone();
        assert_eq!(dilate(g, &mut m, 1, 3), 26);
        // A 3x3x3 cube eroded with NN1 keeps its centre.
        let mut cube = vec![0u8; 343];
        for k in 2..5 {
            for j in 2..5 {
                for i in 2..5 {
                    cube[at(i, j, k)] = 1;
                }
            }
        }
        erode(g, &mut cube, 1, &MaskSettings::default(), &mut Vec::new());
        assert_eq!(count(&cube), 1);
        assert_eq!(cube[at(3, 3, 3)], 1);
    }

    #[test]
    fn depth_counts_layers() {
        let g = Grid::new(9, 9, 9);
        let mut m = vec![0u8; 729];
        for k in 1..8 {
            for j in 1..8 {
                for i in 1..8 {
                    m[i + 9 * j + 81 * k] = 1;
                }
            }
        }
        let (d, note) = depth(g, &m, 1, &MaskSettings::default(), &mut Vec::new());
        assert!(note.is_none());
        assert_eq!(d[4 + 9 * 4 + 81 * 4], 4);
        assert_eq!(d[1 + 9 * 4 + 81 * 4], 1);
        assert_eq!(d[0], 0);
    }

    #[test]
    fn automask_of_a_ball() {
        let g = Grid::new(32, 32, 32);
        let im = ball(g, [15.5, 15.5, 15.5], 10.0, 1000.0, 5.0);
        let mut log = Vec::new();
        let s = MaskSettings {
            verbose: true,
            ..MaskSettings::default()
        };
        let m = automask_image(&im, g, &s, &mut log).unwrap();
        let inside = im.iter().filter(|&&v| v > 500.0).count() as i32;
        let both = m
            .iter()
            .zip(&im)
            .filter(|&(&k, &v)| k != 0 && v > 500.0)
            .count() as i32;
        assert!(
            both == inside && count(&m) < inside + inside / 10,
            "{} {inside}",
            count(&m)
        );
        assert!(
            log.iter()
                .any(|l| matches!(l, Message::InInfo(t) if t.starts_with("Fixed clip level")))
        );
    }

    #[test]
    fn automask_2d_needs_16_by_16() {
        let s = MaskSettings::default();
        assert!(
            automask_image(&[1.0; 15 * 20], Grid::new(15, 20, 1), &s, &mut Vec::new()).is_none()
        );
        let g = Grid::new(20, 20, 1);
        let im = ball(g, [9.5, 9.5, 0.0], 6.0, 100.0, 1.0);
        let m = automask_image(&im, g, &s, &mut Vec::new()).unwrap();
        assert!(count(&m) > 0);
    }
}
