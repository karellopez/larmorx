// SPDX-License-Identifier: Apache-2.0
//! fslmaths' other spatial operations (`specs/fslmaths.md` §10.3-§10.7): `-subsamp2` and
//! `-subsamp2offc`, `-edge`, `-dog_edge`, `-fillh` and `-fillh26`, `-inm` and `-ing`.
//!
//! Each output voxel depends on its own neighbourhood only and is computed in parallel; the
//! whole-image means are block sums added in order (§18), so nothing depends on the thread
//! count.

use std::io::Write;

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_core::math::{self, Libm};
use rayon::prelude::*;

use super::filters::{EMPTY_MASK, abort_lines};
use super::image::Img;
use super::num::{self, Kind, Num};
use super::ops::{self, Stop};

// ------------------------------------------------------------------------------------------
// -subsamp2, -subsamp2offc (§10.3)

/// The new grid's geometry: voxel sizes doubled, the matrices as [`super::image::regrid_x`]
/// maps them (a new voxel `i'` at old voxel `2 i'`, `offc`: `2 i' + ½`, in the frame the image
/// is held in; findings 24).
fn subsample_header<T>(img: &mut Img<T>, offc: bool, nx_new: usize) {
    let half = if offc { 0.5 } else { 0.0 };
    let map = [
        [2.0, 0.0, 0.0, half],
        [0.0, 2.0, 0.0, half],
        [0.0, 0.0, 2.0, half],
    ];
    super::image::regrid_x(img, nx_new, Some(map));
    let h = &mut img.header;
    for a in 1..4 {
        h.pixdim[a] = f64::from((h.pixdim[a] * 2.0) as f32);
    }
    if h.qform_code > 0 {
        // a set qform's rotation and scale as FSL's product of matrices gives it: its zero
        // entries +0 (a qfac of -1 negates a column of the quaternion's matrix, and the
        // product's sums turn those -0 into +0), then the quaternion of that matrix (random
        // testing: FSL's quatern_c is -0 after a subsampling of such an image; findings 24)
        use crate::bet::quatern::{mat_to_quatern, quatern_to_mat};
        let pd = [h.pixdim[0], h.pixdim[1], h.pixdim[2], h.pixdim[3]];
        let m = quatern_to_mat(h.quatern_b, h.quatern_c, h.quatern_d, pd)
            .map(|row| row.map(|v| if v == 0.0 { 0.0 } else { v }));
        let ([b, c, d], qfac) = mat_to_quatern(&m);
        h.quatern_b = b;
        h.quatern_c = c;
        h.quatern_d = d;
        h.pixdim[0] = qfac;
        img.own_quaternion = true;
    }
}

/// One volume subsampled: `0.125 c + 0.0625 F + 0.0312 E + 0.0156 K` at old voxel `2i`
/// (`F`, `E`, `K` its face, edge and corner neighbours' sums), or (`offc`) the mean of the
/// 2 × 2 × 2 block from `2i`; neighbours outside take the nearest edge voxel's value.
fn subsample_volume<T: Num>(
    src: &[T],
    dims: [usize; 3],
    out_dims: [usize; 3],
    offc: bool,
    fix: bool,
) -> (Vec<T>, bool) {
    let [nx, ny, nz] = dims;
    let [ox, oy, _] = out_dims;
    let at = |x: i64, y: i64, z: i64| -> T {
        let c = |v: i64, n: usize| v.clamp(0, n as i64 - 1) as usize;
        src[c(x, nx) + nx * (c(y, ny) + ny * c(z, nz))]
    };
    let mut out = vec![T::zero(); out_dims.iter().product()];
    let changed = out
        .par_chunks_mut((ox * oy).max(1))
        .enumerate()
        .map(|(k, o)| {
            let mut ch = false;
            for j in 0..oy {
                for i in 0..ox {
                    let (x, y, z) = (2 * i as i64, 2 * j as i64, 2 * k as i64);
                    // the sums in double, not in the computing type (a char image's sums do
                    // not wrap: random testing, findings 29)
                    let add = |s: f64, v: T| {
                        let v = v.to_f64();
                        num::nan64(s, v, s + v)
                    };
                    let r = if offc {
                        let mut s = 0.0f64;
                        for dz in 0..2 {
                            for dy in 0..2 {
                                for dx in 0..2 {
                                    s = add(s, at(x + dx, y + dy, z + dz));
                                }
                            }
                        }
                        s / 8.0
                    } else {
                        let (mut f, mut e, mut kk) = (0.0f64, 0.0f64, 0.0f64);
                        for dz in -1i64..=1 {
                            for dy in -1i64..=1 {
                                for dx in -1i64..=1 {
                                    let v = at(x + dx, y + dy, z + dz);
                                    match dx.abs() + dy.abs() + dz.abs() {
                                        1 => f = add(f, v),
                                        2 => e = add(e, v),
                                        3 => kk = add(kk, v),
                                        _ => {}
                                    }
                                }
                            }
                        }
                        let c = at(x, y, z).to_f64();
                        let mut r = 0.125 * c;
                        for (w, s) in [(0.0625, f), (0.0312, e), (0.0156, kk)] {
                            let p = w * s;
                            r = num::nan64(r, p, r + p);
                        }
                        r
                    };
                    let (t, c) = T::convert(r, fix);
                    ch |= c;
                    o[i + ox * j] = t;
                }
            }
            ch
        })
        .reduce(|| false, |a, b| a || b);
    (out, changed)
}

