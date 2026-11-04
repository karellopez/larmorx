// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! Exhaustive comparison of `larmorx_nc_core::glibc_float` with the platform's C library, and
//! of the C library with correctly rounded maths at the arguments mcflirt computes with.
//!
//! On Linux, `f32::sin`, `f32::cos` and `f32::ln` call the C library's `sinf`, `cosf` and
//! `logf`, and `sincosf` is called directly. Run with glibc 2.35 on an x86-64 CPU with FMA
//! (the machine the FSL oracle runs on), every one of the 2^32 inputs of each function must
//! give the same bits as the port (`port`). The tool also reports how often glibc is not
//! correctly rounded (`cr`) over all inputs and over the ranges mcflirt uses: rotation angles
//! for `sincosf` (`make_rot`), probabilities for `logf` (the mutual-information costs), and
//! the sinc kernel of `-sinc_final` and the fourth stage (`MISCMATHS::sinckernel1D`: Blackman
//! window, width 7, 1201 samples; its double `cos` too).
//!
//! `larmorx_nc_core::fdlibm`'s `atanf` is compared on all 2^32 inputs too, and its `atan2f`
//! (`rotmat2euler`) on every pair of special values and on 3 × 2^30 seeded random pairs:
//! random bits, rotation-like pairs (`|y| <= 1`, `x` in `[0.5, 1]`: `atan2(sy, cy)`), and
//! pairs in `[-1, 1]` (`atan2(sx, cx)`, `atan2(sz, cz)`).
//!
//! `expf` (FAST: `larmorx_nc_core::glibc_float::expf`) is compared on all 2^32 inputs too.
//!
//! `copy` (2026-11-04) compares the copies of these functions in `larmorx_core::math`
//! (`sinf_glibc`, ..., `pow_glibc`, the Apache-2.0 home of the same Arm and fdlibm translations)
//! with this crate's: every float input of the one-argument float functions, and for `powf`,
//! `atan2f`, `exp`, `log` and `pow` the sampled families and special values of
//! `validation/investigations/glibc-maths/verifier/src/samples.rs` (more than 10^9 arguments per
//! function), bit for bit. It proves the copy before this crate's functions became re-exports of
//! the core's (commit "the copy" of the glibc maths part); afterwards both sides are the same
//! function. Not part of the default run.
//!
//! Usage: `glibc-float-verify [--threads N] [kernel] [sinf] [cosf] [sincosf] [logf] [expf]
//! [atanf] [atan2f] [copy]` (all but `copy` when none is named). Exits 0 when the port matches the
//! platform everywhere.
#![deny(unsafe_code)]

use larmorx_core::math;
use larmorx_nc_core::{fdlibm, glibc_double, glibc_float, kernel};
use rayon::prelude::*;

#[path = "../../../../../validation/investigations/glibc-maths/verifier/src/samples.rs"]
#[allow(dead_code)]
mod samples;

#[allow(unsafe_code)]
mod libc {
    unsafe extern "C" {
        fn sincosf(x: f32, s: *mut f32, c: *mut f32);
    }

    /// The C library's `sincosf`.
    pub fn sincos(x: f32) -> (f32, f32) {
        let (mut s, mut c) = (0.0f32, 0.0f32);
        // SAFETY: sincosf writes one float through each of the two valid, distinct pointers.
        unsafe { sincosf(x, &mut s, &mut c) };
        (s, c)
    }
}

fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

#[derive(Default, Clone, Copy)]
struct Counts {
    n: u64,
    port: u64,
    cr: u64,
    cr_range: u64,
    n_range: u64,
}

impl Counts {
    fn add(mut self, o: Counts) -> Counts {
        self.n += o.n;
        self.port += o.port;
        self.cr += o.cr;
        self.cr_range += o.cr_range;
        self.n_range += o.n_range;
        self
    }
}

