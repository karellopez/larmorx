// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09: src/3dUnifize.c (mri_clipedges_inplace, median7,
// mri_double_down, mri_double_up, mri_local_percmean, mri_WMunifize, mri_GMunifize),
// src/mri_invert.c (mri_invertcontrast_inplace), src/thd_dset_nbhd.c (mri_get_nbhd_array,
// with SetSearchAboutMaskedVoxel), src/edt_buildmask.c (MCW_build_mask, MCW_spheremask),
// src/cs_qmed.c (qmed_float, median_float4, median_float5, median_float7, median_float9),
// src/mri_percents.c (qsort_float, as a sort) and src/thd_median.c (THD_median_brick).
// 3dUnifize.c, mri_invert.c, thd_dset_nbhd.c and thd_median.c carry no Medical College of
// Wisconsin notice: they were written at the NIH and are in the public domain. edt_buildmask.c,
// cs_qmed.c and mri_percents.c: Major portions Copyright (C) 1994-2000 Medical College of
// Wisconsin (AFNI, Robert W. Cox et al.), released under the GNU GPL version 2 or any later
// version.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! The steps of `3dUnifize`, as AFNI 25.2.09 computes them.
//!
//! - [`local_percmean`]: in a ball around each voxel, the mean of the automasked values
//!   between two percentiles (70 % and 80 % by default): the local white-matter intensity
//!   (WMI). For speed, on volumes over a million voxels, AFNI computes it on a half-size copy
//!   ([`double_down`], the median of each voxel and its 6 face neighbours) with half the
//!   radius, and expands it back ([`double_up`], an average of up to 8 neighbours).
//! - [`wm_unifize`]: every voxel times 1000/WMI, values above 1300 squashed with `tanhf`.
//! - [`gm_unifize`] (`-GM`): a global linear map putting the grey-matter median at 666.
//! - [`invert_contrast`] (`-T2`): `U - x` inside the mask, with `U` the 98.5th percentile.
//! - [`clip_edges`] (`-T2 -T2`): bright voxels with several zero neighbours zeroed.
//!
//! The order of the values within AFNI's sorts never matters: a sorted array of finite floats
//! is the same whatever the algorithm, except where -0 and +0 meet, which no sum or percentile
//! here can tell apart. `qmed_float`'s partial quicksort is translated as it is ([`qmed`]): its
//! even-length median averages two values, in its own order.
//!
//! **Speed.** The local percentile mean is the slow step (AFNI runs it with OpenMP). Each voxel
//! is independent, so larmorx-gpl computes them in parallel, with the same result at any thread
//! count; within a voxel only the values between the two percentiles are sorted (a selection
//! puts them in place), and they are summed in sorted order, as AFNI sums them.

use rayon::prelude::*;

use crate::glibc_floatmath::tanhf;
use crate::mask::{self, Grid, MaskSettings};
use crate::message::Message;

/// `PKVAL`, `PKMID`, `WMCUT`, `WMSCL`.
pub const PKVAL: f32 = 1000.0;
pub const PKMID: f32 = 666.0;
pub const WMCUT: f32 = 1300.0;
pub const WMSCL: f32 = 200.0;

/// `SORT2(a, b)`: `if (a > b) swap`.
#[inline]
fn sort2(p: &mut [f32], a: usize, b: usize) {
    if p[a] > p[b] {
        p.swap(a, b);
    }
}

/// `median7` (3dUnifize.c) and `median_float7` (cs_qmed.c): a sorting network.
pub fn median7(p: &mut [f32; 7]) -> f32 {
    for (a, b) in [
        (0, 1),
        (4, 5),
        (1, 2),
        (5, 6),
        (0, 1),
        (4, 5),
        (0, 4),
        (2, 6),
        (1, 3),
        (3, 5),
        (1, 3),
        (2, 3),
        (3, 4),
        (2, 3),
    ] {
        sort2(p, a, b);
    }
    p[3]
}

fn median5(p: &mut [f32]) -> f32 {
    for (a, b) in [(0, 1), (3, 4), (0, 3), (1, 4), (1, 2), (2, 3), (1, 2)] {
        sort2(p, a, b);
    }
    p[2]
}

