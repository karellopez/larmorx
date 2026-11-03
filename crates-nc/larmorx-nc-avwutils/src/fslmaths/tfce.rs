// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-avwutils 2209.6 (fslmaths.cc: -tfce, -tfceS,
// -tfce_minT, -tfce_delta, -roc) and fsl-newimage 2601.0 (newimagefns.h: tfce, tfce_support).
// Copyright (C) 2000-2008 University of Oxford (Steve Smith, David Flitney, Mark Jenkinson,
// Stuart Clare, Thomas Nichols and Matthew Webster). Rust translation Copyright 2026 Karel Lopez
// Vilaret; modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! fslmaths' threshold-free cluster enhancement (`-tfce`, `-tfceS`) and ROC analysis (`-roc`).
//!
//! - **`-tfce H E connectivity`** (newimagefns' fast `tfce`, each volume on its own): the
//!   interior voxels above `-tfce_minT` sorted by value; for each threshold from `minT` in steps
//!   of `-tfce_delta` (0: the volume's maximum over 100) while below the maximum plus the step
//!   (`float` sums), the voxels above it joined into clusters by `connectivity`, and each one's
//!   enhancement increased by `pow(size, E) * powf(h, H)` (glibc's `pow` and `powf`), a `double`
//!   added to the `float` enhancement. The edge voxels stay 0. The enhancement of a voxel does
//!   not depend on the order of equal values in the sort or on the clusters' numbering.
//! - **`-tfceS H E connectivity X Y Z tfce_thresh`** (`tfce_support`): the cluster holding
//!   voxel (X, Y, Z) (NIfTI's voxel order) followed from `minT` up to the voxel's value, then
//!   the sum of `powf(size, E) * powf(h, H)` from the highest threshold down until it reaches
//!   `tfce_thresh`; the image keeps the voxel's cluster at the threshold reached.
//! - **`-roc AROC-thresh outfile [noise4D] truth`**: the image's and the noise image's values
//!   log transformed (`log(v - min + 1)`, `logf` or `log` by the type), thresholds from the
//!   maximum down in 1000 steps, the true and false positive rates per volume, the curve written
//!   to `outfile` and the area (with its first and third quartiles over the volumes) printed.
//! - **D13.** FSL-153 (a NaN first voxel in the maxima and minima), FSL-173 (the sorted list read
//!   one past its end: bounds tested first, no change), FSL-174 (a step that cannot advance),
//!   FSL-175 (an automatic step of an image without positive values), FSL-176 (connectivities
//!   other than 6, 18, 26), FSL-177 (`-tfceS`'s arrays), FSL-178 (a voxel outside the image),
//!   FSL-179 (a noise image of another size), FSL-180 (a ROC without true or noise voxels).

use std::collections::VecDeque;

use super::*;
use larmorx_nc_core::glibc_float;
use larmorx_nc_core::sse;
use larmorx_nc_core::vstats;

/// `-tfce_minT` and `-tfce_delta` as fslmaths keeps them (`float`, both 0 at the start).
#[derive(Clone, Copy, Default)]
pub(super) struct TfceParams {
    pub min_t: f32,
    pub delta: f32,
}

/// The error of a rule-B correction (FSL goes on with an input that has no meaning).
fn rule_b(ctx: &mut Ctx, d: &'static Defect, what: &str) -> Stop {
    ctx.eput(&format!(
        "larmorx-nc: error: fslmaths: {what} ({}; LARMORX_UPSTREAM_DEFECTS=reproduce runs it as \
         FSL does)\n",
        d.id
    ));
    Stop::Exit(1)
}

/// Armadillo's index check (`Mat::operator(): index out of bounds`, `std::out_of_range`).
fn index_error() -> Stop {
    Stop::Exception(FslError {
        printed: Vec::new(),
        type_name: "std::out_of_range",
        what: "Mat::operator(): index out of bounds".into(),
    })
}

