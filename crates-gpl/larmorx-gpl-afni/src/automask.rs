// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09: src/3dAutomask.c (the work of main after the option
// scan: the dilation and erosion loop, the cut-plane report, the -SI clipping),
// src/thd_automask.c (THD_automask, thd_apply_mask), src/thd_median.c (THD_aveabs_brick),
// src/thd_dsetto1D.c (THD_extract_array), src/mri_to_float.c (mri_to_float,
// mri_scale_to_float) and src/thd_initdblk.c (THD_need_brick_factor).
// 3dAutomask.c, thd_automask.c and thd_median.c carry no Medical College of Wisconsin notice:
// they were written at the NIH and are in the public domain. thd_dsetto1D.c, mri_to_float.c and
// thd_initdblk.c: Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI,
// Robert W. Cox et al.), released under the GNU GPL version 2 or any later version.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! `3dAutomask`'s work on a dataset: the average of |values| over the sub-bricks, the mask
//! ([`crate::mask`]), the dilations and erosions of `-dilate` and `-erode`, the report of empty
//! planes and the `-SI` clipping, and the masked copy of `-apply_prefix`.
//!
//! **Upstream defect AFNI-01 (D13).** Averaging a dataset of several sub-bricks, AFNI applies a
//! brick factor only when it is positive (`THD_extract_array`), so a negative `scl_slope` is
//! dropped, with its magnitude; a single sub-brick gets it (`mri_scale_to_float`). By default
//! larmorx-gpl applies the factor whatever its sign ([`aveabs`]); the mask is nearly the same,
//! since the clip level scales with the data, and the notice is given only where the masks
//! differ (the caller computes both).

use larmorx_core::defects::UpstreamDefects;

use crate::dataset::{self, BrickData, Dataset};
use crate::mask::{self, Grid, MaskSettings};
use crate::message::Message;

/// `THD_need_brick_factor`: a factor other than 0 and 1.
pub fn need_brick_factor(ds: &Dataset) -> bool {
    ds.factors.iter().any(|&f| f != 0.0 && f != 1.0)
}

/// The values of sub-brick `iv` as floats, unscaled (`mri_to_float` without its scan).
fn brick_as_float(data: &BrickData, nvox: usize, iv: usize) -> Vec<f32> {
    let r = iv * nvox..(iv + 1) * nvox;
    match data {
        BrickData::Byte(v) => v[r].iter().map(|&x| f32::from(x)).collect(),
        BrickData::Short(v) => v[r].iter().map(|&x| f32::from(x)).collect(),
        BrickData::Float(v) => v[r].to_vec(),
    }
}

/// `mri_scale_to_float(fac, brick)`: the values times `fac` (as they are when `fac` is 0 or 1),
/// non-finite results 0.
pub fn scale_to_float(data: &BrickData, nvox: usize, iv: usize, fac: f32) -> Vec<f32> {
    let mut v = brick_as_float(data, nvox, iv);
    if fac != 0.0 && fac != 1.0 {
        for x in &mut v {
            *x *= fac;
        }
    }
    dataset::floatscan(&mut v);
    v
}

/// `THD_aveabs_brick`: the mean of |value| over the sub-bricks at each voxel (one sub-brick:
/// |value| times its factor). Several sub-brick are read as `THD_extract_array` reads them:
/// non-finite values 0, then each factor applied when it is positive (AFNI-01 reproduced) or,
/// with `UpstreamDefects::Fix`, whatever its sign; the sum in float, times `(float)(1.0/nvals)`.
pub fn aveabs(ds: &Dataset, mode: UpstreamDefects) -> Vec<f32> {
    let nvox = ds.nxyz.iter().product::<usize>();
    let nvals = ds.nvals;
    if nvals == 1 {
        let mut v = scale_to_float(&ds.data, nvox, 0, ds.factors[0]);
        for x in &mut v {
            *x = x.abs();
        }
        return v;
    }
    let fac = (1.0 / nvals as f64) as f32;
    let scale = need_brick_factor(ds);
    let mut out = vec![0.0f32; nvox];
    let mut series = vec![0.0f32; nvals];
    for (ii, o) in out.iter_mut().enumerate() {
        for (iv, s) in series.iter_mut().enumerate() {
            let k = iv * nvox + ii;
            *s = match &ds.data {
                BrickData::Byte(v) => f32::from(v[k]),
                BrickData::Short(v) => f32::from(v[k]),
                BrickData::Float(v) => v[k],
            };
        }
        dataset::floatscan(&mut series);
        if scale {
            for (s, &f) in series.iter_mut().zip(&ds.factors) {
                if f > 0.0 || (mode.fix() && f != 0.0) {
                    *s *= f;
                }
            }
        }
        let mut sum = 0.0f32;
        for &s in &series {
            sum += s.abs();
        }
        *o = fac * sum;
    }
    out
}

