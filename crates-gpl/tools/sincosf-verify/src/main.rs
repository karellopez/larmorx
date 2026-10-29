// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! The proof that the replica's `sinf` and `cosf`, translated from Arm's Optimized Routines
//! v19.11 (MIT), give the same bits as the earlier translation from glibc 2.35 (LGPL), on all
//! 2^32 inputs of each function, for both of glibc's x86-64 builds (FMA and SSE2).
//!
//! The earlier translation was checked against glibc on all 2^32 inputs (the FMA build); with
//! it, the two translations prove each other. The tool also compares the new translation with
//! the C library of the machine it runs on (`f32::sin` and `f32::cos` call it): on Linux with
//! glibc 2.35 and an x86-64 CPU with FMA, glibc runs its FMA build; started with
//! `GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA` it runs its SSE2
//! build, which `--c-library sse2` compares with. No C is compiled.
//!
//! Usage: `sincosf-verify [--threads N] [--c-library fma|sse2|none] [--vectors N]`. With
//! `--vectors`, prints the C library's values at the branch limits and at up to N inputs in
//! [0, 32] where it is not correctly rounded, as the new module's test vectors. Exits 0 when
//! every comparison agrees on every input.
#![forbid(unsafe_code)]

use larmorx_gpl_afni::glibc_sincosf as glibc;
use larmorx_gpl_afni::sincosf as arm;
use rayon::prelude::*;

/// What is compared, input by input: (new FMA, earlier FMA), (new SSE2, earlier SSE2), and
/// (new build, C library), each for sinf and cosf.
#[derive(Default, Clone, Copy)]
struct Counts {
    fma: [u64; 2],
    sse2: [u64; 2],
    c_library: [u64; 2],
    /// Where the two builds themselves differ (what makes the C library's build visible).
    builds: [u64; 2],
    first: [Option<u32>; 3],
}

impl Counts {
    fn add(mut self, o: Counts) -> Counts {
        for i in 0..2 {
            self.fma[i] += o.fma[i];
            self.sse2[i] += o.sse2[i];
            self.c_library[i] += o.c_library[i];
            self.builds[i] += o.builds[i];
        }
        for k in 0..3 {
            self.first[k] = match (self.first[k], o.first[k]) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
        }
        self
    }
}

/// The same bits, or two NaNs (NaN payloads are not part of either function's definition).
fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

#[derive(Clone, Copy, PartialEq)]
enum CLibrary {
    Fma,
    Sse2,
    None,
}

fn check(bits: u32, lib: CLibrary) -> Counts {
    let x = f32::from_bits(bits);
    let mut c = Counts::default();
    let new_fma = [arm::sinf(x), arm::cosf(x)];
    let old_fma = [glibc::sinf(x), glibc::cosf(x)];
    let new_sse2 = [arm::sse2::sinf(x), arm::sse2::cosf(x)];
    let old_sse2 = [
        glibc::verification::sinf_sse2(x),
        glibc::verification::cosf_sse2(x),
    ];
    for i in 0..2 {
        if !same(new_fma[i], old_fma[i]) {
            c.fma[i] += 1;
            c.first[0].get_or_insert(bits);
        }
        if !same(new_sse2[i], old_sse2[i]) {
            c.sse2[i] += 1;
            c.first[1].get_or_insert(bits);
        }
        c.builds[i] += u64::from(!same(new_fma[i], new_sse2[i]));
    }
    if lib != CLibrary::None {
        let libc = [x.sin(), x.cos()];
        let new = if lib == CLibrary::Fma {
            new_fma
        } else {
            new_sse2
        };
        for i in 0..2 {
            if !same(new[i], libc[i]) {
                c.c_library[i] += 1;
                c.first[2].get_or_insert(bits);
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
            .cloned()
    };
    let threads = value("--threads").map_or(0, |v| v.parse::<usize>().expect("a number"));
    if let Some(n) = value("--vectors") {
        print_vectors(n.parse().expect("a number"));
        return;
    }
    let lib = match value("--c-library").as_deref() {
        None | Some("fma") => CLibrary::Fma,
        Some("sse2") => CLibrary::Sse2,
        Some("none") => CLibrary::None,
        Some(other) => panic!("--c-library fma|sse2|none, not {other}"),
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("thread pool");
    let total = pool.install(|| {
        (0u32..=u16::MAX as u32)
            .into_par_iter()
            .map(|hi| {
                (0u32..=u16::MAX as u32)
                    .map(|lo| check((hi << 16) | lo, lib))
                    .fold(Counts::default(), Counts::add)
            })
            .reduce(Counts::default, Counts::add)
    });
    println!("inputs: all 2^32 floats, each function");
    for (i, name) in ["sinf", "cosf"].iter().enumerate() {
        println!(
            "{name}: Arm translation vs glibc translation, FMA build: {} differ; SSE2 build: {} differ \
             (the two builds differ from each other on {} inputs)",
            total.fma[i], total.sse2[i], total.builds[i]
        );
    }
    if lib != CLibrary::None {
        let build = if lib == CLibrary::Fma { "FMA" } else { "SSE2" };
        for (i, name) in ["sinf", "cosf"].iter().enumerate() {
            println!(
                "{name}: Arm translation ({build} build) vs this machine's C library: {} differ",
                total.c_library[i]
            );
        }
    }
    for (k, what) in ["FMA", "SSE2", "C library"].iter().enumerate() {
        if let Some(b) = total.first[k] {
            println!("first difference ({what}): 0x{b:08x}");
        }
    }
    let ok = total.fma == [0, 0] && total.sse2 == [0, 0] && total.c_library == [0, 0];
    println!("{}", if ok { "EQUAL ON EVERY INPUT" } else { "DIFFERENCES FOUND" });
    std::process::exit(if ok { 0 } else { 1 });
}

/// The C library's sinf and cosf at the branch limits and at up to `n` inputs in [0, 32] where
/// either is not correctly rounded, as `(input bits, sinf bits, cosf bits)`.
fn print_vectors(n: usize) {
    let near = |f: fn(f64) -> f64, x: f32| f(f64::from(x)) as f32;
    let mut picked = vec![
        0x3980_0000u32, // 0x1p-12
        0x3f49_0fdb,    // pi/4
        0x3f49_0fda,
        0x42f0_0000, // 120
        0x42ef_ffff,
    ];
    let (lo, hi) = (0.01f32.to_bits(), 32.0f32.to_bits());
    let step = ((hi - lo) / n.max(1) as u32).max(1);
    let mut b = lo;
    let start = picked.len();
    while b <= hi && picked.len() < start + n {
        for k in b..(b + step).min(hi + 1) {
            let x = f32::from_bits(k);
            if !same(x.sin(), near(larmorx_core::math::sin, x))
                || !same(x.cos(), near(larmorx_core::math::cos, x))
            {
                picked.push(k);
                break;
            }
        }
        b += step;
    }
    for k in picked {
        let x = f32::from_bits(k);
        println!(
            "        (0x{:04x}_{:04x}, 0x{:04x}_{:04x}, 0x{:04x}_{:04x}),",
            k >> 16,
            k & 0xffff,
            x.sin().to_bits() >> 16,
            x.sin().to_bits() & 0xffff,
            x.cos().to_bits() >> 16,
            x.cos().to_bits() & 0xffff
        );
    }
}
