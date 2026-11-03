// SPDX-License-Identifier: Apache-2.0
//! applytopup's undocumented variational-Bayes restorations (`--method=vb2D`, `vb3D` and `vb4D`,
//! `specs/topup.md` §7.5, §10: FSL-141 to FSL-143), a clean-room original written from the spec
//! and black-box runs of FSL's program (`docs/findings/fsl-topup-original.md`, findings 23 to
//! 27).
//!
//! As `lsr` ([`super::apply::lsr`]): the images moved onto the first's space, the collections by
//! the phase encoding's axis, each line's resampling matrices `K_a`; the restored image `f` of a
//! collection has the Gaussian likelihood `φ Σ_a |y_a - K_a f|²` and the smoothness prior `λ fᵀ Q
//! f`, `φ` and `λ` with Gamma priors (shape `1e-10`, scale `1e10`), inferred by mean-field
//! variational Bayes with one Gaussian factor per line. Observed (black-box runs, findings 23 to
//! 27):
//! - `Q = DᵀD`, `D = Σ_d w_d Δ_d` the sum of the second differences along the slice's axes
//!   (`vb2D`) or the volume's (`vb3D`, `vb4D`), weighted `w_d = v_pe / v_d` (the phase
//!   encoding's voxel size over the axis'); the stencil truncated at the image's edges, periodic
//!   along the phase encoding; FSL-143: FSL's coupling between `(±1 along the phase encoding, ±1
//!   slice)` is `w_pe w_z`, half of `DᵀD`'s `2 w_pe w_z`;
//! - blocks: `vb2D` each slice of each volume (50 iterations), `vb3D` each volume (20), `vb4D`
//!   every volume together (20; no coupling between volumes);
//! - the first estimate per line `(Σ_a K_aᵀK_a + P)⁻¹ Σ_a K_aᵀ y_a`, `P` the circulant `(1, -4, 6,
//!   -4, 1)`, with `φ = λ = 1`; then each iteration: `φ = (a + N_φ / 2) / (1/b + (Σ |y_a - K_a
//!   f|² + Σ_l tr(K_lᵀK_l Σ_l)) / 2)`, `λ = (a + N_λ / 2) / (1/b + (fᵀQf + Σ_l tr(Q_ll Σ_l)) /
//!   2)` (integer halves), then every line in storage order (Gauss-Seidel): `Σ_l = (φ K_lᵀK_l +
//!   λ Q_ll)⁻¹`, `f_l = Σ_l (φ K_lᵀ y_l - λ Σ_{m ≠ l} Q_lm f_m)`;
//! - `N_λ` the block's voxels, `N_φ` its data points: every image's (`vb2D`); FSL-142: two per
//!   voxel whatever the collection holds (`vb3D`, `vb4D`); FSL-141: both in 32-bit integers;
//! - no line is skipped: the invalid voxels' moved values (interpolated periodically) enter the
//!   restoration; the collections added, then every invalid voxel 0 (the mask voxel by voxel,
//!   not `lsr`'s whole lines).

use rayon::prelude::*;

use super::apply::{
    Corrected, FieldOnGrid, Image, LsrError, Settings, collections, line_starts, moved_images,
    resampling_matrix,
};
use crate::cli::fmt;

/// The method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VbMethod {
    Vb2D,
    Vb3D,
    Vb4D,
}

impl VbMethod {
    pub fn parse(name: &str) -> Option<VbMethod> {
        match name {
            "vb2D" => Some(VbMethod::Vb2D),
            "vb3D" => Some(VbMethod::Vb3D),
            "vb4D" => Some(VbMethod::Vb4D),
            _ => None,
        }
    }

    fn iterations(self) -> usize {
        if self == VbMethod::Vb2D { 50 } else { 20 }
    }
}

/// The settings: the method and FSL's defects reproduced.
#[derive(Clone, Copy, Debug)]
pub struct VbOptions {
    pub method: VbMethod,
    /// FSL-141: the counts in 32-bit integers.
    pub counts_32: bool,
    /// FSL-142: `vb3D` and `vb4D` count two data points per voxel.
    pub count_two: bool,
    /// FSL-143: the coupling `(±1 along the phase encoding, ±1 slice)` halved.
    pub half_coupling: bool,
}