fn median9(p: &mut [f32]) -> f32 {
    for (a, b) in [
        (1, 2),
        (4, 5),
        (7, 8),
        (0, 1),
        (3, 4),
        (6, 7),
        (1, 2),
        (4, 5),
        (7, 8),
        (0, 3),
        (5, 8),
        (4, 7),
        (3, 6),
        (1, 4),
        (2, 5),
        (4, 7),
        (4, 2),
        (6, 4),
        (4, 2),
    ] {
        sort2(p, a, b);
    }
    p[4]
}

/// `median_float4`.
fn median4(a: f32, b: f32, c: f32, d: f32) -> f32 {
    let (mut a, mut d) = (a, d);
    let t1 = if a > b {
        let t = a;
        a = b;
        t
    } else {
        b
    };
    let t2 = if c > d {
        let t = d;
        d = c;
        t
    } else {
        c
    };
    let t3 = if a > t2 { a } else { t2 };
    let t2 = if d < t1 { d } else { t1 };
    (0.5 * f64::from(t2 + t3)) as f32
}

/// `MED3(a, b, c)`.
fn med3(a: f32, b: f32, c: f32) -> f32 {
    if a < b {
        if a > c {
            a
        } else if b > c {
            c
        } else {
            b
        }
    } else if b > c {
        b
    } else if a > c {
        c
    } else {
        a
    }
}

/// `qmed_float`: the median of `a` (partially sorting it), with AFNI's special cases and its
/// partial quicksort; an even length averages the two middle values (`0.5*(x+y)`, in double,
/// rounded to float).
pub fn qmed(a: &mut [f32]) -> f32 {
    let n = a.len();
    match n {
        0 => return 0.0,
        1 => return a[0],
        2 => return (0.5 * f64::from(a[0] + a[1])) as f32,
        3 => return med3(a[0], a[1], a[2]),
        4 => return median4(a[0], a[1], a[2], a[3]),
        5 => return median5(a),
        7 => {
            let mut p = [a[0], a[1], a[2], a[3], a[4], a[5], a[6]];
            let m = median7(&mut p);
            a.copy_from_slice(&p);
            return m;
        }
        9 => return median9(a),
        _ => {}
    }
    let (mut left, mut right) = (0usize, n - 1);
    let mid = n / 2;
    let nodd = n % 2 == 1;
    while right - left > 1 {
        let mut i = (left + right) / 2;
        if a[left] > a[i] {
            a.swap(left, i);
        }
        if a[left] > a[right] {
            a.swap(left, right);
        }
        if a[i] > a[right] {
            a.swap(right, i);
        }
        let pivot = a[i];
        a[i] = a[right];
        i = left;
        let mut j = right;
        loop {
            // `for( ; a[++i] < pivot ; ) ;` then `for( ; a[--j] > pivot ; ) ;`: the median of
            // three left a value >= pivot at `right` and one <= pivot at `left`.
            loop {
                i += 1;
                if !(a[i] < pivot) {
                    break;
                }
            }
            loop {
                j -= 1;
                if !(a[j] > pivot) {
                    break;
                }
            }
            if j <= i {
                break;
            }
            a.swap(i, j);
        }
        a[right] = a[i];
        a[i] = pivot;
        if i == mid {
            if nodd {
                return pivot;
            }
            let mut temp = a[left];
            for &v in &a[left + 1..i] {
                if v > temp {
                    temp = v;
                }
            }
            return (0.5 * f64::from(pivot + temp)) as f32;
        }
        if i < mid {
            left = i;
        } else {
            right = i;
        }
    }
    if nodd {
        a[mid]
    } else {
        (0.5 * f64::from(a[mid] + a[mid - 1])) as f32
    }
}

