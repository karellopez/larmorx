// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-avwutils 2209.6 (fslmaths.cc: inputParser's
// dimensionality reduction, -pval, -pval0, -cpval, -ztop, -ptoz, -rank, -ranknorm, -roi, -bptf).
// Copyright (C) 2000-2008 University of Oxford (Steve Smith, David Flitney, Mark Jenkinson,
// Stuart Clare, Thomas Nichols and Matthew Webster). Rust translation Copyright 2026 Karel Lopez
// Vilaret; modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! fslmaths' operations along a dimension and its statistics.
//!
//! - **Along a dimension** (`-T`, `-X`, `-Y`, `-Z` followed by `max`, `min`, `mean`, `std`,
//!   `maxn`, `median`, `perc <p>`, `ar1`; any other capital letter reduces nothing): the image
//!   reinitialised with that dimension of size 1, each output value computed from the column of
//!   values along it with newimage's statistics (`larmorx_nc_core::vstats`).
//! - **Statistics** (`-pval`, `-pval0`, `-cpval`: the time points as permutations, the first the
//!   unpermuted statistic; `-ztop`, `-ptoz`: fsl-cprob's `ndtr` and `ndtri`; `-rank`,
//!   `-ranknorm`: Armadillo's sort with fslmaths' unsorting loop).
//! - `-roi` (zero outside a box of voxel coordinates in NIfTI's order) and `-bptf`
//!   (newimagefns' `bandpass_temporal_filter`).
//! - **D13.** FSL-153 (a NaN first value along a dimension), FSL-162 (`-rank`, `-ranknorm` of a
//!   series holding NaN), FSL-167 (the standard deviation without its clamp and guard), FSL-168
//!   (percentiles of values holding NaN), FSL-169 (NaN statistics in the p-values), FSL-170
//!   (`-ztop` of an infinite z), FSL-171 (`-bptf`'s sigmas beyond int), FSL-172 (`-ptoz` of
//!   p-values whose `1 - p` rounds to 1).

use super::*;
use larmorx_nc_core::sse;
use larmorx_nc_core::stdsort::Outside;
use larmorx_nc_core::vstats;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reduce {
    Max,
    Min,
    Mean,
    Std,
    Maxn,
    Median,
    Perc,
    Ar1,
}

