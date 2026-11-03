// SPDX-License-Identifier: Apache-2.0
//! glibc's double maths functions against the correctly rounded ones (`larmorx_core::math`,
//! CORE-MATH's), on the arguments `fslmaths` gives them.
//!
//! - `measure [--log2n N] [--threads T]`: for each of `sin`, `cos`, `tan`, `atan`, `asin`,
//!   `acos`, `exp`, `log` and `pow`, the C library's result (a black box: its code is never
//!   read) against the correctly rounded one on `2^N` random arguments of each family (default
//!   2^25: about 10^8 per function), and on every integer of an int16 image's range: how often
//!   the double results differ, and how often the difference survives the rounding to `float`
//!   (fslmaths' default `-dt float`: `(float) f((double) v)`). Counts only.
//! - `coremath <libcrmath.so> <tan.wc> <atan.wc> [log2n]`: `larmorx_core::math::tan` and
//!   `larmorx_core::math::atan2(x, 1)` (the replica's `-atan`) against CORE-MATH's C `cr_tan`
//!   and `cr_atan` (`build.sh`), on every worst case of `tan.wc` and `atan.wc` at ±x and `2^log2n`
//!   random arguments of each family. Prints `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1).
//!
//! Meaningful on x86-64 Linux with glibc 2.35 (the oracle machine): FSL's programs call these
//! functions there.

use std::sync::atomic::{AtomicU64, Ordering};

use larmorx_core::math;

unsafe extern "C" {
    // The C library's functions (libm), black boxes. `sin` ... `atan` have one symbol version
    // on x86-64 (GLIBC_2.2.5); `exp`, `log` and `pow` bind the newest (GLIBC_2.29), whose
    // values are those of the GLIBC_2.2.5 wrappers FSL's binaries bind wherever the result is
    // finite (the wrappers differ in errno and in the error cases only).
    fn sin(x: f64) -> f64;
    fn cos(x: f64) -> f64;
    fn tan(x: f64) -> f64;
    fn atan(x: f64) -> f64;
    fn asin(x: f64) -> f64;
    fn acos(x: f64) -> f64;
    fn exp(x: f64) -> f64;
    fn log(x: f64) -> f64;
    fn pow(x: f64, y: f64) -> f64;
}

