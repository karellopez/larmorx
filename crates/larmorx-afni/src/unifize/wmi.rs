// SPDX-License-Identifier: Apache-2.0
//! The local white-matter intensity (`specs/3dUnifize.md` §4.3): at every voxel, the mean of
//! the non-zero values in a ball around it between two percentiles.
//!
//! The spec's arithmetic decides the bits only through the values between the two ranks and
//! the order of their float sum (ascending), so the method may find them any way. A
//! straightforward method gathers and sorts every ball (tens of thousands of values per voxel
//! at the default radius); this one keeps the ball's values as counts per *rank* (the position
//! of each distinct non-zero value of the image in their sorted list) and slides the ball from
//! voxel to voxel, adding the values that enter and removing those that leave (the ball's two
//! surfaces, a few percent of it). Each voxel then needs the two order statistics, found
//! through two levels of block counts, and the sum of the values between them, read in
//! ascending order from the counts (a bit set per block skips the ranks absent from the ball).
//! The ball visits a plane row by row, alternating direction, so it is built from scratch only
//! once per plane; planes run in parallel. Every voxel's result is the spec's, bit for bit,
//! whatever the order of the work.

use rayon::prelude::*;

/// The ball of radius `r` (spec §4.3): every offset `(di, dj, dk)` with
/// `di² + dj² + dk² <= r * r`, the sums and the square of `r` in float.
#[derive(Clone, Debug)]
pub struct Ball {
    /// The half-width per axis, `trunc(r)`.
    pub half: i64,
    rr: f32,
}

impl Ball {
    pub fn new(r: f32) -> Ball {
        Ball {
            half: r.trunc() as i64,
            rr: r * r,
        }
    }

    /// Whether the offset is in the ball.
    #[inline]
    pub fn contains(&self, a: i64, b: i64, c: i64) -> bool {
        ((a * a) as f32 + (b * b) as f32) + (c * c) as f32 <= self.rr
    }

    /// The largest `d >= 0` with `(d, a, b)` in the ball, if any (the extent of the line of
    /// the ball through `(a, b)` along the third axis).
    pub fn extent(&self, a: i64, b: i64) -> Option<i64> {
        self.extent_within(a, b, self.half)
    }

    /// [`Ball::extent`], at most `cap` (the ball is tested in float, and its sums grow with `d`,
    /// so the extent is the first `d` whose next is outside).
    pub fn extent_within(&self, a: i64, b: i64, cap: i64) -> Option<i64> {
        if !self.contains(0, a, b) {
            return None;
        }
        let cap = cap.min(self.half);
        let mut d = 0;
        while d < cap && self.contains(d + 1, a, b) {
            d += 1;
        }
        Some(d)
    }

    /// Every offset of the ball.
    pub fn offsets(&self) -> Vec<[i64; 3]> {
        let h = self.half;
        let mut out = Vec::new();
        for c in -h..=h {
            for b in -h..=h {
                for a in -h..=h {
                    if self.contains(a, b, c) {
                        out.push([a, b, c]);
                    }
                }
            }
        }
        out
    }
}

/// The spec's mean of the sorted values `q1 ..= q2` of `n` values given in ascending order by
/// `next` (spec §4.3, step 2), for `n >= 2`.
#[inline]
pub fn ranks(n: usize, bottom: f32, top: f32) -> (usize, usize) {
    let m = (n - 1) as f32;
    (
        (0.01f32 * bottom * m) as usize,
        (0.01f32 * top * m) as usize,
    )
}

/// The values of the image ranked: the distinct non-zero values in ascending order, and each
/// voxel's rank (`NONE` for 0).
struct Ranked {
    values: Vec<f32>,
    rank: Vec<u32>,
}

const NONE: u32 = u32::MAX;

fn rank_values(a: &[f32]) -> Ranked {
    let mut values: Vec<f32> = a.par_iter().copied().filter(|&x| x != 0.0).collect();
    values.par_sort_unstable_by(f32::total_cmp);
    // Equal values (as floats: -0 and +0 never occur, both being 0) share a rank.
    values.dedup_by(|x, y| x == y);
    assert!(values.len() < NONE as usize, "too many distinct values");
    let rank = a
        .par_iter()
        .map(|&x| {
            if x == 0.0 {
                NONE
            } else {
                values.partition_point(|&v| v < x) as u32
            }
        })
        .collect();
    Ranked { values, rank }
}