/// Whether averaging `ds` meets AFNI-01: several sub-bricks, one with a negative factor (and a
/// factor other than 0 and 1 somewhere, without which none is applied).
pub fn meets_afni_01(ds: &Dataset) -> bool {
    ds.nvals > 1 && need_brick_factor(ds) && ds.factors.iter().any(|&f| f < 0.0)
}

/// `THD_automask`: the mask of the average of |values| ([`aveabs`]).
pub fn automask(
    ds: &Dataset,
    s: &MaskSettings,
    mode: UpstreamDefects,
    log: &mut Vec<Message>,
) -> Option<Vec<u8>> {
    let [nx, ny, nz] = ds.nxyz;
    let medim = aveabs(ds, mode);
    mask::automask_image(&medim, Grid::new(nx, ny, nz), s, log)
}

/// `thd_apply_mask`: the dataset with every voxel outside the mask 0, in every sub-brick, in
/// the input's storage type and with its brick factors.
pub fn apply_mask(ds: &Dataset, mask: &[u8]) -> Dataset {
    let nvox = mask.len();
    let keep = |i: usize| mask[i % nvox] != 0;
    let data = match &ds.data {
        BrickData::Byte(v) => BrickData::Byte(
            v.iter()
                .enumerate()
                .map(|(i, &x)| if keep(i) { x } else { 0 })
                .collect(),
        ),
        BrickData::Short(v) => BrickData::Short(
            v.iter()
                .enumerate()
                .map(|(i, &x)| if keep(i) { x } else { 0 })
                .collect(),
        ),
        BrickData::Float(v) => BrickData::Float(
            v.iter()
                .enumerate()
                .map(|(i, &x)| if keep(i) { x } else { 0.0 })
                .collect(),
        ),
    };
    Dataset { data, ..ds.clone() }
}

/// 3dAutomask's `-dilate nd` and `-erode ne` (05 Mar 2003): `nd` dilations (a voxel with at
/// least 3 neighbours of level `nn` in the mask joins it), each followed by filling holes up to
/// `nmm` voxels wide (`nmm` = the largest of 1 and `rint(0.032 n)` over the axes); then `ne`
/// erosions, each followed by the same filling; then every hole (a cluster of non-mask voxels
/// other than the largest) is filled. The messages are AFNI's (`verbose`: 3dAutomask's own
/// `verb`; the clustering messages follow the mask settings).
#[allow(clippy::too_many_arguments)] // 3dAutomask's settings, as its `main` holds them
pub fn dilate_erode(
    g: Grid,
    mask: &mut [u8],
    dilate: i32,
    erode: i32,
    nn: u8,
    verbose: bool,
    s: &MaskSettings,
    log: &mut Vec<Message>,
) {
    if dilate == 0 && erode == 0 {
        return;
    }
    let rint = |x: f64| x.round_ties_even() as i32;
    let mut nmm = 1;
    nmm = nmm.max(rint(0.032 * g.nx as f64));
    nmm = nmm.max(rint(0.032 * g.ny as f64));
    nmm = nmm.max(rint(0.032 * g.nz as f64));
    if verbose && dilate != 0 {
        log.push(Message::Info("Dilating automask\n".to_owned()));
    }
    for _ in 0..dilate {
        mask::dilate(g, mask, 3, nn);
        mask::fillin_completely(g, mask, nmm);
    }
    if verbose && erode != 0 {
        log.push(Message::Info("Eroding automask\n".to_owned()));
    }
    for _ in 0..erode {
        mask::erode(g, mask, nn, s, log);
        mask::fillin_completely(g, mask, nmm);
    }
    for m in mask.iter_mut() {
        *m = u8::from(*m == 0);
    }
    mask::clust(g, mask, s, log);
    for m in mask.iter_mut() {
        *m = u8::from(*m == 0);
    }
}

