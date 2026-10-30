// SPDX-License-Identifier: Apache-2.0
//! `larmorx_core::math::tanh_glibc` (fdlibm's `tanh` over fdlibm's `expm1`, `specs/bet.md`
//! §8.1) against the system's `tanh` (the C library's, called as a black box: its code is never
//! read), bit for bit, on a large random and edge sample.
//!
//! - `verify [--log2n N] [--threads T]`: `2^N` inputs of each random family (default 2^28 each):
//!   bit patterns with a log-uniform magnitude from 2^-80 to 2^6, both signs; values uniform in
//!   (-25, 25); values uniform in (-1, 1); and every input within 2^20 units in the last place
//!   of each branch threshold of the method (2^-55, 1, 22 for `tanh`; for `expm1`'s argument
//!   `2|x|`: 2^-54, 0.5 ln 2, 1.5 ln 2, 20 ln 2, 56 ln 2), both signs; the special values.
//!   Prints the counts and `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1).
//! - `record`: the unit tests' recorded values (`(x, tanh(x))` as bits) and the sequence hash
//!   (`crates/larmorx-core/src/math/tanh_glibc.rs`).
//! - `bench`: nanoseconds per call (this one, the system's).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use larmorx_core::math::tanh_glibc;

unsafe extern "C" {
    /// The C library's `tanh` (libm), a black box.
    fn tanh(x: f64) -> f64;
}

fn sys_tanh(x: f64) -> f64 {
    // SAFETY: a pure function of one double.
    unsafe { tanh(x) }
}

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

