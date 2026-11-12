// SPDX-License-Identifier: Apache-2.0
//! `larmorx_core::math::hypot` (the port of CORE-MATH's `cr_hypot`) against CORE-MATH's C, and
//! how often the C library's `hypot` (glibc 2.35's own code, a black box: never read) differs
//! from the correctly rounded result.
//!
//! `coremath-hypot <libcrhypot.so> [log2n] [--threads T]`: on every pair of special values and
//! on `2^log2n` pairs of each family (random bit patterns; uniform on (-1, 1), the entries of a
//! rotation matrix; the components of random unit quaternions, as SciPy's `Rotation.as_euler`
//! passes them to `hypot`; exponent differences 20 to 34, around the `fma` shortcut at 27; one
//! or both subnormal; near overflow; scaled Pythagorean triples moved by -3 to 3 units in the
//! last place, near exact and midpoint results; small integers), the bits of
//! `larmorx_core::math::hypot`, of the C's `cr_hypot` and of glibc's `hypot` (NaN equals NaN).
//! Prints the counts and `VERIFY-PASS` (larmorx equals the C on every pair) or `VERIFY-FAIL`
//! (exit status 1). Meaningful on x86-64 Linux with glibc 2.35 (the oracle machine).

use std::sync::atomic::{AtomicU64, Ordering};

use larmorx_core::math;

