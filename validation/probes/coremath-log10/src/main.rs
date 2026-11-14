// SPDX-License-Identifier: Apache-2.0
//! `larmorx_core::math::log10` (the port of CORE-MATH's `cr_log10`) against CORE-MATH's C, and
//! how often the C library's `log10` (glibc 2.35's, a black box here) differs from the correctly
//! rounded result; `larmorx_core::math::log10_glibc` is checked against the C library's `log10`
//! on the same arguments.
//!
//! `coremath-log10 <libcrlog10.so> [log2n] [--threads T] [--wc <log10.wc>]`: on special values,
//! on every hard-to-round case of CORE-MATH's `log10.wc` at `x` and `-x`, and on `2^log2n`
//! arguments of each family (random bit patterns; uniform on (0, 1000); near 1, where the
//! accurate path runs; log-uniform normal numbers; subnormal numbers; the powers of ten from
//! 1e-323 to 1e308 moved by -3 to 3 units in the last place; p-values, uniform on (0, 1) and
//! scaled down by up to 2^-1000; integers and dyadic fractions), the bits of
//! `larmorx_core::math::log10`, of the C's `cr_log10` and of glibc's `log10`. NaN results equal
//! any NaN, except that the C's `0.0 / 0.0` for `x < 0` is the processor's default NaN while
//! larmorx gives `0x7ff8000000000000` (counted apart). Prints the counts, the seeded hash of
//! `larmorx-core`'s unit test computed from the C, and `VERIFY-PASS` (larmorx equals the C on
//! every argument and `log10_glibc` equals glibc's) or `VERIFY-FAIL` (exit status 1). Meaningful
//! on x86-64 Linux with glibc 2.35 (the oracle machine).

use std::sync::atomic::{AtomicU64, Ordering};

use larmorx_core::math;