/// The dimensionality reduction `argv[*i]` (`-<capital><name>`); `Ok(Some(1))` when FSL returns
/// 1 (an unknown name, after its message). `-?perc` takes the next argument (`*i` moved past it).
pub(super) fn reduce<T: Arith>(
    ctx: &mut Ctx,
    input: &mut Volume<T>,
    argv: &[String],
    i: &mut usize,
) -> Res<Option<u8>> {
    let op = argv[*i].clone();
    let b = op.as_bytes();
    let name = &b[2..];
    let (mut xo, mut yo, mut zo, mut to) = (1i64, 1i64, 1i64, 1i64);
    match b[1] {
        b'T' => to = input.tsize(),
        b'Z' => zo = input.zsize(),
        b'Y' => yo = input.ysize(),
        b'X' => xo = input.xsize(),
        _ => {}
    }
    let kind = match name {
        b"max" => Some(Reduce::Max),
        b"min" => Some(Reduce::Min),
        b"mean" => Some(Reduce::Mean),
        b"std" => Some(Reduce::Std),
        b"maxn" => Some(Reduce::Maxn),
        b"median" => Some(Reduce::Median),
        b"perc" => Some(Reduce::Perc),
        b"ar1" => Some(Reduce::Ar1),
        _ => None,
    };
    let temp = std::mem::replace(input, Volume::<T>::new(0, 0, 0, 0));
    let (nx, ny, nz, nt) = (
        temp.xsize() / xo,
        temp.ysize() / yo,
        temp.zsize() / zo,
        temp.tsize() / to,
    );
    let mut out = Volume::<T>::new(nx, ny, nz, nt);
    out.copy_properties(&temp);
    let nsize = (xo * yo * zo * to) as usize;
    let Some(kind) = kind else {
        // the message at the first voxel (every size is at least 1)
        *input = out;
        ctx.eput(&format!("Error unknown Dimensionality operation: {op}\n"));
        return Ok(Some(1));
    };
    // -?perc: percentile(atof(argv[i+1]) / 100.0), the float checked at each voxel
    let pvalue = match kind {
        Reduce::Median => 0.5f32,
        Reduce::Perc => (cnum::atof(&argv[*i + 1]) / 100.0) as f32,
        _ => 0.0,
    };
    if kind == Reduce::Perc && (f64::from(pvalue) > 1.0 || f64::from(pvalue) < 0.0) {
        *input = out;
        return Err(Stop::Exception(FslError::imthrow(
            "Percentiles must be in the range [0.0,1.0]",
            4,
        )));
    }
    let (sx, sy, sz) = (
        usize::from(xo != 1),
        usize::from(yo != 1),
        usize::from(zo != 1),
    );
    let st = usize::from(to != 1);
    let old_nvox = temp.nvoxels() as usize;
    let mut col: Vec<T> = vec![T::default(); nsize];
    let mut k = 0usize;
    for t in 0..nt as usize {
        for z in 0..nz as usize {
            for y in 0..ny as usize {
                for x in 0..nx as usize {
                    for (j, c) in col.iter_mut().enumerate() {
                        let idx = temp.idx(
                            (x + j * sx) as i64,
                            (y + j * sy) as i64,
                            (z + j * sz) as i64,
                        );
                        *c = temp.data[(t + j * st) * old_nvox + idx];
                    }
                    out.data[k] = reduce_column(ctx, &mut col, kind, pvalue, &op)?;
                    k += 1;
                }
            }
        }
    }
    *input = out;
    if kind == Reduce::Perc {
        *i += 1;
    }
    Ok(None)
}

/// One output value of a reduction from its column (`col` may be reordered or changed).
fn reduce_column<T: Arith>(
    ctx: &mut Ctx,
    col: &mut [T],
    kind: Reduce,
    pvalue: f32,
    op: &str,
) -> Res<T> {
    Ok(match kind {
        Reduce::Max | Reduce::Min | Reduce::Maxn => {
            let mut e = vstats::extrema(col).expect("a column of at least one value");
            if T::is_nan(col[0])
                && let Some(f) = vstats::extrema_without_nan(col)
            {
                ctx.notice(
                    &defects::FSL_153,
                    format!("{op}: a NaN first value along the dimension"),
                );
                if !ctx.reproduce() {
                    e = f;
                }
            }
            match kind {
                Reduce::Max => e.max,
                Reduce::Min => e.min,
                _ => T::from_int(e.imax as i32),
            }
        }
        Reduce::Mean => ctx.conv::<T>(vstats::mean(col), op),
        Reduce::Std => {
            if vstats::variance_defect(col) {
                ctx.notice(
                    &defects::FSL_167,
                    format!("{op}: a variance of one value or rounded below 0"),
                );
            }
            let corrected = !ctx.reproduce();
            ctx.conv::<T>(vstats::stddev(col, corrected), op)
        }
        Reduce::Median | Reduce::Perc => percentile(ctx, col, pvalue, op)?,
        Reduce::Ar1 => ar1(ctx, col, op),
    })
}

/// `percentile(p)` of a column; FSL-168 where it holds NaN.
fn percentile<T: Arith>(ctx: &mut Ctx, col: &mut [T], p: f32, op: &str) -> Res<T> {
    if T::IS_FLOAT && col.iter().any(|&v| T::is_nan(v)) {
        ctx.notice(&defects::FSL_168, format!("{op}: values holding NaN"));
        if !ctx.reproduce() {
            let mut kept: Vec<T> = col.iter().copied().filter(|&v| !T::is_nan(v)).collect();
            if kept.is_empty() {
                return Ok(T::from_f64(f64::NAN));
            }
            return percentile_sorted(ctx, &mut kept, p, op);
        }
    }
    percentile_sorted(ctx, col, p, op)
}