/// The multiset of the ball's values, as counts per rank with two levels of block counts and a
/// bit set of the ranks present per block of 64.
struct Window {
    count: Vec<u32>,
    /// Ranks present, 64 per word.
    bits: Vec<u64>,
    /// Values per block of 64 ranks.
    block: Vec<u32>,
    /// Values per block of 4096 ranks.
    superblock: Vec<u32>,
    n: usize,
}

impl Window {
    fn new(nranks: usize) -> Window {
        Window {
            count: vec![0; nranks],
            bits: vec![0; nranks.div_ceil(64)],
            block: vec![0; nranks.div_ceil(64)],
            superblock: vec![0; nranks.div_ceil(4096)],
            n: 0,
        }
    }

    #[inline]
    fn add(&mut self, r: u32) {
        if r == NONE {
            return;
        }
        let r = r as usize;
        if self.count[r] == 0 {
            self.bits[r >> 6] |= 1 << (r & 63);
        }
        self.count[r] += 1;
        self.block[r >> 6] += 1;
        self.superblock[r >> 12] += 1;
        self.n += 1;
    }

    #[inline]
    fn remove(&mut self, r: u32) {
        if r == NONE {
            return;
        }
        let r = r as usize;
        self.count[r] -= 1;
        if self.count[r] == 0 {
            self.bits[r >> 6] &= !(1 << (r & 63));
        }
        self.block[r >> 6] -= 1;
        self.superblock[r >> 12] -= 1;
        self.n -= 1;
    }

    /// The rank of the `k`-th smallest value (0-based) and how many values of that rank come
    /// before it.
    fn select(&self, mut k: usize) -> (usize, usize) {
        let mut s = 0;
        while self.superblock[s] as usize <= k {
            k -= self.superblock[s] as usize;
            s += 1;
        }
        let mut b = s * 64;
        while self.block[b] as usize <= k {
            k -= self.block[b] as usize;
            b += 1;
        }
        let mut word = self.bits[b];
        loop {
            let r = b * 64 + word.trailing_zeros() as usize;
            let c = self.count[r] as usize;
            if c > k {
                return (r, k);
            }
            k -= c;
            word &= word - 1;
        }
    }

    /// The mean of the sorted values `q1 ..= q2` (spec §4.3): their float sum in ascending
    /// order, divided by their number in float.
    fn mean(&self, values: &[f32], q1: usize, q2: usize) -> f32 {
        let (mut r, skip) = self.select(q1);
        let mut left = q2 - q1 + 1;
        let mut s = 0.0f32;
        // The values of rank `r` from the `skip`-th on, then the next ranks present.
        let mut c = self.count[r] as usize - skip;
        loop {
            let v = values[r];
            let take = c.min(left);
            for _ in 0..take {
                s += v;
            }
            left -= take;
            if left == 0 {
                break;
            }
            // The next rank present.
            let mut b = r >> 6;
            let mut word = self.bits[b] & (!0u64).checked_shl((r & 63) as u32 + 1).unwrap_or(0);
            while word == 0 {
                b += 1;
                if self.block[b] == 0 {
                    // Skip empty blocks quickly (a block count is 0 exactly when its bits are).
                    while self.block[b] == 0 {
                        b += 1;
                    }
                }
                word = self.bits[b];
            }
            r = b * 64 + word.trailing_zeros() as usize;
            c = self.count[r] as usize;
        }
        s / ((q2 - q1) as f32 + 1.0)
    }

    fn value(&self, values: &[f32], bottom: f32, top: f32) -> f32 {
        match self.n {
            0 => 0.0,
            1 => {
                let (r, _) = self.select(0);
                values[r]
            }
            n => {
                let (q1, q2) = ranks(n, bottom, top);
                self.mean(values, q1, q2)
            }
        }
    }
}