/// An index past the end of one of `-tfceS`'s column vectors of `n` elements: Armadillo's
/// `std::out_of_range`, or for an empty one (no column) armawrap's `(i - 1) / Ncols()`, an
/// integer division by zero (SIGFPE: refused in both modes, FSL-177).
fn array_stop(ctx: &mut Ctx, n: u64) -> Stop {
    if n == 0 {
        return ctx.refuse(
            &defects::FSL_177,
            "-tfceS: an index into an empty column vector (a division by zero)",
        );
    }
    index_error()
}

/// FSL-176: a connectivity other than 6, 18 or 26.
fn check_connectivity(ctx: &mut Ctx, conn: i32, op: &str) -> Res<()> {
    if matches!(conn, 6 | 18 | 26) {
        return Ok(());
    }
    ctx.notice(&defects::FSL_176, format!("{op}: a connectivity of {conn}"));
    if ctx.reproduce() {
        return Ok(());
    }
    Err(rule_b(
        ctx,
        &defects::FSL_176,
        &format!("{op}: the connectivity must be 6, 18 or 26, not {conn}"),
    ))
}

/// The maximum (`max`) or minimum of values as FSL takes it (from the first value: FSL-153) or
/// with the NaN values left out (the default); `nan` set where the two differ.
fn extreme<T: Arith>(ctx: &Ctx, v: &[T], max: bool, nan: &mut bool) -> T {
    let Some(f) = vstats::extrema(v) else {
        return T::from_int(0);
    };
    let x = if T::IS_FLOAT {
        vstats::extrema_without_nan(v).unwrap_or(f)
    } else {
        f
    };
    let (a, b) = if max { (f.max, x.max) } else { (f.min, x.min) };
    if bits(a) != bits(b) {
        *nan = true;
    }
    if ctx.reproduce() { a } else { b }
}

/// `(T)` of a `float` result (`copyconvert`): exact for the floating types, C's conversion
/// (FSL-151) for the integers.
fn from_float<T: Arith>(ctx: &mut Ctx, v: f32, what: &str) -> T {
    match T::DTYPE {
        dt::FLOAT => T::from_f32(v),
        dt::DOUBLE => T::from_f64(f64::from(v)),
        _ => ctx.conv::<T>(f64::from(v), what),
    }
}

/// Whether a `float` loop `for (v = from; v < to; v += step)` stops: `false` when a step leaves
/// `v` where it was or moves it down while `v < to` holds (a step not above `v`'s precision, 0,
/// or negative); a NaN step ends it. Counts the steps it takes (at most about 2^32: each one
/// moves `v` up to another float).
fn float_loop(from: f32, to: f32, step: f32) -> (bool, u64) {
    let mut v = from;
    let mut n = 0u64;
    while v < to {
        let next = v + step;
        if next <= v {
            return (false, n);
        }
        v = next;
        n += 1;
    }
    (true, n)
}

// ------------------------------------------------------------------------------------------------
// -tfce

/// `-tfce <H> <E> <connectivity>` on every volume; `Ok(Some(1))` where fslmaths catches tfce's
/// exception (its message, then exit status 1).
pub(super) fn tfce<T: Arith>(
    ctx: &mut Ctx,
    input: &mut Volume<T>,
    argv: &[String],
    i: usize,
    p: TfceParams,
) -> Res<Option<u8>> {
    let h = cnum::atof(number_arg(ctx, argv, i + 1)?) as f32;
    let e = cnum::atof(number_arg(ctx, argv, i + 2)?) as f32;
    let conn = cnum::atoi(number_arg(ctx, argv, i + 3)?);
    check_connectivity(ctx, conn, "-tfce")?;
    if p.delta.is_nan() {
        ctx.notice(&defects::FSL_174, "-tfce: a NaN threshold step");
        if !ctx.reproduce() {
            return Err(rule_b(
                ctx,
                &defects::FSL_174,
                "-tfce: the threshold step (-tfce_delta) is NaN",
            ));
        }
    }
    let dims = [input.xsize(), input.ysize(), input.zsize()];
    let nvox = input.nvoxels().max(0) as usize;
    for t in 0..input.tsize().max(0) as usize {
        let mut vol = input.data[t * nvox..(t + 1) * nvox].to_vec();
        let r = tfce_volume(ctx, &mut vol, dims, h, e, conn, p)?;
        input.data[t * nvox..(t + 1) * nvox].copy_from_slice(&vol);
        if let Err(msg) = r {
            ctx.eput(&format!(
                "ERROR: TFCE failed, please check your file for valid sizes and voxel values.{msg}\n\nExiting\n"
            ));
            return Ok(Some(1));
        }
    }
    Ok(None)
}