/// `mri_double_down`: a grid half the size (at least 1) along each axis, each voxel the median
/// of the input voxel at twice its index and that voxel's 6 face neighbours (clamped at the
/// edges).
pub fn double_down(far: &[f32], g: Grid) -> (Vec<f32>, Grid) {
    let (nxf, nyf, nzf) = (g.nx, g.ny, g.nz);
    let h = Grid::new((nxf / 2).max(1), (nyf / 2).max(1), (nzf / 2).max(1));
    let nxyf = nxf * nyf;
    let at = |i: usize, j: usize, k: usize| far[i + j * nxf + k * nxyf];
    let mut gar = Vec::with_capacity(h.nvox());
    for kk in 0..h.nz {
        let kuu = 2 * kk;
        let kum = kuu.saturating_sub(1);
        let kup = (kuu + 1).min(nzf - 1);
        for jj in 0..h.ny {
            let juu = 2 * jj;
            let jum = juu.saturating_sub(1);
            let jup = (juu + 1).min(nyf - 1);
            for ii in 0..h.nx {
                let iuu = 2 * ii;
                let ium = iuu.saturating_sub(1);
                let iup = (iuu + 1).min(nxf - 1);
                let mut par = [
                    at(iuu, juu, kuu),
                    at(ium, juu, kuu),
                    at(iup, juu, kuu),
                    at(iuu, jum, kuu),
                    at(iuu, jup, kuu),
                    at(iuu, juu, kum),
                    at(iuu, juu, kup),
                ];
                gar.push(median7(&mut par));
            }
        }
    }
    (gar, h)
}

/// `mri_double_up`: twice the size (plus 1 along an axis whose `add` is set; an axis of size 1
/// stays 1), each voxel `0.125f *` the sum of the 8 input voxels at half its index and the next
/// one along each axis whose output index is odd (clamped at the edges), summed in AFNI's
/// order.
pub fn double_up(far: &[f32], g: Grid, add: [bool; 3]) -> (Vec<f32>, Grid) {
    let (nxf, nyf, nzf) = (g.nx, g.ny, g.nz);
    let up = |n: usize, a: bool| if n == 1 { 1 } else { 2 * n + usize::from(a) };
    let h = Grid::new(up(nxf, add[0]), up(nyf, add[1]), up(nzf, add[2]));
    let nxyf = nxf * nyf;
    let at = |i: usize, j: usize, k: usize| far[i + j * nxf + k * nxyf];
    let pair = |o: usize, n: usize| {
        let mut m = o / 2;
        if m >= n {
            m = n - 1;
        }
        let mut p = m;
        if o % 2 == 1 {
            p += 1;
            if p >= n {
                p = n - 1;
            }
        }
        (m, p)
    };
    let mut gar = Vec::with_capacity(h.nvox());
    for kk in 0..h.nz {
        let (km, kp) = pair(kk, nzf);
        for jj in 0..h.ny {
            let (jm, jp) = pair(jj, nyf);
            for ii in 0..h.nx {
                let (im, ip) = pair(ii, nxf);
                gar.push(
                    0.125f32
                        * (at(im, jm, km)
                            + at(ip, jm, km)
                            + at(im, jp, km)
                            + at(ip, jp, km)
                            + at(im, jm, kp)
                            + at(ip, jm, kp)
                            + at(im, jp, kp)
                            + at(ip, jp, kp)),
                );
            }
        }
    }
    (gar, h)
}

/// `MCW_spheremask(1, 1, 1, radius)`: the offsets `(i, j, k)` with `i^2 + j^2 + k^2 <=
/// radius^2` (in float), the centre included. The order is irrelevant here (the values are
/// sorted).
pub fn sphere(radius: f32) -> Vec<[i32; 3]> {
    let mut pts = vec![[0, 0, 0]];
    // MCW_build_mask: `idx = max_dist/dx` (truncated); NULL when every half-width is 0.
    let n = radius as i32;
    if n < 1 {
        return pts;
    }
    let dist_q = radius * radius;
    for kk in -n..=n {
        let zq = (kk as f32) * (kk as f32);
        for jj in -n..=n {
            let yq = zq + (jj as f32) * (jj as f32);
            for ii in -n..=n {
                let xq = yq + (ii as f32) * (ii as f32);
                if xq <= dist_q && xq > 0.0 {
                    pts.push([ii, jj, kk]);
                }
            }
        }
    }
    pts
}