/// The local white-matter intensity of `a` on a grid of `dims` (spec §4.3) for the ball of
/// radius `radius` and the percentiles `bottom` and `top`, and, for the voxels whose storage
/// index is a multiple of 66,666, whether their ball holds at least two values (AFNI's
/// progress dots). Runs on the current thread pool; the result does not depend on it.
pub fn local_intensity(
    a: &[f32],
    dims: [usize; 3],
    radius: f32,
    bottom: f32,
    top: f32,
) -> (Vec<f32>, usize) {
    let [nx, ny, nz] = dims;
    let ball = Ball::new(radius);
    let h = ball.half;
    // The lines of the ball along x (indexed by dj, dk) and along y (by di, dk), limited to the
    // offsets that can reach the grid: at most n - 1 voxels along an axis of n
    // (`specs/3dUnifize.md` §9.1). An offset beyond them never lands inside the grid, and a line
    // at least as long as its axis covers it from any voxel, so every ball holds the same values;
    // a radius of thousands of voxels (`-EPI` on tiny voxels, AFNI-75 reproduced) needs only the
    // grid's lines.
    let reach = |n: usize| (n as i64 - 1).max(0);
    let lines = |across: i64, along: i64| -> Vec<(i64, i64, i64)> {
        let mut out = Vec::new();
        let (hc, hb) = (h.min(reach(nz)), h.min(across));
        for c in -hc..=hc {
            for b in -hb..=hb {
                if let Some(e) = ball.extent_within(b, c, along) {
                    out.push((b, c, e));
                }
            }
        }
        out
    };
    let x_lines = lines(reach(ny), reach(nx));
    let y_lines = lines(reach(nx), reach(ny));
    let ranked = rank_values(a);
    let nranks = ranked.values.len().max(1);
    let plane = nx * ny;
    let (inx, iny, inz) = (nx as i64, ny as i64, nz as i64);
    let rank_at = |x: i64, y: i64, z: i64| -> u32 {
        if x < 0 || y < 0 || z < 0 || x >= inx || y >= iny || z >= inz {
            NONE
        } else {
            ranked.rank[x as usize + nx * (y as usize + ny * z as usize)]
        }
    };
    let mut out = vec![0f32; a.len()];
    let dots: usize = out
        .par_chunks_mut(plane.max(1))
        .enumerate()
        .map_init(
            || Window::new(nranks),
            |w, (k, slab)| {
                let k = k as i64;
                // The ball at (0, 0, k), from scratch.
                for (b, c, e) in &x_lines {
                    for a in -e..=*e {
                        w.add(rank_at(a, *b, k + c));
                    }
                }
                let mut dots = 0;
                let (mut i, mut j) = (0i64, 0i64);
                let mut forward = true;
                loop {
                    let p = i as usize + nx * j as usize;
                    slab[p] = w.value(&ranked.values, bottom, top);
                    if (k as usize * plane + p).is_multiple_of(66_666) && w.n >= 2 {
                        dots += 1;
                    }
                    // The next voxel: along x, or a step in y at the row's end.
                    let at_end = if forward { i + 1 == inx } else { i == 0 };
                    if at_end {
                        if j + 1 == iny {
                            break;
                        }
                        for (a, c, e) in &y_lines {
                            w.remove(rank_at(i + a, j - e, k + c));
                            w.add(rank_at(i + a, j + 1 + e, k + c));
                        }
                        j += 1;
                        forward = !forward;
                    } else if forward {
                        for (b, c, e) in &x_lines {
                            w.remove(rank_at(i - e, j + b, k + c));
                            w.add(rank_at(i + 1 + e, j + b, k + c));
                        }
                        i += 1;
                    } else {
                        for (b, c, e) in &x_lines {
                            w.remove(rank_at(i + e, j + b, k + c));
                            w.add(rank_at(i - 1 - e, j + b, k + c));
                        }
                        i -= 1;
                    }
                }
                // Empty the window for the next plane of this thread.
                for (b, c, e) in &x_lines {
                    for a in -e..=*e {
                        w.remove(rank_at(i + a, j + b, k + c));
                    }
                }
                debug_assert_eq!(w.n, 0);
                dots
            },
        )
        .sum();
    (out, dots)
}