/// The input of family `f` at index `i` (deterministic).
fn input(f: u32, i: u64) -> f64 {
    let mut s = i ^ (u64::from(f) << 56) ^ 0x5DEE_CE66_D1CE_4E5B;
    let r = splitmix(&mut s);
    match f {
        0 => {
            // log-uniform magnitude 2^-80 .. 2^6, random mantissa, random sign
            let e = 0x3ff - 80 + (r >> 52) % 86;
            let m = r & 0x000f_ffff_ffff_ffff;
            let sign = (splitmix(&mut s) & 1) << 63;
            f64::from_bits(sign | (e << 52) | m)
        }
        1 => ((r >> 11) as f64 / (1u64 << 53) as f64) * 50.0 - 25.0,
        _ => ((r >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0,
    }
}

fn thresholds() -> Vec<f64> {
    let ln2 = std::f64::consts::LN_2;
    vec![
        f64::from_bits(0x3c80_0000_0000_0000), // 2^-55
        1.0,
        22.0,
        f64::from_bits(0x3c90_0000_0000_0000) / 2.0, // 2|x| = 2^-54
        0.5 * ln2 / 2.0,
        1.5 * ln2 / 2.0,
        20.0 * ln2 / 2.0,
        56.0 * ln2 / 2.0,
    ]
}

fn verify(log2n: u32, threads: usize) -> bool {
    let n = 1u64 << log2n;
    let differ = AtomicU64::new(0);
    let first_differ = std::sync::Mutex::new(Vec::<(f64, f64, f64)>::new());
    let t0 = Instant::now();
    for f in 0..3u32 {
        let per = n.div_ceil(threads as u64);
        std::thread::scope(|sc| {
            for t in 0..threads as u64 {
                let differ = &differ;
                let first = &first_differ;
                sc.spawn(move || {
                    let lo = t * per;
                    let hi = ((t + 1) * per).min(n);
                    let mut local = 0u64;
                    for i in lo..hi {
                        let x = input(f, i);
                        let a = tanh_glibc(x);
                        let b = sys_tanh(x);
                        if !same(a, b) {
                            local += 1;
                            let mut g = first.lock().unwrap();
                            if g.len() < 20 {
                                g.push((x, a, b));
                            }
                        }
                    }
                    differ.fetch_add(local, Ordering::Relaxed);
                });
            }
        });
        println!(
            "family {f}: {n} inputs, {} differ so far ({:.1} s)",
            differ.load(Ordering::Relaxed),
            t0.elapsed().as_secs_f64()
        );
    }
    // The thresholds' neighbourhoods, both signs.
    let mut edge = 0u64;
    for th in thresholds() {
        let b = th.to_bits();
        for d in -(1i64 << 20)..=(1i64 << 20) {
            let x = f64::from_bits(b.wrapping_add_signed(d));
            for x in [x, -x] {
                edge += 1;
                let (a, s) = (tanh_glibc(x), sys_tanh(x));
                if !same(a, s) {
                    differ.fetch_add(1, Ordering::Relaxed);
                    let mut g = first_differ.lock().unwrap();
                    if g.len() < 20 {
                        g.push((x, a, s));
                    }
                }
            }
        }
    }
    let specials = [
        0.0,
        -0.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::MAX,
        -f64::MAX,
        710.0,
        -710.0,
        40.0,
    ];
    for &x in &specials {
        edge += 1;
        if !same(tanh_glibc(x), sys_tanh(x)) {
            differ.fetch_add(1, Ordering::Relaxed);
            first_differ
                .lock()
                .unwrap()
                .push((x, tanh_glibc(x), sys_tanh(x)));
        }
    }
    let total = 3 * n + edge;
    let d = differ.load(Ordering::Relaxed);
    println!(
        "edges and specials: {edge} inputs; total {total} inputs, {d} differ ({:.1} s)",
        t0.elapsed().as_secs_f64()
    );
    for (x, a, b) in first_differ.lock().unwrap().iter() {
        println!(
            "  x = {x:e} ({:016x}): ours {:016x}, system {:016x}",
            x.to_bits(),
            a.to_bits(),
            b.to_bits()
        );
    }
    d == 0
}

/// FNV-1a over the result bits of the sequence's inputs (the unit test's).
fn sequence_hash(f: &dyn Fn(f64) -> f64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for i in 0..(1u64 << 20) {
        let x = input((i % 3) as u32, i);
        h ^= f(x).to_bits();
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn record() {
    let mut xs: Vec<f64> = vec![
        1e-20, -3e-17, 1e-9, 0.01, -0.1, 0.17, 0.3466, 0.52, -0.7, 0.99, 1.0, 1.5, -2.25, 3.0,
        5.5, 6.9, 7.1, 12.0, -19.4, 21.99,
    ];
    let mut s = 20261030u64;
    for _ in 0..12 {
        let r = splitmix(&mut s);
        xs.push(((r >> 11) as f64 / (1u64 << 53) as f64) * 40.0 - 20.0);
    }
    println!("    const RECORDED: &[(u64, u64)] = &[");
    for x in xs {
        println!(
            "        (0x{:016x}, 0x{:016x}),",
            x.to_bits(),
            sys_tanh(x).to_bits()
        );
    }
    println!("    ];");
    println!(
        "sequence hash: system {:016x}, tanh_glibc {:016x}",
        sequence_hash(&sys_tanh),
        sequence_hash(&tanh_glibc)
    );
}

fn bench() {
    let xs: Vec<f64> = (0..(1u64 << 22)).map(|i| input(1, i)).collect();
    for (name, f) in [
        ("tanh_glibc", &tanh_glibc as &dyn Fn(f64) -> f64),
        ("system tanh", &sys_tanh),
    ] {
        let mut best = f64::INFINITY;
        for _ in 0..5 {
            let t = Instant::now();
            let mut acc = 0.0;
            for &x in &xs {
                acc += f(x);
            }
            std::hint::black_box(acc);
            best = best.min(t.elapsed().as_secs_f64() * 1e9 / xs.len() as f64);
        }
        println!("{name}: {best:.2} ns per call");
    }
}

unsafe extern "C" {
    fn dlopen(filename: *const std::ffi::c_char, flag: std::ffi::c_int) -> *mut std::ffi::c_void;
    fn dlsym(handle: *mut std::ffi::c_void, symbol: *const std::ffi::c_char)
    -> *mut std::ffi::c_void;
}

type Fn64 = extern "C" fn(f64) -> f64;

/// CORE-MATH's `cr_tanh` from a shared library compiled from its C (`tanh.c`, `-O2
/// -ffp-contract=off -fPIC -shared`).
fn core_math(lib: &str) -> Fn64 {
    let c = std::ffi::CString::new(lib).unwrap();
    // SAFETY: the dynamic loader's calls with NUL-terminated names; the symbol is the C
    // function `double cr_tanh(double)`.
    unsafe {
        let h = dlopen(c.as_ptr(), 2);
        assert!(!h.is_null(), "dlopen({lib})");
        let f = dlsym(h, c"cr_tanh".as_ptr());
        assert!(!f.is_null(), "dlsym(cr_tanh)");
        std::mem::transmute::<*mut std::ffi::c_void, Fn64>(f)
    }
}

/// `larmorx_core::math::tanh` against CORE-MATH's C on its worst cases (both signs) and the
/// random families; counts where glibc's `tanh` is not the correctly rounded result.
fn coremath(lib: &str, wc: &str, log2n: u32, threads: usize) -> bool {
    use larmorx_core::math::tanh as cr;
    let c = core_math(lib);
    let mut worst: Vec<f64> = Vec::new();
    for line in std::fs::read_to_string(wc).unwrap().lines() {
        let w = line.split_whitespace().next().unwrap_or("");
        let w = w.strip_prefix('+').unwrap_or(w);
        if !w.starts_with("0x") && !w.starts_with("-0x") {
            continue;
        }
        worst.push(parse_hex_float(w));
    }
    let mut differ = 0u64;
    let mut glibc_off = 0u64;
    let mut shown = 0;
    for &x in &worst {
        for x in [x, -x] {
            let (a, b) = (cr(x), c(x));
            if !same(a, b) {
                differ += 1;
                if shown < 10 {
                    println!("  wc x = {x:e}: ours {:016x}, C {:016x}", a.to_bits(), b.to_bits());
                    shown += 1;
                }
            }
            glibc_off += u64::from(!same(sys_tanh(x), b));
        }
    }
    println!(
        "worst cases: {} inputs (both signs), {differ} differ from CORE-MATH's C; glibc not correctly rounded on {glibc_off}",
        2 * worst.len()
    );
    let n = 1u64 << log2n;
    let rdiff = AtomicU64::new(0);
    let roff = AtomicU64::new(0);
    for f in 0..3u32 {
        let per = n.div_ceil(threads as u64);
        std::thread::scope(|sc| {
            for t in 0..threads as u64 {
                let (rdiff, roff) = (&rdiff, &roff);
                sc.spawn(move || {
                    let (mut d, mut o) = (0u64, 0u64);
                    for i in t * per..((t + 1) * per).min(n) {
                        let x = input(f, i);
                        let b = c(x);
                        d += u64::from(!same(cr(x), b));
                        o += u64::from(!same(sys_tanh(x), b));
                    }
                    rdiff.fetch_add(d, Ordering::Relaxed);
                    roff.fetch_add(o, Ordering::Relaxed);
                });
            }
        });
    }
    let rd = rdiff.load(Ordering::Relaxed);
    println!(
        "random: {} inputs, {rd} differ from CORE-MATH's C; glibc not correctly rounded on {}",
        3 * n,
        roff.load(Ordering::Relaxed)
    );
    // the unit test's values: a few of each branch and the first worst cases
    let mut rec: Vec<f64> = vec![1e-30, 1e-9, 0.1, 0.2, 0.3, 1.0, 2.5, 3.6, 3.7, 5.0, 12.0, 19.0];
    rec.extend(worst.iter().step_by((worst.len() / 24).max(1)).take(24));
    println!("    const RECORDED: &[(u64, u64)] = &[");
    for x in rec {
        println!("        (0x{:016x}, 0x{:016x}),", x.to_bits(), c(x).to_bits());
    }
    println!("    ];");
    differ == 0 && rd == 0
}

/// A C hex-float literal (`0x1.8p-3`, `-0x1p+0`).
fn parse_hex_float(s: &str) -> f64 {
    let (neg, s) = s.strip_prefix('-').map_or((false, s), |r| (true, r));
    let s = &s[2..];
    let (mant, exp) = s.split_once(['p', 'P']).unwrap();
    let (int, frac) = mant.split_once('.').unwrap_or((mant, ""));
    let mut m: u128 = if int.is_empty() {
        0
    } else {
        u128::from_str_radix(int, 16).unwrap()
    };
    for ch in frac.chars() {
        m = m * 16 + u128::from(ch.to_digit(16).unwrap());
    }
    let e: i32 = exp.parse::<i32>().unwrap() - 4 * frac.len() as i32;
    // m < 2^64 here: rounded once to a double (as C's strtod), then scaled exactly in steps
    let mut v = m as f64;
    let mut e = e;
    while e < -1000 {
        v *= 2f64.powi(-1000);
        e += 1000;
    }
    while e > 1000 {
        v *= 2f64.powi(1000);
        e -= 1000;
    }
    v *= 2f64.powi(e);
    if neg { -v } else { v }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("coremath") {
        let log2n = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(24);
        let ok = coremath(&args[2], &args[3], log2n, 16);
        println!("{}", if ok { "VERIFY-PASS" } else { "VERIFY-FAIL" });
        std::process::exit(i32::from(!ok));
    }
    let mut log2n = 28u32;
    let mut threads = 16usize;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--log2n" => {
                log2n = args[i + 1].parse().unwrap();
                i += 1;
            }
            "--threads" => {
                threads = args[i + 1].parse().unwrap();
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    match args.get(1).map(String::as_str) {
        Some("verify") => {
            if verify(log2n, threads) {
                println!("VERIFY-PASS");
            } else {
                println!("VERIFY-FAIL");
                std::process::exit(1);
            }
        }
        Some("record") => record(),
        Some("bench") => bench(),
        _ => {
            eprintln!("usage: tanh-glibc-verify verify [--log2n N] [--threads T] | record | bench");
            std::process::exit(2);
        }
    }
}