unsafe extern "C" {
    // The C library's log10 (libm), a black box.
    fn log10(x: f64) -> f64;
    fn dlopen(filename: *const std::ffi::c_char, flag: std::ffi::c_int) -> *mut std::ffi::c_void;
    fn dlsym(
        handle: *mut std::ffi::c_void,
        symbol: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_void;
}

type Fn1 = extern "C" fn(f64) -> f64;

fn glibc_log10(x: f64) -> f64 {
    // SAFETY: libm's `double log10(double)`.
    unsafe { log10(x) }
}

/// `cr_log10` of the shared library compiled from CORE-MATH's C (`build.sh`).
fn core_math(lib: &str) -> Fn1 {
    let c = std::ffi::CString::new(lib).unwrap();
    // SAFETY: the dynamic loader's calls with NUL-terminated names; the symbol is the C function
    // `double cr_log10(double)`.
    unsafe {
        let h = dlopen(c.as_ptr(), 2);
        assert!(!h.is_null(), "dlopen({lib})");
        let f = dlsym(h, c"cr_log10".as_ptr());
        assert!(!f.is_null(), "dlsym(cr_log10)");
        std::mem::transmute::<*mut std::ffi::c_void, Fn1>(f)
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

/// 2^k exactly, for -1022 <= k <= 1023.
fn pow2(k: i32) -> f64 {
    assert!((-1022..=1023).contains(&k), "{k}");
    f64::from_bits(((k + 1023) as u64) << 52)
}

/// `v` moved by `d - 3` units in the last place (`d` in 0..7), within the positive numbers.
fn bump(v: f64, d: u64) -> f64 {
    f64::from_bits((v.to_bits() as i64 + d as i64 - 3).max(1) as u64)
}

/// The `i`-th argument of family `fam`.
fn arg(fam: usize, i: u64) -> f64 {
    let a = splitmix64(i.wrapping_mul(16).wrapping_add(fam as u64));
    let b = splitmix64(a ^ 0x5851_f42d_4c95_7f2d);
    match fam {
        0 => f64::from_bits(a),
        1 => unit(a) * 1e3,
        2 => {
            // 1 + t with |t| from 2^-53 to 2^-1
            let t = (unit(a) * 2.0 - 1.0) * pow2(-1 - (b % 52) as i32);
            1.0 + t
        }
        3 => f64::from_bits(((1 + b % 2046) << 52) | (a & 0x000f_ffff_ffff_ffff)),
        4 => f64::from_bits((a & 0x000f_ffff_ffff_ffff).max(1)),
        5 => {
            let k = (a % 632) as i32 - 323;
            let p: f64 = format!("1e{k}").parse().unwrap();
            bump(p, b % 7)
        }
        6 => {
            let p = unit(a).max(f64::from_bits(0x3ca0_0000_0000_0000));
            if b & 1 == 0 {
                p
            } else {
                p * pow2(-(((b >> 1) % 1000) as i32))
            }
        }
        _ => {
            let n = (a % (1u64 << 40)).max(1) as f64;
            n * pow2(-((b % 64) as i32))
        }
    }
}

const FAMILIES: [&str; 8] = [
    "random bit patterns",
    "uniform on (0, 1000)",
    "near 1 (the accurate path)",
    "log-uniform normal numbers",
    "subnormal numbers",
    "powers of ten +-3 ulp",
    "p-values",
    "integers and dyadic fractions",
];

/// Counts of one comparison.
#[derive(Default)]
struct Counts {
    n: u64,
    /// larmorx's `log10` differs from the C (NaN equals NaN).
    bad: u64,
    /// The NaN for `x < 0`: the C's default NaN, larmorx's `0x7ff8...`.
    nan_bits: u64,
    /// glibc's `log10` differs from the correctly rounded C (NaN equals NaN).
    glibc_not_cr: u64,
    /// larmorx's `log10_glibc` differs from glibc's `log10` (bits, NaNs included).
    glibc_port_bad: u64,
}

fn compare(cr: Fn1, x: f64, c: &mut Counts, label: &str) {
    let (l, r, g, lg) = (math::log10(x), cr(x), glibc_log10(x), math::log10_glibc(x));
    c.n += 1;
    let same = l.to_bits() == r.to_bits() || (l.is_nan() && r.is_nan());
    if !same {
        if c.bad < 4 {
            println!(
                "  DIFF {label} {:#018x}: larmorx {:#018x}, C {:#018x}",
                x.to_bits(),
                l.to_bits(),
                r.to_bits()
            );
        }
        c.bad += 1;
    } else if l.to_bits() != r.to_bits() {
        c.nan_bits += 1;
    }
    c.glibc_not_cr += u64::from(!(g.to_bits() == r.to_bits() || (g.is_nan() && r.is_nan())));
    if lg.to_bits() != g.to_bits() {
        if c.glibc_port_bad < 4 {
            println!(
                "  DIFF {label} {:#018x}: log10_glibc {:#018x}, glibc {:#018x}",
                x.to_bits(),
                lg.to_bits(),
                g.to_bits()
            );
        }
        c.glibc_port_bad += 1;
    }
}

fn report(name: &str, c: &Counts) {
    println!(
        "{name}: {} arguments, larmorx differs from the C on {} ({} NaN-bit only: x < 0), glibc's log10 is not correctly rounded on {} ({:.4} %), log10_glibc differs from glibc's on {}",
        c.n,
        c.bad,
        c.nan_bits,
        c.glibc_not_cr,
        100.0 * c.glibc_not_cr as f64 / c.n.max(1) as f64,
        c.glibc_port_bad
    );
}

/// The arguments of `larmorx_core::math::log10`'s unit test `seeded_hash` (those of
/// `log10_glibc`'s): SplitMix64 from 0x123456789abcdef0, one third uniform on (0, 1000), one
/// third random bit patterns, one third subnormal or zero.
fn seeded_hash(f: impl Fn(f64) -> u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut s = 0x1234_5678_9abc_def0u64;
    for i in 0..65_536u64 {
        s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        let r = z ^ (z >> 31);
        let x = match i % 3 {
            0 => (r >> 11) as f64 / (1u64 << 53) as f64 * 1e3,
            1 => f64::from_bits(r),
            _ => f64::from_bits(r & 0x000f_ffff_ffff_ffff),
        };
        h = (h ^ f(x)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lib = args
        .get(1)
        .expect("usage: coremath-log10 <libcrlog10.so> [log2n] [--threads T] [--wc <log10.wc>]");
    let log2n: u32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(24);
    let opt = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let threads: u64 = opt("--threads").and_then(|v| v.parse().ok()).unwrap_or(1);
    let cr = core_math(lib);
    let mut fail = false;

    let specials = [
        0.0,
        -0.0,
        f64::from_bits(1),
        f64::from_bits(2),
        f64::from_bits(0x000f_ffff_ffff_ffff),
        f64::MIN_POSITIVE,
        1.0,
        -1.0,
        10.0,
        0.1,
        1e22,
        1e23,
        f64::MAX,
        -f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        -f64::NAN,
        f64::from_bits(0x7ff0_0000_0000_0001), // signalling NaNs
        f64::from_bits(0xfff0_0000_0000_0123),
        f64::from_bits(0x3ff0_0000_0000_0001),
        f64::from_bits(0x3fef_ffff_ffff_ffff),
    ];
    let mut c = Counts::default();
    for &x in &specials {
        compare(cr, x, &mut c, "special");
    }
    for n in 0..=22 {
        compare(cr, format!("1e{n}").parse().unwrap(), &mut c, "10^n");
    }
    report("special values and 10^n", &c);
    fail |= c.bad > 0 || c.glibc_port_bad > 0;

    if let Some(path) = opt("--wc") {
        let text = std::fs::read_to_string(&path).expect("read the .wc file");
        let mut c = Counts::default();
        let mut parsed = 0u64;
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            let Some(x) = parse_hex_float(t) else {
                continue;
            };
            parsed += 1;
            compare(cr, x, &mut c, "wc");
            compare(cr, -x, &mut c, "wc-neg");
        }
        report(&format!("log10.wc ({parsed} cases, at x and -x)"), &c);
        fail |= c.bad > 0 || c.glibc_port_bad > 0;
    }

    let total = 1u64 << log2n;
    for (fam, name) in FAMILIES.iter().enumerate() {
        let sums: [AtomicU64; 5] = Default::default();
        let chunk = total.div_ceil(threads);
        std::thread::scope(|sc| {
            for t in 0..threads {
                let sums = &sums;
                sc.spawn(move || {
                    let mut c = Counts::default();
                    for i in t * chunk..((t + 1) * chunk).min(total) {
                        compare(cr, arg(fam, i), &mut c, name);
                    }
                    for (s, v) in
                        sums.iter()
                            .zip([c.n, c.bad, c.nan_bits, c.glibc_not_cr, c.glibc_port_bad])
                    {
                        s.fetch_add(v, Ordering::Relaxed);
                    }
                });
            }
        });
        let v: Vec<u64> = sums.iter().map(|s| s.load(Ordering::Relaxed)).collect();
        let c = Counts {
            n: v[0],
            bad: v[1],
            nan_bits: v[2],
            glibc_not_cr: v[3],
            glibc_port_bad: v[4],
        };
        report(name, &c);
        fail |= c.bad > 0 || c.glibc_port_bad > 0;
    }

    // The unit test's hash from the C, its NaN for x < 0 written as larmorx's (0x7ff8...).
    let hc = seeded_hash(|x| {
        if x < 0.0 {
            f64::NAN.to_bits()
        } else {
            cr(x).to_bits()
        }
    });
    let hl = seeded_hash(|x| math::log10(x).to_bits());
    println!("seeded hash: C {hc:#018x}, larmorx {hl:#018x}");
    fail |= hc != hl;
    println!("{}", if fail { "VERIFY-FAIL" } else { "VERIFY-PASS" });
    if fail {
        std::process::exit(1);
    }
}

/// A C hexadecimal floating literal (`0x1.8p-3`, `-0x1p0`, an optional sign), exactly.
fn parse_hex_float(s: &str) -> Option<f64> {
    let (neg, s) = match s.as_bytes().first()? {
        b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => (false, s),
    };
    let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
    let (mant, exp) = s.split_once(['p', 'P'])?;
    let exp: i32 = exp.parse().ok()?;
    let (int, frac) = mant.split_once('.').unwrap_or((mant, ""));
    let digits = format!("{int}{frac}");
    if digits.len() > 15 {
        return None;
    }
    let m = u64::from_str_radix(&digits, 16).ok()?;
    // m * 2^(exp - 4*len(frac)): the cases' significands fit in 53 bits, so `m as f64` is exact,
    // and the two scalings by powers of two are exact (one rounding at most, of a value that is
    // a double: the literal is one)
    let e = exp - 4 * frac.len() as i32;
    let v = m as f64 * pow2(e / 2) * pow2(e - e / 2);
    Some(if neg { -v } else { v })
}
