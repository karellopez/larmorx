// SPDX-License-Identifier: Apache-2.0
//! The half-size image and back (`specs/3dUnifize.md` §4.2 and §4.4).

use rayon::prelude::*;

/// The half grid of `dims`: `max(1, n / 2)` per axis.
pub fn half_dims(dims: [usize; 3]) -> [usize; 3] {
    dims.map(|n| (n / 2).max(1))
}

/// The median of 7 values by the spec's sorting network (§4.2): compare-and-swap steps,
/// `swap(p, q)` when `p > q`; the result is the 4th value.
#[inline]
pub fn median7(mut v: [f32; 7]) -> f32 {
    const NET: [(usize, usize); 14] = [
        (0, 1),
        (4, 5),
        (1, 2),
        (5, 6),
        (0, 1),
        (4, 5),
        (0, 4),
        (2, 6),
        (1, 3),
        (3, 5),
        (1, 3),
        (2, 3),
        (3, 4),
        (2, 3),
    ];
    for (p, q) in NET {
        if v[p] > v[q] {
            v.swap(p, q);
        }
    }
    v[3]
}

/// The half-size image of `a` (§4.2): each half voxel the median of 7 values of `a`, at
/// `(2i, 2j, 2k)` and its 6 face neighbours (indices clamped), in the order centre, x−, x+,
/// y−, y+, z−, z+.
pub fn down(a: &[f32], dims: [usize; 3]) -> (Vec<f32>, [usize; 3]) {
    let [nx, ny, nz] = dims;
    let h = half_dims(dims);
    let [hx, hy, _] = h;
    let at = |x: usize, y: usize, z: usize| a[x + nx * (y + ny * z)];
    let mut out = vec![0f32; h.iter().product()];
    out.par_chunks_mut(hx * hy)
        .enumerate()
        .for_each(|(k, slab)| {
            for j in 0..hy {
                for i in 0..hx {
                    let (x, y, z) = (2 * i, 2 * j, 2 * k);
                    let (x, y, z) = (x.min(nx - 1), y.min(ny - 1), z.min(nz - 1));
                    let v = [
                        at(x, y, z),
                        at(x.saturating_sub(1), y, z),
                        at((x + 1).min(nx - 1), y, z),
                        at(x, y.saturating_sub(1), z),
                        at(x, (y + 1).min(ny - 1), z),
                        at(x, y, z.saturating_sub(1)),
                        at(x, y, (z + 1).min(nz - 1)),
                    ];
                    slab[i + hx * j] = median7(v);
                }
            }
        });
    (out, h)
}

/// The full-size grid AFNI expands a half grid `half` back to, for an input of `dims`
/// (§4.4): an axis of 1 stays 1; otherwise `2m + 1` when the input's size is odd, else `2m`.
pub fn up_dims(half: [usize; 3], dims: [usize; 3]) -> [usize; 3] {
    [0, 1, 2].map(|a| {
        let m = half[a];
        if m == 1 {
            1
        } else if dims[a] % 2 == 1 {
            2 * m + 1
        } else {
            2 * m
        }
    })
}

/// The half image `h` (of `half`) expanded to a grid of `out` (§4.4): per axis `lo = i / 2`
/// and `hi = lo + 1` for odd `i` (both clamped to `m - 1`), the value `0.125f` times the float
/// sum of the 8 corners in the spec's order.
pub fn up(h: &[f32], half: [usize; 3], out: [usize; 3]) -> Vec<f32> {
    let [mx, my, mz] = half;
    let [ox, oy, _] = out;
    let idx = |i: usize, m: usize| {
        let lo = (i / 2).min(m - 1);
        let hi = if i % 2 == 1 { (lo + 1).min(m - 1) } else { lo };
        (lo, hi)
    };
    let at = |x: usize, y: usize, z: usize| h[x + mx * (y + my * z)];
    let mut v = vec![0f32; out.iter().product()];
    v.par_chunks_mut(ox * oy).enumerate().for_each(|(k, slab)| {
        let (z0, z1) = idx(k, mz);
        for j in 0..oy {
            let (y0, y1) = idx(j, my);
            for i in 0..ox {
                let (x0, x1) = idx(i, mx);
                let mut s = at(x0, y0, z0);
                s += at(x1, y0, z0);
                s += at(x0, y1, z0);
                s += at(x1, y1, z0);
                s += at(x0, y0, z1);
                s += at(x1, y0, z1);
                s += at(x0, y1, z1);
                s += at(x1, y1, z1);
                slab[i + ox * j] = 0.125f32 * s;
            }
        }
    });
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_network_gives_the_median() {
        let mut s = 99u64;
        for _ in 0..2000 {
            let mut v = [0f32; 7];
            for x in &mut v {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                *x = (s % 9) as f32 - 4.0;
            }
            let mut sorted = v;
            sorted.sort_by(f32::total_cmp);
            assert_eq!(median7(v), sorted[3], "{v:?}");
        }
        // The sign of a zero: which value comes out decides it.
        assert!(median7([0.0, -0.0, 0.0, -0.0, 0.0, -0.0, 0.0]).is_sign_negative());
    }

    #[test]
    fn sizes() {
        assert_eq!(half_dims([256, 255, 3]), [128, 127, 1]);
        assert_eq!(up_dims([128, 127, 1], [256, 255, 3]), [256, 255, 1]);
        assert_eq!(up_dims([128, 127, 1], [256, 255, 1]), [256, 255, 1]);
        assert_eq!(up_dims([2, 2, 2], [5, 4, 4]), [5, 4, 4]);
    }

    #[test]
    fn up_of_a_constant_is_the_constant() {
        let h = vec![3.0f32; 2 * 3 * 4];
        let v = up(&h, [2, 3, 4], [5, 6, 8]);
        assert!(v.iter().all(|&x| x == 3.0));
    }
}
