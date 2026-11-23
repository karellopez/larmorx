// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRImodeFilter` (FreeSurfer 8.2.0's `utils/mrifilter.cpp` 5712-5772; track A relay 54, queue
//! item 3.2-38 part 2: `mri_vol2vol --morph --filter`).

use std::io::Write;

use larmorx_core::defects::FS_1192;
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{mri_clone, mri_val_range};

/// The histogram of one neighbourhood. FreeSurfer's has the bins `0..=max_val` only; a value
/// below 0 (or one whose `nint` is not a number) is counted outside it (FS-1192): by default the
/// histogram also has the bins down to the smallest value; with `reproduce` such a count is lost
/// and such a bin reads 0.
struct Histo {
    low: i64,
    bins: Vec<i32>,
}

impl Histo {
    fn slot(&self, v: i32) -> Option<usize> {
        let k = i64::from(v) - self.low;
        if k < 0 || k >= self.bins.len() as i64 {
            None
        } else {
            Some(k as usize)
        }
    }

    fn add(&mut self, v: i32) {
        if let Some(k) = self.slot(v) {
            self.bins[k] += 1;
        }
    }

    fn get(&self, v: i32) -> i32 {
        self.slot(v).map_or(0, |k| self.bins[k])
    }
}

/// `MRImodeFilter(mri_src, mri_dst, niter)`: `niter` passes over frame 0 of a copy of the source
/// (`MRIcopy`): each voxel (z slowest, then y, then x) takes the most frequent of the `nint`
/// values of its 3 x 3 x 3 neighbourhood (the volume's edge repeated), the first value in
/// increasing order whose count passes the voxel's own (`(int)` of its value) winning; after
/// each pass the result is copied back. The values counted are `0..=ceil(max)` (`MRIvalRange`
/// over every frame); a maximum of 0 (`|max| < 1e-15`) returns the copy itself. The output: a new
/// volume like the source (`MRIclone`: its other frames 0) or the caller's. FS-1192: FreeSurfer
/// counts a value below 0 (or a value or maximum that is not a number or beyond the `int` range)
/// outside its histogram (memory it does not own; the official build's result then depends on
/// memory contents or it crashes): by default such values are counted in bins of their own,
/// with `reproduce` their counts are lost (a notice either way).
pub fn mri_mode_filter(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<Mri>,
    niter: i32,
) -> Result<Mri, Fatal> {
    let mut tmp = mri_copy(g, out, err, src, None)?.unwrap_new();
    let (_, fmin, fmax) = mri_val_range(g, err, src)?;
    if f64::from(fmax).abs() < 1e-15 {
        return Ok(tmp);
    }
    const WHAT: &str = "MRImodeFilter";
    let max_val = float_to_int(g, f64::from(fmax).ceil(), WHAT);
    let reproduce = g.notices.reproduce();
    let mut low = 0i64;
    let mut high = i64::from(max_val);
    if !reproduce {
        if fmin < -0.5 || fmin.is_nan() {
            low = i64::from(nint(g, f64::from(fmin))).min(0);
        }
        if fmax.is_nan() {
            high = 0;
        }
    }
    if high < low {
        // FreeSurfer's calloc of max_val + 1 <= 0 ints fails, and the program crashes
        g.notices.add(
            &FS_1192,
            format!("{WHAT}: a maximum of {fmax} makes no histogram (FreeSurfer crashes)"),
        );
        return Err(Fatal::Exit(139));
    }
    let mut h = Histo {
        low,
        bins: vec![0; (high - low + 1) as usize],
    };
    // a value FreeSurfer counts outside its bins 0..=max_val
    let mut outside = false;
    let beyond = |v: f32, i: i32| v.is_nan() || i < 0 || i > max_val;
    let mut dst = match dst {
        Some(d) => d,
        None => mri_clone(g, out, err, src, None)?.unwrap_new(),
    };
    let (w, hh, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let (xt, yt, zt) = (
        src.xi.as_ref().expect("the index tables"),
        src.yi.as_ref().expect("the index tables"),
        src.zi.as_ref().expect("the index tables"),
    );
    for _ in 0..niter.max(0) {
        for z in 0..d {
            for y in 0..hh {
                for x in 0..w {
                    h.bins.iter_mut().for_each(|b| *b = 0);
                    let v0 = mri_get_vox_val(g, err, &tmp, x, y, z, 0, 0)?;
                    let start = float_to_int(g, f64::from(v0), WHAT);
                    outside |= beyond(v0, start);
                    for zk in -1..=1 {
                        let zi = Mri::index(zt, z + zk);
                        for yk in -1..=1 {
                            let yi = Mri::index(yt, y + yk);
                            for xk in -1..=1 {
                                let xi = Mri::index(xt, x + xk);
                                let v = mri_get_vox_val(g, err, &tmp, xi, yi, zi, 0, 0)?;
                                let val = nint(g, f64::from(v));
                                outside |= beyond(v, val);
                                h.add(val);
                            }
                        }
                    }
                    let mut max_histo = h.get(start);
                    let mut max_i = start;
                    for k in 0..h.bins.len() {
                        if h.bins[k] > max_histo {
                            max_histo = h.bins[k];
                            max_i = (low + k as i64) as i32;
                        }
                    }
                    mri_set_vox_val(g, err, &mut dst, x, y, z, 0, max_i as f32, 0)?;
                }
            }
        }
        mri_copy(g, out, err, &dst, Some(&mut tmp))?;
    }
    if outside {
        g.notices.add(
            &FS_1192,
            format!(
                "{WHAT}: values from {fmin} to {fmax} counted outside FreeSurfer's histogram of 0 to {max_val} (memory it does not own)"
            ),
        );
    }
    Ok(dst)
}