/// The functions measured: name, glibc's, the correctly rounded one.
type Pair = (&'static str, fn(f64, f64) -> f64, fn(f64, f64) -> f64);

// SAFETY (every wrapper): a pure function of its arguments.
fn g_sin(x: f64, _: f64) -> f64 {
    unsafe { sin(x) }
}
fn g_cos(x: f64, _: f64) -> f64 {
    unsafe { cos(x) }
}
fn g_tan(x: f64, _: f64) -> f64 {
    unsafe { tan(x) }
}
fn g_atan(x: f64, _: f64) -> f64 {
    unsafe { atan(x) }
}
fn g_asin(x: f64, _: f64) -> f64 {
    unsafe { asin(x) }
}
fn g_acos(x: f64, _: f64) -> f64 {
    unsafe { acos(x) }
}
fn g_exp(x: f64, _: f64) -> f64 {
    unsafe { exp(x) }
}
fn g_log(x: f64, _: f64) -> f64 {
    unsafe { log(x) }
}
fn g_pow(x: f64, y: f64) -> f64 {
    unsafe { pow(x, y) }
}

const PAIRS: [Pair; 9] = [
    ("sin", g_sin, |x, _| math::sin(x)),
    ("cos", g_cos, |x, _| math::cos(x)),
    ("tan", g_tan, |x, _| math::tan(x)),
    ("atan", g_atan, |x, _| math::atan2(x, 1.0)),
    ("asin", g_asin, |x, _| math::asin(x)),
    ("acos", g_acos, |x, _| math::acos(x)),
    ("exp", g_exp, |x, _| math::exp(x)),
    ("log", g_log, |x, _| math::log(x)),
    ("pow", g_pow, math::pow),
];

/// SplitMix64.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

/// A random family of arguments: its name, and the argument (and exponent, for `pow`) of
/// index `i`.
#[derive(Clone, Copy)]
enum Family {
    /// Image intensities as `float` voxels: a log-uniform magnitude from 2^-10 to 2^17, random
    /// sign (positive only for `log` and `pow`'s base), widened to double.
    FloatIntensity,
    /// The same as `double` voxels (`-dt double`).
    DoubleIntensity,
    /// Uniform on [-1, 1] as `float` (`asin`, `acos`).
    FloatUnit,
    /// Uniform on [-1, 1] as `double`.
    DoubleUnit,
    /// Uniform on [-40, 40] as `float` (`exp`).
    FloatExp,
    /// Uniform on [-40, 40] as `double`.
    DoubleExp,
}

impl Family {
    fn name(self) -> &'static str {
        match self {
            Family::FloatIntensity => "float, |x| log-uniform 2^-10..2^17",
            Family::DoubleIntensity => "double, |x| log-uniform 2^-10..2^17",
            Family::FloatUnit => "float, uniform [-1, 1]",
            Family::DoubleUnit => "double, uniform [-1, 1]",
            Family::FloatExp => "float, uniform [-40, 40]",
            Family::DoubleExp => "double, uniform [-40, 40]",
        }
    }

    fn is_float(self) -> bool {
        matches!(
            self,
            Family::FloatIntensity | Family::FloatUnit | Family::FloatExp
        )
    }

    fn arg(self, f: usize, i: u64, positive: bool) -> (f64, f64) {
        let mut s = i ^ ((f as u64) << 56) ^ ((self as u64) << 48) ^ 0x5DEE_CE66_D1CE_4E5B;
        let r = splitmix(&mut s);
        let u = (r >> 11) as f64 / (1u64 << 53) as f64;
        let sign = if positive || splitmix(&mut s) & 1 == 0 {
            1.0
        } else {
            -1.0
        };
        let x = match self {
            Family::FloatIntensity | Family::DoubleIntensity => {
                // 2^(-10 + 27 u) with a random mantissa
                let e = (r >> 58) as i32 % 27 - 10;
                let m = 1.0 + (splitmix(&mut s) >> 11) as f64 / (1u64 << 53) as f64;
                sign * m * 2f64.powi(e)
            }
            Family::FloatUnit | Family::DoubleUnit => u * 2.0 - 1.0,
            Family::FloatExp | Family::DoubleExp => u * 80.0 - 40.0,
        };
        let y = ((splitmix(&mut s) >> 11) as f64 / (1u64 << 53) as f64) * 6.0 - 3.0;
        if self.is_float() {
            (f64::from(x as f32), f64::from(y as f32))
        } else {
            (x, y)
        }
    }
}

/// The families each function is measured on.
fn families(name: &str) -> &'static [Family] {
    match name {
        "asin" | "acos" => &[Family::FloatUnit, Family::DoubleUnit],
        "exp" => &[Family::FloatExp, Family::DoubleExp],
        _ => &[Family::FloatIntensity, Family::DoubleIntensity],
    }
}

/// Counts of one measurement: arguments, double results that differ, `float` results that
/// differ.
#[derive(Default)]
struct Count {
    n: u64,
    double: u64,
    float: u64,
}

fn measure_one(pair: &Pair, f: usize, fam: Family, n: u64, threads: usize) -> Count {
    let (name, glibc, cr) = *pair;
    let positive = matches!(name, "log" | "pow");
    let d = AtomicU64::new(0);
    let fl = AtomicU64::new(0);
    let per = n.div_ceil(threads as u64);
    std::thread::scope(|sc| {
        for t in 0..threads as u64 {
            let (d, fl) = (&d, &fl);
            sc.spawn(move || {
                let (mut a, mut b) = (0u64, 0u64);
                for i in t * per..((t + 1) * per).min(n) {
                    let (x, y) = fam.arg(f, i, positive);
                    let (g, c) = (glibc(x, y), cr(x, y));
                    if !same(g, c) {
                        a += 1;
                        b += u64::from(!same(f64::from(g as f32), f64::from(c as f32)));
                    }
                }
                d.fetch_add(a, Ordering::Relaxed);
                fl.fetch_add(b, Ordering::Relaxed);
            });
        }
    });
    Count {
        n,
        double: d.into_inner(),
        float: fl.into_inner(),
    }
}