fn percentile_sorted<T: Arith>(ctx: &mut Ctx, hist: &mut [T], p: f32, op: &str) -> Res<T> {
    match vstats::percentile(hist, p) {
        Ok(v) => Ok(v),
        Err(Outside) => Err(ctx.refuse(
            &defects::FSL_168,
            &format!("{op}: the sort of values holding NaN steps outside them"),
        )),
    }
}

/// `-Tar1`: the column demeaned in its type, then the lag-1 products over its sum of squares
/// added up in the type (`(T)` of each term).
fn ar1<T: Arith>(ctx: &mut Ctx, col: &mut [T], op: &str) -> T {
    let m = ctx.conv::<T>(vstats::mean(col), op);
    for c in col.iter_mut() {
        *c = T::sub(*c, m);
    }
    let sumsq = vstats::sums(col).1;
    let mut acc = T::from_int(0);
    if sumsq != 0.0 {
        for k in 1..col.len() {
            let p = product(col[k], col[k - 1]);
            let term = ctx.conv::<T>(sse::div_f64(p, sumsq), op);
            acc = T::add(acc, term);
        }
    }
    acc
}

/// `a * b` in C's arithmetic (`char` and `short` promoted to `int`; an `int` product wraps
/// around as x86-64's `imul` does), converted to `double`. Not a defect: a wrapped product is
/// below half the sum of squares in magnitude, as the right one is, so under an integer type
/// every term truncates to 0 either way.
fn product<T: Arith>(a: T, b: T) -> f64 {
    match T::DTYPE {
        dt::FLOAT => f64::from(sse::mul_f32(a.to_f32(), b.to_f32())),
        dt::DOUBLE => sse::mul_f64(a.to_f64(), b.to_f64()),
        _ => f64::from(T::to_int(a).wrapping_mul(T::to_int(b))),
    }
}

// ------------------------------------------------------------------------------------------------
// Statistics over the time points

/// `-pval` and `-pval0` (`ignore_zeros`): the share of the time points (the first included)
/// whose value is at least the first one's; with `-pval0` a zero first value gives 0 and zero
/// values are left out. FSL-169: NaN statistics.
pub(super) fn pval<T: Arith>(ctx: &mut Ctx, input: &mut Volume<T>, ignore_zeros: bool, op: &str) {
    let temp = std::mem::replace(input, Volume::<T>::new(0, 0, 0, 0));
    let mut out = Volume::<T>::new(temp.xsize(), temp.ysize(), temp.zsize(), 1);
    out.copy_properties(&temp);
    let nvox = temp.nvoxels() as usize;
    let nt = temp.tsize() as usize;
    for v in 0..nvox {
        let at = |t: usize| temp.data[t * nvox + v];
        let v0 = at(0);
        if ignore_zeros && v0.to_f64() == 0.0 {
            out.data[v] = T::from_int(0);
            continue;
        }
        let has_nan = T::IS_FLOAT && (0..nt).any(|t| T::is_nan(at(t)));
        if has_nan {
            ctx.notice(&defects::FSL_169, format!("{op}: NaN statistics"));
        }
        let fix = has_nan && !ctx.reproduce();
        if fix && T::is_nan(v0) {
            out.data[v] = v0;
            continue;
        }
        let mut pv = 1.0f32;
        let mut cnt = 1i32;
        for t in 1..nt {
            let x = at(t);
            if fix && T::is_nan(x) {
                continue;
            }
            if ignore_zeros && x.to_f64() == 0.0 {
                continue;
            }
            pv += f32::from(u8::from(v0 <= x));
            cnt += 1;
        }
        let denom = if ignore_zeros || fix { cnt } else { nt as i32 };
        pv /= denom as f32;
        out.data[v] = T::from_f32(pv);
    }
    *input = out;
}

