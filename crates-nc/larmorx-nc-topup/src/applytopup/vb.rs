// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-topup 2203.5: applytopup.cpp's vb_resample_2D,
// vb_resample_3D, vb_resample_4D, GetLijk, GetLij, GetLii, tau_update, mu_update and
// add_to_slice). Copyright (C) 2009 University of Oxford (Jesper Andersson). Rust translation
// Copyright 2026 Karel Lopez Vilaret; modified from the originals. Distributed under the FSL
// Licence (see LICENSE).
//! applytopup's undocumented `--method=vb2D`, `vb3D` and `vb4D`: least-squares restoration of the
//! lines along the phase encoding (as `--method=lsr`) with the noise precision `phi` and the
//! regularisation weight `lambda` inferred by variational Bayes (gamma priors of shape `1e-10`
//! and scale `1e10`), the images' prior a second-difference (Laplacian squared) smoothness
//! across the lines: within a slice (`vb2D`: `phi` and `lambda` per slice and volume, 50
//! iterations), across the slices too (`vb3D`: per volume, 20 iterations), and over every
//! volume at once (`vb4D`: one `phi` and `lambda`, 20 iterations).
//!
//! The arithmetic is Armadillo 14.2.1's as armawrap hands the NEWMAT expressions to it (see
//! `docs/findings/fsl-topup.md`): `inv(A) * B` a solve (`dgesv`, the symmetric solver from 100
//! rows), `k * inv(A) * x` the explicit inverse (`dgetrf` + `dgetri`) times `x` by `dgemv` with
//! `alpha = k`, a `CroutMatrix`'s `i() * x` its stored inverse times `x`, `(a.t() * b)
//! .AsScalar()` Armadillo's dot product, the same product inside a scalar expression a `dgemv`
//! of one column, `.Trace()` of the evaluated product; the sparse matrices are fsl-miscmaths'
//! `SpMat<double>` on one thread.

use larmorx_core::defects;
use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::cnum::fmt_g;
use larmorx_nc_core::lapack::blas::{self, MatRef};
use larmorx_nc_core::lapack::blocked::GETRI_MAX_N;
use larmorx_nc_core::miscmaths::{INV_ERROR, SOLVE_ERROR};
use larmorx_nc_core::spmat::SpMat;
use larmorx_nc_core::volume::{FslError, Volume};

use super::{Collections, Ctx, End, R, crout_inverse};
use crate::dispvec::DispVec;
use crate::fileio::TopupFileReader;

/// The three methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Vb {
    D2,
    D3,
    D4,
}

/// The sparse matrices' thread count (`NoOfThreads()`'s default: FSL_NUM_THREADS, else 1; the
/// replica's is FSL's one-thread result).
const NT: u32 = 1;

fn runtime_error(what: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "std::runtime_error",
        what: what.to_owned(),
    }
}

/// `(a.t() * b).AsScalar()`: Armadillo's `op_dot::direct_dot` (two partial sums up to 32
/// elements, BLAS `ddot` beyond).
fn dot(a: &[f64], b: &[f64]) -> f64 {
    arma::dot_blas(a, b, 1)
}

/// `a.t() * b` evaluated inside a scalar expression (`glue_times::apply` of a column's
/// transpose: `dgemv('T')` of `b` as an `n × 1` matrix with `a`).
fn dot_gemv(a: &[f64], b: &[f64]) -> f64 {
    arma::mul(&Mat::col(a), &Mat::col(b), true).at(0, 0)
}

/// `(A.AsNEWMAT().i() * B.AsNEWMAT()).Trace()`: the trace of `solve(A, B)`.
fn trace_solve(a: &SpMat<f64>, b: &SpMat<f64>) -> Result<f64, FslError> {
    let x = arma::solve(&a.as_dense(), &b.as_dense()).map_err(|_| runtime_error(SOLVE_ERROR))?;
    Ok(arma::trace(&x))
}

