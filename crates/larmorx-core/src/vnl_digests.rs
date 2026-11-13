// SPDX-License-Identifier: Apache-2.0
//! Cross-platform checks of the VNL and netlib ports of FreeSurfer's foundations (item 3.1-07):
//! digests of their outputs on seeded inputs, recorded on Linux x86-64 (where
//! `validation/probes/fs-vnl` proves the ports bit-identical to ITK 4.13.2's VNL built by GCC
//! 8.4.0), checked on every platform by `cargo test`. Inputs are SplitMix64 streams and integer
//! arithmetic; the cost functions use plain arithmetic and `larmorx_core::math`'s pure-Rust
//! functions, the same on every platform; a NaN is hashed as one value (its sign and payload are
//! not defined by the source where two NaNs meet). `LARMORX_PRINT_DIGESTS=1` prints the values
//! to record.

use crate::cfmt::{Captured, Silent, Stream};
use crate::defects::UpstreamDefects;
use crate::vnl_cholesky::{Mode, VnlCholesky};
use crate::vnl_lbfgs::{LbfgsGlobal, lbfgs};
use crate::vnl_minimize::{Brent, BrentMinimizer, Powell, bracket_minimum};
use crate::vnl_qr::Qr;
use crate::x87::cases::SplitMix64;
use crate::{math, vnl_eigen, vnl_fixed, vnl_svd, vnl_svd_f32};

const FNV0: u64 = 0xcbf2_9ce4_8422_2325;

struct Digest(u64);