/// `MAX(a, b)` of `-cpval`'s maximum distribution (a float) and a value: compared in their
/// common type (`double` for a double image, else `float`), the result stored as a float.
fn cpval_max<T: Arith>(a: f32, b: T) -> f32 {
    if T::DTYPE == dt::DOUBLE {
        let (x, y) = (f64::from(a), b.to_f64());
        (if x > y { x } else { y }) as f32
    } else {
        let y = b.to_f32();
        if a > y { a } else { y }
    }
}

/// `v <= m` in their common type.
fn cpval_le<T: Arith>(v: T, m: f32) -> bool {
    if T::DTYPE == dt::DOUBLE {
        v.to_f64() <= f64::from(m)
    } else {
        v.to_f32() <= m
    }
}

/// `-cpval`: each time point's maximum over the image (from the image's minimum), then each
/// voxel's share of the time points whose maximum is at least its first value. FSL-169: NaN
/// statistics.
pub(super) fn cpval<T: Arith>(ctx: &mut Ctx, input: &mut Volume<T>) {
    let nvox = input.nvoxels() as usize;
    let nt = input.tsize() as usize;
    let has_nan = T::IS_FLOAT && input.data[..nvox * nt].iter().any(|&v| T::is_nan(v));
    if has_nan {
        ctx.notice(&defects::FSL_169, "-cpval: NaN statistics");
    }
    let fix = has_nan && !ctx.reproduce();
    let min = if fix {
        vstats::extrema_without_nan(&input.data)
            .map(|e| e.min)
            .unwrap_or_else(|| input.min())
    } else {
        input.min()
    };
    let init = if T::DTYPE == dt::DOUBLE {
        min.to_f64() as f32
    } else {
        min.to_f32()
    };
    let mut max_dist = vec![init; nt];
    for (t, m) in max_dist.iter_mut().enumerate() {
        for &x in &input.data[t * nvox..(t + 1) * nvox] {
            if fix && T::is_nan(x) {
                continue;
            }
            *m = cpval_max(*m, x);
        }
    }
    let temp = std::mem::replace(input, Volume::<T>::new(0, 0, 0, 0));
    let mut out = Volume::<T>::new(temp.xsize(), temp.ysize(), temp.zsize(), 1);
    out.copy_properties(&temp);
    for v in 0..nvox {
        let v0 = temp.data[v];
        if fix && T::is_nan(v0) {
            out.data[v] = v0;
            continue;
        }
        let mut c = 0.0f32;
        for &m in &max_dist {
            c += f32::from(u8::from(cpval_le(v0, m)));
        }
        c /= nt as f32;
        out.data[v] = T::from_f32(c);
    }
    *input = out;
}

/// `-ztop`: `1 - ndtr(z)`. FSL-170: an infinite z.
pub(super) fn ztop<T: Arith>(ctx: &mut Ctx, input: &mut Volume<T>) {
    let n = looped_len(input);
    for k in 0..n {
        let z = input.data[k].to_f64();
        let p = if z.is_infinite() {
            ctx.notice(&defects::FSL_170, "-ztop: an infinite z");
            if ctx.reproduce() {
                1.0 - cprob::ndtr(z)
            } else if z > 0.0 {
                0.0
            } else {
                1.0
            }
        } else {
            1.0 - cprob::ndtr(z)
        };
        input.data[k] = ctx.conv::<T>(p, "-ztop");
    }
}

/// `-ptoz`: `ndtri(1 - p)` for `0 < p < 1`, else 0. FSL-172: `1 - p` rounded to 1.
pub(super) fn ptoz<T: Arith>(ctx: &mut Ctx, input: &mut Volume<T>) {
    let n = looped_len(input);
    let mut printed = String::new();
    for k in 0..n {
        let v = input.data[k].to_f64();
        let z = if v <= 0.0 || v >= 1.0 {
            0.0
        } else if 1.0 - v == 1.0 && !ctx.reproduce() {
            ctx.notice(
                &defects::FSL_172,
                "-ptoz: a p-value whose 1 - p rounds to 1",
            );
            -cprob::ndtri(v).0
        } else {
            if 1.0 - v == 1.0 {
                ctx.notice(
                    &defects::FSL_172,
                    "-ptoz: a p-value whose 1 - p rounds to 1",
                );
            }
            let (z, msg) = cprob::ndtri(1.0 - v);
            if let Some(m) = msg {
                printed.push_str(&m);
            }
            z
        };
        input.data[k] = ctx.conv::<T>(z, "-ptoz");
    }
    if !printed.is_empty() {
        ctx.put(&printed);
    }
}