/// Every float: the port against the platform (`libc`), the platform against the correctly
/// rounded function (`cr`), and the latter in `range`.
fn sweep(
    name: &str,
    libc: impl Fn(f32) -> f32 + Sync,
    port: impl Fn(f32) -> f32 + Sync,
    cr: impl Fn(f32) -> f32 + Sync,
    range: impl Fn(f32) -> bool + Sync,
    range_name: &str,
) -> bool {
    let c = (0u32..=u32::MAX >> 16)
        .into_par_iter()
        .map(|hi| {
            let mut c = Counts::default();
            for lo in 0..=0xffffu32 {
                let x = f32::from_bits((hi << 16) | lo);
                let g = libc(x);
                c.n += 1;
                if !same(g, port(x)) {
                    c.port += 1;
                    if c.port <= 3 {
                        println!("{name}: port differs at {x:e} ({:#010x})", x.to_bits());
                    }
                }
                let wrong = !same(g, cr(x));
                c.cr += u64::from(wrong);
                if range(x) {
                    c.n_range += 1;
                    c.cr_range += u64::from(wrong);
                }
            }
            c
        })
        .reduce(Counts::default, Counts::add);
    println!(
        "{name}: port differs from the C library on {} of {} inputs; the C library is not correctly \
         rounded on {} ({:.4} %), {} of {} {range_name} ({:.4} %)",
        c.port,
        c.n,
        c.cr,
        100.0 * c.cr as f64 / c.n as f64,
        c.cr_range,
        c.n_range,
        100.0 * c.cr_range as f64 / c.n_range.max(1) as f64
    );
    c.port == 0
}

fn kernel_report() -> bool {
    let (w, n) = (7, 1201);
    let port = kernel::sinckernel1d("blackman", w, n, &mut Vec::new());
    let hw = (w - 1) / 2;
    let halfnk = ((f64::from(n) - 1.0) / 2.0) as f32;
    let (mut d_sinf, mut d_cos, mut d_val, mut d_port) = (0, 0, 0, 0);
    let mut d_kernel_port = 0;
    for k in 1..=n {
        let x = ((k as f32 - halfnk - 1.0) / halfnk) * hw as f32;
        let y = (std::f64::consts::PI * f64::from(x)) as f32;
        let tiny = f64::from(x.abs()) < 1e-7;
        if !tiny && !same(y.sin(), math::sinf(y)) {
            d_sinf += 1;
        }
        if !tiny && !same(y.sin(), glibc_float::sinf(y)) {
            d_port += 1;
        }
        let a = std::f64::consts::PI * f64::from(x) / f64::from(hw);
        let b = 2.0 * std::f64::consts::PI * f64::from(x) / f64::from(hw);
        for arg in [a, b] {
            if arg.cos().to_bits() != math::cos(arg).to_bits() {
                d_cos += 1;
            }
        }
        let sinc = if tiny {
            (1.0 - f64::from(x.abs())) as f32
        } else {
            y.sin() / y
        };
        let win = if x.abs() > hw as f32 {
            0.0
        } else {
            (0.42 + 0.5 * a.cos() + 0.08 * b.cos()) as f32
        };
        let sinc_cr = if tiny {
            (1.0 - f64::from(x.abs())) as f32
        } else {
            math::sinf(y) / y
        };
        if f64::from(sinc * win).to_bits() != f64::from(sinc_cr * win).to_bits() {
            d_val += 1;
        }
        if f64::from(sinc * win).to_bits() != port[(k - 1) as usize].to_bits() {
            d_kernel_port += 1;
        }
    }
    println!(
        "sinc kernel (blackman, 7, 1201 samples): the C library's sinf is not correctly rounded on \
         {d_sinf} of its arguments (the port differs from it on {d_port}), its double cos on \
         {d_cos} of {}; {d_val} samples differ from the correctly rounded kernel, \
         {d_kernel_port} from the port's",
        2 * n
    );
    // The table of the stage-4 cost's sinc (costfns.cc q_setupkernel: Hanning window, half-width
    // 3, 201 samples): sinf((float)(M_PI*x)) and cos(M_PI*x/3).
    let (mut q_sinf, mut q_port, mut q_cos) = (0, 0, 0);
    for k in 0..201 {
        let x = ((f64::from(k) - 100.0) / 100.0 * 3.0) as f32;
        let y = (std::f64::consts::PI * f64::from(x)) as f32;
        if f64::from(x.abs()) >= 1e-7 {
            q_sinf += u32::from(!same(y.sin(), math::sinf(y)));
            q_port += u32::from(!same(y.sin(), glibc_float::sinf(y)));
        }
        let a = std::f64::consts::PI * f64::from(x) / 3.0;
        q_cos += u32::from(a.cos().to_bits() != math::cos(a).to_bits());
    }
    println!(
        "stage-4 cost sinc table (hanning, 3, 201 samples): the C library's sinf is not correctly \
         rounded on {q_sinf} arguments (the port differs from it on {q_port}), its double cos on \
         {q_cos} of 201"
    );
    // The mutual-information costs' log(no_bins) when no voxel overlaps, and the spline
    // deconvolution's log(tolerance)/log(|z|) (only its integer part is used).
    let d_log = (1..=4096)
        .filter(|&b| f64::from(b).ln().to_bits() != math::log(f64::from(b)).to_bits())
        .count();
    println!("log of the integers 1..4096: the C library is not correctly rounded on {d_log}");
    d_port == 0 && d_kernel_port == 0 && q_port == 0
}

