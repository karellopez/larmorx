// SPDX-License-Identifier: LicenseRef-FSL
// Rust translation Copyright 2026 Karel Lopez Vilaret. Distributed under the FSL Licence (see
// LICENSE).
//! The replica's side of `validation/probes/fsl-avwutils-lib/probe.cc`: for `arma.txt` and
//! `cprob.txt` under `AVW_PROBE_ROOT`, writes `arma.replica` and `cprob.replica` with the lines
//! the probe prints from FSL's own code (Armadillo 14.2.1's `max`/`min`: `arma::direct_max`,
//! `arma::direct_min`; fsl-cprob's `ndtr`, `ndtri`, `erf`, `erfc` with their error reports:
//! `cprob`). Run with `--ignored`.

use larmorx_nc_core::{arma, cprob};
use std::fmt::Write as _;

fn put(out: &mut String, tag: &str, v: f64) {
    writeln!(out, "{tag} {:016x}", v.to_bits()).unwrap();
}

fn values(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .map(|w| f64::from_bits(u64::from_str_radix(w, 16).unwrap()))
        .collect()
}

#[test]
#[ignore]
fn avwutils_probe() {
    let root = std::path::PathBuf::from(std::env::var("AVW_PROBE_ROOT").expect("AVW_PROBE_ROOT"));
    if let Ok(text) = std::fs::read_to_string(root.join("arma.txt")) {
        let mut out = String::new();
        for line in text.lines() {
            let v = values(line);
            if v.is_empty() {
                continue;
            }
            for tag in ["max", "min", "mmax", "mmin"] {
                let r = if tag.ends_with("max") {
                    arma::direct_max(&v)
                } else {
                    arma::direct_min(&v)
                };
                put(&mut out, tag, r);
            }
        }
        std::fs::write(root.join("arma.replica"), out).unwrap();
    }
    if let Ok(text) = std::fs::read_to_string(root.join("cprob.txt")) {
        let mut out = String::new();
        for line in text.lines() {
            let x = values(line)[0];
            put(&mut out, "ndtr", cprob::ndtr(x));
            for (tag, (r, msg)) in [
                ("ndtri", cprob::ndtri(x)),
                ("erf", cprob::erf(x)),
                ("erfc", cprob::erfc(x)),
            ] {
                if let Some(m) = msg {
                    out.push_str(&m);
                }
                put(&mut out, tag, r);
            }
        }
        std::fs::write(root.join("cprob.replica"), out).unwrap();
    }
}
