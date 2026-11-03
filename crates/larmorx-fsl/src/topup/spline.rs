// SPDX-License-Identifier: Apache-2.0
//! The field's B-spline basis and the separable one-axis operators topup's estimation is built
//! from (`specs/topup.md` §4.2, §4.4, §4.6).
//!
//! The field is a tensor product of cubic B-splines on a grid of `n` voxels per axis with a knot
//! spacing of `k` voxels. The coefficients' layout (observed, black-box runs of FSL 6.0.7.23:
//! the `_fieldcoef` images and `--fout` fields of the suite's cases, `docs/findings/
//! fsl-topup-original.md`): with `k = 1`, `n` coefficients centred on the voxels; else
//! `ceil((n + 1) / k) + 2` coefficients, coefficient `i` centred at voxel `(i - 1) k`.
//!
//! Every operator here acts along one axis of an array stored x fastest
//! ([`Op1::apply_axis`]); the 3D operators are their products, applied one axis after the
//! other, so the field, its derivatives, the gradient's projections and the refits cost a few
//! passes over the data.

use rayon::prelude::*;

/// The cubic B-spline (support `|t| < 2`).
pub fn b3(t: f64) -> f64 {
    let a = t.abs();
    if a < 1.0 {
        2.0 / 3.0 - a * a + 0.5 * a * a * a
    } else if a < 2.0 {
        let u = 2.0 - a;
        u * u * u / 6.0
    } else {
        0.0
    }
}

/// Its first derivative.
pub fn b3d1(t: f64) -> f64 {
    let a = t.abs();
    let v = if a < 1.0 {
        -2.0 * a + 1.5 * a * a
    } else if a < 2.0 {
        let u = 2.0 - a;
        -0.5 * u * u
    } else {
        0.0
    };
    if t < 0.0 { -v } else { v }
}

/// Its second derivative.
pub fn b3d2(t: f64) -> f64 {
    let a = t.abs();
    if a < 1.0 {
        -2.0 + 3.0 * a
    } else if a < 2.0 {
        2.0 - a
    } else {
        0.0
    }
}

/// The `d`-th derivative of the cubic B-spline (`d` ≤ 2).
pub fn b3n(t: f64, d: usize) -> f64 {
    match d {
        0 => b3(t),
        1 => b3d1(t),
        _ => b3d2(t),
    }
}

/// One axis of a field: `n` voxels, knot spacing `k` voxels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis {
    pub n: usize,
    pub k: usize,
}

impl Axis {
    pub fn new(n: usize, k: usize) -> Axis {
        Axis { n, k: k.max(1) }
    }

    /// The number of coefficients.
    pub fn ncoef(&self) -> usize {
        if self.k == 1 {
            self.n
        } else {
            (self.n + 1).div_ceil(self.k) + 2
        }
    }

    /// The position (voxels) of coefficient 0's centre.
    pub fn offset(&self) -> f64 {
        if self.k == 1 { 0.0 } else { -(self.k as f64) }
    }

    /// The coefficients whose `d`-th derivative is nonzero at `x` (voxels): the first index (may
    /// be negative) and the four values (per voxel^d).
    pub fn weights(&self, x: f64, d: usize) -> (isize, [f64; 4]) {
        let k = self.k as f64;
        let u = (x - self.offset()) / k;
        let i0 = u.floor() as isize - 1;
        let scale = k.powi(d as i32);
        let mut w = [0.0; 4];
        for (j, wj) in w.iter_mut().enumerate() {
            *wj = b3n(u - (i0 + j as isize) as f64, d) / scale;
        }
        (i0, w)
    }

    /// The basis (derivative `d`) at the positions `xs`: one row per position, the
    /// coefficients outside `0..ncoef` left out.
    pub fn rows(&self, xs: &[f64], d: usize) -> Op1 {
        let nc = self.ncoef();
        let mut op = Op1::new(xs.len(), nc, 4);
        for (r, &x) in xs.iter().enumerate() {
            let (i0, w) = self.weights(x, d);
            for j in 0..4 {
                let i = i0 + j as isize;
                if i >= 0 && (i as usize) < nc && w[j] != 0.0 {
                    op.set(r, j, i as usize, w[j]);
                }
            }
        }
        op
    }