/// `k * A.AsNEWMAT().i() * x`: the explicit inverse (`inv()`) times `x` by `dgemv` with
/// `alpha = k`.
fn scaled_inverse_times(a: &SpMat<f64>, x: &[f64], k: f64) -> Result<Vec<f64>, FslError> {
    let ai = arma::inv(&a.as_dense()).map_err(|_| runtime_error(INV_ERROR))?;
    let n = ai.rows();
    let mut y = vec![0.0; n];
    let am = MatRef { off: 0, ld: n };
    blas::dgemv_n(n, n, k, ai.as_slice(), am, x, 1, 0.0, &mut y);
    Ok(y)
}

/// `A.AsNEWMAT().i() * x`: `solve(A, x)`.
fn solve_vec(a: &SpMat<f64>, x: &[f64]) -> Result<Vec<f64>, FslError> {
    let s = arma::solve(&a.as_dense(), &Mat::col(x)).map_err(|_| runtime_error(SOLVE_ERROR))?;
    Ok(s.as_slice().to_vec())
}

/// A `CroutMatrix`'s `i() * x`: the stored inverse times `x` (`dgemv`).
fn stored_inverse_times(ai: &Mat, x: &[f64]) -> Vec<f64> {
    arma::mul(ai, &Mat::col(x), false).as_slice().to_vec()
}

/// `a * A + b * B` (`SpMat(a * A) += b * B`).
fn combine(a: f64, x: &SpMat<f64>, b: f64, y: &SpMat<f64>) -> Result<SpMat<f64>, FslError> {
    let mut s = x.scaled(a);
    s.add_assign(&y.scaled(b))?;
    Ok(s)
}

/// `v -= (s * L) * x`.
fn sub_scaled(v: &mut [f64], s: f64, l: &SpMat<f64>, x: &[f64]) -> Result<(), FslError> {
    let p = l.scaled(s).mul_vec(x)?;
    for (a, b) in v.iter_mut().zip(p) {
        *a -= b;
    }
    Ok(())
}

/// A sparse `sz × sz` matrix from FSL's `Set(r, c, v)` calls (1-based).
fn sp(sz: usize, sets: &[(usize, usize, f64)]) -> Result<SpMat<f64>, FslError> {
    let mut m = SpMat::new(sz as u32, sz as u32, NT);
    for &(r, c, v) in sets {
        m.set(r as u32, c as u32, v)?;
    }
    Ok(m)
}

/// The circulant band `a13, a12, a11, a12, a13` in FSL's order of `Set` calls.
fn pentadiagonal(sz: usize, a11: f64, a12: f64, a13: f64) -> Result<SpMat<f64>, FslError> {
    let mut s = vec![
        (1, sz - 1, a13),
        (1, sz, a12),
        (1, 1, a11),
        (1, 2, a12),
        (1, 3, a13),
        (2, sz, a13),
        (2, 1, a12),
        (2, 2, a11),
        (2, 3, a12),
        (2, 4, a13),
    ];
    for i in 3..=sz - 2 {
        s.extend([
            (i, i - 2, a13),
            (i, i - 1, a12),
            (i, i, a11),
            (i, i + 1, a12),
            (i, i + 2, a13),
        ]);
    }
    s.extend([
        (sz - 1, sz - 3, a13),
        (sz - 1, sz - 2, a12),
        (sz - 1, sz - 1, a11),
        (sz - 1, sz, a12),
        (sz - 1, 1, a13),
        (sz, sz - 2, a13),
        (sz, sz - 1, a12),
        (sz, sz, a11),
        (sz, 1, a12),
        (sz, 2, a13),
    ]);
    sp(sz, &s)
}

/// The circulant band `a12, a11, a12`.
fn tridiagonal(sz: usize, a11: f64, a12: f64) -> Result<SpMat<f64>, FslError> {
    let mut s = vec![(1, sz, a12), (1, 1, a11), (1, 2, a12)];
    for i in 2..sz {
        s.extend([(i, i - 1, a12), (i, i, a11), (i, i + 1, a12)]);
    }
    s.extend([(sz, sz - 1, a12), (sz, sz, a11), (sz, 1, a12)]);
    sp(sz, &s)
}

/// The diagonal `a11`.
fn diagonal(sz: usize, a11: f64) -> Result<SpMat<f64>, FslError> {
    let s: Vec<_> = (1..=sz).map(|i| (i, i, a11)).collect();
    sp(sz, &s)
}

