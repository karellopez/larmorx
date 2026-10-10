// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! Bit-exact comparison of `larmorx_gpl_afni::csfft` with AFNI 25.2.09's compiled `csfft.c`.
//!
//! Usage: `csfft-verify <data dir>`, where the directory holds the files written by
//! `harness_cox` and `harness_inc` (see README.md). Exits 0 when everything is bit-exact.
#![forbid(unsafe_code)]

use larmorx_gpl_afni::csfft::{
    self, Complex32, CsfftPlan, csfft_cox, csfft_cox_handles, csfft_nextup, csfft_nextup_even,
    csfft_nextup_one35,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

fn read(path: &str) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn i32s(b: &[u8]) -> Vec<i32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| i32::from_le_bytes(*c))
        .collect()
}

fn f64s(b: &[u8]) -> Vec<f64> {
    b.as_chunks::<8>()
        .0
        .iter()
        .map(|c| f64::from_le_bytes(*c))
        .collect()
}

struct Record {
    mode: i32,
    n: usize,
    vec: i32,
    input: Vec<Complex32>,
    output: Vec<Complex32>,
}

fn records(b: &[u8]) -> Vec<Record> {
    let mut out = Vec::new();
    let mut p = 0;
    let rd_i32 = |p: &mut usize| {
        let v = i32::from_le_bytes(b[*p..*p + 4].try_into().unwrap());
        *p += 4;
        v
    };
    while p < b.len() {
        let mode = rd_i32(&mut p);
        let n = rd_i32(&mut p) as usize;
        let vec = rd_i32(&mut p);
        let cx = |p: &mut usize| {
            (0..n)
                .map(|_| {
                    let r = f32::from_le_bytes(b[*p..*p + 4].try_into().unwrap());
                    let i = f32::from_le_bytes(b[*p + 4..*p + 8].try_into().unwrap());
                    *p += 8;
                    Complex32::new(r, i)
                })
                .collect::<Vec<_>>()
        };
        let input = cx(&mut p);
        let output = cx(&mut p);
        out.push(Record {
            mode,
            n,
            vec,
            input,
            output,
        });
    }
    out
}

#[derive(Default)]
struct Tally {
    records: usize,
    values: u64,
    mismatches: u64,
    nan_values: u64,
    first: Vec<String>,
}

fn compare(t: &mut Tally, rec: &Record, got: &[Complex32]) {
    t.records += 1;
    for (k, (g, w)) in got.iter().zip(&rec.output).enumerate() {
        for (part, gv, wv) in [("r", g.r, w.r), ("i", g.i, w.i)] {
            t.values += 1;
            if wv.is_nan() {
                t.nan_values += 1;
            }
            if gv.to_bits() != wv.to_bits() {
                t.mismatches += 1;
                if t.first.len() < 10 {
                    t.first.push(format!(
                        "mode={} n={} vec={} k={k}.{part}: rust {gv:e} ({:#010x}) afni {wv:e} ({:#010x})",
                        rec.mode,
                        rec.n,
                        rec.vec,
                        gv.to_bits(),
                        wv.to_bits()
                    ));
                }
            }
        }
    }
}