/// The restoration, its standard output and which defects changed it.
#[derive(Clone, Debug)]
pub struct VbResult {
    pub corrected: Corrected,
    pub stdout: String,
    /// FSL-141 changed a count (64-bit and 32-bit counts differ).
    pub counts_wrap: bool,
    /// FSL-142 changed a count (a collection of other than two images, vb3D or vb4D).
    pub count_differs: bool,
}

/// Why a restoration cannot run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VbError {
    Lsr(LsrError),
    /// A line's matrix `φ KᵀK + λ Q_ll` is singular.
    Singular,
}

const A0: f64 = 1e-10;
const B0: f64 = 1e10;

/// The prior's stencil in a collection's frame: offsets along the phase encoding `-2..=2` for
/// each neighbouring line `(offset along the other in-plane axis, offset in slices)`.
#[derive(Clone, Debug)]
struct Stencil {
    /// The line's own block (circulant along the line).
    own: [f64; 5],
    /// The other lines' blocks.
    cross: Vec<(isize, isize, [f64; 5])>,
}

impl Stencil {
    /// `DᵀD` for `D = Σ_d w_d Δ_d` over the axes `axes` (each `(axis in the collection's frame:
    /// 0 phase encoding, 1 the other in-plane axis, 2 slices; weight)`), `half` the FSL-143
    /// coupling.
    fn new(axes: &[(usize, f64)], half: bool) -> Stencil {
        let mut d: Vec<([isize; 3], f64)> = Vec::new();
        let add = |o: [isize; 3], v: f64, d: &mut Vec<([isize; 3], f64)>| match d
            .iter_mut()
            .find(|e| e.0 == o)
        {
            Some(e) => e.1 += v,
            None => d.push((o, v)),
        };
        for &(a, w) in axes {
            for (s, v) in [(-1isize, 1.0), (0, -2.0), (1, 1.0)] {
                let mut o = [0isize; 3];
                o[a] = s;
                add(o, w * v, &mut d);
            }
        }
        let mut st: Vec<([isize; 3], f64)> = Vec::new();
        for &(o1, v1) in &d {
            for &(o2, v2) in &d {
                let o = [o2[0] - o1[0], o2[1] - o1[1], o2[2] - o1[2]];
                add(o, v1 * v2, &mut st);
            }
        }
        if half {
            for e in &mut st {
                if e.0[0].abs() == 1 && e.0[1] == 0 && e.0[2].abs() == 1 {
                    e.1 *= 0.5;
                }
            }
        }
        let mut own = [0.0; 5];
        let mut cross: Vec<(isize, isize, [f64; 5])> = Vec::new();
        for (o, v) in st {
            if v == 0.0 {
                continue;
            }
            let k = (o[0] + 2) as usize;
            if o[1] == 0 && o[2] == 0 {
                own[k] += v;
            } else {
                match cross.iter_mut().find(|c| c.0 == o[1] && c.1 == o[2]) {
                    Some(c) => c.2[k] += v,
                    None => {
                        let mut b = [0.0; 5];
                        b[k] = v;
                        cross.push((o[1], o[2], b));
                    }
                }
            }
        }
        cross.sort_by_key(|c| (c.1, c.0));
        Stencil { own, cross }
    }
}

/// The circulant `n × n` matrix (row-major) of the offsets `-2..=2` with `c`, entries that wrap
/// onto each other added.
fn circulant(c: &[f64; 5], n: usize) -> Vec<f64> {
    let mut m = vec![0.0; n * n];
    let ni = n as isize;
    for i in 0..ni {
        for (k, &v) in c.iter().enumerate() {
            if v != 0.0 {
                let j = (i + k as isize - 2).rem_euclid(ni);
                m[i as usize * n + j as usize] += v;
            }
        }
    }
    m
}