fn sqr(x: f64) -> f64 {
    x * x
}

/// `GetLii(sz)`: the square of the circular second difference along the phase encoding
/// (6, -4, 1), the prior of the first estimate.
fn get_lii(sz: usize) -> Result<SpMat<f64>, FslError> {
    pentadiagonal(sz, 6.0, -4.0, 1.0)
}

/// `GetLij(sz, sf)`: the blocks of `AᵀA` for the 2D second difference `A` of a slice (the phase
/// encoding first, the other direction scaled by `sf`): the diagonal block and those one and
/// two lines away.
fn get_lij(sz: usize, sf: f64) -> Result<Vec<SpMat<f64>>, FslError> {
    let c = 2.0 + 2.0 * sf;
    let l0 = pentadiagonal(sz, sqr(c) + 2.0 * sqr(-1.0) + 2.0 * sqr(-sf), -2.0 * c, 1.0)?;
    let l1 = tridiagonal(sz, 2.0 * (-sf) * c, -2.0 * (-sf))?;
    let l2 = diagonal(sz, sqr(-sf))?;
    Ok(vec![l0, l1, l2])
}

/// `GetLijk(sz, sf)`: the blocks of `AᵀA` for the 3D second difference (the second and third
/// directions scaled by `sf[0]` and `sf[1]`): `[0][0]` the diagonal block, `[0][1]` and `[0][2]`
/// one and two lines away in the slice, `[1][0]` and `[2][0]` one and two slices away, `[1][1]`
/// one line and one slice away. FSL-143: FSL's `[1][0]` has its off-diagonal elements at
/// `sf[1]` where `AᵀA` has `2 sf[1]` (as `[0][1]` has `2 sf[0]`); `fixed` gives `AᵀA`'s.
fn get_lijk(sz: usize, sf: [f64; 2], fixed: bool) -> Result<Vec<Vec<SpMat<f64>>>, FslError> {
    let c = 2.0 + 2.0 * sf[0] + 2.0 * sf[1];
    let l00 = pentadiagonal(
        sz,
        sqr(c) + 2.0 * sqr(-1.0) + 2.0 * sqr(-sf[0]) + 2.0 * sqr(-sf[1]),
        2.0 * -c,
        1.0,
    )?;
    let l01 = tridiagonal(sz, 2.0 * (-sf[0] * c), -2.0 * (-sf[0]))?;
    let l02 = diagonal(sz, sqr(-sf[0]))?;
    let a12 = if fixed { -2.0 * (-sf[1]) } else { -(-sf[1]) };
    let l10 = tridiagonal(sz, 2.0 * (-sf[1] * c), a12)?;
    let l11 = diagonal(sz, 2.0 * (-sf[0]) * (-sf[1]))?;
    let l20 = diagonal(sz, sqr(-sf[1]))?;
    Ok(vec![vec![l00, l01, l02], vec![l10, l11], vec![l20]])
}

/// One collection's lines: `K` (the scans' displacement matrices one below the other), `KᵀK`.
struct Line {
    k: SpMat<f64>,
    ktk: SpMat<f64>,
}

/// What a collection's lines share.
struct Coll<'a> {
    row: bool,
    pesz: usize,
    fesz: usize,
    /// (scan, --datain index) of each scan.
    scans_of: &'a [(usize, u32)],
}

