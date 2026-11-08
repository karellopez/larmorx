// SPDX-License-Identifier: Apache-2.0
//! Checks `larmorx_transform::numpy_linalg::lstsq` against `numpy.linalg.lstsq`'s own results,
//! written by `validation/investigations/t2smap/numerics/check_lstsq.py` (which runs this
//! program).
//!
//! `lstsq_check <dir> <threads>`: reads `<dir>/manifest.tsv` (one case per line: id, group,
//! m, nrhs, rcond bits or `none`, LAPACK release, numpy's rank or `err`, then the files of a,
//! b (column-major), x, residuals and s, raw little-endian float64), computes each case at one
//! thread and at `<threads>` threads, and compares bit for bit; prints one line per group
//! (cases, identical) and every differing case (how many columns of x differ); with
//! `LSTSQ_CHECK_TIMES` set, each case's time at one and at `<threads>` threads.

use larmorx_transform::numpy_linalg::{LapackRelease, LstsqError, lstsq};
use std::collections::BTreeMap;
use std::path::Path;

fn read_f64(p: &Path) -> Vec<f64> {
    std::fs::read(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| f64::from_le_bytes(*c))
        .collect()
}

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = Path::new(&args[1]);
    let threads: usize = args.get(2).map_or(4, |t| t.parse().unwrap());
    let text = std::fs::read_to_string(dir.join("manifest.tsv")).unwrap();
    let mut groups: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut failures = 0usize;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let (id, group) = (f[0], f[1]);
        let m: usize = f[2].parse().unwrap();
        let nrhs: usize = f[3].parse().unwrap();
        let rcond = (f[4] != "none").then(|| f64::from_bits(f[4].parse().unwrap()));
        let release = match f[5] {
            "3.12.0" => LapackRelease::V3_12_0,
            "3.12.1" => LapackRelease::V3_12_1,
            r => panic!("release {r}"),
        };
        let a = read_f64(&dir.join(f[7]));
        let b = read_f64(&dir.join(f[8]));
        let run = |t: usize| {
            let mut bb = b.clone();
            lstsq(&a, m, 2, &mut bb, nrhs, rcond, release, t)
        };
        let t0 = std::time::Instant::now();
        let one = run(1);
        let t1 = t0.elapsed();
        let many = run(threads);
        if std::env::var_os("LSTSQ_CHECK_TIMES").is_some() {
            println!(
                "  time {id}: 1 thread {:.3} s, {threads} threads {:.3} s (b copied each time)",
                t1.as_secs_f64(),
                (t0.elapsed() - t1).as_secs_f64()
            );
        }
        let entry = groups.entry(group.to_string()).or_default();
        entry.0 += 1;
        let verdict: Result<(), String> = match (f[6], &one) {
            ("err", Err(LstsqError::DidNotConverge)) => Ok(()),
            ("err", other) => Err(format!("numpy raised LinAlgError, larmorx {other:?}")),
            (_, Err(e)) => Err(format!("larmorx failed: {e}")),
            (rank, Ok(out)) => {
                let want_x = read_f64(&dir.join(f[9]));
                let want_r = read_f64(&dir.join(f[10]));
                let want_s = read_f64(&dir.join(f[11]));
                let mut problems = Vec::new();
                if out.rank.to_string() != rank {
                    problems.push(format!("rank {} vs numpy {rank}", out.rank));
                }
                if !same(&out.x, &want_x) {
                    let cols = (0..nrhs)
                        .filter(|&j| {
                            (0..2).any(|i| {
                                out.x[i * nrhs + j].to_bits() != want_x[i * nrhs + j].to_bits()
                            })
                        })
                        .count();
                    let first = (0..nrhs).find(|&j| {
                        (0..2).any(|i| {
                            out.x[i * nrhs + j].to_bits() != want_x[i * nrhs + j].to_bits()
                        })
                    });
                    let detail = first.map_or(String::new(), |j| {
                        format!(
                            " (first column {j}: larmorx {:e} {:e}, numpy {:e} {:e})",
                            out.x[j],
                            out.x[nrhs + j],
                            want_x[j],
                            want_x[nrhs + j]
                        )
                    });
                    problems.push(format!("x: {cols} of {nrhs} columns differ{detail}"));
                }
                if !same(&out.residuals, &want_r) {
                    problems.push(format!(
                        "residuals differ (lengths {} vs {})",
                        out.residuals.len(),
                        want_r.len()
                    ));
                }
                if !same(&out.s, &want_s) {
                    problems.push(format!("s {:?} vs numpy {:?}", out.s, want_s));
                }
                if problems.is_empty() {
                    Ok(())
                } else {
                    Err(problems.join("; "))
                }
            }
        };
        let threads_agree = match (&one, &many) {
            (Ok(p), Ok(q)) => same(&p.x, &q.x) && same(&p.residuals, &q.residuals),
            (Err(_), Err(_)) => true,
            _ => false,
        };
        match verdict {
            Ok(()) if threads_agree => entry.1 += 1,
            Ok(()) => {
                failures += 1;
                println!(
                    "  DIFF {id} ({group}, m {m}, nrhs {nrhs}): {threads} threads differ from 1"
                );
            }
            Err(msg) => {
                failures += 1;
                println!("  DIFF {id} ({group}, m {m}, nrhs {nrhs}): {msg}");
            }
        }
    }
    for (g, (n, ok)) in &groups {
        println!("lstsq {g}: {ok}/{n} identical");
    }
    println!("LSTSQ-CHECK-DONE failures {failures}");
}