/// fslmaths' ranks of a series (`val` sorted ascending by Armadillo, then "unsorted" by swapping
/// each misplaced value's rank with that of every later sorted value equal to it), 1-based.
fn ranks(val: &[f64]) -> Vec<f64> {
    let n = val.len();
    let mut sortval = val.to_vec();
    let sorted = filters::arma_sort(&mut sortval, false);
    debug_assert!(sorted);
    let mut rank: Vec<f64> = (1..=n).map(|k| k as f64).collect();
    for k in 0..n {
        if val[k] != sortval[k] {
            for l in k + 1..n {
                if sortval[l] == val[k] {
                    rank.swap(l, k);
                    sortval.swap(l, k);
                }
            }
        }
    }
    rank
}

/// `-rank` and `-ranknorm` (`normal`: the ranks' expected order statistics of a uniform, `(r -
/// 0.5) / n`, through `ndtri`, scaled by the series' standard deviation about its mean). FSL-162:
/// a series holding NaN; FSL-167: the standard deviation.
pub(super) fn rank<T: Arith>(ctx: &mut Ctx, input: &mut Volume<T>, normal: bool) -> Res<()> {
    let op = if normal { "-ranknorm" } else { "-rank" };
    let nvox = input.nvoxels() as usize;
    let nt = input.tsize() as usize;
    for v in 0..nvox {
        let all: Vec<f64> = (0..nt).map(|t| input.data[t * nvox + v].to_f64()).collect();
        let keep: Vec<usize> = (0..nt).filter(|&t| !all[t].is_nan()).collect();
        if keep.len() < nt {
            ctx.notice(
                &defects::FSL_162,
                format!("{op}: a time series holding NaN"),
            );
            if ctx.reproduce() {
                return Err(Stop::Exception(filters::sort_nan_error()));
            }
        }
        let val: Vec<f64> = keep.iter().map(|&t| all[t]).collect();
        let r = ranks(&val);
        let out: Vec<f64> = if normal {
            let m = r.len() as f64;
            let u: Vec<f64> = r.iter().map(|x| (x - 0.5) / m).collect();
            if vstats::variance_defect(&val) {
                ctx.notice(
                    &defects::FSL_167,
                    format!("{op}: a variance of one value or rounded below 0"),
                );
            }
            let corrected = !ctx.reproduce();
            u.iter()
                .map(|&p| {
                    let mean = vstats::mean(&val);
                    let sd = vstats::stddev(&val, corrected);
                    mean + cprob::ndtri(p).0 * sd
                })
                .collect()
        } else {
            r
        };
        for (j, &t) in keep.iter().enumerate() {
            input.data[t * nvox + v] = ctx.conv::<T>(out[j], op);
        }
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// -roi and -bptf

/// `-roi <xmin> <xsize> <ymin> <ysize> <zmin> <zsize> <tmin> <tsize>`: every voxel outside the
/// box zeroed (a size of -1: to the image's end; x, y, z in NIfTI's voxel order, mapped to
/// newimage's and rounded, then put in order).
pub(super) fn roi<T: Arith>(
    ctx: &mut Ctx,
    input: &mut Volume<T>,
    argv: &[String],
    i: usize,
) -> Res<()> {
    let mut a = [0i32; 8];
    for (k, v) in a.iter_mut().enumerate() {
        *v = cnum::atoi(number_arg(ctx, argv, i + 1 + k)?);
    }
    let end = |lo: i32, size: i32, max: i64| {
        let hi = lo.wrapping_add(size).wrapping_sub(1);
        if hi == lo.wrapping_sub(2) {
            max as i32
        } else {
            hi
        }
    };
    let (x0, y0, z0, t0) = (a[0], a[2], a[4], a[6]);
    let x1 = end(x0, a[1], input.xsize() - 1);
    let y1 = end(y0, a[3], input.ysize() - 1);
    let z1 = end(z0, a[5], input.zsize() - 1);
    let t1 = end(t0, a[7], input.tsize() - 1);
    let m = input.niftivox2newimagevox_mat();
    let map = |x: i32, y: i32, z: i32| -> [i32; 3] {
        let v = [f64::from(x), f64::from(y), f64::from(z), 1.0];
        let mut r = [0i32; 3];
        for (row, out) in r.iter_mut().enumerate() {
            let mut s = 0.0;
            for (c, &vc) in v.iter().enumerate() {
                s += m.at(row, c) * vc;
            }
            *out = volume::round_f64(s);
        }
        r
    };
    let [mut x0, mut y0, mut z0] = map(x0, y0, z0);
    let [mut x1, mut y1, mut z1] = map(x1, y1, z1);
    if x0 > x1 {
        std::mem::swap(&mut x0, &mut x1);
    }
    if y0 > y1 {
        std::mem::swap(&mut y0, &mut y1);
    }
    if z0 > z1 {
        std::mem::swap(&mut z0, &mut z1);
    }
    let (nx, ny, nz) = (input.xsize(), input.ysize(), input.zsize());
    let mut k = 0usize;
    for t in 0..input.tsize().max(0) {
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let (x, y, z, t) = (x as i32, y as i32, z as i32, t as i32);
                    if x < x0 || x > x1 || y < y0 || y > y1 || z < z0 || z > z1 || t < t0 || t > t1
                    {
                        input.data[k] = T::from_int(0);
                    }
                    k += 1;
                }
            }
        }
    }
    Ok(())
}