impl Coll<'_> {
    /// The line `i` of slice `sl`: `DispVec` from the field, each scan's `K` below the other.
    fn line(&self, ctx: &Ctx, dv: &mut DispVec, field: &Volume, sl: usize, i: usize) -> R<Line> {
        if self.row {
            dv.set_from_row(field, sl as i64, i as i64);
        } else {
            dv.set_from_column(field, sl as i64, i as i64);
        }
        let mut k: Option<SpMat<f64>> = None;
        for &(_, index) in self.scans_of {
            let axis = usize::from(!self.row);
            let psf = ctx.pe(index)[axis] * ctx.rt(index);
            let km = SpMat::from_dense(&dv.k_matrix(psf)?, NT);
            match k.as_mut() {
                None => k = Some(km),
                Some(k) => k.vert_concat(&km)?,
            }
        }
        let k = k.expect("a collection has scans");
        let ktk = k.trans_mult_self(NT)?;
        Ok(Line { k, ktk })
    }

    /// `y`: the line of each scan's volume `d`, one below the other.
    fn data(&self, scans: &[Volume], d: usize, sl: usize, i: usize) -> Vec<f64> {
        let mut y = Vec::with_capacity(self.scans_of.len() * self.pesz);
        for &(s, _) in self.scans_of {
            let v = &scans[s];
            let base = d * (v.xsize() * v.ysize() * v.zsize()) as usize;
            for p in 0..self.pesz {
                let w = if self.row {
                    v.idx(p as i64, i as i64, sl as i64)
                } else {
                    v.idx(i as i64, p as i64, sl as i64)
                };
                y.push(f64::from(v.data[base + w]));
            }
        }
        y
    }

    /// `add_to_slice(ovol[d], mu, sl, row)`.
    fn add_to_slice(&self, ovol: &mut Volume, d: usize, mu: &[Vec<f64>], sl: usize) {
        let n3 = (ovol.xsize() * ovol.ysize() * ovol.zsize()) as usize;
        for (i, m) in mu.iter().enumerate() {
            for (j, &v) in m.iter().enumerate() {
                let w = if self.row {
                    ovol.idx(j as i64, i as i64, sl as i64)
                } else {
                    ovol.idx(i as i64, j as i64, sl as i64)
                };
                let o = &mut ovol.data[d * n3 + w];
                *o = (f64::from(*o) + v) as f32;
            }
        }
    }
}

/// The gamma shapes' counts `a * b / 2`, which FSL computes in `int` (FSL-141: beyond 2^31 the
/// product wraps); `fixed` computes them in 64 bits.
fn half_count(ctx: &Ctx, a: i32, b: i32) -> f64 {
    let wide = i64::from(a) * i64::from(b) / 2;
    let wrapped = i64::from(a.wrapping_mul(b) / 2);
    if wide == wrapped {
        return wide as f64;
    }
    if ctx.con.reproduce() {
        ctx.con.notice(
            &defects::FSL_141,
            "the number of data points wraps around in 32-bit arithmetic, as in applytopup",
        );
        wrapped as f64
    } else {
        ctx.con.notice(
            &defects::FSL_141,
            "the number of data points counted in 64-bit arithmetic",
        );
        wide as f64
    }
}

fn report(ctx: &Ctx, head: &str, l: f64, tau: f64, k: f64, theta: f64) {
    ctx.con.line(&format!(
        "{head} = {},   phi = {},   lambda = {}",
        fmt_g((l * tau) / (k * theta), 6),
        fmt_g(k * theta, 6),
        fmt_g(l * tau, 6)
    ));
}

/// Gamma priors: shapes `l_0 = k_0 = 1e-10`, scales `tau_0 = theta_0 = 1e10`.
const L0: f64 = 1e-10;
const TAU0: f64 = 1e10;
const K0: f64 = 1e-10;
const THETA0: f64 = 1e10;

/// `vb_resample_2D`, `vb_resample_3D` or `vb_resample_4D` (after the movements' resampling).
pub(super) fn vb_resample(
    ctx: &Ctx,
    method: Vb,
    topupfile: &mut TopupFileReader,
    scans: &[Volume],
) -> R<Volume> {
    let field = topupfile.field_as_volume()?;
    let collections = Collections::new(ctx.inindices, ctx.datafile)?;
    let mut ovol = scans[0].clone();
    ovol.data.fill(0.0);
    ctx.con
        .line(&format!("NCollections = {}", collections.groups.len()));
    ctx.con
        .line(&format!("NScans = {}", collections.groups[0].len()));
    let (xs, ys) = (scans[0].xsize() as usize, scans[0].ysize() as usize);
    let reproduce = ctx.con.reproduce();
    let mut wrap_differs = false;
    for g in &collections.groups {
        let row = ctx.pe(g[0].1)[0] != 0.0;
        let (pesz, fesz) = if row { (xs, ys) } else { (ys, xs) };
        let coll = Coll {
            row,
            pesz,
            fesz,
            scans_of: g,
        };
        size_check(method, pesz)?;
        let mut dv = DispVec::new(pesz, reproduce);
        match method {
            Vb::D2 => vb_2d(ctx, &coll, &mut dv, &field, scans, &mut ovol)?,
            Vb::D3 | Vb::D4 => vb_3d_4d(ctx, method, &coll, &mut dv, &field, scans, &mut ovol)?,
        }
        wrap_differs |= dv.ca.wrapped_differently;
    }
    if wrap_differs {
        ctx.con.notice(
            &defects::FSL_133,
            if reproduce {
                "the displacements before a line's first voxel read where applytopup's unsigned \
                 remainder puts them"
            } else {
                "the displacements before a line's first voxel read circularly (from the line's \
                 end)"
            },
        );
    }
    Ok(ovol)
}