/// Every integer of an int16 image's range (positive ones only for `log` and `pow`; `exp`'s
/// finite range), with `pow`'s exponent 0.5.
fn measure_integers(pair: &Pair) -> Count {
    let (name, glibc, cr) = *pair;
    let (lo, hi) = match name {
        "log" | "pow" => (1i32, 32767),
        "exp" => (-745, 709),
        "asin" | "acos" => (-1, 1),
        _ => (-32768, 32767),
    };
    let mut c = Count::default();
    for k in lo..=hi {
        let x = f64::from(k);
        let (g, r) = (glibc(x, 0.5), cr(x, 0.5));
        c.n += 1;
        if !same(g, r) {
            c.double += 1;
            c.float += u64::from(!same(f64::from(g as f32), f64::from(r as f32)));
        }
    }
    c
}

fn percent(a: u64, n: u64) -> String {
    format!("{:.4} %", 100.0 * a as f64 / n.max(1) as f64)
}

fn measure(log2n: u32, threads: usize) {
    let n = 1u64 << log2n;
    println!("function | arguments | n | double results differ | float results differ");
    for (f, pair) in PAIRS.iter().enumerate() {
        for &fam in families(pair.0) {
            let c = measure_one(pair, f, fam, n, threads);
            let fl = if fam.is_float() {
                format!("{} ({})", c.float, percent(c.float, c.n))
            } else {
                "-".to_string()
            };
            println!(
                "{} | {} | {} | {} ({}) | {fl}",
                pair.0,
                fam.name(),
                c.n,
                c.double,
                percent(c.double, c.n)
            );
        }
        let c = measure_integers(pair);
        println!(
            "{} | every integer of int16's range | {} | {} ({}) | {} ({})",
            pair.0,
            c.n,
            c.double,
            percent(c.double, c.n),
            c.float,
            percent(c.float, c.n)
        );
    }
}

unsafe extern "C" {
    fn dlopen(filename: *const std::ffi::c_char, flag: std::ffi::c_int) -> *mut std::ffi::c_void;
    fn dlsym(
        handle: *mut std::ffi::c_void,
        symbol: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_void;
}

type Fn64 = extern "C" fn(f64) -> f64;

/// A function of the shared library compiled from CORE-MATH's C (`build.sh`).
fn core_math(lib: &str, symbol: &std::ffi::CStr) -> Fn64 {
    let c = std::ffi::CString::new(lib).unwrap();
    // SAFETY: the dynamic loader's calls with NUL-terminated names; the symbol is the C
    // function `double cr_<name>(double)`.
    unsafe {
        let h = dlopen(c.as_ptr(), 2);
        assert!(!h.is_null(), "dlopen({lib})");
        let f = dlsym(h, symbol.as_ptr());
        assert!(!f.is_null(), "dlsym({symbol:?})");
        std::mem::transmute::<*mut std::ffi::c_void, Fn64>(f)
    }
}

/// `strtod` of a C99 hexadecimal float (`0x1.8p-3`), as the `.wc` files write them.
fn parse_hex_float(s: &str) -> f64 {
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let s = s.strip_prefix("0x").expect("hex float");
    let (mant, exp) = s.split_once('p').unwrap_or((s, "0"));
    let (int, frac) = mant.split_once('.').unwrap_or((mant, ""));
    let mut m: u128 = u128::from_str_radix(int, 16).unwrap_or(0);
    for ch in frac.chars() {
        m = (m << 4) | u128::from(ch.to_digit(16).unwrap());
    }
    let e: i32 = exp.parse::<i32>().unwrap() - 4 * frac.len() as i32;
    // the worst cases fit in 53 bits: exact
    let v = (m as f64) * 2f64.powi(e);
    if neg { -v } else { v }
}

fn worst_cases(path: &str) -> Vec<f64> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|line| {
            let w = line.split_whitespace().next()?;
            let w = w.strip_prefix('+').unwrap_or(w);
            (w.starts_with("0x") || w.starts_with("-0x")).then(|| parse_hex_float(w))
        })
        .collect()
}