/// `-bptf <hp_sigma> <lp_sigma>`; FSL-171: a sigma beyond int's range.
pub(super) fn bptf<T: Arith>(
    ctx: &mut Ctx,
    input: &mut Volume<T>,
    argv: &[String],
    i: usize,
) -> Res<()> {
    let hp = cnum::atof(number_arg(ctx, argv, i + 1)?);
    let lp = cnum::atof(number_arg(ctx, argv, i + 2)?);
    let (hm, lm) = filters::bptf_half_widths(hp, lp);
    let bad = |on: bool, m: i32| {
        on && (m < 0 || m.checked_add(m).and_then(|x| x.checked_add(1)).is_none())
    };
    if bad(hp > 0.0, hm) || bad(lp > 0.0, lm) {
        ctx.notice(
            &defects::FSL_171,
            "-bptf: a sigma beyond the filter's range",
        );
        if !ctx.reproduce() {
            ctx.eput(&format!(
                "larmorx-nc: error: fslmaths: -bptf: a sigma of {} or {} volumes is beyond the filters' range ({})\n",
                cnum::fmt_g(hp, 6),
                cnum::fmt_g(lp, 6),
                defects::FSL_171.id
            ));
            return Err(Stop::Exit(1));
        }
        match filters::bptf_stop(hp, lp) {
            Some(filters::BptfStop::BadLength) => {
                return Err(Stop::Exception(FslError {
                    printed: Vec::new(),
                    type_name: "std::bad_array_new_length",
                    what: "std::bad_array_new_length".into(),
                }));
            }
            Some(filters::BptfStop::Crash) => {
                return Err(ctx.refuse(&defects::FSL_171, "-bptf: a sigma beyond int's range"));
            }
            None => {}
        }
    }
    let out = filters::bandpass_temporal_filter(input, hp, lp, &mut |x| ctx.conv::<T>(x, "-bptf"));
    *input = out;
    Ok(())
}
