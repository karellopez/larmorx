// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05: src/3dUnifize.c (mri_clipedges_inplace, median7,
// mri_double_down, mri_double_up, mri_local_percmean, mri_WMunifize, mri_GMunifize),
// src/mri_invert.c (mri_invertcontrast_inplace), src/thd_dset_nbhd.c (mri_get_nbhd_array, with
// SetSearchAboutMaskedVoxel), src/edt_buildmask.c (MCW_build_mask, MCW_spheremask), src/cs_qmed.c
// (qmed_float, median_float4, median_float5, median_float7, median_float9), src/mri_percents.c
// (qsort_float, as a sort) and src/thd_median.c (THD_median_brick). 3dUnifize.c, mri_invert.c,
// thd_dset_nbhd.c and thd_median.c carry no Medical College of Wisconsin notice: they were written
// at the NIH and are in the public domain. edt_buildmask.c, cs_qmed.c and mri_percents.c: Major
// portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released by AFNI 26.3.05 under the Creative Commons Attribution 4.0 International licence (CC BY
// 4.0).
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05 (first
// translated from AFNI_25.2.09).
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! The steps of `3dUnifize`, as AFNI 26.3.05 computes them.
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
//! is independent, so larmorx computes them in parallel, with the same result at any thread
//! count; within a voxel only the values between the two percentiles are sorted (a selection
//! puts them in place), and they are summed in sorted order, as AFNI sums them.

use larmorx_core::defects::UpstreamDefects;
use rayon::prelude::*;

use crate::fdlibm::tanhf;
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
// `!(a < b)` is AFNI's loop condition, kept as written (the values are finite).
#[allow(clippy::neg_cmp_op_on_partial_ord)]
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
/// order. `target`, when given, is the output grid instead (AFNI-47 corrected: an axis halved
/// to 1 voxel goes back to its 2 or 3).
pub fn double_up(far: &[f32], g: Grid, add: [bool; 3], target: Option<Grid>) -> (Vec<f32>, Grid) {
    let (nxf, nyf, nzf) = (g.nx, g.ny, g.nz);
    let up = |n: usize, a: bool| if n == 1 { 1 } else { 2 * n + usize::from(a) };
    let h = target.unwrap_or_else(|| Grid::new(up(nxf, add[0]), up(nyf, add[1]), up(nzf, add[2])));
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

/// The `-EPI` radius from a cube root `c`: `18.3 / c` in double, as float, at least 4.14 (as
/// 3dUnifize's `main` computes it).
fn radius_from_cube_root(c: f32) -> f32 {
    let mut uu = (18.3 / f64::from(c)) as f32;
    if uu < 4.14 {
        uu = 4.14;
    }
    uu
}

/// The cube root of the voxel volume that `-EPI`'s radius takes: CORE-MATH's correctly rounded
/// `cbrtf` by default ([`larmorx_core::math::cbrtf`], MIT); with `reproduce` glibc 2.35's,
/// which AFNI's Linux builds call ([`larmorx_core::math::cbrtf_glibc`]: larmorx's clean-room
/// function with glibc 2.35's bits, written from `specs/cbrtf.md` and proved equal to the
/// system's `cbrtf` on all 2^32 inputs; GLIBC-05).
pub fn epi_cube_root(volume: f32, mode: UpstreamDefects) -> f32 {
    if mode.fix() {
        larmorx_core::math::cbrtf(volume)
    } else {
        larmorx_core::math::cbrtf_glibc(volume)
    }
}

/// 3dUnifize `-EPI`'s radius in voxels for the voxel volume `volume` (`|dx * dy * dz|`, a float
/// product): `18.3 / cbrtf(volume)`, at least 4.14, with the cube root of [`epi_cube_root`]:
/// AFNI's radius bit for bit with `reproduce`; by default the correctly rounded one, which
/// differs from AFNI's by one unit in the last place wherever glibc's `cbrtf` does
/// (453,492,162 of the 2^32 inputs) and the quotient keeps it ([`epi_cbrtf_notice`] names
/// GLIBC-05 where that changes the result).
pub fn epi_radius(volume: f32, mode: UpstreamDefects) -> f32 {
    radius_from_cube_root(epi_cube_root(volume, mode))
}

/// How many offsets of the ball of `radius` ([`sphere`], the centre included) lie within the
/// reach of a grid of `dims` voxels: `|i| < nx`, `|j| < ny`, `|k| < nz`, the only offsets a
/// voxel of the grid ever uses. The ball's float sums of squares are tested in [`sphere`]'s
/// order; the work is bounded by the grid, so even a radius beyond any machine is counted.
pub fn ball_in_reach(radius: f32, dims: [usize; 3]) -> u64 {
    let n = radius as i32;
    if n < 1 {
        return 1;
    }
    let reach = |d: usize| i64::from(n).min(d.saturating_sub(1) as i64) as i32;
    let (ri, rj, rk) = (reach(dims[0]), reach(dims[1]), reach(dims[2]));
    let dist_q = radius * radius;
    let mut count = 1u64;
    for kk in -rk..=rk {
        let zq = (kk as f32) * (kk as f32);
        for jj in -rj..=rj {
            let yq = zq + (jj as f32) * (jj as f32);
            for ii in -ri..=ri {
                let xq = yq + (ii as f32) * (ii as f32);
                if xq <= dist_q && xq > 0.0 {
                    count += 1;
                }
            }
        }
    }
    count
}

/// GLIBC-05: where glibc 2.35's `cbrtf` (AFNI's) gives `-EPI` another radius than the correctly
/// rounded cube root, and that changes the result on a grid of `dims` (the ball of neighbours
/// within the grid's reach, [`ball_in_reach`], or the radius printed with one decimal), the
/// notice's detail naming the two radii and what differs, in `mode`'s words (the run's radius
/// first). `None` where the two radii are the same or give the same result. The balls of two
/// radii are nested, so their offsets within reach differ exactly when their counts do.
pub fn epi_cbrtf_notice(volume: f32, dims: [usize; 3], mode: UpstreamDefects) -> Option<String> {
    let (uc, ug) = (
        larmorx_core::math::cbrtf(volume),
        larmorx_core::math::cbrtf_glibc(volume),
    );
    let (rc, rg) = (radius_from_cube_root(uc), radius_from_cube_root(ug));
    if rc.to_bits() == rg.to_bits() || (rc.is_nan() && rg.is_nan()) {
        return None;
    }
    let (pc, pg) = (
        format!("{:.1}", f64::from(rc)),
        format!("{:.1}", f64::from(rg)),
    );
    let (nc, ng) = (ball_in_reach(rc, dims), ball_in_reach(rg, dims));
    let (ball, printed) = (nc != ng, pc != pg);
    if !ball && !printed {
        return None;
    }
    // What the other radius would change: its ball within the grid, its printed value.
    let effects = |n_other: u64, n_this: u64, p_other: &str, p_this: &str| {
        let mut out = String::new();
        if ball {
            out += &format!(
                ", a ball of neighbours of {n_other} offsets within the grid instead of {n_this}"
            );
        }
        if printed {
            out += &format!(", printed {p_other} instead of {p_this}");
        }
        out
    };
    Some(if mode.fix() {
        format!(
            "-EPI's radius is {rc} voxels: 18.3 mm over the correctly rounded cube root ({uc}) \
             of the voxel volume, {volume} mm^3; with glibc 2.35's cbrtf ({ug}), which AFNI \
             calls, it is {rg} voxels{}",
            effects(ng, nc, &pg, &pc)
        )
    } else {
        format!(
            "-EPI's radius is {rg} voxels, as AFNI's: 18.3 mm over glibc 2.35's cbrtf ({ug}) of \
             the voxel volume, {volume} mm^3; with the correctly rounded cube root ({uc}) it is \
             {rc} voxels{}",
            effects(nc, ng, &pc, &pg)
        )
    })
}

/// The bytes [`sphere`] and the neighbourhood buffers of [`local_percmean`]'s `threads` workers
/// need for a ball of `radius` voxels (AFNI-75): the ball's volume `4/3 pi r^3` in points, 12
/// bytes each, twice over while the offsets' vector grows, and 4 bytes a point per worker.
pub fn ball_bytes(radius: f32, threads: usize) -> f64 {
    let r = f64::from(radius.max(0.0));
    let points = 4.0 / 3.0 * std::f64::consts::PI * r * r * r + 1.0;
    points * (24.0 + 4.0 * threads.max(1) as f64)
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

/// AFNI's progress letters (`fprintf(stderr, "A")`, ...), in order with the automask's
/// messages: appended to the same log as [`Message::Raw`] text when `verbose`.
fn letter(verbose: bool, log: &mut Vec<Message>, s: &str) {
    if verbose {
        log.push(Message::Raw(s.to_owned()));
    }
}

/// Whether `mri_local_percmean(fim, vrad, p1, p2)` runs (AFNI returns NULL otherwise): a radius
/// of at least 4 and percentiles in [0, 100] (in either order); equal percentiles take another
/// routine (`mri_local_percentile`), which 3dUnifize's command line cannot reach.
pub fn percmean_args_ok(vrad: f32, p1: f32, p2: f32) -> bool {
    let (p1, p2) = if p1 > p2 { (p2, p1) } else { (p1, p2) };
    !(vrad < 4.0 || p1 < 0.0 || p2 > 100.0 || p1 == p2)
}

/// The first part of `mri_local_percmean`: `A`, then the image (a float copy, non-finite values
/// 0) with every voxel outside its automask 0, which `-amsave` writes. `None` where AFNI's
/// automask is NULL (a 2D image smaller than 16 x 16, or with nothing above its clip level).
pub fn masked_input(
    fim: &[f32],
    g: Grid,
    settings: &MaskSettings,
    verbose: bool,
    log: &mut Vec<Message>,
) -> Option<Vec<f32>> {
    letter(verbose, log, "A");
    let mut aar = fim.to_vec();
    crate::dataset::floatscan(&mut aar);
    let ams = mask::automask_image(&aar, g, settings, log)?;
    for (a, &m) in aar.iter_mut().zip(&ams) {
        if m == 0 {
            *a = 0.0;
        }
    }
    Some(aar)
}

/// The rest of `mri_local_percmean(fim, vrad, p1, p2)`, on the automasked image `aar`
/// ([`masked_input`]): the mean between percentiles `p1` and `p2` (either order) of the non-zero
/// values in a ball of radius `vrad` voxels around each voxel, on the half-size grid with half
/// the radius plus 0.001 when `do_double` (`D`), expanded back (`U`). `expand_to`: the grid to
/// expand the half-size result to, when it differs from what AFNI's `mri_double_up` gives
/// (AFNI-47 corrected); `None` for AFNI's.
#[allow(clippy::too_many_arguments)]
pub fn local_percmean(
    aar: Vec<f32>,
    g: Grid,
    vrad: f32,
    p1: f32,
    p2: f32,
    do_double: bool,
    expand_to: Option<Grid>,
    verbose: bool,
    log: &mut Vec<Message>,
    n_threads: usize,
) -> Vec<f32> {
    let (p1, p2) = if p1 > p2 { (p2, p1) } else { (p1, p2) };
    if do_double {
        letter(verbose, log, "D");
    }
    // `mri_free(aim)` once the half-size copy exists (memory as AFNI's, 2026-10-30).
    let (bar, bg) = if do_double {
        let half = double_down(&aar, g);
        drop(aar);
        half
    } else {
        (aar, g)
    };
    let radius = if do_double {
        0.5f32 * vrad + 0.001f32
    } else {
        vrad
    };
    let nbhd = sphere(radius);
    letter(verbose, log, "V");
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
    // The local means only (`cim`, 4 bytes a voxel, as AFNI's); the dots' voxels are looked at
    // again below (memory as AFNI's, 2026-10-30).
    let all = || -> Vec<f32> {
        (0..bg.nvox())
            .into_par_iter()
            .map_init(
                || Vec::with_capacity(nbhd.len()),
                |nbar, vvv| one(vvv, nbar).0,
            )
            .collect()
    };
    let car = larmorx_core::parallel::with_threads(n_threads, all).unwrap_or_else(|_| {
        let mut nbar = Vec::with_capacity(nbhd.len());
        (0..bg.nvox()).map(|v| one(v, &mut nbar).0).collect()
    });
    if verbose {
        // `if( verb && vvv%66666==0 ) fprintf(stderr,".")`, for voxels with two values or more.
        let mut nbar = Vec::with_capacity(nbhd.len());
        let dots = (0..bg.nvox())
            .step_by(66666)
            .filter(|&v| one(v, &mut nbar).1)
            .count();
        if dots > 0 {
            log.push(Message::Raw(".".repeat(dots)));
        }
    }
    let dim = if do_double {
        double_up(
            &car,
            bg,
            [g.nx % 2 == 1, g.ny % 2 == 1, g.nz % 2 == 1],
            expand_to,
        )
        .0
    } else {
        car
    };
    letter(verbose, log, "U");
    dim
}

/// Whether the half-size grid of `g` expands back to another grid ([`double_up`] keeps an axis
/// of 1 voxel at 1, and halving an axis of 2 or 3 voxels gives 1): AFNI-47.
pub fn duplo_changes_grid(g: Grid) -> bool {
    [g.nx, g.ny, g.nz].iter().any(|&n| n == 2 || n == 3)
}

/// The result of [`wm_unifize`]: the scaled image and the scale image (`sclim`, `-ssave`).
pub struct WmResult {
    pub image: Vec<f32>,
    pub scale: Vec<f32>,
    /// How many values were squashed.
    pub squashed: usize,
}

/// The end of `mri_WMunifize`, given the local white-matter intensity `par` ([`local_percmean`]):
/// every value of `fim` (a float copy, non-finite values 0) times `1000/WMI` (0 where WMI is
/// not positive), then, with `squash`, values above 1300 become
/// `1300 + 200 tanhf((x - 1300)/200)` (`W`, then `[s<count>]`). A `par` shorter than the image
/// (AFNI-47 reproduced: AFNI reads past its end) counts as 0 beyond its end.
pub fn wm_unifize(
    fim: &[f32],
    mut par: Vec<f32>,
    squash: bool,
    verbose: bool,
    log: &mut Vec<Message>,
) -> WmResult {
    let mut gar = fim.to_vec();
    crate::dataset::floatscan(&mut gar);
    par.resize(gar.len(), 0.0);
    let mut nsq = 0;
    for (g, p) in gar.iter_mut().zip(par.iter_mut()) {
        let pval = if *p <= 0.0 { 0.0 } else { PKVAL / *p };
        *p = pval;
        *g *= pval;
        if squash && *g > WMCUT {
            *g = WMCUT + WMSCL * tanhf((*g - WMCUT) / WMSCL);
            nsq += 1;
        }
    }
    letter(verbose, log, "W");
    if nsq > 0 {
        letter(verbose, log, &format!("[s{nsq}]"));
    }
    WmResult {
        image: gar,
        scale: par,
        squashed: nsq,
    }
}

/// `mri_GMunifize`: from the WM-unifized image, the median `P` of the values above 1000,
/// `pupper = 1000 - 1.987654321 (P - 1000)`, `plower` the clip level for 0.4321, the median
/// `G` of the values in `[plower, pupper]`; then every value at or above `plower/3`
/// (`0.333f`) becomes `(1000-666)/(1000-G) (x - 1000) + 1000` (negative results 0), the
/// others 0 (`G`). Nothing when fewer than 111 values are above 1000, or in the range.
pub fn gm_unifize(gar: &mut [f32], verbose: bool, log: &mut Vec<Message>) {
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
    letter(verbose, log, "G");
}

/// `mri_clipedges_inplace(im, uclip, vclip)` (`e`): repeatedly, voxels at or above `uclip`
/// with more than 2 of their 6 face neighbours 0, and voxels at or above `vclip` with more than
/// 4, become 0 (both tests on the image as it was at the start of the pass), until a pass clips
/// nothing.
pub fn clip_edges(
    qmar: &mut [f32],
    g: Grid,
    uclip: f32,
    vclip: f32,
    verbose: bool,
    log: &mut Vec<Message>,
) {
    let (nx, ny, nz, nxy) = (g.nx, g.ny, g.nz, g.nx * g.ny);
    letter(verbose, log, "e");
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
    // In place, with only the mask's values copied for the sort, as AFNI's (no copy of the whole
    // image; memory as AFNI's, 2026-10-30): the values are those of the image with its
    // non-finite values 0, which it becomes only when it is inverted.
    let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
    let mut hist: Vec<f32> = im
        .iter()
        .zip(mask)
        .filter(|&(_, &m)| m != 0)
        .map(|(&v, _)| finite(v))
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
    drop(hist);
    crate::dataset::floatscan(im);
    for (v, &m) in im.iter_mut().zip(mask) {
        if m != 0 && *v > 0.0 {
            *v = ucut - *v;
        }
        if m == 0 || *v < 0.0 {
            *v = 0.0;
        }
    }
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
    fn epi_radius_and_glibc_05() {
        use UpstreamDefects::{Fix, Reproduce};
        // 3 mm voxels: 18.3 / 3 with either cube root; no notice.
        assert_eq!(epi_radius(27.0, Fix), 6.1);
        assert_eq!(epi_radius(27.0, Reproduce), 6.1);
        assert!(epi_cbrtf_notice(27.0, [64, 64, 40], Fix).is_none());
        // Large voxels: the minimum radius.
        assert_eq!(epi_radius(1000.0, Fix), 4.14);
        // 11.969702 mm^3 (`specs/cbrtf.md` §6): glibc's cube root gives a radius of exactly 8
        // (2109 offsets), the correctly rounded one 7.9999995 (2103 offsets).
        let v = f32::from_bits(0x413f_83e6);
        assert_eq!(epi_radius(v, Fix).to_bits(), 0x40ff_ffff);
        assert_eq!(epi_radius(v, Reproduce), 8.0);
        assert_eq!(sphere(epi_radius(v, Fix)).len(), 2103);
        assert_eq!(sphere(8.0).len(), 2109);
        let dims = [24, 24, 16];
        assert_eq!(ball_in_reach(8.0, dims), 2109);
        assert_eq!(ball_in_reach(epi_radius(v, Fix), dims), 2103);
        assert_eq!(
            epi_cbrtf_notice(v, dims, Fix).unwrap(),
            "-EPI's radius is 7.9999995 voxels: 18.3 mm over the correctly rounded cube root \
             (2.2875001) of the voxel volume, 11.969702 mm^3; with glibc 2.35's cbrtf (2.2875), \
             which AFNI calls, it is 8 voxels, a ball of neighbours of 2109 offsets within the \
             grid instead of 2103"
        );
        assert_eq!(
            epi_cbrtf_notice(v, dims, Reproduce).unwrap(),
            "-EPI's radius is 8 voxels, as AFNI's: 18.3 mm over glibc 2.35's cbrtf (2.2875) of \
             the voxel volume, 11.969702 mm^3; with the correctly rounded cube root (2.2875001) \
             it is 7.9999995 voxels, a ball of neighbours of 2103 offsets within the grid \
             instead of 2109"
        );
        // A grid the extra offsets (8 voxels along an axis) cannot reach: nothing changes.
        assert!(epi_cbrtf_notice(v, [8, 8, 8], Fix).is_none());
        // The printed radius only: 18.05 (18.0) correctly rounded, 18.050001 (18.1) glibc's.
        let printed = f32::from_bits(0x3f85_647f);
        let n = epi_cbrtf_notice(printed, dims, Fix).unwrap();
        assert!(
            n.ends_with("it is 18.050001 voxels, printed 18.1 instead of 18.0"),
            "{n}"
        );
        // Radii one unit apart in the last place with the same ball and print: no notice.
        let same = 2.4f32 * 2.4 * 2.78;
        assert_ne!(epi_radius(same, Fix), epi_radius(same, Reproduce));
        assert!(epi_cbrtf_notice(same, dims, Fix).is_none());
        assert!(epi_cbrtf_notice(same, dims, Reproduce).is_none());
        // A zero volume: an infinite radius with both cube roots, nothing to name.
        assert!(epi_radius(0.0, Fix).is_infinite() && epi_radius(0.0, Reproduce).is_infinite());
        assert!(epi_cbrtf_notice(0.0, dims, Fix).is_none());
        // The count within reach is the whole ball where the grid reaches every offset.
        for r in [4.14f32, 6.1, 7.5, 9.15] {
            assert_eq!(
                ball_in_reach(r, [64, 64, 64]),
                sphere(r).len() as u64,
                "{r}"
            );
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
        let (u, h) = double_up(&d, h, [false, false, false], None);
        assert_eq!(h, g);
        assert_eq!(u.len(), 64);
        let (_, h) = double_up(&d, Grid::new(2, 2, 2), [true, false, true], None);
        assert_eq!(h, Grid::new(5, 4, 5));
        // An axis halved to 1 stays 1 (AFNI-47), unless a target grid is given.
        let (_, h) = double_up(&[1.0; 4], Grid::new(1, 2, 2), [true, false, false], None);
        assert_eq!(h, Grid::new(1, 4, 4));
        let t = Grid::new(3, 4, 4);
        let (v, h) = double_up(&[1.0; 4], Grid::new(1, 2, 2), [true, false, false], Some(t));
        assert_eq!((h, v.len()), (t, 48));
        assert!(duplo_changes_grid(Grid::new(3, 400, 400)) && !duplo_changes_grid(g));
    }
}
