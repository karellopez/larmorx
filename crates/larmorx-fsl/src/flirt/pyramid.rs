// SPDX-License-Identifier: Apache-2.0
//! FLIRT's image filters (`specs/flirt.md` §5): the blur to a resolution, the isotropic
//! resampling (the shared engine's reference grids, `larmorx_mri::hmc::grid`), the halving of
//! the pyramid, their weighted forms, and the background value of a border shell (§10a, FSL-75).
//!
//! Every filter keeps a volume's x-fastest layout; the blur's passes and the halving's slices run
//! in parallel, each value computed alone, so the results do not depend on the thread count.

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::math;
use larmorx_mri::hmc::volume::Volume;
use rayon::prelude::*;

/// The blur kernel along an axis of voxel size `d` for resolution `r` (§5): `None` when no blur
/// applies; else the normalised taps (double precision).
pub fn blur_kernel(r: f32, d: f32) -> Option<Vec<f64>> {
    if d.abs() < 1e-8 {
        return None;
    }
    let ratio = r / d;
    if ratio < 1.1 {
        return None;
    }
    // σ = 0.85·ratio/2 with the constant in double, rounded to single (observed: `-debug`'s
    // levels bit for bit; the single-precision product misses the last bit of 2.5 % of them)
    let sigma = (0.85f64 * f64::from(ratio) / 2.0) as f32;
    if sigma < 0.5 {
        return None;
    }
    let n = 2 * ((sigma - 0.001) as i32) + 3;
    let c = n / 2 + 1;
    let s2 = sigma * sigma;
    let denom = f64::from(s2) * 4.0;
    let mut taps: Vec<f64> = (1..=n)
        .map(|k| {
            let q = ((k - c) * (k - c)) as f32;
            math::exp(-(f64::from(q) / denom))
        })
        .collect();
    // two interleaved accumulators (even and odd taps), then added
    let (mut even, mut odd) = (0.0f64, 0.0f64);
    for (i, &t) in taps.iter().enumerate() {
        if i % 2 == 0 {
            even += t;
        } else {
            odd += t;
        }
    }
    let total = even + odd;
    for t in &mut taps {
        *t /= total;
    }
    Some(taps)
}

/// One pass of a separable convolution along `axis` (§5): each output the float32 running
/// sum, in kernel order, of the double products of value and tap; values outside count 0; the
/// kernel centred at `(n − 1)/2`.
fn convolve_axis(data: &[f32], shape: [usize; 3], axis: usize, taps: &[f64]) -> Vec<f32> {
    let [nx, ny, _] = shape;
    let n = shape[axis] as i64;
    let stride = [1, nx, nx * ny][axis];
    let half = ((taps.len() - 1) / 2) as i64;
    let mut out = vec![0.0f32; data.len()];
    let plane = nx * ny;
    out.par_chunks_mut(plane.max(1))
        .enumerate()
        .for_each(|(z, slab)| {
            for (k, o) in slab.iter_mut().enumerate() {
                let idx = z * plane + k;
                let pos = [k % nx, k / nx, z][axis] as i64;
                let base = idx as i64 - pos * stride as i64;
                let mut acc = 0.0f32;
                for (t, &w) in taps.iter().enumerate() {
                    let j = pos + t as i64 - half;
                    if j >= 0 && j < n {
                        let v = data[(base + j * stride as i64) as usize];
                        acc = (f64::from(acc) + f64::from(v) * w) as f32;
                    }
                }
                *o = acc;
            }
        });
    out
}

/// One output value of the convolution along a line: the single-precision running sum, in kernel
/// order, of the double products of value and tap (values outside the line count 0).
#[inline(always)]
fn line_value(at: impl Fn(usize) -> f32, n: usize, pos: usize, taps: &[f64]) -> f32 {
    let half = (taps.len() - 1) / 2;
    let mut acc = 0.0f32;
    for (t, &w) in taps.iter().enumerate() {
        let j = pos as i64 + t as i64 - half as i64;
        if j >= 0 && (j as usize) < n {
            acc = (f64::from(acc) + f64::from(at(j as usize)) * w) as f32;
        }
    }
    acc
}

