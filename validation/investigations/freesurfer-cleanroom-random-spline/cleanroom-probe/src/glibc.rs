// SPDX-License-Identifier: Apache-2.0
//! The clean room of the C library's `rand`, `srand`, `random` and `srandom`
//! (`larmorx_core::glibc_random`, `specs/glibc-random.md`) behind the commands of the black-box
//! probe `validation/investigations/glibc-random-probe/glibc_random_probe.c`: `glibc <command>`
//! prints what the probe's `<command>` prints for the same arguments (`rand`, `random`, `default`,
//! `shared`, `hash`, `long`, `raw`).
//!
//! Two more commands for the validation: `every <seed0> <nseeds> <values> <block> <threads>` prints
//! one line `<first seed> <hash>` per block of `<block>` seeds (the hash the probe's `hash <first
//! seed> <block> <values>` prints; the blocks run on `<threads>` threads and print in order), and
//! `bench <seed> <count>` (the speed of drawing and of seeding, nothing else printed).

use larmorx_core::glibc_random::GlibcRandom;
use std::io::Write;
use std::time::Instant;

const FNV0: u64 = 14_695_981_039_346_656_037;
const FNV_PRIME: u64 = 1_099_511_628_211;

fn fnv(h: u64, v: u32) -> u64 {
    (h ^ u64::from(v)).wrapping_mul(FNV_PRIME)
}

/// A seed as the probe reads it (`strtoll`, decimal here; out of range saturated as `strtoll`
/// does), converted to `unsigned int` modulo 2^32 as C converts a `long`.
fn seed_arg(s: &str) -> Result<u32, String> {
    let wide: i128 = s.parse().map_err(|e| format!("bad seed {s}: {e}"))?;
    let v = wide.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
    Ok(v as u32)
}

fn count_arg(s: &str) -> Result<u64, String> {
    s.parse().map_err(|e| format!("bad count {s}: {e}"))
}

/// The hash of the first `values` values of the seeds `seed0`, `seed0 + 1`, ... (`nseeds` of
/// them, modulo 2^32).
fn block_hash(seed0: u32, nseeds: u64, values: u64) -> u64 {
    let mut h = FNV0;
    let mut g = GlibcRandom::new();
    for k in 0..nseeds {
        g.seed(seed0.wrapping_add(k as u32));
        for _ in 0..values {
            h = fnv(h, g.next_u31());
        }
    }
    h
}

pub fn run(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let usage = "usage: cleanroom-probe glibc rand|random <seed> <count> | default <count> | shared <seed> <count> | hash|raw <seed0> <nseeds> <values> | long <seed> <count> | every <seed0> <nseeds> <values> <block> <threads> | bench <seed> <count>";
    let w = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
    match (args.first().map(String::as_str), args.len()) {
        (Some("rand"), 3) => {
            let mut g = GlibcRandom::seeded(seed_arg(&args[1])?);
            for _ in 0..count_arg(&args[2])? {
                w(writeln!(out, "{}", g.rand()))?;
            }
        }
        (Some("random"), 3) => {
            let mut g = GlibcRandom::seeded(seed_arg(&args[1])?);
            for _ in 0..count_arg(&args[2])? {
                w(writeln!(out, "{}", g.random()))?;
            }
        }
        (Some("default"), 2) => {
            let mut g = GlibcRandom::new();
            for _ in 0..count_arg(&args[1])? {
                w(writeln!(out, "{}", g.rand()))?;
            }
        }
        (Some("shared"), 3) => {
            let s = seed_arg(&args[1])?;
            let n = count_arg(&args[2])?;
            let mut g = GlibcRandom::seeded(s);
            for i in 0..n {
                let v = if i % 2 == 1 {
                    g.random()
                } else {
                    i64::from(g.rand())
                };
                w(writeln!(out, "{v}"))?;
            }
            g.seed(s); // srand(s)
            g.seed(s); // srandom(s)
            for _ in 0..n {
                w(writeln!(out, "{}", g.rand()))?;
            }
            g.seed(s); // srandom(s)
            for _ in 0..n {
                w(writeln!(out, "{}", g.rand()))?;
            }
        }
        (Some("hash"), 4) => {
            let h = block_hash(
                seed_arg(&args[1])?,
                count_arg(&args[2])?,
                count_arg(&args[3])?,
            );
            w(writeln!(out, "{h:016x}"))?;
        }
        (Some("raw"), 4) => {
            let s0 = seed_arg(&args[1])?;
            let nv = count_arg(&args[3])?;
            let mut g = GlibcRandom::new();
            for k in 0..count_arg(&args[2])? {
                g.seed(s0.wrapping_add(k as u32));
                for _ in 0..nv {
                    w(out.write_all(&g.next_u31().to_le_bytes()))?;
                }
            }
        }
        (Some("long"), 3) => {
            let mut g = GlibcRandom::seeded(seed_arg(&args[1])?);
            let h = (0..count_arg(&args[2])?).fold(FNV0, |h, _| fnv(h, g.next_u31()));
            w(writeln!(out, "{h:016x}"))?;
        }
        (Some("every"), 6) => {
            let s0 = seed_arg(&args[1])?;
            let nseeds = count_arg(&args[2])?;
            let nv = count_arg(&args[3])?;
            let block = count_arg(&args[4])?.max(1);
            let threads = count_arg(&args[5])?.max(1) as usize;
            let nblocks = nseeds.div_ceil(block);
            let mut hashes = vec![0u64; nblocks as usize];
            let chunk = hashes.len().div_ceil(threads).max(1);
            std::thread::scope(|scope| {
                for (t, part) in hashes.chunks_mut(chunk).enumerate() {
                    scope.spawn(move || {
                        for (i, h) in part.iter_mut().enumerate() {
                            let b = (t * chunk + i) as u64;
                            let n = block.min(nseeds - b * block);
                            *h = block_hash(s0.wrapping_add((b * block) as u32), n, nv);
                        }
                    });
                }
            });
            for (b, h) in hashes.iter().enumerate() {
                let first = s0.wrapping_add((b as u64 * block) as u32);
                w(writeln!(out, "{first} {h:016x}"))?;
            }
        }
        (Some("bench"), 3) => {
            let seed = seed_arg(&args[1])?;
            let n = count_arg(&args[2])?;
            let t0 = Instant::now();
            let mut g = GlibcRandom::seeded(seed);
            let h = (0..n).fold(FNV0, |h, _| fnv(h, g.next_u31()));
            let draw = t0.elapsed().as_secs_f64();
            let t1 = Instant::now();
            let hs = block_hash(0, 1 << 20, 8);
            let seeding = t1.elapsed().as_secs_f64();
            w(writeln!(
                out,
                "{n} values (long): {draw:.3} s, {:.2} ns each (hash {h:016x}); 2^20 seedings with 8 values (hash 0 1048576 8): {seeding:.3} s, {:.0} ns each (hash {hs:016x})",
                draw * 1e9 / n as f64,
                seeding * 1e9 / f64::from(1u32 << 20)
            ))?;
        }
        _ => return Err(usage.to_string()),
    }
    w(out.flush())
}
