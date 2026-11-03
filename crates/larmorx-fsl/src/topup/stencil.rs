// SPDX-License-Identifier: Apache-2.0
//! The Levenberg-Marquardt Hessian as a 3D stencil (`specs/topup.md` §4.7, §11): the cubic
//! basis functions of coefficients more than three knots apart do not overlap, so each row has
//! at most 7 × 7 × 7 entries, stored densely by offset. Its assembly contracts the voxel
//! weights one axis at a time (each axis' basis products once), its products and the
//! preconditioned conjugate-gradient solve run row by row; every sum has a fixed order, so the
//! results do not depend on the thread count.

use rayon::prelude::*;

use super::model::dot;
use super::spline::Op1;

/// A matrix on a grid of coefficients: row `k`, offsets `d ∈ [-3, 3]³` (index
/// `((dz + 3) · 7 + dy + 3) · 7 + dx + 3`).
#[derive(Clone, Debug)]
pub struct Stencil {
    pub dims: [usize; 3],
    pub data: Vec<f64>,
}

pub const CENTRE: usize = 171;

impl Stencil {
    pub fn zeros(dims: [usize; 3]) -> Stencil {
        Stencil {
            dims,
            data: vec![0.0; dims[0] * dims[1] * dims[2] * 343],
        }
    }

    pub fn n(&self) -> usize {
        self.dims[0] * self.dims[1] * self.dims[2]
    }

    pub fn diagonal(&self) -> Vec<f64> {
        (0..self.n()).map(|k| self.data[k * 343 + CENTRE]).collect()
    }

    /// Adds `Σ_x w(x) φp(x; k) φq(x; k + d)` (`φ` products of the per-axis operators `ops_p`,
    /// `ops_q`, rows the voxels of the grid `grid`, columns the coefficients).
    pub fn add_weighted(
        &mut self,
        w: &[f64],
        grid: [usize; 3],
        ops_p: [&Op1; 3],
        ops_q: [&Op1; 3],
    ) {
        let [nx, ny, nz] = grid;
        let [cx, cy, cz] = self.dims;
        // stage x: T1[z][y][kx][dx]
        let mut t1 = vec![0.0; nz * ny * cx * 7];
        t1.par_chunks_mut(ny * cx * 7)
            .enumerate()
            .for_each(|(z, slab)| {
                let (op, oq) = (ops_p[0], ops_q[0]);
                for y in 0..ny {
                    let row = &mut slab[y * cx * 7..(y + 1) * cx * 7];
                    for x in 0..nx {
                        let wv = w[(z * ny + y) * nx + x];
                        if wv == 0.0 {
                            continue;
                        }
                        for s in 0..op.width {
                            let ps = op.w[x * op.width + s];
                            if ps == 0.0 {
                                continue;
                            }
                            let kx = op.idx[x * op.width + s];
                            for t in 0..oq.width {
                                let qt = oq.w[x * oq.width + t];
                                if qt == 0.0 {
                                    continue;
                                }
                                let lx = oq.idx[x * oq.width + t] as isize;
                                let d = lx - kx as isize + 3;
                                if (0..7).contains(&d) {
                                    row[kx * 7 + d as usize] += wv * ps * qt;
                                }
                            }
                        }
                    }
                }
            });
        // stage y: T2[z][ky][dy][kx][dx]
        let blk = cx * 7;
        let mut t2 = vec![0.0; nz * cy * 7 * blk];
        t2.par_chunks_mut(cy * 7 * blk)
            .enumerate()
            .for_each(|(z, slab)| {
                let (op, oq) = (ops_p[1], ops_q[1]);
                for y in 0..ny {
                    let src = &t1[(z * ny + y) * blk..(z * ny + y + 1) * blk];
                    for s in 0..op.width {
                        let ps = op.w[y * op.width + s];
                        if ps == 0.0 {
                            continue;
                        }
                        let ky = op.idx[y * op.width + s];
                        for t in 0..oq.width {
                            let qt = oq.w[y * oq.width + t];
                            if qt == 0.0 {
                                continue;
                            }
                            let d = oq.idx[y * oq.width + t] as isize - ky as isize + 3;
                            if !(0..7).contains(&d) {
                                continue;
                            }
                            let f = ps * qt;
                            let dst = &mut slab
                                [(ky * 7 + d as usize) * blk..(ky * 7 + d as usize + 1) * blk];
                            for (o, v) in dst.iter_mut().zip(src) {
                                *o += f * v;
                            }
                        }
                    }
                }
            });
        // stage z: H[kz][ky][kx][dz][dy][dx]; per kz, its voxels' contributions in z order
        let (op, oq) = (ops_p[2], ops_q[2]);
        let mut per_k: Vec<Vec<(usize, f64, usize, f64)>> = vec![Vec::new(); cz];
        for z in 0..nz {
            for s in 0..op.width {
                let ps = op.w[z * op.width + s];
                if ps == 0.0 {
                    continue;
                }
                let kz = op.idx[z * op.width + s];
                for t in 0..oq.width {
                    let qt = oq.w[z * oq.width + t];
                    if qt == 0.0 {
                        continue;
                    }
                    let d = oq.idx[z * oq.width + t] as isize - kz as isize + 3;
                    if (0..7).contains(&d) {
                        per_k[kz].push((z, ps, d as usize, qt));
                    }
                }
            }
        }
        let zblk = cy * 7 * blk;
        self.data
            .par_chunks_mut(cy * cx * 343)
            .enumerate()
            .for_each(|(kz, slab)| {
                for &(z, ps, dz, qt) in &per_k[kz] {
                    let f = ps * qt;
                    let src = &t2[z * zblk..(z + 1) * zblk];
                    for ky in 0..cy {
                        for dy in 0..7 {
                            let s2 = &src[(ky * 7 + dy) * blk..(ky * 7 + dy + 1) * blk];
                            for kx in 0..cx {
                                let row = &mut slab[(ky * cx + kx) * 343..(ky * cx + kx + 1) * 343];
                                let off = (dz * 7 + dy) * 7;
                                for dx in 0..7 {
                                    row[off + dx] += f * s2[kx * 7 + dx];
                                }
                            }
                        }
                    }
                }
            });
    }