/// [`convolve_axis`] in place, with the same values: x and y line by line (a copy of the line),
/// z plane by plane (the original planes the kernel still needs kept aside), so that a blur holds
/// one copy of the volume, not three.
fn convolve_axis_in_place(data: &mut [f32], shape: [usize; 3], axis: usize, taps: &[f64]) {
    let [nx, ny, nz] = shape;
    let plane = nx * ny;
    if data.is_empty() {
        return;
    }
    match axis {
        0 => data.par_chunks_mut(nx.max(1)).for_each(|row| {
            let line = row.to_vec();
            for (x, o) in row.iter_mut().enumerate() {
                *o = line_value(|j| line[j], nx, x, taps);
            }
        }),
        1 => data.par_chunks_mut(plane.max(1)).for_each(|slab| {
            let mut line = vec![0.0f32; ny];
            for x in 0..nx {
                for (y, l) in line.iter_mut().enumerate() {
                    *l = slab[x + nx * y];
                }
                for y in 0..ny {
                    slab[x + nx * y] = line_value(|j| line[j], ny, y, taps);
                }
            }
        }),
        _ => {
            let half = (taps.len() - 1) / 2;
            // the original planes below z that the kernel still reads (a ring of `half` planes)
            let mut kept: std::collections::VecDeque<(usize, Vec<f32>)> =
                std::collections::VecDeque::new();
            let mut out = vec![0.0f32; plane];
            for z in 0..nz {
                {
                    let src = &*data;
                    // the planes each tap reads (None outside the volume), in kernel order
                    let planes: Vec<Option<&[f32]>> = (0..taps.len())
                        .map(|t| {
                            let j = z as i64 + t as i64 - half as i64;
                            if j < 0 || j >= nz as i64 {
                                None
                            } else if j as usize >= z {
                                let j = j as usize;
                                Some(&src[j * plane..(j + 1) * plane])
                            } else {
                                kept.iter()
                                    .find(|(i, _)| *i == j as usize)
                                    .map(|(_, p)| p.as_slice())
                            }
                        })
                        .collect();
                    out.par_iter_mut().enumerate().for_each(|(k, o)| {
                        let mut acc = 0.0f32;
                        for (pl, &w) in planes.iter().zip(taps) {
                            if let Some(pl) = pl {
                                acc = (f64::from(acc) + f64::from(pl[k]) * w) as f32;
                            }
                        }
                        *o = acc;
                    });
                }
                let cur = &mut data[z * plane..(z + 1) * plane];
                if half > 0 {
                    kept.push_back((z, cur.to_vec()));
                    while kept.len() > half {
                        kept.pop_front();
                    }
                }
                cur.copy_from_slice(&out);
            }
        }
    }
}

/// The blur of `v` to resolution `r` mm (§5): a separable convolution along x, y, z.
pub fn blur(v: &Volume, r: f32) -> Volume {
    let mut data = v.data.clone();
    for axis in 0..3 {
        if let Some(taps) = blur_kernel(r, v.voxel_size[axis]) {
            convolve_axis_in_place(&mut data, v.shape, axis, &taps);
        }
    }
    Volume::new(v.shape, v.voxel_size, data)
}

/// The 3 × 3 × 3 box sum of `v` (BBR's boundary test, §6.5): the separable convolution with taps
/// `[1, 1, 1]`.
pub fn box3(v: &Volume) -> Volume {
    let mut data = v.data.clone();
    for axis in 0..3 {
        data = convolve_axis(&data, v.shape, axis, &[1.0, 1.0, 1.0]);
    }
    Volume::new(v.shape, v.voxel_size, data)
}

/// The isotropic resampling to `s` mm (§5): the shared engine's reference grid (trilinear, zero
/// outside, float32 steps `s/d`).
pub fn isotropic(v: &Volume, s: f32) -> Volume {
    larmorx_mri::hmc::grid::subsample(v, s)
}

