// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! Exhaustive comparison of `larmorx_gpl_afni::glibc_sincosf` with the platform's C library.
//!
//! `f32::sin` and `f32::cos` call the C library's `sinf` and `cosf`. Run on Linux with glibc
//! 2.35 on an x86-64 CPU with FMA (the machine AFNI's oracle runs on), every one of the 2^32
//! inputs must give the same bits as the port. The tool also reports where glibc's SSE2 build
//! would differ, and where glibc is not correctly rounded.
//!
//! Usage: `sincosf-verify [--threads N] [--vectors N]`. With `--vectors`, prints up to N
//! inputs in [0, 32] (the arguments 3dTshift's weighted sinc uses) where glibc's result is
//! not the correctly rounded one, as Rust test vectors. Exits 0 when the port matches the
//! platform everywhere.
#![forbid(unsafe_code)]

use larmorx_gpl_afni::glibc_sincosf::{cosf, sinf, verification};
use rayon::prelude::*;

#[derive(Default, Clone, Copy)]
struct Counts {
    port_vs_libc: [u64; 2],
    sse2_vs_libc: [u64; 2],
    libc_not_cr: [u64; 2],
    libc_not_cr_0_32: [u64; 2],
}

impl Counts {
    fn add(mut self, o: Counts) -> Counts {
        for i in 0..2 {
            self.port_vs_libc[i] += o.port_vs_libc[i];
            self.sse2_vs_libc[i] += o.sse2_vs_libc[i];
            self.libc_not_cr[i] += o.libc_not_cr[i];
            self.libc_not_cr_0_32[i] += o.libc_not_cr_0_32[i];
        }
        self
    }
}

fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

/// The correctly rounded double result rounded to float (a double rounding, which differs
/// from the correctly rounded float only in rare ties; good enough to flag glibc's misses).
fn cr(f: fn(f64) -> f64, x: f32) -> f32 {
    f(f64::from(x)) as f32
}

fn check(bits: u32) -> Counts {
    let x = f32::from_bits(bits);
    let mut c = Counts::default();
    let libc = [x.sin(), x.cos()];
    let port = [sinf(x), cosf(x)];
    let sse2 = [verification::sinf_sse2(x), verification::cosf_sse2(x)];
    let exact = [
        cr(larmorx_core::math::sin, x),
        cr(larmorx_core::math::cos, x),
    ];
    for i in 0..2 {
        c.port_vs_libc[i] += u64::from(!same(port[i], libc[i]));
        c.sse2_vs_libc[i] += u64::from(!same(sse2[i], libc[i]));
        if x.is_finite() && !same(libc[i], exact[i]) {
            c.libc_not_cr[i] += 1;
            if (0.0..=32.0).contains(&x) {
                c.libc_not_cr_0_32[i] += 1;
            }
        }
    }
    c
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map(|v| v.parse::<usize>().expect("a number"))
    };
    let threads = value("--threads").unwrap_or(0);
    if let Some(n) = value("--vectors") {
        print_vectors(n);
        return;
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("thread pool");
    let total = pool.install(|| {
        (0u32..=u16::MAX as u32)
            .into_par_iter()
            .map(|hi| {
                (0u32..=u16::MAX as u32)
                    .map(|lo| check((hi << 16) | lo))
                    .fold(Counts::default(), Counts::add)
            })
            .reduce(Counts::default, Counts::add)
    });
    println!("inputs: all 2^32 floats");
    for (i, name) in ["sinf", "cosf"].iter().enumerate() {
        println!(
            "{name}: port (FMA build) vs C library: {} differ; SSE2 build vs C library: {} differ; \
             C library not correctly rounded: {} inputs ({} in [0, 32])",
            total.port_vs_libc[i],
            total.sse2_vs_libc[i],
            total.libc_not_cr[i],
            total.libc_not_cr_0_32[i]
        );
    }
    let ok = total.port_vs_libc == [0, 0];
    println!(
        "{}",
        if ok {
            "PORT MATCHES THE C LIBRARY ON EVERY INPUT"
        } else {
            "MISMATCHES FOUND"
        }
    );
    std::process::exit(if ok { 0 } else { 1 });
}

/// Inputs in [0, 32] where the C library's sinf or cosf is not correctly rounded, plus the
/// branch limits, as `(input bits, sinf bits, cosf bits)` for the unit tests.
fn print_vectors(n: usize) {
    let mut picked = Vec::new();
    let limits = [
        0x3980_0000u32, // 0x1p-12
        0x397f_ffff,
        0x3f49_0fdb, // pi/4
        0x3f49_0fda,
        0x42f0_0000, // 120
        0x42ef_ffff,
    ];
    picked.extend(limits);
    let (lo, hi) = (0.01f32.to_bits(), 32.0f32.to_bits());
    let step = ((hi - lo) / n.max(1) as u32).max(1);
    let mut b = lo;
    while b <= hi && picked.len() < n + limits.len() {
        // Scan a block for a miss, then jump ahead, so the vectors spread over [0, 32].
        for k in b..(b + step).min(hi + 1) {
            let x = f32::from_bits(k);
            let miss = !same(x.sin(), cr(larmorx_core::math::sin, x))
                || !same(x.cos(), cr(larmorx_core::math::cos, x));
            if miss {
                picked.push(k);
                break;
            }
        }
        b += step;
    }
    for k in picked {
        let x = f32::from_bits(k);
        println!(
            "    (0x{k:08x}, 0x{:08x}, 0x{:08x}),",
            x.sin().to_bits(),
            x.cos().to_bits()
        );
    }
}