/// `out += scale · (c ⊛ f)` along a periodic line: `out[j] += scale Σ_k c[k] f[j + k - 2]`.
#[inline]
fn add_circ(out: &mut [f64], c: &[f64; 5], f: &[f64], scale: f64) {
    let n = f.len() as isize;
    for (j, o) in out.iter_mut().enumerate() {
        let mut s = 0.0;
        for (k, &v) in c.iter().enumerate() {
            if v != 0.0 {
                s += v * f[(j as isize + k as isize - 2).rem_euclid(n) as usize];
            }
        }
        *o += scale * s;
    }
}

/// A sparse resampling matrix: each row's nonzero `(column, value)`.
struct SparseK {
    rows: Vec<Vec<(usize, f64)>>,
}

impl SparseK {
    fn of_dense(k: &[f64], n: usize) -> SparseK {
        SparseK {
            rows: (0..n)
                .map(|i| {
                    (0..n)
                        .filter(|&j| k[i * n + j] != 0.0)
                        .map(|j| (j, k[i * n + j]))
                        .collect()
                })
                .collect(),
        }
    }

    /// `out += Kᵀ y`.
    fn add_transpose_times(&self, y: &[f64], out: &mut [f64]) {
        for (row, &yi) in self.rows.iter().zip(y) {
            if yi != 0.0 {
                for &(j, v) in row {
                    out[j] += v * yi;
                }
            }
        }
    }

    /// `|y - K f|²`.
    fn residual(&self, y: &[f64], f: &[f64]) -> f64 {
        self.rows
            .iter()
            .zip(y)
            .map(|(row, &yi)| {
                let p: f64 = row.iter().map(|&(j, v)| v * f[j]).sum();
                (yi - p) * (yi - p)
            })
            .sum()
    }
}

/// `KᵀK` of a line's images, row-major `n × n`.
fn gram(ks: &[SparseK], n: usize) -> Vec<f64> {
    let mut g = vec![0.0; n * n];
    for k in ks {
        for row in &k.rows {
            for &(i, a) in row {
                for &(j, b) in row {
                    g[i * n + j] += a * b;
                }
            }
        }
    }
    g
}

/// The inverse of the symmetric `a` (`n × n`, row-major): by its Cholesky factor when it is
/// positive definite, else by Gauss-Jordan elimination with partial pivoting; `None` when it is
/// singular.
fn inverse(a: &[f64], n: usize) -> Option<Vec<f64>> {
    if let Some(l) = super::apply::cholesky(a, n) {
        // L⁻¹ (lower), then Σ = L⁻ᵀ L⁻¹
        let mut li = vec![0.0; n * n];
        for j in 0..n {
            li[j * n + j] = 1.0 / l[j * n + j];
            for i in j + 1..n {
                let mut s = 0.0;
                for k in j..i {
                    s += l[i * n + k] * li[k * n + j];
                }
                li[i * n + j] = -s / l[i * n + i];
            }
        }
        let mut inv = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..=i {
                let mut s = 0.0;
                for k in i..n {
                    s += li[k * n + i] * li[k * n + j];
                }
                inv[i * n + j] = s;
                inv[j * n + i] = s;
            }
        }
        return Some(inv);
    }
    let mut m = a.to_vec();
    let mut inv = vec![0.0; n * n];
    for i in 0..n {
        inv[i * n + i] = 1.0;
    }
    for c in 0..n {
        let p = (c..n).max_by(|&x, &y| m[x * n + c].abs().total_cmp(&m[y * n + c].abs()))?;
        let pv = m[p * n + c];
        if pv == 0.0 || !pv.is_finite() {
            return None;
        }
        if p != c {
            for j in 0..n {
                m.swap(p * n + j, c * n + j);
                inv.swap(p * n + j, c * n + j);
            }
        }
        for j in 0..n {
            m[c * n + j] /= pv;
            inv[c * n + j] /= pv;
        }
        for r in 0..n {
            if r != c {
                let f = m[r * n + c];
                if f != 0.0 {
                    for j in 0..n {
                        m[r * n + j] -= f * m[c * n + j];
                        inv[r * n + j] -= f * inv[c * n + j];
                    }
                }
            }
        }
    }
    Some(inv)
}

