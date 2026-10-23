// SPDX-License-Identifier: Apache-2.0
//! The clip level (`specs/3dAutomask.md` §4) and the gradual clip level (§4.1).
//!
//! The clip level of an image is found on a histogram of 10001 bins of its positive values:
//! a first cut from the top 65 % of the values, then repeated halving of the values above the
//! cut until the cut stops moving. Every float and double step is the spec's.
//!
//! Speed: the histogram's counts and the sum of the squared bin indices are integers, so they
//! are accumulated in parallel over z slabs and added in any order with the same result; the
//! sum of squares is exact in an integer and equal to the spec's double sum as long as every
//! partial sum stays below 2^53 (fewer than about 90 million positive values; beyond that the
//! double sum is formed in storage order, as the spec states it). The eight octants of the
//! gradual clip level are independent and are computed in parallel.

use rayon::prelude::*;

/// The number of the histogram's last bin (the maximum's).
pub const TOP_BIN: usize = 10000;

/// A box of voxels: `lo[a] ..= hi[a]` along each axis (inclusive), in a grid of `dims`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub lo: [usize; 3],
    pub hi: [usize; 3],
}

impl Region {
    /// The whole grid.
    pub fn all(dims: [usize; 3]) -> Region {
        Region {
            lo: [0; 3],
            hi: [
                dims[0].saturating_sub(1),
                dims[1].saturating_sub(1),
                dims[2].saturating_sub(1),
            ],
        }
    }
}

