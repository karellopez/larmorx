// SPDX-License-Identifier: Apache-2.0
//! Checks `larmorx_core::numpy` and `larmorx_core::math::{logf_numpy, exp_glibc, log_glibc}`
//! against numpy's own results, written by
//! `validation/investigations/t2smap/numerics/check_numerics.py` (which runs this program).
//!
//! `numpy_check cases <dir>`: reads `<dir>/manifest.tsv` (one case per line: id, function,
//! input type, shape, parameter, input files, expected file; raw little-endian arrays), computes
//! each case and compares bit for bit; prints one line per function (cases, identical) and every
//! differing case.
//! `numpy_check logf <tsv> <threads>`: for each line (chunk start, length, numpy's digest),
//! the same digest of `logf_numpy` over that chunk of `u32` bit patterns.

use larmorx_core::math::{exp, exp_glibc, log, log_glibc, logf, logf_numpy};
use larmorx_core::numpy::quantile::{
    nanpercentile_linear_axis0, percentile_higher, scoreatpercentile_lower,
};
use larmorx_core::numpy::reduce::{
    add_reduce_axis, average_axis, mean_axis, nanstd_axis, nansum_axis, std_axis, var_axis,
};
use larmorx_core::numpy::umath::log_f64;
use std::collections::BTreeMap;
use std::path::Path;

fn read_f32(p: &Path) -> Vec<f32> {
    std::fs::read(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

fn read_f64(p: &Path) -> Vec<f64> {
    std::fs::read(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| f64::from_le_bytes(*c))
        .collect()
}

fn bytes_f32(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn bytes_f64(v: &[f64]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// The digest the Python side computes: the wrapping sum of `bits(log(x_i)) * (2 i + 1)`.
fn digest(start: u64, len: u64, f: impl Fn(f32) -> f32) -> u64 {
    let mut d = 0u64;
    for i in start..start + len {
        let y = f(f32::from_bits(i as u32)).to_bits() as u64;
        d = d.wrapping_add(y.wrapping_mul(2 * i + 1));
    }
    d
}

fn logf_mode(tsv: &Path, threads: usize) {
    let text = std::fs::read_to_string(tsv).unwrap();
    let lines: Vec<(u64, u64, u64)> = text
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let f: Vec<u64> = l.split('\t').map(|x| x.parse().unwrap()).collect();
            (f[0], f[1], f[2])
        })
        .collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&(start, len, want)) = lines.get(k) else {
                        break;
                    };
                    let got = digest(start, len, logf_numpy);
                    let cr = digest(start, len, logf);
                    results.lock().unwrap().push((start, len, want, got, cr));
                }
            });
        }
    });
    let mut r = results.into_inner().unwrap();
    r.sort();
    let same = r.iter().filter(|x| x.2 == x.3).count();
    let cr_same = r.iter().filter(|x| x.2 == x.4).count();
    let total: u64 = r.iter().map(|x| x.1).sum();
    println!(
        "logf_numpy: {same}/{} chunks identical ({total} inputs); correctly rounded logf: {cr_same} chunks identical",
        r.len()
    );
    for x in r.iter().filter(|x| x.2 != x.3) {
        println!("  DIFF chunk start {:#010x} len {}", x.0, x.1);
    }
}