/// FSL-75 (the register's row): FSL computes the background value of the volume it subsamples
/// (the 10th percentile of its 2-voxel border) and never uses it; for some sizes its buffer for
/// the border values is too small and it writes past it (heap corruption: FSL aborts, crashes
/// or finishes, depending on its heap). Named in both modes where that happens; no output voxel
/// depends on it.
fn note_75(notices: &mut Notices, op: &str, dims: [usize; 3]) {
    let [x, y, z] = dims.map(|n| n as i64);
    let [ex, ey, ez] = [x, y, z].map(|n| 2.min(n - 1).max(0));
    let (ix, iy) = ((x - 2 * ex).max(0), (y - 2 * ey).max(0));
    let values = 2 * ez * ix * iy + 2 * ey * ix * z + 2 * ex * y * z;
    let buffer = 2 * (x - 2 * ex) * (y - 2 * ey) * ez + 2 * (x - 2 * ex) * z * ey + 2 * y * z * ex;
    if x * y * z > 1 && buffer >= values {
        return;
    }
    let what = if x * y * z <= 1 {
        "no border value: FSL reads one from outside its values".to_owned()
    } else {
        format!(
            "FSL writes {values} border values into a buffer of {} (its heap corrupted: FSL may abort or crash)",
            buffer.max(0)
        )
    };
    notices.add(
        &defects::FSL_75,
        format!(
            "{op}: the background value of a {x} x {y} x {z} image, which FSL computes and never \
             uses: {what}; no output voxel depends on it"
        ),
    );
}

/// `-subsamp2` (`offc` false) and `-subsamp2offc`; FSL-163 for a 4D image.
pub fn subsample<T: Num>(img: &mut Img<T>, offc: bool, notices: &mut Notices) -> Result<(), Stop> {
    let [nx, ny, nz, nt] = img.dims;
    let fix = notices.fix();
    let op = if offc { "-subsamp2offc" } else { "-subsamp2" };
    let mut volumes = nt;
    if nt > 1 {
        let odd = [nx, ny, nz].iter().any(|n| n % 2 == 1);
        if fix {
            notices.add(
                &defects::FSL_163,
                format!(
                    "{op} of a 4D image: every volume subsampled (FSL {})",
                    if offc && !odd {
                        "keeps the first volume only"
                    } else {
                        "stops: 3D only method called by higher-dimensional volume."
                    }
                ),
            );
        } else if offc && !odd {
            notices.add(
                &defects::FSL_163,
                format!("{op} of a 4D image of even sizes: the first volume only, as FSL does"),
            );
            volumes = 1;
        } else {
            let what = "3D only method called by higher-dimensional volume.";
            return Err(Stop::Exception(format!(
                "Image Exception : #75 :: {what}\n{}",
                abort_lines("std::runtime_error", what)
            )));
        }
    }
    let dims = [nx, ny, nz];
    note_75(notices, op, dims);
    let out_dims = dims.map(|n| n.div_ceil(2));
    let per = nx * ny * nz;
    let mut data = Vec::with_capacity(out_dims.iter().product::<usize>() * volumes);
    let mut changed = false;
    for t in 0..volumes {
        let (o, ch) =
            subsample_volume(&img.data[t * per..(t + 1) * per], dims, out_dims, offc, fix);
        changed |= ch;
        data.extend_from_slice(&o);
    }
    subsample_header(img, offc, out_dims[0]);
    img.data = data;
    img.dims = [out_dims[0], out_dims[1], out_dims[2], volumes];
    ops::note151(notices, changed, "-subsamp2's values");
    Ok(())
}