/// The halving of `v` (§5): a grid of `⌈n/2⌉` points per axis at the even voxels, each the
/// 3 × 3 × 3 binomial-like average with neighbours outside the image 0.
pub fn halve(v: &Volume) -> Volume {
    let [nx, ny, nz] = v.shape;
    let [mx, my, mz] = [nx.div_ceil(2), ny.div_ceil(2), nz.div_ceil(2)];
    let at = |x: i64, y: i64, z: i64| -> f32 {
        if x < 0 || y < 0 || z < 0 || x >= nx as i64 || y >= ny as i64 || z >= nz as i64 {
            0.0
        } else {
            v.data[x as usize + nx * (y as usize + ny * z as usize)]
        }
    };
    let mut data = vec![0.0f32; mx * my * mz];
    let plane = mx * my;
    data.par_chunks_mut(plane.max(1))
        .enumerate()
        .for_each(|(k, slab)| {
            let z = 2 * k as i64;
            for j in 0..my {
                let y = 2 * j as i64;
                for i in 0..mx {
                    let x = 2 * i as i64;
                    let c = at(x, y, z);
                    let faces = at(x + 1, y, z)
                        + at(x - 1, y, z)
                        + at(x, y + 1, z)
                        + at(x, y - 1, z)
                        + at(x, y, z + 1)
                        + at(x, y, z - 1);
                    let edges = at(x + 1, y + 1, z)
                        + at(x + 1, y - 1, z)
                        + at(x - 1, y + 1, z)
                        + at(x - 1, y - 1, z)
                        + at(x + 1, y, z + 1)
                        + at(x + 1, y, z - 1)
                        + at(x - 1, y, z + 1)
                        + at(x - 1, y, z - 1)
                        + at(x, y + 1, z + 1)
                        + at(x, y + 1, z - 1)
                        + at(x, y - 1, z + 1)
                        + at(x, y - 1, z - 1);
                    let corners = at(x + 1, y + 1, z + 1)
                        + at(x + 1, y + 1, z - 1)
                        + at(x + 1, y - 1, z + 1)
                        + at(x + 1, y - 1, z - 1)
                        + at(x - 1, y + 1, z + 1)
                        + at(x - 1, y + 1, z - 1)
                        + at(x - 1, y - 1, z + 1)
                        + at(x - 1, y - 1, z - 1);
                    let value = 0.125 * f64::from(c)
                        + 0.0625 * f64::from(faces)
                        + 0.0312 * f64::from(edges)
                        + 0.0156 * f64::from(corners);
                    slab[i + mx * j] = value as f32;
                }
            }
        });
    Volume::new([mx, my, mz], v.voxel_size.map(|d| d * 2.0), data)
}

/// `B = [W ≥ 0.01]`: 1 where the weight is at least 0.01 (and at most its maximum), else 0.
pub fn weight_mask(w: &Volume) -> Volume {
    let max = w
        .data
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, |m, x| if x > m { x } else { m });
    let data = w
        .data
        .iter()
        .map(|&x| if x >= 0.01 && x <= max { 1.0 } else { 0.0 })
        .collect();
    Volume::new(w.shape, w.voxel_size, data)
}

/// A filter `f` applied to an image `i` with weight `w` (§5): `f(I·B)/f(B)` where `f(B) ≠ 0`
/// (else 0), and the weight as `f(W)·[f(B) ≥ 0.9]`.
pub fn weighted(i: &Volume, w: &Volume, f: impl Fn(&Volume) -> Volume) -> (Volume, Volume) {
    let b = weight_mask(w);
    let ib = Volume::new(
        i.shape,
        i.voxel_size,
        i.data.iter().zip(&b.data).map(|(&x, &m)| x * m).collect(),
    );
    let fib = f(&ib);
    let fb = f(&b);
    let fw = f(w);
    let img = fib
        .data
        .iter()
        .zip(&fb.data)
        .map(|(&a, &d)| if d != 0.0 { a / d } else { 0.0 })
        .collect();
    let wt = fw
        .data
        .iter()
        .zip(&fb.data)
        .map(|(&a, &d)| if d >= 0.9 { a } else { 0.0 })
        .collect();
    (
        Volume::new(fib.shape, fib.voxel_size, img),
        Volume::new(fw.shape, fw.voxel_size, wt),
    )
}

/// The background value of a volume (§10a): the 10th percentile of its border shell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Background {
    pub value: f32,
    /// FSL's buffer is too small for the shell (FSL-75): FSL corrupts its heap here.
    pub overflow: bool,
    /// A NaN lies in the shell (FSL-22: no defined order in FSL's sort).
    pub nan: bool,
}