/// The voxels of `v` in `region`, row by row (x fastest), as slices.
fn rows<'a>(
    v: &'a [f32],
    dims: [usize; 3],
    region: Region,
) -> impl Iterator<Item = (usize, &'a [f32])> + 'a {
    let [nx, ny, _] = dims;
    let Region { lo, hi } = region;
    (lo[2]..=hi[2]).flat_map(move |k| {
        (lo[1]..=hi[1]).map(move |j| {
            let base = nx * (j + ny * k);
            (k, &v[base + lo[0]..=base + hi[0]])
        })
    })
}

/// The largest value of `region` as a double: `m = -1e38`, replaced by each value `x` with
/// `m < x` (spec §4, step 1).
fn top_of(v: &[f32], dims: [usize; 3], region: Region) -> f64 {
    let mut m = -1e38f64;
    for (_, row) in rows(v, dims, region) {
        for &x in row {
            let x = f64::from(x);
            if m < x {
                m = x;
            }
        }
    }
    m
}

/// The histogram of the positive values (spec §4, step 2): counts per bin, their number and
/// the sum of the squared bin indices (exact).
struct Histogram {
    h: Vec<u64>,
    npos: u64,
    sum_k2: u128,
}

impl Histogram {
    fn new() -> Histogram {
        Histogram {
            h: vec![0; TOP_BIN + 1],
            npos: 0,
            sum_k2: 0,
        }
    }

    fn merge(mut self, other: Histogram) -> Histogram {
        for (a, b) in self.h.iter_mut().zip(&other.h) {
            *a += b;
        }
        self.npos += other.npos;
        self.sum_k2 += other.sum_k2;
        self
    }

    /// `h[i]`, 0 outside the bins (an index below 0 can only arise for huge images).
    fn at(&self, i: i64) -> u64 {
        if (0..=TOP_BIN as i64).contains(&i) {
            self.h[i as usize]
        } else {
            0
        }
    }
}

/// The bin of a positive value `x`: `trunc(s * x + 0.499)` in double.
#[inline]
fn bin(s: f64, x: f32) -> i64 {
    (s * f64::from(x) + 0.499) as i64
}

fn histogram(v: &[f32], dims: [usize; 3], region: Region, s: f64) -> Histogram {
    let fill = |k: usize| {
        let mut h = Histogram::new();
        let slab = Region {
            lo: [region.lo[0], region.lo[1], k],
            hi: [region.hi[0], region.hi[1], k],
        };
        for (_, row) in rows(v, dims, slab) {
            for &x in row {
                if x > 0.0 {
                    let k = bin(s, x);
                    if k <= TOP_BIN as i64 {
                        // k >= 0: s and x are positive.
                        h.h[k as usize] += 1;
                        h.npos += 1;
                        h.sum_k2 += (k as u128) * (k as u128);
                    }
                }
            }
        }
        h
    };
    (region.lo[2]..=region.hi[2])
        .into_par_iter()
        .map(fill)
        .reduce(Histogram::new, Histogram::merge)
}

/// The spec's double sum of `k * k`, in storage order: needed only when the integer sum
/// exceeds what a double holds exactly.
fn sequential_d(v: &[f32], dims: [usize; 3], region: Region, s: f64) -> f64 {
    let mut d = 0.0f64;
    for (_, row) in rows(v, dims, region) {
        for &x in row {
            if x > 0.0 {
                let k = bin(s, x);
                if k <= TOP_BIN as i64 {
                    d += (k * k) as f64;
                }
            }
        }
    }
    d
}

/// The clip level of the values of `v` in `region` for the fraction `frac` (spec §4). A
/// fraction outside `(0, 0.99)` is taken as 0.5.
pub fn clip_level_region(v: &[f32], dims: [usize; 3], region: Region, frac: f32) -> f32 {
    let frac = if frac > 0.0 && frac < 0.99 { frac } else { 0.5 };
    let top = top_of(v, dims, region);
    if top < 1e-100 {
        return 0.0;
    }
    let s = 10000.0 / top;
    let hist = histogram(v, dims, region, s);
    let npos = hist.npos;
    if npos <= 222 {
        return 0.0;
    }
    let d = if hist.sum_k2 < (1u128 << 53) {
        hist.sum_k2 as f64
    } else {
        sequential_d(v, dims, region, s)
    };
    let q = (0.65f32 * npos as f32) as i64;
    let b = (0.5 * (d / npos as f64).sqrt()).round_ties_even() as i64;
    let mut i = TOP_BIN as i64 - 1;
    let mut kk: i64 = 0;
    while i >= b && kk < q {
        kk += hist.at(i) as i64;
        i -= 1;
    }
    let mut cut = i;
    for _ in 0..66 {
        let n: u64 = (cut..TOP_BIN as i64).map(|k| hist.at(k)).sum();
        let half = (n / 2) as i64;
        let mut i = cut;
        let mut kk: i64 = 0;
        while i < TOP_BIN as i64 && kk < half {
            kk += hist.at(i) as i64;
            i += 1;
        }
        let new_cut = (frac * i as f32) as i64;
        if new_cut == cut {
            break;
        }
        cut = new_cut;
    }
    (cut as f64 / s).min(1e38) as f32
}

/// The clip level of the whole image (spec §4).
pub fn clip_level(v: &[f32], dims: [usize; 3], frac: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    clip_level_region(v, dims, Region::all(dims), frac)
}

/// The gradual clip level (spec §4.1): a level per voxel, interpolated between the clip
/// levels of the image's eight octants around its centre of mass.
#[derive(Clone, Debug, PartialEq)]
pub struct GradualLevel {
    /// The octants' levels, `c[x half + 2 * y half + 4 * z half]` (0 lower, 1 upper).
    pub c: [f32; 8],
    /// Per axis: the octant centres' `x0` and the reciprocal of their distance `dx` (spec
    /// step 4).
    pub origin: [f32; 3],
    pub step: [f32; 3],
}

/// `rint` of a float to an integer, half to even.
fn rint_f32(x: f32) -> i64 {
    x.round_ties_even() as i64
}

impl GradualLevel {
    /// The gradual clip level of `v` (spec §4.1) for the fraction `frac`.
    pub fn new(v: &[f32], dims: [usize; 3], frac: f32) -> GradualLevel {
        let [nx, ny, nz] = dims;
        // 1. The centre of mass of |v|, sums in double in storage order.
        let (mut s0, mut si, mut sj, mut sk) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for k in 0..nz {
            for j in 0..ny {
                let row = &v[nx * (j + ny * k)..nx * (j + ny * k) + nx];
                for (i, &x) in row.iter().enumerate() {
                    let a = f64::from(x.abs());
                    s0 += a;
                    si += a * i as f64;
                    sj += a * j as f64;
                    sk += a * k as f64;
                }
            }
        }
        let centre = |sum: f64, n: usize| -> f32 {
            if s0 == 0.0 {
                ((n as f64 - 1.0) / 2.0) as f32
            } else {
                (sum / s0) as f32
            }
        };
        let ic = [
            rint_f32(centre(si, nx)),
            rint_f32(centre(sj, ny)),
            rint_f32(centre(sk, nz)),
        ];
        // 2. The floor of every octant's level.
        let g = 0.333f32 * clip_level(v, dims, frac);
        // 3. The octants' boxes, overlapping by o voxels.
        let mut halves = [[(0usize, 0usize); 2]; 3];
        for a in 0..3 {
            let n = dims[a] as i64;
            let it = n - 1;
            let o = ((0.01 * n as f64).round_ties_even() as i64).max(1);
            let clamp = |x: i64| x.clamp(0, it.max(0)) as usize;
            halves[a] = [(0, clamp(ic[a] + o)), (clamp(ic[a] - o), clamp(it))];
        }
        let c: Vec<f32> = (0..8usize)
            .into_par_iter()
            .map(|oct| {
                let pick = |a: usize| halves[a][(oct >> a) & 1];
                let (x, y, z) = (pick(0), pick(1), pick(2));
                let region = Region {
                    lo: [x.0, y.0, z.0],
                    hi: [x.1, y.1, z.1],
                };
                let level = if x.0 <= x.1 && y.0 <= y.1 && z.0 <= z.1 {
                    clip_level_region(v, dims, region, frac)
                } else {
                    0.0
                };
                if level < g { g } else { level }
            })
            .collect();
        // 4. The octants' centres along each axis.
        let mut origin = [0f32; 3];
        let mut step = [0f32; 3];
        for a in 0..3 {
            let it = dims[a] as f64 - 1.0;
            let x0 = (0.5 * ic[a] as f64) as f32;
            let x1 = (0.5 * (ic[a] as f64 + it)) as f32;
            origin[a] = x0;
            step[a] = if x1 > x0 {
                (1.0 / f64::from(x1 - x0)) as f32
            } else {
                0.0
            };
        }
        GradualLevel {
            c: c.try_into().expect("eight octants"),
            origin,
            step,
        }
    }

    /// The interpolation weight along axis `a` at index `i` (spec §4.1, step 5): clamped to
    /// `[0, 1]`.
    #[inline]
    pub fn weight(&self, a: usize, i: usize) -> f32 {
        let u = (i as f32 - self.origin[a]) * self.step[a];
        u.clamp(0.0, 1.0)
    }

    /// The level at the voxel with weights `u`, `v`, `w` (spec §4.1, step 5): the float sum of
    /// the eight products, each evaluated left to right.
    #[inline]
    pub fn at_weights(&self, u: f32, v: f32, w: f32) -> f32 {
        let c = &self.c;
        let (iu, iv, iw) = (1.0 - u, 1.0 - v, 1.0 - w);
        let mut s = c[0] * iu * iv * iw;
        s += c[1] * u * iv * iw;
        s += c[2] * iu * v * iw;
        s += c[3] * u * v * iw;
        s += c[4] * iu * iv * w;
        s += c[5] * u * iv * w;
        s += c[6] * iu * v * w;
        s += c[7] * u * v * w;
        s
    }

    /// The level at voxel `(i, j, k)`.
    pub fn at(&self, i: usize, j: usize, k: usize) -> f32 {
        self.at_weights(self.weight(0, i), self.weight(1, j), self.weight(2, k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec §4 (observed): 1000 voxels of 100 and 1000 of 10 give a level of 5.
    #[test]
    fn two_groups() {
        let mut v = vec![100.0f32; 1000];
        v.extend(std::iter::repeat_n(10.0f32, 1000));
        assert_eq!(clip_level(&v, [2000, 1, 1], 0.5), 5.0);
        // The same values in another layout: the level does not depend on it.
        assert_eq!(clip_level(&v, [20, 10, 10], 0.5), 5.0);
    }

    #[test]
    fn small_and_empty_images() {
        // 222 positive values or fewer: 0.
        let v = vec![5.0f32; 222];
        assert_eq!(clip_level(&v, [222, 1, 1], 0.5), 0.0);
        assert_eq!(clip_level(&[0.0; 1000], [10, 10, 10], 0.5), 0.0);
        assert_eq!(clip_level(&[], [0, 0, 0], 0.5), 0.0);
        // Negative values only: the top is negative, so the level is 0.
        assert_eq!(clip_level(&[-3.0; 1000], [10, 10, 10], 0.5), 0.0);
    }

    /// A fraction outside (0, 0.99) is 0.5.
    #[test]
    fn fraction_outside_its_range() {
        let v: Vec<f32> = (0..4000).map(|i| (i % 997) as f32 + 1.0).collect();
        let half = clip_level(&v, [4000, 1, 1], 0.5);
        assert_eq!(clip_level(&v, [4000, 1, 1], 0.0), half);
        assert_eq!(clip_level(&v, [4000, 1, 1], 0.995), half);
        assert_ne!(clip_level(&v, [4000, 1, 1], 0.3), half);
    }

    /// The level computed step by step, as the spec states it, against the function.
    #[test]
    fn against_the_spec_by_hand() {
        // A brute-force transcription of spec §4 over a plain slice.
        fn by_hand(v: &[f32], frac: f32) -> f32 {
            let mut m = -1e38f64;
            for &x in v {
                if m < f64::from(x) {
                    m = f64::from(x);
                }
            }
            if m < 1e-100 {
                return 0.0;
            }
            let s = 10000.0 / m;
            let mut h = vec![0i64; 10001];
            let (mut npos, mut d) = (0i64, 0.0f64);
            for &x in v {
                if x > 0.0 {
                    let k = (s * f64::from(x) + 0.499) as i64;
                    if k <= 10000 {
                        h[k as usize] += 1;
                        npos += 1;
                        d += (k * k) as f64;
                    }
                }
            }
            if npos <= 222 {
                return 0.0;
            }
            let q = (0.65f32 * npos as f32) as i64;
            let b = (0.5 * (d / npos as f64).sqrt()).round_ties_even() as i64;
            let (mut i, mut kk) = (9999i64, 0i64);
            while i >= b && kk < q {
                kk += h[i as usize];
                i -= 1;
            }
            let mut cut = i;
            for _ in 0..66 {
                let n: i64 = (cut.max(0)..10000).map(|k| h[k as usize]).sum();
                let half = n / 2;
                let (mut i, mut kk) = (cut, 0i64);
                while i < 10000 && kk < half {
                    kk += if i >= 0 { h[i as usize] } else { 0 };
                    i += 1;
                }
                let nc = (frac * i as f32) as i64;
                if nc == cut {
                    break;
                }
                cut = nc;
            }
            (cut as f64 / s).min(1e38) as f32
        }
        // Seeded values with a heavy tail and many zeros.
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for trial in 0..20 {
            let n = 500 + (next() % 5000) as usize;
            let v: Vec<f32> = (0..n)
                .map(|_| {
                    let r = next() % 1000;
                    if r < 300 {
                        0.0
                    } else {
                        (next() % 100_000) as f32 / (1.0 + (r % 7) as f32)
                    }
                })
                .collect();
            for frac in [0.1f32, 0.2, 0.5, 0.9] {
                assert_eq!(
                    clip_level(&v, [n, 1, 1], frac),
                    by_hand(&v, frac),
                    "trial {trial}, frac {frac}"
                );
            }
        }
    }

    #[test]
    fn gradual_weights_and_sum_order() {
        let dims = [8, 6, 4];
        let v: Vec<f32> = (0..8 * 6 * 4).map(|i| (i % 13) as f32 * 10.0).collect();
        let g = GradualLevel::new(&v, dims, 0.5);
        for a in 0..3 {
            for i in 0..dims[a] {
                let w = g.weight(a, i);
                assert!((0.0..=1.0).contains(&w));
            }
        }
        // At the lower corner (weights 0) the level is c000 exactly.
        assert_eq!(g.at_weights(0.0, 0.0, 0.0), g.c[0]);
        assert_eq!(g.at_weights(1.0, 1.0, 1.0), g.c[7]);
        assert_eq!(g.at_weights(1.0, 0.0, 0.0), g.c[1]);
        assert_eq!(g.at_weights(0.0, 1.0, 0.0), g.c[2]);
        assert_eq!(g.at_weights(0.0, 0.0, 1.0), g.c[4]);
    }
}