// ------------------------------------------------------------------------------------------
// -edge (§10.4)

/// `-edge`: the gradient magnitude by central differences, inside a one-voxel border
/// (in-plane only when the image has at most 2 slices).
pub fn edge<T: Num>(img: &mut Img<T>, notices: &mut Notices) {
    let fix = notices.fix();
    let [nx, ny, nz, _] = img.dims;
    let planar = nz <= 2;
    let axes = if planar { 2 } else { 3 };
    let vox = img.voxel();
    // the denominator over the three axes, also when the differences are in-plane (observed)
    let inv: f64 = (0..3).map(|a| 1.0 / (vox[a] * vox[a])).sum();
    let den = f64::from((2.0 * inv.sqrt()) as f32);
    let per = nx * ny * nz;
    let plane = nx * ny;
    let src = img.data.clone();
    let changed = img
        .data
        .par_chunks_mut(plane.max(1))
        .enumerate()
        .map(|(zt, o)| {
            let (z, t) = (zt % nz, zt / nz);
            let vol = &src[t * per..(t + 1) * per];
            let mut ch = false;
            if !planar && (z == 0 || z + 1 >= nz) {
                return false;
            }
            for y in 1..ny.saturating_sub(1) {
                for x in 1..nx.saturating_sub(1) {
                    let i = x + nx * (y + ny * z);
                    let steps = [1, nx, plane];
                    let mut sum = 0.0f64;
                    // The squares are exact products `d · d` and the terms are added from the
                    // last axis to the first (observed, black-box runs of FSL on a double
                    // image: §10.4's `pow(x, 2)` is FSL's compiled `x · x`, never glibc's pow,
                    // and only this order gives FSL's bits; `validation/investigations/
                    // cleanroom-glibc-dbl/edge_forms.py`); a float or integer image's sum is
                    // the same in any order once stored.
                    for a in (0..axes).rev() {
                        let (p, m) = (vol[i + steps[a]], vol[i - steps[a]]);
                        // C's arithmetic: char and short differences as int, not wrapped to
                        // the type (random testing, findings 29)
                        let d = match T::KIND {
                            Kind::Float | Kind::Double => p.sub(m).to_f64(),
                            _ => p.diff(m),
                        };
                        // x86-64's NaN rules (a voxel size of 0: 0 / 0, inf / inf)
                        let dd = d * d;
                        let vv = vox[a] * vox[a];
                        let q = num::nan64(dd, vv, dd / vv);
                        sum = num::nan64(sum, q, sum + q);
                    }
                    let r = super::reduce::sqrt64(sum);
                    let (v, c) = T::convert(num::nan64(r, den, r / den), fix);
                    ch |= c;
                    o[x + nx * y] = v;
                }
            }
            ch
        })
        .reduce(|| false, |a, b| a || b);
    ops::note151(notices, changed, "-edge's values");
}

// ------------------------------------------------------------------------------------------
// -dog_edge (§10.5)

