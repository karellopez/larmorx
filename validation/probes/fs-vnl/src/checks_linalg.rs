// SPDX-License-Identifier: Apache-2.0
//! The linear algebra FreeSurfer calls through VNL (numerics.cpp's `Open*` wrappers,
//! `MatrixSqrt`, `sc_linalg_cholesky_decomp`, `mri_robust_register`), and `vnl_fft_1d<double>`.

use std::ffi::c_int;

use larmorx_core::cfmt::{Captured, Stream};
use larmorx_core::vnl_cholesky::{Mode, VnlCholesky};
use larmorx_core::vnl_eigen;
use larmorx_core::vnl_fixed;
use larmorx_core::vnl_qr::{Qr, RANK_DEFICIENT};
use larmorx_core::vnl_svd;
use larmorx_core::vnl_svd_f32;
use larmorx_core::x87::cases::SplitMix64;
use larmorx_image::vnl_fft::VnlFft1d;

use crate::ffi::Capture;
use crate::{Lib, Tally};

/// A seeded `r × c` matrix of one of the generator's families.
fn matrix(rng: &mut SplitMix64, r: usize, c: usize, family: u64) -> Vec<f64> {
    let u = |rng: &mut SplitMix64| (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    let mut m = vec![0.0f64; r * c];
    match family {
        // Uniform at a random scale.
        0 => {
            let s = 10f64.powi(rng.range(-3, 3) as i32);
            m.iter_mut().for_each(|v| *v = (u(rng) - 0.5) * s);
        }
        // Small integers (exact cancellations, singular matrices).
        1 => m.iter_mut().for_each(|v| *v = rng.range(-3, 3) as f64),
        // Rank-deficient plus tiny noise.
        2 => {
            let a: Vec<f64> = (0..r).map(|_| u(rng) - 0.5).collect();
            let b: Vec<f64> = (0..c).map(|_| u(rng) - 0.5).collect();
            for i in 0..r {
                for j in 0..c {
                    m[i * c + j] = a[i] * b[j] + if rng.below(2) == 0 { 0.0 } else { (u(rng) - 0.5) * 1e-12 };
                }
            }
        }
        // Sparse: zero rows and columns, a diagonal, signed zeros.
        3 => m.iter_mut().for_each(|v| {
            *v = match rng.below(5) {
                0 => -0.0,
                1 => (u(rng) - 0.5) * 7.0,
                _ => 0.0,
            }
        }),
        // An affine voxel-to-RAS-like matrix (float-like entries, last row 0 0 0 1).
        4 => {
            for i in 0..r {
                for j in 0..c {
                    m[i * c + j] = ((u(rng) - 0.5) * 2.0) as f32 as f64 * if i == j { 1.5 } else { 0.4 };
                }
            }
            if r == c && r >= 3 {
                for j in 0..c {
                    m[(r - 1) * c + j] = if j == c - 1 { 1.0 } else { 0.0 };
                }
                m[(r - 1) * c..].iter_mut().take(c - 1).for_each(|v| *v = 0.0);
            }
        }
        // A rotation-like block (complex eigenvalues) with noise.
        5 => {
            for i in 0..r {
                for j in 0..c {
                    m[i * c + j] = (u(rng) - 0.5) * 0.2;
                }
            }
            for i in (0..r.min(c)).step_by(2) {
                if i + 1 < r.min(c) {
                    let (a, b) = (u(rng), u(rng));
                    m[i * c + i] += a;
                    m[i * c + i + 1] -= b;
                    m[(i + 1) * c + i] += b;
                    m[(i + 1) * c + i + 1] += a;
                }
            }
        }
        // Wide dynamic range.
        _ => m.iter_mut().for_each(|v| *v = (u(rng) - 0.5) * 10f64.powi(rng.range(-30, 30) as i32)),
    }
    m
}

fn symmetrize(m: &mut [f64], n: usize, spd: bool) {
    for i in 0..n {
        for j in 0..i {
            m[j * n + i] = m[i * n + j];
        }
    }
    if spd {
        // M Mᵀ + shift.
        let a = m.to_vec();
        for i in 0..n {
            for j in 0..n {
                let mut s = 0.0;
                for k in 0..n {
                    s += a[i * n + k] * a[j * n + k];
                }
                m[i * n + j] = s + if i == j { 1e-3 } else { 0.0 };
            }
        }
    }
}

fn f32s(m: &[f64]) -> Vec<f32> {
    m.iter().map(|&v| v as f32).collect()
}

fn col_to_row<T: Copy + Default>(cm: &[T], rows: usize, cols: usize) -> Vec<T> {
    let mut out = vec![T::default(); rows * cols];
    for i in 0..rows {
        for j in 0..cols {
            out[i * cols + j] = cm[i + rows * j];
        }
    }
    out
}

/// The message after VNL's `__FILE__` prefix (the build's own path to the header).
fn after_modules(s: &str) -> String {
    s.lines()
        .map(|l| match l.find("Modules/") {
            Some(i) => l[i..].to_owned(),
            None => l.to_owned(),
        })
        .map(|l| l + "\n")
        .collect()
}

pub fn run(lib: &Lib, scale: u64, t: &mut Tally) {
    let mut rng = SplitMix64::new(0x11a1_5eed);
    let mut capture = Capture::new(&std::env::temp_dir());
    let mut skipped_singular = 0u64;
    let mut vreal_unwritten = 0u64;
    let mut one_row_zero = 0u64;
    for case in 0..(20_000 * scale) {
        t.cases += 1;
        let family = rng.below(7);
        let w = |s: &str| format!("case {case} family {family} {s}");

        // Small fixed matrices: vnl_det and vnl_inverse, float and double.
        let n = 2 + rng.below(3) as usize;
        let m = matrix(&mut rng, n, n, family);
        let mf = f32s(&m);
        let dc32 = unsafe { (lib.det_fixed_f32)(n as c_int, mf.as_ptr()) };
        let dc64 = unsafe { (lib.det_fixed_f64)(n as c_int, m.as_ptr()) };
        let (dr32, dr64, ir32, ir64): (f32, f64, Vec<f32>, Vec<f64>) = (
            vnl_fixed::determinant(&mf, n),
            vnl_fixed::determinant(&m, n),
            vnl_fixed::inverse(&mf, n),
            vnl_fixed::inverse(&m, n),
        );
        t.f32s(&|| w(&format!("det{n} f32")), &[dr32], &[dc32]);
        t.f64s(&|| w(&format!("det{n} f64")), &[dr64], &[dc64]);
        if dc32 != 0.0 {
            let mut ic = vec![0.0f32; n * n];
            unsafe { (lib.inverse_fixed_f32)(n as c_int, mf.as_ptr(), ic.as_mut_ptr()) };
            t.f32s(&|| w(&format!("inverse{n} f32")), &ir32, &ic);
        } else {
            skipped_singular += 1;
        }
        if dc64 != 0.0 {
            let mut ic = vec![0.0f64; n * n];
            unsafe { (lib.inverse_fixed_f64)(n as c_int, m.as_ptr(), ic.as_mut_ptr()) };
            t.f64s(&|| w(&format!("inverse{n} f64")), &ir64, &ic);
        } else {
            skipped_singular += 1;
        }

        // vnl_determinant above 4 x 4 (QR), plain and balanced.
        let n = 1 + rng.below(10) as usize;
        let m = matrix(&mut rng, n, n, family);
        let mf = f32s(&m);
        for balance in [false, true] {
            let c32 = unsafe { (lib.determinant_f32)(n as c_int, mf.as_ptr(), c_int::from(balance)) };
            let c64 = unsafe { (lib.determinant_f64)(n as c_int, m.as_ptr(), c_int::from(balance)) };
            t.f32s(&|| w(&format!("determinant n={n} balance={balance} f32")), &[vnl_fixed::determinant_balanced(&mf, n, balance)], &[c32]);
            t.f64s(&|| w(&format!("determinant n={n} balance={balance} f64")), &[vnl_fixed::determinant_balanced(&m, n, balance)], &[c64]);
        }

        // QR in float and double: Q, R, determinant, solve (with its message), QtB, inverse.
        let (r, c) = (1 + rng.below(9) as usize, 1 + rng.below(9) as usize);
        let m = matrix(&mut rng, r, c, family);
        let mf = f32s(&m);
        let b: Vec<f64> = (0..r).map(|_| rng.range(-1000, 1000) as f64 / 64.0).collect();
        let bf = f32s(&b);
        {
            let qr = Qr::new(&mf, r, c);
            let (mut q, mut rr) = (vec![0.0f32; r * r], vec![0.0f32; r * c]);
            let det = unsafe { (lib.qr_f32)(r as c_int, c as c_int, mf.as_ptr(), q.as_mut_ptr(), rr.as_mut_ptr()) };
            t.f32s(&|| w(&format!("qr {r}x{c} f32 Q")), &qr.q(), &q);
            t.f32s(&|| w(&format!("qr {r}x{c} f32 R")), &qr.r(), &rr);
            t.f32s(&|| w(&format!("qr {r}x{c} f32 det")), &[qr.determinant()], &[det]);
            let mut qtb = vec![0.0f32; r];
            unsafe { (lib.qr_qtb_f32)(r as c_int, c as c_int, mf.as_ptr(), bf.as_ptr(), qtb.as_mut_ptr()) };
            t.f32s(&|| w(&format!("qr {r}x{c} f32 QtB")), &qr.qtb(&bf), &qtb);
            if r >= c {
                let mut x = vec![0.0f32; c];
                let (_, _, err) = capture.run(lib, || unsafe {
                    (lib.qr_solve_f32)(r as c_int, c as c_int, mf.as_ptr(), bf.as_ptr(), x.as_mut_ptr())
                });
                let (xr, info) = qr.solve(&bf);
                if r == 1 && info == 1 {
                    // One row whose entry is 0: VNL never writes the solution (ITK-113).
                    one_row_zero += 1;
                } else {
                    t.f32s(&|| w(&format!("qr {r}x{c} f32 solve")), &xr, &x);
                }
                let msg = if info > 0 { format!("{RANK_DEFICIENT}{info}\n") } else { String::new() };
                t.text(&|| w(&format!("qr {r}x{c} f32 solve message")), &msg, &after_modules(&err));
            }
            if r == c {
                let mut inv = vec![0.0f32; r * r];
                let _ = capture.run(lib, || unsafe { (lib.qr_inverse_f32)(r as c_int, mf.as_ptr(), inv.as_mut_ptr()) });
                if !(r == 1 && qr.inverse().1[0] == 1) {
                    t.f32s(&|| w(&format!("qr {r}x{c} f32 inverse")), &qr.inverse().0, &inv);
                }
            }
        }
        {
            let qr = Qr::new(&m, r, c);
            let (mut q, mut rr) = (vec![0.0f64; r * r], vec![0.0f64; r * c]);
            let det = unsafe { (lib.qr_f64)(r as c_int, c as c_int, m.as_ptr(), q.as_mut_ptr(), rr.as_mut_ptr()) };
            t.f64s(&|| w(&format!("qr {r}x{c} f64 Q")), &qr.q(), &q);
            t.f64s(&|| w(&format!("qr {r}x{c} f64 R")), &qr.r(), &rr);
            t.f64s(&|| w(&format!("qr {r}x{c} f64 det")), &[qr.determinant()], &[det]);
            let mut qtb = vec![0.0f64; r];
            unsafe { (lib.qr_qtb_f64)(r as c_int, c as c_int, m.as_ptr(), b.as_ptr(), qtb.as_mut_ptr()) };
            t.f64s(&|| w(&format!("qr {r}x{c} f64 QtB")), &qr.qtb(&b), &qtb);
            if r >= c {
                let mut x = vec![0.0f64; c];
                let (_, _, err) = capture.run(lib, || unsafe {
                    (lib.qr_solve_f64)(r as c_int, c as c_int, m.as_ptr(), b.as_ptr(), x.as_mut_ptr())
                });
                let (xr, info) = qr.solve(&b);
                if r == 1 && info == 1 {
                    // One row whose entry is 0: VNL never writes the solution (ITK-113).
                    one_row_zero += 1;
                } else {
                    t.f64s(&|| w(&format!("qr {r}x{c} f64 solve")), &xr, &x);
                }
                let msg = if info > 0 { format!("{RANK_DEFICIENT}{info}\n") } else { String::new() };
                t.text(&|| w(&format!("qr {r}x{c} f64 solve message")), &msg, &after_modules(&err));
            }
            if r == c {
                let mut inv = vec![0.0f64; r * r];
                let _ = capture.run(lib, || unsafe { (lib.qr_inverse_f64)(r as c_int, m.as_ptr(), inv.as_mut_ptr()) });
                if !(r == 1 && qr.inverse().1[0] == 1) {
                    t.f64s(&|| w(&format!("qr {r}x{c} f64 inverse")), &qr.inverse().0, &inv);
                }
            }
        }

        // vnl_svd<float> as OpenSvdcmp reads it (rows >= cols).
        let (r, c) = (r.max(c), r.min(c));
        let m = matrix(&mut rng, r, c, family);
        let mf = f32s(&m);
        if family != 6 {
            let (mut u, mut wv, mut v) = (vec![0.0f32; r * c], vec![0.0f32; c], vec![0.0f32; c * c]);
            let (ok, _, _) = capture.run(lib, || unsafe {
                (lib.svd_f32)(r as c_int, c as c_int, mf.as_ptr(), u.as_mut_ptr(), wv.as_mut_ptr(), v.as_mut_ptr())
            });
            let ours = vnl_svd_f32::vnl_svd_f32(&mf, r, c);
            t.bits(&|| w(&format!("svd f32 {r}x{c} valid")), u64::from(ours.is_some()), ok as u64);
            if let (Some((uo, wo, vo)), 1) = (ours, ok) {
                t.f32s(&|| w(&format!("svd f32 {r}x{c} U")), &col_to_row(&uo, r, c), &u);
                t.f32s(&|| w(&format!("svd f32 {r}x{c} W")), &wo, &wv);
                t.f32s(&|| w(&format!("svd f32 {r}x{c} V")), &col_to_row(&vo, c, c), &v);
            }
        }

        // vnl_matrix_inverse<double> (square).
        let n = 1 + rng.below(6) as usize;
        let m = matrix(&mut rng, n, n, family);
        if family != 6 {
            let mut inv = vec![0.0f64; n * n];
            let (_, _, err) = capture.run(lib, || unsafe { (lib.matrix_inverse_f64)(n as c_int, m.as_ptr(), inv.as_mut_ptr()) });
            if let Some(ours) = vnl_svd::vnl_inverse(&m, n) {
                if err.is_empty() {
                    t.f64s(&|| w(&format!("matrix_inverse n={n}")), &ours, &inv);
                }
            }
        }

        // Symmetric eigensystems, float and double.
        let n = 1 + rng.below(8) as usize;
        let mut m = matrix(&mut rng, n, n, family);
        symmetrize(&mut m, n, false);
        let mf = f32s(&m);
        if family != 6 {
            let (mut vec, mut val) = (vec![0.0f32; n * n], vec![0.0f32; n]);
            let (_, _, err) = capture.run(lib, || unsafe { (lib.symeig_f32)(n as c_int, mf.as_ptr(), vec.as_mut_ptr(), val.as_mut_ptr()) });
            let (vo, veco, ierr) = vnl_eigen::symmetric_eigensystem_f32(&mf, n);
            let msg = if ierr != 0 { format!("vnl_symmetric_eigensystem: ierr = {ierr}\n") } else { String::new() };
            t.text(&|| w(&format!("symeig f32 n={n} message")), &msg, &err);
            t.f32s(&|| w(&format!("symeig f32 n={n} D")), &vo, &val);
            if ierr == 0 {
                t.f32s(&|| w(&format!("symeig f32 n={n} V")), &veco, &vec);
            }
            let (mut vec, mut val) = (vec![0.0f64; n * n], vec![0.0f64; n]);
            let (_, _, err) = capture.run(lib, || unsafe { (lib.symeig_f64)(n as c_int, m.as_ptr(), vec.as_mut_ptr(), val.as_mut_ptr()) });
            let e = vnl_eigen::symmetric_eigensystem(&m, n);
            let msg = if e.ierr != 0 { format!("vnl_symmetric_eigensystem: ierr = {}\n", e.ierr) } else { String::new() };
            t.text(&|| w(&format!("symeig f64 n={n} message")), &msg, &err);
            t.f64s(&|| w(&format!("symeig f64 n={n} D")), &e.values, &val);
            if e.ierr == 0 {
                t.f64s(&|| w(&format!("symeig f64 n={n} V")), &e.vectors, &vec);
            }
        }

        // vnl_real_eigensystem (finite entries of moderate size: VNL's balanc never returns on
        // others, ITK-112).
        let n = 1 + rng.below(8) as usize;
        let m = matrix(&mut rng, n, n, family);
        if family != 6 {
            let (mut vreal, mut vre, mut vim, mut dre, mut dim) =
                (vec![0.0f64; n * n], vec![0.0f64; n * n], vec![0.0f64; n * n], vec![0.0f64; n], vec![0.0f64; n]);
            let (_, _, err) = capture.run(lib, || unsafe {
                (lib.realeig)(n as c_int, m.as_ptr(), vreal.as_mut_ptr(), vre.as_mut_ptr(), vim.as_mut_ptr(), dre.as_mut_ptr(), dim.as_mut_ptr())
            });
            let es = vnl_eigen::real_eigensystem(&m, n).expect("finite input");
            let msg = if es.ierr != 0 { vnl_eigen::real_eigensystem_failure_message(&m, n, es.ierr) } else { String::new() };
            t.text(&|| w(&format!("realeig n={n} message")), &msg, &err);
            if es.ierr == 0 {
                t.f64s(&|| w(&format!("realeig n={n} D re")), &es.d_re, &dre);
                t.f64s(&|| w(&format!("realeig n={n} D im")), &es.d_im, &dim);
                t.f64s(&|| w(&format!("realeig n={n} V re")), &es.v_re, &vre);
                t.f64s(&|| w(&format!("realeig n={n} V im")), &es.v_im, &vim);
                for col in 0..n {
                    if es.vreal_written[col] {
                        let ours: Vec<f64> = (0..n).map(|r| es.vreal[r * n + col]).collect();
                        let theirs: Vec<f64> = (0..n).map(|r| vreal[r * n + col]).collect();
                        t.f64s(&|| w(&format!("realeig n={n} Vreal column {col}")), &ours, &theirs);
                    } else {
                        vreal_unwritten += 1;
                    }
                }
            }
        }

        // vnl_cholesky in its three modes, with solve, determinant and inverse.
        let n = 1 + rng.below(8) as usize;
        let mut m = matrix(&mut rng, n, n, family);
        let spd = rng.below(3) != 0;
        if rng.below(6) != 0 {
            symmetrize(&mut m, n, spd);
        }
        if family != 6 {
            let mode = rng.below(3) as i32;
            let bvec: Vec<f64> = (0..n).map(|_| rng.range(-100, 100) as f64 / 8.0).collect();
            let finite = m.iter().all(|v| v.is_finite() && v.abs() < 1e100);
            let mut ours_out = Captured::default();
            let mut ch = VnlCholesky::new(&m, n, Mode::from_value(mode), &mut ours_out);
            let rank = ch.rank_deficiency();
            let want = 1 | if finite { 2 } else { 0 } | if rank == 0 { 4 } else { 0 };
            let (mut up, mut lo, mut rc, mut x, mut det, mut inv) =
                (vec![0.0f64; n * n], vec![0.0f64; n * n], 0.0f64, vec![0.0f64; n], 0.0f64, vec![0.0f64; n * n]);
            let (rank_c, _, err) = capture.run(lib, || unsafe {
                (lib.cholesky)(n as c_int, m.as_ptr(), mode, up.as_mut_ptr(), lo.as_mut_ptr(), &mut rc, want, bvec.as_ptr(), x.as_mut_ptr(), &mut det, inv.as_mut_ptr())
            });
            let wc = || w(&format!("cholesky n={n} mode={mode} spd={spd}"));
            t.bits(&|| format!("{} rank", wc()), rank as u64, rank_c as u64);
            t.f64s(&|| format!("{} upper", wc()), &ch.upper_triangle(), &up);
            t.f64s(&|| format!("{} lower", wc()), &ch.lower_triangle(), &lo);
            if mode == 2 && rank == 0 {
                t.f64s(&|| format!("{} rcond", wc()), &[ch.rcond().unwrap()], &[rc]);
            }
            t.f64s(&|| format!("{} solve", wc()), &ch.solve(&bvec), &x);
            if finite {
                t.f64s(&|| format!("{} determinant", wc()), &[ch.determinant()], &[det]);
            }
            if rank == 0 {
                t.f64s(&|| format!("{} inverse", wc()), &ch.inverse(&mut ours_out).unwrap(), &inv);
            }
            // The messages (ITK-108's rcond is memory contents in VNL: compared up to it).
            let ours = ours_out.text(Stream::Err);
            let theirs = if mode == 2 && rank != 0 {
                err.lines()
                    .map(|l| match l.strip_prefix("vnl_cholesky: rcond=") {
                        Some(rest) => format!("vnl_cholesky: rcond=0 {}\n", &rest[rest.find(" so").unwrap() + 1..]),
                        None => format!("{l}\n"),
                    })
                    .collect()
            } else {
                err
            };
            t.text(&|| format!("{} messages", wc()), &ours, &theirs);
        }
    }
    t.notes.push(format!(
        "{skipped_singular} fixed inverses of a zero determinant not compared (VNL returns memory, ITK-107); {vreal_unwritten} Vreal columns of complex pairs not compared (never written, ITK-110); {one_row_zero} one-row QR solutions of a zero entry not compared (never written, ITK-113)"
    ));
}

/// `vnl_fft_1d<double>` for every power of 2 from 2 to 512 (N4 pads its histogram to one: 512
/// for FreeSurfer's 200 bins), random signals, both directions.
pub fn run_fft(lib: &Lib, scale: u64, t: &mut Tally) {
    let mut rng = SplitMix64::new(0xff7_5eed);
    for _ in 0..(40 * scale) {
        // Powers of 2 to 512: bit for bit. From 1024 the port's correctly rounded twiddle factors
        // differ from glibc's sincos (larmorx-image's documented choice): counted, not compared.
        for k in 1..=9 {
            let n = 1usize << k;
            t.cases += 1;
            let x: Vec<f64> = (0..2 * n)
                .map(|_| match rng.below(3) {
                    0 => (rng.range(-1_000_000, 1_000_000) as f64) / 1024.0,
                    1 => 0.0,
                    _ => f64::from_bits(rng.next_u64() & 0x3fff_ffff_ffff_ffff) * 1e-300,
                })
                .collect();
            let fft = VnlFft1d::new(n).expect("a power of 2");
            for forward in [true, false] {
                let mut ours = x.clone();
                let mut theirs = x.clone();
                if forward {
                    fft.fwd_transform(&mut ours);
                } else {
                    fft.bwd_transform(&mut ours);
                }
                // SAFETY: 2n doubles.
                unsafe { (lib.fft)(n as i32, i32::from(forward), theirs.as_mut_ptr()) };
                t.f64s(&|| format!("fft n={n} forward={forward}"), &ours, &theirs);
            }
        }
    }
}