/// The sizes the replica follows FSL at.
fn size_check(method: Vb, pesz: usize) -> R<()> {
    if pesz < 8 {
        return Err(End::Refuse(format!(
            "lines of {pesz} voxels along the phase encoding: Armadillo's solvers for systems \
             this small are not replicated"
        )));
    }
    if pesz > 256 {
        return Err(End::Refuse(format!(
            "lines of {pesz} voxels along the phase encoding: OpenBLAS's LU of more than 256 \
             rows is not replicated yet"
        )));
    }
    if method != Vb::D4 && pesz > GETRI_MAX_N {
        return Err(End::Refuse(format!(
            "lines of {pesz} voxels along the phase encoding: the first estimate's explicit \
             inverse (LAPACK's blocked dgetri, from 100 rows Armadillo's dsytri) is not \
             replicated yet beyond {GETRI_MAX_N} rows"
        )));
    }
    Ok(())
}

/// `vb_resample_2D`'s collection: `phi` and `lambda` per slice and volume.
fn vb_2d(
    ctx: &Ctx,
    c: &Coll,
    dv: &mut DispVec,
    field: &Volume,
    scans: &[Volume],
    ovol: &mut Volume,
) -> R<()> {
    let sf0 = f64::from(scans[0].ydim() / scans[0].xdim());
    let sf = if c.row { 1.0 / sf0 } else { sf0 };
    let lij = get_lij(c.pesz, sf)?;
    let lii = get_lii(c.pesz)?;
    let m = c.pesz as i32;
    let nn = c.fesz as i32;
    let n = (c.scans_of.len() as u32).wrapping_mul(c.pesz as u32) as i32;
    let nt = scans[0].tsize() as usize;
    for sl in 0..scans[0].zsize() as usize {
        ctx.con.line(&format!("slice = {sl}"));
        let lines = (0..c.fesz)
            .map(|i| c.line(ctx, dv, field, sl, i))
            .collect::<R<Vec<_>>>()?;
        for d in 0..nt {
            let y: Vec<Vec<f64>> = (0..c.fesz).map(|i| c.data(scans, d, sl, i)).collect();
            let yty: Vec<f64> = y.iter().map(|v| dot(v, v)).collect();
            let kty = lines
                .iter()
                .zip(&y)
                .map(|(l, v)| l.k.trans_mult(v))
                .collect::<Result<Vec<_>, _>>()?;
            let (mut l, mut tau, mut k, mut theta) = (L0, TAU0, K0, THETA0);
            let mut lambda = Vec::with_capacity(c.fesz);
            let mut mu = Vec::with_capacity(c.fesz);
            for i in 0..c.fesz {
                lambda.push(combine(k * theta, &lines[i].ktk, l * tau, &lii)?);
                mu.push(scaled_inverse_times(&lambda[i], &kty[i], k * theta)?);
            }
            for _ in 0..50 {
                report(ctx, "relative lambda", l, tau, k, theta);
                k = K0 + half_count(ctx, n, nn);
                theta = 1.0 / THETA0;
                for i in 0..c.fesz {
                    let ktk_mu = lines[i].ktk.mul_vec(&mu[i])?;
                    let tr = trace_solve(&lambda[i], &lines[i].ktk)?;
                    theta += ((dot_gemv(&mu[i], &ktk_mu) + yty[i]) + tr) * 0.5;
                    theta -= dot(&mu[i], &kty[i]);
                }
                theta = 1.0 / theta;
                l = L0 + half_count(ctx, m, nn);
                tau = 1.0 / TAU0;
                for i in 0..c.fesz {
                    let tr = trace_solve(&lambda[i], &lij[0])?;
                    let l_mu = lij[0].mul_vec(&mu[i])?;
                    tau += (dot_gemv(&mu[i], &l_mu) + tr) * 0.5;
                    for j in i.saturating_sub(2)..=(i + 2).min(c.fesz - 1) {
                        if j != i {
                            let p = lij[i.abs_diff(j)].mul_vec(&mu[j])?;
                            tau += dot(&mu[i], &p) * 0.5;
                        }
                    }
                }
                tau = 1.0 / tau;
                for i in 0..c.fesz {
                    lambda[i] = combine(k * theta, &lines[i].ktk, l * tau, &lij[0])?;
                    let mut v: Vec<f64> = kty[i].iter().map(|&x| x * (k * theta)).collect();
                    for j in i.saturating_sub(2)..=(i + 2).min(c.fesz - 1) {
                        if j != i {
                            sub_scaled(&mut v, l * tau, &lij[i.abs_diff(j)], &mu[j])?;
                        }
                    }
                    mu[i] = solve_vec(&lambda[i], &v)?;
                }
            }
            report(
                ctx,
                &format!("slice = {sl},   relative lambda"),
                l,
                tau,
                k,
                theta,
            );
            c.add_to_slice(ovol, d, &mu, sl);
        }
    }
    Ok(())
}