/// `tr(A B)` of two `n × n` matrices (row-major).
fn trace_product(a: &[f64], b: &[f64], n: usize) -> f64 {
    let mut s = 0.0;
    for i in 0..n {
        for j in 0..n {
            s += a[i * n + j] * b[j * n + i];
        }
    }
    s
}

/// `y = A x` (`n × n`, row-major).
fn mat_vec(a: &[f64], x: &[f64], n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| (0..n).map(|j| a[i * n + j] * x[j]).sum())
        .collect()
}

/// The count's half: integer halving, in 32-bit integers when `counts_32` (FSL-141).
fn half(count: u64, counts_32: bool) -> (f64, bool) {
    let exact = (count / 2) as f64;
    if counts_32 {
        let c = count as u32 as i32;
        let h = f64::from(c / 2);
        (h, h != exact)
    } else {
        (exact, false)
    }
}

/// One collection's spatial lines.
struct Lines {
    n: usize,
    stride: usize,
    starts: Vec<usize>,
    /// The other in-plane axis' size (lines per slice).
    na: usize,
    /// Per line, per image of the collection: `K`.
    ks: Vec<Vec<SparseK>>,
}

/// The block of a restoration: the spatial lines `lines` (indices into [`Lines`]), the volumes
/// `vols`.
struct Block {
    lines: Vec<usize>,
    vols: Vec<usize>,
    /// The slice printed (vb2D).
    slice: Option<usize>,
}

/// A block's state: per volume, per line of the block, its values.
struct State {
    f: Lines3,
    kty: Lines3,
    /// The traces `tr(KᵀK Σ)` and `tr(Q_ll Σ)` of each line's last factor (shared by the
    /// volumes of a block).
    tr_k: Vec<f64>,
    tr_q: Vec<f64>,
}

/// A block's restored lines: per volume, per line of the block, its values.
type Lines3 = Vec<Vec<Vec<f64>>>;

/// Lines per batch of factors computed in parallel.
const BATCH: usize = 64;

