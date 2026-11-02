// SPDX-License-Identifier: LicenseRef-FSL
// Rust translation Copyright 2026 Karel Lopez Vilaret. Distributed under the FSL Licence (see
// LICENSE).
//! The replica's side of `validation/probes/fsl-miscmaths/`: writes, into `replica.txt` of every
//! case directory under `MM_PROBE_ROOT`, the lines `miscmaths_probe.cc` prints from FSL's own
//! library, from `larmorx_nc_core::{spmat, bfmatrix, nonlin}`. Run with `--ignored`.

// The cost-function keeps the C++ probe's indexed loops, so the two can be compared line by line.
#![allow(clippy::needless_range_loop)]

use larmorx_nc_core::arma::Mat;
use larmorx_nc_core::bfmatrix::BFMatrix;
use larmorx_nc_core::nonlin::{NlMethod, NonlinCf, NonlinParam, nonlin};
use larmorx_nc_core::spmat::{MatrixType, SpElem, SpMat};
use larmorx_nc_core::volume::FslError;
use std::fmt::Write as _;
use std::path::Path;

struct Csc {
    m: u32,
    n: u32,
    irp: Vec<u32>,
    jcp: Vec<u32>,
    val: Vec<f64>,
}

fn u32s(b: &[u8], off: usize, n: usize) -> Vec<u32> {
    (0..n)
        .map(|i| u32::from_le_bytes(b[off + 4 * i..off + 4 * i + 4].try_into().unwrap()))
        .collect()
}

fn f64s(b: &[u8], off: usize, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| f64::from_le_bytes(b[off + 8 * i..off + 8 * i + 8].try_into().unwrap()))
        .collect()
}

fn read_csc(p: &Path) -> Csc {
    let b = std::fs::read(p).unwrap();
    let h = u32s(&b, 0, 3);
    let (m, n, nnz) = (h[0], h[1], h[2] as usize);
    let irp = u32s(&b, 12, nnz);
    let jcp = u32s(&b, 12 + 4 * nnz, n as usize + 1);
    let val = f64s(&b, 12 + 4 * nnz + 4 * (n as usize + 1), nnz);
    Csc {
        m,
        n,
        irp,
        jcp,
        val,
    }
}

fn pd(out: &mut String, tag: &str, v: f64) {
    writeln!(out, "{tag} {:016x}", v.to_bits()).unwrap();
}

fn pvec(out: &mut String, tag: &str, v: &[f64]) {
    writeln!(out, "{tag} n={}", v.len()).unwrap();
    for &x in v {
        pd(out, tag, x);
    }
}

fn psp<T: SpElem>(out: &mut String, tag: &str, s: &SpMat<T>) {
    writeln!(out, "{tag} m={} n={} nz={}", s.nrows(), s.ncols(), s.nz()).unwrap();
    for c in 0..s.ncols() as usize {
        let (ri, val) = s.column(c);
        for (k, &r) in ri.iter().enumerate() {
            writeln!(out, "{tag} {} {} {:016x}", c, r, val[k].to_f64().to_bits()).unwrap();
        }
    }
}

fn pbf(out: &mut String, tag: &str, m: &BFMatrix) {
    match m {
        BFMatrix::Float(s) => psp(out, tag, s),
        BFMatrix::Double(s) => psp(out, tag, s),
        BFMatrix::Full(a) => {
            writeln!(out, "{tag} full {} {}", a.rows(), a.cols()).unwrap();
            for c in 0..a.cols() {
                for r in 0..a.rows() {
                    pd(out, tag, a.at(r, c));
                }
            }
        }
    }
}

trait Wrap: SpElem {
    fn wrap(s: SpMat<Self>) -> BFMatrix;
}
impl Wrap for f32 {
    fn wrap(s: SpMat<f32>) -> BFMatrix {
        BFMatrix::Float(s)
    }
}
impl Wrap for f64 {
    fn wrap(s: SpMat<f64>) -> BFMatrix {
        BFMatrix::Double(s)
    }
}

