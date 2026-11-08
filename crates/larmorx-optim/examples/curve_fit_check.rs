// SPDX-License-Identifier: Apache-2.0
//! Computes the cases `validation/investigations/t2smap/numerics/check_curve_fit.py` writes
//! (which runs this program and compares the results with SciPy's and numpy's own, bit for
//! bit).
//!
//! `curve_fit_check <cases.bin> <out.bin> <threads>`. `cases.bin`: `u32` count, `u8` SciPy
//! (0: 1.18.1, 1: 1.15.3), `u8` numpy's LAPACK (0: 3.12.0, 1: 3.12.1), then the records, each
//! a `u8` kind and its data (little-endian):
//!
//! - 0, `curve_fit` of tedana's monoexponential `s0 · exp(-x / t2)`: `u32 m`, `f64 x[m]`,
//!   `f64 y[m]`, `f64 p0[2]`, `f64 lb[2]`, `f64 ub[2]`;
//! - 1, `scipy.linalg.svd(a, full_matrices=False)`, 2, `numpy.linalg.svd(a,
//!   full_matrices=False)`: `u32 m`, `f64 a[2m]` column-major;
//! - 3, `numpy.dot(x, y)`: `u32 n`, `f64 x[n]`, `f64 y[n]`;
//! - 4, `a.dot(x)`: `u32 rows`, `u32 cols`, `u8 layout` (0: C, 1: F), `f64 a[rows·cols]` as
//!   in memory, `f64 x[cols]`;
//! - 5, sets the `x` of the kind-6 records: `u32 m`, `f64 x[m]`;
//! - 6, as 0 with the last kind-5 `x` and float32 `y`: `f32 y[m]`, `f64 p0[2]`, `lb[2]`,
//!   `ub[2]`.
//!
//! `out.bin` holds one `u32`-length-prefixed blob per record: for 0 and 6 a `u8` 1 and `popt[2]`,
//! `pcov[4]` (row-major), `u8` warn, `u32` nfev, `i32` status, or a `u8` 0 and the text
//! `Class: message`; for 1 and 2 a `u8` 1 and `u[2m]`, `s[2]`, `vt[4]` (column-major), or a `u8`
//! 0 and the text; for 3 the `f64`; for 4 the `f64 y[rows]`; nothing for 5. The model and the
//! method's `pow` use glibc 2.35's functions (`reproduce`: numpy's float64 `exp` with its
//! AVX-512 kernels off, and C's `pow`).

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::math::Libm;
use larmorx_core::numpy::umath::exp_f64;
use larmorx_optim::{Scipy, curve_fit2};
use larmorx_transform::numpy_linalg::blas::{self, Layout};
use larmorx_transform::numpy_linalg::{LapackRelease, gesdd_s2};

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> &[u8] {
        let s = &self.b[self.at..self.at + n];
        self.at += n;
        s
    }
    fn u8(&mut self) -> u8 {
        self.take(1)[0]
    }
    fn u32(&mut self) -> usize {
        u32::from_le_bytes(self.take(4).try_into().unwrap()) as usize
    }
    fn f64s(&mut self, n: usize) -> Vec<f64> {
        self.take(8 * n)
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| f64::from_le_bytes(*c))
            .collect()
    }
    fn f32s(&mut self, n: usize) -> Vec<f64> {
        self.take(4 * n)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f64::from(f32::from_le_bytes(*c)))
            .collect()
    }
    fn pair(&mut self) -> [f64; 2] {
        let v = self.f64s(2);
        [v[0], v[1]]
    }
}

enum Case {
    Fit {
        x: std::sync::Arc<Vec<f64>>,
        y: Vec<f64>,
        p0: [f64; 2],
        lb: [f64; 2],
        ub: [f64; 2],
    },
    Svd {
        a: Vec<f64>,
        m: usize,
        release: LapackRelease,
    },
    Dot(Vec<f64>, Vec<f64>),
    MatVec {
        a: Vec<f64>,
        rows: usize,
        cols: usize,
        layout: Layout,
        x: Vec<f64>,
    },
    Nothing,
}