    /// The basis (derivative `d`) at the voxels `0..n`.
    pub fn at_voxels(&self, d: usize) -> Op1 {
        let xs: Vec<f64> = (0..self.n).map(|i| i as f64).collect();
        self.rows(&xs, d)
    }
}

/// A linear operator along one axis: `out` rows of at most `width` (index, weight) pairs into an
/// input of length `inp`.
#[derive(Clone, Debug)]
pub struct Op1 {
    pub out: usize,
    pub inp: usize,
    pub width: usize,
    pub idx: Vec<usize>,
    pub w: Vec<f64>,
}

impl Op1 {
    pub fn new(out: usize, inp: usize, width: usize) -> Op1 {
        Op1 {
            out,
            inp,
            width,
            idx: vec![0; out * width],
            w: vec![0.0; out * width],
        }
    }

    pub fn set(&mut self, row: usize, slot: usize, index: usize, weight: f64) {
        self.idx[row * self.width + slot] = index;
        self.w[row * self.width + slot] = weight;
    }

    /// A dense matrix (`rows` of `inp` values).
    pub fn dense(m: &[Vec<f64>], inp: usize) -> Op1 {
        let mut op = Op1::new(m.len(), inp, inp);
        for (r, row) in m.iter().enumerate() {
            for (c, &v) in row.iter().enumerate() {
                op.set(r, c, c, v);
            }
        }
        op
    }

    /// The transpose.
    pub fn transpose(&self) -> Op1 {
        let mut cols: Vec<Vec<(usize, f64)>> = vec![Vec::new(); self.inp];
        for r in 0..self.out {
            for s in 0..self.width {
                let w = self.w[r * self.width + s];
                if w != 0.0 {
                    cols[self.idx[r * self.width + s]].push((r, w));
                }
            }
        }
        let width = cols.iter().map(Vec::len).max().unwrap_or(0).max(1);
        let mut op = Op1::new(self.inp, self.out, width);
        for (c, list) in cols.iter().enumerate() {
            for (s, &(r, w)) in list.iter().enumerate() {
                op.set(c, s, r, w);
            }
        }
        op
    }

    /// Row `r` applied to a strided input.
    #[inline]
    fn row(&self, r: usize, input: &[f64], base: usize, stride: usize) -> f64 {
        let mut s = 0.0;
        let o = r * self.width;
        for j in 0..self.width {
            let w = self.w[o + j];
            if w != 0.0 {
                s += w * input[base + self.idx[o + j] * stride];
            }
        }
        s
    }

    /// The operator applied along `axis` of `data` (dimensions `dims`, x fastest): the result
    /// has `dims[axis] = self.out`. Every output value is one row's sum in a fixed order, so the
    /// result does not depend on the thread count.
    pub fn apply_axis(
        &self,
        data: &[f64],
        dims: [usize; 3],
        axis: usize,
    ) -> (Vec<f64>, [usize; 3]) {
        assert_eq!(dims[axis], self.inp, "operator and axis lengths differ");
        let [nx, ny, nz] = dims;
        let mut od = dims;
        od[axis] = self.out;
        let [ox, oy, _oz] = od;
        let mut out = vec![0.0; od[0] * od[1] * od[2]];
        match axis {
            0 => {
                out.par_chunks_mut(ox).enumerate().for_each(|(line, o)| {
                    let base = line * nx;
                    for (r, v) in o.iter_mut().enumerate() {
                        *v = self.row(r, data, base, 1);
                    }
                });
            }
            1 => {
                out.par_chunks_mut(ox * oy).enumerate().for_each(|(z, o)| {
                    for r in 0..oy {
                        for x in 0..ox {
                            o[r * ox + x] = self.row(r, data, z * nx * ny + x, nx);
                        }
                    }
                });
            }
            _ => {
                out.par_chunks_mut(ox * oy).enumerate().for_each(|(r, o)| {
                    for (i, v) in o.iter_mut().enumerate() {
                        *v = self.row(r, data, i, nx * ny);
                    }
                });
                let _ = nz;
            }
        }
        (out, od)
    }
}