/// The probe's cost-function (see `miscmaths_probe.cc`).
struct TestCf {
    c: Vec<f64>,
    d: Vec<f64>,
    q: f64,
    mode: i32,
}

impl NonlinCf for TestCf {
    fn cf(&mut self, p: &[f64]) -> Result<f64, FslError> {
        let n = p.len();
        let mut s = 0.0;
        for i in 0..n {
            let t1 = p[i] - self.c[i];
            let t2 = p[i] * p[i] - self.d[i];
            s += t1 * t1 + 0.5 * (t2 * t2);
        }
        for i in 0..n - 1 {
            let t = p[i + 1] - p[i];
            s += self.q * (t * t);
        }
        Ok(s)
    }
    fn grad(&mut self, p: &[f64]) -> Result<Vec<f64>, FslError> {
        if self.mode == 3 {
            return larmorx_nc_core::nonlin::numerical_grad(self, p);
        }
        let n = p.len();
        Ok((0..n)
            .map(|i| {
                let t2 = p[i] * p[i] - self.d[i];
                let mut v = 2.0 * (p[i] - self.c[i]) + 2.0 * (t2 * p[i]);
                if i > 0 {
                    v += 2.0 * self.q * (p[i] - p[i - 1]);
                }
                if i < n - 1 {
                    v -= 2.0 * self.q * (p[i + 1] - p[i]);
                }
                v
            })
            .collect())
    }
    fn hess(&mut self, p: &[f64], iptr: Option<BFMatrix>) -> Result<BFMatrix, FslError> {
        if self.mode == 3 {
            return larmorx_nc_core::nonlin::numerical_hess(self, p, iptr);
        }
        let n = p.len();
        let (mut irp, mut jcp, mut val) = (Vec::new(), Vec::new(), Vec::new());
        for c in 1..=n {
            jcp.push(irp.len() as u32);
            if c > 1 {
                irp.push(c as u32 - 2);
                val.push(-2.0 * self.q);
            }
            let pc = p[c - 1];
            let mut dg = 2.0 + 2.0 * (3.0 * pc * pc - self.d[c - 1]);
            if c > 1 {
                dg += 2.0 * self.q;
            }
            if c < n {
                dg += 2.0 * self.q;
            }
            irp.push(c as u32 - 1);
            val.push(dg);
            if c < n {
                irp.push(c as u32);
                val.push(-2.0 * self.q);
            }
        }
        jcp.push(irp.len() as u32);
        let n = n as u32;
        Ok(match self.mode {
            0 => BFMatrix::Double(SpMat::from_csc(n, n, &irp, &jcp, &val, 1)?),
            1 => BFMatrix::Float(SpMat::from_csc(n, n, &irp, &jcp, &val, 1)?),
            _ => BFMatrix::Full(SpMat::<f64>::from_csc(n, n, &irp, &jcp, &val, 1)?.as_dense()),
        })
    }
}