unsafe extern "C" {
    // The C library's hypot (libm), a black box.
    fn hypot(x: f64, y: f64) -> f64;
    fn dlopen(filename: *const std::ffi::c_char, flag: std::ffi::c_int) -> *mut std::ffi::c_void;
    fn dlsym(
        handle: *mut std::ffi::c_void,
        symbol: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_void;
}

type Fn2 = extern "C" fn(f64, f64) -> f64;

fn glibc_hypot(x: f64, y: f64) -> f64 {
    // SAFETY: libm's `double hypot(double, double)`.
    unsafe { hypot(x, y) }
}

/// `cr_hypot` of the shared library compiled from CORE-MATH's C (`build.sh`).
fn core_math(lib: &str) -> Fn2 {
    let c = std::ffi::CString::new(lib).unwrap();
    // SAFETY: the dynamic loader's calls with NUL-terminated names; the symbol is the C function
    // `double cr_hypot(double, double)`.
    unsafe {
        let h = dlopen(c.as_ptr(), 2);
        assert!(!h.is_null(), "dlopen({lib})");
        let f = dlsym(h, c"cr_hypot".as_ptr());
        assert!(!f.is_null(), "dlsym(cr_hypot)");
        std::mem::transmute::<*mut std::ffi::c_void, Fn2>(f)
    }
}

fn splitmix64(i: u64) -> u64 {
    let mut z = i.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A uniform double in [0, 1) from 53 random bits.
fn unit(r: u64) -> f64 {
    (r >> 11) as f64 / (1u64 << 53) as f64
}

fn signed(r: u64, v: f64) -> f64 {
    if r & 1 == 1 { -v } else { v }
}

/// The `i`-th pair of family `fam`.
fn pair(fam: usize, i: u64) -> (f64, f64) {
    let a = splitmix64(i.wrapping_mul(16).wrapping_add(fam as u64));
    let b = splitmix64(a ^ 0x5851_f42d_4c95_7f2d);
    let c = splitmix64(b);
    match fam {
        0 => (f64::from_bits(a), f64::from_bits(b)),
        1 => (unit(a) * 2.0 - 1.0, unit(b) * 2.0 - 1.0),
        2 => {
            // A random unit quaternion (normalized Gaussian-like components from four uniforms).
            let q = [
                unit(a) - 0.5,
                unit(b) - 0.5,
                unit(c) - 0.5,
                unit(splitmix64(c)) - 0.5,
            ];
            let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
            let k = (c >> 60) as usize % 4;
            (q[k] / n, q[(k + 1 + (c >> 58) as usize % 3) % 4] / n)
        }
        3 => {
            let x = 1.0 + unit(a);
            let d = 20 + (b % 15) as i32;
            (
                signed(c, x * 2f64.powi((c >> 8) as i32 % 200 - 100)),
                x * 2f64.powi(-d) * (1.0 + unit(b)),
            )
        }
        4 => {
            // One subnormal, one normal or subnormal.
            let s = f64::from_bits(a & ((1u64 << 52) - 1));
            let t = if b & 2 == 0 {
                f64::from_bits(b & ((1u64 << 52) - 1))
            } else {
                f64::from_bits((b & ((1u64 << 52) - 1)) | ((1 + c % 60) << 52))
            };
            (signed(c, s), t)
        }
        5 => {
            let e = 1018 + (c % 6) as i32;
            (
                signed(a, (1.0 + unit(a)) * 2f64.powi(e)),
                (1.0 + unit(b)) * 2f64.powi(e - (b % 4) as i32),
            )
        }
        6 => {
            // Scaled Pythagorean triples (Euclid's formula), moved by a few units in the last place.
            let m = 2 + (a % 3000);
            let n = 1 + (b % (m - 1));
            let (p, q) = ((m * m - n * n) as f64, (2 * m * n) as f64);
            let s = 2f64.powi((c % 600) as i32 - 300);
            let dx = (c >> 16) % 7;
            let dy = (c >> 24) % 7;
            let bump = |v: f64, d: u64| f64::from_bits((v.to_bits() as i64 + d as i64 - 3) as u64);
            (bump(p * s, dx), bump(q * s, dy))
        }
        _ => ((a % (1 << 26)) as f64, signed(c, (b % (1 << 26)) as f64)),
    }
}

const FAMILIES: [&str; 8] = [
    "random bit patterns",
    "uniform on (-1, 1)",
    "unit-quaternion components",
    "exponent differences 20 to 34",
    "subnormal",
    "near overflow",
    "Pythagorean triples +-3 ulp",
    "small integers",
];

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lib = args
        .get(1)
        .expect("usage: coremath-hypot <libcrhypot.so> [log2n] [--threads T]");
    let log2n: u32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(24);
    let threads: u64 = args
        .iter()
        .position(|a| a == "--threads")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let cr = core_math(lib);
    let mut fail = false;
    // Every pair of special values.
    let specials = [
        0.0,
        -0.0,
        f64::from_bits(1),
        f64::from_bits(0x000f_ffff_ffff_ffff),
        f64::MIN_POSITIVE,
        1.0,
        -1.0,
        f64::MAX,
        -f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        -f64::NAN,
        f64::from_bits(0x7ff0_0000_0000_0001), // a signalling NaN
        3.0,
        4.0,
    ];
    let (mut n, mut bad, mut glibc) = (0u64, 0u64, 0u64);
    for &x in &specials {
        for &y in &specials {
            let (l, c, g) = (math::hypot(x, y), cr(x, y), glibc_hypot(x, y));
            n += 1;
            bad += u64::from(!same(l, c));
            glibc += u64::from(!same(g, c));
            if !same(l, c) {
                println!("  DIFF special ({x:e}, {y:e}): larmorx {l:e}, C {c:e}");
            }
        }
    }
    println!("special values: {n} pairs, larmorx differs from the C on {bad}, glibc on {glibc}");
    fail |= bad > 0;
    let total = 1u64 << log2n;
    for (fam, name) in FAMILIES.iter().enumerate() {
        let bad = AtomicU64::new(0);
        let glibc = AtomicU64::new(0);
        let chunk = total.div_ceil(threads);
        std::thread::scope(|sc| {
            for t in 0..threads {
                let (bad, glibc) = (&bad, &glibc);
                sc.spawn(move || {
                    let (mut b, mut g) = (0u64, 0u64);
                    for i in t * chunk..((t + 1) * chunk).min(total) {
                        let (x, y) = pair(fam, i);
                        let c = cr(x, y);
                        if !same(math::hypot(x, y), c) {
                            if b < 4 {
                                println!(
                                    "  DIFF {name} ({:#018x}, {:#018x}): larmorx {:#018x}, C {:#018x}",
                                    x.to_bits(),
                                    y.to_bits(),
                                    math::hypot(x, y).to_bits(),
                                    c.to_bits()
                                );
                            }
                            b += 1;
                        }
                        g += u64::from(!same(glibc_hypot(x, y), c));
                    }
                    bad.fetch_add(b, Ordering::Relaxed);
                    glibc.fetch_add(g, Ordering::Relaxed);
                });
            }
        });
        let (b, g) = (bad.into_inner(), glibc.into_inner());
        println!(
            "{name}: {total} pairs, larmorx differs from the C on {b}, glibc's hypot on {g} ({:.4} %)",
            100.0 * g as f64 / total as f64
        );
        fail |= b > 0;
    }
    println!("{}", if fail { "VERIFY-FAIL" } else { "VERIFY-PASS" });
    if fail {
        std::process::exit(1);
    }
}