/// Counts of differing results among numpy's kernel, glibc's `logf` and the correctly rounded
/// one over the positive floats (all, and `[1, 2^24]`: `log(|x| + 1)` of MRI data), with the
/// largest distance in units in the last place between numpy's and the correctly rounded.
fn logf_stats(threads: usize) {
    let ranges = [
        ("positive finite", 1u32, 0x7f80_0000u32),
        ("[1, 2^24]", 0x3f80_0000, 0x4b80_0001),
    ];
    for (name, lo, hi) in ranges {
        let step = (hi - lo).div_ceil(threads as u32);
        let parts: Vec<[u64; 4]> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..threads as u32)
                .map(|t| {
                    s.spawn(move || {
                        let mut c = [0u64; 4];
                        let a = lo + t * step;
                        let b = (a.saturating_add(step)).min(hi);
                        for bits in a..b {
                            let x = f32::from_bits(bits);
                            let (n, g, r) =
                                (logf_numpy(x), larmorx_core::math::logf_glibc(x), logf(x));
                            c[0] += u64::from(n.to_bits() != r.to_bits());
                            c[1] += u64::from(n.to_bits() != g.to_bits());
                            c[2] += u64::from(g.to_bits() != r.to_bits());
                            let ulp = (n.to_bits() as i64 - r.to_bits() as i64).unsigned_abs();
                            if (n < 0.0) == (r < 0.0) {
                                c[3] = c[3].max(ulp);
                            }
                        }
                        c
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let mut c = [0u64; 4];
        for p in parts {
            c[0] += p[0];
            c[1] += p[1];
            c[2] += p[2];
            c[3] = c[3].max(p[3]);
        }
        let total = u64::from(hi - lo);
        println!(
            "logf over {name} ({total} inputs): numpy vs correctly rounded {} differ (max {} ulp), numpy vs glibc {}, glibc vs correctly rounded {}",
            c[0], c[3], c[1], c[2]
        );
    }
}

fn cases_mode(dir: &Path) {
    let manifest = std::fs::read_to_string(dir.join("manifest.tsv")).unwrap();
    let mut stats: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut diffs = Vec::new();
    for line in manifest.lines().filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let (id, func, dtype, shape, param, in1, in2, out) =
            (f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7]);
        let shape: Vec<usize> = shape.split(',').map(|x| x.parse().unwrap()).collect();
        let s3 = || [shape[0], shape[1], shape[2]];
        let expected = std::fs::read(dir.join(out)).unwrap();
        let got: Vec<u8> = match (func, dtype) {
            ("sum_axis", "f32") => bytes_f32(&add_reduce_axis(&read_f32(&dir.join(in1)), s3())),
            ("sum_axis", "f64") => bytes_f64(&add_reduce_axis(&read_f64(&dir.join(in1)), s3())),
            ("mean_axis", "f32") => bytes_f32(&mean_axis(&read_f32(&dir.join(in1)), s3())),
            ("mean_axis", "f64") => bytes_f64(&mean_axis(&read_f64(&dir.join(in1)), s3())),
            ("var_axis", "f32") => bytes_f32(&var_axis(
                &read_f32(&dir.join(in1)),
                s3(),
                param.parse().unwrap(),
            )),
            ("std_axis", "f32") => bytes_f32(&std_axis(
                &read_f32(&dir.join(in1)),
                s3(),
                param.parse().unwrap(),
            )),
            ("std_axis", "f64") => bytes_f64(&std_axis(
                &read_f64(&dir.join(in1)),
                s3(),
                param.parse().unwrap(),
            )),
            ("nansum_axis", "f32") => bytes_f32(&nansum_axis(&read_f32(&dir.join(in1)), s3())),
            ("nanstd_axis", "f32") => bytes_f32(
                &nanstd_axis(&read_f32(&dir.join(in1)), s3(), param.parse().unwrap()).values,
            ),
            ("average_axis", "f32") => {
                match average_axis(&read_f32(&dir.join(in1)), &read_f64(&dir.join(in2)), s3()) {
                    Ok(v) => bytes_f64(&v),
                    Err(_) => b"ZeroDivisionError".to_vec(),
                }
            }
            ("pct_higher", "f32") => {
                match percentile_higher(&read_f32(&dir.join(in1)), param.parse().unwrap()) {
                    Some(v) => bytes_f32(&[v]),
                    None => b"error".to_vec(),
                }
            }
            ("nanpct_linear", "f32") => {
                let q: Vec<f64> = param.split(',').map(|x| x.parse().unwrap()).collect();
                match nanpercentile_linear_axis0(&read_f32(&dir.join(in1)), shape[0], shape[1], &q)
                {
                    Some(p) if p.float32 => {
                        let v: Vec<f32> = p.values.iter().map(|&x| x as f32).collect();
                        let mut b = b"f32:".to_vec();
                        b.extend(bytes_f32(&v));
                        b
                    }
                    Some(p) => {
                        let mut b = b"f64:".to_vec();
                        b.extend(bytes_f64(&p.values));
                        b
                    }
                    None => b"error".to_vec(),
                }
            }
            ("score_lower", "f64") => {
                match scoreatpercentile_lower(&read_f64(&dir.join(in1)), param.parse().unwrap()) {
                    Some(v) => bytes_f64(&[v]),
                    None => b"error".to_vec(),
                }
            }
            ("exp_glibc", "f64") => bytes_f64(
                &read_f64(&dir.join(in1))
                    .into_iter()
                    .map(exp_glibc)
                    .collect::<Vec<_>>(),
            ),
            ("log_glibc", "f64") => bytes_f64(
                &read_f64(&dir.join(in1))
                    .into_iter()
                    .map(log_glibc)
                    .collect::<Vec<_>>(),
            ),
            ("log_numpy", "f64") => bytes_f64(
                &read_f64(&dir.join(in1))
                    .into_iter()
                    .map(log_f64)
                    .collect::<Vec<_>>(),
            ),
            ("exp_cr", "f64") => bytes_f64(
                &read_f64(&dir.join(in1))
                    .into_iter()
                    .map(exp)
                    .collect::<Vec<_>>(),
            ),
            ("log_cr", "f64") => bytes_f64(
                &read_f64(&dir.join(in1))
                    .into_iter()
                    .map(log)
                    .collect::<Vec<_>>(),
            ),
            ("logf_numpy", "f32") => bytes_f32(
                &read_f32(&dir.join(in1))
                    .into_iter()
                    .map(logf_numpy)
                    .collect::<Vec<_>>(),
            ),
            _ => panic!("unknown case {func} {dtype}"),
        };
        let group = id.split('/').next().unwrap_or("");
        let e = stats.entry(format!("{group} {func} {dtype}")).or_default();
        e.0 += 1;
        if got == expected {
            e.1 += 1;
        } else {
            let unit = if dtype == "f32" && !func.starts_with("average") {
                4
            } else {
                8
            };
            let differing = got
                .chunks(unit)
                .zip(expected.chunks(unit))
                .filter(|(a, b)| a != b)
                .count();
            diffs.push(format!("  DIFF {id} {func} {dtype} shape {shape:?} param {param}: {differing} values differ (lengths {} vs {})", got.len(), expected.len()));
        }
    }
    for (k, (n, same)) in &stats {
        println!("{k}: {same}/{n} identical");
    }
    for d in diffs.iter().take(40) {
        println!("{d}");
    }
    if diffs.len() > 40 {
        println!("  ... {} more", diffs.len() - 40);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("cases") => cases_mode(Path::new(&args[2])),
        Some("logf-stats") => logf_stats(args.get(2).map_or(1, |t| t.parse().unwrap())),
        Some("logf") => logf_mode(
            Path::new(&args[2]),
            args.get(3).map_or(1, |t| t.parse().unwrap()),
        ),
        _ => {
            eprintln!("usage: numpy_check cases <dir> | numpy_check logf <tsv> [threads]");
            std::process::exit(2);
        }
    }
}