/// `tau_update`: the regularisation's scale from the images' smoothness (every volume's for
/// `vb4D`, one volume's for `vb3D`).
fn tau_update(
    method: Vb,
    mu: &[Vec<Vec<Vec<f64>>>],
    lambda: &[Vec<SpMat<f64>>],
    lijk: &[Vec<SpMat<f64>>],
) -> Result<f64, FslError> {
    let nd = mu.len();
    let nsl = mu[0].len();
    let nrow = mu[0][0].len();
    let mut tau = 1.0 / TAU0;
    for k in 0..nsl {
        for i in 0..nrow {
            let tr = trace_solve(&lambda[k][i], &lijk[0][0])?;
            if method == Vb::D4 {
                tau += 0.5 * nd as f64 * tr;
            }
            for m in mu {
                let mki = &m[k][i];
                let l_mu = lijk[0][0].mul_vec(mki)?;
                if method == Vb::D4 {
                    tau += 0.5 * dot(mki, &l_mu);
                } else {
                    tau += (dot_gemv(mki, &l_mu) + tr) * 0.5;
                }
                let mut pair = |l: &SpMat<f64>, other: &[f64]| -> Result<(), FslError> {
                    tau += dot(mki, &l.mul_vec(other)?) * 0.5;
                    Ok(())
                };
                for ii in i.saturating_sub(2)..=(i + 2).min(nrow - 1) {
                    if ii != i {
                        pair(&lijk[0][i.abs_diff(ii)], &m[k][ii])?;
                    }
                }
                for kk in k.saturating_sub(2)..=(k + 2).min(nsl - 1) {
                    if kk != k {
                        pair(&lijk[k.abs_diff(kk)][0], &m[kk][i])?;
                    }
                }
                if k > 0 && i > 0 {
                    pair(&lijk[1][1], &m[k - 1][i - 1])?;
                }
                if k > 0 && i + 1 < nrow {
                    pair(&lijk[1][1], &m[k - 1][i + 1])?;
                }
                if k + 1 < nsl && i + 1 < nrow {
                    pair(&lijk[1][1], &m[k + 1][i + 1])?;
                }
                if k + 1 < nsl && i > 0 {
                    pair(&lijk[1][1], &m[k + 1][i - 1])?;
                }
            }
        }
    }
    Ok(1.0 / tau)
}