/// `ops[0]` along x, `ops[1]` along y, `ops[2]` along z (each optional: `None` keeps the axis).
pub fn apply3(data: &[f64], dims: [usize; 3], ops: [Option<&Op1>; 3]) -> (Vec<f64>, [usize; 3]) {
    let mut cur = data.to_vec();
    let mut d = dims;
    for (axis, op) in ops.iter().enumerate() {
        if let Some(op) = op {
            let (o, od) = op.apply_axis(&cur, d, axis);
            cur = o;
            d = od;
        }
    }
    (cur, d)
}

/// The 1D refit of a sampled field (§4.4): the coefficients minimising the squared error at
/// the voxels plus `0.005²` times the squares of the two second differences of the
/// coefficients that wrap around the ends (observed), as a dense operator from the `n` samples
/// to the coefficients.
pub fn refit_op(axis: Axis) -> Op1 {
    const LAMBDA: f64 = 0.005;
    let nc = axis.ncoef();
    let b = axis.at_voxels(0);
    // A = BᵀB + λ² DᵀD
    let mut a = vec![vec![0.0; nc]; nc];
    for r in 0..b.out {
        for s in 0..b.width {
            let ws = b.w[r * b.width + s];
            if ws == 0.0 {
                continue;
            }
            for t in 0..b.width {
                let wt = b.w[r * b.width + t];
                if wt != 0.0 {
                    a[b.idx[r * b.width + s]][b.idx[r * b.width + t]] += ws * wt;
                }
            }
        }
    }
    let l2 = LAMBDA * LAMBDA;
    // the two second differences of the coefficients that wrap around the ends,
    // `c[n-1] - 2 c[0] + c[1]` and `c[n-2] - 2 c[n-1] + c[0]` (observed: FSL's refitted fields
    // of probes D to H to single precision, findings; the interior differences take no part)
    let ncs = nc as isize;
    for j in [0isize, ncs - 1] {
        let row = [(j - 1, 1.0), (j, -2.0), (j + 1, 1.0)];
        for &(p, wp) in &row {
            for &(q, wq) in &row {
                a[p.rem_euclid(ncs) as usize][q.rem_euclid(ncs) as usize] += l2 * wp * wq;
            }
        }
    }
    // Bᵀ as columns: the operator is A⁻¹ Bᵀ (nc × n)
    let mut bt = vec![vec![0.0; axis.n]; nc];
    for r in 0..b.out {
        for s in 0..b.width {
            let w = b.w[r * b.width + s];
            if w != 0.0 {
                bt[b.idx[r * b.width + s]][r] += w;
            }
        }
    }
    let m = solve_spd(&a, &bt);
    Op1::dense(&m, axis.n)
}

/// `A⁻¹ B` for a symmetric positive definite `A` (Cholesky), `B` given by rows.
fn solve_spd(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = a.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let mut s = a[i][j];
            for k in 0..j {
                s -= l[i][k] * l[j][k];
            }
            if i == j {
                l[i][i] = s.max(f64::MIN_POSITIVE).sqrt();
            } else {
                l[i][j] = s / l[j][j];
            }
        }
    }
    let m = b.first().map_or(0, Vec::len);
    let mut x = b.to_vec();
    for c in 0..m {
        for i in 0..n {
            let mut s = x[i][c];
            for k in 0..i {
                s -= l[i][k] * x[k][c];
            }
            x[i][c] = s / l[i][i];
        }
        for i in (0..n).rev() {
            let mut s = x[i][c];
            for k in i + 1..n {
                s -= l[k][i] * x[k][c];
            }
            x[i][c] = s / l[i][i];
        }
    }
    x
}