/// newimagefns' `tfce(data, H, E, NumConn, minT, deltaT)` of one volume; `Ok(Err(what))` for its
/// `std::runtime_error`s (which fslmaths catches).
fn tfce_volume<T: Arith>(
    ctx: &mut Ctx,
    vol: &mut [T],
    dims: [i64; 3],
    h: f32,
    e: f32,
    conn: i32,
    p: TfceParams,
) -> Res<Result<(), String>> {
    let mut nan_first = false;
    let dmax = extreme(ctx, vol, true, &mut nan_first);
    let max_t = dmax.to_f32();
    let mut delta = p.delta;
    if delta == 0.0 {
        delta = (f64::from(max_t) / 100.0) as f32;
    }
    let mut zeros = false;
    if delta <= 0.0 {
        if p.delta == 0.0 && max_t <= p.min_t {
            // FSL-175: no voxel above the lowest threshold
            ctx.notice(
                &defects::FSL_175,
                "-tfce: a volume without values above the lowest threshold",
            );
            zeros = !ctx.reproduce();
        }
        if !zeros {
            if nan_first && !ctx.reproduce() {
                ctx.notice(&defects::FSL_153, "-tfce: the first voxel is NaN");
            }
            return Ok(Err("Error: tfce requires a positive deltaT input.".into()));
        }
    }
    let [nx, ny, nz] = dims;
    if nx < 3 || ny < 3 || nz < 3 {
        return Ok(Err(
            "Error: tfce currently requires an input with at least 3 voxels extent \
                       into each dimension."
                .into(),
        ));
    }
    let many = if T::DTYPE == dt::DOUBLE {
        dmax.to_f64() / f64::from(delta) > 10000.0
    } else {
        dmax.to_f32() / delta > 10000.0
    };
    if many {
        ctx.put("Warning: tfce has detected a large number of integral steps. This operation may require a great deal of time to complete.\n");
    }
    if zeros {
        for v in vol.iter_mut() {
            *v = T::from_int(0);
        }
        return Ok(Ok(()));
    }
    let limit = max_t + delta;
    if !float_loop(p.min_t, limit, delta).0 {
        ctx.notice(
            &defects::FSL_174,
            "-tfce: a threshold step below the threshold's precision",
        );
        if ctx.reproduce() {
            ctx.eput(&format!(
                "larmorx-nc: error: fslmaths: -tfce: the thresholds stop advancing ({}: FSL never \
                 ends here)\n",
                defects::FSL_174.id
            ));
            return Err(Stop::Exit(1));
        }
        return Err(rule_b(
            ctx,
            &defects::FSL_174,
            "-tfce: the threshold step is below the precision of the thresholds",
        ));
    }
    let idx = |x: i64, y: i64, z: i64| ((z * ny + y) * nx + x) as usize;
    // VecSortI: the interior voxels above minT (FSL-173: FSL's loops over it read one element
    // past its end before testing the bound; here the bound comes first)
    let mut sorted: Vec<(f64, usize)> = Vec::new();
    for z in 1..nz - 1 {
        for y in 1..ny - 1 {
            for x in 1..nx - 1 {
                let k = idx(x, y, z);
                let iv = vol[k].to_f32();
                if iv > p.min_t {
                    sorted.push((f64::from(iv), k));
                }
            }
        }
    }
    sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("no NaN above minT"));
    const INIT: i32 = -1;
    const MASK: i32 = -2;
    let mut offsets: Vec<isize> = Vec::new();
    for dz in -1i64..=1 {
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let d = dx * dx + dy * dy + dz * dz;
                let class = match d {
                    0 => 100,
                    1 => 6,
                    2 => 18,
                    _ => 26,
                };
                if d != 0 && conn >= class {
                    offsets.push(((dz * ny + dy) * nx + dx) as isize);
                }
            }
        }
    }
    let mut lab = vec![INIT; vol.len()];
    let mut enh = vec![0f32; vol.len()];
    let mut queue = VecDeque::new();
    let mut sizes: Vec<f64> = Vec::new();
    let mut start = 0usize;
    let mut cur = p.min_t;
    while cur < limit {
        while start < sorted.len() && sorted[start].0 <= f64::from(cur) {
            lab[sorted[start].1] = INIT;
            start += 1;
        }
        let rest = &sorted[start..];
        for &(_, k) in rest {
            lab[k] = MASK;
        }
        let mut curlab = 0i32;
        for &(_, k) in rest {
            if lab[k] != MASK {
                continue;
            }
            curlab += 1;
            lab[k] = curlab;
            queue.push_back(k);
            while let Some(q) = queue.pop_front() {
                for &o in &offsets {
                    let r = (q as isize + o) as usize;
                    if lab[r] == MASK {
                        lab[r] = curlab;
                        queue.push_back(r);
                    }
                }
            }
        }
        sizes.clear();
        sizes.resize(curlab as usize, 0.0);
        for &(_, k) in rest {
            sizes[(lab[k] - 1) as usize] += 1.0;
        }
        let hh = f64::from(glibc_float::powf(cur, h));
        let ef = f64::from(e);
        for s in &mut sizes {
            *s = sse::mul_f64(glibc_double::pow(*s, ef), hh);
        }
        for &(_, k) in rest {
            let c = sizes[(lab[k] - 1) as usize];
            enh[k] = from_f64_nan::<f32>(sse::add_f64(f64::from(enh[k]), c));
        }
        cur += delta;
    }
    if nan_first && !ctx.reproduce() && enh.iter().any(|&v| v != 0.0) {
        ctx.notice(&defects::FSL_153, "-tfce: the first voxel is NaN");
    }
    for (d, &v) in vol.iter_mut().zip(&enh) {
        *d = from_float::<T>(ctx, v, "-tfce");
    }
    Ok(Ok(()))
}