/// The 2D Gaussian of `-dog_edge` of radius `r` (in voxels), normalised by `1 / Σ`; `libm`'s
/// `exp` (glibc's with `reproduce`, GLIBC-07; not named: `-dog_edge` is a filter).
fn dog_gauss(sigma: f64, r: i64, libm: &Libm) -> Vec<f32> {
    let n = (2 * r + 1) as usize;
    let mut g = vec![0.0f32; n * n];
    if sigma <= 1e-6 {
        g[r as usize + n * r as usize] = 1.0;
    } else {
        for j in 0..n {
            for i in 0..n {
                let (di, dj) = (i as i64 - r, j as i64 - r);
                let q = -((di * di + dj * dj) as f64) / (2.0 * sigma * sigma);
                let e = |glibc: bool| {
                    num::to_f32(if glibc {
                        math::exp_glibc(q)
                    } else {
                        math::exp(q)
                    })
                };
                g[i + n * j] = e(libm.is_glibc());
            }
        }
    }
    let s = g.iter().fold(0.0f32, |a, &v| num::nan32(a, v, a + v));
    let inv = num::to_f32(1.0 / f64::from(s));
    for v in &mut g {
        *v = num::nan32(*v, inv, *v * inv);
    }
    g
}

/// `-dog_edge σ₁ σ₂`: 1 at the zero crossings of the image filtered by the difference of two
/// in-plane Gaussians, 0 elsewhere (FSL-161 for a negative radius, FSL-164 for a 4D image).
pub fn dog_edge<T: Num>(
    img: &mut Img<T>,
    s1: f64,
    s2: f64,
    notices: &mut Notices,
) -> Result<(), Stop> {
    let r = i64::from(num::cvtt(4.0 * s2));
    let fix = notices.fix();
    if r < 0 && fix {
        notices.add(
            &defects::FSL_161,
            format!("-dog_edge {s1} {s2}: a kernel of radius {r} is refused (FSL filters with a kernel without voxels)"),
        );
        return Err(Stop::Refuse(format!(
            "-dog_edge: the kernel's radius (int)(4 * {s2}) = {r} is negative (FSL-161: FSL \
             filters with a kernel without voxels)"
        )));
    }
    let [nx, ny, nz, nt] = img.dims;
    let per = nx * ny * nz;
    // quiet: GLIBC-07 is named only where a voxel's value is a function of its own (§18;
    // the user's decision of 2026-11-04): a filter cannot tell without running twice
    let libm = Libm::quiet(notices.mode());
    let kernel = if r >= 0 {
        let (g1, g2) = (dog_gauss(s1, r, &libm), dog_gauss(s2, r, &libm));
        let data: Vec<f32> = g2
            .iter()
            .zip(&g1)
            .map(|(&b, &a)| num::nan32(b, a, b - a))
            .collect();
        let n = (2 * r + 1) as usize;
        super::kernel::Kernel::from_values([n, n, 1], data)
    } else {
        super::kernel::Kernel::none(false)
    };
    if nt > 1 {
        let detail = match notices.mode() {
            UpstreamDefects::Fix => {
                "-dog_edge of a 4D image: every volume filtered (FSL filters the first only and takes the others' crossings from their raw values)"
            }
            UpstreamDefects::Reproduce => {
                "-dog_edge of a 4D image: the first volume filtered, the others' crossings from their raw values, as FSL does"
            }
        };
        notices.add(&defects::FSL_164, detail);
    }
    let mut out = vec![T::zero(); img.data.len()];
    for t in 0..nt {
        let vol = &img.data[t * per..(t + 1) * per];
        let f: Vec<f64> = if t == 0 || fix {
            filtered(vol, [nx, ny, nz], &kernel)
        } else {
            vol.iter().map(|v| v.to_f64()).collect()
        };
        crossings(&f, [nx, ny, nz], &mut out[t * per..(t + 1) * per]);
    }
    img.data = out;
    Ok(())
}