fn put_f64s(out: &mut Vec<u8>, v: &[f64]) {
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

fn compute(case: &Case, scipy: Scipy) -> Vec<u8> {
    let mut out = Vec::new();
    match case {
        Case::Fit { x, y, p0, lb, ub } => {
            let libm = Libm::quiet(UpstreamDefects::Reproduce);
            // tedana's model: s0 * np.exp(-tes / t2star).
            let mut model = |x: &[f64], p: &[f64; 2], o: &mut [f64]| {
                for (ok, &xk) in o.iter_mut().zip(x) {
                    *ok = p[0] * exp_f64(-xk / p[1]);
                }
            };
            match curve_fit2(&mut model, x, y, *p0, *lb, *ub, scipy, &libm) {
                Ok(r) => {
                    out.push(1);
                    put_f64s(&mut out, &r.popt);
                    put_f64s(
                        &mut out,
                        &[r.pcov[0][0], r.pcov[0][1], r.pcov[1][0], r.pcov[1][1]],
                    );
                    out.push(u8::from(r.warn_cov));
                    out.extend_from_slice(&(r.nfev as u32).to_le_bytes());
                    out.extend_from_slice(&r.status.to_le_bytes());
                }
                Err(e) => {
                    out.push(0);
                    let (class, msg) = e.python();
                    out.extend_from_slice(format!("{class}: {msg}").as_bytes());
                }
            }
        }
        Case::Svd { a, m, release } => match gesdd_s2(a, *m, *release) {
            Ok(s) => {
                out.push(1);
                put_f64s(&mut out, &s.u);
                put_f64s(&mut out, &s.s);
                put_f64s(&mut out, &s.vt);
            }
            Err(e) => {
                out.push(0);
                let class = match e {
                    larmorx_transform::numpy_linalg::GesddError::DidNotConverge => "LinAlgError",
                    _ => "ValueError",
                };
                out.extend_from_slice(format!("{class}: {e}").as_bytes());
            }
        },
        Case::Dot(x, y) => put_f64s(&mut out, &[blas::dot(x, y)]),
        Case::MatVec {
            a,
            rows,
            cols,
            layout,
            x,
        } => put_f64s(&mut out, &blas::matvec(a, *rows, *cols, *layout, x)),
        Case::Nothing => {}
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).expect("cases");
    let threads: usize = args.get(3).map_or(1, |t| t.parse().unwrap());
    let mut r = Reader { b: &bytes, at: 0 };
    let count = r.u32();
    let scipy = if r.u8() == 0 {
        Scipy::V1_18_1
    } else {
        Scipy::V1_15_3
    };
    let numpy_release = if r.u8() == 0 {
        LapackRelease::V3_12_0
    } else {
        LapackRelease::V3_12_1
    };
    let mut cases = Vec::with_capacity(count);
    let mut current_x = std::sync::Arc::new(Vec::new());
    for _ in 0..count {
        let kind = r.u8();
        cases.push(match kind {
            0 => {
                let m = r.u32();
                let x = std::sync::Arc::new(r.f64s(m));
                let y = r.f64s(m);
                Case::Fit {
                    x,
                    y,
                    p0: r.pair(),
                    lb: r.pair(),
                    ub: r.pair(),
                }
            }
            1 | 2 => {
                let m = r.u32();
                Case::Svd {
                    a: r.f64s(2 * m),
                    m,
                    release: if kind == 1 {
                        LapackRelease::V3_12_0
                    } else {
                        numpy_release
                    },
                }
            }
            3 => {
                let n = r.u32();
                Case::Dot(r.f64s(n), r.f64s(n))
            }
            4 => {
                let rows = r.u32();
                let cols = r.u32();
                let layout = if r.u8() == 0 { Layout::C } else { Layout::F };
                Case::MatVec {
                    a: r.f64s(rows * cols),
                    rows,
                    cols,
                    layout,
                    x: r.f64s(cols),
                }
            }
            5 => {
                let m = r.u32();
                current_x = std::sync::Arc::new(r.f64s(m));
                Case::Nothing
            }
            6 => {
                let m = current_x.len();
                Case::Fit {
                    x: current_x.clone(),
                    y: r.f32s(m),
                    p0: r.pair(),
                    lb: r.pair(),
                    ub: r.pair(),
                }
            }
            k => panic!("kind {k}"),
        });
    }
    let t0 = std::time::Instant::now();
    let chunk = cases.len().div_ceil(threads.max(1)).max(1);
    let results: Vec<Vec<u8>> = std::thread::scope(|sc| {
        let handles: Vec<_> = cases
            .chunks(chunk)
            .map(|c| sc.spawn(move || c.iter().map(|k| compute(k, scipy)).collect::<Vec<_>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    let mut out = Vec::new();
    for blob in &results {
        out.extend_from_slice(&(blob.len() as u32).to_le_bytes());
        out.extend_from_slice(blob);
    }
    std::fs::write(&args[2], out).expect("write");
    eprintln!(
        "curve_fit_check: {} records in {:.2} s ({threads} threads)",
        cases.len(),
        t0.elapsed().as_secs_f64()
    );
}
