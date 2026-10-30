// SPDX-License-Identifier: Apache-2.0
//! The robust intensity range and BET's initial parameters (`specs/bet.md` §4.2, §4.3).

use rayon::prelude::*;

use super::Vol;

/// The robust range `(t2, t98)`: the 2nd and 98th percentiles from histograms of 1000 bins,
/// narrowed while a few voxels dominate the range (§4.2).
pub fn robust_range(v: &Vol) -> (f64, f64) {
    let (lo0, hi0) = min_max(&v.data);
    if lo0 == hi0 || v.data.is_empty() {
        return (f64::from(lo0), f64::from(hi0));
    }
    let n = v.data.len();
    let target = n / 50;
    let (mut lo, mut hi) = (lo0, hi0);
    let (mut t2, mut t98) = (f64::from(lo0), f64::from(hi0));
    for pass in 0..10 {
        let last = pass == 9;
        if last {
            (lo, hi) = (lo0, hi0);
        }
        let (flo, fhi) = (f64::from(lo), f64::from(hi));
        let width = fhi - flo;
        let hist = histogram(&v.data, flo, width);
        let (first, end) = if last { (1usize, 998usize) } else { (0, 999) };
        let mut count = 0usize;
        let mut b = first;
        while b < 1000 {
            count += hist[b];
            if count >= target {
                break;
            }
            b += 1;
        }
        let b = b.min(999);
        count = 0;
        let mut u = end as isize;
        while u >= 0 {
            count += hist[u as usize];
            if count >= target {
                break;
            }
            u -= 1;
        }
        let u = u.max(0) as usize;
        let at = |k: usize| f64::from(lo + (k as f64 * width / 1000.0) as f32);
        t2 = at(b);
        t98 = at(u + 1);
        if t98 - t2 >= width / 10.0 || last {
            break;
        }
        let (nlo, nhi) = (at(b.saturating_sub(1)), at((u + 1).min(999) + 1));
        lo = nlo as f32;
        hi = nhi as f32;
        if lo >= hi {
            break;
        }
    }
    (t2, t98)
}

fn min_max(d: &[f32]) -> (f32, f32) {
    d.par_iter()
        .with_min_len(1 << 16)
        .fold(
            || (f32::INFINITY, f32::NEG_INFINITY),
            |(lo, hi), &x| (if x < lo { x } else { lo }, if x > hi { x } else { hi }),
        )
        .reduce(
            || (f32::INFINITY, f32::NEG_INFINITY),
            |a, b| (a.0.min(b.0), a.1.max(b.1)),
        )
}

/// The image's minimum and maximum (NaN ignored).
pub fn range(v: &Vol) -> (f32, f32) {
    min_max(&v.data)
}

fn histogram(d: &[f32], lo: f64, width: f64) -> Vec<usize> {
    let a = 1000.0 / width;
    let b = 1000.0 * (-lo) / width;
    d.par_chunks(1 << 16)
        .map(|c| {
            let mut h = vec![0usize; 1000];
            for &x in c {
                let k = (f64::from(x) * a + b) as i64;
                h[k.clamp(0, 999) as usize] += 1;
            }
            h
        })
        .reduce(
            || vec![0usize; 1000],
            |mut x, y| {
                for (a, b) in x.iter_mut().zip(y) {
                    *a += b;
                }
                x
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
}

/// The initial parameters: thresholds, radius (unless given), centre of gravity (unless
/// given, in mm) and the median within-brain intensity.
pub fn initial(v: &Vol, radius: Option<f64>, centre: Option<[f64; 3]>) -> Initial {
    let (min, max) = range(v);
    let (t2, t98) = robust_range(v);
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
            let mut r = larmorx_core::math::pow(
                0.75 * count * dx * dy * dz / std::f64::consts::PI,
                1.0 / 3.0,
            );
            let mut slabs = 0;
            let dims = [(nx, dx, dy * dz), (ny, dy, dx * dz), (nz, dz, dx * dy)];
            for (n, d, area) in dims {
                if (n as f64) * d < r {
                    r = (count * area / (std::f64::consts::PI * n as f64)).sqrt();
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
    }
}

fn centre_of_gravity(v: &Vol, t2: f64, t98: f64, t: f64) -> [f64; 3] {
    let [nx, ny, _] = v.shape;
    let [dx, dy, dz] = v.voxel;
    let slice = nx * ny;
    let cap = t98 - t2;
    let above = t - t2;
    // per slice, then in order: the same sums at any thread count
    let parts: Vec<[f64; 4]> = v
        .data
        .par_chunks(slice.max(1))
        .enumerate()
        .map(|(k, s)| {
            let mut acc = [0.0f64; 4];
            for j in 0..ny {
                for i in 0..nx {
                    let x = f64::from(s[j * nx + i]) - t2;
                    if x > above {
                        let w = x.min(cap);
                        acc[0] += w * (i as f64 * dx);
                        acc[1] += w * (j as f64 * dy);
                        acc[2] += w * (k as f64 * dz);
                        acc[3] += w;
                    }
                }
            }
            acc
        })
        .collect();
    let mut s = [0.0f64; 4];
    for p in parts {
        for k in 0..4 {
            s[k] += p[k];
        }
    }
    if s[3] > 0.0 {
        [s[0] / s[3], s[1] / s[3], s[2] / s[3]]
    } else {
        [0.0; 3]
    }
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
        assert!((t2 - 19.98).abs() < 0.01, "{t2}");
        assert!((t98 - 979.02).abs() < 0.01, "{t98}");
    }
}