fn main() {
    let dir = std::env::args().nth(1).expect("data dir");
    let f = |name: &str| format!("{dir}/{name}");

    // 1. csfft_nextup / csfft_nextup_one35 / csfft_nextup_even.
    let nu = i32s(&read(&f("nextup.bin")));
    let ne = i32s(&read(&f("nextup_even_internal.bin")));
    let (mut bad_nextup, mut bad_one35, mut bad_even, mut bad_even_ext) = (0, 0, 0, 0);
    for n in 1..=40000usize {
        let t = &nu[3 * (n - 1)..3 * n];
        bad_nextup += usize::from(csfft_nextup(n) != t[0] as usize);
        bad_one35 += usize::from(csfft_nextup_one35(n) != t[1] as usize);
        // AFNI's exported csfft_nextup_even (internal_check = 0) only rounds up to even.
        bad_even_ext += usize::from(n + n % 2 != t[2] as usize);
        bad_even += usize::from(csfft_nextup_even(n) != ne[n - 1] as usize);
    }
    println!(
        "nextup n=1..=40000: csfft_nextup mismatches {bad_nextup}, csfft_nextup_one35 {bad_one35}, \
         csfft_nextup_even(internal) {bad_even}; exported csfft_nextup_even == round-up-to-even mismatches {bad_even_ext}"
    );
    for n in [0usize, 1, 2, 3, 7, 11, 13, 17, 31, 33, 97, 101, 1000, 1025] {
        print!("nextup({n})={} ", csfft_nextup(n));
    }
    println!();
    for n in [3usize, 7, 13, 17, 100, 154, 204] {
        print!("one35({n})={} ", csfft_nextup_one35(n));
    }
    println!();

    // 2. csfft_cox on every AFNI-routed length.
    let recs = records(&read(&f("cox.bin")));
    let c_lengths: BTreeSet<usize> = recs.iter().map(|r| r.n).collect();
    let r_lengths: BTreeSet<usize> = (2..=32768).filter(|&n| csfft_cox_handles(n)).collect();
    println!(
        "routed lengths: AFNI {} / Rust {} / identical sets: {}",
        c_lengths.len(),
        r_lengths.len(),
        c_lengths == r_lengths
    );
    let tshift: BTreeSet<usize> = (3..=4000).map(|ntt| csfft_nextup_one35(ntt + 4)).collect();
    println!(
        "3dTshift lengths nextup_one35(ntt+4), ntt=3..=4000: {} distinct ({}..={}), all tested: {}",
        tshift.len(),
        tshift.first().unwrap(),
        tshift.last().unwrap(),
        tshift.is_subset(&c_lengths)
    );

    let mut plans: HashMap<usize, CsfftPlan> = HashMap::new();
    let mut tally = Tally::default();
    let mut by_mode: BTreeMap<i32, usize> = BTreeMap::new();
    let mut by_vec: BTreeMap<i32, u64> = BTreeMap::new();
    let mut scratch = Vec::new();
    for (idx, rec) in recs.iter().enumerate() {
        let mut x = rec.input.clone();
        if idx % 7 == 0 {
            csfft_cox(rec.mode, &mut x); // the convenience entry point
        } else {
            let plan = plans
                .entry(rec.n)
                .or_insert_with(|| CsfftPlan::new(rec.n).unwrap());
            scratch.resize(plan.scratch_len(), Complex32::default());
            plan.execute_with_scratch(rec.mode, &mut x, &mut scratch);
        }
        let before = tally.mismatches;
        compare(&mut tally, rec, &x);
        *by_mode.entry(rec.mode).or_default() += 1;
        *by_vec.entry(rec.vec).or_default() += tally.mismatches - before;
    }
    println!(
        "csfft_cox: {} records ({} lengths, modes {:?}), {} f32 values compared, {} bit mismatches \
         ({} NaN outputs); mismatches per vector kind {:?}",
        tally.records,
        c_lengths.len(),
        by_mode,
        tally.values,
        tally.mismatches,
        tally.nan_values,
        by_vec
    );
    for l in &tally.first {
        println!("  {l}");
    }

    // 3. The general power-of-two loop (verbatim C copy compiled with AFNI's flags).
    let recs = records(&read(&f("generic.bin")));
    let mut tg = Tally::default();
    for rec in &recs {
        let mut x = rec.input.clone();
        csfft::verification::radix2_general(rec.mode, &mut x);
        compare(&mut tg, rec, &x);
    }
    println!(
        "general radix-2 loop: {} records (n = 2..=65536), {} values, {} bit mismatches",
        tg.records, tg.values, tg.mismatches
    );
    for l in &tg.first {
        println!("  {l}");
    }

    // 4. The correctly rounded cos/sin (larmorx_core::math) vs glibc sincos on every table argument.
    let sc = f64s(&read(&f("sincos.bin")));
    let (mut n_args, mut d_cos, mut d_sin, mut f_cos, mut f_sin, mut max_ulp) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for t in sc.as_chunks::<3>().0 {
        let (a, c, s) = (t[0], t[1], t[2]);
        let (rc, rs) = (
            csfft::verification::table_cos(a),
            csfft::verification::table_sin(a),
        );
        n_args += 1;
        for (got, want, d, fl) in [
            (rc, c, &mut d_cos, &mut f_cos),
            (rs, s, &mut d_sin, &mut f_sin),
        ] {
            if got.to_bits() != want.to_bits() {
                *d += 1;
                max_ulp =
                    max_ulp.max((got.to_bits() as i64 - want.to_bits() as i64).unsigned_abs());
            }
            if (got as f32).to_bits() != (want as f32).to_bits() {
                *fl += 1;
            }
        }
    }
    println!(
        "sincos arguments: {n_args}; correctly rounded vs glibc differ in double: cos {d_cos}, sin {d_sin} (max {max_ulp} ulp); \
         after rounding to f32: cos {f_cos}, sin {f_sin}"
    );

    let ok = bad_nextup + bad_one35 + bad_even + bad_even_ext == 0
        && c_lengths == r_lengths
        && tally.mismatches == 0
        && tg.mismatches == 0
        && f_cos + f_sin == 0;
    println!(
        "{}",
        if ok {
            "ALL BIT-EXACT"
        } else {
            "MISMATCHES FOUND"
        }
    );
    std::process::exit(if ok { 0 } else { 1 });
}