/// The spec's method word for word, for tests: every ball gathered and sorted.
#[cfg(test)]
pub(crate) fn local_intensity_by_hand(
    a: &[f32],
    dims: [usize; 3],
    radius: f32,
    bottom: f32,
    top: f32,
) -> Vec<f32> {
    let [nx, ny, nz] = dims;
    let ball = Ball::new(radius);
    let offsets = ball.offsets();
    let mut out = vec![0f32; a.len()];
    for k in 0..nz as i64 {
        for j in 0..ny as i64 {
            for i in 0..nx as i64 {
                let mut v: Vec<f32> = offsets
                    .iter()
                    .filter_map(|o| {
                        let (x, y, z) = (i + o[0], j + o[1], k + o[2]);
                        if x < 0
                            || y < 0
                            || z < 0
                            || x >= nx as i64
                            || y >= ny as i64
                            || z >= nz as i64
                        {
                            return None;
                        }
                        let val = a[x as usize + nx * (y as usize + ny * z as usize)];
                        (val != 0.0).then_some(val)
                    })
                    .collect();
                v.sort_by(f32::total_cmp);
                let p = i as usize + nx * (j as usize + ny * k as usize);
                out[p] = match v.len() {
                    0 => 0.0,
                    1 => v[0],
                    n => {
                        let (q1, q2) = ranks(n, bottom, top);
                        let mut s = 0.0f32;
                        for x in &v[q1..=q2] {
                            s += x;
                        }
                        s / ((q2 - q1) as f32 + 1.0)
                    }
                };
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(dims: [usize; 3], seed: u64, zeros: u64, levels: u64) -> Vec<f32> {
        let mut s = seed | 1;
        (0..dims.iter().product::<usize>())
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                if s % 100 < zeros {
                    0.0
                } else {
                    // Repeated values (ties) and a few negatives.
                    ((s >> 8) % levels) as f32 * 0.37 - 3.0
                }
            })
            .collect()
    }

    #[test]
    fn the_ball() {
        let b = Ball::new(1.0);
        assert_eq!(b.offsets().len(), 7);
        let b = Ball::new(1.5);
        assert_eq!(b.offsets().len(), 19);
        let b = Ball::new(18.3);
        // The lines along x cover the ball.
        let mut n = 0;
        for c in -18..=18 {
            for a in -18..=18 {
                if let Some(e) = b.extent(a, c) {
                    n += 2 * e + 1;
                }
            }
        }
        assert_eq!(n as usize, b.offsets().len());
    }

    #[test]
    fn against_the_spec_by_hand() {
        for (t, (dims, r, b, top)) in [
            ([9, 7, 5], 2.3f32, 70.0f32, 80.0f32),
            ([12, 5, 6], 3.5, 65.0, 90.0),
            ([6, 9, 1], 4.1, 30.0, 31.0),
            ([1, 1, 8], 2.0, 70.0, 80.0),
            ([5, 4, 3], 9.0, 80.0, 90.0),
        ]
        .into_iter()
        .enumerate()
        {
            for zeros in [0, 40, 95] {
                for levels in [3, 1000] {
                    let a = image(dims, 9 + t as u64 * 31 + zeros + levels, zeros, levels);
                    let (fast, _) = local_intensity(&a, dims, r, b, top);
                    let slow = local_intensity_by_hand(&a, dims, r, b, top);
                    let same = fast
                        .iter()
                        .zip(&slow)
                        .all(|(x, y)| x.to_bits() == y.to_bits());
                    assert!(same, "{dims:?} r {r} zeros {zeros} levels {levels}");
                }
            }
        }
    }

    #[test]
    fn the_same_at_any_thread_count() {
        let dims = [11, 10, 9];
        let a = image(dims, 5, 30, 500);
        let one =
            larmorx_core::parallel::with_threads(1, || local_intensity(&a, dims, 3.2, 70.0, 80.0))
                .unwrap();
        let four =
            larmorx_core::parallel::with_threads(4, || local_intensity(&a, dims, 3.2, 70.0, 80.0))
                .unwrap();
        assert_eq!(one, four);
    }
}
