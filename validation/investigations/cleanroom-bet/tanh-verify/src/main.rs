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

fn main() {
    let args: Vec<String> = std::env::args().collect();
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