#[allow(clippy::too_many_arguments)]
fn restore_block(
    lines: &Lines,
    block: &Block,
    stencil: &Stencil,
    q_own: &[f64],
    members: &[usize],
    moved: &[Vec<Vec<f64>>],
    o: &VbOptions,
    n_images: usize,
    out: &mut String,
) -> Result<(Lines3, bool), VbError> {
    let n = lines.n;
    let nl = block.lines.len();
    let nv = block.vols.len();
    // position of a spatial line in the block
    let mut slot = vec![usize::MAX; lines.starts.len()];
    for (b, &l) in block.lines.iter().enumerate() {
        slot[l] = b;
    }
    let y = |v: usize, k: usize, l: usize| -> Vec<f64> {
        let s0 = lines.starts[l];
        (0..n)
            .map(|j| moved[members[k]][v][s0 + j * lines.stride])
            .collect()
    };
    let mut kty = vec![vec![vec![0.0; n]; nl]; nv];
    for (vi, &v) in block.vols.iter().enumerate() {
        kty[vi] = block
            .lines
            .par_iter()
            .map(|&l| {
                let mut b = vec![0.0; n];
                for (k, km) in lines.ks[l].iter().enumerate() {
                    km.add_transpose_times(&y(v, k, l), &mut b);
                }
                b
            })
            .collect();
    }
    // the first estimate: the line prior (1, -4, 6, -4, 1), φ = λ = 1
    let prior = circulant(&[1.0, -4.0, 6.0, -4.0, 1.0], n);
    let first: Vec<Option<(Vec<f64>, f64, f64)>> = block
        .lines
        .par_iter()
        .map(|&l| {
            let g = gram(&lines.ks[l], n);
            let a: Vec<f64> = g.iter().zip(&prior).map(|(x, p)| x + p).collect();
            let s = inverse(&a, n)?;
            let (tk, tq) = (trace_product(&g, &s, n), trace_product(q_own, &s, n));
            Some((s, tk, tq))
        })
        .collect();
    let mut st = State {
        f: vec![vec![Vec::new(); nl]; nv],
        kty,
        tr_k: vec![0.0; nl],
        tr_q: vec![0.0; nl],
    };
    for (b, r) in first.into_iter().enumerate() {
        let (s, tk, tq) = r.ok_or(VbError::Singular)?;
        st.tr_k[b] = tk;
        st.tr_q[b] = tq;
        for vi in 0..nv {
            st.f[vi][b] = mat_vec(&s, &st.kty[vi][b], n);
        }
    }
    let n_vox = (n * nl * nv) as u64;
    let per_voxel = if o.method != VbMethod::Vb2D && o.count_two {
        2
    } else {
        n_images as u64
    };
    let (h_phi, w1) = half(per_voxel * n_vox, o.counts_32);
    let (h_lam, w2) = half(n_vox, o.counts_32);
    let (mut phi, mut lam) = (1.0f64, 1.0f64);
    let line_word = |rel: f64, phi: f64, lam: f64, upper: bool| {
        format!(
            "{} lambda = {},   phi = {},   lambda = {}\n",
            if upper { "Relative" } else { "relative" },
            fmt::g(rel),
            fmt::g(phi),
            fmt::g(lam)
        )
    };
    let upper = o.method != VbMethod::Vb2D;
    // Q f for one line of one volume
    let q_times = |f: &Vec<Vec<f64>>, b: usize, own_too: bool| -> Vec<f64> {
        let l = block.lines[b];
        let (a0, z0) = ((l % lines.na) as isize, (l / lines.na) as isize);
        let mut r = vec![0.0; n];
        if own_too {
            add_circ(&mut r, &stencil.own, &f[b], 1.0);
        }
        for &(oa, oz, ref c) in &stencil.cross {
            let (a, z) = (a0 + oa, z0 + oz);
            if a < 0 || a as usize >= lines.na || z < 0 {
                continue;
            }
            let m = a as usize + z as usize * lines.na;
            if m >= lines.starts.len() || slot[m] == usize::MAX {
                continue;
            }
            add_circ(&mut r, c, &f[slot[m]], 1.0);
        }
        r
    };
    for _ in 0..o.method.iterations() {
        out.push_str(&line_word(lam / phi, phi, lam, upper));
        // φ
        let r2: f64 = (0..nv)
            .flat_map(|vi| (0..nl).map(move |b| (vi, b)))
            .collect::<Vec<_>>()
            .par_iter()
            .map(|&(vi, b)| {
                let l = block.lines[b];
                lines.ks[l]
                    .iter()
                    .enumerate()
                    .map(|(k, km)| km.residual(&y(block.vols[vi], k, l), &st.f[vi][b]))
                    .sum::<f64>()
            })
            .collect::<Vec<f64>>()
            .iter()
            .sum();
        let tr_k: f64 = st.tr_k.iter().sum::<f64>() * nv as f64;
        if upper {
            out.push_str("Updating phi\n");
        }
        phi = (A0 + h_phi) / (1.0 / B0 + 0.5 * (r2 + tr_k));
        // λ
        let fqf: f64 = (0..nv)
            .flat_map(|vi| (0..nl).map(move |b| (vi, b)))
            .collect::<Vec<_>>()
            .par_iter()
            .map(|&(vi, b)| {
                let qf = q_times(&st.f[vi], b, true);
                qf.iter().zip(&st.f[vi][b]).map(|(a, b)| a * b).sum::<f64>()
            })
            .collect::<Vec<f64>>()
            .iter()
            .sum();
        let tr_q: f64 = st.tr_q.iter().sum::<f64>() * nv as f64;
        if upper {
            out.push_str("Updating lambda\n");
        }
        lam = (A0 + h_lam) / (1.0 / B0 + 0.5 * (fqf + tr_q));
        if upper {
            out.push_str(if o.method == VbMethod::Vb4D {
                "Updating images\n"
            } else {
                "Updating image\n"
            });
        }
        // the images: every line in storage order (Gauss-Seidel), the factors by batches
        for first in (0..nl).step_by(BATCH) {
            let batch: Vec<usize> = (first..(first + BATCH).min(nl)).collect();
            let factors: Vec<Option<(Vec<f64>, f64, f64)>> = batch
                .par_iter()
                .map(|&b| {
                    let g = gram(&lines.ks[block.lines[b]], n);
                    let a: Vec<f64> = g
                        .iter()
                        .zip(q_own)
                        .map(|(x, q)| phi * x + lam * q)
                        .collect();
                    let s = inverse(&a, n)?;
                    let (tk, tq) = (trace_product(&g, &s, n), trace_product(q_own, &s, n));
                    Some((s, tk, tq))
                })
                .collect();
            for (&b, r) in batch.iter().zip(factors) {
                let (s, tk, tq) = r.ok_or(VbError::Singular)?;
                st.tr_k[b] = tk;
                st.tr_q[b] = tq;
                for vi in 0..nv {
                    let qf = q_times(&st.f[vi], b, false);
                    let rhs: Vec<f64> = st.kty[vi][b]
                        .iter()
                        .zip(&qf)
                        .map(|(k, q)| phi * k - lam * q)
                        .collect();
                    st.f[vi][b] = mat_vec(&s, &rhs, n);
                }
            }
        }
    }
    if let Some(z) = block.slice {
        out.push_str(&format!(
            "slice = {z},   relative lambda = {},   phi = {},   lambda = {}\n",
            fmt::g(lam / phi),
            fmt::g(phi),
            fmt::g(lam)
        ));
    }
    Ok((st.f, w1 || w2))
}