/// `mu_update`: line `row` of slice `sl` from its data and its neighbours' current estimates.
#[allow(clippy::too_many_arguments)]
fn mu_update(
    mu: &[Vec<Vec<f64>>],
    kty: &[f64],
    ainv: &Mat,
    lijk: &[Vec<SpMat<f64>>],
    phi: f64,
    lambda: f64,
    sl: usize,
    row: usize,
) -> Result<Vec<f64>, FslError> {
    let nsl = mu.len();
    let nrow = mu[0].len();
    let mut v: Vec<f64> = kty.iter().map(|&x| x * phi).collect();
    for i in row.saturating_sub(2)..=(row + 2).min(nrow - 1) {
        if i != row {
            sub_scaled(&mut v, lambda, &lijk[0][i.abs_diff(row)], &mu[sl][i])?;
        }
    }
    for i in sl.saturating_sub(2)..=(sl + 2).min(nsl - 1) {
        if i != sl {
            sub_scaled(&mut v, lambda, &lijk[i.abs_diff(sl)][0], &mu[i][row])?;
        }
    }
    if sl > 0 && row > 0 {
        sub_scaled(&mut v, lambda, &lijk[1][1], &mu[sl - 1][row - 1])?;
    }
    if sl > 0 && row + 1 < nrow {
        sub_scaled(&mut v, lambda, &lijk[1][1], &mu[sl - 1][row + 1])?;
    }
    if sl + 1 < nsl && row + 1 < nrow {
        sub_scaled(&mut v, lambda, &lijk[1][1], &mu[sl + 1][row + 1])?;
    }
    if sl + 1 < nsl && row > 0 {
        sub_scaled(&mut v, lambda, &lijk[1][1], &mu[sl + 1][row - 1])?;
    }
    Ok(stored_inverse_times(ainv, &v))
}