/// A volume convolved with a kernel by the direct sum (single-precision sums) and held in the
/// computing type, as doubles.
fn filtered<T: Num>(vol: &[T], dims: [usize; 3], k: &super::kernel::Kernel) -> Vec<f64> {
    let [nx, ny, nz] = dims;
    let [kx, ky, kz] = k.size();
    let [cx, cy, cz] = k.centre();
    let double = T::KIND == num::Kind::Double;
    let mut out = vec![0.0f64; vol.len()];
    out.par_chunks_mut((nx * ny).max(1))
        .enumerate()
        .for_each(|(z, o)| {
            for y in 0..ny {
                for x in 0..nx {
                    let mut sum = 0.0f32;
                    for l in 0..kz {
                        let pz = z as i64 + l as i64 - cz;
                        if pz < 0 || pz >= nz as i64 {
                            continue;
                        }
                        for j in 0..ky {
                            let py = y as i64 + j as i64 - cy;
                            if py < 0 || py >= ny as i64 {
                                continue;
                            }
                            for i in 0..kx {
                                let px = x as i64 + i as i64 - cx;
                                if px < 0 || px >= nx as i64 {
                                    continue;
                                }
                                let kv = k.data[i + kx * (j + ky * l)];
                                let s = vol[px as usize + nx * (py as usize + ny * pz as usize)];
                                if double {
                                    let (a, b) = (s.to_f64(), f64::from(kv));
                                    sum = num::acc32(sum, num::nan64(a, b, a * b));
                                } else {
                                    let a = num::to_f32(s.to_f64());
                                    let p = num::nan32(a, kv, a * kv);
                                    sum = num::nan32(sum, p, sum + p);
                                }
                            }
                        }
                    }
                    // held in the computing type (an integer type truncates: observed)
                    let v = num::to_f64(sum);
                    o[x + nx * y] = if T::KIND.is_int() {
                        T::from_f64_c(v).to_f64()
                    } else {
                        v
                    };
                }
            }
        });
    out
}

/// 1 at a zero crossing inside a one-voxel border: `f > 0` with a 6-neighbour `≤ −f`, or
/// `f < 0` with one `> −f`.
fn crossings<T: Num>(f: &[f64], dims: [usize; 3], out: &mut [T]) {
    let [nx, ny, nz] = dims;
    let plane = nx * ny;
    out.par_chunks_mut(plane.max(1))
        .enumerate()
        .for_each(|(z, o)| {
            if z == 0 || z + 1 >= nz {
                return;
            }
            for y in 1..ny.saturating_sub(1) {
                for x in 1..nx.saturating_sub(1) {
                    let i = x + nx * (y + ny * z);
                    let v = f[i];
                    let nb = [i - 1, i + 1, i - nx, i + nx, i - plane, i + plane];
                    let hit = if v > 0.0 {
                        nb.iter().any(|&j| f[j] <= -v)
                    } else if v < 0.0 {
                        nb.iter().any(|&j| f[j] > -v)
                    } else {
                        false
                    };
                    if hit {
                        o[x + nx * y] = T::one();
                    }
                }
            }
        });
}

// ------------------------------------------------------------------------------------------
// -fillh, -fillh26 (§10.6)

/// `-fillh` (6-connectivity) and `-fillh26`: the image binarised as `-bin`; in each volume the
/// zero components that touch no face of the image become 1 (FSL-165: an axis of size 1).
pub fn fill_holes<T: Num>(
    img: &mut Img<T>,
    conn26: bool,
    notices: &mut Notices,
) -> Result<(), Stop> {
    let op = if conn26 { "-fillh26" } else { "-fillh" };
    ops::bin_values(&mut img.data, notices, op);
    let [nx, ny, nz, nt] = img.dims;
    // FSL crashes on any image with a size of 1 along an axis, holes or not (random testing,
    // findings 27)
    let flat = [nx, ny, nz].contains(&1);
    if flat {
        if notices.reproduce() {
            return Err(Stop::Refuse(format!(
                "{op} of an image with a size of 1 along an axis (FSL-165: FSL crashes here, an \
                 integer division by zero)"
            )));
        }
        notices.add(
            &defects::FSL_165,
            format!("{op} of an image with a size of 1 along an axis: that axis has no edge, the holes are filled in the image's plane (FSL crashes)"),
        );
    }
    let per = nx * ny * nz;
    img.data.par_chunks_mut(per.max(1)).for_each(|vol| {
        fill_volume(vol, [nx, ny, nz], conn26);
    });
    let _ = nt;
    Ok(())
}