/// `--method=vb2D|vb3D|vb4D` (§7.5).
pub fn vb(
    s: &Settings,
    o: &VbOptions,
    images: &[Image],
    field: &FieldOnGrid,
) -> Result<VbResult, VbError> {
    let colls = collections(images).map_err(VbError::Lsr)?;
    let nvox = s.dims.iter().product::<usize>();
    let n_vols = images.first().map_or(0, |i| i.volumes.len());
    let (moved, mask) = moved_images(s, images);
    let mut total = vec![vec![0.0f64; nvox]; n_vols];
    let mut stdout = format!(
        "NCollections = {}\nNScans = {}\n",
        colls.len(),
        colls.first().map_or(0, |c| c.1.len())
    );
    let mut counts_wrap = false;
    let mut count_differs = false;
    for (axis, members) in &colls {
        let axis = *axis;
        let other = 1 - axis;
        let n = s.dims[axis];
        let (starts, stride) = line_starts(s.dims, axis);
        let pts: Vec<f64> = members
            .iter()
            .map(|&k| images[k].scan.pe[axis] * images[k].scan.t)
            .collect();
        let ks: Vec<Vec<SparseK>> = starts
            .par_iter()
            .map(|&s0| {
                let h: Vec<f64> = (0..n).map(|j| field.h[s0 + j * stride]).collect();
                pts.iter()
                    .map(|&pt| SparseK::of_dense(&resampling_matrix(&h, pt), n))
                    .collect()
            })
            .collect();
        let lines = Lines {
            n,
            stride,
            starts,
            na: s.dims[other],
            ks,
        };
        let w = |d: usize| s.voxel[axis] / s.voxel[d];
        let stencil = match o.method {
            VbMethod::Vb2D => Stencil::new(&[(0, 1.0), (1, w(other))], false),
            _ => Stencil::new(&[(0, 1.0), (1, w(other)), (2, w(2))], o.half_coupling),
        };
        let q_own = circulant(&stencil.own, n);
        let nz = s.dims[2];
        let na = lines.na;
        let blocks: Vec<Block> = match o.method {
            VbMethod::Vb2D => (0..nz)
                .flat_map(|z| {
                    (0..n_vols).map(move |v| Block {
                        lines: (z * na..(z + 1) * na).collect(),
                        vols: vec![v],
                        slice: Some(z),
                    })
                })
                .collect(),
            VbMethod::Vb3D => (0..n_vols)
                .map(|v| Block {
                    lines: (0..na * nz).collect(),
                    vols: vec![v],
                    slice: None,
                })
                .collect(),
            VbMethod::Vb4D => vec![Block {
                lines: (0..na * nz).collect(),
                vols: (0..n_vols).collect(),
                slice: None,
            }],
        };
        if o.method != VbMethod::Vb2D && members.len() != 2 {
            count_differs = true;
        }
        // independent blocks in parallel, their output in order
        type BlockOut = Result<(Lines3, bool, String), VbError>;
        let results: Vec<BlockOut> = blocks
            .par_iter()
            .map(|block| {
                let mut text = String::new();
                let (f, wrap) = restore_block(
                    &lines,
                    block,
                    &stencil,
                    &q_own,
                    members,
                    &moved,
                    o,
                    members.len(),
                    &mut text,
                )?;
                Ok((f, wrap, text))
            })
            .collect();
        for (block, r) in blocks.iter().zip(results) {
            let (f, wrap, text) = r?;
            counts_wrap |= wrap;
            if let Some(z) = block.slice
                && block.vols[0] == 0
            {
                stdout.push_str(&format!("slice = {z}\n"));
            }
            stdout.push_str(&text);
            for (vi, &v) in block.vols.iter().enumerate() {
                for (b, &l) in block.lines.iter().enumerate() {
                    let s0 = lines.starts[l];
                    for (j, x) in f[vi][b].iter().enumerate() {
                        total[v][s0 + j * stride] += x;
                    }
                }
            }
        }
    }
    // the output masked voxel by voxel (observed: unlike lsr's, whose lines with an invalid voxel
    // are 0 whole)
    let volumes = total
        .into_iter()
        .map(|o| {
            o.iter()
                .zip(&mask)
                .map(|(&x, &ok)| if ok { x as f32 } else { 0.0 })
                .collect()
        })
        .collect();
    Ok(VbResult {
        corrected: Corrected { volumes, mask },
        stdout,
        counts_wrap,
        count_differs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stencil_is_the_squared_laplacian() {
        // 2D, unit weights: centre 20, axial -8, diagonal 2, distance 2: 1
        let s = Stencil::new(&[(0, 1.0), (1, 1.0)], false);
        assert_eq!(s.own, [1.0, -8.0, 20.0, -8.0, 1.0]);
        let get = |a: isize, z: isize| s.cross.iter().find(|c| c.0 == a && c.1 == z).unwrap().2;
        assert_eq!(get(1, 0), [0.0, 2.0, -8.0, 2.0, 0.0]);
        assert_eq!(get(-2, 0), [0.0, 0.0, 1.0, 0.0, 0.0]);
        // 3D, FSL-143: (±1 along the line, ±1 slice) halved
        let s = Stencil::new(&[(0, 1.0), (1, 1.0), (2, 1.0)], true);
        assert_eq!(s.own, [1.0, -12.0, 42.0, -12.0, 1.0]);
        let get = |a: isize, z: isize| s.cross.iter().find(|c| c.0 == a && c.1 == z).unwrap().2;
        assert_eq!(get(0, 1), [0.0, 1.0, -12.0, 1.0, 0.0]);
        assert_eq!(get(1, 1), [0.0, 0.0, 2.0, 0.0, 0.0]);
        assert_eq!(get(1, 0), [0.0, 2.0, -12.0, 2.0, 0.0]);
    }

    #[test]
    fn the_inverse_inverts() {
        let a = [4.0, 1.0, 0.5, 1.0, 3.0, -0.2, 0.5, -0.2, 2.0];
        let neg = [-4.0, 1.0, 0.5, 1.0, 3.0, -0.2, 0.5, -0.2, 2.0];
        for m in [a, neg] {
            let inv = inverse(&m, 3).unwrap();
            for i in 0..3 {
                for j in 0..3 {
                    let s: f64 = (0..3).map(|k| m[i * 3 + k] * inv[k * 3 + j]).sum();
                    assert!((s - f64::from(u8::from(i == j))).abs() < 1e-12);
                }
            }
        }
    }

    #[test]
    fn counts_wrap_in_32_bits() {
        assert_eq!(half(10, true), (5.0, false));
        assert_eq!(half(11, false), (5.0, false));
        let (h, w) = half(1u64 << 32 | 6, true);
        assert_eq!((h, w), (3.0, true));
    }
}