// ------------------------------------------------------------------------------------------------
// -tfceS

/// `-tfceS <H> <E> <connectivity> <X> <Y> <Z> <tfce_thresh>` on every volume.
pub(super) fn tfce_support<T: Arith>(
    ctx: &mut Ctx,
    input: &mut Volume<T>,
    argv: &[String],
    i: usize,
    p: TfceParams,
) -> Res<()> {
    let h = cnum::atof(number_arg(ctx, argv, i + 1)?) as f32;
    let e = cnum::atof(number_arg(ctx, argv, i + 2)?) as f32;
    let conn = cnum::atoi(number_arg(ctx, argv, i + 3)?);
    let xn = cnum::atoi(number_arg(ctx, argv, i + 4)?);
    let yn = cnum::atoi(number_arg(ctx, argv, i + 5)?);
    let zn = cnum::atoi(number_arg(ctx, argv, i + 6)?);
    let thr = cnum::atof(number_arg(ctx, argv, i + 7)?) as f32;
    let [x, y, z] = stats::nifti_to_newimage_vox(input, xn, yn, zn);
    check_connectivity(ctx, conn, "-tfceS")?;
    let (nx, ny, nz) = (input.xsize(), input.ysize(), input.zsize());
    let nvox = input.nvoxels().max(0) as usize;
    let (x, y, z) = (i64::from(x), i64::from(y), i64::from(z));
    let inside = x >= 0 && y >= 0 && z >= 0 && x < nx && y < ny && z < nz;
    let lin = (z * ny + y) * nx + x;
    if !inside {
        ctx.notice(
            &defects::FSL_178,
            format!("-tfceS: the voxel ({xn}, {yn}, {zn}) is outside the image"),
        );
        if !ctx.reproduce() {
            return Err(rule_b(
                ctx,
                &defects::FSL_178,
                &format!("-tfceS: the voxel ({xn}, {yn}, {zn}) is outside the image"),
            ));
        }
        if lin < 0 || lin >= nvox as i64 {
            return Err(ctx.refuse(
                &defects::FSL_178,
                "-tfceS: a voxel whose offset is outside the volume's memory",
            ));
        }
    }
    if p.delta.is_nan() || p.delta < 0.0 {
        ctx.notice(
            &defects::FSL_174,
            "-tfceS: a negative or NaN threshold step",
        );
        if !ctx.reproduce() {
            return Err(rule_b(
                ctx,
                &defects::FSL_174,
                "-tfceS: the threshold step (-tfce_delta) is negative or NaN",
            ));
        }
    }
    let voxel = lin as usize;
    let dims = [nx, ny, nz];
    for t in 0..input.tsize().max(0) as usize {
        let mut vol = input.data[t * nvox..(t + 1) * nvox].to_vec();
        tfce_support_volume(ctx, &mut vol, dims, (h, e, conn, thr), p, voxel, inside)?;
        input.data[t * nvox..(t + 1) * nvox].copy_from_slice(&vol);
    }
    Ok(())
}