fn spmat_ops<T: Wrap>(
    out: &mut String,
    pre: &str,
    a_: &Csc,
    b_: &Csc,
    dmat: &Mat,
    x: &[f64],
    b: &[f64],
) -> Result<(), FslError> {
    let tag = |t: &str| format!("{pre}{t}");
    let a = SpMat::<T>::from_csc(a_.m, a_.n, &a_.irp, &a_.jcp, &a_.val, 1)?;
    let bb = SpMat::<T>::from_csc(b_.m, b_.n, &b_.irp, &b_.jcp, &b_.val, 1)?;
    psp(out, &tag("A"), &a);
    pvec(out, &tag("Ax"), &a.mul_vec(x)?);
    for nt in [2u32, 3, 4, 7] {
        let mut at = a.clone();
        at.set_nthreads(nt);
        pvec(out, &tag(&format!("Ax_nt{nt}")), &at.mul_vec(x)?);
    }
    pvec(out, &tag("Atx"), &a.trans_mult(x)?);
    {
        let mut c = a.clone();
        c.add_assign(&a)?;
        psp(out, &tag("A+A"), &c);
    }
    {
        let mut c = a.clone();
        c.add_assign(&bb)?;
        psp(out, &tag("A+B"), &c);
        c.scale(0.3);
        psp(out, &tag("(A+B)*0.3"), &c);
    }
    {
        let mut c = a.clone();
        c.sub_assign(&bb)?;
        psp(out, &tag("A-=B"), &c);
    }
    {
        let c = bb.scaled(0.7);
        let mut e = a.clone();
        e.add_assign(&c)?;
        psp(out, &tag("A+0.7B"), &e);
    }
    {
        let mut c = bb.clone();
        c.add_assign(&a)?;
        psp(out, &tag("B+A"), &c);
        let mut e = c.clone();
        e.add_assign(&bb)?;
        psp(out, &tag("B+A+B"), &e);
    }
    psp(out, &tag("At"), &a.t(1));
    psp(out, &tag("AtA"), &a.trans_mult_self(1)?);
    psp(out, &tag("A*B"), &SpMat::mul_spmat(&a, &bb, 1)?);
    {
        let mut c = a.clone();
        c.set(1, a_.n, T::from_f64(1.25))?;
        c.add_to(2, 1, T::from_f64(0.5))?;
        c.add_to(a_.m, 2, T::from_f64(-3.0))?;
        psp(out, &tag("set"), &c);
        pd(out, &tag("peek"), c.peek(1, a_.n)?.to_f64());
        pd(out, &tag("peek"), c.peek(a_.m, 1)?.to_f64());
    }
    {
        let mut c = a.clone();
        c.hor_concat(&SpMat::from_dense(&dmat.t(), 1))?;
        psp(out, &tag("A|Dt"), &c);
    }
    {
        let mut c = a.clone();
        c.vert_concat(&SpMat::from_dense(dmat, 1))?;
        psp(out, &tag("A&D"), &c);
    }
    for tol in [1e-3, 1e-10] {
        pvec(
            out,
            &tag("cg"),
            &a.solve_for_x(b, MatrixType::SymPosdef, tol, 500, None, 1)?
                .0,
        );
        pvec(
            out,
            &tag("bicg"),
            &a.solve_for_x(b, MatrixType::Unknown, tol, 500, None, 1)?.0,
        );
    }
    pvec(
        out,
        &tag("cg_it3"),
        &a.solve_for_x(b, MatrixType::SymPosdef, 1e-12, 3, None, 1)?
            .0,
    );
    pvec(
        out,
        &tag("bicg_it3"),
        &a.solve_for_x(b, MatrixType::Asym, 1e-12, 3, None, 1)?.0,
    );
    let mut nb = T::wrap(a.clone());
    let tmp = T::wrap(bb.clone());
    nb.add_to_me(&tmp, 1.0, 1)?;
    nb.add_to_me(&tmp.transpose(1), 1.0, 1)?;
    nb.mul_me_by_scalar(2.0 / 37.0);
    nb.add_to_me(&tmp, 0.001, 1)?;
    pbf(out, &tag("bf"), &nb);
    let inter = dmat.t();
    let mut bfi = BFMatrix::Full(inter);
    let k = dmat.rows();
    let mut mv = Mat::zeros(k, k);
    for i in 0..k {
        mv.set(i, i, 3.0 + (i + 1) as f64);
        if i > 0 {
            mv.set(i, i - 1, 0.25);
            mv.set(i - 1, i, 0.25);
        }
    }
    let mv = BFMatrix::Full(mv);
    nb.vert_concat_below_me(&bfi.transpose(1), 1)?;
    bfi.vert_concat_below_me(&mv, 1)?;
    nb.hor_concat2_my_right(&bfi, 1)?;
    pbf(out, &tag("bfcat"), &nb);
    let bx: Vec<f64> = (1..=nb.ncols() as usize)
        .map(|i| {
            if i <= x.len() {
                x[i - 1]
            } else {
                0.5 * i as f64
            }
        })
        .collect();
    pvec(out, &tag("bfcat*x"), &nb.mul_by_vec(&bx)?);
    pvec(
        out,
        &tag("bfcat\\x"),
        &nb.solve_for_x(&bx, MatrixType::SymPosdef, 1e-8, 200, 1)?.0,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_nonlin(
    out: &mut String,
    tag: &str,
    m: NlMethod,
    mode: i32,
    c: &[f64],
    d: &[f64],
    q: f64,
    maxiter: u32,
    lmset: bool,
) -> Result<(), FslError> {
    let mut cf = TestCf {
        c: c.to_vec(),
        d: d.to_vec(),
        q,
        mode,
    };
    let start = vec![0.0; c.len()];
    let mut np = NonlinParam::new(c.len(), m, &start)?;
    if lmset {
        np.set_equation_solver_max_iter(500);
        np.set_equation_solver_tol(1.0e-3);
    }
    np.set_max_iter(maxiter);
    let st = nonlin(&mut np, &mut cf)?;
    writeln!(out, "{tag} status {} niter {}", st as i32, np.n_iter()).unwrap();
    pd(out, tag, np.cf());
    pd(out, tag, np.lambda());
    pvec(out, tag, np.par());
    Ok(())
}

fn run(dir: &Path, out: &mut String) -> Result<(), FslError> {
    let params = std::fs::read_to_string(dir.join("params.txt")).unwrap();
    let p: Vec<u32> = params
        .split_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
    let (lm_it, scg_it) = (p[0], p[1]);
    let a = read_csc(&dir.join("A.csc"));
    let b_ = read_csc(&dir.join("B.csc"));
    let db = std::fs::read(dir.join("D.bin")).unwrap();
    let h = u32s(&db, 0, 2);
    let (dr, dc) = (h[0] as usize, h[1] as usize);
    let dmat = Mat::from_col_major(dr, dc, &f64s(&db, 8, dr * dc));
    let n = a.n as usize;
    let x = f64s(&std::fs::read(dir.join("x.bin")).unwrap(), 0, n);
    let b = f64s(&std::fs::read(dir.join("b.bin")).unwrap(), 0, n);
    spmat_ops::<f64>(out, "d_", &a, &b_, &dmat, &x, &b)?;
    spmat_ops::<f32>(out, "f_", &a, &b_, &dmat, &x, &b)?;
    let cfb = f64s(&std::fs::read(dir.join("cf.bin")).unwrap(), 0, 2 * n + 1);
    let (c, d, q) = (&cfb[..n], &cfb[n..2 * n], cfb[2 * n]);
    run_nonlin(out, "lm_d", NlMethod::Lm, 0, c, d, q, lm_it, true)?;
    run_nonlin(out, "lm_f", NlMethod::Lm, 1, c, d, q, lm_it, true)?;
    run_nonlin(out, "lm_def", NlMethod::Lm, 0, c, d, q, lm_it, false)?;
    run_nonlin(out, "scg", NlMethod::Scg, 0, c, d, q, scg_it, false)?;
    run_nonlin(out, "scg_short", NlMethod::Scg, 0, c, d, q, 3, false)?;
    if n <= 8 {
        run_nonlin(out, "lm_full", NlMethod::Lm, 2, c, d, q, lm_it, true)?;
        run_nonlin(out, "lm_numeric", NlMethod::Lm, 3, c, d, q, lm_it, true)?;
    }
    Ok(())
}

#[test]
#[ignore]
fn miscmaths_probe() {
    let root = std::env::var("MM_PROBE_ROOT").expect("MM_PROBE_ROOT");
    for e in std::fs::read_dir(&root).unwrap() {
        let dir = e.unwrap().path();
        if !dir.join("params.txt").exists() {
            continue;
        }
        let mut out = String::new();
        if let Err(e) = run(&dir, &mut out) {
            writeln!(out, "exception {}", e.what).unwrap();
        }
        std::fs::write(dir.join("replica.txt"), out).unwrap();
    }
}