/// The voxel mean between percentiles `p1` and `p2` of the values `v` (sorted in part here):
/// `q = (int)(0.01f*p*(n-1))`, the sum of the sorted values `q1..=q2` in float, divided by
/// `q2 - q1 + 1.0f`. One value: itself; none: 0.
fn percmean(v: &mut [f32], p1: f32, p2: f32) -> f32 {
    let n = v.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return v[0];
    }
    // At most n-1 for percentiles up to 100 (3dUnifize allows at most 90).
    let q1 = ((0.01f32 * p1 * (n - 1) as f32) as usize).min(n - 1);
    let q2 = ((0.01f32 * p2 * (n - 1) as f32) as usize).min(n - 1);
    // Put the values of ranks q1..=q2 in place, sorted (ties and signed zeros cannot change the
    // sum: equal values are equal, and a sum starting at +0 is never -0).
    let cmp = |a: &f32, b: &f32| a.total_cmp(b);
    if q1 > 0 {
        v.select_nth_unstable_by(q1, cmp);
    }
    let rest = &mut v[q1..];
    let span = q2 - q1;
    if span + 1 < rest.len() {
        rest.select_nth_unstable_by(span, cmp);
    }
    let window = &mut rest[..=span];
    window.sort_unstable_by(cmp);
    let mut val = 0.0f32;
    for &x in window.iter() {
        val += x;
    }
    val / ((q2 - q1) as f32 + 1.0f32)
}

/// What [`local_percmean`] reports while it works (the letters AFNI prints with its `verb`).
pub struct Progress<'a> {
    pub verbose: bool,
    pub out: &'a mut String,
}

/// `mri_local_percmean(fim, vrad, p1, p2)`: the local mean between percentiles `p1` and `p2`
/// in a ball of radius `vrad` voxels, over the automasked image (`settings` and `log` for the
/// automask; `amsave` receives the automasked image, as `-amsave` writes it). `None` where
/// AFNI returns NULL: a radius below 4, percentiles outside [0, 100], or no automask. `p1 ==
/// p2` (AFNI's `mri_local_percentile`) is not reachable from 3dUnifize's command line and is
/// refused.
#[allow(clippy::too_many_arguments)]
pub fn local_percmean(
    fim: &[f32],
    g: Grid,
    vrad: f32,
    p1: f32,
    p2: f32,
    do_double: bool,
    settings: &MaskSettings,
    log: &mut Vec<Message>,
    progress: &mut Progress<'_>,
    amsave: &mut dyn FnMut(&[f32]),
    n_threads: usize,
) -> Option<Vec<f32>> {
    let (p1, p2) = if p1 > p2 { (p2, p1) } else { (p1, p2) };
    if vrad < 4.0 || p1 < 0.0 || p2 > 100.0 || p1 == p2 {
        return None;
    }
    if progress.verbose {
        progress.out.push('A');
    }
    let mut aar = fim.to_vec();
    crate::dataset::floatscan(&mut aar);
    let ams = mask::automask_image(&aar, g, settings, log)?;
    for (a, &m) in aar.iter_mut().zip(&ams) {
        if m == 0 {
            *a = 0.0;
        }
    }
    amsave(&aar);
    if progress.verbose && do_double {
        progress.out.push('D');
    }
    let (bar, bg) = if do_double {
        double_down(&aar, g)
    } else {
        (aar, g)
    };
    let radius = if do_double {
        0.5f32 * vrad + 0.001f32
    } else {
        vrad
    };
    let nbhd = sphere(radius);
    if progress.verbose {
        progress.out.push('V');
    }
    let (nx, ny, nz) = (bg.nx as i64, bg.ny as i64, bg.nz as i64);
    let nxy = bg.nx * bg.ny;
    let one = |vvv: usize, nbar: &mut Vec<f32>| -> (f32, bool) {
        let ii = (vvv % bg.nx) as i64;
        let kk = (vvv / nxy) as i64;
        let jj = ((vvv - kk as usize * nxy) / bg.nx) as i64;
        nbar.clear();
        for &[di, dj, dk] in &nbhd {
            let (a, b, c) = (ii + i64::from(di), jj + i64::from(dj), kk + i64::from(dk));
            if a < 0 || a >= nx || b < 0 || b >= ny || c < 0 || c >= nz {
                continue;
            }
            let v = bar[a as usize + b as usize * bg.nx + c as usize * nxy];
            if v != 0.0 {
                nbar.push(v);
            }
        }
        let many = nbar.len() > 1;
        (percmean(nbar, p1, p2), many)
    };
    let results: Vec<(f32, bool)> = larmorx_core::parallel::with_threads(n_threads, || {
        (0..bg.nvox())
            .into_par_iter()
            .map_init(
                || Vec::with_capacity(nbhd.len()),
                |nbar, vvv| one(vvv, nbar),
            )
            .collect()
    })
    .unwrap_or_else(|_| {
        let mut nbar = Vec::with_capacity(nbhd.len());
        (0..bg.nvox()).map(|v| one(v, &mut nbar)).collect()
    });
    if progress.verbose {
        // `if( verb && vvv%66666==0 ) fprintf(stderr,".")`, for voxels with two values or more.
        let dots = results
            .iter()
            .enumerate()
            .filter(|(v, r)| v % 66666 == 0 && r.1)
            .count();
        progress.out.extend(std::iter::repeat_n('.', dots));
    }
    let car: Vec<f32> = results.into_iter().map(|r| r.0).collect();
    let dim = if do_double {
        double_up(&car, bg, [g.nx % 2 == 1, g.ny % 2 == 1, g.nz % 2 == 1]).0
    } else {
        car
    };
    if progress.verbose {
        progress.out.push('U');
    }
    Some(dim)
}