impl Digest {
    fn word(&mut self, w: u64) {
        for b in w.to_le_bytes() {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    /// A NaN counts as one value: where two NaNs meet, which one an operation passes on is the
    /// compiler's choice of operand order (LLVM's differs between debug and release builds, and
    /// from GCC's), which the source leaves open.
    fn f64s(&mut self, v: &[f64]) {
        for x in v {
            self.word(if x.is_nan() {
                0x7ff8_0000_0000_0000
            } else {
                x.to_bits()
            });
        }
    }
    fn f32s(&mut self, v: &[f32]) {
        for x in v {
            self.word(if x.is_nan() {
                0x7fc0_0000
            } else {
                u64::from(x.to_bits())
            });
        }
    }
    fn text(&mut self, s: &str) {
        for b in s.bytes() {
            self.word(u64::from(b));
        }
    }
}

fn unit(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

fn ten_to(k: i64) -> f64 {
    if k >= 0 {
        math::powi(10.0, k as u32)
    } else {
        1.0 / math::powi(10.0, (-k) as u32)
    }
}

/// A 1-D or n-D test function (smooth, kinked, stepped, oscillating), recording its points.
struct Problem {
    c: Vec<f64>,
    s: Vec<f64>,
    kind: Vec<u8>,
    trace: Vec<f64>,
}

impl Problem {
    fn new(rng: &mut SplitMix64, n: usize) -> Problem {
        Problem {
            c: (0..n)
                .map(|_| (unit(rng) - 0.5) * ten_to(rng.range(-2, 2)))
                .collect(),
            s: (0..n)
                .map(|_| ten_to(rng.range(-2, 2)) * (0.5 + unit(rng)))
                .collect(),
            kind: (0..n).map(|_| rng.below(5) as u8).collect(),
            trace: Vec::new(),
        }
    }

    fn eval(&mut self, x: &[f64]) -> f64 {
        self.trace.extend_from_slice(x);
        let mut f = 0.0;
        for i in 0..x.len() {
            let d = x[i] - self.c[i] + if i > 0 { 0.2 * x[i - 1] } else { 0.0 };
            f += match self.kind[i] {
                0 => self.s[i] * d * d,
                1 => self.s[i] * d.abs(),
                2 => self.s[i] * d * d + 1e-3 * math::cos(37.0 * d),
                3 => self.s[i] * (d * 8.0).floor().abs(),
                _ => self.s[i] * (1.0 - math::exp(-d * d)),
            };
        }
        f
    }
}

fn digest_minimizers() -> u64 {
    let mut h = Digest(FNV0);
    let mut rng = SplitMix64::new(0x5eed_0301);
    for _ in 0..120 {
        let seed = rng.next_u64();
        let a = rng.range(-3000, 3000) as f64 / 1000.0;
        let b = a + rng.range(-2000, 2000) as f64 / 997.0;
        let xtol = [1e-6, 1e-4, 1e-8][rng.below(3) as usize];
        let mut p = Problem::new(&mut SplitMix64::new(seed), 1);
        let mut f = |x: &[f64]| p.eval(x);
        let br = bracket_minimum(&mut f, a, b);
        let mut bm = BrentMinimizer::new();
        bm.xtol = xtol;
        let x0 = bm.minimize_given_bounds(&mut f, br.a, br.b, br.c);
        let x1 = bm.minimize_given_bounds_and_all_f(&mut f, br.a, br.b, br.c, br.fa, br.fb, br.fc);
        let x2 = BrentMinimizer::new().minimize(&mut f, a);
        let mut brent = Brent::new();
        let ((ax, xx, bx), _) = brent.bracket_minimum(&mut f, 0.0, 1.0, 0.0);
        let (fret, xmin) = brent.minimize_given_bounds(&mut f, bx, xx, ax, xtol);
        let x3 = Brent::new().minimize(&mut f, a);
        h.f64s(&[
            br.a,
            br.b,
            br.c,
            br.fa,
            br.fb,
            br.fc,
            x0,
            x1,
            x2,
            ax,
            xx,
            bx,
            fret,
            xmin,
            x3,
            bm.f_at_last_minimum,
        ]);
        h.f64s(&p.trace);
        let n = 1 + rng.below(4) as usize;
        let mut p = Problem::new(&mut SplitMix64::new(seed ^ 1), n);
        let mut pt: Vec<f64> = (0..n)
            .map(|_| rng.range(-2000, 2000) as f64 / 1000.0)
            .collect();
        let mut pw = Powell::new();
        pw.base.verbose = true;
        let mut out = Captured::default();
        let code = pw.minimize(&mut |x: &[f64]| p.eval(x), &mut pt, &mut out);
        h.f64s(&pt);
        h.word(code.value() as u64);
        h.word(u64::from(pw.base.num_iterations));
        h.word(pw.base.num_evaluations as u64);
        h.f64s(&p.trace);
        h.text(&out.text(Stream::Err));
    }
    h.0
}

/// A cost that falls without end (linear, quadratic, cubic), or jumps to an infinity or to NaNs
/// of its own beyond a threshold, its own arithmetic with SSE's NaNs (`math::x86_nan`), NaN after
/// 20,000 calls (a search that overflows can creep through 10^8 points); records every point's
/// bits, NaNs included.
struct Falling {
    kind: u64,
    dir: Vec<f64>,
    centre: f64,
    scale: f64,
    threshold: f64,
    calls: u64,
    trace: Vec<u64>,
}

impl Falling {
    fn new(rng: &mut SplitMix64, n: usize) -> Falling {
        Falling {
            kind: rng.below(7),
            dir: (0..n)
                .map(|i| if i == 0 { 1.0 } else { (unit(rng) - 0.5) * 0.8 })
                .collect(),
            centre: (unit(rng) - 0.5) * ten_to(rng.range(-2, 3)),
            scale: ten_to(rng.range(-3, 4))
                * (0.5 + unit(rng))
                * if rng.below(2) == 0 { 1.0 } else { -1.0 },
            threshold: ten_to(rng.range(0, 40)),
            calls: 0,
            trace: Vec::new(),
        }
    }

    fn eval(&mut self, x: &[f64]) -> f64 {
        use math::x86_nan::{add_f64 as add, mul_f64 as mul, sub_f64 as sub};
        self.calls += 1;
        self.trace.extend(x.iter().map(|v| v.to_bits()));
        if self.calls > 20_000 {
            return f64::NAN;
        }
        let mut y = 0.0;
        for (xi, di) in x.iter().zip(&self.dir) {
            y = add(y, mul(*xi, *di));
        }
        let d = sub(y, self.centre);
        let s = self.scale;
        let own_nan = f64::from_bits(
            0x7ff8_0000_0000_0000
                | (self.calls & 0xffff)
                | if self.calls & 1 == 1 { 1 << 63 } else { 0 },
        );
        let beyond = d.abs() > self.threshold;
        match self.kind {
            0 => mul(-s, d),
            1 => mul(mul(-s.abs(), d), d),
            2 => mul(mul(mul(s, d), d), d),
            3 if beyond => f64::NEG_INFINITY,
            4 if beyond => own_nan,
            5 if beyond => f64::INFINITY,
            5 => mul(mul(-s.abs(), d), d),
            _ => mul(-s, d),
        }
    }
}

/// An infinity, a NaN (either sign, with or without a payload) or a value near overflow.
fn special(rng: &mut SplitMix64) -> f64 {
    const S: [u64; 8] = [
        0x7ff0_0000_0000_0000,
        0xfff0_0000_0000_0000,
        0x7ff8_0000_0000_0000,
        0xfff8_0000_0000_0000,
        0x7ff8_0000_0000_0042,
        0xfff8_0000_0000_0777,
        0x7fe0_0000_0000_0000,
        0xffe1_0000_0000_0000,
    ];
    f64::from_bits(S[rng.below(8) as usize])
}

/// The minimizers on [`Falling`] costs: every point and result bit for bit, NaNs included (the
/// minimizers' NaNs are SSE's on every platform), and the ITK-114 stops. Returns the digest, the
/// NaN points and results met, and the stops.
fn digest_unbounded() -> (u64, u64, u64) {
    let mut h = Digest(FNV0);
    let (mut nan_values, mut stops) = (0u64, 0u64);
    let is_nan = |w: &u64| f64::from_bits(*w).is_nan();
    let mut rng = SplitMix64::new(0x5eed_0399);
    let nans = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<u64>>();
    for _ in 0..150 {
        let seed = rng.next_u64();
        let mut a = rng.range(-4000, 4000) as f64 / 1000.0 * ten_to(rng.range(0, 6));
        let mut b = if rng.below(8) == 0 {
            a
        } else {
            a + rng.range(-3000, 3000) as f64 / 997.0
        };
        if rng.below(5) == 0 {
            a = special(&mut rng);
        }
        if rng.below(5) == 0 {
            b = special(&mut rng);
        }
        let xtol = [1e-6, 1e-4, 1e-8][rng.below(3) as usize];
        let fresh = |n: usize| Falling::new(&mut SplitMix64::new(seed), n);
        let mut p = fresh(1);
        let br = bracket_minimum(&mut |x: &[f64]| p.eval(x), a, b);
        let mut bm = BrentMinimizer::new();
        bm.xtol = xtol;
        let x0 = bm.minimize_given_bounds_and_all_f(
            &mut |x: &[f64]| p.eval(x),
            br.a,
            br.b,
            br.c,
            br.fa,
            br.fb,
            br.fc,
        );
        let x1 = BrentMinimizer::new().minimize(&mut |x: &[f64]| p.eval(x), a);
        let mut brent = Brent::new();
        let x2 = brent.minimize(&mut |x: &[f64]| p.eval(x), a);
        let results = nans(&[
            br.a,
            br.b,
            br.c,
            br.fa,
            br.fb,
            br.fc,
            x0,
            x1,
            x2,
            bm.f_at_last_minimum,
        ]);
        nan_values += results.iter().filter(|w| is_nan(w)).count() as u64;
        for w in results {
            h.word(w);
        }
        h.word(bm.notices.len() as u64 + 2 * brent.inner.notices.len() as u64);
        stops += (bm.notices.len() + brent.inner.notices.len()) as u64;
        nan_values += p.trace.iter().filter(|w| is_nan(w)).count() as u64;
        for &w in &p.trace {
            h.word(w);
        }
        let n = 1 + rng.below(3) as usize;
        let mut q = fresh(n);
        let mut pt: Vec<f64> = (0..n)
            .map(|_| rng.range(-2000, 2000) as f64 / 1000.0)
            .collect();
        if rng.below(4) == 0 {
            pt[0] = special(&mut rng);
        }
        let mut pw = Powell::new();
        pw.base.maxfev = [3, 10, 40][rng.below(3) as usize];
        let code = pw.minimize(&mut |x: &[f64]| q.eval(x), &mut pt, &mut Silent);
        for w in nans(&pt) {
            h.word(w);
        }
        for w in nans(&[pw.base.start_error, pw.base.end_error]) {
            h.word(w);
        }
        h.word(code.value() as u64);
        h.word(pw.base.num_evaluations as u64);
        h.word(pw.notices.len() as u64);
        stops += pw.notices.len() as u64;
        nan_values += q.trace.iter().filter(|w| is_nan(w)).count() as u64;
        for &w in &q.trace {
            h.word(w);
        }
    }
    (h.0, nan_values, stops)
}

fn digest_lbfgs() -> u64 {
    let mut h = Digest(FNV0);
    let mut rng = SplitMix64::new(0x5eed_0302);
    for case in 0..60 {
        let n = 1 + rng.below(8) as usize;
        let c: Vec<f64> = (0..n).map(|_| (unit(&mut rng) - 0.5) * 4.0).collect();
        let rosen = rng.below(2) == 0;
        let fg = |x: &[f64], g: &mut [f64]| -> f64 {
            let mut f = 0.0;
            g.iter_mut().for_each(|v| *v = 0.0);
            if rosen {
                for i in 0..n {
                    let d = x[i] - c[i];
                    f += d * d * d * d + 0.5 * d * d;
                    g[i] += 4.0 * d * d * d + d;
                }
                for i in 0..n.saturating_sub(1) {
                    let t = x[i + 1] - x[i] * x[i];
                    f += 10.0 * t * t;
                    g[i] += -40.0 * x[i] * t;
                    g[i + 1] += 20.0 * t;
                }
            } else {
                let mut s = 1.0;
                for i in 0..n {
                    s += (x[i] - c[i]) * (x[i] - c[i]);
                }
                f = math::log_glibc(s);
                for i in 0..n {
                    f += 0.01 * x[i] * x[i];
                    g[i] = 2.0 * (x[i] - c[i]) / s + 0.02 * x[i];
                }
            }
            f
        };
        let m = [7i64, 3, 5][rng.below(3) as usize];
        let gtol = [0.1, 0.9][rng.below(2) as usize];
        let x0: Vec<f64> = (0..n)
            .map(|_| rng.range(-3000, 3000) as f64 / 997.0)
            .collect();
        for (mode, nocedal) in [
            (UpstreamDefects::Fix, false),
            (UpstreamDefects::Reproduce, false),
            (UpstreamDefects::Fix, true),
        ] {
            let mut x = x0.clone();
            let mut g = vec![0.0; n];
            let mut diag = vec![0.0; n];
            let mut w = vec![0.0; n * (2 * m as usize + 1) + 2 * m as usize];
            let mut gl = LbfgsGlobal::new();
            gl.gtol = gtol;
            gl.nocedal_first_stage = nocedal;
            let mut iflag = 0i64;
            let iprint = if case % 7 == 0 { [1, 1] } else { [-1, 0] };
            let mut out = Captured::default();
            for _ in 0..200 {
                let f = fg(&x, &mut g);
                lbfgs(
                    n as i64, m, &mut x, f, &g, false, &mut diag, iprint, 1e-5, 1e-16, &mut w,
                    &mut iflag, &mut gl, mode, &mut out,
                );
                if iflag <= 0 {
                    break;
                }
            }
            h.f64s(&x);
            h.word(iflag as u64);
            h.word(gl.nfun as u64);
            h.text(&out.text(Stream::Out));
        }
    }
    h.0
}

fn matrix(rng: &mut SplitMix64, r: usize, c: usize) -> Vec<f64> {
    let family = rng.below(4);
    let scale = ten_to(rng.range(-3, 3));
    (0..r * c)
        .map(|_| match family {
            0 => (unit(rng) - 0.5) * scale,
            1 => rng.range(-3, 3) as f64,
            2 => {
                if rng.below(3) == 0 {
                    (unit(rng) - 0.5) * 5.0
                } else {
                    0.0
                }
            }
            _ => (unit(rng) - 0.5) as f32 as f64,
        })
        .collect()
}

fn digest_linalg() -> u64 {
    let mut h = Digest(FNV0);
    let mut rng = SplitMix64::new(0x5eed_0303);
    for _ in 0..300 {
        // Small fixed matrices and the QR determinant.
        let n = 2 + rng.below(3) as usize;
        let m = matrix(&mut rng, n, n);
        let mf: Vec<f32> = m.iter().map(|&v| v as f32).collect();
        h.f32s(&[vnl_fixed::determinant(&mf, n)]);
        h.f64s(&[vnl_fixed::determinant(&m, n)]);
        h.f32s(&vnl_fixed::inverse(&mf, n));
        h.f64s(&vnl_fixed::inverse(&m, n));
        let n = 5 + rng.below(4) as usize;
        let m = matrix(&mut rng, n, n);
        let mf: Vec<f32> = m.iter().map(|&v| v as f32).collect();
        for balance in [false, true] {
            h.f32s(&[vnl_fixed::determinant_balanced(&mf, n, balance)]);
            h.f64s(&[vnl_fixed::determinant_balanced(&m, n, balance)]);
        }
        // QR.
        let (r, c) = (1 + rng.below(7) as usize, 1 + rng.below(7) as usize);
        let m = matrix(&mut rng, r, c);
        let mf: Vec<f32> = m.iter().map(|&v| v as f32).collect();
        let b: Vec<f64> = (0..r).map(|_| rng.range(-100, 100) as f64 / 8.0).collect();
        let bf: Vec<f32> = b.iter().map(|&v| v as f32).collect();
        let q32 = Qr::new(&mf, r, c);
        let q64 = Qr::new(&m, r, c);
        h.f32s(&q32.q());
        h.f32s(&q32.r());
        h.f32s(&[q32.determinant()]);
        h.f32s(&q32.qtb(&bf));
        h.f64s(&q64.q());
        h.f64s(&q64.r());
        h.f64s(&[q64.determinant()]);
        h.f64s(&q64.qtb(&b));
        if r >= c {
            h.f32s(&q32.solve(&bf).0);
            h.f64s(&q64.solve(&b).0);
        }
        // The float SVD, the SVD inverse.
        let (r, c) = (r.max(c), r.min(c));
        let m = matrix(&mut rng, r, c);
        let mf: Vec<f32> = m.iter().map(|&v| v as f32).collect();
        if let Some((u, w, v)) = vnl_svd_f32::vnl_svd_f32(&mf, r, c) {
            h.f32s(&u);
            h.f32s(&w);
            h.f32s(&v);
        }
        let n = 1 + rng.below(5) as usize;
        let m = matrix(&mut rng, n, n);
        if let Some(inv) = vnl_svd::vnl_inverse(&m, n) {
            h.f64s(&inv);
        }
        // Eigensystems.
        let mut s = matrix(&mut rng, n, n);
        for i in 0..n {
            for j in 0..i {
                s[j * n + i] = s[i * n + j];
            }
        }
        let sf: Vec<f32> = s.iter().map(|&v| v as f32).collect();
        let (vals, vecs, _) = vnl_eigen::symmetric_eigensystem_f32(&sf, n);
        h.f32s(&vals);
        h.f32s(&vecs);
        let es = vnl_eigen::real_eigensystem(&m, n).unwrap();
        h.f64s(&es.d_re);
        h.f64s(&es.d_im);
        h.f64s(&es.v_re);
        h.f64s(&es.v_im);
        h.f64s(&es.vreal);
        // Cholesky of M Mᵀ + 1e-3 I (and of the raw matrix).
        let mut spd = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                spd[i * n + j] = (0..n)
                    .map(|k| m[i * n + k] * m[j * n + k])
                    .fold(0.0, |a, v| a + v)
                    + if i == j { 1e-3 } else { 0.0 };
            }
        }
        for (mat, mode) in [(&spd, Mode::EstimateCondition), (&m, Mode::Verbose)] {
            let mut out = Captured::default();
            let mut ch = VnlCholesky::new(mat, n, mode, &mut out);
            h.word(ch.rank_deficiency() as u64);
            h.f64s(&ch.upper_triangle());
            h.f64s(
                &ch.solve(
                    &b[..n.min(b.len())]
                        .iter()
                        .copied()
                        .chain(std::iter::repeat(1.0))
                        .take(n)
                        .collect::<Vec<_>>(),
                ),
            );
            h.f64s(&[ch.determinant(), ch.rcond().unwrap_or(-1.0)]);
            if let Some(inv) = ch.inverse(&mut Silent) {
                h.f64s(&inv);
            }
            h.text(&out.text(Stream::Err));
        }
    }
    h.0
}

/// The digests recorded on Linux x86-64 (2026-11-12, from this module's own code at the commit
/// that added it; `validation/probes/fs-vnl` proved the ports bit-identical to ITK 4.13.2's VNL).
const MINIMIZERS: u64 = 0x8ef6_0bae_2f47_905c;
const LBFGS: u64 = 0xa7dc_aba7_d3f5_47ed;
const LINALG: u64 = 0xce29_4ebf_05c7_3ab3;

#[test]
fn the_ports_give_linux_x86_64s_bits() {
    let got = [digest_minimizers(), digest_lbfgs(), digest_linalg()];
    if std::env::var_os("LARMORX_PRINT_DIGESTS").is_some() {
        println!(
            "MINIMIZERS = {:#018x}\nLBFGS = {:#018x}\nLINALG = {:#018x}",
            got[0], got[1], got[2]
        );
    }
    assert_eq!(got, [MINIMIZERS, LBFGS, LINALG]);
}

/// The minimizers on costs that fall without end, NaNs compared bit for bit (recorded on Linux
/// x86-64, 2026-11-13; the probe's `unbounded` family proved the port's points and results
/// identical to ITK 4.13.2's VNL there, NaNs included, and identical under `nan_trap.c`'s AArch64
/// emulation of the port).
const UNBOUNDED: u64 = 0x0191_bc98_6239_7c69;

#[test]
fn the_unbounded_minimizers_give_x86_64s_nans() {
    let (got, nan_values, stops) = digest_unbounded();
    if std::env::var_os("LARMORX_PRINT_DIGESTS").is_some() {
        println!("UNBOUNDED = {got:#018x} ({nan_values} NaN values, {stops} ITK-114 stops)");
    }
    assert_eq!(got, UNBOUNDED);
    // What the digest covers (14,039 NaN values, 15 stops when recorded).
    assert!(nan_values > 10_000 && stops > 10, "{nan_values} {stops}");
}