/// The background value of `v` (§10a). `None` for a 1 × 1 × 1 volume (no border: refused).
pub fn background(v: &Volume, mode: UpstreamDefects) -> Option<Background> {
    let [x, y, z] = v.shape.map(|n| n as i64);
    if x * y * z <= 1 {
        return None;
    }
    let e = [x, y, z].map(|n| 2.min(n - 1).max(0));
    let [ex, ey, ez] = e;
    let at = |i: i64, j: i64, k: i64| v.data[(i + x * (j + y * k)) as usize];
    let mut values: Vec<f32> = Vec::new();
    // the xy faces, for each depth
    for d in 0..ez {
        for i in ex..x - ex {
            for j in ey..y - ey {
                values.push(at(i, j, d));
                values.push(at(i, j, z - 1 - d));
            }
        }
    }
    // the xz faces
    for d in 0..ey {
        for i in ex..x - ex {
            for k in 0..z {
                values.push(at(i, d, k));
                values.push(at(i, y - 1 - d, k));
            }
        }
    }
    // the yz faces
    for d in 0..ex {
        for j in 0..y {
            for k in 0..z {
                values.push(at(d, j, k));
                values.push(at(x - 1 - d, j, k));
            }
        }
    }
    let buffer = 2 * (x - 2 * ex) * (y - 2 * ey) * ez + 2 * (x - 2 * ex) * z * ey + 2 * y * z * ex;
    let overflow = buffer < values.len() as i64;
    let nan = values.iter().any(|v| v.is_nan());
    let kept: &mut [f32] = if mode.reproduce() && overflow {
        let n = buffer.max(0) as usize;
        &mut values[..n]
    } else {
        &mut values[..]
    };
    if kept.is_empty() {
        return None;
    }
    // NaN last (FSL-22 corrected); every other value in order
    kept.sort_unstable_by(|a, b| match (a.is_nan(), b.is_nan()) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
    });
    let n = kept.len();
    Some(Background {
        value: kept[n / 10],
        overflow,
        nan,
    })
}

/// The notices of a background value: FSL-75 where FSL's buffer overflows, FSL-22 where a NaN
/// lies in the shell.
pub fn background_notices(b: &Background, what: &str, mode: UpstreamDefects) -> Vec<Notice> {
    let mut out = Vec::new();
    if b.overflow {
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "{what}: the background value is the 10th percentile of every border voxel; FSL's \
                 buffer for them is too small and FSL writes past it (its heap is corrupted)"
            ),
            UpstreamDefects::Reproduce => format!(
                "{what}: the background value from the border voxels FSL's too small buffer holds \
                 (FSL writes the others past it, corrupting its heap)"
            ),
        };
        out.push(Notice::new(&defects::FSL_75, mode, detail));
    }
    if b.nan {
        out.push(Notice::new(
            &defects::FSL_22,
            mode,
            format!("{what}: a NaN in the border; sorted after every number"),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernels() {
        assert!(blur_kernel(1.0, 1.0).is_none());
        assert!(blur_kernel(8.0, 0.0).is_none());
        let k = blur_kernel(8.0, 1.0).unwrap();
        // sigma = 3.4: 2·3 + 3 = 9 taps
        assert_eq!(k.len(), 9);
        let s: f64 = k.iter().sum();
        assert!((s - 1.0).abs() < 1e-12);
        assert!(k[4] > k[3] && (k[3] - k[5]).abs() < 1e-15);
    }

    #[test]
    fn halving_a_constant() {
        let v = Volume::new([4, 4, 4], [1.0; 3], vec![1.0; 64]);
        let h = halve(&v);
        assert_eq!(h.shape, [2, 2, 2]);
        assert_eq!(h.voxel_size, [2.0; 3]);
        // an interior-like point at (2, 2, 2) sees all 27 neighbours
        let full = 0.125 + 0.0625 * 6.0 + 0.0312 * 12.0 + 0.0156 * 8.0;
        assert_eq!(h.data[7], full as f32);
    }

    #[test]
    fn backgrounds() {
        let data: Vec<f32> = (0..125).map(|i| i as f32).collect();
        let v = Volume::new([5, 5, 5], [1.0; 3], data);
        let b = background(&v, UpstreamDefects::Fix).unwrap();
        assert_eq!(b.value, v.background());
        assert!(!b.overflow);
        let one = Volume::new([1, 1, 1], [1.0; 3], vec![3.0]);
        assert!(background(&one, UpstreamDefects::Fix).is_none());
        let three = Volume::new([3, 3, 3], [1.0; 3], (0..27).map(|i| i as f32).collect());
        assert!(background(&three, UpstreamDefects::Fix).unwrap().overflow);
    }
}