/// The result of [`wm_unifize`]: the scaled image and the scale image (`sclim`, `-ssave`).
pub struct WmResult {
    pub image: Vec<f32>,
    pub scale: Vec<f32>,
    /// How many values were squashed.
    pub squashed: usize,
    /// When the half-size grid expanded back has another size than the input (an axis of 2 or
    /// 3 voxels in a volume of more than a million): AFNI reads its scale image past its end
    /// (a defect); the voxels it has none for got scale 0.
    pub short_scale: Option<usize>,
}

/// `mri_WMunifize`: every value times `1000/WMI` (0 where WMI is not positive), then, with
/// `squash`, values above 1300 become `1300 + 200 tanhf((x - 1300)/200)`. `None` where the local
/// mean is (no automask).
#[allow(clippy::too_many_arguments)]
pub fn wm_unifize(
    fim: &[f32],
    g: Grid,
    urad: f32,
    upbot: f32,
    uptop: f32,
    do_double: bool,
    squash: bool,
    settings: &MaskSettings,
    log: &mut Vec<Message>,
    progress: &mut Progress<'_>,
    amsave: &mut dyn FnMut(&[f32]),
    n_threads: usize,
) -> Option<WmResult> {
    let do_double = do_double && fim.len() > 1_000_000;
    let mut par = local_percmean(
        fim, g, urad, upbot, uptop, do_double, settings, log, progress, amsave, n_threads,
    )?;
    let mut gar = fim.to_vec();
    crate::dataset::floatscan(&mut gar);
    let short_scale = (par.len() != gar.len()).then_some(par.len());
    if par.len() < gar.len() {
        par.resize(gar.len(), 0.0);
    }
    let mut nsq = 0;
    for (ii, g) in gar.iter_mut().enumerate() {
        let pval = if par[ii] <= 0.0 { 0.0 } else { PKVAL / par[ii] };
        par[ii] = pval;
        *g *= pval;
        if squash && *g > WMCUT {
            *g = WMCUT + WMSCL * tanhf((*g - WMCUT) / WMSCL);
            nsq += 1;
        }
    }
    par.truncate(gar.len());
    if progress.verbose {
        progress.out.push('W');
        if nsq > 0 {
            progress.out.push_str(&format!("[s{nsq}]"));
        }
    }
    Some(WmResult {
        image: gar,
        scale: par,
        squashed: nsq,
        short_scale,
    })
}