/// The 1D Gram matrix of the basis' `d`-th derivatives (per mm: `voxel` mm per voxel) summed at
/// the integer voxel positions covering every coefficient's support (§4.6; observed: the
/// regularisation `-v` prints is this sum, not the integral), banded (`|i - j| ≤ 3`).
pub fn gram(axis: Axis, d: usize, voxel: f64) -> Vec<[f64; 7]> {
    let nc = axis.ncoef();
    let k = axis.k as f64;
    let lo = (axis.offset() - 2.0 * k).floor() as isize;
    let hi = (axis.offset() + (nc as f64 - 1.0) * k + 2.0 * k).ceil() as isize;
    let scale = voxel.powi(-(2 * d as i32));
    let mut g = vec![[0.0; 7]; nc];
    for x in lo..=hi {
        let (i0, w) = axis.weights(x as f64, d);
        for p in 0..4 {
            let i = i0 + p as isize;
            if i < 0 || i as usize >= nc || w[p] == 0.0 {
                continue;
            }
            for q in 0..4 {
                let j = i0 + q as isize;
                if j < 0 || j as usize >= nc || w[q] == 0.0 {
                    continue;
                }
                g[i as usize][(j - i + 3) as usize] += w[p] * w[q] * scale;
            }
        }
    }
    g
}

/// A banded Gram matrix as an operator.
pub fn band_op(g: &[[f64; 7]]) -> Op1 {
    let n = g.len();
    let mut op = Op1::new(n, n, 7);
    for (i, row) in g.iter().enumerate() {
        for (s, &v) in row.iter().enumerate() {
            let j = i as isize + s as isize - 3;
            if j >= 0 && (j as usize) < n && v != 0.0 {
                op.set(i, s, j as usize, v);
            }
        }
    }
    op
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_of_unity_inside() {
        for &(n, k) in &[(32usize, 2usize), (64, 5), (40, 4), (20, 1), (33, 3)] {
            let a = Axis::new(n, k);
            let op = a.at_voxels(0);
            for r in 0..n {
                let s: f64 = (0..op.width).map(|j| op.w[r * op.width + j]).sum();
                // FSL's layout leaves the last coefficient out of the last voxels' support for
                // some sizes (64 voxels, spacing 5: observed, its `_fieldcoef` has 15)
                let (i0, _) = a.weights(r as f64, 0);
                if i0 >= 0 && (i0 + 3) < a.ncoef() as isize {
                    assert!((s - 1.0).abs() < 1e-12, "n {n} k {k} row {r}: {s}");
                }
            }
        }
    }

    #[test]
    fn coefficient_counts() {
        assert_eq!(Axis::new(32, 2).ncoef(), 19);
        assert_eq!(Axis::new(20, 2).ncoef(), 13);
        assert_eq!(Axis::new(64, 5).ncoef(), 15);
        assert_eq!(Axis::new(40, 4).ncoef(), 13);
        assert_eq!(Axis::new(64, 1).ncoef(), 64);
    }

    #[test]
    fn refit_reproduces_a_field_of_its_space() {
        let a = Axis::new(30, 3);
        let c: Vec<f64> = (0..a.ncoef()).map(|i| (i as f64 * 0.7).sin()).collect();
        let f = a.at_voxels(0).apply_axis(&c, [a.ncoef(), 1, 1], 0).0;
        let c2 = refit_op(a).apply_axis(&f, [30, 1, 1], 0).0;
        let f2 = a.at_voxels(0).apply_axis(&c2, [a.ncoef(), 1, 1], 0).0;
        // the wrapped differences move the weakly sampled end coefficients: the ends' voxels
        // change, the interior keeps the field
        for (x, y) in f.iter().zip(&f2).take(24).skip(6) {
            assert!((x - y).abs() < 1e-3, "{x} {y}");
        }
        // with knot spacing 1 every coefficient is well sampled: the refit keeps every voxel
        let a = Axis::new(32, 1);
        let c: Vec<f64> = (0..32).map(|i| ((i % 8) as f64 - 3.5).abs()).collect();
        let f = a.at_voxels(0).apply_axis(&c, [32, 1, 1], 0).0;
        let c2 = refit_op(a).apply_axis(&f, [32, 1, 1], 0).0;
        let f2 = a.at_voxels(0).apply_axis(&c2, [32, 1, 1], 0).0;
        for (x, y) in f.iter().zip(&f2) {
            assert!((x - y).abs() < 1e-3, "{x} {y}");
        }
    }
}
