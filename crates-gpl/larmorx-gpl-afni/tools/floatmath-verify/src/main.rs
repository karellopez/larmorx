// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! Exhaustive comparison of `larmorx_gpl_afni::glibc_floatmath` with glibc.
//!
//! - Default: `tanhf` against the C library's, on all 2^32 inputs, input by input (in a Rust
//!   program the symbol `tanhf` is glibc's on Linux).
//! - `--checksums`: a hash of the port's results over all inputs, for `tanhf` and `cbrtf`, to
//!   compare with `glibc_reference.c checksums`. `cbrtf` is checked this way only: in a Rust
//!   program the symbol `cbrtf` resolves to the Rust standard library's own, correctly rounded,
//!   cube root.
//!
//! Usage: `floatmath-verify [--threads N] [--checksums]`. Meaningful on Linux with glibc 2.35
//! on x86-64 (the machine AFNI's oracle runs on). Exits 0 when the port matches.
#![deny(unsafe_code)]

use larmorx_gpl_afni::glibc_floatmath::{cbrtf, tanhf};
use rayon::prelude::*;

/// The C library's `tanhf` (a development tool: the product never calls it).
#[allow(unsafe_code)]
mod libc {
    unsafe extern "C" {
        fn tanhf(x: f32) -> f32;
    }

    pub fn tanh(x: f32) -> f32 {
        // SAFETY: a pure function of a float, from the C library std already links.
        unsafe { tanhf(x) }
    }
}

fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

/// `(port differs from glibc, glibc differs from tanh in double rounded to float)`.
fn check(bits: u32) -> (u64, u64) {
    let x = f32::from_bits(bits);
    let libc = libc::tanh(x);
    let near = f64::from(x).tanh() as f32;
    (
        u64::from(!same(tanhf(x), libc)),
        u64::from(x.is_finite() && !same(libc, near)),
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let threads = args
        .iter()
        .position(|a| a == "--threads")
        .and_then(|i| args.get(i + 1))
        .map_or(0, |v| v.parse::<usize>().expect("a number"));
    if args.iter().any(|a| a == "--checksums") {
        // FNV-1a-like over the 32-bit results in input order (NaN as 0x7fc00000), as
        // glibc_reference.c computes it.
        for (name, f) in [("tanhf", tanhf as fn(f32) -> f32), ("cbrtf", cbrtf)] {
            let mut h: u64 = 1_469_598_103_934_665_603;
            for k in 0..=u32::MAX {
                let y = f(f32::from_bits(k));
                let r = if y.is_nan() { 0x7fc0_0000 } else { y.to_bits() };
                h = (h ^ u64::from(r)).wrapping_mul(1_099_511_628_211);
            }
            println!("{name} checksum {h:016x}");
        }
        return;
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("thread pool");
    let (differ, not_cr) = pool.install(|| {
        (0u32..=u16::MAX as u32)
            .into_par_iter()
            .map(|hi| {
                (0u32..=u16::MAX as u32)
                    .map(|lo| check((hi << 16) | lo))
                    .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
            })
            .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
    });
    println!(
        "tanhf, all 2^32 inputs: port vs C library: {differ} differ; C library not correctly \
         rounded: {not_cr} inputs"
    );
    println!(
        "{}",
        if differ == 0 {
            "PORT MATCHES THE C LIBRARY ON EVERY INPUT"
        } else {
            "MISMATCHES FOUND"
        }
    );
    std::process::exit(i32::from(differ != 0));
}
