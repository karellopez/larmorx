// SPDX-License-Identifier: Apache-2.0
//! The robust intensity range and BET's initial parameters (`specs/bet.md` §4.2, §4.3).

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::math::Libm;
use rayon::prelude::*;

use super::Vol;

/// A robust range and FSL-187's part in it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Robust {
    pub t2: f64,
    pub t98: f64,
    /// FSL's range where FSL-187 moved it: with the correction (the default) only, when a value
    /// far above a pass's range (its bin beyond `int`'s range) went into the last bin and FSL's
    /// first bin gives other limits.
    pub fsl: Option<(f64, f64)>,
}

/// The robust range `(t2, t98)`: the 2nd and 98th percentiles from histograms of 1000 bins,
/// narrowed while a few voxels dominate the range (§4.2), corrected for FSL-187.
pub fn robust_range(v: &Vol) -> (f64, f64) {
    let r = robust_range_in(v, UpstreamDefects::Fix);
    (r.t2, r.t98)
}

/// [`robust_range`] in a mode of `upstream_defects`: FSL-187 (a value whose bin number does not
/// fit in an `int`, more than about two billion bin widths above a pass's range, counted in the
/// first bin, as x86-64's conversion gives `INT_MIN`) is corrected by default (the last bin)
/// and reproduced with `reproduce`; with the correction FSL's range is computed beside it only
/// where such a value occurred.
pub fn robust_range_in(v: &Vol, mode: UpstreamDefects) -> Robust {
    let (t2, t98, far) = robust_pass(v, mode.fix());
    let fsl = (mode.fix() && far)
        .then(|| {
            let (a, b, _) = robust_pass(v, false);
            (a, b)
        })
        .filter(|&r| r != (t2, t98));
    Robust { t2, t98, fsl }
}

fn robust_pass(v: &Vol, last_bin: bool) -> (f64, f64, bool) {
    let (lo0, hi0) = min_max(&v.data);
    if lo0 == hi0 || v.data.is_empty() {
        return (f64::from(lo0), f64::from(hi0), false);
    }
    let mut far = false;
    let total = v.data.len();
    let (mut lo, mut hi) = (lo0, hi0);
    let (mut t2, mut t98) = (lo0, hi0);
    // a narrowed range of no width makes the next pass use the full range as the tenth does
    let mut full = false;
    for pass in 0..10 {
        let last = pass == 9;
        if last || full {
            (lo, hi) = (lo0, hi0);
        }
        let ends_out = last || full;
        full = false;
        // §4.2's exact arithmetic: w = hi − lo in float; the bin `trunc(A x + B)` with
        // A = 1000/w, B = (1000·(−lo))/w in double
        let w = f64::from(hi - lo);
        let (hist, f) = histogram(&v.data, f64::from(lo), w, last_bin);
        far |= f;
        let (first, end) = if ends_out {
            (1usize, 998usize)
        } else {
            (0, 999)
        };
        // the voxels in the histogram (the end bins' counts left out with them)
        let n = if ends_out {
            total - hist[0] - hist[999]
        } else {
            total
        };
        let target = n / 50;
        // bins are added while the count is below n / 50: none when that is 0 (observed: a
        // two-voxel image's t2 is one bin below its minimum, its t98 one above its maximum)
        let mut count = 0usize;
        let mut b = first as i64 - 1;
        while count < target && b < end as i64 {
            b += 1;
            count += hist[b as usize];
        }
        count = 0;
        let mut u = end as i64 + 1;
        while count < target && u > first as i64 {
            u -= 1;
            count += hist[u as usize];
        }
        // the bin width δ = w/1000 in double; t2 = lo + float(b·δ), the sum in float
        let delta = w / 1000.0;
        let at = |k: i64| lo + (k as f64 * delta) as f32;
        t2 = at(b);
        t98 = at(u + 1);
        if last || !(f64::from(t98 - t2) < w / 10.0) {
            break;
        }
        // the narrowed range, a formula of its own: float(lo + (b'/1000)·w)
        let b1 = (b - 1).max(0) as f64;
        let u1 = ((u + 1).min(999) + 1) as f64;
        let nlo = (f64::from(lo) + (b1 / 1000.0) * w) as f32;
        let nhi = (f64::from(lo) + (u1 / 1000.0) * w) as f32;
        lo = nlo;
        hi = nhi;
        if lo == hi {
            full = true;
        }
    }
    (f64::from(t2), f64::from(t98), far)
}