/// A random argument of family `f` for the verification: random bit patterns (every finite
/// double, NaN and infinities included), uniform on (-10, 10), uniform on (-2^30, 2^30), and a
/// log-uniform magnitude from 2^-30 to 2^1023.
fn any_arg(f: u32, i: u64) -> f64 {
    let mut s = i ^ (u64::from(f) << 56) ^ 0x2545_F491_4F6C_DD1D;
    let r = splitmix(&mut s);
    let u = (r >> 11) as f64 / (1u64 << 53) as f64;
    match f {
        0 => f64::from_bits(r),
        1 => u * 20.0 - 10.0,
        2 => (u * 2.0 - 1.0) * 1_073_741_824.0,
        _ => {
            let e = 0x3ff - 30 + (r >> 52) % 1053;
            let m = splitmix(&mut s) & 0x000f_ffff_ffff_ffff;
            f64::from_bits(((r & 1) << 63) | (e << 52) | m)
        }
    }
}

fn coremath(lib: &str, tan_wc: &str, atan_wc: &str, log2n: u32, threads: usize) -> bool {
    let c_tan = core_math(lib, c"cr_tan");
    let c_atan = core_math(lib, c"cr_atan");
    let ours: [(&str, fn(f64) -> f64, Fn64, &str); 2] = [
        ("tan", math::tan, c_tan, tan_wc),
        ("atan", |x| math::atan2(x, 1.0), c_atan, atan_wc),
    ];
    let mut pass = true;
    for (name, f, c, wc) in ours {
        let worst = worst_cases(wc);
        let mut differ = 0u64;
        let mut glibc_off = 0u64;
        let g = PAIRS.iter().find(|p| p.0 == name).unwrap().1;
        for &x in &worst {
            for x in [x, -x] {
                let b = c(x);
                if !same(f(x), b) {
                    differ += 1;
                    if differ <= 10 {
                        println!(
                            "  {name} wc x = {x:e}: ours {:016x}, C {:016x}",
                            f(x).to_bits(),
                            b.to_bits()
                        );
                    }
                }
                glibc_off += u64::from(!same(g(x, 0.0), b));
            }
        }
        println!(
            "{name}: worst cases: {} inputs (both signs), {differ} differ from CORE-MATH's C; glibc not correctly rounded on {glibc_off}",
            2 * worst.len()
        );
        pass &= differ == 0;
        let n = 1u64 << log2n;
        for fam in 0..4u32 {
            let rdiff = AtomicU64::new(0);
            let per = n.div_ceil(threads as u64);
            std::thread::scope(|sc| {
                for t in 0..threads as u64 {
                    let rdiff = &rdiff;
                    sc.spawn(move || {
                        let mut d = 0u64;
                        for i in t * per..((t + 1) * per).min(n) {
                            let x = any_arg(fam, i);
                            d += u64::from(!same(f(x), c(x)));
                        }
                        rdiff.fetch_add(d, Ordering::Relaxed);
                    });
                }
            });
            let d = rdiff.into_inner();
            println!("{name}: random family {fam}: {n} inputs, {d} differ from CORE-MATH's C");
            pass &= d == 0;
        }
    }
    println!("{}", if pass { "VERIFY-PASS" } else { "VERIFY-FAIL" });
    pass
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opt = |name: &str, default: usize| -> usize {
        args.iter()
            .position(|a| a == name)
            .and_then(|k| args.get(k + 1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    let threads = opt("--threads", 14);
    match args.get(1).map(String::as_str) {
        Some("measure") => measure(opt("--log2n", 25) as u32, threads),
        Some("coremath") if args.len() >= 5 => {
            let log2n = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(24);
            if !coremath(&args[2], &args[3], &args[4], log2n, threads) {
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!(
                "usage: glibc-double-maths measure [--log2n N] [--threads T]\n       \
                 glibc-double-maths coremath <libcrmath.so> <tan.wc> <atan.wc> [log2n] [--threads T]"
            );
            std::process::exit(2);
        }
    }
}