/// `vb_resample_3D` (every volume on its own) and `vb_resample_4D` (all volumes together).
fn vb_3d_4d(
    ctx: &Ctx,
    method: Vb,
    c: &Coll,
    dv: &mut DispVec,
    field: &Volume,
    scans: &[Volume],
    ovol: &mut Volume,
) -> R<()> {
    let s0 = &scans[0];
    let (nz, nt) = (s0.zsize() as usize, s0.tsize() as usize);
    let sf = if c.row {
        [
            f64::from(s0.xdim() / s0.ydim()),
            f64::from(s0.xdim() / s0.zdim()),
        ]
    } else {
        [
            f64::from(s0.ydim() / s0.xdim()),
            f64::from(s0.ydim() / s0.zdim()),
        ]
    };
    let fixed = !ctx.con.reproduce();
    ctx.con.notice(
        &defects::FSL_143,
        if fixed {
            "the regularisation between neighbouring slices with AᵀA's coefficients"
        } else {
            "the regularisation between neighbouring slices at half AᵀA's coefficients, as in \
             applytopup"
        },
    );
    let lijk = get_lijk(c.pesz, sf, fixed)?;
    let lii = get_lii(c.pesz)?;
    let m = c.pesz as i32;
    // FSL-142: FSL counts two scans' data points whatever the collection holds
    let scans_counted = if c.scans_of.len() != 2 {
        if ctx.con.reproduce() {
            ctx.con.notice(
                &defects::FSL_142,
                "the data points counted for two scans, as in applytopup",
            );
            2
        } else {
            ctx.con.notice(
                &defects::FSL_142,
                &format!("the data points counted for the {} scans", c.scans_of.len()),
            );
            c.scans_of.len() as i32
        }
    } else {
        2
    };
    let n = scans_counted.wrapping_mul(m);
    let mut nn = (c.fesz as u32).wrapping_mul(nz as u32);
    if method == Vb::D4 {
        nn = nn.wrapping_mul(nt as u32);
    }
    let nn = nn as i32;
    let mut lines = Vec::with_capacity(nz);
    for sl in 0..nz {
        let row = (0..c.fesz)
            .map(|i| c.line(ctx, dv, field, sl, i))
            .collect::<R<Vec<_>>>()?;
        lines.push(row);
    }
    // the volumes estimated together: all (vb4D) or one at a time (vb3D)
    let groups: Vec<Vec<usize>> = if method == Vb::D4 {
        vec![(0..nt).collect()]
    } else {
        (0..nt).map(|d| vec![d]).collect()
    };
    for vols in groups {
        let mut kty = Vec::with_capacity(vols.len());
        let mut yty = Vec::with_capacity(vols.len());
        for &d in &vols {
            let mut ks = Vec::with_capacity(nz);
            let mut ys = Vec::with_capacity(nz);
            for (sl, row) in lines.iter().enumerate() {
                let mut kr = Vec::with_capacity(c.fesz);
                let mut yr = Vec::with_capacity(c.fesz);
                for (i, line) in row.iter().enumerate() {
                    let y = c.data(scans, d, sl, i);
                    yr.push(dot(&y, &y));
                    kr.push(line.k.trans_mult(&y)?);
                }
                ks.push(kr);
                ys.push(yr);
            }
            kty.push(ks);
            yty.push(ys);
        }
        let (mut l, mut tau, mut k, mut theta) = (L0, TAU0, K0, THETA0);
        let mut lambda: Vec<Vec<SpMat<f64>>> = Vec::with_capacity(nz);
        let mut mu: Vec<Vec<Vec<Vec<f64>>>> = vols.iter().map(|_| Vec::new()).collect();
        for sl in 0..nz {
            let mut lr = Vec::with_capacity(c.fesz);
            for i in 0..c.fesz {
                lr.push(combine(k * theta, &lines[sl][i].ktk, l * tau, &lii)?);
            }
            lambda.push(lr);
            for (dd, m) in mu.iter_mut().enumerate() {
                let mut row = Vec::with_capacity(c.fesz);
                for i in 0..c.fesz {
                    row.push(if method == Vb::D4 {
                        Vec::new()
                    } else {
                        scaled_inverse_times(&lambda[sl][i], &kty[dd][sl][i], k * theta)?
                    });
                }
                m.push(row);
            }
            if method == Vb::D4 {
                for i in 0..c.fesz {
                    let ai = crout_inverse(&lambda[sl][i].as_dense()).map_err(End::Refuse)?;
                    for (dd, m) in mu.iter_mut().enumerate() {
                        let t = stored_inverse_times(&ai, &kty[dd][sl][i]);
                        m[sl][i] = t.iter().map(|&x| x * (k * theta)).collect();
                    }
                }
            }
        }
        let (head, image) = if method == Vb::D4 {
            ("Relative lambda", "Updating images")
        } else {
            ("Relative lambda", "Updating image")
        };
        for _ in 0..20 {
            report(ctx, head, l, tau, k, theta);
            ctx.con.line("Updating phi");
            k = K0 + half_count(ctx, n, nn);
            theta = 1.0 / THETA0;
            for sl in 0..nz {
                for i in 0..c.fesz {
                    let ktk = &lines[sl][i].ktk;
                    let tr = trace_solve(&lambda[sl][i], ktk)?;
                    if method == Vb::D4 {
                        theta += 0.5 * vols.len() as f64 * tr;
                    }
                    for dd in 0..vols.len() {
                        let mdi = &mu[dd][sl][i];
                        let g = dot_gemv(mdi, &ktk.mul_vec(mdi)?);
                        if method == Vb::D4 {
                            theta += (g + yty[dd][sl][i]) * 0.5;
                        } else {
                            theta += ((g + yty[dd][sl][i]) + tr) * 0.5;
                        }
                        theta -= dot(mdi, &kty[dd][sl][i]);
                    }
                }
            }
            theta = 1.0 / theta;
            ctx.con.line("Updating lambda");
            l = L0 + half_count(ctx, m, nn);
            tau = tau_update(method, &mu, &lambda, &lijk)?;
            ctx.con.line(image);
            for sl in 0..nz {
                for i in 0..c.fesz {
                    lambda[sl][i] = combine(k * theta, &lines[sl][i].ktk, l * tau, &lijk[0][0])?;
                    let ai = crout_inverse(&lambda[sl][i].as_dense()).map_err(End::Refuse)?;
                    for dd in 0..vols.len() {
                        let v = mu_update(
                            &mu[dd],
                            &kty[dd][sl][i],
                            &ai,
                            &lijk,
                            k * theta,
                            l * tau,
                            sl,
                            i,
                        )?;
                        mu[dd][sl][i] = v;
                    }
                }
            }
        }
        for (dd, &d) in vols.iter().enumerate() {
            for sl in 0..nz {
                c.add_to_slice(ovol, d, &mu[dd][sl], sl);
            }
        }
    }
    Ok(())
}