/// `mri_GMunifize`: from the WM-unifized image, the median `P` of the values above 1000,
/// `pupper = 1000 - 1.987654321 (P - 1000)`, `plower` the clip level for 0.4321, the median
/// `G` of the values in `[plower, pupper]`; then every value at or above `plower/3`
/// (`0.333f`) becomes `(1000-666)/(1000-G) (x - 1000) + 1000` (negative results 0), the
/// others 0. Nothing when fewer than 111 values are above 1000, or in the range.
pub fn gm_unifize(gar: &mut [f32], progress: &mut Progress<'_>) {
    let mut pval: Vec<f32> = gar.iter().copied().filter(|&v| v > PKVAL).collect();
    if pval.len() < 111 {
        return;
    }
    let mut pupper = qmed(&mut pval);
    pupper = PKVAL - 1.987_654_3f32 * (pupper - PKVAL);
    let mut plower = mask::cliplevel(gar, 0.4321f32);
    let mut pval: Vec<f32> = gar
        .iter()
        .copied()
        .filter(|&v| v >= plower && v <= pupper)
        .collect();
    if pval.len() < 111 {
        return;
    }
    let pmid = qmed(&mut pval);
    let pfac = (PKVAL - PKMID) / (PKVAL - pmid);
    plower *= 0.333f32;
    for v in gar.iter_mut() {
        if *v >= plower {
            *v = pfac * (*v - PKVAL) + PKVAL;
            if *v < 0.0 {
                *v = 0.0;
            }
        } else {
            *v = 0.0;
        }
    }
    if progress.verbose {
        progress.out.push('G');
    }
}

/// `mri_clipedges_inplace(im, uclip, vclip)`: repeatedly, voxels at or above `uclip` with more
/// than 2 of their 6 face neighbours 0, and voxels at or above `vclip` with more than 4, become
/// 0 (both tests on the image as it was at the start of the pass), until a pass clips nothing.
pub fn clip_edges(qmar: &mut [f32], g: Grid, uclip: f32, vclip: f32, progress: &mut Progress<'_>) {
    let (nx, ny, nz, nxy) = (g.nx, g.ny, g.nz, g.nx * g.ny);
    if progress.verbose {
        progress.out.push('e');
    }
    let edges = |imar: &[f32], pp: usize| {
        let kk = pp / nxy;
        let ii = pp % nx;
        let jj = (pp - kk * nxy) / nx;
        let kp = (kk + 1).min(nz - 1);
        let km = kk.saturating_sub(1);
        let jp = (jj + 1).min(ny - 1);
        let jm = jj.saturating_sub(1);
        let ip = (ii + 1).min(nx - 1);
        let im = ii.saturating_sub(1);
        let e = |i: usize, j: usize, k: usize| i32::from(imar[i + j * nx + k * nxy] == 0.0);
        e(ip, jj, kk)
            + e(im, jj, kk)
            + e(ii, jp, kk)
            + e(ii, jm, kk)
            + e(ii, jj, kp)
            + e(ii, jj, km)
    };
    loop {
        let imar = qmar.to_vec();
        let mut nclip = 0;
        for pp in 0..qmar.len() {
            if imar[pp] < uclip {
                continue;
            }
            if edges(&imar, pp) > 2 {
                qmar[pp] = 0.0;
                nclip += 1;
            }
        }
        for pp in 0..qmar.len() {
            if imar[pp] < vclip {
                continue;
            }
            if edges(&imar, pp) > 4 {
                qmar[pp] = 0.0;
                nclip += 1;
            }
        }
        if nclip == 0 {
            return;
        }
    }
}

/// How [`invert_contrast`] went.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Inversion {
    /// The image is one row: nothing done.
    OneRow,
    /// Fewer than 100 mask voxels: nothing done.
    TooFew(usize),
    /// Inverted about `ucut`.
    Done { ucut: f32 },
    /// The percentile index fell one past the sorted mask values (`-T2up` at or near 100): AFNI
    /// reads past them (a defect); `ucut` is what was used.
    PastTheEnd { ucut: f32, nhist: usize },
}