    /// `y = H x`.
    pub fn mul(&self, x: &[f64], y: &mut [f64]) {
        let [cx, cy, cz] = self.dims;
        y.par_chunks_mut(cx * cy).enumerate().for_each(|(kz, out)| {
            for ky in 0..cy {
                for kx in 0..cx {
                    let row = &self.data[((kz * cy + ky) * cx + kx) * 343..][..343];
                    let mut s = 0.0;
                    for dz in 0..7 {
                        let lz = kz as isize + dz as isize - 3;
                        if lz < 0 || lz as usize >= cz {
                            continue;
                        }
                        for dy in 0..7 {
                            let ly = ky as isize + dy as isize - 3;
                            if ly < 0 || ly as usize >= cy {
                                continue;
                            }
                            let base = (lz as usize * cy + ly as usize) * cx;
                            let off = (dz * 7 + dy) * 7;
                            for dx in 0..7 {
                                let lx = kx as isize + dx as isize - 3;
                                if lx < 0 || lx as usize >= cx {
                                    continue;
                                }
                                let v = row[off + dx];
                                if v != 0.0 {
                                    s += v * x[base + lx as usize];
                                }
                            }
                        }
                    }
                    out[ky * cx + kx] = s;
                }
            }
        });
    }

    /// The diagonal multiplied by `1 + damping`.
    pub fn damped(&self, damping: f64) -> Stencil {
        let mut h = self.clone();
        for k in 0..h.n() {
            h.data[k * 343 + CENTRE] *= 1.0 + damping;
        }
        h
    }
}

/// Solves `H x = b` by conjugate gradients preconditioned with the diagonal, from `x = 0`, until
/// `|r| / |b| < tol` or `max_iter` iterations (§4.7).
pub fn pcg(h: &Stencil, b: &[f64], tol: f64, max_iter: usize) -> Vec<f64> {
    let n = b.len();
    let diag = h.diagonal();
    let minv: Vec<f64> = diag
        .iter()
        .map(|&d| if d != 0.0 { 1.0 / d } else { 1.0 })
        .collect();
    let mut x = vec![0.0; n];
    let mut r = b.to_vec();
    let nb = dot(b, b).sqrt();
    if nb == 0.0 {
        return x;
    }
    let mut z: Vec<f64> = r.iter().zip(&minv).map(|(a, m)| a * m).collect();
    let mut p = z.clone();
    let mut rz = dot(&r, &z);
    let mut ap = vec![0.0; n];
    for _ in 0..max_iter {
        h.mul(&p, &mut ap);
        let pap = dot(&p, &ap);
        if pap == 0.0 || !pap.is_finite() {
            break;
        }
        let alpha = rz / pap;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        if dot(&r, &r).sqrt() / nb < tol {
            break;
        }
        for i in 0..n {
            z[i] = r[i] * minv[i];
        }
        let rz2 = dot(&r, &z);
        let beta = rz2 / rz;
        rz = rz2;
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topup::spline::Axis;

    #[test]
    fn assembly_matches_a_direct_sum() {
        let grid = [6usize, 5, 4];
        let axes = [Axis::new(6, 2), Axis::new(5, 1), Axis::new(4, 2)];
        let ops: [Op1; 3] = std::array::from_fn(|a| axes[a].at_voxels(0));
        let opd: [Op1; 3] = std::array::from_fn(|a| axes[a].at_voxels(usize::from(a == 1)));
        let w: Vec<f64> = (0..grid.iter().product::<usize>())
            .map(|i| ((i * 37) % 11) as f64 - 4.0)
            .collect();
        let dims = axes.map(|a| a.ncoef());
        let mut h = Stencil::zeros(dims);
        h.add_weighted(
            &w,
            grid,
            [&ops[0], &ops[1], &ops[2]],
            [&opd[0], &opd[1], &opd[2]],
        );
        // direct: build dense φ matrices
        let nc: usize = dims.iter().product();
        let phi = |o: &[Op1; 3], v: usize, k: usize| -> f64 {
            let p = [
                v % grid[0],
                (v / grid[0]) % grid[1],
                v / (grid[0] * grid[1]),
            ];
            let q = [
                k % dims[0],
                (k / dims[0]) % dims[1],
                k / (dims[0] * dims[1]),
            ];
            (0..3)
                .map(|a| {
                    let op = &o[a];
                    (0..op.width)
                        .filter(|&s| op.idx[p[a] * op.width + s] == q[a])
                        .map(|s| op.w[p[a] * op.width + s])
                        .sum::<f64>()
                })
                .product()
        };
        let x: Vec<f64> = (0..nc).map(|i| (i as f64 * 0.37).cos()).collect();
        let mut y = vec![0.0; nc];
        h.mul(&x, &mut y);
        for k in 0..nc {
            let mut s = 0.0;
            for l in 0..nc {
                let hkl: f64 = (0..w.len())
                    .map(|v| w[v] * phi(&ops, v, k) * phi(&opd, v, l))
                    .sum();
                s += hkl * x[l];
            }
            assert!((s - y[k]).abs() < 1e-9, "row {k}: {s} vs {}", y[k]);
        }
    }
}