/// `ORIENT_tinystr`: the two letters of an orientation code.
pub fn tinystr(o: Option<u8>) -> [char; 2] {
    match o {
        Some(0) => ['R', 'L'],
        Some(1) => ['L', 'R'],
        Some(2) => ['P', 'A'],
        Some(3) => ['A', 'P'],
        Some(4) => ['I', 'S'],
        Some(5) => ['S', 'I'],
        _ => ['?', '?'],
    }
}

/// The cut-plane report (04 Jun 2002) and the `-SI` clipping (06 Mar 2003) of 3dAutomask:
/// for each axis, how many planes at each end are empty, as INFO lines; with `si` positive,
/// on the axis whose first or last end is superior, everything more than `si` mm inferior to
/// the most superior voxel is removed. Returns the number of mask voxels when clipping
/// happened.
pub fn planes_and_si(
    ds: &Dataset,
    mask: &mut [u8],
    si: f32,
    verbose: bool,
    log: &mut Vec<Message>,
) -> Option<i32> {
    let [nx, ny, nz] = ds.nxyz;
    let nxy = nx * ny;
    let orient = ds.geometry.axes.orient;
    let tiny = |axis: usize| tinystr(orient.map(|o| o[axis]));
    let del = ds.geometry.axes.del;
    let any_x =
        |ii: usize, m: &[u8]| (0..nz).any(|kk| (0..ny).any(|jj| m[ii + jj * nx + kk * nxy] != 0));
    let any_y =
        |jj: usize, m: &[u8]| (0..nz).any(|kk| (0..nx).any(|ii| m[ii + jj * nx + kk * nxy] != 0));
    let any_z =
        |kk: usize, m: &[u8]| (0..ny).any(|jj| (0..nx).any(|ii| m[ii + jj * nx + kk * nxy] != 0));
    // `(int)(SIhh/fabs(DSET_DX)+0.5)`.
    let si_planes = |d: f32| (f64::from(si) / f64::from(d).abs() + 0.5) as i64;
    let (mut siax, mut sibot, mut sitop) = (0usize, 0i64, 0i64);
    let say = |log: &mut Vec<Message>, text: String| {
        if verbose {
            log.push(Message::Info(text));
        }
    };
    // `ii` is the first non-empty plane (n when there is none); `last` the last (-1 for none).
    let first =
        |n: usize, any: &dyn Fn(usize) -> bool| (0..n).find(|&i| any(i)).unwrap_or(n) as i64;
    let last = |n: usize, any: &dyn Fn(usize) -> bool| {
        (0..n).rev().find(|&i| any(i)).map_or(-1, |i| i as i64)
    };
    let axes: [(usize, usize, char, f32); 3] = [
        (0, nx, 'x', del[0]),
        (1, ny, 'y', del[1]),
        (2, nz, 'z', del[2]),
    ];
    for (axis, n, name, d) in axes {
        let any = |i: usize| match axis {
            0 => any_x(i, mask),
            1 => any_y(i, mask),
            _ => any_z(i, mask),
        };
        let f = first(n, &any);
        say(
            log,
            format!(
                "first {f:3} {name}-planes are zero [from {}]\n",
                tiny(axis)[0]
            ),
        );
        if si > 0.0 && tiny(axis)[0] == 'S' {
            siax = axis + 1;
            sibot = f + si_planes(d);
            sitop = n as i64 - 1;
        }
        let l = last(n, &any);
        say(
            log,
            format!(
                "last  {:3} {name}-planes are zero [from {}]\n",
                n as i64 - 1 - l,
                tiny(axis)[1]
            ),
        );
        if si > 0.0 && tiny(axis)[1] == 'S' {
            siax = axis + 1;
            sibot = 0;
            sitop = l - si_planes(d);
        }
    }
    if siax > 0 && sibot <= sitop {
        let cax = ['x', 'y', 'z'][siax - 1];
        say(
            log,
            format!("SI clipping mask along axis {cax} from {sibot}..{sitop}\n"),
        );
        for kk in 0..nz {
            for jj in 0..ny {
                for ii in 0..nx {
                    let c = [ii, jj, kk][siax - 1] as i64;
                    if c >= sibot && c <= sitop {
                        mask[ii + jj * nx + kk * nxy] = 0;
                    }
                }
            }
        }
        let n = mask::count(mask);
        say(log, format!("{n} voxels left [out of {}]\n", nxy * nz));
        return Some(n);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tinystr_codes() {
        assert_eq!(tinystr(Some(0)), ['R', 'L']);
        assert_eq!(tinystr(Some(5)), ['S', 'I']);
        assert_eq!(tinystr(None), ['?', '?']);
    }
}