/// SplitMix64: the seeded pairs of `atan2f_report`.
fn splitmix64(i: u64) -> u64 {
    let mut z = i.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A family of seeded `(y, x)` pairs: its name and the pair of a 64-bit random number.
type PairFamily = (&'static str, fn(u64) -> (f32, f32));

/// A float uniform in `[lo, hi]` from 24 random bits.
fn uniform(bits: u32, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * ((bits >> 8) as f32 / (1u32 << 24) as f32)
}

/// `atan2f`: the port against the C library on every pair of special values and on three
/// families of 2^30 seeded random pairs.
fn atan2f_report() -> bool {
    let specials: Vec<f32> = [
        0.0f32,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        1e-30,
        1e-20,
        1e-10,
        1e-4,
        0.4375,
        0.5,
        0.6875,
        1.0,
        1.1875,
        1.5,
        2.4375,
        1e10,
        3.4e38,
        f32::MAX,
        f32::INFINITY,
        f32::NAN,
    ]
    .iter()
    .flat_map(|&v| [v, -v])
    .collect();
    let mut bad = 0u64;
    let mut n = 0u64;
    for &y in &specials {
        for &x in &specials {
            n += 1;
            if !same(y.atan2(x), fdlibm::atan2f(y, x)) {
                bad += 1;
                if bad <= 3 {
                    println!("atan2f: port differs at ({y:e}, {x:e})");
                }
            }
        }
    }
    println!("atan2f: port differs from the C library on {bad} of {n} pairs of special values");
    let mut ok = bad == 0;
    let families: [PairFamily; 3] = [
        ("random bits", |r| {
            (f32::from_bits(r as u32), f32::from_bits((r >> 32) as u32))
        }),
        ("rotation-like (|y| <= 1, x in [0.5, 1])", |r| {
            (
                uniform(r as u32, -1.0, 1.0),
                uniform((r >> 32) as u32, 0.5, 1.0),
            )
        }),
        ("in [-1, 1]", |r| {
            (
                uniform(r as u32, -1.0, 1.0),
                uniform((r >> 32) as u32, -1.0, 1.0),
            )
        }),
    ];
    for (f, (name, pair)) in families.iter().enumerate() {
        let base = (f as u64) << 40;
        let (bad, cr) = (0u64..1 << 14)
            .into_par_iter()
            .map(|hi| {
                let (mut bad, mut cr) = (0u64, 0u64);
                for lo in 0..1u64 << 16 {
                    let (y, x) = pair(splitmix64(base | (hi << 16) | lo));
                    let g = y.atan2(x);
                    if !same(g, fdlibm::atan2f(y, x)) {
                        bad += 1;
                        if bad <= 3 {
                            println!(
                                "atan2f: port differs at ({y:e}, {x:e}) ({:#010x}, {:#010x})",
                                y.to_bits(),
                                x.to_bits()
                            );
                        }
                    }
                    let want = math::atan2(f64::from(y), f64::from(x)) as f32;
                    cr += u64::from(!same(g, want));
                }
                (bad, cr)
            })
            .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        let total = 1u64 << 30;
        println!(
            "atan2f, {name}: port differs from the C library on {bad} of {total} pairs; the C \
             library is not the float nearest the correctly rounded double on {cr} ({:.4} %)",
            100.0 * cr as f64 / total as f64
        );
        ok &= bad == 0;
    }
    ok
}

/// `copy`: `larmorx_core::math`'s copies against this crate's translations, bit for bit (NaN
/// bits included). Returns whether every result is identical.
fn copy_report() -> bool {
    // Every float input of the one-argument functions.
    type Pair = (&'static str, fn(f32) -> u64, fn(f32) -> u64);
    let unary: [Pair; 8] = [
        (
            "sinf",
            |x| u64::from(glibc_float::sinf(x).to_bits()),
            |x| u64::from(math::sinf_glibc(x).to_bits()),
        ),
        (
            "cosf",
            |x| u64::from(glibc_float::cosf(x).to_bits()),
            |x| u64::from(math::cosf_glibc(x).to_bits()),
        ),
        (
            "sincosf",
            |x| {
                let (s, c) = glibc_float::sincosf(x);
                u64::from(s.to_bits()) << 32 | u64::from(c.to_bits())
            },
            |x| {
                let (s, c) = math::sincosf_glibc(x);
                u64::from(s.to_bits()) << 32 | u64::from(c.to_bits())
            },
        ),
        (
            "logf",
            |x| u64::from(glibc_float::logf(x).to_bits()),
            |x| u64::from(math::logf_glibc(x).to_bits()),
        ),
        (
            "expf",
            |x| u64::from(glibc_float::expf(x).to_bits()),
            |x| u64::from(math::expf_glibc(x).to_bits()),
        ),
        (
            "atanf",
            |x| u64::from(fdlibm::atanf(x).to_bits()),
            |x| u64::from(math::atanf_glibc(x).to_bits()),
        ),
        (
            "atanf with aT[0] = 0x3eaaaaaa",
            |x| u64::from(fdlibm::atanf_with_at0(x, f32::from_bits(0x3eaa_aaaa)).to_bits()),
            |x| u64::from(math::atanf_glibc_with_at0(x, f32::from_bits(0x3eaa_aaaa)).to_bits()),
        ),
        (
            "logf_compat over logf_glibc",
            |x| u64::from(glibc_float::logf_compat(x).to_bits()),
            |x| {
                // The wrapper's rule (x <= 0: -inf or C's NAN) over the core's logf.
                let v = if x <= 0.0 {
                    if x == 0.0 {
                        f32::NEG_INFINITY
                    } else {
                        f32::from_bits(0x7fc0_0000)
                    }
                } else {
                    math::logf_glibc(x)
                };
                u64::from(v.to_bits())
            },
        ),
    ];
    let mut ok = true;
    for (name, ours, core) in unary {
        let (n, bad) = (0u32..=u32::MAX >> 16)
            .into_par_iter()
            .map(|hi| {
                let mut bad = 0u64;
                for lo in 0..=0xffffu32 {
                    let x = f32::from_bits((hi << 16) | lo);
                    if ours(x) != core(x) {
                        bad += 1;
                        if bad <= 2 {
                            println!("copy {name}: differs at {:#010x}", x.to_bits());
                        }
                    }
                }
                (1u64 << 16, bad)
            })
            .reduce(|| (0, 0), |a, c| (a.0 + c.0, a.1 + c.1));
        println!("copy {name}: {bad} of {n} inputs differ (larmorx_core::math against this crate)");
        ok &= bad == 0;
    }
    // The sampled families and special values of the glibc-maths verifier.
    fn families<T: Copy + Sync + Send>(
        name: &str,
        g: u64,
        fams: &[samples::Family<T>],
        specials: &[T],
        differ: impl Fn(T) -> bool + Sync,
    ) -> bool {
        let mut ok = true;
        for (f, fam) in fams.iter().enumerate() {
            let bad: u64 = (0..fam.count >> 16)
                .into_par_iter()
                .map(|c| {
                    (c << 16..(c + 1) << 16)
                        .filter(|&i| differ((fam.arg)(samples::random(g, f as u64, i))))
                        .count() as u64
                })
                .sum();
            println!("copy {name}, {}: {bad} of {} differ", fam.name, fam.count);
            ok &= bad == 0;
        }
        let bad = specials.iter().filter(|&&a| differ(a)).count();
        println!(
            "copy {name}, special values: {bad} of {} differ",
            specials.len()
        );
        ok && bad == 0
    }
    let pairs = |v: &[f32]| -> Vec<(f32, f32)> {
        v.iter()
            .flat_map(|&a| v.iter().map(move |&c| (a, c)))
            .collect()
    };
    let s32 = pairs(&samples::special_f32());
    let s64 = samples::special_f64();
    let p64: Vec<(f64, f64)> = s64
        .iter()
        .flat_map(|&a| s64.iter().map(move |&c| (a, c)))
        .collect();
    ok &= families("powf", 1, &samples::POWF, &s32, |(x, y)| {
        glibc_float::powf(x, y).to_bits() != math::powf_glibc(x, y).to_bits()
    });
    ok &= families("atan2f", 2, &samples::ATAN2F, &s32, |(y, x)| {
        fdlibm::atan2f(y, x).to_bits() != math::atan2f_glibc(y, x).to_bits()
    });
    ok &= families("exp", 3, &samples::EXP, &s64, |x| {
        glibc_double::exp(x).to_bits() != math::exp_glibc(x).to_bits()
    });
    ok &= families("log", 4, &samples::LOG, &s64, |x| {
        glibc_double::log(x).to_bits() != math::log_glibc(x).to_bits()
    });
    ok &= families("pow", 5, &samples::POW, &p64, |(x, y)| {
        glibc_double::pow(x, y).to_bits() != math::pow_glibc(x, y).to_bits()
    });
    ok
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut threads = 0;
    if let Some(i) = args.iter().position(|a| a == "--threads") {
        threads = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(0);
        args.drain(i..(i + 2).min(args.len()));
    }
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global();
    if args.iter().any(|a| a == "copy") {
        std::process::exit(i32::from(!copy_report()));
    }
    let all = args.is_empty();
    let want = |s: &str| all || args.iter().any(|a| a == s);
    let angles = |x: f32| x.abs() <= 0.5;
    let mut ok = true;
    if want("kernel") {
        ok &= kernel_report();
    }
    if want("sinf") {
        ok &= sweep(
            "sinf",
            f32::sin,
            glibc_float::sinf,
            math::sinf,
            angles,
            "angles |x| <= 0.5",
        );
    }
    if want("cosf") {
        ok &= sweep(
            "cosf",
            f32::cos,
            glibc_float::cosf,
            math::cosf,
            angles,
            "angles |x| <= 0.5",
        );
    }
    if want("sincosf") {
        ok &= sweep(
            "sincosf (sine)",
            |x| libc::sincos(x).0,
            |x| glibc_float::sincosf(x).0,
            math::sinf,
            angles,
            "angles |x| <= 0.5",
        );
        ok &= sweep(
            "sincosf (cosine)",
            |x| libc::sincos(x).1,
            |x| glibc_float::sincosf(x).1,
            math::cosf,
            angles,
            "angles |x| <= 0.5",
        );
    }
    if want("logf") {
        ok &= sweep(
            "logf",
            f32::ln,
            glibc_float::logf,
            math::logf,
            |x| x > 0.0 && x <= 1.0,
            "probabilities in (0, 1]",
        );
    }
    if want("expf") {
        // FAST's arguments: minus the Gaussian energies (Initclass) and the log intensities
        ok &= sweep(
            "expf",
            f32::exp,
            glibc_float::expf,
            math::expf,
            |x| (-110.0..=12.0).contains(&x),
            "FAST's arguments [-110, 12]",
        );
    }
    if want("atanf") {
        ok &= sweep(
            "atanf",
            f32::atan,
            fdlibm::atanf,
            |x| math::atan2(f64::from(x), 1.0) as f32,
            |x| x.abs() <= 0.05,
            "small arguments |x| <= 0.05",
        );
        // fdlibm's comment value of aT[0] (0x3eaaaaaa, the translation before 2026-10-25):
        // the inputs whose atanf it changes (1,037,658; 882,244 below 0.4375).
        let comment = f32::from_bits(0x3eaa_aaaa);
        let (n, small) = (0u32..=u32::MAX >> 16)
            .into_par_iter()
            .map(|hi| {
                let (mut n, mut small) = (0u64, 0u64);
                for lo in 0..=0xffffu32 {
                    let x = f32::from_bits((hi << 16) | lo);
                    if !same(fdlibm::atanf(x), fdlibm::atanf_with_at0(x, comment)) {
                        n += 1;
                        small += u64::from(x.abs() < 0.4375);
                    }
                }
                (n, small)
            })
            .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        println!(
            "atanf with fdlibm's comment value of aT[0] (0x3eaaaaaa): differs on {n} of 2^32 inputs \
             ({small} with |x| < 0.4375)"
        );
    }
    if want("atan2f") {
        ok &= atan2f_report();
    }
    std::process::exit(i32::from(!ok));
}