/// The minimum and maximum as a scan from the first voxel finds them (a strictly smaller or
/// larger value replaces the one held): of equal values the first is kept, so a zero's sign is
/// the first zero's (observed, 2026-11-05: `bet -S -v`'s premasked image, whose first voxel is
/// +0 and later ones -0, prints `min 0` in FSL). The chunks are combined in order, so the result
/// does not depend on the thread count.
fn min_max(d: &[f32]) -> (f32, f32) {
    d.par_iter()
        .with_min_len(1 << 16)
        .fold(
            || (f32::INFINITY, f32::NEG_INFINITY),
            |(lo, hi), &x| (if x < lo { x } else { lo }, if x > hi { x } else { hi }),
        )
        .reduce(
            || (f32::INFINITY, f32::NEG_INFINITY),
            |a, b| {
                (
                    if b.0 < a.0 { b.0 } else { a.0 },
                    if b.1 > a.1 { b.1 } else { a.1 },
                )
            },
        )
}

/// The image's minimum and maximum (NaN ignored).
pub fn range(v: &Vol) -> (f32, f32) {
    min_max(&v.data)
}

/// The 1000-bin histogram: bin `trunc(A x + B)` clamped to [0, 999]; a bin at or beyond 2^31
/// (FSL-187: `int`'s range, where FSL's bin is x86-64's `INT_MIN`, so its first) in the last
/// bin when `last_bin`, else in the first; and whether such a value occurred.
fn histogram(d: &[f32], lo: f64, width: f64, last_bin: bool) -> (Vec<usize>, bool) {
    let a = 1000.0 / width;
    let b = 1000.0 * (-lo) / width;
    d.par_chunks(1 << 16)
        .map(|c| {
            let mut h = vec![0usize; 1000];
            let mut far = false;
            for &x in c {
                let y = f64::from(x) * a + b;
                let k = if y >= 2_147_483_648.0 {
                    far = true;
                    if last_bin { 999 } else { 0 }
                } else {
                    (y as i64).clamp(0, 999)
                };
                h[k as usize] += 1;
            }
            (h, far)
        })
        .reduce(
            || (vec![0usize; 1000], false),
            |(mut x, fx), (y, fy)| {
                for (a, b) in x.iter_mut().zip(y) {
                    *a += b;
                }
                (x, fx || fy)
            },
        )
}

/// BET's starting point (§4.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Initial {
    pub min: f32,
    pub max: f32,
    pub t2: f64,
    pub t98: f64,
    pub t: f64,
    pub radius: f64,
    pub centre: [f64; 3],
    pub median: f64,
    /// Whether the radius was taken from a slab along two axes ("Image appears to be 1D").
    pub one_d: bool,
    /// FSL's robust range where FSL-187 moved it (with the correction only; [`Robust::fsl`]).
    pub fsl_187: Option<(f64, f64)>,
}

/// The initial parameters: thresholds, radius (unless given), centre of gravity (unless
/// given, in mm) and the median within-brain intensity, the robust range in `mode` (FSL-187).
pub fn initial(
    v: &Vol,
    radius: Option<f64>,
    centre: Option<[f64; 3]>,
    mode: UpstreamDefects,
    libm: &Libm,
) -> Initial {
    let (min, max) = range(v);
    let Robust { t2, t98, fsl } = robust_range_in(v, mode);
    let t = t2 + 0.1 * (t98 - t2);
    let [nx, ny, nz] = v.shape;
    let [dx, dy, dz] = v.voxel;
    let mut one_d = false;
    let r = match radius {
        Some(r) => r,
        None => {
            let count = v
                .data
                .par_iter()
                .with_min_len(1 << 16)
                .filter(|&&x| f64::from(x) > t)
                .count() as f64;
            let mut r = libm.pow(
                (((0.75 * count) * dx) * dy) * dz / std::f64::consts::PI,
                1.0 / 3.0,
            );
            let mut slabs = 0;
            // §8.1: the field of view and the area times N in float, the divisions in double
            let n32 = count as f32;
            let (fx, fy, fz) = (dx as f32, dy as f32, dz as f32);
            let dims = [
                (nx, fx, (n32 * fy) * fz),
                (ny, fy, (n32 * fx) * fz),
                (nz, fz, (n32 * fx) * fy),
            ];
            for (n, d, area) in dims {
                if f64::from(n as f32 * d) < r {
                    r = libm.pow((f64::from(area) / std::f64::consts::PI) / n as f64, 0.5);
                    slabs += 1;
                }
            }
            one_d = slabs >= 2;
            r
        }
    };
    let c = match centre {
        Some(c) => c,
        None => centre_of_gravity(v, t2, t98, t),
    };
    let median = median(v, t2, t98, r, c);
    Initial {
        min,
        max,
        t2,
        t98,
        t,
        radius: r,
        centre: c,
        median,
        one_d,
        fsl_187: fsl,
    }
}