/// `binarise(thresh)` of a `float` volume: 1 in `[thresh, FLT_MAX]`, else 0.
fn binarised(v: &[f32], thresh: f32) -> Vec<f32> {
    v.iter()
        .map(|&x| f32::from(x >= thresh && x <= f32::MAX))
        .collect()
}

/// newimagefns' `tfce_support(VolIntn, H, E, NumConn, minT, deltaT, Xoi, Yoi, Zoi, threshTFCE)`
/// of one volume (`voxel` the linear offset `value()` reads; outside the image `VolLabl(Xoi,
/// Yoi, Zoi)` is the zero padding).
fn tfce_support_volume<T: Arith>(
    ctx: &mut Ctx,
    vol: &mut [T],
    dims: [i64; 3],
    (h, e, conn, thr): (f32, f32, i32, f32),
    p: TfceParams,
    voxel: usize,
    inside: bool,
) -> Res<()> {
    let nvox = vol.len();
    let orig: Vec<f32> = vol.iter().map(|v| v.to_f32()).collect();
    let max_t = orig[voxel];
    let (delta, thr_num) = if p.delta == 0.0 {
        (sse::div_f32(max_t, 100.0), 101i32)
    } else {
        let q = sse::div_f32(max_t, p.delta);
        (p.delta, cnum::f64_to_i32(f64::from(q)).wrapping_add(1))
    };
    // ColumnVector Clusters(thrNum): no element for a size of 0 or below
    let n_elem = u64::try_from(thr_num).unwrap_or(0);
    let (stops, steps) = float_loop(p.min_t, max_t, delta);
    if !stops {
        // FSL fills its arrays and stops (std::out_of_range); the default refuses (rule B)
        ctx.notice(&defects::FSL_174, "-tfceS: the thresholds stop advancing");
        if ctx.reproduce() {
            return Err(array_stop(ctx, n_elem));
        }
        return Err(rule_b(
            ctx,
            &defects::FSL_174,
            "-tfceS: the threshold step cannot advance the thresholds to the voxel's value",
        ));
    }
    if steps > n_elem {
        ctx.notice(
            &defects::FSL_177,
            format!("-tfceS: {steps} thresholds for arrays of {n_elem}"),
        );
        if ctx.reproduce() {
            return Err(array_stop(ctx, n_elem));
        }
    }
    let mut enh = orig.clone();
    let mut clusters: Vec<f64> = Vec::new();
    let mut thresholds: Vec<f64> = Vec::new();
    let mut thresh = p.min_t;
    while thresh < max_t {
        let temp = binarised(&enh, thresh);
        let (labels, sizes) = filters::connected_components(&temp, dims, conn);
        let lv = labels[voxel];
        if lv <= 0 {
            // ClusterSizes(0): the voxel is in no cluster (an infinite value)
            return Err(array_stop(ctx, sizes.len() as u64));
        }
        clusters.push(sizes[(lv - 1) as usize] as f64);
        thresholds.push(f64::from(thresh));
        for (v, &l) in enh.iter_mut().zip(&labels) {
            if l != lv {
                *v = 0.0;
            }
        }
        thresh += delta;
    }
    let mut summ = 0f32;
    let mut thre = 0f32;
    for k in (0..clusters.len()).rev() {
        let c = glibc_float::powf(clusters[k] as f32, e);
        let s = glibc_float::powf(thresholds[k] as f32, h);
        summ = sse::add_f32(summ, sse::mul_f32(c, s));
        thre = thresholds[k] as f32;
        if summ >= thr {
            break;
        }
    }
    if summ < thr {
        ctx.put("it doesn't reach to specified threshold\n");
    }
    let temp = binarised(&orig, thre);
    let (labels, _) = filters::connected_components(&temp, dims, conn);
    let lf = if inside { labels[voxel] } else { 0 };
    for k in 0..nvox {
        let v = if labels[k] != lf { 0.0 } else { orig[k] };
        vol[k] = from_float::<T>(ctx, v, "-tfceS");
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// -roc

/// `std::min(a, b)` and `std::max(a, b)` of floats (`b < a ? b : a`, `a < b ? b : a`).
fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// `-roc <AROC-thresh> <outfile> [4Dnoiseonly] <truth>`; returns the index of its last
/// argument.
pub(super) fn roc<T: Arith>(
    ctx: &mut Ctx,
    input: &Volume<T>,
    argv: &[String],
    i: usize,
) -> Res<usize> {
    use sse::{add_f32, div_f32, mul_f32, sub_f32};
    let mut i = i + 1;
    let mut aroc_thresh = cnum::atof(number_arg(ctx, argv, i)?) as f32;
    i += 1;
    let out_name = number_arg(ctx, argv, i)?;
    // ofstream ofs(name): a file that cannot be opened takes nothing, silently
    let mut ofs = std::fs::File::create(out_name)
        .ok()
        .map(std::io::BufWriter::new);
    let mut line = |s: String| {
        if let Some(f) = ofs.as_mut() {
            let _ = f.write_all(s.as_bytes());
        }
    };
    let mut separate = true;
    if aroc_thresh < 0.0 {
        separate = false;
        aroc_thresh = -aroc_thresh;
    }
    let mut nan153 = false;
    let mut minval = extreme(ctx, &input.data, false, &mut nan153).to_f32();
    let (nx, ny, nz, nt) = (input.xsize(), input.ysize(), input.zsize(), input.tsize());
    let n = (nx * ny * nz * nt).max(0) as usize;
    // the noise image and its strides
    let mut noise: Vec<f32>;
    let ndims: [i64; 3];
    if separate {
        i += 1;
        let name = string_arg(ctx, argv, i)?;
        let nv = read4d::<f32>(ctx, name)?;
        minval = std_min(extreme(ctx, &nv.data, false, &mut nan153).to_f32(), minval);
        ndims = [nv.xsize(), nv.ysize(), nv.zsize()];
        if nv.dims[..4] != input.dims[..4] {
            ctx.notice(
                &defects::FSL_179,
                "-roc: a noise image of another size than the input",
            );
            if !ctx.reproduce() {
                return Err(rule_b(
                    ctx,
                    &defects::FSL_179,
                    "-roc: the noise image must have the input's size",
                ));
            }
            let last = ((ndims[2] * (nt - 1) + (nz - 1)) * ndims[1] + (ny - 1)) * ndims[0] + nx - 1;
            if last >= nv.data.len() as i64 {
                return Err(ctx.refuse(
                    &defects::FSL_179,
                    "-roc: a noise image smaller than the input (FSL reads and writes outside it)",
                ));
            }
        }
        noise = nv.data;
        let (nxn, nyn, nzn) = (ndims[0], ndims[1], ndims[2]);
        for t in 0..nt {
            for z in 0..nz {
                for y in 0..ny {
                    for x in 0..nx {
                        let k = (((nzn * t + z) * nyn + y) * nxn + x) as usize;
                        noise[k] =
                            glibc_float::logf_compat(add_f32(sub_f32(noise[k], minval), 1.0));
                    }
                }
            }
        }
    } else {
        noise = Vec::new();
        ndims = [nx, ny, nz];
    }
    let nidx = |x: i64, y: i64, z: i64, t: i64| {
        (((ndims[2] * t + z) * ndims[1] + y) * ndims[0] + x) as usize
    };
    let mut loginput = vec![0f32; n];
    for (l, &v) in loginput.iter_mut().zip(&input.data) {
        *l = if T::DTYPE == dt::DOUBLE {
            let a = sse::add_f64(sse::sub_f64(v.to_f64(), f64::from(minval)), 1.0);
            from_f64_nan::<f32>(glibc_double::log_compat(a))
        } else {
            glibc_float::logf_compat(add_f32(sub_f32(v.to_f32(), minval), 1.0))
        };
    }
    let mut aroc = 0f32;
    let (mut fp, mut tp, mut tpprev, mut fpprev) = (0f32, 0f32, 0f32, 0f32);
    let ntu = nt.max(0) as usize;
    let mut tpvals = vec![0f64; ntu];
    let mut tpprevvals = vec![0f64; ntu];
    let mut fpunc = vec![0f64; ntu];
    let mut aroct = vec![0f64; ntu];
    let mut maxlogval = extreme(ctx, &loginput, true, &mut nan153);
    if separate {
        maxlogval = std_max(maxlogval, extreme(ctx, &noise, true, &mut nan153));
    }
    let mut delta = div_f32(maxlogval, 1000.0);
    if delta == 0.0 {
        delta = 1.0;
    }
    i += 1;
    let tname = string_arg(ctx, argv, i)?;
    let (truth_v, warning) = volume::read_volume::<f32>(tname, ctx.run.fsl)?;
    if let Some(w) = warning {
        ctx.eput(&format!("{w}\n"));
    }
    let mut truth = truth_v;
    let maskim = (!separate).then(|| binarised(&truth.data, -0.001f64 as f32));
    // border = 0: every voxel of the input's volume counts
    let mut invtruecount = (nx * ny * nz) as i32;
    let tmax = extreme(ctx, &truth.data, true, &mut nan153);
    let lower = (f64::from(tmax) * 0.05) as f32;
    truth.data = binarised(&truth.data, lower);
    let truecount = truth.data.iter().filter(|&&v| v != 0.0).count() as i32;
    let mut tpim = Volume::<f32>::new(nx, ny, nz, nt);
    tpim.data.copy_from_slice(&loginput);
    tpim.op_volume(Op::Mul, &truth)?.ok();
    if let Some(mask) = &maskim {
        let mut inv = truth.clone();
        for (d, (&tv, &mv)) in inv.data.iter_mut().zip(truth.data.iter().zip(mask)) {
            *d = mul_f32(add_f32(mul_f32(tv, -1.0), 1.0), mv);
        }
        invtruecount = inv.data.iter().filter(|&&v| v != 0.0).count() as i32;
        let mut nz_v = Volume::<f32>::new(nx, ny, nz, nt);
        nz_v.data.copy_from_slice(&loginput);
        nz_v.op_volume(Op::Mul, &inv)?.ok();
        noise = nz_v.data;
    }
    if truecount == 0 || (!separate && invtruecount == 0) {
        let what = if truecount == 0 {
            "-roc: the truth image has no true voxels"
        } else {
            "-roc: the truth image has no voxels of 0 (no noise without a noise image)"
        };
        ctx.notice(&defects::FSL_180, what);
        if !ctx.reproduce() {
            return Err(rule_b(ctx, &defects::FSL_180, what));
        }
    }
    let g = |v: f64| cnum::stream(v);
    let tcf = truecount as f32;
    let icf = invtruecount as f32;
    let mut thresh = add_f32(maxlogval, delta);
    while thresh > 0.0 && fp < aroc_thresh {
        let mut fpfwe = 0f32;
        for t in 0..nt {
            let (mut sig_tp, mut sig_fp) = (0f32, 0f32);
            for z in 0..nz {
                for y in 0..ny {
                    for x in 0..nx {
                        let k = (((nz * t + z) * ny + y) * nx + x) as usize;
                        sig_tp += f32::from(tpim.data[k] >= thresh);
                        sig_fp += f32::from(noise[nidx(x, y, z, t)] >= thresh);
                    }
                }
            }
            let tu = t as usize;
            tpvals[tu] = f64::from(div_f32(sig_tp, tcf));
            fpunc[tu] = f64::from(div_f32(sig_fp, icf));
            fpfwe += f32::from(sig_fp > 0.0);
            if !separate {
                let ex = add_f32(sub_f32(glibc_float::expf(thresh), 1.0), minval);
                line(format!(
                    "{} {} {}\n",
                    g(fpunc[tu]),
                    g(tpvals[tu]),
                    g(f64::from(ex))
                ));
            }
        }
        tp = from_f64_nan::<f32>(miscmaths::mean_col(&tpvals));
        fp = if separate {
            div_f32(fpfwe, nt as f32)
        } else {
            from_f64_nan::<f32>(miscmaths::mean_col(&fpunc))
        };
        if fp < aroc_thresh {
            let w = sub_f32(fp, fpprev);
            aroc = add_f32(aroc, mul_f32(div_f32(add_f32(tp, tpprev), 2.0), w));
            for k in 0..ntu {
                let a = sse::div_f64(sse::add_f64(tpvals[k], tpprevvals[k]), 2.0);
                aroct[k] = sse::add_f64(aroct[k], sse::mul_f64(a, f64::from(w)));
            }
        } else {
            // the update straddles the FP threshold
            let ratio = f64::from(div_f32(sub_f32(aroc_thresh, fpprev), sub_f32(fp, fpprev)));
            let w = f64::from(sub_f32(aroc_thresh, fpprev));
            let half = sse::mul_f64(f64::from(sub_f32(tp, tpprev)), 0.5);
            let d = sse::mul_f64(
                sse::add_f64(f64::from(tpprev), sse::mul_f64(half, ratio)),
                w,
            );
            aroc = from_f64_nan::<f32>(sse::add_f64(f64::from(aroc), d));
            for k in 0..ntu {
                let half = sse::mul_f64(sse::sub_f64(tpvals[k], tpprevvals[k]), 0.5);
                let v = sse::mul_f64(sse::add_f64(tpprevvals[k], sse::mul_f64(half, ratio)), w);
                aroct[k] = sse::add_f64(aroct[k], v);
            }
        }
        tpprev = tp;
        tpprevvals.copy_from_slice(&tpvals);
        fpprev = fp;
        if separate {
            let ex = add_f32(sub_f32(glibc_float::expf(thresh), 1.0), minval);
            line(format!(
                "{} {} {} {} {}\n",
                g(f64::from(fp)),
                g(f64::from(tp)),
                g(sse::sqrt_f64(miscmaths::var_col(&fpunc))),
                g(sse::sqrt_f64(miscmaths::var_col(&tpvals))),
                g(f64::from(ex))
            ));
        }
        thresh = sub_f32(thresh, delta);
    }
    if fp < aroc_thresh {
        // FP never reached the threshold
        let w = sub_f32(aroc_thresh, fp);
        aroc = add_f32(aroc, mul_f32(tp, w));
        for k in 0..ntu {
            aroct[k] = sse::add_f64(aroct[k], sse::mul_f64(tpvals[k], f64::from(w)));
        }
    }
    drop(line);
    if let Some(mut f) = ofs {
        let _ = f.flush();
    }
    if nan153 && !ctx.reproduce() {
        ctx.notice(&defects::FSL_153, "-roc: the first voxel is NaN");
    }
    // quantile's SortAscending throws for a NaN (the text before it on cout is never flushed)
    let (Some(q1), Some(q3)) = (
        miscmaths::quantile_col(&aroct, 1),
        miscmaths::quantile_col(&aroct, 3),
    ) else {
        return Err(Stop::Exception(filters::sort_nan_error()));
    };
    ctx.put(&format!(
        "{} {} {}\n",
        g(f64::from(div_f32(aroc, aroc_thresh))),
        g(f64::from(div_f32(q1, aroc_thresh))),
        g(f64::from(div_f32(q3, aroc_thresh)))
    ));
    Ok(i)
}