/// `mri_invertcontrast_inplace(im, uperc, mask)` for a float image: `U` is the sorted mask
/// value at index `rintf(nhist*uperc*0.01f)` (`uperc` clamped to [90, 100]); inside the mask
/// positive values become `U - x`; values outside the mask, and negative results, become 0.
/// Past the end of the sorted values, `past_end` gives `U`: AFNI's value there is whatever its
/// memory holds.
pub fn invert_contrast(
    im: &mut [f32],
    ny: usize,
    uperc: f32,
    mask: &[u8],
    past_end: &dyn Fn(&[f32]) -> f32,
) -> Inversion {
    if ny == 1 {
        return Inversion::OneRow;
    }
    let uperc = uperc.clamp(90.0, 100.0);
    let mut imar = im.to_vec();
    crate::dataset::floatscan(&mut imar);
    let mut hist: Vec<f32> = imar
        .iter()
        .zip(mask)
        .filter(|&(_, &m)| m != 0)
        .map(|(&v, _)| v)
        .collect();
    let nhist = hist.len();
    if nhist < 100 {
        return Inversion::TooFew(nhist);
    }
    hist.sort_unstable_by(f32::total_cmp);
    let ii = (nhist as f32 * uperc * 0.01f32).round_ties_even() as usize;
    let (ucut, outcome) = if ii < nhist {
        (hist[ii], None)
    } else {
        let u = past_end(&hist);
        (u, Some(Inversion::PastTheEnd { ucut: u, nhist }))
    };
    for (v, &m) in imar.iter_mut().zip(mask) {
        if m != 0 && *v > 0.0 {
            *v = ucut - *v;
        }
        if m == 0 || *v < 0.0 {
            *v = 0.0;
        }
    }
    im.copy_from_slice(&imar);
    outcome.unwrap_or(Inversion::Done { ucut })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medians() {
        let mut p = [5.0, 1.0, 4.0, 2.0, 3.0, 7.0, 6.0];
        assert_eq!(median7(&mut p), 4.0);
        for n in 1..40usize {
            let v: Vec<f32> = (0..n).map(|i| ((i * 7919) % 101) as f32).collect();
            let mut s = v.clone();
            s.sort_by(f32::total_cmp);
            let want = if n % 2 == 1 {
                s[n / 2]
            } else {
                (0.5 * f64::from(s[n / 2] + s[n / 2 - 1])) as f32
            };
            let mut a = v.clone();
            assert_eq!(qmed(&mut a), want, "n = {n}");
        }
    }

    #[test]
    fn sphere_sizes() {
        // The integer points with i^2+j^2+k^2 <= floor(r^2), for radii whose square is not
        // near an integer (4: exactly 16).
        let brute = |r2: i32, n: i32| {
            let mut c = 0;
            for k in -n..=n {
                for j in -n..=n {
                    for i in -n..=n {
                        c += usize::from(i * i + j * j + k * k <= r2);
                    }
                }
            }
            c
        };
        assert_eq!(sphere(4.0).len(), brute(16, 4));
        assert_eq!(sphere(0.5 * 18.3 + 0.001).len(), brute(83, 9));
        assert_eq!(sphere(18.3).len(), brute(334, 18));
        assert_eq!(sphere(4.0).len(), 257);
    }

    #[test]
    fn percentile_mean() {
        let mut v: Vec<f32> = (0..101).rev().map(|i| i as f32).collect();
        // q1 = (int)(0.7*100) = 70 (0.7f*100 rounds to 70.0000...), q2 = 80.
        let q1 = (0.01f32 * 70.0 * 100.0) as usize;
        let q2 = (0.01f32 * 80.0 * 100.0) as usize;
        let want: f32 = (q1..=q2).map(|i| i as f32).sum::<f32>() / ((q2 - q1) as f32 + 1.0);
        assert_eq!(percmean(&mut v, 70.0, 80.0), want);
        assert_eq!(percmean(&mut [3.0], 70.0, 80.0), 3.0);
        assert_eq!(percmean(&mut [], 70.0, 80.0), 0.0);
    }

    #[test]
    fn duplo_down_and_up() {
        let g = Grid::new(4, 4, 4);
        let im: Vec<f32> = (0..64).map(|i| i as f32).collect();
        let (d, h) = double_down(&im, g);
        assert_eq!(h, Grid::new(2, 2, 2));
        assert_eq!(d.len(), 8);
        let (u, h) = double_up(&d, h, [false, false, false]);
        assert_eq!(h, g);
        assert_eq!(u.len(), 64);
        let (_, h) = double_up(&d, Grid::new(2, 2, 2), [true, false, true]);
        assert_eq!(h, Grid::new(5, 4, 5));
    }
}