fn centre_of_gravity(v: &Vol, t2: f64, t98: f64, t: f64) -> [f64; 3] {
    let [nx, ny, _] = v.shape;
    let [dx, dy, dz] = v.voxel;
    let slice = nx * ny;
    let cap = t98 - t2;
    let above = t - t2;
    // one running sum over the whole volume in storage order (§8.1: a per-slice partial sum
    // gives other bits); each voxel adds w and ((w·i)·dx, (w·j)·dy, (w·k)·dz)
    let mut s = [0.0f64; 4];
    for (k, sl) in v.data.chunks(slice.max(1)).enumerate() {
        let kf = k as f64;
        for j in 0..ny {
            let jf = j as f64;
            for i in 0..nx {
                let e = f64::from(sl[j * nx + i]) - t2;
                if e > above {
                    let w = e.min(cap);
                    s[3] += w;
                    s[0] += (w * i as f64) * dx;
                    s[1] += (w * jf) * dy;
                    s[2] += (w * kf) * dz;
                }
            }
        }
    }
    // no voxel above the threshold: 0 / 0, not a number (observed: `c-of-g -nan -nan -nan`)
    [s[0] / s[3], s[1] / s[3], s[2] / s[3]]
}

fn median(v: &Vol, t2: f64, t98: f64, r: f64, c: [f64; 3]) -> f64 {
    let [nx, ny, _] = v.shape;
    let [dx, dy, dz] = v.voxel;
    let r2 = r * r;
    let slice = nx * ny;
    let mut values: Vec<f32> = v
        .data
        .par_chunks(slice.max(1))
        .enumerate()
        .flat_map_iter(|(k, s)| {
            let z = k as f64 * dz - c[2];
            (0..ny).flat_map(move |j| {
                let y = j as f64 * dy - c[1];
                (0..nx).filter_map(move |i| {
                    let x = i as f64 * dx - c[0];
                    let val = s[j * nx + i];
                    let f = f64::from(val);
                    (f > t2 && f < t98 && x * x + y * y + z * z < r2).then_some(val)
                })
            })
        })
        .collect();
    if values.is_empty() {
        return t98;
    }
    // the largest of the floor(n/2) smallest values (observed)
    let k = (values.len() / 2).saturating_sub(1);
    let (_, m, _) = values.select_nth_unstable_by(k, f32::total_cmp);
    f64::from(*m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_and_ramp() {
        let v = Vol::new([4, 4, 4], [1.0; 3], vec![3.0; 64]);
        assert_eq!(robust_range(&v), (3.0, 3.0));
        let data: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        let v = Vol::new([10, 10, 10], [1.0; 3], data);
        let (t2, t98) = robust_range(&v);
        // the bin holding the 20th value from each end (n / 50 = 20), bins 0.999 wide
        assert!((t2 - 18.981).abs() < 1e-4, "{t2}");
        assert!((t98 - 980.019).abs() < 1e-3, "{t98}");
    }

    /// FSL-187 (the register's smallest trigger: 99 values on [0, 1) and one at 10^12): the
    /// default counts the far value in the last bin and gives FSL's range beside it, which
    /// `reproduce` gives; without the far value nothing is named.
    #[test]
    fn fsl_187_far_value() {
        let mut data: Vec<f32> = (0..99).map(|i| i as f32 / 99.0).collect();
        let near = Vol::new([11, 9, 1], [1.0; 3], data.clone());
        let fix = robust_range_in(&near, UpstreamDefects::Fix);
        assert_eq!(fix.fsl, None);
        assert_eq!(fix, robust_range_in(&near, UpstreamDefects::Reproduce));
        data.push(1e12);
        let far = Vol::new([10, 10, 1], [1.0; 3], data);
        let fix = robust_range_in(&far, UpstreamDefects::Fix);
        assert_eq!((fix.t2, fix.t98), robust_range(&far));
        let fsl = fix.fsl.expect("FSL-187 moves this range");
        assert_ne!(fsl, (fix.t2, fix.t98));
        let rep = robust_range_in(&far, UpstreamDefects::Reproduce);
        assert_eq!(((rep.t2, rep.t98), rep.fsl), (fsl, None));
    }
}