fn fill_volume<T: Num>(vol: &mut [T], dims: [usize; 3], conn26: bool) {
    let [nx, ny, nz] = dims;
    let n = vol.len();
    let on_face = |i: usize| {
        let (x, y, z) = (i % nx, (i / nx) % ny, i / (nx * ny));
        let face = |c: usize, len: usize| len > 1 && (c == 0 || c + 1 == len);
        face(x, nx) || face(y, ny) || face(z, nz)
    };
    // the zero voxels reached from the faces
    let mut outside = vec![false; n];
    let mut stack: Vec<usize> = (0..n)
        .filter(|&i| vol[i] == T::zero() && on_face(i))
        .collect();
    for &i in &stack {
        outside[i] = true;
    }
    while let Some(i) = stack.pop() {
        let (x, y, z) = (
            (i % nx) as i64,
            ((i / nx) % ny) as i64,
            (i / (nx * ny)) as i64,
        );
        for dz in -1i64..=1 {
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let m = dx.abs() + dy.abs() + dz.abs();
                    if m == 0 || (!conn26 && m > 1) {
                        continue;
                    }
                    let (px, py, pz) = (x + dx, y + dy, z + dz);
                    if px < 0
                        || py < 0
                        || pz < 0
                        || px >= nx as i64
                        || py >= ny as i64
                        || pz >= nz as i64
                    {
                        continue;
                    }
                    let j = px as usize + nx * (py as usize + ny * pz as usize);
                    if !outside[j] && vol[j] == T::zero() {
                        outside[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
    }
    for (v, &o) in vol.iter_mut().zip(&outside) {
        if *v == T::zero() && !o {
            *v = T::one();
        }
    }
}

// ------------------------------------------------------------------------------------------
// -inm, -ing (§10.7)

/// The block sum of §18: `n` values, `b = max(⌊√n⌋, 10000)`; every `b + 1` counted values the
/// running sum added to the total (one pass in storage order, no copy of the values).
fn block_mean<T: Num>(vals: &[T], keep: &(dyn Fn(T) -> bool + Sync)) -> (f64, usize) {
    let n = vals.len();
    let b = ((n as f64).sqrt().floor() as usize).max(10000);
    let (mut total, mut run) = (0.0f64, 0.0f64);
    let (mut count, mut in_run) = (0usize, 0usize);
    for &v in vals {
        if !keep(v) {
            continue;
        }
        let x = v.to_f64();
        run = num::nan64(run, x, run + x);
        count += 1;
        in_run += 1;
        if in_run == b + 1 {
            total = num::nan64(total, run, total + run);
            run = 0.0;
            in_run = 0;
        }
    }
    (num::nan64(total, run, total + run), count)
}

/// `-inm m` (`global` false: each volume) and `-ing m` (the whole image): multiplied by
/// `m / mean` of the voxels above 0 (as `-bin`).
pub fn normalise<T: Num>(
    img: &mut Img<T>,
    m: f64,
    global: bool,
    notices: &mut Notices,
    err: &mut dyn Write,
) {
    let fix = notices.fix();
    let per = img.per_volume();
    let groups = if global { 1 } else { img.dims[3] };
    let len = if global { img.data.len() } else { per };
    let mut changed = false;
    for g in 0..groups {
        let vals = &mut img.data[g * len..(g + 1) * len];
        let far = far_upper(vals);
        let keep = move |v: T| v > T::zero() && (fix || v < far);
        let (s, n) = block_mean(vals, &keep);
        if far_differs(vals, fix, far) {
            note_far152(notices, vals, if global { "-ing" } else { "-inm" });
        }
        // an empty mask: its mean 0 (FSL's message; m / 0, so zeros become NaN, rule C)
        let mean = if n == 0 {
            let _ = writeln!(err, "{EMPTY_MASK}");
            0.0
        } else {
            s / n as f64
        };
        let f = num::nan64(m, mean, m / mean);
        changed |= ops::map(vals, |v| {
            let x = v.to_f64();
            T::convert(num::nan64(x, f, x * f), fix)
        });
    }
    ops::note151(notices, changed, "-inm's or -ing's values");
}

/// Whether FSL's mask (`0 < v < max + 1`, FSL-152 and FSL-153) differs from `v > 0`.
fn far_differs<T: Num>(vals: &[T], fix: bool, far: T) -> bool {
    let _ = fix;
    ops::any(vals, |v| v > T::zero() && !(v < far))
}

/// FSL-152 or FSL-153 named for the mask of `-inm` or `-ing`.
fn note_far152<T: Num>(notices: &mut Notices, vals: &[T], what: &str) {
    let nan_first = vals.first().is_some_and(|v| v.is_nan());
    let (d, why) = if nan_first {
        (
            &defects::FSL_153,
            "the image's first voxel is NaN, which makes FSL's extremes NaN and its mask empty",
        )
    } else {
        (
            &defects::FSL_152,
            "FSL's mask stops below the maximum plus 1 in the computing type",
        )
    };
    let detail = match notices.mode() {
        UpstreamDefects::Fix => format!("{what}: the mask holds every voxel above 0; {why}"),
        UpstreamDefects::Reproduce => format!("{what}: {why}, as FSL does"),
    };
    notices.add(d, detail);
}

/// FSL's upper far side of `-bin` (FSL-152): the maximum plus 1 in the computing type.
fn far_upper<T: Num>(vals: &[T]) -> T {
    ops::extremes(vals, false).map_or(T::zero(), |(_, hi)| hi.add(T::one()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maths::filters::tests::img;

    #[test]
    fn holes_inside_are_filled_and_fsl_165() {
        let mut data = vec![1.0f32; 27];
        data[13] = 0.0; // the centre: a hole
        data[0] = 0.0; // a corner: touches the faces
        let mut im = img([3, 3, 3, 1], data);
        fill_holes(&mut im, false, &mut Notices::new(UpstreamDefects::Fix)).unwrap();
        assert_eq!((im.data[13], im.data[0]), (1.0, 0.0));
        // a single slice: refused with reproduce, filled in its plane by default
        let mut data = vec![1.0f32; 9];
        data[4] = 0.0;
        let mut im = img([3, 3, 1, 1], data.clone());
        let r = fill_holes(
            &mut im,
            false,
            &mut Notices::new(UpstreamDefects::Reproduce),
        );
        assert!(matches!(r, Err(Stop::Refuse(_))));
        let mut im = img([3, 3, 1, 1], data);
        let mut ns = Notices::new(UpstreamDefects::Fix);
        fill_holes(&mut im, false, &mut ns).unwrap();
        assert_eq!(im.data[4], 1.0);
        assert!(ns.contains(&defects::FSL_165));
    }

    #[test]
    fn an_empty_mask_divides_by_zero() {
        let mut im = img([2, 1, 1, 1], vec![0.0, -3.0]);
        let mut err = Vec::new();
        normalise(
            &mut im,
            100.0,
            false,
            &mut Notices::new(UpstreamDefects::Fix),
            &mut err,
        );
        assert_eq!(im.data[0].to_bits(), num::DEFAULT_NAN_32);
        assert_eq!(im.data[1], f32::NEG_INFINITY);
        assert_eq!(String::from_utf8(err).unwrap(), format!("{EMPTY_MASK}\n"));
    }

    #[test]
    fn edge_with_a_zero_voxel_size_gives_x86_64s_default_nan() {
        // a flat image (0 / 0) and a step (inf / inf), voxel size 0 along x: the default NaN
        // (sign set) on every platform
        let mut flat = img([3, 3, 3, 1], vec![1.0f32; 27]);
        flat.header.pixdim[1] = 0.0;
        edge(&mut flat, &mut Notices::new(UpstreamDefects::Fix));
        assert_eq!(flat.data[13].to_bits(), num::DEFAULT_NAN_32);
        let mut step = img([3, 3, 3, 1], (0..27).map(|i| (i % 3) as f32).collect());
        step.header.pixdim[1] = 0.0;
        edge(&mut step, &mut Notices::new(UpstreamDefects::Fix));
        assert_eq!(step.data[13].to_bits(), num::DEFAULT_NAN_32);
    }

    #[test]
    fn subsampling_halves_the_grid() {
        let data: Vec<f32> = (0..8).map(|v| v as f32).collect();
        let mut im = img([2, 2, 2, 1], data);
        subsample(&mut im, true, &mut Notices::new(UpstreamDefects::Fix)).unwrap();
        assert_eq!((im.dims, im.data.clone()), ([1, 1, 1, 1], vec![3.5]));
        assert_eq!(im.header.pixdim[1], 2.0);
    }
}
